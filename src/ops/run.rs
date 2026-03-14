use crate::exit::{EXIT_GENERAL, ExitError};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::cmp::Reverse;
use std::fs;
use std::path::{Path, PathBuf};
use time::format_description;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunMeta {
    pub run_id: String,
    pub created_at: String,
    pub command: String,
    pub args: Vec<String>,
    pub git_root: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunSummary {
    pub run_id: String,
    pub created_at: String,
    pub command: String,
    pub effective_base_commit: Option<String>,
    pub promoted_head: Option<String>,
}

pub fn runs_dir(git_root: &Path) -> PathBuf {
    git_root.join(".diffship").join("runs")
}

pub fn run_dir(git_root: &Path, run_id: &str) -> PathBuf {
    runs_dir(git_root).join(run_id)
}

pub fn create_run(
    git_root: &Path,
    command: &str,
    args: &[String],
    created_at: String,
) -> Result<RunMeta, ExitError> {
    let runs_dir = runs_dir(git_root);
    fs::create_dir_all(&runs_dir)
        .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to create runs dir: {e}")))?;

    let run_id = next_run_id(&runs_dir)?;
    let dir = run_dir(git_root, &run_id);

    fs::create_dir_all(&dir)
        .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to create run dir: {e}")))?;

    let meta = RunMeta {
        run_id: run_id.clone(),
        created_at,
        command: command.to_string(),
        args: args.to_vec(),
        git_root: git_root.display().to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    };

    let bytes = serde_json::to_vec_pretty(&meta)
        .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to encode run meta: {e}")))?;
    fs::write(dir.join("run.json"), bytes)
        .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to write run.json: {e}")))?;

    Ok(meta)
}

pub fn list_runs(git_root: &Path, limit: usize) -> Result<Vec<RunSummary>, ExitError> {
    let dir = runs_dir(git_root);
    if !dir.exists() {
        return Ok(vec![]);
    }

    let mut summaries = vec![];

    for ent in fs::read_dir(&dir)
        .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to read runs dir: {e}")))?
    {
        let ent = ent.map_err(|e| {
            ExitError::new(EXIT_GENERAL, format!("failed to read runs dir entry: {e}"))
        })?;
        if !ent.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }

        let run_json = ent.path().join("run.json");
        let Ok(bytes) = fs::read(&run_json) else {
            continue;
        };
        let Ok(meta) = serde_json::from_slice::<RunMeta>(&bytes) else {
            continue;
        };

        let (effective_base_commit, promoted_head) = read_head_fields(&ent.path());
        summaries.push(RunSummary {
            run_id: meta.run_id,
            created_at: meta.created_at,
            command: meta.command,
            effective_base_commit,
            promoted_head,
        });
    }

    // RFC3339 is lexicographically sortable. (We also include run_id as a stable tie-breaker.)
    summaries.sort_by_key(|s| (Reverse(s.created_at.clone()), s.run_id.clone()));

    if summaries.len() > limit {
        summaries.truncate(limit);
    }

    Ok(summaries)
}

fn read_head_fields(run_dir: &Path) -> (Option<String>, Option<String>) {
    let apply = read_json_value(run_dir.join("apply.json"));
    let promotion = read_json_value(run_dir.join("promotion.json"));

    let effective_base_commit = apply
        .as_ref()
        .and_then(|v| v.get("effective_base_commit"))
        .and_then(|v| v.as_str())
        .map(ToOwned::to_owned)
        .or_else(|| {
            apply
                .as_ref()
                .and_then(|v| v.get("base_commit"))
                .and_then(|v| v.as_str())
                .map(ToOwned::to_owned)
        });
    let promoted_head = promotion
        .as_ref()
        .and_then(|v| v.get("promoted_head"))
        .and_then(|v| v.as_str())
        .map(ToOwned::to_owned);

    (effective_base_commit, promoted_head)
}

fn read_json_value(path: PathBuf) -> Option<Value> {
    let bytes = fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn next_run_id(runs_dir: &Path) -> Result<String, ExitError> {
    let base = format!("run_{}", current_run_timestamp()?);
    let mut run_id = base.clone();
    let mut suffix = 2usize;
    while runs_dir.join(&run_id).exists() {
        run_id = format!("{}_{}", base, suffix);
        suffix += 1;
    }
    Ok(run_id)
}

fn current_run_timestamp() -> Result<String, ExitError> {
    let now = current_local_time().unwrap_or_else(|_| time::OffsetDateTime::now_utc());
    format_run_timestamp(now)
}

fn current_local_time() -> Result<time::OffsetDateTime, ExitError> {
    let offset = time::UtcOffset::current_local_offset().map_err(|e| {
        ExitError::new(
            EXIT_GENERAL,
            format!("failed to detect local time offset: {e}"),
        )
    })?;
    Ok(time::OffsetDateTime::now_utc().to_offset(offset))
}

fn format_run_timestamp(now: time::OffsetDateTime) -> Result<String, ExitError> {
    let fmt = format_description::parse("[year]-[month]-[day]_[hour][minute][second]")
        .map_err(|e| ExitError::new(EXIT_GENERAL, format!("invalid run time format: {e}")))?;
    now.format(&fmt)
        .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to format run id: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_run_id_uses_human_readable_timestamp_and_suffixes_collisions() {
        let td = tempfile::tempdir().unwrap();
        let dir = td.path();

        let first = next_run_id(dir).unwrap();
        assert!(first.starts_with("run_20"));
        fs::create_dir_all(dir.join(&first)).unwrap();

        let second = next_run_id(dir).unwrap();
        assert_eq!(second, format!("{}_2", first));
    }
}

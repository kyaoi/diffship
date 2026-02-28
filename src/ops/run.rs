use crate::exit::{EXIT_GENERAL, ExitError};
use serde::{Deserialize, Serialize};
use std::cmp::Reverse;
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

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
    fs::create_dir_all(runs_dir(git_root))
        .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to create runs dir: {e}")))?;

    let run_id = format!("run_{}", Uuid::new_v4());
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

        summaries.push(RunSummary {
            run_id: meta.run_id,
            created_at: meta.created_at,
            command: meta.command,
        });
    }

    // RFC3339 is lexicographically sortable. (We also include run_id as a stable tie-breaker.)
    summaries.sort_by_key(|s| (Reverse(s.created_at.clone()), s.run_id.clone()));

    if summaries.len() > limit {
        summaries.truncate(limit);
    }

    Ok(summaries)
}

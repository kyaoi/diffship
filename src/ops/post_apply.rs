use crate::exit::{EXIT_GENERAL, ExitError};
use crate::ops::command_log;
use serde::Serialize;
use std::fs;
use std::path::Path;

#[derive(Debug, Serialize)]
struct PostApplySummary {
    created_at: String,
    ok: bool,
    commands: Vec<PostApplyCommandResult>,
}

#[derive(Debug, Serialize)]
struct PostApplyCommandResult {
    name: String,
    argv: Vec<String>,
    status: i32,
    duration_ms: u128,
    stdout_path: String,
    stderr_path: String,
}

#[derive(Debug, Clone)]
pub struct PostApplyOut {
    pub ok: bool,
    pub logs_path: String,
}

pub fn run(
    run_dir: &Path,
    sandbox_path: &Path,
    commands: &[String],
    created_at: &str,
) -> Result<PostApplyOut, ExitError> {
    let out_dir = run_dir.join("post-apply");
    fs::create_dir_all(&out_dir).map_err(|e| {
        ExitError::new(
            EXIT_GENERAL,
            format!("failed to create post-apply dir: {e}"),
        )
    })?;

    let mut results = vec![];
    let mut ok = true;

    for (idx, cmd) in commands.iter().enumerate() {
        let name = format!("cmd{}", idx + 1);
        let file_stem = format!("{:02}_{}", idx + 1, sanitize_name(&name));
        let argv = vec!["sh".to_string(), "-lc".to_string(), cmd.to_string()];
        let logged =
            command_log::run_and_log(run_dir, "post-apply", &file_stem, sandbox_path, &argv, None)?;
        if logged.record.status != 0 {
            ok = false;
        }
        results.push(PostApplyCommandResult {
            name: name.clone(),
            argv,
            status: logged.record.status,
            duration_ms: logged.record.duration_ms,
            stdout_path: logged.record.stdout_path.clone(),
            stderr_path: logged.record.stderr_path.clone(),
        });
    }

    let summary = PostApplySummary {
        created_at: created_at.to_string(),
        ok,
        commands: results,
    };
    let bytes = serde_json::to_vec_pretty(&summary).map_err(|e| {
        ExitError::new(
            EXIT_GENERAL,
            format!("failed to encode post-apply summary: {e}"),
        )
    })?;
    fs::write(run_dir.join("post_apply.json"), bytes).map_err(|e| {
        ExitError::new(
            EXIT_GENERAL,
            format!("failed to write post_apply.json: {e}"),
        )
    })?;

    Ok(PostApplyOut {
        ok,
        logs_path: out_dir.display().to_string(),
    })
}

fn sanitize_name(s: &str) -> String {
    command_log::sanitize_name(s)
}

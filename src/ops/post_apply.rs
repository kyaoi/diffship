use crate::exit::{EXIT_GENERAL, ExitError};
use serde::Serialize;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::Instant;

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
        let stdout_path = out_dir.join(format!("{}.stdout", file_stem));
        let stderr_path = out_dir.join(format!("{}.stderr", file_stem));

        let start = Instant::now();
        let output = Command::new("sh")
            .args(["-lc", cmd])
            .current_dir(sandbox_path)
            .output();
        let duration_ms = start.elapsed().as_millis();

        match output {
            Ok(out) => {
                let status = out.status.code().unwrap_or(1);
                if status != 0 {
                    ok = false;
                }
                fs::write(&stdout_path, &out.stdout).map_err(|e| {
                    ExitError::new(EXIT_GENERAL, format!("failed to write stdout: {e}"))
                })?;
                fs::write(&stderr_path, &out.stderr).map_err(|e| {
                    ExitError::new(EXIT_GENERAL, format!("failed to write stderr: {e}"))
                })?;

                results.push(PostApplyCommandResult {
                    name: name.clone(),
                    argv: vec!["sh".to_string(), "-lc".to_string(), cmd.to_string()],
                    status,
                    duration_ms,
                    stdout_path: stdout_path
                        .strip_prefix(run_dir)
                        .unwrap_or(&stdout_path)
                        .display()
                        .to_string(),
                    stderr_path: stderr_path
                        .strip_prefix(run_dir)
                        .unwrap_or(&stderr_path)
                        .display()
                        .to_string(),
                });
            }
            Err(e) => {
                ok = false;
                fs::write(&stderr_path, format!("failed to spawn command: {e}\n")).ok();
                results.push(PostApplyCommandResult {
                    name: name.clone(),
                    argv: vec!["sh".to_string(), "-lc".to_string(), cmd.to_string()],
                    status: 1,
                    duration_ms,
                    stdout_path: stdout_path
                        .strip_prefix(run_dir)
                        .unwrap_or(&stdout_path)
                        .display()
                        .to_string(),
                    stderr_path: stderr_path
                        .strip_prefix(run_dir)
                        .unwrap_or(&stderr_path)
                        .display()
                        .to_string(),
                });
            }
        }
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
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

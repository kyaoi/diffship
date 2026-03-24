use crate::cli::VerifyArgs;
use crate::exit::{EXIT_GENERAL, EXIT_VERIFY_FAILED, ExitError};
use crate::ops::command_log;
use crate::ops::config;
use crate::ops::failure_category;
use crate::ops::lock;
use crate::ops::pack_fix;
use crate::ops::patch_bundle;
use crate::ops::run;
use crate::ops::worktree;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Serialize)]
struct VerifySummary {
    run_id: String,
    created_at: String,
    profile: String,
    ok: bool,
    failure_category: Option<String>,
    commands: Vec<VerifyCommandResult>,
}

#[derive(Debug, Serialize)]
struct VerifyCommandResult {
    name: String,
    argv: Vec<String>,
    status: i32,
    duration_ms: u128,
    stdout_path: String,
    stderr_path: String,
}

pub fn cmd(git_root: &Path, args: VerifyArgs) -> Result<(), ExitError> {
    let created_at = lock::now_rfc3339();

    let lock_path = lock::default_lock_path(git_root);
    let info = lock::make_lock_info(
        git_root,
        "verify",
        &[
            format!("--profile={}", args.profile.as_deref().unwrap_or("")),
            format!("--run-id={}", args.run_id.as_deref().unwrap_or("")),
        ],
    );
    let _guard = lock::LockGuard::acquire(&lock_path, info)?;

    let run_id = match &args.run_id {
        Some(id) => id.clone(),
        None => detect_latest_run_with_sandbox(git_root).ok_or_else(|| {
            ExitError::new(
                EXIT_GENERAL,
                "no runnable sandbox found (run diffship apply first, or pass --run-id)",
            )
        })?,
    };

    let run_dir = run::run_dir(git_root, &run_id);
    let manifest = patch_bundle::load_manifest_from_run_bundle(&run_dir)?;
    let cfg = config::resolve_ops_config(
        git_root,
        Some(&manifest),
        config::OpsConfigOverrides {
            verify_profile: args.profile.clone(),
            ..Default::default()
        },
    )?;
    let verify_commands = cfg.verify_commands_for_selected_profile();

    let out = verify_locked(
        git_root,
        &run_id,
        &cfg.verify_profile,
        verify_commands.as_deref(),
        created_at,
    )?;
    if out.ok {
        println!("diffship verify: ok");
        println!("  run_id  : {}", run_id);
        println!("  sandbox : {}", out.sandbox_path);
        println!("  logs    : {}", out.logs_path);
        Ok(())
    } else {
        Err(ExitError::new(
            EXIT_VERIFY_FAILED,
            format!("verify failed (run_id={})", run_id),
        ))
    }
}

#[derive(Debug, Clone)]
pub struct VerifyOut {
    pub ok: bool,
    pub sandbox_path: String,
    pub logs_path: String,
}

/// Internal verify step used by `loop`.
///
/// This function assumes the caller already holds the global ops lock.
pub fn verify_locked(
    git_root: &Path,
    run_id: &str,
    profile: &str,
    custom_profile_commands: Option<&[String]>,
    created_at: String,
) -> Result<VerifyOut, ExitError> {
    let run_dir = run::run_dir(git_root, run_id);
    if !run_dir.exists() {
        return Err(ExitError::new(
            EXIT_GENERAL,
            format!("run not found: {}", run_id),
        ));
    }

    let sb = worktree::read_sandbox_meta(git_root, run_id).ok_or_else(|| {
        ExitError::new(
            EXIT_GENERAL,
            format!("missing sandbox.json for run_id {}", run_id),
        )
    })?;
    let sandbox_path = PathBuf::from(&sb.path);
    if !sandbox_path.exists() {
        return Err(ExitError::new(
            EXIT_GENERAL,
            format!("sandbox path missing on disk: {}", sandbox_path.display()),
        ));
    }

    let plan = build_verify_plan(&sandbox_path, profile, custom_profile_commands);

    let out_dir = run_dir.join("verify");
    fs::create_dir_all(&out_dir)
        .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to create verify dir: {e}")))?;

    let mut results = vec![];
    let mut ok = true;

    for (idx, cmd) in plan.iter().enumerate() {
        let name = format!("{:02}_{}", idx + 1, sanitize_name(&cmd.name));
        let logged =
            command_log::run_and_log(&run_dir, "verify", &name, &sandbox_path, &cmd.argv, None)?;
        if logged.record.status != 0 {
            ok = false;
        }
        results.push(VerifyCommandResult {
            name: cmd.name.clone(),
            argv: cmd.argv.clone(),
            status: logged.record.status,
            duration_ms: logged.record.duration_ms,
            stdout_path: logged.record.stdout_path.clone(),
            stderr_path: logged.record.stderr_path.clone(),
        });
    }

    // NOTE: `created_at` is used in both verify.json and pack-fix. Clone for summary.
    let summary = VerifySummary {
        run_id: run_id.to_string(),
        created_at: created_at.clone(),
        profile: profile.to_string(),
        ok,
        failure_category: failure_category::classify_verify_failure(
            &results
                .iter()
                .map(|result| failure_category::VerifyCommandLike {
                    name: &result.name,
                    argv: &result.argv,
                    status: result.status,
                })
                .collect::<Vec<_>>(),
        ),
        commands: results,
    };
    let bytes = serde_json::to_vec_pretty(&summary).map_err(|e| {
        ExitError::new(
            EXIT_GENERAL,
            format!("failed to encode verify summary: {e}"),
        )
    })?;
    fs::write(run_dir.join("verify.json"), bytes)
        .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to write verify.json: {e}")))?;

    if !ok {
        match pack_fix::try_write_default_pack_fix_zip(
            git_root,
            run_id,
            &run_dir,
            &sandbox_path,
            &created_at,
        ) {
            Ok(p) => eprintln!("diffship verify: pack-fix saved to {}", p.display()),
            Err(e) => eprintln!("diffship verify: pack-fix failed: {}", e),
        }
    }

    Ok(VerifyOut {
        ok,
        sandbox_path: sandbox_path.display().to_string(),
        logs_path: out_dir.display().to_string(),
    })
}

#[derive(Debug, Clone)]
struct VerifyCommand {
    name: String,
    argv: Vec<String>,
}

fn build_verify_plan(
    repo_root: &Path,
    profile: &str,
    custom_profile_commands: Option<&[String]>,
) -> Vec<VerifyCommand> {
    if let Some(cmds) = custom_profile_commands
        && !cmds.is_empty()
    {
        return cmds
            .iter()
            .enumerate()
            .map(|(idx, cmd)| {
                plan_argv(
                    &format!("cfg_{}_{}", profile, idx + 1),
                    vec!["sh".into(), "-lc".into(), cmd.to_string()],
                )
            })
            .collect();
    }

    // Heuristic defaults for M2:
    // - If the repo has justfile and `just` is available: use just recipes.
    // - Else if the repo looks like a Rust crate: use cargo-based checks.
    // - Else: do a minimal git-only check.

    let has_justfile = repo_root.join("justfile").exists() || repo_root.join("Justfile").exists();
    let has_cargo = repo_root.join("Cargo.toml").exists();
    let has_just = which_available("just");

    let p = profile.trim().to_lowercase();

    if has_justfile && has_just {
        if p == "fast" {
            return vec![plan_cmd(
                "just",
                vec!["fmt-check".into(), "lint".into(), "test".into()],
            )];
        }
        return vec![plan_cmd("just", vec!["ci".into()])];
    }

    if has_cargo {
        if p == "fast" {
            return vec![plan_cmd("cargo", vec!["test".into()])];
        }
        return vec![
            plan_cmd(
                "cargo",
                vec!["fmt".into(), "--all".into(), "--".into(), "--check".into()],
            ),
            plan_cmd(
                "cargo",
                vec![
                    "clippy".into(),
                    "--all-targets".into(),
                    "--all-features".into(),
                    "--".into(),
                    "-D".into(),
                    "warnings".into(),
                ],
            ),
            plan_cmd("cargo", vec!["test".into()]),
        ];
    }

    // Generic fallback that works for any repo.
    // Note: this inspects the current diff for whitespace errors.
    vec![plan_cmd("git", vec!["diff".into(), "--check".into()])]
}

fn plan_cmd(bin: &str, args: Vec<String>) -> VerifyCommand {
    let mut argv = vec![bin.to_string()];
    argv.extend(args);
    plan_argv(bin, argv)
}

fn plan_argv(name: &str, argv: Vec<String>) -> VerifyCommand {
    VerifyCommand {
        name: name.to_string(),
        argv,
    }
}

fn which_available(bin: &str) -> bool {
    Command::new("sh")
        .args([
            "-lc",
            &format!("command -v {} >/dev/null 2>&1", shell_escape(bin)),
        ])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn shell_escape(s: &str) -> String {
    // minimal escape used for command -v; safe enough for this context.
    s.replace('"', "\\\"")
}

fn sanitize_name(s: &str) -> String {
    command_log::sanitize_name(s)
}

fn detect_latest_run_with_sandbox(git_root: &Path) -> Option<String> {
    // Prefer the latest run that has sandbox.json.
    let runs_dir = run::runs_dir(git_root);
    let mut best: Option<(String, String)> = None; // (created_at, run_id)

    let entries = fs::read_dir(&runs_dir).ok()?;
    for ent in entries.flatten() {
        let path = ent.path();
        if !path.is_dir() {
            continue;
        }
        let run_id = path.file_name()?.to_string_lossy().to_string();
        let run_json = path.join("run.json");
        let sandbox_json = path.join("sandbox.json");
        if !run_json.exists() || !sandbox_json.exists() {
            continue;
        }

        let bytes = fs::read(&run_json).ok()?;
        let meta: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
        let created_at = meta
            .get("created_at")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        match &best {
            None => best = Some((created_at, run_id)),
            Some((best_created, best_id)) => {
                if created_at > *best_created || (created_at == *best_created && run_id > *best_id)
                {
                    best = Some((created_at, run_id));
                }
            }
        }
    }

    best.map(|(_, id)| id)
}

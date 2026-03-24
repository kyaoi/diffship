use crate::cli::ApplyArgs;
use crate::exit::{
    EXIT_APPLY_FAILED, EXIT_BASE_COMMIT_MISMATCH, EXIT_DIRTY_WORKTREE, EXIT_GENERAL, ExitError,
};
use crate::git;
use crate::ops::command_log;
use crate::ops::failure_category;
use crate::ops::lock;
use crate::ops::pack_fix;
use crate::ops::patch_bundle;
use crate::ops::post_apply;
use crate::ops::run;
use crate::ops::session;
use crate::ops::tasks;
use crate::ops::worktree;
use crate::pathing::resolve_user_path;
use serde::Serialize;
use std::fs;
use std::path::Path;

#[derive(Debug, Serialize)]
struct ApplySummary {
    run_id: String,
    created_at: String,
    session: String,
    base_commit: String,
    declared_base_commit: String,
    effective_base_commit: String,
    bundle_path: String,
    delete_input_zip_requested: bool,
    deleted_input_zip_path: Option<String>,
    input_zip_deleted: Option<bool>,
    bundle_root: String,
    run_bundle_dir: String,
    apply_mode: String,
    tasks_required: bool,
    user_tasks_path: Option<String>,
    post_apply_commands: usize,
    post_apply_ok: Option<bool>,
    post_apply_logs_path: Option<String>,
    pack_fix_path: Option<String>,
    ok: bool,
    failure_category: Option<String>,
    error: Option<String>,
}

pub fn cmd(git_root: &Path, args: ApplyArgs) -> Result<(), ExitError> {
    let created_at = lock::now_rfc3339();

    // Safety default: refuse if the user's worktree is dirty.
    ensure_clean_worktree(git_root)?;

    let lock_path = lock::default_lock_path(git_root);
    let info = lock::make_lock_info(
        git_root,
        "apply",
        &[
            format!("--session={}", args.session),
            format!(
                "--base-commit={}",
                args.base_commit.as_deref().unwrap_or("")
            ),
            format!("--delete-input-zip={}", args.delete_input_zip),
            format!("--keep-sandbox={}", args.keep_sandbox),
        ],
    );
    let _guard = lock::LockGuard::acquire(&lock_path, info)?;

    let out = apply_locked(git_root, args, created_at)?;

    println!("diffship apply: ok");
    println!("  run_id  : {}", out.run_id);
    println!("  session : {}", out.session);
    println!("  sandbox : {}", out.sandbox_path);
    println!("  bundle  : {}", out.run_bundle_dir);
    if out.input_zip_deleted == Some(true) {
        if let Some(p) = &out.deleted_input_zip_path {
            println!("  deleted : {}", p);
        }
    } else if out.input_zip_deleted == Some(false)
        && let Some(p) = &out.deleted_input_zip_path
    {
        eprintln!(
            "diffship apply: warning: failed to delete input zip {} after copying it into the run directory",
            p
        );
    }
    if let Some(p) = &out.user_tasks_path {
        println!("  tasks   : {}", p);
        println!(
            "  note    : promotion will be blocked until tasks are acknowledged (use --ack-tasks)"
        );
    }
    if let Some(p) = &out.post_apply_logs_path {
        println!("  hooks   : {}", p);
    }
    if out.keep_sandbox {
        println!("  next    : diffship verify --run-id {}", out.run_id);
    } else {
        println!("  note    : sandbox cleanup requested; sandbox may be removed best-effort");
    }

    Ok(())
}

#[derive(Debug, Clone)]
pub struct ApplyOut {
    pub run_id: String,
    pub session: String,
    pub sandbox_path: String,
    pub run_bundle_dir: String,
    pub deleted_input_zip_path: Option<String>,
    pub input_zip_deleted: Option<bool>,
    pub user_tasks_path: Option<String>,
    pub post_apply_logs_path: Option<String>,
    pub keep_sandbox: bool,
}

/// Internal apply step used by `loop`.
///
/// This function assumes the caller already holds the global ops lock.
pub fn apply_locked(
    git_root: &Path,
    args: ApplyArgs,
    created_at: String,
) -> Result<ApplyOut, ExitError> {
    let run_meta = run::create_run(
        git_root,
        "apply",
        std::slice::from_ref(&args.bundle),
        created_at.clone(),
    )?;
    let run_dir = run::run_dir(git_root, &run_meta.run_id);

    // Prepare session + sandbox.
    let session_state = session::ensure_session(git_root, &args.session, created_at.clone())?;
    let sandbox = worktree::ensure_sandbox_for_run(
        git_root,
        &args.session,
        &run_meta.run_id,
        created_at.clone(),
    )?;
    worktree::assert_is_git_worktree_dir(Path::new(&sandbox.path))?;

    // Validate and copy patch bundle into the run dir before touching the sandbox.
    let cwd = std::env::current_dir()
        .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to detect current dir: {e}")))?;
    let bundle_path = resolve_user_path(&cwd, &args.bundle)?;
    let cfg = crate::ops::config::resolve_ops_config(git_root, None, Default::default())?;
    let forbid_patterns = cfg.forbid_patterns();
    let editable_diffship_files = cfg.editable_diffship_files();
    let mut bundle = patch_bundle::load_and_copy_into_run(
        git_root,
        &bundle_path,
        &run_dir,
        &forbid_patterns,
        &editable_diffship_files,
    )?;
    let (deleted_input_zip_path, input_zip_deleted) =
        maybe_delete_input_zip(&bundle_path, args.delete_input_zip);
    let declared_base_commit = bundle.manifest.base_commit.clone();
    let effective_base_commit = args
        .base_commit
        .clone()
        .unwrap_or_else(|| declared_base_commit.clone());
    if effective_base_commit != declared_base_commit {
        patch_bundle::rewrite_run_manifest_base_commit(&run_dir, &effective_base_commit)?;
        bundle.manifest.base_commit = effective_base_commit.clone();
    }

    // Surface required user tasks (copied into run_dir by load_and_copy_into_run).
    let tasks_required = bundle.manifest.tasks_required.unwrap_or(false);
    let user_tasks = {
        let p = tasks::user_tasks_path_in_run(&run_dir);
        if p.is_file() {
            Some(p.display().to_string())
        } else {
            None
        }
    };

    // base_commit must match the session head (resolved to full SHA).
    let declared = git::rev_parse(git_root, bundle.manifest.base_commit.trim()).map_err(|e| {
        ExitError::new(
            EXIT_BASE_COMMIT_MISMATCH,
            format!("invalid base_commit in manifest: {}", e.message),
        )
    })?;
    if declared != session_state.head {
        let summary = ApplySummary {
            run_id: run_meta.run_id.clone(),
            created_at: created_at.clone(),
            session: args.session.clone(),
            base_commit: session_state.head.clone(),
            declared_base_commit,
            effective_base_commit: declared.clone(),
            bundle_path: args.bundle.clone(),
            delete_input_zip_requested: args.delete_input_zip,
            deleted_input_zip_path: deleted_input_zip_path.clone(),
            input_zip_deleted,
            bundle_root: bundle.root.display().to_string(),
            run_bundle_dir: bundle.run_bundle_dir.display().to_string(),
            apply_mode: bundle.manifest.apply_mode.as_str().to_string(),
            tasks_required,
            user_tasks_path: user_tasks.clone(),
            post_apply_commands: 0,
            post_apply_ok: None,
            post_apply_logs_path: None,
            pack_fix_path: None,
            ok: false,
            failure_category: Some(failure_category::BASE_COMMIT_MISMATCH.to_string()),
            error: Some(format!(
                "base_commit mismatch: manifest={} session_head={}",
                declared, session_state.head
            )),
        };
        write_apply_summary(&run_dir, &summary)?;
        if !args.keep_sandbox {
            worktree::remove_worktree_best_effort(git_root, Path::new(&sandbox.path));
        }
        return Err(ExitError::new(
            EXIT_BASE_COMMIT_MISMATCH,
            format!(
                "refused: base_commit mismatch (manifest={} session_head={})",
                declared, session_state.head
            ),
        ));
    }

    let sandbox_path = Path::new(&sandbox.path);

    let apply_res = apply_patches_in_sandbox(git_root, &run_dir, sandbox_path, &bundle)?;
    let post_apply_commands = cfg.post_apply_commands().unwrap_or_default();
    let post_apply_out = if apply_res.is_ok && !post_apply_commands.is_empty() {
        Some(post_apply::run(
            git_root,
            &run_dir,
            sandbox_path,
            &post_apply_commands,
            &created_at,
        )?)
    } else {
        None
    };
    let post_apply_ok = post_apply_out.as_ref().map(|out| out.ok);
    let ok = apply_res.is_ok && post_apply_ok.unwrap_or(true);
    let error = if !apply_res.is_ok {
        apply_res.error.clone()
    } else if let Some(out) = post_apply_out.as_ref() {
        if !out.ok {
            Some("post-apply commands failed".to_string())
        } else {
            None
        }
    } else {
        None
    };
    let pack_fix_path = if apply_res.is_ok && post_apply_ok == Some(false) {
        pack_fix::try_write_default_pack_fix_zip(
            git_root,
            &run_meta.run_id,
            &run_dir,
            sandbox_path,
            &created_at,
        )
        .ok()
        .map(|p| p.display().to_string())
    } else {
        None
    };
    let summary = ApplySummary {
        run_id: run_meta.run_id.clone(),
        created_at: created_at.clone(),
        session: args.session.clone(),
        base_commit: session_state.head.clone(),
        declared_base_commit,
        effective_base_commit: declared,
        bundle_path: args.bundle.clone(),
        delete_input_zip_requested: args.delete_input_zip,
        deleted_input_zip_path: deleted_input_zip_path.clone(),
        input_zip_deleted,
        bundle_root: bundle.root.display().to_string(),
        run_bundle_dir: bundle.run_bundle_dir.display().to_string(),
        apply_mode: bundle.manifest.apply_mode.as_str().to_string(),
        tasks_required,
        user_tasks_path: user_tasks.clone(),
        post_apply_commands: post_apply_commands.len(),
        post_apply_ok,
        post_apply_logs_path: post_apply_out.as_ref().map(|out| out.logs_path.clone()),
        pack_fix_path: pack_fix_path.clone(),
        ok,
        failure_category: if ok {
            None
        } else if !apply_res.is_ok {
            Some(failure_category::PATCH_APPLY_FAILED.to_string())
        } else {
            Some(failure_category::POST_APPLY_FAILED.to_string())
        },
        error: error.clone(),
    };
    write_apply_summary(&run_dir, &summary)?;

    if !ok {
        // Rollback inside sandbox, then optionally remove the worktree.
        if !apply_res.is_ok {
            rollback_sandbox(sandbox_path, &sandbox.base_commit);
        }
        if !args.keep_sandbox {
            worktree::remove_worktree_best_effort(git_root, sandbox_path);
        }
        let failure_message = if let Some(path) = pack_fix_path {
            format!(
                "{} (run_id={}). pack-fix saved to {}.",
                error.unwrap_or_else(|| "apply failed".to_string()),
                run_meta.run_id,
                path
            )
        } else {
            error.unwrap_or_else(|| "apply failed".to_string())
        };
        return Err(ExitError::new(EXIT_APPLY_FAILED, failure_message));
    }

    Ok(ApplyOut {
        run_id: run_meta.run_id,
        session: args.session,
        sandbox_path: sandbox.path,
        run_bundle_dir: bundle.run_bundle_dir.display().to_string(),
        deleted_input_zip_path,
        input_zip_deleted,
        user_tasks_path: user_tasks,
        post_apply_logs_path: post_apply_out.map(|out| out.logs_path),
        keep_sandbox: args.keep_sandbox,
    })
}

pub(crate) fn ensure_clean_worktree(git_root: &Path) -> Result<(), ExitError> {
    let out = git::run_git(git_root, ["status", "--porcelain"]).map_err(|e| {
        ExitError::new(
            EXIT_GENERAL,
            format!("failed to check git status: {}", e.message),
        )
    })?;

    // Ignore diffship-managed state under .diffship/.
    let mut dirty = vec![];
    for line in out.lines() {
        // Porcelain v1: XY <path> (or ?? <path>)
        let path = line.get(3..).unwrap_or("").trim();
        if path == ".diffship" || path.starts_with(".diffship/") {
            continue;
        }
        if !path.is_empty() {
            dirty.push(path.to_string());
        }
    }

    if !dirty.is_empty() {
        return Err(ExitError::new(
            EXIT_DIRTY_WORKTREE,
            format!(
                "refused: working tree is dirty (commit or stash your changes)\nfirst few paths: {}",
                dirty.into_iter().take(5).collect::<Vec<_>>().join(", ")
            ),
        ));
    }
    Ok(())
}

fn write_apply_summary(run_dir: &Path, summary: &ApplySummary) -> Result<(), ExitError> {
    let bytes = serde_json::to_vec_pretty(summary).map_err(|e| {
        ExitError::new(EXIT_GENERAL, format!("failed to encode apply summary: {e}"))
    })?;
    fs::write(run_dir.join("apply.json"), bytes)
        .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to write apply.json: {e}")))?;
    Ok(())
}

fn maybe_delete_input_zip(
    bundle_path: &Path,
    delete_requested: bool,
) -> (Option<String>, Option<bool>) {
    if !delete_requested || !is_zip_input(bundle_path) {
        return (None, None);
    }

    let path = bundle_path.display().to_string();
    let deleted = fs::remove_file(bundle_path).is_ok();
    (Some(path), Some(deleted))
}

fn is_zip_input(bundle_path: &Path) -> bool {
    bundle_path.is_file()
        && bundle_path
            .extension()
            .and_then(|s| s.to_str())
            .map(|s| s.eq_ignore_ascii_case("zip"))
            .unwrap_or(false)
}

struct ApplyResult {
    is_ok: bool,
    error: Option<String>,
}

fn apply_patches_in_sandbox(
    git_root: &Path,
    run_dir: &Path,
    sandbox_path: &Path,
    bundle: &patch_bundle::PatchBundle,
) -> Result<ApplyResult, ExitError> {
    let patch_args: Vec<String> = bundle
        .patches
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();

    match bundle.manifest.apply_mode {
        patch_bundle::ApplyMode::GitApply => {
            // Preflight
            let mut check_args = vec!["apply".to_string(), "--check".to_string()];
            check_args.extend(patch_args.clone());
            if let Err(e) =
                run_git_capture(git_root, run_dir, sandbox_path, "01_preflight", &check_args)
            {
                return Ok(ApplyResult {
                    is_ok: false,
                    error: Some(format!("preflight failed: {}", e)),
                });
            }

            let mut apply_args = vec!["apply".to_string()];
            apply_args.extend(patch_args);
            if let Err(e) =
                run_git_capture(git_root, run_dir, sandbox_path, "02_apply", &apply_args)
            {
                return Ok(ApplyResult {
                    is_ok: false,
                    error: Some(format!("apply failed: {}", e)),
                });
            }

            Ok(ApplyResult {
                is_ok: true,
                error: None,
            })
        }
        patch_bundle::ApplyMode::GitAm => {
            // Best-effort preflight using git apply --check (mail patches should still be handled).
            let mut check_args = vec!["apply".to_string(), "--check".to_string()];
            check_args.extend(patch_args.clone());
            if let Err(e) =
                run_git_capture(git_root, run_dir, sandbox_path, "01_preflight", &check_args)
            {
                return Ok(ApplyResult {
                    is_ok: false,
                    error: Some(format!("preflight failed: {}", e)),
                });
            }

            let mut am_args = vec!["am".to_string()];
            am_args.extend(patch_args);
            if let Err(e) = run_git_capture(git_root, run_dir, sandbox_path, "02_git_am", &am_args)
            {
                // Abort best-effort.
                let _ = run_git_capture(
                    git_root,
                    run_dir,
                    sandbox_path,
                    "03_git_am_abort",
                    &["am".to_string(), "--abort".to_string()],
                );
                return Ok(ApplyResult {
                    is_ok: false,
                    error: Some(format!("git am failed: {}", e)),
                });
            }

            Ok(ApplyResult {
                is_ok: true,
                error: None,
            })
        }
    }
}

fn rollback_sandbox(sandbox_path: &Path, base_commit: &str) {
    let _ = git::run_git_in(sandbox_path, ["reset", "--hard", base_commit]);
    let _ = git::run_git_in(sandbox_path, ["clean", "-fdx"]);
}

fn run_git_capture(
    git_root: &Path,
    run_dir: &Path,
    dir: &Path,
    name: &str,
    args: &[String],
) -> Result<(), String> {
    let mut argv = vec!["git".to_string()];
    argv.extend(args.iter().cloned());
    let output = command_log::run_and_log(run_dir, git_root, "apply", name, dir, &argv, None)
        .map_err(|e| e.message)?;

    if output.record.status == 0 {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let msg = if !stderr.trim().is_empty() {
        stderr.trim().to_string()
    } else {
        stdout.trim().to_string()
    };
    Err(msg)
}

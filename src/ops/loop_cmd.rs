use crate::cli::{ApplyArgs, LoopArgs};
use crate::exit::{EXIT_VERIFY_FAILED, ExitError};
use crate::ops::apply;
use crate::ops::config;
use crate::ops::lock;
use crate::ops::patch_bundle;
use crate::ops::promote;
use crate::ops::run;
use crate::ops::verify;
use std::path::Path;

pub fn cmd(git_root: &Path, args: LoopArgs) -> Result<(), ExitError> {
    let created_at = lock::now_rfc3339();

    // Safety default: refuse if the user's worktree is dirty.
    apply::ensure_clean_worktree(git_root)?;

    // Hold the lock for the full loop.
    let lock_path = lock::default_lock_path(git_root);
    let info = lock::make_lock_info(
        git_root,
        "loop",
        &[
            format!("--session={}", args.session),
            format!(
                "--base-commit={}",
                args.base_commit.as_deref().unwrap_or("")
            ),
            format!("--profile={}", args.profile.as_deref().unwrap_or("")),
            format!(
                "--target-branch={}",
                args.target_branch.as_deref().unwrap_or("")
            ),
            format!("--ack-secrets={}", args.ack_secrets),
            format!("--ack-tasks={}", args.ack_tasks),
            format!("--promotion={}", args.promotion.as_deref().unwrap_or("")),
            format!(
                "--commit-policy={}",
                args.commit_policy.as_deref().unwrap_or("")
            ),
        ],
    );
    let _guard = lock::LockGuard::acquire(&lock_path, info)?;

    // Step 1: apply (always keep the sandbox until we know verify+promote outcome)
    let apply_args = ApplyArgs {
        bundle: args.bundle.clone(),
        session: args.session.clone(),
        base_commit: args.base_commit.clone(),
        keep_sandbox: true,
    };
    let applied = apply::apply_locked(git_root, apply_args, created_at.clone())?;

    // Resolve config after apply (bundle manifest is now saved under the run dir).
    let run_dir = run::run_dir(git_root, &applied.run_id);
    let manifest = patch_bundle::load_manifest_from_run_bundle(&run_dir)?;
    let cfg = config::resolve_ops_config(
        git_root,
        Some(&manifest),
        config::OpsConfigOverrides {
            verify_profile: args.profile.clone(),
            target_branch: args.target_branch.clone(),
            promotion_mode: args.promotion.clone(),
            commit_policy: args.commit_policy.clone(),
            ..Default::default()
        },
    )?;
    let verify_commands = cfg.verify_commands_for_selected_profile();

    // Step 2: verify
    let v = verify::verify_locked(
        git_root,
        &applied.run_id,
        &cfg.verify_profile,
        verify_commands.as_deref(),
        created_at.clone(),
    )?;
    if !v.ok {
        eprintln!(
            "diffship loop: verify failed (run_id={}). pack-fix saved under .diffship/runs/{}/pack-fix.zip.",
            applied.run_id, applied.run_id
        );
        return Err(ExitError::new(
            EXIT_VERIFY_FAILED,
            format!("verify failed (run_id={})", applied.run_id),
        ));
    }

    // Step 3: promote (commit)
    promote::promote_locked(
        git_root,
        &applied.run_id,
        &cfg.target_branch,
        &cfg.promotion_mode,
        &cfg.commit_policy,
        args.ack_secrets,
        args.ack_tasks,
        false,
        created_at.clone(),
    )?;

    println!("diffship loop: ok");
    println!("  run_id  : {}", applied.run_id);
    println!("  session : {}", applied.session);

    Ok(())
}

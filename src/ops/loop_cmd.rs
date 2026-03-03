use crate::cli::{ApplyArgs, LoopArgs};
use crate::exit::{EXIT_VERIFY_FAILED, ExitError};
use crate::ops::apply;
use crate::ops::lock;
use crate::ops::promote;
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
            format!("--profile={}", args.profile),
            format!("--target-branch={}", args.target_branch),
            format!("--ack-secrets={}", args.ack_secrets),
        ],
    );
    let _guard = lock::LockGuard::acquire(&lock_path, info)?;

    // Step 1: apply (always keep the sandbox until we know verify+promote outcome)
    let apply_args = ApplyArgs {
        bundle: args.bundle.clone(),
        session: args.session.clone(),
        keep_sandbox: true,
    };
    let applied = apply::apply_locked(git_root, apply_args, created_at.clone())?;

    // Step 2: verify
    let v = verify::verify_locked(git_root, &applied.run_id, &args.profile, created_at.clone())?;
    if !v.ok {
        eprintln!(
            "diffship loop: verify failed (run_id={}). pack-fix is not implemented yet.",
            applied.run_id
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
        &args.target_branch,
        args.ack_secrets,
        false,
        created_at.clone(),
    )?;

    println!("diffship loop: ok");
    println!("  run_id  : {}", applied.run_id);
    println!("  session : {}", applied.session);

    Ok(())
}

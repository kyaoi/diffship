use crate::cli::PromoteArgs;
use crate::exit::{
    EXIT_GENERAL, EXIT_PROMOTION_FAILED, EXIT_SECRETS_ACK_REQUIRED, EXIT_TASKS_ACK_REQUIRED,
    ExitError,
};
use crate::git;
use crate::ops::apply;
use crate::ops::config;
use crate::ops::lock;
use crate::ops::patch_bundle;
use crate::ops::run;
use crate::ops::secrets;
use crate::ops::session;
use crate::ops::tasks;
use crate::ops::worktree;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Deserialize)]
struct VerifySummaryLite {
    ok: bool,
}

#[derive(Debug, Serialize)]
struct PromoteSummary {
    run_id: String,
    created_at: String,
    target_branch: String,
    base_commit: String,
    promoted_head: Option<String>,
    commits: Vec<String>,
    ok: bool,
    error: Option<String>,
    secrets_hits: usize,
    tasks_present: bool,
    user_tasks_path: Option<String>,
}

pub fn cmd(git_root: &Path, args: PromoteArgs) -> Result<(), ExitError> {
    let created_at = lock::now_rfc3339();

    let lock_path = lock::default_lock_path(git_root);
    let info = lock::make_lock_info(
        git_root,
        "promote",
        &[
            format!("--run-id={}", args.run_id.as_deref().unwrap_or("")),
            format!(
                "--target-branch={}",
                args.target_branch.as_deref().unwrap_or("")
            ),
            format!("--ack-secrets={}", args.ack_secrets),
            format!("--ack-tasks={}", args.ack_tasks),
            format!("--keep-sandbox={}", args.keep_sandbox),
        ],
    );
    let _guard = lock::LockGuard::acquire(&lock_path, info)?;

    let run_id = match &args.run_id {
        Some(id) => id.clone(),
        None => detect_latest_verified_run(git_root).ok_or_else(|| {
            ExitError::new(
                EXIT_GENERAL,
                "no verified run found (run diffship verify first, or pass --run-id)",
            )
        })?,
    };

    let run_dir = run::run_dir(git_root, &run_id);
    let manifest = patch_bundle::load_manifest_from_run_bundle(&run_dir)?;
    let cfg = config::resolve_ops_config(
        git_root,
        Some(&manifest),
        config::OpsConfigOverrides {
            target_branch: args.target_branch.clone(),
            promotion_mode: args.promotion.clone(),
            commit_policy: args.commit_policy.clone(),
            ..Default::default()
        },
    )?;

    promote_locked(
        git_root,
        &run_id,
        &cfg.target_branch,
        &cfg.promotion_mode,
        &cfg.commit_policy,
        args.ack_secrets,
        args.ack_tasks,
        args.keep_sandbox,
        created_at,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn promote_locked(
    git_root: &Path,
    run_id: &str,
    target_branch: &str,
    promotion_mode: &str,
    commit_policy: &str,
    ack_secrets: bool,
    ack_tasks: bool,
    keep_sandbox: bool,
    created_at: String,
) -> Result<(), ExitError> {
    apply::ensure_clean_worktree(git_root)?;

    let run_dir = run::run_dir(git_root, run_id);
    if !run_dir.exists() {
        return Err(ExitError::new(
            EXIT_GENERAL,
            format!("run not found: {}", run_id),
        ));
    }

    let verify_ok = read_verify_ok(&run_dir)?;
    if !verify_ok {
        return Err(ExitError::new(
            EXIT_PROMOTION_FAILED,
            format!("refused: verify is not ok for run_id={}", run_id),
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

    // Surface required user tasks and block promotion by default until acknowledged.
    let user_tasks_path = tasks::user_tasks_path_in_run(&run_dir);
    let tasks_present = user_tasks_path.is_file();
    if tasks_present && !ack_tasks {
        return Err(ExitError::new(
            EXIT_TASKS_ACK_REQUIRED,
            format!(
                "refused: required user tasks must be acknowledged before promotion\nsee: {}\nrerun with --ack-tasks after completing them",
                user_tasks_path.display()
            ),
        ));
    }

    // Scan for secrets before promotion.
    let hits = secrets::scan_run_for_secrets(&run_dir)?;
    if !hits.is_empty() {
        secrets::write_secrets_report(&run_dir, &hits)?;
        if !ack_secrets {
            let first = hits
                .iter()
                .take(5)
                .map(|h| format!("{} ({})", h.path, h.reason))
                .collect::<Vec<_>>()
                .join("; ");
            return Err(ExitError::new(
                EXIT_SECRETS_ACK_REQUIRED,
                format!(
                    "refused: secrets detected ({} hits). rerun with --ack-secrets to proceed. first: {}",
                    hits.len(),
                    first
                ),
            ));
        }
    }

    let manifest = patch_bundle::load_manifest_from_run_bundle(&run_dir)?;
    let commit_msg = load_commit_message(&run_dir, &manifest, run_id);

    // Ensure we have commits in the sandbox to promote.
    let sandbox_head_before = git::run_git_in(&sandbox_path, ["rev-parse", "HEAD"])?
        .trim()
        .to_string();
    let sandbox_head = match manifest.apply_mode {
        patch_bundle::ApplyMode::GitApply => {
            if commit_policy == "auto" {
                ensure_commit_in_sandbox(&sandbox_path, &run_dir, &commit_msg)?;
            }
            git::run_git_in(&sandbox_path, ["rev-parse", "HEAD"])?
                .trim()
                .to_string()
        }
        patch_bundle::ApplyMode::GitAm => sandbox_head_before,
    };

    let base_commit = sb.base_commit.clone();
    let commits = list_commits_to_promote(&sandbox_path, &base_commit, &sandbox_head)?;
    if commits.is_empty() {
        return Err(ExitError::new(
            EXIT_PROMOTION_FAILED,
            "promotion had no commits to apply (unexpected)",
        ));
    }

    let effective_target = choose_target_branch(git_root, target_branch)?;

    // Promotion mode switch
    if promotion_mode == "none" {
        let summary = PromoteSummary {
            run_id: run_id.to_string(),
            created_at: created_at.clone(),
            target_branch: effective_target.clone(),
            base_commit: base_commit.clone(),
            promoted_head: None,
            commits: vec![],
            ok: true,
            error: Some("promotion skipped by policy (promotion=none)".to_string()),
            secrets_hits: hits.len(),
            tasks_present,
            user_tasks_path: if tasks_present {
                Some(user_tasks_path.display().to_string())
            } else {
                None
            },
        };
        write_promote_summary(&run_dir, &summary)?;
        if !keep_sandbox {
            worktree::remove_worktree_best_effort(git_root, Path::new(&sb.path));
        }
        return Ok(());
    }

    // Promotion safety: require the target branch head to match the sandbox base.
    let target_head = git::rev_parse(git_root, &effective_target)?;
    if target_head != base_commit {
        let summary = PromoteSummary {
            run_id: run_id.to_string(),
            created_at: created_at.clone(),
            target_branch: effective_target.clone(),
            base_commit: base_commit.clone(),
            promoted_head: None,
            commits: commits.clone(),
            ok: false,
            error: Some(format!(
                "target branch head mismatch: target={} head={} expected_base={}",
                effective_target, target_head, base_commit
            )),
            secrets_hits: hits.len(),
            tasks_present,
            user_tasks_path: if tasks_present {
                Some(user_tasks_path.display().to_string())
            } else {
                None
            },
        };
        write_promote_summary(&run_dir, &summary)?;
        return Err(ExitError::new(
            EXIT_PROMOTION_FAILED,
            format!(
                "refused: target branch head mismatch (target={} head={} expected_base={})",
                effective_target, target_head, base_commit
            ),
        ));
    }

    // Checkout the target branch in the user's working tree and cherry-pick the commits.
    checkout_branch(git_root, &effective_target)?;

    if let Err(e) = cherry_pick_commits(git_root, &commits) {
        // Abort best-effort to restore a clean state.
        let _ = git::run_git(git_root, ["cherry-pick", "--abort"]);

        let summary = PromoteSummary {
            run_id: run_id.to_string(),
            created_at: created_at.clone(),
            target_branch: effective_target.clone(),
            base_commit: base_commit.clone(),
            promoted_head: None,
            commits: commits.clone(),
            ok: false,
            error: Some(e.clone()),
            secrets_hits: hits.len(),
            tasks_present,
            user_tasks_path: if tasks_present {
                Some(user_tasks_path.display().to_string())
            } else {
                None
            },
        };
        write_promote_summary(&run_dir, &summary)?;

        return Err(ExitError::new(
            EXIT_PROMOTION_FAILED,
            format!("promotion failed: {}", e),
        ));
    }

    let promoted_head = git::rev_parse(git_root, "HEAD")?;

    // Advance the session state to the promoted head (future runs base from here).
    let _state =
        session::advance_session(git_root, &sb.session, &promoted_head, created_at.clone())?;

    if !keep_sandbox {
        worktree::remove_worktree_best_effort(git_root, Path::new(&sb.path));
    }

    let summary = PromoteSummary {
        run_id: run_id.to_string(),
        created_at,
        target_branch: effective_target,
        base_commit,
        promoted_head: Some(promoted_head),
        commits,
        ok: true,
        error: None,
        secrets_hits: hits.len(),
        tasks_present,
        user_tasks_path: if tasks_present {
            Some(user_tasks_path.display().to_string())
        } else {
            None
        },
    };
    write_promote_summary(&run_dir, &summary)?;

    println!("diffship promote: ok");
    println!("  run_id  : {}", run_id);
    println!("  target : {}", summary.target_branch);
    println!(
        "  head   : {}",
        summary.promoted_head.as_deref().unwrap_or("")
    );

    Ok(())
}

fn write_promote_summary(run_dir: &Path, summary: &PromoteSummary) -> Result<(), ExitError> {
    let bytes = serde_json::to_vec_pretty(summary).map_err(|e| {
        ExitError::new(
            EXIT_GENERAL,
            format!("failed to encode promotion summary: {e}"),
        )
    })?;
    fs::write(run_dir.join("promotion.json"), bytes).map_err(|e| {
        ExitError::new(EXIT_GENERAL, format!("failed to write promotion.json: {e}"))
    })?;
    Ok(())
}

fn read_verify_ok(run_dir: &Path) -> Result<bool, ExitError> {
    let p = run_dir.join("verify.json");
    let bytes = fs::read(&p)
        .map_err(|e| ExitError::new(EXIT_GENERAL, format!("missing verify.json: {e}")))?;
    let v = serde_json::from_slice::<VerifySummaryLite>(&bytes)
        .map_err(|e| ExitError::new(EXIT_GENERAL, format!("invalid verify.json: {e}")))?;
    Ok(v.ok)
}

fn detect_latest_verified_run(git_root: &Path) -> Option<String> {
    let runs_dir = run::runs_dir(git_root);
    if !runs_dir.exists() {
        return None;
    }

    let mut best: Option<(String, String)> = None; // (created_at, run_id)
    let Ok(rd) = fs::read_dir(&runs_dir) else {
        return None;
    };
    for ent in rd.flatten() {
        if !ent.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        let run_id = ent.file_name().to_string_lossy().to_string();
        let run_dir = ent.path();
        if !run_dir.join("verify.json").exists() {
            continue;
        }
        if worktree::read_sandbox_meta(git_root, &run_id).is_none() {
            continue;
        }
        let bytes = fs::read(run_dir.join("verify.json")).ok()?;
        let v = serde_json::from_slice::<VerifySummaryLite>(&bytes).ok()?;
        if !v.ok {
            continue;
        }

        let meta_path = run_dir.join("run.json");
        let bytes = fs::read(&meta_path).ok()?;
        let meta = serde_json::from_slice::<run::RunMeta>(&bytes).ok()?;

        match &best {
            Some((best_created, best_id)) => {
                if meta.created_at > *best_created
                    || (meta.created_at == *best_created && meta.run_id > *best_id)
                {
                    best = Some((meta.created_at, meta.run_id));
                }
            }
            None => best = Some((meta.created_at, meta.run_id)),
        }
    }

    best.map(|(_, id)| id)
}

fn load_commit_message(
    run_dir: &Path,
    m: &patch_bundle::PatchBundleManifest,
    run_id: &str,
) -> String {
    let p = run_dir.join("bundle").join("commit_message.txt");
    if let Ok(s) = fs::read_to_string(&p) {
        let t = s.trim();
        if !t.is_empty() {
            return ensure_trailing_newline(t);
        }
    }

    // Deterministic fallback.
    let mut out = String::new();
    out.push_str(&format!("{}: apply patch bundle\n\n", m.task_id));
    out.push_str(&format!("run_id: {}\n", run_id));
    out.push_str(&format!("base_commit: {}\n", m.base_commit));
    out.push_str(&format!("apply_mode: {}\n", m.apply_mode.as_str()));
    out.push_str("touched_files:\n");
    for p in &m.touched_files {
        out.push_str(&format!("- {}\n", p));
    }
    ensure_trailing_newline(&out)
}

fn ensure_trailing_newline(s: &str) -> String {
    let mut out = s.to_string();
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

fn ensure_commit_in_sandbox(
    sandbox_path: &Path,
    run_dir: &Path,
    commit_msg: &str,
) -> Result<(), ExitError> {
    // Stage everything and create a single commit.
    let _ = git::run_git_in(sandbox_path, ["add", "-A"])?;

    // If nothing is staged, refuse: we expect apply to have produced a diff.
    let status = Command::new("git")
        .args(["diff", "--cached", "--quiet"])
        .current_dir(sandbox_path)
        .status()
        .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to run git: {e}")))?;
    if status.success() {
        return Err(ExitError::new(
            EXIT_PROMOTION_FAILED,
            "nothing to commit in sandbox",
        ));
    }

    let msg_path = run_dir.join("promotion_commit_message.txt");
    fs::write(&msg_path, commit_msg).map_err(|e| {
        ExitError::new(EXIT_GENERAL, format!("failed to write commit message: {e}"))
    })?;

    let msg_path_s = msg_path.to_string_lossy().to_string();

    git::run_git_in(
        sandbox_path,
        ["commit", "--no-gpg-sign", "--file", msg_path_s.as_str()],
    )?;

    Ok(())
}

fn list_commits_to_promote(
    sandbox_path: &Path,
    base_commit: &str,
    head: &str,
) -> Result<Vec<String>, ExitError> {
    let out = git::run_git_in(
        sandbox_path,
        [
            "rev-list",
            "--reverse",
            &format!("{}..{}", base_commit, head),
        ],
    )?;
    Ok(out
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .map(|l| l.to_string())
        .collect())
}

fn branch_exists(git_root: &Path, name: &str) -> bool {
    Command::new("git")
        .args([
            "show-ref",
            "--verify",
            "--quiet",
            &format!("refs/heads/{}", name),
        ])
        .current_dir(git_root)
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn current_branch(git_root: &Path) -> Option<String> {
    git::run_git(git_root, ["symbolic-ref", "--quiet", "--short", "HEAD"])
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn choose_target_branch(git_root: &Path, requested: &str) -> Result<String, ExitError> {
    let req = requested.trim();
    if req.is_empty() {
        return Err(ExitError::new(
            EXIT_GENERAL,
            "target branch must not be empty",
        ));
    }

    // Default UX: requested is "develop"; if it doesn't exist, fall back to current branch.
    if branch_exists(git_root, req) {
        return Ok(req.to_string());
    }

    if req == "develop"
        && let Some(cur) = current_branch(git_root)
        && branch_exists(git_root, &cur)
    {
        return Ok(cur);
    }

    Err(ExitError::new(
        EXIT_PROMOTION_FAILED,
        format!("target branch does not exist: {}", req),
    ))
}

fn checkout_branch(git_root: &Path, branch: &str) -> Result<(), ExitError> {
    // If already on the branch, do nothing.
    if current_branch(git_root).as_deref() == Some(branch) {
        return Ok(());
    }
    git::run_git(git_root, ["checkout", branch]).map(|_| ())
}

fn cherry_pick_commits(git_root: &Path, commits: &[String]) -> Result<(), String> {
    // Cherry-pick as one command for better UX.
    let mut argv = vec!["cherry-pick".to_string(), "--no-gpg-sign".to_string()];
    argv.extend(commits.iter().cloned());

    let output = Command::new("git")
        .args(&argv)
        .current_dir(git_root)
        .output()
        .map_err(|e| format!("failed to run git cherry-pick: {e}"))?;

    if output.status.success() {
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

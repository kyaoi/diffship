use crate::cli::DoctorArgs;
use crate::exit::{EXIT_GENERAL, ExitError};
use crate::git;
use crate::ops::lock;
use crate::ops::session;
use crate::ops::worktree;
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Serialize)]
struct DoctorReport {
    ok: bool,
    repo_head: String,
    session: String,
    fixed: bool,
    issues: Vec<DoctorIssue>,
}

#[derive(Debug, Serialize)]
struct DoctorIssue {
    kind: String,
    message: String,
    suggested_command: Option<String>,
    safe_fix: bool,
}

pub fn cmd(git_root: &Path, args: DoctorArgs) -> Result<(), ExitError> {
    let now = lock::now_rfc3339();
    let lock_path = lock::default_lock_path(git_root);
    let info = lock::make_lock_info(
        git_root,
        "doctor",
        &[
            format!("--session={}", args.session),
            format!("--fix={}", args.fix),
            format!("--json={}", args.json),
        ],
    );
    let _guard = lock::LockGuard::acquire(&lock_path, info)?;

    let repo_head = git::rev_parse(git_root, "HEAD")?;
    let mut fixed = false;
    let mut issues = collect_issues(git_root, &args.session, &repo_head)?;

    if args.fix && can_apply_safe_fix(&issues) {
        session::repair_session(git_root, &args.session, now)?;
        issues = collect_issues(git_root, &args.session, &repo_head)?;
        fixed = true;
    }

    let report = DoctorReport {
        ok: issues.is_empty(),
        repo_head,
        session: args.session,
        fixed,
        issues,
    };

    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&report)
                .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to encode json: {e}")))?
        );
    } else if report.ok {
        if report.fixed {
            println!("diffship doctor: ok (fixed)");
        } else {
            println!("diffship doctor: ok");
        }
        println!("  session : {}", report.session);
        println!("  head    : {}", report.repo_head);
    } else {
        println!("diffship doctor: issues");
        println!("  session : {}", report.session);
        println!("  head    : {}", report.repo_head);
        for issue in &report.issues {
            println!("  - {}: {}", issue.kind, issue.message);
            if let Some(cmd) = &issue.suggested_command {
                println!("    fix: {}", cmd);
            }
        }
    }

    if report.ok {
        Ok(())
    } else {
        Err(ExitError::new(
            EXIT_GENERAL,
            format!("doctor found {} issue(s)", report.issues.len()),
        ))
    }
}

fn collect_issues(
    git_root: &Path,
    session_name: &str,
    repo_head: &str,
) -> Result<Vec<DoctorIssue>, ExitError> {
    let mut issues = vec![];
    let state = session::read_session_state(git_root, session_name);
    let session_ref_name = session::session_ref(session_name);
    let session_ref_head = git::rev_parse(git_root, &session_ref_name).ok();

    if session_ref_head.is_none() {
        issues.push(DoctorIssue {
            kind: "missing_session_ref".to_string(),
            message: format!("missing {}", session_ref_name),
            suggested_command: Some(format!(
                "diffship session repair --session {}",
                session_name
            )),
            safe_fix: true,
        });
    }

    let Some(state) = state else {
        issues.push(DoctorIssue {
            kind: "missing_session_state".to_string(),
            message: format!("missing .diffship/sessions/{}.json", session_name),
            suggested_command: Some(format!(
                "diffship session repair --session {}",
                session_name
            )),
            safe_fix: true,
        });
        collect_sandbox_issues(git_root, &mut issues);
        return Ok(issues);
    };

    if let Some(ref_head) = session_ref_head.as_deref()
        && ref_head != repo_head
    {
        issues.push(DoctorIssue {
            kind: "session_head_mismatch".to_string(),
            message: format!(
                "session head {} differs from repo HEAD {}",
                ref_head, repo_head
            ),
            suggested_command: Some(format!(
                "diffship session repair --session {}",
                session_name
            )),
            safe_fix: true,
        });
    }

    let worktree_path = Path::new(&state.worktree_path);
    if !worktree_path.exists() {
        issues.push(DoctorIssue {
            kind: "missing_session_worktree".to_string(),
            message: format!("missing session worktree {}", worktree_path.display()),
            suggested_command: Some(format!(
                "diffship session repair --session {}",
                session_name
            )),
            safe_fix: true,
        });
    } else if worktree::assert_is_git_worktree_dir(worktree_path).is_err() {
        issues.push(DoctorIssue {
            kind: "invalid_session_worktree".to_string(),
            message: format!(
                "session worktree is not a git worktree: {}",
                worktree_path.display()
            ),
            suggested_command: Some(format!(
                "diffship session repair --session {}",
                session_name
            )),
            safe_fix: true,
        });
    }

    collect_sandbox_issues(git_root, &mut issues);
    Ok(issues)
}

fn collect_sandbox_issues(git_root: &Path, issues: &mut Vec<DoctorIssue>) {
    for meta in worktree::list_sandbox_metas(git_root) {
        if !Path::new(&meta.path).exists() {
            issues.push(DoctorIssue {
                kind: "missing_sandbox_worktree".to_string(),
                message: format!("sandbox {} is missing on disk", meta.path),
                suggested_command: Some(format!("git worktree remove --force {}", meta.path)),
                safe_fix: false,
            });
        }
    }
}

fn can_apply_safe_fix(issues: &[DoctorIssue]) -> bool {
    !issues.is_empty() && issues.iter().all(|issue| issue.safe_fix)
}

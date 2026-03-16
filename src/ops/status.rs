use crate::cli::StatusArgs;
use crate::exit::{EXIT_GENERAL, ExitError};
use crate::ops::lock;
use crate::ops::run;
use crate::ops::session;
use crate::ops::worktree;
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Serialize)]
struct StatusJson {
    git_root: String,
    repo_head: String,
    lock: LockState,
    sessions: Vec<session::SessionState>,
    sandboxes: Vec<SandboxState>,
    recent_runs: Vec<run::RunSummary>,
}

#[derive(Debug, Serialize)]
struct LockState {
    path: String,
    held: bool,
    info: Option<lock::LockInfo>,
}

#[derive(Debug, Serialize)]
struct SandboxState {
    run_id: String,
    path: String,
    exists: bool,
    meta: Option<worktree::SandboxMeta>,
}

pub fn cmd(git_root: &Path, args: StatusArgs) -> Result<(), ExitError> {
    let lock_path = lock::default_lock_path(git_root);
    let held = lock::is_lock_held(&lock_path).unwrap_or(false);
    let info = lock::read_lock_info(&lock_path);

    let recent_runs = run::list_runs(git_root, args.limit)?;
    let repo_head = crate::git::rev_parse(git_root, "HEAD")?;

    let sessions = session::list_sessions(git_root);

    // Sandboxes: scan the worktree dir to support recovery even if runs list is truncated.
    let sandboxes = list_sandboxes(git_root);

    if args.json {
        let payload = StatusJson {
            git_root: git_root.display().to_string(),
            repo_head: repo_head.clone(),
            lock: LockState {
                path: lock_path.display().to_string(),
                held,
                info,
            },
            sessions,
            sandboxes,
            recent_runs,
        };

        let s = serde_json::to_string_pretty(&payload)
            .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to encode json: {e}")))?;
        println!("{}", s);
        return Ok(());
    }

    if args.heads_only {
        println!("diffship status --heads-only");
        println!("  repo_head : {}", repo_head);

        if sessions.is_empty() {
            println!("  sessions  : (none)");
        } else {
            println!("  sessions  :");
            for s in &sessions {
                println!("    - {}  head={}  base={}", s.name, s.head, s.base_commit);
            }
        }

        if sandboxes.is_empty() {
            println!("  sandboxes : (none)");
        } else {
            println!("  sandboxes :");
            for sb in &sandboxes {
                let base = sb
                    .meta
                    .as_ref()
                    .map(|m| m.base_commit.as_str())
                    .unwrap_or("(unknown)");
                println!("    - {}  base={}  path={}", sb.run_id, base, sb.path);
            }
        }

        if recent_runs.is_empty() {
            println!("  recent_runs: (none)");
        } else {
            println!("  recent_runs:");
            for r in &recent_runs {
                println!(
                    "    - {}  {}  base={}  promoted={}",
                    r.created_at,
                    r.run_id,
                    r.effective_base_commit.as_deref().unwrap_or("(none)"),
                    r.promoted_head.as_deref().unwrap_or("(none)")
                );
            }
        }

        return Ok(());
    }

    println!("diffship status");
    println!("  git_root : {}", git_root.display());
    println!("  lock     : {}", lock_path.display());

    match (held, info) {
        (true, Some(i)) => {
            println!(
                "  lock_held: yes (pid={}, started_at={}, cmd={})",
                i.pid, i.started_at, i.command
            );
        }
        (true, None) => {
            println!("  lock_held: yes (metadata unreadable)");
        }
        (false, Some(i)) => {
            // File may remain after release; show last holder for diagnostics.
            println!(
                "  lock_held: no (last pid={}, started_at={}, cmd={})",
                i.pid, i.started_at, i.command
            );
        }
        (false, None) => {
            println!("  lock_held: no");
        }
    }

    if recent_runs.is_empty() {
        println!("  runs     : (none)");
    } else {
        println!("  runs     :");
        for r in &recent_runs {
            let logs = if r.command_count == 0 {
                String::new()
            } else {
                format!(
                    "  commands={}  phases={}",
                    r.command_count,
                    r.command_phases.join(",")
                )
            };
            println!(
                "    - {}  {}  {}{}",
                r.created_at, r.run_id, r.command, logs
            );
            println!("      run_dir={}", r.run_dir);
            if let Some(path) = &r.commands_index_path {
                println!("      commands_json={}", path);
            }
            if !r.command_phase_dirs.is_empty() {
                println!("      phase_dirs={}", r.command_phase_dirs.join(", "));
            }
        }
    }

    if sessions.is_empty() {
        println!("  sessions : (none)");
    } else {
        println!("  sessions :");
        for s in &sessions {
            println!("    - {}  head={}  wt={}", s.name, s.head, s.worktree_path);
        }
    }

    if sandboxes.is_empty() {
        println!("  sandboxes: (none)");
    } else {
        println!("  sandboxes:");
        for sb in &sandboxes {
            let hint = if sb.exists { "" } else { " (missing on disk)" };
            println!("    - {}  {}{}", sb.run_id, sb.path, hint);
        }
        println!("  hint: you can remove a sandbox via: git worktree remove --force <path>");
    }

    Ok(())
}

fn list_sandboxes(git_root: &Path) -> Vec<SandboxState> {
    let dir = worktree::sandboxes_dir(git_root);
    if !dir.exists() {
        return vec![];
    }

    let mut out = vec![];
    let Ok(rd) = std::fs::read_dir(&dir) else {
        return vec![];
    };
    for ent in rd.flatten() {
        let Ok(ft) = ent.file_type() else {
            continue;
        };
        if !ft.is_dir() {
            continue;
        }
        let run_id = ent.file_name().to_string_lossy().to_string();
        let path = ent.path();
        let meta = worktree::read_sandbox_meta(git_root, &run_id);
        out.push(SandboxState {
            run_id,
            path: path.display().to_string(),
            exists: path.exists(),
            meta,
        });
    }

    out.sort_by(|a, b| a.run_id.cmp(&b.run_id));
    out
}

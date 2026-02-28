use crate::cli::StatusArgs;
use crate::exit::{EXIT_GENERAL, ExitError};
use crate::ops::lock;
use crate::ops::run;
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Serialize)]
struct StatusJson {
    git_root: String,
    lock: LockState,
    recent_runs: Vec<run::RunSummary>,
}

#[derive(Debug, Serialize)]
struct LockState {
    path: String,
    held: bool,
    info: Option<lock::LockInfo>,
}

pub fn cmd(git_root: &Path, args: StatusArgs) -> Result<(), ExitError> {
    let lock_path = lock::default_lock_path(git_root);
    let held = lock::is_lock_held(&lock_path).unwrap_or(false);
    let info = lock::read_lock_info(&lock_path);

    let recent_runs = run::list_runs(git_root, args.limit)?;

    if args.json {
        let payload = StatusJson {
            git_root: git_root.display().to_string(),
            lock: LockState {
                path: lock_path.display().to_string(),
                held,
                info,
            },
            recent_runs,
        };

        let s = serde_json::to_string_pretty(&payload)
            .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to encode json: {e}")))?;
        println!("{}", s);
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
        return Ok(());
    }

    println!("  runs     :");
    for r in recent_runs {
        println!("    - {}  {}  {}", r.created_at, r.run_id, r.command);
    }

    Ok(())
}

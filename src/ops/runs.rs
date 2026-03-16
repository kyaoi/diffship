use crate::cli::RunsArgs;
use crate::exit::{EXIT_GENERAL, ExitError};
use crate::ops::run;
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Serialize)]
struct RunsJson {
    git_root: String,
    runs: Vec<run::RunSummary>,
}

pub fn cmd(git_root: &Path, args: RunsArgs) -> Result<(), ExitError> {
    let runs = run::list_runs(git_root, args.limit)?;

    if args.json {
        let payload = RunsJson {
            git_root: git_root.display().to_string(),
            runs,
        };

        let s = serde_json::to_string_pretty(&payload)
            .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to encode json: {e}")))?;
        println!("{}", s);
        return Ok(());
    }

    if runs.is_empty() {
        println!("diffship runs: (none)");
        return Ok(());
    }

    if args.heads_only {
        println!("diffship runs --heads-only:");
        for r in runs {
            println!(
                "- {}  {}  {}  base={}  promoted={}",
                r.created_at,
                r.run_id,
                r.command,
                r.effective_base_commit.as_deref().unwrap_or("(none)"),
                r.promoted_head.as_deref().unwrap_or("(none)")
            );
        }
        return Ok(());
    }

    println!("diffship runs:");
    for r in runs {
        let logs = if r.command_count == 0 {
            String::new()
        } else {
            format!(
                "  commands={}  phases={}",
                r.command_count,
                r.command_phases.join(",")
            )
        };
        println!("- {}  {}  {}{}", r.created_at, r.run_id, r.command, logs);
        println!("    run_dir={}", r.run_dir);
        if let Some(path) = &r.commands_index_path {
            println!("    commands_json={}", path);
        }
        if !r.command_phase_dirs.is_empty() {
            println!("    phase_dirs={}", r.command_phase_dirs.join(", "));
        }
    }

    Ok(())
}

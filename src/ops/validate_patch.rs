use crate::cli::ValidatePatchArgs;
use crate::exit::{EXIT_GENERAL, ExitError};
use crate::ops::config;
use crate::ops::patch_bundle;
use crate::pathing::resolve_user_path;
use std::path::Path;

pub fn cmd(git_root: &Path, args: ValidatePatchArgs) -> Result<(), ExitError> {
    let cwd = std::env::current_dir()
        .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to detect current dir: {e}")))?;
    let bundle_path = resolve_user_path(&cwd, &args.bundle)?;
    let cfg = config::resolve_ops_config(git_root, None, Default::default())?;
    let report = patch_bundle::validate_bundle_path(
        git_root,
        &bundle_path,
        &cfg.forbid_patterns(),
        &cfg.editable_diffship_files(),
    )?;

    if args.json {
        let json = serde_json::to_string_pretty(&report)
            .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to encode json: {e}")))?;
        println!("{json}");
        return Ok(());
    }

    println!("diffship validate-patch: ok");
    println!("  bundle        : {}", report.bundle_path);
    println!("  bundle_kind   : {}", report.bundle_kind);
    println!("  detected_root : {}", report.detected_root);
    println!("  task_id       : {}", report.manifest.task_id);
    println!("  apply_mode    : {}", report.manifest.apply_mode.as_str());
    println!("  base_commit   : {}", report.manifest.base_commit);
    println!("  touched_files : {}", report.manifest.touched_files.len());
    println!("  patch_files   : {}", report.patch_files.len());
    println!(
        "  tasks         : {}",
        if report.tasks_user_tasks_present {
            "tasks/USER_TASKS.md present"
        } else {
            "(none)"
        }
    );
    println!("  next          : diffship apply {}", args.bundle);
    Ok(())
}

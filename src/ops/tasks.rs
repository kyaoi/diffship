use crate::exit::{EXIT_GENERAL, ExitError};
use crate::ops::patch_bundle::PatchBundleManifest;
use std::path::{Path, PathBuf};

/// Validate the tasks contract declared by a patch bundle manifest.
///
/// Contract (v1): if `tasks_required: true`, the bundle MUST include `tasks/USER_TASKS.md`.
pub fn validate_tasks_contract(
    manifest: &PatchBundleManifest,
    bundle_root: &Path,
) -> Result<(), ExitError> {
    if manifest.tasks_required.unwrap_or(false) {
        let p = bundle_root.join("tasks").join("USER_TASKS.md");
        if !p.is_file() {
            return Err(ExitError::new(
                EXIT_GENERAL,
                format!(
                    "manifest.tasks_required is true but missing tasks/USER_TASKS.md at {}",
                    p.display()
                ),
            ));
        }
    }
    Ok(())
}

/// Path to the user-facing tasks file saved in a run directory.
///
/// Bundles are copied under `<run>/bundle/`, so tasks live under `<run>/bundle/tasks/`.
pub fn user_tasks_path_in_run(run_dir: &Path) -> PathBuf {
    run_dir.join("bundle").join("tasks").join("USER_TASKS.md")
}

/// Whether the run includes user tasks.
#[allow(dead_code)]
pub fn tasks_present_in_run(run_dir: &Path) -> bool {
    user_tasks_path_in_run(run_dir).is_file()
}

#[allow(dead_code)]
pub fn tasks_dir_in_run(run_dir: &Path) -> PathBuf {
    run_dir.join("bundle").join("tasks")
}

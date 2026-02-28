use crate::exit::{EXIT_NOT_GIT_REPO, ExitError};
use std::path::PathBuf;
use std::process::Command;

pub fn git_root() -> Result<PathBuf, ExitError> {
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .map_err(|e| ExitError::new(EXIT_NOT_GIT_REPO, format!("git not available: {e}")))?;

    if !output.status.success() {
        return Err(ExitError::new(
            EXIT_NOT_GIT_REPO,
            "not a git repository (or any of the parent directories)",
        ));
    }

    let s = String::from_utf8_lossy(&output.stdout);
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Err(ExitError::new(
            EXIT_NOT_GIT_REPO,
            "failed to detect git root",
        ));
    }

    Ok(PathBuf::from(trimmed))
}

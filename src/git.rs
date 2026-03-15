use crate::exit::{EXIT_GENERAL, EXIT_NOT_GIT_REPO, ExitError};
use std::path::{Path, PathBuf};
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

pub fn run_git<I, S>(git_root: &Path, args: I) -> Result<String, ExitError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    let output = Command::new("git")
        .args(args)
        .current_dir(git_root)
        .output()
        .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to run git: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(ExitError::new(
            EXIT_GENERAL,
            format!("git failed: {}", stderr.trim()),
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

pub fn run_git_in<I, S>(dir: &Path, args: I) -> Result<String, ExitError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to run git: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(ExitError::new(
            EXIT_GENERAL,
            format!("git failed: {}", stderr.trim()),
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

pub fn rev_parse(git_root: &Path, rev: &str) -> Result<String, ExitError> {
    let out = run_git(git_root, ["rev-parse", rev])?;
    let s = out.trim();
    if s.is_empty() {
        return Err(ExitError::new(
            EXIT_GENERAL,
            "git rev-parse returned empty output",
        ));
    }
    Ok(s.to_string())
}

pub fn short_sha_label(raw: &str) -> String {
    raw.trim()
        .chars()
        .take(7)
        .collect::<String>()
        .to_ascii_lowercase()
}

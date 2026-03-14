use crate::cli::{TestM1CleanupSandboxArgs, TestM1SetupArgs};
use crate::exit::{EXIT_GENERAL, ExitError};
use crate::git;
use crate::ops::lock;
use crate::ops::run;
use crate::ops::session;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub fn worktrees_dir(git_root: &Path) -> PathBuf {
    git_root.join(".diffship").join("worktrees")
}

pub fn sessions_dir(git_root: &Path) -> PathBuf {
    worktrees_dir(git_root).join("sessions")
}

pub fn sandboxes_dir(git_root: &Path) -> PathBuf {
    worktrees_dir(git_root).join("sandboxes")
}

pub fn sandbox_dir(git_root: &Path, run_id: &str) -> PathBuf {
    sandboxes_dir(git_root).join(run_id)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxMeta {
    pub run_id: String,
    pub session: String,
    pub created_at: String,
    pub base_commit: String,
    pub path: String,
}

pub fn sandbox_meta_path(git_root: &Path, run_id: &str) -> PathBuf {
    run::run_dir(git_root, run_id).join("sandbox.json")
}

pub fn write_sandbox_meta(git_root: &Path, meta: &SandboxMeta) -> Result<(), ExitError> {
    let bytes = serde_json::to_vec_pretty(meta)
        .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to encode sandbox meta: {e}")))?;
    fs::write(sandbox_meta_path(git_root, &meta.run_id), bytes).map_err(|e| {
        ExitError::new(
            EXIT_GENERAL,
            format!("failed to write sandbox.json for {}: {e}", meta.run_id),
        )
    })?;
    Ok(())
}

pub fn read_sandbox_meta(git_root: &Path, run_id: &str) -> Option<SandboxMeta> {
    let p = sandbox_meta_path(git_root, run_id);
    let bytes = fs::read(p).ok()?;
    serde_json::from_slice(&bytes).ok()
}

pub fn list_sandbox_metas(git_root: &Path) -> Vec<SandboxMeta> {
    let dir = sandboxes_dir(git_root);
    if !dir.exists() {
        return vec![];
    }

    let mut out = vec![];
    let Ok(rd) = fs::read_dir(&dir) else {
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
        if let Some(meta) = read_sandbox_meta(git_root, &run_id) {
            out.push(meta);
        }
    }

    out.sort_by(|a, b| a.run_id.cmp(&b.run_id));
    out
}

pub fn create_detached_worktree(
    git_root: &Path,
    path: &Path,
    commit: &str,
) -> Result<(), ExitError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            ExitError::new(EXIT_GENERAL, format!("failed to create worktree dir: {e}"))
        })?;
    }

    if path.exists() {
        return Err(ExitError::new(
            EXIT_GENERAL,
            format!(
                "refusing to create worktree because path already exists: {}",
                path.display()
            ),
        ));
    }

    let path_s = path.to_string_lossy().to_string();
    git::run_git(git_root, ["worktree", "add", "--detach", &path_s, commit]).map(|_| ())
}

pub fn remove_worktree_best_effort(git_root: &Path, path: &Path) {
    if !path.exists() {
        return;
    }

    let path_s = path.to_string_lossy().to_string();
    let _ = git::run_git(git_root, ["worktree", "remove", "--force", &path_s]);
}

pub fn assert_is_git_worktree_dir(path: &Path) -> Result<(), ExitError> {
    let out = git::run_git_in(path, ["rev-parse", "--is-inside-work-tree"])?;
    if out.trim() != "true" {
        return Err(ExitError::new(
            EXIT_GENERAL,
            format!("expected a git worktree at {}", path.display()),
        ));
    }
    Ok(())
}

pub fn ensure_sandbox_for_run(
    git_root: &Path,
    session_name: &str,
    run_id: &str,
    created_at: String,
) -> Result<SandboxMeta, ExitError> {
    let session_state = session::ensure_session(git_root, session_name, created_at.clone())?;
    let base_commit = session_state.head;

    let sandbox_path = sandbox_dir(git_root, run_id);
    create_detached_worktree(git_root, &sandbox_path, &base_commit)?;

    let meta = SandboxMeta {
        run_id: run_id.to_string(),
        session: session_name.to_string(),
        created_at,
        base_commit,
        path: sandbox_path.display().to_string(),
    };
    write_sandbox_meta(git_root, &meta)?;
    Ok(meta)
}

#[derive(Debug, Serialize)]
struct TestM1SetupOut {
    run_id: String,
    session: session::SessionState,
    sandbox: SandboxMeta,
}

pub fn test_m1_setup(git_root: &Path, args: TestM1SetupArgs) -> Result<(), ExitError> {
    let created_at = lock::now_rfc3339();

    let lock_path = lock::default_lock_path(git_root);
    let info = lock::make_lock_info(git_root, "__test_m1_setup", &[]);
    let _guard = lock::LockGuard::acquire(&lock_path, info)?;

    let session_state = session::ensure_session(git_root, &args.session, created_at.clone())?;

    // Create a run record first so recovery has a stable handle.
    let run_meta = run::create_run(git_root, "__test_m1_setup", &[], created_at.clone())?;
    let sandbox = ensure_sandbox_for_run(git_root, &args.session, &run_meta.run_id, created_at)?;

    let out = TestM1SetupOut {
        run_id: run_meta.run_id,
        session: session_state,
        sandbox,
    };

    let s = serde_json::to_string_pretty(&out)
        .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to encode json: {e}")))?;
    println!("{}", s);
    Ok(())
}

pub fn test_m1_cleanup_sandbox(
    git_root: &Path,
    args: TestM1CleanupSandboxArgs,
) -> Result<(), ExitError> {
    let sandbox_path = sandbox_dir(git_root, &args.run_id);
    remove_worktree_best_effort(git_root, &sandbox_path);
    Ok(())
}

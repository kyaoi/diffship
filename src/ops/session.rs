use crate::cli::{SessionRepairArgs, TestM1AdvanceSessionArgs};
use crate::exit::{EXIT_GENERAL, ExitError};
use crate::git;
use crate::ops::lock;
use crate::ops::worktree;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

const DEFAULT_SESSION_NAME: &str = "default";

fn sessions_state_dir(git_root: &Path) -> PathBuf {
    git_root.join(".diffship").join("sessions")
}

pub fn session_ref(name: &str) -> String {
    format!("refs/diffship/sessions/{name}")
}

pub fn session_worktree_dir(git_root: &Path, name: &str) -> PathBuf {
    worktree::sessions_dir(git_root).join(name)
}

pub fn session_state_path(git_root: &Path, name: &str) -> PathBuf {
    sessions_state_dir(git_root).join(format!("{name}.json"))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionState {
    pub name: String,
    pub r#ref: String,
    pub created_at: String,
    pub updated_at: String,
    pub base_branch: Option<String>,
    pub base_commit: String,
    pub head: String,
    pub worktree_path: String,
}

pub fn validate_session_name(name: &str) -> Result<(), ExitError> {
    if name.is_empty() {
        return Err(ExitError::new(
            EXIT_GENERAL,
            "session name must not be empty",
        ));
    }
    if name == "." || name == ".." {
        return Err(ExitError::new(EXIT_GENERAL, "invalid session name"));
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
    {
        return Err(ExitError::new(
            EXIT_GENERAL,
            "session name must match [A-Za-z0-9_.-]+",
        ));
    }
    Ok(())
}

pub fn read_session_state(git_root: &Path, name: &str) -> Option<SessionState> {
    let bytes = fs::read(session_state_path(git_root, name)).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn write_session_state(git_root: &Path, state: &SessionState) -> Result<(), ExitError> {
    fs::create_dir_all(sessions_state_dir(git_root))
        .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to create session dir: {e}")))?;

    let bytes = serde_json::to_vec_pretty(state).map_err(|e| {
        ExitError::new(EXIT_GENERAL, format!("failed to encode session state: {e}"))
    })?;
    fs::write(session_state_path(git_root, &state.name), bytes)
        .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to write session state: {e}")))?;
    Ok(())
}

fn detect_base_branch(git_root: &Path) -> Option<String> {
    // Prefer symbolic-ref to avoid returning "HEAD" when detached.
    let out = git::run_git(git_root, ["symbolic-ref", "--quiet", "--short", "HEAD"]).ok()?;
    let s = out.trim();
    if s.is_empty() {
        None
    } else {
        Some(s.to_string())
    }
}

fn ensure_ref_exists(git_root: &Path, name: &str, commit: &str) -> Result<(), ExitError> {
    let r = session_ref(name);
    if git::rev_parse(git_root, &r).is_ok() {
        return Ok(());
    }
    git::run_git(git_root, ["update-ref", &r, commit]).map(|_| ())
}

fn hard_reset_clean_best_effort(worktree_path: &Path, commit: &str) {
    let _ = git::run_git_in(worktree_path, ["reset", "--hard", commit]);
    let _ = git::run_git_in(worktree_path, ["clean", "-fdx"]);
}

pub fn ensure_session(git_root: &Path, name: &str, now: String) -> Result<SessionState, ExitError> {
    let name = if name.is_empty() {
        DEFAULT_SESSION_NAME
    } else {
        name
    };
    validate_session_name(name)?;

    let base_commit = git::rev_parse(git_root, "HEAD")?;

    // If a ref already exists, reuse it. Otherwise seed it from current HEAD.
    let r = session_ref(name);
    let head = match git::rev_parse(git_root, &r) {
        Ok(h) => h,
        Err(_) => {
            ensure_ref_exists(git_root, name, &base_commit)?;
            base_commit.clone()
        }
    };

    let mut state = read_session_state(git_root, name).unwrap_or_else(|| SessionState {
        name: name.to_string(),
        r#ref: r.clone(),
        created_at: now.clone(),
        updated_at: now.clone(),
        base_branch: detect_base_branch(git_root),
        base_commit: base_commit.clone(),
        head: head.clone(),
        worktree_path: session_worktree_dir(git_root, name).display().to_string(),
    });

    // Ensure the worktree exists.
    let wt_path = session_worktree_dir(git_root, name);
    if !wt_path.exists() {
        worktree::create_detached_worktree(git_root, &wt_path, &head)?;
    } else {
        // Refuse to silently proceed if the directory is not a worktree.
        worktree::assert_is_git_worktree_dir(&wt_path).map_err(|e| {
            ExitError::new(
                e.code,
                format!(
                    "session worktree path exists but is not a worktree: {}\nhint: remove it and rerun (path is under .diffship/worktrees)",
                    wt_path.display()
                ),
            )
        })?;
    }

    // Keep it clean and synced to the ref.
    hard_reset_clean_best_effort(&wt_path, &head);

    state.updated_at = now;
    state.base_commit = base_commit;
    state.head = head;
    state.worktree_path = wt_path.display().to_string();
    write_session_state(git_root, &state)?;

    Ok(state)
}

pub fn advance_session(
    git_root: &Path,
    name: &str,
    new_head: &str,
    now: String,
) -> Result<SessionState, ExitError> {
    validate_session_name(name)?;

    let r = session_ref(name);
    git::run_git(git_root, ["update-ref", &r, new_head]).map(|_| ())?;

    let mut state = ensure_session(git_root, name, now.clone())?;
    state.head = new_head.to_string();
    state.updated_at = now;
    write_session_state(git_root, &state)?;

    // Sync the worktree explicitly to the new ref.
    let wt_path = session_worktree_dir(git_root, name);
    hard_reset_clean_best_effort(&wt_path, new_head);

    Ok(state)
}

pub fn list_sessions(git_root: &Path) -> Vec<SessionState> {
    let dir = sessions_state_dir(git_root);
    if !dir.exists() {
        return vec![];
    }

    let mut out = vec![];
    let Ok(rd) = fs::read_dir(dir) else {
        return vec![];
    };

    for ent in rd.flatten() {
        let p = ent.path();
        if p.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let Ok(bytes) = fs::read(&p) else {
            continue;
        };
        let Ok(state) = serde_json::from_slice::<SessionState>(&bytes) else {
            continue;
        };
        out.push(state);
    }

    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

#[derive(Debug, Serialize)]
struct SessionRepairSummary {
    session: String,
    head: String,
    worktree_path: String,
}

pub fn cmd_repair(git_root: &Path, args: SessionRepairArgs) -> Result<(), ExitError> {
    let now = lock::now_rfc3339();
    let lock_path = lock::default_lock_path(git_root);
    let info = lock::make_lock_info(
        git_root,
        "session repair",
        &[format!("--session={}", args.session)],
    );
    let _guard = lock::LockGuard::acquire(&lock_path, info)?;

    let state = repair_session(git_root, &args.session, now)?;
    let summary = SessionRepairSummary {
        session: state.name.clone(),
        head: state.head.clone(),
        worktree_path: state.worktree_path.clone(),
    };

    println!("diffship session repair: ok");
    println!("  session : {}", summary.session);
    println!("  head    : {}", summary.head);
    println!("  wt      : {}", summary.worktree_path);
    Ok(())
}

pub fn repair_session(git_root: &Path, name: &str, now: String) -> Result<SessionState, ExitError> {
    validate_session_name(name)?;

    let active_sandboxes = worktree::list_sandbox_metas(git_root)
        .into_iter()
        .filter(|meta| meta.session == name && Path::new(&meta.path).exists())
        .map(|meta| meta.run_id)
        .collect::<Vec<_>>();
    if !active_sandboxes.is_empty() {
        return Err(ExitError::new(
            EXIT_GENERAL,
            format!(
                "refused: session {} still has active sandboxes: {}\nremove or finish them before repair",
                name,
                active_sandboxes.join(", ")
            ),
        ));
    }

    let head = git::rev_parse(git_root, "HEAD")?;
    let worktree_path = session_worktree_dir(git_root, name);
    if worktree_path.exists() {
        match worktree::assert_is_git_worktree_dir(&worktree_path) {
            Ok(()) => hard_reset_clean_best_effort(&worktree_path, &head),
            Err(_) => {
                let _ = fs::remove_dir_all(&worktree_path);
            }
        }
    }
    if !worktree_path.exists() {
        worktree::create_detached_worktree(git_root, &worktree_path, &head)?;
    }
    hard_reset_clean_best_effort(&worktree_path, &head);

    let r = session_ref(name);
    git::run_git(git_root, ["update-ref", &r, &head]).map(|_| ())?;

    let created_at = read_session_state(git_root, name)
        .map(|s| s.created_at)
        .unwrap_or_else(|| now.clone());
    let state = SessionState {
        name: name.to_string(),
        r#ref: r,
        created_at,
        updated_at: now,
        base_branch: detect_base_branch(git_root),
        base_commit: head.clone(),
        head,
        worktree_path: worktree_path.display().to_string(),
    };
    write_session_state(git_root, &state)?;

    Ok(state)
}

pub fn test_m1_advance_session(
    git_root: &Path,
    args: TestM1AdvanceSessionArgs,
) -> Result<(), ExitError> {
    let now = lock::now_rfc3339();
    let lock_path = lock::default_lock_path(git_root);
    let info = lock::make_lock_info(
        git_root,
        "__test_m1_advance_session",
        &[format!("--run-id={}", args.run_id)],
    );
    let _guard = lock::LockGuard::acquire(&lock_path, info)?;

    let sandbox = worktree::read_sandbox_meta(git_root, &args.run_id).ok_or_else(|| {
        ExitError::new(
            EXIT_GENERAL,
            format!("missing sandbox.json for run_id {}", args.run_id),
        )
    })?;

    let new_head = git::run_git_in(Path::new(&sandbox.path), ["rev-parse", "HEAD"])?;
    let new_head = new_head.trim().to_string();

    let _state = advance_session(git_root, &args.session, &new_head, now)?;
    Ok(())
}

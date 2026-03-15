use crate::cli::CleanupArgs;
use crate::exit::{EXIT_GENERAL, ExitError};
use crate::git;
use crate::ops::lock;
use crate::ops::run;
use crate::ops::session;
use crate::ops::worktree;
use serde::Serialize;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize)]
struct CleanupReport {
    dry_run: bool,
    removed: usize,
    bytes_targeted: u64,
    failed: usize,
    items: Vec<CleanupItem>,
}

#[derive(Debug, Serialize)]
struct CleanupItem {
    kind: String,
    name: String,
    path: String,
    reason: String,
    size_bytes: u64,
    removed: bool,
    error: Option<String>,
}

#[derive(Debug, Clone)]
enum CleanupKind {
    PromotedSandbox,
    OrphanSandbox,
    OrphanSessionWorktree,
    PromotedRun,
    OrphanRun,
    BuildArtifact,
    RulesArtifact,
}

impl CleanupKind {
    fn as_str(&self) -> &'static str {
        match self {
            Self::PromotedSandbox => "promoted_sandbox",
            Self::OrphanSandbox => "orphan_sandbox",
            Self::OrphanSessionWorktree => "orphan_session_worktree",
            Self::PromotedRun => "promoted_run",
            Self::OrphanRun => "orphan_run",
            Self::BuildArtifact => "build_artifact",
            Self::RulesArtifact => "rules_artifact",
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct CleanupScopes {
    include_runs: bool,
    include_builds: bool,
}

#[derive(Debug, Clone)]
struct CleanupCandidate {
    kind: CleanupKind,
    name: String,
    path: PathBuf,
    reason: String,
    size_bytes: u64,
    extra_paths: Vec<PathBuf>,
    extra_files: Vec<PathBuf>,
}

pub fn cmd(git_root: &Path, args: CleanupArgs) -> Result<(), ExitError> {
    let lock_path = lock::default_lock_path(git_root);
    let scopes = CleanupScopes::from_args(&args);
    let info = lock::make_lock_info(
        git_root,
        "cleanup",
        &[
            format!("--dry-run={}", args.dry_run),
            format!("--include-runs={}", scopes.include_runs),
            format!("--include-builds={}", scopes.include_builds),
            format!("--all={}", args.all),
            format!("--json={}", args.json),
        ],
    );
    let _guard = lock::LockGuard::acquire(&lock_path, info)?;

    let candidates = collect_candidates(git_root, scopes)?;
    let mut items = Vec::with_capacity(candidates.len());
    let mut removed = 0usize;
    let mut failed = 0usize;
    let mut bytes_targeted = 0u64;
    let mut removed_any = false;

    for candidate in candidates {
        bytes_targeted += candidate.size_bytes;
        let (item, did_remove) = if args.dry_run {
            (
                CleanupItem {
                    kind: candidate.kind.as_str().to_string(),
                    name: candidate.name,
                    path: candidate.path.display().to_string(),
                    reason: candidate.reason,
                    size_bytes: candidate.size_bytes,
                    removed: false,
                    error: None,
                },
                false,
            )
        } else {
            let result = remove_candidate(git_root, &candidate);
            let removed_now = result.is_ok();
            let error = result.err().map(|e| e.message);
            (
                CleanupItem {
                    kind: candidate.kind.as_str().to_string(),
                    name: candidate.name,
                    path: candidate.path.display().to_string(),
                    reason: candidate.reason,
                    size_bytes: candidate.size_bytes,
                    removed: removed_now,
                    error,
                },
                removed_now,
            )
        };

        if item.removed {
            removed += 1;
            removed_any = true;
        }
        if item.error.is_some() {
            failed += 1;
        }
        if did_remove {
            removed_any = true;
        }
        items.push(item);
    }

    if removed_any && !args.dry_run {
        let _ = git::run_git(git_root, ["worktree", "prune"]);
    }

    let report = CleanupReport {
        dry_run: args.dry_run,
        removed,
        bytes_targeted,
        failed,
        items,
    };

    if args.json {
        let s = serde_json::to_string_pretty(&report)
            .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to encode json: {e}")))?;
        println!("{}", s);
    } else {
        print_report(&report);
    }

    if report.failed > 0 {
        return Err(ExitError::new(
            EXIT_GENERAL,
            format!("cleanup failed for {} item(s)", report.failed),
        ));
    }

    Ok(())
}

impl CleanupScopes {
    fn from_args(args: &CleanupArgs) -> Self {
        Self {
            include_runs: args.all || args.include_runs,
            include_builds: args.all || args.include_builds,
        }
    }
}

fn print_report(report: &CleanupReport) {
    if report.items.is_empty() {
        if report.dry_run {
            println!("diffship cleanup: nothing to do (dry-run)");
        } else {
            println!("diffship cleanup: nothing to do");
        }
        return;
    }

    if report.dry_run {
        println!("diffship cleanup: dry-run");
    } else {
        println!("diffship cleanup: ok");
    }
    println!("  items   : {}", report.items.len());
    println!("  removed : {}", report.removed);
    println!("  bytes   : {}", report.bytes_targeted);
    for item in &report.items {
        let suffix = if let Some(error) = &item.error {
            format!(" error={}", error)
        } else if report.dry_run {
            " action=would-remove".to_string()
        } else if item.removed {
            " action=removed".to_string()
        } else {
            " action=kept".to_string()
        };
        println!(
            "  - {} {} size={} path={} reason={}{}",
            item.kind, item.name, item.size_bytes, item.path, item.reason, suffix
        );
    }
}

fn collect_candidates(
    git_root: &Path,
    scopes: CleanupScopes,
) -> Result<Vec<CleanupCandidate>, ExitError> {
    let mut out = Vec::new();
    collect_sandbox_candidates(git_root, scopes, &mut out)?;
    if scopes.include_runs {
        collect_run_candidates(git_root, &mut out)?;
    }
    if scopes.include_builds {
        collect_build_artifacts(git_root, &mut out)?;
    }
    collect_orphan_session_worktrees(git_root, &mut out)?;
    out.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(out)
}

fn collect_sandbox_candidates(
    git_root: &Path,
    scopes: CleanupScopes,
    out: &mut Vec<CleanupCandidate>,
) -> Result<(), ExitError> {
    let dir = worktree::sandboxes_dir(git_root);
    if !dir.exists() {
        return Ok(());
    }

    for ent in fs::read_dir(&dir)
        .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to read sandboxes dir: {e}")))?
    {
        let ent = ent.map_err(|e| {
            ExitError::new(EXIT_GENERAL, format!("failed to read sandbox entry: {e}"))
        })?;
        if !ent.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }

        let run_id = ent.file_name().to_string_lossy().to_string();
        let path = ent.path();
        let run_dir = run::run_dir(git_root, &run_id);
        let size_bytes = dir_size_bytes(&path);

        if !run_dir.exists() || worktree::read_sandbox_meta(git_root, &run_id).is_none() {
            if !scopes.include_runs || !run_dir.exists() {
                out.push(CleanupCandidate {
                    kind: CleanupKind::OrphanSandbox,
                    name: run_id,
                    path,
                    reason: "run metadata is missing or invalid".to_string(),
                    size_bytes,
                    extra_paths: Vec::new(),
                    extra_files: if run_dir.exists() {
                        vec![run_dir.join("sandbox.json")]
                    } else {
                        Vec::new()
                    },
                });
            }
            continue;
        }

        if run_is_promoted(&run_dir) && !scopes.include_runs {
            out.push(CleanupCandidate {
                kind: CleanupKind::PromotedSandbox,
                name: run_id,
                path,
                reason: "run already has a promoted head".to_string(),
                size_bytes,
                extra_paths: Vec::new(),
                extra_files: vec![run_dir.join("sandbox.json")],
            });
        }
    }

    Ok(())
}

fn collect_run_candidates(
    git_root: &Path,
    out: &mut Vec<CleanupCandidate>,
) -> Result<(), ExitError> {
    let dir = run::runs_dir(git_root);
    if !dir.exists() {
        return Ok(());
    }

    for ent in fs::read_dir(&dir)
        .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to read runs dir: {e}")))?
    {
        let ent = ent
            .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to read run entry: {e}")))?;
        if !ent.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }

        let run_id = ent.file_name().to_string_lossy().to_string();
        let run_dir = ent.path();
        let sandbox_path = worktree::sandbox_dir(git_root, &run_id);
        let sandbox_meta = worktree::read_sandbox_meta(git_root, &run_id);
        let sandbox_exists = sandbox_path.exists();
        let sandbox_size = dir_size_bytes(&sandbox_path);
        let mut extra_paths = Vec::new();
        if sandbox_exists {
            extra_paths.push(sandbox_path);
        }

        if run_is_promoted(&run_dir) {
            out.push(CleanupCandidate {
                kind: CleanupKind::PromotedRun,
                name: run_id,
                path: run_dir.clone(),
                reason: "run already has a promoted head".to_string(),
                size_bytes: dir_size_bytes(&run_dir) + sandbox_size,
                extra_paths,
                extra_files: Vec::new(),
            });
            continue;
        }

        if sandbox_meta.is_none() || !sandbox_exists {
            out.push(CleanupCandidate {
                kind: CleanupKind::OrphanRun,
                name: run_id,
                path: run_dir.clone(),
                reason: "sandbox metadata is missing or the sandbox worktree no longer exists"
                    .to_string(),
                size_bytes: dir_size_bytes(&run_dir) + sandbox_size,
                extra_paths,
                extra_files: Vec::new(),
            });
        }
    }

    Ok(())
}

fn collect_orphan_session_worktrees(
    git_root: &Path,
    out: &mut Vec<CleanupCandidate>,
) -> Result<(), ExitError> {
    let dir = worktree::sessions_dir(git_root);
    if !dir.exists() {
        return Ok(());
    }

    for ent in fs::read_dir(&dir)
        .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to read sessions dir: {e}")))?
    {
        let ent = ent.map_err(|e| {
            ExitError::new(EXIT_GENERAL, format!("failed to read session entry: {e}"))
        })?;
        if !ent.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }

        let name = ent.file_name().to_string_lossy().to_string();
        let has_state = session::read_session_state(git_root, &name).is_some();
        let has_ref = git::rev_parse(git_root, &session::session_ref(&name)).is_ok();
        if has_state || has_ref {
            continue;
        }

        let path = ent.path();
        out.push(CleanupCandidate {
            kind: CleanupKind::OrphanSessionWorktree,
            name,
            path: path.clone(),
            reason: "session state and ref are both missing".to_string(),
            size_bytes: dir_size_bytes(&path),
            extra_paths: Vec::new(),
            extra_files: Vec::new(),
        });
    }

    Ok(())
}

fn collect_build_artifacts(
    git_root: &Path,
    out: &mut Vec<CleanupCandidate>,
) -> Result<(), ExitError> {
    let artifacts_root = git_root.join(".diffship").join("artifacts");
    collect_artifact_entries(
        &artifacts_root.join("handoffs"),
        CleanupKind::BuildArtifact,
        "diffship-owned build artifact",
        out,
    )?;
    collect_artifact_entries(
        &artifacts_root.join("rules"),
        CleanupKind::RulesArtifact,
        "diffship-owned rules artifact",
        out,
    )?;
    Ok(())
}

fn collect_artifact_entries(
    dir: &Path,
    kind: CleanupKind,
    reason: &str,
    out: &mut Vec<CleanupCandidate>,
) -> Result<(), ExitError> {
    if !dir.exists() {
        return Ok(());
    }

    for ent in fs::read_dir(dir).map_err(|e| {
        ExitError::new(
            EXIT_GENERAL,
            format!("failed to read {}: {e}", dir.display()),
        )
    })? {
        let ent = ent.map_err(|e| {
            ExitError::new(
                EXIT_GENERAL,
                format!("failed to read artifact entry under {}: {e}", dir.display()),
            )
        })?;
        let path = ent.path();
        let file_type = ent.file_type().map_err(|e| {
            ExitError::new(
                EXIT_GENERAL,
                format!("failed to inspect artifact entry {}: {e}", path.display()),
            )
        })?;
        if !(file_type.is_dir() || file_type.is_file()) {
            continue;
        }

        out.push(CleanupCandidate {
            kind: kind.clone(),
            name: ent.file_name().to_string_lossy().to_string(),
            path: path.clone(),
            reason: reason.to_string(),
            size_bytes: dir_size_bytes(&path),
            extra_paths: Vec::new(),
            extra_files: Vec::new(),
        });
    }

    Ok(())
}

fn run_is_promoted(run_dir: &Path) -> bool {
    let path = run_dir.join("promotion.json");
    let Ok(bytes) = fs::read(path) else {
        return false;
    };
    let Ok(value) = serde_json::from_slice::<Value>(&bytes) else {
        return false;
    };
    value
        .get("promoted_head")
        .and_then(|v| v.as_str())
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false)
}

fn remove_candidate(git_root: &Path, candidate: &CleanupCandidate) -> Result<(), ExitError> {
    for path in &candidate.extra_paths {
        remove_owned_path(git_root, path)?;
    }
    for path in &candidate.extra_files {
        if path.exists() {
            fs::remove_file(path).map_err(|e| {
                ExitError::new(
                    EXIT_GENERAL,
                    format!("failed to remove {}: {e}", path.display()),
                )
            })?;
        }
    }
    remove_owned_path(git_root, &candidate.path)?;

    Ok(())
}

fn remove_owned_path(git_root: &Path, path: &Path) -> Result<(), ExitError> {
    if !path.exists() {
        return Ok(());
    }

    if path.starts_with(worktree::worktrees_dir(git_root)) {
        worktree::remove_worktree_best_effort(git_root, path);
    }
    if !path.exists() {
        return Ok(());
    }
    let meta = fs::symlink_metadata(path).map_err(|e| {
        ExitError::new(
            EXIT_GENERAL,
            format!("failed to inspect {}: {e}", path.display()),
        )
    })?;
    if meta.is_file() {
        fs::remove_file(path).map_err(|e| {
            ExitError::new(
                EXIT_GENERAL,
                format!("failed to remove {}: {e}", path.display()),
            )
        })?;
        return Ok(());
    }
    fs::remove_dir_all(path).map_err(|e| {
        ExitError::new(
            EXIT_GENERAL,
            format!("failed to remove {}: {e}", path.display()),
        )
    })?;
    Ok(())
}

fn dir_size_bytes(path: &Path) -> u64 {
    let Ok(meta) = fs::symlink_metadata(path) else {
        return 0;
    };
    if meta.is_file() {
        return meta.len();
    }
    if !meta.is_dir() {
        return 0;
    }

    let mut total = 0u64;
    let Ok(rd) = fs::read_dir(path) else {
        return 0;
    };
    for ent in rd.flatten() {
        total += dir_size_bytes(&ent.path());
    }
    total
}

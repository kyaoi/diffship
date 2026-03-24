use crate::cli::PackFixArgs;
use crate::exit::{EXIT_GENERAL, ExitError};
use crate::ops::config;
use crate::ops::lock;
use crate::ops::run;
use crate::ops::strategy;
use crate::ops::worktree;
use crate::pathing::resolve_user_path;
use serde::Deserialize;
use serde_json::Value;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use zip::ZipWriter;
use zip::write::FileOptions;

#[derive(Debug, Deserialize)]
struct VerifyJson {
    ok: Option<bool>,
    profile: Option<String>,
    failure_category: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PostApplyBrief {
    #[serde(default)]
    changed_paths: Vec<String>,
    #[serde(default)]
    change_categories: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct PhaseSummaryBrief {
    ok: Option<bool>,
    failure_category: Option<String>,
}

/// Create a "reprompt zip" that contains run metadata, bundle, verify logs, and sandbox diffs.
///
/// Default output path: `.diffship/runs/<run-id>/pack-fix_<timestamp>_<head7>[_N].zip`.
pub fn cmd(git_root: &Path, args: PackFixArgs) -> Result<(), ExitError> {
    let created_at = lock::now_rfc3339();
    let cwd = std::env::current_dir()
        .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to detect current dir: {e}")))?;

    let lock_path = lock::default_lock_path(git_root);
    let info = lock::make_lock_info(
        git_root,
        "pack-fix",
        &[
            format!("--run-id={}", args.run_id.as_deref().unwrap_or("")),
            format!("--out={}", args.out.as_deref().unwrap_or("")),
        ],
    );
    let _guard = lock::LockGuard::acquire(&lock_path, info)?;

    let run_id = match &args.run_id {
        Some(id) => id.clone(),
        None => detect_latest_run_id(git_root).ok_or_else(|| {
            ExitError::new(
                EXIT_GENERAL,
                "no runs found (run diffship apply first, or pass --run-id)",
            )
        })?,
    };

    let run_dir = run::run_dir(git_root, &run_id);
    if !run_dir.exists() {
        return Err(ExitError::new(
            EXIT_GENERAL,
            format!("run not found: {}", run_id),
        ));
    }

    let out_path = match &args.out {
        Some(p) => resolve_user_path(&cwd, p)?,
        None => default_pack_fix_zip_path(&run_dir, &run_id),
    };

    let sb = worktree::read_sandbox_meta(git_root, &run_id);
    let sandbox_path = sb.map(|m| PathBuf::from(m.path));

    write_pack_fix_zip(
        git_root,
        &run_id,
        &run_dir,
        sandbox_path.as_deref(),
        &out_path,
        &created_at,
    )?;

    println!("diffship pack-fix: created {}", out_path.display());
    Ok(())
}

/// Best-effort creation used by `verify` / `loop` on failure.
pub fn try_write_default_pack_fix_zip(
    git_root: &Path,
    run_id: &str,
    run_dir: &Path,
    sandbox_path: &Path,
    created_at: &str,
) -> Result<PathBuf, ExitError> {
    let out_path = default_pack_fix_zip_path(run_dir, run_id);
    write_pack_fix_zip(
        git_root,
        run_id,
        run_dir,
        Some(sandbox_path),
        &out_path,
        created_at,
    )?;
    Ok(out_path)
}

pub(crate) fn default_pack_fix_zip_path(run_dir: &Path, run_id: &str) -> PathBuf {
    run_dir.join(default_pack_fix_zip_name(run_dir, run_id))
}

fn write_pack_fix_zip(
    git_root: &Path,
    run_id: &str,
    run_dir: &Path,
    sandbox_path: Option<&Path>,
    out_path: &Path,
    created_at: &str,
) -> Result<(), ExitError> {
    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            ExitError::new(EXIT_GENERAL, format!("failed to create output dir: {e}"))
        })?;
    }

    let file = fs::File::create(out_path)
        .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to create zip: {e}")))?;
    let mut zip = ZipWriter::new(file);

    let opts = FileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o644);

    // 1) PROMPT.md / SAFETY.md
    let verify_json_path = run_dir.join("verify.json");
    let verify_info = read_verify_json_brief(&verify_json_path);
    let post_apply_json_path = run_dir.join("post_apply.json");
    let has_post_apply = post_apply_json_path.exists();
    let post_apply_info = read_post_apply_brief(&post_apply_json_path);
    let strategy_resolution = resolve_pack_fix_strategy(git_root, run_dir)?;

    let mut prompt = String::new();
    prompt.push_str("# diffship pack-fix (reprompt kit)\n\n");
    prompt.push_str("This zip is meant to be attached to an AI to produce a fix.\n\n");
    prompt.push_str(&format!("- run_id: `{}`\n", run_id));
    prompt.push_str(&format!("- created_at: `{}`\n", created_at));
    if let Some(profile) = verify_info.0.as_deref() {
        prompt.push_str(&format!("- verify_profile: `{}`\n", profile));
    }
    if let Some(ok) = verify_info.1 {
        prompt.push_str(&format!("- verify_ok: `{}`\n", ok));
    }
    if let Some(failure_category) = verify_info.2.as_deref() {
        prompt.push_str(&format!(
            "- verify_failure_category: `{}`\n",
            failure_category
        ));
    }
    if let Some(post_apply) = post_apply_info.as_ref() {
        prompt.push_str(&format!(
            "- post_apply_changed_paths: `{}`\n",
            post_apply.changed_paths.len()
        ));
        if !post_apply.change_categories.is_empty() {
            prompt.push_str(&format!(
                "- post_apply_change_categories: `{}`\n",
                post_apply.change_categories.join(",")
            ));
        }
    }
    if let Some(strategy) = strategy_resolution.as_ref() {
        prompt.push_str("\n## Suggested strategy\n\n");
        prompt.push_str(
            "Read `strategy.resolved.json` first so the failure-aware recommendation stays grounded before you inspect detailed logs.\n\n",
        );
        prompt.push_str(&format!(
            "- failure_category: `{}`\n",
            strategy.failure_category
        ));
        prompt.push_str(&format!("- strategy_mode: `{}`\n", strategy.strategy_mode));
        prompt.push_str(&format!(
            "- selected_profile: `{}`\n",
            strategy.selected_profile
        ));
        prompt.push_str(&format!(
            "- default_profile: `{}`\n",
            strategy.default_profile
        ));
        if !strategy.alternatives.is_empty() {
            prompt.push_str(&format!(
                "- alternatives: `{}`\n",
                strategy.alternatives.join("`, `")
            ));
        }
        if let Some(tests_expected) = strategy.tests_expected {
            prompt.push_str(&format!("- tests_expected: `{}`\n", tests_expected));
        }
        if let Some(profile) = strategy.preferred_verify_profile.as_deref() {
            prompt.push_str(&format!("- preferred_verify_profile: `{}`\n", profile));
        }
        prompt.push_str(&format!("- reason: {}\n", strategy.reason));
        prompt.push_str(
            "\nTreat this strategy as guidance; use the run evidence below if it points to a narrower fix.\n",
        );
    }
    prompt.push_str("\n## What you should do\n\n");
    if has_post_apply {
        prompt.push_str("1. Inspect `run/post_apply.json` and `run/post-apply/` first to see what local post-apply normalization changed or failed.\n");
        if let Some(post_apply) = post_apply_info.as_ref() {
            if !post_apply.changed_paths.is_empty() {
                prompt.push_str(&format!(
                    "   - changed paths: `{}`\n",
                    post_apply.changed_paths.join("`, `")
                ));
            }
            if !post_apply.change_categories.is_empty() {
                prompt.push_str(&format!(
                    "   - change categories: `{}`\n",
                    post_apply.change_categories.join(", ")
                ));
            }
        }
        prompt.push_str("2. Inspect `run/verify/` logs to see why verification failed after post-apply finished.\n");
        prompt.push_str("3. Inspect `sandbox/git_diff.patch` to see the current uncommitted changes in the sandbox.\n");
        prompt.push_str("4. Create a new patch bundle that fixes the failure, and re-run:\n\n");
    } else {
        prompt.push_str("1. Inspect `run/verify/` logs to see why verification failed.\n");
        prompt.push_str("2. Inspect `sandbox/git_diff.patch` to see the current uncommitted changes in the sandbox.\n");
        prompt.push_str("3. Create a new patch bundle that fixes the failure, and re-run:\n\n");
    }
    prompt.push_str("```bash\ndiffship loop <your-fix-bundle.zip>\n```\n\n");
    prompt.push_str("## Contents\n\n");
    prompt.push_str("- `run/`    : run metadata + apply/post-apply/verify summaries\n");
    prompt.push_str("- `bundle/` : original patch bundle (if present)\n");
    prompt.push_str("- `sandbox/`: git status + git diff from the sandbox worktree\n");

    add_bytes(&mut zip, opts, "PROMPT.md", prompt.as_bytes())?;

    let mut safety = String::new();
    safety.push_str("# Safety notice\n\n");
    safety.push_str("This zip may contain proprietary code or sensitive information.\n");
    safety.push_str("Review the contents before sharing it with any third party.\n");
    add_bytes(&mut zip, opts, "SAFETY.md", safety.as_bytes())?;
    if let Some(strategy) = strategy_resolution.as_ref() {
        let strategy_json = serde_json::to_vec_pretty(strategy).map_err(|e| {
            ExitError::new(
                EXIT_GENERAL,
                format!("failed to encode strategy.resolved.json: {e}"),
            )
        })?;
        add_bytes(&mut zip, opts, "strategy.resolved.json", &strategy_json)?;
    }

    // 2) run dir essentials
    add_if_exists(&mut zip, opts, run_dir.join("run.json"), "run/run.json")?;
    add_if_exists(&mut zip, opts, run_dir.join("apply.json"), "run/apply.json")?;
    add_if_exists(
        &mut zip,
        opts,
        run_dir.join("post_apply.json"),
        "run/post_apply.json",
    )?;
    add_if_exists(
        &mut zip,
        opts,
        run_dir.join("verify.json"),
        "run/verify.json",
    )?;
    let post_apply_dir = run_dir.join("post-apply");
    if post_apply_dir.exists() {
        add_dir_recursive(
            &mut zip,
            opts,
            &post_apply_dir,
            &post_apply_dir,
            "run/post-apply",
        )?;
    }

    let verify_dir = run_dir.join("verify");
    if verify_dir.exists() {
        add_dir_recursive(&mut zip, opts, &verify_dir, &verify_dir, "run/verify")?;
    }

    let bundle_dir = run_dir.join("bundle");
    if bundle_dir.exists() {
        add_dir_recursive(&mut zip, opts, &bundle_dir, &bundle_dir, "bundle")?;
    }

    // 3) sandbox artifacts
    if let Some(sb) = sandbox_path {
        let status = git_capture(sb, &["status", "--porcelain=v1"])?;
        add_bytes(&mut zip, opts, "sandbox/git_status.txt", status.as_bytes())?;

        let diff = git_capture(sb, &["diff", "--no-color"])?;
        add_bytes(&mut zip, opts, "sandbox/git_diff.patch", diff.as_bytes())?;

        // include HEAD for convenience
        let head = git_capture(sb, &["rev-parse", "HEAD"])?;
        add_bytes(&mut zip, opts, "sandbox/head.txt", head.as_bytes())?;

        // also include git_root for reference (relative)
        let rel = path_relative_display(git_root, sb);
        add_bytes(
            &mut zip,
            opts,
            "sandbox/path.txt",
            format!("{}\n", rel).as_bytes(),
        )?;
    } else {
        add_bytes(
            &mut zip,
            opts,
            "sandbox/README.txt",
            b"Sandbox worktree not found. This run may not have a sandbox.\n",
        )?;
    }

    zip.finish()
        .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to finalize zip: {e}")))?;
    Ok(())
}

fn read_verify_json_brief(path: &Path) -> (Option<String>, Option<bool>, Option<String>) {
    let Ok(bytes) = fs::read(path) else {
        return (None, None, None);
    };
    let Ok(v) = serde_json::from_slice::<VerifyJson>(&bytes) else {
        return (None, None, None);
    };
    (v.profile, v.ok, v.failure_category)
}

fn read_post_apply_brief(path: &Path) -> Option<PostApplyBrief> {
    let bytes = fs::read(path).ok()?;
    serde_json::from_slice::<PostApplyBrief>(&bytes).ok()
}

fn resolve_pack_fix_strategy(
    git_root: &Path,
    run_dir: &Path,
) -> Result<Option<strategy::StrategyResolution>, ExitError> {
    let failure_category = detect_failure_category(run_dir);
    if failure_category.is_none() {
        return Ok(None);
    }
    let cfg = config::resolve_workflow_config(git_root)?;
    Ok(strategy::resolve_strategy_from_workflow(
        &cfg,
        failure_category.as_deref(),
    ))
}

fn detect_failure_category(run_dir: &Path) -> Option<String> {
    for name in ["promotion.json", "verify.json", "apply.json"] {
        let Some(summary) = read_phase_summary(&run_dir.join(name)) else {
            continue;
        };
        if summary.ok == Some(false)
            && let Some(category) = summary.failure_category
            && !category.trim().is_empty()
        {
            return Some(category);
        }
    }
    None
}

fn read_phase_summary(path: &Path) -> Option<PhaseSummaryBrief> {
    let bytes = fs::read(path).ok()?;
    serde_json::from_slice::<PhaseSummaryBrief>(&bytes).ok()
}

fn default_pack_fix_zip_name(run_dir: &Path, run_id: &str) -> String {
    if let Some(stem) = run_id.strip_prefix("run_") {
        return format!("pack-fix_{stem}.zip");
    }

    let base_label = detect_base_label(run_dir).unwrap_or_else(|| "HEAD".to_string());
    format!("pack-fix_{}_{}.zip", run_id, base_label)
}

fn detect_base_label(run_dir: &Path) -> Option<String> {
    let apply_path = run_dir.join("apply.json");
    let bytes = fs::read(apply_path).ok()?;
    let value = serde_json::from_slice::<Value>(&bytes).ok()?;
    for key in ["effective_base_commit", "base_commit"] {
        let Some(raw) = value.get(key).and_then(|v| v.as_str()) else {
            continue;
        };
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.chars().all(|c| c.is_ascii_hexdigit()) {
            return Some(crate::git::short_sha_label(trimmed));
        }
        let sanitized = sanitize_label(trimmed);
        if !sanitized.is_empty() {
            return Some(sanitized);
        }
    }
    None
}

fn sanitize_label(raw: &str) -> String {
    let mut out = String::new();
    let mut last_was_sep = false;
    for ch in raw.chars() {
        let mapped = if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
            Some(ch)
        } else {
            Some('-')
        };
        if let Some(ch) = mapped {
            if ch == '-' && last_was_sep {
                continue;
            }
            last_was_sep = ch == '-';
            out.push(ch);
        }
        if out.len() >= 24 {
            break;
        }
    }
    out.trim_matches('-').to_string()
}

fn add_bytes<W: Write + io::Seek>(
    zip: &mut ZipWriter<W>,
    opts: FileOptions,
    name: &str,
    bytes: &[u8],
) -> Result<(), ExitError> {
    zip.start_file(name, opts)
        .map_err(|e| ExitError::new(EXIT_GENERAL, format!("zip start_file failed: {e}")))?;
    zip.write_all(bytes)
        .map_err(|e| ExitError::new(EXIT_GENERAL, format!("zip write failed: {e}")))?;
    Ok(())
}

fn add_if_exists<W: Write + io::Seek>(
    zip: &mut ZipWriter<W>,
    opts: FileOptions,
    src: PathBuf,
    dst: &str,
) -> Result<(), ExitError> {
    if !src.exists() {
        return Ok(());
    }
    let bytes = fs::read(&src).map_err(|e| {
        ExitError::new(
            EXIT_GENERAL,
            format!("failed to read {}: {e}", src.display()),
        )
    })?;
    add_bytes(zip, opts, dst, &bytes)
}

fn add_dir_recursive<W: Write + io::Seek>(
    zip: &mut ZipWriter<W>,
    opts: FileOptions,
    base: &Path,
    cur: &Path,
    prefix: &str,
) -> Result<(), ExitError> {
    for ent in fs::read_dir(cur).map_err(|e| {
        ExitError::new(
            EXIT_GENERAL,
            format!("failed to read dir {}: {e}", cur.display()),
        )
    })? {
        let ent = ent
            .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to read dir entry: {e}")))?;
        let path = ent.path();
        if ent.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            add_dir_recursive(zip, opts, base, &path, prefix)?;
        } else {
            let rel = path
                .strip_prefix(base)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            let name = format!("{}/{}", prefix.trim_end_matches('/'), rel);
            let bytes = fs::read(&path).map_err(|e| {
                ExitError::new(
                    EXIT_GENERAL,
                    format!("failed to read {}: {e}", path.display()),
                )
            })?;
            add_bytes(zip, opts, &name, &bytes)?;
        }
    }
    Ok(())
}

fn git_capture(dir: &Path, args: &[&str]) -> Result<String, ExitError> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to run git: {e}")))?;
    let mut s = String::new();
    s.push_str(&String::from_utf8_lossy(&out.stdout));
    s.push_str(&String::from_utf8_lossy(&out.stderr));
    if out.status.success() {
        Ok(s)
    } else {
        Err(ExitError::new(
            EXIT_GENERAL,
            format!(
                "git command failed: git -C {} {}",
                dir.display(),
                args.join(" ")
            ),
        ))
    }
}

fn detect_latest_run_id(git_root: &Path) -> Option<String> {
    // Prefer the newest run by created_at (RFC3339 is lexicographically sortable).
    let dir = run::runs_dir(git_root);
    if !dir.exists() {
        return None;
    }
    let mut best: Option<(String, String)> = None;
    for ent in fs::read_dir(&dir).ok()?.flatten() {
        if !ent.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        let meta_path = ent.path().join("run.json");
        let bytes = fs::read(&meta_path).ok()?;
        let meta: run::RunMeta = serde_json::from_slice(&bytes).ok()?;
        match &best {
            Some((best_created, best_id)) => {
                if meta.created_at > *best_created
                    || (meta.created_at == *best_created && meta.run_id > *best_id)
                {
                    best = Some((meta.created_at, meta.run_id));
                }
            }
            None => best = Some((meta.created_at, meta.run_id)),
        }
    }
    best.map(|(_, id)| id)
}

fn path_relative_display(git_root: &Path, p: &Path) -> String {
    if let Ok(rel) = p.strip_prefix(git_root) {
        return format!("./{}", rel.display());
    }
    p.display().to_string()
}

use crate::exit::{EXIT_FORBIDDEN_PATH, EXIT_GENERAL, ExitError};
use crate::filter;
use serde::Deserialize;
use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

use crate::ops::config;
use crate::ops::tasks;

/// Parsed and validated patch bundle.
///
/// This module is intentionally strict: validation happens before touching any worktree.

#[derive(Debug, Clone, Deserialize)]
pub struct PatchBundleManifest {
    pub protocol_version: String,
    pub task_id: String,
    pub base_commit: String,
    pub apply_mode: ApplyMode,
    pub touched_files: Vec<String>,

    #[serde(default)]
    pub created_by: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub requires_docs_update: Option<bool>,
    #[serde(default)]
    pub requires_plan_update: Option<bool>,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub tasks_required: Option<bool>,
    #[serde(default)]
    pub secrets_ack_required: Option<bool>,

    // M4: config overrides (optional)
    #[serde(default)]
    pub verify_profile: Option<String>,
    #[serde(default)]
    pub target_branch: Option<String>,
    #[serde(default)]
    pub promotion_mode: Option<String>,
    #[serde(default)]
    pub commit_policy: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ApplyMode {
    GitApply,
    GitAm,
}

impl ApplyMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            ApplyMode::GitApply => "git-apply",
            ApplyMode::GitAm => "git-am",
        }
    }
}

#[derive(Debug, Default)]
struct PatchDiffBlock {
    diff_old_path: Option<String>,
    diff_new_path: Option<String>,
    plus_new_path: Option<String>,
    old_is_dev_null: bool,
    new_file_mode: Option<String>,
    old_mode: Option<String>,
    new_mode: Option<String>,
}

impl PatchDiffBlock {
    fn from_diff_header(rest: &str) -> Self {
        let mut it = rest.split_whitespace();
        let diff_old_path = it.next().map(|s| s.trim_start_matches("a/").to_string());
        let diff_new_path = it.next().map(|s| s.trim_start_matches("b/").to_string());
        Self {
            diff_old_path,
            diff_new_path,
            ..Default::default()
        }
    }

    fn validate(self, patch_path: &Path) -> Result<(), ExitError> {
        if self.old_mode.is_some() || self.new_mode.is_some() {
            return Err(ExitError::new(
                EXIT_FORBIDDEN_PATH,
                format!(
                    "file mode changes are refused (found in {})",
                    patch_path.display()
                ),
            ));
        }

        let Some(mode) = self.new_file_mode.as_deref() else {
            return Ok(());
        };

        match mode {
            "100644" | "100755" => {
                if !self.old_is_dev_null {
                    return Err(ExitError::new(
                        EXIT_FORBIDDEN_PATH,
                        format!(
                            "new file mode is only allowed for /dev/null additions (found in {})",
                            patch_path.display()
                        ),
                    ));
                }
                if self.diff_old_path.is_none()
                    || self.diff_new_path.is_none()
                    || self.plus_new_path.as_deref() != self.diff_new_path.as_deref()
                {
                    return Err(ExitError::new(
                        EXIT_FORBIDDEN_PATH,
                        format!(
                            "new file patch is missing /dev/null or +++ headers (found in {})",
                            patch_path.display()
                        ),
                    ));
                }
            }
            "160000" => {
                return Err(ExitError::new(
                    EXIT_FORBIDDEN_PATH,
                    format!(
                        "submodule changes are refused (found in {})",
                        patch_path.display()
                    ),
                ));
            }
            other => {
                return Err(ExitError::new(
                    EXIT_FORBIDDEN_PATH,
                    format!(
                        "unsupported new file mode {other} (found in {})",
                        patch_path.display()
                    ),
                ));
            }
        }

        Ok(())
    }
}

#[derive(Debug)]
pub struct PatchBundle {
    /// Root directory that contains manifest.yaml and changes/.
    pub root: PathBuf,
    /// The manifest.yaml parsed content.
    pub manifest: PatchBundleManifest,
    /// Absolute paths to patch files under changes/ (sorted).
    pub patches: Vec<PathBuf>,
    /// Copy of the bundle saved under the run directory.
    pub run_bundle_dir: PathBuf,
}

/// Load and parse manifest.yaml from a run directory's saved bundle copy.
pub fn load_manifest_from_run_bundle(run_dir: &Path) -> Result<PatchBundleManifest, ExitError> {
    let p = run_dir.join("bundle").join("manifest.yaml");
    let bytes = fs::read(&p).map_err(|e| {
        ExitError::new(
            EXIT_GENERAL,
            format!("failed to read bundle manifest from {}: {e}", p.display()),
        )
    })?;
    let text = String::from_utf8_lossy(&bytes);
    parse_manifest_yaml(&text)
}

pub fn rewrite_run_manifest_base_commit(
    run_dir: &Path,
    base_commit: &str,
) -> Result<(), ExitError> {
    let path = run_dir.join("bundle").join("manifest.yaml");
    let text = fs::read_to_string(&path).map_err(|e| {
        ExitError::new(
            EXIT_GENERAL,
            format!(
                "failed to read bundle manifest from {}: {e}",
                path.display()
            ),
        )
    })?;

    let mut replaced = false;
    let mut out = Vec::with_capacity(text.lines().count());
    for line in text.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("base_commit:") {
            out.push(format!("base_commit: \"{}\"", base_commit));
            replaced = true;
        } else {
            out.push(line.to_string());
        }
    }

    if !replaced {
        return Err(ExitError::new(
            EXIT_GENERAL,
            format!("manifest missing base_commit in {}", path.display()),
        ));
    }

    fs::write(&path, format!("{}\n", out.join("\n"))).map_err(|e| {
        ExitError::new(
            EXIT_GENERAL,
            format!("failed to rewrite bundle manifest {}: {e}", path.display()),
        )
    })
}

pub fn load_and_copy_into_run(
    git_root: &Path,
    bundle_path: &Path,
    run_dir: &Path,
    extra_forbidden_patterns: &[String],
    editable_diffship_files: &[String],
) -> Result<PatchBundle, ExitError> {
    let materialized_root = materialize_bundle_root(bundle_path, run_dir)?;
    let root = detect_bundle_root(&materialized_root)?;

    let manifest_path = root.join("manifest.yaml");
    let bytes = fs::read(&manifest_path).map_err(|e| {
        ExitError::new(
            EXIT_GENERAL,
            format!(
                "missing manifest.yaml (expected at {}): {e}",
                manifest_path.display()
            ),
        )
    })?;

    let manifest_text = String::from_utf8_lossy(&bytes);
    let manifest = parse_manifest_yaml(&manifest_text)?;

    validate_manifest(&manifest)?;
    validate_touched_files(
        &manifest.touched_files,
        extra_forbidden_patterns,
        editable_diffship_files,
    )?;

    tasks::validate_tasks_contract(&manifest, &root)?;

    let patches = collect_patches(&root)?;
    validate_patches(
        git_root,
        &patches,
        extra_forbidden_patterns,
        editable_diffship_files,
    )?;

    let run_bundle_dir = run_dir.join("bundle");
    copy_bundle_subset(&root, &run_bundle_dir)?;

    Ok(PatchBundle {
        root,
        manifest,
        patches,
        run_bundle_dir,
    })
}

fn validate_manifest(m: &PatchBundleManifest) -> Result<(), ExitError> {
    if m.protocol_version.trim() != "1" {
        return Err(ExitError::new(
            EXIT_GENERAL,
            format!(
                "unsupported protocol_version: {} (expected \"1\")",
                m.protocol_version
            ),
        ));
    }
    if m.task_id.trim().is_empty() {
        return Err(ExitError::new(
            EXIT_GENERAL,
            "manifest.task_id must not be empty",
        ));
    }
    if m.base_commit.trim().is_empty() {
        return Err(ExitError::new(
            EXIT_GENERAL,
            "manifest.base_commit must not be empty",
        ));
    }
    if m.touched_files.is_empty() {
        return Err(ExitError::new(
            EXIT_GENERAL,
            "manifest.touched_files must not be empty",
        ));
    }
    // Read optional fields (forward-compat) so -D warnings doesn't treat them as dead code.
    let _ = m.created_by.as_deref();
    let _ = m.created_at.as_deref();
    let _ = m.requires_docs_update;
    let _ = m.requires_plan_update;
    let _ = m.notes.as_deref();
    let _ = m.tasks_required;
    let _ = m.secrets_ack_required;
    let _ = m.verify_profile.as_deref();
    let _ = m.target_branch.as_deref();
    let _ = m.promotion_mode.as_deref();
    let _ = m.commit_policy.as_deref();

    Ok(())
}

fn parse_manifest_yaml(s: &str) -> Result<PatchBundleManifest, ExitError> {
    // Minimal YAML parser for v1 patch bundles.
    // Supported:
    // - top-level scalars: key: value
    // - touched_files: list ("- path" lines)
    //
    // This is intentionally strict and avoids bringing in a YAML dependency.
    let mut protocol_version: Option<String> = None;
    let mut task_id: Option<String> = None;
    let mut base_commit: Option<String> = None;
    let mut apply_mode: Option<ApplyMode> = None;
    let mut touched_files: Vec<String> = vec![];

    // Optional keys
    let mut created_by: Option<String> = None;
    let mut created_at: Option<String> = None;
    let mut requires_docs_update: Option<bool> = None;
    let mut requires_plan_update: Option<bool> = None;
    let mut notes: Option<String> = None;
    let mut tasks_required: Option<bool> = None;
    let mut secrets_ack_required: Option<bool> = None;
    let mut verify_profile: Option<String> = None;
    let mut target_branch: Option<String> = None;
    let mut promotion_mode: Option<String> = None;
    let mut commit_policy: Option<String> = None;

    let mut in_touched_files = false;

    for raw in s.lines() {
        let line = raw.trim_end();
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // list item (only for touched_files)
        if in_touched_files {
            if let Some(item) = trimmed.strip_prefix("-") {
                let v = unquote(item.trim());
                if v.is_empty() {
                    return Err(ExitError::new(EXIT_GENERAL, "invalid touched_files item"));
                }
                touched_files.push(v.to_string());
                continue;
            }
            // New top-level key begins.
            if !trimmed.starts_with('-') {
                in_touched_files = false;
            }
        }

        // key: value
        let Some((k, v)) = split_kv(trimmed) else {
            return Err(ExitError::new(
                EXIT_GENERAL,
                format!("invalid manifest line: {trimmed}"),
            ));
        };

        match k {
            "protocol_version" => protocol_version = Some(unquote(v).to_string()),
            "task_id" => task_id = Some(unquote(v).to_string()),
            "base_commit" => base_commit = Some(unquote(v).to_string()),
            "apply_mode" => {
                let m = unquote(v);
                apply_mode = Some(match m {
                    "git-apply" => ApplyMode::GitApply,
                    "git-am" => ApplyMode::GitAm,
                    _ => {
                        return Err(ExitError::new(
                            EXIT_GENERAL,
                            format!("invalid apply_mode: {m} (expected git-apply|git-am)"),
                        ));
                    }
                });
            }
            "touched_files" => {
                in_touched_files = true;
            }

            // optional keys
            "created_by" => created_by = Some(unquote(v).to_string()),
            "created_at" => created_at = Some(unquote(v).to_string()),
            "requires_docs_update" => requires_docs_update = parse_bool(v)?,
            "requires_plan_update" => requires_plan_update = parse_bool(v)?,
            "notes" => notes = Some(unquote(v).to_string()),
            "tasks_required" => tasks_required = parse_bool(v)?,
            "secrets_ack_required" => secrets_ack_required = parse_bool(v)?,
            "verify_profile" => verify_profile = Some(unquote(v).to_string()),
            "target_branch" => target_branch = Some(unquote(v).to_string()),
            "promotion_mode" => promotion_mode = Some(unquote(v).to_string()),
            "commit_policy" => commit_policy = Some(unquote(v).to_string()),

            _ => {
                // Ignore unknown keys for forward compatibility.
            }
        }
    }

    Ok(PatchBundleManifest {
        protocol_version: protocol_version
            .ok_or_else(|| ExitError::new(EXIT_GENERAL, "manifest missing protocol_version"))?,
        task_id: task_id.ok_or_else(|| ExitError::new(EXIT_GENERAL, "manifest missing task_id"))?,
        base_commit: base_commit
            .ok_or_else(|| ExitError::new(EXIT_GENERAL, "manifest missing base_commit"))?,
        apply_mode: apply_mode
            .ok_or_else(|| ExitError::new(EXIT_GENERAL, "manifest missing apply_mode"))?,
        touched_files,

        created_by,
        created_at,
        requires_docs_update,
        requires_plan_update,
        notes,
        tasks_required,
        secrets_ack_required,
        verify_profile,
        target_branch,
        promotion_mode,
        commit_policy,
    })
}

fn split_kv(s: &str) -> Option<(&str, &str)> {
    let idx = s.find(':')?;
    let (k, rest) = s.split_at(idx);
    let v = rest.trim_start_matches(':').trim();
    Some((k.trim(), v))
}

fn unquote(s: &str) -> &str {
    let s = s.trim();
    if s.len() >= 2
        && ((s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')))
    {
        return &s[1..s.len() - 1];
    }
    s
}

fn parse_bool(s: &str) -> Result<Option<bool>, ExitError> {
    let t = unquote(s).trim();
    if t.is_empty() {
        return Ok(None);
    }
    match t {
        "true" => Ok(Some(true)),
        "false" => Ok(Some(false)),
        _ => Err(ExitError::new(
            EXIT_GENERAL,
            format!("invalid bool: {t} (expected true|false)"),
        )),
    }
}

fn validate_touched_files(
    paths: &[String],
    extra_forbidden_patterns: &[String],
    editable_diffship_files: &[String],
) -> Result<(), ExitError> {
    for p in paths {
        validate_repo_relative_path(p, extra_forbidden_patterns, editable_diffship_files)?;
    }
    Ok(())
}

fn validate_repo_relative_path(
    path: &str,
    extra_forbidden_patterns: &[String],
    editable_diffship_files: &[String],
) -> Result<(), ExitError> {
    let path = path.trim();
    if path.is_empty() {
        return Err(ExitError::new(
            EXIT_FORBIDDEN_PATH,
            "empty path is not allowed",
        ));
    }
    let pb = Path::new(path);

    if pb.is_absolute() {
        return Err(ExitError::new(
            EXIT_FORBIDDEN_PATH,
            format!("absolute paths are forbidden: {path}"),
        ));
    }

    for c in pb.components() {
        match c {
            Component::ParentDir => {
                return Err(ExitError::new(
                    EXIT_FORBIDDEN_PATH,
                    format!("path traversal is forbidden: {path}"),
                ));
            }
            Component::Prefix(_) => {
                return Err(ExitError::new(
                    EXIT_FORBIDDEN_PATH,
                    format!("windows drive prefixes are forbidden: {path}"),
                ));
            }
            _ => {}
        }
    }

    // Forbidden prefixes (default safety policy): .git/ and .diffship/
    let s = config::normalize_repo_relative_path(path);
    if s == ".git" || s.starts_with(".git/") {
        return Err(ExitError::new(
            EXIT_FORBIDDEN_PATH,
            format!("refusing to touch forbidden path: {path}"),
        ));
    }
    let allow_editable_diffship_path = editable_diffship_files
        .iter()
        .any(|candidate| candidate == &s)
        && config::normalize_supported_editable_diffship_path(&s).is_some();
    if (s == ".diffship" || s.starts_with(".diffship/")) && !allow_editable_diffship_path {
        return Err(ExitError::new(
            EXIT_FORBIDDEN_PATH,
            format!("refusing to touch forbidden path: {path}"),
        ));
    }
    for pattern in extra_forbidden_patterns {
        if filter::pattern_matches_path(pattern, &s) {
            return Err(ExitError::new(
                EXIT_FORBIDDEN_PATH,
                format!("refusing to touch forbidden path by config: {path}"),
            ));
        }
    }

    Ok(())
}

fn collect_patches(root: &Path) -> Result<Vec<PathBuf>, ExitError> {
    let changes = root.join("changes");
    if !changes.exists() {
        return Err(ExitError::new(
            EXIT_GENERAL,
            format!("missing changes/ directory at {}", changes.display()),
        ));
    }

    let mut patches = vec![];
    for ent in fs::read_dir(&changes).map_err(|e| {
        ExitError::new(
            EXIT_GENERAL,
            format!("failed to read changes/ directory: {e}"),
        )
    })? {
        let ent = ent.map_err(|e| {
            ExitError::new(EXIT_GENERAL, format!("failed to read changes entry: {e}"))
        })?;
        if !ent.file_type().map(|t| t.is_file()).unwrap_or(false) {
            continue;
        }
        let p = ent.path();
        if p.extension().and_then(|x| x.to_str()) != Some("patch") {
            continue;
        }
        patches.push(p);
    }
    patches.sort();
    if patches.is_empty() {
        return Err(ExitError::new(
            EXIT_GENERAL,
            "changes/ must contain at least one .patch",
        ));
    }
    Ok(patches)
}

fn validate_patches(
    _git_root: &Path,
    patches: &[PathBuf],
    extra_forbidden_patterns: &[String],
    editable_diffship_files: &[String],
) -> Result<(), ExitError> {
    for p in patches {
        let mut s = String::new();
        fs::File::open(p)
            .and_then(|mut f| f.read_to_string(&mut s))
            .map_err(|e| {
                ExitError::new(
                    EXIT_GENERAL,
                    format!("failed to read patch {}: {e}", p.display()),
                )
            })?;

        // MVP patch policy (see docs/SPEC_V1.md S-OPS-005): refuse risky features.
        if s.contains("GIT binary patch") {
            return Err(ExitError::new(
                EXIT_FORBIDDEN_PATH,
                format!("binary patches are refused (found in {})", p.display()),
            ));
        }
        if s.contains("rename from")
            || s.contains("rename to")
            || s.contains("copy from")
            || s.contains("copy to")
        {
            return Err(ExitError::new(
                EXIT_FORBIDDEN_PATH,
                format!("rename/copy metadata is refused (found in {})", p.display()),
            ));
        }

        // Submodule changes typically include mode 160000.
        if s.contains(" 160000") || s.contains("\t160000") || s.contains("new file mode 160000") {
            return Err(ExitError::new(
                EXIT_FORBIDDEN_PATH,
                format!("submodule changes are refused (found in {})", p.display()),
            ));
        }

        let mut block: Option<PatchDiffBlock> = None;
        for line in s.lines() {
            if let Some(rest) = line.strip_prefix("diff --git ") {
                if let Some(prev) = block.take() {
                    prev.validate(p)?;
                }
                let mut it = rest.split_whitespace();
                let Some(a0) = it.next() else {
                    continue;
                };
                let Some(b0) = it.next() else {
                    continue;
                };
                let a = a0.trim_start_matches("a/");
                let b = b0.trim_start_matches("b/");
                validate_repo_relative_path(a, extra_forbidden_patterns, editable_diffship_files)?;
                validate_repo_relative_path(b, extra_forbidden_patterns, editable_diffship_files)?;
                block = Some(PatchDiffBlock::from_diff_header(rest));
                continue;
            }

            let Some(current) = block.as_mut() else {
                continue;
            };
            if line == "--- /dev/null" {
                current.old_is_dev_null = true;
            } else if let Some(path) = line.strip_prefix("+++ b/") {
                current.plus_new_path = Some(path.trim().to_string());
            } else if let Some(mode) = line.strip_prefix("new file mode ") {
                current.new_file_mode = Some(mode.trim().to_string());
            } else if let Some(mode) = line.strip_prefix("old mode ") {
                current.old_mode = Some(mode.trim().to_string());
            } else if let Some(mode) = line.strip_prefix("new mode ") {
                current.new_mode = Some(mode.trim().to_string());
            }
        }
        if let Some(prev) = block.take() {
            prev.validate(p)?;
        }
    }
    Ok(())
}

fn copy_bundle_subset(root: &Path, out: &Path) -> Result<(), ExitError> {
    if out.exists() {
        // Avoid confusing stale state.
        let _ = fs::remove_dir_all(out);
    }
    fs::create_dir_all(out).map_err(|e| {
        ExitError::new(
            EXIT_GENERAL,
            format!("failed to create bundle copy dir: {e}"),
        )
    })?;

    copy_file_if_exists(&root.join("manifest.yaml"), &out.join("manifest.yaml"))?;
    copy_file_if_exists(&root.join("summary.md"), &out.join("summary.md"))?;
    copy_file_if_exists(
        &root.join("constraints.yaml"),
        &out.join("constraints.yaml"),
    )?;
    copy_file_if_exists(
        &root.join("checks_request.yaml"),
        &out.join("checks_request.yaml"),
    )?;
    copy_file_if_exists(
        &root.join("commit_message.txt"),
        &out.join("commit_message.txt"),
    )?;

    // changes/*.patch (required)
    let changes_in = root.join("changes");
    let changes_out = out.join("changes");
    fs::create_dir_all(&changes_out).map_err(|e| {
        ExitError::new(
            EXIT_GENERAL,
            format!("failed to create changes copy dir: {e}"),
        )
    })?;
    for ent in fs::read_dir(&changes_in).map_err(|e| {
        ExitError::new(
            EXIT_GENERAL,
            format!(
                "failed to read changes copy dir {}: {e}",
                changes_in.display()
            ),
        )
    })? {
        let ent = ent.map_err(|e| {
            ExitError::new(EXIT_GENERAL, format!("failed to read changes entry: {e}"))
        })?;
        if !ent.file_type().map(|t| t.is_file()).unwrap_or(false) {
            continue;
        }
        let p = ent.path();
        if p.extension().and_then(|x| x.to_str()) != Some("patch") {
            continue;
        }
        let name = ent.file_name();
        fs::copy(&p, changes_out.join(name)).map_err(|e| {
            ExitError::new(
                EXIT_GENERAL,
                format!("failed to copy patch {}: {e}", p.display()),
            )
        })?;
    }

    // tasks/ (optional)
    let tasks_in = root.join("tasks");
    if tasks_in.exists() {
        let tasks_out = out.join("tasks");
        copy_dir_recursive(&tasks_in, &tasks_out)?;
    }

    Ok(())
}

fn copy_file_if_exists(src: &Path, dst: &Path) -> Result<(), ExitError> {
    if !src.exists() {
        return Ok(());
    }
    fs::copy(src, dst).map_err(|e| {
        ExitError::new(
            EXIT_GENERAL,
            format!("failed to copy {}: {e}", src.display()),
        )
    })?;
    Ok(())
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), ExitError> {
    fs::create_dir_all(dst).map_err(|e| {
        ExitError::new(
            EXIT_GENERAL,
            format!("failed to create dir {}: {e}", dst.display()),
        )
    })?;
    for ent in fs::read_dir(src).map_err(|e| {
        ExitError::new(
            EXIT_GENERAL,
            format!("failed to read dir {}: {e}", src.display()),
        )
    })? {
        let ent = ent
            .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to read dir entry: {e}")))?;
        let p = ent.path();
        let name = ent.file_name();
        let dst_p = dst.join(name);
        if ent.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            copy_dir_recursive(&p, &dst_p)?;
        } else if ent.file_type().map(|t| t.is_file()).unwrap_or(false) {
            fs::copy(&p, &dst_p).map_err(|e| {
                ExitError::new(EXIT_GENERAL, format!("failed to copy {}: {e}", p.display()))
            })?;
        }
    }
    Ok(())
}

fn materialize_bundle_root(bundle_path: &Path, run_dir: &Path) -> Result<PathBuf, ExitError> {
    if bundle_path.is_dir() {
        return Ok(bundle_path.to_path_buf());
    }

    let is_zip = bundle_path
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.eq_ignore_ascii_case("zip"))
        .unwrap_or(false);
    if !is_zip {
        return Err(ExitError::new(
            EXIT_GENERAL,
            format!(
                "bundle path must be a directory or .zip: {}",
                bundle_path.display()
            ),
        ));
    }

    let out_dir = run_dir.join("bundle_extracted");
    if out_dir.exists() {
        let _ = fs::remove_dir_all(&out_dir);
    }
    fs::create_dir_all(&out_dir)
        .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to create extract dir: {e}")))?;

    extract_zip_safely(bundle_path, &out_dir)?;
    Ok(out_dir)
}

fn extract_zip_safely(zip_path: &Path, out_dir: &Path) -> Result<(), ExitError> {
    // Use Python stdlib zipfile to validate entries (no abs paths / no traversal) and extract.
    // This avoids a Rust zip dependency in M2.
    let code = r#"
import sys, zipfile
from pathlib import PurePosixPath

zip_path = sys.argv[1]
out_dir = sys.argv[2]

z = zipfile.ZipFile(zip_path)
for name in z.namelist():
    # Normalize as posix path regardless of platform.
    p = PurePosixPath(name)
    if name.startswith('/') or name.startswith('\\'):
        print(f"unsafe zip entry (absolute): {name}", file=sys.stderr)
        sys.exit(2)
    if '..' in p.parts:
        print(f"unsafe zip entry (traversal): {name}", file=sys.stderr)
        sys.exit(2)

z.extractall(out_dir)
"#;

    let py = if Command::new("python3").arg("--version").output().is_ok() {
        "python3"
    } else if Command::new("python").arg("--version").output().is_ok() {
        "python"
    } else {
        return Err(ExitError::new(
            EXIT_GENERAL,
            "cannot extract .zip: python (or python3) is required",
        ));
    };

    let out = Command::new(py)
        .arg("-c")
        .arg(code)
        .arg(zip_path)
        .arg(out_dir)
        .output()
        .map_err(|e| {
            ExitError::new(
                EXIT_GENERAL,
                format!("failed to run python zip extract: {e}"),
            )
        })?;

    if out.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&out.stderr);
    Err(ExitError::new(
        EXIT_FORBIDDEN_PATH,
        format!("zip extraction refused: {}", stderr.trim()),
    ))
}

fn detect_bundle_root(materialized: &Path) -> Result<PathBuf, ExitError> {
    // Accept either:
    // 1) materialized is the root (contains manifest.yaml)
    // 2) materialized contains exactly one directory, and that directory is the root.

    if materialized.join("manifest.yaml").exists() {
        return Ok(materialized.to_path_buf());
    }

    let mut dirs = vec![];
    for ent in fs::read_dir(materialized).map_err(|e| {
        ExitError::new(
            EXIT_GENERAL,
            format!("failed to read bundle root {}: {e}", materialized.display()),
        )
    })? {
        let ent = ent.map_err(|e| {
            ExitError::new(EXIT_GENERAL, format!("failed to read bundle entry: {e}"))
        })?;
        if ent.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            dirs.push(ent.path());
        }
    }
    if dirs.len() == 1 {
        let root = dirs.remove(0);
        if root.join("manifest.yaml").exists() {
            return Ok(root);
        }
    }

    Err(ExitError::new(
        EXIT_GENERAL,
        format!(
            "failed to detect patch bundle root (expected manifest.yaml): {}",
            materialized.display()
        ),
    ))
}

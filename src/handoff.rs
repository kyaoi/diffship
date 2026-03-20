use crate::cli::BuildArgs;
use crate::exit::{EXIT_GENERAL, EXIT_PACKING_LIMITS, EXIT_SECRETS_WARNING, ExitError};
use crate::filter::PathFilter;
use crate::git;
use crate::handoff_config::{DEFAULT_PROFILE_NAME, HandoffConfig};
use crate::pathing::resolve_user_path;
use crate::plan::HandoffPlan;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use time::format_description;
use zip::write::FileOptions;
use zip::{CompressionMethod, DateTime as ZipDateTime, ZipWriter};

const AUTO_PATCH_MAX_BYTES: usize = 64 * 1024;
const DEFAULT_MAX_PARTS: usize = 20;
const DEFAULT_MAX_BYTES_PER_PART: u64 = 512 * 1024 * 1024;
const PROJECT_CONTEXT_PER_FILE_MAX_BYTES: usize = 64 * 1024;
const PROJECT_CONTEXT_TOTAL_MAX_BYTES: usize = 512 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RangeMode {
    Direct,
    MergeBase,
    Last,
    Root,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SplitBy {
    File,
    Commit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UntrackedMode {
    Auto,
    Patch,
    Raw,
    Meta,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BinaryMode {
    Raw,
    Patch,
    Meta,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProjectContextMode {
    None,
    Focused,
}

#[derive(Debug, Clone)]
struct RangePlan {
    mode: RangeMode,
    base: String,
    target: String,
    from_rev: Option<String>,
    to_rev: Option<String>,
    a_rev: Option<String>,
    b_rev: Option<String>,
    merge_base: Option<String>,
    commit_count: Option<u64>,
}

#[derive(Debug, Clone, Copy)]
struct SourceSelection {
    include_committed: bool,
    include_staged: bool,
    include_unstaged: bool,
    include_untracked: bool,
}

#[derive(Debug, Clone, Copy)]
struct BinaryPolicy {
    include_binary: bool,
    binary_mode: BinaryMode,
}

#[derive(Debug, Clone)]
struct PackingLimits {
    profile_label: String,
    max_parts: usize,
    max_bytes_per_part: u64,
}

impl PackingLimits {
    fn from_args(args: &BuildArgs) -> Result<Self, ExitError> {
        let max_parts = args.max_parts.unwrap_or(DEFAULT_MAX_PARTS);
        let max_bytes_per_part = args
            .max_bytes_per_part
            .unwrap_or(DEFAULT_MAX_BYTES_PER_PART);
        if max_parts == 0 {
            return Err(ExitError::new(EXIT_GENERAL, "--max-parts must be >= 1"));
        }
        if max_bytes_per_part == 0 {
            return Err(ExitError::new(
                EXIT_GENERAL,
                "--max-bytes-per-part must be >= 1",
            ));
        }
        Ok(Self {
            profile_label: args
                .profile
                .clone()
                .unwrap_or_else(|| DEFAULT_PROFILE_NAME.to_string()),
            max_parts,
            max_bytes_per_part,
        })
    }
}

impl SourceSelection {
    fn from_args(args: &BuildArgs) -> Result<Self, ExitError> {
        let sel = Self {
            include_committed: !args.no_committed,
            include_staged: args.include_staged,
            include_unstaged: args.include_unstaged,
            include_untracked: args.include_untracked,
        };

        if !sel.include_committed
            && !sel.include_staged
            && !sel.include_unstaged
            && !sel.include_untracked
        {
            return Err(ExitError::new(
                EXIT_GENERAL,
                "no sources selected (enable at least one of committed/staged/unstaged/untracked)",
            ));
        }

        Ok(sel)
    }
}

#[derive(Debug, Clone)]
struct FileRow {
    segment: String,
    status: String,
    path: String,
    note: String,
    ins: Option<u64>,
    del: Option<u64>,
    bytes: Option<u64>,
    part: String,
}

#[derive(Debug, Clone)]
struct SegmentOutput {
    name: String,
    patch: String,
    rows: Vec<FileRow>,
}

#[derive(Debug, Clone)]
struct PartOutput {
    name: String,
    patch: String,
    segments: Vec<String>,
}

#[derive(Debug, Clone)]
struct AttachmentEntry {
    zip_path: String,
    bytes: Vec<u8>,
    reason: String,
}

#[derive(Debug, Clone)]
struct ExclusionEntry {
    path: String,
    reason: String,
    guidance: String,
}

#[derive(Debug, Clone, Default)]
struct SegmentBuildResult {
    segment: Option<SegmentOutput>,
    rows: Vec<FileRow>,
    attachments: Vec<AttachmentEntry>,
    exclusions: Vec<ExclusionEntry>,
}

#[derive(Debug, Clone)]
struct CommitView {
    hash7: String,
    subject: String,
    date: String,
    files: Vec<(String, String)>,
    ins: Option<u64>,
    del: Option<u64>,
}

#[derive(Debug, Clone)]
struct SecretHit {
    path: String,
    reason: String,
}

#[derive(Debug, Clone)]
struct BuildOutputPaths {
    staging_dir: PathBuf,
    final_dir: Option<PathBuf>,
    final_zip: Option<PathBuf>,
    cleanup_staging: bool,
}

#[derive(Debug, Serialize)]
struct HandoffManifest {
    schema_version: u32,
    patch_canonical: bool,
    entrypoint: String,
    current_head: String,
    sources: ManifestSources,
    #[serde(skip_serializing_if = "Option::is_none")]
    committed_range: Option<ManifestCommittedRange>,
    filters: ManifestFilters,
    packing: ManifestPacking,
    warnings: ManifestWarnings,
    summary: ManifestSummary,
    reading_order: Vec<String>,
    artifacts: ManifestArtifacts,
    parts: Vec<ManifestPart>,
    task_groups: Vec<ManifestTaskGroup>,
    files: Vec<ManifestFile>,
    commit_views: Vec<ManifestCommitView>,
    attachments: Vec<ManifestAttachment>,
    exclusions: Vec<ManifestExclusion>,
    secret_hits: Vec<ManifestSecretHit>,
}

#[derive(Debug, Serialize)]
struct ManifestSources {
    committed: bool,
    staged: bool,
    unstaged: bool,
    untracked: bool,
    split_by: String,
    untracked_mode: String,
    include_binary: bool,
    binary_mode: String,
}

#[derive(Debug, Serialize)]
struct ManifestCommittedRange {
    mode: String,
    base: String,
    target: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    from_rev: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    to_rev: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    a_rev: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    b_rev: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    merge_base: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    commit_count: Option<u64>,
}

#[derive(Debug, Serialize)]
struct ManifestFilters {
    diffshipignore: bool,
    include: Vec<String>,
    exclude: Vec<String>,
}

#[derive(Debug, Serialize)]
struct ManifestPacking {
    profile: String,
    max_parts: usize,
    max_bytes_per_part: u64,
    reduced_context_paths: Vec<String>,
}

#[derive(Debug, Serialize)]
struct ManifestWarnings {
    reduced_context_count: usize,
    exclusion_count: usize,
    secret_hit_count: usize,
}

#[derive(Debug, Serialize)]
struct ManifestSummary {
    file_count: usize,
    part_count: usize,
    commit_view_count: usize,
    categories: PartContextCategoryCounts,
    segments: BTreeMap<String, usize>,
    statuses: BTreeMap<String, usize>,
}

#[derive(Debug, Serialize)]
struct ManifestArtifacts {
    handoff_md: String,
    manifest_json: String,
    context_xml: String,
    ai_requests_md: String,
    part_paths: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    project_context_json: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    project_context_md: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    project_context_snapshot_root: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    attachments_zip: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    excluded_md: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    secrets_md: Option<String>,
}

#[derive(Debug, Serialize)]
struct ManifestPart {
    part_id: String,
    patch_path: String,
    context_path: String,
    segments: Vec<String>,
    approx_bytes: u64,
    file_count: usize,
    first_files: Vec<String>,
    reduced_context_paths: Vec<String>,
}

#[derive(Debug, Serialize, Clone)]
struct ManifestTaskGroup {
    task_id: String,
    intent_labels: Vec<String>,
    primary_labels: Vec<String>,
    task_shape_labels: Vec<String>,
    edit_targets: Vec<String>,
    context_only_files: Vec<String>,
    review_labels: Vec<String>,
    verification_targets: Vec<String>,
    verification_labels: Vec<String>,
    widening_labels: Vec<String>,
    execution_labels: Vec<String>,
    part_ids: Vec<String>,
    segments: Vec<String>,
    top_files: Vec<String>,
    related_context_paths: Vec<String>,
    related_project_files: Vec<String>,
    suggested_read_order: Vec<String>,
    risk_hints: Vec<String>,
    part_count: usize,
    file_count: usize,
}

#[derive(Debug, Serialize)]
struct ManifestFile {
    category: String,
    segment: String,
    status: String,
    path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    ins: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    del: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    part: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    note: Option<String>,
    change_hints: ManifestFileChangeHints,
    semantic: ManifestFileSemantic,
}

#[derive(Debug, Serialize, Clone)]
struct ManifestFileChangeHints {
    new_file: bool,
    deleted_file: bool,
    rename_or_copy: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    previous_path: Option<String>,
    stored_as_attachment: bool,
    excluded: bool,
    reduced_context: bool,
}

#[derive(Debug, Serialize, Clone)]
struct ManifestFileSemantic {
    language: String,
    generated_like: bool,
    lockfile: bool,
    ci_or_tooling: bool,
    coarse_labels: Vec<String>,
    related_test_candidates: Vec<String>,
    related_source_candidates: Vec<String>,
    related_doc_candidates: Vec<String>,
    related_config_candidates: Vec<String>,
}

#[derive(Debug, Serialize)]
struct ManifestCommitView {
    hash7: String,
    subject: String,
    date: String,
    files: Vec<ManifestCommitFile>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ins: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    del: Option<u64>,
}

#[derive(Debug, Serialize)]
struct ManifestCommitFile {
    path: String,
    part: String,
}

#[derive(Debug, Serialize)]
struct ManifestAttachment {
    path: String,
    reason: String,
    byte_len: usize,
}

#[derive(Debug, Serialize)]
struct ManifestExclusion {
    path: String,
    reason: String,
    guidance: String,
}

#[derive(Debug, Serialize)]
struct ManifestSecretHit {
    path: String,
    reason: String,
}

#[derive(Debug, Serialize)]
struct PartContext {
    schema_version: u32,
    patch_canonical: bool,
    part_id: String,
    patch_path: String,
    context_path: String,
    task_group_ref: String,
    task_shape_labels: Vec<String>,
    task_edit_targets: Vec<String>,
    task_context_only_files: Vec<String>,
    title: String,
    summary: String,
    intent: String,
    intent_labels: Vec<String>,
    review_labels: Vec<String>,
    segments: Vec<String>,
    files: Vec<ManifestFile>,
    scoped_context: PartScopedContext,
    diff_stats: PartContextDiffStats,
    scope: PartContextScope,
    constraints: PartContextConstraints,
    warnings: PartContextWarnings,
    acceptance_criteria: Vec<String>,
}

#[derive(Debug, Serialize)]
struct PartScopedContext {
    hunk_headers: Vec<String>,
    symbol_like_names: Vec<String>,
    import_like_refs: Vec<String>,
    related_test_candidates: Vec<String>,
    files: Vec<PartScopedFileContext>,
}

#[derive(Debug, Serialize)]
struct PartScopedFileContext {
    path: String,
    hunk_headers: Vec<String>,
    symbol_like_names: Vec<String>,
    import_like_refs: Vec<String>,
}

#[derive(Debug, Serialize)]
struct PartContextDiffStats {
    file_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    additions: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    deletions: Option<u64>,
    categories: PartContextCategoryCounts,
    segments: BTreeMap<String, usize>,
    statuses: BTreeMap<String, usize>,
}

#[derive(Debug, Serialize, Default)]
struct PartContextCategoryCounts {
    docs: usize,
    config: usize,
    source: usize,
    tests: usize,
    other: usize,
}

#[derive(Debug, Serialize)]
struct PartContextScope {
    in_scope: Vec<String>,
    out_of_scope: Vec<String>,
}

#[derive(Debug, Serialize)]
struct PartContextConstraints {
    handoff_entrypoint: String,
    manifest_path: String,
    patch_canonical: bool,
    reduced_context: bool,
}

#[derive(Debug, Serialize)]
struct PartContextWarnings {
    reduced_context_paths: Vec<String>,
    bundle_has_attachments: bool,
    bundle_has_exclusions: bool,
    bundle_has_secret_warnings: bool,
}

#[derive(Debug, Serialize, Clone)]
struct ProjectContextManifest {
    schema_version: u32,
    mode: String,
    patch_canonical: bool,
    entrypoint: String,
    rendered_view: String,
    snapshot_root: String,
    summary: ProjectContextSummary,
    top_level_dirs: Vec<ProjectContextTopLevelDir>,
    files: Vec<ProjectContextFile>,
    relationships: Vec<ProjectContextRelationship>,
}

#[derive(Debug, Serialize, Clone)]
struct ProjectContextSummary {
    selected_files: usize,
    changed_files: usize,
    supplemental_files: usize,
    included_snapshots: usize,
    omitted_files: usize,
    total_snapshot_bytes: usize,
    relationship_count: usize,
    categories: BTreeMap<String, usize>,
    priority_counts: BTreeMap<String, usize>,
    edit_scope_counts: BTreeMap<String, usize>,
    verification_relevance_counts: BTreeMap<String, usize>,
    relationship_kinds: BTreeMap<String, usize>,
}

#[derive(Debug, Serialize, Clone)]
struct ProjectContextTopLevelDir {
    path: String,
    file_count: usize,
}

#[derive(Debug, Serialize, Clone)]
struct ProjectContextFile {
    path: String,
    category: String,
    changed: bool,
    source_reasons: Vec<String>,
    usage_role: String,
    priority: String,
    edit_scope_role: String,
    verification_relevance: String,
    verification_labels: Vec<String>,
    why_included: Vec<String>,
    task_group_refs: Vec<String>,
    context_labels: Vec<String>,
    semantic: ManifestFileSemantic,
    outbound_relationships: Vec<ProjectContextFileRelationship>,
    inbound_relationships: Vec<ProjectContextFileRelationship>,
    exists_in_workspace: bool,
    included: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    snapshot_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    byte_len: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    omitted_reason: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
struct ProjectContextFileRelationship {
    kind: String,
    path: String,
}

#[derive(Debug, Serialize, Clone)]
struct ProjectContextRelationship {
    from: String,
    kind: String,
    to: String,
}

#[derive(Debug, Clone)]
struct ProjectContextSnapshot {
    path: String,
    bytes: Vec<u8>,
}

#[derive(Debug, Clone)]
struct ProjectContextBundle {
    manifest: ProjectContextManifest,
    markdown: String,
    snapshots: Vec<ProjectContextSnapshot>,
}

#[derive(Debug, Clone)]
struct ProjectContextFileInputs {
    path: String,
    changed: bool,
    source_reasons: Vec<String>,
    usage_role: String,
    priority: String,
    edit_scope_role: String,
    verification_relevance: String,
    verification_labels: Vec<String>,
    why_included: Vec<String>,
    semantic: ManifestFileSemantic,
    outbound_relationships: Vec<ProjectContextFileRelationship>,
    inbound_relationships: Vec<ProjectContextFileRelationship>,
}

#[derive(Debug, Clone)]
struct TaskGroupComputed {
    manifest: ManifestTaskGroup,
    all_files: BTreeSet<String>,
    related_project_files: BTreeSet<String>,
}

#[derive(Debug, Default, Clone, Copy)]
struct FilePatchClues {
    has_import_churn: bool,
    has_signature_change_like: bool,
    has_api_surface_like: bool,
}

pub fn cmd(git_root: &Path, args: BuildArgs) -> Result<(), ExitError> {
    let cwd = std::env::current_dir()
        .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to detect current dir: {e}")))?;
    let args = resolve_build_args(git_root, &cwd, args)?;
    let resolved_plan = HandoffPlan::from_build_args(&args);
    let head = git::rev_parse(git_root, "HEAD")?;

    let output_paths = resolve_output_paths(&cwd, &args, &head)?;
    let out_dir = output_paths.staging_dir.clone();
    let parts_dir = out_dir.join("parts");
    fs::create_dir_all(&parts_dir)
        .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to create output dir: {e}")))?;
    let result = (|| {
        let sources = SourceSelection::from_args(&args)?;
        let packing_limits = PackingLimits::from_args(&args)?;
        let filters = PathFilter::load(git_root, &args.include, &args.exclude)?;
        let plan = if sources.include_committed {
            Some(build_range_plan(git_root, &args)?)
        } else {
            None
        };
        let split_by = effective_split_by(args.split_by.as_deref(), plan.as_ref())?;
        let untracked_mode = parse_untracked_mode(&args.untracked_mode)?;
        let binary_policy = BinaryPolicy {
            include_binary: args.include_binary,
            binary_mode: parse_binary_mode(&args.binary_mode)?,
        };
        let project_context_mode = parse_project_context_mode(&args.project_context)?;

        let mut parts = Vec::<PartOutput>::new();
        let mut rows = Vec::<FileRow>::new();
        let mut attachments = Vec::<AttachmentEntry>::new();
        let mut exclusions = Vec::<ExclusionEntry>::new();
        let mut commit_views = Vec::<CommitView>::new();

        if let Some(plan) = plan.as_ref() {
            if split_by == SplitBy::Commit {
                let committed_parts = build_committed_parts_by_commit(git_root, plan)?;
                for cp in committed_parts {
                    let part_name = format!("part_{:02}.patch", parts.len() + 1);
                    let segment = filter_segment_output(cp.segment, &filters);
                    let mut segment = apply_tracked_binary_policy(
                        git_root,
                        segment,
                        binary_policy,
                        TrackedBinarySource::Commit(&cp.rev),
                    )?;
                    attachments.append(&mut segment.attachments);
                    exclusions.append(&mut segment.exclusions);
                    let mut segment = segment.segment;
                    if segment.rows.is_empty() && segment.patch.trim().is_empty() {
                        continue;
                    }
                    for row in &mut segment.rows {
                        if row.part.is_empty() {
                            row.part = part_name.clone();
                        }
                    }
                    let mut commit_files = segment
                        .rows
                        .iter()
                        .map(|r| {
                            (
                                r.path.clone(),
                                if r.part.is_empty() {
                                    part_name.clone()
                                } else {
                                    r.part.clone()
                                },
                            )
                        })
                        .collect::<Vec<_>>();
                    commit_files.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
                    commit_views.push(CommitView {
                        hash7: cp.hash7,
                        subject: cp.subject,
                        date: cp.date,
                        files: commit_files,
                        ins: sum_opt(segment.rows.iter().map(|r| r.ins)),
                        del: sum_opt(segment.rows.iter().map(|r| r.del)),
                    });
                    rows.extend(segment.rows.clone());
                    if !segment.patch.trim().is_empty() {
                        parts.push(PartOutput {
                            name: part_name,
                            patch: render_segmented_patch(std::slice::from_ref(&segment)),
                            segments: vec![segment.name],
                        });
                    }
                }
            } else {
                let part_name = format!("part_{:02}.patch", parts.len() + 1);
                let segment = filter_segment_output(
                    build_committed_segment(git_root, plan, &part_name)?,
                    &filters,
                );
                let mut segment = apply_tracked_binary_policy(
                    git_root,
                    segment,
                    binary_policy,
                    TrackedBinarySource::Commit(&plan.target),
                )?;
                attachments.append(&mut segment.attachments);
                exclusions.append(&mut segment.exclusions);
                let segment = segment.segment;
                if !segment.rows.is_empty() || !segment.patch.trim().is_empty() {
                    rows.extend(segment.rows.clone());
                    if !segment.patch.trim().is_empty() {
                        parts.push(PartOutput {
                            name: part_name,
                            patch: render_segmented_patch(std::slice::from_ref(&segment)),
                            segments: vec![segment.name],
                        });
                    }
                }
            }
        }

        let mut worktree_segments = Vec::<SegmentOutput>::new();
        let mut extra_rows = Vec::<FileRow>::new();
        if sources.include_staged {
            let seg = filter_segment_output(build_staged_segment(git_root, "")?, &filters);
            let mut seg = apply_tracked_binary_policy(
                git_root,
                seg,
                binary_policy,
                TrackedBinarySource::Index,
            )?;
            attachments.append(&mut seg.attachments);
            exclusions.append(&mut seg.exclusions);
            let seg = seg.segment;
            if !seg.rows.is_empty() || !seg.patch.trim().is_empty() {
                worktree_segments.push(seg);
            }
        }
        if sources.include_unstaged {
            let seg = filter_segment_output(build_unstaged_segment(git_root, "")?, &filters);
            let mut seg = apply_tracked_binary_policy(
                git_root,
                seg,
                binary_policy,
                TrackedBinarySource::Worktree,
            )?;
            attachments.append(&mut seg.attachments);
            exclusions.append(&mut seg.exclusions);
            let seg = seg.segment;
            if !seg.rows.is_empty() || !seg.patch.trim().is_empty() {
                worktree_segments.push(seg);
            }
        }
        if sources.include_untracked {
            let built = build_untracked_segment(git_root, untracked_mode, binary_policy, &filters)?;
            if let Some(seg) = built.segment {
                worktree_segments.push(seg);
            }
            for row in built.rows {
                if row.part == "attachments.zip" || row.part == "-" {
                    extra_rows.push(row);
                }
            }
            attachments.extend(built.attachments);
            exclusions.extend(built.exclusions);
        }

        let worktree_has_patch = worktree_segments.iter().any(|s| !s.patch.trim().is_empty());
        if !worktree_segments.is_empty() && worktree_has_patch {
            let part_name = format!("part_{:02}.patch", parts.len() + 1);
            for seg in &mut worktree_segments {
                for row in &mut seg.rows {
                    if row.part.is_empty() {
                        row.part = part_name.clone();
                    }
                }
            }
            for seg in &worktree_segments {
                rows.extend(seg.rows.clone());
            }
            let patch = render_segmented_patch(&worktree_segments);
            let mut segments = worktree_segments
                .iter()
                .map(|s| s.name.clone())
                .collect::<Vec<_>>();
            sort_segments(&mut segments);
            parts.push(PartOutput {
                name: part_name,
                patch,
                segments,
            });
        }
        for seg in &worktree_segments {
            rows.extend(seg.rows.clone());
        }

        rows.extend(extra_rows);
        sort_file_rows(&mut rows);

        if parts.is_empty() {
            parts.push(PartOutput {
                name: "part_01.patch".to_string(),
                patch: "# (no changes)\n".to_string(),
                segments: vec!["meta".to_string()],
            });
        }

        apply_packing_fallback(
            &mut parts,
            &mut rows,
            &mut commit_views,
            &mut exclusions,
            &packing_limits,
        )?;

        enforce_packing_limits(&parts, &packing_limits)?;

        let secret_hits = scan_bundle_for_secrets(&parts, &attachments)?;

        for part in &parts {
            write_text_file(&parts_dir.join(&part.name), &part.patch)?;
        }
        let file_semantics = build_file_semantics(git_root, &rows, &parts)?;
        let mut project_context =
            build_project_context_bundle(git_root, &rows, &file_semantics, project_context_mode)?;
        let task_groups = build_manifest_task_group_details(
            &parts,
            &rows,
            &file_semantics,
            project_context.as_ref().map(|bundle| &bundle.manifest),
        );
        if let Some(project_context) = project_context.as_mut() {
            enrich_project_context_with_task_groups(&mut project_context.manifest, &task_groups);
            project_context.markdown = render_project_context_md(&project_context.manifest);
        }
        if let Some(project_context) = project_context.as_ref() {
            write_text_file(
                &out_dir.join(project_context_json_path()),
                &render_project_context_manifest(project_context)?,
            )?;
            write_text_file(
                &out_dir.join(project_context_md_path()),
                &project_context.markdown,
            )?;
            for snapshot in &project_context.snapshots {
                let path = out_dir.join(&snapshot.path);
                if let Some(parent) = path.parent() {
                    fs::create_dir_all(parent).map_err(|e| {
                        ExitError::new(
                            EXIT_GENERAL,
                            format!(
                                "failed to create project context snapshot dir {}: {e}",
                                parent.display()
                            ),
                        )
                    })?;
                }
                fs::write(&path, &snapshot.bytes).map_err(|e| {
                    ExitError::new(
                        EXIT_GENERAL,
                        format!(
                            "failed to write project context snapshot {}: {e}",
                            path.display()
                        ),
                    )
                })?;
            }
        }
        let part_contexts = render_part_contexts(
            &parts,
            &rows,
            &attachments,
            &exclusions,
            &secret_hits,
            &file_semantics,
            &task_groups,
        )?;
        for (path, contents) in &part_contexts {
            write_text_file(&out_dir.join(path), contents)?;
        }
        if !attachments.is_empty() {
            write_attachments_zip(&out_dir.join("attachments.zip"), &attachments)?;
        }
        if !exclusions.is_empty() {
            write_text_file(
                &out_dir.join("excluded.md"),
                &render_excluded_md(&exclusions),
            )?;
        }
        if !secret_hits.is_empty() {
            write_text_file(
                &out_dir.join("secrets.md"),
                &render_secrets_md(&secret_hits),
            )?;
        }

        let changed_paths: Vec<String> = rows.iter().map(|r| r.path.clone()).collect();
        let changed_tree = render_changed_tree(&changed_paths);
        let parts_index = render_parts_index(&parts, &rows);
        let (cat_summary, reading_order) = render_category_summary_and_reading_order(&rows);
        let first_part_rel = parts
            .first()
            .map(|p| format!("parts/{}", p.name))
            .unwrap_or_else(|| "parts/part_01.patch".to_string());

        let handoff = render_handoff_md(&HandoffDocInputs {
            out_dir: &out_dir,
            plan: plan.as_ref(),
            head: &head,
            split_by,
            packing_limits: packing_limits.clone(),
            binary_policy,
            sources,
            untracked_mode,
            changed_tree: &changed_tree,
            rows: &rows,
            cat_summary: &cat_summary,
            parts_index: &parts_index,
            reading_order: &reading_order,
            first_part_rel: &first_part_rel,
            commit_views: &commit_views,
            attachments: &attachments,
            exclusions: &exclusions,
            ignore_enabled: filters.has_ignore_rules(),
            include_patterns: filters.includes(),
            exclude_patterns: filters.excludes(),
            secret_hits: &secret_hits,
            project_context: project_context.as_ref().map(|bundle| &bundle.manifest),
        });
        write_text_file(&out_dir.join("HANDOFF.md"), &handoff)?;
        let handoff_manifest = render_handoff_manifest(&HandoffManifestInputs {
            plan: plan.as_ref(),
            head: &head,
            split_by,
            packing_limits: &packing_limits,
            binary_policy,
            sources,
            untracked_mode,
            rows: &rows,
            parts: &parts,
            commit_views: &commit_views,
            attachments: &attachments,
            exclusions: &exclusions,
            ignore_enabled: filters.has_ignore_rules(),
            include_patterns: filters.includes(),
            exclude_patterns: filters.excludes(),
            secret_hits: &secret_hits,
            reading_order: &reading_order,
            file_semantics: &file_semantics,
            project_context: project_context.as_ref().map(|bundle| &bundle.manifest),
            task_groups: &task_groups,
        })?;
        let ai_requests = render_ai_requests_md(&HandoffManifestInputs {
            plan: plan.as_ref(),
            head: &head,
            split_by,
            packing_limits: &packing_limits,
            binary_policy,
            sources,
            untracked_mode,
            rows: &rows,
            parts: &parts,
            commit_views: &commit_views,
            attachments: &attachments,
            exclusions: &exclusions,
            ignore_enabled: filters.has_ignore_rules(),
            include_patterns: filters.includes(),
            exclude_patterns: filters.excludes(),
            secret_hits: &secret_hits,
            reading_order: &reading_order,
            file_semantics: &file_semantics,
            project_context: project_context.as_ref().map(|bundle| &bundle.manifest),
            task_groups: &task_groups,
        });
        write_text_file(&out_dir.join(ai_requests_md_path()), &ai_requests)?;
        write_text_file(&out_dir.join("handoff.manifest.json"), &handoff_manifest)?;
        let handoff_context_xml = render_handoff_context_xml(&HandoffManifestInputs {
            plan: plan.as_ref(),
            head: &head,
            split_by,
            packing_limits: &packing_limits,
            binary_policy,
            sources,
            untracked_mode,
            rows: &rows,
            parts: &parts,
            commit_views: &commit_views,
            attachments: &attachments,
            exclusions: &exclusions,
            ignore_enabled: filters.has_ignore_rules(),
            include_patterns: filters.includes(),
            exclude_patterns: filters.excludes(),
            secret_hits: &secret_hits,
            reading_order: &reading_order,
            file_semantics: &file_semantics,
            project_context: project_context.as_ref().map(|bundle| &bundle.manifest),
            task_groups: &task_groups,
        });
        write_text_file(
            &out_dir.join(handoff_context_xml_path()),
            &handoff_context_xml,
        )?;

        if let Some(plan_out) = args.plan_out.as_deref() {
            let plan_path = resolve_plan_path(&cwd, plan_out)?;
            resolved_plan
                .write_to_path(&plan_path)
                .map_err(|e| ExitError::new(EXIT_GENERAL, e))?;
            println!(
                "diffship build: wrote plan {} (profile={}, resolved limits)",
                plan_path.display(),
                resolved_plan.profile.as_deref().unwrap_or("none")
            );
        }

        handle_secret_hits(&secret_hits, args.yes, args.fail_on_secrets)?;

        if let Some(zp) = output_paths.final_zip.as_ref() {
            write_zip_from_dir(&out_dir, zp)?;
        }

        if let Some(final_dir) = output_paths.final_dir.as_ref() {
            println!("diffship build: created {}", final_dir.display());
            if !attachments.is_empty() {
                println!(
                    "diffship build: created {}/attachments.zip",
                    final_dir.display()
                );
            }
            if !exclusions.is_empty() {
                println!(
                    "diffship build: created {}/excluded.md",
                    final_dir.display()
                );
            }
            if !secret_hits.is_empty() {
                println!("diffship build: created {}/secrets.md", final_dir.display());
            }
        }
        if let Some(zp) = output_paths.final_zip.as_ref() {
            println!("diffship build: created {}", zp.display());
        }

        Ok(())
    })();

    if output_paths.cleanup_staging {
        let _ = fs::remove_dir_all(&output_paths.staging_dir);
    }

    result
}

fn resolve_build_args(
    git_root: &Path,
    cwd: &Path,
    args: BuildArgs,
) -> Result<BuildArgs, ExitError> {
    if args.out.is_some() && args.out_dir.is_some() {
        return Err(ExitError::new(
            EXIT_GENERAL,
            "--out and --out-dir cannot be used together",
        ));
    }
    let Some(plan_path) = args.plan.clone() else {
        let cfg = HandoffConfig::load(git_root)?;
        let (resolved, _) = cfg.resolve_build_args(args)?;
        return Ok(resolved);
    };
    if build_args_conflict_with_plan(&args) {
        return Err(ExitError::new(
            EXIT_GENERAL,
            "--plan cannot be combined with other explicit handoff selection flags; export a new plan or replay the plan as-is",
        ));
    }
    let path = resolve_plan_path(cwd, &plan_path)?;
    let plan = HandoffPlan::from_file(&path).map_err(|e| ExitError::new(EXIT_GENERAL, e))?;
    let mut effective = plan.into_build_args(args.plan, args.plan_out);
    effective.out_dir = args.out_dir;
    effective.out = args.out;
    effective.zip = args.zip;
    effective.zip_only = args.zip_only;
    effective.yes = args.yes;
    effective.fail_on_secrets = args.fail_on_secrets;
    let cfg = HandoffConfig::load(git_root)?;
    let (resolved, _) = cfg.resolve_build_args(effective)?;
    Ok(resolved)
}

fn build_args_conflict_with_plan(args: &BuildArgs) -> bool {
    args.profile.is_some()
        || args.range_mode != "last"
        || args.from.is_some()
        || args.to.is_some()
        || args.a.is_some()
        || args.b.is_some()
        || args.no_committed
        || !args.include.is_empty()
        || !args.exclude.is_empty()
        || args.include_staged
        || args.include_unstaged
        || args.include_untracked
        || args.split_by.as_deref() != Some("auto")
        || args.untracked_mode != "auto"
        || args.include_binary
        || args.binary_mode != "raw"
        || args.max_parts.is_some()
        || args.max_bytes_per_part.is_some()
        || args.project_context != "none"
}

fn resolve_plan_path(cwd: &Path, raw: &str) -> Result<PathBuf, ExitError> {
    resolve_user_path(cwd, raw)
}

#[derive(Debug, Clone)]
struct CommitSegmentBuild {
    rev: String,
    hash7: String,
    subject: String,
    date: String,
    segment: SegmentOutput,
}

fn build_committed_parts_by_commit(
    git_root: &Path,
    plan: &RangePlan,
) -> Result<Vec<CommitSegmentBuild>, ExitError> {
    let revs = git::run_git(
        git_root,
        [
            "rev-list",
            "--reverse",
            &format!("{}..{}", plan.base, plan.target),
        ],
    )?;
    let mut out = vec![];
    for rev in revs.lines().map(str::trim).filter(|s| !s.is_empty()) {
        let patch = git::run_git(
            git_root,
            [
                "show",
                "--format=",
                "--no-color",
                "--no-ext-diff",
                "--patch",
                "--full-index",
                rev,
            ],
        )?;
        let name_status = git::run_git(git_root, ["show", "--format=", "--name-status", rev])?;
        let numstat = git::run_git(git_root, ["show", "--format=", "--numstat", rev])?;
        let meta = git::run_git(git_root, ["show", "-s", "--format=%h%x09%s%x09%cs", rev])?;
        let mut meta_parts = meta.trim().splitn(3, '\t');
        let hash7 = meta_parts.next().unwrap_or("").to_string();
        let subject = meta_parts.next().unwrap_or("").to_string();
        let date = meta_parts.next().unwrap_or("").to_string();

        let insdel = parse_numstat(&numstat);
        let mut rows = parse_name_status("committed", &name_status, &insdel);
        for r in &mut rows {
            if r.status == "D" {
                r.bytes = Some(0);
            } else {
                r.bytes = git_cat_file_size(git_root, rev, &r.path).ok();
            }
        }

        out.push(CommitSegmentBuild {
            rev: rev.to_string(),
            hash7,
            subject,
            date,
            segment: SegmentOutput {
                name: "committed".to_string(),
                patch,
                rows,
            },
        });
    }
    Ok(out)
}

fn build_committed_segment(
    git_root: &Path,
    plan: &RangePlan,
    part_name: &str,
) -> Result<SegmentOutput, ExitError> {
    let patch = git::run_git(
        git_root,
        [
            "diff",
            "--no-color",
            "--no-ext-diff",
            "--patch",
            "--full-index",
            plan.base.as_str(),
            plan.target.as_str(),
        ],
    )?;

    let name_status = git::run_git(
        git_root,
        [
            "diff",
            "--name-status",
            plan.base.as_str(),
            plan.target.as_str(),
        ],
    )?;
    let numstat = git::run_git(
        git_root,
        [
            "diff",
            "--numstat",
            plan.base.as_str(),
            plan.target.as_str(),
        ],
    )?;

    let insdel_map = parse_numstat(&numstat);
    let mut rows = parse_name_status("committed", &name_status, &insdel_map);
    for r in &mut rows {
        if r.status == "D" {
            r.bytes = Some(0);
        } else {
            r.bytes = git_cat_file_size(git_root, &plan.target, &r.path).ok();
        }
        r.part = part_name.to_string();
    }
    Ok(SegmentOutput {
        name: "committed".to_string(),
        patch,
        rows,
    })
}

fn build_staged_segment(git_root: &Path, part_name: &str) -> Result<SegmentOutput, ExitError> {
    let patch = git::run_git(
        git_root,
        [
            "diff",
            "--cached",
            "--no-color",
            "--no-ext-diff",
            "--patch",
            "--full-index",
            "HEAD",
        ],
    )?;
    let name_status = git::run_git(git_root, ["diff", "--cached", "--name-status", "HEAD"])?;
    let numstat = git::run_git(git_root, ["diff", "--cached", "--numstat", "HEAD"])?;

    let insdel_map = parse_numstat(&numstat);
    let mut rows = parse_name_status("staged", &name_status, &insdel_map);
    for r in &mut rows {
        if r.status == "D" {
            r.bytes = Some(0);
        } else {
            r.bytes = git_cat_file_size(git_root, ":", &r.path).ok();
        }
        r.part = part_name.to_string();
    }
    Ok(SegmentOutput {
        name: "staged".to_string(),
        patch,
        rows,
    })
}

fn build_unstaged_segment(git_root: &Path, part_name: &str) -> Result<SegmentOutput, ExitError> {
    let patch = git::run_git(
        git_root,
        [
            "diff",
            "--no-color",
            "--no-ext-diff",
            "--patch",
            "--full-index",
            "HEAD",
        ],
    )?;
    let name_status = git::run_git(git_root, ["diff", "--name-status", "HEAD"])?;
    let numstat = git::run_git(git_root, ["diff", "--numstat", "HEAD"])?;

    let insdel_map = parse_numstat(&numstat);
    let mut rows = parse_name_status("unstaged", &name_status, &insdel_map);
    for r in &mut rows {
        if r.status == "D" {
            r.bytes = Some(0);
        } else {
            r.bytes = file_size_on_disk(git_root, &r.path);
        }
        r.part = part_name.to_string();
    }
    Ok(SegmentOutput {
        name: "unstaged".to_string(),
        patch,
        rows,
    })
}

fn build_untracked_segment(
    git_root: &Path,
    mode: UntrackedMode,
    binary_policy: BinaryPolicy,
    filters: &PathFilter,
) -> Result<SegmentBuildResult, ExitError> {
    let list = git::run_git(git_root, ["ls-files", "--others", "--exclude-standard"])?;
    let mut paths = list
        .lines()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    paths.sort();

    let mut patch = String::new();
    let mut rows = vec![];
    let mut attachments = vec![];
    let mut exclusions = vec![];

    for rel in paths {
        if !filters.allows(&rel) {
            continue;
        }
        let abs = git_root.join(&rel);
        let bytes = fs::read(&abs).map_err(|e| {
            ExitError::new(
                EXIT_GENERAL,
                format!("failed to read untracked file {}: {e}", abs.display()),
            )
        })?;
        let size = bytes.len() as u64;
        let decoded = String::from_utf8(bytes.clone()).ok();
        let text_patch_ok = decoded
            .as_ref()
            .is_some_and(|_| bytes.len() <= AUTO_PATCH_MAX_BYTES);
        let is_binary = decoded.is_none();

        let policy = match mode {
            UntrackedMode::Patch => UntrackedDisposition::Patch,
            UntrackedMode::Raw => UntrackedDisposition::Raw,
            UntrackedMode::Meta => UntrackedDisposition::Meta,
            UntrackedMode::Auto => {
                if is_binary {
                    if !binary_policy.include_binary {
                        UntrackedDisposition::Meta
                    } else {
                        match binary_policy.binary_mode {
                            BinaryMode::Raw => UntrackedDisposition::Raw,
                            BinaryMode::Patch => UntrackedDisposition::Patch,
                            BinaryMode::Meta => UntrackedDisposition::Meta,
                        }
                    }
                } else if text_patch_ok {
                    UntrackedDisposition::Patch
                } else {
                    UntrackedDisposition::Raw
                }
            }
        };

        match policy {
            UntrackedDisposition::Patch => {
                if decoded.is_none()
                    && (!binary_policy.include_binary
                        || binary_policy.binary_mode != BinaryMode::Patch)
                {
                    return Err(ExitError::new(
                        EXIT_GENERAL,
                        format!(
                            "untracked binary file '{}' requires --include-binary --binary-mode patch/raw/meta",
                            rel
                        ),
                    ));
                }
                if decoded.is_some() && bytes.len() > AUTO_PATCH_MAX_BYTES {
                    return Err(ExitError::new(
                        EXIT_GENERAL,
                        format!(
                            "untracked file '{}' is too large for patch mode; use --untracked-mode raw|meta",
                            rel
                        ),
                    ));
                }
                let diff = run_git_allow_diff_status(
                    git_root,
                    [
                        "diff",
                        "--no-index",
                        "--no-color",
                        "--no-ext-diff",
                        "--patch",
                        "--full-index",
                        "--binary",
                        "--",
                        "/dev/null",
                        rel.as_str(),
                    ],
                )?;
                if !patch.is_empty() && !patch.ends_with('\n') {
                    patch.push('\n');
                }
                patch.push_str(diff.trim_end());
                patch.push('\n');
                rows.push(FileRow {
                    segment: "untracked".to_string(),
                    status: "A".to_string(),
                    path: rel,
                    note: String::new(),
                    ins: decoded.as_ref().map(|s| count_text_lines(s)),
                    del: if decoded.is_some() { Some(0) } else { None },
                    bytes: Some(size),
                    part: String::new(),
                });
            }
            UntrackedDisposition::Raw => {
                let reason = if decoded.is_some() {
                    "stored as raw attachment (auto/raw mode)"
                } else {
                    "binary/unreadable stored as raw attachment"
                };
                attachments.push(AttachmentEntry {
                    zip_path: format!("untracked/{}", rel),
                    bytes,
                    reason: reason.to_string(),
                });
                rows.push(FileRow {
                    segment: "untracked".to_string(),
                    status: "A".to_string(),
                    path: rel,
                    note: "stored in attachments.zip".to_string(),
                    ins: decoded.as_ref().map(|s| count_text_lines(s)),
                    del: Some(0),
                    bytes: Some(size),
                    part: "attachments.zip".to_string(),
                });
            }
            UntrackedDisposition::Meta => {
                let reason = if is_binary && !binary_policy.include_binary {
                    "binary file excluded by default".to_string()
                } else if is_binary {
                    "binary file excluded by binary-mode=meta".to_string()
                } else {
                    "meta-only untracked file".to_string()
                };
                exclusions.push(ExclusionEntry {
                    path: rel.clone(),
                    reason,
                    guidance:
                        "Re-run with --untracked-mode raw/patch or --include-binary --binary-mode raw/patch when contents are needed"
                            .to_string(),
                });
                rows.push(FileRow {
                    segment: "untracked".to_string(),
                    status: "A".to_string(),
                    path: rel,
                    note: "excluded (meta only; see excluded.md)".to_string(),
                    ins: None,
                    del: None,
                    bytes: Some(size),
                    part: "-".to_string(),
                });
            }
        }
    }

    sort_file_rows(&mut rows);
    let patch_rows = rows
        .iter()
        .filter(|r| r.part.is_empty())
        .cloned()
        .collect::<Vec<_>>();
    let segment = if patch.trim().is_empty() {
        None
    } else {
        Some(SegmentOutput {
            name: "untracked".to_string(),
            patch,
            rows: patch_rows,
        })
    };

    Ok(SegmentBuildResult {
        segment,
        rows,
        attachments,
        exclusions,
    })
}

#[derive(Debug, Clone, Copy)]
enum UntrackedDisposition {
    Patch,
    Raw,
    Meta,
}

fn render_segmented_patch(segments: &[SegmentOutput]) -> String {
    let mut out = String::new();
    for seg in segments {
        if seg.patch.trim().is_empty() {
            continue;
        }
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(&format!("# === diffship segment: {} ===\n", seg.name));
        out.push_str(seg.patch.trim_end());
        out.push('\n');
    }
    if out.is_empty() {
        out.push_str("# (no changes)\n");
    }
    out
}

fn default_output_dir(cwd: &Path, head: &str) -> Result<PathBuf, ExitError> {
    let timestamp = timestamp_yyyymmdd_hhmm()?;
    Ok(default_output_dir_for_timestamp(
        cwd,
        &timestamp,
        &crate::git::short_sha_label(head),
    ))
}

fn default_output_dir_for_timestamp(cwd: &Path, timestamp: &str, head_label: &str) -> PathBuf {
    let base = cwd.join(format!("diffship_{timestamp}_{head_label}"));
    if !base.exists() {
        return base;
    }

    for suffix in 2.. {
        let candidate = cwd.join(format!("diffship_{timestamp}_{head_label}_{suffix}"));
        if !candidate.exists() {
            return candidate;
        }
    }

    unreachable!("numeric suffix search is unbounded");
}

fn default_output_zip_path(cwd: &Path, head: &str) -> Result<PathBuf, ExitError> {
    let timestamp = timestamp_yyyymmdd_hhmm()?;
    let head_label = crate::git::short_sha_label(head);
    for suffix in 1.. {
        let stem = if suffix == 1 {
            cwd.join(format!("diffship_{timestamp}_{head_label}"))
        } else {
            cwd.join(format!("diffship_{timestamp}_{head_label}_{suffix}"))
        };
        let zip = stem.with_extension("zip");
        if !stem.exists() && !zip.exists() {
            return Ok(zip);
        }
    }

    unreachable!("numeric suffix search is unbounded");
}

fn resolve_output_paths(
    cwd: &Path,
    args: &BuildArgs,
    head: &str,
) -> Result<BuildOutputPaths, ExitError> {
    let output_parent = args
        .out_dir
        .as_deref()
        .map_or_else(|| Ok(cwd.to_path_buf()), |raw| resolve_user_path(cwd, raw))?;
    if args.zip_only {
        let final_zip = match args.out.as_deref() {
            Some(raw) => {
                let path = resolve_user_path(cwd, raw)?;
                if path.extension().and_then(|ext| ext.to_str()) != Some("zip") {
                    return Err(ExitError::new(
                        EXIT_GENERAL,
                        "--zip-only requires --out to point to a .zip path",
                    ));
                }
                path
            }
            None => default_output_zip_path(&output_parent, head)?,
        };
        if final_zip.exists() {
            return Err(ExitError::new(
                EXIT_GENERAL,
                format!("output path already exists: {}", final_zip.display()),
            ));
        }
        let staging_parent = final_zip.parent().unwrap_or(cwd);
        let staging_dir =
            staging_parent.join(format!(".diffship_build_{}", uuid::Uuid::new_v4().simple()));
        if staging_dir.exists() {
            return Err(ExitError::new(
                EXIT_GENERAL,
                format!(
                    "temporary output path already exists: {}",
                    staging_dir.display()
                ),
            ));
        }
        return Ok(BuildOutputPaths {
            staging_dir,
            final_dir: None,
            final_zip: Some(final_zip),
            cleanup_staging: true,
        });
    }

    let final_dir = match args.out.as_deref() {
        Some(raw) => resolve_user_path(cwd, raw)?,
        None => default_output_dir(&output_parent, head)?,
    };
    if final_dir.exists() {
        return Err(ExitError::new(
            EXIT_GENERAL,
            format!("output path already exists: {}", final_dir.display()),
        ));
    }
    let final_zip = if args.zip {
        let zip_path = final_dir.with_extension("zip");
        if zip_path.exists() {
            return Err(ExitError::new(
                EXIT_GENERAL,
                format!("output path already exists: {}", zip_path.display()),
            ));
        }
        Some(zip_path)
    } else {
        None
    };
    Ok(BuildOutputPaths {
        staging_dir: final_dir.clone(),
        final_dir: Some(final_dir),
        final_zip,
        cleanup_staging: false,
    })
}

fn timestamp_yyyymmdd_hhmm() -> Result<String, ExitError> {
    let now = current_local_time().unwrap_or_else(|_| time::OffsetDateTime::now_utc());
    format_output_timestamp(now)
}

fn current_local_time() -> Result<time::OffsetDateTime, ExitError> {
    let offset = time::UtcOffset::current_local_offset().map_err(|e| {
        ExitError::new(
            EXIT_GENERAL,
            format!("failed to detect local time offset: {e}"),
        )
    })?;
    Ok(time::OffsetDateTime::now_utc().to_offset(offset))
}

fn format_output_timestamp(now: time::OffsetDateTime) -> Result<String, ExitError> {
    let fmt = format_description::parse("[year]-[month]-[day]_[hour][minute]")
        .map_err(|e| ExitError::new(EXIT_GENERAL, format!("invalid time format: {e}")))?;
    now.format(&fmt)
        .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to format time: {e}")))
}

fn parse_range_mode(s: &str) -> Result<RangeMode, ExitError> {
    match s.trim().to_ascii_lowercase().as_str() {
        "direct" => Ok(RangeMode::Direct),
        "merge-base" | "mergeb" | "mergebase" => Ok(RangeMode::MergeBase),
        "last" => Ok(RangeMode::Last),
        "root" => Ok(RangeMode::Root),
        other => Err(ExitError::new(
            EXIT_GENERAL,
            format!("invalid --range-mode '{other}' (expected: direct|merge-base|last|root)"),
        )),
    }
}

fn effective_split_by(raw: Option<&str>, plan: Option<&RangePlan>) -> Result<SplitBy, ExitError> {
    let requested = match raw.unwrap_or("auto").trim().to_ascii_lowercase().as_str() {
        "auto" => {
            return Ok(if plan.and_then(|p| p.commit_count).unwrap_or(0) > 1 {
                SplitBy::Commit
            } else {
                SplitBy::File
            });
        }
        "file" => SplitBy::File,
        "commit" => SplitBy::Commit,
        other => {
            return Err(ExitError::new(
                EXIT_GENERAL,
                format!("invalid --split-by '{other}' (expected: auto|file|commit)"),
            ));
        }
    };
    Ok(requested)
}

fn parse_untracked_mode(raw: &str) -> Result<UntrackedMode, ExitError> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "auto" => Ok(UntrackedMode::Auto),
        "patch" => Ok(UntrackedMode::Patch),
        "raw" => Ok(UntrackedMode::Raw),
        "meta" => Ok(UntrackedMode::Meta),
        other => Err(ExitError::new(
            EXIT_GENERAL,
            format!("invalid --untracked-mode '{other}' (expected: auto|patch|raw|meta)"),
        )),
    }
}

fn parse_binary_mode(raw: &str) -> Result<BinaryMode, ExitError> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "raw" => Ok(BinaryMode::Raw),
        "patch" => Ok(BinaryMode::Patch),
        "meta" => Ok(BinaryMode::Meta),
        other => Err(ExitError::new(
            EXIT_GENERAL,
            format!("invalid --binary-mode '{other}' (expected: raw|patch|meta)"),
        )),
    }
}

fn parse_project_context_mode(raw: &str) -> Result<ProjectContextMode, ExitError> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "none" => Ok(ProjectContextMode::None),
        "focused" => Ok(ProjectContextMode::Focused),
        other => Err(ExitError::new(
            EXIT_GENERAL,
            format!("invalid --project-context '{other}' (expected: none|focused)"),
        )),
    }
}

#[derive(Debug, Clone)]
struct PackUnit {
    origin_part: String,
    path: String,
    segment: String,
    chunk: String,
    bytes: u64,
    context_level: usize,
}

fn apply_packing_fallback(
    parts: &mut Vec<PartOutput>,
    rows: &mut [FileRow],
    commit_views: &mut [CommitView],
    exclusions: &mut Vec<ExclusionEntry>,
    limits: &PackingLimits,
) -> Result<(), ExitError> {
    if !parts_exceed_limits(parts, limits) {
        return Ok(());
    }

    let units = collect_pack_units(parts);
    if units.is_empty() {
        return Ok(());
    }

    let mut sorted_units = units;
    sorted_units.sort_by(|a, b| {
        b.bytes
            .cmp(&a.bytes)
            .then(a.path.cmp(&b.path))
            .then(a.origin_part.cmp(&b.origin_part))
    });

    let mut bins: Vec<Vec<PackUnit>> = vec![];
    let mut bin_bytes: Vec<u64> = vec![];
    let mut dropped: Vec<(PackUnit, String, String)> = vec![];

    for unit in sorted_units {
        let unit = reduce_pack_unit_to_capacity(unit, limits.max_bytes_per_part);
        let mut cost = pack_unit_cost(&unit);
        if cost > limits.max_bytes_per_part {
            dropped.push((
                unit,
                format!(
                    "diff unit exceeds max_bytes_per_part={} even after context reduction (U3→U1→U0)",
                    limits.max_bytes_per_part
                ),
                "Increase --max-bytes-per-part or narrow selected sources/range; context reduction was already attempted".to_string(),
            ));
            continue;
        }

        let mut placed = false;
        for (idx, used) in bin_bytes.iter_mut().enumerate() {
            if *used + cost <= limits.max_bytes_per_part {
                *used += cost;
                bins[idx].push(unit.clone());
                placed = true;
                break;
            }
        }

        if placed {
            continue;
        }

        if bins.len() < limits.max_parts {
            bin_bytes.push(cost);
            bins.push(vec![unit]);
        } else {
            let max_remaining = bin_bytes
                .iter()
                .map(|used| limits.max_bytes_per_part.saturating_sub(*used))
                .max()
                .unwrap_or(0);
            let reduced = reduce_pack_unit_to_capacity(unit.clone(), max_remaining);
            cost = pack_unit_cost(&reduced);
            if cost <= max_remaining {
                for (idx, used) in bin_bytes.iter_mut().enumerate() {
                    if *used + cost <= limits.max_bytes_per_part {
                        *used += cost;
                        bins[idx].push(reduced.clone());
                        placed = true;
                        break;
                    }
                }
            }
            if placed {
                continue;
            }
            dropped.push((
                reduced,
                format!(
                    "max_parts={} reached during fallback packing even after context reduction",
                    limits.max_parts
                ),
                "Increase --max-parts or narrow selected sources/range; context reduction was already attempted".to_string(),
            ));
        }
    }

    let total_units = bins.iter().map(Vec::len).sum::<usize>() + dropped.len();
    if total_units > 0 && dropped.len() == total_units {
        return Err(ExitError::new(
            EXIT_PACKING_LIMITS,
            format!(
                "packing limits exceeded: all diff units were excluded by fallback (max_parts={}, max_bytes_per_part={})",
                limits.max_parts, limits.max_bytes_per_part
            ),
        ));
    }

    let mut rebuilt = Vec::new();
    let mut part_remap: BTreeMap<(String, String), String> = BTreeMap::new();
    let mut reduced_map: BTreeMap<(String, String), usize> = BTreeMap::new();
    for (idx, bin) in bins.into_iter().enumerate() {
        if bin.is_empty() {
            continue;
        }

        let part_name = format!("part_{:02}.patch", idx + 1);
        let mut units = bin;
        units.sort_by(|a, b| {
            segment_rank(&a.segment)
                .cmp(&segment_rank(&b.segment))
                .then(a.path.cmp(&b.path))
                .then(a.origin_part.cmp(&b.origin_part))
        });

        let mut patch = String::new();
        let mut segs = Vec::<String>::new();
        let mut current_seg: Option<String> = None;
        for u in units {
            if current_seg.as_deref() != Some(u.segment.as_str()) {
                if !patch.is_empty() && !patch.ends_with('\n') {
                    patch.push('\n');
                }
                patch.push_str(&format!("# === diffship segment: {} ===\n", u.segment));
                current_seg = Some(u.segment.clone());
                segs.push(u.segment.clone());
            }
            patch.push_str(u.chunk.trim_end());
            patch.push('\n');
            if u.context_level < 3 {
                reduced_map.insert((u.origin_part.clone(), u.path.clone()), u.context_level);
            }
            part_remap.insert((u.origin_part, u.path), part_name.clone());
        }

        segs.sort_by(|a, b| segment_rank(a).cmp(&segment_rank(b)).then(a.cmp(b)));
        segs.dedup();

        rebuilt.push(PartOutput {
            name: part_name,
            patch,
            segments: segs,
        });
    }

    let mut drop_map: BTreeMap<(String, String), (String, String)> = BTreeMap::new();
    for (u, reason, guidance) in dropped {
        drop_map.insert((u.origin_part, u.path), (reason, guidance));
    }

    let mut seen_exclusions = BTreeSet::new();
    for row in rows.iter_mut() {
        if !row.part.starts_with("part_") {
            continue;
        }
        let old_part = row.part.clone();
        let key = (old_part, row.path.clone());
        if let Some(new_part) = part_remap.get(&key) {
            row.part = new_part.clone();
            if let Some(context_level) = reduced_map.get(&key) {
                append_row_note(
                    row,
                    &format!(
                        "packing fallback reduced diff context to U{}",
                        context_level
                    ),
                );
            }
            continue;
        }
        if let Some((reason, guidance)) = drop_map.get(&key) {
            row.part = "-".to_string();
            if row.note.is_empty() {
                row.note = "excluded by packing fallback (see excluded.md)".to_string();
            }
            let uniq = format!("{}::{}", row.path, reason);
            if seen_exclusions.insert(uniq) {
                exclusions.push(ExclusionEntry {
                    path: row.path.clone(),
                    reason: reason.clone(),
                    guidance: guidance.clone(),
                });
            }
        }
    }

    for cv in commit_views.iter_mut() {
        for (path, part) in &mut cv.files {
            let key = (part.clone(), path.clone());
            if let Some(new_part) = part_remap.get(&key) {
                *part = new_part.clone();
            } else if drop_map.contains_key(&key) {
                *part = "-".to_string();
            }
        }
        cv.files.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    }

    if rebuilt.is_empty() {
        rebuilt.push(PartOutput {
            name: "part_01.patch".to_string(),
            patch: "# (no changes)\n".to_string(),
            segments: vec!["meta".to_string()],
        });
    }
    *parts = rebuilt;

    Ok(())
}

fn parts_exceed_limits(parts: &[PartOutput], limits: &PackingLimits) -> bool {
    if parts.len() > limits.max_parts {
        return true;
    }
    parts
        .iter()
        .any(|p| p.patch.len() as u64 > limits.max_bytes_per_part)
}

fn collect_pack_units(parts: &[PartOutput]) -> Vec<PackUnit> {
    let mut out = Vec::new();

    for part in parts {
        let mut current_seg = "meta".to_string();
        let mut current_chunk = String::new();

        for line in part.patch.lines() {
            if let Some(seg) = line
                .strip_prefix("# === diffship segment: ")
                .and_then(|s| s.strip_suffix(" ==="))
            {
                if !current_chunk.is_empty() {
                    push_pack_unit(&mut out, part, &current_seg, &current_chunk);
                    current_chunk.clear();
                }
                current_seg = seg.to_string();
                continue;
            }

            if line.starts_with("diff --git ") {
                if !current_chunk.is_empty() {
                    push_pack_unit(&mut out, part, &current_seg, &current_chunk);
                    current_chunk.clear();
                }
                current_chunk.push_str(line);
                current_chunk.push('\n');
                continue;
            }

            if !current_chunk.is_empty() {
                current_chunk.push_str(line);
                current_chunk.push('\n');
            }
        }

        if !current_chunk.is_empty() {
            push_pack_unit(&mut out, part, &current_seg, &current_chunk);
        }
    }

    out
}

fn push_pack_unit(out: &mut Vec<PackUnit>, part: &PartOutput, segment: &str, chunk: &str) {
    if let Some(path) = patch_chunk_path(chunk) {
        out.push(PackUnit {
            origin_part: part.name.clone(),
            path,
            segment: segment.to_string(),
            chunk: chunk.to_string(),
            bytes: chunk.len() as u64,
            context_level: 3,
        });
    }
}

fn append_row_note(row: &mut FileRow, note: &str) {
    if row.note.is_empty() {
        row.note = note.to_string();
    } else if !row.note.contains(note) {
        row.note.push_str("; ");
        row.note.push_str(note);
    }
}

fn pack_unit_cost(unit: &PackUnit) -> u64 {
    // Safety margin for segment headers and separators added during rebuild.
    unit.bytes + 48
}

fn reduce_pack_unit_to_capacity(unit: PackUnit, max_capacity: u64) -> PackUnit {
    if max_capacity == 0 || pack_unit_cost(&unit) <= max_capacity {
        return unit;
    }

    let mut current = unit;
    for context_level in [1usize, 0usize] {
        if current.context_level <= context_level {
            continue;
        }
        let Some(chunk) = reduce_patch_chunk_context(&current.chunk, context_level) else {
            continue;
        };
        if chunk.len() >= current.chunk.len() {
            continue;
        }
        current.chunk = chunk;
        current.bytes = current.chunk.len() as u64;
        current.context_level = context_level;
        if pack_unit_cost(&current) <= max_capacity {
            break;
        }
    }
    current
}

#[derive(Debug, Clone)]
struct UnifiedHunkHeader {
    old_start: i64,
    old_count: i64,
    new_start: i64,
    new_count: i64,
    suffix: String,
}

#[derive(Debug, Clone)]
struct HunkItem {
    line: String,
    old_delta: i64,
    new_delta: i64,
    trailing: Vec<String>,
}

fn reduce_patch_chunk_context(chunk: &str, max_context: usize) -> Option<String> {
    if chunk.contains("GIT binary patch") {
        return None;
    }

    let lines = chunk.lines().collect::<Vec<_>>();
    let first_hunk = lines.iter().position(|line| line.starts_with("@@ "))?;

    let mut out = Vec::<String>::new();
    out.extend(lines[..first_hunk].iter().map(|line| (*line).to_string()));

    let mut i = first_hunk;
    while i < lines.len() {
        let header = parse_unified_hunk_header(lines[i])?;
        i += 1;

        let start = i;
        while i < lines.len() && !lines[i].starts_with("@@ ") {
            i += 1;
        }
        let body = lines[start..i]
            .iter()
            .map(|line| (*line).to_string())
            .collect::<Vec<_>>();
        let reduced = reduce_hunk_context(&header, &body, max_context)?;
        for (header, body) in reduced {
            out.push(format_unified_hunk_header(&header));
            out.extend(body);
        }
    }

    if out.is_empty() {
        return None;
    }

    let mut rendered = out.join("\n");
    rendered.push('\n');
    Some(rendered)
}

fn reduce_hunk_context(
    header: &UnifiedHunkHeader,
    body: &[String],
    max_context: usize,
) -> Option<Vec<(UnifiedHunkHeader, Vec<String>)>> {
    let mut items = Vec::<HunkItem>::new();
    for line in body {
        if line.starts_with('\\') {
            if let Some(last) = items.last_mut() {
                last.trailing.push(line.clone());
            }
            continue;
        }
        let (old_delta, new_delta) = hunk_line_deltas(line);
        items.push(HunkItem {
            line: line.clone(),
            old_delta,
            new_delta,
            trailing: vec![],
        });
    }

    let change_indices = items
        .iter()
        .enumerate()
        .filter_map(|(idx, item)| {
            if item.old_delta == 0 || item.new_delta == 0 {
                Some(idx)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    if change_indices.is_empty() {
        return None;
    }

    let mut old_positions = vec![0_i64; items.len()];
    let mut new_positions = vec![0_i64; items.len()];
    let mut old_line = header.old_start;
    let mut new_line = header.new_start;
    for (idx, item) in items.iter().enumerate() {
        old_positions[idx] = old_line;
        new_positions[idx] = new_line;
        old_line += item.old_delta;
        new_line += item.new_delta;
    }

    let mut ranges = change_indices
        .into_iter()
        .map(|idx| {
            (
                expand_hunk_start(&items, idx, max_context),
                expand_hunk_end(&items, idx, max_context),
            )
        })
        .collect::<Vec<_>>();
    ranges.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));

    let mut merged = Vec::<(usize, usize)>::new();
    for range in ranges {
        if let Some(last) = merged.last_mut()
            && range.0 <= last.1 + 1
        {
            last.1 = last.1.max(range.1);
            continue;
        }
        merged.push(range);
    }

    let mut out = Vec::<(UnifiedHunkHeader, Vec<String>)>::new();
    for (start, end) in merged {
        let old_start = old_positions[start];
        let new_start = new_positions[start];
        let old_count = items[start..=end]
            .iter()
            .map(|item| item.old_delta)
            .sum::<i64>();
        let new_count = items[start..=end]
            .iter()
            .map(|item| item.new_delta)
            .sum::<i64>();
        let mut reduced_body = Vec::<String>::new();
        for item in &items[start..=end] {
            reduced_body.push(item.line.clone());
            reduced_body.extend(item.trailing.clone());
        }
        out.push((
            UnifiedHunkHeader {
                old_start,
                old_count,
                new_start,
                new_count,
                suffix: header.suffix.clone(),
            },
            reduced_body,
        ));
    }
    Some(out)
}

fn expand_hunk_start(items: &[HunkItem], idx: usize, max_context: usize) -> usize {
    let mut start = idx;
    let mut seen_context = 0usize;
    while start > 0 {
        let prev = start - 1;
        if items[prev].old_delta == 1 && items[prev].new_delta == 1 {
            if seen_context == max_context {
                break;
            }
            seen_context += 1;
        }
        start = prev;
    }
    start
}

fn expand_hunk_end(items: &[HunkItem], idx: usize, max_context: usize) -> usize {
    let mut end = idx;
    let mut seen_context = 0usize;
    while end + 1 < items.len() {
        let next = end + 1;
        if items[next].old_delta == 1 && items[next].new_delta == 1 {
            if seen_context == max_context {
                break;
            }
            seen_context += 1;
        }
        end = next;
    }
    end
}

fn parse_unified_hunk_header(line: &str) -> Option<UnifiedHunkHeader> {
    let rest = line.strip_prefix("@@ -")?;
    let (old_raw, rest) = rest.split_once(" +")?;
    let (new_raw, suffix) = rest.split_once(" @@")?;
    let (old_start, old_count) = parse_hunk_range(old_raw)?;
    let (new_start, new_count) = parse_hunk_range(new_raw)?;
    Some(UnifiedHunkHeader {
        old_start,
        old_count,
        new_start,
        new_count,
        suffix: suffix.to_string(),
    })
}

fn parse_hunk_range(raw: &str) -> Option<(i64, i64)> {
    let (start, count) = match raw.split_once(',') {
        Some((start, count)) => (start, count),
        None => (raw, "1"),
    };
    Some((start.parse().ok()?, count.parse().ok()?))
}

fn format_unified_hunk_header(header: &UnifiedHunkHeader) -> String {
    format!(
        "@@ -{} +{} @@{}",
        format_hunk_range(header.old_start, header.old_count),
        format_hunk_range(header.new_start, header.new_count),
        header.suffix
    )
}

fn format_hunk_range(start: i64, count: i64) -> String {
    if count == 1 {
        start.to_string()
    } else {
        format!("{start},{count}")
    }
}

fn hunk_line_deltas(line: &str) -> (i64, i64) {
    match line.as_bytes().first().copied() {
        Some(b' ') => (1, 1),
        Some(b'-') => (1, 0),
        Some(b'+') => (0, 1),
        _ => (0, 0),
    }
}

fn enforce_packing_limits(parts: &[PartOutput], limits: &PackingLimits) -> Result<(), ExitError> {
    if parts.len() > limits.max_parts {
        return Err(ExitError::new(
            EXIT_PACKING_LIMITS,
            format!(
                "packing limits exceeded: parts={} > max_parts={}",
                parts.len(),
                limits.max_parts
            ),
        ));
    }

    for part in parts {
        let bytes = part.patch.len() as u64;
        if bytes > limits.max_bytes_per_part {
            return Err(ExitError::new(
                EXIT_PACKING_LIMITS,
                format!(
                    "packing limits exceeded: {} bytes={} > max_bytes_per_part={}",
                    part.name, bytes, limits.max_bytes_per_part
                ),
            ));
        }
    }

    Ok(())
}

#[derive(Debug, Clone, Copy)]
enum TrackedBinarySource<'a> {
    Commit(&'a str),
    Index,
    Worktree,
}

#[derive(Debug)]
struct BinaryPolicyResult {
    segment: SegmentOutput,
    attachments: Vec<AttachmentEntry>,
    exclusions: Vec<ExclusionEntry>,
}

fn apply_tracked_binary_policy(
    git_root: &Path,
    segment: SegmentOutput,
    policy: BinaryPolicy,
    source: TrackedBinarySource<'_>,
) -> Result<BinaryPolicyResult, ExitError> {
    if segment.rows.is_empty() {
        return Ok(BinaryPolicyResult {
            segment,
            attachments: vec![],
            exclusions: vec![],
        });
    }

    let mut keep_paths = BTreeSet::new();
    let mut rows = Vec::new();
    let mut attachments = Vec::new();
    let mut exclusions = Vec::new();

    for mut row in segment.rows {
        let is_binary = row.ins.is_none() && row.del.is_none();
        if !is_binary || (policy.include_binary && policy.binary_mode == BinaryMode::Patch) {
            keep_paths.insert(row.path.clone());
            rows.push(row);
            continue;
        }

        let (reason, guidance, note) = if !policy.include_binary {
            (
                "binary file excluded by default",
                "Re-run with --include-binary and choose --binary-mode raw|patch|meta",
                "excluded binary (default policy; see excluded.md)",
            )
        } else if policy.binary_mode == BinaryMode::Meta {
            (
                "binary file excluded by binary-mode=meta",
                "Re-run with --binary-mode raw or patch if the AI needs binary contents",
                "excluded binary (binary-mode=meta; see excluded.md)",
            )
        } else {
            ("", "", "")
        };

        if !reason.is_empty() {
            row.note = note.to_string();
            row.part = "-".to_string();
            exclusions.push(ExclusionEntry {
                path: row.path.clone(),
                reason: reason.to_string(),
                guidance: guidance.to_string(),
            });
            rows.push(row);
            continue;
        }

        // binary-mode=raw
        if row.status == "D" {
            row.note = "excluded binary deletion (see excluded.md)".to_string();
            row.part = "-".to_string();
            exclusions.push(ExclusionEntry {
                path: row.path.clone(),
                reason: "binary deletion cannot be attached as raw snapshot".to_string(),
                guidance: "Use --binary-mode patch if deletion hunks are needed".to_string(),
            });
            rows.push(row);
            continue;
        }

        let bytes = read_tracked_binary_bytes(git_root, source, &row.path)?;
        if let Some(bytes) = bytes {
            attachments.push(AttachmentEntry {
                zip_path: format!("binary/{}", row.path),
                bytes,
                reason: "stored as raw attachment (binary-mode=raw)".to_string(),
            });
            row.note = "stored in attachments.zip".to_string();
            row.part = "attachments.zip".to_string();
            rows.push(row);
        } else {
            row.note = "excluded binary (snapshot unavailable; see excluded.md)".to_string();
            row.part = "-".to_string();
            exclusions.push(ExclusionEntry {
                path: row.path.clone(),
                reason: "binary snapshot unavailable".to_string(),
                guidance: "Use --binary-mode patch if snapshot extraction fails".to_string(),
            });
            rows.push(row);
        }
    }

    sort_file_rows(&mut rows);
    let patch = filter_patch_by_paths(&segment.patch, &keep_paths);
    Ok(BinaryPolicyResult {
        segment: SegmentOutput {
            name: segment.name,
            patch,
            rows,
        },
        attachments,
        exclusions,
    })
}

fn read_tracked_binary_bytes(
    git_root: &Path,
    source: TrackedBinarySource<'_>,
    path: &str,
) -> Result<Option<Vec<u8>>, ExitError> {
    match source {
        TrackedBinarySource::Commit(rev) => git_show_blob_bytes(git_root, rev, path),
        TrackedBinarySource::Index => git_show_blob_bytes(git_root, ":", path),
        TrackedBinarySource::Worktree => Ok(fs::read(git_root.join(path)).ok()),
    }
}

fn git_show_blob_bytes(
    git_root: &Path,
    rev: &str,
    path: &str,
) -> Result<Option<Vec<u8>>, ExitError> {
    let spec = if rev == ":" {
        format!(":{path}")
    } else {
        format!("{rev}:{path}")
    };
    let out = Command::new("git")
        .args(["show", spec.as_str()])
        .current_dir(git_root)
        .output()
        .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to run git show: {e}")))?;

    if out.status.success() {
        return Ok(Some(out.stdout));
    }

    // Missing blob/path is expected in some edge cases (e.g., deletions).
    Ok(None)
}

fn build_range_plan(git_root: &Path, args: &BuildArgs) -> Result<RangePlan, ExitError> {
    let mode = parse_range_mode(&args.range_mode)?;
    match mode {
        RangeMode::Last => {
            let base = git::rev_parse(git_root, "HEAD~1").map_err(|_| {
                ExitError::new(
                    EXIT_GENERAL,
                    "failed to resolve HEAD~1 (repo may have only one commit; try --range-mode=root)",
                )
            })?;
            let target = git::rev_parse(git_root, "HEAD")?;
            let cnt = rev_list_count(git_root, &base, &target).ok();
            Ok(RangePlan {
                mode,
                base,
                target,
                from_rev: Some("HEAD~1".to_string()),
                to_rev: Some("HEAD".to_string()),
                a_rev: None,
                b_rev: None,
                merge_base: None,
                commit_count: cnt,
            })
        }
        RangeMode::Direct => {
            let from = args.from.clone().ok_or_else(|| {
                ExitError::new(EXIT_GENERAL, "--range-mode=direct requires --from <rev>")
            })?;
            let to = args.to.clone().ok_or_else(|| {
                ExitError::new(EXIT_GENERAL, "--range-mode=direct requires --to <rev>")
            })?;
            let base = git::rev_parse(git_root, &from)?;
            let target = git::rev_parse(git_root, &to)?;
            let cnt = rev_list_count(git_root, &base, &target).ok();
            Ok(RangePlan {
                mode,
                base,
                target,
                from_rev: Some(from),
                to_rev: Some(to),
                a_rev: None,
                b_rev: None,
                merge_base: None,
                commit_count: cnt,
            })
        }
        RangeMode::MergeBase => {
            let a = args.a.clone().ok_or_else(|| {
                ExitError::new(EXIT_GENERAL, "--range-mode=merge-base requires --a <rev>")
            })?;
            let b = args.b.clone().ok_or_else(|| {
                ExitError::new(EXIT_GENERAL, "--range-mode=merge-base requires --b <rev>")
            })?;
            let mb = git::run_git(git_root, ["merge-base", a.as_str(), b.as_str()])?;
            let mb = mb.trim().to_string();
            if mb.is_empty() {
                return Err(ExitError::new(
                    EXIT_GENERAL,
                    "git merge-base returned empty output",
                ));
            }
            let base = mb.clone();
            let target = git::rev_parse(git_root, &b)?;
            let cnt = rev_list_count(git_root, &base, &target).ok();
            Ok(RangePlan {
                mode,
                base,
                target,
                from_rev: None,
                to_rev: None,
                a_rev: Some(a),
                b_rev: Some(b),
                merge_base: Some(mb),
                commit_count: cnt,
            })
        }
        RangeMode::Root => {
            let to = args.to.clone().unwrap_or_else(|| "HEAD".to_string());
            let target = git::rev_parse(git_root, &to)?;
            let empty_tree = empty_tree_hash(git_root)?;
            Ok(RangePlan {
                mode,
                base: empty_tree,
                target,
                from_rev: None,
                to_rev: Some(to),
                a_rev: None,
                b_rev: None,
                merge_base: None,
                commit_count: Some(1),
            })
        }
    }
}

fn rev_list_count(git_root: &Path, base: &str, target: &str) -> Result<u64, ExitError> {
    let out = git::run_git(
        git_root,
        ["rev-list", "--count", &format!("{base}..{target}")],
    )?;
    out.trim().parse::<u64>().map_err(|e| {
        ExitError::new(
            EXIT_GENERAL,
            format!(
                "failed to parse rev-list --count output '{}': {e}",
                out.trim()
            ),
        )
    })
}

fn empty_tree_hash(git_root: &Path) -> Result<String, ExitError> {
    let output = Command::new("git")
        .args(["mktree"])
        .current_dir(git_root)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to run git mktree: {e}")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(ExitError::new(
            EXIT_GENERAL,
            format!("git mktree failed: {}", stderr.trim()),
        ));
    }
    let trimmed = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if trimmed.is_empty() {
        return Err(ExitError::new(
            EXIT_GENERAL,
            "git mktree returned empty output",
        ));
    }
    Ok(trimmed)
}

fn git_cat_file_size(git_root: &Path, target_commit: &str, path: &str) -> Result<u64, ExitError> {
    let spec = if target_commit == ":" {
        format!(":{path}")
    } else {
        format!("{target_commit}:{path}")
    };
    let out = git::run_git(git_root, ["cat-file", "-s", spec.as_str()])?;
    out.trim().parse::<u64>().map_err(|e| {
        ExitError::new(
            EXIT_GENERAL,
            format!("failed to parse cat-file -s output '{}': {e}", out.trim()),
        )
    })
}

fn file_size_on_disk(git_root: &Path, path: &str) -> Option<u64> {
    fs::metadata(git_root.join(path)).ok().map(|m| m.len())
}

fn count_text_lines(s: &str) -> u64 {
    if s.is_empty() {
        0
    } else {
        s.lines().count() as u64
    }
}

fn segment_rank(segment: &str) -> u8 {
    match segment {
        "committed" => 0,
        "staged" => 1,
        "unstaged" => 2,
        "untracked" => 3,
        _ => 9,
    }
}

fn path_category_rank(path: &str) -> u8 {
    match path_category_label(path) {
        "docs" => 0,
        "config" => 1,
        "source" => 2,
        "tests" => 3,
        _ => 4,
    }
}

fn path_category_label(path: &str) -> &'static str {
    if path.starts_with("docs/") || path.ends_with(".md") {
        "docs"
    } else if path.starts_with(".github/")
        || path.ends_with(".toml")
        || path.ends_with(".yml")
        || path.ends_with(".yaml")
        || path.ends_with(".json")
        || path.ends_with(".lock")
    {
        "config"
    } else if path.starts_with("src/") {
        "source"
    } else if path.starts_with("tests/") {
        "tests"
    } else {
        "other"
    }
}

fn sort_segments(segments: &mut [String]) {
    segments.sort_by(|a, b| segment_rank(a).cmp(&segment_rank(b)).then(a.cmp(b)));
}

fn sort_file_rows(rows: &mut [FileRow]) {
    rows.sort_by(|a, b| {
        path_category_rank(&a.path)
            .cmp(&path_category_rank(&b.path))
            .then(a.path.cmp(&b.path))
            .then(segment_rank(&a.segment).cmp(&segment_rank(&b.segment)))
            .then(a.status.cmp(&b.status))
            .then(a.note.cmp(&b.note))
    });
}

fn deterministic_zip_file_options() -> FileOptions {
    FileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .last_modified_time(ZipDateTime::default())
        .unix_permissions(0o644)
}

fn run_git_allow_diff_status<I, S>(git_root: &Path, args: I) -> Result<String, ExitError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    let output = Command::new("git")
        .args(args)
        .current_dir(git_root)
        .output()
        .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to run git: {e}")))?;
    let code = output.status.code().unwrap_or(1);
    if code != 0 && code != 1 {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(ExitError::new(
            EXIT_GENERAL,
            format!("git failed: {}", stderr.trim()),
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn write_text_file(path: &Path, contents: &str) -> Result<(), ExitError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            ExitError::new(
                EXIT_GENERAL,
                format!("failed to create parent dir {}: {e}", parent.display()),
            )
        })?;
    }
    let mut s = contents.to_string();
    if !s.ends_with('\n') {
        s.push('\n');
    }
    fs::write(path, s.as_bytes()).map_err(|e| {
        ExitError::new(
            EXIT_GENERAL,
            format!("failed to write {}: {e}", path.display()),
        )
    })
}

fn write_attachments_zip(path: &Path, entries: &[AttachmentEntry]) -> Result<(), ExitError> {
    let file = fs::File::create(path).map_err(|e| {
        ExitError::new(
            EXIT_GENERAL,
            format!("failed to create {}: {e}", path.display()),
        )
    })?;
    let mut zip = ZipWriter::new(file);
    let opts = deterministic_zip_file_options();
    let mut sorted = entries.to_vec();
    sorted.sort_by(|a, b| a.zip_path.cmp(&b.zip_path));
    for entry in sorted {
        zip.start_file(entry.zip_path, opts)
            .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to add zip entry: {e}")))?;
        zip.write_all(&entry.bytes)
            .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to write zip entry: {e}")))?;
    }
    zip.finish().map_err(|e| {
        ExitError::new(
            EXIT_GENERAL,
            format!("failed to finalize attachments zip: {e}"),
        )
    })?;
    Ok(())
}

fn render_excluded_md(exclusions: &[ExclusionEntry]) -> String {
    let mut items = exclusions.to_vec();
    items.sort_by(|a, b| a.path.cmp(&b.path));
    let mut s = String::new();
    s.push_str("# excluded.md\n\n");
    s.push_str(
        "The following items were intentionally excluded from patch parts or attachments.\n\n",
    );
    s.push_str("| path | reason | guidance |\n");
    s.push_str("|---|---|---|\n");
    for item in items {
        s.push_str(&format!(
            "| `{}` | {} | {} |\n",
            item.path, item.reason, item.guidance
        ));
    }
    s
}

fn render_secrets_md(hits: &[SecretHit]) -> String {
    let mut items = hits.to_vec();
    items.sort_by(|a, b| a.path.cmp(&b.path).then(a.reason.cmp(&b.reason)));
    let mut s = String::new();
    s.push_str("# secrets.md\n\n");
    s.push_str("Potential secrets-like content was detected. Paths and reasons are listed below; secret values are intentionally not shown.\n\n");
    s.push_str("| path | reason |\n");
    s.push_str("|---|---|\n");
    for item in items {
        s.push_str(&format!("| `{}` | {} |\n", item.path, item.reason));
    }
    s
}

fn handle_secret_hits(
    hits: &[SecretHit],
    yes: bool,
    fail_on_secrets: bool,
) -> Result<(), ExitError> {
    if hits.is_empty() {
        return Ok(());
    }
    let summary = format!(
        "secrets-like content detected ({} hit(s)); see secrets.md for paths and reasons",
        hits.len()
    );
    eprintln!("warning: {summary}");
    if fail_on_secrets {
        return Err(ExitError::new(
            EXIT_SECRETS_WARNING,
            format!("refused: {summary} (fail-on-secrets)"),
        ));
    }
    if yes {
        return Ok(());
    }
    if io::stdin().is_terminal() && io::stderr().is_terminal() {
        eprint!("Continue anyway? [y/N]: ");
        io::stderr()
            .flush()
            .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to flush prompt: {e}")))?;
        let mut line = String::new();
        io::stdin()
            .read_line(&mut line)
            .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to read answer: {e}")))?;
        let accepted = matches!(line.trim().to_ascii_lowercase().as_str(), "y" | "yes");
        if accepted {
            Ok(())
        } else {
            Err(ExitError::new(
                EXIT_SECRETS_WARNING,
                format!("refused: {summary}; rerun with --yes to continue"),
            ))
        }
    } else {
        Err(ExitError::new(
            EXIT_SECRETS_WARNING,
            format!("refused: {summary}; rerun with --yes to continue or --fail-on-secrets for CI"),
        ))
    }
}

fn scan_bundle_for_secrets(
    parts: &[PartOutput],
    attachments: &[AttachmentEntry],
) -> Result<Vec<SecretHit>, ExitError> {
    let mut hits = Vec::new();
    for part in parts {
        for reason in detect_secret_reasons(&part.patch) {
            hits.push(SecretHit {
                path: format!("parts/{}", part.name),
                reason,
            });
        }
    }
    for item in attachments {
        if item.bytes.len() > 2_000_000 {
            continue;
        }
        let text = String::from_utf8_lossy(&item.bytes);
        for reason in detect_secret_reasons(&text) {
            hits.push(SecretHit {
                path: format!("attachments.zip:{}", item.zip_path),
                reason,
            });
        }
    }
    hits.sort_by(|a, b| a.path.cmp(&b.path).then(a.reason.cmp(&b.reason)));
    hits.dedup_by(|a, b| a.path == b.path && a.reason == b.reason);
    Ok(hits)
}

fn detect_secret_reasons(s: &str) -> Vec<String> {
    let mut reasons = Vec::new();
    if contains_private_key_block(s) {
        reasons.push("private key block".to_string());
    }
    if contains_aws_access_key_id(s) {
        reasons.push("AWS access key id-like".to_string());
    }
    if contains_github_token(s) {
        reasons.push("GitHub token-like".to_string());
    }
    if contains_slack_token(s) {
        reasons.push("Slack token-like".to_string());
    }
    reasons
}

fn filter_segment_output(segment: SegmentOutput, filters: &PathFilter) -> SegmentOutput {
    if !filters.has_ignore_rules() && filters.includes().is_empty() && filters.excludes().is_empty()
    {
        return segment;
    }
    let keep = segment
        .rows
        .iter()
        .filter(|r| filters.allows(&r.path))
        .map(|r| r.path.clone())
        .collect::<BTreeSet<_>>();
    let rows = segment
        .rows
        .into_iter()
        .filter(|r| keep.contains(&r.path))
        .collect::<Vec<_>>();
    let patch = filter_patch_by_paths(&segment.patch, &keep);
    SegmentOutput {
        name: segment.name,
        patch,
        rows,
    }
}

fn filter_patch_by_paths(patch: &str, keep: &BTreeSet<String>) -> String {
    if keep.is_empty() {
        return String::new();
    }
    let mut out = String::new();
    let mut current = String::new();
    for line in patch.lines() {
        if line.starts_with("diff --git ") && !current.is_empty() {
            append_patch_chunk(&mut out, &current, keep);
            current.clear();
        }
        if !current.is_empty() {
            current.push('\n');
        }
        current.push_str(line);
    }
    if !current.is_empty() {
        append_patch_chunk(&mut out, &current, keep);
    }
    out
}

fn append_patch_chunk(out: &mut String, chunk: &str, keep: &BTreeSet<String>) {
    if let Some(path) = patch_chunk_path(chunk)
        && keep.contains(&path)
    {
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(chunk.trim_end());
        out.push('\n');
    }
}

fn patch_chunk_path(chunk: &str) -> Option<String> {
    let first = chunk.lines().next()?;
    let rest = first.strip_prefix("diff --git ")?;
    let mut parts = rest.split_whitespace();
    let a = parts.next()?.strip_prefix("a/").unwrap_or("");
    let b = parts.next()?.strip_prefix("b/").unwrap_or("");
    let path = if b.is_empty() { a } else { b };
    Some(path.to_string())
}

fn contains_private_key_block(s: &str) -> bool {
    s.contains("BEGIN RSA PRIVATE KEY")
        || s.contains("BEGIN OPENSSH PRIVATE KEY")
        || s.contains("BEGIN EC PRIVATE KEY")
        || s.contains("BEGIN PGP PRIVATE KEY BLOCK")
}

fn contains_aws_access_key_id(s: &str) -> bool {
    contains_prefixed_token(s, "AKIA", 16) || contains_prefixed_token(s, "ASIA", 16)
}

fn contains_prefixed_token(s: &str, prefix: &str, len_after: usize) -> bool {
    let bytes = s.as_bytes();
    let p = prefix.as_bytes();
    if bytes.len() < p.len() + len_after {
        return false;
    }
    let mut i = 0;
    while i + p.len() + len_after <= bytes.len() {
        if &bytes[i..i + p.len()] == p {
            let after = &bytes[i + p.len()..i + p.len() + len_after];
            if after
                .iter()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
            {
                return true;
            }
        }
        i += 1;
    }
    false
}

fn contains_github_token(s: &str) -> bool {
    s.contains("ghp_") || s.contains("github_pat_")
}

fn contains_slack_token(s: &str) -> bool {
    s.contains("xoxb-") || s.contains("xoxp-") || s.contains("xoxa-")
}

fn parse_numstat(s: &str) -> HashMap<String, (Option<u64>, Option<u64>)> {
    let mut map = HashMap::new();
    for line in s.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let mut parts = line.split('\t');
        let ins_s = parts.next().unwrap_or("");
        let del_s = parts.next().unwrap_or("");
        let path = parts.next().unwrap_or("").trim();
        if path.is_empty() {
            continue;
        }
        map.insert(
            path.to_string(),
            (ins_s.parse::<u64>().ok(), del_s.parse::<u64>().ok()),
        );
    }
    map
}

fn parse_name_status(
    segment: &str,
    name_status: &str,
    insdel: &HashMap<String, (Option<u64>, Option<u64>)>,
) -> Vec<FileRow> {
    let mut rows = vec![];
    for line in name_status.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut parts = line.split('\t');
        let status = parts.next().unwrap_or("").trim().to_string();
        if status.is_empty() {
            continue;
        }
        let (path, note) = if status.starts_with('R') || status.starts_with('C') {
            let old = parts.next().unwrap_or("").to_string();
            let new = parts.next().unwrap_or("").to_string();
            (new, format!("from {old}"))
        } else {
            (parts.next().unwrap_or("").to_string(), String::new())
        };
        let st = status.chars().next().unwrap_or('?').to_string();
        let (ins, del) = insdel.get(&path).cloned().unwrap_or((None, None));
        rows.push(FileRow {
            segment: segment.to_string(),
            status: st,
            path,
            note,
            ins,
            del,
            bytes: None,
            part: String::new(),
        });
    }
    sort_file_rows(&mut rows);
    rows
}

#[derive(Debug, Default)]
struct TreeNode {
    dirs: BTreeMap<String, TreeNode>,
    files: BTreeSet<String>,
}

fn render_changed_tree(paths: &[String]) -> String {
    let mut root = TreeNode::default();
    for p in paths {
        let parts: Vec<&str> = p.split('/').filter(|s| !s.is_empty()).collect();
        if parts.is_empty() {
            continue;
        }
        let mut cur = &mut root;
        for (i, part) in parts.iter().enumerate() {
            let is_last = i == parts.len() - 1;
            if is_last {
                cur.files.insert((*part).to_string());
            } else {
                cur = cur.dirs.entry((*part).to_string()).or_default();
            }
        }
    }
    let mut out = String::new();
    render_tree_node(&root, 0, &mut out);
    out
}

fn render_tree_node(node: &TreeNode, depth: usize, out: &mut String) {
    let indent = "  ".repeat(depth);
    for (dir, child) in &node.dirs {
        out.push_str(&format!("{indent}{dir}/\n"));
        render_tree_node(child, depth + 1, out);
    }
    for f in &node.files {
        out.push_str(&format!("{indent}{f}\n"));
    }
}

fn render_parts_index(parts: &[PartOutput], rows: &[FileRow]) -> String {
    let mut s = String::new();
    s.push_str("### 3.1 Quick index\n");
    s.push_str("| part | segments | files | approx bytes | first files |\n");
    s.push_str("|---|---|---:|---:|---|\n");

    for part in parts {
        let approx_bytes = part.patch.len();
        let mut files = rows
            .iter()
            .filter(|r| r.part == part.name)
            .map(|r| r.path.clone())
            .collect::<Vec<_>>();
        files.sort();
        files.dedup();

        let preview = if files.is_empty() {
            "-".to_string()
        } else {
            files
                .iter()
                .take(3)
                .map(|p| format!("`{}`", p))
                .collect::<Vec<_>>()
                .join(", ")
        };

        s.push_str(&format!(
            "| `{}` | `{}` | {} | {} | {} |\n",
            part.name,
            part.segments.join(", "),
            files.len(),
            approx_bytes,
            preview,
        ));
    }

    s.push_str("\n### 3.2 Part details\n");
    for part in parts {
        let approx_bytes = part.patch.len();
        let mut top = rows
            .iter()
            .filter(|r| r.part == part.name)
            .map(|r| r.path.clone())
            .collect::<Vec<_>>();
        top.sort();
        top.dedup();
        if top.len() > 8 {
            top.truncate(8);
        }
        s.push_str(&format!("#### {}\n", part.name));
        s.push_str(&format!("- approx bytes: `{}`\n", approx_bytes));
        s.push_str(&format!("- segments: `{}`\n", part.segments.join(", ")));
        s.push_str("- top files:\n");
        for p in top {
            s.push_str(&format!("  - `{}`\n", p));
        }
        s.push('\n');
    }
    s
}

#[derive(Debug, Default)]
struct CatSummary {
    files: BTreeSet<String>,
    parts: BTreeSet<String>,
}

fn render_category_summary_and_reading_order(rows: &[FileRow]) -> (String, Vec<String>) {
    let mut docs = CatSummary::default();
    let mut cfg = CatSummary::default();
    let mut src = CatSummary::default();
    let mut tests = CatSummary::default();
    let mut other = CatSummary::default();

    for r in rows {
        let slot = if r.path.starts_with("docs/") || r.path.ends_with(".md") {
            &mut docs
        } else if r.path.starts_with("src/") {
            &mut src
        } else if r.path.starts_with("tests/") {
            &mut tests
        } else if r.path.starts_with(".github/")
            || r.path.ends_with(".toml")
            || r.path.ends_with(".yml")
            || r.path.ends_with(".yaml")
            || r.path.ends_with(".json")
            || r.path.ends_with(".lock")
        {
            &mut cfg
        } else {
            &mut other
        };
        slot.files.insert(r.path.clone());
        if !r.part.is_empty() && r.part != "-" {
            slot.parts.insert(r.part.clone());
        }
    }

    let mut s = String::new();
    s.push_str(&format!(
        "- Docs: `{}` files → parts: `{}`\n",
        docs.files.len(),
        join_parts(&docs.parts)
    ));
    s.push_str(&format!(
        "- Config/CI: `{}` files → parts: `{}`\n",
        cfg.files.len(),
        join_parts(&cfg.parts)
    ));
    s.push_str(&format!(
        "- Source: `{}` files → parts: `{}`\n",
        src.files.len(),
        join_parts(&src.parts)
    ));
    s.push_str(&format!(
        "- Tests: `{}` files → parts: `{}`\n",
        tests.files.len(),
        join_parts(&tests.parts)
    ));
    s.push_str(&format!(
        "- Other: `{}` files → parts: `{}`\n",
        other.files.len(),
        join_parts(&other.parts)
    ));

    let mut order = vec![];
    push_order(&mut order, "Docs changes", &docs);
    push_order(&mut order, "Config/build changes", &cfg);
    push_order(&mut order, "Source changes", &src);
    push_order(&mut order, "Tests", &tests);
    push_order(&mut order, "Other changes", &other);
    if order.is_empty() {
        order.push("No file changes detected".to_string());
    }
    (s, order)
}

fn push_order(out: &mut Vec<String>, label: &str, cat: &CatSummary) {
    if !cat.files.is_empty() {
        out.push(format!(
            "{}: `{}` ({} files)",
            label,
            join_parts(&cat.parts),
            cat.files.len()
        ));
    }
}

fn join_parts(parts: &BTreeSet<String>) -> String {
    if parts.is_empty() {
        "-".to_string()
    } else {
        parts.iter().cloned().collect::<Vec<_>>().join(", ")
    }
}

struct HandoffDocInputs<'a> {
    out_dir: &'a Path,
    plan: Option<&'a RangePlan>,
    head: &'a str,
    split_by: SplitBy,
    packing_limits: PackingLimits,
    binary_policy: BinaryPolicy,
    sources: SourceSelection,
    untracked_mode: UntrackedMode,
    changed_tree: &'a str,
    rows: &'a [FileRow],
    cat_summary: &'a str,
    parts_index: &'a str,
    reading_order: &'a [String],
    first_part_rel: &'a str,
    commit_views: &'a [CommitView],
    attachments: &'a [AttachmentEntry],
    exclusions: &'a [ExclusionEntry],
    ignore_enabled: bool,
    include_patterns: &'a [String],
    exclude_patterns: &'a [String],
    secret_hits: &'a [SecretHit],
    project_context: Option<&'a ProjectContextManifest>,
}

struct HandoffManifestInputs<'a> {
    plan: Option<&'a RangePlan>,
    head: &'a str,
    split_by: SplitBy,
    packing_limits: &'a PackingLimits,
    binary_policy: BinaryPolicy,
    sources: SourceSelection,
    untracked_mode: UntrackedMode,
    rows: &'a [FileRow],
    parts: &'a [PartOutput],
    commit_views: &'a [CommitView],
    attachments: &'a [AttachmentEntry],
    exclusions: &'a [ExclusionEntry],
    ignore_enabled: bool,
    include_patterns: &'a [String],
    exclude_patterns: &'a [String],
    secret_hits: &'a [SecretHit],
    reading_order: &'a [String],
    file_semantics: &'a BTreeMap<String, ManifestFileSemantic>,
    project_context: Option<&'a ProjectContextManifest>,
    task_groups: &'a [TaskGroupComputed],
}

fn render_handoff_md(inp: &HandoffDocInputs<'_>) -> String {
    let bundle_name = inp
        .out_dir
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| inp.out_dir.display().to_string());

    let (range_mode, range_desc) = if let Some(plan) = inp.plan {
        match plan.mode {
            RangeMode::Direct => (
                "direct",
                format!(
                    "from/to: `{}` → `{}`",
                    plan.from_rev.as_deref().unwrap_or("?"),
                    plan.to_rev.as_deref().unwrap_or("?"),
                ),
            ),
            RangeMode::MergeBase => (
                "merge-base",
                format!(
                    "a/b: `{}` / `{}` (merge-base: `{}`)",
                    plan.a_rev.as_deref().unwrap_or("?"),
                    plan.b_rev.as_deref().unwrap_or("?"),
                    plan.merge_base.as_deref().unwrap_or("?"),
                ),
            ),
            RangeMode::Last => ("last", "HEAD~1..HEAD".to_string()),
            RangeMode::Root => (
                "root",
                format!(
                    "empty-tree → `{}`",
                    plan.to_rev.as_deref().unwrap_or("HEAD")
                ),
            ),
        }
    } else {
        ("disabled", "committed range not included".to_string())
    };

    let mut s = String::new();
    s.push_str("# HANDOFF\n\n");
    s.push_str("## Start Here\n");
    s.push_str("1. Read the TL;DR to understand the scope and included segments.\n");
    s.push_str(
        "2. Use the Change Map to see which files changed and which patch part they belong to.\n",
    );
    s.push_str("3. Read `AI_REQUESTS.md` if you are forwarding this bundle to a hosted AI.\n");
    if inp.project_context.is_some() {
        s.push_str(
            "4. Read `PROJECT_CONTEXT.md` before widening scope beyond the changed files.\n",
        );
        s.push_str("5. Use the Parts Index to decide reading order inside the patch bundle.\n");
    } else {
        s.push_str("4. Use the Parts Index to decide reading order inside the patch bundle.\n");
    }
    s.push_str(&format!(
        "{}. Open the first patch part: `{}`\n",
        if inp.project_context.is_some() { 6 } else { 5 },
        inp.first_part_rel
    ));
    if !inp.attachments.is_empty() {
        s.push_str(&format!(
            "{}. After the patch parts, inspect attachments.zip for raw files that were intentionally kept out of patch text.\n",
            if inp.project_context.is_some() { 7 } else { 6 }
        ));
    }
    if !inp.exclusions.is_empty() {
        s.push_str(&format!(
            "{}. Check excluded.md for files that were omitted on purpose and why.\n",
            if inp.project_context.is_some() { 8 } else { 7 }
        ));
    }
    if !inp.secret_hits.is_empty() {
        s.push_str(&format!(
            "{}. Review secrets.md before sharing the bundle. It lists only paths and reasons, never secret values.\n",
            if inp.project_context.is_some() { 9 } else { 8 }
        ));
    }

    s.push_str("\n---\n\n");
    s.push_str("## TL;DR\n");
    s.push_str(&format!("- Bundle: `{}`\n", bundle_name));
    s.push_str(&format!(
        "- Profile: `{}` (`max_parts={}`, `max_bytes_per_part={}`; split-by=`{}`)\n",
        inp.packing_limits.profile_label,
        inp.packing_limits.max_parts,
        inp.packing_limits.max_bytes_per_part,
        split_label(inp.split_by),
    ));
    s.push_str(&format!(
        "- Binary policy: include=`{}`, mode=`{}`\n",
        yes_no(inp.binary_policy.include_binary),
        binary_mode_label(inp.binary_policy.binary_mode)
    ));
    s.push_str(&format!(
        "- Segments included: committed=`{}`, staged=`{}`, unstaged=`{}`, untracked=`{}`\n",
        yes_no(inp.sources.include_committed),
        yes_no(inp.sources.include_staged),
        yes_no(inp.sources.include_unstaged),
        yes_no(inp.sources.include_untracked),
    ));
    s.push_str(&format!(
        "- Committed range: `{}` ({})\n",
        range_mode, range_desc
    ));
    if let Some(plan) = inp.plan
        && let Some(n) = plan.commit_count
    {
        s.push_str(&format!("- Commit count (approx): `{}`\n", n));
    }
    s.push_str(&format!(
        "- Current HEAD (workspace base): `{}`\n",
        inp.head.trim()
    ));
    s.push_str(&format!(
        "- Ignore rules: `.diffshipignore` = `{}`\n",
        yes_no(inp.ignore_enabled)
    ));
    if !inp.include_patterns.is_empty() {
        s.push_str(&format!(
            "- Include filters: `{}`\n",
            inp.include_patterns.join("`, `")
        ));
    }
    if !inp.exclude_patterns.is_empty() {
        s.push_str(&format!(
            "- Exclude filters: `{}`\n",
            inp.exclude_patterns.join("`, `")
        ));
    }
    if !inp.attachments.is_empty() {
        s.push_str(&format!(
            "- Attachments: `attachments.zip` ({} file(s))\n",
            inp.attachments.len()
        ));
    }
    if !inp.exclusions.is_empty() {
        s.push_str(&format!(
            "- Exclusions: `excluded.md` ({} item(s))\n",
            inp.exclusions.len()
        ));
    }
    if !inp.secret_hits.is_empty() {
        s.push_str(&format!(
            "- Secrets warnings: `secrets.md` ({} hit(s))\n",
            inp.secret_hits.len()
        ));
    }
    s.push_str("- AI request kit: `AI_REQUESTS.md`\n");
    if let Some(project_context) = inp.project_context {
        s.push_str(&format!(
            "- Project context: `PROJECT_CONTEXT.md` + `{}` snapshot(s) (`{}` omitted)\n",
            project_context.summary.included_snapshots, project_context.summary.omitted_files
        ));
    }
    s.push_str("- Reading order:\n");
    for (i, line) in inp.reading_order.iter().enumerate() {
        s.push_str(&format!("  {}. {}\n", i + 1, line));
    }

    s.push_str("\n---\n\n");
    s.push_str("## 1) Range & Sources Summary\n");
    s.push_str("### Committed range\n");
    if let Some(plan) = inp.plan {
        s.push_str("- included: `yes`\n");
        s.push_str(&format!("- mode: `{}`\n", range_mode));
        s.push_str(&format!("- {}\n", range_desc));
        if let Some(mb) = plan.merge_base.as_deref() {
            s.push_str(&format!("- merge-base: `{}`\n", mb));
        }
        if let Some(n) = plan.commit_count {
            s.push_str(&format!("- commit count: `{}`\n", n));
        }
    } else {
        s.push_str("- included: `no`\n- mode: `disabled`\n");
    }

    s.push_str("\n### Current workspace base (for uncommitted segments)\n");
    s.push_str(&format!("- HEAD: `{}`\n", inp.head.trim()));
    s.push_str(&format!(
        "- staged: `{}` (base: `HEAD`)\n",
        yes_no(inp.sources.include_staged)
    ));
    s.push_str(&format!(
        "- unstaged: `{}` (base: `HEAD` / working tree)\n",
        yes_no(inp.sources.include_unstaged)
    ));
    s.push_str(&format!(
        "- untracked: `{}` (base: `HEAD`, mode: `{}`)\n",
        yes_no(inp.sources.include_untracked),
        untracked_label(inp.untracked_mode)
    ));
    s.push_str(&format!(
        "- binary include: `{}` (mode: `{}`)\n",
        yes_no(inp.binary_policy.include_binary),
        binary_mode_label(inp.binary_policy.binary_mode)
    ));
    s.push_str(&format!(
        "- .diffshipignore active: `{}`\n",
        yes_no(inp.ignore_enabled)
    ));
    if !inp.include_patterns.is_empty() {
        s.push_str(&format!(
            "- include filters: `{}`\n",
            inp.include_patterns.join("`, `")
        ));
    }
    if !inp.exclude_patterns.is_empty() {
        s.push_str(&format!(
            "- exclude filters: `{}`\n",
            inp.exclude_patterns.join("`, `")
        ));
    }

    s.push_str("\n---\n\n## 2) Change Map\n\n");
    s.push_str("### 2.1 Changed Tree (changed files only)\n```text\n");
    s.push_str(inp.changed_tree);
    if !inp.changed_tree.ends_with('\n') {
        s.push('\n');
    }
    s.push_str("```\n\n");

    s.push_str("### 2.2 File Table (part mapping)\n");
    s.push_str("| segment | status | path | ins | del | bytes | part | note |\n");
    s.push_str("|---|---:|---|---:|---:|---:|---|---|\n");
    for r in inp.rows {
        let ins = r.ins.map(|v| v.to_string()).unwrap_or_default();
        let del = r.del.map(|v| v.to_string()).unwrap_or_default();
        let bytes = r.bytes.map(|v| v.to_string()).unwrap_or_default();
        let part = if r.part.is_empty() {
            "-"
        } else {
            r.part.as_str()
        };
        s.push_str(&format!(
            "| {} | {} | `{}` | {} | {} | {} | {} | {} |\n",
            r.segment, r.status, r.path, ins, del, bytes, part, r.note
        ));
    }

    s.push_str("\n### 2.3 Category Summary\n");
    s.push_str(inp.cat_summary);
    s.push_str("\n---\n\n## 3) Parts Index\n\n");
    s.push_str("Use this section to decide reading order inside the patch bundle.\n\n");
    s.push_str(inp.parts_index);

    if !inp.commit_views.is_empty() {
        s.push_str("\n---\n\n## 4) Commit View\n");
        for cv in inp.commit_views {
            s.push_str(&format!("### {} {} ({})\n", cv.hash7, cv.subject, cv.date));
            s.push_str(&format!(
                "- stats: `{}` files, `+{} -{}`\n",
                cv.files.len(),
                cv.ins.unwrap_or(0),
                cv.del.unwrap_or(0)
            ));
            s.push_str("- files:\n");
            for (path, part) in &cv.files {
                s.push_str(&format!("  - `{}` → `{}`\n", path, part));
            }
            s.push('\n');
        }
    }

    if !inp.attachments.is_empty() {
        s.push_str("\n---\n\n## 5) Attachments\n");
        s.push_str("- `attachments.zip` contains:\n");
        let mut items = inp.attachments.iter().collect::<Vec<_>>();
        items.sort_by(|a, b| a.zip_path.cmp(&b.zip_path));
        for item in items {
            s.push_str(&format!(
                "  - `{}` (reason: {})\n",
                item.zip_path, item.reason
            ));
        }
    }

    if !inp.exclusions.is_empty() {
        s.push_str("\n---\n\n## 6) Exclusions\nSee `excluded.md`.\n");
    }

    if !inp.secret_hits.is_empty() {
        s.push_str("\n---\n\n## 7) Secrets Warnings\n");
        s.push_str("Potential secrets-like content was detected. Review `secrets.md` before sharing the bundle. Only paths and reasons are listed there.\n");
    }

    s.push_str("\n---\n\n## Where to start\n\n");
    s.push_str("Open this document first.\n");
    s.push_str(
        "Then read `AI_REQUESTS.md` if you need a bundle-local hosted-AI request scaffold.\n",
    );
    if inp.project_context.is_some() {
        s.push_str(
            "Then read `PROJECT_CONTEXT.md` if you need surrounding repo structure beyond the changed files.\n",
        );
    }
    s.push_str(&format!("Then apply/read `{}`.\n", inp.first_part_rel));
    if !inp.attachments.is_empty() {
        s.push_str("After patch parts, inspect `attachments.zip` for raw files that were intentionally kept out of patch text.\n");
    }
    if !inp.secret_hits.is_empty() {
        s.push_str("Before sharing externally, review `secrets.md`.\n");
    }

    s.push_str("\n---\n\n## Notes\n");
    s.push_str("- split-by=commit applies only to committed range; staged/unstaged/untracked remain file-level units.\n");
    s.push_str("- Binary/unreadable files are excluded by default; use `--include-binary --binary-mode raw|patch|meta` to include them.\n");
    s.push_str("- `.diffshipignore` is applied before writing parts / attachments / exclusions.\n");
    s.push_str("- Explicit `--include` / `--exclude` path filters apply consistently to all selected segments.\n");
    s
}

fn build_project_context_bundle(
    git_root: &Path,
    rows: &[FileRow],
    file_semantics: &BTreeMap<String, ManifestFileSemantic>,
    mode: ProjectContextMode,
) -> Result<Option<ProjectContextBundle>, ExitError> {
    if mode == ProjectContextMode::None {
        return Ok(None);
    }

    let mut selected = BTreeMap::<String, BTreeSet<String>>::new();
    let changed_paths = rows
        .iter()
        .map(|row| row.path.clone())
        .collect::<BTreeSet<_>>();
    for path in &changed_paths {
        selected
            .entry(path.clone())
            .or_default()
            .insert("changed".to_string());
    }
    for path in &changed_paths {
        let Some(semantic) = file_semantics.get(path) else {
            continue;
        };
        for candidate in &semantic.related_test_candidates {
            selected
                .entry(candidate.clone())
                .or_default()
                .insert(format!("related-test:{path}"));
        }
        for candidate in &semantic.related_source_candidates {
            selected
                .entry(candidate.clone())
                .or_default()
                .insert(format!("related-source:{path}"));
        }
        for candidate in &semantic.related_doc_candidates {
            selected
                .entry(candidate.clone())
                .or_default()
                .insert(format!("related-doc:{path}"));
        }
        for candidate in &semantic.related_config_candidates {
            selected
                .entry(candidate.clone())
                .or_default()
                .insert(format!("related-config:{path}"));
        }
    }
    for (path, reason) in [
        ("README.md", "root-readme"),
        (".diffship/PROJECT_RULES.md", "project-rules"),
        (".diffship/AI_GUIDE.md", "ai-guide"),
        (".diffship/PROJECT_KIT.md", "project-kit"),
    ] {
        if git_root.join(path).is_file() {
            selected
                .entry(path.to_string())
                .or_default()
                .insert(reason.to_string());
        }
    }

    let mut total_snapshot_bytes = 0usize;
    let mut files = Vec::new();
    let mut snapshots = Vec::new();
    let mut ordered_paths = selected.keys().cloned().collect::<Vec<_>>();
    ordered_paths.sort_by(|a, b| {
        path_category_rank(a)
            .cmp(&path_category_rank(b))
            .then(a.cmp(b))
    });
    let selected_paths = ordered_paths.iter().cloned().collect::<BTreeSet<_>>();
    let project_context_semantics =
        build_project_context_semantics(&selected_paths, file_semantics);
    let relationships =
        build_project_context_relationships(&selected_paths, &project_context_semantics);
    let relationship_refs = build_project_context_relationship_refs(&relationships);
    for path in ordered_paths {
        let reasons = selected
            .get(&path)
            .map(|items| items.iter().cloned().collect::<Vec<_>>())
            .unwrap_or_default();
        let semantic = project_context_semantics
            .get(&path)
            .cloned()
            .unwrap_or_else(|| {
                build_manifest_file_semantic(&path, &selected_paths, FilePatchClues::default())
            });
        let (outbound_relationships, inbound_relationships) = relationship_refs
            .get(&path)
            .cloned()
            .unwrap_or_else(|| (Vec::new(), Vec::new()));
        let changed = changed_paths.contains(&path);
        let usage_role = build_project_context_usage_role(&path, changed, &reasons);
        let priority = build_project_context_priority(
            changed,
            &reasons,
            &outbound_relationships,
            &inbound_relationships,
        );
        let verification_relevance = build_project_context_verification_relevance(
            changed,
            &semantic,
            &reasons,
            &outbound_relationships,
            &inbound_relationships,
        );
        let verification_labels = build_project_context_verification_labels(
            changed,
            &semantic,
            &reasons,
            &outbound_relationships,
            &inbound_relationships,
        );
        let why_included = build_project_context_why_included(changed, &reasons);
        let entry = build_project_context_file(
            git_root,
            ProjectContextFileInputs {
                path: path.clone(),
                changed,
                source_reasons: reasons,
                usage_role,
                priority,
                edit_scope_role: "read_only_context".to_string(),
                verification_relevance,
                verification_labels,
                why_included,
                semantic,
                outbound_relationships,
                inbound_relationships,
            },
            &mut total_snapshot_bytes,
            &mut snapshots,
        )?;
        files.push(entry);
    }

    let manifest = ProjectContextManifest {
        schema_version: 1,
        mode: "focused".to_string(),
        patch_canonical: true,
        entrypoint: "HANDOFF.md".to_string(),
        rendered_view: project_context_md_path().to_string(),
        snapshot_root: project_context_snapshot_root().to_string(),
        summary: build_project_context_summary(&files, &relationships, total_snapshot_bytes),
        top_level_dirs: build_project_context_top_level_dirs(&files),
        files,
        relationships,
    };
    let markdown = render_project_context_md(&manifest);
    Ok(Some(ProjectContextBundle {
        manifest,
        markdown,
        snapshots,
    }))
}

fn build_project_context_file(
    git_root: &Path,
    input: ProjectContextFileInputs,
    total_snapshot_bytes: &mut usize,
    snapshots: &mut Vec<ProjectContextSnapshot>,
) -> Result<ProjectContextFile, ExitError> {
    let workspace_path = git_root.join(&input.path);
    let context_labels = build_project_context_context_labels(&input);
    if !workspace_path.is_file() {
        return Ok(ProjectContextFile {
            path: input.path.clone(),
            category: path_category_label(&input.path).to_string(),
            changed: input.changed,
            source_reasons: input.source_reasons,
            usage_role: input.usage_role,
            priority: input.priority,
            edit_scope_role: input.edit_scope_role,
            verification_relevance: input.verification_relevance,
            verification_labels: input.verification_labels,
            why_included: input.why_included,
            task_group_refs: Vec::new(),
            context_labels: context_labels.clone(),
            semantic: input.semantic,
            outbound_relationships: input.outbound_relationships,
            inbound_relationships: input.inbound_relationships,
            exists_in_workspace: false,
            included: false,
            snapshot_path: None,
            byte_len: None,
            omitted_reason: Some("missing-in-workspace".to_string()),
        });
    }
    if is_generated_like_path(&input.path) {
        return Ok(ProjectContextFile {
            path: input.path.clone(),
            category: path_category_label(&input.path).to_string(),
            changed: input.changed,
            source_reasons: input.source_reasons,
            usage_role: input.usage_role,
            priority: input.priority,
            edit_scope_role: input.edit_scope_role,
            verification_relevance: input.verification_relevance,
            verification_labels: input.verification_labels,
            why_included: input.why_included,
            task_group_refs: Vec::new(),
            context_labels: context_labels.clone(),
            semantic: input.semantic,
            outbound_relationships: input.outbound_relationships,
            inbound_relationships: input.inbound_relationships,
            exists_in_workspace: true,
            included: false,
            snapshot_path: None,
            byte_len: None,
            omitted_reason: Some("generated-like".to_string()),
        });
    }

    let bytes = fs::read(&workspace_path).map_err(|e| {
        ExitError::new(
            EXIT_GENERAL,
            format!(
                "failed to read project context candidate {}: {e}",
                workspace_path.display()
            ),
        )
    })?;
    if bytes.len() > PROJECT_CONTEXT_PER_FILE_MAX_BYTES {
        return Ok(ProjectContextFile {
            path: input.path.clone(),
            category: path_category_label(&input.path).to_string(),
            changed: input.changed,
            source_reasons: input.source_reasons,
            usage_role: input.usage_role,
            priority: input.priority,
            edit_scope_role: input.edit_scope_role,
            verification_relevance: input.verification_relevance,
            verification_labels: input.verification_labels,
            why_included: input.why_included,
            task_group_refs: Vec::new(),
            context_labels: context_labels.clone(),
            semantic: input.semantic,
            outbound_relationships: input.outbound_relationships,
            inbound_relationships: input.inbound_relationships,
            exists_in_workspace: true,
            included: false,
            snapshot_path: None,
            byte_len: Some(bytes.len()),
            omitted_reason: Some("oversized".to_string()),
        });
    }
    if *total_snapshot_bytes + bytes.len() > PROJECT_CONTEXT_TOTAL_MAX_BYTES {
        return Ok(ProjectContextFile {
            path: input.path.clone(),
            category: path_category_label(&input.path).to_string(),
            changed: input.changed,
            source_reasons: input.source_reasons,
            usage_role: input.usage_role,
            priority: input.priority,
            edit_scope_role: input.edit_scope_role,
            verification_relevance: input.verification_relevance,
            verification_labels: input.verification_labels,
            why_included: input.why_included,
            task_group_refs: Vec::new(),
            context_labels: context_labels.clone(),
            semantic: input.semantic,
            outbound_relationships: input.outbound_relationships,
            inbound_relationships: input.inbound_relationships,
            exists_in_workspace: true,
            included: false,
            snapshot_path: None,
            byte_len: Some(bytes.len()),
            omitted_reason: Some("budget-exceeded".to_string()),
        });
    }
    if std::str::from_utf8(&bytes).is_err() {
        return Ok(ProjectContextFile {
            path: input.path.clone(),
            category: path_category_label(&input.path).to_string(),
            changed: input.changed,
            source_reasons: input.source_reasons,
            usage_role: input.usage_role,
            priority: input.priority,
            edit_scope_role: input.edit_scope_role,
            verification_relevance: input.verification_relevance,
            verification_labels: input.verification_labels,
            why_included: input.why_included,
            task_group_refs: Vec::new(),
            context_labels: context_labels.clone(),
            semantic: input.semantic,
            outbound_relationships: input.outbound_relationships,
            inbound_relationships: input.inbound_relationships,
            exists_in_workspace: true,
            included: false,
            snapshot_path: None,
            byte_len: Some(bytes.len()),
            omitted_reason: Some("non-utf8".to_string()),
        });
    }

    let snapshot_path = format!("{}/{}", project_context_snapshot_root(), input.path);
    *total_snapshot_bytes += bytes.len();
    snapshots.push(ProjectContextSnapshot {
        path: snapshot_path.clone(),
        bytes: bytes.clone(),
    });
    Ok(ProjectContextFile {
        path: input.path.clone(),
        category: path_category_label(&input.path).to_string(),
        changed: input.changed,
        source_reasons: input.source_reasons,
        usage_role: input.usage_role,
        priority: input.priority,
        edit_scope_role: input.edit_scope_role,
        verification_relevance: input.verification_relevance,
        verification_labels: input.verification_labels,
        why_included: input.why_included,
        task_group_refs: Vec::new(),
        context_labels,
        semantic: input.semantic,
        outbound_relationships: input.outbound_relationships,
        inbound_relationships: input.inbound_relationships,
        exists_in_workspace: true,
        included: true,
        snapshot_path: Some(snapshot_path),
        byte_len: Some(bytes.len()),
        omitted_reason: None,
    })
}

fn build_project_context_summary(
    files: &[ProjectContextFile],
    relationships: &[ProjectContextRelationship],
    total_snapshot_bytes: usize,
) -> ProjectContextSummary {
    ProjectContextSummary {
        selected_files: files.len(),
        changed_files: files.iter().filter(|file| file.changed).count(),
        supplemental_files: files.iter().filter(|file| !file.changed).count(),
        included_snapshots: files.iter().filter(|file| file.included).count(),
        omitted_files: files.iter().filter(|file| !file.included).count(),
        total_snapshot_bytes,
        relationship_count: relationships.len(),
        categories: count_project_context_categories(files),
        priority_counts: count_project_context_priorities(files),
        edit_scope_counts: count_project_context_edit_scope(files),
        verification_relevance_counts: count_project_context_verification_relevance(files),
        relationship_kinds: count_project_context_relationship_kinds(relationships),
    }
}

fn build_project_context_top_level_dirs(
    files: &[ProjectContextFile],
) -> Vec<ProjectContextTopLevelDir> {
    let mut counts = BTreeMap::<String, usize>::new();
    for file in files {
        let dir = file
            .path
            .split('/')
            .next()
            .filter(|value| !value.is_empty())
            .unwrap_or(".");
        *counts.entry(dir.to_string()).or_insert(0) += 1;
    }
    counts
        .into_iter()
        .map(|(path, file_count)| ProjectContextTopLevelDir { path, file_count })
        .collect()
}

fn count_project_context_categories(files: &[ProjectContextFile]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for file in files {
        *counts.entry(file.category.clone()).or_insert(0) += 1;
    }
    counts
}

fn count_project_context_priorities(files: &[ProjectContextFile]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for file in files {
        *counts.entry(file.priority.clone()).or_insert(0) += 1;
    }
    counts
}

fn count_project_context_edit_scope(files: &[ProjectContextFile]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for file in files {
        *counts.entry(file.edit_scope_role.clone()).or_insert(0) += 1;
    }
    counts
}

fn count_project_context_verification_relevance(
    files: &[ProjectContextFile],
) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for file in files {
        *counts
            .entry(file.verification_relevance.clone())
            .or_insert(0) += 1;
    }
    counts
}

fn count_project_context_relationship_kinds(
    relationships: &[ProjectContextRelationship],
) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for relationship in relationships {
        *counts.entry(relationship.kind.clone()).or_insert(0) += 1;
    }
    counts
}

fn build_project_context_context_labels(input: &ProjectContextFileInputs) -> Vec<String> {
    let mut labels = BTreeSet::new();
    if input.changed {
        labels.insert("changed_target".to_string());
    } else {
        labels.insert("supplemental_context".to_string());
    }
    labels.insert(
        match path_category_label(&input.path) {
            "docs" => "doc_context",
            "config" => "config_context",
            "source" => "source_context",
            "tests" => "test_context",
            _ => "other_context",
        }
        .to_string(),
    );
    if input
        .source_reasons
        .iter()
        .any(|reason| reason.starts_with("related-"))
    {
        labels.insert("related_context".to_string());
    }
    if input.source_reasons.iter().any(|reason| {
        matches!(
            reason.as_str(),
            "root-readme" | "project-rules" | "ai-guide" | "project-kit"
        )
    }) {
        labels.insert("repo_guide_context".to_string());
    }
    if !input.outbound_relationships.is_empty() {
        labels.insert("relationship_source".to_string());
    }
    if !input.inbound_relationships.is_empty() {
        labels.insert("relationship_target".to_string());
    }
    labels.into_iter().collect()
}

fn build_project_context_usage_role(
    path: &str,
    changed: bool,
    source_reasons: &[String],
) -> String {
    if changed {
        return "target".to_string();
    }
    if source_reasons.iter().any(|reason| {
        matches!(
            reason.as_str(),
            "root-readme" | "project-rules" | "ai-guide" | "project-kit"
        )
    }) {
        return "repo_rule".to_string();
    }
    match path_category_label(path) {
        "tests" => "test_reference",
        "docs" => "doc_reference",
        "config" => "config_reference",
        _ => "direct_support",
    }
    .to_string()
}

fn build_project_context_priority(
    changed: bool,
    source_reasons: &[String],
    outbound_relationships: &[ProjectContextFileRelationship],
    inbound_relationships: &[ProjectContextFileRelationship],
) -> String {
    if changed {
        return "primary".to_string();
    }
    if source_reasons
        .iter()
        .any(|reason| reason.starts_with("related-"))
        || !outbound_relationships.is_empty()
        || !inbound_relationships.is_empty()
    {
        return "secondary".to_string();
    }
    "background".to_string()
}

fn build_project_context_verification_relevance(
    changed: bool,
    semantic: &ManifestFileSemantic,
    source_reasons: &[String],
    _outbound_relationships: &[ProjectContextFileRelationship],
    _inbound_relationships: &[ProjectContextFileRelationship],
) -> String {
    if changed || is_test_like_semantic_target(semantic, source_reasons) {
        return "primary".to_string();
    }
    if semantic.lockfile
        || semantic.ci_or_tooling
        || source_reasons
            .iter()
            .any(|reason| reason.starts_with("related-config:"))
        || source_reasons.iter().any(|reason| {
            matches!(
                reason.as_str(),
                "project-rules" | "ai-guide" | "project-kit"
            )
        })
    {
        return "supporting".to_string();
    }
    "background".to_string()
}

fn build_project_context_verification_labels(
    changed: bool,
    semantic: &ManifestFileSemantic,
    source_reasons: &[String],
    outbound_relationships: &[ProjectContextFileRelationship],
    inbound_relationships: &[ProjectContextFileRelationship],
) -> Vec<String> {
    let mut labels = BTreeSet::new();
    if changed {
        labels.insert("changed_target".to_string());
    }
    if source_reasons
        .iter()
        .any(|reason| reason.starts_with("related-test:"))
    {
        labels.insert("related_test".to_string());
    }
    if source_reasons
        .iter()
        .any(|reason| reason.starts_with("related-config:"))
        || semantic.lockfile
        || semantic.ci_or_tooling
    {
        labels.insert("config_or_policy".to_string());
    }
    if source_reasons
        .iter()
        .any(|reason| reason.starts_with("related-doc:"))
    {
        labels.insert("docs_alignment".to_string());
    }
    if semantic
        .coarse_labels
        .iter()
        .any(|label| matches!(label.as_str(), "api_surface_like" | "signature_change_like"))
    {
        labels.insert("api_surface".to_string());
    }
    if semantic
        .coarse_labels
        .iter()
        .any(|label| matches!(label.as_str(), "import_churn" | "lockfile_touch"))
    {
        labels.insert("dependency_or_import".to_string());
    }
    if !outbound_relationships.is_empty() || !inbound_relationships.is_empty() {
        labels.insert("relationship_backed".to_string());
    }
    labels.into_iter().collect()
}

fn is_test_like_semantic_target(
    semantic: &ManifestFileSemantic,
    source_reasons: &[String],
) -> bool {
    source_reasons
        .iter()
        .any(|reason| reason.starts_with("related-test:"))
        || semantic
            .coarse_labels
            .iter()
            .any(|label| label == "test_only")
}

fn build_project_context_why_included(changed: bool, source_reasons: &[String]) -> Vec<String> {
    let mut out = BTreeSet::new();
    if changed {
        out.insert("changed_file".to_string());
    }
    for reason in source_reasons {
        let label = if reason.starts_with("related-test:") {
            "related_test"
        } else if reason.starts_with("related-source:") {
            "related_source"
        } else if reason.starts_with("related-doc:") {
            "related_doc"
        } else if reason.starts_with("related-config:") {
            "related_config"
        } else {
            match reason.as_str() {
                "root-readme" => "root_readme",
                "project-rules" => "project_rules",
                "ai-guide" => "ai_guide",
                "project-kit" => "project_kit",
                _ => continue,
            }
        };
        out.insert(label.to_string());
    }
    out.into_iter().collect()
}

fn build_project_context_semantics(
    selected_paths: &BTreeSet<String>,
    changed_file_semantics: &BTreeMap<String, ManifestFileSemantic>,
) -> BTreeMap<String, ManifestFileSemantic> {
    selected_paths
        .iter()
        .map(|path| {
            let semantic = changed_file_semantics
                .get(path)
                .cloned()
                .unwrap_or_else(|| {
                    build_manifest_file_semantic(path, selected_paths, FilePatchClues::default())
                });
            (path.clone(), semantic)
        })
        .collect()
}

fn build_project_context_relationships(
    selected_paths: &BTreeSet<String>,
    file_semantics: &BTreeMap<String, ManifestFileSemantic>,
) -> Vec<ProjectContextRelationship> {
    let mut out = BTreeSet::<(String, String, String)>::new();
    for path in selected_paths {
        let Some(semantic) = file_semantics.get(path) else {
            continue;
        };
        for candidate in &semantic.related_test_candidates {
            if selected_paths.contains(candidate) {
                out.insert((path.clone(), "related-test".to_string(), candidate.clone()));
            }
        }
        for candidate in &semantic.related_source_candidates {
            if selected_paths.contains(candidate) {
                out.insert((
                    path.clone(),
                    "related-source".to_string(),
                    candidate.clone(),
                ));
            }
        }
        for candidate in &semantic.related_doc_candidates {
            if selected_paths.contains(candidate) {
                out.insert((path.clone(), "related-doc".to_string(), candidate.clone()));
            }
        }
        for candidate in &semantic.related_config_candidates {
            if selected_paths.contains(candidate) {
                out.insert((
                    path.clone(),
                    "related-config".to_string(),
                    candidate.clone(),
                ));
            }
        }
    }
    out.into_iter()
        .map(|(from, kind, to)| ProjectContextRelationship { from, kind, to })
        .collect()
}

fn build_project_context_relationship_refs(
    relationships: &[ProjectContextRelationship],
) -> BTreeMap<
    String,
    (
        Vec<ProjectContextFileRelationship>,
        Vec<ProjectContextFileRelationship>,
    ),
> {
    let mut out = BTreeMap::<
        String,
        (
            Vec<ProjectContextFileRelationship>,
            Vec<ProjectContextFileRelationship>,
        ),
    >::new();
    for rel in relationships {
        out.entry(rel.from.clone())
            .or_default()
            .0
            .push(ProjectContextFileRelationship {
                kind: rel.kind.clone(),
                path: rel.to.clone(),
            });
        out.entry(rel.to.clone())
            .or_default()
            .1
            .push(ProjectContextFileRelationship {
                kind: rel.kind.clone(),
                path: rel.from.clone(),
            });
    }
    for (outbound, inbound) in out.values_mut() {
        outbound.sort_by(|a, b| a.kind.cmp(&b.kind).then(a.path.cmp(&b.path)));
        inbound.sort_by(|a, b| a.kind.cmp(&b.kind).then(a.path.cmp(&b.path)));
    }
    out
}

fn render_project_context_manifest(bundle: &ProjectContextBundle) -> Result<String, ExitError> {
    serde_json::to_string_pretty(&bundle.manifest).map_err(|e| {
        ExitError::new(
            EXIT_GENERAL,
            format!("failed to render project context JSON: {e}"),
        )
    })
}

fn render_project_context_md(manifest: &ProjectContextManifest) -> String {
    let mut s = String::new();
    s.push_str("# PROJECT CONTEXT\n\n");
    s.push_str("This file is supplemental context for hosted AI handoff. Patch parts remain canonical.\n\n");
    s.push_str("## Summary\n");
    s.push_str(&format!("- mode: `{}`\n", manifest.mode));
    s.push_str(&format!(
        "- selected files: `{}` (`{}` changed, `{}` supplemental; `{}` snapshot(s), `{}` omitted)\n",
        manifest.summary.selected_files,
        manifest.summary.changed_files,
        manifest.summary.supplemental_files,
        manifest.summary.included_snapshots,
        manifest.summary.omitted_files
    ));
    s.push_str(&format!(
        "- snapshot root: `{}` (`{}` total bytes, `{}` relationship(s))\n",
        manifest.snapshot_root,
        manifest.summary.total_snapshot_bytes,
        manifest.summary.relationship_count
    ));

    s.push_str("\n## Top-level directories\n");
    for dir in &manifest.top_level_dirs {
        s.push_str(&format!("- `{}`: {} file(s)\n", dir.path, dir.file_count));
    }

    if !manifest.summary.categories.is_empty() {
        s.push_str("\n## Category counts\n");
        for (category, count) in &manifest.summary.categories {
            s.push_str(&format!("- `{}`: {} file(s)\n", category, count));
        }
    }

    if !manifest.summary.priority_counts.is_empty() {
        s.push_str("\n## Priority counts\n");
        for (priority, count) in &manifest.summary.priority_counts {
            s.push_str(&format!("- `{}`: {} file(s)\n", priority, count));
        }
    }

    if !manifest.summary.edit_scope_counts.is_empty() {
        s.push_str("\n## Edit-scope counts\n");
        for (scope, count) in &manifest.summary.edit_scope_counts {
            s.push_str(&format!("- `{}`: {} file(s)\n", scope, count));
        }
    }

    if !manifest.summary.verification_relevance_counts.is_empty() {
        s.push_str("\n## Verification relevance counts\n");
        for (relevance, count) in &manifest.summary.verification_relevance_counts {
            s.push_str(&format!("- `{}`: {} file(s)\n", relevance, count));
        }
    }

    if !manifest.summary.relationship_kinds.is_empty() {
        s.push_str("\n## Relationship kinds\n");
        for (kind, count) in &manifest.summary.relationship_kinds {
            s.push_str(&format!("- `{}`: {} relationship(s)\n", kind, count));
        }
    }

    s.push_str("\n## Selected files\n");
    for file in &manifest.files {
        let reasons = if file.source_reasons.is_empty() {
            "-".to_string()
        } else {
            file.source_reasons.join(", ")
        };
        if file.included {
            s.push_str(&format!(
                "- `{}` [{}] changed=`{}` role=`{}` priority=`{}` edit=`{}` verify=`{}` verify-why=`{}` why=`{}` task-groups=`{}` reasons=`{}` context=`{}` snapshot=`{}` language=`{}` labels=`{}` outbound=`{}` inbound=`{}`\n",
                file.path,
                file.category,
                yes_no(file.changed),
                file.usage_role,
                file.priority,
                file.edit_scope_role,
                file.verification_relevance,
                render_string_list_or_dash(&file.verification_labels),
                render_string_list_or_dash(&file.why_included),
                render_string_list_or_dash(&file.task_group_refs),
                reasons,
                file.context_labels.join(","),
                file.snapshot_path.as_deref().unwrap_or("-"),
                file.semantic.language,
                render_semantic_labels(&file.semantic),
                file.outbound_relationships.len(),
                file.inbound_relationships.len()
            ));
        } else {
            s.push_str(&format!(
                "- `{}` [{}] changed=`{}` role=`{}` priority=`{}` edit=`{}` verify=`{}` verify-why=`{}` why=`{}` task-groups=`{}` reasons=`{}` context=`{}` omitted=`{}` language=`{}` labels=`{}` outbound=`{}` inbound=`{}`\n",
                file.path,
                file.category,
                yes_no(file.changed),
                file.usage_role,
                file.priority,
                file.edit_scope_role,
                file.verification_relevance,
                render_string_list_or_dash(&file.verification_labels),
                render_string_list_or_dash(&file.why_included),
                render_string_list_or_dash(&file.task_group_refs),
                reasons,
                file.context_labels.join(","),
                file.omitted_reason.as_deref().unwrap_or("unknown"),
                file.semantic.language,
                render_semantic_labels(&file.semantic),
                file.outbound_relationships.len(),
                file.inbound_relationships.len()
            ));
        }
    }

    if !manifest.relationships.is_empty() {
        s.push_str("\n## Relationships\n");
        for rel in &manifest.relationships {
            s.push_str(&format!(
                "- `{}` --{}--> `{}`\n",
                rel.from, rel.kind, rel.to
            ));
        }
    }

    s
}

fn render_semantic_labels(semantic: &ManifestFileSemantic) -> String {
    if semantic.coarse_labels.is_empty() {
        "-".to_string()
    } else {
        semantic.coarse_labels.join(",")
    }
}

fn render_string_list_or_dash(items: &[String]) -> String {
    if items.is_empty() {
        "-".to_string()
    } else {
        items.join(",")
    }
}

fn project_context_json_path() -> &'static str {
    "project.context.json"
}

fn project_context_md_path() -> &'static str {
    "PROJECT_CONTEXT.md"
}

fn project_context_snapshot_root() -> &'static str {
    "project_context/files"
}

fn render_ai_requests_md(inp: &HandoffManifestInputs<'_>) -> String {
    let mut s = String::new();
    let reading_order_start = if inp.project_context.is_some() { 4 } else { 3 };
    s.push_str("# AI REQUESTS\n\n");
    s.push_str("Use this file as the bundle-local request scaffold when forwarding the handoff to a hosted AI. Patch parts remain canonical.\n\n");
    s.push_str("## Read order\n");
    s.push_str("1. Read `HANDOFF.md` first.\n");
    s.push_str("2. Use the Change Map plus Parts Index to identify the patch parts that matter.\n");
    if inp.project_context.is_some() {
        s.push_str("3. Read `PROJECT_CONTEXT.md` only when you need surrounding repo structure beyond the changed files.\n");
    }
    for (idx, item) in inp.reading_order.iter().enumerate() {
        s.push_str(&format!("{}. {}\n", idx + reading_order_start, item));
    }

    s.push_str("\n## Hard constraints\n");
    s.push_str("- Patch parts remain canonical. Do not replace them with a repo snapshot.\n");
    s.push_str(
        "- Use the exact current workspace head below when emitting a loop-ready patch bundle.\n",
    );
    s.push_str("- If loop-ready output is impossible, ask for the missing SHA or return `MODE: ANALYSIS_ONLY`. Do not fabricate fallback zip formats.\n");
    s.push_str("- If you return plain text edits instead of a loop-ready patch bundle, keep them as plain text. Do not invent a zip unless the user explicitly asked for a non-ops package.\n");
    s.push_str(&format!(
        "- Current workspace HEAD for loop-ready output: `{}`\n",
        inp.head.trim()
    ));

    s.push_str("\n## Requested output modes\n");
    s.push_str("### 1. Analysis only\n");
    s.push_str("- Return `MODE: ANALYSIS_ONLY` plus the reasoning, risks, and a concrete plan.\n");
    s.push_str("- Use this when the task is exploratory, blocked, or missing the exact base SHA required for a loop-ready patch bundle.\n");

    s.push_str("\n### 2. Plain text diffs or file edits\n");
    s.push_str("- Return unified diffs or file-by-file edits directly in text.\n");
    s.push_str("- Keep the response scoped to the files listed in `HANDOFF.md` unless the request explicitly widens scope.\n");

    s.push_str("\n### 3. Ops-compatible patch bundle\n");
    s.push_str("- Return `MODE: OPS_PATCH_BUNDLE` only when you can produce a valid diffship loop-ready patch bundle.\n");
    s.push_str("- The bundle root must contain `manifest.yaml` and `changes/*.patch`.\n");
    s.push_str("- Use repo-relative paths only.\n");
    s.push_str("- Use `base_commit` = the exact current workspace head above.\n");
    s.push_str("- Use `apply_mode` = `git-apply` unless mail patches are explicitly requested.\n");
    s.push_str("- If you emit `git-am`, default the patch mail author to `Diffship <diffship@example.com>` unless the repository says otherwise.\n");

    s.push_str("\n## Bundle context available\n");
    s.push_str(
        "- `handoff.manifest.json` is the canonical machine-readable summary for this bundle.\n",
    );
    if inp.project_context.is_some() {
        s.push_str("- `project.context.json` / `PROJECT_CONTEXT.md` provide bounded supplemental repo context for hosted AI use.\n");
    } else {
        s.push_str("- No supplemental project-context pack is included in this bundle.\n");
    }
    if !inp.attachments.is_empty() {
        s.push_str("- `attachments.zip` contains raw files that were intentionally kept out of patch text.\n");
    }
    if !inp.exclusions.is_empty() {
        s.push_str("- `excluded.md` records files that were intentionally omitted from the patch payload.\n");
    }
    if let Some(project_context) = inp.project_context {
        s.push_str("\n## Focused project-context guidance\n");
        s.push_str(&format!(
            "- selected files: `{}` (`{}` changed, `{}` supplemental; `{}` relationship(s))\n",
            project_context.summary.selected_files,
            project_context.summary.changed_files,
            project_context.summary.supplemental_files,
            project_context.summary.relationship_count
        ));
        s.push_str("- Read changed focused-context files first. Follow only their direct relationships before widening scope.\n");
        s.push_str("- Use `project.context.json` when you need file-by-file `changed`, `usage_role`, `priority`, `edit_scope_role`, `verification_relevance`, `verification_labels`, `why_included`, `task_group_refs`, `context_labels`, `semantic`, `outbound_relationships`, and `inbound_relationships` data.\n");
        s.push_str("- Use `PROJECT_CONTEXT.md` when you want the same focused selection rendered in text.\n");
        for file in project_context
            .files
            .iter()
            .filter(|file| file.changed)
            .take(5)
        {
            let verify = format!(
                "{}:{}",
                file.verification_relevance,
                render_string_list_or_dash(&file.verification_labels)
            );
            s.push_str(&format!(
                "- changed context: `{}` [{}] role=`{}` priority=`{}` edit=`{}` verify=`{}` tasks=`{}` context=`{}` language=`{}` labels=`{}` direct=`{}`\n",
                file.path,
                file.category,
                file.usage_role,
                file.priority,
                file.edit_scope_role,
                verify,
                render_string_list_or_dash(&file.task_group_refs),
                file.context_labels.join(","),
                file.semantic.language,
                render_semantic_labels(&file.semantic),
                render_project_context_ai_relationships(file)
            ));
        }
    }

    if !inp.task_groups.is_empty() {
        s.push_str("\n## Task-group execution order\n");
        s.push_str("- Read each task group's part context before its patch payload, then widen into focused project context only for the listed related files.\n");
        s.push_str("- Treat `risk_hints` as local verification prompts, not as a substitute for `verify`.\n");
        s.push_str("- Treat `review_labels` as generation/review strategy hints: behavioral changes need deeper reasoning, mechanical updates need scope discipline, and verification-heavy tasks need stronger local check follow-up.\n");
        s.push_str("- Use `task_shape_labels` to decide whether a task is single-area or cross-cutting and whether it deserves extra review/verification budget before you start editing.\n");
        s.push_str("- Use `edit_targets` as the bounded write scope for the task and `context_only_files` as read-only context unless the task explicitly broadens scope.\n");
        s.push_str("- Use `verification_targets` as the bounded set of likely tests/config/policy surfaces to inspect before proposing local verification.\n");
        s.push_str("- Use `verification_labels` to keep verification strategy coarse and bounded: distinguish test follow-up, config/policy follow-up, dependency validation, and lightweight sanity checks before you suggest local commands.\n");
        s.push_str("- Use `widening_labels` to decide whether to stay patch-first or widen into related tests/config/docs/repo rules.\n");
        s.push_str("- Use `execution_labels` to keep the execution flow coarse and deterministic: decide whether to stay patch-only, widen before editing, review repo rules first, and bias toward post-edit verification without improvising a wider repo walk.\n");
        for line in render_ai_request_task_groups(inp.task_groups, inp.project_context) {
            s.push_str(&format!("- {line}\n"));
        }
    }

    s.push_str("\n## Patch-part guidance\n");
    s.push_str("- Use `parts/part_XX.context.json` when you need machine-readable part-local facts such as `intent_labels`, `scoped_context`, and per-file semantic hints.\n");
    s.push_str("- Reuse part `review_labels` before editing: they tell you whether to optimize for behavioral reasoning, mechanical consistency, verification follow-up, or policy-sensitive review.\n");
    for line in render_ai_request_part_guidance(inp.parts, inp.rows, inp.file_semantics) {
        s.push_str(&format!("- {line}\n"));
    }

    s
}

fn render_project_context_ai_relationships(file: &ProjectContextFile) -> String {
    let mut items = file
        .outbound_relationships
        .iter()
        .map(|rel| format!("{}:{}", rel.kind, rel.path))
        .collect::<Vec<_>>();
    if items.is_empty() {
        "-".to_string()
    } else {
        items.truncate(6);
        items.join(", ")
    }
}

fn render_ai_request_task_groups(
    task_groups: &[TaskGroupComputed],
    project_context: Option<&ProjectContextManifest>,
) -> Vec<String> {
    task_groups
        .iter()
        .take(8)
        .map(|group| {
            let project = group
                .manifest
                .related_project_files
                .iter()
                .take(4)
                .map(|path| render_ai_request_project_file(path, project_context))
                .collect::<Vec<_>>();
            format!(
                "`{}` primary=`{}` shape=`{}` review=`{}` intents=`{}` risks=`{}` edit=`{}` context=`{}` verify=`{}` verify-strategy=`{}` widen=`{}` execute=`{}` read=`{}` project=`{}`",
                group.manifest.task_id,
                render_string_list_or_dash(&group.manifest.primary_labels),
                render_string_list_or_dash(&group.manifest.task_shape_labels),
                render_string_list_or_dash(&group.manifest.review_labels),
                render_string_list_or_dash(&group.manifest.intent_labels),
                render_string_list_or_dash(&group.manifest.risk_hints),
                render_string_list_or_dash(&group.manifest.edit_targets),
                render_ai_request_context_only_files(
                    &group.manifest.context_only_files,
                    project_context,
                ),
                render_ai_request_verification_targets(
                    &group.manifest.verification_targets,
                    project_context,
                ),
                render_string_list_or_dash(&group.manifest.verification_labels),
                render_string_list_or_dash(&group.manifest.widening_labels),
                render_string_list_or_dash(&group.manifest.execution_labels),
                render_string_list_or_dash(&group.manifest.suggested_read_order),
                if project.is_empty() {
                    "-".to_string()
                } else {
                    project.join(", ")
                }
            )
        })
        .collect()
}

fn render_ai_request_context_only_files(
    paths: &[String],
    project_context: Option<&ProjectContextManifest>,
) -> String {
    if paths.is_empty() {
        return "-".to_string();
    }
    paths
        .iter()
        .take(5)
        .map(|path| render_ai_request_project_file(path, project_context))
        .collect::<Vec<_>>()
        .join(", ")
}

fn render_ai_request_project_file(
    path: &str,
    project_context: Option<&ProjectContextManifest>,
) -> String {
    let Some(project_context) = project_context else {
        return path.to_string();
    };
    if let Some(file) = project_context.files.iter().find(|file| file.path == path) {
        return format!("{}({}/{})", path, file.usage_role, file.priority);
    }
    path.to_string()
}

fn render_ai_request_verification_targets(
    paths: &[String],
    project_context: Option<&ProjectContextManifest>,
) -> String {
    if paths.is_empty() {
        return "-".to_string();
    }
    paths
        .iter()
        .take(5)
        .map(|path| {
            if let Some(project_context) = project_context
                && let Some(file) = project_context.files.iter().find(|file| file.path == *path)
            {
                return format!(
                    "{}({}/{})",
                    path, file.verification_relevance, file.usage_role
                );
            }
            path.clone()
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn render_ai_request_part_guidance(
    parts: &[PartOutput],
    rows: &[FileRow],
    file_semantics: &BTreeMap<String, ManifestFileSemantic>,
) -> Vec<String> {
    parts
        .iter()
        .take(8)
        .map(|part| {
            let part_rows = rows
                .iter()
                .filter(|row| row.part == part.name)
                .collect::<Vec<_>>();
            let intents = build_part_intent_labels(&part_rows, file_semantics);
            let review = build_review_labels_for_rows(&part_rows, file_semantics);
            let files = part_rows
                .iter()
                .map(|row| row.path.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .take(3)
                .collect::<Vec<_>>();
            format!(
                "`parts/{}` context=`{}` review=`{}` intents=`{}` segments=`{}` files=`{}`",
                part.name,
                part_context_path(&part.name),
                if review.is_empty() {
                    "-".to_string()
                } else {
                    review.join(",")
                },
                if intents.is_empty() {
                    "-".to_string()
                } else {
                    intents.join(",")
                },
                if part.segments.is_empty() {
                    "-".to_string()
                } else {
                    part.segments.join(",")
                },
                if files.is_empty() {
                    "-".to_string()
                } else {
                    files.join(",")
                }
            )
        })
        .collect()
}

fn ai_requests_md_path() -> &'static str {
    "AI_REQUESTS.md"
}

fn build_manifest_task_group_details(
    parts: &[PartOutput],
    rows: &[FileRow],
    file_semantics: &BTreeMap<String, ManifestFileSemantic>,
    project_context: Option<&ProjectContextManifest>,
) -> Vec<TaskGroupComputed> {
    #[derive(Default)]
    struct TaskGroupAccum {
        part_ids: BTreeSet<String>,
        segments: BTreeSet<String>,
        files: BTreeSet<String>,
    }

    let mut grouped = BTreeMap::<Vec<String>, TaskGroupAccum>::new();
    for part in parts {
        let part_rows = rows
            .iter()
            .filter(|row| row.part == part.name)
            .collect::<Vec<_>>();
        let intent_labels = build_part_intent_labels(&part_rows, file_semantics);
        let entry = grouped.entry(intent_labels).or_default();
        entry.part_ids.insert(part.name.clone());
        for segment in &part.segments {
            entry.segments.insert(segment.clone());
        }
        for row in part_rows {
            entry.files.insert(row.path.clone());
        }
    }

    grouped
        .into_iter()
        .enumerate()
        .map(|(idx, (intent_labels, accum))| {
            let task_id = format!("task_{:02}", idx + 1);
            let part_ids = accum.part_ids.into_iter().collect::<Vec<_>>();
            let segments = accum.segments.into_iter().collect::<Vec<_>>();
            let files = accum.files;
            let related_context_paths = part_ids
                .iter()
                .map(|part_id| part_context_path(part_id))
                .collect::<Vec<_>>();
            let related_project_files =
                build_task_group_related_project_files(&files, project_context);
            let verification_targets = build_task_group_verification_targets(
                &files,
                &related_project_files,
                file_semantics,
                project_context,
            );
            let review_labels = build_review_labels_for_rows(
                &rows
                    .iter()
                    .filter(|row| part_ids.contains(&row.part))
                    .collect::<Vec<_>>(),
                file_semantics,
            );
            let verification_labels = build_task_group_verification_labels(
                &files,
                &verification_targets,
                &review_labels,
                file_semantics,
                project_context,
            );
            let widening_labels =
                build_task_group_widening_labels(&files, &related_project_files, project_context);
            let manifest = ManifestTaskGroup {
                task_id: task_id.clone(),
                intent_labels: intent_labels.clone(),
                primary_labels: build_task_group_primary_labels(
                    &intent_labels,
                    &files,
                    file_semantics,
                ),
                task_shape_labels: build_task_group_shape_labels(
                    &files,
                    &review_labels,
                    &verification_labels,
                    &widening_labels,
                ),
                edit_targets: files.iter().cloned().collect(),
                context_only_files: related_project_files
                    .iter()
                    .filter(|path| !files.contains(*path))
                    .cloned()
                    .collect(),
                review_labels: review_labels.clone(),
                part_count: part_ids.len(),
                file_count: files.len(),
                part_ids: part_ids.clone(),
                segments,
                top_files: files.iter().take(8).cloned().collect(),
                verification_targets: verification_targets.clone(),
                verification_labels: verification_labels.clone(),
                widening_labels: widening_labels.clone(),
                execution_labels: build_task_group_execution_labels(
                    &review_labels,
                    &verification_labels,
                    &widening_labels,
                ),
                related_context_paths: related_context_paths.clone(),
                related_project_files: related_project_files.iter().cloned().collect(),
                suggested_read_order: build_task_group_suggested_read_order(
                    &related_context_paths,
                    &part_ids,
                    &related_project_files,
                    project_context,
                ),
                risk_hints: build_task_group_risk_hints(rows, &part_ids, file_semantics),
            };
            TaskGroupComputed {
                manifest,
                all_files: files,
                related_project_files,
            }
        })
        .collect()
}

fn build_task_group_primary_labels(
    intent_labels: &[String],
    files: &BTreeSet<String>,
    file_semantics: &BTreeMap<String, ManifestFileSemantic>,
) -> Vec<String> {
    let mut labels = BTreeSet::new();
    for path in files {
        match path_category_label(path) {
            "source" => {
                labels.insert("source_task".to_string());
            }
            "docs" => {
                labels.insert("docs_task".to_string());
            }
            "config" => {
                labels.insert("config_task".to_string());
            }
            "tests" => {
                labels.insert("test_task".to_string());
            }
            _ => {
                labels.insert("other_task".to_string());
            }
        }
        if let Some(semantic) = file_semantics.get(path) {
            for label in &semantic.coarse_labels {
                match label.as_str() {
                    "api_surface_like" => {
                        labels.insert("api_surface_task".to_string());
                    }
                    "import_churn" => {
                        labels.insert("import_heavy_task".to_string());
                    }
                    "generated_output_touch" => {
                        labels.insert("generated_output_task".to_string());
                    }
                    "lockfile_touch" => {
                        labels.insert("dependency_task".to_string());
                    }
                    "ci_or_tooling_touch" => {
                        labels.insert("repo_tooling_task".to_string());
                    }
                    "repo_rule_touch" => {
                        labels.insert("repo_rule_task".to_string());
                    }
                    "dependency_policy_touch" => {
                        labels.insert("dependency_policy_task".to_string());
                    }
                    "build_graph_touch" => {
                        labels.insert("build_graph_task".to_string());
                    }
                    "test_infrastructure_touch" => {
                        labels.insert("test_infra_task".to_string());
                    }
                    _ => {}
                }
            }
        }
    }
    for label in intent_labels {
        match label.as_str() {
            "cross_area_change" => {
                labels.insert("cross_area_task".to_string());
            }
            "rename_or_copy" => {
                labels.insert("rename_task".to_string());
            }
            "reduced_context" => {
                labels.insert("reduced_context_task".to_string());
            }
            _ => {}
        }
    }
    labels.into_iter().collect()
}

fn build_task_group_related_project_files(
    files: &BTreeSet<String>,
    project_context: Option<&ProjectContextManifest>,
) -> BTreeSet<String> {
    let Some(project_context) = project_context else {
        return BTreeSet::new();
    };
    let mut out = BTreeSet::new();
    let selected = project_context
        .files
        .iter()
        .map(|file| file.path.clone())
        .collect::<BTreeSet<_>>();
    for file in files {
        if selected.contains(file) {
            out.insert(file.clone());
        }
    }
    for relationship in &project_context.relationships {
        if files.contains(&relationship.from) && selected.contains(&relationship.to) {
            out.insert(relationship.to.clone());
        }
        if files.contains(&relationship.to) && selected.contains(&relationship.from) {
            out.insert(relationship.from.clone());
        }
    }
    out
}

fn build_task_group_suggested_read_order(
    related_context_paths: &[String],
    part_ids: &[String],
    related_project_files: &BTreeSet<String>,
    project_context: Option<&ProjectContextManifest>,
) -> Vec<String> {
    let mut items = Vec::new();
    items.extend(related_context_paths.iter().cloned());
    items.extend(part_ids.iter().map(|part_id| format!("parts/{part_id}")));
    if let Some(project_context) = project_context {
        for path in related_project_files {
            if let Some(snapshot_path) = project_context
                .files
                .iter()
                .find(|file| file.path == *path)
                .and_then(|file| file.snapshot_path.clone())
            {
                items.push(snapshot_path);
            }
        }
    }
    items
}

fn build_task_group_risk_hints(
    rows: &[FileRow],
    part_ids: &[String],
    file_semantics: &BTreeMap<String, ManifestFileSemantic>,
) -> Vec<String> {
    let part_ids = part_ids.iter().cloned().collect::<BTreeSet<_>>();
    let mut hints = BTreeSet::new();
    for row in rows.iter().filter(|row| part_ids.contains(&row.part)) {
        let change_hints = build_manifest_file_change_hints(row);
        if change_hints.reduced_context {
            hints.insert("reduced_context".to_string());
        }
        if change_hints.stored_as_attachment {
            hints.insert("attachment_routed".to_string());
        }
        if change_hints.excluded {
            hints.insert("excluded_entry".to_string());
        }
        if let Some(semantic) = file_semantics.get(&row.path) {
            if semantic.lockfile {
                hints.insert("lockfile_touch".to_string());
            }
            if semantic.ci_or_tooling {
                hints.insert("ci_or_tooling_touch".to_string());
            }
            if semantic.generated_like {
                hints.insert("generated_output_touch".to_string());
            }
        }
    }
    hints.into_iter().collect()
}

fn build_task_group_verification_targets(
    files: &BTreeSet<String>,
    related_project_files: &BTreeSet<String>,
    file_semantics: &BTreeMap<String, ManifestFileSemantic>,
    project_context: Option<&ProjectContextManifest>,
) -> Vec<String> {
    let mut targets = BTreeSet::new();
    for path in files {
        let category = path_category_label(path);
        if let Some(semantic) = file_semantics.get(path) {
            let has_api_like = semantic.coarse_labels.iter().any(|label| {
                matches!(label.as_str(), "api_surface_like" | "signature_change_like")
            });
            let has_import_like = semantic
                .coarse_labels
                .iter()
                .any(|label| label == "import_churn");
            if matches!(category, "source" | "tests" | "config")
                || semantic.lockfile
                || semantic.ci_or_tooling
                || has_api_like
                || has_import_like
                || !semantic.related_test_candidates.is_empty()
                || !semantic.related_config_candidates.is_empty()
            {
                targets.insert(path.clone());
            }
            for candidate in &semantic.related_test_candidates {
                targets.insert(candidate.clone());
            }
            for candidate in &semantic.related_config_candidates {
                targets.insert(candidate.clone());
            }
        }
    }
    if let Some(project_context) = project_context {
        for path in related_project_files {
            if let Some(file) = project_context.files.iter().find(|file| file.path == *path)
                && file.verification_relevance != "background"
            {
                targets.insert(path.clone());
            }
        }
    }
    targets.into_iter().collect()
}

fn build_task_group_verification_labels(
    files: &BTreeSet<String>,
    verification_targets: &[String],
    review_labels: &[String],
    file_semantics: &BTreeMap<String, ManifestFileSemantic>,
    project_context: Option<&ProjectContextManifest>,
) -> Vec<String> {
    let mut labels = BTreeSet::new();
    let has_changed_tests = files
        .iter()
        .any(|path| path_category_label(path) == "tests");
    let has_target_tests = verification_targets
        .iter()
        .any(|path| path_category_label(path) == "tests" || is_test_like_path(path));
    let has_target_config = verification_targets.iter().any(|path| {
        path_category_label(path) == "config"
            || file_semantics
                .get(path)
                .is_some_and(|semantic| semantic.lockfile || semantic.ci_or_tooling)
    });
    let has_target_policy = project_context.is_some_and(|project_context| {
        verification_targets.iter().any(|path| {
            project_context
                .files
                .iter()
                .find(|file| file.path == *path)
                .is_some_and(|file| {
                    file.verification_labels.iter().any(|label| {
                        matches!(label.as_str(), "config_or_policy" | "relationship_backed")
                    })
                })
        })
    });

    if has_target_tests {
        labels.insert("test_follow_up".to_string());
    }
    if has_target_config {
        labels.insert("config_follow_up".to_string());
    }
    if has_target_policy {
        labels.insert("policy_follow_up".to_string());
    }
    if review_labels
        .iter()
        .any(|label| label == "behavioral_change_like")
    {
        labels.insert("behavioral_regression_watch".to_string());
    }
    if review_labels
        .iter()
        .any(|label| label == "dependency_or_import_review")
    {
        labels.insert("dependency_validation".to_string());
    }
    if review_labels
        .iter()
        .any(|label| label == "mechanical_update_like")
        && !review_labels
            .iter()
            .any(|label| label == "behavioral_change_like")
    {
        labels.insert("sanity_check_first".to_string());
    }
    if !has_changed_tests && has_target_tests {
        labels.insert("needs_targeted_test_read".to_string());
    }

    labels.into_iter().collect()
}

fn build_task_group_widening_labels(
    files: &BTreeSet<String>,
    related_project_files: &BTreeSet<String>,
    project_context: Option<&ProjectContextManifest>,
) -> Vec<String> {
    let Some(project_context) = project_context else {
        return vec!["patch_only".to_string()];
    };

    let mut labels = BTreeSet::new();
    for path in related_project_files {
        if files.contains(path) {
            continue;
        }
        if let Some(file) = project_context.files.iter().find(|file| file.path == *path) {
            match file.usage_role.as_str() {
                "test_reference" => {
                    labels.insert("read_related_tests".to_string());
                }
                "config_reference" => {
                    labels.insert("read_related_config".to_string());
                }
                "doc_reference" => {
                    labels.insert("read_related_docs".to_string());
                }
                "repo_rule" => {
                    labels.insert("read_repo_rules".to_string());
                }
                "direct_support" => {
                    labels.insert("read_direct_support".to_string());
                }
                _ => {}
            }
        }
    }

    if labels.is_empty() {
        labels.insert("patch_only".to_string());
    }

    labels.into_iter().collect()
}

fn build_task_group_execution_labels(
    review_labels: &[String],
    verification_labels: &[String],
    widening_labels: &[String],
) -> Vec<String> {
    let mut labels = BTreeSet::new();
    if widening_labels.iter().any(|label| label == "patch_only") {
        labels.insert("patch_only_flow".to_string());
    } else {
        labels.insert("widen_before_edit".to_string());
    }
    if widening_labels
        .iter()
        .any(|label| label == "read_repo_rules")
    {
        labels.insert("rules_before_edit".to_string());
    }
    if review_labels
        .iter()
        .any(|label| label == "behavioral_change_like")
    {
        labels.insert("behavior_first".to_string());
    } else if review_labels
        .iter()
        .any(|label| label == "mechanical_update_like")
    {
        labels.insert("mechanical_first".to_string());
    }
    if !verification_labels.is_empty() {
        labels.insert("verify_after_edit".to_string());
    }
    if verification_labels.iter().any(|label| {
        matches!(
            label.as_str(),
            "test_follow_up" | "needs_targeted_test_read"
        )
    }) {
        labels.insert("check_tests_after_edit".to_string());
    }
    if verification_labels
        .iter()
        .any(|label| matches!(label.as_str(), "config_follow_up" | "policy_follow_up"))
    {
        labels.insert("check_config_after_edit".to_string());
    }
    if verification_labels
        .iter()
        .any(|label| label == "dependency_validation")
    {
        labels.insert("check_dependencies_after_edit".to_string());
    }
    labels.into_iter().collect()
}

fn build_task_group_shape_labels(
    files: &BTreeSet<String>,
    review_labels: &[String],
    verification_labels: &[String],
    widening_labels: &[String],
) -> Vec<String> {
    let mut labels = BTreeSet::new();
    let category_count = files
        .iter()
        .map(|path| path_category_label(path).to_string())
        .collect::<BTreeSet<_>>()
        .len();
    let has_context_widening = widening_labels.iter().any(|label| label != "patch_only");
    if category_count > 1 || has_context_widening {
        labels.insert("cross_cutting".to_string());
    } else {
        labels.insert("single_area".to_string());
    }
    if review_labels.len() >= 2
        || review_labels.iter().any(|label| {
            matches!(
                label.as_str(),
                "behavioral_change_like" | "repo_policy_touch" | "verification_surface_touch"
            )
        })
    {
        labels.insert("review_heavy".to_string());
    }
    if verification_labels.len() >= 2
        || verification_labels
            .iter()
            .any(|label| label != "sanity_check_first")
    {
        labels.insert("verification_heavy".to_string());
    }
    labels.into_iter().collect()
}

fn enrich_project_context_with_task_groups(
    manifest: &mut ProjectContextManifest,
    task_groups: &[TaskGroupComputed],
) {
    let mut refs = BTreeMap::<String, BTreeSet<String>>::new();
    for group in task_groups {
        for path in group
            .all_files
            .iter()
            .chain(group.related_project_files.iter())
        {
            refs.entry(path.clone())
                .or_default()
                .insert(group.manifest.task_id.clone());
        }
    }

    for file in &mut manifest.files {
        file.task_group_refs = refs
            .remove(&file.path)
            .unwrap_or_default()
            .into_iter()
            .collect();
        file.edit_scope_role = build_project_context_edit_scope_role(file, task_groups);
    }
    manifest.summary = build_project_context_summary(
        &manifest.files,
        &manifest.relationships,
        manifest.summary.total_snapshot_bytes,
    );
}

fn build_project_context_edit_scope_role(
    file: &ProjectContextFile,
    task_groups: &[TaskGroupComputed],
) -> String {
    let is_edit_target = task_groups.iter().any(|group| {
        group
            .manifest
            .edit_targets
            .iter()
            .any(|path| path == &file.path)
    });
    if is_edit_target || file.changed {
        return "write_target".to_string();
    }

    let is_context_only = task_groups.iter().any(|group| {
        group
            .manifest
            .context_only_files
            .iter()
            .any(|path| path == &file.path)
    });
    if is_context_only {
        if file.usage_role == "repo_rule" {
            return "read_only_rule".to_string();
        }
        if file.verification_relevance != "background" || file.usage_role == "test_reference" {
            return "read_only_verification".to_string();
        }
    }

    "read_only_context".to_string()
}

fn render_handoff_manifest(inp: &HandoffManifestInputs<'_>) -> Result<String, ExitError> {
    let reduced_context_paths = reduced_context_paths(inp.rows);
    let all_rows = inp.rows.iter().collect::<Vec<_>>();
    let manifest = HandoffManifest {
        schema_version: 1,
        patch_canonical: true,
        entrypoint: "HANDOFF.md".to_string(),
        current_head: inp.head.trim().to_string(),
        sources: ManifestSources {
            committed: inp.sources.include_committed,
            staged: inp.sources.include_staged,
            unstaged: inp.sources.include_unstaged,
            untracked: inp.sources.include_untracked,
            split_by: split_label(inp.split_by).to_string(),
            untracked_mode: untracked_label(inp.untracked_mode).to_string(),
            include_binary: inp.binary_policy.include_binary,
            binary_mode: binary_mode_label(inp.binary_policy.binary_mode).to_string(),
        },
        committed_range: inp.plan.map(|plan| ManifestCommittedRange {
            mode: range_mode_label(plan.mode).to_string(),
            base: plan.base.clone(),
            target: plan.target.clone(),
            from_rev: plan.from_rev.clone(),
            to_rev: plan.to_rev.clone(),
            a_rev: plan.a_rev.clone(),
            b_rev: plan.b_rev.clone(),
            merge_base: plan.merge_base.clone(),
            commit_count: plan.commit_count,
        }),
        filters: ManifestFilters {
            diffshipignore: inp.ignore_enabled,
            include: inp.include_patterns.to_vec(),
            exclude: inp.exclude_patterns.to_vec(),
        },
        packing: ManifestPacking {
            profile: inp.packing_limits.profile_label.clone(),
            max_parts: inp.packing_limits.max_parts,
            max_bytes_per_part: inp.packing_limits.max_bytes_per_part,
            reduced_context_paths: reduced_context_paths.clone(),
        },
        warnings: ManifestWarnings {
            reduced_context_count: reduced_context_paths.len(),
            exclusion_count: inp.exclusions.len(),
            secret_hit_count: inp.secret_hits.len(),
        },
        summary: ManifestSummary {
            file_count: all_rows.len(),
            part_count: inp.parts.len(),
            commit_view_count: inp.commit_views.len(),
            categories: count_part_categories(&all_rows),
            segments: count_row_labels(all_rows.iter().copied(), |row| row.segment.clone()),
            statuses: count_row_labels(all_rows.iter().copied(), |row| row.status.clone()),
        },
        reading_order: inp.reading_order.to_vec(),
        artifacts: ManifestArtifacts {
            handoff_md: "HANDOFF.md".to_string(),
            manifest_json: "handoff.manifest.json".to_string(),
            context_xml: handoff_context_xml_path().to_string(),
            ai_requests_md: ai_requests_md_path().to_string(),
            part_paths: inp
                .parts
                .iter()
                .map(|part| format!("parts/{}", part.name))
                .collect(),
            project_context_json: inp
                .project_context
                .map(|_| project_context_json_path().to_string()),
            project_context_md: inp
                .project_context
                .map(|_| project_context_md_path().to_string()),
            project_context_snapshot_root: inp
                .project_context
                .map(|_| project_context_snapshot_root().to_string()),
            attachments_zip: (!inp.attachments.is_empty()).then(|| "attachments.zip".to_string()),
            excluded_md: (!inp.exclusions.is_empty()).then(|| "excluded.md".to_string()),
            secrets_md: (!inp.secret_hits.is_empty()).then(|| "secrets.md".to_string()),
        },
        parts: inp
            .parts
            .iter()
            .map(|part| {
                let mut part_files = inp
                    .rows
                    .iter()
                    .filter(|row| row.part == part.name)
                    .map(|row| row.path.clone())
                    .collect::<Vec<_>>();
                part_files.sort();
                part_files.dedup();
                let mut part_reduced = inp
                    .rows
                    .iter()
                    .filter(|row| row.part == part.name && row_has_reduced_context(row))
                    .map(|row| row.path.clone())
                    .collect::<Vec<_>>();
                part_reduced.sort();
                part_reduced.dedup();
                ManifestPart {
                    part_id: part.name.clone(),
                    patch_path: format!("parts/{}", part.name),
                    context_path: part_context_path(&part.name),
                    segments: part.segments.clone(),
                    approx_bytes: part.patch.len() as u64,
                    file_count: part_files.len(),
                    first_files: part_files.iter().take(5).cloned().collect(),
                    reduced_context_paths: part_reduced,
                }
            })
            .collect(),
        task_groups: inp
            .task_groups
            .iter()
            .map(|group| group.manifest.clone())
            .collect(),
        files: inp
            .rows
            .iter()
            .map(|row| ManifestFile {
                category: path_category_label(&row.path).to_string(),
                segment: row.segment.clone(),
                status: row.status.clone(),
                path: row.path.clone(),
                ins: row.ins,
                del: row.del,
                bytes: row.bytes,
                part: row_part_name(row),
                note: nonempty(row.note.trim()),
                change_hints: build_manifest_file_change_hints(row),
                semantic: file_semantic_for_row(inp.file_semantics, row),
            })
            .collect(),
        commit_views: inp
            .commit_views
            .iter()
            .map(|view| ManifestCommitView {
                hash7: view.hash7.clone(),
                subject: view.subject.clone(),
                date: view.date.clone(),
                files: view
                    .files
                    .iter()
                    .map(|(path, part)| ManifestCommitFile {
                        path: path.clone(),
                        part: part.clone(),
                    })
                    .collect(),
                ins: view.ins,
                del: view.del,
            })
            .collect(),
        attachments: sorted_manifest_attachments(inp.attachments),
        exclusions: sorted_manifest_exclusions(inp.exclusions),
        secret_hits: sorted_manifest_secret_hits(inp.secret_hits),
    };

    serde_json::to_string_pretty(&manifest)
        .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to render manifest JSON: {e}")))
}

fn render_handoff_context_xml(inp: &HandoffManifestInputs<'_>) -> String {
    let reduced_context_paths = reduced_context_paths(inp.rows);
    let mut s = String::new();
    s.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    s.push_str(
        "<handoff-context schema-version=\"1\" rendered-from=\"handoff.manifest.json\" patch-canonical=\"true\">\n",
    );
    s.push_str(&format!(
        "  <entrypoint path=\"{}\" />\n",
        xml_escape("HANDOFF.md")
    ));
    s.push_str(&format!(
        "  <rendered-view path=\"{}\" />\n",
        xml_escape(handoff_context_xml_path())
    ));
    s.push_str(&format!(
        "  <current-head>{}</current-head>\n",
        xml_escape(inp.head.trim())
    ));
    s.push_str(&format!(
        "  <sources committed=\"{}\" staged=\"{}\" unstaged=\"{}\" untracked=\"{}\" split-by=\"{}\" untracked-mode=\"{}\" include-binary=\"{}\" binary-mode=\"{}\" />\n",
        inp.sources.include_committed,
        inp.sources.include_staged,
        inp.sources.include_unstaged,
        inp.sources.include_untracked,
        xml_escape(split_label(inp.split_by)),
        xml_escape(untracked_label(inp.untracked_mode)),
        inp.binary_policy.include_binary,
        xml_escape(binary_mode_label(inp.binary_policy.binary_mode)),
    ));
    if let Some(plan) = inp.plan {
        s.push_str(&format!(
            "  <committed-range mode=\"{}\" base=\"{}\" target=\"{}\"",
            xml_escape(range_mode_label(plan.mode)),
            xml_escape(&plan.base),
            xml_escape(&plan.target),
        ));
        if let Some(from_rev) = &plan.from_rev {
            s.push_str(&format!(" from=\"{}\"", xml_escape(from_rev)));
        }
        if let Some(to_rev) = &plan.to_rev {
            s.push_str(&format!(" to=\"{}\"", xml_escape(to_rev)));
        }
        if let Some(a_rev) = &plan.a_rev {
            s.push_str(&format!(" a=\"{}\"", xml_escape(a_rev)));
        }
        if let Some(b_rev) = &plan.b_rev {
            s.push_str(&format!(" b=\"{}\"", xml_escape(b_rev)));
        }
        if let Some(merge_base) = &plan.merge_base {
            s.push_str(&format!(" merge-base=\"{}\"", xml_escape(merge_base)));
        }
        if let Some(commit_count) = plan.commit_count {
            s.push_str(&format!(" commit-count=\"{}\"", commit_count));
        }
        s.push_str(" />\n");
    }
    s.push_str(&format!(
        "  <filters diffshipignore=\"{}\">\n",
        inp.ignore_enabled
    ));
    for pattern in inp.include_patterns {
        s.push_str(&format!("    <include>{}</include>\n", xml_escape(pattern)));
    }
    for pattern in inp.exclude_patterns {
        s.push_str(&format!("    <exclude>{}</exclude>\n", xml_escape(pattern)));
    }
    s.push_str("  </filters>\n");
    s.push_str(&format!(
        "  <packing profile=\"{}\" max-parts=\"{}\" max-bytes-per-part=\"{}\" reduced-context-count=\"{}\">\n",
        xml_escape(&inp.packing_limits.profile_label),
        inp.packing_limits.max_parts,
        inp.packing_limits.max_bytes_per_part,
        reduced_context_paths.len(),
    ));
    for path in &reduced_context_paths {
        s.push_str(&format!(
            "    <reduced-context-path>{}</reduced-context-path>\n",
            xml_escape(path)
        ));
    }
    s.push_str("  </packing>\n");
    s.push_str("  <artifacts>\n");
    s.push_str("    <artifact path=\"HANDOFF.md\" kind=\"handoff\" />\n");
    s.push_str("    <artifact path=\"handoff.manifest.json\" kind=\"manifest-json\" />\n");
    s.push_str(&format!(
        "    <artifact path=\"{}\" kind=\"rendered-context-xml\" />\n",
        xml_escape(handoff_context_xml_path())
    ));
    s.push_str(&format!(
        "    <artifact path=\"{}\" kind=\"ai-requests\" />\n",
        xml_escape(ai_requests_md_path())
    ));
    if inp.project_context.is_some() {
        s.push_str(&format!(
            "    <artifact path=\"{}\" kind=\"project-context-json\" />\n",
            xml_escape(project_context_json_path())
        ));
        s.push_str(&format!(
            "    <artifact path=\"{}\" kind=\"project-context-md\" />\n",
            xml_escape(project_context_md_path())
        ));
        s.push_str(&format!(
            "    <artifact path=\"{}\" kind=\"project-context-snapshots\" />\n",
            xml_escape(project_context_snapshot_root())
        ));
    }
    for part in inp.parts {
        s.push_str(&format!(
            "    <artifact path=\"parts/{}\" kind=\"patch\" />\n",
            xml_escape(&part.name)
        ));
        s.push_str(&format!(
            "    <artifact path=\"{}\" kind=\"part-context-json\" />\n",
            xml_escape(&part_context_path(&part.name))
        ));
    }
    if !inp.attachments.is_empty() {
        s.push_str("    <artifact path=\"attachments.zip\" kind=\"attachments\" />\n");
    }
    if !inp.exclusions.is_empty() {
        s.push_str("    <artifact path=\"excluded.md\" kind=\"excluded\" />\n");
    }
    if !inp.secret_hits.is_empty() {
        s.push_str("    <artifact path=\"secrets.md\" kind=\"secrets\" />\n");
    }
    s.push_str("  </artifacts>\n");
    s.push_str(&format!(
        "  <warnings exclusion-count=\"{}\" secret-hit-count=\"{}\" attachment-count=\"{}\" />\n",
        inp.exclusions.len(),
        inp.secret_hits.len(),
        inp.attachments.len(),
    ));
    s.push_str("  <parts>\n");
    for part in inp.parts {
        let part_rows = inp
            .rows
            .iter()
            .filter(|row| row.part == part.name)
            .collect::<Vec<_>>();
        let category_counts = count_part_categories(&part_rows);
        let file_paths = part_rows
            .iter()
            .map(|row| row.path.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let reduced_part_paths = part_rows
            .iter()
            .filter(|row| row_has_reduced_context(row))
            .map(|row| row.path.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        s.push_str(&format!(
            "    <part id=\"{}\" patch-path=\"{}\" context-path=\"{}\" approx-bytes=\"{}\" file-count=\"{}\">\n",
            xml_escape(&part.name),
            xml_escape(&format!("parts/{}", part.name)),
            xml_escape(&part_context_path(&part.name)),
            part.patch.len(),
            file_paths.len(),
        ));
        s.push_str(&format!(
            "      <title>{}</title>\n",
            xml_escape(&part_context_title(part, &file_paths, &category_counts))
        ));
        s.push_str(&format!(
            "      <summary>{}</summary>\n",
            xml_escape(&part_context_summary(
                &part.segments,
                &category_counts,
                file_paths.len()
            ))
        ));
        s.push_str(&format!(
            "      <intent>{}</intent>\n",
            xml_escape(&part_context_intent(&category_counts))
        ));
        s.push_str("      <segments>\n");
        for segment in &part.segments {
            s.push_str(&format!(
                "        <segment>{}</segment>\n",
                xml_escape(segment)
            ));
        }
        s.push_str("      </segments>\n");
        s.push_str("      <files>\n");
        for path in &file_paths {
            s.push_str(&format!("        <file path=\"{}\" />\n", xml_escape(path)));
        }
        s.push_str("      </files>\n");
        if !reduced_part_paths.is_empty() {
            s.push_str("      <reduced-context-paths>\n");
            for path in &reduced_part_paths {
                s.push_str(&format!("        <path>{}</path>\n", xml_escape(path)));
            }
            s.push_str("      </reduced-context-paths>\n");
        }
        s.push_str("    </part>\n");
    }
    s.push_str("  </parts>\n");
    s.push_str("</handoff-context>\n");
    s
}

fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(ch),
        }
    }
    out
}

fn split_label(v: SplitBy) -> &'static str {
    match v {
        SplitBy::File => "file",
        SplitBy::Commit => "commit",
    }
}

fn untracked_label(v: UntrackedMode) -> &'static str {
    match v {
        UntrackedMode::Auto => "auto",
        UntrackedMode::Patch => "patch",
        UntrackedMode::Raw => "raw",
        UntrackedMode::Meta => "meta",
    }
}

fn yes_no(v: bool) -> &'static str {
    if v { "yes" } else { "no" }
}

fn binary_mode_label(v: BinaryMode) -> &'static str {
    match v {
        BinaryMode::Raw => "raw",
        BinaryMode::Patch => "patch",
        BinaryMode::Meta => "meta",
    }
}

fn range_mode_label(v: RangeMode) -> &'static str {
    match v {
        RangeMode::Direct => "direct",
        RangeMode::MergeBase => "merge-base",
        RangeMode::Last => "last",
        RangeMode::Root => "root",
    }
}

fn row_has_reduced_context(row: &FileRow) -> bool {
    row.note
        .contains("packing fallback reduced diff context to U")
}

fn reduced_context_paths(rows: &[FileRow]) -> Vec<String> {
    rows.iter()
        .filter(|row| row_has_reduced_context(row))
        .map(|row| row.path.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn row_part_name(row: &FileRow) -> Option<String> {
    match row.part.trim() {
        "" | "-" => None,
        other => Some(other.to_string()),
    }
}

fn nonempty(s: &str) -> Option<String> {
    if s.is_empty() {
        None
    } else {
        Some(s.to_string())
    }
}

fn sorted_manifest_attachments(entries: &[AttachmentEntry]) -> Vec<ManifestAttachment> {
    let mut items = entries
        .iter()
        .map(|entry| ManifestAttachment {
            path: entry.zip_path.clone(),
            reason: entry.reason.clone(),
            byte_len: entry.bytes.len(),
        })
        .collect::<Vec<_>>();
    items.sort_by(|a, b| a.path.cmp(&b.path).then(a.reason.cmp(&b.reason)));
    items
}

fn sorted_manifest_exclusions(entries: &[ExclusionEntry]) -> Vec<ManifestExclusion> {
    let mut items = entries
        .iter()
        .map(|entry| ManifestExclusion {
            path: entry.path.clone(),
            reason: entry.reason.clone(),
            guidance: entry.guidance.clone(),
        })
        .collect::<Vec<_>>();
    items.sort_by(|a, b| a.path.cmp(&b.path).then(a.reason.cmp(&b.reason)));
    items
}

fn sorted_manifest_secret_hits(entries: &[SecretHit]) -> Vec<ManifestSecretHit> {
    let mut items = entries
        .iter()
        .map(|entry| ManifestSecretHit {
            path: entry.path.clone(),
            reason: entry.reason.clone(),
        })
        .collect::<Vec<_>>();
    items.sort_by(|a, b| a.path.cmp(&b.path).then(a.reason.cmp(&b.reason)));
    items
}

fn render_part_contexts(
    parts: &[PartOutput],
    rows: &[FileRow],
    attachments: &[AttachmentEntry],
    exclusions: &[ExclusionEntry],
    secret_hits: &[SecretHit],
    file_semantics: &BTreeMap<String, ManifestFileSemantic>,
    task_groups: &[TaskGroupComputed],
) -> Result<Vec<(String, String)>, ExitError> {
    let mut rendered = Vec::with_capacity(parts.len());
    for part in parts {
        let part_rows = rows
            .iter()
            .filter(|row| row.part == part.name)
            .collect::<Vec<_>>();
        let category_counts = count_part_categories(&part_rows);
        let reduced_context_paths = part_rows
            .iter()
            .filter(|row| row_has_reduced_context(row))
            .map(|row| row.path.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let file_paths = part_rows
            .iter()
            .map(|row| row.path.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let task_group = task_groups
            .iter()
            .find(|group| {
                group
                    .manifest
                    .part_ids
                    .iter()
                    .any(|part_id| part_id == &part.name)
            })
            .ok_or_else(|| {
                ExitError::new(
                    EXIT_GENERAL,
                    format!("failed to find task group for part {}", part.name),
                )
            })?;
        let scoped_context = build_part_scoped_context(part, &part_rows, file_semantics);
        let context = PartContext {
            schema_version: 1,
            patch_canonical: true,
            part_id: part.name.clone(),
            patch_path: format!("parts/{}", part.name),
            context_path: part_context_path(&part.name),
            task_group_ref: task_group.manifest.task_id.clone(),
            task_shape_labels: task_group.manifest.task_shape_labels.clone(),
            task_edit_targets: task_group.manifest.edit_targets.clone(),
            task_context_only_files: task_group.manifest.context_only_files.clone(),
            title: part_context_title(part, &file_paths, &category_counts),
            summary: part_context_summary(&part.segments, &category_counts, file_paths.len()),
            intent: part_context_intent(&category_counts),
            intent_labels: build_part_intent_labels(&part_rows, file_semantics),
            review_labels: build_review_labels_for_rows(&part_rows, file_semantics),
            segments: part.segments.clone(),
            files: part_rows
                .iter()
                .map(|row| ManifestFile {
                    category: path_category_label(&row.path).to_string(),
                    segment: row.segment.clone(),
                    status: row.status.clone(),
                    path: row.path.clone(),
                    ins: row.ins,
                    del: row.del,
                    bytes: row.bytes,
                    part: row_part_name(row),
                    note: nonempty(row.note.trim()),
                    change_hints: build_manifest_file_change_hints(row),
                    semantic: file_semantic_for_row(file_semantics, row),
                })
                .collect(),
            scoped_context,
            diff_stats: PartContextDiffStats {
                file_count: file_paths.len(),
                additions: sum_opt(part_rows.iter().map(|row| row.ins)),
                deletions: sum_opt(part_rows.iter().map(|row| row.del)),
                categories: category_counts,
                segments: count_row_labels(part_rows.iter().copied(), |row| row.segment.clone()),
                statuses: count_row_labels(part_rows.iter().copied(), |row| row.status.clone()),
            },
            scope: PartContextScope {
                in_scope: file_paths.iter().take(8).cloned().collect(),
                out_of_scope: vec![
                    "Files not mapped to this patch part remain out of scope.".to_string(),
                    "Use HANDOFF.md and handoff.manifest.json to understand neighboring parts before widening scope.".to_string(),
                ],
            },
            constraints: PartContextConstraints {
                handoff_entrypoint: "HANDOFF.md".to_string(),
                manifest_path: "handoff.manifest.json".to_string(),
                patch_canonical: true,
                reduced_context: !reduced_context_paths.is_empty(),
            },
            warnings: PartContextWarnings {
                reduced_context_paths: reduced_context_paths.clone(),
                bundle_has_attachments: !attachments.is_empty(),
                bundle_has_exclusions: !exclusions.is_empty(),
                bundle_has_secret_warnings: !secret_hits.is_empty(),
            },
            acceptance_criteria: part_context_acceptance_criteria(
                part,
                &file_paths,
                !reduced_context_paths.is_empty(),
            ),
        };
        let contents = serde_json::to_string_pretty(&context).map_err(|e| {
            ExitError::new(
                EXIT_GENERAL,
                format!("failed to render part context JSON: {e}"),
            )
        })?;
        rendered.push((part_context_path(&part.name), contents));
    }
    Ok(rendered)
}

fn part_context_path(part_name: &str) -> String {
    let stem = part_name.strip_suffix(".patch").unwrap_or(part_name);
    format!("parts/{stem}.context.json")
}

fn handoff_context_xml_path() -> &'static str {
    "handoff.context.xml"
}

fn file_semantic_for_row(
    semantics: &BTreeMap<String, ManifestFileSemantic>,
    row: &FileRow,
) -> ManifestFileSemantic {
    semantics.get(&row.path).cloned().unwrap_or_else(|| {
        build_manifest_file_semantic(&row.path, &BTreeSet::new(), FilePatchClues::default())
    })
}

fn build_manifest_file_change_hints(row: &FileRow) -> ManifestFileChangeHints {
    let previous_path = row.note.trim().strip_prefix("from ").map(ToOwned::to_owned);
    ManifestFileChangeHints {
        new_file: row.status == "A" && previous_path.is_none(),
        deleted_file: row.status == "D",
        rename_or_copy: previous_path.is_some(),
        previous_path,
        stored_as_attachment: row.part == "attachments.zip"
            || row.note.contains("stored in attachments.zip"),
        excluded: row.part == "-" || row.note.contains("see excluded.md"),
        reduced_context: row_has_reduced_context(row),
    }
}

fn build_file_semantics(
    git_root: &Path,
    rows: &[FileRow],
    parts: &[PartOutput],
) -> Result<BTreeMap<String, ManifestFileSemantic>, ExitError> {
    let candidate_paths = related_test_candidate_pool(git_root, rows)?;
    let patch_clues = collect_file_patch_clues(parts);
    let unique_paths = rows
        .iter()
        .map(|row| row.path.clone())
        .collect::<BTreeSet<_>>();
    Ok(unique_paths
        .into_iter()
        .map(|path| {
            let semantic = build_manifest_file_semantic(
                &path,
                &candidate_paths,
                patch_clues.get(&path).copied().unwrap_or_default(),
            );
            (path, semantic)
        })
        .collect())
}

fn related_test_candidate_pool(
    git_root: &Path,
    rows: &[FileRow],
) -> Result<BTreeSet<String>, ExitError> {
    let tracked = git::run_git(git_root, ["ls-files"])?;
    let mut paths = tracked
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect::<BTreeSet<_>>();
    for row in rows {
        paths.insert(row.path.clone());
    }
    Ok(paths)
}

fn build_manifest_file_semantic(
    path: &str,
    candidate_paths: &BTreeSet<String>,
    patch_clues: FilePatchClues,
) -> ManifestFileSemantic {
    let generated_like = is_generated_like_path(path);
    let lockfile = is_lockfile_path(path);
    let ci_or_tooling = is_ci_or_tooling_path(path);
    let repo_rule = is_repo_rule_path(path);
    let dependency_policy = is_dependency_policy_path(path);
    let build_graph = is_build_graph_path(path);
    let test_infrastructure = is_test_infrastructure_path(path);
    let category = path_category_label(path);
    let mut coarse_labels = BTreeSet::new();
    if category == "docs" {
        coarse_labels.insert("docs_only".to_string());
    }
    if category == "config" {
        coarse_labels.insert("config_only".to_string());
    }
    if category == "tests" {
        coarse_labels.insert("test_only".to_string());
    }
    if generated_like {
        coarse_labels.insert("generated_output_touch".to_string());
    }
    if lockfile {
        coarse_labels.insert("lockfile_touch".to_string());
    }
    if ci_or_tooling {
        coarse_labels.insert("ci_or_tooling_touch".to_string());
    }
    if repo_rule {
        coarse_labels.insert("repo_rule_touch".to_string());
    }
    if dependency_policy {
        coarse_labels.insert("dependency_policy_touch".to_string());
    }
    if build_graph {
        coarse_labels.insert("build_graph_touch".to_string());
    }
    if test_infrastructure {
        coarse_labels.insert("test_infrastructure_touch".to_string());
    }
    if patch_clues.has_import_churn {
        coarse_labels.insert("import_churn".to_string());
    }
    if patch_clues.has_signature_change_like {
        coarse_labels.insert("signature_change_like".to_string());
    }
    if patch_clues.has_api_surface_like {
        coarse_labels.insert("api_surface_like".to_string());
    }

    ManifestFileSemantic {
        language: language_label(path).to_string(),
        generated_like,
        lockfile,
        ci_or_tooling,
        coarse_labels: coarse_labels.into_iter().collect(),
        related_test_candidates: infer_related_test_candidates(path, candidate_paths),
        related_source_candidates: infer_related_source_candidates(path, candidate_paths),
        related_doc_candidates: infer_related_doc_candidates(path, candidate_paths),
        related_config_candidates: infer_related_config_candidates(path, candidate_paths),
    }
}

fn language_label(path: &str) -> &'static str {
    let file_name = Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    match Path::new(path).extension().and_then(|ext| ext.to_str()) {
        Some("rs") => "rust",
        Some("py") => "python",
        Some("ts") => "typescript",
        Some("tsx") => "tsx",
        Some("js") => "javascript",
        Some("jsx") => "jsx",
        Some("go") => "go",
        Some("java") => "java",
        Some("kt") => "kotlin",
        Some("swift") => "swift",
        Some("c") | Some("h") => "c",
        Some("cc") | Some("cpp") | Some("cxx") | Some("hpp") | Some("hh") => "cpp",
        Some("json") => "json",
        Some("yaml") | Some("yml") => "yaml",
        Some("toml") => "toml",
        Some("md") => "markdown",
        Some("sh") | Some("bash") | Some("zsh") => "shell",
        _ if matches!(file_name, "Makefile" | "justfile" | "Justfile") => "build-script",
        _ => "unknown",
    }
}

fn is_generated_like_path(path: &str) -> bool {
    let file_name = Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    path.starts_with("dist/")
        || path.starts_with("build/")
        || path.starts_with("target/")
        || path.starts_with("coverage/")
        || file_name.contains(".generated.")
        || file_name.contains("_generated.")
        || file_name.ends_with(".min.js")
}

fn is_lockfile_path(path: &str) -> bool {
    matches!(
        Path::new(path).file_name().and_then(|name| name.to_str()),
        Some(
            "Cargo.lock"
                | "package-lock.json"
                | "pnpm-lock.yaml"
                | "yarn.lock"
                | "poetry.lock"
                | "Gemfile.lock"
                | "composer.lock"
                | "Podfile.lock"
        )
    )
}

fn is_ci_or_tooling_path(path: &str) -> bool {
    let file_name = Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    path.starts_with(".github/")
        || path.starts_with(".gitlab/")
        || matches!(
            file_name,
            "Makefile" | "justfile" | "Justfile" | "mise.toml" | "lefthook.yml" | "lefthook.yaml"
        )
}

fn is_repo_rule_path(path: &str) -> bool {
    matches!(
        path,
        "AGENTS.md"
            | "docs/AI_WORKFLOW.md"
            | "docs/PROJECT_KIT_TEMPLATE.md"
            | "docs/HANDOFF_TEMPLATE.md"
            | ".diffship/PROJECT_RULES.md"
            | ".diffship/AI_GUIDE.md"
            | ".diffship/PROJECT_KIT.md"
    )
}

fn is_dependency_policy_path(path: &str) -> bool {
    matches!(
        Path::new(path).file_name().and_then(|name| name.to_str()),
        Some(
            "Cargo.toml"
                | "package.json"
                | "pyproject.toml"
                | "requirements.txt"
                | "requirements-dev.txt"
                | "constraints.txt"
                | "Gemfile"
                | "go.mod"
                | "go.sum"
                | "Package.swift"
        )
    ) || is_lockfile_path(path)
}

fn is_build_graph_path(path: &str) -> bool {
    matches!(
        Path::new(path).file_name().and_then(|name| name.to_str()),
        Some(
            "Cargo.toml"
                | "package.json"
                | "pyproject.toml"
                | "tsconfig.json"
                | "Makefile"
                | "justfile"
                | "Justfile"
                | "CMakeLists.txt"
                | "build.gradle"
                | "build.gradle.kts"
                | "settings.gradle"
                | "settings.gradle.kts"
                | "Package.swift"
                | "Dockerfile"
        )
    ) || path.ends_with("docker-compose.yml")
}

fn is_test_infrastructure_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.contains("/fixtures/")
        || lower.contains("/fixture/")
        || lower.contains("/mocks/")
        || lower.contains("/mock/")
        || lower.contains("/snapshots/")
        || lower.contains("/snapshot/")
        || lower.contains("/harness/")
        || lower.starts_with("tests/fixtures/")
        || lower.starts_with("test/fixtures/")
}

fn infer_related_test_candidates(path: &str, candidate_paths: &BTreeSet<String>) -> Vec<String> {
    if is_test_like_path(path) {
        return Vec::new();
    }

    let p = Path::new(path);
    let file_name = p
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    let stem = p
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    let ext = p
        .extension()
        .and_then(|name| name.to_str())
        .map(|ext| format!(".{ext}"))
        .unwrap_or_default();
    let rel_no_src = path.strip_prefix("src/").unwrap_or(path);
    let parent = Path::new(rel_no_src)
        .parent()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_default();

    let mut explicit = BTreeSet::new();
    for root in ["tests", "test", "__tests__"] {
        explicit.insert(join_rel(root, rel_no_src));
        if !file_name.is_empty() {
            explicit.insert(join_rel(root, file_name));
        }
        if !stem.is_empty() {
            for candidate in [
                format!("{stem}_test{ext}"),
                format!("test_{stem}{ext}"),
                format!("{stem}.test{ext}"),
                format!("{stem}.spec{ext}"),
            ] {
                let rel = if parent.is_empty() {
                    candidate
                } else {
                    format!("{parent}/{candidate}")
                };
                explicit.insert(join_rel(root, &rel));
            }
        }
    }

    explicit
        .into_iter()
        .filter(|candidate| candidate_paths.contains(candidate) && is_test_like_path(candidate))
        .collect()
}

fn infer_related_source_candidates(path: &str, candidate_paths: &BTreeSet<String>) -> Vec<String> {
    if !is_test_like_path(path) {
        return Vec::new();
    }

    let stripped = strip_test_root(path).unwrap_or(path);
    let normalized = normalize_test_path_to_source_like(stripped);
    let file_name = Path::new(&normalized)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_string();

    let mut explicit = BTreeSet::new();
    explicit.insert(join_rel("src", &normalized));
    if !file_name.is_empty() {
        explicit.insert(join_rel("src", &file_name));
    }

    explicit
        .into_iter()
        .filter(|candidate| candidate_paths.contains(candidate) && candidate.starts_with("src/"))
        .collect()
}

fn infer_related_doc_candidates(path: &str, candidate_paths: &BTreeSet<String>) -> Vec<String> {
    if path.starts_with("docs/") || path == "README.md" {
        return Vec::new();
    }

    let normalized = normalize_path_for_related_docs(path);
    let p = Path::new(&normalized);
    let stem = p
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    let parent = p
        .parent()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_default();

    let mut explicit = BTreeSet::new();
    explicit.insert("README.md".to_string());
    explicit.insert(join_rel("docs", &replace_extension(&normalized, "md")));
    if !stem.is_empty() {
        explicit.insert(join_rel("docs", &format!("{stem}.md")));
        if !parent.is_empty() {
            explicit.insert(join_rel("docs", &format!("{parent}/{stem}.md")));
        }
    }

    explicit
        .into_iter()
        .filter(|candidate| {
            candidate_paths.contains(candidate)
                && (candidate == "README.md" || candidate.starts_with("docs/"))
        })
        .collect()
}

fn infer_related_config_candidates(path: &str, candidate_paths: &BTreeSet<String>) -> Vec<String> {
    let mut explicit = BTreeSet::new();
    if candidate_paths.contains("Cargo.toml") && matches!(language_label(path), "rust") {
        explicit.insert("Cargo.toml".to_string());
    }
    if matches!(
        language_label(path),
        "typescript" | "tsx" | "javascript" | "jsx"
    ) {
        for candidate in ["package.json", "tsconfig.json"] {
            if candidate_paths.contains(candidate) {
                explicit.insert(candidate.to_string());
            }
        }
    }
    if matches!(language_label(path), "python") {
        for candidate in ["pyproject.toml", "requirements.txt"] {
            if candidate_paths.contains(candidate) {
                explicit.insert(candidate.to_string());
            }
        }
    }
    if matches!(language_label(path), "go") && candidate_paths.contains("go.mod") {
        explicit.insert("go.mod".to_string());
    }
    if matches!(language_label(path), "java" | "kotlin") {
        for candidate in [
            "build.gradle",
            "settings.gradle",
            "gradle.properties",
            "pom.xml",
        ] {
            if candidate_paths.contains(candidate) {
                explicit.insert(candidate.to_string());
            }
        }
    }
    if matches!(language_label(path), "swift") && candidate_paths.contains("Package.swift") {
        explicit.insert("Package.swift".to_string());
    }
    if path.starts_with(".github/") {
        for candidate in ["README.md", "docs/ci.md"] {
            if candidate_paths.contains(candidate) {
                explicit.insert(candidate.to_string());
            }
        }
    }
    explicit.into_iter().collect()
}

fn normalize_path_for_related_docs(path: &str) -> String {
    if is_test_like_path(path) {
        normalize_test_path_to_source_like(strip_test_root(path).unwrap_or(path))
    } else {
        path.strip_prefix("src/").unwrap_or(path).to_string()
    }
}

fn strip_test_root(path: &str) -> Option<&str> {
    for root in ["tests/", "test/", "__tests__/"] {
        if let Some(stripped) = path.strip_prefix(root) {
            return Some(stripped);
        }
    }
    None
}

fn normalize_test_path_to_source_like(path: &str) -> String {
    let p = Path::new(path);
    let parent = p
        .parent()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_default();
    let file_name = p
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    let normalized = normalize_test_file_name(file_name);
    if parent.is_empty() {
        normalized
    } else {
        format!("{parent}/{normalized}")
    }
}

fn normalize_test_file_name(file_name: &str) -> String {
    let name = file_name.strip_prefix("test_").unwrap_or(file_name);
    for needle in ["_test.", ".test.", "_spec.", ".spec."] {
        if let Some((prefix, suffix)) = name.split_once(needle) {
            return format!("{prefix}.{suffix}");
        }
    }
    name.to_string()
}

fn replace_extension(path: &str, new_ext: &str) -> String {
    let p = Path::new(path);
    let stem = p.file_stem().and_then(|name| name.to_str()).unwrap_or(path);
    let parent = p
        .parent()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_default();
    let file_name = format!("{stem}.{new_ext}");
    if parent.is_empty() {
        file_name
    } else {
        format!("{parent}/{file_name}")
    }
}

fn join_rel(root: &str, rel: &str) -> String {
    let trimmed = rel.trim_start_matches('/');
    if trimmed.is_empty() {
        root.to_string()
    } else {
        format!("{root}/{trimmed}")
    }
}

fn is_test_like_path(path: &str) -> bool {
    let file_name = Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    path.starts_with("tests/")
        || path.starts_with("test/")
        || path.starts_with("__tests__/")
        || file_name.starts_with("test_")
        || file_name.contains("_test.")
        || file_name.contains(".test.")
        || file_name.contains("_spec.")
        || file_name.contains(".spec.")
}

fn build_part_scoped_context(
    part: &PartOutput,
    part_rows: &[&FileRow],
    file_semantics: &BTreeMap<String, ManifestFileSemantic>,
) -> PartScopedContext {
    let hunk_headers = extract_hunk_headers(&part.patch);
    let changed_lines = collect_changed_patch_lines(&part.patch);
    let symbol_like_names = extract_symbol_like_names(&hunk_headers, &changed_lines);
    let import_like_refs = extract_import_like_refs(&changed_lines);
    let related_test_candidates = part_rows
        .iter()
        .filter_map(|row| file_semantics.get(&row.path))
        .flat_map(|semantic| semantic.related_test_candidates.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let files = build_part_scoped_file_contexts(part, part_rows);

    PartScopedContext {
        hunk_headers,
        symbol_like_names,
        import_like_refs,
        related_test_candidates,
        files,
    }
}

fn build_part_scoped_file_contexts(
    part: &PartOutput,
    part_rows: &[&FileRow],
) -> Vec<PartScopedFileContext> {
    let mut by_path = unique_part_paths(part_rows)
        .into_iter()
        .map(|path| {
            (
                path.clone(),
                PartScopedFileContext {
                    path,
                    hunk_headers: Vec::new(),
                    symbol_like_names: Vec::new(),
                    import_like_refs: Vec::new(),
                },
            )
        })
        .collect::<BTreeMap<_, _>>();

    for (path, chunk) in collect_patch_chunks(&part.patch) {
        let Some(entry) = by_path.get_mut(&path) else {
            continue;
        };
        let headers = extract_hunk_headers(&chunk);
        let changed_lines = collect_changed_patch_lines(&chunk);
        entry.hunk_headers = merge_unique_sorted(&entry.hunk_headers, &headers);
        entry.symbol_like_names = merge_unique_sorted(
            &entry.symbol_like_names,
            &extract_symbol_like_names(&headers, &changed_lines),
        );
        entry.import_like_refs = merge_unique_sorted(
            &entry.import_like_refs,
            &extract_import_like_refs(&changed_lines),
        );
    }

    by_path.into_values().collect()
}

fn collect_file_patch_clues(parts: &[PartOutput]) -> BTreeMap<String, FilePatchClues> {
    let mut clues = BTreeMap::<String, FilePatchClues>::new();
    for part in parts {
        for (path, chunk) in collect_patch_chunks(&part.patch) {
            let changed_lines = collect_changed_patch_lines(&chunk);
            if changed_lines.is_empty() {
                continue;
            }
            let entry = clues.entry(path).or_default();
            entry.has_import_churn |= !extract_import_like_refs(&changed_lines).is_empty();
            entry.has_signature_change_like |= changed_lines
                .iter()
                .any(|line| is_signature_change_like_line(line));
            entry.has_api_surface_like |= changed_lines
                .iter()
                .any(|line| is_api_surface_like_line(line));
        }
    }
    clues
}

fn unique_part_paths(part_rows: &[&FileRow]) -> Vec<String> {
    part_rows
        .iter()
        .map(|row| row.path.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn collect_patch_chunks(patch: &str) -> Vec<(String, String)> {
    let mut chunks = Vec::new();
    let mut current = String::new();

    for line in patch.lines() {
        if line.starts_with("diff --git ") {
            if !current.is_empty() {
                if let Some(path) = patch_chunk_path(&current) {
                    chunks.push((path, current.clone()));
                }
                current.clear();
            }
            current.push_str(line);
            continue;
        }

        if current.is_empty() {
            continue;
        }
        current.push('\n');
        current.push_str(line);
    }

    if !current.is_empty()
        && let Some(path) = patch_chunk_path(&current)
    {
        chunks.push((path, current));
    }

    chunks
}

fn merge_unique_sorted(existing: &[String], incoming: &[String]) -> Vec<String> {
    existing
        .iter()
        .chain(incoming.iter())
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn extract_hunk_headers(patch: &str) -> Vec<String> {
    patch
        .lines()
        .filter_map(|line| {
            let rest = line.strip_prefix("@@ ")?;
            let (_, suffix) = rest.split_once(" @@")?;
            let suffix = suffix.trim();
            if suffix.is_empty() {
                None
            } else {
                Some(suffix.to_string())
            }
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn collect_changed_patch_lines(patch: &str) -> Vec<String> {
    patch
        .lines()
        .filter(|line| {
            (line.starts_with('+') || line.starts_with('-'))
                && !line.starts_with("+++")
                && !line.starts_with("---")
        })
        .map(|line| line[1..].trim().to_string())
        .filter(|line| !line.is_empty())
        .collect()
}

fn extract_symbol_like_names(hunk_headers: &[String], changed_lines: &[String]) -> Vec<String> {
    hunk_headers
        .iter()
        .chain(changed_lines.iter())
        .flat_map(|line| extract_symbol_like_names_from_line(line))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn extract_symbol_like_names_from_line(line: &str) -> Vec<String> {
    let trimmed = line.trim();
    let mut symbols = BTreeSet::new();

    for keyword in [
        "fn ",
        "struct ",
        "enum ",
        "trait ",
        "impl ",
        "mod ",
        "function ",
        "class ",
        "interface ",
        "type ",
        "def ",
        "func ",
    ] {
        if let Some(name) = identifier_after_keyword(trimmed, keyword) {
            symbols.insert(name);
        }
    }

    if trimmed.contains("=>")
        && let Some(name) = identifier_after_keyword(trimmed, "const ")
            .or_else(|| identifier_after_keyword(trimmed, "let "))
            .or_else(|| identifier_after_keyword(trimmed, "var "))
    {
        symbols.insert(name);
    }

    symbols.into_iter().collect()
}

fn identifier_after_keyword(line: &str, keyword: &str) -> Option<String> {
    let idx = line.find(keyword)?;
    let rest = &line[idx + keyword.len()..];
    let candidate = rest
        .trim_start_matches(|c: char| c.is_whitespace() || c == '(' || c == '<')
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == ':' || *c == '.')
        .collect::<String>();
    normalize_symbol_candidate(&candidate)
}

fn normalize_symbol_candidate(candidate: &str) -> Option<String> {
    let trimmed =
        candidate.trim_matches(|c: char| c == ':' || c == '.' || c == '<' || c == '>' || c == '(');
    if trimmed.is_empty() {
        return None;
    }
    if trimmed == "if" || trimmed == "for" || trimmed == "while" || trimmed == "match" {
        return None;
    }
    Some(trimmed.to_string())
}

fn extract_import_like_refs(changed_lines: &[String]) -> Vec<String> {
    changed_lines
        .iter()
        .filter_map(|line| normalize_import_like_ref(line))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn normalize_import_like_ref(line: &str) -> Option<String> {
    let trimmed = line.trim();
    let is_import_like = trimmed.starts_with("use ")
        || trimmed.starts_with("import ")
        || trimmed.starts_with("from ")
        || trimmed.starts_with("mod ")
        || trimmed.starts_with("#include")
        || trimmed.contains("require(");
    if !is_import_like {
        return None;
    }

    Some(
        trimmed
            .trim_end_matches(';')
            .trim_end_matches('{')
            .trim()
            .to_string(),
    )
}

fn is_signature_change_like_line(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return false;
    }
    [
        "pub fn ",
        "fn ",
        "pub struct ",
        "struct ",
        "pub enum ",
        "enum ",
        "pub trait ",
        "trait ",
        "impl ",
        "function ",
        "export function ",
        "class ",
        "export class ",
        "interface ",
        "type ",
        "export type ",
        "def ",
        "async def ",
        "func ",
    ]
    .iter()
    .any(|needle| trimmed.starts_with(needle))
}

fn is_api_surface_like_line(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.starts_with("pub ")
        || trimmed.starts_with("export ")
        || trimmed.starts_with("interface ")
        || trimmed.starts_with("type ")
        || trimmed.starts_with("class ")
        || trimmed.starts_with("trait ")
    {
        return true;
    }
    trimmed.starts_with("def ") && !trimmed.starts_with("def _")
}

fn count_part_categories(rows: &[&FileRow]) -> PartContextCategoryCounts {
    let mut counts = PartContextCategoryCounts::default();
    let unique_paths = rows
        .iter()
        .map(|row| row.path.as_str())
        .collect::<BTreeSet<_>>();
    for path in unique_paths {
        match path_category_label(path) {
            "docs" => counts.docs += 1,
            "config" => counts.config += 1,
            "source" => counts.source += 1,
            "tests" => counts.tests += 1,
            _ => counts.other += 1,
        }
    }
    counts
}

fn count_row_labels<'a, I, F>(rows: I, mut key_fn: F) -> BTreeMap<String, usize>
where
    I: IntoIterator<Item = &'a FileRow>,
    F: FnMut(&'a FileRow) -> String,
{
    let mut counts = BTreeMap::new();
    for row in rows {
        *counts.entry(key_fn(row)).or_insert(0) += 1;
    }
    counts
}

fn build_part_intent_labels(
    rows: &[&FileRow],
    file_semantics: &BTreeMap<String, ManifestFileSemantic>,
) -> Vec<String> {
    let mut labels = BTreeSet::new();
    let counts = count_part_categories(rows);

    if counts.docs > 0 {
        labels.insert("docs_update".to_string());
    }
    if counts.config > 0 {
        labels.insert("config_update".to_string());
    }
    if counts.source > 0 {
        labels.insert("source_update".to_string());
    }
    if counts.tests > 0 {
        labels.insert("test_update".to_string());
    }
    if counts.other > 0 {
        labels.insert("other_update".to_string());
    }

    let active_categories = [
        counts.docs > 0,
        counts.config > 0,
        counts.source > 0,
        counts.tests > 0,
        counts.other > 0,
    ]
    .into_iter()
    .filter(|present| *present)
    .count();
    if active_categories > 1 {
        labels.insert("cross_area_change".to_string());
    }

    for row in rows {
        if row_has_reduced_context(row) {
            labels.insert("reduced_context".to_string());
        }
        let hints = build_manifest_file_change_hints(row);
        if hints.rename_or_copy {
            labels.insert("rename_or_copy".to_string());
        }
        if let Some(semantic) = file_semantics.get(&row.path) {
            if semantic
                .coarse_labels
                .iter()
                .any(|label| label == "api_surface_like")
            {
                labels.insert("api_surface_touch".to_string());
            }
            if semantic
                .coarse_labels
                .iter()
                .any(|label| label == "import_churn")
            {
                labels.insert("import_churn".to_string());
            }
            if semantic.generated_like {
                labels.insert("generated_output_touch".to_string());
            }
        }
    }

    labels.into_iter().collect()
}

fn build_review_labels_for_rows(
    rows: &[&FileRow],
    file_semantics: &BTreeMap<String, ManifestFileSemantic>,
) -> Vec<String> {
    let mut labels = BTreeSet::new();
    let counts = count_part_categories(rows);
    let has_source = counts.source > 0;
    let has_tests = counts.tests > 0;
    let has_docs = counts.docs > 0;
    let has_config = counts.config > 0;
    let has_other = counts.other > 0;

    let mut has_api_surface = false;
    let mut has_signature_change = false;
    let mut has_import_churn = false;
    let mut has_ci_or_tooling = false;
    let mut has_lockfile = false;
    let mut has_generated = false;
    let mut has_related_tests = false;

    for row in rows {
        if let Some(semantic) = file_semantics.get(&row.path) {
            has_api_surface |= semantic
                .coarse_labels
                .iter()
                .any(|label| label == "api_surface_like");
            has_signature_change |= semantic
                .coarse_labels
                .iter()
                .any(|label| label == "signature_change_like");
            has_import_churn |= semantic
                .coarse_labels
                .iter()
                .any(|label| label == "import_churn");
            has_ci_or_tooling |= semantic.ci_or_tooling;
            has_lockfile |= semantic.lockfile;
            has_generated |= semantic.generated_like;
            has_related_tests |= !semantic.related_test_candidates.is_empty();
        }
    }

    if has_source && (has_api_surface || has_signature_change) {
        labels.insert("behavioral_change_like".to_string());
    }
    if !has_source && (has_docs || has_config || has_other || has_generated) {
        labels.insert("mechanical_update_like".to_string());
    }
    if has_source || has_tests || has_config || has_ci_or_tooling || has_lockfile || has_api_surface
    {
        labels.insert("verification_surface_touch".to_string());
    }
    if has_source && !has_tests && has_related_tests {
        labels.insert("needs_related_test_review".to_string());
    }
    if has_config || has_ci_or_tooling || has_lockfile {
        labels.insert("repo_policy_touch".to_string());
    }
    if has_source && has_docs {
        labels.insert("documentation_alignment_needed".to_string());
    }
    if has_import_churn {
        labels.insert("dependency_or_import_review".to_string());
    }

    labels.into_iter().collect()
}

fn part_context_title(
    part: &PartOutput,
    file_paths: &[String],
    category_counts: &PartContextCategoryCounts,
) -> String {
    if let Some(path) = file_paths.first()
        && file_paths.len() == 1
    {
        return format!("{}: {}", part.name, path);
    }
    if file_paths.is_empty() {
        format!("{}: no file changes", part.name)
    } else {
        format!(
            "{}: {} files in {}",
            part.name,
            file_paths.len(),
            primary_category_label(category_counts)
        )
    }
}

fn part_context_summary(
    segments: &[String],
    category_counts: &PartContextCategoryCounts,
    file_count: usize,
) -> String {
    if file_count == 0 {
        return "This part contains no file-level changes.".to_string();
    }
    format!(
        "This part updates {} across {} segment(s): {}.",
        summarize_category_counts(category_counts),
        segments.len(),
        segments.join(", ")
    )
}

fn part_context_intent(category_counts: &PartContextCategoryCounts) -> String {
    if category_total(category_counts) == 0 {
        "Primary area: no file-level changes were recorded for this part.".to_string()
    } else {
        format!(
            "Primary area: {} changes.",
            primary_category_label(category_counts)
        )
    }
}

fn category_total(counts: &PartContextCategoryCounts) -> usize {
    counts.docs + counts.config + counts.source + counts.tests + counts.other
}

fn primary_category_label(counts: &PartContextCategoryCounts) -> &'static str {
    [
        ("documentation", counts.docs),
        ("config/tooling", counts.config),
        ("source", counts.source),
        ("tests", counts.tests),
        ("other", counts.other),
    ]
    .into_iter()
    .max_by(|a, b| a.1.cmp(&b.1).then(a.0.cmp(b.0)))
    .map(|(label, _)| label)
    .unwrap_or("other")
}

fn summarize_category_counts(counts: &PartContextCategoryCounts) -> String {
    let mut items = Vec::new();
    for (label, n) in [
        ("documentation file", counts.docs),
        ("config/tooling file", counts.config),
        ("source file", counts.source),
        ("test file", counts.tests),
        ("other file", counts.other),
    ] {
        if n == 0 {
            continue;
        }
        let suffix = if n == 1 { "" } else { "s" };
        items.push(format!("{n} {label}{suffix}"));
    }
    if items.is_empty() {
        "0 files".to_string()
    } else {
        items.join(", ")
    }
}

fn part_context_acceptance_criteria(
    part: &PartOutput,
    file_paths: &[String],
    reduced_context: bool,
) -> Vec<String> {
    let mut items = vec![
        format!(
            "Apply or review `parts/{}` as the canonical change payload for this part.",
            part.name
        ),
        "Keep edits scoped to the listed files unless a new handoff bundle expands the scope."
            .to_string(),
    ];
    if file_paths.is_empty() {
        items.push(
            "Confirm whether this no-op part can be ignored or removed in a future build."
                .to_string(),
        );
    }
    if reduced_context {
        items.push(
            "Reduced diff context is present; review affected paths carefully before editing further."
                .to_string(),
        );
    }
    items
}

fn sum_opt<I>(vals: I) -> Option<u64>
where
    I: IntoIterator<Item = Option<u64>>,
{
    let mut seen = false;
    let mut sum = 0_u64;
    for v in vals.into_iter().flatten() {
        seen = true;
        sum += v;
    }
    if seen { Some(sum) } else { None }
}

fn write_zip_from_dir(src_dir: &Path, zip_path: &Path) -> Result<(), ExitError> {
    if let Some(parent) = zip_path.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            ExitError::new(
                EXIT_GENERAL,
                format!("failed to create zip output dir: {e}"),
            )
        })?;
    }
    let file = fs::File::create(zip_path)
        .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to create zip: {e}")))?;
    let mut zip = ZipWriter::new(file);
    let opts = deterministic_zip_file_options();
    add_dir_recursive(&mut zip, opts, src_dir, "")?;
    zip.finish()
        .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to finalize zip: {e}")))?;
    Ok(())
}

fn add_dir_recursive<W: Write + io::Seek>(
    zip: &mut ZipWriter<W>,
    opts: FileOptions,
    dir: &Path,
    prefix: &str,
) -> Result<(), ExitError> {
    let mut entries = fs::read_dir(dir)
        .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to read dir: {e}")))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to read dir entry: {e}")))?;
    entries.sort_by_key(|e| e.file_name());
    for ent in entries {
        let path = ent.path();
        let name = ent.file_name();
        let name = name.to_string_lossy();
        let rel = if prefix.is_empty() {
            name.to_string()
        } else {
            format!("{prefix}/{name}")
        };
        if path.is_dir() {
            add_dir_recursive(zip, opts, &path, &rel)?;
        } else if path.is_file() {
            let bytes = fs::read(&path)
                .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to read file: {e}")))?;
            zip.start_file(rel, opts).map_err(|e| {
                ExitError::new(EXIT_GENERAL, format!("failed to add zip entry: {e}"))
            })?;
            zip.write_all(&bytes).map_err(|e| {
                ExitError::new(EXIT_GENERAL, format!("failed to write zip entry: {e}"))
            })?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::iter::FromIterator;
    use time::{Date, Month, PrimitiveDateTime, Time, UtcOffset};

    #[test]
    fn auto_split_becomes_file_for_single_commit() {
        let plan = RangePlan {
            mode: RangeMode::Last,
            base: "a".into(),
            target: "b".into(),
            from_rev: None,
            to_rev: None,
            a_rev: None,
            b_rev: None,
            merge_base: None,
            commit_count: Some(1),
        };
        assert_eq!(
            effective_split_by(Some("auto"), Some(&plan)).unwrap(),
            SplitBy::File
        );
    }

    #[test]
    fn format_output_timestamp_uses_local_offset_fields() {
        let offset = UtcOffset::from_hms(9, 0, 0).unwrap();
        let date = Date::from_calendar_date(2026, Month::March, 7).unwrap();
        let time = Time::from_hms(2, 18, 0).unwrap();
        let dt = PrimitiveDateTime::new(date, time)
            .assume_utc()
            .to_offset(offset);
        assert_eq!(format_output_timestamp(dt).unwrap(), "2026-03-07_1118");
    }

    #[test]
    fn default_output_dir_adds_numeric_suffix_when_timestamp_exists() {
        let td = tempfile::tempdir().unwrap();
        let cwd = td.path();
        fs::create_dir_all(cwd.join("diffship_2026-03-07_1118_abcdef1")).unwrap();
        fs::create_dir_all(cwd.join("diffship_2026-03-07_1118_abcdef1_2")).unwrap();
        let resolved = default_output_dir_for_timestamp(cwd, "2026-03-07_1118", "abcdef1");

        assert_eq!(resolved, cwd.join("diffship_2026-03-07_1118_abcdef1_3"));
    }

    #[test]
    fn language_label_classifies_common_extensions() {
        assert_eq!(language_label("src/lib.rs"), "rust");
        assert_eq!(language_label("web/app.ts"), "typescript");
        assert_eq!(language_label("web/App.tsx"), "tsx");
        assert_eq!(language_label("scripts/build.sh"), "shell");
        assert_eq!(language_label("justfile"), "build-script");
        assert_eq!(language_label("docs/spec.md"), "markdown");
        assert_eq!(language_label("unknown/file.xyz"), "unknown");
    }

    #[test]
    fn semantic_flags_classify_generated_lockfile_and_tooling_paths() {
        assert!(is_generated_like_path("target/debug/app"));
        assert!(is_generated_like_path("web/dist/app.min.js"));
        assert!(is_generated_like_path("src/foo_generated.rs"));
        assert!(!is_generated_like_path("src/lib.rs"));

        assert!(is_lockfile_path("Cargo.lock"));
        assert!(is_lockfile_path("frontend/pnpm-lock.yaml"));
        assert!(!is_lockfile_path("Cargo.toml"));

        assert!(is_ci_or_tooling_path(".github/workflows/ci.yml"));
        assert!(is_ci_or_tooling_path("justfile"));
        assert!(is_ci_or_tooling_path("tooling/mise.toml"));
        assert!(!is_ci_or_tooling_path("src/lib.rs"));

        assert!(is_repo_rule_path("AGENTS.md"));
        assert!(is_repo_rule_path(".diffship/PROJECT_RULES.md"));
        assert!(!is_repo_rule_path("docs/lib.md"));

        assert!(is_dependency_policy_path("Cargo.toml"));
        assert!(is_dependency_policy_path("package.json"));
        assert!(!is_dependency_policy_path("src/lib.rs"));

        assert!(is_build_graph_path("tsconfig.json"));
        assert!(is_build_graph_path("Dockerfile"));
        assert!(!is_build_graph_path("docs/lib.md"));

        assert!(is_test_infrastructure_path("tests/fixtures/api.json"));
        assert!(is_test_infrastructure_path("tests/mocks/client.rs"));
        assert!(!is_test_infrastructure_path("tests/lib_test.rs"));
    }

    #[test]
    fn build_manifest_file_semantic_adds_coarse_labels_from_path_flags() {
        let semantic = build_manifest_file_semantic(
            ".github/workflows/ci.yml",
            &BTreeSet::new(),
            FilePatchClues::default(),
        );
        assert!(semantic.coarse_labels.contains(&"config_only".to_string()));
        assert!(
            semantic
                .coarse_labels
                .contains(&"ci_or_tooling_touch".to_string())
        );

        let generated = build_manifest_file_semantic(
            "target/generated/schema.generated.json",
            &BTreeSet::new(),
            FilePatchClues::default(),
        );
        assert!(
            generated
                .coarse_labels
                .contains(&"generated_output_touch".to_string())
        );

        let test_file = build_manifest_file_semantic(
            "tests/lib_test.rs",
            &BTreeSet::new(),
            FilePatchClues::default(),
        );
        assert!(test_file.coarse_labels.contains(&"test_only".to_string()));

        let repo_rule =
            build_manifest_file_semantic("AGENTS.md", &BTreeSet::new(), FilePatchClues::default());
        assert!(
            repo_rule
                .coarse_labels
                .contains(&"repo_rule_touch".to_string())
        );

        let dependency =
            build_manifest_file_semantic("Cargo.toml", &BTreeSet::new(), FilePatchClues::default());
        assert!(
            dependency
                .coarse_labels
                .contains(&"dependency_policy_touch".to_string())
        );
        assert!(
            dependency
                .coarse_labels
                .contains(&"build_graph_touch".to_string())
        );

        let fixture = build_manifest_file_semantic(
            "tests/fixtures/api.json",
            &BTreeSet::new(),
            FilePatchClues::default(),
        );
        assert!(
            fixture
                .coarse_labels
                .contains(&"test_infrastructure_touch".to_string())
        );
    }

    #[test]
    fn infer_related_test_candidates_returns_existing_stable_matches() {
        let candidates = BTreeSet::from_iter(
            [
                "src/lib.rs",
                "tests/lib.rs",
                "tests/lib_test.rs",
                "tests/nested/foo_test.py",
                "tests/nested/foo.spec.py",
                "tests/ignored.md",
            ]
            .into_iter()
            .map(ToOwned::to_owned),
        );

        assert_eq!(
            infer_related_test_candidates("src/lib.rs", &candidates),
            vec!["tests/lib.rs".to_string(), "tests/lib_test.rs".to_string()]
        );
        assert_eq!(
            infer_related_test_candidates("src/nested/foo.py", &candidates),
            vec![
                "tests/nested/foo.spec.py".to_string(),
                "tests/nested/foo_test.py".to_string(),
            ]
        );
        assert!(infer_related_test_candidates("tests/lib_test.rs", &candidates).is_empty());
    }

    #[test]
    fn infer_related_source_candidates_returns_existing_stable_matches() {
        let candidates = BTreeSet::from_iter(
            [
                "src/lib.rs",
                "src/nested/foo.py",
                "tests/lib_test.rs",
                "tests/nested/test_foo.py",
                "tests/nested/foo.spec.py",
            ]
            .into_iter()
            .map(ToOwned::to_owned),
        );

        assert_eq!(
            infer_related_source_candidates("tests/lib_test.rs", &candidates),
            vec!["src/lib.rs".to_string()]
        );
        assert_eq!(
            infer_related_source_candidates("tests/nested/test_foo.py", &candidates),
            vec!["src/nested/foo.py".to_string()]
        );
        assert_eq!(
            infer_related_source_candidates("tests/nested/foo.spec.py", &candidates),
            vec!["src/nested/foo.py".to_string()]
        );
        assert!(infer_related_source_candidates("src/lib.rs", &candidates).is_empty());
    }

    #[test]
    fn infer_related_doc_and_config_candidates_return_existing_stable_matches() {
        let candidates = BTreeSet::from_iter(
            [
                "src/lib.rs",
                "tests/lib_test.rs",
                "Cargo.toml",
                "README.md",
                "docs/lib.md",
                "docs/nested/foo.md",
                "src/nested/foo.py",
                "tests/nested/test_foo.py",
                "pyproject.toml",
            ]
            .into_iter()
            .map(ToOwned::to_owned),
        );

        assert_eq!(
            infer_related_doc_candidates("src/lib.rs", &candidates),
            vec!["README.md".to_string(), "docs/lib.md".to_string()]
        );
        assert_eq!(
            infer_related_doc_candidates("tests/nested/test_foo.py", &candidates),
            vec!["README.md".to_string(), "docs/nested/foo.md".to_string()]
        );
        assert_eq!(
            infer_related_config_candidates("src/lib.rs", &candidates),
            vec!["Cargo.toml".to_string()]
        );
        assert_eq!(
            infer_related_config_candidates("tests/nested/test_foo.py", &candidates),
            vec!["pyproject.toml".to_string()]
        );
    }

    #[test]
    fn patch_clues_detect_import_signature_and_api_surface_lines() {
        assert!(is_signature_change_like_line(
            "pub fn value(input: i32) -> i32 {"
        ));
        assert!(is_signature_change_like_line("interface ResultShape {"));
        assert!(!is_signature_change_like_line("let value = 1;"));

        assert!(is_api_surface_like_line("pub struct Value {"));
        assert!(is_api_surface_like_line("export function buildThing() {"));
        assert!(is_api_surface_like_line("def render(request):"));
        assert!(!is_api_surface_like_line("def _helper(request):"));

        let parts = vec![PartOutput {
            name: "part_01.patch".to_string(),
            patch: r#"diff --git a/src/lib.rs b/src/lib.rs
index 1111111..2222222 100644
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -1,3 +1,6 @@
-use crate::old_dep;
+use crate::new_dep;
+pub fn value(input: i32) -> i32 {
+export function buildThing() {
  1
 }
"#
            .to_string(),
            segments: vec!["committed".to_string()],
        }];

        let clues = collect_file_patch_clues(&parts);
        let src = clues.get("src/lib.rs").expect("src/lib.rs clues");
        assert!(src.has_import_churn);
        assert!(src.has_signature_change_like);
        assert!(src.has_api_surface_like);
    }

    #[test]
    fn change_hints_classify_rename_attachment_exclusion_and_reduced_context() {
        let renamed = FileRow {
            segment: "committed".to_string(),
            status: "R".to_string(),
            path: "new.txt".to_string(),
            note: "from old.txt".to_string(),
            ins: Some(0),
            del: Some(0),
            bytes: Some(3),
            part: "part_01.patch".to_string(),
        };
        let attachment = FileRow {
            segment: "untracked".to_string(),
            status: "A".to_string(),
            path: "bin.dat".to_string(),
            note: "stored in attachments.zip".to_string(),
            ins: None,
            del: None,
            bytes: Some(4),
            part: "attachments.zip".to_string(),
        };
        let excluded = FileRow {
            segment: "committed".to_string(),
            status: "A".to_string(),
            path: "notes.txt".to_string(),
            note: "excluded (meta only; see excluded.md)".to_string(),
            ins: None,
            del: None,
            bytes: Some(2),
            part: "-".to_string(),
        };
        let reduced = FileRow {
            segment: "committed".to_string(),
            status: "M".to_string(),
            path: "src/lib.rs".to_string(),
            note: "packing fallback reduced diff context to U0".to_string(),
            ins: Some(1),
            del: Some(1),
            bytes: Some(10),
            part: "part_01.patch".to_string(),
        };

        let renamed_hints = build_manifest_file_change_hints(&renamed);
        assert!(renamed_hints.rename_or_copy);
        assert_eq!(renamed_hints.previous_path.as_deref(), Some("old.txt"));
        assert!(!renamed_hints.new_file);

        let attachment_hints = build_manifest_file_change_hints(&attachment);
        assert!(attachment_hints.stored_as_attachment);
        assert!(attachment_hints.new_file);

        let excluded_hints = build_manifest_file_change_hints(&excluded);
        assert!(excluded_hints.excluded);
        assert!(excluded_hints.new_file);

        let reduced_hints = build_manifest_file_change_hints(&reduced);
        assert!(reduced_hints.reduced_context);
        assert!(!reduced_hints.excluded);
    }

    #[test]
    fn scoped_context_extracts_hunk_headers_symbols_and_imports() {
        let patch = r#"diff --git a/src/lib.rs b/src/lib.rs
index 1111111..2222222 100644
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -1,3 +1,6 @@ pub fn old_name() -> i32 {
-use crate::old_dep;
+use crate::new_dep;
+import { helper } from "./helper";
+const makeValue = () => helper();
+fn new_name() -> i32 {
  1
 }
"#;

        let headers = extract_hunk_headers(patch);
        assert_eq!(headers, vec!["pub fn old_name() -> i32 {".to_string()]);

        let changed_lines = collect_changed_patch_lines(patch);
        assert!(changed_lines.contains(&"use crate::new_dep;".to_string()));
        assert!(changed_lines.contains(&"const makeValue = () => helper();".to_string()));

        let symbols = extract_symbol_like_names(&headers, &changed_lines);
        assert!(symbols.contains(&"old_name".to_string()));
        assert!(symbols.contains(&"new_name".to_string()));
        assert!(symbols.contains(&"makeValue".to_string()));

        let imports = extract_import_like_refs(&changed_lines);
        assert_eq!(
            imports,
            vec![
                "import { helper } from \"./helper\"".to_string(),
                "use crate::new_dep".to_string(),
                "use crate::old_dep".to_string(),
            ]
        );

        let part = PartOutput {
            name: "part_01.patch".to_string(),
            patch: patch.to_string(),
            segments: vec!["committed".to_string()],
        };
        let rows = [FileRow {
            segment: "committed".to_string(),
            status: "M".to_string(),
            path: "src/lib.rs".to_string(),
            note: String::new(),
            ins: Some(4),
            del: Some(1),
            bytes: Some(42),
            part: "part_01.patch".to_string(),
        }];
        let file_contexts = build_part_scoped_file_contexts(&part, &[&rows[0]]);
        assert_eq!(file_contexts.len(), 1);
        assert_eq!(file_contexts[0].path, "src/lib.rs");
        assert!(
            file_contexts[0]
                .symbol_like_names
                .contains(&"new_name".to_string())
        );
        assert!(
            file_contexts[0]
                .import_like_refs
                .contains(&"use crate::new_dep".to_string())
        );
    }
}

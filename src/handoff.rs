use crate::cli::BuildArgs;
use crate::exit::{EXIT_GENERAL, EXIT_PACKING_LIMITS, EXIT_SECRETS_WARNING, ExitError};
use crate::filter::PathFilter;
use crate::git;
use crate::handoff_config::{DEFAULT_PROFILE_NAME, HandoffConfig};
use crate::pathing::resolve_user_path;
use crate::plan::HandoffPlan;
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

pub fn cmd(git_root: &Path, args: BuildArgs) -> Result<(), ExitError> {
    let cwd = std::env::current_dir()
        .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to detect current dir: {e}")))?;
    let args = resolve_build_args(git_root, &cwd, args)?;
    let resolved_plan = HandoffPlan::from_build_args(&args);

    let out_dir = match &args.out {
        Some(o) => resolve_user_path(&cwd, o)?,
        None => default_output_dir(
            &args
                .out_dir
                .as_deref()
                .map_or_else(|| Ok(cwd.clone()), |raw| resolve_user_path(&cwd, raw))?,
        )?,
    };

    if out_dir.exists() {
        return Err(ExitError::new(
            EXIT_GENERAL,
            format!("output path already exists: {}", out_dir.display()),
        ));
    }

    let parts_dir = out_dir.join("parts");
    fs::create_dir_all(&parts_dir)
        .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to create output dir: {e}")))?;

    let sources = SourceSelection::from_args(&args)?;
    let packing_limits = PackingLimits::from_args(&args)?;
    let filters = PathFilter::load(git_root, &args.include, &args.exclude)?;
    let head = git::rev_parse(git_root, "HEAD")?;
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
        let mut seg =
            apply_tracked_binary_policy(git_root, seg, binary_policy, TrackedBinarySource::Index)?;
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

    for part in &parts {
        write_text_file(&parts_dir.join(&part.name), &part.patch)?;
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

    let secret_hits = scan_bundle_for_secrets(&parts, &attachments)?;
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
        packing_limits,
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
    });
    write_text_file(&out_dir.join("HANDOFF.md"), &handoff)?;

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

    let mut zip_path = None;
    if args.zip {
        let zp = out_dir.with_extension("zip");
        write_zip_from_dir(&out_dir, &zp)?;
        zip_path = Some(zp);
    }

    println!("diffship build: created {}", out_dir.display());
    if !attachments.is_empty() {
        println!(
            "diffship build: created {}/attachments.zip",
            out_dir.display()
        );
    }
    if !exclusions.is_empty() {
        println!("diffship build: created {}/excluded.md", out_dir.display());
    }
    if !secret_hits.is_empty() {
        println!("diffship build: created {}/secrets.md", out_dir.display());
    }
    if let Some(zp) = zip_path {
        println!("diffship build: created {}", zp.display());
    }

    Ok(())
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

fn default_output_dir(cwd: &Path) -> Result<PathBuf, ExitError> {
    let timestamp = timestamp_yyyymmdd_hhmm()?;
    Ok(default_output_dir_for_timestamp(cwd, &timestamp))
}

fn default_output_dir_for_timestamp(cwd: &Path, timestamp: &str) -> PathBuf {
    let base = cwd.join(format!("diffship_{timestamp}"));
    if !base.exists() {
        return base;
    }

    for suffix in 2.. {
        let candidate = cwd.join(format!("diffship_{timestamp}_{suffix}"));
        if !candidate.exists() {
            return candidate;
        }
    }

    unreachable!("numeric suffix search is unbounded");
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
    if path.starts_with("docs/") || path.ends_with(".md") {
        0
    } else if path.starts_with(".github/")
        || path.ends_with(".toml")
        || path.ends_with(".yml")
        || path.ends_with(".yaml")
        || path.ends_with(".json")
        || path.ends_with(".lock")
    {
        1
    } else if path.starts_with("src/") {
        2
    } else if path.starts_with("tests/") {
        3
    } else {
        4
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
    s.push_str("3. Use the Parts Index to decide reading order inside the patch bundle.\n");
    s.push_str(&format!(
        "4. Open the first patch part: `{}`\n",
        inp.first_part_rel
    ));
    if !inp.attachments.is_empty() {
        s.push_str("5. After the patch parts, inspect attachments.zip for raw files that were intentionally kept out of patch text.\n");
    }
    if !inp.exclusions.is_empty() {
        s.push_str("6. Check excluded.md for files that were omitted on purpose and why.\n");
    }
    if !inp.secret_hits.is_empty() {
        s.push_str("7. Review secrets.md before sharing the bundle. It lists only paths and reasons, never secret values.\n");
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
        fs::create_dir_all(cwd.join("diffship_2026-03-07_1118")).unwrap();
        fs::create_dir_all(cwd.join("diffship_2026-03-07_1118_2")).unwrap();
        let resolved = default_output_dir_for_timestamp(cwd, "2026-03-07_1118");

        assert_eq!(resolved, cwd.join("diffship_2026-03-07_1118_3"));
    }
}

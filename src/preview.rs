use crate::cli::PreviewArgs;
use crate::exit::{EXIT_GENERAL, ExitError};
use crate::pathing::resolve_user_path;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::Path;
use zip::ZipArchive;

#[derive(Debug, Clone)]
struct BundleView {
    entries: BTreeMap<String, Vec<u8>>,
}

#[derive(Debug, Serialize)]
struct PreviewSummary<'a> {
    bundle: String,
    mode: &'a str,
    handoff_md: bool,
    ai_requests_md: bool,
    parts: Vec<String>,
    attachments_zip: bool,
    excluded_md: bool,
    secrets_md: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    structured_context: Option<PreviewStructuredContext>,
}

#[derive(Debug, Serialize)]
struct PreviewText {
    bundle: String,
    entry: String,
    text: String,
}

#[derive(Debug, Serialize)]
struct PreviewStructuredContext {
    manifest_json: bool,
    context_xml: bool,
    part_contexts: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    project_context: Option<PreviewProjectContext>,
    #[serde(skip_serializing_if = "Option::is_none")]
    summary: Option<PreviewManifestSummary>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    reading_order: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    task_groups: Vec<PreviewTaskGroup>,
    #[serde(skip_serializing_if = "Option::is_none")]
    semantic_facts: Option<PreviewSemanticFacts>,
    #[serde(skip_serializing_if = "Option::is_none")]
    coarse_labels: Option<PreviewCoarseLabelFacts>,
    #[serde(skip_serializing_if = "Option::is_none")]
    change_hints: Option<PreviewChangeHintFacts>,
    #[serde(skip_serializing_if = "Option::is_none")]
    scoped_context: Option<PreviewScopedContextFacts>,
}

#[derive(Debug, Clone, Serialize)]
struct PreviewManifestSummary {
    file_count: u64,
    part_count: u64,
    commit_view_count: u64,
    categories: BTreeMap<String, u64>,
    segments: BTreeMap<String, u64>,
    statuses: BTreeMap<String, u64>,
}

#[derive(Debug, Clone, Serialize)]
struct PreviewTaskGroup {
    task_id: String,
    intent_labels: Vec<String>,
    part_ids: Vec<String>,
    segments: Vec<String>,
    top_files: Vec<String>,
    part_count: u64,
    file_count: u64,
}

#[derive(Debug, Clone, Serialize)]
struct PreviewSemanticFacts {
    manifest_file_entries: bool,
    part_context_entries: usize,
}

#[derive(Debug, Clone, Serialize)]
struct PreviewCoarseLabelFacts {
    manifest_file_entries: bool,
    part_context_entries: usize,
}

#[derive(Debug, Clone, Serialize)]
struct PreviewChangeHintFacts {
    manifest_file_entries: bool,
    part_context_entries: usize,
}

#[derive(Debug, Clone, Serialize)]
struct PreviewScopedContextFacts {
    part_context_entries: usize,
}

#[derive(Debug, Clone, Serialize)]
struct PreviewProjectContext {
    manifest_json: bool,
    context_md: bool,
    snapshot_files: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    summary: Option<PreviewProjectContextSummary>,
}

#[derive(Debug, Clone, Serialize)]
struct PreviewProjectContextSummary {
    selected_files: u64,
    included_snapshots: u64,
    omitted_files: u64,
    total_snapshot_bytes: u64,
}

#[derive(Debug, Deserialize)]
struct ManifestPreviewSummaryRaw {
    file_count: u64,
    part_count: u64,
    commit_view_count: u64,
    categories: ManifestCategoryCountsRaw,
    segments: BTreeMap<String, u64>,
    statuses: BTreeMap<String, u64>,
}

#[derive(Debug, Deserialize)]
struct ManifestPreviewTaskGroupRaw {
    task_id: String,
    intent_labels: Vec<String>,
    part_ids: Vec<String>,
    segments: Vec<String>,
    top_files: Vec<String>,
    part_count: u64,
    file_count: u64,
}

#[derive(Debug, Deserialize)]
struct ManifestCategoryCountsRaw {
    docs: u64,
    config: u64,
    source: u64,
    tests: u64,
    other: u64,
}

#[derive(Debug, Deserialize)]
struct ProjectContextPreviewEnvelope {
    summary: ProjectContextPreviewSummaryRaw,
}

#[derive(Debug, Deserialize)]
struct ProjectContextPreviewSummaryRaw {
    selected_files: u64,
    included_snapshots: u64,
    omitted_files: u64,
    total_snapshot_bytes: u64,
}

pub fn cmd(args: PreviewArgs) -> Result<(), ExitError> {
    let cwd = std::env::current_dir()
        .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to detect current dir: {e}")))?;
    let bundle_path = resolve_user_path(&cwd, &args.bundle)?;
    let view = load_bundle(&bundle_path)?;

    if args.list {
        if args.json {
            print_json(&summary_json(&bundle_path, &view, "list"))?;
            return Ok(());
        }
        print_list(&bundle_path, &view);
        return Ok(());
    }

    if let Some(part) = args.part.as_deref() {
        let key = resolve_part_key(part, &view)?;
        let body = read_entry_text(&view, &key)?;
        if args.json {
            print_json(&PreviewText {
                bundle: bundle_path.display().to_string(),
                entry: key,
                text: body,
            })?;
            return Ok(());
        }
        print!("{}", body);
        if !body.ends_with('\n') {
            println!();
        }
        return Ok(());
    }

    let handoff = read_entry_text(&view, "HANDOFF.md")?;
    if args.json {
        print_json(&PreviewText {
            bundle: bundle_path.display().to_string(),
            entry: "HANDOFF.md".to_string(),
            text: handoff,
        })?;
        return Ok(());
    }
    print!("{}", handoff);
    if !handoff.ends_with('\n') {
        println!();
    }
    Ok(())
}

pub(crate) fn load_bundle_summary_value(path: &Path) -> Result<serde_json::Value, ExitError> {
    let view = load_bundle(path)?;
    serde_json::to_value(summary_json(path, &view, "explain"))
        .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to render JSON: {e}")))
}

pub(crate) fn load_bundle_entry_text(path: &Path, entry: &str) -> Result<String, ExitError> {
    let view = load_bundle(path)?;
    read_entry_text(&view, entry)
}

fn print_json<T: Serialize>(value: &T) -> Result<(), ExitError> {
    let s = serde_json::to_string_pretty(value)
        .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to render JSON: {e}")))?;
    println!("{s}");
    Ok(())
}

fn load_bundle(path: &Path) -> Result<BundleView, ExitError> {
    if path.is_dir() {
        return load_bundle_from_dir(path);
    }
    if path.is_file() {
        return load_bundle_from_zip(path);
    }
    Err(ExitError::new(
        EXIT_GENERAL,
        format!("bundle path not found: {}", path.display()),
    ))
}

fn load_bundle_from_dir(root: &Path) -> Result<BundleView, ExitError> {
    fn walk(base: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) -> Result<(), ExitError> {
        let mut entries = fs::read_dir(dir)
            .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to read dir: {e}")))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to read dir entry: {e}")))?;
        entries.sort_by_key(|e| e.file_name());
        for ent in entries {
            let path = ent.path();
            if path.is_dir() {
                walk(base, &path, out)?;
            } else if path.is_file() {
                let rel = path
                    .strip_prefix(base)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .replace('\\', "/");
                let bytes = fs::read(&path).map_err(|e| {
                    ExitError::new(
                        EXIT_GENERAL,
                        format!("failed to read {}: {e}", path.display()),
                    )
                })?;
                out.insert(rel, bytes);
            }
        }
        Ok(())
    }

    let mut entries = BTreeMap::new();
    walk(root, root, &mut entries)?;
    Ok(BundleView { entries })
}

fn load_bundle_from_zip(path: &Path) -> Result<BundleView, ExitError> {
    let file = fs::File::open(path)
        .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to open zip: {e}")))?;
    let mut zip = ZipArchive::new(file)
        .map_err(|e| ExitError::new(EXIT_GENERAL, format!("invalid zip bundle: {e}")))?;

    let mut entries = BTreeMap::new();
    for i in 0..zip.len() {
        let mut f = zip.by_index(i).map_err(|e| {
            ExitError::new(
                EXIT_GENERAL,
                format!("failed to read zip entry at index {}: {e}", i),
            )
        })?;
        if f.is_dir() {
            continue;
        }
        let mut bytes = Vec::new();
        f.read_to_end(&mut bytes).map_err(|e| {
            ExitError::new(
                EXIT_GENERAL,
                format!("failed to read zip entry {}: {e}", f.name()),
            )
        })?;
        entries.insert(f.name().replace('\\', "/"), bytes);
    }
    Ok(BundleView { entries })
}

fn read_entry_text(view: &BundleView, key: &str) -> Result<String, ExitError> {
    let Some(bytes) = view.entries.get(key) else {
        return Err(ExitError::new(
            EXIT_GENERAL,
            format!("bundle entry not found: {}", key),
        ));
    };
    String::from_utf8(bytes.clone())
        .map_err(|_| ExitError::new(EXIT_GENERAL, format!("entry is not UTF-8 text: {}", key)))
}

fn resolve_part_key(raw: &str, view: &BundleView) -> Result<String, ExitError> {
    let normalized = raw.replace('\\', "/");
    let direct = if normalized.starts_with("parts/") {
        normalized.clone()
    } else {
        format!("parts/{}", normalized)
    };
    if view.entries.contains_key(&direct) {
        return Ok(direct);
    }
    if view.entries.contains_key(&normalized) {
        return Ok(normalized);
    }
    Err(ExitError::new(
        EXIT_GENERAL,
        format!("part not found: {}", raw),
    ))
}

fn print_list(path: &Path, view: &BundleView) {
    let parts = part_entries(view);
    let structured = structured_context(view);

    println!("diffship preview");
    println!("  bundle          : {}", path.display());
    println!(
        "  HANDOFF.md      : {}",
        yes_no(view.entries.contains_key("HANDOFF.md"))
    );
    println!(
        "  AI_REQUESTS.md  : {}",
        yes_no(view.entries.contains_key("AI_REQUESTS.md"))
    );
    println!("  parts           : {}", parts.len());
    for p in parts {
        println!("    - {}", p);
    }
    println!(
        "  attachments.zip : {}",
        yes_no(view.entries.contains_key("attachments.zip"))
    );
    println!(
        "  excluded.md     : {}",
        yes_no(view.entries.contains_key("excluded.md"))
    );
    println!(
        "  secrets.md      : {}",
        yes_no(view.entries.contains_key("secrets.md"))
    );
    if let Some(structured) = structured {
        println!(
            "  handoff.manifest.json : {}",
            yes_no(structured.manifest_json)
        );
        println!(
            "  handoff.context.xml  : {}",
            yes_no(structured.context_xml)
        );
        println!(
            "  part contexts        : {}",
            structured.part_contexts.len()
        );
        if let Some(project_context) = structured.project_context.as_ref() {
            println!(
                "  project.context.json : {}",
                yes_no(project_context.manifest_json)
            );
            println!(
                "  PROJECT_CONTEXT.md   : {}",
                yes_no(project_context.context_md)
            );
            println!(
                "  project snapshots    : {}",
                project_context.snapshot_files
            );
            if let Some(summary) = project_context.summary.as_ref() {
                println!(
                    "  project context      : selected={}, snapshots={}, omitted={}, bytes={}",
                    summary.selected_files,
                    summary.included_snapshots,
                    summary.omitted_files,
                    summary.total_snapshot_bytes
                );
            }
        }
        if let Some(summary) = structured.summary {
            println!(
                "  summary              : files={}, parts={}, commit-views={}",
                summary.file_count, summary.part_count, summary.commit_view_count
            );
            println!(
                "  categories           : {}",
                render_count_map(&summary.categories)
            );
            println!(
                "  segments             : {}",
                render_count_map(&summary.segments)
            );
            println!(
                "  statuses             : {}",
                render_count_map(&summary.statuses)
            );
        }
        if !structured.reading_order.is_empty() {
            println!("  reading order:");
            for item in &structured.reading_order {
                println!("    - {}", item);
            }
        }
        if !structured.task_groups.is_empty() {
            println!("  task groups:");
            for group in &structured.task_groups {
                println!(
                    "    - {}: intents={} parts={} files={}",
                    group.task_id,
                    render_string_list(&group.intent_labels),
                    render_string_list(&group.part_ids),
                    render_string_list(&group.top_files)
                );
            }
        }
        if let Some(semantic) = structured.semantic_facts {
            println!(
                "  semantic facts       : manifest-files={}, part-contexts={}/{}",
                yes_no(semantic.manifest_file_entries),
                semantic.part_context_entries,
                structured.part_contexts.len()
            );
        }
        if let Some(coarse) = structured.coarse_labels {
            println!(
                "  coarse labels        : manifest-files={}, part-contexts={}/{}",
                yes_no(coarse.manifest_file_entries),
                coarse.part_context_entries,
                structured.part_contexts.len()
            );
        }
        if let Some(change_hints) = structured.change_hints {
            println!(
                "  change hints         : manifest-files={}, part-contexts={}/{}",
                yes_no(change_hints.manifest_file_entries),
                change_hints.part_context_entries,
                structured.part_contexts.len()
            );
        }
        if let Some(scoped) = structured.scoped_context {
            println!(
                "  scoped part hints    : {}/{}",
                scoped.part_context_entries,
                structured.part_contexts.len()
            );
        }
    }
}

fn summary_json(path: &Path, view: &BundleView, mode: &'static str) -> PreviewSummary<'static> {
    PreviewSummary {
        bundle: path.display().to_string(),
        mode,
        handoff_md: view.entries.contains_key("HANDOFF.md"),
        ai_requests_md: view.entries.contains_key("AI_REQUESTS.md"),
        parts: part_entries(view),
        attachments_zip: view.entries.contains_key("attachments.zip"),
        excluded_md: view.entries.contains_key("excluded.md"),
        secrets_md: view.entries.contains_key("secrets.md"),
        structured_context: structured_context(view),
    }
}

fn part_entries(view: &BundleView) -> Vec<String> {
    let mut parts = view
        .entries
        .keys()
        .filter(|k| k.starts_with("parts/") && k.ends_with(".patch"))
        .cloned()
        .collect::<Vec<_>>();
    parts.sort();
    parts
}

fn part_context_entries(view: &BundleView) -> Vec<String> {
    let mut parts = view
        .entries
        .keys()
        .filter(|k| k.starts_with("parts/") && k.ends_with(".context.json"))
        .cloned()
        .collect::<Vec<_>>();
    parts.sort();
    parts
}

fn structured_context(view: &BundleView) -> Option<PreviewStructuredContext> {
    let manifest_json = view.entries.contains_key("handoff.manifest.json");
    let context_xml = view.entries.contains_key("handoff.context.xml");
    let part_contexts = part_context_entries(view);
    let project_context = project_context_preview(view);
    if !manifest_json && !context_xml && part_contexts.is_empty() && project_context.is_none() {
        return None;
    }
    let manifest = manifest_preview(view);
    let rendered_part_contexts = part_contexts
        .iter()
        .filter_map(|path| part_context_preview(view, path))
        .collect::<Vec<_>>();
    let semantic_facts = Some(PreviewSemanticFacts {
        manifest_file_entries: manifest
            .as_ref()
            .is_some_and(|value| value.has_file_semantics),
        part_context_entries: rendered_part_contexts
            .iter()
            .filter(|value| value.has_file_semantics)
            .count(),
    });
    let coarse_labels = Some(PreviewCoarseLabelFacts {
        manifest_file_entries: manifest
            .as_ref()
            .is_some_and(|value| value.has_coarse_labels),
        part_context_entries: rendered_part_contexts
            .iter()
            .filter(|value| value.has_coarse_labels)
            .count(),
    });
    let change_hints = Some(PreviewChangeHintFacts {
        manifest_file_entries: manifest
            .as_ref()
            .is_some_and(|value| value.has_change_hints),
        part_context_entries: rendered_part_contexts
            .iter()
            .filter(|value| value.has_change_hints)
            .count(),
    });
    let scoped_context = Some(PreviewScopedContextFacts {
        part_context_entries: rendered_part_contexts
            .iter()
            .filter(|value| value.has_scoped_context)
            .count(),
    });

    Some(PreviewStructuredContext {
        manifest_json,
        context_xml,
        part_contexts,
        project_context,
        summary: manifest.as_ref().map(|value| value.summary.clone()),
        reading_order: manifest
            .as_ref()
            .map(|value| value.reading_order.clone())
            .unwrap_or_default(),
        task_groups: manifest.map(|value| value.task_groups).unwrap_or_default(),
        semantic_facts,
        coarse_labels,
        change_hints,
        scoped_context,
    })
}

fn project_context_preview(view: &BundleView) -> Option<PreviewProjectContext> {
    let manifest_json = view.entries.contains_key("project.context.json");
    let context_md = view.entries.contains_key("PROJECT_CONTEXT.md");
    let snapshot_files = view
        .entries
        .keys()
        .filter(|key| key.starts_with("project_context/files/"))
        .count();
    if !manifest_json && !context_md && snapshot_files == 0 {
        return None;
    }
    let summary = view
        .entries
        .get("project.context.json")
        .and_then(|bytes| serde_json::from_slice::<ProjectContextPreviewEnvelope>(bytes).ok())
        .map(|value| PreviewProjectContextSummary {
            selected_files: value.summary.selected_files,
            included_snapshots: value.summary.included_snapshots,
            omitted_files: value.summary.omitted_files,
            total_snapshot_bytes: value.summary.total_snapshot_bytes,
        });
    Some(PreviewProjectContext {
        manifest_json,
        context_md,
        snapshot_files,
        summary,
    })
}

fn manifest_preview(view: &BundleView) -> Option<ManifestPreviewRendered> {
    let bytes = view.entries.get("handoff.manifest.json")?;
    let raw = serde_json::from_slice::<ManifestPreviewSummaryEnvelope>(bytes).ok()?;
    Some(ManifestPreviewRendered {
        summary: PreviewManifestSummary {
            file_count: raw.summary.file_count,
            part_count: raw.summary.part_count,
            commit_view_count: raw.summary.commit_view_count,
            categories: BTreeMap::from([
                ("docs".to_string(), raw.summary.categories.docs),
                ("config".to_string(), raw.summary.categories.config),
                ("source".to_string(), raw.summary.categories.source),
                ("tests".to_string(), raw.summary.categories.tests),
                ("other".to_string(), raw.summary.categories.other),
            ]),
            segments: raw.summary.segments,
            statuses: raw.summary.statuses,
        },
        reading_order: raw.reading_order,
        task_groups: raw
            .task_groups
            .into_iter()
            .map(|group| PreviewTaskGroup {
                task_id: group.task_id,
                intent_labels: group.intent_labels,
                part_ids: group.part_ids,
                segments: group.segments,
                top_files: group.top_files,
                part_count: group.part_count,
                file_count: group.file_count,
            })
            .collect(),
        has_file_semantics: raw.files.iter().any(|file| file.semantic.is_some()),
        has_coarse_labels: raw.files.iter().any(|file| {
            file.semantic
                .as_ref()
                .and_then(|value| value.get("coarse_labels"))
                .is_some()
        }),
        has_change_hints: raw.files.iter().any(|file| file.change_hints.is_some()),
    })
}

struct ManifestPreviewRendered {
    summary: PreviewManifestSummary,
    reading_order: Vec<String>,
    task_groups: Vec<PreviewTaskGroup>,
    has_file_semantics: bool,
    has_coarse_labels: bool,
    has_change_hints: bool,
}

#[derive(Debug, Deserialize)]
struct ManifestPreviewSummaryEnvelope {
    summary: ManifestPreviewSummaryRaw,
    #[serde(default)]
    reading_order: Vec<String>,
    #[serde(default)]
    task_groups: Vec<ManifestPreviewTaskGroupRaw>,
    #[serde(default)]
    files: Vec<ManifestPreviewFileRaw>,
}

#[derive(Debug, Deserialize)]
struct ManifestPreviewFileRaw {
    #[serde(default)]
    semantic: Option<serde_json::Value>,
    #[serde(default)]
    change_hints: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct PartContextPreviewEnvelope {
    #[serde(default)]
    files: Vec<PartContextPreviewFileRaw>,
    #[serde(default)]
    scoped_context: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct PartContextPreviewFileRaw {
    #[serde(default)]
    semantic: Option<serde_json::Value>,
    #[serde(default)]
    change_hints: Option<serde_json::Value>,
}

struct PartContextPreviewRendered {
    has_file_semantics: bool,
    has_coarse_labels: bool,
    has_change_hints: bool,
    has_scoped_context: bool,
}

fn part_context_preview(view: &BundleView, path: &str) -> Option<PartContextPreviewRendered> {
    let bytes = view.entries.get(path)?;
    let raw = serde_json::from_slice::<PartContextPreviewEnvelope>(bytes).ok()?;
    Some(PartContextPreviewRendered {
        has_file_semantics: raw.files.iter().any(|file| file.semantic.is_some()),
        has_coarse_labels: raw.files.iter().any(|file| {
            file.semantic
                .as_ref()
                .and_then(|value| value.get("coarse_labels"))
                .is_some()
        }),
        has_change_hints: raw.files.iter().any(|file| file.change_hints.is_some()),
        has_scoped_context: raw.scoped_context.is_some(),
    })
}

fn render_count_map(counts: &BTreeMap<String, u64>) -> String {
    counts
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn render_string_list(items: &[String]) -> String {
    if items.is_empty() {
        "-".to_string()
    } else {
        items.join(",")
    }
}

fn yes_no(v: bool) -> &'static str {
    if v { "yes" } else { "no" }
}

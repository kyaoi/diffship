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
    summary: Option<PreviewManifestSummary>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    reading_order: Vec<String>,
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
struct ManifestCategoryCountsRaw {
    docs: u64,
    config: u64,
    source: u64,
    tests: u64,
    other: u64,
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
    }
}

fn summary_json(path: &Path, view: &BundleView, mode: &'static str) -> PreviewSummary<'static> {
    PreviewSummary {
        bundle: path.display().to_string(),
        mode,
        handoff_md: view.entries.contains_key("HANDOFF.md"),
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
    if !manifest_json && !context_xml && part_contexts.is_empty() {
        return None;
    }
    let manifest = manifest_preview(view);

    Some(PreviewStructuredContext {
        manifest_json,
        context_xml,
        part_contexts,
        summary: manifest.as_ref().map(|value| value.summary.clone()),
        reading_order: manifest
            .map(|value| value.reading_order)
            .unwrap_or_default(),
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
    })
}

struct ManifestPreviewRendered {
    summary: PreviewManifestSummary,
    reading_order: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ManifestPreviewSummaryEnvelope {
    summary: ManifestPreviewSummaryRaw,
    #[serde(default)]
    reading_order: Vec<String>,
}

fn render_count_map(counts: &BTreeMap<String, u64>) -> String {
    counts
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn yes_no(v: bool) -> &'static str {
    if v { "yes" } else { "no" }
}

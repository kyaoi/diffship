use crate::cli::CompareArgs;
use crate::exit::{EXIT_GENERAL, ExitError};
use crate::pathing::resolve_user_path;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Read;
use std::path::Path;
use zip::ZipArchive;

#[derive(Debug, Serialize)]
struct CompareReport {
    bundle_a: String,
    bundle_b: String,
    mode: String,
    equivalent: bool,
    areas: BTreeMap<String, usize>,
    kinds: BTreeMap<String, usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    structured_context: Option<CompareStructuredContext>,
    diffs: Vec<CompareDiff>,
}

#[derive(Debug, Serialize, Clone)]
struct CompareDiff {
    area: String,
    kind: String,
    path: String,
    detail: String,
}

#[derive(Debug, Serialize)]
struct CompareStructuredContext {
    manifest_a: bool,
    manifest_b: bool,
    summary_diffs: Vec<CompareSummaryDiff>,
    reading_order_diffs: Vec<CompareTextDiff>,
}

#[derive(Debug, Serialize)]
struct CompareSummaryDiff {
    key: String,
    a: u64,
    b: u64,
}

#[derive(Debug, Serialize)]
struct CompareTextDiff {
    key: String,
    a: String,
    b: String,
}

#[derive(Debug, Deserialize)]
struct ManifestSummaryEnvelope {
    summary: ManifestSummaryRaw,
    #[serde(default)]
    reading_order: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ManifestSummaryRaw {
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

pub fn cmd(args: CompareArgs) -> Result<(), ExitError> {
    let cwd = std::env::current_dir()
        .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to detect current dir: {e}")))?;
    let a_path = resolve_user_path(&cwd, &args.bundle_a)?;
    let b_path = resolve_user_path(&cwd, &args.bundle_b)?;

    let a = load_bundle(&a_path)?;
    let b = load_bundle(&b_path)?;

    let mut diffs = Vec::new();
    let mut areas = BTreeMap::new();
    let mut kinds = BTreeMap::new();
    let mut keys = BTreeSet::new();
    keys.extend(a.keys().cloned());
    keys.extend(b.keys().cloned());

    for k in keys {
        match (a.get(&k), b.get(&k)) {
            (Some(_), None) => push_diff(
                &mut diffs,
                &mut areas,
                &mut kinds,
                &k,
                "only_in_a",
                format!("only in A: {k}"),
            ),
            (None, Some(_)) => push_diff(
                &mut diffs,
                &mut areas,
                &mut kinds,
                &k,
                "only_in_b",
                format!("only in B: {k}"),
            ),
            (Some(ba), Some(bb)) => {
                let left = normalize_entry(&k, ba, args.strict);
                let right = normalize_entry(&k, bb, args.strict);
                if left != right {
                    push_diff(
                        &mut diffs,
                        &mut areas,
                        &mut kinds,
                        &k,
                        "content_differs",
                        format!("content differs: {k}"),
                    );
                }
            }
            (None, None) => {}
        }
    }

    let structured_context = compare_structured_context(&a, &b);

    let report = CompareReport {
        bundle_a: a_path.display().to_string(),
        bundle_b: b_path.display().to_string(),
        mode: if args.strict {
            "strict".to_string()
        } else {
            "normalized".to_string()
        },
        equivalent: diffs.is_empty(),
        areas: areas.clone(),
        kinds: kinds.clone(),
        structured_context,
        diffs: diffs.clone(),
    };
    if args.json {
        print_json(&report)?;
        if diffs.is_empty() {
            return Ok(());
        }
        return Err(ExitError::new(
            EXIT_GENERAL,
            "bundle comparison failed (see JSON diff output)",
        ));
    }

    if diffs.is_empty() {
        println!("diffship compare: equivalent");
        println!("  A: {}", a_path.display());
        println!("  B: {}", b_path.display());
        println!(
            "  mode: {}",
            if args.strict { "strict" } else { "normalized" }
        );
        return Ok(());
    }

    eprintln!("diffship compare: different");
    eprintln!(
        "  mode: {}",
        if args.strict { "strict" } else { "normalized" }
    );
    if !areas.is_empty() {
        eprintln!("  areas: {}", render_count_map(&areas));
    }
    if !kinds.is_empty() {
        eprintln!("  kinds: {}", render_count_map(&kinds));
    }
    if let Some(structured) = &report.structured_context
        && !structured.summary_diffs.is_empty()
    {
        eprintln!("  manifest summary diffs:");
        for diff in &structured.summary_diffs {
            eprintln!("    - {}: {} -> {}", diff.key, diff.a, diff.b);
        }
    }
    if let Some(structured) = &report.structured_context
        && !structured.reading_order_diffs.is_empty()
    {
        eprintln!("  manifest reading-order diffs:");
        for diff in &structured.reading_order_diffs {
            eprintln!("    - {}: {:?} -> {:?}", diff.key, diff.a, diff.b);
        }
    }
    for d in &diffs {
        eprintln!("- [{}/{}] {}", d.area, d.kind, d.path);
    }
    Err(ExitError::new(
        EXIT_GENERAL,
        "bundle comparison failed (see diff list above)",
    ))
}

fn push_diff(
    diffs: &mut Vec<CompareDiff>,
    areas: &mut BTreeMap<String, usize>,
    kinds: &mut BTreeMap<String, usize>,
    path: &str,
    kind: &str,
    detail: String,
) {
    let area = classify_area(path).to_string();
    *areas.entry(area.clone()).or_insert(0) += 1;
    *kinds.entry(kind.to_string()).or_insert(0) += 1;
    diffs.push(CompareDiff {
        area,
        kind: kind.to_string(),
        path: path.to_string(),
        detail,
    });
}

fn classify_area(path: &str) -> &'static str {
    if path == "HANDOFF.md" || path == "handoff.manifest.json" {
        "handoff"
    } else if path == "handoff.context.xml" {
        "context"
    } else if path.starts_with("parts/") && path.ends_with(".patch") {
        "patch"
    } else if path.starts_with("parts/") && path.ends_with(".context.json") {
        "context"
    } else if path == "attachments.zip" {
        "attachments"
    } else if path == "excluded.md" {
        "excluded"
    } else if path == "secrets.md" {
        "secrets"
    } else if path == "plan.toml" {
        "plan"
    } else {
        "other"
    }
}

fn render_count_map(counts: &BTreeMap<String, usize>) -> String {
    counts
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn compare_structured_context(
    a: &BTreeMap<String, Vec<u8>>,
    b: &BTreeMap<String, Vec<u8>>,
) -> Option<CompareStructuredContext> {
    let a_summary = manifest_summary(a);
    let b_summary = manifest_summary(b);
    let manifest_a = a_summary.is_some();
    let manifest_b = b_summary.is_some();
    if !manifest_a && !manifest_b {
        return None;
    }

    let mut summary_diffs = Vec::new();
    let mut reading_order_diffs = Vec::new();
    if let (Some(a_summary), Some(b_summary)) = (a_summary, b_summary) {
        push_summary_diff(
            &mut summary_diffs,
            "file_count",
            a_summary.summary.file_count,
            b_summary.summary.file_count,
        );
        push_summary_diff(
            &mut summary_diffs,
            "part_count",
            a_summary.summary.part_count,
            b_summary.summary.part_count,
        );
        push_summary_diff(
            &mut summary_diffs,
            "commit_view_count",
            a_summary.summary.commit_view_count,
            b_summary.summary.commit_view_count,
        );
        compare_named_counts(
            &mut summary_diffs,
            "categories",
            BTreeMap::from([
                ("docs".to_string(), a_summary.summary.categories.docs),
                ("config".to_string(), a_summary.summary.categories.config),
                ("source".to_string(), a_summary.summary.categories.source),
                ("tests".to_string(), a_summary.summary.categories.tests),
                ("other".to_string(), a_summary.summary.categories.other),
            ]),
            BTreeMap::from([
                ("docs".to_string(), b_summary.summary.categories.docs),
                ("config".to_string(), b_summary.summary.categories.config),
                ("source".to_string(), b_summary.summary.categories.source),
                ("tests".to_string(), b_summary.summary.categories.tests),
                ("other".to_string(), b_summary.summary.categories.other),
            ]),
        );
        compare_named_counts(
            &mut summary_diffs,
            "segments",
            a_summary.summary.segments,
            b_summary.summary.segments,
        );
        compare_named_counts(
            &mut summary_diffs,
            "statuses",
            a_summary.summary.statuses,
            b_summary.summary.statuses,
        );
        compare_string_lists(
            &mut reading_order_diffs,
            "reading_order",
            &a_summary.reading_order,
            &b_summary.reading_order,
        );
    }

    Some(CompareStructuredContext {
        manifest_a,
        manifest_b,
        summary_diffs,
        reading_order_diffs,
    })
}

fn manifest_summary(entries: &BTreeMap<String, Vec<u8>>) -> Option<ManifestSummaryEnvelope> {
    let bytes = entries.get("handoff.manifest.json")?;
    serde_json::from_slice::<ManifestSummaryEnvelope>(bytes).ok()
}

fn compare_named_counts(
    out: &mut Vec<CompareSummaryDiff>,
    prefix: &str,
    a: BTreeMap<String, u64>,
    b: BTreeMap<String, u64>,
) {
    let mut keys = BTreeSet::new();
    keys.extend(a.keys().cloned());
    keys.extend(b.keys().cloned());
    for key in keys {
        let a_value = a.get(&key).copied().unwrap_or(0);
        let b_value = b.get(&key).copied().unwrap_or(0);
        push_summary_diff(out, &format!("{prefix}.{key}"), a_value, b_value);
    }
}

fn push_summary_diff(out: &mut Vec<CompareSummaryDiff>, key: &str, a: u64, b: u64) {
    if a != b {
        out.push(CompareSummaryDiff {
            key: key.to_string(),
            a,
            b,
        });
    }
}

fn compare_string_lists(out: &mut Vec<CompareTextDiff>, prefix: &str, a: &[String], b: &[String]) {
    let max_len = a.len().max(b.len());
    for idx in 0..max_len {
        let left = a
            .get(idx)
            .cloned()
            .unwrap_or_else(|| "(missing)".to_string());
        let right = b
            .get(idx)
            .cloned()
            .unwrap_or_else(|| "(missing)".to_string());
        if left != right {
            out.push(CompareTextDiff {
                key: format!("{prefix}[{idx}]"),
                a: left,
                b: right,
            });
        }
    }
}

fn print_json<T: Serialize>(value: &T) -> Result<(), ExitError> {
    let s = serde_json::to_string_pretty(value)
        .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to render JSON: {e}")))?;
    println!("{s}");
    Ok(())
}

fn load_bundle(path: &Path) -> Result<BTreeMap<String, Vec<u8>>, ExitError> {
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

fn load_bundle_from_dir(root: &Path) -> Result<BTreeMap<String, Vec<u8>>, ExitError> {
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

    let mut out = BTreeMap::new();
    walk(root, root, &mut out)?;
    Ok(out)
}

fn load_bundle_from_zip(path: &Path) -> Result<BTreeMap<String, Vec<u8>>, ExitError> {
    let file = fs::File::open(path)
        .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to open zip: {e}")))?;
    let mut zip = ZipArchive::new(file)
        .map_err(|e| ExitError::new(EXIT_GENERAL, format!("invalid zip bundle: {e}")))?;

    let mut out = BTreeMap::new();
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
        out.insert(f.name().replace('\\', "/"), bytes);
    }
    Ok(out)
}

fn normalize_entry(path: &str, bytes: &[u8], strict: bool) -> Vec<u8> {
    if strict {
        return bytes.to_vec();
    }

    if path == "HANDOFF.md"
        && let Ok(s) = String::from_utf8(bytes.to_vec())
    {
        return normalize_handoff(&s).into_bytes();
    }
    if path.starts_with("parts/")
        && path.ends_with(".patch")
        && let Ok(s) = String::from_utf8(bytes.to_vec())
    {
        return normalize_patch(&s).into_bytes();
    }
    bytes.to_vec()
}

fn normalize_handoff(s: &str) -> String {
    let s = replace_hex40_runs(s);
    let mut lines = Vec::new();
    for line in s.lines() {
        if line.starts_with("- Bundle: `") {
            lines.push("- Bundle: `<BUNDLE>`".to_string());
            continue;
        }
        if line.starts_with("| `part_") {
            let mut cols = line.split('|').map(str::to_string).collect::<Vec<_>>();
            if cols.len() == 7 {
                cols[4] = " <BYTES> ".to_string();
                lines.push(cols.join("|"));
                continue;
            }
        }
        if line.trim_start().starts_with("- approx bytes: `") {
            lines.push("- approx bytes: `<BYTES>`".to_string());
            continue;
        }
        lines.push(line.to_string());
    }
    let mut out = lines.join("\n");
    out.push('\n');
    out
}

fn normalize_patch(s: &str) -> String {
    let mut out = String::new();
    for line in replace_hex40_runs(s).lines() {
        out.push_str(line);
        out.push('\n');
    }
    out
}

fn replace_hex40_runs(s: &str) -> String {
    let chars = s.chars().collect::<Vec<_>>();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < chars.len() {
        let end = i + 40;
        if end <= chars.len() && chars[i..end].iter().all(|c| c.is_ascii_hexdigit()) {
            out.push_str("<HEX40>");
            i = end;
            continue;
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

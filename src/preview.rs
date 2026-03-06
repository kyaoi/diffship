use crate::cli::PreviewArgs;
use crate::exit::{EXIT_GENERAL, ExitError};
use serde::Serialize;
use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
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
}

#[derive(Debug, Serialize)]
struct PreviewText {
    bundle: String,
    entry: String,
    text: String,
}

pub fn cmd(args: PreviewArgs) -> Result<(), ExitError> {
    let bundle_path = PathBuf::from(&args.bundle);
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

fn yes_no(v: bool) -> &'static str {
    if v { "yes" } else { "no" }
}

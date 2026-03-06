use crate::cli::CompareArgs;
use crate::exit::{EXIT_GENERAL, ExitError};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use zip::ZipArchive;

pub fn cmd(args: CompareArgs) -> Result<(), ExitError> {
    let a_path = PathBuf::from(&args.bundle_a);
    let b_path = PathBuf::from(&args.bundle_b);

    let a = load_bundle(&a_path)?;
    let b = load_bundle(&b_path)?;

    let mut diffs = Vec::new();
    let mut keys = BTreeSet::new();
    keys.extend(a.keys().cloned());
    keys.extend(b.keys().cloned());

    for k in keys {
        match (a.get(&k), b.get(&k)) {
            (Some(_), None) => diffs.push(format!("- only in A: {}", k)),
            (None, Some(_)) => diffs.push(format!("- only in B: {}", k)),
            (Some(ba), Some(bb)) => {
                let left = normalize_entry(&k, ba, args.strict);
                let right = normalize_entry(&k, bb, args.strict);
                if left != right {
                    diffs.push(format!("- content differs: {}", k));
                }
            }
            (None, None) => {}
        }
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
    for d in &diffs {
        eprintln!("{}", d);
    }
    Err(ExitError::new(
        EXIT_GENERAL,
        "bundle comparison failed (see diff list above)",
    ))
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

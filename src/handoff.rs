use crate::cli::BuildArgs;
use crate::exit::{EXIT_GENERAL, ExitError};
use crate::git;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use time::format_description;
use zip::ZipWriter;
use zip::write::FileOptions;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RangeMode {
    Direct,
    MergeBase,
    Last,
    Root,
}

#[derive(Debug, Clone)]
struct RangePlan {
    mode: RangeMode,
    // Base and target are revision-ish strings acceptable to `git diff`.
    base: String,
    target: String,
    // For display in HANDOFF.md
    from_rev: Option<String>,
    to_rev: Option<String>,
    a_rev: Option<String>,
    b_rev: Option<String>,
    merge_base: Option<String>,
    commit_count: Option<u64>,
}

#[derive(Debug, Clone)]
struct FileRow {
    status: String,
    path: String,
    note: String,
    ins: Option<u64>,
    del: Option<u64>,
    bytes: Option<u64>,
    part: String,
}

pub fn cmd(git_root: &Path, args: BuildArgs) -> Result<(), ExitError> {
    let cwd = std::env::current_dir()
        .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to detect current dir: {e}")))?;

    let out_dir = match &args.out {
        Some(o) => {
            let p = PathBuf::from(o);
            if p.is_absolute() { p } else { cwd.join(p) }
        }
        None => cwd.join(format!("diffship_{}", timestamp_yyyymmdd_hhmm()?)),
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

    let head = git::rev_parse(git_root, "HEAD")?;
    let plan = build_range_plan(git_root, &args)?;

    // MVP: committed-only patch in a single part.
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

    let part_name = "part_01.patch";
    let part_rel = format!("parts/{part_name}");
    let part_path = parts_dir.join(part_name);
    write_text_file(&part_path, &patch)?;

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
    let mut rows = parse_name_status(&name_status, &insdel_map);

    for r in &mut rows {
        if r.status == "D" {
            r.bytes = Some(0);
            continue;
        }
        r.bytes = git_cat_file_size(git_root, &plan.target, &r.path).ok();
        r.part = part_name.to_string();
    }

    rows.sort_by(|a, b| a.path.cmp(&b.path));

    let changed_paths: Vec<String> = rows.iter().map(|r| r.path.clone()).collect();
    let changed_tree = render_changed_tree(&changed_paths);
    let parts_index = render_parts_index(&patch, &rows, part_name);
    let (cat_summary, reading_order) = render_category_summary_and_reading_order(&rows);

    let handoff = render_handoff_md(&HandoffDocInputs {
        out_dir: &out_dir,
        plan: &plan,
        head: &head,
        changed_tree: &changed_tree,
        rows: &rows,
        cat_summary: &cat_summary,
        parts_index: &parts_index,
        reading_order: &reading_order,
        part_rel: &part_rel,
    });

    write_text_file(&out_dir.join("HANDOFF.md"), &handoff)?;

    let mut zip_path = None;
    if args.zip {
        let zp = out_dir.with_extension("zip");
        write_zip_from_dir(&out_dir, &zp)?;
        zip_path = Some(zp);
    }

    println!("diffship build: created {}", out_dir.display());
    if let Some(zp) = zip_path {
        println!("diffship build: created {}", zp.display());
    }

    Ok(())
}

fn timestamp_yyyymmdd_hhmm() -> Result<String, ExitError> {
    let now = time::OffsetDateTime::now_utc();
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
    let s = out.trim();
    s.parse::<u64>().map_err(|e| {
        ExitError::new(
            EXIT_GENERAL,
            format!("failed to parse rev-list --count output '{s}': {e}"),
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

    let s = String::from_utf8_lossy(&output.stdout);
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Err(ExitError::new(
            EXIT_GENERAL,
            "git mktree returned empty output",
        ));
    }

    Ok(trimmed.to_string())
}

fn git_cat_file_size(git_root: &Path, target_commit: &str, path: &str) -> Result<u64, ExitError> {
    // NOTE: This is best-effort; errors should not fail the whole build.
    let spec = format!("{}:{}", target_commit, path);
    let out = git::run_git(git_root, ["cat-file", "-s", spec.as_str()])?;
    let s = out.trim();
    s.parse::<u64>().map_err(|e| {
        ExitError::new(
            EXIT_GENERAL,
            format!("failed to parse cat-file -s output '{s}': {e}"),
        )
    })
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

fn parse_numstat(s: &str) -> HashMap<String, (Option<u64>, Option<u64>)> {
    let mut map = HashMap::new();
    for line in s.lines() {
        if line.trim().is_empty() {
            continue;
        }
        // numstat is TAB-separated: ins<TAB>del<TAB>path
        let mut parts = line.split('\t');
        let ins_s = parts.next().unwrap_or("");
        let del_s = parts.next().unwrap_or("");
        let path = parts.next().unwrap_or("").trim();
        if path.is_empty() {
            continue;
        }
        let ins = ins_s.parse::<u64>().ok();
        let del = del_s.parse::<u64>().ok();
        map.insert(path.to_string(), (ins, del));
    }
    map
}

fn parse_name_status(
    name_status: &str,
    insdel: &HashMap<String, (Option<u64>, Option<u64>)>,
) -> Vec<FileRow> {
    let mut rows = vec![];

    for line in name_status.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // name-status is TAB-separated by default.
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
            let p = parts.next().unwrap_or("").to_string();
            (p, String::new())
        };

        let st = status.chars().next().unwrap_or('?').to_string();
        let (ins, del) = insdel.get(&path).cloned().unwrap_or((None, None));

        rows.push(FileRow {
            status: st,
            path,
            note,
            ins,
            del,
            bytes: None,
            part: "".to_string(),
        });
    }

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

fn render_parts_index(patch: &str, rows: &[FileRow], part_name: &str) -> String {
    let approx_bytes = patch.len();
    let mut top = rows.iter().map(|r| r.path.clone()).collect::<Vec<_>>();
    top.sort();
    if top.len() > 8 {
        top.truncate(8);
    }

    let mut s = String::new();
    s.push_str(&format!("### {part_name}\n"));
    s.push_str(&format!("- approx bytes: `{approx_bytes}`\n"));
    s.push_str("- segments: `committed`\n");
    s.push_str("- top files:\n");
    for p in top {
        s.push_str(&format!("  - `{}`\n", p));
    }

    s
}

fn render_category_summary_and_reading_order(rows: &[FileRow]) -> (String, Vec<String>) {
    let mut docs = vec![];
    let mut cfg = vec![];
    let mut src = vec![];
    let mut tests = vec![];
    let mut other = vec![];

    for r in rows {
        let p = r.path.as_str();
        if p.starts_with("docs/") || p.ends_with(".md") {
            docs.push(r.path.clone());
        } else if p.starts_with("src/") {
            src.push(r.path.clone());
        } else if p.starts_with("tests/") {
            tests.push(r.path.clone());
        } else if p.starts_with(".github/")
            || p.ends_with(".toml")
            || p.ends_with(".yml")
            || p.ends_with(".yaml")
            || p.ends_with(".json")
            || p.ends_with(".lock")
        {
            cfg.push(r.path.clone());
        } else {
            other.push(r.path.clone());
        }
    }

    docs.sort();
    cfg.sort();
    src.sort();
    tests.sort();
    other.sort();

    let mut s = String::new();
    s.push_str(&format!(
        "- Docs: `{}` files → parts: `part_01`\n",
        docs.len()
    ));
    s.push_str(&format!(
        "- Config/CI: `{}` files → parts: `part_01`\n",
        cfg.len()
    ));
    s.push_str(&format!(
        "- Source: `{}` files → parts: `part_01`\n",
        src.len()
    ));
    s.push_str(&format!(
        "- Tests: `{}` files → parts: `part_01`\n",
        tests.len()
    ));
    s.push_str(&format!(
        "- Other: `{}` files → parts: `part_01`\n",
        other.len()
    ));

    // reading order: choose representative lists (paths), but keep it short.
    let mut order = vec![];
    if !docs.is_empty() {
        order.push(format!("Docs changes: `part_01` ({} files)", docs.len()));
    }
    if !cfg.is_empty() {
        order.push(format!(
            "Config/build changes: `part_01` ({} files)",
            cfg.len()
        ));
    }
    if !src.is_empty() {
        order.push(format!("Source changes: `part_01` ({} files)", src.len()));
    }
    if !tests.is_empty() {
        order.push(format!("Tests: `part_01` ({} files)", tests.len()));
    }
    if order.is_empty() {
        order.push("No file changes detected".to_string());
    }

    (s, order)
}

struct HandoffDocInputs<'a> {
    out_dir: &'a Path,
    plan: &'a RangePlan,
    head: &'a str,
    changed_tree: &'a str,
    rows: &'a [FileRow],
    cat_summary: &'a str,
    parts_index: &'a str,
    reading_order: &'a [String],
    part_rel: &'a str,
}

fn render_handoff_md(inp: &HandoffDocInputs<'_>) -> String {
    let bundle_name = inp
        .out_dir
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| inp.out_dir.display().to_string());

    let (range_mode, range_desc) = match inp.plan.mode {
        RangeMode::Direct => (
            "direct",
            format!(
                "from/to: `{}` → `{}`",
                inp.plan.from_rev.as_deref().unwrap_or("?"),
                inp.plan.to_rev.as_deref().unwrap_or("?"),
            ),
        ),
        RangeMode::MergeBase => (
            "merge-base",
            format!(
                "a/b: `{}` / `{}` (merge-base: `{}`)",
                inp.plan.a_rev.as_deref().unwrap_or("?"),
                inp.plan.b_rev.as_deref().unwrap_or("?"),
                inp.plan.merge_base.as_deref().unwrap_or("?"),
            ),
        ),
        RangeMode::Last => ("last", "HEAD~1..HEAD".to_string()),
        RangeMode::Root => (
            "root",
            format!(
                "empty-tree → `{}`",
                inp.plan.to_rev.as_deref().unwrap_or("HEAD")
            ),
        ),
    };

    let mut s = String::new();

    // TL;DR
    s.push_str("## TL;DR\n");
    s.push_str(&format!("- Bundle: `{}`\n", bundle_name));
    s.push_str("- Profile: `mvp` (single part; no size-based splitting yet)\n");
    s.push_str(
        "- Segments included: committed=`yes`, staged=`no`, unstaged=`no`, untracked=`no`\n",
    );
    s.push_str(&format!(
        "- Committed range: `{}` ({})\n",
        range_mode, range_desc
    ));
    if let Some(n) = inp.plan.commit_count {
        s.push_str(&format!("- Commit count (approx): `{}`\n", n));
    }
    s.push_str(&format!(
        "- Current HEAD (workspace base): `{}`\n",
        inp.head.trim()
    ));
    s.push_str("- Reading order:\n");
    for (i, line) in inp.reading_order.iter().enumerate() {
        s.push_str(&format!("  {}. {}\n", i + 1, line));
    }

    s.push_str("\n---\n\n");

    // 1) Range & Sources Summary
    s.push_str("## 1) Range & Sources Summary\n");
    s.push_str("### Committed range\n");
    s.push_str(&format!("- mode: `{}`\n", range_mode));
    s.push_str(&format!("- {range_desc}\n"));
    if let Some(mb) = inp.plan.merge_base.as_deref() {
        s.push_str(&format!("- merge-base: `{}`\n", mb));
    }
    if let Some(n) = inp.plan.commit_count {
        s.push_str(&format!("- commit count: `{}`\n", n));
    }

    s.push_str("\n### Current workspace base (for uncommitted segments)\n");
    s.push_str(&format!("- HEAD: `{}`\n", inp.head.trim()));
    s.push_str("- staged: `no`\n- unstaged: `no`\n- untracked: `no`\n");

    // 2) Change Map
    s.push_str("\n---\n\n");
    s.push_str("## 2) Change Map\n\n");

    s.push_str("### 2.1 Changed Tree (changed files only)\n");
    s.push_str("```text\n");
    s.push_str(inp.changed_tree);
    if !inp.changed_tree.ends_with('\n') {
        s.push('\n');
    }
    s.push_str("```\n\n");

    s.push_str("### 2.2 File Table (part mapping)\n");
    s.push_str("| segment | status | path | ins | del | bytes | part | note |\n");
    s.push_str("|---|---:|---|---:|---:|---:|---|---|\n");
    for r in inp.rows {
        let ins = r
            .ins
            .map(|v| v.to_string())
            .unwrap_or_else(|| "".to_string());
        let del = r
            .del
            .map(|v| v.to_string())
            .unwrap_or_else(|| "".to_string());
        let bytes = r
            .bytes
            .map(|v| v.to_string())
            .unwrap_or_else(|| "".to_string());
        s.push_str(&format!(
            "| committed | {} | `{}` | {} | {} | {} | {} | {} |\n",
            r.status,
            r.path,
            ins,
            del,
            bytes,
            r.part,
            if r.note.is_empty() {
                ""
            } else {
                r.note.as_str()
            },
        ));
    }

    s.push_str("\n### 2.3 Category Summary\n");
    s.push_str(inp.cat_summary);

    // 3) Parts Index
    s.push_str("\n---\n\n");
    s.push_str("## 3) Parts Index\n\n");
    s.push_str(inp.parts_index);

    // Minimal explicit pointer for parts
    s.push_str("\n---\n\n");
    s.push_str("## Where to start\n\n");
    s.push_str(&format!("Open `{}` first.\n", "HANDOFF.md"));
    s.push_str(&format!("Then apply/read `{}`.\n", inp.part_rel));

    // Extra hint for automation
    s.push_str("\n---\n\n");
    s.push_str("## Notes\n");
    s.push_str("- This is an MVP committed-only bundle. Staged/unstaged/untracked sources and size-based splitting are planned.\n");

    s
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

    let opts = FileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o644);

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

    // Deterministic order
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
                ExitError::new(EXIT_GENERAL, format!("zip: start_file failed: {e}"))
            })?;
            zip.write_all(&bytes)
                .map_err(|e| ExitError::new(EXIT_GENERAL, format!("zip: write failed: {e}")))?;
        }
    }

    Ok(())
}

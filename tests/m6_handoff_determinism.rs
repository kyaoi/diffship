use assert_cmd::prelude::*;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn init_repo() -> tempfile::TempDir {
    let td = tempfile::tempdir().expect("tempdir");
    let root = td.path();

    Command::new("git")
        .args(["init", "-q"])
        .current_dir(root)
        .assert()
        .success();

    Command::new("git")
        .args(["config", "user.email", "aoistudy90@gmail.com"])
        .current_dir(root)
        .assert()
        .success();
    Command::new("git")
        .args(["config", "user.name", "kyaoi"])
        .current_dir(root)
        .assert()
        .success();

    td
}

fn commit_all_at(root: &Path, msg: &str, date: &str) {
    Command::new("git")
        .args(["add", "."])
        .current_dir(root)
        .assert()
        .success();
    Command::new("git")
        .args(["commit", "-m", msg, "-q"])
        .env("GIT_AUTHOR_DATE", date)
        .env("GIT_COMMITTER_DATE", date)
        .current_dir(root)
        .assert()
        .success();
}

fn populate_complex_repo(root: &Path) {
    fs::create_dir_all(root.join("docs")).unwrap();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::create_dir_all(root.join("tests")).unwrap();

    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(root.join("docs").join("guide.md"), "old guide\n").unwrap();
    fs::write(
        root.join("src").join("lib.rs"),
        "pub fn value() -> i32 {\n    1\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("tests").join("smoke.rs"),
        "#[test]\nfn smoke() {\n    assert_eq!(1, 1);\n}\n",
    )
    .unwrap();
    commit_all_at(root, "base", "2026-01-01T00:00:00Z");

    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"demo\"\nversion = \"0.2.0\"\n",
    )
    .unwrap();
    fs::write(root.join("docs").join("guide.md"), "new guide\n").unwrap();
    fs::write(
        root.join("src").join("lib.rs"),
        "pub fn value() -> i32 {\n    2\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("tests").join("smoke.rs"),
        "#[test]\nfn smoke() {\n    assert_eq!(2, 2);\n}\n",
    )
    .unwrap();
    commit_all_at(root, "update", "2026-01-01T00:00:01Z");

    fs::create_dir_all(root.join(".github").join("workflows")).unwrap();
    fs::write(
        root.join(".github").join("workflows").join("ci.yml"),
        "name: ci\non: [push]\n",
    )
    .unwrap();
    Command::new("git")
        .args(["add", ".github/workflows/ci.yml"])
        .current_dir(root)
        .assert()
        .success();

    fs::write(
        root.join("src").join("lib.rs"),
        "pub fn value() -> i32 {\n    3\n}\n",
    )
    .unwrap();
    fs::write(root.join("notes.md"), "remember this\n").unwrap();
    fs::write(root.join("blob.bin"), [0_u8, 159, 146, 150]).unwrap();
}

fn populate_simple_repo(root: &Path) {
    fs::create_dir_all(root.join("docs")).unwrap();
    fs::write(root.join("docs").join("guide.md"), "old\n").unwrap();
    commit_all_at(root, "base", "2026-01-01T00:00:00Z");

    fs::write(root.join("docs").join("guide.md"), "new\n").unwrap();
    commit_all_at(root, "update", "2026-01-01T00:00:01Z");
}

fn build_bundle(root: &Path, out_name: &str, extra: &[&str]) -> PathBuf {
    let out = root.join(out_name);
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("diffship");
    cmd.current_dir(root).arg("build");
    for arg in extra {
        cmd.arg(arg);
    }
    cmd.arg("--out").arg(&out);
    cmd.assert().success();
    out
}

fn read_tree_bytes(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(base: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        let mut entries = fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap())
            .collect::<Vec<_>>();
        entries.sort_by_key(|e| e.file_name());
        for ent in entries {
            let path = ent.path();
            if path.is_dir() {
                walk(base, &path, out);
            } else if path.is_file() {
                let rel = path
                    .strip_prefix(base)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                out.insert(rel, fs::read(path).unwrap());
            }
        }
    }

    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
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

fn normalize_handoff_for_golden(s: &str) -> String {
    let s = replace_hex40_runs(s);
    let mut lines = Vec::new();
    for line in s.lines() {
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

fn normalize_patch_for_golden(s: &str) -> String {
    let mut out = String::new();
    for line in replace_hex40_runs(s).lines() {
        out.push_str(line);
        out.push('\n');
    }
    out
}

#[test]
fn normalize_handoff_preserves_utf8_symbols() {
    let s = "- Docs: `1` files → parts: `part_01.patch`
";
    assert_eq!(normalize_handoff_for_golden(s), s);
}

#[test]
fn build_tree_is_deterministic_for_same_inputs() {
    let td1 = init_repo();
    let td2 = init_repo();
    populate_complex_repo(td1.path());
    populate_complex_repo(td2.path());

    let out1 = build_bundle(
        td1.path(),
        "bundle",
        &[
            "--include-staged",
            "--include-unstaged",
            "--include-untracked",
            "--yes",
        ],
    );
    let out2 = build_bundle(
        td2.path(),
        "bundle",
        &[
            "--include-staged",
            "--include-unstaged",
            "--include-untracked",
            "--yes",
        ],
    );

    assert_eq!(read_tree_bytes(&out1), read_tree_bytes(&out2));
}

#[test]
fn build_zip_is_deterministic_for_same_inputs() {
    let td1 = init_repo();
    let td2 = init_repo();
    populate_complex_repo(td1.path());
    populate_complex_repo(td2.path());

    let out1 = build_bundle(
        td1.path(),
        "bundle",
        &["--zip", "--include-untracked", "--yes"],
    );
    let out2 = build_bundle(
        td2.path(),
        "bundle",
        &["--zip", "--include-untracked", "--yes"],
    );

    assert_eq!(
        fs::read(out1.with_extension("zip")).unwrap(),
        fs::read(out2.with_extension("zip")).unwrap()
    );
}

#[test]
fn build_matches_normalized_golden_files() {
    let td = init_repo();
    populate_simple_repo(td.path());

    let out = build_bundle(td.path(), "bundle", &[]);
    let handoff = fs::read_to_string(out.join("HANDOFF.md")).unwrap();
    let patch = fs::read_to_string(out.join("parts").join("part_01.patch")).unwrap();

    let golden_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden")
        .join("m6_simple");

    assert_eq!(
        normalize_handoff_for_golden(&handoff),
        fs::read_to_string(golden_root.join("HANDOFF.normalized.md")).unwrap()
    );
    assert_eq!(
        normalize_patch_for_golden(&patch),
        fs::read_to_string(golden_root.join("part_01.normalized.patch")).unwrap()
    );
}

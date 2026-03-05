use assert_cmd::prelude::*;
use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

fn init_repo() -> TempDir {
    let td = tempfile::tempdir().expect("tempdir");
    let root = td.path();

    Command::new("git")
        .args(["init", "-q"])
        .current_dir(root)
        .assert()
        .success();

    Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(root)
        .assert()
        .success();
    Command::new("git")
        .args(["config", "user.name", "diffship-test"])
        .current_dir(root)
        .assert()
        .success();

    td
}

fn commit_all(root: &Path, msg: &str) {
    Command::new("git")
        .args(["add", "."])
        .current_dir(root)
        .assert()
        .success();
    Command::new("git")
        .args(["commit", "-m", msg, "-q"])
        .current_dir(root)
        .assert()
        .success();
}

fn git_stdout(root: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .unwrap();
    assert!(out.status.success(), "git {:?} failed", args);
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

#[test]
fn build_default_out_creates_bundle_dir_and_uses_last_range() {
    let td = init_repo();
    let root = td.path();

    fs::write(root.join("a.txt"), "one\n").unwrap();
    commit_all(root, "c1");

    fs::write(root.join("a.txt"), "two\n").unwrap();
    commit_all(root, "c2");

    let head = git_stdout(root, &["rev-parse", "HEAD"]);

    // Run build with default output directory name.
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("diffship");
    cmd.current_dir(root).arg("build");
    cmd.assert().success();

    // Find the generated diffship_<timestamp>/ directory.
    let mut bundles = vec![];
    for ent in fs::read_dir(root).unwrap() {
        let ent = ent.unwrap();
        if !ent.file_type().unwrap().is_dir() {
            continue;
        }
        let name = ent.file_name().to_string_lossy().to_string();
        if name.starts_with("diffship_") {
            bundles.push(ent.path());
        }
    }
    assert_eq!(bundles.len(), 1, "expected exactly one bundle dir");

    let bundle = &bundles[0];
    assert!(bundle.join("HANDOFF.md").exists());
    assert!(bundle.join("parts").join("part_01.patch").exists());

    let handoff = fs::read_to_string(bundle.join("HANDOFF.md")).unwrap();
    assert!(handoff.contains("## TL;DR"));
    assert!(handoff.contains("Segments included: committed=`yes`"));
    assert!(handoff.contains(&head));
    assert!(handoff.contains("## 3) Parts Index"));

    let part = fs::read_to_string(bundle.join("parts").join("part_01.patch")).unwrap();
    // Default range-mode is last, so we should see the last commit's change.
    assert!(part.contains("a.txt"));
    assert!(part.contains("+two"));
}

#[test]
fn build_range_mode_direct_accepts_from_to() {
    let td = init_repo();
    let root = td.path();

    fs::write(root.join("a.txt"), "one\n").unwrap();
    commit_all(root, "c1");

    fs::write(root.join("a.txt"), "two\n").unwrap();
    commit_all(root, "c2");

    let out = root.join("bundle_direct");

    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("diffship");
    cmd.current_dir(root)
        .args([
            "build",
            "--range-mode",
            "direct",
            "--from",
            "HEAD~1",
            "--to",
            "HEAD",
            "--out",
        ])
        .arg(&out);

    cmd.assert().success();

    let part = fs::read_to_string(out.join("parts").join("part_01.patch")).unwrap();
    assert!(part.contains("+two"));
}

#[test]
fn build_range_mode_merge_base_uses_merge_base_to_b() {
    let td = init_repo();
    let root = td.path();

    fs::write(root.join("base.txt"), "base\n").unwrap();
    commit_all(root, "base");

    let base_branch = git_stdout(root, &["rev-parse", "--abbrev-ref", "HEAD"]);

    Command::new("git")
        .args(["checkout", "-b", "feature", "-q"])
        .current_dir(root)
        .assert()
        .success();

    fs::write(root.join("feature.txt"), "feature\n").unwrap();
    commit_all(root, "feature");

    Command::new("git")
        .args(["checkout", "-q"])
        .arg(&base_branch)
        .current_dir(root)
        .assert()
        .success();

    fs::write(root.join("main.txt"), "main\n").unwrap();
    commit_all(root, "main");

    let out = root.join("bundle_mergeb");

    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("diffship");
    cmd.current_dir(root)
        .args(["build", "--range-mode", "merge-base", "--a"])
        .arg(&base_branch)
        .args(["--b", "feature", "--out"])
        .arg(&out);

    cmd.assert().success();

    let part = fs::read_to_string(out.join("parts").join("part_01.patch")).unwrap();
    assert!(part.contains("feature.txt"));
    assert!(part.contains("+feature"));
    assert!(!part.contains("main.txt"));
}

#[test]
fn build_with_out_is_deterministic_for_parts() {
    let td = init_repo();
    let root = td.path();

    fs::write(root.join("a.txt"), "one\n").unwrap();
    fs::write(root.join("b.txt"), "hello\n").unwrap();
    commit_all(root, "c1");

    fs::write(root.join("a.txt"), "two\n").unwrap();
    fs::write(root.join("b.txt"), "world\n").unwrap();
    commit_all(root, "c2");

    let out1 = root.join("bundle1");
    let out2 = root.join("bundle2");

    let mut cmd1 = assert_cmd::cargo::cargo_bin_cmd!("diffship");
    cmd1.current_dir(root).args(["build", "--out"]).arg(&out1);
    cmd1.assert().success();

    let mut cmd2 = assert_cmd::cargo::cargo_bin_cmd!("diffship");
    cmd2.current_dir(root).args(["build", "--out"]).arg(&out2);
    cmd2.assert().success();

    let p1 = fs::read(out1.join("parts").join("part_01.patch")).unwrap();
    let p2 = fs::read(out2.join("parts").join("part_01.patch")).unwrap();
    assert_eq!(p1, p2);

    let h1 = fs::read_to_string(out1.join("HANDOFF.md")).unwrap();
    assert!(h1.contains("File Table"));
}

#[test]
fn build_root_mode_works_for_single_commit_repo() {
    let td = init_repo();
    let root = td.path();

    fs::write(root.join("only.txt"), "hello\n").unwrap();
    commit_all(root, "root");

    let out = root.join("bundle_root");

    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("diffship");
    cmd.current_dir(root)
        .args(["build", "--range-mode", "root", "--out"])
        .arg(&out);

    cmd.assert().success();

    let part = fs::read_to_string(out.join("parts").join("part_01.patch")).unwrap();
    assert!(part.contains("only.txt"));
    assert!(part.contains("+hello"));
}

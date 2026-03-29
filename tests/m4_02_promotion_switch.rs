use assert_cmd::prelude::*;
use std::fs;
use std::process::Command;
use tempfile::TempDir;

fn init_repo_with_branches(branches: &[&str]) -> TempDir {
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

    fs::write(root.join("README.md"), "hello\n").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(root)
        .assert()
        .success();
    Command::new("git")
        .args(["commit", "-m", "init", "-q"])
        .current_dir(root)
        .assert()
        .success();

    for br in branches {
        Command::new("git")
            .args(["branch", br])
            .current_dir(root)
            .assert()
            .success();
    }
    if branches.contains(&"develop") {
        Command::new("git")
            .args(["checkout", "-q", "develop"])
            .current_dir(root)
            .assert()
            .success();
    }

    td
}

fn diffship_cmd(home: &std::path::Path) -> Command {
    let mut c = Command::new(assert_cmd::cargo::cargo_bin!("diffship"));
    c.env("HOME", home);
    c
}

fn head(root: &std::path::Path) -> String {
    let out = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(root)
        .output()
        .expect("rev-parse")
        .stdout;
    String::from_utf8_lossy(&out).trim().to_string()
}

fn make_patch_by_editing_readme(repo_root: &std::path::Path, new_line: &str) -> String {
    let readme = repo_root.join("README.md");
    let mut s = fs::read_to_string(&readme).unwrap();
    s.push_str(new_line);
    fs::write(&readme, s).unwrap();

    let out = Command::new("git")
        .args(["diff"])
        .current_dir(repo_root)
        .output()
        .expect("git diff")
        .stdout;
    let patch = String::from_utf8_lossy(&out).to_string();

    Command::new("git")
        .args(["checkout", "--", "README.md"])
        .current_dir(repo_root)
        .assert()
        .success();

    patch
}

fn make_bundle_dir_with_patch(
    repo_root: &std::path::Path,
    base_commit: &str,
    patch_text: &str,
    touched_files: &[&str],
    extra_manifest: &str,
) -> TempDir {
    let td = tempfile::tempdir().expect("bundle tempdir");
    let root = td.path();

    let bundle_root = root.join("patchship_test");
    fs::create_dir_all(bundle_root.join("changes")).unwrap();

    let manifest = format!(
        "protocol_version: \"1\"\ntask_id: \"TEST\"\nbase_commit: \"{}\"\napply_mode: git-apply\ntouched_files:\n{}\n{}",
        base_commit,
        touched_files
            .iter()
            .map(|p| format!("  - \"{}\"", p))
            .collect::<Vec<_>>()
            .join("\n"),
        extra_manifest
    );
    fs::write(bundle_root.join("manifest.yaml"), manifest).unwrap();
    fs::write(bundle_root.join("changes").join("0001.patch"), patch_text).unwrap();

    Command::new("git")
        .args([
            "apply",
            "--check",
            bundle_root.join("changes/0001.patch").to_str().unwrap(),
        ])
        .current_dir(repo_root)
        .assert()
        .success();

    td
}

fn extract_run_id(stdout: &[u8]) -> String {
    let s = String::from_utf8_lossy(stdout);
    for line in s.lines() {
        if let Some(rest) = line.strip_prefix("  run_id  : ") {
            return rest.trim().to_string();
        }
    }
    panic!("run_id not found in output: {s}");
}

fn git_status_porcelain(root: &std::path::Path) -> String {
    let out = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(root)
        .output()
        .expect("git status")
        .stdout;
    String::from_utf8_lossy(&out).to_string()
}

#[test]
fn m4_02_promotion_none_skips_cherry_pick() {
    let home_td = tempfile::tempdir().expect("home");
    let home = home_td.path();

    let td = init_repo_with_branches(&["develop"]);
    let root = td.path();
    let base = head(root);

    let patch = make_patch_by_editing_readme(root, "no-promote\n");
    let bundle_td = make_bundle_dir_with_patch(root, &base, &patch, &["README.md"], "");
    let bundle_root = bundle_td.path().join("patchship_test");

    let out = diffship_cmd(home)
        .args([
            "loop",
            bundle_root.to_str().unwrap(),
            "--promotion",
            "none",
            "--target-branch",
            "develop",
        ])
        .current_dir(root)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let _run_id = extract_run_id(&out);

    // Promotion skipped: repository HEAD should remain unchanged.
    assert_eq!(head(root), base);
}

#[test]
fn m4_02_promotion_working_tree_applies_without_commit() {
    let home_td = tempfile::tempdir().expect("home");
    let home = home_td.path();

    let td = init_repo_with_branches(&["develop"]);
    let root = td.path();
    let base = head(root);

    let patch = make_patch_by_editing_readme(root, "working-tree\n");
    let bundle_td = make_bundle_dir_with_patch(root, &base, &patch, &["README.md"], "");
    let bundle_root = bundle_td.path().join("patchship_test");

    let out = diffship_cmd(home)
        .args([
            "loop",
            bundle_root.to_str().unwrap(),
            "--promotion",
            "working-tree",
            "--target-branch",
            "develop",
        ])
        .current_dir(root)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let _run_id = extract_run_id(&out);

    // working-tree mode should not create a commit on the target branch.
    assert_eq!(head(root), base);

    // But the patch content should be present in the target working tree.
    let readme = fs::read_to_string(root.join("README.md")).unwrap();
    assert!(readme.contains("working-tree"));
    let status = git_status_porcelain(root);
    assert!(status.contains("README.md"));
}

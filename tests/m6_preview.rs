use assert_cmd::prelude::*;
use predicates::prelude::*;
use predicates::str::contains;
use serde_json::Value;
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

#[test]
fn preview_can_show_list_and_part_from_directory_bundle() {
    let td = init_repo();
    let root = td.path();

    fs::write(root.join("README.md"), "base\n").unwrap();
    commit_all(root, "base");
    fs::write(root.join("README.md"), "next\n").unwrap();
    commit_all(root, "next");

    let out = root.join("bundle_preview_dir");
    let mut build = assert_cmd::cargo::cargo_bin_cmd!("diffship");
    build
        .current_dir(root)
        .args(["build", "--out"])
        .arg(&out)
        .assert()
        .success();

    let mut list = assert_cmd::cargo::cargo_bin_cmd!("diffship");
    list.current_dir(root)
        .args(["preview"])
        .arg(&out)
        .arg("--list")
        .assert()
        .success()
        .stdout(contains("HANDOFF.md      : yes").and(contains("parts/part_01.patch")));

    let mut part = assert_cmd::cargo::cargo_bin_cmd!("diffship");
    part.current_dir(root)
        .args(["preview"])
        .arg(&out)
        .args(["--part", "part_01.patch"])
        .assert()
        .success()
        .stdout(contains("diffship segment: committed").and(contains("README.md")));
}

#[test]
fn preview_can_show_part_from_zip_bundle() {
    let td = init_repo();
    let root = td.path();

    fs::write(root.join("README.md"), "base\n").unwrap();
    commit_all(root, "base");
    fs::write(root.join("README.md"), "zip\n").unwrap();
    commit_all(root, "next");

    let out = root.join("bundle_preview_zip");
    let mut build = assert_cmd::cargo::cargo_bin_cmd!("diffship");
    build
        .current_dir(root)
        .args(["build", "--zip", "--out"])
        .arg(&out)
        .assert()
        .success();

    let bundle_zip = out.with_extension("zip");
    let mut part = assert_cmd::cargo::cargo_bin_cmd!("diffship");
    part.current_dir(root)
        .args(["preview"])
        .arg(&bundle_zip)
        .args(["--part", "parts/part_01.patch"])
        .assert()
        .success()
        .stdout(contains("diff --git").and(contains("README.md")));
}

#[test]
fn preview_json_outputs_summary_and_entry_text() {
    let td = init_repo();
    let root = td.path();

    fs::write(root.join("README.md"), "base\n").unwrap();
    commit_all(root, "base");
    fs::write(root.join("README.md"), "json\n").unwrap();
    commit_all(root, "next");

    let out = root.join("bundle_preview_json");
    let mut build = assert_cmd::cargo::cargo_bin_cmd!("diffship");
    build
        .current_dir(root)
        .args(["build", "--out"])
        .arg(&out)
        .assert()
        .success();

    let mut summary_cmd = assert_cmd::cargo::cargo_bin_cmd!("diffship");
    let summary = summary_cmd
        .current_dir(root)
        .args(["preview"])
        .arg(&out)
        .args(["--list", "--json"])
        .output()
        .unwrap();
    assert!(summary.status.success());
    let v: Value = serde_json::from_slice(&summary.stdout).expect("preview summary json");
    assert_eq!(v.get("mode").and_then(|x| x.as_str()), Some("list"));
    assert_eq!(v.get("handoff_md").and_then(|x| x.as_bool()), Some(true));
    assert_eq!(
        v.get("parts").and_then(|x| x.as_array()).map(|x| x.len()),
        Some(1)
    );

    let mut entry_cmd = assert_cmd::cargo::cargo_bin_cmd!("diffship");
    let entry = entry_cmd
        .current_dir(root)
        .args(["preview"])
        .arg(&out)
        .args(["--part", "part_01.patch", "--json"])
        .output()
        .unwrap();
    assert!(entry.status.success());
    let v: Value = serde_json::from_slice(&entry.stdout).expect("preview entry json");
    assert_eq!(
        v.get("entry").and_then(|x| x.as_str()),
        Some("parts/part_01.patch")
    );
    assert!(
        v.get("text")
            .and_then(|x| x.as_str())
            .is_some_and(|x| x.contains("README.md"))
    );
}

#[test]
fn preview_accepts_tilde_bundle_path() {
    let td = init_repo();
    let root = td.path();
    let home = root.join("fake-home");
    fs::create_dir_all(&home).unwrap();

    fs::write(root.join("README.md"), "base\n").unwrap();
    commit_all(root, "base");
    fs::write(root.join("README.md"), "tilde\n").unwrap();
    commit_all(root, "next");

    let out = home.join("bundle_preview_tilde");
    let mut build = assert_cmd::cargo::cargo_bin_cmd!("diffship");
    build
        .env("HOME", home.as_os_str())
        .current_dir(root)
        .args(["build", "--out"])
        .arg(&out)
        .assert()
        .success();

    let mut part = assert_cmd::cargo::cargo_bin_cmd!("diffship");
    part.env("HOME", home.as_os_str())
        .current_dir(root)
        .args([
            "preview",
            "~/bundle_preview_tilde",
            "--part",
            "part_01.patch",
        ])
        .assert()
        .success()
        .stdout(contains("README.md"));
}

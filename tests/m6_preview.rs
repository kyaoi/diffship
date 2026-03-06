use assert_cmd::prelude::*;
use predicates::prelude::*;
use predicates::str::contains;
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

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
fn compare_normalized_accepts_equivalent_bundles() {
    let td = init_repo();
    let root = td.path();

    fs::write(root.join("README.md"), "base\n").unwrap();
    commit_all(root, "base");
    fs::write(root.join("README.md"), "next\n").unwrap();
    commit_all(root, "next");

    let out_a = root.join("bundle_a");
    let out_b = root.join("bundle_b");

    let mut build_a = assert_cmd::cargo::cargo_bin_cmd!("diffship");
    build_a
        .current_dir(root)
        .args(["build", "--out"])
        .arg(&out_a)
        .assert()
        .success();

    let mut build_b = assert_cmd::cargo::cargo_bin_cmd!("diffship");
    build_b
        .current_dir(root)
        .args(["build", "--zip", "--out"])
        .arg(&out_b)
        .assert()
        .success();

    let mut cmp = assert_cmd::cargo::cargo_bin_cmd!("diffship");
    cmp.current_dir(root)
        .args(["compare"])
        .arg(&out_a)
        .arg(out_b.with_extension("zip"))
        .assert()
        .success()
        .stdout(contains("diffship compare: equivalent"));

    let mut strict = assert_cmd::cargo::cargo_bin_cmd!("diffship");
    strict
        .current_dir(root)
        .args(["compare", "--strict"])
        .arg(&out_a)
        .arg(out_b.with_extension("zip"))
        .assert()
        .failure()
        .stderr(contains("diffship compare: different"));
}

#[test]
fn compare_reports_real_content_difference() {
    let td = init_repo();
    let root = td.path();

    fs::write(root.join("README.md"), "base\n").unwrap();
    commit_all(root, "base");
    fs::write(root.join("README.md"), "v1\n").unwrap();
    commit_all(root, "v1");

    let out_a = root.join("bundle_a");
    let mut build_a = assert_cmd::cargo::cargo_bin_cmd!("diffship");
    build_a
        .current_dir(root)
        .args(["build", "--out"])
        .arg(&out_a)
        .assert()
        .success();

    fs::write(root.join("README.md"), "v2\n").unwrap();
    commit_all(root, "v2");

    let out_c = root.join("bundle_c");
    let mut build_c = assert_cmd::cargo::cargo_bin_cmd!("diffship");
    build_c
        .current_dir(root)
        .args(["build", "--out"])
        .arg(&out_c)
        .assert()
        .success();

    let mut cmp = assert_cmd::cargo::cargo_bin_cmd!("diffship");
    cmp.current_dir(root)
        .args(["compare"])
        .arg(&out_a)
        .arg(&out_c)
        .assert()
        .failure()
        .stderr(
            contains("diffship compare: different")
                .and(contains("bundle comparison failed"))
                .and(contains("content differs")),
        );
}

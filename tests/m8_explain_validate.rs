use assert_cmd::prelude::*;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
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

    td
}

fn head(root: &Path) -> String {
    let out = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(root)
        .output()
        .expect("rev-parse")
        .stdout;
    String::from_utf8_lossy(&out).trim().to_string()
}

fn write_project_config(root: &Path, body: &str) {
    let path = root.join(".diffship");
    fs::create_dir_all(&path).unwrap();
    fs::write(path.join("config.toml"), body).unwrap();
}

fn make_patch_by_editing_readme(repo_root: &Path, new_line: &str) -> String {
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
    repo_root: &Path,
    base_commit: &str,
    patch_text: &str,
    touched_files: &[&str],
) -> TempDir {
    let td = tempfile::tempdir().expect("bundle tempdir");
    let root = td.path();

    let bundle_root = root.join("patchship_test");
    fs::create_dir_all(bundle_root.join("changes")).unwrap();

    let manifest = format!(
        "protocol_version: \"1\"\ntask_id: \"TEST\"\nbase_commit: \"{}\"\napply_mode: git-apply\ntouched_files:\n{}\n",
        base_commit,
        touched_files
            .iter()
            .map(|p| format!("  - \"{}\"", p))
            .collect::<Vec<_>>()
            .join("\n")
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

fn only_generated_bundle(root: &Path) -> PathBuf {
    let mut bundles = fs::read_dir(root)
        .unwrap()
        .filter_map(|entry| {
            let entry = entry.ok()?;
            if !entry.file_type().ok()?.is_dir() {
                return None;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            name.starts_with("diffship_").then(|| entry.path())
        })
        .collect::<Vec<_>>();
    bundles.sort();
    assert_eq!(bundles.len(), 1, "expected exactly one generated bundle");
    bundles.remove(0)
}

fn create_failed_verify_run(root: &Path) -> String {
    let base = head(root);
    write_project_config(
        root,
        r#"
[workflow]
default_profile = "prototype-speed"

[workflow.strategy]
mode = "prefer"
default_profile = "no-test-fast"

[workflow.strategy.error_overrides]
verify_test_failed = "regression-test-first"

[verify]
default_profile = "custom"

[verify.profiles.custom]
cmd1 = "cargo test"
"#,
    );

    let patch = make_patch_by_editing_readme(root, "world\n");
    let bundle_td = make_bundle_dir_with_patch(root, &base, &patch, &["README.md"]);
    let bundle_root = bundle_td.path().join("patchship_test");

    let apply_out = Command::new(assert_cmd::cargo::cargo_bin!("diffship"))
        .args(["apply", bundle_root.to_str().unwrap()])
        .current_dir(root)
        .output()
        .expect("apply");
    assert!(apply_out.status.success(), "apply failed");
    let run_id = extract_run_id(&apply_out.stdout);

    Command::new(assert_cmd::cargo::cargo_bin!("diffship"))
        .args(["verify", "--run-id", &run_id])
        .current_dir(root)
        .assert()
        .failure()
        .code(9);

    run_id
}

#[test]
fn validate_patch_json_reports_bundle_contract() {
    let td = init_repo();
    let root = td.path();
    let base = head(root);
    let patch = make_patch_by_editing_readme(root, "world\n");
    let bundle_td = make_bundle_dir_with_patch(root, &base, &patch, &["README.md"]);
    let bundle_root = bundle_td.path().join("patchship_test");

    let out = Command::new(assert_cmd::cargo::cargo_bin!("diffship"))
        .args(["validate-patch", bundle_root.to_str().unwrap(), "--json"])
        .current_dir(root)
        .output()
        .expect("validate-patch");
    assert!(out.status.success(), "validate-patch failed");

    let json: Value = serde_json::from_slice(&out.stdout).expect("json");
    assert_eq!(
        json.get("bundle_kind").and_then(|v| v.as_str()),
        Some("directory")
    );
    assert_eq!(
        json.get("manifest")
            .and_then(|v| v.get("base_commit"))
            .and_then(|v| v.as_str()),
        Some(base.as_str())
    );
    assert_eq!(
        json.get("manifest")
            .and_then(|v| v.get("apply_mode"))
            .and_then(|v| v.as_str()),
        Some("git-apply")
    );
    assert_eq!(
        json.get("patch_files")
            .and_then(|v| v.as_array())
            .map(|v| v.len()),
        Some(1)
    );
}

#[test]
fn explain_latest_run_reports_state_and_next_command() {
    let td = init_repo();
    let root = td.path();
    let run_id = create_failed_verify_run(root);

    let out = Command::new(assert_cmd::cargo::cargo_bin!("diffship"))
        .args(["explain", "--latest"])
        .current_dir(root)
        .output()
        .expect("explain");
    assert!(out.status.success(), "explain failed");

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("target: run"));
    assert!(stdout.contains(&format!("run_id: {run_id}")));
    assert!(stdout.contains("state: recoverable"));
    assert!(stdout.contains(&format!("next: diffship strategy --run-id {run_id}")));
    assert!(stdout.contains("strategy: regression-test-first"));
}

#[test]
fn explain_bundle_json_reports_reading_order() {
    let td = init_repo();
    let root = td.path();

    fs::write(root.join("README.md"), "hello\nworld\n").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(root)
        .assert()
        .success();
    Command::new("git")
        .args(["commit", "-m", "update", "-q"])
        .current_dir(root)
        .assert()
        .success();

    Command::new(assert_cmd::cargo::cargo_bin!("diffship"))
        .arg("build")
        .current_dir(root)
        .assert()
        .success();
    let bundle = only_generated_bundle(root);
    let current_head = head(root);

    let out = Command::new(assert_cmd::cargo::cargo_bin!("diffship"))
        .args(["explain", "--bundle", bundle.to_str().unwrap(), "--json"])
        .current_dir(root)
        .output()
        .expect("bundle explain");
    assert!(out.status.success(), "bundle explain failed");

    let json: Value = serde_json::from_slice(&out.stdout).expect("json");
    assert_eq!(json.get("target").and_then(|v| v.as_str()), Some("bundle"));
    assert_eq!(
        json.get("current_head").and_then(|v| v.as_str()),
        Some(current_head.as_str())
    );
    assert_eq!(json.get("part_count").and_then(|v| v.as_u64()), Some(1));
    assert!(
        json.get("next_read")
            .and_then(|v| v.as_array())
            .map(|items| !items.is_empty())
            .unwrap_or(false)
    );
}

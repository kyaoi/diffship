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
        .args(["config", "user.email", "test@example.com"])
        .current_dir(root)
        .assert()
        .success();
    Command::new("git")
        .args(["config", "user.name", "diffship-test"])
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

fn read_verify_profile(repo_root: &Path, run_id: &str) -> String {
    let p = repo_root
        .join(".diffship")
        .join("runs")
        .join(run_id)
        .join("verify.json");
    let bytes = fs::read(p).expect("verify.json");
    let v: Value = serde_json::from_slice(&bytes).expect("verify json");
    v.get("profile")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

fn read_sandbox_path(repo_root: &Path, run_id: &str) -> PathBuf {
    let p = repo_root
        .join(".diffship")
        .join("runs")
        .join(run_id)
        .join("sandbox.json");
    let bytes = fs::read(p).expect("sandbox.json");
    let v: Value = serde_json::from_slice(&bytes).expect("sandbox json");
    PathBuf::from(v.get("path").and_then(Value::as_str).unwrap_or(""))
}

#[test]
fn verify_uses_configured_profile_commands_by_default() {
    let td = init_repo();
    let root = td.path();
    let base = head(root);

    fs::create_dir_all(root.join(".diffship")).unwrap();
    fs::write(
        root.join(".diffship").join("config.toml"),
        r#"
[verify]
default_profile = "custom"

[verify.profiles.custom]
cmd1 = "printf ok > .verify_marker"
cmd2 = "git diff --check"
"#,
    )
    .unwrap();

    let patch = make_patch_by_editing_readme(root, "world\n");
    let bundle_td = make_bundle_dir_with_patch(root, &base, &patch, &["README.md"]);
    let bundle_root = bundle_td.path().join("patchship_test");

    let out = Command::new(assert_cmd::cargo::cargo_bin!("diffship"))
        .args(["apply", bundle_root.to_str().unwrap()])
        .current_dir(root)
        .output()
        .expect("apply");
    assert!(out.status.success(), "apply failed");
    let run_id = extract_run_id(&out.stdout);

    Command::new(assert_cmd::cargo::cargo_bin!("diffship"))
        .args(["verify", "--run-id", &run_id])
        .current_dir(root)
        .assert()
        .success();

    assert_eq!(read_verify_profile(root, &run_id), "custom");
    let sandbox = read_sandbox_path(root, &run_id);
    assert!(sandbox.join(".verify_marker").exists());
}

#[test]
fn verify_cli_profile_override_bypasses_custom_default_profile() {
    let td = init_repo();
    let root = td.path();
    let base = head(root);

    fs::create_dir_all(root.join(".diffship")).unwrap();
    fs::write(
        root.join(".diffship").join("config.toml"),
        r#"
[verify]
default_profile = "custom"

[verify.profiles.custom]
cmd1 = "printf ok > .verify_marker"
"#,
    )
    .unwrap();

    let patch = make_patch_by_editing_readme(root, "world\n");
    let bundle_td = make_bundle_dir_with_patch(root, &base, &patch, &["README.md"]);
    let bundle_root = bundle_td.path().join("patchship_test");

    let out = Command::new(assert_cmd::cargo::cargo_bin!("diffship"))
        .args(["apply", bundle_root.to_str().unwrap()])
        .current_dir(root)
        .output()
        .expect("apply");
    assert!(out.status.success(), "apply failed");
    let run_id = extract_run_id(&out.stdout);

    Command::new(assert_cmd::cargo::cargo_bin!("diffship"))
        .args(["verify", "--run-id", &run_id, "--profile", "fast"])
        .current_dir(root)
        .assert()
        .success();

    assert_eq!(read_verify_profile(root, &run_id), "fast");
    let sandbox = read_sandbox_path(root, &run_id);
    assert!(!sandbox.join(".verify_marker").exists());
}

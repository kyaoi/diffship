use assert_cmd::prelude::*;
use serde_json::Value;
use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;
use zip::ZipArchive;

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

fn find_default_pack_fix_zip(run_dir: &Path) -> std::path::PathBuf {
    let mut matches = fs::read_dir(run_dir)
        .unwrap()
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .map(|name| name.starts_with("pack-fix_") && name.ends_with(".zip"))
                .unwrap_or(false)
        })
        .collect::<Vec<_>>();
    matches.sort();
    assert_eq!(
        matches.len(),
        1,
        "expected exactly one default pack-fix zip"
    );
    matches.pop().unwrap()
}

fn zip_entry_text(path: &Path, name: &str) -> String {
    let file = fs::File::open(path).expect("zip file");
    let mut zip = ZipArchive::new(file).expect("zip archive");
    let mut entry = zip.by_name(name).expect("zip entry");
    let mut s = String::new();
    use std::io::Read;
    entry.read_to_string(&mut s).expect("entry text");
    s
}

fn create_failed_verify_run(root: &Path) -> (String, std::path::PathBuf) {
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

    let run_dir = root.join(".diffship").join("runs").join(&run_id);
    (run_id, run_dir)
}

#[test]
fn strategy_json_matches_pack_fix_strategy_export() {
    let td = init_repo();
    let root = td.path();
    let (run_id, run_dir) = create_failed_verify_run(root);
    let zip_path = find_default_pack_fix_zip(&run_dir);

    let strategy_out = Command::new(assert_cmd::cargo::cargo_bin!("diffship"))
        .args(["strategy", "--run-id", &run_id, "--json"])
        .current_dir(root)
        .output()
        .expect("strategy");
    assert!(strategy_out.status.success(), "strategy failed");

    let from_cli: Value = serde_json::from_slice(&strategy_out.stdout).expect("strategy json");
    let from_zip: Value =
        serde_json::from_str(&zip_entry_text(&zip_path, "strategy.resolved.json")).unwrap();
    assert_eq!(from_cli, from_zip);
}

#[test]
fn strategy_latest_human_output_summarizes_resolution() {
    let td = init_repo();
    let root = td.path();
    let (run_id, _run_dir) = create_failed_verify_run(root);

    let out = Command::new(assert_cmd::cargo::cargo_bin!("diffship"))
        .args(["strategy", "--latest"])
        .current_dir(root)
        .output()
        .expect("strategy");
    assert!(out.status.success(), "strategy failed");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("diffship strategy"));
    assert!(stdout.contains(&format!("run_id: {run_id}")));
    assert!(stdout.contains("failure_category: verify_test_failed"));
    assert!(stdout.contains("strategy_mode: prefer"));
    assert!(stdout.contains("selected_profile: regression-test-first"));
    assert!(stdout.contains("default_profile: no-test-fast"));
    assert!(stdout.contains("alternatives: no-test-fast"));
    assert!(stdout.contains("tests_expected: true"));
    assert!(stdout.contains("preferred_verify_profile: standard"));
}

#[test]
fn strategy_refuses_run_without_failure_category() {
    let td = init_repo();
    let root = td.path();
    let base = head(root);

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
        .args(["strategy", "--run-id", &run_id])
        .current_dir(root)
        .assert()
        .failure()
        .code(1)
        .stderr(predicates::str::contains(
            "has no failed phase with a normalized failure_category",
        ));
}

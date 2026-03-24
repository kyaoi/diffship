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

fn zip_entries(path: &Path) -> Vec<String> {
    let file = fs::File::open(path).expect("zip file");
    let mut zip = ZipArchive::new(file).expect("zip archive");
    let mut names = vec![];
    for i in 0..zip.len() {
        names.push(zip.by_index(i).expect("entry").name().to_string());
    }
    names.sort();
    names
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

#[test]
fn pack_fix_command_creates_expected_zip_contents() {
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
        .args(["pack-fix", "--run-id", &run_id])
        .current_dir(root)
        .assert()
        .success();

    let run_dir = root.join(".diffship").join("runs").join(&run_id);
    let zip_path = find_default_pack_fix_zip(&run_dir);
    assert!(zip_path.exists());
    let file_name = zip_path.file_name().unwrap().to_str().unwrap();
    let run_stem = run_id.strip_prefix("run_").unwrap_or(&run_id);
    assert_eq!(file_name, format!("pack-fix_{run_stem}.zip"));
    let entries = zip_entries(&zip_path);
    assert!(entries.contains(&"PROMPT.md".to_string()));
    assert!(entries.contains(&"SAFETY.md".to_string()));
    assert!(entries.contains(&"run/run.json".to_string()));
    assert!(entries.contains(&"run/apply.json".to_string()));
    assert!(entries.contains(&"bundle/manifest.yaml".to_string()));
    assert!(entries.contains(&"bundle/changes/0001.patch".to_string()));
    assert!(entries.contains(&"sandbox/git_status.txt".to_string()));
    assert!(entries.contains(&"sandbox/git_diff.patch".to_string()));
    assert!(!entries.contains(&"strategy.resolved.json".to_string()));
}

#[test]
fn verify_failure_auto_creates_pack_fix_zip() {
    let td = init_repo();
    let root = td.path();
    let base = head(root);

    let patch = make_patch_by_editing_readme(root, "bad \n");
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
    let zip_path = find_default_pack_fix_zip(&run_dir);
    assert!(zip_path.exists());
    let file_name = zip_path.file_name().unwrap().to_str().unwrap();
    let run_stem = run_id.strip_prefix("run_").unwrap_or(&run_id);
    assert_eq!(file_name, format!("pack-fix_{run_stem}.zip"));
    let entries = zip_entries(&zip_path);
    assert!(entries.contains(&"PROMPT.md".to_string()));
    assert!(entries.contains(&"run/verify.json".to_string()));
    assert!(entries.contains(&"strategy.resolved.json".to_string()));
    let verify: Value =
        serde_json::from_str(&fs::read_to_string(run_dir.join("verify.json")).unwrap()).unwrap();
    assert_eq!(
        verify.get("failure_category").and_then(|v| v.as_str()),
        Some("verify_failed")
    );
}

#[test]
fn pack_fix_accepts_tilde_out_path() {
    let td = init_repo();
    let root = td.path();
    let home = root.join("fake-home");
    fs::create_dir_all(&home).unwrap();
    let base = head(root);

    let patch = make_patch_by_editing_readme(root, "bad \n");
    let bundle_td = make_bundle_dir_with_patch(root, &base, &patch, &["README.md"]);
    let bundle_root = bundle_td.path().join("patchship_test");

    let out = assert_cmd::cargo::cargo_bin_cmd!("diffship")
        .args(["apply", bundle_root.to_str().unwrap()])
        .current_dir(root)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let run_id = extract_run_id(&out);

    assert_cmd::cargo::cargo_bin_cmd!("diffship")
        .args(["verify", "--run-id", &run_id])
        .current_dir(root)
        .assert()
        .failure()
        .code(9);

    assert_cmd::cargo::cargo_bin_cmd!("diffship")
        .env("HOME", home.as_os_str())
        .args(["pack-fix", "--run-id", &run_id, "--out", "~/fixes/out.zip"])
        .current_dir(root)
        .assert()
        .success();

    assert!(home.join("fixes").join("out.zip").exists());
}

#[test]
fn pack_fix_includes_post_apply_artifacts_when_present() {
    let td = init_repo();
    let root = td.path();
    let base = head(root);

    write_project_config(
        root,
        r#"
[verify]
default_profile = "custom"

[verify.profiles.custom]
cmd1 = "exit 9"

[ops.post_apply]
cmd1 = "printf post-apply >> README.md"
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
    let zip_path = find_default_pack_fix_zip(&run_dir);
    let entries = zip_entries(&zip_path);
    assert!(entries.contains(&"run/post_apply.json".to_string()));
    assert!(entries.contains(&"run/post-apply/01_cmd1.stdout".to_string()));

    let prompt = zip_entry_text(&zip_path, "PROMPT.md");
    assert!(prompt.contains("run/post_apply.json"));
    assert!(prompt.contains("run/post-apply/"));
    assert!(prompt.contains("post_apply_changed_paths: `1`"));
    assert!(prompt.contains("post_apply_change_categories: `docs_touch`"));
    assert!(prompt.contains("changed paths: `README.md`"));
    assert!(prompt.contains("change categories: `docs_touch`"));

    let post_apply = zip_entry_text(&zip_path, "run/post_apply.json");
    assert!(post_apply.contains("\"changed_paths\": ["));
    assert!(post_apply.contains("\"README.md\""));
    assert!(post_apply.contains("\"change_categories\": ["));
    assert!(post_apply.contains("\"docs_touch\""));
}

#[test]
fn pack_fix_prompt_includes_resolved_strategy_guidance() {
    let td = init_repo();
    let root = td.path();
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
    let zip_path = find_default_pack_fix_zip(&run_dir);
    let prompt = zip_entry_text(&zip_path, "PROMPT.md");
    assert!(prompt.contains("## Suggested strategy"));
    assert!(prompt.contains("Read `strategy.resolved.json` first"));
    assert!(prompt.contains("- failure_category: `verify_test_failed`"));
    assert!(prompt.contains("- strategy_mode: `prefer`"));
    assert!(prompt.contains("- selected_profile: `regression-test-first`"));
    assert!(prompt.contains("- default_profile: `no-test-fast`"));
    assert!(prompt.contains("- alternatives: `no-test-fast`"));

    let strategy: Value =
        serde_json::from_str(&zip_entry_text(&zip_path, "strategy.resolved.json")).unwrap();
    assert_eq!(
        strategy
            .get("failure_category")
            .and_then(|value| value.as_str()),
        Some("verify_test_failed")
    );
    assert_eq!(
        strategy
            .get("selected_profile")
            .and_then(|value| value.as_str()),
        Some("regression-test-first")
    );
}

#[test]
fn pack_fix_strategy_export_reports_no_test_fast_metadata() {
    let td = init_repo();
    let root = td.path();
    let base = head(root);

    write_project_config(
        root,
        r#"
[workflow]
default_profile = "prototype-speed"

[workflow.strategy]
mode = "prefer"
default_profile = "no-test-fast"

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
    let zip_path = find_default_pack_fix_zip(&run_dir);
    let prompt = zip_entry_text(&zip_path, "PROMPT.md");
    assert!(prompt.contains("- selected_profile: `no-test-fast`"));
    assert!(prompt.contains("- tests_expected: `false`"));
    assert!(prompt.contains("- preferred_verify_profile: `fast`"));

    let strategy: Value =
        serde_json::from_str(&zip_entry_text(&zip_path, "strategy.resolved.json")).unwrap();
    assert_eq!(
        strategy
            .get("selected_profile")
            .and_then(|value| value.as_str()),
        Some("no-test-fast")
    );
    assert_eq!(
        strategy
            .get("tests_expected")
            .and_then(|value| value.as_bool()),
        Some(false)
    );
    assert_eq!(
        strategy
            .get("preferred_verify_profile")
            .and_then(|value| value.as_str()),
        Some("fast")
    );
}

#[test]
fn pack_fix_strategy_export_is_deterministic_for_same_inputs() {
    let td = init_repo();
    let root = td.path();
    let base = head(root);

    write_project_config(
        root,
        r#"
[workflow]
default_profile = "prototype-speed"

[workflow.strategy]
mode = "prefer"
default_profile = "no-test-fast"

[verify]
default_profile = "custom"

[verify.profiles.custom]
cmd1 = "cargo test"
"#,
    );

    let mut exports = Vec::new();
    for _ in 0..2 {
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
        let zip_path = find_default_pack_fix_zip(&run_dir);
        exports.push(zip_entry_text(&zip_path, "strategy.resolved.json"));
    }

    assert_eq!(exports[0], exports[1]);
}

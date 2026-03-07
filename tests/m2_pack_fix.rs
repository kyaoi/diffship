use assert_cmd::prelude::*;
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

    let zip_path = root
        .join(".diffship")
        .join("runs")
        .join(&run_id)
        .join("pack-fix.zip");
    assert!(zip_path.exists());
    let entries = zip_entries(&zip_path);
    assert!(entries.contains(&"PROMPT.md".to_string()));
    assert!(entries.contains(&"SAFETY.md".to_string()));
    assert!(entries.contains(&"run/run.json".to_string()));
    assert!(entries.contains(&"run/apply.json".to_string()));
    assert!(entries.contains(&"bundle/manifest.yaml".to_string()));
    assert!(entries.contains(&"bundle/changes/0001.patch".to_string()));
    assert!(entries.contains(&"sandbox/git_status.txt".to_string()));
    assert!(entries.contains(&"sandbox/git_diff.patch".to_string()));
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

    let zip_path = root
        .join(".diffship")
        .join("runs")
        .join(&run_id)
        .join("pack-fix.zip");
    assert!(zip_path.exists());
    let entries = zip_entries(&zip_path);
    assert!(entries.contains(&"PROMPT.md".to_string()));
    assert!(entries.contains(&"run/verify.json".to_string()));
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

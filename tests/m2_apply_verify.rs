use assert_cmd::prelude::*;
use std::fs;
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

    // Minimal identity for commits in CI environments.
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

fn diffship_cmd() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin!("diffship"))
}

fn write_project_config(root: &std::path::Path, body: &str) {
    let path = root.join(".diffship");
    fs::create_dir_all(&path).unwrap();
    fs::write(path.join("config.toml"), body).unwrap();
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

fn make_bundle_dir_with_patch(
    repo_root: &std::path::Path,
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

    // Sanity: patch should apply against repo_root (pre-check).
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

    // revert to base for apply test
    Command::new("git")
        .args(["checkout", "--", "README.md"])
        .current_dir(repo_root)
        .assert()
        .success();

    patch
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

#[test]
fn m2_apply_and_verify_happy_path_generic_repo() {
    let td = init_repo();
    let root = td.path();
    let base = head(root);

    let patch = make_patch_by_editing_readme(root, "world\n");
    let bundle_td = make_bundle_dir_with_patch(root, &base, &patch, &["README.md"]);
    let bundle_root = bundle_td.path().join("patchship_test");

    let out = diffship_cmd()
        .args(["apply", bundle_root.to_str().unwrap()])
        .current_dir(root)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let run_id = extract_run_id(&out);

    let run_dir = root.join(".diffship").join("runs").join(&run_id);
    assert!(run_dir.join("run.json").exists());
    assert!(run_dir.join("apply.json").exists());
    assert!(run_dir.join("bundle").join("manifest.yaml").exists());
    assert!(run_dir.join("sandbox.json").exists());

    // verify should run on that run id, using the generic fallback (git diff --check).
    diffship_cmd()
        .args(["verify", "--run-id", &run_id])
        .current_dir(root)
        .assert()
        .success();
    assert!(run_dir.join("verify.json").exists());
}

#[test]
fn m2_apply_rejects_forbidden_paths_in_manifest() {
    let td = init_repo();
    let root = td.path();
    let base = head(root);

    let patch = make_patch_by_editing_readme(root, "world\n");
    let bundle_td = make_bundle_dir_with_patch(root, &base, &patch, &["../pwned.txt"]);
    let bundle_root = bundle_td.path().join("patchship_test");

    diffship_cmd()
        .args(["apply", bundle_root.to_str().unwrap()])
        .current_dir(root)
        .assert()
        .failure()
        .code(7);
}

#[test]
fn m2_apply_refuses_base_commit_mismatch() {
    let td = init_repo();
    let root = td.path();
    let old_base = head(root);

    // Advance HEAD so session head differs from manifest base.
    fs::write(root.join("OTHER.txt"), "x\n").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(root)
        .assert()
        .success();
    Command::new("git")
        .args(["commit", "-m", "advance", "-q"])
        .current_dir(root)
        .assert()
        .success();

    let patch = make_patch_by_editing_readme(root, "world\n");
    let bundle_td = make_bundle_dir_with_patch(root, &old_base, &patch, &["README.md"]);
    let bundle_root = bundle_td.path().join("patchship_test");

    diffship_cmd()
        .args(["apply", bundle_root.to_str().unwrap()])
        .current_dir(root)
        .assert()
        .failure()
        .code(6);
}

#[test]
fn m2_verify_fails_on_whitespace_errors() {
    let td = init_repo();
    let root = td.path();
    let base = head(root);

    // Introduce trailing whitespace; git diff --check should fail.
    let patch = make_patch_by_editing_readme(root, "bad \n");
    let bundle_td = make_bundle_dir_with_patch(root, &base, &patch, &["README.md"]);
    let bundle_root = bundle_td.path().join("patchship_test");

    let out = diffship_cmd()
        .args(["apply", bundle_root.to_str().unwrap()])
        .current_dir(root)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let run_id = extract_run_id(&out);

    diffship_cmd()
        .args(["verify", "--run-id", &run_id])
        .current_dir(root)
        .assert()
        .failure()
        .code(9);
}

#[test]
fn m2_apply_runs_configured_post_apply_commands_in_sandbox() {
    let td = init_repo();
    let root = td.path();
    let base = head(root);

    write_project_config(
        root,
        r#"
[ops.post_apply]
cmd1 = "printf hook >> README.md"
"#,
    );

    let patch = make_patch_by_editing_readme(root, "world\n");
    let bundle_td = make_bundle_dir_with_patch(root, &base, &patch, &["README.md"]);
    let bundle_root = bundle_td.path().join("patchship_test");

    let out = diffship_cmd()
        .args(["apply", bundle_root.to_str().unwrap()])
        .current_dir(root)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let run_id = extract_run_id(&out);
    let run_dir = root.join(".diffship").join("runs").join(&run_id);
    let sandbox_readme = root
        .join(".diffship")
        .join("worktrees")
        .join("sandboxes")
        .join(&run_id)
        .join("README.md");
    let readme = fs::read_to_string(sandbox_readme).unwrap();

    assert!(readme.contains("world\n"));
    assert!(readme.contains("hook"));
    assert!(run_dir.join("post_apply.json").exists());
    assert!(run_dir.join("post-apply").join("01_cmd1.stdout").exists());
}

#[test]
fn m2_apply_fails_when_post_apply_command_fails() {
    let td = init_repo();
    let root = td.path();
    let base = head(root);

    write_project_config(
        root,
        r#"
[ops.post_apply]
cmd1 = "exit 7"
"#,
    );

    let patch = make_patch_by_editing_readme(root, "world\n");
    let bundle_td = make_bundle_dir_with_patch(root, &base, &patch, &["README.md"]);
    let bundle_root = bundle_td.path().join("patchship_test");

    let out = diffship_cmd()
        .args(["apply", bundle_root.to_str().unwrap()])
        .current_dir(root)
        .assert()
        .failure()
        .code(8)
        .get_output()
        .stderr
        .clone();
    let stderr = String::from_utf8_lossy(&out);
    assert!(stderr.contains("post-apply commands failed"));

    let runs_dir = root.join(".diffship").join("runs");
    let latest = fs::read_dir(&runs_dir)
        .unwrap()
        .filter_map(|ent| ent.ok().map(|e| e.path()))
        .filter(|path| path.is_dir())
        .max()
        .unwrap();
    assert!(latest.join("post_apply.json").exists());
}

#[test]
fn m2_apply_accepts_tilde_bundle_path() {
    let td = init_repo();
    let root = td.path();
    let home_td = tempfile::tempdir().unwrap();
    let home = home_td.path();
    let base = head(root);

    let patch = make_patch_by_editing_readme(root, "world\n");
    let bundle_td = make_bundle_dir_with_patch(root, &base, &patch, &["README.md"]);
    let bundle_root = bundle_td.path().join("patchship_test");
    let home_bundle = home.join("bundle");
    fs::create_dir_all(&home_bundle).unwrap();
    fs::copy(
        bundle_root.join("manifest.yaml"),
        home_bundle.join("manifest.yaml"),
    )
    .unwrap();
    fs::create_dir_all(home_bundle.join("changes")).unwrap();
    fs::copy(
        bundle_root.join("changes").join("0001.patch"),
        home_bundle.join("changes").join("0001.patch"),
    )
    .unwrap();

    diffship_cmd()
        .env("HOME", home)
        .args(["apply", "~/bundle"])
        .current_dir(root)
        .assert()
        .success();
}

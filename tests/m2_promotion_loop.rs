use assert_cmd::prelude::*;
use std::fs;
use std::process::Command;
use tempfile::TempDir;
use zip::write::FileOptions;
use zip::{CompressionMethod, ZipWriter};

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

    // Create a develop branch for promotion defaults.
    Command::new("git")
        .args(["checkout", "-q", "-b", "develop"])
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

fn make_bundle_dir_with_patch(
    repo_root: &std::path::Path,
    base_commit: &str,
    patch_text: &str,
    touched_files: &[&str],
    commit_message: Option<&str>,
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

    if let Some(msg) = commit_message {
        fs::write(bundle_root.join("commit_message.txt"), msg).unwrap();
    }

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

fn write_patch_bundle_zip(bundle_root: &std::path::Path, zip_path: &std::path::Path) {
    let file = fs::File::create(zip_path).unwrap();
    let mut zip = ZipWriter::new(file);
    let options = FileOptions::default().compression_method(CompressionMethod::Stored);

    for rel in ["manifest.yaml", "changes/0001.patch"] {
        zip.start_file(format!("patchship_test/{rel}"), options)
            .unwrap();
        use std::io::Write as _;
        zip.write_all(&fs::read(bundle_root.join(rel)).unwrap())
            .unwrap();
    }

    if bundle_root.join("commit_message.txt").exists() {
        zip.start_file("patchship_test/commit_message.txt", options)
            .unwrap();
        use std::io::Write as _;
        zip.write_all(&fs::read(bundle_root.join("commit_message.txt")).unwrap())
            .unwrap();
    }

    zip.finish().unwrap();
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

fn last_commit_message(root: &std::path::Path) -> String {
    let out = Command::new("git")
        .args(["log", "-1", "--pretty=%B"])
        .current_dir(root)
        .output()
        .expect("git log")
        .stdout;
    String::from_utf8_lossy(&out).to_string()
}

fn install_pre_commit_hook(root: &std::path::Path, body: &str) {
    let hook = root.join(".git").join("hooks").join("pre-commit");
    fs::write(&hook, body).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&hook).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&hook, perms).unwrap();
    }
}

#[test]
fn m2_promote_commit_creates_commit_on_branch() {
    let td = init_repo();
    let root = td.path();
    let base = head(root);

    let patch = make_patch_by_editing_readme(root, "world\n");
    let commit_msg = "TEST: hello from bundle\n\nbody\n";
    let bundle_td =
        make_bundle_dir_with_patch(root, &base, &patch, &["README.md"], Some(commit_msg));
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
        .args(["verify", "--run-id", &run_id, "--profile", "fast"])
        .current_dir(root)
        .assert()
        .success();

    diffship_cmd()
        .args(["promote", "--run-id", &run_id, "--target-branch", "develop"])
        .current_dir(root)
        .assert()
        .success();

    let msg = last_commit_message(root);
    assert!(msg.contains("TEST: hello from bundle"));
    let commands = fs::read_to_string(
        root.join(".diffship")
            .join("runs")
            .join(&run_id)
            .join("commands.json"),
    )
    .unwrap();
    assert!(commands.contains("\"phase\": \"promote\""));

    // Session head should advance to the promoted HEAD.
    let session_state =
        fs::read_to_string(root.join(".diffship").join("sessions").join("default.json")).unwrap();
    assert!(session_state.contains(&head(root)));

    // Sandbox should be removed by default.
    assert!(
        !root
            .join(".diffship")
            .join("worktrees")
            .join("sandboxes")
            .join(&run_id)
            .exists()
    );
}

#[test]
fn m2_promote_logs_pre_commit_output_under_run_commands() {
    let td = init_repo();
    let root = td.path();
    let base = head(root);
    install_pre_commit_hook(root, "#!/bin/sh\necho hook-line >&2\n");

    let patch = make_patch_by_editing_readme(root, "world\n");
    let bundle_td = make_bundle_dir_with_patch(root, &base, &patch, &["README.md"], None);
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
        .args(["verify", "--run-id", &run_id, "--profile", "fast"])
        .current_dir(root)
        .assert()
        .success();

    diffship_cmd()
        .args(["promote", "--run-id", &run_id, "--target-branch", "develop"])
        .current_dir(root)
        .assert()
        .success();

    let stderr = fs::read_to_string(
        root.join(".diffship")
            .join("runs")
            .join(&run_id)
            .join("promote")
            .join("03_git_commit.stderr"),
    )
    .unwrap();
    assert!(stderr.contains("hook-line"));
}

#[test]
fn m2_loop_happy_path_promotes_commit() {
    let td = init_repo();
    let root = td.path();
    let base = head(root);

    let patch = make_patch_by_editing_readme(root, "loop\n");
    let bundle_td = make_bundle_dir_with_patch(root, &base, &patch, &["README.md"], None);
    let bundle_root = bundle_td.path().join("patchship_test");

    let out = diffship_cmd()
        .args([
            "loop",
            bundle_root.to_str().unwrap(),
            "--profile",
            "fast",
            "--target-branch",
            "develop",
        ])
        .current_dir(root)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let run_id = extract_run_id(&out);
    // Promotion should have created a commit on develop.
    assert_ne!(head(root), base);
    // And sandbox should be removed.
    assert!(
        !root
            .join(".diffship")
            .join("worktrees")
            .join("sandboxes")
            .join(&run_id)
            .exists()
    );
}

#[test]
fn m2_loop_can_delete_input_zip_after_copying_bundle() {
    let td = init_repo();
    let root = td.path();
    let base = head(root);

    let patch = make_patch_by_editing_readme(root, "world\n");
    let bundle_td = make_bundle_dir_with_patch(root, &base, &patch, &["README.md"], None);
    let bundle_root = bundle_td.path().join("patchship_test");
    let bundle_zip = bundle_td.path().join("patchship_test.zip");
    write_patch_bundle_zip(&bundle_root, &bundle_zip);

    diffship_cmd()
        .args(["loop", "--delete-input-zip"])
        .arg(&bundle_zip)
        .current_dir(root)
        .assert()
        .success();

    assert!(!bundle_zip.exists());
}

#[test]
fn m2_loop_writes_pack_fix_when_post_apply_fails() {
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
    let bundle_td = make_bundle_dir_with_patch(root, &base, &patch, &["README.md"], None);
    let bundle_root = bundle_td.path().join("patchship_test");

    let out = diffship_cmd()
        .arg("loop")
        .arg(&bundle_root)
        .current_dir(root)
        .assert()
        .failure()
        .code(8)
        .get_output()
        .stderr
        .clone();
    let stderr = String::from_utf8_lossy(&out);

    assert!(stderr.contains("post-apply commands failed"));
    assert!(stderr.contains("pack-fix saved to"));

    let runs_dir = root.join(".diffship").join("runs");
    let latest = fs::read_dir(&runs_dir)
        .unwrap()
        .filter_map(|ent| ent.ok().map(|e| e.path()))
        .filter(|path| path.is_dir())
        .max()
        .unwrap();
    assert!(
        latest
            .read_dir()
            .unwrap()
            .filter_map(|ent| ent.ok().map(|e| e.path()))
            .any(|path| path.extension().and_then(|ext| ext.to_str()) == Some("zip"))
    );
}

#[test]
fn m2_loop_accepts_base_commit_override_when_manifest_is_stale() {
    let td = init_repo();
    let root = td.path();
    let old_base = head(root);

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

    let current_head = head(root);
    let patch = make_patch_by_editing_readme(root, "loop\n");
    let bundle_td = make_bundle_dir_with_patch(root, &old_base, &patch, &["README.md"], None);
    let bundle_root = bundle_td.path().join("patchship_test");

    diffship_cmd()
        .args([
            "loop",
            bundle_root.to_str().unwrap(),
            "--base-commit",
            &current_head,
            "--profile",
            "fast",
            "--target-branch",
            "develop",
        ])
        .current_dir(root)
        .assert()
        .success();

    assert_ne!(head(root), current_head);
}

#[test]
fn m2_loop_runs_post_apply_commands_before_promote() {
    let td = init_repo();
    let root = td.path();
    let base = head(root);

    write_project_config(
        root,
        r#"
[ops.post_apply]
cmd1 = "printf post-apply\\n >> README.md"
"#,
    );

    let patch = make_patch_by_editing_readme(root, "loop\n");
    let bundle_td = make_bundle_dir_with_patch(root, &base, &patch, &["README.md"], None);
    let bundle_root = bundle_td.path().join("patchship_test");

    diffship_cmd()
        .args([
            "loop",
            bundle_root.to_str().unwrap(),
            "--profile",
            "fast",
            "--target-branch",
            "develop",
        ])
        .current_dir(root)
        .assert()
        .success();

    let readme = fs::read_to_string(root.join("README.md")).unwrap();
    assert!(readme.contains("loop\n"));
    assert!(readme.contains("post-apply\n"));
}

#[test]
fn m3_promotion_blocks_on_secrets_without_ack() {
    let td = init_repo();
    let root = td.path();
    let base = head(root);

    // Insert an AWS-like access key id in the patch.
    let patch = make_patch_by_editing_readme(root, "AKIA0123456789ABCDEF\n");
    let bundle_td = make_bundle_dir_with_patch(root, &base, &patch, &["README.md"], None);
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
        .args(["verify", "--run-id", &run_id, "--profile", "fast"])
        .current_dir(root)
        .assert()
        .success();

    // Promotion should be blocked unless acked.
    diffship_cmd()
        .args(["promote", "--run-id", &run_id, "--target-branch", "develop"])
        .current_dir(root)
        .assert()
        .failure()
        .code(11);

    // With ack, it should proceed.
    diffship_cmd()
        .args([
            "promote",
            "--run-id",
            &run_id,
            "--target-branch",
            "develop",
            "--ack-secrets",
        ])
        .current_dir(root)
        .assert()
        .success();
}

#[test]
fn m2_promote_refuses_when_verify_not_ok() {
    let td = init_repo();
    let root = td.path();
    let base = head(root);

    // Introduce trailing whitespace; git diff --check should fail.
    let patch = make_patch_by_editing_readme(root, "bad \n");
    let bundle_td = make_bundle_dir_with_patch(root, &base, &patch, &["README.md"], None);
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
        .args(["verify", "--run-id", &run_id, "--profile", "fast"])
        .current_dir(root)
        .assert()
        .failure()
        .code(9);

    // Promotion should fail with a promotion-specific error (verify not ok).
    diffship_cmd()
        .args(["promote", "--run-id", &run_id, "--target-branch", "develop"])
        .current_dir(root)
        .assert()
        .failure()
        .code(13);
}

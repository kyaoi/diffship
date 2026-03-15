use assert_cmd::prelude::*;
use predicates::prelude::*;
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

struct SetupPaths {
    run_id: String,
    session_worktree: String,
    sandbox_path: String,
}

fn create_session_and_sandbox(root: &std::path::Path) -> SetupPaths {
    let out = diffship_cmd()
        .args(["__test_m1_setup"])
        .current_dir(root)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    SetupPaths {
        run_id: v
            .get("run_id")
            .and_then(|x| x.as_str())
            .unwrap()
            .to_string(),
        session_worktree: v
            .get("session")
            .and_then(|x| x.get("worktree_path"))
            .and_then(|x| x.as_str())
            .unwrap()
            .to_string(),
        sandbox_path: v
            .get("sandbox")
            .and_then(|x| x.get("path"))
            .and_then(|x| x.as_str())
            .unwrap()
            .to_string(),
    }
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

#[test]
fn cleanup_dry_run_reports_but_keeps_promoted_sandbox() {
    let td = init_repo();
    let root = td.path();
    let setup = create_session_and_sandbox(root);
    let run_dir = root.join(".diffship").join("runs").join(&setup.run_id);

    fs::write(
        run_dir.join("promotion.json"),
        format!(
            "{{\"run_id\":\"{}\",\"promoted_head\":\"{}\",\"ok\":true}}",
            setup.run_id,
            head(root)
        ),
    )
    .unwrap();

    diffship_cmd()
        .args(["cleanup", "--dry-run"])
        .current_dir(root)
        .assert()
        .success()
        .stdout(predicate::str::contains("promoted_sandbox"))
        .stdout(predicate::str::contains(&setup.run_id));

    assert!(root.join(&setup.sandbox_path).exists());
    assert!(run_dir.join("sandbox.json").exists());
}

#[test]
fn cleanup_removes_promoted_sandbox_and_its_metadata() {
    let td = init_repo();
    let root = td.path();
    let setup = create_session_and_sandbox(root);
    let run_dir = root.join(".diffship").join("runs").join(&setup.run_id);

    fs::write(
        run_dir.join("promotion.json"),
        format!(
            "{{\"run_id\":\"{}\",\"promoted_head\":\"{}\",\"ok\":true}}",
            setup.run_id,
            head(root)
        ),
    )
    .unwrap();

    diffship_cmd()
        .args(["cleanup"])
        .current_dir(root)
        .assert()
        .success()
        .stdout(predicate::str::contains("action=removed"));

    assert!(!root.join(&setup.sandbox_path).exists());
    assert!(!run_dir.join("sandbox.json").exists());
}

#[test]
fn cleanup_removes_orphan_session_worktree() {
    let td = init_repo();
    let root = td.path();
    let setup = create_session_and_sandbox(root);

    diffship_cmd()
        .args(["__test_m1_cleanup_sandbox", "--run-id", &setup.run_id])
        .current_dir(root)
        .assert()
        .success();

    fs::remove_file(root.join(".diffship").join("sessions").join("default.json")).unwrap();
    Command::new("git")
        .args(["update-ref", "-d", "refs/diffship/sessions/default"])
        .current_dir(root)
        .assert()
        .success();

    diffship_cmd()
        .args(["cleanup"])
        .current_dir(root)
        .assert()
        .success()
        .stdout(predicate::str::contains("orphan_session_worktree"));

    assert!(!root.join(&setup.session_worktree).exists());
}

#[test]
fn cleanup_removes_orphan_sandbox_when_run_metadata_is_gone() {
    let td = init_repo();
    let root = td.path();
    let setup = create_session_and_sandbox(root);
    let run_dir = root.join(".diffship").join("runs").join(&setup.run_id);

    fs::remove_dir_all(&run_dir).unwrap();

    diffship_cmd()
        .args(["cleanup"])
        .current_dir(root)
        .assert()
        .success()
        .stdout(predicate::str::contains("orphan_sandbox"));

    assert!(!root.join(&setup.sandbox_path).exists());
}

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

fn head(root: &std::path::Path) -> String {
    let out = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(root)
        .output()
        .expect("rev-parse")
        .stdout;
    String::from_utf8_lossy(&out).trim().to_string()
}

fn create_session_and_sandbox(root: &std::path::Path) -> String {
    let out = diffship_cmd()
        .args(["__test_m1_setup"])
        .current_dir(root)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    v.get("run_id")
        .and_then(|x| x.as_str())
        .unwrap()
        .to_string()
}

#[test]
fn session_repair_realigns_stale_session_head_to_repo_head() {
    let td = init_repo();
    let root = td.path();
    let run_id = create_session_and_sandbox(root);

    diffship_cmd()
        .args(["__test_m1_cleanup_sandbox", "--run-id", &run_id])
        .current_dir(root)
        .assert()
        .success();

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

    let repo_head = head(root);
    diffship_cmd()
        .args(["session", "repair", "--session", "default"])
        .current_dir(root)
        .assert()
        .success();

    let session_head = Command::new("git")
        .args(["rev-parse", "refs/diffship/sessions/default"])
        .current_dir(root)
        .output()
        .expect("rev-parse session")
        .stdout;
    let session_head = String::from_utf8_lossy(&session_head).trim().to_string();
    assert_eq!(session_head, repo_head);

    let state =
        fs::read_to_string(root.join(".diffship").join("sessions").join("default.json")).unwrap();
    assert!(state.contains(&repo_head));
}

#[test]
fn session_repair_refuses_when_active_sandbox_exists() {
    let td = init_repo();
    let root = td.path();
    let _run_id = create_session_and_sandbox(root);

    diffship_cmd()
        .args(["session", "repair", "--session", "default"])
        .current_dir(root)
        .assert()
        .failure()
        .code(1)
        .stderr(predicates::str::contains("active sandboxes"));
}

#[test]
fn doctor_reports_and_fixes_stale_session_head() {
    let td = init_repo();
    let root = td.path();
    let run_id = create_session_and_sandbox(root);

    diffship_cmd()
        .args(["__test_m1_cleanup_sandbox", "--run-id", &run_id])
        .current_dir(root)
        .assert()
        .success();

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

    diffship_cmd()
        .args(["doctor", "--session", "default"])
        .current_dir(root)
        .assert()
        .failure()
        .code(1)
        .stdout(predicates::str::contains("session_head_mismatch"));

    diffship_cmd()
        .args(["doctor", "--session", "default", "--fix"])
        .current_dir(root)
        .assert()
        .success()
        .stdout(predicates::str::contains("ok"));

    let session_head = Command::new("git")
        .args(["rev-parse", "refs/diffship/sessions/default"])
        .current_dir(root)
        .output()
        .expect("rev-parse session")
        .stdout;
    assert_eq!(String::from_utf8_lossy(&session_head).trim(), head(root));
}

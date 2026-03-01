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

#[test]
fn m1_session_and_sandbox_create_advance_cleanup() {
    let td = init_repo();
    let root = td.path();

    // Create session + run + sandbox.
    let out = diffship_cmd()
        .args(["__test_m1_setup"])
        .current_dir(root)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let v: serde_json::Value = serde_json::from_slice(&out).expect("valid json");
    let run_id = v
        .get("run_id")
        .and_then(|x| x.as_str())
        .expect("run_id")
        .to_string();
    let session_wt = v
        .get("session")
        .and_then(|x| x.get("worktree_path"))
        .and_then(|x| x.as_str())
        .expect("session.worktree_path")
        .to_string();
    let sandbox_path = v
        .get("sandbox")
        .and_then(|x| x.get("path"))
        .and_then(|x| x.as_str())
        .expect("sandbox.path")
        .to_string();

    assert!(root.join(".diffship").join("runs").join(&run_id).exists());
    assert!(
        root.join(&session_wt).exists(),
        "session worktree should exist"
    );
    assert!(root.join(&sandbox_path).exists(), "sandbox should exist");

    // Make a commit inside the sandbox (detached HEAD is fine).
    fs::write(root.join(&sandbox_path).join("M1.txt"), "m1\n").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(root.join(&sandbox_path))
        .assert()
        .success();
    Command::new("git")
        .args(["commit", "-m", "m1", "-q"])
        .current_dir(root.join(&sandbox_path))
        .assert()
        .success();

    let sandbox_head = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(root.join(&sandbox_path))
        .output()
        .expect("rev-parse")
        .stdout;
    let sandbox_head = String::from_utf8_lossy(&sandbox_head).trim().to_string();
    assert!(!sandbox_head.is_empty());

    // Advance session ref to the sandbox head.
    diffship_cmd()
        .args(["__test_m1_advance_session", "--run-id", &run_id])
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
    assert_eq!(session_head, sandbox_head);

    // status --json should surface session and sandbox.
    let out = diffship_cmd()
        .args(["status", "--json"])
        .current_dir(root)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).expect("valid json");
    let sessions = v.get("sessions").and_then(|x| x.as_array()).unwrap();
    assert!(
        sessions
            .iter()
            .any(|s| s.get("name") == Some(&serde_json::Value::String("default".to_string())))
    );
    let sandboxes = v.get("sandboxes").and_then(|x| x.as_array()).unwrap();
    assert!(
        sandboxes
            .iter()
            .any(|sb| sb.get("run_id") == Some(&serde_json::Value::String(run_id.clone())))
    );

    // Cleanup the sandbox worktree.
    diffship_cmd()
        .args(["__test_m1_cleanup_sandbox", "--run-id", &run_id])
        .current_dir(root)
        .assert()
        .success();
    assert!(
        !root.join(&sandbox_path).exists(),
        "sandbox should be removed"
    );
}

use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::fs;
use std::process::{Command, Stdio};
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
fn m0_init_status_runs_happy_path() {
    let td = init_repo();
    let root = td.path();

    // init
    diffship_cmd()
        .args(["init"])
        .current_dir(root)
        .assert()
        .success()
        .stdout(predicate::str::contains("diffship init: ok"));

    // Generated files
    assert!(root.join(".diffship").join("PROJECT_KIT.md").exists());
    assert!(root.join(".diffship").join("AI_GUIDE.md").exists());
    assert!(root.join(".diffship").join("config.toml").exists());
    let ai = fs::read_to_string(root.join(".diffship").join("AI_GUIDE.md")).unwrap();
    assert!(ai.contains("# DiffshipOS AI Guide"));
    assert!(ai.contains("What diffship is"));
    assert!(ai.contains("What the AI is expected to produce"));
    assert!(ai.contains("Meaning of files the user may provide"));
    assert!(ai.contains("Additional deliverables beyond file edits"));
    let cfg = fs::read_to_string(root.join(".diffship").join("config.toml")).unwrap();
    assert!(cfg.contains("Copy `[handoff.profiles.*]` stanzas"));
    assert!(cfg.contains("It does not export the full profile catalog."));
    assert!(cfg.contains("output_dir = \"./artifacts/handoffs\""));

    // status --json
    let out = diffship_cmd()
        .args(["status", "--json"])
        .current_dir(root)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let v: serde_json::Value = serde_json::from_slice(&out).expect("valid json");
    assert!(v.get("git_root").is_some());
    assert!(v.get("lock").is_some());
    assert!(v.get("recent_runs").is_some());

    // runs --json
    let out = diffship_cmd()
        .args(["runs", "--json"])
        .current_dir(root)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let v: serde_json::Value = serde_json::from_slice(&out).expect("valid json");
    let runs = v.get("runs").and_then(|x| x.as_array()).unwrap();
    assert!(!runs.is_empty(), "init should create a run record");
}

#[test]
fn m0_lock_busy_returns_exit_10() {
    let td = init_repo();
    let root = td.path();

    // Spawn a process that holds the lock.
    let bin = assert_cmd::cargo::cargo_bin!("diffship");
    let mut child = Command::new(bin)
        .args(["__test_hold_lock", "--ms", "800"])
        .current_dir(root)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn lock holder");

    // Give it a moment to acquire the lock.
    std::thread::sleep(std::time::Duration::from_millis(150));

    // init should refuse with exit code 10
    diffship_cmd()
        .args(["init"])
        .current_dir(root)
        .assert()
        .failure()
        .code(10);

    let _ = child.wait();
}

#[test]
fn m0_init_can_use_custom_template_dir() {
    let td = init_repo();
    let root = td.path();
    let templates = tempfile::tempdir().unwrap();
    fs::write(
        templates.path().join("PROJECT_KIT_TEMPLATE.md"),
        "Custom project kit body\n",
    )
    .unwrap();
    fs::write(
        templates.path().join("AI_PROJECT_TEMPLATE.md"),
        "Custom AI guide body\n",
    )
    .unwrap();

    diffship_cmd()
        .args(["init", "--template-dir"])
        .arg(templates.path())
        .current_dir(root)
        .assert()
        .success();

    let kit = fs::read_to_string(root.join(".diffship").join("PROJECT_KIT.md")).unwrap();
    let ai = fs::read_to_string(root.join(".diffship").join("AI_GUIDE.md")).unwrap();
    assert!(kit.contains("Custom project kit body"));
    assert!(ai.contains("Custom AI guide body"));
}

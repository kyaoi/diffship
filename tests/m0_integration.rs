use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::fs;
use std::io::Read;
use std::process::{Command, Stdio};
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

fn current_branch(root: &std::path::Path) -> String {
    let out = Command::new("git")
        .args(["symbolic-ref", "--quiet", "--short", "HEAD"])
        .current_dir(root)
        .output()
        .expect("symbolic-ref")
        .stdout;
    String::from_utf8_lossy(&out).trim().to_string()
}

fn current_head(root: &std::path::Path) -> String {
    let out = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(root)
        .output()
        .expect("rev-parse")
        .stdout;
    String::from_utf8_lossy(&out).trim().to_string()
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
    assert!(root.join(".diffship").join("PROJECT_RULES.md").exists());
    assert!(root.join(".diffship").join("AI_GUIDE.md").exists());
    assert!(root.join(".diffship").join("WORKFLOW_PROFILE.md").exists());
    assert!(root.join(".diffship").join("forbid.toml").exists());
    assert!(root.join(".diffship").join(".gitignore").exists());
    assert!(
        root.join(".diffship")
            .join("ai_generated_config.toml")
            .exists()
    );
    assert!(root.join(".diffship").join("config.toml").exists());
    let kit = fs::read_to_string(root.join(".diffship").join("PROJECT_KIT.md")).unwrap();
    assert!(kit.contains("# DiffshipOS Project Kit"));
    assert!(kit.contains("## Generated repo snapshot"));
    assert!(kit.contains("## Suggested next steps"));
    assert!(kit.contains("Starter commands"));
    assert!(kit.contains("Attachment-ready summary for external AI tools"));
    assert!(kit.contains("Core workflow: what diffship is"));
    assert!(kit.contains("Customize this section: repository identity"));
    assert!(kit.contains("Core workflow: patch bundle contract the AI must follow"));
    assert!(kit.contains("Customize this section: local commands and gates"));
    assert!(kit.contains("Suggested read-first files"));
    assert!(kit.contains("Generated metadata"));
    let ai = fs::read_to_string(root.join(".diffship").join("AI_GUIDE.md")).unwrap();
    assert!(ai.contains("# DiffshipOS AI Guide"));
    assert!(ai.contains("## Generated repo snapshot"));
    assert!(ai.contains("Starter commands"));
    assert!(ai.contains("Attachment-ready project rules"));
    assert!(ai.contains("Core contract: what diffship is"));
    assert!(ai.contains("Customize this section: repository identity"));
    assert!(ai.contains("Core contract: what the AI is expected to produce"));
    assert!(ai.contains("Core contract: meaning of files the user may provide"));
    assert!(ai.contains("Core contract: additional deliverables beyond file edits"));
    assert!(ai.contains("Generated metadata"));
    let rules = fs::read_to_string(root.join(".diffship").join("PROJECT_RULES.md")).unwrap();
    assert!(rules.contains("# Diffship Project Rules"));
    assert!(rules.contains("Paste this into an external AI workspace"));
    assert!(rules.contains("Use `diffship loop` only with a valid `OPS_PATCH_BUNDLE`."));
    let workflow = fs::read_to_string(root.join(".diffship").join("WORKFLOW_PROFILE.md")).unwrap();
    assert!(workflow.contains("# Diffship Workflow Profile"));
    assert!(workflow.contains("- Profile: `balanced`"));
    assert!(workflow.contains("## Preferred verify cadence"));
    assert!(workflow.contains("## Docs and traceability expectations"));
    let forbid = fs::read_to_string(root.join(".diffship").join("forbid.toml")).unwrap();
    assert!(forbid.contains("[ops.forbid]"));
    assert!(forbid.contains("pnpm-lock.yaml"));
    let gitignore = fs::read_to_string(root.join(".diffship").join(".gitignore")).unwrap();
    assert_eq!(
        gitignore,
        "artifacts/handoffs/\nartifacts/rules/\nruns/\nworktrees/\nsessions/\nlock\n"
    );
    let ai_cfg =
        fs::read_to_string(root.join(".diffship").join("ai_generated_config.toml")).unwrap();
    assert!(ai_cfg.contains("# diffship AI-generated local configuration"));
    assert!(ai_cfg.contains("[ops.editable_diffship]"));
    assert!(ai_cfg.contains("path6 = \".diffship/ai_generated_config.toml\""));
    assert!(ai_cfg.contains("# path7 = \".diffship/config.toml\""));
    let cfg = fs::read_to_string(root.join(".diffship").join("config.toml")).unwrap();
    let branch = current_branch(root);
    let preferred_target = branch.clone();
    assert!(cfg.contains("# Repository snapshot:"));
    assert!(cfg.contains("# - repo:"));
    assert!(cfg.contains(&format!("# - current branch: {}", branch)));
    assert!(cfg.contains(&format!(
        "# - preferred promote target: {}",
        preferred_target
    )));
    assert!(cfg.contains("# Bootstrap workflow profile selected during init: balanced"));
    assert!(cfg.contains("# See `.diffship/WORKFLOW_PROFILE.md`"));
    assert!(cfg.contains("Use this file in two layers"));
    assert!(cfg.contains("Put AI-editable defaults and `.diffship/*` edit allowlist entries"));
    assert!(cfg.contains("Customize this section: choose default verify behavior"));
    assert!(cfg.contains("Customize this section: choose default handoff packing behavior"));
    assert!(cfg.contains(
        "Customize this section: local-only commands to run automatically after a successful apply"
    ));
    assert!(cfg.contains("Keep `post_apply` narrow: repository-local normalization only"));
    assert!(cfg.contains("# Rust-oriented preset:"));
    assert!(cfg.contains("# Node/TS-oriented preset:"));
    assert!(cfg.contains("# Docs/spec-oriented preset:"));
    assert!(cfg.contains("prefer `.diffship/forbid.toml` for dedicated forbid patterns"));
    assert!(cfg.contains("Copy `[handoff.profiles.*]` stanzas"));
    assert!(cfg.contains("It does not export the full profile catalog."));
    assert!(cfg.contains("output_dir = \"./.diffship/artifacts/handoffs\""));
    assert!(cfg.contains(&format!("target_branch = \"{}\"", preferred_target)));

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
    assert!(v.get("repo_head").is_some());
    assert!(v.get("lock").is_some());
    assert!(v.get("recent_runs").is_some());

    diffship_cmd()
        .args(["status", "--heads-only"])
        .current_dir(root)
        .assert()
        .success()
        .stdout(predicate::str::contains("repo_head"));

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
    let head = current_head(root);
    assert!(!runs.is_empty(), "init should create a run record");
    let run_id = runs[0].get("run_id").and_then(|x| x.as_str()).unwrap();
    assert!(run_id.starts_with("run_20"));
    assert!(run_id.contains(&format!("_{}", &head[..7])));
    assert!(
        runs[0]
            .get("run_dir")
            .and_then(|x| x.as_str())
            .is_some_and(|x| x.contains("/.diffship/runs/"))
    );

    diffship_cmd()
        .args(["runs", "--heads-only"])
        .current_dir(root)
        .assert()
        .success()
        .stdout(predicate::str::contains("base="));

    diffship_cmd()
        .args(["runs"])
        .current_dir(root)
        .assert()
        .success()
        .stdout(predicate::str::contains("run_dir="));
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

#[test]
fn m0_init_can_export_rules_zip() {
    let td = init_repo();
    let root = td.path();

    let output = diffship_cmd()
        .args(["init", "--zip"])
        .current_dir(root)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(output).unwrap();
    assert!(stdout.contains("zip      :"));

    let rules_dir = root.join(".diffship").join("artifacts").join("rules");
    let entries: Vec<_> = fs::read_dir(&rules_dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].extension().and_then(|x| x.to_str()), Some("zip"));

    let file = fs::File::open(&entries[0]).unwrap();
    let mut zip = ZipArchive::new(file).unwrap();
    let mut names = vec![];
    for i in 0..zip.len() {
        names.push(zip.by_index(i).unwrap().name().to_string());
    }
    names.sort();
    assert_eq!(
        names,
        vec![
            "AI_GUIDE.md".to_string(),
            "PROJECT_KIT.md".to_string(),
            "PROJECT_RULES.md".to_string(),
            "metadata.json".to_string()
        ]
    );

    let mut metadata = String::new();
    zip.by_name("metadata.json")
        .unwrap()
        .read_to_string(&mut metadata)
        .unwrap();
    let value: serde_json::Value = serde_json::from_str(&metadata).unwrap();
    assert!(value.get("generated_at").is_some());
    assert!(value.get("run_id").is_some());
    assert_eq!(value.get("language").and_then(|x| x.as_str()), Some("en"));
    assert!(value.get("branch").is_some());
    assert!(value.get("forbid_patterns").is_some());
}

#[test]
fn m0_init_can_generate_japanese_project_rules() {
    let td = init_repo();
    let root = td.path();

    diffship_cmd()
        .args(["init", "--lang", "ja"])
        .current_dir(root)
        .assert()
        .success();

    let rules = fs::read_to_string(root.join(".diffship").join("PROJECT_RULES.md")).unwrap();
    assert!(rules.contains("# Diffship プロジェクトルール"));
    assert!(
        rules.contains("外部 AI の project rules / custom instructions に貼るための短縮版です。")
    );
    assert!(rules.contains("`base_commit` が無い、または不確実なら"));
}

#[test]
fn m0_init_can_generate_selected_workflow_profile_guidance() {
    let td = init_repo();
    let root = td.path();

    diffship_cmd()
        .args(["init", "--workflow-profile", "cautious-tdd"])
        .current_dir(root)
        .assert()
        .success();

    let workflow = fs::read_to_string(root.join(".diffship").join("WORKFLOW_PROFILE.md")).unwrap();
    assert!(workflow.contains("- Profile: `cautious-tdd`"));
    assert!(workflow.contains("regression-resistant changes with a test-first bias"));
    assert!(workflow.contains("Start from a failing or missing focused test whenever practical."));

    let cfg = fs::read_to_string(root.join(".diffship").join("config.toml")).unwrap();
    assert!(cfg.contains("# Bootstrap workflow profile selected during init: cautious-tdd"));
}

#[test]
fn m0_init_forbid_stub_enables_detected_lockfiles() {
    let td = init_repo();
    let root = td.path();

    fs::write(root.join("package.json"), "{\n  \"name\": \"demo\"\n}\n").unwrap();
    fs::write(root.join("pnpm-lock.yaml"), "lockfileVersion: '9.0'\n").unwrap();

    diffship_cmd()
        .args(["init", "--force"])
        .current_dir(root)
        .assert()
        .success();

    let forbid = fs::read_to_string(root.join(".diffship").join("forbid.toml")).unwrap();
    assert!(forbid.contains("path1 = \"pnpm-lock.yaml\""));
    assert!(!forbid.contains("# path1 = \"pnpm-lock.yaml\""));
}

#[test]
fn m0_init_refresh_forbid_rewrites_only_forbid_file() {
    let td = init_repo();
    let root = td.path();

    diffship_cmd()
        .args(["init"])
        .current_dir(root)
        .assert()
        .success();

    fs::write(root.join(".diffship").join("PROJECT_RULES.md"), "KEEP\n").unwrap();
    fs::write(root.join("package.json"), "{\n  \"name\": \"demo\"\n}\n").unwrap();
    fs::write(root.join("pnpm-lock.yaml"), "lockfileVersion: '9.0'\n").unwrap();

    diffship_cmd()
        .args(["init", "--refresh-forbid"])
        .current_dir(root)
        .assert()
        .success()
        .stdout(predicate::str::contains("wrote"))
        .stdout(predicate::str::contains(".diffship/forbid.toml"))
        .stdout(predicate::str::contains("skipped"))
        .stdout(predicate::str::contains(".diffship/PROJECT_RULES.md"));

    let rules = fs::read_to_string(root.join(".diffship").join("PROJECT_RULES.md")).unwrap();
    assert_eq!(rules, "KEEP\n");

    let forbid = fs::read_to_string(root.join(".diffship").join("forbid.toml")).unwrap();
    assert!(forbid.contains("path1 = \"pnpm-lock.yaml\""));
    assert!(!forbid.contains("# path1 = \"pnpm-lock.yaml\""));
}

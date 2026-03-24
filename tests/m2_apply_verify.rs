use assert_cmd::prelude::*;
use serde_json::Value;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
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

fn write_project_ai_generated_config(root: &std::path::Path, body: &str) {
    let path = root.join(".diffship");
    fs::create_dir_all(&path).unwrap();
    fs::write(path.join("ai_generated_config.toml"), body).unwrap();
}

fn write_project_forbid(root: &std::path::Path, body: &str) {
    let path = root.join(".diffship");
    fs::create_dir_all(&path).unwrap();
    fs::write(path.join("forbid.toml"), body).unwrap();
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
    make_bundle_dir_with_patch_impl(repo_root, base_commit, patch_text, touched_files, true)
}

fn make_bundle_dir_with_patch_unchecked(
    repo_root: &std::path::Path,
    base_commit: &str,
    patch_text: &str,
    touched_files: &[&str],
) -> TempDir {
    make_bundle_dir_with_patch_impl(repo_root, base_commit, patch_text, touched_files, false)
}

fn make_bundle_dir_with_patch_impl(
    repo_root: &std::path::Path,
    base_commit: &str,
    patch_text: &str,
    touched_files: &[&str],
    check_apply: bool,
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

    if check_apply {
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
    }

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

    zip.finish().unwrap();
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

fn make_patch_adding_file(repo_root: &std::path::Path, rel: &str, body: &str) -> String {
    let path = repo_root.join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(&path, body).unwrap();

    let out = Command::new("git")
        .args([
            "diff",
            "--no-index",
            "--no-color",
            "--no-ext-diff",
            "--patch",
            "--full-index",
            "--",
            "/dev/null",
            rel,
        ])
        .current_dir(repo_root)
        .output()
        .expect("git diff --no-index")
        .stdout;
    let patch = String::from_utf8_lossy(&out).to_string();

    fs::remove_file(&path).unwrap();
    patch
}

#[cfg(unix)]
fn make_patch_adding_executable_file(repo_root: &std::path::Path, rel: &str, body: &str) -> String {
    let path = repo_root.join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(&path, body).unwrap();
    let mut perms = fs::metadata(&path).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&path, perms).unwrap();

    let out = Command::new("git")
        .args([
            "diff",
            "--no-index",
            "--no-color",
            "--no-ext-diff",
            "--patch",
            "--full-index",
            "--",
            "/dev/null",
            rel,
        ])
        .current_dir(repo_root)
        .output()
        .expect("git diff --no-index")
        .stdout;
    let patch = String::from_utf8_lossy(&out).to_string();

    fs::remove_file(&path).unwrap();
    patch
}

#[cfg(unix)]
fn make_patch_changing_readme_mode(repo_root: &std::path::Path) -> String {
    let readme = repo_root.join("README.md");
    let mut perms = fs::metadata(&readme).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&readme, perms).unwrap();

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

fn extract_run_id(stdout: &[u8]) -> String {
    let s = String::from_utf8_lossy(stdout);
    for line in s.lines() {
        if let Some(rest) = line.strip_prefix("  run_id  : ") {
            return rest.trim().to_string();
        }
    }
    panic!("run_id not found in output: {s}");
}

fn extract_sandbox_path(stdout: &[u8]) -> String {
    let s = String::from_utf8_lossy(stdout);
    for line in s.lines() {
        if let Some(rest) = line.strip_prefix("  sandbox : ") {
            return rest.trim().to_string();
        }
    }
    panic!("sandbox path not found in output: {s}");
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
    assert!(run_dir.join("commands.json").exists());
    assert!(run_dir.join("apply").join("01_preflight.stdout").exists());
    assert!(run_dir.join("apply").join("02_apply.stdout").exists());

    // verify should run on that run id, using the generic fallback (git diff --check).
    diffship_cmd()
        .args(["verify", "--run-id", &run_id])
        .current_dir(root)
        .assert()
        .success();
    assert!(run_dir.join("verify.json").exists());
    assert!(run_dir.join("verify").join("01_git.stdout").exists());

    let commands = fs::read_to_string(run_dir.join("commands.json")).unwrap();
    assert!(commands.contains("\"phase\": \"apply\""));
    assert!(commands.contains("\"phase\": \"verify\""));

    diffship_cmd()
        .args(["runs"])
        .current_dir(root)
        .assert()
        .success()
        .stdout(predicates::str::contains("commands=3"))
        .stdout(predicates::str::contains("phases=apply,verify"))
        .stdout(predicates::str::contains("commands_json="))
        .stdout(predicates::str::contains("phase_dirs="));

    diffship_cmd()
        .args(["status"])
        .current_dir(root)
        .assert()
        .success()
        .stdout(predicates::str::contains("run_dir="))
        .stdout(predicates::str::contains("commands_json="))
        .stdout(predicates::str::contains("phase_dirs="));
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
fn m2_apply_rejects_paths_forbidden_by_project_config() {
    let td = init_repo();
    let root = td.path();
    let base = head(root);

    write_project_config(
        root,
        r#"
[ops.forbid]
path1 = "README.md"
"#,
    );

    let patch = make_patch_by_editing_readme(root, "world\n");
    let bundle_td = make_bundle_dir_with_patch(root, &base, &patch, &["README.md"]);
    let bundle_root = bundle_td.path().join("patchship_test");

    diffship_cmd()
        .args(["apply", bundle_root.to_str().unwrap()])
        .current_dir(root)
        .assert()
        .failure()
        .code(7);
}

#[test]
fn m2_apply_rejects_paths_forbidden_by_dedicated_forbid_file() {
    let td = init_repo();
    let root = td.path();
    let base = head(root);

    write_project_forbid(
        root,
        r#"
[ops.forbid]
path1 = "README.md"
"#,
    );

    let patch = make_patch_by_editing_readme(root, "world\n");
    let bundle_td = make_bundle_dir_with_patch(root, &base, &patch, &["README.md"]);
    let bundle_root = bundle_td.path().join("patchship_test");

    diffship_cmd()
        .args(["apply", bundle_root.to_str().unwrap()])
        .current_dir(root)
        .assert()
        .failure()
        .code(7);
}

#[test]
fn m2_apply_rejects_diffship_project_kit_paths_by_default() {
    let td = init_repo();
    let root = td.path();
    let base = head(root);

    let patch = make_patch_adding_file(root, ".diffship/AI_GUIDE.md", "guide\n");
    let bundle_td = make_bundle_dir_with_patch(root, &base, &patch, &[".diffship/AI_GUIDE.md"]);
    let bundle_root = bundle_td.path().join("patchship_test");

    diffship_cmd()
        .args(["apply", bundle_root.to_str().unwrap()])
        .current_dir(root)
        .assert()
        .failure()
        .code(7);
}

#[test]
fn m2_apply_allows_opted_in_diffship_config_path_via_ai_generated_config() {
    let td = init_repo();
    let root = td.path();
    let base = head(root);

    write_project_ai_generated_config(
        root,
        r#"
[ops.editable_diffship]
path1 = ".diffship/config.toml"
"#,
    );

    let patch = make_patch_adding_file(
        root,
        ".diffship/config.toml",
        "[verify]\ndefault_profile = \"fast\"\n",
    );
    let bundle_td = make_bundle_dir_with_patch(root, &base, &patch, &[".diffship/config.toml"]);
    let bundle_root = bundle_td.path().join("patchship_test");

    let out = diffship_cmd()
        .args(["apply", "--keep-sandbox", bundle_root.to_str().unwrap()])
        .current_dir(root)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let sandbox = extract_sandbox_path(&out);
    let sandbox_cfg = std::path::Path::new(&sandbox)
        .join(".diffship")
        .join("config.toml");
    assert_eq!(
        fs::read_to_string(sandbox_cfg).unwrap(),
        "[verify]\ndefault_profile = \"fast\"\n"
    );
}

#[test]
fn m2_apply_rejects_non_allowlisted_diffship_paths_even_if_configured() {
    let td = init_repo();
    let root = td.path();
    let base = head(root);

    write_project_ai_generated_config(
        root,
        r#"
[ops.editable_diffship]
path1 = ".diffship/runs/escape.txt"
"#,
    );

    let patch = make_patch_adding_file(root, ".diffship/runs/escape.txt", "nope\n");
    let bundle_td = make_bundle_dir_with_patch(root, &base, &patch, &[".diffship/runs/escape.txt"]);
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

    let runs_dir = root.join(".diffship").join("runs");
    let latest = fs::read_dir(&runs_dir)
        .unwrap()
        .filter_map(|ent| ent.ok().map(|e| e.path()))
        .filter(|path| path.is_dir())
        .max()
        .unwrap();
    let apply: Value =
        serde_json::from_str(&fs::read_to_string(latest.join("apply.json")).unwrap()).unwrap();
    assert_eq!(
        apply.get("failure_category").and_then(|v| v.as_str()),
        Some("base_commit_mismatch")
    );
}

#[test]
fn m2_apply_accepts_base_commit_override_when_it_matches_session_head() {
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
    let patch = make_patch_by_editing_readme(root, "world\n");
    let bundle_td = make_bundle_dir_with_patch(root, &old_base, &patch, &["README.md"]);
    let bundle_root = bundle_td.path().join("patchship_test");

    let out = diffship_cmd()
        .args([
            "apply",
            bundle_root.to_str().unwrap(),
            "--base-commit",
            &current_head,
        ])
        .current_dir(root)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let run_id = extract_run_id(&out);
    let run_dir = root.join(".diffship").join("runs").join(&run_id);

    let apply_json = fs::read(run_dir.join("apply.json")).unwrap();
    let apply: serde_json::Value = serde_json::from_slice(&apply_json).unwrap();
    assert_eq!(
        apply.get("declared_base_commit").and_then(|v| v.as_str()),
        Some(old_base.as_str())
    );
    assert_eq!(
        apply.get("effective_base_commit").and_then(|v| v.as_str()),
        Some(current_head.as_str())
    );

    let manifest = fs::read_to_string(run_dir.join("bundle").join("manifest.yaml")).unwrap();
    assert!(manifest.contains(&format!("base_commit: \"{}\"", current_head)));
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
cmd1 = "printf '%s\\n' \"$TMPDIR\" && printf hook >> README.md"
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
    let tmpdir = fs::read_to_string(run_dir.join("post-apply").join("01_cmd1.stdout")).unwrap();
    let tmpdir = std::path::PathBuf::from(tmpdir.trim());

    assert!(readme.contains("world\n"));
    assert!(readme.contains("hook"));
    assert!(tmpdir.to_string_lossy().contains(".diffship/tmp/commands/"));
    assert!(tmpdir.to_string_lossy().contains(&run_id));
    assert!(!tmpdir.exists());
    assert!(run_dir.join("post_apply.json").exists());
    assert!(run_dir.join("post-apply").join("01_cmd1.stdout").exists());
    let post_apply: Value =
        serde_json::from_str(&fs::read_to_string(run_dir.join("post_apply.json")).unwrap())
            .unwrap();
    assert_eq!(
        post_apply
            .get("changed_paths")
            .and_then(|v| v.get(0))
            .and_then(|v| v.as_str()),
        Some("README.md")
    );
    assert_eq!(
        post_apply
            .get("change_categories")
            .and_then(|v| v.get(0))
            .and_then(|v| v.as_str()),
        Some("docs_touch")
    );
    assert_eq!(
        post_apply
            .get("normalization_summary")
            .and_then(|v| v.get("changed_path_count"))
            .and_then(|v| v.as_u64()),
        Some(1)
    );
    let commands = fs::read_to_string(run_dir.join("commands.json")).unwrap();
    assert!(commands.contains("\"phase\": \"post-apply\""));
    assert!(
        !root
            .join(".diffship")
            .join("tmp")
            .join("commands")
            .join(&run_id)
            .exists()
    );
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
    assert!(stderr.contains("pack-fix saved to"));

    let runs_dir = root.join(".diffship").join("runs");
    let latest = fs::read_dir(&runs_dir)
        .unwrap()
        .filter_map(|ent| ent.ok().map(|e| e.path()))
        .filter(|path| path.is_dir())
        .max()
        .unwrap();
    assert!(latest.join("post_apply.json").exists());
    assert!(
        latest
            .read_dir()
            .unwrap()
            .filter_map(|ent| ent.ok().map(|e| e.path()))
            .any(|path| path.extension().and_then(|ext| ext.to_str()) == Some("zip"))
    );
    let apply_json = fs::read_to_string(latest.join("apply.json")).unwrap();
    assert!(apply_json.contains("\"pack_fix_path\":"));
    let apply: Value = serde_json::from_str(&apply_json).unwrap();
    assert_eq!(
        apply.get("failure_category").and_then(|v| v.as_str()),
        Some("post_apply_failed")
    );
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

#[test]
fn m2_apply_can_delete_input_zip_after_copying_bundle() {
    let td = init_repo();
    let root = td.path();
    let base = head(root);

    let patch = make_patch_by_editing_readme(root, "world\n");
    let bundle_td = make_bundle_dir_with_patch(root, &base, &patch, &["README.md"]);
    let bundle_root = bundle_td.path().join("patchship_test");
    let bundle_zip = bundle_td.path().join("patchship_test.zip");
    write_patch_bundle_zip(&bundle_root, &bundle_zip);

    let out = diffship_cmd()
        .args(["apply", "--delete-input-zip"])
        .arg(&bundle_zip)
        .current_dir(root)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let run_id = extract_run_id(&out);

    assert!(!bundle_zip.exists());
    let apply_json = fs::read_to_string(
        root.join(".diffship")
            .join("runs")
            .join(run_id)
            .join("apply.json"),
    )
    .unwrap();
    assert!(apply_json.contains("\"delete_input_zip_requested\": true"));
    assert!(apply_json.contains("\"input_zip_deleted\": true"));
}

#[test]
fn m2_apply_accepts_new_text_file_in_patch_bundle() {
    let td = init_repo();
    let root = td.path();
    let base = head(root);

    let patch = make_patch_adding_file(root, "notes.txt", "hello\n");
    let bundle_td = make_bundle_dir_with_patch(root, &base, &patch, &["notes.txt"]);
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
    let sandbox_file = root
        .join(".diffship")
        .join("worktrees")
        .join("sandboxes")
        .join(&run_id)
        .join("notes.txt");

    assert_eq!(fs::read_to_string(sandbox_file).unwrap(), "hello\n");
}

#[cfg(unix)]
#[test]
fn m2_apply_accepts_new_executable_file_in_patch_bundle() {
    let td = init_repo();
    let root = td.path();
    let base = head(root);

    let patch = make_patch_adding_executable_file(root, "script.sh", "#!/bin/sh\necho hi\n");
    let bundle_td = make_bundle_dir_with_patch(root, &base, &patch, &["script.sh"]);
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
    let sandbox_file = root
        .join(".diffship")
        .join("worktrees")
        .join("sandboxes")
        .join(&run_id)
        .join("script.sh");
    let mode = fs::metadata(&sandbox_file).unwrap().permissions().mode() & 0o777;

    assert_eq!(
        fs::read_to_string(&sandbox_file).unwrap(),
        "#!/bin/sh\necho hi\n"
    );
    assert_eq!(mode, 0o755);
}

#[cfg(unix)]
#[test]
fn m2_apply_refuses_existing_file_mode_changes() {
    let td = init_repo();
    let root = td.path();
    let base = head(root);

    let patch = make_patch_changing_readme_mode(root);
    let bundle_td = make_bundle_dir_with_patch_unchecked(root, &base, &patch, &["README.md"]);
    let bundle_root = bundle_td.path().join("patchship_test");

    diffship_cmd()
        .args(["apply", bundle_root.to_str().unwrap()])
        .current_dir(root)
        .assert()
        .failure()
        .code(7);
}

#[test]
fn m2_apply_refuses_unsupported_new_file_mode() {
    let td = init_repo();
    let root = td.path();
    let base = head(root);

    let patch = make_patch_adding_file(root, "link.txt", "hello\n").replacen(
        "new file mode 100644",
        "new file mode 120000",
        1,
    );
    let bundle_td = make_bundle_dir_with_patch_unchecked(root, &base, &patch, &["link.txt"]);
    let bundle_root = bundle_td.path().join("patchship_test");

    diffship_cmd()
        .args(["apply", bundle_root.to_str().unwrap()])
        .current_dir(root)
        .assert()
        .failure()
        .code(7);
}

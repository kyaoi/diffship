use assert_cmd::prelude::*;
use std::fs;
use std::process::Command;
use tempfile::TempDir;

fn init_repo_with_branches(branches: &[&str]) -> TempDir {
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

    for br in branches {
        Command::new("git")
            .args(["branch", br])
            .current_dir(root)
            .assert()
            .success();
    }

    // Prefer develop as a working branch when present.

    if branches.contains(&"develop") {
        Command::new("git")
            .args(["checkout", "-q", "develop"])
            .current_dir(root)
            .assert()
            .success();
    }

    td
}

fn diffship_cmd(home: &std::path::Path) -> Command {
    let mut c = Command::new(assert_cmd::cargo::cargo_bin!("diffship"));
    c.env("HOME", home);
    c
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
    extra_manifest: &str,
) -> TempDir {
    let td = tempfile::tempdir().expect("bundle tempdir");
    let root = td.path();

    let bundle_root = root.join("patchship_test");
    fs::create_dir_all(bundle_root.join("changes")).unwrap();

    let manifest = format!(
        "protocol_version: \"1\"\n\
task_id: \"TEST\"\n\
base_commit: \"{}\"\n\
apply_mode: git-apply\n\
touched_files:\n{}\n{}",
        base_commit,
        touched_files
            .iter()
            .map(|p| format!("  - \"{}\"", p))
            .collect::<Vec<_>>()
            .join("\n"),
        extra_manifest
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

fn extract_run_id(stdout: &[u8]) -> String {
    let s = String::from_utf8_lossy(stdout);
    for line in s.lines() {
        if let Some(rest) = line.strip_prefix("  run_id  : ") {
            return rest.trim().to_string();
        }
    }
    panic!("run_id not found in output: {s}");
}

fn write_global_config(home: &std::path::Path, body: &str) {
    let p = home.join(".config").join("diffship");
    fs::create_dir_all(&p).unwrap();
    fs::write(p.join("config.toml"), body).unwrap();
}

fn write_project_config(repo_root: &std::path::Path, body: &str) {
    let p = repo_root.join(".diffship");
    fs::create_dir_all(&p).unwrap();
    fs::write(p.join("config.toml"), body).unwrap();
}

fn read_verify_profile(repo_root: &std::path::Path, run_id: &str) -> String {
    let p = repo_root
        .join(".diffship")
        .join("runs")
        .join(run_id)
        .join("verify.json");
    let s = fs::read_to_string(&p).expect("verify.json");
    // minimal parse without serde_json dependency
    for line in s.lines() {
        if line.trim_start().starts_with("\"profile\"") {
            // "profile": "full",
            if let Some((_, rhs)) = line.split_once(':') {
                let v = rhs.trim().trim_end_matches(',').trim();
                return v.trim_matches('"').to_string();
            }
        }
    }
    panic!("profile not found: {s}");
}

fn read_promotion_target(repo_root: &std::path::Path, run_id: &str) -> String {
    let p = repo_root
        .join(".diffship")
        .join("runs")
        .join(run_id)
        .join("promotion.json");
    let s = fs::read_to_string(&p).expect("promotion.json");
    for line in s.lines() {
        if line.trim_start().starts_with("\"target_branch\"")
            && let Some((_, rhs)) = line.split_once(':')
        {
            let v = rhs.trim().trim_end_matches(',').trim();
            return v.trim_matches('"').to_string();
        }
    }
    panic!("target_branch not found: {s}");
}

#[test]
fn m4_config_precedence_manifest_overrides_project_global() {
    let home_td = tempfile::tempdir().expect("home");
    let home = home_td.path();

    let td = init_repo_with_branches(&[
        "develop",
        "global_branch",
        "project_branch",
        "manifest_branch",
    ]);
    let root = td.path();
    let base = head(root);

    write_global_config(
        home,
        r#"
[verify]
default_profile = "fast"

[ops.promote]
target_branch = "global_branch"
"#,
    );
    write_project_config(
        root,
        r#"
[verify]
default_profile = "standard"

[ops.promote]
target_branch = "project_branch"
"#,
    );

    let patch = make_patch_by_editing_readme(root, "from-manifest\n");
    let bundle_td = make_bundle_dir_with_patch(
        root,
        &base,
        &patch,
        &["README.md"],
        "verify_profile: \"full\"\ntarget_branch: \"manifest_branch\"\n",
    );
    let bundle_root = bundle_td.path().join("patchship_test");

    let out = diffship_cmd(home)
        .args(["loop", bundle_root.to_str().unwrap()])
        .current_dir(root)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let run_id = extract_run_id(&out);

    assert_eq!(read_verify_profile(root, &run_id), "full");
    assert_eq!(read_promotion_target(root, &run_id), "manifest_branch");
    // And the target branch should have advanced.
    assert_ne!(head(root), base);
}

#[test]
fn m4_config_precedence_cli_overrides_manifest() {
    let home_td = tempfile::tempdir().expect("home");
    let home = home_td.path();

    let td = init_repo_with_branches(&["develop", "manifest_branch", "cli_branch"]);
    let root = td.path();
    let base = head(root);

    write_global_config(
        home,
        r#"
[verify]
default_profile = "fast"

[ops.promote]
target_branch = "manifest_branch"
"#,
    );

    let patch = make_patch_by_editing_readme(root, "from-cli\n");
    let bundle_td = make_bundle_dir_with_patch(
        root,
        &base,
        &patch,
        &["README.md"],
        "verify_profile: \"fast\"\ntarget_branch: \"manifest_branch\"\n",
    );
    let bundle_root = bundle_td.path().join("patchship_test");

    let out = diffship_cmd(home)
        .args([
            "loop",
            bundle_root.to_str().unwrap(),
            "--profile",
            "full",
            "--target-branch",
            "cli_branch",
        ])
        .current_dir(root)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let run_id = extract_run_id(&out);

    assert_eq!(read_verify_profile(root, &run_id), "full");
    assert_eq!(read_promotion_target(root, &run_id), "cli_branch");
    assert_ne!(head(root), base);
}

#[test]
fn m4_config_precedence_project_overrides_global_when_manifest_absent() {
    let home_td = tempfile::tempdir().expect("home");
    let home = home_td.path();

    let td = init_repo_with_branches(&["develop", "global_branch", "project_branch"]);
    let root = td.path();
    let base = head(root);

    write_global_config(
        home,
        r#"
[verify]
default_profile = "fast"

[ops.promote]
target_branch = "global_branch"
"#,
    );
    write_project_config(
        root,
        r#"
[verify]
default_profile = "full"

[ops.promote]
target_branch = "project_branch"
"#,
    );

    let patch = make_patch_by_editing_readme(root, "from-project\n");
    let bundle_td = make_bundle_dir_with_patch(root, &base, &patch, &["README.md"], "");
    let bundle_root = bundle_td.path().join("patchship_test");

    let out = diffship_cmd(home)
        .args(["loop", bundle_root.to_str().unwrap()])
        .current_dir(root)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let run_id = extract_run_id(&out);

    assert_eq!(read_verify_profile(root, &run_id), "full");
    assert_eq!(read_promotion_target(root, &run_id), "project_branch");
    assert_ne!(head(root), base);
}

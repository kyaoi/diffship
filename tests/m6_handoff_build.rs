use assert_cmd::prelude::*;
use serde_json::Value;
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

    td
}

fn commit_all(root: &Path, msg: &str) {
    Command::new("git")
        .args(["add", "."])
        .current_dir(root)
        .assert()
        .success();
    Command::new("git")
        .args(["commit", "-m", msg, "-q"])
        .current_dir(root)
        .assert()
        .success();
}

fn git_stdout(root: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .unwrap();
    assert!(out.status.success(), "git {:?} failed", args);
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn write_global_config(home: &Path, body: &str) {
    let path = home.join(".config").join("diffship");
    fs::create_dir_all(&path).unwrap();
    fs::write(path.join("config.toml"), body).unwrap();
}

fn write_project_config(root: &Path, body: &str) {
    let path = root.join(".diffship");
    fs::create_dir_all(&path).unwrap();
    fs::write(path.join("config.toml"), body).unwrap();
}

fn write_project_ai_generated_config(root: &Path, body: &str) {
    let path = root.join(".diffship");
    fs::create_dir_all(&path).unwrap();
    fs::write(path.join("ai_generated_config.toml"), body).unwrap();
}

fn only_generated_bundle(root: &Path) -> std::path::PathBuf {
    let mut bundles = fs::read_dir(root)
        .unwrap()
        .filter_map(|ent| {
            let ent = ent.ok()?;
            if !ent.file_type().ok()?.is_dir() {
                return None;
            }
            let name = ent.file_name().to_string_lossy().to_string();
            if !name.starts_with("diffship_") {
                return None;
            }
            Some(ent.path())
        })
        .collect::<Vec<_>>();
    bundles.sort();
    assert_eq!(bundles.len(), 1, "expected exactly one generated bundle");
    bundles.remove(0)
}

#[test]
fn build_default_out_creates_bundle_dir_and_uses_last_range() {
    let td = init_repo();
    let root = td.path();

    fs::write(root.join("a.txt"), "one\n").unwrap();
    commit_all(root, "c1");

    fs::write(root.join("a.txt"), "two\n").unwrap();
    commit_all(root, "c2");

    let head = git_stdout(root, &["rev-parse", "HEAD"]);

    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("diffship");
    cmd.current_dir(root).arg("build");
    cmd.assert().success();

    let mut second = assert_cmd::cargo::cargo_bin_cmd!("diffship");
    second.current_dir(root).arg("build");
    second.assert().success();

    let mut bundles = vec![];
    for ent in fs::read_dir(root).unwrap() {
        let ent = ent.unwrap();
        if ent.file_type().unwrap().is_dir() {
            let name = ent.file_name().to_string_lossy().to_string();
            if name.starts_with("diffship_") {
                bundles.push(ent.path());
            }
        }
    }
    bundles.sort();
    assert_eq!(bundles.len(), 2);
    assert_ne!(bundles[0], bundles[1]);
    let head7 = &head[..7];
    let first_name = bundles[0].file_name().unwrap().to_string_lossy();
    let second_name = bundles[1].file_name().unwrap().to_string_lossy();
    assert!(first_name.starts_with("diffship_"));
    assert!(first_name.contains(&format!("_{head7}")));
    assert!(
        second_name.starts_with(&format!("{first_name}_"))
            || second_name.contains(&format!("_{head7}_2"))
    );

    let bundle = &bundles[0];
    assert!(bundle.join("HANDOFF.md").exists());
    assert!(bundle.join("AI_REQUESTS.md").exists());
    assert!(bundle.join("handoff.manifest.json").exists());
    assert!(bundle.join("handoff.context.xml").exists());
    assert!(bundle.join("parts").join("part_01.patch").exists());
    assert!(bundle.join("parts").join("part_01.context.json").exists());

    let handoff = fs::read_to_string(bundle.join("HANDOFF.md")).unwrap();
    assert!(handoff.contains("# HANDOFF"));
    assert!(handoff.contains("## Start Here"));
    assert!(handoff.contains("## TL;DR"));
    assert!(handoff.contains(
        "Segments included: committed=`yes`, staged=`no`, unstaged=`no`, untracked=`no`"
    ));
    assert!(handoff.contains(&head));
    assert!(handoff.contains("## 3) Parts Index"));
    assert!(handoff.contains("### 3.1 Quick index"));
    assert!(handoff.contains("### 3.2 Part details"));
    assert!(handoff.contains("5. Open the first patch part: `parts/part_01.patch`"));

    let ai_requests = fs::read_to_string(bundle.join("AI_REQUESTS.md")).unwrap();
    assert!(ai_requests.contains("# AI REQUESTS"));
    assert!(ai_requests.contains("MODE: ANALYSIS_ONLY"));
    assert!(ai_requests.contains("MODE: OPS_PATCH_BUNDLE"));
    assert!(ai_requests.contains(&head));
    assert!(!ai_requests.contains("## Focused project-context guidance"));
    assert!(ai_requests.contains("## Task-group execution order"));
    assert!(ai_requests.contains("Treat `review_labels` as generation/review strategy hints"));
    assert!(ai_requests.contains(
        "Use `task_shape_labels` to decide whether a task is single-area or cross-cutting"
    ));
    assert!(ai_requests.contains(
        "Use `edit_targets` as the bounded write scope for the task and `context_only_files` as read-only context"
    ));
    assert!(
        ai_requests
            .contains("Use `verification_labels` to keep verification strategy coarse and bounded")
    );
    assert!(ai_requests.contains("Use `widening_labels` to decide whether to stay patch-first or widen into related tests/config/docs/repo rules."));
    assert!(
        ai_requests
            .contains("Use `execution_labels` to keep the execution flow coarse and deterministic")
    );
    assert!(ai_requests.contains("`task_01` primary=`other_task` shape=`single_area` review=`mechanical_update_like` intents=`other_update` risks=`-` edit=`a.txt` context=`-` verify=`-` verify-strategy=`sanity_check_first` widen=`patch_only` execute=`mechanical_first,patch_only_flow,verify_after_edit` read=`parts/part_01.context.json,parts/part_01.patch` project=`-`"));
    assert!(ai_requests.contains("## Patch-part guidance"));
    assert!(ai_requests.contains("Reuse part `review_labels` before editing"));
    assert!(ai_requests.contains("Use `parts/part_XX.context.json` when you need machine-readable part-local facts such as `intent_labels`, `scoped_context`, and per-file semantic hints."));
    assert!(ai_requests.contains("`parts/part_01.patch` context=`parts/part_01.context.json` review=`mechanical_update_like` intents=`other_update` segments=`committed` files=`a.txt`"));

    let part = fs::read_to_string(bundle.join("parts").join("part_01.patch")).unwrap();
    assert!(part.contains("diffship segment: committed"));
    assert!(part.contains("a.txt"));
    assert!(part.contains("+two"));

    let manifest: Value =
        serde_json::from_str(&fs::read_to_string(bundle.join("handoff.manifest.json")).unwrap())
            .unwrap();
    assert_eq!(
        manifest.get("schema_version").and_then(|v| v.as_u64()),
        Some(1)
    );
    assert_eq!(
        manifest.get("patch_canonical").and_then(|v| v.as_bool()),
        Some(true)
    );
    assert_eq!(
        manifest.get("entrypoint").and_then(|v| v.as_str()),
        Some("HANDOFF.md")
    );
    assert_eq!(
        manifest.get("current_head").and_then(|v| v.as_str()),
        Some(head.as_str())
    );
    assert_eq!(
        manifest
            .get("sources")
            .and_then(|v| v.get("committed"))
            .and_then(|v| v.as_bool()),
        Some(true)
    );
    assert_eq!(
        manifest
            .get("artifacts")
            .and_then(|v| v.get("ai_requests_md"))
            .and_then(|v| v.as_str()),
        Some("AI_REQUESTS.md")
    );
    assert_eq!(
        manifest
            .get("artifacts")
            .and_then(|v| v.get("manifest_json"))
            .and_then(|v| v.as_str()),
        Some("handoff.manifest.json")
    );
    assert_eq!(
        manifest
            .get("artifacts")
            .and_then(|v| v.get("context_xml"))
            .and_then(|v| v.as_str()),
        Some("handoff.context.xml")
    );
    assert_eq!(
        manifest
            .get("artifacts")
            .and_then(|v| v.get("part_paths"))
            .and_then(|v| v.get(0))
            .and_then(|v| v.as_str()),
        Some("parts/part_01.patch")
    );
    assert_eq!(
        manifest
            .get("parts")
            .and_then(|v| v.get(0))
            .and_then(|v| v.get("patch_path"))
            .and_then(|v| v.as_str()),
        Some("parts/part_01.patch")
    );
    assert_eq!(
        manifest
            .get("parts")
            .and_then(|v| v.get(0))
            .and_then(|v| v.get("context_path"))
            .and_then(|v| v.as_str()),
        Some("parts/part_01.context.json")
    );
    assert_eq!(
        manifest
            .get("task_groups")
            .and_then(|v| v.get(0))
            .and_then(|v| v.get("intent_labels"))
            .and_then(|v| v.get(0))
            .and_then(|v| v.as_str()),
        Some("other_update")
    );
    assert_eq!(
        manifest
            .get("task_groups")
            .and_then(|v| v.get(0))
            .and_then(|v| v.get("primary_labels"))
            .and_then(|v| v.get(0))
            .and_then(|v| v.as_str()),
        Some("other_task")
    );
    assert_eq!(
        manifest
            .get("task_groups")
            .and_then(|v| v.get(0))
            .and_then(|v| v.get("part_count"))
            .and_then(|v| v.as_u64()),
        Some(1)
    );
    assert_eq!(
        manifest
            .get("task_groups")
            .and_then(|v| v.get(0))
            .and_then(|v| v.get("related_context_paths"))
            .and_then(|v| v.get(0))
            .and_then(|v| v.as_str()),
        Some("parts/part_01.context.json")
    );
    assert_eq!(
        manifest
            .get("task_groups")
            .and_then(|v| v.get(0))
            .and_then(|v| v.get("task_shape_labels"))
            .and_then(|v| v.as_array())
            .map(|items| items
                .iter()
                .filter_map(|item| item.as_str())
                .collect::<Vec<_>>()),
        Some(vec!["single_area"])
    );
    assert_eq!(
        manifest
            .get("task_groups")
            .and_then(|v| v.get(0))
            .and_then(|v| v.get("edit_targets"))
            .and_then(|v| v.as_array())
            .map(|items| items
                .iter()
                .filter_map(|item| item.as_str())
                .collect::<Vec<_>>()),
        Some(vec!["a.txt"])
    );
    assert_eq!(
        manifest
            .get("task_groups")
            .and_then(|v| v.get(0))
            .and_then(|v| v.get("context_only_files"))
            .and_then(|v| v.as_array())
            .map(|items| items
                .iter()
                .filter_map(|item| item.as_str())
                .collect::<Vec<_>>()),
        Some(Vec::<&str>::new())
    );
    assert_eq!(
        manifest
            .get("task_groups")
            .and_then(|v| v.get(0))
            .and_then(|v| v.get("verification_labels"))
            .and_then(|v| v.as_array())
            .map(|items| items
                .iter()
                .filter_map(|item| item.as_str())
                .collect::<Vec<_>>()),
        Some(vec!["sanity_check_first"])
    );
    assert_eq!(
        manifest
            .get("task_groups")
            .and_then(|v| v.get(0))
            .and_then(|v| v.get("widening_labels"))
            .and_then(|v| v.as_array())
            .map(|items| items
                .iter()
                .filter_map(|item| item.as_str())
                .collect::<Vec<_>>()),
        Some(vec!["patch_only"])
    );
    assert_eq!(
        manifest
            .get("task_groups")
            .and_then(|v| v.get(0))
            .and_then(|v| v.get("execution_labels"))
            .and_then(|v| v.as_array())
            .map(|items| items
                .iter()
                .filter_map(|item| item.as_str())
                .collect::<Vec<_>>()),
        Some(vec![
            "mechanical_first",
            "patch_only_flow",
            "verify_after_edit",
        ])
    );
    assert_eq!(
        manifest
            .get("task_groups")
            .and_then(|v| v.get(0))
            .and_then(|v| v.get("suggested_read_order"))
            .and_then(|v| v.get(0))
            .and_then(|v| v.as_str()),
        Some("parts/part_01.context.json")
    );
    assert_eq!(
        manifest
            .get("summary")
            .and_then(|v| v.get("file_count"))
            .and_then(|v| v.as_u64()),
        Some(1)
    );
    assert_eq!(
        manifest
            .get("summary")
            .and_then(|v| v.get("part_count"))
            .and_then(|v| v.as_u64()),
        Some(1)
    );
    assert_eq!(
        manifest
            .get("summary")
            .and_then(|v| v.get("segments"))
            .and_then(|v| v.get("committed"))
            .and_then(|v| v.as_u64()),
        Some(1)
    );
    assert_eq!(
        manifest
            .get("summary")
            .and_then(|v| v.get("statuses"))
            .and_then(|v| v.get("M"))
            .and_then(|v| v.as_u64()),
        Some(1)
    );
    assert_eq!(
        manifest
            .get("reading_order")
            .and_then(|v| v.get(0))
            .and_then(|v| v.as_str()),
        Some("Other changes: `part_01.patch` (1 files)")
    );
    assert_eq!(
        manifest
            .get("files")
            .and_then(|v| v.get(0))
            .and_then(|v| v.get("path"))
            .and_then(|v| v.as_str()),
        Some("a.txt")
    );

    let part_context: Value = serde_json::from_str(
        &fs::read_to_string(bundle.join("parts").join("part_01.context.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        part_context
            .get("patch_canonical")
            .and_then(|v| v.as_bool()),
        Some(true)
    );
    assert_eq!(
        part_context.get("part_id").and_then(|v| v.as_str()),
        Some("part_01.patch")
    );
    assert_eq!(
        part_context.get("patch_path").and_then(|v| v.as_str()),
        Some("parts/part_01.patch")
    );
    assert_eq!(
        part_context.get("context_path").and_then(|v| v.as_str()),
        Some("parts/part_01.context.json")
    );
    assert_eq!(
        part_context
            .get("files")
            .and_then(|v| v.get(0))
            .and_then(|v| v.get("path"))
            .and_then(|v| v.as_str()),
        Some("a.txt")
    );
    assert_eq!(
        part_context
            .get("constraints")
            .and_then(|v| v.get("manifest_path"))
            .and_then(|v| v.as_str()),
        Some("handoff.manifest.json")
    );
    assert_eq!(
        part_context
            .get("review_labels")
            .and_then(|v| v.as_array())
            .map(|items| items
                .iter()
                .filter_map(|item| item.as_str())
                .collect::<Vec<_>>()),
        Some(vec!["mechanical_update_like"])
    );
    assert_eq!(
        part_context
            .get("diff_stats")
            .and_then(|v| v.get("segments"))
            .and_then(|v| v.get("committed"))
            .and_then(|v| v.as_u64()),
        Some(1)
    );
    assert_eq!(
        part_context
            .get("diff_stats")
            .and_then(|v| v.get("statuses"))
            .and_then(|v| v.get("M"))
            .and_then(|v| v.as_u64()),
        Some(1)
    );

    let handoff_context = fs::read_to_string(bundle.join("handoff.context.xml")).unwrap();
    assert!(handoff_context.contains("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
    assert!(handoff_context.contains("<handoff-context"));
    assert!(handoff_context.contains("rendered-from=\"handoff.manifest.json\""));
    assert!(handoff_context.contains("path=\"parts/part_01.context.json\""));
}

#[test]
fn build_structured_context_includes_file_semantic_facts() {
    let td = init_repo();
    let root = td.path();

    fs::create_dir_all(root.join("src")).unwrap();
    fs::create_dir_all(root.join("tests")).unwrap();
    fs::create_dir_all(root.join("target/generated")).unwrap();
    fs::create_dir_all(root.join(".github/workflows")).unwrap();

    fs::write(
        root.join("src/lib.rs"),
        "use crate::old_dep;\npub fn value() -> i32 { 1 }\n",
    )
    .unwrap();
    fs::write(root.join("tests/lib_test.rs"), "#[test]\nfn smoke() {}\n").unwrap();
    fs::write(root.join("Cargo.lock"), "version = 3\n").unwrap();
    fs::write(root.join(".github/workflows/ci.yml"), "name: ci\n").unwrap();
    fs::write(
        root.join("target/generated/schema.generated.json"),
        "{\"v\":1}\n",
    )
    .unwrap();
    commit_all(root, "c1");

    fs::write(
        root.join("src/lib.rs"),
        "use crate::new_dep;\npub fn value(input: i32) -> i32 { input + 2 }\n",
    )
    .unwrap();
    fs::write(root.join("Cargo.lock"), "version = 4\n").unwrap();
    fs::write(
        root.join(".github/workflows/ci.yml"),
        "name: ci\non: push\n",
    )
    .unwrap();
    fs::write(root.join("AGENTS.md"), "Repository rules.\n").unwrap();
    fs::create_dir_all(root.join("tests/fixtures")).unwrap();
    fs::write(root.join("tests/fixtures/api.json"), "{\"ok\":true}\n").unwrap();
    fs::write(
        root.join("target/generated/schema.generated.json"),
        "{\"v\":2}\n",
    )
    .unwrap();
    commit_all(root, "c2");

    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("diffship");
    cmd.current_dir(root).arg("build");
    cmd.assert().success();

    let bundle = only_generated_bundle(root);
    let manifest: Value =
        serde_json::from_str(&fs::read_to_string(bundle.join("handoff.manifest.json")).unwrap())
            .unwrap();

    let files = manifest
        .get("files")
        .and_then(|v| v.as_array())
        .expect("manifest files array");

    let src_entry = files
        .iter()
        .find(|entry| entry.get("path").and_then(|v| v.as_str()) == Some("src/lib.rs"))
        .expect("src/lib.rs entry");
    assert_eq!(
        src_entry
            .get("semantic")
            .and_then(|v| v.get("language"))
            .and_then(|v| v.as_str()),
        Some("rust")
    );
    assert_eq!(
        src_entry
            .get("semantic")
            .and_then(|v| v.get("generated_like"))
            .and_then(|v| v.as_bool()),
        Some(false)
    );
    assert_eq!(
        src_entry
            .get("semantic")
            .and_then(|v| v.get("related_test_candidates"))
            .and_then(|v| v.get(0))
            .and_then(|v| v.as_str()),
        Some("tests/lib_test.rs")
    );
    let src_labels = src_entry
        .get("semantic")
        .and_then(|v| v.get("coarse_labels"))
        .and_then(|v| v.as_array())
        .map(|labels| {
            labels
                .iter()
                .filter_map(|label| label.as_str())
                .collect::<Vec<_>>()
        })
        .expect("src/lib.rs coarse labels");
    assert!(src_labels.contains(&"api_surface_like"));
    assert!(src_labels.contains(&"signature_change_like"));

    let lockfile_entry = files
        .iter()
        .find(|entry| entry.get("path").and_then(|v| v.as_str()) == Some("Cargo.lock"))
        .expect("Cargo.lock entry");
    assert_eq!(
        lockfile_entry
            .get("semantic")
            .and_then(|v| v.get("lockfile"))
            .and_then(|v| v.as_bool()),
        Some(true)
    );
    assert_eq!(
        lockfile_entry
            .get("semantic")
            .and_then(|v| v.get("language"))
            .and_then(|v| v.as_str()),
        Some("unknown")
    );
    assert_eq!(
        lockfile_entry
            .get("semantic")
            .and_then(|v| v.get("coarse_labels"))
            .and_then(|v| v.as_array())
            .map(|labels| {
                labels
                    .iter()
                    .filter_map(|label| label.as_str())
                    .collect::<Vec<_>>()
            }),
        Some(vec![
            "config_only",
            "dependency_policy_touch",
            "lockfile_touch",
        ])
    );

    let tooling_entry = files
        .iter()
        .find(|entry| {
            entry.get("path").and_then(|v| v.as_str()) == Some(".github/workflows/ci.yml")
        })
        .expect("workflow entry");
    assert_eq!(
        tooling_entry
            .get("semantic")
            .and_then(|v| v.get("ci_or_tooling"))
            .and_then(|v| v.as_bool()),
        Some(true)
    );
    assert_eq!(
        tooling_entry
            .get("semantic")
            .and_then(|v| v.get("language"))
            .and_then(|v| v.as_str()),
        Some("yaml")
    );
    assert_eq!(
        tooling_entry
            .get("semantic")
            .and_then(|v| v.get("coarse_labels"))
            .and_then(|v| v.as_array())
            .map(|labels| {
                labels
                    .iter()
                    .filter_map(|label| label.as_str())
                    .collect::<Vec<_>>()
            }),
        Some(vec!["ci_or_tooling_touch", "config_only"])
    );

    let repo_rule_entry = files
        .iter()
        .find(|entry| entry.get("path").and_then(|v| v.as_str()) == Some("AGENTS.md"))
        .expect("AGENTS.md entry");
    assert_eq!(
        repo_rule_entry
            .get("semantic")
            .and_then(|v| v.get("coarse_labels"))
            .and_then(|v| v.as_array())
            .map(|labels| {
                labels
                    .iter()
                    .filter_map(|label| label.as_str())
                    .collect::<Vec<_>>()
            }),
        Some(vec!["docs_only", "repo_rule_touch"])
    );

    let fixture_entry = files
        .iter()
        .find(|entry| entry.get("path").and_then(|v| v.as_str()) == Some("tests/fixtures/api.json"))
        .expect("fixture entry");
    assert_eq!(
        fixture_entry
            .get("semantic")
            .and_then(|v| v.get("coarse_labels"))
            .and_then(|v| v.as_array())
            .map(|labels| {
                labels
                    .iter()
                    .filter_map(|label| label.as_str())
                    .collect::<Vec<_>>()
            }),
        Some(vec!["config_only", "test_infrastructure_touch"])
    );

    let generated_entry = files
        .iter()
        .find(|entry| {
            entry.get("path").and_then(|v| v.as_str())
                == Some("target/generated/schema.generated.json")
        })
        .expect("generated entry");
    assert_eq!(
        generated_entry
            .get("semantic")
            .and_then(|v| v.get("generated_like"))
            .and_then(|v| v.as_bool()),
        Some(true)
    );
    assert_eq!(
        generated_entry
            .get("semantic")
            .and_then(|v| v.get("language"))
            .and_then(|v| v.as_str()),
        Some("json")
    );
    assert_eq!(
        generated_entry
            .get("semantic")
            .and_then(|v| v.get("coarse_labels"))
            .and_then(|v| v.as_array())
            .map(|labels| {
                labels
                    .iter()
                    .filter_map(|label| label.as_str())
                    .collect::<Vec<_>>()
            }),
        Some(vec!["config_only", "generated_output_touch"])
    );

    let part_context: Value = serde_json::from_str(
        &fs::read_to_string(bundle.join("parts").join("part_01.context.json")).unwrap(),
    )
    .unwrap();
    let part_files = part_context
        .get("files")
        .and_then(|v| v.as_array())
        .expect("part files array");
    let part_src_entry = part_files
        .iter()
        .find(|entry| entry.get("path").and_then(|v| v.as_str()) == Some("src/lib.rs"))
        .expect("part src/lib.rs entry");
    assert_eq!(
        part_src_entry
            .get("semantic")
            .and_then(|v| v.get("language"))
            .and_then(|v| v.as_str()),
        Some("rust")
    );
    assert_eq!(
        part_src_entry
            .get("semantic")
            .and_then(|v| v.get("related_test_candidates"))
            .and_then(|v| v.get(0))
            .and_then(|v| v.as_str()),
        Some("tests/lib_test.rs")
    );
    assert_eq!(
        part_src_entry
            .get("semantic")
            .and_then(|v| v.get("coarse_labels"))
            .and_then(|v| v.as_array())
            .map(|labels| {
                labels
                    .iter()
                    .filter_map(|label| label.as_str())
                    .collect::<Vec<_>>()
            }),
        Some(vec![
            "api_surface_like",
            "import_churn",
            "signature_change_like",
        ])
    );
}

#[test]
fn build_part_context_includes_scoped_context_hints() {
    let td = init_repo();
    let root = td.path();

    fs::create_dir_all(root.join("src")).unwrap();
    fs::create_dir_all(root.join("tests")).unwrap();

    fs::write(
        root.join("src/lib.rs"),
        "pub fn value() -> i32 {\n    1\n}\n",
    )
    .unwrap();
    fs::write(root.join("tests/lib_test.rs"), "#[test]\nfn smoke() {}\n").unwrap();
    commit_all(root, "c1");

    fs::write(
        root.join("src/lib.rs"),
        "use crate::helper;\n\npub fn value_v2() -> i32 {\n    let current = helper();\n    current\n}\n",
    )
    .unwrap();
    commit_all(root, "c2");

    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("diffship");
    cmd.current_dir(root).arg("build");
    cmd.assert().success();

    let bundle = only_generated_bundle(root);
    let part_context: Value = serde_json::from_str(
        &fs::read_to_string(bundle.join("parts").join("part_01.context.json")).unwrap(),
    )
    .unwrap();

    assert_eq!(
        part_context
            .get("scoped_context")
            .and_then(|v| v.get("symbol_like_names"))
            .and_then(|v| v.as_array())
            .map(|items| items.iter().any(|item| item.as_str() == Some("value_v2"))),
        Some(true)
    );
    assert_eq!(
        part_context
            .get("scoped_context")
            .and_then(|v| v.get("import_like_refs"))
            .and_then(|v| v.get(0))
            .and_then(|v| v.as_str()),
        Some("use crate::helper")
    );
    assert_eq!(
        part_context
            .get("scoped_context")
            .and_then(|v| v.get("related_test_candidates"))
            .and_then(|v| v.get(0))
            .and_then(|v| v.as_str()),
        Some("tests/lib_test.rs")
    );
    assert!(
        part_context
            .get("scoped_context")
            .and_then(|v| v.get("hunk_headers"))
            .and_then(|v| v.as_array())
            .is_some()
    );
    assert_eq!(
        part_context
            .get("scoped_context")
            .and_then(|v| v.get("files"))
            .and_then(|v| v.get(0))
            .and_then(|v| v.get("path"))
            .and_then(|v| v.as_str()),
        Some("src/lib.rs")
    );
    assert_eq!(
        part_context
            .get("scoped_context")
            .and_then(|v| v.get("files"))
            .and_then(|v| v.get(0))
            .and_then(|v| v.get("symbol_like_names"))
            .and_then(|v| v.as_array())
            .map(|items| items.iter().any(|item| item.as_str() == Some("value_v2"))),
        Some(true)
    );
    assert_eq!(
        part_context
            .get("scoped_context")
            .and_then(|v| v.get("files"))
            .and_then(|v| v.get(0))
            .and_then(|v| v.get("import_like_refs"))
            .and_then(|v| v.get(0))
            .and_then(|v| v.as_str()),
        Some("use crate::helper")
    );
    assert_eq!(
        part_context.get("task_group_ref").and_then(|v| v.as_str()),
        Some("task_01")
    );
    assert_eq!(
        part_context
            .get("task_shape_labels")
            .and_then(|v| v.as_array())
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.as_str())
                    .collect::<Vec<_>>()
            }),
        Some(vec!["review_heavy", "single_area", "verification_heavy"])
    );
    assert_eq!(
        part_context
            .get("task_edit_targets")
            .and_then(|v| v.as_array())
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.as_str())
                    .collect::<Vec<_>>()
            }),
        Some(vec!["src/lib.rs"])
    );
    assert_eq!(
        part_context
            .get("task_context_only_files")
            .and_then(|v| v.as_array())
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.as_str())
                    .collect::<Vec<_>>()
            }),
        Some(Vec::<&str>::new())
    );
    assert_eq!(
        part_context
            .get("intent_labels")
            .and_then(|v| v.as_array())
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.as_str())
                    .collect::<Vec<_>>()
            }),
        Some(vec!["api_surface_touch", "import_churn", "source_update",])
    );
}

#[test]
fn build_structured_context_includes_related_source_candidates_for_tests() {
    let td = init_repo();
    let root = td.path();

    fs::create_dir_all(root.join("src/nested")).unwrap();
    fs::create_dir_all(root.join("tests/nested")).unwrap();

    fs::write(
        root.join("src/nested/foo.py"),
        "def value():\n    return 1\n",
    )
    .unwrap();
    fs::write(
        root.join("tests/nested/test_foo.py"),
        "def test_value():\n    assert value() == 1\n",
    )
    .unwrap();
    commit_all(root, "c1");

    fs::write(
        root.join("tests/nested/test_foo.py"),
        "def test_value():\n    assert value() == 2\n",
    )
    .unwrap();
    commit_all(root, "c2");

    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("diffship");
    cmd.current_dir(root).arg("build");
    cmd.assert().success();

    let bundle = only_generated_bundle(root);
    let manifest: Value =
        serde_json::from_str(&fs::read_to_string(bundle.join("handoff.manifest.json")).unwrap())
            .unwrap();
    let files = manifest
        .get("files")
        .and_then(|v| v.as_array())
        .expect("manifest files array");
    let test_entry = files
        .iter()
        .find(|entry| {
            entry.get("path").and_then(|v| v.as_str()) == Some("tests/nested/test_foo.py")
        })
        .expect("test file entry");

    assert_eq!(
        test_entry
            .get("semantic")
            .and_then(|v| v.get("related_source_candidates"))
            .and_then(|v| v.get(0))
            .and_then(|v| v.as_str()),
        Some("src/nested/foo.py")
    );
}

#[test]
fn build_structured_context_includes_related_doc_and_config_candidates() {
    let td = init_repo();
    let root = td.path();

    fs::create_dir_all(root.join("src")).unwrap();
    fs::create_dir_all(root.join("docs")).unwrap();

    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(root.join("README.md"), "# Demo\n").unwrap();
    fs::write(root.join("docs/lib.md"), "# Lib\n").unwrap();
    fs::write(
        root.join("src/lib.rs"),
        "use crate::old_dep;\npub fn value() -> i32 { 1 }\n",
    )
    .unwrap();
    commit_all(root, "c1");

    fs::write(
        root.join("src/lib.rs"),
        "use crate::new_dep;\npub fn value(input: i32) -> i32 { input + 2 }\n",
    )
    .unwrap();
    commit_all(root, "c2");

    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("diffship");
    cmd.current_dir(root).arg("build");
    cmd.assert().success();

    let bundle = only_generated_bundle(root);
    let manifest: Value =
        serde_json::from_str(&fs::read_to_string(bundle.join("handoff.manifest.json")).unwrap())
            .unwrap();
    let files = manifest
        .get("files")
        .and_then(|v| v.as_array())
        .expect("manifest files array");
    let src_entry = files
        .iter()
        .find(|entry| entry.get("path").and_then(|v| v.as_str()) == Some("src/lib.rs"))
        .expect("src/lib.rs entry");

    assert_eq!(
        src_entry
            .get("semantic")
            .and_then(|v| v.get("related_doc_candidates"))
            .and_then(|v| v.get(0))
            .and_then(|v| v.as_str()),
        Some("README.md")
    );
    assert_eq!(
        src_entry
            .get("semantic")
            .and_then(|v| v.get("related_doc_candidates"))
            .and_then(|v| v.get(1))
            .and_then(|v| v.as_str()),
        Some("docs/lib.md")
    );
    assert_eq!(
        src_entry
            .get("semantic")
            .and_then(|v| v.get("related_config_candidates"))
            .and_then(|v| v.get(0))
            .and_then(|v| v.as_str()),
        Some("Cargo.toml")
    );
}

#[test]
fn build_structured_context_includes_change_hints() {
    let td = init_repo();
    let root = td.path();

    fs::write(root.join("tracked.txt"), "base\n").unwrap();
    commit_all(root, "c1");

    fs::write(root.join("tracked.txt"), "next\n").unwrap();
    commit_all(root, "c2");
    fs::write(root.join("bin.dat"), [0_u8, 159, 146, 150]).unwrap();

    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("diffship");
    cmd.current_dir(root)
        .args(["build", "--include-untracked", "--include-binary"]);
    cmd.assert().success();

    let bundle = only_generated_bundle(root);
    let manifest: Value =
        serde_json::from_str(&fs::read_to_string(bundle.join("handoff.manifest.json")).unwrap())
            .unwrap();
    let files = manifest
        .get("files")
        .and_then(|v| v.as_array())
        .expect("manifest files array");

    let tracked_entry = files
        .iter()
        .find(|entry| entry.get("path").and_then(|v| v.as_str()) == Some("tracked.txt"))
        .expect("tracked.txt entry");
    assert_eq!(
        tracked_entry
            .get("change_hints")
            .and_then(|v| v.get("new_file"))
            .and_then(|v| v.as_bool()),
        Some(false)
    );
    assert_eq!(
        tracked_entry
            .get("change_hints")
            .and_then(|v| v.get("reduced_context"))
            .and_then(|v| v.as_bool()),
        Some(false)
    );

    let attachment_entry = files
        .iter()
        .find(|entry| entry.get("path").and_then(|v| v.as_str()) == Some("bin.dat"))
        .expect("bin.dat entry");
    assert_eq!(
        attachment_entry
            .get("change_hints")
            .and_then(|v| v.get("stored_as_attachment"))
            .and_then(|v| v.as_bool()),
        Some(true)
    );

    let part_context: Value = serde_json::from_str(
        &fs::read_to_string(bundle.join("parts").join("part_01.context.json")).unwrap(),
    )
    .unwrap();
    let part_files = part_context
        .get("files")
        .and_then(|v| v.as_array())
        .expect("part files array");
    let part_tracked_entry = part_files
        .iter()
        .find(|entry| entry.get("path").and_then(|v| v.as_str()) == Some("tracked.txt"))
        .expect("part tracked.txt entry");
    assert_eq!(
        part_tracked_entry
            .get("change_hints")
            .and_then(|v| v.get("new_file"))
            .and_then(|v| v.as_bool()),
        Some(false)
    );
}

#[test]
fn build_project_context_focused_emits_context_pack() {
    let td = init_repo();
    let root = td.path();

    fs::create_dir_all(root.join("src")).unwrap();
    fs::create_dir_all(root.join("tests")).unwrap();
    fs::create_dir_all(root.join("docs")).unwrap();
    fs::create_dir_all(root.join(".diffship")).unwrap();

    fs::write(root.join("Cargo.toml"), "[package]\nname = \"demo\"\n").unwrap();
    fs::write(root.join("README.md"), "# Demo\n").unwrap();
    fs::write(root.join("src/lib.rs"), "pub fn value() -> i32 { 1 }\n").unwrap();
    fs::write(root.join("tests/lib_test.rs"), "#[test]\nfn smoke() {}\n").unwrap();
    fs::write(root.join("docs/lib.md"), "# Lib\n").unwrap();
    fs::write(
        root.join(".diffship/PROJECT_RULES.md"),
        "Keep changes scoped.\n",
    )
    .unwrap();
    commit_all(root, "c1");

    fs::write(root.join("src/lib.rs"), "pub fn value() -> i32 { 2 }\n").unwrap();
    commit_all(root, "c2");

    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("diffship");
    cmd.current_dir(root)
        .args(["build", "--project-context", "focused"]);
    cmd.assert().success();

    let bundle = only_generated_bundle(root);
    assert!(bundle.join("project.context.json").exists());
    assert!(bundle.join("PROJECT_CONTEXT.md").exists());
    assert!(bundle.join("project_context/files/src/lib.rs").exists());
    assert!(
        bundle
            .join("project_context/files/tests/lib_test.rs")
            .exists()
    );
    assert!(bundle.join("project_context/files/docs/lib.md").exists());
    assert!(bundle.join("project_context/files/Cargo.toml").exists());
    assert!(bundle.join("project_context/files/README.md").exists());
    assert!(
        bundle
            .join("project_context/files/.diffship/PROJECT_RULES.md")
            .exists()
    );

    let handoff = fs::read_to_string(bundle.join("HANDOFF.md")).unwrap();
    assert!(handoff.contains("Read `PROJECT_CONTEXT.md` before widening scope"));
    assert!(handoff.contains("Project context: `PROJECT_CONTEXT.md`"));

    let ai_requests = fs::read_to_string(bundle.join("AI_REQUESTS.md")).unwrap();
    assert!(ai_requests.contains("## Focused project-context guidance"));
    assert!(
        ai_requests
            .contains("selected files: `6` (`1` changed, `5` supplemental; `10` relationship(s))")
    );
    assert!(ai_requests.contains("Use `project.context.json` when you need file-by-file `changed`, `usage_role`, `priority`, `edit_scope_role`, `verification_relevance`, `verification_labels`, `why_included`, `task_group_refs`, `context_labels`, `semantic`, `outbound_relationships`, and `inbound_relationships` data."));
    assert!(ai_requests.contains("changed context: `src/lib.rs` [source] role=`target` priority=`primary` edit=`write_target` verify=`primary:api_surface,changed_target,relationship_backed` tasks=`task_01` context=`changed_target,relationship_source,relationship_target,source_context`"));
    assert!(ai_requests.contains("direct=`related-config:Cargo.toml"));
    assert!(ai_requests.contains("related-doc:README.md"));
    assert!(ai_requests.contains("related-doc:docs/lib.md"));
    assert!(ai_requests.contains("related-test:tests/lib_test.rs"));
    assert!(ai_requests.contains("## Task-group execution order"));
    assert!(ai_requests.contains(
        "Use `task_shape_labels` to decide whether a task is single-area or cross-cutting"
    ));
    assert!(ai_requests.contains(
        "Use `edit_targets` as the bounded write scope for the task and `context_only_files` as read-only context"
    ));
    assert!(ai_requests.contains("Use `verification_targets` as the bounded set of likely tests/config/policy surfaces to inspect before proposing local verification."));
    assert!(ai_requests.contains("Use `widening_labels` to decide whether to stay patch-first or widen into related tests/config/docs/repo rules."));
    assert!(
        ai_requests
            .contains("Use `execution_labels` to keep the execution flow coarse and deterministic")
    );
    assert!(ai_requests.contains("`task_01` primary=`api_surface_task,source_task` shape=`cross_cutting,review_heavy,verification_heavy` review=`behavioral_change_like,needs_related_test_review,verification_surface_touch` intents=`api_surface_touch,source_update` risks=`-` edit=`src/lib.rs` context=`Cargo.toml(config_reference/secondary), README.md(repo_rule/secondary), docs/lib.md(doc_reference/secondary), tests/lib_test.rs(test_reference/secondary)` verify=`Cargo.toml(supporting/config_reference), src/lib.rs(primary/target), tests/lib_test.rs(primary/test_reference)` verify-strategy=`behavioral_regression_watch,config_follow_up,needs_targeted_test_read,policy_follow_up,test_follow_up` widen=`read_related_config,read_related_docs,read_related_tests,read_repo_rules` execute=`behavior_first,check_config_after_edit,check_tests_after_edit,rules_before_edit,verify_after_edit,widen_before_edit`"));
    assert!(ai_requests.contains("project=`Cargo.toml(config_reference/secondary), README.md(repo_rule/secondary), docs/lib.md(doc_reference/secondary), src/lib.rs(target/primary)`"));
    assert!(ai_requests.contains("## Patch-part guidance"));
    assert!(ai_requests.contains("`parts/part_01.patch` context=`parts/part_01.context.json` review=`behavioral_change_like,needs_related_test_review,verification_surface_touch` intents=`api_surface_touch,source_update` segments=`committed` files=`src/lib.rs`"));

    let manifest: Value =
        serde_json::from_str(&fs::read_to_string(bundle.join("handoff.manifest.json")).unwrap())
            .unwrap();
    assert_eq!(
        manifest
            .get("task_groups")
            .and_then(|v| v.get(0))
            .and_then(|v| v.get("review_labels"))
            .and_then(|v| v.as_array())
            .map(|items| items
                .iter()
                .filter_map(|item| item.as_str())
                .collect::<Vec<_>>()),
        Some(vec![
            "behavioral_change_like",
            "needs_related_test_review",
            "verification_surface_touch",
        ])
    );
    assert_eq!(
        manifest
            .get("task_groups")
            .and_then(|v| v.get(0))
            .and_then(|v| v.get("task_shape_labels"))
            .and_then(|v| v.as_array())
            .map(|items| items
                .iter()
                .filter_map(|item| item.as_str())
                .collect::<Vec<_>>()),
        Some(vec!["cross_cutting", "review_heavy", "verification_heavy",])
    );
    assert_eq!(
        manifest
            .get("task_groups")
            .and_then(|v| v.get(0))
            .and_then(|v| v.get("edit_targets"))
            .and_then(|v| v.as_array())
            .map(|items| items
                .iter()
                .filter_map(|item| item.as_str())
                .collect::<Vec<_>>()),
        Some(vec!["src/lib.rs"])
    );
    assert_eq!(
        manifest
            .get("task_groups")
            .and_then(|v| v.get(0))
            .and_then(|v| v.get("context_only_files"))
            .and_then(|v| v.as_array())
            .map(|items| items
                .iter()
                .filter_map(|item| item.as_str())
                .collect::<Vec<_>>()),
        Some(vec![
            "Cargo.toml",
            "README.md",
            "docs/lib.md",
            "tests/lib_test.rs",
        ])
    );
    assert_eq!(
        manifest
            .get("task_groups")
            .and_then(|v| v.get(0))
            .and_then(|v| v.get("verification_targets"))
            .and_then(|v| v.as_array())
            .map(|items| items
                .iter()
                .filter_map(|item| item.as_str())
                .collect::<Vec<_>>()),
        Some(vec!["Cargo.toml", "src/lib.rs", "tests/lib_test.rs"])
    );
    assert_eq!(
        manifest
            .get("task_groups")
            .and_then(|v| v.get(0))
            .and_then(|v| v.get("verification_labels"))
            .and_then(|v| v.as_array())
            .map(|items| items
                .iter()
                .filter_map(|item| item.as_str())
                .collect::<Vec<_>>()),
        Some(vec![
            "behavioral_regression_watch",
            "config_follow_up",
            "needs_targeted_test_read",
            "policy_follow_up",
            "test_follow_up",
        ])
    );
    assert_eq!(
        manifest
            .get("task_groups")
            .and_then(|v| v.get(0))
            .and_then(|v| v.get("widening_labels"))
            .and_then(|v| v.as_array())
            .map(|items| items
                .iter()
                .filter_map(|item| item.as_str())
                .collect::<Vec<_>>()),
        Some(vec![
            "read_related_config",
            "read_related_docs",
            "read_related_tests",
            "read_repo_rules",
        ])
    );
    assert_eq!(
        manifest
            .get("task_groups")
            .and_then(|v| v.get(0))
            .and_then(|v| v.get("execution_labels"))
            .and_then(|v| v.as_array())
            .map(|items| items
                .iter()
                .filter_map(|item| item.as_str())
                .collect::<Vec<_>>()),
        Some(vec![
            "behavior_first",
            "check_config_after_edit",
            "check_tests_after_edit",
            "rules_before_edit",
            "verify_after_edit",
            "widen_before_edit",
        ])
    );
    assert_eq!(
        manifest
            .get("artifacts")
            .and_then(|v| v.get("project_context_json"))
            .and_then(|v| v.as_str()),
        Some("project.context.json")
    );
    assert_eq!(
        manifest
            .get("artifacts")
            .and_then(|v| v.get("project_context_md"))
            .and_then(|v| v.as_str()),
        Some("PROJECT_CONTEXT.md")
    );

    let project_context: Value =
        serde_json::from_str(&fs::read_to_string(bundle.join("project.context.json")).unwrap())
            .unwrap();
    let part_context: Value = serde_json::from_str(
        &fs::read_to_string(bundle.join("parts").join("part_01.context.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        part_context.get("task_group_ref").and_then(|v| v.as_str()),
        Some("task_01")
    );
    assert_eq!(
        part_context
            .get("task_shape_labels")
            .and_then(|v| v.as_array())
            .map(|items| items
                .iter()
                .filter_map(|item| item.as_str())
                .collect::<Vec<_>>()),
        Some(vec!["cross_cutting", "review_heavy", "verification_heavy",])
    );
    assert_eq!(
        part_context
            .get("task_edit_targets")
            .and_then(|v| v.as_array())
            .map(|items| items
                .iter()
                .filter_map(|item| item.as_str())
                .collect::<Vec<_>>()),
        Some(vec!["src/lib.rs"])
    );
    assert_eq!(
        part_context
            .get("task_context_only_files")
            .and_then(|v| v.as_array())
            .map(|items| items
                .iter()
                .filter_map(|item| item.as_str())
                .collect::<Vec<_>>()),
        Some(vec![
            "Cargo.toml",
            "README.md",
            "docs/lib.md",
            "tests/lib_test.rs",
        ])
    );
    assert_eq!(
        part_context
            .get("review_labels")
            .and_then(|v| v.as_array())
            .map(|items| items
                .iter()
                .filter_map(|item| item.as_str())
                .collect::<Vec<_>>()),
        Some(vec![
            "behavioral_change_like",
            "needs_related_test_review",
            "verification_surface_touch",
        ])
    );
    assert_eq!(
        project_context
            .get("summary")
            .and_then(|v| v.get("selected_files"))
            .and_then(|v| v.as_u64()),
        Some(6)
    );
    assert_eq!(
        project_context
            .get("summary")
            .and_then(|v| v.get("changed_files"))
            .and_then(|v| v.as_u64()),
        Some(1)
    );
    assert_eq!(
        project_context
            .get("summary")
            .and_then(|v| v.get("supplemental_files"))
            .and_then(|v| v.as_u64()),
        Some(5)
    );
    assert_eq!(
        project_context
            .get("summary")
            .and_then(|v| v.get("included_snapshots"))
            .and_then(|v| v.as_u64()),
        Some(6)
    );
    assert_eq!(
        project_context
            .get("summary")
            .and_then(|v| v.get("relationship_count"))
            .and_then(|v| v.as_u64()),
        Some(10)
    );
    assert_eq!(
        project_context
            .get("summary")
            .and_then(|v| v.get("categories"))
            .and_then(|v| v.get("source"))
            .and_then(|v| v.as_u64()),
        Some(1)
    );
    assert_eq!(
        project_context
            .get("summary")
            .and_then(|v| v.get("categories"))
            .and_then(|v| v.get("docs"))
            .and_then(|v| v.as_u64()),
        Some(3)
    );
    assert_eq!(
        project_context
            .get("summary")
            .and_then(|v| v.get("priority_counts"))
            .and_then(|v| v.get("primary"))
            .and_then(|v| v.as_u64()),
        Some(1)
    );
    assert_eq!(
        project_context
            .get("summary")
            .and_then(|v| v.get("priority_counts"))
            .and_then(|v| v.get("secondary"))
            .and_then(|v| v.as_u64()),
        Some(5)
    );
    assert_eq!(
        project_context
            .get("summary")
            .and_then(|v| v.get("edit_scope_counts"))
            .and_then(|v| v.get("write_target"))
            .and_then(|v| v.as_u64()),
        Some(1)
    );
    assert_eq!(
        project_context
            .get("summary")
            .and_then(|v| v.get("edit_scope_counts"))
            .and_then(|v| v.get("read_only_verification"))
            .and_then(|v| v.as_u64()),
        Some(2)
    );
    assert_eq!(
        project_context
            .get("summary")
            .and_then(|v| v.get("edit_scope_counts"))
            .and_then(|v| v.get("read_only_rule"))
            .and_then(|v| v.as_u64()),
        Some(1)
    );
    assert_eq!(
        project_context
            .get("summary")
            .and_then(|v| v.get("edit_scope_counts"))
            .and_then(|v| v.get("read_only_context"))
            .and_then(|v| v.as_u64()),
        Some(2)
    );
    assert_eq!(
        project_context
            .get("summary")
            .and_then(|v| v.get("verification_relevance_counts"))
            .and_then(|v| v.get("primary"))
            .and_then(|v| v.as_u64()),
        Some(2)
    );
    assert_eq!(
        project_context
            .get("summary")
            .and_then(|v| v.get("verification_relevance_counts"))
            .and_then(|v| v.get("supporting"))
            .and_then(|v| v.as_u64()),
        Some(2)
    );
    assert_eq!(
        project_context
            .get("summary")
            .and_then(|v| v.get("relationship_kinds"))
            .and_then(|v| v.get("related-doc"))
            .and_then(|v| v.as_u64()),
        Some(6)
    );

    let src_entry = project_context
        .get("files")
        .and_then(|v| v.as_array())
        .and_then(|items| {
            items
                .iter()
                .find(|entry| entry.get("path").and_then(|v| v.as_str()) == Some("src/lib.rs"))
        })
        .expect("src/lib.rs project context entry");
    assert_eq!(
        src_entry.get("changed").and_then(|v| v.as_bool()),
        Some(true)
    );
    assert_eq!(
        src_entry.get("usage_role").and_then(|v| v.as_str()),
        Some("target")
    );
    assert_eq!(
        src_entry.get("priority").and_then(|v| v.as_str()),
        Some("primary")
    );
    assert_eq!(
        src_entry.get("edit_scope_role").and_then(|v| v.as_str()),
        Some("write_target")
    );
    assert_eq!(
        src_entry
            .get("verification_relevance")
            .and_then(|v| v.as_str()),
        Some("primary")
    );
    assert_eq!(
        src_entry
            .get("verification_labels")
            .and_then(|v| v.as_array())
            .map(|items| items
                .iter()
                .filter_map(|item| item.as_str())
                .collect::<Vec<_>>()),
        Some(vec!["api_surface", "changed_target", "relationship_backed",])
    );
    assert_eq!(
        src_entry
            .get("why_included")
            .and_then(|v| v.as_array())
            .map(|items| items
                .iter()
                .filter_map(|item| item.as_str())
                .collect::<Vec<_>>()),
        Some(vec!["changed_file"])
    );
    assert_eq!(
        src_entry
            .get("task_group_refs")
            .and_then(|v| v.as_array())
            .and_then(|items| items.first())
            .and_then(|v| v.as_str()),
        Some("task_01")
    );
    assert_eq!(
        src_entry
            .get("semantic")
            .and_then(|v| v.get("language"))
            .and_then(|v| v.as_str()),
        Some("rust")
    );
    let src_labels = src_entry
        .get("semantic")
        .and_then(|v| v.get("coarse_labels"))
        .and_then(|v| v.as_array())
        .map(|labels| {
            labels
                .iter()
                .filter_map(|label| label.as_str())
                .collect::<Vec<_>>()
        })
        .expect("src/lib.rs coarse labels");
    assert!(src_labels.contains(&"api_surface_like"));
    assert!(src_labels.contains(&"signature_change_like"));
    assert_eq!(
        src_entry
            .get("context_labels")
            .and_then(|v| v.as_array())
            .map(|labels| {
                labels
                    .iter()
                    .filter_map(|label| label.as_str())
                    .collect::<Vec<_>>()
            }),
        Some(vec![
            "changed_target",
            "relationship_source",
            "relationship_target",
            "source_context",
        ])
    );
    assert!(
        src_entry
            .get("outbound_relationships")
            .and_then(|v| v.as_array())
            .is_some_and(|items| items.iter().any(|entry| {
                entry.get("kind").and_then(|v| v.as_str()) == Some("related-test")
                    && entry.get("path").and_then(|v| v.as_str()) == Some("tests/lib_test.rs")
            }))
    );
    assert!(
        src_entry
            .get("inbound_relationships")
            .and_then(|v| v.as_array())
            .is_some_and(|items| items.iter().any(|entry| {
                entry.get("kind").and_then(|v| v.as_str()) == Some("related-source")
                    && entry.get("path").and_then(|v| v.as_str()) == Some("tests/lib_test.rs")
            }))
    );

    let test_entry = project_context
        .get("files")
        .and_then(|v| v.as_array())
        .and_then(|items| {
            items.iter().find(|entry| {
                entry.get("path").and_then(|v| v.as_str()) == Some("tests/lib_test.rs")
            })
        })
        .expect("tests/lib_test.rs project context entry");
    assert_eq!(
        test_entry
            .get("source_reasons")
            .and_then(|v| v.as_array())
            .and_then(|items| items.first())
            .and_then(|v| v.as_str()),
        Some("related-test:src/lib.rs")
    );
    assert_eq!(
        test_entry.get("changed").and_then(|v| v.as_bool()),
        Some(false)
    );
    assert_eq!(
        test_entry.get("usage_role").and_then(|v| v.as_str()),
        Some("test_reference")
    );
    assert_eq!(
        test_entry.get("priority").and_then(|v| v.as_str()),
        Some("secondary")
    );
    assert_eq!(
        test_entry.get("edit_scope_role").and_then(|v| v.as_str()),
        Some("read_only_verification")
    );
    assert_eq!(
        test_entry
            .get("verification_relevance")
            .and_then(|v| v.as_str()),
        Some("primary")
    );
    assert_eq!(
        test_entry
            .get("semantic")
            .and_then(|v| v.get("related_source_candidates"))
            .and_then(|v| v.get(0))
            .and_then(|v| v.as_str()),
        Some("src/lib.rs")
    );
    assert_eq!(
        test_entry
            .get("context_labels")
            .and_then(|v| v.as_array())
            .map(|labels| {
                labels
                    .iter()
                    .filter_map(|label| label.as_str())
                    .collect::<Vec<_>>()
            }),
        Some(vec![
            "related_context",
            "relationship_source",
            "relationship_target",
            "supplemental_context",
            "test_context",
        ])
    );
    assert_eq!(
        test_entry
            .get("why_included")
            .and_then(|v| v.as_array())
            .map(|items| items
                .iter()
                .filter_map(|item| item.as_str())
                .collect::<Vec<_>>()),
        Some(vec!["related_test"])
    );
    assert_eq!(
        test_entry
            .get("task_group_refs")
            .and_then(|v| v.as_array())
            .and_then(|items| items.first())
            .and_then(|v| v.as_str()),
        Some("task_01")
    );
    let rules_entry = project_context
        .get("files")
        .and_then(|v| v.as_array())
        .and_then(|items| {
            items.iter().find(|entry| {
                entry.get("path").and_then(|v| v.as_str()) == Some(".diffship/PROJECT_RULES.md")
            })
        })
        .expect(".diffship/PROJECT_RULES.md project context entry");
    assert_eq!(
        rules_entry.get("edit_scope_role").and_then(|v| v.as_str()),
        Some("read_only_context")
    );
    let project_context_md = fs::read_to_string(bundle.join("PROJECT_CONTEXT.md")).unwrap();
    assert!(project_context_md.contains("## Edit-scope counts"));
    assert!(project_context_md.contains(
        "- `src/lib.rs` [source] changed=`yes` role=`target` priority=`primary` edit=`write_target`"
    ));
    assert!(
        project_context
            .get("relationships")
            .and_then(|v| v.as_array())
            .is_some_and(|items| items.iter().any(|entry| {
                entry.get("from").and_then(|v| v.as_str()) == Some("src/lib.rs")
                    && entry.get("kind").and_then(|v| v.as_str()) == Some("related-config")
                    && entry.get("to").and_then(|v| v.as_str()) == Some("Cargo.toml")
            }))
    );

    let project_context_md = fs::read_to_string(bundle.join("PROJECT_CONTEXT.md")).unwrap();
    assert!(project_context_md.contains("selected files: `6` (`1` changed, `5` supplemental"));
    assert!(project_context_md.contains("relationship(s)"));
    assert!(project_context_md.contains("## Category counts"));
    assert!(project_context_md.contains("## Verification relevance counts"));
    assert!(project_context_md.contains("`related-doc`: 6 relationship(s)"));
    assert!(project_context_md.contains("`src/lib.rs` [source] changed=`yes`"));
    assert!(project_context_md.contains("verify=`primary`"));
    assert!(
        project_context_md.contains("verify-why=`api_surface,changed_target,relationship_backed`")
    );
    assert!(project_context_md.contains(
        "context=`changed_target,relationship_source,relationship_target,source_context`"
    ));
    assert!(project_context_md.contains("labels=`"));
    assert!(project_context_md.contains("api_surface_like"));
    assert!(project_context_md.contains("signature_change_like"));
}

#[test]
fn build_out_dir_places_generated_bundle_under_requested_parent() {
    let td = init_repo();
    let root = td.path();

    fs::write(root.join("a.txt"), "one\n").unwrap();
    commit_all(root, "c1");
    fs::write(root.join("a.txt"), "two\n").unwrap();
    commit_all(root, "c2");

    let out_dir = root.join("artifacts").join("handoff");

    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("diffship");
    cmd.current_dir(root)
        .args(["build", "--out-dir"])
        .arg(&out_dir);
    cmd.assert().success();

    let bundles = fs::read_dir(&out_dir)
        .unwrap()
        .filter_map(|ent| {
            let ent = ent.ok()?;
            if !ent.file_type().ok()?.is_dir() {
                return None;
            }
            let name = ent.file_name().to_string_lossy().to_string();
            if !name.starts_with("diffship_") {
                return None;
            }
            Some(ent.path())
        })
        .collect::<Vec<_>>();

    assert_eq!(bundles.len(), 1);
    assert!(bundles[0].join("HANDOFF.md").exists());
}

#[test]
fn build_zip_only_creates_only_zip_by_default() {
    let td = init_repo();
    let root = td.path();

    fs::write(root.join("a.txt"), "one\n").unwrap();
    commit_all(root, "c1");
    fs::write(root.join("a.txt"), "two\n").unwrap();
    commit_all(root, "c2");

    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("diffship");
    cmd.current_dir(root).args(["build", "--zip-only"]);
    cmd.assert().success();

    let mut zip_paths = vec![];
    let mut dir_paths = vec![];
    for ent in fs::read_dir(root).unwrap() {
        let ent = ent.unwrap();
        let name = ent.file_name().to_string_lossy().to_string();
        if !name.starts_with("diffship_") {
            continue;
        }
        if ent.file_type().unwrap().is_file() && name.ends_with(".zip") {
            zip_paths.push(ent.path());
        } else if ent.file_type().unwrap().is_dir() {
            dir_paths.push(ent.path());
        }
    }

    assert_eq!(zip_paths.len(), 1);
    assert!(dir_paths.is_empty());

    let file = fs::File::open(&zip_paths[0]).unwrap();
    let mut zip = ZipArchive::new(file).unwrap();
    assert!(zip.by_name("HANDOFF.md").is_ok());
    assert!(zip.by_name("handoff.manifest.json").is_ok());
    assert!(zip.by_name("handoff.context.xml").is_ok());
    assert!(zip.by_name("parts/part_01.patch").is_ok());
    assert!(zip.by_name("parts/part_01.context.json").is_ok());
}

#[test]
fn build_zip_only_accepts_explicit_zip_out_path() {
    let td = init_repo();
    let root = td.path();

    fs::write(root.join("a.txt"), "one\n").unwrap();
    commit_all(root, "c1");
    fs::write(root.join("a.txt"), "two\n").unwrap();
    commit_all(root, "c2");

    let out = root.join("bundle-output.zip");
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("diffship");
    cmd.current_dir(root)
        .args(["build", "--zip-only", "--out"])
        .arg(&out);
    cmd.assert().success();

    assert!(out.exists());
    assert!(!root.join("bundle-output").exists());
}

#[test]
fn build_rejects_out_and_out_dir_together() {
    let td = init_repo();
    let root = td.path();

    fs::write(root.join("a.txt"), "one\n").unwrap();
    commit_all(root, "c1");
    fs::write(root.join("a.txt"), "two\n").unwrap();
    commit_all(root, "c2");

    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("diffship");
    cmd.current_dir(root)
        .args(["build", "--out-dir", "artifacts", "--out", "bundle"]);
    cmd.assert().failure().stderr(predicates::str::contains(
        "--out and --out-dir cannot be used together",
    ));
}

#[test]
fn build_project_config_can_set_default_out_dir() {
    let td = init_repo();
    let root = td.path();

    fs::write(root.join("a.txt"), "one\n").unwrap();
    commit_all(root, "c1");
    fs::write(root.join("a.txt"), "two\n").unwrap();
    commit_all(root, "c2");

    write_project_config(
        root,
        r#"
[handoff]
output_dir = "artifacts/from-config"
"#,
    );

    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("diffship");
    cmd.current_dir(root).arg("build");
    cmd.assert().success();

    let out_dir = root.join("artifacts").join("from-config");
    let bundles = fs::read_dir(&out_dir)
        .unwrap()
        .filter_map(|ent| {
            let ent = ent.ok()?;
            if !ent.file_type().ok()?.is_dir() {
                return None;
            }
            let name = ent.file_name().to_string_lossy().to_string();
            if !name.starts_with("diffship_") {
                return None;
            }
            Some(ent.path())
        })
        .collect::<Vec<_>>();

    assert_eq!(bundles.len(), 1);
    assert!(bundles[0].join("HANDOFF.md").exists());
}

#[test]
fn build_ai_generated_config_can_set_default_out_dir() {
    let td = init_repo();
    let root = td.path();

    fs::write(root.join("a.txt"), "one\n").unwrap();
    commit_all(root, "c1");
    fs::write(root.join("a.txt"), "two\n").unwrap();
    commit_all(root, "c2");

    write_project_ai_generated_config(
        root,
        r#"
[handoff]
output_dir = "artifacts/from-ai-generated"
"#,
    );

    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("diffship");
    cmd.current_dir(root).arg("build");
    cmd.assert().success();

    let out_dir = root.join("artifacts").join("from-ai-generated");
    let bundles = fs::read_dir(&out_dir)
        .unwrap()
        .filter_map(|ent| {
            let ent = ent.ok()?;
            if !ent.file_type().ok()?.is_dir() {
                return None;
            }
            let name = ent.file_name().to_string_lossy().to_string();
            if !name.starts_with("diffship_") {
                return None;
            }
            Some(ent.path())
        })
        .collect::<Vec<_>>();

    assert_eq!(bundles.len(), 1);
    assert!(bundles[0].join("HANDOFF.md").exists());
}

#[test]
fn build_cli_out_dir_overrides_config_default_out_dir() {
    let td = init_repo();
    let root = td.path();

    fs::write(root.join("a.txt"), "one\n").unwrap();
    commit_all(root, "c1");
    fs::write(root.join("a.txt"), "two\n").unwrap();
    commit_all(root, "c2");

    write_project_config(
        root,
        r#"
[handoff]
output_dir = "artifacts/from-config"
"#,
    );

    let cli_dir = root.join("artifacts").join("from-cli");
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("diffship");
    cmd.current_dir(root)
        .args(["build", "--out-dir"])
        .arg(&cli_dir);
    cmd.assert().success();

    let cli_bundles = fs::read_dir(&cli_dir)
        .unwrap()
        .filter_map(|ent| {
            let ent = ent.ok()?;
            if ent.file_type().ok()?.is_dir() {
                Some(ent.path())
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(cli_bundles.len(), 1);

    let config_dir = root.join("artifacts").join("from-config");
    assert!(
        !config_dir.exists() || fs::read_dir(&config_dir).unwrap().next().is_none(),
        "config out dir should not be used when CLI --out-dir is set"
    );
}

#[test]
fn build_project_config_out_dir_supports_tilde_home() {
    let td = init_repo();
    let root = td.path();
    let home = td.path().join("fake-home");
    fs::create_dir_all(&home).unwrap();

    fs::write(root.join("a.txt"), "one\n").unwrap();
    commit_all(root, "c1");
    fs::write(root.join("a.txt"), "two\n").unwrap();
    commit_all(root, "c2");

    write_project_config(
        root,
        r#"
[handoff]
output_dir = "~/handoffs/from-config"
"#,
    );

    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("diffship");
    cmd.env("HOME", &home).current_dir(root).arg("build");
    cmd.assert().success();

    let out_dir = home.join("handoffs").join("from-config");
    let bundles = fs::read_dir(&out_dir)
        .unwrap()
        .filter_map(|ent| {
            let ent = ent.ok()?;
            if !ent.file_type().ok()?.is_dir() {
                return None;
            }
            let name = ent.file_name().to_string_lossy().to_string();
            if !name.starts_with("diffship_") {
                return None;
            }
            Some(ent.path())
        })
        .collect::<Vec<_>>();

    assert_eq!(bundles.len(), 1);
    assert!(bundles[0].join("HANDOFF.md").exists());
}

#[test]
fn build_can_export_and_replay_plan_toml() {
    let td = init_repo();
    let root = td.path();

    fs::write(root.join("tracked.txt"), "base\n").unwrap();
    commit_all(root, "base");
    fs::write(root.join("tracked.txt"), "next\n").unwrap();
    commit_all(root, "next");
    fs::write(root.join("note.txt"), "hello\n").unwrap();

    let out_a = root.join("bundle_plan_a");
    let plan_a = out_a.join("plan.toml");
    let mut build_a = assert_cmd::cargo::cargo_bin_cmd!("diffship");
    build_a
        .current_dir(root)
        .args([
            "build",
            "--include-untracked",
            "--include",
            "*.txt",
            "--yes",
            "--plan-out",
        ])
        .arg(&plan_a)
        .args(["--out"])
        .arg(&out_a)
        .assert()
        .success();
    assert!(plan_a.exists());
    let plan_a_text = fs::read_to_string(&plan_a).unwrap();
    assert!(plan_a_text.contains("profile = \"20x512\""));
    assert!(plan_a_text.contains("max_parts = 20"));
    assert!(plan_a_text.contains("max_bytes_per_part = 536870912"));

    let out_b = root.join("bundle_plan_b");
    let plan_b = out_b.join("plan.toml");
    let mut build_b = assert_cmd::cargo::cargo_bin_cmd!("diffship");
    build_b
        .current_dir(root)
        .args(["build", "--plan"])
        .arg(&plan_a)
        .args(["--yes", "--plan-out"])
        .arg(&plan_b)
        .args(["--out"])
        .arg(&out_b)
        .assert()
        .success();
    assert!(plan_b.exists());
    let plan_b_text = fs::read_to_string(&plan_b).unwrap();
    assert!(plan_b_text.contains("profile = \"20x512\""));
    assert!(plan_b_text.contains("max_parts = 20"));
    assert!(plan_b_text.contains("max_bytes_per_part = 536870912"));

    let mut cmp = assert_cmd::cargo::cargo_bin_cmd!("diffship");
    cmp.current_dir(root)
        .args(["compare"])
        .arg(&out_a)
        .arg(&out_b)
        .assert()
        .success();
}

#[test]
fn build_profile_flag_records_named_profile_and_limits() {
    let td = init_repo();
    let root = td.path();

    fs::write(root.join("tracked.txt"), "base\n").unwrap();
    commit_all(root, "base");
    fs::write(root.join("tracked.txt"), "next\n").unwrap();
    commit_all(root, "next");

    let out = root.join("bundle_profile_flag");
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("diffship");
    cmd.current_dir(root)
        .args(["build", "--profile", "10x100", "--out"])
        .arg(&out);
    cmd.assert().success();

    let handoff = fs::read_to_string(out.join("HANDOFF.md")).unwrap();
    assert!(handoff.contains(
        "Profile: `10x100` (`max_parts=10`, `max_bytes_per_part=104857600`; split-by=`file`)"
    ));
}

#[test]
fn build_handoff_profile_config_uses_project_default_and_cli_override() {
    let home_td = tempfile::tempdir().expect("home");
    let home = home_td.path();

    let td = init_repo();
    let root = td.path();

    fs::write(root.join("tracked.txt"), "base\n").unwrap();
    commit_all(root, "base");
    fs::write(root.join("tracked.txt"), "next\n").unwrap();
    commit_all(root, "next");

    write_global_config(
        home,
        r#"
[handoff]
default_profile = "10x100"
"#,
    );
    write_project_config(
        root,
        r#"
[handoff]
default_profile = "team"

[handoff.profiles."team"]
max_parts = 7
max_bytes_per_part = 4096
"#,
    );

    let out_project = root.join("bundle_profile_project");
    let mut project_cmd = assert_cmd::cargo::cargo_bin_cmd!("diffship");
    project_cmd
        .env("HOME", home)
        .current_dir(root)
        .args(["build", "--out"])
        .arg(&out_project);
    project_cmd.assert().success();
    let project_handoff = fs::read_to_string(out_project.join("HANDOFF.md")).unwrap();
    assert!(
        project_handoff.contains(
            "Profile: `team` (`max_parts=7`, `max_bytes_per_part=4096`; split-by=`file`)"
        )
    );

    let out_cli = root.join("bundle_profile_cli");
    let mut cli_cmd = assert_cmd::cargo::cargo_bin_cmd!("diffship");
    cli_cmd
        .env("HOME", home)
        .current_dir(root)
        .args(["build", "--profile", "10x100", "--out"])
        .arg(&out_cli);
    cli_cmd.assert().success();
    let cli_handoff = fs::read_to_string(out_cli.join("HANDOFF.md")).unwrap();
    assert!(cli_handoff.contains(
        "Profile: `10x100` (`max_parts=10`, `max_bytes_per_part=104857600`; split-by=`file`)"
    ));
}

#[test]
fn build_range_mode_direct_accepts_from_to() {
    let td = init_repo();
    let root = td.path();

    fs::write(root.join("a.txt"), "one\n").unwrap();
    commit_all(root, "c1");

    fs::write(root.join("a.txt"), "two\n").unwrap();
    commit_all(root, "c2");

    let out = root.join("bundle_direct");

    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("diffship");
    cmd.current_dir(root)
        .args([
            "build",
            "--range-mode",
            "direct",
            "--from",
            "HEAD~1",
            "--to",
            "HEAD",
            "--out",
        ])
        .arg(&out);

    cmd.assert().success();
    let part = fs::read_to_string(out.join("parts").join("part_01.patch")).unwrap();
    assert!(part.contains("+two"));
}

#[test]
fn build_range_mode_merge_base_uses_merge_base_to_b() {
    let td = init_repo();
    let root = td.path();

    fs::write(root.join("base.txt"), "base\n").unwrap();
    commit_all(root, "base");

    let base_branch = git_stdout(root, &["rev-parse", "--abbrev-ref", "HEAD"]);

    Command::new("git")
        .args(["checkout", "-b", "feature", "-q"])
        .current_dir(root)
        .assert()
        .success();
    fs::write(root.join("feature.txt"), "feature\n").unwrap();
    commit_all(root, "feature");

    Command::new("git")
        .args(["checkout", "-q"])
        .arg(&base_branch)
        .current_dir(root)
        .assert()
        .success();
    fs::write(root.join("main.txt"), "main\n").unwrap();
    commit_all(root, "main");

    let out = root.join("bundle_mergeb");
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("diffship");
    cmd.current_dir(root)
        .args(["build", "--range-mode", "merge-base", "--a"])
        .arg(&base_branch)
        .args(["--b", "feature", "--out"])
        .arg(&out);
    cmd.assert().success();

    let part = fs::read_to_string(out.join("parts").join("part_01.patch")).unwrap();
    assert!(part.contains("feature.txt"));
    assert!(part.contains("+feature"));
    assert!(!part.contains("main.txt"));
}

#[test]
fn build_with_out_is_deterministic_for_parts() {
    let td = init_repo();
    let root = td.path();

    fs::write(root.join("a.txt"), "one\n").unwrap();
    fs::write(root.join("b.txt"), "hello\n").unwrap();
    commit_all(root, "c1");

    fs::write(root.join("a.txt"), "two\n").unwrap();
    fs::write(root.join("b.txt"), "world\n").unwrap();
    commit_all(root, "c2");

    let out1 = root.join("bundle1");
    let out2 = root.join("bundle2");

    let mut cmd1 = assert_cmd::cargo::cargo_bin_cmd!("diffship");
    cmd1.current_dir(root).args(["build", "--out"]).arg(&out1);
    cmd1.assert().success();

    let mut cmd2 = assert_cmd::cargo::cargo_bin_cmd!("diffship");
    cmd2.current_dir(root).args(["build", "--out"]).arg(&out2);
    cmd2.assert().success();

    let p1 = fs::read(out1.join("parts").join("part_01.patch")).unwrap();
    let p2 = fs::read(out2.join("parts").join("part_01.patch")).unwrap();
    assert_eq!(p1, p2);

    let h1 = fs::read_to_string(out1.join("HANDOFF.md")).unwrap();
    assert!(h1.contains("File Table"));
}

#[test]
fn build_root_mode_works_for_single_commit_repo() {
    let td = init_repo();
    let root = td.path();

    fs::write(root.join("only.txt"), "hello\n").unwrap();
    commit_all(root, "root");

    let out = root.join("bundle_root");
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("diffship");
    cmd.current_dir(root)
        .args(["build", "--range-mode", "root", "--out"])
        .arg(&out);
    cmd.assert().success();

    let part = fs::read_to_string(out.join("parts").join("part_01.patch")).unwrap();
    assert!(part.contains("only.txt"));
    assert!(part.contains("+hello"));
}

#[test]
fn build_rejects_when_no_sources_are_selected() {
    let td = init_repo();
    let root = td.path();

    fs::write(root.join("only.txt"), "hello\n").unwrap();
    commit_all(root, "root");

    let out = root.join("bundle_none");
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("diffship");
    cmd.current_dir(root)
        .args(["build", "--no-committed", "--out"])
        .arg(&out);
    cmd.assert()
        .failure()
        .stderr(predicates::str::contains("no sources selected"));
}

#[test]
fn build_can_include_staged_without_committed() {
    let td = init_repo();
    let root = td.path();

    fs::write(root.join("tracked.txt"), "base\n").unwrap();
    commit_all(root, "base");

    fs::write(root.join("tracked.txt"), "staged\n").unwrap();
    Command::new("git")
        .args(["add", "tracked.txt"])
        .current_dir(root)
        .assert()
        .success();

    let out = root.join("bundle_staged");
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("diffship");
    cmd.current_dir(root)
        .args(["build", "--no-committed", "--include-staged", "--out"])
        .arg(&out);
    cmd.assert().success();

    let part = fs::read_to_string(out.join("parts").join("part_01.patch")).unwrap();
    assert!(part.contains("diffship segment: staged"));
    assert!(!part.contains("diffship segment: committed"));
    assert!(part.contains("+staged"));

    let handoff = fs::read_to_string(out.join("HANDOFF.md")).unwrap();
    assert!(handoff.contains(
        "Segments included: committed=`no`, staged=`yes`, unstaged=`no`, untracked=`no`"
    ));
    assert!(handoff.contains("| staged | M | `tracked.txt` |"));
}

#[test]
fn build_can_include_unstaged_and_untracked_text() {
    let td = init_repo();
    let root = td.path();

    fs::write(root.join("tracked.txt"), "base\n").unwrap();
    commit_all(root, "base");

    fs::write(root.join("tracked.txt"), "unstaged\n").unwrap();
    fs::write(root.join("notes.txt"), "hello\nworld\n").unwrap();

    let out = root.join("bundle_worktree");
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("diffship");
    cmd.current_dir(root)
        .args([
            "build",
            "--no-committed",
            "--include-unstaged",
            "--include-untracked",
            "--out",
        ])
        .arg(&out);
    cmd.assert().success();

    let part = fs::read_to_string(out.join("parts").join("part_01.patch")).unwrap();
    assert!(part.contains("diffship segment: unstaged"));
    assert!(part.contains("diffship segment: untracked"));
    assert!(part.contains("tracked.txt"));
    assert!(part.contains("notes.txt"));
    assert!(part.contains("+unstaged"));
    assert!(part.contains("+hello"));

    let handoff = fs::read_to_string(out.join("HANDOFF.md")).unwrap();
    assert!(handoff.contains(
        "Segments included: committed=`no`, staged=`no`, unstaged=`yes`, untracked=`yes`"
    ));
    assert!(handoff.contains("| unstaged | M | `tracked.txt` |"));
    assert!(handoff.contains("| untracked | A | `notes.txt` | 2 | 0 |"));
}

#[test]
fn build_split_by_commit_creates_multiple_parts_and_commit_view() {
    let td = init_repo();
    let root = td.path();

    fs::write(root.join("base.txt"), "base\n").unwrap();
    commit_all(root, "base");

    fs::write(root.join("a.txt"), "one\n").unwrap();
    commit_all(root, "feat a");

    fs::write(root.join("b.txt"), "two\n").unwrap();
    commit_all(root, "feat b");

    let out = root.join("bundle_commit_split");
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("diffship");
    cmd.current_dir(root)
        .args([
            "build",
            "--range-mode",
            "direct",
            "--from",
            "HEAD~2",
            "--to",
            "HEAD",
            "--split-by",
            "commit",
            "--out",
        ])
        .arg(&out);
    cmd.assert().success();

    assert!(out.join("parts").join("part_01.patch").exists());
    assert!(out.join("parts").join("part_02.patch").exists());
    let part1 = fs::read_to_string(out.join("parts").join("part_01.patch")).unwrap();
    let part2 = fs::read_to_string(out.join("parts").join("part_02.patch")).unwrap();
    assert!(part1.contains("a.txt"));
    assert!(part2.contains("b.txt"));

    let handoff = fs::read_to_string(out.join("HANDOFF.md")).unwrap();
    assert!(handoff.contains("## 4) Commit View"));
    assert!(handoff.contains("### 3.1 Quick index"));
    assert!(handoff.contains("| `part_01.patch` | `committed` |"));
    assert!(handoff.contains("| `part_02.patch` | `committed` |"));
    assert!(handoff.contains("feat a"));
    assert!(handoff.contains("feat b"));
    assert!(handoff.contains("`a.txt` → `part_01.patch`"));
    assert!(handoff.contains("`b.txt` → `part_02.patch`"));

    let manifest: Value =
        serde_json::from_str(&fs::read_to_string(out.join("handoff.manifest.json")).unwrap())
            .unwrap();
    assert_eq!(
        manifest
            .get("task_groups")
            .and_then(|v| v.get(0))
            .and_then(|v| v.get("intent_labels"))
            .and_then(|v| v.get(0))
            .and_then(|v| v.as_str()),
        Some("other_update")
    );
    assert_eq!(
        manifest
            .get("task_groups")
            .and_then(|v| v.get(0))
            .and_then(|v| v.get("part_count"))
            .and_then(|v| v.as_u64()),
        Some(2)
    );
    assert_eq!(
        manifest
            .get("task_groups")
            .and_then(|v| v.get(0))
            .and_then(|v| v.get("part_ids"))
            .and_then(|v| v.as_array())
            .map(|items| items
                .iter()
                .filter_map(|item| item.as_str())
                .collect::<Vec<_>>()),
        Some(vec!["part_01.patch", "part_02.patch"])
    );
    assert_eq!(
        manifest
            .get("task_groups")
            .and_then(|v| v.get(0))
            .and_then(|v| v.get("related_context_paths"))
            .and_then(|v| v.as_array())
            .map(|items| items
                .iter()
                .filter_map(|item| item.as_str())
                .collect::<Vec<_>>()),
        Some(vec![
            "parts/part_01.context.json",
            "parts/part_02.context.json"
        ])
    );
    assert_eq!(
        manifest
            .get("task_groups")
            .and_then(|v| v.get(0))
            .and_then(|v| v.get("suggested_read_order"))
            .and_then(|v| v.as_array())
            .map(|items| items
                .iter()
                .filter_map(|item| item.as_str())
                .collect::<Vec<_>>()),
        Some(vec![
            "parts/part_01.context.json",
            "parts/part_02.context.json",
            "parts/part_01.patch",
            "parts/part_02.patch",
        ])
    );
}

#[test]
fn build_max_parts_overflow_falls_back_by_merging_commit_units() {
    let td = init_repo();
    let root = td.path();

    fs::write(root.join("base.txt"), "base\n").unwrap();
    commit_all(root, "base");

    fs::write(root.join("a.txt"), "one\n").unwrap();
    commit_all(root, "feat a");

    fs::write(root.join("b.txt"), "two\n").unwrap();
    commit_all(root, "feat b");

    let out = root.join("bundle_max_parts");
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("diffship");
    cmd.current_dir(root)
        .args([
            "build",
            "--range-mode",
            "direct",
            "--from",
            "HEAD~2",
            "--to",
            "HEAD",
            "--split-by",
            "commit",
            "--max-parts",
            "1",
            "--out",
        ])
        .arg(&out);
    cmd.assert().success();

    assert!(out.join("parts").join("part_01.patch").exists());
    assert!(!out.join("parts").join("part_02.patch").exists());
    assert!(!out.join("excluded.md").exists());
    let part = fs::read_to_string(out.join("parts").join("part_01.patch")).unwrap();
    assert!(part.contains("a.txt"));
    assert!(part.contains("b.txt"));
}

#[test]
fn build_fails_with_exit_3_when_part_bytes_limit_is_exceeded() {
    let td = init_repo();
    let root = td.path();

    fs::write(root.join("a.txt"), "one\n").unwrap();
    commit_all(root, "c1");

    fs::write(root.join("a.txt"), "two\n").unwrap();
    commit_all(root, "c2");

    let out = root.join("bundle_max_bytes");
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("diffship");
    cmd.current_dir(root)
        .args(["build", "--max-bytes-per-part", "1", "--out"])
        .arg(&out);
    cmd.assert()
        .failure()
        .code(3)
        .stderr(predicates::str::contains("max_bytes_per_part=1"));
}

#[test]
fn build_part_bytes_overflow_repacks_into_multiple_parts() {
    let td = init_repo();
    let root = td.path();

    fs::write(root.join("a.txt"), "one\n").unwrap();
    fs::write(root.join("b.txt"), "one\n").unwrap();
    commit_all(root, "c1");

    fs::write(root.join("a.txt"), "two\n").unwrap();
    fs::write(root.join("b.txt"), "two\n").unwrap();
    commit_all(root, "c2");

    let baseline = root.join("bundle_baseline");
    let mut base_cmd = assert_cmd::cargo::cargo_bin_cmd!("diffship");
    base_cmd
        .current_dir(root)
        .args(["build", "--out"])
        .arg(&baseline)
        .assert()
        .success();
    let part = fs::read_to_string(baseline.join("parts").join("part_01.patch")).unwrap();
    let limit = (part.len() as u64).saturating_sub(10);
    assert!(limit > 0);

    let out = root.join("bundle_repacked");
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("diffship");
    cmd.current_dir(root)
        .args(["build", "--max-bytes-per-part", &limit.to_string(), "--out"])
        .arg(&out);
    cmd.assert().success();

    assert!(out.join("parts").join("part_01.patch").exists());
    assert!(out.join("parts").join("part_02.patch").exists());
    assert!(!out.join("excluded.md").exists());
}

#[test]
fn build_reduces_diff_context_before_excluding_an_oversized_unit() {
    let td = init_repo();
    let root = td.path();

    let mut base = String::new();
    for i in 1..=20 {
        base.push_str(&format!("line_{i:02} {}\n", "x".repeat(80)));
    }
    fs::write(root.join("big.txt"), &base).unwrap();
    commit_all(root, "base");

    let mut changed = base.clone();
    changed = changed.replace(
        &format!("line_{:02} {}\n", 10, "x".repeat(80)),
        &format!("line_{:02} {}\n", 10, "y".repeat(80)),
    );
    fs::write(root.join("big.txt"), changed).unwrap();
    commit_all(root, "change");

    let baseline = root.join("bundle_context_baseline");
    let mut baseline_cmd = assert_cmd::cargo::cargo_bin_cmd!("diffship");
    baseline_cmd
        .current_dir(root)
        .args(["build", "--out"])
        .arg(&baseline);
    baseline_cmd.assert().success();
    let baseline_part = fs::read_to_string(baseline.join("parts").join("part_01.patch")).unwrap();
    assert!(baseline_part.contains("line_07"));

    let limit = (baseline_part.len() as u64).saturating_sub(250);
    assert!(limit > 0);

    let out = root.join("bundle_context_reduced");
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("diffship");
    cmd.current_dir(root)
        .args(["build", "--max-bytes-per-part", &limit.to_string(), "--out"])
        .arg(&out);
    cmd.assert().success();

    let reduced_part = fs::read_to_string(out.join("parts").join("part_01.patch")).unwrap();
    assert!(reduced_part.contains("line_10"));
    assert!(!reduced_part.contains("line_07"));
    assert!(!out.join("excluded.md").exists());

    let handoff = fs::read_to_string(out.join("HANDOFF.md")).unwrap();
    assert!(handoff.contains("packing fallback reduced diff context"));
}

#[test]
fn build_untracked_auto_stores_binary_in_attachments_zip() {
    let td = init_repo();
    let root = td.path();

    fs::write(root.join("tracked.txt"), "base\n").unwrap();
    commit_all(root, "base");
    fs::write(root.join("bin.dat"), [0_u8, 159, 146, 150]).unwrap();

    let out = root.join("bundle_auto_untracked");
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("diffship");
    cmd.current_dir(root)
        .args([
            "build",
            "--no-committed",
            "--include-untracked",
            "--include-binary",
            "--out",
        ])
        .arg(&out);
    cmd.assert().success();

    let zip_file = fs::File::open(out.join("attachments.zip")).unwrap();
    let mut zip = ZipArchive::new(zip_file).unwrap();
    let mut names = vec![];
    for i in 0..zip.len() {
        names.push(zip.by_index(i).unwrap().name().to_string());
    }
    assert_eq!(names, vec!["untracked/bin.dat"]);

    let handoff = fs::read_to_string(out.join("HANDOFF.md")).unwrap();
    assert!(handoff.contains("## 5) Attachments"));
    assert!(handoff.contains("`untracked/bin.dat`"));
    assert!(handoff.contains("stored in attachments.zip"));
}

#[test]
fn build_untracked_binary_is_excluded_by_default() {
    let td = init_repo();
    let root = td.path();

    fs::write(root.join("tracked.txt"), "base\n").unwrap();
    commit_all(root, "base");
    fs::write(root.join("bin.dat"), [0_u8, 159, 146, 150]).unwrap();

    let out = root.join("bundle_auto_untracked_default_binary");
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("diffship");
    cmd.current_dir(root)
        .args(["build", "--no-committed", "--include-untracked", "--out"])
        .arg(&out);
    cmd.assert().success();

    assert!(!out.join("attachments.zip").exists());
    assert!(out.join("excluded.md").exists());
    let excluded = fs::read_to_string(out.join("excluded.md")).unwrap();
    assert!(excluded.contains("`bin.dat`"));
    assert!(excluded.contains("binary file excluded by default"));
}

#[test]
fn build_committed_binary_with_raw_mode_goes_to_attachments() {
    let td = init_repo();
    let root = td.path();

    fs::write(root.join("bin.dat"), [0_u8, 1_u8, 2_u8, 3_u8]).unwrap();
    commit_all(root, "base");
    fs::write(root.join("bin.dat"), [4_u8, 5_u8, 6_u8, 7_u8]).unwrap();
    commit_all(root, "binary-update");

    let out = root.join("bundle_committed_binary_raw");
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("diffship");
    cmd.current_dir(root)
        .args(["build", "--include-binary", "--binary-mode", "raw", "--out"])
        .arg(&out);
    cmd.assert().success();

    assert!(out.join("attachments.zip").exists());
    let zip_file = fs::File::open(out.join("attachments.zip")).unwrap();
    let mut zip = ZipArchive::new(zip_file).unwrap();
    let mut names = vec![];
    for i in 0..zip.len() {
        names.push(zip.by_index(i).unwrap().name().to_string());
    }
    assert!(names.contains(&"binary/bin.dat".to_string()));
}

#[test]
fn build_committed_binary_with_patch_mode_keeps_patch_text() {
    let td = init_repo();
    let root = td.path();

    fs::write(root.join("bin.dat"), [0_u8, 1_u8, 2_u8, 3_u8]).unwrap();
    commit_all(root, "base");
    fs::write(root.join("bin.dat"), [4_u8, 5_u8, 6_u8, 7_u8]).unwrap();
    commit_all(root, "binary-update");

    let out = root.join("bundle_committed_binary_patch");
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("diffship");
    cmd.current_dir(root)
        .args([
            "build",
            "--include-binary",
            "--binary-mode",
            "patch",
            "--out",
        ])
        .arg(&out);
    cmd.assert().success();

    assert!(!out.join("attachments.zip").exists());
    assert!(!out.join("excluded.md").exists());
    let part = fs::read_to_string(out.join("parts").join("part_01.patch")).unwrap();
    assert!(part.contains("bin.dat"));
    let handoff = fs::read_to_string(out.join("HANDOFF.md")).unwrap();
    assert!(handoff.contains("| committed | M | `bin.dat` |"));
    assert!(handoff.contains("| part_01.patch |"));
}

#[test]
fn build_committed_binary_with_meta_mode_creates_excluded_md() {
    let td = init_repo();
    let root = td.path();

    fs::write(root.join("bin.dat"), [0_u8, 1_u8, 2_u8, 3_u8]).unwrap();
    commit_all(root, "base");
    fs::write(root.join("bin.dat"), [4_u8, 5_u8, 6_u8, 7_u8]).unwrap();
    commit_all(root, "binary-update");

    let out = root.join("bundle_committed_binary_meta");
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("diffship");
    cmd.current_dir(root)
        .args([
            "build",
            "--include-binary",
            "--binary-mode",
            "meta",
            "--out",
        ])
        .arg(&out);
    cmd.assert().success();

    assert!(!out.join("attachments.zip").exists());
    assert!(out.join("excluded.md").exists());
    let excluded = fs::read_to_string(out.join("excluded.md")).unwrap();
    assert!(excluded.contains("`bin.dat`"));
    assert!(excluded.contains("binary file excluded by binary-mode=meta"));
}

#[test]
fn build_untracked_meta_creates_excluded_md() {
    let td = init_repo();
    let root = td.path();

    fs::write(root.join("tracked.txt"), "base\n").unwrap();
    commit_all(root, "base");
    fs::write(root.join("notes.txt"), "hello\n").unwrap();

    let out = root.join("bundle_meta_untracked");
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("diffship");
    cmd.current_dir(root)
        .args([
            "build",
            "--no-committed",
            "--include-untracked",
            "--untracked-mode",
            "meta",
            "--out",
        ])
        .arg(&out);
    cmd.assert().success();

    assert!(out.join("excluded.md").exists());
    assert!(!out.join("attachments.zip").exists());
    let excluded = fs::read_to_string(out.join("excluded.md")).unwrap();
    assert!(excluded.contains("`notes.txt`"));
    let handoff = fs::read_to_string(out.join("HANDOFF.md")).unwrap();
    assert!(handoff.contains("## 6) Exclusions"));
}

#[test]
fn build_respects_diffshipignore_for_committed_and_untracked() {
    let td = init_repo();
    let root = td.path();

    fs::write(root.join("visible.txt"), "keep\n").unwrap();
    fs::write(root.join("secret.txt"), "hide\n").unwrap();
    fs::write(root.join(".diffshipignore"), "secret.txt\nskipdir/\n").unwrap();
    commit_all(root, "base");

    fs::write(root.join("visible.txt"), "keep2\n").unwrap();
    fs::write(root.join("secret.txt"), "hide2\n").unwrap();
    commit_all(root, "second");

    fs::write(root.join("visible.txt"), "keep3\n").unwrap();
    fs::create_dir_all(root.join("skipdir")).unwrap();
    fs::write(root.join("skipdir").join("note.txt"), "ignored\n").unwrap();
    fs::write(root.join("notes.txt"), "shown\n").unwrap();

    let out = root.join("bundle_ignore");
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("diffship");
    cmd.current_dir(root)
        .args(["build", "--include-untracked", "--yes", "--out"])
        .arg(&out);
    cmd.assert().success();

    let mut patch_all = String::new();
    for ent in fs::read_dir(out.join("parts")).unwrap() {
        let path = ent.unwrap().path();
        patch_all.push_str(&fs::read_to_string(path).unwrap());
    }
    assert!(patch_all.contains("visible.txt"));
    assert!(patch_all.contains("+keep2") || patch_all.contains("+keep3"));
    assert!(!patch_all.contains("secret.txt"));
    assert!(patch_all.contains("notes.txt"));
    assert!(!patch_all.contains("skipdir/note.txt"));

    let handoff = fs::read_to_string(out.join("HANDOFF.md")).unwrap();
    assert!(handoff.contains("Ignore rules: `.diffshipignore` = `yes`"));
    assert!(handoff.contains("visible.txt"));
    assert!(!handoff.contains("secret.txt"));
}

#[test]
fn build_include_and_exclude_filters_apply_to_all_segments() {
    let td = init_repo();
    let root = td.path();

    fs::write(root.join("keep.rs"), "fn keep() {}\n").unwrap();
    fs::write(root.join("skip.rs"), "fn skip() {}\n").unwrap();
    commit_all(root, "base");

    fs::write(root.join("keep.rs"), "fn keep() { println!(\"ok\"); }\n").unwrap();
    fs::write(root.join("skip.rs"), "fn skip() { println!(\"no\"); }\n").unwrap();
    commit_all(root, "second");

    fs::write(root.join("staged.rs"), "fn staged() {}\n").unwrap();
    Command::new("git")
        .args(["add", "staged.rs"])
        .current_dir(root)
        .assert()
        .success();

    fs::write(root.join("note.txt"), "keep me\n").unwrap();
    fs::write(root.join("notes.md"), "drop me\n").unwrap();

    let out = root.join("bundle_filters");
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("diffship");
    cmd.current_dir(root)
        .args([
            "build",
            "--include",
            "*.rs",
            "--include",
            "*.txt",
            "--exclude",
            "skip.rs",
            "--include-staged",
            "--include-untracked",
            "--yes",
            "--out",
        ])
        .arg(&out);
    cmd.assert().success();

    let mut patch_all = String::new();
    for ent in fs::read_dir(out.join("parts")).unwrap() {
        let path = ent.unwrap().path();
        patch_all.push_str(&fs::read_to_string(path).unwrap());
    }

    assert!(patch_all.contains("keep.rs"));
    assert!(patch_all.contains("staged.rs"));
    assert!(patch_all.contains("note.txt"));
    assert!(!patch_all.contains("skip.rs"));
    assert!(!patch_all.contains("notes.md"));

    let handoff = fs::read_to_string(out.join("HANDOFF.md")).unwrap();
    assert!(handoff.contains("Include filters: `*.rs`, `*.txt`"));
    assert!(handoff.contains("Exclude filters: `skip.rs`"));
}

#[test]
fn build_secrets_fail_without_yes_in_non_tty() {
    let td = init_repo();
    let root = td.path();

    fs::write(root.join("base.txt"), "safe\n").unwrap();
    commit_all(root, "base");
    fs::write(
        root.join("token.txt"),
        "ghp_abcdefghijklmnopqrstuvwxyz123456\n",
    )
    .unwrap();
    commit_all(root, "secret");

    let out = root.join("bundle_secret_fail");
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("diffship");
    cmd.current_dir(root).args(["build", "--out"]).arg(&out);
    cmd.assert()
        .failure()
        .code(4)
        .stderr(predicates::str::contains(
            "refused: secrets-like content detected",
        ));
    assert!(out.join("secrets.md").exists());
}

#[test]
fn build_secrets_yes_creates_report_and_handoff_note() {
    let td = init_repo();
    let root = td.path();

    fs::write(root.join("base.txt"), "safe\n").unwrap();
    commit_all(root, "base");
    fs::write(
        root.join("token.txt"),
        "ghp_abcdefghijklmnopqrstuvwxyz123456\n",
    )
    .unwrap();
    commit_all(root, "secret");

    let out = root.join("bundle_secret_yes");
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("diffship");
    cmd.current_dir(root)
        .args(["build", "--yes", "--out"])
        .arg(&out);
    cmd.assert().success();

    let secrets = fs::read_to_string(out.join("secrets.md")).unwrap();
    assert!(secrets.contains("parts/part_01.patch"));
    assert!(secrets.contains("GitHub token-like"));
    assert!(!secrets.contains("ghp_abcdefghijklmnopqrstuvwxyz123456"));

    let handoff = fs::read_to_string(out.join("HANDOFF.md")).unwrap();
    assert!(handoff.contains("## 7) Secrets Warnings"));
    assert!(handoff.contains("secrets.md"));
}

#[test]
fn build_fail_on_secrets_flag_exits_4() {
    let td = init_repo();
    let root = td.path();

    fs::write(root.join("base.txt"), "safe\n").unwrap();
    commit_all(root, "base");
    fs::write(
        root.join("token.txt"),
        "ghp_abcdefghijklmnopqrstuvwxyz123456\n",
    )
    .unwrap();
    commit_all(root, "secret");

    let out = root.join("bundle_secret_ci");
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("diffship");
    cmd.current_dir(root)
        .args(["build", "--fail-on-secrets", "--out"])
        .arg(&out);
    cmd.assert()
        .failure()
        .code(4)
        .stderr(predicates::str::contains("fail-on-secrets"));
}

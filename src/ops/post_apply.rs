use crate::exit::{EXIT_GENERAL, ExitError};
use crate::ops::command_log;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Serialize)]
struct PostApplySummary {
    created_at: String,
    ok: bool,
    changed_paths: Vec<String>,
    change_categories: Vec<String>,
    normalization_summary: PostApplyNormalizationSummary,
    commands: Vec<PostApplyCommandResult>,
}

#[derive(Debug, Serialize)]
struct PostApplyNormalizationSummary {
    changed_repo_state: bool,
    changed_path_count: usize,
    category_counts: BTreeMap<String, usize>,
}

#[derive(Debug, Serialize)]
struct PostApplyCommandResult {
    name: String,
    argv: Vec<String>,
    status: i32,
    duration_ms: u128,
    stdout_path: String,
    stderr_path: String,
}

#[derive(Debug, Clone)]
pub struct PostApplyOut {
    pub ok: bool,
    pub logs_path: String,
}

pub fn run(
    git_root: &Path,
    run_dir: &Path,
    sandbox_path: &Path,
    commands: &[String],
    created_at: &str,
) -> Result<PostApplyOut, ExitError> {
    let out_dir = run_dir.join("post-apply");
    fs::create_dir_all(&out_dir).map_err(|e| {
        ExitError::new(
            EXIT_GENERAL,
            format!("failed to create post-apply dir: {e}"),
        )
    })?;

    let before_state = collect_worktree_state(sandbox_path)?;
    let mut results = vec![];
    let mut ok = true;

    for (idx, cmd) in commands.iter().enumerate() {
        let name = format!("cmd{}", idx + 1);
        let file_stem = format!("{:02}_{}", idx + 1, sanitize_name(&name));
        let argv = vec!["sh".to_string(), "-lc".to_string(), cmd.to_string()];
        let logged = command_log::run_and_log(
            run_dir,
            git_root,
            "post-apply",
            &file_stem,
            sandbox_path,
            &argv,
            None,
        )?;
        if logged.record.status != 0 {
            ok = false;
        }
        results.push(PostApplyCommandResult {
            name: name.clone(),
            argv,
            status: logged.record.status,
            duration_ms: logged.record.duration_ms,
            stdout_path: logged.record.stdout_path.clone(),
            stderr_path: logged.record.stderr_path.clone(),
        });
    }

    let after_state = collect_worktree_state(sandbox_path)?;
    let changed_paths = diff_worktree_state(&before_state, &after_state);
    let category_counts = count_post_apply_change_categories(&changed_paths);
    let change_categories = category_counts.keys().cloned().collect::<Vec<_>>();

    let summary = PostApplySummary {
        created_at: created_at.to_string(),
        ok,
        changed_paths: changed_paths.clone(),
        change_categories,
        normalization_summary: PostApplyNormalizationSummary {
            changed_repo_state: !changed_paths.is_empty(),
            changed_path_count: changed_paths.len(),
            category_counts,
        },
        commands: results,
    };
    let bytes = serde_json::to_vec_pretty(&summary).map_err(|e| {
        ExitError::new(
            EXIT_GENERAL,
            format!("failed to encode post-apply summary: {e}"),
        )
    })?;
    fs::write(run_dir.join("post_apply.json"), bytes).map_err(|e| {
        ExitError::new(
            EXIT_GENERAL,
            format!("failed to write post_apply.json: {e}"),
        )
    })?;

    Ok(PostApplyOut {
        ok,
        logs_path: out_dir.display().to_string(),
    })
}

fn sanitize_name(s: &str) -> String {
    command_log::sanitize_name(s)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct WorktreeStateEntry {
    status: String,
    fingerprint: Option<String>,
}

fn collect_worktree_state(
    sandbox_path: &Path,
) -> Result<BTreeMap<String, WorktreeStateEntry>, ExitError> {
    let output = Command::new("git")
        .args(["status", "--porcelain=v1", "--untracked-files=all"])
        .current_dir(sandbox_path)
        .output()
        .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to run git status: {e}")))?;
    if !output.status.success() {
        return Err(ExitError::new(
            EXIT_GENERAL,
            format!(
                "git status failed in post-apply sandbox (status={})",
                output.status.code().unwrap_or(1)
            ),
        ));
    }

    let stdout = String::from_utf8(output.stdout)
        .map_err(|e| ExitError::new(EXIT_GENERAL, format!("git status was not utf-8: {e}")))?;
    let mut state = BTreeMap::new();
    for line in stdout.lines() {
        if line.len() < 4 {
            continue;
        }
        let status = line[..2].to_string();
        let raw_path = &line[3..];
        for path in parse_status_paths(raw_path) {
            let fingerprint = fingerprint_path(sandbox_path, &path)?;
            state.insert(
                path,
                WorktreeStateEntry {
                    status: status.clone(),
                    fingerprint,
                },
            );
        }
    }
    Ok(state)
}

fn parse_status_paths(raw_path: &str) -> Vec<String> {
    if let Some((before, after)) = raw_path.split_once(" -> ") {
        return vec![before.to_string(), after.to_string()];
    }
    vec![raw_path.to_string()]
}

fn fingerprint_path(sandbox_path: &Path, rel_path: &str) -> Result<Option<String>, ExitError> {
    let abs_path = sandbox_path.join(rel_path);
    if !abs_path.exists() || abs_path.is_dir() {
        return Ok(None);
    }
    let output = Command::new("git")
        .args(["hash-object", "--no-filters"])
        .arg(&abs_path)
        .current_dir(sandbox_path)
        .output()
        .map_err(|e| ExitError::new(EXIT_GENERAL, format!("failed to hash {rel_path}: {e}")))?;
    if !output.status.success() {
        return Err(ExitError::new(
            EXIT_GENERAL,
            format!(
                "git hash-object failed for {} (status={})",
                rel_path,
                output.status.code().unwrap_or(1)
            ),
        ));
    }
    let hash = String::from_utf8(output.stdout)
        .map_err(|e| ExitError::new(EXIT_GENERAL, format!("git hash-object was not utf-8: {e}")))?;
    Ok(Some(hash.trim().to_string()))
}

fn diff_worktree_state(
    before: &BTreeMap<String, WorktreeStateEntry>,
    after: &BTreeMap<String, WorktreeStateEntry>,
) -> Vec<String> {
    before
        .keys()
        .chain(after.keys())
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .filter(|path| before.get(path) != after.get(path))
        .collect()
}

fn count_post_apply_change_categories(paths: &[String]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for path in paths {
        *counts
            .entry(post_apply_change_category(path).to_string())
            .or_insert(0) += 1;
    }
    counts
}

fn post_apply_change_category(path: &str) -> &'static str {
    if is_generated_like_path(path) {
        return "generated_touch";
    }
    match path_category_label(path) {
        "docs" => "docs_touch",
        "config" => "config_touch",
        "source" | "tests" => "code_touch",
        _ => "other_touch",
    }
}

fn is_generated_like_path(path: &str) -> bool {
    path.starts_with("dist/")
        || path.starts_with("build/")
        || path.starts_with("target/")
        || path.starts_with("coverage/")
        || path.contains(".generated.")
        || path.contains("_generated.")
        || path.ends_with(".min.js")
}

fn path_category_label(path: &str) -> &'static str {
    let file_name = std::path::Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    if path.starts_with("docs/")
        || matches!(file_name, "README.md" | "CHANGELOG.md")
        || file_name.ends_with(".md")
    {
        return "docs";
    }
    if path.starts_with(".github/")
        || path.starts_with(".gitlab/")
        || path.starts_with(".diffship/")
        || matches!(
            file_name,
            "Cargo.toml"
                | "Cargo.lock"
                | "package.json"
                | "package-lock.json"
                | "pnpm-lock.yaml"
                | "yarn.lock"
                | "tsconfig.json"
                | "pyproject.toml"
                | "go.mod"
                | "go.sum"
                | "build.gradle"
                | "settings.gradle"
                | "gradle.properties"
                | "pom.xml"
                | "Package.swift"
                | "Makefile"
                | "justfile"
                | "Justfile"
                | "mise.toml"
                | "lefthook.yml"
                | "lefthook.yaml"
        )
    {
        return "config";
    }
    if path.starts_with("tests/")
        || path.starts_with("test/")
        || path.starts_with("__tests__/")
        || file_name.starts_with("test_")
        || file_name.contains("_test.")
        || file_name.contains(".test.")
        || file_name.contains("_spec.")
        || file_name.contains(".spec.")
    {
        return "tests";
    }
    if matches!(
        std::path::Path::new(path)
            .extension()
            .and_then(|ext| ext.to_str()),
        Some(
            "rs" | "py"
                | "ts"
                | "tsx"
                | "js"
                | "jsx"
                | "go"
                | "java"
                | "kt"
                | "swift"
                | "c"
                | "cc"
                | "cpp"
                | "cxx"
                | "h"
                | "hh"
                | "hpp"
        )
    ) {
        return "source";
    }
    "other"
}

#!/usr/bin/env python3
from __future__ import annotations
import re
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]

def read(p: Path) -> str:
    return p.read_text(encoding="utf-8")

def write(p: Path, s: str) -> None:
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(s, encoding="utf-8")

def ensure_struct_fields(text: str, struct_name: str, fields_src: str) -> str:
    pat = re.compile(r"(pub\s+struct\s+" + re.escape(struct_name) + r"\s*\{)(.*?)(\n\})", re.S)
    m = pat.search(text)
    if not m:
        raise SystemExit(f"struct not found: {struct_name}")
    body = m.group(2)
    if "promotion" in body and "commit_policy" in body:
        return text
    insert = "\n" + fields_src.rstrip() + "\n"
    new_body = body.rstrip() + insert
    return text[:m.start(2)] + new_body + text[m.end(2):]

def patch_cli_rs() -> None:
    p = REPO_ROOT / "src" / "cli.rs"
    s = read(p)

    promote_fields = """    /// Promotion mode override (none|working-tree|commit)
    #[arg(long, value_name = "MODE")]
    pub promotion: Option<String>,

    /// Commit policy override (auto|manual)
    #[arg(long, value_name = "POLICY")]
    pub commit_policy: Option<String>,
"""
    loop_fields = promote_fields

    s = ensure_struct_fields(s, "PromoteArgs", promote_fields)
    s = ensure_struct_fields(s, "LoopArgs", loop_fields)
    write(p, s)

def patch_loop_cmd_rs() -> None:
    p = REPO_ROOT / "src" / "ops" / "loop_cmd.rs"
    s = read(p)

    # Extend config overrides
    if "promotion_mode:" not in s:
        s = s.replace(
            "target_branch: args.target_branch.clone(),",
            "target_branch: args.target_branch.clone(),\n            promotion_mode: args.promotion.clone(),\n            commit_policy: args.commit_policy.clone(),",
        )

    # Add lock info fields
    if "--promotion" not in s:
        s = s.replace(
            "format!(\"--ack-tasks={}\", args.ack_tasks),",
            "format!(\"--ack-tasks={}\", args.ack_tasks),\n            format!(\"--promotion={}\", args.promotion.as_deref().unwrap_or(\"\")),\n            format!(\"--commit-policy={}\", args.commit_policy.as_deref().unwrap_or(\"\")),",
        )

    # Extend promote_locked call
    s = s.replace(
        "&cfg.target_branch,\n        args.ack_secrets,",
        "&cfg.target_branch,\n        &cfg.promotion_mode,\n        &cfg.commit_policy,\n        args.ack_secrets,",
    )

    write(p, s)

def patch_promote_rs() -> None:
    p = REPO_ROOT / "src" / "ops" / "promote.rs"
    s = read(p)

    # Extend config overrides
    if "promotion_mode:" not in s:
        s = s.replace(
            "target_branch: args.target_branch.clone(),",
            "target_branch: args.target_branch.clone(),\n            promotion_mode: args.promotion.clone(),\n            commit_policy: args.commit_policy.clone(),",
        )

    # Add lock info fields
    if "--promotion" not in s:
        s = s.replace(
            "format!(\"--target-branch={}\", args.target_branch.as_deref().unwrap_or(\"\")),",
            "format!(\"--target-branch={}\", args.target_branch.as_deref().unwrap_or(\"\")),\n            format!(\"--promotion={}\", args.promotion.as_deref().unwrap_or(\"\")),\n            format!(\"--commit-policy={}\", args.commit_policy.as_deref().unwrap_or(\"\")),",
        )

    # Update cmd call site
    s = s.replace(
        "&cfg.target_branch,\n        args.ack_secrets,",
        "&cfg.target_branch,\n        &cfg.promotion_mode,\n        &cfg.commit_policy,\n        args.ack_secrets,",
    )

    # Update promote_locked signature by inserting params after target_branch.
    s = s.replace(
        "target_branch: &str,\n    ack_secrets: bool,",
        "target_branch: &str,\n    promotion_mode: &str,\n    commit_policy: &str,\n    ack_secrets: bool,",
    )

    # Insert promotion=none branch (best-effort).
    if "promotion_mode == \"none\"" not in s:
        s = s.replace(
            "let effective_target = choose_target_branch(git_root, target_branch)?;",
            "let effective_target = choose_target_branch(git_root, target_branch)?;\n\n    // Promotion mode switch\n    if promotion_mode == \"none\" {\n        let summary = PromoteSummary {\n            run_id: run_id.to_string(),\n            created_at: created_at.clone(),\n            target_branch: effective_target.clone(),\n            base_commit: base_commit.clone(),\n            promoted_head: None,\n            commits: vec![],\n            ok: true,\n            error: Some(\"promotion skipped by policy (promotion=none)\".to_string()),\n            secrets_hits: hits.len(),\n            tasks_present,\n            user_tasks_path: if tasks_present {\n                Some(user_tasks_path.display().to_string())\n            } else {\n                None\n            },\n        };\n        write_promote_summary(&run_dir, &summary)?;\n        if !keep_sandbox {\n            worktree::remove_worktree_best_effort(git_root, Path::new(&sb.path));\n        }\n        return Ok(());\n    }",
        )

    # Gate auto commit by commit_policy (only affects git-apply path)
    s = s.replace(
        "ensure_commit_in_sandbox(&sandbox_path, &run_dir, &commit_msg)?;",
        "if commit_policy == \"auto\" {\n                ensure_commit_in_sandbox(&sandbox_path, &run_dir, &commit_msg)?;\n            }",
    )

    write(p, s)

def patch_main_rs() -> None:
    p = REPO_ROOT / "src" / "main.rs"
    s = read(p)

    # Ensure fallback constructors include new fields (best-effort).
    if "commit_policy:" not in s:
        s = re.sub(
            r"(let\s+args\s*=\s*cli::PromoteArgs\s*\{)",
            r"\1\n        promotion: None,\n        commit_policy: None,",
            s,
            count=1,
        )
        s = re.sub(
            r"(let\s+args\s*=\s*cli::LoopArgs\s*\{)",
            r"\1\n        promotion: None,\n        commit_policy: None,",
            s,
            count=1,
        )

    write(p, s)

def add_test() -> None:
    p = REPO_ROOT / "tests" / "m4_02_promotion_switch.rs"
    if p.exists():
        return
    write(p, 'use assert_cmd::prelude::*;\nuse std::fs;\nuse std::process::Command;\nuse tempfile::TempDir;\n\nfn init_repo_with_branches(branches: &[&str]) -> TempDir {\n    let td = tempfile::tempdir().expect("tempdir");\n    let root = td.path();\n\n    Command::new("git")\n        .args(["init", "-q"])\n        .current_dir(root)\n        .assert()\n        .success();\n\n    Command::new("git")\n        .args(["config", "user.email", "test@example.com"])\n        .current_dir(root)\n        .assert()\n        .success();\n    Command::new("git")\n        .args(["config", "user.name", "diffship-test"])\n        .current_dir(root)\n        .assert()\n        .success();\n\n    fs::write(root.join("README.md"), "hello\\n").unwrap();\n    Command::new("git")\n        .args(["add", "."])\n        .current_dir(root)\n        .assert()\n        .success();\n    Command::new("git")\n        .args(["commit", "-m", "init", "-q"])\n        .current_dir(root)\n        .assert()\n        .success();\n\n    for br in branches {\n        Command::new("git")\n            .args(["branch", br])\n            .current_dir(root)\n            .assert()\n            .success();\n    }\n    if branches.contains(&"develop") {\n        Command::new("git")\n            .args(["checkout", "-q", "develop"])\n            .current_dir(root)\n            .assert()\n            .success();\n    }\n\n    td\n}\n\nfn diffship_cmd(home: &std::path::Path) -> Command {\n    let mut c = Command::new(assert_cmd::cargo::cargo_bin!("diffship"));\n    c.env("HOME", home);\n    c\n}\n\nfn head(root: &std::path::Path) -> String {\n    let out = Command::new("git")\n        .args(["rev-parse", "HEAD"])\n        .current_dir(root)\n        .output()\n        .expect("rev-parse")\n        .stdout;\n    String::from_utf8_lossy(&out).trim().to_string()\n}\n\nfn make_patch_by_editing_readme(repo_root: &std::path::Path, new_line: &str) -> String {\n    let readme = repo_root.join("README.md");\n    let mut s = fs::read_to_string(&readme).unwrap();\n    s.push_str(new_line);\n    fs::write(&readme, s).unwrap();\n\n    let out = Command::new("git")\n        .args(["diff"])\n        .current_dir(repo_root)\n        .output()\n        .expect("git diff")\n        .stdout;\n    let patch = String::from_utf8_lossy(&out).to_string();\n\n    Command::new("git")\n        .args(["checkout", "--", "README.md"])\n        .current_dir(repo_root)\n        .assert()\n        .success();\n\n    patch\n}\n\nfn make_bundle_dir_with_patch(\n    repo_root: &std::path::Path,\n    base_commit: &str,\n    patch_text: &str,\n    touched_files: &[&str],\n    extra_manifest: &str,\n) -> TempDir {\n    let td = tempfile::tempdir().expect("bundle tempdir");\n    let root = td.path();\n\n    let bundle_root = root.join("patchship_test");\n    fs::create_dir_all(bundle_root.join("changes")).unwrap();\n\n    let manifest = format!(\n        "protocol_version: \\"1\\"\\ntask_id: \\"TEST\\"\\nbase_commit: \\"{}\\"\\napply_mode: git-apply\\ntouched_files:\\n{}\\n{}",\n        base_commit,\n        touched_files\n            .iter()\n            .map(|p| format!("  - \\"{}\\"", p))\n            .collect::<Vec<_>>()\n            .join("\\n"),\n        extra_manifest\n    );\n    fs::write(bundle_root.join("manifest.yaml"), manifest).unwrap();\n    fs::write(bundle_root.join("changes").join("0001.patch"), patch_text).unwrap();\n\n    Command::new("git")\n        .args([\n            "apply",\n            "--check",\n            bundle_root.join("changes/0001.patch").to_str().unwrap(),\n        ])\n        .current_dir(repo_root)\n        .assert()\n        .success();\n\n    td\n}\n\nfn extract_run_id(stdout: &[u8]) -> String {\n    let s = String::from_utf8_lossy(stdout);\n    for line in s.lines() {\n        if let Some(rest) = line.strip_prefix("  run_id  : ") {\n            return rest.trim().to_string();\n        }\n    }\n    panic!("run_id not found in output: {s}");\n}\n\n#[test]\nfn m4_02_promotion_none_skips_cherry_pick() {\n    let home_td = tempfile::tempdir().expect("home");\n    let home = home_td.path();\n\n    let td = init_repo_with_branches(&["develop"]);\n    let root = td.path();\n    let base = head(root);\n\n    let patch = make_patch_by_editing_readme(root, "no-promote\\n");\n    let bundle_td = make_bundle_dir_with_patch(root, &base, &patch, &["README.md"], "");\n    let bundle_root = bundle_td.path().join("patchship_test");\n\n    let out = diffship_cmd(home)\n        .args([\n            "loop",\n            bundle_root.to_str().unwrap(),\n            "--promotion",\n            "none",\n            "--target-branch",\n            "develop",\n        ])\n        .current_dir(root)\n        .assert()\n        .success()\n        .get_output()\n        .stdout\n        .clone();\n\n    let _run_id = extract_run_id(&out);\n\n    // Promotion skipped: repository HEAD should remain unchanged.\n    assert_eq!(head(root), base);\n}\n')

def main() -> None:
    patch_cli_rs()
    patch_loop_cmd_rs()
    patch_promote_rs()
    patch_main_rs()
    add_test()
    print("OK: applied M4-02 promotion/commit-policy changes")

if __name__ == "__main__":
    main()

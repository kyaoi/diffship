use assert_cmd::prelude::*;
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
    assert_eq!(bundles.len(), 1);

    let bundle = &bundles[0];
    assert!(bundle.join("HANDOFF.md").exists());
    assert!(bundle.join("parts").join("part_01.patch").exists());

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
    assert!(handoff.contains("4. Open the first patch part: `parts/part_01.patch`"));

    let part = fs::read_to_string(bundle.join("parts").join("part_01.patch")).unwrap();
    assert!(part.contains("diffship segment: committed"));
    assert!(part.contains("a.txt"));
    assert!(part.contains("+two"));
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
        .args(["build", "--no-committed", "--include-untracked", "--out"])
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

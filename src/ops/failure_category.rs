use std::borrow::Cow;

pub const BASE_COMMIT_MISMATCH: &str = "base_commit_mismatch";
pub const PATCH_APPLY_FAILED: &str = "patch_apply_failed";
pub const POST_APPLY_FAILED: &str = "post_apply_failed";
pub const VERIFY_FAILED: &str = "verify_failed";
pub const VERIFY_TEST_FAILED: &str = "verify_test_failed";
pub const VERIFY_LINT_FAILED: &str = "verify_lint_failed";
pub const VERIFY_DOCS_FAILED: &str = "verify_docs_failed";
pub const PROMOTION_FAILED: &str = "promotion_failed";
pub const PROMOTION_BLOCKED_SECRETS: &str = "promotion_blocked_secrets";
pub const PROMOTION_BLOCKED_TASKS: &str = "promotion_blocked_tasks";

pub struct VerifyCommandLike<'a> {
    pub name: &'a str,
    pub argv: &'a [String],
    pub status: i32,
}

pub fn classify_verify_failure(commands: &[VerifyCommandLike<'_>]) -> Option<String> {
    let failed = commands.iter().find(|cmd| cmd.status != 0)?;
    Some(classify_verify_command(failed.name, failed.argv).into_owned())
}

fn classify_verify_command<'a>(name: &'a str, argv: &'a [String]) -> Cow<'static, str> {
    let mut text = String::new();
    text.push_str(&name.to_ascii_lowercase());
    if !argv.is_empty() {
        text.push(' ');
        text.push_str(&argv.join(" ").to_ascii_lowercase());
    }

    if contains_any(
        &text,
        &[
            "cargo test",
            "pytest",
            "jest",
            "vitest",
            "go test",
            "rspec",
            "npm test",
            "pnpm test",
            "bun test",
            " just test",
            " test ",
        ],
    ) {
        return Cow::Borrowed(VERIFY_TEST_FAILED);
    }
    if contains_any(
        &text,
        &[
            "clippy",
            "eslint",
            "ruff",
            "flake8",
            "shellcheck",
            "golangci-lint",
            " lint",
            "lint ",
        ],
    ) {
        return Cow::Borrowed(VERIFY_LINT_FAILED);
    }
    if contains_any(
        &text,
        &[
            "docs-check",
            "trace-check",
            "cargo doc",
            "mdbook",
            "mkdocs",
            "sphinx",
            "docusaurus",
            "docs",
            "trace",
        ],
    ) {
        return Cow::Borrowed(VERIFY_DOCS_FAILED);
    }
    Cow::Borrowed(VERIFY_FAILED)
}

fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::{
        VERIFY_DOCS_FAILED, VERIFY_FAILED, VERIFY_LINT_FAILED, VERIFY_TEST_FAILED,
        VerifyCommandLike, classify_verify_failure,
    };

    #[test]
    fn classify_verify_failure_prefers_test_like_commands() {
        let argv = ["test".to_string()];
        let commands = vec![VerifyCommandLike {
            name: "cargo",
            argv: &argv,
            status: 101,
        }];
        assert_eq!(
            classify_verify_failure(&commands).as_deref(),
            Some(VERIFY_TEST_FAILED)
        );
    }

    #[test]
    fn classify_verify_failure_prefers_lint_like_commands() {
        let argv = ["clippy".to_string(), "--all-targets".to_string()];
        let commands = vec![VerifyCommandLike {
            name: "cargo",
            argv: &argv,
            status: 1,
        }];
        assert_eq!(
            classify_verify_failure(&commands).as_deref(),
            Some(VERIFY_LINT_FAILED)
        );
    }

    #[test]
    fn classify_verify_failure_prefers_docs_like_commands() {
        let argv = [
            "-lc".to_string(),
            "just docs-check && just trace-check".to_string(),
        ];
        let commands = vec![VerifyCommandLike {
            name: "sh",
            argv: &argv,
            status: 1,
        }];
        assert_eq!(
            classify_verify_failure(&commands).as_deref(),
            Some(VERIFY_DOCS_FAILED)
        );
    }

    #[test]
    fn classify_verify_failure_falls_back_to_generic_category() {
        let argv = ["diff".to_string(), "--check".to_string()];
        let commands = vec![VerifyCommandLike {
            name: "git",
            argv: &argv,
            status: 2,
        }];
        assert_eq!(
            classify_verify_failure(&commands).as_deref(),
            Some(VERIFY_FAILED)
        );
    }
}

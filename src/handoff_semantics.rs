use std::collections::BTreeSet;
use std::path::Path;

pub(crate) fn language_label(path: &str) -> &'static str {
    let file_name = Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    match Path::new(path).extension().and_then(|ext| ext.to_str()) {
        Some("rs") => "rust",
        Some("py") => "python",
        Some("ts") => "typescript",
        Some("tsx") => "tsx",
        Some("js") => "javascript",
        Some("jsx") => "jsx",
        Some("go") => "go",
        Some("java") => "java",
        Some("kt") => "kotlin",
        Some("swift") => "swift",
        Some("c") | Some("h") => "c",
        Some("cc") | Some("cpp") | Some("cxx") | Some("hpp") | Some("hh") => "cpp",
        Some("json") => "json",
        Some("yaml") | Some("yml") => "yaml",
        Some("toml") => "toml",
        Some("md") => "markdown",
        Some("sh") | Some("bash") | Some("zsh") => "shell",
        _ if matches!(file_name, "Makefile" | "justfile" | "Justfile") => "build-script",
        _ => "unknown",
    }
}

pub(crate) fn is_generated_like_path(path: &str) -> bool {
    let file_name = Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    path.starts_with("dist/")
        || path.starts_with("build/")
        || path.starts_with("target/")
        || path.starts_with("coverage/")
        || file_name.contains(".generated.")
        || file_name.contains("_generated.")
        || file_name.ends_with(".min.js")
}

pub(crate) fn is_lockfile_path(path: &str) -> bool {
    matches!(
        Path::new(path).file_name().and_then(|name| name.to_str()),
        Some(
            "Cargo.lock"
                | "package-lock.json"
                | "pnpm-lock.yaml"
                | "yarn.lock"
                | "poetry.lock"
                | "Gemfile.lock"
                | "composer.lock"
                | "Podfile.lock"
        )
    )
}

pub(crate) fn is_ci_or_tooling_path(path: &str) -> bool {
    let file_name = Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    path.starts_with(".github/")
        || path.starts_with(".gitlab/")
        || matches!(
            file_name,
            "Makefile" | "justfile" | "Justfile" | "mise.toml" | "lefthook.yml" | "lefthook.yaml"
        )
}

pub(crate) fn is_repo_rule_path(path: &str) -> bool {
    matches!(
        path,
        "AGENTS.md"
            | "docs/AI_WORKFLOW.md"
            | "docs/PROJECT_KIT_TEMPLATE.md"
            | "docs/HANDOFF_TEMPLATE.md"
            | ".diffship/PROJECT_RULES.md"
            | ".diffship/AI_GUIDE.md"
            | ".diffship/PROJECT_KIT.md"
    )
}

pub(crate) fn is_dependency_policy_path(path: &str) -> bool {
    matches!(
        Path::new(path).file_name().and_then(|name| name.to_str()),
        Some(
            "Cargo.toml"
                | "package.json"
                | "pyproject.toml"
                | "requirements.txt"
                | "requirements-dev.txt"
                | "constraints.txt"
                | "Gemfile"
                | "go.mod"
                | "go.sum"
                | "Package.swift"
        )
    ) || is_lockfile_path(path)
}

pub(crate) fn is_build_graph_path(path: &str) -> bool {
    matches!(
        Path::new(path).file_name().and_then(|name| name.to_str()),
        Some(
            "Cargo.toml"
                | "package.json"
                | "pyproject.toml"
                | "tsconfig.json"
                | "Makefile"
                | "justfile"
                | "Justfile"
                | "CMakeLists.txt"
                | "build.gradle"
                | "build.gradle.kts"
                | "settings.gradle"
                | "settings.gradle.kts"
                | "Package.swift"
                | "Dockerfile"
        )
    ) || path.ends_with("docker-compose.yml")
}

pub(crate) fn is_test_infrastructure_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.contains("/fixtures/")
        || lower.contains("/fixture/")
        || lower.contains("/mocks/")
        || lower.contains("/mock/")
        || lower.contains("/snapshots/")
        || lower.contains("/snapshot/")
        || lower.contains("/harness/")
        || lower.starts_with("tests/fixtures/")
        || lower.starts_with("test/fixtures/")
}

pub(crate) fn infer_related_test_candidates(
    path: &str,
    candidate_paths: &BTreeSet<String>,
) -> Vec<String> {
    if is_test_like_path(path) {
        return Vec::new();
    }

    let p = Path::new(path);
    let file_name = p
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    let stem = p
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    let ext = p
        .extension()
        .and_then(|name| name.to_str())
        .map(|ext| format!(".{ext}"))
        .unwrap_or_default();
    let rel_no_src = path.strip_prefix("src/").unwrap_or(path);
    let parent = Path::new(rel_no_src)
        .parent()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_default();

    let mut explicit = BTreeSet::new();
    for root in ["tests", "test", "__tests__"] {
        explicit.insert(join_rel(root, rel_no_src));
        if !file_name.is_empty() {
            explicit.insert(join_rel(root, file_name));
        }
        if !stem.is_empty() {
            for candidate in [
                format!("{stem}_test{ext}"),
                format!("test_{stem}{ext}"),
                format!("{stem}.test{ext}"),
                format!("{stem}.spec{ext}"),
            ] {
                let rel = if parent.is_empty() {
                    candidate
                } else {
                    format!("{parent}/{candidate}")
                };
                explicit.insert(join_rel(root, &rel));
            }
        }
    }

    explicit
        .into_iter()
        .filter(|candidate| candidate_paths.contains(candidate) && is_test_like_path(candidate))
        .collect()
}

pub(crate) fn infer_related_source_candidates(
    path: &str,
    candidate_paths: &BTreeSet<String>,
) -> Vec<String> {
    if !is_test_like_path(path) {
        return Vec::new();
    }

    let stripped = strip_test_root(path).unwrap_or(path);
    let normalized = normalize_test_path_to_source_like(stripped);
    let file_name = Path::new(&normalized)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_string();

    let mut explicit = BTreeSet::new();
    explicit.insert(join_rel("src", &normalized));
    if !file_name.is_empty() {
        explicit.insert(join_rel("src", &file_name));
    }

    explicit
        .into_iter()
        .filter(|candidate| candidate_paths.contains(candidate) && candidate.starts_with("src/"))
        .collect()
}

pub(crate) fn infer_related_doc_candidates(
    path: &str,
    candidate_paths: &BTreeSet<String>,
) -> Vec<String> {
    if path.starts_with("docs/") || path == "README.md" {
        return Vec::new();
    }

    let normalized = normalize_path_for_related_docs(path);
    let p = Path::new(&normalized);
    let stem = p
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    let parent = p
        .parent()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_default();

    let mut explicit = BTreeSet::new();
    explicit.insert("README.md".to_string());
    explicit.insert(join_rel("docs", &replace_extension(&normalized, "md")));
    if !stem.is_empty() {
        explicit.insert(join_rel("docs", &format!("{stem}.md")));
        if !parent.is_empty() {
            explicit.insert(join_rel("docs", &format!("{parent}/{stem}.md")));
        }
    }
    explicit
        .into_iter()
        .filter(|candidate| {
            candidate_paths.contains(candidate)
                && (candidate == "README.md" || candidate.starts_with("docs/"))
        })
        .collect()
}

pub(crate) fn infer_related_config_candidates(
    path: &str,
    candidate_paths: &BTreeSet<String>,
) -> Vec<String> {
    let mut explicit = BTreeSet::new();
    if candidate_paths.contains("Cargo.toml") && matches!(language_label(path), "rust") {
        explicit.insert("Cargo.toml".to_string());
    }
    if matches!(
        language_label(path),
        "typescript" | "tsx" | "javascript" | "jsx"
    ) {
        for candidate in ["package.json", "tsconfig.json"] {
            if candidate_paths.contains(candidate) {
                explicit.insert(candidate.to_string());
            }
        }
    }
    if matches!(language_label(path), "python") {
        for candidate in ["pyproject.toml", "requirements.txt"] {
            if candidate_paths.contains(candidate) {
                explicit.insert(candidate.to_string());
            }
        }
    }
    if matches!(language_label(path), "go") && candidate_paths.contains("go.mod") {
        explicit.insert("go.mod".to_string());
    }
    if matches!(language_label(path), "java" | "kotlin") {
        for candidate in [
            "build.gradle",
            "settings.gradle",
            "gradle.properties",
            "pom.xml",
        ] {
            if candidate_paths.contains(candidate) {
                explicit.insert(candidate.to_string());
            }
        }
    }
    if matches!(language_label(path), "swift") && candidate_paths.contains("Package.swift") {
        explicit.insert("Package.swift".to_string());
    }
    if path.starts_with(".github/") {
        for candidate in ["README.md", "docs/ci.md"] {
            if candidate_paths.contains(candidate) {
                explicit.insert(candidate.to_string());
            }
        }
    }
    explicit.into_iter().collect()
}

pub(crate) fn is_test_like_path(path: &str) -> bool {
    let file_name = Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    path.starts_with("tests/")
        || path.starts_with("test/")
        || path.starts_with("__tests__/")
        || file_name.starts_with("test_")
        || file_name.contains("_test.")
        || file_name.contains(".test.")
        || file_name.contains("_spec.")
        || file_name.contains(".spec.")
}

fn normalize_path_for_related_docs(path: &str) -> String {
    if is_test_like_path(path) {
        normalize_test_path_to_source_like(strip_test_root(path).unwrap_or(path))
    } else {
        path.strip_prefix("src/").unwrap_or(path).to_string()
    }
}

fn strip_test_root(path: &str) -> Option<&str> {
    for root in ["tests/", "test/", "__tests__/"] {
        if let Some(stripped) = path.strip_prefix(root) {
            return Some(stripped);
        }
    }
    None
}

fn normalize_test_path_to_source_like(path: &str) -> String {
    let p = Path::new(path);
    let parent = p
        .parent()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_default();
    let file_name = p
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    let normalized = normalize_test_file_name(file_name);
    if parent.is_empty() {
        normalized
    } else {
        format!("{parent}/{normalized}")
    }
}

fn normalize_test_file_name(file_name: &str) -> String {
    let name = file_name.strip_prefix("test_").unwrap_or(file_name);
    for needle in ["_test.", ".test.", "_spec.", ".spec."] {
        if let Some((prefix, suffix)) = name.split_once(needle) {
            return format!("{prefix}.{suffix}");
        }
    }
    name.to_string()
}

fn replace_extension(path: &str, new_ext: &str) -> String {
    let p = Path::new(path);
    let stem = p.file_stem().and_then(|name| name.to_str()).unwrap_or(path);
    let parent = p
        .parent()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_default();
    let file_name = format!("{stem}.{new_ext}");
    if parent.is_empty() {
        file_name
    } else {
        format!("{parent}/{file_name}")
    }
}

fn join_rel(root: &str, rel: &str) -> String {
    let trimmed = rel.trim_start_matches('/');
    if trimmed.is_empty() {
        root.to_string()
    } else {
        format!("{root}/{trimmed}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::iter::FromIterator;

    #[test]
    fn language_label_classifies_common_extensions() {
        assert_eq!(language_label("src/lib.rs"), "rust");
        assert_eq!(language_label("web/app.ts"), "typescript");
        assert_eq!(language_label("web/App.tsx"), "tsx");
        assert_eq!(language_label("scripts/build.sh"), "shell");
        assert_eq!(language_label("justfile"), "build-script");
        assert_eq!(language_label("docs/spec.md"), "markdown");
        assert_eq!(language_label("unknown/file.xyz"), "unknown");
    }

    #[test]
    fn semantic_flags_classify_generated_lockfile_and_tooling_paths() {
        assert!(is_generated_like_path("target/debug/app"));
        assert!(is_generated_like_path("web/dist/app.min.js"));
        assert!(is_generated_like_path("src/foo_generated.rs"));
        assert!(!is_generated_like_path("src/lib.rs"));

        assert!(is_lockfile_path("Cargo.lock"));
        assert!(is_lockfile_path("frontend/pnpm-lock.yaml"));
        assert!(!is_lockfile_path("Cargo.toml"));

        assert!(is_ci_or_tooling_path(".github/workflows/ci.yml"));
        assert!(is_ci_or_tooling_path("justfile"));
        assert!(is_ci_or_tooling_path("tooling/mise.toml"));
        assert!(!is_ci_or_tooling_path("src/lib.rs"));

        assert!(is_repo_rule_path("AGENTS.md"));
        assert!(is_repo_rule_path(".diffship/PROJECT_RULES.md"));
        assert!(!is_repo_rule_path("docs/lib.md"));

        assert!(is_dependency_policy_path("Cargo.toml"));
        assert!(is_dependency_policy_path("package.json"));
        assert!(!is_dependency_policy_path("src/lib.rs"));

        assert!(is_build_graph_path("tsconfig.json"));
        assert!(is_build_graph_path("Dockerfile"));
        assert!(!is_build_graph_path("docs/lib.md"));

        assert!(is_test_infrastructure_path("tests/fixtures/api.json"));
        assert!(is_test_infrastructure_path("tests/mocks/client.rs"));
        assert!(!is_test_infrastructure_path("tests/lib_test.rs"));
    }

    #[test]
    fn infer_related_test_candidates_returns_existing_stable_matches() {
        let candidates = BTreeSet::from_iter(
            [
                "src/lib.rs",
                "tests/lib.rs",
                "tests/lib_test.rs",
                "tests/nested/foo_test.py",
                "tests/nested/foo.spec.py",
                "tests/ignored.md",
            ]
            .into_iter()
            .map(ToOwned::to_owned),
        );

        assert_eq!(
            infer_related_test_candidates("src/lib.rs", &candidates),
            vec!["tests/lib.rs".to_string(), "tests/lib_test.rs".to_string()]
        );
        assert_eq!(
            infer_related_test_candidates("src/nested/foo.py", &candidates),
            vec![
                "tests/nested/foo.spec.py".to_string(),
                "tests/nested/foo_test.py".to_string(),
            ]
        );
        assert!(infer_related_test_candidates("tests/lib_test.rs", &candidates).is_empty());
    }

    #[test]
    fn infer_related_source_candidates_returns_existing_stable_matches() {
        let candidates = BTreeSet::from_iter(
            [
                "src/lib.rs",
                "src/nested/foo.py",
                "tests/lib_test.rs",
                "tests/nested/test_foo.py",
                "tests/nested/foo.spec.py",
            ]
            .into_iter()
            .map(ToOwned::to_owned),
        );

        assert_eq!(
            infer_related_source_candidates("tests/lib_test.rs", &candidates),
            vec!["src/lib.rs".to_string()]
        );
        assert_eq!(
            infer_related_source_candidates("tests/nested/test_foo.py", &candidates),
            vec!["src/nested/foo.py".to_string()]
        );
        assert_eq!(
            infer_related_source_candidates("tests/nested/foo.spec.py", &candidates),
            vec!["src/nested/foo.py".to_string()]
        );
        assert!(infer_related_source_candidates("src/lib.rs", &candidates).is_empty());
    }

    #[test]
    fn infer_related_doc_and_config_candidates_return_existing_stable_matches() {
        let candidates = BTreeSet::from_iter(
            [
                "src/lib.rs",
                "tests/lib_test.rs",
                "Cargo.toml",
                "README.md",
                "docs/lib.md",
                "docs/nested/foo.md",
                "src/nested/foo.py",
                "tests/nested/test_foo.py",
                "pyproject.toml",
            ]
            .into_iter()
            .map(ToOwned::to_owned),
        );

        assert_eq!(
            infer_related_doc_candidates("src/lib.rs", &candidates),
            vec!["README.md".to_string(), "docs/lib.md".to_string()]
        );
        assert_eq!(
            infer_related_doc_candidates("tests/nested/test_foo.py", &candidates),
            vec!["README.md".to_string(), "docs/nested/foo.md".to_string()]
        );
        assert_eq!(
            infer_related_config_candidates("src/lib.rs", &candidates),
            vec!["Cargo.toml".to_string()]
        );
        assert_eq!(
            infer_related_config_candidates("tests/nested/test_foo.py", &candidates),
            vec!["pyproject.toml".to_string()]
        );
    }
}

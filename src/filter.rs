use crate::exit::{EXIT_GENERAL, ExitError};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Default)]
pub struct PathFilter {
    includes: Vec<String>,
    excludes: Vec<String>,
    ignore_rules: Vec<IgnoreRule>,
}

#[derive(Debug, Clone)]
struct IgnoreRule {
    pattern: String,
    dir_only: bool,
}

impl PathFilter {
    pub fn load(
        git_root: &Path,
        includes: &[String],
        excludes: &[String],
    ) -> Result<Self, ExitError> {
        let path = git_root.join(".diffshipignore");
        let mut ignore_rules = Vec::new();

        if path.exists() {
            let text = fs::read_to_string(&path).map_err(|e| {
                ExitError::new(
                    EXIT_GENERAL,
                    format!("failed to read {}: {e}", path.display()),
                )
            })?;
            for raw in text.lines() {
                let line = raw.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                let dir_only = line.ends_with('/');
                let pat = line.trim_start_matches('/').trim_end_matches('/');
                if pat.is_empty() {
                    continue;
                }
                ignore_rules.push(IgnoreRule {
                    pattern: pat.replace('\\', "/"),
                    dir_only,
                });
            }
        }

        Ok(Self {
            includes: normalize_patterns(includes),
            excludes: normalize_patterns(excludes),
            ignore_rules,
        })
    }

    pub fn allows(&self, rel: &str) -> bool {
        let rel = rel.replace('\\', "/");
        if self
            .ignore_rules
            .iter()
            .any(|rule| rule_matches(rule, &rel))
        {
            return false;
        }
        if self
            .excludes
            .iter()
            .any(|pat| explicit_rule_matches(pat, &rel))
        {
            return false;
        }
        if self.includes.is_empty() {
            return true;
        }
        self.includes
            .iter()
            .any(|pat| explicit_rule_matches(pat, &rel))
    }

    pub fn has_ignore_rules(&self) -> bool {
        !self.ignore_rules.is_empty()
    }

    pub fn includes(&self) -> &[String] {
        &self.includes
    }

    pub fn excludes(&self) -> &[String] {
        &self.excludes
    }
}

fn normalize_patterns(patterns: &[String]) -> Vec<String> {
    patterns
        .iter()
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .map(|p| p.trim_matches('/').replace('\\', "/"))
        .collect()
}

fn explicit_rule_matches(pattern: &str, rel: &str) -> bool {
    if pattern.is_empty() {
        return false;
    }
    if simple_glob_match(pattern, rel) {
        return true;
    }
    if !pattern.contains('/') {
        for part in rel.split('/') {
            if simple_glob_match(pattern, part) {
                return true;
            }
        }
    }
    rel == pattern || rel.starts_with(&format!("{pattern}/"))
}

fn rule_matches(rule: &IgnoreRule, rel: &str) -> bool {
    if rule.dir_only {
        return rel == rule.pattern || rel.starts_with(&format!("{}/", rule.pattern));
    }
    explicit_rule_matches(&rule.pattern, rel)
}

fn simple_glob_match(pattern: &str, text: &str) -> bool {
    fn inner(p: &[u8], t: &[u8]) -> bool {
        if p.is_empty() {
            return t.is_empty();
        }
        match p[0] {
            b'*' => inner(&p[1..], t) || (!t.is_empty() && inner(p, &t[1..])),
            b'?' => !t.is_empty() && inner(&p[1..], &t[1..]),
            c => !t.is_empty() && c == t[0] && inner(&p[1..], &t[1..]),
        }
    }
    inner(pattern.as_bytes(), text.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::PathFilter;

    #[test]
    fn include_and_exclude_match_paths_consistently() {
        let td = tempfile::tempdir().expect("tempdir");
        let filters = PathFilter::load(
            td.path(),
            &["src/*.rs".to_string(), "*.md".to_string()],
            &["src/generated.rs".to_string()],
        )
        .expect("load");

        assert!(filters.allows("src/lib.rs"));
        assert!(filters.allows("docs/guide.md"));
        assert!(!filters.allows("src/generated.rs"));
        assert!(!filters.allows("tests/test.rs"));
    }
}

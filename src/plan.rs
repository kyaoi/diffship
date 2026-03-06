#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandoffPlan {
    pub range_mode: String,
    pub from: Option<String>,
    pub to: Option<String>,
    pub a: Option<String>,
    pub b: Option<String>,
    pub include_committed: bool,
    pub include_staged: bool,
    pub include_unstaged: bool,
    pub include_untracked: bool,
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    pub split_by: String,
    pub untracked_mode: String,
    pub include_binary: bool,
    pub binary_mode: String,
    pub max_parts: Option<usize>,
    pub max_bytes_per_part: Option<u64>,
    pub out: Option<String>,
    pub zip: bool,
    pub yes: bool,
    pub fail_on_secrets: bool,
}

impl Default for HandoffPlan {
    fn default() -> Self {
        Self {
            range_mode: "last".to_string(),
            from: None,
            to: None,
            a: None,
            b: None,
            include_committed: true,
            include_staged: false,
            include_unstaged: false,
            include_untracked: false,
            include: vec![],
            exclude: vec![],
            split_by: "auto".to_string(),
            untracked_mode: "auto".to_string(),
            include_binary: false,
            binary_mode: "raw".to_string(),
            max_parts: None,
            max_bytes_per_part: None,
            out: None,
            zip: false,
            yes: false,
            fail_on_secrets: false,
        }
    }
}

impl HandoffPlan {
    pub fn to_build_args(&self) -> Vec<String> {
        let mut out = vec!["build".to_string()];

        if self.range_mode != "last" {
            out.push("--range-mode".to_string());
            out.push(self.range_mode.clone());
        }
        push_opt_flag(&mut out, "--from", self.from.as_deref());
        push_opt_flag(&mut out, "--to", self.to.as_deref());
        push_opt_flag(&mut out, "--a", self.a.as_deref());
        push_opt_flag(&mut out, "--b", self.b.as_deref());

        if !self.include_committed {
            out.push("--no-committed".to_string());
        }
        if self.include_staged {
            out.push("--include-staged".to_string());
        }
        if self.include_unstaged {
            out.push("--include-unstaged".to_string());
        }
        if self.include_untracked {
            out.push("--include-untracked".to_string());
        }

        for pat in &self.include {
            out.push("--include".to_string());
            out.push(pat.clone());
        }
        for pat in &self.exclude {
            out.push("--exclude".to_string());
            out.push(pat.clone());
        }

        if self.split_by != "auto" {
            out.push("--split-by".to_string());
            out.push(self.split_by.clone());
        }
        if self.untracked_mode != "auto" {
            out.push("--untracked-mode".to_string());
            out.push(self.untracked_mode.clone());
        }
        if self.include_binary {
            out.push("--include-binary".to_string());
        }
        if self.binary_mode != "raw" {
            out.push("--binary-mode".to_string());
            out.push(self.binary_mode.clone());
        }
        if let Some(max_parts) = self.max_parts {
            out.push("--max-parts".to_string());
            out.push(max_parts.to_string());
        }
        if let Some(max_bytes) = self.max_bytes_per_part {
            out.push("--max-bytes-per-part".to_string());
            out.push(max_bytes.to_string());
        }
        push_opt_flag(&mut out, "--out", self.out.as_deref());
        if self.zip {
            out.push("--zip".to_string());
        }
        if self.yes {
            out.push("--yes".to_string());
        }
        if self.fail_on_secrets {
            out.push("--fail-on-secrets".to_string());
        }

        out
    }

    pub fn to_shell_command(&self) -> String {
        self.to_build_args()
            .into_iter()
            .map(|arg| shell_quote(&arg))
            .collect::<Vec<_>>()
            .join(" ")
            .replacen("build", "diffship build", 1)
    }
}

fn push_opt_flag(out: &mut Vec<String>, flag: &str, value: Option<&str>) {
    if let Some(value) = value {
        out.push(flag.to_string());
        out.push(value.to_string());
    }
}

fn shell_quote(s: &str) -> String {
    if !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'/' | b'.' | b'_' | b'-' | b'='))
    {
        return s.to_string();
    }
    format!("'{}'", s.replace('\'', "'\"'\"'"))
}

#[cfg(test)]
mod tests {
    use super::HandoffPlan;

    #[test]
    fn build_args_omit_defaults() {
        let plan = HandoffPlan::default();
        assert_eq!(plan.to_build_args(), vec!["build"]);
        assert_eq!(plan.to_shell_command(), "diffship build");
    }

    #[test]
    fn build_args_include_selected_flags() {
        let plan = HandoffPlan {
            range_mode: "direct".to_string(),
            from: Some("HEAD~3".to_string()),
            to: Some("feature branch".to_string()),
            include_staged: true,
            include_untracked: true,
            include: vec!["src/*.rs".to_string()],
            exclude: vec!["src/generated.rs".to_string()],
            split_by: "commit".to_string(),
            zip: true,
            ..HandoffPlan::default()
        };
        assert_eq!(
            plan.to_build_args(),
            vec![
                "build",
                "--range-mode",
                "direct",
                "--from",
                "HEAD~3",
                "--to",
                "feature branch",
                "--include-staged",
                "--include-untracked",
                "--include",
                "src/*.rs",
                "--exclude",
                "src/generated.rs",
                "--split-by",
                "commit",
                "--zip",
            ]
        );
        assert_eq!(
            plan.to_shell_command(),
            "diffship build --range-mode direct --from 'HEAD~3' --to 'feature branch' --include-staged --include-untracked --include 'src/*.rs' --exclude src/generated.rs --split-by commit --zip"
        );
    }
}

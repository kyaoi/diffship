use crate::cli::BuildArgs;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandoffPlan {
    pub profile: Option<String>,
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
            profile: None,
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
    pub fn from_build_args(args: &BuildArgs) -> Self {
        Self {
            profile: args.profile.clone(),
            range_mode: args.range_mode.clone(),
            from: args.from.clone(),
            to: args.to.clone(),
            a: args.a.clone(),
            b: args.b.clone(),
            include_committed: !args.no_committed,
            include_staged: args.include_staged,
            include_unstaged: args.include_unstaged,
            include_untracked: args.include_untracked,
            include: args.include.clone(),
            exclude: args.exclude.clone(),
            split_by: args.split_by.clone().unwrap_or_else(|| "auto".to_string()),
            untracked_mode: args.untracked_mode.clone(),
            include_binary: args.include_binary,
            binary_mode: args.binary_mode.clone(),
            max_parts: args.max_parts,
            max_bytes_per_part: args.max_bytes_per_part,
            out: args.out.clone(),
            zip: args.zip,
            yes: args.yes,
            fail_on_secrets: args.fail_on_secrets,
        }
    }

    pub fn into_build_args(self, plan: Option<String>, plan_out: Option<String>) -> BuildArgs {
        BuildArgs {
            profile: self.profile,
            range_mode: self.range_mode,
            from: self.from,
            to: self.to,
            a: self.a,
            b: self.b,
            no_committed: !self.include_committed,
            include: self.include,
            exclude: self.exclude,
            include_staged: self.include_staged,
            include_unstaged: self.include_unstaged,
            include_untracked: self.include_untracked,
            split_by: Some(self.split_by),
            untracked_mode: self.untracked_mode,
            include_binary: self.include_binary,
            binary_mode: self.binary_mode,
            max_parts: self.max_parts,
            max_bytes_per_part: self.max_bytes_per_part,
            plan,
            plan_out,
            out: self.out,
            zip: self.zip,
            yes: self.yes,
            fail_on_secrets: self.fail_on_secrets,
        }
    }

    pub fn to_build_args(&self) -> Vec<String> {
        let mut out = vec!["build".to_string()];

        push_opt_flag(&mut out, "--profile", self.profile.as_deref());
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

    pub fn to_toml_string(&self) -> String {
        let mut out = String::new();
        out.push_str("# diffship handoff plan\n");
        push_toml_opt(&mut out, "profile", self.profile.as_deref());
        out.push_str(&format!("range_mode = {}\n", toml_string(&self.range_mode)));
        push_toml_opt(&mut out, "from", self.from.as_deref());
        push_toml_opt(&mut out, "to", self.to.as_deref());
        push_toml_opt(&mut out, "a", self.a.as_deref());
        push_toml_opt(&mut out, "b", self.b.as_deref());
        out.push_str(&format!("include_committed = {}\n", self.include_committed));
        out.push_str(&format!("include_staged = {}\n", self.include_staged));
        out.push_str(&format!("include_unstaged = {}\n", self.include_unstaged));
        out.push_str(&format!("include_untracked = {}\n", self.include_untracked));
        out.push_str(&format!("include = {}\n", toml_array(&self.include)));
        out.push_str(&format!("exclude = {}\n", toml_array(&self.exclude)));
        out.push_str(&format!("split_by = {}\n", toml_string(&self.split_by)));
        out.push_str(&format!(
            "untracked_mode = {}\n",
            toml_string(&self.untracked_mode)
        ));
        out.push_str(&format!("include_binary = {}\n", self.include_binary));
        out.push_str(&format!(
            "binary_mode = {}\n",
            toml_string(&self.binary_mode)
        ));
        if let Some(max_parts) = self.max_parts {
            out.push_str(&format!("max_parts = {max_parts}\n"));
        }
        if let Some(max_bytes) = self.max_bytes_per_part {
            out.push_str(&format!("max_bytes_per_part = {max_bytes}\n"));
        }
        out
    }

    pub fn write_to_path(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent)
                .map_err(|e| format!("failed to create {}: {e}", parent.display()))?;
        }
        fs::write(path, self.to_toml_string())
            .map_err(|e| format!("failed to write {}: {e}", path.display()))
    }

    pub fn from_file(path: &Path) -> Result<Self, String> {
        let text = fs::read_to_string(path)
            .map_err(|e| format!("failed to read {}: {e}", path.display()))?;
        Self::from_toml_str(&text)
    }

    pub fn from_toml_str(s: &str) -> Result<Self, String> {
        let mut plan = Self::default();
        for (lineno, raw) in s.lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                return Err(format!("invalid plan line {}: {}", lineno + 1, raw));
            };
            let key = key.trim();
            let value = value.trim();
            match key {
                "profile" => plan.profile = Some(parse_toml_string(value)?),
                "range_mode" => plan.range_mode = parse_toml_string(value)?,
                "from" => plan.from = Some(parse_toml_string(value)?),
                "to" => plan.to = Some(parse_toml_string(value)?),
                "a" => plan.a = Some(parse_toml_string(value)?),
                "b" => plan.b = Some(parse_toml_string(value)?),
                "include_committed" => plan.include_committed = parse_toml_bool(value)?,
                "include_staged" => plan.include_staged = parse_toml_bool(value)?,
                "include_unstaged" => plan.include_unstaged = parse_toml_bool(value)?,
                "include_untracked" => plan.include_untracked = parse_toml_bool(value)?,
                "include" => plan.include = parse_toml_array(value)?,
                "exclude" => plan.exclude = parse_toml_array(value)?,
                "split_by" => plan.split_by = parse_toml_string(value)?,
                "untracked_mode" => plan.untracked_mode = parse_toml_string(value)?,
                "include_binary" => plan.include_binary = parse_toml_bool(value)?,
                "binary_mode" => plan.binary_mode = parse_toml_string(value)?,
                "max_parts" => plan.max_parts = Some(parse_toml_usize(value)?),
                "max_bytes_per_part" => {
                    plan.max_bytes_per_part = Some(parse_toml_u64(value)?);
                }
                "out" => plan.out = Some(parse_toml_string(value)?),
                "zip" => plan.zip = parse_toml_bool(value)?,
                "yes" => plan.yes = parse_toml_bool(value)?,
                "fail_on_secrets" => plan.fail_on_secrets = parse_toml_bool(value)?,
                other => return Err(format!("unknown plan key: {other}")),
            }
        }
        Ok(plan)
    }

    pub fn replay_shell_command_with_overrides(plan_path: &str, plan: &Self) -> String {
        let mut args = vec![
            "diffship".to_string(),
            "build".to_string(),
            "--plan".to_string(),
        ];
        args.push(plan_path.to_string());
        push_opt_flag(&mut args, "--out", plan.out.as_deref());
        if plan.zip {
            args.push("--zip".to_string());
        }
        if plan.yes {
            args.push("--yes".to_string());
        }
        if plan.fail_on_secrets {
            args.push("--fail-on-secrets".to_string());
        }
        args.into_iter()
            .map(|arg| shell_quote(&arg))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

fn push_opt_flag(out: &mut Vec<String>, flag: &str, value: Option<&str>) {
    if let Some(value) = value {
        out.push(flag.to_string());
        out.push(value.to_string());
    }
}

fn push_toml_opt(out: &mut String, key: &str, value: Option<&str>) {
    if let Some(value) = value {
        out.push_str(&format!("{key} = {}\n", toml_string(value)));
    }
}

fn toml_string(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

fn toml_array(values: &[String]) -> String {
    let items = values
        .iter()
        .map(|v| toml_string(v))
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{items}]")
}

fn parse_toml_string(value: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if !(trimmed.starts_with('"') && trimmed.ends_with('"')) {
        return Err(format!("expected TOML string, got: {value}"));
    }
    let inner = &trimmed[1..trimmed.len() - 1];
    let mut out = String::new();
    let mut escaped = false;
    for ch in inner.chars() {
        if escaped {
            out.push(ch);
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        out.push(ch);
    }
    Ok(out)
}

fn parse_toml_bool(value: &str) -> Result<bool, String> {
    match value.trim() {
        "true" => Ok(true),
        "false" => Ok(false),
        other => Err(format!("expected TOML bool, got: {other}")),
    }
}

fn parse_toml_array(value: &str) -> Result<Vec<String>, String> {
    let trimmed = value.trim();
    if !(trimmed.starts_with('[') && trimmed.ends_with(']')) {
        return Err(format!("expected TOML array, got: {value}"));
    }
    let inner = trimmed[1..trimmed.len() - 1].trim();
    if inner.is_empty() {
        return Ok(vec![]);
    }
    inner
        .split(',')
        .map(str::trim)
        .map(parse_toml_string)
        .collect()
}

fn parse_toml_usize(value: &str) -> Result<usize, String> {
    value
        .trim()
        .parse::<usize>()
        .map_err(|e| format!("expected usize, got '{value}': {e}"))
}

fn parse_toml_u64(value: &str) -> Result<u64, String> {
    value
        .trim()
        .parse::<u64>()
        .map_err(|e| format!("expected u64, got '{value}': {e}"))
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
            profile: Some("10x100".to_string()),
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
                "--profile",
                "10x100",
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
            "diffship build --profile 10x100 --range-mode direct --from 'HEAD~3' --to 'feature branch' --include-staged --include-untracked --include 'src/*.rs' --exclude src/generated.rs --split-by commit --zip"
        );
    }

    #[test]
    fn toml_roundtrip_preserves_plan() {
        let plan = HandoffPlan {
            profile: Some("team".to_string()),
            range_mode: "merge-base".to_string(),
            a: Some("main".to_string()),
            b: Some("feature".to_string()),
            include_staged: true,
            include: vec!["src/*.rs".to_string(), "docs/*.md".to_string()],
            exclude: vec!["src/generated.rs".to_string()],
            max_parts: Some(12),
            max_bytes_per_part: Some(1024),
            out: Some("out dir".to_string()),
            zip: true,
            yes: true,
            ..HandoffPlan::default()
        };
        let parsed = HandoffPlan::from_toml_str(&plan.to_toml_string()).expect("parse");
        assert_eq!(parsed.out, None);
        assert!(!parsed.zip);
        assert!(!parsed.yes);
        assert_eq!(parsed.profile, plan.profile);
        assert_eq!(parsed.range_mode, plan.range_mode);
        assert_eq!(parsed.a, plan.a);
        assert_eq!(parsed.b, plan.b);
        assert_eq!(parsed.include, plan.include);
        assert_eq!(parsed.exclude, plan.exclude);
        assert_eq!(
            HandoffPlan::replay_shell_command_with_overrides("tmp/plan.toml", &plan),
            "diffship build --plan tmp/plan.toml --out 'out dir' --zip --yes"
        );
    }
}

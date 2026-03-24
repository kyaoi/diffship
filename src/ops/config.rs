use crate::exit::{EXIT_GENERAL, ExitError};
use crate::ops::patch_bundle::PatchBundleManifest;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

pub const AI_GENERATED_CONFIG_FILE_NAME: &str = "ai_generated_config.toml";
pub const AI_GENERATED_CONFIG_RELATIVE_PATH: &str = ".diffship/ai_generated_config.toml";

pub const SUPPORTED_EDITABLE_DIFFSHIP_PATHS: &[&str] = &[
    ".diffship/.gitignore",
    ".diffship/AI_GUIDE.md",
    ".diffship/config.toml",
    ".diffship/forbid.toml",
    ".diffship/PROJECT_KIT.md",
    ".diffship/PROJECT_RULES.md",
    AI_GENERATED_CONFIG_RELATIVE_PATH,
];

pub fn normalize_repo_relative_path(path: &str) -> String {
    let normalized = path.trim().replace('\\', "/");
    normalized.trim_start_matches("./").to_string()
}

pub fn normalize_supported_editable_diffship_path(path: &str) -> Option<String> {
    let normalized = normalize_repo_relative_path(path);
    SUPPORTED_EDITABLE_DIFFSHIP_PATHS
        .iter()
        .find(|candidate| **candidate == normalized)
        .map(|_| normalized)
}

/// A partial ops configuration where every field is optional.
/// Later sources override earlier sources.
#[derive(Debug, Clone, Default)]
pub struct OpsConfigOverrides {
    pub verify_profile: Option<String>,
    pub verify_profiles: BTreeMap<String, BTreeMap<String, String>>,
    pub post_apply_commands: BTreeMap<String, String>,
    pub forbid_patterns: BTreeMap<String, String>,
    pub editable_diffship_files: BTreeMap<String, String>,
    pub target_branch: Option<String>,
    pub promotion_mode: Option<String>,
    pub commit_policy: Option<String>,
    pub workflow_profile: Option<String>,
    pub workflow_strategy_mode: Option<String>,
    pub workflow_strategy_default_profile: Option<String>,
    pub workflow_strategy_error_overrides: BTreeMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct OpsConfig {
    pub verify_profile: String,
    pub target_branch: String,
    pub promotion_mode: String,
    pub commit_policy: String,
    pub workflow_profile: String,
    pub workflow_strategy_mode: String,
    verify_profiles: BTreeMap<String, BTreeMap<String, String>>,
    post_apply_commands: BTreeMap<String, String>,
    forbid_patterns: BTreeMap<String, String>,
    editable_diffship_files: BTreeMap<String, String>,
    workflow_strategy_default_profile: Option<String>,
    workflow_strategy_error_overrides: BTreeMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct WorkflowConfig {
    pub default_profile: String,
    pub strategy_mode: String,
    strategy_default_profile: Option<String>,
    strategy_error_overrides: BTreeMap<String, String>,
}

impl OpsConfig {
    pub fn defaults() -> Self {
        Self {
            verify_profile: "standard".to_string(),
            target_branch: "develop".to_string(),
            promotion_mode: "commit".to_string(),
            commit_policy: "auto".to_string(),
            workflow_profile: "balanced".to_string(),
            workflow_strategy_mode: "suggest".to_string(),
            verify_profiles: BTreeMap::new(),
            post_apply_commands: BTreeMap::new(),
            forbid_patterns: BTreeMap::new(),
            editable_diffship_files: BTreeMap::new(),
            workflow_strategy_default_profile: None,
            workflow_strategy_error_overrides: BTreeMap::new(),
        }
    }

    fn apply_overrides(&mut self, o: OpsConfigOverrides) {
        if let Some(v) = o.verify_profile {
            self.verify_profile = v;
        }
        if let Some(v) = o.target_branch {
            self.target_branch = v;
        }
        if let Some(v) = o.promotion_mode {
            self.promotion_mode = v;
        }
        if let Some(v) = o.commit_policy {
            self.commit_policy = v;
        }
        if let Some(v) = o.workflow_profile {
            self.workflow_profile = v;
        }
        if let Some(v) = o.workflow_strategy_mode {
            self.workflow_strategy_mode = v;
        }
        if let Some(v) = o.workflow_strategy_default_profile {
            self.workflow_strategy_default_profile = Some(v);
        }
        for (profile, commands) in o.verify_profiles {
            self.verify_profiles.insert(profile, commands);
        }
        for (key, cmd) in o.post_apply_commands {
            self.post_apply_commands.insert(key, cmd);
        }
        for (key, pattern) in o.forbid_patterns {
            self.forbid_patterns.insert(key, pattern);
        }
        for (key, path) in o.editable_diffship_files {
            self.editable_diffship_files.insert(key, path);
        }
        for (key, profile) in o.workflow_strategy_error_overrides {
            self.workflow_strategy_error_overrides.insert(key, profile);
        }
    }

    pub fn verify_commands_for_selected_profile(&self) -> Option<Vec<String>> {
        let m = self.verify_profiles.get(&self.verify_profile)?;
        let mut items = m
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect::<Vec<_>>();
        items.sort_by_key(|(k, _)| profile_command_sort_key(k));
        let cmds = items
            .into_iter()
            .map(|(_, v)| v)
            .filter(|v| !v.trim().is_empty())
            .collect::<Vec<_>>();
        if cmds.is_empty() { None } else { Some(cmds) }
    }

    pub fn post_apply_commands(&self) -> Option<Vec<String>> {
        let mut items = self
            .post_apply_commands
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect::<Vec<_>>();
        items.sort_by_key(|(k, _)| profile_command_sort_key(k));
        let cmds = items
            .into_iter()
            .map(|(_, v)| v)
            .filter(|v| !v.trim().is_empty())
            .collect::<Vec<_>>();
        if cmds.is_empty() { None } else { Some(cmds) }
    }

    pub fn forbid_patterns(&self) -> Vec<String> {
        self.forbid_patterns
            .values()
            .filter(|v| !v.trim().is_empty())
            .cloned()
            .collect()
    }

    pub fn editable_diffship_files(&self) -> Vec<String> {
        let mut out = BTreeSet::new();
        for value in self.editable_diffship_files.values() {
            if let Some(path) = normalize_supported_editable_diffship_path(value) {
                out.insert(path);
            }
        }
        out.into_iter().collect()
    }
}

impl WorkflowConfig {
    pub fn defaults() -> Self {
        Self {
            default_profile: "balanced".to_string(),
            strategy_mode: "suggest".to_string(),
            strategy_default_profile: None,
            strategy_error_overrides: BTreeMap::new(),
        }
    }

    fn apply_overrides(&mut self, o: OpsConfigOverrides) {
        if let Some(v) = o.workflow_profile {
            self.default_profile = v;
        }
        if let Some(v) = o.workflow_strategy_mode {
            self.strategy_mode = v;
        }
        if let Some(v) = o.workflow_strategy_default_profile {
            self.strategy_default_profile = Some(v);
        }
        for (key, profile) in o.workflow_strategy_error_overrides {
            self.strategy_error_overrides.insert(key, profile);
        }
    }

    pub fn strategy_default_profile(&self) -> &str {
        self.strategy_default_profile
            .as_deref()
            .unwrap_or(&self.default_profile)
    }

    pub fn strategy_error_overrides(&self) -> &BTreeMap<String, String> {
        &self.strategy_error_overrides
    }
}

/// Resolve ops configuration with precedence:
/// CLI > manifest > project > global > default.
pub fn resolve_ops_config(
    git_root: &Path,
    manifest: Option<&PatchBundleManifest>,
    cli: OpsConfigOverrides,
) -> Result<OpsConfig, ExitError> {
    let mut cfg = OpsConfig::defaults();

    // global
    if let Some(p) = global_config_path()
        && p.is_file()
    {
        let o = load_config_file(&p)?;
        cfg.apply_overrides(o);
    }

    // project (allow both legacy .diffship.toml and current .diffship/config.toml; latter wins)
    for p in project_config_paths(git_root) {
        if p.is_file() {
            let o = load_config_file(&p)?;
            cfg.apply_overrides(o);
        }
    }

    // bundle manifest (if present)
    if let Some(m) = manifest {
        cfg.apply_overrides(overrides_from_manifest(m));
    }

    // CLI
    cfg.apply_overrides(cli);

    validate(&cfg)?;

    Ok(cfg)
}

pub fn resolve_workflow_config(git_root: &Path) -> Result<WorkflowConfig, ExitError> {
    let mut cfg = WorkflowConfig::defaults();

    if let Some(p) = global_config_path()
        && p.is_file()
    {
        let o = load_config_file(&p)?;
        cfg.apply_overrides(o);
    }

    for p in project_config_paths(git_root) {
        if p.is_file() {
            let o = load_config_file(&p)?;
            cfg.apply_overrides(o);
        }
    }

    validate_workflow_config(&cfg)?;

    Ok(cfg)
}

fn validate(cfg: &OpsConfig) -> Result<(), ExitError> {
    let verify_profile_ok = matches!(cfg.verify_profile.as_str(), "fast" | "standard" | "full")
        || cfg.verify_profiles.contains_key(&cfg.verify_profile);
    if !verify_profile_ok {
        return Err(ExitError::new(
            EXIT_GENERAL,
            format!(
                "invalid verify profile: {} (expected fast|standard|full or [verify.profiles.<name>])",
                cfg.verify_profile
            ),
        ));
    }

    match cfg.promotion_mode.as_str() {
        "none" | "working-tree" | "commit" => {}
        other => {
            return Err(ExitError::new(
                EXIT_GENERAL,
                format!("invalid promotion mode: {other} (expected none|working-tree|commit)"),
            ));
        }
    }

    match cfg.commit_policy.as_str() {
        "auto" | "manual" => {}
        other => {
            return Err(ExitError::new(
                EXIT_GENERAL,
                format!("invalid commit policy: {other} (expected auto|manual)"),
            ));
        }
    }

    if cfg.target_branch.trim().is_empty() {
        return Err(ExitError::new(
            EXIT_GENERAL,
            "target branch must not be empty",
        ));
    }

    validate_workflow_config(&WorkflowConfig {
        default_profile: cfg.workflow_profile.clone(),
        strategy_mode: cfg.workflow_strategy_mode.clone(),
        strategy_default_profile: cfg.workflow_strategy_default_profile.clone(),
        strategy_error_overrides: cfg.workflow_strategy_error_overrides.clone(),
    })?;

    Ok(())
}

fn validate_workflow_config(cfg: &WorkflowConfig) -> Result<(), ExitError> {
    if cfg.default_profile.trim().is_empty() {
        return Err(ExitError::new(
            EXIT_GENERAL,
            "workflow default profile must not be empty",
        ));
    }

    match cfg.strategy_mode.as_str() {
        "suggest" | "prefer" | "force" | "off" => {}
        other => {
            return Err(ExitError::new(
                EXIT_GENERAL,
                format!(
                    "invalid workflow strategy mode: {other} (expected suggest|prefer|force|off)"
                ),
            ));
        }
    }

    if cfg.strategy_default_profile().trim().is_empty() {
        return Err(ExitError::new(
            EXIT_GENERAL,
            "workflow strategy default profile must not be empty",
        ));
    }

    for (category, profile) in cfg.strategy_error_overrides() {
        if category.trim().is_empty() {
            return Err(ExitError::new(
                EXIT_GENERAL,
                "workflow strategy error override category must not be empty",
            ));
        }
        if profile.trim().is_empty() {
            return Err(ExitError::new(
                EXIT_GENERAL,
                format!(
                    "workflow strategy error override profile must not be empty for category {category}"
                ),
            ));
        }
    }

    Ok(())
}

fn overrides_from_manifest(m: &PatchBundleManifest) -> OpsConfigOverrides {
    OpsConfigOverrides {
        verify_profile: m.verify_profile.clone(),
        target_branch: m.target_branch.clone(),
        promotion_mode: m.promotion_mode.clone(),
        commit_policy: m.commit_policy.clone(),
        ..Default::default()
    }
}

fn global_config_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join(".config/diffship/config.toml"))
}

fn project_config_paths(git_root: &Path) -> Vec<PathBuf> {
    vec![
        // legacy (docs/CONFIG.md v0): repo-local root file
        git_root.join(".diffship.toml"),
        // AI-owned local config layer (loaded before the user-owned project config)
        git_root
            .join(".diffship")
            .join(AI_GENERATED_CONFIG_FILE_NAME),
        // current: stored under the diffship directory (written by `diffship init`)
        git_root.join(".diffship").join("config.toml"),
        // dedicated local forbid patterns file
        git_root.join(".diffship").join("forbid.toml"),
    ]
}

/// Load a TOML config file and extract only the supported keys.
///
/// This is intentionally a minimal TOML reader:
/// - Only parses `[table]` and `[table.sub]` headers
/// - Only parses `key = value` scalars (string / bare)
/// - Ignores everything else
fn load_config_file(path: &Path) -> Result<OpsConfigOverrides, ExitError> {
    let text = fs::read_to_string(path).map_err(|e| {
        ExitError::new(
            EXIT_GENERAL,
            format!("failed to read config {}: {e}", path.display()),
        )
    })?;
    Ok(parse_config_toml(&text))
}

fn parse_config_toml(s: &str) -> OpsConfigOverrides {
    let mut out = OpsConfigOverrides::default();
    let mut section: Vec<String> = vec![];

    for raw in s.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        // strip inline comment (best-effort)
        let line = match line.split_once('#') {
            Some((a, _)) => a.trim(),
            None => line,
        };
        if line.is_empty() {
            continue;
        }

        if line.starts_with('[') && line.ends_with(']') {
            let name = line.trim_start_matches('[').trim_end_matches(']').trim();
            section = name
                .split('.')
                .map(|p| p.trim().to_string())
                .filter(|p| !p.is_empty())
                .collect();
            continue;
        }

        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        let key = k.trim();
        let val = unquote(v.trim()).to_string();

        // Supported mappings:
        // - [verify] default_profile = "standard"
        // - [ops] verify_profile = "standard" (legacy stub convenience)
        // - [ops.promote] target_branch / mode
        // - [ops.post_apply] cmd1 = "just fmt-fix"
        // - [ops.forbid] path1 = "pnpm-lock.yaml"
        // - [ops.editable_diffship] path1 = ".diffship/AI_GUIDE.md"
        // - [ops.commit] policy
        // - [workflow] default_profile = "balanced"
        // - [workflow.strategy] mode / default_profile
        // - [workflow.strategy.error_overrides] verify_test_failed = "regression-test-first"
        let section_str: Vec<&str> = section.iter().map(|s| s.as_str()).collect();
        match section_str.as_slice() {
            ["verify"] => {
                if key == "default_profile" {
                    out.verify_profile = Some(val);
                }
            }
            ["verify", "profiles", profile] => {
                out.verify_profiles
                    .entry((*profile).to_string())
                    .or_default()
                    .insert(key.to_string(), val);
            }
            ["ops"] => {
                if key == "verify_profile" {
                    out.verify_profile = Some(val);
                } else if key == "target_branch" {
                    out.target_branch = Some(val);
                } else if key == "promotion_mode" || key == "promotion" {
                    out.promotion_mode = Some(val);
                } else if key == "commit_policy" || key == "commit" {
                    out.commit_policy = Some(val);
                }
            }
            ["ops", "promote"] => {
                if key == "target_branch" {
                    out.target_branch = Some(val);
                } else if key == "mode" {
                    out.promotion_mode = Some(val);
                }
            }
            ["ops", "commit"] => {
                if key == "policy" {
                    out.commit_policy = Some(val);
                }
            }
            ["ops", "post_apply"] => {
                out.post_apply_commands.insert(key.to_string(), val);
            }
            ["ops", "forbid"] => {
                out.forbid_patterns.insert(key.to_string(), val);
            }
            ["ops", "editable_diffship"] => {
                out.editable_diffship_files.insert(key.to_string(), val);
            }
            ["workflow"] => {
                if key == "default_profile" || key == "profile" {
                    out.workflow_profile = Some(val);
                }
            }
            ["workflow", "strategy"] => {
                if key == "mode" {
                    out.workflow_strategy_mode = Some(val);
                } else if key == "default_profile" || key == "profile" {
                    out.workflow_strategy_default_profile = Some(val);
                }
            }
            ["workflow", "strategy", "error_overrides"] => {
                out.workflow_strategy_error_overrides
                    .insert(key.to_string(), val);
            }
            _ => {}
        }
    }

    out
}

fn unquote(s: &str) -> &str {
    let s = s.trim();
    if s.len() >= 2
        && ((s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')))
    {
        return &s[1..s.len() - 1];
    }
    s
}

fn profile_command_sort_key(key: &str) -> (u8, u32, String) {
    if let Some(rest) = key.strip_prefix("cmd")
        && let Ok(n) = rest.parse::<u32>()
    {
        return (0, n, key.to_string());
    }
    (1, 0, key.to_string())
}

#[cfg(test)]
mod tests {
    use super::{parse_config_toml, resolve_ops_config, resolve_workflow_config};
    use tempfile::TempDir;

    fn temp_git_root() -> TempDir {
        tempfile::tempdir().expect("tempdir")
    }

    #[test]
    fn parse_config_toml_reads_workflow_sections() {
        let cfg = parse_config_toml(
            r#"
[workflow]
default_profile = "bugfix-minimal"

[workflow.strategy]
mode = "prefer"
default_profile = "no-test-fast"

[workflow.strategy.error_overrides]
verify_test_failed = "regression-test-first"
verify_docs_failed = "docs-sync-minimal"
"#,
        );

        assert_eq!(cfg.workflow_profile.as_deref(), Some("bugfix-minimal"));
        assert_eq!(cfg.workflow_strategy_mode.as_deref(), Some("prefer"));
        assert_eq!(
            cfg.workflow_strategy_default_profile.as_deref(),
            Some("no-test-fast")
        );
        assert_eq!(
            cfg.workflow_strategy_error_overrides
                .get("verify_test_failed")
                .map(String::as_str),
            Some("regression-test-first")
        );
        assert_eq!(
            cfg.workflow_strategy_error_overrides
                .get("verify_docs_failed")
                .map(String::as_str),
            Some("docs-sync-minimal")
        );
    }

    #[test]
    fn resolve_ops_config_uses_project_workflow_schema_and_fallback_default() {
        let td = temp_git_root();
        let git_root = td.path();
        std::fs::create_dir_all(git_root.join(".diffship")).expect("create .diffship");
        std::fs::write(
            git_root.join(".diffship").join("ai_generated_config.toml"),
            r#"
[workflow]
default_profile = "prototype-speed"

[workflow.strategy]
mode = "suggest"
"#,
        )
        .expect("write ai config");
        std::fs::write(
            git_root.join(".diffship").join("config.toml"),
            r#"
[workflow]
default_profile = "cautious-tdd"

[workflow.strategy]
mode = "force"

[workflow.strategy.error_overrides]
verify_test_failed = "regression-test-first"
"#,
        )
        .expect("write project config");

        let cfg = resolve_ops_config(git_root, None, Default::default()).expect("resolve config");

        assert_eq!(cfg.workflow_profile, "cautious-tdd");
        assert_eq!(cfg.workflow_strategy_mode, "force");
        assert_eq!(
            cfg.workflow_strategy_default_profile
                .as_deref()
                .unwrap_or(&cfg.workflow_profile),
            "cautious-tdd"
        );
        assert_eq!(
            cfg.workflow_strategy_error_overrides
                .get("verify_test_failed")
                .map(String::as_str),
            Some("regression-test-first")
        );
    }

    #[test]
    fn resolve_workflow_config_reads_only_workflow_schema() {
        let td = temp_git_root();
        let git_root = td.path();
        std::fs::create_dir_all(git_root.join(".diffship")).expect("create .diffship");
        std::fs::write(
            git_root.join(".diffship").join("config.toml"),
            r#"
[workflow]
default_profile = "bugfix-minimal"

[workflow.strategy]
mode = "prefer"
default_profile = "no-test-fast"

[workflow.strategy.error_overrides]
verify_docs_failed = "docs-sync-minimal"
"#,
        )
        .expect("write project config");

        let cfg = resolve_workflow_config(git_root).expect("resolve workflow config");

        assert_eq!(cfg.default_profile, "bugfix-minimal");
        assert_eq!(cfg.strategy_mode, "prefer");
        assert_eq!(cfg.strategy_default_profile(), "no-test-fast");
        assert_eq!(
            cfg.strategy_error_overrides()
                .get("verify_docs_failed")
                .map(String::as_str),
            Some("docs-sync-minimal")
        );
    }
}

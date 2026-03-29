use crate::cli::BuildArgs;
use crate::exit::{EXIT_GENERAL, ExitError};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub const DEFAULT_PROFILE_NAME: &str = "20x512";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HandoffProfileDef {
    pub max_parts: usize,
    pub max_bytes_per_part: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedHandoffProfile {
    pub selected_name: String,
    pub display_name: String,
    pub max_parts: usize,
    pub max_bytes_per_part: u64,
}

#[derive(Debug, Clone)]
pub struct HandoffConfig {
    default_profile: String,
    default_output_dir: Option<String>,
    include_committed: bool,
    include_staged: bool,
    include_unstaged: bool,
    include_untracked: bool,
    split_by: String,
    untracked_mode: String,
    include_binary: bool,
    binary_mode: String,
    fail_on_secrets: bool,
    profiles: BTreeMap<String, HandoffProfileDef>,
}

#[derive(Debug, Clone, Default)]
struct HandoffConfigOverrides {
    default_profile: Option<String>,
    default_output_dir: Option<String>,
    include_committed: Option<bool>,
    include_staged: Option<bool>,
    include_unstaged: Option<bool>,
    include_untracked: Option<bool>,
    split_by: Option<String>,
    untracked_mode: Option<String>,
    include_binary: Option<bool>,
    binary_mode: Option<String>,
    fail_on_secrets: Option<bool>,
    profiles: BTreeMap<String, HandoffProfileOverride>,
}

#[derive(Debug, Clone, Default)]
struct HandoffProfileOverride {
    max_parts: Option<usize>,
    max_bytes_per_part: Option<u64>,
}

impl HandoffConfig {
    pub fn load(git_root: &Path) -> Result<Self, ExitError> {
        let mut cfg = Self::defaults();

        if let Some(path) = global_config_path()
            && path.is_file()
        {
            cfg.apply_overrides(load_config_file(&path)?);
        }

        for path in project_config_paths(git_root) {
            if path.is_file() {
                cfg.apply_overrides(load_config_file(&path)?);
            }
        }

        cfg.validate()?;
        Ok(cfg)
    }

    pub fn defaults() -> Self {
        let mut profiles = BTreeMap::new();
        profiles.insert(
            DEFAULT_PROFILE_NAME.to_string(),
            HandoffProfileDef {
                max_parts: 20,
                max_bytes_per_part: 512 * 1024 * 1024,
            },
        );
        profiles.insert(
            "10x100".to_string(),
            HandoffProfileDef {
                max_parts: 10,
                max_bytes_per_part: 100 * 1024 * 1024,
            },
        );
        Self {
            default_profile: DEFAULT_PROFILE_NAME.to_string(),
            default_output_dir: None,
            include_committed: true,
            include_staged: false,
            include_unstaged: false,
            include_untracked: false,
            split_by: "auto".to_string(),
            untracked_mode: "auto".to_string(),
            include_binary: false,
            binary_mode: "raw".to_string(),
            fail_on_secrets: false,
            profiles,
        }
    }

    pub fn available_profile_names(&self) -> Vec<String> {
        self.profiles.keys().cloned().collect()
    }

    pub fn resolve_selection(
        &self,
        requested_name: Option<&str>,
        max_parts_override: Option<usize>,
        max_bytes_override: Option<u64>,
    ) -> Result<ResolvedHandoffProfile, ExitError> {
        let selected_name = requested_name.unwrap_or(&self.default_profile);
        let Some(base) = self.profiles.get(selected_name) else {
            return Err(ExitError::new(
                EXIT_GENERAL,
                format!(
                    "unknown handoff profile: {} (expected one of: {})",
                    selected_name,
                    self.available_profile_names().join(", ")
                ),
            ));
        };

        let max_parts = max_parts_override.unwrap_or(base.max_parts);
        let max_bytes_per_part = max_bytes_override.unwrap_or(base.max_bytes_per_part);
        if max_parts == 0 {
            return Err(ExitError::new(EXIT_GENERAL, "--max-parts must be >= 1"));
        }
        if max_bytes_per_part == 0 {
            return Err(ExitError::new(
                EXIT_GENERAL,
                "--max-bytes-per-part must be >= 1",
            ));
        }

        let display_name =
            if max_parts == base.max_parts && max_bytes_per_part == base.max_bytes_per_part {
                selected_name.to_string()
            } else {
                format!("{selected_name}+override")
            };

        Ok(ResolvedHandoffProfile {
            selected_name: selected_name.to_string(),
            display_name,
            max_parts,
            max_bytes_per_part,
        })
    }

    pub fn resolve_build_args(
        &self,
        mut args: BuildArgs,
    ) -> Result<(BuildArgs, ResolvedHandoffProfile), ExitError> {
        if !args.no_committed && !self.include_committed {
            args.no_committed = true;
        }
        if !args.include_staged && self.include_staged {
            args.include_staged = true;
        }
        if !args.include_unstaged && self.include_unstaged {
            args.include_unstaged = true;
        }
        if !args.include_untracked && self.include_untracked {
            args.include_untracked = true;
        }
        if args.split_by.as_deref() == Some("auto") && self.split_by != "auto" {
            args.split_by = Some(self.split_by.clone());
        }
        if args.untracked_mode == "auto" && self.untracked_mode != "auto" {
            args.untracked_mode = self.untracked_mode.clone();
        }
        if !args.include_binary && self.include_binary {
            args.include_binary = true;
        }
        if args.binary_mode == "raw" && self.binary_mode != "raw" {
            args.binary_mode = self.binary_mode.clone();
        }
        if !args.fail_on_secrets && self.fail_on_secrets {
            args.fail_on_secrets = true;
        }
        let resolved = self.resolve_selection(
            args.profile.as_deref(),
            args.max_parts,
            args.max_bytes_per_part,
        )?;
        args.profile = Some(resolved.selected_name.clone());
        args.max_parts = Some(resolved.max_parts);
        args.max_bytes_per_part = Some(resolved.max_bytes_per_part);
        if args.out.is_none() && args.out_dir.is_none() {
            args.out_dir = self.default_output_dir.clone();
        }
        Ok((args, resolved))
    }

    fn apply_overrides(&mut self, overrides: HandoffConfigOverrides) {
        if let Some(name) = overrides.default_profile {
            self.default_profile = name;
        }
        if let Some(path) = overrides.default_output_dir {
            self.default_output_dir = Some(path);
        }
        if let Some(value) = overrides.include_committed {
            self.include_committed = value;
        }
        if let Some(value) = overrides.include_staged {
            self.include_staged = value;
        }
        if let Some(value) = overrides.include_unstaged {
            self.include_unstaged = value;
        }
        if let Some(value) = overrides.include_untracked {
            self.include_untracked = value;
        }
        if let Some(value) = overrides.split_by {
            self.split_by = value;
        }
        if let Some(value) = overrides.untracked_mode {
            self.untracked_mode = value;
        }
        if let Some(value) = overrides.include_binary {
            self.include_binary = value;
        }
        if let Some(value) = overrides.binary_mode {
            self.binary_mode = value;
        }
        if let Some(value) = overrides.fail_on_secrets {
            self.fail_on_secrets = value;
        }

        for (name, override_profile) in overrides.profiles {
            let entry = self.profiles.entry(name).or_insert(HandoffProfileDef {
                max_parts: 20,
                max_bytes_per_part: 512 * 1024 * 1024,
            });
            if let Some(max_parts) = override_profile.max_parts {
                entry.max_parts = max_parts;
            }
            if let Some(max_bytes_per_part) = override_profile.max_bytes_per_part {
                entry.max_bytes_per_part = max_bytes_per_part;
            }
        }
    }

    fn validate(&self) -> Result<(), ExitError> {
        if !self.profiles.contains_key(&self.default_profile) {
            return Err(ExitError::new(
                EXIT_GENERAL,
                format!(
                    "invalid handoff default profile: {} (expected one of: {})",
                    self.default_profile,
                    self.available_profile_names().join(", ")
                ),
            ));
        }
        if !matches!(self.split_by.as_str(), "auto" | "file" | "commit") {
            return Err(ExitError::new(
                EXIT_GENERAL,
                format!("invalid handoff split.by value: {}", self.split_by),
            ));
        }
        if !matches!(
            self.untracked_mode.as_str(),
            "auto" | "patch" | "raw" | "meta"
        ) {
            return Err(ExitError::new(
                EXIT_GENERAL,
                format!(
                    "invalid handoff untracked.mode value: {}",
                    self.untracked_mode
                ),
            ));
        }
        if !matches!(self.binary_mode.as_str(), "raw" | "patch" | "meta") {
            return Err(ExitError::new(
                EXIT_GENERAL,
                format!(
                    "invalid handoff diff.binary_mode value: {}",
                    self.binary_mode
                ),
            ));
        }
        Ok(())
    }
}

fn global_config_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join(".config/diffship/config.toml"))
}

fn project_config_paths(git_root: &Path) -> Vec<PathBuf> {
    vec![
        git_root.join(".diffship.toml"),
        git_root
            .join(".diffship")
            .join(crate::ops::config::AI_GENERATED_CONFIG_FILE_NAME),
        git_root.join(".diffship").join("config.toml"),
    ]
}

fn load_config_file(path: &Path) -> Result<HandoffConfigOverrides, ExitError> {
    let text = fs::read_to_string(path).map_err(|e| {
        ExitError::new(
            EXIT_GENERAL,
            format!("failed to read config {}: {e}", path.display()),
        )
    })?;
    Ok(parse_config_toml(&text))
}

fn parse_config_toml(s: &str) -> HandoffConfigOverrides {
    let mut out = HandoffConfigOverrides::default();
    let mut section: Vec<String> = vec![];

    for raw in s.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = match line.split_once('#') {
            Some((left, _)) => left.trim(),
            None => line,
        };
        if line.is_empty() {
            continue;
        }

        if line.starts_with('[') && line.ends_with(']') {
            let name = line.trim_start_matches('[').trim_end_matches(']').trim();
            section = name
                .split('.')
                .map(|part| unquote(part.trim()).to_string())
                .filter(|part| !part.is_empty())
                .collect();
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = unquote(value.trim()).to_string();
        let section_path = section.iter().map(|part| part.as_str()).collect::<Vec<_>>();

        match section_path.as_slice() {
            ["handoff"] => {
                if key == "default_profile" || key == "profile" {
                    out.default_profile = Some(value);
                } else if key == "output_dir" || key == "out_dir" {
                    out.default_output_dir = Some(value);
                }
            }
            ["sources"] => match key {
                "include_committed" => out.include_committed = parse_bool(&value),
                "include_staged" => out.include_staged = parse_bool(&value),
                "include_unstaged" => out.include_unstaged = parse_bool(&value),
                "include_untracked" => out.include_untracked = parse_bool(&value),
                _ => {}
            },
            ["split"] => {
                if key == "by" {
                    out.split_by = Some(value);
                }
            }
            ["untracked"] => {
                if key == "mode" {
                    out.untracked_mode = Some(value);
                }
            }
            ["diff"] => {
                if key == "include_binary" {
                    out.include_binary = parse_bool(&value);
                } else if key == "binary_mode" {
                    out.binary_mode = Some(value);
                }
            }
            ["secrets"] => {
                if key == "fail_on_secrets" {
                    out.fail_on_secrets = parse_bool(&value);
                }
            }
            ["handoff", "profiles", profile] | ["profiles", profile] => {
                let entry = out.profiles.entry((*profile).to_string()).or_default();
                if key == "max_parts" {
                    entry.max_parts = value.parse::<usize>().ok();
                } else if key == "max_bytes_per_part" {
                    entry.max_bytes_per_part = value.parse::<u64>().ok();
                }
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
        &s[1..s.len() - 1]
    } else {
        s
    }
}

fn parse_bool(s: &str) -> Option<bool> {
    match s.trim().to_ascii_lowercase().as_str() {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{HandoffConfig, parse_config_toml};
    use crate::cli::BuildArgs;

    #[test]
    fn parse_handoff_profiles_from_config_sections() {
        let cfg = parse_config_toml(
            r#"
[handoff]
default_profile = "team"
output_dir = "artifacts/handoffs"

[handoff.profiles."team"]
max_parts = 8
max_bytes_per_part = 1234

[profiles."legacy"]
max_parts = 4
max_bytes_per_part = 5678
"#,
        );

        assert_eq!(cfg.default_profile.as_deref(), Some("team"));
        assert_eq!(
            cfg.default_output_dir.as_deref(),
            Some("artifacts/handoffs")
        );
        assert_eq!(cfg.profiles.get("team").and_then(|p| p.max_parts), Some(8));
        assert_eq!(
            cfg.profiles
                .get("legacy")
                .and_then(|p| p.max_bytes_per_part),
            Some(5678)
        );
    }

    #[test]
    fn parse_handoff_defaults_from_additional_sections() {
        let cfg = parse_config_toml(
            r#"
[sources]
include_committed = false
include_staged = true
include_unstaged = true
include_untracked = true

[split]
by = "commit"

[untracked]
mode = "raw"

[diff]
include_binary = true
binary_mode = "meta"

[secrets]
fail_on_secrets = true
"#,
        );

        assert_eq!(cfg.include_committed, Some(false));
        assert_eq!(cfg.include_staged, Some(true));
        assert_eq!(cfg.include_unstaged, Some(true));
        assert_eq!(cfg.include_untracked, Some(true));
        assert_eq!(cfg.split_by.as_deref(), Some("commit"));
        assert_eq!(cfg.untracked_mode.as_deref(), Some("raw"));
        assert_eq!(cfg.include_binary, Some(true));
        assert_eq!(cfg.binary_mode.as_deref(), Some("meta"));
        assert_eq!(cfg.fail_on_secrets, Some(true));
    }

    #[test]
    fn resolve_selection_marks_overrides_in_display_name() {
        let cfg = HandoffConfig::defaults();
        let resolved = cfg
            .resolve_selection(Some("10x100"), Some(5), None)
            .unwrap();
        assert_eq!(resolved.selected_name, "10x100");
        assert_eq!(resolved.display_name, "10x100+override");
        assert_eq!(resolved.max_parts, 5);
        assert_eq!(resolved.max_bytes_per_part, 100 * 1024 * 1024);
    }

    #[test]
    fn resolve_build_args_applies_documented_defaults() {
        let mut cfg = HandoffConfig::defaults();
        cfg.apply_overrides(parse_config_toml(
            r#"
[sources]
include_committed = false
include_staged = true
include_untracked = true

[split]
by = "commit"

[untracked]
mode = "patch"

[diff]
include_binary = true
binary_mode = "meta"

[secrets]
fail_on_secrets = true
"#,
        ));

        let (args, _) = cfg
            .resolve_build_args(BuildArgs {
                range_mode: "last".to_string(),
                from: None,
                to: None,
                a: None,
                b: None,
                no_committed: false,
                include: vec![],
                exclude: vec![],
                include_staged: false,
                include_unstaged: false,
                include_untracked: false,
                split_by: Some("auto".to_string()),
                untracked_mode: "auto".to_string(),
                include_binary: false,
                binary_mode: "raw".to_string(),
                profile: None,
                max_parts: None,
                max_bytes_per_part: None,
                plan: None,
                plan_out: None,
                out_dir: None,
                out: None,
                zip: false,
                zip_only: false,
                project_context: "none".to_string(),
                yes: false,
                fail_on_secrets: false,
            })
            .unwrap();

        assert!(args.no_committed);
        assert!(args.include_staged);
        assert!(args.include_untracked);
        assert_eq!(args.split_by.as_deref(), Some("commit"));
        assert_eq!(args.untracked_mode, "patch");
        assert!(args.include_binary);
        assert_eq!(args.binary_mode, "meta");
        assert!(args.fail_on_secrets);
    }
}

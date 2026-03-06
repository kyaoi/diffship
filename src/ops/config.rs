use crate::exit::{EXIT_GENERAL, ExitError};
use crate::ops::patch_bundle::PatchBundleManifest;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// A partial ops configuration where every field is optional.
/// Later sources override earlier sources.
#[derive(Debug, Clone, Default)]
pub struct OpsConfigOverrides {
    pub verify_profile: Option<String>,
    pub verify_profiles: BTreeMap<String, BTreeMap<String, String>>,
    pub target_branch: Option<String>,
    pub promotion_mode: Option<String>,
    pub commit_policy: Option<String>,
}

#[derive(Debug, Clone)]
pub struct OpsConfig {
    pub verify_profile: String,
    pub target_branch: String,
    pub promotion_mode: String,
    pub commit_policy: String,
    verify_profiles: BTreeMap<String, BTreeMap<String, String>>,
}

impl OpsConfig {
    pub fn defaults() -> Self {
        Self {
            verify_profile: "standard".to_string(),
            target_branch: "develop".to_string(),
            promotion_mode: "commit".to_string(),
            commit_policy: "auto".to_string(),
            verify_profiles: BTreeMap::new(),
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
        for (profile, commands) in o.verify_profiles {
            self.verify_profiles.insert(profile, commands);
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
        // current: stored under the diffship directory (written by `diffship init`)
        git_root.join(".diffship").join("config.toml"),
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
        // - [ops.commit] policy
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

use crate::cli::StrategyArgs;
use crate::exit::{EXIT_GENERAL, ExitError};
use crate::ops::config::{self, WorkflowConfig};
use crate::ops::failure_category;
use crate::ops::run;
use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct StrategyResolution {
    pub failure_category: String,
    pub strategy_mode: String,
    pub default_profile: String,
    pub selected_profile: String,
    pub alternatives: Vec<String>,
    pub reason: String,
    pub tests_expected: Option<bool>,
    pub preferred_verify_profile: Option<String>,
}

#[derive(Debug, Clone)]
pub struct StrategyInput<'a> {
    pub failure_category: Option<&'a str>,
    pub strategy_mode: &'a str,
    pub default_profile: &'a str,
    pub error_overrides: &'a BTreeMap<String, String>,
}

#[derive(Debug, Clone, Copy)]
struct CategoryStrategy {
    profile: &'static str,
    structural: bool,
    reason: &'static str,
}

#[derive(Debug, Clone, Copy)]
struct StrategyProfileFacts {
    tests_expected: Option<bool>,
    preferred_verify_profile: Option<&'static str>,
}

#[derive(Debug, Deserialize)]
struct PhaseSummaryBrief {
    ok: Option<bool>,
    failure_category: Option<String>,
}

pub fn cmd(git_root: &Path, args: StrategyArgs) -> Result<(), ExitError> {
    let run_id = match args.run_id {
        Some(id) => id,
        None => run::latest_run_id(git_root)?.ok_or_else(|| {
            ExitError::new(
                EXIT_GENERAL,
                "no runs found (run diffship apply first, or pass --run-id)",
            )
        })?,
    };
    let run_dir = run::run_dir(git_root, &run_id);
    if !run_dir.exists() {
        return Err(ExitError::new(
            EXIT_GENERAL,
            format!("run not found: {}", run_id),
        ));
    }

    let failure_category = detect_failure_category(&run_dir).ok_or_else(|| {
        ExitError::new(
            EXIT_GENERAL,
            format!(
                "run {} has no failed phase with a normalized failure_category",
                run_id
            ),
        )
    })?;
    let cfg = config::resolve_workflow_config(git_root)?;
    let resolution = resolve_strategy_from_workflow(&cfg, Some(&failure_category)).ok_or_else(|| {
        ExitError::new(
            EXIT_GENERAL,
            format!(
                "workflow strategy resolution is disabled for run {} (workflow.strategy.mode=off)",
                run_id
            ),
        )
    })?;

    if args.json {
        let json = serde_json::to_string_pretty(&resolution).map_err(|e| {
            ExitError::new(EXIT_GENERAL, format!("failed to encode strategy JSON: {e}"))
        })?;
        println!("{json}");
        return Ok(());
    }

    println!("diffship strategy");
    println!("run_id: {run_id}");
    println!("failure_category: {}", resolution.failure_category);
    println!("strategy_mode: {}", resolution.strategy_mode);
    println!("selected_profile: {}", resolution.selected_profile);
    println!("default_profile: {}", resolution.default_profile);
    if resolution.alternatives.is_empty() {
        println!("alternatives: -");
    } else {
        println!("alternatives: {}", resolution.alternatives.join(", "));
    }
    if let Some(tests_expected) = resolution.tests_expected {
        println!("tests_expected: {tests_expected}");
    }
    if let Some(profile) = resolution.preferred_verify_profile.as_deref() {
        println!("preferred_verify_profile: {profile}");
    }
    println!("reason: {}", resolution.reason);
    Ok(())
}

pub(crate) fn resolve_for_run(
    git_root: &Path,
    run_dir: &Path,
) -> Result<Option<StrategyResolution>, ExitError> {
    let failure_category = detect_failure_category(run_dir);
    if failure_category.is_none() {
        return Ok(None);
    }
    let cfg = config::resolve_workflow_config(git_root)?;
    Ok(resolve_strategy_from_workflow(
        &cfg,
        failure_category.as_deref(),
    ))
}

pub fn resolve_strategy(input: StrategyInput<'_>) -> Option<StrategyResolution> {
    if input.strategy_mode == "off" {
        return None;
    }

    let failure_category = input
        .failure_category
        .unwrap_or(failure_category::VERIFY_FAILED)
        .to_string();
    let default_profile = input.default_profile.trim().to_string();
    let category_strategy = strategy_for_category(failure_category.as_str());
    let override_profile = input
        .error_overrides
        .get(failure_category.as_str())
        .cloned();

    let selected_profile = if let Some(profile) = override_profile.as_deref() {
        profile.to_string()
    } else {
        match input.strategy_mode {
            "force" | "prefer" if !category_strategy.map(|s| s.structural).unwrap_or(false) => {
                default_profile.clone()
            }
            _ => category_strategy
                .map(|s| s.profile.to_string())
                .unwrap_or_else(|| default_profile.clone()),
        }
    };

    let mut alternatives = Vec::new();
    if let Some(category_strategy) = category_strategy
        && category_strategy.profile != selected_profile
    {
        alternatives.push(category_strategy.profile.to_string());
    }
    if default_profile != selected_profile {
        alternatives.push(default_profile.clone());
    }
    alternatives.dedup();

    let reason = if let Some(profile) = override_profile.as_deref() {
        format!(
            "Per-error workflow override for `{}` selects `{}` before fallback defaults.",
            failure_category, profile
        )
    } else if let Some(category_strategy) = category_strategy {
        if category_strategy.structural {
            format!(
                "{} Repo-default speed preferences stay secondary for this category.",
                category_strategy.reason
            )
        } else {
            match input.strategy_mode {
                "prefer" => format!(
                    "Prefer mode keeps the repo default `{}` selected while retaining `{}` as a category-specific alternative.",
                    default_profile, category_strategy.profile
                ),
                "force" => format!(
                    "Force mode keeps the repo default `{}` selected for this category.",
                    default_profile
                ),
                _ => category_strategy.reason.to_string(),
            }
        }
    } else {
        format!(
            "No category-specific strategy is configured, so the repo default `{}` stays selected.",
            default_profile
        )
    };
    let profile_facts = strategy_profile_facts(selected_profile.as_str());

    Some(StrategyResolution {
        failure_category,
        strategy_mode: input.strategy_mode.to_string(),
        default_profile,
        selected_profile,
        alternatives,
        reason,
        tests_expected: profile_facts.and_then(|facts| facts.tests_expected),
        preferred_verify_profile: profile_facts
            .and_then(|facts| facts.preferred_verify_profile)
            .map(str::to_string),
    })
}

pub fn resolve_strategy_from_workflow(
    cfg: &WorkflowConfig,
    failure_category: Option<&str>,
) -> Option<StrategyResolution> {
    resolve_strategy(StrategyInput {
        failure_category,
        strategy_mode: &cfg.strategy_mode,
        default_profile: cfg.strategy_default_profile(),
        error_overrides: cfg.strategy_error_overrides(),
    })
}

pub(crate) fn detect_failure_category(run_dir: &Path) -> Option<String> {
    for name in ["promotion.json", "verify.json", "apply.json"] {
        let Some(summary) = read_phase_summary(&run_dir.join(name)) else {
            continue;
        };
        if summary.ok == Some(false)
            && let Some(category) = summary.failure_category
            && !category.trim().is_empty()
        {
            return Some(category);
        }
    }
    None
}

fn strategy_for_category(category: &str) -> Option<CategoryStrategy> {
    match category {
        failure_category::PATCH_APPLY_FAILED => Some(CategoryStrategy {
            profile: "patch-repair-only",
            structural: true,
            reason: "Patch apply failures should be repaired at the patch shape / path level before normal bugfix iteration.",
        }),
        failure_category::BASE_COMMIT_MISMATCH => Some(CategoryStrategy {
            profile: "base-realign-first",
            structural: true,
            reason: "Base commit mismatches should be resolved by realigning the patch to the current base before normal code edits.",
        }),
        failure_category::POST_APPLY_FAILED => Some(CategoryStrategy {
            profile: "bugfix-minimal",
            structural: false,
            reason: "Post-apply failures usually need a small corrective fix around the local normalization step.",
        }),
        failure_category::VERIFY_TEST_FAILED => Some(CategoryStrategy {
            profile: "regression-test-first",
            structural: false,
            reason: "Test failures usually benefit from a regression-oriented fix strategy first.",
        }),
        failure_category::VERIFY_LINT_FAILED => Some(CategoryStrategy {
            profile: "bugfix-minimal",
            structural: false,
            reason: "Lint-like failures usually benefit from a small policy-preserving fix before broader changes.",
        }),
        failure_category::VERIFY_DOCS_FAILED => Some(CategoryStrategy {
            profile: "docs-sync-minimal",
            structural: false,
            reason: "Docs and traceability failures usually need a focused documentation sync before broader edits.",
        }),
        failure_category::PROMOTION_BLOCKED_SECRETS | failure_category::PROMOTION_BLOCKED_TASKS => {
            Some(CategoryStrategy {
                profile: "policy-review-first",
                structural: true,
                reason: "Policy-blocked promotions should be reviewed and acknowledged before normal implementation strategy resumes.",
            })
        }
        failure_category::PROMOTION_FAILED => Some(CategoryStrategy {
            profile: "promotion-repair-first",
            structural: true,
            reason: "Promotion failures should be repaired at the cherry-pick / target-branch layer before normal bugfix iteration.",
        }),
        _ => None,
    }
}

fn strategy_profile_facts(profile: &str) -> Option<StrategyProfileFacts> {
    match profile {
        "balanced" => Some(StrategyProfileFacts {
            tests_expected: Some(true),
            preferred_verify_profile: Some("standard"),
        }),
        "cautious-tdd" | "regression-test-first" => Some(StrategyProfileFacts {
            tests_expected: Some(true),
            preferred_verify_profile: Some("standard"),
        }),
        "prototype-speed" => Some(StrategyProfileFacts {
            tests_expected: Some(false),
            preferred_verify_profile: Some("fast"),
        }),
        "bugfix-minimal" => Some(StrategyProfileFacts {
            tests_expected: Some(true),
            preferred_verify_profile: Some("standard"),
        }),
        "no-test-fast" => Some(StrategyProfileFacts {
            tests_expected: Some(false),
            preferred_verify_profile: Some("fast"),
        }),
        "docs-sync-minimal"
        | "patch-repair-only"
        | "base-realign-first"
        | "policy-review-first"
        | "promotion-repair-first" => Some(StrategyProfileFacts {
            tests_expected: Some(false),
            preferred_verify_profile: Some("fast"),
        }),
        _ => None,
    }
}

fn read_phase_summary(path: &Path) -> Option<PhaseSummaryBrief> {
    let bytes = std::fs::read(path).ok()?;
    serde_json::from_slice::<PhaseSummaryBrief>(&bytes).ok()
}

#[cfg(test)]
mod tests {
    use super::{StrategyInput, resolve_strategy};
    use std::collections::BTreeMap;

    #[test]
    fn suggest_mode_prefers_category_guidance_for_behavioral_failures() {
        let overrides = BTreeMap::new();
        let resolution = resolve_strategy(StrategyInput {
            failure_category: Some("verify_test_failed"),
            strategy_mode: "suggest",
            default_profile: "balanced",
            error_overrides: &overrides,
        })
        .expect("resolution");

        assert_eq!(resolution.selected_profile, "regression-test-first");
        assert_eq!(resolution.alternatives, vec!["balanced".to_string()]);
    }

    #[test]
    fn prefer_mode_biases_toward_repo_default_for_behavioral_failures() {
        let overrides = BTreeMap::new();
        let resolution = resolve_strategy(StrategyInput {
            failure_category: Some("verify_test_failed"),
            strategy_mode: "prefer",
            default_profile: "no-test-fast",
            error_overrides: &overrides,
        })
        .expect("resolution");

        assert_eq!(resolution.selected_profile, "no-test-fast");
        assert_eq!(
            resolution.alternatives,
            vec!["regression-test-first".to_string()]
        );
        assert_eq!(resolution.tests_expected, Some(false));
        assert_eq!(resolution.preferred_verify_profile.as_deref(), Some("fast"));
    }

    #[test]
    fn force_mode_keeps_structural_failures_on_category_specific_strategy() {
        let overrides = BTreeMap::new();
        let resolution = resolve_strategy(StrategyInput {
            failure_category: Some("patch_apply_failed"),
            strategy_mode: "force",
            default_profile: "no-test-fast",
            error_overrides: &overrides,
        })
        .expect("resolution");

        assert_eq!(resolution.selected_profile, "patch-repair-only");
        assert_eq!(resolution.alternatives, vec!["no-test-fast".to_string()]);
    }

    #[test]
    fn override_wins_over_mode_bias() {
        let mut overrides = BTreeMap::new();
        overrides.insert(
            "verify_test_failed".to_string(),
            "custom-test-repair".to_string(),
        );
        let resolution = resolve_strategy(StrategyInput {
            failure_category: Some("verify_test_failed"),
            strategy_mode: "force",
            default_profile: "no-test-fast",
            error_overrides: &overrides,
        })
        .expect("resolution");

        assert_eq!(resolution.selected_profile, "custom-test-repair");
        assert_eq!(
            resolution.alternatives,
            vec![
                "regression-test-first".to_string(),
                "no-test-fast".to_string()
            ]
        );
    }

    #[test]
    fn off_mode_disables_resolution() {
        let overrides = BTreeMap::new();
        assert!(
            resolve_strategy(StrategyInput {
                failure_category: Some("verify_test_failed"),
                strategy_mode: "off",
                default_profile: "balanced",
                error_overrides: &overrides,
            })
            .is_none()
        );
    }
}

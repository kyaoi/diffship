use crate::ops::failure_category;
use crate::ops::worktree;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct DerivedRunState {
    pub(crate) state_label: Option<String>,
    pub(crate) failure_category: Option<String>,
    pub(crate) next_command: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PhaseSummaryLite {
    ok: Option<bool>,
    failure_category: Option<String>,
}

pub(crate) fn derive(git_root: &Path, run_id: &str, run_dir: &Path) -> DerivedRunState {
    let sandbox_meta = worktree::read_sandbox_meta(git_root, run_id);
    let stale_sandbox = sandbox_meta
        .as_ref()
        .is_some_and(|meta| !Path::new(&meta.path).exists());
    derive_from_artifacts(
        run_id,
        stale_sandbox,
        sandbox_meta.is_some(),
        read_phase_summary(run_dir.join("promotion.json")),
        read_phase_summary(run_dir.join("verify.json")),
        read_phase_summary(run_dir.join("apply.json")),
    )
}

fn derive_from_artifacts(
    run_id: &str,
    stale_sandbox: bool,
    has_sandbox_meta: bool,
    promotion: Option<PhaseSummaryLite>,
    verify: Option<PhaseSummaryLite>,
    apply: Option<PhaseSummaryLite>,
) -> DerivedRunState {
    if stale_sandbox {
        return DerivedRunState {
            state_label: Some("stale_sandbox".to_string()),
            failure_category: None,
            next_command: Some("diffship doctor".to_string()),
        };
    }

    if let Some(summary) = promotion {
        if summary.ok == Some(true) {
            return DerivedRunState {
                state_label: Some("cleanup_safe".to_string()),
                failure_category: None,
                next_command: Some("diffship cleanup --include-runs".to_string()),
            };
        }
        if summary.ok == Some(false) {
            let next_command = match summary.failure_category.as_deref() {
                Some(category) if category == failure_category::PROMOTION_BLOCKED_TASKS => {
                    Some(format!("diffship promote --run-id {run_id} --ack-tasks"))
                }
                Some(category) if category == failure_category::PROMOTION_BLOCKED_SECRETS => {
                    Some(format!("diffship promote --run-id {run_id} --ack-secrets"))
                }
                _ => Some(format!("diffship promote --run-id {run_id}")),
            };
            let label = match summary.failure_category.as_deref() {
                Some(category) if category == failure_category::PROMOTION_BLOCKED_TASKS => {
                    "blocked_by_tasks"
                }
                Some(category) if category == failure_category::PROMOTION_BLOCKED_SECRETS => {
                    "blocked_by_secrets"
                }
                _ => "recoverable",
            };
            return DerivedRunState {
                state_label: Some(label.to_string()),
                failure_category: summary.failure_category,
                next_command,
            };
        }
    }

    if let Some(summary) = verify {
        if summary.ok == Some(true) {
            return DerivedRunState {
                state_label: Some("ready_to_promote".to_string()),
                failure_category: None,
                next_command: Some(format!("diffship promote --run-id {run_id}")),
            };
        }
        if summary.ok == Some(false) {
            return DerivedRunState {
                state_label: Some("recoverable".to_string()),
                failure_category: summary.failure_category,
                next_command: Some(format!("diffship strategy --run-id {run_id}")),
            };
        }
    }

    if let Some(summary) = apply {
        if summary.ok == Some(true) {
            return DerivedRunState {
                state_label: Some("ready_to_verify".to_string()),
                failure_category: None,
                next_command: Some(format!("diffship verify --run-id {run_id}")),
            };
        }
        if summary.ok == Some(false) {
            return DerivedRunState {
                state_label: Some("recoverable".to_string()),
                failure_category: summary.failure_category,
                next_command: Some(format!("diffship strategy --run-id {run_id}")),
            };
        }
    }

    if has_sandbox_meta {
        return DerivedRunState {
            state_label: Some("active".to_string()),
            failure_category: None,
            next_command: None,
        };
    }

    DerivedRunState {
        state_label: None,
        failure_category: None,
        next_command: None,
    }
}

fn read_phase_summary(path: PathBuf) -> Option<PhaseSummaryLite> {
    let bytes = fs::read(path).ok()?;
    serde_json::from_slice::<PhaseSummaryLite>(&bytes).ok()
}

#[cfg(test)]
mod tests {
    use super::{DerivedRunState, PhaseSummaryLite, derive_from_artifacts};
    use crate::ops::failure_category;

    #[test]
    fn derive_marks_stale_sandbox_before_phase_state() {
        let state = derive_from_artifacts(
            "run_1",
            true,
            true,
            Some(PhaseSummaryLite {
                ok: Some(true),
                failure_category: None,
            }),
            None,
            None,
        );
        assert_eq!(
            state,
            DerivedRunState {
                state_label: Some("stale_sandbox".to_string()),
                failure_category: None,
                next_command: Some("diffship doctor".to_string()),
            }
        );
    }

    #[test]
    fn derive_marks_blocked_promotions_with_ack_hint() {
        let tasks = derive_from_artifacts(
            "run_1",
            false,
            false,
            Some(PhaseSummaryLite {
                ok: Some(false),
                failure_category: Some(failure_category::PROMOTION_BLOCKED_TASKS.to_string()),
            }),
            None,
            None,
        );
        assert_eq!(tasks.state_label.as_deref(), Some("blocked_by_tasks"));
        assert_eq!(
            tasks.next_command.as_deref(),
            Some("diffship promote --run-id run_1 --ack-tasks")
        );

        let secrets = derive_from_artifacts(
            "run_2",
            false,
            false,
            Some(PhaseSummaryLite {
                ok: Some(false),
                failure_category: Some(failure_category::PROMOTION_BLOCKED_SECRETS.to_string()),
            }),
            None,
            None,
        );
        assert_eq!(secrets.state_label.as_deref(), Some("blocked_by_secrets"));
        assert_eq!(
            secrets.next_command.as_deref(),
            Some("diffship promote --run-id run_2 --ack-secrets")
        );
    }

    #[test]
    fn derive_prefers_verify_and_apply_transitions() {
        let verify_ok = derive_from_artifacts(
            "run_1",
            false,
            false,
            None,
            Some(PhaseSummaryLite {
                ok: Some(true),
                failure_category: None,
            }),
            None,
        );
        assert_eq!(verify_ok.state_label.as_deref(), Some("ready_to_promote"));
        assert_eq!(
            verify_ok.next_command.as_deref(),
            Some("diffship promote --run-id run_1")
        );

        let apply_ok = derive_from_artifacts(
            "run_1",
            false,
            false,
            None,
            None,
            Some(PhaseSummaryLite {
                ok: Some(true),
                failure_category: None,
            }),
        );
        assert_eq!(apply_ok.state_label.as_deref(), Some("ready_to_verify"));
        assert_eq!(
            apply_ok.next_command.as_deref(),
            Some("diffship verify --run-id run_1")
        );
    }

    #[test]
    fn derive_marks_active_when_only_sandbox_exists() {
        let state = derive_from_artifacts("run_1", false, true, None, None, None);
        assert_eq!(
            state,
            DerivedRunState {
                state_label: Some("active".to_string()),
                failure_category: None,
                next_command: None,
            }
        );
    }
}

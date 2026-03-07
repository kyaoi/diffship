# Traceability (Spec ↔ Tests ↔ Code)

This document maps spec requirement IDs to tests and implementation modules.

> Enforced by `scripts/check-traceability.sh` (via `just trace-check` / `just ci`).

Conventions:
- Until implementation exists, use `TBD` placeholders.
- Every `S-...` ID in `docs/SPEC_V1.md` must appear here.
- Planned entries may point to **intended** module/test paths; only `Implemented` implies they exist and are enforced by tests.
- Maintain a per-requirement `Status` field:
  - `Planned`: not implemented yet (typically `TBD` remains)
  - `Partial`: partially implemented (tests or code exists, but not both)
  - `Implemented`: implemented and covered by tests
  - `N/A`: explicitly out of scope

---

## Goals / non-goals

- **S-GOAL-001** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs`, `src/cli.rs` — Status: Implemented
- **S-GOAL-002** — Tests: TBD — Code: TBD — Status: Planned
- **S-GOAL-003** — Tests: TBD — Code: TBD — Status: Planned
- **S-GOAL-004** — Tests: `tests/m6_handoff_determinism.rs` — Code: `src/handoff.rs` — Status: Implemented
- **S-GOAL-005** — Tests: `src/plan.rs`, `src/tui/mod.rs`, `tests/m6_handoff_build.rs` — Code: `src/plan.rs`, `src/tui/mod.rs`, `src/handoff.rs`, `src/cli.rs` — Status: Implemented
- **S-GOAL-006** — Tests: TBD — Code: `src/ops/*` — Status: Planned
- **S-GOAL-007** — Tests: TBD — Code: `src/ops/*` — Status: Planned
- **S-GOAL-008** — Tests: TBD — Code: `src/ops/session.rs`, `src/ops/worktree.rs` — Status: Planned
- **S-GOAL-009** — Tests: TBD — Code: `src/ops/config.rs`, `src/ops/promote.rs` — Status: Planned
- **S-GOAL-010** — Tests: `tests/m0_integration.rs` — Code: `src/ops/init.rs` — Status: Implemented

- **S-NONGOAL-001** — Tests: N/A — Code: N/A — Status: N/A
- **S-NONGOAL-002** — Tests: N/A — Code: N/A — Status: N/A
- **S-NONGOAL-003** — Tests: N/A — Code: N/A — Status: N/A
- **S-NONGOAL-004** — Tests: N/A — Code: N/A — Status: N/A

---

## TUI

- **S-TUI-001** — Tests: `tests/m5_tui_cli_parity.rs` — Code: `src/tui/mod.rs`, `src/ops/mod.rs` — Status: Implemented
- **S-TUI-002** — Tests: `src/tui/mod.rs` — Code: `src/tui/mod.rs`, `src/plan.rs` — Status: Implemented
- **S-TUI-003** — Tests: `src/tui/mod.rs` — Code: `src/tui/mod.rs` — Status: Implemented
- **S-TUI-004** — Tests: `src/plan.rs`, `src/tui/mod.rs`, `tests/m6_handoff_build.rs` — Code: `src/plan.rs`, `src/tui/mod.rs`, `src/handoff.rs`, `src/cli.rs` — Status: Implemented
- **S-TUI-005** — Tests: `src/tui/mod.rs` — Code: `src/tui/mod.rs` — Status: Implemented

---

## Sources

- **S-SOURCES-001** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs`, `src/cli.rs` — Status: Implemented
- **S-SOURCES-002** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs`, `src/cli.rs` — Status: Implemented
- **S-SOURCES-003** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs` — Status: Implemented

---

## Range modes

- **S-RANGE-001** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs`, `src/cli.rs` — Status: Implemented
- **S-RANGE-002** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs`, `src/cli.rs` — Status: Implemented
- **S-RANGE-003** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs`, `src/cli.rs` — Status: Implemented

---

## CLI path handling

- **S-PATH-001** — Tests: `src/pathing.rs`, `tests/m2_apply_verify.rs`, `tests/m2_pack_fix.rs`, `tests/m6_preview.rs`, `tests/m6_compare.rs`, `tests/m6_handoff_build.rs` — Code: `src/pathing.rs`, `src/handoff.rs`, `src/preview.rs`, `src/bundle_compare.rs`, `src/ops/apply.rs`, `src/ops/pack_fix.rs` — Status: Implemented

---

## Filters

- **S-FILTER-001** — Tests: `tests/m6_handoff_build.rs`, `src/filter.rs` — Code: `src/filter.rs`, `src/handoff.rs`, `src/cli.rs` — Status: Implemented
- **S-FILTER-002** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs` — Status: Implemented
- **S-FILTER-003** — Tests: `tests/m6_handoff_build.rs` — Code: `src/filter.rs`, `src/handoff.rs`, `src/cli.rs` — Status: Implemented

---

## Untracked

- **S-UNTRACKED-001** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs`, `src/cli.rs` — Status: Implemented
- **S-UNTRACKED-002** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs`, `src/cli.rs` — Status: Implemented
- **S-UNTRACKED-003** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs` — Status: Implemented
- **S-UNTRACKED-004** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs` — Status: Implemented
- **S-UNTRACKED-005** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs` — Status: Implemented

---

## Binary

- **S-BINARY-001** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs`, `src/cli.rs` — Status: Implemented
- **S-BINARY-002** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs`, `src/cli.rs` — Status: Implemented
- **S-BINARY-003** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs` — Status: Implemented

---

## Split

- **S-SPLIT-001** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs`, `src/cli.rs` — Status: Implemented
- **S-SPLIT-002** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs` — Status: Implemented
- **S-SPLIT-003** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs` — Status: Implemented

---

## Profiles

- **S-PROFILE-001** — Tests: `tests/m6_handoff_build.rs`, `src/plan.rs`, `src/tui/mod.rs` — Code: `src/handoff.rs`, `src/handoff_config.rs`, `src/cli.rs`, `src/plan.rs`, `src/tui/mod.rs` — Status: Implemented
- **S-PROFILE-002** — Tests: `tests/m6_handoff_build.rs`, `src/handoff_config.rs` — Code: `src/handoff_config.rs`, `src/handoff.rs`, `src/ops/init.rs` — Status: Implemented

---

## Output

- **S-OUT-001** — Tests: `tests/m6_handoff_build.rs`, `src/handoff.rs` — Code: `src/handoff.rs`, `src/handoff_config.rs` — Status: Implemented
- **S-OUT-002** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs` — Status: Implemented
- **S-OUT-003** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs` — Status: Implemented
- **S-OUT-004** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs` — Status: Implemented

---

## Packing / fallback

- **S-PACK-001** — Tests: `tests/m6_handoff_determinism.rs` — Code: `src/handoff.rs` — Status: Implemented
- **S-PACK-002** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs` — Status: Implemented
- **S-PACK-003** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs` — Status: Implemented
- **S-PACK-004** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs` — Status: Implemented
- **S-PACK-005** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs` — Status: Implemented

---

## Plan export / replay

- **S-PLAN-001** — Tests: `tests/m6_handoff_build.rs` — Code: `src/plan.rs`, `src/handoff.rs`, `src/cli.rs` — Status: Implemented
- **S-PLAN-002** — Tests: `tests/m6_handoff_build.rs`, `src/plan.rs` — Code: `src/plan.rs`, `src/handoff.rs`, `src/cli.rs` — Status: Implemented

---

## Handoff

- **S-HANDOFF-001** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs` — Status: Implemented
- **S-HANDOFF-002** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs` — Status: Implemented
- **S-HANDOFF-003** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs` — Status: Implemented
- **S-HANDOFF-004** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs` — Status: Implemented

---

## Preview

- **S-PREVIEW-001** — Tests: `tests/m6_preview.rs` — Code: `src/preview.rs`, `src/cli.rs` — Status: Implemented
- **S-PREVIEW-002** — Tests: `tests/m6_preview.rs` — Code: `src/preview.rs`, `src/cli.rs` — Status: Implemented

---

## Compare

- **S-COMPARE-001** — Tests: `tests/m6_compare.rs` — Code: `src/bundle_compare.rs`, `src/cli.rs` — Status: Implemented
- **S-COMPARE-002** — Tests: `tests/m6_compare.rs` — Code: `src/bundle_compare.rs`, `src/cli.rs` — Status: Implemented
- **S-COMPARE-003** — Tests: `tests/m6_compare.rs` — Code: `src/bundle_compare.rs`, `src/cli.rs` — Status: Implemented
- **S-COMPARE-004** — Tests: `tests/m6_compare.rs` — Code: `src/bundle_compare.rs`, `src/cli.rs` — Status: Implemented

---

## Patch bundle (input contract)

- **S-PBUNDLE-001** — Tests: `tests/m2_apply_verify.rs` — Code: `src/ops/patch_bundle.rs` — Status: Implemented
- **S-PBUNDLE-002** — Tests: `tests/m2_apply_verify.rs` — Code: `src/ops/patch_bundle.rs` — Status: Implemented
- **S-PBUNDLE-003** — Tests: `tests/m2_apply_verify.rs` — Code: `src/ops/patch_bundle.rs` — Status: Implemented
- **S-PBUNDLE-004** — Tests: `tests/m2_apply_verify.rs` — Code: `src/ops/patch_bundle.rs` — Status: Implemented
- **S-PBUNDLE-005** — Tests: `tests/m2_apply_verify.rs` — Code: `src/ops/patch_bundle.rs` — Status: Implemented
- **S-PBUNDLE-006** — Tests: `tests/m3_tasks.rs` — Code: `src/ops/patch_bundle.rs`, `src/ops/tasks.rs` — Status: Implemented

---

## Apply

- **S-APPLY-001** — Tests: `tests/m2_apply_verify.rs` — Code: `src/ops/apply.rs` — Status: Implemented
- **S-APPLY-002** — Tests: `tests/m2_apply_verify.rs` — Code: `src/ops/apply.rs` — Status: Implemented
- **S-APPLY-003** — Tests: `tests/m2_apply_verify.rs` — Code: `src/ops/apply.rs` — Status: Implemented
- **S-APPLY-004** — Tests: `tests/m2_apply_verify.rs` — Code: `src/ops/apply.rs` — Status: Implemented
- **S-APPLY-005** — Tests: `tests/m2_apply_verify.rs` — Code: `src/ops/apply.rs` — Status: Implemented
- **S-APPLY-006** — Tests: `tests/m2_apply_verify.rs` — Code: `src/ops/apply.rs` — Status: Implemented
- **S-APPLY-007** — Tests: `tests/m2_apply_verify.rs` — Code: `src/ops/apply.rs` — Status: Implemented
- **S-APPLY-008** — Tests: `tests/m2_apply_verify.rs` — Code: `src/ops/run.rs` — Status: Implemented
- **S-APPLY-009** — Tests: `tests/m2_apply_verify.rs`, `tests/m2_promotion_loop.rs` — Code: `src/ops/apply.rs`, `src/ops/post_apply.rs`, `src/ops/config.rs` — Status: Implemented

---

## Commit policy

- **S-COMMIT-001** — Tests: `tests/m2_promotion_loop.rs` — Code: `src/ops/promote.rs` — Status: Implemented
- **S-COMMIT-002** — Tests: `tests/m2_promotion_loop.rs` — Code: `src/ops/promote.rs` — Status: Implemented
- **S-COMMIT-003** — Tests: `tests/m4_config_precedence.rs` — Code: `src/ops/config.rs` — Status: Implemented
- **S-COMMIT-004** — Tests: TBD — Code: `src/ops/promote.rs` — Status: Partial
- **S-COMMIT-005** — Tests: `tests/m2_promotion_loop.rs` — Code: `src/ops/promote.rs` — Status: Implemented

---

## Verify

- **S-VERIFY-001** — Tests: `tests/m2_apply_verify.rs`, `tests/m4_verify_profiles.rs` — Code: `src/ops/verify.rs`, `src/ops/config.rs` — Status: Implemented
- **S-VERIFY-002** — Tests: `tests/m2_apply_verify.rs` — Code: `src/ops/verify.rs` — Status: Implemented
- **S-VERIFY-003** — Tests: `tests/m2_apply_verify.rs` — Code: `src/ops/verify.rs` — Status: Implemented
- **S-VERIFY-004** — Tests: `tests/m2_apply_verify.rs` — Code: `src/ops/verify.rs` — Status: Implemented

---

## pack-fix

- **S-PACKFIX-001** — Tests: `tests/m2_pack_fix.rs` — Code: `src/ops/pack_fix.rs` — Status: Implemented
- **S-PACKFIX-002** — Tests: `tests/m2_pack_fix.rs` — Code: `src/ops/pack_fix.rs` — Status: Implemented

---

## loop

- **S-LOOP-001** — Tests: `tests/m2_promotion_loop.rs` — Code: `src/ops/loop_cmd.rs` — Status: Implemented
- **S-LOOP-002** — Tests: `tests/m2_promotion_loop.rs` — Code: `src/ops/loop_cmd.rs` — Status: Implemented

---

## status

- **S-STATUS-001** — Tests: `tests/m0_integration.rs` — Code: `src/ops/status.rs` — Status: Implemented
- **S-STATUS-002** — Tests: `tests/m0_integration.rs` — Code: `src/ops/status.rs` — Status: Implemented

---

## Secrets (handoff build)

- **S-SECRETS-001** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs` — Status: Implemented
- **S-SECRETS-002** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs` — Status: Implemented
- **S-SECRETS-003** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs`, `src/cli.rs` — Status: Implemented

---

## Ops safety policy

## Sessions

- **S-SESSION-001** — Tests: `tests/m1_worktrees.rs` — Code: `src/ops/session.rs` — Status: Implemented
- **S-SESSION-002** — Tests: `tests/m1_worktrees.rs` — Code: `src/ops/session.rs` — Status: Implemented
- **S-SESSION-003** — Tests: `tests/m1_worktrees.rs` — Code: `src/ops/worktree.rs` — Status: Implemented
- **S-SESSION-004** — Tests: `tests/m1_worktrees.rs` — Code: `src/ops/session.rs` — Status: Implemented

---

## Promotion

- **S-PROMOTE-001** — Tests: `tests/m2_promotion_loop.rs`, `tests/m4_02_promotion_switch.rs` — Code: `src/ops/promote.rs` — Status: Implemented
- **S-PROMOTE-002** — Tests: `tests/m2_promotion_loop.rs` — Code: `src/ops/promote.rs` — Status: Implemented
- **S-PROMOTE-003** — Tests: `tests/m2_promotion_loop.rs` — Code: `src/ops/promote.rs` — Status: Implemented

---

## Ops secrets & user tasks

- **S-OPS-SECRETS-001** — Tests: `tests/m2_promotion_loop.rs` — Code: `src/ops/secrets.rs`, `src/ops/promote.rs` — Status: Implemented
- **S-OPS-SECRETS-002** — Tests: `tests/m2_promotion_loop.rs` — Code: `src/ops/secrets.rs` — Status: Implemented
- **S-OPS-TASKS-001** — Tests: `tests/m3_tasks.rs` — Code: `src/ops/tasks.rs`, `src/ops/patch_bundle.rs`, `src/ops/apply.rs`, `src/ops/promote.rs` — Status: Implemented

---

- **S-OPS-001** — Tests: `tests/m0_integration.rs` — Code: `src/ops/lock.rs` — Status: Implemented
- **S-OPS-002** — Tests: `tests/m0_integration.rs` — Code: `src/ops/lock.rs` — Status: Implemented
- **S-OPS-003** — Tests: `tests/m2_apply_verify.rs` — Code: `src/ops/patch_bundle.rs` — Status: Implemented
- **S-OPS-004** — Tests: `tests/m2_apply_verify.rs` — Code: `src/ops/patch_bundle.rs` — Status: Implemented
- **S-OPS-005** — Tests: `tests/m2_apply_verify.rs` — Code: `src/ops/patch_bundle.rs` — Status: Implemented
- **S-OPS-006** — Tests: `tests/m4_config_precedence.rs` — Code: `src/ops/config.rs`, `src/ops/verify.rs`, `src/ops/loop_cmd.rs`, `src/ops/promote.rs`, `src/ops/patch_bundle.rs`, `src/cli.rs` — Status: Implemented


---

## Runs & logs

- **S-RUN-001** — Tests: `tests/m0_integration.rs` — Code: `src/ops/run.rs` — Status: Implemented
- **S-RUN-002** — Tests: `tests/m2_apply_verify.rs` — Code: `src/ops/run.rs` — Status: Implemented
- **S-RUN-003** — Tests: `tests/m2_pack_fix.rs` — Code: `src/ops/pack_fix.rs` — Status: Implemented

---

## Init (Project kit)

- **S-INIT-001** — Tests: `tests/m0_integration.rs` — Code: `src/ops/init.rs` — Status: Implemented
- **S-INIT-002** — Tests: `tests/m0_integration.rs` — Code: `src/ops/init.rs` — Status: Implemented
- **S-INIT-003** — Tests: `tests/m0_integration.rs` — Code: `src/ops/init.rs` — Status: Implemented
- **S-INIT-004** — Tests: `tests/m0_integration.rs` — Code: `src/ops/init.rs` — Status: Implemented

---

## Exit codes

- **S-EXIT-000** — Tests: `tests/m0_integration.rs` — Code: `src/exit.rs` — Status: Implemented
- **S-EXIT-001** — Tests: `tests/m0_integration.rs` — Code: `src/exit.rs` — Status: Implemented
- **S-EXIT-002** — Tests: `tests/m0_integration.rs` — Code: `src/exit.rs` — Status: Implemented
- **S-EXIT-003** — Tests: `tests/m6_handoff_build.rs` — Code: `src/exit.rs`, `src/handoff.rs` — Status: Implemented
- **S-EXIT-004** — Tests: `tests/m6_handoff_build.rs` — Code: `src/exit.rs`, `src/handoff.rs` — Status: Implemented
- **S-EXIT-005** — Tests: `tests/m2_apply_verify.rs` — Code: `src/exit.rs` — Status: Implemented
- **S-EXIT-006** — Tests: `tests/m2_apply_verify.rs` — Code: `src/exit.rs` — Status: Implemented
- **S-EXIT-007** — Tests: `tests/m2_apply_verify.rs` — Code: `src/exit.rs` — Status: Implemented
- **S-EXIT-008** — Tests: `tests/m2_apply_verify.rs` — Code: `src/exit.rs` — Status: Implemented
- **S-EXIT-009** — Tests: `tests/m2_apply_verify.rs` — Code: `src/exit.rs` — Status: Implemented
- **S-EXIT-010** — Tests: `tests/m0_integration.rs` — Code: `src/exit.rs` — Status: Implemented
- **S-EXIT-011** — Tests: `tests/m2_promotion_loop.rs` — Code: `src/exit.rs` — Status: Implemented
- **S-EXIT-012** — Tests: `tests/m3_tasks.rs` — Code: `src/exit.rs`, `src/ops/promote.rs` — Status: Implemented
- **S-EXIT-013** — Tests: `tests/m2_promotion_loop.rs` — Code: `src/exit.rs`, `src/ops/promote.rs` — Status: Implemented

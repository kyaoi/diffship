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
- **S-TUI-006** — Tests: `src/tui/mod.rs` — Code: `src/tui/mod.rs` — Status: Implemented
- **S-TUI-007** — Tests: `src/tui/mod.rs` — Code: `src/tui/mod.rs` — Status: Implemented
- **S-TUI-008** — Tests: `src/tui/mod.rs` — Code: `src/tui/mod.rs` — Status: Implemented

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
- **S-OUT-005** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs`, `src/cli.rs`, `src/plan.rs`, `src/tui/mod.rs` — Status: Implemented
- **S-OUT-006** — Tests: `tests/m6_handoff_build.rs`, `tests/m6_handoff_determinism.rs` — Code: `src/handoff.rs`, `src/bundle_compare.rs` — Status: Implemented
- **S-OUT-007** — Tests: `tests/m6_handoff_build.rs`, `tests/m6_handoff_determinism.rs`, `tests/m6_compare.rs` — Code: `src/handoff.rs`, `src/bundle_compare.rs` — Status: Implemented
- **S-OUT-008** — Tests: `tests/m6_handoff_build.rs`, `tests/m6_handoff_determinism.rs`, `tests/m6_compare.rs` — Code: `src/handoff.rs`, `src/bundle_compare.rs` — Status: Implemented
- **S-OUT-009** — Tests: `tests/m6_handoff_build.rs`, `tests/m6_handoff_determinism.rs` — Code: `src/handoff.rs` — Status: Implemented
- **S-OUT-010** — Tests: `tests/m6_handoff_build.rs`, `tests/m6_handoff_determinism.rs` — Code: `src/handoff.rs` — Status: Implemented
- **S-OUT-011** — Tests: `src/handoff.rs`, `tests/m6_handoff_build.rs` — Code: `src/handoff.rs` — Status: Implemented
- **S-OUT-012** — Tests: `src/handoff.rs`, `tests/m6_handoff_build.rs` — Code: `src/handoff.rs` — Status: Implemented
- **S-OUT-013** — Tests: `src/handoff.rs`, `tests/m6_handoff_build.rs` — Code: `src/handoff.rs` — Status: Implemented
- **S-OUT-014** — Tests: `src/handoff.rs`, `tests/m6_handoff_build.rs` — Code: `src/handoff.rs` — Status: Implemented
- **S-OUT-015** — Tests: `src/handoff.rs`, `tests/m6_handoff_build.rs` — Code: `src/handoff.rs` — Status: Implemented
- **S-OUT-016** — Tests: `src/handoff.rs`, `tests/m6_handoff_build.rs` — Code: `src/handoff.rs` — Status: Implemented
- **S-OUT-017** — Tests: `tests/m6_handoff_build.rs`, `tests/m6_preview.rs` — Code: `src/handoff.rs`, `src/preview.rs`, `src/cli.rs`, `src/plan.rs` — Status: Implemented
- **S-OUT-018** — Tests: `tests/m6_handoff_build.rs`, `tests/m6_preview.rs` — Code: `src/handoff.rs`, `src/preview.rs` — Status: Implemented
- **S-OUT-019** — Tests: `src/handoff.rs`, `tests/m6_handoff_build.rs` — Code: `src/handoff.rs` — Status: Implemented
- **S-OUT-020** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs` — Status: Implemented
- **S-OUT-021** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs` — Status: Implemented
- **S-OUT-022** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs` — Status: Implemented
- **S-OUT-023** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs` — Status: Implemented
- **S-OUT-024** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs` — Status: Implemented
- **S-OUT-025** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs` — Status: Implemented
- **S-OUT-026** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs` — Status: Implemented
- **S-OUT-027** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs` — Status: Implemented
- **S-OUT-028** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs` — Status: Implemented
- **S-OUT-029** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs` — Status: Implemented
- **S-OUT-030** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs` — Status: Implemented
- **S-OUT-031** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs` — Status: Implemented
- **S-OUT-032** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs` — Status: Implemented
- **S-OUT-033** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs` — Status: Implemented
- **S-OUT-034** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs` — Status: Implemented
- **S-OUT-035** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs` — Status: Implemented
- **S-OUT-036** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs` — Status: Implemented
- **S-OUT-037** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs` — Status: Implemented
- **S-OUT-038** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs` — Status: Implemented
- **S-OUT-039** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs` — Status: Implemented
- **S-OUT-040** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs` — Status: Implemented
- **S-OUT-041** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs` — Status: Implemented
- **S-OUT-042** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs` — Status: Implemented
- **S-OUT-043** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs` — Status: Implemented
- **S-OUT-044** — Tests: `tests/m6_handoff_build.rs`, `src/handoff.rs` unit tests — Code: `src/handoff.rs` — Status: Implemented
- **S-OUT-045** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs` — Status: Implemented
- **S-OUT-046** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs` — Status: Implemented
- **S-OUT-047** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs`, `src/ops/config.rs` — Status: Implemented
- **S-OUT-048** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs` — Status: Implemented

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
- **S-PREVIEW-003** — Tests: `tests/m6_preview.rs` — Code: `src/preview.rs` — Status: Implemented
- **S-PREVIEW-004** — Tests: `tests/m6_preview.rs` — Code: `src/preview.rs` — Status: Implemented
- **S-PREVIEW-005** — Tests: `tests/m6_preview.rs` — Code: `src/preview.rs` — Status: Implemented
- **S-PREVIEW-006** — Tests: `tests/m6_preview.rs` — Code: `src/preview.rs` — Status: Implemented
- **S-PREVIEW-007** — Tests: `tests/m6_preview.rs` — Code: `src/preview.rs` — Status: Implemented
- **S-PREVIEW-008** — Tests: `tests/m6_preview.rs` — Code: `src/preview.rs` — Status: Implemented
- **S-PREVIEW-009** — Tests: `tests/m6_preview.rs` — Code: `src/preview.rs` — Status: Implemented
- **S-PREVIEW-010** — Tests: `tests/m6_preview.rs` — Code: `src/preview.rs` — Status: Implemented

---

## Compare

- **S-COMPARE-001** — Tests: `tests/m6_compare.rs` — Code: `src/bundle_compare.rs`, `src/cli.rs` — Status: Implemented
- **S-COMPARE-002** — Tests: `tests/m6_compare.rs` — Code: `src/bundle_compare.rs`, `src/cli.rs` — Status: Implemented
- **S-COMPARE-003** — Tests: `tests/m6_compare.rs` — Code: `src/bundle_compare.rs`, `src/cli.rs` — Status: Implemented
- **S-COMPARE-004** — Tests: `tests/m6_compare.rs` — Code: `src/bundle_compare.rs`, `src/cli.rs` — Status: Implemented
- **S-COMPARE-005** — Tests: `tests/m6_compare.rs` — Code: `src/bundle_compare.rs` — Status: Implemented
- **S-COMPARE-006** — Tests: `tests/m6_compare.rs` — Code: `src/bundle_compare.rs` — Status: Implemented

---

## Patch bundle (input contract)

- **S-PBUNDLE-001** — Tests: `tests/m2_apply_verify.rs` — Code: `src/ops/patch_bundle.rs` — Status: Implemented
- **S-PBUNDLE-002** — Tests: `tests/m2_apply_verify.rs` — Code: `src/ops/patch_bundle.rs` — Status: Implemented
- **S-PBUNDLE-003** — Tests: `tests/m2_apply_verify.rs` — Code: `src/ops/patch_bundle.rs` — Status: Implemented
- **S-PBUNDLE-004** — Tests: `tests/m2_apply_verify.rs` — Code: `src/ops/patch_bundle.rs` — Status: Implemented
- **S-PBUNDLE-005** — Tests: `tests/m2_apply_verify.rs` — Code: `src/ops/patch_bundle.rs` — Status: Implemented
- **S-PBUNDLE-006** — Tests: `tests/m3_tasks.rs` — Code: `src/ops/patch_bundle.rs`, `src/ops/tasks.rs` — Status: Implemented
- **S-PBUNDLE-007** — Tests: `tests/m2_apply_verify.rs` — Code: `src/ops/patch_bundle.rs` — Status: Implemented

---

## Apply

- **S-APPLY-001** — Tests: `tests/m2_apply_verify.rs` — Code: `src/ops/apply.rs` — Status: Implemented
- **S-APPLY-002** — Tests: `tests/m2_apply_verify.rs` — Code: `src/ops/apply.rs` — Status: Implemented
- **S-APPLY-003** — Tests: `tests/m2_apply_verify.rs` — Code: `src/ops/apply.rs` — Status: Implemented
- **S-APPLY-004** — Tests: `tests/m2_apply_verify.rs` — Code: `src/ops/apply.rs` — Status: Implemented
- **S-APPLY-010** — Tests: `tests/m2_apply_verify.rs`, `tests/m2_promotion_loop.rs` — Code: `src/ops/apply.rs`, `src/ops/loop_cmd.rs`, `src/ops/patch_bundle.rs`, `src/cli.rs`, `src/main.rs` — Status: Implemented
- **S-APPLY-005** — Tests: `tests/m2_apply_verify.rs` — Code: `src/ops/apply.rs` — Status: Implemented
- **S-APPLY-006** — Tests: `tests/m2_apply_verify.rs` — Code: `src/ops/apply.rs` — Status: Implemented
- **S-APPLY-007** — Tests: `tests/m2_apply_verify.rs` — Code: `src/ops/apply.rs` — Status: Implemented
- **S-APPLY-008** — Tests: `tests/m2_apply_verify.rs` — Code: `src/ops/run.rs` — Status: Implemented
- **S-APPLY-009** — Tests: `tests/m2_apply_verify.rs`, `tests/m2_promotion_loop.rs` — Code: `src/ops/apply.rs`, `src/ops/post_apply.rs`, `src/ops/config.rs` — Status: Implemented
- **S-APPLY-011** — Tests: `tests/m2_apply_verify.rs` — Code: `src/ops/post_apply.rs` — Status: Implemented
- **S-APPLY-012** — Tests: `tests/m2_apply_verify.rs` — Code: `src/cli.rs`, `src/main.rs`, `src/ops/apply.rs` — Status: Implemented
- **S-APPLY-013** — Tests: `tests/m2_apply_verify.rs`, `tests/m2_promotion_loop.rs` — Code: `src/ops/apply.rs`, `src/ops/pack_fix.rs` — Status: Implemented

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
- **S-PACKFIX-003** — Tests: `tests/m2_pack_fix.rs` — Code: `src/ops/pack_fix.rs` — Status: Implemented
- **S-PACKFIX-004** — Tests: `tests/m2_pack_fix.rs` — Code: `src/ops/pack_fix.rs` — Status: Implemented
- **S-PACKFIX-005** — Tests: `tests/m2_pack_fix.rs`, `src/ops/strategy.rs` — Code: `src/ops/pack_fix.rs`, `src/ops/strategy.rs`, `src/ops/config.rs` — Status: Implemented
- **S-PACKFIX-006** — Tests: `tests/m2_pack_fix.rs` — Code: `src/ops/pack_fix.rs`, `src/ops/strategy.rs` — Status: Implemented
- **S-PACKFIX-007** — Tests: `tests/m2_pack_fix.rs` — Code: `src/ops/pack_fix.rs`, `src/ops/strategy.rs` — Status: Implemented
- **S-PACKFIX-008** — Tests: `tests/m2_pack_fix.rs`, `tests/m6_handoff_determinism.rs` — Code: `src/ops/pack_fix.rs`, `src/ops/strategy.rs`, `src/handoff.rs` — Status: Implemented

---

## loop

- **S-LOOP-001** — Tests: `tests/m2_promotion_loop.rs` — Code: `src/ops/loop_cmd.rs` — Status: Implemented
- **S-LOOP-002** — Tests: `tests/m2_promotion_loop.rs` — Code: `src/ops/loop_cmd.rs` — Status: Implemented
- **S-LOOP-003** — Tests: `tests/m2_promotion_loop.rs` — Code: `src/cli.rs`, `src/main.rs`, `src/ops/loop_cmd.rs`, `src/ops/apply.rs` — Status: Implemented

---

## status

- **S-STATUS-001** — Tests: `tests/m0_integration.rs` — Code: `src/ops/status.rs` — Status: Implemented
- **S-STATUS-002** — Tests: `tests/m0_integration.rs` — Code: `src/ops/status.rs` — Status: Implemented
- **S-STATUS-003** — Tests: `tests/m0_integration.rs`, `tests/m1_worktrees.rs` — Code: `src/ops/status.rs` — Status: Implemented
- **S-STATUS-004** — Tests: `tests/m0_integration.rs` — Code: `src/ops/runs.rs`, `src/ops/run.rs` — Status: Implemented

---

## Session repair / doctor

- **S-SESSION-005** — Tests: `tests/m7_ops_recovery.rs` — Code: `src/ops/session.rs`, `src/cli.rs`, `src/ops/mod.rs` — Status: Implemented
- **S-SESSION-006** — Tests: `tests/m7_ops_recovery.rs` — Code: `src/ops/session.rs`, `src/ops/worktree.rs` — Status: Implemented
- **S-DOCTOR-001** — Tests: `tests/m7_ops_recovery.rs` — Code: `src/ops/doctor.rs`, `src/cli.rs`, `src/ops/mod.rs` — Status: Implemented
- **S-DOCTOR-002** — Tests: `tests/m7_ops_recovery.rs` — Code: `src/ops/doctor.rs`, `src/ops/session.rs`, `src/ops/worktree.rs` — Status: Implemented

---

## cleanup

- **S-CLEANUP-001** — Tests: `tests/m7_cleanup.rs` — Code: `src/ops/cleanup.rs`, `src/cli.rs`, `src/ops/mod.rs` — Status: Implemented
- **S-CLEANUP-002** — Tests: `tests/m7_cleanup.rs` — Code: `src/ops/cleanup.rs`, `src/ops/worktree.rs`, `src/ops/run.rs` — Status: Implemented
- **S-CLEANUP-003** — Tests: `tests/m7_cleanup.rs` — Code: `src/ops/cleanup.rs`, `src/cli.rs` — Status: Implemented
- **S-CLEANUP-004** — Tests: `tests/m7_cleanup.rs` — Code: `src/ops/cleanup.rs`, `src/ops/run.rs`, `src/ops/session.rs`, `src/ops/worktree.rs` — Status: Implemented
- **S-CLEANUP-005** — Tests: `tests/m7_cleanup.rs` — Code: `src/ops/cleanup.rs`, `src/cli.rs` — Status: Implemented

---

## Secrets (handoff build)

- **S-SECRETS-001** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs` — Status: Implemented
- **S-SECRETS-002** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs` — Status: Implemented
- **S-SECRETS-003** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs`, `src/cli.rs` — Status: Implemented

---

## Ops safety policy

- **S-OPS-007** — Tests: `tests/m2_apply_verify.rs`, `tests/m0_integration.rs` — Code: `src/ops/config.rs`, `src/ops/patch_bundle.rs`, `src/ops/init.rs` — Status: Implemented
- **S-OPS-008** — Tests: `tests/m0_integration.rs`, `tests/m2_apply_verify.rs` — Code: `src/ops/config.rs`, `src/ops/init.rs` — Status: Implemented
- **S-OPS-009** — Tests: `tests/m2_apply_verify.rs`, `tests/m0_integration.rs` — Code: `src/ops/config.rs`, `src/ops/patch_bundle.rs`, `src/ops/init.rs` — Status: Implemented

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
- **S-RUN-004** — Tests: `tests/m0_integration.rs`, `tests/m2_apply_verify.rs`, `tests/m2_promotion_loop.rs` — Code: `src/ops/run.rs`, `src/ops/status.rs`, `src/ops/runs.rs` — Status: Implemented
- **S-RUN-005** — Tests: `tests/m0_integration.rs`, `src/ops/run.rs` — Code: `src/ops/run.rs` — Status: Implemented
- **S-RUN-006** — Tests: `tests/m2_apply_verify.rs`, `tests/m2_promotion_loop.rs` — Code: `src/ops/command_log.rs`, `src/ops/apply.rs`, `src/ops/post_apply.rs`, `src/ops/verify.rs`, `src/ops/promote.rs` — Status: Implemented
- **S-RUN-007** — Tests: `tests/m0_integration.rs`, `tests/m2_apply_verify.rs` — Code: `src/ops/run.rs`, `src/ops/status.rs`, `src/ops/runs.rs` — Status: Implemented
- **S-RUN-008** — Tests: `tests/m2_apply_verify.rs`, `tests/m2_pack_fix.rs`, `tests/m2_promotion_loop.rs`, `tests/m3_tasks.rs` — Code: `src/ops/apply.rs`, `src/ops/verify.rs`, `src/ops/promote.rs`, `src/ops/failure_category.rs` — Status: Implemented
- **S-RUN-009** — Tests: `tests/m2_pack_fix.rs`, `src/ops/failure_category.rs` — Code: `src/ops/verify.rs`, `src/ops/failure_category.rs` — Status: Implemented

---

## Workflow / strategy config

- **S-WORKFLOW-001** — Tests: `src/ops/config.rs` — Code: `src/ops/config.rs` — Status: Implemented
- **S-WORKFLOW-002** — Tests: `src/ops/config.rs` — Code: `src/ops/config.rs` — Status: Implemented
- **S-WORKFLOW-003** — Tests: `src/ops/config.rs` — Code: `src/ops/config.rs` — Status: Implemented
- **S-WORKFLOW-004** — Tests: `src/ops/strategy.rs`, `tests/m2_pack_fix.rs` — Code: `src/ops/strategy.rs`, `src/ops/pack_fix.rs` — Status: Implemented

---

## Init (Project kit)

- **S-INIT-001** — Tests: `tests/m0_integration.rs` — Code: `src/ops/init.rs` — Status: Implemented
- **S-INIT-002** — Tests: `tests/m0_integration.rs` — Code: `src/ops/init.rs` — Status: Implemented
- **S-INIT-003** — Tests: `tests/m0_integration.rs` — Code: `src/ops/init.rs` — Status: Implemented
- **S-INIT-004** — Tests: `tests/m0_integration.rs` — Code: `src/ops/init.rs` — Status: Implemented
- **S-INIT-005** — Tests: `tests/m0_integration.rs` — Code: `src/ops/init.rs`, `src/cli.rs` — Status: Implemented
- **S-INIT-006** — Tests: `tests/m0_integration.rs` — Code: `src/ops/init.rs` — Status: Implemented
- **S-INIT-007** — Tests: `tests/m0_integration.rs` — Code: `src/ops/init.rs`, `src/cli.rs` — Status: Implemented
- **S-INIT-008** — Tests: `tests/m0_integration.rs` — Code: `src/ops/init.rs` — Status: Implemented
- **S-INIT-009** — Tests: `tests/m0_integration.rs` — Code: `src/ops/init.rs`, `src/cli.rs` — Status: Implemented
- **S-INIT-010** — Tests: `tests/m0_integration.rs` — Code: `src/ops/init.rs`, `src/cli.rs` — Status: Implemented
- **S-INIT-011** — Tests: `tests/m0_integration.rs` — Code: `src/ops/init.rs` — Status: Implemented
- **S-INIT-012** — Tests: `tests/m0_integration.rs`, `tests/m4_config_precedence.rs`, `tests/m6_handoff_build.rs` — Code: `src/ops/init.rs`, `src/ops/config.rs`, `src/handoff_config.rs` — Status: Implemented
- **S-INIT-013** — Tests: `tests/m0_integration.rs` — Code: `src/ops/init.rs`, `src/cli.rs` — Status: Implemented

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

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

- **S-GOAL-001** — Tests: TBD — Code: TBD — Status: Planned
- **S-GOAL-002** — Tests: TBD — Code: TBD — Status: Planned
- **S-GOAL-003** — Tests: TBD — Code: TBD — Status: Planned
- **S-GOAL-004** — Tests: `tests/m6_handoff_determinism.rs` — Code: `src/handoff.rs` — Status: Implemented
- **S-GOAL-005** — Tests: TBD — Code: TBD — Status: Planned
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

- **S-TUI-001** — Tests: TBD — Code: `src/tui/*` — Status: Partial
- **S-TUI-002** — Tests: TBD — Code: `src/tui/*` — Status: Planned
- **S-TUI-003** — Tests: TBD — Code: `src/tui/viewer.rs` — Status: Planned
- **S-TUI-004** — Tests: TBD — Code: `src/plan.rs`, `src/tui/*` — Status: Planned

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

## Filters

- **S-FILTER-001** — Tests: TBD — Code: `src/filter.rs` — Status: Planned
- **S-FILTER-002** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs` — Status: Implemented
- **S-FILTER-003** — Tests: TBD — Code: `src/filter.rs` — Status: Planned

---

## Untracked

- **S-UNTRACKED-001** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs`, `src/cli.rs` — Status: Implemented
- **S-UNTRACKED-002** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs`, `src/cli.rs` — Status: Implemented
- **S-UNTRACKED-003** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs` — Status: Implemented
- **S-UNTRACKED-004** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs` — Status: Implemented
- **S-UNTRACKED-005** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs` — Status: Implemented

---

## Binary

- **S-BINARY-001** — Tests: TBD — Code: `src/binary.rs` — Status: Planned
- **S-BINARY-002** — Tests: TBD — Code: `src/binary.rs` — Status: Planned
- **S-BINARY-003** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs` — Status: Implemented

---

## Split

- **S-SPLIT-001** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs`, `src/cli.rs` — Status: Implemented
- **S-SPLIT-002** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs` — Status: Implemented
- **S-SPLIT-003** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs` — Status: Implemented

---

## Output

- **S-OUT-001** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs` — Status: Implemented
- **S-OUT-002** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs` — Status: Implemented
- **S-OUT-003** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs` — Status: Implemented
- **S-OUT-004** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs` — Status: Implemented

---

## Packing / fallback

- **S-PACK-001** — Tests: `tests/m6_handoff_determinism.rs` — Code: `src/handoff.rs` — Status: Implemented
- **S-PACK-002** — Tests: TBD — Code: `src/pack.rs` — Status: Planned
- **S-PACK-003** — Tests: TBD — Code: `src/pack.rs` — Status: Planned
- **S-PACK-004** — Tests: TBD — Code: `src/pack.rs` — Status: Planned
- **S-PACK-005** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs` — Status: Implemented

---

## Handoff

- **S-HANDOFF-001** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs` — Status: Implemented
- **S-HANDOFF-002** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs` — Status: Implemented
- **S-HANDOFF-003** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs` — Status: Implemented
- **S-HANDOFF-004** — Tests: `tests/m6_handoff_build.rs` — Code: `src/handoff.rs` — Status: Implemented

---

## Preview

- **S-PREVIEW-001** — Tests: TBD — Code: `src/preview.rs` — Status: Planned

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

---

## Commit policy

- **S-COMMIT-001** — Tests: `tests/m2_promotion_loop.rs` — Code: `src/ops/promote.rs` — Status: Implemented
- **S-COMMIT-002** — Tests: `tests/m2_promotion_loop.rs` — Code: `src/ops/promote.rs` — Status: Implemented
- **S-COMMIT-003** — Tests: `tests/m4_config_precedence.rs` — Code: `src/ops/config.rs` — Status: Implemented
- **S-COMMIT-004** — Tests: TBD — Code: `src/ops/promote.rs` — Status: Partial
- **S-COMMIT-005** — Tests: `tests/m2_promotion_loop.rs` — Code: `src/ops/promote.rs` — Status: Implemented

---

## Verify

- **S-VERIFY-001** — Tests: `tests/m2_apply_verify.rs` — Code: `src/ops/verify.rs` — Status: Implemented
- **S-VERIFY-002** — Tests: `tests/m2_apply_verify.rs` — Code: `src/ops/verify.rs` — Status: Implemented
- **S-VERIFY-003** — Tests: `tests/m2_apply_verify.rs` — Code: `src/ops/verify.rs` — Status: Implemented
- **S-VERIFY-004** — Tests: `tests/m2_apply_verify.rs` — Code: `src/ops/verify.rs` — Status: Implemented

---

## pack-fix

- **S-PACKFIX-001** — Tests: TBD — Code: `src/ops/pack_fix.rs` — Status: Partial
- **S-PACKFIX-002** — Tests: TBD — Code: `src/ops/pack_fix.rs` — Status: Partial

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
- **S-RUN-003** — Tests: TBD — Code: `src/ops/pack_fix.rs` — Status: Partial

---

## Init (Project kit)

- **S-INIT-001** — Tests: `tests/m0_integration.rs` — Code: `src/ops/init.rs` — Status: Implemented
- **S-INIT-002** — Tests: `tests/m0_integration.rs` — Code: `src/ops/init.rs` — Status: Implemented
- **S-INIT-003** — Tests: `tests/m0_integration.rs` — Code: `src/ops/init.rs` — Status: Implemented

---

## Exit codes

- **S-EXIT-000** — Tests: `tests/m0_integration.rs` — Code: `src/exit.rs` — Status: Implemented
- **S-EXIT-001** — Tests: `tests/m0_integration.rs` — Code: `src/exit.rs` — Status: Implemented
- **S-EXIT-002** — Tests: `tests/m0_integration.rs` — Code: `src/exit.rs` — Status: Implemented
- **S-EXIT-003** — Tests: TBD — Code: `src/exit.rs` — Status: Planned
- **S-EXIT-004** — Tests: TBD — Code: `src/exit.rs` — Status: Planned
- **S-EXIT-005** — Tests: `tests/m2_apply_verify.rs` — Code: `src/exit.rs` — Status: Implemented
- **S-EXIT-006** — Tests: `tests/m2_apply_verify.rs` — Code: `src/exit.rs` — Status: Implemented
- **S-EXIT-007** — Tests: `tests/m2_apply_verify.rs` — Code: `src/exit.rs` — Status: Implemented
- **S-EXIT-008** — Tests: `tests/m2_apply_verify.rs` — Code: `src/exit.rs` — Status: Implemented
- **S-EXIT-009** — Tests: `tests/m2_apply_verify.rs` — Code: `src/exit.rs` — Status: Implemented
- **S-EXIT-010** — Tests: `tests/m0_integration.rs` — Code: `src/exit.rs` — Status: Implemented
- **S-EXIT-011** — Tests: `tests/m2_promotion_loop.rs` — Code: `src/exit.rs` — Status: Implemented
- **S-EXIT-012** — Tests: `tests/m3_tasks.rs` — Code: `src/exit.rs`, `src/ops/promote.rs` — Status: Implemented
- **S-EXIT-013** — Tests: `tests/m2_promotion_loop.rs` — Code: `src/exit.rs`, `src/ops/promote.rs` — Status: Implemented

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
- **S-GOAL-004** — Tests: TBD — Code: TBD — Status: Planned
- **S-GOAL-005** — Tests: TBD — Code: TBD — Status: Planned
- **S-GOAL-006** — Tests: TBD — Code: `src/ops/*` — Status: Planned
- **S-GOAL-007** — Tests: TBD — Code: `src/ops/*` — Status: Planned
- **S-GOAL-008** — Tests: TBD — Code: `src/ops/session.rs`, `src/ops/worktree.rs` — Status: Planned
- **S-GOAL-009** — Tests: TBD — Code: `src/ops/commit.rs`, `src/config.rs` — Status: Planned
- **S-GOAL-010** — Tests: TBD — Code: `src/ops/init.rs` — Status: Planned

- **S-NONGOAL-001** — Tests: N/A — Code: N/A — Status: N/A
- **S-NONGOAL-002** — Tests: N/A — Code: N/A — Status: N/A
- **S-NONGOAL-003** — Tests: N/A — Code: N/A — Status: N/A
- **S-NONGOAL-004** — Tests: N/A — Code: N/A — Status: N/A

---

## TUI

- **S-TUI-001** — Tests: TBD — Code: `src/tui/*` — Status: Planned
- **S-TUI-002** — Tests: TBD — Code: `src/tui/*` — Status: Planned
- **S-TUI-003** — Tests: TBD — Code: `src/tui/viewer.rs` — Status: Planned
- **S-TUI-004** — Tests: TBD — Code: `src/plan.rs`, `src/tui/*` — Status: Planned

---

## Sources

- **S-SOURCES-001** — Tests: TBD — Code: `src/sources.rs` — Status: Planned
- **S-SOURCES-002** — Tests: TBD — Code: `src/sources.rs` — Status: Planned
- **S-SOURCES-003** — Tests: TBD — Code: `src/handoff.rs` — Status: Planned

---

## Range modes

- **S-RANGE-001** — Tests: TBD — Code: `src/range.rs` — Status: Planned
- **S-RANGE-002** — Tests: TBD — Code: `src/cli.rs` — Status: Planned
- **S-RANGE-003** — Tests: TBD — Code: `src/cli.rs` — Status: Planned

---

## Filters

- **S-FILTER-001** — Tests: TBD — Code: `src/filter.rs` — Status: Planned
- **S-FILTER-002** — Tests: TBD — Code: `src/filter.rs` — Status: Planned
- **S-FILTER-003** — Tests: TBD — Code: `src/filter.rs` — Status: Planned

---

## Untracked

- **S-UNTRACKED-001** — Tests: TBD — Code: `src/untracked.rs` — Status: Planned
- **S-UNTRACKED-002** — Tests: TBD — Code: `src/untracked.rs` — Status: Planned
- **S-UNTRACKED-003** — Tests: TBD — Code: `src/untracked.rs` — Status: Planned
- **S-UNTRACKED-004** — Tests: TBD — Code: `src/untracked.rs` — Status: Planned
- **S-UNTRACKED-005** — Tests: TBD — Code: `src/bundle.rs` — Status: Planned

---

## Binary

- **S-BINARY-001** — Tests: TBD — Code: `src/binary.rs` — Status: Planned
- **S-BINARY-002** — Tests: TBD — Code: `src/binary.rs` — Status: Planned
- **S-BINARY-003** — Tests: TBD — Code: `src/bundle.rs` — Status: Planned

---

## Split

- **S-SPLIT-001** — Tests: TBD — Code: `src/split.rs` — Status: Planned
- **S-SPLIT-002** — Tests: TBD — Code: `src/split.rs` — Status: Planned
- **S-SPLIT-003** — Tests: TBD — Code: `src/split.rs` — Status: Planned

---

## Output

- **S-OUT-001** — Tests: TBD — Code: `src/bundle.rs` — Status: Planned
- **S-OUT-002** — Tests: TBD — Code: `src/bundle.rs` — Status: Planned
- **S-OUT-003** — Tests: TBD — Code: `src/bundle.rs` — Status: Planned
- **S-OUT-004** — Tests: TBD — Code: `src/handoff.rs` — Status: Planned

---

## Packing / fallback

- **S-PACK-001** — Tests: TBD — Code: `src/pack.rs` — Status: Planned
- **S-PACK-002** — Tests: TBD — Code: `src/pack.rs` — Status: Planned
- **S-PACK-003** — Tests: TBD — Code: `src/pack.rs` — Status: Planned
- **S-PACK-004** — Tests: TBD — Code: `src/pack.rs` — Status: Planned
- **S-PACK-005** — Tests: TBD — Code: `src/bundle.rs` — Status: Planned

---

## Handoff

- **S-HANDOFF-001** — Tests: TBD — Code: `src/handoff.rs` — Status: Planned
- **S-HANDOFF-002** — Tests: TBD — Code: `src/handoff.rs` — Status: Planned
- **S-HANDOFF-003** — Tests: TBD — Code: `src/handoff.rs` — Status: Planned
- **S-HANDOFF-004** — Tests: TBD — Code: `src/handoff.rs` — Status: Planned

---

## Preview

- **S-PREVIEW-001** — Tests: TBD — Code: `src/preview.rs` — Status: Planned

---

## Patch bundle (input contract)

- **S-PBUNDLE-001** — Tests: TBD — Code: `src/ops/patch_bundle.rs` — Status: Planned
- **S-PBUNDLE-002** — Tests: TBD — Code: `src/ops/patch_bundle.rs` — Status: Planned
- **S-PBUNDLE-003** — Tests: TBD — Code: `src/ops/patch_bundle.rs` — Status: Planned
- **S-PBUNDLE-004** — Tests: TBD — Code: `src/ops/patch_bundle.rs` — Status: Planned
- **S-PBUNDLE-005** — Tests: TBD — Code: `src/ops/patch_bundle.rs` — Status: Planned
- **S-PBUNDLE-006** — Tests: TBD — Code: `src/ops/patch_bundle.rs` — Status: Planned

---

## Apply

- **S-APPLY-001** — Tests: TBD — Code: `src/ops/apply.rs` — Status: Planned
- **S-APPLY-002** — Tests: TBD — Code: `src/ops/apply.rs` — Status: Planned
- **S-APPLY-003** — Tests: TBD — Code: `src/ops/apply.rs` — Status: Planned
- **S-APPLY-004** — Tests: TBD — Code: `src/ops/apply.rs` — Status: Planned
- **S-APPLY-005** — Tests: TBD — Code: `src/ops/apply.rs` — Status: Planned
- **S-APPLY-006** — Tests: TBD — Code: `src/ops/apply.rs` — Status: Planned
- **S-APPLY-007** — Tests: TBD — Code: `src/ops/apply.rs` — Status: Planned
- **S-APPLY-008** — Tests: TBD — Code: `src/ops/run.rs` — Status: Planned

---

## Commit policy

- **S-COMMIT-001** — Tests: TBD — Code: `src/ops/commit.rs`, `src/config.rs` — Status: Planned
- **S-COMMIT-002** — Tests: TBD — Code: `src/ops/commit.rs` — Status: Planned
- **S-COMMIT-003** — Tests: TBD — Code: `src/config.rs` — Status: Planned
- **S-COMMIT-004** — Tests: TBD — Code: `src/ops/apply.rs` — Status: Planned
- **S-COMMIT-005** — Tests: TBD — Code: `src/ops/promote.rs`, `src/ops/commit.rs` — Status: Planned

---

## Verify

- **S-VERIFY-001** — Tests: TBD — Code: `src/ops/verify.rs` — Status: Planned
- **S-VERIFY-002** — Tests: TBD — Code: `src/ops/verify.rs` — Status: Planned
- **S-VERIFY-003** — Tests: TBD — Code: `src/ops/verify.rs` — Status: Planned
- **S-VERIFY-004** — Tests: TBD — Code: `src/ops/verify.rs` — Status: Planned

---

## pack-fix

- **S-PACKFIX-001** — Tests: TBD — Code: `src/ops/pack_fix.rs` — Status: Planned
- **S-PACKFIX-002** — Tests: TBD — Code: `src/ops/pack_fix.rs` — Status: Planned

---

## loop

- **S-LOOP-001** — Tests: TBD — Code: `src/ops/loop.rs` — Status: Planned
- **S-LOOP-002** — Tests: TBD — Code: `src/ops/loop.rs` — Status: Planned

---

## status

- **S-STATUS-001** — Tests: `tests/m0_integration.rs` — Code: `src/ops/status.rs` — Status: Implemented
- **S-STATUS-002** — Tests: `tests/m0_integration.rs` — Code: `src/ops/status.rs` — Status: Implemented

---

## Secrets (handoff build)

- **S-SECRETS-001** — Tests: TBD — Code: `src/secrets.rs` — Status: Planned
- **S-SECRETS-002** — Tests: TBD — Code: `src/secrets.rs` — Status: Planned
- **S-SECRETS-003** — Tests: TBD — Code: `src/secrets.rs` — Status: Planned

---

## Ops safety policy

## Sessions

- **S-SESSION-001** — Tests: `tests/m1_worktrees.rs` — Code: `src/ops/session.rs` — Status: Implemented
- **S-SESSION-002** — Tests: `tests/m1_worktrees.rs` — Code: `src/ops/session.rs` — Status: Implemented
- **S-SESSION-003** — Tests: `tests/m1_worktrees.rs` — Code: `src/ops/worktree.rs` — Status: Implemented
- **S-SESSION-004** — Tests: `tests/m1_worktrees.rs` — Code: `src/ops/session.rs` — Status: Implemented

---

## Promotion

- **S-PROMOTE-001** — Tests: TBD — Code: `src/ops/promote.rs` — Status: Planned
- **S-PROMOTE-002** — Tests: TBD — Code: `src/ops/promote.rs`, `src/config.rs` — Status: Planned
- **S-PROMOTE-003** — Tests: TBD — Code: `src/ops/promote.rs` — Status: Planned

---

## Ops secrets & user tasks

- **S-OPS-SECRETS-001** — Tests: TBD — Code: `src/ops/secrets.rs` — Status: Planned
- **S-OPS-SECRETS-002** — Tests: TBD — Code: `src/ops/secrets.rs` — Status: Planned
- **S-OPS-TASKS-001** — Tests: TBD — Code: `src/ops/tasks.rs` — Status: Planned

---

- **S-OPS-001** — Tests: `tests/m0_integration.rs` — Code: `src/ops/lock.rs` — Status: Implemented
- **S-OPS-002** — Tests: `tests/m0_integration.rs` — Code: `src/ops/lock.rs` — Status: Implemented
- **S-OPS-003** — Tests: TBD — Code: `src/ops/paths.rs` — Status: Planned
- **S-OPS-004** — Tests: TBD — Code: `src/ops/paths.rs` — Status: Planned
- **S-OPS-005** — Tests: TBD — Code: `src/ops/patch_policy.rs` — Status: Planned

---

## Runs & logs

- **S-RUN-001** — Tests: `tests/m0_integration.rs` — Code: `src/ops/run.rs` — Status: Implemented
- **S-RUN-002** — Tests: TBD — Code: `src/ops/run.rs` — Status: Partial
- **S-RUN-003** — Tests: TBD — Code: `src/ops/run.rs` — Status: Planned

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
- **S-EXIT-005** — Tests: TBD — Code: `src/exit.rs` — Status: Planned
- **S-EXIT-006** — Tests: TBD — Code: `src/exit.rs` — Status: Planned
- **S-EXIT-007** — Tests: TBD — Code: `src/exit.rs` — Status: Planned
- **S-EXIT-008** — Tests: TBD — Code: `src/exit.rs` — Status: Planned
- **S-EXIT-009** — Tests: TBD — Code: `src/exit.rs` — Status: Planned
- **S-EXIT-010** — Tests: `tests/m0_integration.rs` — Code: `src/exit.rs` — Status: Implemented
- **S-EXIT-011** — Tests: TBD — Code: `src/exit.rs` — Status: Planned
- **S-EXIT-012** — Tests: TBD — Code: `src/exit.rs` — Status: Planned
- **S-EXIT-013** — Tests: TBD — Code: `src/exit.rs` — Status: Planned

# Implementation status (how to interpret progress)

diffship is developed with **spec-driven development**.

- The **spec** (`docs/SPEC_V1.md`) is the source of truth.
- The **implementation** may be incomplete while development is in progress.
- Progress is tracked per requirement in `docs/TRACEABILITY.md` using `Status:`.

This document explains how to read that status and how to update it.

---

## Inventory snapshot (2026-03-06)

This is the current implementation inventory based on:

- code in `src/`
- integration tests in `tests/`
- user-facing docs (`README.md`, `PLAN.md`, `docs/*`)

### Ops side

| Area | Status | Evidence (code / tests / docs) |
|---|---|---|
| `init` / `status` / `runs` | Implemented | `src/ops/init.rs`, `src/ops/status.rs`, `src/ops/runs.rs`; `tests/m0_integration.rs`; `README.md`, `docs/OPS_WORKFLOW.md` |
| `apply` / `verify` / `promote` / `loop` | Implemented (core) | `src/ops/apply.rs`, `src/ops/verify.rs`, `src/ops/promote.rs`, `src/ops/loop_cmd.rs`; `tests/m2_apply_verify.rs`, `tests/m2_promotion_loop.rs`; `README.md`, `docs/OPS_WORKFLOW.md` |
| `pack-fix` | Implemented | `src/ops/pack_fix.rs` and automatic generation from `verify` failures in `src/ops/verify.rs`; covered by `tests/m2_pack_fix.rs` |
| secrets / tasks / ack | Implemented | `src/ops/secrets.rs`, `src/ops/tasks.rs`, `src/ops/promote.rs`; `tests/m2_promotion_loop.rs`, `tests/m3_tasks.rs`; `docs/OPS_WORKFLOW.md` |
| config precedence | Implemented | `src/ops/config.rs`; `tests/m4_config_precedence.rs`; `README.md`, `docs/CONFIG.md` |
| promotion / commit-policy switching | Implemented | CLI/config wiring in `src/cli.rs`, `src/ops/config.rs`, `src/ops/promote.rs`; tests in `tests/m4_02_promotion_switch.rs` and `tests/m4_config_precedence.rs` |
| TUI v0 (ops + handoff guidance) | Implemented | `src/tui/mod.rs`, `src/plan.rs`, `src/ops/mod.rs`; `tests/m5_tui_cli_parity.rs`, `src/tui/mod.rs` unit tests; `README.md` |

### Handoff side

| Area | Status | Evidence (code / tests / docs) |
|---|---|---|
| `build` command | Implemented | `src/handoff.rs`, `src/cli.rs`; `tests/m6_handoff_build.rs`; `README.md` |
| committed / staged / unstaged / untracked collection | Implemented | `src/handoff.rs`; `tests/m6_handoff_build.rs`; `docs/SPEC_V1.md`, `docs/BUNDLE_FORMAT.md` |
| split-by / profiles / part split | Partial | split-by and part emission exist in `src/handoff.rs`; limit guards via `--max-parts` / `--max-bytes-per-part` are enforced and tested in `tests/m6_handoff_build.rs`; profile presets/config wiring is still future work |
| `HANDOFF.md` generation | Implemented | `src/handoff.rs`; `tests/m6_handoff_build.rs`, `tests/m6_handoff_determinism.rs`; `docs/HANDOFF_TEMPLATE.md` |
| `excluded.md` / `attachments.zip` / `secrets.md` | Implemented | `src/handoff.rs`; `tests/m6_handoff_build.rs`; `docs/BUNDLE_FORMAT.md` |
| filters (`.diffshipignore` + `--include` / `--exclude`) | Implemented | `src/filter.rs`, `src/handoff.rs`, `src/cli.rs`, `src/tui/mod.rs`; `tests/m6_handoff_build.rs`, `src/filter.rs` unit tests; `README.md` |
| determinism / golden tests | Implemented | deterministic ordering/zip metadata in `src/handoff.rs`; `tests/m6_handoff_determinism.rs`, `tests/golden/m6_simple/*`; `docs/DETERMINISM.md` |
| `preview` command | Implemented | `src/preview.rs`, `src/cli.rs`; directory/zip bundles are supported; covered by `tests/m6_preview.rs` |
| `compare` command (bundle reproducibility check) | Implemented | `src/bundle_compare.rs`, `src/cli.rs`; normalized/strict compare is covered by `tests/m6_compare.rs` |
| packing limits / binary policy (runtime) | Implemented | `--max-parts` / `--max-bytes-per-part` and `EXIT_PACKING_LIMITS` are implemented (`src/cli.rs`, `src/handoff.rs`, `src/exit.rs`); `--include-binary` / `--binary-mode raw|patch|meta`, fallback repacking, and context reduction (`U3 -> U1 -> U0`) are covered by `tests/m6_handoff_build.rs` |

### v1 readiness interpretation

- Ops core loop is v1-usable for day-to-day apply/verify/promote with safety defaults.
- Handoff generation is usable for practical diff handoff.
- Remaining v1 gaps are mostly around plan export/replay (`S-TUI-004`) and JSON output for preview/compare.

---

## Status values

### Planned
The requirement is defined in the spec, but is not implemented yet.

Typical mapping:
- `Code: TBD`
- `Tests: TBD` (or planned but not written)

### Partial
Some part exists, but the requirement is not fully satisfied.

Typical mapping:
- either `Code` exists but `Tests: TBD`
- or tests exist but `Code: TBD` (rare, but possible for contract-first work)

Use `Partial` only when there is real, user-visible progress.

### Implemented
The requirement is implemented and verified to the extent defined by the spec.

Typical mapping:
- `Code` points to real files/modules
- `Tests` points to real tests (or `N/A` if explicitly allowed)

### N/A
Not applicable for this version or not relevant (explicitly stated in traceability).

Typical mapping:
- `Code: N/A`
- `Tests: N/A`

---

## How to update status

When you implement a requirement (`S-...`):

1) Update code
2) Add/adjust tests
3) Update `docs/TRACEABILITY.md`:
   - fill in `Code:` and `Tests:` paths
   - set `Status:` appropriately
4) Run gates: `just ci`

If you only add tests (or only add code), use `Partial`.

---

## Important note about `HANDOFF.md`

`HANDOFF.md` is a **generated output** included in bundles. It is **not** stored in the repository.
References to `HANDOFF.md` in docs usually mean “the generated file inside the bundle”.

---

## FAQ

### The spec says X, but the tool does not do X yet. Is that a bug?
Not necessarily. Check `docs/TRACEABILITY.md`:
- If the relevant `S-...` is `Planned`/`Partial`, it may be expected.
- If it is `Implemented`, it is a bug.

### Should we change the spec to match the current implementation?
Usually no. Prefer implementing the spec.
Change the spec only if product decisions changed, and follow `docs/SPEC_CHANGE.md`.

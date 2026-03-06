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
| `pack-fix` | Partial | `src/ops/pack_fix.rs` and automatic generation from `verify` failures in `src/ops/verify.rs`; no dedicated `pack-fix` integration test yet |
| secrets / tasks / ack | Implemented | `src/ops/secrets.rs`, `src/ops/tasks.rs`, `src/ops/promote.rs`; `tests/m2_promotion_loop.rs`, `tests/m3_tasks.rs`; `docs/OPS_WORKFLOW.md` |
| config precedence | Implemented | `src/ops/config.rs`; `tests/m4_config_precedence.rs`; `README.md`, `docs/CONFIG.md` |
| promotion / commit-policy switching | Implemented | CLI/config wiring in `src/cli.rs`, `src/ops/config.rs`, `src/ops/promote.rs`; tests in `tests/m4_02_promotion_switch.rs` and `tests/m4_config_precedence.rs` |
| TUI v0 (ops-focused) | Implemented | `src/tui/mod.rs`, `src/ops/mod.rs`; `tests/m5_tui_cli_parity.rs`; `README.md` |

### Handoff side

| Area | Status | Evidence (code / tests / docs) |
|---|---|---|
| `build` command | Implemented | `src/handoff.rs`, `src/cli.rs`; `tests/m6_handoff_build.rs`; `README.md` |
| committed / staged / unstaged / untracked collection | Implemented | `src/handoff.rs`; `tests/m6_handoff_build.rs`; `docs/SPEC_V1.md`, `docs/BUNDLE_FORMAT.md` |
| split-by / profiles / part split | Partial | split-by and part emission exist in `src/handoff.rs`; limit guards via `--max-parts` / `--max-bytes-per-part` are enforced and tested in `tests/m6_handoff_build.rs`; profile presets/config wiring is still future work |
| `HANDOFF.md` generation | Implemented | `src/handoff.rs`; `tests/m6_handoff_build.rs`, `tests/m6_handoff_determinism.rs`; `docs/HANDOFF_TEMPLATE.md` |
| `excluded.md` / `attachments.zip` / `secrets.md` | Implemented | `src/handoff.rs`; `tests/m6_handoff_build.rs`; `docs/BUNDLE_FORMAT.md` |
| `.diffshipignore` | Implemented | `src/handoff.rs`; `tests/m6_handoff_build.rs`; `README.md` |
| determinism / golden tests | Implemented | deterministic ordering/zip metadata in `src/handoff.rs`; `tests/m6_handoff_determinism.rs`, `tests/golden/m6_simple/*`; `docs/DETERMINISM.md` |
| `preview` command | Not implemented | no `preview` command in `src/cli.rs`; no `src/preview.rs`; spec-only (`docs/SPEC_V1.md`) |
| packing limits / binary policy (runtime) | Future extension / Partial | `EXIT_PACKING_LIMITS` is reserved in `src/exit.rs`; no max-parts/max-bytes enforcement in build path; untracked binary/raw attachment policy is implemented, but `--include-binary` / `--binary-mode` options are not exposed |

### v1 readiness interpretation

- Ops core loop is v1-usable for day-to-day apply/verify/promote with safety defaults.
- Handoff generation is usable for practical diff handoff.
- Remaining v1 gaps are mostly around handoff preview and explicit packing/binary limit policy.

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

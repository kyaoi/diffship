# Implementation status (how to interpret progress)

diffship is developed with **spec-driven development**.

- The **spec** (`docs/SPEC_V1.md`) is the source of truth.
- The **implementation** may be incomplete while development is in progress.
- Progress is tracked per requirement in `docs/TRACEABILITY.md` using `Status:`.

This document explains how to read that status and how to update it.

---

## Inventory snapshot (2026-03-28)

This is the current implementation inventory based on:

- code in `src/`
- integration tests in `tests/`
- user-facing docs (`README.md`, `PLAN.md`, `docs/*`)

### Ops side

| Area | Status | Evidence (code / tests / docs) |
|---|---|---|
| `init` / `status` / `runs` / `explain` | Implemented | `src/ops/init.rs`, `src/ops/status.rs`, `src/ops/runs.rs`, `src/explain.rs`; `tests/m0_integration.rs`, `tests/m8_explain_validate.rs`; `README.md`, `docs/OPS_WORKFLOW.md`; `diffship init` writes `.diffship/.gitignore`, `.diffship/PROJECT_KIT.md`, `.diffship/PROJECT_RULES.md`, `.diffship/AI_GUIDE.md`, `.diffship/WORKFLOW_PROFILE.md`, `.diffship/forbid.toml`, `.diffship/ai_generated_config.toml`, and `.diffship/config.toml`, `--lang en|ja` localizes the paste-ready rules snippet, `--workflow-profile <...>` selects the bootstrap workflow guidance profile, `--refresh-forbid` refreshes only the dedicated forbid file, and the config stubs now separate AI-owned defaults from the user-owned local config while keeping stack-oriented commented `ops.post_apply` presets framed as local normalization; run summaries now derive stable state labels plus next-command hints, and `diffship explain` surfaces the same guidance for runs and handoff bundles |
| `apply` / `verify` / `promote` / `loop` | Implemented (core) | `src/ops/apply.rs`, `src/ops/post_apply.rs`, `src/ops/verify.rs`, `src/ops/promote.rs`, `src/ops/loop_cmd.rs`, `src/ops/failure_category.rs`; `tests/m2_apply_verify.rs`, `tests/m2_promotion_loop.rs`, `tests/m3_tasks.rs`; `README.md`, `docs/OPS_WORKFLOW.md`, `docs/CONFIG.md`; run summaries/TUI now surface command-log counts, phases, direct log artifact paths from `commands.json`, deterministic post-apply changed-path/category summaries, and stable normalized `failure_category` values for apply/verify/promotion failures |
| `validate-patch` preflight | Implemented | `src/ops/validate_patch.rs`, `src/ops/patch_bundle.rs`, `src/cli.rs`; `tests/m8_explain_validate.rs`; `README.md`, `docs/OPS_WORKFLOW.md`, `docs/SPEC_V1.md`; patch-bundle validation now reuses the same contract/path-policy checks as ops preflight without creating a run or mutating the repo |
| `pack-fix` / local strategy inspect | Implemented | `src/ops/pack_fix.rs`, `src/ops/strategy.rs`, and automatic generation from `verify` failures in `src/ops/verify.rs`; reprompt zips now also include `run/post_apply.json` plus `run/post-apply/` logs when local post-apply hooks ran, failure runs now carry deterministic `strategy.resolved.json` plus `PROMPT.md` guidance that points AI at the strategy export before verify-log guidance, built-in metadata such as `tests_expected=false` / `preferred_verify_profile=fast` for the `no-test-fast` fast path, and `diffship strategy` can inspect the same resolved guidance locally without opening the zip; covered by `tests/m2_pack_fix.rs`, `tests/m2_strategy.rs` |
| `cleanup` | Implemented | `src/ops/cleanup.rs`, `src/cli.rs`, `src/ops/mod.rs`; `tests/m7_cleanup.rs`; `README.md`, `docs/OPS_WORKFLOW.md`, `docs/SPEC_V1.md` |
| secrets / tasks / ack | Implemented | `src/ops/secrets.rs`, `src/ops/tasks.rs`, `src/ops/promote.rs`; `tests/m2_promotion_loop.rs`, `tests/m3_tasks.rs`; `docs/OPS_WORKFLOW.md` |
| config precedence | Implemented | `src/ops/config.rs`; `tests/m4_config_precedence.rs`, `src/ops/config.rs`; `README.md`, `docs/CONFIG.md`; the stable workflow schema now also parses `[workflow]`, `[workflow.strategy]`, and deterministic `[workflow.strategy.error_overrides]` mappings for later workflow/strategy exports |
| promotion / commit-policy switching | Implemented | CLI/config wiring in `src/cli.rs`, `src/ops/config.rs`, `src/ops/promote.rs`; tests in `tests/m4_02_promotion_switch.rs` and `tests/m4_config_precedence.rs` |
| TUI v0 (ops + handoff guidance) | Implemented | `src/tui/mod.rs`, `src/plan.rs`, `src/ops/mod.rs`; `tests/m5_tui_cli_parity.rs`, `src/tui/mod.rs` unit tests; the handoff screen includes preview/build flow plus editable plan path / packing limits, visible input help, and structured-context summary plus manifest reading-order lines in the preview pane when the manifest is present; the TUI also includes a compare screen that consumes `diffship compare --json` and surfaces canonical structured-context deltas; `README.md` |

### Handoff side

| Area | Status | Evidence (code / tests / docs) |
|---|---|---|
| `build` command | Implemented | `src/handoff.rs`, `src/cli.rs`, `src/handoff_config.rs`; `tests/m6_handoff_build.rs`; `README.md`; default output naming uses local timestamps plus the current `HEAD` short SHA and collision suffixes when `--out` is omitted, either `--out-dir` or `[handoff].output_dir` can redirect the generated bundle under a custom parent directory, `--project-context focused` adds a deterministic supplemental project-context pack for hosted AI workflows with richer per-file semantic, context-label, usage-role/priority, edit-scope, verification-relevance, task-group-reference, and relationship-summary data, and build now also exports deterministic `WORKFLOW_CONTEXT.md` plus `workflow.context.json` artifacts referenced from `AI_REQUESTS.md` and the handoff manifest; handoff defaults from `[sources]`, `[split]`, `[untracked]`, `[diff]`, and `[secrets]` now feed both CLI/TUI build planning and replayable `plan.toml` export without later config drift |
| committed / staged / unstaged / untracked collection | Implemented | `src/handoff.rs`; `tests/m6_handoff_build.rs`; `docs/SPEC_V1.md`, `docs/BUNDLE_FORMAT.md` |
| split-by / profiles / part split | Implemented | split-by, named handoff profiles (`20x512`, `10x100`, custom config), and part emission exist in `src/handoff.rs`, `src/handoff_config.rs`, `src/cli.rs`, `src/tui/mod.rs`; limits/profile behavior is covered by `tests/m6_handoff_build.rs`; docs and generated config stub now explain that profile catalogs live in config while `plan.toml` exports the selected profile + resolved limits |
| `HANDOFF.md` generation | Implemented | `src/handoff.rs`; `tests/m6_handoff_build.rs`, `tests/m6_handoff_determinism.rs`; `docs/HANDOFF_TEMPLATE.md`; generated handoffs now also carry a bundle-local `AI_REQUESTS.md` scaffold for hosted AI use, including focused project-context reading hints, verification-focused guidance, and per-part intent guidance when those supplemental facts exist |
| `AI_REQUESTS.md` execution recipe | Implemented | `src/handoff.rs`; `tests/m6_handoff_build.rs`; generated handoffs now also reuse manifest task groups plus focused project-context usage metadata to emit a deterministic task-group execution recipe (primary labels, task-shape hints, bounded read order, bounded write-scope hints, focused project-context edit-scope hints, related project files, risk hints, review-strategy hints, verification-target hints, verification-strategy hints, widening-strategy hints, execution-flow hints) for hosted AI use |
| structured handoff manifest (`handoff.manifest.json`) | Implemented | deterministic JSON summary generation in `src/handoff.rs`, including bundle-level aggregate category / segment / status counts, deterministic reading-order guidance, additive cross-part `task_groups`, richer task-group execution hints (primary labels, task-shape labels, bounded write-scope hints, related context/project files, suggested bounded read order, risk hints, review labels, verification targets, verification labels, widening labels, execution labels), per-file semantic facts (language/path hints plus bidirectional source/test relationship candidates and likely docs/config links), deterministic coarse semantic labels (docs/config/test touches plus repo-rule/dependency-policy/build-graph/test-infrastructure/import/signature/API-like hints), and deterministic per-file change-hint facts for rename ancestry, attachment/exclusion routing, and reduced-context fallback; bundle-area classification in `src/bundle_compare.rs`; existence/content and determinism coverage in `tests/m6_handoff_build.rs`, `tests/m6_handoff_determinism.rs`; `docs/BUNDLE_FORMAT.md`, `docs/DETERMINISM.md` |
| per-part structured context (`parts/part_XX.context.json`) | Implemented | deterministic per-part JSON generation in `src/handoff.rs`, including per-part aggregate category / segment / status counts, additive `intent_labels` and `review_labels`, deterministic task-group linkage (`task_group_ref`, `task_shape_labels`, `task_edit_targets`, `task_context_only_files`), the same per-file semantic facts, coarse semantic labels, and per-file change-hint facts for file entries, plus scoped-context hints such as hunk headers, symbol-like names, import-like references, related test candidates, and per-file symbol/import attribution; compare-area classification in `src/bundle_compare.rs`; existence/content and determinism coverage in `tests/m6_handoff_build.rs`, `tests/m6_handoff_determinism.rs`, `tests/m6_compare.rs`; `docs/BUNDLE_FORMAT.md`, `docs/DETERMINISM.md` |
| rendered structured context (`handoff.context.xml`) | Implemented | deterministic XML rendering in `src/handoff.rs`; compare-area classification in `src/bundle_compare.rs`; existence/content and determinism coverage in `tests/m6_handoff_build.rs`, `tests/m6_handoff_determinism.rs`, `tests/m6_compare.rs`; `docs/BUNDLE_FORMAT.md`, `docs/DETERMINISM.md` |
| `excluded.md` / `attachments.zip` / `secrets.md` | Implemented | `src/handoff.rs`; `tests/m6_handoff_build.rs`; `docs/BUNDLE_FORMAT.md` |
| filters (`.diffshipignore` + `--include` / `--exclude`) | Implemented | `src/filter.rs`, `src/handoff.rs`, `src/cli.rs`, `src/tui/mod.rs`; `tests/m6_handoff_build.rs`, `src/filter.rs` unit tests; `README.md` |
| determinism / golden tests | Implemented | deterministic ordering/zip metadata in `src/handoff.rs`; `tests/m6_handoff_determinism.rs`, `tests/golden/m6_simple/*`; `docs/DETERMINISM.md` |
| `preview` command | Implemented | `src/preview.rs`, `src/cli.rs`; directory/zip bundles and `--json` output are supported, `--list` surfaces canonical structured-context summary counts plus manifest reading-order guidance when the bundle includes `handoff.manifest.json`, now also reports lightweight semantic/coarse-label/change-hint/scoped inspection coverage for richer canonical JSON facts, surfaces manifest task groups when present, and surfaces focused project-context artifact presence plus summary counts when that supplemental pack exists; covered by `tests/m6_preview.rs` |
| `compare` command (bundle reproducibility check) | Implemented | `src/bundle_compare.rs`, `src/cli.rs`; normalized compare plus `--strict` extracted-entry byte comparison, area/kind diff classification, manifest summary/reading-order deltas from canonical structured-context JSON, and `--json` output are covered by `tests/m6_compare.rs`; raw zip container byte equality is intentionally out of the current v1 contract |
| plan export / replay | Implemented | `src/plan.rs`, `src/handoff.rs`, `src/cli.rs`, `src/tui/mod.rs`; `tests/m6_handoff_build.rs`, `src/plan.rs` unit tests |
| packing limits / binary policy (runtime) | Implemented | `--max-parts` / `--max-bytes-per-part` and `EXIT_PACKING_LIMITS` are implemented (`src/cli.rs`, `src/handoff.rs`, `src/exit.rs`); `--include-binary` / `--binary-mode raw|patch|meta`, fallback repacking, and context reduction (`U3 -> U1 -> U0`) are covered by `tests/m6_handoff_build.rs` |

### v1 readiness interpretation

- Ops core loop is v1-usable for day-to-day apply/verify/promote with safety defaults.
- Handoff generation is usable for practical diff handoff.
- No immediate gaps remain in the current v1 handoff core; remaining handoff items are now mostly future-extension territory such as additional canonical JSON consumers, deeper semantic extraction, and extra compare/TUI UX polish.

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

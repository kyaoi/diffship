---
name: tui-cli-parity
description: Keep TUI and CLI equivalent for handoff plans, preview/build flow, and compare flow.
---

# TUI / CLI parity

Use this when changing `src/tui/mod.rs`, `src/plan.rs`, or any handoff option exposed in the TUI.

## Read first
1) `docs/SPEC_V1.md` for `S-TUI-001..008` and `S-PLAN-001..002`
2) `docs/OPS_WORKFLOW.md`
3) `docs/TRACEABILITY.md`

## Rules
- Every TUI toggle or editable field must have a CLI equivalent.
- The TUI must be able to export a replayable `plan.toml`.
- The TUI must show an equivalent CLI command for the current handoff state.
- Handoff preview should reuse canonical bundle facts when present; do not invent TUI-only preview semantics.
- Compare view should wrap `diffship compare --json`; do not create a second comparison model in the TUI.

## Main code paths
- `src/tui/mod.rs`
- `src/plan.rs`
- `src/cli.rs`
- `src/handoff.rs`
- `src/preview.rs`
- `src/bundle_compare.rs`

## Tests
- `tests/m5_tui_cli_parity.rs`
- `tests/m6_handoff_build.rs`
- `tests/m6_preview.rs`
- `tests/m6_compare.rs`
- `src/tui/mod.rs` unit tests

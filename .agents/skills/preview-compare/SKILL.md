---
name: preview-compare
description: Change diffship preview or compare behavior safely, including JSON output and TUI compare consumption.
---

# Preview and compare

Use this when changing `diffship preview`, `diffship compare`, or the TUI compare screen.

## Read first
1) `docs/SPEC_V1.md` for `S-PREVIEW-001..010`, `S-COMPARE-001..006`, and `S-TUI-006..008`
2) `docs/BUNDLE_FORMAT.md`
3) `docs/OPS_WORKFLOW.md`
4) `docs/TRACEABILITY.md`

## Preview rules
- `--list` and `--list --json` should surface canonical structured-context summary facts from `handoff.manifest.json` when present.
- Surface reading order, task groups, AI request scaffold presence, project-context summary, and lightweight semantic/change-hint coverage without dumping raw JSON.
- Do not reparse rendered markdown when canonical JSON already provides the fact.

## Compare rules
- Normalized mode is for reproducibility checks.
- `--strict` compares extracted entry bytes, not raw zip container metadata.
- Human-readable and JSON outputs must classify the same differences by area/kind.
- When manifests exist on both sides, surface summary and reading-order deltas from canonical JSON.

## TUI rule
- The compare screen wraps `diffship compare --json`; keep the CLI as the single comparison engine.

## Tests
- `tests/m6_preview.rs`
- `tests/m6_compare.rs`
- `src/tui/mod.rs` unit tests

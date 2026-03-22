---
name: define-options
description: Add or change CLI, TUI, config, or plan options while preserving parity, replayability, and spec/docs alignment.
---

# Define options

Use this when changing flags, config keys, TUI controls, or `plan.toml` fields.

## Read first
1) `docs/SPEC_V1.md` for the command-specific requirements plus `S-TUI-*`, `S-PLAN-*`, and `S-PATH-001`
2) `docs/CONFIG.md` for config-backed defaults
3) `docs/TRACEABILITY.md`

## Keep these surfaces aligned
- `src/cli.rs` for CLI parsing/help
- command implementation (`src/handoff.rs`, `src/preview.rs`, `src/bundle_compare.rs`, `src/ops/*`)
- `src/plan.rs` if the option is replayable
- `src/tui/mod.rs` if the option is user-facing in TUI
- `README.md` and contract docs when behavior or outputs change

## Rules
- Every TUI control must stay expressible via CLI.
- Replayable selection belongs in `plan.toml`; one-shot output destinations stay CLI-time unless the spec says otherwise.
- Config precedence stays `CLI > manifest > project > global > built-in`.
- Path arguments must keep `~/` support and reject `~user`.
- If the option changes bundle contents or patch-bundle semantics, update `docs/BUNDLE_FORMAT.md` or `docs/PATCH_BUNDLE_FORMAT.md` in the same change.

## Tests
- Parser/integration coverage for the flag or config key
- `plan.toml` round-trip when replay is involved
- TUI command reproduction or state coverage when exposed in TUI
- `docs/TRACEABILITY.md` when behavior changes

## Related skills
- Use `tui-cli-parity` when the main risk is TUI/CLI drift.
- Use `init-project-kit` for `diffship init` options.

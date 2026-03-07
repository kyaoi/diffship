# diffship overview
- Purpose: spec-driven Rust CLI/TUI for AI-assisted code handoff bundles and safe patch-bundle ops workflows.
- Core contracts live in `docs/SPEC_V1.md`, `docs/BUNDLE_FORMAT.md`, and `docs/PATCH_BUNDLE_FORMAT.md`.
- Handoff side covers `build`, `preview`, `compare`, plan export/replay, deterministic packing, filters, binary policy, and TUI parity.
- Ops side covers `init`, `status`, `runs`, `apply`, `verify`, `promote`, `loop`, tasks/secrets/ack, config precedence, and pack-fix.
- Main code layout: `src/cli.rs` flags, `src/handoff.rs` bundle build, `src/preview.rs`, `src/bundle_compare.rs`, `src/plan.rs`, `src/tui/mod.rs`, and `src/ops/*` for patch-bundle workflow.
- Tests are primarily integration-style under `tests/` with some focused unit tests in source modules.
- Determinism matters for handoff outputs; safety defaults matter for ops commands.
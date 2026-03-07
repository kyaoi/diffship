# Style and conventions
- Follow `AGENTS.md`: spec-driven first, minimal focused changes, no drive-by refactors.
- If behavior changes, update docs/tests/traceability in the same change.
- Keep TUI and CLI equivalent; TUI settings must map to CLI flags and plan replay.
- Commit messages should be small, task-scoped, and include spec IDs when applicable.
- Repository docs are written in English; user-facing chat responses should be in Japanese.
- Deterministic ordering/newlines/archive contents matter for handoff bundles; machine-parseable stable logs matter for ops.
- Prefer touching the minimum files needed and keep code/test/doc changes tightly aligned.
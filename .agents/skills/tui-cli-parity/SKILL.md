---
name: tui-cli-parity
description: Keep TUI and CLI options equivalent via plan export/replay and command reproduction.
---

# TUI/CLI parity

## Must-have behaviors
- Every TUI toggle has a CLI flag.
- TUI can export a `plan.toml`.
- TUI can show an equivalent CLI command (copy/paste).

## Implementation approach
- Define a single `Plan` struct used by:
  - CLI parsing
  - TUI state
  - Build execution

## Tests
- Parse flags → Plan
- Load plan.toml → Plan
- Ensure both produce identical build behavior (smoke test)

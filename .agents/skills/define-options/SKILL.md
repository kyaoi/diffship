---
name: define-options
description: How to add/modify CLI/TUI options safely and keep the spec + docs consistent.
---

# Define options (CLI/TUI)

## Goal
Add or refine options without breaking parity or confusing users.

## Checklist
1) Update `docs/SPEC_V1.md`
   - Add requirement IDs if behavior changes
   - Ensure the option is expressible in both CLI and TUI

2) Update docs
- `docs/CONFIG.md` if config keys change
- `docs/BUNDLE_FORMAT.md` if outputs change

3) Update `docs/TRACEABILITY.md`

4) Add tests
- Integration tests for CLI arg parsing + behavior
- Optional snapshot test for `HANDOFF.md` structure

5) Run:
```bash
just ci
```

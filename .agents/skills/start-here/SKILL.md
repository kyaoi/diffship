---
name: start-here
description: Read order and working rules for spec-driven changes in diffship; use before any code, doc, or skill update.
---

# Start here

## Read order
1) `AGENTS.md`
2) `docs/AI_WORKFLOW.md`
3) `docs/IMPLEMENTATION_STATUS.md`
4) `docs/SPEC_V1.md`
5) `docs/TRACEABILITY.md`
6) `docs/BUNDLE_FORMAT.md` for handoff work or `docs/PATCH_BUNDLE_FORMAT.md` for ops work
7) `docs/OPS_WORKFLOW.md`, `docs/CONFIG.md`, `docs/HANDOFF_TEMPLATE.md`, `docs/PROJECT_KIT_TEMPLATE.md` as needed

## Working loop
1) Identify the relevant `S-...` requirement IDs before editing.
2) Touch the minimum files needed.
3) If behavior changes, update docs, tests, and `docs/TRACEABILITY.md` in the same change.
4) Keep handoff outputs deterministic and patch parts canonical.
5) Run `just ci` before finishing.

## Current project reality
- Handoff build, preview, compare, structured context, focused project context, init, and core ops flows are implemented.
- Prefer extending existing contracts instead of adding parallel semantics outside the spec/docs.

## Skill map
- `define-options`: CLI/TUI/config/plan changes
- `handoff-structure`: rendered `HANDOFF.md`
- `structured-handoff`: manifest/context/AI request/project-context artifacts
- `preview-compare`: preview/compare/TUI compare
- `ops-safety`: apply/verify/promote/loop/cleanup
- `init-project-kit`: `diffship init` templates and generated guides

---
name: start-here
description: Repo rules + spec-driven workflow for diffship (AI-assisted development OS).
---

# Start here

## Read order
1) `AGENTS.md`
2) `docs/AI_WORKFLOW.md`
3) `docs/PROJECT_KIT_TEMPLATE.md`
4) `docs/IMPLEMENTATION_STATUS.md`
5) `docs/SPEC_V1.md`
6) `docs/BUNDLE_FORMAT.md`
7) `docs/PATCH_BUNDLE_FORMAT.md`
8) `docs/DETERMINISM.md`
9) `docs/HANDOFF_TEMPLATE.md`

## Setup
```bash
mise install
lefthook install
```

## Before finishing
```bash
just ci
```

## Golden constraints
- **Safety first**: ops commands (`apply/verify/loop`) must default to strict checks (isolated worktrees, base commit match, lock).
- Prefer **diff-only** outputs for handoff bundles (avoid full snapshots by default).
- Keep outputs deterministic where applicable (handoff bundle ordering/parts).
- Never run arbitrary commands from AI output; verify runs only locally configured commands.

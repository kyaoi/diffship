# diffship

**diffship** is an **AI-assisted development OS** for Git repos.

It supports two core workflows:

1) **Handoff**: package Git diffs into an **AI-friendly bundle** that stays within upload limits and includes a single navigation document (`HANDOFF.md`).
2) **Ops**: safely apply an AI-produced **patch bundle** back onto your repo, run verification, and generate a “reprompt bundle” when something fails.

This repository is set up for **spec-driven development**:
- `docs/SPEC_V1.md` is the **source of truth**
- changes must keep CI green
- agent workflows are standardized in `.agents/skills/*/SKILL.md`

---

## What diffship is for

Typical workflow:
1) You run an AI agent to implement changes and return a patch bundle
2) You run `diffship loop` locally to apply + verify safely
3) If it fails, you send the generated “reprompt bundle” back to the AI for iteration
4) When it passes, you commit/push as usual (or let diffship auto-commit if enabled)

---

## Quickstart

### 0) Install dev tools (recommended)
This repo uses **mise** + **just** + **lefthook**.

```bash
mise install
lefthook install
```

### 1) Run quality gates
```bash
just ci
```

---

## Commands (v1)

### Handoff
- `diffship` (no args) → starts **TUI** (same as `diffship tui`)
- `diffship tui` → interactive range/options + preview + build
- `diffship build ...` → non-interactive build (scripts/CI)
- `diffship preview <bundle>` → browse a generated handoff bundle (optional)

### Ops
- `diffship init` → generate a ChatGPT Project kit under `.diffship/`
- `diffship apply <patch-bundle>` → apply a patch bundle safely (strict by default)
- `diffship verify` → run verification commands (profiles)
- `diffship pack-fix` → create a reprompt bundle from the last run
- `diffship loop <patch-bundle>` → apply → verify → (on failure) pack-fix
- `diffship status` → show lock state and recent runs

---

## Documentation

- **Spec (v1, source of truth):** `docs/SPEC_V1.md`
- **Handoff bundle format:** `docs/BUNDLE_FORMAT.md`
- **Patch bundle format:** `docs/PATCH_BUNDLE_FORMAT.md`
- **Config:** `docs/CONFIG.md`
- **Handoff template:** `docs/HANDOFF_TEMPLATE.md`
- **Test plan:** `docs/TEST_PLAN.md`
- **Traceability:** `docs/TRACEABILITY.md`
- **Definition of Done:** `docs/DEFINITION_OF_DONE.md`
- **Spec change workflow:** `docs/SPEC_CHANGE.md`
- **Versioning:** `docs/VERSIONING.md`
- **Working with AI:** `docs/AI_WORKFLOW.md`
- **ChatGPT Project kit template:** `docs/PROJECT_KIT_TEMPLATE.md`
- **Determinism policy:** `docs/DETERMINISM.md`
- **Implementation status:** `docs/IMPLEMENTATION_STATUS.md`

---

## Agent workflows

Read:
1) `AGENTS.md`
2) `.agents/skills/start-here/SKILL.md`
3) the relevant skill in `.agents/skills/`

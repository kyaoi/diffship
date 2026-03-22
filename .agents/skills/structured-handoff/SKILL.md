---
name: structured-handoff
description: Evolve AI-facing handoff artifacts such as AI_REQUESTS.md, manifest/context files, and focused project-context packs without breaking patch-canonical rules.
---

# Structured handoff

Use this when changing generated AI-facing bundle artifacts beyond `HANDOFF.md`.

## Scope
- `AI_REQUESTS.md`
- `handoff.manifest.json`
- `handoff.context.xml`
- `parts/part_XX.context.json`
- `project.context.json`
- `PROJECT_CONTEXT.md`
- `project_context/files/...`

## Read first
1) `docs/SPEC_V1.md` output plus preview/compare sections (`S-OUT-*`, `S-PREVIEW-*`, `S-COMPARE-005..006`)
2) `docs/BUNDLE_FORMAT.md`
3) `docs/DETERMINISM.md`
4) `docs/TRACEABILITY.md`

## Non-negotiables
- Patch parts stay canonical for executable changes.
- `HANDOFF.md` stays the primary human/LLM map.
- Structured JSON/XML must be deterministic and derived from local repository facts only.
- Do not add provider-specific semantics or require reparsing rendered markdown to recover machine-readable facts.

## When editing these artifacts
- Keep root-manifest facts and per-part facts consistent.
- If manifest semantics change, update `preview`, `compare`, and TUI consumers in the same change when needed.
- Focused project-context output stays bounded; it is not a whole-repo snapshot fallback.
- Reuse canonical task-group, review, verification, widening, and edit-scope facts instead of inventing duplicate summaries elsewhere.

## Main files and tests
- `src/handoff.rs`
- `src/preview.rs`
- `src/bundle_compare.rs`
- `src/tui/mod.rs`
- `tests/m6_handoff_build.rs`
- `tests/m6_handoff_determinism.rs`
- `tests/m6_preview.rs`
- `tests/m6_compare.rs`

---
name: handoff-structure
description: Evolve generated HANDOFF.md content while keeping it deterministic, map-like, and patch-centric.
---

# HANDOFF.md structure

Use this when changing the rendered entry document inside handoff bundles.

## Read first
1) `docs/SPEC_V1.md` section 5 plus `S-HANDOFF-001..004`
2) `docs/HANDOFF_TEMPLATE.md`
3) `docs/BUNDLE_FORMAT.md`

## Non-negotiables
- `HANDOFF.md` is the primary human/LLM entrypoint.
- Patch parts remain the canonical executable changes.
- `AI_REQUESTS.md` and structured context files are supplemental, not replacements.
- Ordering must stay deterministic.

## Required sections
- TL;DR and reading order (`S-HANDOFF-001`)
- Change Map with tree/table/category summary (`S-HANDOFF-002`)
- Parts Index (`S-HANDOFF-003`)
- Commit-to-parts mapping when `split-by=commit` (`S-HANDOFF-004`)

## Ordering rules
- Categories: docs -> config -> source -> tests -> other
- Within a category: repo-relative path ascending
- Parts: `part_01`, `part_02`, ... in ascending order

## When changing it
1) Update `docs/HANDOFF_TEMPLATE.md`
2) Update generated output logic in `src/handoff.rs`
3) Refresh tests/goldens (`tests/m6_handoff_build.rs`, `tests/m6_handoff_determinism.rs`, `tests/golden/*`)

## Related skills
- Use `structured-handoff` when changing `AI_REQUESTS.md`, manifest/context JSON/XML, or focused project-context artifacts.
- Use `split-by-commit` when the change mainly affects commit grouping and commit-to-parts mapping.

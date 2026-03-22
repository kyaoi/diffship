---
name: split-by-commit
description: Change split-by=commit behavior for committed ranges while keeping commit-to-part mapping deterministic and clear.
---

# split-by commit

Use this when changing commit-level handoff grouping.

## Read first
1) `docs/SPEC_V1.md` for `S-SPLIT-001..003` and `S-HANDOFF-004`
2) `docs/HANDOFF_TEMPLATE.md`
3) `docs/TRACEABILITY.md`

## Rules
- Commit splitting applies to the committed range only.
- Staged, unstaged, and untracked segments remain file-level units.
- `auto` chooses commit split only when the committed range spans multiple commits.
- `HANDOFF.md` must keep a deterministic commit -> parts mapping.

## Expected commit section data
- commit hash (short)
- subject
- date
- optional stats when available
- touched files / part names in deterministic order

## Files and tests
- `src/handoff.rs`
- `tests/m6_handoff_build.rs`
- `tests/m6_handoff_determinism.rs`

## Related skills
- Use `handoff-structure` for rendered `HANDOFF.md` layout changes.
- Use `structured-handoff` if part-context or manifest task grouping changes too.

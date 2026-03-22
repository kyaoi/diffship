---
name: untracked-binary
description: Evolve untracked and binary inclusion, attachments, and exclusions without breaking packing limits or determinism.
---

# Untracked and binary handling

Use this when changing inclusion modes, attachment routing, or exclusion behavior.

## Read first
1) `docs/SPEC_V1.md` for `S-UNTRACKED-*`, `S-BINARY-*`, and `S-PACK-*`
2) `docs/BUNDLE_FORMAT.md`
3) `docs/TRACEABILITY.md`

## Defaults to preserve
- Untracked is off by default.
- Binary content is excluded by default.

## Mode rules
- `untracked-mode`: `auto | patch | raw | meta`
- `binary-mode`: `raw | patch | meta` when `--include-binary`
- `auto` should route small text to patches and large/binary content to deterministic attachment or exclusion handling.

## Output rules
- Raw payloads go into `attachments.zip` under stable prefixes.
- Exclusions must be recorded in `excluded.md` with reasons and guidance.
- `HANDOFF.md` and manifest/context outputs must explain attachment or exclusion decisions.
- Packing fallback and context reduction must remain deterministic.

## Tests
- `tests/m6_handoff_build.rs`
- `tests/m6_handoff_determinism.rs`
- `tests/m6_preview.rs` when preview-visible summary changes

# Deterministic outputs policy

diffship aims to produce bundles that are **as deterministic as practical** so that:

- golden/snapshot tests are reliable
- users can compare bundles across runs
- AI reviewers see stable, low-noise diffs

This document defines what “deterministic” means for v1.

---

## Scope

Determinism applies to:

- `HANDOFF.md` (generated navigation document)
- bundle layout and file naming *inside* the bundle
- ordering of listed items (files/parts/segments)
- text formatting (newlines, encoding)

Determinism does **not** require the default output directory name to be stable (it may include a timestamp),
as long as the **bundle contents** are stable for the same inputs.

---

## Text rules

- Encoding: UTF-8
- Newlines: LF (`\n`)
- No trailing whitespace
- Stable headings and section order

---

## Ordering rules

Use a single, explicit sorting policy everywhere:

1. Category order (when categorizing):
   `docs → config → source → tests → other`
2. Within a category, sort by normalized relative path (ascending, byte order / lexicographic).
3. Parts are ordered by their numeric index (`part_01`, `part_02`, …).

If a list contains mixed sources (committed/staged/unstaged/untracked), the segment order must be explicit and stable.

---

## Bundle / archive rules

For deterministic bundles, avoid embedding unstable metadata:

- Avoid including timestamps in file contents.
- For archives (zip), prefer normalizing or fixing:
  - entry order
  - modification time (if supported)
  - permissions (when possible)

If strict zip determinism is difficult on all platforms, prefer golden tests that compare **extracted, normalized trees**
rather than raw zip bytes.

---

## Recommended testing strategy

- Start with snapshots for:
  - `diffship --help` output
  - `HANDOFF.md`
- For bundle structure, compare:
  - list of paths inside the bundle
  - `HANDOFF.md` content
  - per-part files (normalized)

---

## When changing determinism rules

If you change any ordering or formatting rule:

1) Update this document
2) Update the spec (`docs/SPEC_V1.md`) if it affects user-visible behavior
3) Update golden tests
4) Update traceability status if needed

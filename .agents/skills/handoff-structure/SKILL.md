---
name: handoff-structure
description: How to evolve HANDOFF.md structure while keeping it AI-friendly and stable.
---

# HANDOFF.md structure

## Principle
`HANDOFF.md` is the primary entrypoint for AIs. Keep it:
- short at the top (TL;DR)
- map-like (tree + table + parts index)
- deterministic (stable ordering)

## When changing HANDOFF.md
1) Update `docs/HANDOFF_TEMPLATE.md`
2) Update `docs/SPEC_V1.md` section 5 (handoff requirements)
3) Update tests/snapshots

## Ordering rules
- Category order: docs → config → source → tests → other
- Within category: path ascending
- Parts listed: `part_01`.. in order

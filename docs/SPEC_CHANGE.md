# Spec change workflow (v1)

diffship is developed using **spec-driven development**. This doc defines the smallest workflow
that keeps the spec, tests, and implementation aligned.

---

## When to change the spec

Update the spec whenever you:
- change user-visible behavior (CLI/TUI/options/output layout)
- change defaults or heuristics (e.g., packing rules)
- add or remove an exit code
- introduce a new config key or profile behavior

Minor wording/typo fixes are okay, but avoid “silent behavior changes”.

---

## Checklist

1) Update spec docs
   - `docs/SPEC_V1.md` for requirements and behaviors
   - `docs/BUNDLE_FORMAT.md` if the bundle layout or files change
   - `docs/HANDOFF_TEMPLATE.md` if HANDOFF structure changes

2) Requirement IDs
   - If behavior changes, add or update `S-...` IDs in `docs/SPEC_V1.md`
   - Keep the ID stable once published (don’t reuse an ID for a different meaning)

3) Update traceability
   - Add/adjust mappings in `docs/TRACEABILITY.md` for every impacted `S-...` ID
   - Use `TBD` placeholders if code/tests do not exist yet
   - Update `Status` (`Planned` / `Partial` / `Implemented` / `N/A`) as appropriate

4) Update tests first (recommended)
   - Add/adjust tests to enforce the spec change
   - Prefer golden tests for deterministic outputs (HANDOFF + bundle layout)

5) Implement

6) Run gates
```bash
just ci
```

---

## Breaking changes (v2+)

If a change breaks the v1 contract (CLI flags, exit codes, bundle layout, or HANDOFF structure),
introduce a new versioned doc set (`SPEC_V2.md`, bundle format v2) and clearly document compatibility.

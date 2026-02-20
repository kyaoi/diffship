# Contributing

diffship is built with **spec-driven development**.

## Core workflow
1) Update the spec (if needed): `docs/SPEC_V1.md` and/or `docs/BUNDLE_FORMAT.md`
2) Add/adjust tests to match the spec
3) Implement
4) Update `docs/TRACEABILITY.md`
5) Ensure `just ci` passes and CI is green

See also:
- `docs/SPEC_CHANGE.md` (spec change checklist)
- `docs/VERSIONING.md` (versioning policy)
- `docs/AI_WORKFLOW.md` (how to work with AI safely)
- `docs/DETERMINISM.md` (deterministic outputs policy)
- `docs/IMPLEMENTATION_STATUS.md` (how to interpret current implementation status)

## Local setup
```bash
mise install
lefthook install
just ci
```

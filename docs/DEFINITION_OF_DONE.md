# Definition of Done (DoD)

A change is considered **done** only when:

- `just ci` passes locally
- `just docs-check` passes (docs references are valid)
- CI is green
- If behavior changed:
  - `docs/SPEC_V1.md` updated (with correct requirement IDs)
  - `docs/TRACEABILITY.md` updated (including `Status` for impacted IDs)
  - tests updated/added to cover the change
- If bundle format changed:
  - `docs/BUNDLE_FORMAT.md` updated
  - `docs/HANDOFF_TEMPLATE.md` updated if needed

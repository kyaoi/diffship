# Test Plan (v1)

This plan keeps diffship correct and stable as a **spec-driven** tool.

## Test layers

- **Unit tests**: pure logic (packing, filtering, handoff rendering)
- **Integration tests**: end-to-end CLI workflows (build output layout, determinism)
- **Snapshot / golden tests**: stable outputs only (recommended for `HANDOFF.md` and help text)

## CI gates

CI must run:

- fmt-check
- clippy (deny warnings)
- unit tests
- integration tests
- traceability check
- docs link check

## Golden tests (recommended approach)

Golden tests are valuable when outputs must be deterministic.

Suggested strategy:

- Keep fixtures **small**.
- Normalize unstable fields before comparing (e.g., output directory names).
- Prefer comparing:
  - `HANDOFF.md`
  - `parts/` index summaries
  - command help output (`--help`)

---

## What should NOT be golden-tested

- Wall-clock timestamps
- OS-dependent paths
- Zip metadata that is not normalized

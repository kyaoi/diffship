# Implementation status (how to interpret progress)

diffship is developed with **spec-driven development**.

- The **spec** (`docs/SPEC_V1.md`) is the source of truth.
- The **implementation** may be incomplete while development is in progress.
- Progress is tracked per requirement in `docs/TRACEABILITY.md` using `Status:`.

This document explains how to read that status and how to update it.

---

## Status values

### Planned
The requirement is defined in the spec, but is not implemented yet.

Typical mapping:
- `Code: TBD`
- `Tests: TBD` (or planned but not written)

### Partial
Some part exists, but the requirement is not fully satisfied.

Typical mapping:
- either `Code` exists but `Tests: TBD`
- or tests exist but `Code: TBD` (rare, but possible for contract-first work)

Use `Partial` only when there is real, user-visible progress.

### Implemented
The requirement is implemented and verified to the extent defined by the spec.

Typical mapping:
- `Code` points to real files/modules
- `Tests` points to real tests (or `N/A` if explicitly allowed)

### N/A
Not applicable for this version or not relevant (explicitly stated in traceability).

Typical mapping:
- `Code: N/A`
- `Tests: N/A`

---

## How to update status

When you implement a requirement (`S-...`):

1) Update code
2) Add/adjust tests
3) Update `docs/TRACEABILITY.md`:
   - fill in `Code:` and `Tests:` paths
   - set `Status:` appropriately
4) Run gates: `just ci`

If you only add tests (or only add code), use `Partial`.

---

## Important note about `HANDOFF.md`

`HANDOFF.md` is a **generated output** included in bundles. It is **not** stored in the repository.
References to `HANDOFF.md` in docs usually mean “the generated file inside the bundle”.

---

## FAQ

### The spec says X, but the tool does not do X yet. Is that a bug?
Not necessarily. Check `docs/TRACEABILITY.md`:
- If the relevant `S-...` is `Planned`/`Partial`, it may be expected.
- If it is `Implemented`, it is a bug.

### Should we change the spec to match the current implementation?
Usually no. Prefer implementing the spec.
Change the spec only if product decisions changed, and follow `docs/SPEC_CHANGE.md`.

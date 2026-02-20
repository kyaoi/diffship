# Versioning policy

This project has **three version surfaces** that must stay coherent:

1) CLI/app version (`diffship` SemVer)
2) Bundle format version (e.g., v1 / v2)
3) Spec version document (`docs/SPEC_V1.md`, `docs/SPEC_V2.md`, ...)

---

## 1) CLI/app version (SemVer)

`diffship` follows SemVer:
- **MAJOR**: breaking user-visible changes (flags, defaults, exit codes, output/bundle contract)
- **MINOR**: backward-compatible features
- **PATCH**: backward-compatible fixes

---

## 2) Bundle format version

Bundle format is a **contract** consumed by humans/LLMs and (optionally) tooling.

- “v1” is defined by `docs/BUNDLE_FORMAT.md` and `docs/HANDOFF_TEMPLATE.md`
- If the contract breaks (file names/layout/meaning), create “v2” docs and ensure outputs declare it

Recommendation:
- Keep v1 stable; add new optional fields/files only when strictly backward compatible

---

## 3) Spec version document

- `docs/SPEC_V1.md` describes the v1 behavior and requirements.
- If you introduce a breaking contract change, create `docs/SPEC_V2.md`.
- Keep older spec files in the repo (they are part of the project’s history/contract).

---

## Practical rules

- If you change behavior, update:
  - `docs/SPEC_V*.md`
  - `docs/TRACEABILITY.md`
  - tests
- If you change the bundle contract, update:
  - `docs/BUNDLE_FORMAT.md`
  - `docs/HANDOFF_TEMPLATE.md` (if needed)
- If you bump spec/bundle major versions, bump the app major version too.

# AGENTS.md

Repository-wide rules for humans + AI agents working on **diffship**.

**diffship is spec-driven.** The spec is the source of truth. If behavior changes, update docs + tests in the same change.

> Start here: `.agents/skills/start-here/SKILL.md`
> Human-facing AI guide: `docs/AI_WORKFLOW.md`

---

## Golden Rules (must follow)

1) **Read the spec first**
   - Always start from `docs/SPEC_V1.md`.
   - For formats, also read `docs/BUNDLE_FORMAT.md` (handoff) and `docs/PATCH_BUNDLE_FORMAT.md` (ops).
   - Identify relevant requirement IDs (e.g., `S-UNTRACKED-001`) before coding.

2) **Keep changes small and focused**
   - Touch the minimum files needed.
   - Avoid drive-by refactors.

3) **Determinism where it matters**
   - Handoff bundles must be deterministic (same inputs → same ordering/parts).
   - Ops runs may include timestamps, but logs must remain machine-parseable and stable.

4) **Quality gates are mandatory**
   - Run `just ci` before finishing.
   - If adding behavior, add/update tests and update `docs/TRACEABILITY.md`.

5) **TUI and CLI must stay equivalent**
   - Anything configurable in TUI must be expressible via CLI flags.
   - TUI must be able to export a plan that CLI can replay.

---

## Commit discipline

- One logical task = one commit (as much as practical).
- Include spec ID(s) in the commit message when applicable:
  - Example: `S-SPLIT-001: implement split-by commit for committed range`

---

## Where the “how-to” lives

- Rules: `AGENTS.md`
- Step-by-step recipes: `.agents/skills/*/SKILL.md`
- Product contract: `docs/SPEC_V1.md` + `docs/BUNDLE_FORMAT.md` + `docs/PATCH_BUNDLE_FORMAT.md`
- Human + AI collaboration guide: `docs/AI_WORKFLOW.md`

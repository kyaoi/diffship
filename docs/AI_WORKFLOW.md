# Working with AI (human guide)

This document explains how to collaborate with AI assistants (ChatGPT / Claude / Codex, etc.) while developing **diffship** with **spec-driven development**.

For the practical handoff flow from `diffship build` to "what to send AI / what output to request / how to use the response", see `docs/AI_HANDOFF_FLOW.md`.

> TL;DR: **Spec → Tests → Implementation → Traceability → Gates**. Don’t break the chain.

---

## 0) Preconditions

If the repository was initialized with `diffship init`, also provide the generated local guides when they are relevant:

- `.diffship/PROJECT_KIT.md`
- `.diffship/PROJECT_RULES.md`
- `.diffship/AI_GUIDE.md`
- `.diffship/WORKFLOW_PROFILE.md`

If you maintain a custom init template directory, keep the repository-specific parts of `AI_PROJECT_TEMPLATE.md`
inside the sections marked "Customize this section" so the generated `.diffship/AI_GUIDE.md` stays easy to update.
Apply the same rule to `PROJECT_KIT_TEMPLATE.md` so the generated `.diffship/PROJECT_KIT.md` remains a concise local workflow guide.

* **The spec is the single source of truth**:

  * `docs/SPEC_V1.md`
  * `docs/BUNDLE_FORMAT.md`
  * `docs/HANDOFF_TEMPLATE.md`
* If the spec must change, follow `docs/SPEC_CHANGE.md`.
* Every change must finish with **`just ci` passing**.

---

## 1) What to delegate to AI (and what not to)

### Good AI tasks (recommended)

* Small, self-contained changes (**1 task = ideally 1 commit**)
* Implementing features/fixes **that already exist in the spec**, including tests
* Documentation updates (spec clarifications, how-to, FAQ)
* Refactors **only when behavior is unchanged** and the intent is explicit

### Human-only decisions

* Accepting/rejecting **breaking changes** to the contract
* Security decisions, secrets handling, distribution/release decisions
* Large redesigns (major module reshuffles, architecture reworks)

---

## 2) Rules to keep spec-driven development intact (most important)

When you ask AI to do work, structure the request in this order:

1. **Read the spec**
2. Identify relevant requirement IDs (`S-...`)
3. Propose **tests first** (when possible)
4. Implement
5. Update traceability (`docs/TRACEABILITY.md`)
6. Run gates (`just ci`)

Also make these constraints explicit:

* Keep the change scope minimal (no drive-by refactors)
* Outputs must be **deterministic** (ordering, newlines, archive contents)
* Do not commit generated artifacts (e.g. `diffship_*/`), and don’t bundle them

---

## 3) AI request template (implementation task)

Copy-paste this and fill in `<...>`.

```text
You are an AI engineer working on diffship. We use spec-driven development.

Goal:
  <1–2 lines describing what you want>

Constraints:
  - Treat docs/SPEC_V1.md and docs/BUNDLE_FORMAT.md as the source of truth
  - If the spec changes, follow docs/SPEC_CHANGE.md and update spec/docs/tests/traceability in the same change
  - Keep changes minimal; no unrelated refactors
  - Outputs must be deterministic (ordering, newlines, zip internal ordering, etc.)
  - Do not output or add secrets (tokens/keys/personal data)
  - End in a state where just ci passes

What to do:
  1) List relevant requirement IDs (S-...) and summarize impact briefly
  2) Propose spec/doc diffs if needed (bullet points)
  3) Propose tests to add/update
  4) Implementation plan
  5) Concrete changes (patch or file-by-file edits)
  6) Commands to run (up to just ci)

Input:
  <paste a diffship bundle / patch / git diff if needed>
```

---

## 4) AI request template (review only — safer)

Use this when you want AI to **review**, not implement.

```text
Please review the attached diffship bundle.

Focus:
  - Does behavior match SPEC_V1 and BUNDLE_FORMAT?
  - Is HANDOFF.md clear and deterministic?
  - If the spec changed, does it follow SPEC_CHANGE (spec/docs/tests/traceability updated together)?
  - Is TRACEABILITY updated appropriately?
  - Are there missing tests?

Output:
  - Findings grouped into Must / Should / Nice-to-have
  - If possible, propose minimal patch snippets
```

---

## 5) Basic flow: handing diffs to AI with diffship

1. Make changes (commit or stage)
2. Build a bundle (e.g., `diffship build ...`)
3. Share the produced zip with the AI
4. Ask for review/proposals/patches

Tips:

* Keep the bundle **small** (only necessary diffs; reduce noise)
* Use `.diffshipignore` / `.gitignore` to prevent artifacts and secrets from leaking
* When you share a reprompt zip created by `diffship pack-fix`, tell the AI to read `strategy.resolved.json` before raw verify/post-apply logs when that file is present; built-in profiles may also expose `tests_expected` and `preferred_verify_profile` there
* Before sharing that reprompt zip, you can inspect the same local recommendation with `diffship strategy --run-id <run-id>` or `diffship strategy --latest --json`

---

## 6) Asking better questions (avoid common failure modes)

* Before “what to do”, state **what must not change**
* Decide upfront: are you changing the **spec** or changing the **implementation**?
* When unsure, start with a **review-only** request (Section 4)

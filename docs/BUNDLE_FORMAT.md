# diffship Bundle Format (v1)

This document defines the **bundle contract** produced by `diffship build` and consumed by humans/LLMs (and by `diffship preview` / `diffship compare`).

---

## 1. Directory layout

```
diffship_YYYY-MM-DD_HHMM_<head7>/
  HANDOFF.md
  handoff.manifest.json
  handoff.context.xml
  parts/
    part_01.patch
    part_01.context.json
    part_02.patch
    part_02.context.json
  excluded.md          # only when something is excluded
  attachments.zip      # only when raw attachments exist
  plan.toml            # optional (when exported)
```

A zip bundle (optional) contains the **same layout** at the root.

---

## 2. `HANDOFF.md` (primary entrypoint)

Human/LLM entrypoint: what the bundle represents and how to read it.

Must include:
- TL;DR + recommended reading order
- Included segments (committed/staged/unstaged/untracked) and bases (e.g., HEAD hash)
- Applied path filters (`.diffshipignore`, optional `--include`, optional `--exclude`) when present
- Change map:
  - changed tree
  - file table (path, status, segment, ins/del where available, bytes, part)
  - category summary (docs/config/src/tests/other)
- Parts index (part → top files, segment mix, approximate size)
- If split-by=commit: commit → parts mapping section

See `docs/HANDOFF_TEMPLATE.md` for a recommended structure.

---

## 3. `handoff.manifest.json`

Canonical machine-readable summary for the bundle.

Must be:
- UTF-8, LF, deterministic JSON
- rooted at the bundle top level as `handoff.manifest.json`
- supplemental to patch parts rather than a replacement for them

Must include at least:
- schema version
- `patch_canonical=true`
- entrypoint (`HANDOFF.md`)
- current workspace `HEAD`
- selected sources / split mode / binary + untracked policy
- committed range summary when committed input is present
- applied filters (`.diffshipignore`, include, exclude)
- packing profile / limits / reduced-context warnings
- artifact paths (`parts/*`, optional `attachments.zip`, optional `excluded.md`, optional `secrets.md`)
- parts index and file index
- aggregate row counts by category / segment / status
- deterministic reading-order guidance derived from the selected rows
- structured warning summaries (for example exclusions and secret hits)

JSON is the canonical machine-readable structured-context format for v1. Rendered views MAY be added on top, but they do not replace patch parts or `HANDOFF.md`.

---

## 4. `handoff.context.xml`

Rendered XML view for the bundle-level structured context.

Must be:
- UTF-8, LF, deterministic XML
- rooted at the bundle top level as `handoff.context.xml`
- rendered from the same local deterministic facts as `handoff.manifest.json`

Typical contents:
- entrypoint and rendered-view references
- source/range/filter summary
- packing / warning summary
- artifact references
- part-level summary references (`patch_path`, `context_path`, top files)

This file is an AI-friendly view layer. `handoff.manifest.json` remains the canonical machine-readable source, and patch parts remain the canonical executable changes.

---

## 5. `parts/part_XX.patch`

- UTF-8, LF
- Deterministic ordering (see `docs/DETERMINISM.md`)
- Each part MUST contain clear segment markers (headers) so a reader can see which segment a hunk belongs to.
- When packing fallback is active, diff context MAY be reduced (`U1` / `U0`) to keep a unit inside the configured byte limit.

---

## 6. `parts/part_XX.context.json`

Supplemental machine-readable context for each patch part.

Must be:
- UTF-8, LF, deterministic JSON
- emitted next to the matching patch file (`parts/part_01.patch` → `parts/part_01.context.json`)
- derived from local deterministic repository facts only

Must include at least:
- schema version
- `patch_canonical=true`
- matching patch path and context path
- deterministic title / summary / intent text
- selected segments for that part
- file list and basic diff stats for that part
- aggregate row counts for that part by category / segment / status
- scope / constraints / warning metadata (for example reduced-context paths)

These files help AIs understand each patch part, but they do not replace the patch payload.

---

## 7. `excluded.md`

Must list excluded units with:
- identifier (path or commit)
- segment
- reason
- guidance (e.g., adjust profile, disable include, widen ignore, etc.)

---

## 8. `attachments.zip`

- Stores raw attachments (untracked/binary) under stable prefixes:
  - `untracked/<path>`
  - `binary/<path>`
  - `snapshot/<path>` (only if enabled)
- Binary entries are opt-in (`--include-binary`); default policy excludes binary content.
- `HANDOFF.md` MUST list what was attached and why.

---

## 9. `plan.toml` (optional)

- A replayable description of the handoff selection/options used to build the bundle.
- Export with `diffship build --plan-out <path>` (for example `<bundle>/plan.toml`).
- Replay with `diffship build --plan <path>`.
- Output path / output parent directory / zip emission are CLI-time concerns and may be supplied when replaying the plan.
- Current plan payload includes the selected `profile` name plus resolved numeric limit fields, so replay remains stable if config later changes.
- Named profile definitions themselves stay in config (`[handoff.profiles.*]` / `[profiles.*]`); `plan.toml` is an export of the chosen selection, not a profile catalog dump.

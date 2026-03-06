# diffship Bundle Format (v1)

This document defines the **bundle contract** produced by `diffship build` and consumed by humans/LLMs (and by `diffship preview` / `diffship compare`).

---

## 1. Directory layout

```
diffship_YYYY-MM-DD_HHMM/
  HANDOFF.md
  parts/
    part_01.patch
    part_02.patch
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

## 3. `parts/part_XX.patch`

- UTF-8, LF
- Deterministic ordering (see `docs/DETERMINISM.md`)
- Each part MUST contain clear segment markers (headers) so a reader can see which segment a hunk belongs to.
- When packing fallback is active, diff context MAY be reduced (`U1` / `U0`) to keep a unit inside the configured byte limit.

---

## 4. `excluded.md`

Must list excluded units with:
- identifier (path or commit)
- segment
- reason
- guidance (e.g., adjust profile, disable include, widen ignore, etc.)

---

## 5. `attachments.zip`

- Stores raw attachments (untracked/binary) under stable prefixes:
  - `untracked/<path>`
  - `binary/<path>`
  - `snapshot/<path>` (only if enabled)
- Binary entries are opt-in (`--include-binary`); default policy excludes binary content.
- `HANDOFF.md` MUST list what was attached and why.

---

## 6. `plan.toml` (optional)

- A replayable description of the handoff selection/options used to build the bundle.
- Export with `diffship build --plan-out <path>` (for example `<bundle>/plan.toml`).
- Replay with `diffship build --plan <path>`.
- Output path / zip emission are CLI-time concerns and may be supplied when replaying the plan.

# HANDOFF.md Template (recommended)

> This template shows the intended structure for AI-friendly handoffs.
> The generated HANDOFF.md should follow this structure as closely as possible.

---

## TL;DR
- Bundle: `<name>`
- Profile: `<profile>` (`<max_parts>` parts × `<max_bytes_per_part>` bytes; split-by=`<split-by>`)
- Binary policy: include=`<yes/no>`, mode=`<raw|patch|meta>`
- Segments included: committed=`<yes/no>`, staged=`<yes/no>`, unstaged=`<yes/no>`, untracked=`<yes/no>`
- Committed range: `<mode>` `<from/to or a/b>` (commits: `<n>`)
- Current HEAD (for staged/unstaged/untracked): `<head>`
- Include filters (optional): `<glob1>`, `<glob2>`
- Exclude filters (optional): `<glob3>`
- Project context (optional): `PROJECT_CONTEXT.md` (`<included snapshots>` snapshots, `<omitted>` omitted)
- AI request kit: `AI_REQUESTS.md`
- Reading order:
  1) Docs changes: `<parts/files>`
  2) Config/build changes: `<parts/files>`
  3) Source changes: `<parts/files>`
  4) Tests: `<parts/files>`

---

## 1) Range & Sources Summary
### Committed range
- mode: `<direct|merge-base|last|root>`
- from/to or a/b: `<...>`
- merge-base (if applicable): `<...>`
- commit count: `<n>`

### Current workspace base (for uncommitted segments)
- HEAD: `<hash>`
- staged: `<included?>`
- unstaged: `<included?>`
- untracked: `<included?>` (mode: `<auto|patch|raw|meta>`)
- binary include: `<yes/no>` (mode: `<raw|patch|meta>`)
- `.diffshipignore` active: `<yes/no>`
- include filters (optional): `<...>`
- exclude filters (optional): `<...>`

---

## 2) Change Map

### 2.1 Changed Tree (changed files only)
```
<tree output>
```

### 2.2 File Table (part mapping)
| segment | status | path | ins | del | bytes | part | note |
|---|---:|---|---:|---:|---:|---|---|
| committed | M | src/lib.rs | 10 | 2 | 1234 | part_02 | |
| staged | A | docs/notes.md | 50 | 0 | 456 | part_01 | |

### 2.3 Category Summary
- Docs: `<count>` files → parts: `<...>`
- Config/CI: `<count>` files → parts: `<...>`
- Source: `<count>` files → parts: `<...>`
- Tests: `<count>` files → parts: `<...>`
- Other: `<count>` files → parts: `<...>`

---

## 3) Parts Index
### part_01.patch
- approx bytes: `<n>`
- segments: `<...>`
- top files:
  - `<path1>`
  - `<path2>`

### part_02.patch
...

---

## Project Context (optional)
- Read `PROJECT_CONTEXT.md` before widening scope beyond the changed files.
- `project.context.json` is the canonical machine-readable index for the supplemental project-context pack.
- `project_context/files/...` contains the selected text snapshots.

---

## AI Request Kit
- `AI_REQUESTS.md` provides the deterministic bundle-local hosted-AI request scaffold.
- It should summarize the reading order, supported output modes, and hard loop-safety constraints for the current bundle.

---

## 4) Commit View (only if split-by=commit)
### <hash7> <subject> (<date>)
- stats: `<files>` files, `+<ins> -<del>`
- files:
  - `<path>` → `<part>`
  - `<path>` → `<part>`

---

## 5) Attachments (only if attachments.zip exists)
- `attachments.zip` contains:
  - `untracked/<path>` (reason: ...)
  - `binary/<path>` (reason: ...)
  - `snapshot/<path>` (reason: ...)

---

## 6) Exclusions (only if excluded.md exists)
See `excluded.md`.

---

## Notes
- split-by=commit applies only to committed range; staged/unstaged/untracked remain file-level units.
- Binary/unreadable files are excluded by default unless explicitly included.
- `.diffshipignore` and optional include/exclude filters are applied before writing parts / attachments / exclusions.
- Packing fallback may reduce diff context (`U1` / `U0`) before excluding an oversized unit.

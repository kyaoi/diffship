# HANDOFF

## Start Here
1. Read the TL;DR to understand the scope and included segments.
2. Use the Change Map to see which files changed and which patch part they belong to.
3. Use the Parts Index to decide reading order inside the patch bundle.
4. Open the first patch part: `parts/part_01.patch`

---

## TL;DR
- Bundle: `bundle`
- Profile: `m6` (`max_parts=20`, `max_bytes_per_part=536870912`; split-by=`file`)
- Binary policy: include=`no`, mode=`raw`
- Segments included: committed=`yes`, staged=`no`, unstaged=`no`, untracked=`no`
- Committed range: `last` (HEAD~1..HEAD)
- Commit count (approx): `1`
- Current HEAD (workspace base): `<HEX40>`
- Ignore rules: `.diffshipignore` = `no`
- Reading order:
  1. Docs changes: `part_01.patch` (1 files)

---

## 1) Range & Sources Summary
### Committed range
- included: `yes`
- mode: `last`
- HEAD~1..HEAD
- commit count: `1`

### Current workspace base (for uncommitted segments)
- HEAD: `<HEX40>`
- staged: `no` (base: `HEAD`)
- unstaged: `no` (base: `HEAD` / working tree)
- untracked: `no` (base: `HEAD`, mode: `auto`)
- binary include: `no` (mode: `raw`)
- .diffshipignore active: `no`

---

## 2) Change Map

### 2.1 Changed Tree (changed files only)
```text
docs/
  guide.md
```

### 2.2 File Table (part mapping)
| segment | status | path | ins | del | bytes | part | note |
|---|---:|---|---:|---:|---:|---|---|
| committed | M | `docs/guide.md` | 1 | 1 | 4 | part_01.patch |  |

### 2.3 Category Summary
- Docs: `1` files → parts: `part_01.patch`
- Config/CI: `0` files → parts: `-`
- Source: `0` files → parts: `-`
- Tests: `0` files → parts: `-`
- Other: `0` files → parts: `-`

---

## 3) Parts Index

Use this section to decide reading order inside the patch bundle.

### 3.1 Quick index
| part | segments | files | approx bytes | first files |
|---|---|---:|---:|---|
| `part_01.patch` | `committed` | 1 | <BYTES> | `docs/guide.md` |

### 3.2 Part details
#### part_01.patch
- approx bytes: `<BYTES>`
- segments: `committed`
- top files:
  - `docs/guide.md`


---

## Where to start

Open this document first.
Then apply/read `parts/part_01.patch`.

---

## Notes
- split-by=commit applies only to committed range; staged/unstaged/untracked remain file-level units.
- Binary/unreadable files are excluded by default; use `--include-binary --binary-mode raw|patch|meta` to include them.
- `.diffshipignore` is applied before writing parts / attachments / exclusions.

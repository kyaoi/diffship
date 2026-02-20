---
name: split-by-commit
description: Implement and test split-by=commit (committed only) and keep mapping clear in HANDOFF.md.
---

# split-by commit

## Rule
- Commit splitting applies to committed range only.
- Staged/unstaged/untracked remain file-level units.

## HANDOFF mapping requirements
- Commit header: hash7 + subject + date
- Commit stats: files + ins/del (if available)
- Commit → parts mapping: list touched files and part names

## Tests
- Construct a repo with multiple commits
- Run build with split-by commit
- Assert HANDOFF has commit section and deterministic ordering

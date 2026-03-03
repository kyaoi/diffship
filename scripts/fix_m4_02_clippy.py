#!/usr/bin/env python3
from __future__ import annotations

import re
import sys
from pathlib import Path


def repo_root() -> Path:
    # scripts/xxx.py -> repo root is parent of scripts
    p = Path(__file__).resolve()
    # If placed under scripts/, parents[1] is repo root.
    return p.parents[1]


def read_text(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def write_text(path: Path, text: str) -> None:
    path.write_text(text, encoding="utf-8")


def fix_loop_cmd(text: str) -> tuple[str, bool]:
    # Remove needless struct update: ..Default::default()
    lines = text.splitlines(True)
    out: list[str] = []
    changed = False
    for line in lines:
        if "Default::default()" in line and ".." in line:
            # Most commonly: "            ..Default::default()"
            changed = True
            continue
        out.append(line)
    return "".join(out), changed


def insert_allow_too_many_args(text: str) -> tuple[str, bool]:
    if "pub fn promote_locked" not in text:
        return text, False

    # If already allowed, keep.
    if re.search(r"(?m)^\s*#\[allow\(clippy::too_many_arguments\)\]\s*$", text):
        return text, False

    # Insert attribute immediately above function signature.
    pat = re.compile(r"(?m)^(pub\s+fn\s+promote_locked\s*\()")
    m = pat.search(text)
    if not m:
        return text, False
    idx = m.start(1)
    new = text[:idx] + "#[allow(clippy::too_many_arguments)]\n" + text[idx:]
    return new, True


def collapse_redundant_commit_policy_auto_ifs(text: str) -> tuple[str, bool]:
    # Collapse patterns like:
    # if commit_policy == "auto" {\n  if commit_policy == "auto" {\n ... }
    lines = text.splitlines(True)
    out: list[str] = []
    changed = False

    # Stack of indents for closing braces corresponding to skipped wrapper ifs.
    close_indent_stack: list[str] = []

    i = 0
    pat_if = re.compile(r'^([ \t]*)if\s+commit_policy\s*==\s*"auto"\s*\{\s*$')
    pat_close = re.compile(r'^([ \t]*)\}\s*$')

    while i < len(lines):
        line = lines[i]

        m_if = pat_if.match(line.rstrip("\n"))
        if m_if:
            # Look ahead for consecutive same-condition wrapper ifs.
            j = i + 1
            # Skip empty lines between wrappers? (rare; keep strict to avoid accidental edits)
            if j < len(lines) and pat_if.match(lines[j].rstrip("\n")):
                # Keep the first if.
                out.append(line)
                # Skip all immediately nested redundant wrapper if lines.
                k = j
                while k < len(lines) and pat_if.match(lines[k].rstrip("\n")):
                    indent_k = pat_if.match(lines[k].rstrip("\n")).group(1)  # type: ignore
                    close_indent_stack.append(indent_k)
                    changed = True
                    k += 1
                i = k
                continue

        m_close = pat_close.match(line.rstrip("\n"))
        if m_close and close_indent_stack:
            indent = m_close.group(1)
            # Skip the closing brace for the most-recently skipped wrapper block.
            if indent == close_indent_stack[-1]:
                close_indent_stack.pop()
                changed = True
                i += 1
                continue

        out.append(line)
        i += 1

    return "".join(out), changed


def main() -> int:
    root = repo_root()

    loop_path = root / "src/ops/loop_cmd.rs"
    promote_path = root / "src/ops/promote.rs"

    any_change = False

    if loop_path.exists():
        t = read_text(loop_path)
        nt, ch = fix_loop_cmd(t)
        if ch:
            write_text(loop_path, nt)
            print("OK: fixed src/ops/loop_cmd.rs (remove needless ..Default::default())")
            any_change = True
        else:
            print("OK: no change needed in src/ops/loop_cmd.rs")
    else:
        print("ERR: src/ops/loop_cmd.rs not found", file=sys.stderr)
        return 2

    if promote_path.exists():
        t = read_text(promote_path)
        t1, ch1 = insert_allow_too_many_args(t)
        t2, ch2 = collapse_redundant_commit_policy_auto_ifs(t1)
        if ch1 or ch2:
            write_text(promote_path, t2)
            if ch1:
                print("OK: fixed src/ops/promote.rs (allow too_many_arguments)")
            if ch2:
                print("OK: fixed src/ops/promote.rs (collapse redundant commit_policy==\"auto\" ifs)")
            any_change = True
        else:
            print("OK: no change needed in src/ops/promote.rs")
    else:
        print("ERR: src/ops/promote.rs not found", file=sys.stderr)
        return 2

    if not any_change:
        print("OK: nothing to change")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

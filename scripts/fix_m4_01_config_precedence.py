#!/usr/bin/env python3
from __future__ import annotations
import re
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]

def fix_config_rs() -> bool:
    path = ROOT / "src/ops/config.rs"
    if not path.exists():
        print(f"WARN: missing {path}", file=sys.stderr)
        return False
    s = path.read_text(encoding="utf-8", errors="replace")

    needle = "match section.as_slice() {"
    if needle not in s:
        print("WARN: did not find `match section.as_slice() {` in config.rs (maybe already fixed?)", file=sys.stderr)
        return False

    repl = (
        "let section_str: Vec<&str> = section.iter().map(|s| s.as_str()).collect();\n"
        "        match section_str.as_slice() {"
    )
    s = s.replace(needle, repl, 1)

    path.write_text(s, encoding="utf-8")
    print("OK: fixed src/ops/config.rs")
    return True

def fix_test() -> bool:
    path = ROOT / "tests/m4_config_precedence.rs"
    if not path.exists():
        print(f"WARN: missing {path}", file=sys.stderr)
        return False
    s = path.read_text(encoding="utf-8", errors="replace")

    # Find the exact nested-if block for target_branch extraction and rewrite it to a let-chain if.
    # We intentionally keep this conservative to avoid accidental edits elsewhere.
    # Accept both split_once(':') and split_once(': ').
    pat = re.compile(
        r'(?ms)^(?P<indent>[ \t]*)if line\.trim_start\(\)\.starts_with\("\\\"target_branch\\\""\) \{\n'
        r'(?P=indent)[ \t]*if let Some\(\(\s*_,\s*rhs\s*\)\) = (?P<split>line\.split_once\(\x27:\x27\)|line\.split_once\(\x27: \x27\)) \{\n'
        r'(?P<body>.*?)(?P=indent)[ \t]*\}\n'
        r'(?P=indent)\}\n'
    )
    m = pat.search(s)
    if not m:
        print("WARN: did not find nested if block for target_branch (maybe already fixed?)", file=sys.stderr)
        return False

    indent = m.group("indent")
    split = m.group("split")
    body = m.group("body")

    repl = (
        f"{indent}if line.trim_start().starts_with(\"\\\"target_branch\\\"\")\n"
        f"{indent}    && let Some((_, rhs)) = {split}\n"
        f"{indent}{{\n"
        f"{body}"
        f"{indent}}}\n"
    )
    s = s[:m.start()] + repl + s[m.end():]
    path.write_text(s, encoding="utf-8")
    print("OK: fixed tests/m4_config_precedence.rs")
    return True

def main() -> int:
    changed = False
    if fix_config_rs():
        changed = True
    if fix_test():
        changed = True
    if not changed:
        print("NOTE: no changes applied", file=sys.stderr)
    return 0

if __name__ == "__main__":
    raise SystemExit(main())

#!/usr/bin/env python3
from __future__ import annotations
import sys
from pathlib import Path

TARGET = Path("src/ops/config.rs")

def die(msg: str) -> None:
    print(f"ERROR: {msg}", file=sys.stderr)
    sys.exit(1)

def main() -> None:
    if not TARGET.exists():
        die(f"{TARGET} not found (run from repo root)")

    text = TARGET.read_text(encoding="utf-8")
    lines = text.splitlines(True)

    replaced = False
    i = 0
    while i < len(lines):
        line = lines[i]
        if "if let Some(p) = global_config_path()" in line and line.rstrip().endswith("{"):
            indent = line.split("if")[0]

            def norm(s: str) -> str:
                return s.strip()

            if i + 5 < len(lines):
                l1, l2, l3, l4, l5 = lines[i+1:i+6]
                if (
                    norm(l1).startswith("if p.is_file()") and norm(l1).endswith("{")
                    and norm(l2).startswith("let o = load_config_file(&p)?;")
                    and norm(l3).startswith("cfg.apply_overrides(o);")
                    and norm(l4) == "}"
                    and norm(l5) == "}"
                ):
                    new_block = [
                        f"{indent}if let Some(p) = global_config_path() && p.is_file() {{\n",
                        f"{indent}    let o = load_config_file(&p)?;\n",
                        f"{indent}    cfg.apply_overrides(o);\n",
                        f"{indent}}}\n",
                    ]
                    lines[i:i+6] = new_block
                    replaced = True
                    break
        i += 1

    if not replaced:
        die("target nested-if block was not found; config.rs may have changed")

    TARGET.write_text("".join(lines), encoding="utf-8")

if __name__ == "__main__":
    main()

#!/usr/bin/env bash
set -euo pipefail

python3 scripts/fix_m4_01_collapsible_if.py
echo "OK: fixed collapsible_if in src/ops/config.rs"

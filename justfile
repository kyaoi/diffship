# justfile (command runner)
set shell := ["bash", "-euo", "pipefail", "-c"]

default:
  @just --list

hooks:
  lefthook install

fmt:
  cargo fmt

fmt-check:
  cargo fmt --all -- --check

lint:
  cargo clippy --all-targets --all-features -- -D warnings

test:
  cargo test

it:
  cargo test --tests

trace-check:
  bash scripts/check-traceability.sh

docs-check:
  bash scripts/check-doc-links.sh


ci: fmt-check lint test it trace-check docs-check

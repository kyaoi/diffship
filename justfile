# justfile (command runner)
set shell := ["bash", "-euo", "pipefail", "-c"]

default:
  @just --list

hooks:
  lefthook install

fmt:
  cargo fmt

fmt-fix:
  cargo fmt --all

fmt-check:
  cargo fmt --all -- --check

lint-fix:
  cargo clippy --fix --allow-dirty --allow-staged --all-targets --all-features

lint:
  cargo clippy --all-targets --all-features -- -D warnings

test:
  cargo test
  bash scripts/check-doc-links.sh

it:
  cargo test --tests

trace-check:
  bash scripts/check-traceability.sh

docs-check:
  bash scripts/check-doc-links.sh


ci: fmt-check lint test it trace-check docs-check

ci-fix: fmt-fix lint-fix ci

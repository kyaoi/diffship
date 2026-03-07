# Suggested commands
- Setup: `mise install`, `lefthook install`
- Format: `just fmt` or `cargo fmt --all`
- Lint: `just lint` or `cargo clippy --all-targets --all-features -- -D warnings`
- Tests: `just test`, `just it`, `cargo test`, `cargo test --test <name>`
- Docs/traceability gates: `just docs-check`, `just trace-check`
- Full gate before finishing: `just ci`
- Useful repo commands: `git status --short`, `git branch --show-current`, `rg <pattern>`, `rg --files`, `find src tests docs -maxdepth 2 -type f | sort`
- Entrypoints: `cargo run -- <args>`, `cargo run -- diffship ...` is not needed because binary name is the crate itself.
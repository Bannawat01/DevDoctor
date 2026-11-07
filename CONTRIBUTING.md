# Contributing to DevDoctor

DevDoctor is a local-first Rust diagnostic tool. Small, focused pull requests are preferred.

## Development

Install stable Rust and run from the repository root:

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace --release
```

Windows is the supported MVP platform. Linux CI is experimental and helps catch portability regressions; platform-specific behavior must be labeled clearly in code and documentation.

Do not add telemetry, automatic environment changes, arbitrary script execution, or secret output. Diagnostic probes must remain read-only and bounded.

## Pull requests

Explain the user-visible diagnostic behavior, include deterministic tests, and keep unrelated cleanup out of the change. Never include credentials, connection strings, private paths, or generated build artifacts.

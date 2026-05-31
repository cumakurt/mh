# Contributing to mh

Thank you for contributing to `mh`. All code, identifiers, comments, commit messages, and documentation in this repository must be written in English.

## Development Setup

```bash
git clone https://github.com/cumakurt/mh.git
cd mh
cargo build
cargo test
```

Build with optional remote sync support:

```bash
cargo build --release --features sync
```

## Project Layout

- `src/` — Rust application code
- `migrations/` — SQLite schema migrations
- `tests/` — integration tests
- `scripts/` — packaging helpers
- `install.sh` — Linux installer

## Coding Guidelines

- Keep changes focused and minimal.
- Match existing module structure and naming.
- Prefer explicit error messages over silent failures.
- Add integration tests for database, security, and CLI behavior when changing those areas.

## Running Checks

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release
cargo build --release --features sync
```

## Submitting Changes

1. Create a branch from `main`.
2. Make your changes with tests where appropriate.
3. Ensure `cargo test` and `cargo clippy` pass.
4. Open a pull request with a clear summary and test plan.

## Security

Do not commit real shell history, tokens, passwords, or personal database files. The repository ignores `*.db` files by default.

# Contributing to AutoDev Pipeline

## Development Setup

1. Install Rust: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
2. Clone the repo: `git clone https://github.com/ni9aii/AutoDev.git`
3. Build: `cargo build`
4. Test: `cargo test`
5. Lint: `cargo clippy -- -D warnings`

## Code Style

- Follow `cargo clippy` recommendations (warnings treated as errors in CI)
- All documentation in English (README, CHANGELOG, code comments, commit messages)
- Commit format: `type: description` (fix, feat, chore, docs, refactor)

## Submitting Changes

1. Create a branch: `git checkout -b feat/my-feature`
2. Make changes and commit
3. Push and open a Pull Request against `main`
4. CI must pass (clippy + test + build)

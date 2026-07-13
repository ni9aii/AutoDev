# Contributing to AutoDev

AutoDev is primarily a **skill** that an agent loads and runs — the Rust
binaries are optional accelerators. Contributions fall into two areas: the skill
(skill definition + references) and the Rust tooling.

## Repository layout

```
SKILL.md                     # The skill — primary integration artifact
README.md                    # Install + overview for harness integrators
references/                  # Integration & pattern guides (this folder)
skills/claude-code/SKILL.md  # Claude Code skill surface
src/bin/                     # Optional Rust binaries (run-pipeline, etc.)
tests/                       # Integration tests
```

## Contributing to the skill

The skill is documentation + workflow, so changes are usually edits to
`SKILL.md`, `README.md`, or `references/`.

1. Edit the relevant file. Keep `references/` factual and current — if a guide
   contradicts the code, fix the guide (the code is the source of truth).
2. If you add a new reference doc, link it from `README.md` (the "References"
   section) and from `SKILL.md` where relevant.
3. Open a PR against `main`; CI runs `cargo test` + `clippy` (the Rust parts),
   but doc changes are reviewed by humans.

### Style for docs

- English for all public docs (README, SKILL, references, commit messages).
- Prefer concrete examples (real command invocations, real file paths).
- When describing agent behavior, match what the code actually does — run the
  binary or read `src/bin/` before documenting a phase.

## Contributing to the Rust tooling

1. Install Rust: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
2. Clone: `git clone https://github.com/ni9aii/AutoDev.git`
3. Build: `cargo build`
4. Test: `cargo test`
5. Lint: `cargo clippy -- -D warnings` (warnings are errors in CI)

### Code style

- Follow `cargo clippy` (warnings treated as errors in CI).
- Commit format: `type: description` (`fix`, `feat`, `chore`, `docs`,
  `refactor`).
- Keep commits focused — one logical change per commit.

## Submitting changes

1. Branch: `git checkout -b feat/my-feature`
2. Commit (regularly, one logical change each)
3. Push and open a PR against `main`
4. CI must pass (clippy + test + build) before merge

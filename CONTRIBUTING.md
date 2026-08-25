# Contributing to AutoDev

AutoDev is primarily a **skill** that an agent loads and runs — the Rust
binaries are optional accelerators. Contributions fall into two areas: the skill
(skill definition + references) and the Rust tooling.

## Repository layout

```
SKILL.core.md                # Skill source — edit this, not SKILL.md
SKILL.md                     # Rendered generic surface (generated)
references/                  # Canonical integration & pattern guides (single copy at repo root)
skills/<harness>/references  # Committed SYMLINK -> ../../references (created/refreshed by gen.sh)
skills/<harness>/SKILL.md    # Per-harness rendered surfaces (generated)
docs/                        # User documentation (README links here)
src/bin/                     # Optional Rust binaries (run-pipeline, etc.)
tests/                       # Integration tests
```

There is ONE canonical `references/` directory at the repo root. Each
per-harness skill directory carries a committed symlink
(`skills/<harness>/references -> ../../references`) that `tools/gen.sh`
creates or refreshes idempotently; `install.sh` dereferences it (`rsync -aL`)
so installed skill directories remain self-contained.

### Windows note

On Windows without symlink support, a fresh checkout will show
`skills/<harness>/references` as a plain text file containing
`../../references` instead of an actual link. Use `git config core.symlinks=true`
on a system with symlink support (or work in WSL/Linux) to get real links.

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

### Adding a new harness

Supporting a new agent harness is intentionally small — one overlay file plus
installer wiring. See [docs/new-harness.md](docs/new-harness.md) for the
step-by-step recipe.

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

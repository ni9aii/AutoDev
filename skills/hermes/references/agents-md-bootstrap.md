# AGENTS.md Bootstrap Protocol

Every AutoDev target repository gets a project-level `AGENTS.md` (Hermes,
Claude Code, Kimi and most CLI coding agents read it automatically and inject
it into their system prompt). One file, all executors. This document defines
when to create it, from what template, and how to keep it honest.

## Phase-0 rule

At the start of the `review` phase (before launching reviewers), the
orchestrator checks the project root for `AGENTS.md`:

- **Present** → read it; its rules override `references/rust-conventions.md`
  where they conflict. Quote relevant rules in reviewer prompts so reviewers
  enforce *project* conventions, not just generic ones.
- **Absent** → generate one from the template below, commit it as the first
  fix (`chore: add AGENTS.md with project conventions`), and only then run
  reviewers. Do not skip this silently — note it in the plan.

## Template

```markdown
# AGENTS.md — <project>

## What this project is
<2–4 lines: purpose, language, main crates/binaries.>

## Non-negotiables
- Language/tooling constraints specific to this repo (e.g. "no Python in this repo").
- Test + CI requirements: every feature ships with tests and CI coverage.
- Docs: README/CHANGELOG updated in the same PR as behavior changes.
- Security: no real hostnames/IPs/credentials in code or history — placeholders only.

## Build / test / lint commands
```bash
cargo build --locked
cargo test --locked
cargo clippy --locked -- -D warnings
cargo fmt --check
```
<Adjust to the project's actual commands; list any special env vars or
toolchains (rust-toolchain.toml pins).>

## Git conventions
- Branches `fix/*`, `feat/*`, `chore/*`; never push directly to main.
- Commits: imperative subject, logical units.
- Dependency updates via Renovate only.

## Where good code lives
<Point at 1–3 files that exemplify the house style; agents imitate them.>
```

Fill every section with real values for the target project — an AGENTS.md
with placeholder text is worse than none. If a section truly doesn't apply,
write one line saying why instead of deleting it.

## Maintenance

- When the user corrects an agent on a convention, add it to the target
  project's AGENTS.md in the same session.
- Keep project-specific rules here, not in AutoDev skill surfaces; AutoDev's
  `references/rust-conventions.md` holds only cross-project defaults.
- The drift check (`bash tools/gen.sh && git diff --exit-code`) does not cover
  AGENTS.md — it lives in target repos, not in this one.

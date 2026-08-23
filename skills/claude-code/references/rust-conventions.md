<!-- GENERATED from references/rust-conventions.md by tools/gen.sh — edit the source, not this copy. -->

# Rust Conventions — AutoDev Quality Bar

Shared checklist for all AutoDev roles: **reviewers** (what to flag),
**executors** (what to write), and **verify** (what must pass). Load this file
before writing or reviewing Rust code in any AutoDev run. It is the user's
personal quality bar, not generic advice; when a target project's own AGENTS.md
contradicts something here, the project's AGENTS.md wins.

## Hard rules (flag as CRITICAL when violated)

1. **No Python anywhere in Rust repos.** No `*.py` tooling, no Python build
   steps, no Python-generated artifacts committed to a repo. Scripts are Rust
   binaries or bash. Rationale: the user cannot debug Python.
2. **No `unwrap()` / `expect()` outside tests.** Tests may use them freely.
   In library/CLI code return `Result`, fail with context, or handle explicitly.
   `debug_assert!` is fine. A panic reachable from user input is a CRITICAL bug.
3. **CI gates are non-negotiable:** every change ships with tests AND is covered
   by CI (not just local runs). Gates: `cargo test`, `cargo clippy -- -D warnings`,
   `cargo fmt --check`. New features without a test = CRITICAL finding.
4. **Docs update together with behavior.** README/CHANGELOG/skill surfaces that
   describe changed functionality must be updated in the same PR.
5. **Never commit real infrastructure data.** Hostnames, IPs, usernames, key
   paths get placeholder aliases (`vp-01`, `VP0N_IP`, `your-user`). Public repo
   history included.

## Style rules (flag as IMPORTANT)

- Errors: `thiserror` for library-style enums, `anyhow` (+context) at binary
  edges. Do not hand-roll `Box<dyn Error>` chains in new code.
- Prefer iterators over index loops; no unnecessary `.clone()` in hot paths;
  avoid `String` where `&str` suffices in signatures.
- Modules: one concern per module, `pub(crate)` by default, `pub` only at the
  API surface. Keep `mod.rs` re-export-only.
- Lockstep: `Cargo.lock` committed; dependency bumps go through Renovate (the
  sole dependency bot — Dependabot disabled, Snyk removed).
- Unsafe: none unless justified in a comment with the invariant being upheld.
- Formatting: `cargo fmt` clean; no manual reformatting of untouched code.

## Process conventions

- Branches: `fix/*`, `feat/*`, `chore/*` → never commit directly to main.
  Commits: logical units, imperative subject (`feat:`/`fix:`/`chore:` prefix).
- Verify before reporting done: run `cargo test && cargo clippy -- -D warnings`
  locally and check GitHub Actions status — report real output, not intent.
- When CI reveals a new tooling pitfall, add it to `references/rust-pitfalls.md`
  in the same session (symptom → cause → fix → prevention).

## Reviewer usage

For each rule above, scan the diff/target files and emit findings in the
standard format (`### [SEVERITY] Title`, File, Line). Rules you verified as
satisfied need no findings. In your report summary, add one line:
"Conventions checked against references/rust-conventions.md".

## Executor usage

Before declaring a fix complete, walk the Hard rules top-to-bottom on your own
diff. If your fix adds a new pattern not covered here, propose adding it to
this file in the plan's "Notes" section instead of inventing private style.

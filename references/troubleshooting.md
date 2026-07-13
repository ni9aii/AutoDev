# Troubleshooting & FAQ

Common issues when integrating or running AutoDev.

## "Claude Code CLI is not authenticated"

```
[auto-dev] ERROR Claude Code CLI is installed but NOT authenticated.
[auto-dev] ERROR Re-authenticate with: claude (interactive login)
[auto-dev] ERROR Or use --hermes-mode for delegate_task-based execution.
```

This only happens in **legacy mode** (no `--hermes-mode`), which shells out to
`claude -p`. The pre-flight auth check caught an expired/missing OAuth session.

**Fix:** run `--hermes-mode`. It never calls `claude`, so it works regardless of
Claude Code's auth state (see issue #1). If you specifically need legacy mode,
re-authenticate with `claude` interactively.

## `review-aggregator` exits with "No review directories found"

The aggregator looks for `<dev-notes-root>/<project>/reviews/` and the latest
`<timestamp>/` subdir. This error means that path doesn't exist or is empty.

**Fix:**
- Confirm `--project <name>` matches the folder under the dev-notes root.
- Confirm the reviews were written (Phase 1) before aggregating.
- Confirm the dev-notes root: default `~/obsidian-vault/dev-notes`, override
  with `--dev-notes-root` or `DEV_NOTES_ROOT`.

> If reviews are genuinely empty, `review-aggregator` now writes an **empty
> plan** instead of failing — so a "no findings" run is a clean success, not an
> error.

## Reviews written but the plan is empty

The aggregator only picks up findings in one of the supported shapes:

- `### [CRITICAL] Title`
- `| CRITICAL | Title |`
- `- [CRITICAL] Title`

If reviewers used a different format, findings are silently missed. Update the
reviewer prompt to use the `### [SEVERITY] Title` header shape (see
`references/dev-notes-schema.md`).

## `cargo build` fails

- Requires Rust 1.70+. Install via `rustup` (`curl --proto '=https'
  --tlsv1.2 -sSf https://sh.rustup.rs | sh`).
- The binaries are **optional** — the skill runs end to end with agent-native
  tools. Skip the build entirely if you only use the skill.

## Where did my artifacts go?

All artifacts live under the dev-notes root:

```
$DEV_NOTES_ROOT/<project>/reviews/<timestamp>/
$DEV_NOTES_ROOT/<project>/plans/<timestamp>-plan.md
$DEV_NOTES_ROOT/<project>/ci-reports/<timestamp>-ci-status.md
```

Override the root with `--dev-notes-root` or `DEV_NOTES_ROOT`.

## `--json` output looks mixed with logs

`run-pipeline --json` prints the JSON to **stdout** and all logs to **stderr**.
If you capture both, the JSON is buried. Redirect stderr:

```bash
run-pipeline . full --hermes-mode --json 2>/dev/null
```

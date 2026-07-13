# AutoDev Skill Walkthrough

A phase-by-phase view of what the AutoDev skill does once loaded into your
agent. This complements `references/hermes-delegate-task-integration.md` (which
shows the `delegate_task` mechanics) with the end-to-end picture.

## Invocation

- **Hermes Agent:** `/skill autodev` then follow the skill's prompts.
- **Claude Code:** load `skills/claude-code/SKILL.md` as a skill.
- **Other harnesses:** point the harness at `SKILL.md` (keep `references/`
  alongside it).

No terminal invocation is required — the agent runs the pipeline with its own
native tools. The Rust binaries are optional accelerators.

## Phases

### 1. Plan selection

The skill picks a scope: `review`, `plan` (review + aggregate), `full`
(review + aggregate + execute + verify), or `release`. For a fresh project
start with `full`; for an iterative fix loop, run `full` repeatedly until CI is
green and reviews are empty.

### 2. Review (`delegate_task`, parallel)

Four reviewers run as separate subagents (see the integration guide for the
exact prompts and toolsets):

- **code** — logic, style, idioms, performance
- **security** — vulnerabilities, unsafe code, secrets
- **architecture** — structure, coupling, patterns
- **devops** — CI/CD, dependencies, build, deploy

Each writes its report to
`$DEV_NOTES_ROOT/<project>/reviews/<timestamp>/<role>-review.md`.

### 3. Aggregate (`review-aggregator`)

`review-aggregator --dev-notes --project <project>` parses the latest reviews,
deduplicates, classifies Do Now / Defer, and writes a prioritized plan to
`$DEV_NOTES_ROOT/<project>/plans/<timestamp>-plan.md`.

### 4. Execute (`read_file`+`patch` / `delegate_task`)

For each Do Now fix:

- **≤2 files, ≤20 lines** → `read_file` + `patch` directly.
- **Otherwise** → `delegate_task` with the fix title, severity, file, and
  description.

Deferred items are documented in the plan and skipped.

### 5. Verify (`ci-check`)

`ci-check /path/to/project --dev-notes --project <project>` runs local tests
and queries GitHub Actions CI, writing a report to
`$DEV_NOTES_ROOT/<project>/ci-reports/<timestamp>-ci-status.md`. Proceed only
if green.

### 6. Release (optional)

`run-pipeline <project> release --version vX.Y.Z` (or the manual `git tag` +
GitHub Release flow). Builds the binary, tags, and publishes the release.

### 7. Loop decision

Repeat review → execute → verify until reviews are empty **and** CI passes, or
until a max-iteration cap is hit (default 5). Then report the result.

## Output artifacts

| Artifact | Path |
|----------|------|
| Review reports | `$DEV_NOTES_ROOT/<project>/reviews/<timestamp>/<role>-review.md` |
| Fix plan | `$DEV_NOTES_ROOT/<project>/plans/<timestamp>-plan.md` |
| CI report | `$DEV_NOTES_ROOT/<project>/ci-reports/<timestamp>-ci-status.md` |

## Programmatic use

When wrapping the binary from your own code, pass `--json`:

```bash
run-pipeline /path/to/project full --hermes-mode --json
```

The summary is emitted on **stdout** (parse it); all human log output goes to
**stderr**.

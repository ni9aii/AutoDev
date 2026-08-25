# dev-notes layout

AutoDev keeps its intermediate artifacts in a dev-notes tree (default
`~/Notes/dev-notes`, override via `--dev-notes-root` or the `DEV_NOTES_ROOT`
env var). This is where the skill writes reviews, plans, and CI reports per
project:

```
$DEV_NOTES_ROOT/
└── <project>/
    ├── reviews/
    │   └── YYYYMMDD_HHMMSS/
    │       ├── code-review.md
    │       ├── security-review.md
    │       ├── architecture-review.md
    │       └── devops-review.md
    ├── plans/
    │   ├── YYYYMMDD_HHMMSS-plan.md
    │   └── YYYYMMDD_HHMMSS-plan.json
    └── ci-reports/
        └── YYYYMMDD_HHMMSS-ci-status.md
```

The timestamp directory (`YYYYMMDD_HHMMSS`) is the unit of a pipeline run:
review reports, the plan derived from them, and the CI report all share one
timestamp, which makes every run reproducible and diffable. Pin
`AUTO_DEV_TIMESTAMP` to re-run a single phase against an earlier run's
artifacts (e.g. resume execution after an interrupted run).

## Plan files

Each aggregate run produces two files:

- **`<ts>-plan.md`** — the human artifact: Do Now / Defer sections with
  per-finding blocks (source reviewer, severity, file, description).
- **`<ts>-plan.json`** — the machine-readable sidecar: every item as
  structured JSON (`role`, `severity`, `title`, `description`, `file`,
  `line`, carry-over provenance). Tools prefer the sidecar; if it disagrees
  with the markdown (e.g. after a manual edit), run-pipeline warns loudly.

Carried-over Defer items keep their provenance (`carried_from`, `attempt`).
Items deferred three or more times are flagged WONTFIX candidates for human
decision.

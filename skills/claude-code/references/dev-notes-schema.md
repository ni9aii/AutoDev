# dev-notes Schema

AutoDev stores all intermediate artifacts under a *dev-notes root*. This
document specifies the exact layout, file names, and finding format so you can
generate or consume them from your own tooling.

## Root resolution

The root is resolved in this order:

1. `--dev-notes-root <path>` (CLI flag)
2. `DEV_NOTES_ROOT` environment variable
3. `~/obsidian-vault/dev-notes` (default)

## Directory layout

```
$DEV_NOTES_ROOT/
└── <project>/
    ├── reviews/
    │   └── <timestamp>/
    │       ├── code-review.md
    │       ├── security-review.md
    │       ├── architecture-review.md
    │       └── devops-review.md
    ├── plans/
    │   └── <timestamp>-plan.md
    └── ci-reports/
        └── <timestamp>-ci-status.md
```

- `<project>` — any slug you pass via `--project` (e.g. `fresnel-beacon`).
- `<timestamp>` — `YYYYMMDD_HHMMSS` (e.g. `20260713_063104`).
- Review role filenames follow `<role>-review.md` where role ∈
  `code`, `security`, `architecture`, `devops`.

`review-aggregator` auto-discovers the **most recent** `reviews/<timestamp>/`
directory (by lexical sort of directory names), so you do not pass the
timestamp explicitly — just keep the `<project>/reviews/<timestamp>/` shape.

## Review report format

A report is a markdown file. Findings are parsed from `### [SEVERITY] Title`
headers and run until the next heading. Three shapes are accepted (all
case-insensitive):

```markdown
### [CRITICAL] SQL injection in db.rs
File: `src/db.rs`
Description: User input is concatenated into a query string without
parameterization.
```

```markdown
| IMPORTANT | Missing auth check |
```

```markdown
- [MINOR] Typo in help text
```

### Structured fields

| Field | Source | Notes |
|-------|--------|-------|
| severity | header / table / bullet | `CRITICAL`, `IMPORTANT`, or `MINOR` |
| title | header / table / bullet | text after the severity tag |
| file | `File:` line (regex `File:\s*`?([^`\n]+)`?`) | optional |
| line | `Line:` line (regex `Line:\s*(\d+)`) | optional |
| description | body text | parser-metadata lines are stripped (see below) |

### Metadata stripping

The aggregator removes these lead-in lines from each finding's **description**
so they are not duplicated in the generated plan:

- `File: ...`
- `Description: ...`
- `Line: ...`
- `Source: ...`

Write them in the report (they are convenient for humans) — the aggregator
will pull `File:`/`Line:` into structured fields and drop the raw lines from the
body.

## Plan format

`review-aggregator` writes a markdown plan containing:

- A header with the generated timestamp and finding counts.
- `## Summary by Reviewer` — counts per role and severity.
- `## 🔴 Do Now (Quick Wins)` — each finding as `### Fix N: <title>` with
  `**Source:**`, `**Severity:**`, `**File:**`, `**Description:**`, and
  `**Action:**`.
- `## 🟡 Defer to Next Phase` — same shape for deferred findings.

The plan's `## 🔴 Do Now (Quick Wins)` section is what the execute phase
consumes.

## CI report format

`ci-check` writes a markdown report under `ci-reports/<timestamp>-ci-status.md`
summarizing local test results and GitHub Actions CI status for the project.

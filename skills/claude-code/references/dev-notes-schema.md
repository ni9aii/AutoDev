<!-- GENERATED from references/dev-notes-schema.md by tools/gen.sh — edit the source, not this copy. -->

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

The plan's `## 🔴 Do Now (Quick Wins)` section is what the execute
phase consumes.

### From plan to implementation

Simple fixes (≤2 files, ≤20 lines) can be applied directly from the
`Do Now` section. For complex fixes, use the `plan` skill to decompose
each `Do Now` item into bite-sized implementation tasks (see
`references/plan-autodev-integration.md`). The conversion pattern is:

1. Each `### Fix N: <title>` under `🔴 Do Now` becomes a high-level
   plan entry.
2. If the fix involves multiple files, API changes, or new
   dependencies, write it out as a full implementation plan using the
   `plan` skill format (header + bite-sized TDD tasks).
3. If the fix is a single change in one file, execute it directly per
   the autodev execute phase guidelines.

Example conversion:

**Aggregator output:**
```markdown
### Fix 1: SQL injection in db.rs query builder
**Severity:** CRITICAL
**File:** `src/db.rs`
**Line:** 42
```

**Resulting plan-skill prompt:**
"Write an implementation plan for: Fix SQL injection in db.rs query
builder (CRITICAL). File `src/db.rs:42`. Replace string concatenation
with parameterized queries. Use the plan skill format with bite-sized
TDD tasks."

### Plan file naming convention

Aggregator-generated plans use the timestamp format from their source
review directories: `<timestamp>-plan.md` (e.g.
`20260714_080000-plan.md`). Manually authored implementation plans that
extend an aggregator plan should use the same timestamp to preserve
traceability back to the reviews that produced them.

## CI report format

`ci-check` writes a markdown report under `ci-reports/<timestamp>-ci-status.md`
summarizing local test results and GitHub Actions CI status for the project.

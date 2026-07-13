# AutoDev Patterns — Report Format, Prioritization, Regression Checks

Reference patterns distilled from early pipeline runs. Use these when writing
reviewer prompts, tuning the aggregator, or reviewing a fix before commit.

## Robust report parser

Reviewers format findings inconsistently. The aggregator accepts three shapes
per finding (case-insensitive):

| Format | Example | Matched by |
|--------|---------|------------|
| Markdown header | `### [CRITICAL] Title` | `^###\s*\[(CRITICAL\|IMPORTANT\|MINOR)\]\s*(.+?)$` |
| Table row | `\| CRITICAL \| Title \|` | `^\s*(CRITICAL\|IMPORTANT\|MINOR)\s*\|\s*(.+?)\s*\|` |
| Bullet list | `- [CRITICAL] Title` | `^\\s*[-*]\\s*\\[(CRITICAL\|IMPORTANT\|MINOR)\\]\s*(.+)$` |

The body runs until the next heading. The aggregator then:

1. Strips parser-metadata lead-in lines — `File:`, `Description:`, `Line:`,
   `Source:` — from each finding body so they are not duplicated in the plan.
2. Extracts `File:` and `Line:` via dedicated regexes into structured fields.
3. Deduplicates by severity + title + file (first occurrence wins).
4. Sorts by severity (CRITICAL < IMPORTANT < MINOR), then role, then title.

**When writing a reviewer prompt,** prefer the `### [SEVERITY] Title` header
shape and put `File:` / `Line:` / `Description:` on their own lines. This is
the most reliable for the aggregator and avoids metadata duplication.

## "Do Now / Defer" prioritization

After aggregation, each finding is classified:

**Do Now** (execute in the current iteration) — all must hold:

- single-file change
- no API/signature changes
- no new dependencies
- no cross-module refactor
- examples: bounds checks, spelling, indentation, header guards, `.gitignore`,
  CI timeout tuning

**Defer to next phase** (document in plan, skip execution):

- requires new features (auth, encryption)
- touches multiple modules' APIs
- needs an architectural decision (god-object split, event bus)
- requires hardware testing or Docker setup
- examples: NVS encryption, HTTP auth, `runtime_config_t` refactor

Present both lists to the user for confirmation before executing.

## DevOps false-positive guard

DevOps findings frequently claim "a file is tracked / missing / misconfigured"
without checking reality. **Never act on a DevOps claim without verifying
first:**

| Claimed issue | Verification command | Common false positive |
|---------------|----------------------|----------------------|
| "sdkconfig committed in git" | `git ls-files \| grep "^sdkconfig$"` | File exists locally but is NOT tracked (already in `.gitignore`) |
| "partitions.csv not in CI filters" | `ls partitions.csv 2>/dev/null` | File doesn't exist at all — project uses the default partition table |
| "release.yml missing `-lm`" | `grep "test_led_driver" release.yml` | May already be fixed in a later commit |

Rule: run `git ls-files` / `git status --short` before acting on any
"file tracked/missing" DevOps claim.

## Fix-induced regression checklist

Before committing any patch, verify the 3-line window around the change:

1. **Ordering** — are init/deinit calls in the correct sequence?
   (e.g. clear BEFORE mutex delete)
2. **Synchronization** — is a resource accessed after being freed/destroyed?
3. **Error paths** — does the new error return leave resources locked/allocated?
4. **State consistency** — are global flags updated atomically with the change?

Example:

```c
// BAD: clear AFTER mutex delete → use-after-free
vSemaphoreDelete(led_mutex);
led_driver_clear();  // tries to take led_mutex

// GOOD: clear BEFORE mutex delete
led_driver_clear();
vSemaphoreDelete(led_mutex);
```

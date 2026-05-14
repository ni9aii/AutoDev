# Iteration 2 Patterns — Auto-Dev Pipeline

Session: 2026-05-10, fresnel-beacon Phase 2 completion

## Robust Report Parser Pattern

Subagents use inconsistent severity formatting across iterations:

| Format | Example | Regex |
|--------|---------|-------|
| Markdown header | `### [CRITICAL] Title` | `r"###\s*\[(CRITICAL|IMPORTANT|MINOR)\]\s*(.+?)\n"` |
| Table row | `\| CRITICAL \| Title \|` | `r"\|\s*(CRITICAL|IMPORTANT|MINOR)\s*\|\s*(.+?)\s*\|"` |
| Bullet list | `- [CRITICAL] Title` | `r"^\s*[-*]\s*\[(CRITICAL|IMPORTANT|MINOR)\]\s*(.+)$"` |

**Recommended aggregator approach**: Try all three patterns per report, deduplicate by title similarity (Levenshtein or simple substring match), then sort by severity.

```python
patterns = [
    r"###\s*\[(CRITICAL|IMPORTANT|MINOR)\]\s*(.+?)(?=\n#{1,3}\s|\Z)",
    r"\|\s*(CRITICAL|IMPORTANT|MINOR)\s*\|\s*(.+?)\s*\|",
    r"^\s*[-*]\s*\[(CRITICAL|IMPORTANT|MINOR)\]\s*(.+)$",
]
```

## "Do Now / Defer" Prioritization Framework

After aggregation, classify each finding:

**Do now** (execute in current iteration):
- Single-file change
- No API signature changes
- No new dependencies
- No cross-module refactoring
- Examples: bounds checks, spelling, indentation, header guards, `.gitignore`, CI timeout

**Defer to next phase** (document in plan, skip execution):
- Requires new features (auth, encryption)
- Touches multiple modules' APIs
- Needs architectural decision (god object split, event bus)
- Requires hardware testing or Docker setup
- Examples: NVS encryption, HTTP auth, runtime_config_t refactoring, TRAIL_RADIANS runtime config

Present both lists to user for confirmation before executing.

## DevOps False Positive Patterns

| Claimed issue | Verification command | Common false positive |
|---------------|----------------------|----------------------|
| "sdkconfig committed in git" | `git ls-files \| grep "^sdkconfig$"` | File exists locally but is NOT tracked (already in `.gitignore`) |
| "partitions.csv not in CI filters" | `ls partitions.csv 2>/dev/null` | File doesn't exist at all — project uses default partition table |
| "release.yml missing `-lm`" | `grep "test_led_driver" release.yml` | May already be fixed in a later commit |

**Rule**: Never act on a DevOps "file tracked" claim without running `git ls-files` or `git status --short` first.

## Fix-Induced Regression Checklist

Before committing any patch, verify the 3-line window around the change:

1. **Ordering**: Are init/deinit calls in correct sequence? (e.g. clear BEFORE mutex delete)
2. **Synchronization**: Is a resource accessed after being freed/destroyed?
3. **Error paths**: Does the new error return leave resources locked / allocated?
4. **State consistency**: Are global flags updated atomically with the change?

Example from this session:
```c
// BAD: clear AFTER mutex delete → use-after-free
vSemaphoreDelete(led_mutex);
led_driver_clear();  // tries to take led_mutex

// GOOD: clear BEFORE mutex delete
led_driver_clear();
vSemaphoreDelete(led_mutex);
```

## Graphify + Auto-Dev Integration

1. Run `graphify update .` BEFORE review phase (AST-only, ~2s, no LLM cost)
2. Reviewers can optionally reference `graphify-out/GRAPH_REPORT.md` for architecture context
3. Do NOT run graphify between review and execute — it doesn't affect execution
4. After all fixes are committed and pushed, run `graphify update .` again to keep graph current

# `run-pipeline --json` Output Contract

When invoked with `--json`, `run-pipeline` emits a single JSON object on
**stdout** after the pipeline completes. All human log output (including Hermes
review instructions) is routed to **stderr**, so stdout stays clean for piping
and parsing.

## Example

```bash
run-pipeline /path/to/project review --hermes-mode --json 2>/dev/null
```

```json
{
  "status": "success",
  "version": "0.4.0",
  "project": "/path/to/project",
  "phase": "review",
  "mode": "hermes",
  "timestamp": "20260713_063104",
  "output_dir": "/home/you/obsidian-vault/dev-notes/AutoDev/reviews/20260713_063104"
}
```

## Fields

| Field | Type | Description |
|-------|------|-------------|
| `status` | string | `"success"` when the pipeline completed without error. |
| `version` | string | AutoDev package version (`CARGO_PKG_VERSION`). |
| `project` | string | Absolute or relative path to the target project. |
| `phase` | string | One of `review`, `plan`, `full`, `release`. |
| `mode` | string | `"hermes"` or `"legacy"`. |
| `timestamp` | string | `YYYYMMDD_HHMMSS` of this run; also used in artifact paths. |
| `output_dir` | string | Directory where this run's artifacts are written. |

## Notes for integrators

- **Parse stdout, ignore stderr.** Log lines carry an `[auto-dev]` prefix and
  go to stderr; only the JSON object is on stdout.
- On a fatal error the process exits non-zero and prints an error to stderr;
  no JSON is emitted. Check the exit code before parsing stdout.
- The `output_dir` is where the run wrote (or would write) its reviews/plans;
  use it to locate generated artifacts after a successful run.

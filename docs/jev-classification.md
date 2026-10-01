# Jev classification (optional)

`review-aggregator --jev` re-classifies review findings with **Jev**
(TypeSafe's System One decision model) after the built-in heuristic pass.
The flag is off by default: without it the aggregator's output is
byte-identical to the heuristic-only run.

## How it works

1. Findings are parsed and deduplicated exactly as before; the heuristic
   (`severity + file + no-refactor-keywords`) assigns every finding an
   initial `do_now` / `defer` verdict.
2. With `--jev`, ONE System One request is sent for the whole run: the
   state carries every finding (severity, title, description, file) and
   each question is self-contained — the finding's own data is quoted in
   the question text, so answers cannot be misaligned across findings.
3. A Jev verdict is accepted only at confidence ≥ 0.6 (`CONFIDENCE_GATE`).
   Every rejection path — missing API key, transport error, timeout, low
   confidence, unknown answer type — keeps the heuristic verdict and
   marks the finding `heuristic_fallback`. **Jev never breaks a run.**
4. The plan (markdown and JSON sidecar) records provenance per item, but
   only when the Jev pass actually ran:
   - markdown: a `**Classified by:** jev` (or `heuristic_fallback`) line
     after each rendered item;
   - sidecar: `classification_source` on each `PlanItem`
     (`"jev"` / `"heuristic_fallback"`), omitted otherwise — older
     sidecars and no-flag runs deserialize unchanged.
   `do_now` remains the single field consumers should act on;
   `classification_source` is informational.

## Configuration

| Variable | Description |
|----------|-------------|
| `TYPESAFE_API_KEY` | API key for `api.typesafe.ai` (the default endpoint) |
| `JEV_BASE_URL` | Optional endpoint override. Set to `https://openrouter.ai/api` to use the OpenRouter-hosted System One endpoint (model `jev-latest`) with an OpenRouter key |

If the key is absent and `--jev` was requested, the aggregator prints a
warning to stderr and completes with heuristic classifications — the
plan is identical to a no-flag run.

## CI

- The regular PR pipeline never touches the network: tests use
  `FakeSystemOne`, a scripted in-process fake from kunobi-jev's
  `testing` feature.
- The `jev-smoke` workflow (manual `workflow_dispatch` only) runs one
  live batch against a fixture and asserts every sidecar item carries
  `classification_source`. It skips cleanly when neither
  `TYPESAFE_API_KEY` nor `OPENROUTER_API_KEY` is configured as a secret.

## Implementation notes

- Client: [kunobi-jev](https://crates.io/crates/kunobi-jev) 0.2
  (blocking client, rustls), wrapped in the `autodev-jev` workspace
  crate behind an `AskJev` trait so tests inject the fake.
- Multi-question batches must use **self-contained questions**: asking
  the model to map "finding #N" onto an array position in the state
  returns answers for the wrong findings (observed live during dogfood;
  see the dev-notes dogfood report for the before/after numbers).
- Lockfile maintenance note: `cargo test` (root package only) counts
  116 tests; the workspace-wide count is 124 — `tools/gen-structure.sh`
  uses the latter.

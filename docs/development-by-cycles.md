# Development by cycles

AutoDev's core promise is *loop until release-ready* — and the project holds
itself to it. Every AutoDev release is developed with AutoDev: the pipeline
reviews its own code, plans fixes, executes them, verifies locally and on CI,
and repeats. This document explains the model and shows how a real release
moves through cycles.

## The cycle

```
        ┌────────────────────────────────────────────┐
        │                                            │
        ▼                                            │
  review ──► plan ──► execute ──► verify (local+CI) ─┤
                                                     │
                                     converged? ──yes──► release
```

One **cycle** = one review → plan → execute → verify pass. Each cycle
produces timestamped artifacts (reports, plan, CI status), so progress is
measurable: you can diff cycle N against cycle N−1 and watch the finding
count drop.

## Convergence

The loop stops when new review reports come back empty or contain only
deferred items — that is the "release-ready" signal, decided by evidence, not
by feel. Deferred items carry over between plans with provenance; after
three carries an item is flagged as a WONTFIX candidate for human decision.

## Dogfooding record

AutoDev releases are cut this way:

- v0.8.0 was closed out through dogfood cycles that produced PRs #33–#45
  (restructuring, carry-over, typed plan model) before the release.
- v0.9.0 ships the distribution story described in this repo — one-line
  install, update path, docs restructure — found and driven by the same
  self-review process.

The dogfood trail lives in the dev-notes tree (`<project>/reviews/`,
`<project>/plans/`): every cycle leaves its reports and plan on disk.

## Why this matters for users

Running AutoDev on your project means running the same loop that produces
AutoDev itself. The convergence criterion, the carry-over semantics, and the
verify gates are not aspirational design — they are exercised on every
release of this repository before it reaches you.

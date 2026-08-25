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

AutoDev releases are cut this way. The v0.9.0 release cycle (2026-08-25)
ran three full review → plan → execute → verify cycles before tagging:

| Cycle | Findings | Closed |
|-------|----------|--------|
| 1 | 27 (1 CRITICAL, 9 IMPORTANT, 17 MINOR) | 10 Do Now via PR #57 (+ process fix #56: reviewers must follow the finding schema) |
| 2 | 13 (4 IMPORTANT, 9 MINOR) | 4 Do Now via PR #58 — including a regression the fix batch itself introduced, caught and fixed by the loop |
| 3 | 2 (1 IMPORTANT, 1 MINOR) | converged; both closed (#59 + deferred) |

The trajectory 27 → 13 → 2 is the convergence signal: each cycle re-reviews
the fixes of the previous one, so defects in fixes are caught before release.
Deferred items carry provenance across plans; after three carries they are
flagged WONTFIX candidates for human decision.

The dogfood trail lives in the dev-notes tree (`<project>/reviews/`,
`<project>/plans/`): every cycle leaves its reports and plan on disk.

## Why this matters for users

Running AutoDev on your project means running the same loop that produces
AutoDev itself. The convergence criterion, the carry-over semantics, and the
verify gates are not aspirational design — they are exercised on every
release of this repository before it reaches you.

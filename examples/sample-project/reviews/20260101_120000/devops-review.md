# DevOps Review Report

### [IMPORTANT] CI caches `target/` but not the cargo git index
File: `.github/workflows/ci.yml`
Line: 32
Description: The cargo git checkout is uncached, so every CI run re-fetches git
dependencies. Add `~/.cargo/git` to the cache key to cut 30-60s per run.

### [MINOR] Release workflow has no `draft` review gate
File: `.github/workflows/release.yml`
Description: Tags are pushed and released automatically. Add a manual approval
(or `draft: true`) so a bad tag can be caught before it ships.

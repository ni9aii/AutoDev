# Architecture Review Report

### [IMPORTANT] `Config` struct mixes concerns
File: `src/config.rs`
Description: The `Config` struct carries both runtime tuning and build-time paths.
Split into `RuntimeConfig` and `Paths` so the watchdog settings can change
without rebuilding the path resolution.

### [MINOR] Inconsistent error wrapping
File: `src/error.rs`
Description: Some modules return `anyhow::Error`, others return bespoke enums.
Pick one error strategy at the crate boundary for cleaner call sites.

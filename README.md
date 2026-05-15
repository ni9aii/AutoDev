# AutoDev Pipeline

Automated development pipeline: review → plan → execute → verify → release.

## Architecture

**Hermes Agent (orchestrator) + Claude Code (executor)**

- Hermes: planning, aggregation, decision-making, simple patches
- Claude Code: all coding tasks — fixes, refactoring, tests, CI debugging

## Features

- **4 parallel reviewers**: Code, Security, Architecture, DevOps
- **Finding aggregation**: Do Now / Defer classification
- **Automated execution**: simple fixes via Hermes, complex via Claude Code
- **CI integration**: GitHub Actions status checking
- **Release**: git tag + GitHub Release creation

## Installation

```bash
# Clone
git clone https://github.com/ni9aii/AutoDev.git
cd AutoDev

# Build
cargo build --release

# Install binaries to PATH
cargo install --path .
```

## Requirements

- Rust 1.70+
- Claude Code CLI (`npm install -g @anthropic-ai/claude-code`)
- GitHub PAT (for CI checks and releases)

## Usage

### Full pipeline

```bash
run-pipeline /path/to/project full
```

### Review only

```bash
run-pipeline /path/to/project review
```

### Review + planning

```bash
run-pipeline /path/to/project plan
```

### Release

```bash
run-pipeline /path/to/project release --version v0.2.0
```

## Environment Variables

| Variable | Description |
|----------|-------------|
| `GITHUB_TOKEN` / `GITHUB_PAT` | GitHub API authentication |
| `AUTO_DEV_VERSION` | Fallback version for release phase |

## Project Structure

```
.
├── src/
│   ├── lib.rs                  # Shared modules (log, git, markdown, test_runner)
│   └── bin/
│       ├── run_pipeline.rs     # Main pipeline entry point
│       ├── ci_check.rs         # CI status checker
│       └── review_aggregator.rs # Review aggregation + plan generation
├── .github/workflows/
│   └── ci.yml                  # CI configuration (Arch Linux)
├── Cargo.toml
├── README.md
├── CHANGELOG.md
└── LICENSE
```

## Pipeline Phases

### Phase 1 — Review

Launches 4 reviewers via Claude Code in parallel (3 at a time due to concurrency limits). Each reviewer produces a markdown report with findings classified as CRITICAL / IMPORTANT / MINOR.

Reviewers use **structured file discovery** to avoid context overflow:
1. List project structure via `search_files`
2. Search for relevant patterns (hotspots)
3. Read only matching files

### Phase 2 — Aggregate

The `review-aggregator` binary collects all findings, deduplicates, classifies as Do Now / Defer, and writes a fix plan in markdown.

### Phase 3 — Execute

For each Do Now item:
- Simple fixes (≤2 files, ≤20 lines) → Hermes applies directly
- Complex tasks → delegated to Claude Code print mode

### Phase 4 — Verify

Runs local tests and checks CI status. Fails if tests don't pass.

### Phase 5 — Release

Runs verify first, then:
1. Builds release binary (`cargo build --release`)
2. Creates annotated git tag
3. Pushes tag to origin
4. Creates GitHub Release via API

## License

MIT License — see [LICENSE](LICENSE)

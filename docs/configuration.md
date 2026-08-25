# Configuration

| Variable | Description |
|----------|-------------|
| `GITHUB_TOKEN` / `GITHUB_PAT` | GitHub API auth (CI checks, releases). `gh auth token` is also picked up when available. |
| `DEV_NOTES_ROOT` | Root for dev-notes paths (default: `~/Notes/dev-notes`) |
| `AUTODEV_INSTALL_ROOT` | Base dir for the installer (default: `$HOME`) |
| `AUTO_DEV_TIMESTAMP` | Pin a run's timestamp to resume a single phase against an earlier run's artifacts |

## Release phase requirements

`run-pipeline release --release-version vX.Y.Z` gates on:

1. clean working tree, branch `main`;
2. HEAD's GitHub Actions CI status = success (needs a token);
3. `Cargo.toml` version == the tag version;
4. a curated `## [X.Y.Z]` section in `CHANGELOG.md` — its text becomes the
   GitHub Release body.

A failure at any gate aborts before the tag is created.

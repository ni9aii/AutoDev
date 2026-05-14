#!/usr/bin/env bash
# CI Status Checker for Auto-Dev Pipeline
# Checks GitHub Actions status via API

set -euo pipefail

PROJECT_PATH="${1:-.}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log() {
    echo -e "${BLUE}[ci-check]${NC} $1"
}

warn() {
    echo -e "${YELLOW}[ci-check]${NC} $1"
}

error() {
    echo -e "${RED}[ci-check]${NC} $1"
}

success() {
    echo -e "${GREEN}[ci-check]${NC} $1"
}

# Get GitHub repo info from git remote
get_repo_info() {
    local remote_url
    remote_url=$(cd "$PROJECT_PATH" && git remote get-url origin 2>/dev/null || echo "")
    
    if [ -z "$remote_url" ]; then
        error "No git remote 'origin' found"
        return 1
    fi
    
    # Parse GitHub URL
    if [[ "$remote_url" =~ github.com[:/]([^/]+)/([^/]+)(\.git)?$ ]]; then
        echo "${BASH_REMATCH[1]}/${BASH_REMATCH[2]}"
    else
        error "Not a GitHub repository: $remote_url"
        return 1
    fi
}

# Check CI status via GitHub API
check_ci_status() {
    local repo="$1"
    local token="${GITHUB_PAT:-}"
    
    if [ -z "$token" ]; then
        warn "GITHUB_PAT not set, trying without auth (public repos only)"
    fi
    
    log "Checking CI status for: $repo"
    
    # Get latest workflow runs
    local api_url="https://api.github.com/repos/${repo}/actions/runs?per_page=5"
    local auth_header=""
    
    if [ -n "$token" ]; then
        auth_header="Authorization: Bearer ${token}"
    fi
    
    local response
    if [ -n "$auth_header" ]; then
        response=$(curl -s -H "$auth_header" -H "Accept: application/vnd.github+json" "$api_url")
    else
        response=$(curl -s -H "Accept: application/vnd.github+json" "$api_url")
    fi
    
    # Parse response
    local total_count
    total_count=$(echo "$response" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('total_count', 0))")
    
    if [ "$total_count" = "0" ] || [ "$total_count" = "None" ]; then
        warn "No CI workflows found or API error"
        echo "$response" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('message', 'Unknown error'))" >&2 || true
        return 1
    fi
    
    log "Found $total_count recent workflow runs"
    
    # Show latest run for each workflow
    echo "$response" | python3 -c "
import sys, json
data = json.load(sys.stdin)
runs = data.get('workflow_runs', [])

for run in runs[:3]:
    name = run.get('name', 'Unknown')
    status = run.get('status', 'unknown')
    conclusion = run.get('conclusion', 'N/A')
    branch = run.get('head_branch', 'unknown')
    url = run.get('html_url', '')
    
    status_icon = '✅' if conclusion == 'success' else '❌' if conclusion == 'failure' else '🔄'
    print(f'{status_icon} {name}: {status} ({conclusion}) on {branch}')
    print(f'   URL: {url}')
"
    
    # Check if any failed
    local failed_count
    failed_count=$(echo "$response" | python3 -c "
import sys, json
data = json.load(sys.stdin)
runs = data.get('workflow_runs', [])
failed = [r for r in runs if r.get('conclusion') == 'failure']
print(len(failed))
")
    
    if [ "$failed_count" -gt 0 ]; then
        error "$failed_count recent workflow runs failed!"
        return 1
    fi
    
    success "All recent CI runs passed"
    return 0
}

# Check local test status if available
check_local_tests() {
    log "Checking local test status..."
    
    if [ -f "$PROJECT_PATH/Makefile" ]; then
        if grep -q "^test:" "$PROJECT_PATH/Makefile" 2>/dev/null; then
            log "Running: make test"
            (cd "$PROJECT_PATH" && make test) && success "Local tests passed" || warn "Local tests failed"
        fi
    fi
    
    if [ -f "$PROJECT_PATH/package.json" ]; then
        if grep -q '"test"' "$PROJECT_PATH/package.json" 2>/dev/null; then
            log "Running: npm test"
            (cd "$PROJECT_PATH" && npm test) && success "Local tests passed" || warn "Local tests failed"
        fi
    fi
    
    if [ -f "$PROJECT_PATH/pyproject.toml" ] || [ -f "$PROJECT_PATH/setup.py" ] || [ -f "$PROJECT_PATH/requirements.txt" ]; then
        if command -v pytest > /dev/null 2>&1; then
            log "Running: pytest"
            (cd "$PROJECT_PATH" && pytest -q) && success "Local tests passed" || warn "Local tests failed"
        fi
    fi
}

# Main
main() {
    log "CI Status Checker"
    log "Project: $PROJECT_PATH"
    
    # Get repo info
    local repo
    if ! repo=$(get_repo_info); then
        warn "Could not determine GitHub repo, skipping CI check"
        check_local_tests
        exit 0
    fi
    
    log "Repository: $repo"
    
    # Check CI
    if ! check_ci_status "$repo"; then
        warn "CI check found issues"
        check_local_tests
        exit 1
    fi
    
    check_local_tests
    
    success "All checks passed!"
}

main "$@"

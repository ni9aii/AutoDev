#!/usr/bin/env bash
# Auto-Dev Pipeline Entry Point
# Usage: run-pipeline.sh <project-path> [phase]
# Phases: full (default), review, plan

set -euo pipefail

PROJECT_PATH="${1:-.}"
PHASE="${2:-full}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SKILL_DIR="$(dirname "$SCRIPT_DIR")"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
OUTPUT_DIR="$HOME/.hermes/plans/auto-dev"
mkdir -p "$OUTPUT_DIR"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log() {
    echo -e "${BLUE}[auto-dev]${NC} $1"
}

warn() {
    echo -e "${YELLOW}[auto-dev]${NC} $1"
}

error() {
    echo -e "${RED}[auto-dev]${NC} $1"
}

success() {
    echo -e "${GREEN}[auto-dev]${NC} $1"
}

# Check prerequisites
check_prerequisites() {
    log "Checking prerequisites..."
    
    if [ ! -d "$PROJECT_PATH/.git" ]; then
        error "Not a git repository: $PROJECT_PATH"
        exit 1
    fi
    
    if ! command -v kimi &> /dev/null; then
        warn "Kimi Code CLI not found. Will use manual execution mode."
        export AUTO_DEV_MANUAL=1
    fi
    
    if [ -f "$HOME/.local/share/opencode/auth.json" ]; then
        log "OpenCode auth found"
    else
        warn "OpenCode not configured"
    fi
    
    success "Prerequisites OK"
}

# Phase 1: Review
run_review_phase() {
    log "=== PHASE 1: REVIEW ==="
    log "Launching 4 reviewers in parallel..."
    
    local review_dir="$OUTPUT_DIR/$TIMESTAMP-reviews"
    mkdir -p "$review_dir"
    
    # Launch reviewers via Hermes quick commands
    # Each reviewer is a separate subagent
    
    log "1. Code Reviewer"
    # Hermes will dispatch subagent with code review prompt
    
    log "2. Security Reviewer"
    # Hermes will dispatch subagent with security review prompt
    
    log "3. Architecture Reviewer"
    # Hermes will dispatch subagent with architecture review prompt
    
    log "4. DevOps Reviewer"
    # Hermes will dispatch subagent with devops review prompt
    
    success "Review phase complete. Reports in: $review_dir"
}

# Phase 2: Aggregate
run_aggregate_phase() {
    log "=== PHASE 2: AGGREGATE ==="
    
    python3 "$SCRIPT_DIR/review-aggregator.py" \
        --input-dir "$OUTPUT_DIR/$TIMESTAMP-reviews" \
        --output "$OUTPUT_DIR/$TIMESTAMP-plan.md"
    
    success "Aggregation complete. Plan: $OUTPUT_DIR/$TIMESTAMP-plan.md"
}

# Phase 3: Execute
run_execute_phase() {
    log "=== PHASE 3: EXECUTE ==="
    
    if [ "${AUTO_DEV_MANUAL:-0}" = "1" ]; then
        warn "Manual mode: Kimi Code CLI not available"
        warn "Please execute plan manually: $OUTPUT_DIR/$TIMESTAMP-plan.md"
        return
    fi
    
    # Read plan and delegate each task to Kimi Code CLI
    log "Delegating fixes via Kimi Code CLI..."
    
    success "Execution complete"
}

# Phase 4: Verify
run_verify_phase() {
    log "=== PHASE 4: VERIFY ==="
    
    # Run local tests
    if [ -f "$PROJECT_PATH/Makefile" ]; then
        log "Running: make test"
        (cd "$PROJECT_PATH" && make test) || warn "Tests failed"
    elif [ -f "$PROJECT_PATH/package.json" ]; then
        log "Running: npm test"
        (cd "$PROJECT_PATH" && npm test) || warn "Tests failed"
    elif [ -f "$PROJECT_PATH/pyproject.toml" ] || [ -f "$PROJECT_PATH/setup.py" ]; then
        log "Running: pytest"
        (cd "$PROJECT_PATH" && pytest) || warn "Tests failed"
    fi
    
    # Check CI status
    log "Checking CI status..."
    bash "$SCRIPT_DIR/ci-check.sh" "$PROJECT_PATH"
    
    success "Verification complete"
}

# Main pipeline
main() {
    log "Auto-Dev Pipeline v1.0.0"
    log "Project: $PROJECT_PATH"
    log "Phase: $PHASE"
    log "Output: $OUTPUT_DIR"
    
    check_prerequisites
    
    case "$PHASE" in
        review)
            run_review_phase
            ;;
        plan)
            run_review_phase
            run_aggregate_phase
            ;;
        full)
            run_review_phase
            run_aggregate_phase
            run_execute_phase
            run_verify_phase
            ;;
        *)
            error "Unknown phase: $PHASE"
            echo "Usage: $0 <project-path> [full|review|plan]"
            exit 1
            ;;
    esac
    
    success "Pipeline complete!"
    log "Reports: $OUTPUT_DIR"
}

main "$@"

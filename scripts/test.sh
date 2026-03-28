#!/usr/bin/env bash
set -u

# ============================================================
# prefetch test & verification script
# ============================================================

BOLD='\033[1m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
RED='\033[0;31m'
NC='\033[0m'

PASS=0
FAIL=0
SKIP=0

pass() { echo -e "  ${GREEN}PASS${NC} $*"; PASS=$((PASS + 1)); }
fail() { echo -e "  ${RED}FAIL${NC} $*"; FAIL=$((FAIL + 1)); }
skip() { echo -e "  ${YELLOW}SKIP${NC} $*"; SKIP=$((SKIP + 1)); }

echo -e "${BOLD}prefetch test suite${NC}"
echo "=============================="
echo

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$PROJECT_DIR"

if [ -f "$HOME/.cargo/env" ]; then
    source "$HOME/.cargo/env" 2>/dev/null || true
fi

BIN="./target/release/prefetch"
if [ ! -f "$BIN" ]; then
    BIN="$(command -v prefetch 2>/dev/null || echo "")"
fi
if [ -z "$BIN" ] || [ ! -f "$BIN" ]; then
    echo -e "${RED}Binary not found. Run ./scripts/install.sh first.${NC}"
    exit 1
fi
echo "Using binary: $BIN"
echo

# Shorthand: run the binary with logging suppressed.
# --log-level is a global flag so it goes before the subcommand.
run() { "$BIN" "$@" --log-level error 2>/dev/null; }

# ========================================
echo -e "${BOLD}1. Build & Unit Tests${NC}"
# ========================================

if cargo build --release >/dev/null 2>&1; then
    pass "cargo build --release"
else
    fail "cargo build --release"
fi

TMPFILE=$(mktemp)
if cargo test -p prefetch-gguf -p prefetch-daemon -p prefetch-config >"$TMPFILE" 2>&1; then
    pass "cargo test"
else
    fail "cargo test"
fi
rm -f "$TMPFILE"

echo

# ========================================
echo -e "${BOLD}2. CLI Smoke Tests${NC}"
# ========================================

if run --help | grep -qi "inference\|prefetch\|page cache"; then
    pass "--help"
else
    fail "--help"
fi

if run --version | grep -q "prefetch"; then
    pass "--version"
else
    fail "--version"
fi

if run config path | grep -qi "config"; then
    pass "config path"
else
    fail "config path"
fi

if run config example | grep -q "strategy"; then
    pass "config example"
else
    fail "config example"
fi

if run config show | grep -q "strategy"; then
    pass "config show"
else
    fail "config show"
fi

echo

# ========================================
echo -e "${BOLD}3. Model Discovery${NC}"
# ========================================

DISCOVER_OUTPUT=$(run discover)
if echo "$DISCOVER_OUTPUT" | grep -q "Discovered\|No Ollama models"; then
    pass "discover"
else
    fail "discover"
fi

MODEL_COUNT=$(echo "$DISCOVER_OUTPUT" | grep -c "GB" || true)
if [ "$MODEL_COUNT" -gt 0 ]; then
    pass "found $MODEL_COUNT Ollama model(s)"
    FIRST_MODEL=$(echo "$DISCOVER_OUTPUT" | grep "GB" | head -1 | awk '{print $1}')
else
    skip "no Ollama models found (install with: ollama pull llama3.2:1b)"
    FIRST_MODEL=""
fi

echo

# ========================================
echo -e "${BOLD}4. Cache Status${NC}"
# ========================================

if [ -n "$FIRST_MODEL" ]; then
    if run status "$FIRST_MODEL" | grep -q "GB\|MB"; then
        pass "status $FIRST_MODEL"
    else
        fail "status $FIRST_MODEL"
    fi

    if run status | grep -q "GB\|MB"; then
        pass "status (all models)"
    else
        fail "status (all models)"
    fi
else
    skip "status (no models)"
fi

echo

# ========================================
echo -e "${BOLD}5. Prefetch Warm${NC}"
# ========================================

if [ -n "$FIRST_MODEL" ]; then
    WARM_OUTPUT=$(run warm "$FIRST_MODEL" --force --strategy first-n-layers --layers 2)
    if echo "$WARM_OUTPUT" | grep -q "Completed"; then
        SPEED=$(echo "$WARM_OUTPUT" | grep "Completed" | grep -oE '[0-9]+ MB/s' || echo "unknown")
        pass "warm completed at $SPEED"
    else
        fail "warm $FIRST_MODEL"
    fi

    if run status "$FIRST_MODEL" | grep -q "GB\|MB"; then
        pass "status after warm"
    else
        fail "status after warm"
    fi
else
    skip "warm (no models)"
fi

echo

# ========================================
echo -e "${BOLD}6. GGUF Parser${NC}"
# ========================================

if [ -n "$FIRST_MODEL" ]; then
    BLOB_PATH=$(echo "$DISCOVER_OUTPUT" | grep "$FIRST_MODEL" | awk '{print $NF}')
    if [ -n "$BLOB_PATH" ] && [ -f "$BLOB_PATH" ]; then
        if run status "$BLOB_PATH" | grep -q "block\.\|token_embedding\|GB\|MB"; then
            pass "GGUF layer parsing"
        else
            fail "GGUF layer parsing"
        fi
    else
        skip "blob path not found"
    fi
else
    skip "GGUF parser (no models)"
fi

echo

# ========================================
echo "=============================="
TOTAL=$((PASS + FAIL + SKIP))
echo -e "${BOLD}Results: ${GREEN}$PASS passed${NC}, ${RED}$FAIL failed${NC}, ${YELLOW}$SKIP skipped${NC} (out of $TOTAL)"
echo

if [ "$FAIL" -gt 0 ]; then
    echo -e "${RED}Some tests failed.${NC}"
    exit 1
else
    echo -e "${GREEN}All tests passed!${NC} prefetch is ready to use."
    echo
    echo "Quick start:"
    echo "  prefetch discover         # list models"
    echo "  prefetch status           # check cache"
    echo "  prefetch warm <model>     # warm a model (add --force if low on RAM)"
    echo
fi

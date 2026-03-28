#!/usr/bin/env bash
set -euo pipefail

# ============================================================
# prefetch install script
#
# Installs Rust (if needed), builds the project, and copies
# the binary to a location in your PATH.
# ============================================================

BOLD='\033[1m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
RED='\033[0;31m'
NC='\033[0m'

info()  { echo -e "${GREEN}[+]${NC} $*"; }
warn()  { echo -e "${YELLOW}[!]${NC} $*"; }
error() { echo -e "${RED}[x]${NC} $*"; }

echo -e "${BOLD}prefetch installer${NC}"
echo "=============================="
echo

# --- Check OS ---
OS="$(uname -s)"
case "$OS" in
    Linux|Darwin) info "Detected OS: $OS" ;;
    *) error "Unsupported OS: $OS (only Linux and macOS are supported)"; exit 1 ;;
esac

# --- Check/Install Rust ---
if command -v cargo &>/dev/null; then
    RUST_VER="$(rustc --version)"
    info "Rust already installed: $RUST_VER"
else
    warn "Rust not found. Installing via rustup..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
    info "Rust installed: $(rustc --version)"
fi

# Ensure cargo is in PATH for this session
if ! command -v cargo &>/dev/null; then
    if [ -f "$HOME/.cargo/env" ]; then
        source "$HOME/.cargo/env"
    else
        error "cargo not found in PATH after install. Please restart your shell and re-run."
        exit 1
    fi
fi

# --- Find project root ---
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$PROJECT_DIR"

if [ ! -f "Cargo.toml" ]; then
    error "Cargo.toml not found in $PROJECT_DIR. Are you in the prefetch directory?"
    exit 1
fi

info "Building from: $PROJECT_DIR"

# --- Build ---
info "Building release binary..."
cargo build --release 2>&1

if [ ! -f "target/release/prefetch" ]; then
    error "Build failed: binary not found"
    exit 1
fi

info "Build complete"

# --- Install ---
INSTALL_DIR="$HOME/.cargo/bin"
mkdir -p "$INSTALL_DIR"

cp target/release/prefetch "$INSTALL_DIR/prefetch"
chmod +x "$INSTALL_DIR/prefetch"

info "Installed to: $INSTALL_DIR/prefetch"

# --- Verify ---
if command -v prefetch &>/dev/null; then
    info "Verification: $(prefetch --version)"
else
    warn "prefetch is installed but not in your current PATH."
    warn "Add this to your shell profile:"
    echo
    echo "    export PATH=\"\$HOME/.cargo/bin:\$PATH\""
    echo
fi

echo
echo -e "${GREEN}Installation complete!${NC}"
echo
echo "Next steps:"
echo "  1. Run the test suite:  ./scripts/test.sh"
echo "  2. Discover models:     prefetch discover"
echo "  3. Warm a model:        prefetch warm <model> --force"
echo

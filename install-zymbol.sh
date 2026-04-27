#!/bin/bash
# Installation script for Zymbol-Lang
# Builds release binaries and creates global symlinks

set -e

echo "========================================="
echo "Zymbol-Lang Installation"
echo "========================================="
echo ""

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
ZYMBOL_BIN="$SCRIPT_DIR/target/release/zymbol"
LSP_BIN="$SCRIPT_DIR/target/release/zymbol-lsp"

echo "Building release binaries..."
echo "  cargo build --release --bin zymbol --bin zymbol-lsp"
echo ""

cd "$SCRIPT_DIR"
cargo build --release --bin zymbol --bin zymbol-lsp

echo ""
echo "Installing symlinks in /usr/bin/ (requires sudo)..."
echo ""

sudo ln -sf "$ZYMBOL_BIN" /usr/bin/zymbol
echo "  /usr/bin/zymbol     -> $ZYMBOL_BIN"

sudo ln -sf "$LSP_BIN" /usr/bin/zymbol-lsp
echo "  /usr/bin/zymbol-lsp -> $LSP_BIN"

echo ""
echo "========================================="
echo "Installation complete"
echo "========================================="
echo ""
/usr/bin/zymbol -V
echo ""
echo "Verification:"
echo "  zymbol:     $(readlink -f /usr/bin/zymbol)"
echo "  zymbol-lsp: $(readlink -f /usr/bin/zymbol-lsp)"
echo "========================================="

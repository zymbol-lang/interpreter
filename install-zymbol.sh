#!/bin/bash
# Installation script for Zymbol-Lang
# Creates a global symlink to the zymbol executable

set -e

echo "========================================="
echo "Zymbol-Lang Installation"
echo "========================================="
echo ""

# Get the directory where this script is located
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
ZYMBOL_BIN="$SCRIPT_DIR/target/release/zymbol"

# Check if the release binary exists
if [ ! -f "$ZYMBOL_BIN" ]; then
    echo "❌ Error: Release binary not found at $ZYMBOL_BIN"
    echo "Please build the project first:"
    echo "  cargo build --release"
    exit 1
fi

# Create symlink in /usr/bin/
echo "📦 Creating symlink in /usr/bin/..."
echo "This requires sudo permissions."
echo ""

sudo ln -sf "$ZYMBOL_BIN" /usr/bin/zymbol

# Verify installation
if command -v zymbol &> /dev/null; then
    echo ""
    echo "========================================="
    echo "✅ Installation successful!"
    echo "========================================="
    echo "Zymbol-Lang is now available globally."
    echo ""
    echo "Usage:"
    echo "  zymbol run <file.zy>    # Run a Zymbol file"
    echo "  zymbol --help           # Show help"
    echo ""
    echo "Version:"
    zymbol --version
    echo ""
    echo "Location:"
    echo "  Executable: $ZYMBOL_BIN"
    echo "  Symlink:    /usr/bin/zymbol"
    echo "========================================="
else
    echo "❌ Installation failed. Please check for errors above."
    exit 1
fi

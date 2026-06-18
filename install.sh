#!/bin/bash
# Motif installer for Linux/macOS
# Run: curl -fsSL https://raw.githubusercontent.com/Cashmeran/Motif/main/install.sh | bash

set -e

echo "Installing Motif..."

if ! command -v cargo &>/dev/null; then
    echo "Rust is not installed. Install it from https://rustup.rs"
    exit 1
fi

cargo install --git https://github.com/Cashmeran/Motif.git

echo "✓ Motif installed. Run 'motif' to start."

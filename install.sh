#!/bin/bash
# Motif installer for Linux/macOS
# Run: curl -fsSL https://raw.githubusercontent.com/Cashmeran/Motif/main/install.sh | bash

set -e

REPO="Cashmeran/Motif"
VERSION="v0.3.0"

echo "Installing Motif ${VERSION}..."

# Detect OS and architecture
OS=$(uname -s)
ARCH=$(uname -m)

case "${OS}-${ARCH}" in
    Linux-x86_64)  BINARY="motif-linux-x86_64" ;;
    Linux-aarch64) BINARY="motif-linux-aarch64" ;;
    Darwin-x86_64) BINARY="motif-macos-x86_64" ;;
    Darwin-arm64)  BINARY="motif-macos-arm64" ;;
    *)
        echo "No pre-built binary for ${OS}-${ARCH}. Building from source..."
        if ! command -v cargo &>/dev/null; then
            echo "Rust is not installed. Install it from https://rustup.rs"
            exit 1
        fi
        cargo install --git "https://github.com/${REPO}.git" motif-cli
        echo "✓ Motif installed from source. Run 'motif' to start."
        exit 0
        ;;
esac

# Download pre-built binary
URL="https://github.com/${REPO}/releases/download/${VERSION}/${BINARY}"
DEST="/usr/local/bin/motif"

echo "  Downloading ${BINARY}..."
if command -v curl &>/dev/null; then
    curl -fsSL "${URL}" -o /tmp/motif
elif command -v wget &>/dev/null; then
    wget -q "${URL}" -O /tmp/motif
else
    echo "Neither curl nor wget found."
    exit 1
fi

chmod +x /tmp/motif

# Install (may need sudo)
if [ -w /usr/local/bin ]; then
    mv /tmp/motif "${DEST}"
else
    sudo mv /tmp/motif "${DEST}"
fi

echo "✓ Motif ${VERSION} installed to ${DEST}"
echo "  Run 'motif' to start."

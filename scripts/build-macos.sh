#!/usr/bin/env bash
set -euo pipefail

echo "=== Building RustyBox (macOS) ==="

MODE="${1:-release}"
TARGET="${2:-}"

ARCH_ARG=""
if [ "$TARGET" = "x86_64-apple-darwin" ]; then
    ARCH_ARG="amd64"
elif [ "$TARGET" = "aarch64-apple-darwin" ]; then
    ARCH_ARG="arm64"
fi

# 1. Ensure sing-box is downloaded
if [ ! -f "bin/sing-box" ]; then
    echo "sing-box binary not found. Downloading..."
    ./scripts/download-singbox.sh 1.14.3 bin "$ARCH_ARG"
fi

# 2. Build frontend
echo "Building frontend..."
pushd frontend
npm install
npm run build
popd

# 3. Build Rust application
echo "Building Rust application..."
TARGET_ARGS=()
if [ -n "$TARGET" ]; then
    TARGET_ARGS=(--target "$TARGET")
fi

if [ "$MODE" = "debug" ]; then
    cargo build --package rustybox --bin rustybox "${TARGET_ARGS[@]}"
    echo "Debug binary built."
else
    cargo build --release --package rustybox --bin rustybox "${TARGET_ARGS[@]}"
    echo "Release binary built."
fi

echo "To bundle .app/.dmg with Tauri: npx @tauri-apps/cli build ${TARGET_ARGS[*]}"
echo "To run TUN mode on macOS, use 'Restart as Administrator' in the UI or launch with sudo."

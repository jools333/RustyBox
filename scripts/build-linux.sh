#!/usr/bin/env bash
set -euo pipefail

echo "=== Building RustyBox (Linux) ==="

# 1. Ensure sing-box is downloaded
if [ ! -f "bin/sing-box" ]; then
    echo "sing-box binary not found. Downloading..."
    ./scripts/download-singbox.sh
fi

# 2. Build frontend
echo "Building frontend..."
pushd frontend
npm install
npm run build
popd

# 3. Build Rust application
echo "Building Rust application..."
if [ "${1:-release}" = "debug" ]; then
    cargo build --package rustybox --bin rustybox
    echo "Debug binary: target/debug/rustybox"
else
    cargo build --release --package rustybox --bin rustybox
    echo "Release binary: target/release/rustybox"
fi

echo "Setting capabilities for TUN operations (optional if running root):"
echo "sudo setcap cap_net_admin,cap_net_bind_service=+ep target/release/rustybox"

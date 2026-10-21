#!/usr/bin/env bash
set -euo pipefail

VERSION="${1:-1.14.3}"
TARGET_DIR="${2:-bin}"

echo "=== RustyBox: Downloading sing-box v$VERSION for Linux ==="

mkdir -p "$TARGET_DIR"

ARCH=$(uname -m)
case "$ARCH" in
    x86_64) SB_ARCH="amd64" ;;
    aarch64|arm64) SB_ARCH="arm64" ;;
    *) echo "Unsupported architecture: $ARCH"; exit 1 ;;
esac

TAR_NAME="sing-box-${VERSION}-linux-${SB_ARCH}.tar.gz"
URL="https://github.com/SagerNet/sing-box/releases/download/v${VERSION}/${TAR_NAME}"

echo "Downloading from $URL..."
curl -fsSL "$URL" -o "${TARGET_DIR}/${TAR_NAME}"

tar -xzf "${TARGET_DIR}/${TAR_NAME}" -C "${TARGET_DIR}" --strip-components=1 --wildcards '*/sing-box'
rm -f "${TARGET_DIR}/${TAR_NAME}"
chmod +x "${TARGET_DIR}/sing-box"

echo "sing-box binary installed to ${TARGET_DIR}/sing-box"
"${TARGET_DIR}/sing-box" version

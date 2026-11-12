#!/usr/bin/env bash
set -euo pipefail

VERSION="${1:-1.14.3}"
TARGET_DIR="${2:-bin}"
ARCH_INPUT="${3:-$(uname -m)}"

OS_NAME=$(uname -s)
case "$OS_NAME" in
    Linux) SB_OS="linux" ;;
    Darwin) SB_OS="darwin" ;;
    *) echo "Unsupported OS: $OS_NAME"; exit 1 ;;
esac

case "$ARCH_INPUT" in
    x86_64|amd64) SB_ARCH="amd64" ;;
    aarch64|arm64) SB_ARCH="arm64" ;;
    *) echo "Unsupported architecture: $ARCH_INPUT"; exit 1 ;;
esac

echo "=== RustyBox: Downloading sing-box v$VERSION for ${SB_OS}/${SB_ARCH} ==="

mkdir -p "$TARGET_DIR"

TAR_NAME="sing-box-${VERSION}-${SB_OS}-${SB_ARCH}.tar.gz"
URL="https://github.com/SagerNet/sing-box/releases/download/v${VERSION}/${TAR_NAME}"

echo "Downloading from $URL..."
curl -fsSL "$URL" -o "${TARGET_DIR}/${TAR_NAME}"

TMP_EXTRACT=$(mktemp -d)
trap 'rm -rf "$TMP_EXTRACT"' EXIT
tar -xzf "${TARGET_DIR}/${TAR_NAME}" -C "$TMP_EXTRACT"
SB_BIN=$(find "$TMP_EXTRACT" -name sing-box -type f | head -n 1)
if [ -z "$SB_BIN" ]; then
    echo "Failed to find sing-box binary inside ${TAR_NAME}"
    exit 1
fi
mv "$SB_BIN" "${TARGET_DIR}/sing-box"
rm -f "${TARGET_DIR}/${TAR_NAME}"
chmod +x "${TARGET_DIR}/sing-box"

echo "sing-box binary installed to ${TARGET_DIR}/sing-box"
HOST_ARCH=$(uname -m)
case "$HOST_ARCH" in
    x86_64) HOST_SB_ARCH="amd64" ;;
    aarch64|arm64) HOST_SB_ARCH="arm64" ;;
    *) HOST_SB_ARCH="" ;;
esac
if [ "$SB_ARCH" = "$HOST_SB_ARCH" ]; then
    "${TARGET_DIR}/sing-box" version
fi

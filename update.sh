#!/usr/bin/env bash
set -e

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$REPO_DIR"

echo "=== Проверка релизов RustyBox на GitHub... ==="
LATEST_DEB=$(curl -s https://api.github.com/repos/jools333/RustyBox/releases/latest | grep "browser_download_url.*deb" | head -n 1 | cut -d '"' -f 4 || true)

if [ -n "$LATEST_DEB" ] && [ "$1" != "--build" ]; then
    echo "Найден официальный deb-релиз: $LATEST_DEB"
    echo "Скачивание и быстрая распаковка бинарника..."
    TMP_DEB=$(mktemp --suffix=.deb)
    curl -sL "$LATEST_DEB" -o "$TMP_DEB"
    mkdir -p "$HOME/.local/bin"
    ar p "$TMP_DEB" data.tar.gz 2>/dev/null | tar -xz -O usr/bin/rustybox > "$HOME/.local/bin/rustybox" || \
    ar p "$TMP_DEB" data.tar.xz 2>/dev/null | tar -xJ -O usr/bin/rustybox > "$HOME/.local/bin/rustybox"
    chmod +x "$HOME/.local/bin/rustybox"
    rm -f "$TMP_DEB"
    echo "=== Готово! RustyBox успешно обновлен из официального deb-релиза в ~/.local/bin/rustybox ==="
    exit 0
fi

echo "=== Сборка из исходников (Git) ==="
git pull

echo "=== 1. Сборка фронтенда (React / Vite) ==="
cd "$REPO_DIR/frontend"
npm install
npm run build

echo "=== 2. Сборка бинарника (Rust / Tauri) ==="
cd "$REPO_DIR"
if command -v nix-shell >/dev/null 2>&1; then
    nix-shell -p pkg-config glib gtk3 webkitgtk_4_1 libappindicator-gtk3 librsvg openssl --run '
        export PATH="$HOME/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin:$PATH"
        cargo build --release --package rustybox --bin rustybox
    '
else
    cargo build --release --package rustybox --bin rustybox
fi

echo "=== 3. Установка новой версии ==="
mkdir -p "$HOME/.local/bin"
cp "$REPO_DIR/target/release/rustybox" "$HOME/.local/bin/rustybox"
chmod +x "$HOME/.local/bin/rustybox"

if [ -f "$REPO_DIR/app-icon.svg" ]; then
    mkdir -p "$HOME/.local/share/icons/hicolor/scalable/apps"
    cp "$REPO_DIR/app-icon.svg" "$HOME/.local/share/icons/hicolor/scalable/apps/rustybox.svg"
fi

echo "=== Готово! RustyBox успешно обновлен и установлен в ~/.local/bin/rustybox ==="

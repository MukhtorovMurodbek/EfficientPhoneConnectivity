#!/bin/sh
# Build epcd and install it for the current user (no root needed):
#   ~/.local/bin/epcd, an app entry (for notification grouping) and a login autostart.
set -e
cd "$(dirname "$0")"
PATH="$HOME/.cargo/bin:$PATH"
cargo build --release
install -Dm755 target/release/epcd "$HOME/.local/bin/epcd"
install -Dm644 dist/epcd.desktop "$HOME/.local/share/applications/epcd.desktop"
mkdir -p "$HOME/.config/autostart"
sed "s|^Exec=epcd|Exec=$HOME/.local/bin/epcd|" dist/epcd.desktop > "$HOME/.config/autostart/epcd.desktop"
pkill -x epcd 2>/dev/null || true
setsid -f "$HOME/.local/bin/epcd" >/dev/null 2>&1
echo "Installed and started. epcd will also start on every login."

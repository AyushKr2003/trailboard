#!/usr/bin/env bash
# Install Trailboard's desktop entry and icon for the current user, so it shows
# in app launchers (Omarchy's included) and can open links from other apps.
#
#   scripts/install-desktop.sh [BINARY]   # default: target/release/trailboard
#   scripts/install-desktop.sh --uninstall
#
# The entry runs BINARY by its absolute path, so rebuild in place to update.
# To make Trailboard the default browser afterwards:
#   xdg-settings set default-web-browser trailboard.desktop
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DATA="${XDG_DATA_HOME:-$HOME/.local/share}"
APPS="$DATA/applications"
ICONS="$DATA/icons/hicolor/scalable/apps"

refresh() {
    command -v update-desktop-database >/dev/null && update-desktop-database -q "$APPS" || true
    command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache -qtf "$DATA/icons/hicolor" || true
}

if [[ "${1:-}" == "--uninstall" ]]; then
    rm -f "$APPS/trailboard.desktop" "$ICONS/trailboard.svg"
    refresh
    echo "Removed the Trailboard desktop entry and icon."
    exit 0
fi

BIN="$(realpath -m "${1:-$ROOT/target/release/trailboard}")"
if [[ ! -x "$BIN" ]]; then
    echo "No Trailboard binary at $BIN; build it first (cargo build --release) or pass its path." >&2
    exit 1
fi

mkdir -p "$APPS" "$ICONS"
# Quote the path for the Exec key (spaces and the characters it reserves).
EXEC="\"$(printf '%s' "$BIN" | sed 's/[\\"`$]/\\\\&/g')\""
while IFS= read -r line; do
    [[ "$line" == "Exec=trailboard "* ]] && line="Exec=$EXEC ${line#Exec=trailboard }"
    printf '%s\n' "$line"
done < "$ROOT/share/trailboard.desktop" > "$APPS/trailboard.desktop.tmp"
mv "$APPS/trailboard.desktop.tmp" "$APPS/trailboard.desktop"
install -m 644 "$ROOT/share/trailboard.svg" "$ICONS/trailboard.svg"
refresh
echo "Installed $APPS/trailboard.desktop (runs $BIN) and its icon."

#!/usr/bin/env bash
# Build a portable Trailboard for any x86_64 distro: the release binary with
# the CEF runtime beside it (rpath $ORIGIN), plus the desktop entry, icon and
# docs, packed as target/bundle/trailboard-VERSION-ARCH.tar.zst.
#
#   scripts/bundle.sh            # CEF from $CEF_PATH (default ~/.local/share/cef)
#
# Unpack it anywhere and run ./trailboard URL.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CEF="${CEF_PATH:-$HOME/.local/share/cef}"

if [[ ! -f "$CEF/libcef.so" ]]; then
    echo "No CEF at $CEF; run scripts/install-cef.sh or set CEF_PATH." >&2
    exit 1
fi

cd "$ROOT"
CEF_PATH="$CEF" cargo build --release -p trailboard
VERSION="$(./target/release/trailboard --version | awk '{print $2}')"
NAME="trailboard-$VERSION-$(uname -m)"
OUT="$ROOT/target/bundle/$NAME"

rm -rf "$OUT"
mkdir -p "$OUT/share/applications" "$OUT/share/icons/hicolor/scalable/apps" \
    "$OUT/share/icons/hicolor/symbolic/apps"
install -m 755 target/release/trailboard "$OUT/trailboard"

# CEF's runtime: libraries, data files and locales. Not its headers, CMake
# files or wrapper sources. `chrome-sandbox` comes along but is only used
# where user namespaces are off, and only once made setuid root (see docs/GUIDE.md).
for f in "$CEF"/*; do
    case "$(basename "$f")" in
        include | cmake | libcef_dll | CMakeLists.txt | archive.json) ;;
        *) cp -a "$f" "$OUT/" ;;
    esac
done
# The CEF distribution ships libcef.so with its symbols (~1.4 GB); strip them.
strip --strip-unneeded "$OUT/libcef.so" "$OUT"/libvk_swiftshader.so "$OUT"/libvulkan.so.1 2>/dev/null || true
[[ -f "$OUT/CREDITS.html" ]] && mv "$OUT/CREDITS.html" "$OUT/CEF-CREDITS.html"

install -m 644 share/trailboard.desktop "$OUT/share/applications/trailboard.desktop"
install -m 644 share/trailboard.svg "$OUT/share/icons/hicolor/scalable/apps/trailboard.svg"
install -m 644 share/trailboard-symbolic.svg \
    "$OUT/share/icons/hicolor/symbolic/apps/trailboard-symbolic.svg"
install -m 644 README.md CHANGELOG.md docs/GUIDE.md "$OUT/"

tar -C "$ROOT/target/bundle" --zstd -cf "$OUT.tar.zst" "$NAME"
echo "Packed $OUT.tar.zst ($(du -sh "$OUT.tar.zst" | cut -f1))"

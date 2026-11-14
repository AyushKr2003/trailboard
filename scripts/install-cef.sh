#!/usr/bin/env bash
# Install the CEF binary distribution that matches the `cef` crate into
# $CEF_PATH (default ~/.local/share/cef), laid out the way cef-dll-sys expects.
#
# `export-cef-dir` currently fails because CEF's build index is larger than its
# 10 MB response limit, so this fetches the index with curl instead.
set -euo pipefail

CEF_VERSION_PREFIX="${CEF_VERSION_PREFIX:-156.0.26+}"
PLATFORM="${PLATFORM:-linux64}"
TARGET="${CEF_PATH:-$HOME/.local/share/cef}"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

echo "Fetching CEF build index…"
curl -fsSL -o "$WORK/index.json" https://cef-builds.spotifycdn.com/index.json

read -r NAME SHA1 < <(python3 -I - "$WORK/index.json" "$PLATFORM" "$CEF_VERSION_PREFIX" <<'EOF'
import json, sys
idx = json.load(open(sys.argv[1]))
for v in idx[sys.argv[2]]["versions"]:
    if v["cef_version"].startswith(sys.argv[3]):
        for f in v["files"]:
            if f["type"] == "minimal":
                print(f["name"], f["sha1"])
                sys.exit(0)
sys.exit("no matching CEF build")
EOF
)

echo "Downloading $NAME…"
URL="https://cef-builds.spotifycdn.com/$(python3 -I -c 'import sys,urllib.parse; print(urllib.parse.quote(sys.argv[1]))' "$NAME")"
curl -fSL -o "$WORK/$NAME" "$URL"
echo "$SHA1  $WORK/$NAME" | sha1sum -c -

echo "Extracting to $TARGET…"
tar -xjf "$WORK/$NAME" -C "$WORK"
SRC="$WORK/${NAME%.tar.bz2}"
mkdir -p "$TARGET"
cp -a "$SRC/Release/." "$SRC/Resources/." "$TARGET/"
cp -a "$SRC/CMakeLists.txt" "$SRC/cmake" "$SRC/include" "$SRC/libcef_dll" "$SRC/CREDITS.html" "$TARGET/"
printf '{"type": "minimal", "name": "%s", "sha1": "%s"}\n' "$NAME" "$SHA1" > "$TARGET/archive.json"
echo "Done. Set CEF_PATH=$TARGET when building."

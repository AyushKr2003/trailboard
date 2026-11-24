#!/usr/bin/env bash
# Install the Chromium engine (CEF) Trailboard builds and runs against, laid
# out the way the `cef` crate's build script (cef-dll-sys) expects.
#
#   scripts/install-cef.sh              # the standard CEF build (default)
#   scripts/install-cef.sh --codecs     # our media build: H.264, AAC, HEVC
#   scripts/install-cef.sh --from FILE  # a media build you made yourself
#   scripts/install-cef.sh --help
#
# Both builds are pinned to the exact version the `cef` crate needs and are
# checked against a SHA-256 before anything is installed. The install goes
# to $CEF_PATH (default ~/.local/share/cef) and replaces what is there only
# once the new copy is complete. Run it again to switch between the two.
#
# --codecs: the standard CEF can't play H.264 or AAC (most video outside
# YouTube) because those codecs are patent-licensed. This fetches the same
# CEF version built with them turned on by this repo's `cef-build` branch
# (GitHub Actions; see the README). --from installs such a build from a
# local tar.bz2 (checked against FILE.sha256 when that file is beside it).
# Media builds are for your own use: `scripts/bundle.sh` refuses them.
#
# Keep the pins below in step with `packaging/arch/PKGBUILD` and the `cef`
# version in `crates/web/Cargo.toml`.
set -euo pipefail

CEF_VERSION="156.0.26+g8ea0c73+chromium-156.0.8078.88"

# Standard build: CEF's own download.
STD_NAME="cef_binary_${CEF_VERSION}_linux64_minimal.tar.bz2"
STD_URL="https://cef-builds.spotifycdn.com/${STD_NAME//+/%2B}"
STD_SHA1="cacfbb7d7261fea682fb67c108de7e4347519ff7"
STD_SHA256="600ac0b8f37cb953f884e7c89998a6c200d24959aa74820ad431b61d5ebcf360"
STD_MB=3000 # free space needed (download + unpacked + install)

# Media build: the same CEF commit with proprietary_codecs=true,
# ffmpeg_branding=Chrome, HEVC (VA-API) and Widevine, built by this repo's
# `cef-build` branch and published as the release below.
COD_TAG="cef-156.0.26-media"
COD_NAME="cef_binary_${CEF_VERSION}_linux64_minimal.tar.bz2"
COD_URL="https://github.com/AyushKr2003/trailboard/releases/download/$COD_TAG/${COD_NAME//+/%2B}"
COD_SHA1="0cc2812cde0dbd539fa93509ad583f2f93cc9a88"
COD_SHA256="64577aa57bbbf3025354ba65c9ad94ca4fd9c3cd5e18ae9954354639c45f821a"
COD_SOURCE="https://github.com/AyushKr2003/trailboard/releases/tag/$COD_TAG"
COD_MB=3000

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET="${CEF_PATH:-$HOME/.local/share/cef}"
CODECS=0
FORCE=0
FROM=""

die() { echo "install-cef: $*" >&2; exit 1; }
say() { echo "==> $*"; }

usage() {
    sed -n '2,/^set -euo/p' "${BASH_SOURCE[0]}" | sed '$d; s/^# \{0,1\}//'
    cat <<EOF
Options:
  --codecs     install the media build (H.264, AAC, HEVC)
  --from FILE  install a media build from a local cef_binary_*.tar.bz2
  --standard   install the standard build (the default)
  --force      reinstall even if this build is already installed
  -h, --help   show this help

Environment:
  CEF_PATH     where to install (now: $TARGET)
EOF
}

while (($#)); do
    case "$1" in
        --codecs) CODECS=1 ;;
        --standard) CODECS=0 FROM="" ;;
        --from)
            (($# >= 2)) || die "--from needs a file"
            FROM="$2" CODECS=1
            shift
            ;;
        --from=*) FROM="${1#--from=}" CODECS=1 ;;
        --force) FORCE=1 ;;
        -h | --help) usage; exit 0 ;;
        *) usage >&2; die "unknown option: $1" ;;
    esac
    shift
done

# ---- checks before touching anything -------------------------------------

[[ "$(uname -s)" == Linux ]] || die "Trailboard runs on Linux only."
[[ "$(uname -m)" == x86_64 ]] || die "only x86_64 is supported (this machine is $(uname -m))."

need=(curl sha256sum sha1sum tar bzip2)
missing=()
for t in "${need[@]}"; do command -v "$t" >/dev/null || missing+=("$t"); done
((${#missing[@]} == 0)) || die "missing tools: ${missing[*]} (install them with your package manager)."

if [[ -n "$FROM" ]]; then
    [[ -f "$FROM" ]] || die "no such file: $FROM"
    FROM="$(cd "$(dirname "$FROM")" && pwd)/$(basename "$FROM")"
    NAME="$(basename "$FROM")"
    [[ "$NAME" == "cef_binary_${CEF_VERSION}_linux64_minimal.tar.bz2" ]] ||
        die "$NAME isn't CEF $CEF_VERSION (cef_binary_${CEF_VERSION}_linux64_minimal.tar.bz2), the version the cef crate needs."
    if [[ -f "$FROM.sha256" ]]; then
        (cd "$(dirname "$FROM")" && sha256sum -c --status "$NAME.sha256") ||
            die "$NAME doesn't match $NAME.sha256."
        say "Checksum OK ($NAME.sha256)"
    else
        echo "Note: no $NAME.sha256 beside it; installing unchecked." >&2
    fi
    SHA256="$(sha256sum "$FROM" | cut -d' ' -f1)"
    SHA1="$(sha1sum "$FROM" | cut -d' ' -f1)"
    URL="" MB=2500 KIND=media SOURCE="file://$FROM"
elif ((CODECS)); then
    NAME=$COD_NAME URL=$COD_URL SHA1=$COD_SHA1 SHA256=$COD_SHA256 MB=$COD_MB KIND=media SOURCE=$COD_SOURCE
else
    NAME=$STD_NAME URL=$STD_URL SHA1=$STD_SHA1 SHA256=$STD_SHA256 MB=$STD_MB KIND=standard SOURCE=""
fi

# The archive's SHA-256 is in the archive.json this script writes, so the
# same build isn't installed twice (and a different one with the same
# version, as an earlier third-party codecs build, is replaced).
installed_sha256() {
    local j="$TARGET/archive.json"
    [[ -f "$j" && -f "$TARGET/libcef.so" ]] || return 0
    sed -nE 's/.*"sha256": *"([0-9a-f]{64})".*/\1/p' "$j"
}
if [[ "$(installed_sha256)" == "$SHA256" && $FORCE == 0 ]]; then
    say "This $KIND CEF $CEF_VERSION is already installed in $TARGET (use --force to reinstall)."
    INSTALLED=0
else
    INSTALLED=1
fi

if ((INSTALLED)); then
    if pgrep -x trailboard >/dev/null; then
        die "Trailboard is running; quit it first (it has the engine open)."
    fi

    PARENT="$(dirname "$TARGET")"
    mkdir -p "$PARENT"
    avail=$(df -Pm "$PARENT" | awk 'NR==2 {print $4}')
    ((avail >= MB)) || die "needs about $MB MB free in $PARENT, only $avail MB available."

    # Work next to the target, on disk: /tmp is often RAM-backed and the
    # engine is large. Downloads are kept there so a retry can reuse them.
    WORK="$TARGET.partial"
    DL="$PARENT/.cef-downloads"
    mkdir -p "$DL"
    rm -rf "$WORK"
    trap 'rm -rf "$WORK"' EXIT

    # Both builds have the same file name: keep their downloads apart.
    FILE="$DL/$KIND-$NAME"
    if [[ -n "$FROM" ]]; then
        FILE="$FROM"
    elif [[ -f "$FILE" ]] && echo "$SHA256  $FILE" | sha256sum -c --status -; then
        say "Using the already downloaded $NAME"
    else
        say "Downloading the $KIND CEF ($NAME)…"
        curl -fL --retry 3 --retry-delay 2 -C - -o "$FILE.part" "$URL" ||
            curl -fL --retry 3 --retry-delay 2 -o "$FILE.part" "$URL" ||
            die "download failed: $URL"
        echo "$SHA256  $FILE.part" | sha256sum -c --status - || {
            rm -f "$FILE.part"
            die "the download doesn't match its SHA-256; deleted it. Try again."
        }
        mv "$FILE.part" "$FILE"
    fi
    [[ -n "$FROM" ]] || say "Checksum OK"

    say "Unpacking…"
    mkdir -p "$WORK/x"
    tar -xjf "$FILE" -C "$WORK/x"
    SRC="$WORK/x/${NAME%.tar.bz2}"
    [[ -f "$SRC/Release/libcef.so" && -d "$SRC/Resources" && -d "$SRC/include" ]] ||
        die "the archive doesn't have the expected CEF layout."

    NEW="$WORK/cef"
    mkdir -p "$NEW"
    mv "$SRC"/Release/* "$SRC"/Resources/* "$NEW/"
    mv "$SRC"/{CMakeLists.txt,cmake,include,libcef_dll,CREDITS.html} "$NEW/"
    # `name` and `sha1` are what cef-dll-sys reads (it only checks the
    # version in `name`). `codecs` marks a media build: Trailboard names it
    # in its version line, and scripts/bundle.sh refuses it.
    if ((CODECS)); then
        printf '{"type": "minimal", "name": "%s", "sha1": "%s", "sha256": "%s", "codecs": true, "source": "%s"}\n' \
            "$NAME" "$SHA1" "$SHA256" "$SOURCE" >"$NEW/archive.json"
    else
        printf '{"type": "minimal", "name": "%s", "sha1": "%s", "sha256": "%s"}\n' \
            "$NAME" "$SHA1" "$SHA256" >"$NEW/archive.json"
    fi

    # Swap it in: the old copy goes only once the new one is complete.
    say "Installing into $TARGET…"
    rm -rf "$TARGET.old"
    [[ -e "$TARGET" ]] && mv "$TARGET" "$TARGET.old"
    mv "$NEW" "$TARGET"
    rm -rf "$TARGET.old" "$WORK"
    trap - EXIT
    # The download is only kept to resume a failed run (never delete --from).
    [[ -n "$FROM" ]] || rm -f "$FILE"
    rmdir "$DL" 2>/dev/null || true
fi

# ---- the local cargo config a source build needs ---------------------------

CONFIG="$ROOT/.cargo/config.toml"
if [[ -d "$ROOT/crates" ]]; then
    if [[ ! -f "$CONFIG" ]]; then
        mkdir -p "$ROOT/.cargo"
        cat >"$CONFIG" <<EOF
# Local developer config (absolute paths), written by scripts/install-cef.sh.
# Not committed.
[env]
CEF_PATH = "$TARGET"

[target.x86_64-unknown-linux-gnu]
rustflags = ["-C", "link-arg=-Wl,-rpath,$TARGET"]
EOF
        say "Wrote $CONFIG (CEF_PATH and rpath for cargo)."
    elif ! grep -qF "CEF_PATH = \"$TARGET\"" "$CONFIG"; then
        echo "Note: $CONFIG doesn't point CEF_PATH at $TARGET; check it." >&2
    fi
fi

if ((INSTALLED)); then
    say "Done: the $KIND CEF $CEF_VERSION is in $TARGET."
fi
if ((CODECS)); then
    echo "    H.264, AAC and (with VA-API) HEVC will play. This build is for your own use; scripts/bundle.sh won't package it."
    echo "    Back to the standard build: scripts/install-cef.sh --standard"
fi
echo "    Next: cargo run --release -p trailboard"

#!/usr/bin/env bash
# Build CEF with full media support (see ../cef.conf), either in one go on a
# big machine or as one time-boxed stage of the GitHub Actions chain.
#
#   scripts/build-cef.sh                   # one go: sync, build, package
#   STAGE_DEADLINE=<epoch> scripts/build-cef.sh
#                                          # one stage: stop compiling at the
#                                          # deadline and leave a checkpoint
#
# Directories (override with env):
#   CEF_WORK  source checkout and build output (~80 GB) default /mnt/cef
#   CKPT_IN   checkpoint to resume from       default (none)
#   CKPT_OUT  where to write a checkpoint     default $HOME/ckpt
#   DIST_OUT  where the finished archive goes default $HOME/dist
#
# How resuming works: every stage syncs the same pinned commits, sets every
# source file's mtime to one fixed date, and unpacks the previous stage's
# output directory (newer mtimes) on top. Ninja/Siso then see only the
# unfinished steps as dirty. The output directory is all that has to travel
# between jobs. It stays at src/out/: the build files use paths relative
# to it (../../buildtools/…), so it can't live behind a symlink.
#
# Outputs (for GitHub Actions, when $GITHUB_OUTPUT is set): done=true|false.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=../cef.conf
source "$HERE/cef.conf"

W="${CEF_WORK:-/mnt/cef}"
CKPT_IN="${CKPT_IN:-}"
CKPT_OUT="${CKPT_OUT:-$HOME/ckpt}"
DIST_OUT="${DIST_OUT:-$HOME/dist}"
DEADLINE="${STAGE_DEADLINE:-0}"
OUTDIR_NAME=Release_GN_x64

say() { echo "::group::$*" 2>/dev/null; echo "==> $*"; }
end() { echo "::endgroup::" 2>/dev/null || true; }
out() { [[ -n "${GITHUB_OUTPUT:-}" ]] && echo "$1" >>"$GITHUB_OUTPUT" || true; }
disk() { df -h "$W" "$CKPT_OUT" 2>/dev/null | sort -u; }

mkdir -p "$W" "$CKPT_OUT" "$DIST_OUT"
export GN_DEFINES="${GN_DEFINES//$'\n'/ }"
export CEF_ARCHIVE_FORMAT=tar.bz2
export DEPOT_TOOLS_UPDATE=1
echo "GN_DEFINES=$GN_DEFINES"
disk

AUTOMATE="$W/automate-git.py"
if [[ ! -f "$AUTOMATE" ]]; then
    curl -fsSL -o "$AUTOMATE" \
        "https://raw.githubusercontent.com/chromiumembedded/cef/master/tools/automate/automate-git.py"
fi

common=(
    --download-dir="$W"
    --branch="$CEF_BRANCH"
    --checkout="$CEF_COMMIT"
    --x64-build
    --no-debug-build
    --no-chromium-history
)

# ---- 1. sync the pinned sources and generate the build files ---------------
say "Sync CEF $CEF_COMMIT (branch $CEF_BRANCH) and Chromium"
python3 "$AUTOMATE" "${common[@]}" --no-build --no-distrib
end
SRC="$W/chromium/src"
export PATH="$W/depot_tools:$PATH"

# automate-git.py only generates the build files in its build step, which
# we run ourselves (time-boxed), so generate them here: Release x64 only,
# with the x64 sysroot installed.
say "Generate the build files (gn gen)"
(cd "$SRC/cef" && GN_OUT_CONFIGS="$OUTDIR_NAME" CEF_INSTALL_SYSROOT=x64 \
    python3 tools/gclient_hook.py)
[[ -f "$SRC/out/$OUTDIR_NAME/build.ninja" ]] || { echo "gn gen didn't make out/$OUTDIR_NAME" >&2; exit 1; }
cat "$SRC/out/$OUTDIR_NAME/args.gn"
end

if [[ -x "$SRC/build/install-build-deps.sh" ]] && [[ -n "${CI:-}" ]]; then
    say "Build dependencies"
    sudo "$SRC/build/install-build-deps.sh" --no-prompt || echo "install-build-deps failed; going on (the sysroot covers most)" >&2
    end
fi

# ---- 2. resume from the previous stage ------------------------------------
say "Checkpoint"
OUT="$SRC/out"
# One fixed mtime for every source file, so outputs from an earlier stage
# (newer) count as up to date.
find "$SRC" -path "$SRC/out" -prune -o -exec touch -h -d '2000-01-01 00:00:00' {} + 2>/dev/null || true
if [[ -n "$CKPT_IN" ]]; then
    # Never fall back to a fresh build: that would spend a whole stage
    # redoing finished work.
    [[ -f "$CKPT_IN" ]] || { echo "checkpoint $CKPT_IN is missing" >&2; ls -la "$(dirname "$CKPT_IN")" >&2 || true; exit 1; }
    echo "Resuming from $CKPT_IN ($(du -h "$CKPT_IN" | cut -f1))"
    rm -rf "${OUT:?}/$OUTDIR_NAME"
    zstd -dc "$CKPT_IN" | tar -C "$OUT" -xf -
    rm -f "$CKPT_IN"
fi
disk
end

# ---- 3. compile until done or the deadline -------------------------------
say "Compile $BUILD_TARGETS"
cd "$SRC"
status=0
if ((DEADLINE > 0)); then
    # Leave time to pack and upload the checkpoint (~40 min).
    budget=$((DEADLINE - $(date +%s) - 40 * 60))
    ((budget > 600)) || { echo "less than 10 min left to compile" >&2; budget=600; }
    echo "Compiling for at most $((budget / 60)) min"
    timeout --signal=INT --kill-after=300 "$budget" \
        autoninja -C "out/$OUTDIR_NAME" $BUILD_TARGETS || status=$?
else
    autoninja -C "out/$OUTDIR_NAME" $BUILD_TARGETS || status=$?
fi
end

if ((status == 124 || status == 130 || status == 137)); then
    say "Out of time: writing a checkpoint"
    rm -f "$CKPT_OUT"/out.tar.zst
    tar -C "$OUT" -cf - "$OUTDIR_NAME" | zstd -T0 -3 -o "$CKPT_OUT/out.tar.zst"
    du -h "$CKPT_OUT/out.tar.zst"
    out "done=false"
    end
    exit 0
elif ((status != 0)); then
    echo "Compile failed (exit $status)" >&2
    exit "$status"
fi

# ---- 4. package the minimal distribution ---------------------------------
say "Package the minimal distribution"
cd "$W"
python3 "$AUTOMATE" "${common[@]}" --no-update --no-build --force-distrib \
    --minimal-distrib-only --no-distrib-docs
ARCHIVE="$(ls -t "$SRC"/cef/binary_distrib/cef_binary_*_linux64_minimal.tar.bz2 | head -n1)"
[[ -f "$ARCHIVE" ]] || { echo "no minimal archive was made" >&2; exit 1; }
cp "$ARCHIVE" "$DIST_OUT/"
NAME="$(basename "$ARCHIVE")"
(cd "$DIST_OUT" && sha256sum "$NAME" >"$NAME.sha256" && sha1sum "$NAME" >"$NAME.sha1")
{
    echo "archive: $NAME"
    echo "cef_branch: $CEF_BRANCH"
    echo "cef_commit: $CEF_COMMIT"
    echo "gn_defines: $GN_DEFINES"
    echo "built: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
    [[ -n "${GITHUB_RUN_ID:-}" ]] && echo "run: ${GITHUB_SERVER_URL}/${GITHUB_REPOSITORY}/actions/runs/${GITHUB_RUN_ID}"
} >"$DIST_OUT/BUILD-INFO.txt"
ls -la "$DIST_OUT"
out "done=true"
end

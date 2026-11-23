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
# Steps the previous stage left undone (written below when it ran out of time).
PROGRESS="$OUT/$OUTDIR_NAME/.stage_remaining"
prev_remaining="$(cat "$PROGRESS" 2>/dev/null || true)"
[[ -n "$prev_remaining" ]] && echo "The previous stage left $prev_remaining steps"
disk
end

# ---- 3. compile until done or the deadline -------------------------------
say "Compile $BUILD_TARGETS"
cd "$SRC"
status=0
LOG="$W/compile.log"
# With pipefail, the pipeline's status is autoninja's (or timeout's), not tee's.
if ((DEADLINE > 0)); then
    # Leave time to pack and upload the checkpoint (~40 min).
    budget=$((DEADLINE - $(date +%s) - 40 * 60))
    ((budget > 600)) || { echo "less than 10 min left to compile" >&2; budget=600; }
    echo "Compiling for at most $((budget / 60)) min"
    timeout --signal=INT --kill-after=300 "$budget" \
        autoninja -C "out/$OUTDIR_NAME" $BUILD_TARGETS 2>&1 | tee "$LOG" || status=$?
else
    autoninja -C "out/$OUTDIR_NAME" $BUILD_TARGETS 2>&1 | tee "$LOG" || status=$?
fi
end

# Siso ends an interrupted build with "Build Failure: N done, 0 failed,
# M remaining".
summary="$(grep -E 'done, [0-9]+ failed, [0-9]+ remaining' "$LOG" | tail -n1 || true)"
remaining="$(sed -nE 's/.* ([0-9]+) remaining.*/\1/p' <<<"$summary")"
echo "Compile: exit $status; ${summary:-no summary line}"
if [[ -n "${GITHUB_STEP_SUMMARY:-}" ]]; then
    echo "Compile: exit $status; ${summary:-finished}${prev_remaining:+ (previous stage left $prev_remaining)}" >>"$GITHUB_STEP_SUMMARY"
fi

if ((status == 124 || status == 130 || status == 137)); then
    say "Out of time: writing a checkpoint"
    # A resumed stage must leave fewer steps than the one before it, or the
    # checkpoint isn't being reused (as in run 44937270923) and the rest of
    # the chain would waste its hours: stop with an error.
    if [[ -n "$prev_remaining" && -n "$remaining" ]] && ((remaining >= prev_remaining)); then
        echo "::error::No progress: $remaining steps left, the previous stage left $prev_remaining. The checkpoint was not reused." >&2
        exit 1
    fi
    if [[ -z "$remaining" ]]; then
        echo "::warning::No 'remaining' count in the compile log; the next stage can't check its progress."
        rm -f "$PROGRESS"
    else
        echo "$remaining" >"$PROGRESS"
    fi
    rm -f "$CKPT_OUT"/out.tar.zst
    # POSIX (pax) format: it keeps nanosecond mtimes. Siso's .siso_fs_state
    # records each output's mtime to the nanosecond, and the default GNU
    # format rounds it down to the second, so every output looked changed
    # and each stage rebuilt nearly everything (run 44937270923).
    tar -C "$OUT" --format=posix -cf - "$OUTDIR_NAME" | zstd -T0 -3 -o "$CKPT_OUT/out.tar.zst"
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
# make_distrib.py runs with --allow-partial (we skip the Debug build), so a
# missing libcef.so would give an archive without binaries: check first.
# These are the files it copies for Linux (tools/make_distrib.py).
missing=()
for f in libcef.so chrome_sandbox libvk_swiftshader.so libvulkan.so.1 \
    v8_context_snapshot.bin vk_swiftshader_icd.json chrome_100_percent.pak \
    chrome_200_percent.pak resources.pak icudtl.dat locales/en-US.pak; do
    [[ -e "$SRC/out/$OUTDIR_NAME/$f" ]] || missing+=("$f")
done
if ((${#missing[@]})); then
    echo "::error::The build is missing: ${missing[*]}" >&2
    exit 1
fi
cd "$W"
rm -rf "$SRC/cef/binary_distrib"
python3 "$AUTOMATE" "${common[@]}" --no-update --no-build --force-distrib \
    --minimal-distrib-only --no-distrib-docs
ARCHIVE="$(ls -t "$SRC"/cef/binary_distrib/cef_binary_*_linux64_minimal.tar.bz2 2>/dev/null | head -n1 || true)"
[[ -n "$ARCHIVE" && -f "$ARCHIVE" ]] || { echo "::error::no minimal archive was made" >&2; exit 1; }
# The layout Trailboard's install-cef.sh and the cef crate expect.
contents="$(tar -tjf "$ARCHIVE")"
for f in Release/libcef.so Release/chrome-sandbox Resources/icudtl.dat \
    Resources/resources.pak include/cef_version.h; do
    grep -q "/$f\$" <<<"$contents" || { echo "::error::$f is not in $(basename "$ARCHIVE")" >&2; exit 1; }
done
cp "$ARCHIVE" "$DIST_OUT/"
NAME="$(basename "$ARCHIVE")"
(cd "$DIST_OUT" && sha256sum "$NAME" >"$NAME.sha256" && sha1sum "$NAME" >"$NAME.sha1")
{
    echo "archive: $NAME"
    echo "cef_branch: $CEF_BRANCH"
    echo "cef_commit: $CEF_COMMIT"
    echo "gn_defines: $GN_DEFINES"
    echo "built: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
    if [[ -n "${GITHUB_RUN_ID:-}" ]]; then
        echo "run: ${GITHUB_SERVER_URL}/${GITHUB_REPOSITORY}/actions/runs/${GITHUB_RUN_ID}"
    fi
} >"$DIST_OUT/BUILD-INFO.txt"
ls -la "$DIST_OUT"
out "done=true"
end

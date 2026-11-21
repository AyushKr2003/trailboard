# Trailboard: CEF build branch

This branch holds only the build of the Chromium engine (CEF) that
Trailboard uses, with full media support. It shares no history with
`master`, where the browser lives.

The standard CEF download can't play H.264, AAC or HEVC, and has no
Widevine, because those are patent-licensed or proprietary. This builds the
same CEF version Trailboard pins, with them turned on, and publishes it as a
GitHub release that `scripts/install-cef.sh --codecs` (on `master`) can
install.

## What's built

`cef.conf` pins everything:

- the CEF branch and commit (they must match the `cef` crate on `master`),
- the GN arguments: `proprietary_codecs=true ffmpeg_branding=Chrome` (H.264,
  AAC), `enable_platform_hevc=true use_vaapi=true` (HEVC through the GPU),
  `enable_widevine=true`, an official build without PGO, symbols, ThinLTO or
  CFI (so the link fits a hosted runner),
- the targets compiled.

The result is CEF's own "minimal" distribution:
`cef_binary_<version>_linux64_minimal.tar.bz2`, with `.sha256`, `.sha1` and a
`BUILD-INFO.txt` that records the arguments and the workflow run.

## How it builds

On GitHub-hosted runners (free for a public repository: 4 cores, 16 GB):

1. A push to this branch that changes `cef.conf`, `scripts/` or the
   workflows starts **Build CEF** (`.github/workflows/build.yml`). To
   build again without a change, open the last run on the Actions page and
   use "Re-run all jobs" (the "Run workflow" button only shows workflows
   that are on `master`).
2. It's a chain of up to eight stages (`stage.yml`). A hosted job stops
   after 6 hours; each stage syncs the pinned sources, unpacks the previous
   stage's output directory, compiles until ~40 minutes before its limit,
   and hands on a new checkpoint (a zstd tarball of the output directory, as
   a workflow artifact).
3. Resuming works because every stage sets all source files to one fixed
   mtime, so the build outputs from earlier stages (newer) count as up to
   date and only unfinished steps run.
4. The stage that finishes packs the minimal distribution; the `release`
   job publishes it as the release `cef-<version>-media`.

`scripts/free-disk.sh` removes preinstalled toolchains to make room (~70 GB
needed) and adds swap for the final link.

## Running it on your own machine

`scripts/build-cef.sh` runs in one go without `STAGE_DEADLINE` on a machine
with ~100 GB free and ideally 16 GB+ RAM:

```sh
CEF_WORK=/big/disk/cef CEF_OUT=/big/disk/cef-out DIST_OUT=./dist scripts/build-cef.sh
```

## Updating to a new CEF

1. On `master`, move the `cef` crate to the new version.
2. Here, set `CEF_BRANCH`, `CEF_COMMIT` and `CEF_SHORT` in `cef.conf` and push.
3. When the release is out, point `install-cef.sh` and the Arch PKGBUILD on
   `master` at it (name and SHA-256).

## Licence

Our scripts here are Apache-2.0 (`LICENSE-APACHE`), like Trailboard. CEF is BSD-3-Clause and
Chromium has its own licences (`CREDITS.html` in the archive). H.264, AAC
and HEVC are patent-licensed: these builds are for people's own use.

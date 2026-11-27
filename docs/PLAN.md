# Trailboard: plan

Trailboard is a canvas browser for Linux. Web pages are live cards on an
infinite board; a clicked link opens as a new card beside its source, joined
by a trail edge. Every algorithm (Tidy, placement, Fold) is our own design.

Status: ✅ done · 🟡 partial · ⬜ not started · ⏸ on hold for the owner

This file is kept short. The full decision logs (every rule, number and
verification note) are in git; read them there before changing an older
feature's behaviour:

- M1–M11: `git show 74e0f01:docs/PLAN.md`
- M12 steps 1–16 and the 2026-11-21 bug fixes: `git show f554118:docs/PLAN.md`
- The old changelog, known bugs and future ideas files:
  `git show f554118:CHANGELOG.md`, `…:docs/BUGS.md`, `…:docs/FUTURE.md`

---

## 1. Goals and non-goals

- **Goals:** a native Linux app (Wayland first, X11 too), canvas-first; fast
  and memory-aware on ordinary laptops (7.6 GB RAM, Intel iGPU); local-first
  (board, history, search on disk, no account); private by default (ads,
  trackers, cookie banners blocked); fully usable from the keyboard.
- **Non-goals:** another app's look, branding, server or paid features; sync,
  accounts, payments, telemetry; macOS for now. Windows is planned as M17
  (owner, 2026-11-25), after Linux; Linux stays the first platform.

## 2. Architecture

```
app (trailboard)  winit window · input · omnibox · chrome UI · main loop
  ├─ compositor   wgpu renderer · grid · cards · shadows · edges · text
  ├─ web          CEF off-screen browsers · paint buffers · input · events
  ├─ storage      on-disk formats (atomic, versioned)
  └─ scene        pure model and logic (no GPU/CEF/winit), unit-tested
```

- CEF runs on the main thread (`external_message_pump`), `Rc`/`RefCell`;
  CEF → app only through `web::drain_events()`.
- Each card owns a windowless Chromium; frames are uploaded only when the
  paint serial changes; off-screen cards are hidden, beyond the live cap asleep
  (cards playing media or kept awake sit outside the cap).
- Page scripts talk back through console messages with a private prefix
  (`web::links`, `web::text`, `web::scroll`, `web::focus`…); the app only
  trusts the answer from the card it asked.

## 3. Milestones

| | Milestone | What it gave |
|---|---|---|
| ✅ | M1 Skeleton | Window, wgpu surface, dot grid, camera pan/zoom/fit |
| ✅ | M2 Live page cards | CEF off-screen pages as cards, input forwarding, IME, popups, context menu, live-browser cap, undo/redo |
| ✅ | M3 Many cards and trails | Links open beside their source, trail edges, placement, screen-shaped cards |
| ✅ | M4 Persistence and spaces | Board JSON with autosave and migrations, profile on disk, several spaces |
| ✅ | M5 Board tools | Solo, Tidy, Fold, trail steps, trail names/colours, notes, sticky notes, Save as Markdown |
| ✅ | M6 Chrome UI | Sidebar, Ctrl+K palette, find, page zoom, keyboard-driven board, link hints, shortcut sheet, Omarchy look, settings, start page, downloads |
| ✅ | M7 History and search | `page-text.db` (FTS5), thumbnails, history view, private spaces, recently closed |
| ✅ | M8 Privacy | Adblock lists, cosmetic filtering, cookie-banner reject, YouTube ads, per-site toggle, browsing settings |
| ✅ | M9 Packaging | Bundle script, `--version`, crash reports, PKGBUILD, Chromium sandbox, first-run welcome |
| ⬜ | M10 Optional extras | See §8; most items on hold |
| ✅ | M11 Premium look | Corners setting, cards as sheets, board backgrounds, sidebar, command bar, motion, empty board, review pass |
| ✅ | M12 Keyboard first | Vim-like modes, page keys, scroll sections, command line, cards as tabs, search keywords, jumps/marks/saved pages, user key file, settings and site toggles, navigation extras, caret/visual mode, dialogs and fields, find back, `:messages`, the mode shown by ring and window title |
| ✅ | **M13 Full media support** | Our own media CEF (GitHub Actions, release `cef-156.0.26-media`) installed by `install-cef.sh --codecs` and the Arch package: H.264, AAC and HEVC play (all through VA-API); media controls (`,` keys, `:media`), page full screen as Solo, "can't play" messages; Widevine (software level, fetched by Chromium); README update checklist, engine-version warning, `scripts/media-check.html` |
| ⬜ | **M14 Config file** | One hand-editable `~/.config/trailboard/config.toml` for settings, key bindings and theme; the UI writes it without losing comments |
| ✅ | **M15 Custom trails** | Join any card to any trail and cut one out, so a named, coloured trail can be a group of any cards (our tab groups) |
| 🟡 | **M16 Lighter and quieter** | Cut memory and idle CPU: a measured baseline, then the main process, the main loop, sleeping cards and renderer count |
| 🟡 | **M16b Benchmark in CI** | An orphan `bench` branch: on every push, Trailboard and reference browsers measured on one runner, results kept on the branch, a chart page and a regression check |
| ⬜ | **M17 Windows** | Trailboard on Windows 10 and 11 (x64): the Linux-only parts behind platform seams, CEF for Windows, a Windows CI build, a portable zip; Linux unchanged |

### Owner decisions that still bind

- Cards have no title bar; a card is its page, with a quiet label above it
  ([DESIGN.md](DESIGN.md)). New cards are screen-shaped.
- Corners: Rounded (default), Follow Hyprland or Custom. Board background:
  Colour mood, Photo or Plain.
- No AppImage, Flatpak or AUR work; no UI translations; licence Apache-2.0 only.
- ⏸ On hold until the owner says so (2026-11-15): all AI/ML features
  (semantic search, Explain, anything else), imports from other browsers,
  password managers and passkeys. Don't design or prototype them.
- ⬜ CEF extensions: only when the owner picks it (§8).
- "Read Mode" is called **Solo** in everything the user sees; the code keeps
  `scene::read`, `App::read`, `Command::Read`. "Focus" stays out of
  interface text (it clashes with a field's focus).
- Page inspector (owner, 2026-11-21): in its page's own card, docked on
  the right by default (bottom on request), as in a browser tab; a card
  of its own beside the page stays as the "undocked" choice. Never a
  separate window (F12, Ctrl+Shift+I, `:inspect`, right-click → *Inspect*). CEF 156 draws `ShowDevTools` windows Chrome-style only, which
  can't be windowless, so the card loads the bundled inspector pages
  (`devtools://devtools/bundled/devtools_app.html`) and `web::inspector`
  carries their protocol messages to the page in-process. Never a remote
  debugging port.
- No outside video player as a feature; video plays in the card (M13).
  `:run` stays for anyone who binds a player in the key file themselves.

---

### M12: Keyboard first ✅

Asked by the owner on 2026-11-17 and widened on 2026-11-20: the whole
browser usable from the keyboard alone, vim-style. Key choices follow vim's
common conventions; command names, the key file format, the docs and the
code are our own. A card plays the part a tab plays elsewhere. Step 17
(macros, `:screenshot`, `:print`, user scripts) was dropped by the owner on
2026-11-21.

**Modes** (the pill at the bottom left shows the mode and half-typed keys):

| Mode | Keys go to | Enter by | Leave by |
|---|---|---|---|
| Board | Board keys (cards are like windows) | Esc from Page | — |
| Page | Our page keys first; unbound characters are dropped, keys that type nothing go to the page | Enter on a card, a click, Solo | Esc → Board |
| Insert | Everything to the page, except a few readline-style Ctrl edits | A field gets focus by a user gesture (click, hint, `gi`) | Esc → Page, field blurred |
| Passthrough | Everything (Esc too) but Ctrl+Q and copy/paste | Ctrl+V, or the per-site `tp` setting | Shift+Esc |
| Caret / Visual | Caret moves and selections in the page's text | `v`, `V` | Esc |
| Command | `:` turns the palette into a command line | `:` | Enter / Esc |

**Rules that still bind** (owner answers, 2026-11-17 and 2026-11-20):

- In Page mode f follows a link in the same card, F opens it beside; on the
  board f opens beside and Shift+F is Fit.
- h/j/k/l: on the board they move between cards; in a page they scroll its
  scroll target. Half a section scrolls with Ctrl+d / Ctrl+u.
- `d` closes the card (in a page it goes on to the next in reading order),
  `u` reopens the last closed one. J / K walk cards in Tidy's reading order.
- `b` / `B` open saved pages everywhere; the trails sidebar is Ctrl+B and
  `:sidebar`. Both named saved pages and lettered marks exist.
- `t` starts toggles everywhere (`td`, `tj`, `tp`…); Tidy is `T` and
  `:tidy`; Ctrl+T stays New card.
- `?` in a page finds backward; the shortcut sheet is F1, `:keys`, and `?`
  on the board.
- Ctrl+W alone never closes a card: it only starts `Ctrl+w` sequences
  (h/j/k/l, w/W, p, c/q close). In Insert, Ctrl+W/U/H edit the field.
- Chaining at a scroll section's edge goes on to the enclosing one, like a
  wheel; `overscroll-behavior: contain` stops it.
- `:q`, `:w`, `:e`, `:qa` are aliases; zz/zt/zb are not adopted.

**Steps** (all ✅; details in `git show f554118:docs/PLAN.md`):

1. Key engine: vim notation, counts, pending keys (`scene::keys`).
2. Modes and the first page keys (`App::page_keys`, `web::focus`).
3. Scroll sections (`web::scroll`, `scene::scrolltarget`).
4. Page keys: H/L, r/R, `/` n/N, gi, yank, p/P, o/O.
5. Hint kinds with `;` (copy, download, image, background, field, many).
6. Command line `:` (`scene::command`, history in `commands.json`).
7. Cards as tabs: J/K, Alt+digits, Ctrl+^, gt, d/u, co, gC, `Ctrl+w`.
8. Search keywords (`scene::search`, `settings.json` v2).
9. Jumps, marks and saved pages (`jumps.json`, `saved.json`).
10. User key file `~/.config/trailboard/keys`, aliases, chains, `:run`,
    `:hint`, the live `?` sheet and prefix panel.
11. Settings from the keyboard: `:set`, `:toggle`, `:site`, `t` keys,
    per-site cookies and Passthrough.
12. Navigation extras: `]]`/`[[`, gu/gU, Ctrl+a/x, go/xo/gh/gf/gd,
    primary selection, more hint kinds.
13. Command line power: readline keys in every field, Up/Down history,
    counts, `.`, `:repeat`, `:later`.
14. Caret and visual mode (`web::caret`, `scene::caret`).
15. Dialogs and fields: page alert/confirm/prompt, permissions, typed file
    chooser, Ctrl+e in `$EDITOR`, Shift+Insert.
16. `?` find backward, Esc clears find marks, `:messages`.
- Bug fixes (2026-11-21): Ctrl+W alone keeps the card; Ctrl edits while
  typing; link hints reach into frames (cross-site, 3 levels deep).

**Known limits:** sites with their own single-letter keys need Passthrough;
JS-faked scrolling isn't found as a section (the wheel still reaches it); a
key typed before a field's focus report arrives; caret mode works in the
main frame only; app-started `wl-copy` can't own the primary selection on
Hyprland 0.57.

---

### M13: Full media support ✅

Asked by the owner on 2026-11-20: play the videos and music other
Chromium browsers play. Checked that day (`site/codecs.html` in
`~/.cache/trailboard-e2e-m12`, `codecs.png`): the standard CEF download
plays VP9, AV1, Opus and MP3, but **not H.264, AAC or HEVC, and has no
Widevine** — so Twitch, X, Reddit, Vimeo, most news sites and plain `.mp4`
links show a player stuck at 0:00, and Netflix-style sites don't play.
CEF leaves these off by default for patent licensing; it builds them with
`proprietary_codecs=true ffmpeg_branding=Chrome` (GN defines for CEF's
`automate-git.py`). Swapping in the system's ffmpeg doesn't work (a
release CEF links it in).

**Constraints.** A Chromium build needs ~100 GB of disk and ideally ≥16 GB
of RAM; this machine has 7.6 GB and ~13 GB free, so builds run elsewhere:
on GitHub Actions (see "Approach" below), or as a fallback on a rented
cloud machine (about 32 cores / 64 GB: 2–4 h per build). Each CEF update
(Chromium security fixes every few weeks) needs a rebuild, or the engine
falls behind. H.264 / AAC are patent-licensed: a codecs engine is
installed on the user's machine from its own source (never shipped inside
a Trailboard bundle); the portable bundle keeps the standard CEF. Our own
build replaces the interim third-party one below once it exists.

**Prebuilt test** (owner's request, 2026-11-21). tobagin/karere's release
`cef-156.0.26-proprietary-codecs` is the same CEF commit we pin
(`156.0.26+g8ea0c73`, Chromium 156.0.8078.88), and its headers match ours.
It's unpacked in `~/.cache/trailboard-cef-karere/cef` (sha256 checked) and
swapped in at run time without a rebuild:
`CEF_PATH=$K LD_LIBRARY_PATH=$K target/debug/trailboard` (the binary uses
RUNPATH, so `LD_LIBRARY_PATH` wins). Result (`~/.cache/trailboard-e2e-karere`):
H.264, AAC, MP3, VP9, AV1 and Opus → "probably"; HEVC → no; Widevine → no;
a local H.264/AAC `.mp4` plays in a card (640×360, frames advancing). So a
codecs build works with our off-screen cards unchanged. Real sites were not
tried.

**Interim CEF** (owner's decisions, 2026-11-21; **replaced 2026-11-24** by
our own build, step 2: the pins below now point at release
`cef-156.0.26-media`, and the karere build is no longer used or kept).
What still holds for the media build:

- `scripts/install-cef.sh --codecs` installs it; `--standard` (the default)
  installs CEF's own download. Both are pinned by name and SHA-256 in the
  script (no build-index lookup), checked before install, unpacked on disk
  beside the target (not `/tmp`) and swapped in only when complete. The
  script refuses while Trailboard runs, checks tools and free space, skips
  an installed build unless `--force`, deletes the download after success,
  and writes `.cargo/config.toml` if missing.
- The Arch PKGBUILD uses the codecs build **by default**;
  `TRAILBOARD_CODECS=0 makepkg` gives the standard one.
- `archive.json` keeps the fields the `cef` crate's build script reads
  (`name`, `sha1`; `sha1` is empty for the codecs build, whose release
  gives only a SHA-256; ours has both) and marks it `"codecs": true`. `scripts/bundle.sh`
  refuses such a CEF unless `ALLOW_CODECS=1`, so the portable archive
  never carries it.
- The pins live in two places that must move together with the `cef`
  crate: `scripts/install-cef.sh` and `packaging/arch/PKGBUILD`.
- On the owner's machine: codecs at `~/.local/share/cef`, the standard one
  at `~/.local/share/cef-standard` (for bundles).
- Before AUR: the owner should confirm shipping a PKGBUILD that downloads a
  patent-licensed engine (now our own release) by default is acceptable there.

**Approach: build on GitHub Actions** (owner's choice, 2026-11-21). Instead
of a rented cloud machine, CEF is built on GitHub's hosted runners, in the
way atlas-engineer/cef does it (CEF with H.264/AAC for Nyxt, public
workflow): the compile is split into a chain of up to eight time-boxed
jobs, because a hosted job stops after 6 hours. Each job saves the build
state and `out/` (zstd) as a checkpoint, and the next job resumes from it.
The last job packs a minimal distribution
(`cef_binary_<version>_linux64_minimal.tar.bz2` and a `.sha256`) and
publishes it as a release tagged with the CEF version. This needs no cloud
account and nothing big on this machine, and it still counts as a build we
make ourselves (we own the workflow and can read every log).

- **Where:** the orphan branch **`cef-build`** of this repo (owner's
  choice, 2026-11-21; it shares no history with `master`, so the browser's
  checkout stays small). Worktree on the owner's machine:
  `../trailboard-cef-build`. The repo is public, so hosted runners' minutes
  are free. A push to `cef-build` that changes `cef.conf`, `scripts/` or
  the workflows starts a build; the release is tagged `cef-<version>-media`.
- **Clean room:** we write our own workflow and scripts from the method;
  nothing is copied from atlas-engineer's repo.
- **Pinned:** `cef.conf` on the branch holds the CEF branch (8078) and
  commit (8ea0c73, as `install-cef.sh` pins), the GN arguments and the
  targets. GN: `is_official_build proprietary_codecs ffmpeg_branding=Chrome
  enable_platform_hevc use_vaapi enable_widevine` (CEF requires the last
  anyway), with `chrome_pgo_phase=0 symbol_level=0 use_thin_lto=false
  is_cfi=false` so the link fits a 16 GB runner.
- **How a stage resumes:** each stage syncs the same pinned sources, runs
  `gclient_hook.py` itself (automate-git.py only generates build files in
  its own build step), sets every source file's mtime to 2000-01-01, and
  unpacks the previous stage's output directory on top; only unfinished
  steps are dirty. Checkpoints travel as workflow artifacts (zstd).
- **To check in step 1:** the runner's free disk (a CEF checkout is large;
  remove the preinstalled toolchains first), the per-repo cache and
  artifact size limits for the checkpoints, total wall time per build, and
  whether HEVC (step 3) and Widevine (step 4) flags fit the same chain.
- **Fallback:** a rented cloud machine running the same script, if the
  runners turn out too small.

**Steps:**

- ✅ 1. **Build script and workflow.** Written on `cef-build`
  (2026-11-21). First full run (44937270923, 2026-11-21→23): GN arguments
  accepted, 0 failed compile steps, disk fine (74 GB free), ~73k steps at
  ~2/s, ~14 min setup per stage; but all eight stages ran out of time with
  no net progress and the release was skipped while the run showed green.
  Cause: Siso's `.siso_fs_state` keeps nanosecond mtimes and GNU-format tar
  rounded them to the second, so every resumed output looked changed.
  Fixed 2026-11-23 (02c7861): pax checkpoints, a stage that leaves no fewer
  steps than the last one fails, a `finished` job fails a run no stage
  completed, required files and archive layout checked before release.
  Rerun 45212313507 (2026-11-23→24) finished in two stages (5 h 10 min:
  44,114 of ~70k steps, 908 MB checkpoint; then 4 h 10 min: 15,157 steps,
  the checkpoint reused) and published release `cef-156.0.26-media`
  (167 MB minimal tar.bz2, `.sha256`, `.sha1`, `BUILD-INFO.txt`). README:
  "Building the media engine". The chained GitHub Actions workflow
  above, around `scripts/build-cef.sh`: on a fresh Linux machine,
  fetch CEF's build tooling for the branch matching `install-cef.sh`'s
  version, build the minimal distribution with
  `is_official_build=true proprietary_codecs=true ffmpeg_branding=Chrome`,
  and pack it the way the CEF downloads are packed (same layout, a SHA-1
  and the build flags written beside it). The same script runs inside each
  workflow job (resuming from the checkpoint) or in one go on a big
  machine. Documented in the README: how to start a build, how long it
  takes, and how to fetch the release.
- ✅ 2. **Install and check** (2026-11-24). `install-cef.sh --codecs` and
  the PKGBUILD default now fetch our release (pinned SHA-256 and SHA-1);
  `--from <tarball>` installs a local build (checked against
  `<tarball>.sha256` if present; the name must match the pinned version).
  The script compares the installed `archive.json`'s `sha256`, so another
  build of the same version is replaced, not skipped. `--version`, the
  start log and crash reports end in `standard build` / `media build`
  (`launch::EngineBuild`, from `archive.json` beside the engine; the
  package and `bundle.sh` now keep that file). There is no About page, so
  `--version` is the place. Verified (`~/.cache/trailboard-e2e-media/`,
  `site/codecs.html`): H.264, AAC, HEVC, MP3, VP9, AV1, Opus →
  "probably" and MSE yes; local H.264 and HEVC files play in a card at
  640×360 and 1080p (`codecs.png`), AAC audio plays after a click
  (Chromium's autoplay rule blocks it before); a plain `.mp4` link
  (w3schools `mov_bbb.mp4`) shows its frame; a 1080p H.264 HLS stream
  through hls.js/MSE plays and draws (`hls.html`, `hlsc.png`).
  Vimeo: the owner played videos on vimeo.com in a card by hand (fine).
  Only the bare player link (`player.vimeo.com/video/76979871?autoplay=1`)
  in a scripted run, with no click on the page, stayed white at 00:00;
  the standard CEF shows Vimeo's "Player error" there.
- ✅ 3. **HEVC through the GPU** (2026-11-24). No build flags or switches
  needed beyond the build's own (`enable_platform_hevc use_vaapi`): with
  `--enable-logging=stderr --vmodule=*vaapi*=2,*video_decoder_pipeline*=2`
  (Chromium switches pass through Trailboard's command line) the GPU
  process logs `VaapiVideoDecoder` for HEVC main and **main 10**, and also
  for H.264, VP9 and AV1, so the Intel iGPU decodes all of them; the GPU
  process maps `libva` and `iHD_drv_video.so`. Checked in one card
  (`~/.cache/trailboard-e2e-m13/site/formats.html`, `formats.png`): H.264
  1080p, HEVC 8-bit 1080p, HEVC 10-bit 720p, HEVC 10-bit HDR10 (BT.2020,
  PQ, mastering metadata) 720p, VP9 and AV1 all play together and draw.
  HDR is tone-mapped to SDR: cards are 8-bit sRGB textures, so there's no
  HDR output (not planned). `chrome://media-internals` opens in a card for
  a closer look. On a machine without VA-API, HEVC won't play (Chromium
  has no software HEVC decoder) and the others fall back to software.
- ✅ 4. **Widevine (trial)** (done 2026-11-24). **It works in the media
  build as it is; no rebuild.** The build already has `enable_widevine`,
  and Chromium's component updater fetches Google's CDM by itself into
  `<profile>/browser/WidevineCdm/<version>/` (4.10.3112.0 on the owner's
  real profile) and writes the hint file
  `WidevineCdm/latest-component-updated-widevine-cdm`. On Linux the CDM is
  registered only at startup, from that hint, so it works from the start
  *after* the download. The earlier NotSupportedError came from short-lived
  e2e profiles that never finished the download. Checked
  (`~/.cache/trailboard-e2e-widevine/`, the CDM copied into the isolated
  profile, `site/wv.html` served from localhost): `com.widevine.alpha` is
  granted, Shaka's public Widevine stream (`angel-one-widevine`, no-auth
  licence proxy) gets its licence and plays (frames advancing, 320×240
  stepping up to 768×576), and **the decrypted video draws in the
  off-screen card** (`wv.png`): Linux Widevine is the software level (L3),
  so there's no hardware path that OSR would miss. Turning it off was also
  checked: with the hint file moved aside and `--disable-component-update`,
  the CDM on disk isn't loaded (NotSupportedError). Owner's decision
  (2026-11-24): **no *Protected video* setting**; leave Chromium's default
  (fetched and used). Limits: L3 only, so services that need hardware DRM
  for HD (Netflix-style) cap quality or refuse; real DRM sites weren't
  tried. The "can't play: protected (DRM)" message (`scene::media`) still
  shows when a site refuses, e.g. before the CDM has been fetched.
- ✅ 4a. **Cards that stay awake** (asked by the owner on 2026-11-23).
  Media playing keeps a card outside the live cap (10 min grace after a
  pause), and each card's awake button cycles *Auto*, *Keep awake* and
  *Always active*; see the Handoff of that day. Later: an awake mark on
  sidebar rows, a *Now playing* entry in Ctrl+K.
- ✅ 5. **Media behaviour** (done 2026-11-24). A card's media shows on its
  label (speaker / crossed-out speaker) and `:mute` (earlier). Now:
  - **Controls on any site:** `,` then a key, on the board and in a page
    (`,` was free in both): `,,` / `,Space` play / pause, `,h` / `,l` 10 s
    back / on (a count goes further: `3,l`), `,H` / `,L` a minute, `,0` the
    start, `,j` / `,k` slower / faster (0.25× to 4× in steps), `,=` normal
    speed, `,m` mute (the card's, as `:mute`), `,r` loop, `,f` full window.
    By name `:media <toggle|play|pause|forward [s]|back [s]|seek <time|%|±s>|faster|slower|speed [rate]|mute|loop|full>`
    (`scene::media`), key-file names `media-toggle`… and two palette rows.
    A toast answers: *Paused · 1:23 / 4:56*, *Speed 1.5×*, *Loop on*.
  - **Which media:** each frame's script reports none / present / playing
    (`web::media::Frames`); the op goes to the frame that started playing
    last, else the one whose media changed last, and that frame picks the
    playing element, else the largest one in view. The main frame gets the
    op through DevTools `Runtime.evaluate` with `userGesture`, so play
    with sound and full screen work without a click; other frames by
    plain script. A cross-site embed that's refused play retries once
    after 150 ms, when the top page has had the click and lends it on
    (checked: an `allow="autoplay"` iframe on another port plays with
    sound); a frame refused full screen asks the top page, which puts
    the whole iframe in full screen.
  - **Full window:** a page going full screen (its player's button, or
    `,f`) puts the card in Solo, so the video fills the window
    (`on_fullscreen_mode_change` → `WebEvent::Fullscreen`); leaving it
    puts back what was there (the board, or the card Solo showed). Esc
    takes the page out of full screen first; leaving Solo does too. The
    window itself stays as it is (F11 for the screen). `,f` puts the
    bare `<video>` in full screen with `object-fit: contain` forced while
    it lasts (YouTube's own `cover` cropped it to the window).
  - **Can't play:** a wanted video (autoplay, controls, play() called, or
    ≥120 px wide) failing with a decode / unsupported-source error says
    *its format isn't one Trailboard plays*; encrypted media with no keys
    after 3 s, or a video stuck 5 s after a refused
    `requestMediaKeySystemAccess` (not Clear Key), says *it's protected
    (DRM)*. Once per card, kind and address, as a 6 s toast. The script
    also runs first in each document (`Page.addScriptToEvaluateOnNewDocument`)
    so it sees a DRM request made at once. A site that gives up after the
    refused request without playing shows its own error, not ours.
  - **Outside player: no** (decided here, as in §2): the message names the
    reason; anyone can still bind a player with `:run`.
  - Verified (`~/.cache/trailboard-e2e-m13/`, runs 5–9): every op above in
    a scripted run with the page's own state in its title, full window and
    Esc, a cross-site iframe (play with sound, full window), the format and
    DRM messages, a bad `:media speed 99`; screenshots `iframefull.png`,
    `toast.png`. On YouTube (run 12, Big Buck Bunny watch page, from the
    board without a click): play with sound, pause, `:media forward 30`,
    faster, `,f` full window in Solo (`yt.png`, letterboxed) and Esc back.
    Not checked: other real sites (Twitch, Vimeo, news embeds), a real
    DRM site's message, and hardware media keys (XF86AudioPlay, MPRIS):
    those go to the desktop, and Chromium's MPRIS service isn't wired up
    in CEF; later.
- ✅ 6. **Keeping current** (done 2026-11-24).
  - **README, "Updating CEF":** the nine steps for each engine update
    (bump the crate; standard pins; the media build on `cef-build` and its
    pins; `cargo test`; install; `--version` and the start log; the media
    check; a YouTube smoke test; the changelog).
  - **Start-log warning** (`launch::engine_mismatch`): the version of the
    `libcef.so` actually loaded (`web::loaded_engine_version`, CEF's
    exported `cef_version_info`, which the crate doesn't wrap) against the
    one compiled in (`web::built_engine_version`, which is what
    `install-cef.sh` pins): "the installed CEF X is older/newer than the Y
    this Trailboard was built for; install the matching one with
    scripts/install-cef.sh". Compared as numbers. Asking the library, not
    `archive.json`, catches a hand-swapped engine too.
  - **Pins can't drift** (`launch::tests::engine_pins_agree`): the `cef`
    crate's version, `install-cef.sh`'s `CEF_VERSION` and `COD_TAG`, and
    the PKGBUILD's `_cef`, media release URL and all four checksums must
    agree (checked: a changed PKGBUILD checksum fails it). `cef.conf` lives
    on `cef-build`, so it isn't covered; the README step names it.
  - **`scripts/media-check.html`:** one page, no media files: each codec
    (canPlayType "probably" and MSE), Widevine available, and a clear
    H.264/AAC stream and a Widevine stream (Shaka's public test assets,
    player loaded from jsdelivr) must advance frames; the title ends
    `media check: pass` or `FAIL <check (detail)>`, so a scripted run reads
    it from `cards`. It must be served from localhost: on `file://` the
    Widevine CDM can't be created (Shaka 6002; an opaque origin). HEVC is
    asked again for up to 15 s, as Chromium learns the GPU's decoders
    after it starts.
  - **Found while checking, fixed:** on this laptop NVIDIA's driver (GTX
    1650, `nvidia`) now holds `renderD128` and the iGPU is `renderD129`;
    Chromium's VA-API skips NVIDIA's node and gives up, so HEVC stopped
    playing (err4, "no supported streams") and the rest fell back to
    software. `web::render_node` reads `/sys/class/drm/renderD*/device/driver`
    and, when the first node is `nvidia` and a later one isn't, adds
    `--render-node-override=<that node>` for the browser process (the GPU
    process inherits it) unless the command line has one; the start log
    says so. Nouveau isn't skipped (Mesa has VA-API).
  - Verified (`~/.cache/trailboard-e2e-widevine/`): the media check passes
    (run 10, with the override chosen by itself); step 3's `formats.html`
    plays H.264 1080p and HEVC 8-bit, 10-bit and HDR10 again
    (`formats-nvidia.png`), and VP9 and AV1 each decode on VA-API alone
    (346 frames, none dropped). No mismatch warning with the pinned engine.
    Not verified: the warning with a really older engine (none at hand;
    unit-tested), a machine where NVIDIA's node isn't first. Limit: with
    the override, step 3's six-HD-videos page stalls VP9 and AV1 at 0 s
    while the four H.264/HEVC ones play (one at a time they're fine).

**Risks:** the build may fail on the cloud machine's distro (pin the
image); HEVC depends on the iGPU and driver (and on the render node: see
step 6); rebuild cost and effort every few weeks; a codec
build must never be published by mistake (the bundle script refuses it
unless asked).

---

### M14: Config file ⬜

Asked by the owner on 2026-11-21: one file a user can edit by hand that
holds everything they can change — settings, key bindings and the theme.
Today these live in three places: `$DATA/settings.json` (app-written
JSON), `$XDG_CONFIG_HOME/trailboard/keys` (our line format) and the
Omarchy palette (`theme.rs`, read-only for us).

**Decisions** (2026-11-21):

- **Format: TOML.** It has comments, sections and clear line/column
  errors; it works with `serde`; Linux users know it. Rejected: JSON (no
  comments, poor to hand-edit), YAML (indentation traps, `no` → `false`,
  weaker Rust crates), KDL/RON (less known, weaker editor support).
- **Path:** `$XDG_CONFIG_HOME/trailboard/config.toml`
  (`~/.config/trailboard/config.toml`). Board, history and other state stay
  in `$DATA`; only things the user chooses go in the config.
- **One source of truth.** Once `config.toml` exists, it is the only place
  settings, keys and theme are read from. The settings page, `:set`,
  `:toggle` and `t` keys write it back with **`toml_edit`**, changing only
  the touched value and keeping the user's comments, order and blank lines.
- **Forgiving reads**, as `settings.rs` does now: each field is read on
  its own; a bad or unknown value keeps its default, is reported with its
  line in `:messages` and a toast, and the rest still load. A file that
  doesn't parse at all keeps the last good config.
- **Versioned:** a top-level `version = 1`; later changes migrate.
- **Key bindings keep their command syntax.** One table per mode list,
  `key = "command"` per line, so `scene::userkeys` keeps the meaning; only
  the container changes. An empty string unbinds. Aliases get their own
  table.
- **Theme overrides, not replaces.** Default is to follow Omarchy as now;
  `[theme]` can set any palette key (`accent`, `background`, …, the names
  in `theme::FALLBACK`) as `"#rrggbb"`, plus `mode = "follow" | "dark" |
  "light"`. Unset keys still come from Omarchy or the fallback. Derived
  colours (`quiet`, `loud`, …) are computed from the result, so
  DESIGN.md's contrast rules still hold.

**Shape:**

```toml
version = 1

[general]       # search, new_card, live_pages, default_zoom, language, …
[look]          # corners, corner_radius, board_background, page_colours, scroll_speed, space_panel
[privacy]       # block_ads, history, block_third_party_cookies, do_not_track, javascript, images
[permissions]   # camera, microphone, location, notifications = "ask" | "allow" | "block"
[network]       # proxy, proxy_server
[downloads]     # download_to
[[search.keywords]]   # keyword, name, url
[theme]         # mode, accent = "#7aa2f7", …
[keys.page]     # gs = "open https://web.archive.org/web/{url}"; d = "" unbinds
[keys."board,page"]
[aliases]       # play = "run mpv {hint-url}"
```

**Steps:**

- ⬜ 1. **Reader.** `storage::config`: the `toml` and `toml_edit` crates,
  parse into `Settings`, key entries and theme overrides field by field,
  with errors that carry a line number. Unit tests for every section, bad
  values, unknown keys, a newer `version`.
- ⬜ 2. **Migration and switch.** On start with no `config.toml`, write
  one from `settings.json` and `keys` (commented, every section present,
  defaults shown as comments). Leave the old files untouched as a
  backup; once `config.toml` exists they aren't read, and `:messages`
  says so once. Tests: v1/v2 `settings.json`, a key file with binds,
  unbinds and aliases.
- ⬜ 3. **Writing back.** Every UI path that saved `settings.json` edits
  `config.toml` through `toml_edit` instead (atomic write, comments kept).
  Test: a commented file round-trips through a `:set` unchanged but for
  the one value.
- ⬜ 4. **Live reload and commands.** Watch the file (`desktop`'s
  `notify` watcher); apply on save, with errors in `:messages`.
  `:config` opens it in `$EDITOR`, `:config-reload` re-reads it;
  `:keys-file` points at the `[keys]` part. The `?` sheet shows the
  new bindings live, as now.
- ⬜ 5. **Theme.** `[theme]` overrides on top of the Omarchy palette in
  `theme.rs`, `mode` forcing dark/light. Screenshot check: an override
  accent, a forced light mode, and Omarchy theme switches still followed
  for unset keys.
- ⬜ 6. **Docs.** A config section in `docs/GUIDE.md` with every key, its
  values and default; README mention; CHANGELOG.

**Open (decide while building):** whether per-site settings
(`sitesettings.rs`) belong in the config too (lean: no, they're state
set from the page); whether to accept `config.d/*.toml` drop-ins (lean:
not now).

---

### M15: Custom trails ✅

Asked by the owner on 2026-11-23, as our answer to tab groups ("I like
the coloured custom trail part"). Today only opening a link makes a trail
edge (`Scene::spawn_beside`; undo puts back a closed card's edges): an
existing card, lone or in a trail of its own, can't join another trail
or leave its own except by closing. Trails already have what groups need
otherwise: a name (R), a colour (C), Fold into a stack (S), and a place
in the sidebar (pin, reorder). So rather than a second grouping concept,
any cards can be made a trail.

**Steps:**

- ✅ 1. **Design note.** Decisions below (2026-11-25).
- ✅ 2. **Join and leave in `scene`.** `Scene::join(card, target)`: the
  card's edge from its parent is cut and it gets one from `target`, as if
  opened from it, bringing its followers along under it.
  `Scene::leave(card)`: its edge from the card it came from is cut, so it
  starts a trail of its own (its children stay with it). Each one undo
  step (`History`). Unit tests: a lone card, a card with its own trail,
  refusing a cycle (joining a card to its own descendant), stacks,
  undo/redo. Edges keep their format (no version bump).
  Done 2026-11-25 in `scene::join`: `join_many` / `leave_many` (the
  one-card forms call them) return a `Relinked` record, and
  `History::relinked` makes it one step with a new `Edit::SetEdges` (the
  whole edge list, so undo keeps the order children were opened in, which
  reading order follows). `placement::beside_skipping` places a card
  without the cards about to move blocking it. Also `Scene::parent` and
  `Scene::followers`. Settled while building it:
  - A card that had a parent brings no mark: only a trail's first card
    carries its trail's name and colour into the join. (Moving one card
    out of "Work" onto a lone card shouldn't make a second "Work".)
  - With several cards, one loop refuses the whole join (one clear
    message beats a half-done one); cards already under the target are
    skipped, and *Unchanged* only when none is left.
  - `leave` on a trail's first card does nothing (*Unchanged*): it has no
    parent, and cutting its links would split its trail into pieces. The
    menu entry should be off for it (step 3).
- ✅ 3. **Ways in.** Drag a card's label with a modifier (Alt?) onto
  another card (the target is highlighted while hovering); `:join <card>`
  with card titles offered; a key that picks the target with letter hints;
  `:leave` and an entry in the card's menu. Ctrl+K entries for both.
  Done 2026-11-25:
  - **Keys:** `a` on the board (`join-trail`) puts letters on the other
    cards in view (in reading order; the joining cards and the cards
    opened from them get none, as they'd make a loop). Typing a card's
    letters joins its trail, or so does a click on it; any other key or
    click cancels. `A` (`leave-trail`) leaves. Board mode only: in a page
    `a` stays the page's.
  - **Alt+drag a card's label** onto another card: the dragged cards are
    outlined and a trail line follows the pointer, drawn as trails are
    (rounded right-angle turns, a dot at each end). Over a card, that card
    is outlined and a faint card shows where the grabbed one will land
    beside it (`placement::beside_skipping`, as the join will place it),
    the line running into it, all in that trail's colour (owner,
    2026-11-25: the first dotted line didn't look good). Let go on no
    card: nothing happens. A selected card's drag takes the selection.
  - **Keyboard only** (owner, 2026-11-25): h/j/k/l to a card, v to select
    several, a, then a card's letters; A to leave; `:join` / `:leave`.
    Checked by a run with no clicks (`run7.sh`).
  - **`:join [card]`** (a title, or a number in reading order, as `:card`
    takes; titles offered); alone it shows the letters. **`:leave`.**
  - **Ctrl+K:** *Join a trail…* (A) and *Leave trail* (Shift+A).
  - **Card menu:** *Join a trail…*, and *Leave trail* when the card has a
    parent, as a section before *Inspect*.
  - All act on the selection when the current card is in it, else on the
    current card, through `App::join_cards` / `App::leave_trail`.
  - Messages: *Joined Work*, *Joined the trail of Card e*, *3 cards joined
    …*; *Left Work*; *Can't join a card to its own trail below it*; *The
    first card of a trail can't leave it · join it elsewhere instead*;
    *Not on a trail*; *A card can't join itself*.
  - The glide and the toast came with it (they were step 4's); step 4
    keeps the trail line drawing in and checking the sidebar.
  - Script runner: `adrag` (Alt+drag), `apress` (an Alt press, to look
    mid-drag); `cards` logs each card's parent, trail, mark and label rect.
- ✅ 4. **Show it.** The joined card glides (`motion::Glides`) to its new
  place beside the target (or stays put, per the design note); the trail
  line draws in; the sidebar moves its row under the target; a toast names
  the trail it joined.
  Done 2026-11-25. The glide and toast came with step 3. A join's new line
  draws in from the target over `motion::FOLD` (eased out), following the
  card as it glides; its arrival dot shows once it gets there. The line
  a join or leave cuts fades out over the same time
  (`App::edges_in` / `edges_out`, set in `after_relink`; none when the
  desktop asks for less motion). `scene::route::part` cuts a path to a
  share of its length (unit-tested). The sidebar needed nothing: it is
  rebuilt from `trail_tree` every frame (a unit test now pins a joined
  card under its target and a left one among the other cards). Undo and
  redo of a join or leave change lines at once, as undo does for moves.
- ✅ 5. **Docs and checks.** GUIDE rows, the `?` sheet (`keys::NAMED`),
  CHANGELOG; a scripted e2e run (join, leave, undo) with screenshots.
  Done 2026-11-25: two GUIDE rows (join, leave), the README's trail line
  and key table, the `?` sheet checked (filtered to "trail": `a`, `A`
  and both Alt+drag rows show), and the step 3 e2e run again after step 4
  (same results).

**Decisions (step 1, 2026-11-25):**

- **Join moves a card.** A trail is every card its edges connect,
  so adding an edge to a card that already has a parent would merge
  both whole trails. Instead `join(card, target)` first cuts the card's
  edge from its parent (as `leave` does), then adds `target → card`. The
  card's own followers (its subtree) come with it; the rest of its old
  trail stays as it was. Joining a root brings its whole trail.
- **Inside one trail too.** Joining a card to another card of its own
  trail moves it there (a new parent), unless the target is the card or
  one of its followers: that would make a cycle, so it's refused with a
  toast (*Can't join a card to its own trail below it*). Joining a card
  to its present parent does nothing (no undo step).
- **Where it goes (owner, 2026-11-25): it glides beside the target,** to
  the spot a link opened from the target would take
  (`placement::beside_sized` at the card's own size). Its followers move
  by the same offset, so their shape is kept; they aren't fitted around
  other cards (Tidy does that). The move is part of the join's undo step.
- **Leave keeps the card where it is.** Only the edge goes; the line
  fades out. Its followers stay with it as a trail of its own.
- **Names and colours.** Joined cards take the target trail's mark, field
  by field: the target's name, colour, pin and sidebar place where it has
  one, else the joining trail's. So a lone card dropped onto a named
  trail takes its name, and a named trail dropped onto a lone card keeps
  its own. A card that leaves (with its followers) loses the mark: no
  name, colour, pin or place, so a cut-out card doesn't look like a
  second copy of its old group. Undo gives the old marks back. (Colours
  changed later the same day: see "Joined trails keep their colours"
  below.)
- **Stacks.** A joining card, or a target, in a stack: that stack is
  unfolded first, as `fold` does, and its cards glide out; undo folds it
  again. The same for `leave` on a card in a stack.
- **Several at once: yes**, one undo step. With a selection, every
  selected card joins the target, except the target itself and cards
  whose parent is also joining (they come along under it). They join as
  siblings in reading order of where they are now (top to bottom, then
  left to right), each placed beside the target in turn. `leave` on a
  selection cuts each selected card out the same way.
- **Which cards.** Any card: pages, notes and sticky notes, since a trail
  edge doesn't care what the card shows. A lone card can be a target; it
  becomes the trail's first card.
- **Saved format.** Unchanged: joins and leaves are only edges, marks and
  rects, all saved already (no version bump).
- **Every trail its own colour** (owner, 2026-11-25: "all trails different
  colours so we can tell them apart"). A trail takes a colour when it
  forms: a link or note opened from a lone card, a join of two lone cards,
  a card leaving with followers. The colour is the one the board's trails
  use least, the earliest of blue, green, magenta, orange, cyan, red on a
  tie (`Scene::fresh_trail_colour`, `colour_new_trail`). Six colours, so a
  seventh trail shares one with the trail that has had it longest
  unshared. C still cycles colours, the accent included. A lone card's
  colour isn't shown (no dot) until it has a trail. Board format 7: a
  board of 6 or lower has its uncoloured trails coloured once on load
  (`Scene::colour_trails` in `storage::load_board`), oldest trail first;
  after that, a trail set back to the accent stays so.
- **Shift+A cuts a trail line** (owner, 2026-11-25, replacing "leave
  your parent"): a card with one line has it cut at once (its parent's,
  or, for a trail's first card, its one child's); with two or more,
  letters go on the cards at the other ends (in reading order; a folded
  one's letters on its stack's front) and the one typed or clicked is cut
  from. Cutting splits the trail there: the side away from the trail's
  start leaves with the cards after it (`Scene::cut` over `leave_many`,
  `Scene::neighbours`). Press it twice to free a card from both sides. A
  selection of several still leaves its trails as before. Ctrl+K and the
  card menu call it *Cut a trail line…* (the menu shows it on any card
  with a line).
- **Two kinds of join** (owner, 2026-11-25): a *merge* (the default:
  `a` then letters, `:join`, Alt+drag, *Join a trail…*), where the
  joining cards take the target trail's colour and the trail stays one
  colour; and a join *keeping its colour* (Shift on the last letter,
  `:join-keep`, Alt+Shift+drag, *Join a trail, keeping its colour…* in
  Ctrl+K and the card menu, the unbound `join-trail-keep` key), below.
  `Scene::join_many(cards, target, keep)`; `join` / `join_keeping`.
- **Joining, keeping colours** (owner, 2026-11-25): colour belongs to a card's *colour part*
  (`Scene::colour_part`: its trail's cards of its colour), the name, pin
  and place to the whole trail. A joining part of two or more coloured
  cards keeps its colour; a single card (or an uncoloured part) takes the
  target's; a lone target takes the first kept colour, else a fresh one.
  C recolours one part; R names the whole trail and keeps every part's
  colour (`set_trail_mark` writes the colour to `id`'s part only). A line
  takes the colour of the card it runs into; each further part's first
  card has a colour dot (`trail_labels`). **S folds one colour part**,
  so a trail can have a stack per colour; a line into or out of a folded
  part runs from its stack's front (`Scene::shown_edges`). Leaving: a
  part of another colour than the card it leaves keeps its colour (it
  came in with it); otherwise, two or more cards take a fresh colour.
  Fresh colours count each colour part as one use.
- **Links between trails** (owner, 2026-11-25: keep a card joined to two
  trails, the line staying when one folds). A second trail edge would
  merge the trails, so a *link* is a separate kind of line
  (`scene::link`): kept in `Scene::links`, apart from trail edges, so
  trails, parents, join, leave, Fold, Tidy and the sidebar's tree never
  see it. Each trail keeps its name, colours, group and stack.
  Decisions (owner said "go on" to the defaults): the key is **`gl`**
  (letters on the cards in view; `g` alone does nothing on the board, so
  it is safe as a prefix); it is **dashed**, in the colour of the card
  linked to, routed and dotted as trail lines are, and runs straight up
  or down between cards one above the other; **the card stays put**.
  Also `:link [card]`, *Link to a card…* in Ctrl+K and the card menu,
  Ctrl+Alt+drag a label onto a card; a selection links every card.
  Refused within one trail and for a duplicate. A folded end is drawn at
  its stack's front (`Scene::shown_links`). Shift+A counts link ends
  among a card's lines: picking one removes only the link. Closing a card
  removes its links, undo brings them back (`Edge::link` carries them);
  link and unlink are one undo step (`Edit::SetLinks`). Sidebar: a link
  icon (with a count past one) at the end of a linked card's row; not
  yet clickable. Save as Markdown: a `See also:` item under the card.
  **Board format 8**, so an older build refuses a board with links
  rather than dropping them. Checked by `run13.sh` (`s13-*`).
- **Room for labels between stacked cards** (owner, 2026-11-25: a card's
  title covered the card above). A label is a fixed 32 px on screen, so
  no board gap fits it at every zoom; of the choices (label moves onto
  its own card, label shrinks into the gap, wider gaps) the owner chose
  wider gaps: `placement::GAP_Y` = 120 between cards one above the other
  (new cards and Tidy; side by side stays `GAP` = 60). Labels fit down to
  a zoom of about 0.27. Older boards keep their places until T.
  Checked by `run14.sh` (`s14-gapbefore/gapafter.png`).

### M16: Lighter and quieter 🟡

Asked by the owner on 2026-11-24, after a side-by-side measurement on
their machine: make Trailboard use less memory and CPU than it does now,
and at least no more than a plain Chromium with the same pages.

**Baseline** (2026-11-24, `~/.cache/browser-bench/`: `bench.py`,
`results.jsonl`, screenshots). Each browser alone on workspace 9, a fresh
profile, the same six sites (Wikipedia article, a GitHub repo, BBC News,
YouTube home, MDN JavaScript, a subreddit), 45 s to load, then 60 s
measured. Memory is PSS summed over every process of the run, counted
through a systemd scope (the engine moves its main process into its own
`app-org.chromium.Chromium-<pid>.scope`; that is counted too). CPU is
percent of one core (16 threads here). Live sites vary run to run, so
these are ranges of two runs, good to ±20–30 %.

| | Memory | Peak loading | CPU after load | Processes | Start |
|---|---|---|---|---|---|
| Chromium (stock) | 868–944 MB | 1.1–1.2 GB | 8–12 % | 20 | 0.45–0.61 s |
| A Gecko-based browser | 1.75 GB | 3.1 GB | 11 % | 22 | 1.04 s |
| Trailboard, 6 pages live, no ad blocking | 1.34–1.67 GB | 1.5–1.8 GB | 13–17 % | 22–35 | 0.61–0.72 s |
| Trailboard, defaults (4 live, ad blocking) | 1.28–1.43 GB | 1.4–1.5 GB | 7–24 % | 16–17 | 0.55–0.61 s |

Idle with no sites: Chromium 478 MB, 0.8 % CPU. A short Trailboard run
with only its start card used ~16 % CPU (one run, to be confirmed). Not
measured yet (the run was stopped): idle for the other two, and CPU while
playing a 1080p H.264 video (`site/video.html`). All three load the
VA-API driver (`iHD_drv_video.so`).

**Baseline 2** (2026-11-25, `scripts/bench.py` at `88a17de`, release
build, saved pages without scripts, default live cap, three runs each,
medians; the reference browsers in private windows with throwaway
profiles: "Ref A" is stock Chromium, "Ref B" the Gecko-based one). This
is the table later steps are measured against:

| Case | Program | Memory (PSS) | Range | CPU % of a core | Processes |
|---|---|---|---|---|---|
| idle | Trailboard | 551 MB | 545–557 | 10.4 | 9 |
| idle | Ref A | 434 MB | 428–436 | 3.2 | 12 |
| idle | Ref B | 584 MB | 580–587 | 0.4 | 11 |
| still | Trailboard | 683 MB | 657–689 | 9.7 | 14 |
| still | Ref A | 584 MB | 575–585 | 1.9 | 17 |
| still | Ref B | 787 MB | 762–802 | 1.3 | 16 |
| video-h264 | Trailboard | 557 MB | 553–563 | 27.3 | 9 |
| video-h264 | Ref A | 464 MB | 458–466 | 9.1 | 12 |
| video-h264 | Ref B | 689 MB | 657–712 | 10.5 | 11 |
| video-hevc | Trailboard | 547 MB | 538–548 | 25.5 | 9 |
| video-hevc | Ref A | 456 MB | 451–464 | 9.3 | 12 |
| video-hevc | Ref B | 687 MB | 683–731 | 11.8 | 11 |
| pan | Trailboard | 756 MB | 746–758 | 22.5 | 14 |

Runs agree within ±3 % on memory now. Trailboard is ~100 MB above Ref A
in every case, the size of its main process (`mem` on a still board:
PSS ~240 MB, page frames ~22 MB on the CPU and as much on the GPU for
six cards), and below Ref B. CPU is the clear gap: 10 % of a core doing
nothing (Ref A 2–3 %, Ref B under 1.5 %) and ~3× theirs during video,
which is step 2's polling loop plus step 6's frame copies. VA-API is
loaded by Trailboard and Ref A in every case.

**What it points at**

- Trailboard's main process holds ~300 MB of PSS (window, wgpu, board,
  page textures and thumbnails, history database, blocking rules), on
  top of the pages; the stock browser's whole run is ~400–700 MB less.
- The main loop runs with `ControlFlow::Poll` (`crates/app/src/main.rs`),
  pumping CEF and redrawing without waiting: the likely cause of idle CPU.
- The live cap (4) saved only ~100–250 MB against all six live.
- Process count jumps between runs (22 vs 35 for the same pages).

**What the code does now** (read 2026-11-25):

- `main.rs` loops forever: `engine.pump()` (CEF's `do_message_loop_work`),
  `app.tick()`, `pump_app_events(Some(Duration::ZERO))`, then sleeps 1 ms.
  The loop never waits on anything.
- `App::tick` ends with `request_redraw()` every time, so `draw()` runs at
  the display's rate (`PresentMode::AutoVsync`) with nothing changing: it
  walks every card, rebuilds layers and text, and presents a frame.
- `web` sets `external_message_pump` but has no
  `on_schedule_message_pump_work`, so CEF can't say when it next needs
  pumping; the loop makes up for it by pumping constantly.
- Timers are checked by polling inside `tick` (autosave after 3 s, key
  sequences expiring, `:later`, page-text reads, blocking-list refresh,
  desktop events from `desktop.rs`'s threads through a channel).
- Every live card keeps its last paint twice: `PaintFrame::pixels` on the
  CPU (`web`) and a page texture on the GPU (`compositor`). On the iGPU
  the texture is system memory too. A sleeping card keeps its texture to
  show its last frame; that is by design and stays.
- No CEF switches for process count (`on_before_command_line_processing`
  only sets the render node, `OverlayScrollbar` and quiet-start switches).

**Decisions** (taken as defaults; the owner can change them):

- **Measure first, always.** No change counts without a before/after from
  the step-1 bench on this laptop. A change that saves less than the
  noise (±20 %) is dropped unless it is free.
- **Behaviour doesn't change.** Same pictures, same motion, same keys; a
  card that was live is still live. Lower defaults (live cap, sleeping
  sooner) are proposed with numbers and left to the owner.
- **Site isolation stays on.** No `--single-process`, no
  `--disable-site-isolation-trials`, no `--process-per-tab` tricks that
  put different sites in one renderer. The sandbox stays on.
- **The event loop.** `ControlFlow::Wait` with a deadline: the loop sleeps
  in `pump_app_events(Some(timeout))` until the earliest of (a) the next
  CEF work time from `on_schedule_message_pump_work`, (b) the next app
  deadline (`App::next_deadline()`: animations, toasts, timers, autosave,
  key-sequence expiry), (c) a wake from another thread. Other threads
  (CEF's callback can come on any thread, `desktop.rs`, downloads) wake
  the loop through a waker the app gives `web` at start (a
  `Box<dyn Fn() + Send + Sync>` wrapping winit's `EventLoopProxy`), so
  `web` still doesn't depend on winit.
- **Redraw only when needed.** `App::needs_redraw` is set by input, a new
  `PaintFrame::serial`, any running animation, a window resize or theme
  change, and anything that changes `self.dirty`. A
  `TRAILBOARD_REDRAW=always` escape hatch keeps today's behaviour, for
  bisecting a missed redraw.

**Steps:**

- ✅ 1. **Repeatable benchmark** (2026-11-25). Done as below, with these
  choices: `scripts/bench.py --snapshot` saves the pages to
  `~/.cache/trailboard-bench/site/` with HTML, styles, images and fonts
  but **no scripts** (so runs agree; `--live` loads the real sites), and
  makes the two videos with ffmpeg (`testsrc2`, 1080p30, 30 s, looping).
  The six pages: Wikipedia, a GitHub repo, BBC News, MDN, a subreddit
  (old layout) and ArchWiki; a video home page was dropped since it is
  empty without scripts (the video cases cover video). Each page is
  served from its own `127.0.0.N`, so each is its own site and gets its
  own renderer as on the web. Trailboard starts with a space index
  already written, so no first-run welcome card. CPU is the scopes'
  cgroup `usage_usec` (it counts processes that exit, unlike
  `/proc/<pid>/stat`). The quick check is a script command `mem` rather
  than `log mem` (`log` already takes free text). `--against NAME=CMD`
  fills `{profile}` with a throwaway directory and `{urls}` with the
  pages; reference browsers run in a private window with that profile,
  never a real one. `pan` runs for Trailboard only. Results:
  `~/.cache/trailboard-bench/results.jsonl` (each row has the git rev
  and a `--note`), logs and screenshots under `out/<time>/`.
  The reference browsers needn't run again: a Trailboard-only run adds
  each other program's newest saved runs (same case) to its table, and
  `scripts/bench.py --report` prints the saved runs without running
  anything. Baseline 2's reference rows are the batch `20261125-203848`
  (a copy: `results-baseline-20261125.jsonl` beside `results.jsonl`).
  Plan as written: move `~/.cache/browser-bench/bench.py`
  into `scripts/bench.py` (our own; it compares against any command given
  with `--against`, names no other browser). Pages served locally from
  saved copies (`scripts/bench-site/`, made by `--snapshot` once; kept out
  of git, under `~/.cache`), so runs agree. Cases: **idle** (start card
  only), **still** (six pages loaded, nothing touched), **video** (1080p
  H.264 and HEVC from `scripts/media-check.html`'s files), **pan** (the
  script runner pans for 20 s). Three runs each, medians, PSS through the
  systemd scope as now, CPU from `/proc/<pid>/stat` over the window.
  Output appends to `~/.cache/trailboard-bench/results.jsonl` and prints
  a Markdown table. A script line `log mem` (main-process RSS/PSS, live
  cards, GPU texture bytes) added to `script.rs` for quick checks.
- 🟡 2. **Wait instead of poll** (2026-11-25, code written, uncommitted;
  measuring not finished). Done: `web` stores CEF's
  `on_schedule_message_pump_work` time (`Engine::next_due`, a 33 ms
  fallback as in CEF's own sample), a waker (`web::set_waker`,
  `web::wake`, winit's `EventLoopProxy`) and `web::activity()` (bumped by
  every event and paint). `main.rs`: `ControlFlow::Wait`, pumps CEF when
  due or after a window event, redraws when something moved last frame,
  an event came, a page painted or a deadline passed, then sleeps until
  `min(next_due, deadline)`. `App::next_deadline`: toasts, tip dwell, key
  sequences, prefix panel, sidebar peek, desktop debounce, script waits,
  page-text reads, `:later`, scroll flash, camera rest (sharp re-raster),
  autosave, and `IDLE_TICK` (250 ms) for the rest. The renderer
  fingerprints each frame (`frame_key`, unit-tested) and skips presenting
  an unchanged one; `TRAILBOARD_REDRAW=always` presents every frame.
  Background threads (desktop, board photo, blocking lists) call
  `web::wake()` after sending. Script commands `frames` (presented /
  skipped) and `mem` added. **Found on the way:** winit on Wayland holds
  `RedrawRequested` until the frame callback, so a zero timeout spun the
  loop (pan 109 %); the loop never uses a zero timeout now.
  Measured so far (release): idle main process 5.9 % → 1.2 % (`percpu`
  probe); bench, one run each: idle 5.0 % (was 10.4), video-h264 21.5 %
  (27.3), pan 20.6 % (22.5). An earlier full run *with the spin bug*:
  still 3.4 % (9.7). E2e: `run13.sh` (links) gives the same `cards` log
  and screenshots as before; the fold run's slow-motion frames match.
  **Left to do:** the full bench (`scripts/bench.py --note "step 2"`,
  Trailboard only, ~30 min); the latency check in
  `~/.cache/trailboard-m16/lat/` (`run.sh anim anim.html`: a 60 fps page
  should present ~60 frames/s, compare with `TRAILBOARD_REDRAW=always`;
  `run.sh still still.html`: ~0 frames/s at rest, Ctrl+D scrolls
  smoothly); the other `run*.sh`; then decide whether `IDLE_TICK` and
  the 33 ms pump fallback can be longer. Plan as written, in order, each checked alone:
  a. `on_schedule_message_pump_work(delay)` in `web`: stores the due time
     (atomic, any thread) and calls the waker; `Engine::next_due()`
     returns it; the loop pumps only when due (and once after every
     batch of input, which CEF needs for input latency).
  b. `App::next_deadline()` and `needs_redraw`: audit every `Instant`
     and animation in `app` (camera glide, `scene::motion`, sidebar,
     toasts, sheet, caret blink, loading spinners, ghosts, peeks) so
     each one either reports its next frame or asks for a redraw.
  c. `main.rs`: `ControlFlow::Wait` and the timeout above; drop the 1 ms
     sleep; `draw()` only when `needs_redraw`.
  d. `desktop.rs` and the other threads call the waker after sending.
  Goal: under 1 % of a core on a still board (idle and still cases);
  paint-to-screen latency no worse (checked with a page animating at
  60 fps and scrolling by keys). E2e: the full set of existing `run*.sh`
  scripts still pass, since they lean on timing.
- ⬜ 3. **Where the main process's memory goes.** Measure, don't guess:
  `heaptrack` (or `DHAT` through a feature) on a still run, plus a count
  of page textures × sizes, `PaintFrame` buffers, thumbnail and icon
  caches, `page-text.db`'s SQLite cache, the adblock engine and the font
  atlas. Write the breakdown here.
- ⬜ 4. **Cut the largest parts**, likely candidates in order:
  - `PaintFrame::pixels` duplicates the texture: upload straight from
    `on_paint` into a staging buffer the renderer owns (or keep one
    shared buffer per size), so a live card holds its frame once.
  - A sleeping card's texture at full resolution: keep it until the card
    is off screen, then swap it for a downscaled copy (the size it is
    drawn at, rounded up); wake restores full size from the new paint.
  - Page textures sized to the zoom: a card drawn at 30 % doesn't need a
    100 % texture (ties into `settled_page_scale`).
  - Caches given a size cap (thumbnails decoded on demand, SQLite
    `cache_size`), the filter engine built once and shared.
  - The allocator: try returning freed memory (`malloc_trim` after a
    card sleeps, or a different allocator) only if step 3 shows
    fragmentation.
- ⬜ 5. **Renderers and sleeping cards.** Check by `/proc` that a sleeping
  card's renderer exits (and its frames' renderers); log why processes
  vary between runs (spare renderer, per-site frames, utility processes).
  Try one at a time, with numbers: `--disable-features=SpareRendererForSitePerProcess`,
  `--renderer-process-limit`, discarding hidden cards sooner than the
  live cap (a hidden card that hasn't been seen for N minutes sleeps),
  and a lower default cap (3). Each kept only if site isolation and
  `Awake::Keep`/`Active` work as before.
- ⬜ 6. **Video.** CPU while playing 1080p H.264 and HEVC in a card
  against the reference browser; confirm VA-API decode (`chrome://media-internals`
  in a card, `intel_gpu_top`); count frame copies (GPU → CPU in
  `on_paint` → texture). If copies dominate, note what
  `on_accelerated_paint` (dmabuf, §5) would take; doing it is a later
  milestone of its own.

  **Detailed plan (written 2026-11-26, owner: "work on Trailboard's
  performance", video first; start here in a fresh session).**

  *The gap.* CI run 45773585181 (software rendering, 1080p30 clip in one
  card): Trailboard ~365 % of a core in every codec, Chrome 34–46 %, the
  Qt WebEngine browsers ~180–250 %. Laptop (Baseline 2 + step 2):
  Trailboard 21.5 %, Ref A 9.1 % (H.264). Memory isn't the issue here.

  *What one video frame costs today* (code read 2026-11-26):
  1. Chromium composites the frame and hands it to the browser process
     (OSR, no `on_accelerated_paint`). `windowless_frame_rate` is 60 for
     a visible card (`FULL_FPS`, `app.rs` `show_card`).
  2. `web` `on_paint` (`crates/web/src/lib.rs`, ~l.947) copies the
     **whole** buffer into `PaintFrame::pixels` (`extend_from_slice`,
     8 MB at 1080p) even when the dirty rect is small, then bumps the
     serial with the dirty rect.
  3. The app (`app.rs` ~l.9860) calls `Renderer::update_page_part` with
     `changed_since`; `queue.write_texture` copies the changed part
     again (into wgpu's staging), then to the texture.
  4. `Renderer::render` (`crates/compositor/src/lib.rs` ~l.570): a
     partial redraw only if the damage is ≤ `PARTIAL_MAX` (0.5) of the
     window; a video card bigger than half the window redraws the whole
     window (background dots and grain, shadows, every card) each frame.
     Frames < `MOTION_GAP` (100 ms) apart skip the canvas copy.

  *Steps, each measured alone, kept only if it beats the noise:*
  - ✅ **6a. Measure where the CPU goes.** (Done 2026-11-26.
    `run.py` rows' `by_kind` is `[n, pss, cpu_pct]`, CPU counted per pid
    over the window with its kind taken at first sight; Trailboard runs
    with `TRAILBOARD_PERF=1` (`crates/app/src/perf.rs`: paints/s and
    bytes copied in `on_paint` from `web::paint_stats`, bytes uploaded,
    frames presented/partial/skipped, ms per frame in `render` and % of
    the time; waiting for the surface counts in render), read back into
    the row's `perf`; the summary gets two tables.)

    **First breakdown** (run 45842886738, perf-6a 97f0a78, 3 runs each,
    medians; same runner per case):

    | Case | Total | Main process | CEF children | Paints/s | Copied = uploaded | Frames/s (partial) | Render |
    |---|---|---|---|---|---|---|---|
    | video-h264 (EPYC 9V74) | 371 % (Chrome 37 %) | 350 % | 21 % | 20 | 167 MB/s | 20 (0) | 46 ms/frame, 92 % of the time |
    | video-vp9 (EPYC 7763) | 372 % (Chrome 49 %) | 330 % | 26–42 % | 16–24 | 110–136 MB/s | 16–24 (0) | 38–57 ms, 91–93 % |

    *Read:* the cost is **our own drawing on llvmpipe**: the main thread
    spends ~92 % of its time in `Renderer::render`, ~40–55 ms a frame,
    and llvmpipe's threads spread that over the 4 cores (the main
    process's ~350 %). No frame was partial: the video card is more than
    `PARTIAL_MAX` of the window, so every paint redraws the whole
    1920×1080 window. Because the loop is that busy, CEF's 30 fps video
    is shown at only 16–24 frames/s. Chromium itself is cheap (renderer
    and zygote children ~20–40 %, no GPU process of its own on the
    runner). The `on_paint` copy is ~8 MB a paint, ~170 MB/s: small next
    to the render. *So the order changes:* **6e (draw less) first**, then
    6c (frame rate), with 6b and 6d after, and probably small. (Earlier
    run 45840344177 built master by mistake, see "How to measure"; its
    split matched: main process 305–348 %.) CEF's zygote-forked children
    keep the zygote's command line, so in this run they all show as
    `zygote` (the GPU process among them: it is zygote-forked too). Fixed
    after it: `kind_of` names them by their threads (`VizCompositorTh` →
    gpu-process, `Compositor` → renderer, `Chrome_ChildIOT` → utility),
    and reads Chrome's rewritten title, checked on a live Trailboard and
    Chromium here. (i) On `bench`, `run.py`:
    record CPU per process kind as well as PSS (`by_kind` → `[n, pss,
    cpu_pct]`; ticks per pid are already read), so a row says how much is
    the main process vs the GPU process vs the renderer. (ii) In
    Trailboard, a script command or the existing `frames` counter logged
    once a second in the video case: page paints/s, bytes uploaded/s,
    frames presented/s (full vs partial), and time in `render`. Put the
    first breakdown here before changing code.
  - ✅ **6b. CEF compositing switches** (done 2026-11-27: knob kept, no default; no gain in CI).
    Without accelerated paint, Chromium composites on the GPU process
    and reads the frame back; `--disable-gpu-compositing` makes it
    composite in software straight into the paint buffer. Add an
    env override for extra CEF switches (e.g. `TRAILBOARD_CEF_ARGS`,
    read in `on_before_command_line_processing`, documented as a debug
    knob) so it can be A/B'd without a rebuild, try it in CI and on the
    laptop, and check that VA-API decode still works on the laptop
    (`chrome://media-internals` in a card). Keep only if it helps on
    both, or make it the default only where there's no GPU.
    *How, given CI-only measuring:* add `TRAILBOARD_CEF_ARGS`
    (whitespace-split, appended in `on_before_command_line_processing`,
    `crates/web/src/lib.rs` ~l.430; log what was added). To A/B in CI
    without touching the harness, make a throwaway commit on `perf-6e`
    that sets a default in code, or add an `env` input to `bench.yml` /
    `run.py` on `bench` (asks the owner first: a push to `bench` starts a
    ~1 h run). Try `--disable-gpu-compositing` first; maybe also
    `--disable-gpu` (CI has no GPU anyway). Watch by_kind: today
    gpu-process ~15–20 % and renderer ~20–25 % in the video cases. The
    laptop/VA-API check can't be done under the CI-only rule: if CI
    shows a win, keep it as an env knob, or the default only when the
    adapter is software (wgpu `AdapterInfo::device_type == Cpu`), and
    ask the owner to check VA-API on the laptop themselves.
    **Knob done 2026-11-27:** `TRAILBOARD_CEF_ARGS` (`web::extra_switches`,
    unit-tested; dashes optional, `name=value` kept), browser process
    only; the logger starts after CEF, so the switches are kept in
    `web::extra_cef_args()` and `main` logs "TRAILBOARD_CEF_ARGS added:
    …". Checked here: with `--disable-gpu-compositing`, `chrome://gpu` in
    a card says Compositing "Software only" and lists the switch on its
    command line (without: "Hardware accelerated"); Video Decode still
    says "Hardware accelerated" on the laptop's iGPU (a report only, not
    a playback check). Zygote-forked children keep the zygote's command
    line, so `/proc/*/cmdline` can't show it.
    **CI A/B** (run 45983554997 on throwaway branch `perf-6b-try`,
    e55d4d2: the variable defaulted to `--disable-gpu-compositing`; the
    log confirms it was added): **no change.** video-vp9 (Xeon) 175 %
    (174 % without), gpu-process 13.5 % (13.3), renderer 23.1 % (22.7);
    video-h264 (EPYC 9V45) 140 %, 4.7× Chrome (4.8× without); idle 2.4 %,
    still 0.8 %. The runner has no GPU, so Chromium composited in
    software already; the switch only names what it did. *Decision:*
    keep `TRAILBOARD_CEF_ARGS` as a debug knob, change no default (no win
    to buy with the laptop's VA-API risk). Not tried: `--disable-gpu`
    (same reasoning: nothing for it to turn off on the runner). The
    throwaway branch is to be deleted (local and `origin`).
  - ⏭ **6c. Frame rate.** Skipped (2026-11-27): CEF already paints at
    the video's 30/s (6e's runs), so there is nothing to cut. Check whether a 30 fps video makes CEF paint at
    60/s (6a's paints/s). If so, try `windowless_frame_rate` 30 for a
    card while it plays media (or always, if scrolling stays smooth;
    the latency check in step 2's notes).
  - ✅ **6d. Copy less in `on_paint`.** (Done 2026-11-27, measured; see the notes at the end.) Copy only the dirty rows/rect
    into `PaintFrame::pixels` when the size didn't change (the rest is
    already there). Keeps the CEF → app rule (no app code in CEF
    callbacks). Removing this copy altogether (handing CEF's buffer to
    the renderer) would break that rule: owner's call, ask first.
    *Where:* `crates/web/src/lib.rs` `on_paint` (~l.960–1000): the view
    (`f.pixels.clear(); f.pixels.extend_from_slice(src)`) and the popup
    (`p.pixels`) both copy the whole buffer. When not `resized` and the
    buffer length matches, copy each dirty rect's rows (`popup::bounding`
    of `dirty`, or each rect) from `src` into `f.pixels` at the same
    offsets; a resize, a length change or no dirty rects still copies
    all. `web::paint_stats` counts bytes copied: make it count only what
    is copied, so `perf`'s `copied` MB/s shows the saving. A unit test
    for the row copy (pure function: dst, src, width, rect). *Expect:*
    small for a page-filling video (its dirty rect is the whole page,
    ~8 MB/paint either way), real for carets, spinners and small videos;
    so also look at idle in the run. Keep it if nothing gets worse.
    **Code done 2026-11-27 (`219ad8e` on `perf-6e`):**
    `popup::copy_dirty(dst, src, w, h, same_size, dirty)` copies each
    dirty rect's rows (clipped; overlapping rects may copy a few rows
    twice, cheaper than a union's extra area), or everything after a
    resize, a length change, or no rects; returns the bytes copied, which
    `PAINT_BYTES` now counts (so `perf`'s `copied` drops while
    `uploaded` stays). Used for the page frame and for the popup buffer
    (a popup that changes size, or a new one, still copies whole).
    Unit tests `copy_dirty_takes_only_the_dirty_rows`,
    `copy_dirty_takes_everything_when_it_cant_tell`; clippy clean. Local
    e2e (`~/.cache/trailboard-e2e-6d/run.sh`, shots `a/b/c.png`): typing
    in a field, a `<select>` popup opening and closing, a 10/s counter
    (small rects) and a scroll all draw right, no stale pixels.
    **Result** (perf-6e run 45981644696 at 7bc362c, vs 45973419447 at
    35159a4; medians of the runs with the usual card size, see below):

    | Case (CPU) | before (6e) | 6d | Chrome, same job |
    |---|---|---|---|
    | video-h264 (Xeon 6975P-C, both) | 165 % (4.9×) | 170 % (4.8×) | 33.9 → 35.6 % |
    | video-vp9 | 174 % (Xeon, 4.3×) | 227 % (EPYC 7763, one run, 4.4×) | 40.5 → 51.6 % |
    | idle: copied | 13.8 MB/s | **1.0 MB/s** | — |
    | idle / still CPU | 3.0 / 0.6 % | 4.0 / 0.5 % (other CPU; 2.4 % in the 6b run) | 0.7 / 0.3 % |

    As expected: the video, whose dirty rect is the whole page, copies
    the same ~4.7 MB a paint; idle (a caret) copies 14× less. CPU is
    within noise everywhere. Kept. **Found while reading it:** in 3 of the
    12 video runs since (vp9 runs 1–2 here, h264 run 0 in 45983554997)
    the card filled the window: 8 MB paints, 0 partial frames,
    ~350 % / ~233 %. Not 6d: big from the first second, so the startup
    camera did it (see §7, "First fit uses the requested window size").
    Those runs are left out above; read every run's `copied` (≈140 MB/s
    usual, ≈240 big) before taking medians until it's fixed.
    **Fixed 2026-11-27** (the owner: "do all"): `App::opening_fit` keeps
    the opening cards' rect and the fitted camera; `refit_opening` on
    each `Resized` fits again while the camera is still that one (no goal,
    no Solo); any click, wheel or key drops it. Checked on Hyprland (the
    log says "opening fit redone for a 1920×1080 window", the cards fill
    the window's width). **So from here on the bench's card fills the
    window** (≈1892 px, 8 MB paints, more than `PARTIAL_MAX` of the
    window: full frames again): step 6's numbers so far were mostly on the
    smaller card, and 6f measures both again on the new size.
  - ✅ **6e. Draw less.** Raise `PARTIAL_MAX` (try 0.9) so a large playing
    card redraws only its own rect; and/or skip the background shader
    under opaque cards. Check no seams (screenshots), as for the partial
    redraw.
    **Done 2026-11-27, on branch `perf-6e` (not merged).** Master run
    45845659696 (e1edf21) confirmed 6a on all three codecs: main process
    318–341 % of 359–367 %, `render` 86–91 % of the time, 0 partial
    frames; the card is ~1425×800 of 1920×1080 (55 %, just over 0.5).
    Both halves done in `compositor` `render`: (1) `PARTIAL_MAX` 0.9;
    (2) **occlusion**: `cover` finds the opaque `Shape::Page` (colour
    alpha 1; CEF pages paint over an opaque background) hiding the most
    of the frame's area (scissor, or the whole window), inset past its
    rounded corners (r·(1 − 1/√2) + 1 px). Inside it drawing starts at
    that page's instance (no background, shadow, card body, nor earlier
    layers' text); `cut_px` gives the up-to-four strips round it, drawn
    as before. Works for full frames too, so pan, Solo and a fresh
    canvas gain as well. Unit tests `frames_start_at_the_page_hiding_the_most`,
    `cut_leaves_strips_round_the_hole`; clippy clean. Decision: no new
    `perf` field (run.py's parser expects the current count); partial/s
    already shows (1).
    **First measurement** (2026-11-27; master run 45970171546 vs perf-6e
    run 45971592897; medians of 3, same CPU model only):

    | Case (CPU) | master | perf-6e (occlusion) |
    |---|---|---|
    | video-h264 (EPYC 7763) | 368 %, 38.3 ms/frame, 23.7 fps | 351 %, 29.8 ms/frame, 30 fps |
    | pan (Xeon 6975P-C) | 366 %, 26.6 ms/frame, 35 fps | 349 %, 17.2 ms/frame, 52 fps |

    (vp9 ran on different CPUs: 304 % → 209 %, not comparable.) The
    occlusion cuts a frame by ~22–35 %, and the loop now keeps up with
    the video's 30 fps; total CPU barely moves because the loop draws
    more frames. Screenshots right (card corners, ring, overlapping cards
    in still). **But 0 partial frames still:** a video filling its page
    repaints the whole texture, which `update_page_part` recorded as
    `None` (draw the whole window), and a full frame in motion skipped
    the canvas, so the next one couldn't be partial either. Fixed: only a
    new texture (resize) means `None`; whole-page damage is the page's
    rect; and the canvas is used when the frame's layout equals the last
    one shown (`shown_layout`: only page pixels changed), even in motion.
    **Result** (perf-6e run 45973419447, both video cases on Xeon
    6975P-C): every video frame partial (30/30 a second), 12.7–13.1
    ms/frame, `render` 38–39 % of the time.

    | Case | master (45970171546) | perf-6e final | Chrome, same job |
    |---|---|---|---|
    | video-vp9 | 304 % (9.3× Chrome) | **174 %** (209 % before the fix, same CPU) | 40.5 % → 4.3× |
    | video-h264 | 368 % (8.1× Chrome) | **165 %** | 33.9 % → 4.9× |
    | pan | 366 % | 352 % (more frames: 35 → 53/s) | — |
    | idle / still | 2.8 % / 0.7 % | 3.0 % / 0.6 % | 0.7 % / 0.3 % |

    Video roughly halved; the goal (≤ 2× Chrome) isn't met yet. The
    screenshots are right (card corners, ring, edges, overlapping cards).
    CEF paints at the video's 30/s, not 60, so **6c has little to win**
    here. The main process is still ~135 % while `render` is only ~38 % of
    the main thread's time: llvmpipe's worker threads, the canvas copy, and
    the `on_paint` copy plus upload (6d). Next: 6d, then 6b.
  - 🟡 **6f. Report.** Update the table at the top of M16 with CI and
    laptop numbers; decide with the owner whether dmabuf paint
    (`on_accelerated_paint`, §5) becomes its own milestone.
    **Started 2026-11-27; numbers to come from one joint test with the
    owner** ("do all and then we test all together"):
    (1) CI, all five Trailboard cases on `perf-6e` at its head (also 6g's
    pre-merge run): `gh workflow run bench.yml --ref bench -f ref=perf-6e
    -f cases=all -f only=trailboard,chrome -f runs=3 -f save=false`.
    With the opening-fit fix the video card fills the window, so expect
    video near the ~350 % of the big-card runs (full frames: the page is
    more than `PARTIAL_MAX` of the window), against Chrome's video at the
    same full-window size: the fairer comparison, and the new reference.
    (2) Laptop, owner present: `scripts/bench.py --note "step 6"`
    (Trailboard only, release build), compared with Baseline 2.

    *What step 6 found, for the report:* on a software renderer (CI) the
    cost is our own drawing, per pixel: ~13 ms a frame for a 1425×800
    card, ~30 ms for one filling 1920×1080; partial frames and
    occlusion (6e) cut what is drawn round the page, not the page itself.
    Copying (6d) and Chromium's compositing (6b) were never the cost
    there. On a real GPU (the laptop) drawing is cheap and the copies
    are what's left: 8 MB a paint out of CEF's buffer, then again into a
    texture, 30 times a second for a 1080p video.

    *dmabuf paint, recommendation:* **yes, its own milestone**, after the
    owner picks its place against M17 (Windows). `on_accelerated_paint`
    hands a dmabuf of each frame; importing it into wgpu (Vulkan
    `VK_EXT_external_memory_dma_buf` through wgpu-hal, no public wgpu API
    for it) drops both copies and the upload. It helps only with a GPU:
    CI (llvmpipe) won't show it, so it needs laptop measurements, which
    the CI-only rule doesn't allow today; and it needs a fallback to
    today's path (no GPU, Windows, an import failure). For CI's numbers
    the next lever would be drawing a page shown at 1:1 as a plain copy
    instead of a filtered, rounded quad; not in step 6, noted here only.

    **CI result** (run 45987313397, perf-6e 09d3b77, all five cases, 3
    runs each, medians; every run had the full-window card: `copied`
    ≈249 MB/s in each video run, so the race is gone):

    | Case (CPU) | Trailboard | main / renderer / gpu | Chrome | ÷ Chrome | perf |
    |---|---|---|---|---|---|
    | video-h264 (Xeon 6975P-C) | 268 % | 244 / 19 / 5 % | 35.1 % | 7.6× | 30 fps, 21 ms/frame, 64 % |
    | video-av1 (Xeon 6975P-C) | 260 % | 237 / 19 / 4 % | 35.4 % | 7.3× | 30 fps, 21 ms/frame, 63 % |
    | video-vp9 (Xeon Platinum 8370C) | 345 % | 307 / 32 / 6 % | 63.4 % | 5.4× | **25 fps**, 34 ms/frame, 84 % |
    | pan (Xeon Platinum 8573C) | 354 % | 351 % main | — | — | 39 fps, 23 ms/frame |
    | idle / still (Xeon 6975P-C) | 2.8 / 0.6 % | | 0.6 / 0.3 % | | idle copies 1 MB/s |

    Screenshots checked: every video filling the window and playing,
    clean card corner, the pan moving, the still board drawn. *Read:*
    with the card the size of the window every video frame is a full
    frame again (0 partial; the page is more than `PARTIAL_MAX` of the
    window), so the cost is drawing ~2 M pixels on llvmpipe each frame;
    on the slower vp9 runner the loop drops to 25 fps. These can't be set
    against master's 8–9× directly: master's runs mostly had the smaller
    card (the race). **The step's goal (≤ 2× Chrome in CI) is not met**;
    what's left in CI is drawing the page itself (the 1:1 copy idea
    above), and on a GPU the copies (dmabuf). Pan: 354 % at 39 fps
    (352 % at 53 fps before; the cards are bigger now).
    **Laptop:** not run yet (`scripts/bench.py --note "step 6"`, the
    owner's machine); 6f stays 🟡 until it is, or the owner drops it.
  - ✅ **6g. Merge `perf-6e` into `master`** (done 2026-11-27, owner: "merge it in master"; fast-forward, the pre-merge run is 6f's 45987313397) (owner, 2026-11-27: only
    after 6d, 6b and 6f are done, not step by step). All of step 6 lives on
    the branch `perf-6e` until then. Before merging: `cargo test`,
    `cargo clippy`, and a short CI run of all five Trailboard cases on the
    branch. The push to `master` starts the full ~1 h bench and saves
    its results (the new baseline on the chart); read it like
    "Run … read" in the Handoff.

  *How to measure* (the owner prefers CI to the laptop for bench work):
  push the change to a branch (not `master`, which starts a full
  ~1 h run) and start a short run on it:
  `gh workflow run bench.yml --ref bench -f ref=<branch> -f cases=video-vp9,video-h264 -f only=trailboard,chrome -f runs=3 -f save=false`
  (run 45840344177 found that dispatching on `master` took `github.sha`
  and ignored `-f ref`; fixed 2026-11-26, `REF` takes the sha only on a
  push, so either branch works)
  (~10–15 min; `save=false` keeps experiments out of `results/` and the
  chart). Compare with the same cases on `master` run the same way, on
  the same CPU model (each row's `runner`).
  **Only one bench run at a time:** `bench.yml` has
  `concurrency: bench, cancel-in-progress`, so a second dispatch cancels
  the first. Start one, `gh run watch <id>` in the background, then the
  next. Jobs land on different CPUs (EPYC 7763 / 9V45 / 9V74, Xeon
  6975P-C / Platinum); where the CPUs differ, compare each Trailboard
  number with Chrome's in the same job (Trailboard ÷ Chrome).
  **Reading a run:** `gh run download <id> -p 'rows-*' -p 'shots-*'` into
  the scratchpad; each `rows-<case>.jsonl` line is one run of one program:
  `cpu_pct`, `pss_mib`, `runner`, `by_kind` (`{kind: [n, pss, cpu]}`, the
  main process is the `trailboard` kind) and, for Trailboard, `perf`
  (`paints`, `copied`, `uploaded`, `presented`, `partial`, `render_ms`,
  `render_pct`). Take medians over the 3 runs. Look at the Trailboard
  screenshots (`shots-<case>/<case>/<case>-trailboard-N.png`), cropping
  a card corner to check for seams.
  **Latest numbers to beat** (perf-6e at 35159a4, run 45973419447, Xeon
  6975P-C): video-vp9 174 % (Chrome 40.5 %), video-h264 165 % (Chrome
  33.9 %), pan 352 %, idle 3.0 %, still 0.6 %; video perf: 30 paints/s,
  30 frames/s all partial, ~13 ms/frame, `render` 38 % of the time; main
  process ~135 %, renderer ~20–25 %, gpu-process ~15–20 %. Laptop numbers for the
  final table only: `scripts/bench.py --note "step 6x"`, Trailboard only.

  *Done when:* video CPU in CI at most ~2× Chrome's (now ~9×) and on the
  laptop no more than Ref A + 50 %; idle/still/pan no worse; `cargo test`,
  `cargo clippy`, the e2e scripts, and a screenshot of a playing card.
- ⬜ 7. **Report.** Rerun the bench after each step and keep the table
  above current; README gets a short "Memory and CPU" line only if the
  goal (no more than the reference browser with the same pages) is met.

**Open questions for the owner** (none block steps 1–4): a lower default
live cap or sleeping hidden cards sooner, if step 5 shows a real saving;
whether dmabuf paint (step 6) becomes its own milestone.

---

### M16b: Benchmark in CI 🟡

**Built 2026-11-25** (owner: name the browsers on `bench`, as many as
possible, run on every push). Branch `bench` (orphan): `browsers.toml`
(17), `install.py`, `make_site.py` (pages made by us, not copies),
`run.py` (Xvfb + xdotool; processes by session and tree, not systemd
scopes), `report.py`. Changed from the plan below: `master` carries the
same `.github/workflows/bench.yml` instead of a trigger file (a push only
runs workflows from the pushed branch, and `workflow_dispatch` and
`repository_dispatch` need the file on the default branch anyway); it
takes its tools from `bench` and saves there. Left: the first green run
(browsers that fail are skipped, see the run summary), and the owner
setting Settings → Pages → Source: GitHub Actions for the chart.


Asked by the owner on 2026-11-25: measure Trailboard against reference
browsers on GitHub Actions on every push, from a branch of its own that
holds only the benchmark and its results (no app code, no releases), like
`cef-build`.

**What it can and can't tell.** Hosted runners have no GPU, no VA-API and
no Hyprland, and share their CPU: everything draws and decodes in
software. So CI numbers compare the programs *with each other* (all on
the same runner, in the same job) and *over time*; they are not
comparable with the laptop's (`scripts/bench.py`, M16), which stays the
reference for real numbers.

**Decisions** (taken as defaults; the owner can change them):

- **An orphan branch `bench`**: `bench.py`, `browsers.toml`, `site/`,
  `results/`, `.github/workflows/bench.yml`. Reference browsers are named
  only there (config and results), never in `master`'s docs.
- **Triggers:** a push to `bench`; every push to `master` through a
  ~10-line `.github/workflows/bench-trigger.yml` on `master` that only
  dispatches the `bench` workflow with the pushed commit (GitHub runs a
  push's workflows from the pushed branch); and by hand
  (`gh workflow run bench.yml --ref bench -f ref=<commit>`).
  `concurrency` cancels an older run still going when a newer push comes.
- **Browsers:** those on the runner image (two Chromium-based, one
  Gecko-based, one more Chromium-based), plus two installed by the job
  (a Gecko-based one from its release archive, a Chromium-based one from
  its apt repository). Each starts in a private window with a throwaway
  profile and no first-run screens; its version is saved with each row.
- **Pages:** copies of pages whose licences allow it (Wikipedia, MDN,
  ArchWiki, with attribution) and three pages of our own shaped like a
  news site, a code page and a forum. No copies of other sites' pages in
  a public repo. Videos made by ffmpeg in the job, VP9 and AV1 (the
  standard CEF has no H.264/HEVC).
- **Trailboard:** built from the commit with the standard CEF (cached by
  version), run on a headless sway.

**Steps:**

- ⬜ 1. **The branch.** `git switch --orphan bench`; the files above.
- ⬜ 2. **`bench.py` anywhere.** From `scripts/bench.py`: without
  Hyprland (no `hyprctl`; `grim` on the headless output), each program
  in its own cgroup (`sudo systemd-run --scope`), PSS and CPU as on the
  laptop, programs interleaved run by run (A B C, A B C) so the runner's
  slow moments hit all alike. Writes `results/<date>-<sha>.jsonl`.
- ⬜ 3. **`browsers.toml`.** Per browser: install, start (private,
  throwaway profile, first-run screens off) and version commands.
- ⬜ 4. **The workflow.** A setup job (apt: sway, grim, ffmpeg, Mesa
  software rendering, fonts, build libraries; Rust with the cargo cache;
  the standard CEF, cached; `cargo build --release` of the commit; the
  two extra browsers), then one job per case (idle, still, video, pan)
  in parallel, each running every program three times, then a save job:
  merges the rows, commits them to `bench`
  (`permissions: contents: write`), uploads screenshots and logs as
  artifacts (14 days). About 15 min of setup and 30–60 min of runs.
- ⬜ 5. **Where results show.**
  - The run's **summary page** (`$GITHUB_STEP_SUMMARY`): the medians
    table, each program per case, and Trailboard's change against the
    last run (`gh run view`).
  - **`results/README.md`** on `bench`, rebuilt by every run: the latest
    table and a history, one line per tested commit.
  - **A chart page** on GitHub Pages, built from `results/*.jsonl` by
    the save job: memory and CPU per case over time, one line per
    program, each point linking to its commit and run.
  - Screenshots and logs as **artifacts**, to check that every page
    really loaded.
- ⬜ 6. **The regression check.** The save job fails the run (so GitHub
  emails the owner) when Trailboard is more than 20 % worse than the
  last run's on any case, in memory or CPU, and the reference browsers
  in the same run moved less than 10 % (a slow runner is not a
  regression). The summary names the case and the figures.
- ⬜ 7. **First runs.** Check every program's screenshots (pages drawn,
  video playing, no welcome screens), that three runs agree within
  ±10 % on memory (else a longer settle), and the chart and the check on
  a planted regression (a commit with a busy loop).
- ⬜ 8. **`bench-trigger.yml` on `master`**, once the branch is green.

**Open questions for the owner:** none block the steps. The 20 %
threshold and the browser list can change after the first runs.

---

### M17: Windows ⬜

Asked by the owner on 2026-11-25: run Trailboard on Windows too. Linux
stays the first platform: every step keeps the Linux build, its tests and
its behaviour as they are, and nothing on Linux waits for Windows.

**Where things stand.** The building blocks are cross-platform (winit,
wgpu, CEF, and the pure `scene` and `storage` logic), but the code has no
`cfg(target_os)` anywhere: it assumes Linux. The Linux-only parts:

- `web::sandbox` (namespaces, setuid `chrome-sandbox`) and
  `web::render_node` (VA-API's `/dev/dri` nodes).
- `app::desktop`: the Omarchy theme through inotify, Hyprland's rounding
  and window size through `hyprctl` and its Unix socket.
- `app::clipboard` (the Wayland/X11 selections, the primary selection),
  `app::editor` (`xdg-terminal-exec`, `$EDITOR`), opening files and
  folders with `xdg-open`.
- Paths from `XDG_*` variables (`main.rs`, `backdrop.rs`,
  `storage::downloads`).
- Install and packaging: `scripts/install-cef.sh` and the PKGBUILD fetch
  the `linux64` CEF; our media CEF (`cef-build` branch) is built for
  Linux only; `scripts/bundle.sh` makes a Linux archive.

**Decisions** (taken as defaults; the owner can change them):

- **Windows 10 and 11, x64.** ARM64 later, if asked.
- **The standard CEF first.** It can't play H.264, AAC or HEVC (as on
  Linux with `TRAILBOARD_CODECS=0`); a Windows media CEF is step 6.
  Widevine on Windows needs a signed (VMP) build, which we can't make, so
  protected video is not promised there.
- **A portable zip first** (unpack, run `trailboard.exe`). An installer
  and code signing (without it SmartScreen warns on first run) are the
  owner's call, at step 7.
- **Theme:** the fallback theme, light or dark following Windows' own
  setting, with Windows' accent colour. Corners: *Rounded*; *Follow
  Hyprland* is hidden.
- **Data** in `%LOCALAPPDATA%\Trailboard` (board, profile, history),
  downloads to the user's Downloads folder; `XDG_DATA_HOME` still wins
  when set, so the e2e runs keep working.

**Open questions for the owner:** a Windows machine or VM to test on
(this laptop can't check a Windows build; CI only builds and runs unit
tests); whether to sign the app; an installer or the zip only.

**Steps:**

- ⬜ 1. **Platform seams.** Put each Linux-only part behind a small
  module with a Linux and a Windows side (`cfg(target_os)`), Windows
  first as stubs that do nothing harmful: sandbox and render node (none),
  desktop watching (Windows' light/dark and accent instead of Omarchy,
  no Hyprland), clipboard (the Windows clipboard; no primary selection),
  open file/folder (the shell), the editor (`$EDITOR` or Notepad), data
  and download paths. Linux code moves, it doesn't change: tests and an
  e2e run show the same behaviour.
- ⬜ 2. **CEF on Windows.** Fetch the pinned `windows64` minimal build
  (an `install-cef.ps1`, or a Rust `xtask` that both systems share) and
  the `.cargo/config.toml` it writes; `libcef.dll` and resources beside
  the exe; the same exe as CEF's sub-process; `external_message_pump` on
  the main thread as on Linux. CEF's own sandbox on Windows links
  `cef_sandbox.lib` into the exe: start without it, then turn it on
  before the first release. The engine-pin test covers the new script.
- ⬜ 3. **Window, GPU and input.** wgpu on D3D12 (Vulkan as fallback);
  high-DPI and per-monitor scale; IME (TSF) through winit; key mapping
  checked on Windows layouts. **Ctrl+Alt is AltGr** on many keyboards:
  Ctrl+Alt+drag (link) and any Ctrl+Alt key need another modifier there,
  decided after a test. The Windows key is the system's, so nothing
  binds it.
- ⬜ 4. **CI.** A GitHub Actions `windows-latest` job: `cargo build`,
  `cargo test`, `cargo clippy` (warning-free), with CEF cached. The Linux
  job stays as it is.
- ⬜ 5. **Run it for real.** On a Windows machine: the welcome, opening
  links beside their source, trails, Fold, Tidy, Solo, downloads, the
  sidebar, Ctrl+K, find, history and search, ad blocking, saving and
  reopening a board, a board copied over from Linux (same format). The
  script runner works there too (`TRAILBOARD_SCRIPT`); screenshots with
  the system's own tool. Fix what breaks; record what is Windows-only in
  §7.
- ⬜ 6. **Media on Windows.** Build our media CEF for Windows in the
  `cef-build` workflow (a much larger build than Linux; may need bigger
  runners) and pin it as on Linux; hardware decode through D3D11.
- ⬜ 7. **Package and docs.** `bundle` for Windows: a zip with the exe,
  CEF and licences; then the installer and signing if the owner wants
  them. README: a Windows install section and the badge; GUIDE: Windows
  differences (keys, paths, what isn't there: Omarchy, Hyprland, primary
  selection).

---

## 4. Testing

| Layer | How |
|---|---|
| `scene`, `storage`, omnibox | `cargo test` |
| End-to-end | `TRAILBOARD_SCRIPT=file` with an isolated `XDG_DATA_HOME` under `~/.cache` |
| Visual | `grim -g` with geometry from `hyprctl clients -j` |

Every step ends with tests green, clippy clean, a scripted e2e run, and a
screenshot check for visual changes.

## 5. Risks

| Risk | Mitigation |
|---|---|
| RAM: one renderer per card | Live cap, hidden off-screen pages, snapshots for sleeping cards |
| CPU: software OSR copies | Upload on new paint serial only; dmabuf later |
| CEF version drift | Pin `cef` crate and CEF build; `scripts/install-cef.sh` |
| Wayland quirks | Logical-key shortcuts; script runner for tests |
| Legal | Clean-room only |

---

## 6. Handoff (updated at the end of every session)

**Last session:** 2026-11-27 (fifty-third): **code layout**, asked by
the owner ("work on the folder structure", then "still app folder has
too many files … do a proper folder structure"). On branch
`refactor-layout`, no behaviour change. Decisions: group by feature, not
by layer (a feature's `impl App` methods, state types and widgets live
together); keep the tree shallow (one level of folders per crate); crates
stay flat under `crates/` (matklad's layout, already used). Done:
`app.rs` split into files, then the app crate arranged as `app/` (core)
plus `board/`, `trails/`, `keyboard/`, `page/`, `ui/`, `builtin/`,
`system/`, `dev/`, with 14 feature state types moved out of `app/mod.rs`
(now 978 lines); `App`'s fields and methods `pub(crate)`; imports made
explicit everywhere (no `use super::*`). `scene` grouped into five
folders and `web` into `engine/`, `page/`, `blocking/`, both re-exported
so outside paths are unchanged; `web`'s tests moved beside their code.
`compositor/lib.rs` split into `layer`, `damage` (with its tests),
`renderer`. `storage` left flat (19 small files, one per format). Layout
written in `CLAUDE.md`. Verified after each step: build, clippy clean, 604
tests; e2e runs (typing, popup, counter, scroll; sidebar, palette, key
sheet and settings by their log lines; the renderer by `TRAILBOARD_PERF`
lines, partial frames working). Screenshots stopped once the owner's own
windows overlapped Trailboard's (the shots that caught them were
deleted). Not changed: code isn't rustfmt-formatted (no config; 2,276
diffs on master), so only import blocks were tidied. Git sees moved code
as new files: `git blame -C -C` / `git log --follow -M`. **Next up:** the
owner reviews and merges `refactor-layout` after the `master` bench run
45989683312 is read; later, if wanted: split `App`'s state into
per-feature structs (the real coupling; folders don't fix it), rustfmt
with a config, then the open M16 calls below.

**Session before:** 2026-11-27 (fifty-second): **M16 steps 6d and 6b
done** on `perf-6e`, plus a fix, and 6f started. 6d: `on_paint` copies
only the dirty rects (`popup::copy_dirty`); idle copies 14× less, video
unchanged, CPU within noise (run 45981644696). 6b: `TRAILBOARD_CEF_ARGS`
debug knob; `--disable-gpu-compositing` changed nothing in CI (run
45983554997 on the throwaway branch `perf-6b-try`, deleted locally), so
no default changes. Fix (cf665f3): the opening cards are fitted again
when the window gets its real size; it was a race that decided the
bench's card size (3 of 12 video runs had the full-window card). 6f: the
findings and a dmabuf recommendation are written in 6f; numbers wait for
the joint test. Verified: unit tests, clippy, local e2e with screenshots
(typing, popup, counter, scroll; `chrome://gpu` with and without the
knob; the opening refit on Hyprland), both CI runs' screenshots. The
owner pushes (the session's permission check refuses `git push`). Then the joint CI run
(45987313397, all five cases, owner pushed and dispatched): results in
6f; video 5.4–7.6× Chrome with the full-window card, the ≤ 2× goal not
met. Then **6g: `perf-6e` merged
into `master`** (fast-forward; asked by the owner). **Next up:** read
the full bench that the push to `master` starts (~1 h; the new baseline
on the chart, read it like "Run … read" below); then the owner's calls:
the laptop run for 6f (or drop it), dmabuf's place as a milestone, and
whether to try the 1:1 page copy for CI inside M16.

**Session before:** 2026-11-27 (fifty-first): read master run 45845659696
(it confirms 6a's split on vp9, h264 and AV1); **M16 step 6e done** on
branch `perf-6e` (pushed, not merged): no drawing under opaque pages,
`PARTIAL_MAX` 0.9, a page repainted whole still redrawn only where it
shows, and the canvas kept while only page pixels change. **Video CPU in
CI roughly halved** (vp9 304 → 174 %, h264 368 → 165 %; ~4.5× Chrome,
was ~9×); idle, still and pan no worse (table in 6e). Verified: unit
tests, clippy, three CI runs (45970171546 master, 45971592897 and
45973419447 perf-6e) with screenshots checked. Not verified: the laptop
(owner: CI only), a docked inspector or Solo by eye. **Next up (owner,
2026-11-27): M16 step 6d, then 6b, then 6f, all on branch `perf-6e`;
merge into `master` only after all of step 6 (6g).** Start a fresh
session with `git switch perf-6e` (it holds 6e and this plan); 6c is
skipped (CEF already paints at the video's 30/s). Each step: code + unit
test, clippy, commit and push to `perf-6e`, one short CI run (see "How
to measure", which now says how to read a run and lists the numbers to
beat), screenshots checked, results written into the step. Pushing
`perf-6e` and dispatching the Bench workflow on it is authorised (owner:
"push it and run the bench"); pushing `bench` or `master` is not.

**Session before:** 2026-11-26 (fiftieth): **M16 step 6a done**; the
breakdown is in 6a: ~92 % of the main thread is `Renderer::render`
redrawing the whole window on llvmpipe (no partial frames with a big
video card), Chromium is cheap. **Next up: 6e (draw less)**, then 6c.
Work so far, on branch `perf-6a` (pushed; this file's update committed
locally after the push): `TRAILBOARD_PERF=1` (`crates/app/src/perf.rs`, unit
test) and, uncommitted in `../trailboard-bench`, CPU per process kind
and the perf lines read into each row, with two new summary tables
(`report.py` `by_kind`, checked on made-up rows). Verified: `cargo test`,
`cargo clippy`, `run.py`'s log parsing on a sample log. Not verified: a
real run until CI (owner: bench on CI, not the laptop). Then, asked by
the owner ("fix it all and push it"): the workflow's `REF` fix (both
copies), `kind_of` splitting CEF's zygote children and Chrome's kinds;
perf-6a merged into master and both branches pushed, so a full run of
master with the new tools follows (its summary has the split per kind).

**Session before:** 2026-11-26 (forty-ninth): M16b brought to a trusted
state (runs 45754226156, 45761289882, 45773585181; see "Run … read"
below), the partial redraw measured in CI (idle 45.6 % → ~5 %), and the
video plan written (M16 step 6, "Detailed plan"). Before that,
2026-11-25 (forty-seventh): **M16 step 1, the
benchmark** (asked by the owner). `scripts/bench.py`: `--snapshot` saves
six pages (no scripts) and two 1080p videos under
`~/.cache/trailboard-bench/site/`; runs `idle`, `still`, `video-h264`,
`video-hevc` and `pan`, three each, against any `--against NAME=CMD`,
and prints medians. The script runner's `mem` logs the main process's
memory and page buffer sizes (`app::memstat`, `Renderer::page_texture_bytes`).
Baseline 2 is in M16. The owner's reference browsers run
private with throwaway profiles: `chromium --user-data-dir={profile}
--incognito --no-first-run --no-default-browser-check {urls}`, and
`~/.cache/trailboard-bench/zen.sh {profile} {urls}` (writes a `user.js`:
private windows, no welcome screen). Verified: unit test for `memstat`,
clippy, the full baseline (39 runs) with screenshots checked (pages
drawn, video playing, the pan moving the board). Not verified: `--live`
(the old path, unchanged). Then, same session: **M16 step 2 written but
not finished measuring** (owner paused it); state and what's left are in
step 2 below. The working tree holds steps 1 and 2, uncommitted.

**Session before:** 2026-11-25 (forty-sixth). **M15 Custom trails,
all five steps** (asked by the owner, ahead of M14). Any card can join a
trail or leave one, so a named, coloured trail is a group of any cards:
`scene::join` (`join_many` / `leave_many`, one undo step via
`History::relinked` and `Edit::SetEdges`); in the app `a` (letters on the
cards in view, or a click), `A`, Alt+drag a label onto a card,
`:join [card]` / `:leave`, Ctrl+K and the card menu; the card glides
beside the target, the new line draws in, a cut one fades out, and the
sidebar follows. Decisions under M15 (owner: the card glides beside the
target). Verified: unit tests (12 for join, one for `route::part`),
clippy, scripted e2e runs in `~/.cache/trailboard-e2e-join/` (`run.sh`
and friends; copy `board.json` to `data/trailboard/spaces/1.json` before
each) with screenshots, slow-motion runs for the line, the `?` sheet.
After the owner's review: the Alt+drag line is now a trail line, with a
faint card where the dragged one will land; a keyboard-only run (v, a,
a letter, A, undo) passes (`run7.sh`, `s7-*`). Not verified: a real
Alt+drag by hand under Hyprland (it may keep
Alt+drag for moving windows; then the modifier needs changing), several
selected cards in the app (unit-tested only), the motion at normal speed
by eye.

**Repo state:** on `master`, committed (M15, then Shift+A cutting one trail line).
After that, also asked by the owner: every trail gets a colour of its
own as it forms (least-used first; board format 7 colours older boards
once on load). Unit tests (scene, storage) and an e2e load of a version-6
board (`run8.sh`, `s8-*`). Then: joins keep their colours on request (two kinds of join:
merge, and keeping the colour) and S folds one colour part at a time (see the M15 decisions); unit tests, and an e2e join of a
green trail onto "Work" with each part folded on its own (`run10.sh`,
`s10-all.png`), and merge vs keep by keys and Alt+Shift+drag
(`run11.sh`, `s11-both.png`). Then: Shift+A cuts one trail line, with letters on
the cards at the other ends when there are several (`run12.sh` on a
board shaped like the owner's, `s12-both.png`). Then: **links between
trails** (`gl`, see the M15 decisions; board format 8): unit tests
(link, refusals, undo, close, save, fold, Markdown, dashes), an e2e on
the owner's board shape (`run13.sh`: link by letters, fold the top
trail, cut the link with Shift+A, Ctrl+Alt+drag and undo) with
screenshots `s13-linkletters/linked/linkfold/linkside.png` checked. Not
verified: Ctrl+Alt+drag by hand, the sidebar link icon isn't clickable
yet. Then: **room for labels between stacked cards** (`GAP_Y`, see the
M15 decisions): unit tests for placement and Tidy, and `run14.sh` before
and after T with screenshots.

**M16b first CI run** (2026-11-25, run 45665317928, master ea5b59f; the
owner paused here, check it first next session): build ✅ with the media
CEF; Trailboard runs in CI (sandbox on, llvmpipe Vulkan, pages drawn). At
pause, `pan` ✅ and the five browser cases still measuring. **To check:**
(1) `gh run view 45665317928` and its summary: which of the 17 browsers
installed and opened a window (failed ones say why in the summary; fix
their `browsers.toml` entries on `bench`: likely suspects the GitHub
release downloads Thorium, Floorp, Waterfox, ungoogled-chromium, and Pale
Moon); `gh run download 45665317928 -n shots-<case>` and look at every
browser's screenshot (pages drawn, video playing, no welcome or crash
screens); (2) that `results/` on `bench` got the run's file and README;
(3) **fix `pan`**: run 1 of 3 didn't pan (its screenshot shows the
gallery card filling the window, so the touchpad scrolled a page; 4.6 %
CPU against 360 % for runs 2–3): make `run.py`'s pan script put the
pointer on empty board, or pan by keys, and check each run's screenshot;
(4) software panning costs ~360 % CPU in CI (3.6 cores): expected with
llvmpipe at 60 fps, a baseline only; (5) the chart needs the owner's
Settings → Pages → Source: GitHub Actions (I wasn't allowed to change
it), plus `bench` under Environments → github-pages → deployment branches
for pushes to `bench`; (6) the regression check only works from the
second saved run. The `bench` branch is checked out as a git worktree
at `../trailboard-bench` (edit and push it there). Local edits to this file after ea5b59f are uncommitted
on purpose (a push to master starts a new CI run and cancels a running
one).

**M16b checks done** (2026-11-26, forty-eighth session; run
45665317928 read, fixes pushed to `bench` as run 45754226156). The owner
wants bench work checked on CI, not on the laptop (it's their working
machine). Found and fixed on `bench`:
(1) all four Gecko browsers sat on "profile cannot be loaded" in every
case (so their ~150 MB / 0 % rows were the error box): `run.py` gives
each Gecko run its own `HOME`/XDG dirs, and `install.py` runs as root
with `HOME=/root` instead of the runner's. (2) `gh_get` put the asset
regex inside the jq filter (`\.` is a bad jq escape): Thorium,
ungoogled-chromium, Floorp and Waterfox never installed; now `jq --arg`.
(3) Waterfox releases have no files: CDN by tag. (4) **Thorium dropped**:
GitHub has no Linux builds any more and the only source is an unsigned
apt repo at Chromium 138 (decision; the owner can bring it back).
(5) Video pages title themselves `…: playing` / `…: cannot play`;
`run.py` fails a browser run whose window never says playing (Chromium
and Opera can't play h264, GNOME Web showed black on h264; before, they
counted as ~0.5 % CPU). (6) `pan` presses Escape first: over a focused
card the touchpad scrolls the page (run 0's problem); verified the
board moves the same in each run. **To check on run 45754226156:**
every browser installed, every screenshot a page or a playing video,
pan run 0 moving, results saved; GitHub Pages still needs the owner to
enable it (deploy failed 404). **Still wrong in the harness:** GNOME Web
shows 0 % CPU while playing VP9 (its bwrap-sandboxed processes are
probably not counted); count by cgroup as `scripts/bench.py` does.

**Run 45754226156 read** (2026-11-26, forty-ninth session; measured
ea5b59f, before the partial redraw). All jobs green, regression check
passed (first with a run to compare), the chart deployed to
ayushkr2003.github.io/trailboard. Trailboard medians: idle 582 MB,
45.6 % CPU (runs 45.6/37.0/47.3; the baseline for the partial redraw);
still 697 MB, 0.5 %; video 617–648 MB, ~365–372 %; pan 738 MB, 358 %,
run 0 now pans. The Gecko browsers and the GitHub downloads work; Chromium
and Opera failing h264 is right. Fixed on `bench` after reading it:
(1) Floorp showed its welcome page and a release-notes tab in front of
every case: the `floorp.browser.welcome.page.shown` and
`floorp.releaseNotes.*` prefs. (2) Pale Moon played the video but keeps
the page title out of its window title: the video page now sends a
request to run.py's server (`/beacon/playing` or `/beacon/cannot-play`)
and run.py checks that instead of window titles, Trailboard included.
(3) GNOME Web: a "Set as Default Browser?" dialog (a GSettings key file
of the run's own, `ask-for-default=false`) and "cannot play" on every
codec (the GStreamer -good, -bad and -libav plugins installed).
`master`'s `bench.yml` synced with `bench`'s (it lacked the `HOME=/root`
install fix). **To check on the next run:** idle CPU against 45.6 % and
the idle screenshot (caret, no seams); Floorp, Pale Moon and GNOME Web
rows in every case; GNOME Web's video CPU not ~0 % (if it is, count by
cgroup); Zen idle at 24 % CPU against Firefox's 8 % (its own UI animating?).

**Run 45761289882 read** (2026-11-26; measured 4c931f8, the partial
redraw). **Idle CPU 45.6 % → 5.1 %** (runs 5.6/4.0/5.1); still, video
and pan no worse; idle screenshot right (caret, no seams). Floorp and
Pale Moon now measured in every case. Found and fixed on `bench`:
(1) GNOME Web "played" VP9 at 0 % CPU: the screenshot was frozen on the
clip's last frame and its web process had crashed; the page reported
once and stopped. The page now reports every 2 s while the video moves,
and run.py fails a run whose last report is over 6 s before the end of
the measured window. (2) GNOME Web's ~130-package apt install hit the
10-minute limit in one job: per-browser `timeout` in `browsers.toml`
(1500 s for it), apt retries, and one retry of any failed install.
(3) Each case is its own job and jobs land on different CPU models
(AMD EPYC 7763 / 9V45, Intel Xeon 6975P-C within one run): AV1 went
+50 % for every browser between runs. `report.py` now compares each case
with the newest earlier run of it on the same CPU model, says which, and
skips the regression check where there is none. The chart still mixes
CPU models (later: a line per CPU, or mark the points). (4) Pan run 0
did two thirds of the panning (721 events against ~1100): a cold first
launch. run.py now starts every program once, briefly and unmeasured,
before the measured runs (for all programs, so it stays fair).
**To check on the next run:** GNOME Web installed in all jobs and its
video either measured with real CPU or failed as "stopped"; no other
browser newly failing the 2-second check (Trailboard included); pan's
three runs alike; each case's "compared with" line.

**Run 45773585181 read** (2026-11-26). Same-CPU comparison, warm-up
(pan runs 362/362/361 %), installs and the 2-second playback check all
work. Idle 4.7 %, still 0.8 %. 13 of 16 browsers ran properly in every
case. Owner's decision: skip browsers only where they can't run on the
runner (`skip` in `browsers.toml`, listed in the summary): Chromium and
Opera in video-h264 (no H.264 decoder), GNOME Web in all three video
cases (its web process plays VP9/AV1 for a while, then crashes; H.264
never plays). Measured in their other cases.

**Idle CPU finding (M16):** CI idle 37 % is the start card's search
box caret: every blink (~2/s) repaints the page in CEF, uploads the
texture and redraws the whole window; with llvmpipe that costs ~20–40 %
of a core (layers identical frame to frame, only the page serial
changes; reproduced once with software GL before the owner asked for CI
only). On a GPU it is ~1.5 %. Options: redraw only the damaged rect
(keep the last frame in an offscreen texture, scissor to the card's
dirty rect from `on_paint`, blit), or cheaper shaders for shadows and
grain. Also: the other browsers' "idle" is an empty window, Trailboard's
a live start page, so the case isn't like for like.
**Done (code, 2026-11-26), to check on CI:** partial redraw. `web`
keeps each `PaintFrame`'s last 16 (serial, dirty rect) from
`on_paint` (`PaintFrame::changed_since`; a popup, resize or forgotten
serial means the whole frame). The app passes that to
`Renderer::update_page_part`, which uploads only the changed texels.
`render` keeps the last frame on a canvas texture (when the surface
takes `COPY_DST`) and, if only page pixels changed and they cover at
most half the window, loads it and redraws everything under a scissor
round the changed pixels mapped to the screen (`screen_damage`, with a
filtering margin; a change in the repeated left column covers the whole
quad), then copies the canvas to the swapchain. Frames under 100 ms
apart (video, panning) skip the canvas, so they pay no copy. The
script's `frames` reports partial frames. Unit tests in `web` and
`compositor`. **Check on CI:** idle CPU (was 37 %) and the idle
screenshot (caret, no seams); video and pan no worse; still unchanged.

**Next up (2026-11-26, owner):** Trailboard's performance, **video
first: M16 step 6, its "Detailed plan"** (6a measure → 6b CEF
compositing switch → 6c frame rate → 6d copy less → 6e draw less →
6f report). Then main-process memory (M16 steps 3–4: Trailboard's main
process is ~295–315 MB PSS in CI against Chrome's ~145 MB, the whole
memory gap; its renderers are no bigger than Chrome's). Then idle
(4.7 % vs Chrome 0.6 %, but Trailboard's idle shows a live start page
with a caret, the others an empty window). **Before starting:** the
per-case skips are edited but uncommitted in `../trailboard-bench`
(`browsers.toml` `skip`, `run.py`, `report.py`); this file's notes on
runs 45773585181 and the step-6 plan are uncommitted here. Ask the
owner before committing/pushing (a push to `bench` starts a ~1 h run;
docs-only pushes to `master` don't). CI is green and its data is
trusted except where `skip` says (see "Run 45773585181 read").
Earlier next-up, still open: finish M16 step 2's laptop checks (the
latency runs, the other e2e scripts). Planned (owner, 2026-11-25): M16b, the
benchmark on GitHub Actions from an orphan `bench` branch, with a chart
page and a regression check. Then M14
step 1, the `config.toml` reader (asked by the owner on
2026-11-21; format and decisions in M14). Bundle with
`CEF_PATH=~/.local/share/cef-standard`. Also retake the README
screenshots (`docs/images/`) in the new look, and try Alt+drag by hand. Planned since (owner, 2026-11-25): M17 Windows, after M14 and
M16 (its step 1 changes nothing on Linux, so it could go earlier).

**Backlog:** `:later` lines aren't saved across quits; the messages log is
this session's only; after Esc clears a find, n starts again from the view;
closing a card doesn't ask a page's "leave this page?" first; the
inspector's own extras (device toolbar, screencast, opening a file in
Sources from the page, a docked layout); the primary selection set from our window on
Hyprland; cookies on / off in the page menu; an entered card that navigates
to a Passthrough site stays in Page mode until given the keys again; glide
on undo/redo of moves; follow the desktop's animation switch live; verify
the setuid sandbox path without unprivileged user namespaces; more consent
platforms (TCF frames, Ketch, Termly, Borlabs, Axeptio); scriptlets /
`$redirect` resources (check uBO's licence); procedural cosmetic rules;
auto-scroll the sidebar while dragging; a downloads folder setting;
download progress on the board, ask before quitting with downloads running,
pause/resume; more settings (start page for an empty board, custom search
URL, card size); thumbnails off the main thread; thumbnails on history rows
or the sidebar; keep closed cards across restarts; a private space looking
different; undo for forgetting a page; site icons on history rows; prune
unused icons; letter tiles for sites without an icon; a zoom badge on
zoomed cards; Ctrl+scroll in a page to zoom it; keyboard selection of
several cards; a portal file chooser for Save as Markdown; open `crash.log`
from the crash toast; undoing the close of a stack member puts it back in
the stack; gi on the settings page misses its `input type=search`.

**Notes:**
- grim hangs while the display is off and catches the lock screen while
  locked; take screenshots with the screen unlocked. CEF needs
  `XDG_SESSION_TYPE=x11` (and no `WAYLAND_DISPLAY`) under XWayland.
- Put a `wait` after each `log shot …` script line; the first shot needs ~5 s.
- Script `open` with a data: URL needs percent-encoding; a `file://` page is
  easier. While a script runs, the window's IME stays off.
- The `?` sheet's board and page sections come from the key tables;
  give a new key action a name and text in `keys::NAMED`. App-level
  shortcuts outside the tables go in `hints::FIXED`.
- Known rough edges: a note's last keystroke can be lost if its card closes
  or sleeps in the same frame; a stale full-window frame can flash after
  leaving Solo; zooming can churn the live cap.
- Cookie tests need a real host: `site11/` served on `localhost:8765`
  (`python3 -m http.server`); `file://` pages have no cookies. Type
  `http://` in script addresses: a bare `localhost:8765/…` goes to https.
- Ctrl+S (or Ctrl+s with nothing loading) saves to the real
  `~/Documents/Trailboard/`; keep it out of e2e scripts. Downloads follow
  `user-dirs.dirs` in `XDG_CONFIG_HOME` (`config37` points them at `dl37`).

---

## 7. Known bugs

Open problems waiting for a decision or a session. Move an entry into its
milestone when it is taken up, and delete it here when it is fixed.

- **Performance against other browsers** (CI bench, 2026-11-27): video
  5–8× Chrome's CPU, panning 3.5 cores, idle 4× Chrome, ~150 MB more
  memory. Evidence, causes and what to try: [BUGS.md](BUGS.md) (PB1–PB4).

## 8. Future ideas

Beyond the milestones, roughly in priority order; each needs a short design
note before work starts.

- ⏸ **On hold for the owner:** semantic search (local embeddings next to
  FTS5), Explain (a side panel using the user's own LLM key), import from
  other browsers, password managers and passkeys.
- **Medium term:** CEF extensions for a curated set; PDF cards with
  quote-to-note; board sharing as a self-contained HTML/Markdown snapshot;
  multiple windows on different spaces; accelerated OSR (dmabuf from CEF to
  wgpu, no CPU copies); touchpad gestures; themes (light mode, accents,
  wallpapers per space); accessibility (screen-reader names for cards).
- **Long term:** end-to-end encrypted, self-hostable sync between your own
  devices; collaborative boards (CRDT scene model); macOS/Windows builds;
  a plugin API for custom card types.
- **Out of scope:** copying another app's branding, art, server API,
  prompts or paid features; telemetry without opt-in.

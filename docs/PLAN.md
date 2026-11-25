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
  accounts, payments, telemetry; macOS/Windows for now.

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
| ⬜ | **M16 Lighter and quieter** | Cut memory and idle CPU: a measured baseline, then the main process, the main loop, sleeping cards and renderer count |

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

### M16: Lighter and quieter ⬜

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

**What it points at**

- Trailboard's main process holds ~300 MB of PSS (window, wgpu, board,
  page textures and thumbnails, history database, blocking rules), on
  top of the pages; the stock browser's whole run is ~400–700 MB less.
- The main loop runs with `ControlFlow::Poll` (`crates/app/src/main.rs`),
  pumping CEF and redrawing without waiting: the likely cause of idle CPU.
- The live cap (4) saved only ~100–250 MB against all six live.
- Process count jumps between runs (22 vs 35 for the same pages).

**Steps:**

- ⬜ 1. **Repeatable benchmark.** Move the bench into `scripts/` (our own,
  no other browser named; it compares against whatever command is given),
  finish the idle and video cases, three runs each, and record medians.
  Prefer local copies of pages where possible so runs agree.
- ⬜ 2. **Idle CPU.** Wait for events instead of polling: sleep until the
  next input, a CEF paint or `OnScheduleMessagePumpWork`'s delay, an
  animation frame or a timer; redraw only when something changed. Goal:
  under 1 % of a core with a still board.
- ⬜ 3. **Main process memory.** Measure what the ~300 MB is (heap
  profile, wgpu buffers and textures, thumbnail cache, `page-text.db`
  cache, the filter engine) and cut the largest parts; free a sleeping
  card's texture.
- ⬜ 4. **Sleeping cards and renderers.** Check that a sleeping card's
  renderer really exits; see why the process count varies; consider a
  lower default live cap or sleeping off-screen cards sooner, and CEF
  switches that limit spare and per-frame renderers, without breaking
  site isolation.
- ⬜ 5. **Video.** CPU while playing 1080p H.264 and HEVC in a card, against
  Chromium; confirm hardware decode is used and frames aren't copied
  more than needed (dmabuf later, see §5).
- ⬜ 6. **Report.** Rerun the baseline after each step and keep the table
  here up to date.

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

**Last session:** 2026-11-25 (forty-sixth). **M15 Custom trails,
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

**Session before:** 2026-11-24 (forty-fifth). **M13 finished** (asked by
the owner): step 4, Widevine works in the media build as built (the
decrypted video draws in an off-screen card; owner: no setting), and step
6, keeping current: the README's "Updating CEF" checklist, a start-log
warning when the loaded `libcef.so` isn't the CEF this was built for
(`launch::engine_mismatch`, `web::loaded_engine_version`), a test that
the crate, `install-cef.sh` and the PKGBUILD pin the same engine
(`engine_pins_agree`), and `scripts/media-check.html`. Checking found HEVC
broken on this laptop when NVIDIA's driver takes the first render node;
fixed with `web::render_node` (`--render-node-override` to the iGPU). The
DRM message, GUIDE and CHANGELOG now say protected video works. Details
under M13 steps 4 and 6. Verified: unit tests, clippy, scripted runs in
`~/.cache/trailboard-e2e-widevine/` (media check passes; HEVC 8/10-bit
and HDR play again; screenshots). Not verified: the warning against a
really different engine, a real DRM service.
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
yet.

**Next up:** M14 step 1, the `config.toml` reader (asked by the owner on
2026-11-21; format and decisions in M14). Bundle with
`CEF_PATH=~/.local/share/cef-standard`. Also retake the README
screenshots (`docs/images/`) in the new look, and try Alt+drag by hand.

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

None open.

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

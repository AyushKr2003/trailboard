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
| 🟡 | **M13 Full media support** | H.264/AAC now via an interim third-party CEF (`install-cef.sh --codecs`, Arch default); next our own build (and HEVC, Widevine if they work) on GitHub Actions |
| ⬜ | **M14 Config file** | One hand-editable `~/.config/trailboard/config.toml` for settings, key bindings and theme; the UI writes it without losing comments |

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

### M13: Full media support ⬜

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

**Interim CEF** (owner's decisions, 2026-11-21): use the karere build until
our own build (step 1) exists, then point the same pins at ours. Anyone can
install it:

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
  gives only a SHA-256) and marks it `"codecs": true`. `scripts/bundle.sh`
  refuses such a CEF unless `ALLOW_CODECS=1`, so the portable archive
  never carries it.
- The pins live in two places that must move together with the `cef`
  crate: `scripts/install-cef.sh` and `packaging/arch/PKGBUILD`.
- On the owner's machine: codecs at `~/.local/share/cef`, the standard one
  at `~/.local/share/cef-standard` (for bundles).
- Before AUR: the owner should confirm shipping a PKGBUILD that downloads a
  patent-licensed third-party engine by default is acceptable there.

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

- 🟡 1. **Build script and workflow.** Written on `cef-build`
  (2026-11-21). First full run (44937270923, 2026-11-21→23): GN arguments
  accepted, 0 failed compile steps, disk fine (74 GB free), ~73k steps at
  ~2/s, ~14 min setup per stage; but all eight stages ran out of time with
  no net progress and the release was skipped while the run showed green.
  Cause: Siso's `.siso_fs_state` keeps nanosecond mtimes and GNU-format tar
  rounded them to the second, so every resumed output looked changed.
  Fixed 2026-11-23 (02c7861): pax checkpoints, a stage that leaves no fewer
  steps than the last one fails, a `finished` job fails a run no stage
  completed, required files and archive layout checked before release.
  Rerun 45212313507 started 2026-11-23; expected 2–3 stages. The chained GitHub Actions workflow
  above, around `scripts/build-cef.sh`: on a fresh Linux machine,
  fetch CEF's build tooling for the branch matching `install-cef.sh`'s
  version, build the minimal distribution with
  `is_official_build=true proprietary_codecs=true ffmpeg_branding=Chrome`,
  and pack it the way the CEF downloads are packed (same layout, a SHA-1
  and the build flags written beside it). The same script runs inside each
  workflow job (resuming from the checkpoint) or in one go on a big
  machine. Documented in the README: how to start a build, how long it
  takes, and how to fetch the release.
- ⬜ 2. **Install and check.** `install-cef.sh --from <tarball>` installs a
  local build into `~/.local/share/cef` (the standard download stays the
  default). Trailboard logs at start which build it runs (standard or
  codecs) and the About / `--version` line says so. Verified with
  `site/codecs.html` (H.264, AAC → yes) and real H.264 sites in a card.
- ⬜ 3. **HEVC through the GPU.** Chromium plays HEVC only via the hardware
  decoder (VA-API on Intel). Try the build flags and the VA-API switches
  in CEF's command line; keep them if HEVC plays without breaking other
  video, else record why not.
- ⬜ 4. **Widevine (trial).** Build with `enable_widevine` and the library
  CDM, let CEF fetch Google's CDM, and test a DRM page in a card. Open
  question: whether DRM video draws in off-screen rendering (how cards
  are drawn). If it works, a *Protected video* setting (off by default,
  as it downloads Google's module); if not, record the reason and drop it.
- ✅ 4a. **Cards that stay awake** (asked by the owner on 2026-11-23).
  Media playing keeps a card outside the live cap (10 min grace after a
  pause), and each card's awake button cycles *Auto*, *Keep awake* and
  *Always active*; see the Handoff of that day. Later: an awake mark on
  sidebar rows, a *Now playing* entry in Ctrl+K.
- 🟡 5. **Media behaviour.** Done: a card's media shows on its label
  (speaker / crossed-out speaker) and `:mute`. Still to do:
  `:media` keys and
  commands (play/pause, seek, speed, mute) that work on any site (not the
  site's own keys), a Solo-friendly full window, and a clear message when
  a video still can't play (format or DRM), instead of a blank player.
  Whether that message offers an outside player is decided here.
- ⬜ 6. **Keeping current.** A short checklist in the README for each CEF
  update: rebuild, re-run the codec page, smoke-test video; the start log
  warns when the installed build is older than the version
  `install-cef.sh` pins.

**Risks:** the build may fail on the cloud machine's distro (pin the
image); Widevine may not draw under off-screen rendering; HEVC depends on
the iGPU and driver; rebuild cost and effort every few weeks; a codec
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

**Last session:** 2026-11-23 (forty-second). Three things for the owner.
(1) The CEF build (M13 step 1): the first full run made no progress
(checkpoint mtimes; cause and fix under M13 step 1); the fix is pushed to
`cef-build` (02c7861) and run 45212313507 is building. (2) z framed a card
with an 8 px margin; it now leaves the 48 px Shift+F leaves round a lone
card (`scene::FIT_MARGIN`, also the label double-click), asked by the
owner from a screenshot of Shift+F on one card. Verified: unit test
(`geom::fit_leaves_the_margin_on_the_tighter_side`), clippy, a scripted e2e
run (`~/.cache/trailboard-e2e-fill/`: z and Shift+F screenshots of one card
are pixel-identical). (3) Fold and unfold
were instant; now each card slides onto its own sheet behind the front and
lands as that blank sheet (`motion::folding`, `stack_slot`, drawn from
`App::gathers`, which hide the sheet it is heading for), and unfold runs it
backwards (`Glides::unfold`, a sheet drawn under the card while its page
fades in), 340 ms eased in and out; designed with the owner; verified with
unit tests and a slowed (×8) scripted run with screenshots mid-motion
(`~/.cache/trailboard-e2e-fold/sheet.png`). Not animated yet: undo/redo of a
fold, peeking ([ ]), and the trail lines during a fold (they go at once).
Also looked into: the owner's Google sign-in is lost
on restart because their only space is **private** (`spaces.json`),
whose cookies live in memory by design; told them Ctrl+Shift+P. Offered,
not done (owner said "only z for now"): a maximum card size (the window
less the fit margin at 100 % zoom) and a clearer private-space mark.
The forty-first session's notes (cards that stay awake) are in git:
`git log -p docs/PLAN.md`.
**Repo state:** on `master`; the awake work above is committed (000e96a) and installed on the owner's machine from the Arch package.
**Next up:** M14 step 1, the `config.toml` reader (asked by the owner on
2026-11-21; format and decisions in M14). M13 step 1, the chained
GitHub Actions build on the orphan branch `cef-build` (worktree
`../trailboard-cef-build`): the first run made no progress (checkpoint
mtimes, see M13 step 1); the fix is pushed and run 45212313507 is
building. When it publishes `cef-156.0.26-media`, do M13 step 2. Until our release exists the
karere codecs CEF is installed at `~/.local/share/cef` (M13 "Interim
CEF"); bundle with `CEF_PATH=~/.local/share/cef-standard`. Also
retake the README screenshots (`docs/images/`) in the new look.

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

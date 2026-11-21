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
  paint serial changes; off-screen cards are hidden, beyond the live cap asleep.
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
| ✅ | M12 Keyboard first | Vim-like modes, page keys, scroll sections, command line, cards as tabs, search keywords, jumps/marks/saved pages, user key file, settings and site toggles, navigation extras, caret/visual mode, dialogs and fields, find back, `:messages` |
| ⬜ | **M13 Full media support** | Our own CEF build with H.264/AAC (and HEVC, Widevine if they work), built off this machine, opt-in |

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
Hyprland 0.57; `wi` (inspector in a card) needs devtools hosted off-screen.

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
of RAM; this machine has 7.6 GB and ~13 GB free, so builds run on a rented
cloud machine (about 32 cores / 64 GB: 2–4 h per build). Each CEF update
(Chromium security fixes every few weeks) needs a rebuild, or the engine
falls behind. H.264 / AAC are patent-licensed: fine for the owner's own
use; public releases keep the standard CEF and the codec build stays
opt-in. Only builds we make ourselves are used (no third-party engine
binaries).

**Steps:**

- ⬜ 1. **Build script.** `scripts/build-cef.sh`: on a fresh Linux machine,
  fetch CEF's build tooling for the branch matching `install-cef.sh`'s
  version, build the minimal distribution with
  `is_official_build=true proprietary_codecs=true ffmpeg_branding=Chrome`,
  and pack it the way the CEF downloads are packed (same layout, a SHA-1
  and the build flags written beside it). Documented in the README:
  machine size, time, cost, and how to fetch the result.
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
- ⬜ 5. **Media behaviour.** With the formats in place: a card's media
  shows on its label (playing / muted), `:mute` / `:media` keys and
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

**Last session:** 2026-11-21 (thirty-sixth). Docs only: shortened this
file (M12 to a summary, old logs pointed to git) and `CHANGELOG.md` (one
summary per area), and folded `docs/BUGS.md` and `docs/FUTURE.md` into §7
and §8 here. M12 marked ✅ (the owner removed step 17).
**Repo state:** on `master`; M12 done and committed; M13 planned.
**Next up:** M13 step 1, `scripts/build-cef.sh` for a rented cloud machine
(the owner is looking at a cloud account; a low-memory local mode is an
option if they build here with an external drive). Also retake the README
screenshots (`docs/images/`) in the new look.

**Backlog:** `:later` lines aren't saved across quits; the messages log is
this session's only; after Esc clears a find, n starts again from the view;
closing a card doesn't ask a page's "leave this page?" first; the page
inspector in a card (`wi`); the primary selection set from our window on
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

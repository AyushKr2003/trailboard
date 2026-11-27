# Changelog

All notable changes to Trailboard. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses
[Semantic Versioning](https://semver.org/) once it has releases.

Until the first release this is a summary by area. The full entry-by-entry
log up to 2026-11-21 is in git: `git show f554118:CHANGELOG.md`.

## [Unreleased]

### Added

- **Custom trails: any card can join a trail, or leave one**, so a named,
  coloured trail can hold any cards you like. On the board, `a` puts
  letters on the other cards in view: type one (or click the card) and
  the current card, or the selection, joins that card's trail and glides
  beside it as the trail's line draws in, taking the trail's name (a
  single card takes its colour too). Select several with `v` first to join them together. Or
  Alt+drag a card's label onto another card: a trail line follows the
  pointer, and over a card a faint card shows where it will land. `A`
  cuts a card out of its trail where it stands (with the cards opened
  from it). Also `:join [card]`, `:leave`, *Join a trail…* and *Leave
  trail* in Ctrl+K and the card menu. Each is one undo step.
- **Every trail has a colour of its own**, so trails are easy to tell
  apart: a new trail takes the colour the board's trails use least. C
  still changes it. Boards saved before this get their trails coloured
  once when they open.
- **Shift+A cuts a trail line.** On a card with one line it cuts that
  line; with more, letters show on the cards it is joined to, and typing
  one (or clicking the card) cuts the line to it. The trail splits there.
  Press it again to free the card from its other side too. Also `:leave`,
  and *Cut a trail line…* in Ctrl+K and the card menu.
- **Links between trails.** A card can be joined to a card of another
  trail without the trails merging: `gl`, then the other card's letters
  (or `:link [card]`, *Link to a card…* in Ctrl+K and the card menu, or
  Ctrl+Alt+drag a label onto a card). Both trails keep their own name,
  colour, sidebar group and fold; a dashed line joins the two cards and
  stays, from the stack, when one side is folded. Shift+A on the card
  offers the link among its lines and removes only it. Linked cards show
  a link icon in the sidebar, and Save as Markdown adds a *See also* line.
  Boards with links are saved as format 8, which older builds won't open.
- **Two kinds of join.** A join merges: the joining cards take the
  trail's colour. Or join keeping the colour (Shift on the last letter
  after `a`, `:join-keep`, Alt+Shift+drag, or *Join a trail, keeping its
  colour…*): the trail then shows several colours, S folds the cards of
  one colour into a stack and leaves the others out, and C recolours one
  colour.
- **Seconds on the start page clock.** *Customise* → *Show seconds*, under
  the time format; the clock then ticks each second. Off by default.
- **Media controls on any site.** `,` then a key, on the board or in a
  page, acts on the current card's video or audio: `,` or Space play /
  pause, h / l 10 s back / on (a count goes further), H / L a minute, 0
  the start, j / k slower / faster, = normal speed, m mute, r loop, f full
  window. By name: `:media play`, `:media seek 1:30` (or `50%`, `+10`),
  `:media speed 1.5`, `:media back 30`…; in the key file `media-toggle`
  and the like; in Ctrl+K *Play or pause media* and *Video full window*.
  A toast says what happened (*Paused · 1:23 / 4:56*). It reaches the
  media playing, else the largest in view, in the page or an embedded
  player from another site, and plays with sound without a click on the
  page first.
- **Full screen video fills the window.** When a page goes full screen
  (its player's button, or `,f`), its card goes Solo; Esc brings back
  the page and the board as they were.
- **A message when a video can't play**: its format isn't one the engine
  plays, or it's protected (DRM) and wasn't unlocked, instead of a
  player stuck at 0:00 with no word why.
- **Protected video (Widevine) plays** in the media build, at the
  software level Linux gets (HD on some services needs more). Chromium
  fetches Google's module by itself a few minutes after the first start;
  it works from the start after that.
- **Engine checks.** The start log warns when the installed CEF is older
  or newer than the one this Trailboard was built for, and says how to
  install the right one. `scripts/media-check.html` checks a new engine's
  codecs, Widevine and playback in one card; the README's *Updating CEF*
  lists the steps for each engine update, and a test fails when the pins
  in the install script and the Arch package disagree.
- **A memory and CPU benchmark.** `scripts/bench.py` runs Trailboard
  (and, with `--against`, any other program) alone on a workspace and
  measures memory (PSS over every process), CPU and process count, with
  a still board, two 1080p videos (H.264, HEVC), a pan and an idle
  case, three runs each, against saved copies of six pages. The script
  runner's `mem` line logs the main process's memory, live cards and
  page buffer sizes.
- **`TRAILBOARD_PERF=1`**, a debug knob: while frames are drawn, a line a
  second says what drawing cost (page paints from the engine, bytes
  copied and uploaded, frames presented and how many were partial, time
  in the renderer). The CI benchmark turns it on and also splits CPU by
  process kind (main, GPU, renderer…).
- **`TRAILBOARD_CEF_ARGS`**, a debug knob: extra engine switches,
  separated by spaces (e.g. `TRAILBOARD_CEF_ARGS=--disable-gpu-compositing`),
  to try one without a rebuild. The start log lists what it added.

### Changed

- **Quieter at rest.** With nothing happening the window no longer builds
  a frame four times a second only to find it unchanged, nor wakes 30
  times a second to look for work (each timer now wakes it when due, and
  it keeps drawing only after a frame laid out anew, not after every page
  paint), and the start page's caret stops blinking after 10 s without a
  key or click (it stays shown). Work handed over from a background thread
  (a decoded board photo, a desktop change) and toasts show at once.
- Docs: `docs/BUGS.md` now holds the researched next steps for each gap
  (a 1:1 page drawn as a copy, overdraw cuts, a memory breakdown, the
  `cef` crate's dmabuf import).
- Docs: `docs/BUGS.md` lists the performance gaps the CI bench shows
  against other browsers (video, panning, idle, memory), with evidence
  and next steps. The bench now measures five browsers (Chrome, Brave,
  Firefox, GNOME Web, Falkon) instead of fifteen.
- **A new mark.** The app icon is now a trail of five pages stepping down
  the board, each a skeleton of a different kind of page, in the default
  theme's colours (blue-grey, blue, magenta, orange, yellow). No key behind
  it any more. The one-colour icon and the welcome page's mark follow it.

- **Much less CPU when nothing moves.** The window now sleeps until a
  page paints, input comes, or a timer is due, instead of redrawing at
  the display's rate all the time, and a frame that would look the same
  isn't drawn again. `TRAILBOARD_REDRAW=always` brings back drawing every
  frame.
- **A blinking caret no longer redraws the whole window.** When only a
  page's pixels changed (a caret, a spinner, a small animation), just
  that part of the page is uploaded and only that part of the window is
  drawn again. It matters most without a GPU, where the start page's
  search box alone cost a fifth to a third of a CPU core.
- **Less drawing behind pages, and around a playing video.** Nothing is
  drawn under a page any more (the board, shadows and cards it hides),
  and a large card playing a video redraws only its own part of the
  window instead of the whole board each frame. Without a GPU, a playing
  video was costing over three CPU cores, almost all of it drawing.
- **Less copying when a small part of a page changes.** Only the parts of
  a page the engine says it repainted are copied out of its paint
  buffer, not the whole page each time (a blinking caret was copying
  every pixel of its page).

- **Cards one above the other leave room for the title.** New cards and
  Tidy put more space between cards stacked in a column (120 units
  instead of 60; side by side is unchanged), so a card's title no longer
  covers the card above it, down to well past the zoom where every card
  shows its title. Press T to space out a board made before.
- **HEVC checked on the GPU.** HEVC 8- and 10-bit (HDR10 too, shown in
  SDR), H.264, VP9 and AV1 all decode through VA-API on the Intel iGPU in
  the media build, with no extra switches.

- **Our own media engine.** `scripts/install-cef.sh --codecs` and the Arch
  package (by default) now install the CEF this repo builds on GitHub
  Actions (`cef-build` branch, release `cef-156.0.26-media`: H.264, AAC,
  HEVC through VA-API), in place of the third-party codecs build. New
  `install-cef.sh --from FILE` installs a media build from a local
  tarball (checked against `FILE.sha256` when it's there). The script
  now tells builds apart by their SHA-256, so it replaces another build of
  the same version. `trailboard --version`, the start log and crash
  reports say `standard build` or `media build` (read from the engine's
  `archive.json`, which the package and `scripts/bundle.sh` now keep).
  Checked in a card: H.264, AAC and HEVC files up to 1080p and a 1080p
  H.264 HLS stream play; Widevine isn't available yet.
- **Fold and unfold move.** Folding a trail (S) slid nothing: its cards
  vanished at once and the stack's sheets appeared. Now each card slides
  under the front onto its own sheet of the stack, its page fading on the
  way, so it lands as that sheet; unfolding runs it backwards, each sheet
  sliding out to its card's place as the page shows on it, the trail lines
  following. 340 ms, eased in and out (`scene::motion::FOLD`, `folding`,
  `stack_slot`, `Glides::unfold`); off with the desktop's reduced motion.
- **z frames a card as Shift+F does.** z (and a double-click on a card's
  label) now leaves the same 48 px margin round the card as Shift+F leaves
  round a lone card (`scene::FIT_MARGIN`), instead of 8 px, so the card
  sits in the window with the board showing round it.
- **Nothing covers a page to say its mode.** The *Page* / *Insert* /
  *Passthrough* / *Caret* chip in the bottom-left corner hid page
  controls under it (a video's play button). Now the entered card's ring
  takes the mode's colour (accent, green, red, magenta; `Theme::keys_ring`
  keeps the four apart on any theme) and the window title names the mode,
  what is half-typed, the scroll area and what `;` hints do. Solo draws
  nothing; the title says it. The list of keys that can follow a prefix no
  longer pops up over the page; Ctrl+G mid-sequence shows it. A new setting,
  *Show the page's mode* (`mode-label`: `ring`, the default, or `chip`),
  brings the old chip back.
- **Welcome, start and settings pages redesigned around motion** that
  uses the board's own movements (docs/DESIGN.md, "Motion on app pages").
  The welcome page is a pinned stage where scrolling moves the board's
  camera through six chapters: a link opening beside its source, stepping
  back, folding a trail, switching spaces and Solo. It is followed by a
  card you click to swap where the keys go, shortcut rows that light up
  when their key is pressed, and icons that draw themselves as they come
  in. On the start page the clock's digits turn over as they change, the
  field docks at the top while scrolling, recent cards lean toward the
  pointer, and menus and dialogs grow out of what opened them. The settings
  side column is a trail with a bead that rides it as you scroll; headings
  dock, a choice's thumb slides, a change says "Saved", and filtering
  moves rows to their new places. Nothing loops, and pages are still when
  the desktop asks for less motion (passed to pages as `still`).

- **Shortcut sheet redesigned** (F1, `?`). The sections are stops on a
  trail down the left: a bead rides it as you scroll, the section in view
  is lit, and a click glides to a section. The heading of the section in
  view docks under the header, rows rise into place when the sheet opens
  or the filter changes, the words a filter matched are marked in the
  accent, and each stop shows how many of its rows matched. The row
  under the pointer is lifted, and long lists of keys stay inside the
  panel.

- **Trails sidebar redesigned** (Ctrl+B). Each trail is drawn as it is on
  the board: the site icons are its stops, with lines running down from
  each page and curving into the pages it opened. The way from the
  trail's first page to the current card is lit in the accent, so the
  sidebar shows how you got here, and the current card's icon is ringed.
  The raised tab glides to the new current card, and rows come in one
  after another as the sidebar opens (all of it still when the desktop
  asks for less motion). The old faint guide lines are gone.

- **Trails turn at right angles.** A trail's line now runs straight out
  of its card's side, straight up or down halfway between the cards and
  straight into the next card, instead of an S-curve. Its turns are
  rounded like the cards' corners and follow the *Corners* setting
  (square corners, square turns). The sidebar's trails and the welcome
  page's drawn board turn the same way.

### Fixed

- **Pages opened at start fill the window the window manager gives.**
  The opening view was fitted to the size Trailboard asked for, so when
  a tiling window manager made the window bigger, the cards stayed
  sized for the smaller one. It is fitted again when the real size
  arrives, unless you've already moved the view.

- **HEVC on laptops with an NVIDIA GPU.** When NVIDIA's driver held the
  first render node, Chromium found no hardware video decoder: HEVC didn't
  play at all and H.264, VP9 and AV1 fell back to the processor.
  Trailboard now points video decoding at the Intel or AMD GPU in that
  case (the start log says so); `--render-node-override=` on the command
  line still wins.

- **Link hints reach controls inside a focusable region.** A page that
  wraps its content in a block with `tabindex` (a repository page's file
  list, branch picker, Code button) got one label in the middle of that
  block and none for the links and buttons inside it. Now only links,
  buttons and summaries keep the targets inside them to themselves; a
  plain container with targets inside it gets no label of its own.

- **Smooth j / k scrolling.** Holding j or k scrolled in lurches: each
  key repeat restarted an eased scroll from a standstill. Scrolls now
  glide by our own frame loop, which chases a target each press moves on,
  so a held key flows at a steady speed and settles when released (the
  biggest one-frame jump went from 125 px to 35 px). A wheel or the page
  scrolling mid-glide takes over at once.

### Added

- **Cards that stay awake.** A card playing media (a video, muted or not,
  or audible sound) is never put to sleep: it sits outside the live-page
  cap, keeps playing off screen and folded in a stack (painting at 4 frames
  a second while unseen), and stays awake 10 minutes after a pause. Each
  card's label has an awake button, one circle, that cycles *Auto* (dim
  ring: the cap decides), *Keep awake* (cyan ring, blue where the theme's
  cyan is near its green: never sleeps, hidden off screen so it costs
  little) and *Always active* (solid green disc: never sleeps, never
  hidden, keeps its focus and full frame rate). Kept cards don't count toward the cap, so the
  other cards keep every slot. The mode is saved with the board (`awake`,
  missing means Auto). A speaker on a card playing media mutes it. By name:
  `:awake [auto|keep|active]`, `:mute`; in Ctrl+K *Keep card awake* and
  *Mute or unmute card*.
- **Page inspector.** F12, Ctrl+Shift+I or `:inspect` opens Chromium's
  inspector (elements, styles, console, sources, network) docked in the
  page's own card, on the right or at the bottom, with a divider to drag;
  or in a card of its own beside the page (`:inspect beside`, or the
  button above the card). Right-click → *Inspect* opens it on the clicked
  element. It runs off screen like a page and talks to the page
  in-process (`web::inspector`), with no debugging port. It takes every
  key while clicked (Passthrough), closes with its page, and is never saved.

- **Board and cards.** Web pages as live cards on an infinite board (CEF
  off-screen rendering, wgpu). Links open beside their source, joined by a
  trail edge; new cards are screen-shaped. Pan, zoom, fit, multi-select,
  group move, move and resize by keyboard, undo/redo. A live-browser cap
  (`TRAILBOARD_MAX_LIVE`) puts far cards to sleep.
- **Board tools.** Solo (Tab: the card fills the window), Tidy, Fold and
  stacks with an outline, trail steps (Alt+PgDn / Alt+PgUp), trail names
  and colours, notes and sticky notes, *Add to note*, Save as Markdown.
- **Spaces.** Several boards, each with its own cards, camera and
  background; private spaces (Ctrl+Shift+P); the board saved atomically
  with autosave and migrations.
- **Chrome.** Ctrl+K palette (cards, history, closed cards, commands, open
  beside with Shift+Enter), a trails sidebar and side panel, find in page,
  page zoom, page menu, desktop clipboard, IME, dropdowns and page cursors,
  site icons, downloads, a start page you can make your own, a welcome page
  on the first start, a shortcut sheet (F1), settings as a page (Ctrl+,)
  with the useful parts of Chromium's own.
- **History and search.** Every page's text in `page-text.db` (FTS5),
  thumbnails, a history view (Ctrl+H), forget a page, clear history.
- **Privacy.** Ad and tracker blocking with uBlock-style lists, cosmetic
  filtering, YouTube ads blocked, cookie banners answered with "reject",
  per-site toggles and allowed lists, a shield showing what was done,
  pages in Chromium's sandbox.
- **Keyboard first** (M12). Vim-like modes (Board, Page, Insert,
  Passthrough, Caret/Visual, Command) with a mode pill; counts; scroll
  sections (`s`, `]s`, `S`); link hints and `;` hint kinds, inside frames
  too; a `:` command line with completion, history, readline keys, `.`,
  `:repeat` and `:later`; cards as tabs (J/K, Alt+digits, Ctrl+^, gt, d/u,
  co, gC, `Ctrl+w` sequences); search keywords (`gh foo`); jumps
  (Ctrl+O / Ctrl+I), marks and saved pages (`b` / `B`); a user keys file
  (`~/.config/trailboard/keys`) with bindings, aliases, chains and `:run`;
  `:set` / `:toggle` / `:site` and `t` toggles; `]]` / `[[`, gu/gU,
  Ctrl+a/x and other address moves; caret and visual selection; page
  dialogs, permissions and a typed file chooser from the keyboard; Ctrl+e
  edits a field in `$EDITOR`; `?` finds backward; `:messages`.
- **Look.** Omarchy theme followed live; cards as sheets without title
  bars, quiet labels, corners (Rounded, Follow Hyprland, Custom), board
  backgrounds, motion, an empty-board welcome, a new app icon.
- **Packaging.** `scripts/bundle.sh`, an Arch PKGBUILD, a desktop entry,
  `--version` / `--help`, crash reports, opening pages in the running
  window, `scripts/install-cef.sh`.
- **Testing.** Scripted end-to-end runs via `TRAILBOARD_SCRIPT`.

### Changed

- **Sidebar.** With no trail on the board, the cards are listed straight
  under the space, without an "Other cards" header; the header comes back
  once a trail exists.
- The app icon no longer shows as a white tile in launchers that draw SVG
  without filters (Qt, e.g. the Omarchy shell): the grain layer's strength
  is its `fill-opacity` now, not the filter.
- **H.264 and AAC video.** `scripts/install-cef.sh --codecs` installs a CEF
  build with the patent-licensed codecs (same CEF version, third-party,
  pinned by SHA-256), and the Arch package uses it by default
  (`TRAILBOARD_CODECS=0` for the standard one). `scripts/bundle.sh` refuses
  to package it unless `ALLOW_CODECS=1`.
- `scripts/install-cef.sh` pins both engines by name and SHA-256 (no build
  index lookup), checks tools, disk space and a running Trailboard first,
  works on disk instead of `/tmp`, swaps the new copy in only once it's
  complete, skips a build that's already installed (`--force`), and writes
  `.cargo/config.toml` when it's missing; `--help` lists the options.
- Ctrl+W alone no longer closes a card; it starts `Ctrl+w` sequences.
- Read Mode is now called Solo; `t` starts toggles and Tidy is `T`; the
  trails sidebar is Ctrl+B and `b` / `B` open saved pages.
- Shortcuts follow the logical key, so they work on any layout.
- Version 0.3.0 (`Cargo.toml`, PKGBUILD).
- Docs: `docs/PLAN.md` and this file shortened; `docs/BUGS.md` and
  `docs/FUTURE.md` folded into the plan (§7 and §8).

### Known issues

- Copying uses the page's plain selected text, so pages that change what is
  copied, and "copy" buttons using `navigator.clipboard`, don't reach the
  desktop clipboard. Desktop text is pasted as plain text.
- Text copied from Trailboard leaves the clipboard when Trailboard quits.
- Pages see `devicePixelRatio` equal to the board's raster scale (0.5–2).
- Custom image cursors (`cursor: url(…)`) show the default arrow.
- A card that wakes from sleep reloads its page, losing scroll position and
  form input.
- H.264, AAC and HEVC video and DRM don't play with the standard CEF (M13).
- Typing is tested through scripted key events, not real keyboard input.

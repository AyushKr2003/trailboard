# Trailboard: full plan

Trailboard is a canvas browser for Linux. Web pages are live cards on an infinite
board. A clicked link opens as a new card beside its source, joined by a trail edge.

It is an independent build modelled on Marble (marble.surf), which ships only for
Apple Silicon macOS. Marble's behaviour and architecture were studied with REA
([re-findings.md](re-findings.md)). No Marble code, art, name, prompts or server
is reused. Where Marble's algorithms were not decompiled (Tidy, placement, Fold),
we design our own.

Status legend: ✅ done · 🟡 partial · ⬜ not started

---

## 1. Goals and non-goals

**Goals**
- Native Linux app (Wayland first, X11 too) with Marble's core experience: a spatial
  board of live pages, links opening beside their source, trails, spaces and notes.
- Fast and memory-aware on ordinary laptops (the dev machine has 7.6 GB RAM, Intel iGPU).
- Local-first: the board, history and search live on disk. No account required.
- Private by default: ads, trackers and cookie banners blocked.

**Non-goals (for now)**
- Recreating Marble's look, branding, server or paid features.
- Browser sync, accounts, payments, telemetry.
- macOS/Windows builds (the stack allows them later).

---

## 2. Architecture

```
app (trailboard)  winit window · input · omnibox · chrome UI · main loop
  ├─ compositor   wgpu renderer · dot grid · cards · shadows · edges · glyphon text
  ├─ web          CEF off-screen browsers · paint buffers · input forwarding · events
  ├─ storage      board JSON (atomic) · later: page-text.db (SQLite FTS5)
  └─ scene        pure model: nodes, edges, camera, placement (no GPU/CEF)
```

Rules
- `scene` and `storage` stay free of GPU and browser types, so they are unit-testable.
- `web` knows nothing about winit; the app translates input (`keymap.rs`).
- CEF runs single-threaded on the main thread (`external_message_pump`), so
  shared state uses `Rc`/`RefCell` and CEF → app events go through `web::drain_events()`.
- Each card owns one windowless Chromium browser. Its BGRA frames are uploaded to a
  `wgpu` texture only when the paint serial changes.

Rendering model (mirrors Marble's observed `cef_backend::osr` + `compositor`)
- Software OSR (`on_paint`) first. Accelerated OSR (dmabuf shared textures) comes later, only if profiling needs it.
- Page raster scale follows the settled zoom (0.5–2.0, quarter steps), so zoomed-in text stays sharp.
  CEF 156 OSR ignores the device scale factor (both `screen_info` and
  `--force-device-scale-factor`), so the view is `css × scale` pixels with page
  zoom `scale` (level `ln s / ln 1.2`): layout stays at the card's CSS size.
- Off-screen cards are hidden (`was_hidden`) and stop painting.

---

## 3. Milestones

### M0: Reverse engineering ✅
- Unpack the DMG (UDIF/LZMA → HFS+), inventory with REA, recover module map and
  1,549 function names, SQL schemas, board JSON structs, menus/shortcuts, Explain API.
- Output: `docs/re-findings.md`, `docs/marble-function-names.txt`.

### M1: Skeleton ✅
- Workspace, winit window, wgpu (Vulkan) surface, dot-grid background.
- Camera: pan (drag/scroll/middle-drag), zoom about cursor (Ctrl+scroll), fit-all.

### M2: Live page cards ✅
- CEF 156 bootstrap (subprocesses re-exec the same binary), off-screen browsers.
- Paint → texture upload; rounded cards with title bar, shadow, focus ring, close ×.
- Mouse, wheel and keyboard forwarded to the focused page; logical-key shortcuts.
- Omnibox (Ctrl+L / Ctrl+T): URL vs search detection (DuckDuckGo).

### M3: Many cards and trails ✅
- ✅ Links open beside their source (`on_before_browse` LINK + gesture, popups, Ctrl/middle-click).
- ✅ Bezier trail edges; placement right of the source, sliding down past siblings.
- ✅ Move, resize, close, reopen closed, double-click title → fill view.
- ✅ Hide off-screen pages; zoom-aware page resolution. Decision (2026-11-14):
  done with page zoom instead of the device scale factor, which CEF 156 OSR
  ignores. `web::Page` converts input (CSS px → view px, wheel deltas) and popup
  rects, so the app still works in CSS px. Trade-offs: `devicePixelRatio` equals
  the raster scale; Chromium keeps zoom per origin (shared by same-origin cards,
  fine while the scale is global) and we re-assert it on every navigation, so a
  page may lay out once at a stale zoom; per-card scales or user page zoom
  (Ctrl +/-) will need the two multiplied.
- ✅ Undo/redo for board edits (move, resize, close, open): `scene::History`,
  Ctrl+Z / Ctrl+Shift+Z / Ctrl+Y. Decisions: history is per session (not saved);
  page navigation, titles and stacking are not board edits; after a click inside
  a page, Ctrl+Z goes to the page (text undo) until the board, a title bar or a
  resize grip is clicked; a card restored by undo keeps its id and trail edges
  whose other end still exists, and Ctrl+Shift+T skips cards already restored.
- ✅ Live-browser cap: `scene::liveness::plan`. Decisions: default 6 live
  (`TRAILBOARD_MAX_LIVE` overrides); rank focused → on screen → most recently used,
  ties keep already-live cards; a sleeping card closes its browser but keeps its
  last frame as a dimmed snapshot (dark placeholder if it never painted); it wakes
  on focus, or when on screen and the camera has settled (not mid-pan). Waking
  reloads the card's current URL, so scroll position and form state are lost.
  On restore all pages start asleep and recency is seeded from z-order.
- ✅ `<select>`/popup widgets and cursor shape from pages. Decisions: the popup
  is composited into the page frame inside `web` (no extra texture): while a popup
  is open we keep the clean page pixels so it can move or close without leftovers.
  Its rect is clamped inside the card (`web::popup`) and pointer events over the
  drawn popup are shifted back to where Chromium placed it. The blit uses the
  scale of the pixels actually painted, not the requested one. Page cursors arrive
  as `WebEvent::CursorChanged` (`web::Cursor`, no winit types) and show while the
  pointer is over that page's content; custom bitmap cursors fall back to default.
- ✅ IME / composition input; clipboard shortcuts verified; context menu.
  Decisions (2026-11-14): off-screen Chromium can't reach the desktop clipboard
  (no Wayland surface or input serial; with `--ozone-platform=x11` a copy even
  empties the Wayland clipboard), so the app bridges it with `arboard`
  (wlr data-control): Ctrl+C/X in a page still go to Chromium, and the page's
  selection (`on_text_selection_changed`) is put on the desktop clipboard. Ctrl+V
  sends desktop text through a synthetic `paste` event plus `insertText`
  (`web::Page::paste_text`), unless the desktop still holds what we last copied
  from a page, in which case Chromium pastes natively (keeps formatting). The
  context menu is ours: CEF's model is cleared in `on_before_context_menu` and
  `web::menu` builds items from the click (link, image, editable, selection,
  else page navigation); the app draws it and runs actions (links open beside).
  IME is enabled on the window while keys go to a page or the address bar;
  compositions are forwarded with `ime_set_composition`/`ime_commit_text` and the
  candidate window follows `on_ime_composition_range_changed`. The address bar
  takes committed IME text only (no preedit shown).
- ✅ Card multi-select and group move: `scene::selection` (`Selection`, `in_band`,
  `drag_group`, `move_group`) and `Edit::Batch` in `scene::history`.
  Decisions (2026-11-14): plain drag on the board still pans, so the band is
  **Shift+drag** on the board and it adds to the selection; it picks cards whose
  frame (title bar included) touches it. Shift+click a title bar toggles a card;
  Ctrl+A with no card focused selects all; a plain click on the board, Esc, or a
  click into a page / resize grip clears it. Selection is separate from focus and
  not saved. Dragging a selected card's title bar moves every selected card;
  dragging an unselected one clears the selection. Delete/Backspace (when keys
  aren't going to a page) closes the selection. A group move or close is one undo
  step (`Edit::Batch`, applied in order, reversed on undo; stale parts skipped),
  and closed cards are restored in reverse so trail edges between them come back.
  Undo returns every change (`Vec<Change>`).

### M4: Persistence and Spaces 🟡
- ✅ Board JSON at `$XDG_DATA_HOME/trailboard/board.json`, atomic write, autosave (3 s), save on exit; unreadable file set aside.
- ✅ Browser profile (cookies, cache) persisted under the data dir.
- ✅ Spaces: several boards, each with its own camera and background
  (New / Next / Previous / Rename / Delete Space). Index file `spaces.json`
  (`storage::spaces`). Decisions (2026-11-14):
  - Files: `spaces.json` (version, active, ordered list of id/name/background)
    and one `Board` per space at `spaces/<id>.json`. On first start the old
    `board.json` is moved to `spaces/1.json`. A missing or unreadable index is
    rebuilt from the board files (an unreadable one is set aside first); a
    newer-version index is left alone and the app runs without saving.
  - Keys: Ctrl+Shift+N new (inserted after the current space), Ctrl+PageDown /
    Ctrl+PageUp next / previous (wrapping, like browser tabs), F2 rename (the
    address bar becomes a name field), Ctrl+Shift+B next tint,
    Ctrl+Shift+W delete. Ctrl+Alt combos were avoided because AltGr reports
    Ctrl+Alt on many layouts and would steal typed characters from pages.
  - Delete asks for Enter when the space has cards (any other key or a click
    keeps it), refuses to delete the last space, and moves the board file to
    `spaces/deleted/` rather than erasing it.
  - Switching saves the board and closes every page of the old space. Undo
    history and Ctrl+Shift+T lists are kept per space for the session (not saved).
  - Page keys carry a per-switch epoch above the card id, so late events and
    textures from the old space's pages can't reach the new space's cards
    (card ids restart in every space).
  - The current space shows as a label in the top-left corner when there is
    more than one; switches, renames and deletes show a short toast.
- ✅ Per-space background colour. Since 2026-11-14 (Omarchy look) a space has a
  *tint*: none or a theme colour name (blue, green, magenta, orange, cyan, red)
  mixed lightly into the theme's background, so it suits light and dark themes.
  New spaces take an unused tint. Wallpaper images are in FUTURE.md.
- 🟡 Schema migrations: `spaces.json` is upgraded step by step on read
  (`storage::spaces::migrate`, v1 fixed sRGB `background` → v2 `tint`; the
  upgrade is written back; unknown/newer versions are refused, not touched).
  Boards are still version 1; give them the same kind of step on their first change.

### M5: Board tools 🟡
- ✅ Read Mode: a card fills the window; Tab toggles board ↔ page
  (`scene::read`: `layout`, `pick`). Decisions (2026-11-14):
  - The page *reflows* to the window (CSS size = window below a title bar,
    rastered at the window scale, 1:1) instead of the camera zooming in, so text
    reads at its natural size. The card's board rect and the camera are not
    touched, so leaving shows the board as it was (gliding only if the card
    isn't on screen). Read Mode is not saved; a restart opens the board.
  - Which card: the focused one, else the topmost under the middle of the view,
    else the nearest to it.
  - Keys: Tab toggles unless keys are going to a page (after a click inside it),
    so Tab still moves between form fields; Esc always leaves (the page sees it
    too) and keeps the card focused; a double-click on the bar leaves. Other keys
    (arrows, Space) reach the page, so it scrolls straight away.
  - Anything that would bring a card on screen while reading (a link opened
    beside, Ctrl+T, Ctrl+Shift+T, undo) reads that card instead; the trail edge
    is still added on the board. Closing the read card, F/Ctrl+0 or a space
    switch go back to the board. Ctrl+scroll does nothing while reading (page
    zoom is M6).
- ✅ Tidy (T): our own tree layout per trail (`scene::tidy`). Decisions (2026-11-14):
  - Whole board, every time (a selection-only Tidy could push cards onto the
    untouched ones; left for later). Card sizes never change.
  - A trail is a tree from its edges (first edge into a card wins; cycles and
    dangling edges are tolerated). Root on the left, one column per generation
    as wide as that generation's widest card, gap 60. A card sits at the top
    of the band its descendants fill, so a first child is level with its
    source, as when it was opened beside it. Siblings and trails keep their
    current top-to-bottom order; trails stack with a 120 gap.
  - Lone cards (no edges) go in rows below the trails, wrapping at the widest
    trail or at ⌈√n⌉ cards, whichever is wider.
  - Starts at the board's old top-left corner; running it twice changes nothing
    ("Already tidy" toast). One undo step (`History::moved_many`).
  - The camera glides by the move of the card in the middle of the view
    (`read::pick`), so that card stays put on screen.
  - Key: T with no card focused (was a second "new card" key; N and Ctrl+T
    remain). Cards jump to their places; animating them is a follow-up.
- Fold (S): collapse a trail into a stack; peek/unfold; stack outline.
- Next/Previous on Trail navigation; trail names and colours.
- Notes (N): native Markdown note cards; sticky notes; "Add to note" quotes the
  selected text with a link back to its source card.
- Save note/board as Markdown.

### M6: Chrome UI 🟡
- Trails sidebar: tree per root, pins/favourites, drag to reorder, collapse.
- Ctrl+K palette: fuzzy search over open cards, history and commands.
- Find in page (Ctrl+F) bar; per-page zoom (Ctrl +/−/0 inside a page).
- ✅ Keyboard-shortcut sheet: `?` with no card focused (`app::hints`). Decisions
  (2026-11-14): only when no card is focused, so `?` still types in pages; Esc,
  `?`, any other key or a click hides it, and that click/key does nothing else.
- ✅ Omarchy look (`app::theme`, `app::desktop`), modelled on the conventions of
  Omarchy's own apps. Decisions (2026-11-14):
  - Palette: the whole of `~/.local/state/omarchy/current/theme/colors.toml`
    (quoted, bare or commented values; old key aliases; `mode`, `light.mode` or
    a light background decide light/dark). Without Omarchy: a dark palette with
    Omarchy's `#FFD60A` accent. Surfaces are steps from background toward
    foreground; focus ring, trail edges, caret, loading bar and section titles
    use the accent; multi-select uses `yellow`; highlighted rows `selection`;
    the delete prompt `red`; text on the accent uses black or white by luminance.
  - Followed live by events, never polled: inotify on the theme files, re-armed
    after every change (a theme switch replaces the directory) and on the
    nearest existing ancestor; changes settle 80 ms before they are read.
  - Corners follow Hyprland's `decoration:rounding` (0, sharp, on Omarchy's
    default), asked of `hyprctl` at start and on every `configreloaded>>` from
    Hyprland's event socket (reconnects with back-off). Cards scale it with the
    zoom; small highlights cap it at 4 px; colour dots stay circles.
  - Overlays (address bar, menu, sheet, labels) scale with the shell's
    `[font] base-size` (`omarchy display text size`) over 12 px; card titles
    keep following the zoom. The shortcut sheet shrinks to fit a small window.
  - Pages follow light/dark through DevTools media emulation
    (`prefers-color-scheme`) per page, because windowless (Alloy) browsers
    ignore the request context's Chrome colour scheme. Form controls only
    darken on pages that declare `color-scheme`, as in any browser.
  - Keys: F1 (also inside pages) shows the sheet; Ctrl+Q quits (the board
    autosaves, so nothing is lost). Wayland app id `trailboard`.
  - Test hooks: `TRAILBOARD_OMARCHY_STATE` and `TRAILBOARD_OMARCHY_SHELL`
    point at a fake theme and shell.toml; a `hyprctl` earlier on `PATH` fakes
    the rounding.
- ⬜ A `.desktop` entry and icon (`share/trailboard.desktop`, `share/trailboard.svg`)
  so Trailboard shows in the Omarchy launcher; translations of the UI strings.
- ⬜ Settings panel; download handling and a downloads list.
- Loading/favicons in titles; hover card names when zoomed out.

### M7: History and search ⬜
- `page-text.db` (SQLite FTS5) using the same proven design Marble uses:
  `pages(url, seen_ms)`, `pages_text` FTS5 (`unicode61 remove_diacritics 2`, prefix 2/3),
  `thumbs(jpeg)`, BM25 with title weight 4, newest-N retention.
- Page text and thumbnails captured after load (injected JS → process message).
- Palette and history view query this index.

### M8: Privacy ⬜
- Brave `adblock` engine with EasyList/EasyPrivacy in a resource request handler.
- Cookie-banner auto-reject (cosmetic rules + known CMP actions).
- Per-site "disable protections" toggle.

### M9: Packaging and release ⬜
- Release build profile, bundle layout (binary + CEF files + locales), `.desktop` file, icon.
- AUR `PKGBUILD` (Arch/Omarchy first), then AppImage/Flatpak.
- Crash logging to a local file; `--version`; first-run defaults.

### M10: Optional extras ⬜
See [FUTURE.md](FUTURE.md): semantic search, Explain with your own LLM key, imports, extensions.

---

## 4. Testing strategy

| Layer | How |
|---|---|
| `scene`, `storage`, omnibox | `cargo test` unit tests (geometry, placement, persistence, URL resolution) |
| End-to-end | `TRAILBOARD_SCRIPT=file` drives the real input handlers (click, wheel, fit, open, wait, log, quit) against real Chromium; logs report card/edge counts |
| Visual | Screenshot the window with `grim -g "$(hyprctl clients -j …)"` |
| Persistence | Two scripted runs with an isolated `XDG_DATA_HOME` |

Known tooling limit: on this Hyprland 0.57 session `ydotool` input does not reach
windows and `wtype` sends remapped keycodes. Use the script runner, or
`hyprctl dispatch 'hl.dsp.cursor.move({x=…, y=…})'` to move the pointer.

Every milestone ends with: tests green, clippy clean, one scripted e2e run, one screenshot check.

---

## 5. Risks

| Risk | Mitigation |
|---|---|
| RAM: one Chromium renderer per card | Live-browser cap, hide off-screen pages, snapshots for sleeping cards |
| CPU: software OSR copies every frame | Upload only on new paint serial; later dmabuf accelerated OSR |
| CEF version drift | Pin `cef` crate and CEF build together; `scripts/install-cef.sh` |
| Wayland quirks (input, scale) | Logical-key shortcuts; test on Hyprland and one other compositor |
| Legal | Clean-room behaviour only; never copy Marble code, assets or prompts |

---

## 6. Handoff (updated at the end of every session)

**Last session:** 2026-11-14. M5 Tidy.
**Repo state:** on `master`, committed after 2a82a9f; working tree clean.
**Done this session:**
- `scene::tidy::tidy(&Scene) -> Vec<(NodeId, Rect)>`: trails as trees
  (columns per generation, band-stacked subtrees, top-aligned), trails stacked
  in reading order, lone cards in wrapped rows below; 6 tests (columns, no
  collisions between subtrees, stacking/wrapping, idempotent, cycles and
  dangling edges, empty board).
- App: T with no card focused → `tidy_board` (one undo step, camera keeps the
  centre card on screen, toast "Tidied N cards" / "Already tidy"). T no longer
  opens a new card (N, Ctrl+T do). Shortcut sheet and README rows.
**Next up:** M5 Fold (S): collapse a trail into a stack, peek/unfold, stack
outline. Decide how a folded trail is stored (a `folded` flag on the root in
the board file means the first board migration: v1 → v2, see M4 notes).
Small follow-ups: animate cards to their Tidy places; `.desktop` entry and icon (M6).
**Notes for the next session:**
- Tidy verified e2e (`~/.cache/trailboard-e2e-tidy`, `run.sh` seeds a board of
  five 640×420 coloured pages, trail 1→2, 1→3, 3→4 plus lone 5, scattered and
  overlapping): T moved 5 cards to the expected rects, card 2 (under the view
  centre) kept its screen position, a second T said "Already tidy", undo and
  redo restored / re-applied all five in one step. Screenshots before/after
  checked. `run.sh` waits for `script log: shot …` markers before `grim`.
- Not verified: real keyboard T (script keys only), big boards (layout is
  O(n log n), but each card is a renderer, so not tried beyond 5).
- Read Mode notes from last session still hold: a stale full-window frame can
  show stretched for a moment after leaving Read Mode; the read card never sleeps.
- A sleeping card's last frame is dropped on switch, so after switching back
  cards beyond the live cap show the plain "asleep" placeholder until woken.
- Keep the sheet's rows in `hints::SECTIONS` in step with new shortcuts.
- Live-browser cap: zooming can churn (sleep, then reload in view).
- Keep test boards small (RAM). Use a fresh `XDG_DATA_HOME` under `~/.cache` for
  each e2e run (a saved board plus the URL argument gives two cards; seeding a
  board file and running without a URL is easiest for layout tests).
- `hyprctl clients -j` can report a window off-screen right after it opens
  (open animation); take the screenshot ~5–8 s later.
- Script `open` with a data: URL: percent-encode spaces and `#` (`#` starts a
  script comment); a `file://` test page is easier. While `TRAILBOARD_SCRIPT`
  runs, the window's IME stays off.

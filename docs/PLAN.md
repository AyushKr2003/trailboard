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
- Page device scale follows the settled zoom (0.5–2.0, quantized), so zoomed-in text stays sharp.
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

### M3: Many cards and trails 🟡
- ✅ Links open beside their source (`on_before_browse` LINK + gesture, popups, Ctrl/middle-click).
- ✅ Bezier trail edges; placement right of the source, sliding down past siblings.
- ✅ Move, resize, close, reopen closed, double-click title → fill view.
- ✅ Hide off-screen pages; zoom-aware page resolution.
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
- ⬜ `<select>`/popup widgets (`PET_POPUP` painting) and cursor shape from pages.
- ⬜ IME / composition input; clipboard shortcuts verified; context menu.
- ⬜ Card multi-select (rubber band) and group move.

### M4: Persistence and Spaces 🟡
- ✅ Board JSON at `$XDG_DATA_HOME/trailboard/board.json`, atomic write, autosave (3 s), save on exit; unreadable file set aside.
- ✅ Browser profile (cookies, cache) persisted under the data dir.
- ⬜ Spaces: several boards, each with its own camera and background
  (New / Next / Previous / Rename / Delete Space). Index file `spaces.json`.
- ⬜ Per-space wallpaper/background colour.
- ⬜ Schema migrations (`version` field already present).

### M5: Board tools ⬜
- Read Mode: a card fills the window; Tab toggles board ↔ page.
- Tidy (T): our own tree layout per trail (roots left, children in columns, no overlaps).
- Fold (S): collapse a trail into a stack; peek/unfold; stack outline.
- Next/Previous on Trail navigation; trail names and colours.
- Notes (N): native Markdown note cards; sticky notes; "Add to note" quotes the
  selected text with a link back to its source card.
- Save note/board as Markdown.

### M6: Chrome UI ⬜
- Trails sidebar: tree per root, pins/favourites, drag to reorder, collapse.
- Ctrl+K palette: fuzzy search over open cards, history and commands.
- Find in page (Ctrl+F) bar; per-page zoom (Ctrl +/−/0 inside a page).
- Settings panel; keyboard-shortcut sheet; download handling and a downloads list.
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

**Last session:** 2026-11-14. Added the live-browser cap (M3).
**Repo state:** on `master`, live-browser cap committed after a790233; working tree clean.
**Done this session:**
- `scene::liveness` (`Candidate`, `Plan`, `plan`): pure ranking of which cards
  stay live, with unit tests (cap never exceeded, focused always wakes, stable ties).
- App: `cards` now holds only live browsers; `used` tracks recency (`touch` on
  create/focus). `sync_liveness` runs every tick from `sync_pages`; it sleeps over
  the cap at any time but wakes on-screen cards only once the camera settles.
- Sleeping cards draw their last texture dimmed with "· asleep" after the title;
  never-painted sleepers draw a dark placeholder with "Click to load".
- Restore no longer starts every page; z-order seeds recency.
**Next up:** M3 remaining items in order: `<select>`/popup painting →
IME/clipboard/context menu → multi-select (group move can record one `Edit` per
card; add a batch edit then).
**Notes for the next session:**
- Verified e2e with `TRAILBOARD_MAX_LIVE=2` and 4 cards: oldest cards sleep on
  open, restore wakes the two most recent, clicking a sleeping card wakes it and
  sleeps another, zooming swaps which on-screen cards are live. Screenshots showed
  the dark placeholder and a sleeping card's kept snapshot.
- Zooming in/out can churn (a card sleeps, then reloads when it is back on screen).
  If that feels bad in daily use, add a grace period before sleeping, or keep a
  snapshot-only wake.
- The "Click to load" label and "· asleep" title suffix were not seen in a
  screenshot (text hides below 6 px at the zoom used); only the logic is exercised.
- Real Ctrl+Z key routing (page vs board) was not tested with a physical keyboard.
- Typing into pages has not been verified by hand; ask the user to try it if relevant.
- Keep test boards small (RAM). Use an isolated `XDG_DATA_HOME` under `~/.cache` for e2e runs.
- `hyprctl clients -j` can report a window off-screen right after it opens
  (open animation); take the screenshot ~8 s later. A board's saved camera
  carries over between e2e runs that share a data dir.

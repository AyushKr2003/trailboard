# Trailboard: full plan

Trailboard is a canvas browser for Linux. Web pages are live cards on an infinite
board. A clicked link opens as a new card beside its source, joined by a trail edge.

It is an independent build modelled on Marble (marble.surf), which ships only for
Apple Silicon macOS. Marble's behaviour and architecture were studied with REA
([re-findings.md](re-findings.md)). No Marble code, art, name, prompts or server
is reused. Where Marble's algorithms were not decompiled (Tidy, placement, Fold),
we design our own.

Status legend: ✅ done · 🟡 partial · ⬜ not started · ⏸ on hold for the owner's decision

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
- Paint → texture upload; rounded cards with shadow and focus ring.
  (2026-11-16 redesign, owner's decision: no title bar and no dock. A card is its
  page; its name, buttons and handle are a screen-sized label above it. See
  [DESIGN.md](DESIGN.md).)
  (2026-11-16: every text is one line, cut with "…" at its clip, so long titles never wrap.)
- Mouse, wheel and keyboard forwarded to the focused page; logical-key shortcuts.
- Omnibox (Ctrl+L / Ctrl+T): URL vs search detection (DuckDuckGo; the engine is a setting since M6).

### M3: Many cards and trails ✅
- ✅ Links open beside their source (`on_before_browse` LINK + gesture, popups, Ctrl/middle-click).
- ✅ Bezier trail edges; placement right of the source, sliding down past siblings.
- ✅ New cards are screen-shaped (owner's decision, 2026-11-16, later the same
  day): shaped like a full window on an empty workspace (Hyprland), else the
  monitor, else 16:9, at width `CARD_W`; `CARD_H` is now 576 (16:9).
- ✅ One size for new cards (owner's decision, 2026-11-16): every new card,
  link, start page, note and our own page (welcome, settings, engines) opens
  at `scene::CARD_W`×`CARD_H` (1024×720); `placement::beside` no longer copies
  the source's size. Sticky notes are the exception (280×240).
- ✅ Move, resize, close, reopen closed, double-click title → fill view.
- ✅ Hide off-screen pages; zoom-aware page resolution. Decision (2026-11-14):
  done with page zoom instead of the device scale factor, which CEF 156 OSR
  ignores. `web::Page` converts input (CSS px → view px, wheel deltas) and popup
  rects, so the app still works in CSS px. Trade-offs: `devicePixelRatio` equals
  the raster scale; Chromium keeps zoom per origin (shared by same-origin cards,
  fine while the scale is global) and we re-assert it on every navigation, so a
  page may lay out once at a stale zoom. User page zoom (Ctrl +/−, M6) is
  multiplied in: Chromium gets `scale × zoom`, the view stays `size × scale`.
- ✅ Zoom on card start, board zoom and Read Mode (2026-11-16). Chromium
  saved our raster zooms per site (stale scales, `data:` pages keyed by their
  text), so pages first laid out at an old scale: `web::prefs::forget_saved_zooms`
  drops them at launch, with `partition.default_zoom_level`, which stays 100%
  (Chromium reads it only at startup; moving it at run time made a page
  asked for the new default fall back to the startup one, so Read Mode
  showed pages at 75%); `page_scale` is settled
  before pages are made or woken, and leaving Read Mode goes straight to the
  board's scale. Still open: Chromium shares zoom per site and applies zoom
  and size as two updates, so same-site cards can lay out once at a
  passing width when the scale changes (seen only by page scripts, not
  painted, in e2e). Per-tab zoom exists only in the extensions API.
- ✅ Cards lay out at Read Mode's width (2026-11-16): `read::card_css` gives
  a card's page the window's CSS width and the card's shape at that width, so
  the card is a scaled copy of the full window, Read Mode never reflows the
  page, and a resize scales the page instead of re-laying it out. Notes keep
  their card size. The raster scale stays one for all cards (Chromium shares
  page zoom per site), corrected for default-width cards
  (`board_page_scale`); a resized card is drawn from a texture a little over-
  or under-sampled. At fractional raster scales the width can be 1 px off
  (view px are rounded).
- ✅ Undo/redo for board edits (move, resize, close, open): `scene::History`,
  Ctrl+Z / Ctrl+Shift+Z / Ctrl+Y. Decisions: history is per session (not saved);
  page navigation, titles and stacking are not board edits; after a click inside
  a page, Ctrl+Z goes to the page (text undo) until the board, a title bar or a
  resize grip is clicked; a card restored by undo keeps its id and trail edges
  whose other end still exists, and Ctrl+Shift+T skips cards already restored.
- ✅ Live-browser cap: `scene::liveness::plan`. Decisions: default 6 live
  (the *Pages kept awake* setting, M6; `TRAILBOARD_MAX_LIVE` overrides it for a run); rank focused → on screen → most recently used,
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

### M4: Persistence and Spaces ✅
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
- ✅ Schema migrations: `spaces.json` is upgraded step by step on read
  (`storage::spaces::migrate`, v1 fixed sRGB `background` → v2 `tint` → v3
  `private`; the
  upgrade is written back; unknown/newer versions are refused, not touched).
  Boards the same way (`storage::migrate_board`): v1 → v2 adds `scene.stacks`
  (Fold), v2 → v3 adds `node.note` (Notes), v3 → v4 trail marks, v4 → v5
  sticky notes, v5 → v6 sidebar pins and order; written back on the next save.

### M5: Board tools ✅
- ✅ Read Mode: a card fills the window; Tab toggles board ↔ page
  (`scene::read`: `layout`, `pick`). Decisions (2026-11-14):
  - The page *reflows* to the window (CSS size = the whole window,
    rastered at the window scale, 1:1) instead of the camera zooming in, so text
    reads at its natural size. The card's board rect and the camera are not
    touched, so leaving shows the board as it was (gliding only if the card
    isn't on screen). Read Mode is not saved; a restart opens the board.
  - Which card: the focused one, else the topmost under the middle of the view,
    else the nearest to it.
  - Keys: Tab toggles unless keys are going to a page (after a click inside it),
    so Tab still moves between form fields; Esc always leaves (the page sees it
    too) and keeps the card focused. (2026-11-16: the title bar is gone, so
    the page fills the whole window; the bar's double-click went with it.) Other keys
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
- ✅ Fold (S): a trail collapses into a stack (`scene::fold`). Decisions (2026-11-14):
  - A trail is every card joined to the picked one by edges, either way; its
    members are kept in reading order (roots, then links depth first). The
    picked card (as for Tidy: the topmost under the middle of the view, else
    the nearest) is the front, so the stack sits where you were looking.
  - Stored in the board as `scene.stacks` (board v2): members with their
    offsets from the first member, the front, and the stack's place (`anchor`,
    the offset of the card that was in front when folded). All members share
    the front's position; the front's position *is* the stack's, so dragging,
    Tidy and multi-select move it like any card.
  - Folded-away cards are not drawn, hit, laid out, selected or kept live
    (`Scene::nodes`, `topmost_at`, `bounds`, `draw_order`, `visible_edges`
    skip them; `all_nodes` doesn't); their pages sleep at once.
  - Keys (no card focused, like T): S folds the trail in the middle of the
    view, or unfolds it if it's a stack; `[` / `]` peek at the previous / next
    member (wrapping), which takes the front's place. Unfold lays the trail out
    again around the stack's place, exactly as before if it never moved.
  - Folding a trail that already holds a stack (a link opened from a stack's
    front) unfolds that stack first: one trail, one stack.
  - Fold and unfold are one undo step each (`Edit::Unfold`, `Edit::Refold`
    with the whole saved stack, so undoing an unfold brings back the same front
    in the same place). Peeking is not an edit.
  - Closing the front brings the next member forward in its place; a stack left
    with one card dissolves.
  - Look: up to two card edges behind the front, offset down-right by 10 world
    units (never under 5 px); the title reads `3/4 · Title`; a badge with the
    count on the top-left corner stays readable at any zoom.
  - ✅ Outline (2026-11-14): a click on the count badge, or O (no card
    focused, the stack in the middle of the view), lists the members under
    the badge in reading order (`n  Title`, URL if untitled, cut at 60
    characters), the front one bold with an accent bar and highlighted, then
    *Unfold trail*. Picking a member brings it to the front in place
    (`Scene::show`, like peeking: not an edit, focus unchanged, so `[`/`]`
    keep working); arrows, Enter, Esc and click-outside behave as in the page
    menu, which it shares (`app::menu::Entry`). This is also the mouse way to
    peek; there is no separate mouse peek. Badges are hit before cards
    (they stick out past the corner) unless a card above covers them.
- ✅ Next/Previous on Trail (Alt+PgDn / Alt+PgUp; `scene::trail`). Decisions (2026-11-14):
  - Steps through the trail in reading order (`Scene::trail_of`: roots, then
    links depth first), the same order as a stack's outline, so stepping
    through a folded trail and its outline agree.
  - Starts from the focused card, else the one in the middle of the view
    (`read::pick`). The card reached is focused (keys stay with the board
    until the page is clicked) and brought on screen with the usual glide; in
    Read Mode it is read instead.
  - A card folded away is shown at its stack's front in place, like peeking
    (`Scene::show`); not an edit.
  - Stops at either end with a "Start/End of the trail" toast (no wrap; stacks
    wrap when peeked). A lone card toasts "Not on a trail". Each step toasts
    `n/len · Title`.
  - Keys: Alt+PgDn / Alt+PgUp, which work everywhere (inside a page and in
    Read Mode too), on any layout, and sit beside Ctrl+PgDn/PgUp for spaces.
    Chromium binds nothing to them.
- ✅ Trail names and colours (R / C; `scene::mark`). Decisions (2026-11-14):
  - A trail has no object of its own, so its mark (`Node::trail`: name and
    colour) is kept on every card of it, board v4. Setting it writes all of
    them; a card opened beside one (page or note) takes its source's mark.
    Closing any card, even the first, keeps the name; a trail split by closing
    a card in the middle leaves both halves named. If cards disagree (a card
    put back by undo after a rename), the first card in reading order with a
    name gives the name, likewise for the colour.
  - Names are one line, trimmed, at most 60 characters; empty clears the name.
    Colours are theme colour names (none = accent, then blue, green, magenta,
    orange, cyan, red, the space tints' order), so they follow light and dark
    themes.
  - Keys (no card focused, the trail in the middle of the view, like S/T/O): R
    opens the address bar as a name box holding the current name; C steps the
    colour. Each change is one undo step (`Edit::SetMarks`). A lone card can
    be marked too, and links opened from it join its trail with the mark.
  - Look: a tag in the trail colour (text black or white by luminance) above
    the first card of the trail that is on the board (a folded trail's stack
    front), clear of the stack badge, screen-sized like the badge; a colour
    without a name is a dot. Edges take the trail colour. The step toast reads
    `Name · 2/4 · Title`.
  - Renaming from elsewhere (2026-11-15): R in the sidebar names the
    highlighted row's trail (a header's, or a card row's; a card on no
    trail is named alone and becomes a trail, as R does on the board;
    "Other cards" toasts). A double-click on a trail's header names it
    too: the second click takes back the first click's collapse or
    expand, so the group ends as it was (it flickers once; acting on
    release only after 400 ms would make every click feel slow). A click
    on the tag above a trail's first card opens the name box (the tag had
    no other action, so no double-click); a colour-only dot works too.
    Tags are hit like the stack badge, under any card drawn above them.
    The name box, address bar and find bar sit centred right of an open
    sidebar (`palette::field_rect_from`); the lists stay centred sheets.
  - Not done: renaming from the page menu, or a right-click menu on a
    sidebar header (menus are page menus today).
- ✅ Notes (N): Markdown note cards (`scene::note`, `app::note`). Decisions (2026-11-14):
  - A note is a card (`Node::note: Some(text)`, `url` = `trailboard:note`), so
    it moves, resizes, folds, tidies, sleeps, closes and undoes like any card.
    Board v3 (v2 → v3 adds nothing to old files; v2 builds refuse v3 boards so
    they can't drop note text on save). The title is the first line with text,
    Markdown markers stripped, cut at 80 characters; "Note" when empty.
  - The editor is our own page (`crates/app/src/note.html`) in the card's
    browser, loaded as a `data:` URL with the text and theme colours filled in
    (`PageKind::Note`). This gives IME, selection, clipboard, undo and Read
    Mode for free; Marble also hosts its notes in CEF. Cost: a note is a
    renderer like any card (it sleeps under the live cap like one).
  - Source and rendered views instead of WYSIWYG: a monospace textarea while
    editing; on Esc or blur our own small Markdown renderer (headings,
    emphasis, code, links, autolinks, lists, task lists, quotes, rules, fenced
    code; HTML is escaped; only http(s)/mailto/file links). A click on the view
    edits at that line; a task box toggles without editing; Enter continues a
    list (and ends it on an empty item); Tab inserts two spaces.
  - Page → app: the editor sends its whole text on every input as a console
    message with a private prefix (`web::NOTE_MESSAGE`), accepted only from
    note pages; a 64 KB note round-trips. Clicked links go the same way
    (`NOTE_OPEN`), because a `data:` page may not open `file:` URLs, and open
    beside the note. A note page never navigates (all other navigations are
    cancelled) and reports no title or URL. App → page: `tbTheme` on theme
    changes, `tbSetText` when a quote is added.
  - Keys: N with no page focused (was "new card"; Ctrl+T remains). A new note
    is focused with keys going to it. Ctrl+L on a note opens the bar for a new
    card; Ctrl+R does nothing on a note (it would reload the text it opened
    with); the note menu has no Back/Forward/Reload.
  - Text edits are not board undo steps (the textarea has its own undo).
- ✅ Add to note: the page menu on selected text offers *Add to note*, which
  appends `> quote` + `— [Title](<url>)` to the *note target* (the note last
  focused, edited or made in this space), or makes a new note beside the page
  with a trail edge when there is none. Toast "Added to Title". While reading,
  a new note is not read next.
- ✅ Sticky notes (small coloured notes without a title bar). Decisions (2026-11-14):
  - A sticky note is a note card with `sticky: Some(colour)` (board v5; v4 →
    v5 adds nothing to old files, and v4 builds refuse v5 boards so they
    can't drop the colour on save). Same editor, text, title, export, undo,
    sleep and trails as any note; only its look and chrome differ.
  - No title bar: a 16-unit strip (`STICKY_GRIP_H`) in a deeper shade of its
    colour is its only chrome, drag it to move; no title text, no ×, no
    loading bar. Close with Delete or Ctrl+W, resize by the corner or M.
    `Node::header_h()` gives the strip or title bar height, so `frame()`,
    hit tests and Tidy fit both. 12 units was ~4 px when fitted; 16 is still
    small far out, where M moves it.
  - Paper is the theme colour itself (8% toward the background), with black
    or white ink by contrast (≥ 4.5:1 tested for every colour in dark, Tokyo
    Night and a light palette); links and task boxes use the ink, not the
    accent. Colours are theme names (`STICKY_COLOURS`: yellow, green, cyan,
    blue, magenta, red), so they follow theme changes.
  - Keys: Shift+N makes one in the middle of the view (280×240, yellow, keys
    in it), on no trail. C recolours the card it picks if that is a sticky
    note (the current card, else the one in the middle), else the trail as
    before. A note's page menu toggles *Make sticky note* / *Make note card*.
    Recolouring and toggling are undoable (`Edit::SetSticky`).
- ✅ Save as Markdown (Ctrl+S, note menu; `scene::export`, `storage::export`). Decisions (2026-11-14):
  - Ctrl+S saves the focused note, else the whole board of the space (also
    with a web page focused, where Chromium has nothing on Ctrl+S). A note's
    page menu has *Save as Markdown* (always, below its other items).
  - No file dialog (no portal plumbing yet): files go to
    `Documents/Trailboard/`, Documents from `$XDG_DOCUMENTS_DIR`, else
    `user-dirs.dirs`, else `~/Documents`. Named after the note's title or the
    space's name (path separators and control characters made spaces, cut at
    80 characters); a taken name gets ` 2`, ` 3`, …, so nothing is ever
    overwritten except the file the same note/space was saved to earlier in
    this session (if its name still fits). A toast names the file (`~/…`).
  - A note saves as written. A board saves as `# Space`, then per trail (in
    the order its first card was made) `## Trail name` (or its first card's
    title) and a nested list in reading order (`Scene::trail_tree`), pages as
    `[Title](<url>)`, notes as `Note: Title` with their text indented under
    the item. Folded cards are listed in place. A note on no trail gets its own
    section (its first heading, or `## Title`); pages on no trail go under
    `## Other pages`. Headings in notes move down a level outside code.
  - Not done: a file chooser (xdg-desktop-portal), saving a web page's text.

### M6: Chrome UI ✅
- ✅ Trails sidebar (B; `scene::sidebar`, `app::sidebar`). Decisions (2026-11-14):
  - A panel over the left edge of the board (300 px, at most 45% of the
    window, full height), not a resized board: the board, Read Mode and Fit
    stay as they are. Hidden in Read Mode. The space pill moves right of it.
  - Rows: one group per trail (two or more cards, or a name, as in Save as
    Markdown), in the order its first card was made: a header (▾/▸, a dot in
    the trail's colour, its name or else its first card's title, the number
    of cards), then its cards as a tree in reading order (`trail_tree`),
    indented by depth. Cards on no trail last, under *Other cards*. Folded
    cards stay listed in their place, dimmed. A sticky note shows its colour.
  - B opens it with the keys and the current card's row highlighted (selection
    colour); B or Esc closes it. A click elsewhere takes the keys back and
    the panel stays open (the current card keeps an accent bar on its row);
    B then gives the keys back. Keys while it has them: j/k or arrows,
    PgUp/PgDn, Home/End move the highlight, and landing on a card goes to it
    (current, shown at its stack's front if folded, brought on screen clear
    of the panel; like peeking, not an edit). h/l or ←/→ collapse / expand a
    group, Enter or Space on a card gives its page the keys (panel stays),
    on a header toggles it. Other keys (S, N, T, Delete…) stay board keys.
  - Mouse: a click on a card row goes to it, on a header toggles the group;
    the wheel scrolls the list; the pointer over the panel never reaches pages.
  - Collapsed groups are keyed by the trail's first card and live for the
    session only (reset on a space switch); not in the board file.
  - Pins and order (2026-11-14, board v6): `p` pins the highlighted row's
    trail (a card on no trail becomes a group of its own) or unpins it;
    pinned trails come first, marked ★, with a line under them. Shift+J/K
    (or Shift+↑/↓) move the highlighted trail one place, and dragging a
    trail's header onto another trail's rows moves it there (an accent line
    shows where); a click without a drag still collapses it, now on release.
    A trail moves only among the pinned or among the rest; *Other cards*
    stays last. Pin and place are part of the trail's mark (`TrailMark`
    `pinned`, `place`), so like a name they live on every card of the trail,
    survive a split, pass to cards opened beside, are undone with Ctrl+Z and
    saved. Moving numbers the whole part's places afresh; trails never moved
    follow the moved ones, oldest first; a newly pinned or unpinned trail
    goes last in its part. Shift+H/L still pan while the sidebar has keys.
  - Not done: rename from the sidebar, a minimum pixel step for touchpad
    scrolling (small pixel deltas round to no scroll), dragging a card row
    (to move a card between trails would change edges, not just order),
    auto-scroll while dragging past the list's ends.
- ✅ Ctrl+K palette (`scene::palette`, `app::palette`). Decisions (2026-11-14):
  - Ctrl+K anywhere, inside pages too (like Ctrl+L; a page's own Ctrl+K
    palette loses out). It is the address bar's field (`Target::Palette`, so
    IME, Ctrl+A/C/X/V and clicks work the same) with a result list under it.
    Ctrl+K again or Esc closes; a click outside closes and goes through.
  - Results, ranked together: every card (folded ones too: going there shows
    it at its stack's front), the other spaces, and the board commands (each
    with its keys on the right). A last row opens the text in a new card,
    as a web search, or first if it looks like an address. History (M7) comes
    just before that row.
  - Empty query: cards by last use this session, then stacking order; then
    spaces and commands in a fixed order.
  - Matching (own design): query letters in order, case and spaces ignored;
    DP for the best placement: +1 per letter, +5 at a word start (+3 more at
    the text's start), +4 for following the previous match, −3 per gap, a
    small cost for a late start; texts cut at 200 chars. Ties keep the
    caller's order (cards first). Each item also has a hidden second text (a
    page's URL, a note's body, other words for a command: "exit" finds Quit),
    ranked 8 lower and only when its letters lie nearly together (span ≤ 2×
    the query), so scattered letters in long URLs don't match.
  - Running: a card is gone to like from the sidebar (current, on screen),
    leaving Read Mode first; commands call the same code as their keys and act
    on the current card. The page keeps the keys if it had them.
  - Mouse: hover highlights, a click runs the row, the wheel scrolls the list
    (up to 9 rows, fewer in a short window, with a scroll bar).
  - ✅ Open beside (2026-11-15): Shift+Enter or Shift+click on a row that
    opens a page (a history page not on the board, the last "Open …/search"
    row; in Ctrl+K and Ctrl+H) opens it beside the current card
    (`App::current`: the focused card, else the one mid-view) via
    `Scene::spawn_beside`, with a trail edge, focused with the keys in its
    page. Other rows ignore Shift (a card row still goes to the card, a
    closed card comes back where it was). Shift+Enter in the address bar
    (Ctrl+L, Ctrl+T) does the same instead of loading in place / in the
    middle. No card on the board: in the middle, like Enter. Not shown as a
    hint on the rows (the guide note and README list it).
- ✅ Find in page (Ctrl+F; `omnibox::Target::Find`, `web::Page::find`).
  Decisions (2026-11-14):
  - The bar is the app's one text field at the top (like the palette), not a
    bar on the card: it searches the current card's page (with or without the
    keys). No current live page: a toast.
  - Search as you type, case-insensitive (Chromium's find). Enter / Shift+Enter
    step to the next / previous match; the right end shows "N matches", then
    "3 of N" once a match is active (Chromium picks one only on Enter), or
    "No matches". Ctrl+F in the bar selects its text.
  - It opens with the last search (selected, and run again). Esc, a click
    elsewhere or another box closes it and stops the search (`sync_find`),
    leaving the active match selected; it also closes when its card stops
    being current or goes to sleep.
- ✅ Page zoom (Ctrl +/−/0; `scene::pagezoom`, `storage::zoom`,
  `web::set_page_zoom`). Decisions (2026-11-14):
  - Per site, not per card, like desktop browsers: Chromium keeps zoom per
    host, so per-card zooms of one site would fight. Site = host with port,
    lowercased, `www.` dropped; `file:`/`data:`/`about:` pages share one per
    scheme. Notes are never zoomed.
  - Steps 25%–500% as in desktop browsers. Saved in `$DATA/zoom.json`
    (version 1, shared by all spaces; 100% is not stored). A toast shows
    "site · 110%".
  - Keys act on the current card whether or not its page has the keys. With
    no page current, Ctrl +/− zoom the board and Ctrl+0 fits everything;
    with one, Ctrl+0 is 100% (Shift+F stays Fit).
  - Not done: a zoom badge on zoomed cards' title bars; Ctrl+scroll inside a
    page (it zooms the board); zooming notes.
- ✅ Keyboard-driven board (`scene::nav`, `App::typing`). Decisions (2026-11-14),
  following Omarchy's app conventions (the `omarchy-app` skill: keyboard
  first, `?` sheet, Q quits, Space the main action, bare keys never taken
  from a text field):
  - Two modes instead of "focused = keys to the page": a card can be
    *current* (thin, softer accent ring; board keys) or *have the keys* (heavy solid ring,
    every key but app shortcuts goes to the page). Clicking in a page, Enter
    or Space on the current card, a new note and a card opened from the
    address bar give the keys; a title-bar click, Alt+PgDn and h/j/k/l make a
    card current only. Read Mode always gives the read card the keys.
  - Esc takes the keys back (the card stays current, its page is blurred so
    no caret blinks); Esc on a current card lets go of it.
  - h/j/k/l and arrows: nearest card whose centre lies that way; cost = gap
    ahead + 3 × sideways gap between the cards' ranges (0 when side by side),
    so a card in line wins over a nearer one off to the side. Folded cards are
    skipped. With no current card the first press picks the middle one.
  - Shift+h/j/k/l pans a quarter window; + / - zoom ×1.25 about the middle
    (Ctrl +/- stay free for per-page zoom). Delete closes the current card
    (one undo step), Q quits (the board is saved continuously, so no question).
  - Board keys (f, N, T, S, O, R, C, `[`, `]`, `?`, Ctrl+A, Ctrl+Z/Y) work
    whenever no page has the keys and act on the current card first.
  - Enter on a note opens its editor at the end (`tbEdit`).
  - Not done: the Menu key / Shift+F10 for the page menu, keyboard
    selection of several cards.
- ✅ Keyboard move and resize (M; `scene::arrange`, `App::arrange`). Decisions (2026-11-14):
  - A mode rather than chords: Alt+arrows are back/forward, Ctrl+L the
    address bar, and Super belongs to Hyprland, so M (no page has the keys)
    starts arranging the current card, else the one in the middle of the view.
  - h/j/k/l or arrows move by 40 logical px on screen (so the same feel at
    any zoom); Ctrl takes 8 px steps. Shift resizes, top-left fixed like the
    grip: l / j grow, h / k shrink, never below the 240 minimum.
  - If the card is selected, the whole selection moves with it; resize is
    only ever the one card. A folded stack moves with its front, as with a drag.
  - Enter, Space or M keep the result as one undo step; Esc puts every card
    back. Any other key, a click, a focus change or a space switch keeps it,
    and the key then does what it always does (so Ctrl+Z undoes the whole move).
  - The card gets the heavy ring while arranged, the camera follows it, and a
    pill at the bottom lists the keys for as long as the mode lasts.
- ✅ Everything by keyboard (2026-11-14): every mouse action has a key, as
  Omarchy's apps do. Audit of the mouse paths (`App::mouse_input`, `hit`):
  pan (Shift+h/j/k/l), zoom (+/-), move/resize (M), close (Delete), badge
  (O), sidebar and palette rows, Read Mode bar (Tab/Esc), sticky strip (M)
  were covered. Added:
  - v selects / deselects the current card (Shift+click a title). h/j/k/l no
    longer clear the selection, so v marks several cards as you go, like a
    file manager; Esc clears it, Delete and M act on it. (Band select stays
    mouse only; v + Ctrl+A cover it.)
  - z: the current card fills the view (double-click its title bar).
  - Menu key / Shift+F10: the page menu (right-click). The key is sent to the
    page, so Chromium places it at the selection or focused field and fills in
    the context (link, selection, field), then asks for our menu as a click
    does; a menu opened that way starts with its first item highlighted.
    Menus take j/k besides the arrows.
  - f labels controls too (buttons, inputs, selects, text areas, summary,
    editable content, `[onclick]`, focusable and ARIA-role elements; not
    disabled or `aria-hidden`; a control inside a labelled link is the
    link's). Links with an openable URL open beside as before; other targets
    get a synthesized left click at their middle; fields (text-like inputs,
    text areas, selects, editable, textbox/combobox roles) also get the keys.
    This also ticks a rendered note's task boxes by keyboard.
  - d / u scroll the current page half its height (a wheel event at its
    middle) without giving it the keys.
  - Still mouse only: selecting text in a page's body (fields take
    Shift+arrows; Chromium's caret browsing isn't wired), drawing a band,
    hover cursors.
- ✅ Link hints (`f`; `scene::hint`, `web::links`, `App::link_hints`). Decisions (2026-11-14):
  - f (no page has the keys) acts on the current card, else the one in the
    middle of the view, which becomes current. Fit moves to Shift+F (Ctrl+0
    stays); with no card, f says there is no page.
  - Our script (`web::links::SCRIPT`) runs in the main frame and answers with
    one console message (`web::links::PREFIX`): the layout width, then the
    visible part of each `a[href]` / `area[href]` with an http(s) or file URL
    that is on screen and not covered at its middle (`elementFromPoint`), in
    document order, at most 500. The width scales the rects to the view's CSS
    px if the page is zoomed. Any page could send that message, so the app only
    takes it from the card it asked; following only opens a URL beside, which a
    page could already do with `window.open`. Iframes are not searched.
  - The app draws the labels (accent pills at each link's start, kept inside
    the page), so they follow the theme and stay readable at any board zoom.
    Labels are prefix-free from the home row `asdfghjkl`, one letter shorter
    for the first links (`scene::hint::labels`); a label follows as soon as it
    is typed in full, no Enter. Typed letters dim; labels that no longer fit
    hide; a letter no label continues is ignored; Backspace takes one back.
  - Following opens the link beside its card with a trail edge (as a click
    would) and makes the new card current, so f again walks on. Esc, any other
    key, a click, a scroll, a navigation of the page, Read Mode, a space
    switch or another current card cancels.
- ✅ Keyboard-shortcut sheet: `?` with no card focused (`app::hints`). Decisions
  (2026-11-14): only when no card is focused, so `?` still types in pages; Esc,
  `?`, any other key or a click hides it, and that click/key does nothing else.
  Redesigned (2026-11-16): at most 75% of the window each way, sections in
  1–3 columns (each to the shortest, `hints::flow`), scrolling under a fixed
  header instead of shrinking; typing filters (`hints::filtered`), Esc clears
  the filter then closes, other keys do nothing, only a click outside closes.
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
    keep following the zoom. The shortcut sheet scrolls in a small window.
  - Pages follow light/dark through DevTools media emulation
    (`prefers-color-scheme`) per page, because windowless (Alloy) browsers
    ignore the request context's Chrome colour scheme. Form controls only
    darken on pages that declare `color-scheme`, as in any browser.
  - Keys: F1 (also inside pages) shows the sheet; Ctrl+Q quits (the board
    autosaves, so nothing is lost). Wayland app id `trailboard`.
  - Test hooks: `TRAILBOARD_OMARCHY_STATE` and `TRAILBOARD_OMARCHY_SHELL`
    point at a fake theme and shell.toml; a `hyprctl` earlier on `PATH` fakes
    the rounding.
- ✅ A `.desktop` entry and icon (`share/trailboard.desktop`, `share/trailboard.svg`,
  `scripts/install-desktop.sh`) so Trailboard shows in the Omarchy launcher.
  Decisions (2026-11-14):
  - The entry declares `WebBrowser` and the http/https/HTML types, so it can be
    picked as the default browser (`xdg-settings`); the script doesn't do that
    itself. It installs per user and runs the binary by its absolute path
    (packaging in M9 will install to `/usr`).
  - A second launch on the same profile is handed to the running instance by
    Chromium's own process singleton (`on_already_running_app_relaunch` →
    `WebEvent::Relaunch`; the second process gets `Startup::Handed` and exits
    0). No socket of our own. The profile is per `XDG_DATA_HOME`, so isolated
    test runs never reach the user's window.
  - Icon redesigned (2026-11-16) after Omarchy's own app icons: the object
    seen from the front on a transparent 512 px canvas, Tokyo Night colours,
    one hard `#0e0f17` shadow per piece, and a `<title>`/`<desc>`. A link on
    a page card leads down a trail into a new page beside it, forming a
    lowercase t; few shapes, so it reads at 16 px.
    `share/trailboard-symbolic.svg` is the one-colour version (installed
    under `hicolor/symbolic/apps`).
  - Handed pages open as new cards in the space on screen, near the view's
    middle, the last one current; Read Mode is left first. With no pages the
    window is only asked to come forward (`focus_window`; Hyprland may just
    mark it urgent).
  - Arguments resolve with `launch::target`: an existing file (relative to the
    launch's directory) becomes a `file://` URL, otherwise as the address bar
    would. Several arguments open several cards (first launch too).
- ✅ Visual polish pass (`app::chrome`; 2026-11-14). Decisions:
  - Corners keep following Hyprland (asked: sharp stays on Omarchy's
    default); the polish is depth, edges, type and motion instead.
  - One surface style for everything over the board (`chrome::panel`): a
    two-part shadow (tight contact + wide ambient; `Lift` Low/Mid/High), a
    hairline edge a step off the panel (`chrome::edge`), and a 1 px top
    sheen. Used by the palette, menu, toasts and labels, the shortcut sheet,
    the page preview and the card name label; the sidebar casts the same
    two shadows sideways.
  - The palette's field and list are one sheet with a hairline between them,
    over a dimmed board (`chrome::scrim`); the highlighted row gets a short
    accent bar; a command's keys show as keycaps (`chrome::keycaps`: only a
    single shortcut, modifiers first; hosts and "B · Esc" stay text).
  - Overlays ease in over 150 ms (`chrome::Appear`, cubic ease-out, restarted
    when what they show changes): the palette slides 8 px down into place
    and fades, toasts 10 px, menus 4 px, the sheet fades, the sidebar slides
    in from the left. Hit testing uses the settled places throughout.
  - Cards: a wide ambient shadow plus a contact shadow (the focused card's
    higher), a hairline edge on unfocused cards, an accent glow around the
    current card (stronger while it has the keys), a sheen and a hairline
    on the title bar.
  - The board darkens slightly towards the window's corners (vignette 0.13
    dark, 0.04 light; `Background::vignette`).
  - The empty board says "An empty board" over its hint, keys as keycaps.
- ➖ Translations of the UI strings: dropped (owner's decision, 2026-11-15).
  English only; no string table needed.
- ✅ Settings page (2026-11-16, replaces the panel below): Ctrl+, opens
  `trailboard:settings`, an app page card (`settings_page`, `settings.html`)
  built like the search engine guide; Ctrl+, again goes to it. Side
  navigation by section, a filter box, a note under each setting; switches,
  segmented buttons (≤ 4 choices) or a drop-down; a proxy server field;
  buttons that run *History*, *Downloads*, *Sites with ads allowed*, *Clear
  history*, *Clear cookies*, *Keyboard shortcuts*; the sandbox under About.
  Messages `set:key=value` (`Key::id`, `Settings::choices`/`choose`),
  `proxy-server:`, `run:`; the app re-sends the data on every save. Corners
  follow Hyprland's rounding (`--radius` in the page theme; small parts
  capped at 4px), as do the welcome page, start page, engine guide and
  notes' code blocks (`welcome::colors`, `note::colors` carry `radius`).
- ✅ Board panning like page scrolling (2026-11-16, `scene::scroll::Pan`):
  a notch is 120 px × *Scroll speed*, gliding with an ease-out (τ 70 ms,
  ~0.2 s), notches add up; touchpad 2.5× surface px, at once, with the
  page scroller's fling. Stepped every frame (the fling's speed is timed by
  it); a press, a camera animation or Read Mode stops it. Was: 60 px jumps
  per notch, touchpad 1:1, no fling, no speed setting.
- ✅ Start page cleaned up (2026-11-16): flat page (no glow, dot grid,
  shadows or gradient badges), accent border on focus, an Enter hint that
  says *Search <engine>* or *Go to address* by `omnibox::resolve`'s rule
  (kept in step by hand in `start.html`), the guide as a link under the bar.
- ✅ Start page with pinned groups, recent cards and a Customise panel
  (2026-11-16; `scene::startpage`, `storage::start`, `start.html`).
  Looked at the community favourites rather than browsers' own: Bonjourr
  and Tabliss (the most-starred open-source new tabs: clock first, quiet),
  Bento from r/startpages and the Homepage/Dashy dashboards (links in
  named groups), Tilde (number keys). Decisions:
  - Drawn as a small board: groups and recent pages are cards with a label
    above, a hairline and a low shadow, like the cards on the board.
  - Recent pages show history's thumbnails (`pagetext::thumb`, JPEG `data:`
    URLs), four of them, so the start page is the way back.
  - Ctrl/middle click opens beside the start page with a trail edge
    (`start::BESIDE` → `open_card(.., Some(start))`), the browser's own
    idea of a link; a plain click replaces the start page as before.
  - Only pins the user chose; no "most visited" filler. Up to 8 groups of
    12; a URL once on the whole page; titles and names cut to 60.
  - The page edits a copy and sends the whole state back (`start:` + JSON);
    `StartPage::cleaned` keeps what is valid. Other live start pages get
    `tbStart(...)`.
  - Clock: 12-hour or 24-hour (`hour24`, default 12-hour), a two-way
    segment under *Clock* in Customise rather than following the locale,
    so it is the user's choice.
  - `start.json` version 2 (groups, greeting, name, `hour24`); version 1's flat pins
    are read as one group, "Pinned".
  - Icons: our own icon store first, as PNG `data:` URLs (`Icon::png`,
    stored deflate); else the page loads the site's own `/favicon.ico`,
    then `/apple-touch-icon.png`, straight from the site (never an icon
    service, so no third party learns the pins); only then the site's
    letter in a hue from its host (OKLCH, low chroma). The box stays empty
    while an icon loads, so no letter flashes first.
  - No wallpapers, quotes, weather or to-dos (quiet chrome; the board
    already has notes).
- ✅ Settings panel (Ctrl+,; `scene::settings`, `storage::settings`,
  `palette::settings_rows`), since replaced by the page above. Decisions (2026-11-14):
  - Not a page or a window: the palette's field and list again
    (`Target::Settings`), like the history view, so it looks and works like
    the rest (type to filter, ↑/↓, wheel, Esc). Ctrl+, anywhere, inside pages
    too; Ctrl+, again or Esc closes; also *Settings* in Ctrl+K.
  - Every setting is a short list of choices; no free text, so nothing to
    validate. Enter, → or a click steps the highlighted row on, Shift+Enter, ←
    or a right-click back (←/→ no longer move the caret in this field: a
    filter of a few letters doesn't need it). The highlighted row's value
    shows ‹ ›. Choices wrap; the live-page number stops at its ends. The
    panel stays open after a change; each change applies at once and is saved.
  - The first four: *Search engine* (DuckDuckGo default, Google, Bing, Brave
    Search, Startpage, Ecosia, Kagi; used by the address bar, the palette's
    last row, the page menu's search and command-line words), *Pages kept
    awake* (2–16, default 6), *Keep history* (off stops recording text and
    thumbnails, like a private space, for every space; what is kept stays
    searchable, Clear history still works), *Page colours* (follow the
    theme / light / dark for `prefers-color-scheme`; the board keeps the theme).
  - Saved in `$DATA/settings.json` (version 1, shared by all spaces), next to
    `zoom.json` rather than in `~/.config`, so one `XDG_DATA_HOME` isolates a
    test run. Each field is read on its own: an unknown value (an engine a
    newer build added) keeps its default, the rest still load; a newer
    version is ignored and left alone until a setting changes.
  - `TRAILBOARD_MAX_LIVE` still wins for the run it is set in (the row says
    "5 · TRAILBOARD_MAX_LIVE"); stepping the row replaces it.
  - *Scroll speed* (2026-11-14; Slow 0.6×, Normal, Fast 1.5×, Faster 2.2×):
    scales page scrolling (`scene::scroll`). Pages scroll like a desktop
    browser. Measured in a page: Chromium animates each wheel event unless
    it carries `EVENTFLAG_PRECISION_SCROLLING_DELTA`, so a wheel notch goes
    out as one event of 120 px and Chromium glides it (~200 ms), while
    touchpad movement goes out precise (applied within a frame), × 4 (it was
    1:1 surface px, the "slow scrolling"), and a flick flings on lift
    (speed of the last 100 ms, exponential slow-down, τ 0.33 s, stops under
    60 px/s, sent once per 60 Hz frame; any new scroll stops it). An
    earlier try that eased notches ourselves stacked on Chromium's glide
    (330 ms a notch) and was dropped. Wheel over an unfocused card still
    pans the board.
  - Scrollbars (2026-11-15): Chromium's overlay scrollbars
    (`--enable-features=OverlayScrollbar`): a thin rounded thumb over the
    page while it scrolls or the pointer is near, fading when idle, no
    gutter. Chosen over styling `::-webkit-scrollbar` into every page (would
    fight sites' own styles and need injecting per frame). Measured while
    choosing: a card scrolls at an even ~60 paints/s, no slow frames, so
    the look, not the frame rate, was the problem. Not a setting.
  - Not yet settings: a custom search URL, the default card size, downloads
    folder (downloads use the desktop's folder for now), ad blocking (M8).
- ✅ Downloads and a downloads list (Ctrl+J; `web::download`,
  `scene::download`, `storage::downloads`, `palette::download_rows`).
  Decisions (2026-11-14):
  - No dialog: every download goes straight to the Downloads folder
    (`TRAILBOARD_DOWNLOAD_DIR`, else `XDG_DOWNLOAD_DIR` from
    `user-dirs.dirs`, else `~/Downloads`), as desktop browsers do by default.
    The name is Chromium's suggestion made safe (one path component, no
    control characters, not hidden, ≤200 bytes), numbered "a (1).pdf"
    ("src (1).tar.gz") when a file, a `.crdownload` or another running
    download has it. Chromium asks with `on_before_download`; the app answers
    on its next frame (`download::save_to`). A second ask for a running id
    (Chromium resuming an interrupted transfer by itself) reuses its file.
  - A card whose page never committed (`on_load_start` of the main frame)
    when it starts a download was opened only for that link: it is removed
    (no undo step, not in the closed stack) and the view returns to the card
    the link was on, focused if the new card was.
  - Chromium reports a download only to the client of the page that started
    it, so a page with a running download is never closed: sleeping,
    closing, the blank-card removal and a space switch hand it to `parked`
    (alive, unrendered) until its downloads end. No effect on the live cap.
  - Quit cancels running downloads (Chromium deletes their partial files) and
    they read "Stopped when Trailboard closed"; no resuming across restarts.
  - The list is the palette's list again (`Target::Downloads`): name, then
    progress ("1.5 MB of 3.0 MB · 50%") or how it ended, site, when. Enter
    opens a finished file with `xdg-open` (`TRAILBOARD_OPENER` for tests) or
    downloads a failed / cancelled / stopped one again (current page, any live
    page, else a new card that closes itself); Ctrl+Enter opens its folder;
    Shift+Delete cancels a running one or takes the row off (the file stays).
    Ctrl+J or Esc closes. Toasts on start, finish and failure.
  - Saved in `$DATA/downloads.json` (version 1, newest 200, all spaces);
    a private space's downloads are listed this session only. Files are saved
    there too (it is the user's folder), with the jar's cookies.
  - Page menu: *Download link*, *Save image* (`Page::download`); disabled in
    notes, which never download (`can_download`).
  - Not done: pause/resume, a folder setting (settings are choice lists), a
    progress mark on the board while the list is closed, "show in folder"
    selecting the file (needs the FileManager1 D-Bus call), dangerous-file
    warnings, a confirm before quitting with running downloads.
- ✅ Loading and site icons in titles; card names when zoomed out
  (`scene::icon`, `storage::icons`, `WebEvent::Favicon`). Decisions (2026-11-14):
  - The loading bar along the title bar's bottom edge (existing) is the
    loading mark; no spinner.
  - Icons come from Chromium's favicon list (`on_favicon_urlchange`, first =
    best, `/favicon.ico` last), fetched with `download_image` at ≤32 px and
    kept **per site** (`pagezoom::site_of`, http/https hosts only; `file:`,
    `data:`, notes get none). Every card on a site shows the same icon; a card
    that navigates shows its new site's icon (or none) at once.
  - Saved under `$DATA/icons/<safe name>.icon` (shared by all spaces): magic,
    version 1, size, the site itself (file names make `host:port` safe, so the
    site is kept inside), then BGRA8 with straight alpha. Re-fetched identical
    icons aren't re-written. No pruning yet (a few KB a site).
  - The icon sits at the title bar's left at 16 units and keeps showing down to
    4 px, after the title (≥6 px) is gone, so cards can still be told apart.
    The shader now multiplies by the texture's alpha (pages are opaque).
  - Below `icon::READABLE_TITLE` (9 px titles) the card under the pointer, or
    else the current card, gets a screen-sized label with icon and title under
    it (above when there's no room), kept inside the window. Hidden during a
    drag, a menu, the palette or omnibox, link hints and the shortcut sheet.

### M7: History and search ✅
- ✅ `page-text.db` (`storage::pagetext`, rules in `scene::pagetext`,
  capture in `web::text`). Decisions (2026-11-14):
  - Our own schema, SQLite via `rusqlite` (bundled, so FTS5 is always there):
    `pages(id, url UNIQUE, title, seen_ms)` and an FTS5 table `pages_text`
    (`title`, `url`, `body`; `rowid` = `pages.id`), `unicode61
    remove_diacritics 2`, prefix indexes 2 and 3. Version in `PRAGMA
    user_version` (1); a newer file turns history off and is left alone. WAL.
  - One row per address (fragment included, so a single-page app's views are
    separate), replaced with the latest title and text on every visit. Only
    `http`, `https` and `file` pages; error pages, `data:` and `about:` are
    not kept; notes are never read. Text cut to 64 KiB, titles to 500 bytes.
    The newest 5000 pages are kept (pruned when 100 more have built up).
    Shared by all spaces, like `zoom.json`.
  - When: 1.2 s after a card's page finishes loading, or after it changes its
    title while not loading (single-page apps). The app asks
    (`Page::request_text`); the page answers with a console message; `web`
    drops it unless the address it reports is the one the browser shows, so
    a page can't write history for another address.
  - Query: the typed words, each as a quoted prefix, all required; BM25 with
    title 4, address 2, text 1; ties to the most recent.
  - Palette: with something typed, up to 6 history pages come after the
    board's results, before the web search row. A page a card shows is that
    card's row (so text inside open cards is found too), otherwise "History ·
    host" opens it in a new card. Words starting with a query word are
    highlighted. Results are cached per query.
- ✅ Thumbnails (`storage::thumbs`, `PageIndex::{set_thumb, thumb}`,
  `palette::preview_layer`, `App::load_sleeping_thumbs`). Decisions (2026-11-14):
  - A `thumbs(page_id, jpeg)` table in `page-text.db` (version 2; version 1
    files gain it on open, older builds refuse the file). One per page,
    replaced on every visit; forgetting, clearing and pruning drop it, and
    `secure_delete` wipes the bytes (unit-checked on the file).
  - Taken when the page's text is recorded (same moment, same rules, so
    never in a private space), from the card's last painted frame: shrunk
    by area average to fit 480×360, JPEG quality 72 (`jpeg-encoder`; read
    back with `zune-jpeg`). Test pages came to 3–5 KB. Kept for the newest
    1000 pages only (pruned when 100 more have built up); text for 5000.
  - Shown in two places. (1) The palette and the history view: the
    highlighted row's page (a history page, or a closed card's) beside the
    list, 320 px wide, right of it when the window has room, else under it
    at its right edge, else not at all. (2) Sleeping cards with no picture
    (restored after a restart, or back from another space) get their page's
    thumbnail, cropped to the card's shape (top kept), dimmed like a sleeping
    snapshot, up to 4 looked up per frame, each card once.
  - Not shown on history rows themselves (the list keeps one row height and
    no images per row).
- ✅ History view and forgetting (`Target::History`, `palette::history_rows`,
  `scene::when`, `PageIndex::{browse, forget, clear}`). Decisions (2026-11-14):
  - Ctrl+H opens it in the palette's field and list (one list UI, same keys
    and mouse), not a separate panel or a page. Newest first, up to 500
    pages; typing keeps the pages with all the words (as the palette's
    history search) but stays newest first, as here you look for *when*.
  - Days are not headings (the list has one row height); each row's detail
    says when: "Today 14:32", "Yesterday 09:05", a weekday within the week,
    then "10 Nov", or "10 Nov 2025" in another year. Local time via `chrono`
    (zone offset only; the calendar maths is ours, in `scene::when`).
  - A page a card shows says "On the board" and Enter goes to that card;
    other pages open in a new card. Ctrl+H or Esc closes.
  - Shift+Delete forgets the highlighted page, here and on history rows in
    the Ctrl+K palette (as browsers' address bars do). No toast (the toast
    would wait under the list); the row goes away. Not undoable.
  - *Clear history* (palette command, no key) asks first like deleting a
    space (`Confirm` generalises the old `confirm_delete`): Enter clears, any
    other key or a click keeps it. It also drops pending text reads.
  - Forgetting really removes the words: `secure_delete` on, FTS5
    `optimize` after a forget, `VACUUM` after a clear, then a WAL
    checkpoint (TRUNCATE). Unit- and e2e-checked by grepping the files.
  - A page an open card shows is recorded again on its next load or retitle.
- ✅ Private (unrecorded) spaces (`SpaceEntry::private`, `web::Jar`,
  `SiteZooms::private`, `App::toggle_private`). Decisions (2026-11-14):
  - Private is a flag on a space, not a separate kind of window: Ctrl+Shift+P
    (or *Private space on or off* in the palette) turns it on or off for the
    current space; *New private space* (palette, no key) makes one. Per-space
    history (a history per space) was not done: one shared history, and
    private spaces add nothing to it.
  - In a private space: no page text is read (so nothing reaches
    `page-text.db`), site icons are shown but not written to `icons/`, and
    page zoom changes live in an overlay over the saved zooms that is never
    saved (kept per space for the session, dropped when private is turned off).
  - Cookies, storage and cache: one in-memory CEF request context per private
    space (`web::Jar`, empty cache path), shared by its cards and kept for the
    app's session, so switching away and back keeps you signed in; quitting
    or turning private off forgets it. Toggling reopens the space's pages so
    they move into or out of the jar.
  - The board itself (cards, addresses, notes) is still saved: a private space
    is unrecorded browsing on a kept board. Pages already in history stay
    there (forget them in Ctrl+H). History is still searchable from a private
    space.
  - The corner label shows "· Private" (and appears even with one space).
  - `spaces.json` v3 so an older build refuses the index instead of opening a
    private space and recording it.
- ✅ Recently closed cards as palette results (`palette::Item::Closed`,
  `App::reopen_closed_card`). Decisions (2026-11-14):
  - The palette offers the space's Ctrl+Shift+T stack (this session only, as
    before; not saved), newest first, each card once, minus any back on the
    board through undo. Up to 10 take part in the ranking when something is
    typed; with nothing typed the newest 3 are listed right after the open
    cards. Detail "Closed · host" (or "Closed · Note"), with the trail or
    sticky colour dot it had.
  - Found by title and address (a note by its text), like open cards. A
    history page that a closed card showed is that card's row, so words on
    a closed page find the card itself, not a new card.
  - Enter brings that card back where it was, with its trail links, as one
    undo step (as Ctrl+Shift+T does), and takes it off the stack (older
    entries for the same card too).
  - A card removed by *undoing* its opening is not "closed" (redo brings it
    back), so it is not offered; same as Ctrl+Shift+T.

### M8: Privacy ✅
- ✅ Brave `adblock` engine (0.13, MPL-2.0) with EasyList/EasyPrivacy in a
  resource request handler (`web::shield`, 2026-11-15). Decisions:
  - The lists are not shipped: they are fetched from easylist.to on the first
    start (3 s after it, so the first pages load first) and again when older
    than 4 days (their own `Expires`), into `$DATA/filters/<id>.txt`; file
    mtime is the fetch time. A fetch goes through Chromium (`CefURLRequest`,
    the profile's proxy settings and cache, not tied to a page so never
    blocked itself) and must look like a list (`[Adblock` header, ≥100
    lines) before it replaces the saved one; a failure retries after an hour.
    Until the first fetch, nothing is blocked. `filters/user.txt` holds the
    user's own rules in the same syntax (never fetched). It is watched
    (inotify on `filters/`, made at start so it can be watched; ✅
    2026-11-15): 300 ms after the last change it is read again and, if its
    text differs from what the rules were built with, all lists are rebuilt
    and a toast says "Your filter rules reloaded: N rules" ("… were
    removed" when deleted; "(blocking is off in the settings)" appended when
    it is). Decisions: reads (open/close-nowrite) don't count, or reading it
    would wake the watcher; only the name `user.txt` counts, so editors'
    temp and swap files don't, and a save through a rename does; pages
    already open keep what they loaded, the new rules apply to their next
    requests and loads (no automatic reload).
  - Network and cosmetic rules (`RuleTypes::All`, since the cookie-banner
    item). Building the engine from the three lists (162k lines) takes
    ~110–120 ms (network rules only from two lists: ~80–140 ms) (dependencies build at opt-level 2 even in
    debug) on a thread of its own; the rules are swapped in whole, and a
    newer build wins over a slower older one.
  - Checked in `OnBeforeResourceLoad` on Chromium's IO thread: the one place
    this crate uses locks and atomics (`RwLock` rules, `AtomicBool` switch,
    the app's allowed-site test as a `Send + Sync` closure over an
    `Arc<RwLock<AllowedSites>>`). The `RequestHandler` hands out a
    `ResourceRequestHandler` holding only an `Arc`, never touching the
    page's `Rc` state off the main thread. The `adblock` crate is built
    without its `single-thread` feature, so its `Engine` is `Send + Sync`.
  - The first party is the main frame's URL (the referrer before it is
    known). Main-frame documents are never blocked, nor `data:`/`blob:`.
    Notes get no handler.
  - Each page counts the distinct URLs it blocked since its main frame last
    navigated (Chromium asks twice for one when its preload scanner and the
    parser both want it): "N blocked" in the page menu.
  - ✅ The count on the title bar (2026-11-15): a small outlined shield
    (a `RoundBottom` quad hollowed by a second one; a plain filled shape
    when the zoom makes the outline thinner than a pixel) and the count,
    left of ×, in the title's colour. Decisions: only while blocking is on
    for the page's site and the count is above 0 (`scene::privacy::
    blocked_badge`, capped at "99+"); scaled with the card like the title,
    and dropped when it would leave the title less than 48 units; the title
    is clipped short of it.
  - ✅ Clicking the shield (2026-11-15) opens a short menu under it with
    just the site's blocking entries (`web::menu::blocking_items`, the same
    ones the page menu ends with: *Cookie banner rejected* when it applies,
    then *Allow ads on … · N blocked*). Decided over toggling at once: a
    click on a small mark shouldn't reload the site's pages unasked. The
    target is the drawn badge plus 3 units each side, title bar high,
    recorded while drawing (`App::shields`), so it follows the badge
    exactly and is gone whenever the badge is; the rest of the title bar
    still drags and double-clicks.
  - ✅ "N hidden" beside "N blocked" (2026-11-15): the elements the
    cosmetic rules hide are counted in the page and shown in the menu
    (*Allow ads on … · 22 blocked · 2 hidden*; a zero part is left out).
    Decisions: the cosmetic script counts the elements its sheet's
    `display:none` rules match, not those inside one already hidden, so a
    hidden ad box counts once; it recounts at most once a second, after
    the sheet grows or the DOM changes (a MutationObserver on every page,
    `$generichide` ones too), reports only a changed count
    (`cosmetic::COUNT_PREFIX`), and a frame whose count took over 50 ms
    stops counting and keeps its last figure. Each frame's count lives
    beside its exceptions in `cosmetic::Frames`; `Page::hidden` sums them,
    and they reset when the main frame (or that frame) loads again. The
    title-bar badge shows blocked + hidden, so a page where only elements
    were hidden still gets the shield and its menu.
  - *Block ads and trackers* in the settings (on by default; `block_ads` in
    `settings.json`) turns it off everywhere at once; open pages keep what
    they loaded until reloaded.
- ✅ Cookie-banner auto-reject (cosmetic rules + known CMP actions).
  - ✅ Cosmetic filtering (`web::cosmetic`, 2026-11-15). Decisions:
    - A third list, the EasyList Cookie List (CC-BY 3.0, published at
      `secure.fanboy.co.nz/fanboy-cookiemonster.txt`; easylist.to has no
      copy), fetched and refreshed like the others. Its network rules stop
      many consent platforms' scripts outright (OneTrust's `otBannerSdk.js`
      in the e2e run); its hiding rules catch the rest.
    - When a frame of a web page starts loading (`OnLoadStart`, any frame),
      the app asks the engine for the frame's site rules plus the generic
      selectors that aren't a plain class or id (~880 with all lists) and
      runs a script there that adopts them as a constructed stylesheet
      (`document.adoptedStyleSheets`: not in the DOM, so page scripts that
      rebuild `<head>` don't drop it). One rule per selector, so a selector
      Chromium can't parse drops only itself; selectors or styles that could
      spill out of their rule (braces, comments, open quotes or brackets,
      `url(`) are left out in Rust before that.
    - Generic class/id rules (most of EasyList) are looked up on demand: the
      script watches the frame (`MutationObserver`: added nodes, `class`/`id`
      changes, batched every 30 ms) and reports new names in a console
      message carrying the frame's identifier; the app answers into that
      frame with the selectors they could match, minus the site's
      exceptions (kept per frame, cleared when the main frame navigates).
      At most 20,000 names per frame. `$generichide` pages (EasyList has it
      for `127.0.0.1` and `localhost`) get no generic rules.
    - `:style()` rules that are plain CSS are applied, and `:remove()` ones
      hide; procedural rules (`:has-text()`, `:upward()`, …) and scriptlets
      (`+js()`) are not.
    - The same switches as network blocking: the setting, the per-site
      toggle (it reloads, so the stylesheet goes) and the first party being
      the main frame. Notes get nothing. One switch for ads and cookie
      banners for now.
    - A page can see the hook (`Symbol.for('trailboard.hide')`) and could
      send fake reports; at worst it hides list-matched elements in its own
      frames.
  - ✅ Answering banners with "reject" on known consent platforms
    (`web::consent`, 2026-11-15). Decisions:
    - Our own table of 15 platforms (OneTrust, Didomi, Cookiebot,
      Sourcepoint, Quantcast, Usercentrics, TrustArc, Google Funding
      Choices, consentmanager, CookieYes, Complianz, iubenda, Osano, Klaro,
      Cookie Notice): each a banner selector and its reject button, written
      from the ids and classes the platform's banner shows on any site, not
      taken from other blockers' lists. Quantcast's buttons have no role in
      their markup, so its button must read as a refusal (a whole-label
      pattern in English and the common EU languages). Usercentrics' banner
      is in an open shadow root. Sourcepoint's is a frame of its own
      (`message_id=` in its address) where the button is clicked, plus a
      watch-only rule on the page's `sp_message_container_*`.
    - Clicks the DOM button, never the platform's JS API (an API call could
      overwrite a choice the user made earlier; a button exists only while
      the platform asks). Only when the banner is shown, counting one our
      hiding rules hide (the hide stylesheet is disabled for the check), so
      a hidden banner still gets answered and stops asking. Once per frame,
      watched for 30 s from the start of loading (mutations, plus checks at
      0.5/1.5/4/10 s).
    - Scroll lock: 0.8 s and 2.5 s after a banner is found, if it is gone or
      hidden and `html` or `body` still computes `overflow: hidden`, both
      get `overflow: auto !important`. Not when the banner is still visible
      (no reject button: the user still has to answer it).
    - Same switches as hiding (runs from `cosmetic::on_load_start` after
      `shield::rules_for`): the setting and the per-site toggle cover it.
      No separate switch yet. Outcomes (`rejected`, `no-button`, `shown`,
      `unlocked`) are logged at info level.
    - ✅ Shown in the UI (2026-11-15): a `rejected` from any frame marks the
      page (`Page::banner_rejected`, cleared when its main frame starts
      loading); the page menu then has a dim, unclickable "Cookie banner
      rejected" line above the site toggle, while blocking is on there.
- ✅ YouTube ads (`web::youtube`, 2026-11-15). Network rules can't stop
  them (same servers as the videos), so a script of our own runs in YouTube
  frames (youtube.com and subdomains, youtube-nocookie.com embeds) as they
  start loading, before the cosmetic script. Decisions:
  - Drops `adPlacements`, `adSlots`, `playerAds`, `adBreakHeartbeatParams`
    from the player response before the player reads it: an accessor trap
    on `ytInitialPlayerResponse`/`playerResponse` (first load), and
    wrappers on `JSON.parse` and on `fetch` for `/youtubei/v1/player` and
    `get_watch` replies (moving to another video in the page).
  - Fallback for an ad that plays anyway (script ran late, ad stitched into
    the stream): every 250 ms, while the player has `ad-showing` /
    `ad-interrupting`, click the skip button, mute, rate 16 and seek to the
    end; the user's mute and speed come back when the ad ends.
  - Hides feed/masthead/sidebar ad slots and the "ad blockers are not
    allowed" dialog (removed, its backdrop too, and a video it paused
    plays on).
  - Switches: the setting and the per-site toggle (`shield::active_for`,
    which doesn't need the lists loaded). Outcomes (`pruned`, `skipped`,
    `dialog`) are logged once per frame at info level. Not counted on the
    shield badge yet. YouTube changes its client often; the key names and
    selectors are the parts to update when it breaks.
- ✅ Per-site toggle (2026-11-15): the page menu's last row ("Allow ads on
  example.com · 12 blocked" / "Block ads on example.com") and *Allow or block
  ads on this site* in Ctrl+K. Decisions:
  - A site is `icon::site` (host with port, `www.` dropped) and covers its
    subdomains (`scene::privacy::covers`); turning blocking back on from a
    subdomain removes the entry that covered it. Kept in `$DATA/privacy.json`
    (version 1, `allowed`), shared by all spaces.
  - Toggling reloads the site's live pages bypassing the cache: a plain
    reload lets Blink take scripts and images from its memory cache without
    a request, so they would not be blocked (found in the e2e run).
  - From a private space, an allowed site is in force until quitting but
    never saved (it would record a site visited privately).
  - Only one switch for now: ads and trackers together. Separate switches
    (trackers only, cookie banners) can come with cosmetic filtering.
  - ✅ A list of allowed sites (2026-11-15): *Sites with ads allowed* in
    Ctrl+K opens a list view (`Target::AllowedSites`,
    `palette::allowed_rows`), sorted by site, type to filter. Each row says
    "Until you quit" for a site allowed from a private space and how many
    cards on the board (this space) show it. Decisions: Enter, a left
    click or Shift+Delete blocks ads on the site again (there is nothing
    else to do with a row, and the shield menu undoes it); the list stays
    open, like the settings panel, so several can go in a row; the site's
    live pages reload as with the page-menu toggle. No settings-panel row
    for it yet: the panel has no rows that open other views.

- ✅ Browsing settings (2026-11-16), asked by the owner as "Chromium's
  settings": CEF draws windowless (Alloy style), so `chrome://settings` and
  Chrome's browser UI aren't there; each setting is built on a CEF hook.
  - Profile preferences (`web::prefs`, set on the profile and every private
    jar): spelling, third-party cookies (`profile.cookie_controls_mode`),
    proxy. cef-rs passes an empty error `CefString` as a null pointer and
    CEF then refuses `SetPreference`, so the error starts as `""`.
  - `Accept-Language`: the `intl.accept_languages` preference is stored but
    the network stack only reads the language list when a context starts
    (`CefSettings`/`RequestContextSettings::accept_language_list`), and a
    header set in `OnBeforeResourceLoad` is overwritten. So a change reaches
    sites after a restart (private jars made later get it at once).
  - On requests, in `shield::Guard` (IO thread): `DNT`/`Sec-GPC` headers,
    and images cancelled per site.
  - JavaScript per page through DevTools `Emulation.setScriptExecutionDisabled`,
    sent before each main-frame navigation; a change reloads the pages it
    affects. App pages (welcome, notes) always run theirs.
  - Permissions (`web::permission`): `OnRequestMediaAccessPermission` and
    `OnShowPermissionPrompt`; Ask shows a `Confirm` line (Enter allows), the
    answer holds per origin and kind for the session. Location gets no
    position (no provider in CEF); geolocation and camera need a secure origin.
  - Downloads: Downloads folder, Desktop (`XDG_DESKTOP_DIR`), or a path field
    (`Target::SaveAs`; closing it drops CEF's callback, which cancels).
  - Not done: per-site camera/location lists, notification display (Alloy
    shows none), font sizes, site data other than cookies.
- ✅ Blocking like uBlock Origin (2026-11-16), asked by the owner: sites that
  open in Firefox or Chrome with uBlock Origin showed "turn off your ad
  blocker" walls here, and turning blocking off let every ad in.
  - Cause: a request the lists stop was cancelled outright, so a page that
    checks its ad script loaded (`adsbygoogle`, `googletag`, the IMA SDK) saw
    it fail; and the rules that defeat the walls (`##+js(…)` scriptlets,
    `$redirect=` stand-ins, mostly in uBlock's own lists) had nothing to run.
  - Lists (`scene::privacy::LISTS`): uBlock Origin's default set, adding
    uBlock filters, Badware, Privacy, Unbreak (exceptions for breakage) and
    Quick fixes (fetched daily), and Peter Lowe's list. `!#include` parts are
    fetched beside their list (`list--part.txt`) and spliced in on load;
    `!#if` blocks are kept or dropped as uBlock defines the tokens for a
    desktop Chromium without HTML filtering. Lists may start with
    `! Title:` (uBlock's) as well as `[Adblock`. One rebuild per batch of
    fetches.
  - Resources (`web::resources`, our own JS in `web/src/surrogates/`):
    stand-ins under the names the lists use (`noopjs`, `nooptext`, `1x1.gif`,
    silent MP3/MP4, VAST/VMAP, `googlesyndication_adsbygoogle.js`,
    `googletagservices_gpt.js`, `google-ima.js` (ad requests fail so the
    player goes to the video), analytics stubs that still call
    `hitCallback`/`eventCallback`, BlockAdBlock/FuckAdBlock defusers) and
    the scriptlets that cover most uses in uBlock's lists (`set`, `aopr`,
    `aopw`, `acs`, `aost`, `nostif`, `nosiif`, `nano-stb`, `nano-sib`, `aeld`,
    `nowoif`, `rmnt`, `no-fetch-if`, `no-xhr-if`, `noeval-if`, `json-prune`,
    `json-prune-fetch-response`, `ra`, `rc`, `set-attr`, storage and cookie
    setters, `remove-cookie`, `href-sanitizer`, `nowebrtc`). `trusted-*`
    scriptlets, `rpnt`, `xml-prune`, `m3u-prune` and
    `json-prune-xhr-response` are not there; their rules are skipped.
  - Stand-ins are served by `shield::Stand`, a CEF resource handler on the IO
    thread, with the request's origin allowed (CORS) so `fetch` checks pass.
    They count as blocked.
  - Scriptlets must run before the page's scripts; `execute_java_script`
    from `OnLoadStart` arrives after the first inline ones. The main frame's
    are registered with DevTools `Page.addScriptToEvaluateOnNewDocument` in
    `OnBeforeBrowse` (Page domain on), guarded by the host, and the previous
    page's script removed (`cosmetic::Early`, ids from a DevTools observer).
    Subframes still get theirs at load start. The YouTube script rides the
    same early path.
  - The first rules are built synchronously at start (~120 ms with all nine
    lists), so pages restored at start don't load unblocked.
  - Checked with a local page (stand-ins for `<script>` and a credentialed
    cross-origin `fetch`, `set`, `aopr`, `nostif` before inline scripts) and
    hindustantimes.com (no wall, 13 blocked, 3 stand-ins).

### M9: Packaging and release ✅
- ✅ Bundle layout (`scripts/bundle.sh [--tar]`): `target/bundle/trailboard-VERSION-ARCH/`
  holds the release binary, CEF's runtime files and locales beside it,
  `share/applications/trailboard.desktop`, the icon, README, CHANGELOG and
  `CEF-CREDITS.html`. Decisions (2026-11-14):
  - The binary carries `$ORIGIN` in its rpath (`crates/app/build.rs`, committed),
    after the developer's `CEF_PATH` rpath from the local `.cargo/config.toml`;
    `cef_dir()` already used CEF files next to the executable. So the plain
    `cargo build --release` output is what gets bundled; no separate target dir
    or RUSTFLAGS (which would rebuild everything). The extra developer path is
    harmless where it doesn't exist; a packager's build (no local config) has
    only `$ORIGIN`.
  - `libcef.so` ships stripped (`strip --strip-unneeded`: 1.4 GB → ~250 MB);
    folder ~384 MB, `.tar.zst` ~152 MB. All locales are kept (for UI
    translations later and Chromium's own pages).
  - `chrome-sandbox` comes along (not setuid; README says how to make it so).
    It was left out until the sandbox item below.
  - Default release profile, no LTO: the 7.6 GB machine and build time matter
    more than a smaller 17 MB binary next to 250 MB of CEF. Revisit for the AUR.
- ✅ `--version` / `-V` (`trailboard 0.1.0 (CEF 156.0.26+…)`) and `--help` / `-h`,
  answered before CEF starts, so a running Trailboard is never handed them
  (`launch::command`). The version line is also logged at start.
- ✅ Crash reports: a panic hook (`app::crash`, both the browser process and
  CEF helpers running our binary) appends time, version, thread, place,
  message and backtrace to `$DATA/crash.log` (`storage::crash`, newest
  reports within 256 KB, backtrace cut at 48 KB) after the usual stderr
  message, and leaves `crash-unseen`; the next start shows "Trailboard closed
  unexpectedly last time · report in …" for 8 s and clears the mark. Not
  caught: crashes inside Chromium (signals); those still go to stderr.
- ✅ Arch `PKGBUILD` (`packaging/arch/`, `cd packaging/arch && makepkg -si`).
  AUR upload, AppImage and Flatpak are out of scope: the owner will publish
  when and how they see fit (decision 2026-11-15). Decisions (2026-11-14):
  - Source: the git checkout the PKGBUILD sits in (`git+file://`, its last
    commit), or `TRAILBOARD_GIT`. The repo has no public remote yet, so there
    is no `url=` and no `.SRCINFO`; publishing to the AUR needs the public
    repository URL (the owner's call), a release tag in `source`, and
    `makepkg --printsrcinfo > .SRCINFO`.
  - CEF: the same minimal build `scripts/install-cef.sh` fetches, pinned by
    name and sha256 (its sha1 from CEF's index checked first), laid out in
    `prepare()` as cef-dll-sys expects (hard links, no copy) and handed to
    the build as `CEF_PATH`. No `.cargo/config.toml`, so the binary's runpath
    is only `$ORIGIN`.
  - Layout: the bundle in `/usr/lib/trailboard` (binary + CEF runtime,
    `chrome-sandbox` setuid 4755 since the sandbox item), `/usr/bin/trailboard` a symlink (`current_exe` and
    glibc's `$ORIGIN` both resolve it), desktop entry, icon, docs, and
    CEF's `CREDITS.html` in `/usr/share/licenses/trailboard/`.
  - `options=(!lto !debug)`: makepkg's LTO flags would make CEF's C++ wrapper
    bitcode, and the debug split would carry libcef's symbols. makepkg's own
    strip does what `bundle.sh` did. Still no Rust LTO (build time, memory).
  - `check()` runs only the `scene` and `storage` tests (the app crate's would
    rebuild everything with cfg(test)) and `trailboard --version`.
  - `depends` from the `NEEDED` entries of `libcef.so` and the binary, mapped
    with `pacman -Qo`, plus what winit/wgpu load at run time (`wayland`,
    `vulkan-icd-loader`) and `ttf-font`; `xdg-utils` and `xdg-user-dirs`
    optional. No licence file of our own yet (see the licence item below).
- ✅ Chromium's sandbox instead of `no_sandbox` (`web::sandbox`). Decisions (2026-11-15):
  - On by default. Chromium itself prefers user namespaces and falls back to
    the setuid helper `chrome-sandbox` in the executable's directory; with
    neither it aborts ("No usable sandbox!"). So before `initialize` we probe
    the same way (a forked child tries `unshare(NEWUSER|NEWPID|NEWNET)`,
    which also fails under Ubuntu's AppArmor userns restriction) and check
    the helper is root-owned, setuid and executable; with neither we set
    `no_sandbox` and log a warning, rather than not starting at all. The
    choice is the pure `sandbox::choose` (unit-tested) and is logged at start.
  - `TRAILBOARD_NO_SANDBOX=1` turns it off for a run (debugging). Not a
    setting: nobody should need it day to day. Not a `--no-sandbox` flag
    either, which `launch::target` would read as a search.
  - The same binary stays the subprocess (zygote, renderers); `main` does
    nothing before `web::start` that a sandboxed renderer would need (the
    panic hook can't write `crash.log` from a renderer; stderr still has it).
  - Packaging: the PKGBUILD installs `chrome-sandbox` 4755 (for
    linux-hardened, which has no unprivileged user namespaces; Arch's own
    chromium does the same); `bundle.sh` copies it unprivileged and the
    README says how to make it setuid.
  - ✅ Shown to the user (2026-11-15, fifth session): the settings panel
    (Ctrl+,) ends with a read-only *Sandbox* row ("On · user namespaces",
    "On · setuid helper", "Off · not supported here", "Off ·
    TRAILBOARD_NO_SANDBOX"); Enter or a click on it explains it in a toast,
    which over the settings panel hangs below the list (other lists still
    hide toasts). It is a `palette::Item::Status`, not a setting: ←/→ do
    nothing and it has no ‹ › marks. When the system can't run the sandbox
    (not when `TRAILBOARD_NO_SANDBOX` asked for it), the first start says so
    for 10 s and leaves `sandbox-off-told` in the data folder
    (`storage::marks`), so it isn't repeated; a start with the sandbox on
    clears the mark, so losing support later is told again. When the crash
    notice is showing, the sandbox notice waits for the next start rather
    than replace it.
- ✅ First-run defaults: the very first start adds the welcome page
  (`crates/app/src/welcome.html`; a Markdown note, `guide.md`, until
  2026-11-15) to the left of the first page, and the view fits both. Redesigned
  2026-11-16: its board picture is drawn by script from card data with the
  app's own geometry (trail anchors at 0.6 of the title bar, S-curve handles
  halfway, rings, tags, sticky strip) and the theme's board, dot and sticky
  colours (`welcome::colors`). Decisions (2026-11-14):
  - "First run" means nothing was ever saved in the data folder: no
    `spaces.json`, no space boards, no old `board.json`
    (`storage::spaces::is_first_run`, asked before `open_spaces` writes the
    index). An emptied board or a new space doesn't get the guide again, and
    neither does a run whose saving failed (it would come back every start).
  - It is an ordinary note, not a special card or page: no extra Chromium
    page type, the user can edit, keep, export or close it (Ctrl+W), and it
    is saved with the board. Not focused, so the page keeps the keys.
  - The start page stays (a search box card since 2026-11-15, or the URLs given on
    the command line); the guide goes beside it with
    `placement::left_of` (tops aligned, `free_near` when taken).
  - Changed at the owner's request (2026-11-15): no longer a Markdown note
    but a designed page (`crates/app/src/welcome.html`, `welcome.rs`), on an
    880×720 card (1024×720 since 2026-11-16) the board keeps as `trailboard:welcome`
    (`scene::note::WELCOME_URL`, `Scene::add_welcome`/`welcome_card`). It
    loads as a `data:` URL with the theme's colours (recoloured live like
    notes) in a new `web::PageKind::App`: never navigates (links open
    beside), no title/address/icon/text/history/blocking, no downloads. Its
    buttons send `web::APP_ACTION` console messages that only App pages can
    send; the app knows four (`welcome::action`: new card, palette,
    shortcuts, settings) and ignores the rest. Ctrl+L on it (or on a note)
    opens the address beside instead of doing nothing. Sections: hero with
    a board illustration (cards fading in, trails drawing), three ideas,
    first minute, where your keys go (thin vs thick ring), shortcuts worth
    knowing, good to know, a closing call to action. No web fonts or remote
    assets; motion off under `prefers-reduced-motion`. Old boards keep any
    welcome *note* they have as an ordinary note. English only
    until the translation item.
  - *Welcome note* in Ctrl+K brings it back: it goes to an unedited welcome
    note already on the board (unfolding its stack; `Scene::note_with_text`),
    else adds a fresh copy in the middle of the view, current. An edited
    guide counts as the user's own note, so a fresh copy is added. Typing
    "welcome" ranks the guide card itself (open or recently closed) above
    the command, which is fine: either way the guide comes back.
- ✅ Licence: Apache-2.0 only (owner's decision, 2026-11-15). `LICENSE-APACHE`
  holds the unmodified standard text; `license = "Apache-2.0"` in `Cargo.toml`
  and the `PKGBUILD`. No `LICENSE-MIT`. No copyright line is needed in the
  file itself; one would only go in a `NOTICE` file or source headers.

### M10: Optional extras ⬜
See [FUTURE.md](FUTURE.md). Each item starts only when the owner picks it.
- ⏸ **AI features: on hold for the owner's decision (2026-11-15).** Do not
  design, prototype or build any of these, or add AI dependencies (models,
  ONNX Runtime, LLM APIs, API-key storage), until the owner says so:
  - Semantic search ("search by meaning", a local embedding model).
  - Explain (a side panel using the user's own LLM key).
  - Any other AI or ML feature (summaries, AI tab grouping, page Q&A, …).
- ⏸ **Import from other browsers** (history, bookmarks, sessions, cookies):
  on hold for the owner's decision (2026-11-15). Do not start until the owner says so.
- ⏸ **Password managers and passkeys** (native messaging for 1Password/
  Bitwarden, WebAuthn): on hold for the owner's decision (2026-11-15). Do
  not start until the owner says so.
- ⬜ CEF extensions (a curated set); only when the owner picks it.

### M11: Premium look 🟡
Started by the owner on 2026-11-15 with the sidebar (step 4); take the
other steps only when the owner asks for them.

**Owner answers (2026-11-15):** corners keep following Hyprland's rounding
(sharp on this machine, no separate *Look* setting for now), so "premium"
has to come from spacing, type, depth, icons and motion, not soft corners;
the sidebar both opens with B and peeks from the window's left edge. Aim: Trailboard should
feel as finished and calm as the premium browsers (Zen, Arc) and Marble,
while staying a board of cards and staying clean-room.

**Clean-room note.** Zen, Arc and Marble are references for *principles*
only. Copy no art, icons, names, colour values, gradients, sounds or CSS
from any of them, and don't take measurements from third-party "design
system" pages that imitate them. Every value below is ours to choose.

**Research (2026-11-15), what makes them feel premium:**
- *The page is the hero; the chrome recedes.* Arc and Zen put tabs in a
  vertical sidebar and remove the top tab bar. Zen's compact mode hides the
  sidebar and toolbar entirely and brings them back when the pointer reaches
  the window's edge, or pins them with a key. The page sits as an inset,
  rounded sheet inside a tinted window frame instead of running edge to edge.
- *One colour mood per space.* Both let each space (Arc "Spaces", Zen
  "workspaces") carry its own colour or soft gradient, which tints the
  frame, sidebar and accents, so you know where you are without reading.
  Zen adds an optional grain (noise) over the gradient to avoid banding and
  flatness. Marble gives each space its own background (a photo or colour).
- *Few, consistent surfaces.* A small set of surface steps (base, raised,
  hover, hairline edge) used everywhere; floating things (command bar,
  menus) are one elevated sheet with a soft wide shadow and a faint light
  outline.
- *A central command bar.* Arc's command bar is a centred sheet in the upper
  part of the window, large input text, rows with right-aligned key hints:
  one place for tabs, history and actions. (Trailboard's Ctrl+K palette is
  already this; it needs the finish, not the idea.)
- *Quiet, quick motion.* Short eased transitions (≈150–250 ms) for the
  sidebar sliding, titles fading after it, spaces switching with a slight
  spring; nothing bounces for long.
- *Editorial type.* Marble's site uses large sentence-case display lines and
  small, spaced uppercase labels; Arc keeps UI text small and the command
  input large. A clear type scale (label, body, title, display) matters more
  than the font.
- *Restraint.* Mostly neutral surfaces with one accent colour; colour comes
  from the space's tint, not from many coloured controls.

**Where Trailboard is today:** the Omarchy look (palette from the Omarchy
theme, corners from Hyprland, sharp by default) plus the 2026-11-14 polish
pass (`chrome::panel` surfaces, two-part shadows, hairlines, eased
overlays, vignette, keycaps). Solid, but flat and "tool-like": cards have
square title bars, the board is a plain dotted colour, spaces differ only by
background colour, and the sidebar is a plain list.

**Open question for the owner (ask at the start):** the premium look needs
rounded corners and per-space colour, which the Omarchy look deliberately
doesn't do (corners follow Hyprland, colours follow the Omarchy theme).
Proposed answer: a *Look* setting with two choices, **Trailboard** (the new
look, default) and **Omarchy** (today's, unchanged), so Omarchy users keep
a matching app. Light/dark keeps following the system in both.

**Steps, in order (each a session or two, each screenshot-checked in dark
and light):**
- ⬜ 1. Design tokens: one table of surface steps, hairlines, radii (small
  controls, panels, cards), shadows, spacing (4 px grid) and a type scale,
  in `app::chrome`/`app::theme`, used by every overlay; the *Look* setting
  (`scene::settings`, `storage::settings` with a migration).
- ⬜ 2. Space colour: each space gets a colour mood (a two-stop soft gradient
  picked from a small set of our own presets, or a single colour) that tints
  the board, the sidebar and the accent; optional subtle grain to stop
  banding. Picked from the space's menu / palette; stored in the board file
  (board version bump + migration). Spaces' existing background colours
  migrate to the nearest preset.
- ⬜ 3. Cards as sheets: rounded corners on the card and its page (page
  texture clipped by an SDF in the shader), a slimmer title bar that merges
  with the page top, site icon + title + quiet controls that appear on
  hover, softer depth for unfocused cards and a calm accent ring for the
  current one; Read Mode's single page inset with a margin and rounded
  instead of edge to edge.
- ✅ 4. Sidebar (2026-11-15, `app::sidebar`). Decisions:
  - A sheet floating 8 px in from the window's top, left and bottom
    (`chrome::panel`, mid lift, hairline edge), 280 px wide, corners from
    the theme. Opaque: the renderer has no blur, and a see-through list
    over live pages hurt reading.
  - Head: the space's name, "N cards · M trails" (or "Empty") under it,
    the B keycap on the right. A hairline under it only while rows are
    scrolled beneath.
  - Groups are set apart by a 10 px gap, not rules. With pinned trails,
    small "PINNED" / "TRAILS" labels head the two parts (none without
    pins: restraint). Pinned trails stay rows rather than a strip of tiles:
    rows keep j/k, drag-to-reorder and names readable.
  - Trail header: colour mark in the icon column, bold name in the title
    column, count right-aligned and dim; the chevron only shows when
    collapsed or pointed at. "Other cards" gets a hollow mark so every
    label starts on the same line.
  - Card rows: the site icon (else a letter tile from the host, ¶ for a
    note, the paper colour for a sticky note), the title cut to fit with
    "…" (the renderer wraps otherwise), faint guide lines down each trail
    level. Current card: an accent bar on the sheet's edge, a lifted fill,
    bold. Keyboard highlight: the theme's selection colour; hover: a
    lighter fill.
  - Foot: a chip per space (its tint; grey for none; hollow when private),
    the current one ringed, then "+" for a new space; the pointed chip's
    name beside them. A hairline over the foot while rows go on below.
  - Rows have different heights now (gaps, labels), so the scroll stays a
    first-row index but `Panel::{fits, keep_visible, max_scroll}` work out
    what fits instead of a fixed row count.
  - Edge peek (`sidebar::Peek`): the pointer resting 120 ms on the
    window's left 2 px opens it without the keys; 400 ms away from the
    sheet (24 px slack) closes it; B on a peek keeps it open. It only arms
    once the pointer has been seen inside the window off the edge, so a
    window opening under a pointer on its edge doesn't peek. Not while
    Read Mode, the palette, a menu, the shortcut sheet or a drag is on.
  - Opening slides it in from past the edge (150 ms ease-out, as before).
    Closing is instant; the slide out belongs to step 6.
- ⬜ 5. Command bar finish: the Ctrl+K palette as the centred hero sheet
  (larger input text, section labels in small caps, site icons on rows,
  keycaps right-aligned), and the same finish for the address bar, menus,
  settings and toasts.
- 🟡 6. Motion pass. Done (2026-11-15): Read Mode in and out
  (`scene::read::morph`, 200 ms ease-out): the card's header and content
  move to the read bar and window, screen px per CSS px moving evenly, the
  board dims behind (0.75 of its colour at full), corners square off, the
  title fades in over the second half. Only from a card on screen; a
  switch between read cards stays a cut. Pages are drawn at the CSS size
  their frame was painted at (`web::PaintFrame::css`, `Shape::Page`'s
  extent): cropped when the frame is bigger, the row's left-edge colour
  past it when smaller, never stretched. Left: space switch (cross-fade of the tint and a short slide),
  sidebar peek, card open beside its source (grow from the link's side),
  close (shrink and fade), Tidy (cards glide to their places, already a
  follow-up), all ≈150–250 ms, eased, and off with the system's reduced
  motion setting if one can be read.
- ⬜ 7. Empty board and first start: a calm welcome with a large display
  line and a few keycaps on the space's tint; the app icon redrawn to match.
- ⬜ 8. Review pass: every overlay and state in dark and light, at small
  and large windows and a HiDPI scale; fix what looks off.

**Not in scope:** Arc/Zen features that aren't about the look (Boosts,
split view, auto-archiving tabs, Glance); user CSS themes or mods.

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

**Last session:** 2026-11-15 (thirteenth). Follow-up: Shift+Enter in the
palette to open beside.
**Repo state:** on `master`, committed; working tree clean.
**Done this session:**
- Shift+Enter / Shift+click on a page row of Ctrl+K or Ctrl+H, and
  Shift+Enter in the address bar, open the page beside the current card with
  a trail edge (`App::open_beside_current`). Decisions under M5 (Palette →
  Open beside). Guide note and README mention it.
- Owner asked: Read Mode no longer cuts or squashes. Tab grows the card into
  the window and back (M11 step 6, now 🟡); page frames are drawn at the
  CSS size they were painted at (`PaintFrame::css`, `Shape::Page`).
- Owner asked: overlay scrollbars instead of the classic grey bar (M6
  settings → Scrollbars).
- Owner asked: a complete, professional welcome as a designed web page
  (`welcome.html` in `PageKind::App`; First-run defaults in M9).
**Owner decisions (2026-11-15):** no AppImage, Flatpak or AUR work (the owner
publishes later, on their own); no UI translations; licence is Apache-2.0
only (`LICENSE-APACHE`); all AI features (semantic search, Explain, any
other AI/ML), imports from other browsers, and password managers and
passkeys are all on hold until the owner says otherwise (see M10).
**Next up:** no M10 work: its items are on hold or wait for the owner to
pick them. M11 (premium look): step 4 is done; the other steps wait for
the owner to ask (corners stay Hyprland's, see M11). Take the small
follow-ups below; a good next one is auto-scrolling the sidebar while a
header is dragged, or keyboard selection of several cards.

After that, the other small follow-ups below. Sandbox follow-ups: verify the setuid path on a kernel without unprivileged user namespaces (needs `kernel.unprivileged_userns_clone=0` or linux-hardened, and the package installed). Consent follow-ups: more platforms as real sites show them (TCF
`__tcfapi` frames, Ketch, Termly, Borlabs, Axeptio); a separate *Reject cookie banners* switch
if clicking ever breaks a site. Small blocking follow-ups: scriptlets/`$redirect` resources
(need the `resource-assembler` feature and uBO's resources, check the
licence); serialize the
compiled engine if startup ever shows it; procedural cosmetic rules
(`:has-text()` and friends, run in the page); a separate *Hide cookie
banners* switch if one list ever breaks sites the other doesn't. Other small follow-ups: auto-scroll the sidebar while dragging; a downloads folder setting (needs a free-text row or a
folder picker via xdg-desktop-portal); a progress mark on the board while
downloads run; ask before quitting with downloads running; pause/resume;
more settings (start page for an empty board, custom
search URL, card size); thumbnails are taken on the main thread (55–85 ms each
in a debug build at 1.2 s after load; move the shrink/encode off-thread if
it shows in release); a thumbnail on history rows or the sidebar; Shift+Delete on a closed row could drop it from the stack;
keep closed cards across restarts (board v6?); a private space could look
different beyond its label
(an icon, a hatched board);
undo for forgetting a page; site
icons on history rows; prune `icons/` of sites no card uses; a letter tile for
sites without an icon; the name label for a stack could list its size; a zoom
badge on a zoomed card's title bar; Ctrl+scroll inside a page to zoom it;
keyboard selection of
several cards; link hints inside iframes; xdg-desktop-portal file chooser for
Save as Markdown; animate cards to their Tidy places; a sticky note's
strip is hard to hit when zoomed far out; open `crash.log` from the crash
toast or a palette command.
**Notes for the next session:**
- Welcome page verified e2e (`~/.cache/trailboard-e2e-guide2`, `run.sh 3`
  and `4`/`5`): first start adds it (card 2, 880×720), every section renders
  in Read Mode (`q1`–`q5`), "Open a page" opens the address bar (`k1`),
  closing and Ctrl+K "welcome" brings it back, a restart restores it as
  `trailboard:welcome` with no second first-run. Not verified: the light
  theme (the page has a `light` class), HiDPI, a live theme change, the
  other three buttons (same path), narrow-card layout (< 760 CSS px).
- Read Mode morph verified by frames (`~/.cache/trailboard-e2e-beside`,
  `script4`–`script7`; grim in a loop, `g/`): grows from the card, the
  page at its own scale, the board dims; shrink back clean. Frames from
  gpu-screen-recorder (CPU fallback) showed a dark band that grim's real
  frames don't: a recorder artefact, don't chase it. Not verified: HiDPI,
  light theme, switching read cards (a cut, by design).
- Overlay scrollbars verified e2e (`~/.cache/trailboard-e2e-scroll`,
  `script10`–`script12`, `DELAY=0`): a thin thumb while scrolling on a white
  page and on a dark note, none when idle. Not verified: the thumb on hover
  (the scripted pointer missed the card's edge), Read Mode, a real mouse.
- Open beside verified e2e (`~/.cache/trailboard-e2e-beside`; serve `site/`
  with `python3 -m http.server 8791 --bind 127.0.0.1 -d site`, then
  `run.sh N shot…`): palette Shift+Enter opens card 3 beside card 1 (edge
  1→3), Ctrl+L Shift+Enter card 4 beside 3, a Ctrl+H row for a closed page
  opens beside the current card, an `sclick` on a palette row opens beside
  (`s2.png`: new card focused, edge and trail tag drawn); a card row with
  Shift just goes to the card. Not verified: Read Mode (goes through
  `ensure_visible`, which reads the new card), a real keyboard's Shift.
- Trail renaming verified e2e (`~/.cache/trailboard-e2e-trailname`; start
  `python3 server.py 8781` there, copy `seed/` to `dataN`, `run.sh N
  shot…`): R on the "Async Rust" header and on a card row of the
  Typography trail renames them (toasts); End, R, Esc cancels on a lone
  note; one click collapses "Lisbon in May", a slow second expands, a
  double-click opens the box with the group still expanded (`d1`: box
  right of the sheet); `tagclick 5` opens it holding the name (`t1`),
  renaming changes the tag (`t2`); `tagclick 8` on a dot opens it empty;
  undo brings the old name back. Not verified: a real pointer's
  double-click and cursor over the tag (scripted only), a tag under
  another card, light theme, HiDPI.
- Allowed-sites list verified e2e (`~/.cache/trailboard-e2e-allowed`;
  `seed/` has `privacy.json` allowing 127.0.0.1:8771 and example.com;
  copy it to `dataN`, `run.sh N shot…`): the list logs both rows ("1
  card" on the first), Enter blocks 127.0.0.1:8771 again (1 page
  reloaded, toast), Shift+Delete example.com, the empty state reads "Ads
  are blocked on every site" (`al1`, `al3`), `privacy.json` ends empty;
  in a private space a site allowed shows "Until you quit · 1 card"
  (`al4`). The server on 8771 was another session's (a.html 404s), so
  the blocked count wasn't seen to come back. Not verified: a click on a
  row (same path as Enter), a list long enough to scroll, light theme.
- Hidden count verified e2e (`~/.cache/trailboard-e2e-hidden`; serve
  `site/` on 127.0.0.1:8771 and `frame/` on 127.0.0.1:8772 with
  `python3 -m http.server`; copy `seed/` to `dataN`; `run.sh N shot…`,
  `URL=… run.sh` for another page): h.html with only the user rules
  hides 5 elements plus 1 in its iframe → `cards` logs
  elements_hidden=6, matching the page's own H/S report; after the full
  lists load (they turn generic hiding off on that host) it's 1, the
  shield shows "1" with blocked=0 and its menu reads "Allow ads on
  127.0.0.1:8771 · 1 hidden" (`hid1-crop.png`). theguardian.com:
  "· 22 blocked · 2 hidden" (`real1-crop.png`). Not verified: the 50 ms
  give-up on a huge page (no page here was slow), a light theme, HiDPI.
- Sidebar verified e2e (`~/.cache/trailboard-e2e-sidebar2`; start the
  site with `python3 server.py 8781` there: every 127.0.0.N host is its
  own site, most with a drawn icon; copy `seed/` to `dataN`; `run2.sh N
  shot…`; `TRAILBOARD_OMARCHY_STATE=$PWD/light` gives a light theme):
  B opens it with the keys, j/k step, a row click goes to the card
  (`open2`, `hover2`, `dark4`, `light3` checked by eye, zoomed crops
  `z-top`, `z-foot`); pointer to x=1 peeks it (keys=false) and moving to
  x=900 closes it 400 ms later (`peek2`, `gone2`); no peek at start with
  the pointer on the edge; a chip click switches to that space and "+"
  makes one (`chip5`, also the empty state). Not verified: a list long
  enough to scroll (unit-tested: `keep_visible`, `max_scroll`), dragging a
  header with the new spacing (drop line uses the same `row_rect`), HiDPI,
  a private space's hollow chip, a real pointer at the edge (scripted moves
  only).
- Sidebar follow-ups: slide out on close (step 6); scroll by pixels for a
  smoother wheel; a close button on a card row on hover; drag a card
  between trails; a right-click menu on a trail's header (rename, colour,
  pin).
- Shield menu verified e2e (`~/.cache/trailboard-e2e-shieldmenu`, `run.sh
  N shot…`; serve `site/` with `python3 -m http.server 8771 --bind
  127.0.0.1 --directory site`; copy `seed/` to `dataN` first): blocked=2 →
  `shieldclick 1` logs "shield menu on card 1: [Allow ads on 127.0.0.1:8771
  · 2 blocked]" (`sm1.png`: the menu under the title bar, kept inside the
  window); Esc closes it; a click elsewhere on the title bar opens nothing;
  picking the entry toasts "Ads allowed", the page reloads as "ad+ok",
  blocked=0, and `shieldclick` then finds no shield. Not verified: the
  pointer cursor over the shield (no real pointer here), the "Cookie banner
  rejected" line in this menu (unit-tested; shared with the page menu,
  which was verified e2e), a light theme, HiDPI.
- user.txt reload verified e2e (`~/.cache/trailboard-e2e-userlist`,
  `run.sh N`, serve its `site/` on 127.0.0.1:8771; the script edits the
  file at `log edit …` marks): no file → blocked=0; writing a `/ads/` rule
  toasts "1 rule", Ctrl+R → blocked=2 with the shield (`ul1.png`); `touch`
  → nothing; a temp-file-and-`mv` save → "2 rules"; `rm` → "were removed"
  (`ul2.png`), Ctrl+R → the ad loads, blocked=0. Not verified: the
  "blocking is off" wording in the app (unit-tested only), a filters folder
  that is itself replaced or deleted while running (the watch would be lost
  until restart).
- Title-bar count verified e2e (`~/.cache/trailboard-e2e-badge`, `run.sh N
  shot…`; serve `site/` with `python3 -m http.server 8771 --bind
  127.0.0.1`; `seed/` has a `user.txt` rule blocking `/ads/`): card shows
  the shield and "2" at board zoom and after z (`badge1/2.png`); after
  *Allow or block ads* the reloaded page loads its ad and the mark is gone
  (`badge3.png`). Cookie line verified e2e
  (`~/.cache/trailboard-e2e-badgeconsent`, the consent site on
  127.0.0.2:8773): after "OneTrust rejected" the right-click menu shows
  "Cookie banner rejected" dimmed above "Allow ads on 127.0.0.2:8773"
  (`menu1.png`); after Ctrl+L to the 8771 page it is gone (`menu2.png`).
  The shield is hollow at zoom ≈ 0.9 (`badge2.png`) and filled at ≈ 0.57
  (`badge1.png`), as intended. Not verified: "99+", a light theme, a
  HiDPI screen.
- Sandbox row and notice verified e2e (`~/.cache/trailboard-e2e-sandboxui`,
  `run.sh N [nons]`; `nons` runs inside a user namespace that allows no
  further ones, as on linux-hardened). Run 1 (sandbox on): row 6 "Sandbox"
  "On · user namespaces", → does nothing, Enter explains, no notice, no mark.
  Runs 2a/2b (nons, same data folder): the first start toasts "Pages run
  without Chromium's sandbox …" and writes `sandbox-off-told`, the second
  doesn't; row reads "Off · not supported here" and its explanation shows
  below the panel (screenshot checked). Run 2c (same folder, sandbox on):
  mark cleared. Not verified: the setuid-helper wording on a real setuid
  install, the crash notice and sandbox notice on the same start (reasoned:
  the sandbox one waits), a light theme.
- Toasts are still hidden while other lists (palette, history, downloads)
  are open; only the settings panel shows them, below its list.
- Sandbox verified e2e (`~/.cache/trailboard-e2e-sandbox`, `run.sh N
  shot…` dumps every `trailboard` process's seccomp mode and user/PID
  namespace to `procsN.txt` at `log probe`; `URL=` sets the start page,
  default `chrome://sandbox`). Run 1–2: "sandbox: on (user namespaces)";
  the browser, utility and first zygote in the host namespaces, the forked
  zygote and every renderer in a new user namespace with their own PID
  namespaces and `Seccomp: 2`; Wikipedia loads; `chrome://sandbox` reads
  "You are adequately sandboxed" (only Yama non-broker ptrace is yellow, as
  in Chrome). Run 3, `TRAILBOARD_NO_SANDBOX=1`: warning logged, everything
  in the host namespace, seccomp 0. Run 4 (`run-nons.sh`: inside a user
  namespace with `max_user_namespaces=1`, so no nested ones, as on
  linux-hardened): the probe fails, "sandbox: OFF …" warning, the app
  starts and `chrome://sandbox` says "NOT adequately sandboxed" (without the
  probe Chromium would abort here). Run 5 (sandboxed, a copy of the
  cosmetic test's `data2` with all lists): the cosmetic test page
  `site:H cls:H id:H attr:H unh:S ctrl:S late:H ovf:auto frame:clsH/siteS`
  and the OneTrust consent page `out:reject ovf:auto`, same as before. Not
  verified: the setuid helper path (needs root-owned 4755 and userns off
  system-wide), Ubuntu's AppArmor restriction (reasoned, not run), memory
  against a no-sandbox baseline (renderers 60–175 MB RSS each, as before by
  eye), GPU acceleration differences (this iGPU logs a vaapi error either
  way), downloads and file chooser dialogs under the sandbox (they run in
  the browser process, so should be unaffected).

- Consent verified e2e (`~/.cache/trailboard-e2e-consent`, `run.sh N
  shot…`, `URL=` sets the start page; serve `site/` with `python3 -m
  http.server 8773 --bind 127.0.0.2` and again on 127.0.0.3:8774 for the
  Sourcepoint frame). Each test page (`ot`, `qc`, `dd`, `cky`, `uc`, `sp`,
  `kl`) shows its own made-up banner with the platform's ids after a delay,
  locks `body`, and titles itself `out:<button clicked> ovf:<overflow>` at
  4 s. Run 1 (fresh, user rule hiding `.cky-consent-container`, lists
  fetched 3 s in): OneTrust, Quantcast (by text, past "More options"),
  CookieYes (hidden by our rule, still answered), Usercentrics (shadow
  root) all `out:reject ovf:auto`; Didomi without a reject button: the
  cookie list hides `#didomi-notice`, so `no-button` then unlocked
  (`dd.png`: banner gone, page scrolls). Run 2: Sourcepoint rejected in
  its frame and the page unlocked once the container went; Klaro-like
  banner visible with no reject button: `no-button`, left locked
  (`ovf:hidden`), as intended. Run 3 (real sites): cookieyes.com's own
  banner rejected; didomi.io and usercentrics.com showed no banner from
  here (`dd3.png`, `uc3.png`), so those platforms are verified on test
  pages only. Not verified: EU-only banners on real sites (outside the EU
  here), a site that breaks after a reject click, very slow CMPs (>30 s),
  closed shadow roots.

- Cosmetic filtering verified e2e (`~/.cache/trailboard-e2e-cosmetic`,
  `run.sh N shot…`, `URL=` sets the start page; serve `site/` on
  127.0.0.1:8771 and 127.0.0.2:8773, `frame/` on localhost:8772 and
  127.0.0.3:8774, each with `python3 -m http.server P --bind ADDR`).
  `h.html`/`h2.html` title themselves with what is hidden (H) or shown (S):
  site rule, generic class, id, attribute selector, a site-unhidden class,
  a control, a class added 1 s late, `body:style(overflow)`, and a
  cross-origin iframe. Run 1 (user rules only, then all lists): all as
  expected with user rules; with EasyList loaded, 127.0.0.1 gets no generic
  rules (EasyList's own `@@://127.0.0.1$generichide`, so test on
  127.0.0.2); toggle off → all shown, back on → hidden. Run 2 (all lists,
  127.0.0.2): `site:H cls:H id:H attr:H unh:S ctrl:S late:H ovf:auto
  frame:clsH/siteS`; ~880 selectors per frame. Run 3 (`*-cmp.png`, on vs
  off): OneTrust's and Didomi's banners gone with blocking on, back with it
  off (Didomi's dimmed backdrop too); the box on cookiebot.com is their
  product demo, not a real banner, and shows either way; Stack Overflow
  shows no banner from here at all. OneTrust's was stopped by a network rule
  (`otBannerSdk.js`); run 4 let it load (`@@||cdn.cookielaw.org^` in
  `user.txt`) and the hiding rules removed it (`ot4.png`). Rules from all
  three lists build in 109–116 ms. Not verified: EU-only banners (this
  machine is outside the EU, so many sites never show one), pages left
  unscrollable after their banner is hidden, a flash of the banner before
  the generic rules arrive (screenshots are taken after it settles), memory
  with cosmetic rules loaded (not measured), very dynamic pages (the
  observer's cost on long feeds), the light theme (no UI change).
- Blocking verified e2e (`~/.cache/trailboard-e2e-block`, `run.sh N shot…`;
  serve `site/` with `python3 -m http.server 8771 --bind 127.0.0.1`;
  `a.html` loads `/ads/ad.js`, `/ok.js` and `/ads/pixel.png` and titles
  itself with the scripts that ran; `seed/` has `filters/user.txt` =
  `/ads/*$script,image`; `RUST_LOG=…,web::shield=debug` logs each block).
  Run 1 (fresh): title "ok", ad script and image blocked; EasyList (2.07 MB)
  and EasyPrivacy (1.52 MB) fetched from easylist.to 3 s in, rules from all
  three in 78 ms; palette toggle → "ad+ok", blocked 0, `privacy.json` holds
  `127.0.0.1:8771`; page menu "Block ads on 127.0.0.1:8771" → "ok" again.
  Run 2 (copy): no fetch (fresh), allowed site restored; toggle on → both
  cards "ok", "2 blocked" each (first try showed "ad+ok": the memory-cache
  bug, fixed); menu screenshot `menu2.png` checked (dark theme); setting
  off → menu item gone, Ctrl+R → "ad+ok", `settings.json` `block_ads:
  false`. Run 3: Ctrl+Shift+P private, toggle → toast "…until you quit",
  `privacy.json` still empty; theguardian.com: 13 distinct requests blocked
  (its `ophan` analytics, Sourcepoint's consent script), page loaded and
  titled. RSS with both lists loaded: browser process ~340 MB, all
  Trailboard processes ~1.1 GB with 3 live pages (no baseline measured
  without lists). Not verified: an offline first start (should retry
  hourly, log a warning), a captive portal answer (unit-tested only), a
  list fetched through a proxy, YouTube-style sites that break with
  EasyList (that is what the per-site toggle is for), the light theme.
- Welcome note verified e2e (`~/.cache/trailboard-e2e-guide`, `run.sh N
  shot…`): run 1, fresh `data1` with a `file://` page: "first run: welcome
  note 2", cards at world (-1132,-360 560×720) and (-512,-360 1024×720),
  both on screen; `shot1.png` shows the guide fits without a scrollbar
  (dark theme, a 941 px wide tiled window). Run 2 (a copy of `data1`, same
  URL): 2 cards restored, no second guide, the URL as card 3. Run 3 (fresh,
  no URL): Wikipedia's random page plus the guide. Not verified: the light
  theme, a HiDPI or very small window (the fit zooms out to show both), the
  installed package's first start.
- Palette command verified (`script4`, `log4.txt`, `shot4.png`, a copy of
  `data1`): "welcome note 2 already on the board", Ctrl+W closed it (cards=1),
  the command again: "welcome note 3 added" in the view's middle, current.
- Polish verified with screenshots (`~/.cache/trailboard-e2e-polish`,
  `run.sh N shot…`, `START=file://…/a.html`; `runlight.sh` adds a fake light
  theme from `light/`). `*1.png` are the before shots, `*2–4.png` after:
  board, palette (empty and typed), settings, sidebar, sheet, menu, a
  zoomed card, an empty space, and the palette and empty board in the light
  theme. Not verified: the animations frame by frame (screenshots are taken
  after they settle), rounded corners (Hyprland rounding > 0) with the new
  edges and sheen, HiDPI, private-space and warning (red-edged) toasts.
- PKGBUILD verified (`~/.cache/trailboard-aur`: a copy of the PKGBUILD,
  the CEF tarball symlinked from `~/.cache/cef-dl`, built with
  `TRAILBOARD_GIT=file:///home/shadow/Work/marble-linux makepkg -f`, log in
  `makepkg.log`). ~5 min: release build 3m57, `check()` 126 + 58 tests and
  the version line. Package 158 MB `.pkg.tar.zst`, 377 MB installed;
  runpath `[$ORIGIN]` only; libcef.so stripped (278 MB). E2e
  (`~/.cache/trailboard-e2e-aur`, `log1.txt`): run through a symlink to
  `pkg/…/usr/lib/trailboard/trailboard` under `unshare -rm` with a tmpfs
  over `~/.local/share/cef` and no `CEF_PATH`: CEF 156 loaded, the file card
  live and titled, `quit` clean. Not verified: `pacman -U` itself (not
  installed system-wide; the `/usr/bin` link and desktop entry inside the
  package were checked by listing), `namcap` (not installed), a clean chroot
  build (`extra-x86_64-build`), whether `depends` is complete on a minimal
  system, and a fresh CEF download (the tarball came from the cache, its sha1
  matched CEF's index).
- Packaging verified (`~/.cache/trailboard-e2e-bundle`: `run/` is a copy of
  the bundle, `data/` the isolated `XDG_DATA_HOME`, `s1.txt` ends in
  `panic`). `scripts/bundle.sh --tar` took 1m45 (release build from a warm
  cache). Run 1 under `unshare -rm` with a tmpfs over `~/.local/share/cef`, so
  only the bundle's CEF could load: page card live, titled, then `panic` →
  exit 101, report with a symbolized backtrace in `crash.log`, `crash-unseen`
  left; no leftover processes. (Under the user namespace Chromium logs many
  D-Bus errors; harmless there.) Run 2: board restored, toast logged, mark
  cleared. Run 4 (debug build, mark touched): screenshot `shot4.png` shows the
  toast at the top, fitting a 941 px window, dark theme only. Not verified:
  the bundle on another machine or distro (missing system libraries such as
  nss/gtk3 would show as a loader error), a panic inside a CEF callback or a
  renderer helper (the hook is installed before `web::start`, so it should
  run, then the FFI boundary aborts), `--tar` extraction elsewhere.
- Scrolling verified e2e (`~/.cache/trailboard-e2e-scroll`, `run.sh N`;
  `long.html` puts `ms:scrollY` since its first wheel event in its title).
  One notch: n=1 event, 0→120 px over ~205 ms (Chromium's glide). Flick of 6
  × 8 surface px: each 32 px applied within ~17 ms, then a fling to ~930
  px, done at ~1.2 s. Not verified: a real touchpad or mouse (scripted
  deltas only; winit on Hyprland gives wheels as `LineDelta`, touchpads as
  `PixelDelta` with `Ended` on lift, read from winit/sctk source),
  horizontal scrolling, pages that scroll an inner element, Read Mode.
- Sidebar pins/order verified e2e (`~/.cache/trailboard-e2e-sidebar`,
  `run.sh N shot…`; `data5` is `seed` (a v5 board) plus an edge 5→6, so
  trails Reading (1–4), P5 (5–6), sticky 7 alone). Run 5: B, End, p → "Buy
  milk" pinned on top with ★ and a line under it, toast; `press` on P5's
  header, `move` onto Reading's → accent line above Reading (screenshot),
  `release` → P5 place 0, Reading 1; a click on Reading's header collapsed
  it (no move); Shift+K → Reading first; Shift+K again no change; Ctrl+Z
  undid the last move. Run 6 (copy): order and pin restored, file version 6
  with `pinned`/`place` on every card of each trail. Screenshots pin5,
  dragline5, moved5, restored6 checked, dark theme only. Not verified: a
  real mouse drag (scripted press/move/release only), a drop while the list
  is scrolled, light themes, the ★ glyph with other system fonts (falls back
  through fontdb).
- Downloads verified e2e (`~/.cache/trailboard-e2e-downloads`, `run.sh N
  shot…` sets `TRAILBOARD_DOWNLOAD_DIR=dlN` and `TRAILBOARD_OPENER=opener.sh`
  (logs to `opened.txt`); `python3 server.py` serves :8761: `/` links at
  y=40/100/160/220 CSS px (attachment report.pdf beside, the same with
  `download`, a 2 MB trickle over 8 s, a transfer cut at 1 KB) and an SVG at
  y=300). Run 1: report.pdf via a beside card (card closed, page parked,
  released when done); `download` attr → "report (1).pdf" from card 1; *Save
  image* → img.svg; big.bin showed "650 KB of 2.0 MB · 32%" then done; Enter
  opened img.svg, Ctrl+Enter its folder; Shift+Delete removed a row; Ctrl+J
  toggled; *Downloads* in Ctrl+K. Run 1 found Chromium re-asking for a
  resumed id (fixed; run 2: one half.zip, Failed after 5 tries). Run 2:
  big.bin finished while Space 2 was on screen (parked page). Run 3: quit at
  15% → no file left; run 4 (copy) listed it "Stopped…", Enter fetched it
  again through a new card that closed itself. Run 5: Shift+Delete cancelled
  a running one (file gone), view stayed on the source card; a private
  space's download listed but not in `downloads.json`. Screenshots dl1, dl5
  checked, dark theme only. Not verified: real sites (redirects, big files,
  `blob:`/`data:` downloads, PDFs, which Chromium shows rather than
  downloads), `xdg-open` itself (a stub opener was used), a non-default
  `user-dirs.dirs` (unit-tested), a card put to sleep mid-download (same
  `release` path as close and space switch), disk-full errors.
- Settings verified e2e (`~/.cache/trailboard-e2e-settings`, `run.sh N
  shot…`, `ENVX=VAR=val` passes an env var, `NOURL=1`; serve `site/` on :8751;
  `a.html` titles itself with the `prefers-color-scheme` it sees). Run 1: four
  cards; Ctrl+, listed the defaults; → Google, ← ← live 3 (card 1 then slept,
  "3 live"), Enter history off, → page colours Light ("pages: light");
  `settings.json` held all four; Ctrl+K "rust lang" → "Search Google for …"
  with a google.com URL; *Settings* from Ctrl+K reopened it; a page opened
  after that was not recorded (history still 4). Run 2 (copy, `NOURL=1`,
  `TRAILBOARD_MAX_LIVE=5`): settings restored, the woken a.html saw "light";
  live row said "5 · TRAILBOARD_MAX_LIVE"; → Dark, → Follow the theme: page
  saw "dark" live. Run 3: two clicks on the search row → Bing → Brave
  Search, panel stayed open. Screenshots panel2, dark2 checked, dark theme
  only. Not verified: right-click stepping back (scripts can't right-click
  the board), Ctrl+, typed by a real keyboard inside a page (scripted keys
  only), a light Omarchy theme with *Page colours* Dark, the search engines'
  real result pages (URLs unit-tested only).
- Hand-off verified e2e (`~/.cache/trailboard-e2e-desktop`, `site/a.html`,
  `b.html`; `inst/` has the entry installed by the script for a copy of the
  binary named `trail$board` in `bin dir/`, to test Exec quoting). With one
  instance on `data/`: `cd site; trailboard b.html example.com` → exit 0,
  "opened in the running Trailboard", two cards; no arguments → exit 0, a
  focus request only; `gio launch` of the installed entry with a path → a
  card. The board then held 4 cards. Screenshot relaunch checked. Not
  verified: the Omarchy launcher itself (walker) and a real link click from
  another app, whether Hyprland raises the window (`focus_window` sent;
  without an activation token it may only mark it urgent), the icon at
  16–32 px.
- Thumbnails verified e2e (`~/.cache/trailboard-e2e-thumbs`, `run.sh N
  shot…`, `MAXLIVE=` sets `TRAILBOARD_MAX_LIVE`; serve `site/` on :8741).
  Run 1: a, b, c and c#view each got a 480×338 thumb (3.4–4.6 KB); closed
  c; Ctrl+H previewed Glacier View, Down → "Loading app"; Ctrl+K "glacier"
  previewed the closed card. The window was tiled 941 px wide, so the
  preview sat under the list (first version needed room on the right and
  showed nothing). Run 2 (copy, `NOURL=1 MAXLIVE=1`): both restored cards
  got their thumbs; the sleeping one showed it dimmed. Screenshots hist1,
  restored2 checked, dark theme only. Not verified: the right-of-list
  placement in a wide window (unit-tested), real sites (heavy pages, pages
  still blank at 1.2 s get a blank thumb), HiDPI, light themes.
- Closed cards verified e2e (`~/.cache/trailboard-e2e-closed`, `run.sh 1
  empty1 ocean1 end1`; serve its `site/` on :8731). Opened b and c beside a,
  closed both by the *Close card* command: the empty palette listed Glacier
  View then Ocean Facts as "Closed · 127.0.0.1:8731" after the open card;
  "ocean" ranked the closed card first; "whale" (only in the page text) found
  the closed card, not a History row; Enter reopened card 2 at its old rect;
  Ctrl+Shift+T then reopened card 3, not 2 again; nothing closed was left.
  Screenshots empty1 and ocean1 checked, dark theme only. Not verified: a
  closed note or sticky (unit-tested only), a click on a closed row, other
  spaces' stacks (per-space `Session.closed`, unchanged).
- Private spaces verified e2e (`~/.cache/trailboard-e2e-private`, `run.sh N
  shot…`; serve `site/` on :8721 (`cookie.html` shows the cookie it found,
  then sets one) and `priv/` on :8722 (`index.html`, "marmalade" text, a red
  icon) with `python3 -m http.server`). Run 1: cookie page recorded in space
  1; in a new space it saw `seen=1` (shared profile); Ctrl+Shift+P reopened it
  as "Cookie none" (fresh jar), a second private card saw `seen=1` (same jar);
  the :8722 page, its icon and a Ctrl+= zoom left no trace: not in history,
  no `icons/`, no `zoom.json`, no "marmalade" in any data file but the
  board. Run 2 (copy of run 1, `NOURL=1`): restored still private with a new
  jar; Ctrl+Shift+P off → label lost "· Private", the marmalade page was then
  recorded and its icon saved (first version skipped the save because the
  in-memory icon matched; `SiteIcon::saved` fixes that). Screenshots
  private1, restored2, off2 checked, dark theme only.
- Not verified: real sign-ins (only a script cookie; localStorage/IndexedDB
  in the jar assumed from CEF's incognito behaviour), popups opened by a
  private page (they become cards via `Page::new`, so they get the jar, but
  untested), the jar-creation failure path, light themes.
- History view verified e2e (same harness, `~/.cache/trailboard-e2e-history`,
  server on :8711; data3/data4 are copies of data2). Run 3 (`NOURL=1`): Ctrl+H
  listed 4 pages newest first ("On the board" on the 2 open cards, times
  local); "glac" → Glacier View, Shift+Delete forgot it; *Clear history* asked,
  Esc kept 3 pages, then Enter cleared them; the view said "No pages in
  history"; the db file then held none of the pages' words. Run 4: Enter on a
  history row opened it in a new card; a click on an "On the board" row went
  to that card; Shift+Delete in Ctrl+K on a text-only match forgot it.
  Screenshots hist3, confirm3, empty3 checked, dark theme only.
- Not verified: a big history (500 rows, scrolling, speed of `browse` on the
  main thread), light themes, DST edges of the local time (chrono handles the
  offset per page), the `History is off` state.
- History verified e2e (`~/.cache/trailboard-e2e-history`, `run.sh N shot…`;
  serve `site/` with `python3 -m http.server 8711 --bind 127.0.0.1`: `a.html`
  gardening, `b.html` ocean, `c.html` retitles itself and pushes `#view`
  after 2.5 s). Run 1: a, b, c and c#view recorded ("Loading app" then
  "Glacier View"), a `data:` page not; `history compost/whale/glacier` found
  the right pages, `zebra` (the data page) nothing; after undoing the three
  opens, palette "whal" → "Ocean Facts · History · 127.0.0.1:8711", Enter
  reopened it as a card. Run 2 (copy of run 1, `NOURL=1`): 4 pages still
  there; "tomato comp" → the open "Gardening Tips" card (matched only by its
  text). Screenshots palette1, palette2 checked, dark theme only.
- Not verified: real sites (large pages, many visits, the 5000-page prune
  outside unit tests), iframes' text (only the main frame is read), pages
  that never finish loading, how long the palette's search takes on a big
  index (cached per query, run on the main thread), light themes.
- Icons verified e2e (`~/.cache/trailboard-e2e-icons`, `run.sh N shot…`,
  `NOURL=1` restores the board without opening a URL). Serve `a/` on
  127.0.0.1:8701 (`<link rel=icon>` to a 32 px transparent red circle) and `b/`
  on :8702 (only `/favicon.ico`, green 16 px) with `python3 -m http.server`.
  Run 1: both icons fetched and saved (`icon for 127.0.0.1:8701: 32x32`, …),
  drawn on both cards; zoomed out with no current card → no label; l → label
  for card 2. Run 2 (copy of run 1): icons loaded from disk under the right
  sites (a first version lost the `:port`; the site is now inside the file),
  identical re-fetches not re-saved; pointer on card 1 → its label, pointer on
  the board → none. Run 3 (copy of run 2, :8702 stopped): the error page's
  card still shows the saved green icon beside its title. Screenshots
  titles1, current1, hover2, near3 checked (transparency right), dark theme only.
- Not verified: real sites (only local servers; SVG and multi-size `.ico`
  icons untested), HiDPI sharpness, light themes for the label, a real mouse
  leaving the window (CursorLeft clears the hover), a site whose icon changes
  (it replaces the saved one; untested), the label over the sidebar.
- Stop a test server by port (`ss -ltnp 'sport = :8702'`), never `pkill -f`,
  which killed this session's own shell.
- Keyboard parity verified e2e (`~/.cache/trailboard-e2e-kbd`, `run.sh N
  shot…` opens `controls.html`: a link, a button, a checkbox, a text field, a
  select, a `javascript:` link, a disabled button, each setting the title).
  Run 1: f labelled 6 (A link, S button, D checkbox, F field, G select, H
  script link; none on the disabled one). Run 2: H → title "js link"; F →
  keys to the page, `hello` typed; Menu → Undo/Redo/Cut/Copy/Paste/Select
  all at the field, Undo highlighted; d → "scroll 360", u → "scroll 0"; A
  opened card 2 beside; h v l v → both selected; z filled the view with 2;
  Shift+F10 with the board's keys → Back/Forward/Reload/Select all, k Enter
  ran Select all (which gives the page the keys, so the run's final q went to
  the page). Run 3: S → "count 1", D → "checked true", G then Down Enter →
  "picked two". Screenshots hints1, menu2 checked, dark theme only.
- `run.sh`'s `grim` region caught the neighbouring tiled window too; the
  Trailboard part was correct. Use the client's own geometry when it's tiled.
- Not verified: hints on a note's task boxes, contenteditable and ARIA-role
  targets (in the script, untested), Menu on a selection in the page body or
  on a focused link, a real keyboard's Menu key, light themes.
- Find and zoom verified e2e (`~/.cache/trailboard-e2e-findzoom`, `run.sh N
  shot…` opens `find.html`: "apple" ×4 incl. "Apple" and "pineapple", two
  links, its layout width). Run 1: Ctrl+F, `apple` → 4 matches highlighted;
  Enter → 1 of 4, 2 of 4, Shift+Enter → 1 of 4; `applex` → No matches; Esc
  closed and stopped; Ctrl+F reopened with `apple`; Ctrl+= ×2 → 125% (Chromium
  factor / scale 1.250), f hints sat on the zoomed links; Ctrl+- → 110%, and it
  stayed 110% after the raster scale went 1 → 0.75; a `data:` card opened
  beside stayed 100% (other site); `zoom.json` held `file: 1.1`. Run 2 (copy
  of run 1): restored page laid out at 931 px (1024 / 1.1), toast
  "Local files · 125%", Ctrl+0 → 100% and the entry removed. Screenshots
  find1, zoom1, hints1, toast2 checked, dark theme only.
- Not verified: a real keyboard (Ctrl++ on US layouts is Ctrl+Shift+=, which
  is accepted, but untested on others), light themes for the count, two
  same-site cards zoomed together on screen (both follow Chromium's per-host
  zoom; only one live file card was checked), find in a note or in Read Mode,
  a page that navigates while the bar is open, the "N matches" label on screen
  (logged only).
- Palette verified e2e (`~/.cache/trailboard-e2e-palette`, `run.sh N shot…`,
  seed = the sidebar run's seed in `seed/`). Run 1: Ctrl+K listed 7 cards then
  commands; `milk` → the sticky (letters highlighted), Enter went to it;
  `p3`, ↓↓↑ kept row 0, Enter → card 3; `tidy` Enter tidied; `fold` Enter
  folded the trail; `p2` found the folded card, Enter showed it at the
  front; `example.com` → *Open example.com* first; Ctrl+K again closed;
  `new space` Enter made Space 2; `space 1` found it, Enter switched back;
  keys in P5's page, Ctrl+K opened, Esc closed with the keys still in the
  page; `help` Enter showed the sheet. Run 2: `help` no longer matches file
  URLs; wheel scrolled 3 rows, hover moved the highlight, a click on a row
  went to P1; Ctrl+N N P → row 1; a click outside closed it; in Read Mode,
  `p1` Enter left it and went to P1. Screenshots empty1, milk1, folded1,
  wheel2, read2 checked, dark theme only.
- Not verified: light themes for the list, a real keyboard or mouse, IME
  typing into the palette (shared with the address bar's path), HiDPI scale,
  a board with hundreds of cards (rows are rebuilt on every key, draw, hover
  and wheel event; fine for small boards).
- Sidebar verified e2e (`~/.cache/trailboard-e2e-sidebar`, `run.sh N shot…`,
  seed in `seed/`: trail Reading 1→2, 1→3, 3→4 (green), P5, note 6, green
  sticky 7). Run 1: B with nothing focused highlighted card 7 (the middle of
  the view); h collapsed Other cards, l expanded; End → 7; k×4 → card 4
  with the camera putting it at x 385 (panel ends at 300); S (passed through
  to the board) folded the trail, rows 1–3 dimmed; Esc closed, B reopened on
  4, Esc closed. Run 2 (clicks): row P3 → card 3, S folded with 3 in front;
  click on folded P1 → stack front = 1; header click collapsed, again
  expanded; click on the board → keys=false, panel open; B → keys, B →
  closed. Run 3: j from P3 onto folded P4 → front 4; Enter → keys_in_page,
  j then went to the page. Screenshots open1, folded1, enter3 checked, dark
  theme only, 941 px wide window.
- Not verified: the wheel over the panel and a list longer than the window
  (scroll bar; unit-tested only), light themes, a real keyboard or mouse,
  hover rows with a real pointer, HiDPI scale.
- Sticky notes verified e2e (`~/.cache/trailboard-e2e-sticky`, `run.sh N
  shot…`, seed = the arrange run's seed in `seed/`). Run 1: Shift+N made
  sticky 7 (yellow), typed `# Buy *milk*` / `- [ ] eggs` (title "Buy milk");
  C C → green → cyan, undo → green; Esc, C on the sticky in the middle → cyan,
  undo; note 6's menu showed *Make sticky note*, which made it yellow, undo
  made it a card again. Run 2 (copy of run 1): sticky restored green after
  restart, file version 5. Run 3: a click 7 px above the strip hit the board;
  dragging the strip up 200 px moved it 272 units at zoom 0.735, undo put it
  back. Screenshots sticky1, cyan1, sticky2, zoom2, drag3, sheet1 checked,
  dark theme only.
- Not verified: light themes on screen (contrast unit-tested only), a live
  theme switch recolouring a sticky, a real keyboard Shift+N/C, a sticky
  note in a stack or in Read Mode, resizing one by its corner by mouse.
- Type into a new note only after its page has loaded (script: `wait 1500`
  after N or Shift+N); keys sent earlier are lost.
- Arrange verified e2e (`~/.cache/trailboard-e2e-arrange`, `run.sh N shot…`,
  seed board in `seed/`, copy it to `dataN` first). Run 1 at zoom ≈ 0.32:
  j → card 3, M, l l moved it 252 (2 × 40 px); Shift+L, Shift+J grew it
  126 each, Ctrl+J moved 25 (8 px), Enter; undo restored it in one step, redo
  re-applied; M h h Esc put it back; M l then a board click kept it; Ctrl+A,
  M, k, Enter moved all six, one undo put all back; Shift+H × 8 stopped at
  the 240 minimum. Screenshots arranging1 (heavy ring, card on top, key pill)
  and sheet1 checked, dark theme only.
- Not verified: a real keyboard (script keys only), a folded stack moved by
  keyboard (its front moves like a drag; fold tests cover a moved front),
  light themes for the pill, M in Read Mode (ignored by design), the camera
  following a card moved far off screen.
- Link hints verified e2e (`~/.cache/trailboard-e2e-hints`, `run.sh N shot…`
  opens `links.html`: 12 links plus a hidden, a covered, a `javascript:` and a
  below-the-fold one). Run 1: f labelled exactly the 12 (S…L, AA AS AD AF);
  `a` narrowed to the four A labels (A dimmed); `s` opened link 10 beside as
  card 2 with edge 1 → 2, card 2 current; f on the link-less target page →
  "No links in view"; h back, f, `z` ignored, Backspace, Esc cancelled;
  Shift+F fit. Run 2: f then `d` opened link 2. Screenshots hints1, hints2,
  none1, zoom2 checked, dark theme only.
- Not verified: a zoomed page (Ctrl+= doesn't zoom pages yet, so the width
  scaling is untested), a real keyboard (script keys only), light themes for
  the labels, hints on a note's rendered links, a card woken by f
  (asleep → loading → probably "No links yet"), crowded pages (labels overlap).
- Keyboard verified e2e (`~/.cache/trailboard-e2e-keys`, same `run.sh`;
  seed = the export run's seed). Run 1: after Fit, j made card 3 current, l
  → 4, h → 3; N + `abc`, Esc (keys back, note 6 still current), Enter, ` def`
  → note text `abc def` (editor reopened at its end); k moved to 4, Esc let
  go; Shift+L panned, = = zoomed (0.414 → 0.647); F, l (picked 3), Delete
  closed it, undo brought it back; F1 sheet; Q quit. Screenshots current1
  (thin ring on P3) and sheet1 checked, dark theme only.
- Not verified: a real keyboard (script keys only), non-US layouts for
  h/j/k/l and +/- (physical keys; `+` is Shift+= on US), light themes for
  the rings, arrow keys in Read Mode still reaching the page. Run 2: j then
  Enter on P3; `both2.png` compares the rings (current: 2 px accent at 60%;
  with the keys: 3.5 px solid).
- Save as Markdown verified e2e (`~/.cache/trailboard-e2e-export`, `run.sh N
  shot…` with `XDG_DOCUMENTS_DIR=$D/docs`; `DELAY=0.1` shoots sooner, for
  toasts). Run 1 (marked-trail board + P5): R `Research`, N with
  `# Trip ideas` / `- [ ] pack`, Esc, Ctrl+S wrote `Space 1.md` (trail
  nested, note section, Other pages); right-click on the note → *Save as
  Markdown* wrote `Trip ideas.md`; Esc, Ctrl+S overwrote `Space 1.md`. Run 2
  (restart): Ctrl+S wrote `Space 1 2.md`, toast seen on screen. Screenshots
  menu1, toast2 checked, dark theme only.
- Not verified in the app: a note on a trail or a folded trail in the board
  file (scene-tested only); the error toast for an unwritable Documents
  folder (not tested at all).
- Trail marks verified e2e (`~/.cache/trailboard-e2e-mark`, `run.sh N shot…`,
  seed in `data1.seed`, a v3 board so the migration ran). Run 1: R, typed
  `Research`, Enter, C twice (green) marked all four cards and labelled card 1;
  undo went back to blue, redo to green; fold moved the label to the stack
  front (3), unfold back to 1. Run 2 (copy of run 1's data): marks restored
  after restart, file is version 4; R showed the name selected, Backspace +
  Enter cleared the name and kept the colour. Screenshots named1, folded1,
  prompt2 checked, dark theme only.
- Not verified: light themes for the tag text, a real keyboard R/C, a
  coloured lone card on screen (scene-tested only), inheritance by a link
  opened beside in the app (scene-tested only).
- Notes verified e2e (`~/.cache/trailboard-e2e-note`, `run.sh N shot…` runs
  `scriptN` with `dataN`, `rund.sh` adds `web=debug`; `page.html` selects its
  first paragraph on load). Run 1: N, typed `# Trip ideas` / `- [ ] pack` /
  Enter continued the list; Esc rendered it; *Add to note* on the page
  appended the quote to it. Run 2 (copy of run 1's data): note restored and
  rendered after restart. Run 3: *Add to note* with no note made note 3 beside
  page 2 (edge 2 → 3); its `file:` link opened card 4 beside it (edge 3 → 4);
  a click on a task box saved `- [x] pack`. Run 4: https and file links in a
  typed note both open beside. Run 5: a 63,807-character note saved whole
  after an edit. Screenshots (edit1, view1, quoted1, reload2, beside3, task3)
  checked, dark theme only.
- Not verified: light themes in the editor, a live theme switch recolouring
  an open note, real keyboard/IME typing into a note (script keys only),
  *Add to note* while in Read Mode, a note in a stack's outline.
- ✅ Without a board and without a URL the app opens a start page as card 1:
  a blank card with one search box (`app::start`, `trailboard:start`).
  Enter (or Ctrl+L) loads the search or address in that card, which turns
  into an ordinary page (2026-11-15; was a random Wikipedia page). Verified
  by a scripted run (type + Enter → DuckDuckGo results in card 1) and a
  screenshot.
- ✅ Start page engine picker and search engine guide (`app::engines`,
  `trailboard:engines`): 15 engines in three groups, chosen on the start
  page or the guide and saved like the Settings choice; live start pages
  and the guide follow a change (`tbEngine`). Badges show each engine's own
  icon, loaded from its site (Google's favicon service for Brave, Startpage,
  WolframAlpha and Google, whose own icon addresses change per release),
  over a coloured monogram that stays when the icon can't load. Gibiru and
  Oscobo only publish 16 px icons, so theirs look soft.
- ✅ *New start page* in the Ctrl+K palette (`Command::StartPage`) adds a
  start page card in the middle of the view, taking the keys. Verified by
  a scripted run (Ctrl+K, "new start", Enter → a second start page card).
- ✅ Setting *New card (Ctrl+T) opens*: address bar (default) or start page
  (`prefs::NewCard`, `settings.json` `new_card`). Verified by scripted runs
  in both modes (start page → a new start card; address bar → the floating
  bar, screenshot) and a screenshot of the settings panel. Verified by a scripted run
  (picker → Brave saved; help → guide beside the start card) and
  screenshots. Not verified: the light theme, each engine's results page.
- A note's last keystroke can be lost if its card is closed or put to sleep
  within the same frame (the text is sent on every input, not on close).
- Trail steps verified e2e (`~/.cache/trailboard-e2e-trail`). Not verified: a
  real keyboard Alt+PgDn, the lone-card toast in the app (scene-tested only).
- Outline verified e2e (`~/.cache/trailboard-e2e-outline`); not verified there:
  a real mouse click on the badge, press-drag-release onto a row, light themes.
- Fold verified e2e (`~/.cache/trailboard-e2e-fold`). Not verified: real
  keyboard S/[/] (`[`/`]` on non-US layouts use the physical key), dragging a
  stack by hand, light themes for the stack edges, closing a stack's front in
  the app (scene-tested only).
- Known rough edge: undoing the close of a stack member brings it back as a lone
  card at the stack's place, on top of the stack (it is not put back in the
  stack).
- Keep the sheet's rows in `hints::SECTIONS` in step with new shortcuts.
- Read Mode: a stale full-window frame can show stretched for a moment after
  leaving; the read card never sleeps.
- A sleeping card's last frame is dropped on switch, so after switching back
  cards beyond the live cap show the plain "asleep" placeholder until woken.
- Live-browser cap: zooming can churn (sleep, then reload in view).
- Keep test boards small (RAM). Use a fresh `XDG_DATA_HOME` under `~/.cache` for
  each e2e run; seeding a board file and running without a URL is easiest.
- Put a `wait` after each `log shot …` line, or the screenshot catches the
  next step. `hyprctl clients -j` can report a window off-screen right after it
  opens; the first shot needs ~5 s.
- Script `open` with a data: URL: percent-encode spaces and `#`; a `file://`
  test page is easier. While `TRAILBOARD_SCRIPT` runs, the window's IME stays off.

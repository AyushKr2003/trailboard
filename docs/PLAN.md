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
- ✅ Schema migrations: `spaces.json` is upgraded step by step on read
  (`storage::spaces::migrate`, v1 fixed sRGB `background` → v2 `tint`; the
  upgrade is written back; unknown/newer versions are refused, not touched).
  Boards the same way (`storage::migrate_board`): v1 → v2 adds `scene.stacks`
  (Fold), v2 → v3 adds `node.note` (Notes), written back on the next save.

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
  - Not done: renaming from the page menu or by clicking the tag; the trails
    sidebar (M6) will list names.
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

### M6: Chrome UI 🟡
- 🟡 Trails sidebar (B; `scene::sidebar`, `app::sidebar`). Decisions (2026-11-14):
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
  - Not done: pins/favourites, drag to reorder (both need a stored order, a
    board format change), rename from the sidebar, a minimum pixel step for
    touchpad scrolling (small pixel deltas round to no scroll).
- Ctrl+K palette: fuzzy search over open cards, history and commands.
- Find in page (Ctrl+F) bar; per-page zoom (Ctrl +/−/0 inside a page).
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

**Last session:** 2026-11-14. M6 Trails sidebar (first cut).
**Repo state:** on `master`, committed after 931c2e9; working tree clean.
**Done this session:**
- `scene::sidebar`: `groups`, `Row`/`RowKey`/`Group`, `Sidebar` (rows with
  collapse, highlight by key, step with fallback for a removed row,
  collapse/expand/toggle, scroll_to/scroll_by). Unit tests for all of it.
- `app::sidebar`: `Panel` layout and hit testing (tested), `layer` drawing.
  `App`: B toggles, sidebar keys before board keys, clicks, hover, wheel,
  `ensure_visible` keeps a card clear of the panel, state reset on a space
  switch. Script command `sidebar` logs rows and where to click them.
  Sheet, README, CHANGELOG updated.
**Next up:** the Ctrl+K palette (M6): fuzzy search over open cards and
commands (history needs M7; leave that part for later).
Small follow-ups: sidebar pins/favourites and drag to reorder (need a stored
order, board v6); rename a trail from the sidebar or its tag; the Menu key /
Shift+F10 for the page menu; keyboard selection of several cards; link hints
inside iframes and for buttons/inputs; Omarchy app conventions still missing:
a `.desktop` entry and icon, xdg-desktop-portal file chooser for Save as
Markdown, i18n. Also: animate cards to their Tidy places; a sticky note's
strip is hard to hit when zoomed far out.
**Notes for the next session:**
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
- Without a board and without a URL the app opens a random Wikipedia page as
  card 1 (network); seed a board when that matters.
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

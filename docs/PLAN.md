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
  page may lay out once at a stale zoom. User page zoom (Ctrl +/−, M6) is
  multiplied in: Chromium gets `scale × zoom`, the view stays `size × scale`.
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
  (`storage::spaces::migrate`, v1 fixed sRGB `background` → v2 `tint` → v3
  `private`; the
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
  - Not done: opening a card *beside* the current one from
    the palette (Shift+Enter?), recently closed cards as results.
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

### M7: History and search 🟡
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
- ⬜ Thumbnails (`thumbs` table, a small JPEG per page).
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

**Last session:** 2026-11-14. M7 recently closed cards in the palette.
**Repo state:** on `master`, committed after 52dea2e; working tree clean.
**Done this session:**
- Ctrl+K finds cards closed this session in the current space ("Closed ·
  host"), the newest 3 listed with nothing typed; Enter brings one back where
  it was. History pages a closed card showed resolve to that card. See M7.
**Next up:** M7's thumbnails (the last M7 ⬜), or M6's remaining ⬜
(`.desktop` entry and icon, settings, downloads).
Small follow-ups: Shift+Delete on a closed row could drop it from the stack;
keep closed cards across restarts (board v6?); a private space could look
different beyond its label
(an icon, a hatched board); downloads in a private space (none yet anyway);
undo for forgetting a page; a pause-history toggle; site
icons on history rows; prune `icons/` of sites no card uses; a letter tile for
sites without an icon; the name label for a stack could list its size; a zoom
badge on a zoomed card's title bar; Ctrl+scroll inside a page to zoom it;
Shift+Enter in the palette to open beside; sidebar pins and drag to reorder
(board v6); rename a trail from the sidebar or its tag; keyboard selection of
several cards; link hints inside iframes; xdg-desktop-portal file chooser for
Save as Markdown; i18n; animate cards to their Tidy places; a sticky note's
strip is hard to hit when zoomed far out.
**Notes for the next session:**
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

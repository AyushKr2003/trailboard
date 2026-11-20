# Trailboard: plan

Trailboard is a canvas browser for Linux. Web pages are live cards on an
infinite board; a clicked link opens as a new card beside its source, joined
by a trail edge. Every algorithm (Tidy, placement, Fold) is our own design.

Status: ✅ done · 🟡 partial · ⬜ not started · ⏸ on hold for the owner

This file was condensed on 2026-11-17. The full decision log for M1–M11
(every rule, number and verification note) is in git:
`git show 74e0f01:docs/PLAN.md`. Read it there before changing an older
feature's behaviour.

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
  (`web::links`, `web::text`, `web::cosmetic`…); the app only trusts the
  answer from the card it asked.

## 3. Milestones

| | Milestone | What it gave |
|---|---|---|
| ✅ | M1 Skeleton | Window, wgpu surface, dot grid, camera pan/zoom/fit |
| ✅ | M2 Live page cards | CEF off-screen pages as cards, input forwarding, IME, popups, context menu, live-browser cap, undo/redo |
| ✅ | M3 Many cards and trails | Links open beside their source, trail edges, placement, screen-shaped cards |
| ✅ | M4 Persistence and spaces | Board JSON with autosave and migrations, profile on disk, several spaces |
| ✅ | M5 Board tools | Solo (was Solo), Tidy, Fold, trail steps, trail names/colours, notes, sticky notes, Save as Markdown |
| ✅ | M6 Chrome UI | Sidebar, Ctrl+K palette, find, page zoom, keyboard-driven board, link hints, `?` sheet, Omarchy look, settings, start page, downloads |
| ✅ | M7 History and search | `page-text.db` (FTS5), thumbnails, history view, private spaces, recently closed |
| ✅ | M8 Privacy | adblock lists, cosmetic filtering, cookie-banner reject, YouTube ads, per-site toggle, browsing settings |
| ✅ | M9 Packaging | Bundle script, `--version`, crash reports, PKGBUILD, Chromium sandbox, first-run welcome |
| ⬜ | M10 Optional extras | See below; most items on hold |
| ✅ | M11 Premium look | Corners setting, cards as sheets, board backgrounds, sidebar, command bar, motion, empty board, review pass |
| 🟡 | **M12 Keyboard first** | Vim-like modes, page keys, scroll sections, command line, then (steps 7–17) cards from a page, search keywords, marks, user keys, settings, navigation, caret mode, dialogs (below) |
| ⬜ | **M13 Full media support** | Our own CEF build with H.264/AAC (and HEVC, Widevine if they work), built off this machine, opt-in (below) |

### Owner decisions that still bind

- Cards have no title bar; a card is its page, with a quiet label above it
  ([DESIGN.md](DESIGN.md)). New cards are screen-shaped.
- Corners: Rounded (default), Follow Hyprland or Custom. Board background:
  Colour mood, Photo or Plain.
- No AppImage, Flatpak or AUR work; no UI translations; licence Apache-2.0 only.
- ⏸ On hold until the owner says so (2026-11-15): all AI/ML features
  (semantic search, Explain, anything else), imports from other browsers,
  password managers and passkeys. Don't design or prototype them.
- ⬜ CEF extensions: only when the owner picks it ([FUTURE.md](FUTURE.md)).

### Keyboard model today (what M12 builds on)

- A card is *current* (board keys act on it) or *has the keys* (every key but
  app shortcuts goes to its page). `App::typing()` decides. Enter/Space or a
  click in the page gives the keys; Esc takes them back; Esc again lets go.
  Solo always gives the read card the keys.
- Board keys: h/j/k/l and arrows move between cards (`scene::nav`),
  Shift+h/j/k/l pan, +/- zoom, f link hints (open beside), Shift+F fit, Ctrl+d/u
  scroll the current page half a height (since M12 step 7; d/u close and reopen), v select,
  z fill, M arrange, N note, Shift+N sticky, T tidy, S fold, `[`/`]` stack,
  O outline, R/C trail name/colour, B sidebar, Tab Solo, Delete close,
  Q quit, `?` sheet. Ctrl+K palette, Ctrl+L address, Ctrl+F find, Ctrl+H
  history, Ctrl+J downloads, Ctrl+T/W new/close, Alt+arrows back/forward.
- Bare keys are never taken from a text field.

---

### M12: Keyboard first 🟡

Asked by the owner on 2026-11-17: make Trailboard fully keyboard-driven in a
vim-like way. Key choices follow vim's common conventions; command names,
the key file format and the docs are our own.

**Modes** (a small pill shows the mode and any half-typed keys, e.g.
`PAGE 5g…`):

| Mode | Keys go to | Enter by | Leave by |
|---|---|---|---|
| Board | Board keys (cards are like windows) | Esc from Page | — |
| Page | Our page keys first (scroll, hints, history, yank); unbound keys are dropped | Enter on a card, Solo | Esc → Board |
| Insert | Everything to the page (today's "has the keys") | A text field gets focus (click, hint, `i`, `gi`) | Esc → Page, field blurred |
| Passthrough | Everything, Esc included, for web apps with their own keys | Ctrl+V, or a per-site setting | Shift+Esc |
| Command | `:` opens the palette as a command line | `:` | Enter / Esc |

- Focus on page load does not switch to Insert; only focus the user causes.
- A page script reports text-field focus in and out (inputs, textareas,
  contenteditable, ARIA textbox, shadow DOM, iframes) as a `WebEvent`.

**Scroll sections.** Pages often have several scroll areas (sidebar, list,
content), areas inside areas, and sideways ones (carousels, wide tables).
Each card keeps a *scroll target*, separate from the page's DOM focus, so
typing in one field and scrolling another pane both work.

- A page script (`web::scroll`) finds every element that really scrolls
  (computed overflow, size > client size, visible, not tiny), walking
  shadow DOM, with its parent section, so nesting is known
  (page → pane → list). It reports each one's rect, axes and position; it
  scrolls one on command (by, to start, to end); it reports which section a
  click landed in.
- Choosing the target, in order:
  1. `s` labels every section (the page too, nested ones offset from their
     parent's label); typing a label picks it.
  2. `]s` / `[s` step through sections in reading order, outer before inner.
  3. `S` steps out to the enclosing section (list → pane → page).
  4. A click picks the innermost section under it; so does following a link
     inside a section.
  5. Otherwise automatic: the innermost section at the middle of the view
     (a modal or the main pane), else the largest, else the page.
- Keys on the target: j/k down/up, h/l left/right, d/u and Ctrl+d/u half,
  Ctrl+f/b or Space a full section, gg/G top/bottom, 0/$ left/right edge;
  counts work (`5j`).
- Sideways: if the target doesn't scroll sideways, h/l use the nearest
  enclosing section that does; with none, the pill says so.
- Chaining: at the target's edge the next press scrolls the enclosing
  section, like a wheel; `overscroll-behavior: contain` stops it. gg/G and
  0/$ never chain.
- A target scrolled out of view is brought back into view by its next key.
- Shown: a brief accent outline drawn by the app around the target, and the
  pill shows depth and position (`section 2.1 · 40%`).
- Scrolling is done in the page by default (exact, smooth, honours snap
  points). Cross-origin iframes and custom scrollers that don't move that
  way get a real wheel event at a point inside the section not covered by
  sticky elements. Rects scale with page zoom, as link hints do.
- The section list refreshes on `s`/`]s`/`[s` and when the target is gone; a
  navigation resets to automatic. Board d/u use the current card's target too.
- Pure logic (automatic pick, order, stepping out, chaining) in
  `scene::scrolltarget`, unit-tested; labels from `scene::hint::labels`.

**Steps** (one per session):

- ✅ 1. Key engine (2026-11-17; `scene::keys`, `KeyIn::chord`,
  `App::board_action`). Decisions:
  - Keys are written in vim-style notation (`gg`, `<C-d>`, `<S-Tab>`,
    `<lt>`). A bare character carries its Shift (`G`, `$`); with Ctrl/Alt a
    letter is lower case plus a Shift flag. Letters and digits come from the
    layout-aware key code (h/j/k/l work on any layout, as before); other
    characters from the typed text (`$`, `+`, `{`).
  - Counts: digits before a key, 0 only continues one, capped at 9999;
    actions that repeat (move, pan, zoom, d/u) run that many times, at most
    100; others ignore it. A lone count never times out (as in vim); a
    half-typed sequence runs its shorter binding or is dropped after 1 s.
  - Esc with something half-typed drops only that. A wrong bare key after a
    count is swallowed; a Ctrl/Alt chord goes on to the app's shortcuts. The
    half-typed keys show in a message at the bottom and are dropped by a
    click, a page taking the keys, or any overlay (palette, menu, hints…).
  - Board bindings are exactly the old keys (incl. Shift variants such as
    `{`/`}`, `_`, `T`, `S`); Esc and Tab stay outside the table (they switch
    modes and step 2 reworks them). The table has no sequences yet.
  - Verified e2e (`~/.cache/trailboard-e2e-m12`, `run.sh N shot…`):
    `2h`, `5`+Esc, a count dropped by Ctrl+K, Shift+Q not quitting, `2L`
    panning twice, a stray key after a count swallowed, Delete / Ctrl+Z /
    Ctrl+Shift+Z, f, Enter then `3j` going to the page, Esc then z; the
    pending pill screenshot (`pending.png`). Not verified: a real keyboard
    (script keys only), non-US layouts, light theme for the pill.
- 🟡 2. Modes (2026-11-17; `App::page_keys`, `PageKeys`, `web::focus`,
  `scene::keys::page`, `App::page_action`). Asked by the owner as "use the
  website from the keyboard in Solo", so the first page keys of step 4 came
  with it. Decisions:
  - Board = no card entered; an entered card (Enter, a click in it, Solo)
    is in Page, Insert or Passthrough (`entered()`, `page_mode()`,
    `typing()` = Insert/Passthrough). Leaving a card resets it to Page.
  - Starting mode: Page for web pages (Enter, typed address, palette,
    opened beside, Solo); Insert for notes, stickies and the start page,
    and for a field chosen with f. A click in a page is Insert at once (so
    typing right after a click lands) until the page's focus report says
    it wasn't a field.
  - `web::focus::script` runs in every frame of web and app pages at load
    start: focusin/focusout and every mousedown report "field or not" plus
    `navigator.userActivation.isActive`. Field + user gesture → Insert;
    not a field → Page (not for notes/start page, which type without a
    field we see). A frame whose focus is in a child frame stays quiet.
    Page autofocus on load has no gesture, so nothing changes. Only the
    entered card's reports count (any page can send one about itself).
  - Page mode: characters that aren't bindings do nothing; keys that type
    nothing (Enter, Tab, arrows, Space, PgUp/PgDn, Delete…) and Ctrl/Alt
    chords the app doesn't take go to the page (their releases too, tracked
    in `forwarded`), so Tab focus walking, Enter on a focused link, Space on
    a video and page shortcuts keep working. Tab still toggles Solo outside
    Insert. Ctrl+F stays Find, so Ctrl+F/B don't page (Space does).
  - Esc: in Insert the page sees it, then the field is blurred (`BLUR`,
    through shadow roots) → Page; in Passthrough it is the page's
    (Shift+Esc → Page); in Page it leaves Solo, else the card (→ Board).
  - The mode shows in the corner (bottom left, the message sheet) with any
    half-typed keys (`Page · 5g`); the board's own counts stay centred.
    IME is enabled only in Insert/Passthrough.
  - Interim scrolling (until step 3): a wheel over the page's middle — j/k
    60 CSS px, d/u half the view, gg/G/0/$ a jump of 10⁷ px.
  - Link hints are now drawn in Solo too (they were board-only).
  - Verified e2e (`~/.cache/trailboard-e2e-m12`, `script6`, `script7`):
    Enter → Page, Tab → Solo in Page, 5j/G/gg scroll (`sheet6.png`), gi →
    Insert and typing lands, Esc → Page, click in the field → Insert,
    click on text → Page, Ctrl+V → Passthrough, Esc kept, Shift+Esc → Page,
    yy toast, f in Solo draws labels (`hints_crop.png`), f s loads in the
    same card, H back, F d opens beside (edge, Solo follows), Esc → Board,
    an autofocusing page stays in Page. Board scripts 1 and 3 unchanged.
  - Not done: the per-site "start in Insert" setting. Not verified: a real
    keyboard, fields in iframes and shadow DOM, IME in Insert, the light
    theme for the mode pill, the sidebar open under the pill.
- ✅ 3. Scroll sections (2026-11-17; `web::scroll`, `scene::scrolltarget`,
  `App::got_scroll`). Decisions:
  - The page script keeps a numbered registry of sections per document
    (`0` = the page; weak refs), so the app names a target by number. Each
    command (`list`, `by`, `to`, `pick`) answers with one console message.
  - Done in the page, not in `scene` (it needs live positions, and a round
    trip per key would lag): the automatic pick (innermost section at the
    view's middle; if that is the page and the page doesn't scroll, the
    largest section; else the page), chaining at an edge (skipping outer
    sections that don't scroll that way; `overscroll-behavior` contain or
    none stops it), bringing an out-of-view target back (less than a
    quarter of it showing → `scrollIntoView`, which is that key's whole
    effect), and adding up quick smooth scrolls (each starts from where the
    last one is heading). `scene::scrolltarget` has the reading order,
    `]s`/`[s` (going round), `S`, pill names (`page`, `2`, `2.1`) and label
    spots (nested labels step down past their parent's), unit-tested.
  - A target that something else covers at its middle (a modal) gives way
    to the automatic pick while covered, so a click on the button that
    opened a dialog doesn't keep scrolling the pane behind it.
  - A click picks the innermost section under it; a click on the page itself
    (or a section number gone stale) means automatic again. Hint clicks
    pick too; following a link resets with the navigation (`UrlChanged`).
  - A section is an element with `overflow` auto/scroll/overlay and more
    than 8 px to scroll (smaller overflows are borders and rounding), at
    least 40×24 px in view, not a form field; or a frame at that size. Rects
    are clipped to the view and to the sections around them.
  - Frames: a same-site frame is scrolled by script (no chaining inside
    it); another site's frame, and a page where nothing scrolls up and down
    by script (custom scrollers), get a real wheel at a point inside the
    area not covered by a sticky bar. h/l with nothing sideways toast.
  - `s`, `]s`, `[s` and `S` ask for a fresh list; a scroll of a section not
    in the last list asks for one quietly, to name it in the pill. Smooth
    scrolls follow the motion setting (`still` → instant).
  - Full-section paging stays with the page's own Space / PgDn (Space is
    the page's since step 2, Ctrl+F is Find), so there is no Ctrl+F/B.
  - Verified e2e on `site/sec.html` (`~/.cache/trailboard-e2e-m12`,
    `script9`–`script12`): automatic pick of the main pane (`3j`), `gg`,
    labels (`labels.png`, `modallabels.png`), a nested list and its inner
    list, `S` to the pane and the page with the toast at the top, `G` then
    `j` chaining from the pane into the page, `]s` past hidden sections,
    the wide table's `$`/`0`/`2l`, the carousel stopping at its end
    (contain), a same-site frame by script, a file frame by wheel, an
    off-screen target brought back, a modal as the automatic pick and a
    list behind it giving way, the outline and pill (`flash.png`). Not
    verified: a real keyboard, RTL pages (negative `scrollLeft`), scroll
    snapping, shadow-DOM scrollers, a page zoomed away from 100 %, the
    light theme for the outline.
- ✅ 4. Page keys (2026-11-17; `App::load_here`, `App::open_clipboard`,
  `omnibox::Target::Beside`). H/L, r/R, `/` n/N, gi, yy/yt and o came with
  step 2; this step added ym, p/P and O. Decisions:
  - ym copies `[title](<url>)` through `scene::export::link` (the same
    escaping as Save as Markdown).
  - p/P take the clipboard's text with whitespace folded to single spaces
    and resolve it like the address bar: an address opens, other text is
    searched with the chosen engine. An empty clipboard toasts. "Here"
    follows the address bar's Enter: a web page navigates, a start page
    becomes the page, a note or the welcome page opens it beside. A count
    is ignored.
  - O opens an empty address bar (`Target::Beside`) whose Enter opens beside
    the card, joined by a trail edge, with the keys in Page mode; its hint
    says "Open beside". o keeps its address prefilled.
  - Verified e2e (`~/.cache/trailboard-e2e-m12`, `script13`): p loads the
    copied `file://` address in the same card, P opens it beside (edge
    3→4), O + typed address + Enter opens beside (edge 4→5) in Page mode,
    ym puts `[Page a](<file:///…/a.html>)` on the desktop clipboard
    (read back with `wl-paste`); the O bar (`obar.png`). Not verified: an
    empty clipboard's toast, p on a note or start page, a real keyboard.
- ✅ 5. Hint kinds with `;` (2026-11-17; `scene::hint::Kind`,
  `web::links::Filter`, `App::ask_link_hints`, `App::use_hint`). Decisions:
  - Keys: `;y` copy a link's address, `;d` download a link or image, `;i`
    open an image beside (as F does), `;b` open beside in the background,
    `;e` type in a field, `;m` the keep-hinting mode (open beside in the
    background, labels stay until Esc; chosen labels dim and the corner
    counts them). Bound on the board and in Page mode alike.
  - Each kind labels only what it can use (the page script takes a filter:
    links / images / links + images / fields), so labels stay short. Images
    are `img` elements with an http(s)/file address, at least 16×16 px; an
    image inside a link is the link's (`;d` saves the link).
  - "Background" means the card, the camera and the keys stay where they
    are; the new card grows in beside, joined by a trail edge.
  - While a kind's labels show, the corner says what they do
    (`Hints · Copy a link's address`) in place of the mode pill.
  - Downloads go through the page (`Page::download`), like the menu's
    *Download link*, so cookies apply and the usual download flow follows.
  - Verified e2e (`~/.cache/trailboard-e2e-m12`, `script14`, `script15`,
    page `site/hk.html`, downloads to `dl14` via `TRAILBOARD_DOWNLOAD_DIR`):
    ;y labels only the 3 links and copies b.html; ;i labels the one big
    image (the 8 px one skipped) and opens it beside; ;d labels links and
    the image and saves both; ;b opens c.html beside with focus, Page mode
    and camera unchanged; ;m a s opens a and b and keeps the labels
    (`many.png`); ;e a then typing fills the field in Insert (`typed15.png`).
    Not verified: a real keyboard, the clipboard read back for ;y (logged
    only), images in `srcset`/`picture` beyond `currentSrc`, CSS background
    images (not labelled), the light theme for dimmed labels.
- ✅ 6. Command line `:` (2026-11-20; `scene::command`,
  `storage::commands`, `palette::command_rows`, `App::run_line`). Decisions:
  - It is the palette, not a new box: a palette text that starts with `:`
    is a command line (`:` on the board or in Page mode opens it with the
    `:` typed; typing `:` first in Ctrl+K does the same; deleting the `:`
    goes back to searching). Insert mode types `:` into the page as before.
  - Our own names, lower case with dashes (`space-new`, `clear-history`),
    one table (`scene::command::COMMANDS`) that covers every palette action
    plus arguments: `open`/`beside`/`new [address]` (resolved like the
    address bar), `card <title>`, `space <name>`, `back`/`forward [times]`,
    `reload [fresh]`, `find [words]`, `copy [address|title|markdown]`,
    `zoom <in|out|reset>`, `name [trail name]`, `space-new`/`space-private
    [name]`, `space-rename <name>`, `history [words]`.
  - A name or alias exactly, else a unique start of a name (`tid`); `t` is
    ambiguous and the list shows both. Choices take a unique start too
    (`zoom o`). Space and card arguments pick the same name (case ignored),
    else the best fuzzy match; a card title shared by several goes to the
    one used last.
  - Rows: the typed line when it parses (*Run*), then up to 5 earlier lines
    that start with it (*Recent*), then completions (*Commands*: names that
    start with it first, then fuzzy on the name or what it does; or a
    command's values). Tab puts the highlighted row's line in the field;
    Enter runs a whole command, else fills it in like Tab. With no rows the
    list says why the line isn't a command (`No command “nope”`).
  - History: the last 100 lines as typed, newest first, once each, in
    `$DATA/commands.json` (`version` 1), shared by spaces; nothing is kept
    while a private space is active. Shift+Delete forgets a Recent line.
  - Vim extras (the open question): `:q` (close card), `:w` (save as
    Markdown), `:e` (open) and `:qa` (exit) are aliases. Ctrl+w h/j/k/l is
    left for step 7, where moving between cards from a page belongs with
    jumps. zz/zt/zb are not adopted: the board's `z` (fill) stays, and
    top/bottom have no meaning for a card on the board.
  - Verified e2e (`~/.cache/trailboard-e2e-m12`, `script16`): `:` lists
    44 commands (`cmd_all.png`), `:sp` the five space commands, `:ti` Tab →
    `:tidy` and Enter runs it, `:open` b.html, `:beside` c.html (edge),
    `:zoom o` → 90 %, `:nope` says so (`cmd_err.png`), `:q` closes c,
    `:space-new Work` makes and names a space, `:space ` lists both spaces
    and Tab+Enter goes back, the Recent rows (`cmd_recent.png`),
    Shift+Delete forgets one, `:` from Page mode, `:exit` quits;
    `commands.json` read back. Not verified: a real keyboard (`:` as
    Shift+; on non-US layouts), `:card`, `:copy`, `:find`, `:history`
    with words, a private space skipping history, the light theme.
**Fully keyboard-driven (owner, 2026-11-20).** The owner asked for the
whole browser to be usable from the keyboard alone, vim-style, with the
depth of the keyboard-first browsers they used before (their old config:
search keywords, a link sent to a video player, a dark-pages toggle, bar
toggles, reload of the key file, saved pages, user scripts). Keys follow
common vim conventions; command names, the key file's format, the docs
and the code are our own (clean-room rule). A card plays the part a tab
plays elsewhere. Steps 7–17 below, one per session, in order; questions
marked **Ask** are the owner's call and get asked when their step starts
(the suggested answer is used if the owner says "your call").

- ✅ 7. **Cards from a page** (cards as tabs; 2026-11-20; `scene::tidy::order`,
  `scene::nav::step`/`nth`, `App::card_key`, `App::switch_card`,
  `App::close_from_page`). Decisions:
  - Reading order is Tidy's: each trail from its root, depth first
    (children top to bottom), trails by their roots' places; then the lone
    cards top to bottom, left to right. Folded-away cards are skipped. J/K
    go round at the ends; a count repeats (`3J`).
  - Alt+1…8 is the nth card, Alt+9 always the last; a number past the end
    toasts. Ctrl+^ is the card current before this one (remembered per
    space in `App::focus`); it is bound as `<C-^>`, `<C-6>` and
    `<C-S-6>`, since Ctrl+Shift+6 reaches us as a digit (`KeyIn::chord`
    now keeps a digit with Ctrl/Alt even with Shift). Alt+digits and
    Ctrl+^ work on the board too (no Page mode there).
  - Moving from a page keeps Page mode on the new card (a note too: i to
    write). In Solo, Solo follows the card.
  - gt / T open the command line with `card ` typed (the title list).
    `:card <title>` from a page keeps Page mode and Solo now (it used to
    leave Solo). `:card N` with a number counts in reading order (a card
    titled only with digits loses to that).
  - d in a page closes the card and goes on to the next card in reading
    order (the previous at the end), keeping Page mode and Solo; with no
    card left it is the board. On the board, d is Delete (the selection,
    else the current card, nothing current after). u reopens the newest
    closed card (a count reopens that many); in a page the keys go to it.
  - Ctrl+W in Page mode starts `Ctrl+w` sequences: h/j/k/l and arrows the
    nearest card that way (`scene::nav`), w / W next / previous, p the card
    before, c / q close. Alone it still closes the card once the 1 s pause
    passes. Ctrl+W on the board and in Insert closes at once, as before.
  - co asks first (`Confirm::CloseOthers`, Enter closes), counting folded
    cards too; it is one undo step, so `:undo` (or Ctrl+Z on the board)
    brings them all back and u brings them one at a time. Ctrl+Z in a page
    is the page's, so the toast names `:undo`.
  - gC clones a page (not a note) beside itself with a trail edge, by
    address: the clone's back/forward list starts afresh.
  - Commands: `card-next [n]`, `card-prev [n]`, `card-only` (alias
    `only`), `clone`; `:clo` is now ambiguous (clone, close).
  - Verified e2e (`~/.cache/trailboard-e2e-m12`, `script19`, five cards:
    welcome, start page, a → b → c): K, 2J going round, Ctrl+Shift+6,
    g$, g0, Alt+2, Alt+9, Ctrl+w h, gt + `Page a` + Enter (`cardpick.png`)
    keeping Page mode, gC (edge 3 → 6), d going on to the next card in
    Page mode, u, co + Enter (`closeothers.png`) leaving one card, board d
    and u, J in Solo (Solo follows), d in Solo (Solo reads the next),
    Ctrl+W alone closing after the pause, `:card 1`, `:card-next 2`;
    `script20`: co + Esc keeps the cards, co + Enter then `:undo` brings
    both back. Not verified: a real keyboard (Ctrl+^ on non-US layouts,
    Alt+digits under the desktop's own Alt bindings), the light theme.
  - Planned, in Page mode without going
  back to the board:
  - `J` / `K` next / previous card; `gt` and `T` pick a card by title (the
    `:card` list); `g0` / `g$` first / last; `Alt+1…9` the nth card
    (`Alt+9` the last); `Ctrl+^` the card used before this one.
  - Close / reopen the card, `co` close the other cards in the space (asks
    first), `gC` clone the card beside itself.
  - `Ctrl+w h/j/k/l` to the nearest card that way, staying in Page mode
    (the board's `nav` logic).
  - Commands: `:card-next`, `:card-prev`, `:card-only`, `:clone`, `:card N`.
  - **Owner (2026-11-20):** `J`/`K` walk the cards in reading order (as
    Tidy lays them out). `d` closes the card and `u` reopens the last
    closed one in Page mode (this replaces the 2026-11-17 answer 2); half
    a section scrolls with `Ctrl+d`/`Ctrl+u` only. The board's `d`/`u`
    follow the same rule.
- ⬜ 8. **Search keywords.** `g rust` / `gh foo` / `w trail` typed in the
  address bar, `o`, `O` and `:open` search that engine; the rest uses the
  default engine. A keyword table in Settings (keyword → URL with `{}`),
  versioned in `settings.json` with a migration; defaults for common
  engines. The address bar's suggestion row says which engine
  (*Search GitHub for “foo”*). Pure resolution in `scene` with tests
  (a keyword must be followed by a space; an address still wins).
- ⬜ 9. **Jumps, marks and saved pages.**
  - `Ctrl+o` / `Ctrl+i` back / forward through the cards visited (a jump
    list per space, kept across restarts).
  - `m{a-z}` / `'{a-z}`: a lower-case mark is this card's scroll spot, an
    upper-case one a board spot (camera + card) across spaces; `''` back
    to before the last jump.
  - Saved pages (named, like bookmarks): `:save-page [name]`, `b` / `B`
    open one here / beside with completion, `:saved` lists them (Enter
    opens, Shift+Delete removes). One versioned file in `storage`.
  - **Owner (2026-11-20):** `b` / `B` open saved pages everywhere (board
    and page); the trails sidebar moves to `Ctrl+B` (free today; in a text
    field the page keeps it for bold) and `:sidebar`. Both named saved
    pages and lettered marks (`m{a-z}` / `'{a-z}`) are kept.
- ⬜ 10. **User key file and external commands.**
  - `$CONFIG/trailboard/keys` in our own plain-text format, read by a
    tested parser in `storage`: bind / unbind a key sequence to a command
    line per mode (board, page, insert, hints, command), aliases (a short
    name for a longer line), and chains of commands in one binding.
    Errors are reported with their line, the rest still loads.
  - `:bind`, `:unbind`, `:reload-keys`; `:keys-file` opens it in a card.
  - `:run program args` starts an outside program with `{url}`, `{title}`,
    `{hint-url}` filled in (never through a shell); a hint kind that runs
    a bound command on the chosen link. This covers "play this link in a
    video player" (here, beside, many).
  - The `?` sheet is built from the live tables (user keys included);
    after a prefix (`g`, `;`, `y`…) a small panel lists what can follow.
  - **Owner (2026-11-20):** no outside video player as a feature; video
    plays in the card, and M13 adds the missing formats. `:run` stays for
    anyone who wants to bind a player in the key file themselves.
- ⬜ 11. **Settings from the keyboard.**
  - `:set name value` with completion of names and values; `:set name`
    shows the value; `:reset name`.
  - `:toggle name [values…]` cycles a setting; default keys under a `t`
    prefix in Page mode: dark pages (page colours), the space header,
    the sidebar.
  - Per-site toggles for the current site: JavaScript, images, cookies,
    ads (`sitesettings`, `privacy`), for this session or saved; the page
    reloads.
  - **Owner (2026-11-20):** `t` starts toggles everywhere (board and
    page); Tidy stays on `T` (bound today) and `:tidy`; Ctrl+T stays New
    card. The `?` sheet, guide and palette keys change from `t` to `T`.
- ⬜ 12. **Page navigation extras.**
  - `[[` / `]]` previous / next page (rel links, else "next"/"previous"
    link text), `gu` / `gU` up the address path / to the site's root,
    `Ctrl+a` / `Ctrl+x` change the last number in the address (with a
    count).
  - `go` / `gO` edit the current address here / beside; `xo` open in the
    background beside; `Ctrl+s` stop loading; `gh` home (start page);
    `gf` view source in a card beside; `gd` download the page; `F11`
    full screen; `wi` page inspector (if CEF's devtools fit our window).
  - Yanks: `yd` the site, `yp` the address without tracking parts; Shift
    versions (`yY`, `yT`, `yD`…) to the primary selection; `pp` / `PP`
    open the primary selection.
  - More hint kinds: `;o` put the link's address in `:open` to edit,
    `;h` hover, `;f` every clickable element, `;I` image here, `;Y` link
    to the primary selection.
- ⬜ 13. **Command line power.** Readline keys in the field (`Ctrl+w`,
  `Ctrl+u`, `Ctrl+a`, `Ctrl+e`, `Ctrl+k`, `Alt+b`, `Alt+f`); Up / Down
  walk earlier lines starting with what is typed while Tab / Shift+Tab
  walk the list; `Ctrl+Enter` runs and keeps the line open; `.` repeats
  the last command; a count before `:` or a command (`3:back`); `:later
  ms cmd`, `:repeat n cmd`; Ctrl+Delete removes a list row.
- ⬜ 14. **Visual and caret mode** (was step 9). `v` caret, `V` line
  selection, `h j k l w b e 0 $ gg G { }` move, `o` swap ends, `y` / Enter
  copy, `Y` to the primary selection; starts at a `/` match or the top of
  the view; the mode pill says *Visual*.
- ⬜ 15. **Dialogs and fields from the keyboard.** Our yes/no asks and
  the page's alert / confirm / prompt answer to `y` / `n` / Enter / Esc;
  permission asks (camera, location, notifications) likewise; the file
  chooser takes a typed path; `Ctrl+e` in a field edits its text in
  `$EDITOR` and puts it back; `Shift+Insert` pastes the primary selection.
- ⬜ 16. **Search and messages.** `?` searches backward (**Owner,
  2026-11-20:** yes; the shortcut sheet moves to `F1` and `:keys`); Esc clears the
  find highlight; `:messages` lists the last toasts and errors.
- ⬜ 17. **Stretch.** Macros (`q{a-z}` records, `@{a-z}` plays, `@@`
  again); `:screenshot` of a card; `:print`; small user scripts run on
  matching sites from a folder (our own format, opt-in).

**Owner answers (2026-11-17):**
1. In Page mode f follows a link in the same card and F opens it beside. On
   the board f keeps opening beside (Shift+F stays Fit).
2. d/u keep scrolling half a section (Page mode and board); Ctrl+d/u do the
   same in Page mode. Closing stays Delete; no vim-style d/u for close/undo.
3. "Based on the focus": where the keys are decides. With the board focused
   (Board mode) h/j/k/l move between cards; with a card entered (Page mode)
   they scroll its scroll target.
4. Build steps 1–3, then decide on the rest after trying them.
5. Left to us: chaining at a section's edge, like a wheel.

**Solo (owner, 2026-11-17):** Read Mode is renamed Solo in everything
the user sees (palette, `?` sheet, guide, README, logs); the code keeps
`scene::read`, `App::read` and `Command::Read`. "Focus" stays out of
interface text so it never clashes with a field's focus (Insert mode).

**Extra vim keys** (decided with step 6, see there): `:q`/`:e`/`:w`/`:qa`
yes; Ctrl+w h/j/k/l with step 7; zz/zt/zb no (board z stays).

**Risks:** sites with their own single-letter keys (Passthrough and the
per-site setting cover them); running the scroll script inside cross-origin
iframes is unverified (the wheel fallback still scrolls them); sites that
fake scrolling with JS aren't found as sections (the wheel at the view's
middle still reaches them); a key typed right after a field gains focus,
before the focus message arrives (a click in a page keeps giving the keys at
once).

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
of RAM; this machine has 7.6 GB and ~11 GB free, so builds run on a rented
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
- ⬜ 6. **Keeping current.** A short checklist in the README for each CEF
  update: rebuild, re-run the codec page, smoke-test video; the start log
  warns when the installed build is older than the version
  `install-cef.sh` pins.

**Owner decisions (2026-11-20):** no outside video player as a feature;
video plays in the card. M13 replaces the "send to a video player"
fallback idea; whether a fallback message offers an outside player stays
open until M13 step 5.

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

**Last session:** 2026-11-20 (twenty-fifth). Finished M12 step 7, cards
from a page: J/K in reading order (`scene::tidy::order`), gt/T, g0/g$,
Alt+1…9, Ctrl+^, Ctrl+w h/j/k/l, d close / u reopen (board too; half-page
scroll moved to Ctrl+d/u), co, gC, and `:card N`, `:card-next`,
`:card-prev`, `:card-only`, `:clone`.
**Repo state:** on `master`; M12 steps 1–7 done (step 7 not yet
committed); M12 steps 8–17 and M13 planned.
**Next up:** M12 step 8, search keywords (`g rust`, a keyword table in
Settings, versioned in `settings.json` with a migration). Steps 9–17
follow in order. M13 (full media support, our own CEF build) runs beside
M12; its builds need a cloud machine. Still open from step 2: the
per-site "start in Insert" setting (fits step 11). Also retake the README
screenshots (`docs/images/`) in the new look.

**Small follow-ups (backlog):** glide on undo/redo of moves; follow the
desktop's animation switch live; verify the setuid sandbox path without
unprivileged user namespaces; more consent platforms (TCF frames, Ketch,
Termly, Borlabs, Axeptio); scriptlets/`$redirect` resources (check uBO's
licence); procedural cosmetic rules; auto-scroll the sidebar while
dragging; a downloads folder setting; download progress on the board, ask
before quitting with downloads running, pause/resume; more settings (start
page for an empty board, custom search URL, card size); thumbnails off the
main thread; thumbnails on history rows or the sidebar; keep closed cards
across restarts; a private space looking different; undo for forgetting a
page; site icons on history rows; prune unused icons; letter tiles for
sites without an icon; a zoom badge on zoomed cards; Ctrl+scroll in a page
to zoom it; keyboard selection of several cards; link hints inside
iframes; a portal file chooser for Save as Markdown; open `crash.log` from
the crash toast; undoing the close of a stack member puts it back in the
stack.

**Notes:**
- grim hangs while the display is off and catches the lock screen while
  locked; take screenshots with the screen unlocked. CEF needs
  `XDG_SESSION_TYPE=x11` (and no `WAYLAND_DISPLAY`) under XWayland.
- Put a `wait` after each `log shot …` script line; the first shot needs ~5 s.
- Script `open` with a data: URL needs percent-encoding; a `file://` page is
  easier. While a script runs, the window's IME stays off.
- Keep `hints::SECTIONS` (the `?` sheet) in step with new shortcuts until
  M12 step 8 generates it.
- Known rough edges: a note's last keystroke can be lost if its card closes
  or sleeps in the same frame; a stale full-window frame can flash after
  leaving Solo; zooming can churn the live cap.

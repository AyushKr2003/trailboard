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
| 🟡 | **M12 Keyboard first** | Vim-like modes, page keys, scroll sections, command line, cards from a page, search keywords, jumps and marks, user keys, settings, navigation extras, command line power, caret mode, dialogs and fields, search and messages, then (step 17) stretch (below) |
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
  O outline, R/C trail name/colour, Ctrl+B sidebar (b/B saved pages since
  step 9), Tab Solo, Delete close,
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
  - Not done: the per-site "start in Insert" setting (done in step 11 as
    per-site Passthrough, `tp`). Not verified: a real
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
    *Changed 2026-11-21 (bug fixes below): Ctrl+W alone closes nothing.*
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
- ✅ 8. **Search keywords** (2026-11-20; `scene::search`,
  `Settings::resolve`, `storage::settings` v2, `settings_page`
  `keyword-set:` / `keyword-remove:` / `keyword-reset`). Decisions:
  - Resolution moved from `app::omnibox` to `scene::search` (tested):
    an address (a scheme, or one word with a dot / `localhost`) first;
    then `keyword␣query` with a non-empty query; else the default
    engine. A keyword alone, or glued to its query (`ghfoo`), is a plain
    search. Keywords match ignoring case. Everything that turns typed
    text into a page goes through it: the address bar (Ctrl+L, Ctrl+T,
    o, O), `:open`/`:beside`/`:new`, the palette, start pages, p/P, and
    `trailboard 'gh foo'` from a shell. The page menu's *Search for*
    stays with the default engine (the text is never a keyword line).
  - A keyword is 1–16 letters, digits, `-` or `_` (no dot, so it can't
    look like an address), stored lower case; its address must be http(s)
    with exactly one `{}` (the query, `+` for spaces). A missing name
    becomes the address's host. Adding a keyword that exists replaces it
    in place.
  - Defaults (16): `d` DuckDuckGo, `g` Google, `b` Bing, `br` Brave, `sp`
    Startpage, `k` Kagi, `wa` WolframAlpha, `w` Wikipedia, `gh` GitHub,
    `yt` YouTube, `map` OpenStreetMap, `mdn` MDN, `rs` Docs.rs, `cr`
    crates.io, `aw` ArchWiki, `aur` AUR.
  - `settings.json` version 2 adds `keywords` (the whole table; `[]` is
    none). Version 1 files are migrated to the defaults (a v1 file's
    `keywords` is ignored); bad or repeated rows are skipped with a log
    line. An older build reading a v2 file uses its defaults (the usual
    newer-version rule).
  - Shown: the palette's row says *Search GitHub for “foo”* and comes
    first (a plain search stays last); the lone address bar shows
    *Search GitHub* at its right end only for a keyword (plain searches
    stay quiet).
  - Settings: a *Search keywords* row under *Search and new cards* with a
    list (keyword, name, address; × removes on hover), a form (keyword,
    name, address; Enter or Add/Save), and *Defaults*. Clicking a row
    loads it into the form; Save replaces. Bad input toasts why and the
    form keeps it. The settings search finds the table by "keyword" or
    any keyword, name or address.
  - Verified e2e (`~/.cache/trailboard-e2e-m12`, `script21`, `script22`):
    o + `gh trailboard` (`kw_bar.png`, the note) loads GitHub's search,
    `:open w rust` Wikipedia, `:beside gh` searches DuckDuckGo for "gh",
    Ctrl+K `yt lofi` lists *Search YouTube for “lofi”* (`kw_palette.png`);
    in Settings, filtering by "keyword" (`kw_filter.png`), adding `rs2`
    → Rust std (`kw_added_crop.png`), saved as version 2 in
    `settings.json`, a bad keyword toasts, `:open rs2 vec` opens the
    Rust docs search. Not verified: removing with × and editing by a
    click (only the reset button, pressed by mistake in one run), the
    narrow-card layout of the table, the light theme, a real keyboard.
- ✅ 9. **Jumps, marks and saved pages** (2026-11-20; `scene::jump`,
  `scene::saved`, `storage::jumps`, `storage::saved`, `web::scroll`
  `spot`/`restore`, `App::walk_jumps`, `App::set_mark`, `App::go_mark`).
  Decisions:
  - Jump list: every card made current counts as a visit (keys, clicks,
    the palette, a new card), except the Ctrl+o / Ctrl+i moves themselves.
    One list per space, oldest first, each card once (a card visited
    again moves to the end, as vim's jump list does; going back and then
    visiting doesn't cut the newer entries off). At most 100. Closed cards
    are skipped while walking and dropped when a space loads (this run's
    closed cards stay, for u). With nothing current (after a restart),
    Ctrl+o goes to the newest card first. A count walks that many.
  - Card marks (`a`–`z`) are per card and per space: the page's address,
    the scroll section (a path of child numbers from the document root,
    `""` for the page; a section inside a shadow root falls back to the
    page) and its position. `'a` on the same page (fragment aside)
    scrolls at once; a page that moved on (or is still loading) loads the
    mark's address and scrolls when loading ends. Only web pages load.
  - Board marks (`A`–`Z`) are one per letter across spaces: space, current
    card and camera. Going to one switches space if needed, goes to the
    card (toast if it was closed) and glides the camera there (jumps at
    once across spaces or with motion off).
  - `''` (and ``` `` ```): every card change and every mark jump keeps where
    it left from (board spot, or the page's scroll spot for a card mark);
    `''` goes there, so twice comes back. Not kept across restarts.
  - On the board `m` stays *move the card* (m then h/j/k/l), so marks are
    set with `M` + letter there; in a page `m` + letter. `'` and `` ` ``
    go to marks on both. Ctrl+b in a page leaves the page and opens the
    sidebar with the keys (not in Solo). In the sidebar, bare b no longer
    closes it (Esc or Ctrl+b).
  - Saved pages: one list for all spaces, newest first, names unique
    ignoring case (saving under a taken name replaces it); no name → the
    page's title, else its host; saving a page saved already without a
    name says under which name. Only web pages (not notes, start or app
    pages). `:saved` needs a name, so `b` → Enter opens the highlighted
    (newest) page and `:saved` alone lists the names; `here` follows
    `:open` (this card's page, else a new card in Page mode). Shift+Delete
    on a row of the `saved` / `saved-beside` list forgets it. b with
    nothing saved toasts how to save one.
  - Files: `jumps.json` (`version` 1: jump lists, card marks, board
    marks) and `saved.json` (`version` 1), atomic writes; jumps are saved
    with the board (autosave), saved pages at once. Deleting a space drops
    its jumps and marks.
  - Commands: `saved <name>`, `saved-beside <name>`, `save-page [name]`,
    `mark <letter>`, `to-mark <letter>` (`'` too), `jump-back [n]`,
    `jump-forward [n]`. Value rows can carry their own detail (a saved
    page's host). `:t` is now ambiguous between `to-mark`, `tidy`, `tint`.
  - Verified e2e (`~/.cache/trailboard-e2e-m12`, `script23`–`script25`,
    page `site/long.html` whose title is its scroll position): `20j` `ma`
    `gg` `'a` → y=1200, `''` → 0, `''` → 1200; `mA` (`mark_toast.png`);
    Ctrl+o / `2` Ctrl+o / Ctrl+i over four cards; `'A` back to the long
    card; `:save-page Long one` (`saved_toast.png`); in a new space `b`
    lists it (`saved_list.png`) and Enter opens it; `'A` from that space
    switches back; Ctrl+b opens and closes the sidebar; B + Shift+Delete
    forgets the page and b then toasts; after a restart (`run_keep.sh 24`)
    Ctrl+o and `'a` work from `jumps.json`; a mark in `sec.html`'s inner
    pane (path `1.1.1.1`) comes back to 39 %. Not verified: a real
    keyboard (`'` and `` ` `` on non-US layouts, Ctrl+i vs Tab), a mark
    whose page moved on (load then scroll), Solo with board marks, the
    light theme.
  - Planned:
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
- ✅ 10. **User key file and external commands** (2026-11-20;
  `scene::userkeys`, `storage::keyfile`, `keys::NAMED`, `hints::sections`,
  `App::load_keys`, `App::exec_line`, `App::run_program`). Decisions:
  - File `$XDG_CONFIG_HOME/trailboard/keys` (else `~/.config/…`), one
    entry a line, `#` lines are notes: `bind <modes> <keys> <chain>`,
    `unbind <modes> <keys>`, `alias <name> <chain>`. Modes `board`, `page`,
    `insert`, `hints`, `command`, comma-separated. Keys in the notation
    `scene::keys::parse` already reads (`gs`, `<C-p>`, `<A-x>`, `<F5>`).
    The file has no `version`: it is the user's text, never rewritten;
    we only make it, with a commented template, when `:keys-file` finds
    none. Unknown first words are errors, so a later format can add
    words without breaking old files.
  - A chain is commands split at `|` (`\|` for a bar). Each is an action
    name (`keys::NAMED`, 85 names such as `scroll-down`, `next-card`,
    `hint-yank`), an alias (words after it go after its chain's last
    command), or a `:` command line, checked when bound. An action name
    that does nothing in a mode but is also a command (`tidy` in a page)
    runs as the command. Marks and Alt+digits have no names (`mark a`,
    `to-mark a`, `card 3` reach them). A binding to one action keeps
    counts as before; a binding to a chain runs the whole chain `count`
    times (at most 100).
  - Aliases are read before bindings, so order in the file doesn't
    matter; an alias may not take a command's name or short name; loops
    are found (depth 8). `:alias` typed in the command line works too.
  - In insert, hints and command a binding is one chord with Ctrl or
    Alt, or an F key (letters type there); hints and command bind lines
    only, insert takes page actions too. Esc can't start a binding.
    `unbind` of a start drops every binding under it (`unbind page ;`).
  - Errors: each wrong line is logged with its number; a toast names the
    first and counts the rest; the other lines still apply.
  - `:bind`, `:unbind`, `:alias` change keys for this run only (the toast
    says so); `:reload-keys` starts again from the file, dropping them.
  - Placeholders `{url}`, `{title}` (the current card's page; not notes)
    and `{hint-url}` are filled when a line runs, in the whole line, except
    in `bind`/`unbind`/`alias`/`hint` lines (kept for later) and `run`
    (filled per word, so a title with spaces stays one word). A missing
    value toasts why. Typed lines fill them too (`:open …/{url}`).
  - `:run` splits words like a shell would quote them (`'…'`, `"…"` with
    `\"`, backslash) but never runs one: `Command::new` with stdin/out/err
    null, its own process group, a thread waiting so no zombie stays.
    Toasts *Started prog* or why it couldn't.
  - `:hint <line>` / `:hint-many <line>` label links (`hint::Kind::Run`,
    links only) and run the line with `{hint-url}` on the chosen one;
    many keeps the labels until Esc. The line after `hint` is one command;
    to chain after a hint, name the chain with an alias.
  - The `?` sheet is built when it opens (`hints::sections`): the board's
    and Page mode's tables grouped by action in `NAMED` order, keys as
    alternatives (Shift twins of listed keys hidden, at most 4), the 52
    mark letters and Alt+digits as ranges (`m then a…z`); then sections
    for the user's insert / hints / command keys and aliases when there
    are any; then the fixed app shortcuts and mouse rows. Keycaps read
    " then " as a word between keys.
  - Prefix panel: 400 ms after a key that starts longer bindings, a panel
    above the mode pill (board: above the count) lists the next keys and
    what they do (14 rows, then "and N more"). A key that only starts
    bindings no longer expires after 1 s (it waits for the next key; Esc
    drops it); a key that is a binding *and* a start still runs after 1 s.
  - Command line: an alias or a chain is a runnable first row (*Alias ·
    …*, *Runs 2 commands in turn*), aliases complete while the name is
    typed. New commands `bind`, `unbind`, `alias`, `reload-keys`,
    `keys-file`, `run`, `hint`, `hint-many`; `:u` is now `undo` (it would
    have been ambiguous with `unbind`); `:k`, `:h`, `:r` are ambiguous.
  - Verified e2e (`~/.cache/trailboard-e2e-m12`, `run10.sh`, `script26`,
    `script27`, `config26/trailboard/keys`, `rec.sh` writing its words to
    `rec.txt`): two bad lines named (13, 14) and the rest loaded; `,r`
    ran `rec.sh` with `title: Form page` as one word and the address;
    `,b` an alias opening b here; `d` unbound did nothing; `g` showed the
    prefix panel (`k_prefix.png`); `;v` → `hint rec hint {hint-url}`
    labelled two links (`k_hintrun.png`) and `a` ran it with a.html;
    Alt+q in hints, Alt+z in the command line, Alt+x in Insert each ran
    their line; `:tob` showed the alias row (`k_alias_row.png`); `:bind
    page ,t tidy` toasted (`k_bound.png`); the sheet from the live tables
    (`k_sheet.png`) and filtered to "your" (`k_sheet_filter.png`);
    `:keys-file` showed the file as text in a card (`k_file.png`);
    `:reload-keys`; `,c` chained card-next and top; `x` bound to close
    closed. Not verified: a real keyboard (Alt chords under the
    desktop's own bindings), `:hint-many`, `:unbind`/`:alias` typed,
    a program that isn't found, the light theme.
  - Planned:
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
- ✅ 11. **Settings from the keyboard** (2026-11-20; `scene::setcmd`,
  `Content::Cookies`/`Passthrough` in `scene::sitesettings`,
  `web::prefs` cookie blocks, `App::apply_setting_change`,
  `App::switch_site`). Decisions:
  - Names are the settings file's ids with dashes (`page-colours`,
    `board-background`, `third-party-cookies`), plus `proxy-server`,
    `corner-radius` and `keywords`; an exact name wins, else a unique
    start. Values: a choice's id or its name (case ignored), or a unique
    start of either; switches also take yes/no/true/false/1/0; zoom and
    live pages take any number in range (`125%`). Errors say why
    (`Page colours: “pink” isn't one of theme, light, dark`).
  - `:set name` alone toasts what it is; `:set name value` and `:reset
    name` go through the settings page's own path (`choose_setting`), so
    they apply, save and update an open settings page the same way, then
    toast the new value. `corner-radius` also turns *Corners* to Custom,
    a `proxy-server` *Proxy* to Custom. Keywords can only be shown and
    reset (the settings page edits them). No `:reset all`.
  - `:toggle name`: a switch flips, a list goes to its next choice (as the
    settings page's arrows); with one value it goes between that and the
    default, with several round them (the first if the current isn't one).
  - Completion is two-level: setting names with their current value on the
    right, then the setting's values with the current one marked *now*
    (`command::Kind::Setting`; the values callback now takes the setting
    name). `:site` completes its switches, then on/off.
  - `t` keys (board and page; Tidy is `T` only): `td` flips what pages
    show (dark → Light, light → Dark, whatever *Page colours* was, so it
    always changes something), `th` the space panel (the "header"), `ts`
    the sidebar; `tj`/`ti`/`tc`/`ta` JavaScript, images, cookies, ads for
    the current card's site, saved; Shift (`tJ`…) for this session only;
    `tp` the site starts in Passthrough (the open question from step 2: a
    per-site *Passthrough* rather than *Insert*, as the mode table says;
    applied wherever a card is given the keys in Page mode, and the card
    in use switches at once). Key-file names `dark-pages`, `space-panel`,
    `site-javascript`… `session-ads`, `site-passthrough`.
  - Commands `set`, `reset`, `toggle`, `site <what> [on|off]`,
    `site-session <what> [on|off]` (without on/off it flips; `js` is
    short for javascript). `:t` is now ambiguous with `toggle` too.
  - Session rules are a second `SiteSettings` over the saved one (a
    session choice equal to the default still overrides a saved rule); a
    saved change clears the session rule for that switch. Ads keep their
    own allowed list; "session" uses its unsaved entries (as a private
    space already did).
  - Cookies per site: Chromium's content setting (`set_content_setting`,
    COOKIES, BLOCK) for `https://site/` and `http://site/` on the profile
    and every private jar, which Chromium turns into `[*.]site` (pages and
    requests alike). Chromium saves these in its `Preferences`; ours in
    `sites.json` are the truth, so the start-up rewrite that drops saved
    zooms drops `profile.content_settings.exceptions.cookies` too, and
    they are set again each run. `sites.json` gained optional `cookies`
    and `passthrough` fields and stays version 1 (older builds ignore
    them). The page menu still offers only JavaScript and images.
  - Verified e2e (`~/.cache/trailboard-e2e-m12`, `run11.sh`, `script28`–
    `script31`, page `site11/t11.html` served by `python3 -m http.server
    8765`, whose title reports JS, a cookie and the image): tc → `c=n`,
    ti → `i=n`, tJ → `nojs` (not saved), `:site-session javascript on`,
    `:site images on`, `:site cookies` back to `c=y i=y`; `:set pa`
    names (`s_names.png`), `:set page-colours ` values (`s_values.png`),
    `:set page-colours dark`, `:set scroll-speed` toast, a bad zoom
    toasted, `:set default-zoom 125`, `:toggle history`, `:toggle search
    google bing`, `:reset page-colours`, td, th (`s_nopanel.png`), tp
    and Shift+Esc (`s_pass.png`), ta, `:set corner-radius 4`, the sheet
    filtered by "site" (`s_sheet.png`); after a restart cookies stay
    blocked from `sites.json`, session JS is gone and Enter starts the site
    in Passthrough; with the rule taken out of `sites.json`, Chromium's
    own saved block was dropped and cookies worked again. Not verified: a
    real keyboard, a private space's jar, a site with subdomains
    (`[*.]` pattern assumed from the log of Chromium's `Preferences`),
    `:set language`/`proxy-server` typed, the light theme.
  - **Owner (2026-11-20):** `t` starts toggles everywhere (board and
    page); Tidy stays on `T` (bound today) and `:tidy`; Ctrl+T stays New
    card. The `?` sheet, guide and palette keys change from `t` to `T`.
- ✅ 12. **Page navigation extras** (2026-11-21; `scene::address`,
  `web::pager`, `links::Filter::Clickable`, `keys::Yank`,
  `Target::Behind`, `App::go_address`/`copy_from_card`/`got_pager`).
  Decisions:
  - `]]` / `[[`: a page script (`web::pager`) sends `<link rel>` and the
    links whose rel or short text might say next/previous; the app picks
    (`address::pick_pager`, tested): rel first, then the whole text being
    a word (next, older, older posts, more / previous, prev, newer…),
    then a text starting with one (`Next chapter`), then a lone arrow (»
    › → >). The first of equals; links back to the page don't count. It
    loads in the card.
  - `gu`: the query and fragment go first (one step), then a path
    segment; a count goes further. `gU` the root. `Ctrl+a` / `Ctrl+x`:
    the last digit run after the host (never the host's own digits), a
    count is the step, zero padding keeps its width, it stops at 0.
    These take Ctrl+a / Ctrl+x from the page in Page mode only (Insert
    still selects all / cuts).
  - `go` / `gO`: the address bar with the address, caret at its end (not
    selected, unlike `o`). `xo`: a new `Target::Behind`, Enter opens
    beside and stays. `gh`: a start page *beside* the card (its page is
    kept), with the keys in its search box. `gf`: `view-source:` in a card
    beside; the card keeps its `view-source:` prefix when Chromium reports
    the plain address (`address::reported`). `gd`: Chromium downloads the
    page's address, as the menu's *Download link*.
  - `Ctrl+s` stops a loading page; when nothing loads it saves as
    Markdown, as Ctrl+S always did (the `?` sheet's Ctrl+S row stands).
  - `F11` in the board and page tables (and as an app shortcut in
    Insert): borderless full screen.
  - **`wi` dropped for now:** CEF's `show_dev_tools` with a native window
    opens nothing in our windowless setup (tried, silent). Showing the
    inspector in a card needs the devtools browser hosted as an off-screen
    page (a client of its own, its browser taken in `on_after_created`);
    left in the backlog.
  - Copy actions became `Copy(Yank, primary)`; key-file names `copy-url`,
    `copy-title`, `copy-markdown` stay, new `copy-site`, `copy-clean`,
    `primary-url`… `primary-clean`, `primary-here`, `primary-beside`.
    Tracking parts: `utm_`/`pk_`/`mtm_` keys and a list of click ids
    (`fbclid`, `gclid`, `msclkid`, `igshid`, `si`…).
  - `pp` / `PP` make `p` / `P` wait 1 s (the prefix rule) before opening
    the clipboard; accepted, as `pp` was asked for.
  - The primary selection: `wl-copy`/`wl-paste --primary` under Wayland
    when installed (arboard's data-control path read nothing here), else
    arboard. **Known:** on Hyprland 0.57 a primary selection set while
    our window has focus doesn't stick (the same happens to `wl-copy -p`
    typed in a shell while Trailboard is focused), so `yY`… and `;Y` don't
    reach other apps here; reading (`pp`/`PP`) works.
  - Hint kinds `;o` (`Kind::Edit`, fills `:open <url>`), `;h` (`Hover`:
    the pointer moves to the target's middle), `;f` (`Click`: the `f`
    targets plus labels and elements whose own cursor is a pointer where
    their parent's isn't; every target is clicked, links load in the
    card), `;I` (`ImageHere`), `;Y` (`Primary`).
  - Commands `page-next`, `page-prev`, `up [n]`, `root`, `increment
    [n]`, `decrement [n]`, `stop`, `home`, `view-source`, `save-file`
    (not `download`: `:down` would stop reaching *downloads*),
    `fullscreen`; `copy site|clean`. Names picked so `:so` (solo), `:ne`
    (new) still work.
  - Verified e2e (`~/.cache/trailboard-e2e-m12`, `site12/` served on
    :8765, `run11.sh`/`run12.sh` with `config37` sending downloads to
    `dl37`; `script32`–`script38`): `]]` p/2 → p/3 by "Next »", `[[`
    back by `rel=prev` "‹", Ctrl+a → 3, `2` Ctrl+x → 1, gu → /p/, gU → /,
    gU again toasts; `yp` then P opened `?id=3` (utm and fbclid gone);
    `go` with the caret at the end (`n_go.png`), `#top` added; `xo`
    opened p/4 beside and stayed; `gf` source card (`n_source.png`); gh
    start page beside; `;f` labelled the pointer span and clicked it
    (`h_click.png`), `;h d` hovered the button, `;o` (`h_edit.png`), `;I`
    loaded the image here; Ctrl+s stopped a hanging load; gd saved
    `2.html`; F11 on and off; `PP` opened an address put in the primary
    selection from a shell; the g / y panels (`g_panel.png`,
    `y_panel.png`). Not verified: a real keyboard, `yY`/`;Y` reaching
    another app (see above), X11 sessions, the light theme.
- ✅ 13. **Command line power.** Readline keys in the field (`Ctrl+w`,
  `Ctrl+u`, `Ctrl+a`, `Ctrl+e`, `Ctrl+k`, `Alt+b`, `Alt+f`); Up / Down
  walk earlier lines starting with what is typed while Tab / Shift+Tab
  walk the list; `Ctrl+Enter` runs and keeps the line open; `.` repeats
  the last command; a count before `:` or a command (`3:back`); `:later
  ms cmd`, `:repeat n cmd`; Ctrl+Delete removes a list row.
  - Decided: the readline keys work in every field (address, find,
    names), not only the command line, except Ctrl+A (select all) and
    Ctrl+K (closes Ctrl+K's palette), which are readline's only in the
    command line. Ctrl+W deletes to the last space (a whole address),
    Alt+Backspace / Ctrl+Backspace / Alt+D a word of letters and digits;
    Ctrl+Y puts back the last delete; Ctrl+U keeps the line's `:`. Ctrl+N
    / Ctrl+P still move in the list.
  - Tab walks the list the way a shell's menu does: the list stays the
    one for what was typed (`Omnibox::list_for`) while the field shows
    the row's line; Tab first fills the highlighted row, then moves on,
    going round; typing starts afresh. Enter on a command missing its
    value fills it (as before), so Tab no longer does that job alone.
  - Up / Down: `scene::command::Walk`, lines from *Recent* starting with
    what was typed (the line itself skipped); past the newest the typed
    text comes back. Not kept in a private space, like *Recent*.
  - Counts are part of the chain grammar (`userkeys::steps`): `N cmd`
    becomes `cmd N` when the command takes a count and has none, else
    `repeat N cmd`. `3:` opens `:3 ` (a number and a space lists every
    command, counted). `.` repeats `App::last_line` (this session, private
    spaces included), else the newest *Recent* line; a count replaces the
    line's own.
  - `:repeat` (1–100) and `:later` (ms, up to a day) are checked when
    the line is read, what they hold included; a repeat can't hold
    another (or a count), so nothing runs more than 100 times. They keep
    their placeholders raw and fill them when the held line runs, so a
    title with a bar can't split it. To repeat a chain, escape its bars
    (`repeat 2 back \| tidy`). `:later` toasts when the wait is a second
    or more, and its lines are lost on quit.
  - Verified e2e (`~/.cache/trailboard-e2e-m12`, `site12/` on :8765,
    `run13.sh script39 data39`): Ctrl+W took the address, Ctrl+Y put it
    back, Alt+B / Alt+F / Ctrl+A / Ctrl+K / Ctrl+E / Ctrl+Backspace moved
    and deleted as above (caret logged); `2:` opened `:2 `, `back` from
    p/3 went to p/1 (`c_count.png`); `:forward` then `.` reached p/3; Up
    twice walked the two `open` lines, Down twice came back to `:o`; Tab,
    Tab, Shift+Tab gave `space `, `space-new `, `space ` over a fixed
    list (`c_tab.png`); Ctrl+Enter on `zoom in` zoomed to 110% and kept
    the line; `:repeat 2 back` p/3 → p/1; `:later 1500 forward` toasted
    and ran (`c_later.png`); `:2 forward` → p/3; Ctrl+Delete forgot
    `zoom in`; `:repeat 3 repeat 2 tidy` offered nothing and said why
    (`c_nested.png`). Not verified: a real keyboard (Alt keys on this
    compositor), X11, the light theme.
- ✅ 14. **Visual and caret mode** (was step 9). `v` caret, `V` line
  selection, `h j k l w b e 0 $ gg G { }` move, `o` swap ends, `y` / Enter
  copy, `Y` to the primary selection; starts at a `/` match or the top of
  the view; the mode pill says *Visual*.
  - Decided: three states under `PageKeys::Visual(scene::caret::Mode)`:
    *Caret* (no selection), *Visual* (by characters), *Visual line*. `v`
    in a page starts Caret; `v` then selects from the caret and stops
    again; `V` selects lines from a page or either state. Esc drops the
    selection, then the caret (back to *Page*); `y` / Enter / `Y` copy
    and go back to *Page*. A toast says how many characters were copied.
  - Where it starts: after a find (`last_find` set) the find is ended
    with its match kept as the selection, and the caret goes to the
    match's start; a mouse selection in view goes straight to Visual with
    it; a click's caret in view is kept; else the start of the first line
    of text at the top of the view. No text in view says so.
  - The page does the moving (`web::caret`, `Selection.modify`), keeping
    two points (where the selection began, the caret); each move starts
    from the caret alone and the selection is drawn again, by lines
    grown with `lineboundary`. The browser's word steps stop at line
    breaks and block edges, so `w` / `e` / `b` step on until they land
    on text. `j` / `k` keep the column they started in. The caret is a
    2 px bar in the theme's accent, in a closed shadow root over the page
    (the browser draws none in text you can't type in). The page
    scrolls the caret into view, its scroll areas first.
  - Its own key table (`keys::visual`, user mode `visual`, a section on
    the `?` sheet). Unbound letters do nothing; Ctrl/Alt app shortcuts
    still work; nothing reaches the page. `:` still opens the command
    line. The mode ends when another card becomes current, the page
    loads a new document, or a field is clicked (Insert).
  - Only the main frame's text (not iframes). A copy report is taken
    only after `y` / `Y` asked for one (any page can write the console).
  - Verified e2e (`~/.cache/trailboard-e2e-m12`, page `site12/c/caret.html`
    on :8765, `run13.sh script40 data40`, `script43`): `v` put the caret
    at the top line (`v_caret.png`); `w v e y` copied *heading*; after
    `/needle` Esc, `v v e y` copied *needle*; `V j j` selected three whole
    lines (`v_line.png`) and `o Y` put 143 characters in the primary
    selection; `3w v 2e y` → *quick brown*; `5w v e y` → *fox*; `2w v 3e`
    → *The quick brown*; `4w v 2b o 2w y` → *The quick brown fox*; `G`
    scrolled to the end (`v_bottom.png`) and `{ v } y` copied its last
    paragraph; Esc twice went Visual → Caret → Page; the `?` sheet's
    *Caret and Visual* section (`sheet14.png`). Not verified: a real
    keyboard, right-to-left text, iframes, X11, the light theme.
- ✅ 15. **Dialogs and fields from the keyboard.** Our yes/no asks and
  the page's alert / confirm / prompt answer to `y` / `n` / Enter / Esc;
  permission asks (camera, location, notifications) likewise; the file
  chooser takes a typed path; `Ctrl+e` in a field edits its text in
  `$EDITOR` and puts it back; `Shift+Insert` pastes the primary selection.
  - `web::dialog`: a `JsdialogHandler` (alert, confirm, prompt,
    beforeunload) and a `DialogHandler` (file chooser) per page; each
    waits by id until `dialog::answer` / `dialog::choose`
    (`WebEvent::DialogAsked`, `FilesAsked`, `DialogWithdrawn` on Chromium's
    reset). Before this, CEF cancelled every page dialog (no parent window).
  - Page dialogs queue behind permission asks (`App::dialogs`, one shows
    at a time, only with no field open). Alert / confirm / leave are
    `Confirm::Dialog`, drawn as a question sheet where the field goes
    (`palette::question_layer`, wrapped message, at most 10 lines; site
    as title; keys at its foot). A prompt is a field (`Target::Prompt`)
    with the sheet under it; closing the field cancels. A dialog of a card
    that closed or slept is cancelled, except "leave", which is allowed
    (the card is gone already; closing a card doesn't ask yet).
  - Every yes/no question: Enter or y yes; n, Esc or any other key no (as
    before, a stray key is the safe answer). The sheet lists `y Enter OK
    · n Esc Cancel`; `draw_hints` now groups a key with no label with the
    hint after it (`↑ ↓ Move` reads as one group too).
  - File chooser (`Target::Files`, `pathfield`): `~` or an absolute path;
    Tab completes as far as names agree (a lone folder gets `/`; dotfiles
    only for a dot); `*` in the last part picks several where the page
    takes several (one only for a single-file chooser, which says how many
    fit); a folder, missing file or missing parent keeps the field open
    with a toast. Starts in the session's last folder (else home); Save
    starts with Chromium's suggested name. The page's accept list shows at
    the field's end. No system file dialog (portal) fallback.
  - `Ctrl+e` (`edit-in-editor`) and `Shift+Insert` (`paste-primary`) are
    a new default `insert` table (`keys::insert`), on the `?` sheet as
    *Typing in a page*. The field is read in the focused frame under a
    token (`web::field`; text inputs, text areas, editable content;
    password fields are left out) and its text goes to
    `$XDG_RUNTIME_DIR/trailboard/field-N.txt` (0600). The editor:
    `TRAILBOARD_EDITOR` as is (`{file}` or appended; for editors with
    their own window), else `$VISUAL` / `$EDITOR` / `vi` in
    `xdg-terminal-exec`, `$TERMINAL -e`, or the first known terminal;
    words split at spaces. When it exits the text goes back through
    `execCommand('insertText')` (so the page's handlers and undo see it;
    a value setter plus `input` / `change` if that fails); one trailing
    line end is dropped; unchanged text is left alone. An editor that
    returns within 2 s without saving (a terminal handing off to a
    running one) is followed by watching the file for its first save. A
    gone field or card keeps the file and says where.
  - Shift+Insert reads the primary selection (`wl-paste --primary`, else
    arboard) and pastes it like Ctrl+V's text path (`Page::paste_text`).
  - Verified e2e (`~/.cache/trailboard-e2e-m12`, page `site12/d/dialogs.html`
    on :8765, `run13.sh script50 data50`, `script52`–`54`): the alert sheet
    with a wrapped two-paragraph message (`d_alert.png`), y → `undefined`;
    confirm n → `false`, Enter → `true` (`d_confirm.png`); prompt with
    *Ada* selected and the question under it (`d_prompt.png`), typed
    *Grace* → `Grace`, Esc → `null`; multi-file chooser: typed path, Tab
    → `upload/`, `*.txt` → a.txt and b.txt sent (`d_files.png`); the
    single-file chooser (accepts `.txt image/*`) refused `*.txt` (2 fit),
    then `b` Tab → b.txt sent; Ctrl+e in the textarea with
    `TRAILBOARD_EDITOR` = a stand-in script → the new two-line text in the
    field and *Text put back* (`d_edited.png`); the same through
    `xdg-terminal-exec` → kitty (`script54`); Shift+Insert appended the
    primary selection set from the shell (`d_paste.png`); `:set location
    ask` then the location button → the ask, y allowed it (`d_perm.png`);
    arm leave, r → "Reload this page?" (`d_leave2.png`), n stayed, r Enter
    reloaded; closing the card with leave armed closed it.
    Not verified: X11, the light theme, a real keyboard, `showSaveFilePicker`
    / folder choosers, a page in an iframe asking, many dialogs at once.
  - Known: CEF logs "Default dialog implementation requires a parent
    window handle" once after each answered "leave" question although our
    handler answered it (the page does leave or stay as answered).
    Children the app starts can't own a Wayland selection here: `wl-copy`
    run by the app (Y's primary copy, `:run wl-copy`) stays alive but the
    selection reads empty; from a shell it works. To look into (step 12/14
    code, not this step's).
- ✅ 16. **Search and messages.** `?` searches backward (**Owner,
  2026-11-20:** yes; the shortcut sheet moves to `F1` and `:keys`); Esc clears the
  find highlight; `:messages` lists the last toasts and errors.
  - `?` is `find-back` in the page table (`keys::Action::FindBack`): the
    find bar with `App::find_back` set, placeholder *Find up the page*;
    typing and Enter search up, Shift+Enter down. n follows the last
    search's way, N the other (vim's rule); `/`, Ctrl+F and `:find` search
    down again.
  - Decided: `?` on the board (no card entered) still opens the sheet, as
    there is nothing to search there; F1 and `:keys` / `:help` open it
    anywhere. The empty board's chip, the palette row and the welcome page
    say F1.
  - Esc: `App::find_marks` is the card whose page still shows find marks
    (after n / N, or after the bar closed on a search, which leaves the
    active match selected). An Esc with that card current clears them
    (`Page::clear_find`: Chromium's stop-finding with *clear selection*,
    then the page's selection is emptied) and does nothing else; the next
    Esc does what Esc did before. Starting the caret (v) or a new load
    forgets the marks.
  - `:messages` (`scene::messages`): every toast is logged (the last 200,
    this session only; a toast repeating straight after itself counts up).
    The list (`Target::Messages`, `palette::message_rows`) shows waiting
    `:later` lines first (soonest first, "runs in 3 min"), then messages
    newest first with when and how many times. Enter copies a message /
    runs a waiting line now; Shift+Delete or Ctrl+Delete forgets / cancels
    (`:later` lines now have ids, `LaterLine`). `:messages clear` empties
    the log. "Errors" are the toasts that report them: there is no separate
    error log, and command-line mistakes shown under the line (not as a
    toast) aren't listed.
  - Verified e2e (`~/.cache/trailboard-e2e-m12`, page `site12/f/find.html`
    on :8765, `run13.sh script60 data60`): `?fox` → *Find up the page*
    with Enter *Previous* (`f_back.png`), Enter → match 6 of 6 (it
    wrapped up from the top); Esc closed the bar leaving fox 6 selected
    (`f_left.png`, marks kept); Esc cleared it and stayed in Page mode
    (`f_clear.png`); n marked the matches again (`f_n.png`); `/fox` then
    searched down; F1 showed the sheet (`f_sheet.png`); `:later 60000
    reload` then `:messages` → the waiting line and its toast
    (`f_msgs.png`); Shift+Delete cancelled the line (*Cancelled :reload*
    listed, `f_msgs2.png`); `:messages clear` → *No messages yet*
    (`f_empty.png`). Not verified: a real keyboard, X11, the light theme,
    `?` inside iframes; n after Esc restarts from the view (Chromium
    starts a new search once the selection is gone), not from the match
    that was cleared.
- ✅ **Bug fixes (2026-11-21, from `docs/BUGS.md`).**
  - *Ctrl keys while typing and Ctrl+W.* Owner: "fix all", on the plan in
    BUGS.md. Ctrl+W alone never closes a card: on the board and in a page
    it only starts `Ctrl+w` sequences (`keys::bind_window_keys`, shared),
    and waits for the second key with the panel showing (Esc drops it).
    c / q close; the second key may keep Ctrl held (Ctrl+w Ctrl+h, as in
    vim; the sheet hides these twins). A key that doesn't finish a started
    sequence is swallowed even with Ctrl (`Pending::feed`), so Ctrl+w
    Ctrl+r never reloads; after a lone count Ctrl keys still reach the app
    shortcuts. While typing (`keys::typing_key`): Insert turns Ctrl+W /
    Ctrl+U / Ctrl+H into the page's own Ctrl+Backspace / Shift+Home and
    Backspace / Backspace (`App::edit_page_field`, so frames work too) and
    gives Ctrl+R to the page; other shortcuts (palette, address bar, new
    card…) stay, since they lose nothing typed. Passthrough gives the page
    every Ctrl+letter but Ctrl+Q and Ctrl+C / X / V (the clipboard bridge,
    which forwards them anyway); Ctrl+Shift keys stay ours. The app-level
    Ctrl+W close is gone (only Caret / Visual reached it). Page-mode Ctrl+R /
    E / Y / V / L / F / B keep their meanings (owner's earlier choices).
    Known: Ctrl+U at a line's start deletes the line break before it.
  - *Link hints inside frames* (also the backlog's "link hints inside
    iframes"). `links::frame_script` is an idle listener added to every
    frame below the main one at load start (web pages). The hint script
    (`links::script`) collects the main frame's targets in document order
    with visible frames as places to fill, asks each frame by
    `postMessage` with a one-time token (answers count only from that
    frame's window with that token), and each frame does the same for its
    own frames, 3 levels deep. Waits are 150 / 100 / 50 ms by level, none
    when no frame is in view. Answers are shifted by the frame's content
    box (border and padding), trimmed to it and the view, and dropped
    where `elementFromPoint` isn't the frame. One message as before, so
    parsing, labels and clicking are unchanged; the 500 limit counts the
    whole page. Not covered: frames without scripts, scaled or rotated
    frames (unscaled box), closed shadow DOM.
  - Verified: unit tests (`keys`, `userkeys`, `links`); `place` checked
    under node; scripted runs: Insert Ctrl+W / H / U / R, Page and board
    Ctrl+W with a pause (card kept), Ctrl+w Ctrl+h, Ctrl+w c / q,
    Passthrough Ctrl+W to the page; a page on localhost:8765 with a frame
    from 127.0.0.1:8766 and one inside it: f labels the checkbox, link,
    field and nested button, choosing them ticks, clicks and types;
    screenshot of the labels.
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

**Last session:** 2026-11-21 (thirty-fifth). Fixed both bugs in
`docs/BUGS.md` (see M12 "Bug fixes (2026-11-21)"): Ctrl+W alone never
closes a card (board and page wait for its second key; Ctrl held works),
Insert turns Ctrl+W / U / H into field edits and gives Ctrl+R to the page,
Passthrough gives the page every Ctrl+letter but Ctrl+Q and copy/paste;
link hints reach into frames, cross-site and nested, through an idle
listener in each frame.
**Repo state:** on `master`; M12 steps 1–16 and the bug fixes done and
committed; step 17 and M13 planned. `docs/BUGS.md` is new
and lists none open.
**Next up:** M12 step 17, stretch (macros `q` / `@`, `:screenshot`,
`:print`, opt-in user scripts). M13 (full media support, our own CEF
build) runs beside M12; its builds need a cloud machine. Also retake the
README screenshots (`docs/images/`) in the new look.
**Notes:** `:later` lines still aren't saved across quits (`:messages`
now lists and cancels them). The messages log is this session's only
and holds toasts; command-line mistakes shown under the line aren't in
it. After Esc clears a find, n starts again from the view. Caret mode
works in the main frame's text only (not iframes). `wl-copy` started by
the app doesn't take the selection on this machine (step 15's *Known*).
Closing a card doesn't ask a page's "leave this page?" first.

**Small follow-ups (backlog):** the page inspector in a card (`wi`, step
12); the primary selection set from our window on Hyprland (step 12);
cookies on / off in the page menu (it
offers JavaScript and images); an entered card that navigates to a
Passthrough site stays in Page mode until it is given the keys again;
glide on undo/redo of moves; follow the
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
to zoom it; keyboard selection of several cards; a portal file chooser for Save as Markdown; open `crash.log` from
the crash toast; undoing the close of a stack member puts it back in the
stack; gi on the settings page toasts "No text field here" although its
search box is an `input type=search` in view (seen in step 8's e2e; a
click into it works).

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

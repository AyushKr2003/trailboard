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
| ⬜ | **M12 Keyboard first** | Vim-like modes, page keys, scroll sections, commands, user keys (below) |

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
  Shift+h/j/k/l pan, +/- zoom, f link hints (open beside), Shift+F fit, d/u
  scroll the current page half a height (a wheel at its middle), v select,
  z fill, M arrange, N note, Shift+N sticky, T tidy, S fold, `[`/`]` stack,
  O outline, R/C trail name/colour, B sidebar, Tab Solo, Delete close,
  Q quit, `?` sheet. Ctrl+K palette, Ctrl+L address, Ctrl+F find, Ctrl+H
  history, Ctrl+J downloads, Ctrl+T/W new/close, Alt+arrows back/forward.
- Bare keys are never taken from a text field.

---

### M12: Keyboard first ⬜

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
- ⬜ 4. Page keys: H/L back/forward, r/R reload, `/` with n/N, gi first
  field, yy/yt/ym copy URL/title/Markdown link, p/P open the clipboard URL
  here/beside, o/O address bar here/beside.
- ⬜ 5. Hint kinds with `;`: copy a link, download, open an image, open
  beside in the background, focus a field; a "keep hinting" mode.
- ⬜ 6. Command line `:` in the palette, our own command names with
  arguments, completion and history.
- ⬜ 7. Jumps and marks: Ctrl+o/Ctrl+i through visited cards; `m{a-z}` /
  `'{a-z}` for board spots (camera + current card) and scroll positions.
- ⬜ 8. User keys: a versioned key file read by `storage` (tested parser);
  the `?` sheet built from the live binding table.
- ⬜ 9. Visual/caret mode (stretch): select text with v, copy with y.

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

**Not yet asked of the owner:** extra vim keys (Ctrl+w h/j/k/l between
cards, `:q`/`:e`/`:w`, `zz`/`zt`/`zb` with board z moving to `zf`). Decide
with step 6.

**Risks:** sites with their own single-letter keys (Passthrough and the
per-site setting cover them); running the scroll script inside cross-origin
iframes is unverified (the wheel fallback still scrolls them); sites that
fake scrolling with JS aren't found as sections (the wheel at the view's
middle still reaches them); a key typed right after a field gains focus,
before the focus message arrives (a click in a page keeps giving the keys at
once).

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

**Last session:** 2026-11-17 (twenty-first). Built M12 step 3, scroll
sections (`web::scroll` page script, `scene::scrolltarget`, `s` / `]s` /
`[s` / `S`, the outline and the pill), replacing step 2's interim
wheel-at-the-middle scrolling. Page and board scroll keys now go through
the card's scroll target.
**Repo state:** on `master`; M12 steps 1–2 committed (`e6a09af`); step 3
uncommitted.
**Next up:** the owner tries steps 1–3 and decides on the rest (owner
answer 4). Until then: the per-site "start in Insert" setting left from
step 2, and retaking the README screenshots (`docs/images/`) in the new
look.

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

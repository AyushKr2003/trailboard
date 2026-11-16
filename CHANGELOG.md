# Changelog

All notable changes to Trailboard. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses
[Semantic Versioning](https://semver.org/) once it has releases.

## [Unreleased]

### Added
- **Browsing settings** (Ctrl+,), the parts of Chromium's own settings page
  that Trailboard can offer: *Default page zoom*, *Page language* (what sites
  are asked for, and the spelling dictionary), *Check spelling* (with fixes
  and *Add to dictionary* in the page menu), *Block third-party cookies*,
  *Ask sites not to track* (`DNT`/`Sec-GPC`), *JavaScript*, *Images*,
  *Camera*, *Microphone*, *Location*, *Notifications* (Ask, Allow or Block;
  asked, Enter allows and Esc blocks), *Save downloads to* (Downloads,
  Desktop, or ask for a path) and *Proxy* (System, None or a server).
  *Clear cookies* in Ctrl+K deletes every cookie after asking. The page menu
  turns JavaScript or images on or off for one site (`sites.json`).

### Changed
- **A new shortcut sheet** (`?` or F1): a panel at most three quarters of
  the window (smaller when it needs less) instead of one filling it, with
  the sections in one to three balanced columns as the width allows. What
  doesn't fit scrolls (wheel, arrows, Page Up/Down, Home/End, with a thin
  scrollbar) under a header that stays put, rather than shrinking the
  text. Typing filters it (Esc clears the filter, then closes), keys show
  as keycaps beside what they do, and a click outside closes it.
- **Ad blocking works like uBlock Origin**, so sites that open in Firefox or
  Chrome with uBlock no longer ask you to turn the blocker off here. uBlock
  Origin's own lists (filters, badware, privacy, Unbreak, Quick fixes) and
  Peter Lowe's list join EasyList, EasyPrivacy and the cookie list, with
  their `!#include` parts and `!#if` blocks. A blocked ad or analytics script
  the lists name a stand-in for is answered with a harmless one (empty
  scripts, images and media, Google ads, Tag Manager, Analytics and IMA video
  ads), so pages that check their ads loaded carry on. The lists' scriptlets
  (`##+js(…)`) run in the page before its own scripts, defusing ad-blocker
  detection. Blocking is in place before the first page loads.
- **Settings is a page**: Ctrl+, opens a settings page as a card on the
  board, like other browsers' settings, instead of the list under the
  address field. Sections down the side (Search, Appearance, Privacy, Site
  content, Permissions, Languages, Downloads, Performance and network,
  About), a search box, a note under each setting, switches for on/off,
  buttons or a drop-down for choices, a field for the proxy server, and
  buttons for history, downloads, allowed sites and clearing data.
- **Smoother board scrolling**: a wheel notch now glides the board about
  twice as far as before, easing out like a page, and quick notches add up;
  the touchpad pans faster (2.5× its movement) and keeps going briefly
  after a flick. Both follow the *Scroll speed* setting. Grabbing the board
  or a camera move stops a glide.
- **Cleaner start page**: flat, without the accent glow, the dot grid,
  drop-shadows or gradient badges; a smaller name over the bar; a plain
  accent border on focus. Once something is typed, the bar says what Enter
  will do ("Search DuckDuckGo" or "Go to address", by the address bar's
  own rule). The floating help button became a "Which engine is for me?"
  link under the bar.
- **App pages follow Hyprland's rounding**: the settings page, welcome
  page, start page, search engine guide and notes' code blocks take their
  corners from `decoration:rounding`, like Omarchy's apps: sharp on
  Omarchy's default, small parts capped at 4px, circles stay round.
- **Read Mode has no title bar**: the page fills the whole window, top
  to bottom. Tab or Esc goes back to the board; a loading page shows a
  thin line across its top edge.
- **Start page**: starting with an empty board and no URL opens a blank
  *New page* card with the Trailboard name and icon over one search box
  (the chosen search engine, or an address) instead of a random Wikipedia
  article. Enter loads what was typed in the same card, which then becomes
  an ordinary page; Ctrl+L on it does the same. A picker on the left of
  the box switches search engine (grouped: privacy first, most popular,
  answers), and a help button opens a **search engine guide** card that
  explains each engine, who it suits and what it trades, with a button to
  use it. Each engine shows its own icon, loaded from the web (its
  letters on a coloured badge while it loads or if it can't). *New start
  page* in Ctrl+K opens another one.

### Added
- **YouTube ads blocked**: videos start without the ads before and during
  them, and feed, sidebar and banner ads are gone. YouTube serves ads from
  the same servers as its videos, so the filter lists couldn't stop them;
  Trailboard now removes them from the video's data before the player
  sees them, and skips any that still play (muted and sped through, then
  your sound and speed come back). YouTube's "ad blockers are not allowed"
  dialog is taken away and the video plays on. Works on youtube.com,
  m.youtube.com, music.youtube.com and YouTube videos embedded in other
  sites, under the same *Block ads and trackers* setting and per-site toggle.
- **Settings → New card (Ctrl+T) opens**: *Address bar* (the floating bar
  to type an address or search, as before) or *Start page* (a new start
  page card with the search box and engine picker). Also applies to *New
  card* in Ctrl+K and on the welcome page. Saved as `new_card` in
  `settings.json`; older files keep the address bar.
- **Search engines**: Mojeek, Oscobo, MetaGer, Gibiru, Swisscows,
  WolframAlpha, Qwant and SearXNG (a public instance, searx.be), alongside
  the existing seven.
- **Stacks**: a folded trail looks more like a stack: the cards behind its
  front stand further out and up to three of them show (was two).
- **Welcome page**: the first start now shows a designed welcome page (our
  own HTML, following the theme live) instead of a Markdown note: the idea,
  first steps, where the keys go, the shortcuts worth knowing and what to
  expect, with buttons that open a page, the palette, the shortcuts and the
  settings. *Welcome page* in Ctrl+K brings it back.
- **Scrollbars**: pages show thin overlay scrollbars that appear while
  scrolling and fade when idle (Chromium's `OverlayScrollbar`), instead of
  the classic grey bar with arrow buttons on every card. They take no width
  from the page; pages that style their own scrollbars keep them.
- **Read Mode transition**: Tab grows the card out of its place into the
  window (200 ms, eased) with the board dimming behind it, and leaving
  shrinks it back into its place. It used to be an instant cut.

### Fixed
- **Labels stay on one line.** A card's title or address that is too long
  for its title bar ends in "…" before the × instead of wrapping onto a
  second line. Every other label (palette, menus, sidebar, trail names,
  address bar) does the same: the renderer cuts any text at its clip's
  right edge.
- **Keypad keys**: with Num Lock off, the keypad's Page Up, Page Down,
  Home, End, arrows, Insert and Delete reach pages as those very keys, so
  pages can't tell them from the dedicated ones. With Num Lock on, the
  keypad's digits are digits to Trailboard's shortcuts too (they used to
  count as Page Up, Home and the rest, whatever Num Lock said).
- **A card's title shows once.** Zoomed out a little, the title showed both
  in the title bar and in the name label under the card. The label now
  appears only once the title bar is too small to draw its title.
- **Sidebar**: moving the pointer over the sidebar of a space with no rows
  (e.g. just after switching spaces) no longer crashes the app.
- **No squashed pages after a resize**: a page's last frame is drawn at the
  size it was painted at (cropped, or filled out with the page's edge
  colour) until Chromium paints the new size. Leaving Read Mode used to
  squash the window-sized page into the card for a few frames.

### Added
- **Open beside from the palette**: Shift+Enter (or Shift+click) on a page
  row in Ctrl+K or Ctrl+H, or on the palette's last row, opens the page
  beside the current card, joined to it by a trail edge like a clicked link,
  and makes it current. Shift+Enter in the address bar (Ctrl+L, Ctrl+T) does
  the same. With no card on the board it opens in the middle as Enter does.
- **Rename a trail from the sidebar or its tag**: in the trails sidebar, R
  names the highlighted row's trail (a card on no trail becomes a trail of
  its own), and a double-click on a trail's header does the same without
  collapsing it. On the board, a click on a trail's tag above its first
  card opens the name box too. The name box now sits beside an open
  sidebar instead of over its top.
- **See the sites where ads are allowed**: *Sites with ads allowed* in
  Ctrl+K lists every site where you turned blocking off, with how many
  cards show it and "Until you quit" for those allowed from a private
  space. Enter, a click or Shift+Delete blocks ads on one again.
- **Hidden elements are counted**: the shield's menu now says how many
  ad slots and cookie banners Trailboard hid on the page as well as how
  many requests it blocked (*Allow ads on … · 22 blocked · 2 hidden*), and
  the count on the title bar includes them, so a page where something was
  only hidden shows the shield too.
- **A new trails sidebar**: B now opens a sheet floating over the board
  rather than a panel stuck to the window's edge. It shows the space's
  name and how many cards and trails it holds, keeps trails apart with
  space instead of lines, labels the pinned trails, shows each page's site
  icon (a letter when the site has none), and lines a trail's cards up
  under it with faint guides. Its foot holds a chip for every space: click
  one to switch, + to make a new one.
- **Peek at the sidebar**: rest the pointer on the window's left edge and
  the sidebar slides in; move away and it goes. Press B while it peeks to
  keep it open.
- **Click the shield on a title bar** to see what Trailboard did on the
  site: a short menu opens under it with *Allow ads on …* (and *Cookie
  banner rejected* when it applies), without going through the page's
  right-click menu.
- **Your own filter rules apply as you save them**: edits to
  `filters/user.txt` take effect a moment after saving, without a restart,
  and a short message says how many rules it now holds. Reload a page to
  apply them to what it already loaded.
- **Blocked count on the title bar**: a page where Trailboard stopped ads
  or trackers shows a small shield and how many requests it blocked, next
  to the close button. Allowing ads on the site removes it.
- **"Cookie banner rejected" in the page menu**: when Trailboard clicked a
  cookie banner's reject button for you, the page's right-click menu says
  so, above *Allow ads on …*.
- **Sandbox state in the settings**: the settings panel (Ctrl+,) has a
  *Sandbox* line saying whether pages run in Chromium's sandbox and how;
  pick it for an explanation. When the system can't run the sandbox,
  Trailboard now says so once on screen instead of only in its log.
- **Pages run in Chromium's sandbox**: web pages are now confined the way
  they are in Chrome (user namespaces and seccomp), so a page that exploits
  a browser bug has a much harder time reaching your files. Where the system doesn't support
  that, Trailboard still starts, without the sandbox, and says so in its
  log. The Arch package installs Chromium's `chrome-sandbox` helper for
  kernels without user namespaces. `TRAILBOARD_NO_SANDBOX=1` turns it off
  for one run.
- **Cookie banners answered with "reject"**: on sites using a well-known
  consent platform (OneTrust, Didomi, Cookiebot, Sourcepoint, Quantcast,
  Usercentrics and others), Trailboard clicks the banner's reject button
  for you, even when the banner is already hidden, so the site stops
  asking. A page left unable to scroll once its banner is gone scrolls
  again. The *Block ads and trackers* setting and the per-site toggle
  cover this too.
- **Cookie banners and ad slots hidden**: besides stopping requests, the
  blocker now hides the page elements its lists name, such as consent
  pop-ups and the empty boxes ads leave behind, in every frame of a page,
  including elements that appear later. A third list, the EasyList Cookie
  List, is downloaded with the others. The per-site toggle and the *Block
  ads and trackers* setting cover this too.
- **Ad and tracker blocking**: pages no longer load ads and trackers that
  EasyList and EasyPrivacy know about. The lists are downloaded on the first
  start and kept up to date every few days, in `filters/` in the data
  folder; add your own rules in `filters/user.txt`. Right-click a page for
  *Allow ads on this site* (it shows how many requests were blocked) or use
  *Allow or block ads on this site* in Ctrl+K; the choice is remembered,
  except in a private space. *Block ads and trackers* in the settings
  (Ctrl+,) turns it off everywhere.
- `LICENSE-APACHE`; the project is licensed Apache-2.0 only (`Cargo.toml`, `PKGBUILD`).

### Changed
- **A more polished look**: panels, menus, toasts and labels have softer,
  deeper shadows, a fine edge and a faint top highlight. The Ctrl+K palette
  (and history, settings, downloads) is one sheet over a dimmed board, with
  the highlighted row marked by an accent bar and commands' shortcuts shown
  as keycaps. Overlays ease in instead of popping up, and the sidebar slides
  in. Cards cast layered shadows, the current card glows softly in the
  accent colour, and title bars get a crisper edge. The board darkens
  slightly towards the corners, and an empty board shows its hint with
  keycaps. Corners still follow Hyprland's rounding.

### Fixed
- Opening the shortcut sheet (and other text-heavy overlays) after a lot of
  use could crash Trailboard.

### Added
- **A welcome note on the first start**: a new board begins with a short
  guide to getting around beside the first page. It's an ordinary note, so it
  can be edited, kept or closed with Ctrl+W. *Welcome note* in Ctrl+K
  brings it back.
- **An Arch Linux package**: `cd packaging/arch && makepkg -si` builds
  Trailboard with its own copy of the Chromium runtime and installs it to
  `/usr/lib/trailboard`, with `trailboard` on the `PATH` and the launcher
  entry and icon. No `install-cef.sh` or `.cargo/config.toml` needed.
- **A self-contained build**: `scripts/bundle.sh` makes a folder with the
  release binary and the Chromium runtime beside it (with `--tar`, a
  `.tar.zst` of it, about 150 MB) that runs from wherever it is unpacked;
  `scripts/install-desktop.sh` can point the launcher entry at it.
- **`trailboard --version` and `--help`**, answered without starting the
  browser (and never handed to a Trailboard that is already running).
- **Crash reports**: if Trailboard panics, a report with the version, the
  message and a backtrace is added to `crash.log` in the data folder, and the
  next start says so once.
- **Scrolling like a desktop browser**: touchpad scrolling inside a page is
  about 4× faster than before (it moved the page 1:1 with the fingers) and a
  flick keeps the page going after you lift your fingers, slowing down
  smoothly. A mouse wheel notch scrolls 120 px (was 100) with Chromium's
  smooth glide. New *Scroll speed* setting (Ctrl+,): Slow, Normal, Fast,
  Faster. The test script runner gains `touchpad dx dy [end]`.
- **Trails sidebar: pins and order**: in the sidebar (B), `p` pins a trail
  (or a lone card) to the top, marked ★; Shift+J / Shift+K move a trail up
  or down, and dragging a trail's header moves it too. Pins and order are
  saved with the board (board format v6; older boards upgrade on load) and
  Ctrl+Z undoes them. The test script runner gains `press` and `release`.
- **Downloads**: files go straight to your Downloads folder (from
  `user-dirs.dirs`, or `TRAILBOARD_DOWNLOAD_DIR`), numbered "name (1).ext"
  when the name is taken; a toast says when one starts and ends. A card
  opened for a link that turns out to be a file closes itself and the view
  returns to the page. **Ctrl+J** (or *Downloads* in Ctrl+K) lists them with
  live progress: Enter opens a file (or downloads again one that failed),
  Ctrl+Enter opens its folder, Shift+Delete cancels a running download or
  takes a finished one off the list. The list is kept in `downloads.json`;
  downloads from a private space are not saved. The page menu gains
  *Download link* and *Save image*. Downloads keep going when their card
  sleeps, closes or you switch spaces; quitting stops them and removes the
  partial files.
- **Settings**: Ctrl+, (or *Settings* in Ctrl+K) lists the settings under the
  top field; type to filter, Enter or → changes the highlighted one, ← or
  Shift+Enter goes back, a click changes a row too. Changes apply at once and
  are kept in `settings.json` in the data folder. The settings: the **search
  engine** for the address bar and the palette (DuckDuckGo, Google, Bing,
  Brave Search, Startpage, Ecosia or Kagi), how many **pages are kept awake**
  (2–16, default 6; `TRAILBOARD_MAX_LIVE` still overrides it for one run),
  **keep history** (off stops recording the pages you open), and **page
  colours** (follow the theme, or always light or dark pages).
- **Desktop entry and icon**: `scripts/install-desktop.sh` installs
  `trailboard.desktop` and its icon for your user, so Trailboard shows in the
  app launcher and can be chosen as the default browser.
- **Opening pages in the running window**: starting Trailboard again while it
  runs (from the launcher, a shell, or a link in another app) opens the given
  pages as new cards in the running window instead of failing. Several pages
  can be given at once, and file paths are read from the directory you ran it
  in.
- **Page thumbnails**: a small picture of each page you open is kept with
  its history (for the newest 1000 pages). Ctrl+K and Ctrl+H show the
  highlighted page's picture beside the list (or under it in a narrow
  window), and cards asleep after a restart show their page instead of
  "Click to load" until they wake. Forgetting a page or clearing history
  removes its picture; private spaces take none. `page-text.db` is now
  version 2 (older files are upgraded; older builds refuse the new file).
- **Closed cards in the palette**: Ctrl+K also finds the cards you closed in
  this space since starting Trailboard, by title, address, a note's text or
  the words on the page ("Closed · example.com"). With nothing typed the
  three most recent are listed after the open cards. Enter brings the card
  back where it was, in its trail, as Ctrl+Shift+T does (and can be undone).
- **Private spaces**: Ctrl+Shift+P (or *Private space on or off* in the
  palette) makes the current space private; *New private space* makes a new
  one. Pages in a private space are not kept in history, their site icons and
  page zoom are not saved, and their cookies, site data and cache stay in
  memory, shared by the space's cards until you quit. The board itself is
  still saved. The space's label says "· Private". `spaces.json` is now
  version 3 (older files are upgraded; older builds refuse the new file).
- **History view**: Ctrl+H (or *History* in the palette) lists the pages
  you opened, newest first, with when you saw each ("Today 14:32",
  "Yesterday", a weekday, a date). Type to keep the pages with those words;
  Enter or a click opens one in a new card, or goes to the card already
  showing it. Ctrl+H or Esc closes it.
- **Forget a page**: Shift+Delete on a page in the history view, or on a
  history result in the Ctrl+K palette, removes it and its text from history.
- **Clear history**: a palette command that forgets every page, after asking
  (Enter clears, Esc keeps them). Forgotten text is wiped from the file, not
  just hidden.
- **History search**: every page you open is remembered with its title and
  text (in `page-text.db` in the data folder, newest 5000 pages, for all
  spaces). The Ctrl+K palette now finds pages by the words in them: an open
  card whose page mentions what you typed, and pages from history, which
  open in a new card. Error pages and `data:` pages are not kept. Script
  command `history` logs the index and a search of it.
- **Site icons in title bars**: each card shows its site's icon (favicon)
  before its title. Icons are kept per site and saved (in `icons/` in the data
  folder), so sleeping cards and boards restored after a restart show them
  too, and stay visible when the board is zoomed out past the titles.
- **Card names when zoomed out**: when titles are too small to read, the card
  under the pointer, or else the current card, shows its icon and title in a
  label below it. Script command `icons` logs the known icons.
- **Everything by keyboard**: every mouse action now has a key too.
  - f labels buttons, checkboxes, text fields, selects and other controls as
    well as links: typing a label clicks it, and a field gets the keys so you
    can type in it (a note's task boxes can be ticked this way).
  - v selects or deselects the current card; moving with h/j/k/l keeps the
    selection, so several cards can be picked, then moved (M) or closed.
  - z fills the view with the current card.
  - d / u scroll the current card's page down / up half a page.
  - The Menu key or Shift+F10 opens the page menu (as a right-click does) at
    the selection or the focused field; menus take j/k as well as the arrows.
  - Palette commands *Select or deselect card*, *Fill the view with card*,
    *Page menu*.
- **Find in page**: Ctrl+F opens a find bar for the current card's page.
  Matches are highlighted as you type; Enter and Shift+Enter step to the next
  and previous one, and the bar shows how many there are ("3 of 17", "No
  matches"). Esc closes it. It opens with the last search.
- **Page zoom**: Ctrl++ and Ctrl+- zoom the current card's page (25% to 500%),
  Ctrl+0 puts it back to 100%. As in other browsers the zoom belongs to the
  site: every card on it follows, and it is remembered (in `zoom.json`, for
  all spaces). A toast shows the new zoom. Link hints follow zoomed pages.
- Palette commands *Find in page*, *Zoom page in*, *Zoom page out* and
  *Reset page zoom*. Script commands `zooms` and `find`.
- **Ctrl+K palette**: one box to find any card on the board (folded ones
  too, by title, address or a note's text), switch to another space, or run
  a board command by name (with its shortcut shown beside it). Letters can be
  skipped ("rcc" finds *Reopen closed card*); matched letters are
  highlighted. With nothing typed it lists the cards, most recently used
  first. ↑/↓, Ctrl+N/P or PgUp/PgDn pick, Enter or a click goes there or runs
  it, Esc or Ctrl+K closes. The last row searches the web for what you typed
  (or opens it first, if it is an address). Script `palette` logs its rows.
- **Trails sidebar**: press B with no page having the keys for a list of
  everything on the board down the left edge: each trail with its name,
  colour and size, its cards as a tree in reading order, then the cards on no
  trail. j/k (or the arrows) go to the card on that row, even one folded
  away in a stack; h/l collapse and expand a trail; Enter types in the card's
  page; Esc or B closes it. Clicking a row does the same, and the wheel
  scrolls it. Script `sidebar` logs its rows.
- **Sticky notes**: press Shift+N with no page having the keys for a small
  coloured note without a title bar, ready to type in (Markdown, like any
  note). Drag its coloured strip to move it; C gives it the next colour
  (yellow, green, cyan, blue, magenta, red, from the desktop theme). A note's
  right-click menu has *Make sticky note* / *Make note card*. Colour changes
  are undoable and saved with the board.
- Board files are now version 5 (adds sticky notes); older boards are
  upgraded when read. Script `cards` logs each sticky note's colour.
- **Move and resize by keyboard**: press M with no page having the keys to
  arrange the current card. h/j/k/l or the arrows move it (with the rest of
  the selection if it is selected), Shift with them resizes it, Ctrl takes
  small steps. Enter or M keeps the result as one undo step, Esc puts it back.
  A pill at the bottom shows the keys while you arrange.
- **Link hints**: press f with no page having the keys to label the links
  you can see in the current card's page (or the one in the middle of the
  view) with short home-row labels. Type a label to open that link beside the
  card, which becomes the current card, so f follows on from there. Backspace
  takes a letter back; Esc, another key, a click or a scroll cancels.

### Changed
- h/j/k/l and the arrows no longer clear the selection.
- Ctrl+0 now resets the current page's zoom; it fits everything only when no
  card is current. Shift+F always fits everything.
- Fit everything moved from F to Shift+F (Ctrl+0 still works), because f
  now shows link hints.

- **Keyboard-driven board**, the way Omarchy's apps work: h/j/k/l or the
  arrow keys go to the nearest card in that direction, which becomes the
  current card (thin accent ring). Enter or Space gives its page the keys
  (thick ring; a note opens for writing), Esc gives them back to the board
  with the card still current, and Esc again lets go of it. Shift with
  h/j/k/l or the arrows pans, + and - zoom, Delete closes the current card and
  Q quits. The shortcut sheet has a Keyboard section.

- **Save as Markdown**: Ctrl+S saves the focused note as a Markdown file, or,
  with no note focused, the whole board: each trail as a nested list of links
  under its name, notes in full, pages on no trail at the end. A note's
  right-click menu has *Save as Markdown* too. Files go to
  `Documents/Trailboard/` (from `XDG_DOCUMENTS_DIR` or `user-dirs.dirs`),
  named after the note or space; a toast shows where. Saving again in the same
  session updates that file; otherwise an existing file is never overwritten.
- **Trail names and colours**: press R with no page focused to name the trail
  in the middle of the view (an empty name clears it), and C to give it the
  next colour of the desktop theme (back to the accent after red). The name
  shows on a tag in the trail's colour above its first card (or its stack's
  front), the trail's edges take its colour, and stepping along it with
  Alt+PgDn shows the name in the toast. Links opened from a named trail join
  it with its name and colour, closing any of its cards keeps them, and both
  are undoable and saved with the board.
- Board files are now version 4 (adds trail names and colours); older boards
  are upgraded when read. Script `cards` logs each card's trail mark and the
  trail labels.
- **Notes**: press N with no page focused for a note card. Notes are written
  in Markdown: type in the note, then Esc or click away to see it rendered
  (headings, emphasis, links, lists, task lists, quotes, code). Click it to edit
  again, at the line you clicked; tick a task's box without editing. Enter
  continues a list. Links in a note open beside it. Notes follow the theme,
  are saved with the board, and take their title from their first line.
- **Add to note**: right-click selected text in a page and choose *Add to
  note* to quote it, with a link back to the page, at the end of the note you
  used last; with no note yet, a new note opens beside the page on its trail.
- Board files are now version 3 (adds notes); version 1 and 2 boards are
  upgraded when read.
- Script `cards` logs each note's text; `type` lines may contain `#`.
- **Next/Previous on Trail**: Alt+PgDn / Alt+PgUp go to the next / previous
  page of the trail you are on, in reading order, and focus it (also from
  inside a page and in Read Mode, where the page reached is read). Pages
  folded into a stack come to its front in place. A toast shows `2/4 · Title`,
  or the start or end of the trail.
- **Stack outline**: click a stack's count badge, or press O with no page
  focused, to list the pages in the stack. Pick one (click, or arrows and
  Enter) to bring it to the front, or choose *Unfold trail*. Script command
  `badgeclick ID` clicks a stack's badge.
- **Fold**: press S with no page focused to fold the trail in the middle of the
  view into a stack: only the card you were looking at stays on the board, with
  the edges of the others behind it, `2/4 ·` in its title and a count badge on
  its corner. `[` and `]` peek at the previous and next page of the stack in
  its place; S again unfolds the trail around the stack, as it was. The other
  pages sleep while folded. Fold and unfold are undoable, and stacks are saved
  with the board. Script `cards` logs folded cards (`hidden=`) and stacks.
- Board files are now version 2 (adds stacks); version 1 boards are upgraded
  when read and newer ones are refused, left untouched.
- **Tidy**: press T with no page focused to lay the board out again: each trail
  reads left to right from its first page, one column per step, with nothing
  overlapping; cards without a trail line up in rows below. Card sizes are kept,
  the card in the middle of the view stays where it is on screen, and Ctrl+Z
  undoes the whole Tidy in one step.
- **Read Mode**: Tab makes the focused card (or the one in the middle of the
  view) fill the window, with its page laid out at the window's size under a
  slim title bar. Tab, Esc or a double-click on the bar returns to the board as
  it was. Links opened while reading are added beside on the board and read
  next. After clicking inside a page, Tab goes to the page (form fields); Esc
  still leaves. Script `cards` logs the Read Mode state.
- **Omarchy look**: Trailboard follows the current Omarchy theme live, with its
  palette for the board, cards, address bar, menus, labels and the shortcut
  sheet, light themes as well as dark. Pages get the theme's light or dark
  `prefers-color-scheme`. Corners follow Hyprland's window rounding (asked
  again whenever Hyprland reloads its config), and overlays follow the
  desktop's text size (`omarchy display text size`). Without Omarchy it uses a
  dark palette with Omarchy's yellow accent.
- F1 shows the shortcut sheet from anywhere (also inside a page); Ctrl+Q quits.
- The window's Wayland app id is `trailboard`.
- **Spaces**: several boards, each with its own cards, camera and background
  tint. Ctrl+Shift+N makes a space, Ctrl+PageDown / Ctrl+PageUp go to the
  next / previous one, F2 renames it, Ctrl+Shift+B steps its tint (a theme colour)
  and Ctrl+Shift+W deletes it (Enter confirms when it has cards; the last space
  can't be deleted). The current space shows in the top-left corner when there
  is more than one, and switching shows a short toast. Undo history and closed
  cards are kept per space while the app runs.
- Spaces are stored as `spaces.json` (version 2; version 1 files are upgraded)
  plus `spaces/<id>.json`. The old
  `board.json` becomes the first space; a missing or unreadable index is rebuilt
  from the board files; a deleted space's board is moved to `spaces/deleted/`.
- Script command `spaces` logs every space and which one is active.
- **Multi-select and group move**: Shift+drag on the board draws a selection band
  (cards it touches are added), Shift+click a title bar adds or removes a card, and
  Ctrl+A (no card focused) selects all. Dragging a selected card's title bar moves
  the whole selection; Delete or Backspace closes it. A click on the board or Esc
  clears the selection. Group moves and closes undo in one step, and closed cards
  get their trail edges back.
- **Shortcut sheet**: press `?` with no card focused to see keyboard and mouse
  shortcuts; Esc, `?` or a click hides it. The empty board mentions it.
- Script commands `sdrag` and `sclick` (Shift held); `cards` logs whether each card is selected.
- **Page context menu**: right-click in a page for Open link beside, Copy link
  address, Open/Copy image address, Undo/Redo/Cut/Copy/Paste/Select all in text
  fields, Copy and Search for selected text, or Back/Forward/Reload. Arrow keys
  and Enter work; Esc or a click outside closes it.
- **Desktop clipboard**: Ctrl+C / Ctrl+X in a page now reach other apps, and
  Ctrl+V pastes text copied elsewhere. The address bar supports Ctrl+V, and
  Ctrl+C / Ctrl+X after Ctrl+A.
- **Input methods**: IME composition (e.g. fcitx5 Pinyin) in pages, with the
  candidate window at the text caret; committed text in the address bar.
- Script commands `pclick`/`prclick` (click at page coordinates), `key`, `type`,
  `ime-preedit`, `ime-commit` and `menuclick`.
- **Dropdowns and popups in pages**: `<select>` lists and other popup widgets are
  drawn over the page and can be clicked.
- **Page cursors**: the pointer shows the page's cursor (hand over links, text
  beam, resize arrows, hidden for `cursor: none`).
- Script command `cards` also logs each card's title.
- **Live-browser cap**: at most 6 cards (set with `TRAILBOARD_MAX_LIVE`) keep a
  live browser: the focused card, then cards on screen, then the most recently
  used. The others sleep and show their last frame, dimmed, and wake when clicked
  or brought into view. On start, pages load only as they come on screen.
- **Undo/redo for board edits**: Ctrl+Z, Ctrl+Shift+Z or Ctrl+Y undo and redo
  opening, closing, moving and resizing cards. A restored card gets its live page
  and trail edges back. After a click inside a page, Ctrl+Z goes to the page.
- Script commands `drag`, `undo`, `redo` and `cards` for end-to-end runs.
- **Reverse-engineering notes** for Marble 0.1.28 (macOS), gathered with REA:
  architecture, module map, 1,549 recovered function names, SQLite page-text
  schema, board JSON structures, menus/shortcuts and the Explain API contract
  (`docs/re-findings.md`, `docs/marble-function-names.txt`).
- **Cargo workspace** with crates `scene`, `storage`, `web`, `compositor` and `app` (binary `trailboard`).
- **Board model** (`scene`): cards, trail edges, z-order, hit testing, bounds;
  camera with pan, zoom-about-point and fit; placement beside a source card and
  free placement near a point.
- **Chromium integration** (`web`): CEF 156 off-screen rendering, subprocess
  bootstrap, per-card browsers, paint buffers, mouse/wheel/keyboard forwarding,
  title/URL/loading events, and "open links beside the page" (link clicks,
  popups, Ctrl/middle-click).
- **Compositor** (`compositor`): wgpu renderer with dot-grid background, rounded
  and textured cards, soft shadows, focus ring, bezier trail edges and layered
  glyphon text.
- **App** (`trailboard`): winit window, pan/zoom, focus, move/resize/close/reopen
  cards, double-click to fill the view, omnibox with URL/search detection,
  layout-aware keyboard shortcuts, zoom-aware page resolution and pausing
  off-screen pages.
- **Persistence** (`storage`): board saved atomically to
  `$XDG_DATA_HOME/trailboard/board.json`, autosaved every 3 s and on exit, and
  restored on start. An unreadable board file is moved aside, not overwritten.
- **Scripted end-to-end runs** via `TRAILBOARD_SCRIPT` (click, move, wheel, fit, open, wait, log, quit).
- `scripts/install-cef.sh` installs the CEF build that matches the `cef` crate
  (works around `export-cef-dir` failing on CEF's >10 MB build index).
- README, plan (`docs/PLAN.md`), future plans (`docs/FUTURE.md`) and `CLAUDE.md`.

### Changed
- Board keys (F, N, T, S, O, R, C, `[`, `]`, `?`, Ctrl+A, Ctrl+Z) now work
  whenever no page has the keys, also with a card current, and act on the
  current card before the one in the middle of the view. A title-bar click
  or Alt+PgDn makes a card current without sending it your keys; clicking in
  the page, Enter, or a new card from the address bar does.
- N makes a note instead of opening the address bar for a new card (Ctrl+T
  still does that). Ctrl+L on a note opens the address bar for a new card.
- T on the board now tidies it; Ctrl+T still opens a new card.
- Ctrl+Shift+T now restores the closed card's trail edges and skips cards
  already brought back with undo.

### Fixed
- Text was drawn darker than its colour (it was gamma-decoded twice); light
  greys and theme colours now come out as specified.
- Cards' bottom corners are rounded like the rest of the card; the page image
  used to square them off (new `Shape::RoundBottom` in the compositor).
- Bold text (card titles) used a fallback serif face with wide gaps between
  words and digits; it now uses the sans font's bold face.
- Zoomed-in pages are sharp again: pages now render at the zoom's resolution
  (CEF ignored the device scale we reported, so pages always rendered at 1×).
- Shortcuts now follow the logical key instead of the physical keycode, so they
  work on non-QWERTY layouts and with virtual keyboards (a remapped key was
  being read as Escape).

### Known issues
- Copying uses the page's plain selected text, so pages that change what is
  copied, and "copy" buttons using `navigator.clipboard`, don't reach the desktop
  clipboard. Desktop text is pasted as plain text.
- Text copied from Trailboard leaves the clipboard when Trailboard quits.
- Pages see `devicePixelRatio` equal to the board's raster scale (0.5–2), and a page may lay out once at a stale zoom right after navigating to a new site.
- Custom image cursors (`cursor: url(…)`) show the default arrow.
- A card that wakes from sleep reloads its page, losing scroll position and form input.
- Typing is tested through scripted key events, not real keyboard input.

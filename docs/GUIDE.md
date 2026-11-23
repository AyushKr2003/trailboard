# Trailboard guide

The full reference: every shortcut, what each feature does, and where your
data lives. For the short version, see the [README](../README.md).

- [Keys and actions](#keys-and-actions)
- [Cards and labels](#cards-and-labels)
- [History, downloads, blocking, settings](#history-downloads-blocking-settings)
- [Running it](#running-it)
- [Tests](#tests)

## Keys and actions

The first start opens a welcome page beside the first page: what Trailboard
is, your first minute, where the keys go, the shortcuts worth knowing and what
to expect (sleeping cards, saving, blocking). Its buttons open a page, the
palette, the shortcuts or the settings. Close it with d, and bring it back
with *Welcome page* in Ctrl+K.

| Action | Input |
|---|---|
| Pan | drag empty board, middle-drag, or scroll over the board |
| Zoom | Ctrl + scroll |
| Go to the next card left / down / up / right | h / j / k / l or the arrow keys (the card becomes current, with a thin ring) |
| Type in the current card's page | Enter or Space, or click in the page (a thick ring shows it has the keys) |
| Back to the board / let go of the card | Esc / Esc again |
| Pan / zoom with the keyboard | Shift + h/j/k/l or Shift + arrows / + and - |
| Do a board key several times | type a number first: 3j goes three cards down, 2+ zooms in twice (shown at the bottom while you type; Esc drops it) |
| Move / resize a card with the keyboard | M, then h/j/k/l (Shift resizes, Ctrl takes fine steps); Enter keeps, Esc puts it back |
| Follow a link by keyboard | f labels the links in the current card's page, inside its frames too (embedded forms, a captcha's box, even from other sites); type a label to open it beside (Esc cancels) |
| Do something else with a link, image or field | ; then a letter labels only what that letter uses; type a label: ;y copies a link's address, ;d downloads a link or image, ;i opens an image beside, ;b opens a link beside while you stay where you are, ;m does that for as many links as you like (the labels stay up; Esc ends), ;e types in a field, ;o puts a link's address in `:open` to edit first, ;h moves the pointer over something (menus that open on hover), ;f labels everything clickable (also things only a pointer cursor marks) and clicks it, ;I opens an image here, ;Y puts a link's address in the primary selection. Works on the board and in a page; the window title says what the labels do |
| Close the current card / reopen the last closed / quit | Delete or d / u / Q (Ctrl+Q works everywhere) |
| Focus a page | click it (keyboard and scroll then go to the page) |
| Move / resize a card | drag the label above it / its bottom-right corner |
| Select cards | Shift+drag on the board, Shift+click labels, Ctrl+A (Esc clears) |
| Move / close selected cards | drag a selected card's label / Delete |
| Fill the view with a card | double-click its label |
| Solo (the card fills the window) | Tab (Tab or Esc to go back) |
| Use a page from the keyboard (after Enter, or in Solo) | j/k scroll, h/l sideways, Ctrl+d/Ctrl+u half a section, gg/G top/bottom, 0/$ left/right edge, f follow a link here, F beside, H/L back/forward, r reload, / find, ? find backward (up the page), n/N the next match the way you searched / the other way, Esc clears the find's marks, yy copy the address, yt the title, ym a Markdown link, p/P open the copied address (or search for the copied text) here/beside, o address bar, O an address bar that opens beside; a number repeats (5j). The card's ring shows the mode (accent for *Page*) and the window title names it: nothing is drawn over the page. Mid-sequence (g, y…), Ctrl+G lists what can follow |
| Move about by address (in a page) | ]] / [[ the page's next / previous page (its *next* link, else a link saying Next, Older, », ›…); gu up the address's path (3gu three steps), gU the site's root; Ctrl+a / Ctrl+x the last number in the address up / down (5 Ctrl+a: by five; `page/7` → `page/8`); go / gO edit the address, opening here / beside; xo an address bar that opens beside and keeps you here; Ctrl+s stops loading (when nothing loads it saves as Markdown, as before); gh a start page beside the card; gf the page's source beside; gd downloads the page; F11 full screen. By name: `:page-next`, `:up`, `:root`, `:increment`, `:stop`, `:home`, `:view-source`, `:save-file`, `:fullscreen` |
| Inspect a page (its elements, styles, console, network…) | F12 or Ctrl+Shift+I opens the page inspector inside the current card, on the right of the page (the page lays out narrower, as in a browser tab); the same key closes it (from the inspector too). Right-click → *Inspect* opens it on the element you clicked. Drag the divider between them to share the card differently. The buttons above the card move it to the bottom and back, or to a card of its own beside the page (whose button puts it back in). Clicking the inspector gives it every key (Passthrough: Shift+Esc gives our keys back, Esc is the inspector's console); clicking the page gives them back to the page. It closes with its page, is never put to sleep and isn't saved with the board. By name: `:inspect`, `:inspect right`, `:inspect bottom`, `:inspect beside` |
| More copying (in a page) | yd copies the site (`news.example.com`), yp the address without tracking parts (`utm_…`, `fbclid`…); with Shift (yY, yT, yM, yD, yP) they go to the primary selection, which a middle click pastes; pp / PP open the primary selection here / beside (p alone waits a moment for the second p). `:copy site`, `:copy clean` |
| Scroll one part of a page (a sidebar, a list, a wide table, a carousel, a frame) | The scroll keys move the page's *scroll area*: the one you clicked in last, else the one in the middle of the view (an open dialog, the main pane). s labels every scroll area in view (type a label to pick it), ]s / [s step to the next / previous, S to the one around it. At an area's end the scroll goes on to the one around it, as a wheel does. The chosen area flashes an outline and the window title names it (*section 2.1 · 40%*); a new page starts afresh. Ctrl+d/Ctrl+u on the board use the card's scroll area too |
| Go from card to card without leaving the page (cards are your tabs) | J / K the next / previous card in reading order (as Tidy lays them out: each trail from its first card, then the lone cards), gt or T a card by title (the `:card` list), g0 / g$ the first / last, Alt+1…8 the nth and Alt+9 the last, Ctrl+^ the card you were on before, Ctrl+W then h/j/k/l (or Ctrl+h/j/k/l, Ctrl held) the nearest card that way, Ctrl+W then w / W the next / previous, p the card before. The keys stay in the page (*Page*); in Solo, Solo follows. Alt+1…9, Ctrl+^ and the Ctrl+W keys work on the board too. Ctrl+W alone does nothing: it waits for its second key (Esc drops it). `:card 3`, `:card-next`, `:card-prev` do the same from the command line |
| Go back to cards you visited | Ctrl+O / Ctrl+I go back / forward through the cards you were on in this space, oldest first, each card once (a number goes that many). Kept across restarts. Works on the board and in a page; `:jump-back`, `:jump-forward` |
| Marks: come back to a spot | In a page, m then a lower-case letter marks where the page is scrolled (the scroll area too); ' then the letter scrolls back there, loading the page again if the card has moved on. m then an upper-case letter (M on the board, where m moves the card) marks the board spot: the space, the card and the view; ' then that letter goes back, from any space. '' goes back to where the last jump left from (another card, a mark), and again returns. Marks are kept across restarts; `:mark a`, `:to-mark A` |
| Saved pages (named, like bookmarks) | `:save-page` saves the current page under its title, `:save-page Rust docs` under a name (the same name replaces it). b lists them to open one here, B beside the card (on the board and in a page; the newest first, type to narrow); Shift+Delete forgets the highlighted one. `:saved name`, `:saved-beside name`. Kept in `saved.json`, shared by every space |
| Your own keys | `:keys-file` opens `~/.config/trailboard/keys` in a card (made with commented examples the first time). One entry a line: `bind <modes> <keys> <command>`, `unbind <modes> <keys>`, `alias <name> <command>`; modes are board, page, insert, hints, command and visual (`board,page` for both). Keys are written `gs`, `<C-p>` (Ctrl+P), `<A-x>` (Alt+X), `<S-Tab>`, `<Space>`, `<F5>`; in insert, hints and command a binding is one key with Ctrl or Alt, or an F key. A command is a key's name (`scroll-down`, `top`, `next-card`, `hints-here`…), an alias or a `:` command line, several chained with `\|`; `{url}` and `{title}` are the current page's. `:reload-keys` reads the file again (a wrong line is named with its number; the rest still load). `:bind`, `:unbind`, `:alias` do the same until restart. The F1 sheet lists every key as bound, yours included; after a key that starts others (g, ;, m) a panel lists what can follow |
| Start a program on a page or link | `:run program words…` starts it without a shell (quote words with spaces), `{url}` and `{title}` filled in; `:hint command` labels the links and runs the command on the one you choose with `{hint-url}` its address (`:hint-many` until Esc). For example, in the keys file: `alias play run mpv {hint-url}` and `bind page ;v hint play` |
| Close / reopen / clone cards from a page | d closes the card and goes on to the next one (or Ctrl+W then c or q); u reopens the last closed card (a number reopens that many); co closes every other card in the space after asking (`:card-only`; `:undo` brings them back); gC opens the page again beside it (`:clone`) |
| Type in a page | click a field, choose one with f, or press gi (first field) or i (anywhere); the ring turns green and the title says *Insert*. Esc leaves the field and gives the keys back; Esc again goes to the board. While typing, Ctrl+W deletes the word before the caret, Ctrl+U back to the line's start and Ctrl+H a character, as in our own fields; Ctrl+R goes to the page (no reload) |
| Edit a field in your editor | Ctrl+e while typing in a field: its text opens in `$VISUAL` / `$EDITOR` (in a terminal: `xdg-terminal-exec`, `$TERMINAL`, or the first one found); save and quit, and the text goes back into the field. `TRAILBOARD_EDITOR` names an editor that opens its own window instead (`gvim -f`, `code --wait`; `{file}` marks where the file goes). Password fields stay out |
| Paste the primary selection | Shift+Insert while typing in a field (what you last selected anywhere, as a middle click pastes) |
| Answer a page's dialogs | a page's `alert`, `confirm` and "Leave this page?" show as a sheet near the top, naming the site: Enter or y is OK (Leave), Esc, n or any other key Cancel (Stay). A `prompt` is a field with the page's question under it: Enter sends what you typed, Esc cancels. Our own questions (delete a space, clear history…) take y and n too |
| Choose files for a page | a page's *Choose file* opens a path field instead of a window, in the folder you last chose from: Tab completes, Enter chooses, Esc cancels. `~` is your home; a `*` in the last part picks several files where the page takes several (`~/Pictures/*.png`). It says what the page accepts (`.pdf image/*`); a wrong path keeps the field open and says why |
| Select and copy a page's text from the keyboard | v in a page puts a caret in its text (the ring turns magenta and the title says *Caret*): at the match after a / find, at a selection or click in view, else at the start of the first line in view. h j k l (or the arrows) move it, w / b / e by words, 0 / $ to the line's ends, { / } by paragraphs, gg / G to the page's ends; a number repeats (3w); the page scrolls to keep it in view. v starts a selection from there (*Visual*), V selects whole lines (*Visual line*; V from a page starts there); o moves the other end. y or Enter copies and goes back to *Page*, Y puts it in the primary selection. Esc drops the selection, then the caret. With the mouse's selection in view, v goes straight to *Visual* with it. Your own keys: mode `visual`, names like `caret-word`, `copy-selection` |
| Let a site have every key (its own shortcuts) | Ctrl+V in a page; the ring turns red and the title says *Passthrough* (in Solo only the title does, so if keys seem to do nothing, look there); Shift+Esc ends it. The site gets every Ctrl+letter too (Ctrl+W, Ctrl+R, Ctrl+K…) except Ctrl+Q; copy and paste work as usual |
| Close card / reopen closed | × on its label, d or Delete on the board, Ctrl+W then c / Ctrl+Shift+T |
| Back / forward / reload with the pointer | the arrows on the label of the card under the pointer |
| Board menu: new page, note, tidy, fold, fit, history, settings | right-click the empty board |
| Spaces menu | click the space's name, top left (the button beside it opens the sidebar); *Show the space panel* in the settings hides both |
| Undo / redo board edits | Ctrl+Z / Ctrl+Shift+Z or Ctrl+Y (after clicking the board or a card's label) |
| Address bar / new card | Ctrl+L / Ctrl+T; Shift+Enter opens what you typed beside the current card instead |
| Search one site by keyword | Type a keyword, a space and your search in the address bar, o, O, `:open` or the palette: `gh trailboard` searches GitHub, `w rust` Wikipedia, `g …` Google, `yt …` YouTube. The bar names the site it will search. Change, add or remove keywords in Settings → *Search keywords* (click one to edit it; *Defaults* brings the original set back) |
| Start page: a search box and the pages you opened last (none in a private space) | *New start page* in Ctrl+K, or *New card opens: Start page* in the settings for Ctrl+T; ↓ from the box walks the pages, Enter opens one in the card |
| Pin a site to the start page | the pin on a *Jump back in* card, or *Add a site* in a group; a site's ⋯ or right-click edits, moves or unpins it; drag to reorder or move between groups |
| Open a start page link beside it (with a trail) | Ctrl+click or middle-click; Alt+1–9 opens the first nine pinned sites |
| Choose what the start page shows (clock and 12/24-hour time, greeting and name, recent cards, groups) | *Customise*, top right of the start page |
| Settings search | `/` in the settings page, Enter goes to the first match |
| Palette: find a card (folded ones too, or by words in its page), a card you closed, a page from history, another space or a command | Ctrl+K, type a few letters, ↑/↓ (or Ctrl+N/P) and Enter, or click a row; the last row searches the web or opens what you typed. Shift+Enter (or Shift+click) opens a page from history or the last row beside the current card, joined to it like a link |
| New note (Markdown) | N (while no page has the keys); click a note to edit, Esc or click away to see it rendered |
| New sticky note / its next colour | Shift+N / C on it (while no page has the keys); drag its coloured strip to move it; *Make sticky note* / *Make note card* in a note's right-click menu |
| Quote selected text in a note | right-click the selection → *Add to note* (the note last used, or a new one beside the page) |
| Save as Markdown | Ctrl+S: the focused note, else the whole board (its trails as nested lists, notes in full); also *Save as Markdown* in a note's right-click menu. Files go to `Documents/Trailboard/` |
| Fit everything | Shift+F (while no page has the keys), or Ctrl+0 with no card current |
| Trails sidebar: every trail as a tree, then the other cards | Ctrl+B (from a page too; it takes the keys on the board): j/k go to a card, h/l collapse / expand a trail, Enter types in the card's page, p pins a trail to the top, r or a double-click on a header names a trail, Shift+J/K or dragging a header moves a trail, Esc or Ctrl+B closes; or click a row. Rest the pointer on the window's left edge to peek at it; it slides away when the pointer leaves. Its top has back, forward and reload for the current card, the address pill (a press edits the current card's address), the space's name (a press opens the spaces menu) and *New card*; a card row shows a close button under the pointer. The dots at its foot switch spaces, + makes a new one, the gear opens the settings |
| Tidy: lay trails out left to right, no overlaps | T (while no page has the keys; Ctrl+Z undoes it) |
| Fold the current trail (or the one in the middle of the view) into a stack / unfold it | S (while no page has the keys; Ctrl+Z undoes it) |
| Peek at the previous / next card of a stack | [ / ] (while no page has the keys) |
| List a stack's pages, bring one to the front or unfold | Click the count badge, or O (while no page has the keys) |
| Go to the next / previous card on the trail (folded or not) | Alt+PgDn / Alt+PgUp |
| Name the current trail / give it the next colour | R / C (while no page has the keys; Ctrl+Z undoes it), or click the trail's tag above its first card to name it |
| Back / forward / reload | Alt+← / Alt+→ / Ctrl+R |
| Click a link, button, checkbox or field by keyboard | f, then type its label (links open beside; a field gets the keys) |
| Select / deselect the current card | v (h/j/k/l keep the selection) |
| Fill the view with the current card | z |
| Scroll the current page without entering it | Ctrl+d / Ctrl+u |
| Page menu by keyboard | Menu key or Shift+F10; j/k or ↑/↓, Enter, Esc |
| Command line: run a command by name, with what it needs (`:open rust docs`, `:space Work`, `:zoom out`, `:back 2`) | `:` on the board or in a page (or type `:` first in Ctrl+K). The list offers the commands, or the values a command takes (spaces, card titles, choices); Tab and Shift+Tab walk the list, putting each row's line in the field, Enter runs it (or, for a command still missing its value, fills it in), Ctrl+Enter runs it and keeps the line open. A unique start of a name is enough (`:tid`); `:q` closes the card, `:w` saves as Markdown, `:e` opens, `:exit` quits. Lines you ran come back under *Recent* (Shift+Delete or Ctrl+Delete forgets one; not kept in a private space), and Up / Down bring back earlier lines that start with what you typed |
| Edit the line from the keyboard | In the command line and every other field: Ctrl+W deletes the word before the caret (a whole address), Alt+Backspace or Ctrl+Backspace a smaller word, Alt+D the word after, Ctrl+U back to the start, Ctrl+Y puts back what was deleted; Alt+B / Alt+F or Ctrl+Left / Ctrl+Right move a word, Ctrl+E to the end. In the command line Ctrl+A goes to the start and Ctrl+K deletes to the end (elsewhere Ctrl+A selects all, and in Ctrl+K's palette Ctrl+K closes it) |
| Run a command again, several times, or later | `.` runs the last command line again. A number before `:` or before a command counts: `3:` opens the line as `:3 `, and `:3 back` goes back three pages; a command that takes no count runs that many times (`:3 card-next` steps three cards, `2.` runs the last line twice). `:repeat 3 reload` runs a command several times (up to 100), `:later 2000 reload` after two seconds (in milliseconds, up to a day). To repeat a chain, write its bars as `\\|`: `:repeat 2 back \\| tidy` |
| History: pages you opened, newest first | Ctrl+H, type to filter by words in them, Enter opens (Shift+Enter beside the current card), Shift+Delete forgets the page; *Clear history* in Ctrl+K forgets all |
| Downloads: files saved, newest first | Ctrl+J (or *Downloads* in Ctrl+K), type to filter; Enter opens the file (or downloads again one that didn't finish), Ctrl+Enter opens its folder, Shift+Delete cancels a running download or takes a finished one off the list |
| Settings: search engine, default page zoom, page language, spelling, pages kept awake, history, ads, third-party cookies, Do Not Track, JavaScript, images, camera / microphone / location / notifications, where downloads go, proxy, page colours, scroll speed | Ctrl+, (or *Settings* in Ctrl+K) opens the settings page as a card (or goes to it): sections down the side, a search box, switches, buttons and drop-downs. Changes apply and save at once |
| Settings from the keyboard | `:set name value` (the list offers each setting with its value now, then its values; `:set name` alone shows it), `:reset name` (back to the default), `:toggle name` (flip a switch, or the next choice; `:toggle search google bing` goes round those). Names are the settings page's, with dashes: `page-colours`, `search`, `default-zoom`, `history`, `block-ads`, `proxy-server`, `corner-radius`… |
| Toggles: `t` then a letter, on the board and in a page | td pages dark or light, th the space panel, ts the sidebar; tj / ti / tc / ta JavaScript, images, cookies, ads for the current card's site (saved; the site's pages reload), the same with Shift (tJ, tI, tC, tA) until you quit; tp the site starts in Passthrough. By name: `:site cookies off`, `:site-session javascript on` (without on/off it flips) |
| Find in the current page | Ctrl+F (or / in a page; ? searches up the page), type (matches show as you type); Enter / Shift+Enter next / previous; Esc closes, and Esc again clears the match it leaves selected |
| Zoom the current page in / out / back to the default | Ctrl++ / Ctrl+- / Ctrl+0: every card on that site follows, and the zoom is remembered per site |
| Page context menu | right-click in a page (open link beside, copy or download link, save image, edit, search selection, spelling suggestions in a text field, JavaScript or images on / off for the site) |
| Copy / cut / paste | Ctrl+C / Ctrl+X / Ctrl+V (shared with the desktop clipboard) |
| Leave the page | Esc |
| New space / next / previous | Ctrl+Shift+N / Ctrl+PageDown / Ctrl+PageUp |
| Rename space / next tint | F2 / Ctrl+Shift+B |
| Space's background photo | *Background photo…* in the space's menu or the palette: the desktop's wallpaper, a picture from the theme's backgrounds or `~/Pictures`, or type a picture's path (JPEG, PNG or WebP) |
| Delete space | Ctrl+Shift+W (asks first if it has cards) |
| Private space on / off | Ctrl+Shift+P: pages there aren't kept in history, their site icons and zoom aren't saved, and cookies live in memory until you quit (the board itself is still saved) |
| Show shortcuts | F1, `:keys`, or ? on the board (in a page ? finds backward) |
| See what was said | `:messages` lists the messages shown this session (newest first, repeats counted) and the `:later` lines still waiting. Type to filter; Enter copies a message or runs a waiting line now; Shift+Delete forgets a message or cancels a line; `:messages clear` empties it |
| Quit | Ctrl+Q |

Input methods (e.g. fcitx5) work in pages and in the address bar.

## Cards and labels

New cards are shaped like a full window on an empty workspace (asked of
Hyprland), else like the screen, else 16:9, at 1024 wide.

A card is its page and nothing else. Its name sits on a label above it, the
same size on screen at any zoom: the site's icon (kept per site under
`icons/` in the data folder, so sleeping cards and restored boards keep
theirs) and the title. On the card under the pointer and on the current card
the label also shows the site's address, a shield with what the blocker
stopped, and back, forward, reload and close, with the card's awake button
before them (see below) and a speaker while it plays media. A line glides
along the card's top edge while the page loads, and a sleeping card's label
ends in a moon.
When no card is in view, a line at the foot of the window offers *Fit
everything*; an empty board shows a short welcome on the space's colour,
with *Open a page* (Ctrl+T), *Search everything* (Ctrl+K) and *All keys* (?).

The interface is set in Inter (bundled, SIL OFL). Its design rules (levels,
text strengths, type scale, icons) are in [docs/DESIGN.md](docs/DESIGN.md).

## History, downloads, blocking, settings

Pages you open are remembered with their text in `page-text.db` in the data
folder (the newest 5000), so Ctrl+K can find them by any word on them.
Ctrl+H lists them newest first, with when you saw each; type to keep the
pages with those words, Enter opens one (or goes to the card showing it), and
Shift+Delete forgets the highlighted page (in the Ctrl+K palette too). The
palette command *Clear history* forgets every page after asking. Turn *Keep
history* off in the settings (Ctrl+,) to stop recording pages; what is kept
stays searchable.
A small picture of each page is kept too (for the newest 1000): it shows
beside the highlighted page in Ctrl+K and Ctrl+H, and on sleeping cards
restored after a restart until they load again.

Files a page downloads go straight to your Downloads folder (`XDG_DOWNLOAD_DIR`
from `~/.config/user-dirs.dirs`, else `~/Downloads`; `TRAILBOARD_DOWNLOAD_DIR`
overrides it), as "name (1).ext" if the name is taken, without asking.
*Save downloads to* in the settings picks the Desktop instead, or *Ask each
time*: the path waits in a field at the top (Enter saves, Esc cancels). A card
opened for a link that turns out to be a file closes again and the view goes
back to the page. Ctrl+J lists the downloads with their progress; the list is
kept in `downloads.json` in the data folder (downloads made in a private space
are listed until you quit, not saved). Quitting stops running downloads and
removes their partial files.

Ads, trackers and cookie banners are blocked with the lists uBlock Origin
starts with: EasyList, EasyPrivacy, Peter Lowe's list, uBlock's own filters,
Badware, Privacy, Unbreak and Quick fixes lists, and the EasyList Cookie List
(the `adblock` Rust crate). Requests the lists name are stopped, or answered
with a harmless stand-in where the lists say so (so pages that check their ad
script loaded don't ask you to turn the blocker off); page elements they name
(ad slots, consent pop-ups) are hidden; and their scriptlets run in the page
before its own scripts to defuse ad-blocker detection. The lists are
downloaded to `filters/` in the data folder on the first start and refreshed
every 4 days (Quick fixes daily); your own rules in the same syntax go in
`filters/user.txt` (read again whenever you save it). The page menu's last row turns blocking
off for that site and its subdomains (or back on) and says how many requests
it blocked; *Allow or block ads on this site* in Ctrl+K does the same. Sites
allowed are kept in `privacy.json` (not those allowed from a private space).
*Sites with ads allowed* in Ctrl+K lists them; Enter, a click or
Shift+Delete on one blocks ads there again.
*Block ads and trackers* in the settings turns it off everywhere.

The settings also cover what Chromium's own settings page would (Trailboard
draws pages off screen, so `chrome://settings` isn't there):
- *Default page zoom* for sites without their own (Ctrl+0 goes back to it).
- *Page language*: the languages sites are asked for, and the spelling
  dictionary. *System* follows `$LANG`. Sites see a change after a restart;
  spelling follows at once. *Check spelling* turns the red underlines off;
  right-click a misspelt word for fixes or *Add to dictionary*.
- *Block third-party cookies*, *Ask sites not to track* (`DNT` and `Sec-GPC`
  headers), and *Clear cookies* in Ctrl+K (asks first).
- *JavaScript* and *Images* on or off everywhere; the page menu turns either
  on or off for one site (kept in `sites.json`) and reloads its pages.
- *Camera*, *Microphone*, *Location*, *Notifications*: Ask, Allow or Block.
  Asked, a line at the foot of the window says which site wants what: Enter or y allows,
  Esc, n or any other key blocks, and the answer holds for that site until you quit. Pages only get
  location and the camera on secure sites (`https:` or `localhost`), and
  location has no position source in this Chromium build, so an allowed
  site still hears "position unavailable".
- *Proxy*: System, None, or Custom (asks for `host:port`, or
  `socks5://host:port`); it applies at once.
- *Corners*: Rounded (Trailboard's own, the default), Follow Hyprland (your
  windows' rounding; sharp on Omarchy's default), or Custom (a slider from
  0 to 24 px). Cards, panels, menus and the app's own pages follow at once.
- *Show the page's mode*: Ring and title (the default: the entered card's
  ring takes the mode's colour and the window title names it, so nothing
  covers the page; in Solo only the title does) or Chip in the corner (also
  a label at the window's bottom left, as before). `:set mode-label chip`.
- *Board background*: Colour mood (the default: a soft gradient from the
  space's tint, with fine grain), Photo (the space's own picture, else your
  desktop wallpaper; labels get a small sheet so they read over it), or
  Plain (the space's flat colour).

On [Omarchy](https://omarchy.org/), Trailboard follows the current theme live
(colours, light or dark, including the pages' `prefers-color-scheme`, unless
*Page colours* in the settings says Light or Dark), the desktop text size,
its wallpaper (for *Board background: Photo*) and, with *Corners: Follow
Hyprland*, Hyprland's window rounding. Elsewhere it uses a dark palette with
a yellow accent.

## Running it

`trailboard --help` lists the options. If Trailboard panics, a report (version,
message, backtrace) is appended to `crash.log` in the data folder below, and
the next start says so once.

Pages run inside Chromium's sandbox, which uses unprivileged user namespaces
(on by default on Arch and most distributions; `chrome://sandbox` in a card
shows its state). Where the kernel turns those off (linux-hardened), Chromium
needs its setuid helper `chrome-sandbox` beside the binary instead: the Arch
package installs it that way; in an unpacked bundle, run
`pkexec chown root:root chrome-sandbox && pkexec chmod 4755 chrome-sandbox` in
its folder. With neither, Trailboard still starts, runs pages unconfined and
says so once; the *About* section of the settings (Ctrl+,) shows the state. `TRAILBOARD_NO_SANDBOX=1` turns the sandbox off for one run
(for debugging).

Starting Trailboard again while it runs (from the launcher, a shell or a link
in another app) opens the given pages as new cards in the running window, in
the space on screen, and exits. Arguments can be URLs, hosts, search words or
file paths (relative to where you ran it).

Each space's board is saved to `$XDG_DATA_HOME/trailboard/spaces/<id>.json`
(default `~/.local/share/trailboard/`), listed by `spaces.json`, and restored on
the next start; the browser profile (cookies, cache) lives next to it. A
`board.json` from an earlier version becomes the first space. A deleted space's
board is moved to `spaces/deleted/`, not erased (its jump list and marks
are dropped). Each space's jump list (Ctrl+O / Ctrl+I) and marks are kept in
`jumps.json`, and saved pages in `saved.json`.

Only the 6 most relevant cards keep a live browser (focused first, then on screen,
then most recently used). The rest sleep and show their last frame, dimmed, until
you click them or bring them into view. Change the cap with *Pages kept awake*
in the settings (Ctrl+,), or for one run with `TRAILBOARD_MAX_LIVE=N`.

Some cards stay awake outside that cap, so they never take a slot from the
others:

- **A card playing media** (a video, muted or not, or audible sound) is
  never put to sleep, and keeps playing off screen or folded in a stack
  (it paints slowly while you can't see it). It stays awake for 10 minutes
  after you pause, then follows the cap again.
- **The awake button** on a card's label, a circle, cycles its mode: a dim
  ring is *Auto* (the cap decides), a coloured ring (cyan, or blue in
  themes whose cyan is close to their green) is *Keep awake* (never sleeps;
  off screen it is hidden like any page, so it costs little and its timers
  slow down) and a solid green disc is *Always active* (never sleeps, is
  never hidden and keeps its focus: it runs as if it were the card in
  front of you). A card kept awake shows its ring or disc at the end of
  its label. The mode is
  saved with the board. By name: `:awake`, `:awake auto`, `:awake keep`,
  `:awake active`.
- **The speaker** on a card playing media mutes it, and shows crossed out
  while it is muted. By name: `:mute`.

Every card kept awake is a whole browser, so keep an eye on how many you keep.

Cards glide when Tidy moves them, grow in when they open and shrink away
when they close; a space switch slides its board in. These follow your
desktop: with Hyprland's `animations:enabled` off (or GNOME's
*Animations* off) everything changes at once. `TRAILBOARD_REDUCED_MOTION=1`
turns motion off for one run (`=0` turns it on whatever the desktop says).

## Tests

```sh
cargo test                       # scene, storage, omnibox unit tests
TRAILBOARD_SCRIPT=path cargo run -p trailboard -- URL   # scripted end-to-end run
```

`TRAILBOARD_SCRIPT` drives the real input handlers (`click x y`, `pclick card x y`,
`key ctrl+c`, `type text`, `ime-commit text`, `menuclick label`, `badgeclick id`, `wheel`, `fit`,
`open URL`, `wait ms`, `log`, `quit`); see `crates/app/src/script.rs`.
`TRAILBOARD_SLOW_MOTION=8` makes every board motion 8 times slower, to
screenshot one halfway.


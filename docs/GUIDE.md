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
palette, the shortcuts or the settings. Close it with Ctrl+W, and bring it back
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
| Follow a link by keyboard | f labels the links in the current card's page; type a label to open it beside (Esc cancels) |
| Do something else with a link, image or field | ; then a letter labels only what that letter uses; type a label: ;y copies a link's address, ;d downloads a link or image, ;i opens an image beside, ;b opens a link beside while you stay where you are, ;m does that for as many links as you like (the labels stay up; Esc ends), ;e types in a field. Works on the board and in a page; the corner says what the labels do |
| Close the current card / reopen the last closed / quit | Delete or d / u / Q (Ctrl+Q works everywhere) |
| Focus a page | click it (keyboard and scroll then go to the page) |
| Move / resize a card | drag the label above it / its bottom-right corner |
| Select cards | Shift+drag on the board, Shift+click labels, Ctrl+A (Esc clears) |
| Move / close selected cards | drag a selected card's label / Delete |
| Fill the view with a card | double-click its label |
| Solo (the card fills the window) | Tab (Tab or Esc to go back) |
| Use a page from the keyboard (after Enter, or in Solo) | j/k scroll, h/l sideways, Ctrl+d/Ctrl+u half a section, gg/G top/bottom, 0/$ left/right edge, f follow a link here, F beside, H/L back/forward, r reload, / find, n/N next/previous, yy copy the address, yt the title, ym a Markdown link, p/P open the copied address (or search for the copied text) here/beside, o address bar, O an address bar that opens beside; a number repeats (5j). The corner shows *Page* |
| Scroll one part of a page (a sidebar, a list, a wide table, a carousel, a frame) | The scroll keys move the page's *scroll area*: the one you clicked in last, else the one in the middle of the view (an open dialog, the main pane). s labels every scroll area in view (type a label to pick it), ]s / [s step to the next / previous, S to the one around it. At an area's end the scroll goes on to the one around it, as a wheel does. The chosen area flashes an outline and the corner names it (*section 2.1 · 40%*); a new page starts afresh. Ctrl+d/Ctrl+u on the board use the card's scroll area too |
| Go from card to card without leaving the page (cards are your tabs) | J / K the next / previous card in reading order (as Tidy lays them out: each trail from its first card, then the lone cards), gt or T a card by title (the `:card` list), g0 / g$ the first / last, Alt+1…8 the nth and Alt+9 the last, Ctrl+^ the card you were on before, Ctrl+W then h/j/k/l the nearest card that way. The keys stay in the page (*Page*); in Solo, Solo follows. Alt+1…9 and Ctrl+^ work on the board too. `:card 3`, `:card-next`, `:card-prev` do the same from the command line |
| Close / reopen / clone cards from a page | d closes the card and goes on to the next one (Ctrl+W alone too, after a short pause); u reopens the last closed card (a number reopens that many); co closes every other card in the space after asking (`:card-only`; `:undo` brings them back); gC opens the page again beside it (`:clone`) |
| Type in a page | click a field, choose one with f, or press gi (first field) or i (anywhere); the corner shows *Insert*. Esc leaves the field and gives the keys back; Esc again goes to the board |
| Let a site have every key (its own shortcuts) | Ctrl+V in a page; the corner shows *Passthrough*; Shift+Esc ends it |
| Close card / reopen closed | × on its label or Ctrl+W / Ctrl+Shift+T |
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
| Trails sidebar: every trail as a tree, then the other cards | B (while no page has the keys): j/k go to a card, h/l collapse / expand a trail, Enter types in the card's page, p pins a trail to the top, r or a double-click on a header names a trail, Shift+J/K or dragging a header moves a trail, Esc or B closes; or click a row. Rest the pointer on the window's left edge to peek at it; it slides away when the pointer leaves. Its top has back, forward and reload for the current card, the address pill (a press edits the current card's address), the space's name (a press opens the spaces menu) and *New card*; a card row shows a close button under the pointer. The dots at its foot switch spaces, + makes a new one, the gear opens the settings |
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
| Command line: run a command by name, with what it needs (`:open rust docs`, `:space Work`, `:zoom out`, `:back 2`) | `:` on the board or in a page (or type `:` first in Ctrl+K). The list offers the commands, or the values a command takes (spaces, card titles, choices); Tab puts the highlighted one in the line, Enter runs it. A unique start of a name is enough (`:tid`); `:q` closes the card, `:w` saves as Markdown, `:e` opens, `:exit` quits. Lines you ran come back under *Recent* (Shift+Delete forgets one; not kept in a private space) |
| History: pages you opened, newest first | Ctrl+H, type to filter by words in them, Enter opens (Shift+Enter beside the current card), Shift+Delete forgets the page; *Clear history* in Ctrl+K forgets all |
| Downloads: files saved, newest first | Ctrl+J (or *Downloads* in Ctrl+K), type to filter; Enter opens the file (or downloads again one that didn't finish), Ctrl+Enter opens its folder, Shift+Delete cancels a running download or takes a finished one off the list |
| Settings: search engine, default page zoom, page language, spelling, pages kept awake, history, ads, third-party cookies, Do Not Track, JavaScript, images, camera / microphone / location / notifications, where downloads go, proxy, page colours, scroll speed | Ctrl+, (or *Settings* in Ctrl+K) opens the settings page as a card (or goes to it): sections down the side, a search box, switches, buttons and drop-downs. Changes apply and save at once |
| Find in the current page | Ctrl+F, type (matches show as you type); Enter / Shift+Enter next / previous; Esc closes |
| Zoom the current page in / out / back to the default | Ctrl++ / Ctrl+- / Ctrl+0: every card on that site follows, and the zoom is remembered per site |
| Page context menu | right-click in a page (open link beside, copy or download link, save image, edit, search selection, spelling suggestions in a text field, JavaScript or images on / off for the site) |
| Copy / cut / paste | Ctrl+C / Ctrl+X / Ctrl+V (shared with the desktop clipboard) |
| Leave the page | Esc |
| New space / next / previous | Ctrl+Shift+N / Ctrl+PageDown / Ctrl+PageUp |
| Rename space / next tint | F2 / Ctrl+Shift+B |
| Space's background photo | *Background photo…* in the space's menu or the palette: the desktop's wallpaper, a picture from the theme's backgrounds or `~/Pictures`, or type a picture's path (JPEG, PNG or WebP) |
| Delete space | Ctrl+Shift+W (asks first if it has cards) |
| Private space on / off | Ctrl+Shift+P: pages there aren't kept in history, their site icons and zoom aren't saved, and cookies live in memory until you quit (the board itself is still saved) |
| Show shortcuts | ? (while no page has the keys) or F1 |
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
stopped, and back, forward, reload and close. A line glides along the card's
top edge while the page loads, and a sleeping card's label ends in a moon.
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
  Asked, a line at the foot of the window says which site wants what: Enter allows, Esc
  blocks, and the answer holds for that site until you quit. Pages only get
  location and the camera on secure sites (`https:` or `localhost`), and
  location has no position source in this Chromium build, so an allowed
  site still hears "position unavailable".
- *Proxy*: System, None, or Custom (asks for `host:port`, or
  `socks5://host:port`); it applies at once.
- *Corners*: Rounded (Trailboard's own, the default), Follow Hyprland (your
  windows' rounding; sharp on Omarchy's default), or Custom (a slider from
  0 to 24 px). Cards, panels, menus and the app's own pages follow at once.
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
board is moved to `spaces/deleted/`, not erased.

Only the 6 most relevant cards keep a live browser (focused first, then on screen,
then most recently used). The rest sleep and show their last frame, dimmed, until
you click them or bring them into view. Change the cap with *Pages kept awake*
in the settings (Ctrl+,), or for one run with `TRAILBOARD_MAX_LIVE=N`.

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


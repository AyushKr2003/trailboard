# Trailboard: interface design

How the interface is put together, and why. Code: `crates/app/src/theme.rs`
(colour), `chrome.rs` (surfaces, type, keycaps, tooltips), `icons.rs`,
`hud.rs` (the one on-screen control), and the drawing in `app.rs`,
`palette.rs`, `sidebar.rs`, `hints.rs`.

## Principles

1. **The page is the interface.** A card is its page and nothing else: no
   title bar, no dock, no toolbar. The only thing that stays on screen is a
   small panel at the top left (sidebar button, space name). Owner's
   decision, 2026-11-16.
2. **Always know where you are.** A card's label (above it, the same size
   on screen at any zoom) shows on the current card, the one under the
   pointer, selected cards and stacks; once cards are small on screen
   (zoom under 0.45) every card shows one, so a far-out board keeps its
   landmarks (zoomable interfaces lose people when landmarks shrink away:
   Jul & Furnas, "Critical zones in desert fog", 1998). Up close the pages
   speak for themselves. Labels fade in and out (120 ms). Trail tags and
   stack counts always show: they are marks. A view with no card in it
   offers *Fit everything*. (Quiet labels: owner's decision, 2026-11-17.)
3. **Nothing needs a remembered key.** Each action has a pointer route: a
   card's label (move, back, forward, reload, close, blocker), the board's
   right-click menu, the space's menu, the palette. Menus, the palette's
   foot and tooltips show the key beside the action, which is how keys are
   learnt (Nielsen's heuristics 6 and 7: recognition over recall,
   accelerators for experts).
4. **Quiet chrome.** The accent marks the current card, its trail, the
   chosen row and matched letters; nothing else. Unmarked trails are
   neutral, so colour on the board means something was marked.
5. **One model of depth.** Cards and panels float over the board with
   shadows from one light above; no glows or sheens on controls. The only
   gradient is the board's own colour mood (or its photo), which gives the
   cards something to float over (owner's decision, 2026-11-17).

## Colour

The theme (Omarchy's, or the fallback) gives background, foreground, accent
and named colours. Everything else is derived in OKLab, where equal steps of
lightness look equal whatever the hue:

| Level | What | Token |
|---|---|---|
| 0 | board | `board` |
| 1 | cards, app pages | `card` |
| 2 | floating panels: sidebar, menus, lists, messages | `panel` |
| 3 | under the pointer / chosen | `hover`, `active` |

Dark themes step lighter with each level; light themes go lighter while
there is room below white, then darker. Lines: `line` (hairlines),
`line_strong` (meant to be seen). A recess is `well`.

Text strengths are set by measured contrast on a panel (WCAG 2), not by a
fixed mix: `text` ≥ 7:1, `text2` 4.5:1 (secondary), `text3` 3:1
(placeholders, disabled, decoration only). `accent_text` is the accent moved
until it reads at 4.5:1; `accent_soft` is as much accent on a panel as keeps
text on it at 4.5:1. Tests check these on dark, light, white and
low-contrast themes.

Shadows are the board's own hue, darkened, in two parts (tight contact +
wide ambient), offset and blur growing together with the lift (`Lift::Low`,
`Mid`, `High`). Light themes get weaker ones.

## Type

Inter, bundled (regular and semibold only; semibold is the one emphasis).
One scale, in logical px × the desktop's text scale: 11 caption (keycaps,
counts), 12 small (secondary), 13 body (rows, menus), 18 input (the
palette's and address field), 18 title, 30 display (the one large line an
empty board shows). Section labels in lists are
caption capitals, semibold, spaced 10% (`chrome::spaced`).

## Shape and size

- 4 px grid. Rows 30–36 px, pointer targets at least 24 px (WCAG 2.5.8);
  the top-left controls are 32 px.
- Corners come from the *Corners* setting (owner's decision, 2026-11-17):
  Rounded (10 px, the default), Follow Hyprland (its window rounding, 0 on
  Omarchy's default) or Custom (0–24 px). `Theme::rounding` is the radius
  in use; tokens in `theme.rs`: `card_radius` (the radius), `radius` for
  panels (2 px rounder, so sheets read softer than pages), `small_radius`
  (≤ 6 px), and `card_corner` for a card on screen (scaled with the zoom
  but never under 4 px, nor over a sixth of the card). App pages get it
  as `--radius`.
- Hairlines are one device pixel, drawn inside the edge (`Shape::Ring`).

## Icons

Our own set, line drawings on a 24-unit grid (`icons.rs`), rasterised at the
exact pixel size they are shown at (1.5 px strokes at 16 px), white with
coverage in alpha and tinted when drawn. Add one by adding its strokes; the
tests check it stays inside its box at every size.

## The mark

A keyboard key, seen from a little above, in charcoal: its skirt shows most
at the front, its top is dished. Its legend is what Trailboard does, drawn
in the board's own geometry: a page, a trail that leaves its side and turns
at rounded right angles, and the page it opened, in amber with a soft glow.
A key, because Trailboard is driven from the keyboard; a trail between
pages, because that is what it keeps.

- App icon (`share/trailboard.svg`): the key and its legend, with a soft
  contact shadow. The glow is a filter; a renderer without filters draws
  the legend without it.
- Symbolic (`trailboard-symbolic.svg`, 16 px, one colour): the key's
  outline and the legend.
- The welcome page's mark: the same key at 26 px, in its own colours (not
  the theme's), so the mark looks the same everywhere.

Chosen by the owner on 2026-11-22 after seven rounds of studies (flat
marks of trails, letters and pages, then visual styles; the keycap and
frosted glass styles were the ones liked, and the dark key, "K2", won).

## Motion

Overlays ease out into place in 160 ms and leave at once. Loading is a line
gliding along a card's top edge. Nothing bounces or loops for decoration.
App pages have more motion of their own; see *Motion on app pages* below.

The board moves with cubic ease-out (`scene::motion`), and only in what is
drawn: the model is already at its end, so clicks, saving and undo never
see a card halfway.

| What | How | Time |
|---|---|---|
| Solo in and out | the card grows to the window, board dims | 200 ms |
| Tidy | cards glide from their old places | 220 ms |
| A card opens beside another | grows out of the source's side, 85% → 100%, fading in | 220 ms |
| A card opens elsewhere | grows from 92% in its own middle, fading in | 220 ms |
| A card closes | shrinks to 92% and fades | 160 ms |
| Space switch | the tint cross-fades; the board slides 32 px in from the side the space is on and fades in | 240 ms |
| Sidebar | slides in 150 ms; slides out the same way | 150 ms |
| Sidebar rows | come in from the sheet's left edge, one after another, as it opens | 14 ms apart, 200 ms each |
| Sidebar tab | glides from the old current card's row to the new one (a scroll it follows at once) | 200 ms |

All of it is off when the desktop asks for less motion
(`desktop::reduced_motion`: Hyprland's `animations:enabled`, else GNOME's
`enable-animations`).

## App pages

The welcome, settings, start and note pages are HTML in `data:` URLs
(`*.html` beside their `.rs`). They get the same tokens as CSS variables
(`welcome::colors`, `note::colors`), live through `tbTheme`: the page is
the card level (`--bg`), boxes on it the panel level (`--card`), with
`--hover`, `--well`, `--rule`, `--rulestrong`, three text strengths
(`--fg`, `--dim`, `--faint`), `--accenttext`, `--accentsoft`, `--danger`
and `--shade` (the shadow hue). Same rules as the chrome: the accent only
for a switch that is on, the current section, focus and the main button; a
chosen segment is raised, not coloured; destructive buttons in `--danger`.

### Motion on app pages

The welcome, start and settings pages move more than the chrome, because the
owner asked for motion-led pages (2026-11-21). The rule is that **they move
the way the board moves**: every effect is one of the board's own motions,
not borrowed decoration.

- **Grows from its source.** Things arrive from what led to them: a page
  grows out of its source card's side (85% → 100%, origin at the side), a
  menu or dialog grows out of the button or point that opened it.
- **Lines draw.** Trails, icon strokes and progress draw along their length
  (`pathLength="1"`, dash offset), tied to the scroll where they show it.
- **Scroll is the camera.** The welcome page pins a stage while its run
  scrolls past, and the scroll drives the board's camera through six
  chapters (open beside, step back, fold, spaces, Solo), each a different
  motion. Labels keep their size on screen and show once cards are small;
  dots stay 20–40 px apart, as on the board.
- **Nothing loops.** Motion comes from scroll or input and settles, so a
  page at rest paints nothing (every card is an off-screen renderer). The
  only timer is the start page's clock, whose changed digits turn over once
  a minute.
- **Two curves.** `--out` (the board's ease-out) for what arrives, and
  `--spring` (a `linear()` spring: one overshoot under 4%, settled by 70%)
  for what the hand moves: switches, a choice's sliding thumb, a card that
  leans toward the pointer and springs back.
- **Native first.** Scroll-driven animations (`view()`, `scroll()`,
  `animation-range`), `@starting-style` for arrivals (only on a page's
  first drawing), `sibling-index()` staggers, scroll-state queries for
  headings that dock, view transitions when settings are filtered. Script
  only where choreography needs it (the welcome stage, damped toward the
  scroll at τ = 85 ms).
- **Less motion.** The app passes the desktop's switch as `still` in the
  theme (`welcome::set_still`), along with `prefers-reduced-motion`: no
  transitions or animations, and the welcome stage shows each chapter's
  end state.

They can't load the bundled Inter (a `data:` page has no way to our files,
and embedding it would put a megabyte in every note), so they ask for
`"Inter", "Adwaita Sans", system-ui`: Adwaita Sans is made from Inter.

## The board

- Dots stay 20–40 px apart at any zoom; finer ones fade in while zooming in.
- New cards are shaped like a full window on an empty workspace (else the
  monitor, else 16:9) at 1024 wide: a page on the board has the proportions
  it would have in a window (`placement::card_size`).
- The top-left panel is optional (*Show the space panel*).
- Behind the cards (*Board background*): **Colour mood** (default), the
  space's tint as a two-stop gradient across the window's diagonal
  (`Theme::mood`: hue from the tint, and a neighbouring hue in the far
  corner; lightness from the board, a little lifted top left and lowered
  bottom right, so every tint is equally bright and labels keep 4:1), fine
  grain (±1.5 levels) against banding, dots at 70%. **Photo**: the space's
  picture, else the desktop wallpaper, cut to the window's shape off the
  main thread (`backdrop`), fixed while the board moves, dimmed (0.78 dark,
  0.94 light), no dots; labels get a small panel-coloured sheet (82%) so
  they read over any picture. The mood shows until the photo is read.
  **Plain**: the flat colour with dots.
- Card: the page clipped to the card's rounded shape, hairline (a step
  stronger under the pointer), two-part shadow (wide ambient + tight
  contact) that keeps some size when zoomed out; the current card floats
  higher, the hovered one a touch. Current: 1.5 px accent ring at 80%;
  with the keys: 2.5 px solid in the mode's colour (`Theme::keys_ring`):
  Page the accent, Insert green, Passthrough red, Caret / Visual magenta;
  one too near a colour already taken gives way to the next of its kind.
  Selected: a yellow ring outside that.
- Nothing is drawn over a page to say its mode (the default `mode-label`
  Ring): the ring says it, and the window title names it with what is
  half-typed. Solo has no ring, so there only the title does. The corner
  chip is the `Chip` choice.
- An empty board shows a welcome straight on its colour mood, centred
  beside the sidebar a little above the middle: a dot of the space's
  tint and its name in spaced capitals (· PRIVATE for a private space),
  "Where to first?" in display type, one quiet line (what a trail is, or
  what a private space keeps), and three chips with their keys (*Open a
  page*, *Search everything*, *All keys*), quiet sheets at rest and
  lifted under the pointer; they stack when the row doesn't fit. Over a
  photo it sits on a panel sheet. It arrives like an overlay (160 ms)
  and hides while an overlay or the address bar is open.
- A sleeping card that never painted shows its site icon and title,
  centred, in place of its page.
- Label above a card (26 px high, 6 px above, shown by principle 2): stack
  count, trail tag, site icon, title; on hover or when current also the
  host, shield count and buttons (the awake mark, and a speaker while
  media plays). Otherwise its end shows a speaker for media, the awake
  mark of a card kept awake, or a moon for a sleeping one. Its place is
  the card's handle whether it shows or not.
- Awake mark (`Icon::AwakeRing`, `Icon::AwakeDisc`): one circle, as large
  as the other title-bar icons, whose colour says the mode and whose fill
  backs it up for colour-blind eyes. *Auto*: a ring, dim like the other
  buttons. *Keep awake*: a ring in `Theme::awake_keep` (the theme's cyan,
  or blue / magenta where the cyan is too near the green). *Always active*:
  a solid disc in `Theme::awake_active` (the theme's green).
- Trails leave and arrive near the top of a card with a dot at each end.
  Between, a trail runs straight across out of the source's side, straight
  up or down halfway between the cards, and straight across into the
  target, turning at right angles (`scene::route::elbow`). Each turn is
  rounded with the cards' own corner radius at the zoom shown (the
  *Corners* setting), and is square when the cards are; where the cards
  are close it shrinks to fit. The sidebar's trails and the welcome page's
  drawn board turn the same way. (Owner's decision, 2026-11-21; trails
  were S-curves before.)
- Messages (toasts, questions, key guides) sit at the foot of the board,
  on the same sheet as the palette (high lift); a question that loses
  something has a red edge.

## Lists and menus

- The palette groups its rows under small capital labels (Recent, Spaces,
  Actions; Go to, History, Web when typing). Sections follow their best
  match, so the first row is always the best one. The first row shown
  repeats its section's label.
- The trails sidebar draws each trail as the board does: the site icons
  are its stops, and a line runs down from each page and curves into the
  pages it opened. The way from the trail's first page to the current
  card is in the accent (the board's rule: the accent marks the current
  card and its trail); other trails are neutral, or their own colour once
  marked. The current card is a raised tab with its icon ringed in the
  accent.
- The shortcut sheet (F1, `?`) lists its sections as stops on a trail
  down the left, as the settings page does: a bead rides it as the list
  scrolls, passed stops fill, the section in view is lit and leans toward
  the list, and a click glides there (a clicked stop stays lit even when
  its section can't reach the top). The list is then one column, and the
  heading of the section in view docks under the header, pushed away by
  the next. Rows rise into place one after another when it opens
  (16 ms apart, 220 ms each), quicker when the filter changes; the words
  a filter matched are in `accent_text`. A narrow window drops the trail
  and flows the sections into columns, as before.
- Menus have an icon column (16 px icons, the tick of a current row
  where there is no icon) and keys right-aligned as keycaps.

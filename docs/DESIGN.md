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
palette's and address field), 18 title. Section labels in lists are
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

## Motion

Overlays ease out into place in 160 ms and leave at once. Loading is a line
gliding along a card's top edge. Nothing bounces or loops for decoration.

The board moves with cubic ease-out (`scene::motion`), and only in what is
drawn: the model is already at its end, so clicks, saving and undo never
see a card halfway.

| What | How | Time |
|---|---|---|
| Read Mode in and out | the card grows to the window, board dims | 200 ms |
| Tidy | cards glide from their old places | 220 ms |
| A card opens beside another | grows out of the source's side, 85% → 100%, fading in | 220 ms |
| A card opens elsewhere | grows from 92% in its own middle, fading in | 220 ms |
| A card closes | shrinks to 92% and fades | 160 ms |
| Space switch | the tint cross-fades; the board slides 32 px in from the side the space is on and fades in | 240 ms |
| Sidebar | slides in 150 ms; slides out the same way | 150 ms |

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
  with the keys: 2.5 px solid. Selected: a yellow ring outside that.
- A sleeping card that never painted shows its site icon and title,
  centred, in place of its page.
- Label above a card (26 px high, 6 px above, shown by principle 2): stack
  count, trail tag, site icon, title; on hover or when current also the
  host, shield count and buttons. Its place is the card's handle whether
  it shows or not.
- Trails leave and arrive near the top of a card with a dot at each end.
- Messages (toasts, questions, key guides) sit at the foot of the board,
  on the same sheet as the palette (high lift); a question that loses
  something has a red edge.

## Lists and menus

- The palette groups its rows under small capital labels (Recent, Spaces,
  Actions; Go to, History, Web when typing). Sections follow their best
  match, so the first row is always the best one. The first row shown
  repeats its section's label.
- Menus have an icon column (16 px icons, the tick of a current row
  where there is no icon) and keys right-aligned as keycaps.

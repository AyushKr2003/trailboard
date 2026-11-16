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
2. **Always know where you are.** Every card is named by a label above it
   that keeps its size on screen at any zoom, so a zoomed-out board is still
   readable (zoomable interfaces lose people when landmarks shrink away:
   Jul & Furnas, "Critical zones in desert fog", 1998). A view with no card
   in it offers *Fit everything*.
3. **Nothing needs a remembered key.** Each action has a pointer route: a
   card's label (move, back, forward, reload, close, blocker), the board's
   right-click menu, the space's menu, the palette. Menus, the palette's
   foot and tooltips show the key beside the action, which is how keys are
   learnt (Nielsen's heuristics 6 and 7: recognition over recall,
   accelerators for experts).
4. **Quiet chrome.** The accent marks the current card, its trail, the
   chosen row and matched letters; nothing else. Unmarked trails are
   neutral, so colour on the board means something was marked.
5. **One model of depth.** No glows, sheens, gradients or vignettes.

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
counts), 12 small (secondary), 13 body (rows, menus), 15 input, 18 title.

## Shape and size

- 4 px grid. Rows 30–36 px, pointer targets at least 24 px (WCAG 2.5.8);
  the top-left controls are 32 px.
- Corners follow Hyprland's rounding (0 on Omarchy's default): panels the
  full radius, small parts at most 6 px.
- Hairlines are one device pixel, drawn inside the edge (`Shape::Ring`).

## Icons

Our own set, line drawings on a 24-unit grid (`icons.rs`), rasterised at the
exact pixel size they are shown at (1.5 px strokes at 16 px), white with
coverage in alpha and tinted when drawn. Add one by adding its strokes; the
tests check it stays inside its box at every size.

## Motion

Overlays ease out into place in 160 ms and leave at once. Loading is a line
gliding along a card's top edge. Nothing bounces or loops for decoration.

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
- Card: page, hairline, two-part shadow. Current: 2 px accent ring at 70%;
  with the keys: 3 px solid. Selected: a yellow ring outside that.
- Label above a card (26 px high, 6 px above): stack count, trail tag, site
  icon, title; on hover or when current also the host, shield count and
  buttons. It is the card's handle.
- Trails leave and arrive near the top of a card with a dot at each end.
- Messages (toasts, questions, key guides) sit at the foot of the board.

# Changelog

All notable changes to Trailboard. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses
[Semantic Versioning](https://semver.org/) once it has releases.

Until the first release this is a summary by area. The full entry-by-entry
log up to 2026-11-21 is in git: `git show f554118:CHANGELOG.md`.

## [Unreleased]

### Added

- **Board and cards.** Web pages as live cards on an infinite board (CEF
  off-screen rendering, wgpu). Links open beside their source, joined by a
  trail edge; new cards are screen-shaped. Pan, zoom, fit, multi-select,
  group move, move and resize by keyboard, undo/redo. A live-browser cap
  (`TRAILBOARD_MAX_LIVE`) puts far cards to sleep.
- **Board tools.** Solo (Tab: the card fills the window), Tidy, Fold and
  stacks with an outline, trail steps (Alt+PgDn / Alt+PgUp), trail names
  and colours, notes and sticky notes, *Add to note*, Save as Markdown.
- **Spaces.** Several boards, each with its own cards, camera and
  background; private spaces (Ctrl+Shift+P); the board saved atomically
  with autosave and migrations.
- **Chrome.** Ctrl+K palette (cards, history, closed cards, commands, open
  beside with Shift+Enter), a trails sidebar and side panel, find in page,
  page zoom, page menu, desktop clipboard, IME, dropdowns and page cursors,
  site icons, downloads, a start page you can make your own, a welcome page
  on the first start, a shortcut sheet (F1), settings as a page (Ctrl+,)
  with the useful parts of Chromium's own.
- **History and search.** Every page's text in `page-text.db` (FTS5),
  thumbnails, a history view (Ctrl+H), forget a page, clear history.
- **Privacy.** Ad and tracker blocking with uBlock-style lists, cosmetic
  filtering, YouTube ads blocked, cookie banners answered with "reject",
  per-site toggles and allowed lists, a shield showing what was done,
  pages in Chromium's sandbox.
- **Keyboard first** (M12). Vim-like modes (Board, Page, Insert,
  Passthrough, Caret/Visual, Command) with a mode pill; counts; scroll
  sections (`s`, `]s`, `S`); link hints and `;` hint kinds, inside frames
  too; a `:` command line with completion, history, readline keys, `.`,
  `:repeat` and `:later`; cards as tabs (J/K, Alt+digits, Ctrl+^, gt, d/u,
  co, gC, `Ctrl+w` sequences); search keywords (`gh foo`); jumps
  (Ctrl+O / Ctrl+I), marks and saved pages (`b` / `B`); a user keys file
  (`~/.config/trailboard/keys`) with bindings, aliases, chains and `:run`;
  `:set` / `:toggle` / `:site` and `t` toggles; `]]` / `[[`, gu/gU,
  Ctrl+a/x and other address moves; caret and visual selection; page
  dialogs, permissions and a typed file chooser from the keyboard; Ctrl+e
  edits a field in `$EDITOR`; `?` finds backward; `:messages`.
- **Look.** Omarchy theme followed live; cards as sheets without title
  bars, quiet labels, corners (Rounded, Follow Hyprland, Custom), board
  backgrounds, motion, an empty-board welcome, a new app icon.
- **Packaging.** `scripts/bundle.sh`, an Arch PKGBUILD, a desktop entry,
  `--version` / `--help`, crash reports, opening pages in the running
  window, `scripts/install-cef.sh`.
- **Testing.** Scripted end-to-end runs via `TRAILBOARD_SCRIPT`.

### Changed

- **H.264 and AAC video.** `scripts/install-cef.sh --codecs` installs a CEF
  build with the patent-licensed codecs (same CEF version, third-party,
  pinned by SHA-256), and the Arch package uses it by default
  (`TRAILBOARD_CODECS=0` for the standard one). `scripts/bundle.sh` refuses
  to package it unless `ALLOW_CODECS=1`.
- `scripts/install-cef.sh` pins both engines by name and SHA-256 (no build
  index lookup), checks tools, disk space and a running Trailboard first,
  works on disk instead of `/tmp`, swaps the new copy in only once it's
  complete, skips a build that's already installed (`--force`), and writes
  `.cargo/config.toml` when it's missing; `--help` lists the options.
- Ctrl+W alone no longer closes a card; it starts `Ctrl+w` sequences.
- Read Mode is now called Solo; `t` starts toggles and Tidy is `T`; the
  trails sidebar is Ctrl+B and `b` / `B` open saved pages.
- Shortcuts follow the logical key, so they work on any layout.
- Version 0.2.0 (`Cargo.toml`, PKGBUILD).
- Docs: `docs/PLAN.md` and this file shortened; `docs/BUGS.md` and
  `docs/FUTURE.md` folded into the plan (§7 and §8).

### Known issues

- Copying uses the page's plain selected text, so pages that change what is
  copied, and "copy" buttons using `navigator.clipboard`, don't reach the
  desktop clipboard. Desktop text is pasted as plain text.
- Text copied from Trailboard leaves the clipboard when Trailboard quits.
- Pages see `devicePixelRatio` equal to the board's raster scale (0.5–2).
- Custom image cursors (`cursor: url(…)`) show the default arrow.
- A card that wakes from sleep reloads its page, losing scroll position and
  form input.
- H.264, AAC and HEVC video and DRM don't play with the standard CEF (M13).
- Typing is tested through scripted key events, not real keyboard input.

<div align="center">

<img src="share/trailboard.svg" width="88" alt="Trailboard icon">

# Trailboard

### Browse on a board, not in tabs.

Pages are live cards on an infinite canvas. Click a link and it opens **beside** the page it came from,<br>
so every rabbit hole leaves a trail you can see, follow back, and pick up tomorrow.

![Linux](https://img.shields.io/badge/Linux-Wayland%20%26%20X11-1a1b26?style=flat-square&logo=linux&logoColor=white)
![Rust](https://img.shields.io/badge/Rust-wgpu%20%2B%20Chromium-1a1b26?style=flat-square&logo=rust&logoColor=white)
![License](https://img.shields.io/badge/license-Apache--2.0-1a1b26?style=flat-square)

[Install](#install) · [How it works](#how-it-works) · [Keys](#keys-you-need) · [Guide](docs/GUIDE.md)

<br>

<img src="docs/images/board.png" alt="A Trailboard board: Turbofan opens Jet engine, which branches to Brayton cycle and Frank Whittle, all joined by trail lines">

<sub>One trail, four pages: Turbofan → Jet engine → Brayton cycle, with a branch to Frank Whittle.</sub>

</div>

<br>

## How it works

<table>
<tr>
<td width="33%" valign="top">

**1 · Open a page**

It lands on the board as a card: a real, live web page you can scroll, type in and click.

</td>
<td width="33%" valign="top">

**2 · Follow a link**

The new page opens next to its source, joined by a line. Your path stays on screen.

</td>
<td width="33%" valign="top">

**3 · Find your way back**

Zoom out to see the whole trail, or jump to any card from the palette or sidebar.

</td>
</tr>
</table>

## What's on the board

| | |
|---|---|
| **Trails** | Every link remembers where it came from. Name a trail, colour it, fold it into a stack. |
| **Live cards** | Each card is a full Chromium page. Pan, zoom, drag, resize. Off-screen cards sleep to save memory. |
| **Keyboard first** | `h j k l` between cards, `f` to follow links, `Ctrl+K` for everything else. |
| **Tidy & Fold** | One key lays a messy board out without overlaps; another folds a trail into a stack. |
| **Notes** | Markdown and sticky notes live beside your pages. Quote a selection into a note, save the board as Markdown. |
| **Spaces** | A separate board per topic, each with its own tint. Private spaces keep no history. |
| **Private by default** | Ads, trackers and cookie banners blocked with uBlock Origin's lists. Everything stays on your disk. |
| **Fits your desktop** | Follows your Omarchy theme and Hyprland's rounding live. |

<table>
<tr>
<td width="50%" valign="top">

**Find anything** · `Ctrl+K`

<img src="docs/images/palette.png" alt="The Ctrl+K palette searching for 'whittle'">

Cards, closed cards, history (by any word on the page), commands, other spaces.

</td>
<td width="50%" valign="top">

**See every trail** · `Ctrl+B`

<img src="docs/images/sidebar.png" alt="The trails sidebar listing a trail as a tree">

Each trail as a tree. Walk it with `j`/`k`, pin it, rename it.

</td>
</tr>
</table>

## Install

**Arch / Omarchy**: builds from source, fetches its own Chromium engine, adds a launcher entry.

```sh
git clone https://github.com/AyushKr2003/trailboard && cd trailboard/packaging/arch
makepkg -si
```

**Any other distro**: build a portable archive (~150 MB), unpack it anywhere, run `./trailboard`.

```sh
./scripts/install-cef.sh && ./scripts/bundle.sh
```

<details>
<summary><b>Run from source</b> (for development)</summary>

Needs Rust, CMake, Ninja and a Vulkan driver.

```sh
./scripts/install-cef.sh                 # Chromium engine (CEF) into ~/.local/share/cef
mkdir -p .cargo && cat > .cargo/config.toml <<EOF
[env]
CEF_PATH = "$HOME/.local/share/cef"
[target.x86_64-unknown-linux-gnu]
rustflags = ["-C", "link-arg=-Wl,-rpath,$HOME/.local/share/cef"]
EOF
cargo run --release -p trailboard -- https://en.wikipedia.org/wiki/Turbofan
```

`cargo test` and `cargo clippy` must pass before a change goes in.

</details>

<details>
<summary><b>Sandbox says "off"?</b></summary>

On kernels without unprivileged user namespaces (e.g. linux-hardened), Chromium needs its
setuid `chrome-sandbox` helper. The Arch package installs it; for the archive, see the
[guide](docs/GUIDE.md#running-it).

</details>

## Keys you need

| Key | Does |
|---|---|
| `Ctrl+K` | Find a card, a page, a command |
| `Ctrl+L` · `Ctrl+T` | Address bar · new card |
| `Shift+Enter` | Open what you typed **beside** the current card |
| `h` `j` `k` `l` | Move between cards |
| `Enter` · `Esc` | Type in the page · back to the board |
| `f` | Follow a link by keyboard |
| `Tab` | Solo: the page fills the window |
| `Ctrl+B` · `T` · `S` | Trails sidebar · Tidy · Fold a trail |
| `b` · `:save-page` | Open a saved page · save this one |
| `td` · `tj` · `:set` | Dark pages · JavaScript for this site · any setting by name |
| `]]` · `gu` · `Ctrl+A` | Next page · up the address · the address's last number up |
| `N` | New note |
| `F1` · `:keys` | Every shortcut |

The rest (settings, downloads, history, blocking, where your data lives) is in the **[guide](docs/GUIDE.md)**.

## Your data

Boards, history and the browser profile live in `~/.local/share/trailboard/`.
No account, no sync, no telemetry.

## More

[Guide](docs/GUIDE.md) · [Roadmap](docs/PLAN.md) · [Ideas](docs/FUTURE.md) · [Design rules](docs/DESIGN.md) · [Changelog](CHANGELOG.md)

<br>

<div align="center">
<sub>Apache-2.0 · Inspired by <a href="https://marble.surf">Marble</a>.</sub>
</div>

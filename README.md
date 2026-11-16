<div align="center">

<img src="share/trailboard.svg" width="96" alt="">

# Trailboard

**A browser where pages are cards on an infinite board.**<br>
Click a link and it opens *beside* the page it came from, so your research leaves a trail.

![Linux](https://img.shields.io/badge/Linux-only-1a1a1a?style=flat-square&logo=linux&logoColor=white)
![Rust](https://img.shields.io/badge/Rust-wgpu%20%2B%20CEF-1a1a1a?style=flat-square&logo=rust)
![License](https://img.shields.io/badge/license-Apache--2.0-1a1a1a?style=flat-square)

</div>

```
  ┌──────────────┐       ┌──────────────┐       ┌──────────────┐
  │  Turbofan    │──────▶│  Jet engine  │──────▶│  Brayton     │
  │  ▒▒▒▒▒▒▒▒▒▒  │       │  ▒▒▒▒▒▒▒▒▒▒  │       │  cycle       │
  └──────────────┘       └──────┬───────┘       └──────────────┘
                                │
                         ┌──────▼───────┐    ·   ·   ·   ·   ·
                         │  Whittle     │      no tabs, no
                         │  ▒▒▒▒▒▒▒▒▒▒  │      lost context
                         └──────────────┘    ·   ·   ·   ·   ·
```

## Why

- **Trails, not tabs**: every link you follow stays connected to where you found it.
- **Live cards**: each card is a real Chromium page. Pan, zoom, drag, resize.
- **Keyboard first**: `h j k l` to move, `f` to follow links, `Ctrl+K` for everything.
- **Tidy and fold**: lay out a messy board, or fold a trail into a stack.
- **Notes on the board**: Markdown and sticky notes, export the board as Markdown.
- **Spaces**: separate boards per topic, plus private spaces.
- **Built-in blocking**: ads, trackers and cookie banners, with uBlock Origin's default lists.
- **Fits your desktop**: follows the Omarchy theme and Hyprland rounding live.

## Install

**Arch / Omarchy**

```sh
cd packaging/arch && makepkg -si
```

**From source** (Rust, CMake, Ninja, Vulkan)

```sh
./scripts/install-cef.sh                 # CEF into ~/.local/share/cef
mkdir -p .cargo && cat > .cargo/config.toml <<EOF
[env]
CEF_PATH = "$HOME/.local/share/cef"
[target.x86_64-unknown-linux-gnu]
rustflags = ["-C", "link-arg=-Wl,-rpath,$HOME/.local/share/cef"]
EOF
cargo run --release -p trailboard -- https://en.wikipedia.org/wiki/Turbofan
```

**Other distros:** `./scripts/bundle.sh` makes a portable
`target/bundle/trailboard-VERSION-x86_64.tar.zst` (~150 MB). Unpack it anywhere and run `./trailboard`.

> **Sandbox off?** On kernels without user namespaces (linux-hardened), use the
> Arch package (it installs the setuid `chrome-sandbox` helper), or see the
> [guide](docs/GUIDE.md#running-it) for the bundle.

## Keys you need

| | |
|---|---|
| `Ctrl+K` | Find a card, a page, a command |
| `Ctrl+L` / `Ctrl+T` | Address bar / new card |
| `h j k l` | Move between cards |
| `Enter` / `Esc` | Type in the page / back to the board |
| `f` | Follow a link by keyboard |
| `Tab` | Read Mode (page fills the window) |
| `B` | Trails sidebar |
| `T` / `S` | Tidy the board / fold a trail |
| `N` | New note |
| `?` | All shortcuts |

Everything else is in the **[guide](docs/GUIDE.md)**.

## Docs

[Guide](docs/GUIDE.md) · [Plan](docs/PLAN.md) · [Ideas](docs/FUTURE.md) · [Design](docs/DESIGN.md) · [Changelog](CHANGELOG.md)

<sub>Inspired by Marble.</sub>

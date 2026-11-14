# Marble 0.1.28 (macOS): reverse-engineering findings

Target: `Marble-0.1.28.dmg`, SHA-256 `ecda95ecf72525e3a0a804b9d8336bebafc4fd3de3f3e35dda1559392ee35b89`.
Main binary `Marble.app/Contents/MacOS/Marble`, Mach-O arm64, 70,459,840 bytes,
SHA-256 `09152db4677cd36be366594b026630ad289b987da3f3f3a28d169b8768e6690b`.

Purpose: understand behaviour and architecture so we can build an independent
Linux browser with similar features. Marble's code, art, and server are not reused.

Labels: **[obs]** observed in the bytes · **[der]** derived from observed data ·
**[inf]** inference, not proven · **[unk]** unknown.

REA Evidence (local, in the session scratchpad):
- `inspect_artifact` on the DMG: `ev_805fb7a6…` (root hash only; DMG child inventory needs macOS)
- `inspect_artifact` on `Marble.app`: inventory `ev_22929005…` (267 files, 269 dirs, 1 symlink, complete)
- `inspect_rust_binary`: `ev_e3f4478c…` (no cargo-auditable `.dep-v0` metadata)

## 1. Architecture

| Layer | What Marble uses | Linux availability |
|---|---|---|
| Language | Rust (stable, aarch64-apple-darwin) [obs] | yes |
| Windowing | `winit` 0.30.13 [obs] | yes (X11/Wayland) |
| GPU canvas | `wgpu` 29.0.3 (vendored `wgpu-hal`, Metal backend) [obs] | yes (Vulkan/GL) |
| Text | `cosmic-text` 0.18, `glyphon` 0.11, `swash`, `fontdb` [obs] | yes |
| Web engine | CEF 148.0.10 (Chromium 148) via `cef` crate 148.2.0, **off-screen rendering** (`cef_backend/src/osr/*`) [obs] | yes (CEF ships Linux builds) |
| Storage | `rusqlite` 0.37 with FTS5; JSON workspace files via serde [obs] | yes |
| Semantic search | `ort` 2.0 (ONNX Runtime) + `tokenizers` 0.21; `Resources/meaning/model_quantized.onnx` (118 MB) [obs]; XLM-R Unigram tokenizer, 250,002 vocab [obs] → likely multilingual-e5-small [inf] | yes |
| Ad blocking | Brave `adblock` 0.13.3 + EasyList-style lists + cookie-banner rules (`consent.rs`) [obs] | yes |
| Updates | Velopack 1.2 (`sq.version`, `UpdateMac`) [obs] | yes |
| Telemetry | Sentry 0.48, PostHog (`MARBLE_POSTHOG_KEY`) [obs] | drop |
| Payments | Paddle ("Paid through Paddle") [obs] | n/a |
| Explain 3D | three.js 0.186.1 embedded [obs] | yes |
| Notes editor | ProseMirror (`document/pm.rs` + embedded PM JS) [inf from code shape] | yes |

Rendering model [der]: each page is a CEF off-screen browser. Its frames are
uploaded as GPU textures (`osr/render.rs`, `compositor/mipmap.rs`) and drawn as
cards on an infinite canvas by Marble's own `wgpu` compositor
(`compositor/pipelines.rs`, 12 embedded WGSL shaders). The chrome (sidebar,
palette, omnibox) is drawn natively with glyphon, not in HTML. Explain,
onboarding, and notes are HTML/JS pages hosted inside CEF.

### First-party crates and modules [obs: source paths embedded in the binary]

- `app_shell`: app logic. `document/` (notes: layout, ProseMirror bridge, edit surface),
  `explain/` (AI explain: client, guest, ink, layers ask/bar/feed/tell, script, stand),
  `trails_panel/` (sidebar: dock, fold_rows, gesture, outline, rows),
  `palette/` (⌘K: draw, layout, matching, rows, strip), `omnibox/` (clipboard, suggest, url_edit),
  `import/` (wizard), `onboarding/`, `tour/`, `settings/` (frame, shortcuts), `spaces.rs`,
  `stacks/`, `edges.rs`, `liveness/` (favicon, warmup), `page_zoom.rs`, `search.rs`,
  `persistence.rs`, `wallpaper.rs`, `updater.rs`, `update_gate.rs`, `telemetry.rs`,
  `account.rs`, `passkeys.rs`, `password_managers.rs`, `secrets.rs`, `quarantine.rs`, `platform_mac.rs`
- `cef_backend`: `osr/` (app, contexts, handlers, render, runtime, page_kit, nested_pump,
  extension_watch, extension_windows, native_hosts), `adblock.rs`, `consent.rs`,
  `cookie_sites.rs`, `chrome_cookies.rs`, `download.rs`, `live_page.rs`, `mac_app.rs`
- `compositor`: `frame.rs`, `pipelines.rs`, `mipmap.rs`, `wallpaper.rs`
- `scene`: `command.rs`, `index.rs`, `trail_query.rs` (scene graph and spatial index)
- `storage`: `workspace.rs`, `history.rs`, `page_text.rs`, `page_meaning.rs`, `index.rs`,
  `profiles.rs`, `paths.rs`, `seed.rs`, `install.rs`, `from_pages.rs`
- `chrome_import`: Arc, Chrome (profiles, sessions, cookies via `pickle.rs`), Firefox, Safari, favicons
- `meaning`: embedding inference
- `tools/src/tidy.rs`: the Tidy layout algorithm
- `xtask/src/bundle.rs`: build and packaging

## 2. Data formats

### Page-text DB (`page-text.db`, SQLite) [obs: exact SQL]

```sql
CREATE TABLE IF NOT EXISTS pages (
    id INTEGER PRIMARY KEY,
    url TEXT NOT NULL UNIQUE,
    seen_ms INTEGER NOT NULL
    -- later migration: meant INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS pages_seen ON pages(seen_ms);
CREATE VIRTUAL TABLE IF NOT EXISTS pages_text USING fts5(
    title, body, tokenize = 'unicode61 remove_diacritics 2', prefix = '2 3');
CREATE TABLE IF NOT EXISTS thumbs (page_id INTEGER PRIMARY KEY, jpeg BLOB NOT NULL);
CREATE TABLE IF NOT EXISTS passages (
    page_id INTEGER NOT NULL, n INTEGER NOT NULL, vec BLOB NOT NULL,
    PRIMARY KEY (page_id, n));
```
- Search: `snippet(pages_text, 1, '', '', '…', 12)` ranked by `bm25(pages_text, 4.0, 1.0)` (title weighted 4×) [obs].
- Retention: keep the newest N pages by `seen_ms`, deleting older passages, thumbs, and pages [obs].
- `passages.vec` holds per-passage embeddings from the meaning model [der]; `meant` marks pages already embedded [inf].

### Workspace / board (JSON via serde, written via `*.json.tmp` then rename) [obs]

Struct names and arities are observed. The field-to-struct assignment is inferred from serde identifier order.
- `WorkspaceSnapshot` (11): `workspace`, `camera`, `nodes`, `focused`, `edges`, `bundles`, `labels`, `favorites`, `stacks`, `areas`, (+1 [unk])
- `NodeSnapshot` (20): `id`, `kind`, `rect`, `z`, `url`, `title`, `tint`, `created_at`, `updated_at`, `deleted`, `query`, `body`, `body_format`, `body_scale`, `collapsed`, `trail_name`, `trail_icon`, `trail_color`, `page_zoom`, `outline_rank` [inf]
- `EdgeSnapshot` (7): `from`, `to`, `kind`, `created_at`, `from_side`, `to_side`, (+id) [inf]
- `StackSnapshot` (5): `members`, `front`, `outline_open`, … [inf]
- `AreaSnapshot` (5), `LabelSnapshot` (4), `BundleSnapshot` (6), `FavoriteSnapshot` (5: incl. `label`, `host`), `CameraSnapshot` (2: `center`, `zoom`), `WorkspaceMeta` (5)
- Spaces: `SpaceIndex` (3: `schema_version`, `active`, `spaces`); `SpaceEntry` (3: `id`, `name`, `background`)
- Profiles: `Profiles` (4), `ProfileEntry` (4: incl. `color`, `last_space`)
- Others: `Preferences` (19 fields), `HistoryDoc` (2) / `HistoryEntry` (7), `InstallDoc` (5)
- Tint palette [obs]: charcoal, teal, aqua, moss, deep, indigo, sky, slate, dawn, pink, plum, ember, orange, umber, lime, paper
- Data dir override: `MARBLE_DATA_DIR`; profile: `MARBLE_PROFILE_DIR`, `MARBLE_ACTIVE_PROFILE` [obs]

## 3. Features and commands [obs: menu and command strings]

Canvas: Open / See the Board (Tab), Fit to View (F), Tidy Up (T), Fold trail (S),
Next/Previous on Trail, Read Mode (fills the window), Messy Mode, Show Trails,
Open links beside the page (a link becomes a new card next to its source, with an edge),
Show Card Names on Hover, Zoom In/Out/Actual Size, Fit selection.
Pages: Reload / Force Reload, Stop, Find in Page / Next / Previous, View Source,
Copy Address (also as Markdown), Reopen Closed Page, Pin/Unpin, page zoom,
Disable/Enable protections per site.
Notes: New Note (N), Add to note (quote a selection with a source link), Save as Markdown,
sticky notes, documents, PDF open.
Spaces: New / Next / Previous / Rename / Delete Space, Space background (wallpaper).
Profiles: New / Switch / Edit Profile.
Other: ⌘K palette (fuzzy matching over pages, history, commands), Keyboard Shortcuts
sheet, Settings, Import (Chrome, Arc, Firefox, Safari: history, sessions, cookies,
favicons), password-manager native hosts (1Password), passkeys, onboarding tour.
Privacy: ad and tracker blocking on by default; cookie banners auto-answered "only necessary".

## 4. Explain (AI): server-backed [obs]

- Endpoint: `https://api.marble.surf/explain` (override `MARBLE_EXPLAIN_URL`;
  `MARBLE_EXPLAIN_STANDIN` / `MARBLE_EXPLAIN_REPLAY` for testing).
- Request fields: `title`, `excerpt`, `selection`, `question`, `text`, `marked`,
  `images`/`picture`, `circled`, `drawn`, `cards`, `previous_response_id`.
  Headers: `Authorization: Bearer …` or `X-Marble-Guest` (guest quota with `usage`/`remaining`).
- Size limits shared with the server's `prompt.ts` (UTF-16 units): excerpt 60000,
  drawn 60000, text 2000, title 400, selection 4000, marked 4, picture 800000,
  pictures 2000000, near 800, cards 12.
- Depth modes: `sketch`, `explore`, `build` [obs]. Answers can render ink on the page
  (rings, underlines, arrows, spotlight, step numbers) and three.js scenes [obs].
- Error states: too_big, resting, bad_request, cut_off, refused, unreachable [obs].
- A Linux build needs its own LLM backend; the server prompt is not in the binary [unk].

## 5. Not portable / skip on Linux

`platform_mac.rs`, `quarantine.rs` (xattr / App Translocation), macOS Keychain
(`MARBLE_KEYCHAIN_SERVICE`), `mac_app.rs`, the Metal HAL, Touch ID passkeys, Safari import,
Paddle billing, PostHog/Sentry telemetry.

## 6. Open questions (needs Ghidra/REA decompilation)

- [ ] Tidy algorithm (`tools/src/tidy.rs`)
- [ ] "Open beside" placement rule and edge `from_side`/`to_side`
- [ ] Passage chunking size and embedding pooling for `passages`
- [ ] Fold / stack behaviour
- [ ] Exact JSON field-to-struct mapping

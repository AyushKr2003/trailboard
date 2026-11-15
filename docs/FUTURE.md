# Future plans

Ideas beyond the core milestones in [PLAN.md](PLAN.md), roughly in priority order.
Each needs a short design note before work starts.

> **On hold:** every AI item here (semantic search, Explain, and any other
> AI/ML feature), import from other browsers, and password managers and
> passkeys wait for the owner's decision. Do not start on them until the
> owner says so (see PLAN.md, M10).

## Near term (after M9)

### Semantic search ("search by meaning") ⏸ on hold
- Local embedding model through `ort` (ONNX Runtime) + `tokenizers`; Marble
  ships a ~118 MB int8 multilingual model (likely multilingual-e5-small; unconfirmed).
- Pick our own openly licensed model; download on first use, not bundled.
- Store per-passage vectors in `passages(page_id, n, vec)` next to FTS5, and blend
  BM25 and vector scores in the palette.

### Explain (bring your own LLM key) ⏸ on hold
- Side panel that explains the current page or selection, answers follow-ups,
  and can mark up the page (highlights, arrows, step numbers) through injected JS.
- Uses the user's own API key (Claude API by default), stored in the system keyring
  (Secret Service). No Trailboard server.
- Inputs bounded like a good client should be: title, excerpt, selection, optional
  screenshot. Honour size limits and show cost and usage.

### Import from other browsers ⏸ on hold
- Chrome/Chromium/Brave/Arc, Firefox: history, open tabs/sessions, bookmarks, favicons.
- Cookies only with explicit consent.

### Password managers and passkeys ⏸ on hold
- Native messaging hosts for 1Password/Bitwarden extensions; WebAuthn via CEF.

## Medium term

- **Extensions:** enable CEF's Chrome extension support for a curated set (password managers, uBlock-style lists).
- **PDF cards** with Chromium's viewer and quote-to-note.
- **Board sharing:** export a space as a self-contained HTML/Markdown snapshot with thumbnails.
- **Multiple windows** on different spaces; drag cards between windows.
- **Accelerated OSR:** dmabuf texture sharing from CEF to wgpu (no CPU copies).
- **Touchpad gestures:** pinch zoom, three-finger pan (winit gesture events).
- **Themes:** light mode, accent colours, custom wallpapers per space.
- **Accessibility:** keyboard-only board navigation, screen-reader names for cards.

## Long term

- **Sync between your own devices** (end-to-end encrypted, self-hostable), if wanted.
- **Collaborative boards** (CRDT-based scene model).
- **macOS/Windows builds** (same stack: winit, wgpu, CEF).
- **Plugin API** for custom card types (notes, code, charts).

## Explicitly out of scope

- Copying Marble's branding, art, server API, prompts or paid features.
- Telemetry or analytics without opt-in.

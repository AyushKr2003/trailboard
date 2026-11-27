# Performance bugs

What the CI bench says Trailboard does worse than other browsers, with the
evidence, what is already known, and what to try next. Ranked: the
first is the biggest gap. Move an entry into its milestone in
[PLAN.md](PLAN.md) when it is taken up, and delete it here when it is fixed.

**Research:** 2026-11-27, against the code at `3053e6f`; each PB's
"Next to try" is in the order to take it up. Suggested order across
them: PB3, PB1 (1–2), PB2 (1–3), PB4 (1–3), then dmabuf.

**Source:** bench run [46010069445](https://github.com/AyushKr2003/trailboard/actions/runs/46010069445)
(2026-11-27, Trailboard `5aa154c`, all six cases, 3 runs each, medians).
Raw rows: `results/20261127-1348-5aa154c-46010069445.jsonl` on the `bench`
branch. How to run and read a bench: PLAN.md M16, "How to measure".

## The numbers

CPU is % of one core (4 cores = 400 %), memory is PSS in MB. The CI runner
has **no GPU**: Trailboard draws with wgpu on lavapipe/llvmpipe (software
Vulkan), so its drawing runs on the CPU, inside the `trailboard`
process.

| Case | Trailboard | Chrome | Brave | Firefox | GNOME Web | Falkon |
|---|---|---|---|---|---|---|
| idle, CPU | 2.4 | 0.6 | 1.1 | 3.7 | 0.0 | 9.3 |
| still, CPU | 0.8 | 0.3 | 0.2 | 3.4 | 0.1 | 0.3 |
| video-vp9, CPU | **352** | 49 | 46 | 55 | — | 254 |
| video-av1, CPU | **259** | 34 | 33 | 50 | — | 179 |
| video-h264, CPU | **257** | 32 | 31 | 47 | — | 181 |
| pan, CPU | 348 | — | — | — | — | — |
| idle, memory | 591 | 438 | 471 | 744 | 440 | 412 |
| still, memory | 690 | 529 | 559 | 820 | 993 | 430 |
| video-vp9, memory | 631 | 496 | 515 | 820 | — | 480 |
| startup | 0.22 s | 0.33 s | 0.32 s | 1.04 s | 0.42 s | 0.21 s |

GNOME Web's video crashes on the runner, so it is measured in idle and still
only. Pan is Trailboard-only (nobody else has a board).

Fine already: startup (as fast as anyone), still-page CPU (under 1 %),
memory below Firefox and GNOME Web.

---

## PB1. Video: the main process burns 5–8× Chrome's CPU 🔴

**Seen.** Video with the card filling the 1920×1080 window:

| Case (runner CPU) | Trailboard | main / renderer / gpu | Chrome | ÷ Chrome | perf |
|---|---|---|---|---|---|
| video-vp9 (EPYC 7763) | 352 % | 323 / 25 / 4 % | 48.5 % | 7.3× | 30 fps, 30.3 ms/frame, 91 % |
| video-av1 (Xeon 6975P-C) | 259 % | 237 / 18 / 4 % | 34.1 % | 7.6× | 30 fps, 21.1 ms/frame, 63 % |
| video-h264 (Xeon 6975P-C) | 257 % | 235 / 19 / 4 % | 31.6 % | 8.1× | 30 fps, 20.8 ms/frame, 62 % |

**Not the cause.**
- *Decoding:* our renderer process (where the video is decoded) uses
  18–25 %, the same as Chrome's renderer (15–25 %).
- *Copies and uploads:* `copied` / `uploaded` are **MB/s**; 248 MB/s is 30
  frames × 1920×1080×4 bytes (8.3 MB), so each frame is copied once and
  uploaded once, as it should be. At memory speed that is a few % of a
  core, not 230 %. (M16 6d made the copies dirty-rect only; no CPU change.)
- *Chromium's compositing:* `--disable-gpu-compositing` changed nothing
  (M16 6b, run 45983554997).
- *Frame rate:* CEF already paints at the video's 30/s (M16 6c skipped).

**Cause (known, M16 6f).** Our own drawing, per pixel. Every video frame is
a full frame (`partial 0`): the card is more than `PARTIAL_MAX` (0.9,
`crates/compositor/src/renderer.rs:113`) of the window, so the whole window
is redrawn, and the page is drawn as a filtered, rounded, shadowed quad
through the general shader (`Shape::Page`, `renderer.rs:566`). On llvmpipe
that is ~21–30 ms a frame, 2 M pixels, spread over all cores.

**On a real GPU** (the owner's iGPU) the drawing is cheap; what's left is
the two 8 MB copies per frame (out of CEF's buffer, into a texture).

**Also found (2026-11-27 research).** During video every frame is drawn
into the canvas and then copied to the screen: `use_canvas` stays on while
the layout is unchanged (`shown_layout == Some(layout)`, `renderer.rs`
`render`). That is a third 8 MB copy a frame. A partial frame above
`PARTIAL_MAX` would save almost nothing here: under the page, `cover()`
already starts drawing at the page, so the page's rect *is* the work.

**Next to try.**
1. **Copy a page shown at 1:1 instead of shading it.** When a page quad's
   colour is white, its rect equals its texture size × `extent`, and its
   position is on whole pixels (snap within 0.01 px), copy the texture
   into `cover()`'s hole with `copy_texture_to_texture` (same
   `Bgra8Unorm` format), before the render pass. The pass then loads
   (`LoadOp::Load`, no clear: every other piece gets the opaque
   background anyway), draws the pieces round the hole as now (corners,
   border, shadow stay exact in the general shader), and in the hole
   draws only what comes after the page. The canvas needs `COPY_DST`.
   On llvmpipe a copy is a memcpy, ~1 ms for 8 MB, against 21–30 ms of
   shading. This is the lever for the CI numbers.
2. **No canvas for full frames in motion.** A full frame drawn while
   frames come quickly goes straight to the screen; the canvas is
   refreshed at the pause, as for panning. Saves one 8 MB copy a frame.
3. **Upload from CEF's buffer inside `on_paint`** (owner's call: it
   breaks "CEF → app only through `drain_events`"). Drops the copy into
   `pixels` and 8 MB a card (helps PB4); thumbnails would read back.
4. **dmabuf paint (M16 6f), closer than thought.** Our `cef` crate
   (156.4.0) has an `accelerated_osr` feature with Linux DMA-BUF import
   into wgpu 30 (`cef::osr_texture_import`): it imports the planes as a
   Vulkan image, copies into an app-owned texture and waits before CEF
   reuses the buffer (our `cef_render_handler.h` says it must be copied
   inside the callback). wgpu 30 has
   `Features::VULKAN_EXTERNAL_MEMORY_DMA_BUF`. To check: it seems to make
   a new texture each frame (we want a copy into the page's texture);
   fall back to `on_paint` with no GPU or on import failure; Chromium
   notes linear buffers fail on some NVIDIA drivers. CI has no GPU, so
   only a laptop run shows the gain.

**Goal.** M16's: video ≤ 2× Chrome in CI. Measure each change with the
short run in PLAN M16 ("How to measure"), `cases=video-vp9,video-h264
only=trailboard,chrome`, and compare Trailboard ÷ Chrome in the same job.

## PB2. Panning the board costs 3.5 cores 🔴

**Seen.** `pan`: 348 % CPU, 345 % of it in the main process; 46 fps
presented, 19.5 ms a frame, 90 % of the time in the renderer, 0 partial
frames (Xeon 6975P-C). Page paints are 1/s, so this is all our drawing.

**Cause (likely, same as PB1).** Every pan frame redraws the whole window:
background, every card's page quad, shadows, labels, trail lines, all
through the general shader on llvmpipe. A pan moves everything, so partial
frames can't help.

**Also found (2026-11-27 research).** Overdraw: each card's pixels are
shaded four or five times: the background, two shadow quads (each covers
the whole card plus its blur), the card's frame fill, then the page.
`cover()` cuts the background out under the single largest opaque page
only.

**Next to try.**
1. **Measure per group with timestamp queries** (wgpu
   `TIMESTAMP_QUERY`, which lavapipe has) in `TRAILBOARD_PERF`:
   background, shadows, pages, text. Better than `perf record`, which
   can't name llvmpipe's JIT code.
2. **Cut overdraw.** Shadows as a ring round the card (no interior: it is
   under the opaque page); the frame fill only where the page doesn't
   cover (the label strip); the background cut out under every opaque
   page, not only the largest.
3. **An opaque-page pipeline** for scaled pages: no blending, no SDF, no
   branches, one sample; the rounded edges stay with the general shader
   as in PB1 (1). Mesa's llvmpipe has a linear 2D fast path (2–3× for
   blits) that only engages for simple shaders without depth; whether
   lavapipe reaches it is unknown, so measure it rather than count on it.
4. **Last, owner's call:** cap motion at 30 fps when the adapter is a CPU
   one (`DeviceType::Cpu`); it reaches only 46 fps now.
5. **Not now:** scrolling the last frame and drawing only the strips a
   pan exposes. It needs the board drawn apart from the overlays, and the
   window-fixed gradient and grain changed.

**Goal.** No reference browser has a board, so compare with ourselves:
halve it (≤ 175 %), keeping the motion smooth in the screenshots.

## PB3. Idle: 4× Chrome's CPU, with nothing happening 🟡 (fix written, to measure)

**Seen.** `idle` (start card only, EPYC 9V74): 2.4 % CPU, 2.0 % of it in the
main process, against Chrome's 0.6 % (0.4 % in its browser process).
Trailboard's counters: **1.7 page paints/s**, 1.7 frames presented/s (all
partial, 3 ms each), and **5 renders/s skipped as unchanged**. In `still`
(no paints at all) there are still **4 renders/s skipped**, and 0.7 % CPU.

**Cause (to find).** Two things wake us when nothing changes:
1. Something on the start card repaints ~1.7×/s. A blinking caret in a
   focused text field is the first suspect (blinks are ~2/s); an animation
   or a timer on the page is the other.
2. The main loop asks for a frame ~4–5×/s that turns out unchanged
   (`renderer.rs:500`, `skipped`). Something requests redraws on a timer
   (HUD, hover, a clock, a pending save, `pump` scheduling) instead of only
   on change.

**Found (2026-11-27).**
1. The 4 skipped renders a second were `IDLE_TICK`: the loop asked for a
   frame at every 250 ms wake (`now >= deadline`, `next_deadline` never
   later than `IDLE_TICK`), built every layer and threw it away.
2. Every presented frame asked for one more (`animating()` was "the last
   frame was new"), so each caret paint cost a second, skipped, build.
3. The start page's field has `autofocus`: Chromium blinks its caret
   ~2×/s. (The clock's digit roll adds ~37 frames once a minute.)

**Done (uncommitted, to measure on CI).** The loop still wakes every
`IDLE_TICK` for the timers `tick` polls, but draws by itself only every
`IDLE_DRAW` (1 s; the draw polls the wallpaper and a decoding board
photo). `web::wake()` (background threads handing something over) now
counts as activity, so it draws at once; CEF's pump scheduling only
wakes. A toast asks for a draw (`draw_wanted`). The loop keeps drawing
only after a frame laid out anew (`Renderer::relaid`), not after one
where only page pixels changed. The start page rests its caret after
10 s without a key, click or focus (`caret-animation: manual`, Chromium
139+): it stays shown, steady, as on the desktop.

**Next to try.** A CI run (`cases=idle,still only=trailboard,chrome`):
skipped frames should be ~1/s and paints ~0 after 10 s. If idle is
still above 1 %, log each CEF paint's dirty rect to see what else moves.

**Goal.** Idle ≤ 1 % and still ≤ 0.5 %, near Chrome.

## PB4. About 150 MB more memory than Chrome 🟡

**Seen.**

| Case | Trailboard | Chrome | Trailboard main | Chrome browser + gpu |
|---|---|---|---|---|
| idle | 591 | 438 | 309 | 144 + 78 = 222 |
| still | 690 | 529 | 293–304 | 144 + 74 = 218 |

Our renderers are no heavier than Chrome's (idle: 2 renderers, 112 MB;
Chrome: 3, 105 MB). The gap is the `trailboard` process, ~300 MB, about
80 MB more than Chrome's browser and GPU processes together.

**Cause (to find).** What lives in the main process: CEF's browser side;
wgpu and, **on CI, llvmpipe itself** (software Vulkan runs in our process, so
its framebuffers and shader caches count here, where Chrome's sit in its
gpu-process); page textures (one 8 MB texture per 1080p card, plus the CEF
copy); fonts and glyph atlases; the board.

**Next to try.**
1. **Break it down on CI.** At the end of the idle case, `run.py` saves
   the main process's `smaps` and sums Pss by mapping: libcef, the Vulkan
   driver (llvmpipe, CI only), `[heap]` and anonymous maps, textures.
2. **Suspects, from the code.** The canvas (8 MB) and 2–3 swapchain
   images (8 MB each); per card the texture plus our CPU copy of its
   pixels (8 + 8 MB); wgpu's staging for `write_texture`; the glyph
   atlas; on CI, llvmpipe's per-thread tile buffers and JIT code (which
   Chrome keeps in its GPU process).
3. **Cheap experiment: glibc arenas.** CEF's and llvmpipe's threads each
   grow a malloc arena in our process; try `MALLOC_ARENA_MAX=2` in the
   bench environment first, then `mallopt(M_ARENA_MAX)` at start if it
   pays.
4. **Then cut:** drop the CPU copy once uploaded (PB1 (3) or (4) does
   it), free the canvas after a long idle.

**Goal.** Within 50 MB of Chrome's idle, or know which part we can't cut.

---

## Measurement gaps

- **No GPU in CI.** PB1 and PB2 are mostly software-rendering costs; on a
  real GPU they shrink and PB1's copies matter more. Laptop runs
  (`scripts/bench.py`) are what would show it; the owner prefers CI today.
- **Different CPUs per job.** Each case lands on its own runner (EPYC 7763,
  9V74, Xeon 6975P-C…). Compare Trailboard with Chrome *in the same job*,
  never across jobs.
- **Pan has no reference.** Only Trailboard is measured there.

# Performance bugs

What the CI bench says Trailboard does worse than other browsers, with the
evidence, what is already known, and what to try next. Ranked: the
first is the biggest gap. Move an entry into its milestone in
[PLAN.md](PLAN.md) when it is taken up, and delete it here when it is fixed.

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

**Next to try.**
1. **Draw a page shown at 1:1 as a plain copy.** When a card's page is
   drawn unscaled and axis-aligned, copy its pixels straight to the
   frame (`copy_texture_to_texture`, or a no-filter, no-blend pass over
   just its rect), and draw only the rounded corners, border and shadow
   round it with the general shader. Expected: most of the 21–30 ms goes.
   This is the lever for the CI numbers.
2. **Partial frames for a big card.** When only the page changed (video),
   the damage is the page rect; redraw that rect even when it is over
   `PARTIAL_MAX`, if what's round it is unchanged. Check it combines
   with (1).
3. **dmabuf paint** (`on_accelerated_paint`, PLAN §5): Chromium hands a
   GPU buffer per frame; importing it into wgpu (Vulkan
   `VK_EXT_external_memory_dma_buf` via wgpu-hal) drops both copies and the
   upload. Helps only with a GPU, so CI won't show it; needs a fallback
   (no GPU, Windows, import failure). Recommended as its own milestone (M16
   6f); the owner places it against M17.

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

**Next to try.**
1. Profile one pan frame (`TRAILBOARD_PERF=1` locally, or a `perf record`
   on CI) to split the 19 ms: background dots, page quads, shadows, text.
2. Cheapen what is drawn per pixel: shadows (blur is per pixel), the
   background dot pattern, page quads drawn at a scale (mipmaps / a
   cheaper filter while moving).
3. Cap the redraw rate while panning on a software renderer (e.g. 30 fps)
   if the device can't keep 60 anyway.

**Goal.** No reference browser has a board, so compare with ourselves:
halve it (≤ 175 %), keeping the motion smooth in the screenshots.

## PB3. Idle: 4× Chrome's CPU, with nothing happening 🟡

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

**Next to try.** Run idle locally with `TRAILBOARD_PERF=1` and log who calls
`request_redraw` and why each CEF paint happens (its dirty rect says what
moved). Fix the sources; the frames skipped should drop to ~0.

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

**Next to try.** Break the main process down (`/proc/<pid>/smaps_rollup`
and `smaps` by mapping, on CI and locally) into CEF, Vulkan driver, heap
and textures; check the copy of each page's pixels we keep after uploading
(could it go once it is on the GPU?).

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

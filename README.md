# Trailboard bench

This branch only measures. It holds no app code and makes no releases: it
builds Trailboard from `master` (or any commit), runs it and as many other
browsers as install on a GitHub Actions runner, and keeps the results.

- **Results:** [`results/README.md`](results/README.md) (the latest table and
  Trailboard over time), raw rows in `results/*.jsonl`.
- **Chart:** the GitHub Pages site of this repository: memory and CPU per
  case over time, one line per browser. Needs Settings → Pages → Source:
  *GitHub Actions* once.
- **Each run's summary:** the Actions tab → *Bench* → a run.

## When it runs

- Every push to `master`: that commit is measured. (`master` carries the
  same `.github/workflows/bench.yml`; GitHub runs a push's workflows from
  the pushed branch. It takes its tools from this branch and saves here.)
- Every push to this branch: `master` is measured with the new tools.
- By hand, for any Trailboard commit:
  `gh workflow run bench.yml --ref bench -f ref=<commit>`. The other inputs
  (`cases`, `only`, `runs`, `settle`, `sample`, `save`) make a quick try
  possible, e.g. `-f only=trailboard,chrome -f runs=1 -f save=false`.

A newer run cancels one still going.

## What it measures

Six cases, each its own job, all on identical runners (`ubuntu-24.04`,
an X server at 1920×1080, software rendering: no GPU, no video decoding
hardware):

| Case | What runs |
|---|---|
| `idle` | no pages: each browser's own start page, Trailboard's start card |
| `still` | six test pages loaded (news, encyclopedia, code, forum, docs, gallery), left alone |
| `video-vp9`, `video-av1`, `video-h264` | a 1080p30 clip looping in one page |
| `pan` | Trailboard only: the six pages, the board panned for the whole sample |

Each program runs three times; the programs take turns run by run (A B C,
A B C...), so a slow moment on the runner hits all of them alike. A run
waits `settle` seconds (25) for loading, then measures for `sample` seconds
(30):

- **Memory:** PSS summed over every process of the run (its session and its
  process tree), averaged over the sample. PSS shares memory fairly between
  processes, so the sum is the program's real footprint.
- **CPU:** CPU time over the sample, as a percentage of one core.
- **Processes, start time** (to the first window) and a breakdown of memory
  by process type, in the raw rows.

Every browser runs in a private window with a throwaway profile, first-run
screens off, its own sandbox on.

The test pages (`make_site.py`) are made here, with fixed seeds, so nothing
of anyone else's is copied and every run sees the same pages. They are
served from 127.0.0.11, .12, ... so each page is its own site, as on the web.

## Reading the numbers

These are a runner's numbers: everything draws and decodes in software, so
they compare the programs **with each other** and **over time**, not with a
desktop (Trailboard's `scripts/bench.py` measures on real hardware).

**The regression check** fails a run when Trailboard is more than 20 %
worse than in the last saved run on any case, in memory or CPU (CPU also by
at least one point), while the other browsers in the same run moved less
than 10 %, so a slower runner is not taken for a regression.

## Browsers

`browsers.toml` lists them, each with how to install and start it. A
browser that fails to install or open a window is listed as failed in that
run's summary and doesn't stop the others. To add one, add an entry and push.

The list is one browser per engine (Chrome for Blink, Firefox for Gecko,
GNOME Web for WebKit), Brave as a widely used Blink browser, and Falkon, which
embeds Chromium the way Trailboard embeds CEF. The other Blink and Gecko
browsers measured until 2026-11-27 came within a few points of Chrome or
Firefox and only lengthened the run; their results are still in `results/`.

## Files

| File | Does |
|---|---|
| `.github/workflows/bench.yml` | build → one job per case → report → Pages |
| `browsers.toml` | the browsers |
| `install.py` | installs them, notes which worked |
| `make_site.py` | the test pages and videos |
| `run.py` | runs and measures one case |
| `report.py` | summary, `results/`, chart, regression check |

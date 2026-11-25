#!/usr/bin/env python3
"""Turn the runs into results: the summary, results/README.md, the chart page,
and the regression check.

    python3 report.py NEW.jsonl --results results --summary $GITHUB_STEP_SUMMARY --pages pages-out

NEW.jsonl is this workflow run's rows; it is saved as results/<date>-<sha>.jsonl.
Exits 1 when Trailboard got more than 20 % worse than the last run on a case
(memory or CPU) while the other browsers in the same run moved less than
10 % (a slower runner is not a regression).
"""
import argparse, html, json, os, statistics, sys
from collections import defaultdict

WORSE = 0.20       # Trailboard this much worse than the last run: a regression
STEADY = 0.10      # ... if the other browsers moved less than this
CPU_FLOOR = 1.0    # and CPU rose by at least this many points (idle CPU is small and noisy)
CASES = ["idle", "still", "video-vp9", "video-av1", "video-h264", "pan"]


def load(path):
    rows = []
    for line in open(path):
        line = line.strip()
        if line:
            try:
                rows.append(json.loads(line))
            except ValueError:
                pass
    return rows


def medians(rows):
    """{(case, program): {...medians, n, failed, engine, version}} for one workflow run."""
    groups = defaultdict(list)
    for r in rows:
        groups[(r["case"], r["program"])].append(r)
    out = {}
    for key, rs in groups.items():
        ok = [r for r in rs if r.get("ok")]
        m = {"n": len(ok), "failed": len(rs) - len(ok), "engine": rs[0].get("engine", ""), "version": rs[0].get("version", ""),
             "error": next((r.get("error") for r in rs if r.get("error")), "")}
        for k in ("pss_mib", "peak_pss_mib", "cpu_pct", "procs", "startup_s"):
            vals = [r[k] for r in ok if r.get(k) is not None]
            m[k] = statistics.median(vals) if vals else None
        out[key] = m
    return out


def history(results_dir):
    """Every saved workflow run, oldest first: [(label, meta, medians)]."""
    runs = []
    for f in sorted(os.listdir(results_dir)):
        if f.endswith(".jsonl"):
            rows = load(os.path.join(results_dir, f))
            if rows:
                runs.append((f[:-6], rows[0], medians(rows)))
    return runs


def pct(new, old):
    return None if not old or new is None else (new - old) / old


def regressions(new, old):
    """Cases where Trailboard got worse while the others held steady."""
    found = []
    for case in CASES:
        tb_new, tb_old = new.get((case, "trailboard")), old.get((case, "trailboard"))
        if not tb_new or not tb_old or tb_new["pss_mib"] is None or tb_old["pss_mib"] is None:
            continue
        for metric in ("pss_mib", "cpu_pct"):
            change = pct(tb_new[metric], tb_old[metric])
            if change is None or change <= WORSE:
                continue
            if metric == "cpu_pct" and tb_new[metric] - tb_old[metric] < CPU_FLOOR:
                continue
            others = [pct(new[k][metric], old[k][metric]) for k in new
                      if k[0] == case and k[1] != "trailboard" and k in old and new[k][metric] and old[k][metric]]
            moved = statistics.median(others) if others else 0.0
            if abs(moved) < STEADY or not others:
                found.append(f"{case}: Trailboard {'memory (MB)' if metric == 'pss_mib' else 'CPU (%)'} "
                             f"{tb_old[metric]:.1f} → {tb_new[metric]:.1f} (+{change:.0%}); the others moved {moved:+.0%}")
    return found


def fmt(v, unit=""):
    return "–" if v is None else (f"{v:.0f}{unit}" if unit == " MB" else f"{v:.1f}{unit}" if unit == "" else f"{v}{unit}")


def table(med, prev=None):
    lines = []
    for case in CASES:
        keys = sorted([k for k in med if k[0] == case], key=lambda k: (k[1] != "trailboard", med[k]["pss_mib"] or 1e9))
        if not keys:
            continue
        lines += [f"\n### {case}\n", "| Program | Engine | Memory (PSS) | Peak | CPU % of a core | Processes | Start | Change since last run |",
                  "|---|---|---|---|---|---|---|---|"]
        for k in keys:
            m = med[k]
            name = f"**{k[1]}**" if k[1] == "trailboard" else k[1]
            if not m["n"]:
                lines.append(f"| {name} | {m['engine']} | failed: {m['error'][:60]} | | | | | |")
                continue
            change = ""
            if prev and k in prev and prev[k]["pss_mib"]:
                dm, dc = pct(m["pss_mib"], prev[k]["pss_mib"]), pct(m["cpu_pct"], prev[k]["cpu_pct"])
                change = f"mem {dm:+.0%}" + (f", CPU {dc:+.0%}" if dc is not None else "")
            lines.append(f"| {name} | {m['engine']} | {m['pss_mib']:.0f} MB | {m['peak_pss_mib']:.0f} MB | {m['cpu_pct']:.1f} | "
                         f"{m['procs']:.0f} | {m['startup_s']:.2f} s | {change} |")
    return "\n".join(lines)


COLORS = ["#d62728", "#1f77b4", "#2ca02c", "#ff7f0e", "#9467bd", "#8c564b", "#e377c2", "#7f7f7f", "#bcbd22", "#17becf",
          "#393b79", "#637939", "#8c6d31", "#843c39", "#7b4173", "#3182bd", "#e6550d", "#31a354", "#756bb1"]


def svg_chart(runs, case, metric, label, programs):
    """A line per program over the saved runs; Trailboard drawn thick."""
    w, h, pad = 900, 320, 48
    vals = [m[(case, p)][metric] for _, _, m in runs for p in programs if (case, p) in m and m[(case, p)][metric] is not None]
    if not vals:
        return ""
    top = max(vals) * 1.1 or 1
    n = len(runs)
    xs = lambda i: pad + (w - 2 * pad) * (i / max(n - 1, 1))
    ys = lambda v: h - pad - (h - 2 * pad) * (v / top)
    out = [f"<svg viewBox='0 0 {w} {h}' role=img aria-label='{html.escape(label)} for {case}'>",
           f"<line x1={pad} y1={h - pad} x2={w - pad} y2={h - pad} class=axis />"]
    for t in range(5):
        v = top * t / 4
        out.append(f"<text x={pad - 6} y={ys(v) + 4} text-anchor=end class=tick>{v:.0f}</text>"
                   f"<line x1={pad} y1={ys(v)} x2={w - pad} y2={ys(v)} class=grid />")
    for i, (labelr, meta, _) in enumerate(runs):
        if n <= 12 or i % max(n // 12, 1) == 0:
            out.append(f"<text x={xs(i)} y={h - pad + 16} text-anchor=middle class=tick>{html.escape(labelr[:8])}</text>")
    for j, p in enumerate(programs):
        pts = [(xs(i), ys(m[(case, p)][metric]), labelr, meta) for i, (labelr, meta, m) in enumerate(runs)
               if (case, p) in m and m[(case, p)][metric] is not None]
        if not pts:
            continue
        color = COLORS[j % len(COLORS)]
        width = 4 if p == "trailboard" else 1.5
        out.append(f"<polyline fill=none stroke='{color}' stroke-width={width} points='{' '.join(f'{x:.1f},{y:.1f}' for x, y, _, _ in pts)}' />")
        for x, y, labelr, meta in pts:
            link = meta.get("run_url", "#")
            out.append(f"<a href='{html.escape(link)}'><circle cx={x:.1f} cy={y:.1f} r={4 if p == 'trailboard' else 2.5} fill='{color}'>"
                       f"<title>{html.escape(p)} · {html.escape(labelr)} · {meta.get('trailboard_sha', '')[:7]}</title></circle></a>")
    out.append("</svg>")
    return "".join(out)


def pages(runs, out_dir):
    os.makedirs(out_dir, exist_ok=True)
    programs = sorted({k[1] for _, _, m in runs for k in m}, key=lambda p: (p != "trailboard", p))
    legend = "".join(f"<span><i style='background:{COLORS[j % len(COLORS)]}'></i>{html.escape(p)}</span>" for j, p in enumerate(programs))
    body = ""
    for case in CASES:
        mem = svg_chart(runs, case, "pss_mib", "Memory (MB, PSS)", programs)
        if not mem:
            continue
        cpu = svg_chart(runs, case, "cpu_pct", "CPU (% of a core)", programs)
        body += f"<section><h2>{case}</h2><div class=pair><figure><figcaption>Memory, MB (PSS)</figcaption>{mem}</figure>" \
                f"<figure><figcaption>CPU, % of a core</figcaption>{cpu}</figure></div></section>"
    last = runs[-1][1] if runs else {}
    doc = f"""<!doctype html><html lang=en><meta charset=utf-8><meta name=viewport content='width=device-width,initial-scale=1'>
<title>Trailboard bench</title>
<style>
:root{{--bg:#fff;--fg:#1c1c1c;--dim:#666;--line:#ddd}}
@media (prefers-color-scheme:dark){{:root{{--bg:#16161a;--fg:#e8e8ea;--dim:#9a9aa2;--line:#33333a}}}}
body{{margin:0;padding:24px 16px;font:15px/1.5 system-ui,sans-serif;background:var(--bg);color:var(--fg)}}
main{{max-width:1900px;margin:0 auto}} h1{{margin:0 0 4px}} p{{color:var(--dim);margin:0 0 16px}}
.legend{{display:flex;flex-wrap:wrap;gap:6px 16px;margin:12px 0 24px;font-size:13px}}
.legend i{{display:inline-block;width:12px;height:12px;border-radius:2px;margin-right:6px;vertical-align:-1px}}
.pair{{display:grid;grid-template-columns:repeat(auto-fit,minmax(min(100%,520px),1fr));gap:16px}}
figure{{margin:0}} figcaption{{color:var(--dim);font-size:13px}} svg{{width:100%;height:auto}}
.axis{{stroke:var(--dim)}} .grid{{stroke:var(--line);stroke-width:.5}} .tick{{fill:var(--dim);font-size:11px}}
section{{margin-bottom:32px}}
</style>
<main><h1>Trailboard against other browsers</h1>
<p>GitHub Actions runner, software rendering, saved test pages; three runs each, medians. Newest run:
Trailboard {html.escape(last.get('trailboard_sha', '')[:7])}, {html.escape(last.get('date', ''))}. Lower is better.
Each point links to its workflow run. Raw data: <a href="https://github.com/{html.escape(last.get('repo', ''))}/tree/bench/results">results/</a>.</p>
<div class=legend>{legend}</div>{body}</main></html>"""
    open(os.path.join(out_dir, "index.html"), "w").write(doc)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("new")
    ap.add_argument("--results", default="results")
    ap.add_argument("--summary", default=os.environ.get("GITHUB_STEP_SUMMARY", "/dev/stdout"))
    ap.add_argument("--pages", default="pages-out")
    a = ap.parse_args()

    rows = load(a.new)
    if not rows:
        print("no rows")
        return 1
    meta = rows[0]
    os.makedirs(a.results, exist_ok=True)
    before = history(a.results)
    name = f"{meta.get('date', 'unknown')}-{meta.get('trailboard_sha', 'unknown')[:7]}-{meta.get('run_id', '0')}"
    with open(os.path.join(a.results, name + ".jsonl"), "w") as f:
        f.writelines(json.dumps(r) + "\n" for r in rows)
    new = medians(rows)
    prev = before[-1][2] if before else None
    found = regressions(new, prev) if prev else []

    head = (f"## Bench: Trailboard {meta.get('trailboard_sha', '')[:7]} against {len({k[1] for k in new}) - 1} browsers\n\n"
            f"{meta.get('date', '')} · runner {meta.get('runner', '')} · {meta.get('runs', '?')} runs each, medians · "
            f"[workflow run]({meta.get('run_url', '#')})\n")
    verdict = ("\n> [!WARNING]\n> **Regression:**\n" + "".join(f"> - {f}\n" for f in found)) if found else \
              ("\n✅ No regression against the last run.\n" if prev else "\nFirst run: nothing to compare with yet.\n")
    missing = sorted({k[1] for k, m in new.items() if not m["n"]})
    note = f"\nFailed to run: {', '.join(missing)} (see the logs).\n" if missing else ""
    text = head + verdict + note + table(new, prev)
    open(a.summary, "a").write(text + "\n")

    runs = history(a.results)
    tb = ["| Run | Trailboard | " + " | ".join(f"{c} mem / CPU" for c in CASES) + " |", "|---|---|" + "---|" * len(CASES)]
    for label, m0, med in reversed(runs[-60:]):
        cells = []
        for c in CASES:
            t = med.get((c, "trailboard"))
            cells.append(f"{t['pss_mib']:.0f} MB / {t['cpu_pct']:.1f} %" if t and t["pss_mib"] is not None else "–")
        tb.append(f"| [{label[:15]}]({m0.get('run_url', '#')}) | `{m0.get('trailboard_sha', '')[:7]}` | " + " | ".join(cells) + " |")
    readme = (f"# Results\n\nWritten by the bench workflow after every run. Chart: "
              f"<https://{meta.get('repo', '').split('/')[0].lower()}.github.io/{meta.get('repo', '').split('/')[-1]}/>\n\n"
              f"{head}{verdict}{note}{table(new, prev)}\n\n## Trailboard over time (newest first)\n\n" + "\n".join(tb) + "\n")
    open(os.path.join(a.results, "README.md"), "w").write(readme)
    pages(runs, a.pages)
    print(text)
    return 1 if found else 0


if __name__ == "__main__":
    sys.exit(main())

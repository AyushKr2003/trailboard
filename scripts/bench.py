#!/usr/bin/env python3
"""Measure Trailboard's memory and CPU, and compare it with other programs.

    scripts/bench.py --snapshot              # once: save the test pages and videos
    scripts/bench.py                         # every case, 3 runs each, medians
    scripts/bench.py --case idle,still --runs 1
    scripts/bench.py --against 'ref=some-browser --profile-dir={profile} {urls}'

Each run starts the program alone inside its own systemd scope, puts its
window alone on workspace 9 (Hyprland), lets it load for SETTLE seconds and
then measures for SAMPLE seconds: PSS summed over every process of the run,
CPU time from the scopes' cgroups, process count. Rows are appended to
~/.cache/trailboard-bench/results.jsonl and the medians printed as Markdown.

Each program's runs stay in results.jsonl, so the reference browsers need
running only once: a Trailboard-only run adds their newest saved runs to its
table (`--no-saved` leaves them out), and `--report` prints the saved runs
without running anything.

Cases:
  idle        no pages: Trailboard's start card, or the other program's own
  still       six saved pages, loaded and left alone
  video-h264  a 1080p H.264 video looping in one page
  video-hevc  the same in HEVC
  pan         Trailboard only: the six pages, the board panned for SAMPLE s

Pages are saved copies served from 127.0.0.N (one address per page, so each
is its own site and gets its own renderer, as on the web). A snapshot keeps
the HTML, styles, images and fonts but drops scripts, so runs agree; `--live`
loads the real sites instead.

`--against NAME=CMD` adds another program. CMD is split like a shell line;
`{profile}` becomes a fresh, empty directory deleted after the run and
`{urls}` the case's pages (nothing for idle). Give it a private window and a
throwaway profile so no real profile is touched.
"""
import argparse, hashlib, html, json, os, re, shlex, shutil, signal, statistics, subprocess
import sys, threading, time, urllib.parse, urllib.request
from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer

HOME = os.path.expanduser("~/.cache/trailboard-bench")
SITE = os.path.join(HOME, "site")
RESULTS = os.path.join(HOME, "results.jsonl")
REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
WS = "9"
CASES = ["idle", "still", "video-h264", "video-hevc", "pan"]

# Pages that render on the server, so a copy without scripts still looks right.
PAGES = [
    ("wiki", "https://en.wikipedia.org/wiki/Web_browser"),
    ("repo", "https://github.com/rust-lang/rust"),
    ("news", "https://www.bbc.com/news"),
    ("docs", "https://developer.mozilla.org/en-US/docs/Web/JavaScript"),
    ("forum", "https://old.reddit.com/r/rust/"),
    ("wiki2", "https://wiki.archlinux.org/title/Wayland"),
]
UA = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/156.0 Safari/537.36"


# ---------------------------------------------------------------- snapshot

def fetch(url, limit=15 << 20):
    req = urllib.request.Request(url, headers={"User-Agent": UA, "Accept-Language": "en"})
    with urllib.request.urlopen(req, timeout=20) as r:
        return r.read(limit), r.headers.get_content_type(), r.geturl()


def ext_for(url, ctype):
    known = {"text/css": ".css", "image/png": ".png", "image/jpeg": ".jpg", "image/gif": ".gif",
             "image/webp": ".webp", "image/svg+xml": ".svg", "image/avif": ".avif",
             "font/woff2": ".woff2", "font/woff": ".woff", "font/ttf": ".ttf", "image/x-icon": ".ico"}
    if ctype in known:
        return known[ctype]
    e = os.path.splitext(urllib.parse.urlparse(url).path)[1].lower()
    return e if re.fullmatch(r"\.[a-z0-9]{1,5}", e) else ""


class Saver:
    """Downloads a page's resources into DIR/r/, once each."""

    def __init__(self, dir_):
        self.dir, self.done, self.count = dir_, {}, 0
        os.makedirs(os.path.join(dir_, "r"), exist_ok=True)

    def get(self, url, prefix):
        """The local path for URL (relative, with PREFIX), or None."""
        url = html.unescape(url).strip()
        if not url or url.startswith(("data:", "#", "javascript:", "about:", "mailto:")):
            return None
        if url in self.done:
            return self.done[url] and prefix + self.done[url]
        self.done[url] = None
        if self.count >= 400:
            return None
        try:
            body, ctype, final = fetch(url)
        except Exception as e:  # noqa: BLE001 - any failure leaves the link as is
            print(f"  skip {url[:90]}: {e}", file=sys.stderr)
            return None
        self.count += 1
        name = "r/" + hashlib.sha1(url.encode()).hexdigest()[:16] + ext_for(final, ctype)
        if ctype == "text/css":
            body = self.css(body.decode("utf-8", "replace"), final, "").encode()
        with open(os.path.join(self.dir, name), "wb") as f:
            f.write(body)
        self.done[url] = name[2:]  # relative to r/
        return prefix + name[2:]

    def css(self, text, base, prefix):
        """Save what a stylesheet refers to; files in r/ refer to each other bare."""
        def url_(m):
            local = self.get(urllib.parse.urljoin(base, m.group(2)), prefix)
            return f"url({local})" if local else m.group(0)
        text = re.sub(r"""url\(\s*(['"]?)([^'")]+)\1\s*\)""", url_, text)
        def imp(m):
            local = self.get(urllib.parse.urljoin(base, m.group(1)), prefix)
            return f'@import "{local}"' if local else m.group(0)
        return re.sub(r"""@import\s+['"]([^'"]+)['"]""", imp, text)


def snapshot_page(name, url):
    dir_ = os.path.join(SITE, name)
    shutil.rmtree(dir_, ignore_errors=True)
    os.makedirs(dir_)
    body, _, base = fetch(url)
    text = body.decode("utf-8", "replace")
    saver = Saver(dir_)
    text = re.sub(r"(?is)<script\b.*?</script\s*>", "", text)
    text = re.sub(r"(?is)<(base|meta[^>]+http-equiv=['\"]?content-security-policy)[^>]*>", "", text)
    text = re.sub(r"(?is)<link\b[^>]*\brel=['\"]?(preload|modulepreload|prefetch|preconnect|dns-prefetch)\b[^>]*>", "", text)
    text = re.sub(r"""(?i)\s(integrity|nonce)=("[^"]*"|'[^']*')""", "", text)

    def attr(m):
        local = saver.get(urllib.parse.urljoin(base, html.unescape(m.group(3))), "r/")
        return f"{m.group(1)}={m.group(2)}{local}{m.group(2)}" if local else m.group(0)

    # Stylesheets and icons by href; images, video posters and inputs by src.
    def link(m):
        tag = m.group(0)
        if re.search(r"(?i)rel=['\"]?[^'\">]*(stylesheet|icon)", tag):
            tag = re.sub(r"""(?i)(href)=(["'])([^"']+)\2""", attr, tag)
        return tag
    text = re.sub(r"(?is)<link\b[^>]*>", link, text)
    text = re.sub(r"""(?i)(?<=\s)(src|poster)=(["'])([^"']+)\2""", attr, text)

    def srcset(m):
        first = m.group(3).split(",")[0].strip().split(" ")[0]
        local = saver.get(urllib.parse.urljoin(base, html.unescape(first)), "r/")
        return f"{m.group(1)}={m.group(2)}{local}{m.group(2)}" if local else ""
    text = re.sub(r"""(?i)(?<=\s)(srcset)=(["'])([^"']+)\2""", srcset, text)
    text = re.sub(r"(?is)(<style\b[^>]*>)(.*?)(</style>)",
                  lambda m: m.group(1) + saver.css(m.group(2), base, "r/") + m.group(3), text)
    text = re.sub(r"""(?i)style=(["'])(.*?)\1""",
                  lambda m: f"style={m.group(1)}{saver.css(m.group(2), base, 'r/')}{m.group(1)}", text)
    # Links stay pointing at the real site; nothing here follows them.
    with open(os.path.join(dir_, "index.html"), "w") as f:
        f.write(text)
    print(f"{name}: {url} -> {saver.count} files")


def snapshot():
    os.makedirs(SITE, exist_ok=True)
    for name, url in PAGES:
        try:
            snapshot_page(name, url)
        except Exception as e:  # noqa: BLE001
            print(f"{name}: FAILED {e}", file=sys.stderr)
    vid = os.path.join(SITE, "video")
    os.makedirs(vid, exist_ok=True)
    src = ["-f", "lavfi", "-i", "testsrc2=size=1920x1080:rate=30:duration=30"]
    for codec, args in [("h264", ["-c:v", "libx264", "-profile:v", "high", "-b:v", "6M"]),
                        ("hevc", ["-c:v", "libx265", "-tag:v", "hvc1", "-b:v", "4M"])]:
        out = os.path.join(vid, f"{codec}.mp4")
        subprocess.run(["ffmpeg", "-y", "-loglevel", "error", *src, *args, "-pix_fmt", "yuv420p",
                        "-movflags", "+faststart", out], check=True)
        with open(os.path.join(vid, f"{codec}.html"), "w") as f:
            f.write(f"<!doctype html><title>{codec} 1080p</title><style>body{{margin:0;background:#000}}"
                    f"video{{width:100vw;height:100vh}}</style>"
                    f"<video src={codec}.mp4 autoplay muted loop playsinline></video>")
        print(f"video: {out}")


# ---------------------------------------------------------------- serving

class Quiet(SimpleHTTPRequestHandler):
    def log_message(self, *a):
        pass


class Server(ThreadingHTTPServer):
    def handle_error(self, request, client_address):
        # A program closed mid-request when its run was stopped: expected.
        if not isinstance(sys.exc_info()[1], (ConnectionError, BrokenPipeError)):
            super().handle_error(request, client_address)


def serve():
    """One server per page on 127.0.0.N (so each page is its own site). Returns the URLs by name."""
    urls = {}
    names = [n for n, _ in PAGES] + ["video"]
    for i, name in enumerate(names):
        host = f"127.0.0.{i + 11}"
        srv = Server((host, 0), partial(Quiet, directory=os.path.join(SITE, name)))
        threading.Thread(target=srv.serve_forever, daemon=True).start()
        urls[name] = f"http://{host}:{srv.server_address[1]}/"
    return urls


def case_urls(case, served, live):
    if case == "idle":
        return []
    if case.startswith("video"):
        return [served["video"] + case.split("-")[1] + ".html"]
    if live:
        return [u for _, u in PAGES]
    return [served[n] for n, _ in PAGES]


# ---------------------------------------------------------------- one run

def hypr(*a):
    return subprocess.run(["hyprctl", *a], capture_output=True, text=True).stdout


def pan_script(path, settle, sample):
    """Wait for the pages to load, then pan the board back and forth with touchpad deltas."""
    lines = [f"wait {settle * 1000}", "mem", "move 960 40", "log bench: pan starts"]
    frames = int(sample * 60)
    for i in range(frames):
        dx = 24 if (i // 90) % 2 == 0 else -24
        lines += [f"touchpad {dx} 0", "wait 16"]
    lines += ["touchpad 0 0 end", "log bench: pan ends", "mem"]
    with open(path, "w") as f:
        f.write("\n".join(lines) + "\n")


def measure(label, case, cmd, env, settle, sample, out):
    """Run CMD in a scope; return the figures."""
    unit = f"tb-bench-{label}-{case}-{int(time.time())}"
    before = {c["address"] for c in json.loads(hypr("clients", "-j"))}
    hypr("dispatch", f'hl.dsp.focus({{ workspace = "{WS}" }})')
    time.sleep(0.5)
    log = open(f"{out}.log", "w")
    proc = subprocess.Popen(["systemd-run", "--user", "--scope", "-q", f"--unit={unit}", "--"] + cmd,
                            stdout=log, stderr=subprocess.STDOUT, env=env)
    t0 = time.time()
    cg = None
    for _ in range(50):
        r = subprocess.run(["systemctl", "--user", "show", "-p", "ControlGroup", "--value", f"{unit}.scope"],
                           capture_output=True, text=True).stdout.strip()
        if r:
            cg = "/sys/fs/cgroup" + r
            break
        time.sleep(0.1)
    if not cg:
        proc.kill()
        sys.exit("bench: the run's scope never appeared")
    app_slice = os.path.dirname(cg)
    old_scopes = set(os.listdir(app_slice))

    def groups():
        # The engine moves its main process into a scope of its own.
        return [cg] + [f"{app_slice}/{d}" for d in os.listdir(app_slice)
                       if d not in old_scopes and d.startswith("app-org.chromium.Chromium-")]

    def pids():
        out_ = []
        for g in groups():
            try:
                out_ += [int(p) for p in open(f"{g}/cgroup.procs").read().split()]
            except OSError:
                pass
        return out_

    def pss():
        total = uss = bad = 0
        ps = pids()
        for p in ps:
            try:
                for line in open(f"/proc/{p}/smaps_rollup"):
                    k, v = line.split(":", 1)
                    if k == "Pss":
                        total += int(v.split()[0])
                    elif k in ("Private_Clean", "Private_Dirty"):
                        uss += int(v.split()[0])
            except (OSError, ValueError):
                bad += 1
        return total / 1024, uss / 1024, len(ps), bad

    def cpu_usec():
        total = 0
        for g in groups():
            try:
                total += next(int(l.split()[1]) for l in open(f"{g}/cpu.stat") if l.startswith("usage_usec"))
            except (OSError, StopIteration):
                pass
        return total

    def window():
        for c in json.loads(hypr("clients", "-j")):
            if c["address"] not in before and c["mapped"]:
                return c
        return None

    def alone(c):
        if str(c["workspace"]["id"]) != WS:
            hypr("dispatch", f'hl.dsp.window.move({{ workspace = "{WS}", follow = true, window = "address:{c["address"]}" }})')
        hypr("dispatch", f'hl.dsp.focus({{ workspace = "{WS}" }})')

    startup = None
    while time.time() - t0 < 60:
        w = window()
        if w:
            startup = time.time() - t0
            alone(w)
            break
        time.sleep(0.05)

    peak = 0
    end = t0 + settle
    while time.time() < end:
        peak = max(peak, pss()[0])
        time.sleep(2)

    c0, s0 = cpu_usec(), time.time()
    rows = []
    while time.time() - s0 < sample:
        rows.append(pss())
        time.sleep(3)
    c1, s1 = cpu_usec(), time.time()
    peak = max([peak] + [r[0] for r in rows])

    # PSS by process type at the end.
    by = {}
    for pid in pids():
        try:
            args = open(f"/proc/{pid}/cmdline").read().split("\0")
            kind = next((a.split("=", 1)[1] for a in args if a.startswith("--type=")), None)
            if kind == "utility":
                kind = next((a.split("=", 1)[1].split(".")[-1] for a in args if a.startswith("--utility-sub-type=")), kind)
            if kind is None:
                kind = "child:" + args[-1] if "-contentproc" in args else "main"
            p_ = sum(int(l.split()[1]) for l in open(f"/proc/{pid}/smaps_rollup") if l.startswith("Pss:")) / 1024
            k = by.setdefault(kind, [0, 0])
            k[0] += 1
            k[1] += p_
        except (OSError, ValueError):
            pass

    w = window()
    if w:
        alone(w)
        time.sleep(1)
        w = window() or w
        x, y = w["at"]
        ww, hh = w["size"]
        subprocess.run(["grim", "-g", f"{x},{y} {ww}x{hh}", f"{out}.png"])

    vaapi = False
    for p in pids():
        try:
            if "iHD_drv_video" in open(f"/proc/{p}/maps").read():
                vaapi = True
                break
        except OSError:
            pass

    extra = [os.path.basename(g) for g in groups()[1:]]
    for u in [f"{unit}.scope"] + extra:
        subprocess.run(["systemctl", "--user", "stop", u], capture_output=True)
    try:
        proc.wait(timeout=10)
    except subprocess.TimeoutExpired:
        for u in [f"{unit}.scope"] + extra:
            subprocess.run(["systemctl", "--user", "kill", "-s", "KILL", u], capture_output=True)
    log.close()
    time.sleep(2)

    n = max(len(rows), 1)
    return {
        "startup_s": round(startup, 2) if startup else None,
        "pss_mib": round(sum(r[0] for r in rows) / n),
        "uss_mib": round(sum(r[1] for r in rows) / n),
        "peak_pss_mib": round(peak),
        "procs": round(sum(r[2] for r in rows) / n),
        "cpu_pct": round((c1 - c0) / 1e6 / (s1 - s0) * 100, 1),
        "vaapi": vaapi,
        "by_kind": {k: [v[0], round(v[1])] for k, v in sorted(by.items(), key=lambda x: -x[1][1])},
    }


# ---------------------------------------------------------------- main

def summary(runs):
    """Medians of some runs' figures, the memory range and whether all used VA-API."""
    def med(k):
        vals = [r[k] for r in runs if r.get(k) is not None]
        return statistics.median(vals) if vals else None
    return ({k: med(k) for k in ("pss_mib", "peak_pss_mib", "cpu_pct", "procs", "startup_s")},
            min(r["pss_mib"] for r in runs), max(r["pss_mib"] for r in runs), all(r.get("vaapi") for r in runs))


def saved(case, live, skip):
    """Each program's newest saved batch for this case (not those in SKIP):
    the reference browsers needn't run again to be compared with."""
    batches = {}
    try:
        lines = open(RESULTS).read().splitlines()
    except OSError:
        return []
    for line in lines:
        try:
            r = json.loads(line)
        except ValueError:
            continue
        if r.get("case") != case or r.get("program") in skip or bool(r.get("live_sites")) != live:
            continue
        batches.setdefault(r["program"], {}).setdefault(r["when"], []).append(r)
    out = []
    for name, by_when in sorted(batches.items()):
        when = max(by_when)
        out.append((case, f"{name} (saved {when[:13]})", *summary(by_when[when])))
    return out


def print_table(medians):
    print("| Case | Program | Memory (PSS) | Range | Peak | CPU % of a core | Processes | Start | VA-API |")
    print("|---|---|---|---|---|---|---|---|---|")
    for case, name, m, lo, hi, va in medians:
        start = f"{m['startup_s']} s" if m["startup_s"] is not None else "–"
        print(f"| {case} | {name} | {m['pss_mib']:.0f} MB | {lo}–{hi} | {m['peak_pss_mib']:.0f} MB | "
              f"{m['cpu_pct']:.1f} | {m['procs']:.0f} | {start} | {'yes' if va else 'no'} |")


def git_rev():
    r = subprocess.run(["git", "-C", REPO, "describe", "--always", "--dirty"], capture_output=True, text=True)
    return r.stdout.strip()


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--snapshot", action="store_true", help="save the pages and make the videos, then stop")
    ap.add_argument("--case", default="all", help=f"comma-separated, from {', '.join(CASES)} (default all)")
    ap.add_argument("--runs", type=int, default=3)
    ap.add_argument("--settle", type=int, default=45, help="seconds to load before measuring")
    ap.add_argument("--sample", type=int, default=60, help="seconds measured")
    ap.add_argument("--binary", default=os.path.join(REPO, "target/release/trailboard"))
    ap.add_argument("--live-pages", type=int, help="Trailboard's live-card cap (default: its own default)")
    ap.add_argument("--live", action="store_true", help="load the real sites instead of the saved copies")
    ap.add_argument("--against", action="append", default=[], metavar="NAME=CMD")
    ap.add_argument("--no-trailboard", action="store_true", help="run only the --against programs")
    ap.add_argument("--note", default="", help="saved with each row (what changed)")
    ap.add_argument("--no-saved", action="store_true",
                    help="don't add the other programs' newest saved runs to the table")
    ap.add_argument("--report", action="store_true",
                    help="run nothing: print every program's newest saved runs for the cases")
    a = ap.parse_args()

    if a.snapshot:
        snapshot()
        return
    cases = CASES if a.case == "all" else a.case.split(",")
    for c in cases:
        if c not in CASES:
            sys.exit(f"bench: unknown case {c!r}")
    if a.report:
        print_table([row for c in cases for row in saved(c, a.live, set())])
        return
    if not a.live and not os.path.exists(os.path.join(SITE, "video", "h264.mp4")):
        sys.exit("bench: no saved pages; run `scripts/bench.py --snapshot` first")
    if not a.no_trailboard and not os.access(a.binary, os.X_OK):
        sys.exit(f"bench: no binary at {a.binary} (cargo build --release)")

    programs = [] if a.no_trailboard else [("trailboard", None)]
    for spec in a.against:
        name, _, cmd = spec.partition("=")
        if not cmd:
            sys.exit(f"bench: --against wants NAME=CMD, got {spec!r}")
        programs.append((name, cmd))

    served = serve()
    stamp = time.strftime("%Y%m%d-%H%M%S")
    outdir = os.path.join(HOME, "out", stamp)
    os.makedirs(outdir, exist_ok=True)
    rev = git_rev()
    medians = []
    for case in cases:
        urls = case_urls(case, served, a.live)
        for name, cmd in programs:
            if case == "pan" and cmd is not None:
                continue
            runs = []
            for i in range(a.runs):
                scratch = os.path.join(HOME, "scratch", f"{name}-{case}-{i}")
                shutil.rmtree(scratch, ignore_errors=True)
                os.makedirs(scratch)
                env = dict(os.environ)
                if cmd is None:
                    data = os.path.join(scratch, "data")
                    os.makedirs(os.path.join(data, "trailboard"))
                    # A board saved before, so there is no first-run welcome card.
                    with open(os.path.join(data, "trailboard", "spaces.json"), "w") as f:
                        json.dump({"version": 3, "active": 1, "next_id": 1, "spaces": [
                            {"id": 1, "name": "Space 1", "tint": None, "private": False}]}, f)
                    if a.live_pages:
                        with open(os.path.join(data, "trailboard", "settings.json"), "w") as f:
                            json.dump({"version": 2, "live_pages": a.live_pages}, f)
                    env["XDG_DATA_HOME"] = data
                    env.setdefault("RUST_LOG", "info")
                    if case == "pan":
                        pan_script(os.path.join(scratch, "script"), a.settle, a.sample)
                        env["TRAILBOARD_SCRIPT"] = os.path.join(scratch, "script")
                    argv = [a.binary, *urls]
                else:
                    profile = os.path.join(scratch, "profile")
                    os.makedirs(profile)
                    argv = []
                    for part in shlex.split(cmd):
                        if part == "{urls}":
                            argv += urls
                        else:
                            argv.append(part.replace("{profile}", profile))
                print(f"{case} {name} run {i + 1}/{a.runs} ...", file=sys.stderr, flush=True)
                r = measure(name, case, argv, env, a.settle, a.sample, os.path.join(outdir, f"{case}-{name}-{i}"))
                row = {"when": stamp, "rev": rev, "note": a.note, "case": case, "program": name, "run": i,
                       "live_sites": a.live, "live_pages": a.live_pages, **r}
                with open(RESULTS, "a") as f:
                    f.write(json.dumps(row) + "\n")
                print(f"  {json.dumps({k: v for k, v in row.items() if k != 'by_kind'})}", file=sys.stderr)
                runs.append(r)
                shutil.rmtree(scratch, ignore_errors=True)
            medians.append((case, name, *summary(runs)))
        if not a.no_saved:
            medians += saved(case, a.live, {n for n, _ in programs})

    print(f"\nRev {rev}, {a.runs} run(s) each, settle {a.settle} s, sample {a.sample} s, "
          f"{'live sites' if a.live else 'saved pages'}{', live cap ' + str(a.live_pages) if a.live_pages else ''}. Medians.\n")
    print_table(medians)
    print(f"\nLogs and screenshots: {outdir}")


if __name__ == "__main__":
    signal.signal(signal.SIGINT, signal.default_int_handler)
    main()

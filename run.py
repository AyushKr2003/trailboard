#!/usr/bin/env python3
"""Measure Trailboard and the installed browsers on one machine.

    python3 run.py --case still --trailboard PATH --installed installed.json \
        --site SITE_DIR --runs 3 --out results.jsonl --shots DIR

Runs on an X server (DISPLAY, e.g. Xvfb at 1920x1080). Each run starts one
program in a session of its own with a fresh profile, sizes its window to
the screen, lets it load for SETTLE seconds, then measures for SAMPLE
seconds: PSS summed over every process of the run (its session and its
process tree), CPU time over the window, process count. Programs take turns
run by run (A B C, A B C, ...) so a slow moment on the machine hits all of
them alike. A program that shows no window is recorded as failed.
"""
import argparse, json, os, re, shutil, signal, statistics, subprocess, sys, threading, time, tomllib
from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer

HERE = os.path.dirname(os.path.abspath(__file__))
W, H = 1920, 1080
CLK = os.sysconf("SC_CLK_TCK")
PAGES = ["news", "wiki", "code", "forum", "docs", "gallery"]
CASES = ["idle", "still", "video-vp9", "video-av1", "video-h264", "pan"]

GECKO_PREFS = {
    "browser.privatebrowsing.autostart": True,
    "browser.aboutwelcome.enabled": False,
    "browser.startup.homepage_override.mstone": "ignore",
    "startup.homepage_welcome_url": "",
    "startup.homepage_welcome_url.additional": "",
    "browser.shell.checkDefaultBrowser": False,
    "datareporting.policy.dataSubmissionEnabled": False,
    "toolkit.telemetry.reportingpolicy.firstRun": False,
    "browser.tabs.warnOnClose": False,
    "browser.sessionstore.resume_from_crash": False,
    "zen.welcome-screen.seen": True,
    "app.update.enabled": False,
    "app.update.auto": False,
    "media.autoplay.default": 0,
    # Floorp: no welcome page and no release-notes tab or prompt (without
    # these both open in front of the test pages).
    "floorp.browser.welcome.page.shown": True,
    "floorp.releaseNotes.mode": "disabled",
    "floorp.releaseNotes.choiceConfirmed": True,
    "floorp.releaseNotes.promptShown": True,
    "startup.homepage_override_url": "",
}


# ---------------------------------------------------------------- pages

# What the video page reported this run: [(time, "playing" | "cannot-play")]; measure() clears it.
BEACONS = []


class Quiet(SimpleHTTPRequestHandler):
    def log_message(self, *a):
        pass

    def do_GET(self):
        if self.path.startswith("/beacon/"):
            BEACONS.append((time.time(), self.path[len("/beacon/"):].split("?")[0]))
            self.send_response(204)
            self.end_headers()
            return
        super().do_GET()


class Server(ThreadingHTTPServer):
    daemon_threads = True

    def handle_error(self, request, client_address):
        if not isinstance(sys.exc_info()[1], (ConnectionError, BrokenPipeError)):
            super().handle_error(request, client_address)


def serve(site):
    """Each page from its own 127.0.0.N, so each is its own site (its own renderer)."""
    urls = {}
    for i, name in enumerate(PAGES + ["video"]):
        host = f"127.0.0.{i + 11}"
        # Images are shared: serve the whole site, link each page by its path.
        srv = Server((host, 0), partial(Quiet, directory=site))
        threading.Thread(target=srv.serve_forever, daemon=True).start()
        urls[name] = f"http://{host}:{srv.server_address[1]}/{name}/"
    return urls


def case_urls(case, urls):
    if case == "idle":
        return []
    if case.startswith("video-"):
        return [urls["video"] + case.split("-")[1] + ".html"]
    return [urls[n] for n in PAGES]


# ---------------------------------------------------------------- processes

def stat(pid):
    """(ppid, session, utime+stime ticks) or None."""
    try:
        f = open(f"/proc/{pid}/stat").read().rsplit(")", 1)[1].split()
        return int(f[1]), int(f[3]), int(f[11]) + int(f[12])
    except (OSError, IndexError, ValueError):
        return None


def members(root):
    """Pids of the run: its session, and anything descended from its first process."""
    table = {}
    for p in os.listdir("/proc"):
        if p.isdigit():
            s = stat(int(p))
            if s:
                table[int(p)] = s
    out = {p for p, s in table.items() if s[1] == root}
    kids = {}
    for p, s in table.items():
        kids.setdefault(s[0], []).append(p)
    todo = [root]
    while todo:
        p = todo.pop()
        if p in table:
            out.add(p)
        todo += [k for k in kids.get(p, []) if k not in out]
    return out, {p: table[p][2] for p in out}


def pss(pids):
    total = uss = 0
    for p in pids:
        try:
            for line in open(f"/proc/{p}/smaps_rollup"):
                k, v = line.split(":", 1)
                if k == "Pss":
                    total += int(v.split()[0])
                elif k in ("Private_Clean", "Private_Dirty"):
                    uss += int(v.split()[0])
        except (OSError, ValueError):
            pass
    return total / 1024, uss / 1024


PERF_KEYS = ["paints", "copied", "uploaded", "presented", "partial", "skipped", "render_ms", "render_pct"]


def perf_of(log_path, seconds):
    """Medians of Trailboard's TRAILBOARD_PERF lines over the last `seconds`
    of its log (one line a second while it draws), or None."""
    try:
        lines = [l for l in open(log_path, errors="replace") if "perf: paints" in l]
    except OSError:
        return None
    vals = []
    for l in lines[-int(seconds):]:
        nums = re.findall(r"[-0-9.]+(?=/s| MB/s| ms/frame| %)", l.split("perf:", 1)[1])
        if len(nums) == len(PERF_KEYS):
            vals.append([float(n) for n in nums])
    if not vals:
        return None
    return {k: round(statistics.median(v[i] for v in vals), 1) for i, k in enumerate(PERF_KEYS)} | {"lines": len(vals)}


def kind_of(pid):
    try:
        args = open(f"/proc/{pid}/cmdline").read().split("\0")
    except OSError:
        return "?"
    args = [a for a in args if a]
    # Chrome rewrites its title as one string, the arguments joined by spaces.
    if len(args) == 1 and " --" in args[0]:
        args = args[0].split(" ")
    t = next((a.split("=", 1)[1] for a in args if a.startswith("--type=")), None)
    if t == "zygote":
        # CEF's zygote-forked children keep the zygote's command line: tell
        # them apart by their threads (the GPU process runs the display
        # compositor, a renderer has a compositor thread, any child an IO thread).
        threads = set()
        try:
            for tid in os.listdir(f"/proc/{pid}/task"):
                threads.add(open(f"/proc/{pid}/task/{tid}/comm").read().strip())
        except OSError:
            pass
        if threads & {"VizCompositorTh", "CrGpuMain"}:
            return "gpu-process"
        if "Compositor" in threads:
            return "renderer"
        if "Chrome_ChildIOT" in threads:
            return "utility"
    if t == "utility":
        t = next((a.split("=", 1)[1].split(".")[-1] for a in args if a.startswith("--utility-sub-type=")), t)
    if t:
        return t
    if "-contentproc" in args:
        return "child:" + args[-1]
    return os.path.basename(args[0]) if args else "?"


# ---------------------------------------------------------------- windows

def x(*args):
    return subprocess.run(["xdotool", *args], capture_output=True, text=True).stdout.split()


def windows():
    return set(x("search", "--maxdepth", "1", "--onlyvisible", "--name", ""))


def biggest(ids):
    best, area = None, 0
    for w in ids:
        g = subprocess.run(["xdotool", "getwindowgeometry", "--shell", w], capture_output=True, text=True).stdout
        m = dict(l.split("=", 1) for l in g.split() if "=" in l)
        a = int(m.get("WIDTH", 0)) * int(m.get("HEIGHT", 0))
        if a > area:
            best, area = w, a
    return best if area >= 200 * 200 else None


def screenshot(path):
    subprocess.run(["ffmpeg", "-y", "-loglevel", "error", "-f", "x11grab", "-video_size", f"{W}x{H}",
                    "-i", os.environ.get("DISPLAY", ":99"), "-frames:v", "1", path], capture_output=True)


# ---------------------------------------------------------------- programs

def command(name, spec, case, urls, profile, a):
    """argv and environment for one run."""
    env = dict(os.environ)
    fam = spec["family"]
    if name == "trailboard":
        data = os.path.join(profile, "data")
        os.makedirs(os.path.join(data, "trailboard"))
        # A board saved before: no first-run welcome card.
        json.dump({"version": 3, "active": 1, "next_id": 1, "spaces": [
            {"id": 1, "name": "Space 1", "tint": None, "private": False}]}, open(os.path.join(data, "trailboard", "spaces.json"), "w"))
        # TRAILBOARD_PERF: a line a second on what drawing costs (paints,
        # copies, uploads, frames, render time); read back into the row.
        env.update(XDG_DATA_HOME=data, XDG_CACHE_HOME=os.path.join(profile, "cache"), RUST_LOG="info", TRAILBOARD_PERF="1")
        if case == "pan":
            script = os.path.join(profile, "script")
            # No card focused, so the touchpad pans the board: over a focused
            # card it scrolls the page (the first run of the first CI run did that).
            lines = [f"wait {a.settle * 1000}", "key Escape", "wait 500", "move 960 40",
                     "log bench: pan starts"]
            for i in range(int(a.sample * 60)):
                lines += [f"touchpad {24 if (i // 90) % 2 == 0 else -24} 0", "wait 16"]
            lines += ["touchpad 0 0 end", "log bench: pan ends"]
            open(script, "w").write("\n".join(lines) + "\n")
            env["TRAILBOARD_SCRIPT"] = script
        return [spec["bin"], *urls], env
    if fam == "chromium":
        return [spec["bin"], f"--user-data-dir={profile}", spec.get("private", "--incognito"), "--no-first-run",
                "--no-default-browser-check", "--password-store=basic", "--disable-session-crashed-bubble",
                "--autoplay-policy=no-user-gesture-required", "--window-position=0,0", f"--window-size={W},{H}",
                "--ozone-platform=x11", *urls], env
    if fam == "gecko":
        with open(os.path.join(profile, "user.js"), "w") as f:
            for k, v in GECKO_PREFS.items():
                f.write(f"user_pref({json.dumps(k)}, {json.dumps(v)});\n")
        # A home of its own: Gecko also keeps state outside --profile (the
        # profile registry, its cache), and on the runner it stopped at
        # "profile cannot be loaded" with the shared home.
        home = os.path.join(profile, "home")
        os.makedirs(home)
        env.update(MOZ_ENABLE_WAYLAND="0", MOZ_CRASHREPORTER_DISABLE="1", HOME=home,
                   XDG_CONFIG_HOME=os.path.join(home, ".config"), XDG_CACHE_HOME=os.path.join(home, ".cache"),
                   XDG_DATA_HOME=os.path.join(home, ".local", "share"))
        return [spec["bin"], "--profile", profile, "--no-remote", "--width", str(W), "--height", str(H), *urls], env
    if fam == "epiphany":
        # Its settings live in GSettings, not the profile: a key file of the
        # run's own stops the "Set as Default Browser?" dialog.
        conf = os.path.join(profile, "config")
        os.makedirs(os.path.join(conf, "glib-2.0", "settings"))
        open(os.path.join(conf, "glib-2.0", "settings", "keyfile"), "w").write(
            "[org/gnome/epiphany]\nask-for-default=false\n")
        env.update(GSETTINGS_BACKEND="keyfile", XDG_CONFIG_HOME=conf)
        return [spec["bin"], "--private-instance", f"--profile={profile}", *urls], env
    if fam == "falkon":
        env.update(XDG_CONFIG_HOME=os.path.join(profile, "config"), XDG_DATA_HOME=os.path.join(profile, "data"),
                   XDG_CACHE_HOME=os.path.join(profile, "cache"))
        return [spec["bin"], "--private-browsing", "--no-remote", *urls], env
    if fam == "qutebrowser":
        return [spec["bin"], "--basedir", profile, "-s", "content.private_browsing", "true", *urls], env
    raise ValueError(f"unknown family {fam}")


def measure(name, spec, case, urls, a, shot):
    profile = os.path.join(a.scratch, f"{name}-{case}")
    shutil.rmtree(profile, ignore_errors=True)
    os.makedirs(profile)
    argv, env = command(name, spec, case, urls, profile, a)
    BEACONS.clear()
    before = windows()
    log = open(shot + ".log", "w")
    t0 = time.time()
    proc = subprocess.Popen(argv, env=env, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
    root = proc.pid
    row = {"ok": False, "error": ""}
    try:
        win, startup = None, None
        while time.time() - t0 < 60:
            # A launcher may exit after starting the real program; give up only
            # when nothing of the run is left.
            if proc.poll() is not None and not members(root)[0]:
                break
            win = biggest(windows() - before)
            if win:
                startup = time.time() - t0
                subprocess.run(["xdotool", "windowmove", win, "0", "0", "windowsize", win, str(W), str(H)], capture_output=True)
                break
            time.sleep(0.1)
        if not win:
            row["error"] = "no window within 60 s" if proc.poll() is None else f"exited with {proc.returncode} and no window"
            return row
        peak = 0.0
        while time.time() < t0 + a.settle:
            peak = max(peak, pss(members(root)[0])[0])
            time.sleep(2)
        # CPU over the window: per process, from its first sight (or the
        # start) to its last; a process that exits keeps what it used.
        first, last, kinds = {}, {}, {}
        pids, ticks = members(root)
        first.update(ticks)
        last.update(ticks)
        kinds.update({p: kind_of(p) for p in ticks})
        s0 = time.time()
        samples = []
        n = 0
        while time.time() - s0 < a.sample:
            time.sleep(1)
            pids, ticks = members(root)
            for p, t in ticks.items():
                first.setdefault(p, 0 if n else t)
                last[p] = t
                if p not in kinds:
                    kinds[p] = kind_of(p)
            n += 1
            if n % 3 == 0:
                samples.append((*pss(pids), len(pids)))
        s1 = time.time()
        # The video page reports to our server every 2 s while it plays
        # (make_site.py); it must have played through the measured window, to
        # its last few seconds. This checks every program alike, Trailboard included.
        if case.startswith("video-"):
            playing = [t for t, s in BEACONS if s == "playing" and s0 <= t <= s1]
            if not playing or max(playing) < s1 - 6:
                names = [" ".join(subprocess.run(["xdotool", "getwindowname", w], capture_output=True, text=True).stdout.split())
                         for w in windows() - before]
                screenshot(shot + ".png")
                said = ", ".join(sorted({s for _, s in BEACONS})) or "nothing"
                why = "video not playing" if not playing else f"video stopped {s1 - max(playing):.0f} s before the end"
                row["error"] = f"{why} (page said {said}; windows: {'; '.join(n for n in names if n)[:100]})"
                return row
        cpu_s = sum(last[p] - first[p] for p in last) / CLK
        if not samples:
            samples.append((*pss(pids), len(pids)))
        peak = max([peak] + [s[0] for s in samples])
        # Per process kind: [count, PSS MiB, CPU % of one core]. CPU counts
        # every process seen in the window, PSS and count the ones left at its end.
        span = s1 - s0
        by = {}
        for p in pids:
            k = by.setdefault(kinds.get(p) or kind_of(p), [0, 0.0, 0.0])
            k[0] += 1
            k[1] += pss([p])[0]
        for p in last:
            by.setdefault(kinds.get(p, "?"), [0, 0.0, 0.0])[2] += (last[p] - first[p]) / CLK / span * 100
        screenshot(shot + ".png")
        if name == "trailboard":
            log.flush()
            row["perf"] = perf_of(shot + ".log", a.sample)
        row.update(ok=True, startup_s=round(startup, 2),
                   pss_mib=round(statistics.mean(s[0] for s in samples)), uss_mib=round(statistics.mean(s[1] for s in samples)),
                   peak_pss_mib=round(peak), procs=round(statistics.mean(s[2] for s in samples)),
                   cpu_pct=round(cpu_s / (s1 - s0) * 100, 1),
                   by_kind={k: [v[0], round(v[1]), round(v[2], 1)] for k, v in sorted(by.items(), key=lambda kv: -kv[1][1])})
        return row
    finally:
        pids, _ = members(root)
        for sig in (signal.SIGTERM, signal.SIGKILL):
            for p in pids | {root}:
                try:
                    os.kill(p, sig)
                except OSError:
                    pass
            try:
                proc.wait(timeout=8)
            except subprocess.TimeoutExpired:
                pass
            pids, _ = members(root)
            if not pids:
                break
        log.close()
        time.sleep(2)
        shutil.rmtree(profile, ignore_errors=True)


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--case", required=True, choices=CASES)
    ap.add_argument("--trailboard", required=True, help="the trailboard binary")
    ap.add_argument("--trailboard-version", default="")
    ap.add_argument("--installed", default="installed.json")
    ap.add_argument("--site", required=True)
    ap.add_argument("--runs", type=int, default=3)
    ap.add_argument("--settle", type=int, default=25)
    ap.add_argument("--sample", type=int, default=30)
    ap.add_argument("--only", default="", help="comma-separated programs (default: all installed)")
    ap.add_argument("--out", required=True)
    ap.add_argument("--shots", required=True)
    ap.add_argument("--scratch", default=os.path.expanduser("~/bench-scratch"))
    ap.add_argument("--meta", default="{}", help="JSON saved with every row (commit, run id, runner)")
    a = ap.parse_args()

    browsers = tomllib.load(open(os.path.join(HERE, "browsers.toml"), "rb"))
    installed = json.load(open(a.installed)) if os.path.exists(a.installed) else {}
    programs = [("trailboard", {"family": "trailboard", "bin": os.path.abspath(a.trailboard), "engine": "Blink (CEF)"},
                 a.trailboard_version)]
    if a.case != "pan":
        for name, spec in browsers.items():
            got = installed.get(name, {})
            if got.get("ok") and a.case not in spec.get("skip", []):
                programs.append((name, {**spec, "bin": got["bin"]}, got.get("version", "")))
    if a.only:
        keep = set(a.only.split(","))
        programs = [p for p in programs if p[0] in keep]
    os.makedirs(a.shots, exist_ok=True)
    os.makedirs(a.scratch, exist_ok=True)
    urls = case_urls(a.case, serve(os.path.abspath(a.site)))
    meta = json.loads(a.meta)
    # A first launch on a fresh runner reads the program from disk; the first
    # measured run was the slow one (pan run 0 managed two thirds of the
    # panning). Start each program once, briefly, unmeasured.
    warm = argparse.Namespace(**{**vars(a), "settle": 8, "sample": 0})
    for name, spec, _ in programs:
        measure(name, spec, a.case, urls, warm, os.path.join(a.scratch, f"warmup-{a.case}-{name}"))
    print(f"case {a.case}: {len(programs)} programs, {a.runs} runs each", flush=True)
    for i in range(a.runs):
        for name, spec, ver in programs:
            t = time.time()
            r = measure(name, spec, a.case, urls, a, os.path.join(a.shots, f"{a.case}-{name}-{i}"))
            row = {**meta, "case": a.case, "program": name, "engine": spec.get("engine", ""), "version": ver, "run": i, **r}
            with open(a.out, "a") as f:
                f.write(json.dumps(row) + "\n")
            brief = {k: row.get(k) for k in ("program", "run", "ok", "pss_mib", "cpu_pct", "procs", "startup_s", "error")}
            print(f"{a.case} {json.dumps(brief)} ({time.time() - t:.0f} s)", flush=True)


if __name__ == "__main__":
    main()

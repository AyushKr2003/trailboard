#!/usr/bin/env python3
"""Install the browsers in browsers.toml and say which worked.

    sudo -E env HOME=/root python3 install.py [NAME...] --out installed.json

Each install runs on its own (a failure skips that browser only) with a time
limit. Writes {name: {"ok": bool, "bin": path, "version": str, "error": str}}.
"""
import argparse, glob, json, os, shutil, subprocess, sys, tomllib

HERE = os.path.dirname(os.path.abspath(__file__))

HELPERS = r'''
set -euo pipefail
export DEBIAN_FRONTEND=noninteractive
# gh_get REPO REGEX OUT: the newest release asset of REPO whose name matches REGEX.
gh_get() {
  # The pattern goes to jq as data: pasted into the filter, its \. was a bad escape.
  url=$(gh api "repos/$1/releases/latest" | jq -r --arg re "$2" '.assets[] | select(.name | test($re)) | .browser_download_url' | head -1)
  [ -n "$url" ] || { echo "no asset matching $2 in $1" >&2; return 1; }
  curl -fsSLo "$3" "$url"
}
# apt_repo NAME KEY_URL "URL SUITE COMPONENTS": an apt source signed by KEY_URL.
apt_repo() {
  curl -fsSL "$2" | gpg --dearmor --yes -o "/usr/share/keyrings/$1.gpg"
  echo "deb [arch=amd64 signed-by=/usr/share/keyrings/$1.gpg] $3" > "/etc/apt/sources.list.d/$1.list"
  apt-get update -qq
}
'''


def resolve(bin_):
    if "/" not in bin_:
        return shutil.which(bin_)
    hits = sorted(glob.glob(bin_))
    return hits[0] if hits and os.access(hits[0], os.X_OK) else None


def version(path):
    try:
        out = subprocess.run([path, "--version"], capture_output=True, text=True, timeout=30,
                             env={**os.environ, "DISPLAY": os.environ.get("DISPLAY", ":99")})
        lines = [l for l in (out.stdout + out.stderr).splitlines() if l.strip() and "error" not in l.lower()]
        return lines[-1].strip() if lines else ""
    except Exception as e:  # noqa: BLE001
        return f"? ({e})"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("names", nargs="*")
    ap.add_argument("--out", default="installed.json")
    a = ap.parse_args()
    browsers = tomllib.load(open(os.path.join(HERE, "browsers.toml"), "rb"))
    names = a.names or list(browsers)
    result = {}
    for name in names:
        b = browsers[name]
        print(f"::group::install {name}", flush=True)
        err = ""
        # One retry: a mirror or a release download can be slow or fail once.
        for attempt in (1, 2):
            try:
                subprocess.run(["bash", "-c", HELPERS + b["install"]], check=True, timeout=b.get("timeout", 600))
                err = ""
                break
            except Exception as e:  # noqa: BLE001
                err = str(e).split("\n")[0][:60] + " … " + str(e)[-160:]
                print(f"attempt {attempt} failed: {err}", flush=True)
        path = resolve(b["bin"])
        if not path and not err:
            err = f"{b['bin']} not found after installing"
        result[name] = {"ok": bool(path), "bin": path, "version": version(path) if path else "", "error": err if not path else ""}
        print(json.dumps({name: result[name]}), flush=True)
        print("::endgroup::", flush=True)
    json.dump(result, open(a.out, "w"), indent=1)
    ok = [n for n, r in result.items() if r["ok"]]
    print(f"installed {len(ok)}/{len(result)}: {', '.join(ok)}")
    bad = [f"{n} ({r['error'][:80]})" for n, r in result.items() if not r["ok"]]
    if bad:
        print("not installed: " + "; ".join(bad))


if __name__ == "__main__":
    sys.exit(main())

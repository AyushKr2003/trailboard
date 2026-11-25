#!/usr/bin/env python3
"""Make the test pages and videos: `python3 make_site.py OUT_DIR`.

Six pages shaped like common sites (all made here, so nothing of anyone
else's is copied): a news front page, an encyclopedia article, a code
repository, a forum thread, a documentation page and a photo gallery. They
are still once loaded (no timers), with a little script as real pages have.
Plus one page per video codec, playing a 1080p clip in a loop. The output is
the same every time (fixed seeds).
"""
import os, random, subprocess, sys

WORDS = ("the of and to in is that for it as was with be by on not he this are or his from at which but have an "
         "they you were her all she there would their we him been has when who will more no if out so said what up "
         "its about into than them can only other new some could time these two may then do first any my now such "
         "like our over man me even most made after also did many before must through back years where much your way "
         "well down should because each just those people how too little state good very make world still own see men "
         "work long get here between both life being under never day same another know while last might us great old "
         "year off come since against go came right used take three board trail card page engine render memory").split()


def words(rng, n):
    return " ".join(rng.choice(WORDS) for _ in range(n))


def sentence(rng):
    s = words(rng, rng.randint(8, 22))
    return s[0].upper() + s[1:] + "."


def para(rng, n=None):
    return " ".join(sentence(rng) for _ in range(n or rng.randint(3, 7)))


BASE_CSS = """
*{box-sizing:border-box} body{margin:0;font:16px/1.5 system-ui,sans-serif;color:#222;background:#fff}
header{display:flex;gap:24px;align-items:center;padding:12px 32px;border-bottom:1px solid #ddd;position:sticky;top:0;background:#fff}
header nav a{margin-right:16px;color:#333;text-decoration:none} main{max-width:1200px;margin:0 auto;padding:24px 32px}
img{max-width:100%;display:block;border-radius:6px} a{color:#0b57d0} code,pre{font-family:ui-monospace,monospace}
footer{padding:32px;color:#666;border-top:1px solid #ddd;margin-top:48px}
"""

SCRIPT = """<script>
// What pages commonly do once loaded: build part of the page, measure it, hook events.
document.addEventListener('DOMContentLoaded', () => {
  const list = document.querySelector('[data-more]');
  if (list) for (let i = 0; i < 40; i++) {
    const li = document.createElement('li'); li.textContent = 'Related item ' + (i + 1); list.appendChild(li);
  }
  document.querySelectorAll('a').forEach(a => a.addEventListener('mouseenter', () => a.dataset.seen = 1));
  document.body.dataset.height = document.body.scrollHeight;
});
</script>"""


def page(title, body, extra_css=""):
    nav = "".join(f'<a href="#">{w.title()}</a>' for w in ["home", "news", "world", "work", "life", "about"])
    return (f"<!doctype html><html lang=en><meta charset=utf-8><meta name=viewport content='width=device-width'>"
            f"<title>{title}</title><style>{BASE_CSS}{extra_css}</style>"
            f"<header><b>{title.split(' ')[0]}</b><nav>{nav}</nav></header><main>{body}</main>"
            f"<footer>{words(random.Random(1), 40)}<ul data-more></ul></footer>{SCRIPT}</html>")


def news(rng, imgs):
    cards = "".join(
        f"<article><img src='../img/{imgs[i % len(imgs)]}' alt=''><h3>{sentence(rng)[:70]}</h3><p>{para(rng, 2)}</p></article>"
        for i in range(36))
    css = "main>section{display:grid;grid-template-columns:repeat(4,1fr);gap:24px} h3{font-size:18px;margin:8px 0}"
    return page("Daily news: front page", f"<h1>Top stories</h1><section>{cards}</section>", css)


def wiki(rng, imgs):
    toc = "".join(f"<li><a href='#s{i}'>{words(rng, 3).title()}</a></li>" for i in range(14))
    secs = ""
    for i in range(14):
        rows = "".join(f"<tr><td>{words(rng, 2)}</td><td>{rng.randint(1, 9999)}</td><td>{words(rng, 4)}</td></tr>" for _ in range(8))
        fig = f"<figure style='float:right;width:300px;margin:0 0 16px 24px'><img src='../img/{imgs[i % len(imgs)]}'><figcaption>{sentence(rng)}</figcaption></figure>" if i % 3 == 0 else ""
        refs = "".join(f"<sup><a href='#r{rng.randint(1, 120)}'>[{rng.randint(1, 120)}]</a></sup>" for _ in range(3))
        secs += f"<h2 id=s{i}>{words(rng, 3).title()}</h2>{fig}" + "".join(f"<p>{para(rng)}{refs}</p>" for _ in range(5))
        secs += f"<table border=1 cellpadding=4>{rows}</table>"
    refs = "".join(f"<li id=r{i}>{sentence(rng)} <a href='#'>link</a></li>" for i in range(120))
    css = "aside{position:fixed;left:0;top:60px;width:220px;padding:16px;font-size:14px} main{margin-left:240px} table{border-collapse:collapse;margin:16px 0}"
    return page("Encyclopedia article", f"<aside><b>Contents</b><ol>{toc}</ol></aside><h1>{words(rng, 2).title()}</h1>{secs}<h2>References</h2><ol>{refs}</ol>", css)


def code(rng, imgs):
    files = "".join(f"<tr><td>📁 {rng.choice(['src', 'docs', 'tests', 'scripts', 'crates'])}/{words(rng, 1)}</td><td>{sentence(rng)[:60]}</td><td>{rng.randint(1, 30)} days ago</td></tr>" for _ in range(30))
    kw = ["fn", "let", "mut", "pub", "struct", "impl", "match", "return", "use", "if", "else", "for"]
    lines = "".join(
        f"<div><span class=n>{i + 1}</span>{'    ' * rng.randint(0, 3)}<span class=k>{rng.choice(kw)}</span> "
        f"<span class=i>{words(rng, 1)}</span>(<span class=s>\"{words(rng, 2)}\"</span>, {rng.randint(0, 99)}); <span class=c>// {words(rng, 5)}</span></div>"
        for i in range(400))
    css = (".n{color:#999;display:inline-block;width:48px}.k{color:#a626a4}.s{color:#50a14f}.c{color:#a0a1a7}.i{color:#4078f2}"
           "pre{background:#fafafa;border:1px solid #ddd;padding:16px;font-size:13px} table{width:100%;border-collapse:collapse} td{border-bottom:1px solid #eee;padding:6px}")
    return page("Code repository", f"<h1>org / project</h1><table>{files}</table><h2>README</h2><p>{para(rng)}</p><pre>{lines}</pre>", css)


def forum(rng, imgs):
    posts = ""
    for i in range(150):
        depth = rng.choice([0, 0, 1, 1, 2, 3])
        posts += (f"<div class=post style='margin-left:{depth * 28}px'><div class=meta><b>user{rng.randint(100, 99999)}</b> · "
                  f"{rng.randint(1, 900)} points · {rng.randint(1, 23)} hours ago</div><p>{para(rng, rng.randint(1, 4))}</p>"
                  f"<div class=meta>reply · share · report</div></div>")
    css = ".post{border-left:2px solid #ddd;padding:4px 12px;margin:8px 0}.meta{font-size:12px;color:#777}"
    return page("Forum thread", f"<h1>{sentence(rng)}</h1><p>{para(rng)}</p>{posts}", css)


def docs(rng, imgs):
    side = "".join(f"<li><a href='#'>{words(rng, 2)}</a></li>" for _ in range(60))
    body = ""
    for i in range(12):
        body += f"<h2>{words(rng, 3).title()}</h2><p>{para(rng)}</p><pre><code>"
        body += "\n".join(f"const {words(rng, 1)} = {words(rng, 1)}.{words(rng, 1)}({rng.randint(0, 9)});" for _ in range(8))
        body += f"</code></pre><p>{para(rng)}</p>"
        body += "<table><tr><th>Name</th><th>Type</th><th>Description</th></tr>" + "".join(
            f"<tr><td><code>{words(rng, 1)}</code></td><td>{rng.choice(['string', 'number', 'object'])}</td><td>{sentence(rng)}</td></tr>" for _ in range(5)) + "</table>"
    css = "nav.side{position:fixed;top:60px;left:0;width:260px;bottom:0;overflow:auto;font-size:14px;padding:16px} main{margin-left:280px} pre{background:#f4f4f4;padding:12px}"
    return page("Documentation", f"<nav class=side><ul>{side}</ul></nav><h1>{words(rng, 2).title()}</h1>{body}", css)


def gallery(rng, imgs):
    tiles = "".join(f"<figure><img src='../img/{imgs[i % len(imgs)]}' loading=eager><figcaption>{words(rng, 3)}</figcaption></figure>" for i in range(48))
    css = "section{display:grid;grid-template-columns:repeat(4,1fr);gap:16px} figure{margin:0}"
    return page("Photo gallery", f"<h1>Gallery</h1><section>{tiles}</section>", css)


PAGES = {"news": news, "wiki": wiki, "code": code, "forum": forum, "docs": docs, "gallery": gallery}
VIDEOS = {
    "vp9": ("webm", ["-c:v", "libvpx-vp9", "-deadline", "realtime", "-cpu-used", "8", "-row-mt", "1", "-b:v", "4M"]),
    "av1": ("webm", ["-c:v", "libsvtav1", "-preset", "10", "-b:v", "3M"]),
    "h264": ("mp4", ["-c:v", "libx264", "-preset", "veryfast", "-b:v", "6M"]),
}


def ffmpeg(*args):
    subprocess.run(["ffmpeg", "-y", "-loglevel", "error", *args], check=True)


def main():
    out = sys.argv[1] if len(sys.argv) > 1 else "site"
    os.makedirs(os.path.join(out, "img"), exist_ok=True)
    imgs = []
    for i, src in enumerate(["mandelbrot", "testsrc2", "life=s=800x450:mold=10:r=1", "cellauto=rule=110:s=800x450", "smptehdbars", "gradients=s=800x450:seed=3"] * 2):
        name = f"p{i}.jpg"
        spec = src if "=" in src else f"{src}=s=800x450"
        ffmpeg("-f", "lavfi", "-i", spec, "-ss", str(i % 5), "-frames:v", "1", "-q:v", "4", os.path.join(out, "img", name))
        imgs.append(name)
    for name, make in PAGES.items():
        os.makedirs(os.path.join(out, name), exist_ok=True)
        with open(os.path.join(out, name, "index.html"), "w") as f:
            f.write(make(random.Random(name), imgs))
    vid = os.path.join(out, "video")
    os.makedirs(vid, exist_ok=True)
    for codec, (ext, args) in VIDEOS.items():
        clip = f"{codec}.{ext}"
        try:
            ffmpeg("-f", "lavfi", "-i", "testsrc2=size=1920x1080:rate=30:duration=20", *args, "-pix_fmt", "yuv420p", os.path.join(vid, clip))
        except subprocess.CalledProcessError:
            if codec != "av1":
                raise
            ffmpeg("-f", "lavfi", "-i", "testsrc2=size=1920x1080:rate=30:duration=20", "-c:v", "libaom-av1", "-cpu-used", "8",
                   "-row-mt", "1", "-b:v", "3M", "-pix_fmt", "yuv420p", os.path.join(vid, clip))
        with open(os.path.join(vid, f"{codec}.html"), "w") as f:
            f.write(f"<!doctype html><title>{codec} 1080p</title><style>body{{margin:0;background:#000}}video{{width:100vw;height:100vh}}</style>"
                    f"<video src={clip} autoplay muted loop playsinline></video>")
    print(f"site in {out}: {', '.join(PAGES)} and videos {', '.join(VIDEOS)}")


if __name__ == "__main__":
    main()

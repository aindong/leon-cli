"""The website's build hook (website/mkdocs.yml). It adds what lives outside docs/: the README as
overview.md, the landing page, the brand's font and logo, a What's new page per release from
CHANGELOG.md, and the demo page with whatever media website/fetch-media.sh fetched. Nothing is
copied into git. Links are written for GitHub, relative to each file's place in the repo; here a
link to a site page stays a site link, a link to any other repo file goes to GitHub at
LEON_SITE_REF, and a link to nothing is left for the strict build to report.

The docs, README and changelog come from LEON_SITE_ROOT (the latest release's checkout in
site.yml); the site's own files (website/, brand/) from the checkout this file is in.
"""

import os
import posixpath
import re
from pathlib import Path

from mkdocs.structure.files import File

TOOLS = Path(__file__).resolve().parent.parent
GITHUB = "https://github.com/aindong/leon-cli"
# a media file's name goes into the demo page's HTML: plain names only
MEDIA = re.compile(r"[a-z0-9][a-z0-9._-]*\.(mp4|webm|jpg|png)")
# an inline code span, or a Markdown link's target, or an HTML attribute that holds a URL
LINK = re.compile(r"(`+)[^`].*?\1|(\]\()([^)\s]+)|(\b(?:src|href|poster)=\")([^\"]+)", re.S)
FENCE = re.compile(r"^\s*(```|~~~)")

state = {}


def releases(changelog):
    """[(version, date, body)] for each `## [X.Y.Z]` section, newest first as the file has them.
    The body is the text scripts/release-notes.sh prints before it makes the links absolute."""
    out = []
    current = None
    for line in changelog.split("\n"):
        if line.startswith("## ["):
            m = re.match(r"## \[(\d+\.\d+\.\d+[^\]]*)\](?: - (\S+))?", line)
            current = [m[1], m[2] or "", []] if m else None
            if current:
                out.append(current)
        elif current:
            current[2].append(line)
    bodies = [(v, d, re.sub(r"^(\s*\n)+", "", "\n".join(body)).rstrip("\n")) for v, d, body in out]
    return [(v, d, b) for v, d, b in bodies if b.strip()]


def rewrite(text, origin, page, staged, root, ref, urls):
    """`text` (Markdown with HTML) with its links resolved from `origin`, its path in the repo, for
    the site page at `page`. `staged` maps a repo path to its page or file on the site, `urls` a site
    path to its URL (for raw HTML, which the site generator doesn't rewrite)."""

    def target(link, html):
        if re.match(r"[a-zA-Z][a-zA-Z0-9+.-]*:", link) or link.startswith(("#", "/")):
            return None
        path, hash, anchor = link.partition("#")
        repo = posixpath.normpath(posixpath.join(posixpath.dirname(origin), path))
        if repo == ".." or repo.startswith("../"):
            return None
        site = staged.get(repo) or staged.get(posixpath.join(repo, "README.md"))
        if site:
            if html:
                here = posixpath.dirname(urls.get(page, page))
                there = urls.get(site, site)
                out = posixpath.relpath(there or ".", here or ".") + ("/" if there.endswith("/") else "")
            else:
                out = posixpath.relpath(site, posixpath.dirname(page) or ".")
            return out + hash + anchor
        full = Path(root, repo)
        if not full.exists():
            return None
        kind = "tree" if full.is_dir() else ("raw" if html and link.endswith((".png", ".gif", ".jpg", ".svg")) else "blob")
        return f"{GITHUB}/{kind}/{ref}/{repo}{'/' if path.endswith('/') else ''}{hash}{anchor}"

    def one(m):
        if m[1]:
            return m[0]
        lead, link = (m[2], m[3]) if m[2] else (m[4], m[5])
        new = target(link, html=not m[2])
        return lead + (new or link)

    out, chunk, fenced = [], [], None
    for line in text.split("\n"):
        f = FENCE.match(line)
        if fenced:
            out.append(line)
            if f and f[1] == fenced:
                fenced = None
        elif f:
            if chunk:
                out.append(LINK.sub(one, "\n".join(chunk)))
            chunk = []
            out.append(line)
            fenced = f[1]
        else:
            chunk.append(line)
    if chunk:
        out.append(LINK.sub(one, "\n".join(chunk)))
    return "\n".join(out)


def on_config(config):
    root = Path(os.environ.get("LEON_SITE_ROOT") or TOOLS).resolve()
    config.docs_dir = str(root / "docs")
    media = os.environ.get("LEON_SITE_MEDIA")
    state.update(
        root=root,
        ref=os.environ.get("LEON_SITE_REF") or "dev",
        media=Path(media) if media else None,
        releases=releases((root / "CHANGELOG.md").read_text(encoding="utf-8")),
        langs=[a["lang"] for a in config.extra["alternate"] if a["lang"] != "en"],
    )
    for item in config.nav:
        if isinstance(item, dict) and "What's new" in item:
            pages = item["What's new"]
            at = pages.index("whats-new/index.md") + 1
            pages[at:at] = [{f"leon {v}": f"whats-new/{v}.md"} for v, _, _ in state["releases"]]
    return config


def on_files(files, config):
    root, tools = state["root"], TOOLS
    # site path -> (where its links are relative to in the repo, the repo path it stands for)
    origin = {f.src_uri: f"docs/{f.src_uri}" for f in files if not f.inclusion.is_excluded()}
    context = dict(origin)

    def add(uri, repo=None, links=None, **source):
        files.append(File.generated(config, uri, **source))
        origin[uri] = repo or uri
        context[uri] = links or repo or uri

    add("overview.md", "README.md", abs_src_path=str(root / "README.md"))
    add("index.html", abs_src_path=str(tools / "brand/marketing/index.html"))
    add("og.png", abs_src_path=str(tools / "brand/marketing/og.png"))
    add("assets/leon.css", abs_src_path=str(tools / "website/leon.css"))
    add("assets/logo-mark.svg", abs_src_path=str(tools / "brand/logo-mark.svg"))
    for name in ("instrument-sans-latin.woff2", "OFL.txt"):
        add(f"assets/fonts/{name}", abs_src_path=str(tools / "brand/fonts" / name))

    lines = [
        "# What's new",
        "",
        "Each release of leon, newest first. [The changelog's detail](docs/changelog-detail.md) says",
        f"more about each feature, and [CHANGELOG.md]({GITHUB}/blob/{state['ref']}/CHANGELOG.md) has every change.",
        "",
    ]
    for version, date, body in state["releases"]:
        lines.append(f"- [leon {version}](whats-new/{version}.md)" + (f" · {date}" if date else ""))
        head = f"# leon {version}\n\n"
        meta = [date] if date else []
        meta.append(f"[the release on GitHub]({GITHUB}/releases/tag/v{version})")
        add(f"whats-new/{version}.md", links="CHANGELOG.md", content=head + " · ".join(meta) + "\n\n" + body + "\n")
    add("whats-new/index.md", links="CHANGELOG.md", content="\n".join(lines) + "\n")

    media = []
    if state["media"] and state["media"].is_dir():
        media = sorted(p.name for p in state["media"].iterdir() if MEDIA.fullmatch(p.name) and p.is_file())
    for name in media:
        add(f"media/{name}", abs_src_path=str(state["media"] / name))
    add("demo.md", links="README.md", content=demo(media))

    # what the Markdown of each page is checked against: repo path -> site path
    state["staged"] = {repo: uri for uri, repo in origin.items()}
    state["context"] = context
    state["urls"] = {f.src_uri: f.url for f in files}
    if root != TOOLS:
        # an older release's checkout may not have every page the nav names yet
        config.nav = prune(config.nav, state["urls"])
    return files


def prune(nav, pages):
    out = []
    for item in nav:
        value = next(iter(item.values())) if isinstance(item, dict) else item
        if isinstance(value, list):
            value = prune(value, pages)
            if value:
                out.append({next(iter(item)): value} if isinstance(item, dict) else value)
        elif value in pages or "://" in value:
            out.append(item)
    return out


def demo(media):
    videos = sorted((n for n in media if n.endswith((".mp4", ".webm"))), key=lambda n: (not n.startswith("demo."), n))
    lines = ["# Demo", ""]
    if not videos:
        lines += [
            "The narrated tour is on its way. Until then, leon at work:",
            "",
            '<img src="docs/images/demo-app.gif" alt="The leon app: splitting a tab, switching tabs and sessions, a popup, and an agent working, then blocked" width="100%">',
            "",
        ]
    for video in videos:
        stem = video.rsplit(".", 1)[0]
        poster = next((f"media/{stem}.{e}" for e in ("jpg", "png") if f"{stem}.{e}" in media), "docs/images/app-overview.png")
        title = "A tour of leon" if stem == "demo" else stem.replace("-", " ").replace("_", " ").capitalize()
        lines += [
            f"## {title}",
            "",
            f'<video controls preload="none" playsinline width="100%" poster="{poster}" src="media/{video}"></video>',
            "",
        ]
    lines += [
        "## The command",
        "",
        '<img src="docs/images/demo-cli.gif" alt="The leon command: a new session, a run that waits, typing into Python, then leon attach and a split" width="100%">',
        "",
        "Each release's own clips are on [What's new](whats-new/index.md).",
    ]
    return "\n".join(lines) + "\n"


def on_page_markdown(markdown, page, config, files):
    uri = page.file.src_uri
    return rewrite(markdown, state["context"].get(uri, uri), uri, state["staged"], state["root"], state["ref"], state["urls"])


def on_page_context(context, page, config, nav):
    # the switcher: this page's own translation, or the language's home (alternate.html)
    uri = page.file.src_uri
    m = re.fullmatch(r"i18n/([^/]+)/(.+)", uri)
    english = uri if not m else ("overview.md" if m[2] == "README.md" else m[2])
    there = state["urls"]
    page.leon_alternates = {"en": there[english]} if english in there else {}
    for lang in state["langs"]:
        t = f"i18n/{lang}/" + ("README.md" if english == "overview.md" else english)
        if t in there:
            page.leon_alternates[lang] = there[t]
    return context


def on_post_page(output, page, config):
    return re.sub(r'\s*<meta name="generator"[^>]*>', "", output, count=1)


def on_post_template(output, template_name, config):
    return on_post_page(output, None, config)  # 404.html

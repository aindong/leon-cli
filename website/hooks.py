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

from mkdocs.exceptions import PluginError
from mkdocs.structure.files import File

TOOLS = Path(__file__).resolve().parent.parent
GITHUB = "https://github.com/aindong/leon-cli"
# a media file's name goes into the demo page's HTML: plain names only
MEDIA = re.compile(r"[a-z0-9][a-z0-9._-]*\.(mp4|webm|jpg|png)")
# an inline code span (within a paragraph), a Markdown link or image, a reference definition, or
# an HTML attribute that holds a URL
LINK = re.compile(
    r"(?P<code>(?P<tick>`+)(?:(?!\n[ \t]*\n).)+?(?P=tick)(?!`))"
    r"|(?P<md>(?P<img>!)?\[[^\]]*\]\()(?P<mdlink>[^)\s]+)"
    r"|(?P<ref>^[ \t]*\[[^\]\n]+\]:[ \t]+)(?P<reflink>\S+)"
    r"|(?P<attr>\b(?:src|href|poster)=\")(?P<attrlink>[^\"]+)",
    re.S | re.M,
)
FENCE = re.compile(r"^[ \t]*(`{3,}|~{3,})(.*)$")
# what would run on the public site: translations come from contributors, and GitHub's preview
# strips these where the site would not, so the build refuses them outside code
UNSAFE = re.compile(r"<(?:script|iframe|object|embed)\b|\son[a-z]+\s*=|javascript:", re.I)
# the files in docs/ the site serves: pages and pictures
SERVED = (".md", ".png", ".gif", ".jpg", ".jpeg", ".webp")

state = {}


def releases(changelog):
    """[(version, date, body)] for each `## [X.Y.Z]` section, newest first as the file has them.
    The body is the text scripts/release-notes.sh prints before it makes the links absolute."""
    out = []
    current = None
    for line in changelog.split("\n"):
        if line.startswith("## ["):
            m = re.match(r"## \[(\d+\.\d+\.\d+(?:-[0-9A-Za-z.]+)?)\](?: - (\S+))?", line)
            current = [m[1], m[2] or "", []] if m else None
            if current:
                out.append(current)
        elif current:
            current[2].append(line)
    bodies = [(v, d, re.sub(r"^\n+", "", "\n".join(body)).rstrip("\n")) for v, d, body in out]
    return [(v, d, b) for v, d, b in bodies if b.strip()]


def split(text):
    """[(is_code, text)]: `text` cut into its fenced code blocks and the rest. A fence closes on a run
    of its own character at least as long, with nothing after it."""
    out, lines, fence = [], [], None

    def cut(code):
        if lines:
            out.append((code, "\n".join(lines)))
            lines.clear()

    for line in text.split("\n"):
        m = FENCE.match(line)
        if fence:
            lines.append(line)
            if m and m[1][0] == fence[0] and len(m[1]) >= len(fence) and not m[2].strip():
                cut(True)
                fence = None
        elif m:
            cut(False)
            fence = m[1]
            lines.append(line)
        else:
            lines.append(line)
    cut(fence is not None)
    return out


def prose(text):
    """`text` without its code: fenced blocks and inline spans."""
    rest = "\n".join(t for code, t in split(text) if not code)
    return LINK.sub(lambda m: "" if m["code"] else m[0], rest)


def rewrite(text, origin, page, staged, root, ref, urls):
    """`text` (Markdown with HTML) with its links resolved from `origin`, its path in the repo, for
    the site page at `page`. `staged` maps a repo path to its page or file on the site, `urls` a site
    path to its URL (for raw HTML, which the site generator doesn't rewrite)."""

    def target(link, html, raw):
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
        kind = "tree" if full.is_dir() else ("raw" if raw else "blob")
        return f"{GITHUB}/{kind}/{ref}/{repo}{'/' if path.endswith('/') else ''}{hash}{anchor}"

    def one(m):
        if m["code"]:
            return m[0]
        if m["md"]:
            lead, link, html, raw = m["md"], m["mdlink"], False, bool(m["img"])
        elif m["ref"]:
            lead, link, html, raw = m["ref"], m["reflink"], False, False
        else:
            lead, link, html = m["attr"], m["attrlink"], True
            raw = not lead.startswith("href")
        return lead + (target(link, html, raw) or link)

    return "\n".join(t if code else LINK.sub(one, t) for code, t in split(text))


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
    section = next(i["What's new"] for i in config.nav if isinstance(i, dict) and "What's new" in i)
    at = section.index("whats-new/index.md") + 1
    section[at:at] = [{f"leon {v}": f"whats-new/{v}.md"} for v, _, _ in state["releases"]]
    return config


def on_files(files, config):
    root, tools = state["root"], TOOLS
    # site path -> (where its links are relative to in the repo, the repo path it stands for)
    docs = [f for f in files if f.src_dir == config.docs_dir and not f.inclusion.is_excluded()]
    origin = {f.src_uri: f"docs/{f.src_uri}" for f in docs}
    odd = [uri for uri in origin if not uri.lower().endswith(SERVED)]
    if odd:
        raise PluginError(f"docs/ has files the site would serve as they are, not pages or pictures: {odd}")
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
    state["refs"] = {}
    for version, date, body in state["releases"]:
        lines.append(f"- [leon {version}](whats-new/{version}.md)" + (f" · {date}" if date else ""))
        add(f"whats-new/{version}.md", links="CHANGELOG.md", content=release_page(version, date, body))
        # its links into the source go where its GitHub release's notes go: its own tag
        state["refs"][f"whats-new/{version}.md"] = f"v{version}"
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


def release_page(version, date, body):
    """A release's page: its changelog section, whose `#anchor` links are anchors of CHANGELOG.md."""
    meta = [date] if date else []
    meta.append(f"[the release on GitHub]({GITHUB}/releases/tag/v{version})")
    body = re.sub(r"\]\(#", "](CHANGELOG.md#", body)
    body = re.sub(r"^([ \t]*\[[^\]\n]+\]:[ \t]+)#", r"\1CHANGELOG.md#", body, flags=re.M)
    return f"# leon {version}\n\n" + " · ".join(meta) + "\n\n" + body + "\n"


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
    bad = UNSAFE.search(prose(markdown))
    if bad:
        raise PluginError(f"{uri}: {bad[0]!r} would run on the website; leave HTML scripts and handlers out of the docs")
    ref = state["refs"].get(uri, state["ref"])
    return rewrite(markdown, state["context"].get(uri, uri), uri, state["staged"], state["root"], ref, state["urls"])


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
    return on_post_page(output, None, config)  # the static templates: 404.html, the sitemap

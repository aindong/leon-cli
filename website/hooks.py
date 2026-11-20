"""The website's build hook (website/mkdocs.yml). It adds what lives outside docs/: the README as
overview.md, the landing page, the brand's font and logo, a What's new page per release from
CHANGELOG.md, and the demo page with whatever media website/fetch-media.sh fetched. Nothing is
copied into git. Links are written for GitHub, relative to each file's place in the repo; here a
link to a site page stays a site link, a link to any other repo file goes to GitHub at
LEON_SITE_REF, and a link to nothing is left for the strict build to report.

The docs, README and changelog come from LEON_SITE_ROOT (the latest release's checkout in
site.yml); the site's own files (website/, brand/) from the checkout this file is in.
"""

import hashlib
import http.client
import os
import posixpath
import re
import tempfile
import urllib.request
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
INCLUDE = re.compile(r"<!--#include (sections/[a-z0-9-]+\.html) -->")
FENCE = re.compile(r"^[ \t]*(`{3,}|~{3,})(.*)$")
# what would run on the public site: translations come from contributors, and GitHub's preview
# strips these where the site would not, so the build refuses them in a page's HTML (where code is
# escaped text, so the docs' examples pass): a script or frame tag, an on* handler in a tag, or a
# script URL in an attribute
UNSAFE = re.compile(r"<(?:script|iframe|object|embed|frame)\b|<[a-z][^>]*\son[a-z]+\s*=", re.I)
# an attribute's value in a tag, which a browser reads after decoding its character references
ATTR = re.compile(r"""<[a-z][^>]*>""", re.I)
VALUE = re.compile(r"""=\s*(?:"([^"]*)"|'([^']*)'|([^\s"'>]+))""")
# the files in docs/ the site serves: pages and pictures
SERVED = (".md", ".png", ".gif", ".jpg", ".jpeg", ".webp", ".svg")
# the diagrams' renderer, served from the site itself so a visitor's browser asks no other site for
# it: fetched once at build time and taken only if it is this exact file (npm's mermaid-11.17.2.tgz
# holds the same bytes); a new version is a new pin and hash
MERMAID_VERSION = "11.17.2"
MERMAID_SHA256 = "581ed7d74bd9048d0e3a91363927d72ef22942d7722546b27f7cc29e35390eb8"
MERMAID_URL = f"https://cdn.jsdelivr.net/npm/mermaid@{MERMAID_VERSION}/dist/mermaid.min.js"
DIAGRAM = re.compile(r'<pre class="(?:[^"]*\s)?mermaid[\s"]')

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


def every_release(root):
    """`releases` of CHANGELOG.md, then of docs/changelog-archive.md (earlier releases), whose
    links start from docs/: made CHANGELOG.md's again, which is where they were at the tag."""
    archive = root / "docs/changelog-archive.md"
    older = releases(archive.read_text(encoding="utf-8")) if archive.exists() else []
    latest = releases((root / "CHANGELOG.md").read_text(encoding="utf-8"))
    # a version in both is CHANGELOG.md's, as scripts/release-notes.sh reads it
    return latest + [(v, d, rooted(body)) for v, d, body in older if v not in {r[0] for r in latest}]


def rooted(body):
    """A docs/ file's relative links, as links from the repo's root."""
    up = lambda path: path[3:] if path.startswith("../") else f"docs/{path}"
    body = re.sub(r"\]\(([^):#\s][^):\s]*)\)", lambda m: f"]({up(m[1])})", body)
    return re.sub(r"^(\[[^\]\n]+\]:[ \t]+)([^:#\s][^:\s]*)(?=\s|$)", lambda m: m[1] + up(m[2]), body, flags=re.M)


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


def mermaid(cache, fetch=None):
    """The pinned mermaid.min.js in `cache`, fetched when it isn't there or isn't the pinned file."""
    path = Path(cache) / f"mermaid-{MERMAID_VERSION}.min.js"
    if path.exists() and hashlib.sha256(path.read_bytes()).hexdigest() == MERMAID_SHA256:
        return path
    try:
        data = (fetch or download)(MERMAID_URL)
    except (OSError, http.client.HTTPException) as e:
        raise PluginError(f"fetching {MERMAID_URL}: {e}") from e
    got = hashlib.sha256(data).hexdigest()
    if got != MERMAID_SHA256:
        path.unlink(missing_ok=True)
        raise PluginError(f"{MERMAID_URL} has sha256 {got}, not the pinned {MERMAID_SHA256}")
    path.parent.mkdir(parents=True, exist_ok=True)
    # a name of its own, so two builds filling the cache at once never move each other's file
    with tempfile.NamedTemporaryFile(dir=path.parent, suffix=".part", delete=False) as tmp:
        tmp.write(data)
    Path(tmp.name).replace(path)
    return path


def download(url):
    with urllib.request.urlopen(url, timeout=60) as r:
        return r.read()


def on_config(config):
    root = Path(os.environ.get("LEON_SITE_ROOT") or TOOLS).resolve()
    config.docs_dir = str(root / "docs")
    media = os.environ.get("LEON_SITE_MEDIA")
    state.update(
        root=root,
        ref=os.environ.get("LEON_SITE_REF") or "dev",
        media=Path(media) if media else None,
        releases=every_release(root),
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
    marketing = tools / "brand/marketing"
    add("index.html", content=landing(marketing))
    add("og.png", abs_src_path=str(marketing / "og.png"))
    add("share.jpg", abs_src_path=str(marketing / "share.jpg"))
    # the landing page's pictures, styles and scripts; its fonts are brand/fonts (assets/fonts is a link)
    for path in sorted((marketing / "assets").rglob("*")):
        uri = path.relative_to(marketing).as_posix()
        if path.is_file() and not uri.startswith("assets/fonts/"):
            add(uri, abs_src_path=str(path))
    add("assets/leon.css", abs_src_path=str(tools / "website/leon.css"))
    add("assets/logo-mark.svg", abs_src_path=str(tools / "brand/logo-mark.svg"))
    add("assets/mermaid.min.js", abs_src_path=str(mermaid(tools / "target/site-cache")))
    for font in sorted((tools / "brand/fonts").iterdir()):
        add(f"assets/fonts/{font.name}", abs_src_path=str(font))

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


def landing(folder):
    """brand/marketing/index.html with each `<!--#include sections/x.html -->` replaced by that file:
    the page is kept as a file per section so no file grows past the repo's size limit."""

    def section(m):
        return (folder / m[1]).read_text(encoding="utf-8").rstrip("\n")

    return INCLUDE.sub(section, (folder / "index.html").read_text(encoding="utf-8"))


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
    ref = state["refs"].get(uri, state["ref"])
    return rewrite(markdown, state["context"].get(uri, uri), uri, state["staged"], state["root"], ref, state["urls"])


def script_url(html):
    """The first attribute value in `html`'s tags that a browser would run as a script URL."""
    import html as entities

    for tag in ATTR.findall(html):
        for m in VALUE.finditer(tag):
            value = entities.unescape(m[1] or m[2] or m[3] or "")
            # browsers drop whitespace and control characters in a URL's scheme
            scheme = re.sub(r"[\x00-\x20]", "", value).lower()
            if scheme.startswith(("javascript:", "vbscript:", "data:text/html")):
                return value
    return None


def on_page_content(html, page, config, files):
    bad = UNSAFE.search(html) or script_url(html)
    if bad:
        bad = bad if isinstance(bad, str) else bad[0]
        raise PluginError(f"{page.file.src_uri}: {bad[:60]!r} would run on the website; leave scripts and handlers out of the docs")
    return html


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
    output = re.sub(r'\s*<meta name="generator"[^>]*>', "", output, count=1)
    if page and DIAGRAM.search(output):
        # in the head, so it is defined before the theme's bundle looks for it (else the theme
        # fetches its own from a CDN)
        src = posixpath.relpath("assets/mermaid.min.js", page.url.rstrip("/") or ".")  # directory URLs
        output = output.replace("</head>", f'<script src="{src}"></script></head>', 1)
    return output


def on_post_template(output, template_name, config):
    return on_post_page(output, None, config)  # the static templates: 404.html, the sitemap

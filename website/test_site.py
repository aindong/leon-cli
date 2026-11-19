"""The website's checks (.agent/plans/2026-11-19-site-and-release-kit.md): builds the site into a
temp dir and reads the result, and checks the hook's link rules and changelog reader on their own.

    target/site-venv/bin/python website/test_site.py   (website/build.sh makes the venv)

Run it as a script: `-m unittest` with a path would import it as a module of Python's own `site`.
"""

import os
import re
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

import hooks  # beside this file, which is on sys.path when it runs as a script

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent

GITHUB = "https://github.com/aindong/leon-cli"
LANGS = ["zh-CN", "ja", "ko", "es", "pt-BR"]


def build(out, media=None):
    env = dict(os.environ, LEON_SITE_REF="v9.9.9")
    env.pop("LEON_SITE_ROOT", None)
    env.pop("LEON_SITE_MEDIA", None)
    if media:
        env["LEON_SITE_MEDIA"] = str(media)
    mkdocs = Path(sys.executable).parent / "mkdocs"
    r = subprocess.run(
        [str(mkdocs), "build", "--strict", "-f", str(HERE / "mkdocs.yml"), "-d", str(out)],
        env=env,
        capture_output=True,
        text=True,
    )
    if r.returncode != 0:
        raise AssertionError(f"the strict build failed:\n{r.stdout}\n{r.stderr}")


def read(path):
    return Path(path).read_text(encoding="utf-8")


class Built(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.tmp = tempfile.TemporaryDirectory()
        cls.out = Path(cls.tmp.name) / "site"
        build(cls.out)

    @classmethod
    def tearDownClass(cls):
        cls.tmp.cleanup()

    def page(self, url):
        return read(self.out / url / "index.html")

    def test_every_doc_is_a_page_and_the_landing_page_is_home(self):
        landing = read(self.out / "index.html")
        self.assertIn("Keep your", landing)
        for link in re.findall(r'href="([^"#:]+)"', landing):
            page = self.out / link / "index.html" if link.endswith("/") else self.out / link
            self.assertTrue(page.exists(), f"the landing page's {link}")
        for name in ("Docs", "What's new", "Demo", "Install", "GitHub"):
            self.assertIn(f">{name}", landing)
        self.assertIn("leon is a terminal multiplexer", self.page("overview"))
        tracked = subprocess.run(
            ["git", "ls-files", "docs"], cwd=ROOT, capture_output=True, text=True, check=True
        ).stdout.split()
        docs = [p for p in tracked if p.endswith(".md") and not p.startswith("docs/i18n/")]
        self.assertGreater(len(docs), 30)
        for doc in docs:
            url = doc[len("docs/") : -len(".md")]
            self.assertTrue((self.out / url / "index.html").exists(), f"{doc} has a page")
        for sub in ("cli", "lua", "architecture"):
            self.assertTrue(any(d.startswith(f"docs/{sub}/") for d in docs), sub)
        for lang in LANGS:
            self.assertTrue((self.out / "i18n" / lang / "index.html").exists(), lang)
            self.assertTrue((self.out / "i18n" / lang / "install" / "index.html").exists(), lang)

    def test_the_landing_page_is_whole_and_asks_no_other_server(self):
        landing = read(self.out / "index.html")
        self.assertNotIn("<!--#include", landing, "every section is in place")
        for section in ("hero-h", "app-h", "agents-h", "statement-h", "handoff-h", "cli-h", "questions-h", "close-h"):
            self.assertIn(f'id="{section}"', landing)
        # what a visitor's browser loads: the page's own files, never another server's
        loads = re.findall(r'<(?:link(?![^>]*rel="canonical")|script|img|source|video)\b[^>]*?\s(?:href|src|poster)="([^"]+)"', landing)
        self.assertGreater(len(loads), 10)
        for url in loads:
            self.assertNotRegex(url, r"^(https?:)?//", f"the landing page loads {url}")
            self.assertTrue((self.out / url).exists(), url)
        for css in self.out.joinpath("assets").rglob("*.css"):
            self.assertNotRegex(read(css), r"url\(\s*[\"']?(https?:)?//", css)
        for font in ("instrument-sans-latin.woff2", "dm-mono-400-latin.woff2", "OFL-DMMono.txt"):
            self.assertTrue((self.out / "assets" / "fonts" / font).exists(), font)

    def test_nothing_is_copied_into_git(self):
        tracked = subprocess.run(
            ["git", "ls-files", "website"], cwd=ROOT, capture_output=True, text=True, check=True
        ).stdout.split()
        self.assertFalse([p for p in tracked if p.endswith(".md")], "no copied pages under website/")

    def test_links_between_pages_stay_on_the_site_and_source_links_go_to_the_tag(self):
        overview = self.page("overview")
        self.assertIn('href="../install/', overview)
        self.assertIn('src="../images/banner.png"', overview)
        recipes = self.page("recipes")
        self.assertIn(f'href="{GITHUB}/blob/v9.9.9/examples/project.lua"', recipes)
        ja = self.page("i18n/ja")
        self.assertIn('href="install/"', ja)  # the translation beside it
        self.assertIn('href="../../api/"', ja)  # no translation: the English page
        self.assertIn(f'href="{GITHUB}/blob/v9.9.9/CHANGELOG.md"', ja)

    def test_whats_new_has_a_page_per_release_newest_first(self):
        versions = [v for v, _, _ in hooks.releases(read(ROOT / "CHANGELOG.md"))]
        self.assertIn("0.1.0", versions)
        index = self.page("whats-new")
        at = [index.index(f'href="{v}/"') for v in versions]
        self.assertEqual(at, sorted(at), "newest first, as in CHANGELOG.md")
        self.assertIn("changelog-detail/", index)
        for v in versions:
            self.assertIn(f"leon {v}", self.page(f"whats-new/{v}"))
        self.assertIn("serve_tailscale", self.page("whats-new/0.1.1"))

    def test_a_changelog_anchor_goes_to_the_changelog(self):
        page = hooks.release_page("0.2.0", "2026-12-01", "See [it](#reading-api_version).\n\n[r]: #x")
        self.assertIn("[it](CHANGELOG.md#reading-api_version)", page)
        self.assertIn("[r]: CHANGELOG.md#x", page)

    def test_the_logo_goes_to_the_landing_page_under_the_base_path(self):
        from urllib.parse import urljoin

        base = "https://aindong.github.io/leon-cli/"
        logo = r'<a href="([^"]*)"[^>]*class="md-header__button md-logo"'
        for url in ["overview", "install", "cli/sessions", "i18n/ja"]:
            href = re.findall(logo, self.page(url))[0]
            self.assertEqual(urljoin(f"{base}{url}/", href), base, url)
        href = re.findall(logo, read(self.out / "404.html"))[0]
        self.assertEqual(urljoin(f"{base}missing/page/", href), base, "404")

    def test_the_demo_page_without_media_shows_a_poster_and_no_video(self):
        demo = self.page("demo")
        self.assertNotIn("<video", demo)
        self.assertRegex(demo, r'<img[^>]+src="\.\./images/demo-app\.gif"')

    def test_the_switcher_goes_to_the_page_s_own_translation(self):
        install = self.page("install")
        self.assertIn('href="../i18n/ja/install/" hreflang="ja"', install)
        api = self.page("api")
        self.assertIn('href="../i18n/ja/" hreflang="ja"', api)
        ja_install = self.page("i18n/ja/install")
        self.assertIn('href="../../../install/" hreflang="en"', ja_install)
        self.assertIn('href="../../overview/" hreflang="en"', self.page("i18n/ja"))

    def test_brand_search_and_no_generator_credit(self):
        for html in self.out.rglob("*.html"):
            text = read(html)
            self.assertFalse("Made with" in text or 'name="generator"' in text, html)
        self.assertTrue((self.out / "search" / "search_index.json").exists())
        css = read(self.out / "assets" / "leon.css")
        font = re.search(r"@font-face\s*{[^}]*url\(\"?([^\")]+)", css)
        self.assertTrue(font, "an @font-face")
        self.assertTrue((self.out / "assets" / font[1]).resolve().exists(), font[1])
        config = read(HERE / "mkdocs.yml")
        self.assertIn("scheme: default", config)
        self.assertIn("scheme: slate", config)


class LatestRelease(unittest.TestCase):
    def test_the_latest_release_s_checkout_builds_with_these_files(self):
        # site.yml builds the latest tag's docs with the branch's website/: a page the nav names
        # that the tag doesn't have yet must not fail the deploy
        tags = subprocess.run(
            ["git", "tag", "--list", "v*", "--sort=-v:refname"], cwd=ROOT, capture_output=True, text=True, check=True
        ).stdout.split()
        tags = [t for t in tags if re.fullmatch(r"v\d+\.\d+\.\d+", t)]
        if not tags:
            self.skipTest("no release tags in this clone")
        with tempfile.TemporaryDirectory() as d:
            archive = subprocess.run(["git", "archive", tags[0]], cwd=ROOT, capture_output=True, check=True).stdout
            subprocess.run(["tar", "-x", "-C", d, "-f", "-"], input=archive, check=True)
            env = dict(os.environ, LEON_SITE_ROOT=d, LEON_SITE_REF=tags[0])
            env.pop("LEON_SITE_MEDIA", None)
            r = subprocess.run(
                [str(Path(sys.executable).parent / "mkdocs"), "build", "--strict", "-f", str(HERE / "mkdocs.yml"), "-d", f"{d}/out"],
                env=env, capture_output=True, text=True,
            )
            self.assertEqual(r.returncode, 0, r.stderr)
            self.assertTrue(Path(d, "out", "whats-new", tags[0][1:], "index.html").exists())


class Media(unittest.TestCase):
    def test_videos_with_posters_and_odd_names_ignored(self):
        with tempfile.TemporaryDirectory() as d:
            media = Path(d) / "media"
            media.mkdir()
            for name in ["demo.mp4", "demo.jpg", "link-card.mp4", "Bad name.mp4", "x.exe", "<b>.mp4"]:
                (media / name).write_bytes(b"x")
            out = Path(d) / "site"
            build(out, media)
            demo = read(out / "demo" / "index.html")
            self.assertEqual(demo.count("<video"), 2, demo)
            self.assertIn('poster="../media/demo.jpg"', demo)
            self.assertIn('src="../media/link-card.mp4"', demo)
            self.assertNotIn("Bad name", demo)
            self.assertNotIn("&lt;b&gt;", demo)
            self.assertTrue((out / "media" / "demo.mp4").exists())
            self.assertFalse((out / "media" / "x.exe").exists())


class Links(unittest.TestCase):
    def rewrite(self, text, origin="docs/install.md", page="install.md"):
        staged = {"docs/install.md": "install.md", "README.md": "overview.md", "docs/cli.md": "cli.md"}
        return hooks.rewrite(text, origin, page, staged, root=ROOT, ref="v1.2.3", urls={})

    def test_source_links_go_to_the_tag_and_pages_stay(self):
        self.assertEqual(self.rewrite("[a](../crates/leon/Cargo.toml)"), f"[a]({GITHUB}/blob/v1.2.3/crates/leon/Cargo.toml)")
        self.assertEqual(self.rewrite("[a](../scripts/)"), f"[a]({GITHUB}/tree/v1.2.3/scripts/)")
        self.assertEqual(self.rewrite("[a](cli.md#x)"), "[a](cli.md#x)")
        self.assertEqual(self.rewrite("[a](../README.md)"), "[a](overview.md)")
        self.assertEqual(self.rewrite("[a](docs/cli.md)", "README.md", "overview.md"), "[a](cli.md)")

    def test_review_edge_cases(self):
        blob = f"{GITHUB}/blob/v1.2.3/crates/leon/Cargo.toml"
        # a stray backtick doesn't hide the next paragraph's link
        self.assertIn(blob, self.rewrite("Press ` to type one.\n\n[a](../crates/leon/Cargo.toml)\n\nThen `x`."))
        self.assertEqual(self.rewrite("[a][r]\n\n[r]: ../crates/leon/Cargo.toml"), f"[a][r]\n\n[r]: {blob}")
        self.assertEqual(self.rewrite("![logo](../brand/logo-mark.svg)"), f"![logo]({GITHUB}/raw/v1.2.3/brand/logo-mark.svg)")
        self.assertEqual(self.rewrite("[a\nb](../crates/leon/Cargo.toml)"), f"[a\nb]({blob})")
        self.assertEqual(self.rewrite("[`x`](cli.md#y)"), "[`x`](cli.md#y)")
        nested = "````md\n```lua\n[a](../crates/leon/Cargo.toml)\n```\n````\n[b](../crates/leon/Cargo.toml)"
        self.assertEqual(self.rewrite(nested), nested.rsplit("(", 1)[0] + f"({blob})")

    def test_missing_files_code_and_absolute_links_are_left_alone(self):
        for text in [
            "[a](nope.md)",
            "[a](https://example.com/x)",
            "[a](#here)",
            "`[a](../crates/leon/Cargo.toml)`",
            "```\n[a](../crates/leon/Cargo.toml)\n```",
            "   ~~~lua\n   t[1](../crates/leon/Cargo.toml)\n   ~~~",
        ]:
            self.assertEqual(self.rewrite(text), text)


class Guard(unittest.TestCase):
    def html(self, text):
        # the page's HTML as the build makes it, with the site's own Markdown extensions
        import markdown

        return markdown.markdown(text, extensions=["attr_list", "md_in_html", "tables", "pymdownx.superfences"])

    def test_scripts_in_a_page_stop_the_build(self):
        page = type("P", (), {"file": type("F", (), {"src_uri": "i18n/ja/install.md"})()})()
        for bad in [
            "<script>alert(1)</script>",
            '<img src="x" onerror="alert(1)">',
            "[x](javascript:alert(1))",
            "<iframe src=x></iframe>",
            '![x](data:,){onerror="alert(1)"}',  # attr_list puts the handler on the image
            "```\n<script>alert(1)</script>",  # a fence never closed is not code
            '<a href="java&#x73;cript:alert(1)">x</a>',  # a browser decodes the attribute first
            '<a href=" JaVa&#9;Script:alert(1)">x</a>',
        ]:
            with self.assertRaises(hooks.PluginError, msg=bad):
                hooks.on_page_content(self.html(f"# t\n\n{bad}\n"), page, None, None)
        # in code it is text, as the docs about leon's own web page show
        ok = "# t\n\n`<script>`\n\n```html\n<script src=x></script>\n<a onclick=x>\n```\n\n[javascript: a word](https://example.com/js)\n"
        hooks.on_page_content(self.html(ok), page, None, None)


class Notes(unittest.TestCase):
    def test_the_same_text_release_notes_sh_prints(self):
        changelog = (
            "# Changelog\n\nIntro.\n\n## Reading `api_version`\n\nText.\n\n## [Unreleased]\n\n- soon\n\n"
            "## [0.2.0] - 2026-12-01\n\n\nA release.\n\n### Added\n\n- **A thing.** It does `x`.\n\n\n"
            "## [0.1.0-rc.1] - 2026-11-01\n\n- first\n"
        )
        with tempfile.TemporaryDirectory() as d:
            path = Path(d) / "CHANGELOG.md"
            path.write_text(changelog)
            got = hooks.releases(changelog)
            self.assertEqual([(v, date) for v, date, _ in got], [("0.2.0", "2026-12-01"), ("0.1.0-rc.1", "2026-11-01")])
            self.assertEqual(hooks.releases("## [1.0.0]\n   \nText\n")[0][2], "   \nText", "a line of spaces is a line")
            for version, _, body in got:
                script = subprocess.run(
                    ["bash", str(ROOT / "scripts" / "release-notes.sh"), version, str(path)],
                    capture_output=True, text=True, check=True,
                ).stdout
                self.assertEqual(body + "\n", script, version)


if __name__ == "__main__":
    unittest.main()

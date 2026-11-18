# Translations

leon's README and its everyday guides are translated here, one folder per language. The English
pages are the reference: a translation may lag behind them, and when the two disagree, the English
page is right. Each translated page says so in its second line and links its English page.

## What is translated

| English page | Translated as |
|---|---|
| [README.md](../../README.md) | `docs/i18n/<lang>/README.md` |
| [docs/install.md](../install.md) | `docs/i18n/<lang>/install.md` |
| [docs/shortcuts.md](../shortcuts.md) | `docs/i18n/<lang>/shortcuts.md` |
| [docs/remote-hosts.md](../remote-hosts.md) | `docs/i18n/<lang>/remote-hosts.md` |
| [docs/agents.md](../agents.md) | `docs/i18n/<lang>/agents.md` |
| [docs/recipes.md](../recipes.md) | `docs/i18n/<lang>/recipes.md` |
| [docs/lua.md](../lua.md) | `docs/i18n/<lang>/lua.md` |
| [docs/cli.md](../cli.md) | `docs/i18n/<lang>/cli.md` |

Everything else stays in English: the API, the architecture, research, releases, changelogs, and
the pages under `docs/cli/` and `docs/lua/`. A translated page links the English page for those.

The languages so far, with their folder names: `zh-CN` (简体中文), `ja` (日本語), `ko` (한국어),
`es` (Español) and `pt-BR` (Português (Brasil)). A new language uses its
[BCP 47 tag](https://en.wikipedia.org/wiki/IETF_language_tag) as the folder name.

## Adding or updating a translation

1. Start from `dev`, and note the commit you translate from, its first 12 characters:
   `git rev-parse HEAD | cut -c1-12`.
2. Copy the English page to `docs/i18n/<lang>/<same file name>.md` (or, to update, compare the
   English page with the commit in the translation's header: `git diff <commit> -- docs/install.md`).
3. Make the first two lines the header, exactly this shape:

   ```
   <!-- i18n: source=docs/install.md commit=<the 12-character commit> -->
   > <in your language: "This is a translation. The English page is the reference; it may be newer.", with a link to the English page>
   ```

   The README's source is `README.md`. When you update a page, move the commit to the one you
   translated from.
4. Translate the prose. Keep these as they are, byte for byte:
   - code blocks and inline code: commands, flags, environment variables, file paths, key names
     such as ⌘K, and Lua. The code blocks stay in the same order as in English.
   - the name `leon` (always lower case) and `Leon.app`, and product names: Claude Code, Codex,
     Tailscale, Homebrew.
   - URLs, and anchors that other pages link to.
5. Links: link the translation when the page you link has one in the same language, and the English
   page otherwise (`../../install.md` from `docs/i18n/<lang>/`). Anchors (`#...`) into a translated
   page use its translated heading; into an English page, the English heading. Don't copy images:
   link the English ones, as in `../../images/banner.png`.
6. Only the README has the language switcher, one line near its top, the current language not
   linked: `English · 简体中文 · 日本語 · 한국어 · Español · Português (Brasil)`.
7. Run the checks, and open a pull request into `dev`:

   ```sh
   scripts/i18n-check.sh
   scripts/i18n-status.sh
   ```

`scripts/i18n-check.sh` checks every file here: the header is there and names an English page
that exists, every relative link and image resolves, and the code blocks match the English page's
at the header's commit. `scripts/i18n-status.sh` lists, for each language, the pages whose English
source changed since their header's commit (stale) and the ones not translated yet (missing).
A pull request that changes only translations starts no CI run, so run the check yourself.

### Tone

Plain, friendly and technical, the way developer docs in your language are usually written: for
example です/ます in Japanese, 합니다체 in Korean, concise Chinese without piling up 您, neutral
Spanish with tú, and Brazilian Portuguese with você. Keep the English meaning; don't add or drop
content. If a sentence in English is unclear, open an issue rather than guessing.

## Status

`scripts/i18n-status.sh --markdown` writes this table; it is a snapshot, so run the script for
today's. "stale (N)": the English page changed in N commits since the translation was made.

<!-- i18n-status: start -->
no translations yet
<!-- i18n-status: end -->

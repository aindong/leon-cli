#!/usr/bin/env bash
# website/build.sh [serve]: build leon's website into target/site, as .github/workflows/site.yml does.
# Installs the pinned, hashed website/requirements.txt into target/site-venv (nothing else of leon's
# build needs it), then builds strictly: a broken link or a missing page fails. `serve` previews it.
#   LEON_SITE_ROOT   the checkout the docs, README and changelog come from [default: this one]
#   LEON_SITE_REF    the tag or branch source links point at on GitHub [default: dev]
#   LEON_SITE_MEDIA  a folder of demo videos and posters (website/fetch-media.sh) [default: none]
set -euo pipefail
cd "$(dirname "$0")/.."
export NO_MKDOCS_2_WARNING=1 # the generator's notice about its own future, not about this site
venv=target/site-venv
if ! cmp -s website/requirements.txt "$venv/requirements.txt" 2>/dev/null; then
  rm -rf "${venv:?}"
  python3 -m venv "$venv"
  "$venv/bin/pip" install -q --disable-pip-version-check --require-hashes --no-deps -r website/requirements.txt
  cp website/requirements.txt "$venv/requirements.txt"
fi
case "${1:-}" in
  serve) exec "$venv/bin/mkdocs" serve -f website/mkdocs.yml ;;
  "") exec "$venv/bin/mkdocs" build --strict -f website/mkdocs.yml ;;
  *) echo "usage: website/build.sh [serve]" >&2; exit 2 ;;
esac

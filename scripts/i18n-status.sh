#!/usr/bin/env bash
# i18n-status.sh: how far each translation in docs/i18n/ is behind the English page.
#
#   scripts/i18n-status.sh [--markdown]
#
# For each language folder and each translated page: `ok` when the English source hasn't changed
# since the commit in the translation's header, `stale (N)` when N commits have changed it since,
# `missing` when there is no translation yet. --markdown prints it as the table in
# docs/i18n/README.md. A report, not a check: it exits 0 whatever it finds.
set -euo pipefail
cd "$(dirname "$0")/.."

PAGES="README.md docs/install.md docs/shortcuts.md docs/remote-hosts.md docs/agents.md docs/recipes.md docs/lua.md docs/cli.md"
markdown=""
case "${1:-}" in
    --markdown) markdown=1 ;;
    "") ;;
    *) echo "i18n-status.sh: unknown argument $1 (try --markdown)" >&2; exit 2 ;;
esac

langs=$(find docs/i18n -mindepth 1 -maxdepth 1 -type d -exec basename {} \; 2>/dev/null | sort || true)
if [ -z "$langs" ]; then
    echo "no translations yet"
    exit 0
fi

state() { # $1 the language, $2 the English page
    local f header re='^<!-- i18n: source=[^ ]+ commit=([0-9a-f]{12}) -->$' n
    f="docs/i18n/$1/$(basename "$2")"
    [ -f "$f" ] || { echo missing; return; }
    header=$(head -n 1 "$f")
    [[ $header =~ $re ]] || { echo "no header"; return; }
    n=$(git rev-list --count "${BASH_REMATCH[1]}..HEAD" -- "$2" 2>/dev/null) || { echo "unknown commit"; return; }
    if [ "$n" = 0 ]; then echo ok; else echo "stale ($n)"; fi
}

if [ -n "$markdown" ]; then
    printf '| Page |'
    for l in $langs; do printf ' %s |' "$l"; done
    printf '\n|---|'
    for _ in $langs; do printf -- '---|'; done
    printf '\n'
    for p in $PAGES; do
        printf '| %s |' "$(basename "$p")"
        for l in $langs; do printf ' %s |' "$(state "$l" "$p")"; done
        printf '\n'
    done
else
    for l in $langs; do
        for p in $PAGES; do printf '%-6s %-16s %s\n' "$l" "$(basename "$p")" "$(state "$l" "$p")"; done
    done
fi

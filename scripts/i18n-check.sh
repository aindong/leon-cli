#!/usr/bin/env bash
# i18n-check.sh: check the translations in docs/i18n/ (the conventions are in docs/i18n/README.md).
#
#   scripts/i18n-check.sh
#
# Each file in a language folder (docs/i18n/<lang>/) starts with the i18n header, which names an
# English source that exists and has the same file name, and a commit this clone has; its code
# blocks are the source's at that commit, byte for byte and in the same order. Every relative link
# and image in docs/i18n/ points at a file that exists. Only Markdown lives there: images are
# linked from docs/images/, not copied. Prints each problem and exits 1 if there is any. Needs the
# git history the headers name (a full clone, not a shallow one).
set -euo pipefail
cd "$(dirname "$0")/.."

bad=0
fail() {
    echo "$1: $2" >&2
    bad=1
}

# The fenced code blocks of a Markdown file on stdin, fences included.
blocks() {
    awk '/^[[:space:]]*```/ { print; inside = !inside; next } inside { print }'
}

# The relative link and image targets of a Markdown file on stdin, outside code blocks, without
# their #anchor. Deliberately simple, as in pub_doc_links.rs: no link in these docs holds a `)`.
targets() {
    awk '/^[[:space:]]*```/ { inside = !inside; next } !inside { print }' |
        grep -oE '\]\([^)]*\)|src="[^"]*"' |
        sed -E 's/^\]\(//; s/\)$//; s/^src="//; s/"$//; s/#.*//; s/[[:space:]]+"[^"]*"$//' |
        grep -vE '^$|^[a-z]+:' || true
}

[ -d docs/i18n ] || exit 0
count=0
while IFS= read -r f; do
    count=$((count + 1))
    dir=$(dirname "$f")
    while IFS= read -r t; do
        [ -e "$dir/$t" ] || fail "$f" "links to $t, which doesn't exist"
    done < <(targets <"$f")

    [ "$dir" != docs/i18n ] || continue # the folder's own README is not a translation
    header=$(head -n 1 "$f")
    re='^<!-- i18n: source=([^ ]+) commit=([0-9a-f]{12}) -->$'
    if ! [[ $header =~ $re ]]; then
        fail "$f" "line 1 must be <!-- i18n: source=<path> commit=<12-char commit> -->"
        continue
    fi
    source=${BASH_REMATCH[1]} commit=${BASH_REMATCH[2]}
    note=$(sed -n 2p "$f")
    [[ $note == '> '*']('* ]] ||
        fail "$f" "line 2 must be a '> ' note in the page's language with a link to the English page"
    case $source in docs/i18n/*)
        fail "$f" "its source must be the English page, not $source"
        continue ;;
    esac
    if [ ! -f "$source" ]; then
        fail "$f" "its source $source doesn't exist"
        continue
    fi
    [ "$(basename "$source")" = "$(basename "$f")" ] || fail "$f" "is named after $(basename "$source")"
    if ! git cat-file -e "$commit:$source" 2>/dev/null; then
        fail "$f" "commit $commit has no $source in this clone (a shallow clone? fetch the history)"
        continue
    fi
    if ! diff -q <(git show "$commit:$source" | blocks) <(blocks <"$f") >/dev/null; then
        fail "$f" "its code blocks differ from $source at $commit (they stay byte for byte the English ones):"
        diff <(git show "$commit:$source" | blocks) <(blocks <"$f") | head -n 6 >&2 || true
    fi
done < <(find docs/i18n -name '*.md' | sort)

while IFS= read -r f; do
    fail "$f" "only Markdown lives in docs/i18n/: link the English images in docs/images/ instead"
done < <(find docs/i18n -type f ! -name '*.md' | sort)

[ "$bad" = 0 ] || exit 1
echo "i18n-check: $count files ok"

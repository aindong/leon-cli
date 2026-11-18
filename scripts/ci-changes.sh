#!/usr/bin/env bash
# ci-changes.sh: which CI jobs the files changed since BASE need (ci.yml's first job runs it).
#
#   scripts/ci-changes.sh BASE
#
# Prints one word:
#   none  only translations, images, brand files and community files (ci.yml's paths-ignore
#         usually stops the run before this job starts)
#   docs  English Markdown outside the code too: the doc tests, on Linux
#   full  anything else, and whenever the diff can't be read: the whole gate
set -euo pipefail
set -f # the patterns below are for `case`, not for the shell to expand

# The same list as ci.yml's paths-ignore, in the same order (crates/leon/tests/ci_paths.rs checks).
SKIP="docs/i18n/** docs/images/** brand/** CONTRIBUTING.md CODE_OF_CONDUCT.md SECURITY.md .github/ISSUE_TEMPLATE/** .github/pull_request_template.md"

files=$(git -c core.quotepath=off diff --name-only "${1:-}" HEAD 2>/dev/null) && [ -n "$files" ] || {
  echo full
  exit 0
}
kind=none
while IFS= read -r f; do
  for p in $SKIP; do
    case $f in $p) continue 2 ;; esac
  done
  case $f in
    crates/* | scripts/* | .github/* | examples/*) echo full; exit 0 ;;
    *.md) kind=docs ;;
    *) echo full; exit 0 ;;
  esac
done <<<"$files"
echo "$kind"

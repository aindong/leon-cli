#!/usr/bin/env bash
# The tests that read the docs, in the pinned Rust image (scripts/linux-image.sh): what CI runs for a
# change to English docs alone (.github/workflows/ci.yml). A test reads the docs when a line of its
# code, not a comment, names docs/, README.md, CHANGELOG.md or CLAUDE.md, so a new doc test is
# picked up with no list to keep. The app's tests are macOS only and not among them. Needs Docker.
#   scripts/linux-doc-tests.sh [--list] [--arch arm64|amd64] [TEST-HARNESS-ARGS…]
# --list prints the test files and runs nothing.
set -euo pipefail
cd "$(dirname "$0")/.."

tests() {
    for f in crates/*/tests/*.rs; do
        case $f in crates/leon-app/*) continue ;; esac
        if grep -v '^[[:space:]]*//' "$f" | grep -qE 'docs/|README\.md|CHANGELOG\.md|CLAUDE\.md'; then
            echo "$f"
        fi
    done
}
if [ "${1:-}" = --list ]; then
    tests
    exit 0
fi

. scripts/linux-image.sh
cargo_args=()
for f in $(tests); do
    crate=${f#crates/}
    cargo_args+=(-p "${crate%%/*}" --test "$(basename "$f" .rs)")
done
PACKAGES="zsh lsof minisign jq" # the linux job's (scripts/linux-test.sh): the release doc tests sign
in_image bash -c 'cargo test --locked --no-fail-fast "$@"' bash "${cargo_args[@]}" -- ${ARGS[@]+"${ARGS[@]}"}

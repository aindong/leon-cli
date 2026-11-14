#!/usr/bin/env bash
# Build, lint and test leon's server and CLI on Linux (everything but the GPUI app), then run the
# smoke test, inside the pinned Rust image (scripts/linux-image.sh). Needs Docker.
#   scripts/linux-test.sh [--arch arm64|amd64]
set -euo pipefail
cd "$(dirname "$0")/.."
. scripts/linux-image.sh

# A test file gated to macOS must say why on the line above the gate.
bad=$(grep -rnE -B1 --include='*.rs' '^#!?\[cfg\(.*target_os = "macos"' crates/*/tests crates/*/benches 2>/dev/null |
    awk '/\.rs-[0-9]+-/ { prev = $0; next } /\.rs:[0-9]+:/ && !/cfg\(not\(/ { if (prev !~ /macOS only: /) print; prev = "" }')
if [ -n "$bad" ]; then
    echo "macOS-only test gates without a '// macOS only: <reason>' line above them:" >&2
    echo "$bad" >&2
    exit 1
fi
grep -rn --include='*.rs' '// macOS only: ' crates/*/tests crates/*/benches | sed 's|:[0-9]*:// macOS only: |: |; s/^/skipped on Linux: /' || true

PACKAGES="zsh"
in_image bash -c '
    set -euo pipefail
    cargo clippy --locked --workspace --exclude leon-app --all-targets -- -D warnings
    cargo test --locked --no-fail-fast --workspace --exclude leon-app
    sh scripts/linux-smoke.sh /target/debug/leon
'

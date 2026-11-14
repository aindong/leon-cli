#!/usr/bin/env bash
# Build, lint and test leon's server and CLI on Linux (everything but the GPUI app), then run the
# smoke test, inside the pinned Rust image (scripts/linux-image.sh). Needs Docker.
#   scripts/linux-test.sh [--arch arm64|amd64] [TEST-HARNESS-ARGS…]
# Under Rosetta (amd64 on an Apple Silicon Mac) children inherit stray fds, so tests that spawn
# from several threads at once can deadlock on each other's pipes: pass --test-threads=1 there.
set -euo pipefail
cd "$(dirname "$0")/.."
. scripts/linux-image.sh

# Test code gated to macOS must say why on the line above the gate: any gate in tests/ and benches/,
# and in src/ a gate on a `#[test]` or a test module (other src/ gates are platform code with a
# Linux branch). shortcut: a gate below `#[test]` in src/ isn't seen; a Rust test would see it.
bad=$(find crates/*/src crates/*/tests crates/*/benches -name '*.rs' -exec awk '
    FNR == 1 { prev = ""; pending = "" }
    pending != "" {
        if (FILENAME !~ /\/src\// || $0 ~ /^[[:space:]]*#\[([a-z_]+::)?test\]/ || $0 ~ /^[[:space:]]*(pub )?mod [a-z_]*tests/)
            print pending
        pending = ""
    }
    /^[[:space:]]*#!?\[cfg\(/ && /target_os = "macos"/ && !/cfg\(not\(/ && prev !~ /macOS only: / {
        pending = FILENAME ":" FNR ": " $0
    }
    { prev = $0 }' {} +)
if [ -n "$bad" ]; then
    echo "macOS-only test gates without a '// macOS only: <reason>' line above them:" >&2
    echo "$bad" >&2
    exit 1
fi
grep -rn --include='*.rs' '// macOS only: ' crates/*/src crates/*/tests crates/*/benches |
    sed 's|:[0-9]*:[[:space:]]*// macOS only: |: |; s/^/skipped on Linux: /' || true

PACKAGES="zsh"
in_image bash -c '
    set -euo pipefail
    cargo clippy --locked --workspace --exclude leon-app --all-targets -- -D warnings
    cargo test --locked --no-fail-fast --workspace --exclude leon-app -- "$@"
    sh scripts/linux-smoke.sh /target/debug/leon
' bash ${ARGS[@]+"${ARGS[@]}"}

#!/usr/bin/env bash
# leon-term.sh PROGRAM: leon as a plain terminal for external.sh: a private server whose only
# session runs PROGRAM, then `exec leon app`, so this pid owns the window. The server's pid goes
# in $PERF_DIR/extra-pids (external.sh measures and stops it too).
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
leon="$root/target/release/leon"
"$leon" server start
"$leon" --json server status | python3 -c 'import json,sys; print(json.load(sys.stdin)["pid"])' >>"$PERF_DIR/extra-pids"
sid=$("$leon" new term)
first=$("$leon" --json block ls -s "$sid" | python3 -c 'import json,sys; print(json.load(sys.stdin)[0]["block_id"])')
"$leon" split -s "$sid" -b "$first" -- "$1" >/dev/null
"$leon" block close "$first" >/dev/null
exec "$leon" app

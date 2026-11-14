#!/bin/sh
# The headless server and CLI end to end: start, a session, a program, its screen, its cwd, stop;
# then the same socket found through XDG_RUNTIME_DIR. Private temp dirs, never your own server.
#   scripts/linux-smoke.sh [LEON_BINARY]      (scripts/linux-test.sh runs it on Linux)
set -eu
leon=${1:-leon}
tmp=$(mktemp -d)
# stop whatever a failed step left running; nothing here may change the script's status
cleanup() {
    LEON_RUNTIME_DIR=$tmp/run "$leon" server stop >/dev/null 2>&1 || true
    XDG_RUNTIME_DIR=$tmp/xdg "$leon" server stop >/dev/null 2>&1 || true
    rm -rf "$tmp" || true
}
trap cleanup EXIT
export LEON_RUNTIME_DIR="$tmp/run" LEON_STATE_DIR="$tmp/state" LEON_CONFIG="$tmp/no-init.lua" SHELL=/bin/sh
unset LEON_SESSION LEON_BLOCK LEON_AUTOSTART

fail() { echo "FAIL: $*" >&2; exit 1; }
# step NAME CMD…: run it, keep its stdout in $out, stop naming the step if it fails
step() {
    name=$1
    shift
    out=$("$@" 2>"$tmp/err") || fail "$name: exit $?: $(cat "$tmp/err")"
    echo "ok: $name"
}

step "leon server start" "$leon" server start
[ -S "$LEON_RUNTIME_DIR/server.sock" ] || fail "no socket at $LEON_RUNTIME_DIR/server.sock"
step "leon new" "$leon" new smoke
sid=$out
step "leon run -- echo hi" "$leon" -s "$sid" run --keep-open -- echo hi
id=$out
step "leon wait" "$leon" -s "$sid" wait --for 10s "$id"
step "leon capture" "$leon" -s "$sid" capture -b "$id" --trim
[ "$out" = hi ] || fail "leon capture printed '$out', not 'hi'"

# the shell's own cwd (/proc/<pid>/cwd on Linux), not where it started or an OSC 7 report
step "leon run -- sh (cd /usr)" "$leon" -s "$sid" run --cwd / -- sh -c 'cd /usr && sleep 30'
sh_id=$out
i=0
until "$leon" -s "$sid" block call -b "$sh_id" pwd 2>/dev/null | grep -q '"/usr"'; do
    i=$((i + 1))
    [ $i -lt 100 ] || fail "block pwd never became /usr: $("$leon" -s "$sid" block call -b "$sh_id" pwd 2>&1)"
    sleep 0.1
done
echo "ok: block pwd follows the shell's cwd"

step "leon server stop" "$leon" server stop
[ ! -e "$LEON_RUNTIME_DIR/server.sock" ] || fail "the socket is still there after server stop"

# Without LEON_RUNTIME_DIR the socket is under $XDG_RUNTIME_DIR/leon.
unset LEON_RUNTIME_DIR
mkdir -m 700 "$tmp/xdg"
export XDG_RUNTIME_DIR="$tmp/xdg"
step "leon server start (XDG_RUNTIME_DIR)" "$leon" server start
[ -S "$XDG_RUNTIME_DIR/leon/server.sock" ] || fail "no socket at $XDG_RUNTIME_DIR/leon/server.sock"
step "leon ls (XDG_RUNTIME_DIR)" "$leon" ls
step "leon server stop (XDG_RUNTIME_DIR)" "$leon" server stop
echo "smoke: all steps passed"

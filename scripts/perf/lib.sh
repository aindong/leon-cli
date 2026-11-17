# shellcheck shell=bash
# scripts/perf/lib.sh: what every perf script shares. Source it, then call perf_init NAME.
#
# Every leon it starts uses the release build and a private 0700 temp dir for LEON_RUNTIME_DIR,
# LEON_STATE_DIR and LEON_CONFIG (a missing file, or PERF_INIT_LUA's text), with a temp HOME and
# SHELL=/bin/sh whose prompt is the sentinel bar, so no real session, config or shell rc is read
# or touched. Cleanup stops that server and kills only the pids this script started.
set -euo pipefail

PERF_ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
LEON="$PERF_ROOT/target/release/leon"
WINFRAMES="$PERF_ROOT/target/perf/winframes"
NOW="$PERF_ROOT/target/perf/now"
PERF_PY=$(command -v python3)
PERF_PIDS=()

load1() { sysctl -n vm.loadavg | awk '{print $2}'; }

# Wait (up to 10 minutes) for the 1-minute load to fall under 3 before measuring: other builds
# make timing worthless, and each measurement's own work raises the load for a minute after it.
# PERF_ALLOW_LOAD=1 runs anyway (a dry run: say so beside the numbers and throw them away).
perf_load_gate() {
  local l i
  for i in $(seq 120); do
    l=$(load1)
    awk -v l="$l" 'BEGIN { exit !(l < 3) }' && return 0
    [ "${PERF_ALLOW_LOAD:-0}" = 1 ] && { echo "# load $l >= 3: DRY RUN, these numbers don't count" >&2; return 0; }
    sleep 5
  done
  echo "perf: the 1-minute load stayed at 3 or more for 10 minutes ($l): not measuring" >&2
  ps -Ao pcpu,comm -r | head -8 >&2
  exit 3
}

# A locked screen marks every window occluded and GPUI stops drawing.
perf_screen_gate() {
  local locked
  locked=$(swift - 2>/dev/null <<'SWIFT' || echo 0
import CoreGraphics
let d = CGSessionCopyCurrentDictionary() as? [String: Any] ?? [:]
print(d["CGSSessionScreenIsLocked"] as? Int ?? 0)
SWIFT
)
  if [ "$locked" = 1 ]; then
    echo "perf: the screen is locked; nothing draws (exit 2)" >&2
    exit 2
  fi
}

perf_build() {
  (cd "$PERF_ROOT" && cargo build --release -q -p leon)
  if [ ! -x "$WINFRAMES" ] || [ "$PERF_ROOT/scripts/perf/winframes.swift" -nt "$WINFRAMES" ]; then
    mkdir -p "$(dirname "$WINFRAMES")"
    swiftc -O -o "$WINFRAMES" "$PERF_ROOT/scripts/perf/winframes.swift"
  fi
  if [ ! -x "$NOW" ] || [ "$PERF_ROOT/scripts/perf/now.c" -nt "$NOW" ]; then
    cc -O2 -o "$NOW" "$PERF_ROOT/scripts/perf/now.c"
  fi
}

# perf_init NAME: build, make the private dirs, and open $PERF_OUT/NAME-<time> for this run's files.
perf_init() {
  perf_build
  PERF_DIR=$(mktemp -d)
  chmod 700 "$PERF_DIR"
  mkdir -m 700 "$PERF_DIR/run" "$PERF_DIR/state" "$PERF_DIR/home"
  export LEON_RUNTIME_DIR="$PERF_DIR/run" LEON_STATE_DIR="$PERF_DIR/state" LEON_CONFIG="$PERF_DIR/no-init.lua"
  [ -z "${PERF_INIT_LUA:-}" ] || printf '%s\n' "$PERF_INIT_LUA" >"$LEON_CONFIG"
  export HOME="$PERF_DIR/home" SHELL=/bin/sh ENV="$PERF_DIR/home/.shrc"
  # the prompt: a bar in the sentinel colour, then "$ " (what start-up waits to see drawn)
  cat >"$ENV" <<'SH'
PS1="$(printf '\033[48;2;255;0;128m%40s\033[0m$ ' '')"
SH
  cp "$ENV" "$HOME/.profile"
  RUN="${PERF_OUT:-$PERF_ROOT/.agent/perf}/$1-$(date +%Y%m%d-%H%M%S)"
  mkdir -p "$RUN"
  trap perf_cleanup EXIT
  perf_note "start"
}

perf_cleanup() {
  local p
  # only pids still running as this shell's jobs: one already waited for may be reused by now
  for p in ${PERF_PIDS[@]+"${PERF_PIDS[@]}"}; do
    jobs -p | grep -qx "$p" && kill "$p" 2>/dev/null || true
  done
  "$LEON" --autostart=false server stop >/dev/null 2>&1 || true
  for p in ${PERF_PIDS[@]+"${PERF_PIDS[@]}"}; do
    wait "$p" 2>/dev/null || true
  done
  rm -rf "${PERF_DIR:?}"
}

# perf_stop PID: stop a process this script started, and forget it.
perf_stop() {
  kill "$1" 2>/dev/null || true
  wait "$1" 2>/dev/null || true
  local keep=() p
  for p in ${PERF_PIDS[@]+"${PERF_PIDS[@]}"}; do [ "$p" = "$1" ] || keep+=("$p"); done
  PERF_PIDS=(${keep[@]+"${keep[@]}"})
}

# perf_note WHAT: one line in the run's notes with the load, the thermal state and the power source.
perf_note() {
  local therm power
  therm=$(pmset -g therm 2>/dev/null | awk -F= '/CPU_Speed_Limit/ {print $2+0}')
  power=$(pmset -g batt 2>/dev/null | head -1 | sed "s/.*'\(.*\)'.*/\1/")
  echo "$(date +%H:%M:%S) $1 load=$(load1) cpu_limit=${therm:-none} power=$power" >>"$RUN/notes.txt"
}

perf_server() {
  "$LEON" server start --restore=false
  SERVER_PID=$("$LEON" --json server status | "$PERF_PY" -c 'import json,sys; print(json.load(sys.stdin)["pid"])')
}

# perf_app LOG: start the app with its frame log at LOG; sets APP_PID and waits for its window.
perf_app() {
  LEON_FRAME_LOG="$1" "$LEON" app 2>>"$RUN/app-stderr.txt" &
  APP_PID=$!
  PERF_PIDS+=("$APP_PID")
  local i
  for i in $(seq 100); do
    grep -q '^frame ' "$1" 2>/dev/null && return 0
    sleep 0.1
  done
  echo "perf: the app drew no frame in 10 s" >&2
  exit 1
}

# The boot clock in ns. A stamp costs about 1.5 ms of process start, inside any span it ends.
uptime_ns() { "$NOW"; }

# mem PID: RSS and phys_footprint in KiB ("rss footprint").
mem() {
  local rss fp
  rss=$(ps -o rss= -p "$1" | tr -d ' ')
  fp=$(footprint -p "$1" -f bytes --noCategories 2>/dev/null | awk '/phys_footprint:/ {print int($2 / 1024); exit}')
  echo "${rss:-0} ${fp:-0}"
}

# cpu_s PID: CPU time used so far, in seconds.
cpu_s() { ps -o time= -p "$1" | awk -F'[:.]' '{ if (NF == 3) print $1 * 60 + $2 "." $3; else print $1 * 3600 + $2 * 60 + $3 "." $4 }'; }

# first_block SID: the block a new session starts with.
first_block() {
  "$LEON" --json block ls -s "$1" | "$PERF_PY" -c 'import json,sys; print(json.load(sys.stdin)[0]["block_id"])'
}

# stack SID ANCHOR N CMD [DIR]: N panes of the same size where block ANCHOR is (below each other,
# or DIR=right: side by side), each running `sh -c CMD` without taking the focus; then ANCHOR
# closes. Pane k takes (N-k+1)/(N-k+2) of the one before it, so each ends with 1/N. Prints the
# new blocks' IDs, and appends each split's time in ms to $RUN/splits.txt.
stack() {
  local sid=$1 at=$2 n=$3 cmd=$4 dir=${5:-below} k prev=$2 ratio t0
  for k in $(seq "$n"); do
    ratio=50
    [ "$k" = 1 ] || ratio=$(((100 * (n - k + 1) + (n - k + 2) / 2) / (n - k + 2)))
    [ "$ratio" -le 99 ] || ratio=99 # `leon split` takes 1 to 99 (from 200 panes on)
    t0=$(uptime_ns)
    prev=$("$LEON" split -s "$sid" -b "$prev" --split "$dir" --ratio "$ratio" --focus=false -- sh -c "$cmd")
    [ -n "$prev" ] || { echo "perf: split $k of $n failed" >&2; return 1; }
    echo "$(( ($(uptime_ns) - t0) / 1000000 ))" >>"$RUN/splits.txt"
    echo "$prev"
  done
  "$LEON" block close "$at" >/dev/null
}

# shellcheck shell=bash
# scripts/perf/lib.sh: what every perf script shares. Source it, then call perf_init NAME.
#
# Every leon it starts uses the release build and a private 0700 temp dir for LEON_RUNTIME_DIR,
# LEON_STATE_DIR and LEON_CONFIG (a missing file: no init.lua runs), with a temp HOME and
# SHELL=/bin/sh whose prompt is the sentinel bar, so no real session, config or shell rc is read
# or touched. Cleanup stops that server and kills only the pids this script started.
set -euo pipefail

PERF_ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
LEON="$PERF_ROOT/target/release/leon"
WINFRAMES="$PERF_ROOT/target/perf/winframes"
PERF_PY=$(command -v python3)
PERF_PIDS=()

load1() { sysctl -n vm.loadavg | awk '{print $2}'; }

# Refuse to measure on a busy machine: other builds make timing worthless. PERF_ALLOW_LOAD=1 runs
# anyway (a dry run: say so beside the numbers and throw them away).
perf_load_gate() {
  local l
  l=$(load1)
  if awk -v l="$l" 'BEGIN { exit !(l >= 3) }'; then
    if [ "${PERF_ALLOW_LOAD:-0}" = 1 ]; then
      echo "# load $l >= 3: DRY RUN, these numbers don't count" >&2
    else
      echo "perf: the 1-minute load is $l (>= 3): not measuring; PERF_ALLOW_LOAD=1 for a dry run" >&2
      exit 3
    fi
  fi
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
}

# perf_init NAME: build, make the private dirs, and open $PERF_OUT/NAME-<time> for this run's files.
perf_init() {
  perf_build
  PERF_DIR=$(mktemp -d)
  chmod 700 "$PERF_DIR"
  mkdir -m 700 "$PERF_DIR/run" "$PERF_DIR/state" "$PERF_DIR/home"
  export LEON_RUNTIME_DIR="$PERF_DIR/run" LEON_STATE_DIR="$PERF_DIR/state" LEON_CONFIG="$PERF_DIR/no-init.lua"
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
  for p in "${PERF_PIDS[@]}"; do
    kill "$p" 2>/dev/null || true
  done
  "$LEON" --autostart=false server stop >/dev/null 2>&1 || true
  for p in "${PERF_PIDS[@]}"; do
    wait "$p" 2>/dev/null || true
  done
  rm -rf "${PERF_DIR:?}"
}

# perf_note WHAT: one line in the run's notes with the load, the thermal state and the power source.
perf_note() {
  local therm power
  therm=$(pmset -g therm 2>/dev/null | awk -F= '/CPU_Speed_Limit/ {print $2+0}')
  power=$(pmset -g batt 2>/dev/null | head -1 | sed "s/.*'\(.*\)'.*/\1/")
  echo "$(date +%H:%M:%S) $1 load=$(load1) cpu_limit=${therm:-none} power=$power" >>"$RUN/notes.txt"
}

perf_server() {
  "$LEON" server start
  SERVER_PID=$("$LEON" --json server status | "$PERF_PY" -c 'import json,sys; print(json.load(sys.stdin)["pid"])')
}

# perf_app LOG: start the app with its frame log at LOG; sets APP_PID and waits for its window.
perf_app() {
  LEON_FRAME_LOG="$1" "$LEON" app 2>"$RUN/app-stderr.txt" &
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

uptime_ns() { "$PERF_PY" -c 'import time; print(time.clock_gettime_ns(time.CLOCK_UPTIME_RAW))'; }

# mem PID: RSS and phys_footprint in KiB ("rss footprint").
mem() {
  local rss fp
  rss=$(ps -o rss= -p "$1" | tr -d ' ')
  fp=$(footprint -p "$1" -f bytes --noCategories 2>/dev/null | awk '/phys_footprint:/ {print int($2 / 1024); exit}')
  echo "${rss:-0} ${fp:-0}"
}

# cpu_s PID: CPU time used so far, in seconds.
cpu_s() { ps -o time= -p "$1" | awk -F'[:.]' '{ if (NF == 3) print $1 * 60 + $2 "." $3; else print $1 * 3600 + $2 * 60 + $3 "." $4 }'; }

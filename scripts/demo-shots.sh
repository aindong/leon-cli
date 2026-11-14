#!/usr/bin/env bash
# demo-shots.sh [OUT_DIR]: retake every screenshot in docs/images from a neutral demo, never the
# machine it runs on: a temp HOME whose zsh prompt is `auth %`, scripted programs instead of real
# agents, private LEON_RUNTIME_DIR/STATE_DIR/CONFIG, and a devbox that is a second local server
# behind a fake ssh. Keys go to leon's window through System Events (the terminal running this needs
# Accessibility); shots are window-only (scripts/winshot.swift). Run it on an unlocked screen and
# leave the mouse and keyboard alone for the minute it takes. Check the result with
# `cargo test -p leon --test m13_privacy`.
set -euo pipefail
cd "$(dirname "$0")/.."
repo=$(pwd)
out=${1:-$repo/docs/images}

locked=$(swift - 2>/dev/null <<'SWIFT' || echo 0
import CoreGraphics
let d = CGSessionCopyCurrentDictionary() as? [String: Any] ?? [:]
print(d["CGSSessionScreenIsLocked"] as? Int ?? 0)
SWIFT
)
if [ "$locked" = 1 ]; then
  echo "demo-shots: the screen is locked; a locked screen draws nothing (exit 2)"
  exit 2
fi
cargo build --release -q -p leon
leon=$repo/target/release/leon

d=$(cd "$(mktemp -d)" && pwd -P) # the real path, so the app writes HOME as ~
chmod 700 "$d"
mkdir -m 700 -p "$d/run" "$d/state" "$d/dev/run" "$d/dev/state"
home=$d/home
mkdir -p "$home/bin" "$home/src/auth" "$home/src/site" "$home/src/infra" "$home/src/api" "$home/src/deploy"
# a neutral shell: no user or host in the prompt or the title
cat >"$home/.zshrc" <<'EOF'
PROMPT='%1~ %% '
unsetopt PROMPT_SP
EOF
cat >"$d/init.lua" <<'EOF'
leon.action{ name = "tail_log", title = "Tail the server log", run = function() end }
leon.bind("cmd+shift+l", "tail_log")
leon.bind("ctrl+b>c", "client.tab.new")
leon.bind("ctrl+b>%", "pane.split.right")
leon.bind("ctrl+b>\"", "pane.split.down")
leon.bind("ctrl+b>z", "pane.zoom")
EOF
: >"$d/none.lua"

# show TITLE TEXTFILE [OSC7501 PAIRS]: a "program" that sets its title and status and shows the
# text, again after each resize (the app sizes the pane after it started)
cat >"$home/bin/show" <<'EOF'
#!/bin/sh
printf '\033]2;%s\007' "$1"
[ -n "${3:-}" ] && printf '\033]7501;%s\007' "$3"
text=$2
draw() { printf '\033[H\033[2J\033[3J'; cat "$text"; }
trap draw WINCH
draw
while :; do sleep 100000 & wait $!; done
EOF
chmod +x "$home/bin/show"
b64() { printf %s "$1" | base64; }
txt() { cat >"$d/$1.txt"; }

txt claude <<'EOF'
> Fix the flaky login test in auth/session_test.rs

  I'll read the test first, then run it a few times to see
  how it fails.

  Read auth/session_test.rs (212 lines)
  The test sleeps 50 ms and then checks the session expired.
  On a busy machine the check runs before the timer fires.

  Update auth/session_test.rs
   - std::thread::sleep(Duration::from_millis(50));
   + clock.advance(Duration::from_secs(61));

  Running the tests beside me:
    leon split --label tests -- cargo test -p auth
EOF
txt tests <<'EOF'
   Compiling auth v0.4.0 (/src/auth)
    Finished `test` profile in 4.21s
     Running unittests src/lib.rs

running 18 tests
test session::tests::refresh_keeps_the_id ... ok
test session::tests::expired_session_is_rejected ... ok
test session::tests::login_sets_a_cookie ... ok
test token::tests::round_trip ... ok
test token::tests::rejects_a_bad_signature ... ok
EOF
txt server <<'EOF'
listening on http://127.0.0.1:8080
GET /login 200 3ms
POST /session 201 12ms
GET /me 200 2ms
EOF
txt readme <<'EOF'
> Review the README for the 0.4 release

  Checked every command in README.md against --help: all current.
  Fixed two broken links in the install section.
EOF
txt migrate <<'EOF'
> Migrate the users table to the new schema

  Wrote migrations/0042_users_email_unique.sql
  Dry run on staging: 18,204 rows, 0 conflicts.

  Run it on production now?
EOF
txt api <<'EOF'
> Add rate limiting to the public API

  Plan: a token bucket per API key in src/limits.rs, 100 requests
  a minute, with Retry-After on a 429.

  Edit src/routes.rs to add the middleware?
EOF
txt deploy <<'EOF'
$ ./deploy.sh api v2.3.1
rolling out api v2.3.1 to 4 pods
pod api-7f9c-1 ready
pod api-7f9c-2 CrashLoopBackOff
rollout failed: 2 of 4 pods crashed
EOF
txt build <<'EOF'
$ make release
[ 40%] Building CXX object src/CMakeFiles/core.dir/engine.cpp.o
EOF

base=(env -u LEON_SESSION -u LEON_BLOCK -u ZDOTDIR HOME="$home" SHELL=/bin/zsh PATH="$home/bin:$PATH")
here() { "${base[@]}" LEON_RUNTIME_DIR="$d/run" LEON_STATE_DIR="$d/state" LEON_CONFIG="$d/init.lua" "$leon" "$@"; }
dev() { "${base[@]}" LEON_RUNTIME_DIR="$d/dev/run" LEON_STATE_DIR="$d/dev/state" LEON_CONFIG="$d/none.lua" "$leon" "$@"; }

# the devbox: ssh's argv ends with HOST CMD; this one serves the second server, and build-box fails
cat >"$d/ssh" <<EOF
#!/bin/sh
for a; do host=\$prev; prev=\$a; done
case "\$host" in
  *build-box*) echo "ssh: connect to host build-box port 22: Connection refused" >&2; exit 255 ;;
esac
exec env -u LEON_SESSION -u LEON_BLOCK HOME="$home" SHELL=/bin/zsh LEON_RUNTIME_DIR="$d/dev/run" \
  LEON_STATE_DIR="$d/dev/state" LEON_CONFIG="$d/none.lua" "$leon" server proxy
EOF
chmod +x "$d/ssh"

app_pid=
cleanup() {
  [ -n "$app_pid" ] && kill "$app_pid" 2>/dev/null || true
  here --autostart=false server stop --discard >/dev/null 2>&1 || true
  dev --autostart=false server stop --discard >/dev/null 2>&1 || true
  rm -rf "$d"
}
trap cleanup EXIT

# a tab (window) in SESSION running `show` in DIR; prints its block id
tab() { here -s "$1" run --keep-open -c "$home/src/$2" -- show "$3" "$d/$4.txt" "${5:-}"; }

here config check "$d/init.lua" >/dev/null # a broken demo config would quietly drop the palette's action
here server start >/dev/null
here new auth >/dev/null
claude=$(here -s auth run --keep-open -c "$home/src/auth" -- show "Fix the flaky login test" "$d/claude.txt" \
  "state=working:progress=60:title=$(b64 Claude):msg=$(b64 'running the tests')")
here -s auth window close 1 >/dev/null
tests=$(here -s auth split -b "$claude" --focus=false -c "$home/src/auth" -- show tests.sh "$d/tests.txt")
here -s auth split -b "$tests" --split below --focus=false -c "$home/src/auth" >/dev/null
tab auth auth "dev server" server >/dev/null
here -s auth focus "$claude" >/dev/null 2>&1 || true
here new site >/dev/null
tab site site "Review the README" readme "state=done" >/dev/null
here -s site window close 1 >/dev/null
here new infra >/dev/null
tab infra infra "Migrate the users table" migrate \
  "state=blocked:kind=question:title=$(b64 'Run it on production now?')" >/dev/null
here -s infra window close 1 >/dev/null

# the window: 1440 x 900 points on the main display, sidebar style
display=$(swift - 2>/dev/null <<'SWIFT'
import CoreGraphics
print(CGMainDisplayID())
SWIFT
)
cat >"$d/state/app.json" <<EOF
{"tab_style":"sidebar","pane_headers":true,"sidebar_width":230,"window":{"x":60,"y":60,"w":1440,"h":900},"display":$display,"collapsed_hosts":[]}
EOF

open_app() {
  "${base[@]}" LEON_RUNTIME_DIR="$d/run" LEON_STATE_DIR="$d/state" LEON_CONFIG="$d/init.lua" LEON_SSH="$d/ssh" \
    "$leon" app >/dev/null 2>&1 &
  app_pid=$!
  sleep 4
  front
}
close_app() { kill "$app_pid" 2>/dev/null || true; wait "$app_pid" 2>/dev/null || true; app_pid=; sleep 1; }
front() { osascript -e "tell application \"System Events\" to set frontmost of (first process whose unix id is $app_pid) to true" >/dev/null; sleep 0.5; }
keys() { front; osascript -e "tell application \"System Events\" to $1"; sleep 0.8; }
shot() { sleep 0.5; swift "$repo/scripts/winshot.swift" leon "$out/$1.png" >/dev/null; echo "demo-shots: $1.png"; }

open_app
shot app-status
keys 'keystroke "p" using {command down, shift down}'
keys 'keystroke "log"'
shot app-palette
keys 'key code 53'
keys 'keystroke "b" using control down'
shot app-sequence
keys 'key code 53'
keys 'keystroke "s" using {command down, shift down}'
shot app-top
keys 'keystroke "s" using {command down, shift down}'
close_app

# a devbox with an agent waiting for permission and a failed deploy
dev server start >/dev/null
dev new api >/dev/null
dev -s api run --keep-open -c "$home/src/api" -- show "Add rate limiting" "$d/api.txt" \
  "state=blocked:kind=permission:title=$(b64 'Edit src/routes.rs?')" >/dev/null
dev -s api window close 1 >/dev/null
dev new deploy >/dev/null
dev -s deploy run --keep-open -c "$home/src/deploy" -- show "deploy api v2.3.1" "$d/deploy.txt" \
  "state=error:title=$(b64 'rollout failed: 2 of 4 pods crashed')" >/dev/null
dev -s deploy window close 1 >/dev/null
tab auth auth "make release" build "state=working:progress=40" >/dev/null
here -s auth focus "$claude" >/dev/null 2>&1 || true
here hosts add devbox ssh://dev@devbox >/dev/null
open_app
sleep 3
shot m16b-hosts
keys 'keystroke "k" using command down'
keys 'keystroke "api"'
shot app-switcher
keys 'key code 36'
shot m16b-remote-pane
keys 'keystroke "a" using {command down, shift down}'
sleep 1
shot app-activity
keys 'keystroke "a" using {command down, shift down}'
close_app

here hosts add build-box ssh://me@build-box >/dev/null
open_app
sleep 4
shot m16b-unreachable
close_app

# the empty state with a host: no session anywhere (the app makes one at start, so kill after)
here hosts remove build-box >/dev/null
dev kill api deploy >/dev/null
open_app
here kill auth site infra >/dev/null 2>&1 || true
sleep 2
shot m16b-empty-state
close_app

here hosts remove devbox >/dev/null
open_app
here kill 1 >/dev/null 2>&1 || true # the session the app made at start
sleep 2
shot app-empty
close_app

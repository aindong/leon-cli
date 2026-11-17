#!/usr/bin/env bash
# demo-shots.sh [OUT_DIR]: retake every screenshot in docs/images from a neutral demo, never the
# machine it runs on: a temp HOME whose zsh prompt is `auth %`, scripted programs instead of real
# agents, private LEON_RUNTIME_DIR/STATE_DIR/CONFIG, and a devbox that is a second local server
# behind a fake ssh. Keys go to leon's window through System Events (the terminal running this needs
# Accessibility). A shot is leon's window as the screen draws it, translucency and all, over a plain
# desktop window the script puts up itself (scripts/deskshot.swift checks that no other program's
# window is in the shot). It takes the GUI lock (/tmp/leon-gui.lock) while an app is open and stops
# if anything but leon comes to the front. Run it on an unlocked screen and leave the mouse and
# keyboard alone for the three minutes it takes. Check the result with
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

d=$(mktemp -d)
d=$(cd "$d" && pwd -P) # the real path, so the app writes HOME as ~
chmod 700 "$d"
mkdir -m 700 -p "$d/run" "$d/state" "$d/dev/run" "$d/dev/state"
home=$d/home
mkdir -p "$home/bin" "$home/.config/leon" "$home/src/auth" "$home/src/site" "$home/src/infra" "$home/src/api" "$home/src/deploy"
# a neutral shell: no user or host in the prompt or the title
cat >"$home/.zshrc" <<'EOF'
PROMPT='%1~ %% '
unsetopt PROMPT_SP
EOF
# in the demo's HOME, where Settings names it from ~
cat >"$home/.config/leon/init.lua" <<'EOF'
leon.action{ name = "tail_log", title = "Tail the server log", run = function() end }
leon.bind("cmd+shift+l", "tail_log")
leon.bind("ctrl+b>c", "client.tab.new")
leon.bind("ctrl+b>%", "pane.split.right")
leon.bind("ctrl+b>\"", "pane.split.down")
leon.bind("ctrl+b>z", "pane.zoom")
EOF
: >"$home/.config/leon/none.lua"

# show TITLE TEXTFILE [OSC7501 PAIRS]: a "program" that sets its title and status and shows the
# text, again after each resize (the app sizes the pane after it started). Perl, not sh: the app
# gives a pane whose program is `sh` the shell's icon, and this one's comes from its title.
cat >"$home/bin/show" <<'EOF'
#!/usr/bin/perl
my ($title, $text, $report) = @ARGV;
$| = 1;
print "\e]2;$title\a";
print "\e]7501;$report\a" if $report;
sub draw { open(my $f, '<', $text) or return; local $/; print "\e[H\e[2J\e[3J", <$f>; }
$SIG{WINCH} = \&draw;
draw();
sleep while 1;
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
txt notes <<'EOF'
## 0.4.0

- Sessions survive a server restart
- The login test no longer sleeps
- Rate limits on the public API
EOF
txt build <<'EOF'
$ make release
[ 40%] Building CXX object src/CMakeFiles/core.dir/engine.cpp.o
EOF

txt routes <<'EOF'
use axum::{routing::get, Router};

pub fn routes() -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/v1/orders", get(orders::list).post(orders::create))
        .layer(limits::per_key(100))
}
EOF
txt pytest <<'EOF'
============================= test session starts ==============================
collected 42 items

tests/test_orders.py ..........                                          [ 23%]
tests/test_limits.py ........                                            [ 42%]
tests/test_users.py ........................                             [100%]

============================== 42 passed in 1.84s ==============================
EOF
txt caddy <<'EOF'
INFO  using config from file  {"file": "Caddyfile"}
INFO  serving initial configuration
INFO  http.log.access  {"method": "GET", "uri": "/health", "status": 200}
EOF
txt htop <<'EOF'
  CPU [|||||||||               24.1%]   Tasks: 212, 3 running
  Mem [||||||||||||       6.12G/16.0G]   Load average: 1.42 1.20 0.98

    PID  CPU% MEM%  Command
   4121  31.0  2.4  api-server
   4180  12.5  1.1  postgres
   4202   3.2  0.4  worker --queue orders
EOF

# LEON_RELEASE_URL: the app's look for a release goes to a loopback port that refuses, not to GitHub
base=(env -u LEON_SESSION -u LEON_BLOCK -u ZDOTDIR HOME="$home" SHELL=/bin/zsh PATH="$home/bin:$PATH" LEON_RELEASE_URL=http://127.0.0.1:9 LEON_TAILSCALE="$d/no-tailscale")
here() { "${base[@]}" LEON_RUNTIME_DIR="$d/run" LEON_STATE_DIR="$d/state" LEON_CONFIG="$home/.config/leon/init.lua" "$leon" "$@"; }
dev() { "${base[@]}" LEON_RUNTIME_DIR="$d/dev/run" LEON_STATE_DIR="$d/dev/state" LEON_CONFIG="$home/.config/leon/none.lua" "$leon" "$@"; }

# the devbox: ssh's argv ends with HOST CMD; this one serves the second server, and build-box fails
cat >"$d/ssh" <<EOF
#!/bin/sh
for a; do host=\$prev; prev=\$a; done
case "\$host" in
  *build-box*) echo "ssh: connect to host build-box port 22: Connection refused" >&2; exit 255 ;;
esac
exec env -u LEON_SESSION -u LEON_BLOCK HOME="$home" SHELL=/bin/zsh LEON_RUNTIME_DIR="$d/dev/run" \
  LEON_STATE_DIR="$d/dev/state" LEON_CONFIG="$home/.config/leon/none.lua" "$leon" server proxy
EOF
chmod +x "$d/ssh"

# the machine-wide GUI lock: keys go to whichever app is frontmost, so one taker at a time holds
# the screen, from an app's start to its close
lock=/tmp/leon-gui.lock
held=
take_lock() {
  until mkdir "$lock" 2>/dev/null; do echo "demo-shots: waiting for $lock" >&2; sleep 3; done
  held=1
}
drop_lock() { [ -z "$held" ] || rmdir "$lock" 2>/dev/null || true; held=; }
app_pid=
desk_pid=
cleanup() {
  [ -n "$app_pid" ] && kill "$app_pid" 2>/dev/null || true
  [ -n "$desk_pid" ] && kill "$desk_pid" 2>/dev/null || true
  drop_lock
  here --autostart=false server stop --discard >/dev/null 2>&1 || true
  dev --autostart=false server stop --discard >/dev/null 2>&1 || true
  rm -rf "${d:?}"
}
trap cleanup EXIT

# a tab (window) in SESSION running `show` in DIR; prints its block id
tab() { here -s "$1" run --keep-open -c "$home/src/$2" -- show "$3" "$d/$4.txt" "${5:-}"; }

here config check "$home/.config/leon/init.lua" >/dev/null # a broken demo config would quietly drop the palette's action
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
{"tab_style":"sidebar","pane_headers":true,"sidebar_width":230,"window":{"x":160,"y":120,"w":1440,"h":900},"display":$display,"collapsed_hosts":[]}
EOF

ts=$d/no-tailscale
# a plain desktop behind the window for its translucency (scripts/deskshot.swift), its own binary
# so the window's owner is the pid we start
swiftc -O -o "$d/deskshot" "$repo/scripts/deskshot.swift"
open_app() {
  take_lock
  "$d/deskshot" backdrop &
  desk_pid=$!
  "${base[@]}" LEON_RUNTIME_DIR="$d/run" LEON_STATE_DIR="$d/state" LEON_CONFIG="$home/.config/leon/init.lua" LEON_SSH="$d/ssh" \
    LEON_TAILSCALE="$ts" "$leon" app >/dev/null 2>&1 &
  app_pid=$!
  sleep 4
  front
}
close_app() {
  kill "$app_pid" "$desk_pid" 2>/dev/null || true
  wait "$app_pid" "$desk_pid" 2>/dev/null || true
  app_pid= desk_pid=
  sleep 1
  drop_lock
}
front() { osascript -e "tell application \"System Events\" to set frontmost of (first process whose unix id is $app_pid) to true" >/dev/null; sleep 0.5; }
# keys go only to the app this script started: anything else in front stops the run
in_front() {
  front
  local top
  top=$(osascript -e 'tell application "System Events" to unix id of first process whose frontmost is true')
  [ "$top" = "$app_pid" ] || { echo "demo-shots: pid $top is in front, not leon ($app_pid); stopping" >&2; exit 1; }
}
keys() {
  in_front
  osascript -e "tell application \"System Events\" to $1"
  sleep 0.8
}
# shot NAME [PAD]: leon's frontmost window and PAD points around it, as the screen draws it over the
# plain desktop; deskshot refuses when any other program's window is in the way
shot() {
  in_front
  sleep 0.5
  "$d/deskshot" "$app_pid" "$desk_pid" "$out/$1.png" "${2:-0}"
  echo "demo-shots: $1.png"
}

open_app
shot app-status
shot app-translucent 80
sips -Z 1760 "$out/app-translucent.png" >/dev/null # twice the README's width: sharp at half the bytes
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
# the overview of every tab; Change Theme… resting on another theme (esc goes back); Settings
keys 'keystroke "\\" using {command down, shift down}'
sleep 1.5
shot app-overview
keys 'key code 53'
keys 'keystroke "t" using {command down, shift down}'
keys 'key code 125'
keys 'key code 125'
keys 'key code 125'
sleep 1
shot app-theme
keys 'key code 53'
keys 'keystroke "," using command down'
sleep 1
keys 'key code 125'
shot app-settings
keys 'keystroke "w" using command down'
# a float over the split, then the scratch drawer open below the tiles; both closed again after
notes=$(here -s auth float --new --label notes --rect 0.44,0.08,0.52,0.5 -- show "Release notes" "$d/notes.txt")
sleep 1
shot app-float
here -s auth toggle notes >/dev/null
here -s auth toggle scratch --create >/dev/null
sleep 1.5
shot app-drawer
scratch=$(here -s auth block ls --json | python3 -I -c 'import json,sys; print([b["block_id"] for b in json.load(sys.stdin) if b.get("label") == "scratch"][0])')
here -s auth block close "$notes" >/dev/null
here -s auth block close "$scratch" >/dev/null
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

# a busy tab bar, a tab per program and its icon, with the tabs in the titlebar; then Add Host…
# listing a made-up tailnet (the demo kit's stand-in tailscale)
here new shop >/dev/null
agent=$(tab shop api "Add rate limiting" api "state=working:progress=30:title=$(b64 agent)")
tab shop api "nvim src/routes.rs" routes >/dev/null
tab shop api "pytest -q" pytest >/dev/null
tab shop api "caddy run" caddy >/dev/null
tab shop api "htop" htop >/dev/null
tab shop api "ssh devbox" server >/dev/null
here -s shop focus "$agent" >/dev/null 2>&1 || true
ts=$repo/scripts/demo-video/kit/bin/tailscale
open_app
keys 'keystroke "k" using command down'
keys 'keystroke "nvim"'
keys 'key code 36'
keys 'keystroke "s" using {command down, shift down}'
sleep 1
shot app-icons
keys 'keystroke "s" using {command down, shift down}'
keys 'keystroke "p" using {command down, shift down}'
keys 'keystroke "add host"'
keys 'key code 36'
sleep 2
shot app-tailnet
keys 'key code 53'
close_app
ts=$d/no-tailscale

# the empty state with a host: no session anywhere (the app makes one at start, so kill after)
here hosts remove build-box >/dev/null
dev kill api deploy >/dev/null
open_app
here kill auth site infra shop >/dev/null 2>&1 || true
sleep 2
shot m16b-empty-state
close_app

here hosts remove devbox >/dev/null
open_app
here kill 1 >/dev/null 2>&1 || true # the session the app made at start
sleep 2
shot app-empty
close_app

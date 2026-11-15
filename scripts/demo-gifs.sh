#!/usr/bin/env bash
# demo-gifs.sh [OUT_DIR]: record docs/images/demo-app.gif and demo-cli.gif from a neutral demo, as
# demo-shots.sh takes the screenshots: a temp HOME whose prompt is `$ `, a scripted agent instead of a
# real one, and private LEON_RUNTIME_DIR/STATE_DIR/CONFIG. The app is recorded as a run of captures
# of leon's own window (screencapture -l, never the screen); the CLI in vhs's own headless terminal.
# Needs ffmpeg and vhs (brew install ffmpeg vhs), Accessibility for the terminal running it, and an
# unlocked screen; leave the mouse and keyboard alone for the minute it takes. Check the result
# with `cargo test -p leon --test m13_privacy` and an OCR of the frames.
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
  echo "demo-gifs: the screen is locked; a locked screen draws nothing (exit 2)"
  exit 2
fi
cargo build --release -q -p leon
leon=$repo/target/release/leon

d=$(cd "$(mktemp -d)" && pwd -P) # the real path, so the app writes HOME as ~
chmod 700 "$d"
mkdir -m 700 -p "$d/run" "$d/state" "$d/cli/run" "$d/cli/state" "$d/frames"
home=$d/home
mkdir -p "$home/bin" "$home/src/auth" "$home/src/infra" "$home/src/app"
# a neutral shell: no user or host in the prompt or the title
cat >"$home/.zshrc" <<'EOF'
PROMPT='$ '
unsetopt PROMPT_SP
EOF
# the CLI demo's shells are bash, whose system default prompt shows the host and the login
cat >"$home/.bashrc" <<'EOF'
PS1='$ '
EOF
cp "$home/.bashrc" "$home/.bash_profile"
: >"$d/none.lua"
ln -s "$leon" "$home/bin/leon"

b64() { printf %s "$1" | base64; }
# agent TEXTFILE: a scripted agent that shows its text and reports progress (OSC 7501) until
# $d/blocked exists, then asks for permission
cat >"$home/bin/agent" <<EOF
#!/bin/sh
printf '\033]2;Fix the flaky login test\007'
cat "\$1"
p=10
while [ ! -e "$d/blocked" ]; do
  printf '\033]7501;state=working:progress=%s:title=$(b64 Claude):msg=$(b64 'running the tests')\007' "\$p"
  [ "\$p" -lt 90 ] && p=\$((p + 10))
  sleep 1
done
printf '\033]7501;state=blocked:kind=permission:title=$(b64 'Edit auth/session_test.rs?')\007'
printf '\n  Edit auth/session_test.rs? (y/n) '
while :; do sleep 100000 & wait \$!; done
EOF
# show TITLE TEXTFILE [OSC7501 PAIRS]: a program that sets its title and status and shows the text
cat >"$home/bin/show" <<'EOF'
#!/bin/sh
printf '\033]2;%s\007' "$1"
[ -n "${3:-}" ] && printf '\033]7501;%s\007' "$3"
cat "$2"
while :; do sleep 100000 & wait $!; done
EOF
chmod +x "$home/bin/agent" "$home/bin/show"
cat >"$d/claude.txt" <<'EOF'
> Fix the flaky login test in auth/session_test.rs

  The test sleeps 50 ms and then checks the session expired.
  On a busy machine the check runs before the timer fires.

  Running the tests beside me:
    leon split --label tests -- cargo test -p auth
EOF
cat >"$d/tests.txt" <<'EOF'
running 18 tests
test session::tests::refresh_keeps_the_id ... ok
test session::tests::expired_session_is_rejected ... ok
test token::tests::round_trip ... ok
EOF
cat >"$d/migrate.txt" <<'EOF'
> Migrate the users table to the new schema

  Dry run on staging: 18,204 rows, 0 conflicts.

  Run it on production now?
EOF

base=(env -u LEON_SESSION -u LEON_BLOCK -u ZDOTDIR HOME="$home" SHELL=/bin/zsh PATH="$home/bin:/usr/bin:/bin:/usr/sbin:/sbin")
here() { "${base[@]}" LEON_RUNTIME_DIR="$d/run" LEON_STATE_DIR="$d/state" LEON_CONFIG="$d/none.lua" "$leon" "$@"; }

app_pid=
cleanup() {
  [ -n "$app_pid" ] && kill "$app_pid" 2>/dev/null || true
  here --autostart=false server stop --discard >/dev/null 2>&1 || true
  rm -rf "${d:?}"
}
trap cleanup EXIT

here server start >/dev/null
here new auth >/dev/null
claude=$(here -s auth run --keep-open -c "$home/src/auth" -- agent "$d/claude.txt")
here -s auth window close 1 >/dev/null
here -s auth split -b "$claude" --focus=false -c "$home/src/auth" -- show tests.sh "$d/tests.txt" "state=done" >/dev/null
here new infra >/dev/null
here -s infra run --keep-open -c "$home/src/infra" -- show "Migrate the users table" "$d/migrate.txt" \
  "state=blocked:kind=question:title=$(b64 'Run it on production now?')" >/dev/null
here -s infra window close 1 >/dev/null

display=$(swift - 2>/dev/null <<'SWIFT'
import CoreGraphics
print(CGMainDisplayID())
SWIFT
)
cat >"$d/state/app.json" <<EOF
{"tab_style":"sidebar","pane_headers":true,"sidebar_width":210,"window":{"x":60,"y":60,"w":1200,"h":760},"display":$display,"collapsed_hosts":[]}
EOF

"${base[@]}" LEON_RUNTIME_DIR="$d/run" LEON_STATE_DIR="$d/state" LEON_CONFIG="$d/none.lua" "$leon" app >/dev/null 2>&1 &
app_pid=$!
sleep 4
front() { osascript -e "tell application \"System Events\" to set frontmost of (first process whose unix id is $app_pid) to true" >/dev/null; }
keys() { front; osascript -e "tell application \"System Events\" to $1"; sleep "${2:-0.8}"; }
typ() { keys "keystroke \"$1\"" 0.3; }
front
here -s auth focus "$claude" >/dev/null 2>&1 || true
sleep 1
wid=$(swift - 2>/dev/null <<'SWIFT'
import CoreGraphics
let list = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID) as! [[String: Any]]
let w = list.first { ($0[kCGWindowOwnerName as String] as? String) == "leon" && ($0[kCGWindowLayer as String] as? Int) == 0 }
print(w?[kCGWindowNumber as String] as? Int ?? 0)
SWIFT
)
[ "$wid" != 0 ] || { echo "demo-gifs: no leon window" >&2; exit 1; }

# capture leon's window as fast as screencapture goes, each frame with its time
(
  i=0
  while [ ! -e "$d/stop" ]; do
    f=$(printf '%s/frames/%05d.png' "$d" "$i")
    screencapture -x -o -l"$wid" "$f" && echo "$(perl -MTime::HiRes=time -e 'printf "%.3f", time') $f" >>"$d/frames.txt"
    i=$((i + 1))
  done
) &
rec=$!
sleep 1.5
keys 'keystroke "d" using command down' 1.2          # split right: a shell beside the agent
typ "ls ~/src"; keys 'key code 36' 1.5
keys 'keystroke "k" using command down' 0.8          # the switcher
typ "infra"; sleep 0.8; keys 'key code 36' 2
keys 'keystroke "k" using command down' 0.8
typ "auth"; sleep 0.8; keys 'key code 36' 1.5
keys 'keystroke "j" using command down' 1.2          # the scratch drawer
typ "echo a shell that stays"; keys 'key code 36' 1.5
keys 'keystroke "j" using command down' 1.2
touch "$d/blocked"                                   # the agent asks for permission
sleep 2.5
keys 'keystroke "a" using {command down, shift down}' 3   # Activity: what needs you
keys 'keystroke "a" using {command down, shift down}' 1
touch "$d/stop"
wait "$rec"
kill "$app_pid" 2>/dev/null || true
wait "$app_pid" 2>/dev/null || true
app_pid=

# frames.txt: "time file" lines; each frame lasts until the next one
awk 'NR > 1 { printf "file %s\nduration %.3f\n", f, $1 - t } { t = $1; f = $2 } END { printf "file %s\nduration 1.5\nfile %s\n", f, f }' \
  "$d/frames.txt" >"$d/frames.ffconcat"
gif() { # IN_ARGS... OUT: a 1100 px wide GIF, one palette, repeated frames dropped (the size)
  local out=${*: -1}
  ffmpeg -loglevel error -y "${@:1:$#-1}" -vf \
    "fps=10,scale=1100:-1:flags=lanczos,mpdecimate,split[a][b];[a]palettegen=max_colors=96:stats_mode=diff[p];[b][p]paletteuse=dither=none:diff_mode=rectangle" \
    -loop 0 "$out"
}
gif -f concat -safe 0 -i "$d/frames.ffconcat" "$out/demo-app.gif"
echo "demo-gifs: demo-app.gif"

# the CLI, in vhs's headless terminal: bash with a `$ ` prompt and its own private server
cat >"$d/demo.tape" <<EOF
Output "$d/cli.mp4"
Set Shell "bash"
Set FontSize 17
Set Width 1100
Set Height 620
Set Padding 18
Set TypingSpeed 35ms
Set Theme "Builtin Dark"
Env HOME "$home"
Env SHELL "/bin/bash"
Env BASH_SILENCE_DEPRECATION_WARNING "1"
Env PATH "$home/bin:$(dirname "$(command -v ttyd)"):/usr/bin:/bin:/usr/sbin:/sbin"
Env LEON_RUNTIME_DIR "$d/cli/run"
Env LEON_STATE_DIR "$d/cli/state"
Env LEON_CONFIG "$d/none.lua"
Hide
Type "export PS1='\$ '; cd ~/src/app; clear"
Enter
Sleep 500ms
Show
Sleep 600ms
Type "leon new demo"
Enter
Sleep 1.2s
Type "leon -s demo run --wait --output -- sh -c 'echo building; sleep 1; echo 18 tests passed'"
Enter
Sleep 2.8s
Type "leon -s demo run --label py -- python3 -q"
Enter
Sleep 900ms
Type "leon -s demo wait -b py --text '>>>'"
Enter
Sleep 1s
Type "leon -s demo send -b py 'print(6 * 7)' && leon -s demo send-key -b py enter"
Enter
Sleep 1s
Type "leon -s demo wait -b py --text 42 && leon -s demo capture -b py --trim"
Enter
Sleep 2.2s
Type "leon attach demo"
Enter
Sleep 2.5s
Ctrl+B
Type "%"
Sleep 2s
Type "echo a second pane"
Enter
Sleep 1.8s
Ctrl+B
Type "d"
Sleep 1.8s
EOF
(cd "$d" && vhs demo.tape >/dev/null)
"${base[@]}" LEON_RUNTIME_DIR="$d/cli/run" LEON_STATE_DIR="$d/cli/state" "$leon" --autostart=false server stop --discard >/dev/null 2>&1 || true
gif -i "$d/cli.mp4" "$out/demo-cli.gif"
echo "demo-gifs: demo-cli.gif"

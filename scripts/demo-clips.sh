#!/usr/bin/env bash
# demo-clips.sh [--only NAME] [OUT_DIR]: record one short clip per feature of the next release, each
# as NAME.gif (at most 1.5 MB, for the changelog and the release notes) and NAME.mp4 (for the
# website's demo page, through the media release: docs/releases.md#cutting-a-release). OUT_DIR is
# target/clips by default. The scenes:
#   agents         an agent starts two agents, and leon tells it when they report
#   link-card      resting the mouse on a link shows its card
#   tabs           closing and renaming tabs and sessions
#   cursor-shader  a cursor trail, from scripts/demo-clips/trail.glsl (leon's own, MIT)
# A neutral demo, as demo-gifs.sh makes one: a temp HOME whose prompt is `$ `, scripted programs
# instead of real agents, private LEON_RUNTIME_DIR/STATE_DIR/CONFIG, and a video of leon's window
# only, over a plain desktop of our own (`deskshot record`). Keys go to leon through System Events
# under the GUI lock (/tmp/leon-gui.lock), each only when leon is in front. Needs ffmpeg,
# Accessibility and Screen Recording for the terminal running it, and an unlocked screen; leave
# the mouse and keyboard alone while it runs (about 20 seconds a clip). Check the clips with
# `cargo test -p leon --test m13_privacy` once they are in docs/images, and a look at the frames.
set -euo pipefail
cd "$(dirname "$0")/.."
repo=$(pwd)
scenes=(agents link-card tabs cursor-shader)
if [ "${1:-}" = --only ]; then
  for s in "${scenes[@]}"; do [ "$s" = "${2:-}" ] && only=$s; done
  [ -n "${only:-}" ] || { echo "demo-clips: --only takes one of: ${scenes[*]}" >&2; exit 2; }
  scenes=("$only")
  shift 2
fi
[ $# -le 1 ] || { echo "usage: scripts/demo-clips.sh [--only NAME] [OUT_DIR]" >&2; exit 2; }
out=${1:-$repo/target/clips}
mkdir -p "$out"
out=$(cd "$out" && pwd)

locked=$(swift - 2>/dev/null <<'SWIFT' || echo 0
import CoreGraphics
let d = CGSessionCopyCurrentDictionary() as? [String: Any] ?? [:]
print(d["CGSSessionScreenIsLocked"] as? Int ?? 0)
SWIFT
)
if [ "$locked" = 1 ]; then
  echo "demo-clips: the screen is locked; a locked screen draws nothing (exit 2)"
  exit 2
fi
command -v ffmpeg >/dev/null || { echo "demo-clips: needs ffmpeg (brew install ffmpeg)" >&2; exit 1; }
cargo build --release -q -p leon
leon=$repo/target/release/leon

d=$(mktemp -d)
d=$(cd "$d" && pwd -P) # the real path, so the app writes HOME as ~
chmod 700 "$d"
trap 'rm -rf "${d:?}"' EXIT
home=$d/home
mkdir -p "$home/bin" "$home/src/shelf/src"
: >"$home/src/shelf/src/main.rs"
git -C "$home/src/shelf" init -q -b main # for the shell's `git status`
swiftc -O -o "$d/deskshot" "$repo/scripts/deskshot.swift" 2>/dev/null
# a smaller window than the README's GIF: a clip is one feature, and its GIF must stay small
read -r wx wy ww wh < <("$d/deskshot" window 62)
cat >"$home/.zshrc" <<'EOF'
PROMPT='$ '
unsetopt PROMPT_SP
EOF
ln -s "$leon" "$home/bin/leon"
cp "$repo/scripts/demo-clips/trail.glsl" "$d/trail.glsl"
b64() { printf %s "$1" | base64; }

# worker PROMPT: a scripted agent for `leon agent start`, whose prompt is the task, then leon's
# paragraph on reporting: it works for a moment and reports the task's first line as its result
cat >"$home/bin/worker" <<EOF
#!/usr/bin/perl
my (\$result) = split /\n/, \$ARGV[-1];
\$| = 1;
print "\e]2;\$result\a";
print "\e]7501;state=working:title=$(b64 Claude)\a";
print "\n  ● Working on it…\n";
sleep 4;
print "  ● \$result\n";
system("leon", "notify", "--state", "done", "--body", \$result);
sleep while 1;
EOF
# lead GO: the orchestrator, a scripted agent that, once the file GO exists (the recording runs),
# starts two workers, ends its turn, and shows what leon types into its prompt when they report
cat >"$home/bin/lead" <<'EOF'
#!/usr/bin/perl
$| = 1;
print "\e]2;Release prep\a";
select(undef, undef, undef, 0.2) until -e $ARGV[0];
print "\n  > Get the release ready: run the tests and check the docs.\n\n";
sleep 1;
print "  ● Starting two agents to work on it:\n";
system("leon agent start --split right --label tests worker '18 tests pass' >/dev/null");
print "    leon agent start --label tests\n";
sleep 1;
system("leon agent start --split below --label docs worker 'The docs build, no broken links' >/dev/null");
print "    leon agent start --label docs\n\n  ● I'll wait for their reports.\n\n> ";
system("leon", "notify", "--state", "done", "--body", "waiting for the tests and docs agents");
while (my $line = <STDIN>) {
  print "\n  ● Reports from the agents:\n";
  open(my $inbox, "-|", "leon", "inbox") or last;
  print "    $_" while <$inbox>;
  print "\n> ";
}
EOF
# links: a program that prints a link and waits
cat >"$home/bin/links" <<'EOF'
#!/bin/sh
printf '\033]2;Release checklist\007'
printf '\n  Review the changes before tagging:\n\n  https://example.com/leon/compare/v0.1.1...dev\n\n'
exec sleep 100000
EOF
chmod +x "$home/bin/worker" "$home/bin/lead" "$home/bin/links"

base=(env -i HOME="$home" SHELL=/bin/zsh PATH="$home/bin:/usr/bin:/bin:/usr/sbin:/sbin" TERM=xterm-256color LANG=en_US.UTF-8 LEON_RELEASE_URL=http://127.0.0.1:9)
lock=/tmp/leon-gui.lock
held= app_pid= desk_pid= rec= run=
here() { "${base[@]}" LEON_RUNTIME_DIR="$run/run" LEON_STATE_DIR="$run/state" LEON_CONFIG="$run/init.lua" "$leon" "$@"; }
stop_all() {
  [ -n "$rec" ] && { kill "$rec" 2>/dev/null; wait "$rec" 2>/dev/null; } || true
  [ -n "$app_pid" ] && kill "$app_pid" 2>/dev/null || true
  [ -n "$desk_pid" ] && kill "$desk_pid" 2>/dev/null || true
  [ -n "$app_pid$desk_pid" ] && { wait "$app_pid" "$desk_pid" 2>/dev/null || true; }
  rec= app_pid= desk_pid=
  [ -z "$held" ] || rmdir "$lock" 2>/dev/null || true
  held=
  [ -z "$run" ] || here --autostart=false server stop --discard >/dev/null 2>&1 || true
}
trap 'stop_all; rm -rf "${d:?}"' EXIT

# fresh NAME [LUA]: a server of its own for the scene, with LUA in its init.lua
fresh() {
  run=$d/$1
  mkdir -m 700 -p "$run/run" "$run/state"
  cat >"$run/init.lua" <<EOF
leon.set("notifications.done", false)
leon.set("notifications.blocked", false)
leon.set("notifications.error", false)
leon.set("font.size", 17) -- a clip is shown smaller than the window: text it can be read in
${2:-}
EOF
  here server start >/dev/null
}

front() { osascript -e "tell application \"System Events\" to set frontmost of (first process whose unix id is $app_pid) to true" >/dev/null; }
# keys AS [WAIT]: the check and the key are one AppleScript, so keys go only to the app this
# script started, and anything else in front stops the run
keys() {
  osascript -e "tell application \"System Events\"
    if (unix id of (first process whose frontmost is true)) is not $app_pid then error \"not leon\"
    $1
  end tell" >/dev/null || { echo "demo-clips: leon ($app_pid) isn't in front; no keys sent" >&2; exit 1; }
  sleep "${2:-0.8}"
}
typ() { keys "keystroke \"$1\"" "${2:-0.3}"; }
# point X Y [right]: the mouse at X,Y points from the window's top left, and a right click there
point() {
  [ "$(osascript -e 'tell application "System Events" to unix id of (first process whose frontmost is true)')" = "$app_pid" ] \
    || { echo "demo-clips: leon ($app_pid) isn't in front; no mouse" >&2; exit 1; }
  swift - "$((wx + $1))" "$((wy + $2))" "${3:-move}" <<'SWIFT'
import CoreGraphics
let a = CommandLine.arguments, p = CGPoint(x: Double(a[1])!, y: Double(a[2])!)
let kinds: [CGEventType] = a[3] == "right" ? [.mouseMoved, .rightMouseDown, .rightMouseUp] : [.mouseMoved]
for k in kinds {
  CGEvent(mouseEventSource: nil, mouseType: k, mouseCursorPosition: p, mouseButton: k == .mouseMoved ? .left : .right)?.post(tap: .cghidEventTap)
}
SWIFT
}

# app: the window where every clip has it, then the app, in front, recording
app() {
  local display
  display=$(swift - 2>/dev/null <<'SWIFT'
import CoreGraphics
print(CGMainDisplayID())
SWIFT
)
  cat >"$run/state/app.json" <<EOF
{"tab_style":"sidebar","pane_headers":true,"sidebar_width":200,"window":{"x":$wx,"y":$wy,"w":$ww,"h":$wh},"display":$display,"collapsed_hosts":[]}
EOF
  until mkdir "$lock" 2>/dev/null; do echo "demo-clips: waiting for $lock" >&2; sleep 3; done
  held=1
  "$d/deskshot" backdrop &
  desk_pid=$!
  "${base[@]}" LEON_RUNTIME_DIR="$run/run" LEON_STATE_DIR="$run/state" LEON_CONFIG="$run/init.lua" "$leon" app >/dev/null 2>&1 &
  app_pid=$!
  sleep 4
  front
  sleep 1
  "$d/deskshot" record "$app_pid" "$desk_pid" "$run/clip.mov" >"$run/rec.log" &
  rec=$!
  for _ in $(seq 100); do grep -q recording "$run/rec.log" && break; sleep 0.1; done
  grep -q recording "$run/rec.log" || { cat "$run/rec.log" >&2; echo "demo-clips: the recorder didn't start" >&2; exit 1; }
  sleep 0.6
}

# finish NAME: stop recording, close the app, write NAME.mp4 and a NAME.gif of at most 1.5 MB
finish() {
  sleep 0.8
  kill -TERM "$rec"
  wait "$rec" || { cat "$run/rec.log" >&2; echo "demo-clips: $1: the take failed its check (another window came over it?)" >&2; exit 1; }
  rec=
  stop_all
  ffmpeg -loglevel error -y -i "$run/clip.mov" -vf "scale=1600:-2:flags=lanczos,format=yuv420p" \
    -c:v libx264 -crf 24 -preset slow -movflags +faststart -an "$out/$1.mp4"
  local width
  for width in 1100 960 820 700; do
    ffmpeg -loglevel error -y -i "$run/clip.mov" -vf \
      "fps=12,scale=$width:-1:flags=lanczos,mpdecimate,split[a][b];[a]palettegen=max_colors=80:stats_mode=diff[p];[b][p]paletteuse=dither=none:diff_mode=rectangle" \
      -loop 0 "$out/$1.gif"
    [ "$(wc -c <"$out/$1.gif")" -le 1500000 ] && break
  done
  [ "$(wc -c <"$out/$1.gif")" -le 1500000 ] || { echo "demo-clips: $1.gif is over 1.5 MB even at ${width}px" >&2; exit 1; }
  echo "demo-clips: $1.gif ($(($(wc -c <"$out/$1.gif") / 1024)) KB, ${width}px) and $1.mp4"
}

scene_agents() {
  fresh agents
  here new release >/dev/null
  here -s release run --keep-open -c "$home/src/shelf" -- lead "$run/go" >/dev/null
  here -s release window close 1 >/dev/null
  app
  touch "$run/go"
  sleep 14 # it starts both, they work, report, and leon types the notice into its prompt
  finish agents
}

scene_link-card() {
  fresh link-card
  here new docs >/dev/null
  here -s docs run --keep-open -c "$home/src/shelf" -- links >/dev/null
  here -s docs window close 1 >/dev/null
  app
  # the link's line, the fourth of the pane's text (an estimate: look at the clip's frames, and
  # move it if the card doesn't show); the sidebar is 200 points
  point 330 118
  sleep 2.5
  point 330 118 right
  sleep 2.5
  keys 'key code 53' 1 # Escape
  finish link-card
}

scene_tabs() {
  fresh tabs
  here new api >/dev/null
  here new web >/dev/null
  for t in server tests logs; do here -s api run --keep-open -c "$home/src/shelf" -- sh -c "printf '\033]2;$t\007'; exec zsh -i" >/dev/null; done
  here -s api window close 1 >/dev/null
  app
  keys 'keystroke "p" using {command down, shift down}' 0.6 # the palette: rename this tab
  typ "rename tab" 0.6; keys 'key code 36' 0.8
  typ "deploy" 0.6; keys 'key code 36' 1.2
  keys 'keystroke "w" using {command down, option down}' 1.4 # Close Tab
  keys 'keystroke "p" using {command down, shift down}' 0.6 # rename the session
  typ "rename session" 0.6; keys 'key code 36' 0.8
  typ "shelf api" 0.6; keys 'key code 36' 1.2
  keys 'keystroke "k" using command down' 0.6 # to the other session, and close it
  typ "web" 0.4; keys 'key code 36' 1
  keys 'keystroke "w" using {command down, option down, shift down}' 1.6 # Close Session
  finish tabs
}

scene_cursor-shader() {
  fresh cursor-shader "leon.set(\"cursor_shader\", \"$d/trail.glsl\")
leon.set(\"cursor_shader_duration\", 0.6)"
  here new shelf >/dev/null
  here -s shelf run --keep-open -c "$home/src/shelf" -- zsh -i >/dev/null
  here -s shelf window close 1 >/dev/null
  app
  typ "git status --short" 0.6
  keys 'keystroke "a" using control down' 0.7 # to the line's start: the trail streaks back
  keys 'keystroke "e" using control down' 0.7 # and to its end
  keys 'key code 36' 1
  typ "ls -1" 0.4; keys 'key code 36' 1.2
  typ "echo the cursor leaves a trail" 0.4
  keys 'keystroke "a" using control down' 0.7
  keys 'keystroke "e" using control down' 0.9
  finish cursor-shader
}

for s in "${scenes[@]}"; do
  "scene_$s"
done

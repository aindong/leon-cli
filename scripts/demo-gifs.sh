#!/usr/bin/env bash
# demo-gifs.sh [OUT_DIR]: record docs/images/demo-app.gif and demo-cli.gif from a neutral demo, as
# demo-shots.sh takes the screenshots: a temp HOME whose prompt is `$ `, a scripted agent instead of a
# real one, and private LEON_RUNTIME_DIR/STATE_DIR/CONFIG. The app is a video of leon's window over
# a plain desktop of our own (`deskshot record`: 30 frames a second, so the motion shows, and a take
# is thrown away if any other program's window comes over it); the CLI is recorded in vhs's own
# headless terminal. Keys go to leon through System Events under the GUI lock
# (/tmp/leon-gui.lock), each only when leon is in front. Needs ffmpeg and vhs (brew install ffmpeg
# vhs), Accessibility and Screen Recording for the terminal running it, and an unlocked screen;
# leave the mouse and keyboard alone for the minute it takes. Check the result with
# `cargo test -p leon --test m13_privacy` and a look at the frames.
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
for tool in ffmpeg vhs ttyd; do
  command -v "$tool" >/dev/null || { echo "demo-gifs: needs $tool (brew install ffmpeg vhs)" >&2; exit 1; }
done
cargo build --release -q -p leon
leon=$repo/target/release/leon

d=$(mktemp -d)
d=$(cd "$d" && pwd -P) # the real path, so the app writes HOME as ~
chmod 700 "$d"
mkdir -m 700 -p "$d/run" "$d/state" "$d/cli/run" "$d/cli/state"
home=$d/home
mkdir -p "$home/bin" "$home/src/auth" "$home/src/infra" "$home/src/app"
swiftc -O -o "$d/deskshot" "$repo/scripts/deskshot.swift" 2>/dev/null
# the window: 80% of the screen, centred (as in demo-shots.sh)
read -r wx wy ww wh < <("$d/deskshot" window 80)
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
# no notification banners on the screen; ctrl+alt+g is docs/lua/keys.md's popup recipe, as it is there
cat >"$d/init.lua" <<'EOF'
leon.set("notifications.done", false)
leon.set("notifications.blocked", false)
leon.set("notifications.error", false)
EOF
awk '/<!-- popup-git -->/{on=1;next} /<!-- \/popup-git -->/{on=0} on && !/^```/' docs/lua/keys.md >>"$d/init.lua"
ln -s "$leon" "$home/bin/leon"
# the popup's history: a made-up author and fixed dates, so the same hashes every run
(
  export GIT_AUTHOR_NAME=demo GIT_AUTHOR_EMAIL=demo@example.com GIT_COMMITTER_NAME=demo GIT_COMMITTER_EMAIL=demo@example.com
  n=0
  c() { n=$((n + 1)); export GIT_AUTHOR_DATE="@$((1793000000 + n * 86400)) +0000" GIT_COMMITTER_DATE="@$((1793000000 + n * 86400)) +0000"; }
  cd "$home/src/auth"
  mkdir src tests
  printf '[package]\nname = "auth"\nversion = "0.4.0"\n' >Cargo.toml
  : >README.md; : >src/lib.rs; : >src/session.rs; : >src/token.rs; : >tests/session_test.rs
  git init -q --template= -b main .
  c; git add -A; git commit -q -m "sessions: a cookie per login"
  c; git commit -q --allow-empty -m "tokens: sign with the key from the vault"
  git checkout -q -b expiry
  c; git commit -q --allow-empty -m "sessions expire after an hour idle"
  c; git commit -q --allow-empty -m "a test clock instead of sleeps"
  git checkout -q main
  c; git commit -q --allow-empty -m "refresh keeps the session id"
  c; git merge -q --no-ff expiry -m "merge expiry"
  c; git commit -q --allow-empty -m "login: rate limit by address"
  git checkout -q -b fix-flaky-login
)

b64() { printf %s "$1" | base64; }
# agent TEXTFILE: a scripted agent that shows its text and reports progress (OSC 7501) until
# $d/blocked exists, then asks for permission. Perl, not sh: the app gives a pane whose program is
# `sh` the shell's icon, and an unknown program that reports a status is an agent.
cat >"$home/bin/agent" <<EOF
#!/usr/bin/perl
my (\$text) = @ARGV;
\$| = 1;
my \$asked = 0;
print "\e]2;Fix the flaky login test\a";
sub draw {
  open(my \$f, '<', \$text) or return; local \$/;
  print "\e[H\e[2J\e[3J", <\$f>;
  print "\n  Edit auth/session_test.rs? (y/n) " if \$asked;
}
\$SIG{WINCH} = \&draw;
draw();
my \$p = 10;
until (-e "$d/blocked") {
  print "\e]7501;state=working:progress=\$p:title=$(b64 Claude):msg=$(b64 'running the tests')\a";
  \$p += 10 if \$p < 90;
  sleep 1;
}
print "\e]7501;state=blocked:kind=permission:title=$(b64 'Edit auth/session_test.rs?')\a";
\$asked = 1;
draw();
sleep while 1;
EOF
# show TITLE TEXTFILE [OSC7501 PAIRS]: a program that sets its title and status and shows the
# text, again after each resize (as in demo-shots.sh)
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
chmod +x "$home/bin/agent" "$home/bin/show"
txt() { cat >"$d/$1.txt"; }
. "$repo/scripts/demo-texts.sh"

# a clean env: nothing of this machine (tokens, TERM_PROGRAM, ssh agent) reaches the demo
# LEON_RELEASE_URL: the app's look for a release goes to a loopback port that refuses, not to GitHub
base=(env -i HOME="$home" SHELL=/bin/zsh PATH="$home/bin:/usr/bin:/bin:/usr/sbin:/sbin" TERM=xterm-256color LANG=en_US.UTF-8 LEON_RELEASE_URL=http://127.0.0.1:9)
here() { "${base[@]}" LEON_RUNTIME_DIR="$d/run" LEON_STATE_DIR="$d/state" LEON_CONFIG="$d/init.lua" "$leon" "$@"; }

# the machine-wide GUI lock: keys go to whichever app is frontmost, so one taker at a time holds
# the screen, from the app's start to its close
lock=/tmp/leon-gui.lock
held=
app_pid=
desk_pid=
rec=
cleanup() {
  [ -n "$rec" ] && { kill "$rec" 2>/dev/null; wait "$rec" 2>/dev/null; } || true
  [ -n "$app_pid" ] && kill "$app_pid" 2>/dev/null || true
  [ -n "$desk_pid" ] && kill "$desk_pid" 2>/dev/null || true
  [ -z "$held" ] || rmdir "$lock" 2>/dev/null || true
  here --autostart=false server stop --discard >/dev/null 2>&1 || true
  "${base[@]}" LEON_RUNTIME_DIR="$d/cli/run" LEON_STATE_DIR="$d/cli/state" "$leon" --autostart=false server stop --discard \
    >/dev/null 2>&1 || true
  rm -rf "${d:?}"
}
trap cleanup EXIT

here server start >/dev/null
here new auth >/dev/null
claude=$(here -s auth run --keep-open -c "$home/src/auth" -- agent "$d/claude.txt")
here -s auth window close 1 >/dev/null
# the agent, wide enough to split once more | its tests over a monitor
tests=$(here -s auth split -b "$claude" --ratio 35 --focus=false -c "$home/src/auth" -- show tests.sh "$d/tests.txt" "state=done")
here -s auth split -b "$tests" --split below --focus=false -c "$home/src/auth" -- show htop "$d/htop.txt" >/dev/null
here -s auth run --keep-open -c "$home/src/auth" -- show "dev server" "$d/server.txt" >/dev/null
here new infra >/dev/null
migrate=$(here -s infra run --keep-open -c "$home/src/infra" -- show "Migrate the users table" "$d/migrate.txt" \
  "state=blocked:kind=question:title=$(b64 'Run it on production now?')")
here -s infra window close 1 >/dev/null
here -s infra split -b "$migrate" --focus=false -c "$home/src/infra" -- show "nvim 0042_users_email_unique.sql" "$d/sql.txt" >/dev/null
here -s auth focus "$claude" >/dev/null

display=$(swift - 2>/dev/null <<'SWIFT'
import CoreGraphics
print(CGMainDisplayID())
SWIFT
)
cat >"$d/state/app.json" <<EOF
{"tab_style":"sidebar","pane_headers":true,"sidebar_width":230,"window":{"x":$wx,"y":$wy,"w":$ww,"h":$wh},"display":$display,"collapsed_hosts":[]}
EOF

until mkdir "$lock" 2>/dev/null; do echo "demo-gifs: waiting for $lock" >&2; sleep 3; done
held=1
"$d/deskshot" backdrop &
desk_pid=$!
# not `here app &`: $! must be the app itself, for the keys and the window below
"${base[@]}" LEON_RUNTIME_DIR="$d/run" LEON_STATE_DIR="$d/state" LEON_CONFIG="$d/init.lua" "$leon" app >/dev/null 2>&1 &
app_pid=$!
sleep 4
front() { osascript -e "tell application \"System Events\" to set frontmost of (first process whose unix id is $app_pid) to true" >/dev/null; }
# keys AS [WAIT]: the check and the key are one AppleScript, so keys go only to the app this
# script started, and anything else in front stops the run
keys() {
  osascript -e "tell application \"System Events\"
    if (unix id of (first process whose frontmost is true)) is not $app_pid then error \"not leon\"
    $1
  end tell" >/dev/null || { echo "demo-gifs: leon ($app_pid) isn't in front; no keys sent" >&2; exit 1; }
  sleep "${2:-0.8}"
}
typ() { keys "keystroke \"$1\"" "${2:-0.3}"; }
front
sleep 1

"$d/deskshot" record "$app_pid" "$desk_pid" "$d/app.mov" &
rec=$!
sleep 1.2
keys 'keystroke "d" using command down' 1            # split right: the panes make room
typ "ls src tests"; keys 'key code 36' 1
keys 'keystroke "]" using {command down, shift down}' 1.3   # the next tab fades in, and back
keys 'keystroke "[" using {command down, shift down}' 1
keys 'keystroke "p" using {command down, shift down}' 0.6   # the palette: split below
typ "split below" 0.6; keys 'key code 36' 1.2
keys 'keystroke "w" using command down' 1            # and close it again
keys 'keystroke "g" using {control down, option down}' 2.2   # the git popup (docs/lua/keys.md)
keys 'keystroke "g" using {control down, option down}' 1
keys 'keystroke "k" using command down' 0.6          # the switcher, to another session
typ "infra" 0.5; keys 'key code 36' 1.5
touch "$d/blocked"                                   # the agent asks for permission
sleep 2
keys 'keystroke "a" using {command down, shift down}' 2.2   # Activity: what needs you
keys 'keystroke "a" using {command down, shift down}' 1
kill -TERM "$rec"
wait "$rec"
rec=
kill "$app_pid" "$desk_pid" 2>/dev/null || true
wait "$app_pid" "$desk_pid" 2>/dev/null || true
app_pid= desk_pid=
rmdir "$lock" 2>/dev/null || true
held=

gif() { # IN_ARGS... OUT: a ${width:-1100} px wide GIF, one palette, repeated frames dropped (the size)
  local out=${*: -1}
  ffmpeg -loglevel error -y "${@:1:$#-1}" -vf \
    "fps=${fps:-10},scale=${width:-1100}:-1:flags=lanczos,mpdecimate,split[a][b];[a]palettegen=max_colors=96:stats_mode=diff[p];[b][p]paletteuse=dither=none:diff_mode=rectangle" \
    -loop 0 "$out"
}
# 20 a second: the app's moves last 100 to 160 ms; 1760 px, twice the README's column, as the stills
fps=20 width=1760 gif -i "$d/app.mov" "$out/demo-app.gif"
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
Type "exec env -i HOME='$home' SHELL=/bin/bash BASH_SILENCE_DEPRECATION_WARNING=1 PATH='$home/bin:/usr/bin:/bin:/usr/sbin:/sbin' TERM=xterm-256color LANG=en_US.UTF-8 LEON_RUNTIME_DIR='$d/cli/run' LEON_STATE_DIR='$d/cli/state' LEON_CONFIG='$d/none.lua' PS1='\$ ' bash --noprofile --norc"
Enter
Sleep 300ms
Type "cd ~/src/app; clear"
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
Sleep 2s
Ctrl+B
Type "%"
Sleep 2s
Type "echo a second pane"
Enter
Sleep 1.8s
Ctrl+B
Type "d"
Sleep 1.2s
EOF
(cd "$d" && vhs demo.tape >/dev/null)
gif -i "$d/cli.mp4" "$out/demo-cli.gif"
echo "demo-gifs: demo-cli.gif"

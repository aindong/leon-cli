#!/usr/bin/env bash
# run-demo.sh STEP: the launch demo in the Tart VM `leon-demo`, from the host, over ssh and VNC.
# Nothing here touches the host's own leon, screen or sessions.
#   boot          start the VM headless (6 cores) and wait for ssh
#   push          build keypost, copy target/release/Leon.app (or $DEMO_APP), kit/ and this Mac's
#                 git into the VM (build first: scripts/bundle.sh)
#   setup         a clean take: reset, stage the sessions and agents, full screen, pointer parked
#   take [A-B]    record scenes A to B (default 1-11), pull the clip, make take.mp4 and take-cut.mp4
#   cue [BEATS]   rebuild a take's cue file (take-cue.jsonl) from its beats file (default the last
#                 take's), sending nothing
#   render        the finished videos from the take: renders/leon-demo-main.mp4 (16:9) and
#                 renders/leon-demo-square-30s.mp4 (1:1), each with an .srt
#   shot NAME     a VNC screenshot of the VM to shots/NAME.png
#   retake REV    Leon.app built from commit REV on this Mac (the VM has no toolchain), then push,
#                 setup, take, render: the real take after a change, in one command
#   check         the projects' tests in the VM
#   stop          stop the VM
# Files go to $DEMO_WORK (default target/demo-video). The one-time VM setup (Setup Assistant,
# the TCC grants for sshd, the devbox account, the ssh key) is in README.md next to this script.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$here/../.." && pwd)
vm=${DEMO_VM:-leon-demo} # the VM's name in `tart list` (TART_HOME, if set, says where tart keeps it)
# what a take makes and what the scripts keep between steps: the ssh key, the VM's address, the
# raw take, its beats and cue file, the renders; under target/, which git ignores
work=${DEMO_WORK:-$repo/target/demo-video}
mkdir -p "$work"

vnc_url() { grep -o 'vnc://[^ ]*' "$work/tart-run.log" | tail -1; }
ssh_opts() {
  echo -i "$work/key" -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR \
    -o ControlMaster=auto -o ControlPath=/tmp/ldcm-%C -o ControlPersist=10m \
    -o ServerAliveInterval=5 -o ServerAliveCountMax=2
}
ip() { tart ip "$vm" --wait 120; }
# The cue file (take-cue.jsonl): one JSON object a line, written as the take sends each step, for
# the compositor. Times are seconds on the script's clock (t0); the video runs `lead` ahead of it.
# With CUE_ONLY=1 nothing is sent: the scenes run on a virtual clock, re-anchored at each mark to
# an earlier take's beats file, to rebuild that take's cues (`run-demo.sh cue take-beats.txt`).
cuefile=$work/take-cue.jsonl
vt=0
now() {
  if [ -n "${CUE_ONLY:-}" ]; then echo "$vt"; else python3 -c "import time; print(round(time.time() - $t0, 3))"; fi
}
jstr() { local s=${1//\\/\\\\}; s=${s//\"/\\\"}; printf '"%s"' "$s"; }
cue() { # outside a take (setup, check) there is no clock and nothing to record
  [ -n "${t0:-}" ] || [ -n "${CUE_ONLY:-}" ] || return 0
  printf '{"t": %s, %s}\n' "$(now)" "$1" >>"$cuefile"
}
spend() { vt=$(python3 -c "print(round($vt + $1, 3))"); } # a replayed step's usual length
guest() { ssh $(ssh_opts) "demo@$(cat "$work/.ip")" "bash kit/guest.sh $(printf "%q " "$@")"; }
# g STEP…: guest.sh in the VM as demo (one ssh connection, reused, so beats land on time)
g() {
  cue "\"type\": \"step\", \"step\": $(jstr "$*")"
  if [ -n "${CUE_ONLY:-}" ]; then
    case "$1" in tabs) spend 2.4 ;; app) spend 2.6 ;; *) spend 0.3 ;; esac
    return
  fi
  guest "$@"
}
# k combo KEYS | k key NAME [show] | k type TEXT | k click X Y (points): keys and clicks in the VM
k() {
  case "$1" in
    combo) cue "\"type\": \"key\", \"combo\": $(jstr "$2"), \"show\": true" ;;
    key) cue "\"type\": \"key\", \"combo\": $(jstr "$2"), \"show\": $([ "${3:-}" = show ] || [ "$2" = esc ] && echo true || echo false)" ;;
    type) cue "\"type\": \"type\", \"text\": $(jstr "$2")" ;;
    click) cue "\"type\": \"click\", \"x\": $(($2 * 2)), \"y\": $(($3 * 2))" ;; # raw pixels: the screen is 2x
  esac
  if [ -n "${CUE_ONLY:-}" ]; then
    case "$1" in type) spend "$(python3 -c "print(0.15 + 0.067 * len('''$2'''))")" ;; *) spend 0.15 ;; esac
    return
  fi
  [ "$1" = key ] && set -- key "$2" # "show" is for the cue only
  guest key "$@"
}
vnc() { python3 "$here/vnc.py" "$(vnc_url)" "$@"; }
at() { # at SECONDS: wait until SECONDS after the take started
  if [ -n "${CUE_ONLY:-}" ]; then
    vt=$(python3 -c "print(max($vt, $1))")
    return
  fi
  local now; now=$(python3 -c 'import time; print(time.time())')
  python3 -c "import time; time.sleep(max(0, $t0 + $1 - $now))"
}
# zoom X Y W H UNTIL: the compositor zooms onto this region of the raw frame (pixels) until UNTIL.
# The camera shows the region widened by 12% about its centre and to the frame's shape, at most 1.84x (16:9) so text
# stays sharp: pick regions whose widened edges fall on pane borders, not mid-word
zoom() { cue "\"type\": \"zoom\", \"region\": [$1, $2, $3, $4], \"until\": $5"; }
# caption TEXT UNTIL [SMALL]: a burned-in caption (and an .srt line) from now until UNTIL
caption() { cue "\"type\": \"caption\", \"text\": $(jstr "$1"), \"until\": $2, \"small\": $(jstr "${3:-}")"; }

# Each scene starts where the one before ended; times are seconds from the take's start (t0,
# 1.5 s after the capture starts). `mark` writes a beat's time to take-beats.txt for the edit.
mark() {
  if [ -n "${CUE_ONLY:-}" ]; then # re-anchor the virtual clock on the real take's mark
    local real; real=$(awk -v m="$*" '{ t = $1; $1 = ""; sub(/^ +/, ""); if ($0 == m) { print t; exit } }' "$anchor")
    [ -n "$real" ] && vt=$real
  else
    printf '%6.1f  %s\n' "$(now)" "$*" >>"$work/take-beats.txt"
  fi
  cue "\"type\": \"mark\", \"name\": $(jstr "$*")"
}
scene1() { # the empty state: the lion, "Keep your work in view", the shortcuts
  mark "1 empty state" # no caption: the empty state says it
  # zoomed on its top half from before the first frame (the move ends at -1.6 s, so the video
  # opens on it): the empty state's third row names two other products. It chains into scene 2's zoom
  printf '{"t": -2.3, "type": "zoom", "region": [757, 271, 1365, 768], "until": 4.2}\n' >>"$cuefile"
}
scene2() { # build up: a session, a shell, an agent beside it, then the others one by one
  at 4; mark "2 build-up"; k combo cmd+n
  caption "Start with one terminal." 10.5
  at 5.2; zoom 0 9 1600 900 10.3 # the sidebar and the shell, as it is typed into
  at 5.5; k type "cd work/shelf-api"; k key return
  at 7.3; k type "leon split --ratio 60 -- agent"; k key return
  at 10.6; caption "Add agents as you go." 17.3
  at 11; g build 1  # a dev server tab (leon run shows it), and the session gets its name
  at 12.2; k combo cmd+1
  at 13.4; g build 2
  at 14.6; g build 3
}
scene3() { # the agent opens a pane under itself and runs the tests
  at 17.5; mark "3 agent runs its tests"; g go shelf-tests
  caption "Agents open their own panes." 25.8 "leon split · leon wait · leon capture"
  at 18.2; zoom 1470 960 1378 640 24.6 # the tests pane the agent opened
}
scene4() { # Activity: blocked, error, done; jump to the blocked one and answer
  at 26; mark "4 activity"; k combo cmd+shift+a
  caption "Working, blocked, done: at a glance." 29.4
  at 26.4; zoom 0 60 1040 560 30.2 # the Activity list
  at 28.5; k click 100 100 # the blocked row (the pointer is parked again at once)
  at 29.5; caption "Jump to the one that needs you." 36.8
  at 30; k type y
  at 30.6; k key return
  at 34; k combo cmd+shift+a
}
scene5() { # speed: a 400 MB coloured log floods the shell while the agent takes typing; 50 tabs
  at 37; mark "5 speed"; k combo cmd+k
  at 37.7; k type pagination # not "pag": status-page matches that too
  at 39.2; k key return show
  at 39.8; k combo cmd+alt+left
  at 40.3; k type "cat ~/logs/app.log"; k key return
  at 41.6; mark "5 flood starts"; k combo cmd+alt+right
  caption "115 MB/s through the terminal." 45.9 "and typing keeps up"
  zoom 1000 500 1300 620 45.9 # the flood's right half and the agent's prompt
  at 42.2; k type "also test limit=0 and an offset past the end"; k key return
  at 45.5; mark "5 44 tabs"; g tabs
  at 48.5; mark "5 overview"; k combo 'cmd+shift+\'
  caption "50 tabs. Every one live." 52
  at 52; k key esc
  g untabs # the 44 background tabs go: devbox has to show in the sidebar later
}
scene6() { # ⌘K to the red agent; while it fixes the page, its repo's history in a popup
  at 52.5; mark "6 switch"; k combo cmd+k
  caption "⌘K: any session, tab or pane." 55.1
  at 53.2; k type mobile
  at 54.2; k key return show # its red tests show for a second
  # init.lua's ctrl+alt+g, the recipe in docs/lua/keys.md: git log --graph in a centred popup
  at 55.3; mark "6 popup"; k combo ctrl+alt+g
  caption "A popup on one key: the repo's history." 59.8 "the same key hides it"
  at 55.6; g go status-fix # the agent fixes the page under the popup: its sidebar row turns done
  at 58.4; k combo ctrl+alt+g # hidden: the focus is back on the agent, green
  at 58.7; zoom 0 9 1714 900 60.2 # the sidebar and the agent, to the pane border
}
scene7() { # Settings (⌘,): Appearance, a theme for the whole app, live, and back
  at 60.5; mark "7 settings"; k combo cmd+,
  caption "Your colours, live." 68.8 "Settings · Appearance" # the small line's font has no ⌘
  at 61.4; k key down # the sidebar's next section: Appearance
  # macOS is light in the VM, so the live column is "When macOS is light" (the window is centred)
  at 62.6; k click 937 382 # leon-light
  at 65.4; k click 937 346 # leon-dark again
  at 67.6; k key esc # closes it; the main window has the keys again
}
scene8() { # leon do: a whole tab from one Lua action
  at 69.5; mark "8 leon do"; k combo cmd+j
  caption "Script your layouts in Lua." 76.8 "leon do review"
  at 70.5; k type "leon do review"; k key return
  at 72.4; zoom 637 9 2086 1100 76.6 # the review tab's panes: the camera spans 512 (the sidebar's border) to 2848: tests, server, live tally
}
scene9() { # Tailscale: Add Host lists the tailnet; pick devbox; its session
  at 77; mark "9 tailscale"; k combo cmd+shift+p
  caption "Pick a machine from your tailnet." 84.8
  at 77.8; k type "add host"
  at 79; k key return show
  # the sidebar, the palette and the shell pane: its edges fall on pane borders and column gaps
  at 79.4; zoom 0 9 1768 990 87.0
  at 81.5; k type dev
  at 82.5; k key return show
  at 85; caption "Its sessions, beside your own." 90.6
  at 85.5; k combo cmd+k
  at 86.2; k type nightly
  at 87.2; k key return show
  at 87.6; zoom 0 9 1714 720 90.6 # devbox in the sidebar, its agent with the devbox badge, to the pane border
}
scene10() { # quit the app; the programs keep going; open it again
  at 91; mark "10 quit"; k combo cmd+q
  caption "Quit the app…" 93
  at 93; mark "10 reopen"; g app
  g fullscreen # it opens windowed: the compositor and take-cut.mp4 drop that second
  at 96.9; caption "…everything is still running." 101.3
  at 97; k combo cmd+k
  at 97.7; k type review
  at 98.5; k key return show
}
scene11() { # the payoff: every tab, This Mac and devbox, live
  at 101.5; mark "11 overview"; k combo 'cmd+shift+\' # live cards: screencapture -V needs frames to stop
  caption "Keep your work in view." 104.5
}

case "${1:-}" in
  boot)
    nohup tart run "$vm" --no-graphics --vnc-experimental --no-audio >"$work/tart-run.log" 2>&1 &
    ip >"$work/.ip"
    [ -f "$work/key" ] || { echo "run-demo: no ssh key at $work/key (README, one-time setup step 4)" >&2; exit 1; }
    for ((n = 0; ; n++)); do
      ssh $(ssh_opts) -o ConnectTimeout=2 "demo@$(cat "$work/.ip")" true 2>/dev/null && break
      [ $n -lt 120 ] || { echo "run-demo: no ssh to demo@$(cat "$work/.ip") after 2 minutes" >&2; exit 1; }
      sleep 1
    done
    echo "boot: $(cat "$work/.ip"), $(vnc_url)" ;;
  push)
    ip >"$work/.ip"
    rm -rf "${work:?}/kit"
    cp -R "$here/kit" "$work/kit"
    find "$work/kit" -name __pycache__ -prune -exec rm -rf {} +
    swiftc -O -target arm64-apple-macos13 "$here/keypost.swift" -o "$work/kit/bin/keypost"
    # the popup's git log: the VM has no developer tools, and Apple's git needs only system libraries
    cp "$(xcrun -f git)" "$work/kit/bin/git"
    app=${DEMO_APP:-$repo/target/release/Leon.app}
    [ "$(basename "$app")" = Leon.app ] && [ -x "$app/Contents/MacOS/leon" ] ||
      { echo "run-demo: no Leon.app at $app (scripts/bundle.sh, or DEMO_APP)" >&2; exit 1; }
    COPYFILE_DISABLE=1 tar -C "$(dirname "$app")" -cf - Leon.app |
      ssh $(ssh_opts) "demo@$(cat "$work/.ip")" 'rm -rf ~/leon/target/release/Leon.app && mkdir -p ~/leon/target/release && tar -xf - -C ~/leon/target/release'
    COPYFILE_DISABLE=1 tar -C "$work" -cf - kit |
      ssh $(ssh_opts) "demo@$(cat "$work/.ip")" 'rm -rf ~/kit && tar -xf - -C ~ && tar -C ~ -cf - kit | ssh devbox "rm -rf ~/kit && tar -xf - -C ~"'
    ssh $(ssh_opts) "demo@$(cat "$work/.ip")" 'cat ~/leon/target/release/Leon.app/Contents/MacOS/leon | ssh devbox "cat > ~/.local/bin/leon.new && chmod +x ~/.local/bin/leon.new && mv -f ~/.local/bin/leon.new ~/.local/bin/leon && ~/.local/bin/leon --version"' ;;
  setup)
    ip >"$work/.ip"
    g prefs
    g reset
    g stage
    sleep 2
    g fullscreen
    sleep 2
    echo "setup: ready; check it with: $0 shot setup" ;;
  take)
    range=${2:-1-11}
    from=${range%-*} to=${range#*-}
    secs=${TAKE_SECS:-106}
    # screencapture hangs when it is started in the background inside the VM, so it runs in the
    # foreground of its own ssh session, and that session is in the background here
    ssh $(ssh_opts) "demo@$(cat "$work/.ip")" "rm -f ~/take.mov; screencapture -v -V $secs -x ~/take.mov </dev/null 2>~/take.err; echo \"screencapture: exit \$? \$(cat ~/take.err)\" >&2" &
    rec=$!
    # after a pane closes, a still second ends the VM's recorder early: the parked pointer steps
    # on the right edge (cropped off) the whole take, so the screen is never still
    guest key wiggle "$secs" &
    wig=$!
    trap 'kill $rec $wig 2>/dev/null; ssh $(ssh_opts) "demo@$(cat "$work/.ip")" pkill -INT screencapture' EXIT
    sleep 1.5 # the capture's own start; the edit trims this head
    t0=$(python3 -c 'import time; print(time.time())')
    : >"$work/take-beats.txt"
    printf '{"t": 0, "type": "take", "lead": 1.5, "width": 2880, "height": 1620}\n' >"$cuefile"
    for ((n = from; n <= to; n++)); do "scene$n"; done
    wait "$rec" "$wig"
    trap - EXIT
    ssh $(ssh_opts) "demo@$(cat "$work/.ip")" 'cat ~/take.mov' >"$work/take-raw.mov"
    # 60 fps constant rate at 1080p for editors. The crop keeps 16:9 and takes 32 px off the right:
    # macOS's recording dot (top right) and the parked pointer's sliver (right edge) are there
    ffmpeg -loglevel error -y -i "$work/take-raw.mov" -vf crop=2848:1602:0:9,fps=60,scale=1920:1080:flags=lanczos \
      -c:v libx264 -crf 16 -pix_fmt yuv420p "$work/take.mp4"
    python3 "$here/cut.py" "$work/take.mp4" "$work/take-cut.mp4"
    echo "take: $work/take.mp4, $work/take-cut.mp4; beats: $work/take-beats.txt" ;;
  cue) # cue BEATS: rebuild an earlier take's cue file from its beats file, sending nothing
    anchor=${2:-$work/take-beats.txt}
    [ -f "$anchor" ] || anchor=$work/$anchor # a bare name means the work dir's
    [ -f "$anchor" ] || { echo "run-demo: no beats file $anchor" >&2; exit 1; }
    CUE_ONLY=1
    printf '{"t": 0, "type": "take", "lead": 1.5, "width": 2880, "height": 1620}\n' >"$cuefile"
    for ((n = 1; n <= 11; n++)); do "scene$n"; done
    echo "cue: $cuefile ($(wc -l <"$cuefile" | tr -d ' ') lines, rebuilt from $anchor)" ;;
  render)
    venv=$work/venv
    if ! "$venv/bin/python" -c 'import numpy, PIL' 2>/dev/null; then # none yet, or a failed install
      rm -rf "${venv:?}"
      python3 -m venv "$venv"
      "$venv/bin/pip" install -q -r "$here/compose/requirements.txt"
    fi
    cut=$(python3 "$here/cut.py" --print "$work/take.mp4") # the reopen's windowed seconds, A:B
    mkdir -p "$work/renders"
    nice -n 10 "$venv/bin/python" "$here/compose/compose.py" main "$work/take-raw.mov" "$work/take-cue.jsonl" \
      "$work/renders/leon-demo-main.mp4" --cut="$cut"
    nice -n 10 "$venv/bin/python" "$here/compose/compose.py" square "$work/take-raw.mov" "$work/take-cue.jsonl" \
      "$work/renders/leon-demo-square-30s.mp4" ;;
  shot) mkdir -p "$work/shots"; vnc shot "$work/shots/$2.png"; echo "$work/shots/$2.png" ;;
  retake) # retake REV: build REV's Leon.app from a clean export of that commit, then film and render
    rev=$(git -C "$repo" rev-parse --verify "${2:?run-demo: retake needs a commit}^{commit}")
    src=$work/src
    rm -rf "${src:?}"
    mkdir -p "$src"
    git -C "$repo" archive "$rev" | tar -x -C "$src"
    # its own target dir (kept between retakes, so a later commit builds incrementally); the
    # export has no .git, so the commit goes in by hand for `leon --version`
    (cd "$src" && LEON_GIT_HASH=${rev:0:12} cargo build --release -p leon --target-dir "$work/cargo")
    "$src/scripts/bundle.sh" --bin "$work/cargo/release/leon" --out "$work/app/Leon.app"
    DEMO_APP=$work/app/Leon.app "$0" push
    "$0" setup
    "$0" take
    "$0" render
    echo "retake: $rev filmed and rendered in $work/renders" ;;
  check) ip >"$work/.ip"; g check ;;
  stop) tart stop "$vm" ;;
  *) sed -n '2,19s/^# \{0,1\}//p' "$0"; exit 2 ;;
esac

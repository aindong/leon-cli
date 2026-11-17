#!/usr/bin/env bash
# run-demo.sh STEP: the launch demo in the Tart VM `leon-demo`, from the host, over ssh and VNC.
# Nothing here touches the host's own leon, screen or sessions.
#   boot          start the VM headless (6 cores) and wait for ssh
#   push          build keypost, copy target/release/Leon.app (or $DEMO_APP), kit/ and this Mac's
#                 git into the VM (build first: scripts/bundle.sh)
#   setup         a clean take: reset, stage the sessions and agents, full screen, pointer parked
#   take [A-B]    record scenes A to B (default 1-12), pull the clip, make take.mp4 and take-cut.mp4;
#                 exits 3 when the recording stopped early (the VM's recorder sometimes does)
#   cue [BEATS]   rebuild a take's cue file (take-cue.jsonl) from its beats file (default the last
#                 take's), sending nothing
#   render        the finished videos from the take: renders/leon-demo-main.mp4 (16:9) and
#                 renders/leon-demo-square-30s.mp4 (1:1), each with an .srt
#   shot NAME     a VNC screenshot of the VM to shots/NAME.png
#   retake REV    Leon.app built from commit REV on this Mac (the VM has no toolchain), then push,
#                 setup, take, render: the real take after a change, in one command (a take that
#                 stopped early is filmed again from setup, up to 3 tries)
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
. "$here/scenes.sh" # scene1 … scene12

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
    g terminal # the attach scene's terminal, on the desktop before the app goes full screen
    sleep 1
    g fullscreen
    sleep 2
    echo "setup: ready; check it with: $0 shot setup" ;;
  take)
    range=${2:-1-12}
    from=${range%-*} to=${range#*-}
    secs=${TAKE_SECS:-117}
    # screencapture hangs when it is started in the background inside the VM, so it runs in the
    # foreground of its own ssh session, and that session is in the background here
    ssh $(ssh_opts) "demo@$(cat "$work/.ip")" "rm -f ~/take.mov; screencapture -v -V $secs -x ~/take.mov </dev/null 2>~/take.err; echo \"screencapture: exit \$? \$(cat ~/take.err)\" >&2" &
    rec=$!
    # after a pane closes, a still second ends the VM's recorder early: the parked pointer steps
    # on the right edge (cropped off) the whole take, so the screen is never still
    guest key wiggle "$secs" &
    wig=$!
    trap 'kill $rec $wig 2>/dev/null; ssh $(ssh_opts) "demo@$(cat "$work/.ip")" "pkill -INT screencapture; pkill -f \"[k]eypost wiggle\""' EXIT
    sleep 1.5 # the capture's own start; the edit trims this head
    t0=$(python3 -c 'import time; print(time.time())')
    : >"$work/take-beats.txt"
    printf '{"t": 0, "type": "take", "lead": 1.5, "width": 2880, "height": 1620}\n' >"$cuefile"
    for ((n = from; n <= to; n++)); do "scene$n"; done
    wait "$rec" "$wig"
    trap - EXIT
    ssh $(ssh_opts) "demo@$(cat "$work/.ip")" 'cat ~/take.mov' >"$work/take-raw.mov"
    # `screencapture -v -V` sometimes ends early and still exits 0 (one take in four stopped at
    # 64 of 117 s): a recording more than 2 s short is not a take
    got=$(ffprobe -v error -show_entries format=duration -of csv=p=0 "$work/take-raw.mov" 2>/dev/null || echo 0)
    if python3 -c "import sys; sys.exit(0 if float(sys.argv[1] or 0) < $secs - 2 else 1)" "$got"; then # short
      echo "take: the recording stopped at ${got:-0} s of $secs: film it again (setup, then take)" >&2
      exit 3
    fi
    # the recorder's start varies: the cue's lead becomes the one measured on this take
    lead=$(python3 "$here/lead.py" "$work/take-raw.mov" "$cuefile") || lead="1.5, not measured"
    echo "take: the video runs $lead s ahead of the script"
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
    # the lead `take` measured stays: it is the video's, not the script's
    lead=$(python3 -c 'import json,sys; print(json.loads(open(sys.argv[1]).readline())["lead"])' "$cuefile" 2>/dev/null) || lead=1.5
    printf '{"t": 0, "type": "take", "lead": %s, "width": 2880, "height": 1620}\n' "$lead" >"$cuefile"
    for ((n = 1; n <= 12; n++)); do "scene$n"; done
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
    git -C "$repo" archive "$rev" | tar -xm -C "$src" # -m: now as mtime, so cargo rebuilds what an older REV changed
    # its own target dir (kept between retakes, so a later commit builds incrementally); the
    # export has no .git, so the commit goes in by hand for `leon --version`
    (cd "$src" && LEON_GIT_HASH=${rev:0:12} cargo build --release -p leon --target-dir "$work/cargo")
    "$src/scripts/bundle.sh" --bin "$work/cargo/release/leon" --out "$work/app/Leon.app"
    DEMO_APP=$work/app/Leon.app "$0" push
    for n in 1 2 3; do # a take stops early now and then (exit 3): the state it used is spent, so setup again
      "$0" setup
      rc=0
      "$0" take || rc=$?
      [ $rc -eq 0 ] && break
      [ $rc -eq 3 ] && [ $n -lt 3 ] || exit $rc
      echo "retake: take $n stopped early; filming it again"
    done
    "$0" render
    echo "retake: $rev filmed and rendered in $work/renders" ;;
  check) ip >"$work/.ip"; g check ;;
  stop) tart stop "$vm" ;;
  *) sed -n '2,22s/^# \{0,1\}//p' "$0"; exit 2 ;;
esac

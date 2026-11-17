#!/bin/bash
# guest.sh STEP: the launch demo inside the VM, run as `demo` over ssh by run-demo.sh.
#   reset       quit the app, stop both leon servers, forget their sessions, fresh copies of the projects
#               (status-page with a made-up git history, for the popup)
#   prefs       once per VM: no desktop widgets, no macOS tips, the tailnet's ssh name, the big log
#   stage       the app on an empty leon; the devbox's agent already running on its own server
#   build N     the take's build-up beats: 1 a dev server tab, 2 logtally's agent, 3 status-page's
#   tabs        about 44 more tabs of quiet background jobs, for the 50-tab overview (untabs: gone)
#   go [NAME]   start the agents (or one named beat: shelf-tests, status-fix)
#   answer TEXT type an answer into the blocked agent (the take does it with real keys)
#   app         start the app again (after a ⌘Q in the take)
#   quit        quit the app (kills its pid; the take uses ⌘Q)
#   fullscreen  click the window's green button (pointer parked again at once)
#   key ARGS    keypost ARGS: combo cmd+k | type TEXT | key return | park | click X Y
#   l ARGS      leon ARGS, against the demo's server
#   check       the projects' tests, from fresh copies
set -euo pipefail
kit=$HOME/kit
d=$HOME/leon-demo
# the config is a copy: `leon config set` in the take writes to it
export LEON_RUNTIME_DIR=$d/run LEON_STATE_DIR=$d/state LEON_CONFIG=$d/init.lua
# the stand-in tailscale (kit/bin/tailscale): a made-up tailnet, `status --json` only
export LEON_TAILSCALE=$kit/bin/tailscale
export PATH=$kit/bin:$HOME/python/bin:$HOME/leon/target/release/Leon.app/Contents/MacOS:/usr/bin:/bin:/usr/sbin:/sbin
export SHELL=/bin/zsh
app=$HOME/leon/target/release/Leon.app
b64() { printf %s "$1" | base64; }
app_pid() { pgrep -fx "$app/Contents/MacOS/leon" || true; }
# git_history DIR: a made-up git history for the popup's `git log --graph` (git is the host's, copied
# into kit/bin by `run-demo.sh push`: the VM has no developer tools); fixed dates, the same hashes
git_history() ( # a subshell: its git identity and cd stay in it
  export GIT_AUTHOR_NAME=demo GIT_AUTHOR_EMAIL=demo@example.com GIT_COMMITTER_NAME=demo GIT_COMMITTER_EMAIL=demo@example.com
  local n=0
  c() { n=$((n + 1)); export GIT_AUTHOR_DATE="@$((1793000000 + n * 86400)) +0000" GIT_COMMITTER_DATE="@$((1793000000 + n * 86400)) +0000"; }
  cd "$1"
  git init -q --template= -b main .
  c; git add -A; git commit -q -m "status page: one row per service, from services.json"
  c; git commit -q --allow-empty -m "headline names the worst status"
  git checkout -q -b latency
  c; git commit -q --allow-empty -m "show p95 latency for each service"
  c; git commit -q --allow-empty -m "a dash when a service reports no latency"
  git checkout -q main
  c; git commit -q --allow-empty -m "escape service names in the page"
  c; git merge -q --no-ff latency -m "merge latency"
  c; git commit -q --allow-empty -m "watch.py: rebuild the page when a source changes"
  c; git commit -q --allow-empty -m "5000 services render in under a second"
  git checkout -q -b mobile-layout # the agent's branch
)

case "${1:-}" in
  reset)
    p=$(app_pid)
    if [ -n "$p" ]; then
      kill $p
      for _ in $(seq 50); do kill -0 $p 2>/dev/null || break; sleep 0.1; done
      kill -9 $p 2>/dev/null || true
    fi
    leon --autostart=false server stop --discard >/dev/null 2>&1 || true
    # started without restoring first: a server killed with the VM left saved sessions behind
    ssh devbox '~/.local/bin/leon server start --restore=false; ~/.local/bin/leon server stop --discard' >/dev/null 2>&1 || true
    pkill -f 'kit/bin/agent|traffic.sh|serve.py|watch.py|logtally.py' 2>/dev/null || true
    rm -rf "${d:?}" "$HOME/work" "$kit"/go* /tmp/review.log
    mkdir -m 700 -p "$d" "$d/run" "$d/state"
    cp "$kit/init.lua" "$d/init.lua"
    mkdir -p "$HOME/work"
    cp -R "$kit/projects/." "$HOME/work/"
    git_history "$HOME/work/status-page"
    cp "$kit/zshrc" "$HOME/.zshrc"
    printf 'export PATH=%s\n' "$PATH" >"$HOME/.zshenv"
    ssh devbox 'rm -rf ~/work; find ~/kit -maxdepth 1 -name "go*" -delete; mkdir -p ~/work && cp -R ~/kit/projects/. ~/work/ && cp ~/kit/zshrc ~/.zshrc'
    echo "reset: clean" ;;

  prefs) # once per VM, safe to repeat
    if [ "$(defaults read com.apple.WindowManager StandardHideWidgets 2>/dev/null)" != 1 ]; then
      defaults write com.apple.WindowManager StandardHideWidgets -bool true
      defaults write com.apple.WindowManager StageManagerHideWidgets -bool true
      killall WindowManager 2>/dev/null || true
    fi
    # macOS's "See what's new" tip is a notification banner over the take
    launchctl disable "gui/$(id -u)/com.apple.tipsd" 2>/dev/null || true
    killall tipsd 2>/dev/null || true
    # the tailnet's devbox is this VM's admin account, by real ssh with the devbox key
    grep -q 'devbox.example-tailnet.ts.net' ~/.ssh/config || cat >>~/.ssh/config <<'EOF'

Host devbox.example-tailnet.ts.net
  HostName 127.0.0.1
  User admin
  IdentityFile ~/.ssh/devbox
  StrictHostKeyChecking accept-new
EOF
    mkdir -p "$HOME/logs"
    [ -s "$HOME/logs/app.log" ] || python3 "$kit/bin/mklog.py" "$HOME/logs/app.log" 400 ;;

  stage)
    leon server start --restore=false >/dev/null
    # the devbox's server and agent, over the plain ssh alias: leon's host list stays empty until
    # the take adds devbox from the tailnet
    leon -H ssh://devbox new nightly >/dev/null
    leon -H ssh://devbox -s nightly run --keep-open -c /Users/admin/work/logtally -- /Users/admin/kit/bin/agent /Users/admin/kit/scenes/devbox.scene >/dev/null
    leon -H ssh://devbox -s nightly window close 1 >/dev/null
    "$0" app
    # the app makes a session when there is none: wait for it, then end it, for the empty state
    ids=
    for _ in $(seq 100); do
      ids=$(leon ls --json | python3 -c 'import json,sys; print(" ".join(s["session_id"] for s in json.load(sys.stdin)))')
      [ -n "$ids" ] && break
      sleep 0.1
    done
    [ -n "$ids" ] || { echo "stage: the app made no session in 10 s" >&2; exit 1; }
    leon kill $ids >/dev/null
    touch "$kit/go"; ssh devbox 'touch ~/kit/go' ;;

  build)
    w=$HOME/work
    tab() { # tab SESSION DIR CMD...: a tab running CMD; prints its block id
      local s=$1 dir=$2; shift 2
      leon -s "$s" run --keep-open -c "$dir" -- "$@"
    }
    case "$2" in
      1) s=$(leon ls --json | python3 -c 'import json,sys; print(json.load(sys.stdin)[0]["session_id"])')
         leon session rename "$s" shelf-api >/dev/null
         tab "$s" "$w/shelf-api" sh -c 'printf "\033]2;dev server :8080\007"; (sleep 1; exec ./traffic.sh 8080) & exec python3 -u serve.py 8080' >/dev/null ;;
      2) leon new logtally -c "$w/logtally" >/dev/null
         tab logtally "$w/logtally" agent >/dev/null
         leon -s logtally window close 1 >/dev/null ;;
      3) leon new status-page -c "$w/status-page" >/dev/null
         sp=$(tab status-page "$w/status-page" agent)
         leon -s status-page split -b "$sp" --split below --ratio 30 --focus=false -c "$w/status-page" -- sh -c 'printf "\033]2;watch :5173\007"; exec python3 -u watch.py' >/dev/null
         leon -s status-page window close 1 >/dev/null ;;
    esac ;;

  untabs) leon kill ci workers exports nightly-jobs >/dev/null ;;

  tabs) # 44 tabs in 4 sessions: the overview's 50
    for s in ci workers exports nightly-jobs; do
      leon new "$s" >/dev/null
      for i in $(seq 11); do leon -s "$s" run -- ticker "$s-$i" >/dev/null; done
      leon -s "$s" window close 1 >/dev/null
    done ;;

  app)
    # the same windowed frame every launch (display 1 is the VM's screen), so the green button
    # that makes it full screen is always at 247,85 pt
    python3 -c '
import json, os, sys
p = sys.argv[1]
s = json.load(open(p)) if os.path.exists(p) else {"tab_style": "sidebar", "pane_headers": True, "sidebar_width": 250}
s.update(window={"x": 180, "y": 65, "w": 1080, "h": 680}, display=1)
json.dump(s, open(p, "w"))' "$d/state/app.json"
    open --env LEON_RUNTIME_DIR="$LEON_RUNTIME_DIR" --env LEON_STATE_DIR="$LEON_STATE_DIR" \
      --env LEON_CONFIG="$LEON_CONFIG" --env LEON_TAILSCALE="$LEON_TAILSCALE" --env PATH="$PATH" \
      --env SHELL=/bin/zsh "$app"
    for _ in $(seq 50); do [ -n "$(app_pid)" ] && break; sleep 0.1; done
    sleep 2
    keypost park
    echo "app: pid $(app_pid)" ;;

  fullscreen) keypost click 247 85 ;; # the window's green button (⌃⌘F does nothing in leon)

  key) shift; keypost "$@" ;;
  l) shift; leon "$@" ;;
  quit) p=$(app_pid); [ -n "$p" ] && kill $p; echo "app: quit" ;;

  go)
    if [ -n "${2:-}" ]; then touch "$kit/go.$2"; else touch "$kit/go"; ssh devbox 'touch ~/kit/go'; fi ;;

  answer) b=$(leon -s logtally block ls --json | python3 -c 'import json,sys; print(json.load(sys.stdin)[0]["block_id"])')
    leon -s logtally send -b "$b" "$2"; leon -s logtally send-key -b "$b" enter ;;

  check)
    t=$(mktemp -d)
    cp -R "$kit/projects/." "$t/"
    rc=0
    for p in shelf-api logtally status-page; do
      if (cd "$t/$p" && python3 runtests.py >/dev/null); then echo "check: $p ok"; else echo "check: $p FAILED"; rc=1; fi
      for n in $(ls "$t/$p/.next"); do
        cp -R "$t/$p/.next/$n/." "$t/$p/"
        # status-page's stage 1 is its tests alone: red on purpose
        want=0; [ "$p/$n" = status-page/1 ] && want=1
        if (cd "$t/$p" && python3 runtests.py >/dev/null); then got=0; else got=1; fi
        if [ $got -eq $want ]; then echo "check: $p after .next/$n ok"; else echo "check: $p after .next/$n FAILED"; rc=1; fi
      done
    done
    rm -rf "${t:?}"
    exit $rc ;;

  *) sed -n '2,16s/^# \{0,1\}//p' "$0"; exit 2 ;;
esac

-- The launch demo's leon config: big enough to read on a phone, no system banners in the shot.
leon.set("theme", "leon-dark")
leon.set("font.size", 15)
leon.set("notifications.done", false)
leon.set("notifications.blocked", false)
leon.set("notifications.error", false)
leon.set("check_for_updates", false)

-- leon do review [project=shelf-api]: a review tab for a project: its tests on the left, its dev
-- server top right, and a live tally of that server's log under it.
local function term(cwd, argv, label)
  return leon.layout.block{ label = label, options = { cwd = cwd, command = argv } }
end
leon.action{
  name = "review",
  title = "Open a review tab: tests, dev server, live log tally",
  args = { project = "string?" },
  run = function(ctx, args)
    local home = os.getenv("HOME")
    local dir = home .. "/work/" .. (args.project or "shelf-api")
    local tally = home .. "/work/logtally"
    local w = assert(leon.session.new_window{
      window_label = "review",
      layout = term(dir, { "sh", "-c", "python3 runtests.py; exec zsh -l" }, "review-tests"),
    })
    local srv = assert(leon.session.new_split{
      anchor_id = w.block_id, direction = "horizontal", side = "after", ratio = 0.5,
      layout = term(dir, { "sh", "-c", "(sleep 1; exec ./traffic.sh 8081) & python3 -u serve.py 8081 | tee -a /tmp/review.log" }, "review-server"),
    })
    assert(leon.session.new_split{
      anchor_id = srv.block_id, direction = "vertical", side = "after", ratio = 0.45,
      layout = term(tally, { "sh", "-c", "touch /tmp/review.log; exec python3 logtally.py /tmp/review.log --follow --top 4" }, "review-tally"),
    })
    return w.window_id
  end,
}

-- ctrl+alt+g: the repo's history in a popup over the focused pane; the same key hides it.
-- docs/lua/keys.md's recipe ("A popup on a key"), as it is there.
leon.action{
  name = "git_popup",
  title = "Toggle git popup",
  run = function()
    local result, err = leon.toggle{
      label = "git", create = true, float = true,
      options = { command = { "sh", "-c", "git log --graph --oneline --decorate --color=always | less -R" } },
    }
    if not result then error(err, 0) end
    return result
  end,
}
leon.bind("ctrl+alt+g", "git_popup")

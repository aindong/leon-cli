-- A typical config: actions, modes, sequences.
leon.action{ name = "hello", title = "Say hello", run = function(ctx, args) return "hi" end }
leon.bind("cmd+t", "client.tab.new")
leon.bind("ctrl+b>c", "session.new_window")
leon.bind("ctrl+b>%", "pane.split", { direction = "right" })
leon.bind("cmd+h", "hello")
leon.mode("resize", { exclusive = true })
leon.bind("resize/left", "pane.resize", { direction = "left" })
leon.bind("resize/right", "pane.resize", { direction = "right" })
leon.bind("cmd+r", "client.mode.enter", { name = "resize" })

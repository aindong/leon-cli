leon.bind("cmd+t", "client.tab.new")
leon.bind("hyper+t", "client.tab.new")
leon.action{ name = "Bad.Name", title = "x", run = function() end }
leon.bind("resize/left", function() end)
leon.action{ name = "ok_action", title = "OK", run = function() end, args = { n = "float" } }
leon.bind("cmd+w", "client.tab.close")

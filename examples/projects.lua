-- leon do ./examples/projects.lua dir=. [worktrees=true]
-- Prints one session ID per line; pass a single ID to leon attach to switch to it.
local project = require("project")
for _, id in ipairs(project.open(leon.args)) do print(id) end

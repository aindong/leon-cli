-- Load with require("project") from init.lua, or use projects.lua with leon do.
-- Local recipe: git and filesystem operations run on the machine executing this Lua.
local project = {}

local function call(method, params)
  local result, err = leon.call(method, params)
  if not result then error(err, 0) end
  return result
end

local function exec(argv, cwd)
  local result = leon.exec(argv, { cwd = cwd, timeout = 5 })
  if not result.ok then error(result.stderr ~= "" and result.stderr or "command failed: " .. argv[1], 0) end
  return result.stdout
end

local function directory(path)
  assert(type(path) == "string" and path ~= "", "dir must name an existing directory")
  -- A final newline belongs to pwd; any earlier newlines belong to the path.
  return (exec({ "pwd", "-P" }, path):gsub("\n$", ""))
end

-- Hook called only before creating a session. Replace this function in your init.lua to
-- customize setup. The default copies only named, ignored, regular files at the root.
function project.before_create(dir, args)
  local files = args.copy_files or {}
  assert(type(files) == "table", "copy_files must be an array of filenames")
  if #files == 0 then return end
  assert(args.copy_from, "copy_files needs copy_from")
  local source = directory(args.copy_from)
  if source == dir then return end
  for _, name in ipairs(files) do
    assert(type(name) == "string" and name ~= "" and name ~= "." and name ~= ".."
      and not name:find("[/%z]"), "copy_files accepts root filenames, without / or ..")
    exec({ "git", "check-ignore", "--quiet", "--", name }, source)
    -- Paths are positional arguments, never shell source. noclobber and umask protect
    -- existing files and keep a copied .env private. Symlinks are deliberately refused.
    exec({ "sh", "-c", [[
      set -eu
      src=$1/$3 dst=$2/$3
      if [ -L "$src" ] || [ ! -f "$src" ]; then
        echo 'copy source must be a regular file, not a symlink' >&2; exit 1
      fi
      if [ -L "$dst" ]; then echo 'copy destination is a symlink' >&2; exit 1; fi
      if [ -e "$dst" ]; then exit 0; fi
      umask 077
      set -C
      cat "$src" > "$dst"
    ]], "project-copy", source, dir, name })
  end
end

local function folder_label(dir)
  local label = assert(dir:match("([^/]+)$"), "the filesystem root has no project name")
  -- Match leon's label normalization; reject names its public API cannot represent.
  label = label:gsub("^%s+", ""):gsub("%s+$", "")
  assert(label ~= "" and not label:find("%c"), "project folder needs a nonempty label without control characters")
  return label
end

local function open(dir, args)
  local label = folder_label(dir)
  local found
  for _, session in ipairs(call("session.list", {}).sessions) do
    if session.label == label then
      assert(not found, "multiple sessions named " .. label .. "; rename one before opening this project")
      found = session.session_id
    end
  end
  if found then return found end
  project.before_create(dir, args)
  -- Same public parameter as leon new --cwd; no project/worktree state in the core.
  return call("session.create", { label = label, cwd = dir }).session_id
end

function project.open(args)
  local dir = directory(args.dir or ".")
  local dirs = { dir }
  if args.worktrees then
    dirs = {}
    -- -z disables Git's quoting and uses NULs, so spaces, quotes and newlines survive.
    local listing = exec({ "git", "worktree", "list", "--porcelain", "-z" }, dir)
    -- leon.exec caps each stream at 16 MiB: fail rather than use a truncated list.
    assert(#listing < 16 * 1024 * 1024 and listing:sub(-2) == "\0\0", "incomplete git worktree list")
    for record in listing:gmatch("(.-)%z%z") do
      local path = record:match("^worktree ([^%z]+)%z")
      if path and not record:match("%zbare$") and not record:match("%zprunable") then
        dirs[#dirs + 1] = directory(path)
      end
    end
    assert(#dirs > 0, "no usable worktrees")
  end
  local ids, labels = {}, {}
  for _, path in ipairs(dirs) do
    local label = folder_label(path)
    assert(not labels[label], "worktrees share a folder name; rename one before opening them")
    labels[label] = true
  end
  for _, path in ipairs(dirs) do ids[#ids + 1] = open(path, args) end
  return ids
end

leon.action{
  name = "project_open",
  title = "Open project sessions",
  args = { dir = "string", worktrees = "boolean?", copy_from = "string?", copy_files = "array?" },
  run = function(_, args)
    assert(args.dir:sub(1, 1) == "/", "project_open action needs an absolute dir")
    return project.open(args)
  end,
}
return project

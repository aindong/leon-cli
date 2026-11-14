# Projects and git worktrees

[examples/project.lua](../examples/project.lua) defines a `project_open` Lua action.
[examples/projects.lua](../examples/projects.lua) runs the same recipe through `leon do`.
Both find a session whose label matches the project folder's name, or create one with its first
terminal in that directory. Creation uses `session.create`'s `cwd`, the same parameter as
`leon new --cwd DIR`. Project and worktree policy stays in these examples; copy and customize them.

The examples run locally: use a local server and local folders. Script paths resolve against the
caller's directory; actions require an absolute `dir`, since actions run in the server. Session
names are global: an existing folder-named session is reused even if its shell has since changed
directory. Duplicate session labels or worktrees with the same folder name are errors; rename
them first. Folder names must fit leon's labels (128 characters, no control characters).

## A project

From a checkout of leon, with `leon` on PATH:

```sh
leon do ./examples/projects.lua dir="$PWD"
```

The script prints the session ID. Run `leon attach SESSION_ID` to open it in your terminal, or
select that session in the app. Repeating the script prints the same ID and creates no extra
terminal. You can pass another project with `dir=/path/to/project` or `dir=../project`.
For a new session without the recipe, use `leon new project-name --cwd /path/to/project`.

## Every worktree

```sh
leon do ./examples/projects.lua dir="$PWD" worktrees=true
```

The recipe reads `git worktree list --porcelain -z` and prints one session ID per usable worktree,
including the main checkout. NUL delimiters preserve spaces, quotes and newlines in parent paths.
Bare repositories and worktrees marked prunable are skipped. It doesn't create or remove worktrees;
use `git worktree add` and `git worktree remove` yourself.

## Copy an ignored file into a new worktree

After creating a worktree, opt into copying specific ignored files before its session is created.
Here `.env` must be an ignored regular file at the source root:

```sh
leon do ./examples/projects.lua dir="$PWD" worktrees=true \
  copy_from="$PWD" 'copy_files=[".env"]'
```

Only sessions being created run this hook. Existing destination files stay untouched; new copies
are private (mode 0600, subject to the filesystem). Names must be root filenames, without `/` or
`..`; symlinks and files Git doesn't consider ignored are refused. No ignored files are copied by
default. `copy_from` is the source checkout, and `copy_files` is a JSON array of names. A failed hook
stops the recipe before creating that session; already created sessions and copies remain. Run
one recipe at a time: listing and creating sessions are separate API calls.

## Install the action

Copy `examples/project.lua` beside your `init.lua`, then add this config (change the bound folder
to your own absolute project path):

```lua
require("project")
leon.bind("cmd+shift+o", "project_open", { dir = "/path/to/project" })
```

Reload with `leon config reload`. `leon do project_open dir=/path/to/project` returns an array of
session IDs; select one with `leon attach SESSION_ID` or the app. The binding creates or finds the
sessions and leaves client selection to you. The action accepts the same `worktrees`, `copy_from`
and `copy_files` arguments as the script.

The module exposes `before_create(dir, args)` as the setup hook. Replace it after `require`, or wrap
it to keep the default ignored-file copy. A hook should fail with `error(message)` when setup fails;
it runs before `session.create`, and an action has leon's usual 30-second deadline.

Every `sh` and `lua` block on this page runs in `m21_recipe_docs`; `m21_recipe` checks the real
script and action against a private server, including two git worktrees and ignored-file copying.

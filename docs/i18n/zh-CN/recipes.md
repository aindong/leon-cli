<!-- i18n: source=docs/recipes.md commit=a525beb0ae17 -->
> 本页是译文，以[英文原文](../../recipes.md)为准，英文版可能更新。

# 项目与 git worktree

[examples/project.lua](../../../examples/project.lua) 定义了一个 `project_open` Lua 动作。[examples/projects.lua](../../../examples/projects.lua) 通过 `leon do` 运行同一个配方。两者都会查找标签与项目文件夹名称相同的会话，找不到就创建一个，其第一个终端位于该目录中。创建时使用 `session.create` 的 `cwd`，与 `leon new --cwd DIR` 是同一个参数。项目和 worktree 的策略只放在这些示例中；复制后按需修改即可。

这些示例在本地运行：请使用本地服务器和本地文件夹。脚本路径相对于调用者的目录解析；动作要求 `dir` 为绝对路径，因为动作在服务器中运行。会话名称是全局的：已存在的同名会话会被复用，即使它的 shell 之后切换了目录。重复的会话标签，或文件夹名称相同的多个 worktree 都会报错；请先重命名。文件夹名称必须符合 leon 的标签规则（128 个字符以内，不含控制字符）。

## 单个项目

在 leon 的检出目录中，`leon` 在 PATH 上时：

```sh
leon do ./examples/projects.lua dir="$PWD"
```

脚本会打印会话 ID。运行 `leon attach SESSION_ID` 在终端中打开它，或在应用中选择该会话。重复运行脚本会打印同一个 ID，不会多创建终端。可以用 `dir=/path/to/project` 或 `dir=../project` 传入其他项目。不使用配方新建会话，用 `leon new project-name --cwd /path/to/project`。

## 每个 worktree

```sh
leon do ./examples/projects.lua dir="$PWD" worktrees=true
```

配方会读取 `git worktree list --porcelain -z`，并为每个可用的 worktree（包括主检出目录）打印一个会话 ID。NUL 分隔符可以保留父路径中的空格、引号和换行符。裸仓库和标记为 prunable 的 worktree 会被跳过。它不会创建或删除 worktree；请自己使用 `git worktree add` 和 `git worktree remove`。

## 把被忽略的文件复制到新 worktree

创建 worktree 后，可以选择在其会话创建之前复制指定的被忽略文件。这里 `.env` 必须是源根目录下一个被忽略的普通文件：

```sh
leon do ./examples/projects.lua dir="$PWD" worktrees=true \
  copy_from="$PWD" 'copy_files=[".env"]'
```

只有正在创建的会话会运行这个钩子。目标位置已有的文件不会被改动；新的副本是私有的（权限 0600，取决于文件系统）。名称必须是根目录下的文件名，不能包含 `/` 或 `..`；符号链接以及 Git 不认为是被忽略的文件会被拒绝。默认不复制任何被忽略的文件。`copy_from` 是源检出目录，`copy_files` 是名称的 JSON 数组。钩子失败时，配方会在创建该会话之前停止；已经创建的会话和副本会保留。一次只运行一个配方：列出会话和创建会话是两次独立的 API 调用。

## 安装动作

把 `examples/project.lua` 复制到你的 `init.lua` 旁边，然后加入以下配置（把绑定的文件夹改成你自己项目的绝对路径）：

```lua
require("project")
leon.bind("cmd+shift+o", "project_open", { dir = "/path/to/project" })
```

用 `leon config reload` 重新加载。`leon do project_open dir=/path/to/project` 会返回会话 ID 的数组；用 `leon attach SESSION_ID` 或应用选择其中一个。这个绑定只负责创建或查找会话，选择哪个客户端由你决定。该动作接受与脚本相同的 `worktrees`、`copy_from` 和 `copy_files` 参数。

模块把 `before_create(dir, args)` 作为准备钩子公开。可以在 `require` 之后替换它，或者包装它，以保留默认行为（复制被忽略的文件）。准备失败时，钩子应该用 `error(message)` 报错；它在 `session.create` 之前运行，而动作有 leon 惯常的 30 秒期限。

本页的每个 `sh` 和 `lua` 代码块都会在 `m21_recipe_docs` 中运行；`m21_recipe` 会在一个私有服务器上检查真实的脚本和动作，包括两个 git worktree 和被忽略文件的复制。

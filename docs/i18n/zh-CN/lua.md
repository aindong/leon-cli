<!-- i18n: source=docs/lua.md commit=a525beb0ae17 -->
> 本页是译文，以[英文原文](../../lua.md)为准，英文版可能更新。

# Lua：配置与脚本

leon 在两个地方使用 Lua 5.1：

- **配置**，即 `init.lua`，由服务器在启动时加载：按键绑定、模式和自定义动作。用 `leon config check` 检查；用 `leon config reload` 应用更改。
- **脚本**，由 `leon do` 在你的机器上针对服务器运行：一次性的自动化，或者响应事件的长期运行监听器。

两者共享 `leon.*` 函数（[函数列表](../../lua/scripts.md#function-list)）。这些页面上的每个示例都由测试运行（`crates/leon/tests/m7b_lua_doc.rs`）。

## 配置文件在哪里

服务器读取以下位置中第一个存在的：`server run --config PATH`、`$LEON_CONFIG`、`$XDG_CONFIG_HOME/leon/init.lua`、`~/.config/leon/init.lua`。文件不存在时视为空配置。`require` 会加载它旁边的文件。

```sh
leon config check
leon config reload
leon keymap
leon actions
```

配置在每次加载时从上到下执行。错误的 `leon.*` 调用会被跳过，并给出 `file:line` 消息，其余部分照常加载；语法错误会让整个加载失败，服务器保留上一份正确的配置。这两种情况下 `leon config check` 都以 1 退出。

## 各个页面

- [按键与模式](../../lua/keys.md)：`leon.bind`、按键序列、重复键、tmux 风格预设、vim-navigator、模式与 `leon attach` 的前缀、函数绑定、浮动窗格和临时 shell 按键。
- [自定义动作](../../lua/actions.md)：`leon.action`、`leon.session.<method>` 和 `leon.call`。
- [设置](../../lua/settings.md)：每个 `leon.set` 设置项、它的取值和默认值。
- [脚本与函数](../../lua/scripts.md)：`leon do`、事件、定时器、`leon.exec`、`leon.*` 函数列表以及各项限制。

## 项目会话

见[项目与 git worktree](recipes.md)，其中有一个示例动作和 `leon do` 脚本，按文件夹复用会话，并可选地为 worktree 创建会话、提供被忽略文件的准备钩子。

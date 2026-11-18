<!-- i18n: source=docs/cli.md commit=14af788efc95 -->
> 本页是译文，以[英文原文](../../cli.md)为准，英文版可能更新。

# leon 命令参考

所有 `leon` 命令和参数。`leon <command> --help` 会打印同样内容的简版；如果本页（英文原文）及其在 `docs/cli/` 中的分页漏掉了 `--help` 中出现的命令或参数，测试（`crates/leon/tests/m7b_cli_doc.rs`）就会失败。

## 命令如何找到对象

leon 保存**会话**；一个会话包含若干**窗口**（标签页）；一个窗口以分屏布局包含若干**块**（终端窗格）。每个对象都有一个稳定的 ID，例如 `block:91c0…`。

- **目标。** `-s` 选择会话，`-b` 选择块。目标可以是完整 ID、标签（不区分大小写）、从 1 开始的位置（窗口按标签页顺序计数），或 ID 的最后 4 个或更多字符。如果一个目标匹配到多个对象，leon 会列出所有匹配项并以 3 退出。
- **默认值。** 在 leon 终端内，`LEON_SESSION` 和 `LEON_BLOCK` 指向你自己的会话和块，因此命令不加参数就作用于“这里”。你的会话是当前包含 `LEON_BLOCK` 的那个（窗格可以加入另一个会话），否则就是 `LEON_SESSION`。在 leon 外，请传入 `-s`/`-b`；此时会话级命令作用于该会话活动窗口中获得焦点的块。
- **服务器。** 任何需要服务器的命令，在没有服务器运行时都会启动一个个人服务器（`--autostart=false` 或 `LEON_AUTOSTART=0` 可以关闭这一行为）。socket 位于 `$LEON_RUNTIME_DIR`，否则是 `$XDG_RUNTIME_DIR/leon`，再否则是 `~/.local/state/leon/run`；它的路径必须符合 Unix socket 的长度限制（macOS 上小于 104 字节，Linux 上小于 108 字节），否则 leon 会说明并停止。
- **另一台机器。** `-H HOST` 会通过你自己的 `ssh`，把任何客户端命令发送到另一台机器上的 leon 服务器（见 [cli/hosts.md](../../cli/hosts.md)）。

## 全局参数

这些参数适用于每条命令。

| 参数 | 作用 |
|---|---|
| `--autostart[=true\|false]` | 没有服务器运行时启动一个个人服务器（默认 true；环境变量 `LEON_AUTOSTART`）。 |
| `--json` | 输出 JSON 而不是文本。 |
| `-s, --session SESSION` | 目标会话（默认 `$LEON_SESSION`）。 |
| `-b, --block BLOCK` | 目标块（默认 `$LEON_BLOCK`，否则为活动窗口中获得焦点的块）。 |
| `-H, --host HOST` | 通过 ssh 操作另一台机器的 leon 服务器：`leon hosts` 中的标签，或 `ssh://[USER@]HOST[:PORT]`。也可写作 `-S, --server`。使用它时，`$LEON_SESSION` 和 `$LEON_BLOCK` 会被忽略（它们指向本机的对象）。 |
| `-h, --help` | 打印帮助。 |
| `-V, --version` | 打印版本。 |

## 退出码

| 代码 | 含义 |
|---|---|
| 0 | 成功 |
| 1 | 失败 |
| 2 | 命令行有误（`leon` 遇到拼写错误时会建议最接近的命令） |
| 3 | 找不到目标或目标不明确 |
| 4 | 无法连接服务器 |
| 10 | `self-update --check`：有更新的发布版 |
| 130 | 被中断（Ctrl-C） |

`leon wait` 和 `leon run --wait` 则以程序自身的状态码退出。

## 应用

### `leon app`

打开 leon 应用并连接到服务器：已经在运行的那个（由 CLI 启动的也可以），或者像 `leon server start` 那样先启动一个。CLI 和智能体操控的就是你看到的这些会话。不带命令的 `leon` 效果相同。退出应用（⌘Q、关闭窗口）只是断开连接，所有程序继续运行；应用的“退出并结束所有会话（Quit and End All Sessions…）”或 `leon server stop` 会结束它们。使用 `--autostart=false` 且没有服务器时，它以 4 退出。

不含应用的构建（目前的 Linux，或使用 `--no-default-features` 的 macOS）没有窗口：当 stdin 和 stdout 都是终端时，单独的 `leon` 会运行 `leon attach`（带上给出的 `-s` 和 `-H`），否则打印帮助并以 0 退出（如果给出了 `-s` 或 `-H` 则以 2 退出）；`leon app` 会说明原因并以 1 退出。

## 在终端中

### `leon attach [TARGET]`

在任意终端中使用 leon，本地或通过 ssh 都可以（`ssh -t HOST leon attach`；不加 `-t` 就没有终端，它会以 1 退出）。不带 TARGET（也没有 `-s`）时，它会打开一个选择器，列出所有会话、它们的标签页及其中的块，并实时预览所选块的画面：输入以筛选（按顺序匹配字母，与命令面板的匹配方式相同），↑ 和 ↓（或 ctrl+p、ctrl+n）移动，Enter 连接，Esc 返回或退出。TARGET 可以是一个会话，也可以是一个块，后者会打开它所在的标签页并让它获得焦点。一个会话都没有时，按 Enter 会新建一个。

连接后，它会显示会话的活动标签页：分屏布局中的每个窗格，窗格之间有边框，还有一个显示会话及其标签页的状态栏。按键会发送到获得焦点的窗格，但前缀键和你配置中的绑定除外（`init.lua`；`cmd` 键永远不会从终端传来）。前缀键（`--prefix KEY`，否则为配置中的 `leon.mode("attach", { prefix = … })`，再否则为 ctrl+b）之后：

| 按键 | 作用 |
|---|---|
| `d` | 断开：`leon attach` 以 0 退出，所有程序继续运行 |
| ←↑↓→ 或 `h` `j` `k` `l` | 将焦点移到该方向的窗格 |
| `%`, `"` | 向右、向下拆分窗格 |
| `z` | 放大窗格（再按一次还原） |
| `c` | 新建标签页 |
| `n`, `p` | 下一个、上一个标签页 |
| `s` | 回到选择器 |
| 再按一次前缀键 | 把前缀键输入到窗格中 |

每当这个终端自己对窗格的布局发生变化（连接、调整大小、拆分或放大、某个窗格重新显示）时，窗格都会调整到适合这个终端的大小；此后被其他客户端调整过大小的窗格会被裁剪或填充。退出时（断开、服务器停止、SIGHUP、SIGTERM、SIGINT、SIGQUIT），终端会恢复原样；即使遇到 SIGKILL，也有一个看门狗进程负责恢复。被信号终止时，它以 128 + 信号值退出。

通过 `-H` 连接时，断开的连接在重试期间，状态栏会显示 `reconnecting to HOST (try N)…`，按键要么完整送达窗格，要么完全不送达。如果一秒内没有收到主机的任何消息，输入的按键会先等待一次快速的连接检查再发出；检查失败或连接正在重连时，按键会被丢弃，状态栏会显示有多少按键没有发出。连接恢复后请重新输入。粘贴的内容会和按键一起排队。有一个限制：如果卡顿恰好在那一秒内、在输入一个单词的中途开始，已经在路上的按键仍可能受影响，并可能延迟到达。

## 会话与窗口

`leon new`、`ls`、`kill`、`leon session` 和 `leon window` 见 [cli/sessions.md](../../cli/sessions.md)。

## 运行程序

### `leon run [FLAGS] -- CMD…`

在新窗口中运行一条命令，并打印新块的 ID。命令直接运行，不经过 shell；需要管道或 `&&` 时，用 `leon run -- sh -c '…'`。即使服务器是由从访达（Finder）打开的 Leon.app 启动的，命令也会在你登录 shell 的 PATH 上查找。

- `--label LABEL`：为块命名，以便用 `-b LABEL` 找到它。
- `--keep-open`：程序退出后保留该块，以便仍能读取它的输出。
- `--wait`：等待程序结束，并以它的状态码退出。
- `--output[=true|false]`：与 `--wait` 一起使用时，在程序退出后，于 ID 行之后打印程序的文本：即 `leon capture --trim` 会显示的内容，包括回滚缓冲区和屏幕。stdout 不是终端时（智能体或脚本的 `$(…)`）默认开启，在终端上默认关闭。为了读取文本，`run` 会保留该块直到文本被读取，然后关闭它（除非使用 `--keep-open`）；在此之前被终止的 `run --wait` 会像 `--keep-open` 那样保留它的块。
- `-c, --cwd DIR`：工作目录（默认：当前目录；使用 `-H` 时为主机上的家目录，且 `DIR` 必须是该主机上的绝对路径）。

```sh
leon run --label tests --keep-open -- cargo test
leon run --wait -- make                       # in a script: make's output, then its status
leon run --wait --output=false -- make        # the status alone
```

### `leon split [FLAGS] [-- CMD…]`

拆分一个块，并在新窗格中运行 `CMD`（默认：登录 shell）；打印新块的 ID。它拆分 `-b` 指定的块（也可以指定一个分屏，把新窗格放在整个组的旁边），否则拆分获得焦点的块。

- `--split right|left|above|below`：新窗格的位置（默认 right）。
- `--ratio PCT`：新窗格占用的空间比例，1–99（默认 50）。
- `--label LABEL`、`--keep-open`、`-c, --cwd DIR`：与 `leon run` 相同。
- `--focus[=true|false]`：让新窗格获得焦点（默认 true）。

```sh
leon split
leon split --split below --ratio 30 --label logs -- tail -f /var/log/system.log
leon split --focus=false -b tests -- htop
```

### `leon send [TEXT]`

把 `TEXT` 输入到终端（默认：读取 stdin）。不会追加 Enter；之后接 `leon send-key enter`。

### `leon send-key KEY…`

按顺序按下按键，按键写法与绑定语法相同：`enter`、`escape`、`tab`、`space`、`backspace`、`up`、`ctrl+c`、`alt+left`、`a`…

### `leon capture`

把终端的回滚缓冲区和屏幕内容以文本形式打印出来。

- `--trim`：去掉行尾空格和末尾的空行。

### `leon wait [BLOCK]`

等待终端中的程序退出，并以它的状态码退出（默认块：`-b`，否则为 `$LEON_BLOCK`）。

- `--for DURATION`：超过这个时间就放弃（`500ms`、`10s`、`2m`），并以 1 退出。
- `--text TEXT`：改为等待块的屏幕或回滚缓冲区中出现 `TEXT`，然后以 0 退出（已经存在的文本也算）。超过 `--for` 或块先关闭时以 1 退出。适用于持续运行的程序：提示符、开发服务器的“ready”行。

```sh
id=$(leon run --keep-open -- python3)
leon wait -b "$id" --text '>>>' --for 10s    # Python's prompt: now it reads what you type
leon send -b "$id" 'print(6 * 7)'
leon send-key -b "$id" enter
leon capture -b "$id" --trim
leon send-key -b "$id" ctrl+d
leon wait -b "$id" --for 10s
```

### `leon notify`

为你自己的块（`-b`，否则为 `$LEON_BLOCK`；两者都没有时以 2 退出）报告一个状态，效果与其程序发出的 OSC 7501 报告相同：它会显示在标签页和窗格标题栏上、应用的活动列表（Activity）中，并且对于阻塞、错误以及长时间运行的完成状态，还会发送通知。适用于无法写出 OSC 7501 的工具（钩子、没有 tty 的任务）。它会替换该块的根记录，并沿用该记录的生命周期。

- `--state working|blocked|done|error`：状态（默认 `done`）。
- `--title T`、`--body B`：各一行（最多 192 和 2048 字节）。OSC 7501 会丢弃的文本（控制字符、换行、不可见字符）会被拒绝（以 1 退出），并且不会记录任何内容。

```sh
id=$(leon run --keep-open -- sleep 30)
leon notify -b "$id" --state blocked --title "Deploy" --body "approve the rollout?"
leon block call -b "$id" program_status
leon block close "$id"
```

## 布局

`leon focus`、`move`、`swap`、`zoom` 和 `resize` 见 [cli/layout.md](../../cli/layout.md)。

## 块

### `leon block`

管理块（终端窗格）。

### `leon block ls`

列出一个会话中的块。

### `leon block inspect [BLOCK]`

显示一个块：它的窗口、pid、命令和退出状态，以及程序注册过的恢复命令（`set_resume`）。

### `leon block close [BLOCK]`

关闭一个块并结束其中的程序。

### `leon block call METHOD [ARGS]`

调用一个块方法，例如 `size` 或 `program_status`，也可以用完整名称，例如 `dev.leon.terminal.size`。`ARGS` 是一个 JSON 对象、`@file`，或表示 stdin 的 `-`。

`set_resume` 注册在重启后恢复这个块中程序的命令（`null` 表示清除）；恢复后的块会按照配置中的 `resume` 设置提供它（见 [lua/settings.md](../../lua/settings.md)）：

```sh
leon block call set_resume '{"command": ["claude", "--resume", "c7583d2c-3133-4d13-b5f0-45883f1cabcb"]}'
```

## 自动化

`leon events`、`leon do`、`leon actions`、`leon keymap` 和 `leon api` 见 [cli/automation.md](../../cli/automation.md)。

## 配置与主题

`leon config`（check、reload、get、set）和 `leon theme`（list、import、show）见 [cli/settings.md](../../cli/settings.md)。

## 服务器

`leon server`（run、start、stop、status、proxy）见 [cli/server.md](../../cli/server.md)。

## 主机

`-H HOST` 通过系统的 `ssh` 连接另一台机器上的 leon 服务器。`leon hosts`（list、add、remove、check；`--tailscale` 从你的 tailnet 添加一台机器）以及 `-H` 的工作方式见 [cli/hosts.md](../../cli/hosts.md)。

## 更新

### `leon self-update`

把 leon 更新到最新的发布版；`--check` 只报告（以 10 退出）：[updates.md](../../updates.md)。

## 智能体

`leon skill`（show、setup、status、remove）、`leon float`、`leon toggle` 和 `leon dock` 见 [cli/agents.md](../../cli/agents.md)。

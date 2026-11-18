<!-- i18n: source=docs/agents.md commit=a525beb0ae17 -->
> 本页是译文，以[英文原文](../../agents.md)为准，英文版可能更新。

# 让智能体操控 leon

在 leon 终端中运行的编程智能体（Claude Code、Codex）可以做你在那里能做的事：在自己旁边打开一个窗格、在里面运行测试、等待测试结束、读取屏幕、向 REPL 输入，并关闭自己打开的东西。它用的就是你用的那些 `leon` 命令。本页说明如何教会它，以及它学会之后应该怎样做。

## 一次性设置

```sh
leon skill setup --dry-run   # show what it would write
leon skill setup             # install for every agent it finds
leon skill status            # current, outdated, missing, foreign or unreadable
```

`leon skill setup` 会为它找到的每个智能体安装两样东西。找到的条件是：配置目录存在，或者程序（`claude`、`codex`）在你的 PATH 上，即使你从未运行过它。它会为每个智能体打印选中的原因以及写入的每个文件。如果一个都没找到，它只写入共享的 `~/.agents/skills/leon/`（其他智能体也会读取）并说明这一点。`--agent claude|codex` 只选其中一个：

| | Claude Code | Codex |
|---|---|---|
| 技能（skill）：一个 `SKILL.md` 存根 | `~/.claude/skills/leon/`（`$CLAUDE_CONFIG_DIR`） | `~/.agents/skills/leon/` |
| 运行 `leon skill hook --agent …` 的会话启动钩子 | `~/.claude/settings.json` | `~/.codex/hooks.json`（`$CODEX_HOME`） |

- **技能**让智能体运行 `leon skill show`，它会打印编译进你的 `leon` 二进制文件中的完整指南。升级 leon 就会升级指南，无需重新安装。
- **钩子**在智能体会话启动时运行。在 leon 内，它告诉智能体自己所在的会话和终端块，并记录如何恢复该会话（见下文）；在 leon 外，它什么也不打印，也从不联系服务器。旧版 leon 留下的钩子状态为 `outdated`：运行 `leon skill setup` 即可。

安装可以安全地重复运行。它会原地编辑设置文件，保留你的键及其顺序，每个文件只备份一次到 `<file>.leon-backup`，并拒绝修改不符合预期 JSON 结构的文件。除非传入 `--force`，它不会替换不是由它写入的 `SKILL.md`（旧文件会保留为 `SKILL.md.leon-backup`）。

```sh
leon skill show                    # read the guide the agent reads
leon skill remove --dry-run
leon skill remove                  # removes what setup installed, and nothing else
```

## 我在哪里：`LEON_SESSION` 和 `LEON_BLOCK`

每个 leon 终端都有两个变量：

- `LEON_SESSION`：会话的 ID，例如 `session:3f2a…`。
- `LEON_BLOCK`：这个终端自己的 ID，例如 `block:91c0…`。

leon 命令默认使用它们，所以智能体的 `leon run` 不加任何参数，就会在*它自己的*会话中、在用户其他工作的旁边打开一个标签页。它自己的会话是指当前包含 `LEON_BLOCK` 的那个会话：移动到另一个会话中的窗格（在应用的切换器中按 ⌘↩）会保留启动时的 `LEON_SESSION`，因为运行中程序的环境无法改变，所以 leon 会查找该块，只有在 `LEON_BLOCK` 未设置或已不存在时才使用 `LEON_SESSION`。两个变量都没有设置时，说明智能体不在 leon 内：它应该运行 `leon ls`，并询问要使用哪个会话。

智能体应该如何理解用户的话：

- “这里”、“在你旁边”、“在这个会话里”：指它自己的会话，即包含 `LEON_BLOCK` 的那个。
- “测试那个窗格”、“api 会话”：用户给出的名字始终优先于默认值；用 `-b` 或 `-s` 传入。
- 如果一个名字匹配到多个对象，leon 会把它们列出来并以 3 退出：要去询问，不要猜。

**一个陷阱：** 不带 `-b` 的 `send`、`send-key` 和 `capture` 会作用于 `LEON_BLOCK`，也就是智能体自己的终端。往那里输入，就是往智能体自己的输入框里输入。务必用 `-b` 指定你想要的块。

## 从智能体的视角看一个会话

```sh
# open a pane beside me, run the tests there, keep it after they finish
id=$(leon split --label tests --keep-open -- cargo test)
leon wait "$id"; echo "tests exited $?"
leon capture -b "$id" --trim | tail -40

# drive a REPL
py=$(leon run --label py -- python3 -q)       # -q: no banner to match
leon wait -b "$py" --text '>>>' --for 10s     # its prompt, before typing
leon send -b "$py" 'print(6 * 7)'
leon send-key -b "$py" enter
leon wait -b "$py" --text 42 --for 10s
leon capture -b "$py" --trim | tail -3

# clean up what I created, and only that
leon block close "$id"
leon block close "$py"
```

指南给智能体的规则：

- 为你创建的每个块加上标签，让用户能分清哪些是你的、哪些是他们的。
- 用完后关闭你创建的东西；绝不关闭用户的块，也不关闭你自己的 `LEON_BLOCK`。
- 除非被要求，不要读取用户的终端或向其中输入：屏幕上可能有机密信息，你发送的按键会在他们的 shell 中执行。
- 对于持续运行的程序（开发服务器、REPL），不要等它退出：等待你期望出现的文本，`leon wait -b ID --text TEXT --for 30s`。
- 智能体运行 `leon run --wait -- CMD` 时，一次调用就会打印程序的输出，并以它的状态码退出。
- 需要解析的输出用 `--json`；命令覆盖不到的，用 `leon api list` / `leon api describe`。

## 重启后智能体会回来

服务器停止时（`leon server stop`、注销、重启电脑），每个块的文本都会被保存，恢复后的块会把这些文本显示在一行暗色的 `— restored from <time> —` 和一个新的 shell 之上。程序不会保留下来，保存的命令也绝不会被重新执行。能回来的是程序用 `set_resume` 为自己的块注册的命令。钩子会替你完成注册：Claude Code 用 `claude --resume <id>`，Codex 用 `codex resume <id>`，使用的是智能体传给钩子的会话 id。它只会为在终端前台运行的智能体记录，因此另一个智能体的工具嵌套运行的 `claude -p` 不会替换交互式会话的命令；它也从不为 Claude Code 的打印模式（`-p`、`--print`）记录：一次性运行没有可恢复的内容。

其他任何程序都可以注册自己的命令：

```sh
leon block call set_resume -b "$LEON_BLOCK" '{"command": ["my-agent", "--resume", "42"]}'
leon block inspect "$LEON_BLOCK"        # shows "resume: my-agent --resume 42"
leon block call set_resume -b "$LEON_BLOCK" '{"command": null}'
```

恢复时，块会遵循 `init.lua` 中的 `resume` 设置（`leon.set("resume", "ask")`）：`"ask"`（默认）在新的提示符处输入该命令并等待按 Enter（Ctrl-C 跳过），`"always"` 直接运行，`"never"` 忽略。一条命令只在一次重启时被提供：一旦输入就不会再次保存，因此被跳过或过期的命令不会每次都回来。恢复后的智能体的钩子会再次注册它。

## 程序状态：让用户看到智能体在做什么

智能体和工具可以用 OSC 7501 报告自己的状态（`working`、`blocked`、`done`、`error`），leon 会按终端跟踪这些状态。`leon block call program_status -b "$LEON_BLOCK"` 可以读回状态，每次变化都会成为 `leon events -s SESSION` 中的一个 `program_status_changed` 事件。

### OSC 7501 格式

一份报告是写到终端的一个转义序列：`ESC ] 7501 ;`，然后是用 `:` 连接的 `key=value` 对，最后是 `BEL`（或 `ESC \`）。键由小写字母组成；值只能使用字母、数字和 `_ . , + / = -`。`id` 最多由 8 段用 `/` 连接而成，每段 1 到 32 个字母、数字或 `_ . + -`。发送 `ESC ] 7501 ; ? BEL` 可以询问终端支持什么；leon 会回答 `ESC ] 7501 ; ? : states=idle,working,done,blocked,error : kinds=permission,question,auth ESC \`（不含空格）。

例如，`printf '\033]7501;state=working:progress=40\007'` 表示程序已完成 40%。

- **键。** `state`（必需）：`idle`（无可报告）、`working`、`done`、`blocked`、`error` 或 `clear`。`id` 指定一条记录（不指定即根记录；`/` 表示子记录）。`kind`（仅与 `blocked` 一起使用）：`permission`、`question` 或 `auth`。`progress`：0 到 100，与 `working` 或 `blocked` 一起使用（不指定表示未知）。`app`：`[A-Za-z0-9_.+-]{1,32}`，取自最近一个设置了它的祖先。`title` 和 `msg`：一行不含控制字符的 UTF-8 文本的 base64。
- **规则。** 空白会被去除，格式错误的键值对会被跳过，未知的键会被忽略，重复的键以最后一个为准。未知的 `state` 或错误的 `id` 会让整份报告被丢弃（错误的 `id` 绝不会退回到根记录）。每份报告都会整体替换它的记录；`clear` 会删除该记录及其子记录（不带 `id` 时删除所有记录）。
- **生命周期。** `working` 和 `blocked` 在进程退出或新提示符出现时（OSC 133 `A`）消失。`done` 和 `error` 会一直保留，直到你查看该窗格。完全重置（`RIS`）会清除所有记录；软重置和切换到备用屏幕不会。
- **限制。** 超过以下任何一项，整份报告都会被丢弃：序列 4096 字节，键 16，`msg` 编码后 2732（解码后 2048），`title` 256（192），`app` 32，`id` 总计 128，每段 32，最多 8 层。一个终端最多保留 256 条记录，并丢弃最久未使用的。

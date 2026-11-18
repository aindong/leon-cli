<!-- i18n: source=docs/remote-hosts.md commit=28ab3cd4a46d -->
> 本页是译文，以[英文原文](../../remote-hosts.md)为准，英文版可能更新。

# 远程主机

leon 如何与另一台机器上的 leon 服务器协同工作，承接 [README](README.md#在开发机上工作) 中的入门步骤。命令见 [cli/hosts.md](../../cli/hosts.md)。

断开的 Wi-Fi 连接会在一分钟内不断重试：`leon events` 和应用会恢复，既不丢失也不重复任何事件，`leon attach` 会提示这一点，然后重新连接并重绘。连接卡住或断开期间输入的按键要么完整送达，要么完全不送达，状态栏会显示有多少按键没有发出（卡顿刚开始时发出的按键可能会送达）。请使用 ssh 密钥或 ssh agent：每个连接都是一个独立的 ssh 会话。

应用会在侧边栏中“本机（This Mac）”下方显示 `leon hosts` 中的每台主机，每台都带有它的会话、标签页和一个表示连接状态的圆点。远程窗格的用法与本地窗格相同，标题栏中会显示主机名；新建会话（New Session）、新建标签页（New Tab）和拆分都作用于获得焦点的窗格所在的机器，命令面板中有每台机器的新建会话，以及添加主机（Add Host…）（列出你 tailnet 中的机器，或输入一个标签，再填写 `ssh://` 端点）。点击主机的标签可以把它折叠起来。应用的 ssh 从不询问密码。需要密码的主机会显示 `needs login: run leon hosts check devbox in a terminal`；照做一次（接受主机密钥、解锁你的密钥），应用下一次尝试时就能连上。连接断开期间，该主机的窗格会保留最后的画面，并显示“reconnecting”提示。

![侧边栏中的本机（This Mac）和一台 devbox，各自带有会话和标签页](../../images/m16b-hosts.png)

![devbox 上一个获得焦点的窗格，标题栏中显示主机名](../../images/m16b-remote-pane.png)

![一台无法连接的主机，标签下方显示 ssh 给出的原因](../../images/m16b-unreachable.png)

![添加主机（Add Host…）列出 tailnet 上的机器，每台都显示其系统以及是否在线](../../images/app-tailnet.png)

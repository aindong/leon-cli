<!-- i18n: source=docs/install.md commit=a525beb0ae17 -->
> 本页是译文，以[英文原文](../../install.md)为准，英文版可能更新。

# 从源码构建 leon

属于 [README](README.md#安装) 的一部分。

自己构建 leon 需要 Rust 工具链（[rustup](https://rustup.rs)）和 git。下面两节分别介绍在 macOS 和 Linux 上的构建方法。

## macOS

需要 macOS 11 或更高版本。在本仓库的检出目录中执行：

```sh
scripts/install.sh
```

它会构建 `Leon.app`，复制到 `~/Applications`，并把 `leon` 命令链接到 `~/.local/bin`（用 `--prefix DIR` 可改为 `DIR/bin`）。它从不使用 root，会打印每一步操作，可以安全地重复运行，并且拒绝替换这些路径上不是由它安装的任何东西。`scripts/install.sh --uninstall` 会把两者都删除。可以从访达（Finder）、聚焦搜索（Spotlight）或程序坞（Dock）打开 **Leon**，也可以直接输入 `leon`。如果只想构建应用，`scripts/bundle.sh` 会生成 `target/release/Leon.app`。

应用是 leon 服务器的一个客户端。它会连接到正在运行的服务器（从 shell 启动的或之前启动应用时留下的），没有则自行启动一个。退出应用后，服务器和你的程序会继续运行；`leon server stop`，或 leon 菜单中的 **退出并结束所有会话（Quit and End All Sessions…）** 会结束它们。

## Linux

在 Linux 上，leon 提供服务器、CLI 和 `leon attach`（以及智能体和 Lua 脚本）；应用暂不支持，所以在终端中单独运行 `leon` 会执行 `leon attach`。用 `cargo` 构建：

```sh
cargo build --release -p leon
```

这会得到一个无界面的 `target/release/leon`；把它放到你的 `PATH` 上。它需要 glibc 2.34 或更新版本（Debian 12、Ubuntu 22.04、RHEL 9 及以后）。如果你的会话设置了 `$XDG_RUNTIME_DIR`，socket 会放在 `$XDG_RUNTIME_DIR/leon`。有 Docker 的话，也可以改为构建软件包：

```sh
scripts/package-linux.sh                 # or --arch amd64 / --arch arm64
```

它会把一个 tarball 和一个 `.deb` 写到 `target/linux/<arch>/`，并检查两者都能在干净的容器中安装和运行。在 Debian 或 Ubuntu 上用 `dpkg`（以 root 身份）安装 `.deb`：

```sh
dpkg -i target/linux/$(dpkg --print-architecture)/leon_*.deb
```

或者把 tarball 解包到任意位置，再把其中的 `leon` 放到你的 `PATH` 上：

```sh
tar xzf target/linux/*/leon-*-"$(uname -m)"-linux.tar.gz -C ~/.local
ln -s ~/.local/leon-*-linux/leon ~/.local/bin/leon
```

## 发布文件

`scripts/release.sh` 从某个提交的干净克隆中构建一个发布版包含的文件：`Leon-<version>-macos-<arch>.zip`（Leon.app，ad hoc 签名，未经公证）、macOS 的 CLI tarball、Linux 上 arm64 和 x86_64 的 tarball 与 `.deb`，以及 `SHA256SUMS`。它在 macOS 上运行，构建 Linux 软件包需要 Docker。`leon --version` 会显示发布版本号和构建所用的提交。推送一个标签后，GitHub 上也会做同样的事，为每个文件签名并发布：[releases.md](../../releases.md)。

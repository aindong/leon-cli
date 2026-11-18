<!-- i18n: source=docs/install.md commit=28ab3cd4a46d -->
> これは翻訳です。[英語のページ](../../install.md)が正式な内容で、こちらより新しい場合があります。

# leon をソースからビルドする

[README](README.md#インストール) の一部です。

leon を自分でビルドするには、Rust のツールチェーン（[rustup](https://rustup.rs)）と git が必要です。以下の 2 つの節で、macOS と Linux でのビルド方法を説明します。

## macOS

macOS 11 以降が必要です。このリポジトリのチェックアウトで次を実行します:

```sh
scripts/install.sh
```

これは `Leon.app` をビルドして `~/Applications` にコピーし、`leon` コマンドを `~/.local/bin` にリンクします（`--prefix DIR` を使うと `DIR/bin` になります）。root は一切使わず、行うことを 1 つずつ表示し、何度実行しても安全で、自分がインストールしたものでなければ、それらのパスにあるものを置き換えることはありません。`scripts/install.sh --uninstall` で両方を削除します。**Leon** は Finder、Spotlight、Dock から開くか、`leon` と入力して起動します。アプリだけをビルドするなら、`scripts/bundle.sh` が `target/release/Leon.app` を作ります。

アプリは leon サーバーのクライアントです。動作中のサーバー（シェルから、または以前のアプリの起動で立ち上がったもの）に接続するか、なければ起動します。アプリを終了してもサーバーとプログラムは動き続けます。終了させるには `leon server stop` を実行するか、leon メニューの **終了してすべてのセッションを閉じる（Quit and End All Sessions…）** を選びます。

## Linux

Linux での leon は、サーバー、CLI、`leon attach`（エージェントと Lua スクリプトも）です。アプリはまだないため、ターミナルで `leon` だけを実行すると `leon attach` が動きます。`cargo` でビルドします:

```sh
cargo build --release -p leon
```

これでヘッドレスの `target/release/leon` ができるので、`PATH` に置いてください。glibc 2.34 以降が必要です（Debian 12、Ubuntu 22.04、RHEL 9 以降）。セッションで `$XDG_RUNTIME_DIR` が設定されていれば、ソケットは `$XDG_RUNTIME_DIR/leon` に置かれます。Docker があれば、代わりにパッケージをビルドできます:

```sh
scripts/package-linux.sh                 # or --arch amd64 / --arch arm64
```

これは tarball と `.deb` を `target/linux/<arch>/` に書き出し、どちらもクリーンなコンテナでインストールして動くことを確認します。Debian か Ubuntu では、`.deb` を `dpkg` で（root として）インストールします:

```sh
dpkg -i target/linux/$(dpkg --print-architecture)/leon_*.deb
```

または、tarball を好きな場所に展開して、その中の `leon` を `PATH` に置きます:

```sh
tar xzf target/linux/*/leon-*-"$(uname -m)"-linux.tar.gz -C ~/.local
ln -s ~/.local/leon-*-linux/leon ~/.local/bin/leon
```

## リリースのファイル

`scripts/release.sh` は、1 つのコミットのクリーンなクローンから、リリースに含まれるファイルをビルドします: `Leon-<version>-macos-<arch>.zip`（Leon.app、アドホック署名で公証なし）、macOS 向けの CLI の tarball、Linux の arm64 と x86_64 向けの tarball と `.deb`、そして `SHA256SUMS` です。macOS で動き、Linux のパッケージには Docker が必要です。`leon --version` はリリースと、ビルド元のコミットを表示します。タグを push すると GitHub 上で同じことが行われ、すべてのファイルに署名してリリースを公開します: [releases.md](../../releases.md)。

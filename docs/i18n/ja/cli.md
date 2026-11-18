<!-- i18n: source=docs/cli.md commit=14af788efc95 -->
> これは翻訳です。[英語のページ](../../cli.md)が正式な内容で、こちらより新しい場合があります。

# leon コマンドリファレンス

`leon` のすべてのコマンドとフラグです。`leon <command> --help` は同じ内容を短く表示します。このページ（英語版）と `docs/cli/` にあるその分割ページが `--help` に出るコマンドやフラグを書き漏らすと、テスト（`crates/leon/tests/m7b_cli_doc.rs`）が失敗します。

## コマンドが対象を見つける仕組み

leon は**セッション**を持ちます。セッションは**ウィンドウ**（タブ）を持ち、ウィンドウは分割レイアウトで**ブロック**（ターミナルのペイン）を持ちます。どのオブジェクトにも `block:91c0…` のような変わらない ID があります。

- **ターゲット。** `-s` でセッションを、`-b` でブロックを選びます。ターゲットには、完全な ID、ラベル（大文字小文字は区別しません）、1 から数える位置（ウィンドウはタブの順に数えます）、または ID の末尾 4 文字以上を指定できます。ターゲットが複数のものに一致すると、leon は一致したものを一覧にして 3 で終了します。
- **デフォルト。** leon のターミナルの中では、`LEON_SESSION` と `LEON_BLOCK` が自分のセッションとブロックを指すので、コマンドはフラグなしで「ここ」に対して動きます。自分のセッションとは、いま `LEON_BLOCK` を含んでいるセッション（ペインは別のセッションに移れます）で、なければ `LEON_SESSION` です。leon の外では `-s`/`-b` を渡してください。その場合、セッションに対するコマンドは、そのアクティブなウィンドウのフォーカス中のブロックに対して動きます。
- **サーバー。** サーバーが必要なコマンドは、サーバーが動いていなければ個人用のサーバーを起動します（`--autostart=false` か `LEON_AUTOSTART=0` で無効にできます）。ソケットは `$LEON_RUNTIME_DIR`、なければ `$XDG_RUNTIME_DIR/leon`、それもなければ `~/.local/state/leon/run` に置かれます。そのパスは Unix ソケットに収まる長さ（macOS では 104 バイト未満、Linux では 108 バイト未満）でなければならず、収まらなければ leon はそう伝えて止まります。
- **別のマシン。** `-H HOST` は、あなた自身の `ssh` を通して、クライアントのコマンドを別のマシンの leon サーバーに送ります（[cli/hosts.md](../../cli/hosts.md) を参照）。

## グローバルフラグ

すべてのコマンドで使えます。

| フラグ | 動作 |
|---|---|
| `--autostart[=true\|false]` | サーバーが動いていなければ個人用のサーバーを起動します（デフォルト true、環境変数 `LEON_AUTOSTART`）。 |
| `--json` | テキストの代わりに JSON を出力します。 |
| `-s, --session SESSION` | 対象のセッション（デフォルト `$LEON_SESSION`）。 |
| `-b, --block BLOCK` | 対象のブロック（デフォルト `$LEON_BLOCK`、なければアクティブなウィンドウのフォーカス中のブロック）。 |
| `-H, --host HOST` | ssh 越しに別のマシンの leon サーバーを操作します: `leon hosts` のラベル、または `ssh://[USER@]HOST[:PORT]`。`-S, --server` とも書けます。これを使うと `$LEON_SESSION` と `$LEON_BLOCK` は無視されます（このマシンのオブジェクトを指しているため）。 |
| `-h, --help` | ヘルプを表示します。 |
| `-V, --version` | バージョンを表示します。 |

## 終了コード

| コード | 意味 |
|---|---|
| 0 | 成功 |
| 1 | 失敗 |
| 2 | コマンドラインの誤り（打ち間違いがあると、`leon` は一番近いコマンドを提案します） |
| 3 | ターゲットが見つからない、または曖昧 |
| 4 | サーバーに接続できない |
| 10 | `self-update --check`: 新しいリリースがある |
| 130 | 中断された（Ctrl-C） |

`leon wait` と `leon run --wait` は、代わりにプログラム自身の終了ステータスで終了します。

## アプリ

### `leon app`

leon アプリを開き、サーバーに接続します。すでに動いているサーバー（CLI が起動したものでも構いません）か、`leon server start` と同じように先に起動したサーバーです。CLI とエージェントは、あなたが見ているのと同じセッションを操作します。コマンドなしの `leon` も同じ動作です。アプリを終了しても（⌘Q、ウィンドウを閉じる）デタッチするだけで、すべてのプログラムは動き続けます。アプリの「終了してすべてのセッションを閉じる（Quit and End All Sessions…）」か `leon server stop` で終了させます。`--autostart=false` でサーバーがなければ 4 で終了します。

アプリを含まないビルド（今のところ Linux、または `--no-default-features` の macOS）にはウィンドウがありません。stdin と stdout がターミナルなら、`leon` だけで `leon attach` が（指定された `-s` と `-H` 付きで）動き、そうでなければヘルプを表示して 0 で終了します（`-s` か `-H` を指定していれば 2 で終了）。`leon app` はその旨を伝えて 1 で終了します。

## ターミナルで

### `leon attach [TARGET]`

どのターミナルでも、ローカルでも ssh 越しでも leon を使えます（`ssh -t HOST leon attach`。`-t` がないとターミナルがないので 1 で終了します）。TARGET がなければ（`-s` もなければ）、すべてのセッション、そのタブ、その中のブロックを並べたピッカーを開き、選んだブロックの画面をライブでプレビューします。入力で絞り込み（コマンドパレットと同じく、文字を順番どおりに照合します）、↑ と ↓（または ctrl+p、ctrl+n）で移動、Enter でアタッチ、Esc で戻るか終了します。TARGET にはセッションかブロックを指定でき、ブロックならそのタブがそのブロックにフォーカスした状態で開きます。セッションが 1 つもなければ、Enter で新しく始めます。

アタッチすると、セッションのアクティブなタブを表示します。分割レイアウトのすべてのペインが境界線で区切られ、セッションとそのタブを示すステータスバーが付きます。キーはフォーカス中のペインに送られます。ただし、プレフィックスキーと設定のバインド（`init.lua`。`cmd` キーはターミナルからは決して届きません）は除きます。プレフィックス（`--prefix KEY`、なければ設定の `leon.mode("attach", { prefix = … })`、それもなければ ctrl+b）のあとに:

| キー | 動作 |
|---|---|
| `d` | デタッチ: `leon attach` は 0 で終了し、すべてのプログラムは動き続けます |
| ←↑↓→ または `h` `j` `k` `l` | その方向のペインにフォーカス |
| `%`, `"` | ペインを右に、下に分割 |
| `z` | ペインをズーム（もう一度で元に戻す） |
| `c` | 新しいタブ |
| `n`, `p` | 次のタブ、前のタブ |
| `s` | ピッカーに戻る |
| もう一度プレフィックス | プレフィックスキーをペインに入力 |

ペインは、このターミナル自身のペインのレイアウトが変わるたびに（アタッチ、リサイズ、分割やズーム、ペインが画面に戻ってきたとき）このターミナルに合うサイズになります。その後ほかのクライアントがサイズを変えたペインは、切り詰められるか余白で埋められます。終了時（デタッチ、サーバーの停止、SIGHUP、SIGTERM、SIGINT、SIGQUIT）には、ターミナルを元の状態に戻します。SIGKILL のあとでも、ウォッチドッグのプロセスがそれを行います。シグナルで終了させられた場合は 128 + シグナル番号で終了します。

`-H` 越しでは、切れた接続を再試行している間、ステータスバーに `reconnecting to HOST (try N)…` と表示され、キーはペインにすべて届くか、まったく届かないかのどちらかです。ホストから 1 秒間何の応答もないと、入力したキーは接続の簡単な確認を待ってから送られます。確認に失敗するか、接続が再接続中なら、キーは捨てられ、バーに送られなかったキーの数が表示されます。接続が戻ったらもう一度入力してください。ペーストもキーと同じ列に並んで待ちます。1 つ制限があります。その 1 秒の間、単語の途中で止まり始めた場合は、すでに送られている途中のキーが巻き込まれ、遅れて届くことがあります。

## セッションとウィンドウ

`leon new`、`ls`、`kill`、`leon session`、`leon window` は [cli/sessions.md](../../cli/sessions.md) にあります。

## プログラムの実行

### `leon run [FLAGS] -- CMD…`

新しいウィンドウでコマンドを実行し、新しいブロックの ID を表示します。コマンドはシェルを通さずに直接実行されます。パイプや `&&` を使うなら `leon run -- sh -c '…'` としてください。コマンドは、サーバーが Finder から開いた Leon.app によって起動された場合でも、ログインシェルの PATH から探されます。

- `--label LABEL`: ブロックに名前を付け、`-b LABEL` で見つけられるようにします。
- `--keep-open`: プログラムの終了後もブロックを残し、出力を読めるようにします。
- `--wait`: プログラムを待ち、その終了ステータスで終了します。
- `--output[=true|false]`: `--wait` と一緒に使うと、プログラムの終了後、ID の行に続けてプログラムのテキストを表示します。`leon capture --trim` が表示する内容、つまりスクロールバックと画面です。stdout がターミナルでないとき（エージェントやスクリプトの `$(…)`）はデフォルトでオン、ターミナルではオフです。テキストを読むために、`run` はテキストが読まれるまでブロックを開いたままにし、そのあと閉じます（`--keep-open` がなければ）。それより前に止められた `run --wait` は、`--keep-open` のときと同じくブロックを残します。
- `-c, --cwd DIR`: 作業ディレクトリ（デフォルトは現在のディレクトリ。`-H` のときはホストのホームディレクトリで、`DIR` はそのホスト上の絶対パスでなければなりません）。

```sh
leon run --label tests --keep-open -- cargo test
leon run --wait -- make                       # in a script: make's output, then its status
leon run --wait --output=false -- make        # the status alone
```

### `leon split [FLAGS] [-- CMD…]`

ブロックを分割し、新しいペインで `CMD`（デフォルトはログインシェル）を実行します。新しいブロックの ID を表示します。分割するのは `-b`（分割そのものを指定して、グループ全体の隣にペインを置くこともできます）、なければフォーカス中のブロックです。

- `--split right|left|above|below`: 新しいペインの位置（デフォルトは right）。
- `--ratio PCT`: 新しいペインが占める割合、1〜99（デフォルトは 50）。
- `--label LABEL`、`--keep-open`、`-c, --cwd DIR`: `leon run` と同じです。
- `--focus[=true|false]`: 新しいペインにフォーカスします（デフォルトは true）。

```sh
leon split
leon split --split below --ratio 30 --label logs -- tail -f /var/log/system.log
leon split --focus=false -b tests -- htop
```

### `leon send [TEXT]`

`TEXT` をターミナルに入力します（デフォルトは stdin から読みます）。Enter は付きません。続けて `leon send-key enter` を実行してください。

### `leon send-key KEY…`

キーを順に押します。書き方はバインドの構文と同じです: `enter`、`escape`、`tab`、`space`、`backspace`、`up`、`ctrl+c`、`alt+left`、`a`…

### `leon capture`

ターミナルのスクロールバックと画面をテキストとして表示します。

- `--trim`: 行末の空白と、末尾の空行を取り除きます。

### `leon wait [BLOCK]`

ターミナルのプログラムが終了するのを待ち、その終了ステータスで終了します（デフォルトのブロックは `-b`、なければ `$LEON_BLOCK`）。

- `--for DURATION`: この時間が過ぎたらあきらめ（`500ms`、`10s`、`2m`）、1 で終了します。
- `--text TEXT`: 代わりに、ブロックの画面かスクロールバックに `TEXT` が表示されるまで待ち、0 で終了します（すでに表示されているテキストも数えます）。`--for` を過ぎるか、先にブロックが閉じると 1 で終了します。動き続けるプログラム向けです: プロンプト、開発サーバーの「ready」の行など。

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

自分のブロック（`-b`、なければ `$LEON_BLOCK`。どちらもなければ 2 で終了）の状態を、そのプログラムからの OSC 7501 レポートと同じように報告します。タブとペインのヘッダー、アプリのアクティビティ（Activity）一覧に表示され、対応待ち、エラー、長い実行の完了では通知も出ます。OSC 7501 を書き出せないツール（フックや、tty のないジョブ）のためのものです。ブロックのルートのレコードを置き換え、そのレコードの寿命に従います。

- `--state working|blocked|done|error`: 状態（デフォルトは `done`）。
- `--title T`、`--body B`: それぞれ 1 行（最大 192 バイトと 2048 バイト）。OSC 7501 が捨てるようなテキスト（制御文字、改行、見えない文字）は拒否され（1 で終了）、何も記録されません。

```sh
id=$(leon run --keep-open -- sleep 30)
leon notify -b "$id" --state blocked --title "Deploy" --body "approve the rollout?"
leon block call -b "$id" program_status
leon block close "$id"
```

## レイアウト

`leon focus`、`move`、`swap`、`zoom`、`resize` は [cli/layout.md](../../cli/layout.md) にあります。

## ブロック

### `leon block`

ブロック（ターミナルのペイン）を管理します。

### `leon block ls`

セッションのブロックを一覧にします。

### `leon block inspect [BLOCK]`

ブロックを表示します: ウィンドウ、pid、コマンド、終了ステータス、そしてプログラムが登録していれば再開コマンド（`set_resume`）。

### `leon block close [BLOCK]`

ブロックを閉じ、そのプログラムを終了します。

### `leon block call METHOD [ARGS]`

`size` や `program_status` のようなブロックのメソッド、または `dev.leon.terminal.size` のような完全な名前のメソッドを呼び出します。`ARGS` は JSON オブジェクト、`@file`、または stdin を表す `-` です。

`set_resume` は、再起動後にこのブロックのプログラムを再開するコマンドを登録します（`null` で消去）。復元されたブロックは、設定の `resume` に従ってそれを提案します（[lua/settings.md](../../lua/settings.md) を参照）:

```sh
leon block call set_resume '{"command": ["claude", "--resume", "c7583d2c-3133-4d13-b5f0-45883f1cabcb"]}'
```

## 自動化

`leon events`、`leon do`、`leon actions`、`leon keymap`、`leon api` は [cli/automation.md](../../cli/automation.md) にあります。

## 設定とテーマ

`leon config`（check、reload、get、set）と `leon theme`（list、import、show）は [cli/settings.md](../../cli/settings.md) にあります。

## サーバー

`leon server`（run、start、stop、status、proxy）は [cli/server.md](../../cli/server.md) にあります。

## ホスト

`-H HOST` は、システムの `ssh` を通して別のマシンの leon サーバーにつなぎます。`leon hosts`（list、add、remove、check。`--tailscale` で tailnet からマシンを追加）と、`-H` の仕組みは [cli/hosts.md](../../cli/hosts.md) にあります。

## アップデート

### `leon self-update`

leon を最新のリリースにアップデートします。`--check` は報告だけします（10 で終了）: [updates.md](../../updates.md)。

## エージェント

`leon skill`（show、setup、status、remove）、`leon float`、`leon toggle`、`leon dock` は [cli/agents.md](../../cli/agents.md) にあります。

<!-- i18n: source=docs/agents.md commit=a525beb0ae17 -->
> これは翻訳です。[英語のページ](../../agents.md)が正式な内容で、こちらより新しい場合があります。

# エージェントが leon を操作する

leon のターミナルで動くコーディングエージェント（Claude Code、Codex）は、あなたがそこでできることを同じようにできます。自分の隣にペインを開き、そこでテストを実行し、終わるのを待ち、画面を読み、REPL に入力し、自分が開いたものを閉じます。使うのはあなたと同じ `leon` コマンドです。このページでは、エージェントに教える方法と、覚えたあとのエージェントのふるまい方を説明します。

## 一度だけ設定する

```sh
leon skill setup --dry-run   # show what it would write
leon skill setup             # install for every agent it finds
leon skill status            # current, outdated, missing, foreign or unreadable
```

`leon skill setup` は、見つけたエージェントごとに 2 つのものをインストールします。対象は、設定ディレクトリがあるエージェント、またはプログラム（`claude`、`codex`）が PATH にあるエージェントで、一度も実行したことがなくても対象になります。エージェントごとに、選んだ理由と書き込んだファイルを 1 つずつ表示します。どちらも見つからなければ、共有の `~/.agents/skills/leon/`（ほかのエージェントもこれを読みます）だけを書き込み、そう伝えます。`--agent claude|codex` で 1 つを選べます:

| | Claude Code | Codex |
|---|---|---|
| スキル: `SKILL.md` のスタブ | `~/.claude/skills/leon/`（`$CLAUDE_CONFIG_DIR`） | `~/.agents/skills/leon/` |
| `leon skill hook --agent …` を実行するセッション開始フック | `~/.claude/settings.json` | `~/.codex/hooks.json`（`$CODEX_HOME`） |

- **スキル**は、エージェントに `leon skill show` を実行させます。これは `leon` のバイナリに組み込まれた完全なガイドを表示します。leon をアップグレードすればガイドもアップグレードされ、再インストールは不要です。
- **フック**は、エージェントのセッションが始まるときに動きます。leon の中では、エージェントに自分のセッションとターミナルのブロックを伝え、そのセッションの再開方法を記録します（後述）。leon の外では何も表示せず、サーバーにも接続しません。古い leon のフックは `outdated` になるので、`leon skill setup` を実行してください。

セットアップは何度実行しても安全です。設定ファイルはその場で編集され、あなたのキーとその順序はそのまま残ります。ファイルは一度だけ `<file>.leon-backup` にバックアップされ、想定した JSON の形でないファイルは扱いません。自分が書いたものでない `SKILL.md` は、`--force` を渡さない限り置き換えません（古いものは `SKILL.md.leon-backup` として残ります）。

```sh
leon skill show                    # read the guide the agent reads
leon skill remove --dry-run
leon skill remove                  # removes what setup installed, and nothing else
```

## 自分はどこにいるか：`LEON_SESSION` と `LEON_BLOCK`

leon のターミナルには、どれも 2 つの変数があります:

- `LEON_SESSION`: セッションの ID。例: `session:3f2a…`。
- `LEON_BLOCK`: このターミナル自身の ID。例: `block:91c0…`。

leon のコマンドはこれらをデフォルトとして使うので、エージェントの `leon run` はフラグなしで*自分の*セッションに、ユーザーのほかの作業の隣にタブを開きます。自分のセッションとは、いま `LEON_BLOCK` を含んでいるセッションです。別のセッションに移されたペイン（アプリのスイッチャーで ⌘↩）は、開始時の `LEON_SESSION` を持ったままです。動作中のプログラムの環境は変えられないからです。そのため leon はブロックを調べ、`LEON_BLOCK` が設定されていないかすでに存在しないときだけ `LEON_SESSION` を使います。どちらの変数も設定されていなければ、エージェントは leon の中にいません。その場合は `leon ls` を実行し、どのセッションを使うか尋ねるべきです。

エージェントはユーザーの言葉を次のように解釈すべきです:

- 「ここで」「隣で」「このセッションで」: 自分のセッション、つまり `LEON_BLOCK` を含むセッション。
- 「テストのペイン」「api セッション」: ユーザーが挙げた名前は常にデフォルトより優先されます。`-b` や `-s` で渡します。
- 名前が複数のものに一致すると、leon はそれらを一覧にして 3 で終了します。推測せず、尋ねてください。

**落とし穴が 1 つあります:** `-b` なしの `send`、`send-key`、`capture` は `LEON_BLOCK`、つまりエージェント自身のターミナルに対して動きます。そこに入力すると、エージェント自身の入力欄に入力することになります。対象のブロックを必ず `-b` で渡してください。

## エージェントから見たセッション

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

ガイドがエージェントに与えるルール:

- 作ったブロックにはすべてラベルを付け、ユーザーが自分のものと区別できるようにする。
- 終わったら自分が作ったものを閉じる。ユーザーのブロックや自分自身の `LEON_BLOCK` は決して閉じない。
- 頼まれない限り、ユーザーのターミナルを読んだり入力したりしない。画面には秘密の情報があり、送ったキーはユーザーのシェルで実行される。
- 動き続けるプログラム（開発サーバー、REPL）は、終了を待たない。期待するテキストを待つ: `leon wait -b ID --text TEXT --for 30s`。
- エージェントからの `leon run --wait -- CMD` は、1 回の呼び出しでプログラムの出力を表示し、その終了ステータスで終了する。
- パースする出力には `--json` を使い、コマンドで足りないものには `leon api list` / `leon api describe` を使う。

## 再起動後もエージェントは戻ってくる

サーバーが止まると（`leon server stop`、ログアウト、再起動）、各ブロックのテキストが保存されます。復元されたブロックは、薄く表示された `— restored from <time> —` の行と新しいシェルの上にそのテキストを表示します。プログラムは残らず、保存されたコマンドが再実行されることもありません。戻ってくるのは、プログラムが `set_resume` で自分のブロックに登録したコマンドです。フックがこれを代わりに行います。Claude Code なら `claude --resume <id>`、Codex なら `codex resume <id>` を、エージェントがフックに渡すセッション id から登録します。記録するのは、ターミナルのフォアグラウンドにいるエージェントの場合だけです。そのため、別のエージェントのツールが入れ子で実行した `claude -p` が対話型セッションのコマンドを置き換えることはありません。Claude Code のプリントモード（`-p`、`--print`）でも記録しません。単発の実行には再開するものがないからです。

ほかのプログラムも、自分のコマンドを登録できます:

```sh
leon block call set_resume -b "$LEON_BLOCK" '{"command": ["my-agent", "--resume", "42"]}'
leon block inspect "$LEON_BLOCK"        # shows "resume: my-agent --resume 42"
leon block call set_resume -b "$LEON_BLOCK" '{"command": null}'
```

復元時、ブロックは `init.lua` の `resume` 設定（`leon.set("resume", "ask")`）に従います。`"ask"`（デフォルト）は新しいプロンプトにコマンドを入力して Enter を待ち（Ctrl-C でスキップ）、`"always"` はそのまま実行し、`"never"` は無視します。コマンドが提案されるのは、次の 1 回の再起動のときだけです。一度入力されると再び保存されないので、スキップしたコマンドや古くなったコマンドが毎回戻ってくることはありません。再開したエージェントのフックが、あらためて登録します。

## プログラムの状態：エージェントが何をしているかをユーザーに見せる

エージェントやツールは OSC 7501 で自分の状態（`working`、`blocked`、`done`、`error`）を報告でき、leon はそれをターミナルごとに追跡します。`leon block call program_status -b "$LEON_BLOCK"` で読み返せ、変化のたびに `leon events -s SESSION` に `program_status_changed` イベントが出ます。

### OSC 7501 のフォーマット

レポートは、ターミナルに書き込む 1 つのエスケープシーケンスです: `ESC ] 7501 ;`、次に `:` でつないだ `key=value` のペア、最後に `BEL`（または `ESC \`）。キーは小文字の英字です。値に使えるのは英字、数字、`_ . , + / = -` だけです。`id` は `/` でつないだ最大 8 つのセグメントで、各セグメントは英字、数字、`_ . + -` の 1〜32 文字です。`ESC ] 7501 ; ? BEL` を送ると、ターミナルが何に対応しているかを問い合わせられます。leon は `ESC ] 7501 ; ? : states=idle,working,done,blocked,error : kinds=permission,question,auth ESC \`（空白なし）と答えます。

たとえば `printf '\033]7501;state=working:progress=40\007'` は、プログラムが 40% まで進んでいることを示します。

- **キー。** `state`（必須）: `idle`（報告することなし）、`working`、`done`、`blocked`、`error`、`clear` のいずれか。`id` はレコードの名前です（なければルートのレコード、`/` で子になります）。`kind`（`blocked` のときのみ）: `permission`、`question`、`auth`。`progress`: 0〜100、`working` か `blocked` と一緒に使います（なければ不明）。`app`: `[A-Za-z0-9_.+-]{1,32}`。これを設定している一番近い祖先から引き継ぎます。`title` と `msg`: 制御文字を含まない 1 行の UTF-8 を base64 にしたもの。
- **ルール。** 空白は取り除かれ、形の崩れたペアはスキップされ、未知のキーは無視され、重複したキーは最後のものが有効です。未知の `state` や不正な `id` はレポート全体を捨てます（不正な `id` がルートにフォールバックすることはありません）。各レポートはそのレコードを丸ごと置き換えます。`clear` はレコードとその子を削除します（`id` がなければすべてのレコード）。
- **寿命。** `working` と `blocked` は、プロセスが終了するか新しいプロンプトが始まる（OSC 133 `A`）と消えます。`done` と `error` は、そのペインを見るまで残ります。フルリセット（`RIS`）はレコードを消去しますが、ソフトリセットや代替画面への切り替えでは消えません。
- **上限。** 次のどれかを超えると、レポート全体が捨てられます: シーケンス 4096 バイト、キー 16、`msg` はエンコード後 2732（デコード後 2048）、`title` 256（192）、`app` 32、`id` は全体で 128、セグメントあたり 32、深さ 8。1 つのターミナルが保持するレコードは 256 件で、最も長く使われていないものから捨てます。

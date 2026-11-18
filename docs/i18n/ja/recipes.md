<!-- i18n: source=docs/recipes.md commit=28ab3cd4a46d -->
> これは翻訳です。[英語のページ](../../recipes.md)が正式な内容で、こちらより新しい場合があります。

# プロジェクトと git worktree

[examples/project.lua](../../../examples/project.lua) は `project_open` という Lua アクションを定義します。[examples/projects.lua](../../../examples/projects.lua) は同じレシピを `leon do` で実行します。どちらも、ラベルがプロジェクトのフォルダー名と一致するセッションを探し、なければ最初のターミナルをそのディレクトリで開くセッションを作ります。作成には `session.create` の `cwd` を使います。これは `leon new --cwd DIR` と同じパラメーターです。プロジェクトと worktree の方針はこれらの例の中だけにあります。コピーしてカスタマイズしてください。

これらの例はローカルで動きます。ローカルのサーバーとローカルのフォルダーを使ってください。スクリプトのパスは呼び出し元のディレクトリを基準に解決されます。アクションはサーバーの中で動くため、`dir` には絶対パスが必要です。セッション名はグローバルです。フォルダー名のセッションがすでにあれば、そのシェルがその後ディレクトリを移動していても再利用されます。重複したセッションのラベルや、フォルダー名が同じ worktree はエラーになるので、先に名前を変えてください。フォルダー名は leon のラベルの条件（128 文字以内、制御文字なし）を満たす必要があります。

## 1 つのプロジェクト

leon のチェックアウトで、`leon` が PATH にある状態で:

```sh
leon do ./examples/projects.lua dir="$PWD"
```

スクリプトはセッション ID を表示します。`leon attach SESSION_ID` でターミナルに開くか、アプリでそのセッションを選んでください。スクリプトを繰り返し実行しても同じ ID が表示され、余分なターミナルは作られません。別のプロジェクトは `dir=/path/to/project` や `dir=../project` で渡せます。レシピを使わずに新しいセッションを作るなら、`leon new project-name --cwd /path/to/project` を使います。

## すべての worktree

```sh
leon do ./examples/projects.lua dir="$PWD" worktrees=true
```

レシピは `git worktree list --porcelain -z` を読み、使える worktree ごとに、メインのチェックアウトも含めて 1 つずつセッション ID を表示します。NUL 区切りなので、親パスの空白、引用符、改行もそのまま扱えます。ベアリポジトリと prunable の印が付いた worktree はスキップされます。worktree の作成や削除はしません。`git worktree add` と `git worktree remove` は自分で実行してください。

## 無視されたファイルを新しい worktree にコピーする

worktree を作ったあと、そのセッションが作られる前に、指定した無視されたファイルをコピーするよう選べます。ここでは `.env` が、コピー元のルートにある、無視された通常のファイルである必要があります:

```sh
leon do ./examples/projects.lua dir="$PWD" worktrees=true \
  copy_from="$PWD" 'copy_files=[".env"]'
```

このフックを実行するのは、これから作られるセッションだけです。コピー先にすでにあるファイルには触れません。新しいコピーは非公開になります（モード 0600、ファイルシステムによります）。名前はルートのファイル名でなければならず、`/` や `..` は使えません。シンボリックリンクと、Git が無視されたファイルと見なさないファイルは拒否されます。デフォルトでは、無視されたファイルは何もコピーされません。`copy_from` はコピー元のチェックアウト、`copy_files` は名前の JSON 配列です。フックが失敗すると、レシピはそのセッションを作る前に止まります。すでに作られたセッションとコピーは残ります。レシピは一度に 1 つだけ実行してください。セッションの一覧取得と作成は別々の API 呼び出しです。

## アクションをインストールする

`examples/project.lua` を `init.lua` の隣にコピーし、次の設定を追加します（バインドするフォルダーは、あなたのプロジェクトの絶対パスに変えてください）:

```lua
require("project")
leon.bind("cmd+shift+o", "project_open", { dir = "/path/to/project" })
```

`leon config reload` で再読み込みします。`leon do project_open dir=/path/to/project` はセッション ID の配列を返します。`leon attach SESSION_ID` かアプリで 1 つを選んでください。このバインドはセッションを作成または検索するだけで、どのクライアントで選ぶかはあなたに任せます。アクションは、スクリプトと同じ `worktrees`、`copy_from`、`copy_files` の引数を受け取ります。

モジュールは、準備用のフックとして `before_create(dir, args)` を公開しています。`require` のあとで置き換えるか、デフォルトの動作（無視されたファイルのコピー）を残したいならラップしてください。準備に失敗したときは、フックは `error(message)` で失敗させるべきです。フックは `session.create` の前に動き、アクションには leon の通常の 30 秒の期限があります。

このページの `sh` と `lua` のブロックはすべて `m21_recipe_docs` で実行されます。`m21_recipe` は、2 つの git worktree と無視されたファイルのコピーも含め、実際のスクリプトとアクションを非公開のサーバーで確認します。

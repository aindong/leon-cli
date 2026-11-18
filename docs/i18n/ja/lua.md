<!-- i18n: source=docs/lua.md commit=28ab3cd4a46d -->
> これは翻訳です。[英語のページ](../../lua.md)が正式な内容で、こちらより新しい場合があります。

# Lua：設定とスクリプト

leon は 2 つの場所で Lua 5.1 を使います:

- **設定**、つまり `init.lua`。サーバーが起動時に読み込みます: キーバインド、モード、独自のアクション。`leon config check` で確認し、`leon config reload` で変更を反映します。
- **スクリプト**。`leon do` があなたのマシンでサーバーに対して実行します: 単発の自動化や、イベントに反応する常駐の監視処理。

どちらも `leon.*` の関数を共有します（[一覧](../../lua/scripts.md#function-list)）。これらのページの例はすべてテスト（`crates/leon/tests/m7b_lua_doc.rs`）で実行されます。

## 設定ファイルの場所

サーバーは次のうち最初に見つかったものを読みます: `server run --config PATH`、`$LEON_CONFIG`、`$XDG_CONFIG_HOME/leon/init.lua`、`~/.config/leon/init.lua`。ファイルがなければ空の設定として扱います。`require` はその隣のファイルを読み込みます。

```sh
leon config check
leon config reload
leon keymap
leon actions
```

設定は読み込みのたびに上から下へ実行されます。不正な `leon.*` の呼び出しは `file:line` のメッセージを出してスキップされ、残りは読み込まれます。構文エラーは読み込み全体を失敗させ、サーバーは最後の正しい設定を使い続けます。どちらの場合も `leon config check` は 1 で終了します。

## ページ一覧

- [キーとモード](../../lua/keys.md): `leon.bind`、シーケンス、リピートキー、tmux 風のプリセット、vim-navigator、モードと `leon attach` のプレフィックス、関数のバインド、フローティングペイン、スクラッチシェルのキー。
- [独自のアクション](../../lua/actions.md): `leon.action`、`leon.session.<method>`、`leon.call`。
- [設定項目](../../lua/settings.md): `leon.set` のすべての設定、その値とデフォルト。
- [スクリプトと関数](../../lua/scripts.md): `leon do`、イベント、タイマー、`leon.exec`、`leon.*` の関数一覧と制限。

## プロジェクトのセッション

フォルダーごとにセッションを再利用し、必要なら worktree ごとのセッションと、無視されたファイルを準備するフックも使える、アクションと `leon do` スクリプトの例は、[プロジェクトと git worktree](recipes.md) を参照してください。

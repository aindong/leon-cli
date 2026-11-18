<!-- i18n: source=docs/lua.md commit=a525beb0ae17 -->
> Esta é uma tradução. A [página em inglês](../../lua.md) é a referência; ela pode estar mais atualizada.

# Lua: configuração e scripts

O leon fala Lua 5.1 em dois lugares:

- **A configuração**, `init.lua`, que o servidor carrega ao iniciar: atalhos de teclado, modos e
  suas próprias ações. Verifique-a com `leon config check`; aplique as mudanças com
  `leon config reload`.
- **Os scripts**, que o `leon do` roda na sua máquina contra o servidor: automação avulsa, ou um
  observador de longa duração que reage a eventos.

Os dois compartilham as funções `leon.*` ([a lista](../../lua/scripts.md#function-list), em
inglês). Cada exemplo destas páginas é rodado por um teste (`crates/leon/tests/m7b_lua_doc.rs`).

## Onde a configuração fica

O servidor lê o primeiro de: `server run --config PATH`, `$LEON_CONFIG`,
`$XDG_CONFIG_HOME/leon/init.lua`, `~/.config/leon/init.lua`. Um arquivo que não existe vale como
uma configuração vazia. `require` carrega arquivos ao lado dele.

```sh
leon config check
leon config reload
leon keymap
leon actions
```

Uma configuração roda de cima a baixo a cada carga. Uma chamada `leon.*` inválida é pulada com uma
mensagem `file:line` e o resto carrega normalmente; um erro de sintaxe faz a carga inteira falhar,
e o servidor mantém a última configuração válida. `leon config check` sai com 1 nos dois casos.

## As páginas

- [Keys and modes](../../lua/keys.md) (teclas e modos, em inglês): `leon.bind`, sequências, teclas que se
  repetem, um preset no estilo do tmux, vim-navigator, os modos e o prefixo do `leon attach`,
  atalhos com funções, painéis flutuantes e uma tecla para um shell de rascunho.
- [Your own actions](../../lua/actions.md) (suas próprias ações, em inglês): `leon.action`,
  `leon.session.<method>` e `leon.call`.
- [Settings](../../lua/settings.md) (configurações, em inglês): cada configuração de `leon.set`, os valores e
  o padrão dela.
- [Scripts and the functions](../../lua/scripts.md) (os scripts e as funções, em inglês): `leon do`, eventos,
  timers, `leon.exec`, a lista de funções `leon.*` e os limites.

## Sessões por projeto

Veja [Projetos e worktrees do git](recipes.md) para uma ação de exemplo e um script de `leon do`
que reaproveitam uma sessão por pasta, com sessões opcionais por worktree e um hook de preparação
que copia arquivos ignorados.

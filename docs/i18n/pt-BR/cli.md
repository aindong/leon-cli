<!-- i18n: source=docs/cli.md commit=14af788efc95 -->
> Esta é uma tradução. A [página em inglês](../../cli.md) é a referência; ela pode estar mais atualizada.

# Referência de comandos do leon

Todos os comandos e opções do `leon`. `leon <command> --help` mostra o mesmo texto de forma
resumida; um teste (`crates/leon/tests/m7b_cli_doc.rs`) falha se faltar na página em inglês e nas partes
dela em `docs/cli/` um comando ou opção que o `--help` mostra.

## Como os comandos encontram as coisas

O leon mantém **sessões**; uma sessão contém **janelas** (abas); uma janela contém **blocos**
(painéis de terminal) em um layout dividido. Todo objeto tem um ID estável, como `block:91c0…`.

- **Alvos.** `-s` escolhe uma sessão e `-b` um bloco. Um alvo pode ser um ID completo, um rótulo
  (sem diferenciar maiúsculas), uma posição começando em 1 (as janelas contam na ordem das abas) ou
  os últimos 4 ou mais caracteres de um ID. Se um alvo corresponde a mais de uma coisa, o leon lista
  as correspondências e sai com 3.
- **Padrões.** Dentro de um terminal do leon, `LEON_SESSION` e `LEON_BLOCK` indicam sua própria
  sessão e seu bloco, então os comandos agem "aqui" sem opções. Sua sessão é a que contém o
  `LEON_BLOCK` agora (um painel pode entrar em outra sessão); se não, a de `LEON_SESSION`. Fora do
  leon, passe `-s`/`-b`; os comandos de uma sessão então agem no bloco em foco da janela ativa dela.
- **O servidor.** Qualquer comando que precise do servidor inicia um pessoal se nenhum estiver
  rodando (`--autostart=false` ou `LEON_AUTOSTART=0` desliga isso). O socket fica em
  `$LEON_RUNTIME_DIR`, se não em `$XDG_RUNTIME_DIR/leon`, se não em `~/.local/state/leon/run`; o
  caminho dele precisa caber em um socket Unix (menos de 104 bytes no macOS, 108 no Linux), ou o
  leon avisa e para.
- **Outra máquina.** `-H HOST` envia qualquer comando de cliente para o servidor leon de outra
  máquina, pelo seu próprio `ssh` (veja [cli/hosts.md](../../cli/hosts.md), em inglês).

## Opções globais

Funcionam com todos os comandos.

| Opção | O que faz |
|---|---|
| `--autostart[=true\|false]` | Inicia um servidor pessoal quando nenhum está rodando (padrão true; variável de ambiente `LEON_AUTOSTART`). |
| `--json` | Mostra JSON em vez de texto. |
| `-s, --session SESSION` | Sessão alvo (padrão `$LEON_SESSION`). |
| `-b, --block BLOCK` | Bloco alvo (padrão `$LEON_BLOCK`; se não, o bloco em foco da janela ativa). |
| `-H, --host HOST` | Age no servidor leon de outra máquina, por ssh: um rótulo de `leon hosts`, ou `ssh://[USER@]HOST[:PORT]`. Também `-S, --server`. Com ela, `$LEON_SESSION` e `$LEON_BLOCK` são ignorados (eles indicam objetos desta máquina). |
| `-h, --help` | Mostra a ajuda. |
| `-V, --version` | Mostra a versão. |

## Códigos de saída

| Código | Significado |
|---|---|
| 0 | ok |
| 1 | falhou |
| 2 | linha de comando inválida (`leon` com um erro de digitação sugere o comando mais próximo) |
| 3 | alvo não encontrado ou ambíguo |
| 4 | servidor inacessível |
| 10 | `self-update --check`: há uma versão mais nova |
| 130 | interrompido (Ctrl-C) |

`leon wait` e `leon run --wait` saem, em vez disso, com o status do próprio programa.

## O app

### `leon app`

Abre o app leon, conectado ao servidor: o que já está rodando (um iniciado pela CLI serve), ou um
que ele inicia antes, como faz `leon server start`. A CLI e os agentes controlam as mesmas sessões
que você vê. `leon` sem comando faz o mesmo. Fechar o app (⌘Q, fechar a janela) desconecta e todos
os programas continuam rodando; "Sair e encerrar todas as sessões…" (Quit and End All Sessions…)
no app ou `leon server stop` encerra tudo. Com `--autostart=false` e sem servidor, sai com 4.

Um build sem o app (por enquanto o Linux, ou o macOS com `--no-default-features`) não tem janela:
`leon` sozinho roda `leon attach` (com `-s` e `-H` como foram passados) quando stdin e stdout são
um terminal, e, se não, mostra a ajuda e sai com 0 (sai com 2 se `-s` ou `-H` foi passado);
`leon app` sai com 1 e explica o motivo.

## Em um terminal

### `leon attach [TARGET]`

Use o leon dentro de qualquer terminal, localmente ou por ssh (`ssh -t HOST leon attach`; sem `-t`
não há terminal e ele sai com 1). Sem TARGET (e sem `-s`), abre um seletor com todas as sessões, as
abas delas e os blocos, com uma prévia ao vivo da tela do bloco selecionado: digite para filtrar
(letras em ordem, como na paleta de comandos), ↑ e ↓ (ou ctrl+p, ctrl+n) movem a seleção, Enter conecta, Esc
volta ou sai. TARGET é uma sessão, ou um bloco, cuja aba então abre com ele em foco. Se não houver
nenhuma sessão, Enter cria uma.

Conectado, ele mostra a aba ativa da sessão: cada painel do layout dividido com bordas entre eles,
e uma barra de status com a sessão e as abas dela. As teclas vão para o painel em foco, menos a
tecla de prefixo e os atalhos da sua configuração (`init.lua`; uma tecla `cmd` nunca chega de um
terminal). Depois do prefixo (`--prefix KEY`; se não, o `leon.mode("attach", { prefix = … })` da
configuração; se não, ctrl+b):

| Tecla | Faz |
|---|---|
| `d` | Desconectar: `leon attach` sai com 0 e todos os programas continuam rodando |
| ←↑↓→ ou `h` `j` `k` `l` | Focar o painel nessa direção |
| `%`, `"` | Dividir o painel à direita, para baixo |
| `z` | Ampliar o painel (de novo para desfazer) |
| `c` | Nova aba |
| `n`, `p` | Próxima aba, aba anterior |
| `s` | Voltar ao seletor |
| o prefixo de novo | Digitar a tecla de prefixo no painel |

Os painéis são dimensionados para caber neste terminal sempre que o layout que este terminal tem deles muda (ao
conectar, ao redimensionar, ao dividir ou ampliar, quando um painel volta para a tela); um painel
que outro cliente redimensionou desde então é cortado ou completado. Ao sair (desconexão, o
servidor parando, SIGHUP, SIGTERM, SIGINT, SIGQUIT), o terminal é devolvido como estava; um
processo watchdog faz isso mesmo depois de um SIGKILL. Se for encerrado por um sinal, sai com 128 + o
sinal.

Com `-H`, a barra de status diz `reconnecting to HOST (try N)…` enquanto o leon tenta reconectar
uma conexão que caiu, e as teclas chegam ao painel inteiras ou não chegam. Depois de um segundo sem notícias do
host, as teclas digitadas esperam uma verificação rápida da conexão antes de sair; se ela falhar,
ou se a conexão estiver se restabelecendo, elas são descartadas, e a barra diz quantas teclas não
foram enviadas. Digite de novo quando a conexão voltar. Um texto colado espera na fila junto com as
teclas. Um limite: um travamento que começa dentro desse segundo, no meio de uma palavra, ainda pode
pegar as teclas que já estavam a caminho, e elas podem chegar atrasadas.

## Sessões e janelas

`leon new`, `ls`, `kill`, `leon session` e `leon window` estão em [cli/sessions.md](../../cli/sessions.md) (em inglês).

## Rodar programas

### `leon run [FLAGS] -- CMD…`

Roda um comando em uma janela nova e mostra o ID do bloco novo. O comando roda diretamente, não
por um shell; para pipes ou `&&`, use `leon run -- sh -c '…'`. Ele é procurado no PATH do seu shell
de login, mesmo quando o servidor foi iniciado pelo Leon.app a partir do Finder.

- `--label LABEL`: dá um nome ao bloco, para `-b LABEL` encontrá-lo.
- `--keep-open`: mantém o bloco depois que o programa termina, para você ainda ler a saída.
- `--wait`: espera o programa e sai com o status dele.
- `--output[=true|false]`: com `--wait`, mostra o texto do programa depois da linha do ID quando
  ele termina: o que `leon capture --trim` mostraria, histórico e tela. Ligado por padrão quando o
  stdout não é um terminal (o `$(…)` de um agente ou de um script), desligado em um terminal. Para
  ler o texto, o `run` mantém o bloco aberto até ele ser lido, depois o fecha (a menos que haja
  `--keep-open`); um `run --wait` encerrado antes disso deixa o bloco aberto, como o
  `--keep-open` faria.
- `-c, --cwd DIR`: diretório de trabalho (padrão: o diretório atual; com `-H`, o home do host, e
  `DIR` precisa ser um caminho absoluto lá).

```sh
leon run --label tests --keep-open -- cargo test
leon run --wait -- make                       # in a script: make's output, then its status
leon run --wait --output=false -- make        # the status alone
```

### `leon split [FLAGS] [-- CMD…]`

Divide um bloco e roda `CMD` (padrão: um shell de login) no painel novo; mostra o ID do bloco
novo. Divide o `-b` (que também pode indicar uma divisão, para colocar o painel ao lado de um grupo
inteiro); se não, o bloco em foco.

- `--split right|left|above|below`: onde o painel novo fica (padrão right).
- `--ratio PCT`: a parte do espaço do painel novo, 1–99 (padrão 50).
- `--label LABEL`, `--keep-open`, `-c, --cwd DIR`: como no `leon run`.
- `--focus[=true|false]`: coloca o painel novo em foco (padrão true).

```sh
leon split
leon split --split below --ratio 30 --label logs -- tail -f /var/log/system.log
leon split --focus=false -b tests -- htop
```

### `leon send [TEXT]`

Digita `TEXT` em um terminal (padrão: lê do stdin). Nenhum Enter é adicionado; use em seguida
`leon send-key enter`.

### `leon send-key KEY…`

Pressiona teclas em ordem, escritas na sintaxe dos atalhos: `enter`, `escape`, `tab`, `space`,
`backspace`, `up`, `ctrl+c`, `alt+left`, `a`…

### `leon capture`

Mostra o histórico e a tela de um terminal como texto.

- `--trim`: remove espaços e linhas em branco do final.

### `leon wait [BLOCK]`

Espera o programa de um terminal terminar, e sai com o status dele (bloco padrão: `-b`; se não,
`$LEON_BLOCK`).

- `--for DURATION`: desiste depois desse tempo (`500ms`, `10s`, `2m`) e sai com 1.
- `--text TEXT`: espera, em vez disso, até a tela ou o histórico do bloco mostrar `TEXT`, e sai com
  0 (texto que já está lá conta). Sai com 1 depois do `--for`, ou se o bloco fechar antes. Para um
  programa que continua rodando: um prompt, a linha "ready" de um servidor de desenvolvimento.

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

Informa um status para o seu próprio bloco (`-b`; se não, `$LEON_BLOCK`; sem nenhum, sai com 2),
como um relatório OSC 7501 do programa dele faria: aparece na aba e no cabeçalho do painel, na
lista de Atividade (Activity) do app e, para blocked, error e o done de uma execução longa, como
notificação. Serve para ferramentas que não conseguem escrever OSC 7501 (um hook, um job sem tty).
Substitui o registro raiz do bloco, com a duração desse registro.

- `--state working|blocked|done|error`: o estado (padrão `done`).
- `--title T`, `--body B`: uma linha cada (no máximo 192 e 2048 bytes). Texto que o OSC 7501
  descartaria (caracteres de controle, quebras de linha, caracteres invisíveis) é recusado (sai com
  1), e nada é registrado.

```sh
id=$(leon run --keep-open -- sleep 30)
leon notify -b "$id" --state blocked --title "Deploy" --body "approve the rollout?"
leon block call -b "$id" program_status
leon block close "$id"
```

## Layout

`leon focus`, `move`, `swap`, `zoom` e `resize` estão em [cli/layout.md](../../cli/layout.md) (em inglês).

## Blocos

### `leon block`

Gerencia blocos (painéis de terminal).

### `leon block ls`

Lista os blocos de uma sessão.

### `leon block inspect [BLOCK]`

Mostra um bloco: a janela, o pid, o comando e o status de saída dele, e o comando para retomar, se
o programa dele registrou um (`set_resume`).

### `leon block close [BLOCK]`

Fecha um bloco e encerra o programa dele.

### `leon block call METHOD [ARGS]`

Chama um método de bloco, como `size` ou `program_status`, ou um nome completo como
`dev.leon.terminal.size`. `ARGS` é um objeto JSON, `@file`, ou `-` para stdin.

`set_resume` registra o comando que retoma o programa deste bloco depois de uma reinicialização
(`null` apaga); um bloco restaurado o oferece de acordo com a configuração `resume` (veja
[lua/settings.md](../../lua/settings.md), em inglês):

```sh
leon block call set_resume '{"command": ["claude", "--resume", "c7583d2c-3133-4d13-b5f0-45883f1cabcb"]}'
```

## Automação

`leon events`, `leon do`, `leon actions`, `leon keymap` e `leon api` estão em [cli/automation.md](../../cli/automation.md) (em inglês).

## Configuração e temas

`leon config` (check, reload, get, set) e `leon theme` (list, import, show) estão em
[cli/settings.md](../../cli/settings.md) (em inglês).

## Servidor

`leon server` (run, start, stop, status, proxy) está em [cli/server.md](../../cli/server.md) (em inglês).

## Hosts

`-H HOST` alcança o servidor leon de outra máquina pelo `ssh` do sistema. `leon hosts` (list, add,
remove, check; `--tailscale` adiciona uma máquina da sua tailnet) e como o `-H` funciona estão em
[cli/hosts.md](../../cli/hosts.md) (em inglês).

## Atualizações

### `leon self-update`

Atualiza o leon para a versão mais nova; `--check` só informa (sai com 10):
[updates.md](../../updates.md) (em inglês).

## Agentes

`leon skill` (show, setup, status, remove), `leon float`, `leon toggle` e `leon dock` estão em [cli/agents.md](../../cli/agents.md) (em inglês).

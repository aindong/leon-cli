<!-- i18n: source=docs/agents.md commit=28ab3cd4a46d -->
> Esta é uma tradução. A [página em inglês](../../agents.md) é a referência; ela pode estar mais atualizada.

# Agentes controlam o leon

Um agente de código (Claude Code, Codex) rodando em um terminal do leon pode fazer o mesmo que você
faz ali: abrir um painel ao lado dele mesmo, rodar os testes nele, esperar por eles, ler a tela,
digitar em um REPL e fechar o que abriu. Ele usa os mesmos comandos `leon` que você. Esta página
explica como ensinar isso a ele e como um agente deve se comportar depois que aprende.

## Configure uma vez

```sh
leon skill setup --dry-run   # show what it would write
leon skill setup             # install for every agent it finds
leon skill status            # current, outdated, missing, foreign or unreadable
```

`leon skill setup` instala duas coisas para cada agente que encontra: um cujo diretório de
configuração existe, ou cujo programa (`claude`, `codex`) está no seu PATH, mesmo que você nunca o
tenha rodado. Ele mostra, para cada agente, por que o escolheu e cada arquivo que gravou. Se não
encontrar nenhum, grava só o `~/.agents/skills/leon/` compartilhado (outros agentes também o leem)
e avisa. `--agent claude|codex` escolhe um:

| | Claude Code | Codex |
|---|---|---|
| A skill: um `SKILL.md` mínimo | `~/.claude/skills/leon/` (`$CLAUDE_CONFIG_DIR`) | `~/.agents/skills/leon/` |
| Um hook de início de sessão que roda `leon skill hook --agent …` | `~/.claude/settings.json` | `~/.codex/hooks.json` (`$CODEX_HOME`) |

- **A skill** diz ao agente para rodar `leon skill show`, que mostra o guia completo compilado
  dentro do seu binário `leon`. Atualizar o leon atualiza o guia; não há nada para reinstalar.
- **O hook** roda quando uma sessão do agente começa. Dentro do leon, ele informa ao agente a
  sessão e o bloco de terminal dele, e registra como retomar essa sessão (veja abaixo); fora do
  leon, ele não mostra nada e nunca contata o servidor. Um hook de um leon mais antigo aparece como
  `outdated`: rode `leon skill setup`.

A configuração pode ser rodada de novo sem risco. Ela edita os arquivos de configuração no lugar,
mantendo suas chaves e a ordem delas, faz um backup do arquivo uma vez em `<file>.leon-backup` e
recusa um arquivo que não tenha o formato JSON esperado. Ela não substitui um `SKILL.md` que não
gravou, a menos que você passe `--force` (o antigo fica guardado como `SKILL.md.leon-backup`).

```sh
leon skill show                    # read the guide the agent reads
leon skill remove --dry-run
leon skill remove                  # removes what setup installed, and nothing else
```

## Onde estou: `LEON_SESSION` e `LEON_BLOCK`

Todo terminal do leon tem duas variáveis:

- `LEON_SESSION`: o ID da sessão, por exemplo `session:3f2a…`.
- `LEON_BLOCK`: o ID deste próprio terminal, por exemplo `block:91c0…`.

Os comandos do leon usam essas variáveis por padrão, então o `leon run` de um agente abre uma aba
na *própria* sessão dele, ao lado do resto do trabalho do usuário, sem nenhuma opção. A sessão dele
é a que contém o `LEON_BLOCK` agora: um painel movido para outra sessão (⌘↩ no seletor do app)
mantém o `LEON_SESSION` com que começou, porque o ambiente de um programa em execução não pode
mudar, então o leon procura o bloco e usa `LEON_SESSION` só quando `LEON_BLOCK` não está definido
ou não existe mais. Sem nenhuma das duas variáveis, o agente não está dentro do leon: ele deve rodar
`leon ls` e perguntar qual sessão usar.

Como um agente deve interpretar as palavras do usuário:

- "aqui", "ao seu lado", "nesta sessão": a própria sessão dele, a que contém o `LEON_BLOCK`.
- "o painel dos testes", "a sessão api": um nome dado pelo usuário sempre vence os padrões;
  passe-o com `-b` ou `-s`.
- Se um nome corresponde a mais de uma coisa, o leon lista as opções e sai com 3: pergunte, não
  adivinhe.

**Uma armadilha:** `send`, `send-key` e `capture` sem `-b` agem no `LEON_BLOCK`, que é o terminal
do próprio agente. Digitar ali é digitar na própria entrada do agente. Sempre passe `-b` com o bloco
que você quer.

## Uma sessão do ponto de vista do agente

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

As regras que o guia dá aos agentes:

- Coloque rótulo em cada bloco que você criar, para o usuário distinguir os seus dos dele.
- Feche o que você criou quando terminar; nunca feche os blocos do usuário nem o seu próprio
  `LEON_BLOCK`.
- Não leia nem digite nos terminais do usuário a menos que ele peça: as telas guardam segredos, e
  as teclas que você envia rodam no shell dele.
- Para um programa que continua rodando (um servidor de desenvolvimento, um REPL), não espere ele
  terminar: espere o texto que você espera ver, `leon wait -b ID --text TEXT --for 30s`.
- `leon run --wait -- CMD` a partir de um agente mostra o texto do programa e sai com o status
  dele, em uma única chamada.
- Use `--json` para a saída que você vai analisar, e `leon api list` / `leon api describe` para
  tudo o que os comandos não cobrem.

## Os agentes voltam depois de uma reinicialização

Quando o servidor para (`leon server stop`, logout, reinicialização da máquina), o texto de cada
bloco é salvo, e um bloco restaurado o mostra acima de uma linha esmaecida
`— restored from <time> —` e de um shell novo. Os programas não sobrevivem, e um comando salvo
nunca é rodado de novo. O que volta é um comando que um programa registrou para o próprio bloco com
`set_resume`. O hook faz isso por você: `claude --resume <id>` para o Claude Code,
`codex resume <id>` para o Codex, a partir do id de sessão que o agente passa ao hook. Ele só
registra para um agente em primeiro plano no terminal, então um `claude -p` aninhado, rodado pela
ferramenta de outro agente, não substitui o comando da sessão interativa, e nunca no modo
print do Claude Code (`-p`, `--print`): uma execução avulsa não tem nada para retomar.

Qualquer outro programa pode registrar o seu:

```sh
leon block call set_resume -b "$LEON_BLOCK" '{"command": ["my-agent", "--resume", "42"]}'
leon block inspect "$LEON_BLOCK"        # shows "resume: my-agent --resume 42"
leon block call set_resume -b "$LEON_BLOCK" '{"command": null}'
```

Na restauração, o bloco segue a configuração `resume` do `init.lua` (`leon.set("resume", "ask")`):
`"ask"` (o padrão) digita o comando no novo prompt e espera o Enter (Ctrl-C pula), `"always"` roda o
comando, `"never"` o ignora. Um comando é oferecido em uma única reinicialização: depois de digitado
ele não é salvo de novo, então um comando pulado ou desatualizado não volta toda vez. O hook de um
agente retomado o registra de novo.

## Status do programa: deixe o usuário ver o que o agente está fazendo

Agentes e ferramentas podem informar o estado deles com OSC 7501 (`working`, `blocked`, `done`,
`error`), que o leon acompanha por terminal. `leon block call program_status -b "$LEON_BLOCK"` lê o
estado de volta, e cada mudança é um evento `program_status_changed` em `leon events -s SESSION`.

### O formato OSC 7501

Um relatório é uma sequência de escape escrita no terminal: `ESC ] 7501 ;`, depois pares
`key=value` unidos por `:`, depois `BEL` (ou `ESC \`). Uma chave é formada por letras minúsculas;
um valor usa só letras, dígitos e `_ . , + / = -`. Um `id` tem até 8 segmentos unidos por `/`,
cada um com 1 a 32 letras, dígitos e `_ . + -`. Enviar `ESC ] 7501 ; ? BEL` pergunta o que o
terminal suporta; o leon responde
`ESC ] 7501 ; ? : states=idle,working,done,blocked,error : kinds=permission,question,auth ESC \`
(sem os espaços).

Por exemplo, `printf '\033]7501;state=working:progress=40\007'` diz que o programa está em 40%.

- **Chaves.** `state` (obrigatória): `idle` (nada a informar), `working`, `done`, `blocked`,
  `error` ou `clear`. `id` dá nome a um registro (sem ele é o registro raiz; `/` cria um filho).
  `kind` (só com `blocked`): `permission`, `question` ou `auth`. `progress`: de 0 a 100, com
  `working` ou `blocked` (sem ele significa desconhecido). `app`: `[A-Za-z0-9_.+-]{1,32}`, herdado
  do ancestral mais próximo que o define. `title` e `msg`: base64 de uma linha de UTF-8 sem
  caracteres de controle.
- **Regras.** Os espaços em branco são removidos das pontas, um par malformado é pulado, uma chave
  desconhecida é ignorada e, entre duplicatas, vale a última. Um `state` desconhecido ou um `id`
  inválido descarta o relatório inteiro (um `id` inválido nunca cai para a raiz). Cada relatório
  substitui o registro dele por inteiro; `clear` remove o registro e os filhos dele (sem `id`,
  todos os registros).
- **Duração.** `working` e `blocked` somem quando o processo termina ou um novo prompt começa
  (OSC 133 `A`). `done` e `error` ficam até você olhar o painel. Um reset completo (`RIS`) apaga os
  registros; um reset suave e a troca para a tela alternativa, não.
- **Limites.** Passando de qualquer um destes, o relatório é descartado inteiro: a sequência 4096
  bytes, uma chave 16, `msg` 2732 codificado (2048 decodificado), `title` 256 (192), `app` 32,
  `id` 128 no total, com 32 por segmento e 8 níveis de profundidade. Um terminal guarda 256
  registros e descarta o usado há mais tempo.

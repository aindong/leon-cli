<!-- i18n: source=docs/shortcuts.md commit=28ab3cd4a46d -->
> Esta é uma tradução. A [página em inglês](../../shortcuts.md) é a referência; ela pode estar mais atualizada.

# Atalhos do app

As teclas do Leon.app, a partir do [README](README.md#atalhos-do-app). [lua/keys.md](../../lua/keys.md) (em inglês) explica como mudá-las.

| Teclas | Ação |
|---|---|
| ⌘N | Nova sessão |
| ⌘T | Nova aba |
| ⌘D | Dividir à direita |
| ⌘⇧D | Dividir para baixo |
| ⌘⌥←, ⌘⌥→, ⌘⌥↑, ⌘⌥↓ | Focar o painel nessa direção |
| ⌘⇧↩ | Ampliar o painel (de novo para desfazer) |
| ⌘W | Fechar o painel |
| ⌘⌥F, ⌘⌥⇧F | Fazer o painel flutuar sobre os outros, devolvê-lo à grade (arraste o cabeçalho para mover, uma borda para redimensionar) |
| ⌘J | Mostrar ou esconder a gaveta de rascunho (scratch drawer): um shell na parte de baixo, aberto no diretório do painel em foco, que continua rodando enquanto está escondido |
| ⌘⌃←, ⌘⌃→, ⌘⌃↓ | Encaixar o painel como gaveta à esquerda, à direita ou embaixo (arraste a borda para redimensionar) |
| ⌘⇧], ⌘⇧[ | Próxima aba, aba anterior |
| ⌘1 … ⌘9 | Ir para a aba 1–9 |
| ⌘C, ⌘V | Copiar a seleção, colar |
| ⌘A | Selecionar tudo no painel, inclusive o histórico |
| ⌘F | Buscar no painel: ↩ e ⇧↩ avançam e voltam, ⌥C maiúsculas/minúsculas, ⌥R regex, esc fecha |
| ⌘G, ⌘⇧G | Próxima ocorrência, anterior |
| ⌘⇧C | Modo de cópia: teclas do vi sobre o histórico (h j k l, w b, 0 $, g ⇧G, ⌃U ⌃D), v e ⇧V selecionam, / e ? buscam com n e ⇧N, y copia, esc ou q saem |
| ⌘⇧H | Dicas (Hints): um rótulo em cada URL, caminho, hash, IP e número longo na tela; digite um para copiá-lo, em maiúsculas para digitá-lo no painel |
| ⇧PageUp, ⇧PageDown | Rolar uma página para trás, para a frente |
| ⌘↑, ⌘↓ | Rolar até a linha mais antiga, voltar ao fim |
| ⌘B | Mostrar ou esconder a barra lateral |
| ⌘⇧S | Abas na barra lateral ou na barra de título (fica salvo para a próxima abertura) |
| ⌘⇧P | Paleta de comandos: digite para encontrar uma ação, ↩ executa, esc fecha |
| ⌘, | Ajustes (Settings): todas as configurações em seções, com busca, ↑ e ↓ entre as seções, esc ou ⌘W fecha (gravadas no init.lua) |
| ⌘⇧T | Trocar o tema, mostrado ao vivo enquanto você navega; ↩ mantém, esc volta |
| ⌘K | Trocar para qualquer host › sessão › aba (ou um bloco pelo título): ↩ vai até lá, ⌘↩ move para o lado deste painel (mesma máquina) |
| ⌘⇧A | Atividade (Activity): cada bloco que precisa de você em cada host (bloqueado, erro, concluído, trabalhando); clique em um para ir até ele |
| ⌘⇧\\, ⌘\\ | Visão geral (Overview): todas as abas (⌘\\: as desta sessão), ao vivo; as setas e ↩ levam até elas, digitar filtra; parar sobre uma aba mostra uma prévia |
| ⌘=, ⌘⇧=, ⌘-, ⌘0 | Fonte maior, maior, menor, tamanho padrão (painéis visíveis) |
| ⌘Q | Sair; todos os programas continuam rodando (leon › Sair e encerrar todas as sessões… (Quit and End All Sessions…) encerra tudo) |

![A visão geral (⌘⇧\\): cada aba de cada sessão como uma miniatura ao vivo, com o status](../../images/app-overview.png)

Estes são os padrões. Um `leon.bind` no seu `init.lua` substitui um deles, tecla por tecla, e
`leon.unbind` remove um (⌘Q fica). Sequências de teclas como `ctrl+b>c` mostram uma dica do que
pode vir em seguida. Cada botão mostra a ação e o atalho ao passar o ponteiro. A roda do mouse rola
um painel pelo histórico; arraste para selecionar, option+arrastar para um retângulo; segure ⌘
sobre um link para abri-lo com um clique. Um programa que pede o mouse (um editor ou o `htop`)
recebe os cliques e a roda; segure shift para selecionar mesmo assim.

<!-- i18n: source=docs/remote-hosts.md commit=a525beb0ae17 -->
> Esta é uma tradução. A [página em inglês](../../remote-hosts.md) é a referência; ela pode estar mais atualizada.

# Hosts remotos

Como o leon trabalha com o servidor leon de outra máquina, depois dos primeiros passos do [README](README.md#trabalhe-na-sua-devbox). Os comandos estão em [cli/hosts.md](../../cli/hosts.md) (em inglês).

Quando uma conexão Wi-Fi cai, o leon tenta reconectar por um minuto: `leon events` e o app continuam sem perder nem
repetir nenhum evento, e o `leon attach` avisa, depois reconecta e redesenha a tela. As teclas
digitadas enquanto a conexão está travada ou fora do ar chegam inteiras ou não chegam, e a barra de
status diz quantas não foram enviadas (teclas enviadas bem quando um travamento começa podem
chegar). Use chaves ssh ou um agente ssh: cada conexão é uma sessão ssh própria.

O app mostra na barra lateral cada host de `leon hosts`, abaixo de Este Mac (This Mac), cada um com
suas sessões e abas e um ponto que indica a conexão. Um painel remoto funciona como um local, com o
nome do host no cabeçalho; Nova sessão (New Session), Nova aba (New Tab) e as divisões agem na
máquina do painel em foco, e a paleta tem Nova sessão em cada máquina e Adicionar host… (Add
Host…) (as máquinas da sua tailnet, ou um rótulo e depois um endpoint `ssh://`). Clique no rótulo
de um host para recolhê-lo. O ssh do app nunca pede senha. Um host que precisa de uma diz
`needs login: run leon hosts check devbox in a terminal`; faça isso uma vez (aceite a chave do
host, desbloqueie sua chave) e a próxima tentativa do app consegue entrar. Enquanto uma conexão
está fora do ar, os painéis do host mantêm as últimas telas sob um aviso de "reconnecting".

![A barra lateral com Este Mac e uma devbox, cada um com suas sessões e abas](../../images/m16b-hosts.png)

![Um painel na devbox, em foco, com o nome do host no cabeçalho](../../images/m16b-remote-pane.png)

![Um host que não pode ser alcançado, com o motivo dado pelo ssh abaixo do rótulo](../../images/m16b-unreachable.png)

![Adicionar host… listando as máquinas de uma tailnet, cada uma com o sistema e se está online](../../images/app-tailnet.png)

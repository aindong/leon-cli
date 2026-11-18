<!-- i18n: source=docs/recipes.md commit=28ab3cd4a46d -->
> Esta é uma tradução. A [página em inglês](../../recipes.md) é a referência; ela pode estar mais atualizada.

# Projetos e worktrees do git

[examples/project.lua](../../../examples/project.lua) define uma ação Lua `project_open`.
[examples/projects.lua](../../../examples/projects.lua) roda a mesma receita pelo `leon do`.
Os dois procuram uma sessão cujo rótulo corresponda ao nome da pasta do projeto, ou criam uma com
o primeiro terminal nesse diretório. A criação usa o `cwd` de `session.create`, o mesmo parâmetro
de `leon new --cwd DIR`. As regras sobre projetos e worktrees ficam nesses exemplos; copie e adapte
à sua necessidade.

Os exemplos rodam localmente: use um servidor local e pastas locais. Os caminhos dos scripts são
resolvidos a partir do diretório de quem chama; as ações exigem um `dir` absoluto, porque as ações
rodam no servidor. Os nomes de sessão são globais: uma sessão que já existe com o nome da pasta é
reaproveitada mesmo que o shell dela tenha mudado de diretório desde então. Rótulos de sessão
duplicados ou worktrees com o mesmo nome de pasta são erros; renomeie antes. Os nomes de pasta
precisam caber nos rótulos do leon (128 caracteres, sem caracteres de controle).

## Um projeto

A partir de um clone do leon, com o `leon` no PATH:

```sh
leon do ./examples/projects.lua dir="$PWD"
```

O script mostra o ID da sessão. Rode `leon attach SESSION_ID` para abri-la no seu terminal, ou
selecione essa sessão no app. Repetir o script mostra o mesmo ID e não cria nenhum terminal a mais.
Você pode passar outro projeto com `dir=/path/to/project` ou `dir=../project`.
Para uma sessão nova sem a receita, use `leon new project-name --cwd /path/to/project`.

## Cada worktree

```sh
leon do ./examples/projects.lua dir="$PWD" worktrees=true
```

A receita lê `git worktree list --porcelain -z` e mostra um ID de sessão para cada worktree
utilizável, incluindo o checkout principal. Os delimitadores NUL preservam espaços, aspas e quebras
de linha nos caminhos das pastas pai. Repositórios bare e worktrees marcados como prunable são
pulados. Ela não cria nem remove worktrees; use você mesmo `git worktree add` e
`git worktree remove`.

## Copiar um arquivo ignorado para um worktree novo

Depois de criar um worktree, você pode optar por copiar arquivos ignorados específicos antes de a
sessão dele ser criada. Aqui o `.env` precisa ser um arquivo comum ignorado na raiz de origem:

```sh
leon do ./examples/projects.lua dir="$PWD" worktrees=true \
  copy_from="$PWD" 'copy_files=[".env"]'
```

Só as sessões que estão sendo criadas rodam esse hook. Arquivos que já existem no destino não são
tocados; as cópias novas são privadas (modo 0600, dependendo do sistema de arquivos). Os nomes
precisam ser nomes de arquivo da raiz, sem `/` nem `..`; links simbólicos e arquivos que o Git não
considera ignorados são recusados. Por padrão, nenhum arquivo ignorado é copiado. `copy_from` é o
checkout de origem, e `copy_files` é um array JSON de nomes. Um hook que falha interrompe a receita
antes de criar aquela sessão; as sessões e cópias já criadas permanecem. Rode uma receita de cada
vez: listar e criar sessões são chamadas de API separadas.

## Instalar a ação

Copie `examples/project.lua` para o lado do seu `init.lua` e adicione esta configuração (troque a
pasta do atalho pelo caminho absoluto do seu próprio projeto):

```lua
require("project")
leon.bind("cmd+shift+o", "project_open", { dir = "/path/to/project" })
```

Recarregue com `leon config reload`. `leon do project_open dir=/path/to/project` devolve um array
de IDs de sessão; selecione uma com `leon attach SESSION_ID` ou no app. O atalho cria ou encontra
as sessões e deixa a escolha do cliente com você. A ação aceita os mesmos argumentos `worktrees`,
`copy_from` e `copy_files` que o script.

O módulo expõe `before_create(dir, args)` como hook de preparação. Substitua-o depois do `require`,
ou envolva-o para manter a cópia padrão de arquivos ignorados. Um hook deve falhar com
`error(message)` quando a preparação falha; ele roda antes de `session.create`, e uma ação tem o
prazo habitual do leon de 30 segundos.

Cada bloco `sh` e `lua` desta página roda em `m21_recipe_docs`; `m21_recipe` verifica o script e a
ação reais contra um servidor privado, incluindo dois worktrees do git e a cópia de arquivos
ignorados.

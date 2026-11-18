<!-- i18n: source=docs/install.md commit=a525beb0ae17 -->
> Esta é uma tradução. A [página em inglês](../../install.md) é a referência; ela pode estar mais atualizada.

# Compilar o leon a partir do código-fonte

Parte do [README](README.md#instalação).

Para compilar o leon você mesmo, você precisa de uma toolchain Rust ([rustup](https://rustup.rs)) e do
git. As duas seções abaixo compilam no macOS e no Linux.

## macOS

Você precisa do macOS 11 ou mais recente. A partir de um clone deste repositório:

```sh
scripts/install.sh
```

Isso compila o `Leon.app`, copia para `~/Applications` e cria um link do comando `leon` em
`~/.local/bin` (use `--prefix DIR` para `DIR/bin`). Ele nunca usa root, mostra cada coisa que faz,
pode ser rodado de novo sem risco e se recusa a substituir qualquer coisa nesses caminhos que não
tenha sido instalada por ele. `scripts/install.sh --uninstall` remove os dois. Abra o **Leon** pelo
Finder, pelo Spotlight ou pelo Dock, ou digite `leon`. Para compilar só o app,
`scripts/bundle.sh` gera `target/release/Leon.app`.

O app é um cliente do servidor leon. Ele se conecta ao servidor que está rodando (iniciado pelo
shell ou por uma abertura anterior) ou o inicia. Fechar o app deixa o servidor e seus programas
rodando; `leon server stop`, ou **Sair e encerrar todas as sessões…** (Quit and End All
Sessions…) no menu leon, encerra tudo.

## Linux

No Linux, o leon é o servidor, a CLI e o `leon attach` (os agentes e os scripts Lua também); o app
ainda não existe lá, então `leon` sozinho em um terminal roda `leon attach`. Compile com `cargo`:

```sh
cargo build --release -p leon
```

Isso gera um `target/release/leon` sem interface gráfica; coloque-o no seu `PATH`. Ele precisa da
glibc 2.34 ou mais recente (Debian 12, Ubuntu 22.04, RHEL 9 e posteriores). O socket fica em
`$XDG_RUNTIME_DIR/leon` quando a sua sessão define essa variável. Com Docker, você pode gerar
pacotes em vez disso:

```sh
scripts/package-linux.sh                 # or --arch amd64 / --arch arm64
```

Isso gera um tarball e um `.deb` em `target/linux/<arch>/` e verifica se os dois instalam e rodam
em contêineres limpos. Instale o `.deb` no Debian ou no Ubuntu com `dpkg` (como root):

```sh
dpkg -i target/linux/$(dpkg --print-architecture)/leon_*.deb
```

ou descompacte o tarball em qualquer lugar e coloque o `leon` dele no seu `PATH`:

```sh
tar xzf target/linux/*/leon-*-"$(uname -m)"-linux.tar.gz -C ~/.local
ln -s ~/.local/leon-*-linux/leon ~/.local/bin/leon
```

## Arquivos de uma versão

`scripts/release.sh` gera, a partir de um clone limpo de um commit, os arquivos que uma versão
traz: `Leon-<version>-macos-<arch>.zip` (Leon.app, com assinatura ad hoc e sem notarização), o
tarball da CLI para macOS, um tarball e um `.deb` para Linux em arm64 e x86_64, e `SHA256SUMS`. Ele
roda no macOS e precisa do Docker para os pacotes Linux. `leon --version` mostra a versão e o
commit a partir do qual ela foi compilada. Uma tag enviada ao GitHub faz o mesmo lá, assina cada
arquivo e publica a versão: [releases.md](../../releases.md) (em inglês).

<!-- i18n: source=docs/install.md commit=28ab3cd4a46d -->
> Esta es una traducción. La [página en inglés](../../install.md) es la referencia; puede estar más actualizada.

# Compilar leon desde el código fuente

Parte del [README](README.md#instalación).

Para compilar leon tú mismo necesitas una toolchain de Rust ([rustup](https://rustup.rs)) y git.
Las dos secciones siguientes lo compilan en macOS y en Linux.

## macOS

Necesitas macOS 11 o posterior. Desde una copia de este repositorio:

```sh
scripts/install.sh
```

Eso compila `Leon.app`, la copia en `~/Applications` y enlaza el comando `leon` en `~/.local/bin`
(usa `--prefix DIR` para `DIR/bin`). Nunca usa root, muestra cada cosa que hace, se puede volver a
ejecutar sin riesgo y se niega a reemplazar nada en esas rutas que no haya instalado él.
`scripts/install.sh --uninstall` quita las dos cosas. Abre **Leon** desde el Finder, Spotlight o el
Dock, o escribe `leon`. Para compilar solo la app, `scripts/bundle.sh` genera
`target/release/Leon.app`.

La app es un cliente del servidor leon. Se conecta al que está en marcha (lanzado desde la shell o
por una apertura anterior) o lo arranca. Cerrar la app deja el servidor y tus programas en marcha;
`leon server stop`, o **Salir y terminar todas las sesiones…** (Quit and End All Sessions…) en el
menú leon, los termina.

## Linux

En Linux, leon es el servidor, la CLI y `leon attach` (también los agentes y los scripts de Lua);
la app todavía no está, así que `leon` solo en una terminal ejecuta `leon attach`. Compílalo con
`cargo`:

```sh
cargo build --release -p leon
```

Eso te da un `target/release/leon` sin interfaz gráfica; ponlo en tu `PATH`. Necesita glibc 2.34
o más reciente (Debian 12, Ubuntu 22.04, RHEL 9 y posteriores). El socket va en
`$XDG_RUNTIME_DIR/leon` cuando tu sesión define esa variable. Con Docker puedes generar paquetes en
su lugar:

```sh
scripts/package-linux.sh                 # or --arch amd64 / --arch arm64
```

Eso genera un tarball y un `.deb` en `target/linux/<arch>/` y comprueba que ambos se instalan y se
ejecutan en contenedores limpios. Instala el `.deb` en Debian o Ubuntu con `dpkg` (como root):

```sh
dpkg -i target/linux/$(dpkg --print-architecture)/leon_*.deb
```

o descomprime el tarball en cualquier sitio y pon su `leon` en tu `PATH`:

```sh
tar xzf target/linux/*/leon-*-"$(uname -m)"-linux.tar.gz -C ~/.local
ln -s ~/.local/leon-*-linux/leon ~/.local/bin/leon
```

## Archivos de una versión

`scripts/release.sh` genera, desde un clon limpio de un commit, los archivos que trae una versión:
`Leon-<version>-macos-<arch>.zip` (Leon.app, con firma ad hoc y sin notarizar), el tarball de la CLI
para macOS, un tarball y un `.deb` para Linux en arm64 y x86_64, y `SHA256SUMS`. Funciona en macOS
y necesita Docker para los paquetes de Linux. `leon --version` indica la versión y el commit desde
el que se compiló. Un tag de git enviado a GitHub hace lo mismo allí, firma cada archivo y
publica la versión: [releases.md](../../releases.md) (en inglés).

<!-- i18n: source=README.md commit=b441b60b83aa -->
> Esta es una traducción. La [página en inglés](../../../README.md) es la referencia; puede estar más actualizada.

# leon

[English](../../../README.md) · [简体中文](../zh-CN/README.md) · [日本語](../ja/README.md) · [한국어](../ko/README.md) · Español · [Português (Brasil)](../pt-BR/README.md)

<p align="center"><img src="../../images/banner.png" alt="leon: mantén tu trabajo a la vista. El icono de la app, un león, junto a un boceto de paneles con un agente trabajando, pruebas terminadas y una revisión bloqueada." width="100%"></p>

**leon es un multiplexor de terminales para quien usa varios programas de terminal y agentes de
programación a la vez.** Un servidor mantiene en marcha tus sesiones, pestañas y paneles divididos,
y los programas que hay en ellos. Trabajas con ellos desde una app nativa de macOS, con el comando
`leon` en cualquier terminal, con scripts de Lua o a través de tu agente de programación, que puede
abrir un panel a su lado, ejecutar ahí tus pruebas y leer el resultado. Los programas pueden
indicar si están trabajando, bloqueados o terminados, así ves cuál te necesita. Funciona en macOS y
Linux (por ahora la app es solo para macOS) y está escrito en Rust.

![La app leon: dividir una pestaña, cambiar de pestaña, dividir desde la paleta de comandos, una ventana emergente de git, cambiar de sesión con ⌘K y un agente trabajando que luego queda bloqueado](../../images/demo-app.gif)

![El comando leon: una sesión nueva, una ejecución que espera, escribir en Python y esperar su respuesta, y luego leon attach y una división](../../images/demo-cli.gif)

## Funciones

- **Disposición persistente.** Los programas siguen en marcha cuando cierras la app: cerrar la
  ventana o ⌘Q solo desconecta, y la app vuelve a abrirse con las mismas sesiones, pestañas,
  divisiones y pantallas en vivo. Cuando el propio servidor se detiene, las sesiones, pestañas,
  divisiones y el directorio de cada panel se guardan y vuelven en su siguiente arranque, con los
  mismos IDs que usas en tus scripts (los programas que había en ellos no).
- **El comando `leon`.** `leon run`, `send`, `capture` y `wait` abren paneles, escriben en ellos,
  leen sus pantallas y esperan a los programas, con códigos de salida fijos y salida `--json`.
  `leon attach` muestra una sesión en cualquier terminal, también por ssh.
- **La app de macOS.** Pestañas en la barra lateral o en la barra de título, divisiones, paneles
  flotantes, un cajón auxiliar (scratch drawer), una paleta de comandos, un selector de sesiones,
  historial (scrollback) con búsqueda y modo copia, y ocho temas.
- **Agentes.** `leon skill setup` enseña a Claude Code y Codex a trabajar en paneles junto a sí
  mismos, y sus paneles ofrecen retomar la misma conversación después de un reinicio.
- **Estado y atención.** Un programa que escribe un informe OSC 7501 (los agentes lo hacen) muestra
  su estado en su pestaña y su panel. La lista de Actividad (Activity), el distintivo del Dock y las
  notificaciones te dicen qué está bloqueado, qué falló y qué terminó.
- **Hosts remotos.** `-H devbox` actúa sobre el servidor leon de otra máquina a través de tu propio
  `ssh`, y la app muestra las sesiones de cada host junto a las tuyas.
- **Lua.** Atajos de teclado, modos, tus propias acciones y scripts que reaccionan a eventos.
- **Una API.** Todos los clientes usan la misma API de socket, que se describe a sí misma
  (`leon api list`) y tiene una promesa de compatibilidad para quien escriba su propio cliente.

## Instalación

### Homebrew

```sh
brew install --cask aindong/leon/leon   # macOS: Leon.app, with the leon command
brew install aindong/leon/leon          # the CLI alone, on macOS or Linux
```

Elige uno: el `leon` del cask es el que viene dentro de la app. `brew upgrade` trae las versiones
nuevas. Apple no ha notarizado Leon.app, así que macOS no la abrirá mientras tenga la marca de
cuarentena que Homebrew pone en lo que descarga. Cuando el cask termine de instalarse, y antes de
abrir la app, quita esa marca:

```sh
xattr -dr com.apple.quarantine /Applications/Leon.app
```

### Desde una versión publicada

Cada versión está en <https://github.com/aindong/leon-cli/releases/latest>: `Leon-<version>-macos-<arch>.zip`
(Leon.app), un tarball de la CLI para macOS y otro para Linux, y un `.deb`, cada uno para arm64 y
x86_64. En macOS, descomprime la app en `/Applications` y ejecuta el comando `xattr` de arriba (un
archivo descargado con el navegador también queda en cuarentena). Si la abres antes, macOS la
rechaza (dice que no pudo verificar que Leon esté libre de malware); ejecuta el comando y vuelve a
abrirla. El tarball de la CLI para macOS también pasa la marca a su `leon`: después de
descomprimirlo, ejecuta `xattr -dr com.apple.quarantine` sobre su carpeta. En Linux, con el archivo
de tu máquina ya descargado:

```sh
dpkg -i leon_*.deb                               # Debian or Ubuntu, as root
mkdir -p ~/.local/bin                            # anywhere else: make the folder,
tar xzf leon-*-linux.tar.gz -C ~/.local          # unpack it,
ln -s ~/.local/leon-*-linux/leon ~/.local/bin/   # and put its leon on your PATH
```

Todos los archivos están firmados: [docs/releases.md](../../releases.md#checking-a-download)
explica cómo comprobar uno.

### Actualizar

```sh
leon self-update --check   # is there a newer release?
leon self-update           # download it, check its signature, and replace this leon
```

leon solo se actualiza cuando tú lo pides, y comprueba la firma de la versión antes de instalar
nada. Un leon de Homebrew se actualiza con `brew upgrade`, y uno de un `.deb` instalando el `.deb`
nuevo; `leon self-update` te dice cuál es tu caso. Un servidor en marcha sigue con la versión
anterior hasta `leon server stop`. Los detalles están en [docs/updates.md](../../updates.md).

### Desde el código fuente

Para compilar leon tú mismo, `scripts/install.sh` en macOS o `cargo` en Linux:
[docs/install.md](install.md) tiene los pasos y los archivos que trae una versión.

## Los primeros cinco comandos

Una guía rápida de dos minutos, desde cualquier terminal (el servidor arranca solo). Python hace
de ejemplo de cualquier programa que quieras manejar:

```sh
leon new demo                                    # 1. a session called "demo"
leon -s demo run --label py -- python3 -q        # 2. run a program in a new tab
leon -s demo wait -b py --text '>>>'             #    and wait for its prompt
leon -s demo send -b py 'print(6 * 7)'           # 3. type into it (no Enter yet)
leon -s demo send-key -b py enter                # 4. press Enter
leon -s demo wait -b py --text 42                #    and wait for the answer
leon -s demo capture -b py --trim                # 5. read its screen: 42
```

Los destinos (`-s` sesión, `-b` bloque, el nombre que leon da a un panel) son un ID, una etiqueta,
una posición o el final de un ID. Dentro de una terminal de leon puedes omitir `-s`: los comandos
actúan sobre tu propia sesión. `leon ls` lista las sesiones; `leon --help` lo lista todo, con los
códigos de salida al final.

## Divídelo

```sh
leon -s demo split -b py --split below --ratio 30 -- top     # a pane under py, 30% high
leon -s demo focus up
leon -s demo zoom                                            # fill the window; again to undo
```

En la app: ⌘D divide a la derecha, ⌘⇧D divide hacia abajo, ⌘⌥ más una flecha te mueve entre
paneles, y arrastrar un divisor cambia el tamaño. `leon attach demo` muestra la sesión en esta
terminal (ctrl+b d te desconecta). Cuando termines, `leon kill demo` cierra la sesión y sus
programas.

## En cualquier terminal: `leon attach`

```text
leon attach                  # pick a session, tab or block (type to filter), Enter attaches
leon attach demo             # straight to the session "demo"
ssh -t box leon attach       # the same, on another machine's leon
```

`leon attach` muestra los paneles divididos de la pestaña y una barra de estado, y envía tus teclas
al panel con el foco. Después del prefijo, ctrl+b (`--prefix` o `leon.mode("attach", { prefix })`
en la configuración para cambiarlo):

| Teclas | Acción |
|---|---|
| ctrl+b d | Desconectar; todos los programas siguen en marcha |
| ctrl+b ←↑↓→, ctrl+b h/j/k/l | Pasar el foco al panel en esa dirección |
| ctrl+b %, ctrl+b " | Dividir a la derecha, dividir hacia abajo |
| ctrl+b z | Ampliar el panel (otra vez para deshacer) |
| ctrl+b c | Pestaña nueva |
| ctrl+b n, ctrl+b p | Pestaña siguiente, anterior |
| ctrl+b s | Volver al selector |

Los atajos de tu `init.lua` también funcionan ahí, salvo las teclas con `cmd` (una terminal nunca
las envía) y un atajo sobre la propia tecla del prefijo.

## Trabaja en tu devbox

Los comandos que hablan con el servidor aceptan `-H` para actuar en cambio sobre el servidor leon
de otra máquina, a través de tu propio `ssh` (sin cuenta, relay ni VPN; `~/.ssh/config` y
`ProxyJump` funcionan). Pon `leon` en el PATH de la otra máquina o en `~/.local/bin`; el primer
comando que llegue ahí arranca su servidor, y los programas siguen en marcha cuando te desconectas.

```text
leon hosts add devbox ssh://me@devbox.example.com
leon -H devbox run -c /home/me/app -- make test   # runs on the devbox, in its /home/me/app
leon -H devbox attach               # its sessions, in this terminal
leon hosts check
leon hosts add --tailscale          # from your tailnet (Tailscale SSH: no keys)
```

Las conexiones caídas, las teclas que escribes mientras una está caída y los hosts en la barra
lateral de la app se explican en [docs/remote-hosts.md](remote-hosts.md).


## Deja que un agente lo maneje

```sh
leon skill setup
```

instala una skill y un hook de inicio de sesión para Claude Code y Codex (para cada uno cuyo
directorio de configuración exista o cuyo programa esté en tu PATH; te dice cuáles y por qué).
Después, trabajando en una terminal de leon, pide "ejecuta las pruebas a tu lado", y el agente hace
lo que harías tú:

```sh
id=$(leon split --label tests --keep-open -- cargo test)
leon wait "$id"
leon capture -b "$id" --trim | tail -20
leon block close "$id"
```

Sabe dónde está gracias a `LEON_SESSION` y `LEON_BLOCK`, pone etiqueta a lo que abre y cierra solo
lo que creó. Consulta [docs/agents.md](agents.md).

## Mira qué te necesita

Los programas informan si están trabajando, bloqueados o terminados, y la app lo muestra en sus
pestañas y paneles, en la lista de Actividad (Activity, ⌘⇧A), en el Dock y en notificaciones.
[docs/shortcuts.md](../../shortcuts.md#know-what-needs-you) (en inglés) tiene los detalles y los ajustes.

## Atajos de la app

Todas las teclas de la app, de ⌘N a ⌘Q, y cómo cambiar una, están en
[docs/shortcuts.md](shortcuts.md).


## Configuración

leon lee `~/.config/leon/init.lua` (o `$LEON_CONFIG`). Los ajustes tienen sus propios comandos, y
la ventana de Ajustes (Settings) de la app (⌘,) escribe en el mismo archivo:

```sh
leon config set font.size 15
leon config set theme dark=leon-dark,light=leon-light
leon theme list
leon config check
```

El mismo archivo define atajos de teclado, modos y tus propias acciones, y `leon do` ejecuta
scripts:

```lua
leon.action{
  name = "name_session",
  title = "Name this session",
  args = { label = "string" },
  run = function(ctx, args) return leon.session.set_label{ label = args.label } end,
}
```

```sh
leon config reload
leon do name_session label=api
leon do -e 'print(#leon.session.view().windows .. " tabs")'
```

`leon theme import FILE` trae el tema de otra terminal. Consulta [docs/lua.md](lua.md) y
[docs/cli/settings.md](../../cli/settings.md).

## Hoja de ruta

leon 0.1.0 es la primera versión. Lo que ya está hecho, hito por hito, está en
[docs/roadmap.md](../../roadmap.md) y [CHANGELOG.md](../../../CHANGELOG.md). Lo que viene:

- **La app en Linux.** Linux tiene el servidor, la CLI, `leon attach`, los agentes y Lua; la app
  GPUI es solo para macOS por ahora.
- **Dos clientes en una pestaña.** Con la app y `leon attach` mostrando la misma pestaña, cada
  panel toma el tamaño del cliente que cambió su disposición por última vez, y el otro cliente lo
  recorta o lo rellena.
- **Enlaces con hosts que imitan a otros.** ⌘-clic abre un enlace `http(s)` sin preguntar,
  incluso cuando su host usa letras no ASCII que se parecen a las de otro; solo los demás tipos de
  enlace muestran antes su dirección.
- **Ejecuciones de `codex exec`.** leon distingue una ejecución puntual de `claude -p` de un agente
  interactivo, pero todavía no una ejecución de `codex exec`, a la que aún se le puede ofrecer
  retomar algo pensado para una sesión interactiva.

## Documentación

- [docs/cli.md](cli.md): todos los comandos y opciones.
- [docs/lua.md](lua.md): la configuración y los scripts, con ejemplos que ejecuta una prueba.
- [docs/agents.md](agents.md): agentes que manejan leon, y el formato de estado OSC 7501.
- [docs/recipes.md](recipes.md): sesiones por proyecto y por worktree de git, como receta de Lua.
- [docs/integrators.md](../../integrators.md) (en inglés): cómo escribir un cliente para la API de
  socket de leon en cualquier lenguaje, y la promesa de compatibilidad.
  [docs/api.md](../../api.md) (en inglés): todos los métodos.
- [docs/architecture.md](../../architecture.md) (en inglés): cómo está construido.
  [docs/performance.md](../../performance.md) (en inglés): lo que cuesta, medido.
- [docs/releases.md](../../releases.md) (en inglés): cómo se prepara y se firma una versión, y el
  formato del manifiesto.
- [CHANGELOG.md](../../../CHANGELOG.md) (en inglés): qué cambió.

## Compilar y probar

```sh
cargo build
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

GPUI se compila con su función `runtime_shaders`, así que no necesitas la toolchain de Metal de
Xcode. Las pruebas de terminal ejecutan una shell real en un PTY, y las pruebas de la documentación
ejecutan los comandos de estas páginas. `scripts/linux-test.sh` ejecuta en Linux, dentro de Docker,
las pruebas de todo menos la app. Las pruebas de versiones firman con una clave desechable, así que
necesitan `minisign` y `jq`.

## Contribuir

Los informes de errores, las ideas, el código y las traducciones son bienvenidos.
[CONTRIBUTING.md](../../../CONTRIBUTING.md) (en inglés) explica cómo preparar el entorno, las normas
de la casa y cómo se incorpora un cambio; [docs/i18n/README.md](../README.md) (en inglés) explica
cómo traducir el README y las guías. Todas las personas que participan siguen el
[Código de conducta](../../../CODE_OF_CONDUCT.md) (Code of Conduct, en inglés), y un problema de
seguridad se informa en privado, como indica [SECURITY.md](../../../SECURITY.md).

## Licencia

[MIT](../../../LICENSE). La fuente Instrument Sans de `brand/fonts` está bajo la
[SIL Open Font License](../../../brand/fonts/OFL.txt).

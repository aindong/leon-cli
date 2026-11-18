<!-- i18n: source=docs/cli.md commit=14af788efc95 -->
> Esta es una traducción. La [página en inglés](../../cli.md) es la referencia; puede estar más actualizada.

# Referencia de comandos de leon

Todos los comandos y opciones de `leon`. `leon <command> --help` muestra el mismo texto en versión
corta; una prueba (`crates/leon/tests/m7b_cli_doc.rs`) falla si a la página en inglés y a sus partes en
`docs/cli/` les falta un comando u opción que `--help` muestre.

## Cómo encuentran las cosas los comandos

leon guarda **sesiones**; una sesión contiene **ventanas** (pestañas); una ventana contiene
**bloques** (paneles de terminal) en una disposición dividida. Cada objeto tiene un ID estable,
como `block:91c0…`.

- **Destinos.** `-s` elige una sesión y `-b` un bloque. Un destino puede ser un ID completo, una
  etiqueta (sin distinguir mayúsculas), una posición empezando en 1 (las ventanas se cuentan en el
  orden de las pestañas) o los últimos 4 o más caracteres de un ID. Si un destino coincide con más
  de una cosa, leon lista las coincidencias y sale con 3.
- **Valores por defecto.** Dentro de una terminal de leon, `LEON_SESSION` y `LEON_BLOCK` indican tu
  propia sesión y tu bloque, así que los comandos actúan "aquí" sin opciones. Tu sesión es la que
  contiene `LEON_BLOCK` en este momento (un panel puede unirse a otra sesión); si no,
  `LEON_SESSION`. Fuera de leon, pasa `-s`/`-b`; entonces los comandos de una sesión actúan sobre el
  bloque con el foco de su ventana activa.
- **El servidor.** Cualquier comando que necesite el servidor arranca uno personal si no hay
  ninguno en marcha (`--autostart=false` o `LEON_AUTOSTART=0` lo desactiva). El socket vive en
  `$LEON_RUNTIME_DIR`, si no en `$XDG_RUNTIME_DIR/leon`, si no en `~/.local/state/leon/run`; su ruta
  tiene que caber en un socket Unix (menos de 104 bytes en macOS, 108 en Linux), o leon lo dice y
  se detiene.
- **Otra máquina.** `-H HOST` envía cualquier comando de cliente al servidor leon de otra máquina, a
  través de tu propio `ssh` (consulta [cli/hosts.md](../../cli/hosts.md), en inglés).

## Opciones globales

Funcionan con todos los comandos.

| Opción | Qué hace |
|---|---|
| `--autostart[=true\|false]` | Arranca un servidor personal cuando no hay ninguno en marcha (por defecto true; variable de entorno `LEON_AUTOSTART`). |
| `--json` | Muestra JSON en lugar de texto. |
| `-s, --session SESSION` | Sesión de destino (por defecto `$LEON_SESSION`). |
| `-b, --block BLOCK` | Bloque de destino (por defecto `$LEON_BLOCK`; si no, el bloque con el foco de la ventana activa). |
| `-H, --host HOST` | Actúa sobre el servidor leon de otra máquina, por ssh: una etiqueta de `leon hosts`, o `ssh://[USER@]HOST[:PORT]`. También `-S, --server`. Con ella se ignoran `$LEON_SESSION` y `$LEON_BLOCK` (nombran objetos de esta máquina). |
| `-h, --help` | Muestra la ayuda. |
| `-V, --version` | Muestra la versión. |

## Códigos de salida

| Código | Significado |
|---|---|
| 0 | correcto |
| 1 | falló |
| 2 | línea de comandos incorrecta (`leon` con una errata sugiere el comando más parecido) |
| 3 | destino no encontrado o ambiguo |
| 4 | no se puede contactar con el servidor |
| 10 | `self-update --check`: hay una versión más nueva |
| 130 | interrumpido (Ctrl-C) |

`leon wait` y `leon run --wait` salen en cambio con el estado del propio programa.

## La app

### `leon app`

Abre la app leon, conectada al servidor: el que ya está en marcha (vale uno que haya arrancado la
CLI), o uno que arranca antes, como hace `leon server start`. La CLI y los agentes manejan las
mismas sesiones que ves. `leon` sin comando hace lo mismo. Cerrar la app (⌘Q, cerrar la ventana)
desconecta y todos los programas siguen en marcha; "Salir y terminar todas las sesiones…" (Quit and
End All Sessions…) de la app o `leon server stop` los terminan. Con `--autostart=false` y sin
servidor, sale con 4.

Una compilación sin la app (por ahora Linux, o macOS con `--no-default-features`) no tiene
ventana: `leon` solo ejecuta `leon attach` (con `-s` y `-H` tal como se den) cuando stdin y stdout
son una terminal, y, si no, muestra la ayuda y sale con 0 (sale con 2 si se dio `-s` o `-H`);
`leon app` sale con 1 y lo explica.

## En una terminal

### `leon attach [TARGET]`

Usa leon dentro de cualquier terminal, en local o por ssh (`ssh -t HOST leon attach`; sin `-t` no
hay terminal y sale con 1). Sin TARGET (y sin `-s`) abre un selector con todas las sesiones, sus
pestañas y sus bloques, con una vista previa en vivo de la pantalla del bloque seleccionado:
escribe para filtrar (letras en orden, igual que en la paleta de comandos), ↑ y ↓ (o ctrl+p,
ctrl+n) mueven la selección, Enter conecta, Esc vuelve atrás o sale. TARGET es una sesión, o un bloque, cuya
pestaña se abre entonces con él en foco. Si no hay ninguna sesión, Enter crea una.

Una vez conectado, muestra la pestaña activa de la sesión: cada panel de la disposición dividida
con bordes entre ellos, y una barra de estado con la sesión y sus pestañas. Las teclas van al panel
con el foco, salvo la tecla del prefijo y los atajos de tu configuración (`init.lua`; una tecla
`cmd` nunca llega desde una terminal). Después del prefijo (`--prefix KEY`; si no, el
`leon.mode("attach", { prefix = … })` de la configuración; si no, ctrl+b):

| Tecla | Hace |
|---|---|
| `d` | Desconectar: `leon attach` sale con 0 y todos los programas siguen en marcha |
| ←↑↓→ o `h` `j` `k` `l` | Pasar el foco al panel en esa dirección |
| `%`, `"` | Dividir el panel a la derecha, hacia abajo |
| `z` | Ampliar el panel (otra vez para deshacer) |
| `c` | Pestaña nueva |
| `n`, `p` | Pestaña siguiente, anterior |
| `s` | Volver al selector |
| el prefijo otra vez | Escribir la tecla del prefijo en el panel |

Los paneles se ajustan al tamaño de esta terminal cada vez que cambia la disposición que esta
terminal tiene de ellos (al conectarse, al cambiar de tamaño, al dividir o ampliar, cuando un panel vuelve a la
pantalla); un panel que otro cliente cambió de tamaño desde entonces se recorta o se rellena. Al
salir (desconexión, el servidor se detiene, SIGHUP, SIGTERM, SIGINT, SIGQUIT) la terminal se
devuelve tal como estaba; un proceso vigilante lo hace incluso después de SIGKILL. Si lo termina
una señal, sale con 128 + la señal.

Con `-H`, la barra de estado dice `reconnecting to HOST (try N)…` mientras se reintenta una
conexión caída, y las teclas llegan al panel completas o no llegan. Tras un segundo sin noticias
del host, las teclas que escribes esperan a una comprobación rápida de la conexión antes de salir;
si falla, o la conexión se está restableciendo, se descartan, y la barra dice cuántas teclas no se
enviaron. Vuelve a escribirlas cuando la conexión vuelva. Un pegado espera en la cola con las
teclas. Un límite: un atasco que empieza dentro de ese segundo, a mitad de una palabra, todavía
puede atrapar las teclas que ya estaban en camino, y esas pueden llegar tarde.

## Sesiones y ventanas

`leon new`, `ls`, `kill`, `leon session` y `leon window` están en [cli/sessions.md](../../cli/sessions.md) (en inglés).

## Ejecutar programas

### `leon run [FLAGS] -- CMD…`

Ejecuta un comando en una ventana nueva y muestra el ID del bloque nuevo. El comando se ejecuta
directamente, no a través de una shell; para tuberías o `&&`, usa `leon run -- sh -c '…'`. Se busca
en el PATH de tu shell de inicio de sesión, incluso cuando el servidor lo arrancó Leon.app desde el
Finder.

- `--label LABEL`: pone nombre al bloque, para que `-b LABEL` lo encuentre.
- `--keep-open`: mantiene el bloque después de que el programa termine, para que puedas seguir
  leyendo su salida.
- `--wait`: espera al programa y sale con su estado.
- `--output[=true|false]`: con `--wait`, muestra el texto del programa después de la línea del ID
  cuando termina: lo que mostraría `leon capture --trim`, historial y pantalla. Activado por
  defecto cuando stdout no es una terminal (el `$(…)` de un agente o de un script), desactivado en
  una terminal. Para leer el texto, `run` mantiene el bloque abierto hasta que se ha leído y luego
  lo cierra (salvo con `--keep-open`); un `run --wait` terminado antes de eso deja su bloque
  abierto, como haría `--keep-open`.
- `-c, --cwd DIR`: directorio de trabajo (por defecto: el directorio actual; con `-H`, el home del
  host, y `DIR` tiene que ser una ruta absoluta allí).

```sh
leon run --label tests --keep-open -- cargo test
leon run --wait -- make                       # in a script: make's output, then its status
leon run --wait --output=false -- make        # the status alone
```

### `leon split [FLAGS] [-- CMD…]`

Divide un bloque y ejecuta `CMD` (por defecto: una shell de inicio de sesión) en el panel nuevo;
muestra el ID del bloque nuevo. Divide `-b` (que también puede nombrar una división, para poner el
panel junto a un grupo entero); si no, el bloque con el foco.

- `--split right|left|above|below`: dónde va el panel nuevo (por defecto right).
- `--ratio PCT`: la parte del espacio que ocupa el panel nuevo, 1–99 (por defecto 50).
- `--label LABEL`, `--keep-open`, `-c, --cwd DIR`: como en `leon run`.
- `--focus[=true|false]`: da el foco al panel nuevo (por defecto true).

```sh
leon split
leon split --split below --ratio 30 --label logs -- tail -f /var/log/system.log
leon split --focus=false -b tests -- htop
```

### `leon send [TEXT]`

Escribe `TEXT` en una terminal (por defecto: lo lee de stdin). No añade Enter; sigue con
`leon send-key enter`.

### `leon send-key KEY…`

Pulsa teclas en orden, escritas con la sintaxis de los atajos: `enter`, `escape`, `tab`, `space`,
`backspace`, `up`, `ctrl+c`, `alt+left`, `a`…

### `leon capture`

Muestra el historial y la pantalla de una terminal como texto.

- `--trim`: quita los espacios y las líneas en blanco del final.

### `leon wait [BLOCK]`

Espera a que el programa de una terminal termine, y sale con su estado (bloque por defecto: `-b`;
si no, `$LEON_BLOCK`).

- `--for DURATION`: se rinde pasado ese tiempo (`500ms`, `10s`, `2m`) y sale con 1.
- `--text TEXT`: espera en cambio hasta que la pantalla o el historial del bloque muestren `TEXT`, y
  sale con 0 (el texto que ya está ahí cuenta). Sale con 1 pasado `--for`, o si el bloque se cierra
  antes. Para un programa que sigue en marcha: un prompt, la línea "ready" de un servidor de
  desarrollo.

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

Informa de un estado para tu propio bloque (`-b`; si no, `$LEON_BLOCK`; sin ninguno, sale con 2),
como lo haría un informe OSC 7501 de su programa: se muestra en la pestaña y en la cabecera del
panel, en la lista de Actividad (Activity) de la app y, para blocked, error y el done de una
ejecución larga, como notificación. Es para herramientas que no pueden escribir OSC 7501 (un hook,
un trabajo sin tty). Reemplaza el registro raíz del bloque, con la duración de ese registro.

- `--state working|blocked|done|error`: el estado (por defecto `done`).
- `--title T`, `--body B`: una línea cada uno (como mucho 192 y 2048 bytes). El texto que OSC 7501
  descartaría (caracteres de control, saltos de línea, caracteres invisibles) se rechaza (sale con
  1), y no se registra nada.

```sh
id=$(leon run --keep-open -- sleep 30)
leon notify -b "$id" --state blocked --title "Deploy" --body "approve the rollout?"
leon block call -b "$id" program_status
leon block close "$id"
```

## Disposición

`leon focus`, `move`, `swap`, `zoom` y `resize` están en [cli/layout.md](../../cli/layout.md) (en inglés).

## Bloques

### `leon block`

Gestiona los bloques (paneles de terminal).

### `leon block ls`

Lista los bloques de una sesión.

### `leon block inspect [BLOCK]`

Muestra un bloque: su ventana, pid, comando y estado de salida, y su comando para retomar si su
programa registró uno (`set_resume`).

### `leon block close [BLOCK]`

Cierra un bloque y termina su programa.

### `leon block call METHOD [ARGS]`

Llama a un método de bloque, como `size` o `program_status`, o un nombre completo como
`dev.leon.terminal.size`. `ARGS` es un objeto JSON, `@file`, o `-` para stdin.

`set_resume` registra el comando que retoma el programa de este bloque después de un reinicio
(`null` lo borra); un bloque restaurado lo ofrece según el ajuste `resume` de la configuración
(consulta [lua/settings.md](../../lua/settings.md), en inglés):

```sh
leon block call set_resume '{"command": ["claude", "--resume", "c7583d2c-3133-4d13-b5f0-45883f1cabcb"]}'
```

## Automatización

`leon events`, `leon do`, `leon actions`, `leon keymap` y `leon api` están en [cli/automation.md](../../cli/automation.md) (en inglés).

## Configuración y temas

`leon config` (check, reload, get, set) y `leon theme` (list, import, show) están en
[cli/settings.md](../../cli/settings.md) (en inglés).

## Servidor

`leon server` (run, start, stop, status, proxy) está en [cli/server.md](../../cli/server.md) (en inglés).

## Hosts

`-H HOST` llega al servidor leon de otra máquina a través del `ssh` del sistema. `leon hosts`
(list, add, remove, check; `--tailscale` añade una máquina de tu tailnet) y cómo funciona `-H`
están en [cli/hosts.md](../../cli/hosts.md) (en inglés).

## Actualizaciones

### `leon self-update`

Actualiza leon a la versión más nueva; `--check` solo informa (sale con 10):
[updates.md](../../updates.md) (en inglés).

## Agentes

`leon skill` (show, setup, status, remove), `leon float`, `leon toggle` y `leon dock` están en [cli/agents.md](../../cli/agents.md) (en inglés).

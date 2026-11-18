<!-- i18n: source=docs/agents.md commit=28ab3cd4a46d -->
> Esta es una traducción. La [página en inglés](../../agents.md) es la referencia; puede estar más actualizada.

# Los agentes manejan leon

Un agente de programación (Claude Code, Codex) que funciona en una terminal de leon puede hacer lo
mismo que haces tú ahí: abrir un panel a su lado, ejecutar las pruebas en él, esperarlas, leer la
pantalla, escribir en un REPL y cerrar lo que abrió. Usa los mismos comandos `leon` que tú. Esta
página explica cómo enseñárselo y cómo debe comportarse un agente una vez que lo sabe.

## Configúralo una vez

```sh
leon skill setup --dry-run   # show what it would write
leon skill setup             # install for every agent it finds
leon skill status            # current, outdated, missing, foreign or unreadable
```

`leon skill setup` instala dos cosas para cada agente que encuentra: uno cuyo directorio de
configuración existe, o cuyo programa (`claude`, `codex`) está en tu PATH, aunque nunca lo hayas
ejecutado. Muestra, para cada agente, por qué lo eligió y cada archivo que escribió. Si no encuentra
ninguno, escribe solo el `~/.agents/skills/leon/` compartido (otros agentes también lo leen) y lo
dice. `--agent claude|codex` elige uno:

| | Claude Code | Codex |
|---|---|---|
| La skill: un `SKILL.md` mínimo | `~/.claude/skills/leon/` (`$CLAUDE_CONFIG_DIR`) | `~/.agents/skills/leon/` |
| Un hook de inicio de sesión que ejecuta `leon skill hook --agent …` | `~/.claude/settings.json` | `~/.codex/hooks.json` (`$CODEX_HOME`) |

- **La skill** le dice al agente que ejecute `leon skill show`, que muestra la guía completa
  compilada dentro de tu binario `leon`. Actualizar leon actualiza la guía; no hay nada que volver
  a instalar.
- **El hook** se ejecuta cuando empieza una sesión del agente. Dentro de leon le dice al agente su
  sesión y su bloque de terminal, y registra cómo retomar esa sesión (más abajo); fuera de leon no
  muestra nada y nunca contacta con el servidor. Un hook de un leon más antiguo aparece como
  `outdated`: ejecuta `leon skill setup`.

Se puede volver a ejecutar la configuración sin riesgo. Edita los archivos de ajustes en su sitio,
conservando tus claves y su orden, guarda una copia de seguridad del archivo una vez en
`<file>.leon-backup` y rechaza un archivo que no tenga la forma JSON que espera. No reemplaza un
`SKILL.md` que no escribió a menos que pases `--force` (el anterior se guarda como
`SKILL.md.leon-backup`).

```sh
leon skill show                    # read the guide the agent reads
leon skill remove --dry-run
leon skill remove                  # removes what setup installed, and nothing else
```

## ¿Dónde estoy?: `LEON_SESSION` y `LEON_BLOCK`

Cada terminal de leon tiene dos variables:

- `LEON_SESSION`: el ID de la sesión, p. ej. `session:3f2a…`.
- `LEON_BLOCK`: el ID de esta misma terminal, p. ej. `block:91c0…`.

Los comandos de leon las usan por defecto, así que el `leon run` de un agente abre una pestaña en
*su propia* sesión, junto al resto del trabajo del usuario, sin ninguna opción. Su propia sesión es
la que contiene `LEON_BLOCK` en este momento: un panel movido a otra sesión (⌘↩ en el selector de
la app) conserva el `LEON_SESSION` con el que empezó, porque el entorno de un programa en marcha no
puede cambiar, así que leon busca el bloque y usa `LEON_SESSION` solo cuando `LEON_BLOCK` no está
definido o ya no existe. Sin ninguna de las dos variables, el agente no está dentro de leon: debe
ejecutar `leon ls` y preguntar qué sesión usar.

Cómo debe interpretar un agente las palabras del usuario:

- "aquí", "a tu lado", "en esta sesión": su propia sesión, la que contiene `LEON_BLOCK`.
- "el panel de las pruebas", "la sesión api": un nombre que da el usuario siempre gana a los valores
  por defecto; pásalo con `-b` o `-s`.
- Si un nombre coincide con más de una cosa, leon las lista y sale con 3: pregunta, no adivines.

**Una trampa:** `send`, `send-key` y `capture` sin `-b` actúan sobre `LEON_BLOCK`, que es la propia
terminal del agente. Escribir ahí es escribir en la propia entrada del agente. Pasa siempre `-b`
con el bloque que quieres.

## Una sesión vista desde el agente

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

Las reglas que la guía da a los agentes:

- Pon etiqueta a cada bloque que crees, para que el usuario distinga los tuyos de los suyos.
- Cierra lo que creaste cuando termines; nunca cierres los bloques del usuario ni tu propio
  `LEON_BLOCK`.
- No leas ni escribas en las terminales del usuario a menos que te lo pida: las pantallas guardan
  secretos, y las teclas que envías se ejecutan en su shell.
- Con un programa que sigue en marcha (un servidor de desarrollo, un REPL), no esperes a que
  termine: espera el texto que esperas ver, `leon wait -b ID --text TEXT --for 30s`.
- `leon run --wait -- CMD` desde un agente muestra el texto del programa y sale con su estado, en
  una sola llamada.
- Usa `--json` para la salida que vayas a analizar, y `leon api list` / `leon api describe` para
  todo lo que los comandos no cubren.

## Los agentes vuelven después de un reinicio

Cuando el servidor se detiene (`leon server stop`, cerrar sesión, reiniciar el equipo), se guarda
el texto de cada bloque, y un bloque restaurado lo muestra encima de una línea tenue
`— restored from <time> —` y una shell nueva. Los programas no sobreviven, y un comando guardado
nunca se vuelve a ejecutar. Lo que sí vuelve es un comando que un programa registró para su propio
bloque con `set_resume`. El hook lo hace por ti: `claude --resume <id>` para Claude Code,
`codex resume <id>` para Codex, a partir del id de sesión que el agente pasa a su hook. Solo lo
registra para un agente en primer plano en su terminal, así que un `claude -p` anidado, ejecutado
por la herramienta de otro agente, no reemplaza el comando de la sesión interactiva, y nunca en
el modo print de Claude Code (`-p`, `--print`): una ejecución puntual no tiene nada que retomar.

Cualquier otro programa puede registrar el suyo:

```sh
leon block call set_resume -b "$LEON_BLOCK" '{"command": ["my-agent", "--resume", "42"]}'
leon block inspect "$LEON_BLOCK"        # shows "resume: my-agent --resume 42"
leon block call set_resume -b "$LEON_BLOCK" '{"command": null}'
```

Al restaurar, el bloque sigue el ajuste `resume` de `init.lua` (`leon.set("resume", "ask")`):
`"ask"` (el valor por defecto) escribe el comando en el nuevo prompt y espera a Enter (Ctrl-C lo
omite), `"always"` lo ejecuta, `"never"` lo ignora. Un comando se ofrece en un solo reinicio: una
vez escrito no se vuelve a guardar, así que uno omitido u obsoleto no vuelve cada vez. El hook de un
agente retomado lo registra de nuevo.

## Estado del programa: deja que el usuario vea qué hace el agente

Los agentes y las herramientas pueden informar de su estado con OSC 7501 (`working`, `blocked`,
`done`, `error`), que leon sigue por terminal. `leon block call program_status -b "$LEON_BLOCK"` lo
devuelve, y cada cambio es un evento `program_status_changed` en `leon events -s SESSION`.

### El formato OSC 7501

Un informe es una secuencia de escape escrita en la terminal: `ESC ] 7501 ;`, luego pares
`key=value` unidos por `:`, luego `BEL` (o `ESC \`). Una clave son letras minúsculas; un valor usa
solo letras, dígitos y `_ . , + / = -`. Un `id` son hasta 8 segmentos unidos por `/`, cada uno de 1
a 32 letras, dígitos y `_ . + -`. Enviar `ESC ] 7501 ; ? BEL` pregunta qué admite la terminal; leon
responde `ESC ] 7501 ; ? : states=idle,working,done,blocked,error : kinds=permission,question,auth ESC \`
(sin los espacios).

Por ejemplo, `printf '\033]7501;state=working:progress=40\007'` dice que el programa va por el 40%.

- **Claves.** `state` (obligatoria): `idle` (nada que informar), `working`, `done`, `blocked`,
  `error` o `clear`. `id` da nombre a un registro (sin él es el registro raíz; `/` crea un hijo).
  `kind` (solo con `blocked`): `permission`, `question` o `auth`. `progress`: de 0 a 100, con
  `working` o `blocked` (sin él significa desconocido). `app`: `[A-Za-z0-9_.+-]{1,32}`, tomado del
  ancestro más cercano que lo defina. `title` y `msg`: base64 de una línea de UTF-8 sin caracteres
  de control.
- **Reglas.** Los espacios en blanco se recortan, un par mal formado se omite, una clave
  desconocida se ignora y, si hay duplicados, gana el último. Un `state` desconocido o un `id`
  incorrecto descarta el informe entero (un `id` incorrecto nunca recurre a la raíz). Cada informe
  reemplaza su registro entero; `clear` quita el registro y sus hijos (sin `id`, todos los
  registros).
- **Duración.** `working` y `blocked` desaparecen cuando el proceso termina o empieza un prompt
  nuevo (OSC 133 `A`). `done` y `error` se quedan hasta que miras el panel. Un reinicio completo
  (`RIS`) borra los registros; un reinicio suave y el cambio a la pantalla alternativa no.
- **Límites.** Si se supera cualquiera de estos, el informe se descarta entero: la secuencia 4096
  bytes, una clave 16, `msg` 2732 codificado (2048 decodificado), `title` 256 (192), `app` 32, `id`
  128 en total con 32 por segmento y 8 niveles de profundidad. Una terminal guarda 256 registros y
  descarta el que lleva más tiempo sin usarse.

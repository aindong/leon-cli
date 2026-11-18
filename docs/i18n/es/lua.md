<!-- i18n: source=docs/lua.md commit=28ab3cd4a46d -->
> Esta es una traducción. La [página en inglés](../../lua.md) es la referencia; puede estar más actualizada.

# Lua: configuración y scripts

leon habla Lua 5.1 en dos lugares:

- **La configuración**, `init.lua`, que el servidor carga al arrancar: atajos de teclado, modos y
  tus propias acciones. Compruébala con `leon config check`; aplica los cambios con
  `leon config reload`.
- **Los scripts**, que `leon do` ejecuta en tu máquina contra el servidor: automatización puntual,
  o un vigilante de larga duración que reacciona a eventos.

Los dos comparten las funciones `leon.*` ([la lista](../../lua/scripts.md#function-list), en
inglés). Cada ejemplo de estas páginas lo ejecuta una prueba (`crates/leon/tests/m7b_lua_doc.rs`).

## Dónde vive la configuración

El servidor lee el primero de: `server run --config PATH`, `$LEON_CONFIG`,
`$XDG_CONFIG_HOME/leon/init.lua`, `~/.config/leon/init.lua`. Un archivo que no existe es una
configuración vacía. `require` carga archivos que estén junto a él.

```sh
leon config check
leon config reload
leon keymap
leon actions
```

Una configuración se ejecuta de arriba abajo en cada carga. Una llamada `leon.*` incorrecta se
omite con un mensaje `file:line` y el resto se carga igualmente; un error de sintaxis hace fallar
toda la carga, y el servidor conserva la última configuración válida. `leon config check` sale con
1 en ambos casos.

## Las páginas

- [Keys and modes](../../lua/keys.md) (teclas y modos, en inglés): `leon.bind`, secuencias, teclas que se
  repiten, un preset al estilo de tmux, vim-navigator, los modos y el prefijo de `leon attach`,
  atajos con funciones, paneles flotantes y una tecla para una shell auxiliar.
- [Your own actions](../../lua/actions.md) (tus propias acciones, en inglés): `leon.action`,
  `leon.session.<method>` y `leon.call`.
- [Settings](../../lua/settings.md) (ajustes, en inglés): cada ajuste de `leon.set`, sus valores y su valor
  por defecto.
- [Scripts and the functions](../../lua/scripts.md) (los scripts y las funciones, en inglés): `leon do`,
  eventos, temporizadores, `leon.exec`, la lista de funciones `leon.*` y los límites.

## Sesiones por proyecto

Consulta [Proyectos y worktrees de git](recipes.md) para ver una acción de ejemplo y un script de
`leon do` que reutilizan una sesión por carpeta, con sesiones opcionales por worktree y un hook de
preparación que copia archivos ignorados.

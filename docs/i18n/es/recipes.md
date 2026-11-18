<!-- i18n: source=docs/recipes.md commit=a525beb0ae17 -->
> Esta es una traducción. La [página en inglés](../../recipes.md) es la referencia; puede estar más actualizada.

# Proyectos y worktrees de git

[examples/project.lua](../../../examples/project.lua) define una acción de Lua `project_open`.
[examples/projects.lua](../../../examples/projects.lua) ejecuta la misma receta con `leon do`.
Las dos buscan una sesión cuya etiqueta coincida con el nombre de la carpeta del proyecto, o crean
una con su primera terminal en ese directorio. La creación usa el `cwd` de `session.create`, el
mismo parámetro que `leon new --cwd DIR`. Las reglas sobre proyectos y worktrees viven en estos
ejemplos; cópialos y adáptalos.

Los ejemplos funcionan en local: usa un servidor local y carpetas locales. Las rutas de los
scripts se resuelven respecto al directorio de quien los llama; las acciones exigen un `dir`
absoluto, porque se ejecutan en el servidor. Los nombres de sesión son globales: una sesión que ya
existe con el nombre de la carpeta se reutiliza aunque su shell haya cambiado de directorio desde
entonces. Las etiquetas de sesión duplicadas o los worktrees con el mismo nombre de carpeta son
errores; renómbralos antes. Los nombres de carpeta tienen que caber en las etiquetas de leon (128
caracteres, sin caracteres de control).

## Un proyecto

Desde una copia de leon, con `leon` en el PATH:

```sh
leon do ./examples/projects.lua dir="$PWD"
```

El script muestra el ID de la sesión. Ejecuta `leon attach SESSION_ID` para abrirla en tu terminal,
o selecciona esa sesión en la app. Repetir el script muestra el mismo ID y no crea ninguna terminal
más. Puedes pasar otro proyecto con `dir=/path/to/project` o `dir=../project`.
Para una sesión nueva sin la receta, usa `leon new project-name --cwd /path/to/project`.

## Cada worktree

```sh
leon do ./examples/projects.lua dir="$PWD" worktrees=true
```

La receta lee `git worktree list --porcelain -z` y muestra un ID de sesión por cada worktree
utilizable, incluida la copia principal. Los delimitadores NUL conservan espacios, comillas y
saltos de línea en las rutas de las carpetas padre. Los repositorios bare y los worktrees marcados
como prunable se omiten. No crea ni quita worktrees; usa tú mismo `git worktree add` y
`git worktree remove`.

## Copiar un archivo ignorado a un worktree nuevo

Después de crear un worktree, puedes elegir copiar archivos ignorados concretos antes de que se cree
su sesión. Aquí `.env` tiene que ser un archivo normal ignorado en la raíz de origen:

```sh
leon do ./examples/projects.lua dir="$PWD" worktrees=true \
  copy_from="$PWD" 'copy_files=[".env"]'
```

Solo las sesiones que se están creando ejecutan este hook. Los archivos que ya existen en el
destino no se tocan; las copias nuevas son privadas (modo 0600, según lo permita el sistema de
archivos). Los nombres tienen que ser nombres de archivo de la raíz, sin `/` ni `..`; se rechazan
los enlaces simbólicos y los archivos que Git no considera ignorados. Por defecto no se copia
ningún archivo ignorado. `copy_from` es la copia de origen, y `copy_files` es un array JSON de
nombres. Un hook que falla detiene la receta antes de crear esa sesión; las sesiones y las copias
ya creadas se quedan. Ejecuta una receta a la vez: listar y crear sesiones son llamadas a la API
distintas.

## Instalar la acción

Copia `examples/project.lua` junto a tu `init.lua` y añade esta configuración (cambia la carpeta
del atajo por la ruta absoluta de tu propio proyecto):

```lua
require("project")
leon.bind("cmd+shift+o", "project_open", { dir = "/path/to/project" })
```

Recarga con `leon config reload`. `leon do project_open dir=/path/to/project` devuelve un array de
IDs de sesión; selecciona una con `leon attach SESSION_ID` o en la app. El atajo crea o encuentra
las sesiones y te deja a ti elegir el cliente. La acción acepta los mismos argumentos `worktrees`,
`copy_from` y `copy_files` que el script.

El módulo expone `before_create(dir, args)` como hook de preparación. Reemplázalo después del
`require`, o envuélvelo para conservar la copia por defecto de archivos ignorados. Un hook debe
fallar con `error(message)` cuando la preparación falla; se ejecuta antes de `session.create`, y una
acción tiene el plazo habitual de leon de 30 segundos.

Cada bloque `sh` y `lua` de esta página se ejecuta en `m21_recipe_docs`; `m21_recipe` comprueba el
script y la acción reales contra un servidor privado, incluidos dos worktrees de git y la copia de
archivos ignorados.

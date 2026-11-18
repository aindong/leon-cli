<!-- i18n: source=docs/shortcuts.md commit=28ab3cd4a46d -->
> Esta es una traducción. La [página en inglés](../../shortcuts.md) es la referencia; puede estar más actualizada.

# Atajos de la app

Las teclas de Leon.app, desde el [README](README.md#atajos-de-la-app). [lua/keys.md](../../lua/keys.md) (en inglés) explica cómo cambiarlas.

| Teclas | Acción |
|---|---|
| ⌘N | Sesión nueva |
| ⌘T | Pestaña nueva |
| ⌘D | Dividir a la derecha |
| ⌘⇧D | Dividir hacia abajo |
| ⌘⌥←, ⌘⌥→, ⌘⌥↑, ⌘⌥↓ | Pasar el foco al panel en esa dirección |
| ⌘⇧↩ | Ampliar el panel (otra vez para deshacer) |
| ⌘W | Cerrar el panel |
| ⌘⌥F, ⌘⌥⇧F | Hacer flotar el panel sobre los demás, devolverlo al mosaico (arrastra su cabecera para moverlo, un borde para cambiar su tamaño) |
| ⌘J | Mostrar u ocultar el cajón auxiliar (scratch drawer): una shell abajo, abierta en el directorio del panel con el foco, que sigue en marcha mientras está oculta |
| ⌘⌃←, ⌘⌃→, ⌘⌃↓ | Acoplar el panel como cajón a la izquierda, a la derecha o abajo (arrastra su borde para cambiar su tamaño) |
| ⌘⇧], ⌘⇧[ | Pestaña siguiente, anterior |
| ⌘1 … ⌘9 | Ir a la pestaña 1–9 |
| ⌘C, ⌘V | Copiar la selección, pegar |
| ⌘A | Seleccionar todo el panel, historial incluido |
| ⌘F | Buscar en el panel: ↩ y ⇧↩ avanzan y retroceden, ⌥C mayúsculas/minúsculas, ⌥R regex, esc cierra |
| ⌘G, ⌘⇧G | Coincidencia siguiente, anterior |
| ⌘⇧C | Modo copia: teclas de vi sobre el historial (h j k l, w b, 0 $, g ⇧G, ⌃U ⌃D), v y ⇧V seleccionan, / y ? buscan con n y ⇧N, y copia, esc o q salen |
| ⌘⇧H | Pistas (Hints): una etiqueta en cada URL, ruta, hash, IP y número largo de la pantalla; escribe una para copiarla, en mayúsculas para escribirla en el panel |
| ⇧PageUp, ⇧PageDown | Desplazarse una página atrás, adelante |
| ⌘↑, ⌘↓ | Ir a la línea más antigua, volver al final |
| ⌘B | Mostrar u ocultar la barra lateral |
| ⌘⇧S | Pestañas en la barra lateral o en la barra de título (se guarda para la siguiente apertura) |
| ⌘⇧P | Paleta de comandos: escribe para encontrar una acción, ↩ la ejecuta, esc cierra |
| ⌘, | Ajustes (Settings): todos los ajustes por secciones, con búsqueda, ↑ y ↓ entre secciones, esc o ⌘W cierra (se guardan en init.lua) |
| ⌘⇧T | Cambiar el tema, que se ve en vivo mientras te mueves; ↩ lo conserva, esc vuelve atrás |
| ⌘K | Cambiar a cualquier host › sesión › pestaña (o a un bloque por su título): ↩ va allí, ⌘↩ lo mueve junto a este panel (misma máquina) |
| ⌘⇧A | Actividad (Activity): cada bloque que te necesita en cada host (bloqueado, error, terminado, trabajando); haz clic en uno para ir allí |
| ⌘⇧\\, ⌘\\ | Vista general (Overview): todas las pestañas (⌘\\: las de esta sesión), en vivo; las flechas y ↩ te llevan, escribir filtra; detenerse en una pestaña muestra su vista previa |
| ⌘=, ⌘⇧=, ⌘-, ⌘0 | Fuente más grande, más grande, más pequeña, tamaño por defecto (paneles visibles) |
| ⌘Q | Salir; todos los programas siguen en marcha (leon › Salir y terminar todas las sesiones… (Quit and End All Sessions…) los termina) |

![La vista general (⌘⇧\\): cada pestaña de cada sesión como un mosaico en vivo, con su estado](../../images/app-overview.png)

Estos son los valores por defecto. Un `leon.bind` en tu `init.lua` reemplaza uno, tecla por tecla,
y `leon.unbind` quita uno (⌘Q se queda). Las secuencias de teclas como `ctrl+b>c` muestran una pista
de lo que puede seguir. Cada botón muestra su acción y su atajo al pasar el puntero. La rueda del
ratón desplaza un panel por su historial; arrastra para seleccionar, option+arrastrar para un
rectángulo; mantén ⌘ sobre un enlace para abrirlo con un clic. Un programa que pide el ratón (un
editor o `htop`) recibe los clics y la rueda; mantén shift para seleccionar de todos modos.

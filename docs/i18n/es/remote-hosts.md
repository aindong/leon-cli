<!-- i18n: source=docs/remote-hosts.md commit=28ab3cd4a46d -->
> Esta es una traducción. La [página en inglés](../../remote-hosts.md) es la referencia; puede estar más actualizada.

# Hosts remotos

Cómo trabaja leon con el servidor leon de otra máquina, después de los primeros pasos del [README](README.md#trabaja-en-tu-devbox). Los comandos están en [cli/hosts.md](../../cli/hosts.md) (en inglés).

Una conexión Wi-Fi caída se reintenta durante un minuto: `leon events` y la app continúan sin
perder ni repetir ningún evento, y `leon attach` lo avisa, luego se reconecta y vuelve a dibujar.
Las teclas que escribes mientras la conexión está atascada o caída llegan completas o no llegan, y
la barra de estado dice cuántas no se enviaron (las teclas enviadas justo cuando empieza un atasco
pueden llegar). Usa claves ssh o un agente de ssh: cada conexión es su propia sesión de ssh.

La app muestra en la barra lateral cada host de `leon hosts`, bajo Este Mac (This Mac), cada uno con
sus sesiones y pestañas y un punto que indica su conexión. Un panel remoto funciona como uno local,
con el nombre del host en su cabecera; Sesión nueva (New Session), Pestaña nueva (New Tab) y las
divisiones actúan sobre la máquina del panel con el foco, y la paleta tiene Sesión nueva en cada
máquina y Añadir host… (Add Host…) (las máquinas de tu tailnet, o una etiqueta y luego un endpoint
`ssh://`). Haz clic en la etiqueta de un host para plegarlo. El ssh de la app nunca pide
contraseña. Un host que la necesita dice
`needs login: run leon hosts check devbox in a terminal`; hazlo una vez (acepta la clave del host,
desbloquea tu clave) y el siguiente intento de la app entra. Mientras una conexión está caída, los
paneles del host conservan sus últimas pantallas bajo una nota de "reconnecting".

![La barra lateral con Este Mac y una devbox, cada uno con sus sesiones y pestañas](../../images/m16b-hosts.png)

![Un panel en la devbox, con el foco y el nombre del host en su cabecera](../../images/m16b-remote-pane.png)

![Un host al que no se puede llegar, con el motivo que da ssh bajo su etiqueta](../../images/m16b-unreachable.png)

![Añadir host… con la lista de las máquinas de una tailnet, cada una con su sistema y si está en línea](../../images/app-tailnet.png)

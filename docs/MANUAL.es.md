# Manual de usuario

Este manual explica cómo jugar a Quake con este port, centrándose en lo que
añade respecto al Quake original y a otros ports. Todo lo nuevo es
opcional: con los ajustes predeterminados, el juego se ve y se juega como
Quake.

## Primeros pasos

1. Consigue tu propia copia de Quake. Indica al port la carpeta que
   contiene `id1/` (y los mods a su lado), con `QUAKE_BASEDIR` o
   `-basedir <carpeta>`.
2. Ejecuta `cargo run --release`, o inicia directamente un mod con
   `-game <carpeta> +map <mapa>`.
3. La configuración, las partidas guardadas y las capturas están en
   `~/.local/share/quake-source-port/<juego>/`, una carpeta por mod.

El menú principal ofrece Single Player, Multiplayer, Options, **Help**
(este manual), Quit y **Paquetes de mapas**, con el tema de Quake de fondo
(Options → Audio → Música del menú principal). Todos los menús funcionan
con ratón, teclado, mando y pantalla táctil; pasa por encima de una opción
o selecciónala para ver una descripción emergente.

En la ayuda, arriba/abajo elige una sección; RePág/AvPág,
izquierda/derecha, los botones superiores o la rueda del ratón sobre el
texto lo desplazan.

## Idioma

Options → Juego → **Idioma** elige el idioma de los menús, los mensajes en
pantalla y este manual: inglés, francés, alemán, italiano, español, ruso,
polaco, ucraniano, checo, eslovaco, croata, búlgaro, sueco, finés, danés,
noruego, neerlandés, griego, húngaro, rumano, portugués (Portugal o
Brasil), japonés, coreano, chino (simplificado o tradicional), turco,
indonesio, malayo, filipino, vietnamita, serbio, lituano, letón,
estonio, maltés, albanés, armenio, georgiano,
islandés, irlandés, galés, vasco, catalán, gallego, latín, esperanto, bielorruso, macedonio, esloveno o suajili. **Sistema** sigue el idioma de tu
ordenador. Los textos de juego de la reedición de 2021 solo están
traducidos al francés, alemán, italiano, español y ruso; los demás idiomas
los muestran en inglés. En los idiomas distintos del inglés, los textos
pequeños del juego usan la fuente de los menús, porque la fuente clásica de
Quake solo tiene letras latinas sin tildes. Los mensajes de la consola
siguen en inglés salvo que actives **Traducir la consola**.

## Paquetes de mapas

Menú principal → Paquetes de mapas muestra cada mod y paquete de mapas
instalado con su descripción. Elige uno para cambiar a él, explora sus
mapas e inicia cualquiera directamente. El navegador guarda tus mejores
bajas, secretos y tiempo en cada mapa. Cada mod tiene su propia
configuración, así que los controles y ajustes de un mod no pasan a otro.

**Dónde van los paquetes de mapas.** El port los busca en la carpeta base
de Quake (la que contiene `id1/`, fijada con `QUAKE_BASEDIR` o `-basedir`).
Se muestra cada subcarpeta que contenga datos de juego (un archivo `.pak` o
`.pk3`, un `progs.dat` o una carpeta `maps/`), así que instala un paquete
descomprimiéndolo en su propia carpeta junto a `id1/`, por ejemplo
`<basedir>/ad/` o `<basedir>/my_maps/maps/*.bsp`. Si quieres, añade un
`modinfo.txt` con líneas `title`, `base` y `description` para decidir cómo
se muestra. Los paquetes se detectan al iniciar el juego, así que reinicia
tras añadir uno. Las partidas, capturas y configuración de cada paquete
están en `~/.local/share/quake-source-port/<carpeta>/`, no en la carpeta
del paquete.

## Pantalla dividida

Hasta seis jugadores en un mismo equipo, en cooperativo o partida a muerte.

- Multiplayer → New Game → **Jugadores locales** fija cuántos juegan.
- Cada jugador tiene su propia vista, interfaz, sonido y dispositivo
  (teclado y ratón para el jugador 1, un mando para cada uno de los demás).
- La disposición puede ser apilada, lado a lado, 2×2 o 3×2
  (`r_splitscreen_layout`).
- Cada jugador elige nombre y colores de camisa y pantalón en Multiplayer →
  Setup.

## Mandos

El port está pensado para jugarse con mando.

- **Selector de armas** (`+weaponwheel`): mantén para ralentizar el tiempo y
  elegir un arma con cualquiera de las palancas o el ratón; suelta para
  cambiar.
- **Ayuda al apuntar** (Options → Entrada → Mando): ralentización cerca de
  los enemigos y rotación que los sigue. Solo afecta a la palanca, nunca al
  ratón.
- **Apuntado con giroscopio** (Options → Entrada → Mando → Giroscopio): un
  interruptor, sensibilidades separadas para los ejes de guiñada, cabeceo y
  giro lateral (el giro lateral está desactivado por defecto y gira la vista
  como la guiñada; los valores negativos lo invierten), un filtro de ruido,
  y cuándo se aplica el giroscopio (siempre, desactivado o solo al mantener
  `+gyroaction`). **Estilo de apuntado** cambia la palanca derecha entre el
  apuntado 2D (gira la vista de forma continua) y el **flick stick** (empuja
  la palanca para orientar la vista al instante en esa dirección y luego
  gírala para rotarla); ambos se combinan bien con el giroscopio para
  apuntar con precisión.
- **Vibración**, sensibilidad e inversión de las palancas.
- Los **iconos de botones** coinciden con tu mando (Xbox, PlayStation,
  Nintendo) y se detectan automáticamente.
- Todo se reasigna en Options → Controles, también desde el mando.
- Controles táctiles con tamaño, opacidad y velocidad de vista ajustables.

## Guardar

- Los **autoguardados** rotan aparte de las partidas manuales y rápidas,
  así que nunca sobrescriben las tuyas. Options → Juego → Ajustes de
  autoguardado elige cuántos conservar y cuándo guardar: al entrar en un
  nivel, por intervalos, al encontrar un secreto o todos, y al matar a
  todos los enemigos.
- Las **tarjetas de partidas** de los menús Cargar/Guardar muestran el
  nivel, la salud, la armadura, las armas, las llaves y las bajas/secretos
  de cada partida.
- **Al morir** (Options → Juego → Al morir), en un jugador el juego puede
  preguntar qué hacer en un menú en pausa, cargar la partida más reciente,
  dejarte elegir una o reiniciar el nivel.

## Juego y cámara

Options → Juego:

- Campo visual de 60 a 130, balanceo de la cámara, retroceso del arma,
  correr siempre.
- **Cámara**: primera persona, tercera persona o sobre el hombro (derecho o
  izquierdo).
- Posición del **modelo del arma**: centro, derecha, izquierda u oculto.

## Personalizar: modificadores y complementos

Una partida es *estándar* o *personalizada*, según cómo empieza. New Game
(y elegir un nivel en Elegir nivel o Paquetes de mapas) muestra una
ventana: **Jugar** inicia la partida y **Personalizar la experiencia** abre
los modificadores de juego y los complementos. Cuando hay algo cambiado,
Jugar lleva la etiqueta **PERSONALIZADA** y **Desactivar todas las
modificaciones** (Supr, X en el mando o clic) devuelve las reglas estándar
sin complementos. Las partidas estándar guardan archivos `.sav` normales
que ironwail y vkQuake pueden cargar; las personalizadas guardan archivos
`.qsa` que solo carga este port (igual que la pantalla dividida y el
cooperativo, que otros ports no saben guardar). Durante una partida
personalizada, Menú principal → Personalizar cambia sus modificadores y
complementos en cualquier momento; las reglas de una partida estándar
quedan fijas. Los modificadores funcionan con cualquier mod y se desactivan
en partida a muerte.

- Más difícil: más daño recibido, enemigos más duros, menos munición y
  salud recogidas, monstruos x2–x4, inicio desde cero en cada nivel, sin
  guardado manual.
- Más fácil: menos daño recibido, enemigos más débiles, más munición y
  salud, munición infinita, regeneración de salud, sin daño por caída, sin
  ahogamiento, sin daño propio, saltos dobles o triples.

## Demos

`record <nombre>` y `stop` graban una demo en tu carpeta de usuario;
`playdemo <nombre>` la reproduce. Al terminar la grabación, la demo se
reescribe en segundo plano para que ironwail y vkQuake también puedan
reproducirla. Si un mod usa efectos de partículas que otros ports no saben
mostrar, la versión completa se guarda como `<nombre>.qsd`
(`playdemo <nombre>.qsd`). `demo_convert <nombre>` convierte una demo más
antigua.

## Accesibilidad

Options → Accesibilidad:

- Movimiento reducido e intensidad de los destellos de pantalla.
- Filtros para daltonismo.
- Punto de mira configurable: forma, tamaño, contorno y cambio de color al
  apuntar a un objetivo o al matar; marcadores de impacto opcionales.
- **Subtítulos de sonidos** y ajustes de mensajes (Mensajes y subtítulos),
  tamaño del texto.
- Atacar y correr alternos (en vez de mantener).
- Un control de velocidad del juego para ralentizar toda la partida.
- Apuntado automático con indicador de fijación y ajustes de cono y
  alcance.

## Mejoras

Options → Mejoras. Todo funciona en el cliente y está desactivado por
defecto, así que vale para cualquier mod y en línea.

- **Escombros**: despojos que ruedan y metralla de barriles destrozados,
  también con monstruos de mods; pueden chocar con puertas y ascensores.
- **Sangre mejorada**: salpicaduras en superficies, heridas que siguen a los
  cuerpos, gotas en el aire que se estrellan donde caen.
- **Efectos de impacto**: marcas de perdigones, clavos que se clavan en la
  madera y en los enemigos, marcas de quemadura de explosiones.
- **Agua mejorada**: transparencia, ondas, salpicaduras y cáusticas.
- **Teletransportadores transparentes** que muestran su destino con
  paralaje.
- Oclusión ambiental, desenfoque de movimiento por objeto y de velocidad,
  partículas suaves.
- **Sombreadores de pantalla**: preajustes `.slangp` de RetroArch (CRT y
  otros).

Options → Gráficos ofrece además los ajustes habituales: escala de
renderizado, antialiasing, supermuestreo, filtro de texturas, color de 8
bits con tramado, niebla, iluminación de color e interpolación de
animaciones y movimiento.

## Complementos

Los complementos son armas y equipo nuevos que gestiona el motor, así que
funcionan con el progs de cualquier mod. Solo están disponibles en un
jugador y en cooperativo.

### Empezar una partida con complementos

Una partida solo tiene complementos si empieza con ellos:

1. Single Player → marca la casilla **Complementos** y luego New Game.
2. Elige qué complementos llevar y ajusta sus opciones; después inicia la
   partida nueva.

Las partidas con complementos guardan archivos `.qsa` en lugar de `.sav`,
para que otros ports no intenten cargarlas. Las tarjetas de partidas
muestran una insignia "ADD-ONS". Cargar una partida normal desactiva los
complementos. Durante una partida con complementos, la página
Complementos (consola: `menu_options addons`) activa y desactiva cada uno;
cada complemento tiene una vista previa en bucle, instrucciones, sus
asignaciones y sus propios ajustes.

### Armas

Los complementos ocupan las ranuras de armas normales (elige cuál para cada
uno, o las ranuras 9 y 0) y aparecen en el selector de armas y en la
interfaz. El disparo y el disparo secundario son tus botones de ataque
habituales.

| Complemento | Disparo | Disparo secundario | Munición |
|---|---|---|---|
| **Pistola de portales** | Coloca el portal azul | Coloca el portal naranja | Ninguna |
| **Railgun** | Proyectil perforante que alcanza todo lo que hay en su línea | — | 2 celdas |
| **Gancho** | Se engancha a madera, metal, piedra o enemigos y te atrae | — | Ninguna |
| **Lanzabombas adhesivas** | Mantén para cargar, suelta para lanzar | Detonar | 1 cohete |
| **Bumerán** | Mantén para marcar enemigos, suelta para lanzar; aturde a cada uno por turnos | Borrar marcas | Ninguna |
| **Pulverizador de gel** | Gel azul de repulsión (elástico) | Gel naranja de propulsión (rápido) | Ninguna |
| **Hidrolimpiadora** | Chorro de agua largo que empuja a los enemigos y limpia el gel | Abanico ancho de chorros | Ninguna |

Los portales se colocan en superficies planas del mundo y llevan a través
de ellos a ti, a los monstruos y a los objetos lanzados, conservando su
impulso. Con **Ver a través de los portales** ves el otro lado en directo,
incluidos los portales visibles a través de otros portales; la calidad, el
número de vistas en directo, la distancia y la profundidad de recursión
son ajustables.

### Equipo

El equipo se enciende y se apaga en lugar de empuñarse. Abre el **selector
de equipo** (`+equipwheel`) o usa `goggles cycle` para alternar:

- **Visión nocturna**
- **Visión térmica**

## Multijugador

Además del juego NetQuake en LAN y por Internet (protocolos 15, 666 y 999),
el port se conecta a servidores **QuakeWorld**. Multiplayer → Join Game
acepta una dirección o busca en la LAN. Las partículas programadas de FTE
funcionan en los mods que las usan.

## Línea de comandos y consola

Opciones habituales: `-basedir <carpeta>`, `-game <carpeta>`, `-hipnotic` /
`-rogue` / `-quoth`, `-width <an>`, `-height <al>`, `-fullscreen`, `-window`
y `+comando argumentos` para ejecutar comandos de consola al inicio. La
consola (`~`) acepta los comandos y variables habituales de Quake.

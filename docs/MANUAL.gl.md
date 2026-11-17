# Manual de usuario

Este manual explica como xogar a Quake con este port, centrándose no que
engade respecto ao Quake orixinal e a outros ports. Todo o novo é
opcional: cos axustes predeterminados, o xogo vese e xógase como Quake.

## Primeiros pasos

1. Consegue a túa propia copia de Quake. Indícalle ao port o cartafol que
   contén `id1/` (e os mods ao seu carón), con `QUAKE_BASEDIR` ou
   `-basedir <cartafol>`.
2. Executa `cargo run --release`, ou inicia directamente un mod con
   `-game <cartafol> +map <mapa>`.
3. A configuración, as partidas gardadas e as capturas están en
   `~/.local/share/quake-source-port/<xogo>/`, un cartafol por mod.

O menú principal ofrece Single Player, Multiplayer, Options, **Help**
(este manual), Quit e **Paquetes de mapas**, co tema de Quake de fondo
(Options → Audio → Música do menú principal). Todos os menús funcionan
con rato, teclado, mando e pantalla táctil; pasa por riba dunha opción ou
selecciónaa para ver unha descrición emerxente.

Na axuda, arriba/abaixo elixe unha sección; RePáx/AvPáx,
esquerda/dereita, os botóns superiores ou a roda do rato sobre o texto
desprázano.

## Idioma

Options → Xogo → **Idioma** elixe o idioma dos menús, das mensaxes en
pantalla e deste manual: inglés, francés, alemán, italiano, español, ruso,
polaco, ucraíno, checo, eslovaco, croata, búlgaro, sueco, finés,
dinamarqués, noruegués, neerlandés, grego, húngaro, romanés, portugués
(Portugal ou Brasil), xaponés, coreano, chinés (simplificado ou
tradicional), turco, indonesio, malaio, filipino, vietnamita, serbio,
lituano, letón, estonio, maltés, albanés, armenio, xeorxiano, islandés,
irlandés, galés, éuscaro, catalán, galego, latín, esperanto, bielorruso,
macedonio, esloveno, suahili, árabe, hebreo, persa, urdú, hindi, marathi,
bengalí, punjabi, guxarati, támil, telugu, tailandés, khmer,
amhárico ou cantonés.
**Sistema** segue o idioma do teu ordenador. Os textos de xogo da
reedición de 2021 só están traducidos ao francés, alemán, italiano,
español e ruso; os demais idiomas móstranos en inglés. Nos idiomas
distintos do inglés, os textos pequenos do xogo usan a fonte dos menús,
porque a fonte clásica de Quake só ten letras latinas sen acentos. As
mensaxes da consola seguen en inglés salvo que actives **Traducir a
consola**.

## Paquetes de mapas

Menú principal → Paquetes de mapas mostra cada mod e paquete de mapas
instalado coa súa descrición. Elixe un para cambiar a el, explora os seus
mapas e inicia calquera directamente. O navegador garda as túas mellores
baixas, segredos e tempo en cada mapa. Cada mod ten a súa propia
configuración, así que os controis e axustes dun mod non pasan a outro.

**Onde van os paquetes de mapas.** O port búscaos no cartafol base de
Quake (o que contén `id1/`, fixado con `QUAKE_BASEDIR` ou `-basedir`).
Móstrase cada subcartafol que conteña datos de xogo (un ficheiro `.pak` ou
`.pk3`, un `progs.dat` ou un cartafol `maps/`), así que instala un paquete
descomprimíndoo no seu propio cartafol xunto a `id1/`, por exemplo
`<basedir>/ad/` ou `<basedir>/my_maps/maps/*.bsp`. Se queres, engade un
`modinfo.txt` con liñas `title`, `base` e `description` para decidir como
se mostra. Os paquetes detéctanse ao iniciar o xogo, así que reinicia
despois de engadir un. As partidas, capturas e configuración de cada
paquete están en `~/.local/share/quake-source-port/<cartafol>/`, non no
cartafol do paquete.

## Pantalla dividida

Ata seis xogadores nun mesmo equipo, en cooperativo ou todos contra todos.

- Multiplayer → New Game → **Xogadores locais** fixa cantos xogan.
- Cada xogador ten a súa propia vista, interface, son e dispositivo
  (teclado e rato para o xogador 1, un mando para cada un dos demais).
- A disposición pode ser apilada, lado a lado, 2×2 ou 3×2
  (`r_splitscreen_layout`).
- Cada xogador elixe nome e cores da camisa e dos pantalóns en
  Multiplayer → Setup.

## Mandos

O port está pensado para xogarse con mando.

- **Selector de armas** (`+weaponwheel`): mantén para ralentizar o tempo e
  elixir unha arma con calquera das pancas ou co rato; solta para
  cambiar.
- **Axuda á puntaría** (Options → Entrada → Mando): ralentización preto
  dos inimigos e rotación que os segue. Só afecta á panca, nunca ao rato.
- **Puntaría con xiroscopio** (Options → Entrada → Mando → Xiroscopio): un
  interruptor, sensibilidades separadas para os eixes de guiñada, cabeceo
  e xiro lateral (o xiro lateral está desactivado por defecto e xira a
  vista como a guiñada; os valores negativos invérteno), un filtro de
  ruído, e cando se aplica o xiroscopio (sempre, desactivado ou só ao
  manter `+gyroaction`). **Estilo de puntaría** cambia a panca dereita
  entre a puntaría 2D (xira a vista de forma continua) e o **flick stick**
  (empurra a panca para orientar a vista ao instante nesa dirección e
  despois xira a panca para rotala); ambos combínanse ben co xiroscopio
  para apuntar con precisión.
- **Vibración**, sensibilidade e inversión das pancas.
- As **iconas dos botóns** coinciden co teu mando (Xbox, PlayStation,
  Nintendo) e detéctanse automaticamente.
- Todo se reasigna en Options → Controis, tamén desde o mando.
- Controis táctiles con tamaño, opacidade e velocidade da vista
  axustables.

## Gardar

- Os **gardados automáticos** rotan á parte das partidas manuais e
  rápidas, así que nunca sobrescriben as túas. Options → Xogo → Axustes
  do gardado automático elixe cantos conservar e cando gardar: ao entrar
  nun nivel, por intervalos, ao atopar un segredo ou todos, e ao matar
  todos os inimigos.
- As **tarxetas de partidas** dos menús Cargar/Gardar mostran o nivel, a
  saúde, a armadura, as armas, as chaves e as baixas/segredos de cada
  partida.
- **Ao morrer** (Options → Xogo → Ao morrer), nun xogador o xogo pode
  preguntar que facer nun menú en pausa, cargar a partida máis recente,
  deixarte elixir unha ou reiniciar o nivel.

## Xogo e cámara

Options → Xogo:

- Campo de visión de 60 a 130, balanceo da cámara, retroceso da arma,
  correr sempre.
- **Cámara**: primeira persoa, terceira persoa ou sobre o ombreiro
  (dereito ou esquerdo).
- Posición do **modelo da arma**: centro, dereita, esquerda ou oculto.

## Personalizar: modificadores e complementos

Unha partida é *estándar* ou *personalizada*, segundo como comeza. New
Game (e elixir un nivel en Elixir nivel ou Paquetes de mapas) mostra unha
xanela: **Xogar** inicia a partida e **Personalizar a experiencia** abre
os modificadores de xogo e os complementos. Cando hai algo cambiado,
Xogar leva a etiqueta **PERSONALIZADA** e **Desactivar todas as
modificacións** (Supr, X no mando ou clic) devolve as regras estándar sen
complementos. As partidas estándar gardan ficheiros `.sav` normais que
ironwail e vkQuake poden cargar; as personalizadas gardan ficheiros
`.qsa` que só carga este port (igual que a pantalla dividida e o
cooperativo, que outros ports non saben gardar). Durante unha partida
personalizada, Menú principal → Personalizar cambia os seus modificadores
e complementos en calquera momento; as regras dunha partida estándar
quedan fixas. Os modificadores funcionan con calquera mod e desactívanse
en todos contra todos.

- Máis difícil: máis dano recibido, inimigos máis duros, menos munición e
  saúde recollidas, monstros x2–x4, inicio desde cero en cada nivel, sen
  gardado manual.
- Máis doado: menos dano recibido, inimigos máis febles, máis munición e
  saúde, munición infinita, rexeneración de saúde, sen dano por caída, sen
  afogamento, sen dano propio, saltos dobres ou triplos.

## Demos

`record <nome>` e `stop` gravan unha demo no teu cartafol de usuario;
`playdemo <nome>` reprodúcea. Ao rematar a gravación, a demo reescríbese
en segundo plano para que ironwail e vkQuake tamén poidan reproducila. Se
un mod usa efectos de partículas que outros ports non saben mostrar, a
versión completa gárdase como `<nome>.qsd` (`playdemo <nome>.qsd`).
`demo_convert <nome>` converte unha demo máis antiga.

## Accesibilidade

Options → Accesibilidade:

- Movemento reducido e intensidade dos destellos de pantalla.
- Filtros para o daltonismo.
- Punto de mira configurable: forma, tamaño, contorno e cambio de cor ao
  apuntar a un obxectivo ou ao matar; marcadores de impacto opcionais.
- **Subtítulos de sons** e axustes das mensaxes (Mensaxes e subtítulos),
  tamaño do texto.
- Atacar e correr alternos (en vez de manter).
- Un control da velocidade do xogo para ralentizar toda a partida.
- Puntaría automática con indicador de fixación e axustes de cono e
  alcance.

## Melloras

Options → Melloras. Todo funciona no cliente e está desactivado por
defecto, así que vale para calquera mod e en liña.

- **Cascallos**: restos que rodan e metralla de barrís esnaquizados, tamén
  con monstros de mods; poden bater contra portas e ascensores.
- **Sangue mellorado**: salpicaduras en superficies, feridas que seguen os
  corpos, pingas no aire que estoupan onde caen.
- **Efectos de impacto**: marcas de perdigóns, cravos que se cravan na
  madeira e nos inimigos, marcas de queimadura das explosións.
- **Auga mellorada**: transparencia, ondas, salpicaduras e cáusticas.
- **Teletransportadores transparentes** que mostran o seu destino con
  paralaxe.
- Oclusión ambiental, desenfoque de movemento por obxecto e de velocidade,
  partículas suaves.
- **Sombreadores de pantalla**: predefinicións `.slangp` de RetroArch (CRT
  e outras).

Options → Gráficos ofrece ademais os axustes habituais: escala de
renderizado, antialiasing, supermostraxe, filtro de texturas, cor de 8
bits con tramado, néboa, iluminación de cor e interpolación de animacións
e movemento.

## Complementos

Os complementos son armas e equipo novos que xestiona o motor, así que
funcionan co progs de calquera mod. Só están dispoñibles nun xogador e en
cooperativo.

### Comezar unha partida con complementos

Unha partida só ten complementos se comeza con eles:

1. Single Player → marca a caixa **Complementos** e despois New Game.
2. Elixe que complementos levar e axusta as súas opcións; despois inicia a
   partida nova.

As partidas con complementos gardan ficheiros `.qsa` en lugar de `.sav`,
para que outros ports non intenten cargalas. As tarxetas de partidas
mostran unha insignia "ADD-ONS". Cargar unha partida normal desactiva os
complementos. Durante unha partida con complementos, a páxina
Complementos (consola: `menu_options addons`) activa e desactiva cada un;
cada complemento ten unha vista previa en bucle, instrucións, as súas
asignacións e os seus propios axustes.

### Armas

Os complementos ocupan as rañuras de armas normais (elixe cal para cada
un, ou as rañuras 9 e 0) e aparecen no selector de armas e na interface.
O disparo e o disparo secundario son os teus botóns de ataque habituais.

| Complemento | Disparo | Disparo secundario | Munición |
|---|---|---|---|
| **Pistola de portais** | Coloca o portal azul | Coloca o portal laranxa | Ningunha |
| **Railgun** | Proxectil perforante que alcanza todo o que hai na súa liña | — | 2 celas |
| **Gancho** | Engánchase a madeira, metal, pedra ou inimigos e achégate | — | Ningunha |
| **Lanzabombas adhesivas** | Mantén para cargar, solta para lanzar | Detonar | 1 foguete |
| **Bumerán** | Mantén para marcar inimigos, solta para lanzar; atorda cada un por quendas | Borrar marcas | Ningunha |
| **Pulverizador de xel** | Xel azul de repulsión (elástico) | Xel laranxa de propulsión (rápido) | Ningunha |
| **Hidrolimpadora** | Chorro de auga longo que empurra os inimigos e limpa o xel | Abano ancho de chorros | Ningunha |

Os portais colócanse en superficies planas do mundo e levan a través
deles a ti, aos monstros e aos obxectos lanzados, conservando o seu
impulso. Con **Ver a través dos portais** ves o outro lado en directo,
incluídos os portais visibles a través doutros portais; a calidade, o
número de vistas en directo, a distancia e a profundidade de recursión
son axustables.

### Equipo

O equipo acéndese e apágase en lugar de empuñarse. Abre o **selector de
equipo** (`+equipwheel`) ou usa `goggles cycle` para alternar:

- **Visión nocturna**
- **Visión térmica**

## Multixogador

Ademais do xogo NetQuake en LAN e por Internet (protocolos 15, 666 e 999),
o port conéctase a servidores **QuakeWorld**. Multiplayer → Join Game
acepta un enderezo ou busca na LAN. As partículas programadas de FTE
funcionan nos mods que as usan.

## Liña de ordes e consola

Opcións habituais: `-basedir <cartafol>`, `-game <cartafol>`, `-hipnotic` /
`-rogue` / `-quoth`, `-width <w>`, `-height <h>`, `-fullscreen`,
`-window` e `+orde argumentos` para executar ordes da consola ao iniciar.
A consola (`~`) acepta as ordes e cvars habituais de Quake.

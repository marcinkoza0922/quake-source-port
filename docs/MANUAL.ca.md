# Manual d'usuari

Aquest manual explica com jugar a Quake amb aquest port, i se centra en el
que afegeix respecte al Quake original i a altres ports. Tot allò que és
nou és opcional: amb la configuració per defecte, el joc es veu i es juga
com Quake.

## Primers passos

1. Aconsegueix la teva pròpia còpia de Quake. Indica al port la carpeta que
   conté `id1/` (i els mods que hi hagi al costat), amb `QUAKE_BASEDIR` o
   `-basedir <carpeta>`.
2. Executa `cargo run --release`, o inicia directament un mod amb
   `-game <carpeta> +map <mapa>`.
3. La configuració, les partides desades i les captures de pantalla són a
   `~/.local/share/quake-source-port/<joc>/`, una carpeta per mod.

El menú principal ofereix Single Player, Multiplayer, Options, **Help**
(aquest manual), Quit i **Paquets de mapes**, amb el tema de Quake de fons
(Options → Àudio → Música del menú principal). Tots els menús funcionen
amb ratolí, teclat, comandament i pantalla tàctil; passa per sobre d'una
opció o selecciona-la per veure'n una descripció emergent.

A l'ajuda, amunt/avall tria una secció; RePàg/AvPàg, esquerra/dreta, els
botons superiors o la roda del ratolí sobre el text el desplacen.

## Idioma

Options → Jugabilitat → **Idioma** tria l'idioma dels menús, dels missatges
en pantalla i d'aquest manual: anglès, francès, alemany, italià, castellà,
rus, polonès, ucraïnès, txec, eslovac, croat, búlgar, suec, finès, danès,
noruec, neerlandès, grec, hongarès, romanès, portuguès (Portugal o Brasil),
japonès, coreà, xinès (simplificat o tradicional), turc, indonesi, malai,
filipí, vietnamita, serbi, lituà, letó, estonià, maltès, albanès, armeni,
georgià, islandès, irlandès, gal·lès, basc, català, gallec, llatí,
esperanto, bielorús, macedoni, eslovè o suahili.
**Sistema** segueix l'idioma de l'ordinador. Els textos de joc de la
reedició del 2021 només estan traduïts al francès, l'alemany, l'italià, el
castellà i el rus; els altres idiomes els mostren en anglès. En els idiomes
diferents de l'anglès, el text petit del joc fa servir la font dels menús,
perquè la font clàssica de Quake només té lletres llatines sense accents.
Els missatges de la consola es mantenen en anglès tret que activis
**Tradueix la consola**.

## Paquets de mapes

Menú principal → Paquets de mapes mostra cada mod i paquet de mapes
instal·lat amb la seva descripció. Tria'n un per canviar-hi, explora'n els
mapes i inicia'n qualsevol directament. El navegador recorda les teves
millors baixes, secrets i temps de cada mapa. Cada mod té la seva pròpia
configuració, de manera que les assignacions i els ajustos d'un mod no
passen a un altre.

**On van els paquets de mapes.** El port els busca a la carpeta base de
Quake (la que conté `id1/`, definida amb `QUAKE_BASEDIR` o `-basedir`). Es
mostra cada subcarpeta que contingui dades de joc (un fitxer `.pak` o
`.pk3`, un `progs.dat` o una carpeta `maps/`), així que instal·la un paquet
descomprimint-lo en una carpeta pròpia al costat d'`id1/`, per exemple
`<basedir>/ad/` o `<basedir>/my_maps/maps/*.bsp`. Opcionalment, afegeix-hi
un `modinfo.txt` amb línies `title`, `base` i `description` per decidir com
es mostra. Els paquets es detecten en iniciar el joc, així que reinicia'l
després d'afegir-ne un. Les partides desades, les captures i la
configuració de cada paquet són a
`~/.local/share/quake-source-port/<carpeta>/`, no a la carpeta del paquet.

## Pantalla dividida

Fins a sis jugadors en un mateix ordinador, en cooperatiu o en tots contra
tots.

- Multiplayer → New Game → **Jugadors locals** defineix quantes persones
  juguen.
- Cada jugador té la seva pròpia vista, HUD, so i dispositiu d'entrada
  (teclat i ratolí per al jugador 1, un comandament per a cadascun dels
  altres).
- La disposició de la pantalla pot ser apilada, de costat, 2×2 o 3×2
  (`r_splitscreen_layout`).
- Cada jugador tria un nom i els colors de la camisa i els pantalons a
  Multiplayer → Setup.

## Comandaments

El port està pensat per jugar-hi amb comandament.

- **Selector d'armes** (`+weaponwheel`): mantén-lo premut per alentir el
  temps i triar una arma amb qualsevol de les palanques o amb el ratolí;
  deixa'l anar per canviar-hi.
- **Ajuda a apuntar** (Options → Entrada → Comandament): alentiment a prop
  dels enemics i rotació que els segueix. Només s'aplica a la palanca, mai
  al ratolí.
- **Punteria amb giroscopi** (Options → Entrada → Comandament →
  Giroscopi): un interruptor, sensibilitats separades per als eixos de
  guinyada, capcineig i balanceig (el balanceig està desactivat per defecte
  i gira la vista com la guinyada; els valors negatius l'inverteixen), un
  filtre de soroll, i quan s'aplica el giroscopi (sempre, desactivat o
  només mentre es manté `+gyroaction`). **Estil de punteria** canvia la
  palanca dreta entre la punteria 2D (gira la vista de manera contínua) i
  el **flick stick** (empeny la palanca per orientar la vista a l'instant
  cap a aquella direcció i després gira-la per fer-la rotar); tots dos es
  combinen bé amb el giroscopi per apuntar amb precisió.
- **Vibració**, sensibilitat i inversió de les palanques.
- Les **icones dels botons** coincideixen amb el comandament (Xbox,
  PlayStation, Nintendo) i es detecten automàticament.
- Tot es pot reassignar a Options → Controls, també des del comandament.
- Controls tàctils amb mida, opacitat i velocitat de la vista ajustables.

## Desar

- Els **desaments automàtics** roten a part dels desaments manuals i
  ràpids, de manera que mai no sobreescriuen els teus. Options →
  Jugabilitat → Configuració del desament automàtic tria quants se'n
  conserven i quan es desa: en entrar en un nivell, per temporitzador, en
  trobar un secret o tots, i en eliminar tots els enemics.
- Les **targetes de partida** dels menús de càrrega i desament mostren el
  nivell, la salut, l'armadura, les armes, les claus i les baixes/secrets
  de cada partida desada.
- **En morir** (Options → Jugabilitat → En morir), en un jugador el joc pot
  preguntar què fer en un menú en pausa, carregar la partida desada més
  recent, deixar-te triar una partida o reiniciar el nivell.

## Jugabilitat i càmera

Options → Jugabilitat:

- Camp de visió de 60 a 130, balanceig de la càmera, retrocés de l'arma,
  córrer sempre.
- **Càmera**: primera persona, tercera persona o sobre l'espatlla (dreta o
  esquerra).
- Posició dels **models de l'arma**: centre, dreta, esquerra o amagats.

## Personalitza: modificadors de joc i complements

Una partida és *estàndard* o *personalitzada*, segons com comença. New Game
(i triar un nivell a Tria el nivell o a Paquets de mapes) mostra una
finestra emergent: **Juga** inicia la partida i **Personalitza
l'experiència** obre els modificadors de joc i els complements. Quan hi ha
alguna cosa canviada, Juga porta l'etiqueta **PERSONALITZADA** i
**Desactiva totes les modificacions** (Supr, X al comandament o clic)
restableix les regles estàndard sense complements. Les partides estàndard
desen fitxers `.sav` normals que ironwail i vkQuake poden carregar; les
personalitzades desen fitxers `.qsa` que només carrega aquest port (igual
que les partides en pantalla dividida i en cooperatiu, que altres ports no
saben desar). Durant una partida personalitzada, Menú principal →
Personalitza en canvia els modificadors i complements en qualsevol moment;
les regles d'una partida estàndard queden fixes. Els modificadors funcionen
amb qualsevol mod i es desactiven en tots contra tots.

- Més difícil: més dany rebut, enemics més durs, menys munició i salut
  recollides, monstres x2–x4, inici des de zero a cada nivell, sense
  desament manual.
- Més fàcil: menys dany rebut, enemics més febles, més munició i salut,
  munició infinita, regeneració de salut, sense dany per caiguda, sense
  ofegament, sense dany propi, salts dobles o triples.

## Demos

`record <nom>` i `stop` enregistren una demo a la teva carpeta d'usuari;
`playdemo <nom>` la reprodueix. Quan s'atura l'enregistrament, la demo es
reescriu en segon pla perquè ironwail i vkQuake també la puguin
reproduir. Si un mod fa servir efectes de partícules que altres ports no
saben mostrar, la versió completa es conserva com a `<nom>.qsd`
(`playdemo <nom>.qsd`). `demo_convert <nom>` converteix una demo més
antiga.

## Accessibilitat

Options → Accessibilitat:

- Moviment reduït i intensitat dels flaixos de pantalla.
- Filtres per al daltonisme.
- Punt de mira configurable: forma, mida, contorn i canvi de color sobre
  l'objectiu o en eliminar; marcadors d'impacte opcionals.
- **Subtítols dels sons** i configuració dels missatges (Missatges i
  subtítols), mida del text.
- Atacar i córrer alternats (en lloc de mantenir premut).
- Un control lliscant de velocitat del joc per alentir tota la partida.
- Punteria automàtica amb indicador de fixació i ajustos de con i abast.

## Millores

Options → Millores. Tot funciona al client i està desactivat per defecte,
així que funciona amb qualsevol mod i en línia.

- **Runa**: restes que rodolen i metralla de barrils esmicolats, també amb
  monstres de mods; pot topar amb portes i ascensors.
- **Esquitxades de sang millorades**: esquitxades a les superfícies, ferides
  que segueixen els cossos, gotes a l'aire que s'esclafen on cauen.
- **Efectes d'impacte a les superfícies**: marques de perdigons, claus que
  es claven a la fusta i als enemics, marques de cremades d'explosions.
- **Aigua millorada**: transparència, ones, esquitxades i càustiques.
- **Teletransportadors transparents** que mostren la seva destinació amb
  paral·laxi.
- Oclusió ambiental, desenfocament de moviment per objecte i de velocitat,
  partícules suaus.
- **Shaders de pantalla**: predefinits `.slangp` de RetroArch (CRT i
  altres).

Options → Gràfics ofereix a més els ajustos habituals: escala de
renderització, antialiàsing, supermostreig, filtre de textures, color de 8
bits amb tramat, boira, il·luminació de color i interpolació d'animacions i
moviment.

## Complements

Els complements són armes i equip nous que gestiona el motor, de manera que
funcionen amb el progs de qualsevol mod. Només estan disponibles en un
jugador i en cooperatiu.

### Començar una partida amb complements

Una partida només té complements si comença amb ells:

1. Single Player → marca la casella **Complements** i després New Game.
2. Tria quins complements t'emportes i ajusta'n la configuració; després
   inicia la partida nova.

Les partides amb complements es desen com a fitxers `.qsa` en lloc de
`.sav`, perquè altres ports no les intentin carregar. Les targetes de
partida mostren una insígnia "ADD-ONS". Carregar una partida desada normal
desactiva els complements. Durant una partida amb complements, la pàgina
Complements (consola: `menu_options addons`) activa i desactiva cada
complement; cadascun té una previsualització en bucle, instruccions, les
seves assignacions i la seva pròpia configuració.

### Armes

Els complements ocupen les ranures d'arma habituals (tria quina per a cada
complement, o les ranures 9 i 0) i apareixen al selector d'armes i a l'HUD.
El tret i el tret secundari són els teus botons d'atac habituals.

| Complement | Tret | Tret secundari | Munició |
|---|---|---|---|
| **Pistola de portals** | Col·loca el portal blau | Col·loca el portal taronja | Cap |
| **Railgun** | Projectil perforant que toca tot el que hi ha en línia | — | 2 cel·les |
| **Ganxo** | S'enganxa a fusta, metall, pedra o enemics i t'hi estira | — | Cap |
| **Llançabombes adhesives** | Mantén per carregar, deixa anar per llançar | Detona | 1 coet |
| **Bumerang** | Mantén per marcar enemics, deixa anar per llançar; els atordeix per torns | Esborra les marques | Cap |
| **Polvoritzador de gel** | Gel blau de repulsió (elàstic) | Gel taronja de propulsió (ràpid) | Cap |
| **Hidronetejadora** | Raig d'aigua llarg que empeny els enemics i neteja el gel | Ventall ample de raigs | Cap |

Els portals es col·loquen en superfícies planes del món i et fan passar a
tu, als monstres i als objectes llançats conservant-ne l'impuls. Amb
**Veure a través dels portals** activat veus l'altre costat en directe,
inclosos els portals visibles a través d'altres portals; la qualitat, el
nombre de vistes en directe, la distància i la profunditat de recursió es
poden ajustar.

### Equip

L'equip s'activa i es desactiva en lloc d'empunyar-se. Obre el **selector
d'equip** (`+equipwheel`) o fes servir `goggles cycle` per alternar:

- **Visió nocturna**
- **Visió tèrmica**

## Multijugador

A més del joc NetQuake en LAN i per Internet (protocols 15, 666 i 999), el
port es connecta a servidors **QuakeWorld**. Multiplayer → Join Game
accepta una adreça o cerca a la LAN. Les partícules programades de FTE
funcionen en els mods que les fan servir.

## Línia d'ordres i consola

Opcions habituals: `-basedir <carpeta>`, `-game <carpeta>`, `-hipnotic` /
`-rogue` / `-quoth`, `-width <amplada>`, `-height <alçada>`, `-fullscreen`,
`-window` i `+ordre arguments` per executar ordres de consola en
l'arrencada. La consola (`~`) accepta les ordres i variables habituals de
Quake.

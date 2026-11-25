# Felhasználói kézikönyv

Ez a kézikönyv azt írja le, hogyan játszhatsz a Quake-kel ezzel a porttal,
elsősorban azt, amit az eredeti Quake-hez és más source portokhoz képest
hozzáad. Minden új funkció opcionális: alapbeállításokkal a játék úgy néz ki
és úgy játszható, mint a Quake.

## Első lépések

1. Szerezd be a saját Quake-példányodat. Add meg a portnak azt a mappát,
   amely az `id1/` mappát (és mellette a modokat) tartalmazza, a
   `QUAKE_BASEDIR` vagy a `-basedir <mappa>` segítségével.
2. Futtasd a `cargo run --release` parancsot, vagy indíts el rögtön egy
   modot a `-game <mappa> +map <pálya>` kapcsolókkal.
3. A beállítások, mentések és képernyőképek helye
   `~/.local/share/quake-source-port/<game>/`, modonként egy mappa.

A főmenüben a Single Player, Multiplayer, Options, **Help** (ez a
kézikönyv), Quit és **Pályacsomagok** pont található, a háttérben a Quake
főcímzenéje szól (Options → Hang → Főmenü zenéje). Minden menü működik
egérrel, billentyűzettel, kontrollerrel és érintéssel; vidd a mutatót egy
beállítás fölé, vagy jelöld ki, és megjelenik egy magyarázó súgószöveg.

A Help képernyőn a fel/le választ szakaszt; a Page Up/Page Down, a balra/jobbra, a
vállgombok vagy a szöveg fölötti egérgörgő görgeti.

## Nyelv

Az Options → Játékmenet → **Nyelv** választja ki a menük, a képernyőüzenetek és
ennek a kézikönyvnek a nyelvét: angol, francia, német, olasz, spanyol, orosz,
lengyel, ukrán, cseh, szlovák, horvát, bolgár, svéd, finn, dán, norvég,
holland, görög, magyar, román, portugál (Portugália vagy Brazília), japán,
koreai, kínai (egyszerűsített vagy hagyományos), török, indonéz, maláj,
filippínó, vietnámi, szerb, litván, lett, észt, máltai, albán, örmény, grúz,
izlandi, ír, walesi, baszk, katalán, galiciai, latin, eszperantó, belarusz,
macedón, szlovén, szuahéli, arab, héber, perzsa, urdu, hindi, maráthi,
bengáli, pandzsábi, gudzsaráti, tamil, telugu, thai, khmer, amhara vagy kantoni.
A **Rendszer** a számítógéped nyelvét követi. A 2021-es újrakiadás saját
játékszövegei csak franciául, németül, olaszul, spanyolul és oroszul érhetők
el; a többi nyelv angolul mutatja őket. Az angoltól eltérő nyelveken a kis
játékon belüli szövegek a menü betűtípusát használják, mivel a klasszikus
Quake-betűtípusban csak ékezet nélküli latin betűk vannak. A konzolüzenetek
angolul maradnak, hacsak a **Konzol fordítása** nincs bekapcsolva.

## Pályacsomagok (modböngésző)

A Főmenü → Pályacsomagok felsorol minden telepített modot és pályacsomagot a
leírásukkal együtt. Válassz ki egyet, hogy átválts rá, majd böngészd a
pályáit, és bármelyiket közvetlenül elindíthatod. A böngésző pályánként
nyilvántartja a legjobb öléseidet, titkaidat és idődet. Minden modnak saját
beállításai vannak, így az egyik modhoz igazított hozzárendelések és
beállítások nem szivárognak át egy másikba.

**Hova kerülnek a pályacsomagok.** A port a Quake alapmappájában keresi őket
(abban, amelyik az `id1/` mappát tartalmazza; ezt a `QUAKE_BASEDIR` vagy a
`-basedir` adja meg). Minden ottani almappa, amely játékadatot tartalmaz
(`.pak` vagy `.pk3` fájlt, `progs.dat` fájlt vagy `maps/` mappát),
megjelenik a listában, tehát egy pályacsomagot úgy telepíthetsz, hogy egy
saját mappába csomagolod ki az `id1/` mellé, például
`<basedir>/ad/` vagy `<basedir>/my_maps/maps/*.bsp`. Ha szeretnéd, adj
hozzá egy `modinfo.txt` fájlt `title`, `base` és `description` sorokkal,
hogy meghatározd, hogyan jelenjen meg a listában. A csomagokat a játék
indításkor keresi meg, ezért egy új csomag hozzáadása után indítsd újra.
Az egyes csomagok mentései, képernyőképei és beállításai a
`~/.local/share/quake-source-port/<mappa>/` alatt vannak, nem a csomag
mappájában.

## Osztott képernyő

Legfeljebb hat játékos egy gépen, kooperatív vagy deathmatch módban.

- A Multiplayer → New Game → **Helyi játékosok** adja meg, hányan játszanak.
- Minden játékos saját nézetet, HUD-ot, hangot és beviteli eszközt kap (az
  első játékos billentyűzetet és egeret, a többiek egy-egy kontrollert).
- A képernyő elrendezése lehet egymás alatti, egymás melletti, 2×2 vagy 3×2
  (`r_splitscreen_layout`).
- Minden játékos nevet, valamint ing- és nadrágszínt választ a Multiplayer →
  Setup alatt.

## Kontrollerek

A port úgy készült, hogy kontrollerrel is jól játszható legyen.

- **Fegyverkerék** (`+weaponwheel`): tartsd nyomva, hogy lelassuljon az idő,
  és válassz fegyvert bármelyik karral vagy az egérrel; elengedve vált.
- **Célzássegéd** (Options → Bemenet → Kontroller): lassítás az ellenségek
  közelében és követésükre forduló célzás. Csak a karra vonatkozik, az
  egérre soha.
- **Giroszkópos célzás** (Options → Bemenet → Kontroller → Giroszkóp):
  kapcsoló, valamint külön érzékenység a vízszintes, a függőleges és a dőlési
  tengelyre (a dőlés alapból ki van kapcsolva, és a vízszintes tengelyhez
  hasonlóan fordítja a nézetet; a negatív értékek megfordítják), zajszűrő,
  és hogy mikor működjön a giroszkóp (mindig, kikapcsolva vagy csak a
  `+gyroaction` nyomva tartásakor). A **Célzási stílus** a jobb kart a 2D
  célzás (folyamatosan forgatja a nézetet) és a **flick stick** (nyomd a
  kart, és a nézet azonnal arra fordul, majd a kar forgatásával forgathatod)
  között váltja; mindkettő jól kombinálható a giroszkóppal a finom
  célzáshoz.
- **Rezgés**, karérzékenység és megfordítás.
- A **Gombikonok** igazodnak a kontrolleredhez (Xbox, PlayStation,
  Nintendo), és automatikusan felismerődnek.
- Mindent átállíthatsz az Options → Irányítás alatt, akár a kontrollerről
  is.
- **Select / View ×2**: Képernyőkép; **Start ×2**: Gyorsmentés; **Start**
  (Nyomva tartás): Gyorsbetöltés.
- Érintőképernyős irányítás állítható mérettel, átlátszatlansággal és
  nézési sebességgel.

## Mentés

- Az **Automentések** a kézi mentésektől és gyorsmentésektől külön
  forognak, így sosem írják felül a sajátjaidat. Az Options → Játékmenet →
  Automentés beállításai határozza meg, hányat tartson meg, és mikor
  mentsen: pályakezdéskor, időzítővel, egy titok vagy minden titok
  megtalálásakor, illetve minden ellenség megölésekor.
- A Betöltés/Mentés menük **mentéskártyái** mutatják minden mentés pályáját,
  életerejét, páncélját, fegyvereit, kulcsait és öléseit/titkait.
- **Halálkor** (Options → Játékmenet → Halálkor) egyjátékos módban a játék
  megkérdezheti egy szüneteltetett menüben, mit tegyen, betöltheti a
  legutóbbi mentést, választhatsz mentést, vagy újrakezdheted a pályát.

## Játékmenet és kamera

Options → Játékmenet:

- Látómező 60–130, nézet billegése, fegyver rúgása, mindig fut.
- **Kamera**: belső nézet, külső nézet vagy váll fölötti nézet (jobb vagy
  bal).
- **Fegyvermodellek** helye: középen, jobbra, balra vagy rejtve.

## Testreszabás: játékmódosítók és kiegészítők

Egy játék vagy *eredeti*, vagy *testreszabott*; ez az indításkor dől el. A
New Game (és egy pálya kiválasztása a Pályaválasztásban vagy a
Pályacsomagokban) egy felugró ablakot mutat: a **Játék** elindítja a
játékot, a **Játékélmény testreszabása** megnyitja a Játékmódosítókat és a
Kiegészítőket. Amíg bármi meg van változtatva, a Játék gomb **EGYEDI**
jelölést kap, és a **Minden módosítás kikapcsolása** (Del, kontrolleren X,
vagy kattintás) visszaállítja a szabványos szabályokat kiegészítők nélkül.
Az eredeti játékok szokásos `.sav` fájlokba mentenek, amelyeket az ironwail
és a vkQuake is be tud tölteni; a testreszabott játékok `.qsa` fájlokba
mentenek, amelyeket csak ez a port tölt be (ugyanígy az osztott képernyős
és kooperatív játékok, amelyeket más portok nem tudnak menteni).
Testreszabott játék közben a Főmenü → Testreszabás bármikor módosítja a
játék módosítóit és kiegészítőit; az eredeti játékok szabályai rögzítettek.
A módosítók bármelyik moddal működnek, deathmatchben pedig kikapcsolnak.

- Nehezebb: több elszenvedett sebzés, szívósabb ellenségek, kevesebb
  felvehető lőszer és életerő, 2–4× szörny, minden pálya kezdése nulláról,
  nincs kézi mentés.
- Könnyebb: kevesebb elszenvedett sebzés, gyengébb ellenségek, több lőszer
  és életerő, végtelen lőszer, életerő-regeneráció, nincs esési sebzés,
  nincs fulladás, nincs önsebzés, dupla vagy tripla ugrás.

## Demók

A `record <név>` és a `stop` demót vesz fel a felhasználói mappádba; a
`playdemo <név>` lejátssza. A felvétel leállításakor a demó a háttérben
átíródik, hogy az ironwail és a vkQuake is le tudja játszani. Ha egy mod
olyan részecskeeffekteket használ, amelyeket más portok nem tudnak
megjeleníteni, a teljes változat `<név>.qsd` néven megmarad
(`playdemo <név>.qsd`). A `demo_convert <név>` egy régebbi demót alakít át.

A Főmenü → Demók a mentett demóidat mutatja, és ha a Beállítások → Demók →
Demók automatikus felvétele be van kapcsolva, minden játszott pálya
automatikus demóját is; a legrégebbiek lecserélődnek, ha a helyek beteltek.
Gépeléssel szűrheted a listákat, a Tab a rendezést váltja; a lista melletti
gombok lejátsszák, exportálják, átnevezik, megtartják vagy törlik a
kiválasztott demót, és az átnevezett automatikus demó megmarad. A
hibajelentések tartalmazzák az aktuális pálya automatikus demóját.

Demó közben a Szóköz szüneteltet, a Bal vagy Jobb nyomva tartása visszateker
vagy előreteker, a Fel és Le a sebességet állítja, a PgUp és PgDn 10
másodpercet ugrik, a Home és End az elejére és a következő pályára visz. Az
I és O kijelöl egy klipet; az E azt vagy az egész demót a `videos` mappába
exportálja. A vezérlők a képernyőn maradnak; a H vagy az Elrejtés elrejti
őket, és ha az alsó szélre mutatsz, egy sáv jelenik meg, amely visszahozza
őket. Kattints a gombokra, kattints az idővonalra, hogy oda ugorj, vagy húzd
végig rajta az egeret egy klip kijelöléséhez; az Exportálás megkérdezi a
képkockasebességet, a méretet, a formátumot és a fájlt, és hogy a kijelölt
klipet vagy az egész demót exportálja-e.

Kontrolleren az A szüneteltet, a D-pad hátra- és előretekeri, és sebességet
vált, az LB és RB 10 másodpercet ugrik, a karok lenyomása pedig az elejére
és a következő pályára visz. Az LT és RT kijelöl egy klipet, a B törli, az X
exportál, az Y megtart egy automatikus demót, a View pedig elrejti a
vezérlőket.

A `capture` játék közben elindítja és leállítja a videofelvételt. A
képkockasebesség, a formátum és a minőség a Beállítások → Demók →
Videofelvétel alatt van. A videókhoz ffmpeg kell; nélküle a képkockák
PNG-képként és egy WAV-fájlként mentődnek.

## Akadálymentesség

Options → Akadálymentesség:

- Kevesebb mozgás és a képernyővillanások erőssége.
- Színtévesztő-szűrők.
- Testreszabható célkereszt: alak, méret, körvonal, valamint színváltás
  célponton vagy öléskor; opcionális találatjelzők.
- **Hangfeliratok**; az üzenetek és feliratok helye, mérete és megjelenési
  ideje a HUD-szerkesztőben állítható.
- Kapcsolható (nyomva tartás helyett) támadás és futás.
- Játéksebesség-csúszka az egész játék lelassításához.
- Automatikus célzás befogásjelzővel, valamint kúp- és hatótáv-beállítással.

## Fejlesztések

Options → Fejlesztések. Mind kliensoldali és alapból ki van kapcsolva, így
bármelyik moddal és online is működnek.

- **Törmelék**: guruló húscafatok és a széttört hordókból repülő repeszek,
  a modok szörnyeinél is; nekiütközhetnek az ajtóknak és lifteknek.
- **Továbbfejlesztett vér**: fröccsök a felületeken, a testeket követő
  sebek, repülő cseppek, amelyek szétloccsannak, ahol földet érnek.
- **Becsapódási effektek**: sörétnyomok, fába és ellenségekbe álló szögek,
  robbanási égésnyomok.
- **Továbbfejlesztett víz**: áttetszőség, hullámzás, csobbanások és
  kausztika; **Víz alatti fénytörés**, **Cseppek a képernyőn**.
- **Átlátszó teleporterek**, amelyek parallaxissal mutatják a
  célállomásukat.
- Környezeti takarás, objektumonkénti és sebességi mozgási elmosódás, lágy
  részecskék.
- **Képernyő-shaderek**: RetroArch `.slangp` előbeállítások (CRT és mások).

Az Options → Grafika alatt megtalálható a szokásos renderelési skála,
élsimítás, supersampling, textúraszűrés, 8 bites színek ditheringgel,
köd, színes megvilágítás, valamint animáció- és mozgásinterpolálás is.

## Kiegészítők

A kiegészítők új fegyverek és felszerelések, amelyeket a motor futtat, így
bármelyik mod progs-ával működnek. Csak egyjátékos és kooperatív módban
érhetők el.

### Játék indítása kiegészítőkkel

Egy játékban csak akkor vannak kiegészítők, ha velük indul:

1. Single Player → jelöld be a **Kiegészítők** négyzetet, majd New Game.
2. Válaszd ki, mely kiegészítőket viszed magaddal, állítsd be őket, majd
   indítsd el az új játékot.

A kiegészítős játékok `.sav` helyett `.qsa` fájlokba mentenek, hogy más
portok ne próbálják betölteni őket. A mentéskártyákon „ADD-ONS” jelvény
látható. Egy normál mentés betöltése kikapcsolja a kiegészítőket.
Kiegészítős játék közben a Kiegészítők oldal (konzol: `menu_options addons`)
egyenként be- és kikapcsolja a kiegészítőket; mindegyikhez tartozik
ismétlődő előnézet, útmutató, hozzárendelések és saját beállítások.

### Fegyverek

A kiegészítők a szokásos fegyverhelyekre kerülnek (kiegészítőnként
választható, vagy a 9-es és 0-s hely), és megjelennek a fegyverkeréken és a
HUD-on. A tüzelés és a másodlagos tűz a szokásos támadógombjaid.

| Kiegészítő | Tüzelés | Másodlagos tűz | Lőszer |
|---|---|---|---|
| **Portálfegyver** | Lerakja a kék portált | Lerakja a narancssárga portált | Nincs |
| **Railgun** | Átütő lövedék, amely mindent eltalál egy vonalban | — | 2 cella |
| **Csáklya** | Beakad fába, fémbe, kőbe vagy ellenségbe, és odahúz | — | Nincs |
| **Tapadóbomba-vető** | Nyomva tartva tölt, elengedve kilő | Robbantás | 1 rakéta |
| **Bumeráng** | Nyomva tartva megjelöli az ellenségeket, elengedve eldobod; sorra elkábítja őket | Jelölések törlése | Nincs |
| **Zselészóró** | Kék taszító zselé (pattanós) | Narancssárga gyorsító zselé (gyors) | Nincs |
| **Nagynyomású mosó** | Hosszú vízsugár, amely hátralöki az ellenségeket és lemossa a zselét | Széles sugárlegyező | Nincs |
| **Apa-apa kábel** | A másodlagos tűz egy elérhető, világító elektronikus panelre bedugja az egyik végét; a másik ezután korlátlan lőszerrel villámokat lő | A másodlagos tűz vagy fegyverváltás kirántja | Nincs |
| **Akimbo pisztolyok** | Bal | Jobb | Nincs (Újratöltés: R) |
| **Gépkarabély** | Teljesen automata puska, amely a szögeidből tölt újra | A Másodlagos tűz a vörös pontos irányzékon át céloz: pontosabb lövések és közelebbi nézet | Szögek (Újratöltés: R) |

A portálok sík világfelületekre kerülnek, és lendületükkel együtt
átviszik rajtuk téged, a szörnyeket és az eldobott tárgyakat is. Bekapcsolt
**Átlátás a portálokon** mellett élőben látod a túloldalt, a portálokon
át látható portálokat is; a minőség, az élő nézetek száma, a távolság és a
rekurzió mélysége állítható.

### Felszerelés

A felszerelést be- és kikapcsolni kell, nem kézben tartani. Nyisd meg a
**felszereléskereket** (`+equipwheel`), vagy a `goggles cycle` paranccsal
léptesd végig:

- **Éjjellátó**
- **Hőkamera**

## Többjátékos mód

A LAN-on és interneten zajló NetQuake-játék mellett (15-ös, 666-os és 999-es
protokoll) a port **QuakeWorld** szerverekhez is csatlakozik. A Multiplayer
→ Join Game címet fogad el, vagy a LAN-on keres. Az FTE szkriptelt
részecskéi támogatottak az ezeket használó modoknál.

## Parancssor és konzol

Gyakori kapcsolók: `-basedir <mappa>`, `-game <mappa>`, `-hipnotic` /
`-rogue` / `-quoth`, `-width <w>`, `-height <h>`, `-fullscreen`, `-window`,
valamint `+parancs argumentumok` a konzolparancsok indításkori
futtatásához. A konzol (`~`) a szokásos Quake-parancsokat és cvarokat
fogadja el.

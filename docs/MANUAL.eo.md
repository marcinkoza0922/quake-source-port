# Uzanto-manlibro

Ĉi tiu manlibro priskribas kiel ludi Quake per ĉi tiu motoro, kun emfazo
pri tio, kion ĝi aldonas al la originala Quake kaj al aliaj motoroj. Ĉio
nova estas nedeviga: kun la defaŭltaj agordoj la ludo aspektas kaj ludiĝas
kiel Quake.

## Unuaj paŝoj

1. Akiru vian propran kopion de Quake. Montru al la motoro la dosierujon,
   kiu enhavas `id1/` (kaj eventualajn modojn apude), per `QUAKE_BASEDIR`
   aŭ `-basedir <dosierujo>`.
2. Lanĉu `cargo run --release`, aŭ tuj komencu modon per
   `-game <dosierujo> +map <mapo>`.
3. Agordoj, konservoj kaj ekrankopioj troviĝas en
   `~/.local/share/quake-source-port/<game>/`, po unu dosierujo por modo.

La ĉefmenuo enhavas Single Player, Multiplayer, Options, **Help** (ĉi tiun
manlibron), Quit kaj **Mappakoj**, kun la Quake-temo ludanta en la fono
(Options → Aŭdio → Muziko de ĉefmenuo). Ĉiu menuo funkcias per muso,
klavaro, ludregilo kaj tuŝo; ŝvebigu la muson super opcio aŭ elektu ĝin por
vidi klarigan ŝpruchelpilon.

En la helpo, supren/malsupren elektas sekcion; Page Up/Page Down,
maldekstren/dekstren, la ŝultraj butonoj aŭ la musrado super la teksto
rulumas ĝin.

## Lingvo

Options → Ludado → **Lingvo** elektas la lingvon de la menuoj, de la
surekranaj mesaĝoj kaj de ĉi tiu manlibro: angla, franca, germana, itala,
hispana, rusa, pola, ukraina, ĉeĥa, slovaka, kroata, bulgara, sveda, finna,
dana, norvega, nederlanda, greka, hungara, rumana, portugala (Portugalio aŭ
Brazilo), japana, korea, ĉina (simpligita aŭ tradicia), turka, indonezia,
malaja, filipina, vjetnama, serba, litova, latva, estona, malta, albana,
armena, kartvela, islanda, irlanda, kimra, eŭska, kataluna, galega, latina,
Esperanto, belorusa, makedona, slovena aŭ svahila.
**Sistemo** sekvas la lingvon de via komputilo. La propra ludteksto de la
reeldono de 2021 estas tradukita nur al la franca, germana, itala, hispana
kaj rusa; aliaj lingvoj montras ĝin en la angla. En lingvoj krom la angla
la malgranda teksto en la ludo uzas la menuan tiparon, ĉar la klasika
Quake-tiparo havas nur latinajn literojn sen supersignoj. Konzolaj mesaĝoj
restas en la angla, krom se **Traduki konzolon** estas ŝaltita.

## Mappakoj (modfoliumilo)

Ĉefmenuo → Mappakoj listigas ĉiun instalitan modon kaj mappakon kun ĝia
priskribo. Elektu unu por ŝanĝi al ĝi, poste foliumu ĝiajn mapojn kaj
komencu iun ajn el ili rekte. La foliumilo registras viajn plej bonajn
mortigojn, sekretojn kaj tempon por ĉiu mapo. Ĉiu modo havas siajn proprajn
agordojn, do asignoj kaj agordoj adaptitaj por unu modo ne influas alian.

**Kien iras mappakoj.** La motoro serĉas ilin en la baza dosierujo de Quake
(tiu, kiu enhavas `id1/`, difinita per `QUAKE_BASEDIR` aŭ `-basedir`). Ĉiu
subdosierujo tie, kiu enhavas luddatumojn (dosieron `.pak` aŭ `.pk3`,
`progs.dat` aŭ dosierujon `maps/`), estas listigita; do instalu mappakon
malpakante ĝin en ĝian propran dosierujon apud `id1/`, ekzemple
`<basedir>/ad/` aŭ `<basedir>/my_maps/maps/*.bsp`. Laŭvole aldonu
`modinfo.txt` kun linioj `title`, `base` kaj `description` por regi kiel ĝi
estas listigita. Pakoj estas trovataj kiam la ludo startas, do rekomencu
post aldono de nova. Konservoj, ekrankopioj kaj agordoj de ĉiu pako troviĝas
en `~/.local/share/quake-source-port/<folder>/`, ne en la dosierujo de la
pako.

## Dividita ekrano

Ĝis ses ludantoj sur unu komputilo, kunlabore aŭ en deathmatch.

- Multiplayer → New Game → **Lokaj ludantoj** difinas kiom da homoj ludas.
- Ĉiu ludanto ricevas sian propran vidon, HUD, sonon kaj enigan aparaton
  (klavaro kaj muso por ludanto 1, po unu ludregilo por la ceteraj).
- La ekranaranĝo povas esti stakigita, flank-al-flanke, 2×2 aŭ 3×2
  (`r_splitscreen_layout`).
- Ĉiu ludanto elektas nomon kaj kolorojn de ĉemizo kaj pantalono en
  Multiplayer → Setup.

## Ludregiloj

La motoro estas desegnita por ludado per ludregilo.

- **Armila rado** (`+weaponwheel`): tenu por malrapidigi la tempon kaj
  elekti armilon per iu ajn stango aŭ per la muso; malpremu por ŝanĝi.
- **Celhelpo** (Options → Enigo → Ludregilo): malrapidigo proksime al
  malamikoj kaj turnado, kiu sekvas ilin. Ĝi aplikiĝas nur al la stango,
  neniam al la muso.
- **Giroskopa celado** (Options → Enigo → Ludregilo → Giroskopo): ŝaltilo,
  plus aparta sentemo por la horizontala, vertikala kaj rula aksoj (la rula
  akso estas defaŭlte malŝaltita kaj turnas la vidon kiel la horizontala;
  negativaj valoroj inversigas ĝin), brufiltrilo, kaj kiam la giroskopo
  aplikiĝas (ĉiam, malŝaltita aŭ nur dum `+gyroaction` estas tenata).
  **Celstilo** ŝanĝas la dekstran stangon inter 2D-celado (ĝi turnas la
  vidon senĉese) kaj **flick stick** (puŝu la stangon por tuj turni la
  vidon al tiu direkto, poste turnu la stangon por rotacii ĝin); ambaŭ
  bone kombiniĝas kun la giroskopo por preciza celado.
- **Vibrado**, sentemo kaj inversigo de la stangoj.
- **Butonaj simboloj** kongruas kun via ludregilo (Xbox, PlayStation,
  Nintendo) kaj estas detektataj aŭtomate.
- Reasignu ĉion en Options → Regiloj, ankaŭ de la ludregilo.
- Tuŝekranaj regiloj kun agordebla grando, opakeco kaj rigardrapido.

## Konservado

- **Aŭtokonservoj** rotacias aparte de manaj kaj rapidaj konservoj, do ili
  neniam anstataŭigas viajn proprajn. Options → Ludado → Agordoj de
  aŭtokonservo elektas kiom da ili teni kaj kiam konservi: ĉe eniro en
  nivelon, laŭ tempigilo, ĉe trovo de sekreto aŭ de ĉiuj sekretoj, kaj kiam
  ĉiuj malamikoj mortis.
- **Konservaj kartoj** en la menuoj Load/Save montras la nivelon, sanon,
  kirason, armilojn, ŝlosilojn kaj mortigojn/sekretojn de ĉiu konservo.
- **Ĉe morto** (Options → Ludado → Ĉe morto), la unuludanta reĝimo povas
  demandi kion fari en paŭzita menuo, ŝargi la plej lastan konservon, lasi
  vin elekti konservon, aŭ rekomenci la nivelon.

## Ludado kaj kamerao

Options → Ludado:

- Vidkampo 60–130, balanciĝo de vido, pafila repuŝo, ĉiam kuri.
- **Kamerao**: unua persono, tria persono, aŭ super la ŝultro (dekstra aŭ
  maldekstra).
- Pozicio de **armilmodeloj**: centre, dekstre, maldekstre aŭ kaŝitaj.

## Adapti: ludmodifiloj kaj aldonaĵoj

Ludo estas aŭ *originala* aŭ *adaptita*, elektite kiam ĝi komenciĝas. New
Game (kaj elekto de nivelo en Elekto de nivelo aŭ Mappakoj) montras
ŝprucfenestron: **Ludi** komencas la ludon, **Adapti la sperton** malfermas
Ludmodifiloj kaj Aldonaĵoj. Dum io ajn estas ŝanĝita, Ludi estas markita
**ADAPTITA**, kaj **Malŝalti ĉiujn modifojn** (Del, X sur ludregilo, aŭ
klako) remetas la normajn regulojn sen aldonaĵoj. Originalaj ludoj konservas
ordinarajn `.sav`-dosierojn, kiujn ironwail kaj vkQuake povas ŝargi;
adaptitaj ludoj konservas `.qsa`-dosierojn, kiujn nur ĉi tiu motoro ŝargas
(same por ludoj en dividita ekrano kaj kunludo, kiujn aliaj motoroj ne
povas konservi). Dum adaptita ludo, Ĉefmenuo → Adapti ŝanĝas ĝiajn
modifilojn kaj aldonaĵojn iam ajn; la reguloj de originala ludo restas
fiksitaj. Modifiloj funkcias kun ĉiu modo kaj malŝaltiĝas en deathmatch.

- Pli malfacile: pli da ricevita damaĝo, pli fortaj malamikoj, malpli da
  municiaj kaj sanaj prenaĵoj, 2–4x monstroj, komenco de nulo en ĉiu nivelo,
  neniuj manaj konservoj.
- Pli facile: malpli da ricevita damaĝo, pli malfortaj malamikoj, pli da
  municio kaj sano, senfina municio, sanregenerado, neniu faldamaĝo, neniu
  dronado, neniu memdamaĝo, duoblaj aŭ trioblaj saltoj.

## Demoj

`record <name>` kaj `stop` registras demon en via uzanta dosierujo;
`playdemo <name>` ludas ĝin. Kiam la registrado ĉesas, la demo estas
reskribita en la fono, por ke ironwail kaj vkQuake ankaŭ povu ludi ĝin. Se
modo uzas partiklajn efektojn, kiujn aliaj motoroj ne povas montri, la plena
versio estas konservata kiel `<name>.qsd` (`playdemo <name>.qsd`).
`demo_convert <name>` konvertas pli malnovan demon.

## Alirebleco

Options → Alirebleco:

- Reduktita movado kaj intenso de ekranaj ekbriloj.
- Filtriloj por kolorblindeco.
- Agordebla celilo: formo, grando, konturo, kaj kolorŝanĝoj sur celo aŭ ĉe
  mortigo; nedevigaj trafmarkoj.
- **Sonaj subtekstoj** kaj agordoj de mesaĝoj (Mesaĝoj kaj subtekstoj),
  skalado de teksto.
- Baskuli (anstataŭ teni) por ataki kaj kuri.
- Ŝovilo de ludrapido por malrapidigi la tutan ludon.
- Aŭtomata celado kun indikilo de celkapto kaj agordoj de konuso kaj atingo.

## Plibonigoj

Options → Plibonigoj. Ĉio funkcias ĉe la kliento kaj estas defaŭlte
malŝaltita, do ĝi funkcias kun ĉiu modo kaj enrete.

- **Derompaĵoj**: ruliĝantaj karnopecoj kaj ŝrapnelo el frakasitaj bareloj,
  ankaŭ kun modaj monstroj; ili povas frapiĝi kontraŭ pordoj kaj liftoj.
- **Plibonigita sango**: ŝprucoj sur surfacoj, vundoj kiuj sekvas korpojn,
  flugantaj gutoj kiuj makulas kie ili falas.
- **Trafefektoj**: kuglotruoj de ĉaspafilo, najloj kiuj enfiksiĝas en ligno
  kaj malamikoj, eksplodaj brulmakuloj.
- **Plibonigita akvo**: travidebleco, ondetoj, plaŭdoj kaj kaŭstikoj.
- **Travideblaj teleportiloj**, kiuj montras sian celon kun paralakso.
- Ĉirkaŭa ombrigo, movmalklarigo de objektoj kaj de rapido, molaj
  partikloj.
- **Ekranaj ombrigiloj**: RetroArch-antaŭagordoj `.slangp` (CRT kaj aliaj).

Options → Grafiko ankaŭ havas la kutimajn bildigan skalon, glatigon de
randoj, supersampligon, teksturfiltradon, 8-bitan koloron kun punktumado,
nebulon, koloran lumigadon kaj interpoladon de animacio kaj movado.

## Aldonaĵoj

Aldonaĵoj estas novaj armiloj kaj ekipaĵoj, regataj de la motoro, do ili
funkcias kun la progs de ĉiu modo. Ili disponeblas nur en unuludanta reĝimo
kaj kunludo.

### Komenci ludon kun aldonaĵoj

Ludo havas aldonaĵojn nur se ĝi komenciĝas kun ili:

1. Single Player → marku la markobutonon **Aldonaĵoj**, poste New Game.
2. Elektu kiujn aldonaĵojn kunpreni kaj agordu ilin, poste komencu la novan
   ludon.

Ludoj kun aldonaĵoj konserviĝas kiel `.qsa`-dosieroj anstataŭ `.sav`, por
ke aliaj motoroj ne provu ŝargi ilin. Konservaj kartoj montras insignon
"ADD-ONS". Ŝargo de normala konservo malŝaltas la aldonaĵojn. Dum ludo kun
aldonaĵoj, la paĝo Aldonaĵoj (konzolo: `menu_options addons`) ŝaltas kaj
malŝaltas unuopajn aldonaĵojn; ĉiu havas buklan antaŭrigardon, instrukciojn,
siajn asignojn kaj siajn proprajn agordojn.

### Armiloj

Aldonaĵoj iras en la kutimajn armilajn fendojn (elektu kiun por ĉiu
aldonaĵo, aŭ fendojn 9 kaj 0) kaj aperas sur la armila rado kaj en la HUD.
Pafo kaj alt. pafo estas viaj kutimaj atakbutonoj.

| Aldonaĵo | Pafo | Alt. pafo | Municio |
|---|---|---|---|
| **Portalpafilo** | Metas la bluan portalon | Metas la oranĝan portalon | Neniu |
| **Relpafilo** | Trapika kuglo, kiu trafas ĉion sur linio | — | 2 ĉeloj |
| **Hokpafilo** | Kroĉiĝas kaj tiras vin al ligno, metalo, ŝtono aŭ malamikoj | — | Neniu |
| **Ĵetilo de gluaj bomboj** | Tenu por ŝargi, malpremu por lanĉi | Detonaciigi | 1 raketo |
| **Bumerango** | Tenu por marki malamikojn, malpremu por ĵeti; ĝi svenigas ĉiun laŭvice | Forigi markojn | Neniu |
| **Ĝelŝprucigilo** | Blua repuŝa ĝelo (resaltiga) | Oranĝa propulsa ĝelo (rapida) | Neniu |
| **Premlavilo** | Longa akvoŝpruco, kiu puŝas malamikojn kaj forlavas ĝelon | Larĝa ventumilo de ŝprucoj | Neniu |

Portaloj iras sur plataj surfacoj de la mondo kaj transportas vin, monstrojn
kaj ĵetitajn objektojn kun ilia impeto. Kun **Vidi tra portaloj** ŝaltita vi
vidas la alian flankon vive, inkluzive de portaloj videblaj tra portaloj;
kvalito, nombro de vivaj vidoj, distanco kaj rekursia profundo estas
agordeblaj.

### Ekipaĵoj

Ekipaĵoj estas ŝaltataj kaj malŝaltataj, ne tenataj en la mano. Malfermu la
**ekipaĵan radon** (`+equipwheel`), aŭ uzu `goggles cycle` por trairi:

- **Noktvido**
- **Termika vido**

## Plurludanta reĝimo

Krom ludado per LAN kaj interreto per NetQuake (protokoloj 15, 666 kaj 999)
la motoro konektiĝas al **QuakeWorld**-serviloj. Multiplayer → Join Game
akceptas adreson aŭ serĉas en la LAN. Skriptitaj partikloj de FTE estas
subtenataj por modoj, kiuj uzas ilin.

## Komandlinio kaj konzolo

Oftaj opcioj: `-basedir <dir>`, `-game <dir>`, `-hipnotic` / `-rogue` /
`-quoth`, `-width <w>`, `-height <h>`, `-fullscreen`, `-window`, kaj
`+command args` por plenumi konzolajn komandojn ĉe lanĉo. La konzolo (`~`)
akceptas la kutimajn Quake-komandojn kaj cvar-ojn.

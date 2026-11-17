# Manwal tal-utent

Dan il-manwal jispjega kif tilgħab Quake b'dan il-port, l-aktar dak li
jżid fuq Quake oriġinali u fuq source ports oħra. Kull ħaġa ġdida hija
fakultattiva: bl-issettjar standard il-logħba tidher u tintlagħab bħal Quake.

## L-ewwel passi

1. Ikseb kopja tiegħek ta' Quake. Indika lill-port il-folder li fih
   `id1/` (u l-mods ħdejh), b'`QUAKE_BASEDIR` jew
   `-basedir <folder>`.
2. Ħaddem `cargo run --release`, jew ibda mod dritt b'
   `-game <folder> +map <mappa>`.
3. Il-konfigurazzjoni, is-salvataġġi u l-iscreenshots jinżammu f'
   `~/.local/share/quake-source-port/<logħba>/`, folder wieħed għal kull mod.

Il-menu prinċipali għandu Single Player, Multiplayer, Options, **Help** (dan
il-manwal), Quit u **Pakketti tal-mappep**, bit-tema ta' Quake idoqq fl-isfond
(Options → Awdjo → Mużika tal-menu prinċipali). Kull menu jaħdem bil-maws,
bit-tastiera, bil-kontrollur u bil-mess; għaddi minn fuq għażla jew agħżilha
biex tara spjegazzjoni qasira tagħha.

Fil-Help, fuq/isfel jagħżlu taqsima; Page Up/Page Down, xellug/lemin, il-buttuni
tal-ispallejn jew ir-rota tal-maws fuq it-test jiskrolljawh.

## Lingwa

Options → Gameplay → **Lingwa** tagħżel il-lingwa tal-menus, tal-messaġġi fuq
l-iskrin u ta' dan il-manwal: Ingliż, Franċiż, Ġermaniż, Taljan, Spanjol,
Russu, Pollakk, Ukren, Ċek, Slovakk, Kroat, Bulgaru, Svediż, Finlandiż,
Daniż, Norveġiż, Olandiż, Grieg, Ungeriż, Rumen, Portugiż (il-Portugall jew
il-Brażil), Ġappuniż, Korean, Ċiniż (Simplifikat jew Tradizzjonali), Tork,
Indoneżjan, Malajan, Filippin, Vjetnamiż, Serb, Litwan, Latvjan, Estonjan,
Malti, Albaniż, Armen, Ġorġjan, Iżlandiż, Irlandiż, Welsh, Bask, Katalan,
Galizjan, Latin, Esperanto, Belarussu, Maċedonjan, Sloven jew Swaħili.
**Sistema** ssegwi l-lingwa tal-kompjuter tiegħek. It-test tal-logħba
tal-edizzjoni l-ġdida tal-2021 huwa tradott biss bil-Franċiż, bil-Ġermaniż,
bit-Taljan, bl-Ispanjol u bir-Russu; lingwi oħra juruh bl-Ingliż. F'lingwi
li mhumiex l-Ingliż, it-test żgħir fil-logħba juża l-font tal-menus, għax
il-font klassiku ta' Quake għandu biss ittri Latini bla aċċenti. Il-messaġġi
tal-konsol jibqgħu bl-Ingliż sakemm **Ittraduċi l-konsol** ma jkunx mixgħul.

## Pakketti tal-mappep (browser tal-mods)

Menu prinċipali → Pakketti tal-mappep juri kull mod u pakkett tal-mappep
installat bid-deskrizzjoni tiegħu. Agħżel wieħed biex taqleb għalih, imbagħad
fittex fost il-mappep tiegħu u ibda kwalunkwe waħda minnhom direttament.
Il-browser iżomm l-aħjar riżultati tiegħek ta' qtil, sigrieti u ħin għal
kull mappa. Kull mod iżomm il-konfigurazzjoni tiegħu, allura l-assenjazzjonijiet
u l-issettjar irranġati għal mod wieħed ma jgħaddux għal ieħor.

**Fejn imorru l-pakketti tal-mappep.** Il-port ifittixhom fil-folder bażi ta'
Quake (dak li fih `id1/`, iffissat b'`QUAKE_BASEDIR` jew `-basedir`). Kull
sottofolder hemmhekk li fih data tal-logħba (fajl `.pak` jew `.pk3`,
`progs.dat`, jew folder `maps/`) jidher fil-lista, allura biex tinstalla
pakkett tal-mappep tiftħu ġo folder għalih ħdejn `id1/`, pereżempju
`<basedir>/ad/` jew `<basedir>/my_maps/maps/*.bsp`. Jekk trid, żid fajl
`modinfo.txt` b'linji `title`, `base` u `description` biex tikkontrolla kif
jidher fil-lista. Il-pakketti jinstabu meta tibda l-logħba, allura erġa'
ibda wara li żżid wieħed. Is-salvataġġi, l-iscreenshots u l-konfigurazzjoni
ta' kull pakkett jinżammu f'`~/.local/share/quake-source-port/<folder>/`,
mhux fil-folder tal-pakkett.

## Skrin maqsum

Sa sitt lagħbiena fuq magna waħda, f'co-op jew f'deathmatch.

- Multiplayer → New Game → **Lagħbiena lokali** jiffissa kemm-il persuna
  tilgħab.
- Kull lagħħab ikollu l-veduta, il-HUD, il-ħoss u l-apparat tal-input
  tiegħu (tastiera u maws għall-lagħħab 1, kontrollur għal kull wieħed
  mill-oħrajn).
- It-tqassim tal-iskrin jista' jkun wieħed fuq l-ieħor, ħdejn xulxin, 2×2
  jew 3×2 (`r_splitscreen_layout`).
- Kull lagħħab jagħżel isem u l-kuluri tal-flokk/qalziet f'Multiplayer →
  Setup.

## Kontrolluri

Il-port huwa ddisinjat biex jintlagħab b'gamepad.

- **Rota tal-armi** (`+weaponwheel`): żommha biex tnaqqas il-ħin u tagħżel
  arma bi kwalunkwe stick jew bil-maws; erħi biex taqleb.
- **Għajnuna fl-immirar** (Options → Input → Kontrollur): tnaqqis
  tal-veloċità ħdejn l-għedewwa u rotazzjoni li ssegwihom. Tapplika biss
  għall-istick, qatt għall-maws.
- **Immirar bil-gyro** (Options → Input → Kontrollur → Gyro): swiċċ, flimkien
  ma' sensittività separata għall-assi orizzontali, vertikali u tar-roll
  (ir-roll huwa mitfi b'mod standard u jdawwar il-veduta bħall-assi
  orizzontali, valuri negattivi jaqilbuh), filtru tal-istorbju, u meta
  japplika l-gyro (dejjem, mitfi jew biss waqt li `+gyroaction` ikun miżmum).
  **Stil tal-immirar** jaqleb l-istick tal-lemin bejn immirar 2D (idawwar
  il-veduta kontinwament) u **flick stick** (imbotta l-istick biex il-veduta
  tmur mill-ewwel f'dik id-direzzjoni, imbagħad dawwar l-istick biex
  iddawwarha); it-tnejn jaqblu tajjeb mal-gyro għal immirar preċiż.
- **Vibrazzjoni**, sensittività u inverżjoni tal-isticks.
- **Simboli tal-buttuni** jaqblu mal-kontrollur tiegħek (Xbox, PlayStation,
  Nintendo) u jiġu skoperti awtomatikament.
- Erġa' assenja kollox f'Options → Kontrolli, anki mill-kontrollur.
- Kontrolli tal-iskrin tal-mess b'daqs, opaċità u veloċità tal-ħarsa
  aġġustabbli.

## Salvataġġ

- Is-**salvataġġi awtomatiċi** jduru separatament mis-salvataġġi manwali u
  mis-salvataġġi malajr, allura qatt ma jiktbu fuq tiegħek. Options →
  Gameplay → Issettjar tas-salvataġġ awtomatiku jagħżel kemm iżżomm u meta
  jsalva: fil-bidu tal-livell, perjodikament, meta ssib sigriet jew
  is-sigrieti kollha, u meta toqtol l-għedewwa kollha.
- Il-**kards tas-salvataġġi** fil-menus Load/Save juru l-livell, is-saħħa,
  l-armatura, l-armi, iċ-ċwievet u l-qtil/sigrieti ta' kull salvataġġ.
- **Mal-mewt** (Options → Gameplay → Mal-mewt), fil-logħob waħdek tista'
  tiġi mistoqsi x'għandek tagħmel f'menu ppawżat, jitella' l-aħħar
  salvataġġ, tagħżel salvataġġ, jew jerġa' jibda l-livell.

## Gameplay u kamera

Options → Gameplay:

- Kamp viżiv 60–130, tbandil tal-veduta, rinkul tal-arma, dejjem iġri.
- **Kamera**: l-ewwel persuna, it-tielet persuna, jew fuq l-ispalla (tal-lemin
  jew tax-xellug).
- Pożizzjoni tal-**mudelli tal-armi**: ċentru, lemin, xellug jew moħbija.

## Personalizza: modifikaturi tal-gameplay u add-ons

Logħba hija jew *vanilla* jew *personalizzata*, u dan jintgħażel meta tibda.
New Game (u l-għażla ta' livell f'Għażla tal-livell jew f'Pakketti tal-mappep)
juri popup: **Ilgħab** jibda l-logħba, **Personalizza l-esperjenza** jiftaħ
Modifikaturi tal-gameplay u Add-ons. Sakemm xi ħaġa tkun mibdula, Ilgħab
ikollu t-tikketta **PERSONALIZZATA** u **Itfi l-modifiki kollha** (Del, X
fuq il-gamepad, jew klikk) jerġa' jġib ir-regoli standard bla add-ons.
Il-logħob vanilla jissalva fajls `.sav` ordinarji li ironwail u vkQuake
jistgħu jtellgħu; il-logħob personalizzat jissalva fajls `.qsa` li jtellagħhom
dan il-port biss (hekk ukoll il-logħob fuq skrin maqsum u co-op, li ports
oħra ma jistgħux jissalvaw). Waqt logħba personalizzata, Menu prinċipali →
Personalizza jibdel il-modifikaturi u l-add-ons tagħha fi kwalunkwe ħin;
ir-regoli ta' logħba vanilla jibqgħu fissi. Il-modifikaturi jaħdmu ma'
kwalunkwe mod u jintfew fid-deathmatch.

- Aktar diffiċli: aktar ħsara meħuda, għedewwa aktar iebsa, inqas munizzjon
  u saħħa x'tiġbor, mostri 2–4x, bidu minn żero f'kull livell, bla
  salvataġġi manwali.
- Aktar faċli: inqas ħsara meħuda, għedewwa aktar dgħajfa, aktar munizzjon u
  saħħa, munizzjon bla tmiem, riġenerazzjoni tas-saħħa, bla ħsara
  mill-waqgħat, bla għarqa, bla ħsara lilek innifsek, qabżiet doppji jew
  tripli.

## Demos

`record <isem>` u `stop` jirrekordjaw demo fil-folder tal-utent tiegħek;
`playdemo <isem>` idoqqha. Meta tieqaf ir-reġistrazzjoni, id-demo tinkiteb
mill-ġdid fl-isfond biex ironwail u vkQuake ukoll ikunu jistgħu jdoqquha.
Jekk mod juża effetti tal-partiċelli li ports oħra ma jistgħux juru,
il-verżjoni sħiħa tinżamm bħala `<isem>.qsd` (`playdemo <isem>.qsd`).
`demo_convert <isem>` jikkonverti demo eqdem.

## Aċċessibbiltà

Options → Aċċessibbiltà:

- Moviment imnaqqas u intensità tat-teptip tal-iskrin.
- Filtri għall-għama tal-kulur.
- Crosshair konfigurabbli: forma, daqs, kontorn, u bidla fil-kulur fuq
  il-mira jew mal-qtil; indikaturi tal-laqta fakultattivi.
- **Sottotitoli tal-ħsejjes** u issettjar tal-messaġġi (Messaġġi u
  sottotitoli), daqs tat-test.
- Attakk u ġiri bi qlib (minflok iżżomm).
- Slider tal-veloċità tal-logħba biex tnaqqas il-veloċità tal-logħba kollha.
- Immirar awtomatiku b'indikatur tal-qbid, u issettjar tal-kon u tal-medda.

## Titjib

Options → Titjib. Kollox fuq in-naħa tal-klijent u mitfi b'mod standard,
allura jaħdem ma' kwalunkwe mod u online.

- **Fdalijiet**: biċċiet tal-laħam li jitqalbu u xrapnell minn btieti
  mfarrka, anki mal-mostri tal-mods; jistgħu jħabbtu mal-bibien u
  l-liftijiet.
- **Demm imtejjeb**: tixrid fuq l-uċuħ, ġrieħi li jsegwu l-iġsma, qtar
  fl-arja li jitfarrak fejn jinżel.
- **Effetti tal-impatti**: marki taċ-ċomb tas-senter, imsiemer li jeħlu
  fl-injam u fl-għedewwa, ħruq mill-isplużjonijiet.
- **Ilma mtejjeb**: trasluċidità, mewġ, tixrix u kawstiċi.
- **Teleporters trasparenti** li juru d-destinazzjoni tagħhom bil-parallax.
- Okklużjoni ambjentali, ċajpra tal-moviment għal kull oġġett u
  tal-veloċità, partiċelli rotob.
- **Shaders tal-iskrin**: presets `.slangp` ta' RetroArch (CRT u oħrajn).

Options → Grafika għandu wkoll l-issettjar tas-soltu: skala tar-rendering,
anti-aliasing, supersampling, filtru tat-tessituri, kulur 8-bit bid-dithering,
ċpar, dawl ikkulurit u interpolazzjoni tal-animazzjoni/moviment.

## Add-ons

L-add-ons huma armi u tagħmir ġodda, imħaddma mill-engine, allura jaħdmu
mal-progs ta' kwalunkwe mod. Huma għall-logħob waħdek u co-op biss.

### Kif tibda logħba b'add-ons

Logħba jkollha add-ons biss jekk tibda bihom:

1. Single Player → immarka l-kaxxa **Add-ons**, imbagħad New Game.
2. Agħżel liema add-ons iġġib miegħek u aġġusta l-issettjar tagħhom,
   imbagħad ibda l-logħba l-ġdida.

Il-logħob b'add-ons jissalva bħala fajls `.qsa` minflok `.sav`, biex ports
oħra ma jippruvawx jtellgħuhom. Il-kards tas-salvataġġi juru badge "ADD-ONS".
Jekk ittella' salvataġġ normali, l-add-ons jintfew. Waqt logħba b'add-ons,
il-paġna Add-ons (konsol: `menu_options addons`) tixgħel u titfi kull add-on
għalih; kull wieħed għandu preview f'loop, struzzjonijiet, l-assenjazzjonijiet
tiegħu u l-issettjar tiegħu.

### Armi

L-add-ons imorru fil-postijiet tal-armi normali (agħżel liema għal kull
add-on, jew il-post 9 u 0) u jidhru fir-rota tal-armi u fil-HUD. Spara u
sparar alternattiv huma l-buttuni tal-attakk tas-soltu tiegħek.

| Add-on | Spara | Sparar alternattiv | Munizzjon |
|---|---|---|---|
| **Pistola tal-portali** | Tpoġġi l-portal blu | Tpoġġi l-portal oranġjo | Xejn |
| **Railgun** | Balla li tinfed u tolqot kollox fuq linja | — | 2 ċelloli |
| **Ganċ** | Jaqbad u jiġbdek lejn l-injam, il-metall, il-ġebel jew l-għedewwa | — | Xejn |
| **Launcher tal-bombi li jeħlu** | Żomm biex tiċċarġja, erħi biex titfa' | Isploda | 1 rokit |
| **Boomerang** | Żomm biex timmarka l-għedewwa, erħi biex titfa'; jisturdi lil kull wieħed wara xulxin | Ħassar il-marki | Xejn |
| **Sprejer tal-ġell** | Ġell blu ta' repulsjoni (jaqbeż) | Ġell oranġjo ta' propulsjoni (veloċi) | Xejn |
| **Powerwasher** | Ġett twil tal-ilma li jimbotta l-għedewwa u jaħsel il-ġell | Imrewħa wiesgħa ta' ġettijiet | Xejn |

Il-portali jitqiegħdu fuq uċuħ ċatti tad-dinja u jgħadduk lilek, lill-mostri
u lill-oġġetti mitfugħa mill-ġenb għall-ieħor bil-momentum tagħhom.
B'**Ara minn ġol-portali** mixgħul tara n-naħa l-oħra live, inklużi
portali li jidhru minn ġo portali; il-kwalità, in-numru ta' veduti ħajjin,
id-distanza u l-fond tar-rikorsjoni huma aġġustabbli.

### Tagħmir

It-tagħmir jinxtegħel u jintefa minflok jinżamm f'idejk. Iftaħ ir-**rota
tat-tagħmir** (`+equipwheel`), jew uża `goggles cycle` biex taqleb bejn:

- **Viżjoni bil-lejl**
- **Viżjoni termika**

## Multiplayer

Minbarra l-logħob NetQuake fuq il-LAN u fuq l-internet (protokolli 15, 666 u
999), il-port jikkonnettja ma' servers **QuakeWorld**. Multiplayer → Join
Game jieħu indirizz jew ifittex fil-LAN. Il-partiċelli bl-iskripts ta' FTE
huma appoġġjati għall-mods li jużawhom.

## Linja tal-kmand u konsol

Għażliet komuni: `-basedir <folder>`, `-game <folder>`, `-hipnotic` / `-rogue` /
`-quoth`, `-width <w>`, `-height <h>`, `-fullscreen`, `-window`, u
`+command args` biex tħaddem kmandi tal-konsol fil-bidu. Il-konsol (`~`)
jaċċetta l-kmandi u l-cvars tas-soltu ta' Quake.

# Lietotāja rokasgrāmata

Šī rokasgrāmata apraksta Quake spēlēšanu ar šo portu, īpaši to, ko tas
pievieno salīdzinājumā ar oriģinālo Quake un citiem portiem. Viss jaunais ir
neobligāts: ar noklusējuma iestatījumiem spēle izskatās un spēlējas kā Quake.

## Darba sākšana

1. Iegādājies savu Quake kopiju. Norādi portam mapi, kurā atrodas `id1/`
   (un modi blakus tai), ar `QUAKE_BASEDIR` vai `-basedir <mape>`.
2. Palaid `cargo run --release` vai uzreiz sāc modu ar
   `-game <mape> +map <karte>`.
3. Konfigurācija, saglabājumi un ekrānuzņēmumi glabājas
   `~/.local/share/quake-source-port/<spēle>/`, pa vienai mapei katram
   modam.

Galvenajā izvēlnē ir Single Player, Multiplayer, Options, **Help** (šī
rokasgrāmata), Quit un **Karšu pakas**, un fonā skan Quake tēma
(Options → Audio → Galvenās izvēlnes mūzika). Katra izvēlne darbojas ar
peli, tastatūru, kontrolieri un skārienekrānu; novieto kursoru virs opcijas
vai izvēlies to, lai redzētu paskaidrojošu padomu.

Palīdzībā augšup/lejup izvēlas sadaļu; Page Up/Page Down, pa kreisi/pa
labi, plecu pogas vai peles ritenis virs teksta to ritina.

## Valoda

Options → Spēle → **Valoda** izvēlas izvēlņu, ekrāna ziņu un šīs
rokasgrāmatas valodu: angļu, franču, vācu, itāļu, spāņu, krievu, poļu,
ukraiņu, čehu, slovāku, horvātu, bulgāru, zviedru, somu, dāņu, norvēģu,
nīderlandiešu, grieķu, ungāru, rumāņu, portugāļu (Portugāle vai Brazīlija),
japāņu, korejiešu, ķīniešu (vienkāršotā vai tradicionālā), turku,
indonēziešu, malajiešu, filipīniešu, vjetnamiešu, serbu, lietuviešu,
latviešu, igauņu, maltiešu, albāņu, armēņu, gruzīnu, islandiešu, īru,
velsiešu, basku, katalāņu, galisiešu, latīņu, esperanto, baltkrievu,
maķedoniešu, slovēņu, svahili, arābu, ebreju, persiešu, urdu, hindi,
marathu, bengāļu, pandžabu, gudžaratu, tamilu, telugu, taju, khmeru, amharu vai
kantoniešu.
**Sistēma** seko datora valodai. 2021. gada atkārtotā izdevuma spēles
teksti ir tulkoti tikai franču, vācu, itāļu, spāņu un krievu valodā; citās
valodās tie tiek rādīti angliski. Valodās, kas nav angļu, sīkais spēles
teksts izmanto izvēlņu fontu, jo klasiskajā Quake fontā ir tikai latīņu
burti bez diakritiskajām zīmēm. Konsoles ziņas paliek angliski, ja vien nav
ieslēgts **Tulkot konsoli**.

## Karšu pakas (modu pārlūks)

Galvenā izvēlne → Karšu pakas parāda katru instalēto modu un karšu paku ar
tās aprakstu. Izvēlies vienu, lai pārslēgtos uz to, tad pārlūko tās kartes
un sāc jebkuru no tām tieši. Pārlūks atceras tavus labākos nogalināto,
noslēpumu un laika rezultātus katrā kartē. Katram modam ir sava
konfigurācija, tāpēc viena moda piesaistes un iestatījumi nenonāk citā.

**Kur liek karšu pakas.** Ports tās meklē Quake bāzes mapē (tajā, kurā ir
`id1/`, iestatīta ar `QUAKE_BASEDIR` vai `-basedir`). Katra tās apakšmape,
kurā ir spēles dati (`.pak` vai `.pk3` fails, `progs.dat` vai mape
`maps/`), tiek parādīta, tāpēc instalē karšu paku, atarhivējot to savā
mapē blakus `id1/`, piemēram, `<basedir>/ad/` vai
`<basedir>/my_maps/maps/*.bsp`. Pēc izvēles pievieno `modinfo.txt` ar
rindām `title`, `base` un `description`, lai noteiktu, kā tā tiek parādīta.
Pakas tiek atrastas spēles sākumā, tāpēc pēc jaunas pakas pievienošanas
restartē spēli. Katras pakas saglabājumi, ekrānuzņēmumi un konfigurācija
glabājas `~/.local/share/quake-source-port/<mape>/`, nevis pakas mapē.

## Dalītais ekrāns

Līdz sešiem spēlētājiem pie viena datora, kooperatīvajā režīmā vai
deathmatch.

- Multiplayer → New Game → **Lokālie spēlētāji** nosaka, cik cilvēku spēlē.
- Katram spēlētājam ir savs skats, HUD, skaņa un ievades ierīce (tastatūra
  un pele 1. spēlētājam, pa kontrolierim katram no pārējiem).
- Ekrāna izkārtojums var būt viens virs otra, blakus, 2×2 vai 3×2
  (`r_splitscreen_layout`).
- Katrs spēlētājs izvēlas vārdu un krekla/bikšu krāsas sadaļā
  Multiplayer → Setup.

## Kontrolieri

Ports ir veidots spēlēšanai ar kontrolieri.

- **Ieroču ripa** (`+weaponwheel`): turi, lai palēninātu laiku un izvēlētos
  ieroci ar jebkuru sviru vai peli; atlaid, lai pārslēgtu.
- **Tēmēšanas palīgs** (Options → Ievade → Kontrolieris): palēninājums
  ienaidnieku tuvumā un pagrieziens, kas seko tiem. Tas vienmēr attiecas
  tikai uz sviru, nekad uz peli.
- **Žiro tēmēšana** (Options → Ievade → Kontrolieris → Žiro): slēdzis, kā
  arī atsevišķa jutība horizontālajai, vertikālajai un sānsveres asij
  (sānsvere pēc noklusējuma ir izslēgta un griež skatu kā horizontālā ass,
  negatīvas vērtības to apgriež), trokšņu filtrs un tas, kad žiro darbojas
  (vienmēr, izslēgts vai tikai tad, kad turēts `+gyroaction`).
  **Tēmēšanas stils** pārslēdz labo sviru starp 2D tēmēšanu (tā nepārtraukti
  griež skatu) un **flick stick** (pagrūd sviru, lai uzreiz pavērstu skatu
  tajā virzienā, tad griez sviru, lai to pagrieztu); abi labi apvienojas ar
  žiro precīzai tēmēšanai.
- **Vibrācija**, sviru jutība un invertēšana.
- **Pogu simboli** atbilst tavam kontrolierim (Xbox, PlayStation, Nintendo)
  un tiek noteikti automātiski.
- Visu var pārpiesaistīt sadaļā Options → Vadība, arī no kontroliera.
- Skārienekrāna vadība ar regulējamu izmēru, necaurredzamību un skatīšanās
  ātrumu.

## Saglabāšana

- **Autosaglabājumi** rotējas atsevišķi no manuālajiem un ātrajiem
  saglabājumiem, tāpēc tie nekad nepārraksta tavējos. Options → Spēle →
  Autosaglabāšanas iestatījumi nosaka, cik daudz paturēt un kad saglabāt:
  ieejot līmenī, pēc taimera, atrodot noslēpumu vai visus noslēpumus un
  nogalinot visus ienaidniekus.
- **Saglabājumu kartītes** ielādes/saglabāšanas izvēlnēs rāda katra
  saglabājuma līmeni, veselību, bruņas, ieročus, atslēgas un
  nogalinātos/noslēpumus.
- **Pēc nāves** (Options → Spēle → Pēc nāves) viena spēlētāja režīmā var
  pauzētā izvēlnē jautāt, ko darīt, ielādēt jaunāko saglabājumu, ļaut
  izvēlēties saglabājumu vai sākt līmeni no jauna.

## Spēle un kamera

Options → Spēle:

- Redzes lauks 60–130, skata šūpošanās, ieroča atsitiens, vienmēr skriet.
- **Kamera**: pirmā persona, trešā persona vai pār plecu (labo vai
  kreiso).
- **Ieroču modeļu** novietojums: centrā, pa labi, pa kreisi vai paslēpti.

## Pielāgošana: spēles modifikatori un papildinājumi

Spēle ir vai nu *oriģināla*, vai *pielāgota*, un to izvēlas tās sākumā.
New Game (un līmeņa izvēle sadaļā Līmeņa izvēle vai Karšu pakas) parāda
uznirstošo logu: **Spēlēt** sāk spēli, **Pielāgot pieredzi** atver spēles
modifikatorus un papildinājumus. Kamēr kaut kas ir mainīts, Spēlēt ir
atzīmēts ar **PIELĀGOTS**, un **Atspējot visas modifikācijas** (Del,
kontroliera X vai klikšķis) atjauno standarta noteikumus bez
papildinājumiem. Oriģinālās spēles saglabā parastus `.sav` failus, ko var
ielādēt ironwail un vkQuake; pielāgotās spēles saglabā `.qsa` failus, ko
ielādē tikai šis ports (tāpat arī dalītā ekrāna un kooperatīvās spēles, ko
citi porti nevar saglabāt). Pielāgotas spēles laikā Galvenā izvēlne →
Pielāgot jebkurā brīdī maina tās modifikatorus un papildinājumus;
oriģinālas spēles noteikumi paliek fiksēti. Modifikatori darbojas ar
jebkuru modu un deathmatch režīmā izslēdzas.

- Grūtāk: vairāk saņemto bojājumu, izturīgāki ienaidnieki, mazāk munīcijas
  un veselības atradumu, 2–4× monstri, starts ar pistoli katrā līmenī, bez
  manuālas saglabāšanas.
- Vieglāk: mazāk saņemto bojājumu, vājāki ienaidnieki, vairāk munīcijas un
  veselības, bezgalīga munīcija, veselības atjaunošanās, bez kritiena
  bojājumiem, bez noslīkšanas, bez paša bojājumiem, dubulti vai trīskārši
  lēcieni.

## Demo

`record <nosaukums>` un `stop` ieraksta demo tavā lietotāja mapē;
`playdemo <nosaukums>` to atskaņo. Kad ierakstīšana beidzas, demo fonā
tiek pārrakstīts, lai to varētu atskaņot arī ironwail un vkQuake. Ja mods
izmanto daļiņu efektus, ko citi porti nevar parādīt, pilnā versija tiek
saglabāta kā `<nosaukums>.qsd` (`playdemo <nosaukums>.qsd`).
`demo_convert <nosaukums>` pārveido vecāku demo.

Galvenā izvēlne → Demo rāda jūsu saglabātos demo un, ja ieslēgts Opcijas →
Demo → Automātiski ierakstīt demo, arī katra spēlētā līmeņa automātisko
demo; vecākie tiek aizstāti, kad vietas ir pilnas. Rakstiet, lai filtrētu
sarakstus, un Tab maina kārtošanu; pogas blakus sarakstam atskaņo,
eksportē, pārdēvē, patur vai dzēš izvēlēto demo, un pārdēvēts automātiskais
demo tiek paturēts. Kļūdu ziņojumos ir pašreizējā līmeņa automātiskais
demo.

Demo laikā Atstarpe pauzē, turot Pa kreisi vai Pa labi, tiek tīts atpakaļ
vai uz priekšu, Augšup un Lejup maina ātrumu, PgUp un PgDn pārlec 10
sekundes, bet Home un End iet uz sākumu un uz nākamo karti. I un O atzīmē
klipu; E to vai visu demo eksportē mapē `videos`. Vadīklas paliek ekrānā; H
vai Paslēpt tās paslēpj, un, norādot uz apakšējo malu, parādās josla, kas
tās atgriež. Klikšķiniet pogas, noklikšķiniet uz laika joslas, lai pārlēktu
turp, vai velciet pa to, lai iezīmētu klipu; Eksportēt jautā kadru ātrumu,
izmēru, formātu un failu, kā arī to, vai eksportēt iezīmēto klipu vai visu
demo.

Kontrolierī A pauzē, krustveida pogas tin atpakaļ un uz priekšu un maina
ātrumu, LB un RB pārlec 10 sekundes, un sviru nospiedieni ved uz sākumu un
uz nākamo karti. LT un RT atzīmē klipu, B to notīra, X eksportē, Y patur
automātisko demo, un View paslēpj vadīklas.

`capture` sāk un aptur video ierakstīšanu spēles laikā. Kadru ātrums,
formāts un kvalitāte ir sadaļā Opcijas → Demo → Video ierakstīšana. Video
vajag ffmpeg; bez tā kadri tiek saglabāti kā PNG attēli ar WAV failu.

## Pieejamība

Options → Pieejamība:

- Samazināta kustība un ekrāna uzliesmojumu intensitāte.
- Krāsu akluma filtri.
- Pielāgojams tēmeklis: forma, izmērs, kontūra un krāsas maiņa, esot uz
  mērķa vai nogalinot; neobligāti trāpījumu marķieri.
- **Skaņu subtitri** un ziņu iestatījumi (Ziņas & subtitri), teksta
  mērogošana.
- Uzbrukuma un skriešanas pārslēgšana (turēšanas vietā).
- Spēles ātruma slīdnis visas spēles palēnināšanai.
- Autotēmēšana ar fiksācijas indikatoru, konusa un attāluma iestatījumiem.

## Uzlabojumi

Options → Uzlabojumi. Viss darbojas klienta pusē un pēc noklusējuma ir
izslēgts, tāpēc darbojas ar jebkuru modu un tiešsaistē.

- **Atlūzas**: veļoši miesas gabali un šķembas no saplīsušām mucām, arī no
  modu monstriem; tās var atsisties pret durvīm un liftiem.
- **Uzlabotas asinis**: šļakatas uz virsmām, brūces, kas seko ķermeņiem,
  pilieni gaisā, kas nokrītot izšļakstās.
- **Trieciena efekti**: bises ložu pēdas, naglas, kas iestrēgst kokā un
  ienaidniekos, sprādzienu apdeguma pēdas.
- **Uzlabots ūdens**: caurspīdīgums, viļņi, šļakatas un kaustika.
- **Caurredzami teleporti**, kas rāda savu galamērķi ar paralaksi.
- Apkārtnes aizēnojums, objektu un ātruma kustības izplūdums, mīkstas
  daļiņas.
- **Ekrāna šeideri**: RetroArch `.slangp` iepriekšiestatījumi (CRT un
  citi).

Options → Grafika piedāvā arī ierastos iestatījumus: renderēšanas mērogu,
malu nogludināšanu, supersampling, tekstūru filtrēšanu, 8 bitu krāsas ar
ditheringu, miglu, krāsainu apgaismojumu un animācijas/kustības
interpolāciju.

## Papildinājumi

Papildinājumi ir jauni ieroči un aprīkojums, ko darbina dzinējs, tāpēc tie
darbojas ar jebkura moda progs. Tie ir pieejami tikai viena spēlētāja un
kooperatīvajā režīmā.

### Spēles sākšana ar papildinājumiem

Spēlē ir papildinājumi tikai tad, ja tā ar tiem sākta:

1. Single Player → atzīmē rūtiņu **Papildinājumi**, tad New Game.
2. Izvēlies, kurus papildinājumus ņemt līdzi, un pielāgo to iestatījumus,
   tad Sākt jaunu spēli.

Spēles ar papildinājumiem saglabājas kā `.qsa` faili, nevis `.sav`, tāpēc
citi porti nemēģina tās ielādēt. Saglabājumu kartītes rāda zīmi
"ADD-ONS". Parasta saglabājuma ielāde izslēdz papildinājumus. Spēlējot ar
papildinājumiem, lapa Papildinājumi (konsolē: `menu_options addons`)
ieslēdz un izslēdz atsevišķus papildinājumus; katram ir cikliska
priekšskatījuma animācija, instrukcijas, piesaistes un savi iestatījumi.

### Ieroči

Papildinājumi tiek ievietoti parastajos ieroču slotos (izvēlies katram
papildinājumam, vai slotu 9 un 0) un parādās ieroču ripā un HUD. Šāviens
un alt. šāviens ir tavas parastās uzbrukuma pogas.

| Papildinājums | Šāviens | Alt. šāviens | Munīcija |
|---|---|---|---|
| **Portālu ierocis** | Novieto zilo portālu | Novieto oranžo portālu | Nav |
| **Railgun** | Caurduroša lode, kas trāpa visam vienā līnijā | — | 2 elementi |
| **Kāsis** | Aizķeras aiz koka, metāla, akmens vai ienaidniekiem un pievelk tevi | — | Nav |
| **Lipīgo bumbu metējs** | Turi, lai uzlādētu, atlaid, lai izšautu | Uzspridzināt | 1 raķete |
| **Bumerangs** | Turi, lai atzīmētu ienaidniekus, atlaid, lai mestu; pēc kārtas apdullina katru | Notīrīt atzīmes | Nav |
| **Gela smidzinātājs** | Zilais atgrūšanas gels (atsperīgs) | Oranžais paātrinājuma gels (ātrs) | Nav |
| **Augstspiediena mazgātājs** | Gara ūdens strūkla, kas atgrūž ienaidniekus un nomazgā gelu | Plašs strūklu vēdeklis | Nav |

Portāli tiek novietoti uz līdzenām pasaules virsmām un pārnes tevi,
monstrus un mestus priekšmetus cauri, saglabājot impulsu. Ar ieslēgtu
**Redzēt cauri portāliem** tu redzi otru pusi tiešraidē, ieskaitot
portālus, kas redzami caur portāliem; kvalitāti, tiešo skatu skaitu,
attālumu un rekursijas dziļumu var regulēt.

### Aprīkojums

Aprīkojumu ieslēdz un izslēdz, nevis tur rokās. Atver **aprīkojuma ripu**
(`+equipwheel`) vai izmanto `goggles cycle`, lai pārslēgtu:

- **Nakts redzamība**
- **Termoredze**

## Vairāku spēlētāju spēle

Papildus LAN un interneta NetQuake spēlei (protokoli 15, 666 un 999) ports
pieslēdzas **QuakeWorld** serveriem. Multiplayer → Join Game
pieņem adresi vai meklē LAN tīklā. FTE skriptotās daļiņas tiek atbalstītas
modiem, kas tās izmanto.

## Komandrinda un konsole

Biežākās opcijas: `-basedir <mape>`, `-game <mape>`, `-hipnotic` /
`-rogue` / `-quoth`, `-width <w>`, `-height <h>`, `-fullscreen`,
`-window` un `+komanda argumenti`, lai palaistu konsoles komandas
startējot. Konsole (`~`) pieņem ierastās Quake komandas un cvar.

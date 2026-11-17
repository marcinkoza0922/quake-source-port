# Kasutusjuhend

See juhend kirjeldab Quake'i mängimist selle pordiga ja keskendub sellele,
mida see lisab algsele Quake'ile ja teistele portidele. Kõik uus on
valikuline: vaikesätetega näeb mäng välja ja mängib nagu Quake.

## Alustamine

1. Hangi endale oma Quake'i koopia. Suuna port kausta, mis sisaldab
   kausta `id1/` (ja selle kõrval olevaid mode), kasutades `QUAKE_BASEDIR`
   või `-basedir <kaust>`.
2. Käivita `cargo run --release` või alusta modiga kohe käsuga
   `-game <kaust> +map <kaart>`.
3. Seaded, salvestused ja ekraanipildid asuvad kaustas
   `~/.local/share/quake-source-port/<mäng>/`, iga modi jaoks oma kaust.

Peamenüüs on Single Player, Multiplayer, Options, **Help** (see juhend),
Quit ja **Kaardipaketid**, taustal mängib Quake'i teema
(Options → Heli → Peamenüü muusika). Iga menüü töötab hiire, klaviatuuri,
mängupuldi ja puuteekraaniga; vii kursor valikule või vali see, et näha
seda selgitavat vihjet.

Abiekraanil valib üles/alla jaotise; Page Up/Page Down, vasak/parem,
õlanupud või hiireratas teksti kohal kerivad seda.

## Keel

Options → Mängimine → **Keel** valib menüüde, ekraaniteadete ja selle
juhendi keele: inglise, prantsuse, saksa, itaalia, hispaania, vene, poola,
ukraina, tšehhi, slovaki, horvaadi, bulgaaria, rootsi, soome, taani,
norra, hollandi, kreeka, ungari, rumeenia, portugali (Portugal või
Brasiilia), jaapani, korea, hiina (lihtsustatud või traditsiooniline),
türgi, indoneesia, malai, filipiini, vietnami, serbia, leedu, läti, eesti,
malta, albaania, armeenia, gruusia, islandi, iiri, kõmri, baski, katalaani,
galeegi, ladina, esperanto, valgevene, makedoonia, sloveeni, suahiili,
araabia, heebrea, pärsia, urdu, hindi, marathi, bengali, pandžabi,
gudžarati, tamili, telugu, tai, khmeeri, amhara või kantoni.
**System** järgib arvuti keelt. 2021. aasta uusväljaande enda mängutekst on
tõlgitud ainult prantsuse, saksa, itaalia, hispaania ja vene keelde;
teistes keeltes näidatakse seda inglise keeles. Muudes keeltes peale
inglise kasutab väike mängusisene tekst menüüfonti, sest Quake'i
klassikalises fondis on ainult täppideta ladina tähed. Konsooliteated
jäävad ingliskeelseks, kui **Tõlgi konsool** pole sisse lülitatud.

## Kaardipaketid (modide sirvija)

Peamenüü → Kaardipaketid loetleb kõik paigaldatud modid ja kaardipaketid
koos kirjeldusega. Vali üks, et sellele lülituda, seejärel sirvi selle
kaarte ja alusta ükskõik millist neist otse. Sirvija jätab meelde sinu
parimad tulemused iga kaardi kohta: tapetud vaenlased, saladused ja aeg.
Igal modil on oma seaded, nii et ühe modi jaoks häälestatud klahvid ja
sätted ei leki teise.

**Kuhu kaardipaketid käivad.** Port otsib neid Quake'i baaskaustast (sellest,
milles on `id1/`, määratud `QUAKE_BASEDIR` või `-basedir` abil). Loetletakse
iga sealne alamkaust, mis sisaldab mänguandmeid (`.pak`- või `.pk3`-faili,
`progs.dat`-faili või kausta `maps/`), seega paigalda kaardipakett, pakkides
selle lahti eraldi kausta `id1/` kõrval, näiteks `<basedir>/ad/` või
`<basedir>/my_maps/maps/*.bsp`. Soovi korral lisa fail `modinfo.txt` ridadega
`title`, `base` ja `description`, et määrata, kuidas paketti loetletakse.
Paketid leitakse mängu käivitamisel, seega taaskäivita mäng pärast uue
lisamist. Iga paketi salvestused, ekraanipildid ja seaded asuvad kaustas
`~/.local/share/quake-source-port/<kaust>/`, mitte paketi kaustas.

## Jagatud ekraan

Kuni kuus mängijat ühes arvutis, koostöö- või deathmatch-režiimis.

- Multiplayer → New Game → **Kohalikud mängijad** määrab, mitu inimest
  mängib.
- Igal mängijal on oma vaade, HUD, heli ja sisendseade (mängijal 1
  klaviatuur ja hiir, igal teisel oma mängupult).
- Ekraani paigutus võib olla üksteise all, kõrvuti, 2×2 või 3×2
  (`r_splitscreen_layout`).
- Iga mängija valib nime ning särgi ja pükste värvi menüüs Multiplayer →
  Setup.

## Mängupuldid

Port on loodud mängimiseks mängupuldiga.

- **Relvaratas** (`+weaponwheel`): hoia all, et aega aeglustada ja valida
  relv kumma tahes kangi või hiirega; vabasta, et relva vahetada.
- **Sihtimisabi** (Options → Sisend → Mängupult): aeglustus vaenlaste
  lähedal ja pööre, mis neid jälgib. See mõjub alati ainult kangile, mitte
  kunagi hiirele.
- **Güro-sihtimine** (Options → Sisend → Mängupult → Güro): lüliti ning
  eraldi tundlikkus pöörde-, kalde- ja veeretelje jaoks (veeretelg on
  vaikimisi väljas ja pöörab vaadet nagu pöördetelg, negatiivsed väärtused
  pööravad suuna ümber), mürafilter ning see, millal güro kehtib (alati,
  väljas või ainult siis, kui hoitakse `+gyroaction`). **Sihtimise stiil**
  vahetab parema kangi 2D-sihtimise (pöörab vaadet sujuvalt) ja
  **flick stick**'i vahel (lükka kangi, et vaade kohe sinna suunda pöörata,
  seejärel keera kangi, et seda edasi pöörata); mõlemad sobivad hästi
  kokku güroga täpseks sihtimiseks.
- **Vibratsioon**, kangi tundlikkus ja pööramine.
- **Nupusümbolid** vastavad sinu puldile (Xbox, PlayStation, Nintendo) ja
  tuvastatakse automaatselt.
- Kõike saab ümber määrata menüüs Options → Juhtimine, ka mängupuldilt.
- Puutejuhtimine reguleeritava suuruse, läbipaistmatuse ja vaate kiirusega.

## Salvestamine

- **Autosalvestused** vahelduvad käsitsi- ja kiirsalvestustest eraldi, nii
  et need ei kirjuta kunagi sinu omi üle. Options → Mängimine →
  Autosalvestuse seaded valib, mitu säilitada ja millal salvestada: taseme
  alguses, taimeriga, saladuse või kõigi saladuste leidmisel ja kõigi
  vaenlaste tapmisel.
- **Salvestuskaardid** laadimis- ja salvestusmenüüs näitavad iga
  salvestuse taset, tervist, soomust, relvi, võtmeid ning tapetud
  vaenlasi ja saladusi.
- **Surmal** (Options → Mängimine → Surmal) võib üksikmäng küsida pausil
  olevas menüüs, mida teha, laadida viimase salvestuse, lasta sul
  salvestuse valida või taset uuesti alustada.

## Mängimine ja kaamera

Options → Mängimine:

- Vaateväli 60–130, vaate õõtsumine, relva tagasilöök, alati jooksmine.
- **Kaamera**: esimene isik, kolmas isik või õla tagant (paremalt või
  vasakult).
- **Relvamudelid**: keskel, paremal, vasakul või peidetud.

## Kohandamine: mängu modifikaatorid ja lisad

Mäng on kas *tavaline* või *kohandatud*, valik tehakse selle alustamisel.
New Game (ja taseme valimine menüüs Taseme valik või Kaardipaketid) avab
hüpikakna: **Mängi** alustab mängu, **Kohanda kogemust** avab mängu
modifikaatorid ja lisad. Kui midagi on muudetud, on nupul Mängi silt
**KOHANDATUD** ja **Keela kõik muudatused** (Del, mängupuldi X või klõps)
taastab tavareeglid ilma lisadeta. Tavalised mängud salvestavad
tavalisi `.sav`-faile, mida ironwail ja vkQuake suudavad laadida;
kohandatud mängud salvestavad `.qsa`-faile, mida laadib ainult see port
(samuti jagatud ekraani ja koostöömängud, mida teised pordid salvestada ei
saa). Kohandatud mängu ajal muudab Peamenüü → Kohanda selle modifikaatoreid
ja lisasid igal ajal; tavalise mängu reeglid jäävad fikseerituks.
Modifikaatorid töötavad iga modiga ja lülituvad deathmatch'is välja.

- Raskem: rohkem saadud kahju, tugevamad vaenlased, vähem laskemoona ja
  tervist korjamiseks, 2–4× koletisi, püstolistart igal tasemel, käsitsi
  salvestuseta.
- Lihtsam: vähem saadud kahju, nõrgemad vaenlased, rohkem laskemoona ja
  tervist, lõpmatu laskemoon, tervise taastumine, kukkumiskahjuta,
  uppumiseta, omakahjuta, topelt- või kolmikhüpped.

## Demod

`record <nimi>` ja `stop` lindistavad demo sinu kasutajakausta; `playdemo
<nimi>` esitab selle. Kui lindistamine lõpeb, kirjutatakse demo taustal
ümber, et ka ironwail ja vkQuake saaksid seda esitada. Kui mod kasutab
osakeste efekte, mida teised pordid näidata ei suuda, säilitatakse täisversioon
failina `<nimi>.qsd` (`playdemo <nimi>.qsd`). `demo_convert <nimi>` teisendab
vanema demo.

## Juurdepääsetavus

Options → Juurdepääsetavus:

- Vähendatud liikumine ja ekraanivälkude tugevus.
- Värvipimeduse filtrid.
- Seadistatav sihik: kuju, suurus, kontuur ja värvimuutus sihtmärgil või
  tapmisel; valikulised tabamusmärgid.
- **Helisubtiitrid** ja teadete seaded (Teated ja subtiitrid), teksti
  skaleerimine.
- Rünnaku ja jooksmise lüliti (hoidmise asemel).
- Mängu kiiruse liugur kogu mängu aeglustamiseks.
- Automaatsihtimine lukustuse näidiku, koonuse ja ulatuse seadetega.

## Täiustused

Options → Täiustused. Kõik toimib kliendi poolel ja on vaikimisi väljas,
nii et see töötab iga modiga ja võrgus.

- **Rusud**: veerevad lihatükid ja purunevatest tünnidest lendavad killud,
  ka modide koletistega; need võivad põrkuda vastu uksi ja lifte.
- **Täiustatud vereplekid**: pritsmed pindadel, kehadega kaasa liikuvad
  haavad, lendavad piisad, mis maandudes laiali pritsivad.
- **Pinnatabamuse efektid**: haavlijäljed, puitu ja vaenlastesse kinni
  jäävad naelad, plahvatuste kõrbejäljed.
- **Täiustatud vesi**: läbipaistvus, lainetus, pritsmed ja kaustika.
- **Läbipaistvad teleportid**, mis näitavad sihtkohta parallaksiga.
- Ümbritsev varjestus, objektipõhine ja kiirusähmastus, pehmed osakesed.
- **Ekraanivarjutaja**: RetroArchi `.slangp`-eelseadistused (CRT ja muud).

Options → Graafika pakub ka tavapärast renderdusskaalat, servasilumist,
ülediskreetimist, tekstuurifiltrit, 8-bitiseid värve ditheringuga, udu,
värvilist valgust ning animatsiooni ja liikumise interpoleerimist.

## Lisad

Lisad on uued relvad ja varustus, mida juhib mootor, nii et need töötavad
iga modi progs'iga. Need on mõeldud ainult üksik- ja koostöömänguks.

### Mängu alustamine lisadega

Mängus on lisad ainult siis, kui see alustatakse nendega:

1. Single Player → märgi kast **Lisad**, seejärel New Game.
2. Vali, milliseid lisasid kaasa võtta, ja kohanda nende seadeid, seejärel
   alusta uut mängu.

Lisadega mängud salvestatakse `.sav`-failide asemel `.qsa`-failidena, nii et
teised pordid ei püüa neid laadida. Salvestuskaartidel on märk „ADD-ONS“.
Tavalise salvestuse laadimine lülitab lisad välja. Lisadega mängu ajal
lülitab lisade leht (konsoolis: `menu_options addons`) üksikuid lisasid
sisse ja välja; igaühel on korduv eelvaade, juhised, klahvimäärangud ja oma
seaded.

### Relvad

Lisad lähevad tavalistesse relvapesadesse (vali iga lisa jaoks pesa või
pesad 9 ja 0) ning ilmuvad relvarattale ja HUD-i. Tuli ja alt-tuli on sinu
tavalised rünnakunupud.

| Lisa | Tuli | Alt-tuli | Laskemoon |
|---|---|---|---|
| **Portaalipüss** | Paigutab sinise portaali | Paigutab oranži portaali | Puudub |
| **Railgun** | Läbistav kuul, mis tabab kõike ühel joonel | — | 2 elementi |
| **Haarats** | Haakub puidu, metalli, kivi või vaenlaste külge ja tõmbab sind kohale | — | Puudub |
| **Kleeppommiheitja** | Hoia laadimiseks, vabasta tulistamiseks | Lõhka | 1 rakett |
| **Bumerang** | Hoia vaenlaste märkimiseks, vabasta viskamiseks; uimastab neid kordamööda | Kustuta märgid | Puudub |
| **Geelipihusti** | Sinine tõukegeel (põrkav) | Oranž kiirendusgeel (kiire) | Puudub |
| **Survepesur** | Pikk veejuga, mis lükkab vaenlasi ja peseb geeli maha | Lai joalehvik | Puudub |

Portaalid paigutatakse tasastele maailma pindadele ning need kannavad sind,
koletisi ja visatud objekte läbi koos nende hooga. Kui **Vaata läbi
portaalide** on sees, näed teist poolt otse, sealhulgas portaalide kaudu
nähtavaid portaale; kvaliteet, elavate vaadete arv, kaugus ja rekursiooni
sügavus on reguleeritavad.

### Varustus

Varustust lülitatakse sisse ja välja, mitte ei hoita käes. Ava
**varustusratas** (`+equipwheel`) või kasuta `goggles cycle`, et läbi käia:

- **Öönägemine**
- **Termonägemine**

## Mitmikmäng

Lisaks LAN- ja internetipõhisele NetQuake'i mängule (protokollid 15, 666 ja
999) ühendub port **QuakeWorldi** serveritega. Multiplayer → Join Game
võtab aadressi või otsib LAN-ist. FTE skriptitud osakesed on toetatud
modidele, mis neid kasutavad.

## Käsurida ja konsool

Levinud valikud: `-basedir <kaust>`, `-game <kaust>`, `-hipnotic` / `-rogue` /
`-quoth`, `-width <l>`, `-height <k>`, `-fullscreen`, `-window` ja
`+käsk argumendid`, et käivitusel konsoolikäske täita. Konsool (`~`)
aktsepteerib tavalisi Quake'i käske ja cvar'e.

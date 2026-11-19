# Naudotojo vadovas

Šiame vadove aprašoma, kaip žaisti Quake su šiuo portu, daugiausia dėmesio
skiriant tam, ką jis prideda, palyginti su originaliu Quake ir kitais
portais. Visos naujovės neprivalomos: su numatytaisiais nustatymais
žaidimas atrodo ir žaidžiamas kaip Quake.

## Pradžia

1. Įsigykite savo Quake kopiją. Nurodykite portui aplanką, kuriame yra
   `id1/` (ir šalia esančios modifikacijos), su `QUAKE_BASEDIR` arba
   `-basedir <aplankas>`.
2. Paleiskite `cargo run --release` arba iškart pradėkite modifikaciją su
   `-game <aplankas> +map <žemėlapis>`.
3. Konfigūracija, išsaugojimai ir ekrano nuotraukos laikomi
   `~/.local/share/quake-source-port/<žaidimas>/`, po aplanką kiekvienai
   modifikacijai.

Pagrindiniame meniu yra Single Player, Multiplayer, Options, **Help** (šis
vadovas), Quit ir **Žemėlapių paketai**, o fone skamba Quake tema
(Options → Garsas → Pagrindinio meniu muzika). Kiekvienas meniu veikia su
pele, klaviatūra, valdikliu ir jutikliniu ekranu; užveskite žymeklį ant
parinkties arba ją pasirinkite, ir pamatysite ją paaiškinantį patarimą.

Pagalbos ekrane aukštyn/žemyn renkasi skyrių; Page Up/Page Down,
kairėn/dešinėn, petiniai mygtukai arba pelės ratukas virš teksto jį slenka.

## Kalba

Options → Žaidimas → **Kalba** parenka meniu, ekrano pranešimų ir šio
vadovo kalbą: anglų, prancūzų, vokiečių, italų, ispanų, rusų, lenkų,
ukrainiečių, čekų, slovakų, kroatų, bulgarų, švedų, suomių, danų, norvegų,
nyderlandų, graikų, vengrų, rumunų, portugalų (Portugalijos arba
Brazilijos), japonų, korėjiečių, kinų (supaprastinta arba tradicinė),
turkų, indoneziečių, malajų, filipiniečių, vietnamiečių, serbų, lietuvių,
latvių, estų, maltiečių, albanų, armėnų, gruzinų, islandų, airių, valų,
baskų, katalonų, galisų, lotynų, esperanto, baltarusių, makedonų, slovėnų,
suahilių, arabų, hebrajų, persų, urdu, hindi, maratų, bengalų, pandžabų,
gudžaratų, tamilų, telugų, tajų, khmerų, amharų arba kantoniečių.
**Sistemos** naudoja jūsų kompiuterio kalbą. Paties 2021 m. pakartotinio
leidimo žaidimo tekstai išversti tik į prancūzų, vokiečių, italų, ispanų ir
rusų kalbas; kitomis kalbomis jie rodomi angliškai. Ne anglų kalba smulkus
žaidimo tekstas rodomas meniu šriftu, nes klasikiniame Quake šrifte yra tik
lotyniškos raidės be diakritikų. Konsolės pranešimai lieka angliški, nebent
įjungta **Versti konsolę**.

## Žemėlapių paketai (modifikacijų naršyklė)

Pagrindinis meniu → Žemėlapių paketai išvardija kiekvieną įdiegtą
modifikaciją ir žemėlapių paketą su aprašymu. Pasirinkite vieną, kad į jį
persijungtumėte, tada naršykite jo žemėlapius ir bet kurį iš jų pradėkite
tiesiogiai. Naršyklė įsimena geriausius kiekvieno žemėlapio nužudymų,
slaptaviečių ir laiko rezultatus. Kiekviena modifikacija turi savo
konfigūraciją, todėl vienai modifikacijai suderinti priskyrimai ir
nustatymai nepersikelia į kitą.

**Kur dėti žemėlapių paketus.** Portas jų ieško Quake pagrindiniame
aplanke (tame, kuriame yra `id1/`, nustatomame su `QUAKE_BASEDIR` arba
`-basedir`). Išvardijamas kiekvienas ten esantis poaplankis su žaidimo
duomenimis (`.pak` ar `.pk3` failu, `progs.dat` arba `maps/` aplanku),
todėl žemėlapių paketą įdiekite išpakuodami jį į atskirą aplanką šalia
`id1/`, pavyzdžiui, `<basedir>/ad/` arba `<basedir>/my_maps/maps/*.bsp`.
Pasirinktinai pridėkite `modinfo.txt` su eilutėmis `title`, `base` ir
`description`, kad nustatytumėte, kaip jis rodomas sąraše. Paketai randami
paleidžiant žaidimą, todėl pridėję naują žaidimą paleiskite iš naujo.
Kiekvieno paketo išsaugojimai, ekrano nuotraukos ir konfigūracija laikomi
`~/.local/share/quake-source-port/<aplankas>/`, o ne paketo aplanke.

## Padalytas ekranas

Iki šešių žaidėjų viename kompiuteryje, kooperatiniame režime arba
deathmatch.

- Multiplayer → New Game → **Vietiniai žaidėjai** nustato, kiek žmonių
  žaidžia.
- Kiekvienas žaidėjas turi savo vaizdą, HUD, garsą ir įvesties įrenginį
  (klaviatūra ir pelė 1 žaidėjui, po valdiklį kitiems).
- Ekranas gali būti padalytas vienas virš kito, greta, 2×2 arba 3×2
  (`r_splitscreen_layout`).
- Kiekvienas žaidėjas per Multiplayer → Setup pasirenka vardą ir marškinių
  bei kelnių spalvas.

## Valdikliai

Portas sukurtas žaisti su pulteliu.

- **Ginklų ratas** (`+weaponwheel`): laikykite, kad sulėtintumėte laiką ir
  bet kuria svirtele ar pele pasirinktumėte ginklą; atleiskite, kad jį
  pakeistumėte.
- **Taikymo pagalba** (Options → Įvestis → Valdiklis): sulėtinimas prie
  priešų ir sukimas, sekantis juos. Ji veikia tik svirtelę, niekada pelę.
- **Taikymas giroskopu** (Options → Įvestis → Valdiklis → Giroskopas):
  jungiklis, atskiras horizontalios, vertikalios ir posvyrio ašių jautrumas
  (posvyris pagal numatymą išjungtas ir suka vaizdą kaip horizontali ašis,
  neigiamos vertės jį apverčia), triukšmo filtras ir kada giroskopas veikia
  (visada, išjungta arba tik laikant `+gyroaction`). **Taikymo stilius**
  perjungia dešiniąją svirtelę tarp 2D taikymo (ji tolygiai suka vaizdą) ir
  **flick stick** (pastūmus svirtelę vaizdas akimirksniu nukreipiamas ta
  kryptimi, o sukant svirtelę sukasi toliau); abu gerai derinasi su
  giroskopu tiksliam taikymui.
- **Vibracija**, svirtelės jautrumas ir apvertimas.
- **Mygtukų simboliai** atitinka jūsų pultelį (Xbox, PlayStation, Nintendo)
  ir nustatomi automatiškai.
- Viską perskirkite per Options → Valdymas, taip pat ir valdikliu.
- Jutikliniai valdikliai su reguliuojamu dydžiu, neskaidrumu ir žvilgsnio
  greičiu.

## Išsaugojimas

- **Automatiniai išsaugojimai** keičiasi atskirai nuo rankinių ir greitųjų
  išsaugojimų, todėl niekada neperrašo jūsų pačių išsaugojimų.
  Options → Žaidimas → Automatinio išsaugojimo nustatymai parenka, kiek jų
  laikyti ir kada išsaugoti: pradedant lygį, pagal laikmatį, radus
  slaptavietę ar visas slaptavietes ir nukovus visus priešus.
- **Išsaugojimų kortelės** įkėlimo ir išsaugojimo meniu rodo kiekvieno
  išsaugojimo lygį, sveikatą, šarvus, ginklus, raktus ir
  nužudymus/slaptavietes.
- **Mirus** (Options → Žaidimas → Mirus) vieno žaidėjo režime galima
  pristabdytame meniu paklausti, ką daryti, įkelti naujausią išsaugojimą,
  leisti pasirinkti išsaugojimą arba pradėti lygį iš naujo.

## Žaidimas ir kamera

Options → Žaidimas:

- Matymo laukas 60–130, vaizdo siūbavimas, ginklo atatranka, visada bėgti.
- **Kamera**: pirmasis asmuo, trečiasis asmuo arba per petį (dešinėje ar
  kairėje).
- **Ginklo modelio** padėtis: centre, dešinėje, kairėje arba paslėptas.

## Pritaikymas: žaidimo modifikatoriai ir priedai

Žaidimas yra arba *standartinis*, arba *pritaikytas*; tai pasirenkama jį
pradedant. New Game (taip pat lygio pasirinkimas per Lygio pasirinkimas ar
Žemėlapių paketai) parodo iškylantįjį langą: **Žaisti** pradeda žaidimą,
**Pritaikyti žaidimo patirtį** atveria žaidimo modifikatorius ir priedus.
Kol kas nors pakeista, prie Žaisti rodoma žyma **PRITAIKYTAS**, o
**Išjungti visus pakeitimus** (Del, pultelio X arba spustelėjimas) grąžina
standartines taisykles be priedų. Standartiniai žaidimai išsaugomi kaip
įprasti `.sav` failai, kuriuos gali įkelti ironwail ir vkQuake; pritaikyti
žaidimai išsaugomi kaip `.qsa` failai, kuriuos įkelia tik šis portas (taip
pat ir padalyto ekrano bei kooperatiniai žaidimai, kurių kiti portai
išsaugoti negali). Pritaikyto žaidimo metu Pagrindinis meniu → Pritaikyti
bet kada keičia jo modifikatorius ir priedus; standartinio žaidimo
taisyklės lieka nekeičiamos. Modifikatoriai veikia su bet kuria
modifikacija ir išsijungia deathmatch režime.

- Sunkiau: daugiau gaunamos žalos, stipresni priešai, mažiau šaudmenų ir
  sveikatos radinių, 2–4× monstrų, kiekvienas lygis nuo pistoleto starto,
  be rankinio išsaugojimo.
- Lengviau: mažiau gaunamos žalos, silpnesni priešai, daugiau šaudmenų ir
  sveikatos, begaliniai šaudmenys, sveikatos atsistatymas, be kritimo žalos,
  be skendimo, be žalos sau, dvigubi ar trigubi šuoliai.

## Demo įrašai

`record <pavadinimas>` ir `stop` įrašo demo į jūsų naudotojo aplanką;
`playdemo <pavadinimas>` jį paleidžia. Baigus įrašyti, demo fone
perrašomas, kad jį galėtų paleisti ir ironwail bei vkQuake. Jei
modifikacija naudoja dalelių efektus, kurių kiti portai parodyti negali,
pilna versija išsaugoma kaip `<pavadinimas>.qsd`
(`playdemo <pavadinimas>.qsd`). `demo_convert <pavadinimas>` konvertuoja
senesnį demo.

Pagrindinis meniu → Demo rodo išsaugotus demo ir, kai įjungta Parinktys →
Demo → Automatiškai įrašyti demo, kiekvieno žaisto lygio automatinį demo;
seniausi pakeičiami, kai vietos užpildytos. Rašykite, kad filtruotumėte
sąrašus, o Tab keičia rikiavimą; mygtukai šalia sąrašo paleidžia,
eksportuoja, pervadina, pasilieka arba ištrina pasirinktą demo, o
pervadintas automatinis demo pasiliekamas. Klaidų ataskaitose yra
dabartinio lygio automatinis demo.

Leidžiant demo, Tarpas pristabdo, laikant Kairėn ar Dešinėn sukama atgal ar
pirmyn, Aukštyn ir Žemyn keičia greitį, PgUp ir PgDn peršoka 10 sekundžių, o
Home ir End nueina į pradžią ir į kitą žemėlapį. I ir O pažymi klipą; E jį
ar visą demo eksportuoja į aplanką `videos`. Valdikliai lieka ekrane; H arba
Slėpti juos paslepia, o nurodžius apatinį kraštą pasirodo juosta, kuri juos
grąžina. Spauskite mygtukus, spustelėkite laiko juostą, kad peršoktumėte
ten, arba vilkite per ją, kad pažymėtumėte klipą; Eksportuoti klausia kadrų
dažnio, dydžio, formato ir failo bei ar eksportuoti pažymėtą klipą, ar visą
demo.

Valdiklyje A pristabdo, kryžminis mygtukas suka atgal ir pirmyn bei keičia
greitį, LB ir RB peršoka 10 sekundžių, o lazdelių paspaudimai nukelia į
pradžią ir į kitą žemėlapį. LT ir RT pažymi klipą, B jį išvalo, X
eksportuoja, Y pasilieka automatinį demo, o View paslepia valdiklius.

`capture` pradeda ir sustabdo vaizdo įrašymą žaidžiant. Kadrų dažnis,
formatas ir kokybė yra skiltyje Parinktys → Demo → Vaizdo įrašymas. Vaizdo
įrašams reikia ffmpeg; be jo kadrai išsaugomi kaip PNG paveikslėliai su WAV
failu.

## Prieinamumas

Options → Prieinamumas:

- Sumažintas judesys ir ekrano blyksnių stiprumas.
- Filtrai daltonikams.
- Konfigūruojamas taikiklis: forma, dydis, kontūras ir spalvos keitimas
  taikantis ar nužudžius; pasirinktiniai pataikymo žymekliai.
- **Garsų subtitrai**; pranešimų ir subtitrų padėtis, dydis ir rodymo trukmė
  nustatomi HUD rengyklėje.
- Puolimo ir bėgimo perjungimas (vietoj laikymo).
- Žaidimo greičio slankiklis visam žaidimui sulėtinti.
- Automatinis taikymas su fiksavimo indikatoriumi, kūgio ir nuotolio
  nustatymais.

## Patobulinimai

Options → Patobulinimai. Visi veikia kliento pusėje ir pagal numatymą yra
išjungti, todėl veikia su bet kuria modifikacija ir internete.

- **Nuolaužos**: riedantys kūno gabalai ir sudaužytų statinių skeveldros,
  taip pat su modifikacijų monstrais; jos gali atsitrenkti į duris ir
  liftus.
- **Patobulintas kraujas**: purslai ant paviršių, kūnus sekančios žaizdos,
  ore skriejantys lašai, kurie ištykšta ten, kur nukrenta.
- **Smūgių efektai**: šratų pėdsakai, į medį ir priešus įsmingančios vinys,
  sprogimų nudegimai.
- **Patobulintas vanduo**: permatomumas, raibuliavimas, purslai ir
  kaustika.
- **Permatomi teleportai**, rodantys savo tikslą su paralaksu.
- Aplinkos užtamsinimas, objektų ir greičio judesio suliejimas, minkštos
  dalelės.
- **Ekrano šešėliuokliai**: RetroArch `.slangp` šablonai (CRT ir kiti).

Options → Grafika taip pat turi įprastus nustatymus: atvaizdavimo mastelį,
kraštų glodinimą, supersampling, tekstūrų filtravimą, 8 bitų spalvas su
ditheringu, rūką, spalvotą apšvietimą ir animacijos bei judėjimo
interpoliavimą.

## Priedai

Priedai yra nauji ginklai ir įranga, valdomi variklio, todėl jie veikia su
bet kurios modifikacijos progs. Jie skirti tik vieno žaidėjo ir
kooperatiniam režimui.

### Žaidimo su priedais pradžia

Žaidimas turi priedų tik tada, jei su jais pradedamas:

1. Single Player → pažymėkite langelį **Priedai**, tada New Game.
2. Pasirinkite, kuriuos priedus pasiimti, ir sureguliuokite jų nustatymus,
   tada pradėkite naują žaidimą.

Žaidimai su priedais išsaugomi kaip `.qsa` failai vietoj `.sav`, kad kiti
portai nebandytų jų įkelti. Išsaugojimų kortelės rodo ženklelį „ADD-ONS“.
Įkėlus įprastą išsaugojimą, priedai išjungiami. Žaidžiant su priedais,
puslapis Priedai (konsolė: `menu_options addons`) įjungia ir išjungia
atskirus priedus; kiekvienas turi kartojamą peržiūrą, instrukcijas, savo
priskyrimus ir nustatymus.

### Ginklai

Priedai užima įprastas ginklų vietas (kiekvienam priedui pasirenkama, arba
9 ir 0 vieta) ir rodomi ginklų rate bei HUD. Šūvis ir alternatyvus šūvis
yra jūsų įprasti puolimo mygtukai.

| Priedas | Šūvis | Alternatyvus šūvis | Šaudmenys |
|---|---|---|---|
| **Portalų ginklas** | Padeda mėlyną portalą | Padeda oranžinį portalą | Nėra |
| **Railgun** | Pramušantis šovinys, pataikantis į viską vienoje linijoje | — | 2 elementai |
| **Kablys** | Užsikabina už medžio, metalo, akmens ar priešų ir pritraukia jus | — | Nėra |
| **Lipnių bombų svaidyklė** | Laikykite užtaisyti, atleiskite paleisti | Susprogdinti | 1 raketa |
| **Bumerangas** | Laikykite pažymėti priešus, atleiskite mesti; paeiliui apsvaigina kiekvieną | Išvalyti žymes | Nėra |
| **Gelio purkštuvas** | Mėlynas atstūmimo gelis (atšokti) | Oranžinis pagreičio gelis (greitis) | Nėra |
| **Plovimo aparatas** | Ilga vandens srovė, atstumianti priešus ir nuplaunanti gelį | Plati srovių vėduoklė | Nėra |

Portalai dedami ant plokščių pasaulio paviršių ir perneša jus, monstrus bei
mestus daiktus išlaikydami jų inerciją. Įjungus **Matyti pro portalus**,
kitą pusę matote gyvai, įskaitant portalus, matomus pro portalus; kokybę,
gyvų vaizdų skaičių, atstumą ir rekursijos gylį galima reguliuoti.

### Įranga

Įranga įjungiama ir išjungiama, o ne laikoma rankose. Atverkite **įrangos
ratą** (`+equipwheel`) arba naudokite `goggles cycle`, kad perjungtumėte:

- **Naktinis matymas**
- **Terminis matymas**

## Tinklo žaidimas

Be NetQuake žaidimo per LAN ir internetą (protokolai 15, 666 ir 999),
portas jungiasi prie **QuakeWorld** serverių. Multiplayer → Join Game
priima adresą arba ieško LAN tinkle. FTE scenarijų dalelės palaikomos
modifikacijoms, kurios jas naudoja.

## Komandinė eilutė ir konsolė

Dažnos parinktys: `-basedir <aplankas>`, `-game <aplankas>`, `-hipnotic` /
`-rogue` / `-quoth`, `-width <w>`, `-height <h>`, `-fullscreen`, `-window`
ir `+command args` konsolės komandoms vykdyti paleidžiant. Konsolė (`~`)
priima įprastas Quake komandas ir kintamuosius.

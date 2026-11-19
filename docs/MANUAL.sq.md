# Manuali i përdoruesit

Ky manual shpjegon si të luash Quake me këtë port, me theks te ajo që ai
shton mbi Quake origjinal dhe portet e tjera. Çdo gjë e re është opsionale:
me cilësimet e parazgjedhura loja duket dhe luhet si Quake.

## Fillimi

1. Siguro kopjen tënde të Quake. Tregoji portit dosjen që përmban `id1/`
   (dhe modet pranë saj), me `QUAKE_BASEDIR` ose `-basedir <dir>`.
2. Ekzekuto `cargo run --release`, ose nis menjëherë një mod me
   `-game <dir> +map <map>`.
3. Konfigurimi, ruajtjet dhe pamjet e ekranit ndodhen në
   `~/.local/share/quake-source-port/<game>/`, një dosje për çdo mod.

Menyja kryesore ka Single Player, Multiplayer, Options, **Help** (ky
manual), Quit dhe **Paketat e hartave**, me temën e Quake që luan në sfond
(Options → Audio → Muzika e menysë kryesore). Çdo meny funksionon me miun,
tastierën, kontrolluesin dhe prekjen; kalo mbi një opsion ose zgjidhe për të
parë një këshillë që e shpjegon.

Te Ndihma, lart/poshtë zgjedh një seksion; Page Up/Page Down, majtas/djathtas,
butonat e supeve ose rrota e miut mbi tekst e lëvizin atë.

## Gjuha

Options → Loja → **Gjuha** zgjedh gjuhën e menyve, mesazheve në ekran dhe
të këtij manuali: anglisht, frëngjisht, gjermanisht, italisht, spanjisht,
rusisht, polonisht, ukrainisht, çekisht, sllovakisht, kroatisht, bullgarisht,
suedisht, finlandisht, danisht, norvegjisht, holandisht, greqisht,
hungarisht, rumanisht, portugalisht (Portugali ose Brazil), japonisht,
koreanisht, kinezisht (e thjeshtuar ose tradicionale), turqisht,
indonezisht, malajisht, filipinisht, vietnamisht, serbisht, lituanisht,
letonisht, estonisht, maltisht, shqip, armenisht, gjeorgjisht, islandisht,
irlandisht, uellsisht, baskisht, katalanisht, galicisht, latinisht,
esperanto, bjellorusisht, maqedonisht, sllovenisht, suahili, arabisht,
hebraisht, persisht, urdu, hindisht, marathi, bengalisht, panxhabisht,
guxharatisht, tamilisht, telugu, tajlandisht, kmerisht, amharisht ose
kantonisht.
**Sistemi** ndjek gjuhën e kompjuterit tënd. Teksti i lojës
i ribotimit të 2021 është përkthyer vetëm në frëngjisht, gjermanisht,
italisht, spanjisht dhe rusisht; gjuhët e tjera e shfaqin në anglisht. Në
gjuhë të tjera nga anglishtja, teksti i vogël në lojë përdor fontin e
menysë, sepse fonti klasik i Quake ka vetëm shkronja latine pa shenja
diakritike. Mesazhet e konsolës mbeten në anglisht, përveç kur **Përkthe
konsolën** është aktiv.

## Paketat e hartave (shfletuesi i modeve)

Menyja kryesore → Paketat e hartave liston çdo mod dhe paketë hartash të
instaluar me përshkrimin e saj. Zgjidh një për të kaluar te ajo, pastaj
shfleto hartat e saj dhe nis cilëndo drejtpërdrejt. Shfletuesi mban mend
vrasjet, sekretet dhe kohën tënde më të mirë për çdo hartë. Çdo mod ka
konfigurimin e vet, ndaj caktimet dhe cilësimet e rregulluara për një mod
nuk kalojnë te një tjetër.

**Ku vendosen paketat e hartave.** Porti i kërkon në dosjen bazë të Quake
(ajo që mban `id1/`, e caktuar me `QUAKE_BASEDIR` ose `-basedir`). Çdo
nëndosje aty që përmban të dhëna loje (një skedar `.pak` ose `.pk3`, një
`progs.dat` ose një dosje `maps/`) listohet, ndaj instalo një paketë
hartash duke e shpaketuar në dosjen e vet pranë `id1/`, për shembull
`<basedir>/ad/` ose `<basedir>/my_maps/maps/*.bsp`. Opsionalisht shto një
`modinfo.txt` me rreshtat `title`, `base` dhe `description` për të
përcaktuar si listohet. Paketat gjenden kur niset loja, ndaj rinise pasi
shton një. Ruajtjet, pamjet e ekranit dhe konfigurimi i çdo pakete ndodhen
në `~/.local/share/quake-source-port/<folder>/`, jo në dosjen e paketës.

## Ekran i ndarë

Deri në gjashtë lojtarë në një kompjuter, në bashkëpunim ose deathmatch.

- Multiplayer → New Game → **Lojtarë lokalë** cakton sa vetë luajnë.
- Çdo lojtar ka pamjen, HUD-in, tingullin dhe pajisjen e vet të hyrjes
  (tastierë dhe mi për lojtarin 1, nga një kontrollues për të tjerët).
- Paraqitja e ekranit mund të jetë njëra mbi tjetrën, krah për krah, 2×2
  ose 3×2 (`r_splitscreen_layout`).
- Çdo lojtar zgjedh një emër dhe ngjyrat e bluzës/pantallonave te
  Multiplayer → Setup.

## Kontrolluesit

Porti është projektuar për t'u luajtur me kontrollues.

- **Rrota e armëve** (`+weaponwheel`): mbaje për të ngadalësuar kohën dhe
  zgjidh një armë me cilëndo levë ose me miun; lësho për ta ndërruar.
- **Ndihma në shënjim** (Options → Hyrja → Kontrolluesi): ngadalësim pranë
  armiqve dhe rrotullim që i ndjek. Zbatohet vetëm te leva, kurrë te miu.
- **Shënjim me xhiro** (Options → Hyrja → Kontrolluesi → Xhiro): një
  çelës, plus ndjeshmëri të veçanta për boshtin horizontal, vertikal dhe
  të anësimit (anësimi është joaktiv si parazgjedhje dhe e kthen pamjen si
  boshti horizontal, vlerat negative e përmbysin), një filtër zhurme dhe kur
  zbatohet xhiroja (gjithmonë, joaktiv ose vetëm kur mbahet `+gyroaction`).
  **Stili i shënjimit** e kalon levën e djathtë mes shënjimit 2D (e kthen
  pamjen vazhdimisht) dhe **flick stick** (shtyje levën për ta drejtuar
  menjëherë pamjen në atë drejtim, pastaj rrotulloje levën për ta kthyer);
  të dyja kombinohen mirë me xhiron për shënjim të saktë.
- **Dridhja**, ndjeshmëria dhe përmbysja e levave.
- **Simbolet e butonave** i përshtaten kontrolluesit tënd (Xbox,
  PlayStation, Nintendo) dhe zbulohen automatikisht.
- Ricakto gjithçka te Options → Kontrollet, edhe nga kontrolluesi.
- Kontrolle me prekje me madhësi, opacitet dhe shpejtësi shikimi të
  rregullueshme.

## Ruajtja

- **Autoruajtjet** rrotullohen veçmas nga ruajtjet manuale dhe ruajtjet e
  shpejta, ndaj nuk i mbishkruajnë kurrë tuajat. Options → Loja →
  Cilësimet e autoruajtjes zgjedh sa të mbahen dhe kur të ruhet: në hyrje
  të nivelit, me interval, kur gjendet një sekret ose të gjitha sekretet
  dhe kur vriten të gjithë armiqtë.
- **Kartat e ruajtjeve** në menytë Ngarko/Ruaj tregojnë nivelin, shëndetin,
  parzmoren, armët, çelësat dhe vrasjet/sekretet e çdo ruajtjeje.
- **Në vdekje** (Options → Loja → Në vdekje), loja me një lojtar mund të
  pyesë çfarë të bëhet në një meny të pezulluar, të ngarkojë ruajtjen më të
  fundit, të të lërë të zgjedhësh një ruajtje ose të rinisë nivelin.

## Loja dhe kamera

Options → Loja:

- Fusha e pamjes 60–130, lëkundja e pamjes, zmbrapsja e armës, vrapo
  gjithmonë.
- **Kamera**: vetë i pari, vetë i treti ose mbi sup (djathtas ose majtas).
- Pozicioni i **modelit të armës**: qendër, djathtas, majtas ose i fshehur.

## Përshtat: modifikuesit e lojës dhe shtesat

Një lojë është ose *standarde* ose *e përshtatur*, e zgjedhur kur niset. New
Game (dhe zgjedhja e një niveli te Zgjedhja e nivelit ose Paketat e
hartave) shfaq një dritare: **Luaj** nis lojën, **Përshtat përvojën** hap
Modifikuesit e lojës dhe Shtesat. Sa kohë që diçka është ndryshuar, Luaj
mban etiketën **E PËRSHTATUR** dhe **Çaktivizo të gjitha ndryshimet** (Del,
X në kontrollues ose klik) rikthen rregullat standarde pa shtesa. Lojërat
standarde ruajnë skedarë të zakonshëm `.sav` që ironwail dhe vkQuake mund
t'i ngarkojnë; lojërat e përshtatura ruajnë skedarë `.qsa` që i ngarkon
vetëm ky port (njësoj si lojërat me ekran të ndarë dhe bashkëpunim, që
portet e tjera nuk mund t'i ruajnë). Gjatë një loje të përshtatur, Menyja
kryesore → Përshtat ndryshon modifikuesit dhe shtesat e saj në çdo kohë;
rregullat e një loje standarde mbeten të fiksuara. Modifikuesit
funksionojnë me çdo mod dhe çaktivizohen në deathmatch.

- Më e vështirë: më shumë dëm i marrë, armiq më të fortë, më pak municion
  dhe shëndet nga sendet, 2–4× përbindësha, fillim me pistoletë në çdo
  nivel, pa ruajtje manuale.
- Më e lehtë: më pak dëm i marrë, armiq më të dobët, më shumë municion dhe
  shëndet, municion i pafund, rigjenerim i shëndetit, pa dëm nga rëniet, pa
  mbytje, pa vetëdëmtim, kërcime të dyfishta ose të trefishta.

## Demot

`record <name>` dhe `stop` regjistrojnë një demo në dosjen tënde të
përdoruesit; `playdemo <name>` e luan atë. Kur ndalon regjistrimi, demoja
rishkruhet në sfond që ta luajnë edhe ironwail dhe vkQuake. Nëse një mod
përdor efekte grimcash që portet e tjera nuk mund t'i shfaqin, versioni i
plotë ruhet si `<name>.qsd` (`playdemo <name>.qsd`). `demo_convert <name>`
konverton një demo më të vjetër.

Menyja kryesore → Demot shfaq demot e tua të ruajtura dhe, kur Opsionet →
Demot → Regjistro demot automatikisht është ndezur, një demo automatike për
çdo nivel që luan; më të vjetrat zëvendësohen kur vendet janë plot.
Shkruani për të filtruar listat dhe shtypni Tab për të ndryshuar renditjen;
butonat pranë listës luajnë, eksportojnë, riemërtojnë, mbajnë ose fshijnë
demon e zgjedhur, dhe riemërtimi i një demoje automatike e mban atë.
Raportet e gabimeve përfshijnë demon automatike të nivelit aktual.

Gjatë një demoje, Hapësira bën pauzë, mbajtja e Majtas ose Djathtas kthen
mbrapsht ose shpejton përpara, Lart dhe Poshtë ndryshojnë shpejtësinë, PgUp
dhe PgDn kapërcejnë 10 sekonda, kurse Home dhe End shkojnë në fillim dhe te
harta tjetër. I dhe O shënojnë një klip; E e eksporton atë, ose të gjithë
demon, në dosjen `videos`. Kontrollet mbeten në ekran; H ose Fshih i fsheh,
dhe kur drejton te buza e poshtme shfaqet një shirit që i rikthen. Kliko
butonat, kliko vijën kohore për të kërcyer atje, ose tërhiq mbi të për të
theksuar një klip; Eksporto pyet për shpejtësinë e kuadrove, madhësinë,
formatin dhe skedarin, dhe nëse të eksportojë klipin e theksuar apo tërë
demon.

Në kontrollues, A bën pauzë, d-pad-i kthen pas, çon përpara dhe ndryshon
shpejtësinë, LB dhe RB kërcejnë 10 sekonda, dhe shtypja e levave shkon në
fillim dhe në hartën tjetër. LT dhe RT shënojnë një klip, B e pastron, X
eksporton, Y mban një demo automatike dhe View fsheh kontrollet.

`capture` nis dhe ndalon regjistrimin e një videoje ndërsa luan. Shpejtësia
e kuadrove, formati dhe cilësia janë te Opsionet → Demot → Regjistrimi i
videos. Videot kërkojnë ffmpeg; pa të kuadrot ruhen si imazhe PNG me një
skedar WAV.

## Aksesueshmëria

Options → Aksesueshmëria:

- Lëvizje të pakësuara dhe intensiteti i ndezjeve të ekranit.
- Filtra për daltonizmin.
- Shënjestër e konfigurueshme: forma, madhësia, kontura dhe ndryshimi i
  ngjyrës mbi shenjë ose kur vret; shenja goditjeje opsionale.
- **Titrat e tingujve** dhe cilësimet e mesazheve (Mesazhet & titrat),
  shkallëzimi i tekstit.
- Sulm dhe vrap me ndërrim (në vend të mbajtjes).
- Një rrëshqitës i shpejtësisë së lojës për ta ngadalësuar gjithë lojën.
- Shënjim automatik me tregues kapjeje, cilësime për konin dhe rrezen.

## Përmirësimet

Options → Përmirësimet. Të gjitha funksionojnë në anën e klientit dhe janë
joaktive si parazgjedhje, ndaj punojnë me çdo mod dhe në internet.

- **Rrënojat**: copa mishi që rrokullisen dhe ashkla nga fuçitë e
  copëtuara, edhe me përbindëshat e modeve; ato mund të përplasen me dyert
  dhe ashensorët.
- **Gjak i përmirësuar**: spërkatje në sipërfaqe, plagë që ndjekin trupat,
  pika në ajër që shpërndahen aty ku bien.
- **Efekte goditjeje**: gjurmë saçmash, gozhdë që ngecin në dru dhe armiq,
  gjurmë djegieje nga shpërthimet.
- **Ujë i përmirësuar**: tejdukshmëri, valëzime, spërkatje dhe kaustikë.
- **Teleportues të tejdukshëm** që tregojnë destinacionin me paralaksë.
- Okluzion ambiental, turbullim lëvizjeje për çdo objekt dhe nga
  shpejtësia, grimca të buta.
- **Shaderët e ekranit**: paracaktime RetroArch `.slangp` (CRT dhe të
  tjera).

Options → Grafika ka gjithashtu shkallën e zakonshme të renderimit, zbutjen
e skajeve, supersampling, filtrimin e teksturave, ngjyrat 8-bit me
dithering, mjegullën, ndriçimin me ngjyra dhe interpolimin e
animacionit/lëvizjes.

## Shtesat

Shtesat janë armë dhe pajisje të reja, të drejtuara nga motori, ndaj
funksionojnë me progs të çdo modi. Ato janë vetëm për një lojtar dhe
bashkëpunim.

### Nisja e një loje me shtesa

Një lojë ka shtesa vetëm nëse niset me to:

1. Single Player → shëno kutinë **Shtesat**, pastaj New Game.
2. Zgjidh cilat shtesa të marrësh me vete dhe rregullo cilësimet e tyre,
   pastaj nis lojën e re.

Lojërat me shtesa ruhen si skedarë `.qsa` në vend të `.sav`, që portet e
tjera të mos përpiqen t'i ngarkojnë. Kartat e ruajtjeve shfaqin një shenjë
"ADD-ONS". Ngarkimi i një ruajtjeje të zakonshme i çaktivizon shtesat.
Gjatë një loje me shtesa, faqja Shtesat (konsola: `menu_options addons`)
aktivizon dhe çaktivizon shtesat një nga një; secila ka një parapamje në
cikël, udhëzime, caktimet e saj dhe cilësimet e veta.

### Armët

Shtesat vendosen në vendet e zakonshme të armëve (zgjidh vendin për çdo
shtesë, ose vendet 9 dhe 0) dhe shfaqen në rrotën e armëve dhe në HUD.
Qitja dhe zjarri alternativ janë butonat e tu të zakonshëm të sulmit.

| Shtesa | Qit | Zjarr alternativ | Municioni |
|---|---|---|---|
| **Armë portalesh** | Vendos portalin blu | Vendos portalin portokalli | Asnjë |
| **Railgun** | Predhë shpuese që godet gjithçka në një vijë | — | 2 qeliza |
| **Grremç** | Kapet në dru, metal, gur ose armiq dhe të tërheq drejt tyre | — | Asnjë |
| **Hedhës bombash ngjitëse** | Mbaj për ta ngarkuar, lësho për ta hedhur | Shpërthe | 1 raketë |
| **Bumerang** | Mbaj për të shënuar armiqtë, lësho për të hedhur; i shtang me radhë | Pastro shenjat | Asnjë |
| **Spërkatës xheli** | Xhel blu zmbrapsës (kërcyes) | Xhel portokalli shtytës (i shpejtë) | Asnjë |
| **Pastrues me presion** | Rrymë e gjatë uji që shtyn armiqtë dhe lan xhelin | Ventilator i gjerë rrymash | Asnjë |

Portalet vendosen në sipërfaqe të sheshta të botës dhe të kalojnë ty,
përbindëshat dhe objektet e hedhura me gjithë vrullin e tyre. Me **Shiko
përmes portaleve** aktiv sheh anën tjetër drejtpërdrejt, përfshirë portalet
e dukshme përmes portaleve; cilësia, numri i pamjeve të gjalla, distanca
dhe thellësia e rekursionit janë të rregullueshme.

### Pajisjet

Pajisjet aktivizohen dhe çaktivizohen në vend që të mbahen në dorë. Hap
**rrotën e pajisjeve** (`+equipwheel`) ose përdor `goggles cycle` për të
kaluar nëpër to:

- **Shikim nate**
- **Shikim termik**

## Shumë lojtarë

Përveç lojës NetQuake në LAN dhe internet (protokollet 15, 666 dhe 999),
porti lidhet me serverë **QuakeWorld**. Multiplayer → Join Game pranon një
adresë ose kërkon në LAN. Grimcat e skriptuara FTE mbështeten për modet që
i përdorin.

## Linja e komandës dhe konsola

Opsione të zakonshme: `-basedir <dir>`, `-game <dir>`, `-hipnotic` /
`-rogue` / `-quoth`, `-width <w>`, `-height <h>`, `-fullscreen`, `-window`
dhe `+command args` për të ekzekutuar komanda konsole në nisje. Konsola
(`~`) pranon komandat dhe cvar-ët e zakonshëm të Quake.

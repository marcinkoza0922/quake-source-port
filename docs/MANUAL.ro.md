# Manual de utilizare

Acest manual descrie cum se joacă Quake cu acest port, cu accent pe ce
adaugă față de Quake original și față de alte porturi. Tot ce e nou este
opțional: cu setările implicite, jocul arată și se joacă exact ca Quake.

## Primii pași

1. Procură-ți propria copie de Quake. Indică portului dosarul care
   conține `id1/` (și modurile de lângă el), cu `QUAKE_BASEDIR` sau
   `-basedir <dosar>`.
2. Rulează `cargo run --release` sau pornește direct un mod cu
   `-game <dosar> +map <hartă>`.
3. Configurația, salvările și capturile de ecran se află în
   `~/.local/share/quake-source-port/<joc>/`, câte un dosar pentru fiecare mod.

Meniul principal conține Single Player, Multiplayer, Options, **Help**
(acest manual), Quit și **Pachete de hărți**, cu tema Quake pe fundal
(Options → Audio → Muzică meniu principal). Toate meniurile funcționează cu
mouse-ul, tastatura, controlerul și ecranul tactil; treci cu cursorul peste
o opțiune sau selecteaz-o ca să vezi o explicație.

În Help, sus/jos alege o secțiune; Page Up/Page Down, stânga/dreapta,
butoanele de umăr sau rotița mouse-ului peste text îl derulează.

## Limbă

Options → Joc → **Limbă** alege limba meniurilor, a mesajelor de pe ecran și a
acestui manual: engleză, franceză, germană, italiană, spaniolă, rusă, poloneză,
ucraineană, cehă, slovacă, croată, bulgară, suedeză, finlandeză, daneză,
norvegiană, neerlandeză, greacă, maghiară, română, portugheză (Portugalia sau
Brazilia), japoneză, coreeană, chineză (simplificată sau tradițională), turcă,
indoneziană, malaieziană, filipineză, vietnameză, sârbă, lituaniană, letonă,
estonă, malteză, albaneză, armeană, georgiană, islandeză, irlandeză, galeză,
bască, catalană, galiciană, latină, esperanto, belarusă, macedoneană, slovenă,
swahili, arabă, ebraică, persană, urdu, hindi, marathi, bengaleză, punjabi,
gujarati, tamilă, telugu, thailandeză, khmeră, amharică sau cantoneză.
**Sistem** urmează limba calculatorului tău. Textele de joc proprii ale
reeditării din 2021 sunt traduse doar în franceză, germană, italiană,
spaniolă și rusă; celelalte limbi le afișează în engleză. În alte limbi
decât engleza, textul mic din joc folosește fontul meniurilor, deoarece
fontul clasic Quake are doar litere latine fără diacritice. Mesajele
consolei rămân în engleză, cu excepția cazului în care **Tradu consola** e
pornit.

## Pachete de hărți (browser de moduri)

Meniul principal → Pachete de hărți listează fiecare mod și pachet de hărți
instalat, cu descrierea sa. Alege unul ca să treci la el, apoi răsfoiește-i
hărțile și pornește direct oricare dintre ele. Browserul reține cele mai
bune rezultate la eliminări, secrete și timp pentru fiecare hartă. Fiecare
mod își păstrează propria configurație, astfel că tastele și setările
reglate pentru un mod nu ajung în altul.

**Unde se pun pachetele de hărți.** Portul le caută în dosarul de bază al
lui Quake (cel care conține `id1/`, setat cu `QUAKE_BASEDIR` sau
`-basedir`). Fiecare subdosar de acolo care conține date de joc (un fișier
`.pak` sau `.pk3`, un `progs.dat` sau un dosar `maps/`) apare în listă, așa
că instalezi un pachet dezarhivându-l în propriul dosar lângă `id1/`, de
exemplu `<basedir>/ad/` sau `<basedir>/my_maps/maps/*.bsp`. Opțional, adaugă
un `modinfo.txt` cu liniile `title`, `base` și `description` ca să controlezi
cum apare în listă. Pachetele sunt găsite la pornirea jocului, așa că
repornește-l după ce adaugi unul. Salvările, capturile de ecran și
configurația fiecărui pachet se află în
`~/.local/share/quake-source-port/<dosar>/`, nu în dosarul pachetului.

## Ecran împărțit

Până la șase jucători pe un singur calculator, în cooperativ sau deathmatch.

- Multiplayer → New Game → **Jucători locali** stabilește câte persoane
  joacă.
- Fiecare jucător are propria imagine, propriul HUD, sunet și dispozitiv de
  control (tastatură și mouse pentru jucătorul 1, câte un controler pentru
  ceilalți).
- Aranjarea ecranului poate fi suprapusă, alăturată, 2×2 sau 3×2
  (`r_splitscreen_layout`).
- Fiecare jucător își alege numele și culorile tricoului și pantalonilor în
  Multiplayer → Setup.

## Controlere

Portul este gândit pentru a fi jucat cu un controler.

- **Roata armelor** (`+weaponwheel`): ține apăsat ca să încetinești timpul
  și să alegi o armă cu oricare stick sau cu mouse-ul; eliberează ca să o
  schimbi.
- **Asistență la țintire** (Options → Intrare → Controler): încetinire lângă
  inamici și rotație care îi urmărește. Se aplică doar stick-ului, niciodată
  mouse-ului.
- **Țintire cu giroscop** (Options → Intrare → Controler → Giroscop): un
  comutator, plus sensibilitate separată pentru axele de girație, tangaj și
  ruliu (ruliul e oprit implicit și rotește privirea ca girația, valorile
  negative o inversează), un filtru de zgomot și când se aplică giroscopul
  (mereu, oprit sau doar cât timp e ținut `+gyroaction`). **Stil de
  țintire** comută stick-ul drept între țintire 2D (rotește privirea
  continuu) și **flick stick** (împinge stick-ul ca să îndrepți imediat
  privirea în acea direcție, apoi rotește stick-ul ca s-o rotești); ambele
  se combină bine cu giroscopul pentru țintire fină.
- **Vibrații**, sensibilitatea și inversarea stick-urilor.
- **Simbolurile butoanelor** se potrivesc controlerului tău (Xbox,
  PlayStation, Nintendo) și sunt detectate automat.
- Reatribuie orice în Options → Controale, inclusiv de pe controler.
- Controale tactile cu mărime, opacitate și viteză de privire reglabile.

## Salvare

- **Salvările automate** se rotesc separat de salvările manuale și cele
  rapide, așa că nu le suprascriu niciodată pe ale tale. Options → Joc →
  Setări salvare automată alege câte se păstrează și când se salvează: la
  intrarea în nivel, la interval, la găsirea unui secret sau a tuturor
  secretelor și la eliminarea tuturor inamicilor.
- **Fișele de salvare** din meniurile de încărcare și salvare
  arată nivelul, viața,
  armura, armele, cheile și eliminările/secretele fiecărei salvări.
- **La moarte** (Options → Joc → La moarte), în single player jocul poate
  întreba ce să facă într-un meniu pus pe pauză, poate încărca cea mai
  recentă salvare, te poate lăsa să alegi o salvare sau poate relua nivelul.

## Joc și cameră

Options → Joc:

- Câmp vizual 60–130, balans cameră, recul armă, alergare permanentă.
- **Cameră**: persoana întâi, persoana a treia sau peste umăr (dreapta sau
  stânga).
- Poziția **modelelor armei**: centru, dreapta, stânga sau ascunse.

## Personalizare: modificatori de joc și extensii

Un joc este fie *vanilla*, fie *personalizat*, ales la pornire. New Game (și
alegerea unui nivel în Selectare nivel sau Pachete de hărți) afișează o
fereastră: **Joacă** pornește jocul, **Personalizează experiența** deschide
Modificatori de joc și Extensii. Cât timp ceva e modificat, Joacă poartă
eticheta **PERSONALIZAT**, iar **Dezactivează toate modificările** (Del, X
pe controler sau clic) readuce regulile standard fără extensii. Jocurile
vanilla salvează fișiere `.sav` obișnuite, pe care ironwail și vkQuake le
pot încărca; jocurile personalizate salvează fișiere `.qsa` pe care doar
acest port le încarcă (la fel și jocurile pe ecran împărțit și în
cooperativ, pe care alte porturi nu le pot salva). În timpul unui joc
personalizat, Meniul principal → Personalizare îi schimbă oricând
modificatorii și extensiile; regulile unui joc vanilla rămân fixe.
Modificatorii funcționează cu orice mod și se dezactivează în deathmatch.

- Mai greu: daune primite mai mari, inamici mai rezistenți, mai puțină
  muniție și viață de colectat, monștri x2–x4, start de la zero la fiecare
  nivel, fără salvări manuale.
- Mai ușor: daune primite mai mici, inamici mai slabi, mai multă muniție și
  viață, muniție infinită, regenerare viață, fără daune la cădere, fără
  înec, fără autovătămare, salturi duble sau triple.

## Demonstrații

`record <nume>` și `stop` înregistrează o demonstrație în dosarul tău de
utilizator; `playdemo <nume>` o redă. Când înregistrarea se oprește,
demonstrația este rescrisă în fundal ca s-o poată reda și ironwail și
vkQuake. Dacă un mod folosește efecte de particule pe care alte porturi nu
le pot afișa, versiunea completă se păstrează ca `<nume>.qsd`
(`playdemo <nume>.qsd`). `demo_convert <nume>` convertește o demonstrație
mai veche.

## Accesibilitate

Options → Accesibilitate:

- Mișcare redusă și intensitatea flash-urilor de ecran.
- Filtre pentru daltonism.
- Cătare configurabilă: formă, mărime, contur și schimbarea culorii pe
  țintă sau la eliminare; marcaje de lovire opționale.
- **Subtitrări sunete** și setări pentru mesaje (Mesaje și subtitrări),
  scalarea textului.
- Atac și alergare comutate (în loc de menținute).
- Un glisor pentru viteza jocului, ca să încetinești tot jocul.
- Țintire automată cu indicator de fixare, setări de con și rază.

## Îmbunătățiri

Options → Îmbunătățiri. Toate sunt pe partea de client și oprite implicit,
așa că funcționează cu orice mod și online.

- **Resturi**: bucăți de carne care se rostogolesc și schije din butoaie
  sfărâmate, inclusiv cu monștrii din moduri; pot lovi uși și lifturi.
- **Sânge îmbunătățit**: stropi pe suprafețe, răni care urmează corpurile,
  picături în aer care se împrăștie unde aterizează.
- **Efecte de impact**: urme de alice, cuie care se înfig în lemn și în
  inamici, urme de arsură de la explozii.
- **Apă îmbunătățită**: transparență, unduiri, stropi și caustice.
- **Teleportoare transparente** care își arată destinația cu paralaxă.
- Ocluzie ambientală, motion blur per obiect și de viteză, particule fine.
- **Shadere de ecran**: presetări RetroArch `.slangp` (CRT și altele).

Options → Grafică are și obișnuitele scală de randare, anti-aliasing,
supersampling, filtrare texturi, culori pe 8 biți cu dithering, ceață,
iluminare colorată și interpolarea animațiilor/mișcării.

## Extensii

Extensiile sunt arme și echipamente noi, rulate de motor, astfel că
funcționează cu progs-ul oricărui mod. Sunt doar pentru single player și
cooperativ.

### Pornirea unui joc cu extensii

Un joc are extensii doar dacă pornește cu ele:

1. Single Player → bifează căsuța **Extensii**, apoi New Game.
2. Alege ce extensii iei cu tine și reglează-le setările, apoi pornește
   jocul nou.

Jocurile cu extensii se salvează ca fișiere `.qsa` în loc de `.sav`, ca alte
porturi să nu încerce să le încarce. Fișele de salvare afișează o insignă
„ADD-ONS”. Încărcarea unei salvări normale dezactivează extensiile. Cât timp
joci un joc cu extensii, pagina Extensii (consolă: `menu_options addons`)
pornește și oprește extensiile individual; fiecare are o previzualizare în
buclă, instrucțiuni, atribuirile de taste și propriile setări.

### Arme

Extensiile ocupă sloturile de armă obișnuite (alegi slotul pentru fiecare
extensie sau sloturile 9 și 0) și apar pe roata armelor și în HUD. Focul și
focul secundar sunt butoanele tale obișnuite de atac.

| Extensie | Foc | Foc secundar | Muniție |
|---|---|---|---|
| **Armă de portaluri** | Plasează portalul albastru | Plasează portalul portocaliu | Niciuna |
| **Railgun** | Proiectil perforant care lovește tot ce e pe linie | — | 2 celule |
| **Cârlig** | Se prinde și te trage spre lemn, metal, piatră sau inamici | — | Niciuna |
| **Lansator de bombe lipicioase** | Ține apăsat ca să încarci, eliberează ca să lansezi | Detonează | 1 rachetă |
| **Bumerang** | Ține apăsat ca să marchezi inamici, eliberează ca să arunci; îi amețește pe rând | Șterge marcajele | Niciuna |
| **Pulverizator de gel** | Gel albastru de respingere (elastic) | Gel portocaliu de propulsie (rapid) | Niciuna |
| **Spălător cu presiune** | Jet lung de apă care împinge inamicii și spală gelul | Evantai larg de jeturi | Niciuna |

Portalurile se plasează pe suprafețele plate ale hărții și te transportă pe
tine, monștrii și obiectele aruncate, păstrându-le impulsul. Cu **Vezi prin
portaluri** pornit vezi partea cealaltă live, inclusiv portalurile vizibile
prin alte portaluri; calitatea, numărul de vederi live, distanța și
adâncimea recursivității sunt reglabile.

### Echipament

Echipamentul se pornește și se oprește, nu se ține în mână. Deschide
**roata de echipament** (`+equipwheel`) sau folosește `goggles cycle` ca să
treci prin:

- **Vedere nocturnă**
- **Vedere termică**

## Multiplayer

Pe lângă jocul NetQuake în LAN și pe internet (protocoalele 15, 666 și 999),
portul se conectează la servere **QuakeWorld**. Multiplayer → Join Game
acceptă o adresă sau caută în LAN. Particulele scriptate FTE sunt acceptate
pentru modurile care le folosesc.

## Linie de comandă și consolă

Opțiuni uzuale: `-basedir <dosar>`, `-game <dosar>`, `-hipnotic` / `-rogue` /
`-quoth`, `-width <w>`, `-height <h>`, `-fullscreen`, `-window` și
`+comandă argumente` pentru a rula comenzi de consolă la pornire. Consola
(`~`) acceptă comenzile și variabilele cvar obișnuite din Quake.

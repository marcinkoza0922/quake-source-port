# Brugervejledning

Denne vejledning handler om at spille Quake med denne port, med vægt på det,
den tilføjer i forhold til det originale Quake og andre source ports. Alt
nyt er valgfrit: med standardindstillingerne ser spillet ud og spiller som
Quake.

## Kom i gang

1. Skaf dit eget eksemplar af Quake. Peg porten på den mappe, der
   indeholder `id1/` (og eventuelle mods ved siden af), med `QUAKE_BASEDIR`
   eller `-basedir <mappe>`.
2. Kør `cargo run --release`, eller start et mod direkte med
   `-game <mappe> +map <bane>`.
3. Konfiguration, gemte spil og skærmbilleder ligger i
   `~/.local/share/quake-source-port/<spil>/`, én mappe pr. mod.

Hovedmenuen har Single Player, Multiplayer, Options, **Help** (denne
vejledning), Quit og **Banepakker**, med Quake-temaet spillende i
baggrunden (Options → Lyd → Hovedmenumusik). Alle menuer virker med mus,
tastatur, controller og berøring; hold markøren over eller vælg en
indstilling for at se et værktøjstip, der forklarer den.

I Hjælp vælger op/ned en sektion; Page Up/Page Down, venstre/højre,
skulderknapperne eller musehjulet over teksten ruller den.

## Sprog

Options → Gameplay → **Sprog** vælger sproget i menuer, skærmbeskeder og denne
vejledning: engelsk, fransk, tysk, italiensk, spansk, russisk, polsk, ukrainsk,
tjekkisk, slovakisk, kroatisk, bulgarsk, svensk, finsk, dansk, norsk,
nederlandsk, græsk, ungarsk, rumænsk, portugisisk (Portugal eller Brasilien),
japansk, koreansk, kinesisk (forenklet eller traditionelt), tyrkisk,
indonesisk, malajisk, filipino, vietnamesisk, serbisk, litauisk, lettisk,
estisk, maltesisk, albansk, armensk, georgisk, islandsk, irsk, walisisk,
baskisk, catalansk, galicisk, latin, esperanto, belarusisk, makedonsk, slovensk,
swahili, arabisk, hebraisk, persisk, urdu, hindi, marathi, bengali, punjabi,
gujarati, tamil, telugu, thai, khmer, amharisk eller kantonesisk.
**System** følger din computers sprog. Genudgivelsen fra 2021's
egen spiltekst er kun oversat til fransk, tysk, italiensk, spansk og
russisk; andre sprog viser den på engelsk. På andre sprog end engelsk
bruger den lille tekst i spillet menuskrifttypen, da den klassiske
Quake-skrifttype kun har latinske bogstaver uden accenter. Konsolbeskeder
forbliver på engelsk, medmindre **Oversæt konsol** er slået til.

## Banepakker (mod-browser)

Hovedmenu → Banepakker viser alle installerede mods og banepakker med deres
beskrivelse. Vælg en for at skifte til den, gennemse derefter dens baner,
og start en hvilken som helst af dem direkte. Browseren husker dine bedste
drab, hemmeligheder og tid pr. bane. Hvert mod har sin egen konfiguration,
så tildelinger og indstillinger tilpasset ét mod ikke smitter af på et
andet.

**Hvor banepakker skal ligge.** Porten leder efter dem i Quakes basismappe
(den, der indeholder `id1/`, sat med `QUAKE_BASEDIR` eller `-basedir`).
Alle undermapper der, som indeholder spildata (en `.pak`- eller
`.pk3`-fil, en `progs.dat` eller en `maps/`-mappe), vises på listen, så
installér en banepakke ved at pakke den ud i sin egen mappe ved siden af
`id1/`, for eksempel `<basedir>/ad/` eller `<basedir>/my_maps/maps/*.bsp`.
Tilføj eventuelt en `modinfo.txt` med linjerne `title`, `base` og
`description` for at styre, hvordan den vises. Pakker findes, når spillet
starter, så genstart efter at have tilføjet en. Gemte spil, skærmbilleder
og konfiguration for hver pakke ligger i
`~/.local/share/quake-source-port/<mappe>/`, ikke i pakkens mappe.

## Delt skærm

Op til seks spillere på én maskine, i co-op eller deathmatch.

- Multiplayer → New Game → **Lokale spillere** angiver, hvor mange der
  spiller.
- Hver spiller får sin egen visning, HUD, lyd og inputenhed (tastatur og
  mus for spiller 1, en controller hver til resten).
- Skærmlayoutet kan være stablet, side om side, 2×2 eller 3×2
  (`r_splitscreen_layout`).
- Hver spiller vælger et navn og farver på trøje og bukser under
  Multiplayer → Setup.

## Controllere

Porten er designet til at blive spillet med en gamepad.

- **Våbenhjul** (`+weaponwheel`): hold nede for at sænke tiden og vælge et
  våben med en af sticksene eller musen; slip for at skifte.
- **Sigtehjælp** (Options → Input → Controller): sløvning nær fjender og
  rotation, der følger dem. Den gælder altid kun for sticken, aldrig for
  musen.
- **Gyrosigte** (Options → Input → Controller → Gyro): en til/fra-knap,
  plus separat følsomhed for yaw-, pitch- og roll-akserne (roll er slået
  fra som standard og drejer visningen som yaw, negative værdier vender
  den om), et støjfilter, og hvornår gyro gælder (altid, fra eller kun
  mens `+gyroaction` holdes). **Sigtestil** skifter højre stick mellem
  2D-sigte (den drejer visningen jævnt) og **flick-stick** (skub sticken
  for straks at vende visningen i den retning, og drej derefter sticken for
  at rotere den); de to fungerer godt sammen med gyro til fint sigte.
- **Vibration**, stick-følsomhed og omvending.
- **Knapsymboler** matcher din controller (Xbox, PlayStation, Nintendo) og
  registreres automatisk.
- Tildel alt på ny under Options → Styring, også fra controlleren.
- Berøringsstyring med justerbar størrelse, opacitet og
  kighastighed.

## Gemning

- **Autogemninger** roterer adskilt fra manuelle gemninger og
  hurtiggemninger, så de aldrig overskriver dine egne. Options → Gameplay
  → Indstillinger for autogem vælger, hvor mange der beholdes, og hvornår
  der gemmes: ved baneindgang, med et interval, når en hemmelighed eller
  alle hemmeligheder findes, og når alle fjender er dræbt.
- **Gemmekort** i menuerne til indlæsning og gemning viser bane, liv,
  rustning, våben, nøgler og drab/hemmeligheder for hvert gemt spil.
- **Ved død** (Options → Gameplay → Ved død) kan enkeltspiller spørge,
  hvad der skal ske, i en pauset menu, indlæse det nyeste gemte spil, lade
  dig vælge et gemt spil eller genstarte banen.

## Gameplay og kamera

Options → Gameplay:

- Synsfelt 60–130, kameragyngen, våbenrekyl, løb altid.
- **Kamera**: førsteperson, tredjeperson eller over skulderen (højre eller
  venstre).
- Placering af **våbenmodellen**: midte, højre, venstre eller skjult.

## Tilpas: gameplay-modifikatorer og add-ons

Et spil er enten *standard* eller *tilpasset*, valgt når det starter. New
Game (og valg af en bane i Vælg bane eller Banepakker) viser et pop op-
vindue: **Spil** starter spillet, **Tilpas oplevelsen** åbner
Gameplay-modifikatorer og Add-ons. Så længe noget er ændret, er Spil
mærket **TILPASSET**, og **Slå alle ændringer fra** (Del, gamepad X eller
klik) gendanner standardreglerne uden add-ons. Standardspil gemmer
almindelige `.sav`-filer, som ironwail og vkQuake kan indlæse; tilpassede
spil gemmer `.qsa`-filer, som kun denne port indlæser (det samme gør spil
på delt skærm og co-op-spil, som andre porte ikke kan gemme). Under et
tilpasset spil ændrer Hovedmenu → Tilpas dets modifikatorer og add-ons
når som helst; et standardspils regler forbliver fastlåste. Modifikatorer
virker med ethvert mod og slås fra i deathmatch.

- Sværere: mere modtaget skade, sejere fjender, mindre ammunition og liv
  fra fund, 2–4× monstre, pistolstart på hver bane, ingen manuel gemning.
- Lettere: mindre modtaget skade, svagere fjender, mere ammunition og liv,
  uendelig ammunition, livsregenerering, ingen faldskade, ingen drukning,
  ingen selvskade, dobbelte eller tredobbelte hop.

## Demoer

`record <navn>` og `stop` optager en demo i din brugermappe; `playdemo
<navn>` afspiller den. Når optagelsen stopper, skrives demoen om i
baggrunden, så ironwail og vkQuake også kan afspille den. Hvis et mod
bruger partikeleffekter, som andre porte ikke kan vise, gemmes den fulde
version som `<navn>.qsd` (`playdemo <navn>.qsd`). `demo_convert <navn>`
konverterer en ældre demo.

Hovedmenu → Demoer viser dine gemte demoer og, med Indstillinger → Demoer →
Optag demoer automatisk slået til, en automatisk demo af hver bane, du
spiller; de ældste erstattes, når pladserne er fulde. K beholder en, E
eksporterer den som video, og Del sletter den. Fejlrapporter indeholder den
aktuelle banes automatiske demo.

Under en demo sætter Mellemrum på pause, at holde Venstre eller Højre spoler
tilbage eller frem, Op og Ned ændrer hastigheden, PgUp og PgDn springer 10
sekunder, og Home og End går til starten og til næste bane. I og O markerer
et klip; E eksporterer det, eller hele demoen, til mappen `videos`.
Knapperne bliver på skærmen; H eller Skjul skjuler dem, og når du peger på
den nederste kant, vises en bjælke, der henter dem tilbage. Klik på
knapperne, klik på tidslinjen for at hoppe dertil, eller træk hen over den
for at markere et klip; Eksportér spørger om billedfrekvens, størrelse,
format og fil, og om det markerede klip eller hele demoen skal eksporteres.

`capture` starter og stopper en videooptagelse, mens du spiller.
Billedfrekvens, format og kvalitet findes under Indstillinger → Demoer →
Videooptagelse. Video kræver ffmpeg; uden det gemmes billederne som
PNG-billeder med en WAV-fil.

## Tilgængelighed

Options → Tilgængelighed:

- Reduceret bevægelse og styrke på skærmglimt.
- Farveblindhedsfiltre.
- Konfigurerbart sigtekorn: form, størrelse, omrids og farveskift på et
  mål eller ved et drab; valgfrie træfmarkører.
- **Lydundertekster** og beskedindstillinger (Beskeder & undertekster),
  tekstskalering.
- Angreb og løb som til/fra (i stedet for at holde).
- En skyder for spilhastighed, der sænker hele spillets tempo.
- Automatisk sigte med låseindikator samt indstillinger for kegle og
  rækkevidde.

## Forbedringer

Options → Forbedringer. Alt kører på klientsiden og er slået fra som
standard, så det virker med ethvert mod og online.

- **Vragrester**: tumlende kødstumper og granatsplinter fra sprængte
  tønder, også med mod-monstre; de kan støde mod døre og elevatorer.
- **Forbedret blodstænk**: stænk på overflader, sår der følger kroppe,
  dråber i luften der klasker, hvor de lander.
- **Nedslagseffekter**: hagl-skudhuller, søm der sætter sig fast i træ og
  fjender, brændemærker fra eksplosioner.
- **Forbedret vand**: gennemsigtighed, bølger, plask og kaustik.
- **Gennemsigtige teleportere**, der viser deres destination med
  parallakse.
- Ambient occlusion, bevægelsessløring pr. objekt og ved fart, bløde
  partikler.
- **Skærmshadere**: RetroArch `.slangp`-forudindstillinger (CRT og andre).

Options → Grafik har også de sædvanlige indstillinger: renderskala,
kantudjævning, supersampling, teksturfiltrering, 8-bit farver med
dithering, tåge, farvet belysning og interpolation af animation og
bevægelse.

## Add-ons

Add-ons er nye våben og nyt udstyr, som motoren styrer, så de virker med
ethvert mods progs. De findes kun i enkeltspiller og co-op.

### Start et spil med add-ons

Et spil har kun add-ons, hvis det starter med dem:

1. Single Player → sæt flueben i **Add-ons**, og vælg derefter New Game.
2. Vælg, hvilke add-ons du vil have med, og justér deres indstillinger, og
   start derefter det nye spil.

Spil med add-ons gemmes som `.qsa`-filer i stedet for `.sav`, så andre
porte ikke forsøger at indlæse dem. Gemmekort viser et "ADD-ONS"-mærke.
Indlæsning af et normalt gemt spil slår add-ons fra. Mens du spiller et
spil med add-ons, slår siden Add-ons (konsol: `menu_options addons`)
enkelte add-ons til og fra; hver har en forhåndsvisning i løkke, en
vejledning, sine tildelinger og sine egne indstillinger.

### Våben

Add-ons placeres på de almindelige våbenpladser (vælg hvilken pr. add-on,
eller plads 9 og 0) og vises på våbenhjulet og i HUD'en. Skyd og
alternativ skyd er dine sædvanlige angrebsknapper.

| Add-on | Skyd | Alternativ skyd | Ammunition |
|---|---|---|---|
| **Portalpistol** | Placerer den blå portal | Placerer den orange portal | Ingen |
| **Railgun** | Gennemborende projektil, der rammer alt på en linje | — | 2 celler |
| **Entrehage** | Hager sig fast og trækker dig hen til træ, metal, sten eller fjender | — | Ingen |
| **Klæbebombekaster** | Hold for at lade op, slip for at affyre | Udløs | 1 raket |
| **Boomerang** | Hold for at markere fjender, slip for at kaste; den lammer hver enkelt efter tur | Ryd markeringer | Ingen |
| **Gelsprøjte** | Blå frastødningsgel (hoppende) | Orange fremdriftsgel (hurtig) | Ingen |
| **Højtryksrenser** | Lang vandstråle, der skubber fjender og skyller gel af | Bred vifte af stråler | Ingen |

Portaler placeres på flade overflader i verden og fører dig, monstre og
kastede genstande igennem med deres fart. Med **Se gennem portaler** slået
til ser du den anden side live, også portaler der er synlige gennem
portaler; kvalitet, antal live-visninger, afstand og rekursionsdybde kan
justeres.

### Udstyr

Udstyr slås til og fra i stedet for at blive holdt i hånden. Åbn
**udstyrshjulet** (`+equipwheel`), eller brug `goggles cycle` for at
skifte igennem:

- **Nattesyn**
- **Termisk syn**

## Multiplayer

Ud over NetQuake-spil på LAN og internettet (protokol 15, 666 og 999)
forbinder porten til **QuakeWorld**-servere. Multiplayer → Join Game tager
en adresse eller søger på LAN. FTE-scriptede partikler understøttes for
mods, der bruger dem.

## Kommandolinje og konsol

Almindelige tilvalg: `-basedir <mappe>`, `-game <mappe>`, `-hipnotic` /
`-rogue` / `-quoth`, `-width <b>`, `-height <h>`, `-fullscreen`, `-window`
og `+kommando argumenter` for at køre konsolkommandoer ved opstart.
Konsollen (`~`) accepterer de sædvanlige Quake-kommandoer og cvars.

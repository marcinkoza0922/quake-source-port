# Brukerhåndbok

Denne håndboken handler om å spille Quake med denne porten, med vekt på det
den legger til i forhold til vanlig Quake og andre kildeporter. Alt nytt er
valgfritt: med standardinnstillingene ser spillet ut og spilles som Quake.

## Kom i gang

1. Skaff ditt eget eksemplar av Quake. Pek porten til mappen som
   inneholder `id1/` (og eventuelle moder ved siden av), med `QUAKE_BASEDIR`
   eller `-basedir <dir>`.
2. Kjør `cargo run --release`, eller start en mod direkte med
   `-game <dir> +map <map>`.
3. Konfigurasjon, lagringer og skjermbilder ligger i
   `~/.local/share/quake-source-port/<game>/`, én mappe per mod.

Hovedmenyen har Single Player, Multiplayer, Options, **Help** (denne
håndboken), Quit og **Banepakker**, med Quake-temaet spillende i bakgrunnen
(Options → Lyd → Hovedmenymusikk). Alle menyer virker med mus, tastatur,
kontroller og berøring; hold pekeren over eller velg et alternativ for å se
et verktøytips som forklarer det.

I hjelpen velger opp/ned en seksjon; Page Up/Page Down, venstre/høyre,
skulderknappene eller musehjulet over teksten ruller den.

## Språk

Options → Spilling → **Språk** velger språket i menyene, meldingene på skjermen
og denne håndboken: engelsk, fransk, tysk, italiensk, spansk, russisk, polsk,
ukrainsk, tsjekkisk, slovakisk, kroatisk, bulgarsk, svensk, finsk, dansk,
norsk, nederlandsk, gresk, ungarsk, rumensk, portugisisk (Portugal eller
Brasil), japansk, koreansk, kinesisk (forenklet eller tradisjonell), tyrkisk,
indonesisk, malayisk, filipino, vietnamesisk, serbisk, litauisk, latvisk,
estisk, maltesisk, albansk, armensk, georgisk, islandsk, irsk, walisisk,
baskisk, katalansk, galisisk, latin, esperanto, belarusisk, makedonsk, slovensk,
swahili, arabisk, hebraisk, persisk, urdu, hindi, marathi, bengali, punjabi,
gujarati, tamil, telugu, thai, khmer, amharisk eller kantonesisk.
**System** følger datamaskinens språk. Spillteksten fra 2021-utgaven er
bare oversatt til fransk, tysk, italiensk, spansk og russisk; andre språk
viser den på engelsk. På andre språk enn engelsk bruker den lille teksten i
spillet menyskriften, siden den klassiske Quake-skriften bare har latinske
bokstaver uten aksenter. Konsollmeldinger forblir på engelsk med mindre
**Oversett konsollen** er på.

## Banepakker (modleser)

Hovedmeny → Banepakker viser alle installerte moder og banepakker med
beskrivelse. Velg en for å bytte til den, bla så gjennom banene og start
hvilken som helst av dem direkte. Leseren husker dine beste drap,
hemmeligheter og tid per bane. Hver mod har sin egen konfigurasjon, så
tilordninger og innstillinger tilpasset én mod lekker ikke over i en annen.

**Hvor banepakker skal ligge.** Porten leter etter dem i Quakes basismappe
(den som inneholder `id1/`, satt med `QUAKE_BASEDIR` eller `-basedir`). Alle
undermapper der som inneholder spilldata (en `.pak`- eller `.pk3`-fil, en
`progs.dat` eller en `maps/`-mappe) vises, så installer en banepakke ved å
pakke den ut i en egen mappe ved siden av `id1/`, for eksempel
`<basedir>/ad/` eller `<basedir>/my_maps/maps/*.bsp`. Du kan også legge til
en `modinfo.txt` med linjene `title`, `base` og `description` for å styre
hvordan den vises. Pakker blir funnet når spillet starter, så start på nytt
etter at du har lagt til en. Lagringer, skjermbilder og konfigurasjon for
hver pakke ligger i `~/.local/share/quake-source-port/<folder>/`, ikke i
pakkemappen.

## Delt skjerm

Opptil seks spillere på én maskin, i samarbeid eller deathmatch.

- Multiplayer → New Game → **Lokale spillere** angir hvor mange som spiller.
- Hver spiller får sin egen visning, HUD, lyd og inndataenhet (tastatur og
  mus for spiller 1, én kontroller hver for resten).
- Skjermoppsettet kan være over hverandre, side om side, 2×2 eller 3×2
  (`r_splitscreen_layout`).
- Hver spiller velger navn og skjorte-/buksefarger under Multiplayer →
  Setup.

## Kontrollere

Porten er laget for å spilles med håndkontroller.

- **Våpenhjul** (`+weaponwheel`): hold for å senke farten og velg et våpen
  med en av spakene eller musen; slipp for å bytte.
- **Siktehjelp** (Options → Inndata → Spillkontroller): bremsing nær fiender
  og dreining som følger dem. Den gjelder bare spaken, aldri musen.
- **Gyrosikting** (Options → Inndata → Spillkontroller → Gyro): en bryter,
  pluss separat følsomhet for horisontal, vertikal og rulleakse (rulling er
  av som standard og dreier visningen som den horisontale aksen, negative
  verdier snur den), et støyfilter, og når gyroen gjelder (alltid, av eller
  kun mens `+gyroaction` holdes). **Siktestil** bytter høyre spak mellom
  2D-sikting (den dreier visningen jevnt) og **flick-stick** (skyv spaken
  for å vende visningen straks i den retningen, og drei så spaken for å
  rotere den); begge passer godt sammen med gyro for finsikting.
- **Vibrasjon**, spakfølsomhet og invertering.
- **Knappesymboler** samsvarer med kontrolleren din (Xbox, PlayStation,
  Nintendo) og gjenkjennes automatisk.
- Tilordne alt på nytt i Options → Kontroller, også fra kontrolleren.
- **Select / View ×2**: Skjermbilde; **Start ×2**: Hurtiglagre; **Start**
  (Hold): Hurtiglast.
- Berøringskontroller med justerbar størrelse, dekkevne og blikkfart.

## Lagring

- **Autolagringer** roteres separat fra manuelle lagringer og
  hurtiglagringer, så de overskriver aldri dine egne. Options → Spilling →
  Autolagring-innstillinger velger hvor mange som beholdes og når det
  lagres: ved banestart, på tidtaker, når du finner en hemmelighet eller
  alle hemmeligheter, og når alle fiender er drept.
- **Lagringskort** i Load/Save-menyene viser bane, helse, rustning, våpen,
  nøkler og drap/hemmeligheter for hver lagring.
- **Ved død** (Options → Spilling → Ved død) kan enspiller spørre hva som
  skal skje i en pausemeny, laste inn nyeste lagring, la deg velge en
  lagring, eller starte banen på nytt.

## Spilling og kamera

Options → Spilling:

- Synsfelt 60–130, kameravugging, våpenrekyl, alltid løp.
- **Kamera**: førsteperson, tredjeperson, eller over skulderen (høyre eller
  venstre).
- Plassering av **våpenmodeller**: midten, høyre, venstre eller skjult.

## Tilpass: spillmodifikatorer og tillegg

Et spill er enten *vanlig* eller *tilpasset*, valgt når det starter. New
Game (og valg av bane i Banevalg eller Banepakker) viser et vindu:
**Spill** starter spillet, **Tilpass opplevelsen** åpner Spillmodifikatorer
og Tillegg. Så lenge noe er endret, er Spill merket **TILPASSET**, og
**Slå av alle endringer** (Del, X på kontrolleren, eller klikk) setter
tilbake standardreglene uten tillegg. Vanlige spill lagrer ordinære
`.sav`-filer som ironwail og vkQuake kan laste; tilpassede spill lagrer
`.qsa`-filer som bare denne porten laster (det samme gjelder spill på delt
skjerm og samarbeidsspill, som andre porter ikke kan lagre). Under et
tilpasset spill endrer Hovedmeny → Tilpass modifikatorene og tilleggene når
som helst; et vanlig spills regler forblir låst. Modifikatorer virker med
alle moder og slås av i deathmatch.

- Vanskeligere: mer skade mottatt, tøffere fiender, færre ammo- og
  helsefunn, 2–4x monstre, pistolstart på hver bane, ingen manuell lagring.
- Lettere: mindre skade mottatt, svakere fiender, mer ammo og helse,
  uendelig ammo, helseregenerering, ingen fallskade, ingen drukning, ingen
  egenskade, doble eller tredoble hopp.

## Demoer

`record <name>` og `stop` tar opp en demo i brukermappen din; `playdemo
<name>` spiller den av. Når opptaket stopper, skrives demoen om i
bakgrunnen slik at ironwail og vkQuake også kan spille den av. Hvis en mod
bruker partikkeleffekter som andre porter ikke kan vise, beholdes den fulle
versjonen som `<name>.qsd` (`playdemo <name>.qsd`). `demo_convert <name>`
konverterer en eldre demo.

Hovedmeny → Demoer viser de lagrede demoene dine og, med Innstillinger →
Demoer → Ta opp demoer automatisk slått på, en automatisk demo av hvert
brett du spiller; de eldste erstattes når plassene er fulle. Skriv for å
filtrere listene, og trykk Tab for å bytte sortering; knappene ved siden av
listen spiller av, eksporterer, gir nytt navn til, beholder eller sletter
valgt demo, og en automatisk demo som får nytt navn, beholdes.
Feilrapporter inneholder det gjeldende brettets automatiske demo.

Under en demo pauser Mellomrom, å holde Venstre eller Høyre spoler tilbake
eller frem, Opp og Ned endrer hastigheten, PgUp og PgDn hopper 10 sekunder,
og Home og End går til starten og til neste brett. I og O merker et klipp; E
eksporterer det, eller hele demoen, til mappen `videos`. Kontrollene blir
værende på skjermen; H eller Skjul skjuler dem, og når du peker på
nederkanten, vises en stripe som henter dem tilbake. Klikk på knappene,
klikk på tidslinjen for å hoppe dit, eller dra over den for å markere et
klipp; Eksporter spør om bildefrekvens, størrelse, format og fil, og om det
markerte klippet eller hele demoen skal eksporteres.

På en kontroller pauser A, styrekorset spoler bakover og forover og endrer
hastigheten, LB og RB hopper 10 sekunder, og spakknappene går til starten og
til neste brett. LT og RT merker et klipp, B fjerner det, X eksporterer, Y
beholder en automatisk demo, og View skjuler kontrollene.

`capture` starter og stopper et videoopptak mens du spiller. Bildefrekvens,
format og kvalitet finner du under Innstillinger → Demoer → Videoopptak.
Video krever ffmpeg; uten det lagres bildene som PNG-bilder med en WAV-fil.

## Tilgjengelighet

Options → Tilgjengelighet:

- Redusert bevegelse og styrke på skjermblink.
- Fargeblindfiltre.
- Justerbart trådkors: form, størrelse, kontur og fargeendring på mål eller
  ved drap; valgfrie treffmarkører.
- **Lydteksting**; plassering, størrelse og visningstid for meldinger og
  teksting stilles inn i HUD-redigeringen.
- Angrep og løp av/på (i stedet for å holde).
- En glidebryter for spillfart som senker farten i hele spillet.
- Autosikting med mållåsindikator, innstillinger for kjegle og rekkevidde.

## Forbedringer

Options → Forbedringer. Alt kjører på klientsiden og er av som standard, så
det virker med alle moder og på nett.

- **Rusk**: kjøttbiter som ruller og splinter fra knuste tønner, også med
  monstre fra moder; det kan dunke mot dører og heiser.
- **Forbedret blodsprut**: sprut på flater, sår som følger kroppene, dråper
  i luften som skvetter der de lander.
- **Treffeffekter**: hagleskuddmerker, spiker som setter seg fast i tre og
  fiender, svimerker fra eksplosjoner.
- **Forbedret vann**: gjennomskinnelighet, bølger, sprut og kaustikk;
  **Lysbrytning under vann**, **Dråper på skjermen**.
- **Gjennomsiktige teleportere** som viser destinasjonen med parallakse.
- Ambient okklusjon, bevegelsesuskarphet per objekt og ved fart, myke
  partikler.
- **Skjermshadere**: RetroArch `.slangp`-forhåndsinnstillinger (CRT og
  andre).

Options → Grafikk har også de vanlige innstillingene for gjengivelsesskala,
kantutjevning, supersampling, teksturfiltrering, 8-biters farger med
dithering, tåke, farget belysning og interpolering av animasjon/bevegelse.

## Tillegg

Tillegg er nye våpen og nytt utstyr som drives av motoren, så de virker med
progs fra alle moder. De er kun for enspiller og samarbeid.

### Starte et spill med tillegg

Et spill har bare tillegg hvis det starter med dem:

1. Single Player → kryss av for **Tillegg**, deretter New Game.
2. Velg hvilke tillegg du vil ta med og juster innstillingene, og start så
   et nytt spill.

Spill med tillegg lagres som `.qsa`-filer i stedet for `.sav`, så andre
porter ikke prøver å laste dem. Lagringskort viser merket «TILLEGG». Å laste
inn en vanlig lagring slår av tilleggene. Mens du spiller et spill med
tillegg, slår Tillegg-siden (konsoll: `menu_options addons`) enkelttillegg
av og på; hvert av dem har en forhåndsvisning i sløyfe, instruksjoner,
tilordninger og egne innstillinger.

### Våpen

Tillegg legges i de vanlige våpenplassene (velg hvilken per tillegg, eller
plass 9 og 0) og vises på våpenhjulet og i HUD-en. Ild og alternativ ild er
de vanlige angrepsknappene dine.

| Tillegg | Ild | Alternativ ild | Ammo |
|---|---|---|---|
| **Portalkanon** | Plasser den blå portalen | Plasser den oransje portalen | Ingen |
| **Railgun** | Gjennomborende prosjektil som treffer alt på en linje | — | 2 celler |
| **Entrehake** | Fest deg og trekk deg mot tre, metall, stein eller fiender | — | Ingen |
| **Klebebombekaster** | Hold for å lade, slipp for å skyte | Utløs | 1 rakett |
| **Bumerang** | Hold for å merke fiender, slipp for å kaste; den lammer hver av dem etter tur | Fjern merker | Ingen |
| **Gelsprøyte** | Blå frastøtningsgel (sprettende) | Oransje fremdriftsgel (rask) | Ingen |
| **Høytrykksspyler** | Lang vannstråle som dytter fiender og vasker bort gel | Bred vifte av stråler | Ingen |
| **Han-han-kabel** | Sekundærild mot et lysende elektronikkpanel innen rekkevidde plugger inn den ene enden; den andre skyter da lyn med ubegrenset ammunisjon | Sekundærild eller våpenbytte river den ut | Ingen |
| **Doble pistoler** | Venstre | Høyre | Ingen (Lad om: R) |
| **Automatgevær** | Et helautomatisk gevær som lades med spikrene dine | Alternativ ild sikter gjennom rødpunktsiktet for tettere skudd og nærmere syn | Spiker (Lad om: R) |

Portaler plasseres på flate flater i verdenen og fører deg, monstre og
kastede gjenstander gjennom med farten i behold. Med **Se gjennom
portaler** på ser du den andre siden direkte, også portaler som er synlige
gjennom portaler; kvalitet, antall direktevisninger, avstand og
rekursjonsdybde kan justeres.

### Utstyr

Utstyr slås av og på i stedet for å holdes. Åpne **utstyrshjulet**
(`+equipwheel`), eller bruk `goggles cycle` for å bla gjennom:

- **Nattsyn**
- **Varmesyn**

## Flerspiller

I tillegg til NetQuake-spill på LAN og internett (protokoll 15, 666 og 999)
kobler porten seg til **QuakeWorld**-servere. Multiplayer → Join Game
tar en adresse eller søker i LAN. FTE-skriptede partikler støttes for moder
som bruker dem.

## Kommandolinje og konsoll

Vanlige valg: `-basedir <dir>`, `-game <dir>`, `-hipnotic` / `-rogue` /
`-quoth`, `-width <w>`, `-height <h>`, `-fullscreen`, `-window`, og
`+command args` for å kjøre konsollkommandoer ved oppstart. Konsollen (`~`)
godtar de vanlige Quake-kommandoene og cvar-ene.

# Användarhandbok

Den här handboken beskriver hur du spelar Quake med den här porten, med
tonvikt på det som den lägger till jämfört med originalet och andra
källportar. Allt nytt är valfritt: med standardinställningarna ser spelet
ut och spelas som Quake.

## Komma igång

1. Skaffa ett eget exemplar av Quake. Peka porten mot mappen som
   innehåller `id1/` (och eventuella moddar bredvid den), med
   `QUAKE_BASEDIR` eller `-basedir <mapp>`.
2. Kör `cargo run --release`, eller starta en mod direkt med
   `-game <mapp> +map <bana>`.
3. Konfiguration, sparningar och skärmdumpar ligger i
   `~/.local/share/quake-source-port/<spel>/`, en mapp per mod.

Huvudmenyn har Single Player, Multiplayer, Options, **Help** (den här
handboken), Quit och **Banpaket**, med Quake-temat spelande i bakgrunden
(Options → Ljud → Huvudmenymusik). Alla menyer fungerar med mus,
tangentbord, handkontroll och pekskärm; håll pekaren över eller välj ett
alternativ för att se ett förklarande verktygstips.

I hjälpen väljer upp/ner ett avsnitt; Page Up/Page Down, vänster/höger,
axelknapparna eller mushjulet över texten rullar den.

## Språk

Options → Spelmekanik → **Språk** väljer språk för menyer, skärmmeddelanden och
den här handboken: engelska, franska, tyska, italienska, spanska, ryska,
polska, ukrainska, tjeckiska, slovakiska, kroatiska, bulgariska, svenska,
finska, danska, norska, nederländska, grekiska, ungerska, rumänska,
portugisiska (Portugal eller Brasilien), japanska, koreanska, kinesiska
(förenklad eller traditionell), turkiska, indonesiska, malajiska, filippinska,
vietnamesiska, serbiska, litauiska, lettiska, estniska, maltesiska, albanska,
armeniska, georgiska, isländska, iriska, walesiska, baskiska, katalanska,
galiciska, latin, esperanto, belarusiska, makedonska, slovenska, swahili,
arabiska, hebreiska, persiska, urdu, hindi, marathi, bengali, punjabi,
gujarati, tamil, telugu, thailändska, khmer, amhariska eller kantonesiska.
**System** följer datorns språk. Spelets egen text från 2021 års nyutgåva
finns bara översatt till franska, tyska, italienska, spanska och ryska;
andra språk visar den på engelska. På andra språk än engelska använder den
lilla texten i spelet menytypsnittet, eftersom det klassiska
Quake-typsnittet bara har latinska bokstäver utan accenter.
Konsolmeddelanden förblir på engelska om inte **Översätt konsolen** är på.

## Banpaket (modbläddrare)

Huvudmenyn → Banpaket listar varje installerad mod och varje banpaket med
dess beskrivning. Välj ett för att byta till det, bläddra sedan bland dess
banor och starta vilken som helst direkt. Bläddraren håller reda på dina
bästa resultat för dödade fiender, hemligheter och tid per bana. Varje mod
har sin egen konfiguration, så tilldelningar och inställningar som anpassats
för en mod läcker inte över till en annan.

**Var banpaket hamnar.** Porten letar efter dem i Quakes basmapp (den som
innehåller `id1/`, anges med `QUAKE_BASEDIR` eller `-basedir`). Varje
undermapp där som innehåller speldata (en `.pak`- eller `.pk3`-fil, en
`progs.dat` eller en `maps/`-mapp) listas, så installera ett banpaket genom
att packa upp det i en egen mapp bredvid `id1/`, till exempel
`<basedir>/ad/` eller `<basedir>/my_maps/maps/*.bsp`. Lägg om du vill till
en `modinfo.txt` med raderna `title`, `base` och `description` för att styra
hur det listas. Paket hittas när spelet startar, så starta om efter att ha
lagt till ett. Sparningar, skärmdumpar och konfiguration för varje paket
ligger i `~/.local/share/quake-source-port/<mapp>/`, inte i paketets mapp.

## Delad skärm

Upp till sex spelare på en dator, i samarbete eller deathmatch.

- Multiplayer → New Game → **Lokala spelare** anger hur många som spelar.
- Varje spelare får sin egen vy, HUD, ljud och inmatningsenhet (tangentbord
  och mus för spelare 1, en handkontroll var för de övriga).
- Skärmlayouten kan vara staplad, sida vid sida, 2×2 eller 3×2
  (`r_splitscreen_layout`).
- Varje spelare väljer namn och tröj-/byxfärg under Multiplayer → Setup.

## Handkontroller

Porten är gjord för att spelas med handkontroll.

- **Vapenhjul** (`+weaponwheel`): håll för att sakta ner tiden och välj ett
  vapen med valfri spak eller musen; släpp för att byta.
- **Siktstöd** (Options → Inmatning → Handkontroll): inbromsning nära
  fiender och rotation som följer dem. Det gäller bara spaken, aldrig
  musen.
- **Gyrosikte** (Options → Inmatning → Handkontroll → Gyro): en
  på/av-knapp, plus separat känslighet för gir-, nick- och rollaxeln
  (roll är av som standard och vrider vyn som giraxeln, negativa värden
  vänder riktningen), ett brusfilter och när gyrot gäller (alltid, av eller
  bara medan `+gyroaction` hålls). **Siktstil** växlar höger spak mellan
  2D-sikte (den vrider vyn steglöst) och **flick-spak** (tryck spaken för
  att rikta vyn direkt åt det hållet, vrid sedan spaken för att rotera
  den); båda fungerar bra tillsammans med gyro för finsiktning.
- **Vibration**, spakkänslighet och invertering.
- **Knappsymboler** matchar din handkontroll (Xbox, PlayStation, Nintendo)
  och identifieras automatiskt.
- Ändra alla tilldelningar under Options → Kontroller, även från
  handkontrollen.
- Pekskärmskontroller med justerbar storlek, opacitet och tittkänslighet.

## Spara

- **Autosparningar** roterar separat från manuella sparningar och
  snabbsparningar, så de skriver aldrig över dina egna. Options →
  Spelmekanik → Autosparinställningar väljer hur många som ska behållas och
  när det ska sparas: vid banstart, med ett intervall, när du hittar en
  hemlighet eller alla hemligheter och när du dödat alla fiender.
- **Sparkort** i menyerna Load/Save visar bana, hälsa, rustning, vapen,
  nycklar och dödade fiender/hemligheter för varje sparning.
- **Vid död** (Options → Spelmekanik → Vid död) kan enspelarläget fråga
  vad som ska göras i en pausad meny, ladda den senaste sparningen, låta dig
  välja en sparning eller starta om banan.

## Spelmekanik och kamera

Options → Spelmekanik:

- Synfält 60–130, gungande vy, vapenrekyl, spring alltid.
- **Kamera**: förstaperson, tredjeperson eller över axeln (höger eller
  vänster).
- **Vapenmodellens** position: mitten, höger, vänster eller dold.

## Anpassa: spelmodifierare och tillägg

Ett spel är antingen *vanligt* eller *anpassat*, vilket väljs när det
startar. New Game (och val av bana i Välj bana eller Banpaket) visar en
ruta: **Spela** startar spelet, **Anpassa upplevelsen** öppnar
Spelmodifierare och Tillägg. Så länge något är ändrat är Spela märkt
**ANPASSAT**, och **Inaktivera alla ändringar** (Del, X på handkontrollen
eller klick) återställer standardreglerna utan tillägg. Vanliga spel sparar
vanliga `.sav`-filer som ironwail och vkQuake kan ladda; anpassade spel
sparar `.qsa`-filer som bara den här porten laddar (likaså spel i delad
skärm och samarbete, som andra portar inte kan spara). Under ett anpassat
spel ändrar Huvudmenyn → Anpassa dess modifierare och tillägg när som
helst; ett vanligt spels regler är låsta. Modifierare fungerar med alla
moddar och stängs av i deathmatch.

- Svårare: mer mottagen skada, tuffare fiender, mindre ammunition och hälsa
  från upplockningar, 2–4× monster, pistolstart på varje bana, inga
  manuella sparningar.
- Lättare: mindre mottagen skada, svagare fiender, mer ammunition och
  hälsa, oändlig ammunition, hälsoregenerering, ingen fallskada, ingen
  drunkning, ingen självskada, dubbel- eller trippelhopp.

## Demor

`record <namn>` och `stop` spelar in en demo i din användarmapp;
`playdemo <namn>` spelar upp den. När inspelningen stoppas skrivs demon om
i bakgrunden så att även ironwail och vkQuake kan spela upp den. Om en mod
använder partikeleffekter som andra portar inte kan visa, sparas den
fullständiga versionen som `<namn>.qsd` (`playdemo <namn>.qsd`).
`demo_convert <namn>` konverterar en äldre demo.

Huvudmeny → Demor visar dina sparade demor och, med Alternativ → Demor →
Spela in demor automatiskt påslaget, en automatisk demo av varje bana du
spelar; de äldsta ersätts när platserna är fulla. K behåller en, E
exporterar den som video och Del tar bort den. Felrapporter innehåller den
aktuella banans automatiska demo.

Under en demo pausar Mellanslag, att hålla Vänster eller Höger spolar bakåt
eller framåt, Upp och Ned ändrar hastigheten, PgUp och PgDn hoppar 10
sekunder, och Home och End går till början och till nästa bana. I och O
markerar ett klipp; E exporterar det, eller hela demon, till mappen
`videos`. Kontrollerna ligger kvar på skärmen; H eller Dölj döljer dem, och
att peka på nederkanten visar en remsa som tar tillbaka dem. Klicka på
knapparna, klicka på tidslinjen för att hoppa dit, eller dra över den för
att markera ett klipp; Exportera frågar efter bildfrekvens, storlek, format
och fil, och om det markerade klippet eller hela demon ska exporteras.

`capture` startar och stoppar en videoinspelning medan du spelar.
Bildfrekvens, format och kvalitet finns under Alternativ → Demor →
Videoinspelning. Video kräver ffmpeg; utan det sparas bilderna som
PNG-bilder med en WAV-fil.

## Tillgänglighet

Options → Tillgänglighet:

- Minskad rörelse och intensitet för skärmblixtar.
- Färgblindhetsfilter.
- Anpassningsbart hårkors: form, storlek, kontur och färgbyte vid mål
  eller vid dödande; valfria träffmarkörer.
- **Ljudtextning** och meddelandeinställningar (Meddelanden & undertexter),
  textskalning.
- Växla (i stället för att hålla) attack och spring.
- Ett reglage för spelhastighet som saktar ner hela spelet.
- Autosikte med låsindikator och inställningar för kon och räckvidd.

## Förbättringar

Options → Förbättringar. Allt körs på klientsidan och är av som standard,
så det fungerar med alla moddar och online.

- **Bråte**: köttbitar som rullar och splitter från krossade tunnor, även
  med moddarnas monster; den kan stöta mot dörrar och hissar.
- **Förbättrat blod**: stänk på ytor, sår som följer kropparna, droppar i
  luften som stänker där de landar.
- **Nedslagseffekter**: hagelmärken, spikar som fastnar i trä och fiender,
  brännmärken från explosioner.
- **Förbättrat vatten**: genomskinlighet, krusningar, stänk och kaustik.
- **Genomskinliga teleportrar** som visar sitt mål med parallax.
- Ambient ocklusion, rörelseoskärpa per objekt och vid hög fart, mjuka
  partiklar.
- **Skärmshaders**: RetroArch-förinställningar i `.slangp` (CRT och
  andra).

Options → Grafik har också de vanliga inställningarna: renderingsskala,
kantutjämning, supersampling, texturfiltrering, 8-bitarsfärg med dithering,
dimma, färgad belysning och interpolering av animation/rörelse.

## Tillägg

Tillägg är nya vapen och ny utrustning som drivs av motorn, så de fungerar
med varje mods progs. De finns bara i enspelarläge och samarbete.

### Starta ett spel med tillägg

Ett spel har tillägg bara om det startas med dem:

1. Single Player → kryssa i rutan **Tillägg**, sedan New Game.
2. Välj vilka tillägg du tar med och justera deras inställningar, starta
   sedan det nya spelet.

Spel med tillägg sparas som `.qsa`-filer i stället för `.sav`, så att andra
portar inte försöker ladda dem. Sparkort visar märket ”ADD-ONS”. Att ladda
en vanlig sparning stänger av tilläggen. Under ett spel med tillägg slår
sidan Tillägg (konsol: `menu_options addons`) på och av enskilda tillägg;
vart och ett har en förhandsvisning i slinga, instruktioner, sina
tilldelningar och egna inställningar.

### Vapen

Tillägg hamnar på de vanliga vapenplatserna (välj vilken per tillägg, eller
plats 9 och 0) och visas på vapenhjulet och i HUD:en. Eld och alternativ
eld är dina vanliga attackknappar.

| Tillägg | Eld | Alternativ eld | Ammunition |
|---|---|---|---|
| **Portalpistol** | Placerar den blå portalen | Placerar den orange portalen | Ingen |
| **Railgun** | Genomborrande projektil som träffar allt i en linje | — | 2 celler |
| **Änterhake** | Fastnar och drar dig till trä, metall, sten eller fiender | — | Ingen |
| **Klisterbombkastare** | Håll för att ladda, släpp för att avfyra | Detonera | 1 raket |
| **Bumerang** | Håll för att markera fiender, släpp för att kasta; bedövar var och en i tur | Rensa markeringar | Ingen |
| **Gelspruta** | Blå studsgel (studsig) | Orange fartgel (snabb) | Ingen |
| **Högtryckstvätt** | Lång vattenstråle som knuffar fiender och tvättar bort gel | Bred solfjäder av strålar | Ingen |

Portaler placeras på plana ytor i världen och för dig, monster och kastade
föremål igenom med bibehållen rörelsemängd. Med **Se genom portaler** på
ser du andra sidan levande, även portaler som syns genom portaler; kvalitet,
antal levande vyer, avstånd och rekursionsdjup kan justeras.

### Utrustning

Utrustning slås på och av i stället för att hållas i handen. Öppna
**utrustningshjulet** (`+equipwheel`), eller använd `goggles cycle` för att
stega igenom:

- **Mörkerseende**
- **Värmeseende**

## Flerspelare

Förutom NetQuake-spel över LAN och internet (protokoll 15, 666 och 999)
ansluter porten till **QuakeWorld**-servrar. Multiplayer → Join Game tar en
adress eller söker i LAN. FTE:s skriptade partiklar stöds för moddar som
använder dem.

## Kommandorad och konsol

Vanliga alternativ: `-basedir <mapp>`, `-game <mapp>`, `-hipnotic` /
`-rogue` / `-quoth`, `-width <b>`, `-height <h>`, `-fullscreen`, `-window`
och `+kommando argument` för att köra konsolkommandon vid start. Konsolen
(`~`) tar emot de vanliga Quake-kommandona och cvar-variablerna.

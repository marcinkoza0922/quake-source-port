# Handleiding

Deze handleiding beschrijft hoe je Quake speelt met deze port, met de nadruk
op wat hij toevoegt ten opzichte van vanilla Quake en andere source ports.
Alles wat nieuw is, is optioneel: met de standaardinstellingen ziet het spel
eruit en speelt het als Quake.

## Aan de slag

1. Zorg voor je eigen exemplaar van Quake. Wijs de port de map aan die
   `id1/` bevat (en eventuele mods ernaast), met `QUAKE_BASEDIR` of
   `-basedir <dir>`.
2. Voer `cargo run --release` uit, of start meteen een mod met
   `-game <dir> +map <map>`.
3. Configuratie, saves en screenshots staan in
   `~/.local/share/quake-source-port/<game>/`, één map per mod.

Het hoofdmenu bevat Single Player, Multiplayer, Options, **Help** (deze
handleiding), Quit en **Mappakketten**, met het Quake-thema op de
achtergrond (Options → Audio → Muziek hoofdmenu). Elk menu werkt met muis,
toetsenbord, controller en aanraking; ga met de muis over een optie of
selecteer hem om een tooltip met uitleg te zien.

In Help kies je met omhoog/omlaag een sectie; Page Up/Page Down,
links/rechts, de schouderknoppen of het muiswiel boven de tekst scrollen
erdoorheen.

## Taal

Options → Gameplay → **Taal** kiest de taal van de menu's, de berichten in
beeld en deze handleiding: Engels, Frans, Duits, Italiaans, Spaans, Russisch,
Pools, Oekraïens, Tsjechisch, Slowaaks, Kroatisch, Bulgaars, Zweeds, Fins,
Deens, Noors, Nederlands, Grieks, Hongaars, Roemeens, Portugees (Portugal of
Brazilië), Japans, Koreaans, Chinees (vereenvoudigd of traditioneel), Turks,
Indonesisch, Maleis, Filipijns, Vietnamees, Servisch, Litouws, Lets, Estisch,
Maltees, Albanees, Armeens, Georgisch, IJslands, Iers, Welsh, Baskisch,
Catalaans, Galicisch, Latijn, Esperanto, Belarussisch, Macedonisch, Sloveens,
Swahili, Arabisch, Hebreeuws, Perzisch, Urdu, Hindi, Marathi, Bengaals,
Punjabi, Gujarati, Tamil, Telugu, Thai, Khmer, Amhaars of Kantonees.
**Systeem** volgt de taal van je computer. De eigen speltekst van de
heruitgave uit 2021 is alleen vertaald in het Frans, Duits, Italiaans,
Spaans en Russisch; andere talen tonen die in het Engels. In andere talen
dan Engels gebruikt de kleine tekst in het spel het menulettertype, omdat
het klassieke Quake-lettertype alleen Latijnse letters zonder accenten
heeft. Consoleberichten blijven in het Engels, tenzij **Console vertalen**
aan staat.

## Mappakketten (modbrowser)

Hoofdmenu → Mappakketten toont elke geïnstalleerde mod en elk mappakket
met een beschrijving. Kies er een om ernaar over te schakelen, blader door
de maps en start er meteen een. De browser houdt per map je beste kills,
geheimen en tijd bij. Elke mod heeft zijn eigen configuratie, dus
toewijzingen en instellingen die je voor de ene mod afstemt, lekken niet
door naar een andere.

**Waar mappakketten horen.** De port zoekt ze in de Quake-basismap (de map
met `id1/`, ingesteld met `QUAKE_BASEDIR` of `-basedir`). Elke submap daarin
die speldata bevat (een `.pak`- of `.pk3`-bestand, een `progs.dat` of een
map `maps/`) wordt getoond; installeer een mappakket dus door het in een
eigen map naast `id1/` uit te pakken, bijvoorbeeld `<basedir>/ad/` of
`<basedir>/my_maps/maps/*.bsp`. Voeg eventueel een `modinfo.txt` toe met
regels `title`, `base` en `description` om te bepalen hoe het wordt
getoond. Pakketten worden gevonden bij het starten van het spel, dus start
opnieuw nadat je er een hebt toegevoegd. Saves, screenshots en configuratie
van elk pakket staan in `~/.local/share/quake-source-port/<folder>/`, niet
in de map van het pakket.

## Gesplitst scherm

Tot zes spelers op één computer, in co-op of deathmatch.

- Multiplayer → New Game → **Lokale spelers** bepaalt hoeveel mensen er
  spelen.
- Elke speler krijgt een eigen beeld, HUD, geluid en invoerapparaat
  (toetsenbord en muis voor speler 1, een controller voor elk van de
  anderen).
- De schermindeling kan gestapeld, naast elkaar, 2×2 of 3×2 zijn
  (`r_splitscreen_layout`).
- Elke speler kiest onder Multiplayer → Setup een naam en de kleuren van
  shirt en broek.

## Controllers

De port is ontworpen om met een gamepad te spelen.

- **Wapenwiel** (`+weaponwheel`): houd ingedrukt om de tijd te vertragen en
  kies een wapen met een van beide sticks of met de muis; laat los om te
  wisselen.
- **Richthulp** (Options → Invoer → Controller): vertraging bij vijanden en
  rotatie die ze volgt. Het geldt alleen voor de stick, nooit voor de muis.
- **Gyro-richten** (Options → Invoer → Controller → Gyro): een schakelaar,
  plus aparte gevoeligheid voor de yaw-, pitch- en rolas (rol staat
  standaard uit en draait het beeld zoals yaw, negatieve waarden keren het
  om), een ruisfilter, en wanneer de gyro geldt (altijd, uit of alleen
  zolang `+gyroaction` wordt vastgehouden). **Richtstijl** schakelt de
  rechterstick tussen 2D-richten (het beeld draait traploos) en
  **flick-stick** (duw de stick om het beeld meteen naar die richting te
  draaien, draai daarna de stick om het te roteren); beide combineren goed
  met de gyro voor nauwkeurig richten.
- **Trillen**, stickgevoeligheid en omkeren.
- **Knopsymbolen** passen bij je controller (Xbox, PlayStation, Nintendo)
  en worden automatisch herkend.
- Wijs alles opnieuw toe in Options → Besturing, ook vanaf de controller.
- Aanraakbediening met instelbare grootte, dekking en kijksnelheid.

## Opslaan

- **Autosaves** rouleren los van handmatige saves en quicksaves, zodat ze
  nooit je eigen saves overschrijven. Options → Gameplay →
  Autosave-instellingen bepaalt hoeveel er bewaard blijven en wanneer er
  wordt opgeslagen: bij levelstart, op een timer, bij het vinden van een
  geheim of alle geheimen, en bij het doden van alle vijanden.
- **Savekaarten** in de menu's Load/Save tonen van elke save het level,
  de gezondheid, het pantser, de wapens, de sleutels en kills/geheimen.
- **Bij dood** (Options → Gameplay → Bij dood) kan singleplayer in een
  gepauzeerd menu vragen wat je wilt doen, de meest recente save laden, je
  een save laten kiezen of het level herstarten.

## Gameplay en camera

Options → Gameplay:

- Gezichtsveld 60–130, hoofdbeweging, wapenterugslag, altijd rennen.
- **Camera**: eerste persoon, derde persoon, of over de schouder (rechts
  of links).
- Positie van de **wapenmodellen**: midden, rechts, links of verborgen.

## Aanpassen: spelmodifiers en add-ons

Een spel is *vanilla* of *aangepast*; dat kies je bij het starten. New Game
(en een level kiezen in Level kiezen of Mappakketten) toont een pop-up:
**Spelen** start het spel, **Ervaring aanpassen** opent Spelmodifiers en
Add-ons. Zolang er iets gewijzigd is, krijgt Spelen het label **AANGEPAST**
en zet **Alle aanpassingen uitschakelen** (Del, gamepad X of klik) de
standaardregels zonder add-ons terug. Vanilla spellen slaan gewone
`.sav`-bestanden op die ironwail en vkQuake kunnen laden; aangepaste
spellen slaan `.qsa`-bestanden op die alleen deze port laadt (net als
spellen op gesplitst scherm en co-op, die andere ports niet kunnen
opslaan). Tijdens een aangepast spel wijzigt Hoofdmenu → Aanpassen de
modifiers en add-ons op elk moment; de regels van een vanilla spel liggen
vast. Modifiers werken met elke mod en worden in deathmatch uitgeschakeld.

- Moeilijker: meer ontvangen schade, taaiere vijanden, minder munitie- en
  gezondheidspickups, 2–4× monsters, pistoolstart in elk level, geen
  handmatig opslaan.
- Makkelijker: minder ontvangen schade, zwakkere vijanden, meer munitie en
  gezondheid, oneindige munitie, gezondheidsherstel, geen valschade, geen
  verdrinking, geen eigen schade, dubbele of driedubbele sprongen.

## Demo's

`record <name>` en `stop` nemen een demo op in je gebruikersmap; `playdemo
<name>` speelt hem af. Als de opname stopt, wordt de demo op de achtergrond
herschreven zodat ironwail en vkQuake hem ook kunnen afspelen. Gebruikt een
mod deeltjeseffecten die andere ports niet kunnen tonen, dan blijft de
volledige versie bewaard als `<name>.qsd` (`playdemo <name>.qsd`).
`demo_convert <name>` zet een oudere demo om.

Hoofdmenu → Demo's toont je opgeslagen demo's en, met Opties → Demo's →
Demo's automatisch opnemen aan, een automatische demo van elk gespeeld
level; de oudste worden vervangen als de plaatsen vol zijn. Typ om de
lijsten te filteren en druk op Tab om de sortering te wijzigen; de
knoppen naast de lijst spelen, exporteren, hernoemen, bewaren of
verwijderen de gekozen demo, en een hernoemde automatische demo wordt
bewaard. Foutrapporten bevatten de automatische demo van het huidige
level.

Tijdens een demo pauzeert Spatie, Links of Rechts ingedrukt houden spoelt
terug of vooruit, Omhoog en Omlaag wijzigen de snelheid, PgUp en PgDn
springen 10 seconden, en Home en End gaan naar het begin en naar de volgende
map. I en O markeren een fragment; E exporteert het, of de hele demo, naar
de map `videos`. De bediening blijft in beeld; H of Verbergen verbergt hem,
en wijzen naar de onderrand toont een strook die hem terugbrengt. Klik op de
knoppen, klik op de tijdlijn om daarheen te springen, of sleep eroverheen om
een fragment te markeren; Exporteren vraagt naar beeldsnelheid, grootte,
formaat en bestand, en of het gemarkeerde fragment of de hele demo wordt
geëxporteerd.

Op een controller pauzeert A, de d-pad spoelt terug en vooruit en wijzigt de
snelheid, LB en RB springen 10 seconden en de stickknoppen gaan naar het
begin en naar de volgende map. LT en RT markeren een fragment, B wist het, X
exporteert, Y bewaart een automatische demo en View verbergt de bediening.

`capture` start en stopt een video-opname terwijl je speelt. Beeldsnelheid,
formaat en kwaliteit staan onder Opties → Demo's → Video-opname. Video's
hebben ffmpeg nodig; zonder worden de beelden als PNG-afbeeldingen met een
WAV-bestand opgeslagen.

## Toegankelijkheid

Options → Toegankelijkheid:

- Beperkte beweging en sterkte van schermflitsen.
- Kleurenblindheidsfilters.
- Instelbaar vizier: vorm, grootte, rand en kleurwissel op een doelwit of
  bij een kill; optionele hitmarkers.
- **Geluidsondertitels**; positie, grootte en duur van berichten en
  ondertitels stel je in de HUD-editor in.
- Aanvallen en rennen aan/uit (in plaats van vasthouden).
- Een schuifregelaar voor spelsnelheid om het hele spel te vertragen.
- Auto-richten met een lock-on-indicator en instellingen voor kegel en
  bereik.

## Verbeteringen

Options → Verbeteringen. Alles werkt aan de clientkant en staat standaard
uit, zodat het met elke mod en online werkt.

- **Puin**: tuimelende gibs en scherven van kapotte vaten, ook bij
  monsters uit mods; het kan tegen deuren en liften stoten.
- **Verbeterde bloedspatten**: spatten op oppervlakken, wonden die lichamen
  volgen, druppels in de lucht die openspatten waar ze landen.
- **Inslageffecten**: hagelinslagen van de shotgun, spijkers die in hout en
  vijanden blijven steken, schroeiplekken van explosies.
- **Verbeterd water**: doorschijnendheid, rimpelingen, spetters en
  caustieken.
- **Doorzichtige teleporters** die hun bestemming met parallax tonen.
- Ambient occlusion, bewegingsonscherpte per object en bij snelheid,
  zachte deeltjes.
- **Schermshaders**: RetroArch-voorinstellingen `.slangp` (CRT en andere).

Options → Graphics heeft ook de gebruikelijke renderschaal, anti-aliasing,
supersampling, texturefilter, 8-bit kleur met dithering, mist, gekleurde
verlichting en interpolatie van animatie en beweging.

## Add-ons

Add-ons zijn nieuwe wapens en uitrusting, aangestuurd door de engine,
zodat ze met de progs van elke mod werken. Ze zijn er alleen voor
singleplayer en co-op.

### Een spel met add-ons starten

Een spel heeft alleen add-ons als het ermee begint:

1. Single Player → vink het vakje **Add-ons** aan, dan New Game.
2. Kies welke add-ons je meeneemt en pas hun instellingen aan, en start
   dan het nieuwe spel.

Spellen met add-ons worden opgeslagen als `.qsa`-bestanden in plaats van
`.sav`, zodat andere ports ze niet proberen te laden. Savekaarten tonen een
badge "ADD-ONS". Een normale save laden schakelt add-ons uit. Tijdens een
spel met add-ons schakelt de pagina Add-ons (console:
`menu_options addons`) afzonderlijke add-ons in en uit; elk heeft een
herhalend voorbeeld, instructies, zijn toewijzingen en eigen instellingen.

### Wapens

Add-ons komen in de gewone wapenslots (per add-on te kiezen, of slot 9 en
0) en verschijnen in het wapenwiel en de HUD. Vuren en alt. vuur zijn je
gewone aanvalsknoppen.

| Add-on | Vuren | Alt. vuur | Munitie |
|---|---|---|---|
| **Portaalkanon** | Plaatst het blauwe portaal | Plaatst het oranje portaal | Geen |
| **Railgun** | Doorborend projectiel dat alles op een lijn raakt | — | 2 cellen |
| **Enterhaak** | Grijpt zich vast aan hout, metaal, steen of vijanden en trekt je ernaartoe | — | Geen |
| **Kleefbomwerper** | Vasthouden om op te laden, loslaten om te lanceren | Laten ontploffen | 1 raket |
| **Boemerang** | Vasthouden om vijanden te markeren, loslaten om te werpen; verdooft ze een voor een | Markeringen wissen | Geen |
| **Gelspuit** | Blauwe afstotingsgel (verend) | Oranje voortstuwingsgel (snel) | Geen |
| **Hogedrukspuit** | Lange waterstraal die vijanden wegduwt en gel wegspoelt | Brede waaier van stralen | Geen |

Portalen komen op vlakke oppervlakken van de wereld en brengen jou,
monsters en geworpen voorwerpen met hun vaart erdoorheen. Met **Door
portalen kijken** aan zie je de andere kant live, ook portalen die door
portalen zichtbaar zijn; kwaliteit, aantal live beelden, afstand en
recursiediepte zijn instelbaar.

### Uitrusting

Uitrusting zet je aan en uit in plaats van dat je hem vasthoudt. Open het
**uitrustingswiel** (`+equipwheel`), of gebruik `goggles cycle` om te
wisselen tussen:

- **Nachtzicht**
- **Warmtebeeld**

## Multiplayer

Naast NetQuake via LAN en internet (protocollen 15, 666 en 999) maakt de
port verbinding met **QuakeWorld**-servers. Multiplayer → Join Game neemt
een adres aan of doorzoekt het LAN. Gescripte FTE-deeltjes worden
ondersteund voor mods die ze gebruiken.

## Opdrachtregel en console

Veelgebruikte opties: `-basedir <dir>`, `-game <dir>`, `-hipnotic` /
`-rogue` / `-quoth`, `-width <w>`, `-height <h>`, `-fullscreen`,
`-window`, en `+command args` om bij het opstarten consolecommando's uit te
voeren. De console (`~`) accepteert de gebruikelijke Quake-commando's en
cvars.

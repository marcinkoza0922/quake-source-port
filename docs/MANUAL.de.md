# Benutzerhandbuch

Dieses Handbuch beschreibt, wie man Quake mit diesem Port spielt, vor allem
das, was er gegenüber dem originalen Quake und anderen Source-Ports
hinzufügt. Alles Neue ist optional: mit den Standardeinstellungen sieht das
Spiel aus und spielt sich wie Quake.

## Erste Schritte

1. Besorge dir dein eigenes Exemplar von Quake. Gib dem Port den Ordner an,
   der `id1/` enthält (und die Mods daneben), mit `QUAKE_BASEDIR` oder
   `-basedir <Ordner>`.
2. Starte `cargo run --release`, oder starte direkt einen Mod mit
   `-game <Ordner> +map <Karte>`.
3. Konfiguration, Spielstände und Bildschirmfotos liegen in
   `~/.local/share/quake-source-port/<Spiel>/`, ein Ordner pro Mod.

Das Hauptmenü bietet Single Player, Multiplayer, Options, **Help** (dieses
Handbuch), Quit und **Kartenpakete**, im Hintergrund läuft das Quake-Thema
(Options → Audio → Hauptmenümusik). Jedes Menü funktioniert mit Maus,
Tastatur, Controller und Touch; fahre über eine Option oder wähle sie aus,
um eine erklärende Kurzinfo zu sehen.

In der Hilfe wählt oben/unten einen Abschnitt; Bild auf/Bild ab,
links/rechts, die Schultertasten oder das Mausrad über dem Text scrollen
ihn.

## Sprache

Options → Gameplay → **Sprache** wählt die Sprache der Menüs, der
Bildschirmnachrichten und dieses Handbuchs: Englisch, Französisch, Deutsch,
Italienisch, Spanisch, Russisch, Polnisch, Ukrainisch, Tschechisch,
Slowakisch, Kroatisch, Bulgarisch, Schwedisch, Finnisch, Dänisch,
Norwegisch, Niederländisch, Griechisch, Ungarisch, Rumänisch, Portugiesisch
(Portugal oder Brasilien), Japanisch, Koreanisch, Chinesisch (vereinfacht
oder traditionell), Türkisch, Indonesisch, Malaiisch, Filipino,
Vietnamesisch, Serbisch, Litauisch, Lettisch, Estnisch, Maltesisch,
Albanisch, Armenisch, Georgisch, Isländisch, Irisch, Walisisch, Baskisch, Katalanisch, Galicisch, Latein,
Esperanto, Belarussisch, Mazedonisch, Slowenisch, Suaheli, Arabisch,
Hebräisch, Persisch, Urdu, Hindi, Marathi, Bengalisch, Punjabi, Gujarati,
Tamil, Telugu, Thai, Khmer, Amharisch oder Kantonesisch. **System** folgt der Sprache deines
Computers. Die Spieltexte
der Neuauflage von 2021 gibt es nur auf Französisch, Deutsch, Italienisch,
Spanisch und Russisch; andere Sprachen zeigen sie auf Englisch. In anderen
Sprachen als Englisch nutzen die kleinen Spieltexte die Menüschrift, da die
klassische Quake-Schrift nur lateinische Buchstaben ohne Akzente kennt.
Konsolenmeldungen bleiben auf Englisch, außer **Konsole übersetzen** ist an.

## Kartenpakete (Mod-Browser)

Hauptmenü → Kartenpakete listet jeden installierten Mod und jedes
Kartenpaket mit Beschreibung. Wähle eines, um dorthin zu wechseln, und
durchsuche dann seine Karten und starte jede direkt. Der Browser merkt sich
pro Karte deine besten Abschüsse, Geheimnisse und Zeit. Jeder Mod hat seine
eigene Konfiguration, sodass Belegungen und Einstellungen für einen Mod
nicht in einen anderen durchsickern.

**Wohin Kartenpakete gehören.** Der Port sucht sie im Quake-Basisordner
(der mit `id1/`, festgelegt mit `QUAKE_BASEDIR` oder `-basedir`). Jeder
Unterordner darin, der Spieldaten enthält (eine `.pak`- oder `.pk3`-Datei,
eine `progs.dat` oder einen Ordner `maps/`), wird gelistet. Installiere ein
Kartenpaket also, indem du es in einen eigenen Ordner neben `id1/`
entpackst, zum Beispiel `<basedir>/ad/` oder `<basedir>/my_maps/maps/*.bsp`.
Optional legt eine `modinfo.txt` mit den Zeilen `title`, `base` und
`description` fest, wie es angezeigt wird. Pakete werden beim Spielstart
gefunden, also nach dem Hinzufügen neu starten. Spielstände,
Bildschirmfotos und Konfiguration jedes Pakets liegen in
`~/.local/share/quake-source-port/<Ordner>/`, nicht im Paketordner.

## Splitscreen

Bis zu sechs Spieler an einem Rechner, kooperativ oder im Deathmatch.

- Multiplayer → New Game → **Lokale Spieler** legt fest, wie viele
  mitspielen.
- Jeder Spieler hat eigene Sicht, eigenes HUD, eigenen Ton und eigenes
  Eingabegerät (Tastatur und Maus für Spieler 1, je ein Controller für die
  anderen).
- Die Aufteilung kann übereinander, nebeneinander, 2×2 oder 3×2 sein
  (`r_splitscreen_layout`).
- Jeder Spieler wählt unter Multiplayer → Setup einen Namen und die Farben
  von Shirt und Hose.

## Controller

Der Port ist dafür gemacht, mit einem Gamepad gespielt zu werden.

- **Waffenrad** (`+weaponwheel`): halten, um die Zeit zu verlangsamen und
  mit einem der Sticks oder der Maus eine Waffe zu wählen; loslassen zum
  Wechseln.
- **Zielhilfe** (Options → Eingabe → Controller): Verlangsamung in der Nähe
  von Gegnern und Drehung, die ihnen folgt. Sie wirkt nur auf den Stick,
  nie auf die Maus.
- **Gyro-Zielen** (Options → Eingabe → Controller → Gyro): ein Schalter,
  dazu getrennte Empfindlichkeiten für Gier-, Nick- und Rollachse (Rollen
  ist standardmäßig aus und dreht die Sicht wie Gieren, negative Werte
  kehren es um), ein Rauschfilter und wann Gyro gilt (immer, aus oder nur
  solange `+gyroaction` gehalten wird). **Zielstil** schaltet den rechten
  Stick zwischen 2D-Zielen (er dreht die Sicht stufenlos) und
  **Flick-Stick** (Stick drücken richtet die Sicht sofort in diese Richtung
  aus, danach dreht ein Drehen des Sticks sie weiter); beide lassen sich
  gut mit Gyro für feines Zielen kombinieren.
- **Vibration**, Stick-Empfindlichkeit und -Umkehrung.
- **Tastensymbole** passen zu deinem Gamepad (Xbox, PlayStation, Nintendo)
  und werden automatisch erkannt.
- Alles lässt sich unter Options → Steuerung neu belegen, auch vom
  Controller aus.
- Touch-Steuerung mit einstellbarer Größe, Deckkraft und
  Blickgeschwindigkeit.

## Speichern

- **Automatische Spielstände** rotieren getrennt von manuellen Spielständen
  und Schnellspeicherständen und überschreiben deine eigenen daher nie.
  Options → Gameplay → Autospeichern-Einstellungen legt fest, wie viele
  behalten werden und wann gespeichert wird: bei Levelbeginn, im
  Zeitintervall, beim Finden eines oder aller Geheimnisse und wenn alle
  Gegner besiegt sind.
- **Spielstandkarten** in den Lade- und Speichermenüs zeigen Level,
  Gesundheit, Rüstung, Waffen, Schlüssel und Abschüsse/Geheimnisse jedes
  Spielstands.
- **Beim Tod** (Options → Gameplay → Beim Tod) kann das Einzelspiel in
  einem pausierten Menü fragen, was zu tun ist, den neuesten Spielstand
  laden, dich einen Spielstand wählen lassen oder das Level neu starten.

## Gameplay und Kamera

Options → Gameplay:

- Sichtfeld 60–130, Kamerawippen, Waffenrückstoß, immer rennen.
- **Kamera**: Egoperspektive, Verfolgerperspektive oder über der Schulter
  (rechts oder links).
- Position des **Waffenmodells**: Mitte, rechts, links oder ausgeblendet.

## Anpassen: Modifikatoren und Add-ons

Ein Spiel ist entweder *standard* oder *angepasst*, festgelegt beim Start.
New Game (und die Wahl eines Levels in Levelauswahl oder Kartenpakete)
zeigt ein Fenster: **Spielen** startet das Spiel, **Spielerlebnis
anpassen** öffnet Spielmodifikatoren und Add-ons. Sobald etwas geändert
ist, trägt Spielen die Markierung **ANGEPASST**, und **Alle Änderungen
deaktivieren** (Entf, Gamepad X oder Klick) stellt die Standardregeln ohne
Add-ons wieder her. Standardspiele speichern gewöhnliche `.sav`-Dateien, die
ironwail und vkQuake laden können; angepasste Spiele speichern `.qsa`-
Dateien, die nur dieser Port lädt (ebenso Splitscreen- und Koop-Spiele, die
andere Ports nicht speichern können). Während eines angepassten Spiels
ändert Hauptmenü → Anpassen seine Modifikatoren und Add-ons jederzeit; die
Regeln eines Standardspiels bleiben fest. Modifikatoren funktionieren mit
jedem Mod und schalten sich im Deathmatch ab.

- Schwerer: mehr erlittener Schaden, zähere Gegner, weniger Munition und
  Gesundheit aus Funden, 2–4× Monster, Pistolenstart in jedem Level, kein
  manuelles Speichern.
- Leichter: weniger erlittener Schaden, schwächere Gegner, mehr Munition und
  Gesundheit, unendlich Munition, Gesundheitsregeneration, kein
  Fallschaden, kein Ertrinken, kein Eigenschaden, Doppel- oder
  Dreifachsprünge.

## Demos

`record <Name>` und `stop` nehmen eine Demo in deinem Benutzerordner auf;
`playdemo <Name>` spielt sie ab. Nach dem Ende der Aufnahme wird die Demo im
Hintergrund umgeschrieben, damit auch ironwail und vkQuake sie abspielen
können. Nutzt ein Mod Partikeleffekte, die andere Ports nicht darstellen
können, bleibt die vollständige Fassung als `<Name>.qsd` erhalten
(`playdemo <Name>.qsd`). `demo_convert <Name>` wandelt eine ältere Demo um.

Hauptmenü → Demos listet deine gespeicherten Demos und, wenn Optionen →
Demos → Demos automatisch aufnehmen an ist, eine automatische Demo jedes
gespielten Levels; die ältesten werden ersetzt, wenn alle Plätze belegt
sind. K behält eine, E exportiert sie als Video und Entf löscht sie.
Fehlerberichte enthalten die automatische Demo des aktuellen Levels.

Während eine Demo läuft, pausiert die Leertaste, Halten von Links oder
Rechts spult zurück oder vor, Hoch und Runter ändern das Tempo, Bild↑ und
Bild↓ springen 10 Sekunden, Pos1 und Ende gehen zum Anfang und zur nächsten
Karte. I und O markieren einen Clip; E exportiert ihn, oder die ganze Demo,
in den Ordner `videos`. Die Steuerung bleibt sichtbar; H oder Ausblenden
blendet sie aus, und der Mauszeiger am unteren Rand zeigt eine Leiste, die
sie zurückholt. Klicke auf die Schaltflächen, klicke auf die Zeitleiste, um
dorthin zu springen, oder ziehe darüber, um einen Clip zu markieren;
Exportieren fragt nach Bildrate, Größe, Format und Datei und ob der
markierte Clip oder die ganze Demo exportiert wird.

`capture` startet und stoppt eine Videoaufnahme beim Spielen. Bildrate,
Format und Qualität findest du unter Optionen → Demos → Videoaufnahme.
Videos brauchen ffmpeg; ohne werden die Bilder als PNG mit einer WAV-Datei
gespeichert.

## Barrierefreiheit

Options → Barrierefreiheit:

- Reduzierte Bewegung und Stärke der Bildschirmblitze.
- Filter für Farbenblindheit.
- Einstellbares Fadenkreuz: Form, Größe, Umriss und Farbwechsel auf einem
  Ziel oder bei einem Abschuss; optionale Trefferanzeigen.
- **Geräusch-Untertitel** und Nachrichteneinstellungen (Nachrichten &
  Untertitel), Textgröße.
- Angriff und Rennen umschaltbar (statt halten).
- Ein Spieltempo-Regler, um das ganze Spiel zu verlangsamen.
- Automatisches Zielen mit Erfassungsanzeige sowie Kegel- und
  Reichweiteneinstellungen.

## Verbesserungen

Options → Verbesserungen. Alles läuft clientseitig und ist standardmäßig
aus, funktioniert also mit jedem Mod und online.

- **Trümmer**: rollende Fleischfetzen und Splitter zerbrochener Fässer, auch
  bei Mod-Monstern; sie können gegen Türen und Aufzüge stoßen.
- **Verbessertes Blut**: Spritzer auf Oberflächen, Wunden, die Körpern
  folgen, Tropfen in der Luft, die dort aufklatschen, wo sie landen.
- **Einschlageffekte**: Schrotkugel-Einschüsse, Nägel, die in Holz und
  Gegnern stecken bleiben, Explosions-Brandspuren.
- **Verbessertes Wasser**: Durchsichtigkeit, Wellen, Spritzer und
  Kaustiken.
- **Durchsichtige Teleporter**, die ihr Ziel mit Parallaxe zeigen.
- Umgebungsverdeckung, Bewegungsunschärfe pro Objekt und bei
  Geschwindigkeit, weiche Partikel.
- **Bildschirm-Shader**: RetroArch-Voreinstellungen `.slangp` (CRT und
  andere).

Options → Grafik bietet außerdem die üblichen Einstellungen:
Renderskalierung, Kantenglättung, Supersampling, Texturfilter, 8-Bit-Farben
mit Dithering, Nebel, farbiges Licht sowie Interpolation von Animation und
Bewegung.

## Add-ons

Add-ons sind neue Waffen und Ausrüstung, die von der Engine gesteuert
werden und daher mit den progs jedes Mods funktionieren. Sie gibt es nur im
Einzelspieler- und Koop-Modus.

### Ein Spiel mit Add-ons starten

Ein Spiel hat nur dann Add-ons, wenn es mit ihnen startet:

1. Single Player → das Kästchen **Add-ons** ankreuzen, dann New Game.
2. Wähle, welche Add-ons du mitnimmst, passe ihre Einstellungen an und
   starte dann das neue Spiel.

Spiele mit Add-ons speichern als `.qsa`-Dateien statt `.sav`, damit andere
Ports sie nicht zu laden versuchen. Spielstandkarten zeigen ein Abzeichen
„ADD-ONS“. Das Laden eines normalen Spielstands schaltet Add-ons ab.
Während eines Spiels mit Add-ons schaltet die Seite Add-ons (Konsole:
`menu_options addons`) einzelne Add-ons ein und aus; jedes hat eine
Vorschau in Schleife, eine Anleitung, seine Belegungen und eigene
Einstellungen.

### Waffen

Add-ons belegen die normalen Waffenplätze (pro Add-on wählbar, oder Platz 9
und 0) und erscheinen im Waffenrad und im HUD. Feuer und Alternativfeuer
sind deine üblichen Angriffstasten.

| Add-on | Feuer | Alternativfeuer | Munition |
|---|---|---|---|
| **Portalkanone** | Setzt das blaue Portal | Setzt das orange Portal | Keine |
| **Railgun** | Durchschlagendes Geschoss, das alles auf einer Linie trifft | — | 2 Zellen |
| **Enterhaken** | Hakt sich an Holz, Metall, Stein oder Gegnern fest und zieht dich hin | — | Keine |
| **Haftbombenwerfer** | Halten zum Aufladen, loslassen zum Abschießen | Zünden | 1 Rakete |
| **Bumerang** | Halten, um Gegner zu markieren, loslassen zum Werfen; betäubt jeden nacheinander | Markierungen löschen | Keine |
| **Gelsprüher** | Blaues Abstoßungsgel (federnd) | Oranges Antriebsgel (schnell) | Keine |
| **Hochdruckreiniger** | Langer Wasserstrahl, der Gegner wegdrückt und Gel abwäscht | Breiter Strahlenfächer | Keine |

Portale haften an flachen Oberflächen der Welt und befördern dich, Monster
und geworfene Gegenstände mit ihrem Schwung hindurch. Mit **Durch Portale
sehen** siehst du die andere Seite live, auch Portale, die durch Portale
sichtbar sind; Qualität, Zahl der Live-Ansichten, Entfernung und
Rekursionstiefe sind einstellbar.

### Ausrüstung

Ausrüstung wird ein- und ausgeschaltet statt in der Hand gehalten. Öffne
das **Ausrüstungsrad** (`+equipwheel`) oder nutze `goggles cycle`, um
durchzuschalten:

- **Nachtsicht**
- **Wärmebild**

## Mehrspieler

Neben NetQuake-Spielen im LAN und im Internet (Protokolle 15, 666 und 999)
verbindet sich der Port mit **QuakeWorld**-Servern. Multiplayer → Join Game
nimmt eine Adresse an oder sucht im LAN. Skriptgesteuerte FTE-Partikel
werden für Mods unterstützt, die sie nutzen.

## Kommandozeile und Konsole

Häufige Optionen: `-basedir <Ordner>`, `-game <Ordner>`, `-hipnotic` /
`-rogue` / `-quoth`, `-width <B>`, `-height <H>`, `-fullscreen`, `-window`
und `+Befehl Argumente`, um beim Start Konsolenbefehle auszuführen. Die
Konsole (`~`) akzeptiert die üblichen Quake-Befehle und -Variablen.

# Používateľská príručka

Táto príručka opisuje hranie Quake s týmto portom, najmä to, čo pridáva
oproti pôvodnému Quake a iným portom. Všetko nové je voliteľné: s
predvolenými nastaveniami hra vyzerá a hrá sa ako Quake.

## Začíname

1. Zaobstarajte si vlastnú kópiu Quake. Nasmerujte port na priečinok, ktorý
   obsahuje `id1/` (a mody vedľa neho), pomocou `QUAKE_BASEDIR` alebo
   `-basedir <priečinok>`.
2. Spustite `cargo run --release`, alebo rovno spustite mod pomocou
   `-game <priečinok> +map <mapa>`.
3. Konfigurácia, uložené hry a snímky obrazovky sú v
   `~/.local/share/quake-source-port/<hra>/`, jeden priečinok pre každý mod.

Hlavné menu obsahuje Single Player, Multiplayer, Options, **Help** (túto
príručku), Quit a **Balíky máp** a v pozadí hrá téma Quake
(Options → Zvuk → Hudba v hlavnom menu). Každé menu funguje s myšou,
klávesnicou, ovládačom aj dotykom; ukážte na možnosť alebo ju vyberte a
zobrazí sa popis, ktorý ju vysvetľuje.

V pomocníkovi šípky hore/dole vyberajú sekciu; Page Up/Page Down,
vľavo/vpravo, ramenné tlačidlá alebo koliesko myši nad textom ho posúvajú.

## Jazyk

Options → Hrateľnosť → **Jazyk** určuje jazyk menu, správ na obrazovke a tejto
príručky: angličtina, francúzština, nemčina, taliančina, španielčina, ruština,
poľština, ukrajinčina, čeština, slovenčina, chorvátčina, bulharčina, švédčina,
fínčina, dánčina, nórčina, holandčina, gréčtina, maďarčina, rumunčina,
portugalčina (Portugalsko alebo Brazília), japončina, kórejčina, čínština
(zjednodušená alebo tradičná), turečtina, indonézština, malajčina, filipínčina,
vietnamčina, srbčina, litovčina, lotyština, estónčina, maltčina, albánčina,
arménčina, gruzínčina, islandčina, írčina, waleština, baskičtina, katalánčina,
galícijčina, latinčina, esperanto, bieloruština, macedónčina, slovinčina,
swahilčina, arabčina, hebrejčina, perzština, urdčina, hindčina, maráthčina,
bengálčina, pandžábčina, gudžarátčina, tamilčina, telugčina, thajčina,
khmérčina, amharčina alebo kantončina.
**Systém** sa riadi jazykom vášho počítača. Vlastné herné texty reedície
2021 sú preložené iba do francúzštiny, nemčiny, taliančiny, španielčiny a
ruštiny; ostatné jazyky ich zobrazujú v angličtine. V iných jazykoch ako v
angličtine používa malý text v hre písmo menu, pretože klasické písmo
Quake má iba latinské písmená bez diakritiky. Správy konzoly zostávajú v
angličtine, pokiaľ nie je zapnuté **Prekladať konzolu**.

## Balíky máp (prehliadač modov)

Hlavné menu → Balíky máp zobrazí každý nainštalovaný mod a balík máp s
popisom. Vyberte jeden, prepnete sa naň, potom prehliadajte jeho mapy a
ktorúkoľvek priamo spustite. Prehliadač si pre každú mapu pamätá vaše
najlepšie zabitia, tajomstvá a čas. Každý mod má vlastnú konfiguráciu,
takže priradenia a nastavenia vyladené pre jeden mod neprenikajú do iného.

**Kam patria balíky máp.** Port ich hľadá v základnom priečinku Quake (v
tom, ktorý obsahuje `id1/`, nastavenom cez `QUAKE_BASEDIR` alebo
`-basedir`). Zobrazí sa každý jeho podpriečinok, ktorý obsahuje herné dáta
(súbor `.pak` alebo `.pk3`, `progs.dat` alebo priečinok `maps/`), takže
balík máp nainštalujete rozbalením do vlastného priečinka vedľa `id1/`,
napríklad `<basedir>/ad/` alebo `<basedir>/my_maps/maps/*.bsp`. Voliteľne
pridajte `modinfo.txt` s riadkami `title`, `base` a `description`, ktoré
určujú, ako sa zobrazí. Balíky sa hľadajú pri štarte hry, takže po pridaní
nového hru reštartujte. Uložené hry, snímky obrazovky a konfigurácia
každého balíka sú v `~/.local/share/quake-source-port/<priečinok>/`, nie v
priečinku balíka.

## Rozdelená obrazovka

Až šesť hráčov na jednom počítači, v kooperácii alebo deathmatchi.

- Multiplayer → New Game → **Lokálni hráči** určuje, koľko ľudí hrá.
- Každý hráč má vlastný pohľad, HUD, zvuk a vstupné zariadenie (klávesnicu
  a myš hráč 1, ostatní po jednom ovládači).
- Rozloženie obrazovky môže byť pod sebou, vedľa seba, 2×2 alebo 3×2
  (`r_splitscreen_layout`).
- Každý hráč si zvolí meno a farby trička a nohavíc v Multiplayer →
  Setup.

## Ovládače

Port je navrhnutý na hranie s gamepadom.

- **Koleso zbraní** (`+weaponwheel`): podržte, čas sa spomalí, a vyberte
  zbraň ktoroukoľvek páčkou alebo myšou; pustením ju vezmete do ruky.
- **Asistencia mierenia** (Options → Vstup → Ovládač): spomalenie pri
  nepriateľoch a otáčanie, ktoré ich sleduje. Platí iba pre páčku, nikdy
  pre myš.
- **Mierenie gyroskopom** (Options → Vstup → Ovládač → Gyroskop):
  prepínač, plus samostatná citlivosť pre osi otáčania, sklonu a náklonu
  (náklon je predvolene vypnutý a otáča pohľad ako os otáčania, záporné
  hodnoty ho obracajú), filter šumu a kedy gyroskop platí (vždy, vypnutý
  alebo iba pri držaní `+gyroaction`). **Štýl mierenia** prepína pravú
  páčku medzi 2D mierením (otáča pohľad plynulo) a **flick stick**
  (zatlačením páčky sa pohľad ihneď otočí daným smerom, otáčaním páčky sa
  potom točí ďalej); obe sa dobre kombinujú s gyroskopom na jemné mierenie.
- **Vibrácie**, citlivosť a obrátenie páčok.
- **Symboly tlačidiel** zodpovedajú vášmu ovládaču (Xbox, PlayStation,
  Nintendo) a rozpoznajú sa automaticky.
- Všetko preradíte v Options → Ovládanie, aj priamo z ovládača.
- Dotykové ovládanie s nastaviteľnou veľkosťou, nepriehľadnosťou a
  rýchlosťou pohľadu.

## Ukladanie

- **Automatické uloženia** sa striedajú oddelene od ručných a rýchlych
  uložení, takže nikdy neprepíšu tie vaše. Options → Hrateľnosť →
  Nastavenia automatického ukladania určuje, koľko ich ponechať a kedy
  ukladať: pri vstupe do úrovne, podľa časovača, pri nájdení tajomstva
  alebo všetkých tajomstiev a po zabití všetkých nepriateľov.
- **Karty uložených hier** v menu Load/Save ukazujú úroveň, zdravie,
  brnenie, zbrane, kľúče a zabitia/tajomstvá každého uloženia.
- **Pri smrti** (Options → Hrateľnosť → Pri smrti) sa hra pre jedného
  hráča môže opýtať, čo robiť, v pozastavenom menu, načítať posledné
  uloženie, nechať vás vybrať uloženú hru alebo reštartovať úroveň.

## Hrateľnosť a kamera

Options → Hrateľnosť:

- Zorné pole 60–130, pohupovanie pohľadu, spätný ráz zbrane, vždy bežať.
- **Kamera**: z prvej osoby, z tretej osoby alebo cez rameno (vpravo
  alebo vľavo).
- Pozícia **modelu zbrane**: v strede, vpravo, vľavo alebo skrytý.

## Prispôsobenie: modifikátory hry a doplnky

Hra je buď *štandardná*, alebo *prispôsobená*, a to sa volí pri jej
štarte. Nová hra (aj výber úrovne vo Výbere úrovne alebo v Balíkoch máp)
zobrazí okno: **Hrať** spustí hru, **Prispôsobiť zážitok** otvorí
Modifikátory hry a Doplnky. Kým je niečo zmenené, Hrať má značku
**VLASTNÉ** a **Vypnúť všetky úpravy** (Del, X na gamepade alebo klik)
vráti štandardné pravidlá bez doplnkov. Štandardné hry ukladajú bežné
súbory `.sav`, ktoré načítajú ironwail a vkQuake; prispôsobené hry
ukladajú súbory `.qsa`, ktoré načíta iba tento port (rovnako hry na
rozdelenej obrazovke a v kooperácii, ktoré iné porty uložiť nedokážu).
Počas prispôsobenej hry môžete v Hlavné menu → Prispôsobiť kedykoľvek
zmeniť jej modifikátory a doplnky; pravidlá štandardnej hry zostávajú
pevné. Modifikátory fungujú s každým modom a v deathmatchi sa vypnú.

- Ťažšie: väčšie utrpené poškodenie, odolnejší nepriatelia, menej
  munície a zdravia z predmetov, 2–4x monštrá, štart od nuly v každej
  úrovni, bez ručného ukladania.
- Ľahšie: menšie utrpené poškodenie, slabší nepriatelia, viac munície a
  zdravia, nekonečná munícia, regenerácia zdravia, bez poškodenia pádom,
  bez topenia, bez vlastného poškodenia, dvojité alebo trojité skoky.

## Dema

`record <názov>` a `stop` nahrajú demo do vášho používateľského
priečinka; `playdemo <názov>` ho prehrá. Keď nahrávanie skončí, demo sa na
pozadí prepíše tak, aby ho prehrali aj ironwail a vkQuake. Ak mod používa
efekty častíc, ktoré iné porty nedokážu zobraziť, úplná verzia sa ponechá
ako `<názov>.qsd` (`playdemo <názov>.qsd`). `demo_convert <názov>`
prevedie staršie demo.

Hlavné menu → Demá ukazuje uložené demá a so zapnutým Možnosti → Demá →
Automaticky nahrávať demá aj automatické demo každého odohraného levelu;
najstaršie sa nahrádzajú, keď sú sloty plné. Písaním filtrujete zoznamy a
Tab mení zoradenie; tlačidlá vedľa zoznamu prehrajú, exportujú, premenujú,
ponechajú alebo zmažú vybrané demo a premenovaním sa automatické demo
ponechá. Hlásenia chýb obsahujú automatické demo aktuálneho levelu.

Počas dema Medzerník pozastaví, držanie Vľavo alebo Vpravo pretáča späť
alebo vpred, Hore a Dole menia rýchlosť, PgUp a PgDn skáču o 10 sekúnd a
Home a End idú na začiatok a na ďalšiu mapu. I a O označia klip; E ho, alebo
celé demo, exportuje do priečinka `videos`. Ovládanie zostáva na obrazovke;
H alebo Skryť ho skryje a ukázaním na spodný okraj sa objaví pás, ktorý ho
vráti. Klikajte na tlačidlá, kliknutím na časovú os na ňu skočíte a ťahaním
po nej označíte klip; Exportovať sa spýta na snímkovú frekvenciu, veľkosť,
formát a súbor a na to, či exportovať označený klip, alebo celé demo.

Na ovládači A pozastaví, smerový ovládač pretáča dozadu aj dopredu a mení
rýchlosť, LB a RB skáču o 10 sekúnd a stlačenia páčok idú na začiatok a na
ďalšiu mapu. LT a RT označia klip, B ho zruší, X exportuje, Y ponechá
automatické demo a View skryje ovládanie.

`capture` spúšťa a zastavuje nahrávanie videa počas hrania. Snímková
frekvencia, formát a kvalita sú v Možnosti → Demá → Nahrávanie videa. Videá
vyžadujú ffmpeg; bez neho sa snímky uložia ako obrázky PNG so súborom WAV.

## Prístupnosť

Options → Prístupnosť:

- Obmedzenie pohybu a intenzita zábleskov obrazovky.
- Filtre pre farbosleposť.
- Nastaviteľný zameriavač: tvar, veľkosť, obrys a zmena farby na cieli
  alebo pri zabití; voliteľné značky zásahu.
- **Titulky zvukov**; pozícia, veľkosť a čas zobrazenia správ a titulkov sa nastavujú v editore HUD.
- Prepínanie (namiesto držania) útoku a behu.
- Posuvník rýchlosti hry na spomalenie celej hry.
- Automatické mierenie s indikátorom zamerania a nastavením kužeľa a
  dosahu.

## Vylepšenia

Options → Vylepšenia. Všetko beží na strane klienta a je predvolene
vypnuté, takže funguje s každým modom aj online.

- **Trosky**: kotúľajúce sa kusy tiel a šrapnely z rozbitých sudov, aj pri
  monštrách z modov; môžu narážať do dverí a výťahov.
- **Vylepšená krv**: striekance na povrchoch, rany, ktoré sledujú telá, a
  kvapky vo vzduchu, ktoré sa rozprsknú tam, kde dopadnú.
- **Efekty zásahov**: stopy po brokoch, klince, ktoré uviaznu v dreve a v
  nepriateľoch, stopy po výbuchoch.
- **Vylepšená voda**: priesvitnosť, vlnenie, špliechanie a kaustiky.
- **Priehľadné teleporty**, ktoré ukazujú svoj cieľ s paralaxou.
- Ambientná oklúzia, rozmazanie pohybu objektov a rýchlosťou, mäkké
  častice.
- **Shadery obrazovky**: predvoľby RetroArch `.slangp` (CRT a ďalšie).

Options → Grafika ponúka aj bežnú mierku vykresľovania, vyhladzovanie
hrán, supersampling, filtrovanie textúr, 8-bitové farby s ditheringom,
hmlu, farebné osvetlenie a interpoláciu animácií a pohybu.

## Doplnky

Doplnky sú nové zbrane a vybavenie, ktoré obsluhuje engine, takže fungujú
s progs ľubovoľného modu. Sú iba pre jedného hráča a kooperáciu.

### Spustenie hry s doplnkami

Hra má doplnky, iba ak s nimi začne:

1. Single Player → zaškrtnite políčko **Doplnky**, potom New Game.
2. Vyberte, ktoré doplnky si vezmete, upravte ich nastavenia a potom
   spustite novú hru.

Hry s doplnkami sa ukladajú ako súbory `.qsa` namiesto `.sav`, takže iné
porty sa ich nepokúsia načítať. Karty uložených hier ukazujú odznak
„ADD-ONS“. Načítanie bežného uloženia doplnky vypne. Počas hry s doplnkami
stránka Doplnky (konzola: `menu_options addons`) zapína a vypína
jednotlivé doplnky; každý má opakujúcu sa ukážku, pokyny, svoje priradenia
a vlastné nastavenia.

### Zbrane

Doplnky sa vkladajú do bežných slotov zbraní (slot sa volí pre každý
doplnok, alebo slot 9 a 0) a zobrazujú sa na kolese zbraní aj v HUD.
Streľba a alternatívna streľba sú vaše bežné tlačidlá útoku.

| Doplnok | Streľba | Alternatívna streľba | Munícia |
|---|---|---|---|
| **Portálová zbraň** | Umiestni modrý portál | Umiestni oranžový portál | Žiadna |
| **Railgun** | Prestreľujúci projektil, ktorý zasiahne všetko v línii | — | 2 články |
| **Hák** | Zachytí sa na dreve, kove, kameni či nepriateľoch a pritiahne vás | — | Žiadna |
| **Vrhač lepivých bômb** | Podržaním nabite, pustením vystreľte | Odpáliť | 1 raketa |
| **Bumerang** | Podržaním označte nepriateľov, pustením hoďte; postupne ich omráči | Zrušiť označenie | Žiadna |
| **Rozprašovač gélu** | Modrý odrazový gél (pruží) | Oranžový zrýchľovací gél (rýchly) | Žiadna |
| **Tlakový čistič** | Dlhý prúd vody, ktorý odtláča nepriateľov a zmýva gél | Široký vejár prúdov | Žiadna |

Portály sa umiestňujú na rovné povrchy sveta a prenesú vás, monštrá aj
hodené predmety aj s ich hybnosťou. So zapnutým **Vidieť cez portály**
vidíte druhú stranu naživo, vrátane portálov viditeľných cez portály;
kvalitu, počet živých pohľadov, vzdialenosť a hĺbku rekurzie možno
nastaviť.

### Vybavenie

Vybavenie sa zapína a vypína, nedrží sa v ruke. Otvorte **koleso
vybavenia** (`+equipwheel`), alebo použite `goggles cycle` na prepínanie:

- **Nočné videnie**
- **Termovízia**

## Hra viacerých hráčov

Okrem hry NetQuake cez LAN a internet (protokoly 15, 666 a 999) sa port
pripája aj k serverom **QuakeWorld**. Multiplayer → Join Game prijme
adresu alebo prehľadá LAN. Skriptované častice FTE sú podporované pre
mody, ktoré ich používajú.

## Príkazový riadok a konzola

Bežné voľby: `-basedir <priečinok>`, `-game <priečinok>`, `-hipnotic` /
`-rogue` / `-quoth`, `-width <š>`, `-height <v>`, `-fullscreen`,
`-window` a `+príkaz argumenty` na spustenie príkazov konzoly pri štarte.
Konzola (`~`) prijíma bežné príkazy a premenné Quake.

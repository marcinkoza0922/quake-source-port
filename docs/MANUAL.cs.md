# Uživatelská příručka

Tato příručka popisuje hraní Quake s tímto portem, hlavně to, co přidává
oproti původnímu Quake a jiným source portům. Všechno nové je volitelné:
s výchozím nastavením hra vypadá i hraje jako Quake.

## Začínáme

1. Pořiďte si vlastní kopii Quake. Nasměrujte port na složku, která
   obsahuje `id1/` (a vedle ní případné mody), pomocí `QUAKE_BASEDIR` nebo
   `-basedir <složka>`.
2. Spusťte `cargo run --release`, nebo rovnou spusťte mod pomocí
   `-game <složka> +map <mapa>`.
3. Konfigurace, uložené pozice a snímky obrazovky jsou ve složce
   `~/.local/share/quake-source-port/<hra>/`, jedna složka pro každý mod.

Hlavní nabídka obsahuje Single Player, Multiplayer, Options, **Help** (tuto
příručku), Quit a **Balíčky map** a v pozadí hraje motiv Quake
(Options → Zvuk → Hudba hlavní nabídky). Každá nabídka funguje s myší,
klávesnicí, ovladačem i dotykem; najeďte na volbu nebo ji vyberte a zobrazí
se nápověda, která ji vysvětluje.

V nápovědě vybíráte sekci šipkami nahoru/dolů; Page Up/Page Down,
vlevo/vpravo, ramenní tlačítka nebo kolečko myši nad textem jím posouvají.

## Jazyk

Options → Hratelnost → **Jazyk** volí jazyk nabídek, zpráv na obrazovce a této
příručky: angličtina, francouzština, němčina, italština, španělština, ruština,
polština, ukrajinština, čeština, slovenština, chorvatština, bulharština,
švédština, finština, dánština, norština, nizozemština, řečtina, maďarština,
rumunština, portugalština (Portugalsko nebo Brazílie), japonština, korejština,
čínština (zjednodušená nebo tradiční), turečtina, indonéština, malajština,
filipínština, vietnamština, srbština, litevština, lotyština, estonština,
maltština, albánština, arménština, gruzínština, islandština, irština, velština,
baskičtina, katalánština, galicijština, latina, esperanto, běloruština,
makedonština, slovinština, svahilština, arabština, hebrejština, perština,
urdština, hindština, maráthština, bengálština, paňdžábština, gudžarátština,
tamilština, telugština, thajština, khmerština, amharština nebo kantonština.
**Systém** se řídí jazykem vašeho počítače. Vlastní herní texty remasteru
z roku 2021 jsou přeloženy jen do francouzštiny, němčiny, italštiny,
španělštiny a ruštiny; ostatní jazyky je zobrazují anglicky. V jiných
jazycích než angličtině používá drobný text ve hře písmo nabídek, protože
klasické písmo Quake má jen latinková písmena bez diakritiky. Zprávy
konzole zůstávají anglicky, pokud není zapnuté **Překládat konzoli**.

## Balíčky map (prohlížeč modů)

Hlavní nabídka → Balíčky map vypíše všechny nainstalované mody a balíčky
map s jejich popisem. Vyberte jeden a přepněte se na něj, pak procházejte
jeho mapy a kteroukoli rovnou spusťte. Prohlížeč si u každé mapy pamatuje
vaše nejlepší zabití, tajemství a čas. Každý mod má vlastní konfiguraci,
takže přiřazení kláves a nastavení vyladěná pro jeden mod se nepřenesou do
jiného.

**Kam patří balíčky map.** Port je hledá v základní složce Quake (té, která
obsahuje `id1/`, nastavené pomocí `QUAKE_BASEDIR` nebo `-basedir`). Vypíše
se každá podsložka, která obsahuje herní data (soubor `.pak` nebo `.pk3`,
`progs.dat` nebo složku `maps/`), takže balíček map nainstalujete tak, že
ho rozbalíte do vlastní složky vedle `id1/`, například `<basedir>/ad/` nebo
`<basedir>/my_maps/maps/*.bsp`. Volitelně přidejte `modinfo.txt` s řádky
`title`, `base` a `description`, které určují, jak se balíček zobrazí.
Balíčky se hledají při spuštění hry, takže po přidání nového hru
restartujte. Uložené pozice, snímky obrazovky a konfigurace každého balíčku
jsou ve složce `~/.local/share/quake-source-port/<složka>/`, ne ve složce
balíčku.

## Rozdělená obrazovka

Až šest hráčů na jednom počítači, v kooperaci nebo v deathmatchi.

- Multiplayer → New Game → **Místní hráči** určuje, kolik lidí hraje.
- Každý hráč má vlastní pohled, HUD, zvuk a vstupní zařízení (klávesnici
  a myš pro hráče 1, po jednom ovladači pro ostatní).
- Rozvržení obrazovky může být nad sebou, vedle sebe, 2×2 nebo 3×2
  (`r_splitscreen_layout`).
- Každý hráč si v Multiplayer → Setup zvolí jméno a barvu trička a kalhot.

## Ovladače

Port je navržený pro hraní na gamepadu.

- **Kolo zbraní** (`+weaponwheel`): podržte pro zpomalení času a vyberte
  zbraň kteroukoli páčkou nebo myší; puštěním ji přepnete.
- **Asistence míření** (Options → Vstup → Ovladač): zpomalení u nepřátel
  a otáčení, které je sleduje. Platí vždy jen pro páčku, nikdy pro myš.
- **Míření gyroskopem** (Options → Vstup → Ovladač → Gyroskop): vypínač,
  samostatná citlivost pro osy otáčení, sklonu a náklonu (náklon je ve
  výchozím stavu vypnutý a otáčí pohledem jako osa otáčení, záporné hodnoty
  ho obrátí), filtr šumu a volba, kdy gyroskop platí (vždy, vypnuto nebo
  jen při držení `+gyroaction`). **Styl míření** přepíná pravou páčku mezi
  2D mířením (plynule otáčí pohledem) a **flick stickem** (zatlačením páčky
  natočíte pohled okamžitě tím směrem, otáčením páčky ho pak dál otáčíte);
  obojí se dobře kombinuje s gyroskopem pro přesné míření.
- **Vibrace**, citlivost a obrácení páček.
- **Symboly tlačítek** odpovídají vašemu ovladači (Xbox, PlayStation,
  Nintendo) a rozpoznají se automaticky.
- Vše lze přemapovat v Options → Ovládání, i přímo z ovladače.
- Dotykové ovládání s nastavitelnou velikostí, krytím a rychlostí
  rozhlížení.

## Ukládání

- **Automatická uložení** se střídají odděleně od ručních a rychlých
  uložení, takže nikdy nepřepíšou ta vaše. Options → Hratelnost → Nastavení
  automatického ukládání určuje, kolik jich uchovat a kdy ukládat: při
  vstupu do úrovně, podle časovače, po nalezení tajemství nebo všech
  tajemství a po zabití všech nepřátel.
- **Karty uložených pozic** v nabídkách načtení a uložení ukazují u každé
  pozice úroveň, zdraví, brnění, zbraně, klíče a zabití/tajemství.
- **Při smrti** (Options → Hratelnost → Při smrti) se hra pro jednoho hráče
  může zeptat v pozastavené nabídce, co dělat, načíst poslední uloženou
  pozici, nechat vás vybrat uloženou pozici, nebo restartovat úroveň.

## Hratelnost a kamera

Options → Hratelnost:

- Zorné pole 60–130, pohupování pohledu, zpětný ráz zbraně, vždy běžet.
- **Kamera**: z vlastního pohledu, z pohledu třetí osoby nebo přes rameno
  (pravé nebo levé).
- Pozice **modelů zbraní**: uprostřed, vpravo, vlevo nebo skryté.

## Přizpůsobení: modifikátory hry a doplňky

Hra je buď *původní*, nebo *přizpůsobená*, což se volí při jejím spuštění.
Nová hra (i výběr úrovně ve Výběru úrovně nebo v Balíčcích map) zobrazí
okno: **Hrát** spustí hru, **Přizpůsobit zážitek** otevře Modifikátory hry
a Doplňky. Dokud je cokoli změněno, nese Hrát označení **VLASTNÍ**
a **Vypnout všechny úpravy** (Del, X na gamepadu nebo kliknutí) vrátí
standardní pravidla bez doplňků. Původní hry ukládají běžné soubory `.sav`,
které načte ironwail i vkQuake; přizpůsobené hry ukládají soubory `.qsa`,
které načte jen tento port (stejně jako hry na rozdělené obrazovce
a kooperace, které jiné porty uložit neumějí). Během přizpůsobené hry lze
přes Hlavní nabídka → Přizpůsobit kdykoli měnit její modifikátory a doplňky;
pravidla původní hry zůstávají pevně daná. Modifikátory fungují s každým
modem a v deathmatchi se vypínají.

- Těžší: větší utrpěné poškození, odolnější nepřátelé, méně munice
  a zdraví k sebrání, 2–4× monster, start s pistolí v každé úrovni, bez
  ručního ukládání.
- Lehčí: menší utrpěné poškození, slabší nepřátelé, více munice a zdraví,
  nekonečná munice, regenerace zdraví, bez poškození pádem, bez topení,
  bez poškození sebe, dvojité nebo trojité skoky.

## Dema

`record <název>` a `stop` nahrají demo do vaší uživatelské složky;
`playdemo <název>` ho přehraje. Po skončení nahrávání se demo na pozadí
přepíše, aby ho mohl přehrát i ironwail a vkQuake. Pokud mod používá efekty
částic, které jiné porty neumějí zobrazit, uchová se úplná verze jako
`<název>.qsd` (`playdemo <název>.qsd`). `demo_convert <název>` převede starší
demo.

## Přístupnost

Options → Přístupnost:

- Omezení pohybu a intenzita záblesků obrazovky.
- Filtry pro barvoslepost.
- Nastavitelný zaměřovač: tvar, velikost, obrys a změna barvy na cíli nebo
  při zabití; volitelné značky zásahu.
- **Titulky zvuků** a nastavení zpráv (Zprávy a titulky), velikost textu.
- Přepínání (místo držení) útoku a běhu.
- Posuvník rychlosti hry pro zpomalení celé hry.
- Automatické míření s indikátorem zaměření, nastavením kužele a dosahu.

## Vylepšení

Options → Vylepšení. Vše běží na straně klienta a ve výchozím stavu je
vypnuté, takže to funguje s každým modem i online.

- **Trosky**: kutálející se kusy těl a šrapnely z rozbitých sudů, i u
  monster z modů; mohou narážet do dveří a výtahů.
- **Vylepšené cákance krve**: cákance na površích, zranění, která sledují
  těla, kapky ve vzduchu, které se rozprsknou tam, kam dopadnou.
- **Efekty zásahů**: stopy broků, hřebíky, které se zabodávají do dřeva
  a nepřátel, spáleniny po výbuších.
- **Vylepšená voda**: průsvitnost, vlnky, šplouchání a kaustika.
- **Průhledné teleporty**, které ukazují svůj cíl s paralaxou.
- Okolní zastínění, rozmazání pohybem u objektů i při rychlosti, měkké
  částice.
- **Shadery obrazovky**: předvolby RetroArch `.slangp` (CRT a další).

Options → Grafika nabízí i obvyklé měřítko vykreslování, vyhlazování hran,
supersampling, filtrování textur, 8bitové barvy s ditheringem, mlhu,
barevné osvětlení a interpolaci animací a pohybu.

## Doplňky

Doplňky jsou nové zbraně a vybavení, které obsluhuje engine, takže fungují
s progs jakéhokoli modu. Jsou jen pro hru jednoho hráče a kooperaci.

### Spuštění hry s doplňky

Hra má doplňky, jen pokud s nimi začala:

1. Single Player → zaškrtněte políčko **Doplňky**, pak New Game.
2. Vyberte, které doplňky si vezmete, upravte jejich nastavení a pak
   spusťte novou hru.

Hry s doplňky se ukládají jako soubory `.qsa` místo `.sav`, takže se je
jiné porty nepokusí načíst. Karty uložených pozic ukazují odznak
„ADD-ONS“. Načtení běžné uložené pozice doplňky vypne. Během hry s doplňky
stránka Doplňky (konzole: `menu_options addons`) zapíná a vypíná jednotlivé
doplňky; každý má smyčkovou ukázku, návod, svá přiřazení kláves a vlastní
nastavení.

### Zbraně

Doplňky se řadí do běžných slotů zbraní (pro každý doplněk zvolíte slot,
nebo slot 9 a 0) a objevují se v kole zbraní a v HUD. Střelba
a alternativní střelba jsou vaše obvyklá tlačítka útoku.

| Doplněk | Střelba | Alternativní střelba | Munice |
|---|---|---|---|
| **Portálová puška** | Umístí modrý portál | Umístí oranžový portál | Žádná |
| **Railgun** | Průrazná střela, která zasáhne vše v přímce | — | 2 články |
| **Hák** | Zachytí se na dřevě, kovu, kameni nebo nepřátelích a přitáhne vás | — | Žádná |
| **Vrhač lepicích bomb** | Podržením nabijete, puštěním vystřelíte | Odpálit | 1 raketa |
| **Bumerang** | Podržením označíte nepřátele, puštěním hodíte; postupně je omráčí | Zrušit označení | Žádná |
| **Rozprašovač gelu** | Modrý odpudivý gel (pružný) | Oranžový zrychlující gel (rychlý) | Žádná |
| **Tlakový čistič** | Dlouhý proud vody, který odtlačuje nepřátele a smývá gel | Široký vějíř proudů | Žádná |

Portály se umisťují na rovné povrchy světa a přenášejí vás, monstra
i hozené předměty i s jejich hybností. Se zapnutým **Vidět skrz portály**
vidíte druhou stranu živě, včetně portálů viditelných skrz portály;
kvalitu, počet živých pohledů, vzdálenost a hloubku rekurze lze nastavit.

### Vybavení

Vybavení se zapíná a vypíná, nedrží se v ruce. Otevřete **kolo vybavení**
(`+equipwheel`), nebo použijte `goggles cycle` k přepínání mezi:

- **Noční vidění**
- **Termovize**

## Multiplayer

Kromě hry NetQuake v LAN a po internetu (protokoly 15, 666 a 999) se port
připojí i k serverům **QuakeWorld**. Multiplayer → Join Game
přijme adresu nebo prohledá LAN. Skriptované částice FTE jsou podporované
pro mody, které je používají.

## Příkazová řádka a konzole

Běžné volby: `-basedir <složka>`, `-game <složka>`, `-hipnotic` / `-rogue` /
`-quoth`, `-width <š>`, `-height <v>`, `-fullscreen`, `-window` a
`+příkaz argumenty` ke spuštění příkazů konzole při startu. Konzole (`~`)
přijímá obvyklé příkazy a proměnné (cvars) Quake.

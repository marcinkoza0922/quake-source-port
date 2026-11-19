# Uporabniški priročnik

Ta priročnik opisuje igranje Quaka s tem portom, predvsem tisto, kar dodaja
izvirnemu Quaku in drugim portom izvorne kode. Vse novo je neobvezno: s
privzetimi nastavitvami je igra na pogled in pri igranju takšna kot Quake.

## Prvi koraki

1. Priskrbite si svoj izvod Quaka. Portu pokažite mapo, ki vsebuje `id1/`
   (in mode poleg nje), z `QUAKE_BASEDIR` ali `-basedir <mapa>`.
2. Zaženite `cargo run --release` ali takoj zaženite mod z
   `-game <mapa> +map <zemljevid>`.
3. Nastavitve, shranjene igre in zajemi zaslona so v
   `~/.local/share/quake-source-port/<igra>/`, ena mapa za vsak mod.

Glavni meni ponuja Single Player, Multiplayer, Options, **Help** (ta
priročnik), Quit in **Paketi zemljevidov**, v ozadju pa se predvaja tema
Quake (Options → Zvok → Glasba glavnega menija). Vsak meni deluje z miško,
tipkovnico, krmilnikom in na dotik; postavite kazalec na možnost ali jo
izberite, da vidite opis, ki jo pojasni.

V pomoči gor/dol izbere razdelek; Page Up/Page Down, levo/desno, ramenska
gumba ali kolesce miške nad besedilom ga drsijo.

## Jezik

Options → Igranje → **Jezik** izbere jezik menijev, sporočil na zaslonu in
tega priročnika: angleščina, francoščina, nemščina, italijanščina,
španščina, ruščina, poljščina, ukrajinščina, češčina, slovaščina,
hrvaščina, bolgarščina, švedščina, finščina, danščina, norveščina,
nizozemščina, grščina, madžarščina, romunščina, portugalščina (Portugalska
ali Brazilija), japonščina, korejščina, kitajščina (poenostavljena ali
tradicionalna), turščina, indonezijščina, malajščina, filipinščina,
vietnamščina, srbščina, litovščina, latvijščina, estonščina, malteščina,
albanščina, armenščina, gruzijščina, islandščina, irščina, valižanščina,
baskovščina, katalonščina, galicijščina, latinščina, esperanto,
beloruščina, makedonščina, slovenščina, svahili, arabščina, hebrejščina,
perzijščina, urdujščina, hindijščina, maratščina, bengalščina, pandžabščina,
gudžaratščina, tamilščina, telugijščina, tajščina, kmerščina, amharščina ali
kantonščina.
**Sistem** sledi jeziku vašega računalnika. Besedila igre iz izdaje 2021
so prevedena samo v francoščino, nemščino, italijanščino, španščino in
ruščino; drugi jeziki jih prikažejo v angleščini. V jezikih razen
angleščine drobno besedilo v igri uporablja pisavo menijev, ker ima
klasična pisava Quake le latinične črke brez strešic in naglasov. Sporočila
konzole ostanejo v angleščini, razen če je vklopljeno **Prevedi konzolo**.

## Paketi zemljevidov (brskalnik modov)

Glavni meni → Paketi zemljevidov navede vsak nameščen mod in paket
zemljevidov z opisom. Izberite enega, da preklopite nanj, nato prebrskajte
njegove zemljevide in kateregakoli neposredno zaženite. Brskalnik za vsak
zemljevid beleži vaše najboljše uboje, skrivnosti in čas. Vsak mod ima
lastne nastavitve, zato se dodelitve tipk in nastavitve, prilagojene
enemu modu, ne prenesejo v drugega.

**Kam gredo paketi zemljevidov.** Port jih išče v osnovni mapi Quaka (tisti
z `id1/`, nastavljeni z `QUAKE_BASEDIR` ali `-basedir`). Navedena je vsaka
podmapa v njej, ki vsebuje podatke igre (datoteko `.pak` ali `.pk3`,
`progs.dat` ali mapo `maps/`), zato paket zemljevidov namestite tako, da ga
razpakirate v lastno mapo poleg `id1/`, na primer `<basedir>/ad/` ali
`<basedir>/my_maps/maps/*.bsp`. Po želji dodajte `modinfo.txt` z vrsticami
`title`, `base` in `description`, s katerimi določite, kako je naveden.
Paketi se poiščejo ob zagonu igre, zato jo po dodajanju paketa znova
zaženite. Shranjene igre, zajemi zaslona in nastavitve vsakega paketa so v
`~/.local/share/quake-source-port/<mapa>/`, ne v mapi paketa.

## Deljen zaslon

Do šest igralcev na enem računalniku, v sodelovalni igri ali deathmatchu.

- Multiplayer → New Game → **Lokalni igralci** določi, koliko ljudi igra.
- Vsak igralec ima lasten pogled, HUD, zvok in vnosno napravo (tipkovnico
  in miško za igralca 1, po en krmilnik za ostale).
- Postavitev zaslona je lahko ena nad drugo, ena ob drugi, 2×2 ali 3×2
  (`r_splitscreen_layout`).
- Vsak igralec izbere ime ter barvi majice in hlač v Multiplayer → Setup.

## Krmilniki

Port je zasnovan za igranje s krmilnikom.

- **Kolo orožij** (`+weaponwheel`): držite, da upočasnite čas in izberete
  orožje s katerokoli palico ali miško; spustite za zamenjavo.
- **Pomoč pri merjenju** (Options → Vnos → Krmilnik): upočasnitev blizu
  sovražnikov in vrtenje, ki jim sledi. Vedno velja samo za palico, nikoli
  za miško.
- **Merjenje z žiroskopom** (Options → Vnos → Krmilnik → Žiroskop):
  stikalo, ločena občutljivost za osi zasuka, naklona in nagiba (nagib je
  privzeto izklopljen in obrača pogled kot zasuk, negativne vrednosti ga
  obrnejo), filter šuma ter kdaj žiroskop velja (vedno, izklopljeno ali
  samo med držanjem `+gyroaction`). **Slog merjenja** preklaplja desno
  palico med 2D-merjenjem (zvezno obrača pogled) in **flick stick** (potisk
  palice takoj usmeri pogled v to smer, vrtenje palice ga nato obrača);
  oboje se dobro dopolnjuje z žiroskopom za natančno merjenje.
- **Vibracije**, občutljivost in obrnitev palic.
- **Simboli gumbov** se ujemajo z vašim krmilnikom (Xbox, PlayStation,
  Nintendo) in se zaznajo samodejno.
- Vse lahko znova dodelite v Options → Kontrolniki, tudi s krmilnikom.
- Kontrolniki na zaslonu na dotik z nastavljivo velikostjo, prekrivnostjo
  in hitrostjo pogleda.

## Shranjevanje

- **Samodejna shranjevanja** se kroženjem menjujejo ločeno od ročnih in
  hitrih shranjevanj, zato nikoli ne prepišejo vaših. Options → Igranje →
  Nastavitve samodejnega shranjevanja izbere, koliko jih ohraniti in kdaj
  shranjevati: ob vstopu v stopnjo, v časovnih presledkih, ko najdete
  skrivnost ali vse skrivnosti in ko ubijete vse sovražnike.
- **Kartice shranjenih iger** v menijih za nalaganje in shranjevanje
  pokažejo stopnjo, zdravje, oklep, orožja, ključe ter uboje/skrivnosti
  vsake shranjene igre.
- **Ob smrti** (Options → Igranje → Ob smrti) lahko igra za enega igralca
  v zaustavljenem meniju vpraša, kaj storiti, naloži zadnjo shranjeno igro,
  vam pusti izbrati shranjeno igro ali znova začne stopnjo.

## Igranje in kamera

Options → Igranje:

- Vidno polje 60–130, zibanje pogleda, odsun orožja, vedno teci.
- **Kamera**: prvoosebno, tretjeosebno ali čez ramo (desno ali levo).
- Položaj **modela orožja**: sredina, desno, levo ali skrito.

## Prilagoditev: modifikatorji igre in dodatki

Igra je bodisi *izvirna* bodisi *prilagojena*, kar izberete ob njenem
začetku. New Game (in izbira stopnje v Izbira stopnje ali Paketi
zemljevidov) prikaže okno: **Igraj** začne igro, **Prilagodi izkušnjo**
odpre Modifikatorji igre in Dodatki. Dokler je kar koli spremenjeno, ima
Igraj oznako **PRILAGOJENO**, **Onemogoči vse spremembe** (Del, gumb X na
krmilniku ali klik) pa vrne standardna pravila brez dodatkov. Izvirne igre
shranjujejo običajne datoteke `.sav`, ki jih lahko naložita ironwail in
vkQuake; prilagojene igre shranjujejo datoteke `.qsa`, ki jih naloži samo ta
port (enako velja za igre na deljenem zaslonu in sodelovalne igre, ki jih
drugi porti ne morejo shraniti). Med prilagojeno igro Glavni meni →
Prilagoditev kadar koli spremeni njene modifikatorje in dodatke; pravila
izvirne igre ostanejo fiksna. Modifikatorji delujejo s katerimkoli modom in
se v deathmatchu izklopijo.

- Težje: več prejete škode, močnejši sovražniki, manj streliva in zdravja iz
  pobranih predmetov, 2–4× pošasti, začetek s pištolo v vsaki stopnji, brez
  ročnega shranjevanja.
- Lažje: manj prejete škode, šibkejši sovražniki, več streliva in zdravja,
  neskončno strelivo, obnavljanje zdravja, brez škode ob padcu, brez
  utapljanja, brez lastne škode, dvojni ali trojni skoki.

## Posnetki

`record <ime>` in `stop` posnameta posnetek v vašo uporabniško mapo;
`playdemo <ime>` ga predvaja. Ko se snemanje konča, se posnetek v ozadju
prepiše, da ga lahko predvajata tudi ironwail in vkQuake. Če mod uporablja
učinke delcev, ki jih drugi porti ne znajo prikazati, se celotna različica
ohrani kot `<ime>.qsd` (`playdemo <ime>.qsd`). `demo_convert <ime>`
pretvori starejši posnetek.

Glavni meni → Posnetki prikaže shranjene posnetke in, ko je vklopljeno
Možnosti → Posnetki → Samodejno snemaj posnetke, samodejni posnetek vsake
odigrane ravni; najstarejši se zamenjajo, ko so mesta polna. Tipkajte za
filtriranje seznamov, Tab pa spremeni razvrščanje; gumbi ob seznamu
predvajajo, izvozijo, preimenujejo, obdržijo ali izbrišejo izbrani
posnetek, preimenovan samodejni posnetek pa se obdrži. Poročila o napakah
vsebujejo samodejni posnetek trenutne ravni.

Med posnetkom preslednica ustavi, držanje leve ali desne puščice previja
nazaj ali naprej, gor in dol spreminjata hitrost, PgUp in PgDn skočita 10
sekund, Home in End pa gresta na začetek in na naslednji zemljevid. I in O
označita izsek; E izvozi njega ali cel posnetek v mapo `videos`. Kontrolniki
ostanejo na zaslonu; H ali Skrij jih skrije, kazanje na spodnji rob pa
prikaže trak, ki jih vrne. Kliknite gumbe, kliknite časovnico, da skočite
tja, ali povlecite čeznjo, da označite izsek; Izvozi vpraša za hitrost
sličic, velikost, obliko in datoteko ter ali naj izvozi označeni izsek ali
cel posnetek.

Na krmilniku A ustavi, smerni gumbi previjajo nazaj in naprej ter
spreminjajo hitrost, LB in RB skočita 10 sekund, pritiska palic pa skočita
na začetek in na naslednjo stopnjo. LT in RT označita izsek, B ga počisti, X
izvozi, Y obdrži samodejni posnetek, View pa skrije kontrolnike.

`capture` med igranjem začne in ustavi snemanje videa. Hitrost sličic,
oblika in kakovost so v Možnosti → Posnetki → Snemanje videa. Video
potrebuje ffmpeg; brez njega se sličice shranijo kot slike PNG z datoteko
WAV.

## Dostopnost

Options → Dostopnost:

- Zmanjšano gibanje in jakost bliskov zaslona.
- Filtri za barvno slepoto.
- Nastavljiv merek: oblika, velikost, obroba in sprememba barve na tarči ali
  ob uboju; neobvezne oznake zadetkov.
- **Podnapisi zvokov**; položaj, velikost in trajanje sporočil in podnapisov
  se nastavijo v urejevalniku HUD.
- Preklop (namesto držanja) za napad in tek.
- Drsnik hitrosti igre za upočasnitev celotne igre.
- Samodejno merjenje s kazalnikom zajema ter nastavitvami stožca in dosega.

## Izboljšave

Options → Izboljšave. Vse deluje na strani odjemalca in je privzeto
izklopljeno, zato deluje s katerimkoli modom in prek spleta.

- **Drobci**: kotaleči se kosi mesa in šrapneli iz razbitih sodov, tudi pri
  pošastih iz modov; lahko trkajo ob vrata in dvigala.
- **Izboljšana kri**: brizgi po površinah, rane, ki sledijo telesom, in
  kapljice v zraku, ki se razpršijo tam, kjer pristanejo.
- **Učinki udarcev**: sledi šiber, žeblji, ki se zapičijo v les in
  sovražnike, ožganine eksplozij.
- **Izboljšana voda**: prosojnost, valovi, pljuski in kavstika.
- **Prozorni teleporterji**, ki s paralakso pokažejo svoj cilj.
- Senčenje okolice, zameglitev gibanja posameznih predmetov in hitrosti,
  mehki delci.
- **Senčilniki zaslona**: prednastavitve RetroArch `.slangp` (CRT in
  drugi).

Options → Grafika ima še običajne nastavitve: merilo izrisa, glajenje
robov, supersampling, filter tekstur, 8-bitne barve z ditheringom, megla,
barvna osvetlitev ter interpolacija animacije in gibanja.

## Dodatki

Dodatki so nova orožja in oprema, ki jih poganja pogon, zato delujejo s
progs katerega koli moda. Na voljo so samo v igri za enega igralca in v
sodelovalni igri.

### Začetek igre z dodatki

Igra ima dodatke samo, če se z njimi začne:

1. Single Player → označite polje **Dodatki**, nato New Game.
2. Izberite, katere dodatke vzamete s seboj, prilagodite njihove
   nastavitve in nato začnite novo igro.

Igre z dodatki se shranijo kot datoteke `.qsa` namesto `.sav`, da jih drugi
porti ne poskušajo naložiti. Kartice shranjenih iger pokažejo značko
»ADD-ONS«. Nalaganje običajne shranjene igre izklopi dodatke. Med igro z
dodatki stran Dodatki (konzola: `menu_options addons`) posamezne dodatke
vklaplja in izklaplja; vsak ima predogled v zanki, navodila, svoje
dodelitve tipk in lastne nastavitve.

### Orožja

Dodatki zasedejo običajna mesta za orožje (izberete jih za vsak dodatek
posebej ali mesti 9 in 0) in se pojavijo na kolesu orožij in v HUD-u. Strel
in alternativni strel sta vaša običajna gumba za napad.

| Dodatek | Strel | Alternativni strel | Strelivo |
|---|---|---|---|
| **Portalna pištola** | Postavi modri portal | Postavi oranžni portal | Brez |
| **Railgun** | Prebojni izstrelek, ki zadene vse na črti | — | 2 celici |
| **Kavelj** | Zatakne se za les, kovino, kamen ali sovražnike in vas povleče tja | — | Brez |
| **Metalec lepljivih bomb** | Držite za polnjenje, spustite za izstrelitev | Sproži | 1 raketa |
| **Bumerang** | Držite za označevanje sovražnikov, spustite za met; omami enega za drugim | Počisti oznake | Brez |
| **Brizgalnik gela** | Modri odbojni gel (prožen) | Oranžni pospeševalni gel (hiter) | Brez |
| **Visokotlačni čistilnik** | Dolg vodni curek, ki odriva sovražnike in spira gel | Široka pahljača curkov | Brez |

Portali se postavijo na ravne površine sveta in vas, pošasti ter vržene
predmete prenesejo skozi z ohranjenim zagonom. Z vklopljenim **Pogled skozi
portale** vidite drugo stran v živo, tudi portale, vidne skozi portale;
kakovost, število živih pogledov, razdaljo in globino rekurzije je mogoče
nastaviti.

### Oprema

Opremo vklopite in izklopite, namesto da bi jo držali v roki. Odprite
**kolo opreme** (`+equipwheel`) ali uporabite `goggles cycle` za
preklapljanje:

- **Nočni vid**
- **Termovizija**

## Igra za več igralcev

Poleg igre NetQuake v LAN in prek interneta (protokoli 15, 666 in 999) se
port poveže tudi s strežniki **QuakeWorld**. Multiplayer → Join Game sprejme
naslov ali išče v LAN. Skriptni delci FTE so podprti za mode, ki jih
uporabljajo.

## Ukazna vrstica in konzola

Pogoste možnosti: `-basedir <mapa>`, `-game <mapa>`, `-hipnotic` /
`-rogue` / `-quoth`, `-width <š>`, `-height <v>`, `-fullscreen`, `-window`
in `+ukaz argumenti` za izvajanje ukazov konzole ob zagonu. Konzola (`~`)
sprejema običajne ukaze in spremenljivke Quaka.

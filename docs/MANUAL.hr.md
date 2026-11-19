# Korisnički priručnik

Ovaj priručnik opisuje igranje Quakea s ovim portom, s naglaskom na ono što
dodaje u odnosu na izvorni Quake i druge portove. Sve novo je neobavezno:
sa zadanim postavkama igra izgleda i igra se kao Quake.

## Prvi koraci

1. Nabavite vlastiti primjerak Quakea. Uputite port na direktorij koji
   sadrži `id1/` (i sve modove pokraj njega), pomoću `QUAKE_BASEDIR` ili
   `-basedir <dir>`.
2. Pokrenite `cargo run --release` ili odmah pokrenite mod s
   `-game <dir> +map <mapa>`.
3. Konfiguracija, spremanja i snimke zaslona nalaze se u
   `~/.local/share/quake-source-port/<igra>/`, jedna mapa po modu.

Glavni izbornik sadrži Single Player, Multiplayer, Options, **Help** (ovaj
priručnik), Quit i **Paketi mapa**, a u pozadini svira tema Quakea
(Options → Zvuk → Glazba glavnog izbornika). Svaki izbornik radi s mišem,
tipkovnicom, kontrolerom i dodirom; zadržite pokazivač iznad opcije ili je
odaberite da vidite opis koji je objašnjava.

U Pomoći gore/dolje bira odjeljak; Page Up/Page Down, lijevo/desno, tipke
na ramenima ili kotačić miša iznad teksta pomiču ga.

## Jezik

Options → Igrivost → **Jezik** bira jezik izbornika, poruka na zaslonu i ovog
priručnika: engleski, francuski, njemački, talijanski, španjolski, ruski,
poljski, ukrajinski, češki, slovački, hrvatski, bugarski, švedski, finski,
danski, norveški, nizozemski, grčki, mađarski, rumunjski, portugalski (Portugal
ili Brazil), japanski, korejski, kineski (pojednostavljeni ili tradicionalni),
turski, indonezijski, malajski, filipinski, vijetnamski, srpski, litavski,
latvijski, estonski, malteški, albanski, armenski, gruzijski, islandski, irski,
velški, baskijski, katalonski, galicijski, latinski, esperanto, bjeloruski,
makedonski, slovenski, svahili, arapski, hebrejski, perzijski, urdu, hindski,
marathski, bengalski, pandžapski, gudžaratski, tamilski, teluški, tajlandski,
kmerski, amharski ili kantonski.
**Sustav** prati jezik vašeg računala. Vlastiti tekst igre iz reizdanja iz
2021. preveden je samo na francuski, njemački, talijanski, španjolski i
ruski; ostali jezici prikazuju ga na engleskom. U jezicima osim engleskog
sitni tekst u igri koristi font izbornika, jer klasični font Quakea ima
samo latinična slova bez dijakritika. Poruke konzole ostaju na engleskom
osim ako je uključeno **Prevedi konzolu**.

## Paketi mapa (preglednik modova)

Glavni izbornik → Paketi mapa navodi svaki instalirani mod i paket mapa s
njegovim opisom. Odaberite jedan da se prebacite na njega, zatim pregledajte
njegove mape i izravno pokrenite bilo koju od njih. Preglednik pamti vaša
najbolja ubojstva, tajne i vrijeme po mapi. Svaki mod ima vlastitu
konfiguraciju, pa se dodjele tipki i postavke podešene za jedan mod ne
prelijevaju u drugi.

**Gdje idu paketi mapa.** Port ih traži u osnovnom direktoriju Quakea (onom
koji sadrži `id1/`, postavljenom s `QUAKE_BASEDIR` ili `-basedir`). Navodi
se svaki poddirektorij koji sadrži podatke igre (datoteku `.pak` ili `.pk3`,
`progs.dat` ili mapu `maps/`), pa paket mapa instalirate tako da ga
raspakirate u vlastitu mapu pokraj `id1/`, na primjer `<basedir>/ad/` ili
`<basedir>/my_maps/maps/*.bsp`. Po želji dodajte `modinfo.txt` s retcima
`title`, `base` i `description` kako biste odredili kako se navodi. Paketi
se pronalaze pri pokretanju igre, pa nakon dodavanja paketa ponovno
pokrenite igru. Spremanja, snimke zaslona i konfiguracija svakog paketa
nalaze se u `~/.local/share/quake-source-port/<mapa>/`, a ne u mapi paketa.

## Podijeljeni zaslon

Do šest igrača na jednom računalu, u suradnji ili deathmatchu.

- Multiplayer → New Game → **Lokalni igrači** određuje koliko ljudi igra.
- Svaki igrač dobiva vlastiti prikaz, HUD, zvuk i ulazni uređaj
  (tipkovnicu i miš za igrača 1, po jedan kontroler za ostale).
- Raspored zaslona može biti jedan iznad drugoga, jedan pokraj drugoga,
  2×2 ili 3×2 (`r_splitscreen_layout`).
- Svaki igrač bira ime i boje majice/hlača u Multiplayer → Setup.

## Kontroleri

Port je osmišljen za igranje na kontroleru.

- **Kotač oružja** (`+weaponwheel`): držite da usporite vrijeme i odaberete
  oružje bilo kojom palicom ili mišem; otpustite za promjenu.
- **Pomoć pri ciljanju** (Options → Ulaz → Kontroler): usporavanje blizu
  neprijatelja i rotacija koja ih prati. Uvijek se primjenjuje samo na
  palicu, nikad na miš.
- **Ciljanje žiroskopom** (Options → Ulaz → Kontroler → Žiroskop):
  prekidač, uz zasebnu osjetljivost za osi skretanja, nagiba i valjanja
  (valjanje je zadano isključeno i okreće pogled kao skretanje, negativne
  vrijednosti ga obrću), filtar šuma te kada se žiroskop primjenjuje
  (uvijek, isključeno ili samo dok se drži `+gyroaction`). **Stil
  ciljanja** prebacuje desnu palicu između 2D ciljanja (kontinuirano
  okreće pogled) i **flick sticka** (gurnite palicu da pogled odmah skoči
  u tom smjeru, zatim okrećite palicu da ga rotirate); oba se dobro
  kombiniraju sa žiroskopom za precizno ciljanje.
- **Vibracija**, osjetljivost i obrtanje palice.
- **Simboli tipki** odgovaraju vašem kontroleru (Xbox, PlayStation,
  Nintendo) i prepoznaju se automatski.
- Sve tipke možete ponovno dodijeliti u Options → Kontrole, i s kontrolera.
- Dodirne kontrole s podesivom veličinom, neprozirnošću i brzinom pogleda.

## Spremanje

- **Autospremanja** se izmjenjuju odvojeno od ručnih i brzih spremanja, pa
  nikad ne prepisuju vaša. Options → Igrivost → Postavke autospremanja
  određuje koliko ih se čuva i kada se sprema: pri ulasku u razinu, u
  intervalu, pri pronalasku tajne ili svih tajni te kad su svi neprijatelji
  ubijeni.
- **Kartice spremanja** u izbornicima Učitaj/Spremi prikazuju razinu,
  zdravlje, oklop, oružja, ključeve i ubojstva/tajne svakog spremanja.
- **Pri smrti** (Options → Igrivost → Pri smrti) igra za jednog igrača može
  u pauziranom izborniku pitati što učiniti, učitati najnovije spremanje,
  pustiti vas da odaberete spremanje ili ponovno pokrenuti razinu.

## Igrivost i kamera

Options → Igrivost:

- Vidno polje 60–130, ljuljanje pogleda, trzaj oružja, uvijek trči.
- **Kamera**: prvo lice, treće lice ili preko ramena (desnog ili lijevog).
- Položaj **modela oružja**: sredina, desno, lijevo ili skriven.

## Prilagodba: modifikatori igre i dodaci

Igra je ili *izvorna* ili *prilagođena*, što se bira pri pokretanju. New
Game (i odabir razine u Odabiru razine ili Paketima mapa) prikazuje skočni
prozor: **Igraj** pokreće igru, **Prilagodi iskustvo** otvara Modifikatore
igre i Dodatke. Dok je išta promijenjeno, Igraj nosi oznaku
**PRILAGOĐENO**, a **Onemogući sve izmjene** (Del, X na kontroleru ili klik)
vraća standardna pravila bez dodataka. Izvorne igre spremaju obične
datoteke `.sav` koje ironwail i vkQuake mogu učitati; prilagođene igre
spremaju datoteke `.qsa` koje učitava samo ovaj port (kao i igre na
podijeljenom zaslonu i u suradnji, koje drugi portovi ne mogu spremiti).
Tijekom prilagođene igre Glavni izbornik → Prilagodba u bilo kojem trenutku
mijenja njezine modifikatore i dodatke; pravila izvorne igre ostaju fiksna.
Modifikatori rade sa svakim modom i isključuju se u deathmatchu.

- Teže: više primljene štete, jači neprijatelji, manje streljiva i zdravlja
  iz pokupljenih predmeta, 2–4× čudovišta, start od nule u svakoj razini,
  bez ručnog spremanja.
- Lakše: manje primljene štete, slabiji neprijatelji, više streljiva i
  zdravlja, beskonačno streljivo, obnova zdravlja, bez štete od pada, bez
  utapanja, bez samoozljeđivanja, dvostruki ili trostruki skokovi.

## Demo snimke

`record <naziv>` i `stop` snimaju demo snimku u vašu korisničku mapu;
`playdemo <naziv>` je reproducira. Kad snimanje završi, demo snimka se u
pozadini prepisuje kako bi je i ironwail i vkQuake mogli reproducirati. Ako
mod koristi efekte čestica koje drugi portovi ne mogu prikazati, puna
inačica čuva se kao `<naziv>.qsd` (`playdemo <naziv>.qsd`).
`demo_convert <naziv>` pretvara stariju demo snimku.

Glavni izbornik → Dema prikazuje spremljena dema i, uz uključeno Opcije →
Dema → Automatski snimaj dema, automatski demo svake odigrane razine;
najstarija se zamjenjuju kad su mjesta puna. Tipkajte za filtriranje popisa,
a Tab mijenja poredak; gumbi pokraj popisa reproduciraju, izvoze,
preimenuju, zadržavaju ili brišu odabrani demo, a preimenovani automatski
demo se zadržava. Prijave grešaka sadrže automatski demo trenutne razine.

Tijekom dema Razmaknica pauzira, držanje Lijevo ili Desno premotava unatrag
ili naprijed, Gore i Dolje mijenjaju brzinu, PgUp i PgDn skaču 10 sekundi, a
Home i End idu na početak i na sljedeću kartu. I i O označavaju isječak; E
izvozi njega ili cijeli demo u mapu `videos`. Kontrole ostaju na zaslonu; H
ili Sakrij ih skrivaju, a pokazivanje na donji rub prikazuje traku koja ih
vraća. Kliknite gumbe, kliknite vremensku crtu da skočite onamo ili povucite
preko nje da označite isječak; Izvezi pita za broj sličica, veličinu, format
i datoteku te izvozi li se označeni isječak ili cijeli demo.

Na kontroleru A pauzira, križni gumb premotava natrag i naprijed i mijenja
brzinu, LB i RB skaču 10 sekundi, a pritisci palica idu na početak i na
sljedeću mapu. LT i RT označavaju isječak, B ga briše, X izvozi, Y zadržava
automatski demo, a View skriva kontrole.

`capture` pokreće i zaustavlja snimanje videa tijekom igre. Broj sličica,
format i kvaliteta su u Opcije → Dema → Snimanje videa. Videi trebaju
ffmpeg; bez njega se sličice spremaju kao PNG slike s WAV datotekom.

## Pristupačnost

Options → Pristupačnost:

- Smanjeni pokreti i jačina bljeskova zaslona.
- Filtri za sljepoću za boje.
- Podesiv nišan: oblik, veličina, obrub i promjena boje na meti ili pri
  ubojstvu; neobavezne oznake pogotka.
- **Titlovi zvukova** i postavke poruka (Poruke i titlovi), veličina
  teksta.
- Napad i trčanje prekidačem (umjesto držanja).
- Klizač brzine igre za usporavanje cijele igre.
- Automatsko ciljanje s pokazivačem zahvata te postavkama stošca i dometa.

## Poboljšanja

Options → Poboljšanja. Sve radi na strani klijenta i zadano je isključeno,
pa radi sa svakim modom i na mreži.

- **Krhotine**: komadi mesa koji se kotrljaju i šrapneli razbijenih bačvi,
  i kod čudovišta iz modova; mogu udarati o vrata i dizala.
- **Poboljšana krv**: prskanje po površinama, rane koje prate tijela,
  kapljice u zraku koje se razliju gdje padnu.
- **Efekti udaraca**: tragovi sačme, čavli koji se zabijaju u drvo i
  neprijatelje, garež od eksplozija.
- **Poboljšana voda**: prozirnost, valovi, prskanje i kaustika.
- **Prozirni teleporteri** koji prikazuju svoje odredište s paralaksom.
- Ambijentalno zasjenjenje, zamućenje pokreta po objektu i pri brzini,
  meke čestice.
- **Shaderi zaslona**: RetroArch predlošci `.slangp` (CRT i drugi).

Options → Grafika ima i uobičajeno skaliranje prikaza, zaglađivanje rubova,
supersampling, filtriranje tekstura, 8-bitne boje s ditheringom, maglu,
obojeno osvjetljenje te interpolaciju animacije i kretanja.

## Dodaci

Dodaci su nova oružja i oprema koje pokreće pogon igre, pa rade s progsima
svakog moda. Dostupni su samo u igri za jednog igrača i u suradnji.

### Pokretanje igre s dodacima

Igra ima dodatke samo ako se s njima pokrene:

1. Single Player → označite kućicu **Dodaci**, zatim New Game.
2. Odaberite koje dodatke ponijeti i podesite njihove postavke, zatim
   pokrenite novu igru.

Igre s dodacima spremaju se kao datoteke `.qsa` umjesto `.sav`, pa ih drugi
portovi ne pokušavaju učitati. Kartice spremanja prikazuju oznaku
„ADD-ONS”. Učitavanje običnog spremanja isključuje dodatke. Tijekom igre s
dodacima stranica Dodaci (konzola: `menu_options addons`) uključuje i
isključuje pojedine dodatke; svaki ima pregled u petlji, upute, svoje
dodjele tipki i vlastite postavke.

### Oružja

Dodaci idu u uobičajene utore za oružje (birate za svaki dodatak, ili utor
9 i 0) i pojavljuju se na kotaču oružja i u HUD-u. Paljba i alternativna
paljba su vaše uobičajene tipke za napad.

| Dodatak | Paljba | Alternativna paljba | Streljivo |
|---|---|---|---|
| **Portalni pištolj** | Postavlja plavi portal | Postavlja narančasti portal | Nema |
| **Railgun** | Probojni projektil koji pogađa sve na liniji | — | 2 ćelije |
| **Kuka** | Kači se i privlači vas do drva, metala, kamena ili neprijatelja | — | Nema |
| **Bacač ljepljivih bombi** | Držite za punjenje, otpustite za ispaljivanje | Aktiviranje | 1 raketa |
| **Bumerang** | Držite za označavanje neprijatelja, otpustite za bacanje; omamljuje jednog po jednog | Brisanje oznaka | Nema |
| **Prskalica gela** | Plavi odbojni gel (odskočni) | Narančasti pogonski gel (brzi) | Nema |
| **Visokotlačni perač** | Dugi mlaz vode koji odguruje neprijatelje i ispire gel | Široka lepeza mlazova | Nema |

Portali se postavljaju na ravne površine svijeta i prenose vas, čudovišta i
bačene predmete kroz sebe s njihovim zamahom. Uz uključeno **Pogled kroz
portale** drugu stranu vidite uživo, uključujući portale vidljive kroz
portale; kvaliteta, broj živih prikaza, udaljenost i dubina rekurzije su
podesivi.

### Oprema

Oprema se uključuje i isključuje umjesto da se drži u ruci. Otvorite
**kotač opreme** (`+equipwheel`) ili koristite `goggles cycle` za
izmjenjivanje:

- **Noćni vid**
- **Termovizija**

## Igra za više igrača

Osim LAN i internetske igre NetQuake (protokoli 15, 666 i 999) port se
povezuje i s poslužiteljima **QuakeWorld**. Multiplayer → Join Game prima
adresu ili pretražuje LAN. Skriptirane čestice FTE podržane su za modove
koji ih koriste.

## Naredbeni redak i konzola

Uobičajene opcije: `-basedir <dir>`, `-game <dir>`, `-hipnotic` / `-rogue` /
`-quoth`, `-width <w>`, `-height <h>`, `-fullscreen`, `-window` i
`+naredba argumenti` za pokretanje naredbi konzole pri pokretanju. Konzola
(`~`) prihvaća uobičajene naredbe i cvarove Quakea.

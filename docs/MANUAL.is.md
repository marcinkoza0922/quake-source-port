# Notendahandbók

Þessi handbók fjallar um að spila Quake með þessu porti, með áherslu á það
sem það bætir við upprunalega Quake og önnur port. Allt nýtt er valfrjálst:
með sjálfgefnum stillingum lítur leikurinn út og spilast eins og Quake.

## Fyrstu skref

1. Útvegaðu þér þitt eigið eintak af Quake. Vísaðu portinu á möppuna sem
   inniheldur `id1/` (og öll mod við hliðina á henni), með `QUAKE_BASEDIR`
   eða `-basedir <mappa>`.
2. Keyrðu `cargo run --release`, eða ræstu mod beint með
   `-game <mappa> +map <borð>`.
3. Stillingar, vistanir og skjámyndir eru geymdar í
   `~/.local/share/quake-source-port/<leikur>/`, ein mappa fyrir hvert mod.

Í aðalvalmyndinni eru Single Player, Multiplayer, Options, **Help** (þessi
handbók), Quit og **Borðapakkar**, og Quake-stefið hljómar í bakgrunni
(Options → Hljóð → Tónlist í aðalvalmynd). Allar valmyndir virka með mús,
lyklaborði, fjarstýringu og snertiskjá; færðu bendilinn yfir valkost eða
veldu hann til að sjá skýringu.

Í hjálpinni velur upp/niður hluta; Page Up/Page Down, vinstri/hægri,
axlarhnapparnir eða músarhjólið yfir textanum fletta honum.

## Tungumál

Options → Spilun → **Tungumál** velur tungumál valmynda, skilaboða á skjá
og þessarar handbókar: ensku, frönsku, þýsku, ítölsku, spænsku, rússnesku,
pólsku, úkraínsku, tékknesku, slóvakísku, króatísku, búlgörsku, sænsku,
finnsku, dönsku, norsku, hollensku, grísku, ungversku, rúmensku, portúgölsku
(Portúgal eða Brasilía), japönsku, kóresku, kínversku (einfaldaða eða
hefðbundna), tyrknesku, indónesísku, malaísku, filippseysku, víetnömsku,
serbnesku, litháísku, lettnesku, eistnesku, möltnesku, albönsku, armensku,
georgísku, íslensku, írsku, velsku, basknesku, katalónsku, galisísku,
latínu, esperantó, hvítrússnesku, makedónsku, slóvensku, svahílí, arabísku,
hebresku, persnesku, úrdú, hindí, maratí, bengölsku, púnjabí, gújaratí,
tamílsku, telúgú, taílensku, kmer, amharísku eða kantónsku.
**Kerfi** fylgir tungumáli tölvunnar. Leiktexti endurútgáfunnar frá 2021
er aðeins þýddur á frönsku, þýsku, ítölsku, spænsku og rússnesku; önnur
tungumál sýna hann á ensku. Á öðrum tungumálum en ensku notar smái
textinn í leiknum letur valmyndanna, þar sem klassíska Quake-letrið hefur
aðeins latneska stafi án kommu og annarra tákna. Skilaboð í stjórnborðinu
haldast á ensku nema **Þýða stjórnborð** sé á.

## Borðapakkar (mod-vafri)

Aðalvalmynd → Borðapakkar sýnir öll uppsett mod og borðapakka með lýsingu.
Veldu einn til að skipta yfir í hann, skoðaðu síðan borðin hans og hefðu
hvaða borð sem er beint. Vafrinn heldur utan um bestu dráp, leyndarmál og
tíma þinn á hverju borði. Hvert mod hefur sínar eigin stillingar, svo
úthlutanir og stillingar sem eru lagaðar að einu modi berast ekki yfir í
annað.

**Hvar borðapakkar eiga heima.** Portið leitar að þeim í grunnmöppu Quake
(þeirri sem geymir `id1/`, stillt með `QUAKE_BASEDIR` eða `-basedir`). Hver
undirmappa þar sem inniheldur leikgögn (`.pak`- eða `.pk3`-skrá,
`progs.dat` eða `maps/`-möppu) er sýnd, svo þú setur upp borðapakka með því
að afþjappa honum í eigin möppu við hliðina á `id1/`, til dæmis
`<basedir>/ad/` eða `<basedir>/my_maps/maps/*.bsp`. Ef þú vilt geturðu bætt
við `modinfo.txt` með línunum `title`, `base` og `description` til að
stjórna því hvernig hann birtist. Pakkar finnast þegar leikurinn ræsist,
svo endurræstu eftir að þú bætir einum við. Vistanir, skjámyndir og
stillingar hvers pakka eru geymdar í
`~/.local/share/quake-source-port/<mappa>/`, ekki í möppu pakkans.

## Skiptur skjár

Allt að sex leikmenn á einni tölvu, í samvinnu eða deathmatch.

- Multiplayer → New Game → **Staðbundnir leikmenn** stillir hversu margir
  spila.
- Hver leikmaður fær eigið sjónarhorn, HUD, hljóð og inntakstæki
  (lyklaborð og mús fyrir leikmann 1, eina fjarstýringu hver fyrir hina).
- Skjáuppröðunin getur verið hver undir öðrum, hlið við hlið, 2×2 eða 3×2
  (`r_splitscreen_layout`).
- Hver leikmaður velur nafn og lit á skyrtu og buxum undir Multiplayer →
  Setup.

## Fjarstýringar

Portið er hannað til að spilast með fjarstýringu.

- **Vopnahjól** (`+weaponwheel`): haltu inni til að hægja á tímanum og
  veldu vopn með hvorum pinnanum sem er eða músinni; slepptu til að skipta.
- **Miðaðstoð** (Options → Inntak → Fjarstýring): hæging nálægt óvinum og
  snúningur sem fylgir þeim. Hún virkar aðeins á pinnann, aldrei á músina.
- **Gyro-miðun** (Options → Inntak → Fjarstýring → Gyro): rofi, auk
  aðskilinnar næmni fyrir lárétta ásinn, lóðrétta ásinn og veltiásinn
  (velta er sjálfgefið af og snýr sjónarhorninu eins og lárétti ásinn,
  neikvæð gildi snúa henni við), suðsía, og hvenær gyro gildir (alltaf,
  af eða aðeins meðan `+gyroaction` er haldið). **Miðunarstíll** skiptir
  hægri pinnanum milli 2D-miðunar (hann snýr sjónarhorninu jafnt) og
  **flick-stick** (ýttu pinnanum til að beina sjónarhorninu strax í þá átt,
  snúðu síðan pinnanum til að snúa því); hvort tveggja fer vel með gyro
  fyrir nákvæma miðun.
- **Titringur**, næmni pinna og öfug stýring.
- **Hnappatákn** passa við fjarstýringuna þína (Xbox, PlayStation,
  Nintendo) og greinast sjálfkrafa.
- Endurúthlutaðu öllu í Options → Stýringar, líka úr fjarstýringunni.
- Snertistýringar með stillanlegri stærð, ógegnsæi og sjónhraða.

## Vistun

- **Sjálfvirkar vistanir** skiptast á aðskildar frá handvirkum vistunum og
  flýtivistunum, svo þær skrifa aldrei yfir þínar eigin. Options → Spilun →
  Stillingar sjálfvirkrar vistunar velur hversu margar eru geymdar og
  hvenær er vistað: við upphaf borðs, á tímabili, þegar leyndarmál eða öll
  leyndarmál finnast, og þegar allir óvinir eru drepnir.
- **Vistunarspjöld** í Load/Save-valmyndunum sýna borð, heilsu, brynju,
  vopn, lykla og dráp/leyndarmál hverrar vistunar.
- **Við dauða** (Options → Spilun → Við dauða) getur einspilun spurt hvað
  skuli gera í valmynd þar sem leikurinn er í bið, hlaðið nýjustu vistun,
  leyft þér að velja vistun eða byrjað borðið aftur.

## Spilun og myndavél

Options → Spilun:

- Sjónsvið 60–130, sjónvagg, bakslag vopns, hlaupa alltaf.
- **Myndavél**: fyrstu persónu, þriðju persónu eða yfir öxlina (hægri eða
  vinstri).
- Staða **vopnalíkana**: fyrir miðju, hægri, vinstri eða falin.

## Sérsníða: leikbreytur og viðbætur

Leikur er annaðhvort *venjulegur* eða *sérsniðinn*, og það er valið þegar
hann hefst. New Game (og val á borði í Velja borð eða Borðapakkar) sýnir
sprettiglugga: **Spila** hefur leikinn, **Sérsníða upplifun** opnar
Leikbreytur og Viðbætur. Meðan einhverju er breytt ber Spila merkið
**SÉRSNIÐIÐ** og **Slökkva á öllum breytingum** (Del, X á fjarstýringu eða
smellur) setur aftur venjulegu reglurnar án viðbóta. Venjulegir leikir
vista hefðbundnar `.sav`-skrár sem ironwail og vkQuake geta hlaðið;
sérsniðnir leikir vista `.qsa`-skrár sem aðeins þetta port hleður (það
sama á við um leiki á skiptum skjá og samvinnuleiki, sem önnur port geta
ekki vistað). Í sérsniðnum leik breytir Aðalvalmynd → Sérsníða leikbreytum
hans og viðbótum hvenær sem er; reglur venjulegs leiks haldast fastar.
Leikbreytur virka með hvaða modi sem er og slökkva á sér í deathmatch.

- Erfiðara: meiri skaði á þig, harðari óvinir, færri skotfæra- og
  heilsupakkar, 2–4× skrímsli, skammbyssubyrjun á hverju borði, engar
  handvirkar vistanir.
- Auðveldara: minni skaði á þig, veikari óvinir, meiri skotfæri og heilsa,
  óendanleg skotfæri, heilsuendurnýjun, enginn fallskaði, engin drukknun,
  enginn eigin skaði, tvöföld eða þreföld hopp.

## Upptökur

`record <nafn>` og `stop` taka upp demo-upptöku í notendamöppuna þína;
`playdemo <nafn>` spilar hana. Þegar upptöku lýkur er hún endurskrifuð í
bakgrunni svo ironwail og vkQuake geti líka spilað hana. Ef mod notar
agnabrellur sem önnur port geta ekki sýnt er heila útgáfan geymd sem
`<nafn>.qsd` (`playdemo <nafn>.qsd`). `demo_convert <nafn>` umbreytir eldri
upptöku.

Aðalvalmynd → Kynningar sýnir vistaðar kynningar og, þegar Stillingar →
Kynningar → Taka kynningar upp sjálfkrafa er virkt, sjálfvirka kynningu af
hverju borði sem þú spilar; þær elstu víkja þegar plássin eru full. Skrifaðu
til að sía listana og ýttu á Tab til að breyta röðuninni; hnapparnir við
listann spila, flytja út, endurnefna, geyma eða eyða valinni kynningu, og
sjálfvirk kynning sem er endurnefnd er geymd. Villuskýrslur innihalda
sjálfvirka kynningu núverandi borðs.

Meðan kynning spilast gerir Bil hlé, að halda Vinstri eða Hægri spólar til
baka eða áfram, Upp og Niður breyta hraðanum, PgUp og PgDn hoppa 10
sekúndur, og Home og End fara á byrjunina og á næsta kort. I og O merkja
bút; E flytur hann, eða alla kynninguna, í möppuna `videos`. Stjórnstikan
helst á skjánum; H eða Fela felur hana og þegar bent er á neðri brúnina
birtist rönd sem kallar hana fram aftur. Smelltu á hnappana, smelltu á
tímalínuna til að stökkva þangað eða dragðu yfir hana til að merkja bút;
Flytja út spyr um rammatíðni, stærð, snið og skrá og hvort flytja eigi út
merkta bútinn eða alla kynninguna.

Á fjarstýringu gerir A hlé, stefnuhnapparnir spóla til baka og áfram og
breyta hraðanum, LB og RB stökkva 10 sekúndur og pinnahnapparnir fara á
byrjunina og á næsta kort. LT og RT merkja bút, B hreinsar hann, X flytur
út, Y geymir sjálfvirka kynningu og View felur stjórnstikuna.

`capture` byrjar og stöðvar myndbandsupptöku meðan þú spilar. Rammatíðni,
snið og gæði eru undir Stillingar → Kynningar → Myndbandsupptaka. Myndbönd
þurfa ffmpeg; án þess eru rammarnir vistaðir sem PNG-myndir með WAV-skrá.

## Aðgengi

Options → Aðgengi:

- Minni hreyfing og styrkur skjáleiftra.
- Síur fyrir litblindu.
- Stillanlegur miðkross: lögun, stærð, útlína og litaskipti á skotmarki eða
  við dráp; valfrjáls hittmerki.
- **Hljóðskjátextar** og stillingar skilaboða (Skilaboð og skjátextar),
  stærð texta.
- Árás og hlaup af/á (í stað þess að halda inni).
- Sleði fyrir leikhraða til að hægja á öllum leiknum.
- Sjálfvirk miðun með læsingarvísi og stillingum fyrir keilu og drægni.

## Endurbætur

Options → Endurbætur. Allt keyrir á biðlaranum og er sjálfgefið af, svo það
virkar með hvaða modi sem er og á netinu.

- **Brak**: veltandi kjötbitar og sprengjubrot úr sundruðum tunnum, líka
  með skrímslum úr modum; það getur rekist á hurðir og lyftur.
- **Bættar blóðslettur**: slettur á flötum, sár sem fylgja líkömum, dropar í
  lofti sem slettast þar sem þeir lenda.
- **Höggáhrif á fleti**: kúluför haglabyssu, naglar sem festast í við og
  óvinum, brunaför eftir sprengingar.
- **Bætt vatn**: gegnsæi, gárur, skvettur og ljósbrot.
- **Gegnsæir fjarflytjarar** sem sýna áfangastaðinn með hliðrun.
- Umhverfisskygging, hreyfiþoka á hlutum og við hraða, mjúkar agnir.
- **Skjáskyggir**: RetroArch `.slangp`-forstillingar (CRT og fleiri).

Options → Grafík hefur einnig hefðbundnar stillingar: teiknikvarða,
kantjöfnun, ofursýnatöku, áferðarsíu, 8 bita liti með dithering, þoku,
litaða lýsingu og brúun hreyfimynda og hreyfingar.

## Viðbætur

Viðbætur eru ný vopn og búnaður sem vélin keyrir, svo þær virka með progs
hvaða mods sem er. Þær eru aðeins fyrir einspilun og samvinnu.

### Að hefja leik með viðbótum

Leikur hefur viðbætur aðeins ef hann hefst með þeim:

1. Single Player → hakaðu við **Viðbætur**, síðan New Game.
2. Veldu hvaða viðbætur þú tekur með og stilltu þær, hefðu svo nýja
   leikinn.

Leikir með viðbótum vistast sem `.qsa`-skrár í stað `.sav`, svo önnur port
reyna ekki að hlaða þeim. Vistunarspjöld sýna merkið „ADD-ONS“. Að hlaða
venjulegri vistun slekkur á viðbótum. Meðan leikur með viðbótum er spilaður
kveikir og slekkur síðan Viðbætur (stjórnborð: `menu_options addons`) á
einstökum viðbótum; hver þeirra hefur forskoðun í lykkju, leiðbeiningar,
úthlutanir sínar og eigin stillingar.

### Vopn

Viðbætur fara í venjulegu vopnahólfin (veldu hólf fyrir hverja viðbót, eða
hólf 9 og 0) og birtast á vopnahjólinu og í HUD. Skot og aukaskot eru
venjulegu árásarhnapparnir þínir.

| Viðbót | Skot | Aukaskot | Skotfæri |
|---|---|---|---|
| **Gáttabyssa** | Setur bláu gáttina | Setur appelsínugulu gáttina | Engin |
| **Railgun** | Kúla sem fer í gegnum allt á línunni | — | 2 sellur |
| **Krókbyssa** | Festist og dregur þig að viði, málmi, steini eða óvinum | — | Engin |
| **Límsprengjuvarpa** | Haltu inni til að hlaða, slepptu til að skjóta | Sprengja | 1 eldflaug |
| **Búmerang** | Haltu inni til að merkja óvini, slepptu til að kasta; rotar hvern á fætur öðrum | Hreinsa merki | Engin |
| **Hlaupúðari** | Blátt fráhrindihlaup (skoppandi) | Appelsínugult hröðunarhlaup (hratt) | Engin |
| **Háþrýstidæla** | Löng vatnsbuna sem ýtir óvinum og skolar hlaupi af | Breið blævængsbuna | Engin |

Gáttir fara á slétta fleti heimsins og flytja þig, skrímsli og hluti sem
er kastað í gegn með skriðþunga sínum. Með **Sjá í gegnum gáttir** á sérðu
hina hliðina lifandi, líka gáttir sem sjást í gegnum gáttir; gæði, fjölda
lifandi sýna, fjarlægð og dýpt endurkvæmni má stilla.

### Búnaður

Kveikt og slökkt er á búnaði í stað þess að halda á honum. Opnaðu
**búnaðarhjólið** (`+equipwheel`), eða notaðu `goggles cycle` til að fletta
milli:

- **Nætursjónar**
- **Hitasjónar**

## Fjölspilun

Auk NetQuake-spilunar á staðarneti og interneti (samskiptareglur 15, 666
og 999) tengist portið **QuakeWorld**-þjónum. Multiplayer → Taka þátt í
leik tekur við vistfangi eða leitar á staðarnetinu. Skriftaðar FTE-agnir
eru studdar fyrir mod sem nota þær.

## Skipanalína og stjórnborð

Algengir valkostir: `-basedir <mappa>`, `-game <mappa>`, `-hipnotic` /
`-rogue` / `-quoth`, `-width <b>`, `-height <h>`, `-fullscreen`, `-window`,
og `+skipun færibreytur` til að keyra stjórnborðsskipanir við ræsingu.
Stjórnborðið (`~`) tekur við venjulegum Quake-skipunum og cvar-breytum.

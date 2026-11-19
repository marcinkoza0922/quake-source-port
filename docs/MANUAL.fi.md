# Käyttöopas

Tämä opas kertoo, miten Quakea pelataan tällä portilla, ja keskittyy
siihen, mitä portti tuo alkuperäiseen Quakeen ja muihin lähdeportteihin
verrattuna. Kaikki uusi on valinnaista: oletusasetuksilla peli näyttää ja
pelaa kuin Quake.

## Aloitus

1. Hanki oma kopiosi Quakesta. Osoita portille kansio, joka sisältää
   `id1/`-kansion (ja sen vieressä olevat modit), joko
   `QUAKE_BASEDIR`-muuttujalla tai valitsimella `-basedir <kansio>`.
2. Käynnistä `cargo run --release`, tai käynnistä modi suoraan valitsimilla
   `-game <kansio> +map <kartta>`.
3. Asetukset, tallennukset ja kuvakaappaukset ovat kansiossa
   `~/.local/share/quake-source-port/<peli>/`, yksi kansio kutakin modia
   kohden.

Päävalikossa ovat Single Player, Multiplayer, Options, **Help** (tämä
opas), Quit ja **Karttapaketit**, ja taustalla soi Quaken tunnusmusiikki
(Options → Ääni → Päävalikon musiikki). Jokainen valikko toimii hiirellä,
näppäimistöllä, peliohjaimella ja kosketuksella; vie osoitin asetuksen
päälle tai valitse se nähdäksesi sitä selittävän vihjeen.

Ohjeessa ylös/alas valitsee osion; Page Up/Page Down, vasen/oikea,
olkapainikkeet tai hiiren rulla tekstin päällä vierittävät sitä.

## Kieli

Options → Pelattavuus → **Kieli** valitsee valikkojen, ruutuviestien ja tämän
oppaan kielen: englanti, ranska, saksa, italia, espanja, venäjä, puola,
ukraina, tšekki, slovakki, kroatia, bulgaria, ruotsi, suomi, tanska, norja,
hollanti, kreikka, unkari, romania, portugali (Portugali tai Brasilia), japani,
korea, kiina (yksinkertaistettu tai perinteinen), turkki, indonesia, malaiji,
filipino, vietnam, serbia, liettua, latvia, viro, malta, albania, armenia,
georgia, islanti, iiri, kymri, baski, katalaani, galicia, latina, esperanto,
valkovenäjä, makedonia, sloveeni, swahili, arabia, heprea, persia, urdu,
hindi, marathi, bengali, pandžabi, gudžarati, tamili, telugu, thai, khmer,
amhara tai kantoni.
**Järjestelmä** seuraa tietokoneesi kieltä. Vuoden 2021 uusintajulkaisun
omat pelitekstit on käännetty vain ranskaksi, saksaksi, italiaksi,
espanjaksi ja venäjäksi; muilla kielillä ne näkyvät englanniksi. Muilla
kielillä kuin englannilla pelin pieni teksti käyttää valikkofonttia, koska
klassisessa Quake-fontissa on vain latinalaiset kirjaimet ilman
tarkkeita. Konsoliviestit pysyvät englanninkielisinä, ellei **Käännä
konsoli** ole päällä.

## Karttapaketit (modiselain)

Päävalikko → Karttapaketit luettelee jokaisen asennetun modin ja
karttapaketin kuvauksineen. Valitse yksi vaihtaaksesi siihen, selaa sitten
sen karttoja ja käynnistä mikä tahansa niistä suoraan. Selain pitää kirjaa
parhaista tapoistasi, salaisuuksistasi ja ajastasi kartta kartalta. Jokaisella
modilla on omat asetuksensa, joten yhdelle modille säädetyt näppäimet ja
asetukset eivät valu toiseen.

**Minne karttapaketit kuuluvat.** Portti etsii niitä Quaken peruskansiosta
(siitä, jossa on `id1/`, asetetaan `QUAKE_BASEDIR`-muuttujalla tai
`-basedir`-valitsimella). Jokainen sen alikansio, jossa on pelidataa
(`.pak`- tai `.pk3`-tiedosto, `progs.dat` tai `maps/`-kansio), näytetään
luettelossa, joten asenna karttapaketti purkamalla se omaan kansioonsa
`id1/`-kansion viereen, esimerkiksi `<basedir>/ad/` tai
`<basedir>/my_maps/maps/*.bsp`. Voit halutessasi lisätä
`modinfo.txt`-tiedoston, jonka rivit `title`, `base` ja `description` määräävät, miten
paketti näytetään. Paketit löydetään pelin käynnistyessä, joten käynnistä
peli uudelleen paketin lisäämisen jälkeen. Kunkin paketin tallennukset,
kuvakaappaukset ja asetukset ovat kansiossa
`~/.local/share/quake-source-port/<kansio>/`, eivät pakettikansiossa.

## Jaettu ruutu

Enintään kuusi pelaajaa samalla koneella, yhteistyössä tai deathmatchissa.

- Multiplayer → New Game → **Paikalliset pelaajat** määrää, montako
  pelaajaa pelaa.
- Jokainen pelaaja saa oman näkymän, HUDin, äänen ja ohjauslaitteen
  (näppäimistö ja hiiri pelaajalle 1, muille kullekin oma peliohjain).
- Ruudun asettelu voi olla päällekkäin, rinnakkain, 2×2 tai 3×2
  (`r_splitscreen_layout`).
- Jokainen pelaaja valitsee nimen sekä paidan ja housujen värit kohdassa
  Multiplayer → Setup.

## Peliohjaimet

Portti on suunniteltu pelattavaksi peliohjaimella.

- **Asekiekko** (`+weaponwheel`): pidä pohjassa hidastaaksesi aikaa ja
  valitse ase kummalla tahansa tikulla tai hiirellä; päästä irti
  vaihtaaksesi.
- **Tähtäysapu** (Options → Ohjaimet → Peliohjain): hidastus vihollisten
  lähellä ja kierto, joka seuraa niitä. Se koskee vain tikkua, ei koskaan
  hiirtä.
- **Gyrotähtäys** (Options → Ohjaimet → Peliohjain → Gyro): päälle/pois-kytkin
  sekä erilliset herkkyydet vaaka-, pysty- ja kallistusakselille
  (kallistus on oletuksena pois ja kääntää näkymää kuten vaaka-akseli,
  negatiiviset arvot kääntävät suunnan), kohinasuodin ja se, milloin gyro
  on käytössä (aina, pois tai vain kun `+gyroaction` on pohjassa).
  **Tähtäystyyli** vaihtaa oikean tikun 2D-tähtäyksen (kääntää näkymää
  jatkuvasti) ja **flick-tikun** välillä (työnnä tikkua, niin näkymä
  kääntyy heti siihen suuntaan, ja pyöritä sitten tikkua kääntääksesi
  sitä); kumpikin sopii hyvin yhteen gyron kanssa tarkkaan tähtäykseen.
- **Tärinä**, tikun herkkyys ja käänteisyys.
- **Painikesymbolit** vastaavat ohjaintasi (Xbox, PlayStation, Nintendo)
  ja tunnistetaan automaattisesti.
- Kaikki näppäimet voi määrittää uudelleen kohdassa Options → Ohjaus, myös
  peliohjaimella.
- Kosketusohjaus, jonka koon, peittävyyden ja katseen nopeuden voi säätää.

## Tallentaminen

- **Automaattitallennukset** kiertävät erillään manuaalisista
  tallennuksista ja pikatallennuksista, joten ne eivät koskaan korvaa
  omiasi. Options → Pelattavuus → Automaattitallennuksen asetukset
  määrää, montako säilytetään ja milloin tallennetaan: kentän alussa,
  ajastimella, salaisuuden tai kaikkien salaisuuksien löytyessä ja kun
  kaikki viholliset on tapettu.
- **Tallennuskortit** Lataa- ja Tallenna-valikoissa näyttävät kunkin
  tallennuksen kentän, terveyden, panssarin, aseet, avaimet sekä tapot ja
  salaisuudet.
- **Kuollessa** (Options → Pelattavuus → Kuollessa) yksinpeli voi kysyä
  tauotetussa valikossa, mitä tehdään, ladata uusimman tallennuksen, antaa
  sinun valita tallennuksen tai aloittaa kentän alusta.

## Pelattavuus ja kamera

Options → Pelattavuus:

- Näkökenttä 60–130, näkymän keinunta, aseen potku, juokse aina.
- **Kamera**: ensimmäinen persoona, kolmas persoona tai olan yli (oikealta
  tai vasemmalta).
- **Asemallin** sijainti: keskellä, oikealla, vasemmalla tai piilotettu.

## Mukauta: pelimuokkaimet ja lisäosat

Peli on joko *vakio* tai *mukautettu*, ja se valitaan pelin alkaessa. New
Game (ja kentän valinta Kentän valinnassa tai Karttapaketeissa) näyttää
ponnahdusikkunan: **Pelaa** aloittaa pelin, **Mukauta pelikokemusta**
avaa Pelimuokkaimet ja Lisäosat. Kun jotain on muutettu, Pelaa-painikkeessa
on merkintä **MUKAUTETTU**, ja **Poista kaikki muutokset käytöstä** (Del,
ohjaimen X tai klikkaus) palauttaa vakiosäännöt ilman lisäosia. Vakiopelit
tallentavat tavallisia `.sav`-tiedostoja, jotka ironwail ja vkQuake
osaavat ladata; mukautetut pelit tallentavat `.qsa`-tiedostoja, jotka vain
tämä portti lataa (samoin jaetun ruudun ja yhteistyön pelit, joita muut
portit eivät osaa tallentaa). Mukautetun pelin aikana Päävalikko → Mukauta
muuttaa sen muokkaimia ja lisäosia milloin tahansa; vakiopelin säännöt
pysyvät kiinteinä. Muokkaimet toimivat minkä tahansa modin kanssa ja
kytkeytyvät pois deathmatchissa.

- Vaikeampi: enemmän saatua vahinkoa, sitkeämmät viholliset, vähemmän
  ammuksia ja terveyttä löydöistä, 2–4 kertaa enemmän hirviöitä, puhdas
  aloitus joka kentässä, ei manuaalisia tallennuksia.
- Helpompi: vähemmän saatua vahinkoa, heikommat viholliset, enemmän
  ammuksia ja terveyttä, loputtomat ammukset, terveyden palautuminen, ei
  putoamisvahinkoa, ei hukkumista, ei omaa vahinkoa, kaksois- tai
  kolmoishypyt.

## Demot

`record <nimi>` ja `stop` tallentavat demon käyttäjäkansioosi; `playdemo
<nimi>` toistaa sen. Kun tallennus päättyy, demo kirjoitetaan taustalla
uudelleen, jotta myös ironwail ja vkQuake voivat toistaa sen. Jos modi
käyttää partikkelitehosteita, joita muut portit eivät osaa näyttää, täysi
versio säilytetään nimellä `<nimi>.qsd` (`playdemo <nimi>.qsd`).
`demo_convert <nimi>` muuntaa vanhemman demon.

Päävalikko → Demot listaa tallennetut demosi ja, kun Asetukset → Demot →
Tallenna demot automaattisesti on päällä, automaattisen demon jokaisesta
pelaamastasi tasosta; vanhimmat korvataan, kun paikat ovat täynnä.
Kirjoita suodattaaksesi listoja ja paina Tab vaihtaaksesi järjestystä;
listan vieressä olevat painikkeet toistavat, vievät, nimeävät uudelleen,
säilyttävät tai poistavat valitun demon, ja uudelleennimetty
automaattinen demo säilytetään. Virheraportit sisältävät nykyisen tason
automaattisen demon.

Demon aikana Välilyönti pysäyttää, Vasemman tai Oikean pitäminen kelaa
taakse tai eteen, Ylös ja Alas muuttavat nopeutta, PgUp ja PgDn hyppäävät 10
sekuntia, ja Home ja End vievät alkuun ja seuraavaan karttaan. I ja O
merkitsevät leikkeen; E vie sen tai koko demon kansioon `videos`. Ohjaimet
pysyvät näkyvissä; H tai Piilota piilottaa ne, ja alareunaan osoittaminen
näyttää palkin, joka tuo ne takaisin. Napsauta painikkeita, napsauta
aikajanaa hypätäksesi siihen tai vedä sen yli korostaaksesi leikkeen; Vie
kysyy kuvataajuuden, koon, muodon ja tiedoston sekä sen, viedäänkö
korostettu leike vai koko demo.

Ohjaimella A pysäyttää, ristiohjain kelaa taakse ja eteen ja muuttaa
nopeutta, LB ja RB hyppäävät 10 sekuntia, ja sauvojen painallukset vievät
alkuun ja seuraavaan karttaan. LT ja RT merkitsevät leikkeen, B tyhjentää
sen, X vie, Y säilyttää automaattisen demon ja View piilottaa ohjaimet.

`capture` aloittaa ja lopettaa videon tallennuksen pelatessa. Kuvataajuus,
muoto ja laatu ovat kohdassa Asetukset → Demot → Videon tallennus. Videot
tarvitsevat ffmpegin; ilman sitä kuvat tallennetaan PNG-kuvina WAV-tiedoston
kanssa.

## Esteettömyys

Options → Esteettömyys:

- Vähennetty liike ja ruudun välähdysten voimakkuus.
- Värisokeussuotimet.
- Säädettävä tähtäin: muoto, koko, ääriviiva ja värinvaihto kohteen päällä
  tai tapossa; valinnaiset osumamerkit.
- **Äänitekstitykset** ja viestiasetukset (Viestit ja tekstitykset),
  tekstin koko.
- Hyökkäys ja juoksu päälle/pois-tilassa (pohjassa pitämisen sijaan).
- Pelinopeuden liukusäädin koko pelin hidastamiseen.
- Automaattitähtäys lukituksen ilmaisimella sekä kartion ja kantaman
  asetuksilla.

## Parannukset

Options → Parannukset. Kaikki toimii asiakaspäässä ja on oletuksena pois
päältä, joten ne toimivat minkä tahansa modin kanssa ja verkkopelissä.

- **Sirpaleet**: pyörivät lihanpalat ja hajonneiden tynnyreiden sirpaleet,
  myös modien hirviöillä; ne voivat kolahdella ovia ja hissejä vasten.
- **Parempi veri**: roiskeet pinnoilla, ruumiita seuraavat haavat ja
  ilmassa lentävät pisarat, jotka läiskähtävät laskeutumiskohtaansa.
- **Osumatehosteet**: haulien osumajäljet, puuhun ja vihollisiin
  uppoavat naulat, räjähdysten nokijäljet.
- **Parempi vesi**: läpikuultavuus, väreily, roiskeet ja kaustiikka.
- **Läpinäkyvät teleportit**, jotka näyttävät määränpäänsä parallaksilla.
- Ympäristön varjostus, objektikohtainen ja vauhtiin perustuva
  liike-epäterävyys, pehmeät partikkelit.
- **Ruudun varjostimet**: RetroArchin `.slangp`-esiasetukset (CRT ja
  muut).

Options → Grafiikka sisältää lisäksi tavalliset asetukset:
renderöintiskaala, reunanpehmennys, ylinäytteistys, tekstuurisuodatus,
8-bittiset värit rasteroinnilla, sumu, värillinen valaistus sekä
animaation ja liikkeen interpolointi.

## Lisäosat

Lisäosat ovat uusia aseita ja varusteita, joita pelimoottori ohjaa, joten
ne toimivat minkä tahansa modin progs-koodin kanssa. Ne ovat käytettävissä
vain yksinpelissä ja yhteistyössä.

### Pelin aloittaminen lisäosilla

Pelissä on lisäosia vain, jos se aloitetaan niiden kanssa:

1. Single Player → rastita **Lisäosat**-valintaruutu, sitten New Game.
2. Valitse, mitkä lisäosat otat mukaan, ja säädä niiden asetukset, sitten
   aloita uusi peli.

Lisäosapelit tallentuvat `.qsa`-tiedostoina `.sav`-tiedostojen sijaan,
jotta muut portit eivät yritä ladata niitä. Tallennuskorteissa näkyy
”ADD-ONS”-merkki. Tavallisen tallennuksen lataaminen kytkee lisäosat pois.
Lisäosapelin aikana Lisäosat-sivu (konsolissa: `menu_options addons`)
kytkee yksittäisiä lisäosia päälle ja pois; jokaisella on toistuva
esikatselu, ohjeet, näppäinmääritykset ja omat asetukset.

### Aseet

Lisäosat sijoittuvat tavallisiin asepaikkoihin (valittavissa lisäosittain,
tai paikat 9 ja 0) ja näkyvät asekiekossa ja HUDissa. Tuli ja toissijainen
tuli ovat tavalliset hyökkäysnäppäimesi.

| Lisäosa | Tuli | Toissijainen tuli | Ammukset |
|---|---|---|---|
| **Portaaliase** | Asettaa sinisen portaalin | Asettaa oranssin portaalin | Ei mitään |
| **Railgun** | Lävistävä luoti, joka osuu kaikkeen linjallaan | — | 2 kennoa |
| **Tarttumakoukku** | Tarttuu puuhun, metalliin, kiveen tai vihollisiin ja vetää sinut perille | — | Ei mitään |
| **Tarrapomminheitin** | Pidä ladataksesi, päästä irti laukaistaksesi | Räjäytä | 1 raketti |
| **Bumerangi** | Pidä merkitäksesi vihollisia, päästä irti heittääksesi; tainnuttaa ne vuorotellen | Poista merkinnät | Ei mitään |
| **Geeliruisku** | Sininen hylkimisgeeli (pomppiva) | Oranssi vauhtigeeli (nopea) | Ei mitään |
| **Painepesuri** | Pitkä vesisuihku, joka työntää vihollisia ja pesee geelin pois | Leveä suihkuviuhka | Ei mitään |

Portaalit kiinnittyvät maailman tasaisille pinnoille ja kuljettavat sinut,
hirviöt ja heitetyt esineet läpi vauhti säilyttäen. Kun **Näe portaalien
läpi** on päällä, näet toisen puolen elävänä, myös portaalien läpi näkyvät
portaalit; laatua, elävien näkymien määrää, etäisyyttä ja rekursion
syvyyttä voi säätää.

### Varusteet

Varusteet kytketään päälle ja pois sen sijaan, että niitä pidettäisiin
kädessä. Avaa **varustekiekko** (`+equipwheel`) tai käy ne läpi komennolla
`goggles cycle`:

- **Pimeänäkö**
- **Lämpökamera**

## Moninpeli

Lähiverkko- ja internet-NetQuake-pelin (protokollat 15, 666 ja 999) lisäksi
portti yhdistää **QuakeWorld**-palvelimiin. Multiplayer → Join Game ottaa
osoitteen tai hakee lähiverkosta. FTE:n skriptatut partikkelit ovat tuettuja
niitä käyttäville modeille.

## Komentorivi ja konsoli

Yleiset valitsimet: `-basedir <kansio>`, `-game <kansio>`, `-hipnotic` /
`-rogue` / `-quoth`, `-width <l>`, `-height <k>`, `-fullscreen`, `-window`
ja `+komento argumentit` konsolikomentojen suorittamiseen käynnistyksessä.
Konsoli (`~`) hyväksyy tavalliset Quaken komennot ja muuttujat.

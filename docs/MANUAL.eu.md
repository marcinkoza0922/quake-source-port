# Erabiltzailearen eskuliburua

Eskuliburu honek port honekin Quake jokatzea azaltzen du, eta batez ere
jatorrizko Quake-ri eta beste source port batzuei gehitzen diena. Berri den
guztia aukerakoa da: ezarpen lehenetsiekin jokoak Quake bezala dirudi eta
jokatzen da.

## Hasteko

1. Eskuratu zure Quake kopia. Adierazi portari `id1/` (eta haren ondoko
   modak) duen direktorioa, `QUAKE_BASEDIR` edo `-basedir <dir>` erabiliz.
2. Exekutatu `cargo run --release`, edo abiarazi mod bat zuzenean
   `-game <dir> +map <map>` erabiliz.
3. Konfigurazioa, gordetakoak eta pantaila-argazkiak
   `~/.local/share/quake-source-port/<game>/` karpetan daude, karpeta bat
   mod bakoitzeko.

Menu nagusiak Single Player, Multiplayer, Options, **Help** (eskuliburu
hau), Quit eta **Mapa-paketeak** ditu, eta atzean Quake-ren gaia jotzen da
(Options → Audioa → Menu nagusiko musika). Menu guztiek saguarekin,
teklatuarekin, kontrolagailuarekin eta ukipenarekin funtzionatzen dute;
pasatu sagua aukera baten gainetik edo hautatu, hura azaltzen duen
argibidea ikusteko.

Help atalean, gora/behera teklekin atal bat aukeratzen da; Page Up/Page
Down, ezkerrera/eskuinera, sorbalda-botoiek edo saguaren gurpilak testuaren
gainean korritzen dute.

## Hizkuntza

Options → Jokoa → **Hizkuntza** aukerak menuen, pantailako mezuen eta
eskuliburu honen hizkuntza hautatzen du: ingelesa, frantsesa, alemana,
italiera, gaztelania, errusiera, poloniera, ukrainera, txekiera, eslovakiera,
kroaziera, bulgariera, suediera, finlandiera, daniera, norvegiera,
nederlandera, greziera, hungariera, errumaniera, portugesa (Portugal edo
Brasil), japoniera, koreera, txinera (sinplifikatua edo tradizionala),
turkiera, indonesiera, malaysiera, filipinera, vietnamera, serbiera,
lituaniera, letoniera, estoniera, maltera, albaniera, armeniera, georgiera,
islandiera, irlandera, galesa, euskara, katalana, galiziera, latina,
esperantoa, bielorrusiera, mazedoniera, esloveniera, swahilia, arabiera,
hebreera, persiera, urdua, hindia, marathera, bengalera, punjabera,
gujaratera, tamilera, telugua, thailandiera, khmerera, amharera edo kantonera.
**Sistema** aukerak ordenagailuaren hizkuntza jarraitzen du. 2021eko
berrargitalpenaren joko-testua frantsesera, alemanera, italierara,
gaztelaniara eta errusierara soilik dago itzulita; beste hizkuntzetan
ingelesez agertzen da. Ingelesa ez den hizkuntzetan, jokoko testu txikiak
menuko letra-tipoa erabiltzen du, Quake-ren letra-tipo klasikoak azentu
gabeko letra latindarrak soilik dituelako. Kontsolako mezuak ingelesez
geratzen dira **Itzuli kontsola** aktibatuta ez badago.

## Mapa-paketeak (mod-arakatzailea)

Menu nagusia → Mapa-paketeak aukerak instalatutako mod eta mapa-pakete
guztiak zerrendatzen ditu, bakoitza bere deskribapenarekin. Aukeratu bat
hartara aldatzeko, eta gero arakatu haren mapak eta hasi edozein zuzenean.
Arakatzaileak mapa bakoitzeko zure hilketa, sekretu eta denbora onenak
gordetzen ditu. Mod bakoitzak bere konfigurazioa du, beraz mod baterako
doitutako teklak eta ezarpenak ez dira beste batera igarotzen.

**Non jartzen dira mapa-paketeak.** Portak Quake-ren oinarri-direktorioan
bilatzen ditu (`id1/` duena, `QUAKE_BASEDIR` edo `-basedir` bidez
ezarritakoa). Bertako joko-datuak dituen azpikarpeta oro (`.pak` edo `.pk3`
fitxategi bat, `progs.dat` bat edo `maps/` karpeta bat) zerrendatzen da;
beraz, mapa-pakete bat instalatzeko deskonprimitu `id1/`-en ondoko bere
karpetan, adibidez `<basedir>/ad/` edo `<basedir>/my_maps/maps/*.bsp`.
Nahi izanez gero, gehitu `title`, `base` eta `description` lerroak dituen
`modinfo.txt` fitxategi bat nola zerrendatzen den kontrolatzeko. Paketeak
jokoa abiaraztean aurkitzen dira; beraz, berrabiarazi bat gehitu ondoren.
Pakete bakoitzaren gordetakoak, pantaila-argazkiak eta konfigurazioa
`~/.local/share/quake-source-port/<folder>/` karpetan daude, ez paketearen
karpetan.

## Pantaila zatitua

Sei jokalari arte makina bakarrean, kooperatiboan edo deathmatchean.

- Multiplayer → New Game → **Tokiko jokalariak** aukerak zenbat lagunek
  jokatzen duten ezartzen du.
- Jokalari bakoitzak bere ikuspegia, HUDa, soinua eta sarrera-gailua ditu
  (teklatua eta sagua 1. jokalariarentzat, kontrolagailu bana gainerakoentzat).
- Pantailaren diseinua pilatua, alboz albo, 2×2 edo 3×2 izan daiteke
  (`r_splitscreen_layout`).
- Jokalari bakoitzak izena eta alkandoraren/galtzen koloreak aukeratzen ditu
  Multiplayer → Setup atalean.

## Kontrolagailuak

Porta gamepad batekin jokatzeko diseinatuta dago.

- **Arma-gurpila** (`+weaponwheel`): eutsi denbora moteltzeko eta arma bat
  aukeratzeko edozein palankarekin edo saguarekin; askatu aldatzeko.
- **Xede-laguntza** (Options → Sarrera → Kontrolagailua): moteltzea
  etsaietatik gertu eta haiei jarraitzen dien biraketa. Palankari soilik
  aplikatzen zaio, inoiz ez saguari.
- **Giroskopio bidezko xedea** (Options → Sarrera → Kontrolagailua →
  Giroskopioa): etengailu bat, gehi sentikortasun bereiziak yaw, pitch eta
  roll ardatzetarako (roll desaktibatuta dago lehenespenez eta ikuspegia yaw
  bezala biratzen du; balio negatiboek alderantzikatzen dute), zarata-iragazki
  bat, eta giroskopioa noiz aplikatzen den (beti, desaktibatuta edo
  `+gyroaction` sakatuta dagoen bitartean soilik). **Xede-estiloa** aukerak
  eskuineko palanka 2D xedearen (ikuspegia etengabe biratzen du) eta
  **flick stick**-aren artean aldatzen du (bultzatu palanka ikuspegia
  norabide horretara berehala eramateko, eta gero biratu palanka hura
  biratzeko); biek ondo uztartzen dute giroskopioarekin xede finerako.
- **Bibrazioa**, palankaren sentikortasuna eta alderantzikatzea.
- **Botoi-ikurrak** zure kontrolagailuarekin bat datoz (Xbox, PlayStation,
  Nintendo) eta automatikoki detektatzen dira.
- Esleitu berriro dena Options → Kontrolak atalean, baita kontrolagailutik
  ere.
- Ukipen-pantailako kontrolak, tamaina, opakutasuna eta begiratzeko
  abiadura doigarriekin.

## Gordetzea

- **Autogordetzeak** eskuzko gordetzeetatik eta gordetze azkarretatik
  bereizita txandakatzen dira, beraz ez dituzte inoiz zureak gainidazten.
  Options → Jokoa → Autogordetze-ezarpenak atalean zenbat gorde eta noiz
  gorde aukeratzen da: mailan sartzean, tenporizadore batekin, sekretu bat
  edo sekretu guztiak aurkitzean, eta etsai guztiak hiltzean.
- Kargatu/Gorde menuetako **gordetze-txartelek** gordetako bakoitzaren
  maila, osasuna, armadura, armak, giltzak eta hilketak/sekretuak erakusten
  dituzte.
- **Hiltzean** (Options → Jokoa → Hiltzean), jokalari bakarreko moduan zer
  egin galdetu daiteke pausatutako menu batean, gordetako azkena kargatu,
  gordetako bat aukeratzen utzi, edo maila berrabiarazi.

## Jokoa eta kamera

Options → Jokoa:

- Ikus-eremua 60–130, kameraren kulunka, armaren atzerakada, beti korrika.
- **Kamera**: lehen pertsona, hirugarren pertsona edo sorbalda gainetik
  (eskuina edo ezkerra).
- **Armen ereduak** kokapena: erdian, eskuinean, ezkerrean edo ezkutatuta.

## Pertsonalizatu: joko-aldatzaileak eta gehigarriak

Partida bat *jatorrizkoa* edo *pertsonalizatua* da, hastean aukeratuta.
New Game aukerak (eta Hautatu maila edo Mapa-paketeak ataletan maila bat
aukeratzeak) leiho bat erakusten du: **Jokatu** aukerak partida hasten du,
**Pertsonalizatu esperientzia** aukerak Joko-aldatzaileak eta Gehigarriak
irekitzen ditu. Zerbait aldatuta dagoen bitartean, Jokatu
**PERTSONALIZATUA** etiketarekin agertzen da, eta **Desgaitu aldaketa
guztiak** aukerak (Ezab, gamepadeko X edo klik) arau estandarrak
berrezartzen ditu gehigarririk gabe. Jatorrizko partidek ironwail-ek eta
vkQuake-k karga ditzaketen `.sav` fitxategi arruntak gordetzen dituzte;
partida pertsonalizatuek port honek soilik kargatzen dituen `.qsa`
fitxategiak gordetzen dituzte (baita pantaila zatituko eta kooperatiboko
partidek ere, beste portek ezin baitituzte gorde). Partida pertsonalizatu
batean, Menu nagusia → Pertsonalizatu aukerak haren aldatzaileak eta
gehigarriak edozein unetan aldatzen ditu; jatorrizko partida baten arauak
finkoak dira. Aldatzaileek edozein modekin funtzionatzen dute eta
deathmatchean desaktibatu egiten dira.

- Zailagoa: kalte gehiago jasotzea, etsai gogorragoak, munizio eta osasun
  gutxiago, 2–4x munstro, maila bakoitza pistolarekin hastea, eskuzko
  gordetzerik ez.
- Errazagoa: kalte gutxiago jasotzea, etsai ahulagoak, munizio eta osasun
  gehiago, munizio mugagabea, osasunaren berroneratzea, erorketa-kalterik
  ez, itotzerik ez, norberaren kalterik ez, jauzi bikoitzak edo hirukoitzak.

## Demoak

`record <name>` eta `stop` komandoek demo bat grabatzen dute zure
erabiltzaile-karpetan; `playdemo <name>` komandoak erreproduzitzen du.
Grabazioa gelditzean, demoa atzeko planoan berridazten da, ironwail-ek eta
vkQuake-k ere erreproduzi dezaten. Mod batek beste portek erakutsi ezin
dituzten partikula-efektuak erabiltzen baditu, bertsio osoa `<name>.qsd`
gisa gordetzen da (`playdemo <name>.qsd`). `demo_convert <name>` komandoak
demo zaharrago bat bihurtzen du.

Menu nagusia → Demoak atalak gordetako demoak erakusten ditu eta, Aukerak →
Demoak → Grabatu demoak automatikoki aktibatuta badago, jokatzen duzun maila
bakoitzaren demo automatikoa ere bai; zaharrenak ordezten dira tokiak beteta
daudenean. Idatzi zerrendak iragazteko eta sakatu Tab ordena aldatzeko;
zerrendaren ondoko botoiek hautatutako demoa erreproduzitu, esportatu,
izenez aldatu, gorde edo ezabatzen dute, eta demo automatiko baten izena
aldatzean gorde egiten da. Akats-txostenek uneko mailaren demo automatikoa
dute.

Demo bat erreproduzitzean, Zuriuneak pausatzen du, Ezkerra edo Eskuina
sakatuta edukitzeak atzera edo aurrera eramaten du, Gora eta Behera abiadura
aldatzen dute, PgUp eta PgDn 10 segundo jauzi egiten dute, eta Home eta End
hasierara eta hurrengo mapara doaz. I eta O-k klip bat markatzen dute; E-k
hura, edo demo osoa, `videos` karpetara esportatzen du. Kontrolak pantailan
geratzen dira; H edo Ezkutatu sakatuta ezkutatzen dira, eta beheko ertza
seinalatuz gero itzultzen dituen zerrenda bat agertzen da. Egin klik
botoietan, egin klik denbora-lerroan hara jauzi egiteko, edo arrastatu haren
gainean klip bat nabarmentzeko; Esportatu aukerak fotograma-abiadura,
tamaina, formatua eta fitxategia galdetzen ditu, baita nabarmendutako klipa
ala demo osoa esportatu ere.

Kontrolagailuan, A-k pausatzen du, d-pad-ak atzera eta aurrera egiten du eta
abiadura aldatzen du, LB eta RB-k 10 segundo jauzi egiten dute, eta makilen
botoiek hasierara eta hurrengo mapara eramaten dute. LT eta RT-k klip bat
markatzen dute, B-k garbitzen du, X-k esportatzen du, Y-k demo automatiko
bat gordetzen du eta View-k kontrolak ezkutatzen ditu.

`capture`-k jokatzen ari zaren bitartean bideo-grabazioa hasi eta gelditzen
du. Fotograma-abiadura, formatua eta kalitatea Aukerak → Demoak →
Bideo-grabazioa atalean daude. Bideoek ffmpeg behar dute; hura gabe
fotogramak PNG irudi gisa gordetzen dira WAV fitxategi batekin.

## Irisgarritasuna

Options → Irisgarritasuna:

- Mugimendu murriztua eta pantaila-distiren intentsitatea.
- Daltonismo-iragazkiak.
- Mira konfiguragarria: forma, tamaina, ingerada eta kolore-aldaketak
  xedean edo hilketa batean; aukerako kolpe-markak.
- **Soinuen azpitituluak**; mezuen eta azpitituluen kokapena, tamaina eta
  iraupena HUD editorean ezartzen dira.
- Erasoa eta korrika txandakatzea (eutsi beharrean).
- Joko-abiaduraren graduatzailea joko osoa moteltzeko.
- Xede automatikoa finkatze-adierazlearekin, konoa eta irismena
  doigarriekin.

## Hobekuntzak

Options → Hobekuntzak. Dena bezeroaren aldekoa eta lehenespenez
desaktibatuta, beraz edozein modekin eta sarean funtzionatzen dute.

- **Hondakinak**: biraka doazen haragi-puskak eta upel apurtuen metraila,
  moden munstroekin ere; ateen eta igogailuen kontra jo dezakete.
- **Odol hobetua**: zipriztinak gainazaletan, gorputzei jarraitzen dieten
  zauriak, eta erortzen diren lekuan zapaltzen diren tanta hegalariak.
- **Talka-efektuak**: perdigoi-markak, egurrean eta etsaietan iltzatzen
  diren iltzeak, leherketen erre-markak.
- **Ur hobetua**: gardentasuna, uhinak, zipriztinak eta kaustikoak.
- **Teleportagailu gardenak**, helmuga paralaxearekin erakusten dutenak.
- Giro-oklusioa, objektuko eta abiadurako mugimendu-lausotzea, partikula
  leunak.
- **Pantaila-shaderrak**: RetroArch `.slangp` aurrezarpenak (CRT eta beste).

Options → Grafikoak atalean ohiko aukerak ere badaude: errendatze-eskala,
antialiasinga, supersampling-a, testura-iragazkia, 8 biteko kolorea
dithering-arekin, lainoa, argiztapen koloreduna eta animazioaren/mugimenduaren
interpolazioa.

## Gehigarriak

Gehigarriak arma eta ekipamendu berriak dira, motorrak exekutatuak, beraz
edozein moden progs-ekin funtzionatzen dute. Jokalari bakarrean eta
kooperatiboan soilik daude.

### Partida bat gehigarriekin hastea

Partida batek gehigarriak ditu haiekin hasten bada soilik:

1. Single Player → markatu **Gehigarriak** laukia, eta gero New Game.
2. Aukeratu zein gehigarri eraman eta doitu haien ezarpenak, eta gero Start
   New Game.

Gehigarridun partidak `.qsa` fitxategi gisa gordetzen dira `.sav` gisa
beharrean, beste portak haiek kargatzen saia ez daitezen. Gordetze-txartelek
"ADD-ONS" ikurra erakusten dute. Gordetako partida arrunt bat kargatzeak
gehigarriak desaktibatzen ditu. Gehigarridun partida batean, Gehigarriak
orriak (kontsola: `menu_options addons`) gehigarriak banan-banan aktibatu
eta desaktibatzen ditu; bakoitzak begiztako aurrebista, argibideak, bere
teklak eta bere ezarpenak ditu.

### Armak

Gehigarriak arma-zirrikitu arruntetan sartzen dira (aukeratu zein
gehigarri bakoitzeko, edo 9. eta 0. zirrikituak) eta arma-gurpilean eta
HUDean agertzen dira. Tiroa eta tiro alternatiboa zure eraso-botoi
arruntak dira.

| Gehigarria | Tiroa | Tiro alternatiboa | Munizioa |
|---|---|---|---|
| **Atari-pistola** | Jarri atari urdina | Jarri atari laranja | Bat ere ez |
| **Railgun** | Lerroko guztia jotzen duen bala zulatzailea | — | 2 zelula |
| **Kakoa** | Heldu eta erakarri zure burua egurrera, metalera, harrira edo etsaietara | — | Bat ere ez |
| **Bonba itsaskorren jaurtigailua** | Eutsi kargatzeko, askatu jaurtitzeko | Lehertu | Suziri 1 |
| **Bumerana** | Eutsi etsaiak markatzeko, askatu jaurtitzeko; bakoitza txandaka zorabiatzen du | Garbitu markak | Bat ere ez |
| **Gel-ihinztagailua** | Errepultsio-gel urdina (erreboteka) | Propultsio-gel laranja (azkarra) | Bat ere ez |
| **Presio-garbigailua** | Etsaiak bultzatu eta gela garbitzen duen ur-zorrotada luzea | Zorrotaden haizemaile zabala | Bat ere ez |

Atariak munduko gainazal lauetan jartzen dira eta zu, munstroak eta
jaurtitako objektuak beren abiadurarekin eramaten dituzte alde batetik
bestera. **Ikusi atarien bidez** aktibatuta, beste aldea zuzenean ikusten
duzu, atarien bidez ikusten diren atariak barne; kalitatea, zuzeneko
ikuspegien kopurua, distantzia eta errekurtsio-sakonera doigarriak dira.

### Ekipamendua

Ekipamendua aktibatu eta desaktibatu egiten da, ez da eskuan eramaten.
Ireki **ekipamendu-gurpila** (`+equipwheel`), edo erabili `goggles cycle`
hauen artean txandakatzeko:

- **Gaueko ikusmena**
- **Ikusmen termikoa**

## Jokalari anitz

LAN eta Interneteko NetQuake jokoaz gain (15, 666 eta 999 protokoloak),
porta **QuakeWorld** zerbitzarietara konektatzen da. Multiplayer → Join
Game aukerak helbide bat onartzen du edo LANean bilatzen du. FTE script
bidezko partikulak onartzen dira haiek erabiltzen dituzten modetarako.

## Komando-lerroa eta kontsola

Aukera ohikoak: `-basedir <dir>`, `-game <dir>`, `-hipnotic` / `-rogue` /
`-quoth`, `-width <w>`, `-height <h>`, `-fullscreen`, `-window`, eta
`+command args` abiaraztean kontsola-komandoak exekutatzeko. Kontsolak
(`~`) Quake-ren ohiko komandoak eta cvar-ak onartzen ditu.

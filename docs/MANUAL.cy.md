# Llawlyfr defnyddiwr

Mae'r llawlyfr hwn yn esbonio sut i chwarae Quake gyda'r injan hon, gan
ganolbwyntio ar yr hyn mae'n ei ychwanegu at Quake gwreiddiol ac injans
eraill. Mae popeth newydd yn ddewisol: gyda'r gosodiadau diofyn mae'r gêm
yn edrych ac yn chwarae fel Quake.

## Dechrau arni

1. Mynnwch eich copi eich hun o Quake. Rhowch wybod i'r injan ble mae'r
   ffolder sy'n cynnwys `id1/` (ac unrhyw fodiau wrth ei ymyl), gyda
   `QUAKE_BASEDIR` neu `-basedir <ffolder>`.
2. Rhedwch `cargo run --release`, neu dechreuwch fod yn syth gyda
   `-game <ffolder> +map <map>`.
3. Mae'r ffurfweddiad, y ffeiliau cadw a'r sgrinluniau yn
   `~/.local/share/quake-source-port/<gêm>/`, un ffolder i bob mod.

Mae gan y brif ddewislen Single Player, Multiplayer, Options, **Help** (y
llawlyfr hwn), Quit a **Pecynnau mapiau**, gyda thema Quake yn chwarae yn y
cefndir (Options → Sain → Cerddoriaeth y brif ddewislen). Mae pob dewislen
yn gweithio gyda llygoden, bysellfwrdd, rheolydd a sgrin gyffwrdd; hofranwch
dros opsiwn neu ei ddewis i weld cyngor sy'n ei esbonio.

Yn Help, mae i fyny/i lawr yn dewis adran; mae Page Up/Page Down,
chwith/dde, y botymau ysgwydd neu olwyn y llygoden dros y testun yn ei
sgrolio.

## Iaith

Mae Options → Chwarae → **Iaith** yn dewis iaith y dewislenni, y negeseuon
ar y sgrin a'r llawlyfr hwn: Saesneg, Ffrangeg, Almaeneg, Eidaleg, Sbaeneg,
Rwseg, Pwyleg, Wcreineg, Tsieceg, Slofaceg, Croateg, Bwlgareg, Swedeg,
Ffinneg, Daneg, Norwyeg, Iseldireg, Groeg, Hwngareg, Rwmaneg, Portiwgaleg
(Portiwgal neu Brasil), Japaneeg, Coreeg, Tsieinëeg (Syml neu
Draddodiadol), Twrceg, Indoneseg, Maleieg, Ffilipineg, Fietnameg, Serbeg,
Lithwaneg, Latfieg, Estoneg, Malteg, Albaneg, Armeneg, Georgeg, Islandeg,
Gwyddeleg, Cymraeg, Basgeg, Catalaneg, Galiseg, Lladin, Esperanto,
Belarwseg, Macedoneg, Slofeneg, Swahili, Arabeg, Hebraeg, Perseg, Wrdw,
Hindi, Marathi, Bengaleg, Pwnjabeg, Gwjarati, Tamileg, Telugu, Thai, Chmereg,
Amhareg neu Gantoneg.
Mae **System** yn dilyn iaith eich cyfrifiadur. Dim ond i'r Ffrangeg,
Almaeneg, Eidaleg, Sbaeneg a Rwseg y mae testun gêm ail-ryddhad 2021 wedi'i
gyfieithu; mae ieithoedd eraill yn ei ddangos yn Saesneg. Mewn ieithoedd
heblaw Saesneg mae'r testun bach yn y gêm yn defnyddio ffont y dewislenni,
gan mai dim ond llythrennau Lladin heb acenion sydd yn ffont clasurol
Quake. Mae negeseuon y consol yn aros yn Saesneg oni bai bod **Cyfieithu'r
consol** ymlaen.

## Pecynnau mapiau (porwr modiau)

Mae Prif ddewislen → Pecynnau mapiau yn rhestru pob mod a phecyn mapiau sydd
wedi'u gosod, gyda disgrifiad. Dewiswch un i newid iddo, yna poriwch ei
fapiau a dechrau unrhyw un ohonynt yn syth. Mae'r porwr yn cofio eich
canlyniadau gorau o ran lladd, cyfrinachau ac amser ar gyfer pob map. Mae
gan bob mod ei ffurfweddiad ei hun, felly nid yw bysellau a gosodiadau a
diwniwyd ar gyfer un mod yn gollwng i fod arall.

**Ble mae pecynnau mapiau'n mynd.** Mae'r injan yn chwilio amdanynt yn
ffolder sylfaen Quake (yr un sy'n cynnwys `id1/`, a osodir gyda
`QUAKE_BASEDIR` neu `-basedir`). Mae pob is-ffolder yno sy'n cynnwys data
gêm (ffeil `.pak` neu `.pk3`, `progs.dat`, neu ffolder `maps/`) yn cael ei
restru, felly gosodwch becyn mapiau drwy ei ddadbacio i'w ffolder ei hun
wrth ymyl `id1/`, er enghraifft `<basedir>/ad/` neu
`<basedir>/my_maps/maps/*.bsp`. Os mynnwch, ychwanegwch `modinfo.txt` gyda
llinellau `title`, `base` a `description` i reoli sut mae'n cael ei restru.
Mae pecynnau'n cael eu canfod pan fydd y gêm yn dechrau, felly ailgychwynnwch
ar ôl ychwanegu un. Mae ffeiliau cadw, sgrinluniau a ffurfweddiad pob pecyn
yn `~/.local/share/quake-source-port/<ffolder>/`, nid yn ffolder y pecyn.

## Sgrin hollt

Hyd at chwe chwaraewr ar un peiriant, yn gydweithredol neu mewn deathmatch.

- Mae Multiplayer → New Game → **Chwaraewyr lleol** yn gosod faint o bobl
  sy'n chwarae.
- Mae pob chwaraewr yn cael ei olwg, ei HUD, ei sain a'i ddyfais fewnbwn ei
  hun (bysellfwrdd a llygoden i chwaraewr 1, rheolydd yr un i'r lleill).
- Gall cynllun y sgrin fod wedi'i bentyrru, ochr yn ochr, 2×2 neu 3×2
  (`r_splitscreen_layout`).
- Mae pob chwaraewr yn dewis enw a lliwiau crys/trowsus o dan Multiplayer →
  Setup.

## Rheolyddion gêm

Mae'r injan wedi'i chynllunio i'w chwarae gyda phad gêm.

- **Olwyn arfau** (`+weaponwheel`): daliwch i arafu amser a dewis arf
  gyda'r naill ffon neu'r llygoden; gollyngwch i newid.
- **Cymorth anelu** (Options → Mewnbwn → Rheolydd): arafu ger gelynion a
  chylchdro sy'n eu dilyn. Mae'n effeithio ar y ffon yn unig, byth ar y
  llygoden.
- **Anelu gyro** (Options → Mewnbwn → Rheolydd → Gyro): togl, a
  sensitifrwydd ar wahân i'r echelinau gwyro, traw a rholio (mae rholio i
  ffwrdd yn ddiofyn ac yn troi'r olwg fel gwyro, mae gwerthoedd negatif yn
  ei wrthdroi), hidlydd sŵn, a phryd mae'r gyro'n berthnasol (bob amser, i
  ffwrdd neu dim ond wrth ddal `+gyroaction`). Mae **Arddull anelu** yn
  newid y ffon dde rhwng anelu 2D (mae'n troi'r olwg yn ddi-dor) a **flick
  stick** (gwthiwch y ffon i neidio'r olwg i'r cyfeiriad hwnnw, yna trowch
  y ffon i'w chylchdroi); mae'r ddau'n cyfuno'n dda â'r gyro ar gyfer anelu
  manwl.
- **Dirgryniad**, sensitifrwydd y ffyn a'u gwrthdroi.
- Mae **Symbolau botymau** yn cyfateb i'ch pad (Xbox, PlayStation,
  Nintendo) ac yn cael eu canfod yn awtomatig.
- Ail-neilltuwch bopeth yn Options → Rheolyddion, gan gynnwys o'r rheolydd.
- **Select / View ×2**: Sgrinlun; **Start ×2**: Cadw cyflym; **Start**
  (Dal): Llwytho cyflym.
- Rheolyddion sgrin gyffwrdd gyda maint, didreiddedd a chyflymder edrych
  addasadwy.

## Cadw

- Mae **Awtogadwau** yn cylchdroi ar wahân i ffeiliau cadw â llaw a chadw
  cyflym, felly nid ydynt byth yn trosysgrifo eich rhai chi. Mae Options →
  Chwarae → Gosodiadau awtogadw yn dewis faint i'w cadw a phryd i gadw: wrth
  ddechrau lefel, ar amserydd, wrth ganfod cyfrinach neu bob cyfrinach, ac
  ar ôl lladd pob gelyn.
- Mae **cardiau cadw** yn y dewislenni Load/Save yn dangos lefel, iechyd,
  arfwisg, arfau, allweddi a lladd/cyfrinachau pob ffeil gadw.
- **Wrth farw** (Options → Chwarae → Wrth farw), gall y modd un chwaraewr
  ofyn beth i'w wneud mewn dewislen wedi'i seibio, llwytho'r cadw
  diweddaraf, gadael i chi ddewis ffeil gadw, neu ailddechrau'r lefel.

## Chwarae a chamera

Options → Chwarae:

- Maes golwg 60–130, siglo'r olwg, adlam gwn, rhedeg bob amser.
- **Camera**: person cyntaf, trydydd person, neu dros yr ysgwydd (dde neu
  chwith).
- Safle'r **modelau arfau**: canol, de, chwith neu wedi'u cuddio.

## Addasu: addaswyr chwarae ac ychwanegion

Mae gêm naill ai'n *safonol* neu'n *addasedig*, a ddewisir pan fydd yn
dechrau. Mae New Game (a dewis lefel yn Dewis lefel neu Pecynnau mapiau) yn
dangos naidlen: mae **Chwarae** yn dechrau'r gêm, mae **Addasu'r profiad**
yn agor Addaswyr chwarae ac Ychwanegion. Tra bo unrhyw beth wedi newid, mae
Chwarae wedi'i dagio'n **ADDASEDIG** ac mae **Analluogi pob addasiad** (Del,
X ar y pad gêm, neu glicio) yn adfer y rheolau safonol heb ychwanegion. Mae
gemau safonol yn cadw ffeiliau `.sav` cyffredin y gall ironwail a vkQuake eu
llwytho; mae gemau addasedig yn cadw ffeiliau `.qsa` y mae'r injan hon yn
unig yn eu llwytho (felly hefyd gemau sgrin hollt a chydweithredol, na all
injans eraill eu cadw). Yn ystod gêm addasedig, mae Prif ddewislen → Addasu
yn newid ei haddaswyr a'i hychwanegion unrhyw bryd; mae rheolau gêm safonol
yn aros yn sefydlog. Mae addaswyr yn gweithio gydag unrhyw fod ac yn
diffodd mewn deathmatch.

- Anoddach: mwy o ddifrod i chi, gelynion caletach, llai o fwledi ac iechyd
  i'w casglu, angenfilod x2–4, dechrau o'r newydd ym mhob lefel, dim cadw â
  llaw.
- Haws: llai o ddifrod i chi, gelynion gwannach, mwy o fwledi ac iechyd,
  bwledi diddiwedd, adfywio iechyd, dim difrod cwympo, dim boddi, dim
  hunan-ddifrod, neidiau dwbl neu driphlyg.

## Demos

Mae `record <enw>` a `stop` yn recordio demo i'ch ffolder defnyddiwr; mae
`playdemo <enw>` yn ei chwarae. Pan ddaw'r recordio i ben, mae'r demo'n
cael ei ailysgrifennu yn y cefndir fel y gall ironwail a vkQuake ei chwarae
hefyd. Os yw mod yn defnyddio effeithiau gronynnau na all injans eraill eu
dangos, mae'r fersiwn llawn yn cael ei gadw fel `<enw>.qsd` (`playdemo
<enw>.qsd`). Mae `demo_convert <enw>` yn trosi demo hŷn.

Mae Prif ddewislen → Demos yn rhestru eich demos wedi'u cadw ac, os yw
Opsiynau → Demos → Recordio demos yn awtomatig ymlaen, demo awtomatig o bob
lefel rydych yn ei chwarae; mae'r hynaf yn cael eu disodli pan fydd y
slotiau'n llawn. Teipiwch i hidlo'r rhestrau a phwyswch Tab i newid y
drefn; mae'r botymau wrth ymyl y rhestr yn chwarae, allforio, ailenwi, cadw
neu ddileu'r demo a ddewiswyd, ac mae ailenwi demo awtomatig yn ei gadw.
Mae adroddiadau nam yn cynnwys demo awtomatig y lefel bresennol.

Wrth i demo chwarae, mae Bwlch yn oedi, mae dal Chwith neu De yn weindio'n
ôl neu ymlaen, mae I fyny ac I lawr yn newid y cyflymder, mae PgUp a PgDn yn
neidio 10 eiliad, ac mae Home ac End yn mynd i'r dechrau ac i'r map nesaf.
Mae I ac O yn marcio clip; mae E yn ei allforio, neu'r demo cyfan, i'r
ffolder `videos`. Mae'r rheolyddion yn aros ar y sgrin; mae H neu Cuddio'n
eu cuddio, ac wrth bwyntio at yr ymyl isaf daw stribed i'r golwg sy'n dod â
nhw'n ôl. Cliciwch y botymau, cliciwch y llinell amser i neidio yno, neu
llusgwch ar ei thraws i amlygu clip; mae Allforio'n gofyn am gyfradd
fframiau, maint, fformat a ffeil, ac a ddylid allforio'r clip wedi'i amlygu
neu'r demo cyfan.

Ar reolydd, mae A yn oedi, mae'r pad cyfeiriad yn mynd yn ôl ac ymlaen ac yn
newid y cyflymder, mae LB ac RB yn neidio 10 eiliad, ac mae botymau'r ffyn
yn mynd i'r dechrau ac i'r map nesaf. Mae LT ac RT yn marcio clip, B yn ei
glirio, X yn allforio, Y yn cadw demo awtomatig a View yn cuddio'r
rheolyddion.

Mae `capture` yn dechrau ac yn stopio recordio fideo wrth i chi chwarae. Mae
cyfradd fframiau, fformat ac ansawdd o dan Opsiynau → Demos → Recordio
fideo. Mae fideos angen ffmpeg; hebddo caiff y fframiau eu cadw fel
delweddau PNG gyda ffeil WAV.

## Hygyrchedd

Options → Hygyrchedd:

- Lleihau symudiad a dwyster fflachiadau'r sgrin.
- Hidlyddion dallineb lliw.
- Croesflew ffurfweddadwy: siâp, maint, amlinell, a newid lliw ar darged
  neu wrth ladd; marcwyr taro dewisol.
- **Capsiynau sain**; mae safle, maint a hyd negeseuon a chapsiynau'n cael
  eu gosod yn y golygydd HUD.
- Togl (yn lle dal) ar gyfer ymosod a rhedeg.
- Llithrydd cyflymder y gêm i arafu'r gêm gyfan.
- Anelu awtomatig gyda dangosydd cloi, a gosodiadau côn a phellter.

## Gwelliannau

Options → Gwelliannau. Mae'r cyfan ar ochr y cleient ac i ffwrdd yn
ddiofyn, felly maent yn gweithio gydag unrhyw fod ac ar-lein.

- **Malurion**: darnau cnawd sy'n rholio a shrapnel o gasgenni sy'n
  chwalu, gydag angenfilod modiau hefyd; gall daro yn erbyn drysau a
  lifftiau.
- **Gwaed gwell**: tasgiadau ar arwynebau, clwyfau sy'n dilyn cyrff,
  diferion yn yr awyr sy'n tasgu lle maent yn glanio.
- **Effeithiau trawiad**: olion haels dryll, hoelion sy'n glynu mewn pren a
  gelynion, olion llosg ffrwydradau.
- **Dŵr gwell**: tryleuedd, crychdonnau, tasgiadau a chostigau; **Plygiant
  tanddwr**, **Diferion ar y sgrin**.
- **Teleportwyr tryloyw** sy'n dangos pen eu taith gyda pharalacs.
- Occlwsiwn amgylchol, aneglurder symud fesul gwrthrych ac aneglurder
  cyflymder, gronynnau meddal.
- **Arlliwwyr sgrin**: rhagosodiadau `.slangp` RetroArch (CRT ac eraill).

Mae gan Options → Graffeg hefyd y gosodiadau arferol: graddfa rendro,
gwrth-ddanheddu, uwchsamplu, hidlo gweadau, lliw 8-did gyda gwasgaru
picseli, niwl, goleuo lliw a rhyngosod animeiddio/symudiad.

## Ychwanegion

Arfau ac offer newydd yw ychwanegion, sy'n cael eu rhedeg gan yr injan
felly maent yn gweithio gyda progs unrhyw fod. Maent ar gael mewn gemau un
chwaraewr a chydweithredol yn unig.

### Dechrau gêm gydag ychwanegion

Dim ond os yw'n dechrau gyda nhw y mae gan gêm ychwanegion:

1. Single Player → ticiwch y blwch **Ychwanegion**, yna New Game.
2. Dewiswch pa ychwanegion i ddod gyda chi ac addaswch eu gosodiadau, yna
   dechreuwch y gêm newydd.

Mae gemau ag ychwanegion yn cadw fel ffeiliau `.qsa` yn lle `.sav`, fel nad
yw injans eraill yn ceisio eu llwytho. Mae cardiau cadw'n dangos bathodyn
"ADD-ONS". Mae llwytho ffeil gadw arferol yn diffodd ychwanegion. Wrth
chwarae gêm ag ychwanegion, mae'r dudalen Ychwanegion (consol:
`menu_options addons`) yn troi ychwanegion unigol ymlaen ac i ffwrdd; mae
gan bob un ragolwg mewn dolen, cyfarwyddiadau, ei fysellau a'i osodiadau ei
hun.

### Arfau

Mae ychwanegion yn mynd i'r slotiau arfau arferol (dewiswch pa un i bob
ychwanegyn, neu slot 9 a 0) ac yn ymddangos ar yr olwyn arfau ac yn yr HUD.
Tanio a thanio amgen yw eich botymau ymosod arferol.

| Ychwanegyn | Tanio | Tanio amgen | Bwledi |
|---|---|---|---|
| **Gwn pyrth** | Gosod y porth glas | Gosod y porth oren | Dim |
| **Railgun** | Bwled treiddiol sy'n taro popeth ar linell | — | 2 gell |
| **Bachyn** | Cydio mewn pren, metel, carreg neu elynion a'ch tynnu atynt | — | Dim |
| **Lansiwr bomiau gludiog** | Daliwch i wefru, gollyngwch i lansio | Ffrwydro | 1 roced |
| **Bwmerang** | Daliwch i farcio gelynion, gollyngwch i daflu; mae'n syfrdanu pob un yn ei dro | Clirio'r marciau | Dim |
| **Chwistrellwr gel** | Gel gwrthyrru glas (sboncio) | Gel gyrru oren (cyflym) | Dim |
| **Golchwr pŵer** | Jet dŵr hir sy'n gwthio gelynion ac yn golchi gel i ffwrdd | Gwyntyll lydan o jetiau | Dim |
| **Cebl gwryw-gwryw** | Mae tân eilaidd ar banel electronig disglair o fewn cyrraedd yn plygio un pen i mewn; mae'r pen arall wedyn yn saethu mellt gydag ammo diderfyn | Mae tân eilaidd neu newid arf yn ei rwygo allan | Dim |
| **Pâr o bistolau** | Chwith | De | Dim (Ail-lwytho: R) |
| **Reiffl ymosod** | Reiffl cwbl awtomatig sy'n ail-lwytho o'ch hoelion | Mae Tanio amgen yn anelu drwy'r annel dot coch am ergydion tynnach a golwg agosach | Hoelion (Ail-lwytho: R) |

Mae pyrth yn mynd ar arwynebau gwastad y byd ac yn eich cario chi,
angenfilod a gwrthrychau a deflir drwyddynt gyda'u momentwm. Gyda **Gweld
trwy byrth** ymlaen rydych yn gweld yr ochr arall yn fyw, gan gynnwys pyrth
sy'n weladwy drwy byrth; gellir addasu'r ansawdd, nifer y golygfeydd byw,
y pellter a dyfnder yr atchwel.

### Offer

Mae offer yn cael ei droi ymlaen ac i ffwrdd yn hytrach na'i ddal. Agorwch
yr **olwyn offer** (`+equipwheel`), neu defnyddiwch `goggles cycle` i fynd
drwy:

- **Golwg nos**
- **Golwg thermol**

## Aml-chwaraewr

Yn ogystal â chwarae NetQuake ar LAN a'r rhyngrwyd (protocolau 15, 666 a
999) mae'r injan yn cysylltu â gweinyddion **QuakeWorld**. Mae Multiplayer
→ Ymuno â gêm yn derbyn cyfeiriad neu'n chwilio'r LAN. Mae gronynnau
sgript FTE yn cael eu cefnogi ar gyfer modiau sy'n eu defnyddio.

## Llinell orchymyn a chonsol

Opsiynau cyffredin: `-basedir <ffolder>`, `-game <ffolder>`, `-hipnotic` /
`-rogue` / `-quoth`, `-width <w>`, `-height <h>`, `-fullscreen`, `-window`,
a `+command args` i redeg gorchmynion consol wrth gychwyn. Mae'r consol
(`~`) yn derbyn gorchmynion a cvars arferol Quake.

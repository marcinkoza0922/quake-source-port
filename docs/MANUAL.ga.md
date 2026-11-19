# Lámhleabhar an úsáideora

Míníonn an lámhleabhar seo conas Quake a imirt leis an gcalafort seo, agus
an bhéim ar a gcuireann sé le Quake bunaidh agus le calafoirt foinse eile.
Is roghnach gach rud nua: leis na réamhshocruithe, breathnaíonn an cluiche
agus imrítear é díreach mar Quake.

## Ag tosú

1. Faigh do chóip féin de Quake. Dírigh an calafort ar an bhfillteán ina
   bhfuil `id1/` (agus aon mhodanna in aice leis), le `QUAKE_BASEDIR` nó
   `-basedir <fillteán>`.
2. Rith `cargo run --release`, nó tosaigh mod láithreach le
   `-game <fillteán> +map <léarscáil>`.
3. Coinnítear cumraíocht, sábhálacha agus gabhálacha scáileáin in
   `~/.local/share/quake-source-port/<cluiche>/`, fillteán amháin in
   aghaidh an mhoid.

Sa phríomhroghchlár tá Single Player, Multiplayer, Options, **Help** (an
lámhleabhar seo), Quit agus **Pacáistí Léarscáileanna**, agus téama Quake á
sheinm sa chúlra (Options → Fuaim → Ceol an Phríomhroghchláir). Oibríonn
gach roghchlár le luch, méarchlár, rialtán agus tadhall; cuir an cúrsóir
os cionn rogha nó roghnaigh í chun leid uirlise a fheiceáil a mhíníonn í.

In Help, roghnaíonn suas/síos roinn; scrollaíonn Page Up/Page Down,
clé/deas, na cnaipí gualainne nó roth na luiche os cionn an téacs í.

## Teanga

Roghnaíonn Options → Imirt → **Teanga** teanga na roghchlár, na
dteachtaireachtaí ar an scáileán agus an lámhleabhair seo: Béarla,
Fraincis, Gearmáinis, Iodáilis, Spáinnis, Rúisis, Polainnis, Úcráinis,
Seicis, Slóvaicis, Cróitis, Bulgáiris, Sualainnis, Fionlainnis,
Danmhairgis, Ioruais, Ollainnis, Gréigis, Ungáiris, Rómáinis, Portaingéilis
(an Phortaingéil nó an Bhrasaíl), Seapáinis, Cóiréis, Sínis (Simplithe nó
Traidisiúnta), Tuircis, Indinéisis, Malaeis, Filipínis, Vítneaimis,
Seirbis, Liotuáinis, Laitvis, Eastóinis, Máltais, Albáinis, Airméinis,
Seoirsis, Íoslainnis, Gaeilge, Breatnais, Bascais, Catalóinis, Gailísis,
Laidin, Esperanto, Bealarúisis, Macadóinis, Slóivéinis, Svahaílis, Araibis,
Eabhrais, Peirsis, Urdúis, Hiondúis, Maraitis, Beangáilis, Puinseáibis,
Gúisearáitis, Tamailis, Teileagúis, Téalainnis, Ciméiris, Amáiris nó Cantainis.
Leanann **Córas** teanga do ríomhaire. Níl téacs cluiche an atheisiúna
2021 aistrithe ach go Fraincis, Gearmáinis, Iodáilis, Spáinnis agus Rúisis;
taispeánann teangacha eile i mBéarla é. I dteangacha seachas Béarla,
úsáideann an téacs beag sa chluiche cló na roghchlár, mar níl ag cló
clasaiceach Quake ach litreacha Laidineacha gan síntí fada ná blaiseanna.
Fanann teachtaireachtaí an chonsóil i mBéarla mura bhfuil **Aistrigh an
Consól** air.

## Pacáistí Léarscáileanna (brabhsálaí modanna)

Liostaíonn Príomhroghchlár → Pacáistí Léarscáileanna gach mod agus pacáiste
léarscáileanna suiteáilte lena chur síos. Roghnaigh ceann chun aistriú
chuige, ansin brabhsáil a léarscáileanna agus tosaigh aon cheann acu go
díreach. Coinníonn an brabhsálaí cuntas ar do mharuithe, do rúin agus d'am
is fearr in aghaidh na léarscáile. Tá a chumraíocht féin ag gach mod, mar
sin ní théann sannadh eochracha ná socruithe a shocraigh tú do mhod amháin
isteach i mod eile.

**Cá dtéann pacáistí léarscáileanna.** Lorgaíonn an calafort iad i
mbunfhillteán Quake (an ceann ina bhfuil `id1/`, socraithe le
`QUAKE_BASEDIR` nó `-basedir`). Liostaítear gach fofhillteán ansin ina
bhfuil sonraí cluiche (comhad `.pak` nó `.pk3`, `progs.dat`, nó fillteán
`maps/`), mar sin suiteáil pacáiste léarscáileanna trína dhíphacáil isteach
ina fhillteán féin in aice le `id1/`, mar shampla `<basedir>/ad/` nó
`<basedir>/my_maps/maps/*.bsp`. Más mian leat, cuir `modinfo.txt` leis le
línte `title`, `base` agus `description` chun an chaoi a liostaítear é a
rialú. Aimsítear pacáistí nuair a thosaíonn an cluiche, mar sin atosaigh tar
éis ceann a chur leis. Coinnítear sábhálacha, gabhálacha scáileáin agus
cumraíocht gach pacáiste in `~/.local/share/quake-source-port/<fillteán>/`,
ní i bhfillteán an phacáiste.

## Scáileán scoilte

Suas le seisear imreoirí ar aon ríomhaire amháin, i gcomhoibriú nó i
deathmatch.

- Socraíonn Multiplayer → New Game → **Imreoirí Áitiúla** cé mhéad duine a
  imríonn.
- Faigheann gach imreoir a radharc, a HUD, a fhuaim agus a ghléas ionchuir
  féin (méarchlár agus luch d'imreoir 1, rialtán an duine don chuid eile).
- Is féidir leagan amach an scáileáin a bheith cruachta, taobh le taobh,
  2×2 nó 3×2 (`r_splitscreen_layout`).
- Roghnaíonn gach imreoir ainm agus dathanna léine/bríste faoi Multiplayer →
  Setup.

## Rialtáin

Tá an calafort ceaptha le himirt ar cheap stiúrtha.

- **Roth arm** (`+weaponwheel`): coinnigh é chun an t-am a mhoilliú agus
  arm a roghnú le ceachtar maide nó leis an luch; scaoil chun athrú.
- **Cúnamh Aimsithe** (Options → Ionchur → Rialtán): moilliú in aice le
  naimhde agus rothlú a leanann iad. Ní bhaineann sé ach leis an maide,
  riamh leis an luch.
- **Aimsiú Gíreascóip** (Options → Ionchur → Rialtán → Gíreascóp): lasc,
  móide íogaireacht ar leith do na haiseanna luascadh, tuinairde agus rolla
  (tá rolla as de réir réamhshocraithe agus casann sé an radharc mar a
  dhéanann luascadh; aisiompaíonn luachanna diúltacha é), scagaire torainn,
  agus cathain a bhíonn an gíreascóp i bhfeidhm (i gcónaí, as nó ach fad atá
  `+gyroaction` coinnithe). Aistríonn **Stíl Aimsithe** an maide deas idir
  aimsiú 2T (casann sé an radharc go leanúnach) agus **smeach-mhaide** (brúigh
  an maide chun an radharc a léim i dtreo an treo sin, ansin cas an maide
  chun é a rothlú); oibríonn an dá cheann go maith leis an ngíreascóp le
  haghaidh aimsiú mín.
- **Crith**, íogaireacht agus inbhéartú an mhaide.
- Meaitseálann **Siombailí Cnaipí** do cheap (Xbox, PlayStation, Nintendo)
  agus braitear go huathoibríoch iad.
- Athcheangail gach rud in Options → Rialuithe, ón rialtán féin freisin.
- Rialuithe tadhaill le méid, teimhneacht agus luas amhairc inathraithe.

## Sábháil

- Rothlaíonn **uathshábhálacha** ar leith ó shábhálacha de láimh agus ó
  mhear-shábhálacha, mar sin ní fhorscríobhann siad do chuid féin choíche.
  Roghnaíonn Options → Imirt → Socruithe Uathshábhála cé mhéad atá le
  coinneáil agus cathain a shábháiltear: ag tús leibhéil, ar amadóir, ar rún
  nó gach rún a aimsiú, agus ar gach namhaid a mharú.
- Taispeánann **cártaí sábhála** sna roghchláir Load/Save leibhéal, sláinte,
  armúr, airm, eochracha agus maruithe/rúin gach sábhála.
- **Ar Bhás** (Options → Imirt → Ar Bhás), is féidir leis an aonimreoir
  fiafraí céard is ceart a dhéanamh i roghchlár ar sos, an sábháil is déanaí
  a lódáil, ligean duit sábháil a roghnú, nó an leibhéal a atosú.

## Imirt agus ceamara

Options → Imirt:

- Réimse amhairc 60–130, luascadh amhairc, cic an ghunna, rith i gcónaí.
- **Ceamara**: an chéad phearsa, an tríú pearsa, nó thar an ngualainn (deas
  nó clé).
- Suíomh na **samhla airm**: lár, deas, clé nó i bhfolach.

## Saincheap: mionathraitheoirí imeartha agus breiseáin

Bíonn cluiche *bunúsach* nó *saincheaptha*, rud a roghnaítear nuair a
thosaíonn sé. Taispeánann New Game (agus leibhéal a roghnú in Roghnú
Leibhéil nó in Pacáistí Léarscáileanna) preabfhuinneog: tosaíonn **Imir** an
cluiche, osclaíonn **Saincheap an tEispéireas** Mionathraitheoirí Imeartha
agus Breiseáin. Fad atá aon rud athraithe, cuirtear an chlib **SAINCHEAPTHA**
ar Imir agus cuireann **Díchumasaigh Gach Athrú** (Del, X ar an gceap, nó
cliceáil) na rialacha caighdeánacha ar ais gan bhreiseáin. Sábhálann cluichí
bunúsacha gnáthchomhaid `.sav` ar féidir le ironwail agus vkQuake iad a
lódáil; sábhálann cluichí saincheaptha comhaid `.qsa` nach lódálann ach an
calafort seo (mar a dhéanann cluichí scáileáin scoilte agus comhoibrithe,
nach féidir le calafoirt eile a shábháil). Le linn cluiche shaincheaptha,
athraíonn Príomhroghchlár → Saincheap a mhionathraitheoirí agus a bhreiseáin
am ar bith; fanann rialacha cluiche bhunúsaigh socraithe. Oibríonn
mionathraitheoirí le haon mhod agus múchtar iad i deathmatch.

- Níos deacra: níos mó damáiste faighte, naimhde níos righne, níos lú
  armlóin agus sláinte le piocadh, arrachtaí x2–x4, tús le piostal i ngach
  leibhéal, gan sábháil de láimh.
- Níos éasca: níos lú damáiste faighte, naimhde níos laige, níos mó armlóin
  agus sláinte, armlón gan teorainn, athghiniúint sláinte, gan damáiste
  titime, gan bá, gan féindamáiste, léimeanna dúbailte nó faoi thrí.

## Taispeántais

Taifeadann `record <name>` agus `stop` taispeántas i d'fhillteán úsáideora;
seinneann `playdemo <name>` é. Nuair a stopann an taifeadadh, athscríobhtar
an taispeántas sa chúlra ionas gur féidir le ironwail agus vkQuake é a
sheinm freisin. Má úsáideann mod maisíochtaí cáithníní nach féidir le
calafoirt eile a thaispeáint, coinnítear an leagan iomlán mar `<name>.qsd`
(`playdemo <name>.qsd`). Tiontaíonn `demo_convert <name>` taispeántas níos
sine.

Taispeánann Príomh-roghchlár → Taispeántais na taispeántais atá sábháilte
agat agus, nuair atá Roghanna → Taispeántais → Taifead taispeántais go
huathoibríoch ar siúl, taispeántas uathoibríoch de gach leibhéal a imríonn
tú; cuirtear na cinn is sine as nuair a bhíonn na sliotáin lán. Clóscríobh
chun na liostaí a scagadh agus brúigh Tab chun an t-ord a athrú; seinneann,
easpórtálann, athainmníonn, coinníonn nó scriosann na cnaipí in aice leis
an liosta an taispeántas roghnaithe, agus coinnítear taispeántas
uathoibríoch a athainmnítear. Bíonn taispeántas uathoibríoch an leibhéil
reatha i dtuairiscí fabhtanna.

Le linn taispeántais, cuireann Spás ar sos é, athchasann nó brostaíonn Clé
nó Deas á choinneáil síos, athraíonn Suas agus Síos an luas, léimeann PgUp
agus PgDn 10 soicind, agus téann Home agus End go dtí an tús agus go dtí an
chéad léarscáil eile. Marcálann I agus O gearrthóg; easpórtálann E í, nó an
taispeántas ar fad, chuig an bhfillteán `videos`. Fanann na rialtáin ar an
scáileán; folaíonn H nó Folaigh iad, agus nuair a dhíríonn tú ar an imeall
íochtair taispeántar stiall a thugann ar ais iad. Cliceáil na cnaipí,
cliceáil an amlíne chun léim ann, nó tarraing trasna uirthi chun gearrthóg a
aibhsiú; fiafraíonn Easpórtáil an ráta frámaí, an mhéid, an fhormáid agus an
comhad, agus cé acu an ghearrthóg aibhsithe nó an taispeántas ar fad atá le
heaspórtáil.

Ar rialaitheoir, cuireann A ar sos é, téann an d-pad siar agus ar aghaidh
agus athraíonn sé an luas, léimeann LB agus RB 10 soicind, agus téann cnaipí
na maidí go dtí an tús agus go dtí an chéad léarscáil eile. Marcálann LT
agus RT gearrthóg, glanann B í, easpórtálann X, coinníonn Y taispeántas
uathoibríoch agus folaíonn View na rialuithe.

Tosaíonn agus stopann `capture` taifeadadh físe agus tú ag imirt. Tá ráta
frámaí, formáid agus cáilíocht faoi Roghanna → Taispeántais → Taifeadadh
físe. Teastaíonn ffmpeg ó fhíseáin; gan é sábháiltear na frámaí mar íomhánna
PNG le comhad WAV.

## Inrochtaineacht

Options → Inrochtaineacht:

- Gluaiseacht laghdaithe agus déine splancanna scáileáin.
- Scagairí do dhathdhaille.
- Crosribe inchumraithe: cruth, méid, imlíne, agus athrú datha ar sprioc nó
  ar mharú; marcóirí buailte roghnacha.
- **Fotheidil Fuaime**; socraítear suíomh, méid agus fad teachtaireachtaí
  agus fotheideal san eagarthóir HUD.
- Ionsaí agus rith le scoránú (seachas coinneáil).
- Sleamhnán luas an chluiche chun an cluiche ar fad a mhoilliú.
- Uathaimsiú le táscaire glasála, agus socruithe cóin agus raoin.

## Feabhsuithe

Options → Feabhsuithe. Tá gach rud ar thaobh an chliaint agus as de réir
réamhshocraithe, mar sin oibríonn siad le haon mhod agus ar líne.

- **Smionagar**: giotaí feola agus sceanfhéar ag rolladh ó bhairillí
  briste, le harrachtaí modanna freisin; is féidir leis bualadh in aghaidh
  doirse agus ardaitheoirí.
- **Fuil fheabhsaithe**: steallta ar dhromchlaí, créachtaí a leanann coirp,
  braonta san aer a phleascann san áit a dtuirlingíonn siad.
- **Maisíochtaí tuairte**: rianta piléar gunna gráin, tairní a théann
  isteach in adhmad agus i naimhde, rianta dóite pléasctha.
- **Uisce Feabhsaithe**: tréshoilseacht, cuilithíní, steallta agus cáistic.
- **Teileapórtálaithe tréshoilseacha** a thaispeánann a gceann scríbe le
  parallax.
- Coimhthiú comhthimpeallach, maolú gluaisne in aghaidh an réada agus
  luais, cáithníní boga.
- **Scáthóirí scáileáin**: réamhshocruithe `.slangp` RetroArch (CRT agus
  eile).

Tá na gnáthshocruithe in Options → Grafaic freisin: scála rindreála,
frith-ailiasáil, forshampláil, scagadh uigeachtaí, dath 8-giotán le
díodánú, ceo, soilsiú daite agus idirshuíomh beochana/gluaiseachta.

## Breiseáin

Is airm agus trealamh nua iad breiseáin, á rith ag an inneall ionas go
n-oibríonn siad le progs aon mhoid. Aonimreoir agus comhoibriú amháin.

### Cluiche a thosú le breiseáin

Ní bhíonn breiseáin i gcluiche ach amháin má thosaíonn sé leo:

1. Single Player → cuir tic sa bhosca **Breiseáin**, ansin New Game.
2. Roghnaigh na breiseáin atá uait agus coigeartaigh a socruithe, ansin
   Start New Game.

Sábháiltear cluichí le breiseáin mar chomhaid `.qsa` in ionad `.sav`, ionas
nach ndéanann calafoirt eile iarracht iad a lódáil. Taispeánann cártaí
sábhála suaitheantas "ADD-ONS". Múchann lódáil gnáthshábhála na breiseáin.
Le linn cluiche le breiseáin, cuireann an leathanach Breiseáin (consól:
`menu_options addons`) breiseáin aonair air agus as; tá réamhamharc lúbtha,
treoracha, a shannadh eochracha agus a shocruithe féin ag gach ceann acu.

### Airm

Téann breiseáin sna gnáthshliotáin airm (roghnaigh ceann do gach breiseán,
nó sliotán 9 agus 0) agus taispeántar iad ar an roth arm agus ar an HUD. Is
iad Lámhach agus Lámhach Malartach do ghnáthchnaipí ionsaithe.

| Breiseán | Lámhach | Lámhach Malartach | Armlón |
|---|---|---|---|
| **Gunna Tairseach** | Cuir an tairseach ghorm | Cuir an tairseach oráiste | Dada |
| **Railgun** | Urchar a théann trí gach rud ar líne | — | 2 chill |
| **Crúca** | Greamaigh agus tarraing tú féin chuig adhmad, miotal, cloch nó naimhde | — | Dada |
| **Lainseálaí Greambhuamaí** | Coinnigh le luchtú, scaoil le lainseáil | Pléasc | 1 roicéad |
| **Búmarang** | Coinnigh le naimhde a mharcáil, scaoil le caitheamh; cuireann sé néal ar gach ceann ina sheal | Glan marcanna | Dada |
| **Spréire Glóthaigh** | Glóthach aisbhrú gorm (preabach) | Glóthach tiomána oráiste (tapa) | Dada |
| **Glantóir Ardbhrú** | Scaird fhada uisce a bhrúnn naimhde agus a níonn glóthach | Gaothrán leathan scaird | Dada |

Téann tairseacha ar dhromchlaí cothroma an domhain agus iompraíonn siad thú,
arrachtaí agus rudaí caite tríothu lena móiminteam. Le **Féach Trí
Thairseacha** air feiceann tú an taobh eile beo, tairseacha atá le feiceáil
trí thairseacha san áireamh; is féidir an cháilíocht, líon na radharc beo,
an fad agus doimhneacht na hathchúrsála a choigeartú.

### Trealamh

Cuirtear trealamh air agus as seachas é a choinneáil i do láimh. Oscail an
**roth trealaimh** (`+equipwheel`), nó úsáid `goggles cycle` chun dul tríd:

- **Radharc oíche**
- **Radharc teirmeach**

## Imirt ilimreora

Seachas imirt NetQuake ar LAN agus ar an idirlíon (prótacail 15, 666 agus
999) ceanglaíonn an calafort le freastalaithe **QuakeWorld**. Glacann
Multiplayer → Join Game le seoladh nó cuardaíonn sé an LAN. Tacaítear le
cáithníní scriptithe FTE do mhodanna a úsáideann iad.

## Líne na n-orduithe agus an consól

Roghanna coitianta: `-basedir <dir>`, `-game <dir>`, `-hipnotic` / `-rogue` /
`-quoth`, `-width <w>`, `-height <h>`, `-fullscreen`, `-window`, agus
`+command args` chun orduithe consóil a rith ag an tosú. Glacann an consól
(`~`) leis na gnáthorduithe agus cvaranna Quake.

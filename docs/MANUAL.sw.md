# Mwongozo wa mtumiaji

Mwongozo huu unaeleza jinsi ya kucheza Quake kwa port hii, hasa kile
inachoongeza ikilinganishwa na Quake ya asili na source ports nyingine.
Kila kitu kipya ni cha hiari: kwa mipangilio ya chaguo-msingi mchezo
unaonekana na kuchezwa kama Quake.

## Kuanza

1. Pata nakala yako mwenyewe ya Quake. Ielekeze port kwenye saraka yenye
   `id1/` (na mods zozote kando yake), kwa `QUAKE_BASEDIR` au
   `-basedir <saraka>`.
2. Endesha `cargo run --release`, au anzisha mod moja kwa moja kwa
   `-game <saraka> +map <ramani>`.
3. Mipangilio, hifadhi na picha za skrini huwekwa katika
   `~/.local/share/quake-source-port/<game>/`, folda moja kwa kila mod.

Menyu kuu ina Single Player, Multiplayer, Options, **Help** (mwongozo huu),
Quit na **Vifurushi vya Ramani**, huku wimbo wa Quake ukicheza nyuma yake
(Options → Sauti → Muziki wa Menyu Kuu). Kila menyu inafanya kazi kwa
kipanya, kibodi, kidhibiti na mguso; elea juu ya chaguo au ulichague ili
kuona maelezo yake.

Katika Help, juu/chini huchagua sehemu; Page Up/Page Down, kushoto/kulia,
vitufe vya bega au gurudumu la kipanya juu ya maandishi huyasogeza.

## Lugha

Options → Uchezaji → **Lugha** huchagua lugha ya menyu, ujumbe wa skrini na
mwongozo huu: Kiingereza, Kifaransa, Kijerumani, Kiitaliano, Kihispania,
Kirusi, Kipolandi, Kiukreni, Kicheki, Kislovakia, Kikroeshia, Kibulgaria,
Kiswidi, Kifini, Kidenmaki, Kinorwe, Kiholanzi, Kigiriki, Kihungaria,
Kiromania, Kireno (Ureno au Brazili), Kijapani, Kikorea, Kichina
(Kilichorahisishwa au cha Jadi), Kituruki, Kiindonesia, Kimalei,
Kifilipino, Kivietinamu, Kiserbia, Kilithuania, Kilatvia, Kiestonia,
Kimalta, Kialbania, Kiarmenia, Kijojia, Kiaislandi, Kiayalandi, Kiwelisi,
Kibaski, Kikatalani, Kigalisia, Kilatini, Kiesperanto, Kibelarusi,
Kimasedonia, Kislovenia, Kiswahili, Kiarabu, Kiebrania, Kiajemi, Kiurdu,
Kihindi, Kimarathi, Kibengali, Kipunjabi, Kigujarati, Kitamil, Kitelugu,
Kithai, Kikhmer, Kiamhari au Kikantoni.
**Mfumo** hufuata lugha ya kompyuta yako. Maandishi ya mchezo ya toleo jipya
la 2021 yametafsiriwa kwa Kifaransa, Kijerumani, Kiitaliano, Kihispania na
Kirusi pekee; lugha nyingine huyaonyesha kwa Kiingereza. Katika lugha
zisizo Kiingereza, maandishi madogo ya ndani ya mchezo hutumia fonti ya
menyu, kwa kuwa fonti ya asili ya Quake ina herufi za Kilatini zisizo na
alama tu. Ujumbe wa konsoli hubaki kwa Kiingereza isipokuwa **Tafsiri
Konsoli** imewashwa.

## Vifurushi vya Ramani (kivinjari cha mods)

Menyu kuu → Vifurushi vya Ramani huorodhesha kila mod na kifurushi cha
ramani kilichosakinishwa pamoja na maelezo yake. Chagua kimoja ili
kuhamia kwake, kisha vinjari ramani zake na uanzishe yoyote moja kwa moja.
Kivinjari hufuatilia rekodi zako bora za waliouawa, siri na muda kwa kila
ramani. Kila mod ina mipangilio yake, kwa hivyo vitufe na mipangilio
uliyoweka kwa mod moja haivuji kwenda nyingine.

**Mahali vifurushi vya ramani vinapowekwa.** Port huvitafuta katika saraka
ya msingi ya Quake (ile yenye `id1/`, iliyowekwa kwa `QUAKE_BASEDIR` au
`-basedir`). Kila folda ndogo humo yenye data ya mchezo (faili `.pak` au
`.pk3`, `progs.dat`, au folda `maps/`) huorodheshwa, kwa hivyo sakinisha
kifurushi cha ramani kwa kukifungua ndani ya folda yake kando ya `id1/`,
kwa mfano `<basedir>/ad/` au `<basedir>/my_maps/maps/*.bsp`. Ukipenda,
ongeza `modinfo.txt` yenye mistari ya `title`, `base` na `description` ili
kudhibiti jinsi kinavyoorodheshwa. Vifurushi hupatikana mchezo unapoanza,
kwa hivyo anzisha upya baada ya kuongeza kimoja. Hifadhi, picha za skrini
na mipangilio ya kila kifurushi huwekwa katika
`~/.local/share/quake-source-port/<folder>/`, si ndani ya folda ya
kifurushi.

## Skrini iliyogawanywa

Hadi wachezaji sita kwenye mashine moja, kwa ushirikiano au deathmatch.

- Multiplayer → New Game → **Wachezaji wa Hapa** huweka idadi ya watu
  wanaocheza.
- Kila mchezaji hupata mwonekano, HUD, sauti na kifaa cha kuingiza
  vyake (kibodi na kipanya kwa mchezaji 1, kidhibiti kimoja kwa kila
  mmoja wa wengine).
- Mpangilio wa skrini unaweza kuwa juu-chini, kando kwa kando, 2×2 au 3×2
  (`r_splitscreen_layout`).
- Kila mchezaji huchagua jina na rangi za shati/suruali katika
  Multiplayer → Setup.

## Vidhibiti

Port hii imeundwa kuchezwa kwa kidhibiti cha mchezo.

- **Gurudumu la Silaha** (`+weaponwheel`): shikilia ili kupunguza kasi ya
  muda na uchague silaha kwa stick yoyote au kipanya; achilia kubadilisha.
- **Usaidizi wa Kulenga** (Options → Ingizo → Kidhibiti): kupunguza kasi
  karibu na maadui na mzunguko unaowafuata. Unatumika kwa stick tu, kamwe
  si kwa kipanya.
- **Kulenga kwa Gyro** (Options → Ingizo → Kidhibiti → Gyro): swichi, pamoja
  na unyeti tofauti kwa mihimili ya yaw, pitch na roll (roll imezimwa kwa
  chaguo-msingi na hugeuza mwonekano kama yaw, thamani hasi hugeuza
  mwelekeo), kichujio cha kelele, na wakati gyro inatumika (daima, zima au
  tu ukishikilia `+gyroaction`). **Mtindo wa Kulenga** hubadilisha stick ya
  kulia kati ya kulenga 2D (huzungusha mwonekano mfululizo) na **flick
  stick** (sukuma stick ili mwonekano uelekee upande huo papo hapo, kisha
  zungusha stick kuuzungusha); vyote viwili vinaendana vizuri na gyro kwa
  ulengaji makini.
- **Mtetemo**, unyeti wa stick na ugeuzaji.
- **Alama za Vitufe** zinalingana na kidhibiti chako (Xbox, PlayStation,
  Nintendo) na hutambuliwa kiotomatiki.
- Panga upya kila kitu katika Options → Vidhibiti, hata kutoka kwenye
  kidhibiti.
- Vidhibiti vya skrini ya mguso vyenye ukubwa, uangavu na kasi ya kuangalia
  vinavyorekebishika.

## Kuhifadhi

- **Hifadhi za Kiotomatiki** huzunguka kando na hifadhi za mkono na hifadhi
  za haraka, kwa hivyo haziandiki kamwe juu ya zako. Options → Uchezaji →
  Mipangilio ya Kuhifadhi Kiotomatiki huchagua ngapi zibaki na wakati wa
  kuhifadhi: ukiingia ngazi, kwa kipima muda, ukipata siri au siri zote, na
  ukiua kila adui.
- **Kadi za hifadhi** katika menyu za Kupakia/Kuhifadhi huonyesha ngazi,
  afya, deraya, silaha, funguo na waliouawa/siri za kila hifadhi.
- **Ukifa** (Options → Uchezaji → Ukifa), mchezaji mmoja anaweza kuuliza
  la kufanya katika menyu iliyositishwa, kupakia hifadhi ya karibuni,
  kukuacha uchague hifadhi, au kuanza ngazi upya.

## Uchezaji na kamera

Options → Uchezaji:

- Uga wa mwonekano 60–130, mtikisiko wa mwonekano, kurudi kwa silaha,
  kimbia daima.
- **Kamera**: mtu wa kwanza, mtu wa tatu, au juu ya bega (kulia au
  kushoto).
- Nafasi ya **Modeli za Silaha**: katikati, kulia, kushoto au zimefichwa.

## Binafsisha: virekebishaji vya mchezo na viongezi

Mchezo ni wa *kawaida* au *uliobinafsishwa*, inavyochaguliwa unapoanza. New
Game (na kuchagua ngazi katika Chagua Ngazi au Vifurushi vya Ramani)
huonyesha dirisha: **Cheza** huanzisha mchezo, **Binafsisha Uzoefu**
hufungua Virekebishaji vya Mchezo na Viongezi. Kitu chochote
kikibadilishwa, Cheza huwekewa alama **MAALUM** na **Zima Mabadiliko
Yote** (Del, X ya kidhibiti, au kubofya) hurejesha sheria za kawaida bila
viongezi. Michezo ya kawaida huhifadhi faili za kawaida za `.sav` ambazo
ironwail na vkQuake zinaweza kupakia; michezo iliyobinafsishwa huhifadhi
faili za `.qsa` ambazo port hii pekee hupakia (vivyo hivyo michezo ya
skrini iliyogawanywa na ya ushirikiano, ambayo ports nyingine haziwezi
kuhifadhi). Wakati wa mchezo uliobinafsishwa, Menyu kuu → Binafsisha
hubadilisha virekebishaji na viongezi vyake wakati wowote; sheria za
mchezo wa kawaida haziwezi kubadilishwa. Virekebishaji hufanya kazi na mod
yoyote na huzimika katika deathmatch.

- Ngumu zaidi: madhara zaidi unayopata, maadui sugu zaidi, risasi na afya
  chache zinazookotwa, mazimwi mara 2–4, kuanza na bastola kila ngazi,
  hakuna kuhifadhi kwa mkono.
- Rahisi zaidi: madhara machache unayopata, maadui dhaifu zaidi, risasi na
  afya zaidi, risasi zisizoisha, kurejesha afya, hakuna madhara ya
  kuanguka, hakuna kuzama, hakuna kujiumiza, miruko miwili au mitatu.

## Demo

`record <name>` na `stop` hurekodi demo kwenye folda yako ya mtumiaji;
`playdemo <name>` huicheza. Kurekodi kunaposimama, demo huandikwa upya
chinichini ili ironwail na vkQuake ziweze kuicheza pia. Ikiwa mod inatumia
athari za chembechembe ambazo ports nyingine haziwezi kuonyesha, toleo
kamili huhifadhiwa kama `<name>.qsd` (`playdemo <name>.qsd`).
`demo_convert <name>` hubadilisha demo ya zamani.

Menyu kuu → Demo huonyesha demo ulizohifadhi na, Chaguo → Demo → Rekodi demo
kiotomatiki ikiwa imewashwa, demo ya kiotomatiki ya kila kiwango
unachocheza; za zamani zaidi hubadilishwa nafasi zikijaa. Andika ili kuchuja
orodha na ubonyeze Tab ili kubadili mpangilio; vitufe kando ya orodha
hucheza, huhamisha, hubadilisha jina, hubakiza au hufuta demo uliyochagua,
na kubadilisha jina la demo ya kiotomatiki huibakiza. Ripoti za hitilafu
hujumuisha demo ya kiotomatiki ya kiwango cha sasa.

Demo inapocheza, Nafasi husitisha, kushikilia Kushoto au Kulia hurudisha
nyuma au kusonga mbele haraka, Juu na Chini hubadili kasi, PgUp na PgDn
huruka sekunde 10, na Home na End huenda mwanzo na kwenye ramani inayofuata.
I na O huweka alama ya kipande; E hukihamisha, au demo nzima, kwenye folda
ya `videos`. Vidhibiti hubaki kwenye skrini; H au Ficha huvificha, na
kuelekeza ukingo wa chini huonyesha ukanda unaovirudisha. Bofya vitufe,
bofya ratiba ili kuruka hapo, au buruta juu yake ili kuweka alama ya
kipande; Hamisha huuliza kasi ya fremu, ukubwa, muundo na faili, na kama
kuhamisha kipande kilichowekwa alama au demo nzima.

Kwenye kidhibiti, A husitisha, d-pad hurudisha nyuma, husogeza mbele na
hubadili kasi, LB na RB huruka sekunde 10, na vitufe vya vijiti huenda
mwanzo na kwenye ramani inayofuata. LT na RT huweka alama ya kipande, B
hukifuta, X huhamisha, Y hubakiza demo ya kiotomatiki na View huficha
vidhibiti.

`capture` huanzisha na kusimamisha kurekodi video unapocheza. Kasi ya fremu,
muundo na ubora vipo katika Chaguo → Demo → Kurekodi video. Video zinahitaji
ffmpeg; bila hiyo fremu huhifadhiwa kama picha za PNG pamoja na faili ya
WAV.

## Ufikivu

Options → Ufikivu:

- Mwendo uliopunguzwa na ukali wa mimweko ya skrini.
- Vichujio vya upofu wa rangi.
- Kilenga kinachorekebishika: umbo, ukubwa, ukingo, na kubadilika rangi
  ukilenga au ukiua; alama za kugonga za hiari.
- **Manukuu ya Sauti**; nafasi, ukubwa na muda wa ujumbe na manukuu huwekwa
  katika kihariri cha HUD.
- Shambulio na kukimbia kwa kubadili (badala ya kushikilia).
- Kitelezi cha kasi ya mchezo ili kupunguza kasi ya mchezo mzima.
- Kulenga kiotomatiki chenye kiashiria cha kufunga lengo, na mipangilio ya
  koni na masafa.

## Maboresho

Options → Maboresho. Yote ni ya upande wa mteja na yamezimwa kwa
chaguo-msingi, kwa hivyo yanafanya kazi na mod yoyote na mtandaoni.

- **Vifusi**: vipande vya nyama vinavyoviringika na vipande kutoka kwa
  mapipa yaliyovunjika, pia kwa mazimwi ya mods; vinaweza kugonga milango
  na lifti.
- **Matone ya Damu Bora**: madoa kwenye nyuso, majeraha yanayofuata miili,
  matone hewani yanayotapakaa yanapotua.
- **Athari za Kugonga Nyuso**: alama za risasi za shotgun, misumari
  inayokwama kwenye mbao na maadui, alama za kuungua za milipuko.
- **Maji Bora**: uangavu, mawimbi, mirushiko na caustics.
- **Teleporta za Kuona Kupitia** zinazoonyesha zinakoelekea kwa parallax.
- Kivuli cha mazingira, ukungu wa mwendo kwa kila kitu na kwa kasi,
  chembechembe laini.
- **Shader ya Skrini**: mipangilio tayari ya RetroArch `.slangp` (CRT na
  mingine).

Options → Michoro pia ina kipimo cha uonyeshaji, kulainisha kingo,
supersampling, kichujio cha tekstua, rangi za 8-bit na dithering, ukungu,
mwanga wa rangi na kulainisha uhuishaji/mwendo kama kawaida.

## Viongezi

Viongezi ni silaha na vifaa vipya vinavyoendeshwa na injini, kwa hivyo
vinafanya kazi na progs za mod yoyote. Ni vya mchezaji mmoja na ushirikiano
pekee.

### Kuanzisha mchezo wenye viongezi

Mchezo una viongezi tu ukianza navyo:

1. Single Player → weka alama kwenye kisanduku **Viongezi**, kisha New Game.
2. Chagua viongezi vya kuleta na urekebishe mipangilio yake, kisha anzisha
   mchezo mpya.

Michezo yenye viongezi huhifadhiwa kama faili za `.qsa` badala ya `.sav`,
ili ports nyingine zisijaribu kuzipakia. Kadi za hifadhi huonyesha beji
"ADD-ONS". Kupakia hifadhi ya kawaida huzima viongezi. Ukicheza mchezo
wenye viongezi, ukurasa wa Viongezi (konsoli: `menu_options addons`)
huwasha na kuzima kiongezi kimoja kimoja; kila kimoja kina onyesho la
mzunguko, maelekezo, vitufe vyake na mipangilio yake.

### Silaha

Viongezi huingia kwenye nafasi za kawaida za silaha (chagua kwa kila
kiongezi, au nafasi 9 na 0) na huonekana kwenye gurudumu la silaha na HUD.
Fyatua na mlipuo mbadala ni vitufe vyako vya kawaida vya shambulio.

| Kiongezi | Fyatua | Mlipuo Mbadala | Risasi |
|---|---|---|---|
| **Bunduki ya Lango** | Huweka lango la bluu | Huweka lango la machungwa | Hakuna |
| **Railgun** | Risasi inayopenya inayogonga kila kitu kwenye mstari | — | Seli 2 |
| **Ndoano** | Hunasa na kukuvuta kwenye mbao, chuma, jiwe au maadui | — | Hakuna |
| **Kirusha Mabomu Nata** | Shikilia kuchaji, achilia kurusha | Lipua | Roketi 1 |
| **Bumerangi** | Shikilia kuwawekea maadui alama, achilia kurusha; humduwaza kila mmoja kwa zamu | Futa alama | Hakuna |
| **Kinyunyizi cha Jeli** | Jeli ya bluu ya kurudisha (ya kudunda) | Jeli ya machungwa ya kusukuma (ya kasi) | Hakuna |
| **Powerwasher** | Mkondo mrefu wa maji unaosukuma maadui na kuosha jeli | Mikondo mipana | Hakuna |

Malango huwekwa kwenye nyuso bapa za ulimwengu na hukupitisha wewe, mazimwi
na vitu vilivyorushwa pamoja na kasi yao. **Ona Kupitia Malango**
ikiwashwa unaona upande mwingine moja kwa moja, ikijumuisha malango
yanayoonekana kupitia malango; ubora, idadi ya mionekano hai, umbali na
kina cha marudio vinaweza kurekebishwa.

### Vifaa

Vifaa huwashwa na kuzimwa badala ya kushikwa mkononi. Fungua **Gurudumu
la Vifaa** (`+equipwheel`), au tumia `goggles cycle` kuzungusha kati ya:

- **Maono ya usiku**
- **Maono ya joto**

## Wachezaji wengi

Mbali na mchezo wa NetQuake kwenye LAN na intaneti (itifaki 15, 666 na
999), port huunganishwa na seva za **QuakeWorld**. Multiplayer → Join Game
hupokea anwani au hutafuta kwenye LAN. Chembechembe za hati za FTE
zinaungwa mkono kwa mods zinazozitumia.

## Mstari wa amri na konsoli

Chaguo za kawaida: `-basedir <dir>`, `-game <dir>`, `-hipnotic` / `-rogue` /
`-quoth`, `-width <w>`, `-height <h>`, `-fullscreen`, `-window`, na
`+command args` kuendesha amri za konsoli wakati wa kuanza. Konsoli (`~`)
inakubali amri na cvars za kawaida za Quake.

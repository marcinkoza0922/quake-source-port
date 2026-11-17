# Manwal ng user

Ipinapaliwanag ng manwal na ito kung paano laruin ang Quake gamit ang port
na ito, lalo na ang mga idinadagdag nito sa vanilla Quake at sa ibang source
port. Opsyonal ang lahat ng bago: sa mga default na setting, mukha at
nilalaro ang laro gaya ng Quake.

## Pagsisimula

1. Kumuha ng sarili mong kopya ng Quake. Ituro ang port sa folder na
   may `id1/` (at anumang mod sa tabi nito), gamit ang `QUAKE_BASEDIR` o
   `-basedir <dir>`.
2. Patakbuhin ang `cargo run --release`, o magsimula agad ng mod gamit ang
   `-game <dir> +map <map>`.
3. Nasa `~/.local/share/quake-source-port/<game>/` ang config, save at
   screenshot, isang folder kada mod.

May Single Player, Multiplayer, Options, **Help** (ang manwal na ito), Quit
at **Mga Map Pack** ang main menu, habang tumutugtog sa likod ang tema ng
Quake (Options → Audio → Musika ng Main Menu). Gumagana ang bawat menu sa
mouse, keyboard, controller at touch; i-hover o piliin ang isang opsyon para
makita ang tooltip na nagpapaliwanag dito.

Sa Help, pumipili ng seksyon ang pataas/pababa; ini-scroll ang teksto ng Page
Up/Page Down, kaliwa/kanan, mga shoulder button o ng mouse wheel sa ibabaw
ng teksto.

## Wika

Pinipili ng Options → Gameplay → **Wika** ang wika ng mga menu, mensahe sa
screen at ng manwal na ito: Ingles, Pranses, Aleman, Italyano, Espanyol,
Ruso, Polako, Ukranyano, Tseko, Slovak, Kroata, Bulgaro, Suweko,
Finnish, Danes, Norwegian, Olandes, Griyego, Hungarian, Romanian, Portuges
(Portugal o Brazil), Hapones, Koreano, Tsino (Simplified o
Traditional), Turko, Indonesian, Malay, Filipino, Vietnamese,
Serbian, Lithuanian, Latvian, Estonian, Maltese, Albanian, Armenian,
Georgian, Icelandic, Irish, Welsh, Basque, Catalan, Galician, Latin,
Esperanto, Belarusian, Macedonian, Slovenian, Swahili, Arabe, Hebreo,
Persian, Urdu, Hindi, Marathi, Bengali, Punjabi, Gujarati, Tamil, Telugu,
Thai, Khmer, Amharic o Cantonese.
Sinusunod ng **System** ang wika ng computer mo. Sa Pranses, Aleman,
Italyano, Espanyol at Ruso lang isinalin ang sariling teksto ng laro ng 2021
re-release; sa ibang wika, Ingles ito. Sa mga wikang hindi Ingles, ginagamit
ng maliit na teksto sa laro ang font ng menu, dahil walang tuldik ang mga
letrang Latin ng klasikong font ng Quake. Mananatiling Ingles ang mga
mensahe ng console maliban kung naka-on ang **Isalin ang Console**.

## Mga Map Pack (mod browser)

Inililista ng Main menu → Mga Map Pack ang bawat naka-install na mod at map
pack kasama ang paglalarawan nito. Pumili ng isa para lumipat dito, saka
tingnan ang mga mapa nito at simulan agad ang alinman sa mga ito. Itinatala
ng browser ang pinakamagaling mong bilang ng patay, lihim at oras sa bawat
mapa. May sariling config ang bawat mod, kaya hindi tumatagas sa ibang mod
ang mga bind at setting na inayos para sa isa.

**Saan inilalagay ang mga map pack.** Hinahanap ng port ang mga ito sa base
folder ng Quake (ang may `id1/`, itinakda gamit ang `QUAKE_BASEDIR` o
`-basedir`). Nakalista ang bawat subfolder doon na may game data (isang
`.pak` o `.pk3` file, isang `progs.dat`, o isang `maps/` folder), kaya
mag-install ng map pack sa pamamagitan ng pag-unpack nito sa sarili nitong
folder sa tabi ng `id1/`, halimbawa `<basedir>/ad/` o
`<basedir>/my_maps/maps/*.bsp`. Puwede ring magdagdag ng `modinfo.txt` na
may mga linyang `title`, `base` at `description` para kontrolin kung paano
ito nakalista. Hinahanap ang mga pack kapag nagsimula ang laro, kaya
i-restart pagkatapos magdagdag. Nasa
`~/.local/share/quake-source-port/<folder>/` ang save, screenshot at config
ng bawat pack, hindi sa folder ng pack.

## Split-screen

Hanggang anim na manlalaro sa iisang makina, sa co-op o deathmatch.

- Itinatakda ng Multiplayer → New Game → **Lokal na Manlalaro** kung ilan
  ang maglalaro.
- May sariling tanaw, HUD, tunog at input device ang bawat manlalaro
  (keyboard at mouse para sa manlalaro 1, tig-isang controller para sa iba).
- Puwedeng patong-patong, magkatabi, 2×2 o 3×2 ang layout ng screen
  (`r_splitscreen_layout`).
- Pumipili ang bawat manlalaro ng pangalan at kulay ng damit/pantalon sa
  Multiplayer → Setup.

## Mga Controller

Ginawa ang port para laruin sa gamepad.

- **Weapon Wheel** (`+weaponwheel`): pindutin nang matagal para pabagalin
  ang oras at pumili ng armas gamit ang alinmang stick o ang mouse; bitawan
  para magpalit.
- **Tulong sa Puntirya** (Options → Input → Controller): pagbagal malapit sa
  kalaban at pag-ikot na sumusunod sa kanila. Sa stick lang ito gumagana,
  hindi kailanman sa mouse.
- **Gyro na Puntirya** (Options → Input → Controller → Gyro): isang toggle,
  dagdag ang hiwalay na sensitivity para sa yaw, pitch at roll axis (naka-off
  ang roll bilang default at pinipihit nito ang tanaw gaya ng yaw; binabaligtad
  ito ng negatibong halaga), isang noise filter, at kung kailan gumagana ang
  gyro (palagi, off o habang hawak lang ang `+gyroaction`). Pinagpapalit ng
  **Istilo ng Puntirya** ang kanang stick sa pagitan ng 2D na puntirya
  (tuloy-tuloy nitong pinipihit ang tanaw) at **flick stick** (itulak ang
  stick para agad ituro ang tanaw sa direksyong iyon, saka ikutin ang stick
  para paikutin ito); bagay na bagay ang dalawa sa gyro para sa pinong
  puntirya.
- **Vibration**, sensitivity ng stick at pagbaligtad.
- Tumutugma ang **Simbolo ng Button** sa pad mo (Xbox, PlayStation,
  Nintendo) at awtomatikong natutukoy.
- I-rebind ang lahat sa Options → Mga Kontrol, kahit mula sa controller.
- Touchscreen na kontrol na may naaayos na laki, opacity at bilis ng tingin.

## Pag-save

- Hiwalay na umiikot ang mga **Autosave** sa manual save at quicksave, kaya
  hindi nila kailanman pinapatungan ang sarili mong save. Pinipili ng
  Options → Gameplay → Mga Setting ng Autosave kung ilan ang itatabi at
  kailan magse-save: sa pagpasok sa level, sa timer, sa pagkakita ng isang
  lihim o lahat ng lihim, at sa pagpatay sa bawat kalaban.
- Ipinapakita ng mga **save card** sa Load/Save menu ang level, health,
  armor, armas, susi at patay/lihim ng bawat save.
- **Sa Pagkamatay** (Options → Gameplay → Sa Pagkamatay), sa single player,
  puwedeng magtanong kung ano ang gagawin sa naka-pause na menu, i-load ang
  pinakabagong save, hayaan kang pumili ng save, o ulitin ang level.

## Gameplay at camera

Options → Gameplay:

- Field of view 60–130, pag-alog ng tanaw, sipa ng baril, laging tumakbo.
- **Camera**: first person, third person, o sa ibabaw ng balikat (kanan o
  kaliwa).
- Posisyon ng **Viewmodel**: gitna, kanan, kaliwa o nakatago.

## I-customize: mga modifier ng gameplay at add-on

Ang isang laro ay *vanilla* o *naka-customize*, na pinipili sa pagsisimula
nito. Nagpapakita ng pop-up ang New Game (at ang pagpili ng level sa Pumili
ng Level o Mga Map Pack): sinisimulan ng **Laruin** ang laro, binubuksan ng
**I-customize ang Karanasan** ang Mga Modifier ng Gameplay at Mga Add-on.
Habang may binago, may tatak na **CUSTOM** ang Laruin at ibinabalik ng
**I-disable ang Lahat ng Pagbabago** (Del, gamepad X, o click) ang
karaniwang patakaran nang walang add-on. Nagse-save ang vanilla na laro ng
ordinaryong `.sav` file na kayang i-load ng ironwail at vkQuake; nagse-save
ang naka-customize na laro ng `.qsa` file na port na ito lang ang
naglo-load (gayundin ang split-screen at co-op na laro, na hindi kayang
i-save ng ibang port). Habang naka-customize ang laro, binabago ng Main
menu → I-customize ang mga modifier at add-on nito anumang oras; nakapirmi
ang patakaran ng vanilla na laro. Gumagana ang mga modifier sa anumang mod
at nag-o-off sa deathmatch.

- Mas mahirap: mas maraming natatamong pinsala, mas matitibay na kalaban,
  mas kaunting napupulot na bala at health, 2–4x na halimaw, pistol start sa
  bawat level, walang manual na save.
- Mas madali: mas kaunting natatamong pinsala, mas mahihinang kalaban, mas
  maraming bala at health, walang-hanggang bala, pagbawi ng health, walang
  fall damage, walang pagkalunod, walang self damage, doble o tripleng
  talon.

## Mga Demo

Nire-record ng `record <name>` at `stop` ang demo sa user folder mo;
pinapalabas ito ng `playdemo <name>`. Kapag huminto ang pag-record,
isinusulat ulit ang demo sa background para kaya rin itong i-play ng
ironwail at vkQuake. Kung gumagamit ang mod ng particle effect na hindi
kayang ipakita ng ibang port, itinatabi ang buong bersyon bilang
`<name>.qsd` (`playdemo <name>.qsd`). Kino-convert ng `demo_convert <name>`
ang mas lumang demo.

## Accessibility

Options → Accessibility:

- Bawasan ang galaw at ang tindi ng kislap ng screen.
- Mga filter para sa color blindness.
- Naaayos na crosshair: hugis, laki, balangkas, at pagbabago ng kulay kapag
  nasa target o sa pagpatay; opsyonal na hit marker.
- **Caption ng Tunog** at mga setting ng mensahe (Mensahe at Caption),
  laki ng teksto.
- Toggle (sa halip na hold) na atake at takbo.
- Slider ng bilis ng laro para pabagalin ang buong laro.
- Auto-aim na may lock indicator, at setting ng cone at abot.

## Mga Pagpapahusay

Options → Mga Pagpapahusay. Lahat ay client-side at naka-off bilang
default, kaya gumagana sila sa anumang mod at online.

- **Debris**: gumugulong na gibs at shrapnel mula sa nabasag na bariles,
  pati sa mga halimaw ng mod; puwede itong bumangga sa pinto at lift.
- **Pinahusay na dugo**: talsik sa mga ibabaw, sugat na sumusunod sa
  katawan, patak sa ere na tumitilamsik kung saan lumapag.
- **Epekto ng tama**: marka ng bala ng shotgun, pakong bumabaon sa kahoy at
  kalaban, marka ng sunog sa pagsabog.
- **Pinahusay na Tubig**: pagkalinaw, alon, talsik at caustics.
- **Nakikitang Teleporter** na nagpapakita ng destinasyon nila nang may
  parallax.
- Ambient occlusion, motion blur kada bagay at sa bilis, malambot na
  particle.
- **Shader ng Screen**: RetroArch `.slangp` preset (CRT at iba pa).

Nasa Options → Graphics din ang karaniwang render scale, anti-aliasing,
supersampling, filter ng texture, 8-bit na kulay na may dithering, hamog,
makulay na ilaw at interpolation ng animation/galaw.

## Mga Add-on

Ang mga add-on ay bagong armas at kagamitan na pinapatakbo ng engine, kaya
gumagana sila sa progs ng anumang mod. Para sa single player at co-op lang
ang mga ito.

### Pagsisimula ng laro na may add-on

May add-on lang ang isang laro kung nagsimula ito nang may add-on:

1. Single Player → lagyan ng tsek ang **Mga Add-on**, saka New Game.
2. Piliin kung aling add-on ang dadalhin at ayusin ang mga setting nila,
   saka simulan ang bagong laro.

Nagse-save ang larong may add-on bilang `.qsa` file sa halip na `.sav`,
kaya hindi ito sinusubukang i-load ng ibang port. May "ADD-ONS" na badge
ang mga save card. Pinapatay ng pag-load ng normal na save ang mga add-on.
Habang naglalaro ng larong may add-on, ino-on at ino-off ng pahinang Mga
Add-on (console: `menu_options addons`) ang bawat add-on; may sariling
loop na preview, instruction, bind at setting ang bawat isa.

### Mga Armas

Pumapasok ang mga add-on sa karaniwang weapon slot (pumili kada add-on, o
slot 9 at 0) at lumalabas sa weapon wheel at sa HUD. Ang putok at alt-fire
ay ang karaniwan mong attack button.

| Add-on | Putok | Alt Fire | Bala |
|---|---|---|---|
| **Portal Gun** | Ilagay ang asul na portal | Ilagay ang orange na portal | Wala |
| **Railgun** | Tumatagos na bala na tumatama sa lahat ng nasa linya | — | 2 cell |
| **Hookshot** | Kumapit at hilahin ang sarili sa kahoy, metal, bato o kalaban | — | Wala |
| **Stickybomb Launcher** | Pindutin nang matagal para mag-charge, bitawan para ihagis | Pasabugin | 1 rocket |
| **Boomerang** | Pindutin nang matagal para markahan ang kalaban, bitawan para ihagis; isa-isa nitong ini-stun ang bawat isa | Burahin ang marka | Wala |
| **Gel Sprayer** | Asul na repulsion gel (patalbog) | Orange na propulsion gel (pabilis) | Wala |
| **Powerwasher** | Mahabang water jet na nagtutulak sa kalaban at naghuhugas ng gel | Malapad na pamaypay ng jet | Wala |

Inilalagay ang mga portal sa patag na ibabaw ng mundo at dinadala ka nila,
ang mga halimaw at ang mga inihagis na bagay kasama ang kanilang momentum.
Kapag naka-on ang **Tumingin sa Loob ng Portal**, live mong nakikita ang
kabilang panig, kasama ang mga portal na nakikita sa loob ng portal;
naaayos ang kalidad, bilang ng live na tanaw, layo at lalim ng recursion.

### Kagamitan

Ino-on at ino-off ang kagamitan sa halip na hawakan. Buksan ang
**gulong ng gamit** (`+equipwheel`), o gamitin ang `goggles cycle` para
isa-isahin ang:

- **Night vision**
- **Thermal vision**

## Multiplayer

Bukod sa LAN at internet na NetQuake (protocol 15, 666 at 999), kumokonekta
ang port sa mga **QuakeWorld** server. Tumatanggap ang Multiplayer →
Join Game ng address o naghahanap sa LAN. Suportado ang FTE scripted
particle para sa mga mod na gumagamit nito.

## Command line at console

Karaniwang opsyon: `-basedir <dir>`, `-game <dir>`, `-hipnotic` / `-rogue` /
`-quoth`, `-width <w>`, `-height <h>`, `-fullscreen`, `-window`, at
`+command args` para magpatakbo ng console command sa pagsisimula.
Tumatanggap ang console (`~`) ng karaniwang Quake command at cvar.

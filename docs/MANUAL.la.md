# Liber usus

Hic liber explicat quomodo Quake cum hac versione ludatur, praesertim quae
ea addat ad Quake originale et ad alias versiones fontis. Omnia nova
optionalia sunt: optionibus praefinitis lusus ut Quake videtur et agitur.

## Prima vestigia

1. Exemplar tuum Quake compara. Versioni indicem monstra qui `id1/`
   continet (et modos iuxta eum), per `QUAKE_BASEDIR` vel
   `-basedir <index>`.
2. `cargo run --release` exsequere, vel modum statim incipe per
   `-game <index> +map <tabula>`.
3. Configuratio, servationes et imagines quadri in
   `~/.local/share/quake-source-port/<lusus>/` habitant, singulae plicae
   pro singulis modis.

Index principalis praebet Single Player, Multiplayer, Options, **Help**
(hunc librum), Quit et **Fasciculi tabularum**, dum thema Quake post eum
canit (Options → Sonus → Musica indicis principalis). Omnis index mure,
claviatura, moderatro et tactu utitur; optionem supervola vel elige ut
notam explicativam videas.

In Help sursum/deorsum sectionem eligit; Page Up/Page Down,
sinistra/dextra, globuli umerales vel rota muris super textum eum volvunt.

## Lingua

Options → Ratio ludendi → **Lingua** linguam indicum, nuntiorum in quadro
et huius libri eligit: Anglicam, Francogallicam, Theodiscam, Italicam,
Hispanicam, Russicam, Polonicam, Ucrainicam, Bohemicam, Slovacicam,
Croaticam, Bulgaricam, Suecicam, Finnicam, Danicam, Norvegicam,
Batavicam, Graecam, Hungaricam, Dacoromanicam, Lusitanicam (Lusitaniae
vel Brasiliae), Iaponicam, Coreanam, Sinicam (simplificatam vel
traditam), Turcicam, Indonesianam, Malaianam, Philippinam, Vietnamicam,
Serbicam, Lituanicam, Lettonicam, Estonicam, Melitensem, Albanicam,
Armeniacam, Georgianam, Islandicam, Hibernicam, Cambricam, Vasconicam,
Catalanicam, Gallaecam, Latinam, Esperantam, Ruthenicam albam,
Macedonicam, Slovenicam, Suahilicam, Arabicam, Hebraicam, Persicam, Urdicam,
Hindicam, Marathicam, Bengalicam, Panjabicam, Gujaraticam, Tamulicam,
Telugicam, Thaicam, Khmericam, Amharicam vel Cantonensem.
**Systema** linguam computatri tui sequitur. Textus lusus proprius
editionis anni 2021 tantum Francogallice, Theodisce, Italice, Hispanice et
Russice versus est; aliae linguae eum Anglice monstrant. In linguis
praeter Anglicam textus parvus in lusu litteris indicis utitur, quoniam
litterae classicae Quake solas litteras Latinas sine accentibus habent.
Nuntii consolatorii Anglice manent nisi **Consolatorium vertere** activum
est.

## Fasciculi tabularum (perlustrator modorum)

Index principalis → Fasciculi tabularum omnem modum et fasciculum tabularum
installatum cum descriptione enumerat. Unum elige ut ad eum transeas,
deinde tabulas eius perlustra et quamvis statim incipe. Perlustrator
optimas caedes, arcana et tempus tuum pro singulis tabulis servat.
Quisque modus configurationem propriam habet, ita ut assignationes et
optiones uni modo aptatae in alium non effluant.

**Quo fasciculi tabularum eant.** Versio eos in indice basali Quake quaerit
(eo qui `id1/` continet, per `QUAKE_BASEDIR` vel `-basedir` statuto).
Omnis subindex ibi qui data lusus continet (plicam `.pak` vel `.pk3`,
`progs.dat`, vel indicem `maps/`) enumeratur; itaque fasciculum tabularum
installa eum in indicem proprium iuxta `id1/` expandendo, exempli gratia
`<basedir>/ad/` vel `<basedir>/my_maps/maps/*.bsp`. Si vis, plicam
`modinfo.txt` cum lineis `title`, `base` et `description` adde ut regas
quomodo enumeretur. Fasciculi inveniuntur cum lusus incipit, itaque post
additionem iterum incipe. Servationes, imagines quadri et configuratio
cuiusque fasciculi in `~/.local/share/quake-source-port/<index>/`
habitant, non in indice fasciculi.

## Quadrum divisum

Usque ad sex lusores in una machina, in cooperatione vel deathmatch.

- Multiplayer → New Game → **Lusores locales** statuit quot homines
  ludant.
- Quisque lusor aspectum, HUD, sonum et instrumentum initus proprium habet
  (claviaturam et murem lusor 1, moderatrum singuli ceteri).
- Dispositio quadri potest esse superposita, iuxtaposita, 2×2 vel 3×2
  (`r_splitscreen_layout`).
- Quisque lusor nomen et colores tunicae/bracarum sub Multiplayer → Setup
  eligit.

## Moderatra

Haec versio facta est ut moderatro ludatur.

- **Rota telorum** (`+weaponwheel`): tene ut tempus retardes et telum
  utrovis vecte vel mure eligas; dimitte ut mutes.
- **Auxilium collimandi** (Options → Initus → Moderatrum): retardatio prope
  hostes et rotatio quae eos sequitur. Vecti tantum valet, numquam muri.
- **Collimatio gyroscopica** (Options → Initus → Moderatrum →
  Gyroscopium): commutator, atque sensibilitates separatae pro axibus
  horizontali, verticali et volutationis (volutatio praefinito inactiva
  est et aspectum ut axis horizontalis vertit, valores negativi eam
  invertunt), filtrum strepitus, et quando gyroscopium valeat (semper,
  inactivum vel tantum dum `+gyroaction` tenetur). **Modus collimandi**
  vectem dextrum inter collimationem 2D (aspectum continenter vertit) et
  **flick stick** (vectem impelle ut aspectus statim illuc dirigatur,
  deinde vectem rota ut eum vertas) mutat; ambo cum gyroscopio bene ad
  subtilem collimationem coniunguntur.
- **Vibratio**, sensibilitas et inversio vectis.
- **Signa globulorum** moderatro tuo (Xbox, PlayStation, Nintendo)
  congruunt et sponte agnoscuntur.
- Omnia in Options → Moderamina reassigna, etiam ex moderatro.
- **Select / View ×2**: Imago quadri; **Start ×2**: Celeriter servare;
  **Start** (Tenere): Celeriter onerare.
- Moderamina tactilia cum magnitudine, opacitate et celeritate aspectus
  configurabilibus.

## Servatio

- **Autoservationes** separatim a servationibus manualibus et celeribus
  alternantur, itaque tuas numquam superscribunt. Options → Ratio ludendi →
  Optiones autoservationis eligit quot retineantur et quando servetur:
  initio tabulae, per intervallum, arcano vel omnibus arcanis inventis, et
  omnibus hostibus necatis.
- **Chartae servationum** in indicibus onerandi et servandi tabulam,
  salutem, loricam, tela, claves et caedes/arcana cuiusque servationis
  monstrant.
- **In morte** (Options → Ratio ludendi → In morte) lusus singularis in
  indice intermisso rogare potest quid agendum sit, servationem
  recentissimam onerare, te servationem eligere sinere, vel tabulam iterum
  incipere.

## Ratio ludendi et camera

Options → Ratio ludendi:

- Campus visus 60–130, nutatio aspectus, repercussus teli, semper currere.
- **Camera**: prima persona, tertia persona, vel supra umerum (dextrum vel
  sinistrum).
- Locus **exemplarium telorum**: medium, dextra, sinistra vel celata.

## Accommodare: modificatores et additamenta

Lusus aut *solitus* aut *accommodatus* est, quod initio eligitur. New
Game (et electio tabulae in Tabulam eligere vel Fasciculi tabularum)
fenestellam monstrat: **Ludere** lusum incipit, **Experientiam
accommodare** Modificatores lusus et Additamenta aperit. Dum aliquid
mutatum est, Ludere nota **PROPRIUM** signatur, et **Omnes
modificationes tollere** (Del, X moderatri, vel pressio muris) regulas
solitas sine additamentis restituit. Lusus soliti plicas `.sav` ordinarias
servant quas ironwail et vkQuake onerare possunt; lusus accommodati plicas
`.qsa` servant quas sola haec versio onerat (item lusus in quadro diviso
et cooperationis, quos aliae versiones servare non possunt). Dum lusus
accommodatus agitur, Index principalis → Accommodare modificatores et
additamenta eius quovis tempore mutat; regulae lusus soliti fixae manent.
Modificatores cum quovis modo valent et in deathmatch inactivantur.

- Difficilius: plus damni acceptum, hostes duriores, minus munitionis et
  salutis collectae, monstra ×2–4, initium nudum in omni tabula, nullae
  servationes manuales.
- Facilius: minus damni acceptum, hostes infirmiores, plus munitionis et
  salutis, munitio infinita, regeneratio salutis, nullum damnum casus,
  nulla submersio, nullum damnum sui, saltus duplices vel triplices.

## Demonstrationes

`record <nomen>` et `stop` demonstrationem in indicem usoris tui
registrant; `playdemo <nomen>` eam ludit. Cum registratio desinit,
demonstratio in recessu rescribitur ut ironwail et vkQuake quoque eam
ludere possint. Si modus effectibus particularum utitur quos aliae
versiones monstrare non possunt, forma plena ut `<nomen>.qsd` servatur
(`playdemo <nomen>.qsd`). `demo_convert <nomen>` demonstrationem
vetustiorem convertit.

Index principalis → Demonstrationes servatas tuas ostendit et, si Optiones →
Demonstrationes → Demonstrationes sponte inscribere accensum est,
demonstrationem automaticam omnis gradus quem ludis; vetustissimae
substituuntur cum loci pleni sunt. Scribe ut indices coles, et Tab preme ut
ordinem mutes; bullae iuxta indicem demonstrationem electam ludunt,
exportant, renominant, servant aut delent, et demonstratio automatica
renominata servatur. Relationes errorum demonstrationem automaticam gradus
praesentis continent.

Dum demonstratio luditur, Spatium pausam facit, Sinistram vel Dextram
tenendo retro vel celeriter prorsus volvis, Sursum et Deorsum celeritatem
mutant, PgUp et PgDn decem secunda saliunt, Home et End ad initium et ad
chartam proximam eunt. I et O fragmentum notant; E id, vel totam
demonstrationem, in plicam `videos` exportat. Imperia in visu manent; H vel
Cela ea celat, et ad marginem imam monstrando taenia apparet quae ea
reducit. Premes bullas, temporis lineam preme ut eo salias, aut trahe per
eam ut fragmentum notes; Exporta rogat imagines per secundum, magnitudinem,
formam et documentum, et utrum fragmentum notatum an totam demonstrationem
exportes.

In moderatore A pausam facit, crux directionis retro et prorsus volvit et
celeritatem mutat, LB et RB decem secundas saliunt, et bullae vectium ad
initium et ad tabulam proximam ducunt. LT et RT fragmentum notant, B id
purgat, X exportat, Y demonstrationem automaticam servat et View imperia
celat.

`capture` dum ludis inscriptionem pelliculae incipit et sistit. Imagines per
secundum, forma et qualitas sunt in Optiones → Demonstrationes → Inscriptio
pelliculae. Pelliculae ffmpeg requirunt; sine eo imagines ut picturae PNG
cum documento WAV servantur.

## Accessibilitas

Options → Accessibilitas:

- Motus minutus et vis fulgurum quadri.
- Filtra pro caecitate colorum.
- Reticulum configurabile: forma, magnitudo, ambitus, et colores mutati in
  scopo vel in caede; signa ictuum optionalia.
- **Subscriptiones sonorum**; locus, magnitudo et tempus nuntiorum et
  subscriptionum in editore HUD statuuntur.
- Ferire et currere alternantia (pro tenendo).
- Regula celeritatis lusus ad totum lusum retardandum.
- Collimatio automatica cum indice capturae, cono et spatio
  configurabilibus.

## Augmenta

Options → Augmenta. Omnia apud clientem et praefinito inactiva, itaque
cum quovis modo et in rete valent.

- **Rudera**: frusta volventia et schidiae ex doliis fractis, etiam cum
  monstris modorum; ad ianuas et pegmata impingere possunt.
- **Sanguis auctus**: aspersiones in superficiebus, vulnera quae corpora
  sequuntur, guttae volantes quae ubi cadunt maculant.
- **Effectus ictuum**: vestigia glandium sclopeti, clavi qui ligno et
  hostibus infiguntur, vestigia ustionis explosionum.
- **Aqua aucta**: translucentia, undulae, aspergines et caustica;
  **Refractio subaquatica**, **Guttae in quadro**.
- **Teleportatores perspicui** qui metam suam cum parallaxi monstrant.
- Occlusio ambiens, obscuritas motus per obiecta et celeritatis,
  particulae molles.
- **Shader quadri**: praeconfigurationes RetroArch `.slangp` (CRT et
  aliae).

Options → Graphica etiam solita habet: scalam reddendi, laevigationem
marginum, supersampling, filtrum texturarum, colores 8-bit cum dithering,
nebulam, lumen coloratum et interpolationem animationis/motus.

## Additamenta

Additamenta sunt nova tela et instrumenta, a machina acta, itaque cum
progs cuiusvis modi valent. Tantum in lusu singulari et cooperatione
praesto sunt.

### Lusum cum additamentis incipere

Lusus additamenta habet tantum si cum eis incipit:

1. Single Player → capsam **Additamenta** nota, deinde New Game.
2. Elige quae additamenta afferas et optiones eorum apta, deinde novum
   lusum incipe.

Lusus cum additamentis ut plicae `.qsa` pro `.sav` servantur, ne aliae
versiones eos onerare temptent. Chartae servationum insigne "ADD-ONS"
monstrant. Servatio ordinaria onerata additamenta inactivat. Dum lusus
cum additamentis agitur, pagina Additamenta (consolatorium:
`menu_options addons`) singula additamenta activat et inactivat; quodque
praevisionem iteratam, praecepta, assignationes suas et optiones proprias
habet.

### Tela

Additamenta in locos telorum ordinarios eunt (pro quoque additamento
eligibiles, vel loci 9 et 0) et in rota telorum et in HUD apparent. Ictus
et Ictus alter globuli tui feriendi soliti sunt.

| Additamentum | Ictus | Ictus alter | Munitio |
|---|---|---|---|
| **Telum portarum** | Portam caeruleam ponit | Portam aurantiam ponit | Nulla |
| **Railgun** | Glans transfigens quae omnia in linea ferit | — | 2 cellae |
| **Uncus** | Ligno, metallo, lapidi vel hostibus haeret teque attrahit | — | Nulla |
| **Iaculator bombarum glutinosarum** | Tene ut ones, dimitte ut emittas | Accendere | 1 rocheta |
| **Boomerang** | Tene ut hostes notes, dimitte ut iacias; singulos vicissim stupefacit | Notas purgare | Nulla |
| **Aspersorium gelatinae** | Gelatina caerulea repulsionis (resiliens) | Gelatina aurantia propulsionis (celer) | Nulla |
| **Lavator pressorius** | Iactus aquae longus qui hostes impellit et gelatinam abluit | Flabellum latum iactuum | Nulla |
| **Funis mas-mas** | Ictus alter in tabulam electronicam lucentem intra manum unum caput inserit; alterum tum fulmina cum missilibus infinitis iacit | Ictus alter aut armorum mutatio eum evellit | Nulla |
| **Pistolae geminae** | Sinistra | Dextra | Nulla (Replere: R) |
| **Sclopetum impetus** | Sclopetum automatum quod clavis tuis repletur | Ictus alter per collineatorium puncti rubri collineat, ut ictus densiores et visus propior sint | Clavi (Replere: R) |

Portae in superficiebus planis mundi ponuntur et te, monstra et res
iactas cum impetu suo transferunt. Cum **Per portas videre** activum est,
alteram partem vivam vides, etiam portas per portas visibiles; qualitas,
numerus aspectuum vivorum, distantia et profunditas recursionis
configurari possunt.

### Instrumenta

Instrumenta activantur et inactivantur potius quam manu tenentur. **Rotam
instrumentorum** aperi (`+equipwheel`), vel `goggles cycle` adhibe ut per
ea procedas:

- **Visio nocturna**
- **Visio thermica**

## Plures lusores

Praeter lusum NetQuake in LAN et interrete (protocolla 15, 666 et 999)
haec versio ad ministratra **QuakeWorld** conectitur. Multiplayer → Join
Game inscriptionem accipit vel LAN perquirit. Particulae scriptae FTE pro
modis qui eis utuntur sustinentur.

## Linea mandatorum et consolatorium

Optiones communes: `-basedir <index>`, `-game <index>`, `-hipnotic` /
`-rogue` / `-quoth`, `-width <w>`, `-height <h>`, `-fullscreen`,
`-window`, et `+command args` ad mandata consolatorii initio exsequenda.
Consolatorium (`~`) mandata et cvars Quake solita accipit.

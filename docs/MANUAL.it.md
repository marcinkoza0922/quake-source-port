# Manuale d'uso

Questo manuale spiega come giocare a Quake con questo port, soffermandosi su
ciò che aggiunge rispetto al Quake originale e agli altri source port. Tutte
le novità sono facoltative: con le impostazioni predefinite il gioco ha
l'aspetto e il ritmo di Quake.

## Per iniziare

1. Procurati una copia di Quake. Indica al port la cartella che contiene
   `id1/` (e le mod accanto), con `QUAKE_BASEDIR` o `-basedir <cartella>`.
2. Esegui `cargo run --release`, oppure avvia subito una mod con
   `-game <cartella> +map <mappa>`.
3. Configurazione, salvataggi e screenshot si trovano in
   `~/.local/share/quake-source-port/<gioco>/`, una cartella per mod.

Il menu principale offre Single Player, Multiplayer, Options, **Help**
(questo manuale), Quit e **Pacchetti mappe**, con il tema di Quake in
sottofondo (Options → Audio → Musica del menu principale). Ogni menu
funziona con mouse, tastiera, controller e touch; passa sopra un'opzione o
selezionala per vedere un suggerimento che la spiega.

Nell'aiuto, su/giù sceglie una sezione; Pagina su/Pagina giù,
sinistra/destra, i tasti dorsali o la rotellina sul testo lo fanno
scorrere.

## Lingua

Options → Gioco → **Lingua** sceglie la lingua di menu, messaggi su schermo
e di questo manuale: inglese, francese, tedesco, italiano, spagnolo, russo,
polacco, ucraino, ceco, slovacco, croato, bulgaro, svedese, finlandese,
danese, norvegese, olandese, greco, ungherese, rumeno, portoghese
(Portogallo o Brasile), giapponese, coreano, cinese (semplificato o
tradizionale), turco, indonesiano, malese, filippino, vietnamita,
serbo, lituano, lettone, estone, maltese, albanese, armeno, georgiano,
islandese, irlandese, gallese, basco, catalano, galiziano, latino, esperanto, bielorusso, macedone,
sloveno, swahili, arabo, ebraico, persiano, urdu, hindi, marathi, bengalese,
punjabi, gujarati, tamil, telugu, thailandese, khmer, amarico o cantonese.
**Sistema** segue la lingua del computer. I testi di gioco della
riedizione 2021 sono tradotti solo in francese, tedesco, italiano, spagnolo
e russo; le altre lingue li mostrano in inglese. Nelle lingue diverse
dall'inglese i piccoli testi di gioco usano il carattere dei menu, perché
il carattere classico di Quake ha solo lettere latine senza accenti. I
messaggi della console restano in inglese, a meno che **Traduci la
console** sia attivo.

## Pacchetti mappe (browser delle mod)

Menu principale → Pacchetti mappe elenca ogni mod e pacchetto di mappe
installato con la sua descrizione. Scegline uno per passarvi, poi sfoglia
le sue mappe e avviane una direttamente. Il browser ricorda per ogni mappa
le tue migliori uccisioni, segreti e tempi. Ogni mod ha la propria
configurazione, quindi tasti e impostazioni di una mod non finiscono in
un'altra.

**Dove vanno i pacchetti di mappe.** Il port li cerca nella cartella base
di Quake (quella con `id1/`, impostata con `QUAKE_BASEDIR` o `-basedir`).
Ogni sottocartella che contiene dati di gioco (un file `.pak` o `.pk3`, un
`progs.dat` o una cartella `maps/`) viene elencata: installa quindi un
pacchetto estraendolo nella sua cartella accanto a `id1/`, ad esempio
`<basedir>/ad/` o `<basedir>/my_maps/maps/*.bsp`. Se vuoi, aggiungi un
`modinfo.txt` con le righe `title`, `base` e `description` per decidere come
appare. I pacchetti vengono trovati all'avvio del gioco, quindi riavvia
dopo averne aggiunto uno. Salvataggi, screenshot e configurazione di ogni
pacchetto stanno in `~/.local/share/quake-source-port/<cartella>/`, non
nella cartella del pacchetto.

## Schermo diviso

Fino a sei giocatori su un solo computer, in cooperativa o in deathmatch.

- Multiplayer → New Game → **Giocatori locali** imposta quanti giocano.
- Ogni giocatore ha la propria visuale, interfaccia, audio e dispositivo
  (tastiera e mouse per il giocatore 1, un controller per ciascuno degli
  altri).
- La disposizione può essere impilata, affiancata, 2×2 o 3×2
  (`r_splitscreen_layout`).
- Ogni giocatore sceglie un nome e i colori di maglia e pantaloni in
  Multiplayer → Setup.

## Controller

Il port è pensato per essere giocato con un controller.

- **Ruota delle armi** (`+weaponwheel`): tieni premuto per rallentare il
  tempo e scegliere un'arma con una delle levette o col mouse; rilascia
  per cambiarla.
- **Mira assistita** (Options → Input → Controller): rallentamento vicino
  ai nemici e rotazione che li segue. Agisce solo sulla levetta, mai sul
  mouse.
- **Mira con giroscopio** (Options → Input → Controller → Giroscopio): un
  interruttore, sensibilità separate per gli assi di imbardata, beccheggio
  e rollio (il rollio è disattivato per impostazione predefinita e ruota la
  visuale come l'imbardata; i valori negativi lo invertono), un filtro
  antirumore, e quando si applica il giroscopio (sempre, disattivato o solo
  tenendo premuto `+gyroaction`). **Stile di mira** alterna la levetta
  destra tra la mira 2D (ruota la visuale in modo continuo) e il **flick
  stick** (spingi la levetta per puntare subito la visuale in quella
  direzione, poi ruotala per farla girare); entrambi si combinano bene con
  il giroscopio per una mira precisa.
- **Vibrazione**, sensibilità e inversione delle levette.
- I **simboli dei pulsanti** corrispondono al tuo controller (Xbox,
  PlayStation, Nintendo) e vengono rilevati automaticamente.
- Tutto si riassegna in Options → Comandi, anche dal controller.
- Comandi touch con dimensione, opacità e velocità visuale regolabili.

## Salvataggi

- I **salvataggi automatici** ruotano separatamente da quelli manuali e
  rapidi, quindi non sovrascrivono mai i tuoi. Options → Gioco →
  Impostazioni salvataggio automatico sceglie quanti conservarne e quando
  salvare: a inizio livello, a intervalli, quando trovi un segreto o tutti i
  segreti, e quando uccidi tutti i nemici.
- Le **schede dei salvataggi** nei menu Carica/Salva mostrano livello,
  salute, armatura, armi, chiavi e uccisioni/segreti di ogni salvataggio.
- **Alla morte** (Options → Gioco → Alla morte), in giocatore singolo il
  gioco può chiedere cosa fare in un menu in pausa, caricare il salvataggio
  più recente, farti scegliere un salvataggio o ricominciare il livello.

## Gioco e telecamera

Options → Gioco:

- Campo visivo 60–130, ondeggiamento della visuale, rinculo dell'arma,
  corsa automatica.
- **Telecamera**: prima persona, terza persona o sopra la spalla (destra o
  sinistra).
- Posizione del **modello dell'arma**: centro, destra, sinistra o
  nascosto.

## Personalizza: modificatori e add-on

Una partita è *standard* o *personalizzata*, a seconda di come inizia. New
Game (e la scelta di un livello in Selezione livello o Pacchetti mappe)
mostra una finestra: **Gioca** avvia la partita, **Personalizza
l'esperienza** apre i modificatori di gioco e gli add-on. Quando qualcosa è
cambiato, Gioca porta l'etichetta **PERSONALIZZATA** e **Disattiva tutte le
modifiche** (Canc, X sul controller o clic) ripristina le regole standard
senza add-on. Le partite standard salvano normali file `.sav` che ironwail
e vkQuake possono caricare; quelle personalizzate salvano file `.qsa` che
solo questo port carica (lo stesso vale per schermo diviso e cooperativa,
che gli altri port non sanno salvare). Durante una partita personalizzata,
Menu principale → Personalizza ne cambia modificatori e add-on in qualsiasi
momento; le regole di una partita standard restano fisse. I modificatori
funzionano con qualsiasi mod e si disattivano in deathmatch.

- Più difficile: più danni subiti, nemici più resistenti, meno munizioni e
  salute raccolte, mostri x2–x4, partenza da zero a ogni livello, niente
  salvataggi manuali.
- Più facile: meno danni subiti, nemici più deboli, più munizioni e salute,
  munizioni infinite, rigenerazione della salute, niente danni da caduta,
  niente annegamento, niente danni a se stessi, salti doppi o tripli.

## Demo

`record <nome>` e `stop` registrano una demo nella tua cartella utente;
`playdemo <nome>` la riproduce. Al termine della registrazione la demo
viene riscritta in background perché anche ironwail e vkQuake possano
riprodurla. Se una mod usa effetti di particelle che gli altri port non
sanno mostrare, la versione completa viene conservata come `<nome>.qsd`
(`playdemo <nome>.qsd`). `demo_convert <nome>` converte una demo più
vecchia.

Menu principale → Demo elenca le tue demo salvate e, con Opzioni → Demo →
Registra demo automatiche attivo, una demo automatica di ogni livello
giocato; le più vecchie vengono sostituite quando gli slot sono pieni.
Digita per filtrare gli elenchi e premi Tab per cambiare l'ordine; i
pulsanti accanto all'elenco riproducono, esportano, rinominano, conservano o
eliminano la demo scelta, e rinominare una demo automatica la conserva. Le
segnalazioni di bug includono la demo automatica del livello in corso.

Durante una demo, Spazio mette in pausa, tenere premuto Sinistra o Destra
riavvolge o manda avanti veloce, Su e Giù cambiano la velocità, PagSu e
PagGiù saltano di 10 secondi, Home e Fine vanno all'inizio e alla mappa
successiva. I e O segnano una clip; E la esporta, o l'intera demo, nella
cartella `videos`. I controlli restano sullo schermo; H o Nascondi li
nasconde, e puntando il bordo inferiore appare una striscia che li fa
tornare. Fai clic sui pulsanti, fai clic sulla timeline per saltare lì, o
trascinaci sopra per evidenziare una clip; Esporta chiede frequenza,
dimensione, formato e file, e se esportare la clip evidenziata o tutta la
demo.

Con il controller, A mette in pausa, il d-pad riavvolge, manda avanti e
cambia la velocità, LB e RB saltano di 10 secondi e i pulsanti delle
levette vanno all'inizio e alla mappa successiva. LT e RT segnano una
clip, B la cancella, X esporta, Y conserva una demo automatica e View
nasconde i controlli.

`capture` avvia e ferma la registrazione di un video mentre giochi.
Frequenza, formato e qualità sono in Opzioni → Demo → Registrazione video. I
video richiedono ffmpeg; senza, i fotogrammi vengono salvati come immagini
PNG con un file WAV.

## Accessibilità

Options → Accessibilità:

- Movimento ridotto e intensità dei lampi dello schermo.
- Filtri per il daltonismo.
- Mirino configurabile: forma, dimensione, contorno e cambio di colore sul
  bersaglio o all'uccisione; indicatori di colpo facoltativi.
- **Didascalie dei suoni** e impostazioni dei messaggi (Messaggi e
  didascalie), dimensione del testo.
- Attacco e corsa a commutazione (invece di tenere premuto).
- Un cursore della velocità di gioco per rallentare tutta la partita.
- Mira automatica con indicatore di aggancio e impostazioni di cono e
  portata.

## Miglioramenti

Options → Miglioramenti. Tutto avviene lato client ed è disattivato per
impostazione predefinita, quindi funziona con qualsiasi mod e online.

- **Detriti**: brandelli che rotolano e schegge dei barili distrutti, anche
  con i mostri delle mod; possono urtare porte e ascensori.
- **Sangue migliorato**: schizzi sulle superfici, ferite che seguono i
  corpi, gocce in volo che si spiaccicano dove cadono.
- **Effetti d'impatto**: fori dei pallini, chiodi che si conficcano nel
  legno e nei nemici, bruciature delle esplosioni.
- **Acqua migliorata**: trasparenza, increspature, spruzzi e caustiche.
- **Teletrasporti trasparenti** che mostrano la destinazione con
  parallasse.
- Occlusione ambientale, motion blur per oggetto e da velocità, particelle
  morbide.
- **Shader dello schermo**: preimpostazioni RetroArch `.slangp` (CRT e
  altre).

Options → Grafica offre anche le solite impostazioni: scala di rendering,
antialiasing, supersampling, filtro delle texture, colore a 8 bit con
dithering, nebbia, illuminazione colorata e interpolazione di animazioni e
movimento.

## Add-on

Gli add-on sono nuove armi ed equipaggiamenti gestiti dal motore, quindi
funzionano con il progs di qualsiasi mod. Sono disponibili solo in
giocatore singolo e in cooperativa.

### Iniziare una partita con gli add-on

Una partita ha add-on solo se inizia con essi:

1. Single Player → spunta la casella **Add-on**, poi New Game.
2. Scegli quali add-on portare e regola le loro impostazioni, poi avvia la
   nuova partita.

Le partite con add-on salvano file `.qsa` invece di `.sav`, così gli altri
port non provano a caricarle. Le schede dei salvataggi mostrano un
distintivo "ADD-ONS". Caricare un salvataggio normale disattiva gli add-on.
Durante una partita con add-on, la pagina Add-on (console: `menu_options
addons`) attiva e disattiva i singoli add-on; ognuno ha un'anteprima in
ciclo, istruzioni, assegnazioni dei tasti e impostazioni proprie.

### Armi

Gli add-on occupano i normali slot delle armi (scegli quale per ogni
add-on, oppure gli slot 9 e 0) e compaiono sulla ruota delle armi e
sull'interfaccia. Fuoco e fuoco secondario sono i tuoi soliti tasti
d'attacco.

| Add-on | Fuoco | Fuoco secondario | Munizioni |
|---|---|---|---|
| **Pistola dei portali** | Piazza il portale blu | Piazza il portale arancione | Nessuna |
| **Railgun** | Proiettile perforante che colpisce tutto sulla sua linea | — | 2 celle |
| **Rampino** | Si aggancia a legno, metallo, pietra o nemici e ti tira verso di essi | — | Nessuna |
| **Lanciabombe adesive** | Tieni premuto per caricare, rilascia per lanciare | Fai esplodere | 1 razzo |
| **Boomerang** | Tieni premuto per marcare i nemici, rilascia per lanciare; li stordisce uno dopo l'altro | Cancella i marcatori | Nessuna |
| **Spruzzagel** | Gel blu repulsivo (elastico) | Gel arancione propulsivo (veloce) | Nessuna |
| **Idropulitrice** | Lungo getto d'acqua che respinge i nemici e lava via il gel | Ampio ventaglio di getti | Nessuna |

I portali si piazzano sulle superfici piatte del mondo e trasportano te, i
mostri e gli oggetti lanciati conservandone lo slancio. Con **Vedi
attraverso i portali** vedi l'altro lato in tempo reale, compresi i portali
visibili attraverso altri portali; qualità, numero di viste in tempo reale,
distanza e profondità di ricorsione sono regolabili.

### Equipaggiamento

L'equipaggiamento si accende e si spegne invece di essere impugnato. Apri
la **ruota dell'equipaggiamento** (`+equipwheel`) o usa `goggles cycle` per
passare tra:

- **Visione notturna**
- **Visione termica**

## Multigiocatore

Oltre al gioco NetQuake in LAN e su Internet (protocolli 15, 666 e 999), il
port si collega ai server **QuakeWorld**. Multiplayer → Join Game accetta un
indirizzo o cerca in LAN. Le particelle scriptate FTE sono supportate per
le mod che le usano.

## Riga di comando e console

Opzioni comuni: `-basedir <cartella>`, `-game <cartella>`, `-hipnotic` /
`-rogue` / `-quoth`, `-width <l>`, `-height <a>`, `-fullscreen`, `-window`
e `+comando argomenti` per eseguire comandi della console all'avvio. La
console (`~`) accetta i soliti comandi e variabili di Quake.

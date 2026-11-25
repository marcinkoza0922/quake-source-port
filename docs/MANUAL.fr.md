# Manuel d'utilisation

Ce manuel explique comment jouer à Quake avec ce port, en insistant sur ce
qu'il ajoute à Quake d'origine et aux autres ports. Tout ce qui est nouveau
est facultatif : avec les réglages par défaut, le jeu ressemble à Quake et se
joue comme lui.

## Premiers pas

1. Procurez-vous votre propre copie de Quake. Indiquez au port le dossier
   qui contient `id1/` (et les mods à côté), avec `QUAKE_BASEDIR` ou
   `-basedir <dossier>`.
2. Lancez `cargo run --release`, ou démarrez directement un mod avec
   `-game <dossier> +map <carte>`.
3. La configuration, les sauvegardes et les captures d'écran se trouvent
   dans `~/.local/share/quake-source-port/<jeu>/`, un dossier par mod.

Le menu principal propose Single Player, Multiplayer, Options, **Help** (ce
manuel), Quit et **Packs de cartes**, avec le thème de Quake en fond
(Options → Audio → Musique du menu principal). Tous les menus fonctionnent à
la souris, au clavier, à la manette et au tactile ; survolez ou sélectionnez
une option pour afficher une info-bulle qui l'explique.

Dans l'aide, haut/bas choisit une section ; Page précédente/Page suivante,
gauche/droite, les gâchettes hautes ou la molette sur le texte le font
défiler.

## Langue

Options → Jouabilité → **Langue** choisit la langue des menus, des messages
à l'écran et de ce manuel : anglais, français, allemand, italien, espagnol,
russe, polonais, ukrainien, tchèque, slovaque, croate, bulgare, suédois,
finnois, danois, norvégien, néerlandais, grec, hongrois, roumain, portugais
(Portugal ou Brésil), japonais, coréen, chinois (simplifié ou
traditionnel), turc, indonésien, malais, filipino, vietnamien,
serbe, lituanien, letton, estonien, maltais, albanais, arménien,
géorgien, islandais, irlandais, gallois, basque, catalan, galicien, latin,
espéranto, biélorusse, macédonien, slovène, swahili, arabe, hébreu, persan,
ourdou, hindi, marathi, bengali, pendjabi, goudjarati, tamoul, télougou, thaï,
khmer, amharique ou cantonais.
**Système** suit la langue de votre ordinateur. Les textes de jeu de la
réédition 2021 ne sont traduits qu'en français, allemand, italien, espagnol
et russe ; les autres langues les affichent en anglais. Dans les autres
langues que l'anglais, les petits textes en jeu utilisent la police des
menus, car la police classique de Quake n'a que des lettres latines sans
accents. Les messages de la console restent en anglais sauf si **Traduire
la console** est activé.

## Packs de cartes (navigateur de mods)

Menu principal → Packs de cartes liste chaque mod et pack de cartes installé
avec sa description. Choisissez-en un pour y passer, puis parcourez ses
cartes et lancez celle de votre choix. Le navigateur retient vos meilleurs
résultats en victimes, secrets et temps pour chaque carte. Chaque mod garde
sa propre configuration : les touches et réglages d'un mod ne débordent pas
sur un autre.

**Où placer les packs de cartes.** Le port les cherche dans le dossier de
base de Quake (celui qui contient `id1/`, défini avec `QUAKE_BASEDIR` ou
`-basedir`). Tout sous-dossier contenant des données de jeu (un fichier
`.pak` ou `.pk3`, un `progs.dat` ou un dossier `maps/`) est listé : installez
donc un pack en le décompressant dans son propre dossier à côté de `id1/`,
par exemple `<basedir>/ad/` ou `<basedir>/my_maps/maps/*.bsp`. Ajoutez au
besoin un `modinfo.txt` avec des lignes `title`, `base` et `description`
pour régler son affichage. Les packs sont détectés au lancement du jeu :
redémarrez après en avoir ajouté un. Les sauvegardes, captures et réglages
de chaque pack se trouvent dans `~/.local/share/quake-source-port/<dossier>/`,
pas dans le dossier du pack.

## Écran partagé

Jusqu'à six joueurs sur une même machine, en coopération ou en match à mort.

- Multiplayer → New Game → **Joueurs locaux** fixe le nombre de joueurs.
- Chaque joueur a sa propre vue, son ATH, son son et son périphérique
  (clavier et souris pour le joueur 1, une manette pour chacun des autres).
- L'écran peut être empilé, côte à côte, en 2×2 ou en 3×2
  (`r_splitscreen_layout`).
- Chaque joueur choisit un nom et les couleurs de son haut et de son
  pantalon dans Multiplayer → Setup.

## Manettes

Le port est conçu pour se jouer à la manette.

- **Roue des armes** (`+weaponwheel`) : maintenez pour ralentir le temps et
  choisir une arme avec l'un ou l'autre stick ou la souris ; relâchez pour
  changer d'arme.
- **Aide à la visée** (Options → Périphériques → Manette) : ralentissement
  près des ennemis et rotation qui les suit. Elle ne s'applique qu'au stick,
  jamais à la souris.
- **Visée gyroscopique** (Options → Périphériques → Manette → Gyroscope) :
  un interrupteur, des sensibilités séparées pour les axes de lacet, de
  tangage et de roulis (le roulis est désactivé par défaut et tourne la vue
  comme le lacet ; une valeur négative l'inverse), un filtre anti-bruit, et
  le moment où le gyroscope s'applique (toujours, désactivé ou seulement en
  maintenant `+gyroaction`). **Style de visée** fait passer le stick droit de
  la visée 2D (il tourne la vue en continu) au **flick stick** (poussez le
  stick pour orienter aussitôt la vue dans cette direction, puis tournez-le
  pour la faire pivoter) ; les deux se combinent bien avec le gyroscope pour
  une visée précise.
- **Vibrations**, sensibilité et inversion des sticks.
- Les **symboles des boutons** correspondent à votre manette (Xbox,
  PlayStation, Nintendo) et sont détectés automatiquement.
- Tout se réassigne dans Options → Commandes, y compris depuis la manette.
- **Select / View ×2**: Capture d'écran; **Start ×2**: Sauvegarde rapide;
  **Start** (Maintenir): Chargement rapide.
- Commandes tactiles avec taille, opacité et vitesse de vue réglables.

## Sauvegardes

- Les **sauvegardes auto** tournent séparément des sauvegardes manuelles et
  rapides, et n'écrasent donc jamais les vôtres. Options → Jouabilité →
  Réglages de sauvegarde auto fixe combien en garder et quand sauvegarder :
  en début de niveau, périodiquement, à la découverte d'un secret ou de tous
  les secrets, et quand tous les ennemis sont tués.
- Les **cartes de sauvegarde** des menus de chargement et de sauvegarde
  montrent le niveau, la santé, l'armure, les armes, les clés et les
  victimes/secrets de chaque sauvegarde.
- **À la mort** (Options → Jouabilité → À la mort), en solo, le jeu peut
  demander quoi faire dans un menu en pause, charger la sauvegarde la plus
  récente, vous laisser en choisir une ou recommencer le niveau.

## Jouabilité et caméra

Options → Jouabilité :

- Champ de vision de 60 à 130, balancement de la vue, recul de l'arme,
  toujours courir.
- **Caméra** : première personne, troisième personne ou par-dessus l'épaule
  (droite ou gauche).
- Position du **modèle d'arme** : centre, droite, gauche ou masqué.

## Personnaliser : modificateurs et extensions

Une partie est soit *standard*, soit *personnalisée*, au moment où elle
commence. New Game (et le choix d'un niveau dans Choix de niveau ou Packs de
cartes) affiche une fenêtre : **Jouer** lance la partie, **Personnaliser
l'expérience** ouvre les modificateurs de jeu et les extensions. Dès que
quelque chose est changé, Jouer porte la mention **PERSO** et **Désactiver
toutes les modifications** (Suppr, X à la manette ou clic) rétablit les
règles standard sans extensions. Les parties standard créent des fichiers
`.sav` ordinaires qu'ironwail et vkQuake savent charger ; les parties
personnalisées créent des fichiers `.qsa` que seul ce port charge (de même
pour l'écran partagé et la coopération, que les autres ports ne savent pas
sauvegarder). Pendant une partie personnalisée, Menu principal →
Personnaliser modifie ses modificateurs et extensions à tout moment ; les
règles d'une partie standard restent fixées. Les modificateurs fonctionnent
avec tous les mods et se désactivent en match à mort.

- Plus difficile : plus de dégâts subis, ennemis plus coriaces, moins de
  munitions et de soins ramassés, monstres x2 à x4, départ à zéro à chaque
  niveau, pas de sauvegarde manuelle.
- Plus facile : moins de dégâts subis, ennemis plus faibles, plus de
  munitions et de santé, munitions infinies, régénération de santé, pas de
  dégâts de chute, pas de noyade, pas d'auto-dégâts, doubles ou triples
  sauts.

## Démos

`record <nom>` et `stop` enregistrent une démo dans votre dossier
utilisateur ; `playdemo <nom>` la lit. À la fin de l'enregistrement, la démo
est réécrite en arrière-plan pour qu'ironwail et vkQuake puissent aussi la
lire. Si un mod utilise des effets de particules que les autres ports ne
savent pas afficher, la version complète est conservée sous `<nom>.qsd`
(`playdemo <nom>.qsd`). `demo_convert <nom>` convertit une démo plus
ancienne.

Menu principal → Démos liste vos démos enregistrées et, avec Options → Démos
→ Enregistrer les démos auto activé, une démo automatique de chaque niveau
joué ; les plus anciennes sont remplacées quand les emplacements sont
pleins. Tapez pour filtrer les listes et appuyez sur Tab pour changer le tri
; les boutons à côté de la liste lisent, exportent, renomment, gardent ou
suppriment la démo choisie, et renommer une démo automatique la garde. Les
rapports de bug incluent la démo automatique du niveau en cours.

Pendant une démo, Espace met en pause, maintenir Gauche ou Droite rembobine
ou avance rapidement, Haut et Bas changent la vitesse, PgUp et PgDn sautent
de 10 secondes, et Début et Fin vont au début et à la carte suivante. I et O
marquent un extrait ; E l'exporte, ou toute la démo, dans le dossier
`videos`. Les commandes restent à l'écran ; H ou Masquer les cache, et
pointer le bord inférieur affiche une bande qui les fait revenir. Cliquez
sur les boutons, cliquez sur la chronologie pour y sauter, ou faites-y
glisser la souris pour surligner un extrait ; Exporter demande la fréquence
d'images, la taille, le format et le fichier, et s'il faut exporter
l'extrait surligné ou toute la démo.

À la manette, A met en pause, la croix directionnelle recule, avance
rapidement et change la vitesse, LB et RB sautent de 10 secondes, et les
clics des sticks vont au début et à la carte suivante. LT et RT marquent un
extrait, B l'efface, X exporte, Y garde une démo automatique et View masque
les commandes.

`capture` démarre et arrête l'enregistrement d'une vidéo pendant que vous
jouez. Fréquence d'images, format et qualité sont dans Options → Démos →
Enregistrement vidéo. Les vidéos nécessitent ffmpeg ; sans lui, les images
sont enregistrées en PNG avec un fichier WAV.

## Accessibilité

Options → Accessibilité :

- Réduction des animations et intensité des flashs de l'écran.
- Filtres pour daltoniens.
- Réticule configurable : forme, taille, contour et changement de couleur
  sur cible ou à l'élimination ; marqueurs de touche facultatifs.
- **Sous-titres des sons**; l'emplacement, la taille et la durée des
  messages et des sous-titres se règlent dans l'éditeur d'ATH.
- Attaque et course en bascule (au lieu de maintenir).
- Un curseur de vitesse du jeu pour ralentir toute la partie.
- Visée automatique avec indicateur de verrouillage, réglages du cône et de
  la portée.

## Améliorations

Options → Améliorations. Tout se passe côté client et tout est désactivé
par défaut : cela fonctionne avec n'importe quel mod et en ligne.

- **Débris** : morceaux de chair qui roulent et éclats des barils brisés,
  aussi avec les monstres de mods ; ils peuvent heurter portes et
  ascenseurs.
- **Sang amélioré** : éclaboussures sur les surfaces, blessures qui suivent
  les corps, gouttes en vol qui s'écrasent là où elles tombent.
- **Effets d'impact** : impacts de plombs, clous qui se plantent dans le
  bois et les ennemis, traces de brûlure d'explosion.
- **Eau améliorée** : transparence, ondulations, éclaboussures et
  caustiques; **Réfraction sous l'eau**, **Gouttes sur l'écran**.
- **Téléporteurs transparents** qui montrent leur destination avec
  parallaxe.
- Occlusion ambiante, flou cinétique par objet et de vitesse, particules
  douces.
- **Shaders d'écran** : préréglages RetroArch `.slangp` (CRT et autres).

Options → Graphismes propose aussi les réglages habituels : échelle de
rendu, anticrénelage, suréchantillonnage, filtrage des textures, couleurs
8 bits avec tramage, brouillard, éclairage coloré et interpolation des
animations et des mouvements.

## Extensions

Les extensions sont de nouvelles armes et de nouveaux équipements gérés par
le moteur : elles fonctionnent avec le progs de n'importe quel mod. Elles
sont réservées au solo et à la coopération.

### Commencer une partie avec des extensions

Une partie n'a d'extensions que si elle commence avec :

1. Single Player → cochez la case **Extensions**, puis New Game.
2. Choisissez les extensions à emporter et ajustez leurs réglages, puis
   lancez la nouvelle partie.

Les parties avec extensions se sauvegardent en fichiers `.qsa` au lieu de
`.sav`, pour que les autres ports n'essaient pas de les charger. Les cartes
de sauvegarde affichent un badge « ADD-ONS ». Charger une sauvegarde normale
désactive les extensions. Pendant une partie avec extensions, la page
Extensions (console : `menu_options addons`) active et désactive chaque
extension ; chacune a un aperçu en boucle, des instructions, ses
assignations et ses propres réglages.

### Armes

Les extensions occupent les emplacements d'arme habituels (choisissez
lequel pour chaque extension, ou les emplacements 9 et 0) et apparaissent
sur la roue des armes et l'ATH. Le tir et le tir secondaire sont vos
touches d'attaque habituelles.

| Extension | Tir | Tir secondaire | Munitions |
|---|---|---|---|
| **Pistolet à portails** | Place le portail bleu | Place le portail orange | Aucune |
| **Railgun** | Projectile perforant qui touche tout sur sa ligne | — | 2 cellules |
| **Grappin** | S'accroche au bois, au métal, à la pierre ou aux ennemis et vous y tire | — | Aucune |
| **Lance-bombes collantes** | Maintenir pour charger, relâcher pour lancer | Faire exploser | 1 roquette |
| **Boomerang** | Maintenir pour marquer les ennemis, relâcher pour lancer ; il étourdit chacun tour à tour | Effacer les marques | Aucune |
| **Pulvérisateur de gel** | Gel bleu de répulsion (rebondissant) | Gel orange de propulsion (rapide) | Aucune |
| **Nettoyeur haute pression** | Long jet d'eau qui repousse les ennemis et lave le gel | Large éventail de jets | Aucune |
| **Câble mâle-mâle** | Le tir secondaire sur un panneau électronique lumineux à portée y branche une extrémité ; l'autre lance alors des éclairs avec des munitions illimitées | Le tir secondaire ou un changement d'arme l'arrache | Aucune |
| **Pistolets akimbo** | Gauche | Droite | Aucune (Recharger: R) |
| **Fusil d’assaut** | Un fusil automatique qui se recharge avec vos clous | Tir secondaire vise avec le point rouge pour des tirs plus précis et une vue rapprochée | Clous (Recharger: R) |

Les portails se posent sur les surfaces plates du décor et transportent
vous, les monstres et les objets lancés en conservant leur élan. Avec **Voir
à travers les portails**, vous voyez l'autre côté en direct, y compris les
portails visibles à travers d'autres portails ; la qualité, le nombre de
vues en direct, la distance et la profondeur de récursion sont réglables.

### Équipement

L'équipement s'active et se désactive au lieu d'être tenu en main. Ouvrez
la **roue d'équipement** (`+equipwheel`), ou utilisez `goggles cycle` pour
passer de l'un à l'autre :

- **Vision nocturne**
- **Vision thermique**

## Multijoueur

En plus du jeu NetQuake en réseau local et sur Internet (protocoles 15, 666
et 999), le port se connecte aux serveurs **QuakeWorld**. Multiplayer →
Join Game accepte une adresse ou cherche sur le réseau local. Les particules
scriptées FTE sont prises en charge pour les mods qui les utilisent.

## Ligne de commande et console

Options courantes : `-basedir <dossier>`, `-game <dossier>`, `-hipnotic` /
`-rogue` / `-quoth`, `-width <l>`, `-height <h>`, `-fullscreen`, `-window`,
et `+commande arguments` pour lancer des commandes de console au démarrage.
La console (`~`) accepte les commandes et variables habituelles de Quake.

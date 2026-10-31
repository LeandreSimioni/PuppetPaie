# PuppetPaie — ce qu'un agent doit savoir avant de toucher à quoi que ce soit

Réécrit le 15/10/2026. Depuis le 24/10, on reconstruit l'architecture du moteur, séance par séance
(`journal/PLAN-ARCHITECTURE.md` du privé) ; la démonstration sort après (#259). Les règles sont toutes là, en court ; leur récit est dans le journal du dépôt privé
(`journal/archives/2026-10-15-claude-md-avant.md`, et `2026-10-04-claude-md-avant.md` avant lui). **Les numéros des règles
de fond (0 à 11) ne bougent pas : le code les cite.**

## Es-tu l'orchestrateur ou un exécutant ? (Léandre, 23/10/2026)

Ce fichier s'adresse aux deux ; ce qui touche à la séance entière n'est qu'à l'orchestrateur.
- **L'orchestrateur** est la séance que Léandre a ouverte : il parle à Léandre, lit `ETAT.md` et les tickets, découpe,
  lance les sous-agents, fusionne, lance **la** suite complète, pousse, ouvre la PR, commente et ferme les tickets, écrit
  le journal.
- **Un exécutant** est un sous-agent lancé par l'orchestrateur (sa consigne le dit, il travaille dans un worktree, sur une
  branche de lot). Il fait son lot et rien d'autre, commite sur sa branche, et rend une passation. **Il ne fait jamais** :
  la suite complète (seulement les binaires qu'il nomme), un push, une PR, une fusion dans la branche de séance, un
  commentaire ou une fermeture de ticket, le journal ou `ETAT.md`, ni ne parle à Léandre. Quand ce fichier dit « à la fin
  de la séance », « avant de pousser », « on ferme le ticket », c'est l'orchestrateur ; en cas de doute, c'est sa
  consigne qui tranche.

## Par quoi on commence

1. **`journal/ETAT.md` du dépôt privé** : une page, le point de reprise. Il nomme ce qui se lit ensuite pour le chantier en
   cours (pendant la reconstruction : `journal/SEANCE-1-ECRANS-ET-MODELE.md`, en entier) ; rien d'autre ne se lit d'office.
2. **Les tickets** : les issues GitHub du dépôt privé (celles de Léandre, et celles du filet, étiquetées `retour` et
   `incident`). Une séance prend ceux que Léandre nomme ; les captures d'un ticket se regardent toutes, avant le reste.
3. Le reste s'ouvre quand on en a besoin : `atelier/README.md` (comment une commande s'utilise), `atelier/docs/ARCHITECTURE.md`
   (un format, un calcul), `atelier/docs/RUBRIQUES.md` (une rubrique de paie), `atelier/docs/COMPAGNIES-FICTIVES.md`, `atelier/docs/COMPAGNIE-D-ESSAI.md`
   (déposer en test depuis le nuage). Les archives du journal se cherchent par `grep` dans `journal/archives/INDEX.md`, et on
   n'ouvre que la section du chantier du jour.

## Deux règles de travail (Léandre, 14/10/2026) — elles passent avant le reste

1. **On finit ce qu'on commence.** Un chantier se mène jusqu'à ce qu'il marche pour de vrai, dans la séance. Ne s'arrête que ce
   qui attend vraiment Léandre (une décision, un geste sur son poste), et ça se dit en une ligne.
2. **Pas de précautions de transition tant que Léandre est le seul utilisateur** (jusqu'aux premières compagnies, décembre
   2026) : pas de compatibilité avec l'ancien, pas de circuit provisoire ; on fait directement la version définitive, quitte à
   tout casser. L'ancien marche pour les paies si le logiciel est indisponible deux jours.

## L'architecture s'impose, elle ne se raconte pas (Léandre, 24/10/2026)

- **Le code dit ce qui est, jamais comment on y est arrivé.** Un commentaire donne le *pourquoi* d'une règle (le texte
  déposé qui la fonde, la contrainte de la norme) ; il ne donne jamais l'*historique* d'une décision : pas de « avant, on
  faisait… », « comme le Python », « depuis le #253 », « Léandre a demandé », pas de justification d'un changement
  d'architecture. L'architecture est celle-ci, on l'assume. L'histoire va au journal du privé, nulle part ailleurs ; un
  commentaire d'histoire croisé en passant s'efface.
- **Les mots** : l'**engagement** est au centre et porte ses calculs ; le **contrat** est le document qu'il fait signer ; la
  **paie** est ce qui part avec le bulletin. « Paie » ne nomme jamais l'engagement (#259). Tout document (contrat, DPAE,
  DSN, bulletin, AER, paiements) se fabrique depuis l'engagement validé ; aucun ne recalcule.
- **L'écran commande, le moteur obéit.** Ce que l'utilisateur voit (l'engagement et ses cases, les étapes du #257) dit ce
  que le moteur doit fournir ; la complexité du droit vit dans le calcul et les barèmes datés, jamais dans le parcours
  (pas d'état, de bouton ni de refus de plus pour une règle).
- **Reconstruire, c'est renommer et rebrancher, partie par partie** : on reprend les fonctions existantes, on les renomme,
  on les branche du bon côté, on vérifie **une fois, à la main**, sur une compagnie fictive, que le résultat est juste, puis
  on supprime l'ancien branchement et ses tests dans le même geste. **Aucun test ne vérifie que le nouveau ressemble à
  l'ancien.**
- **Le nombre de tests ne monte pas.** Une fonctionnalité n'apporte pas de tests d'office. Ne s'écrivent que : un test par
  règle tirée d'un texte déposé, un test par bug réel qui a échappé, quelques parcours de bout en bout sur les fictives.
  Un test de l'ancien parcours meurt avec lui.
  Ce n'est pas un interdit, c'est un coût (Léandre, 31/10/2026 : « je ne veux pas que la graisse empêche de réparer les
  os ») : un test qui fige un détail (une phrase, une forme, un ordre, un montant d'un cas particulier) se met en travers
  du changement qu'on veut faire et fait tourner pour rien. **Les tests de structure sont les bienvenus**, ceux qui
  gardent les os quoi qu'on change dessus : une crate ne prend qu'à celles du dessous ; l'engagement ne lit pas le
  journal ; une seule fonction écrit un engagement ; **chaque route dit, en une phrase brève, ce qu'elle fait** (sa fiche,
  pour le LLM comme pour qui lit le code : une route sans explication, ou dont l'explication est trop longue, fait
  échouer le test) et son exemple est accepté ; **chaque phrase affichée tient dans sa place** ; aucun texte de
  la page n'est coupé ; rien ne sort de la machine.

## Les os, les muscles, la graisse (Léandre, 30-31/10/2026)

« Si on veut un corps solide, il faut des os solides ; après on rajoute les muscles, ce sont les routes, et il faut
qu'elles soient bien faites ; après on peut rajouter la graisse : les taux, les imports, les petits bidouillages, les
fonctions avancées. » **L'architecture est la base** : sans elle, « on fera des pansements sur des pansements et on n'aura
plus accès à la structure ». Le modèle et l'écran sont décrits dans `journal/SEANCE-1-ECRANS-ET-MODELE.md` du privé, qui
fait foi.

- **Les os : un seul chemin pour chaque chose.** Une chose (écrire un engagement, dire ce qui est parti, composer
  l'échéance de la DSN, lire le journal) se fait à un seul endroit ; un second chemin se ferme dans le commit qui ouvre le
  premier. L'engagement est au centre, une identité (salarié, début, rang ; le numéro du contrat, posé à la validation, est
  la clé de ce qui est parti). **Une carte porte un salarié et écrit son engagement, et lui seul** ; « Valider
  l'engagement » est un geste ; le mois n'est jamais une unité d'écriture ; **un mois est neuf ou importé** (un mois
  importé ne reçoit pas d'engagement neuf). L'engagement ne lit pas le journal : le moteur le lit **une fois par geste**
  (`envois::journal::lu`), ce qui est **parti** (`departs`) et ce qui est **revenu** dans la même valeur ; une déclaration
  refusée au bilan n'est pas partie (règle 5). L'empreinte qui fait recalculer un engagement est celle de ce dont **son**
  calcul dépend.
- **Les muscles : une route par geste de l'écran, bien faite.** Un geste, une route ; la route obéit (elle ne recopie pas
  une règle du calcul ni n'ajoute d'état, de bouton ou de refus) ; un refus dit quoi corriger et n'écrit rien ; ce qu'elle
  rend est ce que la page lit ; **la route rend la phrase que l'on lit**, la page ne fait que la mise en page et ne décide
  rien.
- **La graisse vient après** : on ne la pose pas sur un os ou un muscle qui n'est pas droit.
- **Un lot de reconstruction enlève, il n'ajoute pas** : il finit avec moins de lignes et moins de tests ; ce qui
  demanderait d'ajouter se dit, ne se fait pas. On ne fait rien que Léandre n'a pas validé ; une pause demandée est une
  pause.
- **Rien ne se débranche** : chaque appel de la page a sa route, chaque route un appelant, chaque geste va de la page au
  fichier écrit, chaque acte de la route à l'envoi. Des Sonnets en lecture le vérifient après chaque passe, face au plan.

## Deux acteurs : la personne devant l'écran, et un LLM en ligne de commande (Léandre, 31/10/2026)

**Le LLM est un second écran** : il passe par les mêmes routes, reçoit les mêmes décisions, les mêmes phrases et les mêmes
refus. Sa documentation sort du code : **chaque route porte sa fiche dans son code** (le geste, la charge et un exemple, ce
qu'elle rend, ses refus et quoi corriger, si elle lit, écrit ou engage, si elle est « avancée ») ; `moteur routes` en tire
le catalogue ; `moteur route … --texte` imprime les phrases que la page affiche. **« Avancé » est le même pour les deux** :
la marque de la fiche range l'écran et le catalogue (le chemin simple d'abord). Un acte réel, des deux côtés, demande le
mot de passe du logiciel tapé par un humain (la page, ou la fenêtre que le moteur ouvre) ; le LLM ne voit ni le mot de
passe ni le coffre. Le LLM écrit au développeur comme une personne (`signalement-envoyer`). **Rien du métier ne s'écrit à
côté** : le moteur dit ce qui est à faire (le panneau des étapes, les rappels, « Pour démarrer ») ; la skill dit seulement
comment le lui demander ; un manque du LLM est un manque du moteur. Le détail : `SEANCE-1-ECRANS-ET-MODELE.md`, « Le
second acteur ».

## La preuve d'un lot (Léandre, 31/10/2026, après la séance 8)

Ce qui n'allait pas pendant la reconstruction : la preuve de chaque lot était un jeu fixe de six gestes (`preuve-r`),
fait pour l'écriture d'une carte ; des lots qui touchaient l'import, la DPAE, les rappels, le mois importé ou la rupture
passaient cette preuve « identique » sans qu'elle regarde rien de ce qu'ils avaient changé. Un lot changeait une clé (la
DPAE rangée par numéro) et un lecteur oublié (le rappel) cassait sans qu'aucun test ne le voie. L'orchestrateur fusionnait
sur « tests verts, preuve identique » et répétait les passations comme des faits. Le tour de jeu suivant trouvait les
défauts que les lots avaient eux-mêmes créés. Désormais :

- **Un lot dit ce qu'il touche** : sa passation liste chaque fonction, clé, format ou identité qu'il change, et **chaque
  lecteur** trouvé par `grep` (appels, chaînes de route, clés JSON lues par la page) ; un lecteur non listé est un
  lecteur non vérifié.
- **La preuve couvre ce que le lot touche**, pas un jeu fixe : pour chaque lecteur, un appel de route ou un geste au banc
  qui le fait passer, avant et après. « Preuve identique » ne vaut que pour ce qu'elle a joué ; la passation dit ce
  qu'elle n'a pas joué.
- **Les parcours des fictives sont la preuve commune** : les scénarios des fictives (un à l'écran, un en ligne de
  commande) couvrent chaque domaine (embauche, DPAE et taux de PAS, mois, contrat et avenant, rupture, arrêt et accident,
  congé, Congés Spectacles, import et mois importé, DSN, clôture) ; leur rejeu (`banc.js verifier`) tourne après chaque
  fusion qui touche un domaine, pas seulement à la fin.
- **L'orchestrateur reproduit avant de répéter** : un point d'une passation ne se dit à Léandre qu'après avoir été vu
  (un appel de route, une capture, un `grep`) ; un lot se dit « fusionné, vérifié par … », jamais « fini », tant qu'un
  tour de jeu n'a pas joué son domaine.
- **Un tour de jeu court après chaque série de lots** sur le domaine touché, plutôt qu'un grand tour à la fin.
- **Frugal** (Léandre : « il ne faut pas que ça devienne une usine à gaz où on rajoute des tests dans tous les sens ») :
  la preuve d'un lot **n'est pas un test de plus** ; c'est un geste joué (un appel de route, un geste au banc) et noté
  dans la passation. Les tests restent ceux de « Le nombre de tests ne monte pas » ; les parcours des deux fictives en
  sont les « quelques parcours de bout en bout » : un rejeu par fictive, pas un test par domaine.

## Une séance, par tickets

- **Un ticket** : lire ce qu'il dit, le reproduire (dans la fenêtre, sur une compagnie fictive, ou par `moteur route`), corriger
  dans le public, prouver (un test quand c'est le moteur ; la fenêtre, à l'écran, quand c'est l'interface), puis **commenter le
  ticket** (ce qui est fait, où, comment le voir) et le **fermer** : corrigé sur la branche, c'est fermé, avec la phrase « ce sera
  réglé dans la nouvelle version » (Léandre, 15/10) ; on n'attend pas la version publiée. Ce qu'un ticket
  demande se fait entier ; ce qu'il ne demande pas ne se fait pas sans le dire.
- **Un manque va dans un ticket, jamais dans un plan ni une consigne** (Léandre, 28/10/2026). Un plan, une règle,
  `ARCHITECTURE.md` disent ce qui doit être ; ce qui ne l'est pas encore (un trou constaté, un reste à faire) s'ouvre en
  ticket du privé, avec ses cases à cocher, et se ferme quand c'est fait. Un constat laissé dans un plan resterait vrai
  aux yeux de la séance suivante une fois réparé.
- **Un retour se trie avant de se corriger** (Léandre, 30/10/2026 : « c'est pas qu'on corrige tout ce que dit le sonnet,
  sinon on crée un logiciel totalement délirant ») : le rapport d'un joueur du banc comme le ticket d'un utilisateur dit ce
  que quelqu'un a vécu, pas ce qu'il faut faire. L'orchestrateur, pour chaque point : le reproduit ou le lit dans le code ;
  se demande si une vraie compagnie le vivrait (ou si c'est le script du joueur) ; **vérifie toujours que les textes
  sont d'accord pour l'implanter** (un Haiku les cherche, voir plus bas). Ne va à un Opus que ce qui tient ; ce qui dépend de la pratique du métier se
  demande à Léandre en une ligne avant ; ce qui ne tient pas se dit au ticket, avec pourquoi.
- **Une consigne ne donne pas de règle sans texte** (Léandre, 30/10/2026) : si l'orchestrateur n'a pas le texte, sa
  consigne pose la question (le fait vu, ce qu'il faut savoir) et demande à l'exécutant de chercher le texte et de dire
  son raisonnement ; un exécutant n'applique pas une règle de consigne qui ne cite pas de texte sans l'avoir cherché.
- **La branche** : chaque séance part de `main` du public sur sa branche ; PR en brouillon tant que ce n'est pas fini ;
  Léandre fusionne. Le privé reçoit, sur la même branche, ce qui s'adresse à Léandre (`journal/`).
- **La fin de séance, c'est Léandre qui la dit** : la passe de tests ne se lance pas avant. Puis : le récit dans
  `journal/archives/<jour>.md`, une ligne dans `INDEX.md`, et `ETAT.md` **remplacé** (jamais empilé ; une page).
- Le contexte se surveille (`moteur guet` en arrière-plan, `--commits` pour sortir au prochain commit ; `moteur/src/atelier/guet.rs`) ;
  c'est Léandre qui décide quand il sonne.

## Où sont les choses

**Deux dépôts, le code n'est que dans le public.** `PuppetPaie` (public) : le code, les textes de référence, les flux GitHub ; on
y crée sa branche, on y code, on y ouvre la PR. `puppetpaieprivate` (privé) : `journal/` (`ETAT.md`, passations, topos,
archives), `reference-privee/` (les textes lus qu'on ne republie pas), et les **tickets** (ses issues). **Les données de la
compagnie ne sont dans aucun dépôt** : le poste, la clé USB, le Drive. Rien de réel n'entre jamais dans le public
(`moteur verifier-avant-commit`, en crochet avant chaque commit, et le test `donnees_reelles`). Dans les anciens textes
du journal, `prive/journal/` se lit `journal/` du privé.

À la racine du public, ce que les gens reçoivent, et l'atelier :

| | ce que c'est |
|---|---|
| `PuppetPaie.cmd` | le lanceur |
| `logiciel/` | **exactement ce qu'une compagnie reçoit** : `puppetpaie.exe` (la fenêtre), `WebView2Loader.dll`, `VERSION`, `donnees/` (barèmes, bases, modèles), `interface/` (la page : `index.html`, `interface.css`, `js/`). C'est ce que publie la mise à jour. |
| `fictives/` | les quatre compagnies fictives, livrées (`docs/COMPAGNIES-FICTIVES.md`) |
| `maj/`, `prive/` | dans une installation seulement, jamais dans le dépôt : la mise à jour de l'installation ; la compagnie de l'installation |
| `atelier/sources/` | l'espace Cargo du moteur : `socle/` et `droit/` (les deux crates du dessous), `engagement/` (l'engagement, ses versions, ce qui est à refaire), `documents/` et `declarations/` (ce que l'engagement validé fait imprimer et déclarer ; `declarations` prend à `documents`, jamais l'inverse), `envois/` (ce qui part et revient : net-entreprises, DPAE, signature, courriel, Drive, retours, et le journal de ce qui est parti ; il reçoit des pièces faites et un mot de passe, et le branchement que `moteur` choisit : le monde, le simulateur ou le message), `moteur/` (programme `moteur` : `src/routes/` une route par geste de l'écran, `src/interface/` le serveur local de la page et la veille, `src/compagnie/` coffre, demande d'un acte réel, sauvegarde, mise à jour, bridage, ouverture, filet, incidents, conseils, `src/import/` le passé d'un ancien logiciel, `src/atelier/` les outils de l'éditeur et la publication, `src/commandes/` la ligne de commande ; exécutables livrés dans `moteur/bin/<système>/`) ; à part, `fenetre/` (Tauri, programme `puppetpaie`), `fabrique-pdf/` |
| `atelier/reference/` | les textes qui fondent les règles (voir « Les sources ») |
| `atelier/outils/`, `docs/`, `depot/`, `installeur/`, `site/`, `maquettes/` | les outils d'atelier (plus de Python depuis le 15/10/2026 : des commandes de `moteur`), les docs, les versions publiées, l'installeur, le site |

**L'interface** : la page `logiciel/interface/` (HTML, CSS, JS sans cadre) parle au moteur par `moteur route` (`js/tuyau.js`) ;
la fenêtre Tauri (`atelier/sources/fenetre/`) ne fait que l'afficher. **Toute décision vit dans le moteur** (règle 2) : une page
affiche et saisit, elle ne calcule ni ne refuse d'elle-même. On compile hors du dépôt (`CARGO_TARGET_DIR`) ; les exécutables
livrés (Windows, Linux, Mac), c'est « Publier » qui les compile.

**Le poste de Léandre est une installation ordinaire** (Léandre, 22/10/2026 : « j'utilise le logiciel comme tout le monde sur
mon poste ») : installée par l'installeur, mise à jour par la Release, sa compagnie dans son `prive/`. **On développe dans le
nuage, et seulement là** : aucune consigne ne suppose qu'il compile, commite, publie ou fait jouer les fictives depuis son
poste. Ce qui lui revient sur son poste, ce sont les gestes d'un utilisateur (essayer dans la fenêtre, configurer sa compagnie,
relever un texte que le nuage n'atteint pas) et ceux de l'éditeur (le serveur du filet). Ses retours y sont sans plafond : le
filet reconnaît le poste de l'éditeur par sa clé. Un miroir hebdomadaire des trois dépôts et des tickets, sur son poste, garde
tout si GitHub disparaît (`journal/ETAT.md` du privé).

**Ce que les gens lisent : on ne promet rien, on propose tout** (Léandre, 22/10/2026). Aucune phrase de l'écran ne dit ce qui
se passera plus tard (au déblocage, dans une prochaine version). Le logiciel bridé s'appelle **« démo »** dans ce qu'on lit à
l'écran, **« bridé »** dans le code et entre nous.
**Rien de remplissable s'il n'y a pas à le remplir** (Léandre, 15/10/2026 : « simple pour les gens qui ont besoin de simple,
compliqué pour les gens qui ont besoin de compliqué ») : un champ n'apparaît que parce qu'un fait l'exige (un motif qui
appelle une date, un arrêt qui appelle le maintien), jamais « au cas où » ; le chemin simple ne montre que les faits, le
chemin compliqué s'ouvre de lui-même quand un fait l'appelle. On prévoit tous les cas dans le moteur, et l'écran n'en
montre que celui qui a lieu.
**L'affichage dit le nom le plus court ; l'explication ne vient que là où l'on explique** (Léandre, 30/10/2026 : « il y
a une différence entre l'affichage et ce en quoi il est explicité ; l'affichage doit être le plus simple possible »).
Une carte, une liste, une pastille, un titre disent le mot qu'un utilisateur reconnaît (« CDDU », « CDD », « CDI ») ; le
sens long va dans l'aide d'un champ, un conseil ou le document qui doit le dire en toutes lettres (le contrat écrit).
**On ne coupe jamais un texte** (Léandre, 31/10/2026 : « je préfère que ça ait une sale gueule et qu'on puisse tout
lire ») : ni points de suspension, ni `overflow: hidden`, ni texte tronqué à N caractères, à l'écran comme dans un
message ou une phrase du moteur. Une phrase plus longue que sa place donne une **alerte au développeur** (un incident
silencieux, comme les autres), pour qu'on aille à la cause : écrire plus bref, dans le moteur — ce qui sert l'écran et le
LLM à la fois.

## Les sous-agents : Opus pour coder, Sonnet ou Haiku pour lire

- **Le code qui reste et ce qui tranche une conception vont à un Opus** (`model: "opus"`) ; **ce qui lit, et ce qui est
  mécanique ou jetable (une suppression, un rafistolage provisoire), va à un Sonnet** ; une recherche simple à un Haiku
  (Léandre, 28/10/2026 : « le code final en Opus, les rafistolages Sonnet »). Un rapport n'est pas une preuve : il se
  vérifie avant de s'affirmer. **Au moindre doute sur un texte, l'orchestrateur envoie un Haiku le chercher** (Léandre,
  30/10/2026 : « ils recherchent, ils coûtent peu ; c'est hyper bien pour faire de la recherche ») : dans
  `atelier/reference/`, par `moteur releve-codes`, sur les sites qui répondent ; il rend les passages et leurs fichiers,
  ou ce qui manque pour le ticket des textes à relever.
- **L'orchestrateur organise, puis vérifie après coup** : il donne à l'Opus le but, les règles, la passation attendue, et le
  laisse faire ; il ne dit pas *comment* faire ce qu'il n'a pas lu dans le code. Puis il vérifie (commits, diff, tests).
- **L'orchestrateur surveille pendant, pas seulement après** (Léandre, 25/10/2026 : « tu devrais regarder que tes agents
  font pas du bordel ») : toutes les 30 minutes, il regarde le worktree de chaque Opus (commits, fichiers touchés, tests
  et données modifiés) sans lire sa conversation ; une heure sans commit, il demande où il en est (et l'Opus dont un point
  dépasse une heure sans aboutir commite, rend sa passation et s'arrête de lui-même) ; un Opus trop chargé
  commite, rend sa passation, et un Opus neuf reprend. En tête de chaque consigne, avant le but : **un test de l'ancien
  parcours qui casse se supprime, il ne se répare pas** ; **aucune donnée destinée à être remplacée ne se convertit**. Un
  Opus qui le fait est arrêté aussitôt ; à la fusion, la liste des tests modifiés se relit avant d'accepter le lot.
  Aussi en tête (Léandre, 27/10/2026) : **aucun test de plus de 30 secondes** (une lenteur est un défaut de conception :
  on réduit le test ou on le supprime) ; **aucune mesure à part**, aucun `tail -f`, aucune attente empilée : une seule
  commande de tests à la fois, rien en fond ; **aucun acte réel lancé à la main**, même sur une fictive (un dépôt qu'on
  lance soi-même est de test ; le réel se voit par l'horloge de la fictive) ; un texte en PDF se transcrit une fois et
  ne se relit qu'à sa nouvelle version.
- **Le relais** : un Opus travaille au plus 1 h 15 environ, ou moins si ses notifications montrent qu'il dépasse
  500 000 jetons ; au relais, il laisse finir sa commande, commite, écrit sa passation, et un Opus neuf reprend. **On
  n'écrit jamais à un Opus qui a fini** : un message le réveille et lui fait relire tout son contexte. Une décision
  envoyée en cours de lot se vérifie dans le commit qui suit.
- **Parler à Léandre** : une décision qui touche le fonctionnement se présente entière, en trois lignes — ce qui se passe,
  à quel moment, ce que ça change —, avec ses contre-arguments, jamais par bribes.
- **Un reste ne remonte pas à Léandre : l'orchestrateur le tranche et le fait corriger** (Léandre, 21/10/2026 : « tu as
  trouvé un problème ? c'est super, maintenant il faut le corriger »). Chaque « laissé de côté », « à trancher », « limite »
  d'une passation se lit comme un travail à faire : l'orchestrateur décide (le code, les textes, le bon sens d'un
  utilisateur), relance l'Opus, vérifie, et seulement ensuite rend compte. Avant d'écrire à Léandre, il relit sa réponse :
  chaque point qu'il lui laisse doit être **une décision d'éditeur** (argent, engagement envers un organisme, publication,
  ce qui change le produit pour toutes les compagnies) **ou un geste sur son poste** ; tout autre point se corrige d'abord.
  **Un trou trouvé pendant la séance se comble dans la même PR, avant de dire à Léandre de fusionner** (Léandre,
  21/10/2026 : « pourquoi tu veux que je merge alors que tout n'est pas fini ? ») : « il existait déjà », « pour la PR
  suivante », « le refus est propre » ne sont pas des raisons de livrer. On ne bâcle pas pour publier plus tôt.
- **Plusieurs Opus à la fois, quatre au plus, et deux Sonnet au plus** (Léandre, 15/10/2026 ; la limite est le disque du
  conteneur : une cible Cargo pèse 2 à 6 Go) : des lots disjoints, chacun dans son worktree et sa cible Cargo
  (`CARGO_TARGET_DIR` seul, un seul profil) ; une passation vérifiée à la fois ; le worktree d'un lot fusionné s'efface
  aussitôt. **La cible, c'est l'orchestrateur qui la choisit** (Léandre, 23/10/2026) : des lots qui se suivent un par un
  partagent une même cible, gardée d'un lot à l'autre (on ne recompile que ce qui a changé) ; des lots en parallèle ont
  chacun la sienne, effacée à la fusion du lot. Sa consigne nomme la cible ; l'exécutant n'en crée pas d'autre.
  **Une cible qui passe d'un checkout à un autre** (un worktree, puis le checkout où l'on fusionne) se nettoie d'abord :
  `cargo clean --profile essais -p <crate>` pour chaque crate de l'espace. Cargo y range les crates par chemin relatif à
  l'espace et les croit à jour d'après les dates : une crate compilée depuis l'autre checkout masque le code présent.
- Ne se délègue pas : une décision de conception, un acte qui engage (dépôt réel, envoi, signature), tout ce qui toucherait au
  coffre. Un Opus **ne réécrit jamais l'historique** (`--amend`, `rebase`, `reset`) et **ne laisse aucune tâche de fond**.
- **Aucun repli Python** : ce qui s'écrit s'écrit en Rust, dans `moteur/` ; ce qui ne se porte pas proprement remonte à Léandre.
  **Pas de Python non plus comme outil** (Léandre, 21/10/2026, devant « a Python patch script ») : le code se modifie avec
  Edit/Write, fonction par fonction (`sed` pour un remplacement mécanique simple) ; un texte se lit avec `pandoc`/`grep` ;
  Playwright se pilote en Node. Aucun script de patch, dans aucun langage.
  On ne parle jamais de jetons à un Opus. Un script ne montre jamais une requête d'authentification ni son corps.

## Les sources : tout texte qu'on lit se dépose (Léandre, 10/10/2026)

**Toute règle vient d'un texte, et ce texte est dans `atelier/reference/sources/` avant que la règle entre dans le code.**
Pas d'exception pour « ce qu'on sait », ni pour un sous-agent (il dépose, ou nomme ce qu'il a lu pour qu'on le dépose).
Une règle sans texte déposé est un souvenir.

- **Le texte brut** tel que servi, `<émetteur>-<objet>-<AAAAMMJJ>.<ext>`, et sa transcription `.md`/`.txt` quand il ne se lit
  pas ; **un `LISEZ-MOI.md` par dossier** (l'URL, la date du relevé, ce qu'on en lit, **où le moteur s'en sert**) ; une
  **inférence** s'écrit comme telle ; la note `_source` d'un barème et le commentaire d'une fonction **citent le fichier
  déposé**, pas une URL seule.
- **Les codes** (travail, sécurité sociale, impôts, CGCT) se relèvent depuis LEGI (`moteur releve-codes`, les textes suivis dans `moteur/src/atelier/releve/codes/textes.rs`,
  `moissonner` puis `rapport`), jamais à la main ; la norme DSN est dans `norme-dsn/texte/` ; le BOFiP brut dans `bofip/` ;
  le BOSS depuis sa page quand elle répond.
- **Ce qui ne se republie pas** : un document d'organisme privé (Audiens, Afdas, CASC-SVP, Agirc-Arrco…) entre par sa
  transcription et son URL, l'original va dans `reference-privee/` du privé ; une pièce réelle (bulletin, courrier, relevé) ne
  va **que** là, et le public n'en garde que l'attendu, sans nom ni NIR. `atelier/reference/` ne se convertit jamais
  (`-text` dans `.gitattributes`) : des empreintes le vérifient.

## Les fichiers qu'on ne lit pas

```
prive/config/coffre.bin          les mots de passe net-entreprises, Urssaf et de la boîte courriel
prive/config/google-jeton.bin    le jeton du compte Google
prive/config/maj-cle-privee.bin  la clé qui signe les mises à jour (éditeur)
```

**Ne jamais lire, afficher, copier ni recopier leur contenu**, ni en extraire une valeur, ni en les décodant ni par
`coffre.lire()`. C'est une **convention, pas une garantie** : la clé maîtresse est scellée par le mot de passe du logiciel et
par la clé du poste (`cle-poste.bin`, dans `~/.config/puppetpaie/` ou `%APPDATA%\PuppetPaie\`, ou dans le dossier portable
marqué `PORTABLE.json`) ; sur le poste, un agent *pourrait* les ouvrir sans mot de passe. Ce qui tient la règle, c'est qu'elle
est écrite ici et qu'aucune commande n'affiche jamais un mot de passe. `moteur coffre` dit ce qui est posé, et cela suffit.

## Ce qu'un agent peut faire, et à quelle condition

| | condition |
|---|---|
| lire, calculer, produire un mois, sauvegarder, relever les retours | aucune |
| **déposer en test** (`depot-dsn --test`, `deposer-dpae --test`…) | aucune, **sans demander d'aval** |
| **déposer une DSN ou une DPAE en réel, envoyer, faire signer** | **le mot de passe du logiciel, tapé par un humain au moment de l'acte** |
| **approuver une publication** (environnement `publication` du public) | **jamais un agent** : les sessions Claude agissent sous l'identité de Léandre ; seul Léandre approuve |

**Deux mots de passe, deux chemins, pas d'autre.** Ouvrir le coffre = la séance (`moteur coffre seance --fenetre`, une boîte,
12 h, lancée avant la première commande qui touche au coffre ; jamais `getpass` ; sans terminal, refus). Un acte réel = le
popup, à chaque fois ; la séance n'est jamais une preuve. **Ne pas contourner** : un agent ne tape pas le mot de passe, ne pose
pas `PUPPETPAIE_MOTDEPASSE` ni la variable d'un organisme, n'ouvre pas le coffre dans un script. Si la clé du poste n'ouvre pas
le coffre, le dire et donner la commande.

**Un dépôt de test ne conserve rien et c'est le seul validateur du projet** (règle 10) : un fichier à contrôler se dépose en
`--test` et on lit le bilan. **Dans le nuage** (sans `prive/config/*.bin`) un dépôt réel est mécaniquement impossible
(`PreuveInsuffisante` avant tout) ; un dépôt de test est possible avec la compagnie d'essai (`docs/COMPAGNIE-D-ESSAI.md`) si
Léandre a posé `NET_ENTREPRISES_MOTDEPASSE` dans l'environnement : un agent ne la pose, ne la modifie ni ne l'affiche.

## Faire tourner le code dans le conteneur, et les tests

**Le moteur Linux est déjà là** : `atelier/sources/moteur/bin/linux/moteur` (et la fenêtre `bin/linux/puppetpaie`), livrés par
l'ancien flux « Moteur Rust » (retiré le 15/10/2026) ; `bin/SOURCES` dit de quelles sources. Pour reproduire une route : `PUPPETPAIE_CODE=logiciel/donnees` puis `echo '{"route":"…","charge":{…}}' |
atelier/sources/moteur/bin/linux/moteur route --racine <copie d'une compagnie fictive>` (sans `PUPPETPAIE_CODE`, l'exécutable
livré cherche ses référentiels au chemin de compilation). **Pour reproduire, observer ou mesurer, on ne compile pas**
(Léandre, 16/10/2026) : l'exécutable Linux livré sert tel quel, tant qu'il marche. On ne compile que pour éprouver ce qu'on
vient de changer dans le moteur (`cd atelier/sources/moteur && cargo build --release`), et les tests compilent toujours. La fenêtre demande
`libwebkit2gtk-4.1-dev libsoup-3.0-dev librsvg2-dev xvfb`.

**Tout doit être vert. Une PR dont la suite est rouge ne se fusionne pas.** Une fois, à la fin, avant de pousser ; pendant le
travail, on corrige sans relancer. Une seule commande d'attente à la fois, en arrière-plan. **La suite complète
(`cargo nextest run` sans `-E`) ne se lance qu'une fois par séance, par l'orchestrateur, sur la branche où tout est
fusionné** (Léandre, 23/10/2026 : « ces tests 20 fois de suite, ça n'a pas de sens ») ; un sous-agent ne lance que les
binaires qu'il nomme un par un (`-E 'binary(routes)'`…).

```bash
cd atelier/sources && rm -rf "${CARGO_TARGET_DIR:-target}/tmp"          # l'espace : socle, droit, engagement, documents, declarations, envois, moteur ; les dossiers de travail des tests (~3 Go)
cargo nextest run --cargo-profile essais --no-fail-fast                  # les sept crates ; la fenêtre n'a pas de tests
```

- Le binaire de l'atelier (`--features atelier` : la redirection vers de faux serveurs locaux, les bulletins fictifs,
  l'enregistreur du banc) n'a aucun test à lui ; quand on touche à ce qui est sous `#[cfg(feature = "atelier")]`, il doit
  compiler sans avertissement, dans sa propre cible : `CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-target}-atelier" cargo check
  --profile essais -p moteur --features atelier --bins`. Que le binaire livré n'en porte rien, c'est `tests/atelier.rs`
  qui le prouve, dans la suite ordinaire.
- **Quatre binaires par domaine dans `moteur/tests/`** (`tests/calcul/`, `tests/dsn/`, `tests/routes/`, `tests/fichiers/`) ; un fichier qui pose une
  variable d'environnement reste seul dans `tests/`. Les aides sont dans `tests/outils/` (un fichier n'en recopie aucune) ;
  un test se monte dans `outils::dossier_de_test`. Un test qui monte une compagnie passe par le moteur entier et reste dans
  `moteur/tests/` ; un test qui n'appelle que les fonctions de `documents`, de `declarations` ou d'`envois` sur des valeurs vit dans sa crate
  (`<crate>/tests/`, ou son `#[cfg(test)]`). Un fichier seul : `-E 'binary(calcul) & test(/^clauses::/)'` ; les tests d'une
  crate : `-p declarations`.
- **Ce qu'un test prouve** : une règle qui cite son texte déposé, ou un contrat (une route refuse et rien n'est écrit ; un
  fichier a la forme que le moteur relit). Jamais une forme (classe d'exception, message, ordre des clés). Un oracle de
  `tests/donnees/` se réécrit par le test qui le lit (`PUPPETPAIE_REECRIRE_ORACLES=1`, jamais posée par un test) ; une grille
  exhaustive se joue sur un échantillon à graine fixe (`outils::tire`).
- Les tests tournent sur des compagnies de test qu'ils montent eux-mêmes (`outils::dossier_de_test`) ; il n'y a plus de
  copie figée des fictives (supprimée le 28/10/2026 : ses règles se reprennent dans les fictives neuves de la séance 8,
  ticket #270 du privé). `fictives/` (à la racine du dépôt) est livrée à part par « Publier » (`moteur/src/atelier/publier.rs`) ; un test ne la
  lit jamais. Une compagnie de test se nomme toujours.
- **Aucun test ne sort de la machine** (21/10/2026 : un test s'était inscrit au vrai filet, d'où des courriels à
  `a@exemple.fr`). `atelier/sources/.cargo/config.toml` pose `PUPPETPAIE_ESSAIS_HORS_RESEAU` pour tout ce que Cargo lance, et
  `socle/src/garde_reseau.rs` refuse alors toute sortie hors de 127.0.0.1 ; un test qui veut voir une sortie nomme son faux
  mandataire dans cette variable. `tests/fichiers/garde_reseau.rs` exige que toute connexion du moteur passe par le garde-fou
  et qu'un `env_clear()` repose la variable.
- `.claude/hooks/session-start.sh` compile la suite d'avance dans le nuage. Doutes sur des tests : `journal/tests/DOUTES-TESTS.md`.

## GitHub Actions : à la fin seulement

Le dépôt est public : les minutes GitHub Actions sont illimitées, ce n'est pas une question de quota. On teste dans le
conteneur parce que c'est là qu'on travaille. Deux flux restent, à la main : « Tests » (`tests.yml`) et « Publier »
(`publier.yml`, qui compile lui-même les exécutables livrés). Le flux « Moteur Rust » est **retiré** (Léandre, 15/10/2026 :
« il ne sert plus à rien »). **Le rythme des versions est à Léandre** : une version par mois, ou une de plus s'il y a vraiment
un truc urgent ; c'est lui qui lance « Publier », jamais un agent. En cas de doute, demander.

## Les règles de fond du logiciel

0. **Ce logiciel n'est écrit pour aucune compagnie en particulier.** Paie du spectacle vivant pour toute compagnie, intermittents
   au cachet comme permanents. La compagnie du poste est le banc d'essai, pas le destinataire : ce qu'elle ne fait pas, d'autres
   le font ; un cas qu'aucune donnée n'éprouve reste un cas à porter.
1. **Rien ne se devine.** Un taux de PAS, un identifiant DGFiP se *lisent* (du compte rendu) ; un bordereau Urssaf se *calcule*
   depuis la table des CTP (`documents/src/bordereau.rs`). Le moteur refuse plutôt que d'inventer ; on ne « débogue » pas un refus
   avec une valeur plausible.
2. **Toute décision vit dans le moteur, jamais dans un front.** Ligne de commande et interface ne font qu'afficher ; un
   garde-fou dans un front se contourne par l'autre. Plus de Python neuf : ce qui s'écrit s'écrit en Rust.
3. **Les modules de transport ne cherchent jamais un secret.** Ils reçoivent un mot de passe ; ils n'ouvrent pas le coffre.
4. **Ne jamais régénérer une déclaration déjà déposée** pour la redéposer : les numéros de contrat se renumérotent dès que les
   compteurs avancent. Un numéro de contrat ne se recalcule pas, il se lit.
5. **Deux canaux de retours.** La DSN répond sur net-entreprises, la DPAE sur `consultation.dpae-edi.urssaf.fr` (nature 41) :
   services, mots de passe et archives distincts (`retours/`, `retours/dpae/`). Un 404 sur une plage veut dire « rien » ; un
   accusé de transport `OK` n'est pas une déclaration acceptée, c'est le bilan (CCO/BAN) qui tranche.
6. **Une nature de contrat n'est pas un détail.** `donnees/contrats-base.json` dit ce que la DSN écrit par nature
   (`cddu_spectacle`, `cdi`, `cdd`) ; un engagement muet est un CDDU. Le numéro d'objet spectacle est **interdit** hors des
   natures 02 et 10 (CCH-11). Le barème a des axes qui s'empilent (statut, nature, santé, catégorie), à un seul endroit
   (`droit::bareme::bareme_applique`). La prévoyance des permanents vient de la convention ; un cadre permanent lit
   `par_categorie.cadre_permanent`. Détail : `docs/ARCHITECTURE.md`, « La nature du contrat ».
7. **Deux contrôles avant tout dépôt** : les usages (une rubrique a-t-elle le *droit* d'être là pour la nature de déclaration)
   et les formats (ce qu'on y a *mis* a-t-il la bonne forme). Un contrôle faux refuserait du licite : c'est lui qu'on retire,
   jamais la déclaration.
8. **Un barème a trois étages** : le socle livré (`donnees/baremes/<année>.json`), la couche de la convention (`par_convention`
   du barème de l'année, qui fait foi : dix conventions en 2026, 3090 et 1285 seulement en 2025 ; une convention sans couche
   refuse), la surcouche de la compagnie (`config/baremes/<année>.json` : taux AT/MP, effectif, depuis `config/effectifs.json`
   écrit par les retours 118). Un seul endroit empile (`droit::contexte::Contexte::bareme_brut`,
   `droit::contexte::fusionner`) ; le taux AT/MP ne s'écrit qu'une fois (`droit::bareme::bareme_atmp`). Détail : `docs/ARCHITECTURE.md`.
9. **Trois destinations, et les données de la compagnie dans aucun dépôt.** `logiciel/`, `fictives/`, `atelier/` (dont
   `atelier/reference/`) : le dépôt public. `journal/`, `reference-privee/`, les tickets : le dépôt privé. `prive/` : aucun
   dépôt (le poste, la clé USB, le Drive ; `/prive/` est dans le `.gitignore` du public).
   a) **Les tests** ne lisent rien de `prive/` : `tests/fichiers/donnees_reelles.rs` refuse, dans `logiciel/`, `fictives/`,
      `atelier/sources/` et `atelier/reference/`, un NIR non inventé, le SIREN et le nom de la compagnie du poste, et un chemin
      vers `prive/` construit par un test. Ce qu'on garde d'une compagnie réelle, ce sont ses attendus, transcrits sans nom ni
      NIR dans un test fictif ; un cas qui a attrapé une vraie erreur se réécrit d'abord en fictif.
   b) Un conflit sur un journal JSON se résout enregistrement par enregistrement, sans rien perdre.
10. **Test = réel.** Un dépôt de test accepté (`OK, 0 anomalie`) vaut un dépôt réel : même moteur, même fichier, même circuit.
    On ne présente jamais un acte « prouvé en test, pas en réel » comme un risque ; reste ouvert ce que le test ne montre pas
    (AER, taux de PAS). Le garde-fou test → réel protège d'un envoi par erreur, il ne met pas le moteur en doute.
11. **La copie de développement ne se voit pas dans l'interface.** `prive-dev/` reste en ligne de commande (`moteur --dev …`) ;
    l'interface s'essaie sur les compagnies fictives.

## Les fins de ligne

Le code et les docs en **LF** (`.gitattributes`) ; `.cmd` et `.bat` en CRLF. Les formats à largeur fixe (gabarits AEM et
DPAE, DSN, retours, `.dsn`) et ce dont on vérifie une empreinte **ne se convertissent jamais** : octet pour octet. Un fichier
hors règle garde la fin de ligne qu'il a.

## Publier une version

On publie depuis GitHub (`journal/finitions/PUBLIER.md` du privé) : à la fin d'une séance, suite verte, la PR du public sur
`main` (Léandre fusionne ; le flux pose la date du jour dans `logiciel/VERSION` : le numéro est la date ; le rythme des publications reste celui dit plus haut). Léandre lance
« Publier » (`.github/workflows/publier.yml`) avec `notes`, et `calcul` **seulement** si la version change un calcul de paie
(la seule phrase qu'une compagnie verra ; un taux qui change n'en est pas une), `exigee` plus rarement encore. **Léandre
approuve** (environnement `publication`) ; le moteur signe avec la clé de GitHub. Une Release porte trois sortes d'archives
(`moteur/src/compagnie/maj.rs`, en tête) : **la mise à jour** (`puppetpaie-<v>-<système>.zip`, le contenu de `logiciel/` à la racine :
**sa forme est un contrat avec toutes les installations posées et ne change plus**), **les fictives** (livrées à part, signées,
téléchargées seulement quand leur empreinte change ; elles ne remplacent que ce qu'une livraison a posé), **l'installation**
(`…-installation.zip`, `logiciel/` et `fictives/`, pour Linux, Mac et le dossier portable) ; plus `derniere.json` et
l'installeur Windows. Une version s'installe dès qu'elle est prête ; `calcul` ne retient rien, c'est une phrase.
Jamais pendant que l'interface tourne ; jamais republier un numéro. Le moteur accepte encore la clé de
signature d'un poste (`maj-cle-privee.bin`, `moteur maj --publier`), mais on ne publie que depuis GitHub. On est en
développement : ne pas signaler qu'aucune version n'a été publiée depuis longtemps.

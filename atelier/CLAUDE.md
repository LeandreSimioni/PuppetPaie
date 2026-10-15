# PuppetPaie — ce qu'un agent doit savoir avant de toucher à quoi que ce soit

Réécrit le 15/10/2026 : le moteur est quasiment fini, on fait de l'interface, par tickets, jusqu'à la sortie en démonstration
(début novembre 2026). Les règles sont toutes là, en court ; leur récit est dans le journal du dépôt privé
(`journal/archives/2026-10-15-claude-md-avant.md`, et `2026-10-04-claude-md-avant.md` avant lui). **Les numéros des règles
de fond (0 à 11) ne bougent pas : le code les cite.**

## Par quoi on commence

1. **`journal/ETAT.md` du dépôt privé** : une page, le point de reprise. Rien d'autre ne se lit d'office.
2. **Les tickets** : les issues GitHub du dépôt privé (celles de Léandre, et celles du filet, étiquetées `retour` et
   `incident`). Une séance prend ceux que Léandre nomme.
3. Le reste s'ouvre quand on en a besoin : `atelier/README.md` (comment une commande s'utilise), `atelier/docs/ARCHITECTURE.md`
   (un format, un calcul), `docs/RUBRIQUES.md` (une rubrique de paie), `docs/COMPAGNIES-FICTIVES.md`, `docs/COMPAGNIE-D-ESSAI.md`
   (déposer en test depuis le nuage). Les archives du journal se cherchent par `grep` dans `journal/archives/INDEX.md`, et on
   n'ouvre que la section du chantier du jour.

## Deux règles de travail (Léandre, 14/10/2026) — elles passent avant le reste

1. **On finit ce qu'on commence.** Un chantier se mène jusqu'à ce qu'il marche pour de vrai, dans la séance. Ne s'arrête que ce
   qui attend vraiment Léandre (une décision, un geste sur son poste), et ça se dit en une ligne.
2. **Pas de précautions de transition tant que Léandre est le seul utilisateur** (jusqu'aux premières compagnies, décembre
   2026) : pas de compatibilité avec l'ancien, pas de circuit provisoire ; on fait directement la version définitive, quitte à
   tout casser. L'ancien marche pour les paies si le logiciel est indisponible deux jours.

## Une séance, par tickets

- **Un ticket** : lire ce qu'il dit, le reproduire (dans la fenêtre, sur une compagnie fictive, ou par `paie route`), corriger
  dans le public, prouver (un test quand c'est le moteur ; la fenêtre, à l'écran, quand c'est l'interface), puis **commenter le
  ticket** (ce qui est fait, où, comment le voir) et le **fermer** quand c'est vrai dans le logiciel livré. Ce qu'un ticket
  demande se fait entier ; ce qu'il ne demande pas ne se fait pas sans le dire.
- **La branche** : chaque séance part de `main` du public sur sa branche ; PR en brouillon tant que ce n'est pas fini ;
  Léandre fusionne. Le privé reçoit, sur la même branche, ce qui s'adresse à Léandre (`journal/`).
- **La fin de séance, c'est Léandre qui la dit** : la passe de tests ne se lance pas avant. Puis : le récit dans
  `journal/archives/<mois>.md`, une ligne dans `INDEX.md`, et `ETAT.md` **remplacé** (jamais empilé ; une page).
- Le contexte se surveille (`atelier/guet.py` en arrière-plan) ; c'est Léandre qui décide quand il sonne.

## Où sont les choses

**Deux dépôts, le code n'est que dans le public.** `PuppetPaie` (public) : le code, les textes de référence, les flux GitHub ; on
y crée sa branche, on y code, on y ouvre la PR. `puppetpaieprivate` (privé) : `journal/` (`ETAT.md`, passations, topos,
archives), `reference-privee/` (les textes lus qu'on ne republie pas), et les **tickets** (ses issues). **Les données de la
compagnie ne sont dans aucun dépôt** : le poste, la clé USB, le Drive. Rien de réel n'entre jamais dans le public
(`python3 atelier/outils/verifier_avant_commit.py` et le test `donnees_reelles` avant chaque commit). Dans les anciens textes
du journal, `prive/journal/` se lit `journal/` du privé.

À la racine du public, ce que les gens reçoivent, et l'atelier :

| | ce que c'est |
|---|---|
| `PuppetPaie.cmd` | le lanceur |
| `logiciel/` | **exactement ce qu'une compagnie reçoit** : `puppetpaie.exe` (la fenêtre), `WebView2Loader.dll`, `VERSION`, `donnees/` (barèmes, bases, modèles), `interface/` (la page : `index.html`, `interface.css`, `js/`). C'est ce que publie la mise à jour. |
| `fictives/` | les quatre compagnies fictives, livrées (`docs/COMPAGNIES-FICTIVES.md`) |
| `maj/`, `prive/` | sur le poste seulement : la mise à jour de l'installation ; la compagnie du poste |
| `atelier/sources/` | `paie/` (le moteur, programme `paie` ; exécutables livrés dans `paie/bin/<système>/`), `fenetre/` (Tauri, programme `puppetpaie`), `fabrique-pdf/` |
| `atelier/reference/` | les textes qui fondent les règles (voir « Les sources ») |
| `atelier/outils/`, `docs/`, `depot/`, `installeur/`, `site/`, `maquettes/` | les outils d'atelier (du Python, à porter), les docs, les versions publiées, l'installeur, le site |

**L'interface** : la page `logiciel/interface/` (HTML, CSS, JS sans cadre) parle au moteur par `paie route` (`js/tuyau.js`) ;
la fenêtre Tauri (`atelier/sources/fenetre/`) ne fait que l'afficher. **Toute décision vit dans le moteur** (règle 2) : une page
affiche et saisit, elle ne calcule ni ne refuse d'elle-même. On compile hors du dépôt (`CARGO_TARGET_DIR`), chaîne
`stable-x86_64-pc-windows-gnu` sur le poste ; l'exe de la fenêtre compilé se copie dans `logiciel/`.

## Les sous-agents : Opus pour coder, Sonnet ou Haiku pour lire

- **Ce qui écrit du code ou tranche une conception va à un Opus** (`model: "opus"`) ; **ce qui lit seulement va à un Sonnet
  ou un Haiku** (une recherche, un inventaire, une relecture ; Haiku pour une recherche simple). Un rapport n'est pas une
  preuve : il se vérifie avant de s'affirmer.
- **L'orchestrateur organise, puis vérifie après coup** : il donne à l'Opus le but, les règles, la passation attendue, et le
  laisse faire ; il ne dit pas *comment* faire ce qu'il n'a pas lu dans le code. Puis il vérifie (commits, diff, tests).
- **Plusieurs Opus à la fois** : des lots disjoints, chacun dans son worktree et sa cible Cargo (`CARGO_TARGET_DIR` seul, un
  seul profil) ; une passation vérifiée à la fois.
- Ne se délègue pas : une décision de conception, un acte qui engage (dépôt réel, envoi, signature), tout ce qui toucherait au
  coffre. Un Opus **ne réécrit jamais l'historique** (`--amend`, `rebase`, `reset`) et **ne laisse aucune tâche de fond**.
- **Aucun repli Python** : ce qui s'écrit s'écrit en Rust, dans `paie/` ; ce qui ne se porte pas proprement remonte à Léandre.
  On ne parle jamais de jetons à un Opus. Un script ne montre jamais une requête d'authentification ni son corps.

## Les sources : tout texte qu'on lit se dépose (Léandre, 10/10/2026)

**Toute règle vient d'un texte, et ce texte est dans `atelier/reference/sources/` avant que la règle entre dans le code.**
Pas d'exception pour « ce qu'on sait », ni pour un sous-agent (il dépose, ou nomme ce qu'il a lu pour qu'on le dépose).
Une règle sans texte déposé est un souvenir.

- **Le texte brut** tel que servi, `<émetteur>-<objet>-<AAAAMMJJ>.<ext>`, et sa transcription `.md`/`.txt` quand il ne se lit
  pas ; **un `LISEZ-MOI.md` par dossier** (l'URL, la date du relevé, ce qu'on en lit, **où le moteur s'en sert**) ; une
  **inférence** s'écrit comme telle ; la note `_source` d'un barème et le commentaire d'une fonction **citent le fichier
  déposé**, pas une URL seule.
- **Les codes** (travail, sécurité sociale, impôts, CGCT) se relèvent depuis LEGI (`outils/relever_les_codes.py`, `TEXTES`,
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
est écrite ici et qu'aucune commande n'affiche jamais un mot de passe. `paie coffre` dit ce qui est posé, et cela suffit.

## Ce qu'un agent peut faire, et à quelle condition

| | condition |
|---|---|
| lire, calculer, produire un mois, sauvegarder, relever les retours | aucune |
| **déposer en test** (`depot-dsn --test`, `deposer-dpae --test`…) | aucune, **sans demander d'aval** |
| **déposer une DSN ou une DPAE en réel, envoyer, faire signer** | **le mot de passe du logiciel, tapé par un humain au moment de l'acte** |
| **approuver une publication** (environnement `publication` du public) | **jamais un agent** : les sessions Claude agissent sous l'identité de Léandre ; seul Léandre approuve |

**Deux mots de passe, deux chemins, pas d'autre.** Ouvrir le coffre = la séance (`paie coffre seance --fenetre`, une boîte,
12 h, lancée avant la première commande qui touche au coffre ; jamais `getpass` ; sans terminal, refus). Un acte réel = le
popup, à chaque fois ; la séance n'est jamais une preuve. **Ne pas contourner** : un agent ne tape pas le mot de passe, ne pose
pas `PUPPETPAIE_MOTDEPASSE` ni la variable d'un organisme, n'ouvre pas le coffre dans un script. Si la clé du poste n'ouvre pas
le coffre, le dire et donner la commande.

**Un dépôt de test ne conserve rien et c'est le seul validateur du projet** (règle 10) : un fichier à contrôler se dépose en
`--test` et on lit le bilan. **Dans le nuage** (sans `prive/config/*.bin`) un dépôt réel est mécaniquement impossible
(`PreuveInsuffisante` avant tout) ; un dépôt de test est possible avec la compagnie d'essai (`docs/COMPAGNIE-D-ESSAI.md`) si
Léandre a posé `NET_ENTREPRISES_MOTDEPASSE` dans l'environnement : un agent ne la pose, ne la modifie ni ne l'affiche.

## Faire tourner le code dans le conteneur, et les tests

**Le moteur Linux est déjà là** : `atelier/sources/paie/bin/linux/paie` (et la fenêtre `bin/linux/puppetpaie`), livrés par
« Moteur Rust » ; `bin/SOURCES` dit de quelles sources. Pour reproduire une route : `PUPPETPAIE_CODE=logiciel/donnees` puis `echo '{"route":"…","charge":{…}}' |
atelier/sources/paie/bin/linux/paie route --racine <copie d'une compagnie fictive>` (sans `PUPPETPAIE_CODE`, l'exécutable
livré cherche ses référentiels au chemin de compilation). **Il retarde souvent sur les sources** (le 15/10, il plantait sur
`logiciel/donnees` plus récent que lui) : ce qu'on vient de changer se compile (`cd atelier/sources/paie && cargo build
--release`), et les tests compilent toujours. La fenêtre demande
`libwebkit2gtk-4.1-dev libsoup-3.0-dev librsvg2-dev xvfb`.

**Tout doit être vert. Une PR dont la suite est rouge ne se fusionne pas.** Une fois, à la fin, avant de pousser ; pendant le
travail, on corrige sans relancer. Une seule commande d'attente à la fois, en arrière-plan.

```bash
cd atelier/sources/paie && rm -rf "${CARGO_TARGET_DIR:-target}/tmp"     # les dossiers de travail des tests (~3 Go)
cargo nextest run --cargo-profile essais --no-fail-fast                  # le moteur ; la fenêtre n'a pas de tests
```

- La suite de l'atelier (`--features atelier`, sa propre cible `<cible>-atelier`), seulement quand on touche à ce qui est sous
  `#[cfg(feature = "atelier")]` : `CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-target}-atelier" cargo nextest run --cargo-profile
  essais --features atelier --lib --test atelier --test fichiers -E 'test(/(^|::)atelier::/)'`.
- **Quatre binaires par domaine** (`tests/calcul/`, `tests/dsn/`, `tests/routes/`, `tests/fichiers/`) ; un fichier qui pose une
  variable d'environnement reste seul dans `tests/`. Les aides sont dans `tests/outils/` (un fichier n'en recopie aucune) ;
  un test se monte dans `outils::dossier_de_test`. Un fichier seul : `-E 'binary(calcul) & test(/^clauses::/)'`.
- **Ce qu'un test prouve** : une règle qui cite son texte déposé, ou un contrat (une route refuse et rien n'est écrit ; un
  fichier a la forme que le moteur relit). Jamais une forme (classe d'exception, message, ordre des clés). Un oracle de
  `tests/donnees/` se réécrit par le test qui le lit (`PUPPETPAIE_REECRIRE_ORACLES=1`, jamais posée par un test) ; une grille
  exhaustive se joue sur un échantillon à graine fixe (`outils::tire`).
- Les tests tournent sur des compagnies fictives et sur **la copie figée** `paie/tests/donnees/fictives/` ; `fictives/` vit
  (le poste la fait jouer et la commite), un test ne la lit jamais. Une compagnie de test se nomme toujours.
- `.claude/hooks/session-start.sh` compile la suite d'avance dans le nuage. Doutes sur des tests : `journal/tests/DOUTES-TESTS.md`.

## GitHub Actions : à la fin seulement

Le dépôt est public : les minutes GitHub Actions sont illimitées, ce n'est pas une raison de s'en priver (Léandre,
15/10/2026). On teste dans le conteneur parce que c'est là qu'on travaille. **« Moteur Rust » ne se lance pas de
soi-même** : il livre `paie/bin/` (Windows, Linux, Mac) au poste et sert à la sortie ; on le lance sur un mot de Léandre.
En cas de doute, demander.

## Les règles de fond du logiciel

0. **Ce logiciel n'est écrit pour aucune compagnie en particulier.** Paie du spectacle vivant pour toute compagnie, intermittents
   au cachet comme permanents. La compagnie du poste est le banc d'essai, pas le destinataire : ce qu'elle ne fait pas, d'autres
   le font ; un cas qu'aucune donnée n'éprouve reste un cas à porter.
1. **Rien ne se devine.** Un taux de PAS, un identifiant DGFiP se *lisent* (du compte rendu) ; un bordereau Urssaf se *calcule*
   depuis la table des CTP (`paie/src/bordereau.rs`). Le moteur refuse plutôt que d'inventer ; on ne « débogue » pas un refus
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
   (`calcul.bareme_applique`). La prévoyance des permanents vient de la convention ; un cadre permanent lit
   `par_categorie.cadre_permanent`. Détail : `docs/ARCHITECTURE.md`, « La nature du contrat ».
7. **Deux contrôles avant tout dépôt** : les usages (une rubrique a-t-elle le *droit* d'être là pour la nature de déclaration)
   et les formats (ce qu'on y a *mis* a-t-il la bonne forme). Un contrôle faux refuserait du licite : c'est lui qu'on retire,
   jamais la déclaration.
8. **Un barème a trois étages** : le socle livré (`donnees/baremes/<année>.json`), la couche de la convention (`par_convention`,
   3090 et 1285 ; une convention sans couche refuse), la surcouche de la compagnie (`config/baremes/<année>.json` : taux AT/MP,
   effectif, depuis `config/effectifs.json` écrit par les retours 118). Un seul endroit empile (`Contexte._bareme_brut`,
   `baremes.fusionner`) ; le taux AT/MP ne s'écrit qu'une fois (`calcul.bareme_atmp`). Détail : `docs/ARCHITECTURE.md`.
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
11. **La copie de développement ne se voit pas dans l'interface.** `prive-dev/` reste en ligne de commande (`paie --dev …`) ;
    l'interface s'essaie sur les compagnies fictives.

## Les fins de ligne

Le code et les docs en **LF** (`.gitattributes`) ; `.cmd` et `.bat` en CRLF. Les formats à largeur fixe (gabarits AEM et
DPAE, DSN, retours, `.dsn`) et ce dont on vérifie une empreinte **ne se convertissent jamais** : octet pour octet. Un fichier
hors règle garde la fin de ligne qu'il a.

## Publier une version

On publie depuis GitHub (`journal/finitions/PUBLIER.md` du privé) : à la fin d'une séance, suite verte, la PR du public sur
`main` (Léandre fusionne ; le flux pose la date du jour dans `logiciel/VERSION`, une version par jour). Léandre lance
« Publier » (`.github/workflows/publier.yml`) avec `notes`, et `calcul` **seulement** si la version change un calcul de paie
(la seule phrase qu'une compagnie verra ; un taux qui change n'en est pas une), `exigee` plus rarement encore. **Léandre
approuve** (environnement `publication`) ; le moteur signe avec la clé de GitHub et une Release porte l'archive, `derniere.json`
et l'installeur, que toute installation lit (`paie maj`, `paie/src/maj.rs`), la sienne comprise. Une version qui change un
calcul attend la DSN des mois saisis avant de s'appliquer (`--malgre-calcul` pour passer outre). Jamais pendant que l'interface
tourne ; jamais republier un numéro. La clé du poste (`maj-cle-privee.bin`, `paie maj --publier`) reste acceptée. On est en
développement : ne pas signaler qu'aucune version n'a été publiée depuis longtemps.

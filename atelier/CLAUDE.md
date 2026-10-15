# PuppetPaie — ce qu'un agent doit savoir avant de toucher à quoi que ce soit

Allégé le 04/10/2026 : les règles sont toutes là, leur récit (dates, incidents
qui les ont fait écrire) est dans `journal/archives/2026-10-04-claude-md-avant.md`.
Les numéros des règles de fond (0 à 11) ne bougent pas : le code les cite.

## Deux règles de travail (Léandre, 14/10/2026) — elles passent avant le reste

1. **On finit ce qu'on commence.** Un chantier se mène jusqu'à ce qu'il marche pour de vrai, dans la séance. Pas de
   « c'est fait, mais il reste ceci » repoussé à plus tard. Ne s'arrête que ce qui attend vraiment Léandre (une décision,
   un geste sur son poste), et ça se dit en une ligne.
2. **Pas de précautions de transition tant que Léandre est le seul utilisateur** (jusqu'à l'arrivée des premières
   compagnies, décembre 2026). Pas de compatibilité avec l'ancien, pas de circuit provisoire, pas d'essai manuel d'un
   circuit qu'on va remplacer : on fait directement la version définitive, proprement, quitte à tout casser. Si le
   logiciel est indisponible deux jours pendant qu'on code, ce n'est pas grave : l'ancien marche pour les paies.

## Le rangement (09/10/2026)

À la racine du dépôt (le dossier du dépôt sur le poste), on ne
voit que ce que les gens reçoivent, et l'atelier en plus :

| | ce que c'est |
|---|---|
| `PuppetPaie.cmd` | le lanceur |
| `logiciel/` | **exactement ce qu'une compagnie reçoit** : `puppetpaie.exe`, `WebView2Loader.dll`, `VERSION`, `donnees/` (barèmes, bases, modèles), `interface/` (la page). Les deux dossiers restent des fichiers visibles et se changent sans changer le moteur (décidé). C'est aussi ce que publie la mise à jour. |
| `fictives/` | les compagnies fictives, livrées aussi (`demo/` avant le 14/10/2026 ; le logiciel renomme l'ancien dossier au lancement) |
| `maj/` | la mise à jour de l'installation, à côté de `logiciel/` : son réglage, la version prête, son journal (jamais dans Git) |
| `prive/` | la compagnie du poste (chez une autre compagnie : la sienne) ; dans aucun dépôt (15/10/2026) |
| `atelier/` | tout le reste : `sources/` (paie, fenetre, fabrique-pdf ; `paie.exe` et les exe Linux/Mac dans `sources/paie/bin/`), `reference/` (les textes qui fondent les règles), `outils/`, `docs/`, `prive-dev/`, `depot/` (versions publiées), plans, maquettes, site. **Le serveur du filet n'est plus ici** (13/10/2026) : il vit dans le dépôt du site de l'éditeur, sous `infra/filet/` |

**Deux dépôts, le code n'est que dans le public (Léandre, 14/10/2026 : « on écrit direct sur le public, sinon on perd
un temps fou en aller-retour »).** Le dépôt `PuppetPaie` (public) est **le seul dépôt du code** : on y crée sa branche, on
y code, on y teste, on y ouvre la PR ; il construit (« Moteur Rust », les exe Windows, Linux, Mac, minutes gratuites) et
publie ; il porte aussi les textes de référence (`atelier/reference/`, 15/10/2026 : « c'est ça qui rend le logiciel
crédible, solide et vérifiable »). Le dépôt `puppetpaieprivate` (privé) ne garde que **le journal** (`journal/` :
`ETAT.md`, passations, topos, archives), `reference-privee/` (les pièces qu'on ne republie pas) et les **tickets** du
filet (ses issues). **Les données de la compagnie ne sont dans aucun dépôt** (15/10/2026) : le poste, la clé USB, le
Drive. Une séance ouvre toujours les deux : le public pour le code, le privé pour lire `ETAT.md`, les tickets, et écrire ce
qui s'adresse à Léandre (topo, marche à suivre, passation, journal) sous `journal/`. Dans les
anciens textes du journal, `prive/journal/` se lit `journal/` du dépôt privé. Rien de réel n'entre jamais dans le public : le
garde-fou (`python3 atelier/outils/verifier_avant_commit.py`) et `donnees_reelles` passent avant chaque commit.

On compile hors du dépôt, sans espace dans le chemin :
`CARGO_TARGET_DIR` dans `%LOCALAPPDATA%\puppetpaie-compilation` du poste, chaîne
`stable-x86_64-pc-windows-gnu` et MinGW (WinLibs), jamais Visual Studio. L'exe de
la fenêtre compilé se copie dans `logiciel/`.

## Par quoi on commence

Un seul document se lit d'office : `journal/ETAT.md` du dépôt privé (s'il est
là), son **point de reprise**. Les archives ne se lisent pas d'office : on cherche
dans `journal/archives/INDEX.md` par `grep` (il pèse plus de 70 Ko), puis on
n'ouvre **que** la section que le chantier du jour concerne.

Les autres s'ouvrent quand on en a besoin, pas avant (ils pèsent plus de 200 Ko,
et ce qu'on lit se repaie à chaque tour de conversation) :

| document | on l'ouvre quand |
|---|---|
| `README.md` | on cherche comment une commande s'utilise |
| `docs/ARCHITECTURE.md` | on touche à un format ou à un calcul |
| `docs/PORTAGE-RUST.md` | on travaille sur le portage Rust, et alors seulement |
| `docs/RUBRIQUES.md` | on ajoute ou corrige une rubrique de paie |

## Une séance, un chantier

Une séance traite **un** chantier, le mène au bout, et s'arrête : deux séances
courtes coûtent moins qu'une longue. Le contexte se surveille comme le dit
`ETAT.md` (« Les plafonds ») : un guet en arrière-plan (`atelier/guet.py`), et **c'est l'utilisateur
qui décide** quand il sonne ; rien ne s'arrête tout seul. En deçà, on ne s'arrête
pas en laissant ouvert ce qui peut se finir.
**C'est l'utilisateur qui dit quand la séance est finie** (04/10/2026) : la passe
de tests de fin ne se lance pas avant qu'il l'ait dit.

En fin de séance : le récit dans `journal/archives/<fichier du mois>`, une
ligne dans `INDEX.md`, et dans `ETAT.md` le point de reprise **remplacé**.

## GitHub Actions : à la fin seulement (l'utilisateur, 01-02/10/2026)

Le quota de minutes est petit. On teste **dans le conteneur**. Pendant le
portage, « Tests » ne tourne qu'à la main, sur Python 3.11 seul. Une PR reste en
brouillon tant que le chantier n'est pas fini.

**« Moteur Rust » NE SE LANCE PAS PENDANT LE PORTAGE.** Il ne sert qu'à livrer
`paie/bin/` au poste de l'utilisateur, qui ne compile pas le Rust. On fusionne
chaque PR du portage sans lui, chaque séance part de `main` sur sa branche ; on
le lance **une seule fois**, à la fin du portage ou le jour où l'utilisateur veut
rouvrir le logiciel. Jamais de soi-même, ni « pour être sûr » ; en cas de doute,
demander.

## Les sous-agents : Opus pour coder, Sonnet ou Haiku pour lire (Léandre, 09/10 et 14/10/2026)

**Ce qui écrit du code ou tranche une conception va à un Opus** (`Agent`, `model: "opus"`) : un lot de portage, une
correction. **Ce qui lit seulement va à un Sonnet ou un Haiku** (`model: "sonnet"` / `"haiku"`), plus rapides et moins
chers (Léandre, 14/10/2026 : « pour faire de la lecture je les trouve hyper performants, moins chers, plus rapides ») :
une recherche en éventail, un inventaire, une relecture ; Haiku pour une recherche simple, Sonnet quand il faut
comprendre ce qu'on lit. Il
rapporte une conclusion courte ; **c'est un rapport, pas une preuve** : il se
vérifie avant de s'affirmer.

**Le rôle de l'orchestrateur : organiser, puis vérifier après coup** (l'utilisateur,
07/10/2026). Il donne à l'Opus une tâche simple — le but, les règles du projet, la
sécurité, la passation attendue — et le laisse faire ; il ne lui dit pas *comment*
faire ce qu'il n'a pas lu lui-même dans le code. Ensuite il vérifie ce qui a été
fait (commits, diff, tests, ce qui est dit prouvé). Être trop directif fait « Claude
puissance Claude » : les doutes de l'orchestrateur s'ajoutent à ceux de l'Opus, une
double précaution qui produit des choses peu saines. Erreur commise une fois (lot I1) :
des pistes de correction écrites sans avoir lu le code, un `cargo test` redemandé
alors qu'il venait d'être mesuré, un lot élargi sans demander.

- Ne se délègue pas : une décision de conception, un acte qui engage (dépôt réel,
  envoi, signature), tout ce qui toucherait au coffre. Les règles de ce fichier
  valent pour le sous-agent ; on les lui rappelle (« ne lis pas `prive/config/*.bin` »).
- **Plusieurs Opus à la fois** (Léandre, 14/10/2026 ; on est monté à quatre) : des lots
  disjoints, chacun dans son worktree et sa cible Cargo ; l'orchestrateur vérifie
  chaque passation après coup, un lot à la fois.
- **Un Opus ne réécrit jamais l'historique** (`--amend`, `rebase`, `reset`) : HEAD
  peut porter des commits qui ne sont pas les siens, déjà poussés (le 04/10/2026, un
  Opus a amendé un commit de l'orchestrateur). **Il ne laisse aucune tâche de fond
  derrière lui** (le même jour, trois boucles d'attente ont tourné à vide). Son
  compte rendu se vérifie : « ne sert d'oracle à aucun test » était faux, `grep` le
  dit en une ligne.
- Pour le portage, les règles des consignes sont dans `ETAT.md`, les consignes d'un
  lot dans `journal/<lot>/consignes-*.md` (un Opus par lot entier ; `cargo test` seulement, **aucune suite Python** ; commit sans pousser).
- **Aucun repli Python** (l'utilisateur, 04/10/2026 : « on fait un logiciel totalement
  en Rust, je ne veux plus de Python »). Une route se porte entière ; ce qui ne se
  porte pas proprement s'arrête et remonte à l'utilisateur, il ne reste pas en Python.
  **On ne parle jamais de jetons à un Opus** : l'orchestrateur guette.
- **Un script de test ou de comparaison n'imprime jamais une requête
  d'authentification ni son corps** (03/10/2026) : le mot de passe de
  `NET_ENTREPRISES_MOTDEPASSE` y est, et une sortie d'outil reste dans le fil. On
  masque la valeur avant d'afficher quoi que ce soit qui touche au réseau.

## Les sources : tout texte qu'on lit se dépose (Léandre, 10/10/2026)

**Toute règle vient d'un texte, et tout texte qu'on a lu pour décider est dans
`atelier/reference/sources/` du dépôt public, avant que la décision soit écrite dans le code.** Pas d'exception
pour « ce qu'on sait », ni pour une page lue en passant, ni pour un sous-agent : ce qu'il lit, il le
dépose, ou il le nomme pour que l'orchestrateur le dépose. Une règle dont le texte n'est pas dans le
dépôt n'est pas une règle, c'est un souvenir.

Comment on dépose :
- **le texte brut**, tel que servi, nommé `<émetteur>-<objet>-<date AAAAMMJJ>.<ext>` (la date du
  texte ou du relevé), et à côté sa transcription `.md`/`.txt` quand le brut ne se lit pas (`pdftotext`) ;
- **un `LISEZ-MOI.md` par dossier** : d'où ça vient (l'URL), quand c'est relevé, ce qu'on en lit (les
  paragraphes, les phrases), **où le moteur s'en sert** (le fichier, la fonction ou la note `_source`) ;
- ce qui est une **inférence** (un texte appliqué à un cas qu'il ne nomme pas) s'écrit comme telle ;
- **les codes** (travail, sécurité sociale, impôts, CGCT) ne se copient pas à la main : on ajoute l'article
  à `TEXTES` dans `outils/relever_les_codes.py` et on relève depuis la base LEGI (`moissonner`, puis
  `rapport` ; Légifrance refuse les robots, le relevé se refait et dit ce qui a bougé) ; **la norme DSN**
  est dans `norme-dsn/texte/` par bloc ; **le BOFiP** se dépose brut dans `bofip/` (identifiant + date de
  version) ; le BOSS se dépose depuis sa page quand elle répond, sinon on dépose ce qu'on a lu en disant
  d'où ça vient, et on note qu'il reste à redéposer ;
- la note `_source` d'un barème, le commentaire d'une fonction, la passation d'un Opus **citent le fichier
  déposé**, pas une URL seule ;
- **ce qui ne se republie pas** (Léandre, 15/10/2026) : un document d'organisme privé (Audiens, Afdas, CASC-SVP,
  Agirc-Arrco…) entre par sa **transcription** et son URL, l'original va dans `reference-privee/` du dépôt privé ; une
  pièce réelle (bulletin, courrier adressé à une compagnie, relevé sur nos DSN) ne va **que** dans `reference-privee/`,
  et le public n'en garde que l'attendu, sans nom ni NIR (règle 9 a). `atelier/reference/` ne se convertit jamais
  (`-text` dans `.gitattributes`) : des empreintes le vérifient.

## Les fichiers qu'on ne lit pas

```
prive/config/coffre.bin          les mots de passe net-entreprises, Urssaf et de la boite courriel
prive/config/google-jeton.bin    le jeton du compte Google
prive/config/maj-cle-privee.bin  la cle qui signe les mises a jour (editeur)
maj/cle-github-graine.txt        la graine de la cle de GitHub, le temps que Leandre la colle (lot P1)
```

**Ne jamais lire, afficher, copier ni recopier le contenu de ces fichiers**, ni
chercher à en extraire une valeur — pas plus en les décodant qu'en passant par
`coffre.lire()` pour en imprimer le résultat. C'est une **convention, pas une
garantie technique** : la clé maîtresse du coffre est scellée deux fois, par le
mot de passe du logiciel **et** par la clé du poste (`cle-poste.bin`, dans
`~/.config/puppetpaie/` ou `%APPDATA%\PuppetPaie\` ; dans un dossier **portable**, marqué
`PORTABLE.json` à côté de `logiciel/`, elle vit dans ce dossier et voyage avec lui, 14/10/2026) ; sur le poste de
l'utilisateur, un agent qui tourne sous son compte *peut* donc les ouvrir **sans
qu'aucun mot de passe soit tapé** (c'est ce qui permet les dépôts de test sans
demander). Hors de ce poste (dépôt Git, conteneur), ni le coffre ni la clé ne
sont là, et la règle ne coûte rien. Ce qui tient la règle, c'est qu'elle
est écrite ici et qu'aucune commande du moteur n'affiche jamais un mot de passe —
ni en sortie, ni en erreur, ni dans un journal. À la place, et cela suffit
toujours : `paie coffre` (ce qui est posé, depuis quand, pour quel usage).

## Ce qu'un agent peut faire, et à quelle condition

| | condition |
|---|---|
| lire, calculer, produire un mois, sauvegarder, relever les retours | aucune |
| **déposer en test** (`depot-dsn --test`, `deposer-dpae --test`…) | aucune, **sans demander d'aval** (l'utilisateur, 29/09/2026) |
| **déposer une DSN ou une DPAE en réel, envoyer, faire signer** | **le mot de passe du logiciel, tapé par un humain au moment de l'acte** |
| **approuver une publication** (l'environnement `publication` du dépôt public) | **jamais un agent** : les sessions Claude agissent sur GitHub sous l'identité de Léandre (vu le 14/10/2026) ; seul Léandre approuve |

**DEUX MOTS DE PASSE, DEUX CHEMINS, ET PAS D'AUTRE (l'utilisateur, 23/09/2026).**
- **Ouvrir le coffre = la séance.** `paie coffre seance --fenetre` :
  une boîte, une fois, 12 h. Un agent la lance **avant** sa première commande
  qui touche au coffre. Toute commande qui trouve le coffre fermé ouvre elle-même
  cette séance dans une boîte — jamais par `getpass`. Sans terminal, `_demander`
  refuse tout de suite. Un dépôt de test peut avoir besoin de cette séance : pour
  ouvrir le coffre, pas pour autoriser l'acte.
- **Un acte réel = le popup, à chaque fois.** La séance n'est **jamais** une
  preuve pour un acte réel.

**Ne pas contourner** : un agent ne tape pas le mot de passe à la place de
l'utilisateur, ne pose pas `PUPPETPAIE_MOTDEPASSE` ni la variable d'un organisme,
n'ouvre pas le coffre dans un script à côté de la commande. Le dépôt réel engage
l'employeur — jusqu'à des ordres de télépaiement réellement prélevés. Si la clé
du poste n'ouvre pas le coffre (voir la mémoire « AppData virtuel »), le dire et
donner la commande à lancer.

Un dépôt de test ne conserve rien et c'est le **seul validateur** du projet
(DSN-Val écarté le 15/09/2026) : un agent qui a un fichier à contrôler le dépose
en `--test` et lit le bilan, sans s'inventer un garde-fou que ce document ne pose
pas.

### Sur un poste distant (le dépôt Git, sans `prive/config/*.bin`)

**Un dépôt RÉEL y est mécaniquement impossible** : `operations_socle._verifier_preuve`
exige le mot de passe du logiciel, qui vit dans `coffre.bin`, absent du dépôt, et
lève `PreuveInsuffisante` **avant** d'aller chercher celui de l'organisme.

**Un dépôt de TEST y est possible si — et seulement si — `NET_ENTREPRISES_MOTDEPASSE`
est posée** : c'est l'utilisateur qui l'a posée, pour cet usage. Un agent ne la
pose, ne la modifie ni ne l'affiche, et ne s'en sert que pour un test. Pour savoir
si elle est là sans lire sa valeur :

```bash
python -c "import os; print('posee' if os.environ.get('NET_ENTREPRISES_MOTDEPASSE') else 'absente')"
```

## Faire tourner le code dans le conteneur

**Plus de Python dans le produit (lot K, 09/10/2026).** Le logiciel est le moteur
Rust (`atelier/sources/paie/`, programme `paie`) et sa fenêtre
(`atelier/sources/fenetre/`, programme `puppetpaie`). Les référentiels livrés sont
dans `logiciel/donnees/` (barèmes, bases, modèles), la page dans `logiciel/interface/`. Le Python ne subsiste que dans l'atelier
(`outils/` : extracteurs, relevés, garde-fou de publication), à porter.

```bash
apt-get install -y libwebkit2gtk-4.1-dev libsoup-3.0-dev librsvg2-dev xvfb   # pour la fenetre seulement
(cd atelier/sources/paie && cargo build --release)
```

**Le moteur Linux est déjà dans le dépôt** (Léandre, 14/10/2026) : `atelier/sources/paie/bin/linux/paie` (et la fenêtre,
`bin/linux/puppetpaie`), livrés par « Moteur Rust » ; `bin/SOURCES` dit de quelles sources. Pour reproduire un
comportement du poste (une route, un envoi), on s'en sert tout de suite plutôt que de compiler :
`echo '{"route":"…","charge":{…}}' | atelier/sources/paie/bin/linux/paie route --racine <copie d'une compagnie>`.
Il peut retarder sur les sources : ce qu'on vient de changer se compile, et les tests compilent toujours.

### Les tests

**Tout doit être vert. Une PR dont la suite est rouge ne se fusionne pas.**

```bash
cd atelier/sources/paie && rm -rf "${CARGO_TARGET_DIR:-target}/tmp"     # les dossiers de travail des tests (~3 Go)
cargo nextest run --cargo-profile essais --no-fail-fast                  # le moteur ; la fenetre n'a pas de tests
```

La suite de l'atelier, à part, dans sa propre cible, seulement quand on touche à ce qui est sous
`#[cfg(feature = "atelier")]` (`src/atelier.rs`, `depot_acte.rs`, `commandes/actes.rs`, `main.rs`…) :

```bash
CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-target}-atelier" cargo nextest run --cargo-profile essais --features atelier \
  --lib --test atelier --test fichiers -E 'test(/(^|::)atelier::/)'
```

- **`cargo nextest`** (11/10/2026, décision D ; `cargo install cargo-nextest --locked` sur le poste) : un processus par
  test, les binaires en parallèle. `cargo test --profile essais` reste vert aussi.
- **La suite de l'atelier** (13/10/2026) prouve `paie-atelier` : la redirection vers un faux serveur local, le refus de
  tout acte réel. `--features atelier` vaut pour toute la caisse (le `paie` de `CARGO_BIN_EXE_paie` y porte alors la
  redirection) : elle ne compile que les trois binaires de tests qui en ont (`--lib`, `atelier`, `fichiers`), n'en joue
  que les modules `atelier`, et vit dans `<cible>-atelier`, jamais dans la cible de la suite livrée. La suite livrée
  prouve, elle, que `paie` ne contient pas la redirection (`tests/atelier.rs`, module `livre`).
- **Le profil `essais`** (10/10/2026, `paie/Cargo.toml`) : le code optimisé sans le LTO de l'exe livré, qui se
  repayait pour chacun des binaires de `tests/`. Dans le nuage, la suite se compile d'avance dès l'ouverture
  (`.claude/hooks/session-start.sh`). Les doutes sur l'utilité de certains tests : `journal/tests/DOUTES-TESTS.md`.
- **Quatre binaires par domaine** (11/10/2026, §226) : `tests/calcul/`, `tests/dsn/`, `tests/routes/`, `tests/fichiers/`,
  un `main.rs` qui déclare un module par fichier. Un fichier seul :
  `cargo nextest run --cargo-profile essais -E 'binary(calcul) & test(/^clauses::/)'`. Un fichier qui pose ou retire
  une variable d'environnement (`set_var`) **ne se groupe pas** : il reste seul dans `tests/`, dans son processus ; un
  fichier neuf va dans le groupe de son domaine.
- **Les aides sont dans `tests/outils/`** (copier, dossiers, compagnies, route et son bac à sable, lecteurs de DSN,
  faux serveur, faux mandataire, faux `Courriel`) : un fichier n'en recopie aucune. Un test se monte dans
  `outils::dossier_de_test(fichier, test)` ; un modèle monté une fois par processus, dans
  `outils::dossier_du_processus`, jamais sous un nom partagé (sous nextest, les processus se le videraient).
- **Ce qu'un test prouve** (12/10/2026, §227) : une règle, qui cite son texte déposé, ou un contrat (une route refuse et
  rien n'est écrit ; un fichier a la forme que le moteur relit). Jamais une forme : ni classe d'exception ni message venu
  du Python, ni l'ordre des clés d'un objet JSON. Le nom du test dit le fait. Un oracle de `tests/donnees/` se compare et
  se réécrit par le test qui le lit (`outils::oracle`, `PUPPETPAIE_REECRIRE_ORACLES=1`, lue, jamais posée par un test ;
  son diff se relit au commit) et ne garde que ce qu'aucun autre test ne prouve ; une grille exhaustive se joue sur un
  échantillon à graine fixe (`outils::tire`). Une copie de compagnie ne prend que ce dont le cas a besoin
  (`compagnie::config_et_mois`, `ses_dsn`), `logiciel/donnees` seulement pour le modifier. Un fichier porte le nom de
  son domaine, jamais d'un lot ; une fonction pure se teste en `#[cfg(test)]` dans `src/`.
- **Une cible par Opus, un seul profil** (12/10/2026) : `CARGO_TARGET_DIR` seul, jamais `CARGO_INCREMENTAL` ni une
  fonctionnalité en plus (le profil entre dans le hachage de Cargo : chaque variante est un jeu complet d'artefacts,
  5 Go au lieu de 2) ; `target/tmp` se vide avant une suite.
- Les tests tournent sur des compagnies fictives et sur la **copie figée des
  fictives livrées**, `paie/tests/donnees/fictives/` : `fictives/` vit (le poste la fait
  jouer et la commite), un test ne la lit jamais.
- **Le minimum de tests : une fois, à la fin, avant de pousser (l'utilisateur,
  29/09 et 04/10/2026).** Pendant le travail, on corrige sans relancer.
- **Une seule commande d'attente à la fois** (09/10/2026) : une suite longue se
  lance en arrière-plan, une fois, et on attend sa notification ; jamais de
  boucles `until` empilées.

## Les règles de fond du logiciel

0. **Ce logiciel n'est écrit pour aucune compagnie en particulier.** Logiciel de
   paie du spectacle vivant, pour toute compagnie qui emploie artistes et
   techniciens — intermittents au cachet comme permanents. La compagnie du poste
   est le **banc d'essai**, pas le destinataire. **Ce qu'elle ne fait pas, d'autres
   le font** (un permanent, la CCNEAC, le SYNDEAC, les jours fériés chômés) : un cas
   qu'aucune de nos données n'éprouve reste un cas à porter. « Ça n'arrivera jamais
   chez nous » n'a jamais justifié de ne pas prévoir la case.
1. **Rien ne se devine.** Un taux de PAS, un identifiant DGFiP se *lisent* (du
   compte rendu métier) ; un montant de bordereau Urssaf se *calcule* depuis la
   table officielle des CTP (`paie/src/bordereau.rs`). Le moteur refuse de
   produire plutôt que d'inventer. Ne pas « déboguer » un refus en fournissant une
   valeur plausible.
2. **Toute décision vit dans le moteur, jamais dans un front.** Les deux faces —
   ligne de commande et interface — ne font qu'afficher ; un garde-fou ajouté dans
   un front se contourne par l'autre. Côté Python, `operations.py` est une façade
   sur des modules par domaine (`operations_*.py`). **Plus de Python neuf**
   (`docs/PORTAGE-RUST.md` §14) : ce qui s'écrit s'écrit en Rust, dans `paie/`.
3. **Les modules de transport ne cherchent jamais un secret.** Ils reçoivent un
   mot de passe ; ils n'ouvrent pas le coffre. `tests/test_ouverture.py` échoue si
   l'un d'eux s'y met.
4. **Ne jamais régénérer une déclaration déjà déposée** pour la redéposer : les
   numéros de contrat se renumérotent dès que les compteurs avancent, et un
   annule-et-remplace aux numéros changés ne remplace plus rien de reconnaissable.
   Un numéro de contrat ne se recalcule pas, il se lit.
5. **Deux canaux de retours, pas un.** La DSN répond sur net-entreprises, la DPAE
   sur `consultation.dpae-edi.urssaf.fr` — services, natures (41 = certificat de
   conformité des DPAE), mots de passe et archives distincts (`retours/` et
   `retours/dpae/`). Chercher une DPAE dans les retours DSN ne rend rien. Un **404
   sur une plage de consultation veut dire « rien sur cette plage »** ; un **accusé
   de transport `OK` n'est pas une déclaration acceptée** — c'est le bilan
   (CCO/BAN) qui tranche.
6. **Une nature de contrat n'est pas un détail.** `donnees/contrats-base.json` dit ce
   que la DSN écrit de la structure d'un contrat, par nature (`cddu_spectacle`,
   `cdi`, `cdd`) ; un engagement qui ne dit rien est un CDDU. Le **numéro d'objet
   spectacle est INTERDIT** hors des natures 02 et 10 (CCH-11) : un permanent ne
   se rattache à aucun spectacle. Le barème a des **axes qui s'empilent** — statut,
   nature, santé, catégorie — et la pile ne s'écrit qu'à **un seul endroit**
   (`calcul.bareme_applique`) : en sauter un étage, c'est déclarer les cotisations
   de quelqu'un d'autre. La prévoyance des permanents vient de la **convention
   collective**, livrée avec le logiciel ; un **cadre permanent** lit
   `par_categorie.cadre_permanent`, pas `cadre`. Le détail (taux, CASC, ce qui se
   réclame salarié par salarié) : `docs/ARCHITECTURE.md`, « La nature du contrat ».
7. **Deux contrôles avant tout dépôt, et ils ne disent pas la même chose.**
   `usages.py` dit si une rubrique a le **droit** d'être là pour la nature de
   déclaration ; `formats.py` dit si ce qu'on y a **mis** a la bonne forme
   (longueur, expression CSL). Les relevés viennent d'`outils/extraire_usages.py`
   et `outils/extraire_formats.py`, confrontés à ce qui a été réellement accepté.
   Un contrôle faux refuserait du licite : c'est lui qu'on retire, jamais la
   déclaration.
8. **Un barème a trois étages** : le **socle** livré (`donnees/baremes/<année>.json`,
   droit commun et spectacle), la couche de la **convention collective** choisie
   par l'`idcc` de la fiche (`par_convention` — 3090 et 1285 ; une convention sans
   couche **refuse**), et la **surcouche** de la compagnie (`config/baremes/<année>.json` :
   son taux AT/MP, ce que son effectif change). L'effectif est dans
   `config/effectifs.json`, écrit par le moteur depuis les retours de nature 118.
   `Contexte._bareme_brut` est le seul endroit qui empile (`baremes.fusionner`) et
   qui contrôle ; un fichier sans marqueur `socle` est un barème complet pris tel
   quel. Le taux AT/MP ne s'écrit **qu'une fois** (`calcul.bareme_atmp`, les
   rubriques portent leur facteur). Le détail (seuils d'effectif, compagnie neuve
   ou ancienne, établissements) : `docs/ARCHITECTURE.md`, « Le barème a trois étages ».
9. **Trois destinations, et les données de la compagnie dans aucun dépôt (Léandre, 15/10/2026).**

   | | ce que c'est | où ça va |
   |---|---|---|
   | `logiciel/`, `fictives/`, `atelier/` (dont `atelier/reference/`) | le programme livré, son code, les taux que la loi et les conventions fixent pour tout le monde, et les textes qui les fondent | le **dépôt public** |
   | `journal/` et `reference-privee/` | le journal de l'atelier ; les pièces lues qu'on ne republie pas (originaux d'organismes, exemples réels) | le **dépôt privé**, avec les tickets du filet |
   | `prive/` | les données de la compagnie : config, barème, paies, sorties, retours | **aucun dépôt** : le poste, la clé USB, le Drive, comme chez toute compagnie (`/prive/` est dans le `.gitignore` du public) |

   Un conflit sur un journal JSON se résout enregistrement par enregistrement, sans
   rien perdre d'un côté ni de l'autre.

   a) **Les tests.** La suite tourne sur des compagnies fictives et sur la copie
      figée des fictives livrées, et ne lit rien de `prive/` :
      `paie/tests/fichiers/donnees_reelles.rs` refuse, dans `logiciel/`, `fictives/`,
      `atelier/sources/` et `atelier/reference/`, un NIR non inventé, le SIREN et le nom de la compagnie du
      poste, et un chemin vers `prive/` construit par un test (la racine par défaut du
      moteur, `prive/` voisine du code, est celle du poste par conception : un test
      nomme toujours sa compagnie). Les preuves sur le réel (`prive/tests/`) sont retirées (décision C,
      11/10/2026) : ce qu'on garde d'une compagnie réelle, ce sont ses attendus
      indépendants du moteur, transcrits sans nom ni NIR dans un test fictif ; un cas
      qui a attrapé une vraie erreur se **réécrit d'abord en fictif** — retirer une
      valeur réelle avant, c'est retirer une preuve. Un test qui ne prouve
      qu'une règle s'écrit sur la fictive. Les fictives sont celles livrées avec le logiciel
      (`fictives/`, copie figée `paie/tests/donnees/fictives/`) : `docs/COMPAGNIES-FICTIVES.md`.
   b) **La préparation d'une publication est un chantier à part, en sommeil** : on
      n'en parle pas dans les comptes rendus ; ce qu'elle demandera est dans
      `AVANT-LE-PREMIER-COMMIT.md` et `PLAN-DEPOT-PUBLIC.md`, et c'est
      l'utilisateur qui l'ouvre.

   Ce qu'on fait sans en parler : n'écrire dans `logiciel/` que ce qui vaut pour
   toutes les compagnies, laisser dans `prive/` ce qui n'appartient qu'à celle-ci.
10. **Test = réel (l'utilisateur, 26/09/2026).** Un dépôt de test accepté (brique
    de contrôle, puis `OK, 0 anomalie`) vaut un dépôt réel : même moteur, même
    fichier, même circuit, seul l'indicateur d'essai change. On ne présente
    **jamais** un acte « prouvé en test, pas en réel » comme un risque, et on ne
    réclame pas un dépôt réel pour tenir une chose pour acquise. Reste ouvert ce
    que le test ne montre pas : les retours qu'un organisme n'émet qu'en réel (AER,
    taux de PAS). Le garde-fou test → réel protège d'un envoi par erreur ; il ne
    met pas le moteur en doute.
11. **La copie de développement ne se voit pas dans l'interface.** `prive-dev/` ne
    figure jamais à l'accueil, mode développeur ou non ; elle reste en ligne de
    commande (`paie --dev ...`). L'interface s'essaie sur les
    compagnies fictives.

## Les fins de ligne : le code en LF, les données telles quelles

Le code et les docs sont en **LF** (`.gitattributes`, `text eol=lf` ;
`tests/test_fins_de_ligne.py` le vérifie). Les `.cmd` et `.bat` restent en CRLF.
Les formats à **largeur fixe ne se convertissent jamais** (gabarits AEM et DPAE,
DSN, AEM de CoolPaie, retours, `.dsn`), ni ce dont on vérifie une empreinte (le
`.csv` de l'Urssaf, les sources de `sources.json`) : ils se comparent octet pour
octet. Un fichier hors règle **garde la fin de ligne qu'il a** : `newline=''`,
`read_bytes()`, et pas d'éditeur qui « corrige ».

## Publier une version

Ce poste est celui de l'éditeur (le réglage de l'installation, `maj/reglage.json`, ou en
repli `prive/config/maj.json` : `"editeur": true`) : il relit les sources (`paie sources
--verifier`, lot S1 du 13/10/2026) et **publie**. Toute installation reçoit, la sienne
comprise (Léandre, 14/10/2026) : la mise à jour est celle du logiciel, une par installation,
cherchée au lancement et appliquée au lancement suivant (`paie maj`, `paie/src/maj.rs`). **On est en développement** : la première
version diffusée portera toutes les conventions visées ; ne pas signaler qu'aucune
version n'a été publiée depuis longtemps.

**Depuis le 14/10/2026 (lot P1), on publie depuis GitHub** (`journal/finitions/PUBLIER.md`) :

1. **À la fin de chaque séance de code**, suite verte, la PR du dépôt public sur sa `main` (Léandre la fusionne). Le numéro de version
   n'est plus à augmenter : le flux pose la date du jour dans `logiciel/VERSION` (une version par jour).
2. Léandre lance le flux « Publier » du dépôt public (`.github/workflows/publier.yml`) ; `notes`, et `calcul`
   **seulement** si la version change un calcul de paie (la seule phrase qu'une compagnie verra ; un taux qui change
   n'en est pas une) ; `exigee` plus rarement encore (norme DSN qui change, déclaration que l'ancienne version ferait
   fausse : elle bloque tout dépôt réel tant qu'elle n'est pas appliquée).
3. **Léandre approuve** (environnement `publication`) ; le moteur signe (`paie maj --publier-github`, la clé de GitHub,
   `CLE_PUBLIQUE_GITHUB`) et une Release porte l'archive, `derniere.json` et l'installeur, que toutes les installations
   lisent (`DEPOT_PAR_DEFAUT`).

Les installations acceptent deux clés : celle de GitHub et celle du poste (`paie maj --publier`, qui écrit dans un
dossier, reste possible). La clé du poste est `prive/config/maj-cle-privee.bin` — on ne la lit pas ; la graine de
GitHub n'existe que dans le secret de l'environnement (et, le temps de la coller, dans un fichier que Léandre efface). Une version qui change un calcul attend la
DSN des mois saisis des compagnies réelles de l'installation avant de s'appliquer (`--malgre-calcul` pour passer outre),
sauf si elle est exigée. Ne jamais appliquer une mise à jour pendant que
l'interface tourne ; ne pas republier un numéro (les deux sont refusés).

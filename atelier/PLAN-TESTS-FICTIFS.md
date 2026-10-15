# Plan : les tests passent sur les compagnies fictives

Écrit le 25/09/2026, à la demande de Léandre, pour être **repris en ligne**,
depuis une séance à distance. Ce plan fait foi : on le lit, on ne le refait pas.

## Pourquoi maintenant

`outils/exporter.py complete` sort la version complète **avec ses tests**.
Or ces tests lisent les données réelles de la compagnie du poste : des NIR, des
salaires, des adresses. Tant que c'est le cas, **la version complète ne se
donne à personne** : ni aux compagnies amies (janvier 2027), ni au dépôt
public. La commande d'export le rappelle à chaque fois.

## La règle : déplacer, pas jeter

On ne « vire » pas la compagnie du poste des tests, on la **déplace**.

- **Les tests livrés avec le logiciel** (`logiciel/tests/`) tournent sur les
  compagnies fictives. Ils ne lisent plus rien de `prive/`.
- **Les tests sur les données réelles** partent dans `prive/tests/`, qui ne
  sort jamais du poste ni du dépôt privé. Ce sont eux qui prouvent le plus :
  les bulletins rejoués au centime contre CoolPaie, les DSN comparées à ce que
  l'Urssaf a vraiment accepté. Aucune compagnie fictive ne peut prouver ça,
  parce qu'aucun organisme ne l'a jamais validée en vrai.

**D'abord fabriquer, ensuite basculer** (CLAUDE.md §9 a) : un test réel ne
quitte `logiciel/tests/` que lorsqu'un test fictif prouve la même chose, ou
qu'il a sa place dans `prive/tests/`. On ne retire jamais une preuve pour
alléger.

## Ce qu'on a déjà

- `outils/compagnies_fictives.py` : **16 compagnies** montées sans rien
  emprunter à `prive/` (`--lister`). Sept ont été déposées en test le
  21/09/2026, toutes `OK, 0 anomalie` (`docs/COMPAGNIES-FICTIVES-DEPOT.md`).
- `outils/compagnies_demo.py` et `demo/` : **4 compagnies** payées de
  janvier 2025 à novembre 2026 par le vrai moteur (3090 et 1285, grandes et
  petites, permanents et intermittents).
- `moteur/compagnies.py` : une fictive porte le marqueur
  `COPIE-DE-DEVELOPPEMENT.json`, et rien de réel ne part d'elle.

## L'inventaire (relevé au grep le 25/09/2026, à affiner)

**44 fichiers sur 74 ouvrent le contexte du poste** : `Contexte()` sans
racine, `RACINE`, `CONFIG` ou `racine_donnees()`. Sur un poste sans `prive/`,
ils lisent ce que `PUPPETPAIE_DONNEES` désigne, ou rien.

```
test_allegements test_arrets test_arrivee test_bareme_socle test_cadre
test_catalogue test_champs test_compagnies_fictives test_contrats_plurimensuels
test_documents_salarie test_dossier_salarie test_dpae_depot
test_dsn_sans_individu test_echeancier test_effectif test_emplois test_etats
test_exercice test_exports test_formats test_importation test_incidents
test_interface test_json test_kali test_maj test_non_regression test_ntt
test_numero_de_l_annee test_operations test_pas_defaut test_permanents
test_permanents_chaine test_prevoyance test_questionnaire test_regularisation
test_repetition test_retours test_retours_rates test_rubriques test_signatures
test_smic test_solde_taxe_apprentissage test_usages
```

**Des valeurs réelles écrites en dur dans le test** : ressemblant à des NIR
dans `test_casc`, `test_champs`, `test_dpae_depot`, `test_etats`,
`test_importation`, `test_incidents`, `test_ntt`, `test_operations`,
`test_questionnaire`, `test_retours`. Le SIRET de la compagnie du poste est dans `test_casc`.

**L'export CoolPaie** (l'historique réel) est lu par `test_exports`,
`test_formats`, `test_importation`, `test_non_regression`, `test_retours`,
`test_solde_taxe_apprentissage`.

Ces listes viennent d'un grep grossier. Une ligne qui ouvre `Contexte()` peut
très bien n'utiliser que le socle livré (barèmes, catalogues) et rien de la
compagnie. C'est l'étape 1 qui tranche, fichier par fichier.

## La marche à suivre

1. **Classer chaque fichier dans une des trois piles**, dans un tableau
   ajouté à ce document :
   - **A, déjà propre** : il ne lit que le socle livré, ou un bac à sable
     qu'il fabrique lui-même. Rien à faire.
   - **B, à basculer** : ce qu'il prouve, une compagnie fictive peut le
     prouver aussi (un calcul, une forme de DSN, un document, une garde).
     On le réécrit sur une fictive, ou sur un bac à sable.
   - **C, à déplacer** : il prouve contre du réel accepté (rejeu CoolPaie
     au centime, DSN déposées, retours d'organismes). Il part dans
     `prive/tests/`.
2. **Un point d'entrée unique pour les fictives dans les tests** : une
   fixture `conftest.py` qui monte une compagnie fictive dans un dossier
   temporaire et pose `PUPPETPAIE_DONNEES` dessus, pour tout le fichier.
   Aucun test ne doit plus pouvoir lire `prive/` par défaut.
3. **Basculer la pile B**, fichier par fichier. Un commit par fichier ou par
   petit groupe, la suite verte à chaque fois. On note ce que prouvait
   l'ancien test et ce que prouve le nouveau.
4. **Déplacer la pile C** dans `prive/tests/`, avec son propre `conftest.py`
   qui pointe sur `prive/`. Elle se lance à part :
   `python -m pytest ../prive/tests -q`.
5. **Le verrou** : un test qui échoue si un fichier de `logiciel/tests/`
   contient un NIR plausible, le SIREN de la compagnie du poste, ou ouvre `prive/`. Il
   remplace le vieux `outils/verifier_avant_commit.py` (porté en Rust le
   15/10/2026 : `paie verifier-avant-commit`), qui ne juge plus rien.
6. **La preuve finale** : `outils/exporter.py complete`, puis la suite lancée
   **dans le paquet exporté**, qui n'a pas de `prive/`. Si elle passe là-bas,
   le paquet se donne. Retirer alors l'avertissement de l'export.

## Ce qu'une séance à distance a, et n'a pas

- **Elle a** : le code et les données (le dépôt privé porte tout, CLAUDE.md
  §9), donc `prive/`, `demo/` et l'export CoolPaie. Le classement et la
  bascule se font entièrement à distance.
- **Elle n'a pas** : le coffre ni la clé du poste. Pas de dépôt réel, et un
  dépôt de test seulement si Léandre a posé `NET_ENTREPRISES_MOTDEPASSE`.
  Ce chantier n'en a pas besoin.
- **Avant de commencer** : `pip install -r logiciel/requirements.txt`
  (cffi, sinon des dizaines de faux échecs ; les PDF passent par la fabrique).
- **Échec déjà connu, pas une régression** :
  `test_solde_taxe_apprentissage::test_les_avrils_de_l_historique_tombent_juste`.
  Il vient de `prive/journal/depots.json` (la DSN d'octobre 2025 sans nom de
  fichier). Il ira de toute façon en pile C.
- **Les fins de ligne** : jamais `sed -i`, voir CLAUDE.md. Relire
  `git diff --stat` ligne par ligne avant chaque commit.
- **Une PR par séance.** Sur le poste de Léandre on commite sur `main` avec
  `git add -A` ; à distance, on ouvre une branche et une PR.

## Terminé quand

- `logiciel/tests/` ne lit plus rien de `prive/`, et le verrou de l'étape 5
  le garantit ;
- `prive/tests/` garde toutes les preuves sur du réel, et passe ;
- la suite passe dans un paquet `complete` exporté, sans `prive/` à côté.

---

## Où on en est — 25/09/2026, séance à distance

**Les six étapes sont faites** ; il reste la pile B à réécrire (plus bas).

- `logiciel/tests/` : **1119 tests, tous verts**, sur la compagnie fictive
  `privee-artistes`, montée par `conftest.py` (point 6) dans un dossier
  jetable, sur août 2026 et avec le barème 2025.
- `prive/tests/` : **159 tests** (`python3 -m pytest ../prive/tests -q`
  depuis `logiciel/`), **tous verts**. L'échec connu de
  `test_solde_taxe_apprentissage` est réglé : cinq DSN réelles du journal
  (2025-10 à 2026-01, et 2026-08) n'avaient pas de nom de fichier. Il a été
  retrouvé par concordance, et `_fichier_retrouve` dit comment : SIRET,
  période, nature, envoi réel, date de constitution égale au jour du dépôt,
  seul candidat du mois. Ce n'est pas une preuve par empreinte, le hash de
  l'accusé n'étant pas le SHA-1 du fichier.
- 1119 + 159 = 1278 = les 1273 d'avant et les 5 du verrou. **Aucun test
  n'a été retiré** : chacun a été soit gardé, soit réécrit, soit déplacé.
- La CI lance les deux suites.
- Le paquet `complete` exporté, qui n'a pas de `prive/`, passe sa suite
  (2 échecs au premier essai, dus au lanceur manquant du verrou, corrigé
  depuis). L'avertissement de l'export est retiré.

### Ce qu'il a fallu au logiciel

- **Les modèles de documents livrés** (`moteur/modeles/`) : le contrat de
  CDDU, le certificat de travail, le reçu pour solde de tout compte, et un
  gabarit DPAE **neutre** (celui de `compagnies_demo`). Ils ne vivaient que
  dans `prive/gabarits/` : sans `prive/`, aucune compagnie, fictive ou non,
  ne produisait de contrat ni de DPAE. `contexte.modele_de` cherche d'abord
  chez la compagnie, puis au poste, et prend le modèle livré en dernier
  recours. Sur le poste de Léandre, rien ne change.

### Le classement (étape 1)

On a mesuré plutôt que deviné : la suite lancée sur la fictive, puis dans
une copie sans `prive/` à côté (même résultat, donc aucune lecture de
`prive/` par un autre chemin). On a aussi cherché les tests **verts à
vide** : ceux qui s'arrêtent sur un `return` quand les données manquent.
Ils passaient sur la fictive sans rien vérifier.

**A — propres, rien à faire** : les 46 fichiers (sur 73) qui ne sont pas
cités ci-dessous.

**B — basculés sur la fictive** :

| fichier | ce qui a changé |
|---|---|
| `test_permanents_chaine` | le bac partait d'une fiche réelle ; il part de BRIOCHE, un salarié fictif. 112 tests remis d'aplomb, dans ce fichier et chez ceux qui l'importent (`test_fctu`, `test_clauses_3090`, `test_feries`) |
| `test_bareme_2025` | la fictive reçoit le barème 2025 (`conftest.py`) |
| `test_exports`, `test_etats`, `test_exercice` | la fictive est montée sur août 2026, le mois que leur bac recopie |
| `test_casc`, `test_etats`, `test_questionnaire` | deux NIR réels (celui de Léandre et celui d'une salariée), le SIRET et l'identité de la compagnie du poste, remplacés par des valeurs inventées à clé juste |

**C — les preuves sur le réel**, déplacées dans `prive/tests/test_<x>_reel.py`
(le fichier entier, ou seulement les tests en cause, aides comprises) :

| fichier | pourquoi |
|---|---|
| `test_non_regression` (34 tests) | le rejeu CoolPaie : bulletins, AEM et DSN comparés au centime. Ses 28 tests sur le code restent dans le logiciel |
| `test_retours`, `test_rubriques`, `test_dpae_depot` (en entier) | les retours d'organismes, le catalogue copié de CoolPaie, le gabarit DPAE réel |
| `test_allegements` ×2, `test_cadre` ×2 | des bulletins et une DSN réels, au centime |
| `test_formats`, `test_solde_taxe_apprentissage`, `test_etats` ×2, `test_exports` ×1, `test_permanents_chaine` (DPAE), `test_interface` (archives) | **verts à vide** sur la fictive : ils lisent les DSN déposées ou l'historique |
| `test_permanents_chaine` (refus faute d'adhésion), `test_permanents` (copie datée des articles), `test_compagnies_fictives` (rien n'est emprunté à la vraie fiche) | un fait de la compagnie réelle ou de son dossier `reference/` |

**B à reprendre — provisoirement en `prive/tests/`**. Ce qu'ils prouvent,
une fictive pourrait le prouver, mais leurs valeurs attendues décrivent
la compagnie du poste : ses salariés (« Ana », « Lenadre »), ses six paies de septembre,
ses deux adhésions, un livre de paie assez long pour se découper en pages.
Pour les réécrire, il faut des jeux fictifs qui aient ce volume. En
attendant, ils tournent sur le réel : aucune preuve n'est perdue.

- `test_interface` ×8 (noms parlés, aperçus, saisie par rubrique)
- `test_operations` ×6 (fiches archivées, documents après DSN et AER)
- `test_catalogue` ×2, `test_amorcage` ×2
- `test_exports` (le PDF découpé), `test_etats` (chaque état dans chaque format)
- `test_exercice` (six paies en septembre), `test_contrats_plurimensuels`,
  `test_dossier_salarie`, `test_dsn_sans_individu`
- `test_dpae_depot` : seuls ses tests sur le gabarit réel sont du C ; le
  reste du fichier est du B, déplacé avec eux parce qu'il plante dès l'import

### Le verrou (étape 5)

`tests/test_verrou_donnees_reelles.py` refuse un NIR qui ne porte pas la
marque d'invention (`999`, `123456` ou `000`), le SIREN de la compagnie du poste, et tout
chemin construit vers `prive/`. Il vérifie aussi que la racine de la suite
n'est pas `prive/`. `outils/verifier_avant_commit.py` n'est pas supprimé :
il sert encore au dépôt public (`AVANT-LE-PREMIER-COMMIT.md`). Il est porté
en Rust le 15/10/2026 (`paie verifier-avant-commit`), et le script retiré.

### Ce que la bascule ne couvre pas

- **Un fichier lancé seul** (`python3 tests/test_x.py`, la boucle du poste
  sans pytest) ne passe pas par `conftest.py` : il lit encore la compagnie
  du poste. Seul pytest garantit la fictive.
- Les noms de bulletins CoolPaie cités comme **sources** dans
  `test_non_regression` (`BSSIMIONI012021.pdf`…) restent : ce sont des
  références, pas des données de paie.

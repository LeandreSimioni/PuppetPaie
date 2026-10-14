# Porter le moteur en Rust — la carte

Document de travail. **Réécrit le 16/09/2026** après une nouvelle lecture
ligne à ligne de `moteur/calcul.py`, `cumul.py`, `plafond.py`, `catalogue.py`,
`contexte.py`, `emplois.py`, `util.py`, de `operations_paies.charger`, du
barème `config/baremes/2026.json` et des corpus disponibles. La première
version datait du 05/09 et décrivait un moteur qui n'existe plus.

Il ne remplace pas `prive/journal/ETAT.md`, qui reste le document de reprise.
Il répond à une seule question : **qu'est-ce qu'on tape, dans quel ordre, pour
que la phase 1 du portage — le calcul d'une paie — soit finie et prouvée.**

Le cap et les raisons du portage sont fixés ailleurs (binaire de 10 à 15 Mo,
sans runtime, janvier 2027). Ici on ne discute que du comment.

---

> ## Le critère, et la décision du 16/09/2026
>
> La carte du 13/09 posait un critère : ne pas commencer avant qu'un mois
> complet ait été produit, déposé et passé sans une modification de
> `calcul.py`, `cumul.py` ni `dsn.py`. **Ce critère n'est pas atteint** : les
> trois ont bougé le 15/09 (contrats sur plusieurs mois). L'éditeur a décidé le
> 16/09 de commencer quand même, la liste des chantiers Python étant vide ;
> le risque accepté est celui-ci : si la DSN de septembre (début octobre)
> révèle un défaut dans l'un des trois, on le corrigera deux fois.
>
> Ce qui limite ce risque, et qui est l'ordre de cette carte : d'abord ce
> qui ne bouge plus (l'arithmétique, les dates, le barème, le plafond), le
> calcul ensuite, `cumul` en dernier — c'est le plus mouvant.

> ## Décision du 17/09/2026 : le portage attend la clôture de janvier 2027
>
> Relecture d'Opus, et l'éditeur l'a suivie : **aucune DSN réelle n'est encore
> sortie du moteur** (jusqu'à décembre, CoolPaie dépose). Le portage doit
> reproduire un calcul prouvé en réel — la DSN de septembre déposée par nous
> en octobre, celles d'après, la clôture 2026 — et pas un calcul prouvé en
> test. D'ici là, chaque défaut trouvé en réel se corrige une fois, en
> Python. **On ne rouvre pas la question avant la clôture de janvier 2027**
> ; c'est écrit pour qu'on n'y revienne pas. Rien de la carte ne change :
> elle s'applique telle quelle le moment venu, après une relecture de ce qui
> a bougé entre-temps (le chantier régularisation touchera `dsn.py`, pas le
> calcul). Si l'éditeur veut anticiper, c'est possible — la carte est prête —
> mais ce sera une décision, pas une dérive.

> ## Note du 26/09/2026 : test = réel
>
> L'éditeur a posé la règle « test = réel » (`CLAUDE.md` règle 10) : un
> dépôt de test accepté vaut un dépôt réel, même moteur, même circuit. La
> **raison** donnée le 17/09 — reproduire un calcul « prouvé en réel, et
> pas en test » — ne tient donc plus. La **date** n'est pas changée par
> cette note : elle est à reprendre par l'éditeur. Piste relevée le même
> jour : un moteur Rust peut s'intégrer au Python comme module (PyO3,
> maturin), ce qui permet de porter module par module et de comparer les
> deux calculs dans le même processus ; ce n'est qu'une transition, la
> cible reste le binaire sans runtime.

> ## Décision du 30/09/2026 soir : la fabrique de PDF d'abord
>
> Léandre : « fais le faiseur de PDF en Rust, comme ça je me rends compte,
> en rajoutant les fonctionnalités ; on aura une brique ». La date du reste
> du portage n'est pas changée par là ; la section « La fabrique de PDF »
> ci-dessous est la mission de la prochaine séance.

> ## Le produit et l'atelier (01/10/2026)
>
> Le binaire Rust est **le produit** : lire les données, calculer, faire les
> PDF, déclarer, l'interface — rien d'autre. La veille, les relevés, les
> extracteurs, les tests et la comparaison au Python (`oracle`) sont
> **l'atelier** : ils ne vont pas dans le binaire. Voir `PLAN-DEPOT-PUBLIC.md`.
> Le crate : `logiciel/paie/`.

---

## La fabrique de PDF — mission de la prochaine séance

### Les mesures du 30/09/2026 (conteneur Linux, `--release`, LTO, `strip`)

Un petit programme par bibliothèque, chacun produisant un PDF relu par
PyMuPDF (accents et « € » corrects partout).

| bibliothèque | poids du programme | ce qu'elle fait |
|---|---|---|
| aucune (programme vide) | 309 Ko | — |
| écrite à la main | 321 Ko | du texte à des coordonnées, les 14 polices standard |
| **genpdfi 0.2.7** | **1,56 Mo** | paragraphes, retour à la ligne, pages, tableaux, gras |
| krilla 0.8.2 | 3,05 Mo | PDF propre, polices, PDF/A — sans mise en page |
| Typst 0.15 (typst-as-lib) | 34,3 Mo (26,4 en `opt-level="z"`) | tout ; embarque ICU, césure de toutes les langues, bibliographies, sans option pour les retirer |
| PyMuPDF (aujourd'hui) | 65 Mo installés, sans Python | — |

Le contrat d'essai de genpdfi (onze articles, un tableau) : quatre pages,
**6,4 Ko** avec l'Helvetica standard (métriques lues dans Liberation Sans),
**3,1 Mo** avec DejaVu embarquée entière — la réduction aux caractères
utilisés existe dans genpdfi (`subsetting.rs`, `FontData::new_with_subset`)
mais s'appelle à la main.

### Le choix : genpdfi, copié dans le dépôt

- genpdfi (`github.com/theiskaa/genpdfi`, fork maintenu de genpdf, MIT ou
  Apache-2.0) : 6 300 lignes, 376 Ko. **Copié** dans le dépôt comme un
  module à nous, avec son dossier `LICENSES/` et la liste de nos
  modifications (Apache l'exige). Ses dépendances (printpdf 0.7, lopdf,
  rusttype…) restent sur crates.io, figées par `Cargo.lock`. Tout copier
  (`cargo vendor`) pèserait 189 Mo, dont 105 de bibliothèques Windows :
  hors du dépôt, seulement pour compiler sans réseau.
- Le certificat Congés Spectacles (forme officielle, case par case) peut
  s'écrire à la main, sans bibliothèque.
- Risques : un seul auteur ; rusttype n'est plus maintenu ; l'Helvetica non
  embarquée exclut le PDF/A (à vérifier : rien ne l'exigerait pour un
  bulletin ou un contrat).

### Où on en est — 30/09/2026 soir (journal §170)

Fait : les étapes 1, 2, 3 et 9 ci-dessous (la 9 reste à voir tourner une
première fois), et les tests de l'étape 10 pour le contrat. Programme de
**2,8 Mo** (genpdfi, PNG, JPEG, SVG). Trois leçons de la comparaison avec
PyMuPDF, écrites dans `fabrique-pdf/FORMAT.md` : Story ignorait
`text-transform` et les largeurs de colonnes, il ajoutait 7,5 pt de marge
au `body`, et il fondait les marges voisines comme CSS — la fabrique fait
de même (sans quoi un contrat d'une page en prenait deux). La signature de
l'employeur est une image **flottante**. Décidé ensuite : le Rust lit le
`.modele` lui-même ; le chemin PyMuPDF se retire après vérification sur le
poste de l'éditeur. La suite (bulletin, avenant, certificats, Congés
Spectacles, tableaux) : `prive/journal/ETAT.md`, « Prochaine séance ».

### Ce qu'il faut faire, dans l'ordre

1. `logiciel/fabrique-pdf/` : un programme Rust qui lit la **description
   d'un document en JSON** (parties, tableaux, signatures, en-tête, pied) et
   écrit le PDF. Le Python l'appelle ; sans lui, il retombe sur PyMuPDF — le
   logiciel marche à chaque étape.
2. genpdfi copié, élagué (césure, images seulement si la signature en a
   besoin — à mesurer), licences en place.
3. **Le contrat CDDU d'aujourd'hui** rendu par la fabrique : même texte que
   le PDF de PyMuPDF (texte extrait comparé), signature de l'employeur
   déposée.
4. **Le contrat fait de parties** (`PLAN-CONTRATS.md` A) : obligatoires
   verrouillées avec leur source, modifiables, libres ; une partie
   obligatoire vide est un refus qui nomme la donnée.
5. **Libre, une page, deux pages** : produire, compter les pages, resserrer
   la police et les espacements, refuser sous un seuil de lisibilité. Pour
   les contrats seulement.
6. **Court ou complet** (contrat) ; le choix par compagnie, la case du
   salarié dans Avancé qui l'emporte.
7. **Le tableau des exports** (`tableaux.py`) : A4 paysage, en-tête des
   colonnes à chaque page, pied numéroté — les vingt-deux états d'un coup.
8. Les polices : Helvetica standard, ou une police embarquée réduite ; le
   dire avec les poids mesurés.
9. **GitHub Actions** fabrique le programme pour Windows, Mac et Linux (le
   poste de l'éditeur n'a pas d'éditeur de liens, §0 : il n'en aura plus
   besoin). Jamais essayé : à valider au premier essai.
10. Tests : `cargo test`, et un test Python qui compare les deux rendus.

---

## 0. L'état de la machine — vérifié le 16/09/2026

| | |
|---|---|
| `rustup`, `cargo`, `rustc` | installés — `rustc 1.98.1`, `cargo 1.98.1`, toolchain `stable-x86_64-pc-windows-msvc` |
| L'éditeur de liens | **toujours absent**. Pas de Visual Studio, pas de Build Tools |
| Le piège neuf | `where link.exe` rend `C:\Program Files\Git\usr\bin\link.exe` — le `link` de coreutils, qui crée des liens de fichiers. `cargo build` l'appelle, et échoue avec « extra operand ». Ce n'est pas le bon programme, et le message ne le dit pas |

Donc rien ne se compile aujourd'hui, et c'est le seul point qui exige du
réseau.

**Deux chemins, et le second est dix fois plus léger.**

1. La chaîne GNU, que rustup fournit **avec son propre éditeur de liens**
   (composant `rust-mingw`), quelques centaines de Mo :

   ```
   rustup toolchain install stable-x86_64-pc-windows-gnu
   rustup default stable-x86_64-pc-windows-gnu
   ```

   Suffit tant que **toutes** les dépendances sont du Rust pur — c'est le cas
   de la phase 1, et c'est de toute façon un objectif du projet.

2. La chaîne MSVC, plusieurs Go :

   ```
   winget install Microsoft.VisualStudio.2022.BuildTools --override "--quiet --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
   ```

   Préférable le jour d'une diffusion publique (binaire sans libc MinGW). Ce
   jour n'est pas aujourd'hui.

**La 1, puis la 2 seulement si le lien échoue.** Et pendant qu'il y a du
réseau, ce qu'on oublie toujours :

```
cargo fetch          # puis, une fois le Cargo.toml stabilisé :
cargo vendor
```

Après quoi `cargo build --offline` fonctionne sans réseau.

---

## 1. Le principe : le Python est l'oracle

Le Rust **lit les mêmes fichiers** : `config/*.json`, `config/baremes/*.json`,
`paies/AAAA-MM.json`, `historique/engagements.json`, et les trois JSON livrés
avec le code (`rubriques-base.json`, `emplois-base.json`,
`nomenclature-emplois-ft.json`). Aucune migration, aucun format nouveau. C'est
ce qui rend la preuve gratuite : dès que le calcul existe, le double run de
septembre à décembre devient un **triple run** — CoolPaie, Python, Rust.

| Niveau | Ce qu'il prouve |
|---|---|
| CoolPaie (`export/` : 75 DSN, 71 fichiers AEM, les bulletins PDF ; les deux bulletins de référence 31 et 32/2026) | la **spécification**. Cinq ans de déclarations acceptées. |
| Python (`moteur/`, ~400 tests) | l'**oracle**. Il reproduit CoolPaie au centime, c'est établi. |
| Rust | doit reproduire **le Python**, chiffre pour chiffre. |

Le Rust ne se compare **jamais** directement à CoolPaie en phase 1. Un écart
Rust/Python est un bug de portage ; un écart Python/CoolPaie serait une
découverte, et elle se fait côté Python.

**L'état de l'oracle au 16/09/2026**, à connaître pour ne pas prendre un
écart connu pour une régression : `python -m moteur calibrer` rend **12
fichiers OK, 4 écarts connus** (55 / 227 / 347 / 22 lignes : janvier et
février 2026 avant le bloc CASC, avril 2026 avec un bloc 82 de
régularisation, mai 2026) et **130 ignorés**, presque tous pour « pas de
barème pour 2021-2025 » — et ils le resteront, voir §12.

**Conséquence de méthode, nouvelle depuis le 05/09 : le portage ne corrige
rien.** La carte du 05/09 prévoyait deux « corrections de conception » à
faire en Rust (compteurs non mutables, bulletin qui stocke son résultat).
Elles sont **retirées** : le triple run exige que les deux moteurs lisent et
écrivent les mêmes fichiers, et `config/compteurs.json` en fait partie ; le
Python a d'ailleurs réglé le problème des numéros consommés autrement
(réservation en mémoire, écriture après production, `Contexte.valider_compteurs`).
Un Rust qui déduirait ses numéros autrement ne serait plus comparable.
Tout se porte à l'identique, y compris ce qui semble améliorable.

---

## 2. `util.py` — l'arithmétique, et les endroits où le Python passe par le flottant

Trois fonctions portent tout le reste :

```python
CENT = Decimal("0.01")
def eur(x):        return Decimal(str(x)).quantize(CENT, rounding=ROUND_HALF_UP)
def pct(base, tx): return eur(Decimal(str(base)) * Decimal(str(tx)) / Decimal(100))
```

**`eur` arrondit au demi supérieur, jamais au pair.** En Rust : `rust_decimal`,
`round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero)`.
`MidpointNearestEven` est le défaut de beaucoup de bibliothèques : c'est
l'erreur à ne pas faire.

**Un seul arrondi, dit.** Le coefficient de la réduction générale (§7.9)
se fige par `.quantize(Decimal("0.0001"), rounding=ROUND_HALF_UP)` — le
même demi supérieur que les centimes (explicite depuis le 17/09/2026 ;
avant, `quantize` sans argument prenait le pair du contexte Python, et un
test de grille tient que les deux n'ont jamais différé). En Rust :
`MidpointAwayFromZero` partout, 2 décimales pour les montants, 4 pour le
coefficient.

**Le piège du `str(x)`.** Python ne convertit jamais un flottant en `Decimal`
directement : il passe par sa représentation textuelle la plus courte.
`Decimal(str(1.115))` vaut exactement `1.115`. Recette Rust :

- lire le fichier en `serde_json::Value` avec `float_roundtrip` (le
  flottant lu est exactement celui de Python), puis `num::dec` : un entier
  tel quel, un flottant par sa représentation la plus courte
  (`format!("{}", x)`, puis `Decimal::from_str`), ce qui reproduit
  `Decimal(str(x))` ;
- `num::dec` accepte aussi une chaîne, pour qu'un barème reste éditable à
  la main.

**Les endroits où le Python calcule en flottant** — à porter tels quels, avec
`f64`, puis la représentation la plus courte (`format!("{}", x)` en Rust est
l'équivalent du `repr` de Python : même algorithme du plus court aller-retour ;
seule la forme diffère — `24` contre `24.0`, `0.00001` contre `1e-05` —,
jamais la valeur) :

| Où | Quoi |
|---|---|
| `Engagement.__post_init__` | `cachets = int(Σ float(quantite))` — une troncature, pas un arrondi ; `heures = float(Σ r.heures(float(q), heures_saisies))` |
| `lignes_brut` | `heures_annexes = Decimal(str(r.heures(float(q), h)))` — produit de flottants |
| `calculer`, CET des cadres | `taux_pat = round(taux_pat + cet["pat"], 3)` — addition et arrondi flottants |
| `reduction_generale` | `Decimal(str(float(crochet) ** float(puissance)))` — la puissance 1,75 |
| `Contexte.smic` | rend un `float`, que `calculer` passe par `Decimal(str(smic))` |
| `LigneCotisation.taux_pat / taux_sal` | des `float`, comparés tels quels dans les tests (`== 13.0`) |

Ce ne sont pas des bugs (les valeurs sont petites et à peu de décimales) ; ce
sont des **règles de fidélité**. Le harnais du §11 est là pour attraper le
cas où un `f64` et un `float` ne s'écriraient pas pareil — il n'y en a pas de
connu.

Le reste de `util.py` (`fmt_eur`, `fmt_taux`, `sans_accent`, `cle_recherche`,
`quand_lisible`) est du confort d'affichage et de saisie.

---

## 3. Les dates : les écrire nous-mêmes

Le moteur a besoin de peu : lire cinq formats (`AAAA-MM-JJ`, `JJ/MM/AAAA`,
`JJMMAAAA`, `AAAAMMJJ`, `JJ-MM-AAAA`) et la **date détaillée**
`{"jour": "...", "lieu": "..."}` (acceptée par `util.d`, c'est l'écriture
d'un jour joué qui dit où) ; comparer, ajouter des jours, compter des jours
entre deux dates, connaître le nombre de jours d'un mois, écrire `AAAA-MM`
et retrouver le premier et le dernier jour d'un mois, énumérer les mois d'un
intervalle (`Engagement.mois_du_contrat`), formater. Une centaine de lignes.

`chrono` et `time` apportent des fuseaux, une horloge, de la localisation, et
de la surface à auditer, pour un outil qui manipule des identifiants
net-entreprises. **Un `date.rs` maison**, jour civil pur, vérifié
exhaustivement contre Python sur 2000-2050.

La DPAE porte un horodatage et l'AEM une heure d'embauche : phase 2, une
horloge système suffit.

---

## 4. Le catalogue des rubriques — `catalogue.py`

Nouveau depuis le 05/09 : le calcul ne connaît plus « cachets et heures »,
il connaît des **lignes de paie**, chacune portant une rubrique du catalogue.

Trois fichiers, et la séparation est la raison d'être du module :

| Fichier | Rôle |
|---|---|
| `moteur/rubriques-base.json` | **le logiciel**, livré avec le code : 14 rubriques (`cachet`, `heure`, `journee`, `semaine`, `mois`, `repetition`, `enseignement`, `majoration`, `defraiement_forfait`, `defraiement_reel`, `avantage_nature`, `prime`, `absence`, `acompte`) |
| `config/rubriques-compagnie.json` | **les données** : rubriques propres à la compagnie, ajustements clé par clé des rubriques de base (absent aujourd'hui) |
| `config/rubriques-actives.json` | **les données** : celles qu'elle a cochées, et les usages — un ordre d'affichage, jamais un calcul |

Ce que le calcul lit d'une `Rubrique` : `unite` (parmi huit), `nature` (parmi
sept), `soumis`, `imposable`, `sens` (`"-"` pour absence et acompte),
`heures_forfait` (12 pour le cachet, 151,67 pour le mois), `compte_pour`
(`droits_et_montant` ou `ouverture_des_droits` — l'enseignement), et la
méthode `heures(quantite, heures_saisies)` : le forfait l'emporte, sinon les
heures saisies × quantité, sinon la quantité si l'unité est l'heure, sinon 0.

Deux refus à porter : un champ inconnu dans une rubrique est refusé (le
schéma est la structure), une `source` vide est refusée. `rubrique(code)`
lève sur un code inconnu.

**À ne pas porter :** `Contexte.rubriques()` et `Contexte.rubrique()` dans
`contexte.py` lisent un `config/rubriques.json` d'avant le catalogue ; plus
personne ne les appelle. Code mort côté Python, rien côté Rust.

---

## 5. L'entrée : `Engagement`

C'est la partie de `calcul.py` qui a le plus changé, et l'endroit où le
portage a le plus de refus à reproduire. Un engagement est **un salarié, un
spectacle, des lignes de paie datées**, dans un **mois**.

### 5.1 Ce qui vient du fichier

`paies/AAAA-MM.json` : `mois`, un `taux_cachet` par défaut, un `lieu` par
défaut, une `date_paiement` par défaut, et `engagements`, chacun avec
`salarie`, `spectacle` (ou celui du mois — absent partout : refus), `lignes`,
et en option `debut`, `fin`, `numero`, `jours`, `date_paiement`, `lieu`,
`metier`, `cadre`, et les six champs d'emploi écrits à la main.

Une ligne : `{rubrique, quantite, taux, heures?, dates?, planning?, lieu?}`.
Les **dates jouées** sont la réunion des `dates` des lignes
(`operations_paies.dates_engagement`), dédoublonnées dans l'ordre de
première apparition, chacune nue ou détaillée `{jour, lieu}` — le lieu de la
date, sinon celui de la ligne. Le **planning** est la réunion des `planning`
des lignes, même forme : des jours des mois suivants, cochés d'avance.

L'ancienne écriture (`dates`, `cachets`, `heures`, `taux_horaire` sur
l'engagement) est **refusée** en nommant l'outil de conversion. À porter :
c'est un refus, pas une compatibilité.

### 5.2 Ce qui se calcule à la construction — dans cet ordre

1. Les dates jouées : lues, triées ; les lieux des dates retirées sont
   oubliés.
2. `lignes` absent → refus. `lignes: []` est **un mois du contrat sans
   cachet** (le contrat court, la DSN doit le porter, rien d'autre n'est dû) :
   il exige `debut`, `fin` et `mois`, sinon refus.
3. **`cachets`** = troncature de la somme des quantités des lignes d'unité
   `cachet` ; **`heures`** = somme des `r.heures(quantite, heures)` des autres
   lignes, `None` si zéro. Les 12 h d'un cachet ne vont pas dans `heures`.
4. `taux_cachet` = le taux de la première ligne `cachet`, sinon la valeur
   passée.
5. `jours` = nombre de dates jouées, sauf s'il est saisi.
6. Le planning : lu, dédoublonné, trié, ses lieux à part.
7. `debut` = saisi, sinon première date jouée ; `fin` = saisie, sinon
   `max(dernière date jouée, dernier jour du planning)`.
8. Refus : sans ligne et contrat qui tient dans son mois (« un mois sans
   cachet n'existe que pour un contrat qui déborde ») ; fin avant début ;
   une date jouée hors du mois (quand le mois est dit) ; un jour de planning
   dans le mois ou après `fin` ; une date jouée hors du contrat.
9. `mois` = saisi, sinon celui de la première date.
10. `jours_contrat` = nombre de jours distincts de (dates jouées + planning),
    sauf s'il est saisi. C'est la **quotité** DSN (40.012/013), jamais zéro.

Chaque refus est un test unitaire Rust, avec un `enum` d'erreurs dont les
messages disent quoi faire — les messages Python sont la référence.

### 5.3 Ce que l'engagement sait dire

| Propriété | Définition |
|---|---|
| `sans_paie` | pas de ligne |
| `debut_mois`, `fin_mois` | les bornes du mois de paie |
| `premier_mois` | `debut >= debut_mois` — contrat à imprimer, DPAE |
| `dernier_mois` | `fin <= fin_mois` — bloc 62, certificat Congés Spectacles |
| `a_cheval` | ni l'un ni l'autre |
| `debut_periode`, `fin_periode` | le contrat **borné au mois** : `max(debut, debut_mois)`, `min(fin, fin_mois)`. C'est la période de paie, et c'est la date à laquelle le barème et le SMIC se lisent |
| `mois_du_contrat` | les `AAAA-MM` du contrat, du premier au dernier |
| `quotite` | `jours_contrat` |
| `lieu_de(jour)` | la date, sinon le planning, sinon l'engagement |
| `dates_du_contrat()`, `lieux_joues()`, `dates_par_lieu()` | pour le contrat imprimé ; phase 4, mais la logique est ici |

### 5.4 Le contrat qui déborde du mois — ce que `charger` fait en plus

Un contrat d'octobre à décembre est trois engagements, un par fichier de mois,
qui disent tous `du 12/10 au 20/12` et portent le **même numéro**. `charger`
(dans `operations_paies`, pas dans `calcul`) fait deux choses que le Rust
devra faire au même endroit :

- sur un mois qui n'est pas le premier : recopier `numero` et
  `jours_contrat` depuis le fichier du **premier mois** (même salarié, mêmes
  bornes) ; sans ce fichier ou sans numéro dedans, **refus** (« produire
  octobre d'abord ») ; sans le fichier, la quotité ne compte que ce qu'on voit
  et on le dit ;
- poser des avertissements (texte) sur la paie.

Pour l'oracle du §11, cette recopie est nécessaire (elle change la quotité
déclarée) ; le refus aussi.

### 5.5 L'emploi de l'engagement — `emplois.py`

Un engagement peut dire `metier: "MAR010"` (et `cadre: true|false`).
`emplois.resoudre` en tire six champs : `emploi` (libellé officiel),
`pcs_ese`, `code_emploi_ft`, `statut` (`artiste` si annexe 10, `technicien`
sinon), `statut_conventionnel`, `statut_categoriel_rc` (`01` cadre, `04`
sinon). Deux fichiers livrés avec le code : `emplois-base.json` (le catalogue
du spectacle vivant) et `nomenclature-emplois-ft.json` (1 236 codes). Un code
hors des deux → refus.

Ensuite `calcul.emploi_de(fiche, engagement)` : ce que l'engagement dit
l'emporte sur la fiche salarié, champ par champ. **C'est le seul endroit** où
la substitution se fait ; tout ce qui suit lit `paie.salarie`.

Ce que la phase 1 doit porter d'`emplois.py` : `metier`, `resoudre`,
`_statut_non_cadre`. Le reste (catalogue, familles, contrôle de fiche) est de
l'interface. Septembre 2026 porte déjà deux engagements avec `metier` : le
corpus les exerce.

---

## 6. Le barème — un fichier de données, en trois couches

`config/baremes/2026.json`. Le corps décrit **les artistes** (annexe 10) :
l'abattement de 30 % de l'arrêté du 24/01/1975 est cuit dans les taux. Les
autres statuts et catégories sont des sections qui **remplacent** des morceaux
du corps.

### 6.1 Le corps

| Clé | Rôle |
|---|---|
| `smic_horaire` | liste datée `{depuis, valeur}` : 12,02 puis 12,31 au 01/06/2026, résolue à `fin_periode`. **Toute valeur du barème peut s'écrire ainsi** depuis le 16/09 ; `Contexte.bareme(date)` les résout (§12) |
| `heures_par_cachet` | 7 (§7.5) |
| `abattement_csg`, `majoration_conges_115`, `majoration_conges_110` | 0,9825 / 1,115 / 1,10 |
| `rubriques_bulletin` | 15 lignes : `id`, `famille`, `libelle`, `base`, `taux_pat`, `taux_sal`, éventuels `composants`, `masquer_base_et_taux` |
| `statuts` | `["artiste"]` — les statuts que le corps calcule |
| `cotisations_dsn`, `taux_chomage_aem`, `cotisation_prevoyance_retraite_supp` | phase 2 |
| `par_statut`, `par_categorie`, `deduction_forfaitaire` | ci-dessous |

Les clés commençant par `_` sont des commentaires sourcés. **Elles doivent
survivre** : `serde` sans `deny_unknown_fields`, et un test qui vérifie que
réécrire le barème depuis le Rust ne perd aucune clé.

Une note à corriger dans le fichier lui-même : `_note_csg` décrit encore deux
arrondis (« arrondi(brut × 0,9825) + arrondi(brut × taux) ») alors que le code
n'en fait qu'un depuis le 13/09 (§7.4). Sur 260 les deux donnent 258,34 ; sur
250 (cadre) ils divergent. Le code fait foi ; la note est fausse.

### 6.2 `bareme_du_statut(bar, statut)` — trois issues

1. Statut vide ou dans `statuts` → le corps.
2. `par_statut[statut]` existe, `_verifie: true`, et aucune rubrique sans
   `taux_pat`, `taux_sal` ni `composants` → le corps avec `rubriques_bulletin`,
   `cotisations_dsn`, `taux_chomage_aem`, `reduction_generale` remplacés par
   ceux de la section (seulement les clés présentes).
3. Sinon **refus**, en nommant tout ce qui bloque (trous, `_mecanismes_absents`,
   `_infere`) : un taux à `null` est une question posée à un humain.

Aujourd'hui : `par_statut.technicien`, vérifié le 13/09 (taux pleins 13,00 /
5,25 / 1,50, Agirc-Arrco 5,23 + 4,79, RGDU).

### 6.3 `bareme_cadre(bar, fiche)` — une surcouche, pas un troisième statut

Si `statut_categoriel_rc == "01"` : `par_categorie.cadre` doit exister et
être `_verifie`, sinon refus. Puis :

- chaque rubrique du corps dont l'`id` est dans `rubriques` reçoit les clés
  de la section (`prevoyance` → base `t1_journaliere`, taux 1,69 / 0,12 ;
  `retraite_t1` → `t1_journaliere`, `composants_pat` [3,94, 1,29],
  `composants_sal` [3,93, 0,86], `cet_si_depasse` {0,21, 0,14} ; `ccn` →
  `t1_journaliere`) ;
- `rubriques_ajoutees` s'insèrent chacune **après** l'`id` qu'elle nomme
  (`retraite_t2` après `retraite_t1`, `apec_t1` après `chomage`, `apec_t2`
  après `apec_t1`), l'ordre du barème étant l'ordre du bulletin ;
- `plafond_journalier` (220) est posé ; `cotisation_prevoyance_retraite_supp`
  et les surcharges `cotisations_dsn` (clé `code81` ou `code81:affectation`)
  aussi.

La composition est `bareme_cadre(bareme_du_statut(bar, statut), fiche)`, dans
cet ordre, et elle est refaite par `dsn.cotisations` : c'est une fonction
pure du barème, à écrire une fois.

### 6.4 `reduction_generale` et `deduction_forfaitaire`

Dans la section technicien : `tmin` 0,02, `tdelta` 0,3781, `puissance` 1,75,
`plafond_smic` 3, `smic_horaire_reference` 12,02 (gelé toute l'année par
décret), `imputation` {chomage 4,00, agirc_arrco 6,01}, `statuts`
["technicien"]. Formule au §7.9.

Au corps : `deduction_forfaitaire.taux` {"20": 16, "25": 18},
`rubriques_abattues` (neuf ids), `_verifie: false` — le calcul suit les
textes, jamais confronté à un bulletin réel : il produit **avec
avertissement**, il ne refuse pas.

### 6.5 Les bases

Les `base` que les rubriques nomment, et où elles se calculent :

| Clé | Par rubrique de salaire | Globale au contrat |
|---|---|---|
| `brut`, `brut_x115`, `brut_x110`, `brut_dfs` | oui | — |
| `csg` | — | oui |
| `t1_journaliere`, `t2_journaliere`, `apec`, `cet` | — | oui (cadres) |

`GLOBALES = (csg, t1_journaliere, t2_journaliere, apec, cet)`. La distinction
est **la** règle du §7.6.

Trois règles héritées : aucun taux en dur dans le Rust ; pas de barème pour
une année → refus avec le message qui dit quoi faire ; SMIC et licence sont
des listes datées résolues à la date demandée.

---

## 7. `calcul.calculer` — l'algorithme, dans l'ordre exact

L'ordre des opérations **fait** le résultat. Le porter, c'est reproduire cette
séquence sans la simplifier.

### 7.1 Le contexte de la paie

```
sal  = emploi_de(ctx.salarie(eng.salarie), eng)      # §5.5
spec = ctx.spectacle(eng.spectacle)
bar  = ctx.bareme(eng.fin_periode)                   # l'année de la PÉRIODE PAYÉE
bar  = bareme_du_statut(bar, sal.statut)             # refus possible
bar  = bareme_cadre(bar, sal)                        # refus possible
```

Un contrat de décembre à février se paie en décembre au barème de décembre.

### 7.2 Les lignes de rémunération

Pour chaque ligne de l'engagement, un `LigneBrut` :

```
q        = Decimal(str(quantite)) ; taux = Decimal(str(taux))
signe    = -1 si r.sens == "-" sinon 1
montant  = signe × eur(taux × q)                     # arrondi LIGNE PAR LIGNE
heures_annexes = Decimal(str(r.heures(float(q), heures_saisies)))
soumis, imposable, code, libelle, unite
```

`rubriques = [montant des lignes soumises]` ; `brut = Σ rubriques`.
`LigneBrut.designation` écrit « 2 Cachets », « 1 Journée de travail »,
« 6 h de Masterclass » — c'est le moteur qui l'écrit, pas le bulletin, et le
pluriel porte sur le premier mot. À porter tel quel : le contrat signé
l'imprime.

### 7.3 La prévoyance dans l'assiette CSG

`prev` = la rubrique d'id `prevoyance` du barème composé ; `taux_prev` son
`taux_pat` (1,11 artiste, 1,69 cadre). Si sa base est `t1_journaliere` et que
le barème a un `plafond_journalier` : `plafond_prev = eur(220 × jours)`.

### 7.4 Les assiettes, par rubrique de salaire

Pour chaque montant `m` :

```
base_prev = m
si plafond_prev et brut > 0 : base_prev = min(brut, plafond_prev) × m / brut   # non arrondi
csg       = eur( m × 0,9825  +  base_prev × taux_prev / 100 )                  # UN SEUL arrondi
brut_x115 = eur( m × 1,115 )
brut_x110 = eur( m × 1,10 )
brut_dfs  = eur( m × (1 − taux_dfs/100) )        # seulement si la fiche exerce la DFS
```

`base_csg = Σ csg`. À connaître par cœur : 260 → 255,45 + 2,886 = 258,336 →
**258,34** ; cadre 250 avec prévoyance sur T1 220 → 245,625 + 3,718 = 249,343
→ **249,34** (CoolPaie 31/2026 ; deux arrondis donneraient 249,35).

La division `min(brut, T1) × m / brut` est le seul endroit du calcul où une
division non arrondie précède un arrondi au centime sans qu'on puisse prouver
qu'aucun demi-centime exact n'est possible (il faut un cadre à plusieurs
rubriques). `rust_decimal` divise à 28 chiffres comme Python ; la grille du
§11 doit contenir des cadres à deux et trois rubriques.

### 7.5 Le SMIC retenu pour la DSN — pas un plafond

```
heures  = min( Σ heures_annexes des lignes , heures_par_cachet × jours )
plafond = eur( smic(fin_periode) × heures )
```

`S21.G00.79.001 = '01'` : le *SMIC retenu pour la réduction générale*.
Garder deux noms distincts dès la première ligne, `smic_retenu_dsn` et
`plafond_urssaf` (§8). Ancrages : 1 cachet février 2026 → 7 h, 84,14 ; juillet
→ 86,17 ; 1 cachet + 40 h sur 11 jours (octobre 2025) → min(52, 77) = 52 h,
617,76.

### 7.6 Les bases globales

```
bases = { brut, csg: base_csg, brut_x115: Σ, brut_x110: Σ }
si plafond_journalier (cadre) :
    t1   = eur(220 × jours)
    bases.t1_journaliere = min(brut, t1)
    bases.t2_journaliere = max(0, min(brut, 8 × t1) − t1)
    bases.apec           = min(brut, 4 × t1)
    bases.cet            = brut si brut > t1 sinon 0
si dfs : bases.brut_dfs = Σ brut_dfs
```

### 7.7 La boucle des cotisations — la règle qui a coûté un centime

Pour chaque rubrique `r` du barème composé, dans l'ordre :

```
cle_base = r.base ; si dfs et cle_base == "brut" et r.id ∈ rubriques_abattues : cle_base = "brut_dfs"
base     = bases[cle_base]
assiettes = [base]                       si cle_base ∈ GLOBALES
          = [b[cle_base] pour b par rubrique]   sinon
taux_pat, taux_sal, comp_pat, comp_sal = r.taux_pat, r.taux_sal, r.composants_pat, r.composants_sal
si r.cet_si_depasse et bases.cet > 0 :                # la CET des cadres, fondue dans T1 et T2
    comp_pat = comp_pat ou [taux_pat] ; comp_pat[-1] += cet.pat
    comp_sal = comp_sal ou [taux_sal] ; comp_sal[-1] += cet.sal
    taux_pat = round(taux_pat + cet.pat, 3) ; taux_sal = round(taux_sal + cet.sal, 3)
pour chaque assiette a :
    si r.composants     : m_pat += Σ pct(a, t) pour t dans composants        # arrondis séparément
    sinon si comp_pat   : m_pat += Σ pct(a, t) pour t dans comp_pat
    sinon si taux_pat   : m_pat += pct(a, taux_pat)
    si comp_sal         : m_sal += Σ pct(a, t) pour t dans comp_sal
    sinon si taux_sal   : m_sal += pct(a, taux_sal)
ligne(r.id, famille, libelle, base, taux_pat, m_pat, taux_sal, m_sal, masquer_base_et_taux)
```

> Les cotisations sur le brut s'appliquent à **chaque rubrique de salaire**,
> chaque montant arrondi, puis sommés. Celles sur une base globale se
> calculent **une seule fois**.

Invisible sur un contrat pur cachets. Visible sur le bulletin 27/2026
(1 cachet à 130 + 12 h à 20) : 212,99 de charges patronales, non 213,00. Les
`composants` sont le même phénomène : `autres_3` 0,680 % = 0,590 + 0,090
arrondis séparément, 286,00 → 1,69 + 0,26 = 1,95, non 1,94. La CET d'un cadre
à 250 : T1 5,440 = 3,94 + (1,29 + 0,21) → deux parts, retraite d'un côté,
CEG + CET de l'autre.

Après la boucle : les lignes `retraite_t2` et `apec_t2` **à base nulle et
montants nuls sont retirées** (un cadre sous la T1 n'a ni T2 ni CET).

### 7.8 La DFS : un avertissement, pas un refus

Si la fiche exerce la DFS, une phrase d'avertissement est posée sur la paie
(« jamais confronté à un bulletin réel »). Le texte est une donnée de sortie :
le porter mot pour mot, il est comparé par l'oracle.

### 7.9 La réduction générale (techniciens)

`reduction_generale(bar, statut, brut, heures_annexes)` rend `None` si la
section n'existe pas, si le statut n'y est pas, si brut ≤ 0 ou heures ≤ 0.
Sinon :

```
smic_ref = eur( smic_horaire_reference × heures )          # 12,02 × heures
si brut ≥ 3 × smic_ref : coefficient = 0
sinon :
    crochet     = min( 1, (3 × smic_ref / brut − 1) / 2 )   # Decimal, 28 chiffres
    terme       = Decimal(str( float(crochet) ** 1.75 ))    # LE flottant
    coefficient = (tmin + tdelta × terme).quantize("0.0001")   # ROUND_HALF_UP (§2)
montant  = eur(brut × coefficient)
chomage  = eur( montant × 4,00 / 100 / (tmin + tdelta) )   ;  agirc = eur( montant × 6,01 / 100 / (tmin + tdelta) )
urssaf   = montant − chomage − agirc
```

Puis une ligne `reduction_generale` (famille « EXONÉRATIONS ET ALLÈGEMENTS DE
COTISATIONS », base brut, `montant_pat = −montant`, taux `None`, masquée), et
`total_pat −= montant`. Ancrages : 4 h à 14 € → smic_ref 48,08, coefficient
0,2691, montant 15,07 ; 7 h à 100 € → 84,14, 0,2550, 25,50 ; au SMIC
0,3981 ; à 3 SMIC 0.

### 7.10 La sortie

```
net            = brut − total_sal
net_imposable  = net + montant_sal(csg_crds_nd)
cout_employeur = brut + total_pat
net_social     = net                      # une propriété : le MNS, sans mutuelle
```

`Paie` porte : `engagement`, `salarie` (la fiche substituée), `spectacle`,
`brut`, `base_csg`, `plafond`, `smic`, `heures`, `lignes` (cotisations),
`remuneration` (les `LigneBrut`), `bases` (les assiettes du §7.6, que la DSN
déclare telles quelles), `total_pat`, `total_sal`, `net_a_payer` (=
`net_social`, une propriété), `net_imposable`, `cout_employeur`,
`numero_bulletin`, `reduction_generale`, `deduction_forfaitaire`,
`avertissements`.

`charger` ajoute ensuite les avertissements des **minima conventionnels**
(`minima.py` : lecture de `minima-conventionnels.json`, paliers par nombre de
cachets, IDCC 3090 ou 1285) et de la case cadre décochée. Ce sont des textes,
ils entrent dans l'oracle ; `minima.py` se porte en phase 1 pour cette seule
raison, en dernier, et il n'y a rien à prouver dedans que des chaînes.

### 7.11 Les valeurs déjà connues — les tests Rust à écrire avant le code

| Cas | Attendu |
|---|---|
| FAVART juin 2026, 2 cachets à 130 | brut 260,00 · CSG 258,34 · pat 149,66 · sal 58,68 · net 201,32 · imposable 208,81 · SMIC retenu 172,34 · coût 409,66 |
| Bulletin 27/2026, 1 cachet 130 + 12 h à 20, 4 jours | brut 370,00 · CSG 367,63 · pat 212,99 · sal 83,50 · net 286,50 · coût 582,99 |
| Ligne `autres_3` (composants) sur 286 | 1,95 — et non 1,94 |
| Lignes `ccn` / `csg_ded` sur 260 | 1,30 / 17,57 |
| SMIC retenu février / juillet 2026, 1 cachet | 84,14 / 86,17 |
| SMIC retenu octobre 2025, 1 cachet + 40 h, 11 jours | 52 h, 617,76 |
| **Bulletin 31/2026, artiste cadre, 1 cachet 250, 1 jour** | brut 250,00 · CSG 249,34 · pat 146,87 · sal 57,10 · net 192,90 · imposable 200,13 · coût 396,87 · pas de RGDU |
| Cadre à 320, 2 jours (T1 440 > brut) | ni `retraite_t2` ni `apec_t2` · `retraite_t1` 16,74 / 15,33 · `apec_t1` 0,12 / 0,08 · CSG 319,81 |
| **Bulletin 32/2026, technicien, 1 journée de 7 h à 100** | brut 100,00 · CSG 99,36 · sal 24,25 · net 75,75 · imposable 78,63 · RGDU smic_ref 84,14, coeff 0,2550, −25,50 · pat 40,70 · coût 140,70 |
| Technicien 4 h à 14 | maladie 13,0 → 7,28 · RGDU 0,2691, −15,07 |
| DFS catégorie 25 sur 130 | base maladie 106,60 · base chômage 130,00 · avertissement posé |
| Statut inconnu, cadre sans section, DFS de catégorie inconnue, métier inconnu | refus, avec le message |
| Plafond Urssaf, les 11 cas de `test_plafond_urssaf_six_bulletins_coolpaie` (table PSS 2021-2026 du test) | dont 624,00 · 2 808,00 · 1 222,00 · 1 291,94 · 360,00 |
| Plafond Urssaf, employeur B, 20-25/03/2023, 6 cachets sur 6 jours d'affilée | **709,55** — plus de cachet isolé, le prorata seul ; 5 cachets un jour sur deux → 1 620,00 |
| Août 2026 complet | 6 bulletins, brut du mois 1 780,00 |
| Septembre 2026 complet | 6 bulletins, dont deux avec `metier` |

Les valeurs des cadres et techniciens viennent de `tests/test_cadre.py` et
`tests/test_allegements.py`, qui construisent leurs fiches en bac à sable :
l'oracle doit pouvoir tourner sur un bac à sable (§11).

---

## 8. `plafond.py` — le « Plafond Urssaf » du bas de bulletin

**Remplacé le 10/10/2026 (vague 1l, `prive/journal/norme/corrections/PASSATION-V1l.md`)** : le plafond suit
l'arrêté du 24/01/1975, art. 3, al. 2 — un artiste engagé moins de cinq jours (bornes du contrat) a 12 × le PSS
horaire par journée de travail datée, sans maximum avec le prorata ; sinon, et pour un technicien, le prorata de
R. 242-2. Le plancher des cachets et `cachets_isoles` décrits ci-dessous n'existent plus. **Vague 1o** : hors de
l'arrêté, le plafond d'un intermittent est, pour chaque paie, le PSS mensuel proratisé sur les jours rémunérés de la
paie (BOSS, assiette générale, § 1090, tolérance appliquée) ; le permanent garde R. 242-2 sur sa période de paie.

Inchangé depuis le 05/09, et c'est la partie la plus sûre du portage.

```
plafond = eur( max( PSS mensuel × jours calendaires du contrat / jours du mois  (mois par mois, sommé),
                    PSS horaire × 12 × cachets ) )
```

**Depuis le 17/09/2026, le plancher ne vaut que pour des cachets isolés** :
`cachets_isoles(dates)` — aucune suite de cinq jours travaillés consécutifs
(`jours_continus_max`) ; sinon le prorata seul (règle Urssaf, bulletin de l'employeur B
20-25/03/2023 → 709,55). `pour_paie` passe `engagement.dates_travail`.

Bornes incluses (du 24 au 24 fait un jour). Les PSS mensuel et horaire sont
**dans le barème** (`plafond_securite_sociale`, depuis le 16/09/2026 — le
code n'en porte plus aucun) ; `plafond.py` reçoit un résolveur année →
(mensuel, horaire), `Contexte.plafond_securite_sociale` en production, une
table dans les tests pour 2021-2025 ; année inconnue → `PlafondInconnu` avec
le message BOSS. Le plancher ne compte **que les cachets**, délibérément
(aucun bulletin ne permet de l'étendre aux heures). Il se calcule sur
`engagement.debut`, `engagement.fin`, `engagement.cachets` — les bornes du
**contrat**, pas de la période : sur un contrat à cheval, c'est le prorata
de tout le contrat qui s'imprime.

**Le prorata est portable sans réserve**, y compris sommé sur plusieurs mois :
la somme de termes `PSS × j / d` avec `d ∈ {28, 29, 30, 31}` est une fraction
dont le dénominateur réduit divise `pgcd(lcm(28,29,30,31), 1000) = 20` quand
elle est décimale exacte — donc au plus deux décimales, jamais `X,XX5`. Aucun
arbitrage d'arrondi ne dépend du 28e chiffre.

Test : les 6 années × 12 mois × toutes les durées de contrat ≤ 62 jours,
comparées au Python. Quelques milliers de cas, une seconde.

---

## 9. `contexte.py` — les fiches, et la fiche telle qu'elle était

`Contexte(racine, date)` lit une fois `entreprise.json`, `salaries.json`,
`spectacles.json` (clés `_` ignorées) — **chacun passé par `telle_qu_au(fiche,
date)`** : une fiche a un `historique` de `{champ, valeur, jusqu_au, note}`,
et jusqu'à cette date incluse, le champ valait cela ; plusieurs fenêtres sur
un champ se lisent par `jusqu_au` croissant, la première qui couvre encore la
date gagne. `date` est **la date de la déclaration**, aujourd'hui par défaut ;
un rejeu passe la date du dépôt d'origine. Vingt lignes, et l'oracle doit
prendre `--date`.

Les fiches ont beaucoup de champs (35 pour un salarié). Ce que la **phase 1**
lit : `nom`, `prenom`, `prenom_affichage`, `nir` (pour le féminin du libellé
d'emploi, et pour `cumul`), `statut`, `statut_categoriel_rc`,
`deduction_forfaitaire`, `code_salarie`, `initiales_contrat`,
`date_naissance`, `anniversaire_droits`, `actif`, `code_emploi_ft` (minima) ;
côté spectacle `titre`, `alias`, `numero_objet`, `actif` ; côté entreprise
`idcc`, `annexe_convention`, `licence_spectacle` (liste datée ou chaîne). En
Rust : une `struct` typée pour ce que la phase lit, **et le reste conservé en
`serde_json::Value`** — une fiche réécrite ne doit rien perdre.

**La recherche approchante**, à porter à l'identique parce que les fichiers
de paie disent « Celi » et « Marin » : clé normalisée (`cle_recherche` :
sans accent, minuscules, alphanumérique) ; exact sur clé / nom / prénom /
« nom prénom » / « prénom nom » **parmi tous, archivés compris** ; puis, parmi
les **actifs** seulement (`actif != false`), préfixe de longueur ≥ 3, puis
distance de Levenshtein ≤ 3 sur prénom et nom ; puis, si une archive
ressemble, un refus qui la nomme ; sinon « inconnu ». Spectacle : exact sur
clé, titre, alias ou numéro d'objet ; puis inclusion dans un sens ou l'autre.

`smic(date)` et `licence(date)` : la dernière entrée dont `depuis ≤ date`.
Les compteurs (`prochain`, `prochain_continu`, `enregistrer_compteur`,
`valider_compteurs`) : réservés en mémoire, écrits après production — même
fichier `compteurs.json`, même comportement ; **`numeroter` se porte tel quel**
(§1), mais l'oracle tourne en `sans_numeros`.

---

## 10. `cumul.py` — ce qui s'accumule, et d'où ça vient

Réécrit depuis le 05/09 : **plus de lecture d'`export/AEM_EDI` pour les
droits.** Deux sources, et une règle pour les départager :

| Source | Ce que c'est |
|---|---|
| `historique/engagements.json` | les 137 contrats attestés avant ce logiciel (AEM de CoolPaie, importées une fois) : `{salarie, debut, fin, numero_objet, heures, cachets, jours, brut, heures_enseignement?}` |
| `paies/AAAA-MM.json` | les mois produits ici, chargés par `charger(ctx, f, sans_numeros=True)` : un engagement par paie non `sans_paie`, bornes = **`debut_periode` / `fin_periode`**, `heures` = round(engagement.heures), `cachets`, `jours`, `brut = paie.brut`, `heures_enseignement` = heures des lignes dont la rubrique dit `ouverture_des_droits`, plafonnées aux heures |

Un mois qui a son fichier de paie est lu là, et l'historique se tait pour ce
mois (`e.fin` au format `AAAA-MM` ∉ mois couverts). Un fichier qui ne se
charge pas est laissé de côté sans erreur. Le tout trié par `(debut, fin,
salarie)`.

Deux décomptes d'heures, et non un : `heures_equivalentes = heures + 12 ×
cachets` (le total déclaré) ; l'enseignement compte pour les 507 dans la
limite de 70 h (120 h à partir de 50 ans à la fin de la fenêtre, âge sans date
de naissance = plafond bas), mais pas pour le montant. `manque = max(0, 507 −
heures_ouverture)`, `cachets_manquants` = division entière par excès.

Fenêtres : année civile ; 12 mois = `fin − 365 jours` (jours pleins, pas
mois calendaires) ; anniversaire notifié par France Travail = fin de la
fenêtre, jamais son début. `position` retient les engagements dont
**`fin`** est dans la fenêtre. `tableau_cumul` : trois colonnes (période,
cumul civil, droits si `anniversaire_droits` est dans la fiche).

**Ce qui se porte en phase 1 : tout sauf `lire_aem`** (le dépouillement d'un
dossier d'AEM à largeur fixe, latin-1, découpe par ligne et jamais par pas de
1 000 — c'est l'import et la phase 2 ; il partage son code avec l'écriture
AEM). `cumul` dépend de `charger` : il vient donc **après** le calcul, et
comme il lit tous les mois du disque, l'oracle le compare séparément
(`oracle droits`).

---

## 11. Le harnais différentiel — comment on prouve

C'est la moitié du travail, et c'est ce qui rend le reste rapide.

### 11.1 Un format d'échange canonique

`sortie_json.brut` fait déjà l'essentiel côté Python : `Decimal` → chaîne,
dates → ISO, clés `_` retirées, dataclasses → objets. Il manque une règle
pour les flottants (`taux_pat: 13.0`) : **normaliser tout nombre en chaîne
décimale sans zéros inutiles** (`"13"`, `"5.23"`, `"0.036"`), montants à deux
décimales (`"149.66"`). Clés triées. Lignes dans l'ordre du barème.
`engagement` sérialisé avec ses dates ISO, ses lieux, son planning.

### 11.2 Une commande des deux côtés

```
python -m moteur oracle paie   <fichier.json> [--racine ...] [--date AAAA-MM-JJ]   > py.json
paie oracle paie   <fichier.json> [--racine ...] [--date ...]                     > rs.json
diff py.json rs.json
```

`oracle` calcule et sérialise : `charger(ctx, fichier, sans_numeros=True)`,
aucun PDF, aucun compteur, rien d'écrit. Sous-commandes : `paie` (un fichier
de mois), `bareme --annee --statut --cadre` (le barème composé, pour prouver
§6.2-6.3 avant le calcul), `plafond` (une grille), `droits <salarie>`
(§10). Côté Python c'est un module neuf d'une soixantaine de lignes,
`moteur/oracle.py` + une entrée CLI — **il ne touche ni `calcul`, ni `cumul`,
ni `dsn`**.

Les erreurs sont des réponses : `{"ok": false, "erreur": "..."}`, comparées
aussi — un refus est un résultat.

### 11.3 Quatre corpus, du plus faible au plus fort

1. **Les mois saisis** — `paies/2026-08.json` (6 bulletins, dont le contrat
   mixte), `2026-09.json` (6, dont deux avec `metier`), puis octobre,
   novembre, décembre au fil du double run. Petit, immédiat, il attrape 90 %
   des fautes.
2. **Les bacs à sable des tests** — `tests/test_cadre.py`,
   `test_allegements.py`, `test_contrats_plurimensuels.py`, `test_exercice.py`
   construisent des dossiers de données temporaires (cadre 31/2026,
   technicien 32/2026, DFS, contrats à cheval, mois sans cachet). Les figer
   dans `tests/oracle/<cas>/` (racine + fichier de mois + `attendu.json`) :
   c'est le corpus qui exerce les branches que la compagnie n'a jamais
   jouées.
3. **Une grille engendrée** — cachets 0 à 10 × taux {60, 100, 130, 250, 260,
   520} × heures 0 à 40 × taux horaires {14, 15, 20, 22,50} × jours 1 à 11 ×
   dates de part et d'autre du 01/06/2026 × statut {artiste, technicien} ×
   cadre {non, oui} × DFS {"", "20", "25"} × une, deux ou trois lignes de
   salaire, plus des contrats traversant deux mois et une année. Quelques
   dizaines de milliers de cas, sans aucune valeur métier ; c'est là qu'on
   attrape le demi-centime (§7.4) et une différence `f64`/`float` (§2).
4. **L'historique 2026** — les 12 DSN identiques et les 10 AEM de 2026
   d'`export/`, tels que `calibrer` les rejoue. Il exige les écrivains DSN et
   AEM : **phase 2**. Les années 2021-2025 n'en font pas partie (§12).

Si les tests du §7.11 passent et que les corpus 1 à 3 ne montrent aucun
écart, la phase 1 est finie. Pas avant, et rien de plus n'est nécessaire.

---

## 12. Les barèmes 2021-2025 — abandonnés, et pourquoi

`config/baremes/` ne contient que `2026.json`, et `calibrer` ignore 130
fichiers sur 146. La carte du 05/09 en faisait un prérequis du portage
(« `calibrer` rejouant les cinq ans »). **Abandonné le 16/09/2026**, sur une
question de l'éditeur (« on va jamais refaire le passé ») et un essai : la DSN
de mars 2025 rejouée avec un barème 2025 rend 155 écarts, dont une dizaine
de taux — le reste est la **norme** (P25 contre P26 : blocs 15 et 31 absents,
pas de 78.006, codes 102/907/072/079 absents, bloc 23 différent). Rejouer
2021-2025 octet pour octet, c'est faire émuler cinq normes à `dsn.py` pour
des déclarations passées et acceptées. Rien à y découvrir.

L'oracle du portage est donc **2026 et la suite** : les 12 DSN identiques,
les 4 écarts connus, les deux bulletins de référence, les mois du double
run. Les 137 contrats de l'historique servent à `cumul` (§10), pas au calcul.

Ce que l'essai a laissé d'utile : **une valeur du barème peut être datée**
(`Contexte.bareme(date)` résout les listes `{depuis, valeur}`), parce que
l'AGS a changé au 01/07/2024 et la contribution chômage au 01/06/2025 en
cours d'année. Le Rust doit le faire au même endroit — dans `contexte.rs`,
au chargement, jamais dans `calcul.rs`.

---

## 13. L'architecture du moteur Rust (décidée le 01/10/2026, PR #95, journal §171)

### 13.1 Le produit et l'atelier

- **Le produit**, c'est ce que les compagnies installent : un petit binaire
  Rust qui lit les données, calcule, fait les PDF, déclare, et porte
  l'interface. Rien d'autre n'y entre.
- **L'atelier**, c'est ce qui sert à le fabriquer : veille KALI, relevés des
  conventions, extracteurs du cahier technique, compagnies fictives, tests,
  et comparaison au Python (l'oracle). Il ne va jamais dans le binaire ni
  dans la mise à jour.
- **La frontière, ce sont les fichiers de données** (barèmes, minima,
  emplois, usages DSN). L'atelier les produit avec leur source et leur date
  (`sources.json`) ; le produit les lit sans savoir comment ils ont été
  relevés.
- Le dépôt public portera les deux, séparés en `produit/` et `atelier/`
  (`PLAN-DEPOT-PUBLIC.md`, « La forme du dépôt public »). **Le déménagement
  se fera à son ouverture, pas au milieu du portage.** Le Python reste à
  l'atelier comme référence tant que le Rust n'est pas prouvé, puis il se
  retire : deux moteurs, ce serait deux vérités.

### 13.2 Le crate

**Un seul crate**, `logiciel/paie/` : une bibliothèque et un binaire `paie`
(le CLI se parse à la main). Trois dépendances, et rien d'autre :

| Crate | Pourquoi | Écarté |
|---|---|---|
| `rust_decimal` (`std`, `maths`) | l'arithmétique décimale ; arrondi au demi supérieur (`MidpointAwayFromZero`) dans `num::eur` et `num::pct` | les flottants comme représentation |
| `serde_json` (`preserve_order`, `float_roundtrip`) | lire et réécrire les mêmes fichiers, dans le même ordre de clés, chaque flottant lu exactement comme Python | `serde` derive, `arbitrary_precision` |
| `fabrique-pdf` (`path = "../fabrique-pdf"`) | le bulletin part à la fabrique **dans le même processus** | un second programme appelé |
| — | le CLI | `clap` |
| — | les dates, écrites dans `date.rs` | `chrono`, `time` |

- **Les fichiers JSON se lisent en `serde_json::Value`**, comme les `dict`
  du Python, pas en structs typées : rien à migrer, aucune clé perdue, une
  clé inconnue survit. Une struct ne décrit jamais le contenu d'un
  fichier : elle sert là où le Python a une dataclass (`Rubrique`,
  `Nature`) ou un objet du code (`Contexte`, `Jour`).
- **Un nombre lu passe par sa représentation la plus courte** (`num::dec`),
  ce qui reproduit exactement `Decimal(str(x))` (§2). Les arrondis se font
  au demi supérieur (`eur`, `pct`).
- `fichiers.rs` lit les JSON livrés avec le code, en cache par chemin et
  signature (date, taille), comme `util.json_en_cache` ; `PUPPETPAIE_CODE`
  dit où est `moteur/`.

Les modules, faits et à faire :

```
paie/
  Cargo.toml        trois dépendances ; profil release ci-dessous
  src/
    lib.rs
    main.rs         `paie oracle paie|bareme|plafond|droits|pas`      §11  prouvé
    num.rs          dec, eur, pct                                     §2   prouvé
    date.rs         jour civil maison                                 §3   prouvé
    fichiers.rs     JSON livrés, en cache                                  fait
    contexte.rs     fiches, telle_qu_au, cle_recherche (approchante),
                    barème daté, SMIC, licence, PSS                   §9   prouvé
    catalogue.rs    Rubrique, les trois fichiers, heures()            §4   prouvé
    emplois.rs      metier, resoudre                                  §5.5 prouvé
    contrats.rs     Nature, les natures de contrat                    §5   prouvé
    plafond.rs      PSS, prorata, plancher                            §8   prouvé
    bareme.rs       la pile : statut, nature, santé, catégorie        §6   prouvé
    engagement.rs   Engagement et ses refus ; charger                 §5   prouvé
    calcul.rs       calculer, réduction générale, DFS                 §7   prouvé
    minima.rs       avertissements, sous_le_smic                           prouvé
    cumul.rs        tout cumul.py sauf lire_aem                       §10  prouvé
    coffre.rs       le coffre : AES-GCM, scrypt, clé du poste              prouvé
    pas.rs          lecture du magasin, grille par défaut, des_paies       prouvé
    bulletin.rs     la description du bulletin                             prouvé
    fabrique.rs     le PDF dans le processus                               prouvé
  tests/
    *.rs            un fichier par module, contre les attendus
    donnees/        les attendus en JSON, et relever*.py qui les tirent
                    du Python (atelier)
```

```toml
[profile.release]
opt-level = "z"
lto = "fat"
codegen-units = 1
panic = "abort"
strip = true
```

### 13.3 La preuve : l'oracle

`python -m moteur oracle paie|bareme|plafond` (`moteur/oracle.py`) écrit un
**JSON canonique** : `json.dumps(…, ensure_ascii=False, sort_keys=True,
indent=1)` suivi d'un saut de ligne ; nombres en chaînes normalisées
(`format(Decimal(str(x)).normalize(), "f")` : `"149.66"`, `"13"`) ; clés
commençant par `_` retirées, sauf `_cle` ; dataclasses en objets, ensembles
en listes triées. **`paie oracle …` doit écrire les mêmes octets** ; un
écart est un défaut du Rust tant que le Python n'est pas prouvé faux.

Les attendus des tests Rust (`paie/tests/donnees/*.json`) sont relevés dans
le Python par `paie/tests/donnees/relever*.py` — ils font partie de
l'atelier, et se relancent quand le Python change.

### 13.4 Les règles de travail

- Commentaires courts, sans historique ni dates.
- Pas de sur-ingénierie : le module fait ce que le Python fait, pas plus.
- **Mieux et plus léger, pas refaire le Python** (Léandre, 05/10/2026) : seules
  les valeurs doivent être identiques (fichiers, montants, ce qui part) ; une
  phrase d'erreur, une classe de refus, un détail d'affichage peuvent différer
  (écart admis et noté, jamais reproduit). Une bibliothèque éprouvée vaut mieux
  qu'une réécriture faite main (`regex`, §199).
- Un Sonnet à la fois (Léandre, 04/10/2026), dans l'arbre principal.
- GitHub Actions à la fin seulement : la PR reste en brouillon, on teste
  dans le conteneur (`cargo test` dans `logiciel/paie`).

---

## 14. La suite, dans cet ordre

**Déjà prouvé** : `num`, `date`, `contexte`, `catalogue`, `emplois`,
`contrats`, `plafond`, `bareme` (la pile entière, avec un port interne de
`prevoyance.py` et d'`etablissements.du_salarie`), `engagement` (dont
`charger`), `calcul` (`lignes_brut`, réduction générale, DFS, `calculer`),
`feries`, `permanents`, `conges` (`operations_permanents.lignes_du_mois`, le
compteur de congés, les suspensions), `solde` (le solde de tout compte :
congés, précarité, préavis, les quatre ruptures), `clauses`, `minima`,
`cumul`, `pas`, `oracle` (`bareme`, `plafond`, `paie`, `droits`, `pas`).
**`paie oracle paie --sans-minima` : les 96 mois des démos et les vrais mois
(`prive/paies`) identiques à l'octet**, plus 309 variantes de démos
(ruptures, préavis, maladie, prime d'ancienneté) ; binaire 1 165 Kio
(02/10/2026).

**À faire**, dans cet ordre :

1. ~~L'arithmétique à 28 chiffres significatifs~~ — **fait le 02/10/2026** :
   `d28.rs` (sorti de `clauses.rs`), et `num::mul`/`div`/`add`/`sub`, `pct`,
   `somme` passent par lui ; 51 opérations brutes remplacées (`calcul`,
   `bareme`, `plafond`, `engagement`, `permanents`). Aucun mois des démos ni
   des vrais mois n'a bougé (96/96, 3/3) : la correction n'est prouvée que par
   les tests de `num.rs` (`97/31 × 21,855` → 68,39, l'ancien code rendait
   68,38). Binaire 1 161 Kio.
2. ~~Minima~~ — **fait le 02/10/2026** (`minima.rs`) : `avertissements` et
   `sous_le_smic` dans `charger` ; `minima::sans_minima` reproduit le
   `mock.patch` de l'oracle Python. Laissés à l'atelier : `etat`,
   `non_compares` et le rapport CLI. `paie oracle paie` **avec minima** :
   99/99 mois identiques (96 des démos, 3 vrais) ; 17 582 cas balayés sur
   les 11 conventions (`tests/donnees/minima.json`). Les démos ne déclenchent
   aucun avertissement de minima : seules les variantes et `prive/` les
   exercent. Non éprouvé : la branche 3097 « titre IV » (aucun emploi du
   catalogue n'en porte).
3. ~~Cumul, PAS~~ — **faits le 02/10/2026** : `cumul.rs` (tout sauf
   `lire_aem` et `enseignement_par_contrat`, qui sont l'import AEM, phase 2) ;
   `pas.rs` (lecture du magasin, péremption, grille par défaut,
   `du_versement`, `des_paies`, `situation`, `diagnostic` ; ce qui écrit et
   l'import d'historique restent en phase 2). Deux oracles neufs des deux
   côtés : `oracle droits` (177/177 identiques, `comparer_droits.sh`) et
   `oracle pas` (294/294, `comparer_pas.sh`), sur les démos, `prive/` et des
   racines variantes (`tests/donnees/racines/`). Une différence connue :
   `engagements_des_paies` écarte un mois sur toute erreur de `charger`, le
   Python seulement sur `ErreurOperation`, `ValueError`, `KeyError`,
   `FileNotFoundError`.
   `contexte.rs` lit ses JSON une fois tant qu'ils ne changent pas : un mois
   des démos 3090 passait de 1,4 s à 0,1 s. 83 tests ; binaire 1 300 Kio.
   **Le calcul est porté.**
4. ~~La description du bulletin~~ — **faite le 02/10/2026** (`bulletin.rs`,
   `fabrique.rs`) : `oracle bulletin` 427/427 identiques (mois entier et
   chaque paie) ; le PDF par `fabrique_pdf::fabriquer` dans le processus,
   728/728 PDF identiques au programme. **Toutes les polices restent dans
   le binaire** (Léandre, 02/10 : elles servent la mise en page ; ~1,4 Mo
   sur ~5 Mo, le reste est le code de la fabrique). Le téléchargement à la
   demande reste une piste pour les écritures que Liberation n'a pas.
   Vitesse (un mois de la démo 3090) : `oracle paie` 0,5 s (Python 1,5 s),
   `oracle bulletin` 0,7 s (Python 3,8 s), grâce aux caches de `contexte`
   (JSON, barème fondu, barème résolu), à la pile du salarié gardée et à
   une dérivation par `charger`. Reste à faire : la pile de barème recopie
   le barème entier à chaque étape (79 % du temps restant).

> ## Décision du 02/10/2026 soir : brancher le Rust module par module
>
> Léandre : « on peut pas utiliser la brique calcul en Rust comme on utilise
> la brique pour créer des PDF ? […] à un moment on a arrêté d'utiliser
> PyMuPDF […] je voulais pareil avec le Python des calculs ». **Le Python
> appelle le Rust, comme il appelle `fabrique-pdf`, et chaque module Python
> remplacé se retire.** Plus d'attente de la phase 6 pour remplacer d'un
> coup ; plus de comparaison au Python une fois un module branché : ce sont
> les tests Python existants qui vérifient, Rust derrière. Ordre :
>
> 1. **Le bulletin** : `bulletin.ecrire` appelle `paie bulletin FICHIER
>    --racine R --sortie X.pdf` (à écrire : recalcul, description, PDF) ; la
>    partie description de `bulletin.py` se retire.
> 2. **Le calcul** : `charger` appelle `paie` et reçoit les paies en JSON ;
>    un adaptateur Python les remet en `Paie`/`Engagement` pour la DSN et
>    l'interface. Se retirent ensuite `calcul.py`, `minima.py`, le calcul
>    d'`operations_permanents.py`, `clauses.py`, `cumul.py`.
> 3. **Avant de retirer**, inventorier ce que le Rust ne fait pas encore :
>    la numérotation des bulletins (le Rust calcule `sans_numeros`), ce qui
>    écrit `pas.json`, `lire_aem`.
> 4. **La DSN** reste en Python (phase 2) et lit les paies calculées par le
>    Rust.
>
> **À optimiser avant de remplacer la brique de calcul** (Léandre, 02/10) :
> le Rust doit être nettement plus rapide que le Python, pas seulement à
> égalité. Profil (callgrind, un mois de la démo 3090) : **la pile de barème
> (`bareme_applique`) fait 79 % du temps restant**, parce que chaque étape
> (`bareme_du_statut`, `bareme_atmp`, … `bareme_vm`) recopie le barème
> entier (`bar.clone()`, `copie(bar)`), là où le Python fait une copie
> superficielle. À faire : passer le barème par valeur le long de la pile et
> le modifier en place, en gardant l'ordre des clés (`preserve_order`).
> Un essai d'un Sonnet le 02/10 s'est arrêté sans rapport ni preuve : il a
> été effacé. Puis reprofiler (`valgrind --tool=callgrind` sur un binaire
> avec `CARGO_PROFILE_RELEASE_DEBUG=1`, `STRIP=false`).

> ## Fait le 02/10/2026 nuit (journal §181)
>
> - **Un seul programme**, `paie` : la ligne de commande de la fabrique vit
>   dans sa bibliothèque (`fabrique_pdf::ligne_de_commande`), `paie pdf …`
>   la sert, `moteur/fabrique.py` appelle `paie`. `fabrique-pdf` n'est plus
>   livré ; le workflow « Moteur Rust » (`paie.yml`) livre `paie/bin/`.
> - **La pile de barème sans recopie** : le barème passe par valeur. Les
>   99 mois (paie, sans minima, bulletin) en 16 s au lieu de 48, sorties
>   identiques à l'octet.
> - **Le service** (`src/service.rs`), la demande en JSON sur l'entrée
>   standard : `paie calculer` (les paies d'une saisie, nombres à leur
>   échelle), `paie bareme-applique` (la pile sur le barème que la DSN
>   choisit), `paie bulletin` (saisies, jeux, numéros, rectificatifs ; écrit
>   le PDF). Côté Python, `moteur/paie_rust.py`.
> - **Branchés et prouvés** : `charger` (99 mois, objets identiques champ
>   par champ), la pile de la DSN (99 DSN identiques), le bulletin (391
>   descriptions identiques). Reste le retrait du Python (`ETAT.md`).

> ## Fait le 02/10/2026, tard (journal §183) : le calcul Python est retiré
>
> - **Quatre services Rust de plus** (`src/service.rs`, `moteur/paie_rust.py`) :
>   `paie droits` (la position des annexes 8 et 10, `ops.droits`), `paie
>   provision-conges` (compteur et indemnité d'un permanent, l'état de
>   provision), `paie preavis` (la durée légale et conventionnelle, le
>   signalement de fin de contrat), `paie lignes` (les lignes d'un permanent
>   sans calcul, pour la DPAE). Prouvés identiques à l'ancien Python : droits
>   sur les quatre démos et la compagnie, provision sur tous les mois des démos.
> - **Retirés** : `calcul.py` (ses objets vivent dans `engagement.py`),
>   `cumul.py`, `clauses.py`, `plafond.py`, `aem.py`, `verif.py` et
>   `calibrer`, `oracle.py`, `charger_python`, la description Python du
>   bulletin, le calcul d'`operations_permanents.py` et de `permanents.py`,
>   les comparaisons de `minima.py` (il garde `etat`), les `relever*.py` et
>   `comparer*.sh`, et les deux tests Rust qui lançaient le Python. Les
>   lectures du barème que la DSN fait encore (`casc_exemptee`,
>   `versement_mobilite`, `regime_local`, `couche_organisation`,
>   `au_dela_du_seuil`) sont dans `baremes.py`. Plus de 13 000 lignes de
>   Python en moins.
> - **Tests** : les tests du seul calcul Python sont partis avec lui (règle
>   de Léandre) ; suite Python 1 250 verts, `cargo test --release` 88 verts.
> - **Mac désactivé** dans « Moteur Rust » jusqu'à la démo de novembre.

> ## Fait le 02/10/2026 nuit (journal §185) : le montant Urssaf
>
> - **`dsn.montant_urssaf`** (l'aperçu) appelle `paie montant-urssaf`
>   (`dsn::montant_urssaf`) : 102 mois identiques au centime avant le retrait
>   du Python du bordereau dans `dsn.py`.
> - **`moteur/bordereau_urssaf.py` retiré** : `paie bordereau` sert le rejeu
>   des 75 DSN de CoolPaie (`prive/tests`), les cas sont dans
>   `paie/tests/bordereau.rs`.
> - **`versement_urssaf` supprimé** de `generer` et de `paie dsn` : le montant
>   Urssaf se calcule toujours, il ne se donne pas.
> - Reste de la phase 2 : le FCTU, l'amorçage, le signalement d'arrêt, puis
>   le retrait de `dsn.py`.

> ## Fait le 03/10/2026 (journal §186) : fin de la phase 2 pour la DSN
>
> - **Le FCTU, l'amorçage et le signalement d'arrêt** s'écrivent en Rust
>   (`fctu.rs`, `amorcage.rs`, `signalement.rs`, commandes `paie fctu`,
>   `paie amorcage`, `paie signalement`, `paie signalement-verifier`).
>   Prouvés contre l'ancien Python, refus compris : environ 9 000 FCTU et
>   amorçages et 36 000 signalements (démos, `prive/`, `prive-dev/`, appels
>   de la suite), tous identiques à l'octet. `fctu.verifier` et
>   `amorcage.verifier` restent en Python pour leurs appelants sans
>   génération (`produire_fctu`, `a_combler`).
> - **`dsn.py` ne garde que ses portes** : `generer`, `ecrire`,
>   `montant_urssaf` (175 lignes, il en faisait 1 148). Ses tests de
>   fonctions internes sont repris en tests Rust dans `dsn.rs`.
> - **Le jour du logiciel** : `paie_rust._appeler` passe toujours
>   `PUPPETPAIE_AUJOURDHUI` (le jour de Paris), et le moteur le lit là où il
>   prenait l'horloge en UTC (droits, solde, date de constitution).
> - Reste de la phase 2 : la DPAE et l'AER.

> ## Fait le 03/10/2026 : l'AER (cadre 5) en Rust
>
> - **`aer.rs` et `cachet.rs`** : la lecture des quantités imprimées, la
>   vérification contre les paies du mois (clé de rapprochement : les dates
>   de contrat), le cadre 5, la signature posée dans sa case, le cachet
>   dessiné en SVG depuis `entreprise.json` (ou le tampon fourni), et le
>   remplissage par la bibliothèque de la fabrique **dans le même
>   processus**. Commandes `paie aer`, `paie aer-verifier`, `paie
>   aer-quantites`, `paie aer-cachet` (JSON sur l'entrée standard ; le PDF
>   passe par des fichiers, `paie_rust.aer_*`).
> - **Les écarts** ne sont pas une erreur du moteur : `{"ok": true,
>   "ecarts": [...]}`, et le Python relève `aer.AttestationInexacte` avec la
>   même liste. Un refus de la fabrique (image illisible, champ en lecture
>   seule) porte la `classe` `FabriqueRefus` et redevient
>   `fabrique.FabriqueRefus`. Le jour de la signature vient du Python
>   (`date.today()` de Paris), jamais de l'horloge du moteur.
> - **Prouvé contre l'ancien Python** (32 attestations : 14 fictives, 14
>   retours 46 de `prive/` et `prive-dev/`, 4 AER déjà signées ; sept
>   compagnies et des fiches bricolées, 45 au total) : 2 846 signatures
>   (1 427 PDF rendus identiques à l'octet, 378 refus pour écarts, 494 refus
>   de la fabrique et 547 refus ordinaires de même phrase), 262
>   vérifications, 32 lectures, 405 cachets SVG identiques comme texte.
> - **`aer.py` : 262 → 113 lignes, `cachet.py` : 222 → 49** (`svg` seul, pour
>   la page de Configuration). Les tests de leurs internes sont dans
>   `paie/tests/aer.rs`.
> - Reste de la phase 2 : la DPAE.

> ## Fait le 03/10/2026 : la DPAE en Rust (journal §187) — fin de la phase 2
>
> - **`dpae.rs`** : l'enregistrement Format 120 (gabarit de la compagnie ou
>   livré, zones employeur lues dans la fiche et normalisées NFKD → ASCII,
>   code Urssaf vérifié contre la table, sexe par `ntt::sexe`, type de
>   contrat et terme du CDD par la nature), le flux latin-1, le `.zip` écrit
>   à la main, le récapitulatif, et les lecteurs (`relire`,
>   `meme_declaration`, `lire_certificat`, codes retour). Commandes `paie
>   dpae` (`enregistrements`, `contenu`, `ecrire`, `recapitulatif`) et `paie
>   dpae-lire`. Le Python envoie la fiche, les dates et la nature : la DPAE
>   n'a besoin ni du contexte ni du calcul. L'horodatage vient du Python.
> - **`nfkd_ascii.rs`** : la table NFKD → ASCII et `str.isdigit` de Python,
>   générée par `outils/generer_nfkd_ascii.py` (pas de crate de
>   normalisation). Seule dépendance neuve : `flate2`, déjà dans le lock par
>   la fabrique.
> - **Prouvé contre l'ancien Python**, à l'octet et à la phrase de refus :
>   8 880 cas (102 mois des démos, de `prive/` et `prive-dev/`), 10 701
>   variantes (dont 4 088 refus), 5 197 appels Unicode (6 écarts, des
>   lettres plus récentes que l'Unicode de Python 3.11, de toute façon
>   refusées en latin-1), 2 690 lectures sur 141 fichiers, 206 écritures
>   `.txt`/`.zip`, 545 appels des suites. Seul changement : un `KeyError` ou
>   `TypeError` d'avant devient `CalculRefuse` (une `ValueError`), même
>   phrase.
> - **`dpae.py` : 414 → 150 lignes** (les portes, et les tables d'adresses
>   que `tests/test_dpae_rust.py` compare au moteur).
> - **La phase 2 est finie** : la DSN, ses signalements, la DPAE et l'AER
>   s'écrivent en Rust. Restent le réseau (5), l'interface et la
>   persistance (6).

> ## Fait le 03/10/2026 (journal §188) : la lecture des retours — début de la phase 5
>
> - **`retours.rs`** et commande `paie retours` (une `operation` par demande :
>   `bilan`, `anomalies`, `anomalies_codees`, `fusionner`, `liste_retours`,
>   `liste_depots`, `taux_pas` et ses quatre stratégies, `releves`, `fiche`,
>   `aer` avec l'archive lue à la main, `rattacher`, `natures`, `libelle`,
>   `constantes`). Les octets partent en base64 (les appelants les ont en
>   mémoire, souvent une réponse HTTP). Côté Python, `paie_rust.retours`.
> - **Une dépendance de plus, `roxmltree` 0.20** (XML en lecture seule, Rust
>   pur, déjà dans le lock par la fabrique). Une couche `controle_expat`
>   refuse ce que roxmltree laisse passer et qu'expat refusait. Les tables
>   Python (chiffres Unicode de `\d`, cp437, cp1252, noms de codecs, noms XML
>   d'expat) sont générées par `outils/generer_tables_retours.py` →
>   `tables_retours.rs`.
> - **Prouvé contre l'ancien Python** : 967 retours de `prive/` et
>   `prive-dev/`, 128 fictifs des démos et leurs variantes (536 580 appels,
>   1 écart) ; 8 015 appels enregistrés des suites, CRM synthétiques et 1 100
>   archives AER (20 écarts) ; 129 352 appels par les portes ; 60 000
>   mutations XML (26 écarts), 4 070 encodages (0). **Écarts restants, tous
>   hors de ce que les organismes envoient** : un JSON de liste avec `NaN`,
>   `Infinity`, `1e400` ou `\ud800` isolé (Python accepte, serde refuse) ; un
>   deflate corrompu au milieu (`BadZipFile` au lieu de `zlib.error`) ; un
>   DOCTYPE à sous-ensemble interne mal formé (roxmltree accepte) ; les phrases
>   des `ParseError`/`JSONDecodeError` (même classe, texte du lecteur Rust) ;
>   un encodage déclaré rare (koi8-r, iso-8859-15…) que Python décodait.
> - **Les classes d'erreur d'avant sont gardées** : le moteur nomme la
>   `classe`, la porte relève `ET.ParseError`, `zipfile.BadZipFile`,
>   `LookupError`, `json.JSONDecodeError`…
> - **`retours.py` : 829 → 346 lignes** ; il garde `BilanDepot` et
>   `Attestation` (et leurs propriétés `reel`, `verdict_connu`, `accepte`)
>   et les constantes que les appelants lisent, comparées au moteur par
>   `tests/test_retours_rust.py`. Les tests de `test_retours_reel.py` sont
>   repris en Rust (`paie/tests/retours.rs`, NIR inventés).
> - Reste de la phase 5 : préparer les dépôts sans réseau (`dsn_api`,
>   `dpae_api`), puis le transport HTTP (`netentreprises.py`). **Le
>   transport se prouve par de vrais dépôts de test, depuis le conteneur** :
>   `NET_ENTREPRISES_MOTDEPASSE` y est posée et `CLAUDE.md` autorise un
>   agent à déposer en `--test` sans aval. Le test vaut le réel (Léandre,
>   03/10/2026), quelques dizaines de dépôts suffisent, un annule-et-remplace
>   compris (normale en test juste avant). Un petit faux serveur ne sert
>   qu'aux pannes que net-entreprises ne produit pas à la demande (délai,
>   erreur 500, réponse coupée, jeton expiré). Jamais d'indicateur `02` :
>   la préparation le garde.

> ## Fait le 03/10/2026 soir (journal §189) : fin de la phase 5
>
> - **`depot.rs`**, commande `paie depot` : la préparation des dépôts DSN et
>   DPAE, sans réseau (aperçu, bascule en test `01`, renumérotation,
>   `verifier`, DPAE). ~303 000 appels contre l'ancien Python, 0 écart.
> - **`transport.rs`**, commande `paie transport` : le client net-entreprises
>   et l'envoi des dépôts. **`ureq` 3**, rustls, certificats du système.
>   Faux serveur pour les pannes ; cinq vrais dépôts de test reçus, leurs
>   retours relevés. Écarts restants et points ouverts : journal §189.
> - **Décisions pour la suite** (§189) : un binaire plus gros s'il est plus
>   rapide (`opt-level = 3` permis) ; l'optimisation se fait après la
>   phase 6, mesurée et repassée à l'oracle ; la mise à jour sera
>   silencieuse (installation par utilisateur, `tauri-plugin-updater` en
>   mode `quiet`) ; un mode clé USB, `prive/` en clair et le coffre chiffré.

> ## Décidé le 03/10/2026 : la phase 6 (Léandre)
>
> - **Tout le Python entre la page et le moteur passe en Rust**, prouvé
>   contre le Python comme aux phases 1 à 5. Pas de relais Python
>   provisoire : le logiciel n'est pas ouvert avant la fin du portage.
> - **Un seul logiciel, deux entrées** : la ligne de commande (`paie …`) et
>   la fenêtre Tauri, mêmes règles d'autorisation (mot de passe retapé pour
>   un acte). Les `.cmd` restent des raccourcis. L'atelier dépose en test,
>   jamais en réel.
> - **Ouvrir, c'est lire** : « Sauvegarder la paie » calcule une fois et
>   écrit la paie avec son résultat ; l'ouverture du logiciel et d'une
>   compagnie relit ces fichiers sans calculer ; « Modifier » relance le
>   calcul. Un mois déclaré garde ses chiffres. Mesure du 03/10 (Linux,
>   `demo-3090-grande`) : l'ouverture d'une compagnie lance 77 calculs, dont
>   56 identiques ; `mois_a_ouvrir` calcule chaque mois non clos pour n'en
>   lire que les états (84 % du temps). Tauri seul : 5,6 s → ~4,5 s.
> - Ordre : le coffre, l'écriture des fichiers (verrou, journal), puis les
>   routes de lecture, d'écriture, les actes, la veille ; la fenêtre Tauri ;
>   la mise à jour silencieuse et le mode clé USB ; le retrait du Python.

> ## Décidé le 03/10/2026 : plus de Python neuf (Léandre)
>
> - **Toute logique nouvelle s'écrit en Rust.** Le Python ne reçoit qu'un
>   appel d'une ligne vers `paie …`, le temps que son module soit porté.
> - **Un défaut d'un module Python pas encore porté** se corrige au minimum
>   s'il gêne, et se note pour être fait au portage du module.
> - **Les tests nouveaux vont dans `paie/tests/`** ; en Python, seulement ce
>   qui tient les portes.

> ## Décidé le 03/10/2026 : la paie calculée, à côté de la saisie (Léandre, journal §191)
>
> - **Le résultat s'écrit dans un fichier à côté** : `paies/AAAA-MM.calcul.json`,
>   pas dans `paies/AAAA-MM.json`. La saisie reste ce que l'utilisateur a
>   tapé ; le calcul se jette sans risque.
> - **Il porte l'empreinte de ses entrées.** À l'ouverture, si elle ne
>   correspond plus et que le mois **n'est pas parti** (DSN non déposée), on
>   recalcule et on réécrit, **sans rien afficher**. Un mois parti garde ses
>   chiffres.
>   Il ne sert qu'à l'affichage : ce qui part (DSN, bulletins) se recalcule
>   toujours. **À la fermeture du mois**, le calcul frais se compare au fichier
>   (`paie calcul-comparer`) : mêmes entrées et autre résultat, c'est un
>   fichier retouché à la main ou une entrée qui échappe à l'empreinte, et cela
>   remonte en **incident** (`calcul-stocke-different`) sans bloquer la DSN.

> ## Fait le 03/10/2026 : le coffre en Rust — début de la phase 6
>
> - **`coffre.rs`**, commande `paie coffre` (une `operation` par demande, JSON
>   sur l'entrée standard ; les secrets n'y passent jamais en argument, et ne
>   sortent que dans la réponse de `lire` et `ouverture_gardee`). Même
>   `coffre.bin` (`VERSION = 2`), même clé du poste au même endroit, mêmes
>   refus et mêmes phrases (`ErreurCoffre`), même ordre des vérifications.
>   Crates : `aes-gcm` 0.10 et `scrypt` 0.11 de RustCrypto sans leurs options
>   par défaut, `getrandom` 0.3 pour l'aléa ; `scrypt`, `salsa20`, `pbkdf2`
>   et `sha2` compilés en `opt-level = 3` (le reste reste en `z` : 32 Mo
>   brassés à chaque essai). Les paramètres scrypt du fichier sont vérifiés
>   comme `hashlib.scrypt` le fait (OpenSSL), mémoire de 96 Mo comprise.
> - **Ce que le moteur ne sait pas : l'heure locale.** Le Python envoie
>   `maintenant` (`AAAA-MM-JJTHH:MM`) et, pour la garde, la `fin`.
> - **`coffre.py` : 920 → 427 lignes**, dont 170 de docstring conservées ; il
>   garde les noms, `SERVICES`, `Secret`, `ErreurCoffre` et des portes
>   (`paie_rust.coffre` relève `ErreurCoffre`, `OSError` avec son errno,
>   `binascii.Error`…). `tests/test_coffre_rust.py` compare ses tables au
>   moteur ; les tests des internes sont dans `paie/tests/coffre.rs`.
> - **Prouvé contre l'ancien Python** (`coffre.py` d'avant, deux mondes par cas :
>   tout ancien contre un mélange ancien/nouveau écrivant et lisant le même
>   coffre) : voir le journal pour les chiffres. Hors preuve, et assumé : un
>   coffre dont `fiches` n'est pas un objet (AttributeError au lieu de
>   ValueError), `~utilisateur` dans `PUPPETPAIE_CLE`, un `HOME` qui commence par
>   `~`, les points de code Unicode non attribués dans une phrase d'erreur,
>   la branche Windows de `dossier_cle` (non exécutable ici).
> - Reste de la phase 6 : l'interface (les routes, Tauri).

> ## Fait le 03/10/2026 : phase 6, l'écriture des fichiers
>
> - **`ecriture.rs` (`paie ecrire-json`)** : `util.ecrire_json` au même
>   octet (`json.dumps(ensure_ascii=False, indent=N) + "\n"`, LF). Trois
>   écarts de `serde_json` refaits : l'indentation (`[]`, `{}`), les flottants
>   (`repr` de Python : `1e+16`, `1e-05`, `100.0`) et les égalités exactes
>   (`…403.25` : Python écrit `.2`, ryu `.3`). Limite : un entier hors de
>   64 bits est lu comme flottant (pas d'`arbitrary_precision`), donc écrit
>   comme tel ; aucun fichier réel n'en porte. L'écriture reste directe, comme
>   en Python (ni fichier temporaire ni renommage).
> - **`verrou.rs` (`paie verrou`)** : `config/.compteurs.verrou`, même
>   fichier, même marque (`QUI pid N JJ/MM/AAAA HH:MM:SS`, que l'appelant
>   donne : le moteur ne lit pas le fuseau), verrou périmé repris, « le
>   sortant ne retire que le sien ». Non branché : le Python tient le verrou
>   le temps de son propre travail, deux processus lancés pour rien ; il
>   servira quand le travail sera en Rust (`Verrou`, rendu en sortant).
> - **`instance.rs`** : la fenêtre unique (`instance.py`), même port 7330
>   et mêmes mots, interopérable avec le Python (test). Non branché : c'est la
>   fenêtre du poste, il n'y a pas de commande.
> - **`journal.rs` (`paie journal noter|lire`)** : `journal.chemin`,
>   `empreinte`, `lire`, `noter`. `journal.noter` ne fait plus que
>   `paie_rust.journal_noter` ; `lire` reste en Python, où chaque état du mois
>   la rappelle une dizaine de fois (un processus à chaque appel), jusqu'à ce
>   que les états passent en Rust avec les routes. Ce qui reste de
>   `journal.py` : les états et les résumés (`embauches_declarees`,
>   `etat_dsn_du_mois`, `etat_des_embauches`, `contrats_demandes`,
>   `signatures_attendues`, `avenants`, `documents_envoyes`, `etat_du_mois`,
>   `resume`, `repetition`), les verdicts des retours (`verdict_du_retour`,
>   `rattacher_retours`, `_noter_rejet`, `dossiers_urssaf`) et `flux_de`.

> ## Fait le 03/10/2026 : phase 6, les états du journal en Rust
>
> - **`etats.rs` (`paie journal <operation>`)** : tout `journal.py` qui lit
>   `depots.json` (états du mois, embauches, contrats, signatures attendues,
>   avenants, `resume`, `repetition`, `deja_depose`, `flux_de`, verdicts et
>   `rattacher_retours`). `journal.py` ne garde que ses portes
>   (`paie_rust.journal_etat`), mêmes noms, signatures et exceptions. Les
>   dictionnaires à clé `(salarie, debut)` voyagent en triplets. Les incidents
>   de rejet sont rendus par le moteur et écrits par la porte
>   (`incidents.noter`). `tables_journal.rs` (généré) porte `str.isalnum()`.
> - Une porte = un processus. Mesuré le 03/10/2026 sur la démo 3090 grande
>   (dernier mois, `subprocess.run` compté) : `journal.etat_du_mois` 2,
>   `operations_retours.bilan_du_mois` 16 (calcul des paies compris, à chaque
>   appel). Acceptable jusqu'aux routes en Rust.
> - **Preuve** contre l'ancien `journal.py` (`dfb1fb6`) : 40 924 cas sur les six
>   journaux réels (`prive`, `prive-dev`, `demo/*`), tous les mois, avec
>   empreintes et trois rattachements de retours chacun ; 235 996 cas sur 240
>   journaux inventés ou mutés (champs absents, ordres, plusieurs dépôts le même
>   mois, rejets, test/réel) ; 0 écart sur les 29 fonctions. Le binaire release
>   fait 8,4 Mo.

> ## Fait le 04/10/2026 : phase 6, le mois en Rust, sans calcul de paie (lot A)
>
> - **`paie route`** (`routes.rs`, l'aiguillage ; `routes_mois.rs`) : `etat-par-paie`,
>   `etat-du-mois`, `embauches-a-declarer`, `mois-a-ouvrir`, et `bilan-du-mois`,
>   `fichiers-deposables` (appelées par les portes Python du même nom). Entrée
>   `{"route", "charge"}`, refus avec leur `classe` ; `paie_rust.route` les relève
>   (`SaisieIncomplete`, `ErreurOperation`, `KeyError`...). `web.py` : une ligne par route.
> - **Aucun calcul** : la saisie se charge sans calculer (engagements, contrat qui
>   déborde, numéros attribués en mémoire sans toucher aux compteurs). Seul le `brut`
>   d'`etat-par-paie` se relit dans `paies/AAAA-MM.calcul.json` ; le bilan n'y va jamais.
>   `calcul-du-mois` tient le mois pour parti si le journal porte une DSN réelle acceptée.
> - `mois_connus`/`mois_a_ouvrir` ne prennent que `paies/AAAA-MM.json` (le Python
>   prenait aussi `2026-09.calcul`). Heure locale : `paris.rs` (heure d'été UE).
>   `avenant.rs` : bouchon de `ecart_au_contrat_signe` (lot B).
> - **Preuve** contre l'ancien Python (`36d3702`, écart au contrat bouchonné) : 30 504
>   cas (six compagnies, tous les mois, cinq dates 2026-2027 ; variantes mutées :
>   validées, rouvertes, DSN réelles/refusées/en attente, fichiers .dsn/.txt et
>   tests au journal, signatures et avenants, numéros absents, saisies et journal
>   abîmés), 0 écart. `mois-a-ouvrir` sur la démo 3090 grande : 178 processus `paie`
>   et 4,1 s avant, 1 processus et 0,28 s après. Binaire release : 8 524 Ko.
>

> ## Fait le 04/10/2026 : phase 6, les routes de l'ouverture (`rappels`, `apercu`)
>
> - **`paie route`** (entrée `{"route", "charge"}`, réponse `{"ok", "resultat"}`
>   ou refus avec sa `classe`) ; `paie_rust.route(ctx, route, charge)` la relève
>   sous son nom d'avant (`SaisieIncomplete`, `KeyError`...). Code dans
>   `routes_ouverture.rs` (`traiter`), aiguillage minimal dans `routes.rs`.
> - **`rappels`** : aucun calcul. Le Python relisait la paie de chaque mois
>   (`paies_validees` passait par `charger`) pour n'en tirer que le nombre de
>   paies validées ; le moteur lit la saisie (engagements construits sans
>   chiffrer) et le journal (`etats.rs`). Seule différence : un mois dont le
>   calcul refuserait n'est plus compté « 0 validée ».
> - **`apercu`** : saisie reçue identique (nombre pour nombre) au fichier
>   enregistré -> relue dans `AAAA-MM.calcul.json` par `calcul_du_mois` (mois
>   « parti » = DSN réelle acceptée au journal) ; sinon calculée en mémoire,
>   rien d'écrit. Les numéros se réservent sans écrire les compteurs.
> - Python : `ops.apercu_mois` reste (la production du mois en a besoin avec
>   ses objets) ; `ops.rappels` et `rappels_dpae` sont des appels d'une ligne.

> ## Fait le 04/10/2026 : phase 6, l'accueil en Rust, la route `etat` (lot D)
>
> - **`paie route etat`** (`routes_accueil.rs`) rend tout le JSON de l'accueil : configuration
>   verifiee (anomalies, forme declaree, description), historique des mois (repli compris),
>   listes, rubriques, metiers, natures, referentiels. Route `configuration` : sa part, pour
>   les routes de `web.py` qui la rendent apres un enregistrement (`_configuration` = une ligne).
>   `champs.rs` porte `verifier_fiche`, `combler_affichage`, `apercu_derive`, `description`.
> - **`moteur/champs-base.json`** : la table des champs (ENTREPRISE, SALARIE, SPECTACLE) sortie de
>   `champs.py` pour que le Rust et le Python lisent la MEME source (sinon deux tables a tenir) ;
>   `champs.py` la charge, 700 lignes de moins, comportement verifie identique. Pas de logique neuve.
> - Restent en Python : `_fiche_verifiee` (autre appelant : enregistrement d'une fiche) et les
>   fonctions de `champs.py` (verifier-fiche, enregistrer_fiche...), jusqu'a leur portage.
>   `fenetre.py` `/etat` est le sondage de la fenetre (version, url, fermee) : sans rapport.
> - Ecarts assumes : `mois_connus` strict (`AAAA-MM`, comme le lot A) la ou le Python prenait
>   aussi `2026-08.calcul` ou `essai` comme mois ; phrase de l'analyseur JSON d'un fichier abime.
> - **Preuve** contre `36d3702` : 30 cas (six compagnies, cinq dates 2026-2027) + 244 cas sur 122
>   variantes mutees (fiches fautives par fuzz, archives, permanents et etablissements, exercice
>   clos, catalogue, metiers, demonstration, boite, fiches attendues, identite, fichiers abimes) :
>   0 ecart hors les deux ecarts assumes. `demo-3090-grande` : 0,29 s et 24 processus `paie`
>   avant, 0,07 s et 1 apres. Binaire release : 8 823 Ko.

> ## Fait le 04/10/2026 : phase 6, l'écart au contrat signé et l'ouverture mesurée (journal §193)
>
> - **`avenant.rs`** (`paie avenant-ecart`) : `ecart_au_contrat_signe` (ce que le contrat
>   parti ne dit plus de la paie), lu dans le PDF parti par la fabrique dans le même
>   processus ; tout ce qui rend "" se décide avant de relire la paie calculée. NFKC sans
>   crate (`nfkc.rs`, `tables_nfkc.rs` générées par `outils/generer_nfkc.py`, 1 174 444 cas
>   contre Python, 0 écart). Preuve : ~10 000 cas dont ~1 460 non vides, 0 écart.
> - **Après fusion des quatre lots** (A, B, C, D), preuves rejouées : 3 210 cas du mois avec
>   le vrai écart au contrat (130 non vides), 4 314 de `rappels`/`apercu`, 274 de l'accueil :
>   0 écart hors les écarts assumés (`mois_connus` strict, phrase d'un JSON abîmé).
>   `export_compta_a_jour` ignore les `.calcul.json` (sinon l'export paraîtrait périmé à chaque
>   ouverture). `paie_rust` ne connaît pas le coffre : `coffre.py` y inscrit `ErreurCoffre`.
> - **L'ouverture de `demo-3090-grande`** (page dans Chromium, premier tour de la veille
>   compris, médiane de 3, ce conteneur) : 13,6 s, 77 calculs et ~460 appels `paie` avant ;
>   3,1 s, 1 calcul (le premier `calcul.json`) et ~115 appels après. Les ~80 `paie journal`
>   restants viennent des routes pas encore portées.

> ## Fait le 04/10/2026 après-midi : phase 6, le lot E et F1 (journal §194)
>
> - **`paie route`**, quatre modules de plus : `routes_saisie.rs` (`saisie`, `echeancier`),
>   `routes_etats.rs` (`identite`, `exercices`, `coffre-etat`, `filet-etat`,
>   `importation-etat`, `maj-etat`, `signalements`, `pieces-parties`), `routes_listes.rs`
>   (`arrets`, `fins-de-contrat`, `regularisations`), `routes_contrats.rs`
>   (`contrats-en-cours`, `contrats-passes`, `emploi`, `rubriques-actives`, `metiers-catalogue`).
>   Aucun calcul de paie, aucun sous-processus. Restent en Python : `google-etat` (réseau) et
>   `diagnostic-etat` (système et version de Python).
> - **Preuve** contre `1c2bce2` : 2 285 + 1 022 + 2 930 + ~69 000 cas, 0 écart inexpliqué
>   (écarts assumés : phrase d'un JSON abîmé, classes parentes `ValueError`/`OSError`,
>   `mois_connus` strict). `web.py` 1 657 → 1 486 lignes. Binaire release : 9 158 Ko.
>
> ## Fait le 04/10/2026 soir : phase 6, les lots F2, F3, F4 en partie (journal §195)
>
> - **`paie route`**, trois modules de plus : `routes_apercus.rs` (`documents-liste`,
>   `bilan-repetition`, `depot-apercu` **pour la DPAE**), `routes_dossiers.rs`
>   (`arret-contrats`, `dossier-formes`, `exports-periodes` ; `exports_tables.json` généré
>   depuis le Python), `routes_fiches.rs` (`normaliser`, `taux-atmp` en lecture,
>   `verifier-fiche`).
> - **Preuve** contre l'ancien Python : 1 713 + 1 426 + 1 494 ; 1 830 + 586 + 983 ;
>   2 743 + 836 + 8 079 cas. Écarts tous expliqués (admis, du Contexte Rust plus tolérant,
>   ou voulus : `xlsx` et `pdf` dits par `web.py`).
> - **Restent en Python** : `bilan-repetition` quand le compte
>   rendu est « attendu » (il relève et écrit), la pose de `taux-atmp`, `revue-spectacles`,
>   `dossier-salarie`, `regularisation-mois` — **à porter, aucun repli Python** (Léandre,
>   04/10/2026).
> - **`changements-declares`, en Rust** (`routes_fiches.rs`, journal §196) : la dérivation
>   forcée, `garder_notes_internes`, `usages_declares` → `trouver_export`, `combler_affichage`.
>   Preuve : 4 437 cas, 0 écart. `enregistrer_fiche` appelle encore en Python
>   `changements_declares` et `garder_notes_internes` (lot G).
> - **`depot-apercu` pour la DSN, en Rust** (`apercu_dsn.rs`, branché par `routes_apercus.rs`) : la
>   lecture du fichier (`depot::lire`), `dsn_recap.lire`, le récapitulatif comparé aux contrats saisis
>   (`saisie_aux_numeros_declares`, dont la lecture de `Bulletins_<Mois>.pdf` par la fabrique), l'état de
>   l'annule-et-remplace avec un `nic` (`Moteur::etat_remplacement`). Preuve : 334 cas contre l'ancien
>   Python, 4 écarts, tous `NaN`/`Infinity` dans un montant (voulus : `Decimal` les accepte, `rust_decimal`
>   non). Retirés du Python : `apercu_depot_dsn`, `recapitulatif_dsn`, `_recapitulatif_sur`,
>   `_remplacement_du_fichier`, `dsn_recap.py`. Gardés (servent ailleurs) : `saisie_aux_numeros_declares`,
>   `numeroter_depuis_archive`, `bulletin.numeros_archives`, `operations_remplacement.etat`. Seule
>   écriture : le cache `paies/AAAA-MM.calcul.json` du calcul du mois, comme avant.
> - **Orphelins Python**, vérifiés le 04/10/2026 : `_etat_logiciel` et
>   `documents_du_mois` retirés (aucun appelant) ; `_etat_concerne` et `suggestions_fiche`
>   gardés, sans appelant en production mais oracles de tests (`test_exports.py`,
>   `test_questionnaire.py`, `test_non_regression.py`), à retirer avec ces tests au lot J.
> - cargo 463 verts, suite 1 280, `prive/tests` 147. Binaire release : 9 293 560 octets.

> ## Fait le 04/10/2026 soir : les cinq aperçus de F2 (journal §197)
>
> - **`paie route`** : `routes_documents.rs` (`signature-apercu`, `avenant-apercu`,
>   `envoi-apercu` avec l'écriture `ranger_aer`), `routes_regularisation.rs`
>   (`regularisation-apercu`), `routes_importation.rs` et `importation.rs`
>   (`importation-apercu`, la lecture des exports DSN et AEM). Branchées dans `web.py`, sans repli.
> - **Preuve** : 975 + 2 394 + 975 (140 AER fictives, arbres de fichiers identiques) + 6 941
>   + 13 200 cas, 0 écart de valeur ; écarts admis sur le nom de classe des refus. Retirée :
>   `apercu_envoi`. cargo 480, suite 1 280, `prive/tests` 147.

> ## Fait le 04/10/2026 nuit : routes écrites, `google-etat`, `dossier-salarie` (journal §198)
>
> - **`paie route`** : `routes_ecrites.rs` (`revue-spectacles`, pose de `taux-atmp`,
>   `regularisation-mois`), `routes_google.rs` (`google-etat` ; l'application dans
>   `moteur/google-application.json`), `routes_dossier_salarie.rs` avec `contrat.rs`,
>   `conges_spectacles.rs`, `fiche_recapitulative.rs` (`dossier-salarie` ; `dossier-envoyer`
>   reste au lot H). Branchées dans `web.py`, sans repli.
> - **Preuve** : 769 + 360 (routes écrites), 37 dont 3 appels réels à Google, 358 (dossier, sans
>   `demo-3090-grande` en entier ni `demo-1285-grande`) ; 0 écart de valeur, écarts admis sur la
>   phrase d'un JSON abîmé et des classes de refus. Retirés : `revue_spectacles`, `poser_taux_atmp`.
> - **Au lot « Incidents et canal éditeur »** : `diagnostic-etat` et `bilan-repetition` attendu.

> ## Décidé le 04/10/2026 : le lot « Incidents et canal éditeur », avant la fenêtre Tauri (Léandre)
>
> - **Absent du plan jusqu'ici, il disparaîtrait avec le Python au lot J.** La version de
>   novembre en dépend (les essais remontent par là) : il se porte **avant le lot J**.
> - **Ce qu'il comprend** :
>   - le journal des incidents : `incidents.py` (847 lignes), `commandes_incidents.py`, dont
>     `depuis_exception` (les erreurs des routes, `web.py`) et `depuis_navigateur` (la route
>     `incident-navigateur`, celle que le lot J fait passer par l'appel Tauri, `interface.js:140`) ;
>   - le canal vers l'éditeur : `editeur.py` ;
>   - « Un problème, une idée ? » : `signalement-apercu`, `signalement-envoyer`,
>     `signalements-lus`, `produire_signalement`, la capture d'écran (`operations_filet.py`) ;
>   - le filet DPAE côté poste : `filet.py`, `operations_filet.py`, `commandes_filet.py`
>     (`filet-verifier`, `filet-activer`, `filet-desactiver`, `preparer_filet_en_fond`).
>     Déjà en Rust, en lecture seule : `filet-etat` et `signalements` (`routes_etats.rs`).
> - **Ne se porte pas** : le serveur (dans le dépôt du site de l'éditeur, sous `infra/filet/`, depuis le
>   13/10/2026), qui ouvre les tickets GitHub. Il reste tel quel ; le poste lui parle comme aujourd'hui.
> - Mêmes règles que les autres lots : prouvé contre le Python, aucun repli. Ce qui engage
>   (désactiver le filet : mot de passe du logiciel) est décidé par le moteur, jamais par la page.

> ## Fait le 05/10/2026 : le lot « Incidents et canal éditeur » (journal §199)
>
> - **`paie route`** : `incidents.rs`, `editeur.rs`, `problemes.rs`, `filet.rs`,
>   `routes_incidents.rs` (`incident-navigateur`, `diagnostic-*`, `signalement-*`,
>   `signalements-lus`, `filet-verifier|activer|desactiver`, et des routes internes pour le
>   Python restant). Branchées dans `web.py`, sans repli. Les motifs passent par `regex`.
> - **Preuve** : environ 10 000 cas, 0 écart de valeur ; trois défauts du Rust corrigés.
> - **Retirés** : `editeur.py`, `filet.py`, l'essentiel d'`incidents.py` et
>   d'`operations_filet.py` ; puis 25 fonctions mortes (analyse en cascade).
> - **Restent** : le cœur d'`incidents.py` (`noter`, `depuis_exception`, `lire`), appelé par du
>   Python pas encore porté ; `_tester` du filet (il dépose une DPAE de test) au lot H ;
>   `bilan-repetition` « attendu » (la relève des retours), proposé au lot H.
> - cargo 507 ; binaire 10,3 Mo.

> ## Fait le 05/10/2026 : le lot G1, les écritures (journal §200)
>
> - **Le lot G se découpe par thèmes** (Léandre) : G1 (fiches, identité, saisie, métiers), G2
>   (régularisations, arrêts, `echeance-faite`, `couts`), G3 (coffre, mot de passe, `cloture`,
>   `export-compta`, `importer`, `maj-malgre-calcul`). Ce qui touche le réseau part au lot H
>   (`coffre-verifier`, `fiche-demander`, `fiches-relever`, `google-autoriser`), avec
>   `bilan-repetition` « attendu » (décidé par Léandre le 05/10).
> - **`paie route`** : `routes_ecritures.rs` (`activer-fiche`, `enregistrer-fiche`,
>   `fiche-oublier-demande`, `poser-identite`, `retirer-identite`, `enregistrer-saisie`, `ajuster`,
>   `metier-retenu`, `metiers-ajoutes`). Branchées dans `web.py`, sans repli ; `FicheRefusee`
>   remonte avec ses anomalies.
> - **Preuve** : environ 8 200 cas sur les quatre démos, fichiers écrits comparés octet pour
>   octet, 0 écart de valeur. Deux défauts du Rust corrigés (un mois impossible écrit sous `forcer` ;
>   une fusion de saisie sur un ancien fichier qui ne se calcule plus). Un historique de fiche qui
>   n'est pas une liste se refuse (Léandre). Écarts admis : classes de refus, une date de dix chiffres
>   que Python lisait de travers.
> - **Retirés** : `activer_fiche`, `oublier_demande`, l'identité, `retenir_metier`, l'ajustement,
>   `apercu_derive`, `_fiche_verifiee`. **Gardés** : `enregistrer_fiche` et `changements_declares`
>   (importation, questionnaire), `enregistrer_saisie` (`produire`), les métiers de `cli.py`.
> - cargo 525 ; binaire 10,9 Mo.

> ## Fait le 05/10/2026 : les lots G2 et G3, les écritures (journal §201)
>
> - **G2** (`routes_ecritures_g2.rs`) : `regularisation-ecrire`, `regularisation-annuler`,
>   `arret-enregistrer`, `arret-reprise`, `arret-oublier`, `echeance-faite`, `couts`. 513 cas, 0 écart
>   de valeur.
> - **G3** (`routes_ecritures_g3.rs`, `exercice_ecrit.rs`, `importation_ecrite.rs`) : `coffre-poser`,
>   `coffre-compte-dpae`, `coffre-oublier`, `mot-de-passe-changer`, `mot-de-passe-retirer`,
>   `google-oublier`, `maj-malgre-calcul`, `cloture` (avec `rouvrir`), `export-compta`, `importer`.
>   698 cas ; un écart de valeur corrigé (un historique importé qui n'est pas un objet se refuse avant
>   toute écriture). Le zip de clôture se prouve par son contenu, pas par ses octets.
> - **G4, à faire** : `valider-paie`, `contrat-recu`, `journal-reprise`, `ecrire-documents`, `exporter`,
>   `retenir-logiciel`, `casc-regularise`, `demonstration-courriels` (rangées dans aucun lot jusqu'ici).
> - **La ligne de commande Python part au lot J** (Léandre) avec ce qu'elle garde en vie.
> - **Remarqué** : l'export compta n'écrit pas le prélèvement à la source (recopié du Python, à corriger
>   après le portage).
> - cargo 549 ; binaire 11,0 Mo.

> ## Fait le 05/10/2026 : le lot G4 (journal §202, §203)
>
> - **Routes** (`routes_ecritures_g4.rs` et des modules neufs) : `valider-paie`, `contrat-recu`,
>   `retenir-logiciel`, `casc-regularise` (`casc_regularisation.rs`, `csv_py.rs`), `demonstration-courriels`
>   (`simulation_courriels.rs`, dépendance `mailparse`), `journal-reprise` (`reprise_journal.rs`),
>   `ecrire-documents` (`ecrire_documents.rs`), `exporter` (`tableaux_export.rs`, `etats_paie.rs`,
>   `etats_organismes.rs`, `etats_personnel.rs`, `etats_compta.rs` ; 26 états, xlsx par `rust_xlsxwriter`).
> - **Le classeur CASC-SVP est recréé** avec `rust_xlsxwriter` (dépendance), sans le modèle `.xlsx` du
>   Python ; mêmes valeurs, mise en forme simplifiée (Léandre).
> - ~4 000 cas de preuve ; cinq écarts de valeur corrigés (dont l'empreinte de `calcul.json` avec un
>   catalogue en cache, qui touchait aussi G3). Branché dans `web.py` sans repli.
> - Python gardé pour ses appelants (ligne de commande, tests) : liste dans `prive/journal/g4/PASSATION.md`.
> - cargo 556 ; pytest 1 211 ; binaire 12,3 Mo.

> ## Décidé le 06/10/2026 : le lot H découpé par thèmes (Léandre)
>
> - **H1, produire sans rien envoyer** : `dpae`, `arret-signaler`, `fin-contrat-signaler`,
>   `fin-contrat-documents`. Les formats sont déjà en Rust (`dpae.rs`, `signalement.rs`, `fctu.rs`) ;
>   reste l'enchaînement. Aucun réseau.
> - **H2, déposer et suivre** : `produire` (il lance le filet, qui dépose : écrit et prouvé ici, pas
>   en H1), `deposer`, `suivre-depot`, `bilan-repetition` « attendu », le filet (`preparer`, `_tester`).
>   Preuve par de vrais dépôts de test depuis le conteneur, comme le transport (§189).
> - **H3, la messagerie et les envois** : le client SMTP/IMAP (`courriel.py` n'a pas d'équivalent
>   Rust), puis `envoyer-documents`, `demander-signature`, `avenant(s)-envoyer`, `dossier-envoyer`,
>   `casc-envoyer`. `lettre` pour SMTP ; IMAP à trancher au début de H3.
> - **H4, les comptes et les fiches** : `coffre-verifier`, `fiche-demander`, `fiches-relever`,
>   `google-autoriser` (ou au lot J).

> ## Fait le 06/10/2026 : le lot H1, produire sans rien envoyer
>
> - **Routes** (`routes_actes_h1.rs`) : `dpae`, `arret-signaler`, `fin-contrat-signaler`, `fin-contrat-documents`.
>   Branchées dans `web.py`, sans repli. Deux Sonnet : l'un écrit (arrêté proprement), l'autre prouve et branche.
> - **Preuve** : 2 125 cas sur les quatre démos (arrêts, fins de CDI et de CDD, lignes DPAE du journal fabriqués
>   dans les copies), arbre de fichiers complet comparé, PDF par le texte, flux DPAE à horodatage masqué ;
>   0 écart de valeur. Écarts admis : classes de refus, la date du jour passée au moteur (le Python lit l'horloge).
> - **Python gardé** (appelants : ligne de commande, simulation, filet de H2) : `produire_dpae`,
>   `operations_arrets.produire`, `produire_fctu`, `documents_de_fin`, `operations_amorcage.produire`.
> - cargo 561.

> ## Décidé le 06/10/2026 : la suite du portage, inventaire fait (Léandre)
>
> Inventaire du 06/10 : 99 modules Python, les routes de `web.py`, les 40 commandes de la ligne de
> commande, les lanceurs et l'atelier. Ce qui n'était rangé dans aucun lot l'est ici.
>
> - **Routes encore en Python** : H2 `produire`, `deposer`, `suivre-depot`, `bilan-repetition`
>   « attendu » ; H3 `envoyer-documents`, `avenant(s)-envoyer`, `demander-signature`,
>   `dossier-envoyer`, `casc-envoyer` ; H4 `coffre-verifier`, `fiche-demander`, `fiches-relever`,
>   `google-autoriser` ; I `veille`, `veille-maintenant` ; J `ouvrir-piece`, `ouvrir-dossier`,
>   `ouvrir-exercices`, `ouvrir-exports`, `exports-periodes`.
> - **H2** : `produire` écrit puis prouvé fichier contre fichier (sans réseau) et branché. Les routes de
>   dépôt restent écrites, non prouvées : **leurs preuves réelles (vrais dépôts de test, garde-fous,
>   relève) se font après le portage, binaire compilé ; aucun dépôt réel avant** (Léandre, seul
>   utilisateur d'ici là). Le juge des dépôts est l'Urssaf, pas le Python ; la référence Python reste
>   dans l'historique git.
> - **H3 découpé en deux thèmes** (Léandre, 06/10/2026 ; ~1 000 lignes de Python, le double de `produire`, qui a
>   pris 403 000 jetons à écrire), chacun avec un Sonnet qui écrit puis un Sonnet neuf qui prouve et branche :
>   **H3a, la messagerie** : le client SMTP/IMAP (`courriel.py`), la mise en forme et l'envoi des courriels
>   (`envoi.py` : `en_mime`, `envoyer`, `relire_et_agir`) ; preuve sans réseau sur les courriels fabriqués, octet
>   par octet. **H3b, les six routes d'envoi** sur H3a : `envoyer-documents`, `demander-signature`,
>   `avenant-envoyer`, `avenants-envoyer`, `dossier-envoyer`, `casc-envoyer`. Le vrai envoi se prouve après le
>   portage, comme les dépôts.
> - **H4** gagne **la sauvegarde vers le Drive** (`sauvegarde.py`, sur `google.py`).
> - **I, le passage de fond** : porter `operations_ouverture.ouverture` — relève des retours (écrite en
>   H2), relève des signatures (sur la messagerie de H3), sauvegarde (H4), filet — et **l'horloge de la
>   démonstration** (`simulation.rattraper`, qui joue les mois d'une démo : produire, déposer, relever).
>   N'en font pas partie : la recherche de mise à jour (`maj.py` télécharge du code Python : remplacée
>   par la mise à jour de Tauri, en J), la boucle des 20 minutes et le serveur (Tauri, en J), la
>   vérification des sources (copie de développement seulement : atelier).
> - **J, d'abord la ligne de commande** (décidé le 05/10 : pas de lot à part, elle part avec J) : trier
>   les 40 commandes en **produit**, refaites en `paie …` (`retours`, `ouverture`, `sauvegarder`,
>   `signatures`, `coffre`, `pas`, `corriger`, `arret`, `dossier`, `exporter`, `echeancier`, `rappels`,
>   `cloture`, `export-compta`, `version`…), **atelier** (`sources`, `minima`, `rubriques`, `emplois`,
>   `dev`, `tester-dpae`, `importer` CoolPaie, `maj publier`) et **abandon** (ce que la fenêtre fait
>   déjà). Puis Tauri : la fenêtre, le choix de la compagnie (`compagnies.py`), les `ouvrir-*`,
>   `exports-periodes`, la mise à jour silencieuse ; **les lanceurs remplacés** : `demarrer.py`,
>   `PuppetPaie.sh/.cmd/.command`, `fenetre.py`, les fenêtres Tk `lanceur*.pyw` et leurs `.cmd`
>   (Retours DSN, Dépôt DPAE, Dépôt DSN). Plus aucun Python sur le poste.
> - **K, le retrait du Python** : chaque module sans appelant (vérifié par `grep`, l'atelier compris) ;
>   **la démonstration n'est pas l'atelier** (Léandre) : c'est une partie du produit (la version de
>   démonstration de novembre, quatre compagnies jouées par le vrai moteur). Son fabricant,
>   `outils/compagnies_demo.py`, doit continuer de marcher : il passe par `paie …`, il ne se retire pas ;
>   l'horloge qui la fait vivre est au lot I. **Les outils de l'atelier qui importent le moteur Python**
>   passent par `paie …` ou se retirent : `compagnies_fictives.py` (le crash-test), `exporter.py`,
>   `extraire_usages.py`, `extraire_formats.py` (la fabrique Python),
>   `auditer_le_catalogue.py`, `essai_contrat_plurimensuel.py` ; **les tests Python** (1 211) qui
>   testent le moteur Python se réécrivent dans `paie/tests/` ou se retirent (chantier « vider
>   l'atelier » d'`ETAT.md`).
> - **Après le portage** : les preuves réelles des dépôts de H2, puis les petits restes d'`ETAT.md`.

> ## Fait le 07/10/2026 : les lots H3 et H4
>
> - **H3a** (la messagerie, `courriel.rs`, `envoi.rs`) et **H3b** (les six routes d'envoi, `routes_envois.rs`,
>   `envois_signature.rs`) : PR #148 et #149, journal §206-207.
> - **H4 coupé en deux (Léandre)** ; **`google-autoriser` déplacé au lot J** (la fenêtre de consentement, Tauri).
> - **H4a** : `coffre-verifier` (`coffre_verifier.rs`), `fiche-demander` et `fiches-relever`
>   (`routes_fiches_courriel.rs`, `fiche_questionnaire.rs`, `courriel_lecture.rs`), branchées sans repli.
> - **H4b** : le client Drive (`drive.rs`, trait `Drive` sans suppression), la sauvegarde (`sauvegarde.rs`),
>   l'archivage (`archivage.rs`), les routes `sauvegarde-derniere` et `sauvegarde-rien-n-a-bouge`
>   (`routes_sauvegarde.rs`) ; la veille de `web.py` branchée ; `sauvegarder` et `archiver` attendent leurs
>   appelants (lots I et J). Dépendance `md-5`.
> - Dix défauts du Python non reproduits (§208). cargo 747 ; binaire 13,23 Mo. PR #150.

> ## Décidé le 04/10/2026 : le lot J, l'interface sous Tauri (Léandre)
>
> - **Le produit en trois parties**, chacune versionnée et signée (Ed25519), livrées par la
>   même mise à jour silencieuse : `PuppetPaie` (le moteur Rust, qui change rarement),
>   `interface/` (HTML/JS/CSS **chargés depuis le disque** par Tauri, pas embarqués : ils
>   changent chaque semaine pendant les essais de novembre-décembre), `donnees/` (les
>   référentiels, avec leurs sources et leurs dates). À côté, `compagnies/`, un dossier par
>   compagnie. Détail : `PLAN-DEPOT-PUBLIC.md`, « La forme du dépôt public ».
> - **Corriger l'interface ne touche jamais le moteur** : son empreinte (`PAIE_SOURCES`, qui
>   couvre `paie/src` et `Cargo.lock`) reste la même, aucun mois n'est recalculé, aucune
>   déclaration ne peut en être affectée. L'interface et les données déclarent la version
>   minimale du moteur qu'elles exigent.
> - **Ce qui se porte au lot J, et rien d'autre** : le point d'appel `interface.js:67`
>   (`fetch("/api/" + route)`) et l'envoi des incidents du navigateur (`interface.js:140`,
>   `/api/incident-navigateur`) passent à l'appel Tauri. Le reste de la page ne se porte pas.
> - **Tout garde-fou vit dans le moteur** (mot de passe retapé, refus, contrôles) : un fichier
>   d'interface se modifie facilement sur le disque, il n'autorise donc jamais rien. La page
>   affiche et demande ; le moteur décide.

> ## Décidé le 08/10/2026 : le lot J découpé, la ligne de commande triée (Léandre)
>
> - **J0, le tri** (`prive/journal/j/TRI-J0.md`) : 45 commandes, produit / atelier (en K) / abandon ;
>   `paie route <nom>` est l'accès complet, les commandes lisibles seulement pour l'usage courant ; `documents`
>   retirée ; `depots` cachée, et l'enquête jointe d'office aux incidents des DSN refusées, à anomalies ou jamais
>   vues (avec « toutes les anomalies remontent », après le portage).
> - **Une IA ne fait rien de réel sans humain** : le moteur demande lui-même le mot de passe d'un acte réel
>   (une fenêtre s'il y a un écran, le terminal si un humain y est, refus sinon) ; il ne passe jamais par un agent.
> - **L'ordre** : J1 (le mot de passe ouvre tout, le moteur le demande), J2a (consultation), J2b (actes), J3a (Tauri,
>   la fenêtre et le pont), J3b (compagnies, `ouvrir-*`, `exports-periodes`, `google-autoriser`), J4 (la mise à jour
>   en trois parties, `maj --publier`), J5 (lanceurs, signature du binaire, livraison).

> ## Fait le 08/10/2026 : J0, J1, J2a écrit (journal §210)
>
> - **J1** (`demande.rs`) : la séance gardée ouvre le coffre (après la clé du poste et le mot de passe tapé) ; un acte
>   réel sans mot de passe dans la charge, le moteur le demande lui-même (fenêtre — Tauri, en J3 —, terminal sans écho,
>   sinon refus) ; la séance, `PUPPETPAIE_MOTDEPASSE` et le mot de passe retenu n'autorisent jamais un acte.
> - **J2a, écrit, non prouvé** (`src/commandes/`) : les 14 commandes de consultation ; les commandes internes du
>   Python passent sous `paie interne NOM` (`paie_rust._appeler`) ; `rpassword` (Windows). Binaire 13 663 152 octets.
> - **La preuve de J2a en trois parties**, avec un binaire de l'atelier, `paie-atelier`, seul à accepter de faux
>   serveurs et qui refuse tout acte réel (`prive/journal/j/consignes-J2a-preuve.md`).
> - pytest 1 211 ; cargo 853.

Mesurer la taille du binaire à la fin de chaque étape et la noter ici :
`paie` 6,1 Mo le 02/10/2026 nuit (calcul, bulletin et fabrique, polices
comprises) ; 6,2 Mo le 02/10/2026 tard, avec les quatre services ; 6,4 Mo
le 02/10/2026 nuit avec la DSN (`cargo build --release` sous Linux, §185) ;
6,5 Mo le 03/10/2026 avec le FCTU, l'amorçage et le signalement (§186) ;
6,6 Mo le 03/10/2026 avec la DPAE et l'AER (§187) ; 6,8 Mo le 03/10/2026 avec
la lecture des retours (§188) ; 8,1 Mo le 03/10/2026 soir avec la préparation
des dépôts et le transport (§189 : rustls, +1,3 Mo) ; 8,2 Mo le 03/10/2026 avec le coffre (+112 Ko : AES-GCM et scrypt) ; 8,2 Mo (8 249 Ko) le 03/10/2026 avec l'écriture,
le verrou et le journal ; 8,4 Mo le 03/10/2026 nuit avec les états du journal (§192) ;
8,9 Mo (8 937 Ko) le 04/10/2026 avec les routes de l'ouverture et l'écart au contrat (§193) ;
9,2 Mo (9 158 Ko) le 04/10/2026 après-midi avec le lot E et F1 (§194) ;
9,3 Mo (9 294 Ko) le 04/10/2026 soir avec F2, F3, F4 en partie (§195) ;
9,4 Mo (9 351 Ko) le 04/10/2026 fin d'après-midi avec `changements-declares` et l'aperçu DSN (§196) ;
9,5 Mo (9 546 Ko) le 04/10/2026 soir avec les cinq aperçus de F2 (§197) ;
10,3 Mo (10 534 Ko) le 05/10/2026 avec le lot Incidents et `regex` (§199);
10,9 Mo (10 607 Ko) le 05/10/2026 avec le lot G1 (§200) ;
11,0 Mo (10 752 Ko) le 05/10/2026 avec les lots G2 et G3 (§201) ;
12,4 Mo (12 359 424 octets) le 06/10/2026 avec G4 et H1 (§204).

---

## 15. Ce que cette carte ne couvre pas

Les phases 2 à 6 : la DSN (phase 2 ; **l'AEM n'est plus émise depuis le
01/06/2026 : rien n'en est porté**, Léandre, 02/10/2026), `calibrer` en Rust (3), les PDF en
gabarits SVG (4 — faite autrement le 01/10/2026 : tous les PDF, AER
comprise, passent par la fabrique Rust `fabrique-pdf/`, et le Python n'a plus
aucune bibliothèque de PDF), le réseau (5), la persistance et l'interface (6 — Tauri sur la
même page HTML, décidé le 16/09). Elles ne se planifient pas utilement avant
que la phase 1 tourne.

**Linux, à la phase 6** (l'éditeur, 24/09/2026 : « tout cela devrait marcher
quand on portera en Rust »). Le lanceur Python ne démarre pas sous Linux
(pywebview sans GTK ni Qt, tkinter absent d'Ubuntu, aucun lanceur ; journal
§129). Tauri règle la cause — un binaire, WebKitGTK sous Linux, plus de
serveur local —, mais la preuve reste à faire : un `.deb` et un `.AppImage`,
installés et ouverts **comme un utilisateur** sur une Ubuntu neuve, avant de
dire que Linux marche.

Deux notes pour la phase 2, relevées en passant et à ne pas perdre :

- `dsn._bases` lit **`paie.bases`** (les assiettes du calcul, sommées par
  rubrique) depuis le 17/09/2026 ; elle ne recalcule plus `brut × 1,115` sur
  le total. `Paie.bases` fait partie de la sortie de `calculer` (§7.10) et de
  l'oracle.
- `dsn.cotisations` recompose le barème (`bareme_cadre(bareme_du_statut(…))`)
  et lit `cotisations_dsn`, avec `lignes` (somme de lignes du bulletin, l'Agirc-
  Arrco d'un cadre « y compris Apec »), `composants`, et les trois blocs 81
  de la RGDU (018 Urssaf, 018 France Travail, 106).

**Le coffre, à la phase 6 : il voyage déjà, on le porte tel quel**
(noté le 03/10/2026). Depuis le 20/09/2026 il n'a plus rien de Windows
(`moteur/coffre.py`, en-tête ; `README.md`, « coffre ») : AES-256-GCM, une
clé maîtresse scellée deux fois — par la **clé du poste**
(`~/.config/puppetpaie/cle-poste.bin`, `%APPDATA%\PuppetPaie\`, jamais dans
`prive/`) et par le **mot de passe du logiciel** (scrypt, paramètres écrits
dans le fichier). C'est ce qui permet déjà d'ouvrir PuppetPaie depuis une
clé USB ou un poste neuf : le mot de passe ouvre le coffre et repose une clé
de poste. Le port Rust (crates `aes-gcm` et `scrypt` de RustCrypto) **lit et
écrit le même `coffre.bin`, `VERSION = 2`** — aucune migration, prouvé par
des coffres écrits par le Python et relus par le Rust, et l'inverse. Ce qui
ne voyage pas, et c'est voulu : `google-jeton.bin` et `maj-cle-privee.bin`
n'ont que l'enveloppe du poste (sur un autre poste, on relie le Drive).
(Plus vrai pour la seconde depuis le 14/10/2026, lot K1 : scellée par la clé
maîtresse du coffre, elle s'ouvre comme lui.)

Rappel de la règle du dépôt public, **révisée le 09/09/2026** : le dépôt est
`claude/logiciel/`, Python compris — le crate Rust y vivra à côté. Jamais
`prive/` : ni `config/`, ni `paies/`, ni `historique/`, ni `retours/`, ni
`gabarits/`, ni `reference/`. L'historique git est définitif — le premier
commit doit déjà être propre.

---

## Ce que la nuit du 19/09/2026 ajoute à porter

Quatre modules neufs, et un seul est du **calcul** — les trois autres sont de
la mise en forme ou du contrôle, et se porteront tard ou jamais.

| module | ce que c'est | pour le portage |
|---|---|---|
| `moteur/feries.py` | les onze fêtes légales (L3133-1) et le **comput de Pâques** | **du calcul pur, à porter tôt** : c'est le seul endroit du logiciel où un algorithme remplace un relevé, et il entre dans le décompte des jours ouvrables, donc dans l'indemnité de congés. Valeurs attendues : Pâques 2024 = 31/03, 2025 = 20/04, 2026 = 05/04, 2027 = 28/03 |
| `moteur/permanents.maintien_de_salaire` | carence, deux tranches, allongement par l'ancienneté, douze mois glissants (D1226-1 à -8) | du calcul, mais sur des entiers de jours : pas de piège d'arrondi. Les **deux tiers se calculent en deux tiers**, pas en 66,66 % — le taux du fichier de données est celui qu'un bulletin imprime |
| `moteur/formats.py` | longueur et expression de contrôle de chaque rubrique | un moteur d'expressions régulières sur le fichier produit. Attention : les expressions sont celles du cahier technique, pas les nôtres |
| `moteur/fctu.py`, `documents_fin.py` | le signalement de fin de contrat, les deux papiers | de la mise en forme. Aucun calcul propre — le FCTU réutilise les blocs de `dsn.py` |

**Le soir du 19/09/2026 en ajoute trois**, et elles sont du calcul pur —
elles ne lisent rien d'autre que `permanents-base.json` :

| fonction | ce qu'elle fait | pour le portage |
|---|---|---|
| `permanents.indemnite_depart_retraite` | D1237-1 : un demi-mois après dix ans, un après quinze, un et demi après vingt, deux après trente | **un palier, pas un cumul** — c'est la seule échelle du logiciel qui ne s'additionne pas, et un port qui la traite comme R1234-2 paie six fois trop. Rien du tout sous dix ans |
| `permanents.indemnite_mise_a_la_retraite` | L1237-7 : le barème de L1234-9, sans ses exclusions de motif | partage le même barème que l'indemnité de licenciement (`_bareme_licenciement`) : une seule fonction à porter pour les trois |
| `permanents.code_52_permis` | CCH-21 à CCH-24 : quel code d'indemnité va avec quel motif de rupture | une table, pas un calcul. À porter avec le contrôle du bloc 52, au point d'écriture |

Et une chose qui **simplifie** le portage : la pile de barème ne s'écrit plus
qu'à un endroit, `calcul.bareme_applique` (statut → nature → santé →
catégorie). Elle était recopiée à trois ; un port qui en oublie un étage
déclare les cotisations de quelqu'un d'autre.

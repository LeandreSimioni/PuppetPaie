# Architecture

## Le principe

CoolPaie a produit cinq ans de déclarations acceptées par l'URSSAF, France
Travail et Audiens. Ces fichiers sont la spécification : chaque taux, chaque
règle d'arrondi et chaque champ de ce moteur en a été déduit, puis vérifié en
rejouant les mêmes mois et en comparant caractère par caractère.

D'où trois règles de conception.

**Le calcul est du code, pas du langage.** Un modèle de langage ne doit jamais
calculer une cotisation. `engagement/src/calcul.rs` applique un barème JSON versionné ;
Claude ne fait que traduire une phrase en fichier de saisie et lancer la
commande.

**Les formats à largeur fixe partent d'un vrai enregistrement.** L'AEM et la
DPAE sont générées en recopiant un enregistrement réel (`gabarits/`) et en n'y
remplaçant que les zones identifiées. Tout ce qui n'a pas été compris reste
donc exactement tel que l'administration l'a déjà accepté.

**Rien n'est réputé juste sans rejeu.** Le moteur a été calé en reconstruisant
chaque DSN et chaque AEM du dossier `export/` à partir de leurs propres données
et en les comparant à l'original (`calibrer`, du temps du Python ; retiré avec
lui et non porté). Ce qui en tient lieu : les attendus
transcrits dans les tests (`atelier/sources/moteur/tests/`) et le dépôt de test,
seul validateur (règle de fond 10).

## Le flux

```
   une phrase, ou engagements/AAAA-MM.json (des engagements, leurs lignes de paie datées)
              │
              ▼
   routes.rs ── les routes du moteur (`moteur route`) : l'orchestration et les
              │    garde-fous, seules à décider ; la ligne de commande
              │    (commandes/) et l'interface (logiciel/interface/, par
              │    `moteur route`) ne font que les appeler. Un aiguillage, un
              │    module par lot de routes derrière (routes_*.rs)
              ▼
   contexte.rs ── config/entreprise.json, salaries.json, spectacles.json,
              │   organismes.json, baremes/AAAA.json, compteurs.json
              ▼
   calcul.rs  ── brut, assiette CSG, plafond, rubriques du catalogue, net, coût
              │
   ┌──────────┼───────────┬────────────┬────────────────────┐
   ▼          ▼           ▼            ▼                    ▼
contrat.rs bulletin.rs  dpae.rs      dsn.rs        conges_spectacles.rs
  PDF        PDF        990 c.   NEODeS P26V01            PDF
   │          │           │            │                    │
   └──────────┴───────────┴────────────┴────────────────────┘
                          │
                       sorties/AAAA-MM/
                          │
        depot.rs / transport.rs ── dépôt par l'API, contre le mot de passe du logiciel
                          │
                     journal.rs ── journal/depots.json : ce qu'un fichier
                                   est devenu, et ce que l'organisme en a dit
```

(Les modules sont dans `atelier/sources/moteur/src/` ; chacun nomme en tête le
module Python qu'il a porté.) Plus d'AEM produite depuis le 01/06/2026 (100 %
DSN, France Travail édite l'attestation lui-même — `moteur aer`) ; rien de son
écriture n'est porté, l'import relit l'archive 2021-2026
(`moteur/src/importation.rs`, `lire_aem`). La source : le courriel de France Travail du
28/05/2026 à la compagnie (`reference-privee/sources/francetravail-100-dsn/
francetravail-courriel-fin-phase-pilote-100-dsn-20260528.pdf` : relevé sur une
compagnie réelle, pièce hors du dépôt public, dépôt privé) — fin des AEM,
AE Cinéma Spectacle remise au salarié, `S21.G00.40.025` vide ; la date vaut
pour cette compagnie, une autre reçoit la sienne.

## L'engagement : un objet, ses paies validées

L'engagement est au centre (crate `engagement`, `atelier/sources/engagement/`). Il vit dans
`engagements/AAAA-MM.json`, le fichier **du mois où il commence**, et nulle part ailleurs ; un engagement de
novembre à janvier est un seul objet de `engagements/2026-11.json`.

- **Sa forme** (`engagement/src/stock.rs`) : son premier mois en clair (salarié, spectacle, `debut`, `fin`, lignes de
  paie datées…) ; chaque mois d'après n'écrit que ce qu'il change, sous `mois_suivants` (`null` : retiré). Le salarié,
  les bornes et le numéro de contrat sont ceux de l'engagement dans tous ses mois (`PARTAGES`) ; la nature, le
  spectacle, l'emploi… se reconduisent d'un mois au suivant (`RECONDUITS`) ; un contrat d'usage commence chaque mois
  d'après avec les jours que le mois d'avant a mis au planning (une ligne par ligne qui en a). Le fichier d'un mois
  porte aussi les défauts du mois (`taux_cachet`, `date_paiement`).
- **Le mois de l'écran** (`stock::vue_du_mois`) : le fichier du mois et chaque engagement d'un mois d'avant qui court
  encore, vu ce mois-là. L'écran lit et écrit cette vue (`stock::ecrire_vue_du_mois`) ; chaque engagement y retrouve
  son objet (numéro, puis salarié). Un engagement validé ne change pas par ce chemin.
- **Trois états** (`engagement/src/etats.rs`), trois types : **en saisie** (calculé à mesure pour l'écran, rien ne
  part de lui) ; **validé** (« Valider l'engagement » écrit sous `valide` la date et une paie par mois couvert, avec
  son prélèvement à la source et l'empreinte de ce dont le calcul dépend : fichiers lus, fiches, saisie des mois lus,
  code du moteur, règles datées) ; **documents partis** pour un mois (le journal des envois, `journal/depots.json`, a
  une ligne `documents` du mois qui nomme le salarié). Tout document se fabrique depuis les paies écrites
  (`etats::validees_du_mois`, `etats::paies_du_mois`, qui refuse un mois dont un engagement est en saisie) ; aucun ne
  recalcule.
- **Chaque document se fabrique à son départ** (`moteur/src/depart.rs`) : la DSN à son dépôt (`fabriquer_dsn`, route
  `dsn-a-deposer`), dès qu'un engagement du mois est validé, sans attendre DPAE ni contrat ; le bulletin et le certificat
  Congés Spectacles à l'envoi des documents (`fabriquer_documents`), qui écrit dans la paie le numéro du bulletin
  (`N/AAAA`) et le premier N° CE (`etats::poser_numeros_des_documents`) ; le contrat à son envoi à la signature, où son
  numéro se fige (`envois_signature::reserver_numeros_envoyes`). Une paie partie puis refaite part sous un numéro
  nouveau, en rectificatif (`rectifie`), l'original gardé. Après la veille de l'échéance, un mois déclaré ne se
  redépose plus (`AnnuleRemplaceHorsDelai`) : sa correction s'écrit d'elle-même et part dans la DSN suivante
  (`depart::corrections_dues`). L'AER ne se fabrique pas : France Travail la renvoie, et elle se vérifie contre les
  paies écrites.
- **Le recalcul de lui-même** (`etats::rafraichir`, à chaque lecture d'un mois, `etats::du_mois`) : une paie validée
  dont l'empreinte ne tient plus et dont les documents ne sont pas partis se recalcule, d'un seul calcul du mois. Si un
  chiffre change, elle dit pourquoi : « taux de PAS reçu », « règle du JJ/MM/AAAA », « calcul corrigé » (le code ou ce
  qu'il livre), « un autre engagement du mois a changé », « configuration de la compagnie modifiée ». L'écran le montre
  (« Recalculé le JJ/MM/AAAA : … »). Sans chiffre changé, rien ne se dit. Un recalcul que le moteur refuse
  garde la paie validée et se note en incident pour l'éditeur (`incidents::poser_le_temoin_des_recalculs`).
- **Modifier…** (route `valider-engagement`, `valide: false`) repasse l'engagement en saisie. Si les documents d'un de
  ses mois sont partis, le moteur pose d'abord sa question (`etats::QUESTION_DOCUMENTS_PARTIS`) et ne bouge qu'avec la
  réponse (`confirme`) ; les paies parties se gardent (`figees`) et se reprennent telles quelles à la validation si la
  saisie de leur mois n'a pas changé.
- **Le numéro de contrat** se pose sur l'engagement quand le contrat part à la signature ou avec son premier mois
  (`stock::poser_numero`), et se relit ensuite ; un mois qui n'est pas le premier ne se numérote pas
  (`engagement::refus_sans_numero`, règle 4).

## La ligne de paie datée

Le fichier de saisie ne décrit pas un engagement par ses dates et son lieu,
mais par ses **lignes de paie** : « 4 cachets à 130 € au Théâtre 71 le 14, 15,
17, 18 ». Une ligne est un groupe de cachets, à un taux, à un endroit, à des
dates. Deux villes, deux lignes — et c'est une seule paie, donc un contrat, un
bulletin, une AEM. Le format est dans le `README.md`.

Ce que ça impose au moteur :

- `dates_engagement` (`engagement/src/lib.rs`) est le **seul endroit** qui sache où
  chercher les jours d'un engagement : dans ses lignes, et nulle part
  ailleurs. `charger` passe par lui, et le décompte des droits (`cumul.rs`) lit ce qu'il a chargé.
- **Une seule écriture** depuis le 13/09/2026. `Engagement` exige
  `lignes` ; `cachets` et `heures` en sont **dérivés** (l'AEM, le bloc 53 de
  la DSN, le plafond et `cumul` les lisent toujours), `taux_cachet` est le
  taux de la première ligne `cachet`, `taux_horaire` n'existe plus. Un
  engagement portant `dates`, `cachets`, `heures` ou `taux_horaire` est
  refusé par `charger`, qui dit ce qu'il a vu et ce qu'une saisie porte.
  Plus aucune saisie n'est à l'ancienne écriture : l'outil de conversion
  (Python) est retiré le 15/10/2026.
- `date::d` (`socle/src/date.rs`) accepte `{"jour": …, "lieu": …}` en plus des quatre formats de
  date : une date détaillée est une date, tout ce qui parcourt des jours
  continue de marcher sans le savoir.
- La construction d'un `Engagement` (`engagement/src/lib.rs`) réunit les deux écritures et n'en
  laisse qu'une : `dates_travail` reste une liste de dates triées, le lieu part
  dans `lieux` (`lieu_de()`, `lieux_joues()`). Le contrat groupe les dates par
  lieu ; le lieu ne se déclare nulle part ailleurs (ni DSN, ni AEM, ni DPAE).
- **L'ancienne écriture n'est plus lue** (`dates`, `cachets`, `heures` ou
  `taux_horaire` sur l'engagement). On l'avait voulue lue sans
  conversion le 13/09/2026 ; la règle d'une seule écriture l'a remplacée le
  soir même. Le moteur refuse un tel engagement et dit ce qu'une
  saisie porte (`engagement/src/lib.rs`, à la
  lecture d'un engagement sans `lignes` et à la lecture du mois). Ce qui reste
  lu tel quel, ce sont les dates d'une ligne écrites sans lieu (une chaîne) à
  côté des dates détaillées.

## Les métiers, les minima, et les sources qu'on date

Trois fichiers de données livrés avec le moteur, et une règle : **toute
donnée réglementaire part d'une source officielle téléchargée, décrite,
datée, vérifiable**. CoolPaie ne sert qu'à vérifier.

- `moteur/nomenclature-emplois-ft.json` — les **1 236 codes emploi** France
  Travail (annexes 8 et 10), **générés** depuis le PDF de la nomenclature.
- `moteur/emplois-base.json` — **129 métiers du spectacle vivant** en onze
  familles. Chaque métier : libellé (et féminin), `emploi` officiel
  (S21.G00.40.006), `pcs_ese` (40.004, depuis les fiches Insee), `statut`
  (déduit de l'annexe, jamais saisi), `cadre_par_defaut`,
  `statut_conventionnel` (40.002 : cadre → 04, artiste et agent de maîtrise
  → 05, administratif non cadre → 06, employé technique → 07),
  `classification_3090` et `minima`. `emplois.resoudre(code, cadre, feminin)`
  rend sept champs : les six déclarés et le libellé lisible
  (`emploi_affichage`, au féminin si la fiche dit sexe 2). `engagements` les
  pose tous les sept sur l'engagement, sauf ce que l'engagement écrit
  lui-même (un `emploi` écrit à la main ne garde pas le libellé du métier) ;
  `emploi_de` les reporte sur la fiche de cet engagement, et le contrat lit
  le libellé de là. Le cadre se combine au statut.
- `moteur/minima-conventionnels.json` — les grilles de la **3090** (annexes
  1, 2, 4, 5 et clauses communes) et de la **1285** (cachet, répétition,
  cirque, musiciens, lyrique, chœur ; NAO 2026 sous `non_etendu`).
  `minima.avertissements(ctx, paie)` compare chaque ligne à son plancher et
  rend une phrase, jamais un refus. **Toute inconnue tire vers le bas** :
  jauge minimale, palier du mois pour un barème trimestriel, la plus basse
  des annexes quand la fiche entreprise ne dit pas la sienne
  (`annexe_convention`). Quand une grille distingue les jauges, le métier
  pointe la ligne la plus basse et la grille porte les autres pour mémoire.
- `logiciel/donnees/sources.json` — le **manifeste** : pour chaque source, titre,
  éditeur, URL, description, version et d'où on la lit, date de relevé,
  fichier de la copie (dans `atelier/reference/sources/`, versionnée dans le
  dépôt depuis le 15/10/2026 ; l'original d'un organisme privé n'y est pas, seule
  sa transcription), SHA-256, mode de vérification, et
  quels fichiers de données elle nourrit. `moteur sources --verifier`
  (poste de l'éditeur seulement, `moteur/src/sources.rs`) retélécharge et compare ; une source `changee` est à RELIRE
  (`--relever` pose la nouvelle version à côté). Légifrance refuse les
  robots (403) : la copie ne se revérifie pas en ligne, mais la source
  porte un bloc `kali` (IDCC, conteneur, texte) et `moteur/src/kali.rs` lit
  les **livraisons open data KALI** de la DILA
  (`echanges.dila.gouv.fr/OPENDATA/KALI/`, une archive par soir) depuis
  le relevé : fiche de la convention relivrée avec des textes nouveaux,
  texte modifié (état, arrêté d'extension). Un texte dont le titre parle
  de salaires, ou le texte cité, rend la source `changee` ; le reste est
  montré en second. Lecture incrémentale, résumée dans
  `atelier/reference/sources/veille-kali.json` (livraisons lues, liste des
  textes de chaque convention, trous de l'index) ; à la première veille,
  l'index est remonté jusqu'à la dernière fiche de chaque convention.
  L'ouverture du Python refaisait la vérification une fois par semaine ;
  en Rust elle ne la fait plus (`moteur/src/ouverture.rs`) : c'est l'éditeur
  qui la lance, avant chaque publication.
- `moteur releve-codes` (`moteur/src/releve_codes.rs`, porté du Python le
  15/10/2026) — **les articles de loi eux-mêmes**, lus dans la base **LEGI**
  de la DILA (même voie que KALI) plutôt qu'à la main sur Légifrance, qui
  refuse les robots. Tous les textes en un seul passage
  (`releve_codes/textes.rs`) : travail, sécurité sociale, collectivités
  territoriales (le versement mobilité), construction, impôts, et les arrêtés
  et la loi que la paie lit. Trois verbes, sur le dossier des sources :
  - `moissonner` tient à jour l'entrepôt `legi-entrepot/`
    (`moteur/src/entrepot_dila.rs`) : la première fois, ou avec
    `--reconstruire`, l'archive globale défile (1,1 Go, en flux, jamais
    écrite sur le disque) puis **chaque livraison quotidienne depuis sa
    date** ; ensuite seules les livraisons nouvelles se téléchargent. Il garde
    la version *en vigueur* de chaque article — `<ETAT>` à VIGUEUR et
    `<DATE_FIN>` à 2999-01-01, vérifié, parce que le dossier
    `code_en_vigueur` porte aussi les versions périmées —, note au passage
    quel article a changé de version, et depuis quelle livraison, puis écrit
    `releve.json` et `xml/` de chaque texte ;
  - `rapport` écrit le document lisible d'un code à partir de ce qui a été
    moissonné, chaque article cité avec l'identifiant de **sa** version ;
  - `comparer` répond à la question que `moissonner` pose. « Huit articles
    ont changé de version » ne dit pas si le *droit* a changé : il faut
    relire chacun contre sa version d'avant, et **cette version-là ne se
    trouve que dans l'archive globale** — ni le relevé, qui ne garde que la
    version en vigueur, ni l'historique Git, où un relevé précédent portait
    déjà celle d'après. `comparer` va l'y chercher **par son identifiant**
    (jamais par le numéro : l'archive porte plusieurs rédactions du même
    article), la range dans `<code>/xml-anterieur/` et écrit le diff,
    alinéa par alinéa. Il s'arrête dès qu'il les a toutes : comptez un quart
    d'heure. Il compare le `BLOC_TEXTUEL` seul, et le dit.
- `moteur releve-conventions` (`moteur/src/releve_conventions.rs`) — le texte
  complet des conventions que cite `sources.json`, tenu dans l'entrepôt
  `kali-entrepot/` : `mettre-a-jour` (les seules livraisons KALI nouvelles ;
  la base entière, 180 Mo, la première fois ou pour une convention nouvelle),
  `article <IDCC> <numéro>…`, `chercher <IDCC> <mot>`.

  Ce que ce dernier verbe a trouvé le 21/09/2026, et qu'un relevé sans
  comparaison aurait laissé passer : sur huit articles changés, **un seul
  changement touchait la paie de près** — le taux de L137-12, passé de 30 à
  40 % —, `L242-1` perdant un alinéa sans que son 7°, le seul dont le moteur
  dépende, bouge d'un mot.

Ce que la confrontation à CoolPaie a montré (`reference-privee/professions-coolpaie.md (relevé sur une compagnie réelle, pièce hors du dépôt public : dépôt privé)`) :
les 51 codes PE concordent ; CoolPaie met l'accessoiriste en PCS `637d`,
l'Insee dit `637c` ; sa colonne « abat. » n'est pas la DFS réglementaire.

## Le barème, et comment il a été retrouvé

Point de départ : le bulletin FAVART de juin 2026, brut 260,00 €.

| Rubrique | Base | Patronal | Salarial |
|---|---|---|---|
| Maladie maternité invalidité décès | brut | 9,100 | — |
| Prévoyance Audiens | brut | 1,110 | 0,120 |
| AT/MP | brut | 1,050 | — |
| Vieillesse plafonnée | brut | 5,990 | 4,830 |
| Vieillesse déplafonnée | brut | 1,480 | 0,280 |
| Retraite complémentaire T1 | brut | 5,740 | 5,300 |
| Famille | brut | 3,680 | — |
| Chômage | brut | 9,750 | 2,400 |
| Autres contributions | brut | 2,836 | — |
| Autres contributions | brut × 1,115 | 0,070 | — |
| Autres contributions | brut × 1,10 | 0,590 + 0,090 | — |
| Congés Spectacles | brut | 15,500 | — |
| Cotisations statutaires CCN | brut | 0,500 | — |
| CSG déductible | assiette CSG | — | 6,800 |
| CSG/CRDS non déductible | assiette CSG | — | 2,900 |

Les taux de vieillesse et de famille sont les taux de droit commun réduits de
30 % : c'est l'abattement applicable aux artistes du spectacle (8,55 × 0,70 =
5,985 ≈ 5,990 ; 6,90 × 0,70 = 4,83 ; 5,25 × 0,70 = 3,675 ≈ 3,680).

**Le plafond de Sécurité sociale est dans le barème** (`plafond_securite_sociale`
: `{"mensuel": 4005, "horaire": 30}`, source BOSS), depuis le 16/09/2026 —
c'était la seule valeur réglementaire qui vivait dans le code. Le module
(`droit/src/plafond.rs`) ne porte plus que la règle, et
`calcul::plafond_de_l_engagement` en est la seule source : le bas du bulletin,
les bases du calcul, la DSN et le bordereau la suivent. La règle, telle que
le module la dit avec ses textes (`atelier/reference/sources/`) :

- un **artiste** engagé moins de cinq jours calendaires : 12 × le PSS horaire
  par journée de travail de la paie (arrêté du 24/01/1975, art. 3, al. 2) ;
- tout **autre intermittent** (artiste engagé cinq jours ou plus, technicien) :
  le PSS mensuel au prorata des jours rémunérés de la paie, mois par mois (la
  tolérance du BOSS, § 1090, appliquée : le BOSS est opposable) ;
- un **permanent** : le PSS au prorata des jours de sa période de paie
  couverts par le contrat (CSS R. 242-2) ; à **temps partiel**, proratisé
  aussi à sa quotité (CSS L. 242-8), sauf si la fiche entreprise dit que
  l'établissement y renonce (`plafond_temps_partiel` : « entier ») ; un
  artiste à temps partiel garde le plafond entier (BOSS § 1090). Le choix se
  déclare en `S21.G00.40.084` à partir de P27V01 (`calcul::plafond_temps_partiel`,
  `dsn::proratisation_du_plafond` ; lots V4b et R5a).

Sur ce plafond s'assoient, pour un intermittent, la vieillesse plafonnée, le
FNAL (`brut_plafonne`, `brut_x115_plafonne`) et le chômage, dans la limite de
quatre plafonds (`brut_chomage`) ; pour un permanent, les tranches et le
chômage, dans la limite de quatre plafonds de la période (`brut_chomage`, lot
V2e).

**Une valeur du barème peut être datée.** Le SMIC l'a toujours été
(`smic_horaire`, liste de `{depuis, valeur}`) ; depuis le 16/09/2026 toute
valeur peut l'être — un taux, un plafond — parce que les DSN de CoolPaie
montrent des changements en cours d'année (AGS 0,20 → 0,25 au 01/07/2024,
chômage 11,45 → 11,40 au 01/06/2025) :

```json
"taux": [{"depuis": "2024-01-01", "valeur": 0.20},
         {"depuis": "2024-07-01", "valeur": 0.25}]
```

`Contexte.bareme(date)` rend le barème **résolu à cette date** ; `calcul` et
`dsn` lisent des nombres et n'en savent rien. Une date antérieure à toute
entrée est un refus, pas un zéro.

**La configuration de la compagnie est datée de même.** Chaque valeur d'une
fiche — `config/entreprise.json` (convention, annexe, adresse, établissements…),
chaque salarié de `salaries.json`, chaque spectacle de `spectacles.json` —
s'écrit `[{"valeur": …}, {"depuis": "AAAA-MM-JJ", "valeur": …}]` : la première
valeur, sans `depuis`, vaut **de tout temps** (un contrat commencé avant la
saisie de la fiche, un mois repris d'un autre logiciel la trouvent), chacune des
suivantes à partir de sa date ; les clés `_…` sont des notes. Les organismes
et contrats de la DSN (`config/organismes.json`, chaque clé, lus par
`Contexte::organismes` ; enregistrer pour un mois écrit à partir de son premier
jour) et les rubriques propres à la compagnie (`config/rubriques-compagnie.json`,
lues par `catalogue::catalogue(racine, jour)`) sont datés de même ;
`rubriques-actives.json`, préférence d'affichage qu'aucun calcul ne lit, ne
l'est pas. Une seule
lecture, `contexte::fiche_au`, sur la même `valeur_au` que les barèmes ; une
valeur non datée se refuse avec la phrase qui dit quoi faire
(`contexte::non_datee`). Le
`Contexte` garde les fiches datées et les lit à `lue_au` ; `Contexte::au(jour)`
et `du_mois(mois)` donnent le contexte d'une période. Le calcul d'une période
lit tout à sa fin (`calculer_periode`), la DSN d'un mois et ses documents à son
dernier jour (`dsn::generer`, `entreprise_du_mois`, et toute route qui porte un
`mois`, `routes::dans_la_fenetre`), et le barème d'un jour applique la
convention de ce jour. On écrit par `contexte::dater`, et seulement par lui : la
première valeur d'un champ s'y écrit de tout temps, quelle que soit la date
donnée ; changer une valeur existante exige sa date d'effet
(`routes_ecritures::ecrire_datee`, l'écran la demande alors) ; ce qui précède
garde l'ancienne valeur.

Trois subtilités, chacune trouvée en cherchant pourquoi un centime manquait.

**L'assiette CSG n'est pas 98,25 % du brut.** Elle vaut
`arrondi(brut × 0,9825) + part patronale de prévoyance`, soit
255,45 + 2,89 = 258,34 pour 260 € de brut, et 510,90 + 5,77 = 516,67 pour 520 €.

**Une ligne du bulletin peut cacher deux cotisations.** La ligne
« autres contributions 0,680 % » sur 286,00 € affiche 1,95 € alors que
286 × 0,68 % = 1,9448 → 1,94. Ce sont en réalité deux contributions arrondies
séparément : 0,590 % → 1,69 et 0,090 % → 0,26. Même mécanisme pour la
cotisation prévoyance `059` de la DSN (0,420 + 0,810 ; « Cotisation individuelle Prévoyance-Assurance-Mutuelle », versée à Audiens Santé Prévoyance, P0983).

**Tous les arrondis sont au demi supérieur.** `round()` de Python arrondissait au
pair le plus proche et se trompait donc d'un centime régulièrement ; `eur`
(`socle/src/num.rs`) arrondit un `Decimal` au demi qui s'éloigne de zéro
(`MidpointAwayFromZero`, le `ROUND_HALF_UP` d'avant).

## Le SMIC déclaré en DSN (composant 01 de la base 03)

Cette section s'appelait « Le plafond déclaré en DSN » : ce n'est pas un
plafond. `S21.G00.79.001 = '01'` est le « Montant du SMIC retenu pour le
calcul de la Réduction générale », rattaché à une base 03 (cahier technique
2026.1, p. 299-300 ; `atelier/reference/norme-dsn/texte/bloc-S21.G00.79.txt` ;
fiche N3 de `prive/journal/norme/`).

Dans les DSN de CoolPaie, il valait **SMIC horaire × 7 h × nombre de
cachets**. Vérifié sur soixante-dix contrats de 2021 à 2026 : la valeur par
cachet suit exactement les revalorisations du SMIC (10,25 → 10,48 → 10,57 →
10,85 → 11,07 → 11,27 → 11,65 → 11,88 → 12,02 → 12,31), et pour les contrats
payés à l'heure c'est SMIC × nombre d'heures — 52 h en octobre 2025, 140 h en
novembre 2023.

Le moteur (`dsn::bases_et_cotisations`) écrit sous chaque base 03 le SMIC de
référence de la réduction générale quand la paie en a une
(`ReductionGenerale::smic_reference`), sinon `Paie::plafond` — qui, malgré
son nom, n'est pas un plafond de Sécurité sociale mais le SMIC horaire × les
heures de la paie (`calcul::calculer`, « 7.5 » ; les heures d'un intermittent
bornées à `heures_par_cachet` × jours) : la valeur de CoolPaie. Un artiste n'a
pas de réduction générale et reçoit quand même ce composant (reste #41 de
`prive/journal/RESTES-A-FINALISER.md`). Le plafond de Sécurité sociale est
décrit plus haut, avec le barème.

## L'AEM V5

Enregistrements de 1000 caractères, latin-1, séparés par CRLF. Zones
identifiées dans `moteur/aem.py` (Python, retiré : plus aucune AEM n'est émise
depuis le 01/06/2026 et son écriture n'est pas portée ; l'import relit l'archive,
`moteur/src/importation.rs`). La queue numérique est celle-ci :

| Position | Contenu |
|---|---|
| 650-653 | nombre d'**heures** effectuées |
| 653-655 | nombre de **cachets** |
| 655-659 | nombre de **jours** travaillés |
| 659-668 | brut en centimes |
| 668-677 | assiette des contributions chômage en centimes |
| 677-681 | taux de contribution chômage × 100 (1215 = 12,15 %) |
| 681-690 | montant de contribution, **arrondi à l'euro** puis × 100 |
| 690-712 | zéros |
| 712-721 | même montant |

Ce schéma a été testé sur les 189 enregistrements émis depuis 2021 : 189/189
conformes. C'est l'arrondi à l'euro, et non au centime, qui bloquait au départ
(119,79 → 120, 7,29 → 7, 36,30 → 36).

### Les positions 650-659, élucidées

Elles se lisaient mal parce qu'un contrat au cachet pur a autant de jours que
de cachets : `00002` + `0002` se laissait interpréter comme « 2 heures, 2
cachets », et 168 des 189 enregistrements sont dans ce cas. Les contrats mixtes
trahissaient l'erreur (`01201`, `02401`, `01604`…).

Le formulaire officiel imprimé par CoolPaie (`affichage/aemV5/aemV501.pdf`)
donne les trois champs accolés : **Nombre d'HEURES effectuées**, **Nombre de
CACHETS**, **Nombre de JOURS travaillés**. La découpe 3 / 2 / 4 est cohérente
sur 185 des 189 enregistrements — les 4 restants sont un même contrat de
décembre 2021 au format antérieur.

Exemple, le contrat du 23 au 26 août 2026 : `012` `01` `0004` = 12 heures,
1 cachet, 4 jours travaillés. Brut 370 € = 130 € de cachet + 12 h à 20 €.

Le moteur calcule donc ces trois champs au lieu de recopier une valeur
inexpliquée. Un contrat mixte se saisit avec une ligne `cachet` **et** une
ligne `heure`, chacune à son taux : le taux du cachet ne s'applique pas aux
heures.

### Heures, cachets et jours sont trois grandeurs indépendantes

Et elles se lisent toutes au même endroit, sur l'engagement. Les **cachets**
sont la somme des quantités des lignes en unité cachet, les **heures** celles
des autres lignes, et les **jours** sont un fait qui se saisit (`jours` sur
l'engagement) : on peut jouer un cachet réparti sur trois jours, ou deux
cachets le même jour. À défaut, les jours valent le nombre de dates jouées
**distinctes**.

Ces trois valeurs partent dans l'AEM (positions 650-659), dans la DSN (bloc
`S21.G00.53` : unité 12 journée, 37 cachets, 35 heures) et dans les droits
(`cumul.rs`). Aucune de ces destinations ne les recalcule — la DSN le faisait
pour les jours jusqu'au 20/09/2026, et déclarait donc autre chose que
l'attestation dès que les deux différaient. Un engagement qui porte des
lignes de paie et **zéro jour** est refusé : l'unité 12 ouvre les droits du
salarié, et un mois payé en compte au moins un. Un mois du contrat sans paye
(`lignes: []`) et un contrat mensuel, qui compte en heures, en sont exclus.

## Les droits (annexes 8 et 10), et d'où viennent les contrats

`droit/src/cumul.rs` compte les 507 heures sur 12 mois (un cachet = 12 h, les
heures d'enseignement plafonnées) et imprime le bloc de cumul du bulletin.
Depuis le 15/09/2026 il ne lit qu'**une source, la nôtre**, en deux fichiers :

| | quoi | quand |
|---|---|---|
| `prive/historique/engagements.json` | les contrats attestés **avant** ce logiciel : les AEM de CoolPaie, importées une fois (`moteur importer`) — dates, numéro d'objet, cachets, heures, jours, brut, numéro et date d'attestation | pour tout mois qui n'a pas de fichier de paie |
| `prive/engagements/AAAA-MM.json` | les engagements saisis ici, lus par `charger` (`engagement/src/lib.rs`) (un seul chemin de calcul : mêmes cachets, heures et brut que le bulletin et la DSN) | pour le mois du fichier — et l'historique se tait |

La règle de départage tient en une phrase : **un mois qui a son fichier de
paie est lu là, jamais deux fois.** Pendant le double run d'août 2026, l'AEM
de CoolPaie et notre fichier disent la même chose (vérifié : six engagements,
mêmes cachets, heures, jours et brut), et c'est le fichier qui compte. Un
fichier qui ne se charge pas (brouillon en cours, année sans barème) ne
couvre pas son mois : l'historique parle pour lui s'il a quelque chose à
dire. Les bornes d'un engagement issu d'une paie sont celles de sa **période
de paie** (le contrat borné au mois) : un contrat d'octobre à décembre compte
ses cachets d'octobre en octobre.

Le bulletin se compte lui-même : `bulletin.ecrire` passe les paies qu'il
imprime à `cumul.lire_engagements(ctx, paies_courantes=...)`, qui les
substitue au fichier de leur mois.

Avant cette date, `cumul.py` lisait `export/AEM_EDI` — le dossier de
CoolPaie — directement. Cela liait les droits à un logiciel qu'on résilie en
janvier 2027, et laissait de côté les mois produits ici (plus d'AEM depuis le
01/06/2026 : la compagnie est en 100 % DSN). `lire_aem` reste (`moteur/src/importation.rs`) : il
dépouille un dossier d'AEM (l'import s'en sert, et nos propres AEM de
`sorties/`), mais plus personne ne l'appelle pour les droits.

### Les conseils « Le saviez-vous ? » (ticket 224)

Une **option de la compagnie** (fiche Entreprise, `conseils`, décochée par défaut) : sans elle, aucune route ne rend
rien. Le texte, ses chiffres et ses sources sont des données (`logiciel/donnees/conseils.json`, chaque citation lue à
l'identique dans un texte déposé, `tests/fichiers/conseils_banque.rs`) ; `moteur/src/conseils.rs` n'a que les conditions.
Trois familles, qui parlent à l'employeur :

| | où | ce qui décide |
|---|---|---|
| le conseil du jour (`conseil-du-jour`, « un autre » : `decalage`) | l'accueil (`js/conseils.js`) | `quand` = « toujours », hors `suspendu` ; rang = `jour × pas mod N`, `pas` premier avec N : N jours de suite font le tour sans répétition, rien ne s'écrit |
| le conseil de saisie (champ `conseil` d'une paie de `apercu`) | sous la carte d'engagement, au plus un | `ordre_saisie` : plus de 208 h d'un technicien, plus de 28 cachets, mois sans allocation (art. 30), 507 heures assurées chez nous, cachet de répétition de la 3090, heures qu'une même somme couvre au minimum horaire, enseignement, suspension, cachets et heures |
| ce que la compagnie a fait (`conseils-salarie`, `conseils-resume`) | la fiche du salarié ; l'accueil | `cumul::position_droits` sur la fenêtre (depuis `anniversaire_droits`, sinon 12 mois glissants) : heures comptées chez nous et part des 507, cachets perdus d'un mois, ce que les conseils de saisie auraient changé |

Un conseil de saisie n'est **pas un avis** (`engagement::Avis`) : il ne passe ni dans les avertissements de la paie, ni au
bulletin, ni à `produire` ; une paie validée (sa carte figée) n'en a pas. Il tient en **une phrase** (`texte`) ; son
`detail` (les deux faces, les chiffres) et sa référence se déplient derrière « pourquoi ? » : le moteur rend les deux, la
page n'affiche que ce qu'elle reçoit. Le salarié s'y nomme **par son seul prénom** (`{prenom}`, `conseils::prenom`), jamais
par un pronom qui le genre (le test `conseils_banque` y veille). Les deux conseils « plus d'heures » ne se montrent que si
la somme couvre le minimum conventionnel (`minima::cachet_de_repetition`, `minima::minimum_de_la_ligne`) et disent leurs
deux faces : précieux s'il manque des heures pour les 507, à éviter si elles sont assurées un mois indemnisé (le coût se
dit en jours d'allocation, art. 30, au demi-jour le plus proche, jamais en euros ; un coût qui s'arrondit à zéro se dit
« moins d'un demi-jour », variante `-minime` de la banque). « Heures pour la même somme » ne parle qu'à partir d'un gain
du poids d'un cachet (12 heures, `conseils::gain_heures`, la justification y est) : hors du seuil des 507 heures, une
heure de plus coûte plus qu'elle ne rapporte. **Le seul cas de certitude** : nos contrats font à eux seuls au moins
507 heures sur la fenêtre ; ses droits sont ouverts, et « intense plutôt que dilué » prend la place des conseils « plus
d'heures ».

### L'import CoolPaie

`moteur/src/importation.rs` lit le dossier `import/` de la compagnie (les DSN
pour l'identité, l'emploi, les numéros d'objet et l'entreprise ; les AEM pour
les contrats ; l'`export/` de CoolPaie avant le 10/10/2026, `atelier/README.md`,
« Reprendre l'historique ») ;
`moteur/src/importation_ecrite.rs` décide quoi écrire, par les mêmes fiches que
l'écran (`routes_ecritures::ecrire_fiche` : même clé, même matricule, même vérification,
même filet). Une fiche existante n'est jamais modifiée : ce que la DSN disait
autrement est rendu dans `ecarts`, et c'est à l'humain de dater un
changement. La dernière DSN où un salarié figure fait foi pour son identité ;
l'annule-et-remplace d'un mois passe après l'original. Un numéro d'objet
sans titre devient `objet-<numéro>`, titre « A COMPLETER », et l'année se lit
dans le numéro (`266Z` = 2026). La base CoolPaie (`bases/*.CSOC`) est
chiffrée : on ne la lit pas.

## La DSN

Norme NEODeS P26V01, puis P27V01 : la version se choisit **par fichier**
(lot V4a, 10/10/2026), depuis `logiciel/donnees/norme-dsn-base.json` — le mois
principal déclaré pour une mensuelle, le jour de constitution pour un
signalement (`declarations/src/norme.rs`). Cette règle de bascule est une **inférence**
(`_inference` de la donnée) : le cahier technique ne la donne pas. Chaque
version a ses tables d'usages et de formats ; `usages` et `formats` prennent
celles de la version que le fichier porte (`S10.G00.00.006`), et le fichier en
fabrication décide par elles des rubriques qu'une version a retirées (`06.003`,
`11.002` en P27V01) ou ajoutées (`Fichier::norme`, `Norme::admet`). Les
tables P27V01 sont relevées des classeurs 2027.1.2 de net-entreprises (fiches
3357 et 3358) par `moteur extraire usages` et `moteur extraire formats` (`moteur/src/extraire/`).
La part patronale de la CET se déclare en `146` en P26V01 et en `148` en P27V01,
sous la base 78 du `131` (type 23), à l'Urssaf (fiche net-entreprises 2556 ;
`cotisation_cet_patronale` de la donnée). Le code APE de la fiche entreprise se
saisit en NAF rév. 2 ou en NAF 2025 (`logiciel/donnees/naf-base.json`, relevé
de l'INSEE) ; un fichier P27V01 refuse un code NAF rév. 2 et dit vers quels codes
NAF 2025 il passe, sans choisir (`declarations/src/naf.rs`).
`declarations/src/dsn.rs` émet les blocs dans l'ordre observé chez CoolPaie : le
générateur Python dont il est le portage reproduisait juillet 2026 à la ligne
près (997 lignes identiques). Il s'en écarte depuis, là où la relecture de la
norme l'a corrigé (`prive/journal/RELECTURE-NORME-DSN.md`).

Ce que le moteur écrit, là où on l'avait d'abord déduit des DSN de CoolPaie
(« correspondances retrouvées », redites le 11/10/2026 d'après la norme
déposée et le code ; les erreurs relevées sont dans les fiches N1 à N4 de
`prive/journal/norme/`) :

- `S21.G00.70.012` et `S21.G00.78.005` sont l'« Identifiant technique
  Affiliation » : « interne au message », unique pour chaque affiliation du
  salarié, le 78.005 d'une base 31 citant le 70.012 de son affiliation (cahier
  technique 2026.1, p. 286 et p. 297 ; `atelier/reference/norme-dsn/texte/bloc-S21.G00.70.txt`,
  `bloc-S21.G00.78.txt`). Ce n'est ni une affectation ni une « contribution
  formation ». La norme laisse la numérotation libre ; le moteur prend 202 + la
  place de la référence d'adhésion, décalée de `places_par_contrat` × (rang du
  contrat − 1), où `places_par_contrat` = max(3, 2 + contributions de branche
  déclarées) : 3 en 3090, 4 en 1285 (`dsn.rs` : `places_par_contrat`,
  `bloc_contrat_fctu`, `bases_et_cotisations`).
- `S21.G00.40.043` est le taux AT/MP notifié à l'établissement (`taux_atmp`
  du barème, que porte la surcouche de la compagnie ; refus s'il manque). Il
  n'a aucun lien avec le bloc 54.
- `S21.G00.54.002` est un **montant** en euros (p. 258). Le moteur écrit un
  bloc 54 par ligne du bulletin qui porte `dsn_autres_bruts`, le type `10` pour
  la déduction forfaitaire spécifique, et les types `92` (santé) et `93`
  (prévoyance et retraite supplémentaire) : `cotisation_sante_patronale` et
  `cotisation_prevoyance_retraite_supp` du barème (les lignes patronales du
  bulletin, ou un taux sur une base), arrondis à l'euro (`dsn::autre_revenu`).
  La règle « arrondi supérieur(cachets × 1,5) » n'existe ni dans la norme ni
  dans le code.
- `S21.G00.53` : `12` et `37` sont des **unités** (53.003), pas des types :
  `12` = journée (les jours distincts travaillés), `37` = cachets isolés (le
  nombre de cachets), `35` = heures d'intermittent hors cachets ; un permanent
  déclare `10` (heure), et ses absences en type `02` (p. 256-257 ; `activites`
  de chaque nature dans `logiciel/donnees/contrats-base.json`, `dsn::activites`).
- `S21.G00.86` (type 07, unité 01) = l'ancienneté en jours depuis
  `date_anciennete` de la fiche jusqu'à la fin du mois déclaré, jamais zéro
  (CCH-12, p. 322 ; `dsn::anciennete_du_mois`). Une fiche qui ne la porte pas
  refuse ; le 01/12/2019 d'avant était la date de la compagnie d'essai.
- `S21.G00.23` : une ligne par CTP de `config/organismes.json` (composé depuis
  `ctp_urssaf` de `logiciel/donnees/organismes-base.json`), sur l'assiette que
  la ligne nomme (`brut`, `csg`, `brut_artiste`, `brut_x115_artiste`,
  `ta_principale`, `chomage_permanent`…), sommée sur les paies du mois
  (`dsn::assiettes_ctp`, plafonnée sous le qualifiant 921) et arrondie à
  l'euro ; le montant se calcule depuis la table des CTP (`bordereau.rs`). La
  norme ne définit pas ces assiettes, elle renvoie au guide Urssaf (p. 42 et
  p. 168).
- `S21.G00.50.008` est l'identifiant du taux porté par le compte rendu métier
  (CRM) du PAS que transmet la DGFiP (p. 244) : il se lit dans les retours
  (`retours.rs`), ne se calcule pas, et ne s'écrit qu'avec un taux de type 01
  (`dsn::bloc_versement`).

**Un contrat qui déborde du mois** (15/09/2026, option B1 de
`prive/journal/CONTRATS-PLURIMENSUELS.md`). `Engagement` connaît son mois
(`mois`, posé par `charger`) et en déduit `premier_mois`, `dernier_mois`,
`debut_periode`/`fin_periode` (le contrat borné au mois). La DSN en tire :

- bloc 40 identique chaque mois — 40.001 début, 40.009 numéro, 40.010 fin
  prévisionnelle sont les identifiants du contrat (fiche net-entreprises
  n° 3344) ;
- blocs 50, 51, 54, 58, 78 sur la **période de paie du mois**, pas sur le
  contrat : la norme veut « a minima » la période de paie (§ 2.1.3.2, p. 28),
  et une date au plus le dernier jour du mois principal (§ 4.4.12, p. 99) ; les
  CCH-11 de 51.001 et CCH-13 de 51.002 (p. 246-247) bornent, elles, par le
  contrat (fiche N2). En P27V01, le 58 d'un versement du mois n'est plus daté :
  il hérite de la datation du 50 (fiche net-entreprises 2983,
  `dsn::dates_du_58_du_mois`) ;
- bloc 62 (fin de contrat) **le dernier mois seulement** ;
- bloc 53 : l'activité du mois, comme toujours ;
- date de constitution (S20.G00.05.007) : le jour où le fichier est fabriqué, comme les signalements et la FCTU (retours Windows, F, 07/10/2026). Elle valait avant la fin de période la plus tardive du mois ; une DSN produite le 07/10 restait « constituée le 23/09 ». Un fichier refait un autre jour porte donc une autre date.

`Engagement.planning` (jours des mois suivants, lus sous `planning` sur les
lignes, avec leur lieu dans `planning_lieux`) ne compte ni dans
`dates_travail`, ni dans les cachets, ni dans la DSN : il donne `fin` par
défaut, `dates_du_contrat()` pour le contrat imprimé, et la reprise du
mois suivant (`engagement_de_reprise`). Le contrat entier recomposé pour le
certificat CS (`paie_du_contrat_entier`) retire le planning des lignes :
chaque mois a payé le sien.

Le barème et le SMIC (`calcul.calculer`) sont ceux de `fin_periode`, pas de
`fin` : un contrat de décembre à février se paie en décembre au barème de
décembre. Sur un contrat qui tient dans son mois — tout l'historique — ces
propriétés valent `debut`/`fin` et rien ne bouge : `calibrer` l'a prouvé sur
75 DSN, et `tests/test_contrats_plurimensuels.py` (du temps du Python) rejouait un contrat fictif
octobre → décembre et lisait les trois DSN — déposées en test le 15/09/2026,
conformes. La quotité 40.012/013 est celle du **contrat entier** (jours
signés : dates du premier mois + planning, `Engagement.jours_contrat`),
identique chaque mois et jamais zéro : le cahier technique 2026.1 interdit
le zéro en unité journée (bloc 40, CCH-11). Comme elle ne bouge pas d'un mois
à l'autre, le CDDU n'a pas de changement de quotité à déclarer en bloc 41 ;
le bloc 41 (41.007 pour la quotité) s'écrit quand une modalité d'un contrat
déjà déclaré change (`changements_contrat.rs`). Plafond 31 en journée
(CCH-12), 250 en heure (CCH-13), sur 40.012 comme sur 40.013 : au-delà,
`dsn::bloc_contrat_fctu` refuse (`unite_quotite.plafond` de `contrats-base.json`).

**Un mois du contrat sans aucun cachet** (`lignes: []`, `Engagement.sans_paie`)
a quand même sa DSN : le contrat en cours avec un versement à zéro — bloc 50,
quatre 51, 53 à 0 jour, 58, et par affiliation prévoyance une base 78 « 31 »
avec sa cotisation 81 « 059 », tous à `0.00` ; ni 54, ni autre base. Pas de
bulletin. Établi par trois dépôts de test le 15/09/2026 (ETAT.md §26.9).

### L'échéance de dépôt : le 5 ou le 15, reportée au jour ouvré, à midi

Depuis le 15/10/2026, **un seul endroit** la donne : `declarations/src/echeance_dsn.rs` (`du_mois`, `de_la_compagnie`).
Le jour est celui de R. 243-6, II (le 5 du mois suivant à partir de cinquante salariés quand la paie est faite dans le
mois, le 15 sinon ; `Contexte::echeance_dsn` choisit selon l'effectif) ; R. 133-14, I le **reporte** : « Si le délai
imparti pour effectuer la déclaration sociale nominative expire un jour férié ou non ouvré, il est prorogé jusqu'au jour
ouvrable suivant » ; l'heure limite est **midi** (cahier technique 2026.1, § 1.4.2), et l'annule-et-remplace intégral
s'arrête la veille à minuit (`Echeance::veille`). Les textes et le raisonnement :
`atelier/reference/sources/echeances-dsn/` (`RAPPORT-20261015.md`, le calendrier de net-entreprises : samedi
05/12/2026 → lundi 07/12, dimanche 15/11/2026 → lundi 16/11).

Trois **inférences**, écrites dans le module et rendues dans le champ `inference` des réponses : « non ouvré » se lit
sur le calendrier du point de dépôt (samedi, dimanche, les onze fêtes légales de `feries::feries`), jamais sur les
fériés chômés de la compagnie (`feries::chomes`) ; « jour ouvrable suivant » se lit « premier jour ouvré » (un vendredi
férié reporte au lundi, aucune pièce officielle ne montre ce cas) ; la veille de l'annule-et-remplace est celle de
l'échéance reportée.

Tout ce qui parle de la date limite de dépôt passe par lui : l'état du mois et l'annule-et-remplace
(`Moteur::limite`), les régularisations, le bilan des mois, l'échéancier, les rappels, les réveils du filet, le mois
hors délai, l'horloge des compagnies fictives, et la date butoir d'un **signalement d'arrêt subrogé** (R. 133-14, II :
avec la DSN du mois de l'événement, donc avec son report). Les routes rendent, sans toucher à leurs champs existants,
`echeance_depot` (`echeance_dsn` sur une obligation) : `jour`, `heure` (`12:00`), `nominale`, `reportee`, `pourquoi`,
`phrase`, `veille_annule_et_remplace`, `textes`, `inference`. **Une seule date n'en dépend pas** : la date de paiement
du bloc 20 (`S21.G00.20.011`, `dsn.rs`, `echeance_mois_suivant`) reste le 5 ou le 15 de R. 243-6, qui ne prévoit
aucun report, et aucun texte déposé ne dit qu'un prélèvement se reporte.

### Un individu s'identifie par son NIR **ou** par son NTT

`S21.G00.30.001` (le NIR) est **conditionnelle**, et c'est ce qui permet de
déclarer quelqu'un qui n'est pas encore immatriculé — une première embauche,
un artiste étranger. En son absence, la norme exige le **numéro technique
temporaire** `S21.G00.30.020` (CCH-12), et le moteur le fabrique :
`sexe + SIREN + matricule`, jamais saisi (`declarations/src/ntt.rs`).

| | |
|---|---|
| ce qui l'écrit | `ntt::poser_nir` / `ntt::poser_ntt`, appelés par les émetteurs de bloc 30 — `dsn.rs`, `fctu.rs`, `signalement.rs`, `amorcage.rs` |
| les règles | CSL-11 (commence par 1 ou 2), CSL-12 (aucun espace), CCH-13 (caractères 2 à 10 = le SIREN), longueur [11,40] — trois d'entre elles ne sont **pas** dans le relevé des formats, comme pour le NIR |
| quand les deux partent ensemble | la déclaration du mois où le NIR a pris effet : « le NTT doit être renseigné sur la première DSN où le NIR est attribué », pour raccorder les deux identités |
| ce qui le date | l'arrivée du NIR sur une fiche qui existait est un changement daté (`routes_ecritures::changements_a_dater`) : la fiche dit « sans NIR » jusqu'à la veille — seule case qui, en se remplissant, demande sa date |
| ce qu'il n'est pas | une donnée de gestion : il n'ouvre **aucun droit**, et ne s'imprime ni sur un bulletin, ni sur un contrat, ni sur un certificat |

Deux conséquences hors DSN : la **DPAE** part alors avec sa zone NIR à blanc,
ce qui vaut demande d'immatriculation (formalité `IMMA`) ; et le **taux de
PAS** ne viendra jamais, la DGFiP n'ayant personne à qui répondre — c'est le
barème de l'article 204 H du CGI qui s'applique, et le moteur ne sait pas
encore le lire : il refuse (`pas.etat`).

Déposé en test le 20/09/2026, un salarié sans NIR, identifié par son seul
NTT : **0 anomalie bloquante, 0 non bloquante**, certificat de conformité
délivré (flux `WShDs9Z0fCO3ouFVp.nLino`).

### Le montant Urssaf : un calcul

(Ce versement a longtemps été nommé « Audiens » ici et dans la commande : les
libellés de `organismes.json` étaient intervertis, corrigés le 10/09/2026 ;
les SIRET et les montants ont toujours été justes.)

Le bordereau Urssaf d'une DSN (22.005, recopié en 20.005) est la somme de ses
lignes 23, chacune valorisée au taux de la table des CTP de l'Urssaf pour la
période (plafonné pour le qualifiant 921, déplafonné pour le 920, le taux AT
arrondi à part, les réductions de format F en déduction), arrondie à l'euro
ligne par ligne : `documents/src/bordereau.rs`, table
`moteur/urssaf-codes-types-personnel.csv`.
Rejouée sur les 75 DSN de CoolPaie, la règle en retrouve 74 au centime
(octobre 2025 : 394 pour 393). Un code absent de la table fait refuser : le
bloc est un **ordre de télépaiement**, un montant faux fait un prélèvement faux.

### Les cotisations se calculent rubrique par rubrique

CoolPaie n'applique pas les taux au brut global : il les applique à **chaque
rubrique de salaire**, arrondit chaque montant, puis somme. L'assiette CSG suit
la même règle ; les cotisations CSG, en revanche, se calculent une seule fois
sur l'assiette agrégée.

Invisible sur un contrat au cachet pur, qui n'a qu'une rubrique — d'où 168
bulletins qui ne trahissaient rien. Le bulletin 27/2026 (1 cachet à 130 € et
12 h de masterclass à 20 €) l'a révélé :

| | rubrique par rubrique | brut global | CoolPaie |
|---|---|---|---|
| Assiette CSG | 367,63 | 367,64 | **367,63** |
| Charges patronales | 212,99 | 213,00 | **212,99** |
| Coût employeur | 582,99 | 583,00 | **582,99** |

L'arrondi lui-même est bien **au demi supérieur** : vérifié sur les 144 cas
ambigus des cotisations `S21.G00.81` de l'historique, 144/144.

## La nature du contrat, et les salariés permanents

Écrit le 18/09/2026, **chantier en cours** : le calcul, la déclaration, le
signalement de fin de contrat et l'écran d'un permanent marchent. Ce qui
manquait était un taux — la prévoyance des permanents — et on le croyait à
demander à Audiens ; le 19/09/2026 il a été **relevé en ligne**, puis rangé là
où toute compagnie le lira : **la convention le fixe, donc le logiciel le
livre**. L'état exact est aux §38 à §45 de `prive/journal/ETAT.md`.

### Le taux d'une branche n'est pas une donnée de compagnie

`logiciel/donnees/prevoyance-conventionnelle-base.json` porte la prévoyance des
permanents telle que la fixe la convention collective, et `droit/src/bareme.rs` (module `prevoyance`)
la lit. 1,20 % de la tranche A est vrai pour **toutes** les entreprises de
l'IDCC 3090 : le mettre dans le barème d'une compagnie, c'est le ranger là où
aucune autre ne le trouvera. La ligne de partage, et elle vaut pour la suite :

| | où ça vit | qui tranche |
|---|---|---|
| ce que la loi ou la convention **impose** | catalogue livré avec le code | personne — le moteur applique |
| ce que l'entreprise **choisit** | `config/entreprise.json` | la compagnie |
| ce que le moteur **ne peut pas savoir** | la fiche du salarié | il pose la question |

Concrètement, pour l'IDCC 3090 : le moteur applique **1,20 % de la tranche A,
100 % patronal** (art. 12.6), **porté à 1,50 %** pour un cadre (ANI du
17/11/2017) — sans rien demander, parce que c'est obligatoire. Une compagnie
qui n'a rien saisi est donc correctement couverte. Elle n'écrit
`prevoyance_permanents` dans sa fiche que pour ce qu'elle a **en plus** :
l'extension facultative à la tranche 2 conventionnelle (2,32 % pour tous,
2,62 % pour les cadres — assiette : du plafond à **quatre** plafonds, pas huit,
d'où la base `t2_conventionnelle`), ou un contrat d'entreprise plus favorable.
Un contrat **sous** le minimum de branche est refusé : ce n'est pas un choix.

Et ce qui ne se devine pas se demande : la prévoyance d'un **non cadre**
s'ouvre après un an d'ancienneté **dans la branche**, pas dans l'entreprise.
Quand le salarié a moins d'un an chez nous et que sa fiche ne dit rien, le
moteur refuse et réclame `prevoyance_branche_depuis` — supposer qu'il vient de
la branche ferait cotiser pour rien, supposer le contraire laisserait un
salarié découvert pendant un an.

Le catalogue porte **deux** conventions. La **1285 (CCNEAC)**, le secteur
subventionné, y est **confirmée depuis le 23/09/2026** — non cadres 0,95 % T1,
cadres 1,50 % T1 + 0,81 % T2, 100 % patronal, dus dès l'embauche (art.
XII.2.1.2 et XII.2.1.5), la tranche 2 allant du plafond à **quatre**
plafonds. La confirmation vient de la **base KALI de la DILA** (open data,
sans compte, là où Légifrance refuse les robots) : la base complète, puis
toutes ses livraisons quotidiennes jusqu'à la veille, pour lire la version
**en vigueur** de chaque article. Elle porte aussi la **santé** des
permanents, qui est un taux de la convention et non un contrat à niveaux :
0,81 % du PMSS en 2026, 0,89 % au 01/01/2027, au moins pour moitié patronal
(XII.2.1.10 et 11) — `calcul._sante_conventionnelle`. Les onze autres
conventions qu'Audiens sert refusent, sans nombres — le mécanisme est écrit,
il n'y manque qu'un bloc. Le mécanisme `applicable: false` reste, pour la
prochaine convention relevée sans être confirmée.

Un écart est à connaître : la CCNEAC ajoute à l'interbranche, **pour ses
intermittents cadres techniciens seulement** (XII.2.2.1), 0,45 % en TA et
0,95 % en TB — 2,14 patronal en TA au lieu de 1,69, et une tranche B qu'une
compagnie privée n'a pas. Ses artistes cadres restent à l'interbranche
(XII.2.2.4). La couche 1285 du barème le porte sous
`par_categorie.cadre.par_statut.technicien`, et la tranche B s'assied sur
`t2_prevoyance_journaliere` (du plafond journalier à quatre fois lui).

### Ce qu'un permanent déclare, et ce qui se réclame salarié par salarié

Venu de `CLAUDE.md` (règle 6) le 04/10/2026. La prévoyance des permanents :
1,20 % de la tranche A pour la 3090, porté à 1,50 % pour un cadre, 100 %
patronal ; la CASC-SVP d'un permanent est celle de l'article 4.22 (0,50 % de la
tranche A mensuelle, 100 % patronale) ; les blocs 54 (92 santé, 93 prévoyance)
et les bases 31 (059) se lisent sur les lignes du bulletin. La section
`par_nature.permanent` du barème est complète depuis le 23/09/2026 (2025 et 2026).

Ce qui reste est **propre à chaque compagnie** et la fait refuser, elle seule :
ses références d'adhésion (`organismes.json`, `affiliations_par_nature.permanent`,
dans l'ordre prévoyance puis santé). **Deux faits se réclament salarié par
salarié** : le niveau de garantie santé (`sante_svp`) et, pour un non cadre de
moins d'un an dans l'entreprise, son ancienneté **dans la branche**
(`prevoyance_branche_depuis`), qui ouvre sa prévoyance.

### Un cadre permanent n'est pas un cadre intermittent

`par_categorie.cadre` du barème décrit un cadre **intermittent** : tranches
journalières, prévoyance de l'accord interbranches. Appliquée à une
administratrice en CDI, elle lui donnait des plafonds à la journée et la
prévoyance de quelqu'un d'autre, en silence. Depuis le 19/09/2026,
`calcul.bareme_cadre` lit `par_categorie.cadre_permanent` quand le contrat est
mensuel : mêmes taux Agirc-Arrco (le régime est unifié), mais l'Apec sur
quatre plafonds **du mois** et la CET sur des tranches mensuelles. Sa
prévoyance, elle, vient de la convention.

`logiciel/donnees/contrats-base.json` porte ce que la DSN écrit de la **structure** d'un
contrat, par nature — ce qui était, jusque-là, écrit dans `dsn.py`. Trois
natures : `cddu_spectacle` (l'intermittent, et le défaut d'un engagement qui
ne dit rien), `cdi`, `cdd`. Chaque code y porte sa valeur, sa phrase et sa
source ; `droit/src/natures.rs` les lit et ne décide rien.

Ce qui sépare un permanent d'un intermittent, en une page :

| | intermittent | permanent |
|---|---|---|
| ce qu'il vend | des cachets, des heures, **à des dates** | une **durée du travail** |
| quotité (40.011) | journée (12), ≤ 31 (CCH-12 du bloc 40) | heure (10), ≤ 250 (CCH-13 du bloc 40, p. 189 ; 3250 est le plafond de la mesure 53.002) |
| fin prévisionnelle | obligatoire | absente sur un CDI |
| numéro d'objet (40.050) | oui | **INTERDIT** — CCH-11 ne l'autorise que pour les natures 02 et 10 |
| congés payés | caisse des Congés Spectacles (40.022, cotisation 15,5 %), sauf un CDDU de douze mois ou plus | chez l'employeur : compteur, indemnité, solde ; sauf le CDD de droit commun de moins de douze mois d'un artiste, déclaré à la caisse (`caisse_conges::affilie`, lot V2f) |
| assiettes FNAL / taxe d'apprentissage | brut **majoré** de 11,5 % et de 10 % (CSS L. 243-1-3, D. 243-0-2), pour un contrat déclaré à la caisse des Congés Spectacles | brut nu ; tranche 1 majorée de 11,5 % (FNAL) et brut majoré de 11,5 % (taxe d'apprentissage) pour un contrat déclaré à la caisse (`par_caisse_conges.affilie` du barème) |
| plafond de Sécurité sociale | par paie : 12 × le PSS horaire par journée pour un artiste engagé moins de cinq jours, sinon le PSS au prorata des jours rémunérés (`plafond.rs`) | la période de paie (R. 242-2), proratisé à la quotité d'un temps partiel sauf renonciation de l'établissement (`40.084` en P27V01) |
| chômage | 9,75 / 2,40 dans la limite de quatre plafonds, recouvré par **France Travail** | 4,00 + AGS 0,25 dans la limite de quatre plafonds de la période, recouvré par l'**Urssaf** (CTP 772, 937) |
| retraite complémentaire | taux Audiens, tranches **journalières** | droit commun, tranches **mensuelles** |
| formation professionnelle | 2,00 % (CTP 983) | 0,55 % (CTP 959) |
| mutuelle santé | aucune | **obligatoire** (ANI, art. L911-7) |
| fin du contrat | à son terme (031) : DSN mensuelle, circuit dérogatoire 62.017 = 01 (la norme le réserve au contrat infra-mensuel, p. 277 : pour un CDDU à cheval sur deux mois, c'est une décision ouverte, fiche N5) ; un FCTU si le salarié le demande. Rompu avant son terme (036, 037, 084…) : 62.017 = 02 et **signalement nature 07** (CCH-12, fiche 2062 ; lot C1, `Nature::signalement_fctu`) | **signalement nature 07** (FCTU) |
| blocs 51 | les quatre valent le brut | 001 brut, 002 assiette chômage, 003 salaire rétabli, 010 salaire de base |
| bloc 53 | jours (12), cachets (37), heures (35) | heures (10), absences en type 02 |

### Le barème a trois étages : le socle livré, la couche de convention, la surcouche de la compagnie

Coupé en deux le 19/09/2026. Un barème mélangeait les taux de **droit
commun** — Urssaf, Agirc-Arrco, chômage, CSG, plafond, SMIC, et les taux de
la **branche** du spectacle — qui sont les mêmes pour tout le monde, et ce
qui est **propre à une entreprise**, qui tient en un nombre : son taux AT/MP
notifié. Mélangés, ils obligeaient chaque compagnie qui installe PuppetPaie à
écrire mille lignes de taux nationaux, c'est-à-dire à relever elle-même ce
que nous avions déjà relevé.

```
logiciel/donnees/baremes/2026.json      le socle : droit commun + tout le spectacle
  └─ section `par_convention`           la couche de SA convention, choisie par l'IDCC
config/baremes/2026.json                ce qui est propre à la compagnie, 2 Ko
config/effectifs.json                   son effectif, écrit par le moteur depuis
                                        les retours de nature 118 (envois/src/releve_retours.rs)
```

**Trois étages, et le deuxième est une bifurcation.** Le corps du socle porte
le droit commun (Urssaf, Agirc-Arrco, CSG, plafond, SMIC) **et** ce qui vaut
pour tout le spectacle : Congés Spectacles 15,50 % (la même caisse pour la
3090 et la 1285), chômage 9,75 / 2,40 des annexes 8 et 10, prévoyance des
intermittents **non cadres** (0,12 / 1,11 — identique dans les deux
conventions), formation 2,00 %, Thalie Santé 0,42 %. Ce qu'une **convention**
change est dans sa couche : la cotisation conventionnelle (CASC-SVP 0,50 %
pour la 3090, **FNAS 1,45 %** pour la CCNEAC) et la prévoyance des
intermittents **cadres** (1,69 interbranche, **2,14** en CCNEAC, plus 0,95 en
tranche B). C'est `idcc` de la fiche entreprise qui choisit — pas le code.

**Une convention sans couche fait refuser le moteur**, et c'est le point :
payer un salarié d'une autre branche avec la CASC-SVP de la 3090 — une
cotisation qu'il ne doit pas, un FNAS qu'il doit et qui manque — ne se
verrait sur aucun bulletin. Une couche `applicable: false` refuse aussi :
ses nombres sont relevés, pas confirmés (ce fut l'état de la 1285 jusqu'au
23/09/2026), et le refus dit ce qu'il reste à relever.

**Ce qu'une couche peut porter en plus de rubriques à fondre** — et que la
1285 a fait écrire :

- `contributions_conventionnelles` : des rubriques posées par
  `calcul.bareme_convention` **après** le statut et la nature du contrat,
  donc pour tout le monde — la section `technicien` du socle recopie la
  CASC-SVP de la 3090 dans son `ccn`, et le FNAS doit la remplacer là aussi.
  Un taux peut y dépendre de la **représentation du personnel** de la fiche
  entreprise (`representation_personnel`, lue par
  `calcul.representation_du_personnel`, confrontée à l'effectif) et de la
  nature du contrat : c'est le FNAS, 1,45 % sous onze salariés ou sans
  représentation, 0,825 % (permanents) / 1,325 % (intermittents) avec un CSE
  conventionnel ou un CSE adhérent, rien avec un CSE qui n'adhère pas.
- `contributions_dsn` : les contributions que la DSN déclare sous une
  **adhésion Audiens** de la compagnie, comme la CASC-SVP de la 3090 — le
  FNAS et le FCAP de la CCNEAC. `dsn.contributions_dsn` les résout mois par
  mois (l'adhésion se lit dans `organismes.json`, `"contribution": "FNAS"`,
  et le moteur refuse sans elle), et `dsn.places_par_contrat` réserve autant
  d'identifiants d'affiliation de plus : trois pour la 3090, comme toujours
  — la numérotation des DSN déposées ne bouge pas —, quatre pour la 1285.

`moteur/baremes.fusionner` empile les trois, et `Contexte._bareme_brut` est
le seul endroit qui le fait : le reste du moteur lit un barème déjà fondu et
n'a pas à savoir d'où vient quel nombre. Un appel direct sans IDCC
(`idcc=None`) ne résout pas la convention et laisse le socle tel quel —
c'est le cas d'un outil ou d'un bac à sable.

**La règle de fusion a une subtilité.** Un dictionnaire se fond clé par clé.
Une **liste d'objets nommés** — les rubriques du bulletin (`id`), les
cotisations DSN (`code81` + `affectation`) — ne se remplace pas en bloc : la
surcouche reprend celles qu'elle nomme et laisse les autres. Une entreprise
de plus de cinquante salariés corrige son FNAL en une ligne :

```json
{"annee": 2026, "socle": 2026,
 "taux_atmp": {"valeur": 2.10, "code_risque": "923AD"},
 "rubriques_bulletin": [{"id": "autres_2", "taux_pat": 0.35}]}
```

Une liste de **valeurs** (les paliers du SMIC, les composants d'une
cotisation), elle, se remplace en entier.

**Le marqueur `socle` est explicite, et c'est voulu.** Un fichier qui ne le
porte pas est un barème **complet**, pris tel quel : ceux d'avant la coupure,
et les bacs à sable des tests. Deviner aurait été dangereux dans les deux
sens — une surcouche prise pour un barème complet paierait avec trois taux,
un barème complet pris pour une surcouche hériterait de taux qu'il avait de
bonnes raisons de ne pas avoir. `baremes.fige()` rend un barème autonome à
partir d'une surcouche, socle fondu et marqueur retiré.

**Le taux AT/MP ne s'écrit plus qu'une fois.** Il en existait cinq copies, et
un test de non-régression pour dire, chaque janvier, laquelle avait été
oubliée. Les rubriques du socle portent maintenant leur **facteur** — 0,70
pour un artiste, dont les taux Urssaf sont abattus de 30 % (arrêté du
24/01/1975), 1 pour tout le monde d'autre — et `calcul.bareme_atmp` fait la
multiplication. Il reste deux copies, dans les CTP de `organismes.json`, et
le test les garde alignées.

**Trois valeurs du socle supposent une entreprise de moins de onze
salariés** : le FNAL à 0,10 % (0,50 % à partir de 50), la formation
professionnelle des permanents à 0,55 % (1,00 % à partir de 11), et le
`tdelta` de la réduction générale, 0,3781 au FNAL 0,10. Elles sont écrites et
nommées dans `_ce_qui_depend_de_l_entreprise`, en tête du socle.

### L'effectif, ses seuils, et ce que le moteur refuse

Venu de `CLAUDE.md` (règle 8) le 04/10/2026, tel quel pour l'essentiel.

**Ce que l'effectif change est écrit pour être LU**, depuis le 21/09/2026 : la
section `seuils_effectif` du socle dit, chemin par chemin, où vit chaque valeur
qui dépend de l'effectif et ce que le socle y porte (formation des permanents à
onze salariés ; FNAL, `tdelta` et échéance de la DSN à cinquante).
`droit/src/contexte.rs` la confronte à l'effectif **calculé par l'Urssaf** — depuis
le 23/09/2026 il ne se saisit plus : il arrive en CRM de nature 118,
`envois/src/releve_retours.rs` l'écrit seul dans `config/effectifs.json` à chaque relevé, et
un seuil n'est franchi qu'après cinq années consécutives (L. 130-1, II) — et
**refuse de payer** si la surcouche ne suit pas le palier atteint.

- **Une compagnie neuve** n'a qu'à renseigner `premiere_embauche` sur sa fiche :
  avant elle, aucun salarié, et pendant cinq ans aucun seuil ne peut être franchi.
- **Une compagnie plus ancienne** dont les retours ne servent plus les vieux 118
  déclare dans `effectifs.json`
  `"jamais_au_dessus_de": {"seuil": 11, "jusqu_a": AAAA, "source": "..."}` — la
  seule clé de ce fichier qui s'écrit à la main.
- Ces seuils-là (formation, FNAL, échéance de la DSN) sont ceux du code de la
  sécurité sociale ; ceux du CSE (CCNEAC, 11 et 50) relèvent du code du travail,
  et le moteur ne les tranche pas avec l'effectif de l'Urssaf : la représentation
  déclarée fait foi (`calcul.representation_du_personnel`).
- Une valeur surchargée dans la surcouche est tenue pour décidée, comme le taux
  AT/MP.
- Le même module refuse une **adhésion à une organisation patronale** dont la
  couche n'est pas relevée (`par_organisation_patronale`) et un **second
  établissement** incomplet (adresse, taux AT/MP) ; complet, il a sa propre DSN
  depuis le 25/09/2026 (`droit/src/contexte.rs`, `declarations/src/dsn.rs`). Le contrôle est appelé dans
  `Contexte._bareme_brut`, seul endroit que les deux fronts traversent.

### Le barème a deux axes, pas un

Le corps d'un barème **décrit un intermittent**. `par_statut` dit s'il est
artiste ou technicien — c'est-à-dire si ses taux Urssaf sont abattus de 30 %
(arrêté du 24/01/1975). `par_nature` dit si son contrat est un CDDU ou un
contrat permanent. **Les deux sont indépendants** : un comédien en CDI garde
ses taux abattus *et* perd les Congés Spectacles. La pile est toujours la
même, et dans cet ordre :

```
bareme_du_statut  →  bareme_de_la_nature  →  bareme_cadre
```

`calcul.calculer`, `dsn.cotisations` et le bloc 54 l'appliquent tous les
trois. En sauter un, c'est déclarer les cotisations de quelqu'un d'autre.

La section `par_nature` empile par identifiant (`rubriques` remplace,
`rubriques_retirees` enlève, `rubriques_ajoutees` insère) ; seule la
**réduction générale se complète** au lieu d'être remplacée — sa formule vient
du statut, et la nature ne change que la destination de la part chômage
(`018:urssaf` et CTP 668 pour un permanent, `018:chomage` et CTP 671 pour un
intermittent).

**La section `par_nature.permanent` du barème 2026 n'est toujours pas
vérifiée**, donc elle refuse — mais ce qui manque a rétréci le 19/09/2026,
quand la fiche de paramétrage Audiens (version 139) a été relue en entier :
elle porte les permanents, et c'est même le seul endroit où ils apparaissent.
La **santé** y est relevée (1,44 / 1,84 / 2,04 % du PMSS selon le niveau) et
la **CASC-SVP** aussi (0,50 % de la tranche A). La **prévoyance des
permanents** n'est plus un trou : son taux vient de la convention, pas du
barème (voir plus haut), et la rubrique porte `_rempli_par` comme la mutuelle.
Ce qui retient encore la section, ce sont trois choses d'une autre nature : la
**répartition de la CASC-SVP**, le **bloc 54 type 93**, et les deux
**affiliations DSN 059** — leurs identifiants techniques viennent de la fiche
de paramétrage, qui n'existe qu'une fois l'entreprise adhérée au régime de sa
branche. La santé, elle, ne
se relève pas dans un barème : c'est un **forfait** dont le niveau se lit sur
le bulletin d'adhésion du salarié (`sante_svp` sur sa fiche), et sans ce
choix le moteur refuse, salarié par salarié.

### Le bordereau Urssaf d'un permanent

Les CTP viennent de la **table des CTP de l'Urssaf** (open data,
`open.urssaf.fr`, re-téléchargeable et donc re-vérifiable) : 100 (RG cas
général, avec le taux AT), 635 et 430 (compléments maladie et allocations
familiales — le CTP 100 porte encore les anciens taux réduits 7,00 et 3,45),
332 (FNAL plafonné), 772 (chômage), 937 (AGS), 959 (CFP), 987 (CPF-CDD, 1 % du
brut d'un CDD de droit commun, C. trav. L. 6331-6 : la somme des cotisations 129 du
nominatif ; le barème l'ajoute par `bareme::bareme_cpf_cdd` depuis le 11/10/2026), 668 (réduction
générale **étendue**, celle qui inclut le chômage : la somme des 018 de l'Urssaf
du nominatif, parts sécurité sociale et chômage, guide déclaratif Urssaf § 8.2 ;
`dsn::reduction_a_l_urssaf`, la même lecture pour le bordereau courant et pour
celui d'une correction, depuis le 10/10/2026). Une correction qui fait
**baisser** la réduction l'écrit sous son CTP de régularisation, 669 (801 pour
le 671 d'un technicien intermittent), en assiette (23.004) et sans signe, et
non en montant négatif (guide § 8.2, schéma ; `reduction_generale.dsn.ctp_regularisation`
du barème, depuis le 11/10/2026). Un mois qui porte un
permanent sans aucun CTP marqué `"nature": "permanent"` **refuse** : le
bordereau est un ordre de prélèvement.

Le CTP 983 de l'intermittent devient conditionnel (`"nature": "intermittent"`) ;
les compléments abattus 432 et 636 ne portent que le brut des **artistes**
(`"statut": "artiste"`), et un technicien intermittent relève des compléments au
taux plein 430 et 635 (`"statut": "technicien"`, depuis le 11/10/2026 : « Les
techniciens ne sont pas concernés » par les taux réduits, page Urssaf déposée dans
`atelier/reference/sources/urssaf-taux-reduits-artistes/`). Une ligne de
`ctp_urssaf` peut dépendre de l'effectif (`effectif_au_moins`, `effectif_sous`, lus
par `dsn::Seuils` dans `config/effectifs.json`) : le FNAL des artistes passe du 334
plafonné au 316 « totalité » à cinquante salariés. L'assiette de la taxe d'apprentissage se
compose — brut majoré de 10 % pour les intermittents, brut nu pour les
permanents, un seul CTP 992.

### Ce que la loi doit à un permanent

`logiciel/donnees/permanents-base.json` porte les nombres, chacun avec son article ;
`droit/src/permanents.rs` porte la règle ; les routes portées d'`operations_permanents.py`
(`moteur/src/routes_actes_h1.rs`, `routes_listes.rs`)
décident. Les articles sont relevés et datés dans
`atelier/reference/sources/code-du-travail/`.

- **congés payés** : 2,5 jours ouvrables par mois, 30 au plus (L3141-3) ;
  période de référence au 1er juin (R3141-4 — le 1er avril du même article
  est celui des *caisses* de congés, donc des intermittents : les deux dates
  cohabitent dans une compagnie de spectacle) ; indemnité **calculée deux
  fois**, le dixième (L3141-24 I) et le maintien (II), le plus favorable
  retenu ; indemnité compensatrice à la fin, **due quelle que soit la cause
  de la rupture** (L3141-28). Le compteur se **dérive des saisies**, jamais
  stocké — même règle que les 507 heures d'un intermittent ;
- **précarité** : 10 % (L1243-8), **pas due pour un contrat d'usage**
  (L1243-10 1°, qui renvoie au 3° de L1242-2) — c'est pourquoi aucun des 146
  contrats de l'historique n'en porte ;
- **préavis** : L1234-1, et **rien sous six mois d'ancienneté** — la loi y
  renvoie à la convention ou aux usages, et le moteur demande plutôt que
  d'inventer ;
- **indemnité de licenciement** : huit mois d'ancienneté (L1234-9), un quart
  puis un tiers de mois (R1234-2), salaire de référence le plus favorable des
  deux formules (R1234-4). Un montant **supra-légal est refusé** : son régime
  social est autre, et le moteur ne sait pas l'assujettir ;
- **les trois autres fins d'un CDI**, depuis le 19/09/2026 — et c'est le
  **motif de rupture** qui choisit, jamais la ressemblance :
  - **rupture conventionnelle** (motif 043) : son indemnité spécifique ne
    peut pas être inférieure à celle du licenciement (L1237-13), et c'est
    tout ce que la loi dit — le montant est celui de la **convention
    signée**, il se saisit. Le moteur calcule le plancher, refuse en dessous,
    et **n'écrit rien** tant que le montant n'est pas dit. Pas de préavis :
    la convention *fixe* la date de rupture ;
  - **mise à la retraite** (038) : au moins l'indemnité de licenciement
    (L1237-7), préavis de L1234-1 (L1237-6). La **condition d'âge n'est pas
    vérifiée** — L1237-5 renvoie au code de la sécurité sociale — et si elle
    n'est pas remplie, « la rupture constitue un licenciement » (L1237-8) :
    le moteur le rappelle à chaque fois ;
  - **départ à la retraite** (039) : **un palier, pas un cumul** (D1237-1).
    Un demi-mois après dix ans, un mois après quinze, un et demi après vingt,
    deux après trente — douze ans valent un demi-mois, pas trois —, et
    **rien du tout sous dix ans**. Salaire de référence de D1237-2, les deux
    mêmes formules que R1234-4 ;

  **Ce qu'elles supportent ne se lit pas dans le code du travail**, qui n'en
  dit pas un mot : le régime social sort de **L242-1 II 7° du code de la
  sécurité sociale**, et lui-même renvoie au régime fiscal de **l'article 80
  duodecies du code général des impôts** — est exclu des cotisations, dans la
  limite de deux plafonds annuels, ce qui n'est pas imposable. Les deux
  articles ont été relevés le 19/09/2026, par le même moissonneur que le code
  du travail (aujourd'hui `moteur releve-codes`, qui lisait dès lors trois codes en
  un passage). Il en sort trois choses :
  - l'indemnité **légale** de licenciement et celle de mise à la retraite ne
    sont **ni soumises ni imposables** (80 duodecies 3° b et 4° b, qui
    exonèrent « le montant prévu […] par la loi ») ;
  - l'indemnité de **départ volontaire** à la retraite, hors plan de
    sauvegarde de l'emploi, n'est dans aucune exonération : elle est
    **imposable, donc soumise**, et se traite comme un salaire. C'est la
    seule des quatre ;
  - celle de **rupture conventionnelle** dépend d'un fait du *salarié* :
    l'exonération d'impôt du 6° ne joue que s'il **n'est pas en droit de
    bénéficier d'une pension**. S'il l'est, elle est imposable — et reste
    hors cotisations (L242-1 II 7°, second alinéa, « y compris lorsqu'elles
    sont imposables »). D'où **deux rubriques** pour une seule indemnité, et
    un moteur qui refuse de produire tant que le fait n'est pas dit.

  Et ce que le moteur **ne** fait pas : la **contribution patronale de 40 %**
  sur la part exonérée d'une rupture conventionnelle ou d'une mise à la
  retraite (L137-12). Elle est due, aucune section du barème ne la porte, et
  le moteur le dit à chaque fois plutôt que de l'inventer. Son taux **se
  lit** dans `permanents-base.json` : il était de 30 % depuis 2023, la LFSS
  2026 l'a porté à 40 % au 31/12/2025, et c'est une livraison quotidienne de
  la DILA qui l'a appris au moteur — l'archive globale de LEGI, datée du
  13/07/2025, porte encore l'ancien.

  Chacune a **son** code d'indemnité au bloc S21.G00.52 — 001, 003, 005 —,
  et le cahier technique interdit de les intervertir (CCH-21 à CCH-24). Le
  contrôle est au point d'écriture du bloc, dans `dsn.rs` (`permanents::code_52_permis`) : c'est le seul
  endroit par où passent toutes les lignes. Avant, le moteur ne connaissait
  que le 007 et le posait sur **toute** fin de CDI — une rupture
  conventionnelle serait partie avec, et CCH-24 l'aurait fait rejeter ;
- **suspensions** (bloc S21.G00.65) : congé sans solde, parental, sabbatique…
  Saisies une fois comme **imprévus** du salarié (`engagement/src/imprevus.rs`) : chaque mois
  traversé en tire sa ligne et son bloc 65, entier, sans recopie. Un **congé payé n'en est pas une** — il est payé, le contrat n'est pas
  suspendu, et le moteur refuse d'écrire le motif « 200 - COP ».

  **Un mois entier sans rémunération** (15/10/2026 ; textes et rapport :
  `atelier/reference/sources/suspension-non-remuneree/`) :
  - la **retenue** d'une absence (arrêt comme suspension) se calcule **au réel du mois** : le salaire du mois
    fois les heures d'absence sur les heures de travail du mois (Cass. soc. 11/02/1982), au taux exact,
    jamais arrondi avant de multiplier ; les heures se comptent au réel, jours ouvrés (lundi à vendredi,
    moins les fériés chômés) fois la durée hebdomadaire sur cinq (fiche DSN 2640 : l'hypothèse « classique »
    quand le planning n'est pas connu ; fiche 2693 : septembre 2026 entier = 22 × 7 = 154 h). Un mois
    entier retire le salaire entier : le brut est 0,00, l'activité rémunérée (53 type 01) 0.00, l'absence
    (53 type 02) les heures réelles ;
  - la **complémentaire santé** : en 3090 les garanties sont suspendues (accord du 19/07/2024, art. 4.2) —
    pas de mutuelle sur le bulletin, pas de rattachement santé dans la DSN ce mois-là (inférence pour la DSN :
    aucune fiche ne le dit) ; ailleurs, la fiche entreprise dit ce que fait le contrat de santé,
    `sante_suspension_non_indemnisee` = « suspendue » ou « maintenue », et sans réponse le mois refuse
    (`calcul::sante_de_la_suspension`) ;
  - un **net fiscal négatif** (une part patronale de santé maintenue et réintégrée) se déclare tel quel en
    50.002, et le montant soumis au PAS (50.013) comme le PAS (50.009) à 0.00 (fiche DSN 1810,
    `pas::du_versement`) ; le net social négatif s'affiche (BOSS, FAQ Q37) ;
  - les **congés payés** ne s'acquièrent pas sur les jours d'une suspension qui n'acquiert pas
    (`acquiert_conges`, L3141-5 ; `imprevus::jours_de_conges_non_acquis`) ; l'**ancienneté** compte pour moitié
    le congé parental (L1225-54) et ne compte pas, en l'avertissant, une autre suspension non rémunérée
    (`imprevus::anciennete_depuis`).

- **jours fériés** : les onze fêtes légales de L3133-1, dont trois qui
  dépendent de Pâques — le seul calcul du logiciel qui remplace un relevé,
  aucune table officielle ne publiant cette date. Mais **férié n'est pas
  chômé** : la loi n'impose que le 1er mai (L3133-4), les dix autres relèvent
  de la convention ou de l'usage. `socle/src/feries.rs` ne retire donc du
  décompte des jours ouvrables que ce que `config/entreprise.json` déclare
  chômer (`jours_feries_chomes`), et **dit** ce qu'il a compté comme
  ouvrable. À noter : L3133-3 exclut nommément les **intermittents** du
  maintien de salaire un jour férié ;
- **maintien de salaire** pendant un arrêt (L1226-1, D1226-1 à D1226-8) : un
  an d'ancienneté appréciée au premier jour de l'absence, sept jours de
  carence — **aucune** pour un accident du travail, mais l'accident de
  *trajet* garde la sienne —, trente jours à 90 % puis trente aux deux tiers,
  allongés de dix jours par tranche de cinq ans et plafonnés à quatre-vingt-dix,
  le tout imputé sur douze mois glissants. Le moteur rend le **brut avant
  déduction** : les indemnités journalières viennent de la caisse et se
  saisissent (D1226-5) ;
- **certificat de travail** (L1234-19, D1234-6) et **reçu pour solde de tout
  compte** (L1234-20, D1234-7, D1234-8) : produits par
  `moteur/src/routes_actes_h1.rs` (porté de `documents_fin.py`), leur texte en donnée dans `gabarits/*.modele`.
  Le certificat est le seul document du logiciel dont le contenu soit
  **plafonné** par la loi — D1234-6 dit « exclusivement ».

Ce qui n'est **toujours pas** calculé est dit plutôt que deviné : les
périodes assimilées à du travail effectif (L3141-4 et L3141-5 sont relevés,
mais pas L3141-5-1 — les deux jours par mois de la loi du 22/04/2024), le
prorata d'un mois incomplet (fait aux jours calendaires, jamais confronté à
un bulletin de référence), la rupture conventionnelle et les départs en
retraite, et les clauses de la convention 3090.

## Les signalements d'événements (DSN événementielle)

Le moteur en écrit quatre, dans trois modules :

| `S20.G00.05.001` | message | écrit par |
|---|---|---|
| `04` | signalement d'arrêt de travail | `declarations/src/signalement.rs` |
| `05` | signalement de reprise suite à arrêt de travail | `declarations/src/signalement.rs` |
| `07` | **fin de contrat de travail unique** (FCTU) | `declarations/src/fctu.rs` |
| `08` | **amorçage des données variables** (SADV) | `declarations/src/amorcage.rs` |

**Trois modules et non un**, parce qu'ils ne portent pas la même chose. Les
deux premiers ne portent pas de paie ; le FCTU ne porte que ça — il déclare le
**solde de tout compte** (cahier technique p. 107, « y compris les données du
mois en cours calculées dans le cadre du solde de tout compte »), et il ne
pouvait donc pas s'écrire avant la paie des permanents. Le quatrième ne porte
ni paie ni événement de contrat : il **demande** — c'est la seule façon
d'obtenir de la DGFiP le taux de prélèvement à la source d'un salarié sans
attendre une DSN mensuelle, depuis que TOPAze est fermé. Un signalement
d'amorçage vaut pour **un unique individu** (base de connaissances DSN, fiche
2117) : cinq salariés à amorcer, c'est cinq fichiers.

**Le bloc 40 est le même dans les quatre**, et c'est voulu : `fctu.rs` et
`amorcage.rs` appellent `dsn::bloc_contrat` (ou `bloc_contrat_fctu`) avec leur nature de déclaration,
et `si_admise` retire ce que cette nature n'admet pas **en lisant le tableau
des usages** de la version du fichier (`usages-dsn-base.json` pour P26V01,
`usages-dsn-p27v01-base.json` pour P27V01) au lieu de recopier la
différence. Une seule vérité sur la façon de décrire un contrat. Ce que la
nature 08 retire par rapport à une mensuelle : le spectacle (`40.049`,
`40.050`, `40.051`), l'accident du travail (`40.039`, `40.040`, `40.043`),
l'identifiant technique d'affiliation (`70.012`) et le bloc 71 — et à
l'inverse, elle **exige** le code APET (`11.002`), que le signalement d'arrêt
interdit — en P26V01 : la norme P27V01 le retire.

**Un CDDU du spectacle n'a pas besoin de FCTU** : la même page donne une
dérogation aux artistes et techniciens du spectacle, et la fin d'un contrat
d'usage part dans la **DSN mensuelle** avec le circuit dérogatoire
`S21.G00.62.017 = '01'` — une rubrique justement **interdite** dans un FCTU.
Le moteur en émet un quand même si le salarié le demande (la norme le permet,
p. 68 et p. 107), en le disant par un avertissement (`fctu.rs`, « La
dérogation du spectacle ») ; la mensuelle qui suit écrit alors encore
62.017 = 01 (fiche N5, 62.017 : une décision ouverte).

**Ce que l'arborescence tranche, et que le tableau des usages ne dit pas.**
Le tableau dit qu'une rubrique a le droit d'être là ; il ne dit pas si son
bloc s'applique. Les cardinalités du modèle (p. 122, recopiées dans
`moteur/fctu-base.json`) le disent :

```
S21.G00.30 - Individu               (1,1)   UN individu, UN contrat :
S21.G00.40 - Contrat                (1,1)   c'est le mot « unique » du nom
S21.G00.62 - Fin du contrat         (1,1)   obligatoire
S21.G00.63 - Préavis                (1,*)   OBLIGATOIRE AUSSI — même sans
                                            préavis, code « 90 »
S21.G00.78 / 81 - bases, cotisations (0,*)  facultatives
```

Et deux contrôles lui sont propres, tenus par le moteur avant tout dépôt :
**CCH-14** (la valeur 31 — base assujettie de prévoyance — interdite en
`S21.G00.78.001`) et **CCH-15** (au moins trois blocs 51, types 001, 002 et
010, par versement).

Déposé en test le 19/09/2026 : **0 anomalie bloquante, 0 non bloquante**,
certificat de conformité délivré.

**Un signalement d'arrêt n'est pas une petite DSN mensuelle.** Il ne porte ni
paie, ni cotisation, ni versement, ni bordereau. (Un FCTU, lui, porte la paie
du solde de tout compte — mais pas plus de bordereau ni d'ordre de
télépaiement : les blocs 20, 22 et 23 lui sont interdits aussi.) Son arborescence tient en huit blocs —
envoi, déclaration, contact, entreprise, établissement, adhésions prévoyance,
individu, contrat, arrêt — et le *tableau des usages* du cahier technique
(2026.1, p. 363-371) **interdit** dans une nature 04 ou 05 la quasi-totalité
des rubriques qu'une mensuelle écrit : le code APE, l'adresse de l'entreprise,
le lieu et le pays de naissance du salarié, son adresse, et tout le bloc 40
sauf trois rubriques.

```
S21.G00.40.001   date de début du contrat      O
S21.G00.40.009   numéro du contrat             O
S21.G00.40.019   identifiant du lieu de travail  C
tout le reste du bloc 40                       I  (interdit)
```

Le contrat est donc **désigné**, jamais redécrit : c'est la DSN mensuelle qui
l'a déclaré, et ce sont ces deux identifiants-là qui doivent correspondre.
D'où `operations_arrets.contrats_au()`, qui lit les contrats dans les saisies
plutôt que d'en fabriquer un.

Une rubrique interdite écrite par mégarde, c'est une anomalie bloquante
**CST-04** côté plate-forme, et le fichier ne déclare rien. Le tableau des
usages est donc relevé dans `logiciel/donnees/arrets-base.json` (section `usages`), et
`tests/test_arrets.py` confrontait chaque fichier produit à ce tableau —
rubrique par rubrique, dans les deux sens (aucune interdite, toutes les
obligatoires). Le générateur, lui, ne lit pas cette section : sinon le test ne
vérifierait plus rien. Ce test était du Python : il est parti avec lui sans être
porté, et plus rien ne lit cette section ; le test Rust des signalements
(`tests/dsn/signalement.rs`) fige le texte de chaque cas.

### Le bloc 60, et ses trois visages

Le même bloc « Arrêt de travail » s'écrit à quatre endroits, avec quatre jeux
de rubriques — et c'est `dsn::bloc_60` qui les tient tous, pour qu'il n'y ait
pas deux vérités sur ce qui part. Il décide de ce qu'il écrit par le tableau
des usages, pour la nature du fichier et jamais une autre (`usage_admet` ;
`atelier/reference/norme-dsn/texte/tableaux-des-usages.txt`, bloc S21.G00.60 ;
reste R1 #42 : le FCTU passait « 01 »).

| rubrique | mensuelle (01) | arrêt (04) | reprise (05) | fin de contrat (07) |
|---|---|---|---|---|
| `60.001` motif de l'arrêt | O | O | O | O |
| `60.002` dernier jour travaillé | O | O | O | O |
| `60.003` fin prévisionnelle | O | O | O | O |
| `60.004` subrogation | C | **O** | **I** | **I** |
| `60.005/.006` dates de subrogation | C | C | I | I |
| `60.007/.008` IBAN / BIC | C | C | I | I |
| `60.010` date de la reprise | C | C | **O** | C |
| `60.011` motif de la reprise | C | C | **O** | C |
| `60.012` date de l'accident | **I** | C | C | **I** |

Trois conséquences qui ne se devinent pas :

- la **subrogation est obligatoire** dans un signalement d'arrêt. Elle ne se
  déduit de rien : répondre *oui* engage la compagnie à percevoir les
  indemnités journalières et à maintenir le salaire. Le moteur refuse un arrêt
  saisi sans réponse ;
- la **date de l'accident** est interdite en DSN mensuelle. Elle ne part que
  dans le signalement, et seulement pour les motifs 04, 05 et 06 (CCH-11) ;
- la **date de reprise** ne se renseigne que si la reprise est **anticipée**
  par rapport à la fin prévisionnelle. Connue mais non anticipée, *« elle ne
  doit pas être valorisée »*.

### Le report dans la DSN mensuelle

Cahier technique §2.5.2 : *« il est nécessaire de reporter les informations
d'un signalement dans la DSN mensuelle consécutive »* — la mensuelle est la
« photo » de fin de mois, et c'est elle qui constitue l'historique de vie
professionnelle sur lequel s'exploitent les signalements suivants. Et bloc
60 : *« en cas de suspension supra-mensuelle, la suspension doit être déclarée
dans TOUTES les DSN mensuelles dont le mois principal déclaré présente une
période commune »*.

`dsn.generer(..., arrets=…)` le fait : le bloc 60 s'écrit sous le bloc 40 du
contrat suspendu, entre le contrat et la fin de contrat (l'ordre de
l'arborescence : 41, 60, 62, 65, 70, 71). `operations_paies.produire_mois` lit
le magasin tout seul et l'annonce dans les avertissements — y compris quand
l'arrêt n'a **jamais été signalé**, parce que la mensuelle ne remplace pas le
signalement : elle le répète.

Si le contrat suspendu n'est déclaré dans aucune paie du mois, la DSN
**refuse** plutôt que de laisser tomber l'arrêt en silence : le bloc 60 est
fils du bloc 40, il n'aurait rien où s'accrocher.

### Le magasin, et la trace des envois

`imprevus/AAAA-MM.json` sous les données (depuis le 15/10/2026, `engagement/src/imprevus.rs`) :
le magasin des **imprévus** d'un salarié, arrêts de travail (`"genre": "arret"`, au mois du
dernier jour travaillé) et suspensions du bloc 65 (`"genre": "suspension"`, au mois de leur
premier jour). Un imprévu se saisit une fois ; le moteur en tire la ligne de paie d'un
permanent (`imprevus::lignes_du_mois`, le seul endroit qui la tire), le bloc 60 ou 65 de chaque
DSN dont le mois le croise, et nomme les cachets d'un intermittent datés pendant l'imprévu.
L'ancien dossier `arrets/` n'est plus lu : sa présence refuse. Chaque arrêt garde ses `envois` : les
signalements déposés pour de vrai (nature, type, numéro d'ordre, identifiant
rendu par le point de dépôt) et les DSN mensuelles qui l'ont reporté. C'est
cette liste qui donne son rang au prochain annule-et-remplace — le cahier
technique (§1.4.3.4) exige une séquence croissante par événement — et un
dépôt de **test** n'y écrit rien : il n'a rien déclaré.

Le dépôt passe par le même `depot-dsn` qu'une mensuelle. Deux adaptations
seulement : un signalement n'a pas de mois principal déclaré (la rubrique y
est interdite), donc c'est le **jour de l'événement** qui s'affiche et se
recopie ; et le garde-fou « une DSN réelle est déjà partie pour ce mois » ne
s'applique pas — plusieurs signalements peuvent partir le même mois, un par
arrêt.

## La DPAE (ex-DUE)

Enregistrements de 990 caractères, **saut de ligne compris** : 989 caractères
puis `LF`, comme le dit le Format 120 (« 990 en incluant le saut de ligne »).
Un enregistrement par contrat, encodage latin-1. Zones dans `declarations/src/dpae.rs` :

| Position | Contenu |
|---|---|
| 121-133 | horodatage de l'envoi, `JJMMAAAAHHMM` — **sans secondes** : le dessin officiel (Format 120) place `C_URSSAF` juste après |
| 133-136 | `C_URSSAF`, code de l'Urssaf réceptrice (117 Île-de-France), lu dans `entreprise.json` → `code_urssaf` |
| 283-315 | nom (32 c.) |
| 315-347 | prénom (32 c.) |
| 347-360 | NIR (13 c., sans la clé) — **facultatif** : à blanc, la DPAE vaut demande d'immatriculation (formalité `IMMA`, transmise à la CPAM). C'est par elle que le NIR arrive, et la DSN déclare l'individu sous son NTT en attendant (`declarations/src/ntt.rs`) |
| 360-368 | date de naissance `JJMMAAAA` |
| 368-412 | lieu de naissance (44 c.) |
| 412-420 | date d'embauche `JJMMAAAA` |
| 420-424 | heure d'embauche `HHMM` |
| 730-731 | sexe `M` / `F` — lu sur le NIR quand il y en a un, sur la case `sexe` de la fiche sinon (`ntt.sexe`, la seule lecture du sexe dans le moteur) |
| 827-829 | département de naissance |
| 864-865 | `C_TYPE_CONTRAT` : `1` CDD, `2` CDI, `3` CTT — **la DPAE n'en connaît que trois**, quand la DSN en a une vingtaine. La valeur vient de la nature du contrat (`moteur/contrats-base.json`, section `dpae`) |
| 865-873 | `D_FIN_CDD`, fin prévue du CDD `JJMMAAAA` — **à blanc pour un CDI**, qui n'a pas de terme |

Le reste est constant : marqueur `DUE`, `912` (N_LG_MESS, la longueur du message hors entête — pas un code Urssaf), `120` (C_VERSION), SIRET, raison
sociale, adresse, téléphone, code NAF, et `MT176` (code du service de santé au
travail). Ces zones viennent du gabarit et ne sont jamais réécrites.

**Niveau de preuve — plus faible qu'ailleurs, à savoir.** CoolPaie n'a laissé
qu'**un seul** fichier DPAE dans tout le dossier
(relevé sur une compagnie réelle, pièce hors du dépôt public : dépôt privé ; 6 enregistrements du
28/08/2026, deux salariés × trois contrats). Les 6 sont reconstitués octet pour
octet, et l'audit des zones montre que tout segment non vide est soit un champ
identifié, soit une donnée d'entreprise stable. Mais aucune variation n'a pu
être observée sur : un autre établissement, un autre service de santé au
travail, une heure d'embauche autre que 19h00.
Pour l'usage de la compagnie — mêmes artistes, mêmes CDD d'usage — c'est couvert.

**Le contrat autre que CDD, lui, n'est plus une supposition** (19/09/2026). Le
`1` en position 864 était décrit ici comme codant « probablement durée
déterminée » ; c'est `C_TYPE_CONTRAT`, et le dessin officiel (catégorie V
« CONTRAT ») donne les trois valeurs. Il sortait en dur du gabarit : un CDI
embauché serait parti déclaré à durée déterminée, avec un terme recopié de
l'engagement. Ce qu'il n'a **toujours pas**, c'est un dépôt réel ou de test en
face : la DPAE d'un permanent est conforme au dessin et aux tests, pas à une
réponse de l'Urssaf.

**Ce que l'Urssaf répond** (onglet « Codes retour » du Format 120, tables dans
`declarations/src/dpae.rs`) : le bilan de conformité (nature 11, XML) porte un `<code>`
par anomalie, et le certificat de conformité (nature 41) est **notre
enregistrement renvoyé** avec ses zones de retour remplies — `C_RET_AR_LOT`
(76-77, le traitement du message), `R_DOSSIER` (425-429, la référence sous
laquelle la DPAE est enregistrée), `C_RETOUR_AR` (430-431, le code par DPAE)
et les formalités déclenchées (`I_CERTIF`, `I_IMMA`, `I_MT`, `I_PMF5`,
971-976). `envois/src/etats.rs` lit le certificat (`dpae::lire_certificat`) et pose sur
chaque embauche sa référence de dossier.

Deux lectures à connaître, observées sur les 16 enregistrements reçus de juin
au 12 septembre 2026, ceux de CoolPaie compris :

- **`99` « autre erreur, données facultatives » est l'état nominal**, toujours
  avec un `R_DOSSIER` et `CERTIF` + `MT` levées. Le tableau de 2013 est plus
  sévère que le service ; une DPAE en `99` avec un dossier est enregistrée.
- **`98` « DPAE déjà existante »** : l'Urssaf n'a rien créé (formalités à `N`)
  parce qu'une DPAE identique existait — vu sur le second test des mêmes
  embauches. L'embauche est déclarée, par la première.

Le moteur écrit le `.txt` **et** le `.txt.zip` du même nom, comme le faisait
CoolPaie ; net-entreprises accepte les deux. `DPAE_<mois>_recapitulatif.txt`
donne la même chose en clair pour la saisie en ligne, qui reste une option
raisonnable pour deux personnes.

## Ce que le rejeu couvrait (`calibrer`, retiré avec le Python)

```
DSN juillet 2026 ......... identique
DSN juin 2026 ............ identique
DSN mars/avril/mai 2026 .. contrats à l'heure, régularisation annuelle S21.G00.82
DSN janvier/février 2026 . structure de blocs antérieure
DSN 2021-2025 ............ non rejouées : barèmes de ces années absents
AEM 2026 ................. 10 fichiers sur 10 identiques octet pour octet
```

Les DSN de 2021 à 2025 ne se rejoueront pas, et c'est décidé (16/09/2026) :
un essai sur mars 2025 a montré que les écarts sont d'abord de **norme**
(P25 : pas de bloc 15 ni 31, pas de 78.006, pas de codes 102/907/072/079),
pas de barème — les rejouer octet pour octet demanderait à `dsn.py` d'émuler
cinq versions de la norme pour des déclarations passées et acceptées. Ce
n'était utile que pour se rassurer, pas pour produire.

## Le dépôt, et le journal

Ces déclarations engagent l'employeur. Le moteur les dépose **par l'API**
(`envois/src/depot.rs` le prépare, `envois/src/transport.rs` l'envoie : net-entreprises
pour la DSN, l'Urssaf pour la DPAE), jamais
par un portail : un contrat écrit, des retours lus par programme, et une
erreur qui se voit — la première DPAE du 12/09/2026 a été refusée cinq fois
pour un code Urssaf faux que le portail DUE de CoolPaie laissait passer.

Trois garde-fous, tous dans le moteur (`moteur/src/depot_acte.rs`, porté d'`operations_declarations.py`, sur la preuve du socle) :

- **Une preuve par acte.** Rien ne part en réel sans le mot de passe du
  logiciel tapé au moment de l'acte, et les trois gestes (`README.md`).
  Sans mot de passe posé, rien ne part du tout (17/09/2026) : le mois
  recopié ne l'a jamais remplacé, et depuis le 21/09/2026 il ne se recopie
  plus — il s'affiche. Plus de « séance » depuis le 16/09/2026 : chaque acte
  porte sa preuve, rien entre les actes.
- **Pas deux fois.** `produire_dpae` refuse une embauche déjà déclarée et non
  refusée (`DpaeDejaDeclaree`), `deposer_dsn` une DSN réelle déjà partie pour
  le mois (`DsnDejaDeposee`). Une déclaration **refusée** repart sans forcer —
  c'est la seule raison de la refaire.
- **Un dépôt n'est pas fini quand il est parti.** `envois/src/journal.rs` écrit une ligne
  par acte sortant dans `journal/depots.json` (dépôt, courriel, demande de
  signature), avec le `flux` rendu par le point de dépôt ; les retours relevés
  à l'ouverture s'y rattachent. `verdict_du_retour` (`envois/src/etats.rs`) lit les bilans (natures
  10/11) : une DPAE est **passée quand son bilan est OK**, le certificat 41
  confirme, il ne décide plus. `resume` dit « REFUSÉ : motif », « bilan OK »,
  « en TEST ».

Déposer la DSN en test avant le dépôt réel, et garder l'ancien logiciel installé le temps
de faire tourner un ou deux mois en parallèle et de comparer.

### Un logiciel, un administrateur (ticket #244, 22/10/2026)

Une installation (le dossier qui porte `logiciel/`, `prive/`, `compagnies/*`, `fictives/*`) appartient à une personne,
l'**administrateur**, qui peut y gérer plusieurs compagnies d'un seul mot de passe ; qui veut deux administrateurs
installe un second logiciel. `moteur/src/administrateur.rs` :

- **Rien de neuf dans le chiffrement.** Chaque compagnie garde son coffre (`config/coffre.bin`) et sa clé maîtresse, scellée
  par la clé du poste et par le mot de passe du logiciel. Rattacher une compagnie, c'est sceller cette enveloppe par le mot
  de passe de l'administrateur ; la détacher, la resceller par un mot de passe à elle. Le **registre**
  (`administrateur.json`, à la racine de l'installation, sans aucun secret) dit lesquelles s'ouvrent ensemble.
- **Changer le mot de passe de l'administrateur** rescelle toutes ses compagnies : tout se prépare
  (`Coffre::preparer_ouverture`), puis tout se remplace ; un échec au milieu remet chacune dans son état d'avant.
- **La fenêtre** demande « Gérer cette compagnie avec le même administrateur ? » à la création d'une compagnie quand une
  autre a déjà un mot de passe ; retient le mot de passe de l'administrateur, une fois tapé, pour ouvrir ses autres
  compagnies sans le redemander (il ouvre, il n'autorise aucun acte) ; et l'accueil devient son **tableau de bord** : les
  déclarations obligatoires de ses compagnies (les rappels de chacune : DSN, signalements, obligations d'`obligations.rs`,
  DPAE), du plus pressé au moins pressé, lues de `config/.tableau.json` sans calcul, refaites en fond
  (route `administrateur-tableau`).
- **Le dépôt groupé** (route `administrateur-deposer`, `{"reel"}`) : ce qui part par la DSN et qui est prêt — en test, un
  fichier écrit pas encore essayé ; en réel, un fichier dont le test est revenu conforme — part compagnie par compagnie
  par la route `deposer` de chacune, avec ses contrôles et son bilan. En réel, un mot de passe tapé au moment de l'acte sert
  de preuve à chaque dépôt, qui le revérifie. Une compagnie qui refuse n'arrête pas les autres. Le bouton n'apparaît qu'à
  partir de deux compagnies qui ont quelque chose de prêt.
- **Une fenêtre par installation** (`interface/instance.rs`) : deux installations s'ouvrent côte à côte sur un poste (la
  clé USB arrive sur un poste qui a déjà son logiciel) ; la même, relancée, ramène sa fenêtre devant. Chacune tient un
  **créneau** de la plage 7330-7429 : un verrou (une prise d'écoute, rendue par le système à la mort du processus) et, au
  port suivant, sa page. Le créneau est **retenu** dans `.fenetre.json` à la racine de l'installation (il voyage avec
  elle) ; à défaut, tiré d'une empreinte de son chemin. Pourquoi un port stable : le navigateur retient un mot de passe
  par origine (`hôte:port`). Un créneau tenu par une autre installation (son verrou répond par son dossier, un autre),
  par une copie de celle-ci (même `.fenetre.json`, autre dossier) ou par un autre programme se saute, et le suivant est
  retenu ; aucun créneau libre : la fenêtre s'ouvre quand même, sans verrou. Ce qui est propre à une fenêtre sur le poste
  (le profil de WebView2/WebKit ou du Chromium, le jeton de sa page, son journal d'avant toute compagnie) va dans
  `<profil>/fenetres/<port>/` — en portable, dans `.fenetre/` de l'installation ; le cookie du jeton porte le port
  (`jeton-<port>`). Ce qui reste commun aux installations d'un poste : la clé du poste (hors mode portable), sans danger —
  elle ne scelle que des enveloppes, chacune d'une clé maîtresse tirée au hasard pour sa compagnie, et se pose une fois
  (`O_EXCL`) ; la séance gardée est **une par coffre** (`ouverture-gardee-<empreinte>.bin`). La mise à jour est déjà
  par installation (`maj/`) ; son relais attend la fin de **sa** fenêtre (le processus sous Windows, le créneau de son
  installation ailleurs), et `moteur maj --appliquer` ne refuse que si **cette** installation est ouverte. Ce que la fenêtre
  d'avant laissait sur le poste et que plus rien ne lit — la séance gardée au nom unique (`ouverture-gardee.bin`), le
  `.jeton-interface` et le profil `navigateur/` communs, le profil WebView2 par défaut de Tauri (`fr.puppetpaie.fenetre`) —
  s'efface au lancement, sans être lu (`interface::effacer_les_restes`).
- **Toutes les compagnies tenues à jour dès le lancement** (`Fenetre::prechauffer`, `etat_garde::Fil`) : chaque vraie
  compagnie de l'installation (pas les fictives) reçoit sa file des états sur **un seul fil de calcul** pour toute la
  fenêtre, la dernière ouverte d'abord (la date de son `journal/interface.log`), puis chacune son tour ; la compagnie
  choisie passe devant dès son choix (pendant qu'on tape son mot de passe). L'état d'ouverture, le tableau (compagnie de
  l'administrateur, sous l'empreinte de l'ouverture) et les mois se refont en fond ; ouvrir sert un état frais sans
  attendre ; minuit passé, tout se revérifie. **Avant le mot de passe, du calcul local seulement** : le fil est marqué
  (`fil_de_fond`), et `garde_reseau` y refuse toute sortie, le coffre y refuse la clé du poste et toute dérivation de mot
  de passe. La veille (réseau, coffre, relève, sauvegarde) ne démarre qu'à l'ouverture d'une compagnie ; le tableau de
  l'accueil ne relève les comptes rendus qu'une fois le mot de passe de l'administrateur tapé dans la fenêtre.


## La version, et la mise à jour dans l'ombre

`VERSION` (une date, `AAAA.MM.JJ`, comparée en entiers) ; `moteur/src/maj.rs`
(porté de `moteur/maj.py`). **Une mise à jour par installation** (lot U1,
14/10/2026) : tout se passe dans `<installation>/maj/`, à côté de `logiciel/`,
et l'installation de l'éditeur reçoit comme les autres. Un dépôt est une
adresse qui sert `derniere.json` et les archives de `logiciel/`.

**Quatre archives de mise à jour par version** (Léandre, 15/10/2026) : une par système
(`windows`, `linux`, `mac` : les fichiers communs et les programmes de ce
système) et une pour tous (`tout-systeme`). Dans `logiciel/`, les programmes de
Windows sont à la racine (`puppetpaie.exe`, `WebView2Loader.dll`), ceux de Linux
et de Mac dans `linux/` et `mac/` (`puppetpaie`, `moteur`), posés par le flux
seulement (`maj::dossier_programmes_de`, `maj::systeme_du_fichier`).
`derniere.json` porte `version`, `notes`, `calcul`, `exigee`, et sous `archives`
une entrée par système publié (`archive`, `sha256`, `signature` ; celle de tous
dit en plus les systèmes qu'elle porte, `systemes`). « Publier » choisit ce qu'il
fabrique (Windows seul, Windows et Linux, ou les trois) ; une installation dont
le système n'est pas publié ne télécharge rien et garde sa version.

**La forme de l'archive de mise à jour est un contrat** avec toutes les
installations posées (ticket #246, 22/10/2026) : le contenu de `logiciel/` à sa
racine, rien d'autre. Le code déjà installé la déballe telle quelle dans
`maj/<version>/logiciel/` et y lit `VERSION` ; une autre forme (le 22/10,
`logiciel/` et `fictives/` dans chaque archive) coupe toutes les installations
de leurs mises à jour. `derniere.json` peut gagner une clé à sa racine (le
code d'avant ne lit que `version` et `archives`), jamais en perdre ni rien
gagner sous `archives` qui ne soit un système.

**Deux autres sortes d'archives, à côté.** L'**archive des fictives**
(`puppetpaie-<version>-fictives.zip`, `maj::archiver_fictives`), signée, que
`derniere.json` nomme sous `fictives` avec l'**empreinte** de leur contenu
(SHA-256 des fichiers livrés et de leurs noms, ni compression ni dates). Les
**archives d'installation** (`puppetpaie-<version>-<système>-installation.zip`,
Linux, Mac et tous ; `maj::archiver_installation`) : `logiciel/` et
`fictives/`, ce qu'on décompresse pour installer ; ni signées ni nommées,
comme l'installeur Windows.

**Les fictives se mettent à jour à part** (Léandre, 22/10/2026). Une
installation retient la livraison qu'elle a dans `fictives/.livraison.json`
(`empreinte`, et `posees` : le nom et l'empreinte de chaque fictive qu'elle a
posée) ; chaque fictive posée porte sa marque, `.livree.json` (son empreinte).
L'archive d'installation les porte, l'installeur Windows aussi (le flux les
pose avant, `moteur maj --marquer-fictives`). `maj::preparer_les_fictives` ne
télécharge l'archive des fictives que si l'empreinte publiée n'est pas celle
retenue (un dépôt de travail, qui porte `.git`, ne la reçoit jamais), la
vérifie comme une archive du logiciel et la range dans `maj/fictives-pretes/`.
Au lancement suivant, avant toute fenêtre, `maj::poser_les_fictives_pretes`
remplace ce que la livraison avait posé (un dossier dont le nom est dans
`posees` et dont la marque dit l'empreinte retenue ; sans livraison retenue,
un dossier au nom d'une fictive livrée dont `COPIE-DE-DEVELOPPEMENT.json` dit
« compagnie fictive <ce nom> »), joué ou non, photo d'origine comprise (elle
se reprend à la prochaine ouverture) ; retire ce qu'elle avait posé et ne
porte plus ; pose ce qui est nouveau. Un dossier qu'elle n'a pas posé n'est
jamais touché : si une fictive livrée en porte le nom, elle n'est pas posée,
et `maj/journal.log` le dit.

- `verifier` — au lancement, en fond : lit `derniere.json`, prend une version
  plus récente, télécharge l'archive de son système (`maj::SYSTEME`), ou celle
  de tous (si elle le porte) quand la sienne n'est pas nommée ou ne se
  télécharge pas, vérifie son
  condensat et sa **signature** de l'éditeur (Ed25519 ; une archive qui ne
  passe pas est refusée, sans repli), la déballe dans `maj/<version>/` (les
  programmes Unix redeviennent exécutables), la **contrôle** (son programme,
  `puppetpaie route`, lit la configuration de chaque compagnie de
  l'installation) et pose `maj/prete.json`.
- `appliquer` — au démarrage suivant, par un relais lancé avant toute
  fenêtre : `logiciel/` → `logiciel/.avant/`, la version prête prend sa
  place ; si ça casse en route, tout revient.
- `revenir` — `.avant/` reprend la place (`moteur maj --revenir`).
- `publier` — par le flux « Publier » du dépôt public, qui compile les
  systèmes choisis (le Mac sur `macos-latest`), signe et pose leurs archives,
  celle de tous et celle des fictives, les archives d'installation,
  `derniere.json` et l'installeur dans une Release (la clé du
  poste, `moteur maj --publier`, reste acceptée) ; jamais republier un numéro.


## L'exercice : rappels, clôture, export compta

Porté d'`operations_exercice.py` (16/09/2026) : `moteur/src/exercice_ecrit.rs` (la clôture,
l'export compta) et `moteur/src/routes_ouverture.rs` (les rappels). Trois choses qui regardent plus loin
que le mois affiché.

**Les rappels** (`rappels(ctx, aujourdhui)`) rendent une liste de `Rappel`,
dans l'ordre où ça se fait. Les mois dont la DSN manque d'abord : depuis le
mois qui suit la dernière DSN réelle passée au journal (ou en attente de son
bilan) jusqu'au mois d'avant aujourd'hui ; sans aucune DSN au journal, le
seul mois d'avant. Chaque rappel porte l'état du mois — `a_saisir` (pas de
fichier de paie : la DSN « néant » passe par l'écriture d'un mois vide),
`a_produire` (saisi, jamais écrit), `a_deposer` (le fichier DSN, choisi par
`fichiers_deposables`), `a_corriger` (bilan KO) — et `en_retard` passé
l'échéance de dépôt (`declarations/src/echeance_dsn.rs` : le 5 ou le 15 du mois suivant,
reporté au jour ouvré, à midi ; le rappel la rend dans `echeance_depot`). Puis, quand l'année d'avant n'est pas close et a des mois :
la clôture (seulement si la DSN de décembre est au journal — sinon c'est son
rappel qui parle), l'export compta (absent, ou plus vieux que la dernière
saisie de l'année), et au premier trimestre la revue des spectacles : lesquels
continuent ; les autres s'archivent (`revue_spectacles`, journal §167 : un
numéro d'objet ne se périme pas). Ce qui fait taire un rappel, c'est le
fait ; jamais un clic — ici, la revue enregistrée.

**La clôture** (`cloturer`) refuse une année déjà close, l'année en cours,
et une année dont un mois avec des paies n'a pas de DSN réelle acceptée
(`forcer` passe outre, et `exercices.json` le garde). Elle écrit l'export
compta si besoin, puis `sorties/exercices/Exercice_<année>.zip` : `engagements/` (le fichier de chaque mois de l'année, et celui du mois où commence un engagement qui court dans l'année),
chaque `sorties/<mois>/` en entier (les contrats signés sont dans `envoi/`),
les retours dont le nom commence par l'année ou par janvier suivant (le
bilan de la DSN de décembre arrive en janvier), `journal/depots.json`, les
fiches de `config/` et le barème de l'année, `compta/`, un `LISEZ-MOI.txt`.
Elle note dans `config/exercices.json` la date, l'archive, son empreinte
SHA-256 et le nombre de fichiers. **Figer** vit dans le socle
(`refuser_si_clos`) et est appelé par `enregistrer_saisie` et
`produire_mois` : un front qui l'esquiverait n'existe pas. `rouvrir_exercice`
retire la note et garde l'archive (avec une trace `_rouverts`).

**L'export compta** (`export_compta`) recalcule chaque mois depuis sa saisie
(`charger`) — les mêmes lignes, les mêmes arrondis que les bulletins — et
construit une écriture par mois, datée du dernier jour : au débit le brut
par spectacle (l'analytique) et la part patronale de chaque ligne de
bulletin ; au crédit un tiers par organisme (part patronale + part
salariale, la RGDU ventilée selon `Paie.reduction_generale`), le PAS s'il y
en a, et le net par salarié. Débit = crédit par construction (brut + charges
patronales = tiers + net), et l'écriture est refusée si ça ne tombe pas.
Les comptes viennent de `plan-comptable-base.json` (PCG : 641, 645x, 431,
437x, 421, 4421), recouvert clé par clé par `config/plan-comptable.json` ;
une ligne de bulletin sans compte fait refuser — on ne devine pas un compte.
Le fichier : `Compta_<année>.csv`, `;`, virgule décimale, UTF-8 avec BOM.
Pourquoi les bulletins et pas la DSN : le montant Urssaf de la DSN est un
bordereau recopié, pas un calcul, et les blocs 81 ne couvrent pas tout ce
que l'Urssaf facture — la comptabilité de la paie, c'est le bulletin ; le
prélèvement réel se rapproche ensuite du compte de tiers.

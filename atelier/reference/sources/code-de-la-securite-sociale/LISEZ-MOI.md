# Code de la sécurité sociale : le relevé LEGI

Les `articles-AAAAMMJJ.md`, `releve.json` et `xml/` de ce dossier sont **écrits par `paie releve-codes`** (`moissonner`, puis
`rapport`) depuis la base LEGI de la DILA ; ils ne se retouchent pas à la main. Les articles suivis sont la liste du texte
`securite-sociale` de `atelier/sources/paie/src/releve_codes/textes.rs`, où chaque domaine dit pourquoi il est là ; le dernier
relevé est le plus récent des `articles-*.md`. Ce fichier dit, par sujet ajouté depuis qu'il existe, ce qu'on en lit et où le
moteur s'en sert (les sujets plus anciens sont décrits dans les commentaires de `textes.rs` et dans les LISEZ-MOI des dossiers
qui les citent : `dfs/`, `plafond-intermittents/`, `majoration-caisses-conges/`…).

## Les signalements d'événement de la DSN et leur délai (relevé du 15/10/2026)

`articles-20261015.md`, section « Les signalements d'evenement de la DSN et leur delai » (`xml/R133-13.xml`, `xml/R133-14.xml`,
versions en vigueur depuis le 01/07/2026).

- **R. 133-13, I** : l'employeur « déclare également les événements suivants […] survenus au cours du mois considéré : 1° Le
  début et la fin de l'arrêt de travail […] ; 2° La fin du contrat de travail ».
- **R. 133-14, II** : ces événements se déclarent « au plus tard en même temps que la déclaration sociale nominative relative au
  mois au cours duquel cet événement est survenu » ; « Toutefois, sont déclarés dans un délai fixé par arrêté […], qui ne peut
  excéder cinq jours ouvrés, la fin du contrat de travail ainsi que, **lorsque l'employeur n'est pas subrogé** dans les droits de
  l'assuré aux indemnités journalières, le début et la fin de l'arrêt de travail ». Exceptions : 1° aucune retenue pour l'arrêt
  sur le salaire du mois où il débute ; 2° les fins de CDD d'usage (3° de L. 1242-2 du code du travail), sauf demande du
  salarié. Le délai est celui de l'arrêté du 2 juillet 2013 (`../arrete-2013-delai-evenements-dsn/`) : cinq jours ouvrés.
- **R. 133-14, IV, 1° et 2°** : la DSN tient lieu de l'attestation de salaire (R. 323-10, R. 441-4) et de l'attestation employeur
  (R. 1234-9 du code du travail).

**Où le moteur s'en sert** : `atelier/sources/paie/src/obligations.rs` et `logiciel/donnees/obligations-base.json`
(`signalement_arret`, `signalement_reprise`, `signalement_fin_contrat`).

## La déclaration d'un accident du travail ou de trajet, et celle d'une maladie professionnelle (relevé du 15/10/2026)

`articles-20261015.md`, sections « La declaration d'un accident du travail ou de trajet » et « La declaration d'une maladie
professionnelle (par la victime) ».

- **L. 441-2** : « L'employeur ou l'un de ses préposés doit déclarer tout accident dont il a eu connaissance à la caisse primaire
  d'assurance maladie dont relève la victime selon des modalités et dans un délai déterminés. »
- **R. 441-3** : « dans les quarante-huit heures non compris les dimanches et jours fériés », « par tout moyen conférant date
  certaine à sa réception » ; son second alinéa (le délai ne court que du jour où l'employeur a été informé) vise certaines
  catégories de L. 311-3, accidentées hors des locaux — L. 311-3 n'est pas relevé : le moteur compte de toute façon depuis la
  connaissance de l'accident par la compagnie (L. 441-2), ce qui couvre ce cas.
- **L. 441-1 et R. 441-2** : la victime informe l'employeur « dans la journée où l'accident s'est produit ou au plus tard dans
  les vingt-quatre heures ».
- **R. 441-6** : les réserves motivées de l'employeur, « dans un délai de dix jours francs à compter de la date à laquelle il
  l'a effectuée » ; **R. 441-7** : la caisse statue en trente jours francs, ou enquête (réserves reçues).
- **L. 411-1 et L. 411-2** : l'accident de trajet « est également considéré comme accident du travail » (motif d'arrêt 04).
- **L. 441-4** : le registre des accidents bénins (sans arrêt ni soins) ; **L. 441-5** : la feuille d'accident que l'employeur
  délivre ; **R. 441-1** ; **R. 441-4** : l'attestation de salaire, que la DSN remplit (R. 133-14, IV).
- **L. 461-5 et R. 461-5** : la maladie professionnelle se déclare « par les soins de la victime », dans les quinze jours de la
  cessation du travail. **Le motif d'arrêt 05 n'appelle donc pas de déclaration de l'employeur.**

**Où le moteur s'en sert** : `atelier/sources/paie/src/obligations.rs` (`de_l_arret`) et `logiciel/donnees/obligations-base.json`
(`declaration_accident`, `declaration_maladie_professionnelle`). Ce que disent le service et le formulaire :
`../accident-du-travail/`.

## Le congé de deuil : ses indemnités journalières et l'attestation de salaire (relevé du 15/10/2026, lot « le deuil »)

`articles-20261015.md`, section « Le conge de deuil d'un enfant : ses indemnites journalieres et l'attestation de salaire »
(`xml/L331-9.xml`, `xml/D331-7.xml`, `xml/R323-10.xml`) ; domaine `conge_de_deuil` de `paie/src/releve_codes/textes.rs`.

- **L. 331-9** (version du 31/12/2025, loi n° 2025-1403) : « Lorsqu'il exerce son droit au congé prévu à l'article L. 3142-1-1
  du code du travail, l'assuré perçoit, pendant la durée du congé et selon les mêmes conditions de liquidation et de service,
  l'indemnité journalière mentionnée à l'article L. 331-3 » ; « L'employeur qui a maintenu le salaire de l'assuré en application
  de l'article L. 3142-2 du code du travail est **subrogé de plein droit** dans les droits de son salarié à l'indemnité
  journalière. »
- **D. 331-7** : l'indemnisation se fractionne comme le congé (D. 3142-1-1 du code du travail).
- **R. 323-10** : l'employeur établit « une attestation se rapportant aux payes effectuées pendant les périodes de référence »,
  adressée à la caisse « sous forme électronique, par l'employeur » ; elle porte, pour chaque paye, la période, les journées et
  heures, le montant et la date, la retenue d'assurances sociales, le numéro de cotisant et l'organisme. R. 133-14, IV dit que la
  DSN en tient lieu — **pas pour le deuil**, que la DSN « ne couvre pas à ce jour » (la page de net-entreprises,
  `../accident-du-travail/`). Que R. 323-10, écrit pour la maladie, serve le deuil est ce que dit cette page (« Deuil
  d'enfant ») : une **inférence** du côté du code, dite dans `obligations-base.json`.

**Où le moteur s'en sert** : `logiciel/donnees/obligations-base.json`, obligation `attestation_salaire` (`textes`, `a_avoir`,
`inferences`) ; `atelier/sources/paie/src/obligations.rs` (`de_l_arret`, route `attestation-declare`).

## La majoration de la réduction générale chez un employeur affilié à une caisse de congés (relevé du 29/10/2026)

`articles-20261029.md`, section « La reduction generale et les deductions forfaitaires pour frais professionnels »
(`xml/D241-10.xml`, version en vigueur depuis le 15/06/2026, issue du décret n° 2026-509 du 12/06/2026) ; domaine
`reduction_generale` de `moteur/src/atelier/releve/codes/textes.rs`.

- **D. 241-10, I** : pour les salariés du IV de L. 241-13, « Coefficient = (T min + (T delta × [(1/2) × (3 × a × salaire minimum
  calculé pour un an / rémunération annuelle brute - 1)] P )) × b », les autres valeurs étant celles de D. 241-7.
- **D. 241-10, IV** : pour les salariés du 3° du IV de L. 241-13 (le paiement des congés mutualisé par une caisse de congés),
  « les valeurs a et b sont respectivement fixées à 1 et à 100/90 ».
- Avec **D. 241-7, II** (`xml/D241-7.xml`), qui arrondit « le résultat obtenu par application de cette formule » à quatre
  décimales : le coefficient majoré s'arrondit une fois, après la multiplication par b.

**Où le moteur s'en sert** : `atelier/sources/droit/src/reduction.rs` (`reduction_generale`, argument `caisse_conges`) et la
note `_majoration_caisse_conges` de `logiciel/donnees/baremes/2026.json` ; le test de la règle est
`droit/tests/droit/reduction.rs`.

## L'effectif au sens de la sécurité sociale, et son décompte (relevé du 29/10/2026)

`articles-20261029.md`, section « L'effectif au sens de la securite sociale » (`xml/L130-1.xml`, `xml/R130-1.xml`, R. 130-1
en vigueur depuis le 02/01/2020).

- **L. 130-1, I** : l'effectif annuel est « la moyenne du nombre de personnes employées au cours de chacun des mois de
  l'année civile précédente » ; **II** : un seuil n'est franchi qu'après cinq années civiles consécutives.
- **R. 130-1, I** : les mois sans salarié ne comptent pas dans la moyenne ; l'effectif se tronque au centième.
- **R. 130-1, II** : comptent les titulaires d'un contrat de travail, sauf les CDD qui remplacent un salarié absent
  (deuxième phrase du 2° de L. 1111-2 du code du travail) ; à temps plein, entièrement ; à temps partiel, au rapport des
  horaires du contrat à la durée légale ou conventionnelle (dernier alinéa de L. 1111-2) ; chacun « à due proportion du
  nombre de jours du mois pendant lequel [il a] été employé ».
- **R. 130-1, III** renvoie à L. 1111-3 du code du travail (apprentis, contrats aidés…), qui n'est pas relevé : ces
  exclusions ne sont pas appliquées.

**Où le moteur s'en sert** : `atelier/sources/envois/src/organismes_simules.rs` (`effectif_annuel`), le compte rendu de
nature 118 que le simulateur rend à une compagnie fictive ; `droit/src/contexte.rs` (`Effectif::atteint`) pour la règle
des cinq ans.

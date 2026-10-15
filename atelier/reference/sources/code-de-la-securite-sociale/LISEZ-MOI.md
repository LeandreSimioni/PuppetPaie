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

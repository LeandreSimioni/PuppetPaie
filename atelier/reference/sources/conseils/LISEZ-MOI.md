# Les conseils « Le saviez-vous ? » (ticket 224)

La banque des conseils de PuppetPaie est `logiciel/donnees/conseils.json` : chaque conseil y cite un texte **déjà déposé**
dans le dépôt (son fichier, l'endroit, une citation à l'identique). Ce dossier-ci ne garde que les textes qu'il a fallu
déposer pour elle, et l'histoire de sa rédaction.

## Les textes déposés pour la banque

Relevé du 21/10/2026 (curl). Les originaux HTML des pages de l'Afdas et les originaux Audiens (HTML, PDF) sont dans
`reference-privee/conseils/` du dépôt privé ; ici, la transcription du corps de la page (menus et scripts retirés), ou le
texte public.

| Fichier | Ce que c'est | URL | Où les conseils s'en servent |
|---|---|---|---|
| `afdas-intermittents-conditions-20261021.md` | Afdas, « Intermittents du spectacle : quelles conditions » : seuils de 48 / 88 / 130 jours ou cachets sur 2 ans, indépendance par rapport à France Travail, délais de carence (12 / 18 / 24 mois), avis sur l'arrêt des financements après le 25/10/2026 | https://www.afdas.com/particulier/connaitre-les-dispositifs-et-les-modalites-dacces-a-la-formation/intermittents-du-spectacle-et-de-laudiovisuel/quelles-conditions.html | conseils `afdas-seuils-jours-cachets`, `afdas-independant-de-france-travail`, `afdas-carence` |
| `afdas-intermittents-financements-20261021.md` | Afdas, « Intermittents du spectacle : quels financements » : barèmes horaires et plafonds par type de formation, CQP, CPF, PTP | https://www.afdas.com/particulier/connaitre-les-dispositifs-et-les-modalites-dacces-a-la-formation/intermittents-du-spectacle-et-de-laudiovisuel/quels-financements.html | aucun conseil aujourd'hui (le conseil sur ses barèmes est écarté : voir plus bas) |
| `legi-travail-D7121-28-a-41-et-L3141-3-a-5-20261021.md` | Code du travail, D. 7121-28 à 41 et L. 3141-3 à 5, texte en vigueur (transcription de `../legi-entrepot/articles.jsonl.gz`) ; D. 7121-31 en vigueur depuis le 01/01/2017 | base LEGI de la DILA (`echanges.dila.gouv.fr/OPENDATA/LEGI/`) | aucun conseil aujourd'hui : il fondait l'arbitrage sur l'ouverture du droit aux Congés Spectacles (plus bas) |
| `audiens-vos-conges-spectacles-page-20261021.md` | Audiens, page « Vos congés spectacles » (salariés) : « vous avez droit à des congés payés dès lors que vous travaillez en CDD de moins de 12 mois » ; indemnité calculée sur les déclarations des employeurs « qui cotisent à chacun de vos cachets » ; demande à partir de la mi-avril, 15 jours avant le départ, au plus tard le 31 mars | https://www.audiens.org/solutions/vos-conges-spectacles.html?univers=p---vos-conges-spectacles | idem (pratique de la Caisse) |
| `cour-des-comptes-caisse-conges-spectacles-s2021-1136-20261021.pdf` / `.txt` | Cour des comptes, observations définitives S2021-1136 (12/05/2021) sur la Caisse des Congés Spectacles : le règlement du régime (art. 2, 4, 9, 10), la période de référence au 1er avril (R. 3141-4), l'indemnité = 1/10 de la rémunération brute déclarée (+ congés sur congés, environ 11,1 %), la commission paritaire qui statue sur « le droit au congé prévu à l'article D.7121-31 » (art. 27 des statuts), le ratio des charges de gestion sur les cotisations, « 1,9% » en fin de période (1,75 % hors Guso) | https://www.audiens.org/files/live/sites/siteAudiens/files/03_documents/groupe/CP/Rapport-Cour-des-comptes-Conges-Spectacles-2021.pdf | conseil `conges-spectacles-ce-que-finance-la-cotisation` (« environ 2 % » de frais de gestion) |
| (privé) `audiens-conges-spectacles-flyer-artistes-FLYER_Conges_Spectacles-20261021.pdf` | l'original du flyer dont la transcription est dans `../chomage-intermittents/` | https://conges-spectacles.audiens.org/files/live/sites/siteCCS/files/resources/documents/FLYER_Conges_Spectacles.pdf | |
| (privé) `audiens-fiche-pratique-conges-spectacles-20261021.pdf`, `audiens-vos-conges-spectacles-page-20261021.html` | fiche pratique Audiens (datée 2022 : « Les salariés permanents ont droit à un temps de congés payés d'au moins 2,5 jours par mois… », barème 15,40 %) ; HTML de la page ci-dessus. Sa transcription de 2023 est `../majoration-caisses-conges/audiens-fiche-pratique-conges-spectacles-202302.txt` (l'exemple : indemnité de 10 % du salaire, « Il reste 21,60 € pour la part patronale et les frais de gestion ») | https://www.audiens.org/files/live/sites/siteAudiens/files/03_documents/entreprise/Fiches-techniques/FP-Conges-Spectacles.pdf | la transcription : conseil `conges-spectacles-ce-que-finance-la-cotisation` |

Les montants et délais de ces pages changent chaque année : ils sont datés du relevé.

## Où le moteur s'en sert

- `logiciel/donnees/conseils.json` : la banque. Un conseil a un `id`, un `domaine`, un `public`, un `texte` (sans le titre
  « Le saviez-vous ? », que l'écran pose ; la phrase courte d'un conseil de saisie), un `detail` pour certains conseils de
  saisie (déplié derrière « pourquoi ? »), une ou plusieurs `source` {`fichier`, `article` (et `code` pour l'entrepôt
  LEGI), `citation`, et `annexe` (« X » ou « VIII ») pour une source tirée d'une annexe du régime des intermittents :
  le moteur ne cite que celles des salariés dont parle le conseil}, un `quand` (« toujours » : un conseil général, celui du démarrage ; `saisie:<condition>` ;
  `historique:<condition>`), le `ciblage` voulu et la `certitude` du rédacteur. Un chiffre daté n'y est jamais écrit en
  dur : `{bareme:…}` et `{minima:…}` le lisent dans `donnees/baremes/<année>.json` et `donnees/minima-conventionnels.json`.
- `atelier/sources/paie/src/conseils.rs` : l'option de la compagnie (fiche Entreprise, `conseils`), la rotation du conseil
  du jour, les conditions de saisie et d'historique, et les constantes de l'article 30 des annexes (`annexes` dans la
  banque).
- `atelier/sources/paie/tests/fichiers/conseils_banque.rs` vérifie que chaque fichier cité existe et que chaque citation
  s'y lit (à la casse, aux blancs, aux apostrophes et aux guillemets près ; « […] » sépare des fragments).

## La rédaction (21/10/2026)

Deux rédacteurs, à partir des textes déposés : A (droits des intermittents, Congés Spectacles, Audiens, Afdas, médecine du
travail, conventions ; 81 conseils) et B (paie, contrats, déclarations, logiciel ; 86). Le cahier des charges : vrai et
sourcé, abordable (deux ou trois phrases de français courant), neutre et licite, une idée par conseil, l'employeur pour
interlocuteur. Leurs blocs (identifiant, domaine, public, texte, source, ciblage, certitude) sont passés tels quels dans la
banque ; les citations reformulées ou prises à cheval sur deux colonnes d'un PDF ont été remplacées par un fragment lu à
l'identique dans le texte déposé.

Arbitrages de Léandre : le numéro d'objet du spectacle est délivré par **France Travail** ; « AEM » se dit « DSN » côté
employeur quand la source le permet ; jamais d'euros d'allocation ; aucune mise en garde ni allusion à la fraude.

**Écartés** (absents de la banque) :

- `allocation-minimale-38-44` : un montant d'allocation en euros ; et « montant de l'allocation maximale » ne se lit pas
  dans le guide déposé.
- `afdas-baremes-horaires` : un barème daté de l'Afdas, absent des données du moteur ; sans ses chiffres, il ne dit plus rien.
- `afdas-demandes-avant-formation` : périssable (fonds épuisés après le 25/10/2026).
- `conges-spectacles-plafonds-metteur-en-scene` : plafonds journaliers datés (juillet 2025), absents des données du moteur ;
  sans eux, il redit `conges-spectacles-plafond-triple-minimum`.
- `audiens-prevoyance-fonds-sante` : taux Audiens 2026 absents des données du moteur à cet endroit ; la phrase sur le Fonds
  ne se lit qu'à cheval sur deux colonnes de la brochure de 2022.

**Tranchés par Léandre le 21/10/2026** (plus rien n'est suspendu) :

- **Le délai des signalements : « cinq jours ouvrés ».** `signalement-fin-contrat-cinq-jours` et
  `signalement-arret-cinq-jours` sont réactivés et citent le texte qui fait foi, CSS R. 133-14 II (version en vigueur au
  01/07/2026, décret 2026-425 ; l'entrepôt LEGI, et `../code-de-la-securite-sociale/xml/R133-14.xml`) : « sont déclarés dans
  un délai fixé par arrêté conjoint […], qui ne peut excéder cinq jours ouvrés, la fin du contrat de travail ainsi que,
  lorsque l'employeur n'est pas subrogé dans les droits de l'assuré aux indemnités journalières, le début et la fin de
  l'arrêt de travail […] » ; et l'arrêté du 2 juillet 2013, art. 1er (entrepôt LEGI, `evenements-dsn-2013` ;
  `../arrete-2013-delai-evenements-dsn/`, aucune version nouvelle au 15/10/2026) : « cinq jours ouvrés ». Fin de contrat et
  arrêt de travail ne sont pas distingués : même phrase, même délai. Les pièces d'application, rejouées le 21/10/2026
  (fiches 113 « 11/02/2022 », 673 « 13/05/2025 », 2062 « 05/09/2025 », dates servies identiques à celles déposées) : fiche 673,
  « dans les 5 jours ouvrés à connaissance de l'évènement » ; cahier technique (`../../norme-dsn/texte/chapitre-5-modeles.txt`
  l. 158, `texte-2027/` l. 155, § 5.2) : fin de contrat « dans les cinq jours ouvrés qui suivent la survenance » ; § 5.3
  arrêt : « dans les 5 jours après la prise de connaissance » ; § 5.4 reprise : « cinq jours ouvrables » ; fiches 113 et
  119 : « 5 jours », sans qualificatif. Ces écarts de rédaction des commentaires cèdent au texte réglementaire
  (**inférence** : aucun texte ne les tranche). Aucune source lue ne définit « jour ouvré » (lundi-vendredi hors fériés :
  usage, inférence).
  Deux conseils voisins citent désormais les textes qui les fondent : `signalement-fin-contrat-cddu-derogatoire` (un CDDU
  infra-DSN en circuit dérogatoire, nature 02 ou 10, motif de recours 05, S21.G00.62.017 = 01, motif de rupture 031, n'a pas
  de signalement de fin de contrat : sa fin part dans la DSN mensuelle, « sauf demande expresse du salarié » ; fiche 2062
  de `../net-entreprises-base-connaissances/` et cahier technique § 5.2 ; le retour sous 48 heures, fiche 1903) et
  `signalement-arret-subrogation` (subrogé, l'arrêt se déclare au plus tard avec la DSN mensuelle : R. 133-14 II al. 1 ;
  fiches 673 et 113).
- **L'ouverture du droit aux Congés Spectacles.** Le conseil `conges-spectacles-droit-ouvre-1er-jour` est **retiré** : il
  opposait D. 7121-31 (« au moins quatre semaines d'engagement ou de trente cachets », en vigueur depuis le 01/01/2017,
  `legi-travail-…md`) à Audiens (flyer : « Le droit à congé est ouvert dès le 1er jour de travail » ; page « Vos congés
  spectacles » : droit « dès lors que vous travaillez en CDD de moins de 12 mois » ; Cour des comptes, art. 9-10 du
  règlement : l'indemnité est 1/10 de la rémunération brute déclarée, sans seuil de jours). Le règlement du régime lui-même
  n'est publié nulle part où l'on ait cherché (non déposé) ; la raison de la divergence n'est qu'une **inférence** (la
  Caisse verse un salaire différé sur toute rémunération déclarée, et la loi de 2016 a supprimé le minimum de dix jours de
  L. 3141-3 auquel D. 7121-31 renvoie). Il est remplacé par `conges-spectacles-ce-que-finance-la-cotisation`, texte de
  Léandre qui évite le seuil contesté : ce que financent les {taux} % (le taux lu dans le barème de l'année) : environ
  10 % du salaire en indemnité (fiche pratique Audiens, l'exemple de 400 €), les cotisations sociales sur cette indemnité,
  et les frais de gestion, environ 2 % des cotisations (Cour des comptes : « 1,9% » en fin de période) ; les salariés
  demandent eux-mêmes leur indemnité. « de ces cotisations » est ajouté au texte de Léandre : sans lui, « 2 % » se lirait
  comme une part du salaire, ce que la source ne dit pas.

**Les conseils sur un salarié** (Léandre, 21/10/2026) : le salarié se nomme par son seul prénom (`{prenom}`), jamais par
un pronom qui le genre ; un conseil de saisie tient en une phrase, son détail derrière « pourquoi ? » ; le coût en jours
d'allocation s'arrondit au demi-jour le plus proche, et un coût qui s'arrondit à zéro se dit « moins d'un demi-jour »
(variantes `-minime`). Les conseils généraux ont été relus dans le même esprit : un pronom qui désignait le salarié
(« s'il justifie », « il reçoit », « entre vous et lui »…) a cédé la place au nom ou à une tournure impersonnelle ; les
noms de catégorie (« un intermittent », « vos salariés ») restent.

**Sans chiffre** : un chiffre daté qui n'est pas dans les données du moteur a été retiré du texte (minima de la CCNEAC,
paliers de la grille PAS, montant de l'indemnité maternité Audiens, plancher de l'indemnité d'incapacité, exemple
d'échéance de 2026) ; ceux qui y sont se lisent par gabarit (plafond de la Sécurité sociale, taux Congés Spectacles,
abattement d'assiette CSG, taux et plafond de la DFS, cachet de répétition de la 3090).

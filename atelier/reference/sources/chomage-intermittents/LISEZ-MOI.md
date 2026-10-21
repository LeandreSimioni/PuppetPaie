# L'assurance chômage des intermittents (annexes VIII et X) : textes en vigueur au 20/10/2026

Relevé du 20/10/2026 pour les **conseils « Le saviez-vous ? »** (ticket 224 : conseiller l'employeur, sur option, sur la
façon d'engager un intermittent pour ses droits, à masse salariale égale). Où le moteur s'en sert : voir la section du même
nom, plus bas.

Les annexes VIII et X elles-mêmes (texte du 15/11/2024, identique octet pour octet au PDF servi par l'Unédic le 20/10/2026)
sont déposées dans `../plafond-intermittents/` (`unedic-annexe8-convention-20241115.txt`,
`unedic-annexe10-convention-20241115.txt`) : **on ne les redépose pas**. Les originaux PDF des textes d'organismes
(Unédic) sont dans `reference-privee/chomage-intermittents/` du dépôt privé ; ici, la transcription (`pdftotext -layout`)
et l'URL.

## Quel texte est en vigueur

- Convention du 15/11/2024 relative à l'assurance chômage, **agréée par arrêté du 19/12/2024**, applicable aux fins de
  contrat à compter du 01/01/2025 (la plupart des évolutions au 01/04/2025), conclue pour 4 ans, du 01/01/2025 au
  31/12/2028 (`unedic-convention-consolidee-20261020.txt`, art. 10 et 11 ; art. 6 §2 : les annexes VIII et X sont
  « maintenues sous réserve des adaptations de cohérence »). Elle a remplacé le décret n° 2019-797.
- Avenant n° 1 du 07/07/2025 (agréé par arrêté du 18/02/2026) : modulation de la contribution patronale ; ne touche pas
  les annexes VIII et X.
- Avenant n° 2 du 10/04/2026 (agréé par arrêté du 19/06/2026, application au 01/09/2026 pour l'essentiel) : rupture
  conventionnelle, congé supplémentaire de naissance, maintien après l'âge de la retraite, listes NAF de l'article 71 de
  l'annexe VIII ; son article 3, 7° et 9°, modifie les annexes VIII et X **sans toucher** aux articles 3, 11, 12 (hors
  périodes neutralisées), 14, 16, 19, 20, 21, 22, 30. Les formules d'allocation sont donc celles du 15/11/2024.
- Circulaire Unédic n° 2026-06 du 01/09/2026 (règlement général, pas les annexes) : source de ces dates d'agrément
  (non déposée : 17 000 lignes, rien sur les annexes VIII et X).

## Les fichiers

| Fichier | Ce que c'est | URL (relevé 20/10/2026) |
|---|---|---|
| `unedic-convention-consolidee-20261020` (.txt) | Convention du 15/11/2024, consolidée avec les avenants 1 et 2 (PDF « 0309 ») | https://www.unedic.org/la-reglementation/conventions-d-assurance-chomage/texte-consolide-de-la-convention-du-15-novembre-2024-relative-a-lassurance-chomage-3 |
| `unedic-avenant1-convention-20250707` (.txt) | Avenant n° 1 du 07/07/2025 | https://s3.eusc-de-east-1.amazonaws.eu/unedic-eks-prod-jobs/unedic/regulatory_texts/TXT-AVN-Avenant_1_07_07_2025%20CG.pdf |
| `unedic-avenant2-convention-20260410` (.txt) | Avenant n° 2 du 10/04/2026 (modifie les annexes VIII et X, art. 3 7° et 9°) | https://s3.eusc-de-east-1.amazonaws.eu/unedic-eks-prod-jobs/unedic/regulatory_texts/TXT-AVN-Avenant_2_10_04_2026.pdf |
| `unedic-reglement-general-consolide-avenants1-2-20261020` (.txt) | Règlement général consolidé (avenants 1 et 2) ; art. 65 (§8 équivalences, §9 totalisation annexes VIII + X) | https://www.unedic.org/la-reglementation/conventions-d-assurance-chomage/texte-consolide-du-reglement-general-annexe-a-la-convention-du-15-novembre-2024-4 |
| `unedic-communique-revalorisation-20250625`, `…20260630` (.txt) | Communiqués du CA de l'Unédic : +0,5 % au 01/07/2025 ; **pas de revalorisation au 01/07/2026** | https://www.unedic.org/actualites/allocations-d-assurance-chomage-revalorisation-de-0-5-au-1er-juillet-2025 ; https://www.unedic.org/actualites/allocations-dassurance-chomage-pas-de-revalorisation-au-1er-juillet-2026 |
| `unedic-circulaire-2025-07-revalorisation-20250701` (.txt) | Circulaire 2025-07 et décision du CA du 25/06/2025 (« l'allocation minimale (hors annexes VIII et X) à 32,13 € ») | https://www.unedic.org/la-reglementation/circulaires/prescription-circulaire-no-2025-07-du-01-juillet-2025 |
| `francetravail-guide-intermittents-20261020` (.pdf, .txt) | Guide « Intermittents du spectacle » de France Travail, édition **septembre 2026** : formules A/B/C, planchers, plafond, franchises, cumul, clause de rattrapage, décompte des cachets et des heures | https://www.francetravail.fr/files/live/sites/PE/files/fichiers-en-telechargement/fichiers-en-telechargement---dem/GUIDE-INTERMITTENT.pdf |
| `francetravail-premiers-pas-intermittence-20261020` (.pdf, .txt) | « Les premiers pas dans l'intermittence du spectacle », France Travail, février 2026 (507 h, 12 h par cachet, AJ minimum 38 € / 44 €) | https://www.francetravail.fr/files/live/sites/PE/files/fichiers-en-telechargement/spectacle/Doc_Primo_Entrants.pdf |
| `francetravail-flyer-franchises-20261020` (.pdf, .txt) | Flyer « Franchises, délai d'attente… » de France Travail (non daté) | https://www.francetravail.fr/files/live/sites/PE/files/masters/spectacle/Flyer_Franchises.pdf |
| `kali-1285-organisation-travail-repetitions-representations-20261020.md` | CCNEAC (IDCC 1285), titres XIII à XVI, organisation du travail : services de répétition (4 h maximum, 2 par jour), représentations multiples le même jour, services des musiciens | extrait de `../kali-entrepot/` (base KALI/DILA) |
| `kali-3090-repetitions-cachets-representations-20261020.md` | CCNSVP (IDCC 3090), annexes artistes : répétition (2 services par jour), cachet du musicien = service de 3 h 30 indivisible, représentations par jour | extrait de `../kali-entrepot/` |

Les minima de salaire (cachet de répétition, de représentation) ne sont pas redéposés : `../kali-1285-x3-remuneration-artistes-20260929.md`,
`../ccneac-nao-2026-grilles-transcription-20260929.md` (accord du 01/07/2026, minima au 01/08/2026) et
`../legifrance-3090-avenant-salaires-20250721.md` (avenant du 21/07/2025, minima au 01/09/2025, étendu le 17/02/2026).

## Les articles du Code du travail à suivre (ne pas recopier : `paie releve-codes`)

Tous sont déjà dans `../legi-entrepot/` (texte `travail`) ; aucun n'est encore dans un domaine de
`atelier/sources/paie/src/releve_codes/textes.rs`. Domaine à créer au moment du module (par exemple `intermittents`) :

- Régime spécifique : **L5424-20** (contribution spécifique), **L5424-21** (allocations de solidarité), **L5424-22** et
  **L5424-23** (négociation des annexes, comité d'expertise), L5422-20.
- Solidarité : **D5424-51 à D5424-54** (allocation de professionnalisation et de solidarité, 507 h), **D5424-55 à D5424-61**
  et **D5424-58** (allocation de fin de droits, 30 €/jour).
- Fraude et sanctions : **L5429-1**, **L5426-5** (pénalité administrative ≤ 3 000 €), **L5426-1-1** (activité non déclarée),
  L5426-8-1/-2/-3 (recouvrement d'indu), R5426-1 à R5426-24 (procédure de pénalité) ; **L8221-1, -3, -5, L8223-1, L8224-1,
  L8224-2, L8271-8** (travail dissimulé : mention d'un nombre d'heures inférieur à celui accompli, L8221-5 2°).
- Qualité d'artiste, contrat : **L7121-2, L7121-3, L7121-4** (présomption de contrat de travail), L1242-2 3° (CDD d'usage).
- Pas retenu : **L5124-1** (il vise l'activité partielle, L5122-1, pas l'assurance chômage ; l'équivalent est L5429-1).

## Ce qu'on en lit, et ce qu'on n'a pas

Voir le rapport de la séance (formules recopiées, règles de décompte, lacunes). Lacunes connues au dépôt :

- Pas d'AFD ni de revalorisation propres aux annexes VIII et X publiées par l'Unédic en 2025-2026 : les communiqués et la
  circulaire 2025-07 donnent les montants du règlement général et disent « hors annexes VIII et X ».
- Légifrance a répondu 403 depuis le conteneur le 20/10/2026 (proxy) : les articles de loi viennent de l'entrepôt LEGI local.
- Les simulateurs de France Travail (https://candidat.francetravail.fr/portail-simulateurs/) sont dynamiques : leur méthode
  n'est pas publiée ; le guide ci-dessus en tient lieu.
- Aucun texte officiel trouvé qui dise combien de cachets d'un même employeur comptent pour un même jour : voir le rapport.

## Congés Spectacles : l'indemnité versée par la Caisse, et la franchise « congés payés » (relevé du 21/10/2026, ticket 224)

Question : l'indemnité de la Caisse dépend-elle des jours ou du salaire ; qu'est-ce qui détermine la franchise de l'assurance
chômage. Les originaux PDF d'Audiens sont dans `reference-privee/chomage-intermittents/` du privé ; ici la transcription
(`pdftotext -layout`) et l'URL. Tous relevés par `curl` le 21/10/2026 (HTTP 200).

| Fichier | Ce que c'est | URL |
|---|---|---|
| `audiens-conges-spectacles-mode-emploi-employeur-202304-20261021` (.txt) | « Congés Spectacles, employeur, mode d'emploi », avril 2023 (FAQ : montant de l'indemnité, durée du congé, plafond de l'assiette) | https://www.audiens.org/files/live/sites/siteAudiens/files/03_documents/entreprise/Fiches-techniques/Conges-Spectacles-Mode-d-emploi-employeur.pdf |
| `audiens-conges-spectacles-plafonds-conventionnels-202507-20261021` (.txt) | Fiche pratique « Plafonds conventionnels des congés payés des artistes et techniciens intermittents », mise à jour juillet 2025 | https://www.audiens.org/files/live/sites/siteAudiens/files/03_documents/entreprise/Cong%c3%a9s%20spectacles/FP-Plafonds-CCS.pdf |
| `audiens-conges-spectacles-flyer-artistes-202604-20261021` (.txt) | Flyer destiné aux artistes (éligibilité, étapes), « avril 2026 » | https://conges-spectacles.audiens.org/files/live/sites/siteCCS/files/resources/documents/FLYER_Conges_Spectacles.pdf |

Non redéposés : la fiche pratique `FP-Conges-Spectacles.pdf` servie aujourd'hui est identique, texte pour texte, à celle de
février 2023 déjà déposée (`../majoration-caisses-conges/audiens-fiche-pratique-conges-spectacles-202302.txt`) ; la web
conférence Audiens du 23/05/2023 (rien sur le calcul des Congés Spectacles au-delà de cette fiche).

Ce qu'on en lit (le détail est dans le rapport de la séance) :
- **Indemnité** : 10 % de la « base congé » déclarée sur la période 1er avril - 31 mars (mode d'emploi, FAQ « Quel est le
  montant de l'indemnité de congé payé ? » ; fiche 2023, exemple 10 % de 400 = 40 €). Aucun nombre de jours n'y entre.
- **Durée du congé** : « 2,5 jours pour 24 jours de travail ou cachets, sans que la durée totale du congé puisse excéder
  trente jours ouvrables » (mode d'emploi, FAQ « Quelle est la durée du congé payé ? »).
- **Plafond de l'assiette** : par jour, d'où l'importance des jours (fiche des plafonds ; D. 7121-37 ; CCN 3090 art. 10.1.2,
  `../kali-3090-titres-vii-x-xi-annexes-20260924.md`).
- **Franchise congés payés** (annexes VIII et X, art. 21 §1 a ; art. 23 §2 de l'annexe VIII) : 2,5 jours par 24 jours travaillés de la période
  de référence, plafonnée à 30 ; les jours travaillés sont ceux de l'AEM (`francetravail-guide-intermittents-20261020.txt`,
  l. 630-655 ; `francetravail-flyer-franchises-20261020.txt`, § 3).
- Lacune : le **règlement du régime** de la Caisse (articles 2, 4, 9, 10, 11 cités par le mode d'emploi) n'est pas publié sur
  les pages relevées ; l'ouverture du droit y est peut-être définie (voir le rapport).

## Où le moteur s'en sert (ticket 224, 21/10/2026)

- `logiciel/donnees/conseils.json` (la banque des conseils) cite ces textes : le guide France Travail (les 208 heures d'un
  technicien, les 12 heures d'un cachet, les répétitions déclarées et payées, la déclaration des artistes), les
  « premiers pas » (l'échéance de la DSN au 15 pour France Travail), les fiches Congés Spectacles, les extraits KALI des
  conventions 1285 et 3090 (services de répétition, cachet du musicien, répétitions annulées). Les annexes VIII et X
  (`../plafond-intermittents/`) fondent les conditions.
- `atelier/sources/paie/src/conseils.rs` évalue les conditions de saisie et d'historique : les heures du mois au regard du
  seuil de l'article 30 (27 jours de 10 h en X, 26 de 8 h en VIII), plus de 28 cachets (art. 3 §1, art. 12 §2), plus de
  208 heures pour un technicien (guide), le cachet de répétition de la 3090 (`minima::cachet_de_repetition`), les heures
  qu'une même somme couvre au minimum horaire de la convention (`minima::minimum_de_la_ligne`), et les 507 heures
  assurées par nos seuls contrats (`cumul::position_droits`). Le coût d'une heure de plus pendant l'indemnisation se dit en
  jours (art. 30 : heures ÷ 10 × 1,3 en X, ÷ 8 × 1,4 en VIII), jamais en euros.

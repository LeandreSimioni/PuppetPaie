# Agirc-Arrco — l'ANI du 17 novembre 2017 (le régime unifié), par la circulaire 2018-09-DC

## D'où ça vient

`agirc-arrco-circulaire-2018-09-dc-ani-20171117-20181219.pdf` : circulaire Agirc-Arrco **2018-09-DC du 19/12/2018**,
98 pages : la lettre d'envoi, puis l'**Accord national interprofessionnel du 17/11/2017** instituant le régime
Agirc-Arrco, ses annexes A et B, la délibération 1, les répertoires. « Applicable au 1er janvier 2019 ».
**Transmis par Léandre le 11/10/2026** (agirc-arrco.fr, textes de référence ; URL à compléter). Empreinte SHA-256 (16
premiers caractères) : `b8af8df7372e4f89`. Transcription : `pdftotext -layout`, `.txt` à côté (5 182 lignes).

## Ce qu'on en lit (lignes du `.txt`)

- **Article 32, tranches de rémunération** (l. 1245-1256) : « la tranche 1 […] est constituée de l'ensemble des
  éléments de l'assiette dont le montant n'excède pas **le plafond fixé en application de l'article L. 241-3 du code
  de la sécurité sociale** ; la tranche 2 […] compris entre [ce] plafond […] et le montant égal à huit fois ce même
  plafond ». **D11 (le plafond proratisé suit-il pour la retraite ?)** : l'ANI ne définit pas son propre plafond, il
  renvoie à celui de L. 241-3 CSS *tel qu'appliqué* ; la proratisation du plafond d'un temps partiel est une règle de
  L. 241-3 lui-même (`../code-de-la-securite-sociale/`). Lecture : les tranches suivent le plafond de Sécurité sociale
  du salarié, proratisé ou non comme en vieillesse ; **inférence** (le renvoi est au texte, pas à un chiffre), rien
  dans l'ANI ne dit le contraire. Le § 2 (assiettes supérieures maintenues depuis avant 1993) ne concerne pas une
  compagnie récente.
- **Article 37** (l. 1351-1364) : CEG 2,15 % T1 et 2,70 % T2 ; **CET 0,35 % sur T1 et T2 « pour les participants dont
  la rémunération excède le plafond »** : c'est la règle que le moteur applique (`Norme::cotisation_cet_patronale`,
  V4a ; en 2027, code 148 selon la fiche 2556).
- Articles 30-31 (l. 1196-1243) : l'assiette est celle des cotisations de Sécurité sociale ; forfaits pour les
  travailleurs hors de France.
- Article 73 (l. 2348-2364) : réduction temporaire du temps de travail dans un contexte économique difficile, droits
  maintenus sur cotisations : hors de notre champ.
- Le reste (affiliation, répartition 60/40, art. 38 s., gestion, allocations) : non lu ce jour.

## Où le moteur s'en sert

`logiciel/donnees/baremes/2026.json` (T1/T2, CEG, CET : les notes `_source` citent Audiens « Paramètres 2026 »,
`atelier/reference/sources/audiens-parametres-2026/`, et la
fiche « taux et répartition ») ; `paie/src/norme.rs` ou équivalent (`cotisation_cet_patronale`).

**Lot R5a (11/10/2026), le plafond d'un temps partiel (D11)** : le cahier CT2026 (l. 3325-3343) fait proratiser la
caisse sur 40.013 / 40.012 pour un « 20 - temps partiel ». Le moteur n'a qu'un plafond par paie
(`paie/src/calcul.rs`, `plafond_de_l_engagement`), d'où sortent la vieillesse plafonnée, T1/T2 et la CET ; il le
proratise par défaut (`calcul::plafond_temps_partiel`) avec le même rapport (`calcul::duree_du_temps_partiel` : 40.013
sur la plus courte de la durée légale et de 40.012). `engagement::modalite_temps_travail` (40.014) compare désormais la
durée du contrat à cette même plus courte durée (C. trav. L. 3123-1, 1°) : un 35 h sous une référence de 169 h (admise
en P26V01) est un temps plein, que la caisse ne proratise pas plus que la paie. 51.011 (heures complémentaires) : une
paie à temps partiel proratisé qui porte des heures en plus du mois refuse (on ne sait pas si ce sont des heures
complémentaires) ; le découpage infra-mensuel d'un changement en cours de mois (Q9) n'est pas fait, le refus reste.
Ce qui reste ouvert : en P26V01 (sans 40.084), un établissement qui renonce à la proratisation déclare un « 20 » que la
caisse proratisera, alors que la paie garde le plafond entier ; le cahier ne dit pas comment le lui dire en 2026.

**Ticket #223 (20/10/2026), la date du prélèvement** : `agirc-arrco-aide-codification-dsn-ct2026-v1.0-20251201.txt`,
p. 47 (l. 2399-2405) : sans date en S21.G00.20.011, « le prélèvement sera exécuté au plus tard le dernier jour ouvré du
mois d'exigibilité des cotisations », le mois qui suit le mois principal déclaré (mensuel) ou le trimestre civil
(trimestriel). `paie/src/ordres_paiement.rs` le dit, dans la boîte de dépôt de la DSN, pour une institution dont
`codes81` porte le 131 ou le 105 : le dernier jour ouvré du mois qui suit la fin de période (S21.G00.20.007). Pour
Audiens (44832375800038), c'est le 25 du mois suivant qu'Audiens dit lui-même (`../audiens-dsn/`, et `jour_de_paiement`
de `logiciel/donnees/organismes-base.json`) : le cahier dit « au plus tard », l'ANI art. 44 cité p. 47 « au plus tard
le 25 du mois ».

## La page « Documentation technique DSN » d'Agirc-Arrco, relevée par le conteneur le 11/10/2026

URL donnée par Léandre : `https://www.agirc-arrco.fr/entreprises/mon-entreprise/calculer-et-declarer/documentation-technique-dsn/`
(page modifiée le 27/04/2026, copie `agirc-arrco-page-documentation-technique-dsn-20261011.html`). Elle **ne sert pas
l'ANI** lui-même (la circulaire ci-dessus vient d'ailleurs ; son URL d'origine reste à noter) ; elle dit que « l'ANI du
17/11/2017 […] ne prévoit pas de distinction catégorielle entre cadres, non-cadres, article 36 », les particularités venant
des branches. Documents relevés (`agirc-arrco.fr/storage/…`), transcrits en `.txt` :

| fichier | ce que c'est | SHA-256 (16) |
|---|---|---|
| `agirc-arrco-aide-codification-dsn-ct2026-v1.0-20251201.pdf` | **Cahier d'aide à la codification DSN pour la retraite complémentaire**, CT 2026.1.0, décembre 2025, 125 p. | `627b06ca383d53b1` |
| `agirc-arrco-aide-codification-dsn-ct2025-v1.3-20250301.pdf` | le même pour le CT 2025.1.2, v1.3, 125 p. | `60732fc344c0e558` |
| `agirc-arrco-faq-temps-partiel-cotisant-temps-plein-201910.pdf` | FAQ « Salarié à temps partiel cotisant temps plein », octobre 2019 | `3161b8ceeef10a9a` |
| `agirc-arrco-faq-changement-de-situation-en-cours-de-mois-v2-2025.pdf` | FAQ « Changement de situation en cours de mois », v2 (exemples 2025) | `7f8146ca47fb0635` |
| `agirc-arrco-faq-changement-de-situation-retroactif-v2-20250331.pdf` | FAQ « Changement de situation rétroactif », v2, 31/03/2025 | `f95556c5b6393321` |
| `agirc-arrco-faq-reduction-generale-2026.pdf` | FAQ « Réduction générale des cotisations » 2026 (RGDU) | `2265f460ea0e307e` |
| `agirc-arrco-faq-exonerations-v2026-20250301.pdf` | FAQ « Exonérations », v2026 | `6cd7d67a426c05d6` |
| `agirc-arrco-faq-sommes-versees-apres-depart-v2-202506.pdf` | FAQ « Sommes versées postérieurement au départ du salarié », juin 2025 | `a274f8d4432c1236` |
| `agirc-arrco-faq-bloc-changement-de-contrat-201707.pdf` | FAQ « Bloc changement de contrat » (41), juillet 2017 | `1501be3ea55383b3` |

Non relevés : LODEOM, décalage de paie 2018, détaché/expatrié, congés payés (caisses BTP), mandataire social,
fractionnement DSN, contrat d'engagement éducatif, activité partielle COVID, apprentis, CET vers PERCO.

### Ce qu'on en lit (lignes des `.txt`)

- **D11, le plafond d'un temps partiel pour la retraite, tranché par Agirc-Arrco lui-même** : cahier CT2026, l. 3325-3331,
  commentaire sous la modalité « 20 - temps partiel » : « le plafond de sécurité sociale de la période d'emploi est
  déterminé en tenant compte du taux d'activité : valeur mensuelle du plafond × (durée contractuelle (40.013) + heures
  complémentaires (51.011)) / durée légale du travail (40.012) », durée conventionnelle au dénominateur si inférieure ;
  « à défaut d'une modalité 20 déclarée, le salarié est considéré comme employé à temps plein ». La FAQ temps partiel
  (octobre 2019) applique la même formule (« PSS réel » = PSS × durée contractuelle / durée légale) et traite le cas « non
  proratisation » (assiette forfaitaire, forfait jours, employeurs multiples). **Ce que ça change pour V4b** : le moteur
  proratise le plafond vieillesse « quand la fiche entreprise le choisit » ; pour Agirc-Arrco, la proratisation d'un temps
  partiel est la règle de lecture de la caisse, pas une option, et elle se lit sur 40.013/40.012/51.011. À porter au lot :
  les tranches T1/T2 d'un temps partiel suivent ce plafond proratisé (ce n'est plus une inférence depuis l'ANI seul).
- **Q9, le changement de situation en cours de mois** (FAQ v2, l. 1-60) : « le salarié concerné par ce changement doit
  faire l'objet d'un **découpage de ses bases assujetties** » ; exemple passage non cadre → cadre au 10 avril : deux
  périodes infra-mensuelles (01-09, 10-30), **salaire et plafond proratisés aux jours de chaque période**, blocs 78 datés
  par période. C'est la réponse (c) de Q9, et elle n'est pas optionnelle pour Agirc-Arrco.
- **D10, le 105** (cahier CT2026, l. 5265-5312) : « à compter de janvier 2025, les codes attendus sont 131 et 132 » ; « le
  code 105 […] est à utiliser **uniquement pour la régularisation des périodes antérieures à 2025** » ; « la valeur 105
  est maintenue en norme pour permettre de déclarer » ces régularisations. En P27V01 elle n'y est plus (tableau de
  combinaisons, `../net-entreprises-base-connaissances/`) : une régularisation 2027 d'un mois 2026 déposé en 105 ne peut
  avoir existé (2026 est déjà en 131) ; seule une régularisation d'un mois 2024 ou antérieur poserait la question.
  Le 131 = retraite + CET + CEG (l. 5303-5305), le 132 = Apec seule.
- **La RGDU 2026** (FAQ réduction générale) : composant 79.001 = 01 requis ; l'exonération porte sur la part patronale du
  taux minimum (limitée à 60 %) sur T1, la CEG patronale sur T1 ; taux retenus après taux d'appel de 127 % (6,20 × 127 %
  × 60 % = 4,72 %) ; répartitions dérogatoires de branche « capées » au taux réellement pris en charge si inférieur à
  60 %. À confronter à `paie/src/reduction_generale.rs` ou équivalent (V3a) le jour du lot.
- Changement rétroactif, sommes versées après le départ, bloc 41, exonérations : non lus ce jour.

Rien n'est codé (11/10/2026).

### Codé au lot R10 (11/10/2026) : Q9, le changement de situation en cours de mois

La FAQ « Changement de situation en cours de mois » (v2) et le cahier CT2026, § 6.1 (« Il est donc nécessaire en cas
de changement en cours de mois de ventiler les bases assujetties par périodes ») sont appliqués par
`atelier/sources/paie/src/periodes.rs` : la paie d'un permanent se découpe aux jours où sa fiche change le statut, la
catégorie, la PCS ou le code emploi, et au jour d'un avenant dont la saisie dit les termes d'avant (`avant`) ; chaque
période élémentaire a son salaire et son plafond au prorata de ses jours (exemple 1), le plafond et le salaire d'une
période à temps partiel à sa durée du travail (exemple 2) ; `dsn.rs` (`bloc_versement`) date les bases 02/03 (et toutes
les autres) de chaque période. Les cotisations 131/132 de chaque période suivent sa pile (l'Apec sur la seule période
cadre). Tests : `atelier/sources/paie/tests/periodes.rs`. **Inférence** (aucun texte déposé) : la réduction générale
garde un coefficient pour le mois, appliqué au brut de chaque période. Les chiffres de la FAQ (2025, PSS 3 925) ne sont
pas rejoués à l'euro : le moteur calcule sur le barème de l'année du mois payé.

## Dans le dépôt public (15/10/2026)

Ce dossier est dans le dépôt public de PuppetPaie. Les originaux suivants, d'un organisme privé, n'y sont pas : seule leur transcription y est (`pdftotext -layout` pour un PDF, une feuille de calcul en CSV, feuille par feuille, le texte d'une page HTML), et c'est elle que citent les renvois. Les condensats donnés plus haut sont ceux des originaux, gardés dans le dépôt privé (`reference-privee/sources/agirc-arrco/`).

- `agirc-arrco-aide-codification-dsn-ct2025-v1.3-20250301.pdf` : transcription `agirc-arrco-aide-codification-dsn-ct2025-v1.3-20250301.txt`
- `agirc-arrco-aide-codification-dsn-ct2026-v1.0-20251201.pdf` : transcription `agirc-arrco-aide-codification-dsn-ct2026-v1.0-20251201.txt`
- `agirc-arrco-circulaire-2018-09-dc-ani-20171117-20181219.pdf` : transcription `agirc-arrco-circulaire-2018-09-dc-ani-20171117-20181219.txt`
- `agirc-arrco-faq-bloc-changement-de-contrat-201707.pdf` : transcription `agirc-arrco-faq-bloc-changement-de-contrat-201707.txt`
- `agirc-arrco-faq-changement-de-situation-en-cours-de-mois-v2-2025.pdf` : transcription `agirc-arrco-faq-changement-de-situation-en-cours-de-mois-v2-2025.txt`
- `agirc-arrco-faq-changement-de-situation-retroactif-v2-20250331.pdf` : transcription `agirc-arrco-faq-changement-de-situation-retroactif-v2-20250331.txt`
- `agirc-arrco-faq-exonerations-v2026-20250301.pdf` : transcription `agirc-arrco-faq-exonerations-v2026-20250301.txt`
- `agirc-arrco-faq-reduction-generale-2026.pdf` : transcription `agirc-arrco-faq-reduction-generale-2026.txt`
- `agirc-arrco-faq-sommes-versees-apres-depart-v2-202506.pdf` : transcription `agirc-arrco-faq-sommes-versees-apres-depart-v2-202506.txt`
- `agirc-arrco-faq-temps-partiel-cotisant-temps-plein-201910.pdf` : transcription `agirc-arrco-faq-temps-partiel-cotisant-temps-plein-201910.txt`
- `agirc-arrco-page-documentation-technique-dsn-20261011.html` : transcription `agirc-arrco-page-documentation-technique-dsn-20261011.txt`

# Le montant net social (`S21.G00.58.004`, « Montant net social » du bulletin) — les textes

Relevé le 10/10/2026 (séance cloud, corrections DSN vague 1b), pour écrire `58.004` selon sa définition et non
plus comme le net à payer.

## Les textes, par rang

1. **Décret n° 2023-1378 du 28/12/2023** (JORFTEXT000048730782) : codifie la définition, appliquée aux ressources
   perçues depuis le 01/01/2024 (déclarées depuis février 2024). Code du travail, **art. R. 3243-1, 9° bis**
   (mention du bulletin) ; code de la sécurité sociale, **art. R. 844-1, II** (prime d'activité) ; code de l'action
   sociale et des familles, art. R. 262-12, II (RSA). **Les articles se relèvent par `outils/relever_les_codes.py`**
   (base LEGI de la DILA : Légifrance refuse les robots) : `code-du-travail/articles-*.md` (R3243-1) et
   `code-de-la-securite-sociale/articles-*.md` (R844-1). La définition : la différence entre, d'une part, les
   montants bruts des sommes et avantages versés en contrepartie ou à l'occasion du travail et des revenus de
   remplacement, **à l'exception du financement par l'employeur des garanties collectives de frais de santé (y
   compris le versement santé), de prévoyance complémentaire et de retraite supplémentaire**, et d'autre part les
   cotisations et contributions sociales à la charge du salarié **y compris son financement des garanties
   collectives de prévoyance complémentaire et de retraite supplémentaire**.
2. **Arrêté du 31/01/2023** (JORFTEXT000047096915) modifiant l'arrêté du 25/02/2016 (libellés du bulletin) : la
   ligne « Montant net social » ; arrêté du 25/06/2024 (JORFTEXT000049831771) : le modèle à deux rubriques obligatoire
   au 01/01/2026, qui renvoie au 9° bis de R. 3243-1. **Déposés le 10/10/2026 (vague 1m)** depuis la base LEGI de la
   DILA, avec l'article 1 consolidé de l'arrêté du 25/02/2016 : voir la dernière section.
3. **La FAQ du ministère du Travail, reprise au BOSS** (rubrique « Montant net social », mise à jour 18/01/2024) :
   le détail. **Le BOSS n'a pas répondu depuis le conteneur le 10/10/2026** (connexion coupée) : la FAQ est déposée
   telle qu'une note d'éditeur de paie la reproduit mot pour mot, avec ses numéros de questions
   (`sage-bulletin-clarifie-2023-net-social-faq-boss.pdf`, Sage 100 Paie, 2023, texte dans le `.txt`). À redéposer
   depuis la page du BOSS dès qu'elle répond. Ce qu'on en lit :
   - **Q 2.2** : « l'ensemble des sommes brutes correspondant aux rémunérations et revenus de remplacement versés par
     les employeurs à leurs salariés (salaires, primes, rémunération des heures supplémentaires, avantages en nature,
     indemnités de rupture, etc.), diminuées des cotisations et contributions sociales qui leur sont applicables. »
   - **Q 2.11** : « L'ensemble de la rémunération brute du salarié est pris en compte, indépendamment des exonérations,
     déductions, abattements ou franchises applicables et de l'assujettissement fiscal ou social. » Donc une indemnité
     de rupture **exonérée** y entre.
   - **Q 2.12**, n'entrent pas : « des données de paie qui ne sont pas des revenus » (un acompte, une avance, une
     saisie : des mouvements, pas des revenus) et, par exception, « les remboursements de frais professionnels dans la
     limite de chacun de leurs plafonds d'exonération », « la part patronale pour le financement des garanties
     collectives à la complémentaire santé obligatoire », les avantages en nature exemptés liés aux activités
     sociales, l'intéressement et la participation placés, les abondements, les IJSS y compris subrogées.
   - **Q 2.14** : sont déduites « l'ensemble des cotisations et contributions sociales obligatoires d'origine légale
     ou conventionnelles à la charge du salarié, ainsi que les cotisations salariales à la complémentaire santé » ;
     **depuis le 01/01/2024** (décret 2023-1378, point 1), les cotisations salariales de prévoyance complémentaire et
     de retraite supplémentaire se déduisent aussi (la note Sage, de 2023, dit encore l'inverse : c'est la règle
     d'avant 2024).
   - Le prélèvement à la source n'est pas déduit (un net *social*, avant impôt).

## Ce que le moteur en fait (vague 1b, confirmé en vague 1m)

`58.004` = brut soumis − cotisations salariales (légales, conventionnelles, santé, prévoyance) + les lignes hors
assiette qui sont des **revenus** (indemnités, imposables ou non) ; **sans** les remboursements de frais professionnels
(défraiements), **sans** les mouvements (acompte). La part patronale santé et prévoyance (Fonds santé des intermittents
compris) n'y entre pas : CSS R. 844-1, II, 1°, auquel renvoie le 9° bis de R. 3243-1 du code du travail (dernière
section). Le net à payer ne change pas. Où : `atelier/sources/engagement/src/calcul.rs`, `calculer`, section 7.10
(`Paie::net_social`) ; `atelier/sources/declarations/src/dsn.rs::net_social` (58.004) ; `atelier/sources/documents/src/bulletin.rs` (« MONTANT NET SOCIAL » ; depuis la vague 1o, le bas
du bulletin suit le modèle de l'arrêté du 25/02/2016, art. 1, I, rédaction du 25/06/2024, `legi-arrete-20160225-bulletin/LEGIARTI000049854119.xml` :
MONTANT NET SOCIAL, REMBOURSEMENTS ET DÉDUCTIONS DIVERSES — défraiements et acompte, Q 39 —, MONTANT NET A PAYER AVANT IMPOT SUR
LE REVENU, IMPOT SUR LE REVENU, MONTANT NET A PAYER (en Euros) ; plus de ligne fusionnée « NET SOCIAL / NET A PAYER AVANT IMPÔT ») ; notes `_source` des
lignes `prevoyance` des barèmes 2025 et 2026. Tests : `tests/net_social.rs` (permanent : Montansier ; intermittent :
BI4.26.06, `la_part_patronale_de_prevoyance_et_du_fonds_sante_d_un_intermittent_reste_hors_du_montant_net_social`).

## La page du BOSS, déposée le 10/10/2026 (apportée par Léandre depuis le poste)

`boss-montant-net-social-20261001.pdf` (transcription `boss-montant-net-social-20261001.txt`, `pdftotext -layout`) :
boss.gouv.fr › Bulletin de paie › Montant net social, **version en vigueur au 01/10/2026**, « mis à jour le
02/10/2026 », imprimée le 10/10/2026. Elle remplace la reproduction Sage du point 3 pour tout ce qu'elle dit.

**Elle contredit le code relevé sur un point, non tranché** :
- le BOSS range dans le montant net social « la part patronale pour le financement des garanties collectives à la
  complémentaire santé obligatoire […], ainsi que pour le financement des garanties collectives mentionnées à
  l'article L. 911-1 […], et le versement santé » (tableau du II.A.1) ; Q 13 (contributions patronales de prévoyance
  « doivent être prises en compte »), Q 14 (versement santé : « bien à prendre en compte »), Q 29 (« La contribution
  patronale à la complémentaire santé […] doit être intégrée au montant net social » ; de même la contribution
  patronale aux garanties de l'article L. 911-1) ;
- le **R. 844-1, II, 1°** relevé dans la base LEGI (`code-de-la-securite-sociale/articles-20261010.md`, version en
  vigueur depuis le **27/06/2026**, modifiée par le **décret n° 2026-535 du 25/06/2026, art. 2**) dit l'inverse :
  « à l'exception du financement par l'employeur des garanties collectives mentionnées à l'article L. 911-1 et du
  versement mentionné au I de l'article L. 911-7-1 ».
Le décret prime la FAQ. Tranché en vague 1m (dernière section) : 2026-535 n'a pas touché ce point, la page du BOSS
n'a pas de texte derrière elle. Le moteur suit aujourd'hui le décret (vague 1b : part patronale hors du montant net social).

Ce que la page dit d'autre, que la reproduction Sage disait autrement ou ne disait pas :
- **Q 13 bis** : les IJSS **subrogées** entrent dans le montant net social, pour leur montant net de CSG/CRDS (la
  reproduction Sage les excluait) ; versées directement par la CPAM, c'est elle qui les déclare. Le catalogue livré
  n'a pas d'IJSS (passation V1c, B).
- PPV : versée directement, dedans ; placée sur un plan d'épargne, dehors (tableau du II.A.1).
- Indemnités complémentaires d'activité partielle, maintien de salaire, indemnités de rupture de toute nature : dedans.
- Q 35 : un rappel ou une régularisation se prend dans le montant net social **du mois où il est versé** (rattaché à
  la période de versement), sauf si seul le montant net social déclaré était faux.
- Q 37 : un montant net social négatif s'affiche négatif ; Q 38 : aucun cumul ne s'affiche.

## Vague 1m (10/10/2026) : la page du BOSS contre le décret, tranché par les textes

### Ce qui est déposé (base LEGI et JORF de la DILA, https://echanges.dila.gouv.fr/OPENDATA/)

| fichier | ce que c'est | d'où |
|---|---|---|
| `legi-r844-1-versions/` (+ `R844-1-versions-20231231-20260627.md`) | CSS R. 844-1 : la version du 31/12/2023 au 27/06/2026 (`LEGIARTI000048841691`, décret 2023-1378) et celle en vigueur depuis le 27/06/2026 (`LEGIARTI000054314917`) | livraison `LEGI_20260626-222225.tar.gz` (l'ancienne version y est réémise avec l'état MODIFIE) ; la nouvelle, du relevé `code-de-la-securite-sociale/xml/` |
| `jorf-decret-2026-535-20260625/` (+ `.md`) | décret n° 2026-535 du 25/06/2026 (JORFTEXT000054310335), JORF n° 0148 du 26/06/2026 : notice, visas, 4 articles | livraison `JORF_20260626-003520.tar.gz` |
| `legi-decret-2025-292-20250329/` (+ `.md`) | décret n° 2025-292 du 29/03/2025 (JORFTEXT000051393074), « relatif au calcul de la prime d'activité » | archive globale `Freemium_legi_global_20250713-140000.tar.gz` |
| `legi-arrete-20160225-bulletin/` (+ `.md`) | arrêté du 25/02/2016 (bulletin de paie), article 1 : versions du 01/01/2022, du 08/02/2023 et du 30/06/2024 (en vigueur, `LEGIARTI000049854119`) | archive globale ; aucune livraison LEGI du 13/07/2025 au 09/10/2026 (456 lues) ne le modifie |
| `legi-arrete-20230131-bulletin/`, `legi-arrete-20240625-bulletin/` | les arrêtés modificatifs du 31/01/2023 (JORFTEXT000047096915) et du 25/06/2024 (JORFTEXT000049831771) | archive globale |

### Ce que disent les textes

1. **Ce que le décret n° 2026-535 a changé** (art. 2) : au **5° du I** de R. 844-1, « et du congé supplémentaire de
   naissance » après « ou d'adoption » ; en vigueur le 27/06/2026, les indemnités de naissance comptées à partir du
   01/07/2026 (art. 3). **Rien d'autre.** La comparaison des deux versions (BLOC_TEXTUEL alinéa par alinéa) ne
   montre que cette ligne : le **II, 1°** (« à l'exception du financement par l'employeur des garanties collectives
   mentionnées à l'article L. 911-1 et du versement mentionné au I de l'article L. 911-7-1 ») est **mot pour mot celui
   du décret n° 2023-1378**, applicable aux ressources perçues depuis le 01/01/2024 (nota de la version de 2023).
2. **Le décret n° 2025-292** modifie le seul D. 843-3 (la fraction des revenus professionnels de la prime d'activité,
   59,85 %, pour les revenus pris en compte à partir du 01/04/2025) ; il **cite** R. 844-1 sans le modifier. Hors sujet
   pour le montant net social.
3. **L'arrêté du 25/02/2016, article 1, II** (rédaction de l'arrêté du 25/06/2024, en vigueur depuis le 30/06/2024 ; la
   rédaction du 08/02/2023 avait les mêmes exceptions) : la valeur du « Montant net social » « correspond au montant
   visé au 9° bis de l'article R. 3243-1 » : les sommes dues en contrepartie ou à l'occasion du travail « à l'exception
   des indemnités prévues à l'article L. 313-1 [...] lorsque l'employeur est subrogé [...], ainsi que du financement
   prévu au III de l'article L. 911-7 et du versement mentionné au I de l'article L. 911-7-1 », moins « le montant
   total des cotisations et contributions sociales d'origine légale ou conventionnelle à la charge du salarié ».
   L'arrêté n'excepte donc que la **santé** patronale et le versement santé, pas la prévoyance patronale.
4. **La page du BOSS du 01/10/2026** (II.A.1, Q 13, Q 14, Q 29) range dans le montant net social la part patronale
   santé, la contribution patronale aux garanties de l'article L. 911-1 (prévoyance, retraite supplémentaire) et le
   versement santé. **Aucun texte déposé ne l'appuie** : ni le décret (qui les excepte toutes), ni l'arrêté (qui excepte
   la santé). La reproduction de 2023 (`sage-...txt`, Q 2.12) disait encore le contraire pour la santé : la doctrine a
   changé, le texte non. Pourquoi : on ne le sait pas. **Inférence, non vérifiée** : la page anticipe une réforme
   annoncée et non publiée ; aucun décret ni arrêté modifiant R. 844-1, R. 3243-1 ou l'arrêté de 2016 n'est paru
   jusqu'à la livraison LEGI du 09/10/2026 (aucune version à venir n'est listée dans leurs XML).

| | R. 844-1 II (décret) | arrêté 2016, art. 1 II | BOSS 01/10/2026 |
|---|---|---|---|
| santé patronale (L. 911-7 III), versement santé (L. 911-7-1) | dehors | dehors | **dedans** |
| prévoyance, retraite supplémentaire patronales (L. 911-1) | dehors | **dedans** (non exceptées) | **dedans** |
| IJSS subrogées | dedans (I, 6° : « sommes destinées à compenser la perte de revenu ») | **dehors** | dedans, nettes de CSG/CRDS (Q 13 bis) |

### La règle retenue pour la paie d'octobre 2026

**Le décret.** Le 9° bis de R. 3243-1 (décret en Conseil d'État) définit la mention du bulletin « tel qu'il est défini
au II de l'article R. 844-1 » ; l'arrêté, pris pour R. 3243-2 (libellés, ordre, regroupement), dit lui-même que sa
valeur « correspond au montant visé au 9° bis » ; la page du BOSS est une doctrine. Le décret prime les deux. Donc :
la part patronale santé, prévoyance et Fonds santé reste **hors** du montant net social, comme depuis la vague 1b ; le
moteur ne change pas de calcul.

**Les mois antérieurs** : le II, 1° de R. 844-1 est le même depuis le 31/12/2023 (ressources perçues depuis le
01/01/2024), l'exception santé de l'arrêté depuis le 08/02/2023 : un mois corrigé de 2024, 2025 ou 2026 se calcule
selon la même règle, quel que soit le mois de la correction. Avant 2024, le décret ne définissait pas encore ce montant
(version du 06/11/2020 non relevée) ; le moteur ne produit pas de mois aussi anciens.

**Le doute qui reste, posé à Léandre (passation V1m)** : la **prévoyance patronale**. Le décret l'exclut ; l'arrêté de
2016 ne l'excepte pas et la page du BOSS l'inclut ; la norme DSN (2026.1, S21.G00.58.003, p. 265) renvoie à « la
définition modifiant l'arrêté du 25 février 2016 ». La hiérarchie des normes tranche pour le décret ; si Léandre veut
suivre l'arrêté et le BOSS, le changement est à un seul endroit (`calcul.rs`, 7.10) et demande de distinguer, dans la
ligne `prevoyance` des intermittents, le Fonds santé (santé) du décès (prévoyance).

### Le reste de la page du BOSS, confronté au moteur

- **Q 13 bis** (IJSS subrogées, nettes) : le catalogue livré n'a pas d'IJSS ni de subrogation ; la norme 2026.1 a un
  type 58.003 = 10 « IJSS subrogées prises en compte dans le calcul du MNS ». À porter le jour où la subrogation l'est,
  et l'arrêté (qui les exclut) contre le décret et le BOSS (qui les incluent) sera à trancher.
- **Q 35 / Q 41** (un rappel se prend dans le montant net social du mois de son versement ; le mois d'origine ne se
  corrige que si seul le montant net social était faux) : la correction d'un mois déposé (`dsn.rs`,
  `bloc_regularisation`) écrit, dans le versement du mois de la correction, l'annulation du 58 déposé et le 58 recalculé
  **aux dates de la période d'origine** (58.001/58.002). C'est le traitement du cas « seul le MNS était faux » ; pour une
  correction qui change ce qui a été versé, la page du BOSS et la norme (« à renseigner en période de versement »)
  demandent le rattachement au mois du versement. Non modifié : question posée dans la passation V1m. **Fait en vague
  1o** (Léandre : « on respecte le texte au maximum ») : `dsn.rs::blocs_58_de_la_correction`. Si ce qui a été versé
  ne change pas (net à payer corrigé = net déjà versé, lu sur le bulletin), l'annulation du 58 déposé et le 58 recalculé
  restent aux dates d'origine (Q 35, premier cas). Si la correction verse ou reprend, le 58 d'origine ne se corrige
  que s'il ne disait pas le montant net social de ce qui a été payé (net déjà versé moins frais et acompte de la paie
  d'origine ; Q 41 : « en précisant quel était le montant net social effectivement versé »), et la différence se
  déclare aux dates du mois du versement (norme 2026.1, S21.G00.58.003, `norme-dsn/texte/bloc-S21.G00.60.txt` l. 14-18,
  p. 265 : « à renseigner en période de versement »). **Inférence** : la « période de versement » est le mois de la
  correction, du 1er au dernier jour (la date du versement, 50.001, en est le dernier jour). Une correction suivante
  relit ces rappels (`versement_declare::VersementDeclare::rappels_net_social`). `prive/journal/norme/corrections/PASSATION-V1o.md`.
- **Q 37** (montant négatif affiché négatif) : rien ne borne `Paie::net_social` ; `fmt_eur` et `dsn::ms` écrivent le
  signe ; le format CSL de 58.004 admet le `-`. Conforme.
- **Q 38** (aucun cumul) : le bulletin n'imprime pas de cumul du montant net social. Conforme.
- **Chèques-vacances, CESU** (tableau du II.A.1, Q 24) : dedans ; le décret les nomme après l'exception, « ainsi
  que la contribution des employeurs aux chèques-vacances », sans « de » : lues comme comptées (inférence de grammaire).
  Le catalogue livré n'en a pas.

## Vague V2g (10/10/2026) : le haut et le milieu du bulletin suivent le modèle

### Ce qui est déposé

- `legi-arrete-20160225-bulletin/` : les articles 3, 4, 5 et 6 de l'arrêté du 25/02/2016 en vigueur
  (`LEGIARTI000047098507`, `...47098456`, `...36902939`, `...36902139`) et leurs versions précédentes, relevés le
  10/10/2026 dans l'archive globale LEGI de la DILA ; transcription dans `legi-arrete-20160225-bulletin.md`.
- Déjà déposés : l'article 1 (le modèle, I) et le code du travail R. 3243-1 et R. 3243-2
  (`code-du-travail/articles-20261010.md`, l. 431-461).

### Ce que disent les textes

- **R. 3243-2** : les informations des 7°, 8°, 9°, 9° bis, 10°, 13° et 14° de R. 3243-1 « sont libellées et ordonnées
  ainsi que, pour les éléments à la charge de l'employeur, regroupées conformément à un modèle défini par arrêté » ; la
  CSG et la CRDS « sont agrégées » ; « les contributions autres que les contributions sociales mentionnées au a du 8°
  sont également agrégées dans une seule rubrique, qui donne le montant total de cette contribution ».
- **Arrêté, art. 1, I** (le modèle) : « MONTANT BRUT » ; « COTISATIONS ET CONTRIBUTIONS SOCIALES OBLIGATOIRES », colonnes
  « Base », « Taux », « Salarié », « Employeur » : « Santé » (« Sécurité Sociale Maladie Maternité Invalidité Décès »,
  « Complémentaire garanties frais de santé obligatoire »), « Accidents du travail & maladies professionnelles » (taux
  et part salariale « - »), « Retraite » (« Sécurité Sociale Vieillesse plafonnée », « ... déplafonnée », « Retraite
  complémentaire, CEG et CET T1 », « ... T2 »), « Famille » (taux et part salariale « - »), « Assurance chômage »,
  « Apec », « Autres charges dues par l'employeur » (base, taux, part salariale « - »), « Cotisations statutaires ou
  prévues par la convention collective », « CSG déductible de l'impôt sur le revenu », « CSG/ CRDS non déductible de
  l'impôt sur le revenu », « CSG/ CRDS sur les revenus non imposables » ; « TOTAL DES COTISATIONS ET CONTRIBUTIONS
  OBLIGATOIRES » ; « EXONÉRATIONS ET ALLÈGEMENTS DE COTISATIONS » ; « COTISATIONS ET CONTRIBUTIONS SOCIALES FACULTATIVES »
  (« Prévoyance, Incapacité, Invalidité, Décès, Autres », « Retraite supplémentaire ») ; puis le bas (vague 1o) ;
  « IMPOT SUR LE REVENU » a les colonnes « Base », « Taux », « Salarié », « Cumul annuel », et une ligne « Montant net des
  HC/ HS/ RTT exonérées » ; enfin « TOTAL VERSÉ PAR L'EMPLOYEUR ». VI : le montant brut et le net à payer mis en valeur.
- **Art. 3** : « Autres contributions dues par l'employeur » agrège les contributions dues uniquement par l'employeur
  (versement transport, FNAL, CSA, forfait social, taxe d'apprentissage, formation professionnelle, effort de
  construction, taxe sur les salaires, contribution au dialogue social).
- **Art. 4** : « Exonérations et allègements de cotisations » = la somme des réductions **qui ne sont pas déduites** de
  chaque cotisation ; côté employeur notamment L. 241-13 (la réduction générale), côté salarié notamment L. 241-17 (les
  heures supplémentaires et complémentaires).
- **Art. 5** : une régularisation suit la même présentation.

### Ce que le moteur en fait

`atelier/sources/documents/src/bulletin.rs`, `tableau_modele` (le rang de chaque ligne de cotisation par son identifiant :
`rang`, `libelle_du_modele`) et `blocs_un` : « MONTANT BRUT » ; les colonnes du modèle, un seul taux, celui du salarié
(R. 3243-1, 8°, a ne l'exige que pour lui ; le modèle met « - » au taux des lignes que l'employeur paie seul) ; les
lignes dans l'ordre et sous les libellés du modèle ; les autres charges en une ligne ; le total des obligatoires AVANT
les exonérations (R. 3243-1, 8°, a), la réduction générale sur la ligne des exonérations, la prévoyance sous les
facultatives ; « IMPÔT SUR LE REVENU PRÉLEVÉ À LA SOURCE » avec son assiette, son taux et son montant ; « TOTAL VERSÉ PAR
L'EMPLOYEUR » (le coût employeur, qui quitte le pied). Aucun montant ne change : les trois sommes du bulletin refont les
totaux de la paie, sinon refus. Tests : `tests/bulletin_modele.rs`, `tests/net_deja_verse.rs`
(`le_lecteur_lit_un_bulletin_ancien_de_la_copie_figee_et_un_bulletin_au_modele_de_l_arrete`).

**Lectures, à confirmer par Léandre** (aucun texte déposé ne les tranche) :
- la **prévoyance**, même obligatoire (accord interbranche des intermittents, convention d'un permanent), est rangée sous
  les facultatives : c'est la seule ligne du modèle qui la nomme. **Inférence.** L'autre lecture la mettrait dans
  « Cotisations statutaires ou prévues par la convention collective ».
- la **caisse des Congés Spectacles** (15,5 %) garde sa ligne et son libellé : le modèle ne la nomme pas, l'article 3 ne
  la cite pas parmi les « notamment ». L'autre lecture l'agrégerait aux autres charges (une contribution due par le seul
  employeur).

**Ce que le modèle demande et que le moteur ne calcule pas** (non imprimé, rien d'inventé) : le « Cumul annuel » de
l'impôt sur le revenu (art. 1, V), la ligne « Montant net des HC/ HS/ RTT exonérées » (art. 1, IV) et la réduction
salariale de L. 241-17 (art. 4), la « CSG/ CRDS sur les revenus non imposables », la « Retraite supplémentaire ».

## Lot R11 (11/10/2026) : deux modèles de bulletin, simplifié et complet

Aucun texte neuf déposé dans ce dossier : la décision se lit dans ceux qui y sont déjà, et dans le code du travail
(`code-du-travail/articles-20261010.md`, R. 3243-1 l. 431-456, R. 3243-2 l. 458-461). R. 3243-4 à R. 3243-6 ont été lus
dans l'entrepôt LEGI (`legi-entrepot/articles.jsonl.gz`) et ajoutés à la liste du relevé (`outils/relever_les_codes.py`,
domaine « bulletin ») : ils se déposeront au prochain `moissonner`.

### Ce qu'on en lit

- **R. 3243-2, al. 1** : les informations des 7°, 8°, 9°, 9° bis, 10°, 13° et 14° « sont **libellées et ordonnées** ainsi
  que, **pour les éléments à la charge de l'employeur, regroupées** conformément à un modèle ». Le modèle impose donc ses
  libellés et son ordre à tout ce qu'il nomme, et ses regroupements à la part employeur. **Aucun texte ne dit qu'on ne
  peut rien imprimer d'autre**, et aucun ne dit qu'on le peut : le modèle (art. 1, I) n'a pas de phrase sur des lignes
  en plus, et l'arrêté n'en a pas d'autre article.
- **R. 3243-1, 8°, a** : le montant et l'assiette de chaque cotisation, des deux parts ; le taux de la seule part
  salariale. Le modèle a **une** colonne « Taux », et met « - » au taux des lignes « Accidents du travail & maladies
  professionnelles » et « Famille », que l'employeur paie seul **à un taux** : la colonne « Taux » du modèle est celle du
  salarié. C'est ce qui tranche, par le texte, la première question laissée ouverte par V2g : le simplifié n'imprime pas
  le taux employeur.
- **R. 3243-2, al. 2** : la CSG et la CRDS « sont agrégées » ; les contributions autres que sociales « sont également
  agrégées dans une seule rubrique, qui donne le montant total ». **R. 3243-6** : seul le bulletin d'un salarié d'un
  **particulier employeur** peut ne pas comporter les cotisations patronales ; **R. 3243-1, 14°** : le « montant total
  versé par l'employeur » est une mention. Un modèle « salarié », sans la part employeur, n'est donc pas ouvert à une
  compagnie.

### Ce que le moteur en fait

`bulletin::Modele` et `bulletin::modele_du_bulletin` : le **simplifié** est le bulletin de V2g, inchangé
Le **complet** garde chaque ligne du modèle, son libellé, sa place, son regroupement et
ses montants ; il ajoute, **sans rien retirer**, une colonne « Taux employeur » et, **sous** la ligne regroupée « Autres
charges dues par l'employeur », une ligne « dont … » par contribution (libellé, base, taux et montant du barème). Que ce
détail ajouté sous le regroupement ne le défasse pas est une **inférence** (le regroupement reste, au même montant ; le
texte n'interdit pas d'en dire plus). La CSG et la CRDS non déductibles restent une seule ligne dans les deux modèles
(al. 2 : elles « sont agrégées ») — c'est déjà une ligne du barème. Tests : `tests/bulletin_modeles.rs`,
`tests/routes_bulletin_modele.rs`. Passation : `prive/journal/restes/PASSATION-R11.md`.

## La FAQ « Règles générales relatives au bulletin de paie » du BOSS (déposée le 13/10/2026, lot D1, pour le point 3)

- **D'où** : le BOSS refuse le conteneur ; copie de l'archive du web (Wayback Machine) de
  `https://boss.gouv.fr/portail/accueil/bulletin-de-paie/regles-generales-relatives-au-bu.html`, capture du 07/02/2025
  (`https://web.archive.org/web/20250207041321/...`). Brut : `boss-regles-generales-bulletin-de-paie-20250207-archive-web.html` ;
  texte : `.txt`. **À redéposer depuis le BOSS** quand il répond (et vérifier qu'une version plus récente ne change rien).
- **Ce qu'on en lit** (`.txt`) : Q 10 (l. 170-172) : « Il n'est pas possible d'indiquer plus de détails pour la rubrique
  « Cotisations et contributions sociales obligatoires » [...] il est autorisé de fournir le détail de certaines rubriques
  (cotisations statutaires, autres charges dues, complémentaire garantie frais de santé obligatoire, etc.) au verso du
  bulletin de paie ou bien sur un document annexe » ; les facultatives peuvent détailler. Q 11 (l. 173-174) : pas de
  lignes supplémentaires pour des contributions propres à un secteur (« ex : BTP ») : les regrouper dans les rubriques
  « autres ». Q 12 (l. 175-178) : contributions légales à la charge exclusive de l'employeur → « Autres charges dues par
  l'employeur » ; conventionnelles hors protection sociale → « Cotisations statutaires ou prévues par la convention
  collective » ; prévoyance → facultatives. Q 18 (l. 192-196) : obligatoires = rendues obligatoires par la loi ; une
  contribution rendue obligatoire par un accord de branche est facultative.
- **Où le moteur s'en sert** (lot D2, 13/10/2026) : `atelier/sources/documents/src/bulletin.rs`. **Q 10** : le modèle
  *complet* n'ajoute plus rien dans la rubrique des obligatoires (ni la colonne « Taux employeur » ni les lignes « dont … »
  de R11, que Q 10 refuse) ; son bulletin est celui du simplifié, à l'octet près, suivi d'une page annexe
  (`blocs_annexe`, document à part dans la description, sans la marque « Bulletin de paie n° ») qui détaille chaque ligne
  de la paie sous la ligne du bulletin qui la porte, avec les deux taux. L'inférence de R11 (« un détail sous le
  regroupement ne le défait pas ») est donc tranchée, contre elle. **Q 11 et Q 12** : la caisse des Congés Spectacles
  (sectorielle, légale — C. trav. L. 3141-32, D. 7121-40 —, à la charge exclusive de l'employeur) se totalise dans
  « Autres charges dues par l'employeur » (`rang`) ; elle n'a plus de ligne à elle. **Q 18** : la prévoyance de branche
  reste dans les facultatives (déjà le cas depuis V2g). Tests : `tests/calcul/bulletin_modeles.rs`
  (`le_complet_imprime_la_meme_paie_que_le_simplifie_*`, `la_caisse_des_conges_spectacles_est_une_autre_charge_due_par_l_employeur`),
  `tests/routes/routes_bulletin_modele.rs`.

## Dans le dépôt public (15/10/2026)

Ce dossier est dans le dépôt public de PuppetPaie. Les originaux suivants, d'un organisme privé, n'y sont pas : seule leur transcription y est (`pdftotext -layout` pour un PDF, une feuille de calcul en CSV, feuille par feuille, le texte d'une page HTML), et c'est elle que citent les renvois. Les condensats donnés plus haut sont ceux des originaux, gardés dans le dépôt privé (`reference-privee/sources/montant-net-social/`).

- `sage-bulletin-clarifie-2023-net-social-faq-boss.pdf` : transcription `sage-bulletin-clarifie-2023-net-social-faq-boss.txt`

## Le modèle de l'arrêté, pour lire un bulletin importé (29/10/2026)

Le lecteur des bulletins importés (`atelier/sources/moteur/src/import/bulletin_lu.rs`) range les rangs du tableau d'un bulletin,
quel que soit le logiciel qui l'a fait, par les libellés du modèle de l'article 1, I (`LEGIARTI000049854119.xml`) : les
familles (« Santé », « Retraite », « Famille », « Assurance chômage », « Autres contributions dues par l'employeur »…),
le total des cotisations, le « Montant net social », le net avant impôt, l'impôt prélevé, le net imposable. Ces libellés,
et ceux que les bulletins lus impriment à leur place, sont dans `logiciel/donnees/bulletins-lecture-base.json`
(`familles`, `resumes`). Tests : `moteur/tests/fichiers/bulletin_lu.rs`.

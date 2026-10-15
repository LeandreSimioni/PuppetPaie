# Le chômage et l'AGS d'un permanent dans la limite de quatre plafonds (lot V2e, 10/10/2026)

Ce dossier rassemble les textes sur lesquels le moteur assied l'assurance chômage (4,00 %) et l'AGS (0,25 %) d'un
salarié permanent (CDI, CDD de droit commun) sur son brut **dans la limite de quatre fois le plafond de la sécurité
sociale de sa période de paie**, au bulletin, dans la DSN (base 07, cotisations 040 et 048) et au bordereau Urssaf
(CTP 772 et 937). Pour un intermittent, la même règle est portée depuis la vague 1i : `../plafond-intermittents/`.

## Les textes, et ce qu'on en lit

### Code du travail (`../code-du-travail/`, relevé LEGI du 10/10/2026, `xml/` et `articles-20261010.md`, rubrique « L'assiette des contributions d'assurance chomage et de l'AGS »)

Relevés par `atelier/outils/relever_les_codes.py` (rubrique `chomage` du code du travail), depuis l'entrepôt LEGI de la
DILA (455 livraisons sur 455, aucune nouvelle ce jour-là).

- **L. 5422-9** (`LEGIARTI000038951969`, en vigueur depuis le 23/08/2019), avant-dernier alinéa : « Les contributions
  mentionnées aux 1° à 3° sont assises sur les rémunérations brutes dans la limite d'un plafond. »
- **L. 5422-20** (`LEGIARTI000037388454`) : les mesures d'application du chapitre « font l'objet d'accords conclus entre
  les organisations représentatives d'employeurs et de salariés » : c'est la convention d'assurance chômage et son
  règlement général, ci-dessous, qui fixent le plafond.
- **L. 5422-14** (`LEGIARTI000037388362`) : l'employeur déclare les rémunérations qui servent au calcul des contributions.
- **L. 3253-18** (`LEGIARTI000018767219`) : « L'assurance [AGS] est financée par des cotisations des employeurs assises
  sur les rémunérations servant de base au calcul des contributions au régime d'assurance-chômage. »

### Règlement général annexé à la convention du 15/11/2024 relative à l'assurance chômage (ce dossier)

- `unedic-reglement-general-convention-20241115.pdf` (et `.txt`, `pdftotext -layout`) :
  https://www.unedic.org/ged/documents/regulatory_texts/pdf/TXT-RG-Reglement_general_annexe_a_la_convention.pdf, lien
  de la page https://www.unedic.org/la-reglementation/conventions-d-assurance-chomage/reglement-general-annexe-a-la-convention-du-15-novembre-2024-relative-a-l-assurance-chomage,
  relevé le 10/10/2026 (PDF du 12/03/2025, 50 pages, sha256 `33039432673f2d38e67e97d6f762d82cb422560287105de363aac848331ebc40`).
- **Art. 49** (p. 34, `.txt` l. 1771-1778), section « Assiette » : al. 1, les contributions « sont assises sur les
  rémunérations brutes [...] entrant dans l'assiette des cotisations de sécurité sociale » ; **al. 2** : « Les
  rémunérations mentionnées au premier alinéa sont comprises dans l'assiette des contributions dans la limite d'un
  plafond fixé à quatre fois le plafond mentionné à l'article L. 241-3 du code de la sécurité sociale. »
- Art. 50-1 § 1er (p. 35) : le taux de la contribution employeur, 4 %.

### Unédic, déjà déposés (`../plafond-intermittents/`)

- Circulaire 2025-05, fiche technique 1.1 : « Les rémunérations dépassant 4 fois le plafond [...] demeurent exclues de
  l'assiette des contributions ».
- Fiche « Assiette des contributions » (mise à jour du 05/10/2026, `unedic-fiche-assiette-des-contributions-20261010.txt`
  l. 24-33) : l'assiette est celle des cotisations de sécurité sociale ; « Ce principe d'identité d'assiette s'applique
  aux cotisations AGS » ; « L'assiette des contributions ne peut dépasser 4 fois le plafond mensuel de la sécurité
  sociale, soit 16 020 € au 1er janvier 2026. »

### Code de la sécurité sociale (`../code-de-la-securite-sociale/`, déjà relevé)

- **R. 242-2, al. 3** : « Lorsque le contrat de travail d'un salarié ne couvre pas l'intégralité des périodes [...], les
  plafonds [...] sont réduits à due proportion du nombre de jours de la période pendant laquelle les personnes sont
  employées. »

### Urssaf, tableau d'équivalence des données individuelles et agrégées (ce dossier)

- `urssaf-equivalence-dida-20260611.csv` : jeu `equivalence-dida` d'open.urssaf.fr (« Tableau équivalence données
  individuelles et agrégées », ODbL, mis à jour le 11/06/2026), exporté le 10/10/2026 par
  https://open.urssaf.fr/api/explore/v2.1/catalog/datasets/equivalence-dida/exports/csv?delimiter=%3B&use_labels=true
  (sha256 `043c5e1752074dddfe3e19b89285aeeca516e2736902a09ea7255dde7479bf9d`, gardé tel que servi). C'est le
  « tableur nommé Guide_Urssaf_equivalenceDIDA » auquel renvoie le guide Urssaf de déclaration en DSN, § 1.2
  (`../editeur-dsn/guide-urssaf-regularisation.pdf`), lui-même cité par la norme DSN 2026.1, § 2.2.1.5
  (`norme-dsn/texte/chapitre-2-principes-de-constitution.txt` l. 1208-1213).
  - l. 1640 : `772;CONTRIBUTIONS ASSURANCE CHOMAGE U2;E;2020-01-01;07;;040;...;772;920;...` : le CTP 772 se rattache à
    la base assujettie **07**, cotisation **040**, et se déclare au bloc 23 sous le qualifiant **920**.
  - l. 280 : `937;COTISATIONS AGS CAS GENERAL U2;E;2021-01-01;07;;048;...;937;920;...` : de même pour l'AGS, base 07,
    cotisation 048, qualifiant 920.

### Norme DSN 2026.1 (`../../norme-dsn/texte/`)

- `bloc-S21.G00.78.txt` l. 209-213 (p. 294) : « 07 - Assiette des contributions d'Assurance Chômage ».
- `bloc-S21.G00.23.txt` l. 35-36 (p. 168) : « 920 - Autre assiette », « 921 - Assiette plafonnée ».

### La même règle en 2025 (lot R8, 11/10/2026)

Le règlement général art. 49 vaut pour toute l'année 2025, et la règle des quatre plafonds avant lui :

- Circulaire Unédic 2025-05 du 1er mai 2025 (déjà déposée,
  `../plafond-intermittents/unedic-circulaire-2025-05-contributions-20250501.txt`), préambule, l. 33-37 : « La
  convention du 15 novembre 2024 relative à l'assurance chômage a été agréée par arrêté du Premier ministre en date du
  19 décembre 2024 [...]. Elle fixe la règlementation d'assurance chômage applicable à compter du 1er janvier 2025,
  relative tant à l'indemnisation qu'aux contributions d'assurance chômage. »
- Même circulaire, fiche technique 1.1, l. 109-117 : « L'article 49 du règlement général annexé à la convention du
  15 novembre 2024 [...] » ; « Les rémunérations dépassant 4 fois le plafond du régime d'assurance vieillesse de la
  sécurité sociale visé à l'article L. 241-3 du code de la sécurité sociale **demeurent** exclues de l'assiette des
  contributions (15 700 € par mois, valeur au 1er janvier 2025). » Le verbe dit que la règle précédait la convention
  (le texte antérieur, le règlement annexé au décret 2019-797 que la circulaire cite pour le bonus-malus, n'est pas
  déposé ; il ne sert pas : l'assiette des paies de 2025 relève de la convention du 15/11/2024).
- C. trav. L. 5422-9 (en vigueur depuis le 23/08/2019) et L. 3253-18 (depuis le 01/05/2008) couvrent 2025 (dates de
  `../code-du-travail/articles-20261010.md`, l. 578 et 600) ; CSS R. 242-2 depuis le 01/01/2020.
- 15 700 € = 4 × 3 925 €, le PSS mensuel 2025 de `logiciel/donnees/baremes/2025.json` (`plafond_securite_sociale`).

## Où le moteur s'en sert

- `atelier/sources/paie/src/calcul.rs`, `calculer` (7.6, branche d'une nature mensuelle) : `brut_chomage` =
  min(brut, 4 × `pss_periode`), le même nombre que `apec_mensuelle` (l'Apec d'un cadre permanent, quatre plafonds aussi).
- `logiciel/donnees/baremes/2026.json` (lot V2e) et `logiciel/donnees/baremes/2025.json` (lot R8),
  `par_nature.permanent` : la rubrique `chomage` sur `brut_chomage` (note `_base`) ; les cotisations DSN 040 et 048,
  `base` et `base78` sur `brut_chomage` (base 07).
- `atelier/sources/paie/src/dsn.rs` : `base_07` (la base 07 que le nominatif d'un contrat déclare), `assiettes_ctp`
  (`chomage_permanent` = la somme des bases 07 des permanents), `assiette_declaree` (la base 07 déposée, pour une
  régularisation), `valeur_ctp` (`CTP_CHOMAGE_PERMANENT` : un fichier des organismes composé avant le lot, qui met 772 et
  937 sur `brut_permanent`, refuse dès que ce brut diffère de la base 07).
- `logiciel/donnees/organismes-base.json` : les CTP 772 et 937 sur `chomage_permanent`.

## Ce qui est une inférence

- **Inférence** : « quatre fois le plafond mentionné à l'article L. 241-3 » se lit, pour un permanent qui entre ou sort
  en cours de mois, comme quatre fois le plafond de sa période d'emploi (R. 242-2, al. 3), comme pour un intermittent
  (`../plafond-intermittents/LISEZ-MOI.md`). Les textes ne nomment pas ce cas.
- **Inférence** : le montant d'assiette du CTP 772 (et du 937) au bloc 23 est la somme des bases 07 des permanents ; le
  tableau d'équivalence rattache le CTP à la base 07 sans écrire la somme, que le guide Urssaf écrit pour le CTP 100
  (« le montant d'assiette [...] 920 est égal à la somme des montants de base assujettie [...] 03 », fiche 1).

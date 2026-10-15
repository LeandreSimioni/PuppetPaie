# Le bloc S21.G00.44 (assujettissement fiscal) : d'où vient chaque code

Lot V3b des corrections DSN, relevé le 10/10/2026. Ce dossier ne porte pas de texte brut : chaque texte est déposé dans
le dossier de son émetteur (ci-dessous), et ce LISEZ-MOI dit ce que le moteur en lit et où. Le moteur :
`atelier/sources/paie/src/assujettissement.rs` (`blocs`), appelé par `dsn::generer` après les bordereaux (22/23) et
avant les individus (30) ; les tests : `atelier/sources/paie/tests/bloc_44.rs`.

## La norme (cahier technique DSN 2026.1, `../../norme-dsn/texte/`)

| où | ce qu'on en lit |
|---|---|
| `chapitre-2-principes-de-constitution.txt`, § 2.3.1, p. 58, l. 1954-2017 | « L'assujettissement des entreprises est déclaré en DSN une fois par an, soit en fin d'exercice, soit en cours d'exercice pour les cas de cessation d'activité. [...] cet assujettissement est déclaré par établissement de l'entreprise. [...] L'assujettissement ou le non-assujettissement doit obligatoirement être déclaré. [...] Hors cas de cessation d'activité de l'entreprise, le renseignement de ce bloc doit intervenir, au plus tard, dans la DSN mensuelle de décembre ». Puis, taxe par taxe : TA (assiette des cotisations de sécurité sociale), contribution supplémentaire (250 salariés, 5 % d'alternants), PEEC (cinquante salariés, CCH L. 313-1 ; 0,45 % des rémunérations « payées au cours de l'année précédente »). |
| même fichier, p. 59, l. 2020-2059 | CFP (0,55 % / 1 % ; 2 % pour les intermittents du spectacle), CPF-CDD (1 %), taxe sur les salaires (« due par les entreprises qui ne sont pas assujetties à la taxe sur la valeur ajoutée » ; assiette de la CSG sans la réduction ; « Les bases imposables aux taux majorés sont déclarées annuellement, uniquement au niveau de l'établissement »). |
| `bloc-S21.G00.44.txt`, p. 240, et la tête de `bloc-S21.G00.45.txt`, p. 241 | les codes 001 à 016 ; « Si une entreprise n'est pas assujettie à une taxe, le motif correspondant [...] doit être renseigné » ; 009 porte « le montant de l'assiette de la taxe sur les salaires au taux normal pour l'ensemble des salariés », et « autant de blocs [...] que nécessaire » pour les taux majorés (015, 016). CCH-11 de 44.001 : montant obligatoire pour 005, 009, 015, 016 ; CCH-12 : interdit pour 002, 004, 006, 008, 010, 014. 44.003 : le millésime, au plus l'année du mois principal. 44.004 : obligatoire pour 002, valeurs de la table MNATA (déposée le 11/10/2026, `../dsn-nomenclatures/table-MNATA-20260924.txt` ; livrée dans `logiciel/donnees/mnata-base.json`, lot R2). |
| `chapitre-8-arborescences.txt`, l. 64-72 | l'ordre : 20, 55, 22, 23, **44**, 30. |

## Les codes (base LEGI de la DILA, par `atelier/outils/relever_les_codes.py`, liste `TEXTES`)

| texte | dossier | ce qu'on en lit | où le moteur s'en sert |
|---|---|---|---|
| C. trav. L. 6131-1, L. 6131-2 | `../code-du-travail/articles-20261010.md` | tout employeur doit la CFP, la CPF-CDD, la contribution supplémentaire, sauf ceux du II (État, collectivités et leurs EPA, hôpitaux publics, employeur sans établissement en France) | 007/008 : ce que le bordereau paie (CTP 983, 959, 971) |
| C. trav. L. 6241-1 (version du 21/02/2026), L. 6241-1-1 | idem | qui doit la TA ; son assiette ; 0,44 % en Alsace-Moselle (L. 6241-1-1, II) | 001 : CTP 992 ou 993 au bordereau ; sinon 002 et le motif MNATA que la fiche dit (`taxe_apprentissage_motif_non_assujettissement`, lot R2), refus sans lui ; 007 refusé à partir du millésime 2026 (`taxe_d_apprentissage`) |
| C. trav. L. 6241-1, rédactions du 01/01/2022 et du 16/02/2025 (relevées le 11/10/2026, lot R2) | `../code-du-travail/xml-anterieur/L6241-1-anterieures-20261011.md` | ni l'une ni l'autre ne nomme les associations à activités non lucratives (inférence : non redevables avant le 21/02/2026, motif MNATA 007) | `MILLESIME_ASSOCIATIONS_REDEVABLES` |
| Loi n° 2026-103 de finances pour 2026, art. 135, XIII | `../loi-de-finances-2026/articles-20261010.md` | l'article qui a réécrit L. 6241-1 : les associations à activités non lucratives deviennent redevables de la TA | aucun code : il dit pourquoi le moteur ne déduit pas la TA du statut d'association (voir la passation) |
| C. trav. L. 6242-1 | `../code-du-travail/articles-20261010.md` | contribution supplémentaire : 250 salariés, quotient d'alternants sous 5 % « au cours de l'année de référence » | 004 sous 250 ; au-delà, le champ `contribution_supplementaire_apprentissage` de la fiche, sinon refus |
| C. trav. L. 6331-1, L. 6331-3 | idem | CFP à 0,55 % sous onze salariés, 1 % au-delà | 007 |
| C. trav. L. 6331-6, D. 6331-72, L. 6321-9 | idem | CPF-CDD : 1 % pour les titulaires d'un CDD, sauf saisonniers, contrats aidés, apprentissage, professionnalisation, L. 6321-9 | `ouvre_cpf_cdd` : 013 si un CDD soumis dans l'année, 014 sinon ; refus pour un CDD sans motif ou sous un dispositif de politique publique. Depuis le lot R2 (11/10/2026), la même règle (`contrat_ouvre_cpf_cdd`) fait payer le 1 % : bulletin (`bareme::bareme_cpf_cdd`, section `par_nature.permanent.cpf_cdd` du barème), cotisation 129, CTP 987 (DIDA l. 1001) |
| C. trav. L. 6331-55 | idem | « Par dérogation [...] à l'article L. 6331-6 » : la participation unique des employeurs d'intermittents du spectacle | le CDD d'usage (motif 05) n'ouvre pas la CPF-CDD (INFÉRENCE : un CDDU de compagnie est un contrat d'intermittent) |
| CCH L. 313-1 | `../code-de-la-construction-et-de-l-habitation/articles-20261010.md` | PEEC : cinquante salariés, 0,45 % des revenus d'activité de l'exercice écoulé, effectif selon L. 130-1 CSS | 005 avec l'assiette de l'année (bases 03), 006 sinon |
| CSS L. 130-1 | `../code-de-la-securite-sociale/articles-20261010.md` | effectif annuel = moyenne de « l'année civile précédente » ; seuil franchi après cinq années consécutives | `Effectif::atteint(seuil, an + 1)` pour la PEEC et la contribution supplémentaire |
| CGI 231, 1679 A | `../code-general-des-impots/articles-20261010.md` | taxe sur les salaires : qui la doit (TVA, franchise de l'art. 293 B), son assiette (CSG sans la réduction), ses taux majorés (2 bis) | 009/010 depuis la fiche (`taxe_sur_les_salaires.assujettie`, `pourcentage_assujettissement`) ; 015/016 (`fractions_majorees`) |
| CGI 235 bis | idem | la cotisation de 2 % de l'employeur qui n'a pas investi sa PEEC | aucun code : relevé parce que la norme le cite (p. 58) |

## Le BOFiP (`../bofip/`)

`BOI-TPS-TS-30-20200624.html` (§ 40, 110, 240), `BOI-TPS-PEEC-10-20141218.html` (§ 40, 50),
`BOI-TPS-PEEC-20-20141218.html` (§ 1, 10, 20) ; transcription des paragraphes utiles et usage dans le moteur :
`../bofip/BOI-TPS-TS-30-et-PEEC-extrait-bloc-44.md`.

## La forme, vue chez l'ancien logiciel

Les DSN de décembre exportées de CoolPaie par une compagnie réelle (2020 à 2025 ; pièces hors du dépôt public : dépôt privé) : six blocs dans chaque DSN de décembre, `001`,
`004`, `006`, `007`, `010`, `013`, chacun avec son millésime, un `44.002` à `0.00` sur 001, 007 et 013. Lu pour la
forme, pas pour les valeurs. Le moteur n'écrit pas de montant là où la norme n'en demande pas, et déclare `014` pour une
compagnie qui n'emploie que des intermittents (L. 6331-55), là où CoolPaie écrivait `013`.

## Les inférences (un texte appliqué à un cas qu'il ne nomme pas)

- 015 et 016 sont les assiettes au premier (8,50 %) et au second (13,60 %) taux majorés, au-delà des seuils de
  l'article 231, 2 bis.
- La part assujettie (`pourcentage_assujettissement`) s'applique aux montants 009, 015 et 016 (BOFiP TS-30, § 240).
- Le montant du 005 est l'assiette des salaires de l'année du millésime ; l'effectif qui déclenche 005, comme celui de
  la contribution supplémentaire, est celui de cette année-là (L. 130-1 : l'année civile qui précède l'obligation, qui
  porte sur l'année d'après).
- 013 ne se déclare que si l'année a eu au moins un CDD soumis à la contribution ; 014 sinon.
- 001 et 007 disent ce que le bordereau de la compagnie paie (`config/organismes.json`).

## Lot D1 (13/10/2026) : la taxe non due sort aussi du bulletin et du nominatif ; l'association la doit dès mars 2026

- **Guide Urssaf des contributions de formation et d'apprentissage, 12/03/2026** (déjà déposé :
  `../majoration-caisses-conges/urssaf-guide-contributions-formation-apprentissage-20260312.txt`, l. 1166-1176 et
  1192-1196) : « À compter de la période d'emploi de mars 2026, conformément au XIII de l'article 135 de la loi
  n° 2026-103 [...] la taxe d'apprentissage est également due par les associations [...] à activités non lucratives ».
  L'inférence de R2 (le millésime 2026 entier) se resserre : janvier et février 2026 ne la portent pas ; le millésime
  2026 se déclare toujours 001 (mars à décembre).
- **C. trav. L. 6241-1, I et III** : qui doit la taxe, qui n'en est « pas redevable ». Une taxe non due ne se calcule
  pas : la décision du mois (`assujettissement::taxe_apprentissage_non_due_au_mois`) retire la ligne du bulletin
  marquée `"taxe_apprentissage": true` (barèmes 2025 et 2026, `autres_3`) et la cotisation 130 du bloc 81
  (`bareme::bareme_taxe_apprentissage`, un étage de la pile), et les CTP 992/993 du bordereau (`dsn::lignes_bordereau`,
  `lignes_regularisees`). Le bloc 44 et le solde d'avril gardent la décision du millésime (`taxe_apprentissage_non_due`).
- Test : `tests/dsn/bloc_44.rs`, `la_taxe_non_due_ne_se_calcule_ni_au_bulletin_ni_au_nominatif_ni_au_bordereau`.

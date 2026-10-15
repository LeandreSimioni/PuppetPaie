# Les nomenclatures de la DSN (NEODeS, P27V01) — le fichier cumulé du 24/09/2026

## D'où ça vient

`net-entreprises-neodes-nomenclatures-P27V01-20260924.csv` : le fichier `NEODESP27_CUMUL_P27V01.csv` tel que servi,
**transmis par Léandre le 11/10/2026** depuis le poste, puis **relevé à l'identique par le conteneur le même jour**
(même empreinte) depuis la page des nomenclatures P27V01 : `https://www.net-entreprises.fr/nomenclatures-dsn-p27v01/`,
lien « fichier cumulé » `https://portail.net-entreprises.fr/nmcl/telechargeFichierGenere/NEODESP27_P27V01_NEODESP27_CUMUL_P27V01.csv`
(la page sert aussi chaque table en `.csv` séparé, `…/telechargeFichierCsv/NEODESP27-<TABLE>-P27V01.csv`). Il porte en
tête la date **24/09/2026** (première cellule de l'en-tête) ; c'est la date du relevé retenue dans le nom.

`net-entreprises-neodes-nomenclatures-P26V01-20260924.csv` : le même fichier pour la **P26V01** (page
`https://www.net-entreprises.fr/nomenclatures-dsn-p26v01/`, donnée par Léandre ; lien
`…/telechargeFichierGenere/NEODESP26_P26V01_NEODESP26_CUMUL_P26V01.csv`), relevé par le conteneur le 11/10/2026, même
date en tête (24/09/2026), 177 032 lignes. Différences avec le P27V01 : la table **ERADS** (11 lignes) n'existe qu'en
P27V01, et IDCC a 792 lignes en P26V01 contre 789 ; **MNATA, PAYS (avec XK Kosovo) et PAY sont identiques** dans les
deux versions : ce que D4 et D5 disent vaut dès les DSN 2026.
Empreinte SHA-256 (16 premiers caractères) : `e6db327b2cd8341f`. **Ne pas convertir** : latin-1, séparateur `;`, fins
de ligne telles quelles ; il se compare octet pour octet.

Dessin : `<table>;<code>;<libellé>;<trigramme>;<date de début de validité>;<date de fin de validité>`, 177 040 lignes,
38 tables. À côté, en UTF-8, les tables qu'on lit (extraites par `iconv` + `awk`, sans rien d'autre) :

| fichier | table(s) | lignes |
|---|---|---|
| `table-MNATA-20260924.txt` | MNATA, motifs de non-assujettissement à la taxe d'apprentissage (`S21.G00.44.004`) | 19 |
| `tables-PAYS-et-PAY-20260924.txt` | PAYS (274 codes) et PAY (261 : les mêmes sans la France, Monaco, les DROM et COM) | 535 |
| `table-PCSESE-20260924.txt` | PCS-ESE 2017 (`S21.G00.40.004`) | 430 |
| `table-IDCC-20260924.txt` | IDCC (`S21.G00.40.017`), avec dates | 789 |
| `tables-organismes-20260924.txt` | OPCO, PREV, MUT, ASSU, OCI, IVO, CPCP, RRC (organismes des blocs 15, 20, 70, 71, 81) | 1 058 |

Les autres tables du fichier (non transcrites) : RAT (codes risque AT, 512), NAF, INSEE (communes, 36 816), HEX
(codes de la Hexaposte), EFE (établissements de la fonction publique, 115 002), BAR (barème du PAS par année et
tranche), NNE, NET, NEHMED, NEHNMED, ART, DET, CMET, CCLA, CCP, FESAIEG, AAETH, ERADS, IBC, ICI, ODRT, DLG*.

## Ce qu'on en lit

### D4 — la table MNATA (`S21.G00.44.004`)

Dix-neuf motifs, 001 à 019, sans date. Ceux qui peuvent concerner une compagnie :
- **007 « Associations sans but lucratif »** ;
- 008 « Syndicats professionnels » ; 001 « Non assujetti par choix du régime fiscal (option IR en catégorie BNC) » ;
  011 « Personnes physiques ou morales assujetties aux BNC ou BA » ; 014 « Les groupements d'employeurs ».

Ce que ça change : `paie/src/assujettissement.rs` écrit que le dépôt n'a pas la table et refuse le code 002 (non
assujetti à la TA) faute de motif. La table est là ; **rien n'est codé** (séance du 11/10/2026 : on dépose, on ne
construit pas). Reste la question Q8 de `prive/journal/norme/corrections/A-RECUPERER-ET-A-TRANCHER.md` : une
association à activité non lucrative avant le 21/02/2026 déclare-t-elle 002 + MNATA 007, ou 001 ? La table ne le dit
pas (elle liste les motifs, pas les conditions) ; c'est L. 6241-1 (déposé dans `../code-du-travail/`) qui tranche.

### D5 — la table PAYS (`S21.G00.30.015`, pays de naissance) et PAY

PAYS a **274 codes**, PAY 261 : PAY = PAYS moins FR, MC, BL, GF, GP, MF, MQ, NC, PF, PM, RE, WF, YT (la France, Monaco
et les DROM-COM). Les deux ont **XK KOSOVO**. Aucune date de validité sur les lignes. Les treize codes que PAY n'a pas sont
exactement ceux que la fiche net-entreprises 2951 interdit en `30.015` pour un individu né à l'étranger (« une valeur
différente de FR, GP, BL, MF, MQ, GF, RE, PM, YT, WF, PF, NC, MC », `../net-entreprises-base-connaissances/`, fiche 2951,
l. 45) : **PAY est la liste des pays de naissance d'un individu né à l'étranger**, PAYS la liste complète (inférence).

Confrontée à `logiciel/donnees/pays-base.json` (213 codes, lus du COG 2026, `../insee-cog-pays/`) : les 213 codes du
moteur sont tous dans PAYS ; PAYS a **61 codes de plus**, de trois sortes :
- **États disparus**, que le COG n'a pas : AN Antilles néerlandaises, BU Birmanie, CS Tchécoslovaquie, DD RDA, RH
  Rhodésie du Sud, SU URSS, TP Timor oriental, YU Yougoslavie, ZR Zaïre, DY, HV, NH, VD, YD, NT, PZ, PU, PC, JT, MI,
  WK, NQ, FQ, CT ;
- **XK Kosovo** ;
- **territoires** que le COG rattache à un État (BL, BM, BV, CC, CX, FK, GG, GS, GU, HM, IM, JE, KY, MP, MS, NF, PR,
  SJ, TC, TF, TK, UM, VG, VI, AI, AS) et les **DROM-COM** (GF, GP, MQ, RE, YT, NC, PF, PM, WF, MF).

Ce que ça change : un salarié né au Kosovo, en URSS ou en Yougoslavie a un code que la DSN accepte ; le moteur ne le
propose pas (V1k, V2h : « un pays de naissance hors liste refuse »). **Rien n'est codé** ; à ouvrir en lot : lire PAYS
à la place du COG pour `30.015`, ou l'ajouter au COG (le COG garde le pays de rattachement, qui sert à la codification
UE de `30.013`, `paie/src/codification_ue.rs`). `../insee-cog-pays/LISEZ-MOI.md` et `../union-europeenne/LISEZ-MOI.md`
attendaient cette table (« Reste à déposer ») : elle est ici.

### Les autres tables lues en passant

- PCSESE : 353b, 353c, 354b, 354c, 354e, 354f, 465b, 637c ont les libellés des fiches INSEE déposées
  (`../insee-pcs-ese-2017-fiches/`), sans date.
- IDCC : 1285 (01/01/1984), 3090 (01/07/2013), 1790, 1922, 2121, 3241 présents, sans date de fin.
- OPCO : `02` = AFDAS. IVO : `44832375800038` AUDIENS (AAR, retraite), `P0983` AUDIENS PREVOYANCE (CTI) ; PREV :
  `P0983`. À confronter aux codes que le moteur écrit en `S21.G00.20.001` et `.70.002` (non fait ce 11/10).
- BAR : le barème du PAS par année (2017 à 2026), `année/grille/tranche/de/à`, avec le taux ; celui du moteur vient du
  BOFiP (`../bofip/BOI-BAREME-000037-*`), à comparer un jour.

## Où le moteur s'en sert

- **D5 (lot R2, 11/10/2026)** : la table PAYS (et l'appartenance à PAY) est livrée dans
  `logiciel/donnees/pays-dsn-base.json` (sa note `_source` cite `tables-PAYS-et-PAY-20260924.txt`, vérifié identique aux
  lignes PAYS/PAY du cumul P27V01 brut). `S21.G00.30.015` s'y lit (`paie/src/pays.rs`, `de_naissance`,
  `de_la_table_dsn`) ; la fiche choisit le pays de naissance dans cette liste (`etat.pays_naissance`,
  `pays::servir_naissance`) ; le questionnaire lit aussi son libellé (`naissance_du_nom`). Le pays de nationalité garde
  le COG (la table ne dit ni l'UE ni le rattachement d'un territoire, d'où se déduit `30.013`).
- **D4 (lot R2, 11/10/2026)** : la table MNATA est livrée dans `logiciel/donnees/mnata-base.json` (vérifiée identique
  aux lignes MNATA du cumul P27V01 brut) ; `paie/src/assujettissement.rs` (`taxe_d_apprentissage`, `motifs_mnata`)
  écrit 002 et `44.004` avec le motif de la fiche entreprise quand le bordereau ne paie pas la taxe. Q8 : l'ancienne
  rédaction de L. 6241-1 est relevée (`../code-du-travail/xml-anterieur/L6241-1-anterieures-20261011.md`).

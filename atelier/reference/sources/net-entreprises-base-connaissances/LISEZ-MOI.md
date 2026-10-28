# La base de connaissances net-entreprises (fiches consignes DSN)

Les fiches de `net-entreprises.custhelp.com` (« Base de connaissances net-entreprises »), relevées depuis le conteneur
quand le site répond (il répond le 11/10/2026 : Léandre a donné l'adresse, le relevé est fait par `curl`). Chaque fiche :
le brut (`.html`, tel que servi) et sa transcription (`.txt`, le corps de la réponse, tableaux aplatis en lignes `|`).
Nommage : `net-entreprises-fiche<n°>-<objet>-<date de modification AAAAMMJJ>`.

## Fiche 2556 — Cotisations de retraite complémentaire Agirc-Arrco dues à partir du 01/01/2023

- URL : https://net-entreprises.custhelp.com/app/answers/detail_dsn/a_id/2556/
- Relevée le 11/10/2026 ; « Date de modification : 08/07/2026 » ; versions en vigueur listées : 05/02/2026, 15/12/2025,
  08/12/2025, 28/05/2025, 16/03/2022 (la version servie est la plus récente).
- C'est la fiche à laquelle renvoie le guide déclaratif Urssaf du 04/06/2026 (`sources/editeur-dsn/`), demandée par la
  passation V2d et par `ETAT.md` (« à récupérer »).

**Ce qu'on en lit** (`.txt`, lignes citées) :
- l. 13-49 : vers l'Agirc-Arrco, **131** (retraite complémentaire + CET + CEG, T1 sous la base **02** — « 24 » ou
  « 43 » dans les cas spécifiques —, T2 sous la base **03** — « 22 » ou « 23 » dans les cas spécifiques) et **132**
  (Apec, sous 03), **sans `81.002`** ; « 105 […] supprimé en version de norme P27V01. Ce code ne doit plus être utilisé »
  (l. 49).
- l. 55-71 : vers l'Urssaf (fiabilisation de la RGDU), **142** sous la base 02 (**43 pour les techniciens
  intermittents du spectacle**, l. 63), `81.002` = **SIRET de l'Urssaf**, montant = part patronale T1 de la retraite
  complémentaire + part patronale T1 de la CEG, **taux réel** en `81.007`.
- l. 73-81 : **146** sous la base 03 (22 ou 23 dans les cas spécifiques) = part patronale T2 retraite complémentaire +
  part patronale T2 de la CEG ; **en 2026, la CET T1 et T2 y est aussi** (transitoire), en un seul bloc ou, toléré, en
  plusieurs blocs 146 (l. 493-503) ; deux options pour la CET en 2026 : tout sous 03, ou T1 sous 02 et T2 sous 03
  (l. 505-509).
- l. 83-89 : **148** (P27V01) = part patronale de la CET T1 et T2, **entièrement sous la base 03**, `81.002` = SIRET de
  l'Urssaf (exemple cas n° 1, janvier 2027 : « 8,82 », « SIRET de la caisse Urssaf ou MSA »). Un salarié toujours sous
  le PMSS n'a ni 146 ni 148.
- l. 1132-1164 : régularisations d'une période ≥ janvier 2026 : corriger le 131 si la cotisation est erronée, le 142
  si la part patronale T1 change (assiette, taux…), et à partir de 2027 le 146 (part patronale T2) et le 148 (CET) ;
  une erreur de répartition salarié/employeur ne corrige que le 142. Renvoi à la fiche 2398 (`81.007`).

**Où le moteur s'en sert** :
- `atelier/sources/paie/src/dsn.rs`, `parts_patronales_agirc_arrco` (lot V2d, `PASSATION-V2d.md`) : 142 après le 131 de
  T1, 146 après celui de T2, un 146 à part pour la CET (l'option « plusieurs blocs » de 2026, l. 497-503) ; Urssaf en
  `81.002`, taux de la ligne du bulletin. **À vérifier** (vu en déposant, non fait) : la base du 142 d'un technicien
  intermittent doit être la **43**, pas la 02 (l. 63).
- `src/norme.rs`, `Norme::cotisation_cet_patronale` (lot V4a, `PASSATION-V4a.md`) : en P27V01 la CET patronale
  **refuse** faute de texte. **Cette fiche le donne** : code 148, sous la base 03, `81.002` = SIRET de l'Urssaf
  (l. 83-89 et cas n° 1) — réponse à la question Q15 de `A-RECUPERER-ET-A-TRANCHER.md`. **Codé par le lot R5b
  (11/10/2026)** : `logiciel/donnees/norme-dsn-base.json`, P27V01, `cotisation_cet_patronale = "148"` (`_source_cet` cite
  cette fiche et le tableau des combinaisons P27V01 de la fiche 1265, l. 88, 150, 152 : 148 sous 03, 22, 23) ; la CET
  part sous la base 78 du 131 qui la porte (type 23 dans les barèmes livrés), à l'Urssaf. Tests :
  `paie/tests/norme_par_fichier.rs`, `la_cet_d_un_cadre_part_en_146_en_p26v01`,
  `la_cet_d_un_cadre_part_en_148_sous_la_base_deplafonnee_en_p27v01`, `la_cet_fondue_d_un_cadre_intermittent_part_en_148_en_p27v01`.
- La « base 23 » que V2d laissait ouverte : la fiche ne la réserve qu'aux « cas spécifiques » ; le cas nominal est la
  base 03 (l. 43, 75, 85).

## Les autres fiches relevées le 11/10/2026 (déposées, PAS ENCORE LUES : à lire le jour où le point se code)

Léandre : « on met juste les documents utiles », « ne relis pas ». Chaque fiche est là en brut et en transcription ;
ce qu'on en lira s'écrira ici, fiche par fiche, au moment de s'en servir. Catégories parcourues : norme 2026 et 2027,
net social, intermittents, Agirc-Arrco, identification, contrats, Urssaf, entreprise. Les fiches de la catégorie
« La norme DSN 2027 » n° 3355 à 3358 et 3382 n'ont pas répondu (titre vide, trois essais) : à reprendre.

| n° | titre (date de modification) | sert à | fichier |
|---|---|---|---|
| 2956 | Changement des modalités du contrat - S21.G00.41 | V3c : le bloc 41 (changements contrat) ; Q9 (changement en cours de mois) | `net-entreprises-fiche2956-bloc-41-changement-modalites-contrat-20250429` |
| 2955 | Déclarer une mutation ou un transfert de contrat | V3c : 41.012 (mutation d'un établissement à l'autre), clôture dans l'ancien établissement | `net-entreprises-fiche2955-mutation-transfert-contrat-20250818` |
| 1952 | Modalité déclarative de correction d'une date de début de contrat | V3c : 41.021 ; 21/10/2026 : la correction tardive d'une date de début, portée (voir « Fiche 1952 » plus bas) | `net-entreprises-fiche1952-correction-date-debut-contrat-20250326` |
| 3272 | La suspension temporaire du contrat de travail en DSN | V4b : le bloc 65 ; Q17 (congé familial non payé) | `net-entreprises-fiche3272-suspension-contrat-bloc-65-20260610` |
| 2952 | Le numéro technique temporaire - NTT | V3d : le NTT déclaré qui accompagne le NIR | `net-entreprises-fiche2952-ntt-20250703` |
| 2953 | Différence entre un NIR, un NIA et un NTT | V3d : NIA → NIR en 31.008 | `net-entreprises-fiche2953-nir-nia-ntt-20250323` |
| 1957 | Régularisation suite à l’utilisation à tort du NTT à la place du NIR | V3d : un NTT utilisé à tort | `net-entreprises-fiche1957-regularisation-ntt-a-tort-20250703` |
| 2950 | Identification des individus : les remontées et corrections | V3d : le bloc 31, les corrections d'identité | `net-entreprises-fiche2950-identification-remontees-corrections-20250703` |
| 2627 | DSN - Identification des individus – Remontées au sein du CRM Identité et corrections | V3d : le CRM identité (nature 121), non exploité | `net-entreprises-fiche2627-crm-identite-corrections-20250703` |
| 2951 | Lieu de naissance : code pays et département | V1k/V2h : 30.015, le code pays de naissance (D5, table Pays) | `net-entreprises-fiche2951-lieu-naissance-code-pays-departement-20250703` |
| 2638 | Modalités déclaratives du lieu de naissance | V1k/V2h : 30.015, 30.029 ; D5 | `net-entreprises-fiche2638-lieu-de-naissance-modalites-20250703` |
| 2983 | Le montant net social - MNS | V4b : la datation de 58.001/.002 en 2027 (Q, D8) | `net-entreprises-fiche2983-montant-net-social-20261009` |
| 3392 | "Fiche chapeau" Modalités déclaratives et fiabilisation du montant net social (MNS) | V4b : le net social, fiche chapeau | `net-entreprises-fiche3392-mns-fiche-chapeau-20260814` |
| 3386 | Déclaration de la part exonérée de cotisations sociales des indemnités de rupture en bloc « Élément de revenu calculé en net -S21.G00.58 » | V4b : le type 13 du bloc 58 | `net-entreprises-fiche3386-indemnites-rupture-part-exoneree-bloc-58-20260703` |
| 3323 | Déclaration des indemnités journalières subrogées prises en compte dans le calcul du montant net social (MNS) | V4b : le type 14 du bloc 58 (vu sans faire) | `net-entreprises-fiche3323-ij-subrogees-mns-20260707` |
| 2963 | Contribution Supplémentaire à l’Apprentissage (CSA) | V3b : 44.003/004, CTP 998 (vu sans faire) | `net-entreprises-fiche2963-contribution-supplementaire-apprentissage-20250326` |
| 1265 (suite) | **Les tableaux de combinaisons 78/79/81 que la fiche cite**, relevés le 11/10/2026 depuis `www.net-entreprises.fr/media/documentation/` (le site répond au conteneur ce jour) : `net-entreprises-fiche1265-combinaisons-78-79-81-p27v01-20260630.xlsx` (finalisé le 30/06/2026, SHA-256 `c42d9c8ac5814bbb`) et `…-p26v01-20250716.xlsx` (16/07/2025, `dff52c59203e5dcf`), une feuille par organisme ; transcrites en `.txt` pour URSSAF CN, AGIRC-ARRCO, CONGES SPECT, FRANCE TRAVAIL, OC, Caisse Congés Payés, DGFiP. La fiche elle-même, imprimée en PDF par Léandre le 11/10 (`…-20260708.pdf`), est la même que le HTML déposé. **Ce que le passage P26 → P27 change** (diff des transcriptions) : Agirc-Arrco, **le 105 disparaît** (seul le 131 reste, avec 109 et 132) et la base 11 (forfaitaire) s'ouvre au 131 ; Urssaf, le 146 devient « hors CET » et le **148** (CET patronale) apparaît sous les bases 03, 22, 23 (fiche 2556 confirmée) ; France Travail, 042, 043 et 908 disparaissent ; le composant 01 se renomme « réduction générale dégressive unique ». **D10** : aucune base n'admet le 105 en P27V01, une régularisation 2027 d'un mois déposé en 105 ne peut donc que passer en 131 (inférence : le tableau dit ce qui est attendu, pas comment régulariser). **D7** : rien sur la DFS dans la feuille Urssaf. | `net-entreprises-fiche1265-combinaisons-78-79-81-*` |
| 3369 | Proratisation du plafond de Sécurité sociale pour les salariés à temps partiel et en forfait jour réduit (créée le 10/03/2026, modifiée le 03/04/2026 ; **PDF imprimé par Léandre le 11/10/2026**) | **D9 fait.** « Par défaut, le salarié à temps partiel cotise uniquement à hauteur de la quotité de travail de son contrat. L'employeur peut choisir de ne pas proratiser le plafond […] pour tous les salariés à temps partiel de son établissement » : **la proratisation est la règle, la non-proratisation le choix**, et le choix vaut pour l'établissement entier. En P27V01 : `40.084 = 01` si l'employeur proratise, `02` s'il ne proratise pas, pour un temps partiel (`40.014 = 20`) comme pour un **forfait jour réduit** (qui reste `40.014 = 10` temps plein, `40.011 = 20`, Cass. soc. 12/03/2014) ; les blocs 78 comme en P26V01 (exemple : 80 %, brut 4 100, base 02 = 3 204 proratisée ou 4 005 pleine). Les absences relèvent de la fiche 2713 (53.003 valeur 40). BOSS assiette générale § 830. **Ce que ça change pour V4b** : le moteur fait de la proratisation une option « proratisé » de la fiche entreprise ; la fiche dit l'inverse (proratisé par défaut, le choix est de ne pas le faire) et ajoute le forfait jour réduit, que le moteur n'a pas (règle 0). Q16 (choix par compagnie) : la fiche dit « établissement », sans date. Rien n'est codé. | `net-entreprises-fiche3369-proratisation-plafond-temps-partiel-40-084-20260403` |
| 1336 | Déclaration de la rubrique « Taux de déduction forfaitaire spécifique pour frais professionnels - S21.G00.40.023 » (modifiée le 22/03/2025 ; **PDF imprimé par Léandre le 11/10/2026** ; le brut `.html` a été relevé le 13/10/2026 par `curl`, même date de modification 22/03/2025) | V2d/D7 : la DFS en DSN. Elle dit : 40.023 porte « systématiquement le taux de l'abattement appliqué ou applicable », la base de calcul est « le montant global des rémunérations, indemnités, primes et le cas échéant des indemnités de remboursement de frais » après déduction ; plafond 7 600 € par an et par salarié ; absence de refus exprès du salarié ; non-cumul avec l'exclusion des frais. **Rien sur les bases 78/81 ni sur le 51 type 001** : D7 reste à moitié ouvert. | `net-entreprises-fiche1336-dfs-taux-40-023-20250322` |
| 862 | AUDIENS - Caisse des congés spectacles | V2f/V4b : 40.022, 40.086 (Q, population de 40.086) | `net-entreprises-fiche862-audiens-caisse-conges-spectacles-20250322` |
| 3344 | Spécificités du contrat et des rémunérations des intermittents du spectacle en DSN | N1/V1 : spécificités du contrat et des rémunérations des intermittents | `net-entreprises-fiche3344-intermittents-contrat-remunerations-20251113` |
| 3327 | Cotisations en DSN des périodes d'arrêt maladie non-professionnelle à AUDIENS (caisse de congés spectacles) | V2a : arrêt maladie d'un intermittent et la caisse des congés | `net-entreprises-fiche3327-arret-maladie-cotisations-audiens-20250804` |
| 3394 | Gestion de la période de transition P26/P27 pour la déclaration des aménagements du temps de travail | V4a : la transition P26/P27 (bloc 75) ; D1 (bascule), partiellement | `net-entreprises-fiche3394-transition-p26-p27-amenagement-temps-travail-20260902` |
| 1265 | Base assujettie et cotisations individuelles : renseigner les blocs 78, 79 et 81 | V3e : bloc 79 (type 18) ; N3 : bases et cotisations | `net-entreprises-fiche1265-blocs-78-79-81-20260708` |
| 3377 | Déclaration des cotisations OC en cas de multi contrats de travail dans le mois | V3e/N4 : affiliations prévoyance avec plusieurs contrats dans le mois (70.016 en 2027) | `net-entreprises-fiche3377-oc-multi-contrats-dans-le-mois-20260527` |
| 3360 | Application des principes du fait générateur | V0b/V2c : régularisations, fait générateur des cotisations | `net-entreprises-fiche3360-fait-generateur-20260130` |
| 1392 | Le "numéro d’ordre" | V1 point 3 : le numéro d'ordre des signalements (S20.G00.05.004) | `net-entreprises-fiche1392-numero-d-ordre-20250417` |
| 3381 | Indemnités de fin de contrat | V4b : indemnités de rupture (type 13), bloc 52 | `net-entreprises-fiche3381-indemnites-fin-de-contrat-20260603` |
| 2958 | Déclarer un CDD | V1 point 5 : motifs de recours et de rupture d'un CDD | `net-entreprises-fiche2958-declarer-un-cdd-20260209` |

## Lues le 11/10/2026 (séance « documents », D8) : les fiches du montant net social

- **2983** (modifiée le 09/10/2026), « Comment déclarer le montant net social ? » : le bloc 58 « héritant de la datation
  du bloc Versement individu (parent par défaut), les rubriques de datation 58.001 et 58.002 **ne sont pas
  obligatoires** » ; l'exemple du cas nominal (mars 2027) les laisse « Non renseignée ». Elles **servent à la
  correction** d'un MNS d'un mois antérieur (« MNS absent, incomplet, erroné ») : on rattache le complément « avec la
  date du mois du versement auquel ce MNS était associé », et les deux datations sont admises, le mois
  (01032027-31032027) ou le jour du versement (26032027-26032027) ; la correction se fait « en annule et remplace ou en
  différentiel », montants positifs ou négatifs, et le bloc 58 de correction peut se mettre sous le bloc 50 du mois
  courant si le bloc 50 d'origine n'a pas d'autre erreur. Une erreur de **versement** (trop versé) se compense sur le
  versement suivant (RNF et MNS d'avril), sans dater. **Ce que ça change pour V4b** (`paie/src/dsn.rs`, `dates_du_58`,
  qui écrit la date de versement deux fois) : ce n'est pas faux (la datation au jour du versement est l'une des deux
  formes admises), mais le cas nominal officiel les omet ; à trancher au lot (omettre dans le cas nominal, dater
  seulement la correction d'un mois antérieur, ce que `versement_declare` fait déjà relire). **Codé au lot R5a
  (11/10/2026)** : en P27V01, le 58 du versement du mois n'a plus de dates (`dsn::dates_du_58_du_mois`) ; une correction
  date le 58 d'origine (annulation comprise, `dsn::annuler_les_58`) et son rappel, au jour du versement (`dates_du_58`).
- **3386** (03/07/2026), part exonérée des indemnités de rupture : type **13** du bloc 58 « à partir de 2027 », le
  montant = la part exonérée de cotisations (le plafond : le plus élevé de l'indemnité légale+conventionnelle, 50 %
  du total, deux fois le brut annuel) ; la part exonérée de CSG-CRDS est un autre montant (le plus faible des deux).
  Exemple de correction : un 58 type 13 daté du mois d'origine (01112027-30112027) avec le différentiel. V4b l'a codé.
- **3323** (07/07/2026), IJ subrogées : type **10** (IJSS subrogées, nettes de CSG-CRDS, dès 2026) et type **14**
  (IJ complémentaires de prévoyance subrogées exonérées, dès 2027), « en date de versement », à côté du type 03 ;
  seules les IJ complémentaires versées par l'employeur se déclarent, celles versées directement par l'organisme
  non. Le moteur ne fait pas la subrogation (vu sans faire, V4b) : un cas à porter (règle 0).
- **3392** (14/08/2026), fiche chapeau : renvoie aussi à **3387** (fraction représentative des frais d'emploi, bloc 58),
  813 et 814 (service à la personne, chèques vacances), non déposées ; 3387 concerne a priori les élus locaux (à
  vérifier, non relevée).

## Lot R10 (11/10/2026) : la fiche 1265, l. 52, et la fiche 2956, l. 216-225

- **1265, l. 52** : « Pour tout changement intervenant en cours de mois, il convient de ventiler les bases assujetties par
  périodes afin de permettre un calcul correspondant à la situation exacte du salarié. » La fiche vaut pour tous les
  organismes qu'elle liste, Urssaf CN compris. Où le moteur s'en sert : `atelier/sources/paie/src/dsn.rs`,
  `bloc_versement` (les blocs 78/79/81 de chaque période élémentaire d'une paie découpée, `periodes.rs`).
- **2956, l. 216-225** (« Déclarer un changement de statut en cours de mois ») : il est « admis par exception » de
  déclarer le salarié sur deux fractions de DSN du même mois. Le moteur ne s'en sert pas : il déclare une paie découpée
  dans une seule DSN, comme la norme (§ 2.1.3.2) et la FAQ Agirc-Arrco le décrivent.

## Lot U7 (13/10/2026) : les fiches 2502 (CPF-CDD) et 2503 (CFP)

Relevées le 13/10/2026 par `curl` (sans en-tête `User-Agent` : avec celui d'un navigateur, le site répond 401), HTTP 200 ;
la page urssaf.fr des contributions de formation y renvoie (`../majoration-caisses-conges/`).

- **2502** — « Contribution dédiée au financement du compte personnel de formation (CPF) pour les titulaires de CDD »,
  https://net-entreprises.custhelp.com/app/answers/detail_dsn/a_id/2502/ ; « Date de modification : 22/03/2025 ».
  `net-entreprises-fiche2502-cpf-cdd-20250322.{html,txt}`. Ce qu'on en lit : les contrats exclus (dont « Les contrats des
  salariés intermittents du spectacle », l. 14) ; la cotisation 129 sous la base 03, `81.002` = SIRET de l'Urssaf ; le CTP
  au bloc 23 renvoyé au guide de l'Urssaf. Rien sur les caisses de congés.
- **2503** — « Contribution à la formation professionnelle », a_id 2503 ; « Date de modification : 29/04/2025 ».
  `net-entreprises-fiche2503-contribution-formation-professionnelle-20250429.{html,txt}`. Ce qu'on en lit : l. 36 « Pour
  les entreprises de BTP, de transport et de docker dont les congés payés, sont indemnisés par des caisses de congés payés,
  les salaires à déclarer sont ceux effectivement payés par l'entreprise, majorés d'un taux forfaitaire de 11,5% » ; la 128
  sous la base 03 (l. 43) ; l. 104, au 81.003 : « Pour les salariés affiliés à une caisse de congés payés du BTP, du transport
  ou des dockers, le montant d'assiette est majoré de 11,5% ».

**Où le moteur s'en sert** : les socles `logiciel/donnees/baremes/2025.json` et `2026.json`, `par_nature.permanent`
(`cotisations_dsn[128]`, `cpf_cdd.cotisation_dsn`, `rubriques_ajoutees[autres_4]`, leurs `_quoi`/`_base`) ; `src/dsn.rs`,
`assiettes_ctp` et `assiette_declaree` (`cfp_permanent`). Lecture complète : `../majoration-caisses-conges/LISEZ-MOI.md`,
« La forme de la déclaration ».

## Le signalement de fin de contrat (FCTU) : pour quels contrats, et ce qui en dispense (lot T1, 13/10/2026)

Relevées le 13/10/2026 par `curl` (le site répond), dans la catégorie « La gestion d'une Fin de contrat de travail (FCTU) »
(`list-search/p/1899`) ; la fiche chapeau 3308 en dresse la liste. Le brut est `.html`, la transcription `.txt`.

### Fiche 2062 — « Quels sont les cas de dérogation à la déclaration des fins de contrat par signalement ? »

- URL : https://net-entreprises.custhelp.com/app/answers/detail_dsn/a_id/2062/ ; « Date de modification : 05/09/2025 » ; créée
  le 20/12/2018. Fichier : `net-entreprises-fiche2062-derogations-fin-de-contrat-fctu-20250905.{html,txt}`. Le conseil
  `signalement-fin-contrat-cddu-derogatoire` de `logiciel/donnees/conseils.json` la cite (ticket 224).
- **Règle générale** (fiche 119, `…fiche119-…`, et R. 1234-9 du code du travail, cité par 2062 en dernière ligne) : « toute fin
  de contrat de travail, quels qu'en soient le motif et l'origine, doit donner lieu à la remise […] d'une Attestation Employeur »,
  et en DSN l'attestation s'obtient par le FCTU dans les 5 jours de la fin. Tout contrat déclaré en DSN a donc un FCTU, sauf
  les trois cas ci-dessous.
- **Dérogation 1, le CDD d'usage « CDDU-D »** (txt l. 7-13) : déclaré **uniquement dans la DSN mensuelle**, sans FCTU (sauf
  demande expresse du salarié), si les quatre critères sont réunis : nature 40.007 = 02 (ou 10), motif de recours 40.021 = 05
  (contrat d'usage), modalité 62.017 = 01 (circuit dérogatoire), **motif de rupture 62.002 = 031** (fin de CDD). « **Tous les
  autres motifs de rupture doivent être déclarés par l'émission d'un signalement FCTU.** » Et : « la dérogation n'est applicable
  en DSN que pour les contrats CDDU infra-DSN » (la fiche 1741 le définit : début et fin entre deux processus de paie).
- **Dérogation 2, le contrat de mission** (nature 03) : DSN mensuelle seulement, pour les motifs 032, 036, 037, 084 ; les autres
  motifs, FCTU.
- **Exclusion de nature** : pas de FCTU pour les natures 20, 21, 29, 50, 70, 80, 81, 89, 90, 91, 92, 93.
- **Exclusion de population** (motif d'exclusion DSN 40.025) : pas de FCTU pour les fonctionnaires, les navigants de la marine
  marchande, les marins-pêcheurs, les dockers carte G, et, **mot à mot**, pour : « Contrat de travail faisant l'objet d'une
  attestation employeur mensuelle (AEM) pour un ouvrier ou technicien de l'édition d'enregistrement sonore, de la production
  cinématographique et audiovisuelle, de la radio, de la diffusion et du spectacle » et « […] AEM pour un artiste du spectacle ».
  Un FCTU envoyé pour un tel contrat ne produit pas d'AER, un compte rendu métier signale l'anomalie, « aucune information ne
  parvient à France Travail », la déclaration « doit être opérée en dehors de la DSN » (l'AEM, déposée à France Travail).
- Fiche 3037 (`…fiche3037-…`, modifiée le 23/03/2025) : les intermittents du spectacle font partie des populations qui « ne
  relevant pas pour la transmission de l'attestation employeur du périmètre de la DSN » (avec les agents publics, les
  expatriés, les dockers, les navigants, les marins-pêcheurs) et transmettent leur attestation par formulaire.

**Ce qu'on en lit pour le moteur** (lecture, avec ses limites écrites) : un CDI et un CDD ordinaire (natures 01, 02 hors usage)
ont un FCTU ; un CDDU d'artiste ou de technicien du spectacle dont l'attestation passe par l'**AEM** est **exclu** du FCTU
(motif d'exclusion 40.025), quel que soit le motif de rupture ; un CDDU qui resterait en DSN (sans AEM) ne dispense du FCTU que
pour le motif **031** en circuit dérogatoire et infra-DSN : **un 036, 037, 084, 095… d'un tel CDDU se signale** (« tous les autres
motifs »). Ce que le texte ne dit pas : si une compagnie de spectacle dépose l'AEM *ou* le FCTU est un choix de population
(« 40.025 » est une rubrique que l'employeur renseigne) ; aucune fiche lue ne le dit en toutes lettres.

**Où le moteur s'en sert** :
- `logiciel/donnees/contrats-base.json` (`cddu_spectacle.fin_de_contrat.signalement_fctu = false`, l. 153-154, dont la note cite
  « la fiche net-entreprises 2062 », jusqu'ici non déposée ; `cdi` l. 304 et `cdd` l. 430 : `true`) ;
  `atelier/sources/paie/src/contrats.rs` `Nature::signalement_fctu` ;
- `atelier/sources/paie/src/fctu.rs` (le signalement, nature 07, l. 354 : refus pour une nature sans FCTU), qui n'écrit pas
  40.025 : la rubrique est conditionnelle (C) en nature 07 dans `usages-dsn-base.json` l. 1702-1710 (`arrets-base.json` l. 330 la range
  « CII ») ;
- `atelier/sources/paie/src/routes_ouverture.rs` `fins_a_signaler` (l. 733) et `rappels_fin_de_contrat` (les 5 jours ouvrés, calculés depuis le 15/10/2026 par `obligations.rs`, `de_la_fin`, avec `logiciel/donnees/obligations-base.json`),
  `routes_listes.rs` l. 434.

**Ce que le texte contredit ou ne couvre pas** : le moteur tient « un CDDU du spectacle ne fait PAS l'objet d'un FCTU » pour
toujours vrai, alors que 2062 le dit vrai (a) par l'exclusion AEM (40.025), (b) par la dérogation, pour le seul motif 031.
Un CDDU rompu avant son terme (036, 037…) hors AEM serait donc à signaler. Rien n'est modifié ici (lot sans code) ; point à
trancher dans un lot à part.

**Tranché par le lot C1 (14/10/2026)** : la règle est maintenant `Nature::signalement_fctu(motif)` (`atelier/sources/paie/src/contrats.rs`),
lue dans `contrats-base.json`, `cddu_spectacle.fin_de_contrat` : `signalement_fctu: true` (R. 1234-9, fiche 119) et
`circuit_derogatoire` (motif 031 seul ; 62.017 = 01 au 031, 02 sinon, CCH-11 et CCH-12 de 62.017, p. 277 du cahier). Un CDDU
rompu avant son terme écrit 62.017 = 02 dans la mensuelle et paraît dans les fins à signaler (route `fins-de-contrat`, rappels de
l'accueil, `paie fin-contrat liste`) ; arrivé à son terme (031), rien ne change. Le CDDU a aussi la liste de motifs de la nature 02
(CCH-11 de 62.002). Inférence écrite : depuis le 100 % DSN, 40.025 « doit rester vide » (`../francetravail-100-dsn/`), l'exclusion
« AEM » ne joue donc plus. Test : `atelier/sources/paie/tests/dsn/fin_cddu.rs`. Ce qui reste ouvert (la demande expresse du
salarié, le CDDU non infra-DSN) : `contrats-base.json`, `circuit_derogatoire._reste_ouvert`, et `prive/journal/finitions/PASSATION-C1.md`.

### Fiche 119 — Quelles informations renseigner concernant le préavis pour un signalement fin de contrat ?

URL https://net-entreprises.custhelp.com/app/answers/detail_dsn/a_id/119/ ; modifiée le 12/05/2025. « La fin de contrat de travail d'un salarié doit **toujours** faire l'objet de la transmission d'un signalement […] "Fin du contrat - S21.G00.62" pour avertir France Travail » ; les informations sont reprises dans la DSN mensuelle ; le signalement est émis « dans les cinq jours après la survenance de l'événement » ; le bloc 63 « doit être renseigné dans le signalement […] mais n'a pas d'utilité dans la DSN mensuelle suivante ». Moteur : `fctu.rs` (bloc 63 obligatoire, l. 253), `obligations.rs` (`de_la_fin`) et `logiciel/donnees/obligations-base.json` (`signalement_fin_contrat`, 15/10/2026).

### Fiche 3037 — Qu'est-ce que l'Attestation Employeur Destinée à France Travail – AED ?

URL https://net-entreprises.custhelp.com/app/answers/detail_dsn/a_id/3037/ ; modifiée le 23/03/2025. Depuis le 01/01/2022 le FCTU est la seule modalité de l'attestation employeur en DSN. « Certains employeurs ayant des populations ne relevant pas […] du périmètre de la DSN (agents publics […], **intermittents du spectacle**, expatriés, dockers, navigants, marins-pêcheurs) doivent, uniquement pour ces populations, transmettre leurs attestations employeur par le formulaire dématérialisé » (l'AEM pour les intermittents, voir 2062). Moteur : `contrats-base.json`, `cddu_spectacle.signalement_fctu = false`.

### Fiche 1741 — Qu'est qu'un contrat infra DSN mensuelle ?

URL https://net-entreprises.custhelp.com/app/answers/detail_dsn/a_id/1741/ ; modifiée le 07/07/2025. Le contrat infra-DSN est celui dont les dates de début **et** de fin tombent entre deux processus de paie, donc « intégralement déclaré au sein de la DSN mensuelle » de son mois de fin ; ce n'est ni une question de durée ni de calendrier. C'est la condition de la dérogation CDDU-D de la fiche 2062. Moteur : le circuit dérogatoire 62.017 = 01 (`dsn.rs` l. 1749, `contrats-base.json`).

### Fiche 1902 — Comment déclarer les fins de contrat de travail dans le circuit dérogatoire (CDDU-D) ?

URL https://net-entreprises.custhelp.com/app/answers/detail_dsn/a_id/1902/ ; modifiée le 12/05/2025. Le circuit dérogatoire (62.017 = 01) vaut pour la nature 02 ou 10, le motif de recours 05, le motif de rupture 031 (mêmes critères que 2062). Correction après la date d'exigibilité : un FCTU, reporté dans la DSN du mois suivant ; l'annulation d'une fin déclarée à tort : un bloc 62 de code 099 dans la DSN du mois suivant ; le salarié peut refuser le circuit dérogatoire : FCTU dans les 5 jours. Moteur : `dsn.rs` (62.017), `signalement.rs`.

### Fiche 3308 — « Fiche chapeau » : la gestion d'une fin de contrat de travail (FCTU)

URL https://net-entreprises.custhelp.com/app/answers/detail_dsn/a_id/3308/ ; modifiée le 07/07/2025. Le sommaire de la catégorie : 2655 (mois de rattachement 50.020), 2648 (rupture conventionnelle), 2358 (assistants familiaux), 2131 (annule et remplace), 2130 (signalement FCTU), 2072 (annuler une date de fin), 2062 (dérogations), 1982 (complément de solde), 1903 (retours AER d'un CDDU-D), 1902, 1722 (annulation d'une fin transmise à tort), 1264 (préavis), 673 (information générale sur les signalements), 238 (fin de travail anticipée). Déposées ici : 2062, 1902, 1741, 119, 3037, 673, 1722, 1903 ; les autres fiches de la liste ne sont pas déposées (elles ne servent à aucun point du moteur aujourd'hui).

## La DFS en DSN : l'abattement et les rémunérations de type 001 et 002 (lot T1, 13/10/2026)

### Fiche 484 — « Faut-il appliquer l'abattement sur les frais professionnels dans la rémunération brute non plafonnée et le salaire brut soumis à contributions Assurance Chômage ? »

- URL : https://net-entreprises.custhelp.com/app/answers/detail_dsn/a_id/484/ ; « Date de modification : 01/07/2025 » ; créée le
  13/04/2015. Fichier : `net-entreprises-fiche484-abattement-frais-professionnels-brut-non-plafonne-et-chomage-20250701.{html,txt}`.
  Trouvée en parcourant les 869 fiches de la base (catalogue relevé le 13/10/2026) par une recherche de « déduction forfaitaire
  spécifique », « DFS », « abattement » ; la recherche de la base elle-même ne répond pas.
- **Ce qu'on en lit** (txt, l. 3-6) : « Le montant renseigné dans le bloc "Rémunération - S21.G00.51" avec le type […] "**001 -
  Rémunération brute non plafonnée**" **doit prendre en compte l'abattement pour frais professionnels**. » Dans le cas général, le
  type **002** (salaire brut soumis à contributions d'assurance chômage) aussi. **Deux exceptions** pour l'assurance chômage : les
  journalistes (avant l'abattement de 30 %) et, « pour les activités relevant des critères des **intermittents du spectacle** au
  sens de l'Assurance-chômage, les rémunérations versées depuis le 1er juillet 2017 sont prises en compte **avant application de
  l'abattement** pour les professions admises […] de 20 % ou de 25 % » (art. 21 § 1er du règlement général pour les artistes et
  techniciens). « Les primes et indemnités de fin de contrat (bloc 52) doivent être transmises sous leur forme brute non abattue. »
  L'exemple : 4 500 € de salaire + 500 € de frais = 5 000 ; abattement de 25 % : 3 750 ; abattement 1 250 € ; **001 = 3 750**,
  et pour un intermittent **002 = 5 000** (non abattu).
- **C'est le texte de D7 pour le 51 de type 001** (`dfs/LISEZ-MOI.md`, « Ce qui manque », point 7) : un salarié sous DFS déclare
  son 51 de type 001 **abattu** et son 51 de type 002 **non abattu** s'il est intermittent du spectacle. Le moteur
  (`dsn::brut_du_51`, `dsn::remunerations`) fait déjà ainsi. La fiche ne parle pas des blocs 78 : voir `dfs/LISEZ-MOI.md`.

### Fiche 1970 — « Comment déclarer un VRP multicarte en DSN […] ? » (l'exemple d'un salarié sous DFS)

- URL : https://net-entreprises.custhelp.com/app/answers/detail_dsn/a_id/1970/ ; « Date de modification : 27/06/2025 ».
  Fichier : `net-entreprises-fiche1970-vrp-multicarte-dfs-exemple-20250627.{html,txt}`. Trouvée par la même recherche que la
  fiche 484 (13/10/2026).
- **Ce qu'on en lit** (txt, section « Fiabilisation des DSN des VRP multicartes : Le correct remplissage pour les VRP qui
  bénéficient de la Déduction Forfaitaire Spécifique ») : pour un salarié « dont la rémunération brute est de **20 000 €** et qui
  bénéficie de la DFS » (taux 40.023 = **30.00**, « sert au calcul du montant du remboursement de la contribution CSG/CRDS »),
  la DSN de mars porte un **bloc 54 de type 10 de 6 000,00** (= 30 % de 20 000), une **base assujettie 03 de 14 000,00**
  (= 20 000 − 6 000, donc **abattue**) et une **base 02 de 10 284,00 (3 × 3 428,00)**, le plafond de la période, atteint par la
  base abattue ; les 81 (076, 045, 068, 074, 075…) y sont rattachés sans montant dans l'exemple.
- **C'est le seul texte déposé qui montre une base 03 abattue sous DFS** (D7) : le bloc 54 de type 10 = la déduction, la base 03 =
  le brut moins la déduction. **Limite** : l'exemple est celui d'un VRP multicarte, pas d'un artiste ; que l'artiste ait la même
  forme est une **inférence** (la DFS s'applique de même à la base de cotisation, fiche 1336 : « la base de calcul des cotisations
  est constituée par le montant global des rémunérations […] à laquelle on applique la déduction »). Les bases 23 et 43
  (Agirc-Arrco) et le 81.003 n'y figurent pas.
- Le moteur : `dsn::cle_abattue`, `dsn::BASES_ABATTUES_PAR_LA_DFS`, `dsn::autre_revenu` (le bloc 54 de type 10 = `brut − brut_du_51`).

### Fiche 827 — « Comment déclarer les avantages en nature (autres éléments du revenu brut) ? »

- URL : https://net-entreprises.custhelp.com/app/answers/detail_dsn/a_id/827/ ; « Date de modification : 22/03/2025 ». Fichier :
  `net-entreprises-fiche827-avantages-en-nature-autre-element-revenu-brut-20250322.{html,txt}`.
- **Ce qu'on en lit** : le montant du bloc 54 (types 02 à 06, avantages en nature) est « le montant brut de l'avantage en nature,
  tel qu'il figure dans le bulletin de salaire avant déduction des cotisations sociales » ; « **Si le salarié bénéficie de la
  déduction forfaitaire spécifique pour frais professionnels, le montant de l'avantage en nature à reporter en bloc "Autre élément
  de revenu brut - S21.G00.54" ne tient pas compte de cette déduction.** » L'avantage reste un élément de la base assujettie.
- Le moteur : `dsn::autre_revenu` (l. 1887-1900) écrit le montant de la ligne sans l'abattre, puis le bloc 54 de type 10
  (la déduction) à part : accord avec le texte.

### Fiche 3344, relue pour le FCTU : le motif d'exclusion 40.025 d'un intermittent (déjà déposée, lue le 13/10/2026)

`net-entreprises-fiche3344-intermittents-contrat-remunerations-20251113.txt`, l. 19, 160-172 et 436-446 :
- l. 19 : « En juillet 2025, une phase d'expérimentation a été lancée afin de remplacer progressivement l'attestation employeur
  mensuelle par la DSN. »
- l. 160-172 (le contrat d'un intermittent) : « **Motif d'exclusion DSN - S21.G00.40.025** : A renseigner avec l'une des valeurs
  ci-contre. » **05** « Contrat de travail faisant l'objet d'une attestation employeur mensuelle (AEM) pour un ouvrier ou technicien
  de l'édition d'enregistrement sonore, de la production cinématographique et audiovisuelle, de la radio, de la diffusion et du
  spectacle » ; **06** « […] AEM pour un artiste du spectacle » ; « Ces valeurs ne seront plus à renseigner par les employeurs dès
  lors que France Travail leur en donnera l'autorisation » (l. 166 et 448). Le même exemple se répète l. 436-446 pour l'artiste.
- C'est la rubrique qui donne le sens à l'exclusion de la fiche 2062 (« motif d'exclusion (S21.G00.40.025) »).
- **Constat sur le moteur** (`grep -rn "40.025" atelier/sources logiciel/donnees`) : la rubrique n'est écrite par aucun site de
  génération ; elle n'existe que dans les tables d'usages et de formats (`usages-dsn-*-base.json`, `formats-dsn-*-base.json`) et
  la liste des conditionnelles de `arrets-base.json`. Un CDDU d'artiste ou de technicien déclaré sans 40.025 = 05 ou 06 n'est pas
  signalé comme exclu du FCTU. Rien n'est modifié ici (lot sans code) ; point à trancher dans un lot à part.

### Fiche 673 — rappel de consignes pour les signalements d'arrêt de travail, de reprise et de fin de contrat

URL https://net-entreprises.custhelp.com/app/answers/detail_dsn/a_id/673/ ; modifiée le 13/05/2025. Les signalements « Fin de contrat
de travail unique » « doivent être envoyés dans les **5 jours ouvrés** à connaissance de l'événement ». « Dans le cas des CDD
d'usage, il est possible que l'employeur, avec l'accord du salarié, ait recours au dispositif dérogatoire » : transmission « exclusivement
à France Travail via la DSN mensuelle (sans transmission de signalement) » ; les fins de mission des intérimaires de même (renvoi à la
fiche 2062). Il est **interdit de doubler** un signalement DSN par une AE par l'ancienne modalité. Toute information d'une fin de
contrat doit être reportée dans la DSN mensuelle consécutive. Moteur : `obligations.rs` et `logiciel/donnees/obligations-base.json` (le signalement d'arrêt, de reprise et de fin de contrat, cinq jours ouvrés, ou avec la mensuelle si l'employeur est subrogé ; 15/10/2026).

### Fiche 1983 — la prolongation d'un arrêt, et les dates de subrogation (relevée le 21/10/2026)

URL https://net-entreprises.custhelp.com/app/answers/detail_dsn/a_id/1983/ ; « Date de modification : 22/03/2025 » ; créée le
10/10/2018. Trouvée par la recherche du site (mot « prolongation ») et par la fiche chapeau 3304 « Gestion des arrêts, reprises et
suspensions » (non déposée : un sommaire). Fichiers : `net-entreprises-fiche1983-prolongation-arret-dates-de-subrogation-20250322.html`
(le brut tel que servi) et `.txt` (le corps, transcrit par `pandoc`, tableaux aplatis).

**Ce qu'on en lit** (`.txt`, lignes citées) :
- l. 13 : « En cas de prolongation d'arrêt, il ne faut pas envoyer de nouveau signalement de nature " arrêt de travail ". Il suffit
  de déclarer la nouvelle date de fin de l'arrêt en DSN mensuelle, au niveau de la rubrique " Date de fin prévisionnelle -
  S21.G00.60.003 " » ;
- l. 17-31 : les données de subrogation (60.004 à 60.008) « ne sont pas présentes en DSN mensuelle, elles n'apparaissent que dans
  le signalement » ; la fin de subrogation est la « date de fin de subrogation maximale prévue dans le cadre de la convention
  collective ou de l'accord de branche » ;
- l. 37-39 : une subrogation « insuffisante ou erronée » se corrige par « un nouveau signalement de nature " arrêt de travail " de
  type annule et remplace, dans lequel une nouvelle date de fin de subrogation sera renseignée », « dans des délais raisonnables
  [...] avant le prochain paiement d'indemnités journalières » ; l. 66 : « le plus rapidement possible dès connaissance de la
  prolongation de l'arrêt » (aucun délai chiffré) ;
- l. 110 : si la subrogation « court toujours », « il n'est pas nécessaire d'effectuer un nouveau signalement ».

**Où le moteur s'en sert** (21/10/2026) : `atelier/sources/paie/src/routes_imprevus.rs` (`arret_prolonger`, route
`arret-prolonger`, `paie arret prolonger` : la prolongation ne change que la fin prévisionnelle, rien ne part ;
`subrogation_a_revoir` demande la fin de subrogation quand elle tombe avant la nouvelle fin) ; `obligations.rs`
(`rectificatif_du`) et `logiciel/donnees/obligations-base.json` (`signalement_arret_rectificatif` : une fin de subrogation
repoussée après le dépôt du signalement d'arrêt appelle un annule et remplace ; le délai de cinq jours ouvrés est celui d'un
signalement d'arrêt, fiche 673 — **inférence**, la fiche n'en chiffre aucun). Le message de `signalement::verifier` pour une
reprise après la fin prévisionnelle renvoie à la prolongation (il disait « un nouvel arrêt »). Tests :
`tests/routes/arret_prolonge.rs`.

### Fiche 1722 — annuler une fin de contrat transmise à tort (bloc 62, motif 099)

URL https://net-entreprises.custhelp.com/app/answers/detail_dsn/a_id/1722/ ; modifiée le 26/03/2025. Dans une DSN mensuelle, une fin
de contrat déclarée à tort s'annule, une fois l'échéance passée, par un bloc 62 de motif **099 - Annulation** dans la DSN du mois
suivant ; le 099 est **interdit** dans un FCTU (nature 07), qui a sa propre annulation (type « 04 - annule ») ; il annule une fin
déclarée à tort, il ne la corrige pas. Moteur : `fctu.rs` (`ANNULE`, type 04) ; le 099 en mensuelle n'a pas été cherché.

Elle dit aussi comment une fin se **corrige** : « une correction de fin de contrat s'opère par l'émission d'un bloc " Fin du
contrat - S21.G00.62 " dans la DSN mensuelle suivante portant les informations correctes qui écraseront celles précédemment
déclarées ». Moteur (21/10/2026) : la fin d'un contrat déclaré corrigée après la limite de l'annule-et-remplace, son bloc 62
redéclaré sous le bloc 40 de la correction (`dsn.rs`, `generer`, `fin_corrigee`) ; test
`tests/dsn/regularisation_declaree.rs`, `une_date_de_debut_reculee_et_une_fin_corrigee_apres_la_limite_se_declarent`.

### Fiche 1952 — corriger une date de début de contrat après l'échéance (bloc 41, 41.021)

URL https://net-entreprises.custhelp.com/app/answers/detail_dsn/a_id/1952/ ; modifiée le 26/03/2025 ; déposée par le lot V3c
(`net-entreprises-fiche1952-correction-date-debut-contrat-20250326.html`, `.txt`). Ce qu'on en lit : passé l'échéance, la date
de début se corrige dans la DSN suivante, la nouvelle en S21.G00.40.001, l'ancienne dans un bloc « Changements Contrat »
(41.001, date de la modification, et 41.021) ; l'exemple date 41.001 du jour de la correction en paie ; 41.001 au plus tard
le dernier jour du mois principal déclaré ; 41.021 jamais avec la profondeur de recalcul 41.028. **Où le moteur s'en sert**
(21/10/2026) : `changements_contrat::ancienne_date_de_debut`, appelé par `dsn::generer` pour une régularisation
(« Corriger ce mois ») ; l'ancienne date se lit dans la DSN déposée (`versement_declare`, `debut_contrat`) ; la saisie et
l'aperçu : `routes_regularisation` (`apercu_correction`, `phrase_des_dates`, `propager_les_bornes`),
`jumeaux::apparier_meme_numero`. Un contrat sur plusieurs mois : ses dates suivent dans chaque mois saisi (la date de début
« est un des identifiants du contrat », fiche 3344), chaque mois se corrige pour lui-même, et la DSN suivante porte un seul
bloc 40 et un seul bloc 41 (le dernier mois déclaré fait foi, `produire_regularisations::versement_declare_de`), un versement
par mois civil (cahier technique §2.2.2.5). Seul refus : des dates qui sortiraient le contrat d'un mois où il est saisi avec
ses jours. Tests : `tests/dsn/regularisation_declaree.rs` (`une_date_de_debut_avancee_apres_la_limite_se_declare_par_le_bloc_41`,
`un_contrat_sur_deux_mois_dont_le_debut_est_avance_se_corrige_mois_par_mois` et suivants), `tests/routes/modifier_apres_echeance.rs`.

### Fiche 1903 — les retours de France Travail pour un CDDU-D

URL https://net-entreprises.custhelp.com/app/answers/detail_dsn/a_id/1903/ ; modifiée le 03/06/2025. Pour chaque DSN mensuelle portant
au moins un CDD d'usage en circuit dérogatoire, France Travail met à disposition « sous 48 h » une archive des AER (une AER CDDU par
individu **et par libellé d'emploi** 40.006, 12 lignes au plus) et/ou un compte rendu métier. Rien de ce texte n'est lu par le moteur
aujourd'hui (aucun relevé des retours de nature 44 ne distingue le CDDU-D : `retours.rs` l. 60 la range « AER ou compte-rendu métier FCTU »).

### Fiche 2681 — « Réduction générale et absence : Quels sont les grands principes pour proratiser le SMIC ? »

URL https://net-entreprises.custhelp.com/app/answers/detail_dsn/a_id/2681/ ; modifiée le 09/06/2026. La RGDU en cas d'absence ou de
mois incomplet se déclare au bloc 51 par deux types : **028** « Rémunération perçue hors éléments non affectés par l'absence »
(numérateur) et **029** « Rémunération habituelle mois complet hors […] » (dénominateur), tous deux « **avant déduction forfaitaire
spécifique (DFS)** » ; SMIC du mois = SMIC mensuel × 028 / 029. Contraste avec la fiche 484 : le type 001 est abattu, les 028 et 029
ne le sont pas. Le moteur : `grep -rn '"028"\|"029"' atelier/sources logiciel/donnees` ne trouve rien, il n'écrit pas ces types (pas de
proratisation du SMIC par ces rubriques ; `droit::reduction::reduction_generale_sous_dfs` calcule sur le brut abattu). Rien n'est modifié ici.

## La contribution supplémentaire à l'apprentissage : fiches 2588, 2963 et 2624 (lot D2, 13/10/2026)

- **2624, « Les CRM Effectifs »** : URL https://net-entreprises.custhelp.com/app/answers/detail_dsn/a_id/2624/ (trouvée
  par la catégorie « Les comptes rendus métiers - CRM », `list-search/p/2134`) ; « Date de modification : 29/04/2025 » ;
  relevée le 13/10/2026. Brut : `net-entreprises-fiche2624-crm-effectifs-20250429.html` ; texte : `.txt`. Ce qu'on en lit :
  le CRM de nature 118 porte les effectifs que l'Urssaf calcule ; la table des codes : **A01** effectif moyen annuel
  standard, A02 BOETH, A03 ECAP, A04 assujettissement OETH, **A05** « Effectif moyen annuel CSA CFIP (des Contrats
  Favorisant l'Insertion Professionnelle (numérateur)) », **A07** « Effectif moyen annuel CSA (de détermination du taux
  d'alternants (dénominateur)) » ; « Début mars : pour la remontée des effectifs EMA CSA et CSA CFIP ».
- **2588** (déposée par D1) et **2963** : la CSA de N se déclare dans la DSN de mars N+1, « sur un seul établissement »,
  bloc 82 code 079 (montant, du 0101N au 3112N, 82.005 le SIRET de l'Urssaf) ; l'exonération, code 080 à 0.00 ; le CTP
  au bloc 23 (le guide Urssaf du 12/03/2026, `majoration-caisses-conges/`, l. 2441-2463 : le CTP 998, « une déclaration
  en montant », taux de 100 %).
- **Où le moteur s'en sert** : `atelier/sources/paie/src/csa.rs` (`declaration`, `taux`), appelé par `dsn.rs` (le bloc 82
  et la ligne 998 du bordereau, établissement principal) et `produire_mois.rs` (la phrase du mois de mars). Tests :
  `tests/dsn/bloc_44.rs` (`la_dsn_de_mars_declare_la_contribution_supplementaire_de_l_annee_precedente` et suivants),
  `csa::tests`. Inférences (passation D2) : l'Alsace-Moselle par l'assiette de son CTP 993 ; la progression de 10 % comme
  le guide la calcule (A05 de N contre A05 de N-1) ; « excède deux mille » sous la règle des cinq ans.

## Le trop-versé : fiches 1931 et 1328 (21/10/2026)

Une correction à la baisse (« Corriger ce mois ») peut faire ressortir négative la somme qu'un mois doit à un organisme
(cas : un contrat de juillet annulé en entier, DSN d'octobre). `S21.G00.20.005` est « non signé » : ces deux fiches,
avec la norme et le guide Urssaf, disent ce qu'il en advient.

- **1931, « DSN - Corrections au niveau du bloc Versement Organisme de Protection Sociale - Gérer les situations
  créditrices »** : URL https://net-entreprises.custhelp.com/app/answers/detail_dsn/a_id/1931 ; « Date de modification :
  23/03/2025 » ; relevée le 21/10/2026 par `curl`. Brut : `net-entreprises-fiche1931-situations-creditrices-versement-20250323.html` ;
  texte : `.txt` (le corps de la réponse). Ce qu'on en lit : l. 4, « les rubriques « Montant du versement » ne peuvent pas
  être renseignées avec des montants négatifs. Lorsque la somme totale [du PAS et de ses régularisations] est négative,
  ces rubriques doivent être renseignées à « 0.00 » » ; l. 13, une DSN initiale créditrice ne déclare pas de bloc DGFiP,
  « si par commodité le logiciel en produit un, il devra être déclaré [...] à 0,00 » ; l. 17, un annule et remplace
  créditeur le déclare à 0,00 ; l. 19 et 40-41, « une demande de remboursement pourra être déposée auprès du SIE », par la
  messagerie sécurisée de l'espace professionnel impots.gouv.fr. (La fiche 1923, sa jumelle PASRAU, dit la même chose ;
  non déposée.)
- **1328, « La ventilation du versement des cotisations »** : URL https://net-entreprises.custhelp.com/app/answers/detail_dsn/a_id/1328 ;
  « Date de modification : 22/03/2025 » ; relevée le 21/10/2026. Brut et texte :
  `net-entreprises-fiche1328-ventilation-du-versement-20250322`. Ce qu'on en lit (l. 40) : pour un organisme
  complémentaire, le versement est la somme des composants (bloc 55) « sauf si cette somme fait ressortir un montant
  négatif, auquel cas un montant nul doit être renseigné » — la règle du CCH-12 de 20.005, redite.
- **Ce que rien ne dit** (cherché le 21/10/2026 dans la base net-entreprises — « montant du versement négatif »,
  « crédit de cotisations », « trop versé » —, les fiches 2417 France Travail et 1266 échéances, le dossier DSN et la page
  Congés Spectacles d'Audiens, le cahier Agirc-Arrco CT2026) : comment l'Agirc-Arrco, la Caisse des Congés Spectacles et
  France Travail rendent un crédit. Le moteur verse zéro (la norme l'autorise à chacun) et dit à la compagnie de le leur
  demander.
- **Où le moteur s'en sert** : `atelier/sources/paie/src/bordereau.rs`, « le trop-versé » (`a_verser`, `reprise_urssaf`,
  `credit` : le seul endroit qui dit, par organisme, le montant écrit et ce que la compagnie fait du crédit) ;
  `dsn.rs` (`versement_dgfip` : un PAS négatif s'écrit à 0.00 au lieu de refuser ; les versements Urssaf, complémentaires
  et autres passent par `bordereau`) ; `ordres_paiement.rs` (le crédit de chaque ordre, lu dans le fichier, à l'écran de
  dépôt). Test : `tests/dsn/regularisation_declaree.rs`, `un_contrat_qui_n_a_pas_eu_lieu_se_corrige_a_zero`.

## Le guide « De la DSN à l'attestation employeur », version 3.2 du 28/03/2023 (21/10/2026)

- URL : https://www.net-entreprises.fr/media/documentation/correspondances-dsn-ae.pdf ; relevé le 21/10/2026. Fichiers :
  `net-entreprises-de-la-dsn-a-l-attestation-employeur-v3-2-20230328.{pdf,txt}` (transcription `pdftotext -layout`).
- Ce qu'on en lit (pages 10 et 11) : pour une DSN mensuelle, France Travail (encore « Pôle emploi » dans le texte) répond
  par un avis de réception (nature 40) et, pour le spectacle, par un CRM (45, « ANOBLOQUANTE ») et une archive d'AE (46,
  `AER_MENSUELLE_[idFlux]_[rang].zip`). Le dispositif DSN transmet à France Travail toutes les 30 minutes au plus (valeur
  « paramétrable »). **Une DSN mensuelle annule et remplace** produit « éventuellement » une nouvelle archive d'AE et de
  nouveaux CRM ; ceux de la DSN remplacée ne sont plus disponibles (code 410 en API, message au tableau de bord). La
  fiche 1903 donne le délai : « sous 48h ».
- Où le moteur s'en sert : `etats.rs` (`etat_dsn_du_mois` : l'AER attendue est celle de la dernière DSN acceptée du
  mois, donc de l'annule et remplace) ; `routes_documents.rs` (`documents_prets` : les documents ne partent pas tant
  qu'elle n'est pas revenue).

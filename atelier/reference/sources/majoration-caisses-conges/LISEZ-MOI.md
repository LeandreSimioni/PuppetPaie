# La majoration de 11,5 % des employeurs affiliés à une caisse de congés payés

Pourquoi le FNAL et le versement mobilité d'un intermittent se calculent sur le brut × 1,115, sa taxe d'apprentissage sur
le brut × 1,10 (CTP 334 et 332, 900 et 921, 992 ; `brut_x115`, `brut_x115_plafonne`, `brut_x110` des barèmes ;
`majoration_conges_115`, `majoration_conges_110`). Déposé le 10/10/2026 : jusque-là, la majoration n'était tenue que par les DSN
de CoolPaie (`organismes-base.json`, `_note_ctp`).

## Les fichiers

- `acoss-lettre-circulaire-2013-019-20130328.pdf` (texte : `.txt`, `pdftotext -layout`) : **lettre circulaire Acoss
  n° 2013-0000019 du 28/03/2013**, 35 pages, qui commente la LFSS 2013. Référence donnée par Léandre ; le PDF est
  relevé le 10/10/2026 sur `https://beymedias.brightspotcdn.com/8f/e3/d29f18bcafbbfe30444cd9347d36/90718328-lettrecirculaire.pdf`
  (copie d'un tiers, pas le site de l'Acoss : à remplacer par l'original si on le trouve). Le § 1.3 (lignes 303-345
  du `.txt`) et l'annexe 1 (questions-réponses, lignes 1788-1894).
- `audiens-fiche-pratique-conges-spectacles-202302.pdf` (texte : `.txt`) : fiche pratique « Les Congés Spectacles »
  d'Audiens, février 2023, apportée par Léandre.
- `tissot-fnal-vt-majoration-11-5-20170711.mht` (texte de l'article seul : `.txt`) : Éditions Tissot, « FNAL et
  versement de transport sur les indemnités de congés payés : comment appliquer la majoration de 11,5 % ? », publié le
  17/04/2013, modifié le 11/07/2017, enregistré par Léandre le 10/10/2026. Écrit pour le BTP. Un éditeur, pas un texte.

- `urssaf-guide-contributions-formation-apprentissage-20260312.pdf` (texte : `.txt`, `pdftotext -layout`, qui signale une
  table de renvois abîmée et la reconstruit) : « Guide des contributions de formation professionnelle et d'apprentissage des
  employeurs 2026 », Urssaf Caisse nationale, « Version du 12 mars 2026 », 1 501 510 octets, sha256 `320d2b56…febeb12`. Relevé
  le 13/10/2026 (lot U7) par `curl` sur
  `https://www.urssaf.fr/files/live/sites/urssaffr/files/outils-documentation/guides/Guide-CFPTA.pdf` (lien de la page
  ci-dessous ; HTTP 200).
- `urssaf-contributions-formation-professionnelle-cfp-cpf-cdd-20240327.html` (texte de l'article seul : `.txt`) : page
  urssaf.fr « Les contributions à la formation professionnelle (CFP et CPF-CDD) », « Publié le 27 mars 2024 », relevée le
  13/10/2026 sur `https://www.urssaf.fr/accueil/employeur/cotisations/liste-cotisations/formation-professionnelle.html`
  (le site ferme la connexion à `curl` sans en-tête `User-Agent` de navigateur ; HTTP 200 avec).
- Les fiches net-entreprises 2502 (CPF-CDD) et 2503 (CFP) auxquelles la page et le guide renvoient sont dans
  `../net-entreprises-base-connaissances/`.

## Ce qu'on en lit

- Circulaire, § 1.3 : la caisse de congés paie les cotisations de sécurité sociale, la CSG-CRDS et l'assurance chômage
  sur les indemnités de congés qu'elle verse ; la **LFSS 2013, art. 40**, transfère aux employeurs « la charge du
  paiement du FNAL et du versement de transport dus sur les indemnités de congés payés », « sous forme d'une
  majoration du FNAL et du versement de transport dont ils sont redevables au titre des rémunérations versées […] à
  compter du 1er janvier 2013 ». « Le taux de ces majorations est fixé par décret à 11,5 % (art. **L. 243-1-3** nouveau du
  code de la sécurité sociale et **décret n° 2012-1552 du 28 décembre 2012**). » « En pratique, la déclaration […] est
  effectuée par une majoration de l'assiette afférente aux contributions VT et FNAL », sans CTP propre.
- Fiche Audiens, exemple de bulletin d'un artiste (brut 400, abattement 25 %, brut abattu 300 ; lignes 64-95 du
  `.txt`) : « aide au logement TA 334,50 × 0,07 % = 0,23 » — 334,50 = 300 × 1,115, la majoration sur le brut abattu ;
  la CSA (0,30), le dialogue social (0,016) et la formation professionnelle (2,1) sur 300, sans majoration. Et
  (lignes 29-31) : une cotisation congés « sur chaque bulletin de paie d'artiste ou de technicien embauché sous
  contrat à durée déterminée de moins de 12 mois ».
- Circulaire, **annexe 1** (relue dans le `.txt` le 10/10/2026, lot V1n) :
  - **R2** (lignes 1806-1822) : « Si la rémunération perçue par le salarié excède le plafond, seul le montant plafonné
    est concerné par la majoration de 11,5 % » ; sinon « l'intégralité des rémunérations donne lieu à la majoration ».
    Ce que le lot 1i avait posé par inférence est donc écrit.
  - **R5** (lignes 1868-1879) : la majoration « ne concerne que les seules rémunérations des salariés que l'employeur
    est tenu de déclarer obligatoirement à une caisse de congés-payés » ; pour les entrepreneurs de spectacles, « le
    personnel artistique et technique […] qui n'a pas été occupé d'une manière continue pendant les 12 mois précédant
    la demande de congés ».
  - **R6** (lignes 1880-1894) : sous déduction forfaitaire spécifique, l'assiette ne descend pas sous l'assiette
    minimum, et « la majoration de 11,5 % est également appliquée sur l'assiette minimum » — la majoration porte sur
    l'assiette abattue, ce que la fiche Audiens montre (300 abattus, FNAL sur 334,50).
- Tissot : « En cas d'option pour l'abattement de 10 %, ces prélèvements, dont la base est alignée sur celle des
  cotisations de Sécurité sociale, sont assis sur le brut abattu » ; « pour le versement de transport, le CTP 900 » ;
  la majoration « ne concerne donc pas les rémunérations des salariés en CDD d'une durée au moins égale à un an » (BTP).

## Ce qui est en vigueur aujourd'hui (relevé LEGI du 10/10/2026)

Relevé par `atelier/outils/relever_les_codes.py` (base LEGI de la DILA, 455 livraisons sur 455), groupe
`majoration_caisses` de `TEXTES` :

- `../code-de-la-securite-sociale/articles-20261010.md`, « Les majorations des employeurs affilies a une caisse de
  conges payes » (XML : `../code-de-la-securite-sociale/xml/L243-1-3.xml`, `D243-0-2.xml`, `D243-0-3.xml`) :
  - **CSS L. 243-1-3** (`LEGIARTI000043701866`, en vigueur depuis le 01/01/2022), 1° : les employeurs affiliés aux
    caisses de congés (L. 3141-32 du code du travail) « s'acquittent de leurs cotisations mentionnées à l'article
    L. 813-4 du code de la construction et de l'habitation [FNAL], de leurs versements mentionnés aux articles
    L. 2333-64 et L. 2531-2 du code général des collectivités territoriales [versement mobilité] ainsi que de leurs
    contributions mentionnées aux articles L. 6131-2, L. 6242-1 et L. 6331-6 du code du travail par le versement
    libératoire de majorations proportionnelles » ; 2° : les cotisations de sécurité sociale, la CSG, la CRDS et la
    contribution de L. 14-10-4 du CASF (la contribution solidarité autonomie) sont versées par la caisse elle-même.
  - **CSS D. 243-0-2** (`LEGIARTI000044972558`, depuis le 01/01/2022) : « 1° Le taux des majorations prévues à
    l'article L. 243-1-3 est fixé à 11,5 %. 2° Par exception au 1°, le taux de majoration pour la taxe d'apprentissage
    mentionnée à l'article L. 6241-1 et la contribution supplémentaire à l'apprentissage mentionnée à l'article
    L. 6242-1 est fixé à 10 % pour les salariés intermittents du spectacle mentionnés à l'article L. 6331-55. »
  - CSS D. 243-0-3 : le taux du versement des caisses elles-mêmes (3,66 % hors BTP) ; ne concerne pas l'employeur.
- `../code-du-travail/articles-20261010.md`, « Les contributions de formation et d'apprentissage majorees chez un
  employeur affilie a une caisse de conges » : L. 3141-32 (les caisses), **L. 6131-2** (la contribution unique à la
  formation professionnelle et à l'alternance = la taxe d'apprentissage de L. 6241-1 + la contribution à la formation
  de L. 6331-1 et L. 6331-3), L. 6241-1 (la taxe d'apprentissage), L. 6242-1 (la contribution supplémentaire à
  l'apprentissage, entreprises d'au moins 250 salariés), **L. 6331-6** (la contribution CPF-CDD, 1 %), **L. 6331-55**
  (la participation unique des intermittents du spectacle, au moins 2 %, « par dérogation » à L. 6331-6, L. 6331-1 et
  L. 6331-3), et D. 7121-28, D. 7121-40, D. 7121-41 (l'affiliation aux Congés Spectacles ; « les employeurs déclarent
  à la caisse de congés payés le personnel artistique et technique qu'ils n'ont pas employé de façon continue pendant
  les douze mois précédant la demande de congé »).
- Le versement mobilité lui-même : `../code-general-des-collectivites-territoriales/` (L. 2333-64, L. 2531-2).
- **Non relevé** : L. 813-4 du code de la construction et de l'habitation (le FNAL), hors des codes de `TEXTES`.

## Ce que le moteur en fait (lot V1n, 10/10/2026)

Une majoration se pose **une fois**, dans le barème : `majoration_conges_115` (1,115) et `majoration_conges_110` (1,10)
du corps (l'intermittent), 1,0 pour `par_nature.permanent`. `calcul::calculer` en tire `brut_x115`, `brut_x110`,
`brut_x115_plafonne` (R2) et, sous déduction forfaitaire, leurs formes abattues (`brut_x115_dfs`, `brut_x110_dfs`,
`brut_x115_plafonne_dfs`, R6) ; le bulletin, le bloc 81 et le bordereau lisent ces assiettes.

- **FNAL** (CTP 332/334, bloc 81 `049`) : `brut_x115_plafonne`. Conforme (L. 243-1-3, D. 243-0-2 1°, R2).
- **Versement mobilité** (bloc 81 `081`, CTP 900 et 921) : `brut_x115` depuis le lot V1n (`bareme::bareme_vm`,
  `dsn::lignes_bordereau`) ; le brut avant. Le 78.004 de la base 03 reste le brut.
- **Taxe d'apprentissage** (bloc 81 `130`, CTP 992, solde 995 en avril) : `brut_x110`. Conforme (D. 243-0-2 2°) ; le
  solde se calcule sur les assiettes 992 déclarées, donc majorées.
- **Formation des intermittents** (bloc 81 `128`, 2 %) : le brut, sans majoration. L. 243-1-3 cite L. 6131-2 (qui
  renvoie à L. 6331-1 et L. 6331-3) et L. 6331-6, pas L. 6331-55, contribution dérogatoire. *Lecture* : le texte ne
  la vise pas ; la fiche Audiens (formation 2,1 % sur 300, non majorée) va dans le même sens.
- **Contribution supplémentaire à l'apprentissage** (L. 6242-1) : 250 salariés et plus ; le moteur ne la calcule pas.
  Si elle l'était un jour : 10 % pour un intermittent (D. 243-0-2 2°).
- **CPF-CDD** (L. 6331-6, bloc 81 `129`, CTP 987) et **formation de droit commun** (L. 6331-1, bloc 81 `128`, CTP 959) : pas
  dues par un intermittent (L. 6331-55 y déroge) ; un CDI n'est pas affilié (majoration 1,0). Pour un **CDD de droit commun
  d'un artiste déclaré à la caisse** : `brut_x115` depuis le lot U7, voir « La CPF-CDD d'un CDD déclaré à la caisse » plus bas.
- **CSA, dialogue social, cotisations de sécurité sociale, CSG/CRDS** : pas dans L. 243-1-3 1° ; brut, sans majoration.
- **Le permanent** : pas affilié, majorations à 1,0 ; rien ne change.
- **Qui est déclaré à la caisse** (lot V2f, `../caisse-conges-affiliation/LISEZ-MOI.md`) : un CDDU de douze mois ou plus
  (du début au terme signés) ne l'est pas — ni Congés Spectacles ni majoration (R5), dès son premier bulletin ; un CDD
  de droit commun de moins de douze mois d'un artiste l'est — Congés Spectacles, et les majorations à 11,5 %, taxe
  d'apprentissage comprise (D. 243-0-2, 1° : le 10 % ne vise que les intermittents de L. 6331-55). Restent à décider :
  le technicien en CDD de droit commun, les contrats successifs qui font douze mois bout à bout.

## La CPF-CDD d'un CDD déclaré à la caisse (lot U5, 13/10/2026)

**La question** : le CDD de droit commun d'un artiste de moins de douze mois est déclaré à la caisse des Congés Spectacles
(`../caisse-conges-affiliation/LISEZ-MOI.md`, lot V2f) et paie les majorations de L. 243-1-3 sur le FNAL, le versement
mobilité et la taxe d'apprentissage. Paie-t-il aussi la majoration sur sa contribution CPF-CDD (L. 6331-6, 1 %) ?

**Ce que le texte dit** (tous les textes sont déposés ici ou dans les dossiers des codes ; relevé LEGI du 13/10/2026,
`../code-de-la-securite-sociale/articles-20261013.md`, `../code-du-travail/articles-20261013.md`) :

1. **CSS L. 243-1-3, 1°** : « les employeurs affiliés auxdites caisses de congés s'acquittent de leurs cotisations
   mentionnées à l'article L. 813-4 du code de la construction et de l'habitation, de leurs versements mentionnés aux
   articles L. 2333-64 et L. 2531-2 du code général des collectivités territoriales **ainsi que de leurs contributions
   mentionnées aux articles L. 6131-2, L. 6242-1 et L. 6331-6 du code du travail** par le versement libératoire de
   majorations proportionnelles aux cotisations et contributions correspondantes dont ils sont redevables au titre des
   rémunérations qu'ils versent pour l'emploi de leurs salariés. Le taux de ces majorations est fixé par décret ».
   **L. 6331-6 est nommé** : la CPF-CDD est une des contributions que la majoration acquitte.
2. **CSS D. 243-0-2, 1°** : « Le taux des majorations prévues à l'article L. 243-1-3 est fixé à 11,5 %. » Le 2° (10 %) ne
   vise que « la taxe d'apprentissage mentionnée à l'article L. 6241-1 et la contribution supplémentaire à l'apprentissage
   mentionnée à l'article L. 6242-1 » des salariés intermittents de L. 6331-55 : **la CPF-CDD est à 11,5 %**.
3. **C. trav. L. 6331-6** : la contribution est « égale à 1 % du revenu d'activité retenu pour le calcul des cotisations
   sociales des titulaires d'un contrat à durée déterminée », hors CDD de formation et d'alternance (D. 6331-72) et
   saisonniers. **L. 6331-55** y déroge pour les « salariés intermittents du spectacle […] pour lesquels il est d'usage
   constant de ne pas recourir au contrat à durée indéterminée » : le CDD d'usage n'est pas concerné ; le CDD de droit commun
   (L. 1242-2, 1° et 2°) l'est.
4. **C. trav. L. 6131-2** (même alinéa de L. 243-1-3) : la taxe d'apprentissage et la contribution à la formation
   professionnelle de L. 6331-1 et L. 6331-3 sont majorées de la même façon : la formation professionnelle de droit
   commun du CDD déclaré (0,55 % sous onze salariés) est dans le même cas que la CPF-CDD.

**Donc** (lecture du texte, sans autre inférence que « qui est affilié », décidée au lot V2f) : un CDD de droit commun d'un
artiste affilié à la caisse paie sa CPF-CDD à 1 % × 1,115 du revenu d'activité retenu, et non 1 %. Sa contribution à la
formation professionnelle de L. 6331-1 est majorée de même.

**Ce que le texte ne dit pas** (inférence ou choix à faire avant de coder) :
- **La forme de la déclaration.** La lettre circulaire Acoss 2013-019 (§ 1.3, lignes 303-345 du `.txt` : « en pratique, la déclaration […] est
  effectuée par une majoration de l'assiette afférente aux contributions VT et FNAL », sans CTP propre) date de 2013 et ne
  parle que du FNAL et du versement transport ; aucun texte déposé ne dit si le 81.003 de la cotisation 129 porte l'assiette
  majorée (comme le fait le moteur pour le FNAL) ou l'assiette nue avec un montant majoré, ni si le CTP 987 du bordereau
  (somme des 81.003 de la 129) suit l'assiette majorée.
- **Sous déduction forfaitaire spécifique**, la base de L. 6331-6 est « le revenu d'activité retenu pour le calcul des cotisations
  sociales » : l'assiette abattue ; la majoration porte alors sur l'assiette abattue (lettre circulaire, annexe 1, R6, pour le
  FNAL), ce que les assiettes `brut_x115_dfs` du moteur savent déjà faire.
- Si le moteur arrondit la majoration à l'euro ou au centime (le bulletin calcule chaque composant au centime ; le nominatif
  ne s'arrondit pas, guide Urssaf § 1.6).

**Ce que le moteur faisait avant le lot U7** (constaté le 13/10/2026, lot U5) : `bareme::bareme_cpf_cdd` ajoute 1,0 aux composants de
`autres_1` et la cotisation 129 sur la base `brut`, sans la majoration `majoration_conges_115` que `par_caisse_conges.affilie`
a posée ; aucun test ne regarde un artiste en CDD de droit commun affilié (`tests/dsn/cpf_cdd.rs` ne couvre qu'une attachée de
production, technicienne, non déclarée à la caisse). La forme de la déclaration était à trouver.

### La forme de la déclaration (lot U7, 13/10/2026)

**Les textes de l'Urssaf et de net-entreprises répondent** (déposés le 13/10/2026, voir « Les fichiers ») :

- **CFP** (L. 6331-1). Guide Urssaf du 12/03/2026, `.txt` l. 657-671 (p. 21-22) : « Conformément aux articles L. 243-1-3 et
  D. 243-0-2 du code de la sécurité sociale, s'agissant des entreprises de BTP, de transport ou des dockers, dont les congés
  payés sont réglés par des caisses, les salaires à déclarer sont ceux effectivement payés par l'entreprise, majorés d'un
  coefficient forfaitaire de 1,1150 au titre des indemnités de congés (soit une majoration de 11,50 % de la masse salariale).
  Néanmoins, l'entreprise conserve la possibilité de majorer au réel » ; exemple : « (100 000 x 1,115) = 111 500, soit une
  assiette de 111 500 euros ». Son tableau de la cotisation 128, l. 805-826 (p. 25-26) : base 03 ; « S21.G00.81.003 Montant
  d'assiette [...] Pour les salariés affiliés à une caisse de congés payés du BTP, du Transport et des Dockers, le montant
  d'assiette doit intégrer la majoration de 11,5% ». Fiche net-entreprises 2503 (`../net-entreprises-base-connaissances/
  net-entreprises-fiche2503-contribution-formation-professionnelle-20250429.txt`, l. 36 et 104) : la même phrase, et
  « le montant d'assiette est majoré de 11,5% » au 81.003, le 78.004 de l'exemple restant le salaire (3000.00). Page urssaf.fr
  (`urssaf-contributions-formation-professionnelle-cfp-cpf-cdd-20240327.txt`, l. 71-77) : « Oui, les salaires à déclarer
  sont ceux que vous avez effectivement payés, majorés d'un cœfficient forfaitaire de 1,1150 » ; CTP 959 (l. 101).
- **CPF-CDD** (L. 6331-6). Guide, l. 966-990 (p. 31) : la même phrase et le même exemple sous « l'assiette de la contribution
  dédiée au financement du Compte personnel de formation des titulaires de Contrats à durée déterminée » ; l. 1061-1066
  (p. 33) : la cotisation 129 au bloc 81, « La masse salariale assujettie à la contribution au CPF-CDD est déclarée par le
  CTP 987 ». Page urssaf.fr, l. 129-135 : « devez-vous majorer l'assiette de la contribution au CPF-CDD ? Oui, les salaires
  à déclarer sont ceux [...] majorés d'un coefficient forfaitaire de 1,1150 ». La fiche 2502 (`net-entreprises-fiche2502-
  cpf-cdd-20250322.txt`) donne la 129 sous la base 03 et ne parle pas des caisses.
- **Les intermittents** (guide, l. 710-712, p. 22) : « c'est la caisse de congés payés Audiens qui déclare et règle auprès de
  l'Urssaf la CFP des intermittents sur la fraction de masse salariale retenue pour le calcul des congés payés » : leur 2 %
  (CTP 983) reste sur le brut, ce que le moteur faisait.

**Ce que le moteur en fait** (lot U7) : comme le FNAL. Le barème du permanent (`par_nature.permanent`, socles 2025 et 2026)
met la formation sur sa propre ligne du bulletin, `autres_4` (« Autres contributions dues par l'employeur », regroupée avec
les autres au modèle), sur `brut_x115` ; la CSA et le dialogue social restent dans `autres_1`, sur le brut (L. 243-1-3, 2° :
la caisse les verse). `cpf_cdd.rubrique` est `autres_4` ; les 81.003 des 128 et 129 sont `brut_x115`, le 78.004 de la base
03 reste le brut. Au bordereau, le 987 est la somme des 81.003 de la 129 (déjà) et le 959 celle des 81.003 de la 128 des
permanents (`cfp_permanent`, `dsn::assiettes_ctp` ; un `config/organismes.json` qui met le 959 ou le 971 sur
`brut_permanent` se lit ainsi, `dsn::ctp_urssaf`). La majoration vaut 1,0 hors de la caisse : rien ne bouge pour un CDI, un
CDD de technicien ou un CDD de douze mois. Preuve : `tests/dsn/cpf_cdd.rs`,
`un_cdd_declare_a_la_caisse_majore_de_11_5_pour_cent_l_assiette_de_sa_formation_et_de_sa_cpf_cdd` (2 400,00 → 2 676,00 ;
14,72 + 26,76 au bulletin et aux 81.004 ; 2 676 aux CTP 959 et 987).

**Inférences, écrites comme telles** :
- Le guide, la page et la fiche nomment « le BTP, le transport ou les dockers » ; ils citent L. 243-1-3, qui vise tous les
  employeurs affiliés aux caisses de L. 3141-32, la caisse des Congés Spectacles comprise (D. 7121-28 à D. 7121-41). La forme
  est appliquée au spectacle par ce renvoi.
- Le 81.003 de la **129** : le guide ne le dit qu'au 81.003 de la **128** ; pour la CPF-CDD il dit que « les salaires à
  déclarer » sont majorés, et le CTP 987 est la somme des 81.003 de la 129 (DIDA). Le moteur majore le 81.003 de la 129 comme
  celui de la 128.
- Le coefficient forfaitaire, pas « le réel » : l'option du réel est laissée à l'employeur ; le moteur ne la propose pas.
- L'arrondi : chaque contribution au centime sur le bulletin (`calcul::somme_pct`, comme les autres lignes) ; l'assiette
  majorée au centime par rubrique de salaire (`eur(montant × 1,115)`), comme celles du FNAL et de la taxe d'apprentissage.
- Sous déduction forfaitaire spécifique : sans objet ici, un contrat au mois la refuse (`calcul::dfs_applicable`).

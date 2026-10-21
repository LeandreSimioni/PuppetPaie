# Code du travail : le relevé LEGI

Les `articles-AAAAMMJJ.md`, `releve.json` et `xml/` de ce dossier sont **écrits par `atelier/outils/relever_les_codes.py`**
(`moissonner`, puis `rapport`) depuis la base LEGI de la DILA ; ils ne se retouchent pas à la main. Les articles voulus sont la
liste `TEXTES["travail"]["voulus"]` de l'outil ; le dernier relevé est le plus récent des `articles-*.md`. Ce fichier dit, par
sujet, ce qu'on en lit et où le moteur s'en sert.

## La fin d'un CDD : à son terme, ou avant (relevé du 13/10/2026, lot U5)

`articles-20261013.md`, section « La fin d'un CDD : a son terme, ou avant » (XML : `xml/L1243-1.xml`, `xml/L1243-2.xml`,
`xml/L1243-5.xml`), et, dans la section « Le preavis et l'indemnite de licenciement », L1234-1 (`xml/L1234-1.xml`).

- **L1234-1** (`LEGIARTI000006901112`, depuis le 01/05/2008) : « **Lorsque le licenciement n'est pas motivé par une faute
  grave**, le salarié a droit : 1° S'il justifie chez le même employeur d'une ancienneté de services continus inférieure à six
  mois, à un préavis dont la durée est déterminée par la loi, la convention ou l'accord collectif de travail ou, à défaut, par
  les usages […] ; 2° […] entre six mois et moins de deux ans, à un préavis d'un mois ; 3° […] d'au moins deux ans, à un préavis
  de deux mois. » C'est le préavis du **licenciement**, par la première phrase de l'article elle-même ;
  les mises et départs à la retraite y renvoient (L1237-6 et L1237-10, `articles-20261013.md` l. 325 et 345).
- **L1243-5** (`LEGIARTI000029946316`, depuis le 22/12/2014) : « Le contrat de travail à durée déterminée **cesse de plein droit
  à l'échéance du terme**. Lorsqu'il est conclu en application du 6° de l'article L. 1242-2, il prend fin avec la réalisation de
  l'objet pour lequel il a été conclu après un délai de prévenance au moins égal à deux mois. » Le texte ne donne **aucun
  préavis** à un CDD qui arrive à son terme ; le délai de prévenance de deux mois est celui du CDD **à objet défini**
  (L1242-2, 6°), que le moteur ne porte pas.
- **L1243-1** (`LEGIARTI000029946319`) : sauf accord des parties, le CDD ne peut être rompu avant l'échéance du terme qu'en cas
  de faute grave, de force majeure ou d'inaptitude ; **L1243-2** (`LEGIARTI000031087476`) : le salarié qui justifie d'un CDI
  peut le rompre avant le terme, avec un préavis d'« un jour par semaine », deux semaines au plus. Ce préavis-là est celui d'une
  **rupture anticipée à l'initiative du salarié** (motif de rupture 037 de la DSN, `bloc-S21.G00.62.txt` l. 265 ; le 036 est celui de l'employeur) ; le moteur ne le calcule pas.

**Où le moteur s'en sert** : `logiciel/donnees/permanents-base.json`, `preavis.motifs_sans_preavis["031"]` (fin de CDD arrivé à
son terme : aucun préavis, cite L1243-5) ; `atelier/sources/paie/src/permanents.rs` (`preavis_mois`, `raison_sans_preavis`) ;
les phrases de `routes_listes.rs` et `routes_ouverture.rs` (`ce_qui_manque`, « La loi ne donne aucun preavis pour ce motif »)
et `solde.rs` (un préavis déclaré non effectué sur un motif sans préavis). Le bloc S21.G00.63 d'un signalement de fin de CDD se
déclare avec le type « 90 - pas de clause de préavis applicable » (norme DSN, `atelier/reference/norme-dsn/texte/bloc-S21.G00.63.txt`).
Test : `tests/routes/routes_listes.rs`, `un_cdd_arrive_a_son_terme_n_a_aucun_preavis`.

**Inférence** : la lecture de « motif de rupture 031 » comme « CDD arrivé à son terme » est celle de la norme DSN
(`bloc-S21.G00.62.txt`, « 031 - fin de contrat à durée déterminée ou fin d'accueil occasionnel »).

## La rupture d'un CDD avant son terme, motif par motif (relevé du 13/10/2026, lot U7)

Ajoutés à `TEXTES["travail"]["voulus"]` (`fin_de_cdd`) et relevés par `moissonner` (0 livraison nouvelle sur 458, « manquants :
aucun »), puis `rapport travail` : **L1243-3**, **L1243-4**, **L1226-4-2**, **L1226-4-3**, **L1226-20** (`articles-20261013.md`,
même section ; `xml/`).

- **L1243-3** (`LEGIARTI000006901214`) : la rupture à l'initiative du salarié « en dehors des cas prévus aux articles L. 1243-1 et
  L. 1243-2 ouvre droit pour l'employeur à des dommages et intérêts ».
- **L1243-4** (`LEGIARTI000024026880`) : la rupture à l'initiative de l'employeur « en dehors des cas de faute grave, de force
  majeure ou d'inaptitude » ouvre au salarié des dommages et intérêts « d'un montant au moins égal aux rémunérations qu'il aurait
  perçues jusqu'au terme du contrat » ; al. 2 : la force majeure d'un sinistre donne « une indemnité compensatrice dont le montant
  est égal aux rémunérations qu'il aurait perçues jusqu'au terme du contrat ».
- **L1226-4-2**, **L1226-4-3**, **L1226-20** : l'inaptitude d'un salarié en CDD ; la rupture « ouvre droit [...] à une indemnité »
  (au moins celle de L1234-9, le double pour une inaptitude d'origine professionnelle), versée comme l'indemnité de précarité.

**Ce qu'ils tranchent, par motif de la nature 02** (`contrats-base.json`, CCH-11) : aucun de ces articles ne donne de préavis,
sauf L1243-2. 036 (employeur), 084 (accord, « Sauf accord des parties », L1243-1), 095 et 096 (faute grave ou lourde, L1243-1),
083 (force majeure, L1243-1 et L1243-4), 033 (inaptitude, L1243-1 et L1226-4-3) : **aucun préavis** ; ce qu'ils ouvrent est une
indemnité ou des dommages et intérêts, que le moteur ne calcule pas. 037 (salarié) : **le préavis de L1243-2** s'il justifie d'un
CDI, un jour par semaine de la durée totale du contrat, deux semaines au plus, sauf accord ; le moteur ne sait ni le CDI ni le
terme d'origine (un CDD rompu porte sa date de rupture en `fin`) : il dit la règle et demande la durée.

**Où le moteur s'en sert** : `permanents-base.json`, `preavis.rupture_anticipee_cdd` (`motifs_sans_preavis`, `motifs_a_dire`),
lue pour un contrat à durée déterminée seulement (083 finit aussi un CDI) ; `permanents.rs` (`preavis_mois`, `regle_du_preavis`,
`raison_sans_preavis`, qui prennent `cdd`) ; `solde::duree_du_preavis` (ni la loi du licenciement ni la convention pour 037) ; les
deux écrans, `routes_listes::ce_qui_manque` et `routes_ouverture::ce_qui_manque_au_signalement`. Tests :
`tests/routes/routes_listes.rs` (`un_cdd_rompu_avant_son_terme_…`, `un_cdd_rompu_par_le_salarie_…`),
`tests/routes/routes_ouverture.rs` (`le_rappel_d_une_fin_de_cdd_…`), `tests/calcul/permanents.rs` (`le_solde_d_un_cdd_rompu_…`).

**Lectures, écrites comme telles** : « aucun préavis » est l'absence de texte qui en donne un, L1234-1 ne visant que le
licenciement ; pour 084, la date de fin est celle que l'accord fixe. **Question ouverte**, close par le lot P1 (section « Le préavis
non effectué, motif par motif ») : le solde de tout compte payait un préavis non effectué au titre de L1234-5 (licenciement) pour
un 037 dont le préavis était saisi à la main ; aucun texte ne la donne côté CDD, il ne la paie plus.

## L1234-5, l'indemnité compensatrice de préavis : ce que le texte place où (lot T1, 13/10/2026)

**L1234-5 était déjà relevé** (`TEXTES["travail"]["voulus"]["rupture"]`, `articles-20261013.md` l. 250-254, `xml/L1234-5.xml`,
`LEGIARTI000006901118`, en vigueur depuis le 01/05/2008) ; l'entrepôt `../legi-entrepot/` est à jour au 13/10/2026 (dernière livraison
appliquée `20261012-212534`, la dernière du serveur le même jour) : `moissonner` n'a rien à ajouter, `rapport` ne ferait que
redoubler `articles-20261013.md`. Ce qui manquait n'était pas l'article, c'était sa place dans le code, que le XML donne
(`<CONTEXTE>`, `<TITRE_TM>`) : **Livre II, Titre III « Rupture du contrat de travail à durée indéterminée », Chapitre IV
« Conséquences du licenciement », Section 1 « Préavis et indemnité de licenciement », sous-section 1 « Préavis et indemnité
compensatrice de préavis ».**

- Le texte : « Lorsque le salarié n'exécute pas le préavis, il a droit, sauf s'il a commis une faute grave, à une indemnité
  compensatrice. L'inexécution du préavis, **notamment en cas de dispense par l'employeur**, n'entraîne aucune diminution des
  salaires et avantages que le salarié aurait perçus s'il avait accompli son travail jusqu'à l'expiration du préavis, indemnité de
  congés payés comprise. » Il ne nomme ni l'auteur de la rupture ni la nature du contrat ; il se lit dans son chapitre, celui du
  **licenciement d'un CDI**.
- **Un CDD n'y figure pas** : sa rupture avant terme est au Titre IV (L1243-1 à L1243-4, `xml/L1243-*.xml`), qui ne donne qu'un
  préavis (L1243-2, du salarié) et, à l'employeur fautif, des dommages et intérêts (L1243-4), jamais une « indemnité compensatrice
  de préavis ».
- **Pour un 037** (rupture anticipée du CDD à l'initiative du salarié, `bloc-S21.G00.62`), aucun texte déposé n'ouvre à l'employeur
  ni au salarié une indemnité de préavis : L1243-2 donne le préavis du salarié, L1243-3 des dommages et intérêts à l'employeur s'il
  le viole. **Lecture, écrite comme telle** : L1234-5 est un droit du salarié licencié (ou dispensé de préavis par l'employeur) ;
  l'appliquer à un 037 serait lui faire dire ce que son chapitre ne dit pas. Ce que le droit jurisprudentiel dit d'un salarié
  démissionnaire dispensé par l'employeur n'est **pas** un texte du dépôt (non lu, donc non écrit). Le démissionnaire d'un CDI
  relève de L1237-1 (« l'existence et la durée du préavis sont fixées par la loi, ou par convention ou accord collectif »,
  `LEGIARTI000006901174`, dans l'entrepôt, non relevé : aucun point du moteur ne le lit aujourd'hui).
- **Où le moteur s'en sert** : `atelier/sources/paie/src/solde.rs` (l. 322-350, le « préavis non effectué (L1234-5) » : la rubrique
  `indemnite_preavis`, mois × salaire, et l'avertissement « congés payés comprise »). Il s'appliquait à tout motif ; depuis le
  lot P1, motif par motif (section « Le préavis non effectué, motif par motif » ci-dessous) : plus rien pour un 037.

## Le préavis non effectué, motif par motif (relevé LEGI du 13/10/2026, lot P1)

Ajoutés à `TEXTES["travail"]["voulus"]` (`preavis_non_effectue`) et relevés par `moissonner` (0 livraison nouvelle sur 458,
« manquants : aucun », « périmés : aucun »), puis `rapport travail` : **L1237-1**, **L1237-2**, **L1226-4**, **L1226-12**,
**L1226-14**, **L1234-12**, **L1234-13**, **L1221-25**, **L1221-26** (`articles-20261013.md`, section « Le preavis non effectue :
ce que chaque fin de contrat en fait » ; `xml/`). Trouvés en cherchant dans l'entrepôt tous les articles du code du travail en
vigueur qui citent « L. 1234-5 » (L1226-14, L1226-4, L1234-13, et hors sujet L3253-3, L8252-2, L8254-2) ou le mot « préavis ».

Ce que chaque motif connu du moteur (`contrats-base.json`, natures `cdi` et `cdd`) fait d'un préavis non exécuté :

| motif | ce que dit le texte | le moteur |
|---|---|---|
| 020, 014 (licenciement) | **L1234-5** (chapitre « Conséquences du licenciement ») : « il a droit, sauf s'il a commis une faute grave, à une indemnité compensatrice » | paie `indemnite_preavis` (DSN 52 : 023) |
| 087, 088 (faute grave, lourde) | L1234-1 (« Lorsque le licenciement n'est pas motivé par une faute grave ») et L1234-5 (« sauf s'il a commis une faute grave ») | aucun préavis, rien à payer |
| 091 (inaptitude non professionnelle) | **L1226-4**, al. 3 : « Par dérogation à l'article L. 1234-5, l'inexécution du préavis ne donne pas lieu au versement d'une indemnité compensatrice » | **rien** (lot P1 ; payait avant) |
| 092 (inaptitude professionnelle) | **L1226-14**, al. 1 : « une indemnité compensatrice d'un montant égal à celui de l'indemnité compensatrice de préavis prévue à l'article L. 1234-5 » (cas du 2e al. de **L1226-12** ; pas due si refus abusif du reclassement, al. 2) | paie le même montant, sur sa rubrique `indemnite_preavis_inaptitude`, déclarée en « 032 » (lot P2 ; c'était 023) |
| 059 (démission) | **L1237-1** : « l'existence et la durée du préavis sont fixées par la loi, ou par convention ou accord collectif » ; **L1237-2** : dommages et intérêts à l'employeur si la rupture est abusive. Aucune indemnité au salarié | **rien** (lot P1) |
| 039 (départ à la retraite) | L1237-10 : le salarié « respecte un préavis » dont la durée suit L1234-1 ; aucun renvoi à L1234-5 | **rien** (lot P1 ; lecture : la même que pour la démission) |
| 037 (CDD rompu par le salarié) | L1243-2 (son préavis), L1243-3 (dommages et intérêts à l'employeur) ; rien au Titre IV ne donne d'indemnité compensatrice | **rien** (lot P1) |
| 036, 084, 095, 096, 083, 033 (CDD) ; 031 ; 043 | aucun préavis (sections ci-dessus) | rien à payer |
| 034 (fin d'essai, employeur) | **L1221-25** : un délai de prévenance (24 h, 48 h, deux semaines, un mois selon la présence) ; s'il n'est pas respecté, « une indemnité compensatrice [...] égale au montant des salaires et avantages que le salarié aurait perçus s'il avait accompli son travail jusqu'à l'expiration du délai de prévenance » | la calcule depuis le lot P2 (`preavis.delai_de_prevenance`) : ligne `indemnite_delai_prevenance`, DSN 52 « 017 » ; décompte : une lecture (ci-dessous) |
| 035 (fin d'essai, salarié) | **L1221-26** : un délai de prévenance que le salarié respecte ; aucune indemnité | rien à payer (prouvé par un test, lot P2) |
| 083 (CDI, force majeure) | **L1234-12** : la cessation de l'entreprise pour force majeure « libère l'employeur de l'obligation de respecter le préavis » ; **L1234-13** : un sinistre ouvre une indemnité « d'un montant égal » à L1234-5 + L1234-9 (DSN 52 : 018) | ne sait pas lequel : paie comme avant, et le dit (à trancher) |
| 038 (mise à la retraite) | L1237-6 : le préavis de L1234-1 ; aucun renvoi à L1234-5 | **à trancher** : paie comme avant, et le dit |
| 058 (prise d'acte), 065 (décès de l'employeur) | aucun article du code relevé | **à trancher** : paie comme avant, et le dit |

**Lectures, écrites comme telles** : L1234-5 se lit dans son chapitre (le licenciement d'un CDI) ; un article qui renvoie à L1234-1
pour la **durée** (L1237-6, L1237-10) ne renvoie pas pour autant à L1234-5. Pour la démission, le départ à la retraite et le 037,
c'est le salarié qui doit le préavis : aucun texte ne lui paie celui qu'il n'exécute pas. Ce que la jurisprudence dit d'un
salarié démissionnaire **dispensé par l'employeur** n'est pas un texte du dépôt (non lu, non écrit) ; s'il y a lieu de payer, la
ligne se saisit à la main. Pour 038, 058, 065 et 083, aucun texte déposé ne tranche dans un sens ni dans l'autre : le moteur ne
change pas ce qu'il faisait, et son avertissement le dit.

**Où le moteur s'en sert** : `logiciel/donnees/permanents-base.json`, `preavis.indemnite_compensatrice` (`motifs_sans_indemnite`,
`motifs_par_un_autre_article`, `motifs_a_trancher`) et `preavis.motifs_sans_preavis["034"]`, `["035"]` ;
`atelier/sources/paie/src/permanents.rs` (`indemnite_de_preavis`, `IndemniteDePreavis`, et depuis le lot P2 `rubrique_de_l_indemnite_de_preavis` : la rubrique, donc le
code du bloc 52, selon le motif, `preavis.indemnite_compensatrice.rubrique_par_motif`) ; `solde.rs` (`solde_de_tout_compte`,
« le préavis non effectué ») ; `routes_accueil.rs` (`faits_de_rupture` : la case « préavis non effectué, payé en indemnité » n'est
proposée que là où elle paie). Tests : `tests/calcul/permanents.rs` (`un_cdd_rompu_par_le_salarie_ne_paie_aucune_indemnite_de_preavis`,
`une_demission_…`, `un_depart_a_la_retraite_…`, `un_licenciement_pour_inaptitude_non_professionnelle_…`, `un_licenciement_paie_…`,
`un_licenciement_pour_inaptitude_professionnelle_…`, `une_mise_a_la_retraite_…`), `tests/routes/routes_accueil.rs`
(`les_natures_payees_au_mois_descendent_quand_la_compagnie_dit_en_employer`) ; lot P2 : `tests/dsn/preavis.rs`
(`le_preavis_non_effectue_d_une_inaptitude_professionnelle_se_declare_en_032`, le témoin `…_d_un_licenciement_se_declare_en_023`,
`le_code_032_n_accompagne_qu_une_inaptitude`).

### Le délai de prévenance d'une fin d'essai (lot P2, 13/10/2026)

**L1221-25** (déjà relevé, `articles-20261013.md`) : le délai (24 h sous huit jours de présence, 48 h de huit jours à un mois,
deux semaines après un mois, un mois après trois mois), « La période d'essai, renouvellement inclus, ne peut être prolongée du
fait de la durée du délai de prévenance », et l'indemnité compensatrice, « sauf s'il a commis une faute grave », « égale au
montant des salaires et avantages que le salarié aurait perçus s'il avait accompli son travail jusqu'à l'expiration du délai de
prévenance, indemnité compensatrice de congés payés comprise ». Pour un CDD : seulement s'il stipule « une période d'essai d'au
moins une semaine ». Le bloc 52 la nomme : « 017 - Dommages et intérêts dus à la non remise du contrat ou dus à un CDD ou à une
rupture de période d'essai (délai de prévenance) » (`../../norme-dsn/texte/bloc-S21.G00.52.txt`, p. 253).

**Lectures, écrites comme telles** (aucun texte déposé ne dit le décompte) : la présence va du premier jour du contrat au jour
de la notification, compris ; « un mois », « trois mois » se mesurent comme l'ancienneté du moteur, « après » voulant dire
au-delà ; le délai court du lendemain de la notification en jours entiers (24 h : un jour ; 48 h : deux ; deux semaines :
quatorze ; un mois : jusqu'au même quantième du mois suivant) ; la part du délai après la fin du contrat se paie au salaire
mensuel, chaque jour au prorata de son mois. Les « avantages » et les congés payés de ces jours ne sont pas ajoutés (le moteur
le dit). À confirmer par l'orchestrateur ou Léandre : une circulaire ou une jurisprudence sur le décompte n'est pas déposée.

**Où le moteur s'en sert** : `logiciel/donnees/permanents-base.json`, `preavis.delai_de_prevenance` (`tranches`, `_lecture`,
`_cdd`, `_faute_grave`) ; `rubriques-base.json`, `indemnite_delai_prevenance` ; `permanents.rs` (`regle_du_delai_de_prevenance`,
`delai_de_prevenance`) ; `solde.rs` (`delai_de_prevenance`). Tests : `tests/calcul/permanents.rs` (une tranche par test,
`une_fin_d_essai_…`, `le_delai_de_prevenance_ne_se_devine_pas`, `une_fin_d_essai_a_l_initiative_du_salarie_ne_lui_donne_rien`),
`tests/dsn/preavis.rs` (`l_indemnite_du_delai_de_prevenance_se_declare_en_017`).

## L'effectif de la formation professionnelle : L6331-1, L6331-1 A, L6331-3 (relevé LEGI du 13/10/2026, lot F1)

Relevés depuis l'entrepôt LEGI (`relever_les_codes.py`, `assujettissement_fiscal`), dans `articles-20261013.md` :
- **L6331-1** : « L'employeur de moins de onze salariés s'acquitte de la contribution à la formation professionnelle […] par
  le versement de 0,55 % » ; **L6331-3** : « L'employeur d'au moins onze salariés […] 1 % » ;
- **L6331-1 A** (`LEGIARTI000038610054`, depuis le 01/01/2020) : « Pour l'application du présent chapitre, l'effectif salarié
  et le franchissement d'un seuil d'effectif salarié sont déterminés selon les modalités prévues à l'article L. 130-1 du code de
  la sécurité sociale » — cinq années civiles consécutives pour franchir le seuil, et « le franchissement à la baisse d'un seuil
  d'effectif sur une année civile a pour effet de faire à nouveau courir » le délai (CSS L130-1, II, dans
  `../code-de-la-securite-sociale/articles-20261013.md`). Le guide Urssaf des contributions de formation du 12/03/2026
  (`../majoration-caisses-conges/urssaf-guide-contributions-formation-apprentissage-20260312.txt`, l. 568-586) le dit de la CFP.

**Où le moteur s'en sert** : `logiciel/donnees/baremes/2025.json` et `2026.json`, `seuils_effectif.valeurs[formation]`
(`source`, `sous_le_seuil`) ; `contexte::controler_effectif` refuse une surcouche qui porte la formation à une autre valeur que
0,55 % tant que le seuil n'est pas franchi (`Effectif::atteint`). Test : `tests/calcul/seuils_effectif.rs`.

## Le maintien de salaire d'un arrêt, et le congé payé suspendu : L1226-1, D1226-1 à D1226-5, L3142-2 (imprévus, 15/10/2026)

Rien de neuf relevé : les articles sont ceux de `articles-20261015.md` (L1226-1, D1226-1 à D1226-8) et de
`articles-20261010.md` (L3142-2). Ce qui change, c'est où le moteur s'en sert depuis qu'un arrêt de travail ou une suspension
est un **imprévu** saisi une fois sur le salarié (`paie/src/imprevus.rs`) :
- **L1226-1** (« incapacité résultant de maladie ou d'accident », pas pour les « salariés intermittents ») : le drapeau
  `maintien_legal` des motifs 01, 04, 05, 06 de `logiciel/donnees/arrets-base.json` (une inférence, écrite comme telle) ;
  `imprevus::maintien_possible` ne dit le maintien de la loi que pour eux, et la route `arret-maintien` refuse un maintien sur un
  contrat qui n'est pas payé au mois ;
- **D1226-3** (pas de carence pour un accident du travail ou une maladie professionnelle, « à l'exclusion des accidents de
  trajet ») : le champ `origine` des motifs 04 (`trajet`), 05 et 06 (`at_mp`), lu par `imprevus::origine` — plus de case à
  remplir ;
- **D1226-4** (les jours déjà indemnisés dans les douze mois) : le champ `jours_deja_indemnises` de l'arrêt ;
- **L3142-2** (les congés de L3142-1 « n'entraînent pas de réduction de la rémunération ») : une suspension au motif 637 ne
  retire rien du salaire (`imprevus::lignes_du_mois`).
Tests : `tests/calcul/imprevus.rs`, `tests/routes/routes_imprevus.rs`.

## Le congé de deuil d'un enfant : L3142-1-1, D3142-1-1 (relevé LEGI du 15/10/2026, lot « le deuil »)

`articles-20261015.md`, section « Le conge de deuil d'un enfant » (`xml/L3142-1-1.xml`, `LEGIARTI000041976515`, depuis le
01/07/2020 ; `xml/D3142-1-1.xml`, `LEGIARTI000042411489`, depuis le 10/10/2020) ; domaine `conge_de_deuil` de
`paie/src/releve_codes/textes.rs`.

- **L3142-1-1** : « en cas de décès de son enfant âgé de moins de vingt-cinq ans ou d'une personne âgée de moins de vingt-cinq
  ans à sa charge effective et permanente, le salarié a droit, sur justification, à un congé de deuil de huit jours qui peuvent
  être fractionnés […]. **Le salarié informe l'employeur vingt-quatre heures au moins avant le début de chaque période
  d'absence.** Le congé de deuil peut être pris dans un délai d'un an à compter du décès de l'enfant. » C'est le motif d'arrêt
  « 19 - Deuil d'enfant » de la DSN (S21.G00.60.001) ; ses indemnités journalières sont celles de CSS L. 331-9
  (`../code-de-la-securite-sociale/`).
- **D3142-1-1** : deux périodes au plus, d'au moins une journée chacune.

**Où le moteur s'en sert** : `logiciel/donnees/obligations-base.json`, obligation `attestation_salaire` (ses `textes`, la
`connaissance_presumee` qui rappelle que le salarié prévient vingt-quatre heures avant, `a_avoir`) ; le calcul est dans
`atelier/sources/paie/src/obligations.rs` (`de_l_arret`). Tests : `tests/routes/routes_obligations.rs`.

## La déclaration préalable à l'embauche (21/10/2026)

Relevés le 21/10/2026 depuis l'entrepôt LEGI (`paie releve-codes moissonner`, puis `rapport` : `articles-20261021.md`,
section « La declaration prealable a l'embauche ») ; domaine `dpae` de `paie/src/releve_codes/textes.rs`.

- **L1221-10** (`xml/L1221-10.xml`, `LEGIARTI000006900849`) : « L'embauche d'un salarié ne peut intervenir qu'après
  déclaration nominative accomplie par l'employeur auprès des organismes de protection sociale désignés à cet effet. »
- **R1221-1** (`xml/R1221-1.xml`, `LEGIARTI000035492419`) : la DPAE comporte, 3°, la « Date et heure d'embauche ».
- **R1221-4** (`xml/R1221-4.xml`, `LEGIARTI000024214323`) : elle « est adressée au plus
  tôt dans les huit jours précédant la date prévisible de l'embauche ».

Ce qu'on en tire quand la date de début d'un contrat déjà déclaré change (inférence, écrite comme telle dans le code) :
avancée, l'embauche a eu lieu avant la date que la DPAE déclarait, la déclaration ne l'a pas précédée (L1221-10) — une DPAE à
la date vraie est à déposer ; reculée, la DPAE déposée a précédé l'embauche et portait sa date prévisible (R1221-4) — rien à
refaire. Le guide de l'API DPAE de l'Urssaf (`../../norme-dpae/5491-API-DPAE-Fonctionnement.pdf`, p. 3 et 9) : pas
d'annule-et-remplace en DPAE ; un doublon est rejeté (code 98).

**Où le moteur s'en sert** : `atelier/sources/paie/src/routes_mois.rs` (`etat_par_paie`, `embauches_a_declarer`, l'étape
DPAE de `bilan_du_mois`) et `routes_regularisation::debut_declare_avant`. Tests : `tests/routes/modifier_apres_echeance.rs`.

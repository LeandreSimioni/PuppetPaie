# Qui l'employeur déclare à la caisse des Congés Spectacles

Pourquoi un CDDU de douze mois ou plus n'a ni Congés Spectacles ni majorations d'assiette, et pourquoi un CDD de droit
commun de moins de douze mois d'un artiste les a (`atelier/sources/paie/src/caisse_conges.rs`,
`affilie` ; l'étage `bareme::bareme_caisse_conges` ; la section `par_caisse_conges` des barèmes 2025 et 2026). Déposé
le 10/10/2026, lot V2f (`prive/journal/norme/corrections/PASSATION-V2f.md`).

## Les fichiers de ce dossier

- `kali-1285-ix1-conges-payes-caisse-20261010.md` : CCNEAC (IDCC 1285), art. IX.1 « Congés payés »
  (KALIARTI000046279857) et l'avenant du 5 avril 2022 qui y ajoute la déclaration à la caisse (KALIARTI000046265853),
  VIGUEUR_ETEN depuis le 15/06/2022. Transcription littérale du champ `contenu`, relevée le 10/10/2026 dans l'entrepôt
  local `../kali-entrepot/articles.jsonl.gz` (base KALI de la DILA, dernière livraison lue 20260924-214040).

## Les textes déposés ailleurs, et ce qu'on en lit

- **Code du travail** (relevé LEGI du 10/10/2026, `../code-du-travail/articles-20261010.md`, section « Les contributions
  de formation et d'apprentissage majorees… », et `xml/`) :
  - **D. 7121-40** : « Les employeurs mentionnés aux articles D. 7121-28 et D. 7121-29 s'affilient, pour le personnel
    artistique et technique qu'ils emploient, à la caisse de congés payés prévue à l'article D. 7121-38. »
  - **D. 7121-41** : « Les employeurs déclarent à la caisse de congés payés le personnel artistique et technique qu'ils
    n'ont pas employé de façon continue pendant les douze mois précédant la demande de congé. »
  - Ni l'un ni l'autre ne nomme une nature de contrat : le texte vise le **personnel artistique et technique**, quel que
    soit son contrat, et la sortie tient à **l'emploi continu de douze mois chez le même employeur**, mesuré à la
    demande de congé.
  - **L. 1242-2** (`../code-du-travail/articles-20260923.md`, l. 185-200) : 1° et 2°, le CDD de droit commun
    (remplacement, accroissement temporaire) ; 3°, le CDD d'usage, « emplois pour lesquels, dans certains secteurs […],
    il est d'usage constant de ne pas recourir au contrat de travail à durée indéterminée en raison de la nature de
    l'activité exercée et du caractère par nature temporaire de ces emplois ».
  - **L. 6331-55** (`articles-20261010.md`) : les « salariés intermittents du spectacle […] pour lesquels il est d'usage
    constant de ne pas recourir au contrat à durée indéterminée » — les mots du 3° de L. 1242-2.
- **Fiche pratique Audiens « Les Congés Spectacles »**, février 2023
  (`../majoration-caisses-conges/audiens-fiche-pratique-conges-spectacles-202302.txt`) :
  - l. 17-21 : la caisse sert « les congés payés aux artistes et techniciens qui n'ont pas été employés de manière
    continue chez un même employeur pendant les douze mois précédant leur demande de congé et ce, quelle que soit la
    nationalité du salarié ou la nature du contrat de travail (CDD de droit commun, CDD d'usage, etc) » ;
  - l. 29-30 : « Une cotisation congés est calculée sur chaque bulletin de paie d'artiste ou de technicien embauché
    sous contrat à durée déterminée de moins de 12 mois. »
- **CCN 3090**, art. 10.1 (KALIARTI000044322266, `../kali-3090-titres-vii-x-xi-annexes-20260924.md`, l. 232-268) :
  - 10.1.1, al. 4 et 5 : le droit commun s'applique « également aux salariés employés sous contrat à durée déterminée
    d'usage lorsqu'ils ont été occupés de manière continue, **dans le cadre d'un même contrat de travail**, pendant les
    12 mois qui précèdent leur demande de congés » ; les salariés « relevant de la caisse des congés spectacles, engagés
    sous contrat à durée déterminée dit d'usage » ont les dispositions particulières, « sauf » ce cas ;
  - 10.1.1, al. 1 (les « artistes, les techniciens et les administratifs » employés douze mois) : **exclu de
    l'extension** (arrêté du 29 mai 2013) ;
  - 10.1.2 : D. 7121-41 repris.
- **CCNEAC (1285)**, art. IX.1 (ce dossier) : D. 7121-41 repris, sans la précision « même contrat ».
- **Lettre circulaire Acoss 2013-019**, annexe 1, R5 (`../majoration-caisses-conges/acoss-lettre-circulaire-2013-019-20130328.txt`,
  l. 1868-1879) : la majoration de 11,5 % « ne concerne que les seules rémunérations des salariés que l'employeur est
  tenu de déclarer obligatoirement à une caisse de congés-payés » — le personnel « qui n'a pas été occupé d'une manière
  continue pendant les 12 mois précédant la demande de congés ».
- **CSS D. 243-0-2** (`../code-de-la-securite-sociale/articles-20261010.md`) : 1°, 11,5 % ; 2°, 10 % pour la taxe
  d'apprentissage « pour les salariés intermittents du spectacle mentionnés à l'article L. 6331-55 ».
- **Liste du champ de l'annexe VIII** (Unédic, 16/12/2016, `../unedic-annexe8-liste-champ-application-20161216.pdf`,
  transcription `.txt` par `pdftotext -layout` le 10/10/2026), p. 1 : l'annexe VIII « s'applique aux ouvriers et
  techniciens » ; p. 21-22 (l. 930-1000 du `.txt`), spectacle vivant (IDCC 1285, 3090) : les fonctions, dont
  « Administrateur de production », « Administrateur de tournée », « Attaché de production/chargé de production ».

## Ce que le moteur en fait (lot V2f)

`caisse_conges::affilie(nature, fiche, début, terme)` :

1. **Un CDI : jamais** déclaré. *Lecture* : D. 7121-41 ne nomme pas le CDI ; la fiche Audiens limite la cotisation au
   « contrat à durée déterminée de moins de 12 mois », et c'est la caisse qui la recouvre.
2. **Un contrat à durée déterminée de douze mois ou plus**, du début au terme signés (le lendemain du terme atteint le
   même jour douze mois plus tard), **n'est pas déclaré, dès son premier bulletin**. Fiche Audiens (« de moins de
   12 mois ») ; CCN 3090 (« dans le cadre d'un même contrat de travail ») ; D. 7121-41 (douze mois continus). Un CDD sans
   terme refuse.
3. **Un CDDU de moins de douze mois** : déclaré (c'était tout le moteur jusque-là).
4. **Un CDD de droit commun de moins de douze mois d'un ARTISTE** : déclaré. L'artiste est le salarié dont le métier
   (`code_emploi_ft` tel que la paie le lit) est un emploi de l'annexe 10 de la nomenclature des emplois du spectacle.
   « Personnel artistique » (D. 7121-40), « artiste » (fiche Audiens) : pas de doute.
5. **Un CDD de droit commun de moins de douze mois d'un TECHNICIEN de la liste** : déclaré. L'annexe 8, art. 1er, § 2
   (`../plafond-intermittents/unedic-annexe8-convention-20241115.txt`, l. 16-22), range « les ouvriers et techniciens
   engagés dans le cadre d'un contrat à durée déterminée, pour assurer l'une des fonctions énumérées dans la liste
   figurant au titre XIV, [...] dans les domaines d'activité ou dans les entreprises énumérés dans la même liste »,
   domaines « définis par les numéros d'identifiant des conventions collectives (IDCC) et la nomenclature des activités
   françaises (NAF) » : dans les mêmes termes que l'artiste de l'annexe 10, sans distinguer CDD et CDDU ; D. 7121-38
   (« le personnel artistique et technique employé de façon intermittente »), D. 7121-40 et D. 7121-41 ne distinguent pas
   non plus. Le technicien est le salarié dont le métier est au catalogue (`emplois-base.json`) et dont la fonction est
   nommée par la liste de la convention de l'employeur (`droit::emplois::Metier::nomme_par_l_annexe8` : `conventions` sans
   `conventions_hors_liste` ; la liste 6 pour la 1285, la 7 pour la 3090, circulaire Unedic 2016-25,
   `../unedic-circulaire-2016-25-annexes-8-10.txt`, l. 567-573 ; le staffeur, le bottier ou le serrurier ne sont que sur
   la 6, le poursuiteur ou le directeur artistique que sur la 7). L'administrateur,
   le chargé et l'attaché de production d'une compagnie du spectacle vivant sont sur cette liste (liste 6 et 7, n° 2 et
   7) : leur CDD de moins de douze mois est déclaré, et la caisse leur paie leurs congés. *Inférence* : l'employeur est
   dans le domaine de sa convention (pour le spectacle vivant, les trois catégories de la liste couvrent tout employeur
   qui fait jouer un spectacle ; les codes NAF des autres domaines ne sont pas confrontés). Un métier hors catalogue ne
   dit pas sur quelle liste il est : non déclaré.

Ce qui en découle, par la pile (`par_caisse_conges` du barème) :

- `non_affilie` (CDDU de douze mois ou plus) : ni la ligne de la caisse, ni la cotisation 066, ni les majorations
  d'assiette (1,0 : FNAL, versement mobilité, taxe d'apprentissage sur le brut — R5) ; ni 40.022, ni certificat Congés
  Spectacles ; pas de majoration de la réduction générale (D. 241-10, que le calcul déduit de l'assiette de la caisse).
- `affilie` (CDD de droit commun d'un artiste ou d'un technicien de la liste) : par-dessus `par_nature.permanent`, la ligne de la caisse et le 066 du corps, le
  40.022, les majorations à 11,5 % — FNAL plafonné (`t1_mensuelle_x115`, R2), versement mobilité, et taxe
  d'apprentissage **à 11,5 % et non 10 %** (D. 243-0-2 : le 10 % vise les intermittents de L. 6331-55, c'est-à-dire le
  CDD d'usage ; *lecture*). Au bordereau : 992 sur la somme des `brut_x110`, 332 des permanents sur `t1_x115_permanent`.
  Les lignes de congés payés de l'employeur refusent (la caisse les paie), et le solde de tout compte ne compose pas
  d'indemnité compensatrice de congés (voir « L'indemnité compensatrice d'un salarié déclaré à la caisse » ci-dessous).

## Ce qui n'est pas porté (décisions à prendre)

- Des **contrats successifs** chez le même employeur qui, bout à bout, font douze mois continus ; un contrat
  **prolongé** au-delà de douze mois (le texte mesure à la demande de congé ; la CCN 3090 dit « même contrat », la
  CCNEAC ne le dit pas).
- (Porté au lot U7, 13/10/2026.) La **formation professionnelle de droit commun** et la **CPF-CDD** du CDD déclaré se
  majorent de 11,5 % (L. 243-1-3 1°) : ligne `autres_4` du permanent, 81.003 des 128 et 129, CTP 959 et 987 sur `brut_x115`
  (`../majoration-caisses-conges/LISEZ-MOI.md`, « La CPF-CDD d'un CDD déclaré à la caisse »).
- L'assiette Congés Spectacles d'un CDD au mois : `mois` et `absence_non_remuneree` y entrent (catalogue) ; l'indemnité
  de précarité, le maintien de salaire n'y entrent pas — aucun texte déposé ne le dit.

## L'indemnité compensatrice d'un salarié déclaré à la caisse (relevé LEGI du 02/11/2026)

Textes : `../code-du-travail/articles-20261102.md` (XML : `../code-du-travail/xml/L3141-30.xml`, `xml/L1242-16.xml`,
`xml/D7121-38.xml`).

- **L. 3141-30** (`LEGIARTI000033020675`) : « Les articles L. 3141-28 et L. 3141-29 ne sont pas applicables lorsque
  l'employeur est tenu d'adhérer à une caisse de congés en application de l'article L. 3141-32. » L. 3141-28 est
  l'indemnité compensatrice due à la rupture.
- **D. 7121-38** : la caisse « assure le service des congés annuels au personnel artistique et technique employé de façon
  intermittente » ; D. 7121-41 dit qui lui est déclaré.
- **L. 1242-16** : l'indemnité compensatrice du CDD est due « dès lors que le régime des congés applicable dans
  l'entreprise ne lui permet pas de les prendre effectivement ».

*Lecture* : le salarié que l'employeur déclare à la caisse (`caisse_conges::affilie`) reçoit ses congés de la caisse ;
l'employeur ne lui doit pas d'indemnité compensatrice. Celui qu'il ne déclare pas (CDD de douze mois ou plus, technicien en
CDD de droit commun, CDI) la reçoit de l'employeur (L. 3141-28, L. 1242-16), comme la CCN 3090, art. 10.1.1, le dit du
salarié occupé douze mois.

**Où le moteur s'en sert** : `engagement/src/solde.rs`, `solde_de_tout_compte` (l'indemnité compensatrice ne se compose
que pour un contrat que l'employeur ne déclare pas), qui le demande à `calcul::saisie_declaree_a_la_caisse_des_conges`
(la fiche telle que la paie la lit au dernier jour du contrat, puis `caisse_conges::affilie`).

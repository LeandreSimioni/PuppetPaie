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
5. **Un CDD de droit commun d'un technicien** : **non porté, décision à prendre** ; le moteur garde ce qu'il faisait (non
   déclaré). D. 7121-40 dit « personnel […] technique » sans le définir ; la seule liste que le moteur ait, l'annexe 8,
   range parmi les « ouvriers et techniciens » l'administrateur, le chargé et l'attaché de production (liste de
   l'Unédic, p. 21-22), et la démonstration `demo-3090-grande` paie précisément une attachée de production en CDD de
   droit commun, congés payés par l'employeur (indemnité compensatrice à la fin du contrat). Lire « technique » comme
   « annexe 8 » changerait cela ; le lire comme la grille « technique » de la CCN 3090 n'a pas d'équivalent hors du
   catalogue ; une case de la fiche serait une saisie. C'est un choix.

Ce qui en découle, par la pile (`par_caisse_conges` du barème) :

- `non_affilie` (CDDU de douze mois ou plus) : ni la ligne de la caisse, ni la cotisation 066, ni les majorations
  d'assiette (1,0 : FNAL, versement mobilité, taxe d'apprentissage sur le brut — R5) ; ni 40.022, ni certificat Congés
  Spectacles ; pas de majoration de la réduction générale (D. 241-10, que le calcul déduit de l'assiette de la caisse).
- `affilie` (CDD de droit commun d'un artiste) : par-dessus `par_nature.permanent`, la ligne de la caisse et le 066 du corps, le
  40.022, les majorations à 11,5 % — FNAL plafonné (`t1_mensuelle_x115`, R2), versement mobilité, et taxe
  d'apprentissage **à 11,5 % et non 10 %** (D. 243-0-2 : le 10 % vise les intermittents de L. 6331-55, c'est-à-dire le
  CDD d'usage ; *lecture*). Au bordereau : 992 sur la somme des `brut_x110`, 332 des permanents sur `t1_x115_permanent`.
  Les lignes de congés payés de l'employeur refusent (la caisse les paie).

## Ce qui n'est pas porté (décisions à prendre)

- Le **technicien en CDD de droit commun** (point 5).
- Des **contrats successifs** chez le même employeur qui, bout à bout, font douze mois continus ; un contrat
  **prolongé** au-delà de douze mois (le texte mesure à la demande de congé ; la CCN 3090 dit « même contrat », la
  CCNEAC ne le dit pas).
- (Porté au lot U7, 13/10/2026.) La **formation professionnelle de droit commun** et la **CPF-CDD** du CDD déclaré se
  majorent de 11,5 % (L. 243-1-3 1°) : ligne `autres_4` du permanent, 81.003 des 128 et 129, CTP 959 et 987 sur `brut_x115`
  (`../majoration-caisses-conges/LISEZ-MOI.md`, « La CPF-CDD d'un CDD déclaré à la caisse »).
- L'assiette Congés Spectacles d'un CDD au mois : `mois` et `absence_non_remuneree` y entrent (catalogue) ; l'indemnité
  de précarité, le maintien de salaire n'y entrent pas — aucun texte déposé ne le dit.

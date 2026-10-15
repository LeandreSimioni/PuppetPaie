# Le format des fiches de paramétrage des organismes complémentaires (FPOC), versions 1.3.9 et 1.4.0

## D'où ça vient

`net-entreprises-fiche691-fiches-de-parametrage-oc-20260723.zip` : l'archive que sert la **fiche 691** de la base de
connaissances net-entreprises (`https://net-entreprises.custhelp.com/app/answers/detail_dsn/a_id/691/`, citée par la page
`https://www.net-entreprises.fr/declaration/precontrole-dsn-fpoc/`), relevée par le conteneur le 11/10/2026 ; elle
contient deux archives « R139 - maj 23 juillet 2026 » et « R140 maj 23 juillet 2026 », dépliées ici. Empreinte SHA-256
(16) de l'archive : voir le commit. Fichiers dépliés :

| fichier | ce que c'est |
|---|---|
| `fpoc-specification-r1.4.0-20260721.pdf` (+ `.txt`) | « Fiche de paramétrage DSN - Organismes Complémentaires », version **1.4.0** (onglets Règles, Versions, Données, Schéma), 15 p., datée du 21/07/2026 |
| `fpoc-specification-r1.3.9-20260721.pdf` (+ `.txt`) | la même en version **1.3.9** (celle des fiches reçues aujourd'hui : `atelier/reference/fiches-parametrage/audiens-2025-12-13-fiche139-expurgee.xml`), 14 p. |
| `fpoc-r1.4.0.xsd`, `fpoc-r1.3.9.xsd` | les schémas XML |
| `fpoc-presentation-versions-139-140-20260720.pptx` (+ `.txt`) | « Fiches de paramétrage V139 - V140, mise à jour du 20 juillet 2026 » : calendrier, conditions tarifaires, delta 139 → 140 |

Les exemples XML (`ExempleMin`, `ExempleMax`) de chaque version sont dans l'archive, non dépliés.

## Ce qu'on en lit (11/10/2026 ; rien n'est codé)

- **C'est le format que `paie/src` lit** quand il relève les fiches Audiens par l'API : la 1.3.9 est la version servie
  (le nom du fichier déposé, `fiche139`), la **1.4.0** arrive (date de la version : 16/06/2025 puis 30/06/2026 ; « purge
  du stock de fiches : non planifié »). Ce qui change en 1.4.0 (onglet Versions, l. 49-75 ; diapositives 20-22) : plafond
  et plancher facultatifs sur les bases et montants forfaitaires (contrôles OC.40, OC.41), cardinalités des critères
  salariés, seuil de 99 valeurs, deux conditions tarifaires de plus, alignement sur le CT 2026.1.0. **À porter** : le
  lecteur de fiches devra accepter les deux versions.
- **Q14 (une famille : combien de bases 79 ?) : c'est la fiche qui le dit**, par ses « conditions tarifaires » (spécification
  1.4.0, l. 504-531 ; diapositives 16-18) : tarif unique « 01 = assuré et tous ses ayants droit », « 02 = isolé », « 03 = avec
  1 conjoint », « 04 = avec 1 enfant »… ; ou composants « 90 = ouvrant droit », « 91 = conjoint », « 92 = chaque ayant droit »,
  « 94 = chaque enfant ». Les diapositives donnent, pour chaque combinaison, **le nombre de bases et montants spécifiques
  (BMS) à déclarer** et les compteurs 70.008/70.009/70.010/70.011 à renseigner : une condition « 01 » = une seule base ;
  « 90/91/94 » = trois (ouvrant droit, conjoint, par enfant). Le moteur ne doit pas choisir : il lit la condition de la fiche.
- **Option et population** (l. 279-297) : « Option [0,1], à ne renseigner que si la donnée doit » figurer en DSN ; code
  option 70.004 et son libellé, « ancien code option » en cas de renumérotation technique. Même chose pour la population
  (70.005). Confirme la FAQ des OC (`../faq-dsn-organismes-complementaires/`) et V3e.
- **D3, un morceau** : la fiche peut porter des **bornes sur la date d'affiliation** (`S21.G00.70.014` : « valeur de la
  borne inférieure / supérieure », l. citées dans les deux spécifications), c'est-à-dire qu'un contrat de la fiche ne vaut
  que pour les affiliations datées dans l'intervalle. Ce n'est pas la liste des cas d'usage de 70.014 (toujours
  introuvable), mais une raison de plus de lire 70.014 depuis la fiche salarié quand l'affiliation ne commence pas avec le
  contrat.
- Niveaux de fiche (l. 20-37) : 2A (contrats, éléments de calcul), 2B (critères salariés : statut, PCS-ESE, ancienneté),
  2C (combinaisons de contrats compatibles, « affiliations obligatoires / automatiques »).

## Où le moteur s'en sert

Le lecteur des fiches (`atelier/sources/paie/src/retours.rs`, `fiche_parametrage`, opération `fiche` de `paie retours`) :
**il lit les deux versions depuis le lot R5b (11/10/2026)**. `FORMATS_FICHE` nomme le format d'après `Version` (139 →
1.3.9, 140 → 1.4.0 ; une autre valeur se lit comme avant, sans format) ; chaque contrat rend ses bases et montants
spécifiques (code nature 79.001, condition tarifaire et son libellé, taux, montant, coef, assiette, et en 1.4.0 le plancher
et le plafond `BasePlancher`/`BasePlafond`), son option et sa population, et `compteurs` : les rubriques 70.008-70.011 que
ses conditions appellent (`CONDITIONS_TARIFAIRES`, la règle de la spécification R1.4.0 l. 527-531, R1.3.9 l. 418-423).
Inférences écrites dans le code : 01, 02 et 90 n'appellent aucun compteur (diapositives 16-17) ; 92 (« chaque ayant
droit ») se range avec les nombres (70.009). Une condition hors des deux formats refuse (les compteurs ne se devinent pas).
Le lecteur ne contrôle pas les combinaisons OC.39 (c'est le concentrateur qui les contrôle à réception).
Fixtures fictives validées contre les deux XSD (`xmllint --schema`) : `paie/tests/donnees/fiches-parametrage/
fiche-fictive-139.xml` et `fiche-fictive-140.xml` ; tests : `paie/tests/retours.rs`, `une_fiche_1_3_9_se_lit_avec_ses_conditions_tarifaires`,
`une_fiche_1_4_0_se_lit_avec_ses_planchers_et_plafonds`, `une_condition_tarifaire_inconnue_refuse`.
**Reste à faire** (hors du lecteur) : que la DSN d'un permanent santé déclare autant de bases 79 et les compteurs que la
fiche dit (Q14) ; aujourd'hui elle ne lit pas la fiche (V3e : une base 79 type 18).

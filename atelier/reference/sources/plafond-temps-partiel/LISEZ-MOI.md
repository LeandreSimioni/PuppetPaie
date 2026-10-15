# Le plafond de la Sécurité sociale d'un salarié à temps partiel, et S21.G00.40.084

Déposé le 10/10/2026, lot V4b (`prive/journal/norme/corrections/PASSATION-V4b.md`). Ce dossier ne porte pas de texte
brut : les textes lus sont relevés ailleurs dans `prive/reference/`, et ce qui suit dit où, ce qu'on en lit, et où le
moteur s'en sert.

## Relu le 13/10/2026 (lot D1) : la santé ne suit pas la correction du temps partiel, la prévoyance la suit

**Ce qui suit tranche le point 4 de « Ce qu'on en lit » du lot R5a (santé et prévoyance), resté en inférence.** Aucun
texte neuf : tout est déjà déposé.

- **La santé est un forfait sur la valeur mensuelle du plafond.** Accord 3090 du 19/07/2024, art. 13
  (`../kali-3090-accord-sante-permanents-20240719-20261011.md`, l. 365-379) : « 1,44 % du PMSS au 1er janvier 2026 » ;
  accord 1285 du 30/01/2026, art. 6 (`../kali-1285-accord-frais-de-sante-20260130-20261011.md`, l. 145-153) : « 0,81 %
  du plafond mensuel de la sécurité sociale au titre de la couverture du seul salarié ». Le « plafond mensuel » est la
  valeur que l'arrêté fixe chaque année (CSS L. 241-3, D. 242-17, D. 242-18 : `../code-de-la-securite-sociale/
  articles-20261013.md`, l. 302-380). R. 242-2, I, dernier alinéa (même fichier) « corrige le plafond » retenu pour les
  cotisations **plafonnées** du salarié, à la durée de son contrat : un forfait n'est pas plafonné.
- **Le texte qui le confirme : la dispense du temps partiel.** CSS R. 242-1-6, 2°, c (relevé le 13/10/2026, `../code-de-la-securite-sociale/articles-20261013.md` et `xml/R242-1-6.xml`,
  `LEGIARTI000038701542`, en vigueur depuis le 01/11/2019 ; repris mot pour mot par les deux accords : 3090 art. 9.2.1,
  l. 228 ; 1285 XII.2.1.8, l. 101) : un salarié « à temps partiel [...] dont l'adhésion au système de garanties les
  conduirait à s'acquitter d'une cotisation au moins égale à 10 % de [sa] rémunération brute » peut se dispenser. Cette
  dispense n'aurait pas d'objet si la cotisation baissait avec le temps de travail. R. 242-1-6 est ajouté à `TEXTES` de
  `atelier/outils/relever_les_codes.py` (domaine `sante_dispenses`).
- **La prévoyance suit le plafond corrigé.** Titre XII de la 1285 (`../kali-1285-titres-ii-iii-x-xi-xii-20260923.md`,
  l. 73) : « la tranche 1 porte sur la rémunération égale au plafond de la sécurité sociale visé à l'article L. 241-3 »,
  le même renvoi que l'ANI Agirc-Arrco du 17/11/2017 (art. 32), que le cahier Agirc-Arrco CT2026 fait proratiser
  (l. 3325-3333). **Inférence** pour la 3090 : ses tranches A disent « tranche A » sans renvoi écrit ; on les lit comme
  la 1285.
- **Reste une inférence** (V3e, non tranchée ici) : le forfait santé d'un contrat qui ne couvre pas tout le mois est
  réduit au prorata des jours (R. 242-2, I, al. 3) ; aucun accord ne le dit.

**Où le moteur s'en sert** : `atelier/sources/paie/src/calcul.rs`, `plafond_de_la_sante` (le plafond de la période sans
la correction) : `pss_periode` (la base de la rubrique `mutuelle` et de la base 18 du bloc 79) et la part patronale de
santé dans l'assiette CSG. Test : `tests/dsn/rubriques_2027.rs`,
`la_sante_d_un_temps_partiel_se_pose_sur_le_plafond_mensuel_entier`.

## Relu le 11/10/2026 (lot R5a) : la proratisation est la règle, y renoncer est le choix

**Ce qui suit corrige les points 1 et 3 de « Ce qu'on en lit » plus bas (lecture du lot V4b, 10/10/2026).**

- **Fiche net-entreprises 3369** (modifiée le 03/04/2026, déposée le 11/10/2026 :
  `../net-entreprises-base-connaissances/net-entreprises-fiche3369-proratisation-plafond-temps-partiel-40-084-20260403.pdf`
  et `.txt`) : « **Par défaut**, le salarié à temps partiel cotise uniquement à hauteur de la quotité de travail de son
  contrat. L'employeur **peut choisir de ne pas proratiser** le plafond de Sécurité sociale pour tous les salariés à
  temps partiel de son **établissement**. » Exemples : 80 %, brut 4 100 → `40.084 = 01`, base 02 = 3 204 ; `40.084 = 02`,
  base 02 = 4 005. Le **forfait jour réduit** (`40.011 = 20`, `40.014 = 10`) en relève aussi (01 ou 02 selon le choix du
  salarié).
- **CSS L. 242-8** (déjà relevé, `../code-de-la-securite-sociale/articles-20261010.md`) : « il est opéré un abattement
  d'assiette » — un impératif ; R. 242-2 en donne la mesure ; BOSS § 760 : « la possibilité de renoncer ».
- **L. 242-10, 1°** : L. 242-8 « n'est pas applicable » aux salariés dont l'emploi donne lieu à des taux, assiettes ou
  montants spécifiques. **BOSS § 1090** (`../boss/boss-assiette-generale-20260801.txt`, l. 1470-1478) : ils « ne sont pas
  éligibles à la réduction du plafond » ; « Sont notamment visés **les artistes du spectacle** ». Le cahier Agirc-Arrco
  CT2026 (`../agirc-arrco/agirc-arrco-aide-codification-dsn-ct2026-v1.0-20251201.txt`, l. 3348-3356) rappelle la même
  exclusion (« Salariés dont l'emploi donne lieu à des taux, assiettes ou montant de cotisations spécifiques »).
- **Norme 2027.1.2** (`../../norme-dsn/`) : journal de maintenance (`dsn-journal-maintenance-2027.1.2-20260615.txt`),
  évolution 16 (l. 347-366, la CCH-12 de 40.084 retirée) et 21 (l. 433-452, 40.084 passe de O à C en DSN mensuelle) ;
  note différentielle 2026.1.2 → 2027.1.2 (`dsn-note-differentielle-ct-2026.1.2-vers-2027.1.2-20251224.txt`, § 4.5,
  l. 1737-1790) : « Pour un salarié à temps partiel, cette rubrique doit être renseignée avec le choix de l'employeur
  [...]. Pour un salarié en forfait jour réduit, [...] le choix du salarié ». Rien pour un temps plein ni un contrat
  d'usage.

**Ce qu'on en lit** :
1. Le plafond d'un salarié au mois à temps partiel (`40.014 = 20`) se proratise par défaut ; la fiche entreprise dit si
   l'établissement y renonce (`plafond_temps_partiel` = « entier »). Les valeurs « proratisé » et « entier » gardent le
   sens du lot V4b ; seul le vide change (V4b : « pas choisi », plafond entier et DSN de 2027 refusée ; désormais : la
   règle).
2. Un artiste (statut « artiste », ou vide, comme le barème le lit) est exclu : plafond entier, quel que soit le choix.
   `40.084 = 02` pour lui est une **inférence** (la norme ne dit rien d'un salarié que la loi exclut ; 02 est ce que le
   calcul fait).
3. `40.084` ne s'écrit que pour un temps partiel (01 ou 02) ; plus de `02` sur un temps plein ni un CDDU.
4. **Inférence** inchangée (V4b) : le moteur n'a qu'un plafond par paie, et toutes les bases le suivent. Pour la retraite
   complémentaire, c'est **lu** : l'ANI du 17/11/2017, art. 32, définit T1 par renvoi au plafond de L. 241-3, et le cahier
   CT2026 (l. 3325-3333) fait proratiser la caisse sur 40.013 / 40.012. Pour la santé et la prévoyance, les accords 3090 et
   1285 déposés le 11/10 parlent du PMSS sans mot sur le temps partiel : **le moteur les proratise aussi, ce qui reste à
   trancher** (voir `prive/journal/restes/PASSATION-R5a.md`).
5. **Le choix est par compagnie, sans date** (Q16) ; la fiche 3369 dit « établissement ».
6. **Pas porté** : le forfait jour réduit (aucune nature n'a `40.011 = 20`) ; L. 242-10, 2° (plafond fractionné entre
   plusieurs employeurs).

Le moteur : `calcul::plafond_temps_partiel` (la règle, la renonciation, l'exclusion), `calcul::plafond_de_l_engagement`,
`dsn::proratisation_du_plafond` (40.084), `changements_contrat` (41.066, datée aussi par le statut de la fiche salarié).

## Les textes, et où ils sont

- **Code de la sécurité sociale**, relevé LEGI du 10/10/2026 par `atelier/outils/relever_les_codes.py` (base LEGI de
  la DILA, 456 livraisons sur 456), `../code-de-la-securite-sociale/articles-20261010.md` et `xml/` :
  - **R. 242-2, I, dernier alinéa** (`LEGIARTI000039222466`, l. 250) : « Pour les salariés mentionnés à l'article
    L. 3123-1 du code du travail autres que ceux mentionnés à l'article L. 242-10, **l'employeur est en droit de
    corriger le plafond**, sans pouvoir augmenter sa valeur mensuelle, à due proportion de la durée de travail inscrite
    à leur contrat de travail au titre de la période où ils sont présents dans l'entreprise, majorée du nombre d'heures
    complémentaires au sens des articles L. 3123-8, L. 3123-20, L. 3123-21 et L. 3123-28 du code du travail effectuées
    au cours de la période mentionnée aux deux premiers alinéas, rapportée à celle correspondant à la plus courte des
    durées mentionnées au 1° de l'article L. 3123-1 du code du travail. »
  - **L. 242-8** (`LEGIARTI000033021383`, section « Le plafond d'un salarie a temps partiel ») : pour les salariés à
    temps partiel, « il est opéré un abattement d'assiette destiné à compenser la différence » avec les cotisations
    d'un temps complet pour une durée de travail identique — la base législative que R. 242-2 met en œuvre.
  - **L. 242-10** (`LEGIARTI000031685873`) : L. 242-8 et L. 242-9 « ne sont pas applicables » 1° aux salariés dont
    l'emploi donne lieu à des taux, assiettes ou montants spécifiques (L. 242-4-4, L. 242-3) ; 2° à ceux dont l'emploi
    régulier et simultané par plusieurs employeurs fractionne le plafond ; ni aux intérimaires, ni aux salariés en
    activité partielle.
- **Code du travail**, même relevé, `../code-du-travail/articles-20261010.md`, section « Le temps partiel, la duree
  legale… » : **L. 3123-1** (le temps partiel : une durée inférieure à la durée légale ou, si elle est inférieure, à
  la durée conventionnelle ou de l'établissement), **L. 3121-27** (35 heures par semaine), L. 3121-28 (au-delà, une
  heure supplémentaire), L. 3123-8, L. 3123-20, L. 3123-21, L. 3123-28 (les heures complémentaires).
- **BOSS**, assiette générale, version du 01/08/2026 (`../boss/boss-assiette-generale-20260801.txt`), **§ 760-770**
  (l. 1031-1072) : le plafond d'un temps partiel « peut être calculé par l'employeur » à due proportion ; « valeur
  mens. du plafond x (durée contractuelle + heures complémentaires) / durée légale du travail » ; « Le salarié à temps
  partiel et l'employeur ont la possibilité de renoncer à cet ajustement du plafond pour cotiser, sans réduction de
  plafond, comme si le salarié était à temps plein » ; exemple « Plafond mensuel x (121,33 h / 151,67 h) x (12 jours /
  31 jours) » ; « Ce rapport ne peut pas conduire à un résultat supérieur à la valeur mensuelle du plafond ».
- **Norme DSN 2027.1.1**, S21.G00.40.084 (p. 216, `../../norme-dsn/texte-2027/bloc-S21.G00.40.txt` l. 2114-2129) :
  « Pour un salarié à temps partiel, cette rubrique doit être renseignée avec **le choix de l'employeur** de proratiser
  le plafond de Sécurité sociale en fonction de la quotité de travail du contrat. » Valeurs 01 Oui, 02 Non ; CCH-12 :
  avec 02, 40.085 est interdite. Usages p. 374 (`tableaux-des-usages.txt` l. 329) : O en mensuelle, I en arrêt,
  reprise et FCTU, C en amorçage. Le libellé de 40.085 (« la proratisation du plafond de Sécurité sociale ne s'applique
  pas au calcul de ses cotisations pour la vieillesse régime de base et la retraite complémentaire ») dit que la
  proratisation porte sur ces deux-là.

## Ce qu'on en lit

1. **C'est une option de l'employeur** (« est en droit de corriger », « peut être calculé par l'employeur », « le
   choix de l'employeur »), pas une règle : le moteur qui ne proratisait pas ne calculait pas faux, il appliquait le
   plafond entier, que l'employeur et le salarié peuvent garder (BOSS § 760).
2. La correction : plafond de la période × durée du contrat / la plus courte de la durée légale (151,67 h) et de la
   durée collective de l'entreprise, sans dépasser le plafond de la période.
3. **Inférence** : le temps partiel est celui que la DSN déclare (40.014 = 20, une durée du contrat inférieure à la
   quotité de référence) ; un contrat d'usage (40.014 = 99) et un temps plein ne sont pas concernés, et 40.084 y vaut
   02, la seule valeur que le calcul ne contredit pas (la norme ne dit rien d'eux).
4. **Inférence** : le moteur n'a qu'un plafond par paie (`pss_periode`), d'où sortent la vieillesse plafonnée, le FNAL,
   la retraite complémentaire (tranches 1 et 2), la prévoyance, la santé au forfait, le chômage et l'AGS (quatre
   plafonds), l'Apec et la CET. La correction s'y applique donc partout. Les textes déposés couvrent la vieillesse
   (R. 242-2) et, par le libellé de 40.085, la retraite complémentaire ; pour le chômage et l'AGS (règlement Unédic),
   la prévoyance et la santé (conventions), le dépôt n'a pas de texte qui dise si leur plafond suit la correction.

## Où le moteur s'en sert

- `atelier/sources/paie/src/plafond.rs`, `plafond_permanent` : le prorata calendaire et la correction.
- `atelier/sources/paie/src/calcul.rs`, `plafond_temps_partiel` (le choix, lu dans la fiche entreprise
  `plafond_temps_partiel` : « proratisé » ou « entier »), `duree_du_temps_partiel` (le rapport ; une ligne d'heures en
  plus du mois refuse, faute de savoir si ce sont des heures complémentaires), `plafond_de_l_engagement` (le plafond
  du bulletin, de toutes les bases et de la DSN).
- `atelier/sources/paie/src/dsn.rs`, `proratisation_du_plafond` : S21.G00.40.084 ; un temps partiel sans choix refuse.
- `atelier/sources/paie/src/changements_contrat.rs` : S21.G00.41.066 quand 40.084 change.
- `logiciel/donnees/champs-base.json`, `plafond_temps_partiel` : la question de la fiche entreprise.
- `atelier/sources/paie/src/periodes.rs` (lot R10) : un temps partiel qui commence ou finit en cours de mois (avenant
  daté avec ses termes d'avant) découpe la paie ; chaque période a le plafond de ses jours, corrigé à sa propre durée
  du travail. BOSS, assiette générale, § 730 (`../boss/boss-assiette-generale-20260801.txt`, l. 1007-1010) : « Le cas
  échéant, les différentes règles d'ajustement du plafond sont cumulables » ; R. 242-2, I : « à due proportion de la
  durée de travail inscrite à leur contrat de travail au titre de la période où ils sont présents ».

## Ce qui n'est pas déposé

- Le règlement général d'assurance chômage déposé (`../chomage-permanents/unedic-reglement-general-convention-20241115.txt`)
  ne dit rien du plafond d'un temps partiel (cherché : « temps partiel » n'y vise que des périodes d'affiliation) ; la
  circulaire Unédic sur l'assiette, l'ANI Agirc-Arrco du 17/11/2017 (tranches d'un temps partiel) et les accords de
  prévoyance et de santé ne sont pas déposés : ce qu'ils disent du plafond d'un temps partiel n'est pas relu (point 4
  ci-dessus).

# Le référentiel des services de prévention et de santé au travail (SPST), relevé du 23/10/2026

Relevé par un agent de recherche (Haiku) de la séance du 23/10/2026 au soir, vérifié par l'orchestrateur.

## Ce qui est déposé ici

- **`urssaf-sst-dpae-20261023.csv`** : le référentiel « sst_dpae » de l'Urssaf, tel que servi (avec BOM, séparateur `;`,
  409 lignes plus l'en-tête). Colonnes : `mt_code_ur;mt_lib_court;mt_lib;mt_adr_part_1;mt_adr_part_2;mt_code_post;mt_bur_dist`.
  - D'où : `https://open.urssaf.fr/explore/dataset/sst_dpae/download/?format=csv&timezone=Europe/Paris&use_labels_for_header=true`.
  - Licence : ODbL, d'après la page data.gouv.
- **`datagouv-spst-liste-20261023.html`** : la page data.gouv du jeu, « Services de prévention et de santé au travail
  (SPST) », Urssaf Caisse nationale. Elle indique une dernière mise à jour au 8 avril 2026 et dit que c'est la liste
  « qui doit être utilisée dans le cadre de la DPAE ».

## Ce qu'on en lit

- **Le code d'un SPST est dans `mt_lib_court`** (`MT161`, `MT171`, …), rattaché à une Urssaf (`mt_code_ur`).
- **La médecine interne** est `MT01`, « MEDECINE INTERNE ETABLISSEMENT ». Elle figure 26 fois, une par Urssaf : le code
  seul ne désigne donc pas une ligne.
- **`MT250`**, saisi par un Sonnet au banc le 23/10, n'est pas dans le référentiel. Un code inventé, donc.

## Pourquoi on l'a relevé : la rubrique DSN S21.G00.11.025

- **La norme 2027 déposée** (`norme-dsn/texte-2027/bloc-S21.G00.11.txt`, l. 232-237) dit : « Identifiant du Service de
  Prévention et de Santé au Travail (SPST) — Cette rubrique doit être renseignée par l'identifiant du service de
  prévention et de santé au travail (SPST). La définition de cet identifiant est précisée dans la fiche consigne
  dédiée. »
- **La note différentielle** 2026.1.2 → 2027.1.2 (`dsn-note-differentielle-ct-2026.1.2-vers-2027.1.2-20251224.txt`,
  l. 3494-3504) dit : la déclaration « doit être réalisée à partir du référentiel SPST de l'Urssaf dans lequel les SPST
  autonomes sont précisés avec la valeur « MT01 - MEDECINE INTERNE ETABLISSEMENT » ». Elle donne aussi le contrôle
  CCH-11 (l. 3562-3570) : un contact de type 17 est obligatoire avec MT01, interdit sinon. Le moteur le fait depuis le
  lot A2.
- **La fiche consigne** est la n° 3370 de net-entreprises, « Service de prévention et de santé au travail (SPST) en
  DSN mensuelle » : `https://net-entreprises.custhelp.com/app/answers/detail_dsn/a_id/3370`. **Elle n'est pas lue** :
  le 23/10/2026, le site répondait « en cours de maintenance ». Elle reste à relever et à déposer ici.
- **Deux sources secondaires la citent.** Ce sont des éditeurs privés, donc les originaux sont dans
  `reference-privee/spst-dsn/` du dépôt privé.
  - **Legisocial**, « Comment déclarer le SPST en DSN mensuelle ? »
    (`https://www.legisocial.fr/actualites-sociales/8098-comment-declarer-spst-dsn-mensuelle.html`, lu le 23/10/2026) :
    « L'identifiant à déclarer doit être recherché dans le référentiel « sst_dpae » publié par l'Urssaf. » L'employeur
    qui relève d'un seul SPST interentreprises y porte son identifiant, et un SPST autonome se déclare « MT01 » avec un
    contact de type 17.
  - **ADP**, « Services de prévention et de santé au travail à déclarer en DSN en 2027 » : « La valeur provient du
    référentiel Urssaf (« MTXX ») ».

## Inférence, écrite comme telle

Le code de la DPAE (« code service de santé au travail », celui de `medecine_travail` dans la fiche) et la valeur de
S21.G00.11.025 viennent du même référentiel. Ce serait donc le même code `MTxxx`. **Aucun texte lu ne le dit en ces
mots.** À confirmer par la fiche 3370 avant que le moteur écrive la rubrique pour un service externe.

## Où le moteur s'en sert

Nulle part encore. La rubrique S21.G00.11.025 n'est écrite que pour MT01, en norme 2027 (`dsn::spst_ecrit`, lot A2).
Deux suites possibles, une fois la fiche 3370 lue :
- écrire la rubrique pour tout SPST ;
- contrôler le code `medecine_travail` contre ce référentiel (un code absent, comme `MT250`, se refuse).

# NEOReS : la norme des comptes rendus des organismes (CRM 118 à 121, 124…)

Relevé le 20/10/2026 par le conteneur, depuis la page « Norme et documentation NEOReS » de net-entreprises
(`https://www.net-entreprises.fr/declaration/norme-et-documentation-neores/`, « Modifié le 14 octobre 2026 »), que cite la
page « Les Comptes Rendus Métiers DSN » (`../gip-mds-api-dsn/net-entreprises-page-comptes-rendus-metiers-dsn-20211130.html`).
Le guide de l'API DSN v6.8 (`../gip-mds-api-dsn/`, « Documents de référence », [9]) cite ces textes sans les joindre à
son archive.

| fichier | ce que c'est | servi à |
|---|---|---|
| `net-entreprises-page-norme-et-documentation-neores-20261014.html` | la page, telle que servie | l'URL ci-dessus |
| `gip-mds-neores-cahier-technique-2026.1.2-20251024.pdf` (+ `.txt`, `pdftotext -layout`) | « Cahier technique NEORES », version 2026.1.2 (norme en production), 55 p., daté du 24/10/2025 | `/media/documentation/neores/dsn-cahier-technique-2026.1.pdf` (Last-Modified 18/11/2025) |
| `gip-mds-neores-cahier-technique-2027.1.0-20260303.pdf` (+ `.txt`) | le cahier 2027.1.0 (norme à venir), daté du 03/03/2026 | `/media/documentation/neores/neores-cahier-technique-2027.1.pdf` (13/03/2026) |
| `gip-mds-neores-note-differentielle-2026.1.2-2027.1.0-20260313.pdf` (+ `.txt`) | la note différentielle CT 2026.1.2 → CT 2027.1.0 | `/media/documentation/neores/neores-differentiel-cahier-technique-2026.1-2027.1.pdf` (13/03/2026) |
| `gip-mds-neores-table-types-de-parametres-20230703.xlsx` (+ `.csv`, LibreOffice, la feuille unique) | « Table des types de paramètres – RO.B008.R001 (décembre 2022) » | `/media/documentation/neores/neores-table-types-de-parametres.xlsx` (03/07/2023) |

Les schémas XSD (`NEOReS_Master_Message_2026.1.1.xsd`, `NEOReS_datatypes_2026.1.1.xsd`, `NEORES-2026-détails.xlsx`…)
que le guide nomme ne sont servis ni par cette page, ni par l'archive du guide (v6.8 déposée, v6.9 servie le 20/10/2026),
ni par le kit de développement `https://github.com/ArchitectureDSN/dsn-api` (cloné le 20/10/2026 : du Java, pas de XSD) ;
les adresses devinées (`/media/documentation/neores/NEOReS_Master_Message_2026.1.1.xsd`, `…/neores-xsd.zip`) rendent 404.
Le « référentiel unique des contrôles » de la page n'est pas déposé : il n'a pas été lu (le compte rendu porte lui-même le
libellé de chaque contrôle, `RO.B004.R002`).

## Ce qu'on en lit

- **§2.2** : un bloc s'écrit `RO.Bnnn`, une rubrique `RO.Bnnn.Rnnn` ; **à la transmission, les points deviennent des
  tirets bas** (`RO_B001_R001`). C'est ce que le lecteur cherche.
- **§4 et « Arborescences » (§5)** : l'envoi (`RO`, sa version `RO.R001`), le flux DSN de rattachement (`RO.B000`), le
  profil (`RO.B001` : 01 retour sur une DSN mensuelle ou un signalement, 02 fin d'exercice, 03 rappel, 04 paramètre utile
  en paie ; 05 et 06 en 2027), le contexte (`RO.B002` : identifiant, code et libellé du retour, 02 = annule et remplace,
  organisme émetteur, période, dates), la validation (`RO.B003` : OK / ANO), les cibles d'action (`RO.B004` : code et
  libellé du traitement, type d'action, niveau, catégorie, résultat du contrôle, nombres d'individus et de contrats,
  contrôle, référence, action attendue, aide, conséquences, caractère substituable, rappel, exception), leurs détails
  (`RO.B005`), les individus (`RO.B006`), les données DSN (`RO.B007` : rubrique, valeur déclarée, valeur corrigée ou
  préconisée, période, résultat ; `RO.B010` : valeurs acceptées), les paramètres (`RO.B008` : type, valeur, effet, fin,
  rubrique DSN, date de calcul) et les écarts d'identité (`RO.B009` : déclaré et référence SNGI). Chaque énumération est
  recopiée dans `paie/src/retours.rs`, à côté de sa rubrique.
- **La forme XML** : le cahier donne les blocs, leur imbrication et leurs rubriques, pas le schéma. Qu'un bloc soit un
  élément qui porte ses rubriques et ses blocs enfants est une **inférence** (celle que le lecteur des effectifs éprouve
  depuis 2022 sur les 118 reçus : `RO_B008` et ses `RO_B008_R00n`) ; le lecteur cherche donc chaque bloc où qu'il soit.
- **CT 2027.1.0 / note différentielle** : déclencheurs 05 (récapitulatif des substitutions) et 06 (informatif) ; le
  caractère substituable et le rappel descendent de `RO.B004` (R016, R017) à `RO.B005` (R005, R006), avec
  `RO.B005.R007` (responsable de l'interlocution) ; deux blocs nouveaux, `RO.B011` (information : code, libellé, numéro
  d'enregistrement de la procédure, information, référence, aide) et `RO.B012` (identifiants de l'individu).
- **La table des types de paramètres** : 118 « Effectifs », A01 effectif moyen annuel, A02 BOETH, A03 ECAP, A04
  assujettissement OETH, A05 CSA CFIP, A07 CSA (niveau entreprise).

## Où le moteur s'en sert

- `paie/src/retours.rs`, `lire_neores` : tout compte rendu NEOReS lu en valeurs (retour, conformité, actions, paramètres,
  identités, informations), avec les tables `DECLENCHEURS_NEORES`, `TYPES_ACTION_NEORES`, `RESULTATS_NEORES`,
  `SUBSTITUABLE_NEORES`, `TYPES_PARAMETRE_NEORES`.
- `paie/src/releve_retours.rs`, `lire_crm118` : les effectifs du 118 sont les paramètres `RO.B008` de ce même lecteur
  (un seul mécanisme) ; `reconstruire_effectifs` en écrit `config/effectifs.json`.
- `paie/src/historique_mois.rs`, `lire_neores` : le compte rendu dit en phrases pour l'Historique (route
  `historique-mois`) ; une cible d'action à corriger se range parmi les anomalies, un paramètre se dit en phrase.
- Tests : `paie/tests/routes/historique_mois.rs`,
  `les_comptes_rendus_neores_et_des_organismes_complementaires_se_traduisent` (un 118 et un 119 fabriqués en fictif).

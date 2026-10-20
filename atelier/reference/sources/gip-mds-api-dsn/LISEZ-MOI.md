# L'API DSN de net-entreprises (GIP-MDS) : ce que la plate-forme rend d'un dépôt

Déposé le 14/10/2026 par le lot O1 (l'ordre des signalements, la plate-forme faisant foi).

## D'où ça vient

Le **brut** est déjà déposé : `atelier/reference/norme-dsn/dsn-guide-implementation-API-v6.8.zip` (relevé le 03/09/2026,
SHA-256 `36e54492b89f2740dfeeddeeb8c8f3b0d4477715ca4577fd2c874672be690ecb`, voir `norme-dsn/LISEZMOI.md`). Ce dossier
en garde les **transcriptions**, parce que le PDF et le DOCX ne se lisent pas tels quels, et, relevés le 20/10/2026, le
brut de ce que l'archive ne contient pas (la fiche 692, deux pages de net-entreprises) :

| fichier | transcription de | comment |
|---|---|---|
| `gip-mds-guide-implementation-api-dsn-v6.8-20260706.txt` | `DSN_Guide-implementation-API_v6-8_2026-07-06.pdf` (dans l'archive) | `pdftotext -layout`, entier |
| `gip-mds-specifications-schema-bilans-vt2.2-20201118.txt` | `DSN-NormalisationBilans-Spécifications du schéma-VT2.2.docx` (dans l'archive) | le texte de `word/document.xml`, lignes vides retirées ; les schémas (images) n'y sont pas |
| `ctip-crm-dsn-oc-p3-v1.1.4-20250617.txt` | `CRM-DSN-OC-P3-V1.1.4-17062025.pdf` (dans l'archive) : la spécification des comptes rendus métier des organismes complémentaires (CTIP, France Assureurs, Mutualité française), onglets Règles, Versions, Données, Schéma, codes retour | `pdftotext -layout`, entier ; le schéma (p. 9) est une image |
| `ctip-crm-dsn-oc-p3-v1.1.3-20250617.txt` | `CRM-DSN-OC-P3-V1.1.3-17062025.pdf` (dans l'archive), la version 1.1.3 | `pdftotext -layout` |
| `ctip-crm-oc-presentation-v1.1.3-v1.1.4-20200130.txt` | `CRM-OC-1.1.3_v2020_CRM-OC-1.1.4_vDef.pdf` (dans l'archive) : ce que 1.1.4 ajoute (criticité, nombre d'erreurs critiques, nature de l'événement) | `pdftotext -layout` |
| `ctip-crm-oc-modifications-v1.1.3-20180801.txt` | `CRM-OC-1.1.3 - 1er août 2018-V0.pdf` (dans l'archive) | `pdftotext -layout` |
| `ctip-dsn-crm-oc-phases-2-3-20160601.txt` | `DSN-CRM-OC-Phase2-3-02-06-2016.pdf` (dans l'archive) : la présentation du modèle (éléments `Fichier`, `Declaration`, `RetourMetier`, `ContexteRetourMetier`, `MandatNetE`, `MandatProp` et leurs attributs) | `pdftotext -layout` ; les attributs de `RetourMetier` et `ContexteRetourMetier` sont dans les diagrammes des p. 11 et 17 (images), lus à l'écran et recopiés ci-dessous |
| `net-entreprises-fiche692-crm-oc-p2-p3-20231116.zip` | **brut** : l'archive que sert la fiche 692 de la base de connaissances (`https://net-entreprises.custhelp.com/app/answers/detail_dsn/a_id/692/`, redirigée vers `https://www.net-entreprises.fr/media/documentation/compte-rendu-metier-p2-p3.zip`, Last-Modified 16/11/2023, SHA-256 `fa888c6db2ad66b0287f02064d57e4a7323c0ac1c434d8b90902ca97d501fb41`), relevée le 20/10/2026 ; la FAQ des OC y renvoie (`../faq-dsn-organismes-complementaires/`, § 12). Deux archives : « CRM V.1.1.4 du 15 septembre 2023 » et « CRM V1.1.3 du 15 septembre 2023 » (chacune : la spécification en `.xlsx`, le schéma `.xsd`, trois exemples XML) | telle que servie |
| `ctip-crm-dsn-oc-p3-v1.1.4-schema-20230915.xsd` | `CRM-DSN-OC-P3-V1.1.4-15092023.xsd`, déplié de l'archive de la fiche 692 | tel quel (ISO-8859-1, CRLF) |
| `net-entreprises-page-comptes-rendus-metiers-dsn-20211130.html` (+ `.txt`, le texte des balises) | la page « Les Comptes Rendus Métiers DSN » (`https://www.net-entreprises.fr/declaration/comptes-rendus-metiers-dsn/`, « Modifié le 30 novembre 2021 »), relevée le 20/10/2026 | telle que servie |
| `net-entreprises-page-api-dsn-20261020.html` | la page « API DSN » (`https://www.net-entreprises.fr/declaration/api-dsn/`), relevée le 20/10/2026 : son seul document est l'archive du guide | telle que servie |

## Ce qu'on en lit

**Le guide (v6.8 du 06/07/2026).**
- §3.3 « Service de consultation des dépôts » (p. 24-25) : `lister-depots` rend, par dépôt, l'identifiant du flux, sa
  date et son statut (OK, KO). Rien de la déclaration.
- §3.4.2 « Recherche par déclarant et plage horaire » (p. 28) et §3.4.4 « Réponse du service » (p. 30) :
  `lister-retours-declarant` rend, par flux, son identifiant, et par retour la publication, la production, l'OPS, la
  **nature**, le **statut** (OK, KO, ANO, EN COURS) et l'adresse du document. Le contenu de la déclaration n'est que
  dans le document.
- §5.2 « Suivi du traitement technique par le SI DSN » (p. 68-69) : nature 10, l'accusé (AEE) ou l'avis de rejet ;
  nature 11, le certificat de conformité ou le bilan d'anomalies (KO : « vous devez analyser les raisons et corriger »).
- §8.2 « Période de rétention des retours » (p. 93) : « Les données de suivi des retours sont disponibles via le service
  de consultation selon les mêmes règles d'historique que le tableau de bord (13 mois). Les retours disponibles au
  téléchargement ont une période de rétention de 3 mois par le SI DSN. Au-delà, les retours sont purgés et ne sont pas
  archivés par le SI DSN. »
- §8.3 « Contraintes de polling » (p. 93-94) : l'intervalle entre deux recherches (en-tête `Expires`) et l'étendue
  d'une plage (en-tête `Accept-Ranges`) ; 429 quand on ne les respecte pas (§9.1.1, p. 95).

**La spécification du schéma des bilans (VT2.2, 18/11/2020).** Une déclaration d'un bilan porte son identification
(rang, nature S20.G00.05.001, type .002, fraction .003, ordre .004, mois .005, id_annulation .006, SIREN, NIC du siège,
NIC d'affectation) et son bilan (état, nombre de salariés et de contrats, conformité, compteurs). Les éléments `contrat`
(depuis `v01r09`) et la liste des salariés de `declaration_bilan` (`v02r01`) existent dans les versions récentes du
schéma ; **les bilans que la plate-forme nous a rendus sont en `v01r08`** et ne les portent pas (un certificat de nature 11 reçu le 24/09/2026 par une compagnie réelle ; pièce hors du dépôt public : dépôt privé). Une anomalie peut porter l'environnement d'un
salarié (NIR, nom…) quand elle s'y rattache.

**Les comptes rendus des organismes complémentaires (nature 51 ; CTIP, France Assureurs, Mutualité française).**
- Spécification V1.1.4 du 17/06/2025 (`ctip-crm-dsn-oc-p3-v1.1.4-20250617.txt`), « Règles » : un CRM par déclaration,
  en XML, schéma commun, **ISO-8859-1** ; CRT (type 2 : niveaux fichier et déclaration, code retour global et
  complément) puis CRC (type 3 : le détail des retours métier) ; un CRM remplace le précédent du même organisme (FAQ des OC,
  § 12).
- « Données », niveau 2 : **code retour global** 0 DSN acceptée, 1 acceptée avec corrections à apporter, 2 partiellement
  prise en compte, 3 non prise en compte ; son complément explicatif ; le producteur (code, raison sociale), le contact
  (nom, courriel, téléphone), les nombres de salariés, d'erreurs, d'erreurs critiques, d'affiliations et de radiations.
  Niveau 3 : les mandats SEPA à créer (Net-Entreprises ou propriétaire : BIC, IBAN, ICS, URL) et les retours métier
  (code « SXXXnn », E erreur ou I information ; libellé imposé ; message complémentaire ; criticité C critique « à
  corriger impérativement dans la DSN suivante », P passable ; rubrique ou bloc à l'origine, sa valeur et son libellé ;
  valeur corrigée ou attendue ; date de l'événement ; nombre d'erreurs similaires). Niveau 4 : le contexte (rubrique,
  libellé, valeur, libellé de la valeur), « a minima les noms, prénoms, NIR et matricule du salarié »
  (`ctip-dsn-crm-oc-phases-2-3-20160601.txt`, p. 17).
- Les noms XML viennent du **schéma** de la fiche 692 (`ctip-crm-dsn-oc-p3-v1.1.4-schema-20230915.xsd`) : `Fichier`
  (`TypCRM`, `CodeEnvoi`, `VersionCRM`…), `Declaration` (`NatureDecla`, `TypeDecla`, `NoOrdreDecla`, `DateMoisDec`,
  `CodeOC`, `CodeProducteurCRM`, `NomProducteurCRM`, `NomContact`, `MailContact`, `TelContact`, `CodeRetourDecla` l. 1001,
  `ComplementRetourDecla`, `NbreErreur`, `NbreCritique`, `NbreAff`, `NbreRad`, `NbreSal`), `MandatNetE` / `MandatProp`
  (`BIC`, `IBAN`, `ICS`, `URLMandatProp`), `RetourMetier` (`CodeRetourMetier`, `LibelCodeRetourMetier`,
  `DescriptionRetour`, `CriticiteRetour` l. 514, `RubriqueBlocRetour`, `LibelRubBlocRetour`, `ValeurRubriqueErreur`,
  `LibelValRubriqueErreur`, `ValeurCorrigeeAjoutee`, `LibelValCorrigeeAjoutee`, `DateEvt`, `NbErreursSimilaires`),
  `ContexteRetourMetier` (`Rubrique`, `Libelle`, `Valeur`, `LibelVal`). Les mêmes noms sont dans les diagrammes de la
  présentation de 2016 (p. 5, 11, 17, 18). Le schéma de la fiche est daté du 15/09/2023 ; la spécification déposée,
  du 17/06/2025, ne change que des valeurs permises (versions de norme, libellés de codes retour), pas les noms.

**Les comptes rendus de l'AGIRC-ARRCO (nature 71).** Le guide (« Documents de référence », [7] ; tableau des retours,
note (*) : « le format harmonisé n'a pu être respecté pour les CRM AGIRC-ARRCO. Un format temporaire est utilisé ») les
dit au format `Schéma XSD RC V07.xsd`, `accuseReceptionDSNDTO.xsd`, `notificationCcmdDSNDTO.xsd` ; §5.8.2 : « CR métier
(CRE) » et « CR métier (NOT) » ; la page des CRM de net-entreprises : « le compte rendu métier de type synthèse ; le CRM
de type notification ». **Ces schémas ne sont publiés nulle part où le conteneur a cherché, le 20/10/2026** : l'archive
du guide (v6.8 déposée ; v6.9 servie ce jour, même liste de documents d'organismes), les pages net-entreprises « API
DSN », « Comptes rendus métiers DSN », « Norme et documentation NEOReS », « Norme et documentation DSN », le tableau de
bord éditeurs, la recherche du site ; la base de connaissances (les fiches 1 à 3500 parcourues : aucune ne redirige vers
un document CRM AGIRC-ARRCO) ; agirc-arrco.fr (« Documentation technique DSN » : des FAQ et cahiers de codification,
pas de format de CRM ; DSN-FIAB) ; le kit `ArchitectureDSN/dsn-api` sur GitHub ; des adresses devinées
(`/media/documentation/RC_V07.xsd`, `…/accuseReceptionDSNDTO.xsd`, `…/crm-agirc-arrco.zip` : 404). Le CTIP
(`ctip.asso.fr`) ne présente pas de certificat valide pour son nom. Ces comptes rendus gardent donc leur statut et leur
pièce brute.

## Où le moteur s'en sert

- `paie/src/retours.rs`, `declarations_du_bilan` : ce qu'un nature 11 identifie d'une déclaration (nature, type, ordre,
  SIREN, NIC, état, essai ou réel, codes d'anomalies) — et rien d'autre, puisque rien d'autre n'y est.
- `paie/src/ordre_signalements.rs` : la relève des trois derniers mois (§8.2) avant d'écrire un signalement, et
  l'ordre suivant du plus grand que la plate-forme (ses bilans) et le journal connaissent.
- `prive/journal/finitions/PASSATION-O1.md`, point 1 : la réponse « ce que la plate-forme donne et ne donne pas ».
- `paie/src/retours.rs`, `lire_rapport` : un rapport au format DSN harmonisé (la spécification VT2.2 : type, message,
  `envoi_etat`, déclarations, anomalies avec code, catégorie, message, rubrique déclenchante, ligne, nombre et salarié,
  informations collectives) lu pour l'Historique.
- `paie/src/historique_mois.rs` (route `historique-mois`, 20/10/2026) : l'organisme et le format de chaque nature de
  retour (guide, §1.3, tableau p. 11-13), ce que dit un statut (§3.4.4, `flux.retour.statut` : OK, KO, ANO, EN COURS).
  Les comptes rendus NEOReS (118 à 121, 124…) se lisent par `retours::lire_neores` (`../neores/`), ceux des organismes
  complémentaires (51) par `retours::lire_crm_oc` (ci-dessus) ; ceux de l'AGIRC-ARRCO (71), dont le format n'est pas
  publié, disent leur statut et ouvrent la pièce.
  Le format de la nature 93, laissé vide par le tableau, est lu comme harmonisé : une inférence (la 95 de la DGFiP a son
  exemple sous « Format DSN harmonisé », §6.3).
- `paie/src/retours.rs`, `lire_crm_oc` (20/10/2026) : un CRM d'organisme complémentaire lu d'après le schéma V1.1.4 et
  sa spécification (tables `RETOURS_GLOBAUX_OC`, `CRITICITES_OC`) ; `paie/src/historique_mois.rs`, `lire_crm_oc` : dit en
  phrases (organisme producteur, code retour global, complément, mandats SEPA à créer, contact) et en anomalies (chaque
  retour métier, son salarié lu dans le contexte). Test : `paie/tests/routes/historique_mois.rs`,
  `les_comptes_rendus_neores_et_des_organismes_complementaires_se_traduisent` (un CRC fabriqué en fictif).

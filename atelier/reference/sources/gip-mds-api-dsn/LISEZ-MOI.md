# L'API DSN de net-entreprises (GIP-MDS) : ce que la plate-forme rend d'un dépôt

Déposé le 14/10/2026 par le lot O1 (l'ordre des signalements, la plate-forme faisant foi).

## D'où ça vient

Le **brut** est déjà déposé : `atelier/reference/norme-dsn/dsn-guide-implementation-API-v6.8.zip` (relevé le 03/09/2026,
SHA-256 `36e54492b89f2740dfeeddeeb8c8f3b0d4477715ca4577fd2c874672be690ecb`, voir `norme-dsn/LISEZMOI.md`). Ce dossier
n'en garde que les **transcriptions**, parce que le PDF et le DOCX ne se lisent pas tels quels :

| fichier | transcription de | comment |
|---|---|---|
| `gip-mds-guide-implementation-api-dsn-v6.8-20260706.txt` | `DSN_Guide-implementation-API_v6-8_2026-07-06.pdf` (dans l'archive) | `pdftotext -layout`, entier |
| `gip-mds-specifications-schema-bilans-vt2.2-20201118.txt` | `DSN-NormalisationBilans-Spécifications du schéma-VT2.2.docx` (dans l'archive) | le texte de `word/document.xml`, lignes vides retirées ; les schémas (images) n'y sont pas |

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

## Où le moteur s'en sert

- `paie/src/retours.rs`, `declarations_du_bilan` : ce qu'un nature 11 identifie d'une déclaration (nature, type, ordre,
  SIREN, NIC, état, essai ou réel, codes d'anomalies) — et rien d'autre, puisque rien d'autre n'y est.
- `paie/src/ordre_signalements.rs` : la relève des trois derniers mois (§8.2) avant d'écrire un signalement, et
  l'ordre suivant du plus grand que la plate-forme (ses bilans) et le journal connaissent.
- `prive/journal/finitions/PASSATION-O1.md`, point 1 : la réponse « ce que la plate-forme donne et ne donne pas ».

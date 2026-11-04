# Les formats d'import des logiciels comptables

Les notices des éditeurs dont `documents/src/etats_compta.rs` (les exports comptables de la paie) écrit le format.
Documents d'organismes privés : ici leur transcription (`pdftotext -layout` de la page imprimée, les liens de navigation de la base Sage retirés : leurs numéros ont la forme d'un NIR), l'original est dans
`reference-privee/sources/formats-comptables/` du dépôt privé. Fournis par Léandre le 04/11/2026, imprimés depuis
les centres d'aide des éditeurs.

| fichier | ce que c'est | où le moteur s'en sert |
|---|---|---|
| `sage-50-structure-ximport-20261104.txt` | Sage 50 (ex-Ciel Compta), « Comprendre la structure du fichier d'import XImport », base de connaissances Sage, modifiée le 13/06/2026 : enregistrement sans délimiteur de 137 caractères au plus, zones et longueurs | export XIMPORT |
| `sage-50-format-import-parametrable-20261104.txt` | Sage 50, « Créer un format d'import paramétrable » (renvoie à la structure XImport) | — (contexte) |
| `sage-100-description-format-pnm-20261104.txt` | Sage 100 Comptabilité, « Description du format PNM » (2021) | export Sage (PNM) |
| `sage-100-ecritures-format-pnm-20261104.txt` | Sage 100, « Les écritures au format Sage (*.pnm) » : contrôles et exemples | export Sage (PNM) |
| `sage-100-format-import-export-parametrable-20261104.txt` | Sage 100, « Configurer le Format import / export paramétrable », modifiée le 01/12/2025 | — (contexte) |
| `cegid-quadra-fichier-entree-ascii-20261104.txt` | Cegid, « Fichier d'entrée ASCII dans QuadraCOMPTA » (`QC_ASC`) : zones, positions, longueurs | export Quadra |
| `cegid-quadra-aide-export-import-ascii-20261104.txt` | QuadraCOMPTA, aide « Outils, Suivi : Export / Import ASCII » | export Quadra |
| `ebp-format-importation-ecritures-ascii-20261104.txt` | EBP Comptabilité, « Format d'importation des écritures en ASCII » (page de Sofakom, revendeur EBP ; la page porte des publicités) | export EBP |
| `memsoft-oxygene-structure-fichiers-ascii-20261104.txt` | Memsoft Oxygène, « Structure des fichiers ASCII utilisés dans le module d'importation de données » — tout format géré « à l'exception des formats de taille fixe », CSV à point-virgule conseillé | export Oxygène |
| `pennylane-importer-des-ecritures-20261104.txt` | Pennylane, centre d'aide, « Importer des écritures » (Excel ou CSV ; champs obligatoires ; code journal de 5 caractères au plus) | export Pennylane |
| `pennylane-importer-cegid-expert-tra-20261104.txt` | Pennylane, « Importer un fichier de CEGID EXPERT au format TRA » | — (contexte) |

Le FEC se fonde sur l'article A47 A-1 du Livre des procédures fiscales et le BOI-CF-IOR-60-40-20 (textes publics, à
déposer à part).

# Loi n° 2025-199 du 28 février 2025 de financement de la sécurité sociale pour 2025

Relevée par Léandre depuis son poste le 06/11/2026 (Légifrance refuse le conteneur), version « en vigueur au 01/03/2025 »,
« dernière mise à jour des données de ce texte : 01 octobre 2026 ». NOR ECOX2421198L, JORF n° 0051 du 28 février 2025.
56 pages.

| fichier | ce que c'est | SHA-256 |
|---|---|---|
| `legifrance-loi-2025-199-20250228.pdf` | le texte, imprimé de Légifrance | `b3688dfd…26337e5e` |
| `legifrance-loi-2025-199-20250228.txt` | sa transcription (`pdftotext -layout`) | `6a406144…1d811093` |

## Ce qu'on en lit

L'**article 18** (en vigueur du 01/03/2025 au 01/01/2026) modifie L. 241-2-1 et L. 241-6-1 : Légifrance n'imprime que la
liste des articles modifiés, non leurs nouvelles valeurs. Le rapport annexé dit l'effet : « l'abaissement des points de
sortie des réductions des cotisations d'assurance maladie et d'allocations familiales respectivement à 2,25 et 3,3 fois
la valeur du SMIC ». Les articles eux-mêmes (réduction de 6 points et de 1,8 point) sont dans `../legi-entrepot/`.

## Où le moteur s'en sert

Les seuils `seuil_smic` 2,25 (CTP 635, maladie) et 3,3 (CTP 430, allocations familiales) des lignes DSN 102 et 907 du
technicien et du permanent, barème 2025 (`logiciel/donnees/baremes/2025.json`).

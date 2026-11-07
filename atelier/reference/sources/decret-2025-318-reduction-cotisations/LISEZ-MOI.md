# Décret n° 2025-318 du 4 avril 2025 relatif aux modalités d'application de divers dispositifs de réduction de cotisations patronales

Relevé par Léandre depuis son poste le 06/11/2026 (Légifrance refuse le conteneur). NOR ECOS2508412D, JORF n° 0083 du
6 avril 2025, texte n° 20, « version initiale » (JORFTEXT000051430146).
ELI : https://www.legifrance.gouv.fr/eli/decret/2025/4/4/ECOS2508412D/jo/texte

| fichier | ce que c'est | SHA-256 |
|---|---|---|
| `legifrance-decret-2025-318-20250406.pdf` | le texte, imprimé de Légifrance | `1a5722a4…03e9e119` |
| `legifrance-decret-2025-318-20250406.txt` | sa transcription (`pdftotext -layout`) | `7590ed4f…abaa675f` |

## Ce qu'on en lit

**Article 1er, 5°, a** : à D. 241-7, I, « les valeurs : « 0,3194 » et : « 0,3234 » sont respectivement remplacées par les
valeurs : « 0,3193 » et : « 0,3233 » » (le T de la réduction générale, FNAL 0,10 % et 0,50 %). **Article 5** : ce 5°, a
s'applique aux rémunérations des périodes d'emploi à compter du **1er mai 2025**. Les autres articles (D. 241-1-2,
D. 241-3-2, D. 241-6, D. 241-24, D. 711-8) ne servent pas le moteur.

## Où le moteur s'en sert

Le `t` daté (0,3194 jusqu'au 30/04/2025, 0,3193 ensuite) de `par_statut.technicien.reduction_generale` du barème 2025
(`logiciel/donnees/baremes/2025.json`), avec D. 241-7 en ses versions du 31/12/2023 et du 07/04/2025
(`../legifrance-versions-datees/`).

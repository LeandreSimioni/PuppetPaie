# La NAF 2025 (INSEE) — structure et table de correspondance depuis la NAF rév. 2

## D'où ça vient

Deux fichiers de l'INSEE, **transmis par Léandre le 11/10/2026** depuis le poste, page insee.fr « NAF 2025 » :
`https://www.insee.fr/fr/information/8181066` (donnée par Léandre le 11/10/2026). Déposés tels que reçus, sans conversion.

| fichier | ce que c'est | date interne | SHA-256 (16) |
|---|---|---|---|
| `insee-naf-2025-structure-20241004.xlsx` | `Structure_NAF_2025_Maj_2024-10-04.xlsx` : la nomenclature NAF 2025 complète (onglet « NAF 2025 » : NACE Rev. 2.1, sous-classe NAF 2025, intitulé ; onglets sections, divisions, groupes, classes, sous-classes) | modifié le 04/10/2024 | `6bbb9b9affc7d547` |
| `insee-naf-2025-correspondances-naf-rev2-vers-naf-2025-20260107.xlsx` | `Correspondances_NAFrev2-NAF2025.xlsx` : pour chaque sous-classe NAF rév. 2 (2008), la ou les sous-classes NAF 2025 et le contenu qui y passe, avec le type (« Unique », « Multiple ») | modifié le 07/01/2026 | `8746ed6b5ec70dca` |

Transcriptions (extraites par `openpyxl`, lignes prises telles quelles) : les divisions **59, 60, 90 et 91** (audiovisuel,
radio-télé, création artistique et spectacle, patrimoine) de chaque fichier, dans les deux `.txt` à côté.

Et le texte qui les porte : `legifrance-decret-2025-736-naf-2025-20250731.mht` (page Légifrance
`https://www.legifrance.gouv.fr/loda/id/JORFTEXT000052019213/`, capturée par Léandre le 11/10/2026, SHA-256
`151a69440762f124`), transcription des visas et des articles 1 à 7 dans le `.txt` à côté. **Décret n° 2025-736 du
31/07/2025** (JORF du 01/08/2025) : art. 1er, la NAF 2025 est approuvée, « adaptation de la NACE Rév. 2.1 » ; **art. 2,
« entre en vigueur le 1er janvier 2027 »** ; art. 5, le code APE attribué par l'INSEE « ne saurait suffire à créer des
droits ou des obligations » ; art. 6, le décret de 2007 (NAF rév. 2) abrogé au 01/01/2027 pour la nomenclature
d'activités.

## Ce qu'on en lit

- **Les codes NAF 2025 du spectacle** (structure, division 90) : `90.11Y` création littéraire et composition musicale,
  `90.12Y` création en arts visuels, `90.13Y` autres activités de création artistique (dont la chorégraphie),
  **`90.20Y` activités de spectacle**, `90.31G` lieux d'arts visuels, `90.31H` gestion de salles de spectacle vivant,
  **`90.39G` production et organisation d'évènements culturels**, `90.39H` soutien technique aux spectacles.
- **`90.01Z` (arts du spectacle vivant) se scinde** (correspondance « Multiple ») : « à l'exception de la production
  de spectacle, tout le contenu » → **`90.20Y`** ; « la production de spectacle » → **`90.39G`**. `90.02Z` (soutien au
  spectacle vivant) se scinde en `90.39G` (production, organisation) et `90.39H` (soutien technique). `90.04Z` (salles)
  → `90.31G` ou `90.31H`.
- Conséquence pour une compagnie : **le code n'est pas déductible mécaniquement** de l'ancien ; c'est l'INSEE qui
  attribue le nouveau code APE (notification à l'entreprise, ou avis de situation SIRENE). La fiche entreprise de la
  compagnie du poste et des quatre démonstrations porte `9001Z` (`config/entreprise.json`, clé `naf`) : **au
  01/01/2027, à remplacer par le code notifié** (`90.20Y` ou `90.39G` selon ce que l'INSEE retient).
- Le format change : lettre finale **Y, G, H**… (plus seulement Z, A, B) ; la table NAF du fichier des nomenclatures
  DSN (`../dsn-nomenclatures/`, table `NAF`, 732 lignes) est à confronter le jour du lot (est-ce déjà la 2025 ?).

## Où le moteur s'en sert

**Lot R5b (11/10/2026)** : `atelier/outils/extraire_naf.py` relève les deux classeurs dans
`logiciel/donnees/naf-base.json` (747 sous-classes NAF 2025, 732 sous-classes NAF rév. 2 et leurs correspondances ;
codes sans point, comme la fiche entreprise les porte ; aucun code commun aux deux nomenclatures, l'outil le vérifie) :

```
python3 atelier/outils/extraire_naf.py prive/reference/sources/insee-naf-2025/insee-naf-2025-structure-20241004.xlsx prive/reference/sources/insee-naf-2025/insee-naf-2025-correspondances-naf-rev2-vers-naf-2025-20260107.xlsx logiciel/donnees/naf-base.json
```

`atelier/sources/paie/src/naf.rs` s'en sert : à la saisie (`champs.rs`, `verifier_champ`), un code APE de l'une ou l'autre
nomenclature passe, un code inconnu des deux refuse (et « 90.20Y » dit de l'écrire sans le point) ; un fichier DSN en
norme P27V01 (`regles.code_naf_en_naf_2025` de `norme-dsn-base.json` ; mensuelle, FCTU, arrêt, amorçage) refuse un code
NAF rév. 2 de la fiche entreprise ou d'un établissement, avec la phrase qui dit vers quels codes NAF 2025 il passe (pour
`9001Z` : 90.20Y ou 90.39G, et ce qui va dans chacun) et que c'est l'INSEE qui attribue le code ; le moteur ne choisit
pas. Un fichier P26V01 ne se contrôle pas (la date de la mise en production de janvier 2027 n'est pas publiée, D1).
Tests : `paie/tests/naf_2025.rs`. Références dans `logiciel/donnees/sources.json` (`insee-naf-2025-structure`,
`insee-naf-rev2-vers-naf-2025`). Reste ouvert : le bulletin, le contrat et la DPAE impriment ou écrivent le code de la
fiche tel quel (aucun texte déposé ne dit quelle nomenclature la DPAE accepte en janvier 2027).

Avant le lot R5b (11/10/2026, séance « documents ») : nulle part. Ce qu'un lot devra regarder : `logiciel/donnees/champs-base.json`
(le champ `naf`, « Code APE (NAF) », son contrôle de forme s'il en a un), `paie/src/bulletin.rs` et `avenant.rs` (le
code imprimé), `amorcage.rs` (`11.002` en P26V01) ; et la page de l'actualité net-entreprises du 08/10/2026
(`../net-entreprises-actualites/`) pour la DSN.

# Unicode : la base de caractères et les pages de code que le moteur lit

Le moteur n'embarque pas de crate de normalisation ni de propriétés Unicode : quatre tables de `atelier/sources/paie/src/`
en tiennent lieu, et `paie generer-tables atelier/reference/sources atelier/sources` (`src/generer_tables.rs`)
les écrit depuis les fichiers de ce dossier (et de `python/`, `w3c/`). Le test `tables_generees` (`tests/fichiers/`)
vérifie que les tables commitées sont celles que ces textes donnent.

## Les fichiers

| fichier | d'où | relevé |
|---|---|---|
| `unicode-ucd-18.0.0-unicodedata-20261015.txt` | https://www.unicode.org/Public/18.0.0/ucd/UnicodeData.txt (HTTP 200, `text/plain; charset=utf-8`, 2 243 593 octets) | 15/10/2026 |
| `unicode-ucd-18.0.0-derivednormalizationprops-20261015.txt` | https://www.unicode.org/Public/18.0.0/ucd/DerivedNormalizationProps.txt (HTTP 200, 1 396 877 octets ; en-tête « DerivedNormalizationProps-18.0.0.txt, Date: 2026-02-03 ») | 15/10/2026 |
| `unicode-ucd-18.0.0-derivednumerictype-20261015.txt` | https://www.unicode.org/Public/18.0.0/ucd/extracted/DerivedNumericType.txt (HTTP 200, 22 001 octets) | 15/10/2026 |
| `unicode-mappings-microsoft-cp437-20261015.txt` | https://www.unicode.org/Public/MAPPINGS/VENDORS/MICSFT/PC/CP437.TXT (HTTP 200, 9 794 octets ; « cp437_DOSLatinUS to Unicode table », table 2.00, 04/24/96) | 15/10/2026 |
| `unicode-mappings-microsoft-cp1252-20261015.txt` | https://www.unicode.org/Public/MAPPINGS/VENDORS/MICSFT/WINDOWS/CP1252.TXT (HTTP 200, 9 355 octets ; « cp1252 to Unicode table », table 2.01, 04/15/98) | 15/10/2026 |
| `unicode-licence-v3-20261015.txt` | https://www.unicode.org/license.txt (la licence des fichiers de données, Unicode License v3) | 15/10/2026 |
| `unicode-norme-18.0.0-chapitre-3-extraits-20261015.md` | **transcription** (extraits) de https://www.unicode.org/versions/Unicode18.0.0/core-spec/chapter-3/ ; l'original va dans `reference-privee/sources/unicode/` du privé | 15/10/2026 |
| `unicode-politique-de-stabilite-extraits-20261015.md` | **transcription** (extraits) de https://www.unicode.org/policies/stability_policy.html ; l'original va dans `reference-privee/sources/unicode/` du privé | 15/10/2026 |

La dernière version publiée le 15/10/2026 est la 18.0.0 (https://www.unicode.org/Public/UCD/latest/ReadMe.txt : « final
data files and charts for version 18.0.0 of the Unicode Character Database »).

**Ce qui se republie et ce qui ne se republie pas** (https://www.unicode.org/copyright.html) : les fichiers de données de
`https://www.unicode.org/Public/` sont sous la licence Unicode v3 (libre, à condition que la notice de copyright les
accompagne : c'est `unicode-licence-v3-20261015.txt`) ; la norme (core spec) et les pages de politique ne se copient que
« for personal or internal business purposes and not for public distribution ». D'où les deux transcriptions
partielles ici, et les originaux au privé.

## Ce qu'on en lit

- **`UnicodeData.txt`** (format : UAX #44), une ligne par point de code, champs séparés par `;` : 2 catégorie générale,
  3 classe de combinaison canonique, 5 correspondance de décomposition (`<étiquette>` devant une décomposition de
  compatibilité, rien devant une canonique). Les plages `<…, First>`/`<…, Last>` (idéogrammes, syllabes Hangul, usage
  privé…) valent pour chaque point de code entre les deux.
- **`DerivedNormalizationProps.txt`** : la propriété `Full_Composition_Exclusion` (les caractères qu'une composition
  canonique ne refait pas : exclusions de `CompositionExclusions.txt`, singletons, décompositions non initiales ;
  chapitre 3, D113).
- **`DerivedNumericType.txt`** : `Numeric_Type` (`Decimal`, `Digit`, `Numeric`), la propriété que la documentation de
  Python donne pour `str.isdigit` et `str.isnumeric` (`python/LISEZ-MOI.md`).
- **Le chapitre 3** (transcription) : D65, la décomposition de compatibilité (« recursively applying both the compatibility
  mappings and the canonical mappings […] until no characters can be further decomposed, and then reordering nonspacing
  marks according to Section 3.11 ») ; D108-D109, l'ordre canonique (on échange deux voisins tant que
  `ccc(A) > ccc(B) > 0`, soit un tri stable par classe entre deux caractères de classe 0) ; D113-D114, le composite
  primaire (décomposable canoniquement et pas `Full_Composition_Exclusion`) ; § 3.12.2, la décomposition des syllabes
  Hangul par le calcul (`SBase = AC00`, `LBase = 1100`, `VBase = 1161`, `TBase = 11A7`, `LCount = 19`, `VCount = 21`,
  `TCount = 28`).
- **Les pages de code** : trois colonnes séparées par des tabulations (octet, point de code, `#nom`) ; un octet sans point
  de code est `#UNDEFINED` (cp1252 : 0x81, 0x8D, 0x8F, 0x90, 0x9D). **Inférence** : la table du moteur écrit U+FFFD pour
  ces octets, parce que `pyexpat` décode avec `replace` et lit U+FFFD comme « non défini » (`python/LISEZ-MOI.md`) ; c'est ce
  que Python donnait (comparé le 15/10/2026 : les 128 octets hauts des deux pages, tels que la table les avait, sont ceux
  de ces fichiers).

## Pourquoi une version plus récente que celle de Python 3.11 (Unicode 14.0.0)

Jusqu'au 15/10/2026, les tables venaient de l'Unicode de l'interpréteur Python (14.0.0 en 3.11, 15.1.0 en 3.13 : les
sorties différaient selon la version, sans qu'aucun en-tête le dise). La **politique de stabilité** (transcription, « Strong
Normalization Stability », applicable depuis Unicode 4.1) dit : pour toute chaîne faite de caractères attribués dans deux
versions V et U, `toNFKD_V(S) = toNFKD_U(S)` (et de même NFC, NFD, NFKC) ; « once a character is assigned, its
decomposition mapping will not change » et « its canonical combining class will not change ». Une version plus récente
n'**ajoute** donc à la normalisation que des caractères nouveaux. Elle le dit aussi pour `Numeric_Type=Digit` (« No new
assignment ») et pour l'égalité `General_Category=Nd` ⇔ `Numeric_Type=Decimal`.

**Ce qu'elle ne dit pas** : la catégorie générale (« Identity Stability » : « properties may still be changed […]
Examples […] General_Category ») et donc `str.isalnum` (une lettre, L*) ou `\d` (Nd) peuvent, en principe, perdre un
caractère d'une version à l'autre. Constaté le 15/10/2026 entre 14.0.0 et 18.0.0 : aucune perte (seulement des ajouts),
voir la passation du lot C.

## Où le moteur s'en sert

| table (`atelier/sources/paie/src/`) | lue par | ce qu'elle porte |
|---|---|---|
| `tables_nfkc.rs` | `nfkc.rs` (`avenant.rs` : le texte d'un PDF se compare après NFKC) | décomposition de compatibilité complète (D65), classes de combinaison, compositions (D114) |
| `nfkd_ascii.rs` | `dpae.rs` (le flux DPAE en ASCII ; `str.isdigit` du SIRET, du téléphone, du code Urssaf) | ce que NFKD laisse d'ASCII, `Numeric_Type` ∈ {Decimal, Digit} |
| `tables_retours.rs` | `retours.rs` (lecture des retours DSN : `\d`, noms d'un `.zip` en cp437, XML en windows-1252) | Nd, cp437, cp1252 (et `python/`, `w3c/`) |
| `tables_journal.rs` | `etats.rs` (`journal._meme_nom`, le rapprochement des noms) | L* ou un `Numeric_Type` |

Monter de version : déposer les nouveaux fichiers (même nommage), changer les trois constantes `UNICODE_*` de
`src/generer_tables.rs`, lancer `paie generer-tables`, et lire le `git diff` des tables (rien ne doit disparaître de la
normalisation).

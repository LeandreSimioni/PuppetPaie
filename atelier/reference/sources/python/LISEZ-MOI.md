# Python : ce que le moteur imite de l'interpréteur (propriétés Unicode des chaînes, encodages de `pyexpat`)

Le moteur garde, à l'octet, des comportements de
l'interpréteur : `str.isdigit`, `str.isalnum`, `\d` des expressions régulières, et ce que `pyexpat` (ElementTree) fait d'un
encodage déclaré dans un XML. Ces textes disent quelle propriété Unicode chaque fonction traduit ; les propriétés
elles-mêmes se lisent dans `unicode/`. `paie generer-tables` (`src/generer_tables.rs`) écrit les tables.

## Les fichiers

| fichier | d'où | relevé |
|---|---|---|
| `python-docs-3.11-stdtypes-20261015.html` | https://docs.python.org/3.11/library/stdtypes.html (HTTP 200, `text/html`, 640 927 octets) | 15/10/2026 |
| `python-docs-3.11-re-20261015.html` | https://docs.python.org/3.11/library/re.html (HTTP 200, 234 784 octets) | 15/10/2026 |
| `python-docs-3.11-codecs-20261015.html` | https://docs.python.org/3.11/library/codecs.html (HTTP 200, 164 943 octets) | 15/10/2026 |
| `python-cpython-3.11.17-encodings-aliases-20261015.txt` | https://raw.githubusercontent.com/python/cpython/v3.11.17/Lib/encodings/aliases.py (HTTP 200, 15 677 octets) | 15/10/2026 |
| `python-cpython-3.11.17-pyexpat-20261015.txt` | https://raw.githubusercontent.com/python/cpython/v3.11.17/Modules/pyexpat.c (HTTP 200, 75 621 octets) | 15/10/2026 |
| `python-releve-codecs-pyexpat-3.11.17-20261015.txt` | **relevé, pas un texte publié** : voir plus bas | 15/10/2026 |

La documentation et les sources de CPython sont sous la licence de la Python Software Foundation (PSF License Version 2),
qui en permet la copie avec la notice de copyright (« Copyright © 2001-2026 Python Software Foundation »). La liste des
modules de `Lib/encodings/` n'a pas pu se relever par l'API de GitHub (HTTP 403 depuis le conteneur, 15/10/2026).

## Ce qu'on en lit : les propriétés

`stdtypes.html` (Python 3.11) :

- **`str.isdigit()`** : « Formally, a digit is a character that has the property value Numeric_Type=Digit or
  Numeric_Type=Decimal. » → `nfkd_ascii.rs`, `CHIFFRES` : `unicode/…-derivednumerictype-…`, `Decimal` ou `Digit`.
- **`str.isdecimal()`** : « Formally a decimal character is a character in the Unicode General Category "Nd". »
- **`\d`** (`re.html`, motifs de texte) : « Matches any Unicode decimal digit (that is, any character in Unicode character
  category [Nd]). » → `tables_retours.rs`, `DECIMAUX` : `unicode/…-unicodedata-…`, catégorie `Nd`.
- **`str.isalnum()`** : « A character c is alphanumeric if one of the following returns True: c.isalpha(), c.isdecimal(),
  c.isdigit(), or c.isnumeric(). » ; **`str.isalpha()`** : « general category property being one of "Lm", "Lt", "Lu", "Ll",
  or "Lo" » ; **`str.isnumeric()`** : « Numeric_Type=Digit, Numeric_Type=Decimal or Numeric_Type=Numeric ». Nd et
  `Numeric_Type=Decimal` sont le même ensemble (politique de stabilité, `unicode/`). → `tables_journal.rs`, `ALNUM` :
  catégorie L*, ou un `Numeric_Type` quelconque.

**Inférence** (non lue dans un texte de Python) : CPython tire ces propriétés de `UnicodeData.txt` et de `Unihan` à la
compilation (`Tools/unicode/makeunicodedata.py`, non déposé). Les valeurs numériques d'Unihan que `DerivedNumericType.txt`
inclut sont toutes des idéogrammes de catégorie `Lo`, donc déjà lettres pour `isalnum` ; l'écart possible ne change rien.
Vérifié le 15/10/2026 : depuis les fichiers d'Unicode 14.0.0, `paie generer-tables` redonne à l'octet (hors en-tête) les
tables que Python 3.11.17 avait produites.

## Ce qu'on en lit : les encodages d'un XML (`tables_retours.rs`, `CODECS_*`)

- **La règle**, `pyexpat.c`, `PyUnknownEncodingHandler` (l. 1375-1422) : pour un encodage qu'expat ne lit pas lui-même,
  pyexpat décode les 256 octets avec le codec de Python (`PyUnicode_Decode(template_buffer, 256, name, "replace")`) ; si
  cela ne rend pas 256 caractères : `ValueError("multi-byte encodings are not supported")` ; sinon la table sert, U+FFFD
  valant « octet non défini » (`info->map[i] = -1`). Un nom que Python ne connaît pas : le décodage échoue
  (`LookupError`).
- **Les noms** : les modules du paquet `encodings` et les clés et valeurs du dictionnaire `aliases` de `aliases.py`, codecs
  de texte seulement (`codecs.html` : « Text Encodings » contre « Binary Transforms »), normalisés comme
  `encodings.normalize_encoding`, en minuscules (`retours::normaliser`).
- **Le relevé** `python-releve-codecs-pyexpat-3.11.17-20261015.txt` : la classe de chaque nom dépend de la table que chaque
  codec de Python donne aux 256 octets, ce qu'aucun texte ne dit en une page (il faudrait les ~120 modules de
  `Lib/encodings/`). Le relevé est donc fait **par l'interpréteur** (Python 3.11.17, le 15/10/2026, la fonction
  `codecs_connus` de l'ancien `atelier/outils/generer_tables_retours.py`, supprimé le même jour) et **figé** : il dit
  exactement ce que le moteur accepte aujourd'hui, et `paie generer-tables` le recopie trié. Cinq classes : `latin1` (la
  table identité), `ascii` (l'ASCII, le reste refusé), `cp1252`, `autres` (une autre table de 256 caractères, que le moteur
  ne porte pas : `ParseError`), `refuses` (pas 256 caractères : `ValueError`).
- **D'où vient chaque nom** (recoupement du 15/10/2026 avec `iana/iana-character-sets-20261015.xml`, noms et alias
  normalisés de la même façon, et avec `aliases.py`) :

  | classe | noms | au registre IANA | dans `aliases.py` seulement | nom de module seulement |
  |---|---|---|---|---|
  | `latin1` | 16 | 8 (`cp819`, `csisolatin1`, `ibm819`, `iso_8859_1`, `iso_8859_1_1987`, `iso_ir_100`, `l1`, `latin1`) | 5 | 3 (`charmap`, `raw_unicode_escape`, `unicode_escape`) |
  | `ascii` | 40 | 14 (dont `us_ascii`, `utf_8`, `iso_2022_jp`, `iso_2022_jp_2`) | 25 | 1 (`utf_8_sig`) |
  | `cp1252` | 3 | 1 (`windows_1252`) | 2 (`1252`, `cp1252`) | 0 |
  | `autres` | 265 | 172 | 81 | 12 (`cp1006`, `cp720`, `cp737`, `cp856`, `cp874`, `cp875`, `koi8_t`, `mac_arabic`, `mac_croatian`, `mac_farsi`, `mac_romanian`, `palmos`) |
  | `refuses` | 98 | 28 | 67 | 3 (`idna`, `punycode`, `undefined`) |

  Le registre IANA fonde donc 223 des 422 noms ; les autres sont des alias ou des modules propres à Python. La liste reste
  celle de Python : un nom qu'un autre lecteur XML accepterait et que Python ne connaissait pas reste refusé, comme avant.
  Les retours des données de test du dépôt déclarent `ISO-8859-1` (ou rien : UTF-8), que `retours::codage` reconnaît
  avant la table ; ceux du poste n'ont pas été relus pour ce relevé.

## Où le moteur s'en sert

- `atelier/sources/paie/src/nfkd_ascii.rs` (`CHIFFRES`), lu par `dpae.rs` ;
- `atelier/sources/paie/src/tables_retours.rs` (`DECIMAUX`, `CODECS_*`), lu par `retours.rs` (`est_chiffre`, `codage`) ;
- `atelier/sources/paie/src/tables_journal.rs` (`ALNUM`), lu par `etats.rs`.

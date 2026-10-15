# IANA : le registre des jeux de caractères

## Le fichier

| fichier | d'où | relevé |
|---|---|---|
| `iana-character-sets-20261015.xml` | https://www.iana.org/assignments/character-sets/character-sets.xml (HTTP 200, `application/xml`, 111 307 octets) : *Character Sets*, le registre des noms de jeux de caractères (RFC 2978) | 15/10/2026 |

## Ce qu'on en lit

Chaque `<record>` donne un nom (`<name>`), un nom préféré pour MIME (`<preferred_alias>`) et des alias (`<alias>`) :
`ISO_8859-1:1987` (préféré : `ISO-8859-1` ; alias `iso-ir-100`, `ISO_8859-1`, `latin1`, `l1`, `IBM819`, `CP819`, `csISOLatin1`), `US-ASCII`,
`windows-1252` (alias `cswindows1252`)…

## Où le moteur s'en sert

Nulle part directement : le registre sert à dire d'où viennent les noms d'encodage que `retours.rs` connaît
(`tables_retours.rs`, `CODECS_*`), qui sont ceux de Python. Le recoupement (223 noms sur 422 sont au registre) est dans
`python/LISEZ-MOI.md`.

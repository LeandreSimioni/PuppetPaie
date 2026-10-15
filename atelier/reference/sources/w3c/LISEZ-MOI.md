# W3C : les caractères d'un nom XML

## Les fichiers

| fichier | d'où | relevé |
|---|---|---|
| `w3c-xml-1.0-quatrieme-edition-20060816.html` | https://www.w3.org/TR/2006/REC-xml-20060816/ (HTTP 200, `text/html; charset=utf-8`, 214 369 octets) : *Extensible Markup Language (XML) 1.0 (Fourth Edition)*, W3C Recommendation du 16 août 2006 | 15/10/2026 |
| `w3c-xml-1.0-cinquieme-edition-20081126.html` | https://www.w3.org/TR/xml/ (HTTP 200, 219 926 octets) : la cinquième édition, W3C Recommendation du 26 novembre 2008 | 15/10/2026 |

Les recommandations du W3C se copient et se diffusent avec leur notice (W3C Document License, rappelée dans chaque page).

## Ce qu'on en lit

- **Quatrième édition, annexe B « Character Classes »**, productions [84] `Letter ::= BaseChar | Ideographic`, [85]
  `BaseChar`, [86] `Ideographic`, [87] `CombiningChar`, [88] `Digit`, [89] `Extender` (des listes `[#xXXXX-#xYYYY] |
  #xZZZZ`) ; et § 2.3, [4] `NameChar ::= Letter | Digit | '.' | '-' | '_' | ':' | CombiningChar | Extender`, [5]
  `Name ::= (Letter | '_' | ':') (NameChar)*`. C'est la règle d'**expat**, donc de `xml.etree.ElementTree` que le Python
  utilisait pour lire les retours : vérifié le 15/10/2026, les tables que l'ancien script relevait en faisant lire `<X/>` et
  `<aX/>` à ElementTree, un point de code à la fois (Python 3.11.17), sont exactement ces productions, à une différence
  près : `NOM_SUITE` comptait aussi les quatre blancs de XML (U+0009, U+000A, U+000D, U+0020), parce que `<a />` est bien
  formé (le blanc avant `/>`, production [44]) : un artefact de la mesure, pas un caractère de nom. `paie generer-tables`
  ne les reprend pas ; le moteur n'en voit pas la différence, `retours.rs` coupant un nom au premier blanc (`blanc_xml`)
  avant de le vérifier (`nom_valide`).
- **Cinquième édition**, § 2.3, [4] `NameStartChar` et [4a] `NameChar` : des plages bien plus larges (tout l'Unicode hors
  quelques blocs). C'est ce qu'accepte `roxmltree` ; c'est **pourquoi** `retours.rs` a une couche `controle_expat`, qui
  refuse ce que roxmltree laisse passer et qu'expat refusait. La cinquième édition ne nourrit aucune table.

## Où le moteur s'en sert

`atelier/sources/paie/src/tables_retours.rs`, `NOM_DEBUT` (`Letter` ou `_`) et `NOM_SUITE` (`NameChar`), le deux-points à
part : écrites par `paie generer-tables` (`src/generer_tables.rs`, `production`) depuis la quatrième édition ; lues par
`retours::nom_valide` (`controle_expat`).

# Légifrance — des versions datées d'articles

Chaque article est déposé **tel que Légifrance l'imprime** (`.pdf`, « Imprimer » de la page de l'article, dans la version
datée qu'il faut), et sa transcription à côté (`.txt`, `pdftotext -layout`). Le nom porte la date de **début** de la
version. Relevés le 04/11/2026 par Léandre sur son poste : Légifrance refuse le conteneur (HTTP 403), et `moteur
releve-codes` ne garde que la version en vigueur d'un article.

| fichier | version | ce qu'on en lit | où ça sert |
|---|---|---|---|
| `legifrance-css-D242-4-20231231` | CSS D. 242-4, du 31/12/2023 au 01/01/2026 | « A compter du 1er janvier 2024 8,55 % 6,90 % » : l'assurance vieillesse plafonnée, part patronale et salariale ; la déplafonnée à côté | la vieillesse du barème 2025 |
| `legifrance-css-L136-8-20201216` | CSS L. 136-8, du 16/12/2020 au 01/07/2025 | « 1° A 9,2 % pour la contribution sociale » sur les revenus d'activité | la CSG du barème 2025 (premier semestre) |
| `legifrance-css-L136-8-20250701` | CSS L. 136-8, du 01/07/2025 au 31/12/2025 | « 1° A 9,2 % pour la contribution sociale » sur les revenus d'activité | la CSG du barème 2025 (second semestre) |
| `legifrance-css-L136-8-20251231` | CSS L. 136-8, du 31/12/2025 au 27/06/2026 | le même taux de 9,2 % | la CSG du barème 2026, jusqu'au 27/06/2026 |
| `legifrance-cgi-154-quinquies-20230311` | CGI 154 quinquies, du 11/03/2023 au 27/06/2026 | la CSG déductible « à hauteur de 6,8 points » | la part déductible de la CSG en 2025 et 2026 |
| `legifrance-cgi-293-B-20250101` | CGI 293 B, du 01/01/2025 au 01/03/2025 | la franchise en base de TVA (85 000 / 37 500) | la note du barème 2025 qui la cite |
| `legifrance-cgi-204-H-20260221` | CGI 204 H, du 21/02/2026 au 01/07/2026 | les grilles du taux par défaut du prélèvement à la source | `pas_taux_par_defaut` du barème 2026, de mai à juin |
| `legifrance-ct-L2262-1-20080501` | C. trav. L. 2262-1, depuis le 01/05/2008 | un accord lie les adhérents des organisations signataires | la portée d'un accord de salaires (barème 2026) |
| `legifrance-decret-2015-87-20150201` | décret n° 2015-87 du 28/01/2015 | le financement mutualisé des organisations syndicales et patronales ; il ne fixe pas lui-même le taux : le 0,016 % est à C. trav. D. 2135-34, déjà dans l'entrepôt LEGI (`legi-entrepot/articles.jsonl.gz`) | aucun calcul ; lu pour la contribution au dialogue social |


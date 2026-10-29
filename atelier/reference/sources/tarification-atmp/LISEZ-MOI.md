# La tarification des accidents du travail (AT/MP) : les deux arrêtés (relevés le 29/10/2026)

Les deux arrêtés ont été relevés par Léandre sur Légifrance depuis son poste (Légifrance refuse le conteneur), et
imprimés en PDF tels que le site les sert ; leur transcription est faite par `pdftotext -layout`. Les articles du code de
la sécurité sociale qu'ils appliquent (D. 242-6-2, D. 242-6-14) sont relevés depuis LEGI dans
`../code-de-la-securite-sociale/xml/`.

| fichier | ce que c'est | URL | SHA-256 |
|---|---|---|---|
| `legifrance-arrete-tarification-atmp-2026-20261029.pdf` | Arrêté du 30 décembre 2025 relatif à la tarification des risques AT/MP pour l'année 2026 (NOR SFHS2536273A, JORF n°0306 du 31/12/2025), le texte seul (annexes repliées) | https://www.legifrance.gouv.fr/jorf/id/JORFTEXT000053213574 (le texte consolidé « en vigueur au 29/10/2026 ») | `5b2cf385…5dec00` |
| `legifrance-arrete-tarification-atmp-2026-20261029.txt` | Sa transcription | — | `e19bc6ff…f55caa` |
| `legifrance-arrete-tarification-atmp-2026-annexes-20261029.pdf` | Le même arrêté, annexes dépliées (annexe 1 : taux nets collectifs 2026) | idem | `85bc22de…68744001c` |
| `legifrance-arrete-tarification-atmp-2026-annexes-20261029.txt` | Sa transcription | — | `de608606…5630fb93` |
| `legifrance-arrete-tarification-atmp-1995-20261029.pdf` | Arrêté du 17 octobre 1995 relatif à la tarification des risques AT/MP (NOR SANS9502262A), version consolidée en vigueur au 29/10/2026 : règles de classement et nomenclature | https://www.legifrance.gouv.fr/loda/id/JORFTEXT000000555723 | `21a84c1e…ea20499` |
| `legifrance-arrete-tarification-atmp-1995-20261029.txt` | Sa transcription | — | `8820081a…c9c57e33` |

L'URL exacte de chaque page n'a pas été notée au relevé ; celles du tableau sont celles des textes par leur NOR, à
vérifier au prochain relevé.

## Ce qu'on en lit

- **Arrêté du 30/12/2025, article 1** (transcription du texte, l. 30-36) : « Les taux nets collectifs visés aux articles
  D. 242-6-11 et D. 242-6-18 à D. 242-6-23 du code de la sécurité sociale sont fixés par l'annexe 1 ». **Article 5**
  (l. 68-75) : les activités visées au I de l'article D. 242-6-14 sont celles de l'annexe 1 « pour lesquelles le taux net
  est suivi des lettres TC ». **Article 6** : il s'applique aux cotisations dues au titre de 2026.
- **Annexe 1** (transcription des annexes), les risques qu'une structure du spectacle peut se voir notifier :
  - l. 506-509 : **92.3AD**, « Création et interprétation littéraires et artistiques (y compris les artistes). (3) Services
    annexes des spectacles (tout intermittent du spectacle). », **1,50 %, TC** ; la note (3) (l. 549) : « Y compris la
    décoration sur émail, sur faïence et sur porcelaine (non annexée à une fabrique) » ;
  - l. 500-504 : **92.1CC**, « Production de films et de programmes pour le cinéma, la télévision, la radiodiffusion, etc.
    … Enregistrement sonore et édition musicale. Distribution et projection de films. Activités photographiques (hors
    agences de presse). Gestion d'activités de spectacles et gestion d'activités culturelles et socio-éducatives. »,
    **1,07 %**, sans TC ;
  - l. 497 : **91.3EA**, « Associations culturelles et socio-éducatives ne gérant pas d'équipements. », **1,20 %, TC** ;
  - l. 534-537 : **92.7CC**, « Attractions foraines avec et sans montage de manèges ou de chapiteaux (5), et autres
    spectacles et services récréatifs (6). », **1,78 %**, sans TC ; (5) (l. 556-557) : « les cirques ambulants, les
    ménageries et toutes attractions mues par la force motrice, mécanique ou électrique » ; (6) (l. 560-561) : « bal,
    dancing, patinage, curiosités naturelles, spectacles son et lumière, parcs zoologiques, parc d'attractions et de
    loisirs ».
- **D. 242-6-14, I** (`../code-de-la-securite-sociale/xml/D242-6-14.xml`) : « Les établissements exerçant une activité dont la
  liste est fixée par arrêté du ministre chargé de la sécurité sociale conservent un taux de cotisation collectif quel que
  soit leur effectif de salariés ». Avec l'article 5 de l'arrêté : **le taux d'un risque « TC » est le taux collectif de
  l'annexe 1, quel que soit l'effectif** (1,50 % en 2026 pour 92.3AD).
- **D. 242-6-2** (`../code-de-la-securite-sociale/xml/D242-6-2.xml`) : tarification collective sous 20 salariés, mixte de
  20 à 149, individuelle à 150 et plus (effectif global de l'entreprise, R. 130-1). Un risque sans « TC » n'a le taux
  collectif de l'annexe que sous 20 salariés ; au-delà, son taux est celui que la CARSAT notifie.
- **Arrêté du 17/10/1995, article 1, I, 1°** (transcription, l. 31-38) : le classement d'un établissement « est effectué
  en fonction de l'activité exercée » ; plusieurs activités : « son activité principale, qui est celle exercée par le plus
  grand nombre de salariés » ; à égalité, celle qui « engendre le risque le plus important ». **III** (l. 73-79) : le taux
  « fonctions support » se demande, mais « ces dispositions ne s'appliquent pas aux établissements dont le taux est fixé en
  application de l'article D. 242-6-14 » : pas de taux bureau (« B ») en 92.3AD ni en 91.3EA.
- **La norme DSN** (`../../norme-dsn/texte/bloc-S21.G00.40.txt`, l. 1224-1252 et 1303-1315) : le code risque
  (S21.G00.40.040) se lit sur la notification de la CARSAT, « B » compris s'il y est ; l'employeur ne déclare qu'un code
  qui lui a été notifié ; sans notification, **999ZZ**, et alors le taux (S21.G00.40.043) est interdit (SIG-11).
  **Aucun texte déposé ne dit ce que le bulletin cotise sous 999ZZ** : le moteur ne calcule pas de paie sans taux et le dit.

Non relevé : l'annexe 3 (Alsace-Moselle), dont les taux attendent leur relevé (ticket #307) ; l'arrêté de l'année 2025
(le barème 2025 n'a donc pas de table des risques, et son taux se saisit).

## Où le moteur s'en sert

- `logiciel/donnees/baremes/2026.json`, `risques_atmp` : les cinq codes (92.3AD, 92.1CC, 91.3EA, 92.7CC, 999ZZ), leur
  libellé tel que l'annexe 1 l'écrit, leur taux et leur « TC » ; `_source` cite ce dossier.
- `droit::bareme::risques_atmp`, `droit::bareme::taux_de_l_arrete` : la lecture de cette table ; le taux d'un risque TC,
  ou d'un risque sans TC sous 20 salariés.
- `moteur::interface::compagnies::ajouter_venant` (par `moteur::routes::fiches::poser_atmp_de_la_creation`) : à la
  création d'une compagnie, 92.3AD et le taux de l'arrêté de l'année se posent d'eux-mêmes dans
  `config/baremes/<année>.json` ; la route `taux-atmp` les dit à l'écran (Configuration › Entreprise), avec leur libellé,
  l'explication, où le lire, et l'avancé qui les change.
- `moteur::import::dsn_aem` : le code risque et le taux qu'une DSN importée déclare (S21.G00.40.040, .043).

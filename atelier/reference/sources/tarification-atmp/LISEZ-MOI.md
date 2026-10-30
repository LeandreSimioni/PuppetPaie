# La tarification des accidents du travail (AT/MP) : les arrêtés (relevés le 29/10/2026)

Les arrêtés ont été relevés par Léandre sur Légifrance depuis son poste (Légifrance refuse le conteneur), imprimés en PDF
tels que le site les sert, annexes dépliées ; leur transcription est faite par `pdftotext -layout`. Les articles du code de
la sécurité sociale qu'ils appliquent (D. 242-6-2, D. 242-6-14) sont relevés depuis LEGI dans
`../code-de-la-securite-sociale/xml/`.

| fichier | ce que c'est | URL | SHA-256 |
|---|---|---|---|
| `legifrance-arrete-tarification-atmp-2026-complet-20261029.pdf` | Arrêté du 30 décembre 2025 relatif à la tarification des risques AT/MP pour l'année 2026 (NOR SFHS2536273A, JORF n°0306 du 31/12/2025), version initiale, ses trois annexes dépliées | https://www.legifrance.gouv.fr/eli/arrete/2025/12/30/SFHS2536273A/jo/texte | `a22476fa…7c03fe` |
| `legifrance-arrete-tarification-atmp-2026-complet-20261029.txt` | Sa transcription | — | `9ad8ea91…19ee9ba` |
| `legifrance-arrete-tarification-atmp-2025-complet-20261029.pdf` | Arrêté du 29 avril 2025 relatif à la tarification des risques AT/MP pour l'année 2025 (NOR TSSS2510229A, JORF n°0102 du 30/04/2025), version initiale, ses trois annexes dépliées | https://www.legifrance.gouv.fr/eli/arrete/2025/4/29/TSSS2510229A/jo/texte | `89f20200…c09f8d` |
| `legifrance-arrete-tarification-atmp-2025-complet-20261029.txt` | Sa transcription | — | `0937b0f1…6e73a1` |
| `legifrance-arrete-tarification-atmp-1995-20261029.pdf` | Arrêté du 17 octobre 1995 relatif à la tarification des risques AT/MP (NOR SANS9502262A), version consolidée en vigueur au 29/10/2026 : règles de classement et nomenclature | https://www.legifrance.gouv.fr/loda/id/JORFTEXT000000555723 (par son NOR, à vérifier au prochain relevé) | `21a84c1e…ea20499` |
| `legifrance-arrete-tarification-atmp-1995-20261029.txt` | Sa transcription | — | `8820081a…c9c57e33` |
| `legifrance-decret-2026-951-atmp-niveau-entreprise-20261030.pdf` | Décret n° 2026-951 du 13 octobre 2026 généralisant le calcul des taux de cotisation AT/MP au niveau de l'entreprise (NOR TRSS2625216D, JORF n°0240 du 14/10/2026), version initiale, relevé par Léandre le 30/10/2026 | https://www.legifrance.gouv.fr/eli/decret/2026/10/13/TRSS2625216D/jo/texte | `5faecc11…0d445cde8` |
| `legifrance-decret-2026-951-atmp-niveau-entreprise-20261030.md` | Sa transcription, à la main (le PDF n'a pas de couche de texte lisible) | — | `7bdae1eb…bd88cca0` |

## Ce qu'on en lit

- **Arrêté du 30/12/2025** (transcription 2026) : **article 1** (l. 32-37) : « Les taux nets collectifs visés aux articles
  D. 242-6-11 et D. 242-6-18 à D. 242-6-23 du code de la sécurité sociale sont fixés par l'annexe 1 » ; **article 4**
  (l. 54-60) : ceux de l'article D. 242-35, « applicables dans les départements du Haut-Rhin, du Bas-Rhin et de la
  Moselle », par l'annexe 3 ; **article 5** (l. 63-70) : les activités visées au I de l'article D. 242-6-14 sont celles de
  l'annexe 1 « pour lesquelles le taux net est suivi des lettres TC » ; **article 6** : il s'applique aux cotisations de 2026.
  L'arrêté du 29/04/2025 dit la même chose pour 2025 (transcription 2025, articles 1 à 6).
- **Annexe 1**, les risques qu'une structure du spectacle peut se voir notifier (2026, l. 486-551 ; 2025, l. 474-525) :
  - **92.3AD**, « Création et interprétation littéraires et artistiques (y compris les artistes). (3) Services annexes des
    spectacles (tout intermittent du spectacle). », **TC**, **1,50 %** en 2026, **1,65 %** en 2025 ; note (3) : « Y compris la
    décoration sur émail, sur faïence et sur porcelaine (non annexée à une fabrique) » ;
  - **92.1CC**, « Production de films et de programmes pour le cinéma, la télévision, la radiodiffusion, etc. …
    Enregistrement sonore et édition musicale. Distribution et projection de films. Activités photographiques (hors
    agences de presse). Gestion d'activités de spectacles et gestion d'activités culturelles et socio-éducatives. », sans TC,
    **1,07 %** (2026), **1,04 %** (2025) ;
  - **91.3EA**, « Associations culturelles et socio-éducatives ne gérant pas d'équipements. », **TC**, **1,20 %** (2026),
    **1,22 %** (2025) ;
  - **92.7CC**, « Attractions foraines avec et sans montage de manèges ou de chapiteaux (5), et autres spectacles et services
    récréatifs (6). », sans TC, **1,78 %** les deux années ; (5) : « les cirques ambulants, les ménageries et toutes
    attractions mues par la force motrice, mécanique ou électrique » ; (6) : « bal, dancing, patinage, curiosités
    naturelles, spectacles son et lumière, parcs zoologiques, parc d'attractions et de loisirs ».
- **Annexe 3** (Alsace-Moselle), par groupes (2026, l. 1495-1600 ; 2025, l. 1379-1484) : 91.3EA et 92.1CC au **groupe 1**
  (1,23 % en 2026, 1,00 % en 2025) ; 92.3AD et 92.7CC au **groupe 3** (3,34 % en 2026, 3,42 % en 2025). L'annexe 3 ne porte
  pas de lettres TC. L'article D. 242-35, auquel l'article 4 renvoie, **n'est pas relevé** : ce qu'il dit de l'effectif en
  Alsace-Moselle n'est pas établi ; le moteur ne pose donc pas ces taux de lui-même (il les dit, et le taux se saisit).
- **D. 242-6-14, I** (`../code-de-la-securite-sociale/xml/D242-6-14.xml`) : « Les établissements exerçant une activité dont
  la liste est fixée par arrêté du ministre chargé de la sécurité sociale conservent un taux de cotisation collectif quel
  que soit leur effectif de salariés ». Avec l'article 5 : **le taux d'un risque « TC » est le taux collectif de l'annexe 1,
  quel que soit l'effectif**.
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

## Où le moteur s'en sert

- `logiciel/donnees/baremes/2025.json` et `2026.json`, `risques_atmp` : les cinq codes (92.3AD, 92.1CC, 91.3EA, 92.7CC,
  999ZZ), leur libellé tel que l'annexe 1 l'écrit, leur taux, leur « TC » et leur groupe de l'annexe 3 ; `_source` cite ce
  dossier.
- `droit::bareme::risques_atmp`, `droit::bareme::taux_de_l_arrete` : la lecture de cette table ; le taux d'un risque TC,
  ou d'un risque sans TC sous 20 salariés ; rien en Alsace-Moselle.
- `moteur::routes::atmp` : à la création d'une compagnie (`poser_a_la_creation`, appelé par
  `moteur::interface::compagnies::ajouter_venant`), 92.3AD et le taux de l'arrêté de l'année se posent d'eux-mêmes dans
  `config/baremes/<année>.json` ; la route `taux-atmp` les dit à l'écran (Configuration › Entreprise), avec leur libellé,
  l'explication, où le lire, et l'avancé qui les change ; `declarations::demarrage::poses` les dit dans « Pour démarrer ».
- `droit::bareme::taux_atmp_reporte`, appelé par `droit::contexte::Contexte::bareme_brut` : une année que la compagnie n'a
  pas dite reprend le code risque de la dernière année dite ; s'il est « TC » et que le taux d'avant était déjà le taux
  collectif, le taux de l'année est celui de l'annexe 1 de l'arrêté de l'année (D. 242-6-14, I). Sinon (un taux notifié
  propre, un risque sans TC, 999ZZ, l'Alsace-Moselle, une année dont l'arrêté n'est pas déposé), le taux notifié de
  l'année se demande, en disant pourquoi.
- `moteur::import::parametrage` : le code risque et le taux qu'une DSN importée déclare (S21.G00.40.040, .043).
- **Le décret n° 2026-951 du 13/10/2026** (transcription `legifrance-decret-2026-951-atmp-niveau-entreprise-20261030.md`) :
  pour les cotisations dues **à compter de 2027**, une entreprise en tarification individuelle ou mixte a un taux
  déterminé au niveau de l'entreprise « pour chaque établissement ou groupe d'établissements appartenant à la même
  catégorie de risque » (art. 1, D. 242-6-1 ; art. 5) ; un établissement nouvellement créé dans une catégorie de risque
  que l'entreprise n'a pas encore, et toute entreprise en tarification collective, ont les **taux nets collectifs**
  l'année de la création et les deux années civiles suivantes (art. 4, D. 242-6-17). **Le moteur ne s'en sert pas
  encore** : le taux par établissement est au ticket #311 ; D. 242-6-17 dit le taux d'un établissement nouveau, pas ce
  que le bulletin cotise sous 999ZZ avant la notification (ticket #307).

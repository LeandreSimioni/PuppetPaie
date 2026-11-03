# Les indemnités de mission à l'étranger et les taux de chancellerie (DGFiP)

Deux jeux de données de la DGFiP, publiés sur data.economie.gouv.fr, exportés en CSV par Léandre le 03/11/2026 (le site
refuse le conteneur du nuage) :

| fichier | jeu de données | ce qu'il dit |
|---|---|---|
| `dgfip-frais-de-mission-etranger-20261103.csv` | <https://data.economie.gouv.fr/explore/dataset/dgfip-frais-de-mission-etranger/> | l'indemnité journalière de mission par pays (`code_pays`, `nom_pays`), en monnaie locale (`code_monnaie`), et sa date d'effet (`date`) ; 961 lignes |
| `dgfip-taux-de-change-20261103.csv` | <https://data.economie.gouv.fr/explore/dataset/dgfip-taux-de-change/> (INFÉRENCE : l'identifiant du jeu suit le nom du fichier exporté ; à confirmer à son premier relevé) | le taux de chancellerie d'une monnaie (`monnaie_source`) pour un pays (`code_pays`), en euros pour une unité (`taux`), daté (`date`, le 1er ou le 16 du mois, quand il change) ; 24 168 lignes |
| `dgfip-webpays-20261103.txt`, `dgfip-webmonn-20261103.txt` | (les mêmes, tables annexes) | les pays et les monnaies, avec leurs remplacements |

**Ce qu'on en lit.**
- Le montant en vigueur est `groupe_1` (les groupes 2 à 5 sont à zéro depuis la réforme de 2006 : un seul taux par pays).
  Il vaut jusqu'à la ligne suivante du même code : une période saisonnière est une suite de lignes datées (New York :
  `2026-01-01` 360 USD puis `2026-09-01` 490 USD ; l'Afrique du Sud : `2025-12-15` 185 € puis `2026-03-02` 138 €). La
  table ne porte une saison qu'une fois publiée : d'où le relevé avant chaque publication.
- Une ville que l'annexe de l'arrêté du 3 juillet 2006 distingue a son propre code : `NY` New York, `VT` Toronto, `VV`
  Vancouver, `TY` Tokyo, `VL` Lomé, `NV` Abuja, Lagos et Port-Harcourt (`logiciel/donnees/missions/villes.json`). `LG`
  (Luxembourg-ville, 2006) n'a plus de ligne à l'annexe : un code qu'aucune liste ne nomme n'est pas proposé.
- Les valeurs sont celles de l'annexe en vigueur depuis le 27/11/2025 (`../arrete-2006-indemnites-de-mission/`) :
  Allemagne 240 €, Espagne 212 €, États-Unis 360 USD, Sénégal 91 800 XOF, Belgique 206 €.
- INFÉRENCE : la conversion en euros se fait au taux de chancellerie en vigueur le jour de la mission (la dernière ligne
  de la monnaie pour ce pays dont la date ne le passe pas). Aucun texte déposé ne dit quel taux ; la page Urssaf renvoie
  au site des taux de chancellerie (`../urssaf-frais-professionnels/urssaf-frais-professionnels-20261030.txt` l. 913-914).

**Où le moteur s'en sert.** Les deux CSV sont livrés tels quels dans `logiciel/donnees/missions/` (`indemnites.csv`,
`change.csv`) ; `droit::missions` les lit, `engagement::frais_au_dela` en tire la limite d'un jour à l'étranger (le choix
« Lieu » d'une ligne de nuitée ou de repas). `moteur releve-missions` (`atelier/sources/moteur/src/atelier/releve/missions.rs`)
les relève par l'API d'opendatasoft avant chaque publication, et les dépose ici, datés.

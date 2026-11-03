# legisocial — « Allocations forfaitaires - Frais professionnels 2025 »

Relevée par Léandre depuis son poste le 03/11/2026 : la page de legisocial.fr (un éditeur privé de documentation
sociale), « Dernière mise à jour le 24 décembre 2024 », qui reprend les montants du BOSS (« Valeurs confirmées par le
BOSS, mise à jour du 24 décembre 2024 », l. 69 et 104). URL :
https://www.legisocial.fr/reperes-sociaux/allocations-forfaitaires-frais-professionnels-2025.html.
Un document d'éditeur privé ne se republie pas : ici sa transcription seule ; le PDF imprimé est dans le dépôt privé,
`reference-privee/frais-professionnels/legisocial-allocations-forfaitaires-frais-professionnels-2025-20241224.pdf`
(SHA-256 `428bb190…f295a85b`).

| fichier | ce que c'est | SHA-256 |
|---|---|---|
| `legisocial-allocations-forfaitaires-frais-professionnels-2025-20241224.txt` | la transcription du PDF (`pdftotext -layout`) | `bbca6250…dec288ef` |

## Pourquoi cette page

Du 1er janvier au 6 septembre 2025, les limites sont celles de l'arrêté du 20 décembre 2002
(`../dfs/historique/legifrance-arrete-20021220-frais-professionnels-20250907.pdf`) : ses montants sont ceux de sa
rédaction du 02/11/2022 (20,20 €, 9,90 €, 69,50 €, 51,60 €), revalorisés chaque 1er janvier, et « le barème des
montants est établi et diffusé par l'Agence centrale des organismes de sécurité sociale » (art. 10, l. 338-345 de la
transcription). Le barème de l'Acoss pour 2025 n'est pas déposé ; cette page en donne les montants, tels que le BOSS les
publiait au 24/12/2024.

## Ce qu'on en lit (montants pour 2025)

- **Repas** (l. 44-67) : 7,40 € sur le lieu de travail ; 21,10 € en déplacement, contraint de prendre son repas au
  restaurant ; 10,30 € non contraint (hors des locaux de l'entreprise ou sur chantier).
- **Grand déplacement en métropole** (l. 72-101) : par repas 21,10 € les trois premiers mois (17,90 € du 4e au 24e mois,
  14,80 € du 24e au 72e) ; logement et petit déjeuner à Paris et dans les Hauts-de-Seine, la Seine-Saint-Denis et le
  Val-de-Marne 75,60 € (64,30 €, 52,90 €), dans les autres départements 56,10 € (47,70 €, 39,30 €).

Ce sont les montants de l'arrêté du 4 septembre 2025 en vigueur le 07/09/2025 (art. 3, 1° et 3°, art. 5, I, 1°) :
l'année 2025 n'a qu'un jeu de montants.

## Où le moteur s'en sert

Les limites de 2025 du barème, du 1er janvier au 6 septembre : `logiciel/donnees/baremes/2025.json`, `repas_deplacement`
et `logement_grand_deplacement` (`engagement::frais_au_dela`).

# Urssaf — « Les frais professionnels » (les montants de l'année)

Relevée par Léandre depuis son poste le 03/11/2026 (urssaf.fr refuse le conteneur) : la page « Les frais professionnels »
d'urssaf.fr, « mis à jour le 30 octobre 2026 », imprimée en PDF.

| fichier | ce que c'est | SHA-256 |
|---|---|---|
| `urssaf-frais-professionnels-20261030.pdf` | la page, imprimée | `80d60c58…49ecb2bd` |
| `urssaf-frais-professionnels-20261030.txt` | sa transcription (`pdftotext -layout`) | `c56bf4dd…edb54d79` |

## Ce qu'on en lit

L'arrêté du 4 septembre 2025 (`../dfs/arrete-2025-frais-professionnels/`) fixe les montants en vigueur le 07/09/2025 et
les revalorise chaque 1er janvier (art. 10) ; cette page publie les montants revalorisés.

- **Repas, montants pour 2026** (l. 120-135) : 7,50 € sur le lieu de travail ; 21,40 € en déplacement, contraint de
  prendre son repas au restaurant ; 10,40 € en déplacement, non contraint.
- **Grand déplacement en métropole, montants pour 2026** (l. 772-820) : repas 21,40 € les trois premiers mois (18,20 €
  du 4e au 24e mois, 15,00 € du 24e au 72e) ; logement et petit déjeuner à Paris et dans les Hauts-de-Seine, la
  Seine-Saint-Denis et le Val-de-Marne 76,60 € (65,10 €, 53,60 €), dans les autres départements 56,80 € (48,30 €,
  39,80 €). L. 761-769 : les montants valent pour trois mois de déplacement, continus ou discontinus ; réduits de 15 % à
  compter du 4e mois, de 30 % à compter du 25e, et rien au-delà de six ans. **L'arrêté du 4 septembre 2025 dit autre
  chose au-delà de deux ans** (art. 5, II : 70 % « jusqu'au soixantième mois », rien au-delà) : la page reprend la durée
  de l'arrêté de 2002 (quatre années supplémentaires). Le moteur n'applique aucune des deux réductions (il ne compte pas
  la durée d'affectation sur un même lieu).
- **Outre-mer, montants depuis le 22 septembre 2023** (l. 827-887) : Martinique, Guadeloupe, Guyane, La Réunion, Mayotte,
  Saint-Pierre-et-Miquelon, Saint-Barthélemy, Saint-Martin : hébergement 120 € (102 €, 84 €), repas 20 € (17 €, 14 €) ;
  Nouvelle-Calédonie, Wallis-et-Futuna, Polynésie française : hébergement 120 € (102 €, 84 €), repas 24 € (20,40 €,
  16,80 €). Le moteur ne les applique pas : il ne sait pas qu'une ligne est outre-mer.
- **Étranger** (l. 899-921) : l'indemnité de mission du pays, « consultables sur le site du ministère de l'Économie »
  (les taux de chancellerie), réduite de 65 % si le salarié est logé, de 17,5 % ou 35 % s'il est nourri. Le moteur ne les
  applique pas.

## Où le moteur s'en sert

Les limites de 2026 du barème : `logiciel/donnees/baremes/2026.json`, `repas_deplacement` et `logement_grand_deplacement`
(`engagement::frais_au_dela`).

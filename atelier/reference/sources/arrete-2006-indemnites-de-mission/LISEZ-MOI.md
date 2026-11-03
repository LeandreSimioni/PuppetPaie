# L'arrêté du 3 juillet 2006 : les indemnités de mission des personnels civils de l'État

`legifrance-arrete-20060703-indemnites-de-mission-20261103.txt` : l'arrêté du 3 juillet 2006 fixant les taux des
indemnités de mission prévues à l'article 3 du décret n° 2006-781 (NOR BUDB0620004A), dernière mise à jour du
27/11/2025, copié depuis Légifrance par Léandre le 03/11/2026 (le site refuse le conteneur) ; transcription telle que reçue.

**Ce qu'on en lit.**
- Art. 1, a) (en vigueur depuis le 22/09/2023) : outre-mer, 120 € l'hébergement ; 20 € le repas (Martinique,
  Guadeloupe, Guyane, Réunion, Mayotte, Saint-Pierre-et-Miquelon, Saint-Barthélemy, Saint-Martin), 24 € (Nouvelle-
  Calédonie, Wallis et Futuna, Polynésie française). Les taux de la métropole (90, 120, 140 €) ne valent que pour la
  fonction publique : un salarié en métropole suit l'arrêté du 4 septembre 2025 (`../dfs/`). Le BOSS reprend l'outre-mer
  et Monaco (§ 1340-1360, 1400, `../boss/boss-frais-professionnels-20261013.txt`).
- Art. 1, b) : 150 € d'hébergement pour un travailleur handicapé en situation de mobilité réduite. Le moteur sait si un
  salarié est bénéficiaire de l'obligation d'emploi (`statut_boeth`), pas s'il est à mobilité réduite : non porté.
- Art. 2 : à l'étranger, l'indemnité est réduite de 65 % quand l'agent est logé gratuitement, de 17,5 % quand il est
  nourri à l'un des repas, de 35 % aux deux (BOSS § 1390 : à appliquer pour la limite d'exonération).
- L'annexe (en vigueur depuis le 27/11/2025) : un montant journalier par pays, en monnaie locale ; certaines villes ont
  leur ligne (Toronto, Vancouver, New York en deux périodes, Tokyo, Lomé, Abuja/Lagos/Port-Harcourt) et l'Afrique du Sud un
  montant du 15/12 au 01/03. Les valeurs dans le temps se lisent dans la table de la DGFiP (`../dgfip-frais-de-mission/`).

**Où le moteur s'en sert.** Barème `frais_hors_metropole` (art. 1, a) ; `droit::missions` et `engagement::frais_au_dela`
(l'annexe et l'art. 2 : la réduction de 65 % quand la compagnie paie l'hôtel, la case `loge`).

# Le décret n° 2006-781 du 3 juillet 2006 : les frais de déplacement des personnels civils de l'État

`articles-20261103.md`, `xml/`, `releve.json` : relevés par `moteur releve-codes moissonner` puis `rapport` le
03/11/2026, depuis la base LEGI de la DILA (`JORFTEXT000000242359`, NOR BUDB0620002D ; l'identifiant se lit dans les
liens des articles de l'entrepot qui le citent). Le texte suivi : `atelier/sources/moteur/src/atelier/releve/codes/textes.rs`,
clé `missions-2006`. Le décret entier (articles 1 à 14) est dans l'entrepot (`../legi-entrepot/`) ; le relevé garde les
deux qui servent.

**Ce qu'on en lit.**
- Art. 3 : l'indemnité de mission rembourse forfaitairement les frais supplémentaires de repas et d'hébergement ; à
  l'étranger, logé ou nourri gratuitement, elle est réduite d'un pourcentage fixé par arrêté (l'arrêté du même jour,
  art. 2 : `../arrete-2006-indemnites-de-mission/`).
- Art. 7 : à l'étranger, un arrêté fixe les taux des indemnités de mission par pays, ou par ville ou par région.
- **Le décret ne dit ni dans quelle monnaie l'indemnité se paie, ni à quel taux elle se convertit.** Aucun de ses articles
  (1 à 14) ne parle de monnaie, de change ou de taux de chancellerie. La conversion au taux de chancellerie du jour reste
  une INFÉRENCE (`../dgfip-frais-de-mission/LISEZ-MOI.md`).

**Où le moteur s'en sert.** `droit::missions` (l'indemnité du jour à l'étranger, la limite d'exonération que le BOSS
lui adosse, § 1380-1390).

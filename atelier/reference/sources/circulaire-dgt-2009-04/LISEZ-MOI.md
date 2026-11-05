# Circulaire DGT n° 2009-4 du 17 mars 2009 (rupture conventionnelle d'un CDI)

`travail-circulaire-dgt-2009-04-20090317.pdf` : la circulaire telle que publiée au Bulletin officiel du ministère du
travail (NOR : MTST0980929C, « texte non paru au Journal officiel »), envoyée par Léandre le 05/11/2026.
`travail-circulaire-dgt-2009-04-20090317.txt` : son texte, extrait par `pdftotext -layout`, sans retouche.

Ce qu'on en lit : l'indemnité spécifique de rupture conventionnelle d'un salarié de moins d'un an d'ancienneté « lui est
due au prorata du nombre de mois de présence » (`.txt` l. 212) ; le salaire de référence lisse au prorata les primes
(l. 192).

**Où le moteur s'en sert** : `logiciel/donnees/permanents-base.json`, `rupture_conventionnelle.indemnite` (sa `source`, une convergence avec R1234-1 et R1234-2) : sous huit mois d'ancienneté, le plancher au prorata des mois complets, au taux de R1234-2 en vigueur (un quart) et non à celui de l'exemple de la circulaire (un cinquième, le taux de 2009) ; `droit::permanents::indemnite_rupture_conventionnelle`.

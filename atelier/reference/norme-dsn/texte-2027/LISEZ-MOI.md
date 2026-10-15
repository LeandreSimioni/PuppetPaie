# Le cahier technique NEODeS 2027.1.1, en texte, par bloc et par chapitre

D'où ça vient : `../dsn-cahier-technique-2027.1.1.pdf` (cahier du 10/06/2026, 392 pages, SHA-256
`1593448611c1d328232730210a9ef9efce8bdd5521cf4cbbff2713db5bae30bf`), déposé par Léandre le 18/09/2026
(voir `../LISEZMOI.md`). Découpé le 10/10/2026 (lot V4a), comme `../texte/` l'est pour la 2026.1.

## Comment c'est découpé

- Chaque page : `pdftotext -layout -f N -l N`, fins de ligne nettoyées, toute suite de trois espaces ou plus
  repliée en deux, les lignes vides successives réduites à une. Chaque page commence par `--- p. N ---` (numéro
  de page **du PDF**, pas le « N / 370 » imprimé). La même transformation, appliquée au PDF 2026.1, redonne
  octet pour octet les 72 fichiers de `../texte/` (vérifié le 10/10/2026, page par page).
- Les chapitres : pages du sommaire (p. 1 à 6) — introduction p. 7-8, chapitre 1 p. 9-17, 2 p. 18-71, 3 p. 72-82,
  4 p. 83-103, 5 p. 104-110, 6 p. 111-112, 7 p. 113-115, 8 p. 116-127 (avec la page de titre « Rubriques »).
- Les blocs : la page de début est celle que donne le sommaire (« Sxx.G00.xx …… page ») ; la fin est la page
  d'avant le bloc suivant quand celui-ci commence en haut de page, **la page où il commence** sinon (un bloc qui
  finit en milieu de page s'y termine : la page est alors dans les deux fichiers). Le dernier bloc (`S90.G00.90`)
  va jusqu'à la page d'avant les tableaux.
  **Différence avec `../texte/`** : la 2026 avait été découpée d'après l'en-tête de page seul (l'en-tête nomme le
  premier bloc qui *commence* sur la page) ; un bloc y perd la fin qui déborde sur la page du suivant (ainsi
  `S10.G00.00.006`, p. 129 de la 2026, n'est pas dans `../texte/bloc-S10.G00.00.txt`, seulement dans
  `bloc-S10.G00.01.txt`), et un bloc qui ne commence aucune page n'a pas de fichier (`S89.G00.43` en 2026).
  Ici, rien ne se perd : `S21.G00.72` (p. 293), `S89.G00.35` (p. 338) et `S89.G00.43` (p. 339) ont leur fichier,
  et `S10.G00.00.006` est dans `bloc-S10.G00.00.txt` (p. 129).
- `tableaux-des-usages.txt` : p. 371-392 (tableau des usages, puis tableau des usages de contrôles), comme
  `../texte/tableaux-des-usages.txt` (p. 363-382 de la 2026.1).

## Où le moteur s'en sert

Ces fichiers sont la lecture ; les tables que le moteur applique se relèvent depuis le PDF par les outils de
l'atelier (`atelier/outils/extraire_usages.py`, `extraire_formats.py`) : `logiciel/donnees/usages-dsn-base.json`
et `formats-dsn-base.json`, partie `P27V01`. La version de la norme qu'un fichier porte (`S10.G00.00.006`) se
lit de `logiciel/donnees/norme-dsn-base.json` ; la seule valeur que ce cahier admet est « P27V01 - Année 2027 Version 1 »
(p. 129 du PDF, `bloc-S10.G00.00.txt`).

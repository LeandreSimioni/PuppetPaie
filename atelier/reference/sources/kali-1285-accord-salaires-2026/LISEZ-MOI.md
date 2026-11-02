# CCNEAC (IDCC 1285) : l'accord de salaires du 1er juillet 2026 et son extension

Relevé le 2026-11-02 dans les données ouvertes de la DILA, chaque XML tel que l'archive le livre (octet pour octet) :

| fichiers | archive (données ouvertes DILA) |
|---|---|
| `jorf-1285-arrete-extension-*-20261102.xml` : l'arrêté du 7 septembre 2026 (JORFTEXT000054860820, version) et ses articles 1 à 3 (JORFARTI000054860824, …826, …828) | https://echanges.dila.gouv.fr/OPENDATA/JORF/JORF_20260919-002738.tar.gz (`jorf/global/texte/version/…`, `jorf/global/article/…`) |
| `kali-1285-accord-salaires-2026-*-20261102.xml` : l'accord (KALITEXT000054981887, version et structure) et ses articles (préambule, 1er, 1.1, 1.1.1 à 1.1.4, 1.2, 2, 3, 4, 5) | https://echanges.dila.gouv.fr/OPENDATA/KALI/KALI_20261015-214025.tar.gz (`kali/global/texte/…`, `kali/global/article/KALI/ARTI/00/00/54/98/…`) |

Transcription : `kali-1285-accord-salaires-2026-transcription-20261102.md` (articles 2 à 5 de l'accord, les trois articles
de l'arrêté, et ce qu'on en lit). Les grilles des articles 1.x sont transcrites depuis le texte signé dans
`../ccneac-nao-2026-grilles-transcription-20260929.md`.

## Ce qu'on en lit

- L'accord est **étendu** : arrêté du 7 septembre 2026, NOR TRST2622487A, JORF n° 0219 du 19 septembre 2026 ; son
  article 1 rend obligatoires toutes ses stipulations pour tous les employeurs de la convention (les articles 1.1.1 à 1.1.3
  sous réserve du principe « à travail égal, salaire égal ») ; son article 2 fait prendre effet à l'extension **à la date
  de publication**, le 19/09/2026.
- L'accord lui-même s'applique au 01/08/2026 pour les minima et au **01/09/2026 pour les indemnités et primes** (art. 5) ;
  avant l'extension, il ne lie que les adhérents de ses six organisations d'employeurs signataires (C. trav. L2262-1,
  `../legi-entrepot/articles.jsonl.gz`, clé `["travail", "L2262-1"]`).
- L'indemnité de déplacement (art. 2) : à compter du 01/09/2026, **21,40 €** chaque repas principal, **76,60 €** chambre et
  petit déjeuner, 7,50 € le petit déjeuner seul (119,40 € au total) ; jusqu'au 31/08/2026, 20,70 €, 74,30 € et 7,30 €.

## Où le moteur s'en sert

- `logiciel/donnees/baremes/2026.json`, `par_convention."1285".bareme.tarifs_conventionnels` : `repas` et `nuitee`, listes
  datées (20,70 / 74,30 jusqu'au 18/09/2026 ; 21,40 / 76,60 depuis le 19/09/2026, date de l'extension) ; la note `_source`
  dit pourquoi le 19/09 et pas le 01/09. Lues par `engagement::tarif_conventionnel` (le taux d'une ligne « Repas » ou
  « Nuitée » saisie sans taux, `engagement::lignes_au_tarif`) et par `moteur::routes::mois::dire_les_tarifs` (la phrase
  qui dit d'où vient le montant).
- Les minima de l'article 1 (au 01/08/2026) : `logiciel/donnees/minima-conventionnels.json` n'en porte qu'une partie, sous
  `1285.non_etendu` (cachets dramatiques et répétitions, pour les adhérents d'une organisation signataire).
- `logiciel/donnees/sources.json`, source `kali-1285-accord-salaires-20260701`.

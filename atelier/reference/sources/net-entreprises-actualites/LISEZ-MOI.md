# Les actualités de net-entreprises.fr (pages déposées)

Capturées depuis le poste par Léandre (`.mht`, Chrome), transcription à côté (le corps de l'article seul, extrait du
HTML ; la page complète reste dans le `.mht`). Nom : `net-entreprises-actualite-<objet>-<date de publication>`.

## `net-entreprises-actualite-naf-2025-en-dsn-janvier-2027-20261008.mht` — « Nouvelle nomenclature NAF à compter de janvier 2027 ! »

URL : `https://www.net-entreprises.fr/nouvelle-nomenclature-naf-a-compter-de-janvier-2027/`, **publié et modifié le
08/10/2026**, capturé le 11/10/2026. Empreinte SHA-256 (16 premiers caractères) : `edf313e615f317b4`, fichier déposé tel quel.

Ce qu'elle dit (transcription, 14 lignes) :
- la **NAF 2025** entre en vigueur en **janvier 2027** (décret n° 2025-736 du 31/07/2025) ;
- « Par simplification, **le code NAF n'est plus à renseigner en DSN** » : en P27V01, suppression de `S21.G00.06.003`
  (APEN) et `S21.G00.11.002` (APET) ; le système DSN les récupère lui-même pour les partenaires ;
- bloc 85 (lieu de travail) : plus de code APE si le lieu est SIRETisé ; sinon, « continuer à le renseigner dès janvier
  2027 avec la nouvelle codification NAF » ;
- **bascule** : « A compter de la mise en production DSN de janvier 2027, il est prévu que la nomenclature NAF 2025
  s'applique quelle que soit la version de norme (P27 et P26). »

Ce que ça confirme et ce que ça change :
- **V4a** a déjà retiré `06.003` et `11.002` des DSN P27V01 (`logiciel/donnees/norme-dsn-base.json`, relevé sur le
  cahier 2027.1.1) : la page le dit en clair, c'est sa source de doctrine.
- **D1** (la règle de bascule, `prive/journal/norme/corrections/A-RECUPERER-ET-A-TRANCHER.md`) : le **schéma « Pour
  résumer »** (image de la page, transcrit dans le `.txt` depuis la capture de Léandre, 11/10/2026) dessine janvier
  2027 coupé par la **« MEP P27 »** (mise en production) : avant elle, **seule la norme P26** se déclare, en NAF 2008 ;
  après elle, **P26 et P27 sont toutes deux déclarables**, en NAF 2025, P26 avec `06.003`/`11.002`/`85.002`, P27 sans
  `06.003` ni `11.002`. Ce qu'on en tient : **une DSN P26V01 se dépose encore en janvier 2027 après la mise en
  production** ; la DSN de décembre 2026 déposée en janvier 2027 peut donc partir en P26V01, et en P27V01 après la MEP.
  Ce que la page ne dit pas : le jour de la MEP, et jusqu'à quand P26V01 reste accepté au-delà de janvier (le schéma
  s'arrête au 31/01/2027). Une DSN P26V01 déposée après la MEP écrit `06.003`/`11.002` **en NAF 2025** (le moteur
  écrit le code de la fiche entreprise tel quel : c'est la fiche qu'il faudra mettre à jour).
- **Hors DSN, le code NAF reste imprimé** : sur le bulletin (`paie/src/bulletin.rs`, `naf` de la fiche entreprise),
  le contrat (`avenant.rs`, `cachet.rs`), l'amorçage (`amorcage.rs`, `11.002` selon l'usage). Au 01/01/2027, la fiche
  entreprise de chaque compagnie passe en NAF 2025 : **le code 90.01Z de la compagnie du poste a-t-il un nouveau
  code ?** La table de passage NAF 2008 → NAF 2025 (INSEE) n'est pas déposée ; la table NAF du fichier des
  nomenclatures (`../dsn-nomenclatures/`, 732 lignes, non transcrite) est à confronter.

Le **décret n° 2025-736 du 31/07/2025** (JORF) est déposé depuis le 11/10/2026 (page Légifrance capturée par Léandre,
`../insee-naf-2025/legifrance-decret-2025-736-naf-2025-20250731.{mht,txt}` ; le 13/10/2026 (lot T1), le JORF du 01/08/2025 de
l'open data DILA, `JORFTEXT000052019213`, art. 1 à 7 + annexe, confirme les articles 1er et 2 : NAF 2025 approuvée, en vigueur le
01/01/2027). La structure
NAF 2025 et la table de correspondance de l'INSEE sont déposées (11/10/2026) : `../insee-naf-2025/` (`90.01Z` →
`90.20Y` ou `90.39G`, selon l'INSEE).

Où le moteur s'en sert : `logiciel/donnees/norme-dsn-base.json`, P27V01, `regles.code_naf_en_naf_2025` (lot R5b,
11/10/2026) : « Il convient néanmoins de s'assurer que le code NAF soit modifié en adéquation avec cette nouvelle
nomenclature » et « la nomenclature NAF 2025 s'applique quelle que soit la version de norme » après la mise en production ;
`paie/src/naf.rs`, `verifier_pour` : un fichier P27V01 refuse un code NAF rév. 2 de la fiche entreprise. INFÉRENCE : un
fichier P27V01 n'existe qu'après la mise en production (le schéma « Pour résumer » ne montre P27 qu'après la « MEP P27 »).

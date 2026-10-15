# Fiches de paramétrage des organismes

Les documents que les organismes publient pour les éditeurs et les gestionnaires de paie (taux, plafonds, répartitions).
Plusieurs sont cités par `logiciel/donnees/sources.json` (empreintes) et par les notes `_source` des barèmes : on ne
les renomme pas.

## `Audiens-Parametres-2025.pdf` (texte : `Audiens-Parametres-2025.txt`, `pdftotext -layout`)

« Paramètres Audiens 2025 », 4 pages, apporté par Léandre depuis le poste le 10/10/2026. Ce qu'on en lit :
- p. 3, **Apec** : « La cotisation est calculée sur le salaire brut dans la limite de 4 fois le plafond de la Sécurité
  sociale. Le taux est de 0,06 %, réparti entre l'employeur pour 0,036 % et le salarié pour 0,024 %. » Mêmes valeurs
  qu'en 2026 : confirme les lignes `apec_t1` / `apec_t2` du barème 2025, héritées du socle 2026 (lot 1g, qui notait
  « aucun texte 2025 déposé ne les confirme »).
- p. 1-2 : plafonds de la retraite complémentaire et de la prévoyance des intermittents (annuels pour les non-cadres,
  journaliers pour les cadres ; prévoyance : plafond journalier ; Apec : 4 plafonds).
- p. 3-4 : prévoyance et santé des intermittents (accord interbranches, assiette : brut dans la limite de la tranche A
  journalière) ; Congés Spectacles (15,50 % du 01/01/2025 au 31/03/2026).

## `Audiens-Parametres-2026.pdf` : déplacé

« Paramètres Audiens 2026 » est rangé depuis le 11/10/2026 (lot R6, reste #177) dans
`atelier/reference/sources/audiens-parametres-2026/`, avec sa transcription et son `LISEZ-MOI.md` (d'où, quand, ce
qu'on en lit page par page, où le moteur s'en sert). `sources.json` et les notes `_source` des barèmes 2025 et 2026
y renvoient. Ce dossier n'est pas renommé pour autant : les autres fichiers restent ici.

## Dans le dépôt public (15/10/2026)

Ce dossier est dans le dépôt public de PuppetPaie. Les originaux suivants, d'un organisme privé, n'y sont pas : seule leur transcription y est (`pdftotext -layout` pour un PDF, une feuille de calcul en CSV, feuille par feuille, le texte d'une page HTML), et c'est elle que citent les renvois. Les condensats donnés plus haut sont ceux des originaux, gardés dans le dépôt privé (`reference-privee/fiches-parametrage/`).

- `Audiens-Parametres-2025.pdf` : transcription `Audiens-Parametres-2025.txt`
- `Audiens-Webconf-Nouveautes-2026.pdf` : transcription `Audiens-Webconf-Nouveautes-2026.txt`

Restent hors du dépôt public, dans le dépôt privé (`reference-privee/fiches-parametrage/`) :

- `Audiens-taux-2026-conges-spectacles-20260904.xls` : relevé sur une compagnie réelle
- `Audiens-taux-2026-sante-prevoyance-20260904.xls` : relevé sur une compagnie réelle
- `audiens-2025-12-13-fiche139.xml` : relevé sur une compagnie réelle ; une transcription expurgée (sans nom, SIREN ni contact) est publiée : `audiens-2025-12-13-fiche139-expurgee.xml`
- `lire-xls-audiens.py` : pièce dont la nature est douteuse (ni texte d'un émetteur identifié, ni sa transcription)

# Audiens, « Paramètres 2026 » (fiche technique, 4 pages)

## D'où ça vient, quand

- **Le texte brut** : `Audiens-Parametres-2026.pdf`, tel que servi par Audiens à l'adresse
  https://www.audiens.org/files/live/sites/siteAudiens/files/03_documents/entreprise/Fiches-techniques/Parametres-et-taux-2026.pdf
  (l'URL de `logiciel/donnees/sources.json`, source `audiens-parametres-et-taux-2026`). Relevé le 12/09/2026
  (`releve_le` de la même source) ; le PDF dit lui-même avoir été fabriqué le 22/06/2026 (`pdfinfo`). Empreinte
  SHA-256 : `62e55d5b40421ae624f6c083fc24e330e2a9a831caa03a57d5258403c7f0d68a` (celle de `sources.json`, inchangée).
- **Sa transcription** : `Audiens-Parametres-2026.txt`, `pdftotext -layout` du même fichier (11/10/2026), pour la
  lecture et la recherche ; c'est le PDF qui fait foi.
- **Déplacé le 11/10/2026** (lot R6, reste #177) depuis `atelier/reference/fiches-parametrage/`, où il était rangé sans
  `LISEZ-MOI.md` à lui : `git mv`, le fichier n'a pas changé d'un octet. `sources.json` (`fichier`), les notes
  `_source` des barèmes 2025 et 2026 et leur relevé de test (`atelier/sources/paie/tests/donnees/contexte.json`),
  `atelier/reference/fiches-parametrage/LISEZ-MOI.md` et `reference-privee/veille-taux.md (relevé sur une compagnie réelle, pièce hors du dépôt public : dépôt privé)` suivent. Les journaux et les
  archives gardent l'ancien chemin : ce sont des récits datés.

Le millésime 2025 (« Paramètres Audiens 2025 », même gabarit) reste dans `atelier/reference/fiches-parametrage/`,
décrit dans le `LISEZ-MOI.md` de ce dossier.

## Ce qu'on en lit

Pages du PDF (le pied de page numérote 2 à 4 ; la page 1 n'a pas de numéro) :

- **p. 1, « Les plafonds des régimes »** : T1 = 48 060 € l'année, 4 005 € le mois, 220 € la journée (intermittent
  cadre) ; T2 = 336 420 € l'année, 28 035 € le mois, 1 540 € la journée. « Intermittents du spectacle non-cadres : les
  plafonds applicables sont les plafonds annuels » ; « Intermittents du spectacle cadres : les plafonds applicables
  sont les plafonds journaliers - dans la limite du plafond mensuel » ; prévoyance des intermittents : « le plafond
  journalier - dans la limite du plafond mensuel » ; « Cotisation Apec pour les cadres : le plafond applicable est 4
  fois le plafond de la Sécurité sociale ».
- **p. 2, « Les modalités de calcul en retraite complémentaire »** : les tranches d'un permanent « prorata
  temporis » ; celles d'un intermittent non cadre sur les plafonds annuels « quelle que soit la durée du travail » ;
  d'un cadre, sur le plafond journalier ; la CET « due sur la totalité du salaire dans la limite de la T1 + T2 » si le
  salaire dépasse le PSS.
- **p. 3, « Taux de cotisation »**, population par population (parts salariale / patronale) :
  - permanents : retraite T1 3,15 / 4,72, T2 8,64 / 12,95 ; CEG T1 0,86 / 1,29, T2 1,08 / 1,62 ; CET 0,14 / 0,21 ;
    Apec 0,024 / 0,036 sur « T1 x 4 » ;
  - artistes intermittents non cadres : retraite T1 annuelle 4,44 / 4,45, T2 10,79 / 10,80 ; CEG et CET comme
    ci-dessus ;
  - techniciens intermittents non cadres : retraite T1 annuelle 3,93 / 3,94, T2 10,79 / 10,80 ;
  - artistes et techniciens intermittents cadres : retraite T1 journalière 3,93 / 3,94, T2 journalière 8,64 / 12,95 ;
    Apec 0,024 / 0,036 sur « T1 journalière x4 » ;
  - journalistes pigistes : retraite 5,08 / 7,62 et CEG 0,86 / 1,29 sur la totalité du salaire ;
  - « Le taux sur T1 et la répartition sur T1 et T2 peuvent être différents en application d'obligations résultant
    de Conventions collectives, d'accords de retraite ou de conditions particulières adoptées au sein de
    l'entreprise. »
- **p. 4, « Association pour l'emploi des cadres (APEC) »** : « La cotisation est calculée sur le salaire brut dans
  la limite de 4 fois le plafond de la Sécurité sociale. Le taux est de 0,06 %, réparti entre l'employeur pour
  0,036 % et le salarié pour 0,024 %. »
- **p. 4, prévoyance et santé des intermittents** (accord interbranches) : assiette « le salaire brut dans la limite
  de la tranche A journalière » ; non cadres : prévoyance 0,12 salariale + 0,34 patronale, Fonds collectif du
  spectacle pour la santé 0,77 patronale (total 0,12 / 1,11) ; cadres : 0,12 + 0,88, Fonds 0,81 (total 0,12 / 1,69) ;
  « la CCN des entreprises artistiques et culturelles prévoit un taux spécifique en prévoyance pour les intermittents
  cadres ».
- **p. 4, Congés Spectacles** : assiette « le salaire brut avant tout abattement pour frais professionnel, hors
  application de plafond spécifique » ; taux 15,50 % « pour la période du 1er janvier 2026 au 31 mars 2027 ».

## Où le moteur s'en sert

- `logiciel/donnees/sources.json`, source `audiens-parametres-et-taux-2026` : « Source des taux `retraite_t1` du
  socle » (p. 3), `nourrit` : `baremes/2026.json`. La vérification des sources n'est pas portée en Rust
  (`atelier/CLAUDE.md`) : le chemin y est tenu à jour pour le jour où elle le sera.
- `logiciel/donnees/baremes/2026.json` et `2025.json` : les notes `_source` de l'Apec d'un cadre, la cotisation
  « 132 - Cotisation Apec » (`par_categorie.cadre.cotisations_ajoutees` et `par_categorie.cadre_permanent`) : taux,
  parts et assiette des lignes `apec_t1` / `apec_t2` du bulletin (p. 3 et p. 4). Le barème 2025 cite ce millésime
  2026 parce que ses valeurs Apec sont héritées du socle 2026 ; le millésime 2025 les confirme
  (`fiches-parametrage/LISEZ-MOI.md`).
- Les taux de retraite complémentaire, de CEG et de CET des quatre populations du barème 2026 (corps des artistes,
  `par_statut.technicien`, `par_categorie.cadre`, `par_nature.permanent`) ont ces valeurs (p. 3), retraite et CEG
  réunies sur une ligne : 5,30 / 5,74 = 4,44 + 0,86 / 4,45 + 1,29 pour un artiste ; 4,79 / 5,23 pour un technicien et
  pour la T1 journalière d'un cadre ; 4,01 / 6,01 et 9,72 / 14,57 (T2) pour un permanent ;
  `atelier/reference/sources/agirc-arrco/LISEZ-MOI.md` dit ce que le cahier technique Agirc-Arrco y ajoute.
- La prévoyance des intermittents et le Fonds santé (p. 4) sont aussi dans `accord-interbranche-prevoyance-
  intermittents/` (l'accord lui-même), que la note `_source` de la ligne `prevoyance` du barème cite.

**Inférence** (écrite comme telle) : que les « parts » de la p. 3 soient celles de toute compagnie de la branche.
Le renvoi de bas de page de la p. 3 (« peuvent être différents en application […] de Conventions collectives,
d'accords de retraite ou de conditions particulières ») est ce qui fait vivre, à côté du socle, les couches de
convention et de compagnie du barème (`atelier/docs/ARCHITECTURE.md`, « Le barème a trois étages »).

## Dans le dépôt public (15/10/2026)

Ce dossier est dans le dépôt public de PuppetPaie. Les originaux suivants, d'un organisme privé, n'y sont pas : seule leur transcription y est (`pdftotext -layout` pour un PDF, une feuille de calcul en CSV, feuille par feuille, le texte d'une page HTML), et c'est elle que citent les renvois. Les condensats donnés plus haut sont ceux des originaux, gardés dans le dépôt privé (`reference-privee/sources/audiens-parametres-2026/`).

- `Audiens-Parametres-2026.pdf` : transcription `Audiens-Parametres-2026.txt`

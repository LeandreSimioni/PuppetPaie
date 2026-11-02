# La subrogation des indemnités journalières, et le bordereau de paiement de la CPAM

Relevé le 02/11/2026 ; les pages et le pack ont été apportés par Léandre le même jour (le nuage n'atteint pas ameli.fr :
403). Les PDF sont tels que servis ; chaque `.txt` est leur transcription (`pdftotext -layout`).

## Les pages

- **`ameli-subrogation-salaire-arret-20261102.pdf`** (`.txt`) : ameli.fr, Entreprise, « La subrogation de salaire en cas
  d'arrêt de travail », article du 08/01/2026 (`https://www.ameli.fr/entreprise/vos-salaries/arret-de-travail/subrogation-salaire`).
  Ce qu'on en lit : la subrogation demande que « le salaire maintenu au cours de cette période [soit] au moins égal au
  montant des indemnités perçues » ; elle se demande dans le signalement d'arrêt de la DSN ; sa date de début est celle
  du début de l'arrêt, sa date de fin « au terme de la durée maximale de maintien du salaire et de la subrogation, telle
  qu'elle est définie par votre convention collective » (« Il ne faut surtout pas se limiter à la date de fin de l'arrêt
  transmis par votre salarié ») ; une prolongation n'appelle pas de nouveau signalement ; les indemnités sont versées à
  l'employeur « tous les 14 jours en moyenne » ; le bordereau de paiement (BPIJ) est fait en XML pour les logiciels.
- **`net-entreprises-fiche3028-subrogation-assurance-maladie-20261102.pdf`** (`.txt`) : base de connaissances
  net-entreprises, fiche 3028 « La subrogation pour l'Assurance maladie », modifiée le 23/03/2025. La même règle des dates
  (« N'indiquez pas la date de fin de l'arrêt de travail ou du congé, mais bien la date de fin de la période pendant
  laquelle peut s'appliquer le maintien de salaire et la subrogation ») et la subrogation de plein droit de R323-11 du code
  de la sécurité sociale.

## Le pack du BPIJ (`bpij-edi/`)

Le « Bordereau de paiement des indemnités journalières en mode EDI » (CNAMTS), tel que l'offre la page d'ameli :
`cnamts-bpij-modele-v02-00r3-20120306.pdf` (le modèle des données, version 02.00R3 du 06/03/2012),
`cnamts-entbpij-modele-v01-00-20110608.pdf` (l'en-tête d'échange, version 01.00 du 08/06/2011), et leurs schémas XSD
(`cnamts-bpij-xsd-v02-00-20121018.zip`, `cnamts-entbpij-xsd-v01-00.zip`, gardés tels que livrés). Ce qu'on en lit
(`cnamts-bpij-modele-v02-00r3-20120306.txt`, l. 689-860) : un bordereau porte, par salarié subrogé, des **prestations**
de nature `IJNOR` (« I.J. normale »), `IJMAJ` (« I.J. majorée »), `CAR` (carence, montant 0,00), `IJPRE`, et des lignes
`RETCSG` / `RETRDS` (retenues CSG et RDS) et `REGCSG` / `REGRDS` (leurs régulations) ; chaque prestation dit ses dates
(`DateDebPrest`, `DateFinPrest`), le nombre d'indemnités payées (`NbIJ`), le **prix unitaire** (`PU`) et le montant ;
des indus et rappels (`RegularisationPrestation`) ; la domiciliation bancaire du mandataire. **Le montant par jour est
donc brut** (le prix unitaire, avant CSG et CRDS, qui se retiennent sur leurs propres lignes).

## Où le moteur s'en sert

- `engagement::maintien` (`atelier/sources/engagement/src/maintien.rs`) : la subrogation d'un arrêt et ses dates
  (`subrogation` : du premier jour de l'arrêt à la fin de la durée maximale du maintien) ; les indemnités se saisissent sur
  l'arrêt par jour, **brutes et nettes de CSG et CRDS** (`ij_brute`, `ij_nette`, et `_majoree` pour un accident du
  travail dès le 29e jour), comme le relevé et le BPIJ les écrivent ; la route `arret-indemnites`.
- `logiciel/donnees/permanents-base.json`, section `indemnites_journalieres`.
- **Ce que le BPIJ permettrait, et que le moteur ne fait pas** : lire le bordereau XML que la CPAM met à disposition de
  l'employeur subrogé, pour remplir les indemnités de chaque arrêt sans saisie (le prix unitaire normal et majoré, les
  retenues CSG et RDS, les jours payés), rapprocher ce que la caisse a versé de ce que la paie a avancé (indus, rappels,
  carence), et dire quand un arrêt subrogé n'a rien reçu. Un ticket, pas ce lot.

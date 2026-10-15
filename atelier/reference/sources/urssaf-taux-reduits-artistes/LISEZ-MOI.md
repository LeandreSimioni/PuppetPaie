# Urssaf — « Les taux réduits cas particuliers » (artistes du spectacle, mannequins, journalistes, professions médicales)

## D'où ça vient

`urssaf-taux-reduits-cas-particuliers-20260715.pdf` : la page urssaf.fr « Les taux réduits cas particuliers », **mise à
jour le 15/07/2026**, imprimée en PDF par Léandre le 11/10/2026 (7 pages ; urssaf.fr coupe les connexions du
conteneur). URL (déduite des liens internes de la page, à confirmer) :
`https://www.urssaf.fr/accueil/employeur/cotisations/liste-cotisations/taux-reduits-cas-particuliers.html`.
Empreinte SHA-256 (16 premiers caractères) : `15cdbb7695912106`. Transcription : `pdftotext -layout`, `.txt` à côté.

## Ce qu'on en lit (section « Les artistes du spectacle », l. 1-95 du `.txt`)

- **Qui** : « Seuls les artistes du spectacle sont concernés […]. Les techniciens ne sont pas concernés. » L'employeur
  dont le spectacle vivant est l'activité principale déclare à l'Urssaf ; occasionnel, au Guso.
- **Sur quoi** : « sur l'ensemble des rémunérations », « sur chaque cachet […] peu importe leur nombre » ; avec la DFS,
  « vous appliquez les taux réduits sur les rémunérations calculées après la DFS ».
- **Quoi** : « La réduction de taux est de 30 % », parts salariale et patronale, sur maladie, vieillesse, **AT
  (« abattu et arrondi à la décimale la plus proche »)**, allocations familiales, FNAL, versement mobilité. **Pas** la
  CSG-CRDS ni la CSA.
- **Le tableau 2026** : maladie 4,90 (plein 9,10) ; CSA 0,30 ; vieillesse patronale **1,48 totalité et 5,99 plafonnée** ;
  AF 2,42 (plein 3,68) ; dialogue social 0,016 ; FNAL 0,07 plafonné sous 50 salariés, 0,35 à partir de 50 ; AT = taux
  de l'entreprise abattu de 30 % ; VM 70 % ; salarié : vieillesse **0,28 totalité et 4,83 plafonnée**, CSG 2,40 + 6,80 et
  CRDS 0,50 sur 98,25 % dans la limite de 192 240 € ; Alsace-Moselle 0,91.
- **Le plafond des engagements de moins de cinq jours** (l. 86-92) : « un plafond aux cotisations d'assurance
  vieillesse pour chaque journée de travail égal à douze fois le plafond horaire », quels que soient les heures, la
  répartition des jours, la nature du travail (répétitions, enregistrements, représentations), le nombre de cachets.
- **CTP** : 312 (artistes cas général abattement 30 %), 316 (FNAL artistes totalité), 334 (FNAL artistes plafonné),
  636 (complément maladie abattement 30 %).
- Journalistes (l. 160-250), hors de notre champ, mais une phrase générale (l. 238) : « L'application de taux réduits ne
  permet pas d'effectuer la réduction du plafond de la Sécurité sociale ni d'appliquer la régularisation de l'assiette
  plafonnée » ; elle est écrite sous les journalistes, **on ne l'étend pas aux artistes sans texte** (le BOSS § 1090,
  `../boss/`, dit pour eux « ne sont pas éligibles à la réduction du plafond » et admet la proratisation aux jours
  rémunérés).
- Textes cités en bas : L. 311-3 16° CSS, arrêté du 26/03/1987, arrêté du 03/02/1975, L. 7122-22 et R. 7122-29 CT,
  **arrêté du 24/01/1975** (déposé : `../arrete-1975-artistes-du-spectacle/`), L. 7123-6 CT.

**Ce n'est pas la page « assiette plafonnée » (D13)** : celle-ci parle des taux ; le plafond n'y est qu'un paragraphe.
D13 reste à chercher sous ce titre, s'il existe encore (la page des taux a pu l'absorber).

## Où le moteur s'en sert

`logiciel/donnees/baremes/2026.json`, section artiste : les notes `_source` recomposent 5,99 / 4,83 / 1,48 / 0,28 /
3,68 / 0,07 depuis la table BOSS × 0,70 et l'arrêté de 1975 ; cette page les **donne directement**, à citer en plus le
jour où le barème se relit. L'AT « abattu et arrondi à la décimale la plus proche » : le barème fait 1,50 × 0,70 = 1,05
(`calcul.bareme_atmp`) ; l'arrondi à une décimale est à vérifier sur un taux CARSAT qui ne tombe pas juste.
`logiciel/donnees/organismes-base.json` : CTP 312, 334, 636 présents ; **316** (FNAL totalité, 50 salariés et plus)
absent, cohérent tant que l'effectif est sous 50 (règle 0 : une case à prévoir). Rien n'est codé ce 11/10/2026.

**Lot R2 (11/10/2026)** : « Seuls les artistes du spectacle sont concernés […]. Les techniciens ne sont pas concernés »
(l. 24) : les CTP 432 et 636 (abattus) ne portent plus que le brut des artistes, et un technicien intermittent relève
de 430 et 635 (taux plein, comme son nominatif 102 à 1,80 et 907 à 6,00) : `logiciel/donnees/organismes-base.json`,
`dsn::ctp_urssaf` (un fichier composé avant se relit ainsi). Preuves : `tests/plafond_intermittents.rs`,
`tests/regularisation_declaree.rs`.

**CTP 316 (lot R2, 11/10/2026)** : « Fnal : Effectif de moins de 50 salariés : 0,07 % dans la limite du plafond ; Effectif
50 salariés et plus : 0,35 % » (l. 89-92), « CTP 316 : FNAL artistes totalité ; CTP 334 : FNAL artistes plafonné »
(l. 148-149). `organismes-base.json` : 334 porte `effectif_sous: 50`, 316 (920, `brut_x115_artiste`, la totalité du brut
majoré) `effectif_au_moins: 50` ; `dsn::Seuils` lit l'effectif de `config/effectifs.json` (cinq années consécutives,
`Effectif::atteint`) pour l'année du mois ; un fichier composé avant, sans 316, refuse à cinquante salariés
(`dsn::fnal_des_artistes`). **Inférence** : à cinquante, le 316 *remplace* le 334 (la page donne un seul taux par tranche).
Preuve : `tests/plafond_intermittents.rs` (`a_cinquante_salaries_…`).

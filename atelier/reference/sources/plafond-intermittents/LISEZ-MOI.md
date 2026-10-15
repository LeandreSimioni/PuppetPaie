# Le plafond d'un intermittent : vieillesse plafonnée, FNAL, chômage (point 22, vague 1i, 10/10/2026)

Ce dossier rassemble les textes sur lesquels le moteur assied, pour un intermittent, la vieillesse plafonnée, le FNAL
et la contribution chômage sur **son** plafond, celui que le bas du bulletin imprime (`paie/src/plafond.rs`,
`calcul::plafond_de_l_engagement`). Les articles de code et l'arrêté de 1975 ne sont pas copiés ici : ils sont relevés
dans la base LEGI de la DILA par `atelier/outils/relever_les_codes.py` (rubrique `plafond` du code de la sécurité
sociale, texte `artistes-1975`), et déposés à côté.

## Les textes, et ce qu'on en lit

### Code de la sécurité sociale (`../code-de-la-securite-sociale/`, relevé LEGI du 10/10/2026, `xml/` et `articles-20261010.md`)

- **L. 241-3** (`LEGIARTI000051289018`, en vigueur depuis le 01/01/2026) : la vieillesse est assise sur les revenus
  d'activité « dans la limite d'un plafond fixé annuellement ».
- **D. 242-4** (`LEGIARTI000053302465`, 01/01/2026) : 8,55 % / 6,90 % « sur la part de la rémunération dans la limite
  du plafond prévu au premier alinéa de l'article L. 241-3 » ; 2,11 % / 0,40 % sur la totalité.
- **R. 242-2** (`LEGIARTI000039222466`, 01/01/2020), I : calcul « chaque mois, dans la limite de la valeur mensuelle
  du plafond » ; al. 2, plafond « ajusté prorata temporis, en fonction de la périodicité de la paie, pour les salariés
  mentionnés aux articles L. 3242-3 » (intermittents, temporaires) ; al. 3 : « Lorsque le contrat de travail d'un
  salarié ne couvre pas l'intégralité des périodes [...], les plafonds [...] sont réduits à due proportion du nombre de
  jours de la période pendant laquelle les personnes sont employées. » C'est le prorata calendaire de `plafond.rs`.
- **D. 242-17 à D. 242-19** : valeurs journalière (IV de D. 242-17) et horaire (D. 242-19 : « la valeur horaire est
  égale à la valeur mensuelle multipliée par 12 et divisée par le nombre d'heures annuelles de travail »), arrondies à
  l'euro. Le plafond horaire 2026 (30 €) est dans `logiciel/donnees/baremes/2026.json`, `plafond_securite_sociale`.

### Arrêté du 24 janvier 1975 (`../arrete-1975-artistes-du-spectacle/`, JORFTEXT000000310494, relevé LEGI du 10/10/2026)

- **art. 1** : taux des artistes à 70 % du régime général ; **art. 2** : FNAL et versement transport à 70 %.
- **art. 3, al. 2** (`LEGIARTI000006782866`, en vigueur depuis le 30/12/2006) : « Pour les périodes d'engagement
  continu inférieures à cinq jours, le plafond mentionné à l'article L. 241-3 du code de la sécurité sociale applicable
  à chaque journée de travail accomplie par un artiste du spectacle pour un même employeur est égal à douze fois le
  plafond horaire, quels que soient le nombre d'heures et la nature du travail effectués dans ladite journée. »
- art. 3 bis : les utilisations secondaires (rediffusions) ; hors champ du moteur.

### Annexes VIII et X du 15/11/2024 au règlement d'assurance chômage (ce dossier)

- `unedic-annexe8-convention-20241115.pdf` (et `.txt`), `unedic-annexe10-convention-20241115.pdf` (et `.txt`) :
  https://unedic.org/ged/documents/regulatory_texts/pdf/TXT-ANX-RG-Annexe_8_2024.pdf et `..._10_2024.pdf`, relevés le
  10/10/2026. **Art. 49, al. 2** (annexe VIII p. 23, `.txt` l. 1203-1204 ; annexe X `.txt` l. 1181-1182) : « Les
  rémunérations mentionnées au premier alinéa sont comprises dans l'assiette des contributions dans la limite d'un
  plafond fixé à quatre fois le plafond mentionné à l'article L. 241-3 du code de la sécurité sociale. » Art. 50 : les
  taux (4 % + 5 % employeur, 2,40 % salarié ; 4,50 % pour certains CDD).
- `unedic-circulaire-2025-05-contributions-20250501.pdf` (et `.txt`) : https://www.unedic.org/ged/documents/circulars/pdf/PRE-CIRC-Circulaire_n_2025-05_du_1er_mai_2025.pdf.
  Fiche technique 1.1 : « Les rémunérations dépassant 4 fois le plafond [...] demeurent exclues de l'assiette des
  contributions [...]. Cette règle est également applicable aux salariés relevant des annexes ». Vaut aussi pour les
  permanents (règlement général, art. 49, déposé depuis dans `../chomage-permanents/`, lot V2e).
- `unedic-fiche-assiette-des-contributions-20261010.html` (et `.txt`) :
  https://www.unedic.org/la-reglementation/fiches-thematiques/assiette-des-contributions, mise à jour du 05/10/2026 :
  l'AGS suit la même assiette ; « Intermittents du spectacle et VRP multicartes : Le plafond est appliqué employeur
  par employeur. »

### Norme DSN 2026.1 (`../../norme-dsn/texte/`)

- `bloc-S21.G00.78.txt` l. 209-213 (p. 294) : « 02 - Assiette brute plafonnée », « 03 - Assiette brute
  déplafonnée », « 07 - Assiette des contributions d'Assurance Chômage ».
- `bloc-S21.G00.23.txt` l. 35-36 (p. 168) : « 920 - Autre assiette », « 921 - Assiette plafonnée ».

## Où le moteur s'en sert

- `paie/src/calcul.rs`, `calculer` (7.6) : pour un intermittent, `brut_plafonne` = min(brut, plafond),
  `brut_x115_plafonne`, `brut_chomage` = min(brut, 4 × plafond) ; `plafond_de_l_engagement` (la seule source du
  plafond, celle du bas de bulletin).
- `logiciel/donnees/baremes/2026.json` : `vieil_plaf` sur `brut_plafonne`, `autres_2` (FNAL) sur
  `brut_x115_plafonne`, `chomage` sur `brut_chomage` (artiste et technicien) ; DSN : base 02 = `brut_plafonne`, 049 sur
  `brut_x115_plafonne`, base 07 et 040/048/041 sur `brut_chomage`.
- `paie/src/dsn.rs`, `assiettes_ctp` / `valeur_ctp` : sous le qualifiant 921, l'assiette plafonnée de la population
  (312/921, 334/921, 100/921, 332/921) ; `assiette_declaree` pour une régularisation.

## Ce qui est une inférence, et ce qui manque

- **Plus une inférence depuis le lot V1n** : la majoration de 11,5 % du FNAL d'un intermittent s'applique à l'assiette
  plafonnée ; au-delà du plafond, c'est le plafond qu'elle majore (`brut_x115_plafonne` = plafond × 1,115) : lettre
  circulaire Acoss 2013-019, annexe 1, R2, et CSS L. 243-1-3, D. 243-0-2 (`../majoration-caisses-conges/LISEZ-MOI.md`).
- **Inférence** : « quatre fois le plafond mentionné à l'article L. 241-3 » se lit comme quatre fois le plafond de la
  période du contrat (R. 242-2, arrêté de 1975), et non quatre fois le plafond mensuel : l'annexe renvoie au plafond de
  L. 241-3, que R. 242-2 réduit ; la fiche Unédic dit « employeur par employeur » pour les intermittents.
- **`plafond.rs` suit l'arrêté à la lettre depuis la vague 1l** (10/10/2026, tranché par Léandre ;
  `prive/journal/norme/corrections/PASSATION-V1l.md`) : un artiste engagé moins de cinq jours (du début à la fin du
  contrat) a 12 × le plafond horaire par journée de travail datée ; sinon, et pour un technicien, le prorata de
  R. 242-2. **Inférence** : la « période d'engagement continu » est le contrat (ses bornes `debut` et `fin`). Voir
  aussi le BOSS, assiette générale, § 680, 690, 1090 et 1100 (`../boss/`) et le modèle de l'Urssaf
  (`../urssaf-mon-entreprise/`).
- **Vague 1o (10/10/2026, tranché par Léandre : le BOSS est opposable)** : hors de l'arrêté de 1975 (artiste engagé
  cinq jours ou plus, technicien), le plafond n'est plus le prorata calendaire de tout le contrat mais, **pour chaque
  paie**, le PSS mensuel proratisé sur les **jours ayant donné lieu à rémunération** dans la paie (les dates
  distinctes des lignes) : la tolérance du BOSS, assiette générale, § 1090 (`../boss/boss-assiette-generale-20260801.txt`,
  l. 1472-1483), avec la formule du § 700. Un jour rémunéré qui ne se lit pas refuse. Le permanent garde R. 242-2 sur
  sa période de paie. `prive/journal/norme/corrections/PASSATION-V1o.md`.
- **Reste à déposer** : la page Urssaf « artistes du spectacle, assiette plafonnée » (le conteneur reçoit une
  réponse vide) ; le BOSS (refuse le conteneur) ; le règlement général annexé à la convention du 15/11/2024, art. 49
  (permanents), déposé depuis : `../chomage-permanents/` (lot V2e). Le texte de la majoration de 11,5 % est déposé depuis : `../majoration-caisses-conges/`.

## Relevé du 11/10/2026 (séance « documents », D11 : l'assiette chômage d'un temps partiel)

- `unedic-fiche-assiette-des-contributions-20260922.pdf` (+ `.txt`) : la version PDF de la fiche thématique « Assiette
  des contributions » que la page sert (`https://www.unedic.org/la-reglementation/fiches-thematiques/assiette-des-contributions`,
  PDF daté du 22/09/2026, mis en ligne le 05/10/2026), relevée par le conteneur (unedic.org répond). Deux pages : principe
  d'identité d'assiette avec la Sécurité sociale, « l'assiette des contributions ne peut dépasser 4 fois le plafond mensuel
  de la sécurité sociale, soit 16 020 € au 1er janvier 2026 » (l. 25), « Intermittents du spectacle et VRP multicartes : le
  plafond est appliqué employeur par employeur » (l. 82). **Pas un mot sur le temps partiel.**
- Le règlement général annexé à la convention du 15/11/2024 (`../chomage-permanents/…txt`, l. 1778) et l'annexe VIII
  (`unedic-annexe8-convention-20241115.txt`, l. 1204) disent la même chose en droit : les contributions sont assises sur
  les rémunérations « dans la limite d'un plafond fixé à quatre fois le plafond mentionné à l'article L. 241-3 du code de
  la sécurité sociale ». **Il n'existe apparemment pas de circulaire Unédic propre au temps partiel** (recherche sur
  unedic.org le 11/10/2026 : rien sous ce titre ; la liste des circulaires n'est plus servie à l'ancienne adresse). Lecture :
  comme l'ANI Agirc-Arrco (`../agirc-arrco/`), l'Unédic renvoie au plafond de L. 241-3 *tel qu'appliqué* ; si l'employeur
  proratise le plafond d'un temps partiel (fiche net-entreprises 3369, règle par défaut), le plafond de 4 fois suit ;
  **inférence depuis le renvoi**, qu'aucun texte Unédic ne contredit. En pratique, pour un temps partiel sous 4 plafonds,
  la question ne change aucun montant.


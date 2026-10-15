# Le Bulletin officiel de la sécurité sociale (BOSS) — les pages déposées

Le BOSS (boss.gouv.fr) ne répond pas depuis le conteneur (connexion coupée) : ses pages sont imprimées en PDF par
Léandre depuis le poste et déposées ici, transcription à côté (`pdftotext -layout`). Nom : `boss-<rubrique>-<date de
la version en vigueur>`. La page « Montant net social » est rangée avec ses textes, dans `../montant-net-social/`.

## `boss-assiette-generale-20260801.pdf` — Règles d'assujettissement › Assiette générale

URL : `http://boss.gouv.fr/portail/sites/siteboss/accueil/regles-dassujettissement/assiette-generale.html`,
**version en vigueur au 01/08/2026**, imprimée le 10/10/2026. Six chapitres : affiliation ; assiette de la CSG et de
la CRDS (§ 290 : abattement de 1,75 % jusqu'à quatre plafonds annuels) ; assiette des cotisations ; forfait social ;
fait générateur ; **chapitre 6, le plafond de la sécurité sociale** (§ 650 à la fin).

Ce que le moteur en lit (chapitre 6) :
- § 680 : les valeurs 2026 (horaire **30 €**, journalier 220 €…) ; « La valeur horaire n'est utilisable que […] pour
  le calcul du plafond pour les artistes du spectacle engagés pour des périodes continues inférieures à cinq jours »
  (renvoi à l'arrêté du 24/01/1975, déposé dans `../arrete-1975-artistes-du-spectacle/`, et à L. 241-3, D. 242-16 à
  D. 242-19).
- § 690 à 750 : l'ajustement du plafond « à proportion des jours couverts par le contrat de travail », jours
  calendaires moins les absences sans maintien de salaire ; entrée ou sortie en cours de mois (R. 242-2).
- § 760 à 790 : temps partiel (proratisation à la durée contractuelle, sur option), forfait jours. **§ 760-770
  appliqués depuis le lot V4b** (l. 1031-1072) : le plafond d'un salarié à temps partiel « peut être calculé par
  l'employeur » à due proportion de sa durée contractuelle, « rapportée à la durée légale du travail (ou
  conventionnelle si elle est inférieure) », et « le salarié à temps partiel et l'employeur ont la possibilité de
  renoncer à cet ajustement » ; le rapport « ne peut pas conduire à un résultat supérieur à la valeur mensuelle du
  plafond ». Le choix est dans la fiche entreprise (`plafond_temps_partiel`), et S21.G00.40.084 le déclare en 2027 :
  `../plafond-temps-partiel/LISEZ-MOI.md`.
- § 660 : le plafond se détermine à chaque paie et se régularise (progressivement ou en fin d'année).
- **§ 1090** (l. 1472-1483) : les artistes du spectacle « ne sont pas éligibles à la réduction du plafond » ; non
  mensualisés, le plafond « dépend uniquement de la périodicité de la paie » ; « Toutefois, il est admis que : pour les
  salariés intermittents, le plafond soit proratisé pour ne tenir compte que des jours ayant donné lieu à
  rémunération ». **Tolérance appliquée depuis la vague 1o** (10/10/2026, tranché par Léandre : le BOSS est opposable,
  et le but n'est pas de faire payer le plus aux compagnies) : tout intermittent hors de l'arrêté de 1975 (artiste
  engagé cinq jours ou plus, technicien) a, pour chaque paie, le PSS mensuel × jours rémunérés du mois / jours du mois
  (la formule du § 700) ; les jours rémunérés sont les dates distinctes des lignes de la paie, et une journée dite et
  non datée refuse. La vague 1l l'avait écartée (« une tolérance, pas le texte ») et appliquait le prorata calendaire
  de tout le contrat.
- § 1100 : « 12 fois le plafond horaire par jour d'engagement » pour un artiste engagé moins de cinq jours ; le moteur
  suit l'arrêté (« par journée de travail accomplie »).

Où le moteur s'en sert : `atelier/sources/paie/src/plafond.rs` (le plafond d'un contrat), `calcul::plafond_de_l_engagement`
(lots 1i, 1l et 1o, `prive/journal/norme/corrections/`).

## `boss-frais-professionnels-20261013.pdf` — Autres éléments de rémunération › Frais professionnels

URL : `https://boss.gouv.fr/portail/accueil/autres-elements-de-remuneration/frais-professionnels.html`, **version en vigueur
au 01/01/2026**, imprimée par Léandre le 13/10/2026 à 21 h 30 (47 pages), transcription `pdftotext -layout` à côté. Dix
chapitres ; le moteur lit le **chapitre 9, « Déduction forfaitaire spécifique pour frais professionnels »** (§ 2120 à 2330,
l. 2614-2956 de la transcription). Les règles de la DFS elles-mêmes (taux, plafond, option, annexe 2) sont dans l'arrêté
du 4 septembre 2025, déposé dans `../dfs/` ; le BOSS y ajoute la doctrine et les tolérances ci-dessous.

Ce qu'on en lit (chapitre 9) :
- **§ 2120** : professions de l'article 5 de l'annexe IV du CGI (rédaction au 31/12/2000) ; plafond **7 600 €** par salarié
  et par année civile ; les secteurs à taux personnalisés, dont **le spectacle vivant**, suivent la section 3.
- **§ 2130-2140** : la DFS suppose des frais effectivement supportés, l'employeur garde les justificatifs. **§ 2310** : pour
  les huit secteurs, dont le spectacle vivant, **par tolérance depuis le 01/01/2023, le bénéfice est admis même sans frais
  réellement supportés** (l'exemple cité : la caisse de congés payés qui applique la DFS aux indemnités de congés). Les
  autres conditions restent dues.
- **§ 2150, absence** : la DFS ne s'applique « que sur la rémunération correspondant à un travail effectif » ; un mois
  complet d'absence (maladie, congés) n'ouvre pas la DFS pour ce mois. **À confronter au moteur** : un intermittent au
  cachet n'a pas d'absence ; un salarié au mois sous DFS est refusé aujourd'hui (`calcul::dfs_applicable`), la règle
  servira le jour où il sera porté.
- **§ 2160, plusieurs employeurs** : la limite de 7 600 € « peut être appréciée séparément, au niveau de chaque
  employeur » (tolérance, sauf abus manifeste). C'est ce que le moteur fait (`calcul::dfs_deja_deduite`, par salarié et par
  compagnie) : le point 5 du `LISEZ-MOI` de `../dfs/` (le cumul d'avant le logiciel, chez un autre employeur) n'est donc pas
  un manque.
- **§ 2180-2210, l'option** : convention ou accord collectif, ou accord du CSE ; à défaut, le consentement de chaque
  salarié, informé des conséquences sur ses droits, recueilli chaque année ; le silence vaut accord ; la décision du salarié
  prend effet l'année civile suivante. **§ 2330, huit secteurs** : un consentement à durée indéterminée couvre toute la
  période de transition ; pour un salarié embauché depuis le 01/01/2023 sans accord collectif, le consentement est requis,
  et l'employeur peut proposer qu'il vaille pour les contrats ultérieurs chez lui. Le moteur : `dfs_salarie` de la fiche
  (`contrat` / `consentement` / `refus`, `calcul.rs` l. 553-660).
- **§ 2220-2230, assiette minimale** : la DFS ne ramène pas l'assiette d'une paie sous le SMIC et ses accessoires légaux ;
  **les artistes hors du champ du SMIC (pas de durée du travail) n'y sont pas soumis**. Le moteur : l'assiette minimum ne
  vaut que pour les contrats à l'heure ou au mois (refusés sous DFS aujourd'hui).
- **§ 2240-2290, non-cumul** : les remboursements de frais (réels ou forfaitaires) et prises en charge directes entrent dans
  l'assiette avant l'abattement, sauf l'annexe 2 de l'arrêté (défraiements de tournée des artistes dramatiques, lyriques,
  chorégraphiques et régisseurs de théâtre ; allocations de saison ; frais des chefs d'orchestre, musiciens et choristes en
  déplacement…). Les outils de travail (véhicule, ordinateur) ne sont pas une prise en charge de frais. **MAIS § 2320, huit
  secteurs dont le spectacle vivant : « par tolérance », depuis le 01/01/2023 et jusqu'à l'extinction, « l'ensemble des
  remboursements de frais professionnels définis par l'arrêté du 4 septembre 2025 peut faire l'objet d'un cumul avec la
  déduction forfaitaire spécifique » ; leur intégration dans l'assiette avant l'abattement « n'est pas obligatoire ».** Le
  rescrit BOSS-RES-000010 borne la tolérance : une indemnité de transport conventionnelle n'est pas un remboursement de
  frais au sens de l'arrêté, elle se réintègre dès le premier euro. **Conséquence pour le moteur, à porter (décision par le
  texte, pas par Léandre)** : le refus d'un remboursement de frais hors annexe 2 pour un salarié sous DFS
  (`calcul::dfs_applicable`, point 4 du `LISEZ-MOI` de `../dfs/`) n'a plus de raison d'être pour un remboursement de frais
  au sens de l'arrêté : il reste hors de l'assiette abattue, comme ceux de l'annexe 2 ; seule une indemnité conventionnelle
  (qui n'est pas un frais) se réintègre. Le BOSS est opposable (Léandre, 10/10/2026, § 1090 de l'assiette générale). **Porté par le
  lot DF2 (14/10/2026)** : le barème le dit (`deduction_forfaitaire.frais_cumulables`, 2025 et 2026, son `_frais_cumulables`
  cite § 2300, 2310, 2320 et 2325), `calcul::dfs_applicable` le lit ; une rubrique de nature `frais` (catalogue ou
  compagnie) reste hors de l'assiette abattue ; test : `tests/calcul/dfs.rs`,
  `un_remboursement_de_frais_sous_dfs_reste_hors_de_l_assiette_abattue_par_la_tolerance_du_boss`. Le rescrit
  BOSS-RES-000010 est dans la page déposée (§ 2325, l. 2923-2934) : pas de dépôt à part.
- **§ 2300, le calendrier du spectacle vivant** : à 20 % → −1 point en 2024 et 2025, −2 points de 2026 à 2028, −3 points de
  2029 à 2031, zéro au 01/01/2032 ; à 25 % → −2 points en 2024 et 2025, −3 points de 2026 à 2031, zéro au 01/01/2032. Donc
  **2026 : 16 % et 18 %** ; 2027 : 14 % et 15 %. Même calendrier que l'arrêté du 4 septembre 2025 (annexe 1), que les barèmes
  2025 et 2026 suivent (`_professions`, `_plafond` de `logiciel/donnees/baremes/<année>.json`).
- Hors chapitre 9, pour mémoire : § 760 (chapitre 4) et § 990, le non-cumul de la DFS avec la prise en charge des
  transports et du carburant, et sa dérogation pour les 50 % obligatoires des titres de transport.

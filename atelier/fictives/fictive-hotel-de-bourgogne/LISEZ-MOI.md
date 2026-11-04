# La Compagnie de l'Hôtel de Bourgogne : la fictive de référence avec des permanents

Une compagnie de théâtre, **fictive** (tout ce qui suit est inventé ; NIR, SIRET et IBAN ont une forme valide et la marque
d'un numéro inventé). Convention 1285 (entreprises artistiques et culturelles), trois permanents en CDI et cinq
intermittents, venue d'un logiciel précédent (sPAIEctacle) au 1er janvier 2026 avec ses papiers de 2025.

## Ce que ce dossier porte

| | |
|---|---|
| `papiers-2025/` | ce qu'elle importe : les 110 bulletins de 2025 de l'ancien logiciel, un PDF par bulletin (`<mois>-<code du salarié>-<rang>.pdf`), et la DSN de décembre 2025 qu'il a déposée (`DSN_Decembre2025.dsn`) |
| `gestes.jsonl` | **le parcours de référence de 2026** : un scénario (`moteur/src/atelier/scenario.rs`) écrit comme l'administratrice l'aurait joué, par les routes seulement (celles de l'écran et du LLM), du 2 janvier 2026 au 30 janvier 2027 |
| `description.json` | la compagnie, ses salariés et ses bulletins de 2025, dont `moteur-atelier fabriquer-bulletins description.json papiers-2025 --gabarit spaiectacle` refait les PDF |

L'année 2025 a été jouée dans PuppetPaie, par les routes, comme l'administratrice l'aurait jouée (les trois CDI repris à
leurs vrais débuts, chaque série d'intermittents validée la veille, DPAE, contrats, documents, une DSN par mois) ; ses
papiers en sont tirés :

- **les bulletins** : un par paie (un contrat, un mois), ses lignes telles que PuppetPaie les a payées (rubrique, quantité,
  taux), sa période, ses jours travaillés, son numéro d'objet, ses cotisations, son net imposable et l'impôt prélevé,
  tels que PuppetPaie les a calculés : bulletins et DSN de décembre disent la même chose ;
- **la DSN de décembre 2025** : celle que PuppetPaie a fabriquée et déposée en jouant décembre, octet pour octet (fins de
  ligne CRLF comprises), sauf l'identité du logiciel émetteur (S10.G00.00.001 à .003 : « GHS », « sPAIEctacle »,
  « 12.4.0 »). Une compagnie n'apporte souvent que sa dernière DSN : c'est une **donnée** fixe, que rien du moteur ne
  fabrique et que l'import relit telle quelle.

## Ce que l'import en fait

Une compagnie ajoutée « D'un logiciel précédent » au 02/01/2026 qui importe `papiers-2025/` reprend : la fiche de
l'entreprise et les huit salariés (noms accentués), deux spectacles par leur numéro d'objet (titre proposé d'après les
bulletins), 74 contrats d'intermittents et 3 engagements de permanents (CH1.22.01 du 01/03/2022, MA2.22.01 du
05/09/2022, CL3.24.01 du 08/01/2024, les numéros de la DSN de décembre), sans écart de compteurs ni de nets. Les 11 bulletins de
décembre complètent les paies que la DSN déclare. Janvier 2026 continue les trois CDI sous leurs numéros. « Pour
démarrer » demande ce que ni les bulletins ni la DSN ne portent : le compte Urssaf, la santé au travail, qui signe, la
date de la première embauche, l'effectif de 2025, la taxe sur les salaires, les titres des spectacles, et les congés acquis des trois
permanents.

## La compagnie

- **Compagnie de l'Hôtel de Bourgogne**, association loi 1901 fondée en 2021, 12 rue Mauconseil, 75001 Paris ; SIRET
  123 456 824 00010 ; APE 9001Z ; compte Urssaf 117000000015 ; licence PLATESV-R-2022-001284 ; convention 1285.
  Subventionnée par la Ville de Paris et la DRAC Île-de-France ; n'adhère à aucun syndicat d'employeurs ; pas de comité
  social et économique (procès-verbal de carence).
- Contact : contact@hotel-bourgogne.exemple.invalid, 01 40 00 00 12. Hortense Champmeslé, administratrice, signe les
  contrats et les déclarations (par délégation de la présidente).
- Banque du Marais, agence de la rue Étienne-Marcel ; BIC **BDMRFR2AXXX**, IBAN **FR76 1234 5678 9000 0777 8880 147**.
- **Santé au travail** : service interentreprises de Paris, code **MT075**.
- **Congés Spectacles** : numéro d'adhérent **1842960731B**.
- **Prévoyance et santé des permanents** : Audiens (P0983), « PREVOYANCE CCNEAC » et « SANTE CCNEAC ».
- **Taxe sur les salaires** : la billetterie et les cessions sont soumises à la TVA ; en 2025, 72 % des recettes et
  produits (subventions comprises) y étaient soumis ; la compagnie dépasse la franchise en base.
- **Première embauche** : Hortense Champmeslé, en CDI, le **1er mars 2022**.
- **Effectif de 2025** : **3,65**, l'effectif annuel moyen que l'Urssaf a calculé de ses DSN de 2025 (compte rendu
  « Effectifs », nature 118, reçu le 16/02/2026 dans l'année jouée).
- **Congés payés à la reprise**, au 1er janvier 2026, tels que le dernier bulletin de décembre 2025 de l'ancien
  logiciel les porte : aucun jour pris ; sur la période passée (juin 2024 à mai 2025), 30 jours acquis et 30 qui
  restent (les trois sont là depuis avant le 1er juin 2024 : douze mois à 2,5 jours ouvrables, C. trav. L3141-3) ; sur
  la période en cours (juin à décembre 2025), 17,5 jours acquis. Le brut de chaque période, douze et sept salaires
  mensuels aux montants de l'équipe ci-dessous :

  | | période passée | période en cours |
  |---|---|---|
  | Champmeslé | 47 321,04 € | 27 603,94 € |
  | Marivaux | 29 848,68 € | 17 411,73 € |
  | Clairon | 23 295,36 € | 13 588,96 € |
- **Spectacles** : Les Fausses Confidences (Marivaux), 256Z00921041 ; Le Barbier de Séville (Beaumarchais),
  256Z00921042.

## L'équipe

Tous de nationalité française, nés en France.

| | nom | né(e) le, à | NIR | emploi |
|---|---|---|---|---|
| CDI, cadre | Champmeslé Hortense | 12/06/1976 à Rouen (76) | 2 76 06 76 999 001 42 | administratrice de production |
| CDI, non cadre | Marivaux Gaspard | 03/02/1984 à Lille (59) | 1 84 02 59 999 002 94 | régisseur |
| CDI, non cadre, temps partiel | Clairon Philomène | 21/09/1991 à Nantes (44) | 2 91 09 44 999 003 23 | chargée de production |
| intermittent | Molière Anselme | 14/01/1980 à Lyon (69) | 1 80 01 69 999 004 02 | comédien |
| intermittente | Dorval Séraphine | 08/11/1987 à Toulouse (31) | 2 87 11 31 999 005 18 | comédienne |
| intermittent | Lemaître Octavien | 27/05/1975 à Bordeaux (33) | 1 75 05 33 999 006 20 | comédien |
| intermittent | Lekain Barnabé | 30/08/1982 à Dijon (21) | 1 82 08 21 999 007 48 | technicien lumière |
| intermittente | Talma Capucine | 16/03/1993 à Strasbourg (67) | 2 93 03 67 999 008 76 | technicienne de plateau |

- **Hortense Champmeslé** : temps plein (151,67 h par mois), depuis le 01/03/2022, 3 943,42 € brut par mois (26,00 € de
  l'heure).
- **Gaspard Marivaux** : temps plein, depuis le 05/09/2022, 2 487,39 € (16,40 € de l'heure).
- **Philomène Clairon** : temps partiel à 80 % (28 h par semaine, 121,33 h par mois ; lundi, mardi, jeudi et vendredi,
  de 9 h à 16 h 30), depuis le 08/01/2024, 1 941,28 € (16,00 € de l'heure).
- Cachet de représentation : Molière 175 €, Dorval 165 €, Lemaître 170 € ; un service de répétition de 4 heures, 64 €,
  deux par jour. Lekain : 17,50 € de l'heure, sept heures par jour de jeu. Talma : 17,50 € de l'heure, huit heures
  par jour de jeu (le moteur refuse un service pour un technicien) : les textes de la 1285 ne définissent le service que pour les artistes
  (`atelier/reference/sources/chomage-intermittents/kali-1285-organisation-travail-repetitions-representations-20261020.md`), le titre VI ne
  compte les techniciens qu'en heures (`atelier/reference/sources/kali-1285-titre-vi-duree-du-travail-20261102.md`).
- Numéros Congés Spectacles : Molière M8001423, Dorval D8711207, Lemaître L7505316, Lekain L8208554, Talma T9303098.
- Courriels : `<prénom>.<nom>@exemple.invalid`, sans accents (hortense.champmesle@exemple.invalid…).

## L'année 2025

Une représentation par jour, le soir, à Paris ; chacun rentre dormir chez soi. M = Molière, D = Dorval, L = Lemaître,
K = Lekain (lumière), T = Talma (plateau). 74 contrats d'intermittents ; les trois permanents sont payés chaque mois,
sans congé, arrêt ni rupture.

| dates | quoi | qui |
|---|---|---|
| mar. 14 au ven. 17 janvier | répétitions, 4 jours (Les Fausses Confidences) | M, D, L |
| ven. 24, sam. 25, dim. 26 janvier | Les Fausses Confidences, 3 représentations | M, D, L, K, T |
| ven. 7, sam. 8, dim. 9 février | Le Barbier de Séville, 3 représentations | M, D, L, K |
| ven. 21, sam. 22 février | Le Barbier de Séville, 2 représentations | M, L, K |
| ven. 14, sam. 15, dim. 16, ven. 21, sam. 22 mars | Le Barbier de Séville, 5 représentations | M, D, L, K, T |
| ven. 4, sam. 5, dim. 6 avril | Les Fausses Confidences, 3 représentations | M, D, L, K |
| mar. 22, mer. 23, jeu. 24 avril | répétitions, 3 jours (Le Barbier de Séville) | D, L |
| ven. 16, sam. 17, dim. 18 mai | Les Fausses Confidences, 3 représentations | M, D, L, K, T |
| ven. 30, sam. 31 mai | Le Barbier de Séville, 2 représentations | D, L, K |
| ven. 6 au dim. 8, ven. 13 au dim. 15 juin | Le Barbier de Séville, festival, 6 représentations ; défraiement au forfait de 25 € par jour | M, D, L, K, T |
| ven. 4, sam. 5, dim. 6 juillet | Les Fausses Confidences, 3 représentations | M, D, L, K |
| ven. 11, sam. 12 juillet | Le Barbier de Séville, 2 représentations | M, L, K |
| lun. 8 au ven. 12 septembre | répétitions, 5 jours (Les Fausses Confidences) | M, D, L |
| ven. 19, sam. 20, dim. 21 septembre | Les Fausses Confidences, 3 représentations | M, D, L, K, T |
| ven. 10, sam. 11, dim. 12 octobre | Le Barbier de Séville, 3 représentations | M, D, L, K |
| ven. 24, sam. 25 octobre | Le Barbier de Séville, 2 représentations | M, D, K |
| ven. 7, sam. 8, dim. 9, ven. 14, sam. 15 novembre | Les Fausses Confidences, 5 représentations | M, D, L, K, T |
| ven. 5, sam. 6, dim. 7 décembre | Le Barbier de Séville, 3 représentations | M, D, L, K, T |
| ven. 19, sam. 20 décembre | Les Fausses Confidences, 2 représentations | M, L, K |

Août : les permanents seuls.

## L'année 2026 : le rejeu et la livraison

- **Le rejeu, preuve commune en ligne de commande** : `moteur/tests/rejeu.rs`,
  `la_fictive_de_l_hotel_de_bourgogne_tient_son_annee_et_se_livre` (`cargo nextest run --cargo-profile essais -E
  'binary(rejeu) & test(hotel)'`, une trentaine de secondes). La fictive se livre au 02/11/2026 comme « Publier » la
  livre ; aucune route ne refuse ; l'horloge, ce jour-là, ne trouve rien à faire ; les huit cartes de décembre sont
  écrites et non validées ; passée au 31/01/2027, l'horloge seule valide décembre, déclare, envoie et clôt 2026 ;
  chaque mois a ses étapes faites ; chaque DSN déclare chaque salarié au taux transmis par la DGFiP (type 01) ; les
  paies, les DSN et les dépôts se comparent à l'oracle `moteur/tests/donnees/fictives/fictive-hotel-de-bourgogne.json`
  (`PUPPETPAIE_REECRIRE_ORACLES=1`).
- **La fictive livrée** : `moteur maj --marquer-fictives --logiciel logiciel` (le flux « Publier ») la refait dans
  `fictives/fictive-hotel-de-bourgogne`, rejouée jusqu'au jour de la publication, comme le Fil de Fer ; à la main,
  `moteur-atelier scenario livrer atelier/fictives/fictive-hotel-de-bourgogne fictives/fictive-hotel-de-bourgogne
  [--jusqu-au AAAA-MM-JJ]`. Les paies de janvier 2027 des deux CDI attendent le barème de 2027.

## Le parcours, mois par mois

- **2 janvier 2026** : la compagnie s'ajoute « D'un logiciel précédent », importe ses papiers de 2025, remplit « Pour
  démarrer » (compte Urssaf, santé au travail, première embauche, qui signe, taxe sur les salaires, l'effectif de 2025,
  les congés à la reprise des trois CDI, les titres des spectacles), crée Les Plaideurs (256Z00921043), fait la revue
  des spectacles et valide les trois CDI de janvier.
- **Chaque série** : les engagements validés quelques jours avant le premier jour, la DPAE (test puis réel), les
  contrats à la signature ; la DSN déposée entre le 1er et le 5 du mois suivant (test puis réel), puis les documents.
- **Mars** : l'arrêt maladie de Marivaux (dernier jour travaillé le vendredi 6, prescrit du 9 au 22), su le 9 ; son
  signalement d'arrêt (test puis réel) ; la prolongation au 3 avril, reçue le 20 ; le 30, les indemnités journalières
  du relevé de la CPAM : **40,89 € brut, 38,15 € net par jour** (la moitié du salaire journalier de base, 3 × 2 487,39 /
  91,25, CSS R323-4 ; net de CSG et CRDS, 6,7 %). La compagnie est subrogée et maintient le salaire (1285, X.4.6).
- **Mai** : la tournée de Bretagne, le 13 à Rennes, le 14 à Lorient, les 15 et 16 à Quimper ; huit repas (le dîner du
  13, déjeuner et dîner des 14, 15 et 16, le déjeuner du 17) et quatre nuitées par personne, au tarif de la convention,
  chacun à sa date ; le repas du 17 ne fait pas un jour du contrat. Les 8 heures supplémentaires de Marivaux, saisies
  le 18.
- **Juin** : la démission de Clairon, remise le 2 (préavis du 3 juin au 2 juillet, son dernier jour).
- **Juillet** : le congé de Champmeslé du 20 au 31 ; le 3, le signalement de fin du contrat de Clairon (test puis réel)
  et ses documents de fin.
- **Août** : sans intermittent ; Champmeslé en congé du 1er au 7, Marivaux du 10 au 28.
- **Septembre** : l'augmentation de Champmeslé, décidée le 7, à 4 100,00 € au 1er septembre, et son avenant à la
  signature ; le 29, les engagements d'Orléans (9, 10 et 11 octobre) et leurs contrats.
- **Octobre** : le 2, la quatrième représentation d'Orléans (le 17) : les cinq engagements partis comptent une date de
  plus, et leurs avenants partent à la signature.
- **Janvier 2027** : la DSN de décembre, la revue des spectacles de 2027, la clôture de 2026 le 29.

## Le calendrier 2026

Une représentation par jour, le soir ; deux services de répétition par jour, à la salle de la compagnie (Paris). Chacun
rentre dormir chez soi, sauf pendant la tournée de mai.

| dates | quoi | où | qui |
|---|---|---|---|
| lun. 12 au ven. 16 janvier | répétitions, Le Barbier de Séville | Paris | M, D, L |
| ven. 23 au dim. 25 janvier | Le Barbier de Séville, 3 représentations | Paris, théâtre du Marais-Neuf | M, D, L, K, T |
| ven. 30, sam. 31 janvier | Le Barbier de Séville, 2 représentations | Vitry-sur-Seine | M, L, K |
| ven. 6 au dim. 8 février | Les Fausses Confidences, 3 représentations | Versailles | M, D, L, K, T |
| ven. 20, sam. 21 février | Les Fausses Confidences, 2 représentations | Saint-Denis | M, L, K |
| ven. 13 au dim. 15 mars | Le Barbier de Séville, 3 représentations | Paris, théâtre du Marais-Neuf | M, D, L, K, T |
| ven. 20, sam. 21 mars | Le Barbier de Séville, 2 représentations | Créteil | M, D, L, K, T |
| ven. 10 au dim. 12 avril | Les Fausses Confidences, 3 représentations | Paris, théâtre du Marais-Neuf | M, D, L, K, T |
| lun. 20 au ven. 24 avril | répétitions, Les Plaideurs | Paris | M, D, L |
| mer. 13 au sam. 16 mai | Les Fausses Confidences, tournée de Bretagne | Rennes, Lorient, Quimper | M, D, L, K, T |
| ven. 22, sam. 23 mai | Le Barbier de Séville, 2 représentations | Paris, théâtre du Marais-Neuf | M, D, K |
| ven. 5 au dim. 7, ven. 12 au dim. 14 juin | Le Barbier de Séville, festival, 6 représentations | Paris, parc de Bagatelle | M, D, L, K, T |
| ven. 3 au dim. 5 juillet | Les Fausses Confidences, 3 représentations | Avignon | M, D, L, K, T |
| ven. 10, sam. 11 juillet | Les Fausses Confidences, 2 représentations | Villeneuve-lès-Avignon | D, L, K |
| lun. 7 au ven. 11 septembre | répétitions, Les Plaideurs | Paris | M, D, L |
| ven. 18 au dim. 20 septembre | Les Fausses Confidences, 3 représentations | Paris, théâtre du Marais-Neuf | M, D, L, K, T |
| ven. 9 au dim. 11, sam. 17 octobre | Les Plaideurs, création, 4 représentations | Orléans | M, D, L, K, T |
| ven. 23, sam. 24 octobre | Le Barbier de Séville, 2 représentations | Paris, théâtre du Marais-Neuf | M, L, K |
| ven. 6 au dim. 8, ven. 13, sam. 14 novembre | Les Plaideurs, 5 représentations | Paris, théâtre du Marais-Neuf | M, D, L, K, T |
| ven. 4 au dim. 6 décembre | Les Fausses Confidences, 3 représentations | Rouen | M, D, L, K, T |
| ven. 18, sam. 19 décembre | Les Plaideurs, 2 représentations | Paris, théâtre du Marais-Neuf | M, D, K |

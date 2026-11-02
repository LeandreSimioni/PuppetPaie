# La Compagnie du Fil de Fer : la fictive de référence légère

Une petite compagnie de marionnettes, **fictive** (tout ce qui suit est inventé ; NIR, SIRET et IBAN ont une forme valide
et la marque d'un numéro inventé). Convention 3090, intermittents au cachet seulement, venue d'un logiciel précédent
(sPAIEctacle) fin 2025, une année 2026.

## Ce que ce dossier porte, et ce qui en sort

| | |
|---|---|
| `gestes.jsonl` | **le parcours de référence** : un scénario (`moteur/src/atelier/scenario.rs`) écrit comme l'administratrice l'aurait joué, par les routes seulement (celles de l'écran et du LLM) |
| `papiers-2025/` | ce qu'elle importe : les quinze bulletins de septembre à décembre 2025 et la DSN de décembre 2025 de l'ancien logiciel |
| `description.json` | la compagnie, ses salariés et ses bulletins de 2025, dont `moteur-atelier fabriquer-bulletins description.json papiers-2025 --gabarit spaiectacle` refait les PDF |

La DSN de décembre 2025 est une **donnée** : c'est un fichier que l'ancien logiciel a déposé, fixe, que rien du moteur ne
fabrique et que l'import relit tel quel.

- **Le rejeu, preuve commune en ligne de commande** : `moteur/tests/rejeu.rs`, `la_fictive_du_fil_de_fer_tient_son_annee_et_se_livre`
  (`cargo nextest run --cargo-profile essais -E 'binary(rejeu) & test(fil_de_fer)'`, une dizaine de secondes). La
  fictive se livre au 02/11/2026 comme « Publier » la livre ; aucune route ne refuse ; l'horloge, ce jour-là, ne trouve
  rien à faire ; passée au 31/01/2027, elle seule valide décembre, déclare, envoie et clôt 2026 ; chaque mois a ses
  étapes faites ; chaque DSN déclare chaque salarié au taux transmis par la DGFiP (type 01) ; les paies, les DSN et les
  dépôts se comparent à l'oracle `moteur/tests/donnees/fictives/fictive-fil-de-fer.json` (`PUPPETPAIE_REECRIRE_ORACLES=1`).
- **La fictive livrée** : `moteur maj --marquer-fictives --logiciel logiciel` (le flux « Publier ») la refait dans
  `fictives/fictive-fil-de-fer`, rejouée jusqu'au jour de la publication ; à la main, `moteur-atelier scenario livrer
  atelier/fictives/fictive-fil-de-fer fictives/fictive-fil-de-fer [--jusqu-au AAAA-MM-JJ]`. Des gestes d'après ce jour,
  seules les saisies s'écrivent (les cartes, non validées ; les fiches) : sur le poste, seul le temps passe, et
  l'horloge de la fictive valide, envoie et dépose à leur date. Un seul chemin, `scenario::livrer`, sert le rejeu et la
  livraison. Ce que la personne fait elle-même en janvier 2027 (la revue des spectacles, le FCAP-SVP) lui reste.
- **Le parcours court à l'écran** : `atelier/outils/banc/fil-de-fer.js` (livrée au 02/11/2026 : la DSN d'octobre
  déposée, la carte de décembre de Solène Marchetot validée, dans la page).

## Le parcours, mois par mois

- **5 janvier 2026** : la compagnie s'ajoute « D'un logiciel précédent », importe ses papiers de 2025, remplit « Pour
  démarrer » (compte Urssaf, santé au travail AST71, Congés Spectacles, signataire, première embauche, banque, taxe sur
  les salaires, la fiche d'Armand Tessier, les courriels, les titres des deux spectacles), fait la revue des spectacles.
- **Chaque mois joué** : les engagements validés quelques jours avant le premier cachet, la DPAE (test puis réel), les
  contrats à la signature ; la DSN déposée après le versement du dernier jour du mois (test puis réel, entre le 2 et le
  5 du mois suivant), puis les documents aux salariés. Juillet, août, octobre et novembre n'ont pas d'engagement : leur
  DSN part sans salarié.
- **Repas** à chaque jour de jeu loin de chez soi, **nuitées** à Roanne et Montbrison (7 au 9 avril) et à Nevers (18 et
  19 septembre), au tarif de la convention. Basile Ferrand, technicien, dort hors de chez lui en avril sans justificatif
  (la part de la nuitée au-delà de la limite du BOSS entre dans le brut) et en septembre avec « justificatif conservé ».
- **Le FCAP-SVP** 2026 payé (le minimum) et coché en février 2026 ; celui de 2027, calculé sur 2026, en janvier 2027.
- **Janvier 2027** : la DSN de décembre, la revue des spectacles de 2027, la clôture de 2026 le 29.

## Le calendrier 2026

Chaque ligne est un cachet par personne et par jour de jeu ou de répétition (51 cachets en tout). Les représentations
sont une par jour, le soir, sauf mention contraire. Qui joue : S = Solène, B = Basile, J = Joséphine, O = Octave,
A = Armand.

| dates | quoi | où | qui | cachets |
|---|---|---|---|---|
| lun. 19 et mar. 20 janvier | répétitions (reprise de Mademoiselle Poussière), salle de l'atelier | Chalon-sur-Saône | S, B | 4 |
| ven. 6, sam. 7, dim. 8 février | Le Grand Fil, 3 représentations | Dole, salle Jean-Moulin | S, B | 6 |
| ven. 13 et sam. 14 mars | Mademoiselle Poussière, 2 représentations | Autun, théâtre de la Halle | S, B, O, A | 8 |
| mar. 7 et mer. 8 avril | Le Grand Fil, 2 représentations | Roanne, salle des Fêtes | S, B | 4 |
| jeu. 9 et ven. 10 avril | Le Grand Fil, 2 représentations | Montbrison, centre culturel | S, B | 4 |
| lun. 11 et mar. 12 mai | Mademoiselle Poussière, 2 représentations | Lons-le-Saunier, théâtre municipal | S, B, J | 6 |
| ven. 12, sam. 13, dim. 14 juin | Le Grand Fil, festival, 3 représentations | Mâcon, parc des Expositions | S, B | 6 |
| ven. 18, sam. 19, dim. 20 septembre | Mademoiselle Poussière, 3 représentations | Nevers, théâtre de la Maison de la culture | S, B, J | 9 |
| ven. 4 et sam. 5 décembre | Le Grand Fil, 2 représentations | Dijon, théâtre des Feuillants | S, B | 4 |

La tournée d'avril (Roanne puis Montbrison, du mardi 7 au vendredi 10) se fait sans rentrer à Chalon : l'équipe dort sur
place les nuits du 7, du 8 et du 9 avril. À Nevers (18 au 20 septembre), l'équipe dort sur place les nuits du 18 et du 19.
Pour tous les autres lieux, chacun rentre dormir chez soi le soir même (Joséphine à Dijon, Octave à Mâcon, les autres à
Chalon). Le Grand Fil se joue avec Solène en scène et Basile en régie ; Mademoiselle Poussière ajoute Joséphine en scène,
Octave et Armand pour la musique et le son quand ils sont notés.

Le contrat d'un salarié est un engagement pour un spectacle : un même salarié peut avoir plusieurs engagements dans le
mois, un par série de représentations.

## Ce que la compagnie défraie

La compagnie défraie ses salariés : **les repas** (chaque jour de jeu ou de répétition passé loin de chez soi, sur place
à midi ou le soir) et **les nuitées** quand l'équipe dort hors de chez elle, **au tarif de la convention collective**
(les montants sont à chercher dans la convention). Elle ne défraie pas autre chose.

## Ce que la compagnie sait d'elle-même (et qui ne figure pas sur les bulletins)

- **Service de santé au travail** : la compagnie adhère à l'association de santé au travail de Saône-et-Loire ; le code
  qui figure sur son attestation d'adhésion est **AST71**.
- **Congés Spectacles** : le numéro d'adhérent de la compagnie à la caisse des Congés Spectacles (celui des courriers
  d'Audiens) est **2840517390C**.
- **TVA et taxe sur les salaires** : la compagnie est une association de marionnettistes sous la franchise en base : elle
  n'est **pas assujettie à la TVA** et ne facture pas de TVA à ses organisateurs.
- **Première embauche** : la compagnie n'avait jamais employé personne avant ses premiers cachets de septembre 2025 ;
  sa toute première embauche est celle de ses trois premiers bulletins, **le 12 septembre 2025** (Le Grand Fil).
- **Qui signe** : Léonie Garrel, présidente de l'association et administratrice bénévole, signe les contrats et les
  déclarations.
- **Nationalité** : les cinq salariés sont de nationalité française et nés en France (les lieux de naissance sont dans le
  tableau ci-dessus).
- **Courriels des salariés** (pour leur envoyer contrats et documents) :

| salarié | courriel |
|---|---|
| Marchetot Solène | solene.marchetot@exemple.invalid |
| Ferrand Basile | basile.ferrand@exemple.invalid |
| Albaret Joséphine | josephine.albaret@exemple.invalid |
| Delmas Octave | octave.delmas@exemple.invalid |
| Tessier Armand | armand.tessier@exemple.invalid |

- **Banque** : le compte de la compagnie (IBAN ci-dessus) est domicilié au Crédit du Fil, agence de Chalon ; son BIC est
  **FDFRFR2AXXX**.
- **Papiers de décembre 2025** : la déclaration mensuelle que l'ancien logiciel a déposée pour décembre 2025 est dans
  `papiers-2025/` avec les bulletins (`DSN_Decembre2025.dsn`).

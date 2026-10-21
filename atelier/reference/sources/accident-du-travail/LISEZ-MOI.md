# La déclaration d'accident du travail ou de trajet (DAT) — les pages et le formulaire (relevés le 15/10/2026)

La déclaration d'accident est **hors DSN** : l'employeur la fait à la caisse primaire de la victime, en ligne sur
net-entreprises ou par le cerfa 14463*03, **dans les quarante-huit heures, dimanches et jours fériés non compris**, à compter de
la connaissance de l'accident. Le droit est dans le code de la sécurité sociale, relevé depuis LEGI
(`../code-de-la-securite-sociale/`, domaines « La declaration d'un accident du travail ou de trajet » et « La declaration d'une
maladie professionnelle » de `paie/src/releve_codes/textes.rs`) ; ce dossier garde ce que les organismes disent du **service**
et du **formulaire**. Ameli (`www.ameli.fr`) refuse le conteneur (403, « Vérification de sécurité ») : sa page employeur et son
exemplaire du cerfa n'ont pas pu être relevés.

| fichier | ce que c'est | URL | SHA-256 |
|---|---|---|---|
| `net-entreprises-compte-entreprise-dat-20260331.html` | La page « Compte entreprise – Services de l'Assurance Maladie – Risques professionnels » de net-entreprises, telle que servie (`dateModified` 31/03/2026), avec sa section « La Déclaration d'Accident du Travail - DAT » | https://www.net-entreprises.fr/declaration/compte-entreprise-service-de-depot-de-pj/ | `ef302e06…b27544e` |
| `net-entreprises-compte-entreprise-dat-20260331.txt` | Sa transcription (le texte, sans balises) | — | `dda7e37c…a5f906` |
| `service-public-r14587-declarer-accident-travail-20260417.html` | La fiche « Déclarer un accident du travail ou de trajet » d'Entreprendre Service Public (« Vérifié le 17 avril 2026 ») : le cerfa 14463*03, émis par la Cnam | https://entreprendre.service-public.gouv.fr/vosdroits/R14587 | `d497c4f9…761424b95` |
| `service-public-r14587-declarer-accident-travail-20260417.txt` | Sa transcription | — | `7f35ed55…129901d` |
| `cnam-cerfa-14463-03-declaration-accident-travail-20170907.pdf` | Le formulaire cerfa 14463*03 (S6200i) et sa notice 50261#04, modèle fixé par l'arrêté du 26/09/2017. **Copie tierce** : celle d'ameli (`https://www.ameli.fr/sites/default/files/formulaires/128/s6200_homol_ix_17_rempl.pdf`, lien donné par Service Public) est refusée au conteneur ; celle-ci est servie par Previssima, octets d'origine (PDF de la Cnam, modifié le 07/09/2017, chiffré en lecture seule) | https://www.previssima.fr/files/previssima/documents_pdf/telechargements/cerfa_1446303_declaration_accident_du_travail.pdf | `460611d2…ab22e677` |
| `cnam-cerfa-14463-03-declaration-accident-travail-20170907.txt` | Sa transcription (`pdftotext -layout` ; quelques mots de la police du PDF sortent illisibles) | — | `6ba24ba5…8520497` |
| `cnda-essentiel-offre-edi-employeurs-20200424.html` | « L'essentiel sur l'offre EDI Employeurs » du CNDA (Centre national de dépôt et d'agrément de l'Assurance Maladie), telle que servie (modifiée le 24/04/2020 selon `wp-json`), relevée le 20/10/2026 | https://cnda.ameli.fr/editeurs/referencer-un-logiciel/teleservices-employeurs/lessentiel-sur-loffre-edi-employeurs/ | `3c54afce…a3ddbb` |
| `cnda-essentiel-offre-edi-employeurs-20200424.txt` | Sa transcription | — | `b0c1fe4c…7d071` |
| `cnda-referencer-logiciel-teleservices-employeurs-20221003.html` | « Téléservices Employeurs — Comment faire référencer un logiciel EDI Employeurs ? » du CNDA (modifiée le 03/10/2022), relevée le 20/10/2026. Son lien « Télécharger le protocole » (`https://cnda.ameli.fr/editeurs/telecharger-les-protocoles/`) répond 404 | https://cnda.ameli.fr/editeurs/referencer-un-logiciel/teleservices-employeurs/ | `c1617b7a…3a66` |
| `cnda-referencer-logiciel-teleservices-employeurs-20221003.txt` | Sa transcription | — | `7aa06675…a677` |
| `cnda-mode-operatoire-referencement-20221003.html` | « Comment faire référencer un logiciel par le CNDA ? » (mode opératoire général, modifié le 03/10/2022), relevée le 21/10/2026 : « Quelque soit le type de logiciel […], vous devez dans un premier temps télécharger et signer le contrat de service » ; « Après enregistrement du contrat, le CNDA vous adressera votre Numéro d'Identification Editeur (N.I.E) ». Son lien vers les documents contractuels répond 404 ; le formulaire d'inscription en ligne (`sicnda.ameli.fr`) ne propose que PS libéral, transporteur, établissement de santé (vu par Léandre le 21/10/2026) : pour un logiciel employeur, on passe par « Nous contacter » (inférence) | https://cnda.ameli.fr/editeurs/referencer-un-logiciel/mode-operatoire/ | `aeaab29f…f03a22aa` |
| `cnda-mode-operatoire-referencement-20221003.txt` | Sa transcription | — | `9ba4149d…c88ab99c` |
| `net-entreprises-actualite-dat-codes-nationalite-20260608.html` | L'actualité « DAT – mise à jour des codes nationalité » de net-entreprises (08/06/2026), relevée le 20/10/2026 | https://www.net-entreprises.fr/declaration-daccident-de-travail-ou-trajet-dat-mise-a-jour-des-codes-nationalite/ | `bf488289…320e` |
| `net-entreprises-actualite-dat-codes-nationalite-20260608.txt` | Sa transcription | — | `f7983e5c…a5f9` |
| `net-entreprises-actualite-ajout-reserves-dat-20230118.html` | L'actualité « Ajout de réserves aux DAT » de net-entreprises (18/01/2023), relevée le 20/10/2026 | https://www.net-entreprises.fr/ajout-de-reserves-aux-dat/ | `ea3fcc54…6de6` |
| `net-entreprises-actualite-ajout-reserves-dat-20230118.txt` | Sa transcription | — | `b321a3f7…d89` |

## Le canal ouvert aux logiciels (relevé le 20/10/2026, ticket #205)

Léandre, 20/10/2026 : la déclaration d'accident « fait partie des logiciels de paie » ; pas de cerfa. Ce que disent les
textes déposés ici :

- **Il n'y a pas d'API ni d'échange de machine à machine pour émettre une DAT.** Le CNDA (`cnda-essentiel-…txt`) : « DAT :
  Emission des Déclaration d'Accident du Travail (s'effectue en EDI « posté ») » — « EDI « posté » : les fichiers XML sont
  déposés sur le portail Net-Entreprises.fr » ; l'EDI « machine » (point à point, par un opérateur EDI) n'est offert que pour
  **recevoir** les bordereaux de paiement des indemnités journalières (BPIJ). Net-entreprises (`net-entreprises-compte-…txt`) :
  la DAT en ligne se fait « Par saisie de formulaire à l'écran » ou « Par dépôt de fichier structuré issu de votre logiciel de
  paie (EDI) ». Le « dépôt de fichier » est donc un geste humain sur le portail, sous l'habilitation au service DAT ; rien
  qu'un programme ne puisse envoyer seul, comme il le fait d'une DSN ou d'une DPAE par l'API de dépôt.
- **Le fichier exige un logiciel certifié.** « La transmission en mode EDI des DAT dématérialisées nécessite l'utilisation
  d'un logiciel certifié et paramétré pour ce type de transfert de données » ; le certificat qualité « est matérialisé par une
  référence véhiculée dans les flux concernés. **Un flux adressé sans Certificat Qualité sera automatiquement rejeté** par le
  système d'information de l'Assurance Maladie ». La certification est gratuite ; elle commence par la signature du
  « Protocole DAT » avec le CNDA (`cnda-referencer-…txt`), qui ouvre à l'éditeur un espace personnel où se trouvent « toute la
  documentation nécessaire » (le format des fichiers XML), l'outil de vérification locale TEDI et le plan de tests ; l'éditeur
  dépose ensuite son dossier de recette (flux XML, résultats TEDI), et le CNDA délivre le certificat.
- **Le format n'est pas public dans ce qui se relève.** Le CNDA : « L'Assurance Maladie met à disposition l'ensemble de la
  documentation technique permettant la constitution des fichiers XML sur son site ameli.fr » — ameli refuse le conteneur
  (403) ; le paquet complet est dans l'espace de l'éditeur après le protocole. L'actualité du 08/06/2026 montre que ce format
  vit (la rubrique `Code_nationalite` a changé de valeurs le 27/05/2026 ; « Pour les déclarants ayant eu un code erreur […] il
  est nécessaire de mettre à jour votre paramétrage »).
- **Pas d'environnement de test pour l'employeur** : le seul banc décrit est la recette du CNDA (TEDI et plan de tests).
- **Les accusés** : « 3 accusés de réception mis à la disposition du déclarant : l'accusé de dépôt puis 2 accusés de réception
  transmis par courriel » ; les réserves motivées se rattachent en ligne à une DAT non mortelle dans les dix jours francs
  (`net-entreprises-actualite-ajout-reserves-…txt`).

**Ce que le moteur en fait** : rien n'en est écrit en code. Faire la DAT depuis le logiciel demande d'abord une décision de
Léandre, comme éditeur : signer le protocole DAT avec le CNDA (gratuit), et ensuite seulement, le paquet documentaire reçu et
déposé ici, écrire le fichier XML et le faire certifier. Même certifié, le fichier se dépose à la main sur net-entreprises
(EDI « posté »). D'ici là, la déclaration se saisit à l'écran sur net-entreprises : `obligations-base.json` le dit dans le
libellé de `declaration_accident.ou` (code `net-entreprises`, champ `edi` qui cite `cnda-essentiel-…txt`).

## Ce qu'on en lit

- **net-entreprises, section DAT** (`.txt`, à partir de « La déclaration d'accident du travail ou de trajet (DAT) en ligne ») :
  « La déclaration d'accident du travail ou de trajet (DAT) est à effectuer auprès de la Caisse primaire d'assurance maladie
  (CPAM) dont dépend la victime **au plus tard 48 heures après avoir eu connaissance de l'accident** » ; la DAT en ligne donne
  « 3 accusés de réception » ; « Vous avez la possibilité de rattacher des réserves à une DAT non mortelle durant un délai
  règlementaire de **10 jours francs à partir de la date de dépôt initiale de la DAT** » ; l'accès au service vient « sous
  48h » après l'inscription — il faut donc s'inscrire **avant** d'en avoir besoin. Plus haut, la même page : « Votre entreprise
  est soumise à l'obligation DSN ? Désormais, il suffit de produire vos signalements Arrêt de travail […] » (l'attestation de
  salaire passe par le signalement), sauf « les cas spécifiques des temps partiels thérapeutiques et des congés pour deuil qui ne
  sont pas à ce jour couverts par la DSN ». Et, pour l'attestation de salaire : « L'attestation de salaire pour le versement des
  Indemnités Journalières de la Sécurité Sociale (DSIJ) est obligatoire pour tout arrêt de travail, quel qu'en soit le motif »,
  « Deuil d'enfant » compris ; « Maladie, accident du travail, maladie professionnelle, femme enceinte dispensée de travail, deuil
  d'enfant : **Dès réception de l'avis d'arrêt de travail** » ; elle se transmet « Par saisie de formulaire à l'écran » ou « Par
  dépôt de fichier structuré issu de votre logiciel de paie (EDI) certifié », par le service « Att salaire IJ - Attestation de
  salaire pour le versement des indemnités journalières » du compte entreprise. **Pour un deuil d'enfant, c'est donc
  l'attestation hors DSN qui est due, pas le signalement** ; « dès réception » n'est pas un délai chiffré (le moteur prend le
  jour même de la connaissance de l'arrêt, et dit que c'est une inférence).
- **Service Public, R14587** : « À remplir en 4 exemplaires. L'employeur doit en envoyer 3 exemplaires, par lettre recommandée
  avec accusé de réception, à la caisse primaire dont dépend le salarié victime de l'accident, dans les 48 heures après eu
  connaissance de l'accident. Il doit conserver le 4e exemplaire pendant 5 ans. » ; « Réalisable aussi en ligne ».
- **Le cerfa 14463*03** : en tête, « AU PLUS TARD 48 HEURES (non compris les dimanches et jours fériés) APRES AVOIR PRIS
  CONNAISSANCE DE L'ACCIDENT » ; ses rubriques (l'employeur, la victime, puis « LES INFORMATIONS RELATIVES A L'ACCIDENT » : date
  et heure, lieu, lieu de travail habituel ou occasionnel ou trajet, activité de la victime, nature de l'accident, objet dont le
  contact a blessé la victime, réserves motivées, siège et nature des lésions, transport, autres victimes, horaire de travail,
  « Accident constaté / connu le … heure … par l'employeur / par ses préposés / décrit par la victime », conséquences, rapport
  de police, « LE TEMOIN ou LA PREMIERE PERSONNE AVISEE », « LE TIERS » et sa société d'assurance) ; la notice : les réserves
  « ne pourront être prises en compte que si elles portent sur les circonstances de temps et de lieu de l'accident ou sur
  l'existence d'une cause totalement étrangère au travail » ; l'amende de qui néglige « la déclaration des accidents à la
  Caisse primaire dans les 48 heures ou de délivrer à la victime la feuille d'accident » (L. 471-1, R. 471-3).

## Où le moteur s'en sert

`logiciel/donnees/obligations-base.json`, obligation `declaration_accident` : `ou` (les deux voies, net-entreprises et le
courrier), `a_avoir` (ce qu'il faut avoir sous la main, tiré des rubriques du cerfa), `suites.reserves` (dix jours francs),
`textes`. Le calcul (point de départ, quarante-huit heures sans dimanches ni fériés, état) est dans
`atelier/sources/paie/src/obligations.rs` (`de_l_arret`, `Calendrier::heures_apres`) ; les routes `arrets`,
`arret-obligations`, `arret-connaissance` et `accident-declare` le rendent, et le rappel de l'ouverture (route `rappels`,
`routes_ouverture::rappels_obligations`, depuis le 20/10/2026) le rappelle tant qu'il n'est pas dit fait. **Le logiciel ne fait pas la déclaration** : il la
rappelle, avec sa date butoir, et retient qu'elle est faite quand la compagnie le dit.

**Le deuil d'enfant (motif d'arrêt 19, depuis le 15/10/2026)** : la même page (le paragraphe « Votre entreprise est soumise à
l'obligation DSN ? » et ce qui suit sur la DSIJ) fonde, dans `obligations-base.json`, les `sauf_motifs` de `signalement_arret`
et de `signalement_reprise` (pas de signalement pour un motif que la DSN ne couvre pas ; le texte de l'avertissement que
`signalement::verifier` ajoute à `arret-signaler` quand on en écrit un quand même, la norme l'admettant) et l'obligation
`attestation_salaire` (`ou` : `net-entreprises-ecran`, `net-entreprises-edi` ; `delai` : « dès réception de l'avis d'arrêt
de travail », inférence dite). Calcul : `obligations.rs` (`de_l_arret`) ; dite faite par la route `attestation-declare`
(`paie arret declare --attestation`), gardée dans `imprevus/obligations.json`. Le droit du congé de deuil et de ses
indemnités journalières est relevé depuis LEGI : C. trav. L. 3142-1-1, D. 3142-1-1 (`../code-du-travail/`), CSS L. 331-9,
D. 331-7, R. 323-10 (`../code-de-la-securite-sociale/`).

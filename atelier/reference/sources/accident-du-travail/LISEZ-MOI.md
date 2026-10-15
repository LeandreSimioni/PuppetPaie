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

## Ce qu'on en lit

- **net-entreprises, section DAT** (`.txt`, à partir de « La déclaration d'accident du travail ou de trajet (DAT) en ligne ») :
  « La déclaration d'accident du travail ou de trajet (DAT) est à effectuer auprès de la Caisse primaire d'assurance maladie
  (CPAM) dont dépend la victime **au plus tard 48 heures après avoir eu connaissance de l'accident** » ; la DAT en ligne donne
  « 3 accusés de réception » ; « Vous avez la possibilité de rattacher des réserves à une DAT non mortelle durant un délai
  règlementaire de **10 jours francs à partir de la date de dépôt initiale de la DAT** » ; l'accès au service vient « sous
  48h » après l'inscription — il faut donc s'inscrire **avant** d'en avoir besoin. Plus haut, la même page : « Votre entreprise
  est soumise à l'obligation DSN ? Désormais, il suffit de produire vos signalements Arrêt de travail […] » (l'attestation de
  salaire passe par le signalement), sauf « les cas spécifiques des temps partiels thérapeutiques et des congés pour deuil qui ne
  sont pas à ce jour couverts par la DSN ».
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
`arret-obligations`, `arret-connaissance` et `accident-declare` le rendent. **Le logiciel ne fait pas la déclaration** : il la
rappelle, avec sa date butoir, et retient qu'elle est faite quand la compagnie le dit.

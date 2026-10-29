# L'IBAN et le BIC : la forme, la clé, ce qui est exigé

Relevé le 29/10/2026 depuis le conteneur, pour le compte bancaire d'un salarié (sa fiche, et la fiche qu'on lui demande
par courriel) et celui de la compagnie. La norme ISO 13616 (l'IBAN) et ISO 7064 (sa clé) sont payantes ; le site de
l'EPC (European Payments Council), celui de SWIFT (registre IBAN) et celui de l'ISO 20022 répondent 403 au conteneur.
La clé se lit donc dans le texte d'une banque centrale qui transpose ISO 13616 et en écrit l'algorithme pas à pas.

| fichier | d'où | ce qu'on en lit |
|---|---|---|
| `tcmb-communique-iban-20261029.html` / `.txt` | https://www.tcmb.gov.tr/wps/wcm/connect/EN/TCMB%2BEN/Bottom%2BMenu/IBAN/Communique (Banque centrale de la République de Turquie, *Communiqué on the Implementation of the International Bank Account Number*, version anglaise) | Art. 3 c) : l'IBAN est « generated according to the standard ISO 13616 » ; art. 4 (7) : « The calculation and validation of check digits shall be made by using the MOD 97-10 method developed according to the ISO 7064 » ; **annexe 3, « Validation of the IBAN Check Digits »** (`.txt`, l. 189-213) : les blancs et les caractères non alphanumériques s'ôtent ; les quatre premiers caractères passent à droite ; chaque lettre devient un nombre selon la table de l'annexe 2 (A = 10 … Z = 35, l. 175-184) ; **le reste de la division par 97 doit être 1** ; l. 219-227 : le calcul peut se faire par morceaux. Les annexes 1 et 2 (la structure turque, TR et 26 caractères) ne valent que pour la Turquie. |
| `eurlex-reglement-260-2012-sepa-20261029.xhtml` / `.txt` | Règlement (UE) n° 260/2012 du 14 mars 2012 (virements et prélèvements en euros, SEPA), JO L 94 du 30/03/2012, p. 22, servi par l'Office des publications (`https://publications.europa.eu/resource/celex/32012R0260`, EUR-Lex renvoyant un défi anti-robots) ; version d'origine, non consolidée | Art. 2, 15) : l'IBAN, « dont les éléments sont spécifiés par l'Organisation internationale de normalisation (ISO) » (`.txt`, l. 282) ; 16) le BIC (l. 286) ; 17) la norme ISO 20022 XML (l. 290) ; **art. 5, 7** (l. 459) : depuis le 1er février 2016, les prestataires « n'exigent pas des utilisateurs de services de paiement qu'ils indiquent le code BIC » ; annexe, 1 a) (l. 725) : l'identifiant du compte est l'IBAN ; 1 b) (l. 729) : le format des messages est ISO 20022 XML. |

La norme DSN (déjà déposée) dit la forme que la DSN accepte : IBAN de 15 à 34 caractères `[A-Z0-9]+`, « s'appuie sur la
norme ISO 13616:2007 », contrôle CSL-11 « vérification de la clé IBAN » (`../../norme-dsn/texte/bloc-S21.G00.20.txt`,
l. 146-160) ; BIC de 8 ou 11 caractères `[A-Z]{4}[A-Z]{2}[A-Z0-9]{2}([A-Z0-9]{3})?` (même bloc, l. 140-144).

## Ce que le moteur en retient

- `declarations/src/champs.rs`, `faute_iban` : l'IBAN saisi perd ses blancs et passe en majuscules ; il a de 15 à 34
  caractères, des lettres et des chiffres seulement, deux lettres de pays et deux chiffres de clé en tête ; sa clé se
  vérifie par l'annexe 3 du communiqué (reste 1 modulo 97). `faute_bic` : 8 ou 11 caractères de la forme de la DSN.
  Les deux s'appliquent au compte de la fiche Entreprise et à celui d'un salarié (`banque`).
- Le BIC d'un salarié est **facultatif** (art. 5, 7 du règlement) ; celui de la compagnie reste demandé par
  « Pour démarrer », parce que la DSN le déclare (S21.G00.20.003).
- **Le virement des nets** (l'ordre de virement SEPA, message ISO 20022 `pain.001`) **n'est pas écrit** : le règlement
  n'impose que la norme ISO 20022 XML, sans en donner la forme ; la forme du message est dans les *Implementation
  Guidelines* de l'EPC et dans le guide du CFONB, qu'aucun site n'a servi au conteneur. Un **constat**, pas une règle :
  tant que ce texte n'est pas déposé, le moteur ne fabrique aucun fichier de virement.

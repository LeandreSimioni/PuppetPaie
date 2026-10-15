# Pays et territoires étrangers : le Code officiel géographique de l'INSEE (millésime 2026)

Relevé le 10/10/2026 sur insee.fr (le serveur répond depuis le conteneur), pour la liste des pays de la fiche salarié
(pays de naissance, `S21.G00.30.015` ; pays de nationalité, d'où se déduit `S21.G00.30.013`), vague 1k. Les `.csv`
sont tels que servis (UTF-8, fins de ligne d'origine : ne pas convertir) ; les `.txt` sont la transcription des pages
(balises ôtées).

| fichier | d'où | ce qu'on en lit |
|---|---|---|
| `insee-cog-pays-territoires-etrangers-20260101.csv` | https://www.insee.fr/fr/statistiques/fichier/8740222/v_pays_territoire_2026.csv | 229 pays et territoires actuels : `COG`, `ACTUAL`, `CRPAY`, `ANI`, `LIBCOG`, `LIBENR`, `CODEISO2`, `CODEISO3`, `CODENUM3`. sha256 `0dd9d1e83cfba3229b77444747fab0bba7d0ac1acec0442e8e5123c72fa1502d`. |
| `insee-cog-pays-territoires-depuis-1943-20260101.csv` | https://www.insee.fr/fr/statistiques/fichier/8740222/v_pays_et_territoire_depuis_1943.csv | Les couples code - libellé depuis 1943, avec leurs dates : URSS (99123, fin 1991-12-25), Yougoslavie (99121, fin 2003-02-04), Tchécoslovaquie (99115, fin 1993-01-01). **Aucune colonne de code ISO.** sha256 `ec584a234189b4cb28eed0ce18ff1183cc10bd16aca17bb84c92520e92ac7d5a`. |
| `insee-cog-2026-page-telechargement-20260101.html` / `.txt` | https://www.insee.fr/fr/information/8740222 (« Code officiel géographique au 1er janvier 2026 ») | Le dessin du fichier « Pays et territoires étrangers » : « CODEISO2 / 2 / Code du pays sur 2 caractères conforme à la norme internationale ISO 3166 de représentation des noms de pays » ; « CRPAY / 5 / Code officiel géographique de l'actuel pays de rattachement » ; « LIBCOG / 70 / Libellé utilisé dans le COG » ; « LIBENR / 200 / Nom officiel du pays, ou composition détaillée du territoire ». Les noms « suivent les règles de toponymie […] décrites dans l'arrêté du 4 novembre 1993 paru au J.O. du 25 janvier 1994 ». |
| `insee-cog-2026-modalites-20260101.html` / `.txt` | https://www.insee.fr/fr/information/8740218 (« Liste de modalités des fichiers téléchargeables du COG ») | « Code actualité du pays ou du territoire étranger » : « 1 Actuel » ; « 4 Territoire ayant son propre code officiel géographique ». |

## Ce que le moteur en retient

`logiciel/donnees/pays-base.json` (fabriqué depuis le `.csv` le 10/10/2026) : une ligne par code ISO à deux lettres,
avec le nom du COG (`LIBCOG`), le nom officiel (`LIBENR`), le code COG, `ACTUAL`, et le code ISO du pays de
rattachement (`CRPAY` traduit). 213 lignes. Le moteur s'en sert dans `atelier/sources/paie/src/pays.rs` (la liste, la
recherche par code et par nom, ce que la page reçoit), `codification_ue.rs` (le pays de nationalité), `champs.rs` (la
forme de champ « pays »), `routes_fiches.rs` (`deriver` : la nationalité DSN écrite depuis le pays),
`fiche_questionnaire.rs` (`pays_lu`), `dsn.rs`, `fctu.rs`, `amorcage.rs` (`S21.G00.30.015`).

Écartées (liste `ecartes` du JSON, avec la raison) :
- **14 lignes sans code ISO** (`CODEISO2` vide) : le Kosovo (99157), et des regroupements de territoires
  (dépendances de la Couronne britannique — Man, Jersey, Guernesey —, territoires des États-Unis en Amérique — Porto
  Rico, îles Vierges —, territoires antarctiques, etc.). Le COG ne leur donne pas de code ISO ; le logiciel n'en
  invente pas.
- **2 lignes qui reprennent le code d'un État** : territoires espagnols en Afrique du Nord (ES), territoires portugais
  de l'océan Atlantique (PT). Le code désigne l'État.
- Les États disparus (URSS, Yougoslavie, Tchécoslovaquie) ne sont que dans le fichier « depuis 1943 », sans code ISO.

## Les inférences

- **La table « Pays » de la DSN** (cahier technique 2026.1, p. 173 : « Le Code pays de naissance est à déterminer
  dans la table "Codes Pays" ISO 3166-1-A2 » ; `prive/reference/norme-dsn/texte/bloc-S21.G00.30.txt`, l. 297-303)
  n'est pas déposée (elle se publie sur net-entreprises, que le conteneur ne consulte pas). Lire les codes ISO du COG
  comme ceux de cette table est une **inférence** : les deux renvoient à ISO 3166. Un code que la DSN refuserait se
  verrait au premier dépôt de test.
- **Un territoire prend la codification UE de son pays de rattachement** (Groenland : Danemark, 02 ;
  Aruba, Curaçao, Saint-Martin néerlandais, Bonaire : Pays-Bas, 02 ; Åland : Finlande, 02 ; Gibraltar, Sainte-Hélène,
  Pitcairn : Royaume-Uni, 04 ; Hong Kong, Macao : Chine, 04). Le COG dit le rattachement, pas la nationalité : c'est
  une **inférence**. **Exception : les îles Féroé** (FO), que les traités n'appliquent pas (TFUE art. 355 § 5 a ;
  `../union-europeenne/LISEZ-MOI.md`, « Les îles Féroé ») : 04. Taïwan, le Sahara occidental, l'Antarctique n'ont pas de
  rattachement : 04.
- Les départements et collectivités d'outre-mer français ne sont pas des pays étrangers et ne sont pas dans ce
  fichier : une naissance outre-mer se déclare FR (le département de naissance la dit).

## Reste à déposer

- La table « Pays » de la DSN (net-entreprises / dsn-info), pour confronter la liste.
- La norme ISO 3166-1 elle-même (payante) : le COG en est la lecture déposée.

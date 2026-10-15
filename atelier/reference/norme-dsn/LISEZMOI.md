# Norme DSN — les textes et outils qui font foi

Copies des documents officiels dont dépend `moteur/dsn.py` et le dépôt. Rien
ici n'est versionné sur le dépôt public : ce sont des documents publics,
retéléchargeables aux adresses indiquées. Chaque entrée dit ce que contient
le fichier, de quand il date, quand on l'a relevé, et son condensat SHA-256
(`sha256sum <fichier>` pour vérifier que la copie n'a pas bougé).

## La norme

| Fichier | Contenu | Relevé le | SHA-256 |
|---|---|---|---|
| `dsn-cahier-technique-2026.1.pdf` | Cahier technique NEODeS 2026.1 (norme du 6 janvier 2025, en production toute l'année 2026). La référence de chaque rubrique et de chaque contrôle CSL/CCH/CRE. | 03/09/2026 | `76749006…b04ff68536` |
| `dsn-cahier-technique-2027.1.1.pdf` | Cahier technique NEODeS 2027.1.1 du 10 juin 2026 (norme de janvier 2027). Déposé par Léandre le 18/09/2026. Relu ce jour sur la fin de CDDU, le FCTU, 62.017 et 40.025 : identique à la 2026.1 (voir `../sources/francetravail-100-dsn/LISEZMOI.md`). Découpé en texte le 10/10/2026 (`texte-2027/`, son `LISEZ-MOI.md`) ; ses tables d'usages et de formats relevées le même jour (`logiciel/donnees/usages-dsn-p27v01-base.json`, `formats-dsn-p27v01-base.json`, lot V4a). | 18/09/2026 | `15934486…5bae30bf` |
| `dsn-guide-implementation-API-v6.8.zip` | Guide d'implémentation de l'API de dépôt (dépôt, consultation des bilans, retours). Source de `moteur/dsn_api.py`. | 03/09/2026 | `36e54492…c874672be690ecb` |

## DSN-Val — l'outil d'auto-contrôle (point du 15/09/2026)

Source : [Outils d'auto-contrôle Dsn-Val et brique de contrôle](https://www.net-entreprises.fr/declaration/outils-de-controle-dsn-val/),
net-entreprises.fr, page modifiée le 23 juillet 2026, relevée le 15/09/2026.

| Fichier | Contenu | URL d'origine | SHA-256 |
|---|---|---|---|
| `dsn-val-page-net-entreprises-20260915.html` | La page officielle telle que relevée : versions, liens, prérequis. | https://www.net-entreprises.fr/declaration/outils-de-controle-dsn-val/ | `dcc1b543…0bc502149b7dd403` |
| `dsn-val-guide-utilisateur.pdf` | Guide utilisateur, 2 pages, ancien (renvoie encore à dsn-info.fr) : télécharger, ouvrir, lire le résumé. | https://www.net-entreprises.fr/media/documentation/guide-utilisateur/enregistrer-utiliser-outil-dsn-val.pdf | `6f979817…0722bae8a590b1` |
| `dsn-val-notice-installation-fiche1262.html` | Notice d'installation Windows (fiche 1262, modifiée 02/09/2021) : dézipper, ne pas installer sur le bureau, chemin < 260 caractères, supprimer l'ancienne version avant. | https://net-entreprises.custhelp.com/app/answers/detail/a_id/1262/kw/notice | `2cd7d87c…f2659c8043d2fe` |
| `dsn-val-protocole-brique-de-controle-202510.pdf` | Protocole GIP-MDS (version octobre 2025) réservé aux éditeurs pour obtenir la **brique de contrôle** — le moteur dont DSN-Val est l'habillage graphique. | https://www.net-entreprises.fr/media/documentation/protocole-utilisation-brique-de-controle.pdf | `2eb7dc51…c7b4c9498af9e47dab` |
| `coolpaie-brique-dsnval-journal-livraison.txt` | Journal de livraison de la brique embarquée par CoolPaie (copie de `DSNVAL/BT_DV_livraison_journal.txt` à la racine du dossier CoolPaie). Dernière livraison : 2024.1.0.7 du 10/01/2024. | — (fichier CoolPaie) | `ad9ac78d…a5b4531ef2a499a2497dbd4` |

### Ce que dit la page au 15/09/2026

**Version en production : 2026.1.0.16 du 23 juillet 2026**, contrôles du
cahier technique 2026.1 avec le JMN n°1 (mise à jour des nomenclatures de
juillet 2026). Téléchargement libre, sans compte :

- `https://dsn-val.net-entreprises.fr/2026.1/autocontrole-dsn-val_2026.exe` (189 Mo, installateur)
- `https://dsn-val.net-entreprises.fr/2026.1/autocontrole-dsn-val_win32_2026.1.0.16_64.zip` (100 Mo)
- `https://dsn-val.net-entreprises.fr/2026.1/autocontrole-dsn-val_linux_2026.1.0.16_64.zip`

**Version 2027.1.0.2 du 1er juillet 2026** déjà en ligne (« en éditeur fin
septembre »), cahier technique 2027.1 du 24 décembre 2025 : même schéma
d'adresses sous `/2027.1/`. Deux anomalies non bloquantes connues (bloc 15
non référencé en 70 ; 11.024 acceptée à tort). Ce sera l'outil des DSN de
janvier 2027.

**Prérequis :** Windows 64 bits et **Java 64 bits installé à part**
(références OpenJDK 8 et 11, même architecture que l'outil). Il n'y a pas de
Java sur le poste au 15/09/2026. Entrée : le fichier DSN structuré (lignes
`S10.G00…`), donc notre `DSN_Mois.dsn` tel quel. Garder « nombre maximal
d'anomalies » à 200. Avertissement de la page : un changement de socle
technique est à l'étude, qui imposerait une réinstallation complète (les
mises à jour automatiques ne suivraient pas).

### Ce que le dossier `DSNVAL/` de CoolPaie est — et n'est pas

Ce n'est pas DSN-Val : c'est la **brique de contrôle** (moteur Java
« Saturne » de la CNAV, service qui lit `entree/` et écrit `sortie/`), que
seuls les éditeurs signataires du protocole reçoivent (adresses IP
déclarées, zone d'échange, interdiction de transmettre à un tiers). Sa seule
norme installée est `n4ds.dsn.p03v01` (2024), ses jars datent du 08/01/2024 :
**inutilisable pour une DSN 2026, et définitivement** — elle ne se mettra
plus à jour, et on ne peut pas l'obtenir nous-mêmes.

CoolPaie lui-même l'a lâchée. Ses notes de version (`VER 4.0.0.8.*.txt` à
la racine du dossier CoolPaie) :

- 4.0.0.8.292 (07/07/2023) et .293 (12/09/2023) : « Mise à jour brique DSNVAL 2023.1.0.15 / .16 » — dernières mises à jour ;
- **4.0.0.8.302 (27/02/2025) : « DSN - Désactivation temporaire de la brique DSN-VAL (#3011) »** ;
- la ligne est encore là dans la 4.0.0.8.312 du 10/06/2026. Le « temporaire » dure depuis dix-huit mois : CoolPaie ne valide plus rien en local.

### Décision — 15/09/2026, dossier clos (`journal/ETAT.md` §26.7)

**DSN-Val n'est pas installé et ne le sera pas.** Le **dépôt en test** (`python -m moteur depot-dsn … --test`, `S10.G00.00.005
= 01`) fait tourner la même brique de contrôle, à jour, côté plate-forme, et
ajoute les bilans d'identification. C'est le validateur de référence.
DSN-Val n'apporterait que le hors-ligne, au prix de 100 à 189 Mo plus un
JDK 64 bits, à refaire à chaque norme — « lourd et inutile » (Léandre). Si
un jour il fallait quand même l'installer : JDK 64 bits (Adoptium 11), le
zip 64 bits dézippé hors du bureau, puis ouvrir le `.dsn` dans l'outil.

## Relevé du 11/10/2026 : la norme 2027.1.2 (base de connaissances net-entreprises, catégorie « La norme DSN 2027 »)

Léandre a montré la catégorie (`https://net-entreprises.custhelp.com/app/answers/list-search/p/2182`) ; le conteneur a
relevé les pièces jointes des fiches (les pages ne portent pas de titre, elles servent le fichier) :

| fiche | fichier déposé | ce que c'est |
|---|---|---|
| 3382 | `dsn-journal-maintenance-2027.1.2-20260615.pdf` (+ `.txt`) | **Journal de maintenance de la norme 2027.1.2** : V1 (semaine du 15/06/2026, évolutions 1 à 13 = le CT 2027.1.1), V2 (semaine du 14/09/2026, évolutions 14 à 22 = le CT 2027.1.2) ; « Date de prise d'effet en production : Janvier 2027 » (l. 15-21). |
| 3356 | `dsn-note-differentielle-ct-2026.1.2-vers-2027.1.2-20251224.pdf` (+ `.txt`) | Note différentielle CT 2026.1.2 → CT 2027.1.2 (publié en septembre 2026). Non lue. |
| 3358 | `dsn-tableau-des-usages-2027.1.2-20251224.xlsx` | Le tableau des usages 2027.1.2. `logiciel/donnees/usages-dsn-p27v01-base.json` (V4a, PDF 2027.1.1) **est refait sur ce classeur** (lot R5b, 11/10/2026 : voir plus bas). |
| 3357 | `dsn-tableaux-categories-controles-datatypes-usages-2027.1.2-20251224.xlsx` | Catégories, contrôles, datatypes et usages 2027.1.2 (feuille « Standard » : `P27V01`, révision `20260909`, description `2027.1.0.3`). `logiciel/donnees/formats-dsn-p27v01-base.json` **est refait sur ce classeur** (feuilles « Fields » et « Data Types », lot R5b). |
| 3355 | `dsn-cahier-technique-2027.1.1-base-connaissances-20251224.pdf` | « Cahier technique DSN 2027.1.1 » tel que servi par la base ; **empreinte différente** de `dsn-cahier-technique-2027.1.1.pdf` déjà déposé (md5 bbb16f70… contre 91a66c33…) : à comparer avant de s'en servir. Le CT 2027.1.2 lui-même n'est pas dans la catégorie. |

**Ce qu'on a lu du journal** (`dsn-journal-maintenance-2027.1.2-20260615.txt`, lignes citées), et ce que cela change
pour ce que les lots V4a et V4b ont codé sur le 2027.1.1 :
- l. 347-366, évolution 16 : la **CCH-12 de `S21.G00.40.084` est retirée** (le « Après » est vide).
- l. 368-381 et 395-407, évolutions 17 et 19 : **`S21.G00.40.086` et `S21.G00.41.068` : « À ne pas renseigner pour la
  version de norme P27V01 »**, « n'est plus attendue en version de norme P27V01 », suppression annoncée en P28V01. Le
  lot V4b écrit `40.086 = 01` sur les contrats déclarés à la caisse : **à retirer** (et la question de la population
  tombe).
- l. 433-452, évolution 21 : **`S21.G00.40.084` passe de O à C en DSN mensuelle** (I en 04/05/07, C en 08/09/10) ; l'usage
  « Oui » en invocations (l. 455-467). La valeur `02` que V4b écrit pour un temps plein et un CDDU (inférence « la seule
  valeur qui ne contredit pas le calcul ») n'a plus de raison d'être : la rubrique ne se renseigne que pour un temps
  partiel ou un forfait jour réduit (justification l. 447-450).
- l. 197-230, évolutions 9 et 10 : le bloc 58 ne change que pour le type 12 (défraiement parlementaire, à ne pas
  renseigner) ; la datation « en date de versement » (V4b) n'est pas touchée.
- l. 183 et 383, évolutions 8 et 18 : `41.045` (échelon SNCF) ; l. 409-431, évolution 20 : `85.001` CME-11 (NAF 2025 au
  1er janvier 2027). Sans effet sur le moteur.
- Rien sur la règle de bascule (quel fichier part en P27V01) : « prise d'effet en production : Janvier 2027 » est tout.

### Les tables P27V01 refaites sur les classeurs 2027.1.2 (lot R5b, 11/10/2026)

Le CT 2027.1.2 n'est pas servi en PDF : les deux outils de l'atelier lisent désormais aussi un classeur
(`atelier/outils/lire_xlsx.py`, bibliothèque standard ; `extraire_usages.py` lit la feuille « 1 - Tableau des usages »,
`extraire_formats.py` les feuilles « Fields » et « Data Types » : longueur = `Lg Min`/`Lg Max` du type, `csl` = sa
`Regexp`). Commandes :

```
python3 atelier/outils/extraire_usages.py atelier/reference/norme-dsn/dsn-tableau-des-usages-2027.1.2-20251224.xlsx logiciel/donnees/usages-dsn-p27v01-base.json
python3 atelier/outils/extraire_formats.py atelier/reference/norme-dsn/dsn-tableaux-categories-controles-datatypes-usages-2027.1.2-20251224.xlsx logiciel/donnees/formats-dsn-p27v01-base.json
```

**Ce qui a bougé entre le relevé 2027.1.1 (PDF) et le 2027.1.2 (classeurs)**, et rien d'autre :
- usages : `S21.G00.40.084` en mensuelle, O → C (journal de maintenance, évolution 21, l. 433-452) ; mêmes 591 rubriques,
  aucun autre usage ne change (`40.086` et `41.068` gardent leur usage : le « ne pas renseigner en P27V01 » des
  évolutions 17 et 19 est dans leur description, pas dans le tableau). Les libellés sont identiques une fois les blancs
  ramenés et les accents recomposés (NFC : le classeur écrit `S21.G00.72.001` en accents décomposés).
- formats : `S21.G00.40.070` CSL `(0[1-9])|(1[0-2])` → `(0[1-9]|[1-9][0-9])` (évolution 15) ; `S21.G00.41.045`
  `(0[0-9])|(1[0-2])` → `[0-9]{2}` (évolution 18) ; `S89.G00.32.014` (code de distribution à l'étranger) n'a pas de ligne
  dans la feuille « Fields » du classeur (elle est au tableau des usages et a un contrôle CSL-11 dans « Messages ») :
  elle perd sa longueur [1,50], donc tout contrôle de forme (le moteur ne l'écrit pas). Les deux expressions écartées de
  `S21.G00.81.001` sur le PDF (« [(table », « [(A-Z ») n'existent pas dans le classeur (type sans expression) : la
  rubrique garde sa longueur [3,3].
- Les 588 autres formats sont identiques au relevé du PDF 2027.1.1 : c'est la preuve que la lecture du classeur redonne
  celle du cahier (le PDF 2026.1 n'a pas de classeur pour refaire la preuve de V4a).
- La note différentielle 2026.1.2 → 2027.1.2 (fiche 3356) confirme les tableaux d'usages de `40.070` (C I I I I C I) et de
  `41.045` (C N N N N C N) (l. 5151, 5262) ; ses changements de CSL sont ceux du journal.

### Deux lectures du cahier technique 2026.1 (lot U5, 13/10/2026)

**SIG-18 de S21.G00.81** (`texte/bloc-S21.G00.81.txt`, l. 158-163 ; cahier technique 2026.1, p. 303) : « Si un bloc "Cotisation
individuelle - S21.G00.81" de type "131 - Cotisation régime unifié Agirc-Arrco" est renseigné sous un bloc "Base assujettie -
S21.G00.78" parent qui présente une "Date de début de période de rattachement - S21.G00.78.002" et une "Date de fin de période
de rattachement - S21.G00.78.003" **incluses dans le mois principal déclaré**, alors au moins un bloc "Cotisation individuelle -
S21.G00.81" de type "142 - Cotisation régime unifié Agirc-Arrco (part patronale tranche T1)" doit être renseigné sous le même bloc
"Versement individu - S21.G00.50". » Un 131 sous une base d'un autre mois (une régularisation) n'appelle donc **aucun** 142 :
exiger le 142 de tout versement qui porte un 131 est plus strict que la norme. Le moteur écrit toujours le 142 à côté du 131
(`dsn.rs`, `parts_patronales_agirc_arrco`), ce que la norme permet ; l'aide de test `sig_18` (`tests/dsn/agirc_arrco.rs`) lit
maintenant le contrôle tel qu'il est écrit (mois principal : `S20.G00.05.005`).

**S21.G00.40.036, « Code emplois multiples »** (`texte/bloc-S21.G00.40.txt`, l. 1174-1183) : « Cette rubrique précise si le salarié
a plusieurs emplois chez un même employeur au cours d'une même période de paie. » 01 emploi unique, 02 emplois multiples, 03
situation non connue ; aucun contrôle CCH ni SIG ne la traite (`texte/chapitre-4-controles.txt`). L'aide Agirc-Arrco de codification
DSN (`../sources/agirc-arrco/agirc-arrco-aide-codification-dsn-ct2025-v1.3-20250301.txt`, l. 3642-3648 et 3664-3675) donne un
exemple : « un salarié peut cumuler, au cours d'une même période d'activité, un emploi relevant du statut cadre et un emploi
relevant du statut non cadre. Dans ce cas, utiliser le "Code emplois multiples - S21.G00.40.036" = "02 - emplois multiples" sur
chaque emploi », avec une modalité d'exercice du temps de travail différente de « 10 - temps plein ». **Aucun de ces textes ne dit
si deux contrats du même emploi (même fonction, deux cachets de dates différentes) sont « plusieurs emplois »** : « emploi » n'y
est pas défini. Le moteur déclare 01 pour deux contrats du même emploi
(`tests/routes/routes_produire.rs`, `un_salarie_qui_tient_deux_emplois_dans_le_mois_est_en_emplois_multiples`) : une inférence,
que la norme ne contredit pas et ne confirme pas.

### La date de notification de la rupture dans un FCTU (lot C2, 14/10/2026)

**CCH-12 de S21.G00.62.003** (`texte/bloc-S21.G00.62.txt`, l. 328-330 ; cahier technique 2026.1, p. 275) : « La rubrique est
obligatoire si le "Motif de la rupture du contrat - S21.G00.62.002" est "011", "012", "014", "015", "020", "025", "034", "035", "036",
"037", "058", "059", "082", "083", "087", "088", "089", "095", "096", "097","111", "112", "113", "115" ou "117". » La même liste en
2027.1.1 (`texte-2027/bloc-S21.G00.62.txt`, l. 299-301). C'est aussi, mot pour mot, la liste de l'anomalie du certificat de
conformité (nature 11) rendu KO le 14/10/2026 au dépôt de test du FCTU d'un CDDU rompu en 036 sans cette date. En DSN mensuelle
(nature 01), la rubrique est interdite (tableau des usages) : la règle ne vaut que pour le signalement. Où le moteur s'en sert :
`logiciel/donnees/fctu-base.json`, `notification_rupture` (la liste, avec sa `_source`) ; `fctu::notification_exigee`, lue par
`fctu::verifier` (le signalement ne s'écrit pas sans la date) et par `routes_listes::ce_qui_manque` (la liste des fins à signaler
et les rappels de l'accueil la demandent). Test : `tests/dsn/fin_cddu.rs`,
`la_rupture_qui_se_notifie_ne_se_signale_pas_sans_sa_date_de_notification`.

# L'arrondi à l'euro des montants déclarés : la fraction de 0,50 est comptée pour 1

Pourquoi le bordereau Urssaf (blocs 22 et 23), les versements (bloc 20) et les lignes de correction de la DSN s'arrondissent à
l'euro **la moitié vers le haut** (`atelier/sources/paie/src/bordereau.rs`, `euro` ; `src/dsn.rs`, `rond`). Déposé le
13/10/2026, lot U5. Jusque-là le bordereau arrondissait la moitié vers le haut et `dsn.rs` au pair (comme `round()` de Python).

## Le texte

**CSS L. 133-10** (`LEGIARTI000029956107`, en vigueur depuis le 01/01/2015), relevé LEGI du 13/10/2026 :
`../code-de-la-securite-sociale/articles-20261013.md` (l. 421-423), XML : `../code-de-la-securite-sociale/xml/L133-10.xml` :

> Le montant des cotisations et contributions sociales et de leurs assiettes déclarées aux organismes de sécurité sociale
> en application du présent code, du code de l'action sociale et des familles ou du code rural et de la pêche maritime est
> arrondi à l'euro le plus proche. La fraction d'euro égale à 0,50 est comptée pour 1.

C'est la règle. **CSS L. 130-1 ne la dit plus** : sa rédaction en vigueur (depuis le 01/01/2025) ne traite que de l'effectif ;
le guide Urssaf de la DSN la cite encore (voir plus bas).

## Les textes qui la répètent ou l'appliquent

- **Guide Urssaf « Comment déclarer et régulariser les cotisations Urssaf en DSN »**, 04/06/2026
  (`../editeur-dsn/guide-urssaf-regularisation.txt`, l. 580-592) : « 1.6. Gestion des arrondis pour les déclarations de
  cotisations. Les arrondis sont effectués selon la règle de l'article L130-1 du Code de la Sécurité sociale, c'est à dire à
  l'euro le plus proche […] Seules les données agrégées se voient appliquer un arrondi […] En revanche les données nominatives
  ne sont pas concernées par la règle d'arrondi […] Tous les montants des blocs "Cotisation agrégée - S21.G00.23" doivent être
  arrondis à l'euro le plus proche, qu'il s'agisse de montants d'assiette ou de montants de cotisations ; Les montants de
  cotisations des blocs "Versement organisme de protection sociale - S21.G00.20" et "Bordereau de cotisation due - S21.G00.22"
  supportent également la même règle d'arrondi. » Le guide renvoie à L. 130-1 par un numéro d'article que la loi a déplacé :
  c'est L. 133-10 qui porte aujourd'hui les mots.
- **Norme DSN 2026.1**, `../../norme-dsn/texte/bloc-S21.G00.20.txt` l. 204 et `bloc-S21.G00.22.txt` l. 263 : « Urssaf :
  montant, arrondi à l'entier le plus proche (les deux décimales étant valorisées à zéro) » ; l. 194 : « DGFiP : montant,
  arrondi à l'entier le plus proche » ; l. 200-201 : « IRCANTEC : montant arrondi à l'euro le plus proche selon la règle
  suivante : jusqu'à 49 centimes, à l'euro inférieur et à partir de 50 centimes à l'euro supérieur ». La norme ne dit le demi
  que pour l'Ircantec.
- **Unédic, règlement général annexé à la convention du 15/11/2024, art. 53**
  (`../chomage-permanents/unedic-reglement-general-convention-20241115.txt`, l. 2225-2226 ; mêmes mots dans les annexes VIII et
  X, `../plafond-intermittents/`) : « Le montant des contributions est arrondi à l'euro le plus proche. La fraction d'euro
  égale à 0,50 est comptée pour 1, conformément aux dispositions de l'article L. 133-10 du code de la sécurité sociale. »
- **CGI art. 1724** (`LEGIARTI000006312863`, `../code-general-des-impots/articles-20261013.md` l. 88-91) : « La liquidation de
  toutes sommes à recevoir, à quelque titre et pour quelque cause que ce soit, est arrondie à l'euro le plus proche. La fraction
  d'euro égale à 0,50 est comptée pour 1. »

## Ce que le moteur en fait

`bordereau::euro` est le seul endroit qui arrondit à l'euro : la moitié vers le haut (`MidpointAwayFromZero`). Les lignes 23
(`montant_ligne`), les lignes de correction du bordereau, les assiettes du CTP 900/921/381, le montant d'un versement 20 à
taux sur le brut, et le versement à la DGFiP s'en servent. Le nominatif (blocs 78, 79, 81) n'est pas arrondi (guide, § 1.6).

## Ce qui est une inférence

- **Le demi pour la DGFiP** (`dsn.rs`, versement 20 du prélèvement à la source) : L. 133-10 vise « les organismes de sécurité
  sociale » ; la norme dit « arrondi à l'entier le plus proche » sans le demi. CGI 1724 le dit pour « toutes sommes à recevoir »
  du fisc : lire le versement du PAS comme une telle somme est une inférence.
- **Un montant négatif** (une régularisation qui diminue) : « comptée pour 1 » ne dit pas le signe ; le moteur arrondit comme
  l'opposé (-2,50 donne -3). Aucun texte déposé ne le dit (relu au lot U7, 13/10/2026 : L. 133-10, CGI 1724, le guide Urssaf
  § 1.6, la norme aux blocs 20, 22 et 23 — dont la règle de l'Ircantec, « jusqu'à 49 centimes, à l'euro inférieur et à partir
  de 50 centimes à l'euro supérieur », `bloc-S21.G00.20.txt` l. 200-201 — ne parlent que de la fraction). Raison de garder la
  symétrie : une régularisation qui retire un montant déclaré (-2,50 après +2,50) retire l'euro que l'arrondi avait ajouté
  (-3 après +3) ; arrondir -2,50 à -2 laisserait un euro. Prouvé, comme inférence, par
  `tests/calcul/bordereau.rs`, `un_montant_negatif_s_arrondit_comme_son_oppose_par_inference`.
- **Les 74 bordereaux réels retrouvés au centime** (`atelier/sources/paie/tests/donnees/bordereaux-reels.json`, 75 déposés par
  CoolPaie) sont un **constat**, pas un texte : ils concordent avec L. 133-10, ils ne le fondent pas.
- **Le bloc 54 des types 92 et 93** (parts patronales de santé et de prévoyance déclarées au nominatif, `dsn.rs`) s'arrondit à
  l'euro : ni L. 133-10 ni la norme (`bloc-S21.G00.54.txt` ne parle pas d'arrondi) ne le disent, le guide excepte le
  nominatif. La fraction de 0,50 y suit la même fonction que le reste de la DSN, faute de texte contraire.

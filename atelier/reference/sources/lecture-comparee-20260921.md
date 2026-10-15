# Les huit articles qui ont changé de version : ce qui a changé DANS LE TEXTE

*Établi le 21/09/2026, à partir de la base LEGI de la DILA.*

Le relevé du 21/09/2026 dit que huit articles ont changé de version depuis
l'archive globale du 13/07/2025. **Changer de version n'est pas changer de
sens** : un article renuméroté, ou dont un alinéa étranger à la paie est
réécrit, reçoit un nouvel identifiant sans que rien de ce qui nous concerne
bouge. Ce document répond à la seule question qui vaille : *est-ce que ce
qui a changé touche ce que le moteur calcule ?*

## Ce document, et celui que la machine écrit

Ce qui suit est une **lecture** : elle dit, pour chaque article, si ce qui a
changé touche ce que le moteur calcule. Aucune machine ne le dira à notre
place. En revanche le **diff brut** dont elle part, lui, se rejoue :

    python3 outils/relever_les_codes.py comparer <dossier des sources>

La commande relit chaque article qui a changé contre sa version d'avant et
écrit `changements-de-version-AAAAMMJJ.md` à côté des relevés, les versions
antérieures étant rangées dans `<code>/xml-anterieur/` comme pièces
justificatives. Les deux documents se lisent ensemble : celui de la machine
dit **ce qui a changé**, celui-ci **ce que ça vaut**.

## Comment il a été établi

Le relevé ne garde que la version en vigueur ; l'ancienne ne s'y trouve
plus, et l'historique Git ne l'a pas non plus (le passage du 19/09/2026
portait déjà ces mêmes versions). Les huit versions antérieures ont donc été
extraites **de l'archive globale du 13/07/2025, par leur identifiant exact**
-- `LEGIARTI...`, celui que `_changes_depuis_le_global` note sous `avant` --
et non par leur numéro : on ne voulait pas « une vieille version », on
voulait *celle* que le relevé précédent tenait pour en vigueur. 1 521 540
fichiers d'article parcourus, 8 retrouvés sur 8. Les deux textes ont ensuite
été comparés alinéa par alinéa.

## Le tableau

| article | ce qui a changé | le moteur ? |
| --- | --- | --- |
| **CSS L137-12** | le taux de la contribution patronale : **30 % → 40 %** | **déjà à jour** |
| **CSS L161-17-2** | calendrier de l'âge d'ouverture entièrement réécrit | **à décider** |
| CSS L242-1 | le 8° (gains sur titres) supprimé ; **le 7° intact** | sans effet |
| CSS L136-1-1 | le a bis) (gains sur titres) réécrit | sans effet |
| **CT L3141-5** | un **8°** ajouté : le mandat électif local | **à décider** |
| CT L1237-5 | « y compris avant son embauche » | déjà à jour |
| CT L1237-9 | moment d'attribution de l'indemnité, et un alinéa neuf | déjà à jour |
| CT L1226-1 | le maintien tombe en cas de fraude avérée | déjà à jour |

Aucun identifiant de version périmé n'est cité nulle part dans `moteur/` ni
`docs/` -- vérifié article par article.

## Ce qui compte

### CSS L137-12 : 30 % → 40 %, et le moteur le savait déjà

« Le taux de cette contribution est fixé à 30 % » devient « fixé à 40 % ».
C'est le seul alinéa modifié. Cette contribution patronale frappe la part
exonérée des indemnités de mise à la retraite et de rupture
conventionnelle ; le moteur ne la calcule pas -- aucune section du barème ne
la porte -- mais il en **annonce le montant** dans un avertissement.

Le moteur lit ce taux, il ne l'écrit pas : `permanents-base.json` le tient
de la livraison du 02/01/2026 et porte bien **40**. L'avertissement est donc
juste. En revanche **deux rubriques disaient encore « la contribution de
30 % »** (`rubriques-base.json`, rupture conventionnelle et mise à la
retraite) : le même dépôt donnait deux taux, et le faux était le plus
visible. Corrigé.

### CSS L161-17-2 : le calendrier de l'âge est réécrit

L'ancienne rédaction tenait en deux alinéas : soixante-quatre ans pour les
assurés nés **à compter du 1er janvier 1968**, et, pour les générations
antérieures, un âge « fixé par décret [...] à raison de trois mois par
génération ». La nouvelle, issue de la loi n° 2025-1403 du 30 décembre 2025
de financement de la sécurité sociale pour 2026, fait deux choses :

1. elle **décale la génération pivot d'un an** -- soixante-quatre ans pour
   les assurés nés à compter du **1er janvier 1969** ;
2. elle **inscrit les paliers dans la loi** au lieu de les renvoyer au
   décret, génération par génération : 62 ans 3 mois (nés entre septembre et
   décembre 1961), 62 ans 6 mois (1962), 62 ans 9 mois (janvier 1963 à mars
   1965), 63 ans (avril à décembre 1965), 63 ans 3 mois (1966), 63 ans
   6 mois (1967), **63 ans 9 mois (1968)** -- cette dernière génération
   passant ainsi de 64 ans à 63 ans 9 mois.

Pour la paie, cet article ne se lit jamais seul : `L1237-5` du code du
travail renvoie au 1° de `L351-8`, qui renvoie à « l'âge prévu au premier
alinéa de l'article L. 161-17-2 **augmenté de trois années** ». L'âge de la
mise à la retraite d'office suit donc le calendrier ci-dessus, majoré de
trois ans -- **soixante-sept ans pour les assurés nés à compter du 1er
janvier 1969**, et un palier propre à chaque génération antérieure.

**Ce que le moteur en fait, aujourd'hui : rien.** Il avertit au lieu de
vérifier, et il le justifiait par une phrase devenue fausse -- « L161-17-2
n'est pas relevé ». Il l'est. Écrire la vérification est une décision de
conduite, pas une correction de texte : elle n'est pas prise ici.

### CT L3141-5 : un 8° s'ajoute aux périodes assimilées

Le 7° gagne un point-virgule, et un **8°** apparaît : les périodes de
suspension du contrat pour l'exercice d'un **mandat électif local**
(articles L3142-83 à L3142-87, dans les limites du dernier alinéa de
L3142-88) sont désormais assimilées à du travail effectif pour
l'acquisition des congés payés.

Le moteur compte l'acquisition sur le temps de **présence** et avertit dès
qu'une suspension traverse un mois, faute d'appliquer la liste des périodes
assimilées. Le nouveau 8° allonge cette liste sans changer ce que le moteur
fait ; il ne le rend pas plus faux qu'avant. À noter tout de même : les
articles auxquels ce 8° renvoie (L3142-83 à L3142-88) **ne sont pas
relevés**. Les ajouter à la liste des voulus serait le préalable de tout
codage.

## Ce qui ne change rien, et pourquoi il fallait le vérifier

**CSS L242-1.** Un alinéa disparaît : le **8°**, sur le gain net réalisé sur
des titres souscrits ou acquis par des salariés ou dirigeants -- les
« management packages », transférés au 163 bis H du code général des impôts.
Le **7°**, celui qui sort de l'assiette les indemnités de rupture dans la
limite de deux plafonds annuels, et son alinéa sur les salariés en droit de
bénéficier d'une pension de retraite, sont **identiques mot pour mot**.
C'est ce 7° que le moteur invoque, et lui seul : rien à faire.

**CSS L136-1-1.** Un seul alinéa modifié sur cinquante-six, le a bis) du
même sujet : les gains sur titres, désormais définis par renvoi au 163 bis H
du CGI. Tout le reste -- dont ce qui commande l'assiette de la CSG et de la
CRDS des indemnités de rupture -- est inchangé.

**CT L1237-5.** L'âge de la mise à la retraite peut désormais avoir été
atteint « **y compris avant son embauche** » ; et l'employeur interroge le
salarié sur son intention de « bénéficier **ou continuer de bénéficier** »
d'une pension. Deux précisions sur un cas -- le salarié déjà retraité que
l'on embauche -- que le moteur ne traite pas différemment.

**CT L1237-9.** L'indemnité de départ à la retraite n'est plus attribuée
« lors de la première liquidation complète de la retraite » mais « lorsque
le salarié fait valoir ses droits à une pension de vieillesse de droit
propre au titre du régime de base auquel il est affilié au titre de l'emploi
qu'il occupe dans l'entreprise ». Un alinéa neuf permet en outre d'affecter
l'indemnité au maintien de rémunération d'une fin de carrière à temps
partiel, par accord. `permanents-base.json` **cite déjà la nouvelle
rédaction** : le travail du 19/09/2026 l'avait intégrée.

**CT L1226-1.** Le maintien de salaire ne s'applique pas « en cas de fraude
avérée du salarié afin d'obtenir le versement des indemnités
journalières [...] dont l'employeur a été informé ». Une exclusion que le
moteur ne peut pas constater seul -- elle suppose une information de la
caisse -- et qui ne change aucun calcul.

## Ce qui reste à décider, et n'est pas fait ici

1. **Vérifier l'âge de la mise à la retraite** plutôt que l'avertir.
   L161-17-2 est relevé, le calendrier est connu, la règle est
   « L161-17-2 + trois ans ». C'est un changement de comportement du moteur.
2. **Relever L3142-83 à L3142-88**, sans quoi le 8° neuf de L3141-5 ne peut
   pas être appliqué.
3. **La contribution de L137-12** reste due et non calculée. Le taux est
   juste, l'avertissement aussi ; la calculer est un autre sujet.

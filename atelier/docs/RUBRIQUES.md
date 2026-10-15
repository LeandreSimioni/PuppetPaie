# Construire une rubrique de paie

Question de l'éditeur, le 09/09/2026 : *« il faut saisir toutes les rubriques et
essayer de comprendre comment CoolPaie les a construites. Dans le futur on
n'aura plus CoolPaie ; si on veut en construire, il faut soit comprendre la
règle, soit trouver des docs officielles. »*

Et sa reformulation du 10/09, qui est la vraie : **comment construit-on une
rubrique ?** Pas « comment CoolPaie a rempli son catalogue » — ça, c'est lire
le travail d'un autre. La question qui rend autonome, c'est : sur quelle
règle, quel texte, quel raisonnement. Parce qu'après janvier 2027 il n'y aura
plus de catalogue à copier, et qu'il faudra savoir dire, pour une forme
d'engagement qui n'existe dans aucune liste, ce que valent ses heures et ce
qu'elle supporte.

Ce document est la réponse : **sept questions, dans l'ordre, chacune avec sa
source et ce que coûte une erreur.** Il dit aussi, et c'est neuf depuis le
20/09/2026, **les quatre champs qu'il ne faut pas remplir** parce que rien ne
les applique encore — ils étaient huit le matin, sept le soir, et quatre au
lendemain.

---

## 1. Ce qu'est une rubrique — et ce qu'elle n'est pas

Une rubrique est une **forme** de paie : ce qu'on compte, ce que ça supporte,
ce que ça apporte aux droits du salarié. Ce n'est pas un montant, ni une
durée figée.

C'est là que le catalogue de CoolPaie s'est perdu : **48 de ses 81 rubriques
sont la même rubrique avec un nombre gelé.** `j7 j8 j10 j12` sont une journée
dont on n'a jamais su demander les heures ; `s5 s6` une semaine ; `srp3 srp
srp3cg# srp4cg#` une répétition. Notre catalogue en compte vingt-sept et en
exprime davantage, parce que le nombre se saisit. Une journée de 5 heures se
saisit ; CoolPaie aurait exigé un `j5` qui n'existe pas.

**Ce qui reste distinct, c'est ce qui change de régime.** Un défraiement au
forfait et un défraiement au réel se déclarent `07` et `09` : deux rubriques.
Une journée de 7 h et une journée de 10 h : une seule.

### Trois grandeurs indépendantes, et deux d'entre elles sont des faits

| | ce que c'est | qui le porte |
|---|---|---|
| les **jours** travaillés | un fait | la saisie, avec les dates |
| les **heures** d'une rubrique horaire | un fait | la saisie |
| les **heures d'un cachet** | une convention : 12 h que personne n'a faites | le catalogue, avec sa source |

Une seule des trois est une constante réglementaire. **Aucune rubrique ne
porte de coefficient `jours`** — `Rubrique.jours()` lève une exception exprès,
pour que personne ne recrée la colonne de CoolPaie en croyant combler un
manque. Leur colonne `jours` n'était pas une règle : c'était une supposition
sur une donnée qu'ils ne demandaient pas, avec quatre valeurs incohérentes
pour la même notion. Le détail est dans
`reference-privee/anatomie-rubrique-coolpaie.md (relevé sur une compagnie réelle, pièce hors du dépôt public : dépôt privé)`.

---

## 2. La méthode : sept questions, dans cet ordre

Chaque question a un champ, une source, et un coût si on se trompe. Les
champs sont ceux de `Rubrique` (`paie/src/catalogue.rs`, les vingt-huit de
`CHAMPS_CONNUS` : un champ inconnu est refusé). La liste de qui les applique,
`catalogue.CHAMPS` du Python, et son test sont partis avec le Python sans être
portés (`catalogue.rs`, en tête).

### Q1 — Qu'est-ce qu'on compte ? → `unite`

`cachet`, `heure`, `jour`, `semaine`, `mois`, `service`, `forfait`,
`pourcentage`. Une unité hors de cette liste est **refusée** : mieux vaut
s'arrêter que produire une quantité dont personne ne sait ce qu'elle mesure.

*Source :* ce que le contrat paie réellement. C'est la seule des sept qui ne
se cherche pas dans un texte.

*Si on se trompe :* le bulletin dit « 3 Journées » là où il fallait lire
« 3 Cachets », et le décompte des heures part de travers.

### Q2 — Qu'est-ce qu'on demande à celui qui saisit ? → `saisie`

`quantite`, `taux`, et `heures` quand la rubrique ne les connaît pas
d'avance. Une journée demande ses heures ; un cachet non, il en a douze par
règlement.

*Si on se trompe :* on retombe dans le travers des `j7 j8 j10 j12` — figer
dans le code une donnée que l'utilisateur connaît et qu'il suffit de lui
demander.

### Q3 — Est-ce que ça ajoute ou ça retire ? → `sens`

`+` par défaut, `-` pour une absence ou une retenue. Le signe porte **aussi
sur les heures** : une absence retire des heures, et les compter en plus
ferait déclarer un salarié absent comme s'il avait travaillé double.

### Q4 — Qu'est-ce que ça supporte ? → `soumis`, `imposable`

Deux booléens, et ils ne vont pas toujours ensemble : l'indemnité de rupture
conventionnelle d'un salarié en droit à une pension est **imposable et non
soumise**. C'est le seul endroit du catalogue où les deux divergent (cette
indemnité et son supplément), et il existe.

**`imposable` commande la rémunération nette fiscale** (DSN `S21.G00.50.002`,
le « Net imposable » du bulletin, et ce qui en découle : 50.004, 50.013,
50.009 du PAS) depuis la vague 1a (10/10/2026,
`prive/journal/norme/corrections/PASSATION-V1a.md`). Elle part du brut soumis,
retire les cotisations salariales, réintègre la CSG/CRDS non déductible et la
santé patronale, et n'**ajoute**, des lignes hors assiette (`soumis` faux), que
celles dont le catalogue dit `imposable` vrai (`calcul.rs`, `calculer`, 7.10) :
la norme fait « DEDUIRE : […] les sommes versées à titre de remboursement de
frais professionnels » et « AJOUTER : […] les indemnités imposables à l'impôt
sur le revenu » (cahier technique 2026.1, § 2.3.2, p. 60-61 ;
`atelier/reference/norme-dsn/texte/chapitre-2-principes-de-constitution.txt`). Un défraiement, un acompte, une indemnité exonérée
(CGI 80 duodecies) n'y entrent donc pas, quand avant ils y passaient par le
net à payer. Une ligne **soumise** reste dans la RNF par le brut, même dite
non imposable : aucune rubrique livrée n'est dans ce cas, et le moteur ne sait
pas encore en retirer une (une heure supplémentaire exonérée qu'une compagnie
ajouterait, par exemple).

*Sources :* CSS L.242-1 pour l'assiette des cotisations, CGI 80 duodecies
pour les indemnités de rupture, arrêté du 20/12/2002 pour les frais
professionnels, et la convention collective pour ce qu'elle ajoute.

*Si on se trompe :* une cotisation oubliée est une dette qui se découvre deux
ans plus tard, avec ses majorations. Une cotisation de trop se paie tout de
suite et ne se récupère pas toujours.

> **La `nature` ne répond pas à cette question.** `salaire`, `frais`,
> `indemnite`… sont une **étiquette**, utile à la lecture et au tri de
> l'écran, et rien de plus. Jusqu'au 20/09/2026 un commentaire de
> `catalogue.py` affirmait le contraire — « c'est la nature qui commande
> l'assujettissement » — et c'était faux : `soumis` et `imposable` sont
> écrits rubrique par rubrique, et rien ne les confronte à la nature. En
> faire une règle demanderait de lui donner ses bornes, que la loi met
> partout : un défraiement est hors assiette **dans les limites** de
> l'arrêté, et y rentre au-delà.
>
> **Une exception, d'une ligne hors assiette seulement** (vague 1b, 10/10/2026) :
> `soumis` faux une fois posé, la nature dit ce qu'est la ligne pour le
> **montant net social** (DSN `S21.G00.58.004`, bulletin). `frais` : un
> remboursement de frais, hors du montant net social ; `indemnite` : un revenu
> (exonéré ou non), dedans ; `retenue` : un mouvement de trésorerie (acompte,
> avance, saisie), hors. Toute autre nature sur une ligne hors assiette est
> **refusée** au calcul (`catalogue.rs`, `Rubrique::hors_assiette`). Textes :
> décret 2023-1378, CSS R. 844-1 II, FAQ du BOSS Q 2.2, 2.11, 2.12
> (`atelier/reference/sources/montant-net-social/LISEZ-MOI.md`).

### Q5 — Qu'est-ce que ça apporte aux droits du salarié ? → `heures_forfait`, `compte_pour`, `plafond_annuel_heures`

C'est la question la plus dangereuse, parce qu'une erreur ici **ne se voit
nulle part** : elle se paie des années plus tard, quand le salarié compte ses
507 heures.

- **`heures_forfait`** n'existe **que** là où la réglementation accorde des
  heures qui n'ont pas été faites. Aujourd'hui, un seul cas : les **12 heures
  du cachet** (annexe X, art. 3). Partout ailleurs les heures sont saisies,
  parce qu'elles sont un fait. Un forfait sur une rubrique dont l'unité est
  l'heure est **refusé** : il compterait deux fois.
- **`compte_pour`** dit dans lequel des deux décomptes la rubrique tombe.
  Toutes les heures ne comptent pas pour la même chose :

  | | ce qu'il répond |
  |---|---|
  | `droits_et_montant` | le cas ordinaire |
  | `ouverture_des_droits` | les heures d'enseignement : elles font atteindre les 507, mais n'entrent **pas** dans le calcul du montant de l'ARE |

  Un compteur unique répondrait juste à une question et faux à l'autre — et
  c'est la fausse qu'on lirait : « j'ai mes heures » et « mon allocation sera
  calculée là-dessus » ne se lisent pas au même endroit.
- **`plafond_annuel_heures`** borne ce qu'on retient, par tranche d'âge : 70 h
  d'enseignement avant 50 ans, 120 h à partir de 50 ans.
- **`plafond_mensuel`** borne le nombre d'unités converties dans un mois : **28
  cachets** (annexe X, art. 3). Il se compte sur le **total du mois et par
  salarié** — pas contrat par contrat : trois contrats de dix cachets dans le
  même mois en font trente, et un plafond appliqué contrat par contrat les
  laisserait tous passer. Et il ne borne ni ce qu'on paie ni ce qu'on déclare :
  les cachets au-delà sont joués, payés, déclarés, et n'apportent plus
  d'heures. C'est pourquoi il vit dans `paie/src/cumul.rs`, le décompte des droits, et
  non dans le calcul.

Les deux plafonds sont **indépendants** et le restent : l'enseignement est dans
les heures saisies, que le plafond mensuel ne touche pas.

*Sources :* règlement d'assurance chômage, **annexes VIII et X**, décret du
13 juillet 2016. Les deux annexes demandent **507 heures sur 12 mois** depuis
ce décret ; la durée ne les distingue donc plus. Ce qui les distingue : un
artiste peut être payé au cachet, un technicien est payé à l'heure et ses
heures sont les heures faites.

> **Il n'y a plus de cachet de 8 heures.** Le décret du 13 juillet 2016 a
> supprimé la distinction cachet isolé (12 h) / cachet groupé (8 h) : **tous
> les cachets valent 12 heures**, quelle que soit la durée du contrat. Le
> catalogue de CoolPaie porte encore 8 h, et cette valeur a servi cinq ans
> chez nous. C'est l'exemple le plus net de ce que coûte la recopie.

### Q6 — Qu'est-ce que ça déclare ? → `dsn_autres_bruts`, `dsn_bloc52`

**`dsn_autres_bruts`** porte le code `S21.G00.54.001` d'un élément de revenu
brut qui n'est pas du salaire : un défraiement au forfait (07) ou au réel
(09), un avantage en nature (02 repas, 03 logement, 04 véhicule, 05 NTIC, 06
autres — `genre`, à la saisie, choisit lequel). Les codes admis sont relevés
dans le cahier technique et tenus dans `catalogue.AUTRES_BRUTS` : un code
inventé est refusé à la construction de la rubrique, et non au retour du
point de dépôt.

**`dsn_bloc52`** porte le code d'une prime ou d'une indemnité de rupture.

Deux champs restent sans lecteur à cette question, et il ne faut donc pas
les remplir : **`aem`** (l'AEM n'est plus émise, et elle recevait des nombres,
pas des natures) et **`dsn_type`**, qui fait doublon avec trois champs qui
marchent. Voir le §5.

### Q7 — D'où vient la règle ? → `source`

**Jamais vide** — une rubrique sans source est refusée à la construction. Ce
n'est pas une formalité : c'est ce qui permet à quelqu'un d'autre, plus tard,
de vérifier le nombre au lieu de le croire. La source nomme le texte et sa
date, pas « la réglementation ».

La source de chacune se lit dans `logiciel/donnees/rubriques-base.json` (et
`config/rubriques-compagnie.json` pour celles d'une compagnie) ; la commande
`rubriques --detail` du Python qui l'affichait n'est pas portée.

---

## 3. Où se trouvent les réponses

| ce qu'on y trouve | où |
|---|---|
| les 12 h du cachet, les 507 heures, les limites d'enseignement | règlement d'assurance chômage, **annexes VIII et X** (unedic.org) |
| l'assiette des cotisations | **CSS L.242-1**, relevé par `paie releve-codes` |
| le régime des indemnités de rupture | **CGI 80 duodecies**, **CSS L.136-1-1**, même outil |
| les frais professionnels et leurs limites | **arrêté du 20/12/2002** |
| les jours et heures à déclarer, les codes emploi | **cahier technique AEM V5**, France Travail |
| les codes de rémunération, le bloc 54 | **cahier technique DSN** (NEODeS), dans `atelier/reference/norme-dsn/` |
| les taux réglementaires | **table des paramètres du BOSS**, en CSV |
| les minima, les durées de service, la prévoyance | **convention collective** (IDCC 3090, 1285) sur KALI |

**Une archive n'est pas une source à jour.** Le relevé LEGI du 19/09/2026 a
corrigé sept articles sur soixante-trois, dont un taux passé de 30 % à 40 %.
Ce qui date un relevé, ce sont les livraisons passées après.

---

## 4. Les trois pièges, nommés

**1. Inventer un coefficient parce qu'il « semble juste ».** Ces nombres
partent dans l'AEM et dans le plafond DSN. Un coefficient faux ne se voit
nulle part et fausse les droits du salarié plusieurs années plus tard. Si la
règle n'est pas trouvée, elle **se demande** — à France Travail, à Audiens, à
la convention. Le moteur refuse de produire plutôt que d'inventer.

**2. Recopier le catalogue de CoolPaie.** Même la copie ne se copie pas
correctement : faite avec soin depuis leur propre export, elle était fausse à
4 % — deux rubriques manquantes et un code erroné (`qrs` pour `grs`),
c'est-à-dire ce qu'on tape. Et elle porte au moins une valeur périmée depuis
dix ans (le cachet de 8 h). `prive/config/rubriques.json` garde cette copie
**pour comprendre et vérifier, jamais pour payer** ; un test interdit à un
module de production de la consulter.

**3. Remplir un champ que rien n'applique.** Le piège le plus récent, et le
plus silencieux des trois. Il a sa section.

---

## 5. Ce que le catalogue ne tient pas — relevé du 20/09/2026

L'audit (`outils/auditer_le_catalogue.py`, Python, retiré au lot K sans être
porté) relisait le moteur Python et cherchait,
pour chacun des vingt-huit champs d'une rubrique, qui l'appliquait. **Huit
n'étaient lus que par la commande qui les affiche.** `plafond_mensuel` a été
branché le jour même — les 28 cachets, comptés sur le total du mois (§49.8) —
puis trois autres le lendemain (§51). Il en reste **quatre**.

| champ | ce qu'il a l'air de faire | ce qu'il fait |
|---|---|---|
| `plafond` | l'unité de proratisation du plafond Urssaf, cinq modes | rien — `plafond.py` ne connaît que le mode « cachet », en dur |
| `aem` | la nature déclarée à France Travail | rien — l'AEM reçoit des nombres, pas des natures |
| `dsn_type` | vers quel bloc DSN la ligne part | rien — l'aiguillage se fait par trois autres champs, qui marchent |
| `annexe` | VIII ou X | rien |

Et les trois branchés le 20/09/2026, qui ne sont plus dans cette liste :

| champ | ce qu'il fait maintenant |
|---|---|
| `dsn_autres_bruts` | un bloc 54 par ligne qui le porte, code vérifié contre la norme (`paie/src/calcul.rs`, `paie/src/dsn.rs`) |
| `conges_spectacles` | l'assiette `brut_cs`, un sous-ensemble du brut ligne par ligne, que le barème nomme |
| `arret_de_travail` | `dsn.generer` refuse de produire le mois quand la ligne est là et l'arrêt absent ; depuis le 15/10/2026, la ligne d'un permanent se tire de l'arrêt saisi (`paie/src/imprevus.rs`), et le refus ne vise plus qu'une ligne écrite à la main |

**Pourquoi c'est grave et pas seulement inélégant.** Celui qui construit une
rubrique les remplit de bonne foi. Il écrit `dsn_autres_bruts: "09"` pour un
défraiement au réel, relevé dans la norme, et croit avoir déclaré quelque
chose. Rien ne part, et rien ne le dit. C'est exactement l'erreur que le §14.1
annonçait : invisible à la production, invisible au contrôle, découverte des
mois plus tard par un organisme — ou jamais.

**Ce qui les tenait, du temps du Python.** Chaque champ portait, dans
`catalogue.CHAMPS`, sa question et son lecteur — ou, s'il n'en avait pas,
**ce qu'il faudrait pour le brancher**. Un champ mort et muet était refusé.
L'audit confrontait le tableau au code **dans les deux sens** et échouait si
l'un des deux bougeait sans l'autre, sous un test ; et `rubriques --detail`
disait, rubrique par rubrique, ce qu'elle promettait en vain. Rien de cela
n'est porté (`paie/src/catalogue.rs`, en tête) : le moteur Rust refuse un
champ inconnu, mais plus rien ne confronte un champ à son lecteur.

Aucun des quatre n'est bloquant aujourd'hui : nos intermittents sont payés
par des rubriques dont tout ce qui compte est appliqué, et le rejeu de cinq
ans le prouve. Et **trois des quatre ne se branchent pas par du code** — c'est
ce que le passage du 20/09 a établi :

- **`plafond`** demande de relever les quatre autres modes de proratisation
  Urssaf, mode par mode, à leur source, sans jamais toucher au mode
  « cachet » : c'est le seul qui soit prouvé, calé au centime sur cinq ans ;
- **`aem`** n'aura de sens que si une AEM redevient ligne à ligne — elle
  n'est plus émise depuis le 01/06/2026. Le brancher n'est pas la question :
  **le retirer** en est une ;
- **`dsn_type`** fait doublon avec `salaire_de_base`, `retabli` et
  `dsn_bloc52`, qui sont lus. Le brancher voudrait dire les remplacer par
  lui, pas s'ajouter à eux — un arbitrage, pas une tâche ;
- **`annexe`** attend un usage : le cumul compte les heures sans demander de
  quelle annexe elles viennent, et c'est juste tant que le seuil est le même
  pour les deux (décret du 13 juillet 2016). Il servirait le jour où elles
  divergeraient, ou pour refuser un cachet à un technicien.

---

## 6. Un exemple complet

Une compagnie veut payer un **filage** : une répétition générale de 5 heures,
qui compte pour une journée de travail.

| question | réponse | pourquoi |
|---|---|---|
| Q1 unité | `jour` | on compte des filages, pas des heures |
| Q2 saisie | `quantite`, `taux`, `heures` | la durée varie d'un filage à l'autre : on la demande |
| Q3 sens | `+` | elle paie |
| Q4 soumis / imposable | `true` / `true` | c'est du salaire, CSS L.242-1 |
| Q5 heures | **pas de `heures_forfait`** | 5 h faites sont 5 h ; aucun texte n'en accorde d'autres |
| Q5 compte_pour | `droits_et_montant` | ce n'est pas de l'enseignement |
| Q6 déclaration | **rien** | les champs existent, rien ne les applique |
| Q7 source | le texte qui fixe la durée de service, IDCC 3090 | jamais vide |

```json
"filage": {
  "libelle": "Filage",
  "unite": "jour",
  "nature": "salaire",
  "saisie": ["quantite", "taux", "heures"],
  "heures_forfait": null,
  "soumis": true,
  "imposable": true,
  "conges_spectacles": true,
  "source": "IDCC 3090, clauses ... : le service de filage. Heures saisies, aucune forfaitisation."
}
```

Elle l'écrit dans **son** fichier, `config/rubriques-compagnie.json`, et non
dans le catalogue livré : celui-ci ne décrit aucune compagnie, il décrit ce
que la réglementation permet de payer. Si le filage relevait d'un texte de
branche, sa place serait dans `moteur/rubriques-base.json` — même partage que
le barème.

---

## 7. Ce qui n'est toujours pas élucidé

**Le `#` final de CoolPaie.** Vingt-et-une rubriques sur 81 le portent :
activité partielle, primes exceptionnelles, heures supplémentaires. L'hypothèse
— **non vérifiée** — est qu'il marque un traitement social hors régime commun.
Elle explique dix-neuf des vingt-et-une. Sans conséquence pour nous : notre
catalogue n'en dépend pas.

**La correspondance complète des codes DSN.** Cinq rubriques de CoolPaie
seulement portaient un code, et ces cinq-là sont adossées à `S21.G00.54.001`
de la norme, valeur par valeur (voir `reference-privee/anatomie-rubrique-coolpaie.md` : relevé sur une compagnie réelle, pièce hors du dépôt public, dépôt privé). Les
autres alimentent le brut, déclaré globalement. La correspondance pour une
rubrique **neuve** reste à établir depuis NEODeS, et c'est le §5 qui dit
pourquoi ça ne presse pas encore.

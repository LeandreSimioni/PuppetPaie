# Compagnies fictives : ce que le point de dépôt en dit

Le 21 septembre 2026, les sept DSN que les compagnies fictives savent
produire (`COMPAGNIES-FICTIVES.md`) ont été **déposées en test** sur
net-entreprises. C'était le chantier ouvert par l'en-tête de
`outils/compagnies_fictives.py` : « LE DEPOT DE TEST RESTE A FAIRE. C'est le
seul validateur du projet. » Jusque-là, ces fichiers étaient conformes au
tableau des usages et aux formats de la norme, **et à rien d'autre**.

Un dépôt de test ne conserve rien : les contrôles s'exécutent, le bilan
revient, aucune déclaration n'est enregistrée.

**Le résultat, d'abord : les sept sont `etat_conformite OK`, zéro anomalie**,
déposées sous le SIRET réel de la compagnie, en annule-et-remplace sur le
mois d'octobre 2026. Y arriver a demandé de franchir trois murs, et chacun a
appris quelque chose.

---

## Premier mur : nos propres clés — et il parlait de nos données

Sept dépôts, sept bilans `KO`, **cinq anomalies bloquantes par compagnie**.
Aucune ne venait du moteur. Toutes descendaient d'une seule incohérence du
fabricant : la fiche entreprise portait un NIC écrit à la main (`00017`) à
côté d'un SIRET calculé, et les deux ne désignaient pas le même
établissement.

| contrôle | ce qu'il disait | pourquoi |
|---|---|---|
| `S10.G00.01.002/CCH-11` | le NIC de l'émetteur ne respecte pas la clé SIRET | NIC constant, SIREN variable : la clé ne tombe juste que par hasard |
| `S21.G00.06.002/CSL-11` | le NIC du siège ne respecte pas la clé | idem |
| `S21.G00.11.001/CSL-12` | le NIC de l'établissement ne respecte pas la clé | idem |
| `S21.G00.40.019/CCH-11` (×1 par contrat) | lieu de travail hors établissement employeur, sans bloc 85 | la rubrique vient de `entreprise.siret` (…00018), les blocs 06 et 11 du `nic` (00017) : deux établissements dans la même déclaration |

C'est exactement ce que le fichier promettait de ne pas laisser passer —
« les SIRET par une clé de Luhn juste : un test ne doit pas pouvoir passer
sur un numéro impossible ». La promesse tenait pour `_siret()` ; elle ne
tenait pas pour le NIC posé à côté, et le test qui vérifiait la clé du SIRET
ne pouvait pas le voir. **Corrigé** : le NIC est dérivé du SIRET fabriqué
(`nics = [s[9:] for s in sirets]`), un profil déclare un **nombre
d'établissements** et non plus des NIC, et
`test_compagnies_fictives.py` verrouille les trois assertions qui manquaient.

Ce que cela dit du moteur mérite d'être noté : **il n'a rien inventé**. Il a
écrit le NIC là où la norme veut un NIC et le SIRET là où elle veut un
identifiant de lieu de travail, fidèlement — jusqu'à rendre visible que les
deux se contredisaient. Une donnée fausse a produit une déclaration
**refusée**, pas une déclaration fausse et muette.

## Deuxième mur : `B1-201-01`, et pourquoi le SIRET réel

Corrigées, les sept ne portaient **plus aucune anomalie de forme**. Restaient
deux codes, dont aucun ne parlait du fichier :

- `B1-201-01`, bloquant — `Siret <…> inconnu.`
- `B1-207-01`, non bloquant — `La transmission est tardive pour la periode
  2026-06-01` (on déposait juin en septembre).

**`B1-201-01` est un contrôle de référentiel, pas de forme** : l'Urssaf ne
connaît pas ces SIREN et ne les connaîtra jamais, ils sont inventés. Elle
s'arrête là, et **ses contrôles métier** — bordereaux, CTP, cohérence de
l'agrégé et du nominatif — **ne s'exécutent jamais derrière ce refus**.

D'où l'option `--siren` (annoncée par l'en-tête de l'outil depuis le début,
et qui n'existait pas) : la compagnie fictive **emprunte l'identité
déclarative** de la compagnie qui dépose — SIREN, NIC, SIRET, raison sociale,
adresse, compte Urssaf, références Congés Spectacles et France Travail. Rien
d'autre. Les salariés, les contrats, la paie, la convention, l'effectif, le
taux AT/MP restent fabriqués : **on emprunte une porte d'entrée, pas un
contenu.** L'IBAN et le déclarant ne sont pas empruntés non plus — un dépôt
de test ne prélève rien, et il n'y a aucune raison de promener de vraies
coordonnées bancaires ni un nom de personne dans un jeu d'essai.

`B1-201-01` a disparu du premier coup.

## Troisième mur : `B1-208-10`, et le compteur d'essais

Derrière, un autre refus, sur les sept : **`B1-208-10 — il existe déjà une
déclaration normale`**. La plate-forme garde mémoire des dépôts de test, et
tous les mois de 2026 ont été éprouvés un jour ou l'autre sous ce SIREN.

Ce qu'on a mesuré en cherchant un mois libre, et qui corrige une note du
projet : `commandes_dsn.py` disait cette mémoire valable « au moins cinq
jours ». **Les mois déposés le 15/09 bloquaient encore le 21** — six jours —
et janvier comme juillet bloquaient aussi. « Au moins cinq jours » est un
plancher, pas une durée.

La réponse de la norme à « il en existe déjà une » n'est pas d'attendre :
c'est un **annule-et-remplace**, type 03 avec un numéro d'ordre strictement
supérieur au dernier rang accepté (c'est tout l'en-tête de `dsn.py`, « le
numéro d'ordre est le seul rattachement »). D'où le **compteur d'essais**,
idée de l'éditeur : `--essai N`, et sans valeur il se **compte dans le journal
des dépôts** de la compagnie qui dépose — lui seul sait ce qui est parti sous
ce SIREN, et c'est au SIREN que la plate-forme attache son rang.

**Le compteur n'existe qu'en test, et le moteur le tient** : `produire_mois`
refuse un rang d'essai supérieur à 1 sur un envoi réel. En réel, ce numéro
désigne LA déclaration qu'on remplace ; il se lit sur le certificat de
conformité du dépôt précédent, il ne se compte pas. Un compteur qui
s'incrémente tout seul remplacerait une déclaration acceptée sans que
personne l'ait voulu. Un test verrouille ce refus.

Deux essais de plus ont appris où un annule-et-remplace ne passe pas, sur
janvier :

- `B1-207-05` — *DSN « annule et remplace » impossible le jour de l'échéance.*
- `B1-208-12` — *Date de la déclaration à remplacer échue.*

**Un annule-et-remplace ne vaut que si l'échéance du mois n'est pas passée.**
D'où le choix d'octobre 2026 (échéance le 15/11) : septembre, dont la
première DSN réelle part en octobre, n'a pas été touché.

---

## Le résultat

Sept dépôts en test, sous le SIRET réel, mois d'octobre 2026, rangs d'essai 4
à 10 — chacun produit puis déposé à la suite, le compteur lisant le journal
entre deux.

| compagnie | essai | flux | bilan |
|---|---|---|---|
| `privee-artistes` | 4 | `WS.x7M-XRLD5obABr3C2NmB` | **OK, 0 anomalie** |
| `privee-techniciens` | 5 | `WSwGTQ5o.TgQbHB9ewxirq.` | **OK, 0 anomalie** |
| `privee-mixte-intermittents` | 6 | `WSao7gQIWloBLkJh9SRXBp0` | **OK, 0 anomalie** |
| `privee-creuse` | 7 | `WSO-JJOb8NWwbxP1HGa705v` | **OK, 0 anomalie** |
| `privee-onze` | 8 | `WSJyg8ebKmysrGNhmzoeueA` | **OK, 0 anomalie** |
| `privee-syndeac` | 9 | `WS.BLZHeRFETq6NVfSGmNZa` | **OK, 0 anomalie** |
| `privee-deux-etablissements` | 10 | `WSTz6Y9yetdxbaGRIiuru0O` | **OK, 0 anomalie** |

Tous les contrôles — structure, cohérence, cardinalité **et métier de
l'Urssaf** — passent, sur sept formes de compagnie que le banc d'essai
n'a jamais eues : techniciens seuls, deux populations dans le même envoi,
mois sans aucun salarié, plus de onze salariés, adhérente SYNDEAC, deux
établissements. Les bilans et les accusés sont archivés dans `prive/retours/`.

### Les permanents, le 23 septembre 2026 au soir

Deux dépôts de plus, en annule-et-remplace sur le même mois d'octobre 2026,
**les premières DSN de permanents sous la forme complète** de
`par_nature.permanent` : CASC-SVP d'un permanent (art. 4.22), blocs 54 types
92 et 93, trois bases 31 lues sur le bulletin (prévoyance, santé, CASC ou
FNAS/FCAP), Apec et CET dans le 105, mutuelle patronale dans l'assiette CSG,
et la majoration 100/90 des techniciens intermittents.

| compagnie | ce qu'elle porte | flux | bilan |
|---|---|---|---|
| `privee-tout` | 3090 : deux CDI dont une cadre, un artiste et un technicien intermittents | `WS57D3RxGzPkYtFdso9iwJS` | **OK, 0 anomalie** |
| `publique-tout` | 1285 : un CDI, un artiste et un technicien intermittents, FNAS et FCAP | `WSXHk.ch5ifSrRNxs.ExPrj` | **OK, 0 anomalie** |

Et le 24/09/2026, une de plus (journal §115) : **le premier supplément
conventionnel déclaré à part**. Vingt-cinq ans d'ancienneté, départ volontaire
à la retraite : 3 600 EUR sous 005 (un mois et demi, D1237-1), 1 200 EUR sous
006 (le demi-mois de plus de la 3090, art. 7.9).

| compagnie | ce qu'elle porte | flux | bilan |
|---|---|---|---|
| `privee-depart-retraite` | 3090 : un CDI de 2001, motif 039, blocs 52 types 020, 005 et 006 | `WSAexSrj8Ql6awOtrBotNt-` | **OK, 0 anomalie** |

Un premier essai (`WS3YK5ILteoCbvCRyJWdMij`) est tombé sur `B1-208-11` :
l'ordre 13 du fichier était reparti à 11, recalculé sur le journal de la copie
de développement, qui ne connaissait pas les essais faits depuis `prive/`.
Corrigé : un essai ne part jamais plus bas que l'ordre que le fichier porte
(`operations_declarations.ordre_d_essai`).

Le compte-rendu Urssaf (nature 61) est OK, avec deux remarques non bloquantes
sur chacune : `BA-209-27` (des lignes de cotisation identiques cumulées en
une) et `BA-210-01` (le mandat SEPA de l'IBAN fabriqué, à valider en ligne).
La nature 90 « KO » qui suit est le **formulaire d'adhésion au prélèvement**
(`mon.urssaf.fr/tldp_tlr_formadh/…`), le même lien que pour les sept dépôts du
21/09 : l'IBAN est fabriqué, rien de la déclaration.

Ce que cela ne prouve pas : le montant d'Audiens. Un dépôt de test passe les
contrôles de la plate-forme et de l'Urssaf ; les bases 31 d'une adhésion que
la compagnie n'a pas (fabriquée ici) ne sont rapprochées d'aucun contrat.

### La production audiovisuelle (IDCC 2642), le 25 septembre 2026

La première DSN d'une **autre convention que le spectacle vivant** :
`audiovisuel-techniciens`, un cadreur et une scripte d'un tournage, en
annule-et-remplace sur octobre 2026. Ce qu'elle porte de neuf : l'APPAV
(0,06 % de la tranche 1, accord du 22/02/2010) à la place de la CASC-SVP, en
base 31 + cotisation 059 sous une adhésion Audiens fabriquée (`APPAV`,
référence 108).

| compagnie | ce qu'elle porte | flux | bilan |
|---|---|---|---|
| `audiovisuel-techniciens` | IDCC 2642, l'APPAV sous adhésion Audiens | `WSOGwC-Unam3KmKFP3OT6dW` | **OK, 0 anomalie** ; compte-rendu métier Urssaf OK (un seul avis, non bloquant : le mandat de prélèvement) ; accusé de distribution à l'organisme complémentaire (nature 50) OK |

Ce que cela ne dit pas : que la **référence** d'adhésion soit la bonne. Elle
est fabriquée, et un dépôt de test ne la confronte à aucun contrat Audiens.
Une vraie compagnie de la 2642 la lit sur sa fiche de paramétrage.

### La production cinématographique (IDCC 3097), le 25 septembre 2026

`cinema-tournage` : un électricien de prise de vues et une scripte à
l'heure, une comédienne au cachet, en annule-et-remplace sur octobre 2026
(ordre 16). Ce qu'elle porte de neuf : le CCHSCT (0,04 %) et le paritarisme
(0,045 %), deux contributions patronales sur le brut plafonné à la T2, sous
l'adhésion Audiens ; la prévoyance de l'interbranche pour les non cadres,
l'article 45 étant absorbé par ses taux uniques (journal §137).

| compagnie | ce qu'elle porte | flux | bilan |
|---|---|---|---|
| `cinema-tournage` | IDCC 3097, CCHSCT et paritarisme sous adhésion Audiens | `WScO7oi0YX785.OButPRBEx` | **OK, 0 anomalie** ; compte-rendu métier Urssaf OK (un seul avis, non bloquant : le mandat de prélèvement) |

Même réserve que pour la 2642 : les références d'adhésion sont fabriquées.

### La création et l'événement (IDCC 3252) et la télédiffusion (IDCC 3241), le 27 septembre 2026

Deux compagnies de même forme — un régisseur lumière intermittent à
l'heure, une administratrice cadre et un chargé de production en CDI —, en
annule-et-remplace sur octobre 2026 (ordres 23 et 24). Ce qu'elles portent
de neuf : **aucune contribution de branche** sur le bulletin ni dans la DSN
(leur paritarisme est annuel, hors DSN), et la protection sociale de leurs
**permanents** sous deux adhésions fabriquées.

- `evenement-tout` (3252) : le régime de la branche, prévoyance 0,79 % de la
  T1 moitié-moitié (la cadre au plancher de 1,50 %, tout patronal) et santé
  1,10 % du PMSS moitié-moitié (journal §149).
- `telediffusion-tout` (3241) : le contrat de l'entreprise, que l'annexe 6
  ne chiffre pas — taux fabriqués, 0,80 % non cadres, 1,50 % cadres, santé
  1,20 % du PMSS (journal §150).

| compagnie | ce qu'elle porte | flux | bilan |
|---|---|---|---|
| `evenement-tout` | IDCC 3252, prévoyance et santé de branche des permanents | `WSkes1uSNT1J7VONF9KA8nq` | **OK, 0 anomalie** ; compte-rendu métier Urssaf OK (deux avis non bloquants : `BA-209-27`, lignes identiques cumulées ; `BA-210-01`, le mandat de prélèvement) |
| `telediffusion-tout` | IDCC 3241, prévoyance et santé au contrat de l'entreprise | `WSztGZyow422pSHNzNaIshq` | **OK, 0 anomalie** ; compte-rendu métier Urssaf OK (les deux mêmes avis) |

Le premier essai de `evenement-tout` n'est pas parti : le tunnel sortant du
poste distant s'est fermé au bout de sept secondes (`Connection reset by
peer`) ; le second, identique, est passé. Même réserve que pour la 2642 :
les références d'adhésion, et pour la 3241 les taux du contrat, sont
fabriqués.

## Ce que ce dépôt ne prouve pas

1. **Les compagnies qui refusent n'ont rien déposé**, faute de produire :
   les permanents (barème `par_nature.permanent` non vérifié -- levé le
   23/09/2026, voir plus haut), la CCNEAC
   (couche 1285 relevée en partie), le stagiaire (nature de contrat inconnue).
   Le dépôt ne les concerne pas. Elles étaient sept le matin du 21 septembre ;
   elles sont neuf depuis le soir — voir le point suivant.
2. **Les trois qui produisaient sans garde-fou** — `privee-onze`,
   `privee-syndeac`, `privee-deux-etablissements` — sont passées **OK** comme
   les autres, et c'était tout le point : ce qui leur manquait était un
   **refus du moteur**, pas une anomalie de la norme. Le point de dépôt ne dit
   rien de ce que le logiciel ne dit pas. `privee-onze` a déclaré, avec onze
   salariés, la contribution formation d'une entreprise qui en a moins de
   onze : l'Urssaf l'a acceptée sans un mot, et elle avait raison de le
   faire — la norme ne connaît pas l'effectif de l'employeur.

   **Les trois refusent depuis le soir du 21/09/2026** (`moteur/entreprise.py`,
   et la section `seuils_effectif` du socle) : ces trois dépôts sont donc les
   derniers de leur espèce, et ils ne se rejoueront pas tels quels. Ce qui les
   remplace pour le cas des onze salariés est `privee-onze-surcouchee`, la même
   compagnie dont la surcouche écrit ce que son effectif change — elle produit,
   et **elle n'a pas encore été déposée**. Ce n'est pas un oubli : sa surcouche
   ne touche que `par_nature.permanent` et le versement mobilité, et cette
   compagnie n'emploie **aucun permanent** — pas une ligne de sa DSN ne diffère
   de celle de `privee-onze`, déposée ci-dessus. Le dépôt à faire un jour est
   celui d'une compagnie de onze salariés **avec un permanent**, et il attend
   le même verrou que tous les autres : la section `par_nature.permanent` du
   barème, non vérifiée.
3. **Un dépôt de test n'est pas un dépôt réel.** Les montants ne sont pas
   prélevés, les organismes ne conservent rien, et aucune AER ne reviendra.

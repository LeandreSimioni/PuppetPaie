# Plan : un dépôt public propre, des données qui ne se perdent pas

> Écrit le 21 septembre 2026 au soir, à la demande de Léandre, **sans rien
> implanter**. Il était alors annoncé comme le chantier suivant ; il est
> depuis **en sommeil**, comme `logiciel/AVANT-LE-PREMIER-COMMIT.md` : c'est
> Léandre qui l'ouvre. D'ici là, ce document ne se cite pas à chaque session.

Il répond à cinq demandes, dans l'ordre où elles ont été posées :

1. le logiciel ne doit plus porter nos données réelles — seulement les
   compagnies fictives ;
2. dire **exactement** quoi mettre dans une session à distance pour déposer
   en test ;
3. un dépôt public propre, sans la compagnie du banc d'essai, sans la copie de CoolPaie, relisible,
   avec un historique visible ;
4. ne jamais rien perdre : l'ordinateur disparaît, tout se reprend ;
5. une PR à chaque séance de travail local, pour que rien ne dorme sur un
   seul disque.

---

## La forme du dépôt public : un produit, un atelier (Léandre, 01/10/2026)

Le dépôt public porte **les deux**, séparés au premier coup d'œil :

```
produit/     ce qui s'installe : le binaire Rust (moteur, fabrique, interface)
  donnees/   barèmes, minima, emplois, usages DSN… chacun avec sa source et sa date
atelier/     ce qui sert à le construire : veille KALI, relevés, extracteurs du
             cahier technique, compagnies fictives, tests, comparaison au Python
demo/        les compagnies fictives
docs/
```

- **La frontière, ce sont les fichiers de données.** L'atelier les produit,
  le produit les lit, sans savoir comment ils ont été relevés. Chaque fichier
  dit d'où il vient et de quand date son relevé (`sources.json`) : c'est là
  que se lit la transparence.
- **La mise à jour ne livre que `produit/`.** L'atelier se consulte sur GitHub,
  il ne s'installe pas.
- **Le Python reste à l'atelier tant qu'il sert de référence au Rust**, puis il
  se retire : deux moteurs, ce serait deux vérités.
- `prive/` et les preuves sur données réelles n'y vont jamais.

Le déménagement se fait à l'ouverture du dépôt public, une fois le moteur en
Rust — pas au milieu du portage. Sur un dépôt public, les minutes GitHub
Actions sont gratuites (Windows et Mac compris) ; le dépôt privé de travail
garde la règle « à la fin seulement » (`logiciel/CLAUDE.md`).

### Le produit en trois parties (Léandre, 04/10/2026)

Le produit se livre en **trois parties, chacune versionnée et signée
(Ed25519)**, par la même mise à jour silencieuse :

```
PuppetPaie   le moteur Rust : change rarement
interface/   HTML, JS, CSS, chargés depuis le disque par Tauri (pas
             embarqués) : ils changent chaque semaine pendant les essais
             de novembre-décembre
donnees/     les référentiels, avec leurs sources et leurs dates
compagnies/  à côté : un dossier par compagnie
```

- **Corriger l'interface ne touche jamais le moteur.** Son empreinte
  (`PAIE_SOURCES`, qui couvre `paie/src` et `Cargo.lock`) reste la même :
  aucun mois n'est recalculé, aucune déclaration ne peut en être affectée.
- **L'interface et les données déclarent la version minimale du moteur**
  qu'elles exigent.
- **Tout garde-fou vit dans le moteur** (mot de passe retapé, refus,
  contrôles). Un fichier d'interface se modifie facilement sur le disque : il
  n'autorise donc jamais rien. La page affiche et demande ; le moteur décide.
- Au lot J, seuls deux points de la page changent (voir
  `logiciel/docs/PORTAGE-RUST.md`, lot J) ; le reste ne se porte pas.

---

## D'où vient la vérité, sans CoolPaie (Léandre, 03/10/2026)

**Trois parties, deux publiques.** Le **logiciel** (le produit) et
l'**atelier** deviennent ensemble le dépôt public : rien de réel n'y entre,
seulement des compagnies fictives. **Les données de Léandre** (`prive/`)
restent à part, pour toujours.

**Les tests de rejeu CoolPaie se retirent.** Ils ont servi tant que le
moteur se réglait : CoolPaie était la seule référence extérieure, des DSN
acceptées par l'Urssaf. Le moteur est réglé, le Rust est prouvé contre le
Python, et ces tests ne peuvent pas entrer dans l'atelier public. On ne
fabrique pas de « faux documents du passé » avec notre moteur : ce serait le
moteur qui se teste lui-même, ce que font déjà les compagnies fictives. Les
quelques cas qui ont un jour attrapé une vraie erreur se réécrivent en
fictif dans l'atelier ; le reste de `prive/tests` qui dépend de CoolPaie
part. Les documents CoolPaie restent dans `prive/`, comme archive. (Cela
remplace la règle du 3 c) ci-dessous, « d'abord fabriquer, ensuite
basculer », pour le rejeu CoolPaie.)

**La vérité vient désormais de trois sources :**

1. **La lecture scrupuleuse des textes** : cahier technique DSN, conventions
   collectives, Code du travail, et la veille de l'atelier. C'est la seule
   source en amont : les deux autres arrivent après coup.
2. **Les dépôts de test des utilisateurs.** Un dépôt de test passe les mêmes
   contrôles qu'un dépôt réel : le test vaut le réel. Un annule-et-remplace
   se teste aussi, une normale déposée en test juste avant (la répétition).
   Ce que le test ne contrôle pas, c'est le montant : un taux faux mais
   plausible passe.
3. **Les retours des déclarations réelles sur les postes des utilisateurs** :
   comptes rendus métier, certificats, bilans. Eux seuls disent si le montant
   était juste.

**La boucle.** Une anomalie (en test ou en réel) remonte en ticket dans le
dépôt privé dédié (serveur filet). Dans l'atelier, on la **reproduit sur une
compagnie fictive** (même convention, même situation, faux noms, faux NIR) ;
ce cas devient un test public, puis le logiciel se corrige. **Une donnée qui
arrive d'un ticket ne passe jamais telle quelle dans l'atelier** : elle
redevient fictive avant.

**Toutes les anomalies remontent, et toutes sont pertinentes.** Aucun tri à
la source : une anomalie non bloquante, un avertissement, un code « autre
erreur » sur une déclaration enregistrée en disent autant qu'un rejet. Ce qui
existe (`moteur/incidents.py`, `journal._noter_rejet`) ne fait pas encore
cela ; deux trous relevés le 03/10/2026 :

- **seuls les retours rejetés partent** (`verdict == "KO"`) : un retour
  accepté qui porte des anomalies non bloquantes ne remonte pas ;
- **une DPAE enregistrée ne remonte pas ses anomalies** : le code 99
  (« autre erreur, données facultatives ») de chaque certificat 41 depuis
  juin 2026 n'est jamais arrivé chez l'éditeur.

À faire : toute anomalie de tout retour (DSN, DPAE, et les organismes à
venir), test comme réel, accepté comme rejeté, part au journal des incidents
et de là au dépôt des tickets, en codes seulement (`retours.anomalies_codees`,
jamais un nom ni une valeur). Et vérifier qu'une préversion installée chez
quelqu'un connaît bien le serveur (`EDITEUR.json` dans le paquet) : sans lui,
rien ne part.

---

## 0. Ce que tu as compris — c'est juste, à deux nuances près

Ton résumé tient. Les trois rôles sont bien ceux-là :

| rôle | ce qu'il fait | ce qu'il lui faut |
|---|---|---|
| **éditeur, à distance** | ouvrir une session Claude sur le dépôt, travailler, proposer une PR | le dépôt public, et rien de nous |
| **utilisateur** | télécharger PuppetPaie, s'en servir, recevoir les mises à jour | le logiciel, ses propres données |
| **éditeur, en local** | ce qu'on fait ce soir | le dépôt + `prive/` sur la machine |

Et le découpage sur l'ordinateur est bien `logiciel/` (le dépôt) et `prive/`
(les données, sauvegardées dans le Drive).

**Nuance 1 — « le logiciel est sur le dépôt Git ».** Pas encore. Le dépôt
d'aujourd'hui (`puppetpaieprivate`) est **privé** et porte les deux :
`logiciel/` **et** `prive/`. C'est pour ça qu'une session à distance voit
aujourd'hui nos NIR, nos salaires et nos déclarations. Le chantier 2
ci-dessous est exactement celui-là.

**Nuance 2 — « si je perds l'ordinateur je peux tout reprendre ».** Pas
aujourd'hui, et c'est le point le plus important de ce document. La
sauvegarde **monte** dans le Drive ; **rien ne redescend**. Il n'existe
aucune commande `restaurer`. Voir le chantier 4.

---

## 1. Ce qu'une session à distance doit porter pour déposer en test

### Ce qu'il faut, aujourd'hui

Un dépôt de test n'est pas une déclaration : les contrôles s'exécutent, le
bilan revient, rien n'est conservé. Mais il passe par les **vrais comptes** et
sous le **vrai SIRET** — sinon le contrôle de référentiel `B1-201-01` arrête
tout (« SIRET inconnu »), et aucun contrôle métier ne s'exécute. Il faut donc
deux choses, et elles sont de natures différentes :

**a) Deux mots de passe, dans les variables d'environnement.** Ce sont les
noms que le moteur lit, et ils passent **devant** le coffre :

```
NET_ENTREPRISES_MOTDEPASSE=...      # les DSN (dépôt et retours)
URSSAF_MOTDEPASSE=...               # les DPAE (dépôt et retours)
```

C'est tout pour les secrets. Pas de mot de passe du logiciel : il n'ouvre
qu'un coffre, et il n'y a pas de coffre là-bas — et **un dépôt réel y est
mécaniquement impossible**, puisque la preuve exigée est ce mot de passe.
C'est le garde-fou, et il tient tout seul.

**b) L'identité déclarante.** Le SIRET, le nom et le prénom du déclarant, le
code Urssaf, le numéro de service DPAE. Aujourd'hui ils vivent dans
`prive/config/entreprise.json` — donc une session à distance n'y arrive
qu'en emportant nos données. **C'est ce qu'il faut couper**, et c'est du
code à écrire (chantier 3) :

```
PUPPETPAIE_DECLARANT_SIRET=...
PUPPETPAIE_DECLARANT_NOM=Simioni
PUPPETPAIE_DECLARANT_PRENOM=Leandre
PUPPETPAIE_DECLARANT_CODE_URSSAF=...
```

Le SIRET n'est pas un secret : il est public, sur tout devis et tout contrat.
Ce qui est secret, ce sont les deux mots de passe, et eux seuls.

### Donc, dans l'écran « Modifier l'environnement » de ta capture

- **Nom** : `Puppet` — parfait.
- **Accès réseau** : `Complet` — nécessaire, les dépôts sortent.
- **Variables d'environnement** : les deux mots de passe aujourd'hui, les six
  lignes ci-dessus quand le chantier 3 sera fait.
- **Identifiants API** : rien. Ce ne sont pas des clés d'API.
- **Script de configuration** : `npm install` ne sert à rien ici, c'est du
  JavaScript. Il faudra :

```bash
#!/bin/bash
pip install -r requirements.txt
```

`requirements.txt` existe depuis ce matin et reprend `requirements-tests.txt`.
Sans lui, `cffi` manque, et une poignée de fichiers de tests ne
tombent pas en échec : ils **meurent à l'import**, ce qui ressemble à une
catastrophe et n'en est pas une.

---

## 2. Le dépôt public : une histoire neuve, pas un nettoyage

**Le point qui commande tout : un historique ne se nettoie pas.** Celui du
dépôt actuel porte des NIR, des salaires, des adresses et des déclarations
réelles dans des centaines de commits. Réécrire cette histoire est long,
faillible, et une seule branche oubliée suffit à la rendre inutile.

Donc : **un dépôt neuf, une histoire neuve, un premier commit propre.** Le
dépôt actuel reste ce qu'il est — privé, le registre de ce qui s'est
réellement passé, y compris nos déclarations. On ne le supprime pas, on ne le
publie jamais.

**Confirmé par Léandre le 04/10/2026** : un nouveau dépôt public sain, parti
d'un premier commit propre. Le dépôt actuel (`puppetpaieprivate`) ne sert
plus, ensuite, qu'à **recevoir les tickets d'incident** ; on n'y développe
plus.

`logiciel/AVANT-LE-PREMIER-COMMIT.md` décrit déjà le geste (§4) et les trois
décisions qui restent (licence, `client_secret` Google, nom du dépôt). Ce
plan-ci ajoute ce qu'il ne dit pas.

### Les étapes, dans l'ordre

1. **Déménager.** Le dossier de travail s'appelle encore
   `Documents\coolpaie - Copie` et contient la copie de CoolPaie : `CIRSO/`,
   six dossiers `DSNVAL*`, `kernel_coolpaie`, quarante fichiers `VER
   4.0.0.8.*.txt`, `jre_coolpaie251/`. Rien de tout cela n'appartient à
   PuppetPaie. Le déménagement est un simple déplacement :

   ```
   <le dossier de l'utilisateur>\PuppetPaie\
       logiciel\        (le dépôt public)
       prive\           (nos données, jamais publiées)
   ```

   La copie de CoolPaie reste où elle est, de son côté, tant que CoolPaie est
   payé (décembre). Ce qui la lit chez nous, c'est `importer` et `calibrer` :
   ils prennent un chemin, ils le prendront ailleurs.

2. **Vérifier ce qui partirait.** `paie verifier-avant-commit` sur tout
   `logiciel/`, et lire la liste **en entier**. Il cherche NIR, IBAN, BIC,
   SIRET, identités, adresses — et il lit ce qu'il faut protéger dans
   `prive/`, hors du dépôt, pour ne pas porter lui-même la liste.

3. **Les tests d'abord** (chantier 3). Tant que les tests portent de vraies
   déclarations, le garde-fou crie à chaque passage, et un garde-fou qui crie
   toujours finit ignoré. C'est pour ça que l'ordre compte.

4. **Le premier commit**, avec le hook posé cette fois
   (`git config core.hooksPath .githooks`), et `git status` relu ligne par
   ligne avant de valider.

5. **Les trois décisions.** La licence (aucune n'est choisie), le
   `client_secret` Google (il peut rester — Google l'assume pour les
   applications de bureau —, mais publié il use plus vite le plafond de cent
   comptes), et le nom : `puppetpaie`.

---

## 3. Les tests sur compagnies fictives seules

C'est le chantier qui rend le dépôt public possible, et il est plus gros que
les autres. Trois choses :

**a) Ce qui existe déjà.** `outils/compagnies_fictives.py` monte quinze
compagnies entières sans rien emprunter à `prive/`, avec des SIRET dont la
clé de Luhn tombe juste. Sept ont été déposées en test ce matin, toutes `OK,
0 anomalie`. La fabrique est là ; ce sont les **tests** qui n'y sont pas
encore.

**b) Ce qui manque, et qu'on a découvert ce soir.** Une compagnie fabriquée
ne porte pas `gabarits/due_tel_gabarit.txt` : impossible de produire une
DPAE sans copier ce fichier depuis `prive/`. Le gabarit est un formulaire
officiel, pas une donnée : il a sa place dans le logiciel.

**c) La règle à ne pas oublier, et elle coupe dans l'autre sens.** Un test
qui perd ses valeurs réelles perd sa valeur de preuve. On ne retire une
déclaration réelle d'un test que le jour où une compagnie fictive prouve la
même chose. **D'abord fabriquer, ensuite basculer** — jamais l'inverse.

Et c'est ici que se règle le (b) du chantier 1 : l'identité déclarante lue
depuis l'environnement, pour qu'une session à distance dépose en test sans
qu'un seul fichier de `prive/` existe.

---

## 4. Ne rien perdre — et ce qui manque vraiment

### Ce qui est vrai aujourd'hui

`prive/` monte dans le Drive, par différences, à chaque ouverture. Ce qui n'y
monte pas, volontairement : `coffre.bin`, `google-jeton.bin`, le jeton de
session, les fichiers de travail. Un coffre sur le Drive serait un coffre qui
voyage.

### Trois trous, et le troisième est le plus gênant

1. **Aucune commande ne redescend.** Il faut écrire `python -m moteur
   restaurer <dossier>` : elle retrouve le dossier PuppetPaie du Drive, vide
   ou neuf, et rend `prive/` tel qu'il était. C'est le cœur de ta demande, et
   ça n'existe pas.

2. **La clé de ce poste ne peut pas être sauvegardée** — c'est sa raison
   d'être. Sur une machine neuve, le coffre se rouvre avec **le mot de passe
   du logiciel**, qui repose une clé. Donc : ce mot de passe doit exister
   ailleurs que dans ta tête ou ta mémoire musculaire. Un gestionnaire de mots
   de passe, un papier dans un tiroir — mais quelque part.

3. **`maj-cle-privee.bin` est sauvegardé dans le Drive, et pas `coffre.bin`.**
   Les deux sont scellés de la même façon, par la même clé de poste. L'un est
   exclu, l'autre non. Ce n'est pas dangereux — sans la clé du poste, le
   fichier ne s'ouvre pas — mais c'est **incohérent**, donc c'est un choix que
   personne n'a fait. Il faut le faire : ou les deux montent (et alors la
   perte de la machine ne coûte que le mot de passe du logiciel), ou aucun.
   Je penche pour **les deux montent** : ce sont des fichiers illisibles sans
   une clé qui, elle, ne voyage jamais, et la clé de signature ne se retape
   pas.

### Et ce qu'il faut écrire noir sur blanc

Une page « **si je perds l'ordinateur** », en cinq lignes, dans
`LISEZ-MOI-DABORD.md` : installer PuppetPaie, `restaurer`, taper le mot de
passe du logiciel, reposer les trois mots de passe d'organisme, relier le
Drive. Une procédure qu'on n'a jamais lue le jour où on en a besoin est une
procédure qui n'existe pas — donc on l'**essaie une fois**, sur un dossier
vide, avant d'en avoir besoin.

---

## 5. Une PR à chaque séance

Ce qu'il faut, et c'est peu :

- la séance travaille sur une branche `claude/<sujet>` — c'est déjà le cas ;
- elle finit par un commit, une poussée, une PR ;
- la PR porte le récit de la séance, comme les commits d'aujourd'hui.

Deux choses à trancher :

- **Qui pousse ?** Pousser sort le code de la machine. Aujourd'hui je le fais
  seulement quand tu le dis. Tu peux me donner une autorisation permanente
  pour `logiciel/` une fois le dépôt public — mais **jamais** pour le dépôt
  privé qui porte `prive/`.
- **Et `prive/` ?** Il ne va pas dans le dépôt public. Sa sauvegarde, c'est le
  Drive, pas Git. Une fois les deux dépôts séparés, `prive/` **sort
  complètement** de Git : une seule sauvegarde, celle du Drive, et le
  chantier 4 la rend récupérable.

---

## 6. L'ordre, et ce que ça coûte

| | chantier | pourquoi là | effort |
|---|---|---|---|
| 1 | **`restaurer` + les trois trous** (§4) | c'est le seul risque réel : aujourd'hui, un disque mort coûte tout | un jour |
| 2 | **Le déménagement** (§2.1) | mécanique, et il faut le faire avant de figer des chemins | deux heures |
| 3 | **L'identité déclarante par l'environnement** (§1b) | c'est ce qui permet de travailler à distance sans nos données | une demi-journée |
| 4 | **Le gabarit DPAE dans le logiciel** (§3b) | une ligne, mais elle bloque les DPAE fictives | une heure |
| 5 | **Les tests sur compagnies fictives** (§3) | le gros morceau, et ce qui débloque la publication | plusieurs jours |
| 6 | **Le dépôt public** (§2) | quand 5 est fini, il ne reste que trois décisions | une demi-journée |
| 7 | **La PR par séance** (§5) | une habitude, pas un chantier | rien |

Deux remarques sur cet ordre.

**Le chantier 1 passe devant tout**, y compris devant la publication. Un dépôt
public qu'on prépare pendant six semaines pendant qu'un disque unique porte
cinq ans de paie, c'est l'ordre inverse du risque.

**Et rien de tout cela ne passe devant la DSN de septembre**, déposée en
octobre : c'est la première déclaration réelle sortie du moteur, et elle
commande le portage Rust. Ces sept chantiers se logent autour d'elle, pas à
sa place.

---

## 7. Le premier lancement sur Windows et Mac — RÉGLÉ (30/09/2026)

Un programme non signé, téléchargé par un navigateur, fait afficher un avis
au premier lancement (Windows, Mac). **Décidé : rien à payer.** Le guide
d'installation donne la manière simple — « Débloquer » dans les propriétés
du zip sous Windows, une ligne à coller dans le Terminal sur Mac — et les
mises à jour, téléchargées par le logiciel lui-même, n'affichent rien. La
signature payante (certificat, programme Apple) ne se rouvre que si des
inconnus installent le logiciel sans accompagnement. **Question close : ne
pas la reposer.**

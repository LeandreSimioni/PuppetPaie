# Plan : un dépôt public propre, des données qui ne se perdent pas

> Écrit le 21 septembre 2026, à la demande de Léandre. **Le plan est fait** : le dépôt
> public `PuppetPaie` existe depuis le 14/10/2026 et, depuis le 15/10/2026, c'est lui
> le dépôt de travail ; le privé ne garde que `journal/`, `reference-privee/` et les
> tickets ; les tests tournent sur compagnies fictives seules (règle de fond 9) ; on
> dépose en test depuis le nuage avec la compagnie d'essai (`docs/COMPAGNIE-D-ESSAI.md`) ;
> `paie restaurer` rend `prive/` depuis le Drive. Ne restent ici que les décisions que
> le code cite (« La forme du dépôt public », « D'où vient la vérité », § 4).

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
- Au lot J, seuls deux points de la page changent ;
  le reste ne se porte pas.

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
part. Les documents CoolPaie restent dans `prive/`, comme archive.

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
existe (`paie/src/incidents.rs`, et l'incident d'un retour, `incident` dans
`paie/src/etats.rs`) ne fait pas encore cela ; deux trous relevés le
03/10/2026, toujours vrais le 15/10/2026 :

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
---

## 4. Ne rien perdre — et ce qui manque vraiment

### Ce qui est vrai aujourd'hui

`prive/` monte dans le Drive, par différences, à chaque ouverture. Ce qui n'y
monte pas, volontairement : `coffre.bin`, `google-jeton.bin`, le jeton de
session, les fichiers de travail. Un coffre sur le Drive serait un coffre qui
voyage.

### Trois trous, et le troisième est le plus gênant

**Comblés depuis** : le premier par `paie restaurer` (`restauration.rs`, 13/10/2026) ;
le troisième par la sauvegarde, qui emporte le coffre (`sauvegarde.rs` : Léandre,
13/10/2026, redit le 14/10).

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


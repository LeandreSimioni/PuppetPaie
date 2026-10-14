# Plan : changer de mois instantanément

Écrit le 09/10/2026 sur le poste, à la demande de Léandre, pour la séance en ligne
qui fera le travail. Rien de ce plan n'est codé. Il reprend la décision du 08/10/2026
(`prive/journal/ETAT.md`, « LA VÉRITÉ EN JSON, L'ÉCRAN NE FAIT QUE LIRE ») et en fait
le cahier des charges. Ce plan fait foi.

---

## 1. L'objectif, en une phrase

**Quand on change de mois, l'écran LIT et ne CALCULE rien.** Il affiche le mois en
moins de 50 ms. Les calculs gardent toute leur précision, mais ils se font
**au moment où quelque chose change** (une saisie, une fiche, un barème, un acte
noté au journal, un retour d'organisme), et jamais au moment où on regarde.

> Léandre, 08/10 : « la vérité est dans les JSON, et c'est immédiat ; pourquoi ça
> recalcule à chaque fois ? »
> Léandre, 09/10 : « je veux que tu les retires, juste de la lecture. »

## 2. Ce qu'on croyait fait, et ce qui l'est vraiment

Le plan du 08/10 avait quatre étapes. Deux seulement ont été faites (PR #157) :

| Étape | Où on en est |
|---|---|
| (1) Les valeurs du contrat envoyé gardées en JSON | faite |
| (2) `paies/AAAA-MM.etat.json` écrit après chaque écriture (`paie/src/etat_garde.rs`) | faite, **mais elle ne couvre que 2 des 9 lectures** |
| (3) Une route `mois` qui rend tout d'un coup, **servie dans la fenêtre sans lancer de processus** | **pas faite** |
| (4) L'écran qui ne lit que cette route | **pas faite** |

D'où le ressenti de Léandre : « on dirait qu'on est revenu une version avant ».

## 3. Ce qui se passe aujourd'hui à un changement de mois

Mesuré le 09/10/2026 dans la fenêtre, sur une démo puis sur la compagnie réelle,
avec les `etat.json` déjà faits :

```
clic sur ›
 ├─ 350 ms   attente volontaire (moisVoisin → allerAuMois(v, 350))
 ├─ ~200 ms  « saisie »              ← un processus neuf
 ├─ ~100 ms  « contrats-en-cours »   ← un processus neuf, CALCUL
 └─ ~650 ms  en parallèle, un processus neuf pour chacune de ces 7 demandes :
              etat-du-mois           lecture de etat.json (+ empreinte)
              etat-par-paie          lecture de etat.json (+ empreinte)
              embauches-a-declarer   CALCUL
              arrets                 CALCUL
              fins-de-contrat        CALCUL
              journal-reprise        CALCUL : relit tout le journal (0,75 s sur la cie réelle)
              apercu                 CALCUL : refait la paie du mois entier
total : 1,3 à 1,5 s. Pendant les 600 premières ms, l'écran porte le nom du
nouveau mois au-dessus des cartes de l'ancien (c'est le « je reste sur septembre »).
```

**Un processus qui ne fait rien coûte déjà 0,19 à 0,20 s** (`paie route veille`,
mesuré trois fois). Neuf processus, c'est donc au moins 1,8 s de CPU, avant même
la moindre lecture. Lire `etat.json` ne coûte rien. Ce qui coûte, c'est :

1. **un processus par demande** (`interface/api.rs::route`, un choix hérité du Python, lot J3a) ;
2. **sept demandes sur neuf qui calculent** au lieu de lire ;
3. **l'empreinte** (`etat_garde::empreinte`) : à chaque lecture, elle relit et
   hache toutes les saisies, tout `config/` et `journal/depots.json` (370 Ko) ;
4. **l'attente de 350 ms** et l'ordre en trois temps côté page.

## 4. Le principe

```
   ÉCRITURE (rare)                                LECTURE (à chaque clic)
   ─────────────────                              ───────────────────────
   enregistrer-saisie, fiche, barème,             GET /api/mois?mois=2026-11
   acte au journal, retour d'organisme,                   │
   import, clôture…                                       ▼
        │                                         la fenêtre lit
        ▼                                         paies/2026-11.etat.json
   on CALCULE tout ce que l'écran                 (dans son propre processus,
   du mois affiche, une fois,                      aucun processus lancé)
   et on l'écrit dans                                     │
   paies/AAAA-MM.etat.json                                ▼
   (écriture atomique, déjà en place)             l'écran se peint en une fois
```

- **La vérité ne change pas** : saisie, `calcul.json`, journal, fiches. `etat.json`
  en est le résumé. Ce n'est pas une seconde vérité.
- **La précision ne change pas** : ce sont les mêmes fonctions de calcul, appelées
  au même moment qu'une écriture au lieu de l'être à chaque affichage. Le contenu
  doit être **identique octet pour octet** à ce que rendent les routes actuelles
  (voir le §8).
- **Un état ne peut pas mentir** : s'il ne correspond plus à ses sources, il se
  refait. Mais le contrôle doit être une **lecture de dates de fichiers**, pas un
  hachage du contenu (voir l'étape B).

## 5. Le piège : ce qui dépend du jour

Certaines choses changent sans que personne n'écrive rien, simplement parce que
le jour passe : l'échéance de la DSN, le mois fermé (`hors_delai`), la pastille
« contrat modifié », `remplacement.admis` (annule et remplace), le PAS jugé au jour,
une DPAE en retard.

Aujourd'hui, la date du jour est dans l'empreinte, donc tout se refait une fois
par jour. **La cible** :
- `etat.json` stocke **les dates limites** (échéance DSN, date de fermeture,
  limite de l'annule et remplace…), pas leur verdict ;
- **le verdict se fait à la lecture**, en comparant ces dates à aujourd'hui. Ce
  n'est pas un calcul, c'est une comparaison de dates ;
- avant de coder, lister **chaque** valeur des routes du §3 qui lit
  `ctx.aujourdhui` (grep `aujourdhui` dans `routes_mois.rs`, `routes_listes.rs`,
  `routes_contrats.rs`, `reprise_journal.rs`, `routes_ouverture.rs`) et décider
  pour chacune : date limite stockée, ou rien à faire.

Le brut, s'il dépend du PAS au jour, est un cas à trancher avec Léandre si la
comparaison de dates ne suffit pas.

## 6. Les étapes

### A. Tout l'écran d'un mois dans `etat.json`

`etat_garde::refaire` écrit déjà `paies` (etat-par-paie) et `du_mois`
(etat-du-mois). On y ajoute, **avec les mêmes fonctions qu'aujourd'hui** :

| Clé dans `etat.json` | Tirée de | Remarque |
|---|---|---|
| `saisie` | `routes_saisie::saisie` | avec `existe` et `hors_delai` (en date) |
| `contrats_en_cours` | `routes_contrats::contrats_en_cours_route` | |
| `embauches` | `routes_mois` « embauches-a-declarer » | |
| `fins` | `routes_listes::fins_de_contrat` | |
| `apercu` | `routes_ouverture::apercu` sur la saisie **enregistrée** | les montants des cartes |

Deux lectures ne dépendent pas du mois : `arrets` et `journal-reprise`. Elles vont
dans **un état de la compagnie**, `paies/compagnie.etat.json`, écrit par le même
mécanisme. `journal-reprise` doit cesser de relire tout le journal à chaque fois.

`FORMAT` passe à 2 : les anciens `etat.json` se refont d'eux-mêmes.

### B. L'empreinte devient une lecture de dates

On remplace le hachage du contenu par la liste `(chemin, date de modification,
taille)` des mêmes sources. Le seul coût est un `stat` par fichier, une trentaine
en tout, soit moins d'une milliseconde. On garde dans l'empreinte la version du
moteur (`PAIE_SOURCES`) et l'emplacement du dossier. **On en retire la date du
jour** (voir le §5). Une modification faite à la main est toujours vue, puisqu'elle
change la date du fichier.

### C. Recalculer au bon moment, et seulement ce qu'il faut

- Le refait existe déjà après une écriture qui nomme un mois
  (`routes.rs`, `apres_ecriture`). Il faut le compléter :
  - une écriture qui touche **tous les mois** (fiche salarié, `config/`, barème,
    import, retour d'organisme relevé à l'ouverture) refait **le mois courant et
    le mois affiché**. Les autres mois se refont à leur prochaine lecture, puisque
    leur empreinte aura changé ;
  - un contrat qui déborde sur le mois suivant : écrire en septembre refait aussi
    octobre ;
  - les écritures qui passent par `journal::noter` (actes, dépôts, retours,
    signatures) refont le mois de l'acte.
- **Une lecture qui trouve un état périmé** (une modification à la main, une
  nouvelle version du moteur) **le refait une fois et le dit** :
  `"en_calcul": true`. L'écran affiche alors l'ancien état avec « en calcul… »,
  puis se repeint. On ne bloque jamais l'écran.

### D. La route `mois`, servie DANS la fenêtre

- `GET /api/mois?mois=AAAA-MM` est traité **dans `interface/api.rs`, sans
  `Command::new`** : on lit `etat.json` et `compagnie.etat.json`, on contrôle
  l'empreinte de l'étape B, on fait les comparaisons de dates du §5, et on renvoie.
- Seul ce qui n'a rien à lire passe par un processus : le refait, en arrière-plan,
  quand l'état est périmé.
- Le coffre n'est pas dans l'empreinte : `mot_de_passe_pose` se relit à chaque
  fois, comme aujourd'hui (`etat_garde::route`). C'est une lecture de fichier.
- Les anciennes routes (`etat-par-paie`, `saisie`…) restent pour la ligne de
  commande et pour les autres écrans.

### E. L'écran ne fait plus que cette lecture

Dans `logiciel/interface/interface.js` :

- `chargerMois` fait **un seul** `api("mois", {mois})`, puis peint tout :
  saisie, contrats en cours, actions, embauches, arrêts, fins, états des paies,
  aperçu ;
- **aucun** appel à `apercu` au chargement. L'aperçu se recalcule seulement
  quand on modifie la saisie (c'est un changement, donc un calcul est normal) ;
- **la liste se vide dès que le nom du mois change**, avec « chargement… ».
  L'écran ne montre plus jamais un mois sous le nom d'un autre ;
- l'attente de 350 ms disparaît pour un mois déjà en mémoire. Pour un mois neuf,
  on la ramène à environ 120 ms, juste ce qu'il faut pour regrouper des clics rapides ;
- `MEMO_MOIS` et `relireEnFond` deviennent presque inutiles. On les simplifie,
  sans les garder par habitude ;
- `rafraichirActions` et les repeints après une écriture relisent aussi `mois`.

## 7. Ce qu'on ne touche pas

- Les calculs eux-mêmes (barèmes, DSN, bulletins) : aucune formule ne change.
- Les écritures : elles gardent leur processus et leur journal.
- La règle du 08/10 : **la fenêtre ne garde rien en mémoire**, elle relit les fichiers.
- `prive/` : les essais se font sur `prive-dev` ou sur les démos, jamais sur la
  compagnie réelle (voir la mémoire « l'atelier ne pollue pas la cie réelle »).

## 8. Comment on saura que c'est fini

1. **Même contenu** : pour chaque mois des quatre démos, de `prive-dev` et de la
   copie figée des tests, ce que rend `mois` est identique à ce que rendaient les
   neuf routes d'avant (un test Rust qui compare les deux, clé par clé).
2. **Temps** : un changement de mois dans la fenêtre, mesuré comme le 09/10
   (sonde sur `fetch` et sur le DOM, voir le §3), **moins de 50 ms** entre le
   clic et l'écran peint, une seule requête, aucun processus lancé
   (`tasklist` stable pendant le défilement).
3. **Jamais faux** : on modifie une saisie à la main, puis on change de mois →
   « en calcul… », puis le bon état. On avance l'horloge de la démo d'un jour
   au-delà d'une échéance → le mois passe fermé **sans recalcul**.
4. **Jamais l'ancien mois sous le nouveau nom** : on clique cinq fois vite sur ›
   et on vérifie, à chaque instant, que le nom affiché correspond aux cartes ou
   à « chargement… ».
5. `cargo test` vert, puis exe Windows refaits par mingw et commités dans
   `logiciel/paie/bin/windows/` (jamais le workflow GitHub « Moteur Rust »).

## 9. L'ordre de travail conseillé

1. Le §5 d'abord : la liste de ce qui dépend du jour. Sans elle, le reste ment.
2. Étape A, puis le test d'égalité du §8.1.
3. Étape B.
4. Étape D, la route dans la fenêtre.
5. Étape E, l'écran.
6. Étape C, le recalcul au bon moment, puis les essais du §8.3.
7. Les exe, l'`ETAT.md`, la PR.

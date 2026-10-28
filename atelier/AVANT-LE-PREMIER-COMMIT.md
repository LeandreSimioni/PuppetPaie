# Avant le premier commit

> **RÉVEILLÉ LE 09/10/2026 (Léandre : « on va préparer l'envoi sur le dépôt
> public »).** Le Python est retiré du produit (lot K) ; la section 0 dit où en
> est le terrain.
>
> **FAIT LE 14/10/2026 (Léandre : « on met tout sur le dépôt public »)** : le dépôt
> `LeandreSimioni/PuppetPaie` (créé le 04/10, vide) reçoit son premier commit :
> tout ce que Git suivait dans le dépôt de travail, sauf `prive/`, `atelier/prive-dev/`, `import/`, les deux fichiers
> scellés du bac à sable, `logiciel/donnees/google-application.json` (le
> `client_secret`, §3 : un secret GitHub) et `atelier/PUPPETPAIE-TEL-QU-IL-EST.md` (le
> récit du banc d'essai). **Depuis le 15/10/2026, le public est le dépôt de travail** ;
> le privé ne garde que `journal/`, `reference-privee/` et les tickets. **La licence est posée** :
> `LICENCE` (texte français) et `LICENCE-EN` (texte anglais) de l'EUPL v. 1.2, relevés
> le 14/10/2026 sur `joinup.ec.europa.eu` (`.../inline-files/EUPL v1_2 FR.txt` et
> `.../custom-page/attachment/2020-03/EUPL-1.2 EN.txt`), BOM et CRLF retirés, texte
> inchangé ; les trois `Cargo.toml` disent `license = "EUPL-1.2"`.

## 0. Le terrain au 09/10/2026

**Ce qui est prêt**
- Le produit est en Rust seul : `paie/` (moteur), `fenetre/` (Tauri),
  `donnees/` (référentiels), `interface/` (la page). Suite : `cargo test` vert.
- Les tests ne lisent plus `prive/` ni `demo/` : ils montent leurs propres
  compagnies de test.
- Le nom de la compagnie réelle n'apparaît plus dans le code (un test de
  signature le portait, corrigé).

**Ce que le garde-fou relève** (alors `python3 atelier/outils/verifier_avant_commit.py`, porté en Rust
le 15/10/2026 : `paie verifier-avant-commit` ;
réveillé le 14/10/2026, lot N1) : il lit désormais tout ce que Git emporterait
hors de `prive/` (`demo/`, les `config/`, `.dsn` et `.xml` compris), reconnaît
un NIR inventé à la marque du verrou de la suite (`999`, `123456`, `000`) et
un NIR écrit avec sa clé. **Hors d'`atelier/prive-dev/`, il ne trouve plus
rien.** Dans `atelier/prive-dev/` (la copie de développement : à sortir du
dépôt public, Léandre tranche), 5 672 occurrences dans 954 fichiers, dont
3 779 valeurs réelles de `prive/config/`. Limite : un vrai NIR absent de
`salaries.json` (un ancien salarié) ne serait reconnu que par le motif
générique ; les PDF (`import/`) ne se lisent pas.

**Ce qui pèse** : `paie/bin/` (65 Mo, les programmes compilés commités),
`paie/tests/donnees/` (47 Mo, dont 28 Mo de démos figées). Sur un dépôt
public, les programmes se livrent par des *releases* et non dans l'historique ;
les démos figées se remplacent par des démos rejouées par `vie_fictive` quand
elles seront refaites.

**Le produit et l'atelier ne sont pas encore séparés** : la mise à jour
(`paie maj --publier`, `maj::archiver`) emporte tout `logiciel/`, sources,
tests et outils compris. La forme visée (`PLAN-DEPOT-PUBLIC.md` : `produit/`,
`atelier/`) reste à faire.

**L'atelier garde du Python, à porter** : `outils/` (extracteurs de la norme
DSN, relevés KALI/DILA, tables NFKC, garde-fou). La vérification des sources
(`sources --verifier`, chez l'éditeur) vivait dans le moteur Python : elle n'existe
plus et sera à réécrire en Rust. Le serveur du filet est un service à part,
déployé sur le VPS du site, en Python : il vit dans le dépôt du site
de l'éditeur (`infra/filet/`, depuis le 13/10/2026), hors de ce dépôt.

**Décidé le 08/10/2026** (`ETAT.md`) : **licence EUPL 1.2** (le texte
officiel est à poser en `LICENCE` — pas encore fait : à prendre sur le site de
la Commission, pas à recopier de mémoire) ; **signature des exe par SignPath
Foundation**, qui demande, dans l'ordre : le dépôt public sain, l'EUPL dans
chaque dépôt, une page « Code signing policy » sur le site de l'éditeur, la double
authentification de chaque membre, une option pour couper l'envoi des
incidents à l'éditeur.

**Restent à Léandre** : le `client_secret` Google (`donnees/google-application.json`,
section 3 ; un secret GitHub) ; le nom du dépôt est `PuppetPaie`, créé
le 04/10/2026. **Et ce que le public laisse derrière, à décider** : les textes déposés
(`atelier/reference/`, que les `_source` du code citent) sont dans le dépôt public
depuis le 15/10/2026, sauf l'original d'un organisme privé (seule sa transcription y
est), les exemples réels et ce qui porte une donnée de la compagnie ; `atelier/site/` nomme l'éditeur (mentions légales) ; les démos
portent des chemins du poste (suspens du verrou, `PASSATION-N1.md`). L'identifiant de la fenêtre est
tranché depuis le 13/10/2026 : `fr.puppetpaie.fenetre` (`fenetre/tauri.conf.json`,
du nom du site de l'éditeur) ; le changer encore déplacerait le dossier de
données de la fenêtre sur chaque poste.

---

## 1. Ce qui a été retiré, et comment

Le principe : **retirer les identités, garder les chiffres.** Les montants
sont ce qui prouve que le moteur est calé au centime ; les noms n'apportent
rien. Un test qui perd ses valeurs réelles perd sa valeur de preuve — c'était
le piège à éviter.

| Fichier | Ce qu'il portait | Ce qui a été fait |
|---|---|---|
| `tests/test_retours.py` | les deux NIR complets, le SIRET, un compte-rendu DGFiP réel | le CRM réel est devenu un **gabarit** : sa structure est intacte, ses NIR sont des marqueurs remplacés **au moment du test** par ceux de la configuration, qui n'est pas versionnée |
| `tests/test_interface.py` | deux NIR en dur pour tester la clé | le test lit désormais les fiches et vérifie que **chaque clé déclarée correspond à son NIR** — plus fort qu'avant |
| `tests/test_non_regression.py` | une adresse de courriel réelle | lue depuis la configuration |
| `moteur/retours.py` | un NIR réel dans deux exemples | NIR fabriqué |
| `moteur/champs.py` | une adresse réelle en exemple | adresse inventée |
| `moteur/conges_spectacles.py` | le SIRET en exemple | SIRET fabriqué (clé de Luhn juste) |
| `moteur/netentreprises.py` | « NOM PRÉNOM » du déclarant | formulation générique |

Les valeurs fabriquées ont **des clés qui se vérifient** : un test ne doit pas
pouvoir passer sur un numéro impossible. Elles sont listées, avec leur raison
d'être, dans `FICTIVES` de `paie/src/verifier_avant_commit.rs` (en tête de
`outils/verifier_avant_commit.py` avant le 15/10/2026).

Vérifié après nettoyage : **117 tests verts**, et `python -m moteur calibrer`
rend exactement le même rapport qu'avant.

## 2. Le garde-fou, en place

`paie verifier-avant-commit` (`paie/src/verifier_avant_commit.rs`, porté le 15/10/2026 depuis
`outils/verifier_avant_commit.py`) cherche NIR, IBAN, BIC, SIRET, identités et
adresses dans tout ce qui partirait. Il **ne contient aucune donnée
sensible** : il lit ce qu'il faut protéger dans `prive/config/`, hors du
dépôt. Un garde-fou qui porterait la liste des NIR à protéger serait
exactement ce qu'il prétend empêcher.

Deux façons de s'en servir, et il en faut deux — un hook s'installe, et peut
ne pas l'être :

```bash
git config core.hooksPath atelier/.githooks     # refuse le commit fautif
paie verifier-avant-commit                       # à la demande, sur ce qui part
```

Le dépôt de travail est public depuis le 14/10/2026 : le crochet se pose sur chaque
poste qui commite (la commande ci-dessus, une fois), et `tests/fichiers/donnees_reelles.rs`
double le garde dans la suite, sans lire `prive/`.

Deux pièges rencontrés en l'écrivant, corrigés, à ne pas réapprendre :

- le motif du BIC (`[A-Z]{4}FR…`) trouvait le mot **« chiffres »** dans une
  phrase française — 56 fausses alertes. Un BIC est en capitales : la
  recherche ne doit pas ignorer la casse ;
- le motif du NIR acceptait un mois `60` et lisait l'horodatage
  `20260906080000` comme un numéro de sécurité sociale.

Un garde-fou qui crie au loup à chaque passage finit ignoré le jour où il a
raison. C'est pour cela que ces deux corrections comptent.

## 3. Ce qui reste à décider

- **La licence** : EUPL 1.2 (Léandre, 08/10/2026).
- **Le `client_secret` Google** (`donnees/google-application.json`) — **DÉCIDÉ (Léandre, 14/10/2026) : il est livré dans chaque Release, mais pas dans le code public : c'est le secret GitHub `PUPPETPAIE_GOOGLE_APPLICATION`, posé par le flux « Publier ».** Google
  l'assume pour les applications de bureau, qui ne peuvent rien garder
  secret, et il ne donne accès à rien sans l'autorisation d'un utilisateur. Ce
  qu'il porte, c'est notre quota et le plafond de 100 comptes — publié, il
  s'use plus vite. Décision à prendre en connaissance de cause, pas par défaut.
- **Le nom du dépôt** : `puppetpaie`, sans doute.

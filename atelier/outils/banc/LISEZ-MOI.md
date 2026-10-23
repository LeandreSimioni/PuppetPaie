# Le banc d'essai daté (#236, phase 2)

Faire vivre une compagnie dans l'interface, comme un testeur, en avançant le calendrier — et que tout ce qu'on y fait
s'enregistre dans un **scénario** rejouable sans navigateur (`paie/src/scenario.rs`). Le banc ne décide rien : il lance
le binaire de l'atelier (`paie-atelier`, qui refuse tout acte réel) sur une copie d'installation **bridée** neuve, et lui
parle par `/atelier/<geste>` (`paie/src/interface/mod.rs`, `Fenetre::atelier`).

## La notice d'un agent qui joue au banc

**0. Le programme** (une fois, par l'orchestrateur ; plusieurs bancs le partagent) :

```bash
cd atelier/sources/paie && CARGO_TARGET_DIR=<cible>-atelier cargo build --profile essais --features atelier --bin paie-atelier
export PUPPETPAIE_ATELIER=<cible>-atelier/essais/paie-atelier
```

**1. Lancer** une installation neuve au jour où la compagnie commence (un dossier hors du dépôt, à toi seul) :

```bash
node atelier/outils/banc/banc.js lancer /tmp/banc-<nom> --jour 2026-11-02
```

Le banc copie `logiciel/` et `fictives/` du dépôt dans `/tmp/banc-<nom>/installation/` (bridée : aucun `.edition.json`),
lance la fenêtre sans navigateur, rien ne sort de la machine (`PUPPETPAIE_ESSAIS_HORS_RESEAU`). L'heure simulée est
9 h (Paris) ce jour-là, et elle coule ensuite au pas de la montre.

**2. Jouer dans la page**, par un script Playwright en Node (jamais Python), qui commence ainsi :

```js
const banc = require("<dépôt>/atelier/outils/banc/banc.js");
const { page, fermer } = await banc.ouvrir("/tmp/banc-<nom>", { compagnie: "<nom de ta compagnie>" });
// ... page.click, page.fill, page.selectOption : ce qu'un testeur ferait ...
await banc.capture(page, "/tmp/banc-<nom>/captures/01-quelque-chose.png");
await fermer();
```

`banc.ouvrir` ouvre un Chromium sans tête sur la fenêtre (le jeton est posé), règle l'horloge du navigateur sur le jour
simulé, et, si l'accueil s'affiche, clique la tuile de `compagnie`. Un script court par étape ; la fenêtre, elle, reste
lancée entre deux scripts. Le modèle complet, du « + » à la DSN : `exemple.js` (à côté).

**3. Créer sa compagnie par « + »**, comme un testeur : la tuile « Ajouter une compagnie » (`.tuile.ajouter .invite`), le
message (« Continuer », `#avis-continuer`), le nom (`.tuile.ajouter input`), « Créer et ouvrir ». Elle est alors une
**fictive ajoutée** : tout s'y fait, mais ses envois (DPAE, DSN, contrats) sont refusés.

**4. Rendre ses envois simulés** — le geste d'atelier, une fois, la compagnie ouverte dans la page :

```bash
node atelier/outils/banc/banc.js simuler /tmp/banc-<nom>
```

Elle devient une fictive aux envois simulés : son horloge joue désormais ce qui est dû (DPAE le jour de l'embauche, le
mois le 1er du mois suivant, la DSN à l'échéance, les documents), les dépôts sont joués, les retours reviennent.

**5. Avancer le calendrier** (vers l'avant seulement), la compagnie ouverte :

```bash
node atelier/outils/banc/banc.js jour /tmp/banc-<nom> 2026-12-16
```

Le moteur passe à ce jour, 9 h ; la veille de la compagnie passe : l'horloge joue tout ce qui était dû entre-temps et le
dit (une ligne par geste joué, « — non joué (…) » quand elle a refusé : c'est à noter). Puis, dans la page,
`page.reload()` (ou un nouveau `banc.ouvrir`). En script : `await banc.jour("/tmp/banc-<nom>", "2026-12-16")`.

**6. Le reste** :

| | |
|---|---|
| `node banc.js etat <banc>` | le jour et l'heure simulés, le scénario, la compagnie ouverte, ses envois simulés ou non |
| `node banc.js capture <banc> <fichier.png>` | une capture de ce que la fenêtre montre (pour un écran précis : `banc.capture(page, …)` dans ton script) |
| `node banc.js arreter <banc>` | arrête la fenêtre ; installation et scénario restent |
| `node banc.js verifier <banc>` | le rejeu du scénario refait-il la compagnie du banc ? (voir 8) |
| `node banc.js lancer <banc>` (sans `--jour`) | relance un banc arrêté : le scénario reprend une minute après son dernier geste |

**7. Les règles** :

- **Tout passe par la page.** Le scénario n'enregistre que ce que la fenêtre sert : un fichier de la compagnie modifié à
  la main ne s'y trouve pas, et le rejeu ne le refera pas. Si l'écran ne permet pas un geste, on peut appeler, depuis la
  page, la route qu'elle appellerait (`await page.evaluate(() => api("<route>", {...}))`) : c'est enregistré aussi —
  mais **on le note** dans sa passation, c'est un ticket (voir « Ce qui bloque »).
- Un fichier à déposer (un PDF, une DSN, un export à importer) se pose dans le dossier que la page désigne (`import/` de
  la compagnie, ou un dossier tapé dans le champ) : le scénario en garde une copie (`scenario/pieces/`).
- Le calendrier n'avance que vers demain ; on ne retouche pas l'heure de la machine.
- Ce qu'on fait dans une fictive livrée (Les Deux Rives…) s'enregistre mais ne se rejoue pas : seule la compagnie créée
  par « + » est rejouée.
- À la fin : `verifier`, puis `arreter`, toujours : aucune fenêtre ne reste derrière soi.

**8. Vérifier, à la fin de la partie** (la preuve que le scénario suffit) :

```bash
node atelier/outils/banc/banc.js verifier /tmp/banc-<nom>
```

Le scénario se rejoue sans navigateur dans `/tmp/banc-<nom>/verification/`, et la compagnie qui en sort se compare,
fichier par fichier, à celle du banc (le banc tire les mêmes aléas que le rejeu). « Le rejeu refait la compagnie du
banc » : c'est bon. Une ligne `DIFFERENCE` nomme un fichier : quelque chose a écrit sans passer par la fenêtre, ou le
rejeu s'écarte — à mettre dans la passation, tel quel.

Les mêmes commandes, sans le banc (`paie-atelier scenario …`, ou l'exemple `vie_fictive` du moteur ordinaire) :

```bash
paie-atelier scenario rejouer  <scénario> <installation neuve> [--lectures]   # dit ce que l'horloge a joué, ECART sinon
paie-atelier scenario verifier <scénario> <installation du banc> <dossier de travail>
paie-atelier scenario elaguer  <scénario> <scénario sans lectures>            # pour le figer (phase 4)
cd atelier/sources/paie && CARGO_TARGET_DIR=<cible> cargo run --profile essais --example vie_fictive -- rejouer <scénario> <dossier neuf>
```

## Ce que contient un banc

```
/tmp/banc-<nom>/
  installation/        logiciel/, fictives/ (dont fictives/ajoutee-<ta compagnie>/)
  scenario/            gestes.jsonl (un geste par ligne) et pieces/ (les fichiers déposés) : le scénario
  banc.json            la fenêtre lancée (pid, port, jeton) ; fenetre.log son journal
  local/, cle/         le dossier de la fenêtre et la clé du poste, propres au banc
```

## Ce qui bloque

Rien, sur le chemin d'une compagnie neuve, du « + » à la DSN de son premier mois : `exemple.js` le fait par la seule
page, sans `api(...)` (rejoué le 23/10/2026, lot A du #236). Ce que le modèle y montre :

- « Pour démarrer » (en tête de Configuration › Entreprise) dit ce qui manque encore ; chaque ligne mène à sa case,
  visible et le curseur dedans (le code du service de santé au travail, la taxe sur les salaires…), sans quitter la
  fiche en cours de saisie.
- La qualité du contact DSN ne se demande pas : le moteur écrit le type 04 (`dsn::type_du_contact`).
- Une fiche salarié s'ouvre sur « Métier » ; l'intitulé d'emploi apparaît, prérempli, quand le métier est choisi, et
  un enregistrement sans métier refuse en disant « Métier : à choisir dans la liste ».

Un obstacle relevé en jouant s'écrit ici, avec le geste qui le contourne, jusqu'à ce qu'il soit levé.

## Pour le rejeu en test

Un scénario qu'on fige devient un test (`paie/tests/rejeu.rs`, `tests/outils/rejeu.rs`) : `scenario elaguer` l'écrit
sans ses lectures (le rejeu les écarte déjà ; le test `un_scenario_elague_se_rejoue_a_l_identique` le prouve) dans
`paie/tests/donnees/scenarios/<nom>/`, puis on ajoute le test sur le modèle des Tréteaux du Banc ; l'oracle s'écrit par
`PUPPETPAIE_REECRIRE_ORACLES=1`.

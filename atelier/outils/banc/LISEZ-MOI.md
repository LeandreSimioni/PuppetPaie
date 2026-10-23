# Le banc d'essai daté (#236, phase 2)

Faire vivre une compagnie dans l'interface, comme un testeur, en avançant le calendrier — et que tout ce qu'on y fait
s'enregistre dans un **scénario** rejouable sans navigateur (`paie/src/scenario.rs`). Le banc ne décide rien : il lance
le binaire de l'atelier (`paie-atelier`, qui refuse tout acte réel) sur une copie d'installation **bridée** neuve, et lui
parle par `/atelier/<geste>` (`paie/src/interface/mod.rs`, `Fenetre::atelier`).

## La notice d'un agent qui joue au banc

**0. Le programme** (une fois, par l'orchestrateur ; plusieurs bancs le partagent) :

```bash
cd atelier/sources/paie && CARGO_TARGET_DIR=<cible> cargo build --profile essais --features atelier --bin paie-atelier
export PUPPETPAIE_ATELIER=<cible>/essais/paie-atelier
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
- `arreter` à la fin, toujours : aucune fenêtre ne reste derrière soi.

**8. Rejouer** ce qu'on a joué, sans navigateur, dans un dossier neuf (la preuve que le scénario suffit) :

```bash
cd atelier/sources/paie && CARGO_TARGET_DIR=<cible> cargo run --profile essais --example vie_fictive -- /tmp/banc-<nom>/scenario /tmp/rejeu-<nom>
```

Il dit ce que l'horloge a joué, le temps du rejeu, et `ECART` pour toute route qui ne se passe pas comme au banc.

## Ce que contient un banc

```
/tmp/banc-<nom>/
  installation/        logiciel/, fictives/ (dont fictives/ajoutee-<ta compagnie>/)
  scenario/            gestes.jsonl (un geste par ligne) et pieces/ (les fichiers déposés) : le scénario
  banc.json            la fenêtre lancée (pid, port, jeton) ; fenetre.log son journal
  local/, cle/         le dossier de la fenêtre et la clé du poste, propres au banc
```

## Ce qui bloque (relevé en jouant le modèle, 23/10/2026)

- La qualité du déclarant DSN n'est pas dans la fiche Entreprise (elle « vaut 04 » et reste dans la fiche), mais une
  compagnie neuve ne l'a pas, et « Pour démarrer » l'exige : contournement par la route `enregistrer-fiche` (`exemple.js`).
- « Code du service de santé au travail » bloque la DPAE, mais il est replié sous « Avancé — cas particuliers ».
- Le métier d'un salarié se choisit dans la liste `#config-metier-choix` ; le champ libre « Emploi » ne suffit pas.

## Pour le rejeu en test

Un scénario qu'on fige devient un test (`paie/tests/rejeu.rs`, `tests/outils/rejeu.rs`) : copier `scenario/` dans
`paie/tests/donnees/scenarios/<nom>/`, ajouter le test sur le modèle des Tréteaux du Banc, l'oracle s'écrit par
`PUPPETPAIE_REECRIRE_ORACLES=1`.

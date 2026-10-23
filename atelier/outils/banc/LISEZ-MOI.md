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

## Une compagnie qui vient d'un logiciel précédent

Une compagnie qui entre par « D'un logiciel précédent » importe, par la page, les bulletins de son ancien logiciel. Ces
bulletins sont **fictifs** : l'orchestrateur les fabrique d'une description JSON, au gabarit CoolPaie ou sPAIEctacle
que l'import sait lire, un PDF par bulletin (`paie/src/atelier/fabriquer_bulletins.rs`, la fabrique même des tests) :

```bash
paie-atelier fabriquer-bulletins description.json /tmp/banc-<nom>/bulletins --gabarit spaiectacle
```

Puis le joueur les dépose dans le dossier que la page désigne (`import/` de la compagnie, ou le champ du dossier), comme
tout fichier (règle 7 ci-dessus). L'outil calcule ce qu'un ancien logiciel aurait imprimé : le brut (quantité × taux),
des cotisations **vraisemblables** (taux d'un faux bulletin, ni relevés ni recalculés : l'import ne recalcule rien, le
bulletin fait foi) mais **cohérentes** (brut − cotisations salariales + non soumis = net avant impôt ; − PAS = net à
payer), le net imposable, le PAS au taux donné, et les cumuls de l'année par salarié. Il refuse, avec sa raison, ce que
l'import ne reprendrait pas entier : un libellé de ligne que le gabarit ne connaît pas ou sans rubrique sûre (pas de
contrat), une ligne non soumise sur CoolPaie, un NIR ou un Siret à clé fausse, des dates hors de la période.

Une description minimale (identité inventée, règle 9 a : Siret de la famille `123456…` à clé de Luhn juste, NIR de la
commune 999 à clé juste ; rien de réel) :

```json
{
  "gabarit": "coolpaie",
  "entreprise": {
    "raison_sociale": "La Fanfare des Toits", "adresse": "7 rue des Gouttières", "code_postal": "44000", "commune": "Nantes",
    "siret": "12345678200010", "naf": "9001Z", "urssaf": "527000000001", "licence": "PLATESV-R-2020-000123",
    "convention": {"idcc": "3252", "libelle": "Spectacle vivant, création et événement"}
  },
  "salaries": [
    {"code": "1", "nom": "Scaramouche", "prenom": "Tiberio", "nir": "185079999912388", "naissance": "1985-07-14",
     "adresse": "3 quai de la Fosse", "code_postal": "44000", "commune": "Nantes", "conges_spectacles": "Z1234567"},
    {"code": "2", "nom": "Colombine", "prenom": "Isabella", "nir": "290039999900145", "naissance": "1990-03-02",
     "adresse": "12 rue Kervégan", "code_postal": "44000", "commune": "Nantes"}
  ],
  "bulletins": [
    {"salarie": "1", "emploi": "MUSICIEN", "statut": "artiste", "debut": "2025-03-10", "fin": "2025-03-14",
     "dates": [["2025-03-10", "2025-03-11"], ["2025-03-14", "2025-03-14"]],
     "objet": {"numero": "256Z00920031", "titre": "Les Toits en Fanfare"},
     "lignes": [{"libelle": "Cachets isolés", "quantite": 3, "taux": 146}], "taux_pas": 2.5},
    {"salarie": "2", "emploi": "CHARGEE DE DIFFUSION", "statut": "non cadre", "regime": "permanent",
     "debut": "2025-03-01", "fin": "2025-03-31", "lignes": [{"libelle": "Heures", "quantite": 86.67, "taux": 14.5}]}
  ]
}
```

| clé d'un bulletin | |
|---|---|
| `salarie` | le `code` d'un salarié de la liste |
| `emploi`, `statut` | imprimés tels quels ; `statut` « artiste » fait une fiche d'artiste à l'import |
| `regime` | `intermittent` (défaut : chômage des annexes 8 et 10, Congés Spectacles) ou `permanent` |
| `debut`, `fin` | la période ; le mois du bulletin est celui de `fin` |
| `dates` | les jours travaillés, en paires [début, fin] (défaut : toute la période) ; ils font les jours du contrat |
| `objet` | `{"numero", "titre"}` : le spectacle (numéro d'objet), pour un CDDU |
| `lignes` | `{"libelle", "quantite", "taux"}` ; libellés du gabarit : `Cachets isolés`, `Cachets groupés`, `Heures`, `Service(s) répét. 4h (contrat<5j)` et `Masterclass / Action Culturelle` (CoolPaie), `Service de répétition 4 heures` (sPAIEctacle) |
| `non_soumis` | sPAIEctacle seulement : `Défraiement non soumis`, `Prime de panier non soumise`, `Acompte` |
| `taux_pas`, `regle_le`, `numero`, `analytique` | facultatifs : le taux du PAS (0), le jour du règlement (`fin`), le numéro CoolPaie (`k/année`), le code analytique sPAIEctacle |

Le gabarit se donne dans la description ou par `--gabarit` (qui l'emporte). Les plafonds de la Sécurité sociale imprimés
sont connus pour 2024 à 2026 ; une autre année : `"plafonds": {"2023": {"mensuel": 3666, "journalier": 202}}`. Ce que
l'import ne lit pas d'un bulletin (la nature du contrat : CDI, CDD, CDDU ; la qualité de cadre) reste à configurer dans la
page après l'import.

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

# Comment ce dépôt est rangé (15/10/2026)

**PuppetPaie est un logiciel de paie du spectacle vivant, pour toute compagnie qui emploie des artistes et des
techniciens**, intermittents au cachet comme salariés permanents. Moteur Rust (`moteur`) et fenêtre (`puppetpaie`),
multiplateforme. Les consignes complètes d'un agent sont dans `CLAUDE.md`, à côté.

Deux dépôts, et une frontière à retenir : **le code est public, les données d'une compagnie ne sont dans aucun dépôt.**

```
PuppetPaie/  (ce dépôt, public)
├── PuppetPaie.cmd      le lanceur
├── logiciel/           exactement ce qu'une compagnie reçoit : la fenêtre, VERSION, donnees/ (barèmes, bases,
│                       modèles), interface/ (la page HTML, CSS, JS)
├── fictives/           les quatre compagnies fictives livrées, sur lesquelles on essaie l'interface
├── maj/                (poste seulement) la mise à jour de l'installation
├── prive/              (poste seulement, jamais dans Git) la compagnie du poste : config, paies, sorties, retours
└── atelier/            tout le reste :
    ├── sources/        moteur/ (le moteur ; exécutables dans moteur/bin/<système>/), fenetre/ (Tauri), fabrique-pdf/
    ├── reference/      les textes qui fondent chaque règle (un LISEZ-MOI par dossier)
    ├── docs/           ARCHITECTURE, RUBRIQUES, COMPAGNIES-FICTIVES, COMPAGNIE-D-ESSAI
    ├── outils/         les outils d'atelier (relevés, extracteurs, garde-fou de publication)
    ├── depot/, installeur/, site/, maquettes/
    └── README.md       comment chaque commande s'utilise

puppetpaieprivate/  (le dépôt privé)
├── journal/            ETAT.md (le point de reprise, une page), passations, topos, archives/
├── reference-privee/   les textes lus qu'on ne republie pas (originaux d'organismes, pièces réelles)
└── (ses issues)        les tickets : ceux de Léandre, et ceux du filet (retours, incidents)
```

Sur le poste d'une compagnie, le dépôt privé n'existe pas : `logiciel/`, `fictives/`, `maj/` et `prive/` vivent côte à
côte, dans `Documents\PuppetPaie` ou dans un dossier portable (`PORTABLE.json`, une clé USB).

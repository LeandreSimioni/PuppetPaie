# Comment ce dépôt est rangé

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
├── prive/              (poste seulement, jamais dans Git) la compagnie du poste : config, engagements, paies, sorties, retours, journal
└── atelier/            tout le reste :
    ├── sources/        l'espace Cargo : socle/, droit/, engagement/, documents/, declarations/, envois/, moteur/
    │                   (le programme moteur ; exécutables livrés dans moteur/bin/<système>/) ; fenetre/ (Tauri),
    │                   fabrique-pdf/
    ├── reference/      les textes qui fondent chaque règle (un LISEZ-MOI par dossier)
    ├── docs/           ARCHITECTURE, RUBRIQUES, COMPAGNIES-FICTIVES, COMPAGNIE-D-ESSAI, COOLPAIE-SORTIR-SES-DOCUMENTS
    ├── outils/         banc/ (le banc d'essai daté, piloté en Node), demos/
    ├── bac-a-sable/    une installation en retard d'une version, pour essayer la mise à jour
    ├── depot/, installeur/, site/, maquettes/
    ├── README.md       comment chaque commande s'utilise
    └── AVANT-LE-PREMIER-COMMIT.md   ce qui n'entre jamais dans le dépôt, et le garde avant commit

puppetpaieprivate/  (le dépôt privé)
├── journal/            ETAT.md (le point de reprise, une page), passations, topos, archives/
├── reference-privee/   les textes lus qu'on ne republie pas (originaux d'organismes, pièces réelles)
└── (ses issues)        les tickets : ceux de Léandre, et ceux du filet (retours, incidents)
```

Sur le poste d'une compagnie, le dépôt privé n'existe pas : `logiciel/`, `fictives/`, `maj/` et `prive/` vivent côte à
côte, dans `Documents\PuppetPaie` ou dans un dossier portable (`PORTABLE.json`, une clé USB).

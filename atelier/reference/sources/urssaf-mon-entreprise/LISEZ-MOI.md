# Mon-entreprise (Urssaf) — les règles publiées des intermittents du spectacle

Ce n'est pas un texte de loi : c'est **le modèle de calcul que l'Urssaf publie** (simulateurs de mon-entreprise.urssaf.fr,
paquet `modele-social`, code ouvert : github.com/betagouv/mon-entreprise, dossier `modele-social`). Il dit comment
l'Urssaf applique les textes ; il cite ses références (Légifrance, Audiens). Une règle qu'on n'y lit que là reste une
**lecture de l'Urssaf**, à confronter au texte qu'elle cite.

## Les fichiers

- `urssaf-mon-entreprise-intermittents-artiste-20261010.mht` : la page
  `https://mon-entreprise.urssaf.fr/documentation/salarié/régimes-spécifiques/intermittents-du-spectacle/artiste`,
  enregistrée par Léandre depuis le poste le 10/10/2026 ; texte dans le `.txt` (extrait du HTML).
- `urssaf-modele-social-11.1.0-20260716.tgz` : le paquet npm `modele-social` 11.1.0, publié le 16/07/2026
  (`https://registry.npmjs.org/modele-social/-/modele-social-11.1.0.tgz`), relevé le 10/10/2026 depuis le conteneur.
  Les règles sont le JSON de `package/dist/index.js`.
- `urssaf-modele-social-11.1.0-intermittents-20260716.json` : les 18 règles de ce paquet dont le nom contient
  « intermittents » ou « congés spectacle », ou qui portent « 11.5% », extraites telles quelles (`node`).

## Ce qu'on en lit

- **Plafond d'un artiste** (`… artiste . plafond proratisé`) : applicable si « nombre jours travaillés < 5 » (la
  question : « Pour combien de jours continus l'artiste est-il engagé ? ») ; il vaut « plafond sécurité sociale .
  horaire × 12 × nombre jours travaillés » et remplace le plafond **dans les cotisations FNAL et vieillesse**.
  Concorde avec l'arrêté du 24/01/1975, art. 3 (`../arrete-1975-artistes-du-spectacle/`), et le BOSS, assiette
  générale, § 680 (`../boss/`).
- **Taux abattus d'un artiste** (`… artiste . réduction de taux`) : 70 % des taux d'allocations familiales, FNAL,
  maladie, vieillesse, versement mobilité ; non applicable à une « activité accessoire » (cours, stages) ; la RGDU
  n'est pas applicable ; AT/MP propre (1,12 %, 1,54 % en Alsace-Moselle).
- **Acteur de complément** au cinéma : un jour, assiette forfaitaire de 9 SMIC horaires sous 6 % du plafond.
- **Aucune majoration de 11,5 % de l'assiette** n'apparaît dans les règles des intermittents : les seuls « 11.5% » du
  paquet sont des cotisations minimales des indépendants. Le FNAL et la vieillesse plafonnée d'un artiste y portent sur
  le brut dans la limite du plafond, sans majoration. **À confronter** au barème du moteur (FNAL d'un artiste sur
  `brut_x115`, CTP 334/332 × 1,115), dont le texte n'est toujours pas déposé.
- Caisse des congés spectacles : 15,40 % du brut (référence Audiens, art. L. 3141-30).

Où le moteur s'en sert : `atelier/sources/paie/src/plafond.rs` (lots 1i, 1l), barème `logiciel/donnees/baremes/`.

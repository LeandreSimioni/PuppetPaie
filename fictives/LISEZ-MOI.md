# Les quatre compagnies fictives

Quatre compagnies **fictives**, payées tous les mois de **janvier 2025 à
décembre 2026** par le vrai moteur : contrats, DPAE, bulletins, certificats
Congés Spectacles et DSN, au barème de l'année (2025 puis 2026).

| dossier | compagnie | convention | équipe |
|---|---|---|---|
| `fictive-3090-grande` | Les Marées hautes — théâtre en tournée, Tours | 3090, annexe 4 | 3 permanents (une administratrice cadre en CDI, un chargé de production en CDI, une attachée de production en CDD sept. 2025 → juin 2026) + 6 intermittents, 3 spectacles |
| `fictive-3090-petite` | Le Fil tendu — marionnettes, Clermont-Ferrand | 3090, annexe 4 | 3 intermittents, 2 spectacles, des mois sans personne |
| `fictive-1285-grande` | Les Deux Rives — compagnie conventionnée, Lyon | 1285 (CCNEAC) | 2 permanents en CDI (un administrateur cadre, une chargée de production embauchée en avril 2025) + 5 intermittents, 2 spectacles |
| `fictive-1285-petite` | La Boussole — danse, Montpellier | 1285 (CCNEAC) | 3 intermittents, 2 pièces, des mois sans personne |

Chaque dossier a son `FICTIVE.md` : l'équipe, les spectacles, le brut
et le coût de chaque mois, et ce que le moteur a dit.

## Chaque compagnie a son identité

Son SIRET, sa raison sociale, son adresse, son signataire, son téléphone et
son centre de médecine du travail — sur les contrats, les bulletins, les
certificats, dans la DSN **et dans la DPAE**. Son logo et la signature de son
signataire sont des dessins SVG de quelques centaines d'octets
(`gabarits/identite/`), posés en vectoriel sur chaque document.

| compagnie | signataire | logo |
|---|---|---|
| Les Marées hautes | Adrienne Lecouvreur, directrice | trois vagues et un soleil, bleu |
| Le Fil tendu | George Sand, présidente | une marionnette à fils, rouge |
| Les Deux Rives | Pauline Viardot, présidente | un pont, vert |
| La Boussole | Lili Boulanger, présidente | une rose des vents, ocre |

Jusqu'au 24/09/2026, le moteur prenait le logo, la signature et l'employeur
de la DPAE dans le dossier `gabarits/` **du poste** : ces compagnies
sortaient avec l'identité de la compagnie réelle. C'est corrigé dans le
moteur (`contexte.gabarits_de`, `dpae.employeur`), pour toute compagnie.
Le 25/09/2026, même chose pour les modèles de contrat, de certificat et de
reçu : une compagnie fictive prend désormais le modèle livré avec le
logiciel, jamais celui du poste (`contexte.modele_de`). Les contrats portaient
encore le centre de médecine du travail de la compagnie réelle ; la
compagnie fictive a été reproduite ce jour-là.

## Rien n'y est réel, et rien n'en part en réel

- Les SIREN, SIRET et NIR sont fabriqués (clés justes, famille `123456…`,
  commune de naissance 999) ; les noms sont ceux du calendrier
  républicain ; les numéros d'objet, les lieux et les références Audiens
  sont inventés.
- Chaque dossier porte `COPIE-DE-DEVELOPPEMENT.json` : les dernières portes
  avant le réseau refusent une DSN ou une DPAE réelle, la sauvegarde et la
  publication (`paie/src/interface/compagnies.rs`, `est_copie` et
  `est_fictive` ; `paie/src/routes_accueil.rs`, `simulation_active`).
  **Ne pas le retirer.**
- Le montant Urssaf de chaque DSN est **calculé** par le moteur, comme pour
  une vraie compagnie : la somme des lignes 23, chacune valorisée à la table
  des CTP de l'Urssaf (journal §162, `paie/src/bordereau.rs`).
- **Les taux d'impôt (prélèvement à la source) sont fabriqués**, comme
  s'ils venaient des comptes-rendus de la DGFiP (`config/pas.json`, journal
  §115). Chaque salarié a le sien (27/09/2026 : une compagnie de
  compagnie fictive montre un logiciel qui marche, sans alerte). Le taux arrive
  après la première DSN qui déclare le salarié : un intermittent à son
  premier contrat part donc sur la grille de l'article 204 H (type 13),
  puis sur son taux (type 01) au contrat suivant.

## L'horloge : la compagnie vit toute seule

On arrive sur le bureau d'un administrateur : ce qu'il a fait est fait, ce
qui tombe maintenant reste à faire. À chaque ouverture, l'horloge
(`paie/src/horloge.rs`, `rattraper`) joue ce qui était dû avant
aujourd'hui, à sa date, et seulement ce qui n'a pas été fait :

| quand | ce qui se joue |
|---|---|
| le 1er du mois | le mois s'écrit (contrats, bulletins, DSN), ses DPAE partent, les contrats reviennent signés |
| à l'échéance de la DSN (le 15 du mois suivant, reporté au jour ouvré) | la DSN part, puis les documents aux salariés |
| fin janvier | l'année passée se clôt, s'exporte pour la comptabilité, ses numéros d'objet s'archivent |
| fin de fenêtre | les échéances qu'on coche (taux AT/MP, FCAP-SVP) se cochent |

Ce que le visiteur a fait lui-même n'est jamais refait. L'horloge garde sa
mémoire dans `config/horloge.json`. Les saisies s'arrêtent en décembre
2026, faute de barème 2027.

## Les ouvrir

Dans l'interface : l'accueil propose chaque compagnie fictive en tuile, et
elle s'ouvre sans mot de passe. En ligne de commande, la compagnie est
`PUPPETPAIE_DONNEES` (depuis la racine du dépôt) :

```bash
PUPPETPAIE_DONNEES=fictives/fictive-3090-grande paie echeancier
```

(sous Windows : `set PUPPETPAIE_DONNEES=fictives\fictive-3090-grande` puis la
même commande.)

## D'où elles viennent

Ces dossiers ont été fabriqués par un générateur Python
(`logiciel/outils/compagnies_demo.py`), parti avec le Python et **non
porté** : ils ne se refont plus d'un coup. Ils vivent : l'horloge les fait
jouer à chaque ouverture, et le poste commite ce qu'elle a joué.

## Le poids

21 Mo pour les quatre, 655 PDF. Un bulletin d'une page pèse ~32 Ko, un
certificat Congés Spectacles ~3 Ko : les polices ne gardent que les glyphes
employés, le logo et la signature sont des tracés, et chaque PDF est
recompacté (`bulletin.compacter`). Le reste du poids vient de ce que chaque
document existe deux fois — le PDF du mois, et un PDF par salarié dans
`envoi/`, celui qui part par courriel.

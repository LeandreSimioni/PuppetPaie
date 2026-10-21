# Les conseils « Le saviez-vous ? » (ticket 224)

La banque des conseils de PuppetPaie est `logiciel/donnees/conseils.json` : chaque conseil y cite un texte **déjà déposé**
dans le dépôt (son fichier, l'endroit, une citation à l'identique). Ce dossier-ci ne garde que les textes qu'il a fallu
déposer pour elle, et l'histoire de sa rédaction.

## Les textes déposés pour la banque

Relevé du 21/10/2026 (curl). Les originaux HTML des pages de l'Afdas sont dans `reference-privee/conseils/` du dépôt privé ;
ici, la transcription du corps de la page (menus et scripts retirés).

| Fichier | Ce que c'est | URL | Où les conseils s'en servent |
|---|---|---|---|
| `afdas-intermittents-conditions-20261021.md` | Afdas, « Intermittents du spectacle : quelles conditions » : seuils de 48 / 88 / 130 jours ou cachets sur 2 ans, indépendance par rapport à France Travail, délais de carence (12 / 18 / 24 mois), avis sur l'arrêt des financements après le 25/10/2026 | https://www.afdas.com/particulier/connaitre-les-dispositifs-et-les-modalites-dacces-a-la-formation/intermittents-du-spectacle-et-de-laudiovisuel/quelles-conditions.html | conseils `afdas-seuils-jours-cachets`, `afdas-independant-de-france-travail`, `afdas-carence` |
| `afdas-intermittents-financements-20261021.md` | Afdas, « Intermittents du spectacle : quels financements » : barèmes horaires et plafonds par type de formation, CQP, CPF, PTP | https://www.afdas.com/particulier/connaitre-les-dispositifs-et-les-modalites-dacces-a-la-formation/intermittents-du-spectacle-et-de-laudiovisuel/quels-financements.html | aucun conseil aujourd'hui (le conseil sur ses barèmes est écarté : voir plus bas) |

Les montants et délais de ces pages changent chaque année : ils sont datés du relevé.

## Où le moteur s'en sert

- `logiciel/donnees/conseils.json` : la banque. Un conseil a un `id`, un `domaine`, un `public`, un `texte` (sans le titre
  « Le saviez-vous ? », que l'écran pose), une ou plusieurs `source` {`fichier`, `article` (et `code` pour l'entrepôt
  LEGI), `citation`}, un `quand` (« toujours » : un conseil général, celui du démarrage ; `saisie:<condition>` ;
  `historique:<condition>`), le `ciblage` voulu et la `certitude` du rédacteur. Un chiffre daté n'y est jamais écrit en
  dur : `{bareme:…}` et `{minima:…}` le lisent dans `donnees/baremes/<année>.json` et `donnees/minima-conventionnels.json`.
- `atelier/sources/paie/src/conseils.rs` : l'option de la compagnie (fiche Entreprise, `conseils`), la rotation du conseil
  du jour, les conditions de saisie et d'historique, et les constantes de l'article 30 des annexes (`annexes` dans la
  banque).
- `atelier/sources/paie/tests/fichiers/conseils_banque.rs` vérifie que chaque fichier cité existe et que chaque citation
  s'y lit (à la casse, aux blancs, aux apostrophes et aux guillemets près ; « […] » sépare des fragments).

## La rédaction (21/10/2026)

Deux rédacteurs, à partir des textes déposés : A (droits des intermittents, Congés Spectacles, Audiens, Afdas, médecine du
travail, conventions ; 81 conseils) et B (paie, contrats, déclarations, logiciel ; 86). Le cahier des charges : vrai et
sourcé, abordable (deux ou trois phrases de français courant), neutre et licite, une idée par conseil, l'employeur pour
interlocuteur. Leurs blocs (identifiant, domaine, public, texte, source, ciblage, certitude) sont passés tels quels dans la
banque ; les citations reformulées ou prises à cheval sur deux colonnes d'un PDF ont été remplacées par un fragment lu à
l'identique dans le texte déposé.

Arbitrages de Léandre : le numéro d'objet du spectacle est délivré par **France Travail** ; « AEM » se dit « DSN » côté
employeur quand la source le permet ; jamais d'euros d'allocation ; aucune mise en garde ni allusion à la fraude.

**Écartés** (absents de la banque) :

- `allocation-minimale-38-44` : un montant d'allocation en euros ; et « montant de l'allocation maximale » ne se lit pas
  dans le guide déposé.
- `afdas-baremes-horaires` : un barème daté de l'Afdas, absent des données du moteur ; sans ses chiffres, il ne dit plus rien.
- `afdas-demandes-avant-formation` : périssable (fonds épuisés après le 25/10/2026).
- `conges-spectacles-plafonds-metteur-en-scene` : plafonds journaliers datés (juillet 2025), absents des données du moteur ;
  sans eux, il redit `conges-spectacles-plafond-triple-minimum`.
- `audiens-prevoyance-fonds-sante` : taux Audiens 2026 absents des données du moteur à cet endroit ; la phrase sur le Fonds
  ne se lit qu'à cheval sur deux colonnes de la brochure de 2022.

**Suspendus** (dans la banque, `suspendu` posé, jamais affichés) : `conges-spectacles-droit-ouvre-1er-jour` (D. 7121-31,
« au moins quatre semaines d'engagement ou de trente cachets », contre Audiens, « dès le 1er jour de travail ») ;
`signalement-fin-contrat-cinq-jours` et `signalement-arret-cinq-jours` (« 5 jours », « 5 jours ouvrés » ou « ouvrables »).

**Sans chiffre** : un chiffre daté qui n'est pas dans les données du moteur a été retiré du texte (minima de la CCNEAC,
paliers de la grille PAS, montant de l'indemnité maternité Audiens, plancher de l'indemnité d'incapacité, exemple
d'échéance de 2026) ; ceux qui y sont se lisent par gabarit (plafond de la Sécurité sociale, taux Congés Spectacles,
abattement d'assiette CSG, taux et plafond de la DFS, cachet de répétition de la 3090).

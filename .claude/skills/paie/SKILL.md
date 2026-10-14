---
name: paie
description: Fabrique les paies du spectacle vivant avec PuppetPaie - bulletins, contrats, DPAE, certificats Congés Spectacles et DSN mensuelle, pour les intermittents comme pour les permanents. Déclencher dès que l'utilisateur demande une ou plusieurs paies, un bulletin, un cachet, une DSN, une DPAE/DUE, ou parle d'engager quelqu'un sur un spectacle à des dates données. Exemples : « fais 2 paies pour Celi et Marin les 2 et 3 octobre pour Aaah Zerbinette », « la DSN d'octobre », « il faut déclarer Célimène sur 3 cachets ».
---

# Faire une paie avec PuppetPaie

Les exemples ci-dessous sont ceux du banc d'essai ;
la marche est la même pour toute compagnie.

Le calcul est fait par `moteur/`, pas par toi. Ton rôle : comprendre la
demande, écrire le fichier de saisie, lancer la commande, relire la sortie.
Ne calcule jamais une cotisation ni un net à la main, même pour vérifier —
le moteur est calé au centime sur les bulletins réels, pas toi.

## Marche à suivre

1. **Comprendre.** Il faut : qui, quelles dates jouées, quel spectacle. Le reste
   a des valeurs par défaut (130 € le cachet, un cachet par date, contrat du
   premier au dernier jour joué).

2. **Écrire `paies/AAAA-MM.json`.**

```json
{
  "mois": "2026-10",
  "spectacle": "aaah zerbinette",
  "taux_cachet": 130.0,
  "date_paiement": "2026-10-31",
  "engagements": [
    { "salarie": "Celi",     "dates": ["2026-10-02", "2026-10-03"] },
    { "salarie": "Marin", "dates": ["2026-10-02", "2026-10-03"] }
  ]
}
```

   Les noms sont résolus de façon tolérante (« Celi », « Mairn », « zerbinette »).
   Champs facultatifs par engagement : `cachets` si le nombre diffère du nombre
   de dates, `taux_cachet`, `debut`/`fin` si le contrat déborde des dates
   jouées, `heures` si la personne est payée à l'heure, `lieu`, `numero`.

   Un mois peut contenir plusieurs contrats pour la même personne : un
   engagement par contrat, chacun avec ses dates.

3. **Lancer** `python -m moteur mois paies/AAAA-MM.json` depuis `logiciel/`.

4. **Relire et rendre compte.** Reprends le tableau affiché (brut, net, coût
   employeur par personne), dis où sont les fichiers et rappelle ce qui reste
   à faire (ci-dessous). Ne recopie pas le détail des cotisations : c'est dans le PDF.

## Ce qu'il faut demander avant de lancer

- Le spectacle est inconnu du catalogue → demander son **numéro d'objet
  France Travail** et l'ajouter à `config/spectacles.json`. Ne jamais inventer
  un numéro d'objet. France Travail en attribue un par spectacle ou
  production (net-entreprises, fiche 3344), qui sert tant que le spectacle se
  joue.
- La personne est inconnue → demander nom, prénom, NIR + clé, date et lieu de
  naissance, adresse, e-mail, et l'ajouter à `config/salaries.json`.
- Le taux du cachet n'est pas 130 € → le demander plutôt que le supposer.

## Après la génération

Rappelle l'ordre des dépôts, il compte :

1. **DPAE** — **avant** le premier jour de travail, par l'API Urssaf
   (`deposer-dpae`, ou `Depot DPAE.cmd`). Si le contrat a déjà commencé,
   dis-le clairement : la DPAE est en retard.
2. **Contrat** signé.
3. **DSN** — la **déposer en test** (`depot-dsn --test`) et lire le bilan
   **avant** le dépôt réel. Plus d'AEM à déposer depuis le 01/06/2026 (100 %
   DSN) : France Travail édite l'attestation depuis la DSN et la renvoie dans
   les retours (nature 46) ; il reste à la remettre au salarié.
4. **Certificats Congés Spectacles** — `CertificatsCS_<Mois><Année>.pdf` :
   **rien à déposer nulle part**, ils sont dus AU SALARIÉ, avec son bulletin.
   Les N° CE sont une séquence continue de l'employeur : ils sont consommés
   dès que le mois est produit, ne les rejoue pas pour rien.

Le montant du télépaiement **URSSAF** (bloc `S21.G00.20`) se **calcule** depuis
le 29/09/2026 (`paie/src/bordereau.rs`) : rien à demander à l'utilisateur.
`python -m moteur mois` **ferme le mois**, et refuse (`MoisPasPret`) tant que la
DPAE et le contrat de chaque embauche du mois ne sont pas partis.

Échéance DSN : le 15 du mois suivant, le 5 à partir de cinquante salariés
(le 15, ou le 5 à partir de 50 salariés : déduit de l'effectif).

## Limites à signaler plutôt qu'à contourner

- Contrat mêlant cachets et heures : il faut `heures` **et** `taux_horaire`
  dans la saisie (le taux du cachet ne s'applique pas aux heures). Sans le taux
  horaire, le moteur refuse le contrat plutôt que de deviner.
- Un arrêt de travail (`arret`), un mois déjà déposé à corriger (`corriger`),
  la fin d'un contrat de permanent (`fin-contrat`) ont leurs commandes : voir
  le README. La régularisation annuelle (bloc `S21.G00.82`) reste hors moteur.
- Année sans barème : le **socle** vit dans `moteur/baremes/<année>.json`,
  livré avec le logiciel (c'est une version à publier, pas une saisie) ; la
  compagnie n'a que sa **surcouche** dans `config/baremes/<année>.json` (son
  taux AT/MP). Le moteur s'arrête avec un message explicite : ne pas recopier
  un barème complet à la main.

## En cas de doute sur un chiffre

Le calcul est celui du moteur Rust (`paie/`) : `cargo test --release` dans
`paie/` rejoue ses attendus, la suite Python vérifie ce qui en sort (DSN,
bulletins, documents). C'est la réponse à « est-ce que ça calcule juste ? » —
pas une vérification mentale.

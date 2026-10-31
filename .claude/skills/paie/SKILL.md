---
name: paie
description: Fabrique les paies du spectacle vivant avec PuppetPaie - bulletins, certificats Congés Spectacles et DSN mensuelle, avec la DPAE et le contrat qui les précèdent, pour les intermittents comme pour les permanents. Déclencher dès que l'utilisateur demande une ou plusieurs paies, un bulletin, un cachet, une DSN, une DPAE/DUE, ou parle d'engager quelqu'un sur un spectacle à des dates données. Exemples : « fais 2 paies pour Fanchon et Valentin les 14 et 15 décembre pour Les Pupazzi », « la DSN d'octobre », « il faut déclarer Nicole sur 3 cachets ».
---

# Faire une paie avec PuppetPaie

Le calcul est fait par le moteur `moteur`, jamais par toi. Ton rôle : comprendre la demande, écrire le fichier de saisie,
lancer la commande, relire la sortie. Ne calcule jamais une cotisation ni un net de tête, même pour vérifier.

`moteur` : `atelier/sources/moteur/bin/windows/moteur.exe` sous Windows (`bin/linux/moteur`, `bin/mac/moteur`), lancé depuis la
racine du dossier PuppetPaie. La compagnie est `prive/` à côté de `logiciel/` (ou le dossier de `PUPPETPAIE_DONNEES`) ;
les chemins `paies/…` et `sorties/…` se lisent depuis elle, et un mois nu (`2026-12`) désigne sa saisie.

## Marche à suivre

1. **Comprendre** : qui, quelles dates jouées, quel spectacle, quel cachet.
2. **Écrire les engagements** : une carte par salarié dans la fenêtre, ou la route `enregistrer-saisie` (`{"mois", "saisie": <un engagement>}`, une carte à la fois) ; ils vont dans `engagements/2026-12.json`, de cette forme :

```json
{
  "mois": "2026-12",
  "taux_cachet": 150.0,
  "date_paiement": "2026-12-31",
  "engagements": [
    { "salarie": "Brioche", "spectacle": "pupazzi",
      "lignes": [ { "rubrique": "cachet", "quantite": 2, "taux": 150, "lieu": "Thiers",
                    "dates": ["2026-12-14", "2026-12-15"] } ] }
  ]
}
```

   Tout passe par `lignes` : `dates`, `cachets`, `heures` posés sur l'engagement sont refusés. Un contrat mixte = une
   ligne `cachet` et une ligne `heure`, chacune à son taux. Par engagement, au besoin : `debut`/`fin` (contrat qui
   déborde des dates jouées), `numero` (dès le 2e mois d'un contrat sur plusieurs mois), `lieu`, `date_paiement`,
   `metier` (code emploi, `moteur emplois --chercher …`). Un permanent : `"nature": "cdi"` ou `"cdd"`, sans spectacle,
   avec `salaire_mensuel` et `quotite_contrat`. Les noms sont tolérants (« Brioch », « Valentin », « pupazzi »).
3. **Montrer les chiffres avant tout acte** (brut, net, coût, sans rien écrire) :
   `echo '{"route":"apercu","charge":{"saisie": <contenu du fichier>}}' | moteur route --racine prive`
   (un `moteur` livré veut aussi `PUPPETPAIE_CODE` = le dossier `logiciel/donnees`).
4. **Avant le premier jour** : la DPAE (`moteur deposer-dpae 2026-12`, voir la compétence `declarations`) et le contrat envoyé
   à la signature (compétence `contrat`). Ce sont deux actes réels : c'est l'utilisateur qui les fait partir.
5. **Fermer le mois** : `moteur mois 2026-12`. Il écrit dans `sorties/2026-12/` les bulletins, les certificats Congés
   Spectacles et la DSN, et les pièces de chaque salarié dans `sorties/2026-12/envoi/`.
6. **Rendre compte** : le tableau affiché (brut, net, coût par personne), où sont les fichiers, ce qui reste à faire.

## Les refus à attendre (ne pas les contourner)

- `Le mois … ne peut pas encore se fermer` : DPAE pas déposée, ou contrat pas envoyé à la signature. Le dire.
- `Salarie inconnu` : la fiche manque. La faire créer dans la fenêtre (Configuration › Salariés) : NIR et clé,
  naissance, adresse, courriel, emploi. Ne jamais inventer un NIR.
- `Spectacle inconnu` : il faut son **numéro d'objet France Travail** (Configuration › Spectacles,
  ou `config/spectacles.json`). Ne jamais en inventer un.
- `Pas de bareme pour <année>` : le socle (`logiciel/donnees/baremes/`) vient avec une version du logiciel ; la
  compagnie n'a que sa surcouche (`config/baremes/<année>.json`, son taux AT/MP). Ne pas recopier un barème.
- Le cachet n'est pas celui de la saisie habituelle : le demander, ne pas le supposer.

## Après la production

1. **DSN** : `moteur depot-dsn sorties/2026-12/DSN_Decembre2026.dsn --test` (sans aval), lire le bilan. Le dépôt
   réel est à l'utilisateur. Échéance : le 15 du mois suivant (le 5 à partir de 50 salariés), à midi, reportée au jour ouvré suivant un week-end ou un férié.
2. **Certificats Congés Spectacles** : dus **au salarié**, rien à déposer. Leurs N° CE forment une suite continue :
   ne pas reproduire un mois pour rien.
3. **Envoi aux salariés** : `moteur envoi 2026-12` montre ce qui partirait ; l'envoi demande le mot de passe du
   logiciel, tapé par l'utilisateur.

Un arrêt de travail (`moteur arret`), un mois déjà déposé à corriger, la fin d'un permanent
(`moteur fin-contrat`) : compétence `declarations`. Ne jamais reproduire un mois déjà déposé pour le corriger.

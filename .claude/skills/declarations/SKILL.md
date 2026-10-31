---
name: declarations
description: Où, quand et comment déposer les déclarations sociales avec PuppetPaie - DPAE, DSN mensuelle, signalements (arrêt de travail, fin de contrat), Congés Spectacles, Audiens. Déclencher quand l'utilisateur demande une échéance, un calendrier de dépôt, où déposer un fichier, ce qui reste à déclarer, ou signale un rejet de déclaration.
---

# Calendrier et dépôt des déclarations

Pour toute compagnie du spectacle vivant (IDCC 3090 ou 1285, intermittents et permanents). `moteur` est le moteur
(compétence `paie`) ; les chemins se lisent depuis le dossier de la compagnie.

**La règle de tout dépôt** : le **test** d'abord (`--test`), sans demander d'aval ; c'est le seul validateur. Le
**réel** est refusé tant que ce contenu-là n'a pas réussi en test, et il demande le mot de passe du logiciel, **tapé
par l'utilisateur** au moment de l'acte (dans la fenêtre : « Envoyer les DPAE… », « Déposer la DSN… »). Un agent ne
lance jamais `--reel`, ne tape pas ce mot de passe, ne pose aucune variable de mot de passe.

Ce qui reste à faire : les routes `rappels` et `echeancier`. Ce qui est parti : la route `historique-mois`.

## DPAE (déclaration préalable à l'embauche)

- **Quand** : dans les 8 jours qui précèdent l'embauche, et **avant** le premier jour de travail. En retard, c'est
  une infraction : le dire franchement.
- **Produire** : la route `depot-apercu` (`{"quoi":"dpae","mois":"2026-12"}`, embauche déclarée à 8 h). Elle n'écrit que la DPAE (`sorties/<mois>/DPAE_*.txt`,
  son `.zip` et son `_recapitulatif.txt`) et, au besoin, les demandes de taux de PAS. Une embauche déjà
  passée : DPAE hors délai.
- **Vérifier** : la même route `depot-apercu` (relit, contrôle les délais,
  n'envoie rien). Des embauches à plus de 8 jours sont refusées : `moteur route` `depot-apercu` avec
  `{"quoi":"dpae","mois":"2026-12","engagements":[…]}` pour les seules dates proches.
- **Test** : la route `deposer` (`{"quoi":"dpae","mois":"2026-12"}`). **Réel** : l'utilisateur. Retours à part : la route `ouverture`
  (`retours/dpae/`, nature 41 = certificat de conformité).

## DSN mensuelle

- **Quand** : le 15 du mois suivant ; le 5 à partir de cinquante salariés (déduit de l'effectif des retours), **à midi** ;
  un samedi, un dimanche ou un jour férié reporte au jour ouvré suivant (R. 133-14, I ; `declarations/src/echeance_dsn.rs`, que
  l'échéancier et les rappels disent). Due aussi les mois sans paie (envoi néant).
- **Fichier** : `sorties/<mois>/DSN_<Mois><Année>.dsn`, écrite au dépôt depuis les engagements validés.
- la route `depot-apercu` (`{"quoi":"dsn","mois":…}`) : ce qui partirait, sans réseau. Puis `deposer`, en test, lire le bilan.
- Le montant URSSAF (`S21.G00.20`) est **calculé** par le moteur : rien à saisir.
- Retours : la route `ouverture` (accusé, bilan, comptes rendus). Un accusé `OK` n'est pas une acceptation : c'est le bilan
  qui tranche. Le taux de PAS (nature 94) va de lui-même dans `config/pas.json` au relevé.

## Arrêt de travail (signalement, nature 04)

Dû dans les **cinq jours ouvrés** suivant la connaissance de l'arrêt, ou avec la DSN du mois si la compagnie est subrogée
(CSS R. 133-14, II ; arrêté du 2 juillet 2013) ; la DSN mensuelle le reporte en plus, d'elle-même. la route `arrets` dit, sous chaque
arrêt, chaque obligation, sa date butoir et où on en est (le moteur la calcule : `obligations.rs`) ; la route `arret-connaissance`
(`salarie`, `dernier_jour`, `le`) dit quand la compagnie l'a su, si ce n'est pas le premier jour d'arrêt
(ou le jour de l'accident).

**Accident du travail ou de trajet (motifs 04 et 06) : une déclaration d'accident en plus, HORS DSN**, à la CPAM de la victime,
dans les **48 heures, dimanches et jours fériés non compris**, après que la compagnie l'a su (CSS L. 441-2, R. 441-3) : en ligne
sur net-entreprises (service DAT, s'y habiliter d'avance) ou par le cerfa 14463*03 en recommandé. Le logiciel ne la fait pas ;
il la rappelle, et la retient faite quand on le lui dit : la route `accident-declare` (`ou` :
net-entreprises|courrier, `reference` ; `retirer` l'efface). Une maladie professionnelle (05) se déclare par le salarié.

**Deuil d'enfant (motif 19) : pas de signalement, mais l'attestation de salaire (DSIJ) HORS DSN**, sur net-entreprises (service
« Attestation de salaire », écran ou EDI), dès réception de l'avis d'arrêt — la DSN ne couvre pas le deuil ; la dire faite :
la route `attestation-declare`.

```
route arret-contrats {"salarie":"BRIOCHE","le":"2026-12-14"}
route arret-enregistrer {"salarie":"BRIOCHE","dernier_jour":"2026-12-14","fin":"2026-12-20","motif":"01","subrogation":"02"}
route arret-signaler {"salarie":"BRIOCHE","dernier_jour":"2026-12-14"}
route deposer {"quoi":"dsn","fichier":"sorties/2026-12/SignalementArret_BRIOCHE_20261214.dsn"}
```

La route `imprevus` liste les motifs. La subrogation est obligatoire (oui/non : c'est à l'employeur de répondre). Une
reprise ne se signale que si elle est **anticipée** (`arret-reprise`, puis `arret-signaler`). Corriger un
signalement parti : `--remplace <identifiant du certificat de conformité>`, jamais un second signalement normal.

## Fin de contrat (FCTU, nature 07)

- **CDDU du spectacle arrivé à son terme : pas de FCTU.** Sa fin part dans la DSN mensuelle (circuit dérogatoire).
  Rompu avant son terme, il se signale.
- **Permanent (CDI, CDD)** : la route `fins-de-contrat` (ce qui se termine, ce qui manque), puis
  `fin-contrat-signaler` et `fin-contrat-documents` (certificat de travail, reçu
  pour solde de tout compte). Le fichier se dépose par `deposer`, avec `fichier`.

## AEM, Congés Spectacles, Audiens

- **Plus d'AEM** : France Travail édite l'attestation depuis la DSN et la renvoie dans les retours ; la relève les
  range, `envoyer-documents` les remet au salarié.
- **Congés Spectacles et Audiens** sont déclarés par la DSN : rien à leur envoyer en plus, seulement les paiements.
- Le **certificat Congés Spectacles** (`CertificatsCS_<Mois><Année>.pdf`) est dû **au salarié**, à l'issue du
  contrat. Son N° CE est une suite continue de l'employeur, jamais remise à 1.

## Un mois déjà déposé, un rejet
- Avant la veille de l'échéance : modifier l'engagement, le revalider et redéposer (annule-et-remplace). Après : modifier
  l'engagement et le revalider suffit, la DSN du mois suivant porte la correction d'elle-même. **Jamais** reproduire un mois déposé.
  `--motif`) ; écrire la correction demande le mot de passe du logiciel. **Jamais** reproduire un mois déposé.
- Un rejet : lire le message de net-entreprises, trouver la donnée (`config/` ou la saisie), régénérer, redéposer en
  test. Ne jamais corriger un `.dsn` à la main. Les échecs vus par le logiciel : la route `incidents-lire`.

---
name: declarations
description: Où, quand et comment déposer les déclarations sociales avec PuppetPaie - DPAE, DSN mensuelle, signalements (arrêt de travail, fin de contrat), Congés Spectacles, Audiens. Déclencher quand l'utilisateur demande une échéance, un calendrier de dépôt, où déposer un fichier, ce qui reste à déclarer, ou signale un rejet de déclaration.
---

# Calendrier et dépôt des déclarations

Pour toute compagnie du spectacle vivant (IDCC 3090 ou 1285, intermittents et permanents). `paie` est le moteur
(compétence `paie`) ; les chemins se lisent depuis le dossier de la compagnie.

**La règle de tout dépôt** : le **test** d'abord (`--test`), sans demander d'aval ; c'est le seul validateur. Le
**réel** est refusé tant que ce contenu-là n'a pas réussi en test, et il demande le mot de passe du logiciel, **tapé
par l'utilisateur** au moment de l'acte (dans la fenêtre : « Envoyer les DPAE… », « Déposer la DSN… »). Un agent ne
lance jamais `--reel`, ne tape pas ce mot de passe, ne pose aucune variable de mot de passe.

Ce qui reste à faire : `paie rappels`, `paie echeancier`. Ce qui est parti : `paie journal` (`--quoi dpae`).

## DPAE (déclaration préalable à l'embauche)

- **Quand** : dans les 8 jours qui précèdent l'embauche, et **avant** le premier jour de travail. En retard, c'est
  une infraction : le dire franchement.
- **Produire** : `paie dpae 2026-12` (`--heure 0800` par défaut). Elle n'écrit que la DPAE (`sorties/<mois>/DPAE_*.txt`,
  son `.zip` et son `_recapitulatif.txt`) et, au besoin, les demandes de taux de PAS. Code de sortie 3 : embauche déjà
  passée, DPAE hors délai.
- **Vérifier** : `paie deposer-dpae sorties/2026-12/DPAE_Decembre2026.txt` (essai à blanc : relit, contrôle les délais,
  n'envoie rien). Des embauches à plus de 8 jours sont refusées : `paie dpae 2026-12 --seulement BRIOCHE:2026-12-14`
  pour les seules dates proches.
- **Test** : `paie deposer-dpae <fichier> --test`. **Réel** : l'utilisateur. Retours à part : `paie retours --dpae`
  (`retours/dpae/`, nature 41 = certificat de conformité).

## DSN mensuelle

- **Quand** : le 15 du mois suivant ; le 5 à partir de cinquante salariés (déduit de l'effectif des retours), **à midi** ;
  un samedi, un dimanche ou un jour férié reporte au jour ouvré suivant (R. 133-14, I ; `paie/src/echeance_dsn.rs`, que
  l'échéancier et les rappels disent). Due aussi les mois sans paie (envoi néant).
- **Fichier** : `sorties/<mois>/DSN_<Mois><Année>.dsn`, écrit par `paie mois`.
- `paie depot-dsn <fichier> --apercu` : ce qui partirait, sans réseau ni écriture. Puis `--test`, lire le bilan.
- Le montant URSSAF (`S21.G00.20`) est **calculé** par le moteur : rien à saisir.
- Retours : `paie retours` (accusé, bilan, comptes rendus). Un accusé `OK` n'est pas une acceptation : c'est le bilan
  qui tranche. Le taux de PAS (nature 94) ne s'écrit qu'avec `paie retours --appliquer` ; `paie pas` montre l'état.

## Arrêt de travail (signalement, nature 04)

Dû dans les **cinq jours ouvrés** suivant la connaissance de l'arrêt, ou avec la DSN du mois si la compagnie est subrogée
(CSS R. 133-14, II ; arrêté du 2 juillet 2013) ; la DSN mensuelle le reporte en plus, d'elle-même. `paie arret` dit, sous chaque
arrêt, chaque obligation, sa date butoir et où on en est (le moteur la calcule : `obligations.rs`) ; `paie arret connu CLE
--dernier-jour … --le … [--heure HH:MM] [--de accident]` dit quand la compagnie l'a su, si ce n'est pas le premier jour d'arrêt
(ou le jour de l'accident).

**Accident du travail ou de trajet (motifs 04 et 06) : une déclaration d'accident en plus, HORS DSN**, à la CPAM de la victime,
dans les **48 heures, dimanches et jours fériés non compris**, après que la compagnie l'a su (CSS L. 441-2, R. 441-3) : en ligne
sur net-entreprises (service DAT, s'y habiliter d'avance) ou par le cerfa 14463*03 en recommandé. Le logiciel ne la fait pas ;
il la rappelle, et la retient faite quand on le lui dit : `paie arret declare CLE --dernier-jour … --le … --ou
net-entreprises|courrier [--reference …]` (`--retirer` l'efface). Une maladie professionnelle (05) se déclare par le salarié.

```
paie arret contrats BRIOCHE --le 2026-12-14
paie arret saisir BRIOCHE --dernier-jour 2026-12-14 --fin 2026-12-20 --motif 01 --subrogation non
paie arret signaler BRIOCHE --dernier-jour 2026-12-14
paie depot-dsn sorties/2026-12/SignalementArret_BRIOCHE_20261214.dsn --test
```

`paie arret motifs` liste les motifs. La subrogation est obligatoire (oui/non : c'est à l'employeur de répondre). Une
reprise ne se signale que si elle est **anticipée** (`arret reprise`, puis `arret signaler … --reprise`). Corriger un
signalement parti : `--remplace <identifiant du certificat de conformité>`, jamais un second signalement normal.

## Fin de contrat (FCTU, nature 07)

- **CDDU du spectacle arrivé à son terme : pas de FCTU.** Sa fin part dans la DSN mensuelle (circuit dérogatoire).
  Rompu avant son terme, il se signale.
- **Permanent (CDI, CDD)** : `paie fin-contrat liste 2026-12` (ce qui se termine, ce qui manque), puis
  `paie fin-contrat signaler 2026-12 <CLE>` et `paie fin-contrat documents 2026-12 <CLE>` (certificat de travail, reçu
  pour solde de tout compte). Le fichier se dépose par `paie depot-dsn … --test`.

## AEM, Congés Spectacles, Audiens

- **Plus d'AEM** : France Travail édite l'attestation depuis la DSN et la renvoie dans les retours ; `paie aer --mois
  2026-12` les range, `paie envoi` les remet au salarié.
- **Congés Spectacles et Audiens** sont déclarés par la DSN : rien à leur envoyer en plus, seulement les paiements.
- Le **certificat Congés Spectacles** (`CertificatsCS_<Mois><Année>.pdf`) est dû **au salarié**, à l'issue du
  contrat. Son N° CE est une suite continue de l'employeur, jamais remise à 1.

## Un mois déjà déposé, un rejet

- Avant l'échéance : refaire le mois et redéposer (annule-et-remplace). Après : `paie corriger <mois>` (aperçu sans
  `--motif`) ; écrire la correction demande le mot de passe du logiciel. **Jamais** reproduire un mois déposé.
- Un rejet : lire le message de net-entreprises, trouver la donnée (`config/` ou la saisie), régénérer, redéposer en
  test. Ne jamais corriger un `.dsn` à la main. Les échecs vus par le logiciel : `paie incidents`.

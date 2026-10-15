---
name: declarations
description: Où, quand et comment déposer les déclarations sociales avec PuppetPaie - DPAE, DSN mensuelle, signalements (arrêt de travail, fin de contrat), Congés Spectacles, Audiens. Déclencher quand l'utilisateur demande une échéance, un calendrier de dépôt, où déposer un fichier, ce qui reste à déclarer, ou signale un rejet de déclaration.
---

# Calendrier et dépôt des déclarations

Écrit pour toute compagnie du spectacle vivant (IDCC 3090 ou 1285,
intermittents et permanents) ; les exemples chiffrés sont ceux du
banc d'essai.

## DPAE - déclaration préalable à l'embauche

- **Où** : l'API Urssaf (DUE MtoM) — `deposer-dpae` ou `Depot DPAE.cmd`,
  voir plus bas ; à la main, sur net-entreprises.fr ou urssaf.fr. Ses retours
  arrivent sur un canal à part (`retours/dpae/`, nature 41).
- **Quand** : dans les 8 jours qui précèdent l'embauche, et **obligatoirement
  avant** que la personne commence à travailler.
- **Produire** : `python -m moteur dpae paies/<mois>.json --heure HHMM`. Cette
  commande ne fait que la DPAE : ni Audiens, ni numéro de bulletin consommé.
  Ne pas passer par `mois` pour une DPAE — elle est due un mois plus tôt.
- **Fichier** : `sorties/<mois>/DPAE_<Mois><Année>.txt` — enregistrements de
  990 caractères collés bout à bout. Pour saisir en ligne, utiliser
  `DPAE_<Mois><Année>_recapitulatif.txt`. Le `.zip` sert au dépôt manuel ;
  l'API, elle, refuse les archives.
- **Déposer par l'API** : `Depot DPAE.cmd` ouvre la fenêtre. Essai à blanc par
  défaut ; le dépôt réel demande la case cochée et le mot de passe **Urssaf**
  (pas celui de net-entreprises : ce sont deux comptes distincts). Jamais
  déposé automatiquement après la production.
- **Le dépôt de test** parcourt toute la chaîne sans rien déclarer : bouton
  « Dépôt de test », ou `--test`. `C_VERSION` bascule en `TST` en mémoire. À
  faire avant tout premier dépôt réel du mois si l'on a un doute.
- **Une seule fenêtre à la fois** : ne pas mettre dans un même dépôt des
  embauches à plus de 8 jours. La commande refuse, et c'est voulu.
- Une DPAE en retard est une infraction : le dire franchement plutôt que de la
  faire passer discrètement.

## AEM - attestation employeur mensuelle : plus rien à déposer

- **Depuis le 01/06/2026, plus d'AEM** : la compagnie est en 100 % DSN
  (notifiée par France Travail le 28/05/2026). France Travail édite
  l'attestation à partir de la DSN et la renvoie dans les retours DSN
  (nature 45 : compte-rendu ; nature 46 : l'attestation, un PDF par salarié).
  Il ne reste qu'à la **remettre au salarié** (`envoi`).
- Les AEM d'avant (`AEM_<Mois><Année>_EDIV5.txt`, déposées sur
  declarationspectacle.francetravail.fr) restent l'historique que lit
  `importer`.
- Le **numéro d'objet** du spectacle est délivré par France Travail, un par
  spectacle ou production (net-entreprises, fiche 3344 ; 12 caractères). Il
  sert tant que le spectacle se joue : on ne le redemande pas chaque année.
  En janvier, PuppetPaie demande seulement quels spectacles continuent.

## DSN mensuelle

- **Où** : net-entreprises.fr. **Toujours déposer en test d'abord**
  (`depot-dsn --test`) : c'est le validateur qui fait foi — la brique de
  contrôle officielle, à jour — et il détecte ce que le moteur ne voit pas.
  DSN-Val (l'outil local) n'est pas installé et ne le sera pas : 200 Mo + Java
  pour le même moteur (`atelier/reference/norme-dsn/LISEZMOI.md`).
- **Quand** : le 15 du mois suivant ; le 5 à partir de cinquante salariés
  cinq ans de suite. Déduit de l'effectif calculé par l'Urssaf, et le 15 quand
  il est inconnu.
- **Fichier** : `sorties/<mois>/DSN_<Mois><Année>.dsn`, norme NEODeS P26V01.
- **Montant URSSAF** du bloc `S21.G00.20` : ordre de télépaiement,
  **calculé** depuis le 29/09/2026 — la somme des lignes 23 du bordereau à la
  table des CTP de l'Urssaf (`paie/src/bordereau.rs`). Rien à saisir.
- Une DSN « néant » reste due les mois sans paie.

## Signalement d'arrêt de travail (DSN événementielle)

Un intermittent malade **pendant ses dates** : le contrat est suspendu, et
deux déclarations le disent.

- **Où** : net-entreprises.fr, le même point de dépôt que la DSN mensuelle —
  `python -m moteur depot-dsn <fichier> --test` puis `--reel`.
- **Quand** : dans les **cinq jours** qui suivent la prise de connaissance de
  l'arrêt par l'employeur (cahier technique 2026.1 §5.3).
- **Produire** : `python -m moteur arret saisir CLE --dernier-jour AAAA-MM-JJ
  --fin AAAA-MM-JJ --motif 01 --subrogation non`, puis
  `python -m moteur arret signaler CLE --dernier-jour AAAA-MM-JJ`.
- **Fichier** : `sorties/<mois>/SignalementArret_<CLE>_<JJMMAAAA>.dsn`.
- **Et la DSN mensuelle du mois le REPORTE**, obligatoirement — elle le fait
  d'elle-même à la production, et le dit dans les avertissements. Ce n'est pas
  la même déclaration : le signalement sert la prestation, la mensuelle
  l'historique. L'une ne remplace pas l'autre.
- **La reprise** ne se signale (nature 05) que si elle est **anticipée** par
  rapport à la fin prévue de l'arrêt. Sinon, la DSN du mois suffit :
  `python -m moteur arret reprise CLE --dernier-jour AAAA-MM-JJ --le AAAA-MM-JJ`.
- **Corriger un signalement déjà parti** : un annule-et-remplace, jamais un
  second signalement normal. `arret signaler … --remplace IDENTIFIANT`, où
  l'identifiant est celui du certificat de conformité du dépôt précédent.

## Signalement de fin de contrat (FCTU, nature 07)

**Pour un intermittent (CDDU du spectacle) : pas de FCTU**, et ce n'est pas un
oubli. La fin d'un CDDU part dans la DSN mensuelle avec le circuit
dérogatoire (`S21.G00.62.017 = 01`) — tranché sur pièces le 18/09/2026. Si la
question revient, ne pas déranger France Travail : les sources sont dans
`atelier/reference/sources/francetravail-100-dsn/`.

**Pour un permanent (CDI, CDD)** : le signalement existe, il porte la paie du
solde de tout compte — `python -m moteur fin-contrat liste|signaler|documents
AAAA-MM CLE`, déposé comme une DSN (`depot-dsn --test` puis `--reel`). Voir
`moteur/fctu.py`.

## Congés Spectacles et Audiens

Depuis le passage en DSN, les congés spectacles (15,5 % du brut) et les
cotisations Audiens sont déclarés **par la DSN** : rien à envoyer à Audiens en
plus. Ne restent que les paiements, sur bordereau.

**Mais il reste un document dû au SALARIÉ** : le *certificat d'emploi Congés
Spectacles*, à lui remettre à l'issue de chaque contrat — c'est la pièce qui lui
sert à réclamer ses congés payés à Audiens. Il ne part nulle part ailleurs.

**Le moteur l'édite depuis le 09/09/2026** : `CertificatsCS_<Mois><Année>.pdf`,
produit par `python -m moteur mois` à côté du bulletin et de l'AEM. Audiens a
délivré le numéro d'édition sur papier blanc le 07/09/2026 (adhérent
`0123450001F`, premier N° CE `1`).

Deux choses à savoir quand la question revient :

- **Le N° CE est une séquence continue de l'employeur**, pas une séquence
  annuelle. Elle ne repart jamais à 1. Deux certificats au même numéro seraient
  deux chèques au même numéro : Audiens écrit qu'ils « ont valeur de chèque émis
  sur votre compte ».
- **L'arriéré n'est pas rattrapé.** Aucun certificat n'a jamais été remis depuis
  le 11/10/2019. Si l'utilisateur parle de congés spectacles, c'est le sujet à
  remonter. Détail dans `../prive/journal/CONGES-SPECTACLES.md` §6 bis.

## Échéances de paiement portées par la DSN

| Organisme | Bloc | Échéance |
|---|---|---|
| URSSAF | `S21.G00.20` réf. CS00012345 | 15 du mois suivant |
| Formation professionnelle (P0983) | `S21.G00.20` | fin du trimestre suivant |
| Congés Spectacles, Audiens, retraite | `S21.G00.20` | selon bordereau |

## En cas de rejet

Reprendre le message de net-entreprises, l'identifier dans le fichier
(`RUBRIQUE,'valeur'`), corriger la donnée dans `config/` ou dans le fichier de
saisie, régénérer, revalider. Ne jamais corriger le `.dsn` à la main : la
correction se perdrait au prochain mois.

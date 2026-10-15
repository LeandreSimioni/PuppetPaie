# La compagnie d'essai : un vrai dépôt de test sans `prive/`

Pour un développeur (ou une séance Claude dans le nuage) qui n'a pas les données de sa compagnie sur la machine, mais
qui veut faire passer une DSN, un signalement ou une DPAE **en test** sur la vraie plate-forme — le seul validateur du
projet (`atelier/CLAUDE.md`, règle 10). Lot E1, 15/10/2026 ; le code : `atelier/sources/paie/src/essai.rs`.

Pour développer et faire passer la suite de tests, **rien de tout cela n'est utile** : les tests tournent sur les
compagnies fictives et de faux serveurs, sans aucune variable.

## Ce qu'il faut poser dans l'environnement

Trois variables, posées dans les réglages de l'environnement (jamais dans la conversation, jamais dans un fichier du
dépôt). Une nouvelle séance les prend ; celle qui tourne ne les voit pas.

| variable | ce qu'elle porte |
|---|---|
| `PUPPETPAIE_COMPAGNIE` | l'identité de **votre** compagnie : un objet JSON sur une ligne (ci-dessous) |
| `NET_ENTREPRISES_MOTDEPASSE` | le mot de passe net-entreprises du déclarant (DSN, signalements, retours DSN) |
| `URSSAF_MOTDEPASSE` | le mot de passe du compte DPAE MtoM d'urssaf.fr (DPAE, retours DPAE) |

`PUPPETPAIE_COMPAGNIE` a **exactement** ces clés, toutes des textes (une clé qui manque ou en trop est refusée, en
la nommant ; chaque valeur est confrontée, au montage, au format des rubriques de la DSN qu'elle remplit) :

```
siren, nic                      le SIREN (9 chiffres) et le NIC de l'établissement (5) ; le SIRET s'en déduit
raison_sociale, adresse,        l'en-tête de l'entreprise (S10.G00.01, S21.G00.06, S21.G00.11 ; le bloc employeur
code_postal, commune, naf       de la DPAE)
medecine_travail                le code du centre de médecine du travail (C_MT_DCL de la DPAE, 10 caractères au plus)
declarant      {nom, prenom}    la personne inscrite au service DSN de net-entreprises, telle qu'inscrite
declarant_dsn  {civilite,       la personne que les organismes contactent : civilité 01 (Monsieur) ou 02 (Madame),
                nom_prenom,     nom et prénom, téléphone, courriel (S10.G00.02, S20.G00.07) ; qualite est le CODE du
                tel, email,     type de contact (S20.G00.07.004, deux caractères : 04 pour les cotisations de Sécurité
                qualite}        sociale ; cahier technique, bloc S20.G00.07), pas un intitulé
api_dpae       {nom, prenom,    le compte DPAE d'urssaf.fr, sans accents ; siret et service peuvent être vides
                siret, service} (siret vide : celui de la compagnie)
```

Ces valeurs ne sont pas des secrets, mais elles désignent une compagnie et des personnes : **elles ne s'affichent
jamais** (le moteur n'en cite aucune, ni en succès ni en refus ; un agent dit « posée » ou « absente »). Elles sont
les **vôtres** : un déclarant n'est habilité que pour ses SIRET, et c'est lui qui répond de ce qui part.

Pour savoir si elles sont posées, sans lire leur valeur :

```bash
for v in PUPPETPAIE_COMPAGNIE NET_ENTREPRISES_MOTDEPASSE URSSAF_MOTDEPASSE; do
  if [ -n "${!v}" ]; then echo "$v posee"; else echo "$v absente"; fi
done
```

## Monter la compagnie d'essai

```bash
paie essai monter [DOSSIER] [--depuis FICTIVE]
```

- copie une compagnie **fictive** livrée (défaut : `fictive-3090-grande`, qui a des permanents et des intermittents
  chaque mois ; `--depuis` prend un autre nom de `fictives/` ou un chemin) dans `DOSSIER` (défaut :
  `<dossier temporaire>/puppetpaie-essai`), **hors de tout dépôt Git** — le montage refuse sinon ;
- donne à sa fiche (`config/entreprise.json`) l'identité de `PUPPETPAIE_COMPAGNIE`, et en retire celle de la fictive ;
  tout le reste (salariés inventés, contrats, paies, barème, organismes) reste celui de la fictive ;
- ne garde ni ses retours, ni ses dépôts de DSN (joués sous le SIREN de la fictive) ; garde ses DPAE et ses contrats
  revenus signés, qui disent qu'un mois peut se fermer ;
- pose le marqueur d'une **compagnie d'essai** : ses dépôts de **test** vont au réseau (une fictive les joue), et
  **tout dépôt réel y est refusé** (par la dernière porte avant le réseau, et faute de coffre) ;
- relancé sur une compagnie d'essai existante, ne réécrit que l'identité (journal, DSN et retours gardés).

Un refus ne laisse rien sur le disque.

## La suite d'une séance

Toutes les commandes suivantes nomment la compagnie par `PUPPETPAIE_DONNEES`. Sans `prive/` voisin, c'est
indispensable : le moteur prendrait sinon le logiciel lui-même pour compagnie. Dans le nuage, le moteur Linux livré
est `atelier/sources/paie/bin/linux/paie` (il peut retarder sur les sources : sinon, `cargo build --release`).

```bash
paie essai monter
export PUPPETPAIE_DONNEES=/tmp/puppetpaie-essai     # le dossier que la commande a dit
```

**Une DSN mensuelle de test** :

```bash
paie mois paies/2026-09.json                        # le mois s'écrit sous votre identité (--forcer pour un mois passé)
paie depot-dsn sorties/2026-09/DSN_Septembre2026.dsn --test
paie retours                                        # quelques minutes plus tard : le bilan (CCO/BAN) fait foi
```

La base d'essai de net-entreprises garde les essais **par SIRET**, y compris ceux faits depuis un autre poste ; le
journal de la compagnie d'essai, neuf, ne les connaît pas (et les bilans ne disent pas le mois d'une déclaration : le
moteur ne peut pas le lire sur la plate-forme). Si l'on sait qu'un essai du même mois est déjà passé ailleurs, ou si
le bilan refuse l'ordre (B1-208-11, « dernier ordre accepté AAAAMMnn »), redéposer avec `--ordre` au-dessus : l'essai
part alors en annule-et-remplace à cet ordre. Sans `--ordre`, chaque nouvel essai du même mois part déjà à un ordre
plus grand que le précédent (le journal compte ses essais) : `--ordre` évite de remonter un à un.

```bash
paie depot-dsn sorties/2026-09/DSN_Septembre2026.dsn --test --ordre 7
```

Une DSN copiée de la fictive (écrite avant le montage, sous son SIREN) est refusée : produire le mois d'abord.

**Un signalement** (fin de contrat, arrêt, demande de taux) : `paie fin-contrat signaler AAAA-MM SALARIE --test`,
`paie arret …`, `paie pas --demander --test`, puis `paie depot-dsn <fichier> --test`. Son ordre se lit sur la
plate-forme à l'écriture (lot O1).

**Une DPAE de test** :

```bash
paie dpae paies/2026-11.json                        # --seulement SALARIE:AAAA-MM-JJ pour une seule embauche
paie deposer-dpae sorties/2026-11/DPAE_Novembre2026.txt --test
paie retours --dpae --flux <le flux rendu>
```

## Ce qui reste à confirmer au premier dépôt

Inférences, qu'aucun texte déposé ne dit (le premier bilan tranche) : que la plate-forme d'essai accepte un en-tête
réel sur un contenu fictif (NIR inventés à clé juste, références d'adhésion et organismes de la fictive) ; que la
civilité, la qualité et l'adresse ne sont pas confrontées au répertoire SIRENE.

Les retours relevés s'archivent dans la compagnie d'essai et disparaissent avec elle : ils ne remplacent pas le relevé
du poste de la compagnie.

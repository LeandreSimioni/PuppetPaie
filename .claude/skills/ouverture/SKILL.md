---
name: ouverture
description: Ouvrir PuppetPaie au début d'une session — sauvegarder le dossier privé dans le Drive et relever les retours des organismes —, consulter l'état du coffre, et savoir quelles actions sont permises. Déclencher au début de toute session de travail sur les paies, quand l'utilisateur demande « où on en est », s'il parle des retours, d'une sauvegarde, d'un mot de passe, ou avant de tenter un dépôt DSN ou DPAE.
---

# Ouvrir PuppetPaie

`moteur` : `atelier/sources/moteur/bin/windows/moteur.exe` sous Windows (`bin/linux/moteur`, `bin/mac/moteur`), depuis la racine
du dossier PuppetPaie. La compagnie est `prive/` à côté de `logiciel/` (ou le dossier de `PUPPETPAIE_DONNEES`).

## Au début d'une session

```
moteur ouverture        # sauvegarde Drive, retours DSN et DPAE, signatures ; --pas-avant 20 : rien si moins de 20 min
moteur rappels          # ce qui attend : DPAE en retard, DSN du mois d'avant, clôture
moteur echeancier       # la date, ce qui est à faire ce mois-ci, l'année qui vient
```

L'ouverture **n'échoue jamais** : sans réseau, sans compte Google, sans secret posé, elle le dit et rend la main.
`--json` partout pour une sortie lisible par un programme. Puis, au besoin :

```
moteur retours          # le détail des retours DSN (moteur retours --dpae : ceux des DPAE)
moteur pas              # les taux de prélèvement à la source, et ceux à demander
moteur incidents        # ce que le logiciel n'a pas su faire : plantages, refus, rejets
moteur journal          # les dépôts faits
moteur sauvegarder --point   # où en est la dernière sauvegarde, sans réseau
```

Une compagnie qui arrive : `moteur arrivee` dit ce qui manque (rien n'est écrit). Son historique va dans `import/` ;
`moteur importer` en rend l'aperçu, `moteur importer --ecrire` le reprend.

## Ce qui est permis

| | condition |
|---|---|
| lire, calculer, produire un mois, sauvegarder, relever les retours | aucune |
| déposer en test (`depot-dsn --test`, `deposer-dpae --test`) | aucune, **sans demander d'aval** |
| **déposer une DSN ou une DPAE en réel, envoyer, faire signer** | **le mot de passe du logiciel, tapé par un humain au moment de l'acte** |

**Deux mots de passe, deux chemins** :

- **Ouvrir le coffre = la séance** : `moteur coffre seance --fenetre`, une boîte, une fois, 12 h ; la lancer avant la
  première commande qui touche au coffre si la clé du poste ne l'ouvre pas (sinon la commande la demande elle-même).
  Elle sert aux relevés et aux tests ; elle n'est **jamais** une preuve pour un acte réel.
- **Un acte réel = le mot de passe retapé, à chaque fois**, par un humain.

Dépôt réel demandé : **dire que c'est à l'utilisateur de le lancer** (la fenêtre PuppetPaie, ou la commande `--reel`
dans son terminal). Ne pas contourner : ne pas taper le mot de passe à sa place, ne pas poser
`PUPPETPAIE_MOTDEPASSE` ni `NET_ENTREPRISES_MOTDEPASSE`, ne pas ouvrir le coffre dans un script. Un dépôt réel
engage la compagnie, ordres de télépaiement compris.

## Le coffre

```
moteur coffre                          # ce qui est posé, depuis quand, pour quoi : jamais une valeur
moteur coffre verifier                 # une tentative de connexion par compte, pas plus
moteur coffre poser net-entreprises    # à taper par l'utilisateur (DSN)
moteur coffre poser urssaf             # à taper par l'utilisateur (DPAE)
moteur coffre poser courriel --compte <adresse>
moteur coffre ouverture                # le mot de passe du logiciel
moteur coffre seance fermer            # refermer la séance
```

**Ne jamais lire, afficher ni copier `prive/config/coffre.bin`, `google-jeton.bin` ni `maj-cle-privee.bin`.**
`moteur coffre` répond à toutes les questions légitimes sans montrer un secret. Le Drive : `moteur google`
(`moteur google autoriser` pour le relier).

## Deux pièges

1. **Ne jamais régénérer une déclaration déjà déposée** : les numéros de contrat se renumérotent dès que les
   compteurs avancent. Pour regarder un fichier : `moteur depot-dsn <fichier> --apercu` (ni réseau, ni disque).
2. **Le test d'abord, toujours.** Le réel est refusé tant que ce contenu-là n'a pas réussi en test ; le moteur bascule
   lui-même l'envoi en test, en mémoire, sans toucher au fichier. La base de test a sa propre mémoire : un refus de
   test qui parle d'un essai antérieur se lit dans la phrase du moteur, il ne se contourne pas.

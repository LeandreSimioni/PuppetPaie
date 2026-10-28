---
name: contrat
description: Prépare les contrats d'engagement à durée déterminée d'usage (CDDU) des artistes et techniciens avec PuppetPaie, et leur envoi à la signature. Déclencher quand l'utilisateur demande un contrat, une lettre d'engagement, un CDDU, ou veut formaliser l'engagement d'un artiste sur des dates - sans forcément faire la paie tout de suite.
---

# Contrat d'engagement (CDDU)

Le contrat est écrit par le moteur `moteur` depuis la **même saisie** que la paie (`paies/AAAA-MM.json`) : il colle au
bulletin par construction. On ne le rédige jamais à la main.

## Le préparer

1. Écrire l'engagement dans la saisie du mois (compétence `paie`) : salarié, spectacle, lignes avec leurs dates et
   leur lieu. `moteur nouveau 2026-12` crée le fichier.
2. Le contrat part **à la signature depuis la fenêtre** : onglet Paie, « Envoyer à la signature… ». Le moteur écrit
   alors le PDF (`sorties/<mois>/envoi/Contrat_<CLE>_<numero>_<Mois><Année>.pdf`) et l'envoie avec un bouton
   « Je signe » ; la réponse est relevée par `moteur signatures` (et à chaque ouverture), puis le contrat contresigné
   repart au salarié. **C'est un acte** : le mot de passe du logiciel, tapé par l'utilisateur. Un agent prépare,
   l'utilisateur envoie.
3. Un contrat signé à la main se note dans la fenêtre (sa pastille, sur la ligne du contrat).

`moteur mois` refuse de fermer un mois tant que le contrat de chaque embauche n'est pas parti à la signature
(« contrat pas encore envoye a la signature »), comme pour la DPAE.

## Ce que le contrat dit

CDD d'usage au titre de l'article L.1242-2 3° du code du travail, de l'accord interbranche du 12 octobre 1998 et de la
convention collective de la compagnie (`idcc` de la fiche : 3090 ou 1285). Il porte : les parties, l'emploi et le
spectacle, le début et la fin, les dates de travail, le lieu, la rémunération brute et le net estimé, la date de
paiement, les congés payés par la Caisse des Congés Spectacles, les annexes VIII et X, et une clause de droits
voisins : la rémunération ne couvre que la prestation, toute captation demande une autorisation écrite distincte.
Le modèle est `logiciel/donnees/modeles/contrat-cddu.modele`.

## Points de vigilance

- **Le lieu** : sans `lieu` sur les lignes (ou l'engagement), le contrat dit « au lieu de la représentation ».
  Le renseigner quand il est connu.
- **Une captation** (vidéo, disque, diffusion) ne se règle pas dans ce contrat : autorisation écrite séparée, et le
  plus souvent une rémunération distincte. Le signaler si l'utilisateur parle de filmer.
- **Le numéro** (initiales, rang, année : `BR1.26.05`) est tiré par le moteur (`config/compteurs.json`) et repris tel
  quel dans la DSN ; il se fige à la production. Ne jamais le bricoler ; un contrat sur plusieurs mois porte dans
  chaque mois suivant le `numero` du premier (`moteur nouveau <mois> --reprendre`).
- **Un contrat signé qui change** (dates, lieu, rémunération) part en **avenant**, depuis la même action de la
  fenêtre ; un contrat déjà commencé ne s'annule pas.
- **Un permanent** (CDI, CDD ordinaire) : le moteur n'imprime que le contrat d'un CDD d'usage ; celui du permanent se
  rédige à part, et le mois le rappelle.
- Faire signer **avant** le premier jour de travail, comme la DPAE.

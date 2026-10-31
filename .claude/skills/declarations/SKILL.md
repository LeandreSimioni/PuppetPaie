---
name: declarations
description: Où, quand et comment déposer les déclarations sociales avec PuppetPaie - DPAE, DSN mensuelle, signalements (arrêt de travail, fin de contrat), Congés Spectacles, Audiens. Déclencher quand l'utilisateur demande une échéance, un calendrier de dépôt, ce qui reste à déclarer, ou signale un rejet de déclaration.
---

# Demander au moteur

Voir la compétence `paie` : le moteur (`atelier/sources/moteur/bin/<système>/moteur`, ou `moteur` dans une installation) se demande par `moteur routes` (catalogue), `moteur routes NOM` (fiche) et `echo '{"route":"…","charge":{…}}' | moteur route [--racine <compagnie>] [--texte]`. Ce qui est à faire, il le dit (`pour-demarrer`, `rappels`, `etat-du-mois`). Un acte réel : le moteur ouvre la boîte où l'humain tape le mot de passe ; le LLM ne le voit ni ne le tape, n'ouvre pas le coffre ; un test se fait sans demander. Un refus dit quoi corriger : le dire, ne rien inventer. Un manque : `signalement-envoyer`. Les `prive/config/*.bin` ne se lisent jamais.

Routes des déclarations : `echeancier`, `rappels`, `depot-apercu`, `deposer`, `suivre-depot`, `bilan-repetition`, `historique-mois`, `arret-obligations`, `fins-de-contrat`.

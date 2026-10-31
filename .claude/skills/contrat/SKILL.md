---
name: contrat
description: Prépare les contrats d'engagement à durée déterminée d'usage (CDDU) des artistes et techniciens avec PuppetPaie, et leur envoi à la signature. Déclencher quand l'utilisateur demande un contrat, une lettre d'engagement, un CDDU, ou veut formaliser l'engagement d'un artiste sur des dates.
---

# Demander au moteur

Voir la compétence `paie` : le moteur (`atelier/sources/moteur/bin/<système>/moteur`, ou `moteur` dans une installation) se demande par `moteur routes` (catalogue), `moteur routes NOM` (fiche) et `echo '{"route":"…","charge":{…}}' | moteur route [--racine <compagnie>] [--texte]`. Ce qui est à faire, il le dit (`pour-demarrer`, `rappels`, `etat-du-mois`). Un acte réel : le moteur ouvre la boîte où l'humain tape le mot de passe ; le LLM ne le voit ni ne le tape, n'ouvre pas le coffre ; un test se fait sans demander. Un refus dit quoi corriger : le dire, ne rien inventer. Un manque : `signalement-envoyer`. Les `prive/config/*.bin` ne se lisent jamais.

Routes du contrat : `signature-apercu`, `demander-signature`, `avenant-apercu`, `contrats-imprimer`, `contrat-recu`, `pieces-parties`.

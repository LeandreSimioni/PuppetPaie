---
name: ouverture
description: Ouvrir PuppetPaie au début d'une session (sauvegarde, retours des organismes), consulter l'état du coffre, savoir quelles actions sont permises. Déclencher au début de toute session sur les paies, quand l'utilisateur demande « où on en est », parle des retours, d'une sauvegarde, d'un mot de passe, ou avant un dépôt DSN ou DPAE.
---

# Demander au moteur

Voir la compétence `paie` : le moteur (`atelier/sources/moteur/bin/<système>/moteur`, ou `moteur` dans une installation) se demande par `moteur routes` (catalogue), `moteur routes NOM` (fiche) et `echo '{"route":"…","charge":{…}}' | moteur route [--racine <compagnie>] [--texte]`. Ce qui est à faire, il le dit (`pour-demarrer`, `rappels`, `etat-du-mois`). Un acte réel : le moteur ouvre la boîte où l'humain tape le mot de passe ; le LLM ne le voit ni ne le tape, n'ouvre pas le coffre ; un test se fait sans demander. Un refus dit quoi corriger : le dire, ne rien inventer. Un manque : `signalement-envoyer`. Les `prive/config/*.bin` ne se lisent jamais.

Routes de l'ouverture : `ouverture`, `pour-demarrer`, `rappels`, `coffre-etat`, `google-etat`.

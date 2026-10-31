---
name: paie
description: Fabrique les paies du spectacle vivant avec PuppetPaie - bulletins, certificats Congés Spectacles et DSN mensuelle, avec la DPAE et le contrat qui les précèdent. Déclencher dès que l'utilisateur demande une ou plusieurs paies, un bulletin, un cachet, une DSN, une DPAE/DUE, ou parle d'engager quelqu'un sur un spectacle à des dates données. Exemples : « fais 2 paies pour Fanchon et Valentin les 14 et 15 décembre pour Les Pupazzi », « la DSN d'octobre ».
---

# Demander au moteur

Le LLM est un second écran : il passe par les mêmes routes que la page. Le moteur décide, calcule, refuse et dit ce qui est à faire ; cette compétence dit seulement comment le lui demander. Ne calcule rien de tête, n'invente aucune valeur.

- Le moteur : `atelier/sources/moteur/bin/<système>/moteur` dans le dépôt, `moteur` dans une installation (avec `PUPPETPAIE_CODE=logiciel/donnees` pour un exécutable livré).
- `moteur routes` : le catalogue (chaque route : son geste, sa charge, un exemple, ce qu'elle rend, ses refus et quoi corriger, si elle lit, écrit ou engage, « avancé »). `moteur routes NOM` : une fiche.
- Jouer une route : `echo '{"route":"NOM","charge":{…}}' | moteur route [--racine <dossier de la compagnie>] [--texte]` (`--texte` : la phrase que la page affiche).
- Sans compagnie : la route `compagnies` (liste, `chemin`), puis `compagnie-ajouter`.
- Ce qui est à faire, le moteur le dit : routes `pour-demarrer`, `rappels`, `etat-du-mois` (le panneau des étapes).
- Un acte réel (dépôt réel, envoi, signature) : le moteur ouvre lui-même une boîte où l'humain tape le mot de passe. Le LLM ne voit ni ne tape jamais un mot de passe, ne pose aucune variable de mot de passe, n'ouvre pas le coffre. Un test (`reel: false`) se fait sans demander.
- Un refus dit quoi corriger : le dire à la personne, ne pas inventer de valeur.
- Écrire au développeur : la route `signalement-envoyer` (le champ `ecran` dit ce que le LLM a tenté).
- Les fichiers `prive/config/*.bin` ne se lisent jamais.

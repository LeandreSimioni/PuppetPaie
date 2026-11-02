# PuppetPaie en ligne de commande

PuppetPaie se pilote aussi en ligne de commande, par les mêmes gestes que sa fenêtre : chaque geste est une route, qui
rend les mêmes décisions, les mêmes phrases et les mêmes refus. Le logiciel calcule, décide et dit ce qui est à faire ;
ne calcule rien de tête et n'invente aucune valeur.

**Le programme.** L'installation est le dossier qui contient ce `logiciel/` (sous Windows, par défaut
`Documents\PuppetPaie`). Le programme est `logiciel/linux/moteur` sous Linux, `logiciel/mac/moteur` sous macOS,
`logiciel\puppetpaie.exe` sous Windows ; ci-dessous, `moteur`. Il a deux sous-commandes, `routes` et `route`.

**Demander.**
- `moteur routes` : le catalogue des routes (les plus courantes ; `--avance` : toutes ; `--json` : toutes, chacune dit si
  elle est avancée). `moteur routes NOM` : la fiche d'une route (son geste, sa charge, un exemple, ce qu'elle rend, ses
  refus et quoi corriger, si elle lit, écrit ou engage).
- Jouer une route : `echo '{"route":"NOM","charge":{…}}' | moteur route [--racine <dossier de la compagnie>] [--texte]`
  (`--texte` : la phrase que la fenêtre affiche). Un refus sort avec le code 1, la réponse dit pourquoi ; une commande
  mal dite (option inconnue), avec le code 2.
- La compagnie : la route `compagnies` (sans `--racine`) dit les compagnies de l'installation ; le `chemin` de chacune est
  le `--racine` de ses routes. Sans compagnie : `compagnie-ajouter`.
- Ce qui est à faire : les routes `pour-demarrer`, `rappels`, `etat-du-mois`.

**Les règles.**
- Un acte réel (dépôt réel, envoi, signature) : le programme ouvre lui-même une boîte où la personne tape le mot de passe
  du logiciel. Tu ne vois ni ne tapes jamais un mot de passe, ne poses aucune variable de mot de passe, n'ouvres pas le
  coffre. Un essai (`reel: false`) se fait sans demander.
- Un refus dit quoi corriger : dis-le à la personne, n'invente pas de valeur. Ce qui n'est pas une phrase (un chemin, une clé, un fichier, un détail) vient à côté de `erreur`, dans la réponse (`chemin`, `cle`, `fichier`, `detail`).
- Écrire au développeur : la route `signalement-envoyer`, avec ou sans `--racine` (le champ `ecran` dit ce que tu as tenté).
- Les fichiers `prive/config/*.bin` (et ceux d'une compagnie, `config/*.bin`) ne se lisent jamais.

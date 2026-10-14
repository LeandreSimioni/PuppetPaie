# Le bac à sable de la mise à jour

Une installation de compagnie : `logiciel/` (le programme, en retard d'une version) et
`prive/`, la démo 1285 copiée (une version se contrôle sur les données de la compagnie
avant d'être acceptée), avec `prive/config/maj.json` (pas l'éditeur ; les versions
viennent de `atelier/depot/`). Le marqueur `COPIE-DE-DEVELOPPEMENT.json` de la démo est
retiré : une copie de développement ne reçoit jamais de mise à jour.

Une seule fenêtre PuppetPaie par poste : si une autre tourne (un autre agent), le
lancement la ramène devant et s'arrête, sans mise à jour. La fermer d'abord.

Essayer une mise à jour :

1. Chez l'éditeur, publier une version plus récente que `logiciel/VERSION` du bac
   (`paie maj --publier`, dans un PowerShell ordinaire).
2. `logiciel\puppetpaie.exe maj` : il trouve la version, la télécharge, vérifie la
   signature, la contrôle et la pose dans `prive/maj/` (« prête »).
3. Double-cliquer `logiciel\puppetpaie.exe` : aucune fenêtre ne doit s'ouvrir avant la
   neuve. `prive/journal/interface.log` dit ce qui s'est passé ; l'ancienne version est
   dans `logiciel/.avant/`.

Pour recommencer : `logiciel\puppetpaie.exe maj --revenir`.

`logiciel/`, `prive/maj/` et `prive/journal/` ne vont pas dans le dépôt (`.gitignore`).

# Le bac à sable de la mise à jour

Une installation : `logiciel/` (le programme, en retard d'une version) et `prive/`, la
fictive 1285 copiée (une version se contrôle sur les données des compagnies de
l'installation avant d'être acceptée). Le réglage de la mise à jour est celui de
l'installation (lot U1) ; le bac le porte encore à l'ancienne, dans
`prive/config/maj.json` (lu en repli : pas l'éditeur ; les versions viennent de
`atelier/depot/`). Le marqueur `COPIE-DE-DEVELOPPEMENT.json` de la fictive est retiré :
`prive/` est la compagnie du poste.

Une seule fenêtre PuppetPaie par poste : si une autre tourne (un autre agent), le
lancement la ramène devant et s'arrête, sans mise à jour. La fermer d'abord.

Essayer une mise à jour :

1. Chez l'éditeur, publier une version plus récente que `logiciel/VERSION` du bac
   (`paie maj --publier`, dans un PowerShell ordinaire).
2. `logiciel\puppetpaie.exe maj` : il trouve la version, la télécharge, vérifie la
   signature, la contrôle et la pose dans `maj/` (« prête »), à côté de `logiciel/`.
3. Double-cliquer `logiciel\puppetpaie.exe` : aucune fenêtre ne doit s'ouvrir avant la
   neuve. `maj/journal.log` dit ce qui s'est passé ; l'ancienne version est
   dans `logiciel/.avant/`.

Pour recommencer : `logiciel\puppetpaie.exe maj --revenir`.

`logiciel/`, `maj/` et `prive/journal/` ne vont pas dans le dépôt (`.gitignore`).

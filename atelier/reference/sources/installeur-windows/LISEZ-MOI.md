# Installeur Windows — ce qui a été lu pour le construire (lot I1, 14/10/2026)

Relevé le 14/10/2026 par l'Opus du lot I1, déposé par l'orchestrateur. Où le moteur s'en sert : nulle part dans `paie/` ;
ces textes fondent `atelier/installeur/puppetpaie.nsi` et `atelier/installeur/construire.sh` (la compilation croisée et
l'installeur NSIS), décrits dans `prive/journal/finitions/PASSATION-I1.md`.

| fichier | d'où | ce qu'on en lit |
|---|---|---|
| `tauri-windows-installer-20261014.{html,txt}` | https://v2.tauri.app/distribute/windows-installer/ | la compilation croisée de la fenêtre Tauri 2 pour Windows ; les options d'installation de WebView2 (téléchargeur, embarqué, fixe) — l'installeur n'en embarque aucune : il renvoie à la page de Microsoft |
| `microsoft-webview2-distribution-20261014.{html,txt}` | https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/distribution | la détection du Runtime WebView2 par la valeur `pv` de la clé de registre (ce que le `.nsi` vérifie avant de proposer la page de Microsoft) |
| `microsoft-webview2-telechargement-20261014.html` | https://developer.microsoft.com/en-us/microsoft-edge/webview2/ | la page que l'installeur propose d'ouvrir quand WebView2 manque |
| `microsoft-msi-uninstall-registry-key-20261014.{html,txt}` | https://learn.microsoft.com/en-us/windows/win32/msi/uninstall-registry-key | les valeurs de la clé `Uninstall` (DisplayName, UninstallString, InstallLocation…) que le `.nsi` écrit sous HKCU |
| `nsis-exemple-install-per-user-20261014.nsi` | https://raw.githubusercontent.com/kichik/nsis/master/Examples/install-per-user.nsi | l'installation pour l'utilisateur seul (`RequestExecutionLevel user`, `SetShellVarContext current`, `$LOCALAPPDATA`) |
| `nsis-exemple-example2-20261014.nsi` | https://raw.githubusercontent.com/kichik/nsis/master/Examples/example2.nsi | la forme d'un installeur avec désinstalleur |

La documentation de nsis.sourceforge.io a répondu 403 au conteneur : non lue, à redéposer depuis le poste si l'on en a
besoin. Les décisions prises sans texte (dossier par défaut `%LOCALAPPDATA%\PuppetPaie`, `EDITION.json` toujours démo,
désinstalleur à la racine) sont des choix, nommés comme tels dans la passation, à valider par Léandre.

## Dans le dépôt public (15/10/2026)

Ce dossier est dans le dépôt public de PuppetPaie. Les originaux suivants, d'un organisme privé, n'y sont pas : seule leur transcription y est (`pdftotext -layout` pour un PDF, une feuille de calcul en CSV, feuille par feuille, le texte d'une page HTML), et c'est elle que citent les renvois. Les condensats donnés plus haut sont ceux des originaux, gardés dans le dépôt privé (`reference-privee/sources/installeur-windows/`).

- `microsoft-msi-uninstall-registry-key-20261014.html` : transcription `microsoft-msi-uninstall-registry-key-20261014.txt`
- `microsoft-webview2-distribution-20261014.html` : transcription `microsoft-webview2-distribution-20261014.txt`
- `microsoft-webview2-telechargement-20261014.html` : transcription `microsoft-webview2-telechargement-20261014.txt`
- `nsis-exemple-example2-20261014.nsi` : transcription `nsis-exemple-example2-20261014.txt`
- `nsis-exemple-install-per-user-20261014.nsi` : transcription `nsis-exemple-install-per-user-20261014.txt`
- `tauri-windows-installer-20261014.html` : transcription `tauri-windows-installer-20261014.txt`

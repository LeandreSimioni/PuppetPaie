# PuppetPaie

Un logiciel de paie pour le **spectacle vivant** : bulletins, contrats, DPAE,
certificats Congés Spectacles et DSN mensuelle (plus d'AEM depuis le
01/06/2026 : 100 % DSN), pour les
intermittents au cachet comme pour les salariés permanents. Il est écrit pour
**toute compagnie**, quelle que soit sa convention ; son premier utilisateur
est aussi son banc d'essai, une compagnie chez qui il
remplace CoolPaie (150 €/an). Le moteur est écrit en **Rust**,
multiplateforme (`atelier/sources/paie/`, le programme `paie`, et la fenêtre
`atelier/sources/fenetre/`, le programme `puppetpaie`) ; le Python est retiré
du produit depuis le 09/10/2026 (lot K).

Le calcul n'est pas confié au modèle de langage : il est fait par du code
déterministe, calé sur les fichiers réellement déposés depuis 2021.
Claude sert à parler à ce moteur en français, pas à calculer la paie.

## Vérifié contre CoolPaie

| Ce qui est rejoué | Résultat |
|---|---|
| Bulletin FAVART juin 2026 (2 cachets, 260 € brut) | identique **au centime** — brut, assiette CSG, 149,66 € patronal, 58,68 € salarial, 201,32 € net, 208,81 € imposable |
| DSN de juillet 2026 (4 contrats, 997 lignes) | **identique ligne pour ligne**, hors le nom du logiciel émetteur |
| DPAE du 28/08/2026 (6 enregistrements) | **identiques octet pour octet** — mais c'est le seul fichier DPAE laissé par CoolPaie, voir docs/ARCHITECTURE.md |
| 10 fichiers AEM V5 de 2026 | **identiques octet pour octet** |
| Schéma AEM sur 189 enregistrements 2021-2026 | validé à 189/189 |

Ce rapport est celui du calcul Python, retiré le 02/10/2026 : le moteur Rust
(`atelier/sources/paie/`), qui calcule depuis, en reproduisait les sorties à l'octet sur tous
les mois des compagnies fictives et de la compagnie.

## L'arrivée d'une compagnie

```
paie arrivee
```

dit, dans l'ordre, ce qui est prêt et ce qui manque — la fiche de
l'entreprise, l'import de CoolPaie (sans objet si la compagnie n'en vient
pas), les fiches des salariés, l'effectif (les CRM de nature 118 de l'Urssaf,
ou la date de la première embauche), le taux AT/MP notifié, les taux de PAS,
les accès net-entreprises et Urssaf — avec, pour chaque manque, la commande
qui le fait avancer. Elle n'écrit rien et sort en erreur tant qu'une étape
est à faire.

## Reprendre l'historique : l'import

Ce qui est déjà déclaré ne se ressaisit pas. **Un seul dossier, tout dedans**
(10/10/2026) : le dossier `import/` de la compagnie reçoit tout ce que
l'ancien logiciel a laissé — bulletins de paie en PDF, DSN, AEM, archives
zip —, sans qu'aucun logiciel ait à être nommé ; le moteur trie par le
contenu des fichiers et reconstruit tout ce qu'il peut.

```
paie importer                  # l'aperçu, en JSON (`tri` : le sort de chaque fichier, et pourquoi) : rien n'est écrit
paie importer --ecrire         # le reprend
paie importer --dossier D      # lit un autre dossier que import/
```

Ou, dans l'interface : **Configuration › Reprendre l'historique** — le
dossier prérempli, « Parcourir… » pour en prendre un autre, « Ouvrir le
dossier » pour y déposer les fichiers, « Voir ce qui serait repris » (le même
compte rendu), puis « Reprendre ». Les fichiers lus sont copiés dans
l'historique de la compagnie ; le dossier `import/` peut ensuite être vidé.

| Ce qui est lu | D'où | Ce qui est écrit |
|---|---|---|
| l'entreprise (SIREN, NAF, adresse, IDCC, licence, affiliation CS) | S10 et blocs 06/11/40 des DSN | `config/entreprise.json`, **seulement si la fiche est vide** |
| les salariés (NIR, état civil, adresse, courriel, emploi, PCS, statut) | bloc S21.G00.30 et dernier bloc 40 de la **dernière** DSN où ils figurent | une fiche par NIR inconnu ; une fiche existante n'est **jamais modifiée**, ce que la DSN disait autrement est signalé |
| les spectacles (numéros d'objet) | `S21.G00.40.050` des DSN, et les AEM | une fiche `objet-<numéro>` par numéro inconnu, titre « A COMPLETER » (ni la DSN ni l'AEM ne portent le titre), active seulement si le numéro est de l'année |
| l'historique des contrats (dates, cachets, heures, jours, brut) | chaque enregistrement AEM, dédoublonné | `historique/engagements.json` |

**L'import COPIE ce qu'il reprend** — les DSN, les AEM et les PDF dont une
page au moins est reprise — dans `historique/bulletins/`, sous le nom de sa
copie (une empreinte : rien de personnel dans le nom), et relit l'aperçu
**dans ces copies** (`paie/src/importation_ecrite.rs`). Un fichier déjà copié
n'est jamais écrasé ; s'il revient avec un autre contenu sous le même nom,
l'import le signale. Pour que le dossier d'un salarié ait ses années
CoolPaie, les PDF sont à sortir de CoolPaie avant :
`docs/COOLPAIE-SORTIR-SES-DOCUMENTS.md`.

Jusqu'au 10/10/2026 (journal, §216-§217), l'import lisait le dossier `export/`
de CoolPaie et le copiait dans `historique/coolpaie/` (`DSN/`, `AEM_EDI/`, les
PDF). Le moteur relit encore cette copie chez une compagnie importée ainsi
(la DSN déposée d'un mois, le dossier d'un salarié), mais ni l'écran ni la
ligne de commande ne passent plus par là.

L'import se rejoue sans dommage : ce qui existe est laissé, ce qui manque est
ajouté. La base CoolPaie elle-même (`bases/*.CSOC`) est chiffrée et n'est pas
lue — on n'en a pas besoin.

**Pourquoi l'historique compte.** Les droits à l'assurance chômage (annexes
8 et 10 : 507 h sur 12 mois, un cachet = 12 h) se comptent sur tout ce que la
compagnie a déclaré, et `paie droits` comme le bloc de cumul du
bulletin lisent **une seule source, la nôtre** : l'historique importé, puis
nos fichiers de paie. Un mois qui a son fichier dans `paies/` est lu là, et
l'historique se tait pour ce mois-là — pendant un double run, rien n'est
compté deux fois. Une fois l'import fait, le logiciel n'a plus besoin du
dossier `import/` : il lit ses copies.

## Faire une paie

Trois façons, au choix — elles font le même travail, avec les mêmes
garde-fous, parce qu'elles passent toutes par le même moteur
(`atelier/sources/paie/` : ses routes, que la fenêtre appelle et que
`paie route NOM` ouvre à la ligne de commande).

**Dans le chat.** « Fais 2 paies pour Celi et Marin les 2 et 3 octobre 26,
pour Aaah Zerbinette. » Claude écrit le fichier de saisie, lance le moteur et
rend les fichiers. La compétence `paie` décrit exactement ce qu'il fait.

**Dans l'interface.** Double-cliquer sur le raccourci de son système (à la
racine du dossier sous Windows, dans `atelier/` sous Mac et Linux) : une
fenêtre s'ouvre, et c'est tout le logiciel.

| système | ses programmes dans `logiciel/` | pour l'installer | sa mise à jour | raccourci | ce qu'il lance |
|---|---|---|---|---|---|
| Windows | `puppetpaie.exe` et `WebView2Loader.dll`, à la racine (suivis par Git ; l'installeur Windows ne pose qu'eux) | `PuppetPaie-<JJ.MM.AAAA>-installeur.exe` | `puppetpaie-<version>-windows.zip` | `PuppetPaie.cmd`, à la racine | `logiciel/puppetpaie.exe` (avec `WebView2Loader.dll`, `donnees/` et `interface/` à côté) |
| Linux | `linux/puppetpaie` (la fenêtre) et `linux/paie` (le moteur) | `puppetpaie-<version>-linux-installation.zip` | `puppetpaie-<version>-linux.zip` | `logiciel/linux/puppetpaie` directement, ou `atelier/PuppetPaie.sh` (clic droit › Lancer comme un programme, ou `./PuppetPaie.sh`) | `logiciel/linux/puppetpaie` (la fenêtre), sinon `logiciel/linux/paie interface` ; dans un dépôt de travail, le programme de `atelier/sources/paie/bin/linux/` |
| Mac | `mac/puppetpaie` et `mac/paie`, universels (Intel et Apple) | `puppetpaie-<version>-mac-installation.zip` | `puppetpaie-<version>-mac.zip` | `atelier/PuppetPaie.command` (clic droit › Ouvrir la première fois) | `atelier/PuppetPaie.sh` : `logiciel/mac/puppetpaie`, sinon `logiciel/mac/paie interface` ; dans un dépôt de travail, `atelier/sources/paie/bin/mac/` |
| tous | les systèmes publiés à la fois | `puppetpaie-<version>-tout-systeme-installation.zip` | `puppetpaie-<version>-tout-systeme.zip` | celui du système où l'on est | un dossier portable qui passe d'une machine à l'autre ; le repli d'une installation dont le système n'a pas son archive |

**Pour installer, on prend l'installeur (Windows) ou l'archive d'installation**
(`…-installation.zip`, ticket #246) : `logiciel/` et, à côté, `fictives/`, les
quatre compagnies fictives. Décompressée dans un dossier neuf (Linux, Mac, un
dossier portable), elle s'ouvre bridée sur ses fictives ; le geste de
l'accueil la débloque.

**Bridé, le « + » de l'accueil ajoute une compagnie fictive** (`fictives/ajoutee-<nom>`,
ticket #236) dont rien ne part. **Au déblocage**, l'accueil demande lesquelles
garder, et redemande tant qu'il en reste : une compagnie gardée passe sous
`compagnies/`, devient réelle (son mot de passe du logiciel se choisit à sa
première ouverture, son coffre est gardé) et porte `config/demo.json`, qui fait
dire à chaque mois saisi en démo que rien n'en est parti ; une compagnie
décochée est supprimée. **Supprimer une compagnie** se fait sous la fiche
Entreprise de sa Configuration : une compagnie créée en démo sur une simple
confirmation, une compagnie réelle sur le mot de passe du logiciel (le double
des bulletins se conserve cinq ans, article L3243-4) ; une fictive livrée ne
se supprime pas, elle se remet à zéro. Le dossier est renommé en dossier caché
(`.supprimee-…`) puis effacé ; un reste s'efface au lancement suivant
(`paie/src/interface/suppression.rs`).

**L'archive de mise à jour** (`puppetpaie-<version>-<système>.zip`, celle que
`derniere.json` nomme et que le logiciel télécharge tout seul) ne sert pas à
installer : elle porte le contenu de `logiciel/` à sa racine, et rien d'autre.
Cette forme est un contrat avec toutes les installations déjà posées, dont le
code la déballe telle quelle : elle ne change plus (tête de `paie/src/maj.rs`).

**Les compagnies fictives voyagent à part** (`puppetpaie-<version>-fictives.zip`,
signée, nommée dans `derniere.json` sous `fictives` avec l'empreinte de son
contenu). Une installation ne la télécharge que si les fictives ont changé
depuis celles qu'elle a (`fictives/.livraison.json`) ; elles remplacent alors,
au lancement suivant, les fictives que la livraison avait posées, jouées ou
non, photo d'origine comprise ; une fictive retirée disparaît, une nouvelle
apparaît. Un dossier que la livraison n'a pas posé (une fictive copiée et
renommée, une fictive faite sur le poste) n'est jamais touché, ni rien hors de
`fictives/`. La mise à jour du logiciel, elle, ne touche jamais `fictives/`.

Le flux « Publier » compile les programmes des trois systèmes (Linux et Mac ne
sont posés dans `logiciel/linux/` et `logiciel/mac/` que par lui : jamais
suivis par Git) et publie les quatre archives de mise à jour ensemble (Léandre, 15/10/2026 :
« Il y a les quatre : une par système et une tout-système, ensemble ») ;
chaque installation prend celle de son système (`paie/src/maj.rs`). Au
lancement de « Publier », on choisit ce qu'on publie (`systemes` : `tous`,
`windows-linux`, `windows`) : seules les archives fabriquées sont publiées,
celle de tous porte ce qui a été fabriqué (et le dit, `systemes` dans
`derniere.json`), et une installation dont le système n'est pas publié garde
sa version et le dit, sans rien télécharger.

Plus de Python (lot J5) : la fenêtre est le programme `puppetpaie`, une
fenêtre Tauri (WebView2 sous Windows, WebKit sous Mac et Linux) qui tient le
serveur de la page et l'arrête avec elle (`atelier/sources/fenetre/`).
Sans moteur de rendu (un Linux sans WebKitGTK), le lanceur passe à
`paie interface`, qui ouvre **la même page dans le navigateur**, en fenêtre
d'application si un Chromium est installé, dans un onglet sinon. Si un mot de
passe du logiciel est posé, la fenêtre s'ouvre sur une boîte qui le demande.

```bash
paie interface       # sans fenêtre : la page dans le navigateur (--port N, --sans-navigateur, --veille MINUTES)
```

La page : le mois, les engagements, l'aperçu
chiffré qui se recalcule à mesure, et la production. Un second écran édite
l'entreprise, les salariés et les spectacles, en vérifiant chaque champ au
format qu'exigent la DSN et l'AEM. Le serveur n'écoute que la machine
(127.0.0.1), sur un port propre à l'installation et stable d'un lancement à
l'autre (7331 à 7429, retenu dans `.fenetre.json`) — pour que le navigateur
reconnaisse le site `puppetpaie.localhost` et retrouve le mot de passe du
logiciel, et que deux installations s'ouvrent côte à côte sur un poste — et
exige un jeton. Les dépôts et les envois se font depuis l'interface, mot de
passe du logiciel retapé à chaque acte.

**En ligne de commande.**

```bash
paie nouveau 2026-10
```

Éditer `paies/2026-10.json` (noms, dates, spectacle), puis :

```bash
paie mois paies/2026-10.json
```

Sortie dans `sorties/2026-10/` :

| Fichier | Où il va |
|---|---|
| `Contrats_Octobre2026.pdf` | à faire signer |
| `DPAE_Octobre2026.txt` (+ récapitulatif) | net-entreprises.fr, **avant** le début du contrat |
| `Bulletins_Octobre2026.pdf` | aux artistes |
| `CertificatsCS_Octobre2026.pdf` (+ récapitulatif) | **aux artistes** — congés spectacles ; rien à envoyer à Audiens |
| `DSN_Octobre2026.dsn` | dépôt en test (`depot-dsn --test`), lecture du bilan, puis dépôt réel |

Plus d'AEM depuis le 01/06/2026 (100 % DSN) : France Travail édite
l'attestation lui-même, et `paie aer --mois 2026-10` range celles qu'il a
envoyées.

Puis, pour envoyer aux salariés :

```
paie envoi paies/2026-10.json
```

Un message par salarié — bulletin, certificat Congés Spectacles et AER France Travail —
et une page de relecture qui s'ouvre dans le navigateur : chaque message y est
affiché en entier, chaque pièce jointe s'ouvre d'un clic, et rien ne part avant
la confirmation. Les pièces jointes viennent de `sorties/2026-10/envoi/`, où
chaque document est écrit salarié par salarié : le fichier groupé porte la paie
de tout le monde et ne doit jamais partir. Un salarié qui a plusieurs contrats
dans le mois a **un** fichier, avec un bulletin par page.

### Un mois produit dont les pièces individuelles manquent

À l'écran : le bouton « Écrire les documents des salariés ». La commande
`documents` du Python n'a pas de commande en Rust ; sa route, oui :

```
echo '{"route": "ecrire-documents", "charge": {"mois": "2026-08"}}' | paie route --racine prive
```

(`--racine` : le dossier de la compagnie, ici depuis la racine du dépôt.)

Le cas est celui d'un mois produit par une version antérieure au découpage par
salarié : le fichier groupé est archivé, `sorties/<mois>/envoi/` est vide, et
`envoi` refuse — « les documents du mois ne sont pas encore écrits ». Cette
commande les **redécoupe** : elle ne reproduit rien, ne redéclare rien, ne fait
avancer aucun compteur de bulletin ni de contrat, et lit les numéros dans le
fichier archivé. Ne pas reproduire le mois à la place : sur un mois déjà
déposé, les numéros de contrat changeraient (voir `CLAUDE.md`, règle 4). À
l'écran, c'est ce bouton.

## La boîte courriel : un mot de passe d'application

Depuis le 17/09/2026, l'envoi (bulletins, contrats) et la relève des
signatures passent par **IMAP et SMTP**, avec un mot de passe d'application
que chaque compagnie fabrique dans son compte Google : validation en deux
étapes activée, puis myaccount.google.com › Sécurité › **Mots de passe des
applications**. Seize lettres, à poser avec l'adresse de la boîte :

```bash
paie coffre poser courriel --compte compagnie@gmail.com
paie coffre verifier          # la boîte s'ouvre-t-elle
```

Ou dans la fenêtre : Configuration › Mots de passe. Rien à demander à
Google : aucune permission Gmail, donc ni audit ni plafond de comptes. Une
autre messagerie que Gmail se déclare dans `config/courriel.json`
(`{"imap": ..., "smtp": ..., "port_smtp": 465}` ; 587 = STARTTLS).

C'est la **seule** boîte : sans ce mot de passe, rien ne part aux salariés
et les signatures ne reviennent pas. Le compte Google (ci-dessous) n'y est
pour rien, il ne porte que le Drive.

## Le compte Google : le Drive, rien d'autre

La sauvegarde du dossier privé et l'archivage d'un mois vont dans le Drive
d'un compte Google lié **une fois** : Configuration › Mots de passe ›
**Lier le Drive**, ou

```
paie google autoriser
```

Une fenêtre Google s'ouvre dans le navigateur, on choisit son compte, on
accepte, on revient au logiciel. C'est tout : pas de console Google, pas de
projet à créer, rien à recopier. Le logiciel embarque **PuppetPaie**,
l'application déclarée une fois par la compagnie, la même pour tout le monde
— comme le font rclone ou gcloud. Lier ou oublier le compte demande le mot de
passe du logiciel, comme tout acte.

```
paie google etat          # vérifie que l'autorisation tient
paie google oublier       # efface le jeton local
```

Deux permissions demandées, toutes deux *non sensibles* au sens de Google :
**`drive.file`** (écrire dans le Drive, uniquement les fichiers créés par
l'application — elle ne voit rien d'autre) et **`userinfo.email`** (connaître
l'adresse du compte). Depuis le 18/09/2026 l'application n'en déclare pas
d'autre : plus aucune permission Gmail, donc ni audit de sécurité ni plafond
de comptes. L'accès se retire quand on veut sur myaccount.google.com ›
Sécurité › Vos connexions à des applications tierces.

### Le contrat se signe d'un clic

`Demander la signature` envoie le contrat avec un bouton **« Je signe »**. Le
salarié appuie dessus : sa messagerie s'ouvre sur une réponse toute écrite
(« Je soussigné(e)… Code de signature : SIG-XXXXXX »), il l'envoie. À
l'ouverture du logiciel et toutes les vingt minutes ensuite — ou par
`paie signatures` — la réponse est relevée dans la boîte : le
journal note le contrat **signé, à l'heure du message**, le message est
archivé tel quel dans `sorties/<mois>/envoi/` (`Signature_*.eml`, c'est la
preuve), et le contrat **contresigné** — la case de l'artiste porte qui,
quand, depuis quelle adresse, quel code — scellé contre la modification, lui
est renvoyé aussitôt. Une réponse venue d'une autre adresse que celle de la
fiche est signalée, pas retenue ; un contrat signé à la main se note
toujours d'un clic sur sa pastille.

C'est une signature électronique *simple* (art. 1367 du code civil), pas un
service certifié : le procédé identifie le signataire (son adresse) et le lie
à l'acte (le code, le message gardé).

Le jeton est écrit dans `config/google-jeton.bin`, scellé par la clé de ce
poste (AES-256-GCM, voir `paie/src/coffre.rs`) — illisible sur une autre
machine, et illisible dans une sauvegarde du dossier privé, puisque la clé vit
ailleurs.

Tant que l'application n'est pas passée par la vérification Google, l'écran
« Google n'a pas validé cette application » apparaît la première fois :
*Paramètres avancés* → *Accéder à PuppetPaie…*. La marche à suivre pour la faire
disparaître, et celle pour utiliser sa propre application Google plutôt que la
nôtre, étaient en tête de `moteur/google.py` (Python, retiré au lot K) ; la
déclaration de l'application est `logiciel/donnees/google-application.json`.

## À l'ouverture : le coffre, la sauvegarde, les retours

Depuis le 11/09/2026, le logiciel travaille seul dès qu'on l'ouvre — interface
ou Claude Code, c'est la même fonction :

```bash
paie ouverture
```

1. il **sauvegarde tout le dossier privé** dans le Drive, par différences :
   seuls les fichiers neufs ou modifiés repartent (comparaison sur l'empreinte
   MD5 que Google publie lui-même) ;
2. il **relève les retours** des organismes et les archive dans `retours/` ;
3. il **relève les signatures** (les réponses « Je signe » dans la boîte) ;
4. une fois par semaine, il **vérifiait les sources** des données livrées
   (minima, métiers — voir plus bas) : pas encore porté en Rust.

L'interface le fait au démarrage puis toutes les 20 minutes
(`--veille MINUTES`, `0` pour une seule fois à l'ouverture), et affiche un
bandeau qui dit où ça en est. Le passage périodique ne touche au réseau que si
quelque chose a bougé sur le disque — sinon il ne coûte rien.

**Une ouverture n'échoue jamais.** Pas de réseau, pas de compte Google, pas de
secret posé : elle le note et rend la main. Une paie se calcule très bien hors
ligne.

### Où mettre les mots de passe

Deux comptes distincts, et un troisième mot de passe qui n'appartient qu'au
logiciel :

```bash
paie coffre poser net-entreprises   # la DSN
paie coffre poser urssaf            # la DPAE
paie coffre poser courriel --compte compagnie@gmail.com   # la boîte
paie coffre ouverture               # le mot de passe PuppetPaie
paie coffre                         # ce qui est posé, jamais les valeurs
paie coffre verifier                # ils ouvrent bien une session ?
```

**`--fenetre` si la saisie du terminal ne capture pas tout.** C'est arrivé le
11/09/2026 : la saisie masquée n'a lu qu'un caractère des deux mots de passe,
et la panne n'est ressortie qu'une heure plus tard sous la forme d'un « HTTP
422 » qui ne désignait rien. Le coffre refuse désormais un secret de moins de
six caractères, et `--fenetre` ouvre un champ masqué qui ne dépend pas du
terminal :

```bash
paie coffre poser net-entreprises --fenetre
```

`coffre verifier` fait **une** tentative d'authentification par compte — jamais
plus : au-delà de quelques échecs, net-entreprises bloque temporairement le
compte.

Chacun se tape dans un champ masqué, deux fois : jamais sur la ligne de
commande, où il resterait dans l'historique du shell. Le fichier est
`prive/config/coffre.bin`, **scellé par AES-256-GCM** — sur Windows, sur Mac
et sur Linux, depuis le 20/09/2026 (avant : DPAPI, et du clair ailleurs).

Ce qui l'ouvre, ce sont deux enveloppes d'une même clé maîtresse :

| | |
|---|---|
| la **clé de ce poste** | `~/.config/puppetpaie/cle-poste.bin`, ou `%APPDATA%\PuppetPaie\`. Elle permet de travailler seul : relever les retours, sauvegarder, déposer en test. Dans un dossier **portable** (`PORTABLE.json` à côté de `logiciel/` : une clé USB, choisie à l'installation), elle vit dans ce dossier et voyage avec lui, sur n'importe quel ordinateur et n'importe quel système ; le coffre y reste protégé par le mot de passe du logiciel. `PUPPETPAIE_CLE` passe devant les deux. |
| le **mot de passe du logiciel** | il ouvre le coffre sur une machine qui n'a pas cette clé — une sauvegarde restaurée, un poste neuf — et repose une clé au passage. |

La clé du poste vit **hors du dossier des données**, et c'est tout l'intérêt :
`prive/` est sauvegardé sur le Drive et copié ; une sauvegarde ne transporte
donc qu'un fichier illisible. En contrepartie, la perdre c'est perdre
`google-jeton.bin`, qui n'a pas de seconde enveloppe. `maj-cle-privee.bin` (la
clé qui signe les mises à jour) est scellée depuis le 14/10/2026 par la clé
maîtresse du coffre : elle s'ouvre où il s'ouvre, et seulement avec lui.

**Un coffre écrit avant le 20/09/2026 ne se convertit pas.** Il n'y a pas de
commande de migration, et c'est voulu : elle n'aurait servi qu'une fois, à une
seule personne, pour trois mots de passe qui se retapent en une minute. On
supprime les trois fichiers — `coffre.bin`, `google-jeton.bin` et
`maj-cle-privee.bin`, fermés de la même façon — et on repose :

```bash
paie coffre ouverture
paie coffre poser net-entreprises
paie coffre poser urssaf
paie coffre poser courriel --compte <adresse>
paie google autoriser          # relier le Drive
paie maj --cle --remplacer     # si ce poste publie les mises à jour
```

La dernière ligne rend une nouvelle clé publique, à reporter dans
`paie/src/maj.rs` (`CLE_PUBLIQUE`). Tant qu'aucune autre compagnie ne reçoit de
mise à jour, ça ne coûte rien ; le jour où ce sera le cas, il faudra la leur
transmettre autrement.

### Le mot de passe du logiciel, et ce qu'il autorise

Tout se règle aussi depuis la fenêtre : **Configuration → Mots de passe**
pose ou change le mot de passe du logiciel, remplit le coffre (net-entreprises,
Urssaf), vérifie qu'un secret ouvre bien une session chez l'organisme (une
tentative, pas plus), l'oublie. La page ne voit jamais une valeur posée, et
modifier le coffre demande le mot de passe du logiciel s'il y en a un.


Le mot de passe du logiciel est demandé à **deux moments** : à l'ouverture de
la fenêtre PuppetPaie, et à **chaque acte qui engage** — dépôt réel d'une DSN
ou d'une DPAE, envoi des documents, demande de signature. Rien ne dispense de
le retaper pour un acte réel.

**La séance du coffre** (23/09/2026) est autre chose : quand la clé du poste
n'ouvre pas le coffre (sauvegarde restaurée, poste neuf), le mot de passe du
logiciel l'ouvre **une fois, pour 12 heures**, dans une petite fenêtre —
`paie coffre seance --fenetre`, ou d'elle-même à la première
commande qui en a besoin (relevé des retours, dépôt de test).
`coffre seance fermer` la referme. Elle ne vaut **jamais** preuve pour un acte
réel. (Une autre « séance », du 11 au 16/09/2026, était exigée au dépôt en
plus du mot de passe retapé ; elle a été retirée.)

**Le mot de passe est obligatoire.** La fenêtre le fait choisir à sa
première ouverture (deux fois, quatre caractères au moins) ; en ligne de
commande, `paie coffre ouverture`. Sans mot de passe posé, le
logiciel lit, calcule, produit, sauvegarde, relève les retours et dépose en
test — mais **rien ne part en réel** : ni DSN, ni DPAE, ni envoi, ni
signature. **Le mois ne se recopie plus** (21/09/2026) : il s'écrit en
grand, juste avant l'invite du mot de passe, et la fenêtre de confirmation
le redit sur son bouton. Il disait quel mois on croyait déposer, il ne
prouvait pas qu'un humain était là — n'importe quel programme sait recopier
un mois. Donné quand même (`--confirmer-mois`, `--confirmer`), il reste
vérifié contre le fichier : un script qui se trompe de mois est arrêté.

| | sans mot de passe posé | mot de passe posé |
|---|---|---|
| lire, produire, sauvegarder | oui | oui |
| relever les retours, dépôt de test | oui | oui |
| déposer en réel, envoyer, faire signer | **non** | oui, mot de passe retapé |

**Ce que ça a coûté, honnêtement.** Avant, aucun mot de passe n'était stocké :
c'était la garantie. Elle a été échangée contre l'autonomie, à la demande de
l'éditeur. Ce qui la remplace est écrit en tête de `paie/src/coffre.rs`, et deux
choses sont tenues par des tests : le secret ne sort jamais (ni par `etat()`,
ni en clair sur le disque), et les modules qui parlent au réseau ne peuvent pas
ouvrir le coffre eux-mêmes.

### La sauvegarde

```bash
paie sauvegarder          # les différences
paie sauvegarder --point  # où en est la dernière, sans réseau
paie sauvegarder --tout   # tout renvoyer, sans comparer
paie restaurer [DOSSIER]  # le retour : redescendre prive/ du Drive (vérifié, rien d'effacé) — LISEZ-MOI-DABORD, « Si je perds l'ordinateur »
```

Vers `PuppetPaie / sauvegarde / …` dans le Drive, en conservant l'arborescence.
Le premier passage monte tout (une quinzaine de minutes pour 16 Mo) ; les
suivants ne coûtent que la comparaison — une dizaine de secondes quand rien n'a
changé.

**Elle ne supprime jamais rien là-bas.** Un fichier effacé ici reste dans le
Drive : une sauvegarde qui répercute les suppressions n'est pas une sauvegarde,
c'est un miroir, et un miroir recopie fidèlement les accidents.

Le coffre (`coffre.bin`) monte (décidé par Léandre, 13 et 14/10/2026) : il est
scellé par le mot de passe du logiciel, qui l'ouvre sur un poste neuf après
`paie restaurer` ; sans ce mot de passe, il ne s'ouvre nulle part ailleurs.
Ne montent jamais : `google-jeton.bin` (le compte se relie de nouveau),
`.compteurs.verrou`, `.sauvegarde.json`, `.ouverture.json` (de l'état machine,
qui ment ailleurs), et le rebut (`*.tmp`).

## Archiver dans le Drive

```
paie archiver 2026-10
```

Dépose tout `sorties/2026-10/` dans `PuppetPaie / 2026-10` sur le Drive (le
sous-dossier `envoi/` est exclu : ce sont les mêmes documents redécoupés par
salarié, déjà partis par courriel).

Autres commandes : `paie fiche` (rappel des données de référence),
`paie interface` ; `paie --aide` les liste toutes.

**Pour un programme.** Toute commande accepte `--json` : même travail, mêmes
garde-fous, sortie lisible par une machine. Les montants y sont des chaînes
(`"201.32"`, jamais un flottant), les dates en `AAAA-MM-JJ`, et une erreur
métier reste une réponse — `{"ok": false, "erreur": {"type": "MoisPasPret",
…}}` — sur laquelle un appelant branche son comportement. C'est par là que
passe l'interface, et que passerait une API.

## La DPAE, séparément

La déclaration préalable à l'embauche est due **avant le premier jour
travaillé**, donc bien avant la paie. Elle se produit seule, sans montant
Urssaf et sans consommer de numéro de bulletin :

```bash
paie dpae paies/2026-09.json --heure 0800
```

Elle dit, pour chaque contrat, si le délai est tenu — la DPAE ne s'envoie que
dans les 8 jours qui précèdent l'embauche.

**Le dépôt.** Deux voies. À la main sur net-entreprises.fr, rubrique DPAE
(dépôt du `.txt`, ou saisie en ligne à partir du `_recapitulatif.txt`). Ou par
l'API de l'Urssaf, depuis la fenêtre : elle montre qui est déclaré, et n'envoie
rien tant que le mot de passe du logiciel n'est pas tapé. C'est le mot de passe
qui vaut confirmation. (Les raccourcis `Depot DPAE.cmd` et `Retours DSN.cmd`
d'avant le rangement du 09/10/2026 n'existent plus.)

En ligne de commande, même chose :

```bash
paie deposer-dpae sorties/2026-09/DPAE_Septembre2026.txt
```

Sans `--reel`, rien ne part. Avec, le mois des embauches s'affiche en grand
et le mot de passe du logiciel est demandé. (`--confirmer 2026-09` reste
accepté, et vérifié, pour une commande scriptée.)

Et il y a un **vrai dépôt de test**, qui parcourt toute la chaîne sans rien
déclarer — bouton « Dépôt de test » dans la fenêtre, ou :

```bash
paie deposer-dpae sorties/2026-09/DPAE_Septembre2026.txt --test
```

Le champ `C_VERSION` bascule de `120` à `TST` **en mémoire** au moment de
l'envoi ; le fichier sur le disque reste un fichier réel. L'API DPAE ne demande
aucune inscription, seulement un compte en ligne Urssaf. Détail : `../prive/journal/ETAT.md` §7 bis.

## La répétition avant l'irréversible

> **Aucun acte irréversible sans une répétition réussie du même contenu.**

Un dépôt réel — DSN ou DPAE — est refusé tant que **ce contenu-là** n'est pas
passé en dépôt de test. Pas ce fichier-là : ce contenu-là. La comparaison porte
sur les octets, parce qu'un `DSN_Septembre2026.dsn` testé lundi et regénéré
mardi porte le même nom sans plus rien avoir de commun. Un octet qui bouge rend
le test caduc, et le refus le dit.

Le fichier n'a pas à être modifié pour la répétition : c'est le moteur qui
bascule l'envoi en test, en mémoire, juste avant de l'envoyer. On teste donc le
fichier qu'on déposera, et le seul octet qui change entre les deux est celui
que le logiciel pose lui-même.

Trois refus, trois gestes différents — le message dit lequel : un test qui a
**échoué** se corrige (il cite l'anomalie), un test **périmé** se refait, une
absence de test se comble. Dans la page, le bouton du dépôt de test est dans
la boîte du refus.

**La sortie de secours existe, et elle est tracée.** Le service de test sera
indisponible un 5 du mois à 23 h, et bloquer une déclaration à l'échéance est
pire qu'un dépôt imparfait :

```bash
paie depot-dsn sorties/2026-09/DSN_Septembre2026.dsn --reel \
    --sans-repetition "le service de test répond 503 depuis ce matin"
```

Le motif est **écrit**, pas coché : un drapeau booléen posé par le code appelant
ne se relit pas six mois plus tard. Il part au journal des dépôts avec la ligne,
et la commande comme la page le redisent au moment où il sert.

Ce verrou se pose **en plus** de la preuve (le mot de passe du logiciel), jamais
à sa place : la preuve dit qu'un humain veut l'acte, la répétition dit que
l'acte passera.

## Les retours DSN

Après chaque dépôt, les organismes répondent : accusé, certificat de conformité,
bilans, puis les comptes-rendus métier. Celui de la **DGFiP, nature 94**, porte
le taux de prélèvement à la source de chaque salarié et son identifiant — les
rubriques `S21.G00.50.006/.007/.008`, qui ne se calculent pas.

```bash
paie retours
```

Ou, sans ligne de commande : la fenêtre les relève à l'ouverture.

Le mot de passe n'est plus demandé si le coffre le porte (`paie
coffre poser net-entreprises`) : relever les retours est une lecture, et c'est
ce qui permet à l'ouverture de le faire seule. Le champ reste, pour le cas où
aucun coffre n'est posé — et ce qui y est tapé passe devant le coffre.

Sans coffre, en ligne de commande, pour cette fenêtre seulement :

```bash
$env:NET_ENTREPRISES_MOTDEPASSE = (Read-Host "mot de passe net-entreprises")
```

La commande télécharge et archive tout dans `retours/`, puis **affiche** les
taux lus sans rien écrire. Il faut `--appliquer` pour toucher `config/pas.json`.
`paie pas` montre l'état courant et d'où il vient.

### Un taux se périme au bout de deux mois

Le taux transmis par la DGFiP vaut **jusqu'à la fin du deuxième mois suivant
celui de sa mise à disposition**. Pour un permanent déclaré tous les mois, la
règle ne se voit jamais : le compte-rendu du dépôt précédent repousse
l'échéance avant qu'elle tombe. **Pour un intermittent qui joue en janvier
puis en juillet, le taux de janvier est périmé depuis le 31 mars** — et c'est
alors la grille de l'article 204 H du CGI qui s'applique, pas le taux du
magasin.

`paie pas` date donc chaque taux (« en cours de validité, jusqu'au
31/10/2026 », ou « périmé ») et liste, en dessous, **ce qu'il faut demander à
la DGFiP**.

### Demander un taux : le signalement d'amorçage (nature 08)

TOPAze est fermé. Son remplaçant est le **signalement d'amorçage des données
variables** (SADV) : il part hors du rythme mensuel, et la DGFiP répond **sous
cinq jours ouvrés** par un CRM de nature 94, que `retours` ramasse comme les
autres. Le bilan d'identification des salariés (BIS) revient, lui,
immédiatement.

```bash
paie pas --demander            # un fichier par salarié à amorcer
paie pas --demander --test     # les mêmes, en fichiers d'essai
paie depot-dsn <fichier> --test   # un dépôt par individu (puis --reel)
```

**Un signalement, un individu, un fichier** (base de connaissances DSN, fiche
2117) : cinq salariés à amorcer, c'est cinq dépôts.

**Le déclencheur est « pas de taux valide », pas « nouveau contrat ».** Un taux
couvre deux mois ; un intermittent qui enchaîne les cachets est déjà couvert,
et le service n'a pas vocation à être appelé en masse. Un salarié dont la
demande est déjà partie ne repart pas non plus — `pas.json` retient « demandé
le X, réponse attendue avant le Y », compté en jours **ouvrés**, et le CRM qui
arrive solde la demande.

**Deux portes, et deux seulement**, pour le taux d'un salarié :

- **motif 07**, changement de tiers déclarant ou de logiciel déclaratif. Il se
  joue **une fois**, à la bascule d'un logiciel à l'autre, et demande les taux
  de tout le monde sans attendre une embauche. C'est le défaut de
  `--demander` ;
- **motif 01**, embauche effective. Il part **avec la DPAE**, dans le même
  geste et le même mot de passe : `paie dpae` produit les deux, et
  dit ce qu'il a demandé — ou pourquoi il n'a rien demandé.

Il n'existe **aucun motif « mon taux a expiré »**. Un intermittent déjà sous
contrat dont le taux périme sans nouvelle embauche n'a aucun levier : c'est la
grille de l'article 204 H, et rien d'autre. Un salarié **sans NIR** non plus —
la DGFiP répond à une identité, un numéro technique temporaire n'en est pas
une pour elle. C'est la loi, pas un manque du logiciel.

Un taux périmé est **rendu quand même** par `paie pas`, marqué : il
dit quel taux on avait et depuis quand il ne vaut plus. Mais **il ne part pas
dans la DSN** — c'est la grille qui reprend la main (ci-dessous). Une fiche
archivée reste hors de cette liste : son taux est périmé, et ça n'intéresse
personne tant qu'elle n'est pas réactivée.

### Ce que la DSN déclare quand il n'y a pas de taux

Première embauche, taux périmé, salarié sans NIR : la DGFiP n'a pas donné de
taux, et la loi ne laisse pas le choix. C'est la **grille de taux par défaut**
de l'article 204 H du CGI — type 13, et **pas d'identifiant**
(`S21.G00.50.008` n'est obligatoire qu'avec un taux transmis par la DGFiP,
type 01 : CCH-11, p. 244). Le moteur ne s'arrête donc plus : il
calcule, versement par versement.

Ce que le bloc 50 porte alors :

| rubrique | ce qu'elle dit |
|---|---|
| `50.002` | la rémunération nette fiscale, **entière** |
| `50.013` | le montant soumis : la RNF moins l'abattement contrats courts, jamais négatif |
| `50.006/.007` | le taux lu dans la grille, et son type |

L'abattement n'a pas de rubrique : il se lit dans l'écart entre 50.002 et
50.013 (cahier technique 2026.1, p. 245). `50.012` est réservée aux
abattements des assistants maternels et familiaux ; le moteur ne l'écrit plus
depuis la vague 1a (10/10/2026).

L'abattement (la moitié du SMIC net imposable mensuel) vaut pour un CDD dont
le terme n'excède pas deux mois ou dont le terme est imprécis, et seulement
pendant les **deux premiers mois** de l'embauche : le cachet d'un intermittent
en est toujours un, un CDI jamais. Un cachet ordinaire tombe donc à zéro —
l'abattement l'efface.

Ce n'est pas une bonne nouvelle pour le salarié : la grille prélève le plus
souvent **plus** que son taux réel, et il ne récupère la différence qu'à sa
déclaration de revenus l'année suivante. D'où l'appel de taux, ci-dessus.

**Rien ne part en silence, et rien ne bloque.** Trois endroits le disent :

- **à l'ouverture** (et à chaque veille) : une ligne par salarié sans taux
  valide, avec la date à laquelle son taux a cessé de l'être. C'est le moment
  utile — il reste trois semaines avant la DSN pour demander un taux et le
  recevoir ;
- **au récapitulatif d'avant dépôt**, en ligne de commande comme dans la
  page : « N versement(s) sur M partent sur la grille par défaut ». Le dépôt
  n'est pas refusé — la loi prévoit ce cas exactement, et refuser
  empêcherait la paie sans empêcher l'erreur ;
- **sur le bulletin**, qui imprime le prélèvement réel et le **net payé**.
  Le « net à payer avant impôt » ne change pas, et le montant net versé de la
  DSN non plus : le cahier technique 2026.1 (p. 244) dit que le prélèvement à
  la source ne s'en déduit pas.

Ce module **ne dépose rien** : il n'a aucune fonction d'envoi. Pour produire une
DSN en envoi de test (`S10.G00.00.005 = 01`, données non conservées) :
`paie mois paies/2026-09.json --test`.

## Un arrêt de travail

Un intermittent tombe malade pendant ses dates. Le contrat est signé, les
cachets sont prévus, et l'arrêt **suspend le contrat en cours**. Deux
déclarations le disent, et ce ne sont pas la même :

- le **signalement d'arrêt de travail** (DSN événementielle, nature 04), dû
  **dans les cinq jours** qui suivent la prise de connaissance par
  l'employeur. Il informe l'Assurance Maladie au plus près de l'événement,
  pour les indemnités journalières ;
- le **report dans la DSN mensuelle** : obligatoire (cahier technique 2026.1
  §2.5.2), et il se fait tout seul à la production du mois. Un arrêt qui dure
  est reporté par **chaque** DSN dont le mois croise l'arrêt.

```bash
paie arret                      ce qui est saisi, et ce qui attend
paie arret motifs               les motifs de la norme
paie arret contrats Celi --le 2026-10-07     ce qu'un arrêt suspendrait
paie arret saisir Celi --dernier-jour 2026-10-07 --fin 2026-10-20 \
                                  --motif 01 --subrogation non
paie arret signaler Celi --dernier-jour 2026-10-07
paie depot-dsn sorties/2026-10/SignalementArret_Celi_20261007.dsn --test
```

`saisir` **n'envoie rien** : l'arrêt est écrit dans le magasin des **imprévus**
(`imprevus/AAAA-MM.json`), au mois de son dernier jour travaillé. C'est un fait
saisi **une fois** : la production de chaque mois qu'il traverse en tire le bloc
60 de la DSN et, pour un permanent, la ligne qui retire le salaire des jours
d'absence (et le maintien de salaire saisi sur l'arrêt, `paie arret maintien
Celi --dernier-jour 2026-10-07 --mois 2026-10 --montant 300`) ; les cachets
d'un intermittent datés pendant l'arrêt sont nommés, jamais retirés d'office. `signaler` **écrit un fichier**, et c'est `depot-dsn` qui
le dépose — même point de dépôt, même mot de passe du logiciel, même journal
qu'une DSN mensuelle. Comme pour une DSN : **le dépôt de test d'abord**.

**L'arrêt prolongé, la reprise plus tôt que prévu** :

```bash
paie arret prolonger Celi --dernier-jour 2026-10-07 --fin 2026-11-05
paie arret prolonger Celi --dernier-jour 2026-10-07 --subrogation-fin 2027-01-07
paie arret reprise Celi --dernier-jour 2026-10-07 --le 2026-10-15
```

Une prolongation **ne fait partir aucun signalement** : elle change la fin
prévisionnelle, que la DSN mensuelle de chaque mois traversé porte
(`S21.G00.60.003` ; fiche net-entreprises 1983). Si l'arrêt est subrogé et finit
désormais après sa subrogation, le moteur le dit et demande la fin de subrogation
(il ne connaît pas le maximum de la convention) ; repoussée après le dépôt du
signalement d'arrêt, elle fait naître un **signalement d'arrêt rectificatif**
(annule et remplace, `paie arret signaler ... --remplace <identifiant>`). Une
reprise **avant** la fin prévue fait naître le signalement de reprise (05) ; à
la date prévue, rien à saisir : la DSN du mois la porte. Dans la fenêtre,
« Prolonger l'arrêt » et « Le salarié a repris plus tôt » ne paraissent que sur un
arrêt en cours.

Ce que le moteur refuse plutôt que de le deviner :

- un arrêt qui ne suspend **aucun contrat** saisi ce jour-là. Un signalement
  désigne le contrat par son numéro et sa date de début, tels que la DSN
  mensuelle les a déclarés — ils se lisent, ils ne s'inventent pas ;
- un arrêt **sans réponse sur la subrogation**. Elle est obligatoire dans un
  signalement (`S21.G00.60.004`), et répondre *oui* engage la compagnie à
  percevoir les indemnités journalières et à maintenir le salaire — avec ses
  dates, son IBAN et son BIC ;
- la **date de l'accident** absente pour un accident du travail, de trajet ou
  une maladie professionnelle — ou présente pour une maladie ordinaire, où
  elle est interdite ;
- une **reprise non anticipée** signalée : le signalement de reprise (nature
  05) ne s'émet que si le salarié reprend **avant** la fin prévue de l'arrêt.
  Sinon la DSN du mois suffit ;
- une DSN mensuelle qui **laisserait tomber** un arrêt dont le contrat n'est
  pas déclaré ce mois-là : le bloc `S21.G00.60` se déclare sous le contrat
  suspendu, et un arrêt qui disparaît en silence est un arrêt non déclaré.

**Ce que chaque arrêt fait naître, et pour quand** : la route `arrets` le rend
sous chaque arrêt (`paie/src/obligations.rs`, règles dans
`logiciel/donnees/obligations-base.json`, chacune avec son texte déposé) — le
signalement et son délai, la déclaration d'accident du travail (hors DSN,
quarante-huit heures), et pour un **deuil d'enfant** (motif 19), à la place du
signalement, l'**attestation de salaire** sur net-entreprises (hors DSN, dès
réception de l'avis d'arrêt) : la DSN ne couvre pas les congés pour deuil.
Le signalement d'un deuil s'écrit quand même si on le demande (la norme
l'admet), avec un avertissement. La compagnie dit le jour où elle a su
l'arrêt, et ce qu'elle a fait hors DSN :

```bash
paie arret connu Celi --dernier-jour 2026-10-07 --le 2026-10-08
paie arret declare Celi --dernier-jour 2026-10-07 --le 2026-10-08 --ou net-entreprises --reference 123
paie arret declare Celi --dernier-jour 2026-10-07 --le 2026-10-08 --ou net-entreprises-ecran --attestation
paie arret declare Celi --dernier-jour 2026-10-07 --retirer [--attestation]
```

Un signalement déjà parti ne se corrige pas en en renvoyant un autre : il faut
un **annule-et-remplace** (type 03), qui vise l'identifiant rendu par le point
de dépôt — celui du certificat de conformité — et porte un numéro d'ordre
supérieur. `arret signaler --remplace IDENTIFIANT`.

Dans la fenêtre, tout cela vit dans l'onglet **Paie**, colonne de droite,
sous « Arrêts de travail » : la liste de ceux qui croisent le mois affiché,
leur état (*à signaler* / *signalé*, et la DSN qui les a reportés), et les
boutons pour en saisir un, noter une reprise ou écrire le signalement.

Le signalement de **fin de contrat** (FCTU, nature 07) a sa commande,
`fin-contrat` (plus bas, avec les permanents). Un CDDU n'en a pas besoin —
sa fin part dans la DSN mensuelle, avec le circuit dérogatoire — sauf si le
salarié le demande : le moteur l'écrit alors, et le dit.

## Corriger un mois déjà déposé

Tant que l'échéance du 15 n'est pas passée, une DSN se corrige en refaisant
le mois et en redéposant : un **annule-et-remplace** intégral. Après, la
norme l'interdit (cahier technique 2026.1, §2.2.2.5) : *« les corrections
doivent être portées par une DSN mensuelle ultérieure »*, par les mêmes
blocs, **datés de la période d'origine**.

C'est ce que fait `corriger`. La correction est un objet à part
(`regularisations/AAAA-MM.json`, au mois d'**origine**) : **la saisie
déposée ne bouge jamais**, et la DSN du mois courant l'embarque
d'elle-même, en annulation-remplacement — l'ancienne paie en négatif, la
nouvelle en positif, aux dates du mois corrigé. Côté URSSAF, un bordereau
et un ordre de paiement **par mois régularisé** (§2.2.3.2) ; une correction
qui *baisse* un mois s'écrit en **reprise de crédit** (guide Urssaf, §6.4) :
bordereau du mois d'origine négatif, pas d'ordre pour lui, l'ordre du mois
courant diminué d'autant — et si le crédit dépasse le bordereau courant, le
moteur refuse et renvoie vers l'URSSAF.

La différence de net se paie — ou se retient — avec la paie du mois courant,
par un **bulletin rectificatif** du mois d'origine : un numéro neuf de la
suite, le numéro du bulletin qu'il rectifie (lu dans l'archive), le
prélèvement à la source sur la différence tel que la DSN le déclare, et le
net à verser ou à retenir. Il s'archive à côté de l'original
(`sorties/<origine>/Rectificatif_…pdf`) et part au salarié avec les pièces
du mois (`envoi/BulletinRectificatif_…pdf`).

```bash
paie corriger                      toutes les corrections, et leur état
paie corriger 2026-07              l'état du mois, et ses engagements déclarés
paie corriger 2026-07 --extraire Celi --debut 2026-07-03 > corrige.json
paie corriger 2026-07 --engagement corrige.json --salarie Celi --debut 2026-07-03
paie corriger 2026-07 --engagement corrige.json --salarie Celi --debut 2026-07-03 --motif "un cachet oublié"
paie corriger 2026-07 --annuler 1
```

Sans `--motif`, c'est un **aperçu** : avant, après, différence, et rien
n'est écrit. Avec, la correction est enregistrée et attend ; la prochaine
production du mois courant la porte et le dit, le dépôt réel la marque
portée. Une correction déjà portée ne s'annule plus — une nouvelle
correction peut la contredire.

**Écrire ou annuler une correction demande le mot de passe du logiciel** —
au terminal ici, par le popup dans la page. Rien ne part sur le moment,
mais ce qui s'écrit là repartira tout seul dans la prochaine DSN, sur un
mois déjà déclaré aux organismes : c'est un acte.

Dans la fenêtre, tout cela vit dans **Historique › Corriger un mois
déposé** : les mois qui se corrigent, ceux dont la DSN est en retard, les
corrections écrites et leur état. On y modifie les lignes d'un engagement
comme à la saisie — calendrier compris, sur le mois d'origine — avec
l'avant/après sous les yeux. Le numéro de contrat, lui, est celui qui a été
déclaré : il est relu dans la DSN déposée quand la saisie ne le porte pas,
et sans lui le moteur refuse plutôt que d'en inventer un.

Un mois **fermé** (échéance passée, DSN jamais partie) se rouvre depuis la
même page, ou depuis l'onglet Paie : le bouton « Ce mois est fermé — le
rouvrir » ouvre l'avertissement des pénalités, et c'est la réponse qui
rouvre. En ligne de commande, c'est `--forcer`.

Ce qui se corrige : les **lignes** d'un engagement déclaré (un cachet
oublié, un montant faux, des heures). Pas le contrat lui-même : son numéro,
ses bornes et son spectacle identifient la déclaration d'origine. Un
engagement oublié en entier n'est pas une correction, c'est un contrat en
retard — avec sa DPAE en retard.

## Le fichier de saisie

```json
{
  "mois": "2026-10",
  "taux_cachet": 130.0,
  "date_paiement": "2026-10-31",
  "engagements": [
    { "salarie": "Celi", "spectacle": "aaah zerbinette",
      "lignes": [
        { "rubrique": "cachet", "quantite": 1, "taux": 130,
          "lieu": "Théâtre de la Ville", "dates": ["2026-10-12"] },
        { "rubrique": "cachet", "quantite": 4, "taux": 130,
          "lieu": "Théâtre 71",
          "dates": ["2026-10-14", "2026-10-15", "2026-10-17", "2026-10-18"] }
      ] }
  ]
}
```

**Une ligne de paie porte ses jours et son lieu** : « 4 cachets à 130 € au
Théâtre 71 le 14, 15, 17, 18 ». Deux villes, deux lignes — et c'est **une
seule paie** : même spectacle, donc même numéro d'objet, donc un contrat, un
bulletin. Les jours de l'engagement sont la réunion de ceux de ses
lignes. Le contrat groupe les dates par lieu et les nomme toutes ; le lieu ne
se déclare nulle part ailleurs (ni DSN, ni AEM, ni DPAE).

**Une seule écriture, depuis le 13/09/2026.** Un engagement n'a que des
`lignes` ; `dates`, `cachets`, `heures` ou `taux_horaire` posés sur
l'engagement lui-même sont **refusés**. Un contrat mixte se saisit
avec une ligne `cachet` et une ligne `heure`, chacune à son taux.

Les noms sont tolérants : « Celi », « celimene », « Mairn » avec la faute de
frappe, « zerbinette » — tout est résolu vers la bonne fiche. Un nom ou un
spectacle vraiment inconnu provoque une erreur explicite plutôt qu'une paie
fausse.

Par engagement on peut préciser `debut` / `fin` (si le contrat déborde des
dates jouées), `numero`, `lieu` (celui des lignes qui n'en portent pas),
`date_paiement`, et le **métier** de ce contrat :

```json
{ "salarie": "Celi", "spectacle": "aaah zerbinette", "metier": "REG120",
  "cadre": true, "lignes": [ … ] }
```

`metier` est un code emploi France Travail. Le catalogue
(`paie emplois`, 129 métiers du spectacle vivant, `--tous` pour
les 1 236 codes de la nomenclature) le traduit en six champs déclarés —
libellé officiel, PCS-ESE, statut, statut conventionnel, cadre — que
l'engagement peut écrire lui-même par-dessus. Sans `metier`, c'est l'emploi
de la fiche du salarié. Un code inconnu est refusé.

**Les minima conventionnels** (CCN 3090 et CCNEAC 1285, dans
`logiciel/donnees/minima-conventionnels.json`) sont confrontés à chaque ligne :
cachet et répétition d'un artiste, heure, journée ou semaine d'un technicien.
Une ligne sous le plancher donne un **avertissement, jamais un refus**, sous
la paie dans l'interface et dans `avertissements` en `--json`. Le plancher
dépend de l'annexe de la convention (`annexe_convention` sur la fiche
entreprise : 1 théâtre, 4 tournée…) ; sans elle, ou quand une grille
distingue les jauges, le moteur retient **le plus bas** — il se tait plutôt
que de crier à tort.

**Et il dit ce qu'il n'a comparé à rien** (21/09/2026). Son silence voulait
dire deux choses à la fois — « au-dessus du minimum » et « il n'y a pas de
minimum ici » —, si bien qu'on pouvait lire six paies sans un mot et les
croire vérifiées. Une ligne de travail sans plancher donne donc une phrase
qui dit **pourquoi** : un métier que la classification de la CCNEAC ne
nomme pas, un métier sans ligne de minima, un salaire mensuel sous la 3090
(comparé à rien). Non bloquant comme le reste, et **une
phrase par raison**, pas par ligne.

La commande `minima` du Python — ce qui est comparé, ce qui ne l'est pas, et
ce qui est relevé sans être lu — **n'est pas portée** en Rust ; les
avertissements, eux, sortent avec chaque paie (`paie/src/minima.rs`). Elle
était la réponse à « est-ce que mes paies sont vérifiées ? ». Elle distinguait une grille **relevée** d'une grille **appliquée** : les
cachets d'artistes de la CCNEAC sont relevés et servent, et depuis le
23/09/2026 ses emplois autres qu'artistiques aussi — groupe de l'article XI.3
(`classification_1285` du catalogue), échelon de la fiche (`echelon_ccneac`,
le premier sans lui), à l'heure comme au mois ; la NAO 2026, non étendue, dort dans le fichier sans
être lue, et la commande le dit plutôt que de le laisser croire.

**Les sources** de ces données — notices France Travail, nomenclature des
emplois, PCS-ESE de l'Insee, avenants salaires sur Légifrance, NAO — sont
décrites et datées dans `logiciel/donnees/sources.json`, leurs copies rangées
dans `atelier/reference/sources/`. **`paie sources`** les relit (`paie/src/sources.rs`,
porté en Rust au lot S1, 13/10/2026) : pour chacune, ce qu'elle contient, sa
version, la date du relevé et son âge, et si la copie colle au condensat du
manifeste (conforme, différente, absente). **`--verifier`** retourne voir en
ligne : une source qui se télécharge est reprise et comparée — `inchangee`,
`changee` (à RELIRE, avec les fichiers de données qu'elle nourrit),
`injoignable` (pas de réseau, ou le lien a changé) ; une source qui ne se
revérifie pas en ligne est `manuelle`, avec la page à ouvrir. **`--relever`**
(qui emporte `--verifier`) pose la nouvelle version d'une source changée **à
côté** de l'ancienne, suffixée du jour, jamais par-dessus. `--json` rend la
même chose pour un programme.

C'est un **outil de l'éditeur**, lancé avant chaque publication (`atelier/CLAUDE.md`,
« Publier une version ») : la commande est dans le `paie` livré, comme
`paie maj --publier`, et refusée de la même façon ailleurs que sur son poste
(le réglage de l'installation, `"editeur": true` ; jamais depuis une copie). Elle
ne figure pas dans l'aide. Une installation ne fait que recevoir la mise à jour
signée ; la publication ne vérifie pas d'elle-même que les sources l'ont été.

**Les tables Unicode du moteur** (`socle/src/tables_nfkc.rs`, `paie/src/nfkd_ascii.rs`,
`tables_retours.rs`, `tables_journal.rs` : la normalisation NFKC/NFKD, les
chiffres et lettres de `str.isdigit`/`str.isalnum`/`\d`, cp437 et cp1252, les
encodages qu'un XML de retour peut déclarer, les caractères d'un nom XML)
s'écrivent depuis les textes déposés (`atelier/reference/sources/unicode/`,
`python/`, `w3c/`, chacun avec son `LISEZ-MOI.md`) par **`paie generer-tables
atelier/reference/sources atelier/sources`** (`paie/src/generer_tables.rs`,
15/10/2026 ; avant, quatre scripts Python les tiraient de l'Unicode de
l'interpréteur). Le test `tables_generees` vérifie que les tables commitées sont
celles que les textes donnent. Monter de version d'Unicode : déposer les
nouveaux fichiers, changer les constantes `UNICODE_*` du module, relancer, lire
le `git diff` (`unicode/LISEZ-MOI.md`).

Légifrance refuse les robots, mais la DILA publie chaque soir en open data
ce qui a bougé dans les conventions collectives (base KALI, une archive de
quelques centaines de Ko, sans compte). Pour les sources qui portent un bloc
`kali` (CCN 1285 et 3090), `--verifier` lit ces livraisons depuis le relevé
(`paie/src/kali.rs`) et dit si la convention a bougé : un texte dont le titre
parle de salaires, ou le texte cité lui-même, met la source « à relire » avec
son titre, sa date et son état (étendu ou non) ; les autres mouvements sont
montrés en second. `--relever` garde le XML du texte touché
(`atelier/reference/sources/kali/`), qui contient le texte intégral. La lecture
est résumée dans `atelier/reference/sources/veille-kali.json` (le même fichier
que du temps du Python : la veille reprend où elle s'était arrêtée) ; la
première fois, elle remonte l'index (environ un an) jusqu'à la dernière fiche
de chaque convention, pour savoir ensuite rattacher un texte à la sienne : une
minute, une fois. L'ouverture hebdomadaire du Python la refaisait toute
seule ; en Rust, l'ouverture accepte l'option et ne vérifie rien
(`paie/src/ouverture.rs`) : c'est l'éditeur qui la lance.

Quand une source a changé, le référentiel qu'elle nourrit se **relève** par
**`paie extraire`** (`paie/src/extraire/`, porté en Rust le 15/10/2026 depuis
les `atelier/outils/extraire_*.py`) : `naf` (les deux classeurs INSEE →
`naf-base.json`), `usages` et `formats` (le classeur de net-entreprises ou le
cahier technique en PDF → `usages-dsn-*.json`, `formats-dsn-*.json`), `urssaf`
(la Table des Urssaf, la liste des sites, le COG 2015 →
`urssaf-dpae-base.json`). Les sources, puis la sortie, se nomment toujours ;
une sortie qui existe ne se remplace qu'avec `--remplacer`, et rien ne s'écrit
avant que le relevé soit fait. Le `_source` du JSON nomme la commande, les
fichiers déposés et le jour du relevé. Par exemple :
`paie extraire naf <structure.xlsx> <correspondances.xlsx> logiciel/donnees/naf-base.json --remplacer`
(les commandes de chaque table sont dans `atelier/reference/norme-dsn/LISEZMOI.md`
et `atelier/reference/sources/insee-naf-2025/LISEZ-MOI.md`). Puis
`git diff logiciel/donnees/` dit ce qui a bougé, et
`tests/fichiers/extraction.rs` vérifie que chaque table livrée est bien ce que
ses textes déposés donnent.

**Les textes eux-mêmes**, articles de loi et de convention, se relèvent dans les
bases LEGI et KALI de la DILA et se gardent dans deux entrepôts du dépôt
(`atelier/reference/sources/legi-entrepot/` et `kali-entrepot/` : tous les
articles des textes suivis, et la liste des livraisons déjà appliquées ;
`paie/src/entrepot_dila.rs`). Deux commandes de l'atelier, qui prennent le
dossier des sources :

```
paie releve-codes moissonner atelier/reference/sources     # les livraisons LEGI nouvelles, puis releve.json et xml/ de chaque texte
paie releve-codes rapport atelier/reference/sources [code]  # articles-<date>.md, le document lisible
paie releve-codes comparer atelier/reference/sources       # chaque article changé, relu contre sa version d'avant (archive globale, 1,1 Go en flux)
paie releve-conventions mettre-a-jour atelier/reference/sources
paie releve-conventions article atelier/reference/sources 3090 7.6
paie releve-conventions chercher atelier/reference/sources 3090 licenciement
```

Les textes suivis sont dans `paie/src/releve_codes/textes.rs` (un article
ajouté y est déjà dans l'entrepôt : `moissonner` l'écrit sans rien télécharger)
et, pour les conventions, ceux que cite `sources.json`. `--reconstruire` refait
défiler l'archive globale (1,1 Go pour LEGI, 180 Mo pour KALI) : on ne le
lance pas sans raison.

### La nature du contrat, et les salariés permanents

*Chantier en cours depuis le 18/09/2026. Ce qui suit marche ; ce qui manque
est dit à la fin.*

Un engagement peut dire sa **nature de contrat**. Sans elle, c'est un CDD
d'usage du spectacle — l'intermittent, ce que sont les 146 contrats de
l'historique, et c'est pour cela qu'aucune saisie existante n'a besoin d'être
touchée.

```json
{ "salarie": "Camille", "nature": "cdi",
  "debut": "2026-01-05", "numero": "AD1.26.1",
  "quotite_contrat": 151.67,
  "lignes": [ { "rubrique": "mois", "quantite": 1,
                "taux": 2200.00, "heures": 151.67 } ] }
```

Un contrat permanent (`cdi`, `cdd`) n'a **ni dates jouées, ni spectacle** :

- il a une **durée du travail** (`quotite_contrat`, en heures par mois —
  151,67 pour un temps plein à 35 h), et non des jours cochés ;
- **il ne se rattache à aucun spectacle** : la norme DSN interdit le numéro
  d'objet hors des CDD d'usage (contrôle CCH-11). Un contrat rattaché à un
  spectacle est un contrat d'usage, et se saisit comme tel ;
- un **CDI n'a pas de fin** tant qu'on ne lui en donne pas une, et rien ne le
  déclare terminé ;
- un **CDD ordinaire** doit dire son `motif_recours` (remplacement,
  accroissement temporaire d'activité, emploi saisonnier…) : c'est lui qui
  rend le contrat licite, et la DSN l'exige ;
- quand un contrat permanent se termine, il dit son `motif_rupture`.
  Un CDD a un défaut (fin de contrat à durée déterminée) ; **un CDI n'en a
  aucun** — démission, licenciement, rupture conventionnelle et départ à la
  retraite n'ouvrent pas les mêmes droits, et le moteur refuse de choisir.

Comme un CDDU de tournée, un contrat permanent est **saisi dans chacun de ses
mois**, avec les mêmes `debut` et `numero`.

**Une saisie de permanent dit des FAITS, pas des lignes.** Depuis le
18/09/2026, `charger` en déduit les lignes du bulletin — salaire de base et
son prorata, congés pris et l'indemnité qui les couvre, primes, solde de
tout compte le dernier mois. Un arrêt de travail ou une suspension (congé
parental, sabbatique, sans solde…) n'est **pas** un fait du mois : c'est un
**imprévu**, saisi une fois sur le salarié (`paie arret saisir`, `paie
suspension saisir`), dont le moteur tire la ligne de chaque mois qu'il traverse
(une saisie qui porte encore des `absences` est refusée) :

```json
{ "salarie": "Camille", "nature": "cdi", "debut": "2026-01-05",
  "numero": "AD1.26.1", "salaire_mensuel": 2200.00, "quotite_contrat": 151.67,
  "conges_pris": [ { "debut": "2026-07-13", "fin": "2026-07-17" } ],
  "primes":      [ { "libelle": "Prime de tournée", "montant": 200 } ] }
```

Et quand il se termine, trois faits de plus, qui ne servent qu'au
**signalement de fin de contrat** (la DSN mensuelle les interdit) :

```json
  "fin": "2026-10-31", "motif_rupture": "059",
  "notification_rupture": "2026-09-30",
  "preavis": { "type": "01", "debut": "2026-10-01", "fin": "2026-10-31" },
  "transaction": "02"
```

```bash
paie fin-contrat liste 2026-10            ce qui se termine, et ce qui manque
paie fin-contrat signaler 2026-10 Camille --test
paie fin-contrat documents 2026-10 Camille   certificat + reçu
```

`liste` **dit ce qui manque** plutôt que de deviner : le moteur ne fabrique ni
une date de notification, ni un préavis. Pour le préavis il cite la loi sans
l'appliquer — la durée de L1234-1 quand elle existe, et sous six mois
d'ancienneté le fait qu'elle renvoie à la convention. Le signalement est dû
dans les **cinq jours ouvrés** qui suivent la fin, et l'ouverture le rappelle.

**Deux faits à poser une fois pour toutes** :

- sur la **fiche du salarié**, sa complémentaire santé — elle est obligatoire
  (L911-7) et son montant dépend de SON contrat, pas d'un barème :
  `"sante_svp": { "niveau": "2", "conjoint": true, "enfants": 2 }` ;
- sur **`config/entreprise.json`**, les jours fériés que la compagnie
  **chôme** : `"jours_feries_chomes": "tous"`, ou la liste des noms. La loi
  n'en impose qu'un, le 1er mai ; les dix autres relèvent de la convention ou
  de l'usage. Tant que rien n'est dit, le moteur ne retire que le 1er mai du
  décompte des congés — et il le dit à chaque fois.

**Tout cela se saisit aussi à l'écran** depuis le 19/09/2026. Un engagement
porte une case **Nature**, et c'est elle qui change la forme du bloc : un
permanent perd le spectacle et les lignes de paie, et gagne son salaire
mensuel, sa quotité, et trois listes de faits — congés pris, absences,
primes. Sa fin, quand elle se signale, se saisit sous « Contrat » (motif de
la rupture, notification, préavis, transaction), et une boîte **Fin de
contrat**, à côté des arrêts de travail, liste ce qui se termine ce mois-ci,
**dit ce qui manque** plutôt que de proposer un bouton qui refuserait, écrit
le signalement et enchaîne sur le dépôt. Les deux papiers dus au salarié
s'écrivent du même endroit. La complémentaire santé, elle, se choisit sur la
fiche du salarié : un niveau, un conjoint, des enfants.

La **prévoyance des permanents**, qui manquait au début du chantier, est
relevée depuis le 19/09/2026 et livrée avec le logiciel : la convention
collective la fixe (`logiciel/donnees/prevoyance-conventionnelle-base.json`,
`docs/ARCHITECTURE.md`, « Le taux d'une branche n'est pas une donnée de
compagnie »). Ce qui reste propre à chaque compagnie, et la fait refuser
tant qu'il manque, ce sont ses références d'adhésion (`config/organismes.json`).

### Un contrat sur plusieurs mois

Un CDDU signé une fois pour une tournée d'octobre à décembre est **un seul
contrat, saisi dans chacun de ses mois**. Chaque fichier de mois reste
autonome : il porte les cachets joués ce mois-là, et le contrat entier par
ses bornes — `debut`, `fin`, et, dès le deuxième mois, le `numero` que le
premier a déclaré :

```json
paies/2026-10.json   { "salarie": "Celi", "spectacle": "aaah zerbinette",
                       "debut": "2026-10-12", "fin": "2026-12-20",
                       "lignes": [ …les cachets d'octobre… ] }
paies/2026-11.json   { "salarie": "Celi", "spectacle": "aaah zerbinette",
                       "debut": "2026-10-12", "fin": "2026-12-20",
                       "numero": "FA1.26.14",
                       "lignes": [ …les cachets de novembre… ] }
```

Le rôle du mois se **déduit** des dates, il ne se saisit pas :

| | contrat imprimé, DPAE | bulletin, DSN | certificat Congés Spectacles |
|---|---|---|---|
| premier mois (`debut` dans le mois) | oui | oui | non |
| mois du milieu | non | oui | non |
| dernier mois (`fin` dans le mois) | non | oui, avec la fin de contrat (bloc 62) | oui, **pour le contrat entier** |

La DSN déclare le même bloc contrat chaque mois (début, numéro, fin
prévisionnelle : ce sont ses identifiants), la période de paie bornée au
mois, et la fin de contrat le dernier mois seulement. Le certificat Congés
Spectacles du dernier mois additionne les cachets et le brut de tous les
mois du contrat, lus dans les fichiers précédents — le seul regard en
arrière du moteur.

**Le numéro se fige à la production** : `produire_mois` réécrit `numero`
dans chaque engagement du fichier produit et pose `numeros_figes: true`.
Le mois suivant le recopie (`paie nouveau 2026-11 --reprendre`
écrit les contrats en cours dans le fichier neuf, bornes et numéro, à
compléter avec les jours ; dans l'interface, « + reprendre un contrat en
cours »). Un mois du milieu dont le premier mois n'a pas été produit est
refusé : sans numéro connu, le moteur en tirerait un autre.

**Le planning.** Une ligne de paie peut porter, sous `planning`, les jours
des mois *suivants* cochés d'avance — dans l'interface, le calendrier d'une
ligne a une flèche vers le mois suivant, et le premier jour coché là-bas
demande « vous êtes en train de créer un contrat sur plusieurs mois.
Valider ? » :

```json
{ "rubrique": "cachet", "quantite": 3, "taux": 130, "lieu": "Avallon",
  "dates": ["2026-05-12", "2026-05-24", "2026-05-26"],
  "planning": ["2026-06-05", "2026-06-06"] }
```

**L'historique dans le calendrier.** La flèche vers le mois *précédent*
remonte aussi loin qu'on veut, en lecture seule : les mois passés montrent
les contrats déclarés des salariés cochés — une plage grisée quand on ne
connaît que début, fin et nombre de cachets (l'historique importé des AEM,
avant août 2026), les jours joués en gris plein quand le mois a été produit
ici — et la liste des contrats du mois sous la grille, avec le titre du
spectacle. C'est ce que la compagnie a déclaré, pas la position du salarié :
ses autres employeurs n'y sont pas. Rien ne s'y coche.

Le planning n'est pas payé ce mois-ci ; il sert à trois choses : `fin` se
cale sur son dernier jour sans qu'on l'écrive, le contrat imprimé liste
toutes les dates de la tournée (et dit que sa rémunération chiffre le
mois), et le mois venu, l'écran — qui reprend d'office chaque contrat en cours
(ou `nouveau --reprendre`) — retrouve la ligne déjà datée — même rubrique, même taux,
même lieu, quantité = nombre de jours pour un cachet. Un jour de planning
dans le mois est refusé (c'est un jour joué), de même qu'un planning
au-delà d'une `fin` écrite. Sans planning, le contrat imprimé ne connaît
que le mois et renvoie au planning pour la suite — le relire avant
signature.

## Ce qu'il faut vérifier à chaque mois

1. **Le montant Urssaf** du bloc `S21.G00.20` de la DSN. C'est un ordre de
   télépaiement, et il se **calcule** depuis le 29/09/2026 : c'est la somme des
   lignes du bordereau (bloc 23), chacune valorisée à la table des CTP de
   l'Urssaf et arrondie à l'euro (`paie/src/bordereau.rs`) — 74 des 75
   DSN de CoolPaie retrouvées au centime. Rien à recopier :

   ```
   paie mois paies/2026-09.json
   ```

   La commande **ferme le mois** : elle refuse (`MoisPasPret`) tant que la DPAE
   et le contrat de chaque embauche du mois ne sont pas partis.
2. **La DSN en dépôt de test** (`depot-dsn --test`) avant tout dépôt réel.
   C'est le validateur qui fait foi : la même brique de contrôle que DSN-Val,
   à jour, côté plate-forme (voir `atelier/reference/norme-dsn/LISEZMOI.md`).
   Depuis le 20/09/2026 ce n'est plus une discipline mais une **règle du
   moteur** : un dépôt réel dont le contenu exact n'est pas passé en test est
   refusé, pour la DSN comme pour la DPAE — voir « La répétition avant
   l'irréversible ».
3. **Le numéro d'objet du spectacle** : France Travail en délivre un nouveau
   chaque année (`216Z…` = 2021 … `266Z…` = 2026) — voir « Chaque janvier ».
4. **Les compteurs** de `config/compteurs.json` (numéros de bulletin, de contrat
   et d'attestation AEM) au premier mois d'utilisation réelle : ils ont été
   estimés à partir des dernières sorties CoolPaie, pas lus dans sa base.
   Ils ne sont écrits qu'une fois tous les fichiers du mois produits : un essai,
   un rejeu ou une commande interrompue ne consomme aucun numéro.

## À l'ouverture : ce que le logiciel rappelle

Le mois fini, la fenêtre s'ouvre sur le suivant — et dit ce qui attend. Le
2 octobre : *« Il est temps d'envoyer la DSN de septembre. Avez-vous bien
fini les paies de septembre ? »*, avec l'état exact du mois (tant de paies,
tant de validées, écrit ou pas) et un bouton **Vérifier et envoyer** qui
ouvre le mois, l'écrit s'il ne l'est pas, puis enchaîne sur le récapitulatif
de dépôt et son mot de passe. Rien ne part sans ce dernier geste. Passé le
15, le rappel dit « en retard ». « Plus tard » ferme la boîte, qui reviendra
à l'ouverture suivante tant que le journal des dépôts n'a pas une DSN réelle
passée pour ce mois — qu'elle vienne de ce logiciel ou d'un autre (celle de
CoolPaie pendant le double run est relevée des retours, donc connue).

```
paie rappels                    la même liste, en ligne de commande
paie rappels --le 2026-10-02    ... à une autre date
```

### Les conseils « Le saviez-vous ? »

Sur option (fiche Entreprise, bloc « Vos salariés », « Afficher des conseils », décochée par défaut). Cochée : un conseil
du jour à l'accueil (« un autre » en montre un suivant), un conseil d'une phrase sous la carte d'un engagement quand la
saisie appelle quelque chose d'utile à savoir (les heures qui comptent pour les 507 heures, un mois au-delà de 28 cachets,
un cachet de répétition que la convention prévoit…), son détail et sa référence derrière « pourquoi ? », et, dans la
fiche d'un salarié, ce que vos contrats ont fait compter pour ses droits. Chaque conseil cite son texte ; la banque est
`logiciel/donnees/conseils.json` (`atelier/docs/ARCHITECTURE.md`, « Les conseils »).

```
echo '{"route":"conseil-du-jour","charge":{}}' | paie route --racine <compagnie>
echo '{"route":"conseils-salarie","charge":{"salarie":"<CLE>"}}' | paie route --racine <compagnie>
```

### La DPAE préparée d'avance, et son filet de sécurité

Dès qu'un contrat part — envoyé à la signature, ou écrit avec le mois pour
être imprimé —, sa DPAE s'écrit et part **en test**, sans rien demander :
c'est le même fichier que le dépôt réel reprendra, déjà répété. Le test ne
déclare rien. Il se fait à l'ouverture, à chaque passage de la veille, et
juste après l'envoi des contrats ; une panne devient une ligne d'incident,
et le passage suivant réessaie.

Huit jours avant l'embauche, la boîte des rappels dit **« DPAE à envoyer —
embauche du … »**, avec le bouton **Envoyer la DPAE**, qui ouvre le dépôt de
ce fichier (le réel, avec le mot de passe du logiciel). Le rappel se tait
dès que la DPAE réelle est au journal, et dit « en retard » une fois
l'embauche commencée.

**Le filet** (Configuration › Mots de passe, une option à cocher) : le
logiciel confie au serveur de PuppetPaie un réveil pour la veille de chaque
embauche dont le contrat est parti, et le désarme dès que la DPAE part.
Un réveil qui n'a pas été désarmé sonne : **un** courriel à l'adresse
choisie. Si la DPAE est partie, personne ne reçoit rien. Le serveur ne
reçoit ni nom, ni salarié, ni contrat : l'adresse (confirmée une fois par un
lien), un code tiré au hasard et le jour de l'embauche. Son code et son
installation sont dans le dépôt du site de l'éditeur, sous `infra/filet/` (depuis le
13/10/2026 ; il tourne sur le VPS du site). Le poste s'identifie auprès de lui
par une clé que le serveur lui donne (`config/diagnostic.json` : `poste`,
`cle`, `serveur`), demandée au premier envoi ; plus de secret partagé.

```
paie filet                       l'option, les réveils armés
paie filet activer ADRESSE       s'inscrire (un lien de confirmation part)
paie filet preparer              un passage : tester, armer, désarmer
paie filet suivies               les embauches sans trace de DPAE
```

## Chaque janvier

1. **Le taux AT/MP**, et lui seul, si vous êtes une compagnie : l'Assurance
   Maladie le notifie courant janvier (compte AT/MP sur net-entreprises), il
   est propre à votre établissement, et il s'écrit une fois dans
   `config/baremes/<année>.json` :

   ```json
   {"annee": 2027, "socle": 2027,
    "taux_atmp": {"valeur": 1.50, "code_risque": "923AD"}}
   ```

   Tout le reste — SMIC, plafond de Sécurité sociale, taux Urssaf,
   Agirc-Arrco, chômage, CSG, taux de la branche — est **livré avec le
   logiciel** (`logiciel/donnees/baremes/<année>.json`, le même pour toutes les
   compagnies) et arrive par la mise à jour. Trois valeurs du socle supposent
   une entreprise de **moins de onze salariés** (FNAL 0,10 %, formation
   0,55 %, `tdelta` 0,3781) : au-delà, on les corrige dans son propre
   fichier, en nommant la rubrique par son `id`.

   **L'effectif ne se saisit pas** (23/09/2026) : l'Urssaf le calcule et le
   renvoie dans les retours DSN (nature 118, l'été qui suit l'année) ; le
   relevé l'écrit seul dans `config/effectifs.json`, et un seuil n'est
   franchi qu'après cinq années consécutives au-dessus (art. L. 130-1 CSS).
   Une **compagnie neuve** renseigne seulement la date de sa **première
   embauche** (Configuration › La compagnie) : les années d'avant n'ont pas
   de salarié, et aucun seuil ne peut être franchi pendant cinq ans. Une
   compagnie plus ancienne dont les vieux retours ne sont plus servis
   déclare dans ce fichier, une fois, qu'aucun seuil n'a été atteint jusqu'à
   une année :
   `"jamais_au_dessus_de": {"seuil": 11, "jusqu_a": 2024, "source": "..."}`.
   La notification du taux AT/MP arrive elle aussi dans les retours
   (nature 34) : le relevé l'archive et l'ouverture le rappelle.

   **Si vous éditez le socle** (c'est notre cas, pas celui d'une compagnie) :
   partir du précédent — SMIC horaire, taux de cotisations, **plafond de
   Sécurité sociale** (`plafond_securite_sociale`, table des paramètres du
   BOSS), taux de la déduction forfaitaire, paramètres de la réduction
   générale. Un taux qui change en cours d'année s'écrit
   daté : `[{"depuis": "2027-01-01", "valeur": …}, {"depuis": "2027-07-01",
   "valeur": …}]`.
2. **Les numéros d'objet** : France Travail Spectacle en délivre un par
   spectacle et par année (`266Z…` en 2026, `276Z…` en 2027). L'ouverture le
   rappelle au premier trimestre, avec la liste des spectacles à mettre à
   jour — Configuration › Spectacles, date d'effet au 1er janvier.
3. **L'export compta** : le journal des écritures de paie de l'année, une
   écriture par mois, équilibrée au centime — salaires bruts et charges
   patronales au débit, organismes (part patronale + salariale), net à payer
   et prélèvement à la source au crédit. Tout part des bulletins recalculés,
   pas de la DSN. Un CSV (`;`, virgule décimale, colonnes journal / date /
   compte / libellé / débit / crédit / analytique) que tout logiciel de
   comptabilité importe ; un format propre à un logiciel s'ajoutera le jour
   où une compagnie le demande. Le plan comptable est une donnée
   (`logiciel/donnees/plan-comptable-base.json`, à faire valider par le comptable) ;
   une compagnie pose le sien dans `config/plan-comptable.json`, clé par clé.

   ```
   paie export-compta 2026            sorties/exercices/Compta_2026.csv
   paie export-compta 2026 --du 1 --au 6
   ```
4. **La clôture**, après la DSN de décembre : elle fige l'année (ses saisies
   et ses sorties ne se modifient plus — rouvrir reste possible) et réunit
   tout ce qu'il faut garder dans `sorties/exercices/Exercice_<année>.zip` :
   saisies, bulletins, contrats signés, DPAE, DSN, certificats, courriels,
   retours des organismes, journal des dépôts, fiches du jour, export compta.
   Refusée tant qu'un mois avec des paies n'a pas de DSN réelle acceptée au
   journal (`--forcer` passe outre, et c'est noté).

   ```
   paie cloture 2026
   paie cloture 2026 --rouvrir
   paie cloture                       l'état de chaque exercice
   ```

L'ouverture propose les trois derniers d'elle-même en janvier ; le reste de
l'année, on les retrouve dans Historique › L'exercice.

## Les exports pour le comptable

Un verbe unique, `exporter`, et un état pour commencer : le **livre de
paie**. Une ligne par bulletin ; en colonnes le brut, les cotisations
salariales et patronales, le net imposable, le net à payer, le prélèvement à
la source et le coût employeur. Les colonnes sont **fixes** — une colonne par
rubrique rencontrée ferait un tableau différent chaque mois, qu'on ne pourrait
pas rapprocher du précédent.

Trois périodes, et une seule à la fois : un mois, une année, toutes les
années. Trois regroupements, et ce sont trois usages : **par salarié** (ce
qu'on envoie au comptable), **par mois** (ce qu'on rapproche de la banque),
**par spectacle** (ce qui justifie une subvention). Chaque groupe porte son
sous-total, et le pied tombe exactement sur la somme des bulletins.

Cela vit à deux endroits, et ce sont les mêmes appels : **l'onglet Exports**
de l'interface — quoi, quand, comment, et un aperçu avant d'écrire — et la
ligne de commande.

```
paie exporter                               ce qu'on sait sortir
paie exporter livre-de-paie --annee 2026
paie exporter livre-de-paie --mois 2026-09 --par mois
paie exporter livre-de-paie --toutes-les-annees --format xlsx
paie exporter livre-de-paie --annee 2026 --format pdf
paie exporter livre-de-paie --annee 2026 --apercu
paie exporter livre-de-paie --annee 2026 --coordonnees
```

Quatre choses à savoir :

- le fichier se range dans **`exports/<année>/`**, à côté de `sorties/`, et
  nulle part ailleurs : un livre de paie emporte des salaires ;
- **`--coordonnees`** ajoute le NIR, la date de naissance et l'adresse. Sans
  l'option, rien de tout cela ne sort — un export qui les emporterait sans
  qu'on l'ait demandé serait un export qu'on envoie sans y penser ;
- **trois formats, et ils valent pour tous les états** : `csv`, `xlsx`, `pdf`.
  C'est une propriété de la machinerie d'export, pas de l'état exporté — un
  état neuf écrit ses lignes et ses colonnes, les trois formats le suivent.
  Le **PDF** s'édite en A4 paysage, repose l'en-tête des colonnes à chaque
  page et numérote ; le **classeur** écrit les montants en nombres, pour
  qu'ils s'additionnent (`rust_xlsxwriter`, dans le programme). Le PDF est
  fait par la fabrique de PDF, elle aussi dans le programme. C'est le CSV qui
  fait foi ;
- chaque ligne porte **d'où elle vient** (colonne `Source`), parce qu'il y a
  deux sources et qu'elles n'ont pas les mêmes colonnes :
  - **saisie** — le mois a son fichier dans `paies/`. Les bulletins sont
    recalculés au centime, et toutes les colonnes sont remplies ;
  - **DSN déposée** — le mois est antérieur à PuppetPaie. C'est ce que la DSN
    a déclaré avoir versé, importé de CoolPaie dans
    `historique/paies.json` : brut, net imposable, net à payer, PAS. **Une
    DSN ne déclare pas la répartition des cotisations entre part salariale et
    part patronale** (bloc 81 : un montant par code, les deux parts
    confondues), donc pas le coût employeur non plus. Ces colonnes restent
    **vides** sur ces lignes, les totaux ne les comptent pas, et l'export dit
    sur combien de bulletins chaque total porte.

  Un mois qui a une saisie est lu là, jamais dans l'historique — même règle
  que pour les droits (le moteur Rust, `paie/src/cumul.rs`).

Un export **lit** : il ne calcule aucune paie, ne dépose rien, ne consomme
aucun numéro. Un mois qui n'a pas été produit n'a pas de numéro de contrat
figé, et la colonne reste vide plutôt que de porter un numéro qui n'a jamais
été déclaré.

### Tous les états (23/09/2026)

`paie exporter` sans argument les liste, rangés par famille comme
dans le menu de l'onglet. Chacun se demande pour un mois, une année ou toutes
les années (sauf mention), et s'écrit en **CSV, texte tabulé, XLSX, PDF, HTML
ou JSON** — la machinerie commune `paie/src/tableaux_export.rs`, que chaque état suit
sans rien avoir à dire. Chaque fichier porte en pied ses avertissements et
ses **sources** (l'article, la norme ou le format qui le fonde).

| famille | état | ce qu'il sort | lit |
|---|---|---|---|
| paie | `livre-de-paie` | une ligne par bulletin (ci-dessus) | saisie + DSN |
| | `livre-de-paie-detaille` | chaque bulletin ligne à ligne : rémunération, cotisations (base, taux, deux parts) | saisie |
| | `recapitulatif-annuel` | un salarié par ligne, le brut des 12 mois, les cumuls (année ou toutes) | saisie + DSN |
| | `masse-salariale` | un mois par ligne : salariés, bulletins, masses, coût | saisie + DSN |
| | `etat-des-virements` | le net à verser par salarié et par mois, et sa date quand la DSN la déclare | saisie + DSN |
| | `prelevement-a-la-source` | assiette, abattement, type de taux, taux, montant (`--coordonnees` : NIR) | saisie + DSN |
| cotisations | `cotisations-par-organisme` | par organisme (ou `--par mois`) : base, taux, parts ; RGDU ventilée | saisie |
| | `versements-organismes` | les versements ordonnés par chaque DSN déposée (bloc 20) | DSN déposée |
| | `bordereau-urssaf` | CTP, assiettes, total dû (blocs 22 et 23) | DSN déposée |
| | `reduction-generale` | coefficient, montant, parts Urssaf / chômage / retraite | saisie |
| | `assiettes` | les assiettes de chaque bulletin et le plafond | saisie |
| | `regularisation-casc-svp` | la régularisation CASC-SVP (§74) | DSN déposée |
| | `fcap-svp` | le FCAP-SVP de la 3090 : 0,1 % de la tranche A de l'année, plancher 80 €, plafond 300 €, payé hors DSN (année ou toutes) | DSN déposée |
| comptabilité | `ecritures-comptables` | le journal de paie, une pièce par mois ; en plus `--format fec`, `ximport`, `oxygene`, `sage`, `quadra`, `ebp`, `pennylane`, ou `--logiciel` | saisie |
| | `grand-livre-paie` | les écritures par compte, un solde par compte | saisie |
| | `balance-paie` | débit, crédit, solde par compte | saisie |
| spectacles | `cout-par-spectacle` | le coût employeur de chaque spectacle, salarié par salarié | saisie + DSN |
| | `heures-et-cachets` | cachets, heures (annexes 8 et 10), jours, brut | saisie + AEM |
| | `contrats` | numéro, nature, emploi, spectacle, dates, brut | saisie + DSN |
| personnel | `registre-du-personnel` | le registre unique (L1221-13, D1221-23), une ligne par embauche | saisie + DSN |
| | `fichier-du-personnel` | les fiches, coordonnées comprises (pas de période) | fiches |
| | `journal-des-declarations` | chaque DSN, DPAE, envoi, signature : date, mode, issue | journal |

```
paie exporter cotisations-par-organisme --annee 2026 --par mois
paie exporter ecritures-comptables --annee 2026 --format fec
paie exporter ecritures-comptables --mois 2026-09 --format ximport
paie exporter ecritures-comptables --annee 2026 --format oxygene
paie exporter ecritures-comptables --annee 2026 --logiciel ebp
paie exporter registre-du-personnel --toutes-les-annees --format pdf
paie exporter versements-organismes --annee 2025 --format xlsx
```

**Ce qui lit « saisie » écarte les mois d'avant PuppetPaie, et le dit** :
la DSN ne déclare pas la répartition des cotisations, rien n'est
reconstitué. **Ce qui lit « DSN déposée » lit le fichier parti** (règle du
§74.2) : un mois sans DSN réelle acceptée n'y est pas, et un état sans aucune
DSN refuse.

**Les formats de comptabilité** (`paie/src/etats_compta.rs`) :

- **`fec`** — un *extrait* du journal de paie aux 18 zones du Fichier des
  Écritures Comptables (art. A47 A-1 du LPF) : tabulation, dates AAAAMMJJ,
  virgule décimale, en-tête des zones. **Ce n'est pas le FEC de
  l'entreprise**, que seul le logiciel de comptabilité produit ; c'est ce
  qu'il importe (Paheko, et tout logiciel qui lit le FEC). `ValidDate` reste
  vide : la validation se fait à l'import.
- **`ximport`** — le `XIMPORT.TXT` de Ciel Compta (lu aussi par EBP), à
  largeurs fixes, jeu de caractères Windows. Relevé dans une documentation
  tierce : **faire un premier import d'essai** avant de s'y fier.
- **`tsv`** — le texte tabulé aux colonnes de sPAIEctacle (journal, date,
  pièce, compte, libellé, débit, crédit), que Paheko importe en « Complet ».
- **`oxygene`** — Oxygène Comptabilité (Memsoft), « Import écritures à
  partir d'1 seul fichier », format EXCEL2 : point-virgule, en-tête des
  codes en majuscules (CODSOC … NUMCP2), dates AAAAMMJJ, n° de pièce sur
  10 (`PAIE202608`). Relevé dans l'aide de Memsoft ; séparateur décimal
  (virgule) et jeu de caractères (Windows) non précisés par elle.
- **`sage`** — Sage 100 Comptabilité, « Écritures Sage (*.pnm) », format
  105 caractères, la première ligne au nom de la compagnie. Relevé dans la
  *Description du format PNM* publiée par Sage (2022).
- **`quadra`** — Cegid Quadra / Quadratus, enregistrements `M` du fichier
  ASCII (QC_ASC.doc, 2023) : montants en centimes signés, folio `000`,
  comptes sur 8 (`641100` → `64110000`) ; un compte de plus de 8 caractères
  fait refuser (préférer `421{code:03}` à `421{cle}`).
- **`ebp`** — EBP Comptabilité, « Fichiers au format EBP V3 pour
  Windows » : virgule, sans en-tête, libellé et pièce entre guillemets,
  point décimal. Relevé dans la notice d'EBP (01/07/2006) reproduite par un
  revendeur ; renommé `ECRITURES.TXT`, EBP le propose à l'ouverture.
- **`pennylane`** — CSV aux colonnes que nomme son centre d'aide (Date,
  Code journal, Numéro de compte, Libellé, Débit, Crédit, Code
  analytique) ; on les fait correspondre au premier import.
- Tous ceux-là : **faire un premier import d'essai** sur un dossier de test.
- **Le logiciel du comptable** se choisit une fois — dans l'onglet
  Exports (menu « Logiciel du comptable », le nom et le format), ou
  `paie compta-logiciel oxygene` — et reste retenu
  (`config/compta.json`) : il fixe le format des écritures. Onze entrées :
  `oxygene`, `sage`, `quadra`, `ciel`, `ebp`, `pennylane`, `paheko`, `acd`
  (le `tsv`, par son « Interface générique »), `loop`, `isacompta`,
  `autre` (le FEC). `--logiciel` et `--format` passent outre le temps d'un
  export. Le plan comptable est **contrôlé pour ce format dès le choix**
  (compte ou journal trop long, sous-compte qui ne se remplit pas).
- **Les comptes et leurs libellés** accompagnent les écritures : en tête du
  fichier Quadra (enregistrements C, sans toucher un compte existant),
  `COMPTES.TXT` pour EBP, `oxygene-plan-comptable-paie-….csv` pour Oxygène.
- **L'analytique par spectacle** (case dans l'onglet, `--analytique`) :
  les salaires bruts ventilés par spectacle — CSECT (Oxygène), ligne A
  (Sage), code affaire (Quadra), ligne `>` (EBP), zone analytique
  (XIMPORT), colonne (Pennylane). Codes tirés des titres, ou fixés dans
  `config/plan-comptable.json` : `"sections": {"Gilles & Cassandre": "GILL"}` ;
  deux spectacles au même code font refuser. Les sections doivent exister
  dans le logiciel du comptable.
- **Pas de TRA (Cegid)** : il exige en tête tout le paramétrage du dossier
  (exercice, devises, TVA, comptes, journaux), et Pennylane refuse un TRA
  qui ne sort pas de Cegid. Cegid Loop importe le FEC.
- **Un 421 par salarié**, si le comptable en tient un : dans
  `config/plan-comptable.json`, `"net": {"compte": "421000", "libelle": "Net
  a payer", "par_salarie": "421{code:03}"}` (ou `"421{cle}"`). Le FEC garde
  421000 en compte général et porte le sous-compte en compte auxiliaire
  (CompAuxNum, CompAuxLib) ; les autres formats, le grand livre et la
  balance portent le sous-compte à la place du compte général.

Les écritures sont **les mêmes** que l'export compta de la clôture
(`ecritures_du_mois`, même plan comptable, même refus d'un compte deviné) ;
seuls la période et le format changent.

### Le dossier d'un salarié (23/09/2026)

Tous les documents d'un salarié — ou de chacun —, sur un mois, une année ou
toute la période, en une fois. Dans l'onglet **Exports**, « Le dossier d'un
salarié » : on coche ce qu'on sort.

```
paie dossier FAVART --toutes-les-annees        # un ZIP
paie dossier --annee 2025 --seulement bulletins  # un PDF par salarié
paie dossier --mois 2026-08 --ensemble         # tous dans un fichier
paie dossier FAVART --annee 2026 --envoyer     # et l'envoyer (mot de passe)
paie dossier --toutes-les-annees --apercu      # compter les pages
```

| catégorie | d'où |
|---|---|
| `bulletins` | **les pages mêmes** du fichier archivé du mois (`Bulletins_<Mois>.pdf`), celles du salarié seulement : rien n'est recalculé ni réimprimé (règle 4) ; une page introuvable refuse ; CoolPaie tels quels |
| `contrats` | réédités aux numéros de l'archive (ou figés dans la saisie) ; CoolPaie tels quels |
| `contrats-signes` | tels quels, et dans le ZIP les courriels de signature (`Signature_*.eml`) |
| `certificats-cs` | les pages mêmes de `CertificatsCS_<Mois>.pdf`, à ses N° CE (lus à partir du récapitulatif du mois) ; sans récapitulatif ou sans archive, aucun certificat, et le dossier le dit |
| `attestations` | l'AER de France Travail et les AEM de CoolPaie, telles quelles |
| `documents-fin` | certificat de travail et reçu pour solde de tout compte, tels quels |
| `recapitulatif` | ses heures et cachets, et son récapitulatif annuel |

Une catégorie : un PDF. Plusieurs : un ZIP d'un PDF par catégorie. Les
fichiers vont dans `exports/dossiers/`. **Aucun numéro n'est consommé** ; une
page rééditée le dit en pied (« Réédité par PuppetPaie le … »). Les pages
CoolPaie sont rattachées au salarié par le NIR, ou par le nom **et** le
prénom, et datées par leur période. L'envoi passe par la boîte de la
compagnie, avec le mot de passe du logiciel ; un dossier `--ensemble` ne
s'envoie à personne.

## La version, et la mise à jour dans l'ombre

Le logiciel porte un numéro de version — une date, dans le fichier
`VERSION` — que `paie version` affiche, comme la fenêtre de
lancement et le bandeau de l'interface.

**Une installation, un logiciel, une mise à jour** (lot U1, 14/10/2026) : ce
n'est pas une compagnie qui se met à jour, c'est le logiciel. Au lancement, il
**cherche une nouvelle version** pour lui-même, sans qu'aucune compagnie soit
ouverte, au dépôt que dit le réglage de l'installation (`maj/reglage.json`, à
côté de `logiciel/`, clé `depot` : un dossier ou une adresse web ; en repli,
`prive/config/maj.json` ; sinon l'adresse par défaut du code). Si elle existe,
l'archive de son système (`derniere.json` en nomme quatre : `windows`, `linux`,
`mac`, et `tout-systeme`, prise si la sienne manque) est téléchargée dans le
fil de fond, vérifiée (condensat, puis **signature de l'éditeur**, une par
archive — Ed25519, la clé publique est dans `paie/src/maj.rs`,
`CLE_PUBLIQUE` : un dépôt compromis ne peut pas glisser une version qui n'est
pas la nôtre), déballée dans `maj/`, à côté de `logiciel/`, et **contrôlée** :
son programme doit lire la configuration de chaque compagnie de l'installation.
Ce qui s'est passé s'écrit dans `maj/journal.log`. Alors
seulement le bandeau dit *« version 2026.09.16 — la 2026.10.02 est prête :
redémarrez PuppetPaie quand vous voulez »*. Rien d'autre ne change : le
logiciel en cours continue.

Au démarrage suivant, le programme **bascule** avant toute fenêtre : il
lance un relais hors de `logiciel/` (sous Windows, un programme ne se
remplace pas pendant qu'il tourne), qui met ce qui était dans `logiciel/`
dans `logiciel/.avant/`, la version prête à sa place, et ouvre la neuve. Si
quelque chose casse en route, tout revient.
`paie maj --revenir` reprend la version d'avant.

Pas d'avertissement : un dépôt injoignable, une version qui ne passe pas
le contrôle, c'est un détail dans le rapport d'ouverture, jamais un
incident — une compagnie hors ligne fait sa paie. Une seule chose s'affiche
après coup, une fois : la phrase que la version porte quand elle **change
un calcul** (`--calcul` au moment de publier). Un taux qui change n'en est
pas une : la déclaration est mensuelle, le mois suivant rattrape.

Deux garde-fous de plus depuis le 17/09/2026 :

- **Une version qui change un calcul attend la DSN.** Entre la saisie d'un
  mois et son dépôt, les bulletins sont produits, parfois envoyés ; changer
  le calcul ferait une DSN qui ne dit plus ce que dit le bulletin. Tant qu'un
  mois saisi d'une compagnie réelle de l'installation attend sa DSN (une fictive
  ne retient rien : elle ne dépose jamais), la version reste prête, le bandeau dit *« attend la DSN de 2026-11 »*, et elle
  s'applique après le dépôt. Pour passer outre sciemment (la correction est
  justement celle qu'il faut avant de déposer) : le bouton du bandeau, ou
  `paie maj --appliquer --malgre-calcul`.
- **Deux sortes de versions.** La plupart sont silencieuses. Quelques-unes
  sont **exigées** (`--exigee "pourquoi"` à la publication : une norme DSN
  qui change, une déclaration que l'ancienne version ferait fausse) : une
  fois téléchargée, le bandeau le dit fort — *« fermez et rouvrez PuppetPaie
  avant de déclarer »* —, **aucun dépôt réel ne part** tant qu'elle n'est
  pas appliquée, et elle passe outre l'attente du calcul.

Chez l'éditeur (`"editeur": true` dans le réglage de l'installation, ou en repli
`prive/config/maj.json`), on **publie** en plus, après relecture des sources (voir
plus haut) ; son installation reçoit comme les autres une version plus récente
que la sienne :

```bash
paie maj --cle                      # une fois : la clé qui signe
paie maj --publier --notes "..." [--calcul "..."] [--exigee "..."]
```

pose les archives **signées** (une par système dont la fenêtre est dans
`logiciel/`, et celle de tous) et `derniere.json` dans le dépôt, et refuse de
republier un numéro déjà pris : augmenter `VERSION` d'abord. La clé privée
vit dans `prive/config/maj-cle-privee.bin`, scellée par la clé maîtresse
du coffre (14/10/2026) : elle s'ouvre où le coffre s'ouvre, par la clé du
poste ou, sur un autre, par le mot de passe du logiciel ; elle dépend donc du
`coffre.bin` qui l'a scellée. `paie maj --cle` refuse de la fabriquer sans mot
de passe du logiciel, et rescelle ainsi une clé faite avant (scellée par la
clé du poste seule), sans changer la paire. Perdue, les compagnies refuseraient
toute version suivante. Le dépôt n'est qu'une
adresse ; le format est le même sur un disque et sur un site.

## Les compagnies fictives (le crash-test)

Le moteur se vérifie sur les déclarations **réelles** d'une seule compagnie :
c'est ce qui lui donne sa valeur de preuve, et c'est aussi son angle mort. Le
banc d'essai n'emploie aucun permanent, ne relève pas de la CCNEAC,
n'adhère à aucune organisation patronale, n'a qu'un établissement et ne prend
pas de stagiaire — cinq cas qu'aucune de nos données n'éprouve.

`outils/compagnies_fictives.py` fabriquait seize compagnies **de toutes
pièces** — rien n'était emprunté à `prive/` — et leur faisait produire un
mois. Il est parti avec le Python (lot K, 09/10/2026) et **n'est pas porté** :
`docs/COMPAGNIES-FICTIVES.md` et `docs/COMPAGNIES-FICTIVES-DEPOT.md` gardent
son dernier bilan (21/09/2026), qui n'est plus régénéré.

Les tests du moteur Rust montent leurs propres compagnies fictives
(`sources/paie/tests/donnees/racines/`, et celles que les tests écrivent) et
lisent la copie figée des compagnies fictives (`sources/paie/tests/donnees/fictives/`).
Les identités y sont inventées et **leurs clés se vérifient** : clé de NIR
calculée, clé de Luhn du SIRET, IBAN de forme juste. Un test ne doit pas
pouvoir passer sur un numéro impossible.

**Ce n'est pas un réparateur.** Les refus sont voulus tant que les taux
manquent, et la liste des cas muets est le travail à venir, pas un défaut à
cacher. Le seul validateur reste le **dépôt de test** net-entreprises, qui ne
conserve rien : les DSN produites ici sont conformes au tableau des usages et
aux formats de la norme, **et à rien d'autre**, tant qu'elles n'y sont pas
passées.

### La compagnie d'essai : le dépôt de test sans `prive/`

Une fictive ne va jamais au réseau (ses dépôts sont joués). Pour un **vrai**
dépôt de test depuis une machine qui n'a pas les données de la compagnie (une
séance dans le nuage, le poste d'un autre développeur) : `paie essai monter`
copie une fictive hors du dépôt et lui donne l'identité d'une vraie compagnie,
lue dans **une** variable d'environnement, `PUPPETPAIE_COMPAGNIE` (un objet
JSON sur une ligne : SIREN, NIC, en-tête, déclarant, contact, compte DPAE).
Ses dépôts de test partent sur net-entreprises et à l'Urssaf ; tout dépôt réel
y est refusé. Les clés, la suite des commandes et ce qui reste à confirmer :
`docs/COMPAGNIE-D-ESSAI.md`.

### Le banc d'essai daté, le scénario et son rejeu (#236)

Pour faire **vivre** une compagnie fictive dans l'interface, comme un testeur,
mois après mois : `node outils/banc/banc.js lancer <banc> --jour AAAA-MM-JJ`
pose une installation bridée neuve hors du dépôt et y lance la fenêtre de
l'atelier (`paie-atelier interface --enregistrer … --jour …`) ; on joue dans la
page avec Playwright (Node), on crée sa compagnie par « + », `simuler` en fait
une fictive aux envois simulés, `jour <banc> AAAA-MM-JJ` avance le calendrier
et l'horloge de la fictive joue ce qui était dû. La notice entière, celle
qu'un agent suit : `outils/banc/LISEZ-MOI.md` ; le modèle d'une partie :
`outils/banc/exemple.js`.

Pendant ce temps, **l'enregistreur** (`paie/src/scenario.rs`, dans le seul
binaire de l'atelier) consigne chaque route que la fenêtre sert, avec sa
charge, son instant et ce qu'on avait déposé (`gestes.jsonl`, `pieces/`).
Le scénario se **rejoue** sans navigateur, depuis un dossier vide, dans une
installation que le rejeu monte lui-même, bridée en toutes lettres :
`paie-atelier scenario rejouer <scénario> <dossier neuf>` (ou, sans le binaire
de l'atelier, `cargo run --profile essais --example vie_fictive -- rejouer …`) ;
`outils/demos/les-faisceaux/` en est un, écrit à la main. Au rejeu comme au banc,
le temps du moteur est figé à l'instant de chaque geste et les aléas suivent une
suite fixe (`socle/src/temps.rs`) : deux rejeux écrivent les mêmes octets, et
`banc.js verifier` (`scenario verifier`) contrôle, fichier par fichier, que le
rejeu refait la compagnie du banc. `scenario elaguer` ôte les lectures d'un
scénario pour le figer dans `paie/tests/donnees/scenarios/<nom>/`, où il devient
un test (`paie/tests/rejeu.rs`) : il compare les paies, les DSN, la DPAE, le
texte des bulletins et des certificats CS et les journaux à un oracle
(`PUPPETPAIE_REECRIRE_ORACLES=1`).

## Ce que le moteur ne fait pas

- **Régularisations annuelles** (bloc DSN `S21.G00.82`, vu en avril 2026).
- **Payer un salarié permanent.** Le chantier est ouvert (voir « La nature du
  contrat ») : le calcul, la déclaration, le signalement de fin de contrat et
  l'écran sont là. La **prévoyance** n'est plus ce qui manque — elle vient de
  la convention collective, dans un catalogue livré avec le logiciel (1,20 %
  de la tranche A pour l'IDCC 3090, 1,50 % pour un cadre, à la charge de
  l'employeur) —, ni la mutuelle santé, ni la CASC-SVP, relevées le
  19/09/2026 sur la fiche de paramétrage Audiens. Ce qui retient le barème,
  ce sont les **identifiants d'affiliation** de la DSN, qui n'existent
  qu'une fois l'entreprise adhérée au régime de sa branche.
- **Déposer sans qu'on le lui demande.** La DSN et la DPAE partent par l'API,
  mais contre le mot de passe du logiciel tapé au moment de l'acte, et jamais
  deux fois pour le même mois ou la même embauche. Une déclaration part sous la
  responsabilité de l'employeur. (Plus d'AEM à déposer depuis le 01/06/2026 :
  la compagnie est en 100 % DSN, France Travail édite l'attestation lui-même.)

## Organisation

Le code et les données sont séparés, et c'est structurel. Deux raisons, et
la seconde est celle qui sert tous les jours : `logiciel/` a pour destination
un dépôt public, et une autre compagnie utilisera ce code avec **ses**
données à elle — le logiciel doit donc savoir que les siennes sont ailleurs.

Le rangement du 09/10/2026 (`atelier/CLAUDE.md`, « Le rangement ») :

```
PuppetPaie.cmd            le lanceur (Windows)
logiciel/                 exactement ce qu'une compagnie reçoit, et ce que publie
                          la mise à jour : puppetpaie.exe, WebView2Loader.dll,
                          VERSION (une date), donnees/ (barèmes, bases, modèles,
                          sources.json), interface/ (la page) ; et, posés par le
                          flux « Publier » seulement, linux/ et mac/ (puppetpaie,
                          paie) : une archive par système, et une pour tous
fictives/                 les compagnies fictives, livrées aussi
maj/                      la mise à jour de l'installation (jamais dans Git)
prive/                    les données de la compagnie du poste
atelier/                  ← ce dossier : tout le reste
  sources/paie/           le moteur (programme `paie`) : calcul, bulletin,
                          contrat, DPAE, certificats CS, DSN, signalements,
                          dépôts, retours, envois, exports, la mise à jour
                          (`src/maj.rs`) ; ses tests dans `tests/`
                          (`cargo test --profile essais`) ; paie.exe et les
                          programmes Linux et Mac dans `bin/`
  sources/fenetre/        la fenêtre (programme `puppetpaie`, Tauri)
  sources/fabrique-pdf/   la fabrique de PDF
  reference/              les textes qui fondent les règles, tels que servis
                          (lois et codes, conventions, norme DSN, BOFiP,
                          BOSS…), leur transcription et un LISEZ-MOI.md par
                          dossier (15/10/2026 ; d'un organisme privé, seule la
                          transcription) ; `paie sources` y vérifie les copies
  docs/                   ARCHITECTURE.md, RUBRIQUES.md,
                          COMPAGNIES-FICTIVES.md et COMPAGNIES-FICTIVES-DEPOT.md
                          (le dernier bilan du crash-test, 21/09/2026)
  outils/                 les outils d'atelier : `banc/` (le banc d'essai daté, en
                          Node : LISEZ-MOI.md), `demos/` (un scénario de
                          démonstration). Plus un fichier Python depuis le
                          15/10/2026 : le garde avant commit, le guet, les tables
                          Unicode, les relevés des référentiels, des codes et des
                          conventions sont des commandes de `paie`
                          (`verifier-avant-commit`, `guet`, `generer-tables`,
                          `extraire`, `releve-codes`, `releve-conventions`)
  PuppetPaie.sh, PuppetPaie.command   les lanceurs Linux et Mac
  prive-dev/, depot/ (versions publiées), site/, maquettes/
                          (le serveur du filet : dépôt du site de l'éditeur, infra/filet/)
.claude/                  à la racine : compétences Claude (paie, contrat,
                          declarations, ouverture)

prive/                    les données de la compagnie
  config/                 entreprise, salariés, spectacles, barèmes, compteurs,
                          effectifs (écrit par le moteur)
  paies/                  fichiers de saisie, un par mois
  imprevus/               les imprévus des salariés (arrêts de travail,
                          suspensions), au mois où ils commencent (créé au
                          premier) ; l'ancien dossier arrets/ n'est plus lu
  exports/                les états pour le comptable et les organismes
  historique/             les contrats et les paies attestés avant ce logiciel
                          (l'import), et les copies de ce qu'il a repris
                          (`bulletins/` ; `coolpaie/` avant le 10/10/2026)
  sorties/                les fichiers produits
  gabarits/               enregistrements AEM et DPAE réels servant de patrons
  retours/                ce que renvoient les organismes
  reference/              des extraits de l'historique CoolPaie ; les textes
                          déposés sont dans `atelier/reference/` (15/10/2026)
  journal/                ETAT.md — où en est le projet
```

La compagnie est `PUPPETPAIE_DONNEES` si elle est posée, sinon le dossier
`prive/` voisin de `logiciel/` (`paie/src/commandes/compagnie.rs`) ;
`paie --dev` prend la copie de développement. Un fichier de données s'écrit
tel quel s'il existe, sinon sous la compagnie ; un mois nu (`2026-10`)
désigne sa saisie.

Deux faces, une seule vérité : la ligne de commande et l'interface passent
toutes deux par les routes du moteur, qui portent l'orchestration et les
garde-fous. Aucun front ne décide quoi que ce soit.

Voir [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) pour le détail des formats et la façon
dont chaque taux a été retrouvé.

### Avant un commit : le garde des données réelles

Rien de la compagnie du poste n'entre dans le dépôt public. `paie
verifier-avant-commit` cherche, dans ce que Git emporterait (hors de
`prive/`), les NIR, IBAN et BIC qui ne portent pas la marque d'invention, et
les vraies valeurs qu'il lit dans `prive/config/` quand il est là (SIRET,
SIREN, comptes des organismes, IBAN, BIC, NIR, courriels, adresses, nom et
prénom des salariés). Il dit le fichier, la ligne et ce que c'est, jamais la
valeur ; il sort en 0 (rien), 1 (trouvé) ou 2 (il n'a pas pu vérifier). Le
test `donnees_reelles` de la suite le complète sans lire `prive/`.

```bash
paie verifier-avant-commit             # tout ce qui partirait
paie verifier-avant-commit --indexes   # ce que l'index porte : le prochain commit
git config core.hooksPath atelier/.githooks   # une fois : le crochet le lance à chaque commit
```

Le crochet (`atelier/.githooks/pre-commit`) prend le `paie` de
`PUPPETPAIE_FABRIQUE`, sinon celui que Cargo a compilé (`CARGO_TARGET_DIR`,
sinon `atelier/sources/target`), sinon celui que le dépôt livre
(`atelier/sources/paie/bin/<système>/`) ; si aucun ne connaît la commande, le
commit est refusé. Une trouvaille se regarde : une valeur réelle sort du
dépôt, une valeur inventée s'inscrit dans `FICTIVES`
(`atelier/sources/paie/src/verifier_avant_commit.rs`). Passer outre en
connaissance de cause : `git commit --no-verify`.

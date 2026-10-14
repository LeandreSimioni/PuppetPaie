# Comment ce dossier est rangé

**PuppetPaie est un logiciel de paie du spectacle vivant, pour toute compagnie
qui emploie des artistes et des techniciens** — intermittents au cachet comme
salariés permanents. La compagnie du poste en est le premier utilisateur et le
banc d'essai, pas le destinataire : ce qu'elle ne fait pas, d'autres le font,
et le logiciel doit le porter. Sa destination technique est un portage en
**Rust, multiplateforme**, une fois le moteur réglé.

Deux dossiers, et la frontière entre les deux est la seule chose à retenir.

```
puppetpaieprivate/
│
├── logiciel/     ← LE LOGICIEL. C'est lui qui ira sur GitHub, en public.
│                   Du code, des documents, aucune donnée réelle.
│
├── prive/        ← NOS DONNÉES. Elles n'iront jamais dans le dépôt public.
│                   NIR, adresses, salaires, déclarations déposées.
│
├── prive-dev/    ← LA COPIE DE DÉVELOPPEMENT de prive/ : les essais de DSN
│                   et de DPAE s'y font, EN LIGNE DE COMMANDE SEULEMENT
│                   (python -m moteur --dev ...) ; rien n'en part en réel.
│                   Elle n'apparaît jamais dans l'interface (26/09/2026).
│
├── demo/         ← LES COMPAGNIES FICTIVES : c'est sur elles qu'on essaie
│                   l'interface.
│
└── depot/        ← LES VERSIONS PUBLIÉES (python -m moteur maj --publier) :
                    l'archive et derniere.json. En attendant le dépôt en ligne.
```

À côté de ce fichier, **`PUPPETPAIE-TEL-QU-IL-EST.md`** (périmé depuis le portage Rust) décrit le logiciel
tel qu'il était le 21/09/2026, relu le 23/09 et mis à jour le 26/09, pour en discuter hors de
l'outil qui l'a construit — ses chiffres sont ceux de ces jours-là, et sa dernière section
liste ce qui grince. À réécrire quand il ne dit plus vrai.
**`PLAN-DEPOT-PUBLIC.md`** dit comment le dépôt public se fera, le jour où
Léandre l'ouvrira.
**`atelier/`** (07/10/2026) : les outils du développement qui ne partent pas avec le
logiciel (le guet des sessions Claude). Il accueillera le reste de l'atelier à
l'ouverture du dépôt public.

**Une seconde frontière, dans `logiciel/` (01/10/2026)** : le **produit**
(ce que les compagnies installent — demain un petit binaire Rust) et
l'**atelier** (veille des conventions, relevés, extracteurs, tests,
compagnies fictives). Aujourd'hui ils sont mêlés dans `moteur/` et tout part
dans la mise à jour ; la séparation se fera à l'ouverture du dépôt public.
Voir `PLAN-DEPOT-PUBLIC.md`, « La forme du dépôt public ».

---

## Pourquoi séparer

Parce qu'on veut publier le logiciel sans publier la paie de deux personnes.

Tant que le code et les données vivaient mêlés, la seule protection était une
liste de dossiers à ne pas envoyer — et il suffisait d'un oubli. Maintenant la
question ne se pose plus : **on publiera `logiciel/`, un point c'est tout.** Ce
qui est dans `prive/` ne peut pas partir par accident dans le dépôt public,
puisqu'il n'en fera pas partie. (Le dépôt de travail, lui, est privé et porte
les deux : voir « Le dépôt GitHub » plus bas.)

Un effet de bord utile : le logiciel sait désormais que ses données sont
ailleurs. C'est exactement ce qu'il faudra le jour où une autre compagnie
l'utilisera — ses données à elle, notre code.

---

## Ce qu'il y a dans `logiciel/`

| | |
|---|---|
| `paie/` | **le moteur Rust** : tout le logiciel ; ses tests (`cargo test`) tournent sur des fictives et sur une copie figée des démos. Le Python est retiré (lot K, 09/10/2026) |
| `fenetre/` | la fenêtre Tauri (programme `puppetpaie`) |
| `donnees/` | les référentiels livrés : barèmes, bases, modèles, avec leurs sources (`sources.json`) |
| `interface/` | la page que la fenêtre affiche |
| `docs/` | ARCHITECTURE.md (les formats et les taux), RUBRIQUES.md (comment une rubrique se construit), PORTAGE-RUST.md, COMPAGNIES-FICTIVES.md et COMPAGNIES-FICTIVES-DEPOT.md |
| `outils/` | l'atelier, seul endroit où le Python reste (à porter) : extracteurs de la norme DSN, relevés, garde-fou de publication ; `outils/demos/`, les scénarios joués par `paie/examples/vie_fictive.rs` |
| `site/` | les pages publiques de PuppetPaie, à mettre sur le site de l'éditeur |
| `.claude/` | les compétences : ce que Claude Code sait faire du logiciel |
| `README.md` | le mode d'emploi |
| `lanceur*.pyw`, `*.cmd` | les fenêtres à double-cliquer (retours DSN, dépôt DPAE) |
| `AVANT-LE-PREMIER-COMMIT.md` | **ce qui reste à nettoyer avant de publier** |

## Ce qu'il y a dans `prive/`

| | |
|---|---|
| `config/` | l'entreprise, les salariés, les spectacles, les barèmes, les compteurs, l'effectif (écrit par le moteur) |
| `paies/` | les saisies, un fichier par mois |
| `historique/` | les contrats attestés avant ce logiciel, importés des AEM de CoolPaie (`python -m moteur importer`) — ce que les droits lisent pour les mois d'avant |
| `sorties/` | les bulletins, contrats, DSN, AEM produits |
| `exports/` | les états pour le comptable, les organismes, les financeurs |
| `export-coolpaie/` | la copie du dossier `export/` de CoolPaie, d'où l'import lit |
| `gabarits/` | les enregistrements AEM et DPAE réels servant de patrons, le logo, la signature |
| `retours/` | ce que renvoient les organismes après dépôt |
| `maj/` | une version téléchargée et contrôlée, en attente du prochain démarrage (`prete.json`) — n'existe que chez une compagnie qui reçoit les mises à jour |
| `config/maj.json` | qui est ce poste (`editeur`) et où sont les versions (`depot`) |
| `reference/` | l'historique CoolPaie qui a servi à retrouver les règles ; `exemples/dsn-val/` : les DSN fictives à passer en test avant un premier vrai |
| `journal/` | **ETAT.md** (où en est le projet), `archives/` et son `INDEX.md`, et les dossiers de chantier (CONGES-SPECTACLES.md, PLAN-EXPORTS.md…) |
| `config/coffre.bin` | **les mots de passe des organismes.** On ne le lit pas. |

### Le fichier qu'on ne lit pas

`prive/config/coffre.bin` porte les mots de passe net-entreprises, Urssaf et de la boîte courriel,
scellés par AES-256-GCM (Windows, Mac et Linux). À côté, `google-jeton.bin`, et
`maj-cle-privee.bin`, la clé qui signe les mises à jour (17/09/2026). **On ne
les lit pas, on ne les affiche pas, on ne les recopie pas** —
ni à la main, ni par un assistant.

La clé qui les ouvre vit **hors de ce dossier**, dans
`~/.config/puppetpaie/` (ou `%APPDATA%\PuppetPaie\`) : une sauvegarde de
`prive/` ne transporte donc pas de quoi les lire.

C'est une convention, pas une garantie : un programme qui tourne sous ce compte
*peut* les déchiffrer. Ce qui la tient, c'est qu'elle est écrite ici et dans
`logiciel/CLAUDE.md`, et qu'aucune commande du moteur n'affiche jamais un mot
de passe. Pour tout le reste, `python -m moteur coffre` dit ce qui est posé
sans rien montrer.

### Si je perds l'ordinateur

La sauvegarde du Drive (`PuppetPaie / sauvegarde`) est la copie de `prive/`. Sur le poste neuf :

1. Installer PuppetPaie.
2. Relier le Drive de la compagnie : `paie google autoriser` (si elle avait son propre `config/google.json`, le remettre d'abord).
3. `paie restaurer` : `prive/` redescend, chaque fichier vérifié par son empreinte, rien d'effacé ; la commande dit ce qu'il reste à faire.
4. Le mot de passe du logiciel : `paie coffre ouverture` (l'actuel si le coffre est revenu, sinon un nouveau).
5. Ce qui manque, que `paie restaurer` nomme. Le coffre monte dans la sauvegarde depuis le 14/10/2026 (décidé par Léandre) : revenu, il s'ouvre par le mot de passe du logiciel, et les mots de passe d'organisme qu'il porte reviennent avec lui ; sans mot de passe du logiciel posé sur l'ancien poste, il ne s'ouvre pas, et les trois se reposent (`paie coffre poser net-entreprises`, `urssaf`, `courriel --compte <adresse>`). La clé qui signe les mises à jour (chez l'éditeur) redescend aussi : scellée par le coffre, elle s'ouvre avec lui ; s'il manque, `paie restaurer` le dit, et il reste `paie maj --cle --remplacer` (une nouvelle clé publique à faire parvenir aux compagnies). Une clé faite avant le 14/10 se rescelle par le coffre en lançant `paie maj --cle` sur son poste. Le jeton Google ne monte pas : on relie le compte de nouveau.

On l'essaie une fois avant d'en avoir besoin : `paie restaurer <un dossier vide>`.

`ETAT.md` est le document à lire en premier dans une nouvelle session de
travail. Il est ici et non dans `logiciel/` parce qu'il raconte l'enquête avec
de vrais chiffres : des NIR, des taux de prélèvement à la source, des salaires.

---

## Rien ne change dans l'usage

Les commandes s'écrivent depuis `logiciel/`, exactement comme avant :

```bash
python -m moteur mois paies/2026-10.json
```

Et il y a maintenant une interface, qui fait la même chose dans une page :

```bash
python -m moteur interface
```

`paies/2026-10.json` est cherché tel quel, puis dans `prive/`. Les habitudes,
les compétences Claude et les lanceurs continuent de marcher sans rien changer.

Le logiciel trouve ses données dans cet ordre : la variable d'environnement
`PUPPETPAIE_DONNEES` si elle existe, sinon `prive/` à côté du code, sinon le
dossier du code lui-même.

---

## Le dépôt GitHub

Il existe, il s'appelle **`puppetpaieprivate`**, et il est **privé**. Il porte
le code **et** les données, et c'est voulu : on écrit le programme, et on le
vérifie sur de vraies déclarations, de vrais bulletins, de vrais taux. Un
moteur de paie testé sur des données inventées ne prouverait rien.

**La destination de `logiciel/` reste un dépôt public**, le jour où Léandre
le décidera — c'est pour cela que la séparation existe, et c'est aussi ce
qui rendra le logiciel installable par une autre compagnie, avec **ses**
données à elle à côté du même code.

Mais ce n'est **pas un sujet de session** : ce qu'une publication demandera
est écrit une fois pour toutes dans `logiciel/AVANT-LE-PREMIER-COMMIT.md`,
qui dort jusque-là. On ne le rappelle pas en fin de compte rendu, et on ne
retire jamais une donnée réelle d'un test pour « préparer le terrain ».

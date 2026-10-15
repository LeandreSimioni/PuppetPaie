# GitHub : environnements protégés, approbation d'un déploiement (relevé du 14/10/2026)

Relevé pour décider comment publier une mise à jour signée depuis le dépôt public `LeandreSimioni/PuppetPaie`, approuvée
par Léandre depuis son téléphone (décision de principe du 14/10/2026 ; à construire après le lot U1).

| fichier | d'où | ce qu'on en lit |
|---|---|---|
| `github-docs-environnements-20261014.*` | https://docs.github.com/en/actions/how-tos/deploy/configure-and-manage-deployments/manage-environments | « Environments, environment secrets, and deployment protection rules are available in public repositories for all current GitHub plans. » ; « If you are on a GitHub Free, GitHub Pro, or GitHub Team plan, other deployment protection rules, such as a wait timer or required reviewers, are only available for public repositories. » ; « workflow jobs that use this environment can only access these secrets after any configured rules (for example, required reviewers) pass. » ; option « Prevent self-review ». |
| `github-docs-deploiements-et-environnements-20261014.*` | https://docs.github.com/en/actions/reference/workflows-and-actions/deployments-and-environments | « If the environment requires approval, a job cannot access environment secrets until one of the required reviewers approves it. » ; « If you are using GitHub Free, environment secrets are only available in public repositories. » ; « Deployment branches and tags are available for all public repositories. » |
| `github-docs-approuver-un-deploiement-20261014.*` | https://docs.github.com/en/actions/how-tos/deploy/configure-and-manage-deployments/review-deployments | la marche pour approuver un travail en attente ; « At this point, the job can access any secrets stored in the environment. » |
| `github-blog-approbation-sur-mobile-20210401.*` | https://github.blog/changelog/2021-04-01-reviewing-deployments-on-github-mobile | « GitHub Actions deployment reviews are now available on GitHub Mobile. » (2021 : le comportement actuel de l'application n'est pas revérifié). |

Ce qu'on en tire : sur un dépôt **public**, un compte gratuit peut poser une clé de signature comme **secret
d'environnement** et exiger l'approbation de Léandre avant que le travail qui signe y accède. « Prevent self-review » ne
doit **pas** être coché tant que Léandre est seul (il lance et il approuve). Inférence, non vérifiée : l'API d'approbation
(`pending_deployments`) accepte tout appelant qui est un approbateur requis ; un outil qui agirait avec le jeton de
Léandre pourrait donc approuver — d'où la règle à écrire : aucun agent n'approuve un déploiement.

Où le moteur s'en servira : le flux de publication du dépôt public (lot à venir, après U1). Rien encore dans le code.

## La dernière Release, lue par toutes les installations (lot P1, relevé du 14/10/2026)

| fichier | d'où | ce qu'on en lit |
|---|---|---|
| `github-docs-lien-vers-la-derniere-release-20261014.*` | https://docs.github.com/en/repositories/releasing-projects-on-github/linking-to-releases | « To link directly to a download of your latest release asset that was manually uploaded, the suffix is /releases/latest/download/asset-name.zip. » |

Constaté le 14/10/2026 (`curl -I` sur un dépôt public quelconque, rien déposé de plus) : `…/releases/latest/download/<fichier>`
répond **302** vers `…/releases/download/<étiquette>/<fichier>`, qui redirige à son tour vers le stockage de GitHub.
Inférence : « latest » ne désigne ni un brouillon ni une pré-version (le flux publie donc une Release ordinaire, marquée
« latest »).

Où le moteur s'en sert : `paie/src/maj.rs`, `DEPOT_PAR_DEFAUT` (`https://github.com/LeandreSimioni/PuppetPaie/releases/latest/download`),
lu par `lire_depot`, qui suit les redirections (ureq 3, dix au plus) ; le flux `.github/workflows/publier.yml`. Preuve :
`tests/fichiers/mise_a_jour.rs`, `deux_cles_acceptees_une_troisieme_refusee_par_un_depot_qui_redirige_comme_github`.

## Dans le dépôt public (15/10/2026)

Ce dossier est dans le dépôt public de PuppetPaie. Les originaux suivants, d'un organisme privé, n'y sont pas : seule leur transcription y est (`pdftotext -layout` pour un PDF, une feuille de calcul en CSV, feuille par feuille, le texte d'une page HTML), et c'est elle que citent les renvois. Les condensats donnés plus haut sont ceux des originaux, gardés dans le dépôt privé (`reference-privee/sources/github/`).

- `github-blog-approbation-sur-mobile-20210401.html` : transcription `github-blog-approbation-sur-mobile-20210401.txt`
- `github-docs-approuver-un-deploiement-20261014.html` : transcription `github-docs-approuver-un-deploiement-20261014.txt`
- `github-docs-deploiements-et-environnements-20261014.html` : transcription `github-docs-deploiements-et-environnements-20261014.txt`
- `github-docs-environnements-20261014.html` : transcription `github-docs-environnements-20261014.txt`
- `github-docs-lien-vers-la-derniere-release-20261014.html` : transcription `github-docs-lien-vers-la-derniere-release-20261014.txt`

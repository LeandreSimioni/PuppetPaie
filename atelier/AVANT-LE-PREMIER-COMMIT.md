# Avant de commiter : rien de réel n'entre dans le dépôt

Le dépôt est public, et l'historique d'un dépôt est définitif : une valeur poussée ne s'efface plus. Ce qui identifie
une personne ou la compagnie du poste (un NIR, un IBAN, un BIC, un SIRET, un courriel, une adresse, un nom) n'y entre
jamais. **On retire les identités, on garde les chiffres** : les montants prouvent que le calcul est juste, les noms
n'apportent rien. Un test qui perd ses valeurs perd sa preuve ; ce qu'on garde d'un cas réel, ce sont ses attendus,
transcrits sur des valeurs inventées (règle de fond 9 a de `CLAUDE.md`).

## Deux gardes

- **Le garde avant commit** : `moteur verifier-avant-commit` (`sources/moteur/src/atelier/verifier_avant_commit.rs`).
  Il cherche, dans ce que Git emporterait, trois formes (un NIR, un IBAN, un BIC), puis, si `prive/config/` est là, les
  vraies valeurs de la compagnie du poste qu'il y lit (`entreprise.json`, et pour chaque fiche de `salaries.json` le
  NIR, le numéro Congés Spectacles, le courriel, l'adresse, le nom et le prénom ; jamais `prive/config/*.bin`). Il ne
  contient aucune donnée réelle, et il ne dit jamais la valeur trouvée : le fichier, la ligne, et ce que c'est.
  Sortie : 0 rien trouvé, 1 trouvé, 2 pas pu vérifier.
- **Le verrou de la suite** : `sources/moteur/tests/fichiers/donnees_reelles.rs`, à chaque passage de tests, sans lire
  `prive/`.

Le garde tourne en crochet avant chaque commit (`atelier/.githooks/pre-commit`, avec `--indexes` : ce que l'index
porte) ; le crochet se pose une fois par checkout, et `.claude/hooks/session-start.sh` le pose dans le nuage :

```bash
git config core.hooksPath atelier/.githooks     # refuse le commit fautif
moteur verifier-avant-commit                    # à la demande, sur ce qui part
```

Le crochet se sert du premier moteur qui connaît la commande : celui que nomme `PUPPETPAIE_FABRIQUE`, puis celui de la
cible Cargo, puis l'exécutable livré (`sources/moteur/bin/<système>/moteur`). Rien à compiler pour commiter.

## Un commit refusé

Une trouvaille se regarde ; on ne « nettoie » pas un test parce que le garde le signale.

- **Une valeur réelle** sort du dépôt : elle est remplacée par une valeur inventée, et le cas se réécrit en fictif.
- **Une valeur inventée** se reconnaît : un NIR inventé porte `999`, `123456` ou `000` dans ses treize chiffres
  (`INVENTE`) ; toute autre valeur fabriquée s'inscrit, avec sa raison d'être, dans `FICTIVES`. Une valeur fabriquée a
  une clé qui se vérifie (clé du NIR, clé de Luhn du SIRET, clé de l'IBAN) : un test ne doit pas pouvoir passer sur un
  numéro impossible.
- **L'exemple imprimé par l'émetteur d'un texte de référence** (`atelier/reference/`) s'inscrit dans
  `EXEMPLES_DES_TEXTES`.

`git commit --no-verify` passe outre ; il ne sert que quand le garde se trompe, et le garde se corrige alors dans le
même geste.

## Ce qui n'est pas dans le dépôt

- `prive/` (`/prive/` dans `.gitignore`) : la compagnie du poste, sur le poste, la clé USB et le Drive seulement ;
- `logiciel/donnees/google-application.json` : l'application Google du logiciel, que le flux « Publier » écrit depuis le
  secret `PUPPETPAIE_GOOGLE_APPLICATION` et livre dans la Release.

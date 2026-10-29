# Compagnies fictives

Trois sortes de compagnies qui ne sont pas une vraie compagnie : les fictives livrées, celles qu'on ajoute sur un
logiciel bridé, et les compagnies de test que montent les tests et le banc.

## Les fictives livrées

Elles sont dans `fictives/`, à la racine du dépôt (`fictive-1285-grande`, `fictive-1285-petite`, `fictive-3090-grande`,
`fictive-3090-petite` ; l'équipe et les spectacles de chacune : `fictives/LISEZ-MOI.md`). « Publier » les livre à part
du logiciel, signées, et une installation ne les retélécharge que si leur empreinte change ; la livraison ne remplace
que ce qu'elle a posé (`moteur/src/atelier/publier.rs`, `moteur/src/compagnie/maj.rs`).

- **Leur marqueur** (`COPIE-DE-DEVELOPPEMENT.json`, `"copie_de": "compagnie fictive …"`) les dit fictives. Le branchement
  des envois est alors **le simulateur** (`envois/src/branchement.rs`) : un dépôt DSN ou DPAE rend un flux `FICTIF-…`,
  la boîte courriel est simulée, France Travail rend une AER fictive. Rien ne quitte le poste, et aucun acte réel n'est
  possible.
- **Leur horloge** (`moteur/src/horloge.rs`) : à chaque ouverture, la fictive joue ce qu'une compagnie bien tenue aurait
  fait avant aujourd'hui, à sa date (valider les engagements, envoyer contrat et DPAE le jour de l'embauche, déposer la
  DSN à l'échéance, envoyer les documents, clore l'année). Ce que le visiteur a déjà fait lui-même n'est pas refait ;
  ce qui vient après aujourd'hui lui reste. L'horloge ne joue rien hors d'une compagnie fictive (`HorlogeFictive::verifier`).
- Elles ont leur photo d'origine et leur remise à zéro (`interface::compagnies::est_fictive_livree`).
- Un test ne les lit jamais.

## Les fictives ajoutées (logiciel bridé)

Sur un logiciel bridé, le « + » de l'accueil pose, après un message, `fictives/ajoutee-<nom>`, avec `"ajoutee": true`
dans son marqueur (`interface::compagnies::est_ajoutee`, la seule lecture de cette marque ; un dossier de `fictives/`
sans marqueur de fictive se lit aussi comme ajouté). Tout s'y fait ; le branchement de ses envois est **le message** :
dépôts DSN et DPAE (test compris), courriels et filet rendent le refus `EnvoiBride` (`moteur/src/compagnie/bridage.rs`)
avant que rien ne s'écrive au journal. Elle n'a ni photo d'origine ni remise à zéro ; la livraison des fictives ne la
touche jamais ; `.gitignore` l'écarte du dépôt.

## Les compagnies de test

- **Les tests** montent chacun leur compagnie (`moteur/tests/outils`, `outils::dossier_de_test`) et la nomment ; aucune
  compagnie n'est copiée d'un dossier figé.
- **Le banc** : une compagnie fictive jouée dans l'interface, dont l'enregistreur (binaire de l'atelier) consigne chaque
  route appelée et chaque jour avancé, dans un scénario (`moteur/tests/donnees/scenarios/treteaux-du-banc`,
  `atelier/outils/banc/`). Il se rejoue sans navigateur :
  `cargo run --profile essais --example vie_fictive -- rejouer <scénario> <installation neuve>`, et le test du rejeu
  (`moteur/tests/rejeu.rs`) compare ses sorties (engagements, DSN, DPAE, bulletins, certificats, journaux) à un oracle.
- **La compagnie d'essai** sert aux dépôts de test réels depuis le nuage : `COMPAGNIE-D-ESSAI.md`.

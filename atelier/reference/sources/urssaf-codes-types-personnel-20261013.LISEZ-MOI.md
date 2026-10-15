# Table des CTP Urssaf, relevé du 13/10/2026 (`paie sources --verifier`, lot S1)

- **D'où** : `https://open.urssaf.fr/explore/dataset/histocodestypescsv/download/?format=csv&timezone=Europe/Berlin&use_labels_for_header=true&csv_separator=%3B`
  (l'entrée `urssaf-codes-types-personnel` de `logiciel/donnees/sources.json`). Relevé le 13/10/2026 depuis le conteneur,
  par la commande `paie sources --verifier` puis `--relever`, portée en Rust ce jour-là.
- **Ce qui a changé depuis la copie du 18/09/2026** (`urssaf-codes-types-personnel.csv`, à côté) : **trois lignes
  nouvelles, toutes pour le CTP 532 « CONTRIBUTION PPVE »** (déplafonné 30,0 ; périodes 2011-2015, 2015-2027,
  2027-2030) ; le reste de la table est le même, réordonné. Comparé par `diff` des deux fichiers triés.
- **Où le moteur s'en sert** : `paie/src/bordereau.rs` lit la copie livrée `logiciel/donnees/urssaf-codes-types-personnel.csv`.
  Le CTP 532 (plan de partage de la valorisation de l'entreprise) ne sert à aucune rubrique du moteur : **la copie livrée
  n'est pas recopiée** (`sources.json`, « à recopier quand la source change » : rien ne change pour le calcul). Le
  manifeste (`releve_le`, condensat) se met à jour sur le poste de l'éditeur, par `paie sources --relever`.

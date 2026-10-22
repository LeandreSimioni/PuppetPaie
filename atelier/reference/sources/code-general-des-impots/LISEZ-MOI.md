# Code général des impôts : le relevé LEGI

Les `articles-AAAAMMJJ.md`, `releve.json` et `xml/` de ce dossier sont **écrits par `paie releve-codes`** (`moissonner`, puis
`rapport`) depuis la base LEGI de la DILA (`https://echanges.dila.gouv.fr/OPENDATA/LEGI/`) ; ils ne se retouchent pas à la
main. Les articles suivis sont la liste du texte `impots` de `atelier/sources/paie/src/releve_codes/textes.rs`, où chaque
domaine dit pourquoi il est là ; le dernier relevé est le plus récent des `articles-*.md`. `formulaires/` garde les notices
de la DGFiP de la taxe sur les salaires (2501-SD, 2502-SD), copiées telles que servies. Ce fichier dit, par sujet ajouté
depuis qu'il existe, ce qu'on en lit et où le moteur s'en sert (les sujets plus anciens sont décrits dans les commentaires de
`textes.rs` et dans les LISEZ-MOI des dossiers qui les citent : `bloc-44-assujettissement/`, `suspension-non-remuneree/`…).

## Les plafonds de la franchise en base de TVA (relevé du 21/10/2026, ticket #239)

`articles-20261021.md`, section « La taxe sur les salaires » : `xml/293_B.xml` (`LEGIARTI000052488142`, version en vigueur
depuis le 01/03/2025, modifiée par la loi n° 2025-127 du 14/02/2025, art. 32, et la loi n° 2025-1044 du 03/11/2025, art. 1),
à côté de `xml/231.xml`.

- **CGI art. 231, 1, 2e alinéa** : « Les rémunérations versées par les employeurs dont le chiffre d'affaires réalisé au cours
  de l'année civile précédant le versement de ces rémunérations n'excède pas les plafonds définis aux I et I bis de l'article
  293 B sont exonérées de la taxe sur les salaires. »
- **CGI art. 293 B, I** : la franchise vaut pour qui n'a pas réalisé un chiffre d'affaires « excédant les plafonds suivants »,
  ligne « Année civile précédente » : **85 000 €** de « chiffre d'affaires national total » et **37 500 €** de « chiffre
  d'affaires national afférent aux prestations de services autres que les ventes à consommer sur place et les prestations
  d'hébergement » (ligne « Année en cours » : 93 500 et 41 250 €, qui ne servent pas ici).
- **CGI art. 293 B, I bis** : les avocats, les auteurs d'œuvres de l'esprit et les artistes-interprètes, pour leurs propres
  opérations : 50 000 et 35 000 €.
- **Notice 2502-SD** (`formulaires/impots-2502-sd-2025-notice.pdf`, C, 1) : ce chiffre d'affaires « doit s'apprécier au regard
  de l'ensemble des recettes et autres produits, y compris ceux qui n'entrent pas dans le champ d'application de la TVA
  (notamment la perception de dividendes et subventions non imposables à la TVA) ».

**Inférences**, écrites comme telles :
- la colonne qui compte est « année civile précédente » : 231 compare le chiffre d'affaires de l'année d'avant le versement ;
- une compagnie vend des prestations de services (un spectacle cédé, une billetterie, un atelier) : son plafond est 37 500 €
  pour ces recettes, et 85 000 € pour l'ensemble, subventions comprises (la notice les compte dans le chiffre d'affaires ; elles
  ne sont pas des prestations, elles comptent dans le total) ;
- le I bis ne vise pas une compagnie employeur (elle n'est ni avocat, ni auteur, ni artiste-interprète) : il n'est pas repris ;
- **2025** : cette version de 293 B vaut depuis le 01/03/2025. Les salaires de janvier et février 2025 relevaient de la version
  précédente, qui n'est pas déposée : l'entrepôt LEGI ne garde que la version en vigueur, et le relevé dit seulement qu'elle a
  changé (`LEGIARTI000051215135` → `LEGIARTI000052488142`, livraison du 04/11/2025).

**Où le moteur s'en sert** : `logiciel/donnees/baremes/2025.json` et `2026.json`, `taxe_sur_les_salaires.plafonds_franchise`
(`prestations_de_services`, `total`, `en_vigueur_depuis`) ; `assujettissement::question_franchise`
(`atelier/sources/paie/src/assujettissement.rs`) en écrit l'aide et les deux réponses de la question « Vos recettes de l'an
dernier » (Configuration › Entreprise, « Taxe sur les salaires »), par la route `taxe-sur-les-salaires-regime` (`franchise`) ;
depuis le ticket #245, chaque réponse dit ce que mesure chaque plafond, en mots de compagnie (37 500 € de « spectacles
vendus », 85 000 € de « recettes en tout, subventions comprises » : les deux inférences ci-dessus), et la conclusion sous les
cases (`assujettissement::regime_taxe_sur_les_salaires`) nomme les deux plafonds.
Le test `les_plafonds_de_la_franchise_viennent_du_bareme_qui_cite_le_texte_depose` (`tests/routes/routes_fiches.rs`) relit les
montants dans `xml/293_B.xml`.

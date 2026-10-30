# Les taux de versement mobilité par commune

| fichier | ce que c'est | relevé |
|---|---|---|
| `datagouv-taux-versement-mobilite-par-commune-20261030.csv` (SHA-256 `cc63227d0530e41f662c216468eefc51fcc7f664087d50b2ee9552ec2bde17c6`) | « Taux de versement mobilité en vigueur par commune, avec la zone de versement et l'AOM », jeu de données de data.gouv.fr (https://www.data.gouv.fr/datasets/taux-de-versement-mobilite-en-vigueur-par-commune…), producteur Emet Expertise, **licence ODbL** ; il reprend le fichier de l'Urssaf (« fichier Urssaf modifié le 09/09/2026 », relevé du 22/09/2026), mis à jour sur data.gouv.fr le 29/10/2026 | transmis par Léandre le 30/10/2026 |

Données sous licence Open Database License (ODbL) : republiées ici telles quelles, avec leur source.

## Ce qu'on en lit

- Une ligne par commune où l'Urssaf publie un taux (20 284 communes) : `code_insee`, `commune`, `departement`, `taux_vm`
  (versement mobilité, en %), `taux_vma` (additionnel), `taux_vmr` (régional et rural), `aom` (autorité organisatrice de
  la mobilité), `zone_url`. Exemple : Nantes Métropole, 2 %.
- **Le taux en vigueur au 09/09/2026, sans date d'effet** : le fichier ne dit pas depuis quand un taux vaut, ni ce qu'il
  valait avant. Un taux qui change (souvent au 1er janvier ou au 1er juillet) demandera un relevé neuf, daté.

## Où le moteur s'en sert

Pas encore.

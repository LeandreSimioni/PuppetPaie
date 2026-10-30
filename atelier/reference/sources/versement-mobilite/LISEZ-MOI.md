# Les taux de versement mobilité par commune

| fichier | ce que c'est | relevé |
|---|---|---|
| `datagouv-taux-versement-mobilite-par-commune-20261030.csv` (SHA-256 `cc63227d0530e41f662c216468eefc51fcc7f664087d50b2ee9552ec2bde17c6`) | « Taux de versement mobilité en vigueur par commune, avec la zone de versement et l'AOM », jeu de données de data.gouv.fr (https://www.data.gouv.fr/datasets/taux-de-versement-mobilite-en-vigueur-par-commune…), producteur Emet Expertise, **licence ODbL** ; il reprend le fichier de l'Urssaf (« fichier Urssaf modifié le 09/09/2026 », relevé du 22/09/2026), mis à jour sur data.gouv.fr le 29/10/2026 | transmis par Léandre le 30/10/2026 |

Données sous licence Open Database License (ODbL) : republiées ici telles quelles, avec leur source.

## Ce qu'on en lit

- Une ligne par commune où l'Urssaf publie un taux (20 284 communes) : `code_insee`, `commune`, `departement`, `taux_vm`
  (versement mobilité, en %), `taux_vma` (additionnel), `taux_vmr` (régional et rural), `aom` (autorité organisatrice de
  la mobilité), `zone_url`. Exemple : Nantes Métropole, 2 %.
- **La source et ses dates** (page du jeu, `datagouv-page-jeu-versement-mobilite-20261030.md`) : le jeu est dérivé du fichier de l'Urssaf Caisse nationale « Table_Taux_VM_VMA_VMRR » (open.urssaf.fr, ODbL), « pour les taux, leurs dates d'effet et l'organisme percepteur » ; le dérivé ne garde que le taux **en vigueur au 09/09/2026, sans date d'effet** et sans historique. Pour lire un taux à la date de la paie, le fichier de l'Urssaf est à relever (open.urssaf.fr ne répond pas depuis le conteneur, data.gouv.fr non plus).

## Où le moteur s'en sert

Pas encore.

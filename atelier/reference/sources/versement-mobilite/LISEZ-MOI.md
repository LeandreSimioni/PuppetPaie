# Les taux de versement mobilité par commune

| fichier | ce que c'est | relevé |
|---|---|---|
| `urssaf-table-taux-vm-vma-vmrr-20261030.csv` (SHA-256 `5771b850d455efed86b5498e952d0ac2457c67fe2e8f89368240fac6cc5532df` ; UTF-8 avec BOM, fins de ligne CRLF, gardées telles quelles) | **la table de l'Urssaf Caisse nationale « Table_Taux_VM_VMA_VMRR »** (open.urssaf.fr, **licence ODbL**), exportée en CSV | transmis par Léandre le 30/10/2026 |
| `datagouv-taux-versement-mobilite-par-commune-20261030.csv` (SHA-256 `cc63227d0530e41f662c216468eefc51fcc7f664087d50b2ee9552ec2bde17c6`) | « Taux de versement mobilité en vigueur par commune, avec la zone de versement et l'AOM », jeu de data.gouv.fr (producteur Emet Expertise, **licence ODbL**), dérivé de la table de l'Urssaf (« fichier Urssaf modifié le 09/09/2026 », relevé du 22/09/2026) ; sa page : `datagouv-page-jeu-versement-mobilite-20261030.md` (et le PDF) | transmis par Léandre le 30/10/2026 |

Données sous licence Open Database License (ODbL) : republiées ici telles quelles, avec leur source.

## Ce qu'on en lit

- **La table de l'Urssaf fait foi.** Une ligne par commune et par période (69 330 lignes) : `code_commune`,
  `nom_commune`, `region`, `date_debut`, `date_fin` (vide : en vigueur ; 20 555 lignes), `taux_vm` (versement mobilité,
  en %), `taux_vma` (additionnel), `taux_vmr` (régional et rural), `code_partenaire_vm`, `code_partenaire_vma`,
  `code_partenaire_vmr` (l'organisme percepteur). Exemple : Nantes (44109) a ses taux depuis le 01/01/1992 (1,7 %), puis
  1,8 % du 01/01/2005 au 30/06/2011, etc.
- **La table de data.gouv.fr** n'en est qu'un dérivé : le seul taux en vigueur au 09/09/2026, sans date d'effet, avec en plus
  le nom de l'autorité organisatrice de la mobilité (`aom`), tiré de la liste du Cerema.

## Où le moteur s'en sert

Pas encore : le lot V de la séance 8 y lit le taux à la commune du lieu de travail et à la date de la paie.

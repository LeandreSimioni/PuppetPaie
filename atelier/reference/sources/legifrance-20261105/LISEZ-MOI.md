# Code du travail : pages de Légifrance envoyées par Léandre (05/11/2026)

Les PDF sont les pages de Légifrance imprimées par Léandre le 05/11/2026 ; chaque `.txt` est leur texte, extrait par
`pdftotext -layout`, sans retouche. Le relevé LEGI du dossier `code-du-travail/` (écrit par l'outil, qui ne se retouche
pas à la main) n'avait pas L3141-7, L1243-6, L1243-7, L1243-11 et L1243-12 ; les autres articles de ces sections y sont
aussi.

| Fichier | Articles | Où le moteur s'en sert |
|---|---|---|
| `legifrance-code-travail-L3141-7-20261105` | L3141-7 (version du 10/08/2016) : une durée de congé qui n'est pas un nombre entier « est portée au nombre entier immédiatement supérieur » | `logiciel/donnees/permanents-base.json`, `conges_payes.acquisition.arrondi` |
| `legifrance-code-travail-L1221-19-a-L1221-26-20261105` | la période d'essai ; L1221-25 : le délai de prévenance, et son dernier alinéa, l'indemnité compensatrice « indemnité compensatrice de congés payés comprise » | `engagement/src/solde.rs` (rupture pendant l'essai, délai de prévenance non effectué) ; `permanents-base.json`, `preavis.delai_de_prevenance` |
| `legifrance-code-travail-L1243-1-a-L1243-4-20261105` | la rupture anticipée du CDD ; L1243-1 : la rupture « avant l'échéance du terme », pour la faute grave, la force majeure, l'inaptitude ou l'accord des parties (la version déposée ne nomme pas la période d'essai) | `permanents-base.json`, `indemnite_precarite.motifs_rupture_exclus["034"]` |
| `legifrance-code-travail-L1243-5-a-L1243-12-20261105` | l'échéance du terme ; L1243-8 : l'indemnité de fin de contrat « à l'issue » du contrat ; L1243-10 : les cas où elle n'est pas due | `permanents-base.json`, `indemnite_precarite` |

**Une lecture, pas un texte** : qu'une rupture de CDD pendant l'essai n'ouvre pas l'indemnité de précarité se déduit de
L1243-1 (la rupture « avant l'échéance du terme » n'y est encadrée que pour la faute grave, la force majeure, l'inaptitude ou l'accord des parties ; l'essai est celui du contrat, L1242-10) et de L1243-8 (l'indemnité
n'est due qu'« à l'issue » du contrat) ; L1243-10 ne nomme pas ce cas. Aucun arrêt déposé ne la confirme.

## Envoyés ensuite, le même jour

| Fichier | Articles | Où le moteur s'en sert |
|---|---|---|
| `legifrance-code-travail-L1234-1-a-L1234-8-20261105` | préavis et indemnité compensatrice de préavis. **Page imprimée dans sa « version en vigueur au 01 mai 2008 »** : L1234-1 à L1234-7 n'ont pas changé depuis ; L1234-8 y est la version close le 24/12/2025 (la version actuelle n'est pas déposée ; le moteur ne cite pas L1234-8). L1234-7 : « La cessation de l'entreprise ne libère pas l'employeur de l'obligation de respecter le préavis. » | `permanents-base.json`, `preavis.indemnite_compensatrice.motifs_a_trancher["065"]` |
| `legifrance-code-travail-L3142-79-a-L3142-88-20261105` | congés des salariés candidats ou élus à un mandat parlementaire ou local, version en vigueur au 05/11/2026 (L3142-83 à L3142-88 depuis le 24/12/2025) | `permanents-base.json`, `conges_payes` : le 8° de L3141-5 (mandat électif local) y renvoie |
| `legifrance-code-travail-L1234-11-20261105` | L1234-11, version en vigueur depuis le 24/12/2025 : une suspension du contrat ne rompt pas l'ancienneté du droit à l'indemnité de licenciement, mais n'entre pas dans la durée exigée (sauf les élus locaux de L3142-88) | aucun encore : envoyé pour la question de l'ancienneté au terme d'un préavis non effectué, qu'il ne tranche pas (il parle des suspensions) |

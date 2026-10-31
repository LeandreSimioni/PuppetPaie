# Le salaire d'un mois incomplet (ticket #315 du privé)

| fichier | ce que c'est | relevé |
|---|---|---|
| `legifrance-cass-soc-19990120-96-45042-20261030.pdf` (et sa transcription `.md`) | Cass. soc., 20/01/1999, n° 96-45.042, inédit | par Léandre, sur Légifrance, le 30/10/2026 |

Lu aussi, sans le republier : l'article des Éditions Lamy Liaisons « Jurisprudence : calcul par rapport à l'horaire réel »
(publié le 21/02/2025, mis à jour le 26/03/2025). L'original, d'un éditeur privé, est dans le dépôt privé,
`reference-privee/sources/mois-incomplet/`. Il ne fonde rien : il recense les arrêts.

## Ce qu'on en lit

- **L'arrêt** : une salariée a quitté l'entreprise le 25 février ; le conseil de prud'hommes lui avait accordé « le 25/30e
  de son salaire de base ». La Cour casse : « il lui appartenait de rechercher l'horaire de travail dans l'entreprise et de
  déterminer le salaire dû à Mme X... en multipliant la rémunération horaire par le nombre d'heures de travail réellement
  effectuées ». La rémunération horaire est « le quotient du salaire mensuel par le nombre d'heures de travail dans
  l'entreprise pour le mois considéré ». Visa : « l'article 2 de la loi n° 78-49 du 10 janvier 1978 relative à la mensualisation et à la procédure conventionnelle » (date telle que l'arrêt l'écrit).
- **Donc, pour un mois incomplet** (entrée, sortie, changement de salaire en cours de mois) : salaire mensuel ÷ heures du mois
  dans l'entreprise × heures réellement travaillées par le salarié. Ni le trentième, ni les jours calendaires.
- L'article Lamy cite l'arrêt de principe, publié au Bulletin : Cass. soc., 24/06/1992, n° 89-41.387 (3/30e cassé, même
  motif). **Il n'est pas déposé** (#315).
- Une convention peut dire autrement : la CCN 1285, art. X.3, pour le premier ou le dernier mois incomplet d'un artiste
  (1/21 par jour ouvré du contrat, `../kali-1285-x3-remuneration-artistes-20260929.md`).

## Où le moteur s'en sert

- `droit::periodes::part_du_mois` : la part du salaire mensuel qu'une période payée porte (une entrée, une sortie, un
  avenant au jour, l'indemnité du délai de prévenance, la retenue d'une absence) ; le quotient des heures de la période
  sur celles du mois, comptées sur les **jours travaillés** du salarié, jours fériés compris
  (`droit::conges::JoursTravailles` : la case « Jours travaillés » de sa fiche, du lundi au vendredi tant qu'elle n'en dit
  pas d'autres ; fiche DSN 2693, exemple 4, dans `../suspension-non-remuneree/`). Un férié chômé qu'il ne voit pas payé
  (moins de trois mois d'ancienneté, L3133-3 ; jamais le 1er mai, L3133-5 : `../code-du-travail/`,
  `droit::conges::feries_non_payes`) reste une heure du mois et n'est pas une heure payée.
- `droit::periodes::regle_du_mois_incomplet` : la CCN 1285, art. X.3, pour le premier ou le dernier mois d'un artiste.
  L'article ne borne pas le résultat au salaire mensuel ; le moteur ne le borne pas, et l'aperçu le dit.
- Ce qui l'emploie : la ligne « mois » de `engagement::conges::lignes_du_mois`, les périodes d'un mois coupé par un
  avenant (`engagement::periodes::engagements_des_periodes`), les absences (`engagement::imprevus::lignes_du_mois`),
  l'indemnité du délai de prévenance (`engagement::solde`) ; l'horaire d'un salarié pour un mois :
  `engagement::conges::horaire_du_mois`. La ligne du bulletin dit la part (« 105 h sur les 154 h du mois »), l'aperçu
  cite l'arrêt.
- Le plafond de la sécurité sociale d'un mois incomplet reste aux jours calendaires (R. 242-2) : ce n'est pas le salaire.

# Audiens et la DSN : à qui l'on paie, et quand (relevé du 20/10/2026, ticket #223)

## Ce qui est déposé ici

| fichier | ce que c'est | URL |
|---|---|---|
| `audiens-dossier-dsn-20261020.md` | transcription de la section « Payer les cotisations Audiens Santé Prévoyance » du dossier DSN d'Audiens | https://www.audiens.org/solutions/entreprise-dossier-dsn-declaration-sociale-nominative.html |
| `audiens-declarer-payer-conges-spectacles-20261020.md` | transcription du haut de la page « Déclarer et payer les cotisations Congés Spectacles » | https://www.audiens.org/solutions/declarer-et-payer-les-cotisations-conges-spectacles.html |

Audiens est un organisme privé : les pages telles que servies (HTML) sont dans le dépôt privé,
`reference-privee/sources/audiens-dsn/`, avec leurs empreintes données dans chaque transcription.

## Ce qu'on en lit

- **Les trois identifiants de versement (S21.G00.20.001) d'Audiens** : « Pour la retraite Audiens > 44832375800038 »,
  « Pour les Congés Spectacles > 77567608300030 », « Pour Audiens Santé Prévoyance > P0983 ». La table IVO des
  nomenclatures DSN dit la même chose (`../dsn-nomenclatures/tables-organismes-20260924.txt`, l. 480 « AUDIENS » AAR,
  l. 492 « Caisse des Congés spectacles » AUD, l. 625 « AUDIENS PREVOYANCE » CTI ; table PREV, l. 32). **P0983 est
  donc Audiens Santé Prévoyance, pas l'AFDAS** : le libellé « Formation professionnelle (AFDAS) » de
  `logiciel/donnees/organismes-base.json` était faux, et le code 059 qu'il porte est « Cotisation individuelle
  Prévoyance-Assurance-Mutuelle » (cahier technique 2026.1, `norme-dsn/texte/bloc-S21.G00.81.txt`, l. 228). L'AFDAS
  n'est pas un organisme de versement de la DSN : c'est l'OPCO (table OPCO, code 02, l. 3), et la contribution à la
  formation professionnelle part à l'Urssaf par ses CTP (`organismes-base.json`, `ctp_urssaf` : le 983 des
  intermittents, et la CFP des permanents dont la note cite le guide Urssaf des contributions de formation).
- **L'échéance** : « L'échéance de paiement pour Audiens est pour un mois M, le 25 du mois M+1. Cette date est
  automatiquement retenue pour les paiements retraite et Congés Spectacles mais doit être renseignée dans le bloc
  paiement à destination d'Audiens Santé Prévoyance (P0983). » Trimestrielle (« entreprises de – 11 salariés ») : le 25
  du mois qui suit le dernier mois du trimestre. Et pour les Congés Spectacles : « Les périodicités d'appel des
  cotisations sont identiques à celles de la retraite et de la prévoyance. »

## Où le moteur s'en sert

- `logiciel/donnees/organismes-base.json` : le nom de chaque organisme (`nom`), ce que couvre son versement (`objet`)
  et, pour les trois identifiants d'Audiens, `jour_de_paiement` = 25.
- `atelier/sources/paie/src/dsn.rs` : la date de paiement (S21.G00.20.011) du versement à P0983 est le 25 du mois qui
  suit le mois déclaré (elle était la fin du mois suivant le trimestre).
- `atelier/sources/paie/src/ordres_paiement.rs` : la boîte de dépôt de la DSN dit, pour la retraite et les Congés
  Spectacles d'Audiens, « au plus tard le 25 » du mois suivant.

## Ce qui reste ouvert

L'échéance trimestrielle (moins de onze salariés, sur option : la fiche de paramétrage d'Audiens d'une compagnie dit
« Periodicite="Mensuel" ») n'est pas modélisée : le moteur verse chaque mois. Une compagnie au trimestre ne verserait
qu'à la dernière DSN du trimestre (FAQ DSN des organismes complémentaires, `../faq-dsn-organismes-complementaires/`).

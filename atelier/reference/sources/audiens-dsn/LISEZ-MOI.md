# Audiens et la DSN : à qui l'on paie, et quand (relevé du 20/10/2026, ticket #223)

## Ce qui est déposé ici

| fichier | ce que c'est | URL |
|---|---|---|
| `audiens-dossier-dsn-20261020.md` | transcription de la section « Payer les cotisations Audiens Santé Prévoyance » du dossier DSN d'Audiens | https://www.audiens.org/solutions/entreprise-dossier-dsn-declaration-sociale-nominative.html |
| `audiens-declarer-payer-conges-spectacles-20261020.md` | transcription du haut de la page « Déclarer et payer les cotisations Congés Spectacles » | https://www.audiens.org/solutions/declarer-et-payer-les-cotisations-conges-spectacles.html |
| `audiens-fiche-specificites-conges-spectacles-20261023.txt` | transcription (`pdftotext -layout`) de la « Fiche de spécificités — Paramétrage DSN Congés Spectacles » (PDF du 16/02/2023), relevée le 23/10/2026 (Léandre l'a transmise de nouveau le 30/10/2026 : même empreinte) | https://www.audiens.org/files/live/sites/siteAudiens/files/03_documents/entreprise/DSN/DSN-Fiche-specificites-Conges-spectacles.pdf |
| `audiens-conges-spectacles-mode-emploi-employeur-20261030.txt` | transcription (`pdftotext -layout`) de la brochure « Congés Spectacles employeur — Mode d'emploi » (12 pages, avril 2023), transmise par Léandre le 30/10/2026 | non transmise |

Audiens est un organisme privé : les pages telles que servies (HTML) sont dans le dépôt privé,
`reference-privee/sources/audiens-dsn/`, avec leurs empreintes données dans chaque transcription.

## Ce qu'on en lit

- **Le code Congés Spectacles de S21.G00.40.022 se déduit du numéro d'adhérent** (fiche de spécificités, l. 80-81) :
  « 97 puis le numéro d'adhérent à la caisse Congés spectacles (10 chiffres + 1 lettre). Ces 2 identifiants doivent
  être accolés, sans espaces, ni tiret, etc… EX : 970000000001A ». La norme dit la même forme (« Indiquer "97", le
  numéro d'affiliation et la clé », `../../norme-dsn/texte/bloc-S21.G00.40.txt`, l. 921-933) : le « numéro
  d'affiliation et la clé » de la norme est donc le numéro d'adhérent d'Audiens (dix chiffres et une lettre). Une
  compagnie ne saisit que son numéro d'adhérent ; le moteur écrit `97` + ce numéro.

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

- **Les trois numéros des Congés Spectacles** (mode d'emploi employeur, avril 2023, `audiens-conges-spectacles-mode-emploi-employeur-20261030.txt`) :
  le **numéro d'affiliation de l'entreprise est son SIRET** (« Numéro d'affiliation ») ; le **numéro d'adhérent**
  que la DSN écrit après `97` est celui de la fiche de spécificités (dix chiffres et une lettre) ; le **numéro
  d'immatriculation du salarié** « est formé d'une lettre suivie de six chiffres », et sa mention sur le certificat
  d'emploi « est facultative ». L'employeur immatricule lui-même un salarié de nationalité étrangère.
- **Le certificat et la cotisation** (même brochure) : le salaire du certificat est le brut « avant toute déduction
  forfaitaire spécifique », plafonnable au triple du minimum conventionnel de la catégorie ; cotisation patronale de
  15,50 % « à compter des paies d'avril 2023 » (D. 7121-44) ; période de référence du 1er avril au 31 mars, deux
  certificats pour un contrat en cours au 31 mars ; les activités de formateur ne se déclarent pas ; un arrêt pour
  accident du travail se déclare jusqu'à la fin prévue du contrat.

## Où le moteur s'en sert

- `atelier/sources/documents/src/conges_spectacles.rs` : `code_dsn` écrit S21.G00.40.022 = `97` + le numéro d'adhérent
  (`congesspectacles_adherent`, le seul numéro Congés Spectacles de la fiche ; `dsn.rs` l'appelle), `faute_adherent`
  contrôle sa forme (dix chiffres et une lettre) à la saisie (`champs::verifier_champ`) comme à l'écriture de la DSN ;
  `moteur/src/import/dsn_aem.rs` reprend d'une DSN le numéro d'adhérent qui suit le `97`.

- `logiciel/donnees/champs-base.json`, `num_conges_spectacles` : le numéro d'immatriculation du salarié, facultatif,
  imprimé sur le bulletin (`documents/src/bulletin.rs`).
- `logiciel/donnees/organismes-base.json` : le nom de chaque organisme (`nom`), ce que couvre son versement (`objet`)
  et, pour les trois identifiants d'Audiens, `jour_de_paiement` = 25.
- `atelier/sources/declarations/src/dsn.rs` : la date de paiement (S21.G00.20.011) du versement à P0983 est le 25 du mois qui
  suit le mois déclaré (elle était la fin du mois suivant le trimestre).
- `atelier/sources/declarations/src/ordres_paiement.rs` : la boîte de dépôt de la DSN dit, pour la retraite et les Congés
  Spectacles d'Audiens, « au plus tard le 25 » du mois suivant.

## Ce qui reste ouvert

L'échéance trimestrielle (moins de onze salariés, sur option : la fiche de paramétrage d'Audiens d'une compagnie dit
« Periodicite="Mensuel" ») n'est pas modélisée : le moteur verse chaque mois. Une compagnie au trimestre ne verserait
qu'à la dernière DSN du trimestre (FAQ DSN des organismes complémentaires, `../faq-dsn-organismes-complementaires/`).

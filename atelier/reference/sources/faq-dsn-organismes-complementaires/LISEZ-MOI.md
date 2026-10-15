# La FAQ DSN des organismes complémentaires (CTIP, FFA, FNMF), pour les déclarants

## D'où ça vient

`net-entreprises-faq-dsn-oc-declarants-v4-202310.pdf` : « DSN Organismes Complémentaires (prévoyance, complémentaire
santé, retraite supplémentaire) — FAQ », **version 4.0, octobre 2023**, 58 pages, rédigée par les représentants des
organismes complémentaires (CTIP, FFA, FNMF). URL donnée par le document lui-même (p. 3) :
`https://www.net-entreprises.fr/media/documentation/faq-dsn-oc-declarants.pdf`. **Transmise par Léandre le 11/10/2026.**
Empreinte SHA-256 (16 premiers caractères) : `d3b84578cb40e78c`. Transcription : `pdftotext -layout`, `.txt` à côté.

Antérieure aux rubriques `70.014`/`70.015` (dates d'affiliation) : elle ne les connaît pas. **D3 reste ouvert.**

## Ce qu'on en lit (lignes du `.txt`)

- **Le bloc 70, ce qui est obligatoire** (l. 599-614) : `70.013` (l'adhésion du bloc 15) et `70.012` (le lien vers les
  bases 78) ; conditionnels, « selon les paramètres de calcul de la cotisation et les informations portées par
  l'organisme dans sa fiche de paramétrage » : `70.004` (option), `70.005` (population), `70.008` à `.011` (ayants
  droit) « si elles interviennent dans le calcul de la cotisation ». C'est ce que `paie/src/dsn.rs` fait (V3e, sur le
  cahier technique p. 287) : la FAQ le confirme.
- **L'option `70.004`** (l. 619-660) : « seules les options dont les cotisations sont précomptées sur salaire doivent
  être renseignées » ; pas de liste de valeurs, « les valeurs de code sont fournies […] par vos organismes
  complémentaires […] dans les fiches de paramétrage ». **Q14 et D14** (les codes d'option sont-ils ceux du contrat de
  branche ?) : la FAQ dit que c'est la fiche de paramétrage qui les donne, rien de plus.
- **Un changement d'option en cours de mois** (l. 670-681) : aucune structure ne le notifie ; la DSN du mois porte le
  nouveau code, l'organisme le voit par différence entre deux mois.
- **La dispense** (l. 548-560) : « pour un salarié ne faisant pas l'objet d'une affiliation sur un contrat, **aucun bloc
  S21.G00.70 n'est à renseigner** » ; l'organisme ne reçoit que les salariés qui ont un bloc 70. **Q13** (faut-il des
  dates à la dispense ?) : pour la DSN, non ; la dispense est une absence d'affiliation. Le motif et les dates sont
  une affaire de pièce justificative entre l'employeur et le salarié, pas de déclaration.
- **Affilié sans cotisation sur le mois** (l. 688-700) : le bloc 70 reste, avec un 78 de code **31** et un 81 de
  montant **0.00** (sinon l'organisme engagerait une radiation). Un permanent en absence totale non payée : à porter.
- **Les ayants droit** (l. 973-1010 et § 7.4) : `70.008`/`70.011` (adultes, enfants) ou `70.009` (tous), « y compris à
  0 », seulement quand le forfait en dépend ; **le bloc 73 est facultatif et ne cause jamais de rejet** ; pas de
  signalement de changement de situation familiale, on répercute dans la DSN du mois. **Q14** (une base 79 type 18 ou
  trois ?) : la FAQ parle des nombres du bloc 70, pas du découpage du 79 ; elle ne tranche pas.
- Carence et franchise (l. 562-595) : les blocs 70, 78, 79, 81 se déclarent quand même.
- Le reste (fiche de paramétrage FPOC § 3, codification des contrats § 5.1, radiation § 5.2, bulletin d'affiliation
  § 5.3, cotisations § 7, établissement § 8, paiement § 10, retraite supplémentaire § 11, CRM § 12) : non lu ce jour.

## Où le moteur s'en sert

Nulle part encore (11/10/2026, séance « documents »). Les commentaires de `paie/src/dsn.rs` (bloc 70, l. 879-890 et
943-946) citent le cahier technique ; ils pourront citer cette FAQ pour la dispense (aucun bloc 70) et l'affilié sans
cotisation (78 code 31, 81 à 0.00).

## Pièce secondaire : la fiche technique GHS (sPAIEctacle) « Contrôle du paramétrage OC »

`ghs-spaiectacle-fiche-technique-controle-parametrage-oc-20191014.pdf` (+ `.txt`) : fiche technique de l'éditeur GHS pour
son logiciel sPAIEctacle, « EM 14.10.19 » (PDF du 14/06/2021), 7 pages, **transmise par Léandre le 11/10/2026** (origine :
site GHS, URL non notée). SHA-256 (16) : `793315c53f5b0748`. **Un éditeur, pas un organisme** : on n'en tire aucune règle,
seulement la lecture d'une fiche de paramétrage Audiens (p. 4) :
- « Certains organismes (dont Audiens) affectent à chaque tableau un entête Groupe » = la catégorie de salariés ; « la
  mention ETAM […] correspond aux Non Cadres » ; la référence contrat en gras (« INTERBRANCHE » cadres intermittents,
  « PRV EAC » cadres permanents) ; « quand il n'est pas précisé INTERMITTENTS, la ligne concerne les permanents » ;
- « Type de base ou forfait » : **18 = base forfaitaire > plafond mensuel (prorata temporis pour Audiens)**, 20 = montant
  fixe ; 10 à 14 = salaire abattu (brut, TA/T1U, T2U, TB, TC) ;
- « Montant et/ou taux » = le taux global, « sans précision sur la répartition » ;
- p. 1 : « Audiens ne fait pas remonter ces anomalies » (références contrat, options) : une DSN « OK » peut porter une
  incohérence avec la fiche ; p. 6 : « les options de mutuelle Audiens ne peuvent pas être regroupées avec le socle en
  une seule retenue » : **une ligne de fiche = une retenue** (Q14 : le socle et l'option sont deux lignes, donc deux
  cotisations, inférence depuis une consigne d'éditeur).

## Dans le dépôt public (15/10/2026)

Ce dossier est dans le dépôt public de PuppetPaie. Les originaux suivants, d'un organisme privé, n'y sont pas : seule leur transcription y est (`pdftotext -layout` pour un PDF, une feuille de calcul en CSV, feuille par feuille, le texte d'une page HTML), et c'est elle que citent les renvois. Les condensats donnés plus haut sont ceux des originaux, gardés dans le dépôt privé (`reference-privee/sources/faq-dsn-organismes-complementaires/`).

- `ghs-spaiectacle-fiche-technique-controle-parametrage-oc-20191014.pdf` : transcription `ghs-spaiectacle-fiche-technique-controle-parametrage-oc-20191014.txt`

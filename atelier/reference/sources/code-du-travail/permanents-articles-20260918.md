# Code du travail — ce qui fait la paie d'un permanent

Relevé le **18/09/2026** sur Légifrance, article par article, dans la version
« en vigueur au 18/09/2026 ». Chaque article porte son identifiant Légifrance
(`LEGIARTI…`) : c'est lui qui permet de retrouver exactement cette version,
`https://www.legifrance.gouv.fr/codes/article_lc/<identifiant>`.

Ces textes nourrissent `moteur/permanents-base.json`. **Légifrance refuse les
robots** (403 sur un téléchargement automatique) : cette copie ne se revérifie
pas en ligne comme les autres sources, elle se relit à la main — comme les deux
autres relevés Légifrance du dossier (`legifrance-1285-…`, `legifrance-3090-…`).

À relire quand une loi de finances ou une loi travail touche l'un de ces
articles ; la veille KALI ne couvre que les conventions collectives, pas le
code.

---

## Congés payés

### Acquisition — L3141-3 (`LEGIARTI000033020826`, en vigueur depuis le 10/08/2016)

> Le salarié a droit à un congé de deux jours et demi ouvrables par mois de
> travail effectif chez le même employeur.
>
> La durée totale du congé exigible ne peut excéder trente jours ouvrables.

### Période de référence — L3141-10 (`LEGIARTI000033020790`) et R3141-4 (`LEGIARTI000033453122`)

L3141-10 : un accord d'entreprise ou, à défaut, une convention ou un accord de
branche peut fixer le début de la période de référence, et majorer la durée du
congé en raison de l'âge, de l'ancienneté ou du handicap.

R3141-4 (en vigueur depuis le 01/01/2017) :

> À défaut d'accord prévu à l'article L. 3141-10, le point de départ de la
> période prise en compte pour le calcul du droit au congé est fixé au 1er juin
> de chaque année.
> Toutefois, dans les professions où en application de l'article L. 3141-32
> l'employeur est tenu de s'affilier à une caisse de congé, le point de départ
> de l'année de référence est fixé au 1er avril.

**Ce que ça veut dire pour une compagnie de spectacle.** Le 1er avril est la
période de la *caisse* — les Congés Spectacles, où vont les intermittents.
Un permanent n'y est pas affilié : ses congés sont payés par l'employeur, donc
sa période de référence commence le **1er juin**. Les deux dates cohabitent
dans la même compagnie, et c'est normal.

### Indemnité — L3141-24 (`LEGIARTI000049461568`, en vigueur depuis le 24/04/2024)

> I. — Le congé annuel prévu à l'article L. 3141-3 ouvre droit à une indemnité
> égale au dixième de la rémunération brute totale perçue par le salarié au
> cours de la période de référence.
>
> Pour la détermination de la rémunération brute totale, il est tenu compte :
> 1° De l'indemnité de congé de l'année précédente ;
> 2° Des indemnités afférentes à la contrepartie obligatoire sous forme de
> repos prévues aux articles L. 3121-30, L. 3121-33 et L. 3121-38 ;
> 3° Des périodes assimilées à un temps de travail par l'article L. 3141-4 et
> par les 1° à 6° de l'article L. 3141-5 […] ;
> 4° Des périodes assimilées à un temps de travail par le 7° du même article
> L. 3141-5 […] dans la limite d'une prise en compte à 80 % de la rémunération
> associée à ces périodes.
>
> Lorsque la durée du congé est différente de celle prévue à l'article
> L. 3141-3, l'indemnité est calculée selon les règles fixées au présent I et
> proportionnellement à la durée du congé effectivement dû.
>
> II. — Toutefois, l'indemnité prévue au I du présent article **ne peut être
> inférieure au montant de la rémunération qui aurait été perçue pendant la
> période de congé si le salarié avait continué à travailler**.
>
> Cette rémunération […] est calculée en fonction :
> 1° Du salaire gagné dû pour la période précédant le congé ;
> 2° De la durée du travail effectif de l'établissement.

C'est la **règle des deux calculs** : un dixième de la rémunération de la
période de référence, ou le maintien du salaire — le plus favorable des deux,
et c'est le maintien qui fait le plancher.

Le 4° (80 % de la rémunération des périodes d'arrêt maladie non
professionnelle) vient de la loi n° 2024-364 du 22/04/2024, qui a mis le droit
français en conformité avec le droit européen.

### Indemnité compensatrice — L3141-28 (`LEGIARTI000033020681`)

> Lorsque le contrat de travail est rompu avant que le salarié ait pu
> bénéficier de la totalité du congé auquel il avait droit, il reçoit, pour la
> fraction de congé dont il n'a pas bénéficié, une indemnité compensatrice de
> congé déterminée d'après les articles L. 3141-24 à L. 3141-27.
>
> L'indemnité est due **que cette rupture résulte du fait du salarié ou du fait
> de l'employeur**.

---

## Fin d'un CDD : l'indemnité de précarité

### Le principe — L1243-8 (`LEGIARTI000006901219`, en vigueur depuis le 01/05/2008)

> Lorsque, à l'issue d'un contrat de travail à durée déterminée, les relations
> contractuelles de travail ne se poursuivent pas par un contrat à durée
> indéterminée, le salarié a droit, à titre de complément de salaire, à une
> indemnité de fin de contrat destinée à compenser la précarité de sa
> situation.
>
> Cette indemnité est égale à **10 % de la rémunération totale brute versée au
> salarié**.
>
> Elle s'ajoute à la rémunération totale brute due au salarié. Elle est versée
> à l'issue du contrat en même temps que le dernier salaire et figure sur le
> bulletin de salaire correspondant.

### Le taux réduit — L1243-9 (en vigueur depuis le 01/01/2019)

Un accord de branche étendu ou un accord d'entreprise peut limiter l'indemnité
à **6 %** en contrepartie d'un accès privilégié à la formation professionnelle.
*(Non applicable tant qu'aucun accord de la compagnie ne le prévoit.)*

### Quand elle n'est pas due — L1243-10 (`LEGIARTI000006901221`)

> L'indemnité de fin de contrat n'est pas due :
> 1° Lorsque le contrat est conclu au titre du **3° de l'article L. 1242-2** ou
> de l'article L. 1242-3, sauf dispositions conventionnelles plus favorables ;
> 2° Lorsque le contrat est conclu avec un jeune pour une période comprise dans
> ses vacances scolaires ou universitaires ;
> 3° Lorsque le salarié refuse d'accepter la conclusion d'un contrat de travail
> à durée indéterminée pour occuper le même emploi ou un emploi similaire,
> assorti d'une rémunération au moins équivalente ;
> 4° En cas de rupture anticipée du contrat due à l'initiative du salarié, à sa
> faute grave ou à un cas de force majeure.

**Le 1° est le CDD d'usage** (L. 1242-2 3° : « emplois pour lesquels […] il est
d'usage constant de ne pas recourir au contrat à durée indéterminée »). C'est
la raison pour laquelle aucun des 146 contrats de l'historique ne porte
d'indemnité de précarité, et c'est écrit noir sur blanc : un intermittent n'y a
pas droit.

---

## Fin d'un CDI

### Préavis — L1234-1 (`LEGIARTI000006901112`, en vigueur depuis le 01/05/2008)

> Lorsque le licenciement n'est pas motivé par une faute grave, le salarié a
> droit :
> 1° S'il justifie chez le même employeur d'une ancienneté de services continus
> **inférieure à six mois**, à un préavis dont la durée est déterminée par la
> loi, la convention ou l'accord collectif de travail ou, à défaut, par les
> usages pratiqués dans la localité et la profession ;
> 2° S'il justifie […] d'une ancienneté comprise **entre six mois et moins de
> deux ans**, à un préavis d'**un mois** ;
> 3° S'il justifie […] d'une ancienneté **d'au moins deux ans**, à un préavis de
> **deux mois**.
>
> Toutefois, les dispositions des 2° et 3° ne sont applicables que si la loi, la
> convention ou l'accord collectif de travail, le contrat de travail ou les
> usages ne prévoient pas un préavis ou une condition d'ancienneté de services
> plus favorable pour le salarié.

### Indemnité compensatrice de préavis — L1234-5

> Lorsque le salarié n'exécute pas le préavis, il a droit, sauf s'il a commis
> une faute grave, à une indemnité compensatrice.
>
> L'inexécution du préavis, notamment en cas de dispense par l'employeur,
> n'entraîne aucune diminution des salaires et avantages que le salarié aurait
> perçus s'il avait accompli son travail jusqu'à l'expiration du préavis,
> **indemnité de congés payés comprise**.

### Droit à l'indemnité de licenciement — L1234-9 (`LEGIARTI000035644154`, depuis le 24/09/2017)

> Le salarié titulaire d'un contrat de travail à durée indéterminée, licencié
> alors qu'il compte **8 mois d'ancienneté ininterrompus** au service du même
> employeur, a droit, **sauf en cas de faute grave**, à une indemnité de
> licenciement.

### Son montant — R1234-1 et R1234-2 (depuis le 27/09/2017)

R1234-1 :

> L'indemnité de licenciement prévue à l'article L. 1234-9 ne peut être
> inférieure à une somme calculée par année de service dans l'entreprise et
> tenant compte des mois de service accomplis au-delà des années pleines. En
> cas d'année incomplète, l'indemnité est calculée proportionnellement au
> nombre de mois complets.

R1234-2 :

> L'indemnité de licenciement ne peut être inférieure aux montants suivants :
> 1° **Un quart de mois** de salaire par année d'ancienneté pour les années
> jusqu'à dix ans ;
> 2° **Un tiers de mois** de salaire par année d'ancienneté pour les années à
> partir de dix ans.

### Le salaire de référence — R1234-4 (`LEGIARTI000035644695`, depuis le 27/09/2017)

> Le salaire à prendre en considération pour le calcul de l'indemnité de
> licenciement est, **selon la formule la plus avantageuse pour le salarié** :
> 1° Soit la moyenne mensuelle des **douze derniers mois** précédant le
> licenciement, ou lorsque la durée de service du salarié est inférieure à
> douze mois, la moyenne mensuelle de la rémunération de l'ensemble des mois
> précédant le licenciement ;
> 2° Soit le **tiers des trois derniers mois**. Dans ce cas, toute prime ou
> gratification de caractère annuel ou exceptionnel, versée au salarié pendant
> cette période, n'est prise en compte que dans la limite d'un montant calculé
> à due proportion.

---

## Ce qui n'est PAS dans ce relevé, et qu'il faudra relever

- **L3141-4 et L3141-5** : les périodes assimilées à du travail effectif pour
  l'acquisition des congés (congés payés de l'année précédente, congé
  maternité, arrêt pour accident du travail dans la limite d'un an, et depuis
  la loi du 22/04/2024 l'arrêt maladie non professionnelle à 2 jours ouvrables
  par mois). Tant qu'ils ne sont pas relevés, le moteur compte l'acquisition
  sur le **temps de présence**, et le dit.
- **Les indemnités de rupture conventionnelle** (L1237-11 et suivants), la
  **mise à la retraite** et le **départ à la retraite** : aucun n'est codé.
- **La convention collective 3090** : ses clauses sur le préavis, l'indemnité
  de licenciement et les congés supplémentaires peuvent être plus favorables
  que la loi, et c'est alors elle qui s'applique. Les grilles de salaire sont
  relevées (`moteur/minima-conventionnels.json`), pas ces clauses-là.

# Code du travail -- extrait de l'entrepot LEGI (releve du 15/10/2026, recherche suspension non remuneree)

| | |
|---|---|
| source | base LEGI de la DILA, open data (`https://echanges.dila.gouv.fr/OPENDATA/LEGI/`), par l'entrepot local `atelier/reference/sources/legi-entrepot/` (`articles.jsonl.gz`, mis a jour le 2026-10-15, voir `etat.json`) |
| texte | Code du travail (`LEGITEXT000006072050`) |
| comment | **extrait par un script de lecture de l'entrepot, PAS par `paie releve-codes`** : le moteur Linux livre (`paie/bin/linux/paie`) n'a pas cette commande (elle est sous `--features atelier`), et ces articles ne sont pas encore dans `paie/src/releve_codes/textes.rs`. Chaque article est celui de la version `en_vigueur` de l'entrepot (`<ETAT>` VIGUEUR, `<DATE_FIN>` 2999-01-01), texte de la balise `BLOC_TEXTUEL/CONTENU`, mise en forme retiree. A ajouter a `textes.rs` pour que le prochain `moissonner` les suive. |

---

### L1225-47 (`LEGIARTI000047293352`, en vigueur depuis le 2023-03-11, <ETAT> VIGUEUR, <DATE_FIN> 2999-01-01)

> Pendant la période qui suit l'expiration du congé de maternité ou d'adoption, tout salarié justifiant d'une ancienneté minimale d'une année a le droit :
> 1° Soit au bénéfice d'un congé parental d'éducation durant lequel le contrat de travail est suspendu ;
> 2° Soit à la réduction de sa durée de travail, sans que cette activité à temps partiel puisse être inférieure à seize heures hebdomadaires.

### L1225-48 (`LEGIARTI000029336639`, en vigueur depuis le 2014-08-06, <ETAT> VIGUEUR, <DATE_FIN> 2999-01-01)

> Le congé parental d'éducation et la période d'activité à temps partiel ont une durée initiale d'un an au plus. Ils peuvent être prolongés deux fois pour prendre fin au plus tard au terme des périodes définies aux deuxième et quatrième alinéas, quelle que soit la date de leur début.
> Le congé parental d'éducation et la période d'activité à temps partiel prennent fin au plus tard au troisième anniversaire de l'enfant.
> En cas de naissances multiples, le congé parental d'éducation peut être prolongé jusqu'à l'entrée à l'école maternelle des enfants. Pour les naissances multiples d'au moins trois enfants ou les arrivées simultanées d'au moins trois enfants adoptés ou confiés en vue d'adoption, il peut être prolongé cinq fois pour prendre fin au plus tard au sixième anniversaire des enfants.
> En cas d'adoption d'un enfant de moins de trois ans, le congé parental et la période d'activité à temps partiel prennent fin à l'expiration d'un délai de trois ans à compter de l'arrivée au foyer de l'enfant.
> Lorsque l'enfant adopté ou confié en vue de son adoption est âgé de plus de trois ans mais n'a pas encore atteint l'âge de la fin de l'obligation scolaire, le congé parental et la période d'activité à temps partiel ne peuvent excéder une année à compter de l'arrivée au foyer.

### L1225-49 (`LEGIARTI000006900934`, en vigueur depuis le 2008-05-01, <ETAT> VIGUEUR, <DATE_FIN> 2999-01-01)

> En cas de maladie, d'accident ou de handicap graves de l'enfant appréciés selon des modalités définies par décret en Conseil d'Etat, le congé parental et la période d'activité à temps partiel prennent fin au plus tard une année après les dates limites définies à l'article L. 1225-48.

### L1225-50 (`LEGIARTI000006900935`, en vigueur depuis le 2008-05-01, <ETAT> VIGUEUR, <DATE_FIN> 2999-01-01)

> Le salarié informe son employeur du point de départ et de la durée de la période pendant laquelle il entend bénéficier soit d'un congé parental d'éducation, soit d'une réduction de sa durée du travail.
> Lorsque cette période suit immédiatement le congé de maternité ou le congé d'adoption, le salarié informe l'employeur au moins un mois avant le terme de ce congé. Dans le cas contraire, l'information est donnée à l'employeur deux mois au moins avant le début du congé parental d'éducation ou de l'activité à temps partiel.

### L1225-51 (`LEGIARTI000006900936`, en vigueur depuis le 2008-05-01, <ETAT> VIGUEUR, <DATE_FIN> 2999-01-01)

> Lorsque le salarié entend prolonger ou modifier son congé parental d'éducation ou sa période d'activité à temps partiel, il en avertit l'employeur au moins un mois avant le terme initialement prévu et l'informe de son intention soit de transformer le congé parental en activité à temps partiel, soit de transformer l'activité à temps partiel en congé parental.
> Toutefois, pendant la période d'activité à temps partiel ou à l'occasion des prolongations de celle-ci, le salarié ne peut pas modifier la durée du travail initialement choisie sauf accord de l'employeur ou lorsqu'une convention ou un accord collectif de travail le prévoit expressément.

### L1225-52 (`LEGIARTI000006900937`, en vigueur depuis le 2008-05-01, <ETAT> VIGUEUR, <DATE_FIN> 2999-01-01)

> En cas de décès de l'enfant ou de diminution importante des ressources du foyer, le salarié a le droit :
> 1° S'il bénéficie du congé parental d'éducation, soit de reprendre son activité initiale, soit d'exercer son activité à temps partiel dans la limite de la durée initialement prévue par le contrat de travail ;
> 2° S'il travaille à temps partiel pour élever un enfant, de reprendre son activité initiale et, avec l'accord de l'employeur, d'en modifier la durée.
> Le salarié adresse une demande motivée à l'employeur un mois au moins avant la date à laquelle il entend bénéficier de ces dispositions.

### L1225-53 (`LEGIARTI000006900938`, en vigueur depuis le 2008-05-01, <ETAT> VIGUEUR, <DATE_FIN> 2999-01-01)

> Le salarié en congé parental d'éducation ou qui travaille à temps partiel pour élever un enfant ne peut exercer par ailleurs aucune activité professionnelle autre que les activités d'assistance maternelle définies par le titre II du livre IV du code de l'action sociale et des familles.

### L1225-54 (`LEGIARTI000047293349`, en vigueur depuis le 2023-03-11, <ETAT> VIGUEUR, <DATE_FIN> 2999-01-01)

> La durée du congé parental d'éducation à temps plein est prise en compte pour moitié pour la détermination des droits que le salarié tient de son l'ancienneté.
> Lorsqu'un salarié réduit son temps de travail dans le cadre d'un congé parental, la durée du congé parental d'éducation à temps partiel est assimilée à une période de travail effectif pour la détermination des droits que le salarié tient de son ancienneté.
> Le salarié conserve le bénéfice de tous les avantages qu'il avait acquis avant le début du congé.

### L1225-55 (`LEGIARTI000006900940`, en vigueur depuis le 2008-05-01, <ETAT> VIGUEUR, <DATE_FIN> 2999-01-01)

> A l'issue du congé parental d'éducation ou de la période de travail à temps partiel ou dans le mois qui suit la demande motivée de reprise de l'activité initiale mentionnée à l'article L. 1225-52, le salarié retrouve son précédent emploi ou un emploi similaire assorti d'une rémunération au moins équivalente.

### L1225-56 (`LEGIARTI000037385637`, en vigueur depuis le 2019-01-01, <ETAT> VIGUEUR, <DATE_FIN> 2999-01-01)

> Au cours du congé parental d'éducation ou d'une période d'activité à temps partiel pour élever un enfant, le salarié a le droit de suivre, à son initiative, un bilan de compétences.
> Pendant cette période, il n'est pas rémunéré.
> Il bénéficie de la législation de sécurité sociale relative à la protection en matière d'accidents du travail et de maladies professionnelles prévue à l'article L. 6342-5 pour les stagiaires de la formation professionnelle.

### L1225-57 (`LEGIARTI000029336723`, en vigueur depuis le 2014-08-06, <ETAT> VIGUEUR, <DATE_FIN> 2999-01-01)

> Le salarié qui reprend son activité initiale à l'issue du congé parental d'éducation ou d'une période d'activité à temps partiel pour élever un enfant a droit à l'entretien professionnel mentionné au I de l'article L. 6315-1.
> Au cours de cet entretien, l'employeur et le salarié organisent le retour à l'emploi du salarié ; ils déterminent les besoins de formation du salarié et examinent les conséquences éventuelles du congé sur sa rémunération et l'évolution de sa carrière.
> A la demande du salarié, l'entretien peut avoir lieu avant la fin du congé parental d'éducation.

### L1225-58 (`LEGIARTI000006900945`, en vigueur depuis le 2008-05-01, <ETAT> VIGUEUR, <DATE_FIN> 2999-01-01)

> Le salarié bénéficiant d'un congé parental d'éducation ou exerçant son activité à temps partiel pour élever un enfant bénéficie de plein droit du bilan de compétences mentionné à l'article L. 6313-1, dans les conditions d'ancienneté mentionnées à l'article L. 1225-47.

### L1225-59 (`LEGIARTI000006900946`, en vigueur depuis le 2008-05-01, <ETAT> VIGUEUR, <DATE_FIN> 2999-01-01)

> Le salarié reprenant son activité initiale bénéficie d'un droit à une action de formation professionnelle, notamment en cas de changement de techniques ou de méthodes de travail.
> Le salarié peut également bénéficier de ce droit avant l'expiration de la période pendant laquelle il entendait bénéficier d'un congé parental d'éducation ou d'une période d'activité à temps partiel. Dans ce cas, il est mis fin au congé parental d'éducation ou à l'exercice d'une activité à temps partiel pour élever un enfant.

### L3142-28 (`LEGIARTI000038952020`, en vigueur depuis le 2019-08-23, <ETAT> VIGUEUR, <DATE_FIN> 2999-01-01)

> Le salarié a droit à un congé sabbatique pendant lequel son contrat de travail est suspendu.
> Le droit à ce congé est ouvert au salarié justifiant, à la date de départ en congé, d'une ancienneté minimale dans l'entreprise, cumulée, le cas échéant, sur plusieurs périodes non consécutives, ainsi que de six années d'activité professionnelle et n'ayant pas bénéficié depuis une durée minimale, dans la même entreprise, d'un congé sabbatique, d'un congé pour création d'entreprise ou d'un congé spécifique mentionné à l'article L. 6323-17-1 d'une durée d'au moins six mois. L'ancienneté acquise dans toute autre entreprise du même groupe, au sens de l'article L. 2331-1, est prise en compte au titre de l'ancienneté dans l'entreprise.

### L3142-29 (`LEGIARTI000033022115`, en vigueur depuis le 2016-08-10, <ETAT> VIGUEUR, <DATE_FIN> 2999-01-01)

> L'employeur peut différer le départ en congé dans la limite de six mois à compter de la demande, en fonction de la proportion de salariés absents dans l'entreprise au titre du congé ou en fonction du nombre de jours d'absence prévus au titre du même congé. Dans les entreprises de moins de trois cents salariés, cette limite est portée à neuf mois.
> L'employeur peut également différer ce congé sur le fondement de l'article L. 3142-114 et, pour les entreprises de moins de trois cents salariés, le refuser sur le fondement du 1° de l'article L. 3142-113 selon les modalités prévues aux deux derniers alinéas du même article L. 3142-113.

### L3142-30 (`LEGIARTI000036262899`, en vigueur depuis le 2017-12-22, <ETAT> VIGUEUR, <DATE_FIN> 2999-01-01)

> L'employeur informe le salarié soit de son accord sur la date de départ choisie par l'intéressé, soit du report de cette date, soit de son refus.
> L'accord de l'employeur est réputé acquis à défaut de réponse dans un délai de trente jours à compter de la présentation de la demande.

### L3142-31 (`LEGIARTI000033022109`, en vigueur depuis le 2016-08-10, <ETAT> VIGUEUR, <DATE_FIN> 2999-01-01)

> A l'issue du congé, le salarié retrouve son précédent emploi ou un emploi similaire assorti d'une rémunération au moins équivalente et bénéficie de l'entretien professionnel mentionné au I de l'article L. 6315-1.
> Il ne peut invoquer aucun droit à être réemployé avant l'expiration du congé.

### L3142-32 (`LEGIARTI000033022105`, en vigueur depuis le 2016-08-10, <ETAT> VIGUEUR, <DATE_FIN> 2999-01-01)

> Pour mettre en œuvre le droit à congé du salarié mentionné à l'article L. 3142-28, une convention ou un accord collectif d'entreprise ou, à défaut, une convention ou un accord de branche détermine :
> 1° Les durées minimale et maximale du congé et le nombre de renouvellements ;
> 2° La condition d'ancienneté requise dans l'entreprise pour ouvrir droit à ce congé ;
> 3° La durée minimale dans l'entreprise durant laquelle le salarié ne doit pas avoir bénéficié des dispositifs mentionnés au second alinéa de l'article L. 3142-28 ;
> 4° Les plafonds mentionnés aux articles L. 3142-29, L. 3142-114 et L. 3142-115 ;
> 5° Les conditions et délais d'information de l'employeur par le salarié sur sa demande de congé ainsi que sur la date de son départ et sur la durée envisagée de ce congé.

### L3142-33 (`LEGIARTI000033022101`, en vigueur depuis le 2016-08-10, <ETAT> VIGUEUR, <DATE_FIN> 2999-01-01)

> Cette convention ou cet accord détermine également les modalités de report des congés payés dus au salarié qui bénéficie du congé.

### L3142-34 (`LEGIARTI000033022097`, en vigueur depuis le 2016-08-10, <ETAT> VIGUEUR, <DATE_FIN> 2999-01-01)

> A défaut de convention ou d'accord mentionné à l'article L. 3142-32, les dispositions suivantes sont applicables :
> 1° La durée minimale du congé est de six mois et sa durée maximale est de onze mois ;
> 2° Le droit à ce congé est ouvert au salarié justifiant, à la date de départ en congé, d'une ancienneté dans l'entreprise d'au moins trente-six mois, consécutifs ou non, et n'ayant pas bénéficié dans l'entreprise, au cours des six années précédentes, des dispositifs mentionnés au second alinéa de l'article L. 3142-28 ;
> 3° Les conditions et délais mentionnés au 5° de l'article L. 3142-32 sont fixés par décret ;
> 4° Les plafonds mentionnés à l'article L. 3142-29 sont fixés par décret.

### L3142-105 (`LEGIARTI000033022292`, en vigueur depuis le 2016-08-10, <ETAT> VIGUEUR, <DATE_FIN> 2999-01-01)

> Le salarié qui crée ou reprend une entreprise a droit, sous réserve d'une condition d'ancienneté dans l'entreprise et dans les conditions fixées à la présente section :
> 1° Soit à un congé ;
> 2° Soit à une période de travail à temps partiel.
> L'ancienneté acquise dans toute autre entreprise du même groupe, au sens de l'article L. 2331-1, est prise en compte au titre de l'ancienneté dans l'entreprise.

### L3142-106 (`LEGIARTI000033022288`, en vigueur depuis le 2016-08-10, <ETAT> VIGUEUR, <DATE_FIN> 2999-01-01)

> L'article L. 3142-105 s'applique également au salarié qui exerce des responsabilités de direction au sein d'une entreprise répondant, au moment où il sollicite son congé, aux critères de jeune entreprise innovante définie à l'article 44 sexies-0 A du code général des impôts.

### L3142-107 (`LEGIARTI000033022281`, en vigueur depuis le 2016-08-10, <ETAT> VIGUEUR, <DATE_FIN> 2999-01-01)

> L'employeur peut différer le départ en congé ou le début de la période de travail à temps partiel, dans la limite de six mois à compter de la demande du salarié, sans préjudice de l'application des articles L. 3142-113 et L. 3142-114.

### L3142-108 (`LEGIARTI000033021923`, en vigueur depuis le 2016-08-10, <ETAT> VIGUEUR, <DATE_FIN> 2999-01-01)

> A l'issue du congé, le salarié retrouve son précédent emploi ou un emploi similaire assorti d'une rémunération au moins équivalente.

### L3142-109 (`LEGIARTI000033022279`, en vigueur depuis le 2016-08-10, <ETAT> VIGUEUR, <DATE_FIN> 2999-01-01)

> A l'issue du congé ou de la période de travail à temps partiel, si le salarié souhaite mettre fin à la relation de travail, les conditions de la rupture sont celles prévues par son contrat de travail, à l'exception de celles relatives au préavis. Le salarié est, de ce fait, dispensé de payer une indemnité de rupture.
> Le salarié ne peut invoquer aucun droit à être réemployé avant l'expiration du congé.

### L3242-1 (`LEGIARTI000006902858`, en vigueur depuis le 2008-05-01, <ETAT> VIGUEUR, <DATE_FIN> 2999-01-01)

> La rémunération des salariés est mensuelle et indépendante, pour un horaire de travail effectif déterminé, du nombre de jours travaillés dans le mois. Le paiement mensuel neutralise les conséquences de la répartition inégale des jours entre les douze mois de l'année.
> Pour un horaire équivalent à la durée légale hebdomadaire, la rémunération mensuelle due au salarié se calcule en multipliant la rémunération horaire par les 52/12 de la durée légale hebdomadaire.
> Le paiement de la rémunération est effectué une fois par mois. Un acompte correspondant, pour une quinzaine, à la moitié de la rémunération mensuelle, est versé au salarié qui en fait la demande.
> Ces dispositions ne s'appliquent pas aux salariés travaillant à domicile, aux salariés saisonniers, aux salariés intermittents et aux salariés temporaires.

### L3242-2 (`LEGIARTI000006902859`, en vigueur depuis le 2008-05-01, <ETAT> VIGUEUR, <DATE_FIN> 2999-01-01)

> La mensualisation n'exclut pas les divers modes de calcul du salaire aux pièces, à la prime ou au rendement.

### L3242-3 (`LEGIARTI000006902860`, en vigueur depuis le 2008-05-01, <ETAT> VIGUEUR, <DATE_FIN> 2999-01-01)

> Les salariés ne bénéficiant pas de la mensualisation sont payés au moins deux fois par mois, à seize jours au plus d'intervalle.

### L3251-1 (`LEGIARTI000006902873`, en vigueur depuis le 2008-05-01, <ETAT> VIGUEUR, <DATE_FIN> 2999-01-01)

> L'employeur ne peut opérer une retenue de salaire pour compenser des sommes qui lui seraient dues par un salarié pour fournitures diverses, quelle qu'en soit la nature.

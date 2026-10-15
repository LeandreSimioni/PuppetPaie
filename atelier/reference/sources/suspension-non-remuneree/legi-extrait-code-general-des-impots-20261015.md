# Code general des impots -- extrait de l'entrepot LEGI (releve du 15/10/2026, recherche suspension non remuneree)

| | |
|---|---|
| source | base LEGI de la DILA, open data (`https://echanges.dila.gouv.fr/OPENDATA/LEGI/`), par l'entrepot local `atelier/reference/sources/legi-entrepot/` (`articles.jsonl.gz`, mis a jour le 2026-10-15, voir `etat.json`) |
| texte | Code general des impots (`LEGITEXT000006069577`) |
| comment | **extrait par un script de lecture de l'entrepot, PAS par `paie releve-codes`** : le moteur Linux livre (`paie/bin/linux/paie`) n'a pas cette commande (elle est sous `--features atelier`), et ces articles ne sont pas encore dans `paie/src/releve_codes/textes.rs`. Chaque article est celui de la version `en_vigueur` de l'entrepot (`<ETAT>` VIGUEUR, `<DATE_FIN>` 2999-01-01), texte de la balise `BLOC_TEXTUEL/CONTENU`, mise en forme retiree. A ajouter a `textes.rs` pour que le prochain `moissonner` les suive. |

---

### 83 (`LEGIARTI000054373766`, en vigueur depuis le 2026-07-01, <ETAT> VIGUEUR, <DATE_FIN> 2999-01-01)

> Le montant net du revenu imposable est déterminé en déduisant du montant brut des sommes payées et des avantages en argent ou en nature accordés :
> 1° Les cotisations de sécurité sociale, y compris :
> a) Les cotisations d'assurance vieillesse versées en exercice des facultés de rachat prévues aux articles L. 351-14-1 du code de la sécurité sociale, L. 9 bis du code des pensions civiles et militaires de retraite, ainsi que celles prévues par des dispositions réglementaires ayant le même objet prises sur le fondement de l'article L. 711-1 du code de la sécurité sociale ;
> b) Les cotisations versées aux régimes de retraite complémentaire mentionnés au chapitre Ier du titre II du livre IX du code de la sécurité sociale ;
> c) Les cotisations au régime public de retraite additionnel obligatoire institué par l'article 76 de la loi n° 2003-775 du 21 août 2003 portant réforme des retraites ;
> 1° 0 bis Les cotisations versées conformément aux dispositions du règlement CEE n° 1408/71 du Conseil du 14 juin 1971 relatif à l'application des régimes de sécurité sociale aux travailleurs salariés et aux membres de leur famille qui se déplacent à l'intérieur de la Communauté ou conformément aux stipulations d'une convention ou d'un accord international relatif à l'application des régimes de sécurité sociale ;
> 1° bis (Abrogé)
> 1° ter (Abrogé à compter de l'entrée en vigueur de la loi n° 2002-73 du 17 janvier 2002).
> 1° quater Les cotisations ou primes versées aux régimes de prévoyance complémentaire obligatoires et collectifs, au sens du sixième alinéa de l'article L. 242-1 du code de la sécurité sociale, à condition, lorsque ces cotisations ou primes financent des garanties portant sur le remboursement ou l'indemnisation de frais occasionnés par une maladie, une maternité ou un accident, que ces garanties respectent les conditions mentionnées à l'article L. 871-1 du code de la sécurité sociale. Il en va de même des cotisations ou primes versées par les employeurs publics et leurs agents aux contrats collectifs de protection sociale complémentaire pour lesquels la souscription des agents est rendue obligatoire en application d'un accord prévu à l'article L. 827-2 du code général de la fonction publique ou en application de l'arrêté mentionné au II de l'article L. 4123-3 du code de la défense.
> Les cotisations ou les primes mentionnées au premier alinéa s'entendent, s'agissant des cotisations à la charge de l'employeur, de celles correspondant à des garanties autres que celles portant sur le remboursement ou l'indemnisation de frais occasionnés par une maladie, une maternité ou un accident.
> Les cotisations à la charge de l'employeur correspondant à des garanties portant sur le remboursement ou l'indemnisation de frais occasionnés par une maladie, une maternité ou un accident sont ajoutées à la rémunération prise en compte pour la détermination des bases d'imposition.
> Les cotisations ou les primes déductibles en application des premier et deuxième alinéas le sont dans la limite d'un montant égal à la somme de 5 % du montant annuel du plafond mentionné à l'article L. 241-3 du code de la sécurité sociale et de 2 % de la rémunération annuelle brute, sans que le total ainsi obtenu puisse excéder 2 % de huit fois le montant annuel du plafond précité. En cas d'excédent, celui-ci est ajouté à la rémunération.
> 2° Les cotisations ou primes versées aux régimes de retraite supplémentaire, y compris ceux gérés par une institution mentionnée à l'article L. 370-1 du code des assurances pour les contrats mentionnés à l'article L. 143-1 dudit code lorsqu'ils ont pour objet l'acquisition et la jouissance de droits viagers, auxquels le salarié est affilié à titre obligatoire, les cotisations versées, à compter du 1er janvier 1993, à titre obligatoire au régime de prévoyance des joueurs professionnels de football institué par la charte du football professionnel ainsi que les cotisations versées, à compter du 1er janvier 2026, par les joueurs professionnels de rugby au titre du pécule de reconversion institué par la convention collective du rugby professionnel.
> Il en va de même des versements mentionnés au 3° de l'article L. 224-2 du code monétaire et financier dans un plan d'épargne retraite mentionné à l'article L. 224-13 ou à l'article L. 224-23 du même code.
> Les versements mentionnés aux alinéas précédents sont déductibles dans la limite, y compris les versements de l'employeur, de 8 % de la rémunération annuelle brute retenue à concurrence de huit fois le montant annuel du plafond mentionné à l'article L. 241-3 du code de la sécurité sociale. En cas d'excédent, celui-ci est ajouté à la rémunération ;
> La limite mentionnée au troisième alinéa est réduite, le cas échéant, des sommes versées aux plans d'épargne retraite qui sont exonérées en application du 18° de l'article 81 ;
> 2°-0 bis (Abrogé) ;
> 2°-0 ter Dans les limites prévues au quatrième alinéa du 1° quater, les cotisations versées aux régimes de prévoyance complémentaire et, dans les limites prévues aux troisième et dernier alinéas du 2°, les cotisations versées aux régimes de retraite complémentaire répondant aux conditions fixées à l'article 3 de la directive 98/49/ CE du Conseil du 29 juin 1998 relative à la sauvegarde des droits à pension complémentaire des travailleurs salariés et non salariés qui se déplacent à l'intérieur de la Communauté ou à celles prévues par les conventions ou accords internationaux de sécurité sociale, auxquels les personnes désignées au 1 du I de l'article 155 B étaient affiliées ès qualités dans un autre Etat avant leur prise de fonctions en France. Les cotisations sont déductibles jusqu'au 31 décembre de la huitième année suivant celle de leur prise de fonctions ;
> 2°-0 quater La contribution prévue à l'article L. 137-11-1 du code de la sécurité sociale dans la limite de la fraction acquittée au titre des premiers 1 000 € de rente mensuelle ;
> 2° bis Les contributions versées par les salariés en application des dispositions de l'article L. 5422-9 du code du travail et destinées à financer le régime d'assurance des travailleurs privés d'emploi ;
> 2° ter (Abrogé).
> 2° quater (Abrogé).
> 2° quinquies (Abrogé).
> 3° Les frais inhérents à la fonction ou à l'emploi lorsqu'ils ne sont pas couverts par des allocations spéciales.
> La déduction à effectuer du chef des frais professionnels est calculée forfaitairement en fonction du revenu brut, après défalcation des cotisations, contributions et intérêts mentionnés aux 1° à 2° ter ; elle est fixée à 10 % du montant de ce revenu. Elle est limitée à 14 555 € pour l'imposition des rémunérations perçues en 2025 ; chaque année, le plafond retenu pour l'imposition des revenus de l'année précédente est relevé dans la même proportion que la limite supérieure de la première tranche du barème de l'impôt sur le revenu.
> Le montant de la déduction forfaitaire pour frais professionnels ne peut être inférieur à 509 €, sans pouvoir excéder le montant brut des traitements et salaires. Cette disposition s'applique séparément aux rémunérations perçues par chaque membre du foyer fiscal désigné aux 1 et 3 de l'article 6.
> La somme figurant au troisième alinéa est révisée chaque année dans la même proportion que la limite supérieure de la première tranche du barème de l'impôt sur le revenu.
> Les bénéficiaires de traitements et salaires sont également admis à justifier du montant de leurs frais réels, soit dans la déclaration visée à l'article 170, soit sous forme de réclamation adressée au service des impôts dans le délai prévu aux articles R* 196-1 et R* 196-3 du livre des procédures fiscales. Le montant des frais réels à prendre en compte au titre de l'acquisition des immeubles, des véhicules et autres biens dont la durée d'utilisation est supérieure à un an s'entend de la dépréciation que ces biens ont subie au cours de l'année d'imposition.
> Sont assimilées à des frais professionnels réels les dépenses exposées en vue de l'obtention d'un diplôme ou d'une qualification professionnelle dans la perspective d'une insertion ou d'une conversion professionnelle par les personnes tirant un revenu de la pratique d'un sport.
> Les frais de déplacement de moins de quarante kilomètres entre le domicile et le lieu de travail sont admis, sur justificatifs, au titre des frais professionnels réels. Lorsque la distance est supérieure, la déduction admise porte sur les quarante premiers kilomètres, sauf circonstances particulières notamment liées à l'emploi justifiant une prise en compte complète. Les frais de déplacement mentionnés au présent alinéa engagés par un passager au titre du partage des frais dans le cadre d'un covoiturage défini à l'article L. 3132-1 du code des transports sont admis, sur justificatifs, au titre des frais professionnels réels.
> Lorsque les bénéficiaires de traitements et salaires optent pour le régime des frais réels, l'évaluation des frais de déplacement, autres que les frais de péage, de garage ou de parking et d'intérêts annuels afférents à l'achat à crédit du véhicule utilisé, peut s'effectuer sur le fondement d'un barème forfaitaire fixé par arrêté du ministre chargé du budget en fonction de la puissance administrative du véhicule, retenue dans la limite maximale de sept chevaux, du type de motorisation du véhicule, et de la distance annuelle parcourue.
> Lorsque les bénéficiaires mentionnés au huitième alinéa ne font pas application dudit barème, les frais réels déductibles, autres que les frais de péage, de garage ou de parking et d'intérêts annuels afférents à l'achat à crédit du véhicule utilisé, ne peuvent excéder le montant qui serait admis en déduction en application du barème précité, à distance parcourue identique, pour un véhicule de la puissance administrative maximale retenue par le barème.
> Les frais, droits et intérêts d'emprunt versés pour acquérir ou souscrire des parts ou des actions d'une société ayant une activité industrielle, commerciale, artisanale, agricole ou libérale dans laquelle le salarié ou le dirigeant exerce son activité professionnelle principale sont admis, sur justificatifs, au titre des frais professionnels réels dès lors que ces dépenses sont utiles à l'acquisition ou à la conservation de ses revenus. Les intérêts admis en déduction sont ceux qui correspondent à la part de l'emprunt dont le montant est proportionné à la rémunération annuelle perçue ou escomptée au moment où l'emprunt est contracté. La rémunération prise en compte s'entend des revenus mentionnés à l'article 79 et imposés sur le fondement de cet article. La fraction des versements effectués au titre des souscriptions ou acquisitions de titres donnant lieu aux réductions d'impôt prévues aux articles 199 terdecies-0A, 199 terdecies-0 AB ou 199 terdecies-0 B, ainsi que les souscriptions et acquisitions de titres figurant dans un plan d'épargne en actions défini à l'article 163 quinquies D ou dans un plan d'épargne salariale prévu au titre III du livre III de la troisième partie du code du travail, ne peuvent donner lieu à aucune déduction d'intérêts d'emprunt.

### 204 A (`LEGIARTI000033812067`, en vigueur depuis le 2019-01-01, <ETAT> VIGUEUR, <DATE_FIN> 2999-01-01)

> 1. Les revenus imposables à l'impôt sur le revenu suivant les règles applicables aux salaires, aux pensions ou aux rentes viagères ou dans les catégories des bénéfices industriels et commerciaux, des bénéfices agricoles, des bénéfices non commerciaux et des revenus fonciers, à l'exception des revenus mentionnés à l'article 204 D, donnent lieu, l'année au cours de laquelle le contribuable en a la disposition ou de leur réalisation, à un prélèvement.
> 2. Le prélèvement prend la forme :
> 1° Pour les revenus mentionnés à l'article 204 B, d'une retenue à la source effectuée par le débiteur lors du paiement de ces revenus ;
> 2° Pour les revenus mentionnés à l'article 204 C, d'un acompte acquitté par le contribuable.
> 3. Le prélèvement effectué par le débiteur ou acquitté par le contribuable s'impute sur l'impôt sur le revenu dû par ce dernier au titre de l'année au cours de laquelle il a été effectué. S'il excède l'impôt dû, l'excédent est restitué.

### 204 B (`LEGIARTI000046860893`, en vigueur depuis le 2023-01-01, <ETAT> VIGUEUR, <DATE_FIN> 2999-01-01)

> Sous réserve des dérogations prévues à l'article 204 C, donnent lieu à l'application de la retenue à la source prévue au 1° du 2 de l'article 204 A les revenus soumis à l'impôt sur le revenu suivant les règles applicables aux salaires, aux pensions ou aux rentes viagères à titre gratuit.

### 204 E (`LEGIARTI000048806579`, en vigueur depuis le 2025-09-01, <ETAT> VIGUEUR, <DATE_FIN> 2999-01-01)

> Le prélèvement prévu à l'article 204 A est calculé en appliquant au montant des revenus, déterminé dans les conditions prévues aux articles 204 F et 204 G, un taux selon les modalités prévues aux articles 204 H et 204 I.
> Le prélèvement peut être modifié sur demande du contribuable dans les conditions prévues à l'article 204 J.
> Le taux du prélèvement pour les conjoints ou partenaires liés par un pacte civil de solidarité soumis à imposition commune est, sauf option contraire du contribuable, individualisé dans les conditions prévues à l'article 204 M.

### 204 F (`LEGIARTI000036354271`, en vigueur depuis le 2019-01-01, <ETAT> VIGUEUR, <DATE_FIN> 2999-01-01)

> L'assiette de la retenue à la source prévue au 1° du 2 de l'article 204 A sur les revenus mentionnés à l'article 204 B est constituée du montant net imposable à l'impôt sur le revenu des sommes versées et des avantages accordés, avant application de l'article 80 sexies, de la deuxième phrase du premier alinéa du 1° de l'article 81, du 3° de l'article 83 et des deuxième et dernier alinéas du a du 5 de l'article 158.

### 204 H (`LEGIARTI000054373568`, en vigueur depuis le 2026-07-01, <ETAT> VIGUEUR, <DATE_FIN> 2999-01-01)

> I. – 1. L'administration fiscale calcule pour chaque foyer fiscal le taux prévu à l'article 204 E. Il est égal au rapport entre le montant de l'impôt sur le revenu du foyer fiscal afférent aux revenus mentionnés au 1 de l'article 204 A, sous déduction des crédits d'impôt correspondant à ces revenus prévus par les conventions fiscales internationales, et ces mêmes revenus pour leurs montants déterminés dans les conditions mentionnées à l'article 204 F et à l'article 204 G, à l'exception des 6° et 7° du 2 et du 4 du même article 204 G.
> Pour le calcul du premier terme du numérateur, l'impôt sur le revenu résultant de l'application des règles prévues aux 1 à 4 du I de l'article 197 ou, le cas échéant, à l'article 197 A est multiplié par le rapport entre les montants nets imposables des revenus mentionnés au 1 de l'article 204 A, les déficits étant retenus pour une valeur nulle, et le revenu net imposable au barème progressif de l'impôt sur le revenu, hors déficits, charges et abattements déductibles du revenu global.
> 2. L'impôt sur le revenu et les revenus pris en compte mentionnés au 1 sont ceux de l'avant-dernière année pour le calcul du taux relatif aux versements de l'acompte acquittés et aux retenues à la source effectuées entre le 1er janvier et le 31 août de l'année au cours de laquelle le contribuable dispose des revenus ou réalise les bénéfices, et ceux de l'année précédente pour le calcul du taux relatif aux versements de l'acompte acquittés et aux retenues à la source effectuées entre le 1er septembre et le 31 décembre.
> Toutefois, dans le cas où l'impôt sur le revenu de l'avant-dernière année ou de la dernière année n'a pu être établi, l'impôt sur le revenu et les revenus pris en compte pour le calcul du taux sont ceux de la dernière année pour laquelle l'impôt a été établi à la date du calcul de l'acompte par l'administration ou de la transmission du taux au débiteur des revenus en application du 4, sans que cette année ne puisse être antérieure à l'antépénultième année par rapport à l'année de prélèvement.
> 3. Le taux est arrondi à la décimale la plus proche. La fraction de décimale égale à 0,50 est comptée pour un.
> 4. Le taux, assorti des calculs qui l'ont déterminé, est communiqué au contribuable par l'administration fiscale. Celle-ci transmet le taux au débiteur mentionné au 1° du 2 de l'article 204 A.
> II. – Par dérogation au I, le taux prévu à l'article 204 E est nul pour les contribuables qui remplissent cumulativement les deux conditions suivantes :
> 1° L'impôt sur le revenu, avant imputation du prélèvement prévu à l'article 204 A, mis en recouvrement au titre des revenus des deux dernières années d'imposition connues est nul ;
> 2° Le montant des revenus, au sens du 1° du IV de l'article 1417, de la dernière année d'imposition connue est inférieur à 29 310 € par part de quotient familial.
> Pour l'appréciation de la condition prévue au 1° du présent II, les crédits d'impôt prévus au A et au 3 du E du II de l'article 60 de la loi n° 2016-1917 du 29 décembre 2016 de finances pour 2017 ne sont pas pris en compte.
> Le montant des revenus prévu au 2° du présent II est indexé chaque année comme la limite supérieure de la première tranche du barème de l'impôt sur le revenu.
> III. – 1. Lorsque le débiteur ne dispose pas d'un taux calculé par l'administration fiscale ou lorsque l'année dont les revenus ont servi de base au calcul du taux est antérieure à l'antépénultième année par rapport à l'année de prélèvement, il est appliqué un taux proportionnel fixé dans les conditions suivantes :
> a) Pour les contribuables autres que ceux mentionnés aux b et c du présent 1 :
> Base mensuelle de prélèvement
> Taux proportionnel
> Inférieure à 1 635 €
> 0 %
> Supérieure ou égale à 1 635 € et inférieure à 1 698 €
> 0,5 %
> Supérieure ou égale à 1 698 € et inférieure à 1 807 €
> 1,3 %
> Supérieure ou égale à 1 807 € et inférieure à 1 928 €
> 2,1 %
> Supérieure ou égale à 1 928 € et inférieure à 2 060 €
> 2,9 %
> Supérieure ou égale à 2 060 € et inférieure à 2 170 €
> 3,5 %
> Supérieure ou égale à 2 170 € et inférieure à 2 315 €
> 4,1 %
> Supérieure ou égale à 2 315 € et inférieure à 2 738 €
> 5,3 %
> Supérieure ou égale à 2 738 € et inférieure à 3 135 €
> 7,5 %
> Supérieure ou égale à 3 135 € et inférieure à 3 571 €
> 9,9 %
> Supérieure ou égale à 3 571 € et inférieure à 4 019 €
> 11,9 %
> Supérieure ou égale à 4 019 € et inférieure à 4 690 €
> 13,8 %
> Supérieure ou égale à 4 690 € et inférieure à 5 624 €
> 15,8 %
> Supérieure ou égale à 5 624 € et inférieure à 7 037 €
> 17,9 %
> Supérieure ou égale à 7 037 € et inférieure à 8 789 €
> 20 %
> Supérieure ou égale à 8 789 € et inférieure à 12 200 €
> 24 %
> Supérieure ou égale à 12 200 € et inférieure à 16 523 €
> 28 %
> Supérieure ou égale à 16 523 € et inférieure à 25 937 €
> 33 %
> Supérieure ou égale à 25 937 € et inférieure à 55 558 €
> 38 %
> Supérieure ou égale à 55 558 €
> 43 %
> b) Pour les contribuables domiciliés en Guadeloupe, à La Réunion et en Martinique :
> Base mensuelle de prélèvement
> Taux proportionnel
> Inférieure à 1 875 €
> 0 %
> Supérieure ou égale à 1 875 € et inférieure à 1 989 €
> 0,5 %
> Supérieure ou égale à 1 989 € et inférieure à 2 191 €
> 1,3 %
> Supérieure ou égale à 2 191 € et inférieure à 2 392 €
> 2,1 %
> Supérieure ou égale à 2 392 € et inférieure à 2 642 €
> 2,9 %
> Supérieure ou égale à 2 642 € et inférieure à 2 786 €
> 3,5 %
> Supérieure ou égale à 2 786 € et inférieure à 2 881 €
> 4,1 %
> Supérieure ou égale à 2 881 € et inférieure à 3 170 €
> 5,3 %
> Supérieure ou égale à 3 170 € et inférieure à 3 920 €
> 7,5 %
> Supérieure ou égale à 3 920 € et inférieure à 5 016 €
> 9,9 %
> Supérieure ou égale à 5 016 € et inférieure à 5 697 €
> 11,9 %
> Supérieure ou égale à 5 697 € et inférieure à 6 599 €
> 13,8 %
> Supérieure ou égale à 6 599 € et inférieure à 7 907 €
> 15,8 %
> Supérieure ou égale à 7 907 € et inférieure à 8 789 €
> 17,9 %
> Supérieure ou égale à 8 789 € et inférieure à 9 989 €
> 20 %
> Supérieure ou égale à 9 989 € et inférieure à 13 738 €
> 24 %
> Supérieure ou égale à 13 738 € et inférieure à 18 253 €
> 28 %
> Supérieure ou égale à 18 253 € et inférieure à 27 858 €
> 33 %
> Supérieure ou égale à 27 858 € et inférieure à 60 893 €
> 38 %
> Supérieure ou égale à 60 893 €
> 43 %
> c) Pour les contribuables domiciliés en Guyane et à Mayotte :
> Base mensuelle de prélèvement
> Taux proportionnel
> Inférieure à 2 008 €
> 0 %
> Supérieure ou égale à 2 008 € et inférieure à 2 170 €
> 0,5 %
> Supérieure ou égale à 2 170 € et inférieure à 2 420 €
> 1,3 %
> Supérieure ou égale à 2 420 € et inférieure à 2 728 €
> 2,1 %
> Supérieure ou égale à 2 728 € et inférieure à 2 833 €
> 2,9 %
> Supérieure ou égale à 2 833 € et inférieure à 2 930 €
> 3,5 %
> Supérieure ou égale à 2 930 € et inférieure à 3 026 €
> 4,1 %
> Supérieure ou égale à 3 026 € et inférieure à 3 362 €
> 5,3 %
> Supérieure ou égale à 3 362 € et inférieure à 4 639 €
> 7,5 %
> Supérieure ou égale à 4 639 € et inférieure à 6 005 €
> 9,9 %
> Supérieure ou égale à 6 005 € et inférieure à 6 772 €
> 11,9 %
> Supérieure ou égale à 6 772 € et inférieure à 7 858 €
> 13,8 %
> Supérieure ou égale à 7 858 € et inférieure à 8 644 €
> 15,8 %
> Supérieure ou égale à 8 644 € et inférieure à 9 577 €
> 17,9 %
> Supérieure ou égale à 9 577 € et inférieure à 11 115 €
> 20 %
> Supérieure ou égale à 11 115 € et inférieure à 14 953 €
> 24 %
> Supérieure ou égale à 14 953 € et inférieure à 19 020 €
> 28 %
> Supérieure ou égale à 19 020 € et inférieure à 30 482 €
> 33 %
> Supérieure ou égale à 30 482 € et inférieure à 64 341 €
> 38 %
> Supérieure ou égale à 64 341 €
> 43 %
> d) Les limites des tranches des grilles prévues aux a à c sont réduites ou augmentées proportionnellement à la période à laquelle se rapporte le calcul de l'acompte mentionné à l'article 204 C ou proportionnellement à la périodicité usuelle de versement de la rémunération principale, telle qu'elle résulte de la loi, des conventions collectives et accords collectifs, des contrats ou des usages, par le débiteur des revenus mentionnés à l'article 204 B. Toutefois, le débiteur des revenus de remplacement peut effectuer cette réduction ou cette augmentation des tranches proportionnellement à la période à laquelle se rapporte le versement.
> Pour les salaires versés au titre d'un contrat à durée déterminée ou d'un contrat de mission dont le terme initial n'excède pas deux mois ou, s'il s'agit d'un contrat à terme imprécis, dont la durée minimale n'excède pas deux mois, les grilles prévues aux a à c s'appliquent, dans la limite des deux premiers mois d'embauche, aux versements effectués au titre ou au cours d'un mois après un abattement égal à la moitié du montant mensuel du salaire minimum de croissance et sans procéder aux ajustements prévus au premier alinéa du présent d.
> Pour les revenus mentionnés au même article 204 C, les grilles prévues aux a à c du présent 1 s'appliquent à ces revenus majorés de 11 %.
> e) (Abrogé)
> 2. Par dérogation au I du présent article, le taux prévu au 1 du présent III est également applicable aux revenus des personnes rattachées, au sens des 2° et 3° du 3 de l'article 6, ou à charge, au sens des articles 196 et 196 A bis, au titre de la dernière année pour laquelle l'impôt a été établi.
> IV. – 1. Sur option du contribuable, le taux mentionné au III du présent article est appliqué aux traitements et salaires soumis à la retenue à la source prévue au 1° du 2 de l'article 204 A.
> L'option peut être exercée à tout moment auprès de l'administration fiscale et est mise en œuvre au plus tard le troisième mois qui suit celui de la demande. Elle est tacitement reconduite, sauf dénonciation dans les trente jours qui suivent la communication au contribuable d'un nouveau taux de prélèvement.
> 2. Lorsque le montant de la retenue à la source résultant de l'application de ce taux est inférieur à celui qui aurait résulté de l'application du taux prévu, selon le cas, au I du présent article, à l'article 204 I, à l'article 204 J ou à l'article 204 M, le contribuable acquitte un complément de retenue à la source égal à la différence entre ces deux montants.
> Ce complément est calculé et versé par le contribuable au plus tard le dernier jour du mois suivant celui de la perception du revenu selon les modalités prévues aux 4 et 6 de l'article 1663 C et à l'article 1680 A.
> A défaut de paiement, le recouvrement du complément de retenue à la source est assuré et poursuivi selon les mêmes modalités et sous les mêmes garanties et sûretés que l'impôt sur le revenu. Le rôle d'impôt sur le revenu servant de base au calcul du taux de retenue qui aurait dû être appliqué à défaut d'option vaut titre exécutoire en vue de l'exercice des poursuites consécutives à son non-paiement.

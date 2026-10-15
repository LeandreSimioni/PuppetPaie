SOURCE : base KALI de la DILA, open data (https://echanges.dila.gouv.fr/OPENDATA/KALI/), par l'entrepot local
  prive/reference/sources/kali-entrepot/ (outils/relever_les_conventions.py), etat des livraisons : voir etat.json
CONTENEUR : KALICONT000005635964 (CCN des entreprises artistiques et culturelles, IDCC 1285)
TEXTE : KALITEXT000054370607 -- Accord du 30 janvier 2026 relatif au remboursement de frais de santé
RELEVE LE : 2026-10-11
ETAT : TRANSCRIPTION -- le texte des 14 articles tel que la DILA le livre (XML KALI, balise CONTENU, mise en forme
retiree), dans l'ordre des identifiants ; etats : VIGUEUR_NON_ETEN. NON ETENDU a la date du releve : il ne s'impose qu'aux adherents des organisations signataires. Depose pour D12.

CE QU'ON EN LIT (11/10/2026, seance << documents >> ; rien n'est code)
---------------------------------------------------------------------
1. L'accord reecrit le titre XII de la CCN 1285 pour les PERMANENTS (XII.2.1.8 dispenses, XII.2.1.10 cotisations,
   XII.2.1.11 financement, annexe de garanties avec options 1, 2 et 3). Il est VIGUEUR_NON_ETEN a la date du releve :
   il ne s'impose de plein droit qu'aux entreprises adherentes des organisations signataires ; l'extension est a
   guetter dans les livraisons KALI (etat.json). Le moteur applique deja 0,81 % du PMSS en 2026
   (prevoyance-conventionnelle-base.json) : c'est ce texte, art. 6, qui le dit -- a citer le jour du lot, avec la
   reserve de l'extension.
2. LA COTISATION (art. 6, XII.2.1.10) : << 0,74 % du plafond mensuel de la securite sociale au titre de la couverture du
   seul salarie >> au 01/07/2025, << 0,81 % >> au 01/01/2026 (Alsace-Moselle 0,53 puis 0,58). Un pourcentage du PMSS,
   sans un mot sur le temps partiel : comme la 3090, pas de proratisation (inference depuis le silence). L'employeur
   finance au moins 50 % (art. 7).
3. LES DISPENSES (art. 4, XII.2.1.8) : memes cas que la 3090 (de droit a, b, c ; facultatives dont c) temps partiel
   et apprentis a 10 % de la remuneration, d) deja couvert avec justification annuelle). Q13 : memes bornes datees.
   Portees par le moteur (lot R7) : `sante_dispense_depuis` / `sante_dispense_jusqu_au` de la fiche,
   `calcul::dispense_de_la_periode`, 70.014 dans `dsn::rattachements` (b : << jusqu'a l'echeance du contrat
   individuel >>, l. 78).
4. LES OPTIONS (annexe) : << options 1, 2 et 3 >> de garanties, au-dela du socle << seul salarie >> ; leurs codes DSN
   (70.004) ne sont pas dans l'accord, ils viennent de la fiche de parametrage de l'assureur (D14).


==============================================================================
KALIARTI000054370611 | VIGUEUR_NON_ETEN | du 2026-03-01 au 2999-01-01
chemin : 
article : 1er

Le présent accord a pour objet de formaliser le régime « frais de santé » au profit de l'ensemble des salariés permanents, visés à l'article 3, des entreprises entrant dans le champ d'application défini à l'article 2.

==============================================================================
KALIARTI000054370612 | VIGUEUR_NON_ETEN | du 2026-03-01 au 2999-01-01
chemin : 
article : 2

Le présent accord s'applique aux entreprises entrant dans le champ d'application professionnel de la convention collective nationale des entreprises artistiques et culturelles (IDCC 1285) du 1er janvier 1984, étendue par arrêté du 4 janvier 1994 publié au JORF du 26 janvier 1994, et de ses avenants en vigueur, ci-après la convention collective.

==============================================================================
KALIARTI000054370613 | VIGUEUR_NON_ETEN | du 2026-03-01 au 2999-01-01
chemin : 
article : 3

L'article XII.2.1.7 « Bénéficiaires du régime collectif et obligatoire » est désormais rédigé comme suit :

« XII.2.1.7. Bénéficiaires du régime collectif et obligatoire santé des salariés permanents
XII.2.1.7.a. Bénéficiaires

Le régime de remboursement de frais de santé s'applique à l'ensemble des salariés permanents non-cadres et cadres sous contrat de travail à durée déterminée ou indéterminée, sans condition d'ancienneté, affiliés en leur nom propre au régime général de sécurité sociale ou au régime local d'Alsace-Moselle de sécurité sociale, à l'exclusion des salariés intermittents du spectacle.

En effet, il est rappelé que l'ensemble des salariés intermittents employés par les entreprises entrant dans le champ d'application de la convention collective, cadres ou non-cadres, artistes ou techniciens, bénéficient du régime de protection sociale complémentaire issu de l'accord interbranches du 20 décembre 2006 “instituant des garanties collectives et obligatoires pour l'ensemble des intermittents du spectacle”.

XII.2.1.7.b. Régime collectif et obligatoire

Les entreprises doivent tout mettre en œuvre pour que soit respectée pleinement l'exigence d'application intégrale du dispositif conventionnel.

L'adhésion au régime est obligatoire pour tous les salariés permanents définis à l'article 3 du présent accord, sous réserve des cas de dispenses de droit visées à l'article 5.1 et des cas de dispenses facultatives visées à l'article 5.2.

Leur adhésion s'impose donc dans les relations individuelles de travail, et les salariés permanents concernés ne pourront s'opposer au précompte de leur quote-part de cotisations, sauf en cas d'application d'un cas de dispenses facultatives visées à l'article 5.2. »

==============================================================================
KALIARTI000054370614 | VIGUEUR_NON_ETEN | du 2026-03-01 au 2999-01-01
chemin : 
article : 4

L'article XII.2.1.8 « Cotisations » est supprimé et désormais rédigé comme suit :

« XII.2.1.8. Les dispenses d'affiliation

XII.2.1.8.1. Cas de dispenses d'affiliation de droit

À leur initiative, les salariés permanents peuvent se dispenser d'adhérer au contrat collectif s'ils répondent à l'un des cas de dispense dit “de droit”. Ces dispenses s'appliquent obligatoirement même si elles ne sont pas inscrites dans l'acte formalisant le régime au sein de l'entreprise.

Conformément aux dispositions des articles L. 911-7 et D. 911-2 du code de la sécurité sociale, les salariés permanents suivants auront la faculté de refuser, à leur initiative, leur adhésion :
a) Les salariés permanents bénéficiaires d'une couverture complémentaire en application de l'article L. 861-3 du code de la sécurité sociale (c'est-à-dire la “couverture santé solidaire” dite CSS). Cette dispense peut jouer jusqu'à la date à laquelle les salariés permanents cessent de bénéficier de cette couverture ;
b) Les salariés permanents couverts par une assurance individuelle de frais de santé à titre principal ou d'ayants droit au moment de leur embauche. Cette dispense ne peut jouer que jusqu'à l'échéance du contrat individuel ;
c) Les salariés permanents bénéficiant pour les mêmes risques, y compris en qualité d'ayants droit, de prestations servies au titre d'un autre emploi, en tant que bénéficiaire de l'un ou l'autre des dispositifs suivants :
– dispositif collectif et obligatoire des salariés permanents remplissant les conditions mentionnées au 4° du II de l'article L. 242-1 du code de la sécurité sociale ;
– dispositif prévu par le décret n° 2007-1373 du 19 septembre 2007 relatif à la participation de l'État et de ses établissements publics au financement de la protection sociale complémentaire de leurs personnels ou par le décret n° 2011-1474 du 8 novembre 2011 relatif à la participation des collectivités territoriales et de leurs établissements publics au financement de la protection sociale complémentaire de leurs agents ;
– contrat d'assurance groupe issu de la loi n° 94-126 du 11 février 1994 relative à l'initiative et à l'entreprise individuelle dit “contrat Madelin” ;
– régime local d'assurance maladie du Haut-Rhin, du Bas-Rhin et de la Moselle, en application des articles D. 325-6 et D. 325-7 du code de la sécurité sociale ;
– régime complémentaire d'assurance maladie des industries électriques et gazières en application du décret n° 46-1541 du 22 juin 1946 ;
d) Les salariés permanents en contrat à durée déterminée dont la durée de la couverture collective à adhésion obligatoire dont ils bénéficient en matière de frais de santé est inférieure à trois mois, s'ils justifient bénéficier par ailleurs d'une couverture “responsable” au sens de l'article L. 871-1 du code de la sécurité sociale.

Les salariés permanents peuvent se dispenser s'ils respectent les conditions prévues à l'article D. 911-2 du code de la sécurité sociale. Ces salariés permanents ne pourront solliciter, par écrit, auprès de leur employeur, leur dispense d'adhésion qu'au moment de leur embauche ou de la prise d'effet des couvertures mentionnées aux a et d ci-dessus. La demande de dispense doit être accompagnée, le cas échéant, de tous justificatifs nécessaires.

À défaut de respecter les prescriptions détaillées ci-dessus, le salarié sera automatiquement affilié au contrat collectif à adhésion obligatoire.

XII.2.1.8.2. Cas de dispenses d'affiliation facultatives

En application de l'article R. 242-1-6 du code de la sécurité sociale, les signataires de la présente convention ont négocié les facultés de dispense au bénéfice de certains salariés permanents.

Ces derniers peuvent, à leur initiative, et quelle que soit leur date d'embauche, refuser d'adhérer au contrat collectif s'ils le souhaitent, à condition que leur employeur l'ait expressément prévu dans l'acte formalisant le régime dans l'entreprise et à condition d'être dans l'une des situations visées ci-après :
a) Les salariés permanents et les apprentis bénéficiaires d'un contrat de travail à durée déterminée d'une durée au moins égale à 12 mois, à condition de justifier par écrit, en produisant tous documents, d'une couverture individuelle souscrite par ailleurs en matière de remboursement de frais médicaux, conformément à l'article R. 242-1-6, 2°, a, du code de la sécurité sociale ;
b) Les salariés permanents et les apprentis bénéficiaires d'un contrat de travail à durée déterminée d'une durée inférieure à 12 mois, même s'ils ne bénéficient pas d'une couverture individuelle souscrite, par ailleurs, conformément à l'article R. 242-1-6, 2°, b, du code de la sécurité sociale ;
c) Les salariés permanents à temps partiel et les apprentis dont l'adhésion au système de garanties les conduirait à s'acquitter d'une cotisation au moins égale à 10 % de leur rémunération brute, conformément à l'article R. 242-1-6, 2°, c, du code de la sécurité sociale ;
d) Les salariés permanents bénéficiant, y compris au titre d'un seul et même emploi ou en qualité d'ayants droit, sous réserve de la justifier chaque année, d'une couverture collective de remboursement de frais de santé servie :
– dans le cadre d'un dispositif de prévoyance complémentaire remplissant les conditions mentionnées au 4° du II de l'article L. 242-1 du code de la sécurité sociale ;
– par le régime local d'assurance maladie du Haut-Rhin, du Bas-Rhin et de la Moselle, en application des articles D. 325-6 et D. 325-7 du code de la sécurité sociale ;
– par le régime complémentaire d'assurance maladie des industries électriques gazières en application du décret n° 46-1541 du 22 juin 1946 ;
– dans le cadre des dispositions prévues par le décret n° 2007-1373 du 19 septembre 2007 relatif à la participation de l'État et de ses établissements publics au financement de la protection sociale complémentaire de leurs personnels ;
– dans le cadre des dispositions prévues par le décret n° 2011-1474 du 8 novembre 2011 relatif à la participation des collectivités territoriales et de leurs établissements publics au financement de la protection sociale complémentaire de leurs agents ;
– dans le cadre des contrats d'assurance de groupe issus de la loi n° 94-126 du 11 février 1994 relative à l'initiative et à l'entreprise individuelle, dits “contrats Madelin” ;
– par le régime spécial de sécurité sociale des gens de mer ;
– par la caisse de prévoyance et de retraite des personnels de la SNCF ;
e) Les salariés permanents embauchés avant la mise en place, par décision unilatérale, d'un dispositif de protection sociale complémentaire collectif et obligatoire, en application du 1° de l'article R. 242-1-6 du code de la sécurité sociale.

Les cas de dispense listés ci-dessus peuvent être invoqués à tout moment par les salariés permanents, dès lors que leur employeur a prévu l'un des cas de dispense précités dans l'acte formalisant le régime, qu'ils en remplissent les conditions et en justifient. »

==============================================================================
KALIARTI000054370639 | VIGUEUR_NON_ETEN | du 2026-03-01 au 2999-01-01
chemin : 
article : 5

L'article XII.2.1.9 « Prestations » est désormais rédigé comme suit :

« XII.2.1.9. Garanties frais de santé

Le présent accord permet aux salariés permanents, tels que définis par l'article 3, d'accéder à un niveau minimal de garanties de protection sociale complémentaire au sens de l'article L. 911-2 du code de la sécurité sociale, en matière de remboursement de frais de santé.

Les salariés permanents bénéficient du niveau minimal de garanties défini par le présent accord quel que soit l'organisme assureur retenu par leur entreprise, et tel que détaillé au sein de l'annexe 1.

Les entreprises peuvent, si elles le souhaitent, adapter les dispositions du présent accord à condition d'assurer des garanties au moins équivalentes au sens de l'article L. 2253-1 du code du travail.

Elles peuvent aussi améliorer les garanties du niveau minimal si elles le souhaitent. Il est communiqué à titre indicatif au sein de l'annexe 1 des niveaux de garanties supérieurs.

La mise en place d'un régime de protection sociale complémentaire collectif par la voie négociée est alors encouragée. Le comité social et économique est consulté préalablement à la mise en place d'une garantie collective ou à la modification de celle-ci, dans les conditions prévues par la loi. »

==============================================================================
KALIARTI000054370642 | VIGUEUR_NON_ETEN | du 2026-03-01 au 2999-01-01
chemin : 
article : 6

Est créé un article XII.2.1.10 « Cotisations » rédigé comme suit :

« XII.2.1.10. Cotisations

À compter du 1er juillet 2025

Pour les salariés permanents relevant du régime général de la sécurité sociale, les cotisations frais de santé s'élèvent à 0,74 % du plafond mensuel de la sécurité sociale au titre de la couverture du seul salarié.

Pour les salariés permanents relevant du régime local d'Alsace-Moselle, le montant de la cotisation frais de santé est fixé à 0,53 % du plafond mensuel de la sécurité sociale au titre de la couverture du seul salarié.

À compter du 1er janvier 2026

Pour les salariés permanents relevant du régime général de la sécurité sociale, les cotisations frais de santé s'élèvent à 0,81 % du plafond mensuel de la sécurité sociale au titre de la couverture du seul salarié.

Pour les salariés permanents relevant du régime local d'Alsace-Moselle, le montant de la cotisation frais de santé est fixé à 0,58 % du plafond mensuel de la sécurité sociale au titre de la couverture du seul salarié.

Les parties rappellent que les cotisations des salariés sont déductibles de l'assiette de l'impôt sur le revenu dans les conditions de l'article 83, 1° quater du code général des impôts. »

==============================================================================
KALIARTI000054370644 | VIGUEUR_NON_ETEN | du 2026-03-01 au 2999-01-01
chemin : 
article : 7

Est créé un article XII.2.1.11 « Financement », rédigé comme suit :

« XII.2.1.11. Financement

Toutes les entreprises entrant dans le champ d'application du présent accord ont l'obligation de prendre en charge au moins 50 % du montant de la cotisation brute globale destinée au financement de la garantie “santé socle” pour le salarié.

Les entreprises peuvent négocier ou mettre en place, à leur initiative, dans le respect des procédures prévues à l'article L. 911-1 du code de la sécurité sociale, une répartition plus favorable aux salariés permanents. »

==============================================================================
KALIARTI000054370646 | VIGUEUR_NON_ETEN | du 2026-03-01 au 2999-01-01
chemin : 
article : 8

Est créé un article XII.2.1.12 « Maintien de garanties en cas de suspension du contrat de travail indemnisée », rédigé comme suit :

« XII.2.1.12. Maintien de garanties en cas de suspension du contrat de travail indemnisée

Le bénéfice des garanties mises en place par le présent accord est maintenu au profit des salariés permanents dont le contrat de travail est suspendu, pour la période au titre de laquelle ils bénéficient :
– soit d'un maintien total ou partiel de leur rémunération ;
– soit d'indemnités journalières complémentaires financées au moins en partie par l'employeur, qu'elles soient versées directement par celui-ci ou pour son compte par l'intermédiaire d'un tiers ;
– soit d'un revenu de remplacement versé par l'employeur. Ce cas concerne notamment les salariés permanents placés en activité partielle ou en activité partielle de longue durée, dont l'activité est totalement suspendue ou dont les horaires sont réduits, ainsi que toute période de congé rémunéré par l'employeur (reclassement, mobilité…).

Les contributions de l'employeur et des salariés permanents dont la suspension du contrat de travail est indemnisée sont maintenues selon les règles prévues au contrat collectif d'assurance, pendant la totalité des périodes de suspension du contrat de travail indemnisées. »

==============================================================================
KALIARTI000054370647 | VIGUEUR_NON_ETEN | du 2026-03-01 au 2999-01-01
chemin : 
article : 9

Est créé un article XII.2.1.13 « Maintien des garanties après rupture du contrat de travail », rédigé comme suit :

« XII.2.1.13. Maintien des garanties après rupture du contrat de travail

En application de l'article L. 911-8 du code de la sécurité sociale, les anciens salariés permanents se voient maintenir dans les mêmes conditions que les salariés permanents en activité, le régime de frais de santé en vigueur dans l'entreprise, en cas de rupture de leur contrat de travail ouvrant droit à une prise en charge par le régime d'assurance chômage (à l'exception du licenciement pour faute lourde).

En cas de modification ou de révision des garanties des salariés permanents en activité, les garanties des anciens salariés permanents bénéficiant du dispositif de portabilité seront modifiées ou révisées dans les mêmes conditions.

Le droit à portabilité est conditionné au respect de l'ensemble des conditions fixées par l'article L. 911-8 du code de la sécurité sociale et sera mis en œuvre dans les conditions déterminées par cet article.

L'employeur signale le maintien des garanties dans le cadre du dispositif de portabilité dans le certificat de travail. »

==============================================================================
KALIARTI000054370650 | VIGUEUR_NON_ETEN | du 2026-03-01 au 2999-01-01
chemin : 
article : 10

Est créé un article XII.2.1.14 « Commission paritaire nationale de suivi », rédigé comme suit :

« XII.2.1.14. Commission paritaire nationale de suivi

Il est institué une commission paritaire de suivi du régime frais de santé dont les modalités de fonctionnement seront précisées en tant que de besoin dans son règlement intérieur.

Cette commission paritaire de suivi est composée de 8 représentants des organisations d'employeurs et de 8 représentants des fédérations syndicales de salariés représentatives dans la branche.

1. Chaque fédération syndicale de salariés, reconnue représentative, dispose d'un poste de titulaire et d'un poste de suppléant.

Les sièges restants sont répartis au prorata de la représentativité effective, précisée par un arrêté du ministère du travail.

2. Chaque organisation syndicale représentative représentant les employeurs dispose d'un poste de titulaire et d'un poste de suppléant. »

==============================================================================
KALIARTI000054370651 | VIGUEUR_NON_ETEN | du 2026-03-01 au 2999-01-01
chemin : 
article : 11

Le présent accord entre en vigueur à compter du 1er mars 2026. Il se substituera pleinement à compter de cette date aux stipulations prévues dans les différents accords et leurs avenants portant sur le même objet.

Il pourra être révisé selon les règles prévues aux articles L. 2222-5, L. 2222-6, L. 2261-7 et L. 2261-8 du code du travail, et dénoncé selon les règles prévues aux articles L. 2261-9, L. 2261-10, L. 2261-11, L. 2261-12 et L. 2261-13 du même code.

Le présent accord sera déposé en 2 exemplaires (une version sur support papier signée des parties et une version sur support électronique) auprès des services centraux du ministre chargé du travail, dans les conditions prévues aux articles L. 2231-6 et D. 2231-2 et suivants du code du travail.

En outre, un exemplaire sera établi pour chaque partie.

Suivant les dispositions de l'article L. 2261-23-1 du code du travail, les parties signataires n'ont pas retenu de dispositions spécifiques telles que visées par l'article L. 2232-10-1 du code du travail à l'attention des entreprises de moins de 50 salariés permanents, dès lors que le présent accord vise à modifier le régime collectif obligatoire de frais de santé dont doivent bénéficier les salariés permanents relevant de la convention collective et ce quel que soit l'effectif de leur entreprise.

Les entreprises ayant mis en place des garanties de remboursement de frais de santé antérieurement à l'entrée en vigueur du présent accord doivent les adapter, dans un délai de 3 mois à compter de cette entrée en vigueur, pour les rendre au moins égales aux prestations exposées en annexe 1.

==============================================================================
KALIARTI000054370665 | VIGUEUR_NON_ETEN | du 2026-03-01 au 2999-01-01
chemin : 
article : 12

Le présent accord fera l'objet des formalités de notification, publicité et dépôt conformément aux dispositions légales et réglementaires. Les signataires du présent accord demandent au ministère du travail son extension à toutes les entreprises entrant dans le champ d'application de la convention collective.

==============================================================================
KALIARTI000054370667 | VIGUEUR_NON_ETEN | du 2026-03-01 au 2999-01-01
chemin : Préambule
article : (sans numero)

La convention collective nationale pour les entreprises artistiques et culturelles (ci-après la convention collective) a été signée en date du 1er janvier 1984. Celle-ci instaurait un régime de prévoyance et de frais de santé à destination de la branche des entreprises artistiques et culturelles (IDCC 1285) par un avenant signé en date du 26 juin 2008.

Ce dernier institue des garanties collectives et obligatoires, d'une part, « incapacité-invalidité-décès » et, d'autre part, « remboursement de frais de santé ».

Plusieurs avenants successifs s'en sont suivis concernant les évolutions desdits régimes de la branche des entreprises artistiques et culturelles.

Dans ces conditions les organisations patronales et syndicales représentatives au niveau national dans la branche se sont réunies afin de compléter et refondre le régime de protection sociale et les garanties collectives et obligatoires en matière de « remboursement de frais de santé » des salariés permanents de la branche.

C'est dans ce contexte que les parties à la convention collective de branche ont décidé de revoir le régime « remboursement de frais de santé » préexistant dans le présent accord.

Celui-ci vient mettre à jour les articles XII.2.1.7 à XII.2.1.9 préexistants de la convention collective relatifs à la mise en place de garanties frais de santé et ajouter de nouveaux articles XII.2.1.10, XII.2.1.11, XII.2.1.12, XII.2.1.13 et XII.2.14.

Les parties au présent accord ont pris en compte la négociation et la conclusion d'accords collectifs interbranches instituant des garanties spécifiques au profit des salariés intermittents du spectacle.

Les signataires de la présente convention rappellent leur attachement à un dialogue social régulier, constructif au regard des intérêts communs de leurs mandants respectifs, et fondé sur la confiance. Ce dialogue social est organisé de façon à piloter la mise en œuvre, le suivi de la réalisation et les éventuelles évolutions du présent accord.

==============================================================================
KALIARTI000054370668 | VIGUEUR_NON_ETEN | du 2026-03-01 au 2999-01-01
chemin : Annexe
article : (sans numero)

Annexe 1

Tableau de garanties

Les remboursements interviennent dans la limite des frais restant à charge après remboursement de la sécurité sociale et d'éventuels organismes complémentaires.

Les montants sont exprimés y compris le remboursement de la sécurité sociale sauf pour les lentilles correctrices des options 1, 2 et 3.

(Tableaux non reproduits, consultables en ligne sur le site Légifrance, rubrique Bulletins officiels des conventions collectives, pages 18 à 21.)

 https://www.legifrance.gouv.fr/download/file/pdf/boc_20260015_0000_0003.pdf/BOCC


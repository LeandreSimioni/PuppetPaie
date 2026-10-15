SOURCE : base KALI de la DILA, open data (https://echanges.dila.gouv.fr/OPENDATA/KALI/), par l'entrepot local
  prive/reference/sources/kali-entrepot/ (outils/relever_les_conventions.py), etat des livraisons : voir etat.json
CONTENEUR : KALICONT000028157262 (CCN des entreprises du secteur prive du spectacle vivant, IDCC 3090)
TEXTE : KALITEXT000050394834 -- Accord du 19 juillet 2024 relatif au regime obligatoire de frais de sante des salaries
  hors salaries intermittents du spectacle (page Legifrance donnee par Leandre le 11/10/2026 :
  https://www.legifrance.gouv.fr/conv_coll/id/KALITEXT000050394834/?idConteneur=KALICONT000028157262)
RELEVE LE : 2026-10-11
ETAT : TRANSCRIPTION -- le texte des 30 articles tel que la DILA le livre (XML KALI, balise CONTENU, mise en forme
retiree), dans l'ordre des identifiants ; tous VIGUEUR_ETEN depuis le 01/01/2025. Les articles 13 et 14 etaient deja
recopies dans kali-3090-titres-iv-v-sante-20260923.md ; ceci est le texte entier (D12).

CE QU'ON EN LIT (11/10/2026, seance << documents >> ; rien n'est code)
---------------------------------------------------------------------
1. LA COTISATION EST UN POURCENTAGE DU PMSS (art. 13) : socle 1,29 % (2025), 1,44 % (2026), 1,60 % (2027), 1,75 %
   (2028) ; annexe II 1,74 / 1,84 / 1,94. L'employeur finance au moins 50 %. Le texte NE DIT RIEN d'un salarie a temps
   partiel : pas de proratisation de l'assiette, pas de plafond reduit. Lecture : la cotisation sante d'un temps partiel
   est la meme que celle d'un temps plein (le forfait suit le PMSS entier), sauf dispense (ci-dessous). La question du
   plafond proratise (V4b, 40.084) ne concerne donc pas la sante 3090 : INFERENCE depuis le silence du texte, a
   confronter au contrat d'adhesion de la compagnie (fiche de parametrage Audiens, D14).
2. LES DISPENSES (art. 9.1, 9.2) : adhesion obligatoire, << sous reserve des cas de dispenses facultatives visees a
   l'article 9.2.1 et des cas de dispenses de droit visees a l'article 9.2.2 >>. Facultatives (R. 242-1-6 CSS) :
   c) << les salaries a temps partiel et les apprentis dont l'adhesion [...] les conduirait a s'acquitter d'une
   cotisation au moins egale a 10 % de leur remuneration brute >> ; d) les salaries deja couverts, y compris en ayant
   droit, << sous reserve de la justifier chaque annee >>. De droit : CSS (a), assurance individuelle jusqu'a son
   echeance (b), couverture au titre d'un autre emploi (c). La demande : << declaration sur l'honneur [...] accompagnee
   de justificatifs >>, revocable a tout moment par ecrit. Q13 (faut-il des dates a la dispense ?) : le texte en donne
   deux qui bornent une dispense -- l'echeance du contrat individuel (b) et la justification annuelle (d) ; une
   dispense peut donc cesser, et la fiche devrait pouvoir la dater. Le moteur ne porte qu'un motif (V3e).
   OU LE MOTEUR S'EN SERT (lot R7, Q13 tranchee par Leandre le 11/10/2026) : `sante_dispense_depuis` et
   `sante_dispense_jusqu_au` de la fiche (champs-base.json, CCNEAC seulement comme `sante_dispense`) ;
   `calcul::dispense_de_la_periode` (hors des bornes, affilie ; une borne en cours de mois refuse) ;
   `dsn::rattachements` (70.014 = lendemain de la fin de dispense). LOT R12 (11/10/2026) : la 3090 a ses dispenses
   dans le moteur -- les trois champs s'ouvrent aux permanents 3090 (`si_entreprise` 1285 et 3090 ; la note `_note` de
   `sante_dispense` cite les motifs 9.2.1 a-e et 9.2.2 a-e), `bareme::bareme_sante` retire la mutuelle SVP d'un
   dispense, `dsn::affiliations_du_permanent` ne le rattache pas a la sante (ni bloc 70, ni base 31) et refuse
   toujours une dispense que le bulletin ne suit pas. Le motif reste un texte libre : rien ne le code en DSN.
   Test : `tests/options_sante.rs`, `un_permanent_3090_dispense_de_sante_n_est_pas_affilie_a_la_sante`.
3. LE VERSEMENT SANTE (art. 10) : CDD de trois mois ou moins, ou temps partiel de 15 heures par semaine ou moins,
   dispenses a leur initiative (L. 911-7, III), avec un << versement sante >> calcule selon D. 911-8 CSS. Non porte
   par le moteur (regle 0 : un cas a prevoir pour un permanent court ou a petit temps partiel).
4. LES GARANTIES ADDITIONNELLES (art. 13, fin) : obligatoires dans l'entreprise, l'employeur en finance au moins
   50 % ; facultatives, << integralement a la charge des salaries ayant choisi d'opter >>. C'est la base de l'option
   (S21.G00.70.004) : elle existe par l'acte de l'entreprise, pas par l'accord -- la FAQ des OC le dit aussi
   (sources/faq-dsn-organismes-complementaires/). Q14 : pas de code d'option dans l'accord.
5. LE DEGRE ELEVE DE SOLIDARITE (art. 12.2) : au moins 2 % de la cotisation HT, collecte par l'assureur sur la
   cotisation : rien a calculer en paie.
6. L'annexe II (grille de garanties) est transcrite en fin de fichier (deux articles << Annexes >>).


==============================================================================
KALIARTI000050394841 | VIGUEUR_ETEN | du 2025-01-01 au 2999-01-01
chemin : Préambule
article : (sans numero)

Les organisations patronales et syndicales représentatives au niveau national dans la branche des entreprises du secteur privé du spectacle vivant (IDCC 3090), ci-après la convention collective, ont mis en place un socle minimal de branche en protection sociale complémentaire définissant les garanties de frais de santé au bénéfice des salariés visés au titre Ier de la convention collective, à l'exclusion des salariés intermittents du spectacle déjà couverts dans le cadre de l'accord interbranche du 20 décembre 2006 (modifié par l'avenant n° 1 du 16 juin 2008 et l'avenant n° 2 du 18 décembre 2008). 

À ce jour, ce régime en matière de protection sociale complémentaire des salariés hors intermittents du spectacle est encadré par les dispositions suivantes : 
– le titre XIII de la convention collective nationale des entreprises du secteur privé du spectacle vivant ; 
– l'accord du 15 décembre 2015 relatif au régime collectif obligatoire de frais de santé ; 
– l'article IV. 3 de l'annexe 2 de la convention collective nationale des entreprises du secteur privé du spectacle vivant du 12 février 2012 et ses avenants du 19 juin 2012 du 12 octobre 2017 ; 
– l'accord du 2 juillet 2001 relatifs aux frais de santé des théâtres privés modifié par l'avenant du 3 juillet 2013. 

Les partenaires sociaux de la branche se sont réunis afin de faire évoluer le dispositif consacré aux salariés, hors intermittents du spectacle. Il résulte de ces négociations la rédaction du présent accord, qui institue un régime de couverture complémentaire aux prestations de la sécurité sociale concernant le risque « frais de santé ». Cet accord est destiné à améliorer significativement la protection sociale des salariés permanents au sein de la convention collective, dans un cadre mutualisé  (1). 

Les signataires de la présente convention rappellent leur attachement à un dialogue social régulier, constructif au regard des intérêts communs de leurs mandants respectifs, et fondé sur la confiance. Ce dialogue social est organisé de façon à piloter la mise en œuvre, le suivi de la réalisation et les éventuelles évolutions du présent accord. 

Les entreprises peuvent, si elles le souhaitent, adapter les dispositions du présent accord à condition d'assurer des garanties au moins équivalentes au sens de l'article L. 2253-1 du code du travail. La mise en place d'un régime de protection sociale complémentaire collectif par la voie négociée est alors encouragée. Le comité social et économique est consulté préalablement à la mise en place d'une garantie collective ou à la modification de celle-ci, dans les conditions prévues par la loi.

 (1) Les mots « dans un cadre mutualisé » sont exclus de l'extension en tant qu'ils contreviennent aux dispositions de l'article L. 912-1 du code de la sécurité sociale relatives à la couverture des risques et à la recommandation d'organismes complémentaires.  
(Arrêté du 5 mars 2025 - art. 1)

==============================================================================
KALIARTI000050394843 | VIGUEUR_ETEN | du 2025-01-01 au 2999-01-01
chemin : Titre Ier Dispositions générales
article : 1er

Le présent accord a pour objet l'institution d'un socle minimal de garanties conventionnelles de frais de santé complémentaire de branche.

==============================================================================
KALIARTI000050394844 | VIGUEUR_ETEN | du 2025-01-01 au 2999-01-01
chemin : Titre Ier Dispositions générales
article : 2

Le champ d'application du présent accord est celui de la convention collective nationale des entreprises du secteur privé du spectacle vivant (IDCC 3090) du 3 février 2012, étendue le 29 mai 2013 (JORF 7 juin 2013), et de ses avenants en vigueur, ci-après la convention collective.

==============================================================================
KALIARTI000050394845 | VIGUEUR_ETEN | du 2025-01-01 au 2999-01-01
chemin : Titre Ier Dispositions générales
article : 3

Le présent accord, bénéficie sans condition d'ancienneté à tous les salariés employés par les entreprises entrant dans le champ d'application défini par l'article 2 des présentes, ci-après dénommés salariés permanents, à l'exclusion des salariés intermittents du spectacle.

Les salariés intermittents du spectacle ne relèvent pas du présent accord, dans la mesure où ils peuvent bénéficier d'une garantie « frais de santé » au titre du régime spécifique prévue par l'accord interbranche du 20 décembre 2006 (modifié par l'avenant n° 1 du 16 juin 2008 et l'avenant n° 2 du 18 décembre 2008).

Pour l'application du présent article, est considéré :

– comme salarié permanent, tout salarié qui ne peut être qualifié au sens du présent article comme salarié intermittent du spectacle ;

– comme salarié intermittent du spectacle, tout salarié effectuant des prestations artistiques ou techniques, employé en contrat à durée déterminée, y compris en CDD d'usage, dont la fonction est reprise, soit dans la liste des emplois pour lesquels le recours au contrat de travail à durée déterminée d'usage est autorisé par la convention collective, soit dans la liste des emplois annexes 8 et 10 au régime d'assurance chômage.

==============================================================================
KALIARTI000050394846 | VIGUEUR_ETEN | du 2025-01-01 au 2999-01-01
chemin : Titre Ier Dispositions générales
article : 4



==============================================================================
KALIARTI000050394847 | VIGUEUR_ETEN | du 2025-01-01 au 2999-01-01
chemin : Titre Ier Dispositions générales
article : 4.1

Le bénéfice des garanties mises en place par le présent accord est maintenu au profit des salariés permanents dont le contrat de travail est suspendu, pour la période au titre de laquelle ils bénéficient :

– soit d'un maintien total ou partiel de leur rémunération ;

– soit d'indemnités journalières complémentaires financées au moins en partie par l'employeur, qu'elles soient versées directement par celui-ci ou pour son compte par l'intermédiaire d'un tiers ;

– soit d'un revenu de remplacement versé par l'employeur. Ce cas concerne notamment les salariés permanents placés en activité partielle ou en activité partielle de longue durée, dont l'activité est totalement suspendue ou dont les horaires sont réduits, ainsi que toute période de congé rémunéré par l'employeur (reclassement, mobilité…).

Les contributions de l'employeur et des salariés permanents dont la suspension du contrat de travail est indemnisée sont maintenues selon les règles prévues au contrat collectif d'assurance, pendant la totalité des périodes de suspension du contrat de travail indemnisées.

==============================================================================
KALIARTI000050394848 | VIGUEUR_ETEN | du 2025-01-01 au 2999-01-01
chemin : Titre Ier Dispositions générales
article : 4.2

Le bénéfice des garanties mises en place par le présent accord est suspendu pour les salariés permanents dont le contrat de travail est suspendu et ne donne lieu à aucune indemnisation.

Sont notamment concernés par cette suspension de garanties les salariés permanents se trouvant dans l'un des cas suivants :

– le congé sabbatique visé aux articles L. 3142-28 et suivants du code du travail ;

– le congé parental d'éducation total, visé aux articles L. 1225-47 et suivants du code du travail ;

– le congé pour création d'entreprise visé aux articles L. 3142-105 et suivants du code du travail ;

– le congé sans solde, tel que convenu après accord entre l'employeur et le salarié.

Les salariés susmentionnés peuvent néanmoins demander à rester affiliés au contrat collectif d'assurance, au-delà de la période de suspension visée à l'alinéa précédent, sous réserve de s'acquitter intégralement de la cotisation afférente, part salariale et part patronale comprises. Dans ce cas, l'organisme assureur prélève la cotisation directement auprès du salarié qui bénéficiera d'un maintien des garanties ci-après définies pendant la période de suspension de son contrat de travail.

==============================================================================
KALIARTI000050394852 | VIGUEUR_ETEN | du 2025-01-01 au 2999-01-01
chemin : Titre Ier Dispositions générales
article : 5

En application de l'article L. 911-8 du code de la sécurité sociale, les anciens salariés se voient maintenir dans les mêmes conditions que les salariés en activité, le régime de frais de santé en vigueur dans l'entreprise, en cas de rupture de leur contrat de travail ouvrant droit à une prise en charge par le régime d'assurance chômage (à l'exception du licenciement pour faute lourde).

En cas de modification ou de révision des garanties des salariés en activité, les garanties des anciens salariés bénéficiant du dispositif de portabilité seront modifiées ou révisées dans les mêmes conditions.

Le droit à portabilité est conditionné au respect de l'ensemble des conditions fixées par l'article L. 911-8 du code de la sécurité sociale et sera mis en œuvre dans les conditions déterminées par cet article.

L'employeur signale le maintien des garanties dans le cadre du dispositif de portabilité dans le certificat de travail.

==============================================================================
KALIARTI000050394855 | VIGUEUR_ETEN | du 2025-01-01 au 2999-01-01
chemin : Titre Ier Dispositions générales
article : 6

Le présent accord permet aux salariés permanents, tels que définis par l'article 3, d'accéder à un niveau minimal de garanties de protection sociale complémentaire au sens de l'article L. 911-2 du code de la sécurité sociale, en matière de remboursement de frais de soins de santé, dans les conditions définies au titre II du présent accord.

Les salariés permanents bénéficient du niveau minimal de garanties défini par le présent accord quel que soit l'organisme assureur retenu par leur entreprise.
 (1)

Pour les entreprises relevant de l'annexe II de la convention collective, le niveau minimal de garantie est supérieur à celui défini pour les autres entreprises. Il figure en annexe au présent accord et s'impose aux dites entreprises.

 (1) Alinéa étendu sous réserve du respect des dispositions de l'article L. 2253-1 du code du travail relatif à la hiérarchie des normes conventionnelles en application duquel l'accord de branche prévaut sur l'accord d'entreprise sauf lorsque ce dernier assure des garanties au moins équivalentes. 

(Arrêté du 5 mars 2025 - art. 1)

==============================================================================
KALIARTI000050394857 | VIGUEUR_ETEN | du 2025-01-01 au 2999-01-01
chemin : Titre Ier Dispositions générales
article : 7

Les entreprises disposant déjà d'un contrat d'assurance collectif de protection sociale complémentaire au profit de leurs salariés, antérieurement à la date d'entrée en vigueur du présent accord, pourront maintenir leur contrat à la condition que les garanties offertes par leur contrat soient au moins équivalentes à celles définies dans le présent accord.

À défaut de garanties équivalentes, elles devront, au plus tard dans le délai mentionné à l'article 17 ci-après :

– soit adapter les dispositions de leur contrat d'assurance collectif à un niveau au moins équivalent à celui défini dans le présent accord ;

– soit souscrire un nouveau contrat d'assurance collectif auprès de l'organisme assureur de leur choix, à un niveau au moins équivalent à celui du présent accord.

==============================================================================
KALIARTI000050394858 | VIGUEUR_ETEN | du 2025-01-01 au 2999-01-01
chemin : Titre II Dispositions relatives aux garanties conventionnelles de frais de soins de santé
article : 8

L'adhésion au régime est obligatoire pour tous les salariés définis à l'article 3 du présent accord.

Les salariés sont tenus d'adhérer au contrat collectif mis en place à titre obligatoire, sous réserve des cas de dispenses facultatives visées à l'article 9.2.1 et des cas de dispenses de droit visées à l'article 9.2.2

Leur adhésion s'impose donc dans les relations individuelles de travail, et les salariés concernés ne pourront s'opposer au précompte de leur quote-part de cotisations, sauf en cas d'application d'un cas de dispenses facultatives visées à l'article 9.2.1.

==============================================================================
KALIARTI000050394859 | VIGUEUR_ETEN | du 2025-01-01 au 2999-01-01
chemin : Titre II Dispositions relatives aux garanties conventionnelles de frais de soins de santé
article : 9



==============================================================================
KALIARTI000050394860 | VIGUEUR_ETEN | du 2025-01-01 au 2999-01-01
chemin : Titre II Dispositions relatives aux garanties conventionnelles de frais de soins de santé
article : 9.1

Par dérogation au caractère obligatoire du contrat collectif, certains salariés peuvent être dispensés d'adhésion, à leur demande, sous réserve de fournir à leur employeur tout justificatif nécessaire précisant le cadre dans lequel cette dispense est formulée, la dénomination de l'organisme assureur portant le contrat souscrit, ou, le cas échéant, la date de fin de ce droit s'il est borné, suivant les exigences réglementaires en vigueur.

Cette déclaration prend la forme d'une déclaration sur l'honneur du salarié, accompagnée de justificatifs adéquats. La demande de dispense comporte la mention selon laquelle le salarié a été préalablement informé par l'employeur des conséquences de son choix, à savoir une absence de couverture et la renonciation aux droits à portabilité.

Les salariés peuvent, à tout moment, revenir sur leur demande de dispense, et solliciter, auprès de l'employeur, par écrit, leur affiliation au contrat collectif. En tout état de cause, les salariés cessant de justifier de la situation leur permettant de bénéficier d'un cas de dispense seront tenus de cotiser et d'adhérer au contrat collectif à adhésion obligatoire.

==============================================================================
KALIARTI000050394861 | VIGUEUR_ETEN | du 2025-01-01 au 2999-01-01
chemin : Titre II Dispositions relatives aux garanties conventionnelles de frais de soins de santé
article : 9.2

9.2.1.   Les dispenses facultatives

En application de l'article R. 242-1-6 du code de la sécurité sociale, les signataires de la présente convention ont négocié les facultés de dispense au bénéfice de certains salariés.

Ces derniers peuvent, à leur initiative, et quelle que soit leur date d'embauche, refuser d'adhérer au contrat collectif s'ils le souhaitent, à condition que leur employeur l'ait expressément prévu dans l'acte formalisant le régime dans l'entreprise et à condition d'être dans l'une des situations visées ci-après :

a) Les salariés et les apprentis bénéficiaires d'un contrat de travail à durée déterminée d'une durée au moins égale à 12 mois, à condition de justifier par écrit, en produisant tous documents, d'une couverture individuelle souscrite par ailleurs en matière de remboursement de frais médicaux, conformément à l'article R. 242-1-6,2°, a, du code de la sécurité sociale.

b) Les salariés et les apprentis bénéficiaires d'un contrat de travail à durée déterminée d'une durée inférieure à 12 mois, même s'ils ne bénéficient pas d'une couverture individuelle souscrite, par ailleurs, conformément à l'article R. 242-1-6,2°, b, du code de la sécurité sociale.

c) Les salariés à temps partiel et les apprentis dont l'adhésion au système de garanties les conduirait à s'acquitter d'une cotisation au moins égale à 10 % de leur rémunération brute, conformément à l'article R. 242-1-6,2°, c, du code de la sécurité sociale.

d) Les salariés bénéficiant, y compris au titre d'un seul et même emploi ou en qualité d'ayants droit, sous réserve de la justifier chaque année, d'une couverture collective de remboursement de frais de santé servie :

– dans le cadre d'un dispositif de prévoyance complémentaire remplissant les conditions mentionnées au sixième alinéa de l'article L. 242-1 du code de la sécurité sociale ;

– par le régime local d'assurance maladie du Haut-Rhin, du Bas-Rhin et de la Moselle, en application des articles D. 325-6 et D. 325-7 du code de la sécurité sociale ;

– par le régime complémentaire d'assurance maladie des industries électriques gazières en application du décret n° 46-1541 du 22 juin 1946 ;

– dans le cadre des dispositions prévues par le décret n° 2007-1373 du 19 septembre 2007 relatif à la participation de l'État et de ses établissements publics au financement de la protection sociale complémentaire de leurs personnels ;

– dans le cadre des dispositions prévues par le décret n° 2011-1474 du 8 novembre 2011 relatif à la participation des collectivités territoriales et de leurs établissements publics au financement de la protection sociale complémentaire de leurs agents ;

– dans le cadre des contrats d'assurance de groupe issus de la loi n° 94-126 du 11 février 1994 relative à l'initiative et à l'entreprise individuelle, dits « contrats Madelin » ;

– par le régime spécial de sécurité sociale des gens de mer ;

– par la caisse de prévoyance et de retraite des personnels de la SNCF.

e) Les salariés embauchés avant la mise en place, par décision unilatérale, d'un dispositif de protection sociale complémentaire collectif et obligatoire, en application 1° de l'article R. 242-1-6 du code de la sécurité sociale.

Les cas de dispense listés ci-dessus peuvent être invoqués à tout moment par les salariés, dès lors que leur employeur a prévu l'un des cas de dispense précités dans l'acte formalisant le régime, qu'ils en remplissent les conditions et en justifient.

9.2.2.   Les dispenses de droit

À leur initiative, les salariés peuvent se dispenser d'adhérer au contrat collectif s'ils répondent à l'un des cas de dispense dit « de droit ». Ces dispenses s'appliquent obligatoirement même si elles ne sont pas inscrites dans l'acte formalisant le régime au sein de l'entreprise.

Conformément aux dispositions des articles L. 911-7 et D. 911-2 du code de la sécurité sociale, les salariés suivants auront la faculté de refuser, à leur initiative, leur adhésion :

a) Les salariés bénéficiaires d'une couverture complémentaire en application de l'article L. 861-3 du code de la sécurité sociale (c'est-à-dire la « couverture santé solidaire » dite CSS). Cette dispense peut jouer jusqu'à la date à laquelle les salariés cessent de bénéficier de cette couverture.

b) Les salariés couverts par une assurance individuelle de frais de santé à titre principal ou d'ayants droit au moment de leur embauche. Cette dispense ne peut jouer que jusqu'à l'échéance du contrat individuel.

c) Les salariés bénéficiant pour les mêmes risques, y compris en qualité d'ayants droit, de prestations servies au titre d'un autre emploi, en tant que bénéficiaire de l'un ou l'autre des dispositifs suivants :

– dispositif collectif et obligatoire des salariés remplissant les conditions mentionnées à l'article L. 242-1 alinéa 6 du code de la sécurité sociale, étant précisé que pour les couples de salariés travaillant dans la même entreprise, l'un des deux membres du couple doit alors être affilié en propre, l'autre en qualité d'ayant droit ;

– dispositif prévu par le décret n° 2007-1373 du 19 septembre 2007 relatif à la participation de l'État et de ses établissements publics au financement de la protection sociale complémentaire de leurs personnels ou par le décret n° 2011-1474 du 8 novembre 2011 relatif à la participation des collectivités territoriales et de leurs établissements publics au financement de la protection sociale complémentaire de leurs agents ;

– contrat d'assurance groupe issu de la loi n° 94-126 du 11 février 1994 relative à l'initiative et à l'entreprise individuelle dit « contrat Madelin » ;

– régime local d'assurance maladie du Haut-Rhin, du Bas-Rhin et de la Moselle, en application des articles D. 325-6 et D. 325-7 du code de la sécurité sociale ;

– régime complémentaire d'assurance maladie des industries électriques et gazières en application du décret n° 46-1541 du 22 juin 1946.

d) Les salariés en contrat à durée déterminée dont la durée de la couverture collective à adhésion obligatoire dont ils bénéficient en matière de frais de santé est inférieure à trois mois, s'ils justifient bénéficier par ailleurs d'une couverture « responsable » au sens de l'article L. 871-1 du code de la sécurité sociale.

e) Les salariés embauchés avant la mise en place, par décision unilatérale, du régime prévoyant une cotisation salariale, en application de l'article 11 de la loi n° 89-1009 du 31 décembre 1989 dite « loi Évin ».

Les salariés peuvent se dispenser s'ils respectent les conditions prévues à l'article D. 911-2 du code de la sécurité sociale. Ces salariés ne pourront solliciter, par écrit, auprès de leur employeur, leur dispense d'adhésion qu'au moment de leur embauche ou de la prise d'effet des couvertures mentionnées aux a et d ci-dessus. La demande de dispense doit être accompagnée, le cas échéant, de tous justificatifs nécessaires.

À défaut de respecter les prescriptions détaillées ci-dessus, le salarié sera automatiquement affilié au contrat collectif à adhésion obligatoire.

==============================================================================
KALIARTI000050394887 | VIGUEUR_ETEN | du 2025-01-01 au 2999-01-01
chemin : Titre II Dispositions relatives aux garanties conventionnelles de frais de soins de santé
article : 10

Les salariés en contrat à durée déterminée d'une durée inférieure ou égale à trois mois, ou embauché en contrat de travail à temps partiel d'une durée de 15 heures par semaine ou moins peuvent se dispenser, à leur initiative, d'adhérer au contrat collectif, dans les conditions fixées à l'article L. 911-7, III, du code de la sécurité sociale, s'ils justifient bénéficier d'une couverture respectant les conditions fixées à l'article L. 871-1 du même code.

Sous réserve de respecter les conditions précitées, les salariés susvisés peuvent obtenir, de la part de leur employeur, un financement dit « versement santé », afin de participer à la prise en charge de la couverture santé individuelle responsable qu'ils auront souscrite par ailleurs.

Le versement se substitue ainsi à la participation patronale versée dans le cadre d'un contrat collectif et obligatoire, ainsi qu'à la portabilité.

Les modalités de calcul de ce versement sont fixées à l'article D. 911-8 du code de la sécurité sociale. Ce versement santé n'est pas cumulable avec le bénéfice de la couverture santé solidaire, le bénéfice d'une couverture collective et obligatoire, y compris en tant qu'ayant droit, ou d'une couverture complémentaire donnant lieu à la participation financière d'un employeur du secteur public.

==============================================================================
KALIARTI000050394891 | VIGUEUR_ETEN | du 2025-01-01 au 2999-01-01
chemin : Titre II Dispositions relatives aux garanties conventionnelles de frais de soins de santé
article : 11

Le présent accord, est mis en œuvre conformément aux prescriptions des articles L. 871-1, R. 871-1, R. 871-2, L. 242-1, R. 242-1 et D. 242-1 et suivants du code de la sécurité sociale, ainsi que de l'article 83-1° quater du code général des impôts.

==============================================================================
KALIARTI000050394899 | VIGUEUR_ETEN | du 2025-01-01 au 2999-01-01
chemin : Titre II Dispositions relatives aux garanties conventionnelles de frais de soins de santé
article : 11.1

Le niveau minimal de garanties de remboursement de frais de soins de santé visé à l'article 6 du présent accord, ou socle minimal de garanties obligatoires, est défini par la grille présentée en annexe 1 du présent accord.

Par dérogation, comme le prévoit l'article 6 du présent accord, les salariés des entreprises relevant de l'annexe II de la convention collective bénéficient d'un socle minimal de garanties obligatoires supérieur qui est défini par l'annexe 2 des présentes. Cette annexe II tient compte des spécificités du secteur en termes d'organisation du travail (contrat de travail, durée du travail et classification professionnelle).

==============================================================================
KALIARTI000050394900 | VIGUEUR_ETEN | du 2025-01-01 au 2999-01-01
chemin : Titre II Dispositions relatives aux garanties conventionnelles de frais de soins de santé
article : 11.2

Des garanties additionnelles peuvent venir compléter le socle minimal de garanties de remboursement de frais de soins de santé défini à l'article 11.1 du présent accord.

Les entreprises entrant dans le champ d'application du présent accord, peuvent mettre en place ces garanties additionnelles, à titre obligatoire ou facultatif, dans le respect du dialogue social.

==============================================================================
KALIARTI000050394901 | VIGUEUR_ETEN | du 2025-01-01 au 2999-01-01
chemin : Titre II Dispositions relatives aux garanties conventionnelles de frais de soins de santé
article : 12

Le présent article institue des prestations à caractère non directement contributif présentant un degré élevé de solidarité, pouvant notamment prendre la forme d'une prise en charge partielle ou totale de la cotisation pour certains salariés, d'une politique de prévention, ou encore de prestations d'action sociale.

Les signataires affirment leur attachement à la mise en œuvre d'une politique d'action sociale correspondant à la situation spécifique de la branche, et venant en supplément des actions pouvant déjà être développées par ailleurs.

==============================================================================
KALIARTI000050394902 | VIGUEUR_ETEN | du 2025-01-01 au 2999-01-01
chemin : Titre II Dispositions relatives aux garanties conventionnelles de frais de soins de santé
article : 12.1

La commission visée à l'article 12.3 chargée du suivi du dispositif définit annuellement les axes prioritaires de branche et les niveaux de prise en charge correspondants.

==============================================================================
KALIARTI000050394903 | VIGUEUR_ETEN | du 2025-01-01 au 2999-01-01
chemin : Titre II Dispositions relatives aux garanties conventionnelles de frais de soins de santé
article : 12.2

Les entreprises entrant dans le champ d'application du présent accord sont tenues d'affecter au financement des actions et prestations du degré élevé de solidarité (DES) au moins 2 % de la cotisation nettes de taxes (HT) sur les cotisations d'assurance des contrats collectifs frais de santé, au titre des garanties minimales et des garanties additionnelles obligatoires, ou un budget équivalent. Les actions et prestations sont financées dans la limite des fonds disponibles.

Les éléments relevant du degré élevé de solidarité seront mentionnés dans la notice d'information remise au salarié par l'employeur et établie par l'organisme assureur. La transmission de ces informations par les organismes assureur peut être prévue dans les contrats collectifs qui lient les organismes assureurs à l'employeur.

Cette part de la cotisation globale est collectée directement par l'organisme assureur sur les cotisations qu'il perçoit des entreprises.

Au moins une fois par an, les entreprises établissent, selon le périmètre de consolidation qu'elles retiennent (groupe, entreprise ou établissement), un document par lequel elles retracent les éléments de financement consacrés aux prestations à caractère non directement contributif détaillant le degré élevé de solidarité, ainsi que la liste des actions et des prestations déployées ou proposées durant l'exercice écoulé.

==============================================================================
KALIARTI000050394904 | VIGUEUR_ETEN | du 2025-01-01 au 2999-01-01
chemin : Titre II Dispositions relatives aux garanties conventionnelles de frais de soins de santé
article : 12.3

Le suivi des régimes de protection sociale complémentaire et de l'action sociale de branche est assuré par la commission paritaire permanente de négociation et d'interprétation (CPPNI). Cette commission se réunit autant de fois que nécessaire et fixe au moins une fois par an à l'ordre du jour le suivi de ces dispositifs et leur évolution.

==============================================================================
KALIARTI000050394905 | VIGUEUR_ETEN | du 2025-01-01 au 2999-01-01
chemin : Titre II Dispositions relatives aux garanties conventionnelles de frais de soins de santé
article : 13

Lorsque les garanties minimales obligatoires sont mises en place, au sein de l'entreprise, l'employeur doit s'engager au minimum à hauteur de 50 % de la cotisation globale venant financer la couverture minimale instituée à titre obligatoire.

Pour le socle la cotisation globale du régime frais de santé s'élève à :

– 1,29 % du PMSS au 1er janvier 2025 ;

– 1,44 % du PMSS au 1er janvier 2026 ;

– 1,60 % du PMSS au 1er janvier 2027 ;

– 1,75 % du PMSS au 1er janvier 2028 ;

Pour l'annexe II la cotisation globale du régime frais de santé s'élève à :

– 1,74 % du PMSS au 1er janvier 2025 ;

– 1,84 % du PMSS au 1er janvier 2026 ;

– 1,94 % du PMSS au 1er janvier 2027 ;

Les entreprises peuvent négocier ou mettre en place, à leur initiative, dans le respect des procédures prévues à l'article L. 911-1 du code de la sécurité sociale, une répartition plus favorable aux salariés.

Lorsque les garanties additionnelles sont mises en place, à titre obligatoire, au sein de l'entreprise, l'employeur doit s'engager au minimum à hauteur de 50 % de la cotisation globale venant financer la couverture instituée à titre obligatoire.

Lorsque les garanties additionnelles sont mises en place, à titre facultatif, au sein de l'entreprise, la cotisation afférente est intégralement à la charge des salariés ayant choisi d'opter pour ce niveau supérieur de garanties.

==============================================================================
KALIARTI000050394907 | VIGUEUR_ETEN | du 2025-01-01 au 2999-01-01
chemin : Titre II Dispositions relatives aux garanties conventionnelles de frais de soins de santé
article : 14

Conformément aux articles L. 932-6 du code de la sécurité sociale, L. 141-4 du code des assurances et L. 221-6 du code de la mutualité, l'employeur, en sa qualité de souscripteur, est tenu de remettre, à chaque salarié et à tout nouvel embauché, une notice d'information détaillée, établie par l'organisme assureur, résumant notamment les garanties et leurs modalités d'application.

Les prestations, décrites dans la notice d'information remise au salarié par l'employeur, relèvent de la seule responsabilité de l'organisme assureur, l'engagement de l'employeur ne portant que sur le paiement des cotisations et sur le respect des obligations imposées par les articles L. 911-7, II, et suivants du code de la sécurité sociale.

Les salariés seront informés, préalablement et individuellement, selon la même méthode, de toute modification de leurs droits et obligations.

==============================================================================
KALIARTI000050394912 | VIGUEUR_ETEN | du 2025-01-01 au 2999-01-01
chemin : Titre II Dispositions relatives aux garanties conventionnelles de frais de soins de santé
article : 15

Le salarié cesse de bénéficier des garanties de remboursement de frais de soins de santé prévues par le présent accord dans les cas suivants :

– en cas de rupture du contrat de travail, à l'exception des périodes de portabilité visées à l'article 5 du présent accord ;

– en cas de suspension du contrat de travail non indemnisée visée à l'article 4.2 du présent accord ;

– en cas de liquidation de la pension de retraite relevant d'un régime obligatoire de sécurité sociale, à l'exception des cas de cumul de ladite pension de retraite avec une activité salariée, notamment en cas de cumul-emploi retraite ou de retraite progressive ;

– en cas de non-paiement de la cotisation frais de santé par l'entreprise, dans les conditions prévues par les dispositions légales applicables en la matière ;

– en cas de décès du salarié ;

– en cas de résiliation du contrat d'assurance.

==============================================================================
KALIARTI000050394913 | VIGUEUR_ETEN | du 2025-01-01 au 2999-01-01
chemin : Titre II Dispositions relatives aux garanties conventionnelles de frais de soins de santé
article : 16

Le suivi des régimes de protection sociale complémentaire de branche est assuré par la commission paritaire permanente de négociation et d'interprétation (CPPNI).

Cette commission se réunit autant de fois que nécessaire. Elle se rencontre a minima deux fois par an pour prendre connaissance :

– au premier semestre, les résultats au 31 décembre de l'année passée ;

– au second semestre la présentation des comptes prévisionnels.

Les enjeux d'actualités et les éventuels impacts sur le régime lui seront présentés.

==============================================================================
KALIARTI000050394914 | VIGUEUR_ETEN | du 2025-01-01 au 2999-01-01
chemin : Titre II Dispositions relatives aux garanties conventionnelles de frais de soins de santé
article : 17

Le présent accord entre en vigueur à compter du 1er janvier 2025. Le présent accord se substituera pleinement à compter de cette date aux dispositions prévues dans les différents accords et leurs avenants portant sur le même objet et notamment ceux listés ci-après :

– le titre XIII de la convention collective nationale des entreprises du secteur privé du spectacle vivant ;

– accord du 15 décembre 2015 relatif au régime collectif obligatoire de frais de santé ;

– article IV-3 de l'annexe II de la convention collective nationale des entreprises du secteur privé du spectacle vivant du 12 février 2012 et ses avenants du 19 juin 2012 et du 12 octobre 2017 ;

– accord du 2 juillet 2001 relatifs aux frais de santé des théâtres privés modifié par l'avenant du 3 juillet 2013.

Ainsi, à l'issue de l'entrée en vigueur du présent accord, les accords mentionnés ci-dessus seront annulés et remplacés par ce même accord.

Suivant les dispositions de l'article L. 2261-23-1 du code du travail, les parties signataires n'ont pas retenu de dispositions spécifiques telles que visées par l'article L. 2232-10-1 du code du travail à l'attention des entreprises de moins de 50 salariés, dès lors que le présent accord vise à modifier le régime collectif obligatoire de frais de santé dont doivent bénéficier les salariés relevant de la convention collective et ce quel que soit l'effectif de leur entreprise.

==============================================================================
KALIARTI000050394917 | VIGUEUR_ETEN | du 2025-01-01 au 2999-01-01
chemin : Titre II Dispositions relatives aux garanties conventionnelles de frais de soins de santé
article : 18

Le présent accord fera l'objet des formalités de notification, publicité et dépôt conformément aux dispositions légales et réglementaires. Les signataires du présent accord demandent au ministère du travail son extension à toutes les entreprises entrant dans le champ d'application de la convention collective nationale des entreprises du secteur privé du spectacle vivant sur l'ensemble des départements français, y compris les collectivités d'outre-mer.

Les partenaires sociaux de la branche rappellent que dans l'attente de l'extension du présent accord, celui-ci s'appliquera à compter de la date fixée par l'article 17 des présentes, aux entreprises adhérentes des organisations patronales signataires.

==============================================================================
KALIARTI000050394918 | VIGUEUR_ETEN | du 2025-01-01 au 2999-01-01
chemin : Annexes
article : (sans numero)

Annexe 1

Tableau de garantie socle minimal ensemble du personnel permanent hors salariés de l'annexe II

Les remboursements interviennent dans la limite des frais restant à charge après remboursement de la sécurité sociale et d'éventuels organismes complémentaires. Leurs montants sont exprimés y compris le remboursement de la sécurité sociale.

(Tableau non reproduit, consultable en ligne sur le site Légifrance, rubrique Bulletins officiels des conventions collectives.)

 https://www.legifrance.gouv.fr/download/file/pdf/boc_20240039_0000_0006.pdf/BOCC

==============================================================================
KALIARTI000050394919 | VIGUEUR_ETEN | du 2025-01-01 au 2999-01-01
chemin : Annexes
article : (sans numero)

Annexe 2

Tableau de garantie socle minimal salariés de l'annexe II

Les remboursements interviennent dans la limite des frais restant à charge après remboursement de la sécurité sociale et d'éventuels organismes complémentaires. Leurs montants sont exprimés y compris le remboursement de la sécurité sociale.

(Tableau non reproduit, consultable en ligne sur le site Légifrance, rubrique Bulletins officiels des conventions collectives.)

 https://www.legifrance.gouv.fr/download/file/pdf/boc_20240039_0000_0006.pdf/BOCC


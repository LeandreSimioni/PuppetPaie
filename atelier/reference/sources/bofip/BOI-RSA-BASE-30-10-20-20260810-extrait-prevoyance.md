# BOI-RSA-BASE-30-10-20 (version du 10/08/2026) — extrait : prévoyance complémentaire, § 200 à 240

| | |
|---|---|
| source | https://bofip.impots.gouv.fr/bofip/5956-PGP.html/identifiant=BOI-RSA-BASE-30-10-20-20260810 |
| page brute | `BOI-RSA-BASE-30-10-20-20260810.html` (même dossier, telle que servie) |
| relevé le | 2026-10-10 (séance cloud, corrections DSN vague 1b) |
| ce qu'on en lit | § 200 : la part patronale de prévoyance correspondant à des garanties **frais de santé** (maladie, maternité, accident) est un complément de rémunération **imposable** ; la part couvrant incapacité, invalidité, décès, dépendance est déductible sous plafond. § 230 : la déduction suppose des garanties « auxquelles le salarié est **affilié à titre obligatoire** ». |
| où ça sert | `calcul.rs` (`reintegrations` : la part patronale `mutuelle` entre dans le net imposable, `S21.G00.50.002`) ; la note `_source` de la ligne `prevoyance` des intermittents dans `baremes/2026.json` (le Fonds collectif du spectacle pour la santé n'est pas une garantie à laquelle le salarié est affilié : pas réintégré, inférence sur un cas que le BOFiP ne nomme pas) |

## Texte (transcription de la page, sans mise en forme)

II. Cotisations versées aux régimes de prévoyance complémentaire
A. Rappel des conditions de déduction
200
Le 1° quater de l’article 83 du CGI autorise la déduction, sous plafond :
des cotisations ou primes à la charge des salariés ;
de la part des cotisations à la charge des employeurs et, le cas échéant, du comité d’entreprise, correspondant à des garanties autres que celles portant sur le remboursement et l’indemnisation de frais de santé. Il s’agit, en pratique, des cotisations versées au titre des garanties incapacité, invalidité, décès et dépendance, versées aux régimes de prévoyance complémentaire mis en place dans l’entreprise dans les conditions prévues à l’article L. 911-1 du CSS auxquels le salarié est affilié à titre obligatoire.
Remarque : Les cotisations à la charge de l’employeur et, le cas échéant, du comité d’entreprise qui correspondent à des garanties portant sur le remboursement ou l’indemnisation de frais de santé (maladie, maternité ou accident) constituent des compléments de rémunération qui sont compris dans la rémunération imposable du salarié (BOI-RSA-CHAMP-20-30-50).
La déduction est admise si les conditions suivantes sont remplies.
1. Conditions liées aux contrats desrégimes de prévoyance complémentaire
a. Champ de la prévoyance complémentaire
210
Les contrats doivent concerner des prestations complémentaires de prévoyance destinées à couvrir les risques décès, les risques portant atteinte à l’intégrité physique de la personne ou liés à la maternité, les risques d’incapacité de travail ou d’invalidité, les risques d’inaptitude et de maladie et le risque chômage.
Sont également assimilées à de la prévoyance les prestations dépendance visant à faire face à la perte d’autonomie ou des prestations destinées aux proches aidants.
Les prestations accordées peuvent revêtir des formes différentes (par exemple, pension d’orphelin, rente éducation, etc.).
Remarque : Pour plus de précisions sur le champ des prestations complémentaires de prévoyance concernées, il convient de se reporter au A de la section 2 du chapitre 2 de la rubrique « Protection sociale complémentaire » du BOSS.
b. Nature des garanties portant sur les frais de santé
215
L’article L. 871-1 du CSS dispose que les contrats de régimes de prévoyance complémentaire mis en place dans les entreprises :
ne couvrent pas la prise en charge (même partielle) de la participation forfaitaire et de la franchise, respectivement prévues au II et au III de l’article L. 160-13 du CSS ;
ne couvrent pas la prise en charge (totale ou partielle) de certaines majorations de participation ou dépassements d’honoraires (CSS, art. R. 871-1) ;
peuvent couvrir la prise en charge totale ou partielle de certaines prestations ou dépassements d’honoraires définis à l’article R. 871-2 du CSS.
Remarque : Pour plus de précisions sur la nature des garanties couvertes par le contrat, il convient de se reporter au B de la section 2 du chapitre 2 de la rubrique « Protection sociale complémentaire » du BOSS.
c. Faculté de rachat
220
Sauf pour ce qui concerne la couverture du risque décès ou en cas d’invalidité absolue et définitive ou perte totale et irréversible d’autonomie correspondant au 3° de l’article L. 341-4 du CSS, les contrats doivent être exclusifs de tout versement en capital.
2. Conditions liéesau caractère obligatoire des garanties mises en place
230
Aux termes des dispositions du premier alinéa du 1° quater de l’article 83 du CGI, le bénéfice de la déduction des cotisations ou primes aux régimes de prévoyance complémentaire mis en place dans l’entreprise est subordonné à la condition que ces sommes financent des garanties auxquelles le salarié est affilié à titre obligatoire.
235
Le caractère obligatoire du régime n’est pas remis en cause lorsque l’acte instituant le régime prévoit des cas de dispense en application des dispositions du septième au neuvième alinéa de l’article R. 242-1-6 du CSS.
240
Il est admis que le caractère obligatoire du régime ne soit pas remis en cause lorsque celui-ci prévoit, pendant douze mois au maximum, le maintien temporaire du bénéfice des garanties au profit des salariés dont le contrat de travail est rompu conformément aux dispositions de l’article L. 911-8 du CSS.
Ainsi, dès lors que le maintien de la couverture complémentaire correspond à la poursuite du contrat obligatoire et collectif dont bénéficiaient les salariés avant la rupture de leur contrat de travail, il est admis que les cotisations versées dans le cadre du maintien des droits à couverture complémentaire santé et prévoyance des anciens salariés au chômage prévu par l’article L. 911-8 du CSS restent déductibles de leur revenu imposable sur la base et dans les conditions du 1° quater de l’article 83 du CGI.
Les plafonds s’apprécient selon les modalités de droit commun, c’est-à-dire en prenant en compte la rémunération annuelle brute du salarié l’année de paiement des cotisations par le salarié ou par l’employeur à l’organisme de prévoyance.
Les cotisations versées aux régimes de prévoyance complémentaire qui excèdent les plafonds de déduction, ainsi que celles qui correspondent au maintien de la couverture pendant une durée supérieure à 12 mois, ne peuvent pas être admises en déduction sur le fondement du 1° quater de l’article 83 du CGI. La part patronale doit être réintégrée dans la rémunération imposable et la part salariale n’est pas déductible des revenus soumis à l’impôt sur le revenu.
3. Conditions liées au caractère collectif des garanties mises en place
a. Les garanties doivent concerner l’ensemble du personnel salarié ou une catégorie objective de salariés

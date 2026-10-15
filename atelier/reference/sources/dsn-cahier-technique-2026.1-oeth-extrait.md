# DSN, cahier technique 2026.1 : ce qui concerne l'OETH

Extrait le 25/09/2026 de `atelier/reference/norme-dsn/dsn-cahier-technique-2026.1.pdf` : bloc S21.G00.13 (Complément OETH) et codes 060 à 073 du bloc S21.G00.82 (contrôles CCH-21, -22, -24).

```
[15,34] 
CSL 00 : [A-Z0-9]+ 
 
 
S21.G00.13 
Complément OETH 
S21.G00.13 
 
 
 
Ce bloc est à renseigner dans le cas où l’entreprise est concernée par un accord agréé au titre de l’obligation d’emploi 
des travailleurs handicapés et/ou par la déclaration de bénéficiaires de l’obligation d’emploi des travailleurs handicapés 
(BOETH) mis à disposition de l’entreprise. 
Il peut également être renseigné dans le cas où la déclaration des stagiaires BOETH accueillis dans l'entreprise n'a pas 
pu être effectuée au préalable dans les DSN mensuelles de l'année concernée. Si les stagiaires BOETH ont déjà été 
déclarés à maille nominative dans les déclarations précédentes, ils ne doivent pas être déclarés dans ce bloc. 
 
 
 
Accord agréé OETH 
S21.G00.13.001 
Type BOETH externe 
S21.G00.13.002 
Nombre BOETH externe 
S21.G00.13.003 
Millésime de rattachement 
S21.G00.13.004 
 
 
 
Accord agréé OETH 
 
S21.G00.13.001 
Oeth.Accord 
 
Cette rubrique est à renseigner dans le cas où l’entreprise est concernée par un accord agréé au titre de 

=== PAGE 150
 
 
 
 
 
S21.G00.13  
2025-12-24 
 
 
 
150 / 361 
 
 
 
 
l’obligation d’emploi des travailleurs handicapés pour l’année de rattachement à laquelle le bloc 
"Complément OETH - S21.G00.13" est souscrit. 
 
CRE-11 : Valeurs autorisées 
 
 
 
X 
 
[12,12] 
 
Table AAETH - Codes des accords agréés pour l’emploi des 
travailleurs handicapés 
 
Type BOETH externe 
 
S21.G00.13.002 
Oeth.Type 
 
 
Cette rubrique permet de renseigner le type de Bénéficiaires de l’Obligation d’Emploi des Travailleurs 
Handicapés (BOETH) mis à disposition de l’entreprise pour l’année de rattachement à laquelle le bloc 
"Complément OETH - S21.G00.13" est souscrit. 
Cette rubrique permet également de déclarer les stagiaires BOETH accueillis dans l'entreprise n'ayant pas 
pu être déclarés au préalable dans les DSN mensuelles de l'année concernée. Si les stagiaires BOETH ont 
déjà été déclarés à maille nominative dans les déclarations précédentes, ils ne doivent pas être déclarés 
dans cette rubrique. 
 
 
X 
 
[2,2] 
 
 
 
 
 
01 - BOETH intérimaires 
02 - BOETH salariés d'un groupement d'employeurs mis à disposition 
03 - BOETH stagiaires n’ayant pas fait l’objet d’une déclaration nominative au préalable 
04 - BOETH salariés d’une association intermédiaire mis à disposition 
05 - BOETH salariés d’une agence de mannequin mis à disposition 
06 - BOETH salariés d’une entreprise de travail à temps partagé (ETTP) mis à disposition 
 
Nombre BOETH externe 
 
S21.G00.13.003 
Oeth.Nombre 
 
 
Cette rubrique permet de renseigner le nombre associé au type de Bénéficiaires de l’Obligation d’Emploi 
des Travailleurs Handicapés (BOETH) mis à disposition de l’entreprise pour l’année de rattachement à 
laquelle le bloc "Complément OETH - S21.G00.13" est souscrit. Cette rubrique permet également de 
déclarer le nombre de stagiaires BOETH accueillis dans l'entreprise n'ayant pas pu être déclarés au 
préalable dans les DSN mensuelles de l'année concernée. Si les stagiaires BOETH ont déjà été déclarés à 
maille nominative dans les déclarations précédentes, cette rubrique ne doit pas être renseignée. 
 
 
CCH-11 : Si la rubrique "Type BOETH - S21.G00.13.002" est renseignée alors la rubrique "Nombre BOETH externe - 
S21.G00.13.003" est obligatoire, et, inversement : si la rubrique "Nombre BOETH externe - S21.G00.13.003" est 
renseignée alors la rubrique "Type BOETH - S21.G00.13.002" est obligatoire. 
Si l'une de ces deux rubriques est absente, alors l'autre est interdite. 
 
 
N 
 
[4,8] 
CSL 00 : \d{1,5}\.\d{2} 
 
Millésime de rattachement 
 
S21.G00.13.004 
Oeth.Rattachement 
 
Année au titre de laquelle le bloc "Complément OETH - S21.G00.13" est souscrit. 
 
 
CCH-11 : Le "Millésime de rattachement - S21.G00.13.004" doit obligatoirement être inférieur ou égal aux quatre 
derniers caractères (Année) du mois principal déclaré au niveau de la rubrique "Date du mois principal déclaré – 
S20.G00.05.005". 
 
CCH-12 : Un bloc "Complément OETH - S21.G00.13" ne peut pas être déclaré avec la seule présence d'une rubrique 
"Millésime de rattachement - S21.G00.13.004". Il doit obligatoirement être renseigné une rubrique "Accord agréé 

=== PAGE 151
 
 
 
 
 
S21.G00.15  
2025-12-24 
 
 
 
151 / 361 
 
 
 
 
OETH - S21.G00.13.001" ou "Type BOETH externe - S21.G00.13.002". 
 
 
N 
 
[4,4] 
CSL 00 : 2([0-9]{3}) 
 

cotisations », « 025 - Contribution aux régimes supplémentaires de retraite à prestations définies - Rente », « 026 - 
Contribution aux régimes supplémentaires de retraite à prestations définies - Prime », « 027 - Contribution aux régimes 
supplémentaires de retraite à prestations définies - Dotations », « 028 - Contribution additionnelle sur les rentes 

=== PAGE 312
 
 
 
 
 
S21.G00.82  
2025-12-24 
 
 
 
312 / 361 
 
 
 
 
liquidées », « 029 - Contribution aux régimes supplémentaires de retraite à prestations définies. Rente à taux 7% », « 
030 - Contribution aux régimes supplémentaires de retraite à prestations définies. Rente à taux 14% », « 031 - 
Contribution additionnelle de solidarité pour l'autonomie », « 032 - Contribution Sociale généralisée au taux de 3,80% 
+ RDS sur revenu de remplacement », « 033 - Contribution Sociale généralisée au taux de 6,20% + RDS sur revenu de 
remplacement », « 034 - Contribution Sociale généralisée au taux de 6,60% + RDS sur revenu de remplacement », « 
035 - Contribution Sociale généralisée au taux de 7,50% + RDS sur revenu de remplacement », « 041 - Cotisation 
maladie sur les avantages de préretraite », « 042 - Cotisation maladie sur les avantages de retraite », « 043 - Cotisation 
maladie Alsace-Moselle sur les avantages de retraite  », « 044 - Cotisation forfait social à 8% », « 045 - Cotisation 
forfait social à 20% », « 047 - Cotisation forfait social à 16% », « 048 - Cotisation forfait social à 10% », « 049 - CSG 
au taux de 8.30% + RDS sur revenus de remplacement », « 050 - CSG au taux de 9.20% + RDS sur revenus de 
remplacement  », « 060 - Déduction ECAP », « 061 - Déduction de sous-traitance (EA, ESAT, TIH, portage salarial) », 
« 062 - Dépense déductible liée aux travaux d’accessibilité », « 063 - Dépense déductible liée au maintien et à la 
reconversion professionnelle », « 064 - Dépense déductible liée aux prestations d’accompagnement et de 
sensibilisation », « 065 - Contribution OETH brute avant déductions », « 066 - Contribution OETH nette », « 067 - 
Contribution OETH nette après écrêtement (pour les millésimes strictement antérieurs à 2025) », « 068 - Contribution 
OETH réelle due », « 069 - Dépenses OETH prévues par l’accord et non réalisées », « 071 - Dépense déductible liée à 
la participation à des événements », « 072 - Dépense déductible liée aux partenariats avec des associations », « 073 - 
Dépenses déductibles liées aux actions concourant à la professionnalisation et aux achats auprès des EA, ESAT, TIH », 
« 074 - Exonération Taxe d’apprentissage pour un employeur d’apprenti(s), dont la masse salariale est inférieure à 6 
SMICs », « 075 - Déductions relatives aux CFA d’entreprise et/ou au financement des offres nouvelles de formation 
par apprentissage (Art. L6241-2 code du travail) », « 076 - Solde de la taxe d'apprentissage versé en numéraire », « 077 
- Réduction du solde de la taxe d’apprentissage liée à des subventions aux CFA (Art. L6241-4 du code du travail) versé 
en nature », « 078 - Réduction du solde de la taxe d’apprentissage liée à des créances alternants (Art. L6241-4 du code 
du travail) », « 079 - Contribution supplémentaire à l’apprentissage (CSA) », « 080 - Exonération de la contribution 
supplémentaire à l’apprentissage (CSA) » et « 091 - Cotisation relative aux droits spécifiques passés des activités non 
régulées (DSPNR) » sont interdites dans la rubrique « Code de cotisation - S21.G00.82.002 ». 
 
CCH-21 : Si la rubrique « Code de cotisation – S21.G00.82.002 » est renseignée avec la valeur « 060 - Déduction 
ECAP », « 061 - Déduction de sous-traitance (EA, ESAT, TIH, portage salarial) », « 062 - Dépense déductible liée aux 
travaux d’accessibilité », « 063 - Dépense déductible liée au maintien et à la reconversion professionnelle », « 064 - 
Dépense déductible liée aux prestations d’accompagnement et de sensibilisation », « 065 - Contribution OETH brute 
avant déductions », « 066 - Contribution OETH nette », « 067 - Contribution OETH nette après écrêtement (pour les 
millésimes strictement antérieurs à 2025) », « 068 - Contribution OETH réelle due », « 071 - Dépense déductible liée à 
la participation à des événements », « 072 - Dépense déductible liée aux partenariats avec des associations » ou « 073 - 
Dépenses déductibles liées aux actions concourant à la professionnalisation et aux achats auprès des EA, ESAT, TIH », 
alors l’année renseignée au niveau des rubriques « Date de début de période de rattachement - S21.G00.82.003 » et « 
Date de fin de période de rattachement S21.G00.82.004 » doit être identique. 
Ce contrôle vise à interdire les déclarations qui portent sur deux exercices annuels différents pour les codes de 
cotisation cités ci-dessus. 
 
CCH-22 : Si un bloc « Cotisation établissement - S21.G00.82 » est renseigné avec la valeur « 065 - Contribution 
OETH brute avant déductions » au niveau de la rubrique « Code de cotisation - S21.G00.82.002 », alors un autre bloc 
« Cotisation établissement - S21.G00.82 » doit obligatoirement être déclaré avec la même période (S21.G00.82.003 et 
S21.G00.82.004) avec la valeur « 066 - Contribution OETH nette » au niveau de la rubrique « Code de cotisation - 
S21.G00.82.002 », et, inversement : si un bloc « Cotisation établissement - S21.G00.82 » est renseigné avec la valeur « 
066 - Contribution OETH nette » au niveau de la rubrique « Code de cotisation - S21.G00.82.002 », alors un autre bloc 
« Cotisation établissement - S21.G00.82 » doit obligatoirement être déclaré avec la même période (S21.G00.82.003 et 
S21.G00.82.004) avec la valeur « 065 - Contribution OETH brute avant déductions » au niveau de la rubrique « Code 
de cotisation - S21.G00.82.002 ». 
 
CCH-24 : Si un bloc "Cotisation établissement - S21.G00.82" est renseigné avec la valeur "068 - Contribution OETH 
réelle due" au niveau de la rubrique "Code de cotisation - S21.G00.82.002", alors un autre bloc "Cotisation 
établissement - S21.G00.82" doit obligatoirement être déclaré avec la même période (S21.G00.82.003 et 
S21.G00.82.004) avec la valeur "065 - Contribution OETH brute avant déductions" au niveau de la rubrique "Code de 
cotisation - S21.G00.82.002",  et, inversement : si un bloc "Cotisation établissement - S21.G00.82" est renseigné avec 
la valeur "065 - Contribution OETH brute avant déductions" au niveau de la rubrique "Code de cotisation - 
S21.G00.82.002", alors un autre bloc "Cotisation établissement - S21.G00.82" doit obligatoirement être déclaré avec la 
même période (S21.G00.82.003 et S21.G00.82.004) avec la valeur "068 - Contribution OETH réelle due" au niveau de 
la rubrique "Code de cotisation - S21.G00.82.002". 
 

```

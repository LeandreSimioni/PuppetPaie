"""Releve les articles de loi dont la paie a besoin, chez la DILA.

POURQUOI CET OUTIL. Legifrance refuse les robots (403) : les articles du
§38.3 avaient ete releves A LA MAIN, un par un, ce qui marche une fois et se
perime en silence. La base LEGI est publique en open data
(`echanges.dila.gouv.fr/OPENDATA/LEGI/`), exactement comme KALI pour les
conventions collectives : une archive globale et une livraison par soir. Le
relevé devient donc RE-VERIFIABLE automatiquement, comme la table des
parametres du BOSS et celle des CTP de l'Urssaf.

    python3 outils/relever_les_codes.py moissonner <dossier des sources>
    python3 outils/relever_les_codes.py rapport <dossier des sources> [code]
    python3 outils/relever_les_codes.py comparer <dossier des sources>

TROIS CODES, UN SEUL PASSAGE (19/09/2026). Le code du travail dit ce qu'une
indemnite de rupture VAUT ; le code de la securite sociale dit ce qu'elle
SUPPORTE, et le code general des impots ce qu'elle coute a celui qui la
recoit. Les trois se lisent dans la meme archive : ce qui coute, c'est de
faire defiler 1,1 Go, pas de reconnaitre un article de plus au passage. Le
dossier donne en argument est celui des SOURCES ; chaque code y ecrit dans le
sien.

`moissonner` TIENT UN ENTREPOT A JOUR (journal §122) : `legi-entrepot/` dans
le dossier des sources garde TOUS les articles en vigueur des quatre codes et
la liste des livraisons deja appliquees. La premiere fois (ou avec
`--reconstruire`), l'archive globale defile -- 1,1 Go, un quart d'heure --,
puis les livraisons depuis sa date ; ensuite, seules les livraisons
nouvelles se telechargent (quelques secondes pour une semaine). Ajouter un
article a `TEXTES` ne demande plus aucun telechargement : il est deja dans
l'entrepot. Avant le 24/09/2026, chaque passage relisait tout, une heure. `rapport` ecrit le document lisible a partir de ce
qui a ete moissonne.

`comparer` REPOND A LA QUESTION QUE `moissonner` POSE (21/09/2026).
« Huit articles ont change de version » ne dit pas si le DROIT a change : un
article dont un alinea etranger a la paie est reecrit recoit un identifiant
neuf sans que rien de ce qui nous concerne bouge. Il faut donc relire chaque
article contre sa version d'AVANT -- et cette version-la ne se trouve nulle
part ailleurs que dans l'archive globale. Le relevé ne garde que la version
en vigueur ; l'historique Git ne l'a pas non plus, parce qu'un relevé
precedent portait deja, lui aussi, la version d'apres. Comptez un quart
d'heure : `comparer` s'arrete des qu'il les a toutes.

LE PIEGE PROPRE A `comparer` : on cherche par IDENTIFIANT, jamais par numero.
L'archive globale porte plusieurs versions du meme article ; demander
« L242-1 » y rapporte un paquet de redactions dont on ne saurait laquelle
etait en vigueur. Ce qu'on veut n'est pas « une vieille version », c'est
CELLE que le relevé precedent tenait pour en vigueur -- celle que
`_changes_depuis_le_global` nomme sous `avant`.

TROIS PIEGES, payes tous les trois :

  1. le dossier du dump s'appelle `code_en_vigueur`, mais il porte AUSSI les
     versions anterieures de chaque article. La version qui vaut est celle
     dont <ETAT> est VIGUEUR et <DATE_FIN> 2999-01-01 -- et `etat` existe
     aussi comme ATTRIBUT, sur d'autres balises, ce qui fait attraper
     n'importe quoi. Premiere version de ce script : sept articles sur
     cinquante-cinq gardes dans une version perimee, dont L3141-3 (les 2,5
     jours de conges) et R1234-2 (l'indemnite de licenciement) ;
  2. un article modifie recoit un NOUVEL identifiant de version. On filtre
     donc les livraisons sur le NUMERO d'article, pas sur l'identifiant ;
  3. un numero d'article n'appartient a aucun code par lui-meme. `L242-1`
     est un article du code de la securite sociale, et rien n'interdit a un
     autre code d'en porter un du meme nom. Ce qui tranche, c'est le
     LEGITEXT dans le chemin du fichier -- pas le numero.
"""
import json
import os
import re
import sys
import tarfile
import time
import urllib.request

# L'entrepot commun (journal §122) vit a cote de ce script.
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

BASE = "https://echanges.dila.gouv.fr/OPENDATA/LEGI/"
GLOBAL = "Freemium_legi_global_20250713-140000.tar.gz"
DEPUIS = "20250713"
XML = "xml"                  # le sous-dossier des pieces justificatives

# LES CODES QU'ON LIT, et ou leur relevé se depose. Un seul passage de
# l'archive les sert TOUS : ce qui coute, c'est de faire defiler 1,1 Go, pas
# de reconnaitre un article de plus au passage.
#
# CE QUE LA PAIE A BESOIN DE LIRE. Un article ajoute ici sera moissonne au
# prochain passage ; rien n'est devine, tout est relevé.
TEXTES = {
    "travail": {
        "legitext": "LEGITEXT000006072050",
        "nom": "Code du travail",
        "dossier": "code-du-travail",
        "titre": "Code du travail -- ce que la paie lit",
        "voulus": {
            "conges": ["L3141-3", "L3141-4", "L3141-5", "L3141-5-1",
                       "L3141-6", "L3141-9", "L3141-10", "R3141-4",
                       "L3141-24", "L3141-28"],
            "feries": ["L3133-1", "L3133-2", "L3133-3", "L3133-4", "L3133-5",
                       "L3133-6"],
            "maintien": ["L1226-1", "D1226-1", "D1226-2", "D1226-3",
                         "D1226-4", "D1226-5", "D1226-6", "D1226-7",
                         "D1226-8"],
            "precarite": ["L1242-2", "L1242-12", "L1243-8", "L1243-9",
                          "L1243-10"],
            "rupture": ["L1234-1", "L1234-4", "L1234-5", "L1234-9",
                        "R1234-1", "R1234-2", "R1234-4"],
            "documents": ["L1234-19", "L1234-20", "D1234-6", "D1234-7",
                          "D1234-8"],
            "retraite": ["L1237-5", "L1237-6", "L1237-7", "L1237-8",
                         "L1237-9", "L1237-10", "D1237-1", "D1237-2",
                         "D1237-2-1"],
            "rupture_conventionnelle": ["L1237-11", "L1237-12", "L1237-13",
                                        "L1237-14", "L1237-15", "L1237-16"],
            # LE BULLETIN DE PAIE (21/09/2026). Le §11.1 tient une decision
            # a une reference citee de memoire : << tout paiement de salaire
            # s'accompagne d'un bulletin (L3243-2) >>, et c'est elle qui
            # commande le bulletin RECTIFICATIF du mois d'origine. Une
            # decision qui repose sur un article jamais relu est une
            # decision qu'on ne peut pas defendre.
            #
            # R3243-3 N'EST PAS DE LA LISTE, et ce n'est pas un oubli : le
            # passage du 21/09/2026 l'a rendu dans une version ABROGEE
            # depuis le 27/02/2016 (LEGIARTI000018533802). Il ne s'agissait
            # pas du piege 1 de l'en-tete -- l'article n'a plus AUCUNE
            # version en vigueur a trouver. Le laisser ici ferait dire
            # << perime >> au garde-fou a chaque passage, pour un article
            # qui n'existe plus : le bruit finirait par couvrir le vrai
            # signal, celui d'un article vivant qu'on aurait garde vieux.
            # Ce qu'il disait -- la mention des cotisations PATRONALES sur
            # le bulletin -- se lit desormais dans R3243-1, qui est de la
            # liste et en vigueur.
            # R3243-4 A R3243-6 (lot R11, 11/10/2026) : ce que le bulletin ne
            # peut pas dire (la greve, la representation), et la seule
            # derogation qui dispense de la part patronale (le particulier
            # employeur, R3243-6) -- lus dans l'entrepot pour trancher les
            # modeles de bulletin ; a relever au prochain `moissonner`.
            "bulletin": ["L3243-1", "L3243-2", "L3243-3", "L3243-4",
                         "R3243-1", "R3243-2", "R3243-4", "R3243-5",
                         "R3243-6"],
            # L'EFFECTIF DU CSE (23/09/2026, §80.1). Les seuils de 11 et 50
            # de la CCNEAC -- et le FNAS qui en decoule -- sont ceux du comite
            # social et economique, que la convention rattache au code du
            # travail (elle cite L2315-61). L'effectif s'y compte selon
            # L1111-2, et le seuil de onze doit tenir douze mois consecutifs
            # (L2311-2) : ce n'est PAS l'effectif de l'Urssaf (L130-1 du code
            # de la securite sociale, plus bas). C'est ce qui fait que
            # `calcul.representation_du_personnel` prend la representation
            # DECLAREE pour un fait, sans l'opposer au CRM 118.
            "effectif_cse": ["L1111-2", "L2311-2", "L2315-61"],
            # LES MAJORATIONS DES EMPLOYEURS AFFILIES A UNE CAISSE DE CONGES
            # (10/10/2026, lot V1n). L243-1-3 du code de la securite sociale
            # cite ces articles-la : la caisse (L3141-32), les contributions
            # de formation et d'apprentissage qu'il majore (L6131-2, L6242-1,
            # L6331-6), la taxe d'apprentissage (L6241-1) et les intermittents
            # du spectacle (L6331-55) que D243-0-2 met a 10 %.
            # D7121-28, D7121-40 et D7121-41 : qui s'affilie aux Conges
            # Spectacles, et pour qui -- le personnel artistique et technique
            # qu'il n'a pas employe de facon continue pendant les douze mois
            # precedant la demande de conge (la lettre circulaire Acoss 2013-019,
            # annexe 1, R5, y borne la majoration).
            "majoration_caisses": ["L3141-32", "L6131-2", "L6241-1",
                                   "L6242-1", "L6331-6", "L6331-55",
                                   "D7121-28", "D7121-40", "D7121-41"],
            # LE CHOMAGE ET L'AGS D'UN PERMANENT (10/10/2026, lot V2e).
            # L5422-9 : les contributions << sont assises sur les remunerations
            # brutes dans la limite d'un plafond >> ; L5422-20 renvoie ses
            # mesures d'application a la convention d'assurance chomage, dont le
            # reglement general (art. 49) fixe ce plafond a quatre fois celui de
            # L. 241-3 du code de la securite sociale ; L5422-14 : l'employeur
            # declare les remunerations qui servent au calcul. L3253-18 : l'AGS
            # est assise sur les memes remunerations que le chomage.
            "chomage": ["L5422-9", "L5422-14", "L5422-20", "L3253-18"],
            # LE BLOC S21.G00.44 DE LA DSN DE DECEMBRE (10/10/2026, lot V3b) :
            # qui est assujetti a quoi. L6131-1 : tout employeur concourt a la
            # formation (CFP, CPF-CDD, contribution supplementaire), sauf ceux
            # de son II (Etat, collectivites et leurs EPA, hopitaux publics,
            # employeur sans etablissement en France) ; L6241-1-1 : l'assiette
            # de la taxe d'apprentissage ; L6331-1 et L6331-3 : la CFP a moins
            # et a partir de onze salaries. L6131-2, L6241-1, L6242-1, L6331-6
            # et L6331-55 sont deja releves plus haut (`majoration_caisses`).
            # D6331-72 : les CDD qui ne donnent pas lieu a la contribution
            # CPF-CDD (contrats aides, apprentissage, professionnalisation, et
            # ceux de L6321-9) ; le CDD saisonnier est exclu par L6331-6 meme.
            # L6331-1 A (13/10/2026, lot F1) : l'effectif de la CFP et le
            # franchissement de son seuil de onze se comptent selon L130-1 du
            # code de la securite sociale -- cinq ans pour le franchir a la
            # hausse, et une annee sous le seuil fait recourir le delai (le
            # controle `seuils_effectif` de `contexte.rs`).
            "assujettissement_fiscal": ["L6131-1", "L6241-1-1", "L6331-1 A",
                                        "L6331-1", "L6331-3", "D6331-72",
                                        "L6321-9"],
            # LE TEMPS PARTIEL ET LA DUREE LEGALE (10/10/2026, lot V4b). R242-2
            # du code de la securite sociale permet a l'employeur de corriger
            # le plafond des salaries de L3123-1 a due proportion de leur
            # duree contractuelle, majoree des heures complementaires de
            # L3123-8, L3123-20, L3123-21 et L3123-28 : S21.G00.40.084 de la
            # norme 2027 declare ce choix. L3121-27 (35 h) et L3121-28 (au-dela,
            # une heure supplementaire) : la quotite de reference 2027
            # (S21.G00.40.012) << ne doit pas inclure les heures
            # supplementaires structurelles >>.
            "temps_partiel": ["L3121-27", "L3121-28", "L3123-1", "L3123-8",
                              "L3123-20", "L3123-21", "L3123-28"],
            # LES CONGES POUR EVENEMENTS FAMILIAUX (10/10/2026, lot V4b) : ils
            # << n'entrainent pas de reduction de la remuneration >> (L3142-2).
            # C'est ce que dit `remunere` du motif 637 (`permanents-base.json`),
            # et en 2027 le bloc S21.G00.65 ne declare plus qu'une absence << sans
            # maintien de sa remuneration normale >>.
            "evenements_familiaux": ["L3142-1", "L3142-2"],
            # LE REGISTRE UNIQUE DU PERSONNEL (11/10/2026, reste R1 #147) : ses
            # mentions (D1221-23 : la nationalite, 1°) sont ce que
            # `etats_personnel::registre_du_personnel` ecrit ; il citait les
            # articles sans qu'ils soient dans le releve.
            "registre": ["L1221-13", "D1221-23", "D1221-24"],
            # LA FIN D'UN CDD (13/10/2026, lot U5) : L1243-5 la fait cesser de
            # plein droit a l'echeance du terme (aucun preavis : L1234-1 est
            # celui du licenciement) ; L1243-1 et L1243-2 disent les ruptures
            # avant le terme, dont le preavis du salarie qui a trouve un CDI.
            # Lot U7 : L1243-3 et L1243-4 (ce que coute une rupture anticipee
            # hors des cas de L1243-1), L1226-4-2, L1226-4-3 et L1226-20 (la
            # rupture d'un CDD pour inaptitude, 033) : le preavis de chaque
            # motif de rupture anticipee.
            "fin_de_cdd": ["L1243-1", "L1243-2", "L1243-3", "L1243-4", "L1243-5",
                           "L1226-4-2", "L1226-4-3", "L1226-20"],
            # LE PREAVIS NON EFFECTUE, MOTIF PAR MOTIF (13/10/2026, lot P1).
            # L1234-5 est au chapitre du licenciement ; ce que les autres fins
            # de contrat en font est dans ces articles-ci : la demission
            # (L1237-1) et sa rupture abusive (L1237-2), l'inaptitude non
            # professionnelle (L1226-4 : << par derogation a l'article
            # L. 1234-5 >>, pas d'indemnite), l'inaptitude professionnelle
            # (L1226-12, L1226-14 : une indemnite d'un montant egal), la force
            # majeure d'un CDI (L1234-12, L1234-13), la periode d'essai
            # (L1221-25, L1221-26 : un delai de prevenance, pas un preavis).
            "preavis_non_effectue": ["L1237-1", "L1237-2", "L1226-4", "L1226-12",
                                     "L1226-14", "L1234-12", "L1234-13",
                                     "L1221-25", "L1221-26"],
        },
        "titres": {
            "conges": "Les conges payes",
            "feries": "Les jours feries",
            "maintien": "Le maintien de salaire pendant un arret de travail",
            "precarite": "Le CDD et l'indemnite de fin de contrat",
            "rupture": "Le preavis et l'indemnite de licenciement",
            "documents": "Les documents remis a la fin du contrat",
            "retraite": "Le depart et la mise a la retraite",
            "rupture_conventionnelle": "La rupture conventionnelle",
            "bulletin": "Le bulletin de paie : ce qu'il porte, et quand il est du",
            "effectif_cse": "L'effectif qui compte pour le comite social et economique",
            "majoration_caisses": ("Les contributions de formation et "
                                   "d'apprentissage majorees chez un employeur "
                                   "affilie a une caisse de conges"),
            "chomage": ("L'assiette des contributions d'assurance chomage et "
                        "de l'AGS"),
            "assujettissement_fiscal": ("Qui doit la formation professionnelle "
                                        "et la taxe d'apprentissage (bloc "
                                        "S21.G00.44 de la DSN)"),
            "temps_partiel": ("Le temps partiel, la duree legale et les "
                              "heures complementaires et supplementaires"),
            "evenements_familiaux": "Les conges pour evenements familiaux",
            "registre": "Le registre unique du personnel",
            "fin_de_cdd": "La fin d'un CDD : a son terme, ou avant",
            "preavis_non_effectue": ("Le preavis non effectue : ce que chaque "
                                     "fin de contrat en fait"),
        },
    },
    "securite-sociale": {
        "legitext": "LEGITEXT000006073189",
        "nom": "Code de la securite sociale",
        "dossier": "code-de-la-securite-sociale",
        "titre": "Code de la securite sociale -- ce que la paie lit",
        # POURQUOI CE CODE-LA, a partir du 19/09/2026 : le code du travail dit
        # ce qu'une indemnite de rupture VAUT ; il ne dit pas un mot de ce
        # qu'elle SUPPORTE. Le regime social d'une indemnite de rupture
        # conventionnelle ou de retraite -- cotisations, CSG/CRDS, et la
        # contribution patronale de 30 % -- vit ici, et nulle part ailleurs.
        "voulus": {
            # R242-1 (11/10/2026, lot R9) : son troisieme alinea renvoie a des
            # arretes << les conditions et limites dans lesquelles la
            # remuneration peut faire l'objet d'un abattement pour frais
            # professionnels >> (l'arrete du 4 septembre 2025, plus bas) ; son
            # sixieme pose l'assiette minimum que la deduction forfaitaire
            # specifique ne peut pas entamer (art. 9 de l'arrete).
            "assiette": ["L242-1", "R242-1"],
            # L241-13 (lot R9) : la reduction generale << ne tient compte des
            # deductions au titre de frais professionnels calculees
            # forfaitairement en pourcentage de cette remuneration que dans
            # des limites et conditions fixees par arrete >> (III).
            "reduction_generale": ["L241-13", "D241-7"],
            # LE MONTANT NET SOCIAL (10/10/2026) : sa definition est codifiee
            # par le decret 2023-1378 au II de R844-1 (prime d'activite), le
            # meme texte que le 9 bis de R3243-1 du code du travail. C'est
            # lui qui dit ce que S21.G00.58.004 contient (frais professionnels
            # exclus, part patronale sante/prevoyance exclue, cotisations
            # salariales de prevoyance deduites depuis 2024).
            "net_social": ["R844-1"],
            "csg": ["L136-1-1"],
            "contribution_rupture": ["L137-12"],
            # L1237-5 du code du travail ne dit pas l'age de la mise a la
            # retraite : il RENVOIE a ces deux articles-la. Sans eux, la
            # condition d'age ne se verifie pas, et L1237-8 en fait un
            # licenciement.
            # L161-17-2 EST CELUI QUI PORTE LE NOMBRE, et il manquait :
            # L1237-5 du code du travail renvoie au 1er de L351-8, qui
            # renvoie lui-meme a << l'age prevu au premier alinea de
            # l'article L. 161-17-2 AUGMENTE DE TROIS ANNEES >>. Sans lui, la
            # condition d'age de la mise a la retraite ne se verifie pas, et
            # le moteur AVERTIT au lieu de verifier
            # (`permanents-base.json`, `_pourquoi_pas_verifie`).
            "age_de_la_retraite": ["L161-17-2", "L351-1", "L351-8"],
            # L'EFFECTIF DE L'URSSAF (23/09/2026, §80.1) : moyenne annuelle,
            # regle des cinq ans -- celui que porte le CRM 118 et qui regle
            # les seuils de cotisation (formation, FNAL, echeance de la DSN,
            # versement mobilite via L2333-64). Il ne dit rien du CSE.
            "effectif": ["L130-1"],
            # LE PLAFOND (10/10/2026, lot V1i) : la vieillesse plafonnee
            # (L241-3, et son taux D242-4), le FNAL et le chomage d'un
            # intermittent s'assoient sur LE plafond de sa periode. R242-2 dit
            # qu'il se reduit au prorata des jours du contrat ; D242-17 a
            # D242-19 disent ses valeurs journaliere et horaire. La regle
            # propre aux artistes (douze fois le plafond horaire par jour) est
            # dans l'arrete du 24 janvier 1975, plus bas.
            "plafond": ["L241-3", "D242-4", "R242-2", "D242-17", "D242-18",
                        "D242-19"],
            # LE PLAFOND D'UN TEMPS PARTIEL (10/10/2026, lot V4b) : L242-8 pose
            # l'abattement d'assiette des salaries a temps partiel, que le
            # dernier alinea du I de R242-2 met en oeuvre (l'employeur << est
            # en droit de corriger le plafond >>) ; L242-10, qu'il cite, en
            # exclut les emplois a taux ou assiette specifiques et le plafond
            # fractionne entre plusieurs employeurs. `paie/src/plafond.rs`.
            "plafond_temps_partiel": ["L242-8", "L242-10"],
            # LA DISPENSE D'UN TEMPS PARTIEL DE LA SANTE (13/10/2026, lot D1).
            # R242-1-6, 2°, c : un salarie a temps partiel peut se dispenser
            # quand la cotisation atteindrait 10 % de sa remuneration -- ce
            # qui dit qu'un forfait de sante ne baisse pas avec le temps de
            # travail (`calcul::plafond_de_la_sante`).
            "sante_dispenses": ["R242-1-6"],
            # LES MAJORATIONS DES EMPLOYEURS AFFILIES A UNE CAISSE DE CONGES
            # (10/10/2026, lot V1n) : le FNAL, le versement mobilite et les
            # contributions de formation se paient par une majoration de
            # 11,5 % (10 % pour la taxe d'apprentissage d'un intermittent du
            # spectacle) ; `majoration_conges_115` des baremes.
            "majoration_caisses": ["L243-1-3", "D243-0-2", "D243-0-3"],
            # L'ARRONDI A L'EURO DES MONTANTS DECLARES (13/10/2026, lot U5) :
            # L130-1 ne le dit plus (le guide Urssaf de la DSN le cite encore) ;
            # il est dans L133-10, qui compte la fraction egale a 0,50 pour 1.
            "arrondi": ["L133-10"],
        },
        "titres": {
            "assiette": "L'assiette des cotisations, et ce qui en sort",
            "reduction_generale": ("La reduction generale et les deductions "
                                   "forfaitaires pour frais professionnels"),
            "net_social": "Le montant net social (S21.G00.58.004) : sa definition",
            "csg": "L'assiette de la CSG et de la CRDS",
            "contribution_rupture": ("La contribution patronale sur les "
                                     "indemnites de rupture"),
            "age_de_la_retraite": "L'age auquel la retraite s'ouvre",
            "effectif": "L'effectif au sens de la securite sociale",
            "plafond": "Le plafond de la securite sociale et sa reduction",
            "plafond_temps_partiel": ("Le plafond d'un salarie a temps partiel "
                                      "(l'abattement d'assiette)"),
            "sante_dispenses": ("Les dispenses d'adhesion aux garanties "
                                "collectives de sante"),
            "majoration_caisses": ("Les majorations des employeurs affilies a "
                                   "une caisse de conges payes"),
            "arrondi": ("L'arrondi a l'euro des cotisations et de leurs "
                        "assiettes declarees"),
        },
    },
    "collectivites": {
        # L'IDENTIFIANT EST VERIFIE, PAS SUPPOSE (21/09/2026) : une sonde a
        # fait defiler l'archive globale -- 1 750 418 articles -- et a
        # releve sous quel LEGITEXT vivent ces numeros-la. C'est la
        # precaution du piege 3 de l'en-tete, appliquee AVANT de moissonner
        # plutot qu'apres : un LEGITEXT faux ne donne pas une erreur, il
        # donne un relevé vide qu'on lirait comme << ces articles n'existent
        # pas >>.
        "legitext": "LEGITEXT000006070633",
        "nom": "Code general des collectivites territoriales",
        "dossier": "code-general-des-collectivites-territoriales",
        "titre": "Code general des collectivites territoriales -- le versement mobilite",
        # POURQUOI CE CODE-LA. Le versement mobilite est du par les
        # employeurs de ONZE salaries et plus etablis dans le ressort d'une
        # autorite organisatrice qui l'a institue, et son taux est fixe
        # LOCALEMENT -- il n'a pas de valeur de droit commun. Depuis le
        # 21/09/2026 le moteur REFUSE de payer un effectif de onze sans une
        # attestation, et ce refus renvoie a deux articles cites de memoire,
        # jamais relus. On les releve avant d'ecrire la moindre rubrique.
        "voulus": {
            "hors_idf": ["L2333-64", "L2333-66", "L2333-67", "L2333-68",
                         "L2333-70", "L2333-71", "L2333-72", "L2333-73",
                         "L2333-74", "L2333-75"],
            "idf": ["L2531-2", "L2531-3", "L2531-4", "L2531-6"],
        },
        "titres": {
            "hors_idf": "Le versement mobilite hors Ile-de-France",
            "idf": "Le versement mobilite en Ile-de-France",
        },
    },
    "artistes-1975": {
        # UN ARRETE, PAS UN CODE (10/10/2026, lot V1i). La base LEGI range les
        # textes non codifies sous leur JORFTEXT, dans le chemin du fichier
        # comme le LEGITEXT d'un code (`TNC_en_vigueur/JORF/TEXT/.../
        # JORFTEXT000000310494/article/...`) : le meme filtre les attrape.
        # C'est lui qui fixe le plafond d'un artiste engage moins de cinq
        # jours continus : douze fois le plafond horaire par journee de
        # travail (`paie/src/plafond.rs`).
        "legitext": "JORFTEXT000000310494",
        "nom": ("Arrete du 24 janvier 1975 fixant le taux des cotisations de "
                "securite sociale et d'allocations familiales dues au titre "
                "de l'emploi des artistes du spectacle"),
        "dossier": "arrete-1975-artistes-du-spectacle",
        "titre": "Arrete du 24 janvier 1975 (artistes du spectacle) -- le plafond",
        "voulus": {"artistes": ["1", "2", "3", "3 bis"]},
        "titres": {"artistes": "Les cotisations et le plafond des artistes du spectacle"},
    },
    "frais-professionnels-2025": {
        # LA DEDUCTION FORFAITAIRE SPECIFIQUE (11/10/2026, lot R9). Un arrete,
        # pas un code, range sous son JORFTEXT comme celui de 1975 ; trouve
        # par une sonde de l'archive globale et des livraisons (le titre
        # << frais professionnels deductibles >>), pas suppose. Il a ABROGE et
        # remplace l'arrete du 20/12/2002 le 07/09/2025 (livraisons LEGI des
        # 08 et 09/09/2025). Son article 9 dit qui peut exercer la DFS (les
        # professions de l'article 5 de l'annexe IV du CGI dans sa redaction
        # du 31/12/2000), a quelle condition (accord collectif, CE, CSE ou DP,
        # a defaut chaque salarie), dans quelle limite (7 600 EUR par annee
        # civile), a quels taux d'annee en annee (II : le spectacle vivant
        # jusqu'a zero en 2032), ce que l'assiette garde (les remboursements
        # de frais, sauf l'annexe 2) et ce qu'elle ne peut pas entamer
        # (R242-1, al. 6). `paie/src/calcul.rs` (`deduction_forfaitaire`,
        # `abattement_dfs`) ; les taux, `deduction_forfaitaire` des baremes.
        "legitext": "JORFTEXT000052198430",
        "nom": ("Arrete du 4 septembre 2025 relatif aux frais professionnels "
                "deductibles pour le calcul des cotisations de securite "
                "sociale"),
        "dossier": "dfs/arrete-2025-frais-professionnels",
        "titre": ("Arrete du 4 septembre 2025 (frais professionnels) -- la "
                  "deduction forfaitaire specifique"),
        # Les deux annexes (les professions et leurs taux ; les frais qui
        # restent hors assiette) n'ont pas de <NUM> dans la base : ce
        # moissonneur, qui range par numero, ne les garde pas. Elles sont
        # deposees brutes a cote (prive/reference/sources/dfs/LISEZ-MOI.md).
        "voulus": {"frais": ["1", "2", "9"]},
        "titres": {"frais": ("Les frais professionnels et la deduction "
                             "forfaitaire specifique")},
    },
    "reduction-generale-dfs-2019": {
        # LA REDUCTION GENERALE SOUS DFS (lot R9). CSS L241-13, III : elle << ne
        # tient compte des deductions au titre de frais professionnels
        # calculees forfaitairement [...] que dans des limites et conditions
        # fixees par arrete >>. C'est celui-ci (article 1er dans la redaction
        # de l'arrete du 4 septembre 2025, art. 11) : la reduction calculee
        # apres l'abattement ne peut exceder 130 % de celle calculee sans.
        # `paie/src/calcul.rs` (`reduction_generale_sous_dfs`).
        "legitext": "JORFTEXT000039626366",
        "nom": ("Arrete du 4 decembre 2019 relatif au benefice de la reduction "
                "prevue a l'article L. 241-13 du code de la securite sociale "
                "pour les employeurs appliquant la deduction forfaitaire "
                "specifique"),
        "dossier": "dfs/arrete-2019-reduction-generale",
        "titre": ("Arrete du 4 decembre 2019 -- la reduction generale sous "
                  "deduction forfaitaire specifique"),
        "voulus": {"reduction": ["1", "2"]},
        "titres": {"reduction": "La reduction generale apres l'abattement"},
    },
    "construction": {
        # LA PARTICIPATION DES EMPLOYEURS A L'EFFORT DE CONSTRUCTION
        # (10/10/2026, lot V3b). La DSN de decembre declare chaque annee si
        # l'employeur y est assujetti (bloc S21.G00.44, codes 005 et 006) :
        # c'est L313-1 de ce code qui dit qui la doit (cinquante salaries,
        # hors Etat et collectivites), et comment se compte l'effectif.
        "legitext": "LEGITEXT000006074096",
        "nom": "Code de la construction et de l'habitation",
        "dossier": "code-de-la-construction-et-de-l-habitation",
        "titre": "Code de la construction et de l'habitation -- la PEEC",
        "voulus": {"peec": ["L313-1"]},
        "titres": {"peec": "La participation des employeurs a l'effort de construction"},
    },
    "loi-de-finances-2026": {
        # UNE LOI, PAS UN CODE (10/10/2026, lot V3b) : son article 135 a
        # reecrit L6241-1 du code du travail (version du 21/02/2026) et rend
        # redevables de la taxe d'apprentissage les associations a activites
        # non lucratives. Ce qu'on y cherche : a partir de quand.
        "legitext": "JORFTEXT000053508155",
        "nom": "Loi n° 2026-103 du 19 fevrier 2026 de finances pour 2026",
        "dossier": "loi-de-finances-2026",
        "titre": "Loi de finances pour 2026 -- la taxe d'apprentissage des associations",
        "voulus": {"taxe_apprentissage": ["135"]},
        "titres": {"taxe_apprentissage": "La taxe d'apprentissage des associations a activites non lucratives"},
    },
    "impots": {
        "legitext": "LEGITEXT000006069577",
        "nom": "Code general des impots",
        "dossier": "code-general-des-impots",
        "titre": "Code general des impots -- ce que la paie lit",
        # L242-1 du code de la securite sociale RENVOIE a cet article-la pour
        # dire ce qui sort de l'assiette : le lire sans lui, c'est lire une
        # moitie de phrase.
        # 235 bis : la cotisation de 2 % de l'employeur qui n'a pas investi
        # sa participation a l'effort de construction (norme DSN, p. 58 ; bloc
        # S21.G00.44, lot V3b).
        "voulus": {"rupture": ["80 duodecies"],
                   "peec": ["235 bis"],
                   # LA TAXE SUR LES SALAIRES (journal §117) : l'employeur qui
                   # n'est pas assujetti a la TVA -- beaucoup d'associations
                   # du spectacle. Le bareme (231) et l'abattement des
                   # associations (1679 A) ; 1679 dit quand elle se verse.
                   "taxe_salaires": ["231", "1679", "1679 A"],
                   # L'ARRONDI A L'EURO DES SOMMES DUES A LA DGFIP (13/10/2026,
                   # lot U5) : le versement du prelevement a la source
                   # (S21.G00.20.005 de la DGFiP, << arrondi a l'entier le plus
                   # proche >>) est une somme a recevoir de l'administration.
                   "arrondi": ["1724"],
                   # LE PRELEVEMENT A LA SOURCE D'UN MOIS (13/10/2026, lot D1) :
                   # 1671, 1 et 2 -- la retenue est « effectuee par le debiteur
                   # lors du paiement », au taux transmis « au plus tard le
                   # deuxieme mois suivant sa transmission » ; ce qui a ete
                   # retenu ne se recalcule pas (`pas::des_paies`).
                   "pas": ["1671"]},
        "titres": {"rupture": "Le regime fiscal des indemnites de rupture",
                   "peec": "La participation des employeurs a l'effort de construction (la cotisation de 2 %)",
                   "taxe_salaires": "La taxe sur les salaires",
                   "arrondi": "L'arrondi a l'euro des sommes a recevoir",
                   "pas": "La retenue a la source : quand, et a quel taux"},
    },
}

# LEGITEXT -> cle du texte, et cle du texte -> les numeros qu'on y cherche.
PAR_LEGITEXT = {t["legitext"]: cle for cle, t in TEXTES.items()}
TOUS = {cle: {n for liste in t["voulus"].values() for n in liste}
        for cle, t in TEXTES.items()}

RE_NUM = re.compile(r"<NUM>([^<]*)</NUM>")
RE_ETAT = re.compile(r"<ETAT>([^<]*)</ETAT>")
RE_DEBUT = re.compile(r"<DATE_DEBUT>([^<]*)</DATE_DEBUT>")
RE_FIN = re.compile(r"<DATE_FIN>([^<]*)</DATE_FIN>")
RE_ID = re.compile(r"(LEGIARTI\d+)\.xml$")
RE_TEXTE = re.compile(r"<BLOC_TEXTUEL>(.*?)</BLOC_TEXTUEL>", re.S)


def _flux(url):
    return tarfile.open(fileobj=urllib.request.urlopen(url, timeout=300),
                        mode="r|gz")


def _passer(tar, articles, journal, changes=None, livraison="", tous=False,
            seuls=None):
    """Fait defiler une archive et garde les articles en vigueur.

    `tar` : une archive ouverte (en flux pour la globale, en memoire pour
    une livraison). `tous` : garder TOUS les articles des codes suivis --
    c'est ce que fait l'entrepot (journal §122) --, et non les seuls voulus.

    `changes` : chaque fois qu'une livraison remplace la version d'un
    article, on note laquelle et quand. C'est la seule chose qui compte au
    deuxieme passage -- « aucun des articles n'a bouge » est une phrase qui
    doit se prouver, pas se supposer.

    `seuls` : ne regarder que ces textes-la (des cles de `TEXTES`) -- c'est
    ce que fait `_completer` pour un texte ajoute apres la construction de
    l'entrepot.
    """
    suivis = {lt: cle for lt, cle in PAR_LEGITEXT.items()
              if seuls is None or cle in seuls}
    vus, t0 = 0, time.time()
    for m in tar:
        if not m.isfile():
            continue
        if m.name.endswith(".dat") and "suppression" in m.name:
            # CE QUE LA LIVRAISON RETIRE : un article dont la version gardee
            # est dans la liste sort de l'entrepot.
            retires = {l.strip().rsplit("/", 1)[-1] for l in tar.extractfile(m)
                       .read().decode("utf-8", "replace").splitlines() if l.strip()}
            for cle in [k for k, a in articles.items() if a.get("id") in retires]:
                journal.append("retire par %s : %s %s" % (livraison, cle[0], cle[1]))
                del articles[cle]
            continue
        if "/article/" not in m.name:
            continue
        # LE NUMERO NE SUFFIT PAS A DIRE DE QUEL CODE IL EST : "L242-1" existe
        # au code de la securite sociale, et rien n'interdit qu'il existe
        # ailleurs. C'est le chemin du fichier, qui porte le LEGITEXT, qui
        # tranche -- et il tranche AVANT qu'on lise le contenu.
        texte = next((cle for legitext, cle in suivis.items()
                      if legitext in m.name), None)
        if texte is None:
            continue
        vus += 1
        try:
            txt = tar.extractfile(m).read().decode("utf-8", "replace")
        except Exception as e:                                 # noqa: BLE001
            journal.append("illisible %s : %s" % (m.name, e))
            continue
        num = RE_NUM.search(txt)
        if not num or not (tous or num.group(1).strip() in TOUS[texte]):
            continue
        num = (texte, num.group(1).strip())
        etat = RE_ETAT.search(txt)
        fin = RE_FIN.search(txt)
        debut = RE_DEBUT.search(txt)
        en_vigueur = (etat and etat.group(1) == "VIGUEUR"
                      and fin and fin.group(1) == "2999-01-01")
        ancien = articles.get(num)
        if ancien and ancien["en_vigueur"] and not en_vigueur:
            continue
        mid = RE_ID.search(m.name)
        neuf = {"id": mid.group(1) if mid else "",
                "debut": debut.group(1) if debut else "",
                "en_vigueur": bool(en_vigueur),
                "xml": txt}
        if changes is not None and ancien and ancien["id"] != neuf["id"]:
            changes.append({"texte": num[0], "article": num[1],
                            "avant": ancien["id"],
                            "apres": neuf["id"], "depuis": neuf["debut"],
                            "livraison": livraison})
        articles[num] = neuf
    return vus, time.time() - t0


def _livraisons():
    page = urllib.request.urlopen(BASE, timeout=120).read().decode("utf-8", "replace")
    return [n for n in sorted(set(re.findall(r"LEGI_(\d{8}-\d{6})\.tar\.gz", page)))
            if n[:8] > DEPUIS]


def entrepot(racine: str):
    """L'entrepot des quatre codes, a cote des releves (journal §122)."""
    from entrepot_dila import Entrepot
    return Entrepot(os.path.join(racine, "legi-entrepot"))


def moissonner(racine: str, reconstruire: bool = False) -> None:
    """Tenir l'entrepot a jour, puis en tirer les articles voulus. `racine`
    est le dossier des sources ; chaque code y ecrit dans le sien.

    LA PREMIERE FOIS SEULEMENT (ou avec `reconstruire`), l'archive globale
    defile -- 1,1 Go, un quart d'heure -- et l'entrepot garde TOUS les
    articles en vigueur des quatre codes, pas seulement les voulus. Ensuite,
    seules les livraisons pas encore appliquees se telechargent, plusieurs a
    la fois, et s'appliquent dans l'ordre. Ajouter un article a `TEXTES` ne
    coute plus rien : il est deja dans l'entrepot (journal §122).
    """
    from entrepot_dila import archive, en_ordre, flux
    os.makedirs(racine, exist_ok=True)
    ent = entrepot(racine)
    journal = []
    if ent.existe() and not reconstruire:
        ent.charger()
        print("== entrepot : %d articles, %d livraisons deja appliquees =="
              % (len(ent.articles), len(ent.lues)), flush=True)
        # UN TEXTE AJOUTE A `TEXTES` APRES LA CONSTRUCTION (11/10/2026, lot
        # R9 : l'arrete du 4 septembre 2025 sur les frais professionnels) n'a aucun
        # article dans l'entrepot : l'archive globale ne l'a pas garde. On la
        # refait defiler pour lui seul, puis les livraisons deja lues.
        absents = [cle for cle in TEXTES
                   if not any(k[0] == cle for k in ent.articles)]
        if absents:
            _completer(ent, absents, journal)
    else:
        print("== archive globale (construction de l'entrepot) ==", flush=True)
        # L'ARCHIVE GLOBALE EST LE SOCLE : sans elle il n'y a rien a comparer.
        # Elle fait 1,1 Go en flux, et une coupure de transport n'est pas une
        # reponse -- on rejoue, trois fois, avant d'abandonner.
        for essai in (1, 2, 3):
            try:
                vus, sec = _passer(flux(BASE + GLOBAL), ent.articles, journal, tous=True)
                break
            except Exception as e:                             # noqa: BLE001
                print("  archive globale, essai %d : %s" % (essai, e), flush=True)
                if essai == 3:
                    raise
                ent.articles.clear()
                time.sleep(10 * essai)
        ent.etat = {"global": GLOBAL, "lues": [], "changes": [],
                    "construit_le": time.strftime("%Y-%m-%d")}
        print("  %d fichiers d'article lus en %.0fs ; %d articles gardes"
              % (vus, sec, len(ent.articles)), flush=True)
        ent.sauver()
    toutes = _livraisons()
    neuves = [n for n in toutes if n not in ent.lues]
    print("== %d livraison(s) nouvelle(s) sur %d ==" % (len(neuves), len(toutes)),
          flush=True)
    changes = []
    for i, (url, octets) in enumerate(en_ordre(["%sLEGI_%s.tar.gz" % (BASE, n)
                                                 for n in neuves]), 1):
        nom = neuves[i - 1]
        if isinstance(octets, Exception):
            # UNE LIVRAISON MANQUEE ARRETE LA SUITE : appliquer celle du
            # lendemain par-dessus un trou ferait un entrepot faux sans le
            # dire. La prochaine moisson reprendra ici.
            journal.append("livraison %s : %s -- arret, reprise au prochain passage"
                           % (nom, octets))
            break
        _passer(archive(octets), ent.articles, journal, changes, nom, tous=True)
        ent.etat["lues"].append(nom)
        # ON ECRIT EN CHEMIN, tous les vingt-cinq. Un relevé qui ne dit pas ou
        # il s'est arrete est un relevé qu'on croit complet.
        if i % 25 == 0:
            ent.etat["changes"] += changes
            changes = []
            ent.sauver()
            print("  [%d/%d] %s" % (i, len(neuves), nom), flush=True)
    ent.etat["changes"] += changes
    ent.sauver()
    articles = {k: a for k, a in ent.articles.items() if k[1] in TOUS[k[0]]}
    changes_voulus = [c for c in ent.etat["changes"]
                      if c["article"] in TOUS.get(c["texte"], ())]
    _ecrire(racine, articles, sorted(ent.lues), len(toutes), changes_voulus)
    manque = sorted("%s %s" % (cle, n) for cle, nums in TOUS.items()
                    for n in nums if (cle, n) not in articles)
    perimes = sorted("%s %s" % cle for cle, a in articles.items()
                     if not a["en_vigueur"])
    print("== fini ==")
    print("  livraisons : %d appliquees sur %d" % (len(ent.lues & set(toutes)), len(toutes)))
    print("  manquants :", " ".join(manque) if manque else "aucun")
    # LE GARDE-FOU : un article garde dans une version perimee ferait
    # appliquer un droit abroge sans que rien ne le signale.
    print("  perimes   :", " ".join(perimes) if perimes else "aucun")
    print("  changes   :", " ".join(sorted({"%s (%s)" % (c["article"], c["livraison"])
                                            for c in changes_voulus}))
          if changes_voulus else "aucun")
    for l in journal[:20]:
        print("  !", l)


def _completer(ent, absents, journal) -> None:
    """Les textes de `absents`, lus dans l'archive globale puis dans chaque
    livraison deja appliquee a l'entrepot, dans l'ordre : l'entrepot les
    tient ensuite comme s'il les avait gardes des le depart. Le cout est
    celui d'une construction (l'archive globale en flux, puis les
    livraisons), mais une fois par texte ajoute."""
    from entrepot_dila import archive, en_ordre, flux
    print("== textes ajoutes depuis la construction : %s ==" % ", ".join(absents),
          flush=True)
    for essai in (1, 2, 3):
        try:
            vus, sec = _passer(flux(BASE + GLOBAL), ent.articles, journal,
                               tous=True, seuls=absents)
            break
        except Exception as e:                                 # noqa: BLE001
            print("  archive globale, essai %d : %s" % (essai, e), flush=True)
            if essai == 3:
                raise
            for k in [k for k in ent.articles if k[0] in absents]:
                del ent.articles[k]
            time.sleep(10 * essai)
    print("  %d fichiers d'article de ces textes lus en %.0fs" % (vus, sec),
          flush=True)
    lues = sorted(ent.lues)
    for i, (url, octets) in enumerate(en_ordre(["%sLEGI_%s.tar.gz" % (BASE, n)
                                                 for n in lues]), 1):
        if isinstance(octets, Exception):
            # Une livraison manquee laisserait ces textes dans une version
            # ancienne sans le dire : on s'arrete, et on n'ecrit rien.
            raise RuntimeError("livraison %s : %s -- les textes %s ne sont "
                               "pas completes" % (lues[i - 1], octets,
                                                  ", ".join(absents)))
        _passer(archive(octets), ent.articles, journal, None, lues[i - 1],
                tous=True, seuls=absents)
    ent.sauver()
    print("  %d livraisons rejouees pour ces textes" % len(lues), flush=True)


def _ecrire(racine, articles, lues, totales=0, changes=()) -> None:
    """Un relevé par code, chacun dans son dossier.

    CHAQUE DOSSIER SE LIT SEUL : son `releve.json` ne parle que de ses
    articles, et le nombre de livraisons lues y est repete. Un code dont le
    relevé ne dirait ou il en est que par le dossier d'a cote serait un
    relevé qu'on croit complet.
    """
    for cle, texte in TEXTES.items():
        siens = {num: a for (t, num), a in articles.items() if t == cle}
        dossier = os.path.join(racine, texte["dossier"])
        # Le XML de chaque article dans `xml/`, le relevé a cote : c'est la
        # piece justificative qui reste a cote du compte rendu.
        os.makedirs(os.path.join(dossier, XML), exist_ok=True)
        for num, a in siens.items():
            open(os.path.join(dossier, XML, "%s.xml" % _fichier(num)), "w",
                 encoding="utf-8").write(a["xml"])
        json.dump({"_source": BASE, "_global": GLOBAL,
                   "_texte": texte["nom"], "_legitext": texte["legitext"],
                   "_livraisons_lues": len(lues),
                   "_livraisons_totales": totales or len(lues),
                   # Ce qui reste a lire quand un passage s'interrompt : le
                   # document le dira, plutot que de se donner pour complet.
                   "_complet": len(lues) == (totales or len(lues)),
                   "_derniere_livraison": lues[-1] if lues else "",
                   "_changes_depuis_le_global": [
                       c for c in changes if c.get("texte", "travail") == cle],
                   "_releve_le": time.strftime("%Y-%m-%d"),
                   "articles": {n: {k: v for k, v in a.items() if k != "xml"}
                                for n, a in siens.items()}},
                  open(os.path.join(dossier, "releve.json"), "w",
                       encoding="utf-8"),
                  ensure_ascii=False, indent=1)


def _fichier(num: str) -> str:
    """Le nom de fichier d'un article. « 80 duodecies » en est un aussi."""
    return num.replace(" ", "_").replace("/", "-")


def _contenu(xml: str) -> str:
    import html
    m = RE_TEXTE.search(xml)
    if not m:
        return ""
    t = re.sub(r"<[^>]+>", "\n", m.group(1))
    t = html.unescape(t)
    return re.sub(r"\n\s*\n+", "\n", t).strip()


def _jour(aaaammjj: str) -> str:
    """20250713 -> 13/07/2025. Une date se lit, elle ne se dechiffre pas."""
    if len(aaaammjj) < 8 or not aaaammjj[:8].isdigit():
        return aaaammjj          # pas une date : on la rend telle quelle
    return "%s/%s/%s" % (aaaammjj[6:8], aaaammjj[4:6], aaaammjj[:4])


def _ce_qui_a_change(meta: dict) -> str:
    """La seule phrase qui compte au deuxieme passage.

    « Aucun n'a change » n'a de valeur que dit par une machine qui a
    reellement passe les livraisons -- et il faut alors dire combien.
    """
    changes = meta.get("_changes_depuis_le_global") or []
    if not changes:
        return ("**aucun de ces %d articles n'a change de version** depuis "
                "l'archive globale" % len(meta["articles"]))
    return "**%d article(s) ont change de version** : %s" % (
        len(changes), " ; ".join(
            "%s (%s -> %s, livraison du %s)"
            % (c["article"], c["avant"], c["apres"], _jour(c["livraison"][:8]))
            for c in changes))


def rapport(racine: str, quel: str = "") -> None:
    """Le document lisible de chaque code : un titre par domaine, un article
    par entree. `quel` limite a un seul code."""
    for cle, texte in TEXTES.items():
        if quel and cle != quel:
            continue
        dossier = os.path.join(racine, texte["dossier"])
        cible = os.path.join(dossier, "articles-%s.md"
                             % time.strftime("%Y%m%d"))
        _rapport(dossier, cible, texte)


def _rapport(dossier: str, cible: str, texte: dict) -> None:
    meta = json.load(open(os.path.join(dossier, "releve.json"), encoding="utf-8"))
    lignes = [
        "# %s" % texte["titre"],
        "",
        "**Releve automatiquement**, et c'est l'interet : un article lu a la "
        "main sur Legifrance -- qui refuse les robots (403) -- se perime en "
        "silence. Ceux-ci viennent de la base **LEGI** de la DILA, en open "
        "data, par `outils/relever_les_codes.py` -- la meme voie que KALI "
        "pour les conventions collectives (§29). Le releve se REFAIT, et "
        "c'est tout le sujet : il dit ce qui a bouge depuis la derniere "
        "fois.",
        "",
        "| | |",
        "|---|---|",
        "| source | `%s` |" % meta["_source"],
        "| archive globale | `%s` |" % meta["_global"],
        "| code | %s (`%s`) |" % (texte["nom"], texte["legitext"]),
        "| livraisons lues | **%d sur %d**, dans l'ordre depuis le %s |"
        % (meta["_livraisons_lues"], meta.get("_livraisons_totales",
                                              meta["_livraisons_lues"]),
           _jour(DEPUIS)),
        "| ce qu'elles disent | %s |" % _ce_qui_a_change(meta),
        "| releve le | %s |" % meta["_releve_le"],
        "| articles | %d |" % len(meta["articles"]),
        "",
    ]
    # CE QUI RESTE A LIRE, quand un passage s'est interrompu. Un relevé qui
    # tairait la difference se donnerait pour complet, et c'est precisement
    # ce qu'on ne veut pas d'une source.
    # Le defaut se DEDUIT des nombres plutot que d'un booleen absent : un
    # relevé ecrit par une version plus ancienne du moissonneur ne doit pas
    # se donner pour complet par omission.
    complet = meta.get("_complet",
                       meta["_livraisons_lues"]
                       >= meta.get("_livraisons_totales", 0))
    if not complet:
        reste = meta["_livraisons_totales"] - meta["_livraisons_lues"]
        lignes += [
            "**PASSAGE INCOMPLET.** Il reste **%d livraisons** a passer -- "
            "les plus recentes. Ce qui suit est donc vrai au %s pour les %d "
            "premieres ; `outils/relever_les_codes.py moissonner` le "
            "refait en entier, et c'est tout l'interet : un relevé a la main "
            "ne se refait pas."
            % (reste, _jour(meta["_derniere_livraison"][:8]),
               meta["_livraisons_lues"]),
            "",
        ]
    lignes += [
        "**ET LA VERSION EST VERIFIEE.** Le dossier du dump s'appelle "
        "`code_en_vigueur`, mais il porte AUSSI les versions anterieures de "
        "chaque article : la premiere version du moissonneur en a garde sept "
        "sur cinquante-cinq dans une version PERIMEE, dont L3141-3 du code du "
        "travail (les 2,5 jours de conges) et R1234-2 (l'indemnite de "
        "licenciement). Chaque article ci-dessous est verifie en vigueur -- "
        "`<ETAT>` a VIGUEUR et `<DATE_FIN>` a 2999-01-01 --, et celui qui ne "
        "le serait pas est signale a sa ligne.",
        "",
        "Chaque article porte l'identifiant de SA VERSION (`LEGIARTI...`) et "
        "la date depuis laquelle elle est en vigueur : c'est ce qui permet "
        "de dire, au prochain relevé, si quelque chose a bouge.",
        "",
    ]
    for domaine, numeros in texte["voulus"].items():
        lignes += ["---", "", "## %s" % texte["titres"][domaine], ""]
        for num in numeros:
            a = meta["articles"].get(num)
            if not a:
                lignes += ["### %s -- ABSENT DU RELEVE" % num, ""]
                continue
            xml = open(os.path.join(dossier, XML, "%s.xml" % _fichier(num)),
                       encoding="utf-8").read()
            etat = "" if a["en_vigueur"] else "  **VERSION PERIMEE**"
            lignes += ["### %s (`%s`, en vigueur depuis le %s)%s"
                       % (num, a["id"], a["debut"], etat), "",
                       "> " + _contenu(xml).replace("\n", "\n> "), ""]
    open(cible, "w", encoding="utf-8").write("\n".join(lignes) + "\n")
    print("%d articles -> %s" % (len(meta["articles"]), cible))


ANTERIEUR = "xml-anterieur"   # les versions d'avant, a cote des pieces


def _changements(racine: str) -> list:
    """Tous les changements de version notes par le dernier `moissonner`,
    tous codes confondus. C'est le releve qui dit ce qu'il y a a comparer :
    on ne redecide pas ici quels articles ont bouge."""
    tout = []
    for cle, texte in TEXTES.items():
        chemin = os.path.join(racine, texte["dossier"], "releve.json")
        if not os.path.exists(chemin):
            continue
        meta = json.load(open(chemin, encoding="utf-8"))
        for c in meta.get("_changes_depuis_le_global") or []:
            tout.append(dict(c, texte=c.get("texte", cle)))
    return tout


def _avant(racine: str, changements: list) -> dict:
    """Va chercher dans l'archive globale les versions d'AVANT, par leur
    identifiant. Rend {(texte, article): xml} et ecrit chaque piece a cote
    des autres, dans `<code>/xml-anterieur/`.

    ON S'ARRETE DES QU'ON LES A TOUTES. L'archive porte 1,75 million
    d'articles et les versions cherchees ne sont qu'une poignee : sortir au
    dernier trouve a economise un cinquieme du defilement, le 21/09/2026.
    """
    vise = {c["avant"]: (c["texte"], c["article"]) for c in changements}
    trouves, vus, t0 = {}, 0, time.time()
    tar = _flux(BASE + GLOBAL)
    for m in tar:
        if not m.isfile() or "/article/" not in m.name:
            continue
        vus += 1
        mid = RE_ID.search(m.name)
        if not mid or mid.group(1) not in vise:
            continue
        cle, num = vise[mid.group(1)]
        xml = tar.extractfile(m).read().decode("utf-8", "replace")
        trouves[(cle, num)] = xml
        dossier = os.path.join(racine, TEXTES[cle]["dossier"], ANTERIEUR)
        os.makedirs(dossier, exist_ok=True)
        open(os.path.join(dossier, "%s.xml" % _fichier(num)), "w",
             encoding="utf-8").write(xml)
        print("  %s %s (%d/%d)" % (num, mid.group(1), len(trouves), len(vise)),
              flush=True)
        if len(trouves) == len(vise):
            break
    print("  %d fichiers parcourus en %.0fs ; %d/%d versions anterieures "
          "retrouvees" % (vus, time.time() - t0, len(trouves), len(vise)),
          flush=True)
    return trouves


def _alineas(xml: str) -> list:
    """Le texte d'un article, alinea par alinea, espaces normalises.

    On compare le BLOC_TEXTUEL et lui seul : ni les visas, ni les liens, ni
    les notes. Un changement qui n'y figurerait pas ECHAPPE a cette lecture,
    et le document le dit plutot que de le taire.
    """
    return [re.sub(r"\s+", " ", l).strip()
            for l in _contenu(xml).split("\n") if l.strip()]


def comparer(racine: str) -> None:
    """Relit chaque article qui a change contre sa version d'avant, et ecrit
    le document des differences."""
    changements = _changements(racine)
    cible = os.path.join(racine, "changements-de-version-%s.md"
                         % time.strftime("%Y%m%d"))
    if not changements:
        # RIEN A COMPARER N'EST PAS RIEN A DIRE : le document existe quand
        # meme, et il date le passage qui n'a rien trouve.
        open(cible, "w", encoding="utf-8").write(
            "# Les changements de version : aucun\n\n"
            "Le dernier `moissonner` n'a note **aucun** changement de version "
            "depuis l'archive globale `%s`. Il n'y a donc rien a relire.\n\n"
            "*Etabli le %s.*\n" % (GLOBAL, time.strftime("%d/%m/%Y")))
        print("aucun changement -> %s" % cible)
        return
    print("== %d article(s) a relire contre leur version d'avant =="
          % len(changements), flush=True)
    for essai in (1, 2, 3):
        try:
            avant = _avant(racine, changements)
            break
        except Exception as e:                                 # noqa: BLE001
            print("  archive globale, essai %d : %s" % (essai, e), flush=True)
            if essai == 3:
                raise
            time.sleep(10 * essai)
    _ecrire_comparaison(racine, cible, changements, avant)


def _ecrire_comparaison(racine, cible, changements, avant) -> None:
    import difflib
    lignes = [
        "# Ce qui a change DANS LE TEXTE des articles qui ont change de "
        "version",
        "",
        "*Etabli le %s par `outils/relever_les_codes.py comparer`.*" %
        time.strftime("%d/%m/%Y"),
        "",
        "Changer de version n'est pas changer de sens : un article dont un "
        "alinea etranger a la paie est reecrit recoit un identifiant neuf "
        "sans que rien de ce qui nous concerne bouge. Ce document montre, "
        "article par article, **ce qui a reellement change dans le texte** "
        "-- il ne dit pas ce qu'il faut en faire, ce qui demande une "
        "lecture.",
        "",
        "Les versions d'avant viennent de l'archive globale `%s`, cherchees "
        "**par leur identifiant** et non par leur numero : on ne voulait pas "
        "une vieille version, on voulait celle que le relevé precedent "
        "tenait pour en vigueur. Elles sont gardees dans "
        "`<code>/%s/`, a cote des pieces justificatives." % (GLOBAL, ANTERIEUR),
        "",
        "La comparaison porte sur le `BLOC_TEXTUEL` seul -- ni visas, ni "
        "liens, ni notes. **Un changement qui n'y figurerait pas echappe a "
        "cette lecture.**",
        "",
    ]
    for c in sorted(changements, key=lambda c: (c["texte"], c["article"])):
        nom = TEXTES[c["texte"]]["nom"]
        lignes += ["---", "",
                   "## %s (%s)" % (c["article"], nom), "",
                   "`%s` -> `%s`, en vigueur depuis le %s, livraison du %s."
                   % (c["avant"], c["apres"], _jour(c["depuis"].replace("-", "")[:8])
                      if c["depuis"] else "?", _jour(c["livraison"][:8])),
                   ""]
        xml_a = avant.get((c["texte"], c["article"]))
        if xml_a is None:
            # UNE VERSION INTROUVABLE SE DIT. La taire laisserait croire que
            # l'article a ete relu, alors qu'il ne l'a pas ete.
            lignes += ["**Version anterieure INTROUVABLE dans l'archive "
                       "globale.** Cet article n'a donc PAS ete relu contre "
                       "son passe.", ""]
            continue
        dossier = os.path.join(racine, TEXTES[c["texte"]]["dossier"])
        xml_b = open(os.path.join(dossier, XML, "%s.xml" % _fichier(c["article"])),
                     encoding="utf-8").read()
        a, b = _alineas(xml_a), _alineas(xml_b)
        if a == b:
            lignes += ["**Texte identique** : %d alineas de part et d'autre. "
                       "Seule la version a change." % len(a), ""]
            continue
        lignes += ["%d alineas -> %d. Ce qui differe :" % (len(a), len(b)), "",
                   "```diff"]
        for l in difflib.unified_diff(a, b, "avant", "apres", lineterm="", n=1):
            if not l.startswith(("---", "+++", "@@")):
                lignes.append(l)
        lignes += ["```", ""]
    open(cible, "w", encoding="utf-8").write("\n".join(lignes) + "\n")
    print("%d article(s) -> %s" % (len(changements), cible))


def main():
    if len(sys.argv) < 3:
        print(__doc__)
        return 2
    quoi = sys.argv[1]
    if quoi == "moissonner":
        moissonner(sys.argv[2], reconstruire="--reconstruire" in sys.argv)
    elif quoi == "rapport":
        rapport(sys.argv[2], sys.argv[3] if len(sys.argv) > 3 else "")
    elif quoi == "comparer":
        comparer(sys.argv[2])
    else:
        print("Verbe inconnu : %s" % quoi)
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main())

# Être éditeur DSN — la charte, et la régularisation

Relevé le 17/09/2026. Documents publics, retéléchargeables aux adresses
données ; `sha256sum <fichier>` pour vérifier qu'une copie n'a pas bougé.

| Fichier | Contenu | URL d'origine | SHA-256 |
|---|---|---|---|
| `charte-editeurs-revue-forme.pdf` | Charte de partenariat DSN entre les éditeurs de logiciels de paie et le GIP-MDS, version mars 2024 (mise à jour 05/03/2024), 6 pages. | https://www.net-entreprises.fr/media/documentation/charte-editeurs-revue-forme.pdf | `212eebfad46f3f6c397f174ff69d7b5cbc7aeccbd6d9307782735bc528b8356c` |
| `mode-operatoire-procedure-editeur.pdf` | Mode opératoire de signature de la charte (avril 2024), 10 pages : identifiants reçus par mail de « Déploiement DSN », code à usage unique, espace éditeur, « Valider la nouvelle charte éditeur », code éditeur. | https://www.net-entreprises.fr/media/documentation/mode-operatoire-procedure-editeur.pdf | `7c19e2bf2284d372a60c616b929c0fc96b04b8b860083c7f3afc22160afaf146` |
| `guide-urssaf-regularisation.pdf` | Guide Urssaf « Comment déclarer et régulariser les cotisations Urssaf en DSN », version du 04/06/2026, 41 pages (1 001 122 octets). **Téléchargé à la main par Léandre le 18/09/2026** : urssaf.fr coupe les connexions automatisées (deux essais le 17/09), le fichier n'est donc pas relevable par `sources`. | https://www.urssaf.fr/files/live/sites/urssaffr/files/outils-documentation/guides/Guide-declaration-regularisation-cotisations-sociales-Urssaf-DSN.pdf | `7be4aabb6c3b0c7f118d7232e22580e705e01d7d5d8b0080c3328de5a33c4456` |

Page d'origine : https://www.net-entreprises.fr/declaration/charte-editeurs-dsn/
(liste des signataires 2024 : https://www.net-entreprises.fr/media/documentation/liste-des-signataires-2024.pdf).

## Ce que dit la charte (lue en entier le 17/09/2026)

- **Volontaire, pas une homologation.** Art. 2 : « l'adhésion à la charte
  n'emporte pas homologation des logiciels de l'éditeur [...] ni vérification
  par les organismes ou administrations de la conformité du fonctionnement
  d'une version d'un logiciel ». Ce qu'elle apporte : « la mention sur
  Net-entreprises.fr de l'identité de l'éditeur signataire dans une liste de
  référence », dès adhésion.
- **Elle nous concerne déjà.** Art. 1 : « Les dispositions de la charte sont
  également applicables aux entreprises qui utilisent leur propre logiciel de
  paie et souhaitent émettre des DSN. »
- **Les engagements de l'éditeur** (art. 3), ceux qui nous touchent :
  - un logiciel conforme au cahier technique, « incluant des fonctionnalités
    complètes de régularisations sur exercices antérieurs » ;
  - **afficher clairement ce qu'il ne couvre pas** (notre §9 « ce que le
    logiciel ne fait pas ») ;
  - appliquer la dernière norme « et basculer sur la nouvelle version de
    norme dès sa mise en œuvre, à la fin du mois de janvier de chaque
    année » ;
  - tester chaque nouveau cahier technique sur la plateforme éditeur
    (`test.net-entreprises.fr`) avant la mise en production ;
  - interpréter les comptes rendus métiers (CRM), notamment normalisés, et
    permettre « la correction des erreurs de paie et/ou erreurs de
    déclaration [...] sur un mois suivant le mois de paie où l'erreur a été
    commise » ;
  - en cas de changement de logiciel, fournir au déclarant les données
    identifiantes (SIRET, NIR, numéro de contrat, date de début) et savoir
    reprendre celles de l'ancien — le « signalement d'amorçage des données
    variables » ;
  - relayer la base de connaissances, remonter les alertes au GIP-MDS.
- **Signer** (art. 4) : un primo-signataire envoie la charte signée sur
  papier, scannée, à `DeploiementDSN@gip-mds.fr` (ou par courrier, GIP-MDS,
  4-14 rue Ferrus, 75014 Paris) ; il reçoit ensuite des identifiants et un
  code éditeur pour l'espace de signature en ligne (mode opératoire).
  Signée « par un représentant de l'éditeur dûment mandaté » ; « sans limite
  de durée ».

**Décision de Léandre, 17/09/2026 : signer la charte en décembre 2026, avant
d'envoyer le logiciel aux compagnies amies.** Prérequis : la régularisation
après échéance (`journal/REGULARISATION-APRES-ECHEANCE.md`), et une page
« ce que le logiciel ne couvre pas » lisible par une entreprise.

## Ce que dit le guide Urssaf sur la régularisation (lu le 17/09/2026)

Chapitres 3 (régularisation), 5 (néant), 6 (paiement), 7 (entité
d'affectation), 1.6 (arrondis). Confronté à `dsn.py` (`_bordereaux_regularises`,
`_montant_regularise`, les ordres 20 par mois régularisé) :

| Le guide | Le moteur |
|---|---|
| Un bloc 22 par mois de survenance de l'erreur, daté de ce mois (§3.3.1, 2.3.2) | ✅ |
| « Si l'erreur porte sur une assiette, la régularisation s'opère en différentiel » (p. 19) — les 23 en différence | ✅ (23.004 = après − avant) |
| Taux et plafonds de la période d'emploi, pas du mois de paie (§3.1) | ✅ (barème du mois d'origine) |
| 78/79/81 datés du mois de survenance (§2.3.2) ; A/R ou différentiel au choix, A/R préconisé pour une erreur de taux (§3.5) | ✅ A/R (−avant, +après) ; l'équivalence 23 = Σ81 tient en somme |
| « Autant de paiements que de périodes » (§6.1) ; bloc 20 jamais négatif (p. 19) | ✅ un 20 Urssaf par mois régularisé |
| 22/23/20 arrondis à l'euro, nominatif jamais (§1.6) | ✅ |
| 20.002 / 22.002 « entité d'affectation » : ETT, mannequins, producteurs de films seulement (ch. 7) | ✅ non renseignées |
| Néant avec régularisations : un 22 du mois principal + un par mois régularisé (ch. 5) | à vérifier le jour venu |
| **Reprise de crédit** (§6.4) : un 22.005 **négatif** sur le mois d'origine est admis, repris sur le bloc 20 du mois courant dans la limite du bordereau courant, en prévenant l'Urssaf si le montant est important | ✅ 22 et 23 négatifs sur le mois d'origine, pas d'ordre 20 pour lui, l'ordre du mois courant diminué ; au-delà du bordereau courant, l'ordre du mois tombe à zéro et le reste est dit à la compagnie (« contactez votre Urssaf », cahier technique § 2.2.3.2) ; `bordereau::reprise_urssaf` et `bordereau::credit` (21/10/2026) |

Preuves en dépôt de test : hausse, `WSt0jCJDR28oaFJxx6CIOMF` (17/09, CRM 61
« 16 euros [...] période de rattachement de août 2026 ») ; baisse en reprise
de crédit, `WSG2852ykznx4wIZqKwjhQs` (18/09, CRM 61 « télédéclaration 532
euros, télépaiement 501 euros »), 0 anomalie les deux fois.

## La part patronale Agirc-Arrco, 142 et 146 (lu le 10/10/2026, lot V2d)

Transcription : `guide-urssaf-regularisation.txt` (`pdftotext -layout` du PDF
ci-dessus, empreinte inchangée ; les pages sont celles du PDF, séparées par un
saut de page dans le `.txt`).

- p. 34 (§ 8.2, la réduction générale, point 2 ; lignes 1379-1389 du `.txt`) et
  p. 37-38 (lignes 1576-1589) : « À compter de la version de norme P25V01, le
  montant des cotisations parts patronales de retraite complémentaire Agirc
  Arrco (tranche 1) et de CEG (Tranche 1) ainsi que le taux sont à déclarer en
  bloc S21.G00.81 sous le code « 142 - Cotisation régime unifié Agirc-Arrco
  (part patronale tranche T1) » » ; « À compter de la version de la norme
  P26V01, le montant des cotisations parts patronales de retraite
  complémentaire Agirc-Arrco (Tranche 2 de CEG (Tranche 2) et de CET (tranches
  1 et 2) ainsi que le taux sont à déclarer [...] sous le code « 146 » ».
- p. 38 (lignes 1591-1599) : « il est toléré de multiplier les blocs
  « S21.G00.81 » avec le code cotisation « 146 » pour déclarer séparément
  chacun des éléments sous ce même code » ; « Une tolérance est accordée en
  2026 sur le contenu déclaratif des cotisations associées au code « 146 » mais
  une mise en conformité est attendue dès le 1er janvier 2027 (instruction en
  cours) ».
- p. 36 (tableau du montant net social, lignes 1490-1497) : la retraite
  complémentaire salariale de tranche 1 se recalcule « 81.004 pour le type
  131 - 81.004 pour le type 142 » : le 131 garde les parts salariale ET
  patronale, le 142 n'en est qu'une partie.
- p. 5 (lignes 190-193) : « Chaque cotisation individuelle est rattachée à la
  base assujettie juridique correspondante. Le taux de cotisation
  (S21.G00.81.007) est indiqué pour chaque code de cotisation ».
- Le guide renvoie à la « fiche consigne Cotisations de retraite
  complémentaire AGIRC ARRCO dues à partir du 01/01/2023 » (dsn-info,
  net-entreprises) : **non relevée** (le conteneur n'a pas le droit d'aller sur
  net-entreprises) ; à déposer ici.

Où le moteur s'en sert : `atelier/sources/paie/src/dsn.rs`,
`parts_patronales_agirc_arrco` et `sans_la_cet` (appelées par `cotisations`) ;
la norme elle-même (142 et 146 dans la modalité Urssaf de 81.001, SIG-18) est
dans `atelier/reference/norme-dsn/texte/bloc-S21.G00.81.txt`. **Inférences du
moteur**, que le guide ne nomme pas : le 142/146 sous le bloc 78 du 131 qu'il
détaille (base 23, admise par la modalité Urssaf de 78.001, norme p. 293) ; la
CET fondue dans les lignes de tranche d'un cadre intermittent séparée par
différence (la ligne sans la CET, au même arrondi, et le reste) ; un 146 à part
pour la CET (la tolérance de la p. 38), faute de texte qui dise l'assiette et le
taux d'un 146 unique qui mêlerait tranche 2 et CET.

## La réduction générale d'un permanent, CTP 668 (lu le 10/10/2026, lot V3a)

Même transcription (`guide-urssaf-regularisation.txt`).

- p. 33 (§ 8.2, lignes 1289-1290) : « Pour la réduction générale, il est important de veiller à la cohérence entre
  les données agrégées déclarées via les CTP 668, 669, 671, 801 et les données individuelles relatives aux salariés
  déclarés en bloc 81 code 018. »
- p. 33 (tableau, cas 1, lignes 1300-1311) : « CTP 668 : réduction générale étendue / CTP 669 : régularisation de
  réduction générale étendue » ; « La somme des données individuelles (S21.G00.81.004 : code 018) doit être égale aux
  données agrégées (S21.G00.23.005) » ; « Faire porter uniquement la réduction générale sur les cotisations patronales
  de sécurité sociale (régime général et assurance chômage) » ; « 018 : réduction générale des cotisations patronales
  de sécurité sociale (régime général + assurance chômage) ». Cas 2 (lignes 1320-1333), « pour les salariés pour
  lesquels la contribution d'assurance chômage n'est pas recouvrée par les Urssaf/CGSS » : CTP 671 et 801.
- p. 34 (schéma, lignes 1341-1357) : 668/671 en 23.005 (montant), 669/801 en 23.004 (assiette) ; « Attention, le
  montant agrégé contient uniquement les montants Urssaf et assurance chômage, il est à comparer avec le code 018 du
  bloc B81. »
- Le tableau d'équivalence DIDA de l'Urssaf (`../chomage-permanents/urssaf-equivalence-dida-20260611.csv`) le redit :
  l. 1411-1416 (668, « à utiliser en remplacement du CTP 671 sauf pour les salariés pour lesquels la contribution à
  l'assurance chômage n'est pas recouvrée par les Urssaf et CGSS (cas de l'emploi d'intermittents par un employeur
  professionnel du spectacle) ») ; l. 654-664 (669 : « permet de reverser un montant de réduction de cotisations trop
  élevé. Contrairement au CTP 668, le montant reversé doit être indiqué en rubrique S21.G00.23.004 ») ; l. 1418-1421
  (801, le même pour le 671).

Où le moteur s'en sert : `atelier/sources/paie/src/dsn.rs`, `reduction_a_l_urssaf` et `Lecture::reduction_a_l_urssaf`
(le CTP 668 d'un permanent = l'opposé de la somme de ses 018 chez l'Urssaf, parts sécurité sociale et chômage ; le 671
d'un technicien intermittent = sa seule 018 chez l'Urssaf), pour le bordereau courant (`assiettes_ctp`) comme pour
celui d'une correction (`assiette_declaree`). **Appliqué le 11/10/2026 (lot R2)** : une correction qui fait baisser
la réduction écrit le 669 (801 pour le 671), qualifiant 921 (DIDA l. 654 : « 669;…;921 »), en 23.004 et sans signe
(schéma p. 34, l. 1341-1362 : « les montants ne doivent pas comporter de signe ; les CTP comprennent les signes
nécessaires ») : `dsn::lignes_regularisees` et `ctp_de_regularisation`, qui lit `reduction_generale.dsn.ctp_regularisation`
du barème (`par_nature.permanent` : 669 ; `par_statut.technicien` : 801). Le montant du bordereau ne change pas (669 et
801 sont valorisés à 100 % par la table des CTP). Preuve : `tests/regularisation_declaree.rs`.

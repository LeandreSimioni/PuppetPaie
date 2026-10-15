# Un mois entier de suspension non rémunérée (permanent CDI ou CDD mensualisé) — les textes

Relevé le **15/10/2026** (séance de recherche documentaire, question posée par le moteur : un permanent en congé parental
total, congé sabbatique ou congé sans solde sur tout le mois, bloc DSN S21.G00.65 ; il reste 0,07 € de brut par l'arrondi du
taux horaire, la part patronale de la complémentaire santé est réintégrée, le montant soumis au PAS devient négatif, le
contrôle des formats refuse). **Le rapport est `RAPPORT-20261015.md`** (même dossier) : il répond point par point, texte à
l'appui, et dit ce qui est inférence. Rien n'est codé.

Nommage : `<émetteur>-<objet>-<AAAAMMJJ>.<ext>` ; le brut est tel que servi, la transcription `.txt` (ou `.md`) est à côté.
Les empreintes SHA-256 sont celles du **brut**.

## Ce qui n'a pas pu être relevé

- **Le BOSS** (rubrique « Protection sociale complémentaire », chapitre sur la suspension du contrat) : `boss.gouv.fr` ne répond
  pas depuis le conteneur le 15/10/2026 (en `https`, la connexion est coupée par le serveur : `curl` « Recv failure: Connection
  reset by peer » avec TLS 1.2 et 1.3, avec et sans `User-Agent` ; en `http`, 302 vers l'`https` qui est coupé ; l'outil de
  lecture de pages répond 503 ; `web.archive.org` est coupé aussi). Même constat que `../boss/LISEZ-MOI.md` (10/10/2026).
  **Substitut** : le texte que le BOSS reprend, l'**instruction interministérielle DSS/3C/5B/2021/127 du 17/06/2021**, déposée
  ici depuis le site de la Sécurité sociale (voir ci-dessous). Le BOFiP déposé (`../bofip/BOI-RSA-BASE-30-10-20-*`) renvoie
  lui-même à ce chapitre du BOSS (« se reporter au A de la section 2 du chapitre 2 de la rubrique PSC »).
- **Légifrance** : 403 (comme toujours). Les codes viennent de l'entrepôt LEGI local (voir `legi-extrait-*`).
- **Cass. soc. 20/01/1999, n° 96-45.042** (cité par la DDETS, ci-dessous) : absent de la base CASS « Freemium » (décision non
  publiée au bulletin) ; non déposé, non lu.

## Les fichiers

| fichier | ce que c'est | URL, date du relevé | SHA-256 du brut |
|---|---|---|---|
| `dss-instruction-3c5b-2021-127-suspension-contrat-prevoyance-20210617.pdf` + `.txt` (`pdftotext -layout`) | Instruction interministérielle n° DSS/3C/5B/2021/127 du 17/06/2021 « relative au traitement social du financement patronal de la prévoyance complémentaire collective et obligatoire en cas de suspension du contrat de travail » (NOR SSAS2118819J ; abroge la fiche 7 de la circulaire DSS/5B/2009/32). 7 pages. | `https://www.securite-sociale.fr/files/live/sites/SSFR/files/medias/DSS/2021/Instruction%20InterM%20raitement%C2%A0social%20du%20financement%20patronal%20de%20la%20pr%C3%A9voyance%20compl%C3%A9mentaire%20collective%20et%20obligatoire%20en%20cas%20de%C2%A0suspension%20du%20contrat%20de%20travail.pdf?lang=en`, relevé le 15/10/2026 | `efaa41050c382bb44a8e76f6c52c28b1227fb26611cf036b862dbb2ea8f15ece` |
| `net-entreprises-fiche1810-rnf-negative-pas-20250322.html` + `.txt` | Fiche DSN-info n° 1810 « Rémunération nette fiscale négative » (modifiée le 22/03/2025) : le PAS quand la RNF est négative (maintien de cotisations pendant une suspension de contrat). | `https://net-entreprises.custhelp.com/app/answers/detail_dsn/a_id/1810/`, 15/10/2026 | `bcbc1e6d525ede4bdc49a311851e69771abf230e116b07355b8f41f954f54a12` |
| `net-entreprises-fiche2640-volumes-activite-s21g00-53-20260929.html` + `.txt` | Fiche n° 2640 « Les volumes d'activité rémunérée et d'absence partiellement ou non rémunérée — Activité S21.G00.53 » (modifiée le 29/09/2026). | `…/a_id/2640/`, 15/10/2026 | `279c019b6ddda91281583e8b75d24f480159e3554e9cb9d318b8a0454d630537` |
| `net-entreprises-fiche2693-volumes-activite-exemples-20260929.html` + `.txt` | Fiche n° 2693 « Déclarer les volumes d'activité (S21.G00.53) — Exemples » (modifiée le 29/09/2026) : **exemples 6 et 10, un mois entier d'absence sans rémunération**. | `…/a_id/2693/`, 15/10/2026 | `b941fe9e5bfd48e9f8cdfca3f014cac787722b2d7466052bf0c5c411e1d54746` |
| `net-entreprises-fiche807-conges-parentaux-familiaux-20260708.html` + `.txt` | Fiche n° 807 « Les différents congés parentaux et familiaux » (08/07/2026) : motifs 632, 501, renvois aux articles du code du travail. | `…/a_id/807/`, 15/10/2026 | `db05d78dbacb45a3ae7dc579b31598fa2474f2654b24c90c45751024b6b76505` |
| `net-entreprises-fiche639-conge-parental-invalidite-20260609.html` + `.txt` | Fiche n° 639 « Déclarer un congé parental d'éducation ou une invalidité catégorie 1, 2 ou 3 » (09/06/2026) : temps plein = bloc 65, motif 632. | `…/a_id/639/`, 15/10/2026 | `6b1b19d88c64d7eb1660e2c3c135c30a7367b00904c2d707a21860897d95487d` |
| `net-entreprises-dsn-solution-technique-pas-v19-20170308.pdf` + `.txt` | « DSN — Données et axes structurants en conséquence prévus pour le PAS », version 19.0 du 08/03/2017 (GIP-MDS et DGFiP), 53 pages. **Ancien** (norme 2017, avant la rubrique « Montant soumis au PAS ») : sert de corroboration pour la RNF négative (exemple 9 : « il a la possibilité de renseigner un montant négatif »). | `https://net-entreprises.fr/media/documentation/dsn-solution-technique-pas.pdf`, 15/10/2026 | `54feccbd9704cc1a2cf3fa4d38c40d1fa1c375df2fc0d669b793443790a06267` |
| `bofip-boi-ir-pas-20-10-10-20200728.html` + `.txt` | BOFiP BOI-IR-PAS-20-10-10, « Assiette de la retenue à la source », version en vigueur du 28/07/2020 (la dernière servie) : l'assiette du PAS est le montant net imposable (§ 1 et 10). | `https://bofip.impots.gouv.fr/bofip/11249-PGP.html` (redirige vers `…/identifiant=BOI-IR-PAS-20-10-10-20200728`), 15/10/2026 | `65353696f44b9475ae8fc02c4735aa5dda7fccf3c2f173c7d93e3b7f035d6b0a` |
| `dreets-limousin-qr-assmat-retenues-absence-20150101.pdf` + `.txt` | Questions/réponses de la DIRECCTE Limousin (pôle Travail), janvier 2015, « Salaire / absences », 3-2 « Mode de calcul des retenues pour absence » : l'administration du travail dit la règle de la Cour de cassation (salaire horaire réel du mois) et cite les deux arrêts. Écrit pour les assistants maternels, mais le passage vaut pour tout salarié mensualisé. | `https://nouvelle-aquitaine.dreets.gouv.fr/sites/nouvelle-aquitaine.dreets.gouv.fr/IMG/pdf/QR_ASSMAT_SALAIRE_ABSENCES_3-2.pdf`, 15/10/2026 | `d33a0c6bea65ee6c3426231d6b7933fbace3dce0512630b733d2b734acc030ec` |
| `cass-soc-19820211-80-40359-JURITEXT000007009242.xml` | Cour de cassation, chambre sociale, **11 février 1982, n° 80-40.359**, publié au bulletin n° 90 : la retenue par heure d'absence d'un salarié payé au mois. Texte tel que la DILA le livre (XML). | Base CASS de la DILA, archive `https://echanges.dila.gouv.fr/OPENDATA/CASS/Freemium_cass_global_20250713-140000.tar.gz` (260 063 213 octets, SHA-256 `18658ed203e3cf9cd7f5e61aae7bbfe5d19895eddaa9cb41ecf2aa5dd44ee960`), membre `cass/global/sociale/JURI/TEXT/00/00/07/00/92/JURITEXT000007009242.xml`, 15/10/2026 | `01f0f562b1ff7d4c0646d6a4d82ce41efd837cdc6f7ab6920b6aa3d916a5924a` |
| `cass-soc-19870514-84-43854-JURITEXT000007018764.xml` | Cour de cassation, chambre sociale, **14 mai 1987, n° 84-43.854**, publié au bulletin : le taux horaire d'un salarié mensualisé se calcule sur le nombre d'heures du mois (156 h en février 1982). Même archive, membre `…/JURITEXT000007018764.xml`. | idem | `48b0221b61027279c01f27f550114d1d128258126a23ce17f17aa19acaccdb02` |
| `legi-extrait-code-du-travail-20261015.md` | Code du travail : L1225-47 à L1225-59 (congé parental d'éducation), L3142-28 à L3142-34 (congé sabbatique), L3142-105 à L3142-109 (création d'entreprise), L3242-1 à L3242-3 (mensualisation), L3251-1. | entrepôt LEGI local (`../legi-entrepot/`, mis à jour le 2026-10-15), 15/10/2026 | `186379741f88d509532b5cc95c2480c616b281b6a78213c2109460eaa228d53d` |
| `legi-extrait-code-de-la-securite-sociale-20261015.md` | CSS : L242-1, R242-1-1 à R242-1-6, L911-1, L911-2, L911-7, L911-8, D911-1 à D911-8, L136-1-1, L136-2. | idem | `1d3abd1e8e81d4220ae74e67387cb817ec0665ade28b7b9caaf44a70ce70415c` |
| `legi-extrait-code-general-des-impots-20261015.md` | CGI : 83 (1° quater), 204 A, 204 B, 204 E, 204 F, 204 H. | idem | `58801605d5b77239e04479a63a6d47e7fe2a122da70fcb39b2c3a7346554923b` |

**Les trois `legi-extrait-*` ne sont pas des relevés `paie releve-codes`** : le moteur Linux livré n'a pas cette commande (elle est sous
`--features atelier`), et ces articles ne sont pas dans `atelier/sources/paie/src/releve_codes/textes.rs`. Ils ont été lus
dans l'entrepôt LEGI local (`articles.jsonl.gz`, version `en_vigueur`, `<DATE_FIN>` 2999-01-01) par un script de lecture jetable,
sans rien modifier. **À faire par une séance qui touche au code** : ajouter ces articles à `textes.rs` (domaines « congé
parental et sabbatique », « protection sociale complémentaire », « assiette du PAS ») pour que `moissonner` les suive.

## Ce qu'on en lit, et où le moteur s'en servira (rien n'est codé)

Voir le rapport. En une phrase par texte :

- l'instruction DSS de 2021 (II.B) : pour un salarié absent « pour des raisons autres que médicales » (congé parental, sabbatique,
  sans solde) et sans indemnisation, le maintien des garanties est **facultatif** ; s'il est maintenu, la limite d'exonération se
  calcule sur une rémunération reconstituée (moyenne des douze mois précédents) ;
- la fiche 1810 : une RNF négative est un cas prévu (« maintien de cotisations sociales pendant une période de […] suspension de
  contrat ») ; **le montant soumis au PAS se déclare à 0.00**, jamais négatif ; le PAS est 0.00 ;
- les fiches 2640 et 2693 : un individu entièrement absent a bien une DSN ; activité de type 01 à 0.00, absence de type 02 « au
  réel » (les heures réelles du mois) ;
- la Cour de cassation (1982, 1987) : la retenue par heure d'absence est le quotient du salaire mensuel par le nombre d'heures de
  travail du mois considéré ; la DDETS en tire « le salaire horaire réel du mois » ;
- L1225-54 : le congé parental à temps plein compte pour moitié dans l'ancienneté ; L3141-5 ne l'assimile pas à du travail effectif.

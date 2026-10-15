# Union européenne et Espace économique européen : quels États

Relevé le 10/10/2026 sur EUR-Lex (le serveur répond depuis le conteneur), pour la codification UE de la DSN
(`S21.G00.30.013`, vague 1j). Les `.html` sont tels que servis ; les `.txt` en sont la transcription (balises ôtées).

| fichier | d'où | ce qu'on en lit |
|---|---|---|
| `eurlex-tue-art52-version-consolidee-20160607.html` / `.txt` | https://eur-lex.europa.eu/legal-content/FR/TXT/HTML/?uri=CELEX:12016M052 (TUE, version consolidée, JO C 202 du 07/06/2016, p. 44) | Art. 52 § 1 : « Les traités s'appliquent au Royaume de Belgique, à la République de Bulgarie, […] au Royaume de Suède et au Royaume-Uni de Grande-Bretagne et d'Irlande du Nord. » 28 États, dont le Royaume-Uni. |
| `eurlex-accord-retrait-royaume-uni-20200131.html` / `.txt` | https://eur-lex.europa.eu/legal-content/FR/TXT/HTML/?uri=CELEX:12020W/TXT (JO L 29 du 31/01/2020, p. 7) | Considérant : « le droit de l'Union […] cesse d'être applicable au Royaume-Uni à partir de la date d'entrée en vigueur du présent accord » ; art. 126 : la période de transition « se termine le 31 décembre 2020 » ; art. 185 : entrée en vigueur. |
| `eurlex-accord-eee-19940103.html` / `.txt` | https://eur-lex.europa.eu/legal-content/FR/TXT/HTML/?uri=CELEX:21994A0103(01) (JO L 1 du 03/01/1994, p. 3) | Préambule : les parties contractantes d'origine, dont l'Autriche, la Finlande, l'Islande, le Liechtenstein, la Norvège, la Suède et la Suisse. |
| `eurlex-synthese-accord-eee-20160524.html` / `.txt` | https://eur-lex.europa.eu/legal-content/FR/TXT/HTML/?uri=LEGISSUM:em0024 (synthèse EUR-Lex, dernière modification le 24/05/2016) | « Il porte actuellement sur les 28 pays de l'Union européenne (UE) et trois des quatre pays membres de l'Association européenne de libre-échange (AELE) : l'Islande, le Liechtenstein et la Norvège. » ; « la Suisse ait décidé ultérieurement de rejeter cet accord » ; l'Autriche, la Finlande et la Suède ont rejoint l'Union en 1995. |

## Ce que le moteur en retient

- **L'Union** : les 28 États de l'art. 52 § 1, moins le Royaume-Uni, soit 27 (la France comprise).
- **L'EEE hors de l'Union** : l'Islande, le Liechtenstein et la Norvège.
- **Le Royaume-Uni et la Suisse** ne sont dans aucune des deux listes : « reste du monde » (04). C'est une
  **inférence par absence** : la norme DSN ne les nomme pas.
- La synthèse EUR-Lex est un document de présentation (« n'a qu'une valeur documentaire ») : elle sert à dire qui
  est partie **aujourd'hui** à l'accord EEE, que le texte de 1994 ne dit pas.

Le moteur s'en sert dans `atelier/sources/paie/src/codification_ue.rs` (`UNION`, `EEE_HORS_UNION`, `code_du_pays`),
avec la norme (cahier technique DSN 2026.1, p. 173 : `atelier/reference/norme-dsn/texte/bloc-S21.G00.30.txt`, l. 267-277).
Les codes ISO à deux lettres des pays (ceux de `S21.G00.30.015`) ne viennent pas d'un texte déposé ici : depuis la vague 1k,
ils se lisent dans le Code officiel géographique de l'INSEE (`../insee-cog-pays/LISEZ-MOI.md`).

## L'heure d'été (relevé le 13/10/2026)

EUR-Lex ne répondait pas au conteneur ce jour-là (défi anti-robots, HTTP 202 vide) ; le même document a été servi par
l'Office des publications (`https://publications.europa.eu/resource/celex/32000L0084`, qui renvoie la page EUR-Lex
`32000L0084`). Le `.html` est tel que servi, le `.txt` sa transcription.

| fichier | d'où | ce qu'on en lit |
|---|---|---|
| `eurlex-directive-2000-84-ce-heure-d-ete-20010202.html` / `.txt` | Directive 2000/84/CE du Parlement européen et du Conseil du 19 janvier 2001 concernant les dispositions relatives à l'heure d'été, JO L 31 du 02/02/2001, p. 21-22 (CELEX 32000L0084) | Art. 1 : l'heure d'été est « la période de l'année pendant laquelle l'heure est avancée de soixante minutes par rapport à l'heure du reste de l'année » ; art. 2 : « À compter de l'année 2002, la période de l'heure d'été commence, dans chaque État membre, à 1 heure du matin, temps universel, le dernier dimanche de mars » ; art. 3 : « […] se termine, dans chaque État membre, à 1 heure du matin, temps universel, le dernier dimanche d'octobre » ; art. 4 : la Commission publie le calendrier tous les cinq ans ; considérant (1) : la directive 97/44/CE réglait les années 1998 à 2001. |

**Ce que le moteur en retient** : `atelier/sources/paie/src/paris.rs` (`decalage`, `dernier_dimanche`) : UTC+2 du dernier
dimanche de mars, 1 h UTC, au dernier dimanche d'octobre, 1 h UTC ; UTC+1 le reste de l'année. Ce texte vaut **à partir de 2002** ;
les années 1996-2001 relevaient des directives précédentes (97/44/CE, 94/21/CE), non déposées : aucune date de fichier du
logiciel n'y remonte, le moteur ne les porte pas. L'Union n'a pas mis fin à l'heure d'été : la proposition de 2018 n'a pas été
adoptée à la date du relevé (inférence par absence : aucun acte abrogeant la directive 2000/84/CE n'est publié sur EUR-Lex).

## Les îles Féroé (relevé le 13/10/2026, lot U5)

EUR-Lex ne répondait pas au conteneur (défi anti-robots) ; l'Office des publications a servi les deux textes :
`https://publications.europa.eu/resource/celex/12016E355` avec `Accept: application/xhtml+xml` et `Accept-Language: fra`
(HTTP 200, `application/xhtml+xml`, 7 953 octets, `last-modified` du 20/07/2019), et
`https://publications.europa.eu/resource/celex/11972B%2FPRO%2F02` avec `Accept: text/html` et `Accept-Language: fr` (redirigé
vers `cellar:95c52e1e-e2df-48e5-a251-e8c60dfb84a4.0001.01/DOC_1`, HTTP 200, `text/html`). Les fichiers bruts sont tels que
servis ; les `.txt` en sont la transcription (balises ôtées, entités décodées).

| fichier | d'où | ce qu'on en lit |
|---|---|---|
| `eurlex-tfue-art355-version-consolidee-20160607.xhtml` / `.txt` | TFUE, version consolidée, JO C 202 du 07/06/2016, p. 197, art. 355 (CELEX 12016E355) | Chapeau : « Outre les dispositions de l'article 52 du traité sur l'Union européenne relatives au champ d'application territoriale des traités, les dispositions suivantes s'appliquent ». **Point 5** : « Par dérogation à l'article 52 du traité sur l'Union européenne et aux paragraphes 1 à 4 du présent article: a) **les traités ne s'appliquent pas aux îles Féroé** ». Point 4 : « Les dispositions des traités s'appliquent aux îles Åland » ; point 2 : les pays et territoires d'outre-mer de l'annexe II « font l'objet du régime spécial d'association défini dans la quatrième partie ». L'art. 52 § 2 du TUE (déjà déposé, ligne 17 de son `.txt`) y renvoie : « Le champ d'application territoriale des traités est précisé à l'article 355 du traité sur le fonctionnement de l'Union européenne. » |
| `eurlex-acte-adhesion-1972-protocole-2-iles-feroe-19720327.html` / `.txt` | Actes relatifs à l'adhésion du Danemark, de l'Irlande, de la Norvège et du Royaume-Uni, protocole n° 2 concernant les îles Féroé, JO L 73 du 27/03/1972, p. 163 (CELEX 11972B/PRO/02), texte en capitales tel que le Journal officiel l'a numérisé | Art. 2 : « SI LE GOUVERNEMENT DANOIS FAIT LES DÉCLARATIONS VISÉES À L'ARTICLE 1, LES DISPOSITIONS DE L'ACTE D'ADHÉSION SONT APPLICABLES AUX ÎLES FÉROÉ » (la déclaration n'a jamais été faite : le point 5 a) de l'art. 355 TFUE le constate) ; **art. 4 : « LES RESSORTISSANTS DANOIS RÉSIDANT AUX ÎLES FÉROÉ NE SONT CONSIDÉRÉS COMME RESSORTISSANTS D'UN ÉTAT MEMBRE AU SENS DES TRAITÉS ORIGINAIRES QU'À COMPTER DE LA DATE À LAQUELLE CES TRAITÉS ORIGINAIRES DEVIENDRAIENT APPLICABLES À CES ÎLES »**. |

**Ce qu'on en tire.** Les îles Féroé sont hors du territoire de l'Union (TFUE 355 § 5 a) : c'est le texte en vigueur, la
version consolidée de 2016) ; hors de l'EEE aussi, puisque l'art. 126 § 1 de l'accord EEE (déposé plus haut,
`eurlex-accord-eee-19940103.txt`, ligne 781) ne l'applique qu'« aux territoires où le traité instituant la Communauté
économique européenne […] sont d'application » ; et ses ressortissants danois résidents ne sont pas des ressortissants d'un
État membre (protocole n° 2, art. 4). Pour la codification `S21.G00.30.013` : un pays de nationalité « Féroé » est **04 -
Reste du Monde**, pas 02 (`atelier/sources/paie/src/pays.rs`, `codification` ; `codification_ue.rs`, `TERRITOIRES_HORS_UNION`).

**Ce que ce texte ne dit pas** (inférence, et ce qu'elle laisse en l'état) : le protocole n° 2 ne parle que des ressortissants
danois *résidant* aux Féroé ; un Danois du Danemark qui est né aux Féroé garde la nationalité danoise et se déclare au
Danemark (02), pas aux Féroé. Les autres territoires associés (Groenland, Aruba, Curaçao, Saint-Martin néerlandais,
Bonaire), qui relèvent de l'art. 355 § 2 et de son annexe II (non déposée), gardent la codification de leur pays de
rattachement : leurs ressortissants (Danois, Néerlandais) sont des ressortissants d'un État membre, ce que le protocole n° 2
refuse aux seuls résidents des Féroé. L'annexe II n'est pas déposée.

## Reste à déposer

- L'avis d'entrée en vigueur de l'accord de retrait (le 01/02/2020), au JO L 29 du 31/01/2020 : son identifiant
  CELEX n'a pas été trouvé ; l'art. 126 suffit pour une paie de 2026 (transition finie le 31/12/2020).
- Le protocole portant adaptation de l'accord EEE (17/03/1993, sortie de la Suisse) : la synthèse le dit seulement.
- La liste ISO 3166-1 des codes de pays.

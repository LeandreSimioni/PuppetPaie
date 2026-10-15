# Le « 100 % DSN » des intermittents — ce qui est écrit, relevé le 18/09/2026

Question posée le 17/09 (relecture du document « tel qu'il est ») : la fin
d'un CDDU d'intermittent qui court sur plusieurs mois, donc porté par
plusieurs DSN mensuelles, se déclare-t-elle dans la DSN mensuelle du dernier
mois (comme le fait `moteur/dsn.py`, bloc 62 au dernier mois, `62.017 = 01`),
ou exige-t-elle un signalement FCTU sous cinq jours ouvrés ? Et France
Travail réédite-t-il l'AER CS d'un mois corrigé par une DSN ultérieure ?

Léandre, le 18/09 : ne pas déranger France Travail, la réponse est
forcément déjà écrite quelque part. Voici ce qui a été trouvé, et ce qu'on
en conclut.

## Les textes de la norme (officiels)

| Fichier | Contenu | Relevé le | SHA-256 |
|---|---|---|---|
| `../../norme-dsn/dsn-cahier-technique-2026.1.pdf` | §2.5.3 « Dérogation pour les CDDU » : le circuit dérogatoire « ne peut s'appliquer qu'aux contrats à durée déterminée d'usage infra DSN mensuelle [...] intégralement porté dans une seule DSN mensuelle. Dans les autres cas, le déclarant doit procéder selon la procédure de droit commun (signalement) ». §5.2 « Signalement FCTU » : « transmis dans les cinq jours ouvrés [...] **sauf dans le cadre des dérogations existantes** : salariés intérimaires [...] ; **artistes du spectacle ou ouvriers et techniciens de l'édition d'enregistrement sonore, de la production cinématographique et audiovisuelle, de la radio, de la diffusion et du spectacle (AEM)** ; déclarant ayant recours à la dérogation pour les contrats d'usage infra-DSN mensuelle ». Rubrique 62.017 : CCH-11 (obligatoire pour tout CDDU dont on déclare la fin), CCH-12 (01 autorisé si CDD + usage + motif 031 + 62.006) — **aucun contrôle ne vérifie l'infra-mensualité**. | 03/09/2026 | voir `../../norme-dsn/LISEZMOI.md` |
| `../../norme-dsn/dsn-cahier-technique-2027.1.1.pdf` | Cahier technique 2027.1.1 du 10/06/2026 (déposé par Léandre le 18/09). Sur tous ces points, **mot pour mot identique** à la 2026.1 : §2.5.3, §5.2, 62.017 et ses contrôles, 40.025 avec ses valeurs 05 et 06. La norme n'a pas été réécrite pour le 100 % DSN. | 18/09/2026 | `15934486…5bae30bf` |

## Le courriel de France Travail (officiel, déposé le 11/10/2026)

| Fichier | Contenu | Relevé le | SHA-256 |
|---|---|---|---|
| `francetravail-courriel-fin-phase-pilote-100-dsn-20260528.pdf` (+ `.txt`) | Courriel de **France Travail** (`noreply@email.francetravail.fr`) à la compagnie, **28/05/2026, 9 h 38**, « Fin de la phase pilote DSN - Passage en 100% DSN au 1er juin 2026 », imprimé depuis Gmail par Léandre. « Votre entreprise ne sera plus en phase pilote DSN à compter du **1er juin 2026**. Vous serez désormais en 100% DSN » : (1) « **Fin des Attestations Employeur Mensuelles (AEM)** : vous n'avez plus à émettre d'AEM pour vos salariés, ni à les transmettre à France Travail » ; (2) « vous devez désormais leur remettre leurs **AE Cinéma Spectacle** » ; (3) « **Ne plus renseigner de motif d'exclusion DSN dans la rubrique S21.G00.40.025. Celle-ci doit rester vide.** » C'est **l'autorisation** que la fiche 3344 annonçait (« dès lors que France Travail leur en donnera l'autorisation »), et la source de la date du 01/06/2026 (`docs/ARCHITECTURE.md`, « Plus d'AEM produite depuis le 01/06/2026 »). Il vaut pour **cette compagnie** : une autre compagnie a reçu, ou recevra, le sien (généralisation annoncée pour janvier 2027 par les éditeurs, ci-dessous) ; la date de bascule est donc une donnée de la compagnie, pas du logiciel. | 11/10/2026 | `2a5c1258…3f4c1eb9` |

La page Movinmotion ci-dessous a été relue le 11/10/2026 (capture `.mht` de Léandre) : même article, non redéposé.

## Les fiches net-entreprises (officielles, antérieures à la bascule)

| Fichier | Contenu | Relevé le | SHA-256 |
|---|---|---|---|
| `net-entreprises-fiche-2062-derogations-fin-de-contrat-20250905.html` | Fiche 2062 (modifiée 05/09/2025). Dérogation CDDU-D : conditions a-d, « la dérogation n'est applicable en DSN que pour les contrats CDDU infra-DSN », « aucun signalement de type FCTU n'est attendu quand la dérogation est appliquée ». **Exclusion de population** : « il ne doit donc pas être réalisé de signalements FCTU en DSN pour les contrats concernés par un motif d'exclusion (S21.G00.40.025) : [...] contrat de travail faisant l'objet d'une AEM pour un artiste du spectacle ». Un FCTU envoyé par erreur pour un tel contrat : pas d'AER, un CRM d'anomalie, « aucune information ne parvient à France Travail ». | 18/09/2026 | `fff721e3…96e419f0` |
| `net-entreprises-fiche-3344-intermittents-20251113.html` | Fiche 3344 (13/11/2025). « En juillet 2025, une phase d'expérimentation a été lancée afin de remplacer progressivement l'attestation employeur mensuelle par la DSN. » 40.025 : « ces valeurs ne seront plus à renseigner par les employeurs dès lors que France Travail leur en donnera l'autorisation » (c'est le mail du 28/05/2026 pour la compagnie). **L'exemple officiel est un artiste en CDDU du 01/07/2025 au 31/12/2025** — six mois — déclaré en DSN mensuelle, 40.010 = 31122025 : la norme prévoit bien le CDDU d'intermittent sur plusieurs mois en mensuelle. Rien sur la fin de contrat. | 18/09/2026 | `218da797…6f0771eb` |

## Les éditeurs du pilote (secondaires, postérieurs à la bascule)

| Fichier | Contenu | Relevé le | SHA-256 |
|---|---|---|---|
| `movinmotion-fin-des-aem-20260601.html` | Movinmotion, 01/06/2026 — « nous travaillons avec France Travail sur la mise en place de ce nouveau dispositif ». Tableau avant/après : « Contrats longs (courants sur plusieurs mois) : 1 AEM éditée à la fin de chaque mois concerné par le contrat → **1 AE CS éditée à la fin du dernier mois concerné par le contrat** ». « Mise en place définitive au mois de janvier 2027. » | 18/09/2026 | `e2302540…2c479ef2` |
| `mescachets-aem-aer-cs-20260903.html` | MesCachets, 03/09/2026 — « Pour un contrat long couvrant plusieurs mois, l'attestation est produite à la fin du dernier mois concerné. » Erreur : « contacter votre employeur afin qu'une correction soit adressée à France Travail ». Généralisation janvier 2027. | 18/09/2026 | `24c403fc…4e64fbc2` |
| `comenolo-aer-cs.html` | Comenolo (non daté) — « Pour un contrat qui court sur plusieurs mois, une seule AER CS est éditée à la fin. » | 18/09/2026 | `cc3b550a…7f72d082` |
| `towr-fin-aem-20260607.html` | Towr, 07/06/2026 — « Si une période ne remonte pas, ou remonte mal, ton employeur ou son service de paie peut rectifier la déclaration. » | 18/09/2026 | `bfca67af…672cc894` |

Pages officielles relues le 18/09 et **sans réponse** : francetravail.fr
« AEM et intermittents du spectacle » (dit encore que les AEM sont
obligatoires, ne connaît que la certification du compte au 20/07/2026) ;
la base net-entreprises n'a aucune fiche sur le 100 % DSN. `www.net-entreprises.fr`
(journal de maintenance 2027, note différentielle 2026.1 → 2027.1) était
injoignable le 18/09 au soir depuis ce poste : à relire quand il répond.

## Ce qu'on en conclut (18/09/2026)

1. **Pas de signalement FCTU pour un intermittent, quelle que soit la durée
   du contrat.** La norme (2026.1 et 2027.1, §5.2) range les artistes et
   techniciens du spectacle parmi les dérogations *existantes* au délai de
   cinq jours, **à côté** de la dérogation CDDU infra-mensuelle : ce sont
   deux dérogations distinctes, et la sectorielle ne dépend pas de la
   durée. Sous l'AEM, la fiche 2062 interdisait même le FCTU pour eux.
   Sous le 100 % DSN, France Travail édite l'AER CS « à la fin du dernier
   mois concerné par le contrat » (trois éditeurs, dont un partenaire du
   pilote) — c'est-à-dire à partir de la DSN mensuelle qui porte le bloc 62,
   exactement ce que fait `dsn.py`.
2. **`62.017` reste à `01`.** La rubrique est obligatoire (CCH-11) ; `02`
   annonce un signalement qui ne viendra jamais ; `01` dit « la fin est
   déclarée dans la mensuelle », ce qui est vrai. La condition
   d'infra-mensualité du §2.5.3 est une condition de la dérogation
   *CDDU-D* (délai, pénalités), pas de la dérogation sectorielle, et aucun
   contrôle ne la vérifie. C'est la valeur que CoolPaie a toujours envoyée
   et que France Travail a lue pour l'AER d'août.
3. **L'AER CS d'un mois corrigé** : aucune source ne dit si elle est
   rééditée. Les éditeurs disent seulement « l'employeur rectifie sa
   déclaration ». Le premier dépôt réel qui porte une correction le dira ;
   d'ici là, ne rien promettre au salarié.
4. Le premier vrai contrat sur plusieurs mois vérifiera le point 1 par
   l'AER CS reçue (nature 46) : une seule, au dernier mois, portant les
   cachets de tous les mois.

**Précisé par le lot C1 (14/10/2026).** Le point 1 vaut pour la fin
**à son terme** (motif 031). Un CDDU rompu avant son terme (036, 037,
084…) ne peut pas porter 62.017 = 01 (CCH-12 : 01 « uniquement si » le
motif est 031 ou 099), et la fiche 2062 veut un FCTU pour « tous les
autres motifs de rupture » ; l'exclusion « AEM » (fiche 2062, § 5.2 du
cahier) suit 40.025, vide depuis le courriel du 28/05/2026 (inférence
écrite). Le moteur écrit donc 02 et range cette fin parmi celles qui se
signalent (`contrats-base.json`, `cddu_spectacle.fin_de_contrat.circuit_derogatoire`).

## Dans le dépôt public (15/10/2026)

Ce dossier est dans le dépôt public de PuppetPaie. Les originaux suivants, d'un organisme privé, n'y sont pas : seule leur transcription y est (`pdftotext -layout` pour un PDF, une feuille de calcul en CSV, feuille par feuille, le texte d'une page HTML), et c'est elle que citent les renvois. Les condensats donnés plus haut sont ceux des originaux, gardés dans le dépôt privé (`reference-privee/sources/francetravail-100-dsn/`).

- `comenolo-aer-cs.html` : transcription `comenolo-aer-cs.txt`
- `mescachets-aem-aer-cs-20260903.html` : transcription `mescachets-aem-aer-cs-20260903.txt`
- `movinmotion-fin-des-aem-20260601.html` : transcription `movinmotion-fin-des-aem-20260601.txt`
- `towr-fin-aem-20260607.html` : transcription `towr-fin-aem-20260607.txt`

Restent hors du dépôt public, dans le dépôt privé (`reference-privee/sources/francetravail-100-dsn/`) :

- `francetravail-courriel-fin-phase-pilote-100-dsn-20260528.pdf` : relevé sur une compagnie réelle
- `francetravail-courriel-fin-phase-pilote-100-dsn-20260528.txt` : relevé sur une compagnie réelle

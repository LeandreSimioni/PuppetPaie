# Les échéances de la DSN et leur report (samedi, dimanche, jour férié) — relevé du 15/10/2026

La question : quand une échéance de DSN tombe un samedi, un dimanche ou un jour férié, est-elle reportée, et à quand ? Et
ses voisines : le « jour ouvré » des signalements, les quarante-huit heures de la déclaration d'accident du travail. Le
raisonnement est dans `RAPPORT-20261015.md` ; ce fichier dit ce que chaque pièce est, ce qu'on y lit, et où le moteur
s'en sert. Les textes tranchent (Léandre, 15/10/2026) : le report de l'échéance mensuelle est entré dans le moteur le jour
même (« Où le moteur s'en sert »).

## Ce qui est déposé ici

| fichier | ce que c'est | URL ou origine | SHA-256 |
|---|---|---|---|
| `net-entreprises-calendrier-des-declarations-20261015.html` | La page « Calendrier des déclarations » de net-entreprises.fr, **telle que servie** le 15/10/2026 (la page affiche, mois par mois, les dates d'exigibilité des trois ou quatre mois à venir) | https://www.net-entreprises.fr/calendrier-des-declarations/ | `aa9f068668e99bbdb6a9630c614e9a759cfdc1002beb6a80803ac49bd34b9a8a` |
| `net-entreprises-calendrier-des-declarations-20261015.txt` | Sa transcription : le bloc du calendrier seul (octobre à décembre 2026), sans le bandeau ni le pied de page | — | `2447450b9d37e5dd3eb0334161a7834f00595197797e9d72944732128c0139f0` |
| `net-entreprises-fiche113-signalement-arret-de-travail-20220211.html` | La fiche 113 de la base de connaissances net-entreprises, telle que servie (« Date de modification : 11/02/2022 ») | https://net-entreprises.custhelp.com/app/answers/detail/a_id/113/~/signalement-arr%C3%AAt-de-travail | `7bb120e4dfa3248226d20ba2e417e97afe275ec8005087b09432d9bae9f1b28d` |
| `net-entreprises-fiche113-signalement-arret-de-travail-20220211.txt` | Sa transcription (le corps de la réponse) | — | `21e63894cc1026080dd4a1a8d584605d2c3615876068771ad3d78d911c571837` |
| `net-entreprises-gestion-djt-employeurs-20220729.pdf` | « Pour une bonne gestion des arrêts de travail dans la DSN », document de l'Assurance Maladie et de net-entreprises (métadonnées du PDF : créé le 14/08/2019, modifié le 29/07/2022). **Seul le PDF est déposé** ; il se lit par `pdftotext -layout` | https://www.net-entreprises.fr/media/documentation/gestion-djt-employeurs.pdf | `1e8251c85de5fc3b064ec4f855fffa3f549e96f125e3e36e4f624b6378e4208f` |
| `legi-securite-sociale-R243-6-LEGIARTI000034668597.xml` | **R. 243-6 du code de la sécurité sociale**, le XML LEGI de la version en vigueur depuis le 01/01/2018 (toujours en vigueur au 15/10/2026), tiré de l'entrepôt `../legi-entrepot/articles.jsonl.gz` (articles en vigueur des textes suivis, clé `securite-sociale`) : l'article n'est pas encore dans la liste de `releve_codes/textes.rs`, donc pas dans un `articles-AAAAMMJJ.md` | LEGI (DILA), `echanges.dila.gouv.fr/OPENDATA/LEGI/` | `d38910c83a2ffe68860af13f330337dfdfd69aa2dfc9b15792df49ef9432c915` |
| `droit-org-code-de-procedure-civile-articles-640-a-642-1-20261015.xml` | Les articles 640, 641, 642 et 642-1 du **code de procédure civile**, extraits tels quels (les quatre éléments `<article>`) de la copie de LEGI de `codes.droit.org` (`lastup="2026-10-01"`). **Copie tierce de LEGI** : Légifrance répond 403 au conteneur et le code de procédure civile n'est pas suivi par `paie releve-codes` | https://codes.droit.org/payloads/Code%20de%20proc%C3%A9dure%20civile.xml (charge entière non déposée) | `96c1aba215d85b76ccc2ee6a53ecacc9bde35af551d7db5dae550ae2dba96302` |

La charge entière de `codes.droit.org` (1,8 Mo, non déposée) avait pour SHA-256 `8b3189238e43c317be7c0fced134aa3962c418ef4e49d242d5f0788e97145117`.

## Ce qui est cité sans être redéposé

- `../code-de-la-securite-sociale/xml/R133-14.xml` et `articles-20261015.md` (R. 133-14, en vigueur depuis le 01/07/2026,
  décret 2026-425 du 30 mars 2026) ; `xml/R133-13.xml` ; `xml/R441-3.xml` (R. 441-3, en vigueur depuis le 01/12/2019).
- `../arrete-2013-delai-evenements-dsn/` (arrêté du 2 juillet 2013, article 1er : « cinq jours ouvrés »).
- `../../norme-dsn/texte/chapitre-1-la-norme.txt` (§ 1.4.2) et `chapitre-5-modeles.txt` (§ 5.2 à 5.4), extraits du
  cahier technique 2026.1 (CT2026.1.2 du 24/12/2025) ; le cahier 2027.1.1 dit la même chose aux mêmes endroits.
- `../net-entreprises-base-connaissances/net-entreprises-fiche673-…-20250513.txt` (« 5 jours ouvrés à connaissance de
  l'évènement »).
- `../accident-du-travail/` (le cerfa 14463*03 : « 48 heures (non compris les dimanches et jours fériés) »).

## Ce qu'on en lit, en une ligne chacun

- **R. 133-14, I, 3e phrase** : « Si le délai imparti pour effectuer la déclaration sociale nominative expire un jour férié
  ou non ouvré, il est prorogé jusqu'au jour ouvrable suivant. » C'est le texte du report de l'échéance mensuelle.
- **R. 243-6, II** : le 5 (≥ 50 salariés et paie dans le mois de la période de travail) ou le 15 du mois suivant ; **aucun
  report dans cet article** (aucun « férié », « ouvrable » ni « ouvré » dans son XML).
- **Calendrier net-entreprises** : novembre 2026, DSN d'octobre : jeudi 5 et **lundi 16** (le 15 est un dimanche) ;
  décembre 2026, DSN de novembre : **lundi 7** (le 5 est un samedi) et mardi 15. La pratique du point de dépôt est le
  lundi suivant, pour un samedi comme pour un dimanche.
- **Cahier technique § 1.4.2** : échéance au 5e ou au 15e jour, « heure limite du dépôt […] 12h (midi) en France
  métropolitaine » ; aucun mot sur le week-end ni les fériés (le texte entier du cahier a été cherché : « férié »,
  « week-end », « samedi », « dimanche » n'y figurent que dans des énumérations sans rapport).
- **Code de procédure civile, 642** : « Le délai qui expirerait normalement un samedi, un dimanche ou un jour férié ou chômé
  est prorogé jusqu'au premier jour ouvrable suivant. » (Ce n'est pas le code civil.)
- **Fiche 113** : « dans un délai de 5 jours » (sans « ouvrés ») ; **fiche 673** : « 5 jours ouvrés à connaissance de
  l'évènement » ; **cahier § 5.2** : « cinq jours ouvrés qui suivent la survenance » ; **§ 5.3** : « 5 jours après la prise
  de connaissance » ; **§ 5.4** : « cinq jours ouvrables qui suivent la reprise ».
- **R. 441-3** : « dans les quarante-huit heures non compris les dimanches et jours fériés » : le samedi n'est pas nommé.

## Où le moteur s'en sert

- **`atelier/sources/paie/src/echeance_dsn.rs`**, le seul endroit qui donne l'échéance de dépôt d'une DSN mensuelle :
  le 5 ou le 15 du mois suivant (R. 243-6, II ; le choix est `Contexte::echeance_dsn`, l'effectif), **reporté au premier
  jour ouvré suivant** un samedi, un dimanche ou une fête légale (R. 133-14, I ; le calendrier de net-entreprises pour le
  samedi), **à midi** (cahier technique § 1.4.2). Ses inférences, écrites dans le module et rendues dans le champ
  `inference` de chaque réponse : « non ouvré » se lit sur le calendrier du point de dépôt (samedi, dimanche, les onze
  fêtes légales de `feries::feries`), pas sur les fériés chômés de la compagnie ; « jour ouvrable suivant » se lit
  « premier jour ouvré » (un vendredi férié reporte au lundi) ; la veille de l'annule-et-remplace est celle de l'échéance
  reportée.
- Ses appelants : l'état du mois et l'annule-et-remplace (`routes_mois.rs`, `Moteur::limite`, et par lui `produire_mois.rs`,
  `depot_acte.rs`, `routes_regularisation.rs`), les régularisations (`routes_listes.rs`), le bilan des mois
  (`routes_accueil.rs`), l'échéancier (`routes_saisie.rs`, `logiciel/donnees/echeancier-base.json`), les rappels
  (`routes_ouverture.rs`), les réveils du filet (`filet.rs`), le mois hors délai (`routes_saisie.rs`, `avenant.rs`),
  l'horloge des compagnies fictives (`horloge.rs`) et la date butoir d'un **signalement d'arrêt subrogé**
  (`obligations.rs`, `signalement_dsn` : R. 133-14, II, premier alinéa, la DSN du mois de l'événement, et son report).
  Les routes rendent, à côté de leurs champs, `echeance_depot` (`echeance_dsn` sur une obligation) : le jour, `heure`
  (`12:00`), la date `nominale`, `reportee`, `pourquoi`, les `textes` et l'`inference`.
- **Ce qui n'en dépend pas** : la date de paiement du bloc 20 (`S21.G00.20.011`, `dsn.rs`, `echeance_mois_suivant`) reste
  le 5 ou le 15 de R. 243-6, qui ne prévoit pas de report ; aucun texte déposé ne dit qu'un prélèvement se reporte.
  De même la date du prélèvement Urssaf que la boîte de dépôt montre (ticket #223, `ordres_paiement.rs`) : la date
  nominale de `echeance_dsn::du_mois`, sans report.
- Les cinq jours ouvrés des signalements non subrogés (`obligations.rs`, `Calendrier::jours_ouvres_apres`) et les
  quarante-huit heures de l'accident (`Calendrier::heures_apres`) ne passent pas par ce report : les premiers sont déjà
  comptés en jours ouvrés, les seconds excluent d'eux-mêmes dimanches et fériés (R. 441-3).
- Les tests : `atelier/sources/paie/tests/calcul/echeance_dsn.rs` (la règle) et `tests/routes/echeance_dsn.rs` (ce que
  les routes rendent).
- Reste ouvert (`RAPPORT-20261015.md`, « Ce qui reste ouvert ») : le vendredi férié (inférence : lundi), le calendrier
  de qui « non ouvré » parle (inférence : le point de dépôt), la veille de l'annule-et-remplace quand l'échéance est
  reportée.

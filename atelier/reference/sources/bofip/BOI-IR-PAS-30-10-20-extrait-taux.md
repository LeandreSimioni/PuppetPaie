# BOI-IR-PAS-30-10-20 (version du 03/12/2018) : la mise à disposition du taux de prélèvement et sa durée de validité

Page brute : `BOI-IR-PAS-30-10-20-20181203.html`, servie le 30/10/2026 par
https://bofip.impots.gouv.fr/bofip/11166-PGP.html/identifiant=BOI-IR-PAS-30-10-20-20181203 (trouvée par les liens de
l'actualité ACTU-2018-00014, `11180-PGP` ; « Version en vigueur du 03/12/2018 à aujourd'hui »). Extrait, tel quel :

> **10** — Le débiteur applique aux revenus soumis à la RAS (BOI-IR-PAS-10-10-10) de chaque bénéficiaire le taux de
> prélèvement qui lui a été transmis par l’administration fiscale ou à défaut, le taux résultant de l'application d'une
> grille de taux par défaut.
>
> **20** — […] le taux de prélèvement à la source est mis à disposition du collecteur par le biais d'un compte-rendu
> établi par l'administration fiscale en retour de chaque déclaration sociale nominative (DSN) […]
>
> Les personnes tenues d'effectuer la retenue à la source et qui versent pour la première fois un revenu peuvent
> demander à l'administration fiscale, par anticipation, la mise à disposition du taux propre au contribuable. […]
>
> **30** — Le compte-rendu est mis mensuellement à la disposition des débiteurs de la RAS […]. Le compte-rendu est
> transmis le mois au cours duquel la DSN ou la déclaration PASRAU a été déposée par le collecteur, dans un délai qui ne
> devrait pas excéder 5 jours ouvrés après réception de la déclaration par l'administration fiscale.
>
> **C. Durée de validité du taux**
>
> **70** — Conformément au premier alinéa du 2 de l'article 1671 du CGI, le taux de prélèvement est applicable jusqu'à
> la fin du deuxième mois qui suit sa mise à disposition du débiteur par l'administration fiscale.
>
> Exemple 1 : Un collecteur transmet une DSN le 5 février pour déclarer les revenus d'un salarié versés le 29 janvier.
> L'administration fiscale met à disposition du collecteur le compte-rendu comportant le taux de prélèvement du salarié
> le 10 février. Le taux est valide jusqu'au 30 avril. Le collecteur peut appliquer ce taux pour précompter la retenue à
> la source sur les revenus versés au titre des mois de février, mars et avril déclarés respectivement le 5 mars, le
> 5 avril et le 5 mai.
>
> **80** — Le débiteur de la RAS applique le taux issu du compte-rendu le plus récent transmis par l'administration
> fiscale. Toutefois, si le débiteur n'a pas la possibilité d'appliquer le taux le plus récent mis à sa disposition dans
> le dernier compte rendu, il peut alors appliquer un taux de prélèvement issu d'un compte-rendu antérieur dès lors que ce
> taux est toujours valide à la date de son application. […]
>
> La fin de la relation contractuelle entre un salarié et son employeur ne fait pas obstacle à ce que ce dernier applique
> le taux transmis par l'administration fiscale pour tout revenu versé postérieurement à la date de fin du contrat […],
> dès lors que le taux est encore valide. Au-delà de la période de validité du dernier taux transmis par l'administration
> fiscale, l'employeur appliquera alors un taux par défaut.

## Ce qu'on en lit, et où le moteur s'en sert

- § 70 : un taux vaut jusqu'à la fin du deuxième mois qui suit sa mise à disposition (le compte rendu) ; c'est la lettre
  du CGI, art. 1671, 2 (`code-general-des-impots/articles-20261021.md`). `droit::pas::VALIDITE_MOIS` et
  `droit::pas::fin_de_validite`, éprouvés sur l'exemple 1 (test `un_taux_vaut_jusqu_a_la_fin_du_deuxieme_mois_qui_suit_sa_mise_a_disposition`).
- § 80 : le taux appliqué doit être valide le jour où il s'applique, c'est-à-dire au paiement (CGI, art. 1671, 1).
  **Inférence** (aucun texte ne date un taux qu'on lit dans une DSN déjà déposée, la DSN ne portant pas la date du compte
  rendu) : un taux de type 01 qu'une DSN reprise applique a été mis à disposition au plus tard le jour du versement
  (S21.G00.50.001) ; l'import le date de ce jour, la date la plus tardive que les textes permettent
  (`moteur/src/commandes/pas.rs`, `derniers_taux_des_dsn`, champ `applique_le` lu par `droit::pas::mise_a_disposition`).
- § 20, second alinéa : la demande de taux par anticipation (le signalement d'amorçage) ; le simulateur y répond par un
  compte rendu (`envois/src/organismes_simules.rs`).

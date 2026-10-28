# BOI-IR-PAS-30-10-50 (version du 27/02/2019) : le prélèvement à la source d'un rappel et d'un trop-versé

Page brute : `BOI-IR-PAS-30-10-50-20190227.html`, servie le 28/10/2026 par
https://bofip.impots.gouv.fr/bofip/11688-PGP.html/identifiant=BOI-IR-PAS-30-10-50-20190227 (trouvée par le plan
https://bofip.impots.gouv.fr/doctrine/BOI-IR-PAS, puis BOI-IR-PAS-30 et BOI-IR-PAS-30-10 ; « Version en vigueur du
27/02/2019 à aujourd'hui »). SHA-256 du brut : `8ac1b6c68f7278121b40ffcfc9e9bbc43ee96d0b14b5ff8443dd82b629beb1a2`.
Titre : « IR - Prélèvement à la source de l'impôt sur le revenu - Modalités d'application du prélèvement - Modalités
d'application de la retenue à la source - Régularisation des erreurs de taux et d'assiette de prélèvement et
régularisation des trop versés de revenus ». Extraits, tels quels (`pandoc -t plain`) :

> **1** — La présente section distingue, d’une part, la régularisation des erreurs d'assiette et de taux de
> prélèvement à la source ayant abouti à une insuffisance ou un excédent de retenue à la source et, d’autre part, la
> régularisation des trop versés de revenus ayant une incidence sur le prélèvement à la source.
>
> La régularisation s'entend ici des seuls cas où le fait générateur du prélèvement à la source (le versement de
> l'élément de rémunération) est déjà intervenu. Un rappel de paie ou de prestation ne donne pas lieu à régularisation,
> mais doit être soumis au prélèvement à la source et porté dans la déclaration souscrite au titre du mois au cours
> duquel est intervenu le versement du rappel.

> **30** — […] Pour une erreur de taux, le collecteur applique le taux régularisé à la rémunération nette fiscale
> déclarée initialement le mois de l'erreur. […] Pour une erreur de calcul de la rémunération nette fiscale, le
> collecteur applique également le taux utilisé initialement le mois de l'erreur à la rémunération nette fiscale
> régularisée.

> **80** — La compensation correspond aux cas les plus courants de régularisation des trop versés de traitements,
> salaires ou revenus de remplacement. Elle consiste pour le débiteur de la retenue à la source, lors de la
> régularisation, à diminuer le revenu versé au bénéficiaire du montant du trop versé, le solde à verser étant nul ou
> positif. Lorsque la régularisation est effectuée par voie de compensation, le débiteur ne doit pas renseigner le bloc
> « régularisation » sur la déclaration mentionnée à l'article 87-0 A du CGI. Il applique au solde de la rémunération,
> déduction faite de l'indu, le dernier taux mis à sa disposition selon les modalités prévues au BOI-IR-PAS-30-10-20.

> **90** — Lorsque tout ou partie du montant trop versé ne peut être régularisé par voie de compensation avec un revenu
> soumis à la retenue à la source versé au même bénéficiaire, le montant de l'indu récupéré auprès du bénéficiaire pour
> son montant net imposable avant imputation de la retenue à la source est renseigné dans le bloc « régularisation » de
> la déclaration mentionnée à l'article 87-0 A du CGI.
>
> Pour le calcul de la retenue à la source à régulariser, il est, dans ce cas, fait application du taux de prélèvement
> qui avait été appliqué lors du versement de l'indu de rémunération, à savoir le taux de prélèvement initialement
> utilisé.

## Ce qu'on en lit

- Corriger la paie d'un mois déjà déclaré en la faisant monter, c'est verser un **rappel** (§ 1) : il se prélève comme
  un versement du mois où il est payé, au taux qui vaut ce mois-là (le taux DGFiP valide, sinon la grille de
  l'article 204 H du CGI), et se déclare dans la DSN de ce mois.
- La faire baisser, c'est un **trop-versé**. Le versement de la correction ne porte que la différence, négative : elle
  ne se compense avec rien dans ce versement (§ 80), elle va au bloc de régularisation (S21.G00.56, type 03) au **taux
  initialement utilisé** (§ 90), celui du versement déclaré.
- **Inférence** : le versement de la correction est un versement à part (son propre bloc S21.G00.50) ; la grille
  s'applique donc à lui seul, avec l'abattement d'un contrat court si le contrat en est un.

## Où le moteur s'en sert

`atelier/sources/engagement/src/etats.rs`, `prelevement_de_la_correction` : le prélèvement du versement d'une
correction ; la DSN (`declarations/src/dsn.rs`, `bloc_regularisation`) et le bulletin rectificatif
(`declarations/src/produire_regularisations.rs`, `versement_de_la_correction`) le lisent.

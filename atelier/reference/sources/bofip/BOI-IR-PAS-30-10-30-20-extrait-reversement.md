# BOI-IR-PAS-30-10-30-20 (version du 29/03/2019) : la date du reversement de la retenue à la source

Page brute : `BOI-IR-PAS-30-10-30-20-20190329.html`, servie le 20/10/2026 par
https://bofip.impots.gouv.fr/bofip/11181-PGP.html/identifiant=BOI-IR-PAS-30-10-30-20-20190329 (trouvée par le jeu de
données ouvert « bofip-vigueur » de data.economie.gouv.fr ; « Version en vigueur du 29/03/2019 à aujourd'hui »).
Extrait, tel quel :

> **II. Date à laquelle le reversement doit être opéré**
>
> **A. Principe d'un reversement mensuel**
>
> 50 — Pour les employeurs ou débiteurs relevant de la déclaration sociale nominative (DSN), le reversement de la
> retenue à la source intervient le mois suivant celui au cours duquel a eu lieu la retenue ou, si le débiteur est un
> employeur qui procède à la paie après la période mensuelle d'emploi (pratique dite du décalage de paie), le mois au
> cours duquel a eu lieu la retenue.
>
> Le versement doit être effectué au plus tard :
> - le 15 du mois pour les entreprises comptant au maximum quarante-neuf salariés ;
> - le 5 du mois pour les autres.
>
> **B. Option pour un reversement trimestriel**
>
> 60 — L'employeur dont l'effectif est inférieur à onze salariés peut, en application du quatrième aliéna du 2 de
> l'article 1671 du CGI, opter pour un reversement au plus tard le 15 du premier mois du trimestre suivant celui au
> cours duquel ont eu lieu les prélèvements.

C'est la date que l'art. 1671, 2 du CGI renvoie à un décret (« à une date fixée par décret le mois suivant ») ; le
BOFiP la donne, et cite pour PASRAU l'article 357 H ter de l'annexe III au CGI.

**Où le moteur s'en sert** : `atelier/sources/paie/src/ordres_paiement.rs`, la date du versement à la DGFiP dans la boîte
de dépôt de la DSN : le 5 ou le 15 du mois suivant, selon l'effectif (`Contexte::echeance_dsn`, le même seuil de
cinquante). L'option trimestrielle (§ 60) n'est pas modélisée : le moteur reverse chaque mois.

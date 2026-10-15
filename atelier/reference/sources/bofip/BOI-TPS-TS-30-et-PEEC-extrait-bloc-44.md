# BOFiP — taxe sur les salaires (liquidation) et PEEC (champ, base) : ce que le bloc S21.G00.44 en lit

Relevé le 10/10/2026 (lot V3b), pages brutes à côté, telles que servies par `bofip.impots.gouv.fr` ; identifiants
trouvés par l'API du jeu de données ouvert `bofip-vigueur` de `data.economie.gouv.fr` (requête sur
`identifiant_juridique like "BOI-TPS-TS-%"` et `"BOI-TPS-PEEC%"`). Transcription des seuls paragraphes qui servent.

| fichier | URL | SHA-256 (début) |
|---|---|---|
| `BOI-TPS-TS-30-20200624.html` | https://bofip.impots.gouv.fr/bofip/6686-PGP.html/identifiant=BOI-TPS-TS-30-20200624 | `aa3268782953b713` |
| `BOI-TPS-PEEC-10-20141218.html` | https://bofip.impots.gouv.fr/bofip/6360-PGP.html/identifiant=BOI-TPS-PEEC-10-20141218 | `c9b30274abea0722` |
| `BOI-TPS-PEEC-20-20141218.html` | https://bofip.impots.gouv.fr/bofip/6361-PGP.html/identifiant=BOI-TPS-PEEC-20-20141218 | `6d22ce5acd33d242` |

## BOI-TPS-TS-30 (TPS - Taxe sur les salaires - Liquidation), version du 24/06/2020

§ 40 : « Les majorations de taux applicables en métropole, soit 4,25 % (8,50 % - 4,25 %) et 9,35 % (13,60 % - 4,25 %),
s'appliquent à la fraction des rémunérations individuelles annuelles qui excèdent les seuils [...]. Les indications
relatives aux majorations ne concernent que le calcul de la taxe dans les départements métropolitains. En effet, les
taux majorés ne sont pas applicables dans les départements d'outre-mer. »

§ 110 : « si un salarié est rémunéré par plusieurs établissements dépendant d'une même entreprise, c'est l'ensemble des
rémunérations versées par l'entreprise à ce salarié qui est à prendre en considération pour la détermination de la base
de calcul des majorations. »

§ 240 : « la taxe sur les salaires due par les employeurs partiellement assujettis à la TVA est déterminée en appliquant
le rapport d'assujettissement défini au 1 de l'article 231 du CGI à chacune des fractions des rémunérations
individuelles passibles des taux majorés et non au montant total de chaque rémunération avant sa répartition entre les
tranches d'imposition. À titre de simplification, le montant de la taxe due peut également être déterminé en appliquant
ce rapport directement au montant de la taxe calculée sur le montant total de chacune de ces rémunérations. Ces deux
méthodes conduisent à un résultat identique » (exemple : 180 000 €, seuils 8 004 et 15 981, rapport 40 %).

**Où le moteur s'en sert** : `atelier/sources/paie/src/assujettissement.rs`, `assiettes_taxe_sur_les_salaires` (la part
assujettie appliquée à l'assiette au taux normal ET à chaque fraction majorée, codes 009, 015, 016) et
`fractions_majorees` (que l'état « taxe sur les salaires », `etats_compta.rs`, emploie aussi) ; le refus outre-mer de
`blocs` (§ 40). Un salarié n'a qu'un établissement dans PuppetPaie (sa fiche) : toute son année se déclare là (§ 110).

## BOI-TPS-PEEC-10 (PEEC - Champ d'application), version du 18/12/2014

§ 40 : « Les employeurs occupant au minimum vingt salariés et assujettis à la taxe sur les salaires sont tenus de
participer à l'effort de construction de logements. Il en est ainsi même dans le cas où l'employeur est, en tout ou
partie, exonéré de la taxe sur les salaires en raison de sa situation au regard de la taxe sur la valeur ajoutée (TVA) ».
§ 50 : « En pratique, tous les employeurs occupant au moins vingt salariés doivent participer à l'effort de construction
à moins qu'ils ne bénéficient d'une exonération ». Le seuil est de cinquante depuis le 01/01/2020 (CCH L. 313-1, relevé
dans `../code-de-la-construction-et-de-l-habitation/`), et l'effectif suit L. 130-1 CSS (même article, dernier alinéa) :
les § 80 à 240 (décompte de l'effectif au 31 décembre) sont dépassés.

**Où le moteur s'en sert** : la PEEC (005/006) ne dépend pas de la taxe sur les salaires de la fiche, seulement de
l'effectif (`blocs`, `effectif_atteint(ctx, 50, an, ...)`).

## BOI-TPS-PEEC-20 (PEEC - Base et montant), version du 18/12/2014

§ 1 : « La base de la participation [...] est constituée par le montant des salaires payés par les employeurs au cours
de l'année civile écoulée, entendu au sens des règles prévues aux chapitres Ier et II du titre IV du livre II du code de
la sécurité sociale ». § 10 : « les employeurs imposables au titre de l'année N doivent réaliser, avant le 31 décembre de
l'année N, des versements ou des investissements [...] calculé à raison des salaires payés au cours de l'année N-1. »
§ 20 : « les salaires sont appréciés comme en matière de taxe d'apprentissage ».

**Où le moteur s'en sert** : `assiette_peec` (montant du 005 = les salaires de l'année du millésime, base 03 de chaque
contrat). INFÉRENCE : la norme DSN ne dit pas de quelle année est l'assiette du 005 ; le millésime N porte les salaires
de N, ceux sur lesquels se calcule l'investissement de N+1.

# Compagnies fictives : le jeu de problemes

Deux sortes de compagnies fictives (lot U1, 14/10/2026 : le mot « demonstration » a quitte le logiciel) :
- **celles livrees avec le logiciel**, dans `fictives/` (`fictive-1285-grande`, `fictive-1285-petite`,
  `fictive-3090-grande`, `fictive-3090-petite`), payees de janvier 2025 a decembre 2026 ; leur marqueur
  (`COPIE-DE-DEVELOPPEMENT.json`, `"copie_de": "compagnie fictive …"`) fait jouer leurs envois. Les tests montent leurs propres
  compagnies de test ; les regles se prouvent sur celles-ci. Une installation posee avant le lot U1 les a sous
  `demo/demo-*` : le logiciel renomme ce dossier au lancement (`interface::compagnies::ranger_les_fictives`) ;
- **celles qu'on ajoute sur un logiciel bride** (ticket #236) : le « + » de l'accueil, apres un message, pose
  `fictives/ajoutee-<nom>` avec la marque `"ajoutee": true` dans son marqueur (`interface::compagnies::est_ajoutee`, lue
  la seulement). Tout s'y fait ; ses envois (depots DSN et DPAE, test compris, courriels, filet) ne sont pas joues mais
  refuses par le moteur (`bridage::refus_d_envoi`), sans rien ecrire au journal des depots. Pas de photo d'origine ni de
  remise a zero ; la livraison des fictives ne la touche jamais et ne la livre pas ; `.gitignore` l'ecarte du depot ;
- **le jeu de problemes** ci-dessous, que montait `outils/compagnies_fictives.py` : parti avec le Python (lot K, 09/10/2026)
  et **non porte** ; aucun test ne le monte, et ce bilan ne se regenere plus (`atelier/README.md`, « Les compagnies fictives »).

Son dernier bilan, produit par `outils/compagnies_fictives.py --tout` sur le mois 2026-06 (21/09/2026) ; les messages cites
sont ceux du moteur Python d'alors.
Cet outil ne repare rien : la liste EST le travail a venir, et ce qui en sort est le travail fait.

Ce que ces DSN valent au point de depot -- sept y ont ete deposees EN TEST le 21/09/2026, dont trois qui refusent depuis le meme soir -- est dans `COMPAGNIES-FICTIVES-DEPOT.md`, a cote.

| compagnie | ce qu'elle eprouve | attendu | obtenu |
|---|---|---|---|
| `privee-artistes` | Le cas de reference : deux artistes, deux spectacles, des cachets | produit | produit |
| `privee-techniciens` | Un mois sans aucun artiste : regisseurs payes a l'heure, reduction generale, CTP 100/332/671, annexe VIII | produit | produit |
| `privee-mixte-intermittents` | Les deux populations dans le meme envoi : le partage des assiettes (brut_artiste / brut_technicien) et les deux taux AT/MP dans la meme DSN | produit | produit |
| `privee-permanents` | Un CDI et un CDD ordinaires, aucun intermittent | produit | produit |
| `privee-cadre` | Une administratrice de production en CDI, statut categoriel 01 : tranches mensuelles, Apec, prevoyance a 1,50 % | produit | produit |
| `privee-tout` | Le cas le plus courant d'une compagnie qui grandit : une equipe permanente et des intermittents sur les productions | produit | produit |
| `privee-depart-retraite` | Un CDI ouvert en 2001 qui finit par un depart volontaire a la retraite (motif 039) : la loi sous 005, le supplement de la convention 3090 (art. 7.9) sous 006 | produit | produit |
| `privee-creuse` | La DSN est due quand meme : declaration normale sans individu, envoi neant, bloc 08 des organismes sans rattachement | produit | produit |
| `privee-onze` | Onze salaries et une surcouche qui ne dit que le taux AT/MP : la formation professionnelle des permanents reste a 0,55 % et le versement mobilite n'est pas tranche | refuse | refuse |
| `privee-onze-surcouchee` | La meme, dont la surcouche suit son effectif : la formation des permanents a 1,00 % et l'attestation qu'aucun versement mobilite ne la concerne | produit | produit |
| `privee-syndeac` | L'adhesion porte la retraite complementaire des artistes intermittents a 10,16 % en T1, 50/50 (Audiens, tableau n 2, releve le 25/09/2026) | produit | produit |
| `privee-oeth` | Vingt-quatre d'effectif d'assujettissement en 2025 : la DSN d'avril 2026 porte la declaration OETH (bloc 13 type 01, bloc 82 codes 062/065/066/068, CTP 730), ajoutee le 25/09/2026 et eprouvee sur avril -- pas encore deposee en test | produit | produit |
| `privee-deux-etablissements` | Un SIREN, deux NIC : un siege et un lieu | refuse | refuse |
| `privee-stage` | Une convention de stage : gratification exoneree sous le seuil, pas de contrat de travail, un bloc 40 de nature propre | refuse | refuse |
| `publique-artistes` | Meme paie, autre branche : FNAS a la place de la CASC-SVP | produit | produit |
| `publique-permanents` | Le cas ordinaire d'une scene conventionnee : des permanents, et des intermittents autour | produit | produit |
| `publique-tout` | La forme la plus repandue du secteur subventionne | produit | produit |
| `publique-techniciens` | Les techniciens de la CCNEAC : leur section de statut recopie la CASC-SVP de la 3090, que le FNAS doit remplacer | produit | produit |

## Ce qui refuse, et ce que le refus dit

### privee-onze -- Compagnie de droit prive de onze salaries, surcouche muette

Le moteur produisait sans rien dire jusqu'au 21/09/2026 ; il refuse depuis, et dit quoi ecrire. Deux griefs : la contribution a la formation professionnelle des permanents est encore a 0,55 % (1,00 % a partir de onze, art. L. 6331-1), et le VERSEMENT MOBILITE, du a partir de onze salaries, n'est porte par rien -- ni rubrique, ni ligne DSN, ni CTP. Ce qui leve le refus est `privee-onze-surcouchee`, a cote.

Ce que le moteur repond :

> ValueError : Bareme 2026, entreprise de 11 salaries : le moteur REFUSE de payer -- formation, versement_mobilite (non porte). Le socle livre avec le logiciel est ecrit pour le plus petit palier -- moins de onze salaries --, et ce qui change au-dessus n'est pas ecrit ici. Produire quand meme donnerait des bulletins et une DSN faux, sans un mot : c'est ce que le moteur faisait avant le 21/09/2026. Ce qui manque, et ou : - formation (a partir de 11 salaries) La contribution a la formation professionnelle des PERMANENTS passe de 0,55 % a 1,00 % du brut a partir de onze salaries. L'intermittent est a 2,00 % (CT (...)

### privee-syndeac -- DEPUIS LE 25/09/2026 ELLE PRODUIT (couche relevee chez Audiens : 10,16 % pour les artistes intermittents) ; ce qui suit est l'histoire -- Compagnie de droit prive adherente d'une organisation patronale

C'ETAIT LE PIRE CAS : le moteur produisait sans rien dire, la DSN partait au taux de droit commun, et aucun controle ne lisait l'adhesion. Depuis le 21/09/2026 le champ est declare (`organisation_patronale`), le socle porte une couche par organisation, et celle du SYNDEAC est `applicable: false` -- rien de ce qu'elle change n'est releve a la source. Le refus le dit et nomme les trois choses a relever : le taux, sa repartition, son perimetre.

Ce que le moteur repond :

> ValueError : Bareme 2026, adhesion a l'organisation patronale SYNDEAC (Syndicat national des entreprises artistiques et culturelles) : la couche existe mais n'est pas applicable, le moteur ne paie donc pas. RIEN N'EST RELEVE, et ce qu'on croit savoir tient en une phrase jamais verifiee : la retraite complementaire des adherents serait a 10,16 % au lieu du taux de droit commun. Trois choses a relever A LA SOURCE avant `applicable: true` -- (1) LE TAUX exact et sa REPARTITION employeur / salarie, qui ne se deduit pas d'un total ; (2) SON PERIMETRE : toutes les populations, ou les seuls permanents, ou les seu (...)

### privee-deux-etablissements -- Compagnie de droit prive a deux etablissements

La fiche entreprise ne porte qu'un `nic`, et le moteur ne declare qu'un etablissement : il faudrait un bloc 11 par etablissement, le rattachement de chaque salarie au sien, un taux AT/MP par etablissement, et `retours.py` lit le compte de salaries a la premiere occurrence (§57). Jusqu'au 21/09/2026 la DSN sortait au premier en silence ; le moteur refuse depuis, plutot que de declarer tout le monde au siege.

Ce que le moteur repond :

> ValueError : config/entreprise.json : la fiche declare 2 etablissements, et le moteur n'en declare qu'UN -- celui du `nic` 00018. Ce qu'il faudrait pour les servir, et qui n'existe pas : un bloc S21.G00.11 par etablissement, le rattachement de chaque salarie au sien (lieu de travail compris), un taux AT/MP par etablissement, et la lecture des retours par etablissement -- `retours.py` lit aujourd'hui le compte de salaries a la premiere occurrence. Etablissements que la fiche porte en plus : 00026. Le moteur refuse plutot que de declarer tout le monde au siege : une DSN qui rattache un salarie au mauvais eta (...)

### privee-stage -- Compagnie de droit prive avec un stagiaire

Rien n'existe : `moteur/contrats-base.json` ne connait que cddu_spectacle, cdi et cdd, et aucune rubrique ne porte une gratification de stage.

Ce que le moteur repond :

> ErreurOperation : Engagement 1 de 2026-06 : "Nature de contrat inconnue : 'stage'. Connues : cdd, cddu_spectacle, cdi."


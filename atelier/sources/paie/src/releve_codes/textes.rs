//! LES TEXTES QU'ON LIT, et ou leur releve se depose (`TEXTES` de `atelier/outils/relever_les_codes.py`, retire le
//! 15/10/2026). Un seul passage de l'archive les sert TOUS : ce qui coute, c'est de faire defiler 1,1 Go, pas de
//! reconnaitre un article de plus au passage.
//!
//! CE QUE LA PAIE A BESOIN DE LIRE. Un article ajoute ici est deja dans l'entrepot (`legi-entrepot/`, qui garde TOUS les
//! articles en vigueur des textes suivis) : `paie releve-codes moissonner` l'ecrit au prochain passage, sans rien
//! telecharger de plus ; un TEXTE ajoute, lui, fait relire l'archive globale pour lui seul (`completer`). Rien n'est devine,
//! tout est releve. L'ordre est celui du document lisible (`rapport`) : texte par texte, domaine par domaine.
//!
//! Les commentaires disent pourquoi chaque article est la ; ils citent le moteur sous les noms qu'il avait quand
//! l'article est entre (`calcul.representation_du_personnel`, ...).

/// Un domaine d'un texte : un titre du document lisible, et les numeros d'article qu'on y cherche.
pub struct Domaine {
    pub cle: &'static str,
    pub titre: &'static str,
    pub numeros: &'static [&'static str],
}

/// Un texte de la base LEGI : un code (son LEGITEXT) ou un texte non codifie (son JORFTEXT, range de la meme facon dans
/// le chemin du fichier), et le dossier des sources ou son releve se depose.
pub struct Texte {
    pub cle: &'static str,
    pub legitext: &'static str,
    pub nom: &'static str,
    pub dossier: &'static str,
    pub titre: &'static str,
    pub domaines: &'static [Domaine],
}

pub static TEXTES: &[Texte] = &[
    // ------------------------------------------------------------------
    Texte {
        cle: "travail",
        legitext: "LEGITEXT000006072050",
        nom: "Code du travail",
        dossier: "code-du-travail",
        titre: "Code du travail -- ce que la paie lit",
        domaines: &[
            Domaine { cle: "conges", titre: "Les conges payes", numeros: &["L3141-3", "L3141-4", "L3141-5", "L3141-5-1", "L3141-6", "L3141-9", "L3141-10", "R3141-4", "L3141-24", "L3141-28"] },
            Domaine { cle: "feries", titre: "Les jours feries", numeros: &["L3133-1", "L3133-2", "L3133-3", "L3133-4", "L3133-5", "L3133-6"] },
            Domaine { cle: "maintien", titre: "Le maintien de salaire pendant un arret de travail", numeros: &["L1226-1", "D1226-1", "D1226-2", "D1226-3", "D1226-4", "D1226-5", "D1226-6", "D1226-7", "D1226-8"] },
            Domaine { cle: "precarite", titre: "Le CDD et l'indemnite de fin de contrat", numeros: &["L1242-2", "L1242-12", "L1243-8", "L1243-9", "L1243-10"] },
            Domaine { cle: "rupture", titre: "Le preavis et l'indemnite de licenciement", numeros: &["L1234-1", "L1234-4", "L1234-5", "L1234-9", "R1234-1", "R1234-2", "R1234-4"] },
            Domaine { cle: "documents", titre: "Les documents remis a la fin du contrat", numeros: &["L1234-19", "L1234-20", "D1234-6", "D1234-7", "D1234-8"] },
            Domaine { cle: "retraite", titre: "Le depart et la mise a la retraite", numeros: &["L1237-5", "L1237-6", "L1237-7", "L1237-8", "L1237-9", "L1237-10", "D1237-1", "D1237-2", "D1237-2-1"] },
            Domaine { cle: "rupture_conventionnelle", titre: "La rupture conventionnelle", numeros: &["L1237-11", "L1237-12", "L1237-13", "L1237-14", "L1237-15", "L1237-16"] },
            // LE BULLETIN DE PAIE (21/09/2026). Le §11.1 tient une decision
            // a une reference citee de memoire : << tout paiement de salaire
            // s'accompagne d'un bulletin (L3243-2) >>, et c'est elle qui
            // commande le bulletin RECTIFICATIF du mois d'origine. Une
            // decision qui repose sur un article jamais relu est une
            // decision qu'on ne peut pas defendre.
            //
            // R3243-3 N'EST PAS DE LA LISTE, et ce n'est pas un oubli : le
            // passage du 21/09/2026 l'a rendu dans une version ABROGEE
            // depuis le 27/02/2016 (LEGIARTI000018533802). Il ne s'agissait
            // pas du piege 1 de l'en-tete -- l'article n'a plus AUCUNE
            // version en vigueur a trouver. Le laisser ici ferait dire
            // << perime >> au garde-fou a chaque passage, pour un article
            // qui n'existe plus : le bruit finirait par couvrir le vrai
            // signal, celui d'un article vivant qu'on aurait garde vieux.
            // Ce qu'il disait -- la mention des cotisations PATRONALES sur
            // le bulletin -- se lit desormais dans R3243-1, qui est de la
            // liste et en vigueur.
            // R3243-4 A R3243-6 (lot R11, 11/10/2026) : ce que le bulletin ne
            // peut pas dire (la greve, la representation), et la seule
            // derogation qui dispense de la part patronale (le particulier
            // employeur, R3243-6) -- lus dans l'entrepot pour trancher les
            // modeles de bulletin ; a relever au prochain `moissonner`.
            Domaine { cle: "bulletin", titre: "Le bulletin de paie : ce qu'il porte, et quand il est du", numeros: &["L3243-1", "L3243-2", "L3243-3", "L3243-4", "R3243-1", "R3243-2", "R3243-4", "R3243-5", "R3243-6"] },
            // L'EFFECTIF DU CSE (23/09/2026, §80.1). Les seuils de 11 et 50
            // de la CCNEAC -- et le FNAS qui en decoule -- sont ceux du comite
            // social et economique, que la convention rattache au code du
            // travail (elle cite L2315-61). L'effectif s'y compte selon
            // L1111-2, et le seuil de onze doit tenir douze mois consecutifs
            // (L2311-2) : ce n'est PAS l'effectif de l'Urssaf (L130-1 du code
            // de la securite sociale, plus bas). C'est ce qui fait que
            // `calcul.representation_du_personnel` prend la representation
            // DECLAREE pour un fait, sans l'opposer au CRM 118.
            Domaine { cle: "effectif_cse", titre: "L'effectif qui compte pour le comite social et economique", numeros: &["L1111-2", "L2311-2", "L2315-61"] },
            // LES MAJORATIONS DES EMPLOYEURS AFFILIES A UNE CAISSE DE CONGES
            // (10/10/2026, lot V1n). L243-1-3 du code de la securite sociale
            // cite ces articles-la : la caisse (L3141-32), les contributions
            // de formation et d'apprentissage qu'il majore (L6131-2, L6242-1,
            // L6331-6), la taxe d'apprentissage (L6241-1) et les intermittents
            // du spectacle (L6331-55) que D243-0-2 met a 10 %.
            // D7121-28, D7121-40 et D7121-41 : qui s'affilie aux Conges
            // Spectacles, et pour qui -- le personnel artistique et technique
            // qu'il n'a pas employe de facon continue pendant les douze mois
            // precedant la demande de conge (la lettre circulaire Acoss 2013-019,
            // annexe 1, R5, y borne la majoration).
            Domaine { cle: "majoration_caisses", titre: "Les contributions de formation et d'apprentissage majorees chez un employeur affilie a une caisse de conges", numeros: &["L3141-32", "L6131-2", "L6241-1", "L6242-1", "L6331-6", "L6331-55", "D7121-28", "D7121-40", "D7121-41"] },
            // LE CHOMAGE ET L'AGS D'UN PERMANENT (10/10/2026, lot V2e).
            // L5422-9 : les contributions << sont assises sur les remunerations
            // brutes dans la limite d'un plafond >> ; L5422-20 renvoie ses
            // mesures d'application a la convention d'assurance chomage, dont le
            // reglement general (art. 49) fixe ce plafond a quatre fois celui de
            // L. 241-3 du code de la securite sociale ; L5422-14 : l'employeur
            // declare les remunerations qui servent au calcul. L3253-18 : l'AGS
            // est assise sur les memes remunerations que le chomage.
            Domaine { cle: "chomage", titre: "L'assiette des contributions d'assurance chomage et de l'AGS", numeros: &["L5422-9", "L5422-14", "L5422-20", "L3253-18"] },
            // LE BLOC S21.G00.44 DE LA DSN DE DECEMBRE (10/10/2026, lot V3b) :
            // qui est assujetti a quoi. L6131-1 : tout employeur concourt a la
            // formation (CFP, CPF-CDD, contribution supplementaire), sauf ceux
            // de son II (Etat, collectivites et leurs EPA, hopitaux publics,
            // employeur sans etablissement en France) ; L6241-1-1 : l'assiette
            // de la taxe d'apprentissage ; L6331-1 et L6331-3 : la CFP a moins
            // et a partir de onze salaries. L6131-2, L6241-1, L6242-1, L6331-6
            // et L6331-55 sont deja releves plus haut (`majoration_caisses`).
            // D6331-72 : les CDD qui ne donnent pas lieu a la contribution
            // CPF-CDD (contrats aides, apprentissage, professionnalisation, et
            // ceux de L6321-9) ; le CDD saisonnier est exclu par L6331-6 meme.
            // L6331-1 A (13/10/2026, lot F1) : l'effectif de la CFP et le
            // franchissement de son seuil de onze se comptent selon L130-1 du
            // code de la securite sociale -- cinq ans pour le franchir a la
            // hausse, et une annee sous le seuil fait recourir le delai (le
            // controle `seuils_effectif` de `contexte.rs`).
            Domaine { cle: "assujettissement_fiscal", titre: "Qui doit la formation professionnelle et la taxe d'apprentissage (bloc S21.G00.44 de la DSN)", numeros: &["L6131-1", "L6241-1-1", "L6331-1 A", "L6331-1", "L6331-3", "D6331-72", "L6321-9"] },
            // LE TEMPS PARTIEL ET LA DUREE LEGALE (10/10/2026, lot V4b). R242-2
            // du code de la securite sociale permet a l'employeur de corriger
            // le plafond des salaries de L3123-1 a due proportion de leur
            // duree contractuelle, majoree des heures complementaires de
            // L3123-8, L3123-20, L3123-21 et L3123-28 : S21.G00.40.084 de la
            // norme 2027 declare ce choix. L3121-27 (35 h) et L3121-28 (au-dela,
            // une heure supplementaire) : la quotite de reference 2027
            // (S21.G00.40.012) << ne doit pas inclure les heures
            // supplementaires structurelles >>.
            Domaine { cle: "temps_partiel", titre: "Le temps partiel, la duree legale et les heures complementaires et supplementaires", numeros: &["L3121-27", "L3121-28", "L3123-1", "L3123-8", "L3123-20", "L3123-21", "L3123-28"] },
            // LES CONGES POUR EVENEMENTS FAMILIAUX (10/10/2026, lot V4b) : ils
            // << n'entrainent pas de reduction de la remuneration >> (L3142-2).
            // C'est ce que dit `remunere` du motif 637 (`permanents-base.json`),
            // et en 2027 le bloc S21.G00.65 ne declare plus qu'une absence << sans
            // maintien de sa remuneration normale >>.
            Domaine { cle: "evenements_familiaux", titre: "Les conges pour evenements familiaux", numeros: &["L3142-1", "L3142-2"] },
            // LE CONGE DE DEUIL D'UN ENFANT (15/10/2026, lot << le deuil >>). L3142-1-1 : huit jours, fractionnables
            // (D3142-1-1 : deux periodes d'au moins une journee), pris dans l'annee du deces ; le salarie << informe
            // l'employeur vingt-quatre heures au moins avant le debut de chaque periode d'absence >>. C'est le motif d'arret
            // 19 de la DSN ; ses indemnites journalieres sont celles de L331-9 du code de la securite sociale (plus bas).
            // `paie/src/obligations.rs` (l'attestation de salaire hors DSN, `obligations-base.json`).
            Domaine { cle: "conge_de_deuil", titre: "Le conge de deuil d'un enfant", numeros: &["L3142-1-1", "D3142-1-1"] },
            // LE REGISTRE UNIQUE DU PERSONNEL (11/10/2026, reste R1 #147) : ses
            // mentions (D1221-23 : la nationalite, 1°) sont ce que
            // `etats_personnel::registre_du_personnel` ecrit ; il citait les
            // articles sans qu'ils soient dans le releve.
            Domaine { cle: "registre", titre: "Le registre unique du personnel", numeros: &["L1221-13", "D1221-23", "D1221-24"] },
            // La DPAE quand la date d'embauche d'un contrat declare change (21/10/2026, `routes_mois::etat_par_engagement`) :
            // l'embauche apres la declaration (L1221-10), la date d'embauche dans la declaration (R1221-1, 3°), la date
            // previsible (R1221-4).
            Domaine { cle: "dpae", titre: "La declaration prealable a l'embauche", numeros: &["L1221-10", "R1221-1", "R1221-4"] },
            // LA FIN D'UN CDD (13/10/2026, lot U5) : L1243-5 la fait cesser de
            // plein droit a l'echeance du terme (aucun preavis : L1234-1 est
            // celui du licenciement) ; L1243-1 et L1243-2 disent les ruptures
            // avant le terme, dont le preavis du salarie qui a trouve un CDI.
            // Lot U7 : L1243-3 et L1243-4 (ce que coute une rupture anticipee
            // hors des cas de L1243-1), L1226-4-2, L1226-4-3 et L1226-20 (la
            // rupture d'un CDD pour inaptitude, 033) : le preavis de chaque
            // motif de rupture anticipee.
            Domaine { cle: "fin_de_cdd", titre: "La fin d'un CDD : a son terme, ou avant", numeros: &["L1243-1", "L1243-2", "L1243-3", "L1243-4", "L1243-5", "L1226-4-2", "L1226-4-3", "L1226-20"] },
            // LE PREAVIS NON EFFECTUE, MOTIF PAR MOTIF (13/10/2026, lot P1).
            // L1234-5 est au chapitre du licenciement ; ce que les autres fins
            // de contrat en font est dans ces articles-ci : la demission
            // (L1237-1) et sa rupture abusive (L1237-2), l'inaptitude non
            // professionnelle (L1226-4 : << par derogation a l'article
            // L. 1234-5 >>, pas d'indemnite), l'inaptitude professionnelle
            // (L1226-12, L1226-14 : une indemnite d'un montant egal), la force
            // majeure d'un CDI (L1234-12, L1234-13), la periode d'essai
            // (L1221-25, L1221-26 : un delai de prevenance, pas un preavis).
            Domaine { cle: "preavis_non_effectue", titre: "Le preavis non effectue : ce que chaque fin de contrat en fait", numeros: &["L1237-1", "L1237-2", "L1226-4", "L1226-12", "L1226-14", "L1234-12", "L1234-13", "L1221-25", "L1221-26"] },
            // UN MOIS ENTIER DE SUSPENSION NON REMUNEREE (15/10/2026) : lus dans l'entrepot par la recherche du jour
            // (`atelier/reference/sources/suspension-non-remuneree/legi-extrait-code-du-travail-20261015.md`), suivis
            // depuis. Le conge parental d'education (L1225-54 : sa duree compte pour moitie dans l'anciennete,
            // `imprevus::anciennete_depuis`), le conge sabbatique et le conge pour creation d'entreprise (rien sur
            // l'anciennete ; L3142-33, le report des conges payes), la mensualisation (L3242-1 : la remuneration mensuelle
            // est « independante, pour un horaire de travail effectif determine », du nombre de jours du mois ; la retenue
            // d'une absence au reel du mois, `imprevus::lignes_du_mois`), et L3251-1 (les retenues sur salaire).
            Domaine { cle: "conge_parental", titre: "Le conge parental d'education", numeros: &["L1225-47", "L1225-48", "L1225-49", "L1225-50", "L1225-51", "L1225-52", "L1225-53", "L1225-54", "L1225-55", "L1225-56", "L1225-57", "L1225-58", "L1225-59"] },
            Domaine { cle: "conge_sabbatique", titre: "Le conge sabbatique et le conge pour creation d'entreprise", numeros: &["L3142-28", "L3142-29", "L3142-30", "L3142-31", "L3142-32", "L3142-33", "L3142-34", "L3142-105", "L3142-106", "L3142-107", "L3142-108", "L3142-109"] },
            Domaine { cle: "mensualisation", titre: "La mensualisation et les retenues sur salaire", numeros: &["L3242-1", "L3242-2", "L3242-3", "L3251-1"] },
            // LA LICENCE D'ENTREPRENEUR DE SPECTACLES (#236, 23/10/2026) : de qui elle est exigee (S21.G00.40.049 de la
            // DSN, `demarrage::licence_exigee`, `dsn::licence_du_contrat`) ; la norme DSN y renvoie (« L.7122-3 et
            // suivants »). L7122-22 et L7122-23 : l'organisateur occasionnel, qui passe par le guichet unique.
            Domaine { cle: "licence_spectacles", titre: "L'entrepreneur de spectacles vivants et sa declaration", numeros: &["L7122-1", "L7122-2", "L7122-3", "L7122-4", "L7122-5", "L7122-6", "L7122-22", "L7122-23"] },
        ],
    },
    // ------------------------------------------------------------------
    // POURQUOI CE CODE-LA, a partir du 19/09/2026 : le code du travail dit
    // ce qu'une indemnite de rupture VAUT ; il ne dit pas un mot de ce
    // qu'elle SUPPORTE. Le regime social d'une indemnite de rupture
    // conventionnelle ou de retraite -- cotisations, CSG/CRDS, et la
    // contribution patronale de 30 % -- vit ici, et nulle part ailleurs.
    Texte {
        cle: "securite-sociale",
        legitext: "LEGITEXT000006073189",
        nom: "Code de la securite sociale",
        dossier: "code-de-la-securite-sociale",
        titre: "Code de la securite sociale -- ce que la paie lit",
        domaines: &[
            // R242-1 (11/10/2026, lot R9) : son troisieme alinea renvoie a des
            // arretes << les conditions et limites dans lesquelles la
            // remuneration peut faire l'objet d'un abattement pour frais
            // professionnels >> (l'arrete du 4 septembre 2025, plus bas) ; son
            // sixieme pose l'assiette minimum que la deduction forfaitaire
            // specifique ne peut pas entamer (art. 9 de l'arrete).
            Domaine { cle: "assiette", titre: "L'assiette des cotisations, et ce qui en sort", numeros: &["L242-1", "R242-1"] },
            // L241-13 (lot R9) : la reduction generale << ne tient compte des
            // deductions au titre de frais professionnels calculees
            // forfaitairement en pourcentage de cette remuneration que dans
            // des limites et conditions fixees par arrete >> (III).
            Domaine { cle: "reduction_generale", titre: "La reduction generale et les deductions forfaitaires pour frais professionnels", numeros: &["L241-13", "D241-7"] },
            // LE MONTANT NET SOCIAL (10/10/2026) : sa definition est codifiee
            // par le decret 2023-1378 au II de R844-1 (prime d'activite), le
            // meme texte que le 9 bis de R3243-1 du code du travail. C'est
            // lui qui dit ce que S21.G00.58.004 contient (frais professionnels
            // exclus, part patronale sante/prevoyance exclue, cotisations
            // salariales de prevoyance deduites depuis 2024).
            Domaine { cle: "net_social", titre: "Le montant net social (S21.G00.58.004) : sa definition", numeros: &["R844-1"] },
            Domaine { cle: "csg", titre: "L'assiette de la CSG et de la CRDS", numeros: &["L136-1-1"] },
            Domaine { cle: "contribution_rupture", titre: "La contribution patronale sur les indemnites de rupture", numeros: &["L137-12"] },
            // L1237-5 du code du travail ne dit pas l'age de la mise a la
            // retraite : il RENVOIE a ces deux articles-la. Sans eux, la
            // condition d'age ne se verifie pas, et L1237-8 en fait un
            // licenciement.
            // L161-17-2 EST CELUI QUI PORTE LE NOMBRE, et il manquait :
            // L1237-5 du code du travail renvoie au 1er de L351-8, qui
            // renvoie lui-meme a << l'age prevu au premier alinea de
            // l'article L. 161-17-2 AUGMENTE DE TROIS ANNEES >>. Sans lui, la
            // condition d'age de la mise a la retraite ne se verifie pas, et
            // le moteur AVERTIT au lieu de verifier
            // (`permanents-base.json`, `_pourquoi_pas_verifie`).
            Domaine { cle: "age_de_la_retraite", titre: "L'age auquel la retraite s'ouvre", numeros: &["L161-17-2", "L351-1", "L351-8"] },
            // L'EFFECTIF DE L'URSSAF (23/09/2026, §80.1) : moyenne annuelle,
            // regle des cinq ans -- celui que porte le CRM 118 et qui regle
            // les seuils de cotisation (formation, FNAL, echeance de la DSN,
            // versement mobilite via L2333-64). Il ne dit rien du CSE.
            Domaine { cle: "effectif", titre: "L'effectif au sens de la securite sociale", numeros: &["L130-1"] },
            // LE PLAFOND (10/10/2026, lot V1i) : la vieillesse plafonnee
            // (L241-3, et son taux D242-4), le FNAL et le chomage d'un
            // intermittent s'assoient sur LE plafond de sa periode. R242-2 dit
            // qu'il se reduit au prorata des jours du contrat ; D242-17 a
            // D242-19 disent ses valeurs journaliere et horaire. La regle
            // propre aux artistes (douze fois le plafond horaire par jour) est
            // dans l'arrete du 24 janvier 1975, plus bas.
            Domaine { cle: "plafond", titre: "Le plafond de la securite sociale et sa reduction", numeros: &["L241-3", "D242-4", "R242-2", "D242-17", "D242-18", "D242-19"] },
            // LE PLAFOND D'UN TEMPS PARTIEL (10/10/2026, lot V4b) : L242-8 pose
            // l'abattement d'assiette des salaries a temps partiel, que le
            // dernier alinea du I de R242-2 met en oeuvre (l'employeur << est
            // en droit de corriger le plafond >>) ; L242-10, qu'il cite, en
            // exclut les emplois a taux ou assiette specifiques et le plafond
            // fractionne entre plusieurs employeurs. `droit/src/plafond.rs`.
            Domaine { cle: "plafond_temps_partiel", titre: "Le plafond d'un salarie a temps partiel (l'abattement d'assiette)", numeros: &["L242-8", "L242-10"] },
            // LA DISPENSE D'UN TEMPS PARTIEL DE LA SANTE (13/10/2026, lot D1).
            // R242-1-6, 2°, c : un salarie a temps partiel peut se dispenser
            // quand la cotisation atteindrait 10 % de sa remuneration -- ce
            // qui dit qu'un forfait de sante ne baisse pas avec le temps de
            // travail (`calcul::plafond_de_la_sante`).
            Domaine { cle: "sante_dispenses", titre: "Les dispenses d'adhesion aux garanties collectives de sante", numeros: &["R242-1-6"] },
            // LA PROTECTION SOCIALE COMPLEMENTAIRE PENDANT UNE SUSPENSION (15/10/2026 ;
            // `atelier/reference/sources/suspension-non-remuneree/legi-extrait-code-de-la-securite-sociale-20261015.md`) :
            // le caractere collectif et obligatoire qui exclut les contributions patronales de l'assiette (R242-1-1 a
            // R242-1-5), la couverture sante obligatoire des salaries (L911-1, L911-2, L911-7, L911-8, D911-1 a D911-8),
            // et l'assiette de la CRDS (L136-2). Le maintien de la sante d'un mois suspendu sans indemnisation :
            // `calcul::sante_de_la_suspension`.
            Domaine { cle: "protection_complementaire", titre: "Les garanties collectives de protection sociale complementaire", numeros: &["R242-1-1", "R242-1-2", "R242-1-3", "R242-1-4", "R242-1-5", "L911-1", "L911-2", "L911-7", "L911-8", "D911-1", "D911-2", "D911-3", "D911-4", "D911-5", "D911-6", "D911-7", "D911-8", "L136-2"] },
            // LES MAJORATIONS DES EMPLOYEURS AFFILIES A UNE CAISSE DE CONGES
            // (10/10/2026, lot V1n) : le FNAL, le versement mobilite et les
            // contributions de formation se paient par une majoration de
            // 11,5 % (10 % pour la taxe d'apprentissage d'un intermittent du
            // spectacle) ; `majoration_conges_115` des baremes.
            Domaine { cle: "majoration_caisses", titre: "Les majorations des employeurs affilies a une caisse de conges payes", numeros: &["L243-1-3", "D243-0-2", "D243-0-3"] },
            // L'ARRONDI A L'EURO DES MONTANTS DECLARES (13/10/2026, lot U5) :
            // L130-1 ne le dit plus (le guide Urssaf de la DSN le cite encore) ;
            // il est dans L133-10, qui compte la fraction egale a 0,50 pour 1.
            Domaine { cle: "arrondi", titre: "L'arrondi a l'euro des cotisations et de leurs assiettes declarees", numeros: &["L133-10"] },
            // LES SIGNALEMENTS D'EVENEMENT ET LEUR DELAI (15/10/2026, lot << les obligations d'un arret >>). R133-13 dit
            // quels evenements la DSN declare (le debut et la fin d'un arret, la fin du contrat) ; R133-14, II, quand :
            // avec la mensuelle du mois, sauf la fin du contrat et -- quand l'employeur n'est PAS subroge -- le debut et la
            // fin de l'arret, << dans un delai fixe par arrete [...] qui ne peut exceder cinq jours ouvres >> (l'arrete du
            // 2 juillet 2013, plus bas) ; ses exceptions : aucune retenue sur le salaire du mois (1°), les fins de CDD
            // d'usage du 3° de L1242-2 (2°). `paie/src/obligations.rs`.
            Domaine { cle: "signalements", titre: "Les signalements d'evenement de la DSN et leur delai", numeros: &["R133-13", "R133-14"] },
            // LA DECLARATION D'ACCIDENT DU TRAVAIL (15/10/2026). Hors DSN : L441-2 la met a la charge de l'employeur << de
            // tout accident dont il a eu connaissance >>, R441-3 en fixe le delai (quarante-huit heures, non compris les
            // dimanches et jours feries), R441-2 celui de la victime envers l'employeur (vingt-quatre heures), R441-6 les
            // reserves motivees (dix jours francs). L411-1 et L411-2 disent que l'accident de trajet en est un ; L441-5 la
            // feuille d'accident ; R441-4 l'attestation de salaire que la DSN remplit (R133-14, IV). `paie/src/obligations.rs`.
            Domaine { cle: "accident_du_travail", titre: "La declaration d'un accident du travail ou de trajet", numeros: &["L411-1", "L411-2", "L441-1", "L441-2", "L441-4", "L441-5", "R441-1", "R441-2", "R441-3", "R441-4", "R441-6", "R441-7"] },
            // LA MALADIE PROFESSIONNELLE N'EST PAS DECLAREE PAR L'EMPLOYEUR : L461-5 la met << par les soins de la
            // victime >>, R461-5 lui donne quinze jours. Le motif d'arret 05 n'appelle donc pas de declaration d'accident.
            Domaine { cle: "maladie_professionnelle", titre: "La declaration d'une maladie professionnelle (par la victime)", numeros: &["L461-5", "R461-5"] },
            // LE CONGE DE DEUIL ET SES INDEMNITES JOURNALIERES (15/10/2026, lot << le deuil >>). L331-9 : pendant le conge
            // de L3142-1-1 du code du travail, l'assure percoit l'indemnite journaliere de L331-3 ; l'employeur qui maintient
            // le salaire (L3142-2) est subroge de plein droit. D331-7 : l'indemnisation se fractionne comme le conge.
            // R323-10 : l'attestation de salaire que l'employeur adresse a la caisse pour les indemnites journalieres, et que
            // la DSN remplit d'ordinaire (R133-14, IV) -- pas pour le deuil, que la DSN ne couvre pas (la page de
            // net-entreprises, `accident-du-travail/`). `paie/src/obligations.rs`, l'obligation `attestation_salaire`.
            Domaine { cle: "conge_de_deuil", titre: "Le conge de deuil d'un enfant : ses indemnites journalieres et l'attestation de salaire", numeros: &["L331-9", "D331-7", "R323-10"] },
        ],
    },
    // ------------------------------------------------------------------
    // L'IDENTIFIANT EST VERIFIE, PAS SUPPOSE (21/09/2026) : une sonde a
    // fait defiler l'archive globale -- 1 750 418 articles -- et a
    // releve sous quel LEGITEXT vivent ces numeros-la. C'est la
    // precaution du piege 3 de l'en-tete, appliquee AVANT de moissonner
    // plutot qu'apres : un LEGITEXT faux ne donne pas une erreur, il
    // donne un relevé vide qu'on lirait comme << ces articles n'existent
    // pas >>.
    // POURQUOI CE CODE-LA. Le versement mobilite est du par les
    // employeurs de ONZE salaries et plus etablis dans le ressort d'une
    // autorite organisatrice qui l'a institue, et son taux est fixe
    // LOCALEMENT -- il n'a pas de valeur de droit commun. Depuis le
    // 21/09/2026 le moteur REFUSE de payer un effectif de onze sans une
    // attestation, et ce refus renvoie a deux articles cites de memoire,
    // jamais relus. On les releve avant d'ecrire la moindre rubrique.
    Texte {
        cle: "collectivites",
        legitext: "LEGITEXT000006070633",
        nom: "Code general des collectivites territoriales",
        dossier: "code-general-des-collectivites-territoriales",
        titre: "Code general des collectivites territoriales -- le versement mobilite",
        domaines: &[
            Domaine { cle: "hors_idf", titre: "Le versement mobilite hors Ile-de-France", numeros: &["L2333-64", "L2333-66", "L2333-67", "L2333-68", "L2333-70", "L2333-71", "L2333-72", "L2333-73", "L2333-74", "L2333-75"] },
            Domaine { cle: "idf", titre: "Le versement mobilite en Ile-de-France", numeros: &["L2531-2", "L2531-3", "L2531-4", "L2531-6"] },
        ],
    },
    // ------------------------------------------------------------------
    // UN ARRETE, PAS UN CODE (10/10/2026, lot V1i). La base LEGI range les
    // textes non codifies sous leur JORFTEXT, dans le chemin du fichier
    // comme le LEGITEXT d'un code (`TNC_en_vigueur/JORF/TEXT/.../
    // JORFTEXT000000310494/article/...`) : le meme filtre les attrape.
    // C'est lui qui fixe le plafond d'un artiste engage moins de cinq
    // jours continus : douze fois le plafond horaire par journee de
    // travail (`droit/src/plafond.rs`).
    Texte {
        cle: "artistes-1975",
        legitext: "JORFTEXT000000310494",
        nom: "Arrete du 24 janvier 1975 fixant le taux des cotisations de securite sociale et d'allocations familiales dues au titre de l'emploi des artistes du spectacle",
        dossier: "arrete-1975-artistes-du-spectacle",
        titre: "Arrete du 24 janvier 1975 (artistes du spectacle) -- le plafond",
        domaines: &[
            Domaine { cle: "artistes", titre: "Les cotisations et le plafond des artistes du spectacle", numeros: &["1", "2", "3", "3 bis"] },
        ],
    },
    // ------------------------------------------------------------------
    // LE DELAI DES SIGNALEMENTS D'EVENEMENT (15/10/2026). R133-14, II, du code de la securite sociale renvoie le delai de
    // la fin du contrat et de l'arret non subroge a << un arrete conjoint du ministre charge de la securite sociale et du
    // ministre charge de l'emploi, qui ne peut exceder cinq jours ouvres >>. C'est celui-ci (Legifrance, JORFTEXT000027683422) :
    // son article 1er fixe ce delai a cinq jours ouvres. Un arrete, pas un code, range sous son JORFTEXT comme celui de 1975.
    // `paie/src/obligations.rs`, et `logiciel/donnees/obligations-base.json`.
    Texte {
        cle: "evenements-dsn-2013",
        legitext: "JORFTEXT000027683422",
        nom: "Arrete du 2 juillet 2013 fixant le delai de transmission des evenements dans le cadre de la declaration sociale nominative",
        dossier: "arrete-2013-delai-evenements-dsn",
        titre: "Arrete du 2 juillet 2013 (DSN) -- le delai des signalements d'evenement",
        domaines: &[
            Domaine { cle: "delai", titre: "Le delai de transmission des evenements", numeros: &["1"] },
        ],
    },
    // ------------------------------------------------------------------
    // LA DEDUCTION FORFAITAIRE SPECIFIQUE (11/10/2026, lot R9). Un arrete,
    // pas un code, range sous son JORFTEXT comme celui de 1975 ; trouve
    // par une sonde de l'archive globale et des livraisons (le titre
    // << frais professionnels deductibles >>), pas suppose. Il a ABROGE et
    // remplace l'arrete du 20/12/2002 le 07/09/2025 (livraisons LEGI des
    // 08 et 09/09/2025). Son article 9 dit qui peut exercer la DFS (les
    // professions de l'article 5 de l'annexe IV du CGI dans sa redaction
    // du 31/12/2000), a quelle condition (accord collectif, CE, CSE ou DP,
    // a defaut chaque salarie), dans quelle limite (7 600 EUR par annee
    // civile), a quels taux d'annee en annee (II : le spectacle vivant
    // jusqu'a zero en 2032), ce que l'assiette garde (les remboursements
    // de frais, sauf l'annexe 2) et ce qu'elle ne peut pas entamer
    // (R242-1, al. 6). `paie/src/calcul.rs` (`deduction_forfaitaire`,
    // `abattement_dfs`) ; les taux, `deduction_forfaitaire` des baremes.
    // Les deux annexes (les professions et leurs taux ; les frais qui
    // restent hors assiette) n'ont pas de <NUM> dans la base : ce
    // moissonneur, qui range par numero, ne les garde pas. Elles sont
    // deposees brutes a cote (atelier/reference/sources/dfs/LISEZ-MOI.md).
    Texte {
        cle: "frais-professionnels-2025",
        legitext: "JORFTEXT000052198430",
        nom: "Arrete du 4 septembre 2025 relatif aux frais professionnels deductibles pour le calcul des cotisations de securite sociale",
        dossier: "dfs/arrete-2025-frais-professionnels",
        titre: "Arrete du 4 septembre 2025 (frais professionnels) -- la deduction forfaitaire specifique",
        domaines: &[
            Domaine { cle: "frais", titre: "Les frais professionnels et la deduction forfaitaire specifique", numeros: &["1", "2", "9"] },
        ],
    },
    // ------------------------------------------------------------------
    // LA REDUCTION GENERALE SOUS DFS (lot R9). CSS L241-13, III : elle << ne
    // tient compte des deductions au titre de frais professionnels
    // calculees forfaitairement [...] que dans des limites et conditions
    // fixees par arrete >>. C'est celui-ci (article 1er dans la redaction
    // de l'arrete du 4 septembre 2025, art. 11) : la reduction calculee
    // apres l'abattement ne peut exceder 130 % de celle calculee sans.
    // `paie/src/calcul.rs` (`reduction_generale_sous_dfs`).
    Texte {
        cle: "reduction-generale-dfs-2019",
        legitext: "JORFTEXT000039626366",
        nom: "Arrete du 4 decembre 2019 relatif au benefice de la reduction prevue a l'article L. 241-13 du code de la securite sociale pour les employeurs appliquant la deduction forfaitaire specifique",
        dossier: "dfs/arrete-2019-reduction-generale",
        titre: "Arrete du 4 decembre 2019 -- la reduction generale sous deduction forfaitaire specifique",
        domaines: &[
            Domaine { cle: "reduction", titre: "La reduction generale apres l'abattement", numeros: &["1", "2"] },
        ],
    },
    // ------------------------------------------------------------------
    // LA PARTICIPATION DES EMPLOYEURS A L'EFFORT DE CONSTRUCTION
    // (10/10/2026, lot V3b). La DSN de decembre declare chaque annee si
    // l'employeur y est assujetti (bloc S21.G00.44, codes 005 et 006) :
    // c'est L313-1 de ce code qui dit qui la doit (cinquante salaries,
    // hors Etat et collectivites), et comment se compte l'effectif.
    Texte {
        cle: "construction",
        legitext: "LEGITEXT000006074096",
        nom: "Code de la construction et de l'habitation",
        dossier: "code-de-la-construction-et-de-l-habitation",
        titre: "Code de la construction et de l'habitation -- la PEEC",
        domaines: &[
            Domaine { cle: "peec", titre: "La participation des employeurs a l'effort de construction", numeros: &["L313-1"] },
        ],
    },
    // ------------------------------------------------------------------
    // UNE LOI, PAS UN CODE (10/10/2026, lot V3b) : son article 135 a
    // reecrit L6241-1 du code du travail (version du 21/02/2026) et rend
    // redevables de la taxe d'apprentissage les associations a activites
    // non lucratives. Ce qu'on y cherche : a partir de quand.
    Texte {
        cle: "loi-de-finances-2026",
        legitext: "JORFTEXT000053508155",
        nom: "Loi n° 2026-103 du 19 fevrier 2026 de finances pour 2026",
        dossier: "loi-de-finances-2026",
        titre: "Loi de finances pour 2026 -- la taxe d'apprentissage des associations",
        domaines: &[
            Domaine { cle: "taxe_apprentissage", titre: "La taxe d'apprentissage des associations a activites non lucratives", numeros: &["135"] },
        ],
    },
    // ------------------------------------------------------------------
    // L242-1 du code de la securite sociale RENVOIE a cet article-la pour
    // dire ce qui sort de l'assiette : le lire sans lui, c'est lire une
    // moitie de phrase.
    // 235 bis : la cotisation de 2 % de l'employeur qui n'a pas investi
    // sa participation a l'effort de construction (norme DSN, p. 58 ; bloc
    // S21.G00.44, lot V3b).
    Texte {
        cle: "impots",
        legitext: "LEGITEXT000006069577",
        nom: "Code general des impots",
        dossier: "code-general-des-impots",
        titre: "Code general des impots -- ce que la paie lit",
        domaines: &[
            Domaine { cle: "rupture", titre: "Le regime fiscal des indemnites de rupture", numeros: &["80 duodecies"] },
            Domaine { cle: "peec", titre: "La participation des employeurs a l'effort de construction (la cotisation de 2 %)", numeros: &["235 bis"] },
            // LA TAXE SUR LES SALAIRES (journal §117) : l'employeur qui
            // n'est pas assujetti a la TVA -- beaucoup d'associations
            // du spectacle. Le bareme (231) et l'abattement des
            // associations (1679 A) ; 1679 dit quand elle se verse. 293 B (ticket #239, 21/10/2026) : les plafonds de
            // la franchise en base de TVA, auxquels renvoie le 2e alinea du 1 de 231 -- sous eux, pas de taxe ; leurs
            // montants entrent au bareme de l'annee (`taxe_sur_les_salaires.plafonds_franchise`).
            Domaine { cle: "taxe_salaires", titre: "La taxe sur les salaires", numeros: &["231", "293 B", "1679", "1679 A"] },
            // L'ARRONDI A L'EURO DES SOMMES DUES A LA DGFIP (13/10/2026,
            // lot U5) : le versement du prelevement a la source
            // (S21.G00.20.005 de la DGFiP, << arrondi a l'entier le plus
            // proche >>) est une somme a recevoir de l'administration.
            Domaine { cle: "arrondi", titre: "L'arrondi a l'euro des sommes a recevoir", numeros: &["1724"] },
            // LE PRELEVEMENT A LA SOURCE D'UN MOIS (13/10/2026, lot D1) :
            // 1671, 1 et 2 -- la retenue est « effectuee par le debiteur
            // lors du paiement », au taux transmis « au plus tard le
            // deuxieme mois suivant sa transmission » ; ce qui a ete
            // retenu ne se recalcule pas (`prelevement::des_paies`).
            Domaine { cle: "pas", titre: "La retenue a la source : quand, et a quel taux", numeros: &["1671"] },
            // L'ASSIETTE DU PRELEVEMENT A LA SOURCE ET LA SANTE PATRONALE REINTEGREE (15/10/2026 ;
            // `atelier/reference/sources/suspension-non-remuneree/legi-extrait-code-general-des-impots-20261015.md`) : 83,
            // 1° quater (la part patronale « frais de sante » n'est pas deductible : elle se reintegre au net fiscal), et
            // 204 A a 204 H (la retenue a la source, son assiette « montant net imposable », son taux). Une remuneration
            // nette fiscale negative : 50.013 a 0.00 (`pas::du_versement`, fiche DSN 1810).
            Domaine { cle: "assiette_pas", titre: "L'assiette de la retenue a la source, et la sante patronale reintegree", numeros: &["83", "204 A", "204 B", "204 E", "204 F", "204 H"] },
        ],
    },
];

//! LA FICHE D'UNE ROUTE ET LE CATALOGUE (`moteur routes`). Chaque route porte sa fiche dans la liste `ROUTES` de son
//! module : le geste qu'elle sert, sa charge et un exemple, ce qu'elle rend, ses refus et quoi corriger, si elle lit,
//! ecrit ou engage, si elle est « avancee », et la place des phrases qu'elle rend. La page et le LLM passent par les memes
//! routes ; le catalogue est leur documentation, tiree du code : celles d'une compagnie et celles de l'installation
//! (`accueil`). Les routes de l'atelier (l'horloge du banc, la route d'essai qui tombe) n'y sont pas.

use serde_json::{json, Value};

pub use crate::routes::affiche::Affiche;

/// Ce que fait la route.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Acte {
    /// Elle lit : rien ne s'ecrit.
    Lit,
    /// Elle ecrit dans la compagnie ; rien ne part.
    Ecrit,
    /// Un acte reel (un envoi, un depot, une signature) : le mot de passe du logiciel, tape par un humain.
    Engage,
}

impl Acte {
    pub fn mot(self) -> &'static str {
        match self {
            Acte::Lit => "lit",
            Acte::Ecrit => "écrit",
            Acte::Engage => "engage (le mot de passe du logiciel, tapé par un humain)",
        }
    }
}

/// LES PLACES DE L'ECRAN, telles que la page les nomme : l'onglet, et le sous-onglet de la Configuration. Une place se
/// nomme d'une seule facon, ici ; une fiche ne peut nommer qu'une de ces places.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Place {
    Paie,
    Fiches,
    Metiers,
    Organismes,
    Bulletin,
    ModelesContrat,
    Identite,
    Coffre,
    Import,
    Historique,
    Exports,
    Signaler,
    Bandeau,
    Accueil,
}

impl Place {
    /// Le nom que l'ecran montre (la page le porte en `data-place`).
    pub fn nom(self) -> &'static str {
        match self {
            Place::Paie => "Paie",
            Place::Fiches => "Configuration › Entreprise, Salariés, Spectacles",
            Place::Metiers => "Configuration › Métiers",
            Place::Organismes => "Configuration › Organismes et contrats",
            Place::Bulletin => "Configuration › Modèle de bulletin",
            Place::ModelesContrat => "Configuration › Modèles de contrat",
            Place::Identite => "Configuration › Signature & tampon",
            Place::Coffre => "Configuration › Mots de passe",
            Place::Import => "Configuration › Reprendre l'historique",
            Place::Historique => "Historique",
            Place::Exports => "Exports",
            Place::Signaler => "Un problème, une idée ?",
            Place::Bandeau => "Le bandeau",
            Place::Accueil => "L'accueil des compagnies",
        }
    }
}

/// OU LA ROUTE SE VOIT A L'ECRAN.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Ecran {
    /// La page de cette place l'appelle (son ecran est dessine).
    Dessine(Place),
    /// Sans ecran dessine : sous « Avancé » de sa place, la page l'affiche depuis sa fiche (`js/avance.js`), avec ce
    /// bouton (le `geste` est pour le LLM).
    Auto(Place, Affiche),
    /// Une lecture qui nourrit les ecrans (l'etat, la configuration, le mois) : elle n'a pas de place.
    Lecture,
}

impl Ecran {
    pub fn place(self) -> Option<Place> {
        match self {
            Ecran::Dessine(p) | Ecran::Auto(p, _) => Some(p),
            Ecran::Lecture => None,
        }
    }

    fn mot(self) -> &'static str {
        match self {
            Ecran::Dessine(_) => "dessine",
            Ecran::Auto(..) => "auto",
            Ecran::Lecture => "lecture",
        }
    }

    /// Ce que `moteur routes` en dit.
    pub fn texte(self) -> String {
        match self {
            Ecran::Dessine(p) => p.nom().to_string(),
            Ecran::Auto(p, _) => format!("{} › Avancé", p.nom()),
            Ecran::Lecture => "(une lecture des écrans)".to_string(),
        }
    }
}

/// LA FORME D'UNE VALEUR DE LA CHARGE : ce que l'ecran saisit (une case, un nombre, un mois…) et ce que le LLM ecrit.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Forme {
    Texte,
    OuiNon,
    Nombre,
    /// AAAA-MM.
    Mois,
    /// AAAA-MM-JJ.
    Date,
    /// Une liste de textes.
    Liste,
    /// Une valeur composee (un objet, une liste d'objets) : le `sens` la decrit ; l'ecran ne la saisit pas d'une case.
    Objet,
}

impl Forme {
    pub fn mot(self) -> &'static str {
        match self {
            Forme::Texte => "texte",
            Forme::OuiNon => "oui_non",
            Forme::Nombre => "nombre",
            Forme::Mois => "mois",
            Forme::Date => "date",
            Forme::Liste => "liste",
            Forme::Objet => "objet",
        }
    }
}

/// Un champ de la charge : son nom, ce que l'ecran en montre (`libelle`) et sa forme, son sens pour le LLM, et s'il est
/// « avance » (l'ecran le range sous Avancé, ou seul un fait l'appelle).
#[derive(Clone, Copy, Debug)]
pub struct Champ {
    pub nom: &'static str,
    pub libelle: Affiche,
    pub forme: Forme,
    pub sens: &'static str,
    pub avance: bool,
}

impl Champ {
    pub fn en_json(&self) -> Value {
        json!({
            "nom": self.nom,
            "libelle": self.libelle.texte(),
            "forme": self.forme.mot(),
            "sens": self.sens,
            "avance": self.avance,
        })
    }
}

/// LES CASES DE LA CARTE D'UN CONTRAT PAYE AU MOIS, dans la charge `saisie` (les faits du mois et ses termes). La marque
/// « avancé » range l'ecran et le catalogue a la fois : la page met sous « Avancé » de la carte les cases marquees, et
/// seulement celles que l'engagement appelle (`faits_avances` de la route apercu, `engagement::heures_en_plus::faits_appeles`).
pub const CASES_D_UN_MOIS_PAYE: &[Champ] = &[
    Champ { nom: "salaire_mensuel", libelle: Affiche::new("Salaire mensuel brut"), forme: Forme::Nombre, sens: "le salaire mensuel brut du contrat", avance: false },
    Champ { nom: "quotite_contrat", libelle: Affiche::new("Heures par mois"), forme: Forme::Nombre, sens: "ses heures par mois", avance: false },
    Champ { nom: "conges_pris", libelle: Affiche::new("Congés pris"), forme: Forme::Objet, sens: "les congés pris du mois : [{debut, fin}]", avance: false },
    Champ { nom: "primes", libelle: Affiche::new("Primes"), forme: Forme::Objet, sens: "les primes du mois : [{montant, rubrique, libelle}]", avance: false },
    Champ {
        nom: "lignes", libelle: Affiche::new("Repas et nuitées"), forme: Forme::Objet,
        sens: "ses repas et nuitées du mois, comme les lignes d'un intermittent : [{rubrique « repas » ou « nuitee », quantite, taux, dates}], aux mêmes limites",
        avance: false,
    },
    Champ { nom: "heures_supplementaires", libelle: Affiche::new("Heures supplémentaires"), forme: Forme::Nombre, sens: "un temps plein : ses heures supplémentaires du mois, que le moteur majore", avance: true },
    Champ {
        nom: "heures_supplementaires_a_50", libelle: Affiche::new("Dont à 50 %"), forme: Forme::Nombre,
        sens: "dont celles au-delà de la huitième d'une semaine (majorées de 50 % quand la convention ne fixe pas les paliers)",
        avance: true,
    },
    Champ { nom: "heures_complementaires", libelle: Affiche::new("Heures complémentaires"), forme: Forme::Nombre, sens: "un temps partiel : ses heures complémentaires du mois, que le moteur majore", avance: true },
];

/// LES CASES DE LA FIN DU CONTRAT, dans la charge `saisie` : les faits du mois ou il finit (`engagement::stock::DE_LA_FIN`),
/// la ou le moteur les ecrit quel que soit le mois de la carte. Ce que chaque motif appelle : `faits_de_rupture` des
/// `natures` de la route etat.
pub const CASES_DE_LA_FIN: &[Champ] = &[
    Champ { nom: "fin", libelle: Affiche::new("Dernier jour du contrat"), forme: Forme::Date, sens: "le dernier jour du contrat, AAAA-MM-JJ", avance: false },
    Champ { nom: "motif_rupture", libelle: Affiche::new("Motif de la fin"), forme: Forme::Texte, sens: "le code du motif, \"059\" pour une démission : `motifs_rupture` des `natures` de la route etat", avance: false },
    Champ { nom: "notification_rupture", libelle: Affiche::new("Notifiée le"), forme: Forme::Date, sens: "le jour où la rupture a été notifiée, AAAA-MM-JJ", avance: false },
    Champ { nom: "preavis", libelle: Affiche::new("Préavis"), forme: Forme::Objet, sens: "{type, debut, fin} : type de `types_preavis` de la route etat (\"90\" sans préavis), ses dates s'il en a", avance: false },
    Champ { nom: "dernier_jour_paye", libelle: Affiche::new("Dernier jour payé"), forme: Forme::Date, sens: "le dernier jour payé, s'il n'est pas la fin", avance: false },
];

/// Les cases d'une carte que porte la charge de la fiche (`saisie`), s'il y en a.
fn cases(charge: &[Champ]) -> &'static [Champ] {
    if charge.iter().any(|c| c.nom == "saisie") { CASES_D_UN_MOIS_PAYE } else { &[] }
}

/// Les cases de la fin du contrat que porte la charge de la fiche (`saisie`), s'il y en a.
fn cases_de_la_fin(charge: &[Champ]) -> &'static [Champ] {
    if charge.iter().any(|c| c.nom == "saisie") { CASES_DE_LA_FIN } else { &[] }
}

/// Un refus : sa classe, et ce qu'il faut corriger.
#[derive(Clone, Copy, Debug)]
pub struct Refus {
    pub classe: &'static str,
    pub corriger: &'static str,
}

/// LA FICHE D'UNE ROUTE.
#[derive(Clone, Copy, Debug)]
pub struct Fiche {
    pub nom: &'static str,
    /// Le geste, en une phrase breve.
    pub geste: &'static str,
    pub charge: &'static [Champ],
    /// Une charge que la route accepte sur une compagnie de test (`tests/fichiers/routes.rs`), en JSON.
    pub exemple: &'static str,
    pub rend: &'static str,
    pub refus: &'static [Refus],
    pub acte: Acte,
    pub avance: bool,
    /// La place, en caracteres, de la phrase que la route rend pour l'affichage (`phrase`) ; 0 : elle n'en rend pas.
    pub phrase: usize,
    /// Ou la route se voit a l'ecran.
    pub ecran: Ecran,
}

/// Ce que bien des fiches partagent : le mois de l'ecran, et les refus de la forme de la charge.
pub const MOIS: Champ = Champ { nom: "mois", libelle: Affiche::new("Mois"), forme: Forme::Mois, sens: "le mois de l'écran, AAAA-MM", avance: false };
pub const SALARIE: Champ = Champ { nom: "salarie", libelle: Affiche::new("Salarié"), forme: Forme::Texte, sens: "la clé de la fiche du salarié (ou son nom)", avance: false };
pub const MANQUE: Refus = Refus { classe: "KeyError", corriger: "le champ de la charge, ou la fiche, que la phrase nomme" };
pub const ILLISIBLE: Refus = Refus { classe: "ValueError", corriger: "la date ou la valeur que la phrase dit illisible" };
pub const FORME: Refus = Refus { classe: "TypeError", corriger: "le champ de la charge que la phrase nomme" };
pub const OPERATION: Refus = Refus { classe: "ErreurOperation", corriger: "ce que la phrase dit ; rien n'est écrit" };

impl Fiche {
    pub fn en_json(&self) -> Value {
        json!({
            "nom": self.nom,
            "geste": self.geste,
            "charge": self.charge.iter().map(|c| c.en_json()).collect::<Vec<_>>(),
            "cases": cases(self.charge).iter().map(|c| c.en_json()).collect::<Vec<_>>(),
            "cases_de_la_fin": cases_de_la_fin(self.charge).iter().map(|c| c.en_json()).collect::<Vec<_>>(),
            "exemple": serde_json::from_str::<Value>(self.exemple).unwrap_or(Value::Null),
            "rend": self.rend,
            "refus": self.refus.iter().map(|r| json!({"classe": r.classe, "corriger": r.corriger})).collect::<Vec<_>>(),
            "acte": match self.acte { Acte::Lit => "lit", Acte::Ecrit => "ecrit", Acte::Engage => "engage" },
            "avance": self.avance,
            "phrase": self.phrase,
            "ecran": self.ecran.place().map(Place::nom),
            "affichage": self.ecran.mot(),
            "bouton": match self.ecran { Ecran::Auto(_, b) => Value::from(b.texte()), _ => Value::Null },
        })
    }

    pub fn en_texte(&self) -> String {
        let mut t = format!("{}{} : {}\n  Acte : {}\n  Charge :\n", self.nom, if self.avance { " (avancé)" } else { "" }, self.geste, self.acte.mot());
        for c in self.charge {
            t.push_str(&format!("    {}{} : {}\n", c.nom, if c.avance { " (avancé)" } else { "" }, c.sens));
        }
        if !cases(self.charge).is_empty() {
            t.push_str("  Cases de la carte d'un contrat au mois (dans saisie) :\n");
            for c in cases(self.charge) {
                t.push_str(&format!("    {}{} : {}\n", c.nom, if c.avance { " (avancé)" } else { "" }, c.sens));
            }
        }
        if !cases_de_la_fin(self.charge).is_empty() {
            t.push_str("  Cases de la fin du contrat (dans saisie, sur la carte de n'importe quel mois du contrat) :\n");
            for c in cases_de_la_fin(self.charge) {
                t.push_str(&format!("    {} : {}\n", c.nom, c.sens));
            }
        }
        t.push_str(&format!("  Exemple : echo '{{\"route\":\"{}\",\"charge\":{}}}' | moteur route\n", self.nom, self.exemple));
        t.push_str(&format!("  Rend : {}\n", self.rend));
        t.push_str(&format!("  Écran : {}\n", self.ecran.texte()));
        if self.phrase > 0 {
            t.push_str(&format!("  Phrase : au plus {} caractères\n", self.phrase));
        }
        t.push_str("  Refus :\n");
        for r in self.refus {
            t.push_str(&format!("    {} : {}\n", r.classe, r.corriger));
        }
        t
    }
}

/// La fiche de `nom` dans `fiches`, si elle y est.
pub fn dans(fiches: &[Fiche], nom: &str) -> Option<Fiche> {
    fiches.iter().find(|f| f.nom == nom).copied()
}

/// La route du catalogue, pour la page (l'acte d'une route, ce qui est avance) : avec `avance`, ce que
/// `moteur routes --json` imprime.
pub const ROUTES: &[Fiche] = &[Fiche {
    nom: "routes",
    geste: "Dire les routes : leur fiche, le chemin simple d'abord.",
    charge: &[Champ { nom: "avance", libelle: Affiche::new("Toutes les routes"), forme: Forme::OuiNon, sens: "vrai : toutes les routes, les avancées comprises", avance: true }],
    exemple: r#"{"avance": true}"#,
    rend: "routes : la fiche de chaque route (nom, geste, charge, exemple, rend, refus, acte, avance, phrase)",
    refus: &[FORME],
    acte: Acte::Lit,
    avance: true,
    phrase: 0,
    ecran: Ecran::Lecture,
}];

/// Les fiches du catalogue en JSON : le chemin simple, ou toutes avec `avance`.
pub fn en_json(avance: bool) -> Value {
    json!({"routes": catalogue().iter().filter(|f| avance || !f.avance).map(Fiche::en_json).collect::<Vec<_>>()})
}

/// Les modules dont les routes portent leur fiche, dans l'ordre du catalogue : l'ordre ou leurs exemples se jouent.
pub const FICHES: [&[Fiche]; 28] = [
    crate::routes::engagement::ROUTES,
    crate::etat_garde::ROUTES,
    crate::routes::mois::ROUTES,
    crate::routes::etapes::ROUTES,
    crate::routes::imprevus::ROUTES,
    crate::routes::dossier_salarie::ROUTES,
    crate::routes::contrats::ROUTES,
    crate::routes::modeles_contrat::ROUTES,
    crate::routes::depots::ROUTES,
    crate::routes::documents::ROUTES,
    crate::routes::coffre::ROUTES,
    crate::routes::import::ROUTES,
    crate::routes::archives::ROUTES,
    crate::routes::historique::ROUTES,
    crate::routes::fiches::ROUTES,
    crate::routes::identite::ROUTES,
    crate::routes::metiers::ROUTES,
    crate::routes::bulletin::ROUTES,
    crate::compagnie::organismes::ROUTES,
    crate::routes::filet::ROUTES,
    crate::historique_mois::ROUTES,
    crate::routes::ouverture::ROUTES,
    crate::routes::rappels::ROUTES,
    crate::compagnie::conseils::ROUTES,
    crate::compagnie::ouverture::ROUTES,
    crate::obligations::ROUTES,
    crate::routes::accueil::ROUTES,
    ROUTES,
];

/// LE CATALOGUE : toutes les routes, d'une compagnie et de l'installation, chacune une fois, dans l'ordre des modules.
pub fn catalogue() -> Vec<Fiche> {
    FICHES.iter().flat_map(|l| l.iter().copied()).collect()
}

/// `moteur routes [NOM] [--avance] [--json]` : le catalogue en texte (le chemin simple ; toutes avec `--avance`), en JSON
/// (toutes : chaque fiche porte `avance`), ou la fiche entiere d'une route.
pub fn commande(args: &[String]) -> i32 {
    // Une option que `routes` ne prend pas se refuse, comme a `moteur route` (code 2) : sa valeur se lirait comme un nom.
    if let Some(o) = args.iter().find(|a| a.starts_with("--") && !["--json", "--avance"].contains(&a.as_str())) {
        eprintln!("option inconnue : {} (moteur routes [NOM] [--avance] [--json] ; --racine et --date sont de moteur route)", o);
        return 2;
    }
    let json = args.iter().any(|a| a == "--json");
    let avance = args.iter().any(|a| a == "--avance");
    let nom = args.iter().find(|a| !a.starts_with("--"));
    let tout = catalogue();
    if let Some(nom) = nom {
        let Some(f) = dans(&tout, nom) else {
            eprintln!("route inconnue : {} (moteur routes --avance les dit toutes)", nom);
            return 2;
        };
        if json {
            dire!("{}", f.en_json());
        } else {
            dire_sans_ligne!("{}", f.en_texte());
        }
        return 0;
    }
    if json {
        dire!("{}", en_json(true));
        return 0;
    }
    let montrees: Vec<&Fiche> = tout.iter().filter(|f| avance || !f.avance).collect();
    dire!("Les routes (echo '{{\"route\":NOM,\"charge\":{{…}}}}' | moteur route ; la fiche : moteur routes NOM ; toutes : --avance)\n");
    let large = montrees.iter().map(|f| f.nom.len()).max().unwrap_or(0);
    for f in &montrees {
        dire!("  {:<large$}  {:<5}  {}  [{}]", f.nom, match f.acte { Acte::Lit => "lit", Acte::Ecrit => "écrit", Acte::Engage => "engage" }, f.geste, f.ecran.texte());
    }
    let cachees = tout.len() - montrees.len();
    if cachees > 0 {
        dire!("\n{} routes avancées ne sont pas montrées : moteur routes --avance", cachees);
    }
    0
}

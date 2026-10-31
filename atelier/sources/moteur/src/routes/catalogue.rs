//! LA FICHE D'UNE ROUTE ET LE CATALOGUE (`moteur routes`). Chaque route porte sa fiche dans la liste `ROUTES` de son
//! module : le geste qu'elle sert, sa charge et un exemple, ce qu'elle rend, ses refus et quoi corriger, si elle lit,
//! ecrit ou engage, si elle est « avancee », et la place des phrases qu'elle rend. La page et le LLM passent par les memes
//! routes ; le catalogue est leur documentation, tiree du code. Les routes de l'atelier (l'horloge du banc, la fenetre
//! de l'administrateur, la route d'essai qui tombe) n'y sont pas.

use serde_json::{json, Value};

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

/// Un champ de la charge : son nom, son sens, et s'il est « avance » (l'ecran le range sous Avancé, ou seul un fait
/// l'appelle).
#[derive(Clone, Copy, Debug)]
pub struct Champ {
    pub nom: &'static str,
    pub sens: &'static str,
    pub avance: bool,
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
    /// Le geste, en une phrase breve (au plus `GESTE_MAX` caracteres).
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
}

/// La longueur d'un geste.
pub const GESTE_MAX: usize = 80;

/// Ce que bien des fiches partagent : le mois de l'ecran, et les refus de la forme de la charge.
pub const MOIS: Champ = Champ { nom: "mois", sens: "le mois de l'écran, AAAA-MM", avance: false };
pub const SALARIE: Champ = Champ { nom: "salarie", sens: "la clé de la fiche du salarié (ou son nom)", avance: false };
pub const MANQUE: Refus = Refus { classe: "KeyError", corriger: "le champ de la charge, ou la fiche, que la phrase nomme" };
pub const ILLISIBLE: Refus = Refus { classe: "ValueError", corriger: "la date ou la valeur que la phrase dit illisible" };
pub const FORME: Refus = Refus { classe: "TypeError", corriger: "le champ de la charge que la phrase nomme" };
pub const OPERATION: Refus = Refus { classe: "ErreurOperation", corriger: "ce que la phrase dit ; rien n'est écrit" };

impl Fiche {
    pub fn en_json(&self) -> Value {
        json!({
            "nom": self.nom,
            "geste": self.geste,
            "charge": self.charge.iter().map(|c| json!({"nom": c.nom, "sens": c.sens, "avance": c.avance})).collect::<Vec<_>>(),
            "exemple": serde_json::from_str::<Value>(self.exemple).unwrap_or(Value::Null),
            "rend": self.rend,
            "refus": self.refus.iter().map(|r| json!({"classe": r.classe, "corriger": r.corriger})).collect::<Vec<_>>(),
            "acte": match self.acte { Acte::Lit => "lit", Acte::Ecrit => "ecrit", Acte::Engage => "engage" },
            "avance": self.avance,
            "phrase": self.phrase,
        })
    }

    pub fn en_texte(&self) -> String {
        let mut t = format!("{}{} : {}\n  Acte : {}\n  Charge :\n", self.nom, if self.avance { " (avancé)" } else { "" }, self.geste, self.acte.mot());
        for c in self.charge {
            t.push_str(&format!("    {}{} : {}\n", c.nom, if c.avance { " (avancé)" } else { "" }, c.sens));
        }
        t.push_str(&format!("  Exemple : echo '{{\"route\":\"{}\",\"charge\":{}}}' | moteur route\n", self.nom, self.exemple));
        t.push_str(&format!("  Rend : {}\n", self.rend));
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

/// Les modules dont les routes portent leur fiche, dans l'ordre du catalogue : l'ordre ou leurs exemples se jouent.
pub const FICHES: [&[Fiche]; 24] = [
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
    crate::routes::fiches::ROUTES,
    crate::compagnie::organismes::ROUTES,
    crate::routes::filet::ROUTES,
    crate::historique_mois::ROUTES,
    crate::routes::ouverture::ROUTES,
    crate::routes::rappels::ROUTES,
    crate::compagnie::conseils::ROUTES,
    crate::compagnie::releve_signatures::ROUTES,
    crate::compagnie::ouverture::ROUTES,
    crate::obligations::ROUTES,
    crate::compagnie::administrateur::ROUTES,
];

/// LE CATALOGUE : toutes les routes d'une compagnie, chacune une fois, dans l'ordre des modules.
pub fn catalogue() -> Vec<Fiche> {
    FICHES.iter().flat_map(|l| l.iter().copied()).collect()
}

/// `moteur routes [NOM] [--avance] [--json]` : le catalogue (le chemin simple ; toutes avec `--avance`), ou la fiche
/// entiere d'une route.
pub fn commande(args: &[String]) -> i32 {
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
            println!("{}", f.en_json());
        } else {
            print!("{}", f.en_texte());
        }
        return 0;
    }
    let montrees: Vec<&Fiche> = tout.iter().filter(|f| avance || !f.avance).collect();
    if json {
        println!("{}", json!({"routes": montrees.iter().map(|f| f.en_json()).collect::<Vec<_>>()}));
        return 0;
    }
    println!("Les routes (echo '{{\"route\":NOM,\"charge\":{{…}}}}' | moteur route ; la fiche : moteur routes NOM ; toutes : --avance)\n");
    let large = montrees.iter().map(|f| f.nom.len()).max().unwrap_or(0);
    for f in &montrees {
        println!("  {:<large$}  {:<5}  {}", f.nom, match f.acte { Acte::Lit => "lit", Acte::Ecrit => "écrit", Acte::Engage => "engage" }, f.geste);
    }
    0
}

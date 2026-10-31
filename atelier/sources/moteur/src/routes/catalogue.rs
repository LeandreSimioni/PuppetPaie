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

/// LA FICHE D'UNE ROUTE. `geste` vide : la fiche reste a ecrire (`a_ecrire`).
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

impl Fiche {
    /// La fiche d'une route qui n'est pas encore ecrite : le catalogue la nomme, sans rien en dire.
    pub const fn a_ecrire(nom: &'static str) -> Fiche {
        Fiche { nom, geste: "", charge: &[], exemple: "", rend: "", refus: &[], acte: Acte::Lit, avance: false, phrase: 0 }
    }

    pub fn remplie(&self) -> bool {
        !self.geste.is_empty()
    }

    pub fn en_json(&self) -> Value {
        if !self.remplie() {
            return json!({"nom": self.nom, "fiche": false});
        }
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
        if !self.remplie() {
            return format!("{} : sa fiche n'est pas encore écrite.\n", self.nom);
        }
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

/// LE CATALOGUE : toutes les routes d'une compagnie, chacune une fois, dans l'ordre des modules.
pub fn catalogue() -> Vec<Fiche> {
    use crate::routes::*;
    let noms: Vec<&[&'static str]> = vec![
        &crate::etat_garde::ROUTES,
        mois::ROUTES,
        &etapes::ROUTES,
        contrats::ROUTES,
        modeles_contrat::ROUTES,
        documents::ROUTES,
        depots::ROUTES,
        imprevus::ROUTES,
        &crate::obligations::ROUTES,
        &crate::obligations::ROUTES_LECTURE,
        fiches::ROUTES,
        &[crate::compagnie::organismes::ROUTE_LIRE, crate::compagnie::organismes::ROUTE_ECRIRE],
        coffre::ROUTES,
        &[crate::compagnie::coffre_verifier::ROUTE],
        filet::ROUTES,
        &[filet::ROUTE_OUBLIER],
        import::ROUTES,
        archives::ROUTES,
        &[crate::historique_mois::ROUTE],
        &dossier_salarie::ROUTES,
        ouverture::ROUTES,
        &[declarations::demarrage::ROUTE, COMPAGNIE_SUPPRESSION, rappels::ROUTE],
        &crate::compagnie::conseils::ROUTES,
        &crate::compagnie::releve_signatures::ROUTES,
        &crate::compagnie::ouverture::ROUTES,
    ];
    let mut v: Vec<Fiche> = engagement::ROUTES.to_vec();
    for n in noms.into_iter().flatten() {
        if !v.iter().any(|f| f.nom == *n) {
            v.push(Fiche::a_ecrire(n));
        }
    }
    v
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
    let montrees: Vec<&Fiche> = tout.iter().filter(|f| f.remplie() && (avance || !f.avance)).collect();
    let a_ecrire: Vec<&str> = tout.iter().filter(|f| !f.remplie()).map(|f| f.nom).collect();
    if json {
        println!("{}", json!({"routes": montrees.iter().map(|f| f.en_json()).collect::<Vec<_>>(), "sans_fiche": a_ecrire}));
        return 0;
    }
    println!("Les routes (echo '{{\"route\":NOM,\"charge\":{{…}}}}' | moteur route ; la fiche : moteur routes NOM ; toutes : --avance)\n");
    let large = montrees.iter().map(|f| f.nom.len()).max().unwrap_or(0);
    for f in &montrees {
        println!("  {:<large$}  {:<5}  {}", f.nom, match f.acte { Acte::Lit => "lit", Acte::Ecrit => "écrit", Acte::Engage => "engage" }, f.geste);
    }
    if !a_ecrire.is_empty() {
        println!("\nSans fiche encore ({}) : {}", a_ecrire.len(), a_ecrire.join(", "));
    }
    0
}

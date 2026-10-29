//! LA FABRIQUE DE BULLETINS FICTIFS d'un logiciel precedent, a la maniere de deux logiciels de paie : CoolPaie (A)
//! et sPAIEctacle (B), leurs positions relevees sur leurs bulletins. Une page se decrit (les
//! structures `A` et `B`), la fabrique la dessine (`texte_fixe`, positions du haut-gauche de la page), et le lecteur
//! (`bulletin_lu`) la relit comme un vrai bulletin.
//!
//! UN SEUL CODE POUR DEUX USAGES : les tests (`bulletin_lu`, `routes::import`) l'incluent tel quel
//! (`tests/bulletins_fictifs/mod.rs`, par `#[path]`), et l'outil d'atelier `moteur-atelier fabriquer-bulletins`
//! (`fabriquer_bulletins.rs`, a cote) le compile sous `--features atelier`. Ce fichier ne depend donc que de
//! `serde_json`, `fabrique_pdf` et `std` (rien de `crate::`), et le binaire livre ne le contient pas.
//!
//! Les textes sont generiques (`S: AsRef<str>`, `&'static str` par defaut : les exemples des tests s'ecrivent en
//! litteraux, l'outil en `String`). Rien de reel : noms, adresses, NIR, Siret, numeros d'objet, titres et montants
//! sont ceux qu'on lui donne, inventes (regle 9 a).

#![allow(dead_code)]

use serde_json::{json, Value};
use std::path::Path;

// ------------------------------------------------------------------------------------------- la fabrique

pub fn t(b: &mut Vec<Value>, x: f64, y: f64, texte: &str) {
    b.push(json!({"type": "texte_fixe", "texte": texte, "x": x, "y": y, "taille": 9.0}));
}

/// Un nombre, aligne a droite sur `x`.
pub fn r(b: &mut Vec<Value>, x: f64, y: f64, texte: &str) {
    b.push(json!({"type": "texte_fixe", "texte": texte, "x": x, "y": y, "taille": 9.0, "alignement": "droite"}));
}

pub fn grand(b: &mut Vec<Value>, x: f64, y: f64, texte: &str, taille: f64) {
    b.push(json!({"type": "texte_fixe", "texte": texte, "x": x, "y": y, "taille": taille}));
}

/// Les octets d'un PDF d'une page par liste de blocs, A4.
pub fn pdf_octets(pages: Vec<Vec<Value>>) -> Result<Vec<u8>, String> {
    let d = json!({
        "format": 3,
        "page": {"largeur": 595.22, "hauteur": 842.0, "marges": {"haut": 0.0, "droite": 0.0, "bas": 0.0, "gauche": 0.0}},
        "police": {"taille": 9.0, "interligne": 1.25, "couleur": "#000000"},
        "documents": pages.into_iter().map(|b| json!({"blocs": b})).collect::<Vec<_>>()
    });
    fabrique_pdf::fabriquer(&d.to_string()).map_err(|r| format!("la fabrique refuse : {}", r.0))
}

/// Un PDF d'une page par liste de blocs, A4 (les tests : un refus arrete le test).
pub fn pdf(pages: Vec<Vec<Value>>, chemin: &Path) {
    let octets = pdf_octets(pages).unwrap_or_else(|m| panic!("{}", m));
    std::fs::write(chemin, octets).unwrap();
}

fn s<S: AsRef<str>>(x: &S) -> &str {
    x.as_ref()
}

// ------------------------------------------------------------------------------------------- l'identite

/// Ce qu'un bulletin imprime de l'entreprise et du salarie.
#[derive(Clone, Debug)]
pub struct Identite<S = &'static str> {
    /// La raison sociale, la rue, « code postal commune ».
    pub entreprise: [S; 3],
    /// Quatorze chiffres, sans espace (sPAIEctacle les espace lui-meme).
    pub siret: S,
    pub naf: S,
    pub urssaf: S,
    /// La convention, sur deux rangs, telle qu'elle s'imprime.
    pub convention: [S; 2],
    pub licence: S,
    /// Le nom (CoolPaie l'ecrit en capitales avant le prenom ; sPAIEctacle, en capitales apres).
    pub nom: S,
    pub prenom: S,
    /// La rue, « code postal commune ».
    pub adresse: [S; 2],
    /// JJ/MM/AAAA.
    pub naissance: S,
    /// Quinze caracteres, sans espace.
    pub nir: S,
    /// Le code (ou matricule) du salarie dans l'ancien logiciel.
    pub code: S,
    pub conges_spectacles: Option<S>,
}

/// Le Siret espace comme sPAIEctacle l'imprime : 3, 3, 3, 5.
fn siret_espace(siret: &str) -> String {
    if siret.len() != 14 || !siret.is_ascii() {
        return siret.to_string();
    }
    format!("{} {} {} {}", &siret[..3], &siret[3..6], &siret[6..9], &siret[9..])
}

/// Le NIR espace comme sPAIEctacle l'imprime : 1, 2, 2, 2, 3, 3, 2.
fn nir_espace(nir: &str) -> String {
    if nir.len() != 15 || !nir.is_ascii() {
        return nir.to_string();
    }
    let mut out = Vec::new();
    let mut k = 0;
    for n in [1, 2, 2, 2, 3, 3, 2] {
        out.push(&nir[k..k + n]);
        k += n;
    }
    out.join(" ")
}

// ------------------------------------------------------------------------------------------- gabarit A

/// Une ligne du tableau des cotisations : un titre de famille (`famille` sans nombres), ou une cotisation.
/// (x du libelle, libelle, famille attendue, base, taux employeur, montant employeur, taux salarie, montant salarie)
/// Une base `B` s'imprime comme le brut, `C` comme la base de la CSG.
pub type RangA<S = &'static str> = (f64, S, &'static str, S, S, S, S, S);

pub struct A<S = &'static str> {
    pub id: Identite<S>,
    pub numero: S,
    /// Numero d'objet et titre du spectacle.
    pub objet: Option<(S, S)>,
    pub emploi: S,
    pub statut: S,
    pub periode: S,
    /// Les rangs des dates travaillees.
    pub dates: Vec<S>,
    /// (quantite, libelle, taux, montant)
    pub lignes: Vec<(S, S, S, S)>,
    pub brut: S,
    pub csg: S,
    /// Les cotisations, dans l'ordre du bulletin clarifie.
    pub cotisations: Vec<RangA<S>>,
    /// Le total des cotisations : (employeur, salarie).
    pub total: [S; 2],
    /// Le net social, qui est aussi le net a payer avant impot (CoolPaie n'a pas de ligne non soumise).
    pub net: S,
    /// (base, taux, montant, cumul)
    pub pas: [S; 4],
    /// (periode, cumul)
    pub net_imposable: [S; 2],
    pub net_a_payer: S,
    pub regle: S,
    /// « mode de reglement » (absent avant 2023).
    pub mode: bool,
    /// « Cachets: » ou « Heures: »
    pub unite: S,
    /// (unite, brut, plafond, cout employeur) x (periode, cumul)
    pub cumuls: [[S; 2]; 4],
    pub total_verse: S,
}

pub fn page_a<S: AsRef<str>>(a: &A<S>) -> Vec<Value> {
    let mut b = Vec::new();
    let b_ = &mut b;
    let id = &a.id;
    grand(b_, 320.0, 45.0, &format!("Bulletin de paie n° {}", s(&a.numero)), 11.0);
    if let Some((objet, titre)) = &a.objet {
        t(b_, 354.3, 65.0, &format!("numéro d'objet : {}", s(objet)));
        t(b_, 354.3, 76.0, &format!("Spectacle : {}", s(titre)));
    }
    grand(b_, 28.4, 120.0, s(&id.entreprise[0]), 11.0);
    t(b_, 283.5, 120.0, &format!("{} {}", s(&id.nom).to_uppercase(), s(&id.prenom).to_uppercase()));
    t(b_, 28.4, 132.0, s(&id.entreprise[1]));
    t(b_, 283.5, 132.0, &s(&id.adresse[0]).to_uppercase());
    t(b_, 28.4, 143.0, s(&id.entreprise[2]));
    t(b_, 283.5, 143.0, &s(&id.adresse[1]).to_uppercase());
    for (y, gauche, valeur_g, droite, valeur_d) in [
        (172.0, "N° Siret :", s(&id.siret), "Date de naissance :", s(&id.naissance)),
        (183.0, "NAF(APE) :", s(&id.naf), "N° Sécurité Sociale :", s(&id.nir)),
        (194.0, "Urssaf :", s(&id.urssaf), "Code Salarié :", s(&id.code)),
        (205.0, "Conv. collective :", s(&id.convention[0]), "Emploi :", s(&a.emploi)),
        (227.0, "Licence spec. N° :", s(&id.licence), "Abattement :", "0 %"),
    ] {
        t(b_, 28.4, y, gauche);
        if !valeur_g.is_empty() {
            t(b_, 127.5, y, valeur_g);
        }
        t(b_, 283.5, y, droite);
        t(b_, 436.4, y, valeur_d);
    }
    if !s(&id.convention[1]).is_empty() {
        t(b_, 127.5, 216.0, s(&id.convention[1]));
    }
    t(b_, 283.5, 217.5, "Statut :");
    t(b_, 436.4, 217.5, s(&a.statut));
    t(b_, 283.5, 238.0, "N° de Congés Spectacles :");
    if let Some(c) = &id.conges_spectacles {
        t(b_, 436.4, 238.0, s(c));
    }
    t(b_, 79.5, 262.0, "Périodes :");
    t(b_, 135.9, 262.0, s(&a.periode));
    t(b_, 42.8, 273.0, "Dates travaillées :");
    for (k, d) in a.dates.iter().enumerate() {
        t(b_, 135.9, 273.0 + 11.0 * k as f64, s(d));
    }
    let mut y = 308.0;
    t(b_, 104.9, y, "LIBELLE");
    t(b_, 324.3, y, "Base");
    t(b_, 382.6, y, "Part Employeur");
    t(b_, 504.6, y, "Part Salarié");
    for (q, libelle, taux, montant) in &a.lignes {
        y += 11.2;
        t(b_, 56.7, y, &format!("{} {}", s(q), s(libelle)));
        r(b_, 346.0, y, s(taux));
        r(b_, 453.7, y, s(montant));
    }
    y += 11.2;
    t(b_, 56.7, y, "SALAIRE BRUT");
    r(b_, 453.7, y, s(&a.brut));
    y += 14.2;
    for (x, texte) in [(394.1, "Taux"), (453.9, "Montant"), (504.8, "Taux"), (561.6, "Montant")] {
        r(b_, x, y, texte);
    }
    for (x, libelle, _, base, te, me, ts, ms) in a.cotisations.iter() {
        y += 11.2;
        t(b_, *x, y, s(libelle));
        let base = match s(base) {
            "B" => s(&a.brut),
            "C" => s(&a.csg),
            autre => autre,
        };
        for (xd, v) in [(345.9, base), (394.2, s(te)), (453.7, s(me)), (504.9, s(ts)), (561.5, s(ms))] {
            if !v.is_empty() {
                r(b_, xd, y, v);
            }
        }
    }
    y += 22.0;
    t(b_, 36.8, y, "Total des Cotisations et Contributions");
    r(b_, 434.0, y, s(&a.total[0]));
    r(b_, 561.4, y, s(&a.total[1]));
    y += 20.3;
    t(b_, 33.9, y, "NET SOCIAL");
    r(b_, 561.6, y, s(&a.net));
    y += 11.0;
    t(b_, 33.9, y, "NET A PAYER AVANT IMPOT SUR LE REVENU");
    r(b_, 561.5, y, s(&a.net));
    // Le pied.
    y += 11.5;
    t(b_, 33.9, y, "Impôt sur le revenu");
    t(b_, 310.9, y, "base");
    t(b_, 396.3, y, "taux personnalisé");
    t(b_, 486.8, y, "montant");
    t(b_, 543.0, y, "cumul");
    y += 14.0;
    t(b_, 33.9, y, "Impôt sur le revenu prélevé à la source");
    r(b_, 331.7, y, s(&a.pas[0]));
    r(b_, 466.8, y, s(&a.pas[1]));
    t(b_, 471.6, y, "%");
    r(b_, 516.0, y, s(&a.pas[2]));
    r(b_, 567.2, y, s(&a.pas[3]));
    y += 8.5;
    t(b_, 33.9, y, "Net Imposable");
    r(b_, 518.9, y, s(&a.net_imposable[0]));
    r(b_, 567.2, y, s(&a.net_imposable[1]));
    y += 8.5;
    t(b_, 33.9, y, "Montant net des heures compl / suppl exonérées");
    r(b_, 518.9, y, "0,00");
    r(b_, 567.2, y, "0,00");
    y += 13.0;
    t(b_, 131.8, y, "Période");
    t(b_, 203.6, y, "Cumul");
    t(b_, 435.7, y, "NET A PAYER AU SALARIE");
    t(b_, 255.2, y + 2.0, &format!("Réglement le : {}", s(&a.regle)));
    t(b_, 460.3, y + 10.0, "(EN EUROS)");
    for (k, libelle) in [s(&a.unite), "Salaire brut:", "Plafond Urssaf:", "Cout employeur:"].iter().enumerate() {
        let yk = y + 14.0 + 11.2 * k as f64;
        t(b_, 36.8, yk, libelle);
        r(b_, 170.2, yk, s(&a.cumuls[k][0]));
        r(b_, 235.5, yk, s(&a.cumuls[k][1]));
    }
    if a.mode {
        t(b_, 255.2, y + 14.0, "mode de règlement : Virement");
    }
    grand(b_, 469.5, y + 32.0, &format!("{} €", s(&a.net_a_payer)), 14.0);
    t(b_, 439.7, y + 51.0, "Total versé par l'employeur");
    grand(b_, 478.9, y + 72.0, &format!("{} €", s(&a.total_verse)), 11.0);
    grand(b_, 31.0, 810.0, "coolpaie 4.0.0.8.312", 7.0);
    b
}

// ------------------------------------------------------------------------------------------- gabarit B

/// (x, libelle, famille attendue, base, taux salarial, part salarie, part employeur)
pub type RangB<S = &'static str> = (f64, S, &'static str, S, S, S, S);

pub struct B<S = &'static str> {
    /// L'identite : l'entreprise sur trois rangs, la convention sur deux, le code du salarie en haut a droite.
    pub id: Identite<S>,
    pub periode: S,
    pub periode_courte: S,
    pub reglement: S,
    pub objet: S,
    pub analytique: S,
    pub emploi: S,
    /// « Artiste », « Technicien », « Non cadre »...
    pub statut: S,
    /// (quantite, libelle, base, montant)
    pub lignes: Vec<(S, S, S, S)>,
    pub brut: (S, S),
    pub cotisations: Vec<RangB<S>>,
    pub total: (S, S, S),
    pub apres_total: Vec<RangB<S>>,
    pub net_social: Option<S>,
    pub non_soumis: Vec<(S, S, S, S)>,
    pub net_avant_impot: S,
    /// L'en-tete du PAS : (x, mot) ; puis la ligne du PAS (libelle, [(x droite, valeur)]).
    pub entete_pas: Vec<(f64, &'static str)>,
    pub pas: (S, Vec<(f64, S)>),
    pub net_imposable: Option<(S, S)>,
    /// Les cumuls : (libelle, periode, cumul).
    pub cumuls: Vec<(S, S, S)>,
    pub net_paye: S,
    /// « Total verse par » : avec le cadre de l'allegement (2020-2022), la valeur dessous ; sinon a droite.
    pub allegement: bool,
    pub total_verse: S,
}

pub fn page_b<S: AsRef<str>>(p: &B<S>) -> Vec<Value> {
    let mut b = Vec::new();
    let b_ = &mut b;
    let id = &p.id;
    grand(b_, 39.0, 36.0, s(&id.entreprise[0]), 10.0);
    grand(b_, 375.0, 38.0, "Bulletin de paie", 14.0);
    t(b_, 545.6, 36.0, s(&id.code));
    t(b_, 39.0, 54.0, s(&id.entreprise[1]));
    t(b_, 375.0, 55.0, "Période");
    t(b_, 443.0, 55.0, s(&p.periode));
    t(b_, 39.0, 76.0, s(&id.entreprise[2]));
    t(b_, 375.0, 67.5, s(&p.periode_courte));
    t(b_, 39.0, 93.0, "Siret");
    t(b_, 87.0, 93.0, &siret_espace(s(&id.siret)));
    t(b_, 39.0, 102.0, "Code APE");
    t(b_, 87.0, 102.0, s(&id.naf));
    t(b_, 375.0, 107.0, "Règlement");
    t(b_, 443.0, 107.0, s(&p.reglement));
    t(b_, 375.0, 121.0, "Numéro d'objet");
    if !s(&p.objet).is_empty() {
        t(b_, 443.0, 121.0, s(&p.objet));
    }
    t(b_, 39.0, 126.5, &format!("Convention Collective : {}", s(&id.convention[0])));
    t(b_, 375.0, 133.0, "Analytique");
    if !s(&p.analytique).is_empty() {
        t(b_, 443.0, 133.0, s(&p.analytique));
    }
    if !s(&id.convention[1]).is_empty() {
        t(b_, 39.0, 135.7, s(&id.convention[1]));
    }
    t(b_, 39.0, 145.0, "Durée congés payés : code du travail, art L3141-3 à 11");
    grand(b_, 314.0, 175.0, &format!("{} {}", s(&id.prenom), s(&id.nom).to_uppercase()), 10.0);
    t(b_, 39.0, 176.5, "Emploi");
    t(b_, 108.0, 176.5, s(&p.emploi));
    t(b_, 39.0, 187.5, "Statut");
    t(b_, 108.0, 187.5, s(&p.statut));
    t(b_, 314.0, 200.0, s(&id.adresse[0]));
    t(b_, 39.0, 209.5, "NIR");
    t(b_, 108.0, 209.5, &nir_espace(s(&id.nir)));
    if let Some(c) = &id.conges_spectacles {
        t(b_, 39.0, 220.5, "N° Congés Sp.");
        t(b_, 108.0, 220.5, s(c));
    }
    let (cp, commune) = s(&id.adresse[1]).split_once(' ').unwrap_or((s(&id.adresse[1]), ""));
    t(b_, 314.0, 222.0, cp);
    if !commune.is_empty() {
        t(b_, 369.0, 222.0, commune);
    }
    t(b_, 314.0, 233.0, "FRANCE");
    t(b_, 499.0, 264.0, "XYZ");
    t(b_, 399.8, 279.5, "Taux");
    t(b_, 458.2, 279.5, "Part");
    t(b_, 523.2, 279.5, "Part");
    t(b_, 126.2, 286.5, "libellé");
    t(b_, 339.7, 286.5, "base");
    t(b_, 394.5, 290.0, "salarial");
    t(b_, 452.7, 290.0, "salarié");
    t(b_, 509.2, 290.0, "employeur");
    let mut y = 308.5;
    let ligne = |b_: &mut Vec<Value>, y: f64, (q, libelle, base, montant): &(S, S, S, S)| {
        t(b_, 59.5, y, &format!("{} ", s(q)));
        t(b_, 74.0, y, s(libelle));
        r(b_, 374.4, y, s(base));
        r(b_, 494.4, y, s(montant));
    };
    for l in &p.lignes {
        ligne(b_, y, l);
        y += 11.0;
    }
    t(b_, 74.0, y, s(&p.brut.0));
    r(b_, 494.4, y, s(&p.brut.1));
    let cotisation = |b_: &mut Vec<Value>, y: f64, c: &RangB<S>| {
        t(b_, c.0, y, s(&c.1));
        if !s(&c.3).is_empty() {
            r(b_, 374.4, y, s(&c.3));
        }
        if !s(&c.4).is_empty() {
            r(b_, 419.7, y, s(&c.4));
            t(b_, 421.9, y, "%");
        }
        if !s(&c.5).is_empty() {
            r(b_, 495.0, y, s(&c.5));
        }
        if !s(&c.6).is_empty() {
            r(b_, 559.0, y, s(&c.6));
        }
    };
    y += 9.0;
    for c in &p.cotisations {
        y += if c.0 < 50.0 { 13.0 } else { 10.0 };
        cotisation(b_, y, c);
    }
    y += 13.0;
    t(b_, 74.0, y, s(&p.total.0));
    r(b_, 494.4, y, s(&p.total.1));
    r(b_, 558.3, y, s(&p.total.2));
    for c in &p.apres_total {
        y += 12.0;
        cotisation(b_, y, c);
    }
    if let Some(n) = &p.net_social {
        y += 18.0;
        t(b_, 74.0, y, "MONTANT NET SOCIAL");
        r(b_, 494.2, y, s(n));
    }
    for l in &p.non_soumis {
        y += 18.0;
        ligne(b_, y, l);
    }
    let mut y = 655.0;
    t(b_, 39.0, y, "NET A PAYER AVANT IMPOT SUR LE REVENU");
    r(b_, 494.2, y, s(&p.net_avant_impot));
    y += 19.0;
    t(b_, 99.6, y, "Impôt sur le revenu");
    for (x, mot) in &p.entete_pas {
        t(b_, *x, y, mot);
    }
    if let Some((n, c)) = &p.net_imposable {
        y += 17.0;
        t(b_, 39.0, y, "Montant net imposable");
        r(b_, 486.2, y, s(n));
        r(b_, 563.0, y, s(c));
        y += 13.0;
        t(b_, 39.0, y, "Montant net des HC/HS/RTT exonérées");
        r(b_, 563.0, y, "0,00");
    }
    y += 16.0;
    t(b_, 39.0, y, s(&p.pas.0));
    for (x, v) in &p.pas.1 {
        r(b_, *x, y + 0.5, s(v));
    }
    let mut y = 737.0;
    t(b_, 39.0, y, "BASE");
    t(b_, 131.0, y, "Période");
    t(b_, 189.0, y, "Cumul");
    t(b_, 475.0, y + 3.0, "Net payé");
    for (libelle, periode, cumul) in &p.cumuls {
        y += 11.0;
        t(b_, 39.0, y, s(libelle));
        if !s(periode).is_empty() {
            r(b_, 165.0, y, s(periode));
            r(b_, 220.0, y, s(cumul));
        }
    }
    grand(b_, 493.3, 765.0, &format!("{} €", s(&p.net_paye)), 14.0);
    t(b_, 445.0, 787.0, "Total versé par");
    t(b_, 450.2, 795.0, "l'employeur");
    if p.allegement {
        t(b_, 510.0, 787.0, "Allègement de");
        t(b_, 506.0, 795.0, "cotisations empl.");
        grand(b_, 460.0, 812.0, &format!("{} €", s(&p.total_verse)), 11.0);
        grand(b_, 532.0, 812.0, "0,00 €", 11.0);
    } else {
        grand(b_, 524.7, 792.0, &format!("{} €", s(&p.total_verse)), 11.0);
    }
    grand(b_, 504.7, 828.0, "GHS - sPAIEctacle ©", 6.5);
    b
}

/// Une page sans un mot : un trait seul, aucune police (comme une impression en traces).
pub fn pdf_en_traces() -> Vec<u8> {
    let flux = "10 10 m 190 190 l S";
    let objets = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Contents 4 0 R /Resources << >> >>".to_string(),
        format!("<< /Length {} >>\nstream\n{}\nendstream", flux.len(), flux),
    ];
    let mut o: Vec<u8> = b"%PDF-1.7\n\n".to_vec();
    let mut positions = Vec::new();
    for (k, corps) in objets.iter().enumerate() {
        positions.push(o.len());
        o.extend_from_slice(format!("{} 0 obj\n{}\nendobj\n", k + 1, corps).as_bytes());
    }
    let debut = o.len();
    o.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objets.len() + 1).as_bytes());
    for p in positions {
        o.extend_from_slice(format!("{:010} 00000 n \n", p).as_bytes());
    }
    o.extend_from_slice(format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n", objets.len() + 1, debut).as_bytes());
    o
}

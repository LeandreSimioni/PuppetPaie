//! AER Cinema Spectacle : la section 5, celle qui est a notre charge
//! (`moteur/aer.py`).
//!
//! France Travail edite l'attestation a partir de notre DSN et nous la renvoie
//! (retour de nature 46, voir moteur/retours.py). Elle arrive PRE-REMPLIE des
//! faits -- salarie, contrats, cachets, brut -- et VIDE de son authentification :
//! 158 champs sur 217 sont a blanc, dont le cadre 5.
//!
//! ```text
//!     Je soussigne(e), (Nom) ... (Prenom) ... agissant en qualite de ...
//!     certifie que les renseignements indiques sur la presente attestation
//!     sont exacts ...
//!     Toute fausse declaration est passible de deux ans d'emprisonnement
//!     et d'une amende pouvant aller jusqu'a 30 000 euros (art. L. 5429-1).
//! ```
//!
//! C'est donc le seul endroit du logiciel ou l'on appose une signature sur une
//! declaration de veracite. Deux consequences tenues ici :
//!
//! 1. ON NE REMPLIT QUE LA SECTION 5. Les chiffres de France Travail ne sont
//!    jamais retouches : ce sont eux qui font foi, et les corriger nous-memes
//!    reviendrait a maquiller un document officiel. Si un chiffre est faux,
//!    c'est la DSN qu'on corrige, et France Travail reedite l'attestation.
//!
//! 2. ON REFUSE DE SIGNER UNE ATTESTATION QU'ON SAIT FAUSSE. `verifier` compare
//!    les quantites imprimees sur l'attestation a la saisie de paie du mois. Un
//!    ecart n'est pas une erreur d'affichage : c'est un renseignement inexact au
//!    sens de l'article L. 5429-1, et il vaut refus.
//!
//! Le cas qui a fait ecrire ce module : aout 2026, deux contrats d'une compagnie
//! reelle, un cachet plus douze heures de masterclass. Les douze heures ne
//! sont pas parties dans la DSN ; l'attestation les affiche a blanc. Signer
//! aurait certifie exact un document amputant chaque salarie de douze heures
//! sur les 507 des annexes 8 et 10.
//!
//! Le cachet de l'entreprise (EMPLOYEUR_CACHET) est DESSINE depuis
//! `entreprise.json` (voir `cachet.rs`). Il restait vide jusqu'au 22/09/2026,
//! au motif que << la compagnie n'en a pas >> -- ce qui etait faux : elle en
//! avait un, il etait simplement dans la case SIGNATURE, ou il n'avait rien a
//! faire. Le formulaire n'exige ni l'un ni l'autre ; il les distingue, et nous
//! aussi desormais.
//!
//! La lecture des champs et l'ecriture du formulaire sont celles de la
//! fabrique de PDF, appelee dans le meme processus (`fabrique_pdf::formulaire`).
//! Le jour de la signature vient de l'appelant (le jour du logiciel, celui de
//! Paris) : ce module ne lit jamais l'horloge.

use std::io::Read;
use std::path::Path;

use serde_json::{json, Map, Value};

use crate::cachet;
use crate::date::{en_texte, Jour};
use crate::fichiers::vrai;
use crate::num::Resultat;

/// Les champs du cadre 5, et rien d'autre.
pub const CHAMPS_TEXTE: [&str; 10] = [
    "EMPLOYEUR_NOM",
    "EMPLOYEUR_PRENOM",
    "EMPLOYEUR_QUALITE_LIB",
    "EMPLOYEUR_LIEU_SIGN",
    "EMPLOYEUR_DATE_SIGN_J",
    "EMPLOYEUR_DATE_SIGN_M",
    "EMPLOYEUR_DATE_SIGN_A",
    "EMPLOYEUR_CORRES_NOM",
    "EMPLOYEUR_CORRES_TEL",
    "EMPLOYEUR_CORRES_EMAIL",
];
pub const CHAMP_SIGNATURE: &str = "EMPLOYEUR_SIGNATURE";
/// LA CASE D'A COTE, laissee vide jusqu'au 22/09/2026. Voir `cachet.rs` :
/// ce qu'on posait dans la case SIGNATURE etait un tampon, et celle-ci
/// restait blanche. Chacun chez soi.
pub const CHAMP_CACHET: &str = "EMPLOYEUR_CACHET";

// Quantites imprimees par France Travail, par rang de contrat (01 a 05).
const QUANTITES: [(&str, &str); 2] = [("cachets", "SAL_NB_CACHET_"), ("heures", "SAL_NB_HEURES_")];

// LA CLE DE RAPPROCHEMENT, ET POURQUOI CE N'EST PAS LE NUMERO DE CONTRAT.
// L'attestation imprime bien CON_NUMERO_nn -- << MA2.26.15 >> -- mais ce numero
// vient d'un compteur qui a avance depuis. Recharger paies/2026-08.json
// aujourd'hui rend MA2.26.18, et le rapprochement echouerait sur tout.
// Les dates de contrat, elles, ne bougent pas.
const DEBUT: &str = "SAL_DT_DEB_";
const FIN: &str = "SAL_DT_FIN_";
const NUM_CONTRAT: &str = "CON_NUMERO_";

/// QUAND UNE AER CINEMA SPECTACLE EST DUE : a l'intermittent (CDD d'usage du spectacle, la nature payee a l'activite de
/// `contrats-base.json`, annexes 8 et 10), le mois ou son contrat FINIT -- une seule par employeur et par mois, et pour un
/// contrat sur plusieurs mois, « 1 AE CS éditée à la fin du dernier mois concerné par le contrat »
/// (`atelier/reference/sources/francetravail-100-dsn/movinmotion-fin-des-aem-20260601.txt`, l. 152-166 ;
/// `mescachets-aem-aer-cs-20260903.txt`, l. 47 et 69 : « salariés intermittents du spectacle relevant des annexes 8 et
/// 10 », « l'attestation est produite à la fin du dernier mois concerné » ; `comenolo-aer-cs.txt`, l. 37). France Travail
/// l'edite depuis la DSN mensuelle qui porte la fin du contrat (son bloc 62 ; `francetravail-100-dsn/LISEZMOI.md`,
/// « Ce qu'on en conclut », point 1). Un permanent (CDI, CDD de droit commun) n'en a pas : sa fin de contrat est un
/// signalement (`contrats-base.json`, `cdd.fin_de_contrat.signalement_fctu`), et l'attestation de droit commun qui en
/// revient n'est pas celle-ci.
pub fn due(e: &crate::engagement::Engagement) -> Resultat<bool> {
    if e.nat()?.mensuelle() {
        return Ok(false);
    }
    let (debut, fin) = (e.debut_mois()?, e.fin_mois()?);
    Ok(e.fin.is_some_and(|f| debut <= f && f <= fin))
}

/// Pourquoi une attestation n'est pas rendue.
#[derive(Debug, Clone, PartialEq)]
pub enum Echec {
    /// `AttestationInexacte` : un chiffre ne correspond pas a la paie du mois.
    Ecarts(Vec<String>),
    /// La fabrique de PDF refuse (`FabriqueRefus`) : sa phrase.
    Fabrique(String),
    /// Tout autre refus (une cle absente de la fiche, un jour illisible...).
    Autre(String),
}

impl From<String> for Echec {
    fn from(m: String) -> Echec {
        Echec::Autre(m)
    }
}

type R<T> = Result<T, Echec>;

fn fabrique<T>(r: Result<T, fabrique_pdf::Refus>) -> R<T> {
    r.map_err(|e| Echec::Fabrique(e.0.replace('\n', " ")))
}

/// Ce qu'on appose. Rien n'est invente : tout vient d'entreprise.json.
#[derive(Debug, Clone, PartialEq)]
pub struct Cadre5 {
    pub nom: String,
    pub prenom: String,
    pub qualite: String,
    pub lieu: String,
    pub jour: String,
    pub mois: String,
    pub annee: String,
    pub correspondant: String,
    pub telephone: String,
    pub courriel: String,
}

/// `ent[cle]` : la cle doit exister (`KeyError`).
fn req<'a>(v: &'a Value, cle: &str) -> Resultat<&'a Value> {
    v.get(cle).ok_or_else(|| crate::fichiers::absent(cle))
}

impl Cadre5 {
    pub fn depuis(ent: &Value, aujourdhui: &Jour) -> Resultat<Cadre5> {
        let sig = req(ent, "signataire")?;
        let dec = req(ent, "declarant_dsn")?;
        // Le lieu de signature est le siege, pas le lieu du spectacle.
        let lieu = match ent.get("commune_affichage").filter(|v| vrai(v)) {
            Some(v) => en_texte(v),
            None => en_texte(req(ent, "commune")?),
        };
        Ok(Cadre5 {
            nom: en_texte(req(sig, "nom")?),
            prenom: en_texte(req(sig, "prenom")?),
            qualite: en_texte(req(sig, "fonction")?),
            lieu,
            jour: format!("{:02}", aujourdhui.jour),
            mois: format!("{:02}", aujourdhui.mois),
            annee: format!("{:04}", aujourdhui.an),
            // << Personne a joindre concernant cette attestation >> : c'est le
            // declarant DSN, pas le signataire. France Travail rappelle pour
            // une question de declaration, et c'est l'employeur qui les depose.
            correspondant: en_texte(req(dec, "nom_prenom")?),
            telephone: en_texte(req(dec, "tel")?),
            courriel: en_texte(req(dec, "email")?).to_lowercase(),
        })
    }

    pub fn valeurs(&self) -> Vec<(&'static str, String)> {
        let v = [
            &self.nom,
            &self.prenom,
            &self.qualite,
            &self.lieu,
            &self.jour,
            &self.mois,
            &self.annee,
            &self.correspondant,
            &self.telephone,
            &self.courriel,
        ];
        CHAMPS_TEXTE.iter().copied().zip(v.into_iter().cloned()).collect()
    }
}

/// Une quantite du formulaire -> entier. '' et None valent zero : seuls les
/// chiffres comptent.
pub fn entier(v: Option<&str>) -> Resultat<i128> {
    let t: String = v.unwrap_or("").chars().filter(|c| c.is_ascii_digit()).collect();
    if t.is_empty() {
        return Ok(0);
    }
    t.parse::<i128>().map_err(|_| "quantite illisible : trop de chiffres".to_string())
}

/// << 23/08/2026 >> -> date. None si ce n'est pas une date ; une date qui
/// n'existe pas (31/02) leve, comme `datetime.date`.
pub fn jour(t: &str) -> Resultat<Option<Jour>> {
    let t = t.trim();
    let b = t.as_bytes();
    let chiffres = |a: usize, z: usize| b[a..z].iter().all(u8::is_ascii_digit);
    if b.len() != 10 || b[2] != b'/' || b[5] != b'/' || !chiffres(0, 2) || !chiffres(3, 5) || !chiffres(6, 10) {
        return Ok(None);
    }
    let n = |a: usize, z: usize| t[a..z].parse::<u32>().unwrap();
    let (j, m, a) = (n(0, 2), n(3, 5), n(6, 10));
    if a < 1 {
        return Err(crate::date::annee_impossible(a));
    }
    if !(1..=12).contains(&m) {
        return Err(crate::date::mois_impossible(m));
    }
    Jour::nouveau(a as i32, m, j).map(Some).ok_or_else(|| crate::date::jour_impossible(a, m, j))
}

/// Un contrat tel qu'IMPRIME sur l'attestation.
#[derive(Debug, Clone, PartialEq)]
pub struct Ligne {
    pub debut: Option<Jour>,
    pub fin: Option<Jour>,
    pub numero: String,
    pub cachets: i128,
    pub heures: i128,
}

fn valeur(champs: &Value, nom: &str) -> Option<String> {
    champs.get(nom).and_then(|c| c.get("valeur")).and_then(Value::as_str).map(str::to_string)
}

/// Les quantites imprimees, par (debut, fin) ; les dates se comparent comme des
/// dates, sans passer par une chaine : l'attestation ecrit JJ/MM/AAAA, la paie
/// tient des dates. Deux rangs de meme cle : le dernier l'emporte, a la place
/// du premier (un dictionnaire).
pub fn lire_quantites(champs: &Value) -> Resultat<Vec<Ligne>> {
    let mut out: Vec<Ligne> = Vec::new();
    for rang in 1..=5 {
        let nom = |p: &str| format!("{}{:02}", p, rang);
        let (deb, fin) = (valeur(champs, &nom(DEBUT)), valeur(champs, &nom(FIN)));
        let (deb, fin) = match (deb.filter(|v| !v.is_empty()), fin.filter(|v| !v.is_empty())) {
            (Some(d), Some(f)) => (d, f),
            _ => continue,
        };
        let (debut, fin) = (jour(&deb)?, jour(&fin)?);
        if debut.is_none() && fin.is_none() {
            continue;
        }
        let q = |i: usize| entier(valeur(champs, &nom(QUANTITES[i].1)).as_deref());
        let ligne = Ligne {
            debut,
            fin,
            cachets: q(0)?,
            heures: q(1)?,
            numero: valeur(champs, &nom(NUM_CONTRAT)).unwrap_or_default().trim().to_string(),
        };
        match out.iter_mut().find(|l| l.debut == debut && l.fin == fin) {
            Some(l) => *l = ligne,
            None => out.push(ligne),
        }
    }
    Ok(out)
}

/// Ce que `verifier` lit d'une paie : son engagement, et rien d'autre.
#[derive(Debug, Clone, PartialEq)]
pub struct PaieAer {
    pub debut: Jour,
    pub fin: Jour,
    pub cachets: i128,
    pub heures: i128,
}

/// Les ecarts entre l'attestation et la paie. Liste vide = concordance.
///
/// On compare ce qui se perd en silence : les cachets et les heures. Le brut
/// n'est pas compare ici -- un ecart de brut se verrait au premier coup d'oeil
/// sur le bulletin, alors qu'une case d'heures vide ne se voit pas.
pub fn ecarts(imprime: &[Ligne], paies: &[PaieAer]) -> Vec<String> {
    let mut ecarts = Vec::new();
    for p in paies {
        let ligne = match imprime.iter().find(|l| l.debut == Some(p.debut) && l.fin == Some(p.fin)) {
            Some(l) => l,
            None => continue, // contrat d'un autre salarie
        };
        for (quoi, vu, n) in [("cachets", ligne.cachets, p.cachets), ("heures", ligne.heures, p.heures)] {
            if vu != n {
                ecarts.push(format!(
                    "{} ({} au {}) : {} {} sur l'attestation, {} dans la paie",
                    if ligne.numero.is_empty() { "contrat" } else { &ligne.numero },
                    p.debut.iso(),
                    p.fin.iso(),
                    vu,
                    quoi,
                    n
                ));
            }
        }
    }
    ecarts
}

/// Les champs du formulaire, lus par la fabrique.
pub fn champs(donnees: &[u8]) -> R<Value> {
    fabrique(fabrique_pdf::formulaire::champs(donnees))
}

/// `verifier` : les ecarts entre l'attestation `donnees` et ces paies.
pub fn verifier(donnees: &[u8], paies: &[PaieAer]) -> R<Vec<String>> {
    let lues = lire_quantites(&champs(donnees)?)?;
    Ok(ecarts(&lues, paies))
}

/// La signature occupe le haut de la case, sur 80 % de sa largeur.
///
/// Meme parti que pour le certificat Conges Spectacles : un tampon pose en
/// travers, pas une image etiree aux dimensions de la case. `rect` :
/// [x0, y0, x1, y1] en points, l'origine en haut a gauche de la page.
pub fn cadre_signature(rect: [f64; 4]) -> [f64; 4] {
    let [x0, y0, x1, y1] = rect;
    let largeur = (x1 - x0) * 0.80;
    let hauteur = (y1 - y0) * 0.55;
    [x0 + 4.0, y0 + 4.0, x0 + 4.0 + largeur, y0 + 4.0 + hauteur]
}

fn rect_de(case: &Value) -> Option<(&Value, [f64; 4])> {
    let r = case.get("rect").filter(|v| vrai(v))?;
    let v: Vec<f64> = r.as_array()?.iter().filter_map(Value::as_f64).collect();
    if v.len() != 4 {
        return None;
    }
    Some((r, [v[0], v[1], v[2], v[3]]))
}

/// Ce que `authentifier` recoit, en dehors du PDF lui-meme.
pub struct Authentification<'a> {
    pub ent: &'a Value,
    /// Le jour de la signature : celui du logiciel (Paris), donne par l'appelant.
    pub aujourdhui: Jour,
    /// La signature de la compagnie (PNG, JPEG ou SVG), si elle en a une.
    pub signature: Option<&'a str>,
    /// Les paies du mois, pour la verification. Les omettre, c'est signer sans
    /// regarder -- possible, mais il faut le vouloir.
    pub paies: Option<&'a [PaieAer]>,
    /// Signer malgre un ecart. Reserve au cas ou l'ecart est explique et
    /// accepte ; il laisse une trace dans le journal de la commande.
    pub forcer: bool,
    /// Le tampon de la compagnie, si elle en a pose un. Sans lui, le cachet est
    /// DESSINE depuis `entreprise.json`.
    pub cachet_fichier: Option<&'a str>,
}

/// L'attestation, cadre 5 rempli et signee. Rend les octets du PDF.
pub fn authentifier(donnees: &[u8], a: &Authentification) -> R<Vec<u8>> {
    // L'ordre des refus est celui du Python : l'ecart d'abord (il lit le
    // formulaire), puis les cles de la fiche, puis la lecture du formulaire.
    if let Some(paies) = a.paies {
        let ecarts = ecarts(&lire_quantites(&champs(donnees)?)?, paies);
        if !ecarts.is_empty() && !a.forcer {
            return Err(Echec::Ecarts(ecarts));
        }
    }

    let cadre = Cadre5::depuis(a.ent, &a.aujourdhui)?;
    // Les champs du cadre 5 que CE formulaire porte, et rien d'autre : la
    // fabrique refuse d'ecrire dans un champ en lecture seule -- ceux de France
    // Travail le sont tous --, et l'apparence de chaque champ est regeneree,
    // pour qu'il se lise pareil dans tous les lecteurs.
    let champs = champs(donnees)?;
    let mut valeurs = Map::new();
    for (nom, v) in cadre.valeurs() {
        if champs.get(nom).is_some() {
            valeurs.insert(nom.to_string(), Value::String(v));
        }
    }
    let mut images: Vec<Value> = Vec::new();
    if let Some((rect, r)) = champs.get(CHAMP_SIGNATURE).and_then(rect_de) {
        if let Some(sig) = a.signature.filter(|s| Path::new(s).exists()) {
            // La signature est posee PAR-DESSUS la page, en travers de la case,
            // comme une signature manuscrite. Le champ lui-meme reste vide : on
            // ne tape pas son nom a la place. PNG, JPEG ou SVG (22/09/2026).
            let _ = rect;
            let page = champs[CHAMP_SIGNATURE].get("page").cloned().unwrap_or(Value::Null);
            images.push(cachet::image_fichier(&page, &json!(cadre_signature(r)), sig));
        }
    }
    if let Some((rect, _)) = champs.get(CHAMP_CACHET).and_then(rect_de) {
        // LE SIEN S'IL EN A UN, LE NOTRE SINON. Le dessin est le defaut et non
        // une obligation : tout ce qu'un cachet porte est deja dans
        // `entreprise.json`, donc une compagnie qui installe le logiciel n'a
        // RIEN a fournir (regle 0). Mais celle qui a deja son tampon le pose
        // dans Configuration, et c'est lui qui part ; qu'elle le retire, et le
        // dessin reprend la main.
        let page = champs[CHAMP_CACHET].get("page").cloned().unwrap_or(Value::Null);
        if let Some(tampon) = cachet::image(&page, rect, a.ent, Some(&a.aujourdhui), a.cachet_fichier) {
            images.push(tampon);
        }
    }
    remplir(donnees, &valeurs, &images)
}

/// `fabrique.remplir` : les champs texte `valeurs` ecrits (apparence regeneree)
/// et les `images` posees, par la fabrique du meme processus. Elle refuse
/// l'ensemble pour un champ inconnu, en lecture seule ou trop long, une image
/// illisible, une page qui n'existe pas. Pas de repli.
pub fn remplir(donnees: &[u8], valeurs: &Map<String, Value>, images: &[Value]) -> R<Vec<u8>> {
    let description = json!({"format": fabrique_pdf::FORMAT, "champs": valeurs, "images": images});
    let texte = serde_json::to_string(&description).map_err(|e| Echec::Autre(e.to_string()))?;
    fabrique(fabrique_pdf::formulaire::remplir(donnees, &texte))
}

// ------------------------------------------------------------ le service `paie aer*`

fn lire_demande(entree: &str) -> Resultat<Value> {
    serde_json::from_str(entree).map_err(|e| format!("demande illisible : {}", e))
}

fn chemin<'a>(d: &'a Value, cle: &str) -> Option<&'a str> {
    d.get(cle).and_then(Value::as_str)
}

fn pdf_de(d: &Value) -> Resultat<Vec<u8>> {
    let f = chemin(d, "fichier").ok_or("demande illisible : il faut `fichier`")?;
    let mut octets = Vec::new();
    std::fs::File::open(f)
        .and_then(|mut h| h.read_to_end(&mut octets))
        .map_err(|e| format!("{} illisible ({})", f, e))?;
    Ok(octets)
}

fn paies_de(v: &Value) -> Resultat<Vec<PaieAer>> {
    let mut out = Vec::new();
    for p in v.as_array().ok_or("demande illisible : `paies` doit etre une liste")? {
        let jour = |c: &str| Jour::lire(&en_texte(p.get(c).unwrap_or(&Value::Null)));
        let entier = |c: &str| p.get(c).and_then(Value::as_i64).map(i128::from).ok_or(format!("demande illisible : `{}` doit etre un entier", c));
        out.push(PaieAer { debut: jour("debut")?, fin: jour("fin")?, cachets: entier("cachets")?, heures: entier("heures")? });
    }
    Ok(out)
}

fn ok() -> Map<String, Value> {
    let mut m = Map::new();
    m.insert("ok".into(), Value::Bool(true));
    m
}

/// La reponse d'un echec : les ecarts sont `{"ok": true, "ecarts": [...]}` (le
/// Python releve `AttestationInexacte`), un refus de la fabrique porte sa
/// `classe`, le reste est un refus ordinaire.
fn reponse(r: R<Map<String, Value>>) -> Resultat<Value> {
    let mut m = ok();
    match r {
        Ok(corps) => m.extend(corps),
        Err(Echec::Ecarts(e)) => {
            m.insert("ecarts".into(), Value::Array(e.into_iter().map(Value::String).collect()));
        }
        Err(Echec::Fabrique(e)) => {
            m.insert("ok".into(), Value::Bool(false));
            m.insert("erreur".into(), Value::String(e));
            m.insert("classe".into(), Value::String("FabriqueRefus".into()));
        }
        Err(Echec::Autre(e)) => return Err(e),
    }
    Ok(Value::Object(m))
}

/// `aer` : l'attestation signee. Demande : `fichier` (le PDF), `sortie` (le PDF
/// signe y est ecrit), `entreprise`, `aujourdhui`, `paies` (null : on signe sans
/// regarder, sinon `[{"debut", "fin", "cachets", "heures"}]`), `signature`,
/// `cachet_fichier`, `forcer`. Reponse : `{"chemin"}`, ou `{"ecarts"}` si rien
/// n'est signe.
fn authentifier_service(d: &Value) -> Resultat<Value> {
    let donnees = pdf_de(d)?;
    let sortie = chemin(d, "sortie").ok_or("demande illisible : il faut `sortie`")?;
    let ent = d.get("entreprise").ok_or("demande illisible : il faut `entreprise`")?;
    let aujourdhui = Jour::lire(&en_texte(d.get("aujourdhui").ok_or("demande illisible : il faut `aujourdhui`")?))?;
    let paies = d.get("paies").filter(|v| !v.is_null()).map(paies_de).transpose()?;
    let a = Authentification {
        ent,
        aujourdhui,
        signature: chemin(d, "signature"),
        paies: paies.as_deref(),
        forcer: d.get("forcer").and_then(Value::as_bool).unwrap_or(false),
        cachet_fichier: chemin(d, "cachet_fichier"),
    };
    reponse(authentifier(&donnees, &a).and_then(|octets| {
        std::fs::write(sortie, &octets).map_err(|e| Echec::Autre(format!("ecriture impossible de {} : {}", sortie, e)))?;
        let mut m = Map::new();
        m.insert("chemin".into(), Value::String(sortie.to_string()));
        Ok(m)
    }))
}

/// `aer-verifier` : les ecarts entre l'attestation et les paies.
/// Reponse : `{"ecarts": [...]}` (vide : concordance).
fn verifier_service(d: &Value) -> Resultat<Value> {
    let donnees = pdf_de(d)?;
    let paies = paies_de(d.get("paies").ok_or("demande illisible : il faut `paies`")?)?;
    reponse(verifier(&donnees, &paies).map(|e| {
        let mut m = Map::new();
        m.insert("ecarts".into(), Value::Array(e.into_iter().map(Value::String).collect()));
        m
    }))
}

/// `aer-quantites` : les quantites imprimees, `[{"debut", "fin", "numero",
/// "cachets", "heures"}]` ; une date absente est null.
fn quantites_service(d: &Value) -> Resultat<Value> {
    let donnees = pdf_de(d)?;
    reponse(champs(&donnees).and_then(|c| lire_quantites(&c).map_err(Echec::from)).map(|lues| {
        let jour = |j: Option<Jour>| j.map(|j| Value::String(j.iso())).unwrap_or(Value::Null);
        let mut m = Map::new();
        m.insert(
            "quantites".into(),
            Value::Array(
                lues.into_iter()
                    .map(|l| {
                        json!({"debut": jour(l.debut), "fin": jour(l.fin), "numero": l.numero,
                               "cachets": l.cachets.to_string(), "heures": l.heures.to_string()})
                    })
                    .collect(),
            ),
        );
        m
    }))
}

/// `aer-cachet` : le cachet dessine (SVG) depuis la fiche `entreprise`, ou
/// `{"svg": null}` s'il n'y a rien a dessiner. `aujourdhui` : facultatif.
fn cachet_service(d: &Value) -> Resultat<Value> {
    let ent = d.get("entreprise").ok_or("demande illisible : il faut `entreprise`")?;
    let jour = d.get("aujourdhui").filter(|v| !v.is_null()).map(|v| Jour::lire(&en_texte(v))).transpose()?;
    let mut m = ok();
    m.insert("svg".into(), cachet::svg(ent, jour.as_ref()).map(Value::String).unwrap_or(Value::Null));
    Ok(Value::Object(m))
}

/// Les commandes `paie aer`, `aer-verifier`, `aer-quantites`, `aer-cachet` :
/// la demande en JSON sur l'entree standard. None : commande inconnue.
pub fn service(quoi: &str, entree: &str) -> Option<Resultat<Value>> {
    let f: fn(&Value) -> Resultat<Value> = match quoi {
        "aer" => authentifier_service,
        "aer-verifier" => verifier_service,
        "aer-quantites" => quantites_service,
        "aer-cachet" => cachet_service,
        _ => return None,
    };
    Some(lire_demande(entree).and_then(|d| f(&d)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn une_date_jj_mm_aaaa_se_lit_un_jour_qui_n_existe_pas_refuse_le_reste_s_ignore() {
        assert_eq!(jour(" 23/08/2026 "), Ok(Some(Jour::nouveau(2026, 8, 23).unwrap())));
        assert_eq!(jour("2026-08-23"), Ok(None));
        assert_eq!(jour(""), Ok(None));
        assert!(jour("31/02/2026").is_err());
        assert!(jour("01/13/2026").is_err());
    }

    #[test]
    fn entier_ne_garde_que_les_chiffres() {
        assert_eq!(entier(None), Ok(0));
        assert_eq!(entier(Some("")), Ok(0));
        assert_eq!(entier(Some("12 h")), Ok(12));
        assert_eq!(entier(Some("1 234")), Ok(1234));
        assert_eq!(entier(Some("-3")), Ok(3));
    }

    #[test]
    fn cadre_signature_haut_de_la_case_sur_80_pour_cent() {
        let proche = |a: [f64; 4], b: [f64; 4]| a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-9);
        assert!(proche(cadre_signature([10.0, 20.0, 110.0, 100.0]), [14.0, 24.0, 94.0, 68.0]));
        // la case de l'AER : 100,7 x 48,2 points
        let [x0, y0, x1, y1] = cadre_signature([35.1, 687.4, 135.8, 735.6]);
        assert!((x0 - 39.1).abs() < 1e-9 && (y0 - 691.4).abs() < 1e-9);
        assert!((x1 - x0 - 100.7 * 0.8).abs() < 1e-9);
        assert!((y1 - y0 - 48.2 * 0.55).abs() < 1e-9);
    }
}

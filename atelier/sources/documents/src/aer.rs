//! AER Cinema Spectacle : la section 5, celle qui est a notre charge.
//!
//! France Travail edite l'attestation a partir de notre DSN et nous la renvoie
//! (retour de nature 46, `retours/`). Elle arrive PRE-REMPLIE des
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
//! `entreprise.json` (voir `cachet.rs`). Le cachet de l'entreprise et la signature sont deux cases distinctes du
//! formulaire : le formulaire n'exige ni l'un ni l'autre, et le moteur les distingue.
//!
//! La lecture des champs et l'ecriture du formulaire sont celles de la
//! fabrique de PDF, appelee dans le meme processus (`fabrique_pdf::formulaire`).
//! Le jour de la signature vient de l'appelant (le jour du logiciel, celui de
//! Paris) : ce module ne lit jamais l'horloge.

use std::path::Path;

use serde_json::{json, Map, Value};

use engagement::cachet;
use socle::date::{en_texte, Jour};
use socle::fichiers::vrai;
use socle::num::Resultat;

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
/// LA CASE D'A COTE de la signature : elle porte le cachet (voir `cachet.rs`) ; le tampon ne se pose pas
/// dans la case SIGNATURE. Chacun chez soi.
pub const CHAMP_CACHET: &str = "EMPLOYEUR_CACHET";

// Quantites imprimees par France Travail, par rang de contrat (01 a 05).
const QUANTITES: [(&str, &str); 2] = [("cachets", "SAL_NB_CACHET_"), ("heures", "SAL_NB_HEURES_")];

// LA CLE DE RAPPROCHEMENT : LE NUMERO DE CONTRAT ET LA PERIODE. Le cadre 4 de l'attestation a une colonne
// par PERIODE d'emploi -- un mois d'un contrat --, et le numero du contrat (`CON_NUMERO_nn`, avec ses dates
// `CON_NUM_DT_DEB/FIN_nn`) n'est imprime que dans la colonne de sa premiere periode ; les colonnes suivantes du meme
// contrat le laissent a blanc (releve le 23/10/2026 sur une vraie AER d'octobre 2026, un contrat du 29/09 au 16/10 :
// une colonne 29/09-30/09 numerotee, puis 01/10-16/10 sans numero ; la piece reelle est au depot prive,
// `reference-privee/exemples/`). Une colonne se rapproche donc de la paie DU MOIS de sa periode, sous le numero de
// contrat que la DSN a declare (regle 4 : il se lit -- `apercu_dsn::paies_aux_numeros_declares`, jamais le compteur
// d'aujourd'hui). Le rapprochement par les seules dates du contrat laissait passer sans rien dire un contrat sur
// plusieurs mois : aucune de ses colonnes n'avait les dates du contrat.
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
pub fn due(e: &engagement::Engagement) -> Resultat<bool> {
    if e.nat()?.mensuelle() {
        return Ok(false);
    }
    let (debut, fin) = (e.debut_mois()?, e.fin_mois()?);
    Ok(e.fin.is_some_and(|f| debut <= f && f <= fin))
}

/// LA FIN DE CONTRAT QUI SE SIGNALE (CDD et CDI de droit commun, CDDU rompu avant son terme : `Nature::signalement_fctu`)
/// dans le mois : France Travail en edite l'attestation employeur rematerialisee (AER) en retour du signalement FCTU
/// (nature 44 : « AER » en PDF quand le signalement passe, compte rendu metier quand il est refuse -- guide
/// d'implementation de l'API DSN v6.8, §5.7.4 a 5.7.6 et tableau des formats,
/// `atelier/reference/sources/gip-mds-api-dsn/gip-mds-guide-implementation-api-dsn-v6.8-20260706.txt`), et l'employeur
/// doit la remettre au salarie (fiche net-entreprises 3037,
/// `net-entreprises-base-connaissances/net-entreprises-fiche3037-attestation-employeur-aed-fctu-20250323.txt`). Le moteur
/// ne la range pas pour l'envoi : la forme de ce PDF (ses champs, son cadre d'authentification) n'est relevee sur
/// aucune piece deposee. Le courriel des documents le dit (`envoi::NOTE_SANS_ATTESTATION_FCTU`), sans rien promettre.
pub fn attestation_fctu(e: &engagement::Engagement) -> Resultat<bool> {
    let nat = e.nat()?;
    if !nat.signalement_fctu(e.motif_rupture.as_deref().filter(|m| !m.is_empty())) {
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
    v.get(cle).ok_or_else(|| socle::fichiers::absent(cle))
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
/// n'existe pas (31/02) se refuse.
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
        return Err(socle::date::annee_impossible(a));
    }
    if !(1..=12).contains(&m) {
        return Err(socle::date::mois_impossible(m));
    }
    Jour::nouveau(a as i32, m, j).map(Some).ok_or_else(|| socle::date::jour_impossible(a, m, j))
}

/// Une colonne du cadre 4 telle qu'IMPRIMEE sur l'attestation : la periode, le numero du contrat (repris de la colonne
/// precedente quand celle-ci ne l'imprime pas : la suite du meme contrat), les quantites.
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

/// Les colonnes imprimees, dans l'ordre. Une colonne sans dates de periode n'en est pas une ; une colonne sans numero
/// prend celui de la colonne precedente (la suite du meme contrat). Deux colonnes de meme cle (numero, periode) : la
/// derniere l'emporte, a la place de la premiere.
pub fn lire_quantites(champs: &Value) -> Resultat<Vec<Ligne>> {
    let mut out: Vec<Ligne> = Vec::new();
    let mut numero_courant = String::new();
    for rang in 1..=5 {
        let nom = |p: &str| format!("{}{:02}", p, rang);
        let numero = valeur(champs, &nom(NUM_CONTRAT)).unwrap_or_default().trim().to_string();
        if !numero.is_empty() {
            numero_courant = numero;
        }
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
        let ligne = Ligne { debut, fin, cachets: q(0)?, heures: q(1)?, numero: numero_courant.clone() };
        match out.iter_mut().find(|l| l.debut == debut && l.fin == fin && l.numero == ligne.numero) {
            Some(l) => *l = ligne,
            None => out.push(ligne),
        }
    }
    Ok(out)
}

/// Ce que `verifier` lit d'une paie : le numero de contrat DECLARE, la periode du mois (le contrat borne au mois de la
/// paie : `PaieAer::du_mois`), les quantites du mois.
#[derive(Debug, Clone, PartialEq)]
pub struct PaieAer {
    pub numero: String,
    pub debut: Jour,
    pub fin: Jour,
    pub cachets: i128,
    pub heures: i128,
}

impl PaieAer {
    /// La periode d'une paie : son contrat (`e.debut`, `e.fin`) borne au mois de la paie.
    pub fn du_mois(e: &engagement::Engagement) -> Resultat<PaieAer> {
        let (d, f) = (e.debut_mois()?, e.fin_mois()?);
        let fin = e.fin.ok_or_else(|| format!("contrat {} sans fin", e.numero.clone().unwrap_or_default()))?;
        Ok(PaieAer {
            numero: e.numero.clone().unwrap_or_default(),
            debut: if e.debut > d { e.debut } else { d },
            fin: if fin < f { fin } else { f },
            cachets: i128::from(e.cachets),
            heures: e.heures.unwrap_or(0.0) as i128,
        })
    }
}

fn periode(debut: Option<Jour>, fin: Option<Jour>) -> String {
    let j = |x: Option<Jour>| x.map(|j| j.iso()).unwrap_or_else(|| "?".to_string());
    format!("{} au {}", j(debut), j(fin))
}

fn nom_du_contrat(numero: &str) -> &str {
    if numero.is_empty() {
        "contrat"
    } else {
        numero
    }
}

/// Les ecarts entre l'attestation et la paie. Liste vide = concordance. `paies` : TOUTES les periodes que l'attestation
/// doit porter pour ce salarie (les paies des contrats dont l'AER est due, mois par mois).
///
/// Chaque colonne se rapproche de la paie de meme numero et de meme periode : ses cachets et ses heures doivent etre
/// ceux de la paie (ce qui se perd en silence ; un ecart de brut se verrait au premier coup d'oeil sur le bulletin). Une
/// colonne qu'aucune paie n'explique est un ecart ; une periode de la paie qu'aucune colonne ne porte aussi : signer,
/// ce serait certifier exacte une attestation qui ajoute ou retire une periode d'emploi.
pub fn ecarts(imprime: &[Ligne], paies: &[PaieAer]) -> Vec<String> {
    let mut ecarts = Vec::new();
    for ligne in imprime {
        let p = paies.iter().find(|p| p.numero == ligne.numero && Some(p.debut) == ligne.debut && Some(p.fin) == ligne.fin);
        let Some(p) = p else {
            ecarts.push(format!(
                "{} ({}) : une periode sur l'attestation qu'aucune paie n'explique ({} cachets, {} heures)",
                nom_du_contrat(&ligne.numero),
                periode(ligne.debut, ligne.fin),
                ligne.cachets,
                ligne.heures
            ));
            continue;
        };
        for (quoi, vu, n) in [("cachets", ligne.cachets, p.cachets), ("heures", ligne.heures, p.heures)] {
            if vu != n {
                ecarts.push(format!(
                    "{} ({}) : {} {} sur l'attestation, {} dans la paie",
                    nom_du_contrat(&ligne.numero),
                    periode(ligne.debut, ligne.fin),
                    vu,
                    quoi,
                    n
                ));
            }
        }
    }
    for p in paies {
        if !imprime.iter().any(|l| l.numero == p.numero && l.debut == Some(p.debut) && l.fin == Some(p.fin)) {
            ecarts.push(format!(
                "{} ({}) : une periode de la paie ({} cachets, {} heures) que l'attestation ne porte pas",
                nom_du_contrat(&p.numero),
                periode(Some(p.debut), Some(p.fin)),
                p.cachets,
                p.heures
            ));
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
    // L'ordre des refus : l'ecart d'abord (il lit le
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
            // ne tape pas son nom a la place. PNG, JPEG ou SVG.
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
        if let Some(tampon) = cachet::image(&page, rect, a.ent, a.cachet_fichier) {
            images.push(tampon);
        }
    }
    remplir(donnees, &valeurs, &images)
}

/// Les champs texte `valeurs` ecrits (apparence regeneree)
/// et les `images` posees, par la fabrique du meme processus. Elle refuse
/// l'ensemble pour un champ inconnu, en lecture seule ou trop long, une image
/// illisible, une page qui n'existe pas. Pas de repli.
pub fn remplir(donnees: &[u8], valeurs: &Map<String, Value>, images: &[Value]) -> R<Vec<u8>> {
    let description = json!({"format": fabrique_pdf::FORMAT, "champs": valeurs, "images": images});
    let texte = serde_json::to_string(&description).map_err(|e| Echec::Autre(e.to_string()))?;
    fabrique(fabrique_pdf::formulaire::remplir(donnees, &texte))
}

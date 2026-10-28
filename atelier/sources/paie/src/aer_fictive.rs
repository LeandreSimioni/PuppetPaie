//! L'AER d'une COMPAGNIE FICTIVE (tickets #225-#227, repris par #236) : ce que France Travail renverrait.
//!
//! Chez une vraie compagnie, France Travail edite l'« Attestation employeur pour les activites relevant des annexes 8
//! et 10 » (AER Cinema Spectacle) a partir des DSN deposees, et la publie en retour de nature 46 : une archive .zip d'un
//! PDF par salarie (`retours::aer_spectacle`). Le logiciel la range, en remplit et signe le cadre 5
//! (`aer::authentifier`), et elle part au salarie avec ses documents. Une compagnie fictive ne depose rien nulle part :
//! sans ce module, son AER n'existait pas, et ses intermittents ne la recevaient jamais.
//!
//! Ici, on FABRIQUE cette archive a la place de France Travail, depuis les DSN que la compagnie fictive a jouees -- ce
//! que France Travail lit, et rien d'autre : le salarie (bloc 30), l'employeur et ses contrats d'usage (bloc 40) dont la
//! DSN du mois declare la fin (bloc 62), et pour chacun ses periodes d'emploi, mois par mois, lues dans la DSN reelle de
//! chaque mois du contrat (heures, cachets, jours, brut, date de paiement). Une attestation par salarie et par mois ; un
//! contrat sur plusieurs mois y porte tous ses mois (« 1 AE CS éditée à la fin du dernier mois concerné par le contrat »,
//! `aer::due`, sources citees la) ; cinq periodes au plus par PDF (les colonnes 01 a 05 du formulaire). Puis l'archive
//! suit LE MEME CHEMIN qu'une vraie (`releve_retours::rattacher_simulation_detail` : l'archive dans `retours/`, puis
//! `routes_documents::ranger_aer`, qui signe le cadre 5 et la pose ou l'envoi la prend).
//!
//! LE GABARIT est celui du formulaire de France Travail -- trois pages A4, les cadres 1 (employeur) a 5
//! (authentification), et les NOMS DE CHAMPS AcroForm qu'il porte : `ENTETE_*`, `EMP_*`, `ADR_EMP_*`, `SALARIE_*`,
//! `CON_LIB_EMPLOI` ; au cadre 4, par colonne `nn`, `CON_NUMERO_nn`, `CON_NUM_DT_DEB_nn`, `CON_NUM_DT_FIN_nn`,
//! `CON_OBJET_nn` (le contrat, dans la colonne de sa premiere periode), `SAL_DT_DEB_nn`, `SAL_DT_FIN_nn`,
//! `SAL_NB_HEURES_nn`, `SAL_NB_CACHET_nn`, `SAL_TPS_TRA_nn`, `SAL_MONTANT_nn`, `SAL_DT_PAI_nn` (la periode) ; au cadre 5,
//! les `EMPLOYEUR_*` que remplit `aer.rs`. Relevé le 23/10/2026 sur une vraie attestation (octobre 2026), gardée dans le
//! depot prive (`reference-privee/exemples/`, une piece reelle : regle 9) ; d'elle, on ne reprend que ces noms et la
//! place des cadres, aucune valeur. Les champs imprimes sont en lecture seule, ceux du cadre 5 vides. Le PDF est un
//! formulaire de la fabrique (`fabrique_pdf::fabriquer`, puis `fabrique_pdf::formulaire::ajouter_champs`) ; il dit en
//! tete et en pied de chaque page, en rouge, qu'il est le document d'une compagnie fictive et qu'aucun organisme ne l'a
//! emis. Les cases a cocher du formulaire (licence, civilite, nationalite, cadre) ne sont pas reprises : la fabrique
//! n'ajoute que des champs texte ; ce qu'elles diraient s'ecrit en clair quand la DSN le dit.

use std::collections::HashMap;
use std::io::Write;

use serde_json::{json, Value};

use crate::apercu_dsn::{lire_recap, Contrat, Individu};
use droit::contexte::Contexte;
use socle::date::{en_texte, Jour};
use socle::num::{Decimal, Resultat};

/// Le « code » du nom d'archive (`AER_<siret>__<MMAAAA>_<code>_<JJMMAAAA>_<NOM>_<rang>_<id>.pdf`) : celui d'une fictive.
const CODE: &str = "FICTIVE";
/// La nature de contrat de la DSN d'un contrat d'usage (S21.G00.40.007 ; `contrats-base.json`, `cddu_spectacle`).
const NATURE_CDDU: &str = "02";
/// Les colonnes du cadre 4 : 01 a 05 (`aer::lire_quantites`).
const COLONNES: usize = 5;

const ROUGE: &str = "#B22222";
const NOIR: &str = "#1F2A33";
const GRIS: &str = "#7A848C";
const BLANC: &str = "#FFFFFF";

// ------------------------------------------------------------------ ce que la DSN dit, en plus de `lire_recap`

/// Ce que `apercu_dsn::lire_recap` ne lit pas et que l'attestation imprime : le salarie (naissance, adresse), et par
/// contrat l'IDCC (40.017), la caisse de conges payes (40.022) et la licence (40.049) ; la date du premier versement.
#[derive(Default, Clone)]
struct Complement {
    naissance: String,
    lieu_naissance: String,
    adresse: String,
    code_postal: String,
    commune: String,
    paiement: String,
    /// numero de contrat -> rubrique (`017`, `022`, `049`) -> valeur.
    contrats: HashMap<String, HashMap<String, String>>,
}

fn complements(octets: &[u8]) -> HashMap<String, Complement> {
    let texte = socle::texte::texte_latin1(octets);
    let mut out: HashMap<String, Complement> = HashMap::new();
    let mut nir = String::new();
    let mut contrat = String::new();
    let mut en_attente: HashMap<String, String> = HashMap::new();
    for ligne in socle::texte::lignes_py(&texte) {
        let Some((k, v)) = ligne.split_once(',') else { continue };
        let v = v.trim().trim_matches('\'').to_string();
        let (bloc, champ) = (k.get(..10).unwrap_or(""), k.get(11..).unwrap_or(""));
        match (bloc, champ) {
            ("S21.G00.30", "001") => {
                nir = v;
                out.entry(nir.clone()).or_default();
            }
            ("S21.G00.40", "001") => {
                contrat.clear();
                en_attente.clear();
            }
            ("S21.G00.40", "009") => {
                contrat = v;
                let c = out.entry(nir.clone()).or_default();
                c.contrats.entry(contrat.clone()).or_default().extend(en_attente.drain());
            }
            ("S21.G00.40", "017" | "022" | "049") => {
                if contrat.is_empty() {
                    en_attente.insert(champ.to_string(), v);
                } else if let Some(c) = out.get_mut(&nir) {
                    c.contrats.entry(contrat.clone()).or_default().insert(champ.to_string(), v);
                }
            }
            ("S21.G00.30", _) | ("S21.G00.50", "001") => {
                let Some(c) = out.get_mut(&nir) else { continue };
                match (bloc, champ) {
                    ("S21.G00.30", "006") => c.naissance = v,
                    ("S21.G00.30", "007") => c.lieu_naissance = v,
                    ("S21.G00.30", "008") => c.adresse = v,
                    ("S21.G00.30", "009") => c.code_postal = v,
                    ("S21.G00.30", "010") => c.commune = v,
                    ("S21.G00.50", "001") if c.paiement.is_empty() => c.paiement = v,
                    _ => {}
                }
            }
            _ => {}
        }
    }
    out
}

// ------------------------------------------------------------------ les periodes

/// Une colonne du cadre 4 : une periode d'emploi d'un contrat, dans un mois ; `contrat` n'est porte que par la colonne
/// de sa premiere periode (comme sur le formulaire).
struct Periode {
    contrat: Option<(String, String, String, String)>,
    debut: Jour,
    fin: Jour,
    heures: Decimal,
    cachets: Decimal,
    jours: Decimal,
    brut: Decimal,
    paiement: String,
}


fn mois_suivant(m: &str) -> String {
    let (a, mo): (i32, u32) = (m.get(..4).and_then(|x| x.parse().ok()).unwrap_or(0), m.get(5..7).and_then(|x| x.parse().ok()).unwrap_or(1));
    if mo >= 12 {
        format!("{:04}-01", a + 1)
    } else {
        format!("{:04}-{:02}", a, mo + 1)
    }
}

/// La DSN reelle deposee pour `m` (le journal), lue ; `None` si le mois n'en a pas.
fn dsn_deposee(ctx: &Contexte, lignes: &[Value], m: &str) -> Option<Vec<Individu>> {
    let l = crate::etats::dsn_du_mois(lignes, &ctx.entreprise, m).ok()?;
    let fichier = l.get("fichier").map(socle::valeur::valeur_en_texte).filter(|f| !f.is_empty() && f != "None")?;
    let p = std::path::Path::new(&fichier);
    let chemin = if p.is_absolute() { p.to_path_buf() } else { ctx.racine.join("sorties").join(m).join(&fichier) };
    lire_recap(&std::fs::read(chemin).ok()?).ok()
}

/// Les periodes d'un contrat qui finit ce mois-ci : une par mois du contrat dont la DSN reelle le porte.
fn periodes(ctx: &Contexte, lignes: &[Value], mois: &str, nir: &str, c: &Contrat, paiement: &str) -> Resultat<Vec<Periode>> {
    let (Ok(debut), Ok(fin)) = (Jour::lire(&c.debut), Jour::lire(&c.fin)) else {
        return Err(format!("contrat {} : dates illisibles dans la DSN ({} - {})", c.numero, c.debut, c.fin));
    };
    let mut out = Vec::new();
    let mut m = c.debut.get(..7).unwrap_or(mois).to_string();
    while m.as_str() < mois {
        let lues = dsn_deposee(ctx, lignes, &m).unwrap_or_default();
        let trouve = lues.iter().find(|i| i.nir == nir).and_then(|i| i.contrats.iter().find(|x| x.numero == c.numero).map(|x| (i, x)));
        if let Some((i, x)) = trouve {
            let date = i.versements.first().map(|v| v.date.clone()).unwrap_or_default();
            out.push(periode(x, debut, fin, &m, &date, out.is_empty())?);
        }
        m = mois_suivant(&m);
    }
    out.push(periode(c, debut, fin, mois, paiement, out.is_empty())?);
    Ok(out)
}

fn periode(x: &Contrat, debut: Jour, fin: Jour, m: &str, paiement: &str, premiere: bool) -> Resultat<Periode> {
    let (d, f) = (socle::date::premier_du_mois(m)?, socle::date::dernier_du_mois(m)?);
    Ok(Periode {
        contrat: premiere.then(|| (x.numero.clone(), debut.jj_mm_aaaa(), fin.jj_mm_aaaa(), x.objet.clone())),
        debut: if debut > d { debut } else { d },
        fin: if fin < f { fin } else { f },
        heures: x.heures,
        cachets: x.cachets,
        jours: x.jours,
        brut: x.brut,
        paiement: Jour::lire(paiement).map(|j| j.jj_mm_aaaa()).unwrap_or_default(),
    })
}

// ------------------------------------------------------------------ le dessin

fn texte(x: f64, y: f64, t: &str, taille: f64, gras: bool, couleur: &str) -> Value {
    json!({"type": "texte_fixe", "x": x, "y": y, "texte": t, "taille": taille, "gras": gras, "couleur": couleur})
}

fn centre(x: f64, y: f64, t: &str, taille: f64, gras: bool, couleur: &str) -> Value {
    json!({"type": "texte_fixe", "x": x, "y": y, "texte": t, "taille": taille, "gras": gras, "couleur": couleur, "alignement": "centre"})
}

fn trait_(x1: f64, y1: f64, x2: f64, y2: f64, ep: f64, couleur: &str) -> Value {
    json!({"type": "trait", "x1": x1, "y1": y1, "x2": x2, "y2": y2, "epaisseur": ep, "couleur": couleur})
}

fn case(b: &mut Vec<Value>, r: [f64; 4]) {
    let [x0, y0, x1, y1] = r;
    for (a, bb, c, d) in [(x0, y0, x1, y0), (x1, y0, x1, y1), (x1, y1, x0, y1), (x0, y1, x0, y0)] {
        b.push(trait_(a, bb, c, d, 0.4, GRIS));
    }
}

/// Un cadre numerote : le numero dans un carre noir, le titre en capitales, un filet.
fn cadre(b: &mut Vec<Value>, y: f64, n: &str, titre: &str) {
    b.push(trait_(30.0, y - 4.0, 48.0, y - 4.0, 16.0, NOIR));
    b.push(centre(39.0, y, n, 10.0, true, BLANC));
    b.push(texte(56.0, y, titre, 10.0, true, NOIR));
    b.push(trait_(30.0, y + 6.0, 565.0, y + 6.0, 0.6, NOIR));
}

fn etiquette(b: &mut Vec<Value>, x: f64, y: f64, t: &str) {
    b.push(texte(x, y, t, 7.5, false, NOIR));
}

/// La mention de la compagnie fictive, en tete et en pied de chaque page.
fn mention(b: &mut Vec<Value>, page: usize) {
    b.push(trait_(30.0, 48.0, 565.0, 48.0, 18.0, ROUGE));
    b.push(centre(297.6, 51.0, "COMPAGNIE FICTIVE — DOCUMENT DE DÉMONSTRATION : aucun organisme ne l'a émis, il n'a aucune valeur", 8.0, true, BLANC));
    b.push(trait_(30.0, 818.0, 565.0, 818.0, 0.5, ROUGE));
    b.push(texte(30.0, 829.0, "Compagnie fictive : imitation de l'attestation que France Travail édite d'après la DSN, pour la démonstration du logiciel.", 6.5, false, ROUGE));
    b.push(texte(565.0, 829.0, &format!("{}/3", page), 7.0, false, NOIR));
    if let Some(Value::Object(o)) = b.last_mut() {
        o.insert("alignement".into(), json!("droite"));
    }
}

/// Les champs ajoutes au dessin.
#[derive(Default)]
struct Champs(Vec<Value>);

impl Champs {
    /// Imprime (lecture seule) ; une valeur vide n'ecrit pas de champ (le formulaire la laisse a blanc).
    fn imprime(&mut self, page: usize, nom: &str, r: [f64; 4], v: &str, taille: f64, alignement: &str) {
        if !v.trim().is_empty() {
            self.0.push(json!({"nom": nom, "page": page, "rect": r, "valeur": v, "lecture_seule": true, "taille": taille, "alignement": alignement}));
        }
    }
    /// A remplir par l'employeur (le cadre 5).
    fn vide(&mut self, nom: &str, r: [f64; 4]) {
        self.0.push(json!({"nom": nom, "page": 3, "rect": r, "taille": 9}));
    }
}

fn entier(d: Decimal) -> String {
    if d.is_zero() {
        String::new()
    } else {
        d.trunc().normalize().to_string()
    }
}

/// Le PDF d'une attestation : le salarie `ind` (et ce que la DSN dit de plus, `plus`), ses periodes (cinq au plus),
/// l'emploi et le motif de rupture de son contrat, l'employeur `ent`, edite le `edite_le`.
fn attestation(ent: &Value, ind: &Individu, plus: &Complement, periodes: &[&Periode], emploi: &str, motif: &str, idcc: &str, edite_le: Jour, remplace: Option<&str>) -> Resultat<Vec<u8>> {
    if periodes.is_empty() || periodes.len() > COLONNES {
        return Err(format!("une attestation porte de 1 a {} periodes ({} donnees)", COLONNES, periodes.len()));
    }
    let champ = |c: &str| ent.get(c).map(en_texte).unwrap_or_default();
    let premier_contrat = ind.contrats.iter().find(|c| c.nature == NATURE_CDDU).map(|c| c.numero.clone()).unwrap_or_default();
    let du_contrat = |r: &str| plus.contrats.get(&premier_contrat).and_then(|m| m.get(r)).cloned().unwrap_or_default();
    let mut f = Champs::default();
    let (mut p1, mut p2, mut p3): (Vec<Value>, Vec<Value>, Vec<Value>) = (Vec::new(), Vec::new(), Vec::new());

    // L'en-tete de chaque page : la provenance, le salarie, le logiciel.
    // UNE DSN QUI ANNULE ET REMPLACE (type 03 ou 05, S20.G00.05.002) : France Travail reedite, et l'en-tete le dit, avec
    // l'ordre de la declaration (comme la vraie : « Attestation annule et remplace éditée … N° d'ordre … »).
    let provenance = match remplace {
        Some(ordre) => format!("Attestation annule et remplace éditée pour la démonstration (compagnie fictive) le {}. N° d'ordre {} — imitation : France Travail n'a rien édité", edite_le.jj_mm_aaaa(), ordre),
        None => format!("Attestation éditée pour la démonstration (compagnie fictive) le {} — imitation : France Travail n'a rien édité", edite_le.jj_mm_aaaa()),
    };
    let qui = format!("{}, {} {}", ind.nir, ind.nom, ind.prenoms);
    f.imprime(1, "ENTETE_PROVENANCE", [7.0, 5.0, 588.0, 16.0], &provenance, 6.5, "centre");
    f.imprime(1, "ENTETE_SALARIE", [88.0, 16.0, 571.0, 26.0], &qui, 6.5, "centre");
    // Un nom de champ n'est porte qu'une fois : les pages 2 et 3 redisent l'en-tete en texte.
    for b in [&mut p2, &mut p3] {
        b.push(centre(297.6, 13.0, &provenance, 6.5, false, NOIR));
        b.push(centre(297.6, 23.5, &qui, 6.5, false, NOIR));
    }
    f.imprime(1, "ENTETE_MENTION_LOGICIEL", [88.0, 26.0, 571.0, 37.0], "Logiciel utilisé PUPPETPAIE - reconstituée à partir des déclarations DSN de la compagnie fictive", 6.5, "centre");
    for (b, n) in [(&mut p1, 1), (&mut p2, 2), (&mut p3, 3)] {
        mention(b, n);
    }

    // Page 1 : le titre, a qui elle est destinee, les cadres 1 a 3.
    p1.push(texte(55.0, 92.0, "Attestation employeur pour les activités relevant des", 14.0, true, NOIR));
    p1.push(texte(55.0, 109.0, "annexes 8 et 10 au règlement d'assurance chômage", 14.0, true, NOIR));
    p1.push(texte(55.0, 123.0, "Issue du processus de déclaration prévue par l'article R.133-14 II 2° du code de la sécurité sociale", 8.0, false, NOIR));
    p1.push(texte(55.0, 148.0, "À SIGNER PAR L'EMPLOYEUR ET À REMETTRE AU SALARIÉ", 9.0, true, NOIR));
    p1.push(texte(55.0, 159.0, "Articles L. 5422-14 et R. 5422-6 du code du travail", 7.5, false, NOIR));
    for (i, l) in [
        "Cette attestation est destinée au salarié ayant conclu",
        "plusieurs contrats à durée déterminée d'usage d'une durée",
        "inférieure ou égale à un mois pour la réalisation d'activités",
        "relevant des annexes VIII et X au règlement d'assurance",
        "chômage et dont le motif de fin de contrat de travail est une",
        "fin de CDD. En cas de contrat d'une durée supérieure à 1 mois,",
        "une attestation est remise au salarié pour chaque contrat.",
    ]
    .iter()
    .enumerate()
    {
        p1.push(texte(55.0, 180.0 + 10.0 * i as f64, l, 7.5, false, NOIR));
    }
    for (i, l) in [
        "ATTENTION, cette attestation doit être transmise par le salarié",
        "à France Travail : en la téléchargeant sur son espace personnel si,",
        "au terme de sa demande d'allocations dématérialisée, il lui est",
        "demandé de la transmettre.",
    ]
    .iter()
    .enumerate()
    {
        p1.push(texte(320.0, 180.0 + 10.0 * i as f64, l, 7.5, i == 0, NOIR));
    }

    cadre(&mut p1, 315.0, "1", "EMPLOYEUR");
    etiquette(&mut p1, 30.0, 343.0, "N° SIRET");
    f.imprime(1, "EMP_SIRET", [75.0, 334.0, 253.0, 346.0], &champ("siret"), 8.5, "gauche");
    etiquette(&mut p1, 300.0, 343.0, "Code APE/NAF");
    f.imprime(1, "EMP_APE", [368.0, 333.0, 432.0, 345.0], &champ("naf"), 8.5, "gauche");
    etiquette(&mut p1, 470.0, 343.0, "N° IDCC");
    f.imprime(1, "EMP_IDCC", [515.0, 333.0, 566.0, 345.0], &champ("idcc"), 8.5, "gauche");
    etiquette(&mut p1, 30.0, 362.0, "Nom ou raison sociale");
    f.imprime(1, "ENSEIGNE", [146.0, 352.0, 567.0, 365.0], &champ("raison_sociale"), 8.5, "gauche");
    etiquette(&mut p1, 30.0, 380.0, "Adresse");
    f.imprime(1, "ADR_EMP_CONCATENEE", [74.0, 370.0, 567.0, 383.0], &champ("adresse"), 8.5, "gauche");
    etiquette(&mut p1, 30.0, 397.0, "Code postal");
    f.imprime(1, "ADR_EMP_CP", [90.0, 388.0, 155.0, 399.0], &champ("code_postal"), 8.5, "gauche");
    etiquette(&mut p1, 165.0, 397.0, "Commune");
    f.imprime(1, "ADR_EMP_LOCALITE", [224.0, 387.0, 567.0, 399.0], &champ("commune"), 8.5, "gauche");
    etiquette(&mut p1, 30.0, 425.0, "Êtes-vous titulaire d'une licence spectacle ou d'un n° de récépissé de déclaration d'entrepreneur");
    etiquette(&mut p1, 30.0, 435.0, "de spectacles vivants valant licence ?");
    let licence = du_contrat("049");
    etiquette(&mut p1, 200.0, 446.0, if licence.is_empty() { "Non" } else { "Oui, n°" });
    f.imprime(1, "EMP_LICENCE_1", [236.0, 437.0, 494.0, 448.0], &licence, 8.5, "gauche");
    etiquette(&mut p1, 30.0, 485.0, "Numéro d'affiliation à la caisse de congé spectacle :");
    let caisse = du_contrat("022");
    etiquette(&mut p1, 30.0, 503.0, if caisse.is_empty() { "Non" } else { "Oui, N°" });
    f.imprime(1, "EMP_CAISSE_CP_ART_CODE", [78.0, 493.0, 331.0, 505.0], &caisse, 8.5, "gauche");

    cadre(&mut p1, 548.0, "2", "SALARIÉ");
    etiquette(&mut p1, 30.0, 590.0, "Nom de famille (nom de naissance)");
    f.imprime(1, "SALARIE_NOM_FAMILLE", [209.0, 580.0, 563.0, 594.0], &ind.nom, 9.0, "gauche");
    etiquette(&mut p1, 30.0, 605.0, "Nom d'usage (nom d'époux (se))");
    etiquette(&mut p1, 30.0, 624.0, "Prénom(s)");
    f.imprime(1, "SALARIE_PRENOMS", [86.0, 614.0, 563.0, 627.0], &ind.prenoms, 9.0, "gauche");
    etiquette(&mut p1, 30.0, 639.0, "Adresse");
    f.imprime(1, "SALARIE_ADRESSE", [73.0, 630.0, 563.0, 642.0], &plus.adresse, 8.5, "gauche");
    etiquette(&mut p1, 30.0, 652.0, "Code postal");
    f.imprime(1, "SALARIE_CP", [93.0, 643.0, 158.0, 655.0], &plus.code_postal, 8.5, "gauche");
    etiquette(&mut p1, 170.0, 652.0, "Commune");
    f.imprime(1, "SALARIE_ADR_LOCALITE", [237.0, 643.0, 563.0, 655.0], &plus.commune, 8.5, "gauche");
    etiquette(&mut p1, 30.0, 667.0, "NIR (n° de Sécurité sociale)");
    f.imprime(1, "SALARIE_NIR", [165.0, 658.0, 331.0, 670.0], &ind.nir, 8.5, "gauche");
    etiquette(&mut p1, 30.0, 681.0, "Date de naissance");
    let n = &plus.naissance;
    if n.len() == 8 {
        f.imprime(1, "SALARIE_DT_NAISSANCE_J", [123.0, 672.0, 149.0, 683.0], &n[..2], 8.5, "centre");
        f.imprime(1, "SALARIE_DT_NAISSANCE_M", [155.0, 672.0, 181.0, 683.0], &n[2..4], 8.5, "centre");
        f.imprime(1, "SALARIE_DT_NAISSANCE_A", [186.0, 672.0, 238.0, 683.0], &n[4..], 8.5, "centre");
    }
    etiquette(&mut p1, 280.0, 681.0, "Lieu de naissance");
    f.imprime(1, "SALARIE_LIEU_NAISSANCE", [361.0, 672.0, 561.0, 684.0], &plus.lieu_naissance, 8.5, "gauche");

    cadre(&mut p1, 745.0, "3", "CONTRAT DE TRAVAIL");
    etiquette(&mut p1, 30.0, 773.0, "Emploi occupé");
    f.imprime(1, "CON_LIB_EMPLOI", [109.0, 764.0, 562.0, 776.0], emploi, 8.5, "gauche");
    let motif_lu = if motif == "031" { "Fin de CDD".to_string() } else { format!("motif {} (S21.G00.62.002)", motif) };
    etiquette(&mut p1, 30.0, 792.0, &format!("Motif de la rupture : {}", motif_lu));

    // Page 2 : le cadre 4, une colonne par periode.
    cadre(&mut p2, 70.0, "4", "PRESTATIONS DE TRAVAIL ET SALAIRES");
    let xs: [(f64, f64); COLONNES] = [(147.0, 227.0), (230.0, 310.0), (315.0, 396.0), (397.0, 478.0), (482.0, 563.0)];
    let lignes_4: [(f64, f64, &str); 11] = [
        (82.0, 100.0, "Numéro de contrat"),
        (105.0, 120.0, "Début du contrat (du)"),
        (125.0, 140.0, "Fin du contrat (au)"),
        (145.0, 172.0, "Numéro d'objet"),
        (184.0, 206.0, "N° IDCC de la prestation (si différent)"),
        (218.0, 233.0, "Début de période (du)"),
        (236.0, 252.0, "Fin de période (au)"),
        (258.0, 273.0, "Heures"),
        (282.0, 298.0, "Cachets*"),
        (306.0, 322.0, "Nombre de jours travaillés"),
        (339.0, 360.0, "Salaire brut (non plafonné)"),
    ];
    for (y0, y1, t) in lignes_4 {
        etiquette(&mut p2, 32.0, (y0 + y1) / 2.0 + 2.5, t);
        p2.push(trait_(30.0, y1 + 1.5, 565.0, y1 + 1.5, 0.3, GRIS));
    }
    etiquette(&mut p2, 32.0, 392.0, "Date de paiement");
    p2.push(trait_(30.0, 402.0, 565.0, 402.0, 0.3, GRIS));
    p2.push(texte(32.0, 414.0, "* Uniquement pour les artistes", 7.0, false, GRIS));
    for (i, p) in periodes.iter().enumerate() {
        let n = format!("{:02}", i + 1);
        let (x0, x1) = xs[i];
        let r = |y0: f64, y1: f64| [x0, y0, x1, y1];
        if let Some((numero, deb, fin, objet)) = &p.contrat {
            f.imprime(2, &format!("CON_NUMERO_{}", n), r(82.0, 100.0), numero, 8.5, "centre");
            f.imprime(2, &format!("CON_NUM_DT_DEB_{}", n), r(105.0, 120.0), deb, 8.5, "centre");
            f.imprime(2, &format!("CON_NUM_DT_FIN_{}", n), r(125.0, 140.0), fin, 8.5, "centre");
            f.imprime(2, &format!("CON_OBJET_{}", n), r(145.0, 172.0), objet, 8.0, "centre");
            let idcc_contrat = plus.contrats.get(numero).and_then(|m| m.get("017")).cloned().unwrap_or_default();
            if !idcc_contrat.is_empty() && idcc_contrat != idcc {
                f.imprime(2, &format!("CON_IDCC_{}", n), r(184.0, 206.0), &idcc_contrat, 8.5, "centre");
            }
        }
        f.imprime(2, &format!("SAL_DT_DEB_{}", n), r(218.0, 233.0), &p.debut.jj_mm_aaaa(), 8.5, "centre");
        f.imprime(2, &format!("SAL_DT_FIN_{}", n), r(236.0, 252.0), &p.fin.jj_mm_aaaa(), 8.5, "centre");
        f.imprime(2, &format!("SAL_NB_HEURES_{}", n), r(258.0, 273.0), &entier(p.heures), 8.5, "centre");
        f.imprime(2, &format!("SAL_NB_CACHET_{}", n), r(282.0, 298.0), &entier(p.cachets), 8.5, "centre");
        f.imprime(2, &format!("SAL_TPS_TRA_{}", n), r(306.0, 322.0), &entier(p.jours), 8.5, "centre");
        f.imprime(2, &format!("SAL_MONTANT_{}", n), r(339.0, 360.0), &format!("{:.2}", p.brut.round_dp(2)), 8.5, "centre");
        f.imprime(2, &format!("SAL_DT_PAI_{}", n), r(379.0, 400.0), &p.paiement, 8.5, "centre");
    }
    for (titre, y, entetes) in [
        ("Primes et indemnités", 460.0, ["Numéro de contrat", "Intitulé de la prime", "Date de paiement", "Montant brut (non plafonné)"]),
        ("Activité partielle", 660.0, ["Numéro de contrat", "Du (JJ/MM/AA) au (JJ/MM/AA)", "Nombre d'heures", "Indemnité d'activité partielle"]),
    ] {
        p2.push(texte(32.0, y, titre, 9.0, true, NOIR));
        for (i, e) in entetes.iter().enumerate() {
            p2.push(texte(45.0 + 130.0 * i as f64, y + 16.0, e, 7.0, false, GRIS));
        }
        for k in 0..5 {
            let yy = y + 36.0 + 20.0 * k as f64;
            p2.push(texte(32.0, yy, &format!("{:02}", k + 1), 7.5, false, GRIS));
            p2.push(trait_(30.0, yy + 5.0, 565.0, yy + 5.0, 0.3, GRIS));
        }
    }

    // Page 3 : les absences, les autres remunerations, le cadre 5.
    for (titre, y, entetes, n) in [
        ("Périodes d'absence du salarié (congés maladie, maternité, paternité, d'adoption)", 90.0, ["Numéro de contrat", "Motif d'absence", "", "Du (JJ/MM/AA) au (JJ/MM/AA)"], 8),
        ("Si présence d'autres rémunérations :", 340.0, ["Du (JJ/MM/AA) au (JJ/MM/AA)", "Date de paiement", "", "Autres rémunérations brutes"], 6),
    ] {
        p3.push(texte(32.0, y, titre, 9.0, true, NOIR));
        for (i, e) in entetes.iter().enumerate() {
            p3.push(texte(45.0 + 130.0 * i as f64, y + 16.0, e, 7.0, false, GRIS));
        }
        for k in 0..n {
            let yy = y + 36.0 + 20.0 * k as f64;
            p3.push(texte(32.0, yy, &format!("{:02}", k + 1), 7.5, false, GRIS));
            p3.push(trait_(30.0, yy + 5.0, 565.0, yy + 5.0, 0.3, GRIS));
        }
    }
    cadre(&mut p3, 525.0, "5", "AUTHENTIFICATION PAR L'EMPLOYEUR");
    etiquette(&mut p3, 35.0, 554.0, "Je soussigné(e), (Nom)");
    case(&mut p3, [131.0, 542.0, 569.0, 558.0]);
    f.vide("EMPLOYEUR_NOM", [131.0, 542.0, 569.0, 558.0]);
    etiquette(&mut p3, 35.0, 571.0, "(Prénom)");
    case(&mut p3, [74.0, 560.0, 569.0, 575.0]);
    f.vide("EMPLOYEUR_PRENOM", [74.0, 560.0, 569.0, 575.0]);
    etiquette(&mut p3, 35.0, 591.0, "agissant en qualité de");
    case(&mut p3, [130.0, 581.0, 551.0, 594.0]);
    f.vide("EMPLOYEUR_QUALITE_LIB", [130.0, 581.0, 551.0, 594.0]);
    etiquette(&mut p3, 35.0, 610.0, "certifie que les renseignements indiqués sur la présente attestation sont exacts et notamment, en cas de");
    etiquette(&mut p3, 35.0, 620.0, "cessation du contrat, que le motif de la rupture est une fin de contrat à durée déterminée.");
    case(&mut p3, [38.0, 630.0, 290.0, 646.0]);
    f.vide("EMPLOYEUR_LIEU_SIGN", [38.0, 630.0, 290.0, 646.0]);
    etiquette(&mut p3, 318.0, 642.0, "Le");
    for (nom, r) in [
        ("EMPLOYEUR_DATE_SIGN_J", [333.0, 632.0, 359.0, 645.0]),
        ("EMPLOYEUR_DATE_SIGN_M", [363.0, 632.0, 389.0, 645.0]),
        ("EMPLOYEUR_DATE_SIGN_A", [393.0, 632.0, 444.0, 645.0]),
    ] {
        case(&mut p3, r);
        f.vide(nom, r);
    }
    etiquette(&mut p3, 35.0, 676.0, "Signature de l'employeur");
    case(&mut p3, [31.0, 683.0, 152.0, 764.0]);
    f.vide("EMPLOYEUR_SIGNATURE", [31.0, 683.0, 152.0, 764.0]);
    etiquette(&mut p3, 199.0, 664.0, "Personne à joindre concernant cette attestation");
    case(&mut p3, [199.0, 671.0, 392.0, 694.0]);
    f.vide("EMPLOYEUR_CORRES_NOM", [199.0, 671.0, 392.0, 694.0]);
    etiquette(&mut p3, 160.0, 706.0, "Téléphone");
    case(&mut p3, [206.0, 697.0, 386.0, 709.0]);
    f.vide("EMPLOYEUR_CORRES_TEL", [206.0, 697.0, 386.0, 709.0]);
    etiquette(&mut p3, 160.0, 722.0, "E-mail");
    case(&mut p3, [192.0, 711.0, 392.0, 733.0]);
    f.vide("EMPLOYEUR_CORRES_EMAIL", [192.0, 711.0, 392.0, 733.0]);
    for (i, l) in [
        "Toute fausse déclaration est passible de deux ans",
        "d'emprisonnement et d'une amende pouvant aller",
        "jusqu'à 30 000 euros (Art. L. 5429-1 du code du travail)",
        "Vous devez conserver cette attestation",
    ]
    .iter()
    .enumerate()
    {
        p3.push(texte(199.0, 745.0 + 9.0 * i as f64, l, 7.0, i == 3, NOIR));
    }
    etiquette(&mut p3, 430.0, 678.0, "Cachet de l'entreprise");
    case(&mut p3, [401.0, 685.0, 562.0, 765.0]);
    f.vide("EMPLOYEUR_CACHET", [401.0, 685.0, 562.0, 765.0]);

    let description = json!({
        "format": fabrique_pdf::FORMAT,
        "page": {"largeur": 595.28, "hauteur": 841.89, "marges": {"haut": 0, "droite": 0, "bas": 0, "gauche": 0}},
        "titre": format!("Attestation employeur annexes 8 et 10 (compagnie fictive) — {} {}", ind.nom, ind.prenoms),
        "documents": [{"blocs": p1}, {"blocs": p2}, {"blocs": p3}]
    });
    let dessin = documents::fabrique::fabriquer_octets(&description)?;
    let champs = json!({"format": fabrique_pdf::FORMAT, "champs": f.0});
    fabrique_pdf::formulaire::ajouter_champs(&dessin, &champs.to_string()).map_err(|r| r.0.replace('\n', " "))
}

/// Le nom d'un PDF dans l'archive, au motif de France Travail (`retours::nom_aer`) : le SIRET, la periode `MMAAAA`, le
/// jour d'edition, le nom et les prenoms du salarie (que `retours::rattacher` rapproche de sa fiche).
fn nom_dans_l_archive(siret: &str, mois: &str, edite_le: Jour, ind: &Individu, rang: usize, id: usize) -> String {
    let periode: String = format!("{}{}", mois.get(5..7).unwrap_or("00"), mois.get(..4).unwrap_or("0000"));
    let qui: String = format!("{} {}", ind.nom, ind.prenoms).split_whitespace().collect::<Vec<_>>().join("_");
    let siret: String = siret.chars().filter(char::is_ascii_digit).collect();
    format!(
        "AER_{}__{}_{}_{:02}{:02}{:04}_{}_{}_{}.pdf",
        if siret.is_empty() { "0".to_string() } else { siret },
        periode,
        CODE,
        edite_le.jour,
        edite_le.mois,
        edite_le.an,
        qui,
        rang,
        id
    )
}

/// L'archive de nature 46 que France Travail publierait pour la DSN `dsn` du mois `mois` : un PDF par salarie dont la
/// DSN declare la fin d'un contrat d'usage (cinq periodes par PDF, les mois anterieurs du contrat lus dans les DSN
/// reelles deposees). `None` : aucune AER Cinema Spectacle n'est due ce mois-ci, France Travail ne publie rien.
pub(crate) fn archive(ctx: &Contexte, dsn: &[u8], mois: &str, edite_le: Jour) -> Resultat<Option<Vec<u8>>> {
    let individus = lire_recap(dsn)?;
    let plus = complements(dsn);
    // Le type et l'ordre de la declaration (S20.G00.05.002 et .004) : 03 et 05 annulent et remplacent.
    let texte = socle::texte::texte_latin1(dsn);
    let rubrique = |r: &str| {
        socle::texte::lignes_py(&texte)
            .into_iter()
            .find_map(|l| l.strip_prefix(r).and_then(|v| v.strip_prefix(',')).map(|v| v.trim().trim_matches('\'').to_string()))
            .unwrap_or_default()
    };
    let ordre = rubrique("S20.G00.05.004");
    let remplace = matches!(rubrique("S20.G00.05.002").as_str(), "03" | "05").then_some(ordre.as_str());
    let lignes = crate::etats::charger(&ctx.racine).map_err(|e| e.message)?;
    let ent = &ctx.entreprise;
    let siret = ent.get("siret").map(en_texte).unwrap_or_default();
    let idcc = ent.get("idcc").map(en_texte).unwrap_or_default();
    let mut pdfs: Vec<(String, Vec<u8>)> = Vec::new();
    for ind in &individus {
        let mut finis: Vec<&Contrat> = ind.contrats.iter().filter(|c| c.nature == NATURE_CDDU && !c.fin_declaree.is_empty()).collect();
        if finis.is_empty() {
            continue;
        }
        finis.sort_by(|a, b| (a.debut.as_str(), a.fin.as_str()).cmp(&(b.debut.as_str(), b.fin.as_str())));
        let complement = plus.get(&ind.nir).cloned().unwrap_or_default();
        let paiement = ind.versements.first().map(|v| v.date.clone()).unwrap_or_default();
        let mut toutes: Vec<Periode> = Vec::new();
        for c in &finis {
            toutes.extend(periodes(ctx, &lignes, mois, &ind.nir, c, &paiement)?);
        }
        let emploi = finis.iter().map(|c| c.emploi.clone()).find(|e| !e.is_empty()).unwrap_or_default();
        let motif = finis.last().map(|c| c.motif_fin.clone()).unwrap_or_default();
        let refs: Vec<&Periode> = toutes.iter().collect();
        for (rang, lot) in refs.chunks(COLONNES).enumerate() {
            let pdf = attestation(ent, ind, &complement, lot, &emploi, &motif, &idcc, edite_le, remplace)?;
            let id = 900_001 + pdfs.len();
            pdfs.push((nom_dans_l_archive(&siret, mois, edite_le, ind, rang + 1, id), pdf));
        }
    }
    if pdfs.is_empty() {
        return Ok(None);
    }
    let mut z = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for (nom, octets) in &pdfs {
        z.start_file(nom.as_str(), options).map_err(|e| e.to_string())?;
        z.write_all(octets).map_err(|e| e.to_string())?;
    }
    Ok(Some(z.finish().map_err(|e| e.to_string())?.into_inner()))
}


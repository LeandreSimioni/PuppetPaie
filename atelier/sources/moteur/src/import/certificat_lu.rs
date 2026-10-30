//! LE LECTEUR DES CERTIFICATS D'EMPLOI CONGES SPECTACLES. Le certificat que l'employeur remet a l'intermittent a la fin
//! de son contrat (« Le certificat d'emploi est remis au salarié à l'issue de son contrat de travail », article 4 du
//! reglement du regime ; « Les certificats d'emploi à remettre aux salariés sont établis sur papier blanc et édités
//! automatiquement par votre logiciel » : Audiens, « Congés Spectacles employeur -- Mode d'emploi », avril 2023,
//! `atelier/reference/sources/audiens-dsn/audiens-conges-spectacles-mode-emploi-employeur-20261030.txt`, l. 187-195)
//! porte la mise en page de la Caisse, quel que soit le logiciel qui l'edite.
//!
//! IL SE LIT COMME UN TEXTE A RUBRIQUES, PAS COMME DES POSITIONS SUR LA PAGE : chaque rubrique (« N° SIRET »,
//! « COMPTE », « NIR », « NOM DE NAISSANCE », « PRENOM », « EMPLOI », « CADRE », « DEBUT », « FIN », « BASE CONGE »,
//! « SALAIRE BRUT »...) ouvre un rang, et sa valeur est ce qui la suit sur ce rang, jusqu'a la rubrique suivante que ce
//! rang peut porter (`RANGS`). Un rang se fait des mots dont le milieu vertical est a moins de `TOLERANCE` du premier.
//! Les dates s'ecrivent `JJ MM AAAA` ou `JJ/MM/AAAA` selon le logiciel ; les deux se lisent.
//!
//! Ce que le certificat porte et que l'import reprend :
//! - le N° SIRET de l'employeur, son numero d'adherent a la Caisse (« COMPTE » : dix chiffres et une lettre,
//!   `documents::conges_spectacles::CLE_ADHERENT`), sa raison sociale (imprimee sur le rang du NIR, sans rubrique) ;
//! - le NIR du salarie (treize caracteres, puis sa cle, verifiee), ses nom de naissance, prenom, adresse (le bloc
//!   au-dessus du titre : son nom, puis ses rangs d'adresse jusqu'au code postal), son emploi, cadre ou non ;
//! - le contrat : DEBUT, FIN, le « NB JOURS OU CACHETS », la base congé et le salaire brut, tous deux « arrondi[s] à
//!   l'euro le plus proche » (la mention est sur le certificat) ;
//! - le N° CE (le numero du certificat), la date de signature.
//!
//! RIEN NE SE DEVINE (regle 1) : une rubrique qui ne se lit pas reste vide ; un certificat sans NIR, sans dates, sans
//! nombre de jours ou cachets ou sans salaire brut n'est pas repris, et le refus dit ce qui manque.

use std::path::Path;

use serde_json::Value;

use socle::date::Jour;
use socle::num::Decimal;

/// Deux mots sont sur le meme rang quand leurs milieux verticaux sont a moins de cela.
const TOLERANCE: f64 = 4.0;

/// Un mot de la page et sa boite.
#[derive(Clone, Debug)]
struct Mot {
    texte: String,
    x0: f64,
    cy: f64,
    y0: f64,
}

/// Les jetons d'une rubrique : minuscules, sans accents, tout ce qui n'est ni lettre ni chiffre lu comme un blanc
/// (« N° », « D'USAGE », « d’agrément » se lisent pareil sur les deux logiciels).
pub(crate) fn jetons(t: &str) -> Vec<String> {
    socle::texte::ascii_nfkd(&t.replace(['\u{2019}', '\u{2018}'], "'"))
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .map(String::from)
        .collect()
}

/// Les rubriques : (cle, jetons, celles qui peuvent la suivre sur son rang).
const RANGS: [(&str, &str, &[&str]); 13] = [
    ("siret", "n siret", &["compte", "ce"]),
    ("compte", "compte", &["ce"]),
    ("ce", "n ce", &[]),
    ("nir", "nir", &[]),
    ("nom", "nom de naissance", &["prenom"]),
    ("prenom", "prenom", &[]),
    ("usage", "nom d usage", &["pseudonyme"]),
    ("pseudonyme", "pseudonyme", &[]),
    ("emploi", "emploi", &[]),
    ("cadre", "cadre", &[]),
    ("debut", "debut", &["fin"]),
    ("base", "base conge", &["brut"]),
    ("date", "date", &[]),
];

/// Les rubriques qui ne commencent pas un rang : (cle, jetons).
const SUIVANTES: [(&str, &str); 2] = [("fin", "fin"), ("brut", "salaire brut")];

/// Un certificat d'emploi lu.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Certificat {
    pub page: usize,
    /// Quatorze chiffres.
    pub siret: Option<String>,
    /// Le numero d'adherent de l'employeur, ecrit comme la fiche le garde (`documents::conges_spectacles::forme_adherent`).
    pub compte: Option<String>,
    /// La raison sociale, imprimee sur le rang du NIR.
    pub employeur: Option<String>,
    /// Le N° CE.
    pub numero: Option<String>,
    /// Les treize caracteres du NIR, et sa cle s'il l'imprime.
    pub nir: Option<String>,
    pub cle_nir: Option<String>,
    pub nom: Option<String>,
    pub prenom: Option<String>,
    pub nom_usage: Option<String>,
    pub pseudonyme: Option<String>,
    /// Le nom tel que le bloc d'adresse l'ecrit, puis les rangs de l'adresse.
    pub nom_complet: Option<String>,
    pub adresse: Vec<String>,
    pub emploi: Option<String>,
    /// « OUI » ou « NON ».
    pub cadre: Option<bool>,
    pub debut: Option<Jour>,
    pub fin: Option<Jour>,
    /// Le « NB JOURS OU CACHETS ».
    pub nombre: Option<i64>,
    pub base_conge: Option<Decimal>,
    pub brut: Option<Decimal>,
    /// La date de signature.
    pub signe_le: Option<Jour>,
}

impl Certificat {
    /// Ce qui manque pour que le certificat atteste un contrat ; `None` : rien.
    pub fn manque(&self) -> Option<String> {
        let mut m: Vec<&str> = Vec::new();
        if self.nir.is_none() {
            m.push("le NIR");
        }
        if self.debut.is_none() || self.fin.is_none() {
            m.push("les dates de travail");
        }
        if self.nombre.is_none() {
            m.push("le nombre de jours ou cachets");
        }
        if self.brut.is_none() {
            m.push("le salaire brut");
        }
        (!m.is_empty()).then(|| format!("certificat Congés Spectacles dont {} ne se lit pas", m.join(", ")))
    }

    /// Ce qu'il dit du salarie, sous la forme qu'un bulletin lu donne (pour caviarder ce qui partirait chez l'editeur).
    pub fn salarie(&self) -> crate::import::bulletin_lu::Salarie {
        crate::import::bulletin_lu::Salarie {
            nom_complet: self.nom_complet.clone(),
            nom: self.nom.clone(),
            prenom: self.prenom.clone(),
            nir: self.nir.as_ref().map(|n| format!("{}{}", n, self.cle_nir.clone().unwrap_or_default())),
            adresse: self.adresse.clone(),
            emploi: self.emploi.clone(),
            ..Default::default()
        }
    }
}

/// Une page de certificat : ce qui s'en lit, ou pourquoi ce n'est pas repris (et ce qui s'en est lu quand meme).
#[derive(Clone, Debug)]
pub struct PageCertificat {
    pub page: usize,
    pub resultat: Result<Certificat, (String, Certificat)>,
}

/// Les pages du PDF qui sont des certificats d'emploi Congés Spectacles, lues. Les autres pages n'y sont pas.
pub fn lire(chemin: &Path) -> Result<Vec<PageCertificat>, String> {
    let (texte, _) = fabrique_pdf::inspection::inspecter(&chemin.to_string_lossy()).map_err(|r| format!("PDF que la fabrique n'ouvre pas : {}", r.0))?;
    let releve: Value = serde_json::from_str(&texte).map_err(|e| format!("releve du PDF illisible ({})", e))?;
    let pages = releve.get("pages").and_then(Value::as_array).cloned().unwrap_or_default();
    Ok(pages.iter().enumerate().filter_map(|(i, p)| lire_page(p, i + 1)).collect())
}

fn mots_de(p: &Value) -> Vec<Mot> {
    let mut mots = Vec::new();
    for m in p.get("mots").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]) {
        let texte = m.get("texte").and_then(Value::as_str).unwrap_or("").trim().to_string();
        let r: Vec<f64> = m.get("rect").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_f64).collect()).unwrap_or_default();
        if texte.is_empty() || r.len() != 4 {
            continue;
        }
        mots.push(Mot { texte, x0: r[0], cy: (r[1] + r[3]) / 2.0, y0: r[1] });
    }
    mots
}

/// Les mots en rangs, de haut en bas, chaque rang de gauche a droite.
fn en_rangs(mut mots: Vec<Mot>) -> Vec<Vec<Mot>> {
    mots.sort_by(|a, b| a.cy.total_cmp(&b.cy).then(a.x0.total_cmp(&b.x0)));
    let mut rangs: Vec<Vec<Mot>> = Vec::new();
    for m in mots {
        match rangs.last_mut() {
            Some(r) if (m.cy - r[0].cy).abs() <= TOLERANCE => r.push(m),
            _ => rangs.push(vec![m]),
        }
    }
    for r in rangs.iter_mut() {
        r.sort_by(|a, b| a.x0.total_cmp(&b.x0));
    }
    rangs
}

/// Un rang en jetons, chacun avec le mot d'ou il vient.
fn jetons_du_rang(r: &[Mot]) -> Vec<(String, usize)> {
    r.iter().enumerate().flat_map(|(i, m)| jetons(&m.texte).into_iter().map(move |j| (j, i))).collect()
}

/// La rubrique `phrase` commence-t-elle au jeton `k` ?
fn rubrique_en(js: &[(String, usize)], k: usize, phrase: &str) -> Option<usize> {
    let p: Vec<&str> = phrase.split(' ').collect();
    (k + p.len() <= js.len() && p.iter().enumerate().all(|(i, x)| js[k + i].0 == *x)).then_some(p.len())
}

/// Le rang contient-il ces jetons a la suite ?
fn contient(js: &[(String, usize)], phrase: &str) -> bool {
    (0..js.len()).any(|k| rubrique_en(js, k, phrase).is_some())
}

/// Les valeurs des rubriques d'un rang : la premiere rubrique qui commence un rang, puis celles qui peuvent la suivre.
/// Une valeur : les mots entre sa rubrique et la suivante, tels qu'imprimes.
fn valeurs(r: &[Mot]) -> Vec<(&'static str, Vec<String>)> {
    let js = jetons_du_rang(r);
    let mut out: Vec<(&'static str, Vec<String>)> = Vec::new();
    // (cle, premier jeton de la valeur, mot de la rubrique)
    let mut bornes: Vec<(&'static str, usize, usize)> = Vec::new();
    let mut k = 0;
    let mut suivantes: Vec<&str> = Vec::new();
    while k < js.len() {
        let trouve = if bornes.is_empty() {
            RANGS.iter().find_map(|(c, p, s)| rubrique_en(&js, k, p).map(|n| (*c, n, s.to_vec())))
        } else {
            RANGS
                .iter()
                .map(|(c, p, s)| (*c, *p, s.to_vec()))
                .chain(SUIVANTES.iter().map(|(c, p)| (*c, *p, Vec::new())))
                .filter(|(c, _, _)| suivantes.contains(c))
                .find_map(|(c, p, s)| rubrique_en(&js, k, p).map(|n| (c, n, s)))
        };
        match trouve {
            // Une rubrique commence un mot : « FIN » ne se lit pas dans la valeur « 14/03/2025 », ni au milieu d'un mot.
            Some((c, n, s)) if k == 0 || js[k - 1].1 != js[k].1 => {
                bornes.push((c, k + n, js[k].1));
                suivantes = s;
                k += n;
            }
            _ => k += 1,
        }
    }
    for (i, (c, debut, _)) in bornes.iter().enumerate() {
        let fin_jeton = bornes.get(i + 1).map_or(js.len(), |b| b.1 - rubrique_longueur(b.0));
        let premier = js.get(*debut).map(|j| j.1);
        let dernier = if fin_jeton > *debut { js.get(fin_jeton - 1).map(|j| j.1) } else { None };
        let mots = match (premier, dernier) {
            (Some(a), Some(b)) if b >= a => r[a..=b].iter().map(|m| m.texte.clone()).collect(),
            _ => Vec::new(),
        };
        out.push((c, mots));
    }
    out
}

fn rubrique_longueur(cle: &str) -> usize {
    RANGS.iter().map(|(c, p, _)| (*c, *p)).chain(SUIVANTES.iter().copied()).find(|(c, _)| *c == cle).map_or(0, |(_, p)| p.split(' ').count())
}

/// Une date `JJ/MM/AAAA` (un mot) ou `JJ MM AAAA` (trois), en tete de `mots` : la date, et le nombre de mots pris.
pub(crate) fn date_en_tete(mots: &[String]) -> Option<(Jour, usize)> {
    if let Some(d) = mots.first().and_then(|m| crate::import::bulletin_lu::lire_date(m)) {
        return Some((d, 1));
    }
    let [j, m, a, ..] = mots else { return None };
    let ok = |t: &str, n: usize| t.len() == n && t.chars().all(|c| c.is_ascii_digit());
    if !(ok(j, 2) || ok(j, 1)) || !(ok(m, 2) || ok(m, 1)) || !ok(a, 4) {
        return None;
    }
    Jour::nouveau(a.parse().ok()?, m.parse().ok()?, j.parse().ok()?).map(|d| (d, 3))
}

fn texte(mots: &[String]) -> Option<String> {
    let t = mots.join(" ").trim().to_string();
    (!t.is_empty()).then_some(t)
}

/// Un montant arrondi a l'euro (« 1 234 », « 480 ») : des chiffres, des blancs de milliers.
fn montant(mots: &[String]) -> Option<Decimal> {
    let t: String = mots.concat();
    (!t.is_empty() && t.chars().all(|c| c.is_ascii_digit() || c == ',' || c == '.')).then(|| crate::import::bulletin_lu::lire_nombre(&t)).flatten()
}

/// Le NIR imprime en tete du rang : treize caracteres (chiffres, et 2A ou 2B pour un ne en Corse), puis sa cle de deux
/// chiffres ; la suite du rang est la raison sociale de l'employeur. (NIR, cle, nombre de mots pris.)
pub(crate) fn nir_en_tete(mots: &[String]) -> Option<(String, Option<String>, usize)> {
    let mut tout = String::new();
    let mut pris = 0;
    for m in mots {
        let t: String = m.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_uppercase();
        let chiffres = t.chars().all(|c| c.is_ascii_digit());
        let corse = t.chars().any(|c| c.is_ascii_digit()) && t.chars().all(|c| c.is_ascii_digit() || c == 'A' || c == 'B');
        if t.is_empty() || !(chiffres || corse) || tout.len() + t.len() > 15 {
            break;
        }
        tout.push_str(&t);
        pris += 1;
        if tout.len() == 15 {
            break;
        }
    }
    match tout.len() {
        13 => Some((tout, None, pris)),
        15 => Some((tout[..13].to_string(), Some(tout[13..].to_string()), pris)),
        _ => None,
    }
}

/// Une page : `None` si ce n'est pas un certificat d'emploi Congés Spectacles (son titre, la Caisse, ses rubriques de
/// contrat).
fn lire_page(p: &Value, page: usize) -> Option<PageCertificat> {
    let rangs = en_rangs(mots_de(p));
    let js: Vec<Vec<(String, usize)>> = rangs.iter().map(|r| jetons_du_rang(r)).collect();
    let titre = js.iter().position(|j| contient(j, "certificat d emploi"))?;
    if !js.iter().any(|j| contient(j, "conges spectacles")) || !js.iter().any(|j| contient(j, "nb jours ou cachets")) || !js.iter().any(|j| contient(j, "salaire brut")) {
        return None;
    }
    let mut c = Certificat { page, ..Default::default() };
    let mut fautes: Vec<String> = Vec::new();
    // Le bloc d'adresse : les rangs au-dessus du titre, le premier est le nom.
    let titre_y = rangs[titre].iter().map(|m| m.y0).fold(f64::MAX, f64::min);
    let bloc: Vec<String> = rangs[..titre].iter().filter(|r| r.iter().all(|m| m.y0 < titre_y)).map(|r| r.iter().map(|m| m.texte.as_str()).collect::<Vec<_>>().join(" ")).collect();
    if let Some((nom, reste)) = bloc.split_first() {
        c.nom_complet = Some(nom.clone());
        c.adresse = reste.to_vec();
    }
    for r in &rangs[titre..] {
        for (cle, v) in valeurs(r) {
            match cle {
                "siret" => {
                    let s: String = v.concat().chars().filter(char::is_ascii_digit).collect();
                    c.siret = (s.len() == 14).then_some(s);
                }
                "compte" => c.compte = texte(&v).map(|t| documents::conges_spectacles::forme_adherent(&t)),
                "ce" => c.numero = texte(&v),
                "nir" => match nir_en_tete(&v) {
                    Some((nir, cle, pris)) => {
                        if let Some(k) = &cle {
                            if declarations::champs::cle_nir(&nir).as_deref() != Some(k.as_str()) {
                                fautes.push("la clé du NIR imprimé ne correspond pas à son numéro".into());
                                continue;
                            }
                        }
                        c.nir = Some(nir);
                        c.cle_nir = cle;
                        c.employeur = texte(&v[pris..]);
                    }
                    None => c.employeur = texte(&v),
                },
                "nom" => c.nom = texte(&v),
                "prenom" => c.prenom = texte(&v),
                "usage" => c.nom_usage = texte(&v),
                "pseudonyme" => c.pseudonyme = texte(&v),
                "emploi" => c.emploi = texte(&v),
                "cadre" => {
                    c.cadre = match jetons(&v.join(" ")).first().map(String::as_str) {
                        Some("oui") => Some(true),
                        Some("non") => Some(false),
                        _ => None,
                    }
                }
                "debut" => c.debut = date_en_tete(&v).map(|x| x.0),
                "fin" => {
                    if let Some((d, n)) = date_en_tete(&v) {
                        c.fin = Some(d);
                        let reste = &v[n..];
                        if let [x] = reste {
                            c.nombre = x.parse::<i64>().ok().filter(|n| *n >= 0);
                        }
                    }
                }
                "base" => c.base_conge = montant(&v),
                "brut" => c.brut = montant(&v),
                "date" => c.signe_le = v.first().and_then(|m| crate::import::bulletin_lu::lire_date(m)),
                _ => {}
            }
        }
    }
    if let (Some(d), Some(f)) = (c.debut, c.fin) {
        if f < d {
            fautes.push("la fin du travail précède son début".into());
        }
    }
    let resultat = match (fautes.first(), c.manque()) {
        (Some(f), _) => Err((format!("certificat Congés Spectacles : {}", f), c)),
        (None, Some(m)) => Err((m, c)),
        (None, None) => Ok(c),
    };
    Some(PageCertificat { page, resultat })
}

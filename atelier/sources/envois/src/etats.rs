//! CE QUE DIT LE JOURNAL DES ENVOIS : les etats du mois, les resumes, la repetition d'un depot, le rattachement des
//! retours, les contrats partis, les signatures attendues, les embauches declarees. Rien n'y calcule de paie. Les lignes
//! se lisent dans leur forme (`journal::Ligne`) ; un contrat se designe par sa `Paire` (la cle du salarie, le numero du
//! contrat, ou `""`).
//!
//! `Dico`, `items` et les lectures de `documents::valeur` qu'il reexporte servent d'autres modules qui lisent encore des
//! `Value` : aucun ne lit le journal.

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use documents::valeur::vrai;
use socle::mois::Mois;
use crate::journal::{chemin, Rangement};
use crate::ligne::{Acte, AvenantParti, ContratAttendu, Ligne, Mode, PersonneCertifiee, RetourContrat, RetourOrganisme, Verdict};
use crate::retours;
use socle::echec::{erreur, Echec, R};
use documents::valeur::{nom_type, nombre, valeur_en_texte};
pub use documents::valeur::{get, getd, objet_vide, ou, texte_vide};
use crate::tables_journal::ALNUM;

pub const DSN: &str = "dsn";
pub const DPAE: &str = "dpae";
pub const DOCUMENTS: &str = "documents";
pub const SIGNATURE: &str = "contrat-signature";
const NATURE_CERTIFICAT_DPAE: &str = "41";

/// Les retours du canal DPAE.
pub const NATURES_DPAE: [(&str, &str); 4] = [
    ("10", "avis de rejet du depot"),
    ("11", "certificat de conformite / bilan d'anomalies"),
    ("20", "bilan d'identification des salaries"),
    ("41", "certificat de conformite des DPAE"),
];

pub fn est(v: &Value, s: &str) -> bool {
    v.as_str() == Some(s)
}

/// `for x in v` : une liste, les cles d'un dictionnaire, les caracteres d'un texte.
pub fn items(v: &Value) -> R<Vec<Cow<'_, Value>>> {
    match v {
        Value::Array(a) => Ok(a.iter().map(Cow::Borrowed).collect()),
        Value::Object(o) => Ok(o.keys().map(|k| Cow::Owned(Value::String(k.clone()))).collect()),
        Value::String(s) => Ok(s.chars().map(|c| Cow::Owned(Value::String(c.to_string()))).collect()),
        _ => Err(erreur("TypeError", socle::fichiers::attendu("une liste", nom_type(v)))),
    }
}

pub fn s(t: &str) -> Value {
    Value::String(t.to_string())
}

// ---------------------------------------------------- les dictionnaires

/// La cle de hachage d'une valeur : `1`, `1.0` et `true` ne font qu'une ;
/// une liste ou un dictionnaire ne se hache pas.
fn cle_de_hachage(v: &Value) -> R<String> {
    match v {
        Value::Null => Ok("N".into()),
        Value::String(t) => Ok(format!("s{}:{}", t.len(), t)),
        Value::Array(_) => Err(erreur("TypeError", socle::fichiers::pas_une_cle("list"))),
        Value::Object(_) => Err(erreur("TypeError", socle::fichiers::pas_une_cle("dict"))),
        autre => {
            let x = nombre(autre).unwrap_or(0.0);
            Ok(format!("n{:?}", if x == 0.0 { 0.0 } else { x }))
        }
    }
}

/// Une table (ou un ensemble) a cle composee, dans l'ordre d'insertion : reposer
/// une cle change la valeur et garde la place.
pub struct Dico<V> {
    pub index: HashMap<String, usize>,
    pub items: Vec<(Vec<Value>, V)>,
}

impl<V> Dico<V> {
    pub fn nouveau() -> Self {
        Dico { index: HashMap::new(), items: Vec::new() }
    }

    pub fn cle(k: &[Value]) -> R<String> {
        let mut t = String::new();
        for x in k {
            t.push_str(&cle_de_hachage(x)?);
            t.push('\u{1f}');
        }
        Ok(t)
    }

    pub fn get(&self, k: &[Value]) -> R<Option<&V>> {
        Ok(self.index.get(&Self::cle(k)?).map(|&i| &self.items[i].1))
    }

    pub fn get_mut(&mut self, k: &[Value]) -> R<Option<&mut V>> {
        let i = self.index.get(&Self::cle(k)?).copied();
        Ok(i.map(move |i| &mut self.items[i].1))
    }

    pub fn contient(&self, k: &[Value]) -> R<bool> {
        Ok(self.index.contains_key(&Self::cle(k)?))
    }

    pub fn poser(&mut self, k: Vec<Value>, v: V) -> R<()> {
        let c = Self::cle(&k)?;
        match self.index.get(&c) {
            Some(&i) => self.items[i].1 = v,
            None => {
                self.index.insert(c, self.items.len());
                self.items.push((k, v));
            }
        }
        Ok(())
    }

    pub fn sortie(&self, f: impl Fn(&V) -> Value) -> Value {
        Value::Array(
            self.items
                .iter()
                .map(|(k, v)| {
                    let mut l = k.clone();
                    l.push(f(v));
                    Value::Array(l)
                })
                .collect(),
        )
    }

    pub fn cles(&self) -> Value {
        Value::Array(self.items.iter().map(|(k, _)| Value::Array(k.clone())).collect())
    }
}

/// UN CONTRAT, tel que le journal le designe : la cle du salarie et le numero du contrat (`""` sans numero).
pub type Paire = (String, String);

fn paire(salarie: &str, numero: &str) -> Paire {
    (salarie.to_string(), numero.to_string())
}

/// L'indice, dans `paies`, de la paie du contrat n° `numero` de ce salarie.
pub fn paie_du_contrat(paies: &[engagement::calcul::Paie], salarie: &str, numero: &str) -> R<Option<usize>> {
    for (i, x) in paies.iter().enumerate() {
        if x.salarie.id == salarie && x.engagement.numero.as_deref() == Some(numero) && x.engagement.nat().map_err(Echec::from)?.modele_contrat().is_some() {
            return Ok(Some(i));
        }
    }
    Ok(None)
}

// ------------------------------------------------------------------ les etats

fn erreur_io(e: &std::io::Error, p: &Path) -> Echec {
    let classe = match e.kind() {
        std::io::ErrorKind::NotFound => "FileNotFoundError",
        std::io::ErrorKind::PermissionDenied => "PermissionError",
        _ => "OSError",
    };
    erreur(classe, socle::echec::du_disque(p, e))
}

// ------------------------------------------------------------ les embauches

/// Les embauches qu'une DPAE reelle declare, par (salarie, numero du contrat) : partie, et pas refusee (`refuse_par_un_retour`),
/// et quand ; une DPAE refusee se redeclare. Une embauche sans numero ne se relie a rien.
pub fn embauches_par_depot(lignes: &[Ligne]) -> HashMap<Paire, String> {
    let mut d = HashMap::new();
    for l in lignes {
        let Some(e) = l.dpae().filter(|e| e.mode == Mode::Reel && e.accepte && !refuse_par_un_retour(l)) else { continue };
        for x in &e.detail.embauches {
            if let Some(n) = &x.numero {
                d.insert(paire(&x.salarie, n), e.quand.clone());
            }
        }
    }
    d
}

/// Les lignes d'une DSN de l'etablissement `nic` (son NIC ; `None` : `principal`, le NIC de la fiche entreprise).
pub fn de_l_etablissement<'a>(lignes: impl IntoIterator<Item = &'a Ligne>, nic: Option<&str>, principal: &str) -> Vec<&'a Ligne> {
    let voulu = nic.filter(|n| !n.is_empty()).unwrap_or(principal);
    lignes.into_iter().filter(|l| l.etablissement().unwrap_or(principal) == voulu).collect()
}

/// CE QU'EST DEVENUE LA DSN REELLE D'UN MOIS (la derniere acceptee de son etablissement) : passee (le bilan OK, ou le
/// verdict qu'une ligne reprise garde), refusee, ses anomalies, l'attestation employeur recue.
#[derive(Debug, Clone, serde::Serialize)]
pub struct EtatDsnDuMois {
    pub quand: String,
    pub fichier: Option<String>,
    pub flux: String,
    pub passee: bool,
    pub refusee: bool,
    pub anomalies: Vec<String>,
    pub aer_recue: bool,
}

pub fn etat_dsn_du_mois(lignes: &[Ligne], mois: Mois, nic: Option<&str>, principal: &str) -> Option<EtatDsnDuMois> {
    // Un signalement porte le mois de son evenement : ce n'est pas la DSN du mois.
    let du_mois = crate::journal::filtrer(lignes, Some(Acte::Dsn), Some(mois), true).filter(|l| !l.signalement());
    let l = de_l_etablissement(du_mois, nic, principal).into_iter().filter(|l| l.accepte()).last()?;
    let e = l.dsn()?;
    let (mut bilan_ok, mut aer, mut anomalies) = (false, false, Vec::new());
    for r in &e.retours {
        bilan_ok |= r.nature == "11" && r.verdict() == "OK";
        aer |= r.nature == "46";
        if r.ko() {
            anomalies.extend(r.anomalies().iter().cloned());
        }
    }
    // UN DEPOT REPRIS DES RETOURS (`reprise_journal`) : sa piece EST le bilan de la plate-forme, et la ligne
    // en garde le verdict (`detail.etat_declaration`) -- tant qu'aucun bilan n'est range en face, c'est lui qui dit.
    let sans_bilan = !e.retours.iter().any(|r| r.nature == "11");
    let repris_ok = sans_bilan && e.detail.reconstruit && e.detail.etat_declaration.as_deref().unwrap_or("").eq_ignore_ascii_case("OK");
    let refusee = refuse_par_un_retour(l);
    Some(EtatDsnDuMois { quand: e.quand.clone(), fichier: e.fichier.clone(), flux: e.flux.clone(), passee: (bilan_ok || repris_ok) && !refusee, refusee, anomalies, aer_recue: aer })
}

/// Deux graphies du meme nom (la cle d'une fiche, le nom d'un certificat) : leurs lettres et chiffres, en capitales.
fn meme_nom(cle: &str, nom: &str) -> bool {
    let plat = |t: &str| -> String { t.to_uppercase().chars().filter(|&c| lettre_ou_chiffre(c)).collect() };
    let a = plat(cle);
    !a.is_empty() && a == plat(nom)
}

/// Vrai pour une lettre ou un chiffre Unicode (lettres Lu, Ll, Lt, Lm, Lo et Numeric_Type, d'apres la table deposee : `tables_journal`).
pub fn lettre_ou_chiffre(c: char) -> bool {
    let c = c as u32;
    ALNUM
        .binary_search_by(|&(a, b)| {
            if c < a {
                std::cmp::Ordering::Greater
            } else if c > b {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Equal
            }
        })
        .is_ok()
}

/// Ce que les retours d'une DPAE partie en disent : `(passee, refusee)`. Passee : le certificat (nature 41) ou un bilan OK ;
/// refusee : un retour KO, sans certificat ni bilan OK.
fn verdict_dpae(retours: &[RetourOrganisme]) -> (bool, bool) {
    let (mut passee, mut ko) = (false, false);
    for r in retours {
        passee |= r.nature == NATURE_CERTIFICAT_DPAE || (r.nature == "11" && r.verdict() == "OK");
        ko |= r.ko();
    }
    (passee, ko && !passee)
}

/// CE QU'EST DEVENUE UNE EMBAUCHE DECLAREE (la derniere DPAE reelle partie qui la porte) : quand, son fichier, sa fin,
/// passee, refusee, les anomalies, les retours, et ce que le certificat de l'Urssaf en dit (le dossier, deja existante).
#[derive(Debug, Clone, serde::Serialize)]
pub struct EtatEmbauche {
    pub quand: String,
    pub fichier: Option<String>,
    pub fin: Option<String>,
    pub passee: bool,
    pub refusee: bool,
    pub anomalies: Vec<String>,
    pub retours: Vec<RetourOrganisme>,
    pub dossier: String,
    pub deja_existante: bool,
}

/// Ce que chaque DPAE reelle partie est devenue, par (salarie, numero du contrat).
pub fn etat_des_embauches(lignes: &[Ligne]) -> HashMap<Paire, EtatEmbauche> {
    let mut etat = HashMap::new();
    for l in lignes {
        let Some(e) = l.dpae().filter(|e| e.mode == Mode::Reel && e.accepte) else { continue };
        let (passee, refusee) = verdict_dpae(&e.retours);
        let anomalies: Vec<String> = e.retours.iter().filter(|r| r.ko()).flat_map(|r| r.anomalies().iter().cloned()).collect();
        let certifiees: Vec<&PersonneCertifiee> = e.retours.iter().filter(|r| r.nature == NATURE_CERTIFICAT_DPAE).flat_map(|r| r.personnes()).collect();
        for x in &e.detail.embauches {
            let Some(n) = &x.numero else { continue };
            // La derniere ligne du certificat pour ce nom et ce premier jour.
            let cert = certifiees.iter().rev().find(|c| c.embauche == x.debut && meme_nom(&x.salarie, &c.nom));
            etat.insert(
                paire(&x.salarie, n),
                EtatEmbauche {
                    quand: e.quand.clone(),
                    fichier: e.fichier.clone(),
                    fin: x.fin.clone(),
                    passee,
                    refusee,
                    anomalies: anomalies.clone(),
                    retours: e.retours.clone(),
                    dossier: cert.map(|c| c.dossier.clone()).unwrap_or_default(),
                    deja_existante: cert.is_some_and(|c| c.deja_existante),
                },
            );
        }
    }
    etat
}

// -------------------------------------------------------------- les contrats

/// Comment chaque contrat est parti la derniere fois : `imprime` (remis en main propre, `detail.main_propre`) ou
/// `courriel`. Un contrat imprime peut toujours partir par courriel : il part alors comme un envoi, non comme un rappel.
pub fn contrats_partis_par(lignes: &[Ligne]) -> HashMap<Paire, &'static str> {
    let mut d = HashMap::new();
    for e in lignes.iter().filter_map(Ligne::signature).filter(|e| e.accepte) {
        let par = if e.detail.main_propre { "imprime" } else { "courriel" };
        for c in &e.detail.contrats {
            d.insert(paire(&c.salarie, c.numero_dit()), par);
        }
    }
    d
}

/// CE QUI EST REVENU D'UN CONTRAT PARTI (signe, ou la mention retiree), sur la ligne de son envoi (`journal::rattacher`),
/// dans l'ordre ou c'est revenu.
pub fn revenus(l: &Ligne) -> &[RetourContrat] {
    l.signature().filter(|e| e.accepte).map_or(&[], |e| e.retours.as_slice())
}

/// Les contrats repartis apres leur signature sur une autre version de l'engagement que celle signee : ils attendent une
/// nouvelle signature. Un avenant n'en fait pas un.
pub fn renvoyes_depuis_la_signature(lignes: &[Ligne]) -> HashSet<Paire> {
    let mut envoye: HashMap<Paire, Option<u64>> = HashMap::new();
    let mut signe: HashMap<Paire, Option<u64>> = HashMap::new();
    let mut renvoye: HashSet<Paire> = HashSet::new();
    for l in lignes.iter().filter(|l| l.accepte()) {
        if let Some(e) = l.signature() {
            for c in &e.detail.contrats {
                let k = paire(&c.salarie, c.numero_dit());
                // La version de l'engagement sur laquelle ce contrat part (`journal::versions_des_contrats`).
                let v = e.versions.iter().flatten().find(|x| x.salarie == c.salarie && c.numero.as_ref() == Some(&x.numero)).map(|x| x.version);
                if c.est_avenant() {
                    envoye.insert(k, v);
                    continue;
                }
                if let Some(Some(deja)) = signe.get(&k) {
                    if v.is_some_and(|v| v != *deja) {
                        renvoye.insert(k.clone());
                    }
                }
                envoye.insert(k, v);
            }
        }
        for d in revenus(l).iter().filter(|d| d.recu) {
            let k = paire(&d.salarie, d.numero_dit());
            signe.insert(k.clone(), envoye.get(&k).copied().flatten());
            renvoye.remove(&k);
        }
    }
    renvoye
}

/// Les contrats dont le dernier mot est « recu » : (salarie, numero) -> quand.
pub fn contrats_recus(lignes: &[Ligne]) -> HashMap<Paire, String> {
    let mut etat: HashMap<Paire, Option<String>> = HashMap::new();
    for d in lignes.iter().flat_map(revenus) {
        // La note d'un avenant (signe, ou defait) ne dit rien du contrat lui-meme.
        if d.avenant.is_some() {
            continue;
        }
        etat.insert(paire(&d.salarie, d.numero_dit()), d.recu.then(|| d.quand.clone()).filter(|q| !q.is_empty()));
    }
    etat.into_iter().filter_map(|(k, v)| v.map(|v| (k, v))).collect()
}

/// UNE SIGNATURE ATTENDUE : la demande partie sous `code` (le mois, quand, a qui, sa piece, ce qu'elle est : un contrat ou
/// un avenant), dont un contrat au moins n'est pas revenu signe ; `rang` est sa ligne au journal, `envoyes` les
/// identifiants des messages partis (jamais pris pour une reponse).
#[derive(Debug, Clone, serde::Serialize)]
pub struct Attendue {
    pub code: String,
    pub mois: Option<Mois>,
    pub quand: String,
    pub salarie: droit::contexte::IdSalarie,
    pub adresse: String,
    pub fichier: String,
    pub acte: String,
    pub avenant: Option<i64>,
    pub rang: usize,
    pub apres_signature: bool,
    pub envoyes: Vec<String>,
    pub prenom: String,
    pub nom: String,
    pub contrats: Vec<ContratAttendu>,
}

pub fn signatures_attendues<'f>(fiche_de: impl Fn(&str) -> Option<&'f droit::fiche::ChampsSalarie>, lignes: &[Ligne], mois: Option<Mois>) -> Vec<Attendue> {
    // Par CONTRAT (salarie, numero) : une reponse ne releve que le contrat qu'elle signe (Retours Windows, B, suite).
    let recus = contrats_recus(lignes);
    let renvoyes = renvoyes_depuis_la_signature(lignes);
    let mut derniere_recue: HashMap<Paire, usize> = HashMap::new();
    for (i, l) in lignes.iter().enumerate() {
        for d in revenus(l).iter().filter(|d| d.recu) {
            derniere_recue.insert(paire(&d.salarie, d.numero_dit()), i);
        }
    }
    // Les reponses deja relevees, par envoi (son code) et par contrat ou avenant : une reponse ne se note qu'une fois.
    // La mention « signe » retiree a la main l'emporte sur la reponse deja lue ; seul un nouvel envoi (un autre code)
    // attend une nouvelle reponse.
    let releve = |code: &str, salarie: &str, numero: &str, avenant: Option<i64>| (code.to_string(), salarie.to_string(), numero.to_string(), avenant);
    let deja_releves: HashSet<(String, String, String, Option<i64>)> = lignes
        .iter()
        .flat_map(revenus)
        .filter(|d| d.recu && d.par_courriel())
        .filter_map(|d| d.code.as_deref().filter(|c| !c.is_empty()).map(|c| releve(c, &d.salarie, d.numero_dit(), d.avenant)))
        .collect();
    let mut par_code: HashMap<String, usize> = HashMap::new();
    let mut demandes: Vec<Attendue> = Vec::new();
    for (i, l) in lignes.iter().enumerate() {
        let Some(e) = l.signature().filter(|e| e.accepte) else { continue };
        if mois.is_some_and(|m| e.mois != Some(m)) {
            continue;
        }
        for c in &e.detail.contrats {
            let Some(code) = c.code.as_deref().filter(|c| !c.is_empty()) else { continue };
            let k = paire(&c.salarie, c.numero_dit());
            let apres_signature = matches!(c.acte.as_deref(), Some("avenant") | Some("remplace")) || renvoyes.contains(&k);
            let n = *par_code.entry(code.to_string()).or_insert_with(|| {
                let fiche = fiche_de(c.salarie.as_str());
                let prenom = fiche.map(|f| if f.prenom_affichage.texte().is_empty() { f.prenom.texte().to_string() } else { f.prenom_affichage.texte().to_string() }).unwrap_or_default();
                demandes.push(Attendue {
                    code: code.to_string(),
                    mois: e.mois,
                    quand: e.quand.clone(),
                    salarie: c.salarie.clone(),
                    adresse: c.adresse.clone().unwrap_or_default(),
                    fichier: c.fichier.clone(),
                    acte: c.acte.clone().unwrap_or_else(|| "contrat".into()),
                    avenant: c.avenant,
                    rang: i,
                    apres_signature: false,
                    envoyes: Vec::new(),
                    nom: fiche.and_then(|f| f.nomme()).unwrap_or_else(|| c.salarie.to_string()),
                    prenom,
                    contrats: Vec::new(),
                });
                demandes.len() - 1
            });
            let d = &mut demandes[n];
            if let Some(m) = c.message.as_deref().filter(|m| !m.is_empty()) {
                if !d.envoyes.iter().any(|x| x == m) {
                    d.envoyes.push(m.to_string());
                }
            }
            d.apres_signature |= apres_signature;
            d.contrats.push(ContratAttendu {
                salarie: c.salarie.clone(),
                debut: c.debut.clone(),
                fin: c.fin.clone(),
                numero: c.numero.clone(),
                spectacle: c.spectacle.clone().unwrap_or_default(),
                fichier: c.fichier.clone(),
                avenant: c.avenant,
                mois: c.mois,
            });
        }
    }
    demandes
        .into_iter()
        .filter(|d| {
            d.contrats.iter().any(|c| {
                let k = paire(&c.salarie, c.numero_dit());
                let revenu = deja_releves.contains(&releve(&d.code, &c.salarie, c.numero_dit(), c.avenant))
                    || if d.apres_signature { derniere_recue.get(&k).is_some_and(|&rang| rang >= d.rang) } else { recus.contains_key(&k) };
                !revenu
            })
        })
        .collect()
}

/// Les avenants de chaque contrat (sa `Paire`), dans l'ordre du journal : chacun son numero, et `signe` (le jour ou il
/// est note revenu signe, ou rien).
pub fn avenants_par_contrat(lignes: &[Ligne]) -> HashMap<Paire, Vec<AvenantParti>> {
    let mut out: HashMap<Paire, Vec<AvenantParti>> = HashMap::new();
    for l in lignes.iter().filter(|l| l.accepte()) {
        if let Some(e) = l.signature() {
            for c in e.detail.contrats.iter().filter(|c| c.est_avenant()) {
                let liste = out.entry(paire(&c.salarie, c.numero_dit())).or_default();
                liste.retain(|a| a.numero != c.avenant);
                liste.push(AvenantParti {
                    numero: c.avenant,
                    quand: e.quand.clone(),
                    mois: c.mois,
                    fichier: c.fichier.clone(),
                    code: c.code.clone(),
                    contrat: c.numero.clone(),
                    effet: c.effet.clone(),
                    modifications: c.modifications.clone().unwrap_or_default(),
                    signe: None,
                });
            }
        }
        for d in revenus(l) {
            // Le retour d'UN avenant ne vaut que pour lui, et se defait (`recu: false`).
            let Some(un) = d.avenant else { continue };
            if let Some(liste) = out.get_mut(&paire(&d.salarie, d.numero_dit())) {
                for a in liste.iter_mut().filter(|a| a.numero == Some(un)) {
                    if !d.recu {
                        a.signe = None;
                    } else if a.signe.is_none() {
                        a.signe = Some(d.quand.clone());
                    }
                }
            }
        }
    }
    out
}

/// CE QUE LES DOCUMENTS DU MOIS DISAIENT, par salarie : le resume de sa paie (`envoi::resume_de`) dans la derniere ligne
/// d'envoi de documents qui l'a servi (un envoi coupe en route ne porte que ceux qu'il a servis). Qu'ils soient partis,
/// c'est l'engagement qui le sait (`journal::departs`) ; ceci n'en dit que le contenu.
pub fn resumes_des_documents(lignes: &[Ligne], mois: Mois) -> HashMap<String, Option<crate::envoi::ResumeDePaie>> {
    let mut out = HashMap::new();
    for e in crate::journal::filtrer(lignes, Some(Acte::Documents), Some(mois), false).filter_map(Ligne::documents) {
        for s in &e.detail.salaries {
            out.insert(s.cle.to_string(), s.resume.clone());
        }
    }
    out
}

/// Les flux des depots de test, chacun une fois.
pub fn flux_de_test(lignes: &[Ligne]) -> Vec<String> {
    let mut d: Vec<String> = Vec::new();
    for l in lignes.iter().filter(|l| l.mode() == Mode::Test && !l.flux().is_empty()) {
        if !d.iter().any(|f| f == l.flux()) {
            d.push(l.flux().to_string());
        }
    }
    d
}

// ------------------------------------------------------------------- resume

/// Un horodatage ISO a la francaise, ou vide.
pub fn quand_lisible(iso: &str) -> String {
    let c: Vec<char> = iso.chars().collect();
    if c.len() < 10 || c[4] != '-' || c[7] != '-' {
        return iso.to_string();
    }
    let tranche = |a: usize, b: usize| c[a..b].iter().collect::<String>();
    let jour = format!("{}/{}/{}", tranche(8, 10), tranche(5, 7), tranche(0, 4));
    if c.len() >= 16 {
        format!("{} à {}", jour, tranche(11, 16))
    } else {
        jour
    }
}

fn pluriel(n: usize) -> &'static str {
    if n > 1 {
        "s"
    } else {
        ""
    }
}

/// LA PHRASE D'UN ENVOI DU JOURNAL : quand il est parti, et ce qui en est revenu. Un contrat est revenu signe quand le
/// journal le note recu (`contrats_recus`) ; un avenant, quand sa propre note le dit (`avenants_par_contrat`) -- les
/// memes lectures que l'etat de l'engagement.
pub fn resume(ligne: Option<&Ligne>, recus: &HashMap<Paire, String>, avenants: &HashMap<Paire, Vec<AvenantParti>>) -> String {
    let Some(ligne) = ligne else { return "pas encore envoyé".into() };
    let mut quand = quand_lisible(ligne.quand());
    if ligne.mode() == Mode::Test {
        quand = format!("en TEST le {}", quand);
    }
    if !ligne.accepte() {
        return format!("envoi du {} refusé au dépôt — {}", quand, ligne.reponse());
    }
    let parti = if ligne.main_propre() { "imprimé" } else { "envoyé" };
    let le = if quand.starts_with("en TEST") { quand.clone() } else { format!("le {}", quand) };
    let prefixe = format!("{} {}", parti, le);
    match ligne {
        Ligne::ContratSignature(e) => {
            let n = e.detail.contrats.len();
            let revenus = e
                .detail
                .contrats
                .iter()
                .filter(|c| {
                    let k = paire(&c.salarie, c.numero_dit());
                    if c.est_avenant() {
                        avenants.get(&k).is_some_and(|l| l.iter().any(|a| a.numero == c.avenant && a.signe.is_some()))
                    } else {
                        recus.contains_key(&k)
                    }
                })
                .count();
            if n == 0 {
                prefixe
            } else if revenus == n {
                format!("{} — {}", prefixe, if n == 1 { "revenu signé".to_string() } else { format!("tous revenus signés ({})", n) })
            } else if revenus > 0 {
                format!("{} — {} contrat{} sur {} revenu{} signé{}", prefixe, revenus, pluriel(revenus), n, pluriel(revenus), pluriel(revenus))
            } else {
                format!("{} — pas encore revenu signé", prefixe)
            }
        }
        Ligne::Documents(_) | Ligne::ContratContresigne(_) | Ligne::RegularisationCasc(_) => prefixe,
        Ligne::DossierSalarie(_) => format!("{} — pas encore de retour", prefixe),
        Ligne::FicheDemande(e) if e.retours.is_empty() => format!("{} — pas encore de retour", prefixe),
        Ligne::FicheDemande(_) => format!("{} — retour reçu", prefixe),
        Ligne::Dsn(e) => resume_des_retours(&prefixe, &le, &e.retours),
        Ligne::Dpae(e) => resume_des_retours(&prefixe, &le, &e.retours),
    }
}

/// Ce que les comptes rendus d'une DSN ou d'une DPAE disent, a la suite de `prefixe` (`le` : quand elle est partie).
fn resume_des_retours(prefixe: &str, le: &str, retours: &[RetourOrganisme]) -> String {
    let Some(dernier) = retours.last() else { return format!("{} — pas encore de retour", prefixe) };
    let refus: Vec<&RetourOrganisme> = retours.iter().filter(|r| r.ko()).collect();
    let certificat = retours.iter().rev().find(|r| r.nature == NATURE_CERTIFICAT_DPAE);
    if !refus.is_empty() && certificat.is_none() {
        let mut motifs: Vec<&str> = Vec::new();
        for a in refus.iter().flat_map(|r| r.anomalies()) {
            if !motifs.contains(&a.as_str()) {
                motifs.push(a);
            }
        }
        let suite = if motifs.is_empty() { String::new() } else { format!(" : {}", motifs.join(" ; ")) };
        return format!("envoyé {} — REFUSÉ{}", le, suite);
    }
    if let Some(c) = certificat {
        let dossiers: Vec<&str> = c.personnes().iter().filter(|l| l.enregistree && !l.dossier.is_empty()).map(|l| l.dossier.as_str()).collect();
        let existantes: Vec<&str> = c.personnes().iter().filter(|l| l.deja_existante).map(|l| l.nom.as_str()).collect();
        let mut fin = " — certificat de conformité reçu : passée".to_string();
        if !dossiers.is_empty() {
            fin.push_str(&format!(" (dossier{} {})", pluriel(dossiers.len()), dossiers.join(", ")));
        }
        if !existantes.is_empty() {
            fin.push_str(&format!(" ; déjà existante à l'Urssaf, rien de créé : {}", existantes.join(", ")));
        }
        return format!("{}{}", prefixe, fin);
    }
    if retours.iter().any(|r| r.nature == "11" && r.verdict() == "OK") {
        let apres = retours.iter().filter(|r| r.nature != "10" && r.nature != "11").count();
        let suite = if apres > 0 { format!(" ; {} compte{}-rendu{} d'organismes reçu{}", apres, pluriel(apres), pluriel(apres), pluriel(apres)) } else { String::new() };
        return format!("{} — bilan OK, aucune anomalie{}", prefixe, suite);
    }
    let choix = [dernier.libelle.as_str(), dernier.nature.as_str()].into_iter().find(|t| !t.is_empty()).unwrap_or("retour reçu");
    format!("{} — {}", prefixe, choix)
}

// ------------------------------------------------------ la repetition avant l'acte

/// UNE DECLARATION REFUSEE AU BILAN n'a rien depose : « c'est seulement après avoir satisfait à l'ensemble des
/// contrôles […] qu'une déclaration sera acceptée », et une annule-et-remplace « doit annuler une déclaration
/// précédemment reçue » (cahier technique DSN, § 4.1 et 4.2, `atelier/reference/norme-dsn/texte/chapitre-4-controles.txt`) :
/// la suivante est une normale. Une DPAE : un retour KO sans certificat ni bilan OK (`verdict_dpae`). Une DSN : un retour
/// KO ; pour un depot repris des retours (`reprise_journal`) sans bilan range en face, le verdict que la ligne garde.
pub fn refuse_par_un_retour(ligne: &Ligne) -> bool {
    match ligne {
        Ligne::Dpae(e) => verdict_dpae(&e.retours).1,
        Ligne::Dsn(e) => {
            if e.retours.iter().any(RetourOrganisme::ko) {
                return true;
            }
            let bilan = e.retours.iter().any(|r| r.nature == "11");
            !bilan && e.detail.reconstruit && e.detail.etat_declaration.as_deref().unwrap_or("").eq_ignore_ascii_case("KO")
        }
        _ => false,
    }
}

/// Le compte rendu d'un depot : `ko` des qu'un retour refuse ; `ok` seulement quand LE BILAN le dit -- la nature 11
/// (certificat de conformite ou bilan d'anomalies) pour une DSN, le certificat 41 pour une DPAE ; `attendu` sinon.
/// L'ACCUSE D'ENREGISTREMENT (nature 10) NE COMPTE PAS : il dit « envoi recu », `etat_envoi` OK, pas « declaration
/// conforme » (regle 5 ; `retours::NATURES` : « 10 - Accuse d'enregistrement electronique »). Un test dont seul
/// l'accuse est la ne doit pas s'afficher « testee — conforme » quand l'essai precedent du meme contenu a pris
/// B1-208-11.
fn compte_rendu_du_depot(retours: &[RetourOrganisme]) -> &'static str {
    let mut ok = false;
    for r in retours {
        if r.ko() {
            return "ko";
        }
        ok = ok || ((r.nature == "11" || r.nature == NATURE_CERTIFICAT_DPAE) && r.verdict() == "OK");
    }
    if ok {
        "ok"
    } else {
        "attendu"
    }
}

/// Le dernier depot reel accepte de ce fichier (`empreinte`), s'il y en a un.
pub fn deja_depose(lignes: &[Ligne], quoi: Acte, empreinte: &str) -> Option<Ligne> {
    crate::journal::filtrer(lignes, Some(quoi), None, true).filter(|l| l.accepte() && l.empreinte() == Some(empreinte)).last().cloned()
}

/// Ce qui a fait echouer un depot de test, en une phrase : les anomalies des retours refuses, sinon la reponse du depot.
pub fn pourquoi_rate(ligne: &Ligne) -> String {
    let mut anomalies: Vec<&str> = Vec::new();
    for a in ligne.retours_organisme().iter().filter(|r| r.ko()).flat_map(|r| r.anomalies()) {
        if !anomalies.contains(&a.as_str()) {
            anomalies.push(a);
        }
    }
    if !anomalies.is_empty() {
        return anomalies.join(" ; ");
    }
    let rep = ligne.reponse().trim();
    if rep.is_empty() {
        "motif non lu".into()
    } else {
        rep.to_string()
    }
}

/// OU EN EST LE TEST D'UN FICHIER (`repetition`) : sa derniere ligne de test conforme (`conforme`), son compte rendu
/// (`ok`, `ko`, `attendu`, ou rien sans test conforme), les essais rates du meme contenu et ceux d'un contenu d'avant sous
/// le meme nom, la couleur et les mots de l'encart qui le dit.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Repetition {
    pub empreinte: String,
    pub fichier: String,
    pub conforme: Option<Ligne>,
    pub compte_rendu: &'static str,
    pub ratees: Vec<Ligne>,
    pub perimees: Vec<Ligne>,
    pub couleur: &'static str,
    pub motif: Option<String>,
    pub etat: EtatDuTest,
    pub titre: String,
    pub phrase: String,
}

/// OU EN EST LE TEST, dit une fois : teste et conforme (le reel peut partir), en test (son compte rendu est attendu),
/// refuse (ses anomalies : `motif`), ou a faire (jamais essaye, ou un test d'un contenu d'avant).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EtatDuTest {
    Testee,
    EnTest,
    TestRefuse,
    Ecrite,
}

impl EtatDuTest {
    /// Son nom, tel que la reponse l'ecrit.
    pub fn nom(self) -> &'static str {
        match self {
            EtatDuTest::Testee => "testee",
            EtatDuTest::EnTest => "en_test",
            EtatDuTest::TestRefuse => "test_refuse",
            EtatDuTest::Ecrite => "ecrite",
        }
    }
}

impl Repetition {
    /// Le flux du test conforme (`""` sans lui).
    pub fn flux_conforme(&self) -> String {
        self.conforme.as_ref().map(|c| c.flux().to_string()).unwrap_or_default()
    }

    /// Les lignes qu'il porte, pour y poser ce que la page en montre.
    pub fn lignes_mut(&mut self) -> impl Iterator<Item = &mut Ligne> {
        self.conforme.iter_mut().chain(self.ratees.iter_mut()).chain(self.perimees.iter_mut())
    }
}

pub fn repetition(lignes: &[Ligne], quoi: Acte, empreinte: &str, nom: &str) -> Repetition {
    let mut conforme: Option<&Ligne> = None;
    let (mut ratees, mut perimees) = (Vec::new(), Vec::new());
    for l in crate::journal::filtrer(lignes, Some(quoi), None, false).filter(|l| l.mode() == Mode::Test) {
        if l.empreinte() == Some(empreinte) {
            if l.accepte() && !refuse_par_un_retour(l) {
                conforme = Some(l);
            } else {
                ratees.push(l.clone());
            }
        } else if l.fichier() == Some(nom) {
            perimees.push(l.clone());
        }
    }
    let compte_rendu = conforme.map_or("", |c| compte_rendu_du_depot(c.retours_organisme()));
    let motif = ratees.last().map(pourquoi_rate);
    // LA COULEUR DE L'ENCART qui le dit (la charte des etats, `socle::ecran::couleur_de_l_etat`) : teste conforme, vert ;
    // essaye et rate (ou un test d'un contenu d'avant), a corriger, orange ; en test, ou jamais essaye, une information.
    let couleur = match conforme {
        Some(_) if compte_rendu == "ok" => "vert",
        Some(_) => "gris",
        None if !ratees.is_empty() || !perimees.is_empty() => "orange",
        None => "gris",
    };
    let etat = match conforme {
        Some(_) if compte_rendu == "ok" => EtatDuTest::Testee,
        Some(_) => EtatDuTest::EnTest,
        None if !ratees.is_empty() => EtatDuTest::TestRefuse,
        None => EtatDuTest::Ecrite,
    };
    // CE QUE L'ENCART DU TEST DIT, titre et phrase, d'apres ou il en est : la page et `moteur route` les lisent tels quels.
    let cette = if quoi == Acte::Dpae { "Cette DPAE" } else { "Cette DSN" };
    let quand_conforme = conforme.map(|c| quand_lisible(c.quand())).unwrap_or_default();
    let derniere = |l: &[Ligne]| l.last().map(|x| quand_lisible(x.quand())).unwrap_or_default();
    const ENSUITE: &str = "Le dépôt réel se fait une fois le test revenu conforme.";
    let (titre, phrase) = match etat {
        EtatDuTest::Testee => (format!("{} est testée — conforme. Voulez-vous l'envoyer ?", cette), format!("Déposée en test le {}, avec ce même contenu.", quand_conforme)),
        EtatDuTest::EnTest => (format!("{} est en test — son compte rendu est attendu", cette), format!("Envoyée en test le {}. {}", quand_conforme, ENSUITE)),
        EtatDuTest::TestRefuse => (
            "Le test est refusé".into(),
            format!(
                "Essayé le {} : {}. Le même refus attendrait le dépôt réel : c'est cela qu'il faut corriger.",
                derniere(&ratees),
                motif.as_deref().unwrap_or_default().trim_end_matches('.')
            ),
        ),
        EtatDuTest::Ecrite if !perimees.is_empty() => (
            "Pas de test valable pour ce contenu".into(),
            format!(
                "Un fichier du même nom a été essayé le {}, mais son contenu a changé depuis : le test ne vaut plus. {}",
                derniere(&perimees),
                ENSUITE
            ),
        ),
        EtatDuTest::Ecrite => ("D'abord, le dépôt de test".into(), format!("Ce contenu n'a jamais été déposé en test. {}", ENSUITE)),
    };
    Repetition { empreinte: empreinte.to_string(), fichier: nom.to_string(), conforme: conforme.cloned(), compte_rendu, ratees, perimees, couleur, motif, etat, titre, phrase }
}

/// La pastille de chaque personne des certificats de conformite que la ligne porte : ce que la page en montre.
pub fn poser_les_pastilles(l: &mut Ligne) {
    let retours = match l {
        Ligne::Dsn(e) => &mut e.retours,
        Ligne::Dpae(e) => &mut e.retours,
        _ => return,
    };
    for p in retours.iter_mut().flat_map(|r| r.dpae.iter_mut().flatten()) {
        p.pastille = Some(p.pastille_due());
    }
}

// -------------------------------------------------------------------- flux_de

fn egal_sans_casse(c: char, voulu: char) -> bool {
    c.to_ascii_lowercase() == voulu || (voulu == 'i' && (c == '\u{131}' || c == '\u{130}'))
}

fn mot(c: &[char], mut j: usize, mot: &str) -> Option<usize> {
    for v in mot.chars() {
        if j < c.len() && egal_sans_casse(c[j], v) {
            j += 1;
        } else {
            return None;
        }
    }
    Some(j)
}

fn blancs(c: &[char], mut j: usize) -> usize {
    while j < c.len() && socle::texte::blanc(c[j]) {
        j += 1;
    }
    j
}

/// `<\s*idflux\s*>\s*([^<\s]+)\s*<`, insensible a la casse.
fn flux_xml(c: &[char]) -> Option<String> {
    for i in 0..c.len() {
        if c[i] != '<' {
            continue;
        }
        let Some(j) = mot(c, blancs(c, i + 1), "idflux") else { continue };
        let j = blancs(c, j);
        if j >= c.len() || c[j] != '>' {
            continue;
        }
        let d = blancs(c, j + 1);
        let mut f = d;
        while f < c.len() && c[f] != '<' && !socle::texte::blanc(c[f]) {
            f += 1;
        }
        if f == d {
            continue;
        }
        let k = blancs(c, f);
        if k < c.len() && c[k] == '<' {
            return Some(c[d..f].iter().collect());
        }
    }
    None
}

/// `"idflux"\s*:\s*"([^"]+)"`, insensible a la casse.
fn flux_json(c: &[char]) -> Option<String> {
    for i in 0..c.len() {
        if c[i] != '"' {
            continue;
        }
        let Some(j) = mot(c, i + 1, "idflux") else { continue };
        if j >= c.len() || c[j] != '"' {
            continue;
        }
        let j = blancs(c, j + 1);
        if j >= c.len() || c[j] != ':' {
            continue;
        }
        let d = blancs(c, j + 1);
        if d >= c.len() || c[d] != '"' {
            continue;
        }
        let mut f = d + 1;
        while f < c.len() && c[f] != '"' {
            f += 1;
        }
        if f < c.len() && f > d + 1 {
            return Some(c[d + 1..f].iter().collect());
        }
    }
    None
}

pub fn flux_de(reponse: &str) -> String {
    if reponse.is_empty() {
        return String::new();
    }
    let c: Vec<char> = reponse.chars().collect();
    if let Some(m) = flux_xml(&c) {
        return m;
    }
    if let Some(m) = flux_json(&c) {
        return m;
    }
    let nu = reponse.trim_matches(socle::texte::blanc);
    let n = nu.chars().count();
    if (8..=64).contains(&n) && nu.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-') {
        nu.to_string()
    } else {
        String::new()
    }
}

// ------------------------------------------------------- les verdicts des retours

enum Jeton {
    Etoile,
    Un,
    Car(char),
    Classe(Vec<(char, char)>, bool),
}

/// Le motif d'un `glob` de `fnmatch` : `*`, `?`, `[...]`.
fn jetons(motif: &str) -> Vec<Jeton> {
    let p: Vec<char> = motif.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < p.len() {
        let c = p[i];
        i += 1;
        match c {
            '*' => {
                if !matches!(out.last(), Some(Jeton::Etoile)) {
                    out.push(Jeton::Etoile);
                }
            }
            '?' => out.push(Jeton::Un),
            '[' => {
                let mut j = i;
                if j < p.len() && p[j] == '!' {
                    j += 1;
                }
                if j < p.len() && p[j] == ']' {
                    j += 1;
                }
                while j < p.len() && p[j] != ']' {
                    j += 1;
                }
                if j >= p.len() {
                    out.push(Jeton::Car('['));
                } else {
                    let mut corps: &[char] = &p[i..j];
                    i = j + 1;
                    let nie = corps.first() == Some(&'!');
                    if nie {
                        corps = &corps[1..];
                    }
                    let mut plages = Vec::new();
                    let mut k = 0;
                    while k < corps.len() {
                        if k + 2 < corps.len() && corps[k + 1] == '-' {
                            plages.push((corps[k], corps[k + 2]));
                            k += 3;
                        } else {
                            plages.push((corps[k], corps[k]));
                            k += 1;
                        }
                    }
                    out.push(Jeton::Classe(plages, nie));
                }
            }
            c => out.push(Jeton::Car(c)),
        }
    }
    out
}

fn correspond(j: &[Jeton], nom: &[char]) -> bool {
    let sans_casse = cfg!(windows);
    let egal = |a: char, b: char| if sans_casse { a.to_lowercase().eq(b.to_lowercase()) } else { a == b };
    let (mut ji, mut ni) = (0, 0);
    let mut reprise: Option<(usize, usize)> = None;
    while ni < nom.len() {
        let ok = match j.get(ji) {
            Some(Jeton::Etoile) => {
                reprise = Some((ji, ni));
                ji += 1;
                continue;
            }
            Some(Jeton::Un) => true,
            Some(Jeton::Car(c)) => egal(*c, nom[ni]),
            Some(Jeton::Classe(plages, nie)) => plages.iter().any(|&(a, b)| a <= nom[ni] && nom[ni] <= b) != *nie,
            None => false,
        };
        if ok {
            ji += 1;
            ni += 1;
        } else if let Some((rj, rn)) = reprise {
            reprise = Some((rj, rn + 1));
            ji = rj + 1;
            ni = rn + 1;
        } else {
            return false;
        }
    }
    while matches!(j.get(ji), Some(Jeton::Etoile)) {
        ji += 1;
    }
    ji == j.len()
}

/// UN RETOUR TEL QUE LA LISTE DES RETOURS LE DONNE (`retours::liste_retours_d_octets`, une `Value` par retour) : son
/// flux, sa nature, sa publication, son statut.
struct Annonce {
    flux: String,
    nature: String,
    publication: Option<String>,
    statut: Option<String>,
}

impl Annonce {
    fn de(r: &Value) -> Annonce {
        let texte = |k: &str| r.get(k).filter(|v| !v.is_null()).map(valeur_en_texte);
        Annonce { flux: texte("flux").unwrap_or_default(), nature: texte("nature").unwrap_or_default(), publication: texte("publication"), statut: texte("statut") }
    }

    /// La cle de son contenu telecharge (`rattacher_retours_avec_contenus`).
    fn cle(&self) -> (String, String, String) {
        (self.flux.clone(), self.nature.clone(), self.publication.clone().unwrap_or_default())
    }
}

/// La cle du contenu telecharge du retour `r` (tel que la liste des retours le decrit) : son flux, sa nature, sa
/// publication.
pub fn cle_du_contenu(r: &Value) -> (String, String, String) {
    Annonce::de(r).cle()
}

/// La piece archivee du retour `r` d'un envoi `quoi`, dans `retours/` (`retours/dpae/` pour une DPAE).
fn archive_du_retour(racine: &Path, r: &Annonce, quoi: Acte) -> R<Option<PathBuf>> {
    let dossier = if quoi == Acte::Dpae { racine.join(crate::retours::RETOURS_DPAE) } else { racine.join(crate::retours::RETOURS) };
    if !dossier.is_dir() {
        return Ok(None);
    }
    let flux: String = if r.flux.is_empty() { "sansflux".to_string() } else { r.flux.clone() };
    let motif = jetons(&format!("*_nature{}_{}*", r.nature, flux));
    let mut noms = Vec::new();
    for e in std::fs::read_dir(&dossier).map_err(|e| erreur_io(&e, &dossier))? {
        let e = e.map_err(|e| erreur_io(&e, &dossier))?;
        let nom = e.file_name().to_string_lossy().into_owned();
        if correspond(&motif, &nom.chars().collect::<Vec<_>>()) {
            noms.push(nom);
        }
    }
    noms.sort();
    Ok(noms.into_iter().find(|n| n != "journal.json").map(|n| dossier.join(n)))
}

fn lire_archive(f: &Path) -> R<Vec<u8>> {
    std::fs::read(f).map_err(|e| erreur_io(&e, f))
}

/// Ce que le certificat de conformite d'une DPAE (nature 41) dit de chaque personne ; rien s'il ne se lit pas.
fn verdict_du_certificat(contenu: &[u8]) -> R<Option<Verdict>> {
    let texte: String = contenu.iter().map(|&b| b as char).collect();
    let lignes = declarations::dpae::lire_certificat(&texte);
    if lignes.is_empty() {
        return Ok(None);
    }
    let texte_de = |l: &Value, k: &str| l.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    let mut personnes = Vec::new();
    let mut anomalies: Vec<String> = Vec::new();
    for l in &lignes {
        let lot = texte_de(l, "lot");
        if lot != "00" && !lot.is_empty() {
            if let Some((_, nom)) = declarations::dpae::CODES_RET_AR_LOT.iter().find(|(c, _)| *c == lot) {
                anomalies.push(format!("{} (lot)", nom));
            }
        }
        let p = PersonneCertifiee {
            nom: texte_de(l, "nom"),
            prenom: texte_de(l, "prenom"),
            embauche: texte_de(l, "embauche"),
            dossier: texte_de(l, "dossier"),
            code: texte_de(l, "code"),
            libelle: texte_de(l, "libelle"),
            enregistree: l.get("enregistree").is_some_and(vrai),
            deja_existante: l.get("deja_existante").is_some_and(vrai),
            formalites: l.get("formalites").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).map(String::from).collect(),
            pastille: None,
        };
        if !p.enregistree && !p.deja_existante {
            anomalies.push(format!("{} -- {}", p.libelle, p.nom));
        }
        personnes.push(p);
    }
    let mut vus: Vec<String> = Vec::new();
    for a in anomalies {
        if !vus.contains(&a) {
            vus.push(a);
        }
    }
    Ok(Some(Verdict { verdict: if vus.is_empty() { "OK" } else { "KO" }.into(), anomalies: vus, dpae: Some(personnes) }))
}

/// Ce que la piece du retour `r` d'un envoi `quoi` en dit : le bilan (nature 10 ou 11) ou le certificat d'une DPAE
/// (nature 41) ; rien pour une autre nature, ou une piece absente ou illisible.
fn verdict_du_retour(racine: &Path, r: &Annonce, quoi: Acte, contenu: Option<&[u8]>) -> R<Option<Verdict>> {
    let nature = r.nature.as_str();
    if nature != "10" && nature != "11" && !(quoi == Acte::Dpae && nature == NATURE_CERTIFICAT_DPAE) {
        return Ok(None);
    }
    let octets = match contenu {
        Some(c) => c.to_vec(),
        None => match archive_du_retour(racine, r, quoi)? {
            None => return Ok(None),
            Some(f) => lire_archive(&f)?,
        },
    };
    if nature == NATURE_CERTIFICAT_DPAE {
        return verdict_du_certificat(&octets);
    }
    let b = retours::bilan_d_octets(&octets);
    if b.is_null() {
        return Ok(None);
    }
    let etat = |k: &str| b.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    let verdict = if !etat("etat_declaration").is_empty() { etat("etat_declaration") } else { etat("etat_envoi") };
    let anomalies = retours::anomalies_lisibles_d_octets(&octets)?.iter().map(valeur_en_texte).collect();
    Ok(Some(Verdict { verdict: verdict.to_uppercase(), anomalies, dpae: None }))
}

/// Le libelle d'une nature de retour, selon le canal de l'envoi.
fn libelle_nature(code: &str, acte: Acte) -> String {
    if code.is_empty() {
        return String::new();
    }
    if acte == Acte::Dpae {
        return match NATURES_DPAE.iter().find(|(k, _)| *k == code) {
            Some((_, l)) => l.to_string(),
            None => format!("nature {} (inconnue du canal DPAE)", code),
        };
    }
    retours::libelle_nature(code)
}

/// L'INCIDENT D'UN RETOUR QUI REJETTE (le format du journal des incidents, que `moteur` ecrit) : rien quand le retour ne
/// rejette pas, ou quand son calcul echoue (il ne leve jamais).
fn incident(racine: &Path, ligne: &Ligne, r: &Annonce, verdict: &Verdict, contenus: &HashMap<(String, String, String), Vec<u8>>) -> Option<Value> {
    let calcul = || -> R<Option<Value>> {
        if verdict.verdict != "KO" {
            return Ok(None);
        }
        let quoi = ligne.acte();
        let mut anomalies: Vec<Value> = Vec::new();
        if quoi == Acte::Dpae && r.nature == NATURE_CERTIFICAT_DPAE {
            for d in verdict.dpae.iter().flatten().filter(|d| !d.enregistree && !d.deja_existante) {
                anomalies.push(json!({"code": d.code, "message": d.libelle}));
            }
        } else {
            let contenu = match contenus.get(&r.cle()) {
                Some(c) => c.clone(),
                None => match archive_du_retour(racine, r, quoi)? {
                    Some(f) => lire_archive(&f)?,
                    None => Vec::new(),
                },
            };
            anomalies = retours::anomalies_codees_d_octets(&contenu).map_err(|e| Echec { classe: e.classe, message: e.message })?;
        }
        // Sans doublon, puis les cles par ordre alphabetique.
        let mut vus: Vec<String> = Vec::new();
        let mut uniques: Vec<Value> = Vec::new();
        for a in &anomalies {
            let Value::Object(o) = a else {
                return Err(erreur("AttributeError", "anomalie illisible"));
            };
            let mut p: Vec<(&String, &Value)> = o.iter().collect();
            p.sort_by(|x, y| x.0.cmp(y.0));
            for (_, v) in &p {
                cle_de_hachage(v)?;
            }
            let k = p.iter().map(|(k, v)| format!("{}={}", k, cle_de_hachage(v).unwrap_or_default())).collect::<Vec<_>>().join("\u{1e}");
            if !vus.contains(&k) {
                vus.push(k);
                uniques.push(Value::Object(p.into_iter().map(|(k, v)| (k.clone(), v.clone())).collect()));
            }
        }
        let mut cles: Vec<(bool, String, Value)> = Vec::new();
        for a in uniques {
            let bloquant = !est(get(&a, "categorie")?, "bloquant");
            let code = match getd(&a, "code", texte_vide())? {
                Value::String(t) => t.clone(),
                autre => return Err(erreur("TypeError", format!("'code' : {}", socle::fichiers::incomparables(nom_type(autre), "str")))),
            };
            cles.push((bloquant, code, a));
        }
        cles.sort_by(|x, y| (x.0, &x.1).cmp(&(y.0, &y.1)));
        let anomalies: Vec<Value> = cles.into_iter().map(|c| c.2).collect();
        let mut codes = Vec::new();
        for a in &anomalies {
            if vrai(get(a, "code")?) {
                codes.push(get(a, "code")?.clone());
            }
        }
        let mut phrases = Vec::new();
        for a in anomalies.iter().take(10) {
            let t = format!("{} : {}", valeur_en_texte(get(a, "code")?), valeur_en_texte(get(a, "message")?));
            phrases.push(t.trim_matches(|c| c == ' ' || c == ':').to_string());
        }
        let resume = if phrases.join(" ; ").is_empty() { "Rejet sans motif lisible".to_string() } else { phrases.join(" ; ") };
        let type_declaration = ligne.dsn().and_then(|e| e.detail.type_declaration.clone()).unwrap_or_default();
        Ok(Some(json!({
            "canal": if quoi == Acte::Dpae { "dpae" } else { "dsn" },
            "ou": format!("retour-{}", quoi.nom()),
            "quoi": codes.first().cloned().unwrap_or_else(|| s("KO")),
            "resume": resume,
            "detail": {
                "reel": ligne.reel(),
                "nature_retour": r.nature,
                "type_declaration": type_declaration,
                "anomalies": anomalies.iter().take(30).collect::<Vec<_>>(),
            },
        })))
    };
    calcul().ok().flatten()
}

/// Le nombre ajoute ; les incidents a noter (dans
/// l'ordre) s'ajoutent a `incidents`, que l'appelant ecrit dans le journal des
/// incidents, meme quand le rattachement s'arrete sur une erreur.
fn rattacher_retours(racine: &Path, retours: &[Value], contenus: &HashMap<(String, String, String), Vec<u8>>, incidents: &mut Vec<Value>) -> R<i64> {
    if !chemin(racine).exists() {
        return Ok(0);
    }
    let mut lignes = (*crate::journal::lu(racine)?.lignes).clone();
    let mut rangements: Vec<(usize, Rangement)> = Vec::new();
    let mut par_flux: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, l) in lignes.iter().enumerate() {
        if !l.flux().is_empty() {
            par_flux.entry(l.flux().to_string()).or_default().push(i);
        }
    }
    let mut ajoutes = 0;
    for r in retours.iter().map(Annonce::de) {
        for &li in par_flux.get(&r.flux).map(Vec::as_slice).unwrap_or(&[]) {
            let quoi = lignes[li].acte();
            let verdict = verdict_du_retour(racine, &r, quoi, contenus.get(&r.cle()).map(Vec::as_slice))?;
            let ligne = lignes[li].clone();
            let connus = match &mut lignes[li] {
                Ligne::Dsn(e) => &mut e.retours,
                Ligne::Dpae(e) => &mut e.retours,
                _ => continue,
            };
            let neuf = RetourOrganisme { nature: r.nature.clone(), verdict: None, anomalies: None, dpae: None, libelle: libelle_nature(&r.nature, quoi), statut: r.statut.clone(), publication: r.publication.clone() };
            if let Some(x) = connus.iter_mut().find(|x| x.meme(&neuf)) {
                if let Some(v) = verdict.filter(|_| x.verdict.is_none()) {
                    x.poser(v.clone());
                    rangements.push((li, Rangement::Complement { nature: x.nature.clone(), publication: x.publication.clone(), verdict: v.clone() }));
                    ajoutes += 1;
                    incidents.extend(incident(racine, &ligne, &r, &v, contenus));
                }
                continue;
            }
            let mut neuf = neuf;
            if let Some(v) = verdict {
                incidents.extend(incident(racine, &ligne, &r, &v, contenus));
                neuf.poser(v);
            }
            connus.push(neuf.clone());
            rangements.push((li, Rangement::Retour(neuf)));
            ajoutes += 1;
        }
    }
    crate::journal::ranger(racine, rangements).map_err(|m| erreur("OSError", m))?;
    Ok(ajoutes)
}

/// Ce qu'un rattachement rend : le nombre de retours ranges (ou l'erreur qui l'a arrete), et les incidents qu'il a
/// releves, que `moteur` note au journal des incidents, meme quand le rattachement s'est arrete sur une erreur.
pub struct AvecIncidents<T> {
    pub resultat: R<T>,
    /// `{"canal": "dsn"|"dpae", "ou", "quoi", "resume", "detail"}`, dans l'ordre.
    pub incidents: Vec<Value>,
}

/// Range les retours `retours` (la liste des retours) en face des lignes du journal qu'ils concernent (par le flux).
/// `contenus` donne, pour chaque `(flux, nature, publication)` telecharge, les octets du retour : le verdict se lit dedans.
pub fn rattacher_retours_avec_contenus(racine: &Path, retours: &[Value], contenus: &[((String, String, String), Vec<u8>)]) -> AvecIncidents<i64> {
    let mut incidents: Vec<Value> = Vec::new();
    let dico: HashMap<(String, String, String), Vec<u8>> = contenus.iter().cloned().collect();
    let resultat = rattacher_retours(racine, retours, &dico, &mut incidents);
    AvecIncidents { resultat, incidents }
}

/// `rattacher_retours_avec_contenus` sans contenus : les retours deja archives.
pub fn rattacher_retours_ecrits(racine: &Path, retours: &[Value]) -> AvecIncidents<i64> {
    rattacher_retours_avec_contenus(racine, retours, &[])
}

// ------------------------------------------------------ le contrat parti

/// Le dernier contrat parti a la signature pour cette paie :
/// le mois, le numero, le fichier, la date d'envoi et, s'il est revenu, celle de la signature.
/// Un avenant n'en est pas un.
#[derive(Debug, Clone)]
pub struct Parti {
    pub mois: Mois,
    pub numero: Option<engagement::numeros::NumeroContrat>,
    pub fichier: String,
    pub quand: String,
    pub signe: Option<String>,
}

impl Parti {
    /// Le jour qui date le contrat dans un avenant : celui de sa signature, sinon de son envoi.
    pub fn date(&self) -> String {
        self.signe.as_deref().unwrap_or(&self.quand).chars().take(10).collect()
    }
}

/// Le contrat n° `numero` de ce salarie, tel qu'il est parti a la signature.
pub fn contrat_parti(lignes: &[Ligne], salarie: &str, numero: &str) -> Option<Parti> {
    let mut parti: Option<Parti> = None;
    for l in lignes.iter().filter(|l| l.accepte()) {
        if let Some(e) = l.signature() {
            for c in e.detail.contrats.iter().filter(|c| c.salarie.as_str() == salarie && c.numero_dit() == numero && !c.est_avenant()) {
                parti = Some(Parti { mois: c.mois, numero: c.numero.clone(), fichier: c.fichier.clone(), quand: e.quand.clone(), signe: None });
            }
        }
        for d in revenus(l) {
            if let Some(p) = parti.as_mut() {
                if d.recu && d.salarie.as_str() == salarie && d.numero_dit() == numero && p.signe.is_none() {
                    p.signe = Some(d.quand.clone());
                }
            }
        }
    }
    parti
}

/// Le fichier d'une piece partie, dans `sorties/<mois>/` ou son `envoi/`. Un NOM, jamais un chemin : ce qui porte un
/// separateur ne se resout pas.
pub fn chemin_piece(racine: &Path, mois: Mois, nom: &str) -> Option<PathBuf> {
    if nom.is_empty() || ["/", "\\", ".."].iter().any(|x| nom.contains(x)) {
        return None;
    }
    let base = racine.join("sorties").join(mois.to_string());
    [base.join("envoi").join(nom), base.join(nom)].into_iter().find(|f| f.is_file())
}

/// Les avenants du contrat parti -- pas ceux d'un contrat qu'il a remplace. `tous` : les avenants de son contrat
/// (`avenants_par_contrat`).
pub fn avenants_du_contrat(tous: Option<&Vec<AvenantParti>>, parti: &Parti) -> Vec<AvenantParti> {
    tous.into_iter().flatten().filter(|x| x.quand >= parti.quand).cloned().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un journal sur le disque, pour ce qui l'ecrit ou le lit.
    fn journal(nom: &str, lignes: &[Ligne]) -> PathBuf {
        let p = std::env::temp_dir().join(format!("paie-etats-{}-{}", nom, std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(p.join(crate::journal::JOURNAL)).unwrap();
        let texte: String = lignes.iter().enumerate().map(|(i, l)| format!("{}\n", json!({"id": i.to_string(), "ligne": l}))).collect();
        std::fs::write(chemin(&p), texte).unwrap();
        p
    }

    fn ligne(quoi: &str, mode: &str, quand: &str, detail: Value, retours: Value) -> Value {
        json!({"quoi": quoi, "quand": quand, "origine": "", "mois": "2026-06", "fichier": "f.dsn", "empreinte": "e1",
               "mode": mode, "qui": "?", "accepte": true, "destinataire": "", "flux": "", "retours": retours,
               "reponse": "", "trace": null, "detail": detail})
    }

    fn lue(v: Value) -> Ligne {
        serde_json::from_value(v).unwrap()
    }

    fn contrat(sal: &str, code: &str) -> Value {
        json!({"salarie": sal, "debut": "2026-06-04", "fin": "2026-06-30", "numero": "SA1.26.01", "fichier": "c.pdf", "spectacle": null,
               "code": code, "empreinte": "e1", "message": "m1", "mois": "2026-06"})
    }

    fn recu(sal: &str, origine: Option<&str>) -> Value {
        let mut r = json!({"salarie": sal, "debut": "2026-06-04", "numero": "SA1.26.01", "nom": sal, "recu": true, "quand": "2026-06-12T10:00:00", "qui": "t", "reponse": ""});
        if let Some(o) = origine {
            r["origine"] = json!(o);
        }
        r
    }

    fn signature(quand: &str, c: Value) -> Value {
        ligne("contrat-signature", "reel", quand, json!({"contrats": [c]}), json!([]))
    }

    #[test]
    fn une_reponse_deja_relevee_ne_se_renote_pas_apres_la_mention_retiree() {
        let ctx = |_: &str| None;
        let mut lu = recu("ALPHA", Some("courriel"));
        lu["code"] = json!("C1");
        let mut retiree = recu("ALPHA", None);
        retiree["recu"] = json!(false);
        let mut envoi = signature("2026-06-05T10:00:00", contrat("ALPHA", "C1"));
        envoi["retours"] = json!([lu, retiree]);
        let mut l = vec![lue(envoi)];
        assert!(signatures_attendues(&ctx, &l, None).is_empty());
        l.push(lue(signature("2026-06-13T10:00:00", contrat("ALPHA", "C3"))));
        assert_eq!(signatures_attendues(&ctx, &l, None)[0].code, "C3");
    }

    #[test]
    fn la_repetition_et_le_deja_depose() {
        let avec = |mode: &str, quand: &str, empreinte: &str, fichier: &str, accepte: bool, retours: Value| {
            let mut l = ligne("dsn", mode, quand, json!({"repetition": null, "ordre_test": null}), retours);
            l["empreinte"] = json!(empreinte);
            l["fichier"] = json!(fichier);
            l["accepte"] = json!(accepte);
            lue(l)
        };
        let l = vec![
            avec("test", "2026-06-01T09:00:00", "eB", "DSN.dsn", true, json!([])),
            avec("test", "2026-06-01T10:00:00", "eA", "f.dsn", true, json!([{"nature": "11", "verdict": "OK", "libelle": "", "statut": null, "publication": null}])),
            avec("test", "2026-06-01T11:00:00", "eA", "f.dsn", false, json!([])),
            avec("reel", "2026-06-02T09:00:00", "eA", "f.dsn", true, json!([])),
        ];
        let rep = repetition(&l, Acte::Dsn, "eA", "DSN.dsn");
        assert_eq!((rep.compte_rendu, rep.conforme.as_ref().unwrap().quand()), ("ok", "2026-06-01T10:00:00"));
        assert_eq!((rep.ratees.len(), rep.perimees.len(), rep.etat), (1, 1, EtatDuTest::Testee));
        let vide = repetition(&l, Acte::Dpae, "eA", "x");
        assert_eq!((vide.conforme.is_none(), vide.compte_rendu, vide.etat), (true, "", EtatDuTest::Ecrite));
        assert_eq!(deja_depose(&l, Acte::Dsn, "eA").unwrap().quand(), "2026-06-02T09:00:00");
        assert!(deja_depose(&l, Acte::Dsn, "eB").is_none());
        let r = |verdict: &str| RetourOrganisme { nature: "11".into(), verdict: Some(verdict.into()), anomalies: None, dpae: None, libelle: String::new(), statut: None, publication: None };
        assert_eq!(compte_rendu_du_depot(&[r("OK"), r("KO")]), "ko");
        assert_eq!(compte_rendu_du_depot(&[]), "attendu");
    }

    const BAN_KO: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?><ns2:rapport xmlns:ns2=\"http://www.gip-mds.fr/\" type=\"BAN\">\
         <envoi><idflux>WSBBBB</idflux></envoi><declaration><declaration_bilan><etat>KO</etat>\
         <declaration_anomalie><code>CST-04</code><categorie>bloquant</categorie><message>Code URSSAF invalide</message>\
         <identifiant>S21.G00.20.001</identifiant><nom_famille>FAVART</nom_famille></declaration_anomalie>\
         </declaration_bilan></declaration></ns2:rapport>";

    #[test]
    fn le_rattachement_pose_le_verdict_et_signale_l_incident() {
        let mut l = ligne("dsn", "test", "2026-06-01T10:00:00", json!({"repetition": null, "ordre_test": null}), json!([]));
        l["flux"] = json!("WSBBBB");
        let mut autre = ligne("dpae", "reel", "2026-06-01T10:00:00", json!({"repetition": null}), json!([]));
        autre["flux"] = json!("WSCCCC");
        let r = journal("rattachement", &[lue(l), lue(autre)]);
        let retours = vec![
            json!({"flux": "WSBBBB", "nature": "11", "publication": "p1", "statut": "recu"}),
            json!({"flux": "WSINCONNU", "nature": "11", "publication": "p2", "statut": "recu"}),
            json!({"flux": "WSCCCC", "nature": "20", "publication": "p3", "statut": "recu"}),
        ];
        let mut contenus = HashMap::new();
        contenus.insert(("WSBBBB".to_string(), "11".to_string(), "p1".to_string()), BAN_KO.as_bytes().to_vec());
        let mut inc = Vec::new();
        assert_eq!(rattacher_retours(&r, &retours, &contenus, &mut inc).unwrap(), 2);
        assert_eq!(inc.len(), 1);
        assert_eq!((&inc[0]["canal"], &inc[0]["ou"], &inc[0]["quoi"]), (&json!("dsn"), &json!("retour-dsn"), &json!("CST-04")));
        assert_eq!(inc[0]["resume"], "CST-04 : Code URSSAF invalide");
        assert_eq!(inc[0]["detail"]["reel"], false);
        assert_eq!(inc[0]["detail"]["anomalies"][0]["rubrique"], "S21.G00.20.001");
        // ni le nom du salarie ni la valeur ne partent chez l'editeur
        assert!(!inc[0].to_string().contains("FAVART"));
        let fichier = crate::journal::lu(&r).unwrap().lignes;
        let pose = &fichier[0].retours_organisme()[0];
        assert_eq!(pose.verdict(), "KO");
        assert_eq!(pose.anomalies(), ["Code URSSAF invalide (bloquant) -- FAVART"]);
        assert_eq!(pose.libelle, "Certificat de conformite / bilan d'anomalies");
        assert_eq!(fichier[1].retours_organisme()[0].libelle, "bilan d'identification des salaries");
        // un second passage ne rajoute rien
        assert_eq!(rattacher_retours(&r, &retours, &contenus, &mut Vec::new()).unwrap(), 0);
        std::fs::remove_dir_all(&r).unwrap();
    }
}

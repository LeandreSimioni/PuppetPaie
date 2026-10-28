//! La DSN mensuelle, dans la version de la norme NEODeS que son mois principal declare appelle
//! (`norme::de_la_mensuelle`, `logiciel/donnees/norme-dsn-base.json`).
//!
//! Le fichier est une suite de lignes `RUBRIQUE,'valeur'` separees par CRLF,
//! ecrites en latin-1 par l'appelant. Le moteur refuse plutot que d'ecrire une
//! valeur qu'il ne sait pas.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::fmt::Display;
use std::path::Path;

use rust_decimal::RoundingStrategy;
use serde_json::{Map, Value};

use droit::bareme::{self, bareme_applique_au_contrat};
use documents::bordereau::{self, Destinataire, LigneBordereau};
use droit::reduction::au_dela_du_seuil;
use engagement::calcul::Paie;
use droit::contexte::{aujourdhui, Contexte};
use droit::natures::{contrat_court, Nature};
use socle::date::{en_texte, Jour};
use socle::fichiers::{dossier_du_code, json_en_cache, vrai};
use socle::num::{add, dec, dec_avec_echelle, div, eur, mul, pct, sub, Decimal, Resultat};
use droit::pas::{self, Triplet, ZONE_DEFAUT};

pub const LOGICIEL: &str = "PUPPETPAIE";
pub const VERSION: &str = "1.0.0";
pub const SIRET_FRANCE_TRAVAIL: &str = "13000548100010";

/// L'accumulateur de lignes DSN, et la version de la norme du fichier qu'il fabrique.
#[derive(Default)]
pub struct Fichier {
    pub lignes: Vec<String>,
    norme: Option<crate::norme::Norme>,
    /// L'individu en cours d'ecriture n'a qu'UN bloc « Contrat - S21.G00.40 » dans la declaration : S21.G00.78.006 ne
    /// s'ecrit pas. « Dans le cas de la declaration d'un contrat unique, cette rubrique n'est pas a renseigner. »
    /// (cahier technique 2026.1, p. 298, `atelier/reference/norme-dsn/texte/bloc-S21.G00.79.txt` l. 9-14 ; reste R1 #26).
    /// Pose par `generer` individu par individu ; faux ailleurs (la rubrique s'ecrit comme avant).
    pub contrat_unique: bool,
}

impl Fichier {
    /// Un morceau de fichier sans version (un bloc qu'on insere, un essai) : il ne decide d'aucune rubrique.
    pub fn nouveau() -> Fichier {
        Fichier::default()
    }

    /// Le fichier d'une version de la norme : `S10.G00.00.006` s'ecrit depuis elle (`ecrire_la_version`), et les
    /// rubriques qu'une version a retirees ou ajoutees se decident par elle (`norme()`).
    pub fn en_norme(norme: crate::norme::Norme) -> Fichier {
        Fichier { lignes: Vec::new(), norme: Some(norme), contrat_unique: false }
    }

    /// La version de la norme de ce fichier ; un morceau sans version refuse de decider.
    pub fn norme(&self) -> Resultat<&crate::norme::Norme> {
        self.norme.as_ref().ok_or_else(|| {
            "Fichier DSN sans version de la norme : les rubriques qu'il admet ne se decident pas (`Fichier::en_norme`).".to_string()
        })
    }

    /// `S10.G00.00.006`, le numero de version de la norme utilisee.
    pub fn ecrire_la_version(&mut self) -> Resultat<()> {
        let v = self.norme()?.version.clone();
        self.add(crate::norme::RUBRIQUE, v);
        Ok(())
    }

    pub fn add(&mut self, rubrique: &str, valeur: impl Display) {
        self.lignes.push(format!("{},'{}'", rubrique, valeur));
    }

    /// Une valeur absente ne s'ecrit pas « None » : c'est un refus.
    pub fn add_opt(&mut self, rubrique: &str, valeur: Option<impl Display>) -> Resultat<()> {
        match valeur {
            Some(v) => {
                self.add(rubrique, v);
                Ok(())
            }
            None => Err(absente(rubrique)),
        }
    }

    /// `f.add(rubrique, v)` pour une valeur JSON : `str(v)`, refus sur `None`.
    pub fn add_v(&mut self, rubrique: &str, valeur: &Value) -> Resultat<()> {
        if valeur.is_null() {
            return Err(absente(rubrique));
        }
        self.add(rubrique, en_texte(valeur));
        Ok(())
    }

    /// Les rubriques ecrites, `(rubrique, valeur)`, dans l'ordre.
    pub fn rubriques(&self) -> Vec<(String, String)> {
        self.lignes
            .iter()
            .map(|l| match l.split_once(',') {
                Some((k, v)) => (k.to_string(), v.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')).unwrap_or(v).to_string()),
                None => (l.clone(), String::new()),
            })
            .collect()
    }

    pub fn texte(&self) -> String {
        let mut t = self.lignes.join("\r\n");
        t.push_str("\r\n");
        t
    }
}

fn absente(rubrique: &str) -> String {
    format!(
        "{} : valeur absente. Une rubrique DSN sans valeur ne s'ecrit pas -- et surtout pas avec le mot None.",
        rubrique
    )
}

// ------------------------------------------------------------- les valeurs

const NUL: Value = Value::Null;

pub fn g<'a>(v: &'a Value, k: &str) -> &'a Value {
    v.get(k).unwrap_or(&NUL)
}

/// `v[k]` : la cle doit exister.
pub fn req<'a>(v: &'a Value, k: &str) -> Resultat<&'a Value> {
    v.get(k).ok_or_else(|| socle::fichiers::absent(k))
}

/// Le bloc `S10.G00.02` (contact émetteur), commun à la mensuelle et aux signalements : UNE personne, celle que les
/// organismes contactent (`declarant_dsn` de la fiche entreprise), civilité, nom, courriel et téléphone. Cahier
/// technique DSN 2026.1, p. 132 (`norme-dsn/texte/bloc-S10.G00.02.txt`, lignes 15-45) : « Acteur à contacter, chez
/// le déclarant » ; `02.001` : « 01 - monsieur / 02 - madame » ; `02.002` : « Nom, prénom de l'agent de l'émetteur
/// pouvant donner des précisions sur cet envoi ». Le nom ne vient plus du signataire (le courriel et le téléphone
/// venaient déjà du déclarant : deux personnes dans le même bloc), et la civilité n'est plus « 01 » en dur : elle
/// se lit dans la fiche, sinon refus.
pub fn contact_emetteur(f: &mut Fichier, ent: &Value) -> Resultat<()> {
    let decl = req(ent, "declarant_dsn")?;
    let civilite = txt(g(decl, "civilite"));
    if civilite != "01" && civilite != "02" {
        return Err(format!(
            "La civilité de la personne que les organismes contactent {} : la saisir dans Configuration › Entreprise, « Qui les organismes contactent », case Civilité (Monsieur ou Madame ; `declarant_dsn.civilite` de config/entreprise.json, 01 ou 02). Elle s'écrit en S10.G00.02.001 (cahier technique DSN, p. 132). Rien n'est écrit.",
            if civilite.is_empty() { "manque".to_string() } else { format!("vaut « {} », ni 01 ni 02", civilite) }
        ));
    }
    f.add("S10.G00.02.001", civilite);
    if !vrai(g(decl, "nom_prenom")) {
        return Err("Le nom de la personne que les organismes contactent manque : le saisir dans Configuration › Entreprise, « Qui les organismes contactent » (`declarant_dsn.nom_prenom`). Il s'écrit en S10.G00.02.002 et S20.G00.07.001. Rien n'est écrit.".to_string());
    }
    f.add_v("S10.G00.02.002", req(decl, "nom_prenom")?)?;
    f.add_v("S10.G00.02.004", req(decl, "email")?)?;
    f.add_v("S10.G00.02.005", req(decl, "tel")?)?;
    Ok(())
}

/// LE TYPE DU CONTACT CHEZ LE DECLARE (S20.G00.07.004), commun a la mensuelle, aux signalements et a l'amorcage. Le
/// moteur ne declare qu'UN contact (`declarant_dsn`) : celui qui repond aux organismes pour toute la declaration. La
/// norme (cahier technique 2027.1.1, `norme-dsn/texte-2027/bloc-S20.G00.07.txt`, rubrique « Type », les valeurs
/// lignes 67-80) donne la liste ; « 04 - Contact chez le declare recouvrant, entre autres, des cotisations de Securite
/// Sociale (Urssaf CN, MSA) et contributions assurance chomage et cotisations AGS (France Travail) » est celui-la. Les
/// types 14 et 17 (referent handicap des 250 salaries et plus, medecine du travail interne) ne sont pas ce contact-la.
/// Une compagnie neuve n'a donc rien a choisir (#236 : la fiche ne montre pas cette case, et « Pour demarrer » ne la
/// demande plus) ; une valeur posee dans la fiche (`declarant_dsn.qualite`) reste lue telle quelle.
pub const TYPE_CONTACT_ORDINAIRE: &str = "04";

/// Le type du contact a ecrire en S20.G00.07.004 : celui de la fiche s'il y en a un, sinon [`TYPE_CONTACT_ORDINAIRE`].
pub fn type_du_contact(decl: &Value) -> Value {
    match decl.get("qualite") {
        Some(q) if vrai(q) => q.clone(),
        _ => Value::String(TYPE_CONTACT_ORDINAIRE.to_string()),
    }
}

/// LA MEDECINE DU TRAVAIL INTERNE DE L'ETABLISSEMENT, dans l'identifiant de son service de prevention et de sante au travail
/// (S21.G00.11.025 : « Cette rubrique doit etre renseignee par l'identifiant du service de prevention et de sante au travail
/// (SPST) », `X [1,14]`, conditionnelle en mensuelle ; cahier 2026.1, `norme-dsn/texte/bloc-S21.G00.12.txt` l. 33-37, et
/// 2027.1.1, `texte-2027/bloc-S21.G00.11.txt` l. 232-237). La norme 2027 y nomme les SPST autonomes : « MT01 - MEDECINE
/// INTERNE ETABLISSEMENT » (note differentielle 2026.1.2 -> 2027.1.2, § 13,
/// `norme-dsn/dsn-note-differentielle-ct-2026.1.2-vers-2027.1.2-20251224.txt` l. 3497-3503), et en fait la condition du
/// contact de type 17 (`contact_sante_travail`) ; la norme 2026 n'a ni l'une ni l'autre. La fiche garde ce code pour la
/// DPAE (`medecine_travail`, « le code de votre service de prevention et de sante au travail »).
///
/// Rend `MT01` quand la fiche le dit et que la version du fichier est 2027 ou apres ; sinon rien. L'identifiant d'un
/// service EXTERNE ne s'ecrit pas encore : sa definition est « precisee dans la fiche consigne dediee », qui n'est pas
/// deposee, et que le code de la DPAE soit cet identifiant serait une inference (la rubrique est conditionnelle).
fn spst_ecrit(f: &Fichier, ent_e: &Value) -> Resultat<Option<String>> {
    let spst = txt(g(ent_e, "medecine_travail")).trim().to_string();
    if spst != "MT01" || f.norme()?.version.as_str() < "P27V01" || !usage_admet(f, "S21.G00.11.025", "01")? {
        return Ok(None);
    }
    Ok(Some(spst))
}

/// LE CONTACT DE LA MEDECINE DU TRAVAIL INTERNE (S20.G00.07 de type 17). Norme 2027 (cahier technique 2027.1.1,
/// `norme-dsn/texte-2027/bloc-S20.G00.07.txt` l. 46-65 et 81, CCH-11 de S20.G00.07.004) : « Si et seulement si la
/// declaration contient un bloc Etablissement - S21.G00.11 avec la rubrique Identifiant du SPST - S21.G00.11.025 renseignee
/// avec la valeur MT01 - MEDECINE INTERNE ETABLISSEMENT [...], alors un bloc Contact chez le declare - S20.G00.07 de type 17
/// [...] doit obligatoirement etre present. Dans le cas contraire, il est interdit. » ; c'est « le medecin du travail ou
/// un interlocuteur place sous sa responsabilite, pour la transmission des arrets de travail ». Il vient de la fiche
/// (`contact_sante_travail`, montre et demande quand le code vaut MT01, `champs-base.json`) ; sans lui, la DSN refuse.
fn contact_sante_travail(f: &mut Fichier, ent_e: &Value) -> Resultat<()> {
    if spst_ecrit(f, ent_e)?.as_deref() != Some("MT01") {
        return Ok(());
    }
    let c = g(ent_e, "contact_sante_travail");
    let manque: Vec<&str> = [("nom_prenom", "nom et prénom"), ("tel", "téléphone"), ("email", "courriel")]
        .into_iter()
        .filter(|(k, _)| txt(g(c, k)).trim().is_empty())
        .map(|(_, l)| l)
        .collect();
    if !manque.is_empty() {
        return Err(format!(
            "La médecine du travail de la compagnie est interne (code MT01) : la DSN déclare qui y répond, le médecin du travail ou une personne sous sa responsabilité (S20.G00.07, type 17). Manque : {}. Le saisir dans Configuration › Entreprise, « Qui répond pour la médecine du travail interne ». Rien n'est écrit.",
            manque.join(", ")
        ));
    }
    f.add_v("S20.G00.07.001", g(c, "nom_prenom"))?;
    f.add_v("S20.G00.07.002", g(c, "tel"))?;
    f.add_v("S20.G00.07.003", g(c, "email"))?;
    f.add("S20.G00.07.004", "17");
    Ok(())
}

/// `str(v or "")`.
pub fn txt(v: &Value) -> String {
    if vrai(v) {
        en_texte(v)
    } else {
        String::new()
    }
}

pub fn liste(v: &Value) -> &[Value] {
    v.as_array().map(Vec::as_slice).unwrap_or(&[])
}

fn zero() -> Decimal {
    Decimal::new(0, 2)
}

/// `"%.Nf" % x` : passe par le flottant.
fn fmt_f(x: Decimal, n: usize) -> String {
    let f: f64 = x.to_string().parse().unwrap_or(0.0);
    format!("{:.*}", n, f)
}

/// `_mt` : deux decimales, jamais « -0.00 ».
pub fn mt(x: Decimal) -> String {
    if x.is_zero() {
        "0.00".into()
    } else {
        fmt_f(x, 2)
    }
}

/// `_ms` : un montant signe.
pub fn ms(x: Decimal, signe: i64) -> String {
    mt(mul(x, Decimal::from(signe)))
}

/// A l'euro, la fraction egale a 0,50 comptee pour 1 (CSS L. 133-10) : `bordereau::euro`, comme les lignes 23.
fn rond(x: Decimal) -> Decimal {
    bordereau::euro(x)
}

pub fn jour(v: &Value) -> Resultat<Jour> {
    socle::date::d(v)
}

pub fn jj(v: &Value) -> Resultat<String> {
    Ok(jour(v)?.jjmmaaaa())
}

fn fin_mois(an: i32, mo: u32) -> Jour {
    Jour::nouveau(an, mo, socle::date::jours_du_mois(an, mo)).unwrap()
}

fn premier(mois: &str) -> Resultat<Jour> {
    socle::date::premier_du_mois(mois)
}

fn tuple(j: Jour) -> (i32, u32, u32) {
    (j.an, j.mois, j.jour)
}

fn cle_salarie(p: &Paie) -> String {
    txt(g(&p.salarie, "_cle"))
}

// ------------------------------------------------- les emplois multiples (S21.G00.40.036)

/// L'emploi qu'un contrat declare : ce que le bloc 40 en dit (PCS-ESE `40.004`, code emploi `40.005`, libelle
/// `40.006`), lu sur la fiche telle que l'engagement l'a faite (`emplois::emploi_de`).
fn emploi_declare(p: &Paie) -> (String, String, String) {
    let s = &p.salarie;
    (txt(g(s, "pcs_ese")), txt(g(s, "code_emploi_ft")), txt(g(s, "emploi")))
}

/// Les contrats enregistres d'une periode de paie (les engagements du mois, sans montants) ; rien si le mois n'a pas de
/// saisie. Ils completent ceux qu'une declaration porte : la DSN d'un etablissement ne porte pas les contrats d'un
/// autre, un signalement n'en porte qu'un.
pub fn contrats_du_mois(ctx: &Contexte, mois: &str) -> Resultat<Vec<Paie>> {
    Ok(engagement::embauches_du_mois(ctx, mois)?.unwrap_or_default())
}

/// `S21.G00.40.036` (code emplois multiples) du contrat `p` : « 02 - emplois multiples » si le salarie tient plus
/// d'un emploi chez la compagnie dans la periode de paie, « 01 - emploi unique » sinon. Cahier technique DSN 2026.1,
/// p. 199 (`norme-dsn/texte/bloc-S21.G00.40.txt`, lignes 1174-1183) : « Cette rubrique précise si le salarié a
/// plusieurs emplois chez un même employeur au cours d'une même période de paie. » `autres` : les contrats de la
/// meme periode (ceux de la declaration et ceux de la saisie), de tous les salaries ; seuls ceux de `p` comptent.
/// INFERENCE, ecrite comme telle : un emploi est ce que le bloc 40 decrit (PCS, code, libelle), la norme ne le
/// definit pas autrement ; c'est ce que CoolPaie ecrivait (02 si et seulement si deux codes emploi distincts le meme
/// mois, 148 contrats de son historique, fiche N1 de la relecture).
pub fn emplois_multiples(p: &Paie, autres: &[&Paie]) -> &'static str {
    let cle = cle_salarie(p);
    let mut emplois: BTreeSet<(String, String, String)> = autres.iter().filter(|q| cle_salarie(q) == cle).map(|q| emploi_declare(q)).collect();
    emplois.insert(emploi_declare(p));
    if emplois.len() > 1 {
        "02"
    } else {
        "01"
    }
}

// ------------------------------------------------- les etablissements

pub fn nics(fiche: &Value) -> Vec<String> {
    let mut out = vec![bareme::nic_principal(fiche)];
    out.extend(bareme::secondaires(fiche).iter().map(|e| en_texte(g(e, "nic"))));
    out
}

/// `etablissements.fiche_de` : l'entreprise vue depuis l'etablissement `nic`.
fn fiche_de(fiche: &Value, nic: &str) -> Resultat<Value> {
    if nic.is_empty() || nic == bareme::nic_principal(fiche) {
        return Ok(fiche.clone());
    }
    let e = bareme::entree(fiche, nic).ok_or_else(|| format!("Etablissement inconnu : {}.", nic))?;
    let mut vue = fiche.as_object().cloned().unwrap_or_default();
    let n = en_texte(req(&e, "nic")?);
    vue.insert("nic".into(), Value::String(n.clone()));
    vue.insert("siret".into(), Value::String(txt(g(fiche, "siren")) + &n));
    for cle in ["adresse", "code_postal", "commune", "naf", "adresse_affichage", "commune_affichage", "complement_adresse"] {
        if vrai(g(&e, cle)) {
            vue.insert(cle.into(), e[cle].clone());
        }
    }
    vue.insert("_etablissement".into(), Value::String(n));
    let mut siege = Map::new();
    siege.insert("nic".into(), Value::String(bareme::nic_principal(fiche)));
    siege.insert("siret".into(), g(fiche, "siret").clone());
    vue.insert("_siege".into(), Value::Object(siege));
    Ok(Value::Object(vue))
}

/// L'etablissement sous lequel un salarie se declare : la fiche entreprise vue depuis l'etablissement de rattachement
/// que dit SA fiche (`etablissement`, sinon le principal ; un NIC que la fiche entreprise ne porte pas refuse).
///
/// « Dans le cadre d'un contrat de travail, il s'agit de l'etablissement de rattachement administratif du salarie »
/// (S21.G00.11.001, cahier technique DSN 2026.1, p. 144, `atelier/reference/norme-dsn/texte/bloc-S21.G00.11.txt`). Le
/// NIC est une donnee identifiante : un signalement porte « les memes valeurs que celles portees dans la derniere DSN
/// mensuelle deposee » (§ 2.1.1, p. 18 ; § 2.5.1, p. 67, `chapitre-2-principes-de-constitution.txt`). La mensuelle
/// (une DSN par etablissement, qui refuse un salarie d'un autre, `generer`) et les signalements (arret, reprise, FCTU,
/// amorcage) le lisent donc au meme endroit : `bareme::etablissement_du_salarie`, puis `fiche_de`.
pub fn etablissement_du_salarie(ent: &Value, salarie: &Value) -> Resultat<Value> {
    fiche_de(ent, &bareme::etablissement_du_salarie(ent, salarie)?)
}

// ----------------------------------------------------- les entrees

/// Un arret de travail a reporter (`signalement.Arret`).
#[derive(Clone, Debug)]
pub struct Arret {
    pub v: Value,
}

impl Arret {
    fn s(&self, k: &str) -> String {
        txt(g(&self.v, k))
    }

    /// Une periode commune avec [debut, fin] : la regle du bloc 60 (`imprevus::arret_croise`).
    pub fn couvre(&self, debut: &Jour, fin: &Jour) -> Resultat<bool> {
        engagement::imprevus::arret_croise(&self.v, debut, fin)
    }

    /// L'arret suspend-il le contrat de cette paie ? La regle des imprevus (`imprevus::du_contrat`) : le meme salarie, la
    /// meme date de debut du contrat, le meme numero quand l'un et l'autre en ont un.
    fn du_contrat(&self, p: &Paie) -> bool {
        engagement::imprevus::du_contrat(&self.v, &cle_salarie(p), p.engagement.numero.as_deref(), &p.engagement.debut).unwrap_or(false)
    }
}

fn nomenclature_subrogation(libelle: &str) -> Resultat<String> {
    let t = json_en_cache(&dossier_du_code().join("arrets-base.json"))?;
    for c in liste(&t["subrogation"]["codes"]) {
        if txt(g(c, "libelle")) == libelle {
            return Ok(txt(g(c, "code")));
        }
    }
    Err(format!("subrogation : pas de code « {} » dans arrets-base.json", libelle))
}

/// Le bloc « Arret de travail - S21.G00.60 » (`signalement.bloc_60`).
///
/// Ce que chaque nature admet du bloc se lit dans le tableau des usages (cahier technique 2026.1, p. 363-382,
/// `atelier/reference/norme-dsn/texte/tableaux-des-usages.txt`, bloc S21.G00.60) : la subrogation (60.004 a 60.008) est
/// C en 01, O ou C en 04, I en 05 et en 07 ; la date de l'accident (60.012), I en 01 et en 07. `nature` est celle du
/// fichier, jamais une autre (reste R1 #42 : le FCTU passait « 01 »).
pub fn bloc_60(f: &mut Fichier, a: &Arret, nature: &str, banque: &Value) -> Resultat<()> {
    let v = &a.v;
    f.add_v("S21.G00.60.001", g(v, "motif"))?;
    f.add("S21.G00.60.002", jj(g(v, "dernier_jour_travaille"))?);
    f.add("S21.G00.60.003", jj(g(v, "fin_previsionnelle"))?);
    if usage_admet(f, "S21.G00.60.004", nature)? && vrai(g(v, "subrogation")) {
        f.add_v("S21.G00.60.004", g(v, "subrogation"))?;
        if a.s("subrogation") == nomenclature_subrogation("oui")? {
            if !vrai(g(banque, "iban")) || !vrai(g(banque, "bic")) {
                return Err("Subrogation demandee : l'IBAN et le BIC de la compagnie (S21.G00.60.007 et .008) sont \
obligatoires -- CCH-11. Ils se lisent dans la fiche entreprise, ils ne se saisissent pas dans l'arret."
                    .into());
            }
            f.add("S21.G00.60.005", jj(g(v, "subrogation_debut"))?);
            f.add("S21.G00.60.006", jj(g(v, "subrogation_fin"))?);
            f.add_v("S21.G00.60.007", &banque["iban"])?;
            f.add_v("S21.G00.60.008", &banque["bic"])?;
        }
    }
    if vrai(g(v, "reprise")) {
        f.add("S21.G00.60.010", jj(g(v, "reprise"))?);
        if vrai(g(v, "reprise_motif")) {
            f.add_v("S21.G00.60.011", g(v, "reprise_motif"))?;
        }
    }
    if usage_admet(f, "S21.G00.60.012", nature)? && vrai(g(v, "date_accident")) {
        f.add("S21.G00.60.012", jj(g(v, "date_accident"))?);
    }
    Ok(())
}

/// Une correction d'un mois depose : ce qui a ete declare, et la paie corrigee.
///
/// `declare` est le dernier etat DECLARE du contrat, ecrit depuis la paie que la DSN a declaree (`versement_declare::de_la_paie`) : les blocs negatifs
/// (50.002, 50.004, 51, 53, 54, 58, 78, 79, 81) l'annulent tel qu'il est parti, et l'agrege (bordereau Urssaf du mois
/// d'origine, versements 20) en retranche ce que ses lignes 78 et 81 recomposent (`assiette_declaree`,
/// `cumuler_le_declare`) ; le bloc 55 se lit dans les blocs negatifs ecrits (`composants_de_versement`). `avant` est
/// la paie d'avant refaite par le moteur du jour : elle ne donne plus que le contrat (ses bornes, sa nature, son statut,
/// sa pile de bareme), aucun montant.
pub struct Regularisation {
    pub mois: String,
    pub avant: Paie,
    pub declare: crate::versement_declare::VersementDeclare,
    pub apres: Paie,
    pub taux_pas_origine: Option<String>,
    /// Le net a payer avant impot deja verse au salarie pour ce contrat, lu dans la derniere paie declaree
    /// (`produire_regularisations::versement_declare_de`) : ce qui dit si la correction verse ou reprend quelque chose,
    /// et a quel montant net social correspondait ce qui a ete paye (`blocs_58_de_la_correction`).
    pub net_deja_verse: Decimal,
    /// Le jour ou la correction a ete ecrite (`decidee_le`) : la date de la modification (S21.G00.41.001) quand elle
    /// change la date de debut du contrat (`changements_contrat::ancienne_date_de_debut`). `None` : inconnu.
    pub decidee_le: Option<Jour>,
}

/// Les parametres de `generer`, avec leurs defauts.
pub struct Options {
    pub date_constitution: Option<Jour>,
    pub fraction: String,
    pub type_declaration: String,
    pub envoi_test: bool,
    pub ordre: i64,
    pub remplace: Option<String>,
    pub etablissement: Option<String>,
    /// Les taux imposes : cle -> triplet, ou `None` (la grille).
    pub pas_par_salarie: HashMap<String, Option<Triplet>>,
    /// Sur un annule et remplace (type 03 ou 05) : la declaration remplacee portait-elle un versement DGFiP
    /// (S21.G00.20.002 = DGFIP_PAS) ? `None` : on ne sait pas, et la DSN refuse s'il n'y a rien a reverser ce mois-ci.
    pub remplacee_versait_pas: Option<bool>,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            date_constitution: None,
            fraction: "11".into(),
            type_declaration: "01".into(),
            envoi_test: false,
            ordre: 1,
            remplace: None,
            etablissement: None,
            pas_par_salarie: HashMap::new(),
            remplacee_versait_pas: None,
        }
    }
}

// ------------------------------------------------------- les cotisations

/// Une cotisation d'un versement (`dsn.cotisations`).
#[derive(Clone, Debug)]
pub struct Cotisation {
    pub bloc78: String,
    pub base78: Decimal,
    pub code81: String,
    pub org: Option<String>,
    pub affectation: Option<String>,
    pub cle_total: Option<String>,
    pub code_insee: Option<String>,
    pub base81: Option<Decimal>,
    pub taux: Option<Value>,
    pub montant: Option<Decimal>,
    /// Le composant d'une base 31 (S21.G00.79.001, et son montant 79.004) quand la source du taux de la ligne le dit
    /// (`_composant_79`, `bareme::bareme_sante`) ; `None` : la tranche A, sur le brut.
    pub composant79: Option<(String, Decimal)>,
    /// La part patronale de la CET (`parts_patronales_agirc_arrco`) : son code se lit dans la version de la norme du
    /// fichier (`Norme::cotisation_cet_patronale`), pas dans `code81`.
    pub cet: bool,
}

fn opt_txt(v: &Value) -> Option<String> {
    if v.is_null() {
        None
    } else {
        Some(en_texte(v))
    }
}

/// Les assiettes que le calcul a ecrites dans la paie, telles quelles : rien ne se recalcule.
pub fn bases(p: &Paie, bar: &Value) -> Resultat<HashMap<String, Decimal>> {
    let b = &p.bases;
    let mut out = HashMap::new();
    let lue = |cle: &str| -> Resultat<Decimal> { b.get(cle).copied().ok_or_else(|| format!("paie ecrite illisible : la base `{}`", cle)) };
    out.insert("brut".into(), p.brut);
    out.insert("brut_cs".into(), lue("brut_cs")?);
    out.insert("csg".into(), p.base_csg);
    out.insert("brut_x115".into(), lue("brut_x115")?);
    out.insert("brut_x110".into(), lue("brut_x110")?);
    out.insert("zero".into(), zero());
    if vrai(g(bar, "plafond_journalier")) {
        out.insert("t1_journaliere".into(), lue("t1_journaliere")?);
    }
    // Les assiettes plafonnees d'un intermittent (`calcul::calculer`, 7.6) : les bases 02 et 07 portent ce que le
    // bulletin a calcule, sur le meme plafond (point 22).
    for cle in [
        "t1_mensuelle", "t2_mensuelle", "apec_mensuelle", "cet_mensuelle", "pss_periode", "t2_conventionnelle",
        "brut_plafonne", "brut_x115_plafonne", "brut_chomage", "t1_mensuelle_x115",
    ] {
        if let Some(x) = b.get(cle) {
            out.insert(cle.into(), *x);
        }
    }
    // Les assiettes abattues d'une paie sous deduction forfaitaire specifique (`calcul::calculer`, 7.5 bis), que
    // `cle_abattue` fait lire aux bases de securite sociale et d'Agirc-Arrco.
    if p.deduction_forfaitaire.is_some() {
        for cle in ["brut_dfs", "brut_x115_dfs", "brut_x110_dfs", "brut_plafonne_dfs", "brut_x115_plafonne_dfs"] {
            if let Some(x) = b.get(cle) {
                out.insert(cle.into(), *x);
            }
        }
    }
    Ok(out)
}

fn base<'a>(bases: &'a HashMap<String, Decimal>, cle: &Value) -> Resultat<Decimal> {
    let k = en_texte(cle);
    bases.get(&k).copied().ok_or_else(|| socle::fichiers::absent(&k))
}

/// Les bases assujetties que la deduction forfaitaire specifique abat (11/10/2026 ;
/// `atelier/reference/sources/dfs/LISEZ-MOI.md`, « La DSN ») : 02 et 03, l'assiette des cotisations de securite sociale
/// (CSS R. 242-1, al. 3 ; arrete du 4 septembre 2025 relatif aux frais professionnels, art. 1er et 9), et 23 et 43,
/// celles d'Agirc-Arrco (cahier d'aide a la codification Agirc-Arrco CT2026, l. 3490-3498 : « La solution ainsi retenue
/// pour le regime general de la Securite sociale s'applique en principe au regime AGIRC-ARRCO »). Que la base 78 porte
/// l'assiette abattue, et pas seulement son 81.003 : le meme cahier declare la « Base VRP multicartes apres abattement
/// pour frais professionnels » en 02 et 03 (l. 6230-6236), et la norme 2026.1, chapitre 2 (p. 60-61,
/// `atelier/reference/norme-dsn/texte/chapitre-2-principes-de-constitution.txt` l. 2116-2160), part du « montant brut [...]
/// servant de base au calcul des cotisations de securite sociale » et y AJOUTE « le montant de l'abattement forfaitaire
/// d'assiette pour frais professionnels » : ce brut-la est abattu. INFERENCE, ecrite comme telle : aucun texte depose ne
/// le dit d'un artiste en toutes lettres (la fiche net-entreprises 1336 ne dit rien des bases). Ne sont pas abattues :
/// 04 (la CSG, son propre abattement), 07 (le chomage d'un intermittent, « avant application de l'abattement », annexes
/// VIII et X, art. 49), 27 (les Conges Spectacles, « avant toute deduction forfaitaire specifique », bloc 78 p. 292) et
/// 31 (la prevoyance, dont les textes ne renvoient pas a l'assiette de la securite sociale).
pub const BASES_ABATTUES_PAR_LA_DFS: [&str; 4] = ["02", "03", "23", "43"];

/// La cle de `bases` que lit une base 78 de code `bloc78`, ou une cotisation qu'elle porte, pour cette paie : sous
/// deduction forfaitaire specifique et sous une base de `BASES_ABATTUES_PAR_LA_DFS`, la forme abattue que le bulletin a
/// calculee (`brut_dfs`...). Une cle sans forme abattue refuse : la DSN ne se fait pas sur une assiette que le bulletin
/// n'a pas.
pub fn cle_abattue(p: &Paie, bloc78: &str, cle: &str) -> Resultat<String> {
    if p.deduction_forfaitaire.is_none() || !BASES_ABATTUES_PAR_LA_DFS.contains(&bloc78) || cle == "zero" {
        return Ok(cle.to_string());
    }
    let abattue = format!("{}_dfs", cle);
    if p.bases.contains_key(&abattue) {
        return Ok(abattue);
    }
    Err(format!(
        "{} : la deduction forfaitaire specifique abat la base {}, mais le calcul n'a pas de forme abattue de << {} >> \
(atelier/reference/sources/dfs/LISEZ-MOI.md). Rien n'est ecrit.",
        qui_contrat(p),
        bloc78,
        cle
    ))
}

/// Le brut que la paie declare en 51 de type 001 : sous deduction forfaitaire specifique, le brut abattu. Guide Urssaf
/// « Comment declarer et regulariser les cotisations Urssaf en DSN » du 04/06/2026, p. 36
/// (`atelier/reference/sources/editeur-dsn/guide-urssaf-regularisation.txt`, l. 1440-1459) : les revenus bruts du montant
/// net social se recomposent par « 51 de type 001 + 54 de type 10 - 54 de type 35 » ; le BOSS (montant net social, Q 26,
/// `atelier/reference/sources/montant-net-social/`, l. 387-393) veut le brut entier, « independamment des abattements » :
/// le 51 de type 001 est donc le brut moins la deduction que porte le 54 de type 10 (`autre_revenu`).
pub fn brut_du_51(p: &Paie) -> Decimal {
    match (&p.deduction_forfaitaire, p.bases.get("brut_dfs")) {
        (Some(_), Some(abattu)) => *abattu,
        _ => p.brut,
    }
}

fn somme_lignes(p: &Paie, ids: &Value, patronale_seule: bool) -> Decimal {
    let ids: Vec<String> = liste(ids).iter().map(en_texte).collect();
    p.lignes
        .iter()
        .filter(|l| ids.contains(&l.id))
        .fold(zero(), |s, l| add(s, if patronale_seule { l.montant_pat } else { add(l.montant_pat, l.montant_sal) }))
}

/// La pile de bareme de cette paie, sur le bareme du mois choisi.
fn bareme_de(p: &Paie, bar: &Value) -> Resultat<Value> {
    let nat = p.engagement.nat()?;
    let fin = p.engagement.fin_periode()?;
    let cpf_cdd = droit::cpf_cdd::contrat_ouvre_cpf_cdd(
        &nat,
        p.engagement.motif_recours.as_deref(),
        &format!("Contrat {} de {}", p.engagement.numero.as_deref().unwrap_or("sans numero"), p.engagement.salarie),
    )?;
    bareme_applique_au_contrat(bar.clone(), &p.salarie, &nat, Some(&fin), p.declaree_a_la_caisse_des_conges()?, cpf_cdd)
}

/// Les cotisations d'un versement, dans l'ordre du bareme.
pub fn cotisations(p: &Paie, bar: &Value) -> Resultat<Vec<Cotisation>> {
    let contribs = liste(g(bar, "_contributions_du_mois")).to_vec();
    let exclues = places_non_affiliees(p, g(bar, "_entreprise"))?;
    let bar = bareme_de(p, bar)?;
    let bases = bases(p, &bar)?;
    let mut out: Vec<Cotisation> = Vec::new();
    for c in liste(req(&bar, "cotisations_dsn")?) {
        let seuil = g(c, "seuil_smic");
        if !seuil.is_null() && !au_dela_du_seuil(&bar, p.brut, p.heures_travaillees, Some(seuil))? {
            continue;
        }
        let bloc78 = en_texte(req(c, "bloc78")?);
        let abattue = |cle: &Value| -> Resultat<Decimal> { base(&bases, &Value::String(cle_abattue(p, &bloc78, &en_texte(cle))?)) };
        let mut l = Cotisation {
            bloc78: bloc78.clone(),
            base78: abattue(req(c, "base78")?)?,
            code81: en_texte(req(c, "code81")?),
            org: opt_txt(g(c, "org")),
            affectation: opt_txt(g(c, "affectation")),
            cle_total: None,
            code_insee: None,
            base81: None,
            taux: None,
            montant: None,
            composant79: None,
            cet: false,
        };
        if vrai(g(c, "cle_total")) {
            l.cle_total = Some(en_texte(&c["cle_total"]));
        }
        if vrai(g(c, "code_insee")) {
            l.code_insee = Some(en_texte(&c["code_insee"]));
        }
        if vrai(g(c, "lignes")) {
            l.montant = Some(somme_lignes(p, &c["lignes"], false));
            l.composant79 = composant_79(p, &bar, &c["lignes"]);
        } else if !req(c, "taux")?.is_null() {
            let b = abattue(req(c, "base")?)?;
            l.base81 = Some(b);
            l.taux = Some(c["taux"].clone());
            l.montant = Some(if vrai(g(c, "composants")) {
                liste(&c["composants"]).iter().try_fold(zero(), |s, t| Ok::<_, String>(add(s, pct(b, dec_avec_echelle(t)?))))?
            } else {
                pct(b, dec_avec_echelle(&c["taux"])?)
            });
        }
        let unifie = l.code81 == AGIRC_ARRCO_UNIFIE;
        out.push(l);
        if unifie {
            let l131 = out[out.len() - 1].clone();
            out.extend(parts_patronales_agirc_arrco(p, &bar, c, &l131)?);
        }
    }
    let nat = p.engagement.nat()?;
    let nature = nat.bareme.clone().unwrap_or_else(|| "intermittent".into());
    for contrib in &contribs {
        let natures = g(contrib, "natures");
        if natures.get(&nature).is_none() {
            continue;
        }
        let dernier = out
            .iter()
            .enumerate()
            .filter(|(_, l)| l.affectation.as_deref().map_or(false, |a| !a.is_empty()))
            .map(|(i, _)| i as i64)
            .max()
            .unwrap_or(out.len() as i64 - 1);
        out.insert(
            (dernier + 1) as usize,
            Cotisation {
                bloc78: "31".into(),
                base78: bases["zero"],
                code81: "059".into(),
                org: None,
                affectation: opt_txt(&natures[&nature]),
                cle_total: None,
                code_insee: None,
                base81: None,
                taux: None,
                montant: Some(somme_lignes(p, g(contrib, "lignes"), false)),
                composant79: None,
                cet: false,
            },
        );
    }
    if let Some(rg) = &p.reduction_generale {
        if !rg.montant.is_zero() {
            let dsn = g(g(&bar, "reduction_generale"), "dsn");
            let par = |k: &str, defaut: &str| -> String {
                let v = g(dsn, k);
                if v.is_null() { defaut.to_string() } else { en_texte(v) }
            };
            let org_de = |code: &str| -> Option<String> {
                liste(g(&bar, "cotisations_dsn"))
                    .iter()
                    .find(|c| txt(g(c, "code81")) == code)
                    .and_then(|c| opt_txt(g(c, "org")))
            };
            let urssaf = org_de("075");
            let ft = org_de("040");
            // Sous la base 03 : abattue sous deduction forfaitaire specifique (`cle_abattue`), comme la reduction
            // elle-meme se calcule sur le brut abattu (`droit::reduction::reduction_generale_sous_dfs`).
            let brut_03 = bases[&cle_abattue(p, "03", "brut")?];
            for (code, org, part, cle) in [
                (par("code81_urssaf", "018"), urssaf, rg.urssaf, par("cle_total_urssaf", "018:urssaf")),
                (par("code81_chomage", "018"), ft, rg.chomage, par("cle_total_chomage", "018:chomage")),
                (par("code81_retraite", "106"), None, rg.agirc_arrco, "106".to_string()),
            ] {
                if !part.is_zero() {
                    out.push(Cotisation {
                        bloc78: "03".into(),
                        base78: brut_03,
                        code81: code,
                        org,
                        affectation: None,
                        cle_total: Some(cle),
                        code_insee: None,
                        base81: Some(brut_03),
                        taux: None,
                        montant: Some(-part),
                        composant79: None,
                        cet: false,
                    });
                }
            }
        }
    }
    // La base 31 d'une affiliation que le salarie n'a pas ce mois-la (`rattachements`) ne se declare pas : elle citerait
    // (78.005) un bloc 70 absent. Sa cotisation est nulle -- sinon la paie et l'affiliation se contredisent.
    let mut gardees = Vec::with_capacity(out.len());
    for c in out {
        let place = c.affectation.as_deref().and_then(|a| a.trim().parse::<usize>().ok()).and_then(|a| a.checked_sub(202));
        if c.bloc78 == "31" && place.map_or(false, |x| exclues.contains(&x)) {
            if c.montant.map_or(false, |m| !m.is_zero()) {
                return Err(format!(
                    "{} : une cotisation {} de {} sous une affiliation prevoyance que le salarie n'a pas ce mois-la \
(S21.G00.70). La paie et l'affiliation se contredisent ; rien n'est declare.",
                    qui_contrat(p),
                    c.code81,
                    mt(c.montant.unwrap_or_default())
                ));
            }
            continue;
        }
        gardees.push(c);
    }
    Ok(regrouper_par_base(gardees))
}

/// Le composant de la base 31 des `lignes` : le type que la source du taux de la premiere qui en porte un lui donne
/// (`_composant_79` de la pile, `bareme::bareme_sante`), et la base de cette ligne au bulletin.
fn composant_79(p: &Paie, pile: &Value, lignes: &Value) -> Option<(String, Decimal)> {
    let rubriques = liste(g(pile, "rubriques_bulletin"));
    liste(lignes).iter().map(en_texte).find_map(|id| {
        let type_79 = rubriques.iter().find(|r| txt(g(r, "id")) == id).map(|r| txt(g(r, "_composant_79"))).filter(|t| !t.is_empty())?;
        let base = p.lignes.iter().find(|l| l.id == id)?.base;
        Some((type_79, base))
    })
}

/// « 131 - Cotisation regime unifie Agirc-Arrco » : modalite Agirc-Arrco, parts salariale et patronale confondues
/// (cahier technique 2026.1, p. 302 lignes 80-81 et p. 305 ligne 287, `atelier/reference/norme-dsn/texte/bloc-S21.G00.81.txt`).
pub const AGIRC_ARRCO_UNIFIE: &str = "131";
/// « 142 - Cotisation regime unifie Agirc-Arrco (part patronale tranche T1) » et « 146 - [...] (part patronale tranche
/// T2) » : deux codes de la modalite URSSAF de 81.001 (p. 302, lignes 106-112 ; libelles p. 305, lignes 292 et 296), pas
/// de la modalite Agirc-Arrco (lignes 80-81). SIG-18 (p. 303, lignes 158-163) : un 131 sous une base 78 datee dans le
/// mois principal veut au moins un 142 sous le meme versement.
pub const AGIRC_ARRCO_PATRONALE_T1: &str = "142";
pub const AGIRC_ARRCO_PATRONALE_T2: &str = "146";
/// Les lignes du bulletin dont un 131 fait la somme : la retraite des deux tranches (CEG comprise) et la CET.
const LIGNES_AGIRC_ARRCO: [&str; 3] = ["retraite_t1", "retraite_t2", "cet"];

/// La part patronale d'une ligne du bulletin, et ce qui y est de la CET. Un cadre intermittent paie la CET FONDUE dans ses
/// lignes de tranche 1 et de tranche 2 (`cet_si_depasse`, `calcul::calculer` 7.7 : le dernier composant patronal prend la
/// CET en plus) ; la retraite et la CEG seules sont ce que la ligne vaudrait sans elle, au meme arrondi (chaque composant
/// sur la base de la ligne), et la CET le reste. Une ligne que ce calcul ne retrouve pas au centime refuse : sa CET ne
/// se separe pas sans deviner.
fn sans_la_cet(p: &Paie, l: &engagement::calcul::LigneCotisation, r: &Value) -> Resultat<(Decimal, Option<(Decimal, f64)>)> {
    let cet = g(r, "cet_si_depasse");
    if !vrai(cet) || p.bases.get("cet").map_or(true, |b| b.is_zero()) {
        return Ok((l.montant_pat, None));
    }
    let cet_pat = socle::num::flottant(req(cet, "pat")?)?;
    let comp = match droit::bareme::taux_liste(r.get("composants_pat"))? {
        Some(c) => c,
        None => vec![socle::num::flottant(req(r, "taux_pat")?)?],
    };
    let sans = droit::bareme::somme_pct(l.base, &comp)?;
    let mut avec = comp;
    if let Some(dernier) = avec.last_mut() {
        *dernier += cet_pat;
    }
    if droit::bareme::somme_pct(l.base, &avec)? != l.montant_pat {
        return Err(format!(
            "La ligne « {} » du bulletin ({} EUR de part patronale sur {} EUR) ne se refait pas, CET comprise, a partir de \
ses taux : la part de la CET (146 en 2026, 148 en 2027) ne s'en separe pas sans deviner. Rien n'est ecrit.",
            l.id,
            mt(l.montant_pat),
            mt(l.base)
        ));
    }
    Ok((sans, Some((sub(l.montant_pat, sans), cet_pat))))
}

/// Les parts patronales Agirc-Arrco que l'Urssaf attend a cote d'un 131, sous la meme base 78. Guide Urssaf « Comment
/// declarer et regulariser les cotisations Urssaf en DSN », version du 04/06/2026
/// (`atelier/reference/sources/editeur-dsn/guide-urssaf-regularisation.pdf`, transcription `.txt` a cote), p. 34 et
/// p. 37-38 : « le montant des cotisations parts patronales de retraite complementaire Agirc Arrco (tranche 1) et de CEG
/// (Tranche 1) ainsi que le taux sont a declarer [...] sous le code 142 » ; « le montant des cotisations parts patronales
/// de retraite complementaire Agirc-Arrco (Tranche 2), de CEG (Tranche 2) et de CET (tranches 1 et 2) ainsi que le taux
/// [...] sous le code 146 ». Le 131 garde les deux parts : le meme guide recalcule la retraite complementaire SALARIALE
/// de tranche 1 comme « 81.004 pour le type 131 - 81.004 pour le type 142 » (p. 36, tableau du montant net social).
///
/// Chacune porte l'organisme Urssaf (81.002 « SIRET de l'Urssaf », p. 307), son assiette (81.003, « a renseigner pour une
/// cotisation », p. 308) et son taux (81.007) : ceux de la ligne du bulletin. La CET n'a pas la base de la tranche 2 : elle
/// part dans un 146 a elle, ce que le guide admet (« il est tolere de multiplier les blocs [...] 146 [...] pour declarer
/// separement chacun des elements sous ce meme code », p. 38). Le bloc 78 est celui du 131 : « Chaque cotisation
/// individuelle est rattachee a la base assujettie juridique correspondante » (p. 5), et la modalite Urssaf de 78.001
/// admet la base 23 (norme p. 293, ligne 133). Une part nulle ne s'ecrit pas. En 2027 la CET a son code, 148, « entierement
/// sous l'assiette deplafonnee » (03, 22 ou 23 ; fiche net-entreprises 2556, l. 83-87) : le code s'ecrit selon la version
/// du fichier (`Cotisation::cet`, `Norme::cotisation_cet_patronale`), la base reste celle du 131 (type 23).
fn parts_patronales_agirc_arrco(p: &Paie, pile: &Value, c: &Value, l131: &Cotisation) -> Resultat<Vec<Cotisation>> {
    let rubriques = liste(req(pile, "rubriques_bulletin")?);
    let rubrique = |id: &str| rubriques.iter().find(|r| txt(g(r, "id")) == id);
    // Le 131 nomme ses lignes, ou porte un taux sur l'assiette de la ligne de retraite qu'il declare (`bareme::poser_t1`).
    let ids: Vec<String> = if vrai(g(c, "lignes")) {
        liste(&c["lignes"]).iter().map(en_texte).collect()
    } else {
        let base = g(c, "base");
        LIGNES_AGIRC_ARRCO.iter().filter(|id| rubrique(id).map_or(false, |r| g(r, "base") == base)).map(|s| s.to_string()).collect()
    };
    if ids.is_empty() || ids.iter().any(|id| !LIGNES_AGIRC_ARRCO.contains(&id.as_str())) {
        return Err(format!(
            "Le bareme declare un 131 (Agirc-Arrco) fait {} : sa part patronale (142, 146), que l'Urssaf attend sous le \
meme versement (cahier technique 2026.1, SIG-18 p. 303), ne se lit que sur les lignes de retraite du bulletin \
({}). Rien n'est ecrit.",
            if ids.is_empty() { "d'aucune ligne du bulletin".to_string() } else { format!("des lignes {}", ids.join(", ")) },
            LIGNES_AGIRC_ARRCO.join(", ")
        ));
    }
    let org = liste(req(pile, "cotisations_dsn")?)
        .iter()
        .find(|x| txt(g(x, "code81")) == "075" && vrai(g(x, "org")))
        .map(|x| en_texte(&x["org"]))
        .ok_or_else(|| "La part patronale Agirc-Arrco (142, 146) se declare a l'Urssaf (81.002), et le bareme ne nomme l'Urssaf sur aucune cotisation 075. Rien n'est ecrit.".to_string())?;
    let part = |code: &str, base: Decimal, taux: f64, montant: Decimal| Cotisation {
        bloc78: l131.bloc78.clone(),
        base78: l131.base78,
        code81: code.into(),
        org: Some(org.clone()),
        affectation: l131.affectation.clone(),
        cle_total: None,
        code_insee: None,
        base81: Some(base),
        taux: Some(serde_json::json!(taux)),
        montant: Some(montant),
        composant79: None,
        cet: false,
    };
    let mut out = Vec::new();
    let (mut cet_montant, mut cet_base, mut cet_taux) = (zero(), zero(), None::<f64>);
    for id in &ids {
        // La CET d'un non cadre n'existe pas ; une tranche 2 nulle n'est pas au bulletin (`calcul::calculer`).
        let (Some(r), Some(l)) = (rubrique(id), p.lignes.iter().find(|l| &l.id == id)) else { continue };
        let (montant, cet) = sans_la_cet(p, l, r)?;
        let taux = match &cet {
            Some(_) => socle::num::flottant(req(r, "taux_pat")?)?,
            None => l.taux_pat.ok_or_else(|| format!("La ligne « {} » du bulletin n'a pas de taux patronal : sa part Agirc-Arrco ne se declare pas.", id))?,
        };
        if let Some((m, t)) = cet {
            if cet_taux.map_or(false, |x| x != t) {
                return Err(format!("La CET des lignes {} n'a pas un seul taux patronal : sa part patronale (146 en 2026, 148 en 2027) ne s'ecrit pas.", ids.join(", ")));
            }
            cet_taux = Some(t);
            cet_montant = add(cet_montant, m);
            cet_base = add(cet_base, l.base);
        }
        if !montant.is_zero() {
            let code = if id == "retraite_t1" { AGIRC_ARRCO_PATRONALE_T1 } else { AGIRC_ARRCO_PATRONALE_T2 };
            // La ligne « cet » d'un permanent est toute de la CET ; celle d'un cadre intermittent est fondue dans ses
            // tranches et se separe plus bas (`sans_la_cet`).
            out.push(Cotisation { cet: id == "cet", ..part(code, l.base, taux, montant) });
        }
    }
    if let (Some(t), false) = (cet_taux, cet_montant.is_zero()) {
        out.push(Cotisation { cet: true, ..part(AGIRC_ARRCO_PATRONALE_T2, cet_base, t, cet_montant) });
    }
    Ok(out)
}

/// Les cotisations d'une meme base se suivent, pour s'ecrire sous un seul bloc 78. Une base est la « Somme des
/// montants assujettis de maniere homogene a une ou plusieurs cotisations sociales » (norme DSN 2026.1, bloc 78,
/// p. 291 ; `norme-dsn/texte/bloc-S21.G00.78.txt:60`) : la reduction generale (018, 106) et le versement mobilite
/// (081), ajoutes apres les bases 31 et 43, ouvraient sinon un second bloc 03 au meme montant sous le meme
/// versement, et la base 03 nominative valait le double. Ordre stable : une ligne rejoint la derniere de sa base,
/// les autres ne bougent pas.
fn regrouper_par_base(lignes: Vec<Cotisation>) -> Vec<Cotisation> {
    let mut out: Vec<Cotisation> = Vec::with_capacity(lignes.len());
    for l in lignes {
        let meme = |c: &Cotisation| c.bloc78 == l.bloc78 && c.affectation == l.affectation && c.base78 == l.base78;
        match out.iter().rposition(meme) {
            Some(i) => out.insert(i + 1, l),
            None => out.push(l),
        }
    }
    out
}

// ------------------------------------------------ adhesions et affiliations

/// L'adhesion `reference` du fichier des organismes vaut-elle pour la periode `debut`..=`fin` (`droit::adhesions`) ?
/// Une reference que le fichier ne porte pas (une contribution de branche du bareme) n'a pas de date.
pub fn adhesion_en_vigueur(orgs: &Value, reference: &str, debut: &Jour, fin: &Jour) -> Resultat<bool> {
    match liste(g(orgs, "adhesions_prevoyance")).iter().find(|a| en_texte(g(a, "reference")) == reference) {
        Some(a) => droit::adhesions::en_vigueur(a, debut, fin),
        None => Ok(true),
    }
}

/// Le premier et le dernier jour du mois `AAAA-MM`.
fn bornes_du_mois(mois: &str) -> Resultat<(Jour, Jour)> {
    Ok((socle::date::premier_du_mois(mois)?, socle::date::dernier_du_mois(mois)?))
}

/// `_affiliations` : les references d'adhesion auxquelles cette nature rattache, parmi celles qui valent pour la
/// periode `debut`..=`fin`.
pub fn affiliations(orgs: &Value, nat: &Nature, contribs: &Value, (debut, fin): (Jour, Jour)) -> Resultat<Vec<String>> {
    let cle = nat.bareme.clone().unwrap_or_else(|| "intermittent".into());
    let sec = g(g(orgs, "affiliations_par_nature"), &cle);
    if !vrai(sec) {
        return Err(format!(
            "config/organismes.json : `affiliations_par_nature` ne dit rien de '{}' (contrat {}). Le bloc \
S21.G00.70.013 rattache le salarie a une adhesion prevoyance ; il ne se devine pas.",
            cle, nat.libelle
        ));
    }
    let refs = g(sec, "references");
    if !vrai(refs) {
        let pourquoi = if vrai(g(sec, "_a_confirmer")) { en_texte(&sec["_a_confirmer"]) } else { "A relever.".into() };
        return Err(format!(
            "Contrat {} : les adhesions prevoyance d'un contrat '{}' ne sont pas confirmees (config/organismes.json, \
`affiliations_par_nature.{}.references` est `null`).\n  {}\n  Declarer quand meme le rattacherait aux adhesions \
d'une autre population -- ce que la plate-forme accepte sans rien dire.",
            nat.libelle, cle, cle, pourquoi
        ));
    }
    let mut out: Vec<String> = Vec::new();
    for r in liste(refs).iter().map(en_texte) {
        if adhesion_en_vigueur(orgs, &r, &debut, &fin)? {
            out.push(r);
        }
    }
    for c in liste(contribs) {
        if g(c, "natures").get(&cle).is_some() {
            out.push(en_texte(g(c, "reference")));
        }
    }
    Ok(out)
}

/// Une affiliation que le contrat declare (bloc S21.G00.70), dans l'ordre de `affiliations`.
pub struct Affiliation {
    /// Sa place parmi les references de la nature : elle fait l'identifiant (70.012) et l'affectation de sa base 31
    /// (`202 + place`, les `cotisations_dsn` du bareme). Pour un permanent, 0 est la prevoyance et 1 la sante -- l'ordre
    /// que `organismes::composer` ecrit et que le socle attend.
    pub place: usize,
    /// La reference de l'adhesion (70.013 = 15.005).
    pub reference: String,
    /// L'option retenue par le salarie (70.004), quand l'organisme la demande pour ce contrat.
    pub option: Option<String>,
    /// Les ayants droit conjoint (70.008) et enfants (70.011), quand ils entrent dans le calcul de la cotisation.
    pub ayants_droit: Option<(i64, i64)>,
    /// La date de debut de l'affiliation (70.014), quand ce n'est pas celle du contrat.
    pub debut: Option<Jour>,
}

/// Ce qui affilie un permanent, ce mois-la, aux deux adhesions de sa nature : la prevoyance de sa convention, selon la
/// regle de sa cotisation (`bareme::affiliation_prevoyance`), et la sante, sauf dispense.
struct AffiliationsDuPermanent {
    prevoyance: bareme::AffiliationPrevoyance,
    sante: bool,
}

/// `None` pour un contrat qui n'est pas paye au mois : un intermittent est affilie a toutes les adhesions de sa nature.
fn affiliations_du_permanent(p: &Paie, ent: &Value) -> Resultat<Option<AffiliationsDuPermanent>> {
    if !p.engagement.nat()?.mensuelle() {
        return Ok(None);
    }
    let fin = p.engagement.fin_periode()?;
    let prevoyance = bareme::affiliation_prevoyance(ent, &p.salarie, &fin)?;
    // La dispense (CCNEAC XII.2.1.8 ; 3090, accord du 19/07/2024, art. 9.2 ; `sante_dispense`) : le bulletin n'a plus
    // de mutuelle (`bareme_sante`), et le
    // salarie n'est pas rattache au contrat de sante -- 70.014/.015 decrivent la dispense comme ce qui ouvre ou ferme
    // l'affiliation (cahier technique 2026.1, p. 287).
    let dispense = vrai(g(&p.salarie, "sante_dispense"));
    // Un mois entierement suspendu sans indemnisation, garanties suspendues (`calcul::sante_de_la_suspension` : 3090,
    // accord sante du 19/07/2024, art. 4.2 et 15) : pas de cotisation, et pas de rattachement a la sante ce mois-la --
    // comme une dispense. INFERENCE NOMMEE : aucune fiche DSN deposee ne dit comment declarer une affiliation sante
    // suspendue (pas de bloc 70 ? un bloc 70 avec une fin de rattachement ? une base 31 a zero ?) ; le moteur n'ecrit ni
    // bloc 70 ni base 31 de sante pour ce mois (`atelier/reference/sources/suspension-non-remuneree/RAPPORT-20261015.md`,
    // point 2 et « Ce qui reste ouvert », 3).
    let suspendue = vrai(g(&p.salarie, droit::bareme::SANTE_SUSPENDUE));
    if suspendue && p.lignes.iter().any(|l| l.id == "mutuelle") {
        return Err(format!(
            "{} : sa sante est suspendue ce mois-ci ({}) et le bulletin cotise a la mutuelle. Rien n'est declare.",
            qui_contrat(p),
            txt(g(&p.salarie, droit::bareme::SANTE_SUSPENDUE))
        ));
    }
    if dispense && p.lignes.iter().any(|l| l.id == "mutuelle") {
        return Err(format!(
            "{} : la fiche porte une dispense de complementaire sante (`sante_dispense` : « {} ») et le bulletin cotise a \
la mutuelle (convention {}) : il a ete calcule sans elle. La DSN ne choisit pas entre l'affiliation et la dispense : \
recalculer le bulletin si le salarie est dispense, retirer `sante_dispense` de la fiche s'il est affilie. Rien n'est \
declare.",
            qui_contrat(p),
            txt(g(&p.salarie, "sante_dispense")),
            txt(g(ent, "idcc"))
        ));
    }
    Ok(Some(AffiliationsDuPermanent { prevoyance, sante: !dispense && !suspendue }))
}

/// Les places (`Affiliation::place`) des adhesions de sa nature auxquelles le salarie n'est pas affilie ce mois-la.
fn places_non_affiliees(p: &Paie, ent: &Value) -> Resultat<Vec<usize>> {
    let mut out = Vec::new();
    if let Some(a) = affiliations_du_permanent(p, ent)? {
        if a.prevoyance == bareme::AffiliationPrevoyance::NonAffilie {
            out.push(0);
        }
        if !a.sante {
            out.push(1);
        }
    }
    Ok(out)
}

/// Les affiliations que le contrat declare (bloc S21.G00.70), parmi les references `refs` de sa nature
/// (`affiliations`) : celles du salarie ce mois-la, avec ce que l'organisme demande de chacune.
///
/// Cahier technique DSN 2026.1 (`atelier/reference/norme-dsn/texte/bloc-S21.G00.70.txt`, p. 285-286 ;
/// `bloc-S21.G00.71.txt`, p. 287) : 70.004 « fournies par l'organisme de prévoyance et [...] à renseigner seulement sur
/// sa demande » ; 70.008 et 70.011 « si elle intervient dans le calcul de la cotisation » ; 70.014 « lorsque la date de
/// début d'affiliation au contrat OC ne correspond pas à la date de début du contrat de travail : [...] fin d'une
/// dispense d'affiliation, etc. ». Le moteur date ce qu'il sait : une adhesion ouverte apres le debut du contrat
/// (`depuis`, Thalie Sante au 01/01/2026) et une prevoyance que l'anciennete dans la branche ouvre
/// (`prevoyance_branche_depuis`). La date s'ecrit chaque mois ou le contrat se declare (inference : la condition de la
/// norme reste vraie ; la liste des cas d'usage de net-entreprises n'est pas dans le depot).
/// `ent` : la fiche de l'entreprise que la paie a lue (`_entreprise` de son bareme, `entreprise_du`).
pub fn rattachements(p: &Paie, orgs: &Value, ent: &Value, refs: Vec<String>) -> Resultat<Vec<Affiliation>> {
    let e = &p.engagement;
    let permanent = affiliations_du_permanent(p, ent)?;
    let fin = e.fin_periode()?;
    let mut out = Vec::new();
    for (place, reference) in refs.into_iter().enumerate() {
        let mut debut: Option<Jour> = None;
        if let Some(a) = &permanent {
            if place == 0 {
                match &a.prevoyance {
                    bareme::AffiliationPrevoyance::NonAffilie => continue,
                    bareme::AffiliationPrevoyance::DesLEmbauche => {}
                    bareme::AffiliationPrevoyance::Le(j) => debut = Some(*j),
                    bareme::AffiliationPrevoyance::AuPlusTardLe(j) => {
                        if *j > e.debut {
                            return Err(format!(
                                "{} : la prevoyance de sa convention s'ouvre apres une anciennete DANS LA BRANCHE, et sa \
fiche ne date que son anciennete dans l'entreprise (`date_anciennete`) : assez pour la cotisation, pas pour dire si \
l'affiliation a commence avec le contrat (le {}) ou plus tard, au plus tard le {}. La DSN date le debut de \
l'affiliation quand ce n'est pas celui du contrat (S21.G00.70.014, cahier technique p. 287) ; elle ne le devine pas.\n  \
A dire sur sa fiche : Configuration › Salariés, bloc « Salarié au mois », « Dans la branche depuis » : la date de son entree dans la branche (s'il n'y \
travaillait pas avant, sa date d'embauche chez nous). Rien n'est declare.",
                                qui_contrat(p),
                                e.debut.jj_mm_aaaa(),
                                j.jj_mm_aaaa()
                            ));
                        }
                    }
                }
            }
            if place == 1 && !a.sante {
                continue;
            }
            // Affilie a la sante a la fin d'une dispense (Q13) : 70.014 « fin d'une dispense d'affiliation »
            // (`bloc-S21.G00.71.txt`, p. 287), le lendemain de son dernier jour. Ce mois-ci n'est plus dispense
            // (`calcul::dispense_de_la_periode`) : une fin de dispense posee est passee.
            if place == 1 {
                if let Some(j) = p.salarie.get("sante_dispense_jusqu_au").filter(|v| vrai(v)) {
                    let lendemain = Jour::lire(&txt(j))?.plus_jours(1)?;
                    if lendemain <= fin && debut.map_or(true, |x| lendemain > x) {
                        debut = Some(lendemain);
                    }
                }
            }
        }
        let adhesion = liste(g(orgs, "adhesions_prevoyance")).iter().find(|a| en_texte(g(a, "reference")) == reference);
        if let Some(depuis) = adhesion.map(|a| txt(g(a, "depuis"))).filter(|d| !d.is_empty()) {
            let d = Jour::lire(&depuis)?;
            if debut.map_or(true, |x| d > x) {
                debut = Some(d);
            }
        }
        let (option, ayants_droit) = match adhesion {
            Some(a) => option_retenue(p, ent, a, &reference)?,
            None => (None, None),
        };
        out.push(Affiliation { place, reference, option, ayants_droit, debut: debut.filter(|d| *d > e.debut) });
    }
    Ok(out)
}

/// L'option retenue par le salarie (70.004) et ses ayants droit (70.008, 70.011) sous l'adhesion `adhesion`, quand
/// `organismes-base.json` dit que l'organisme les demande pour ce contrat (`contrats`, `options` : la sante des
/// permanents 3090, fiche de parametrage Audiens version 139, groupes 00003 a 00008). Ils se lisent de `sante_svp`, la
/// meme fiche que la cotisation (`bareme::bareme_sante`) ; sans elle, ou d'un niveau que la table n'a pas, refus.
fn option_retenue(p: &Paie, ent: &Value, adhesion: &Value, reference: &str) -> Resultat<(Option<String>, Option<(i64, i64)>)> {
    let base = json_en_cache(&dossier_du_code().join("organismes-base.json"))?;
    let libelle = txt(g(adhesion, "libelle"));
    let contrats = g(&base, "contrats");
    let idcc = txt(g(ent, "idcc"));
    let options = liste(g(contrats, "tous"))
        .iter()
        .chain(liste(g(contrats, &idcc)))
        .find(|c| txt(g(c, "libelle")) == libelle && vrai(g(c, "options")))
        .map(|c| g(c, "options"));
    let Some(options) = options else {
        return Ok((None, None));
    };
    let s = &p.salarie;
    let lieu = format!(
        "{} : affilie a {} (S21.G00.70.013 = {}), dont Audiens demande l'option retenue par le salarie (S21.G00.70.004 ; \
fiche de parametrage Audiens, groupes << PERMANENT SVP SANTE >>, `organismes-base.json`)",
        qui_contrat(p),
        libelle,
        reference
    );
    if bareme::alsace_moselle(ent, s) && g(options, "regime_local") != &Value::Bool(true) {
        return Err(format!(
            "{} : ses options sont du regime general (GE), et ce salarie releve du regime local d'Alsace-Moselle -- le code \
de son option n'est dans aucune fiche de parametrage du depot. Il se lit sur la fiche de parametrage Audiens de la \
compagnie ; rien n'est declare.",
            lieu
        ));
    }
    let choix = g(s, "sante_svp");
    let niveau = txt(g(choix, "niveau"));
    let enfants = match g(choix, "enfants") {
        Value::Null => Some(0),
        Value::Number(n) => n.as_i64(),
        Value::String(x) if x.trim().is_empty() => Some(0),
        Value::String(x) => x.trim().parse::<i64>().ok(),
        _ => None,
    }
    .filter(|n| (0..=99).contains(n));
    let conjoint = vrai(g(choix, "conjoint"));
    let famille = conjoint || enfants.map_or(false, |n| n > 0);
    let code = g(g(g(options, "niveaux"), &niveau), if famille { "famille" } else { "isole" }).as_str();
    match (code, enfants) {
        (Some(c), Some(n)) => Ok((Some(c.to_string()), famille.then_some((i64::from(conjoint), n)))),
        _ => Err(format!(
            "{} : elle se lit sur la fiche du salarie, `sante_svp` : {{\"niveau\": \"1\" | \"2\" | \"3\", \
\"conjoint\": true | false, \"enfants\": 0 a 99}}, d'apres son bulletin d'adhesion -- la meme fiche que sa cotisation. \
Lu : {}. Rien n'est declare.",
            lieu,
            if choix.is_null() { "rien".to_string() } else { choix.to_string() }
        )),
    }
}

/// `places_par_contrat` : combien d'identifiants d'affiliation un contrat se reserve.
pub fn places_par_contrat(bar: &Value) -> i64 {
    3.max(2 + liste(g(bar, "contributions_dsn")).len() as i64)
}

/// Les references de chaque nature (hors `ref`), en vigueur pendant le mois.
fn refs_du_mois(orgs: &Value, sec: &Value, mois: &str) -> Resultat<Option<Vec<String>>> {
    let refs: Vec<String> = if sec.is_object() { liste(g(sec, "references")).iter().map(en_texte).collect() } else { vec![] };
    if refs.is_empty() {
        return Ok(None);
    }
    let (debut, fin) = bornes_du_mois(mois)?;
    let mut out = Vec::new();
    for r in refs {
        if adhesion_en_vigueur(orgs, &r, &debut, &fin)? {
            out.push(r);
        }
    }
    Ok(Some(out))
}

/// `casc_svp` : la CASC-SVP de ce mois, ou `None`.
fn casc_svp(ctx: &Contexte, ent: &Value, orgs: &Value, mois: &str) -> Resultat<Option<Value>> {
    let base = crate::casc::base()?;
    if txt(g(ent, "idcc")) != en_texte(g(&base, "idcc")) || mois < txt(g(&base, "depuis")).as_str() {
        return Ok(None);
    }
    if bareme::casc_exemptee(ent)? {
        return Ok(None);
    }
    if crate::casc::mois_regularises(ctx)?.iter().any(|m| m == mois) {
        return Ok(None);
    }
    let dsn = g(&base, "dsn");
    let contrat = en_texte(g(dsn, "contrat"));
    let adh = liste(g(orgs, "adhesions_prevoyance")).iter().find(|a| txt(g(a, "libelle")) == contrat);
    let adh = match adh {
        Some(a) => a,
        None => {
            return Err(format!(
                "Convention 3090 : la CASC-SVP se declare en DSN depuis janvier 2025, et config/organismes.json n'a \
pas d'adhesion {} (`adhesions_prevoyance`). Sa reference se lit dans la fiche de parametrage Audiens ; elle ne se \
devine pas. Rien n'est ecrit.",
                contrat
            ))
        }
    };
    let reference = en_texte(g(adh, "reference"));
    let mut natures = Map::new();
    if let Some(m) = g(orgs, "affiliations_par_nature").as_object() {
        for (n, s) in m {
            let tous: Vec<String> = if s.is_object() { liste(g(s, "references")).iter().map(en_texte).collect() } else { vec![] };
            if tous.is_empty() || tous.contains(&reference) {
                continue;
            }
            let refs = refs_du_mois(orgs, s, mois)?.unwrap_or_default();
            if refs.len() >= 3 {
                return Err(format!(
                    "Contrat '{}' : {} adhesions deja, plus de place pour la CASC-SVP (trois identifiants \
d'affiliation par contrat). Rien n'est ecrit.",
                    n,
                    refs.len()
                ));
            }
            natures.insert(n.clone(), Value::String((202 + refs.len()).to_string()));
        }
    }
    let mut out = Map::new();
    out.insert("reference".into(), Value::String(reference));
    out.insert("libelle".into(), adh["libelle"].clone());
    out.insert("lignes".into(), g(dsn, "lignes_bulletin").clone());
    out.insert("natures".into(), Value::Object(natures));
    Ok(Some(Value::Object(out)))
}

/// La DSN de `mois` porte-t-elle la CASC-SVP ? La regle qui l'y met (`casc_svp`) le dit ; sans adhesion CASCSVP au
/// fichier des organismes, elle ne la porte pas.
pub fn porte_la_casc(ctx: &Contexte, mois: &str) -> Resultat<bool> {
    let ctx = ctx.du_mois(mois)?;
    let Some(orgs) = ctx.organismes()? else { return Ok(false) };
    Ok(matches!(casc_svp(&ctx, &ctx.entreprise, &orgs, mois), Ok(Some(_))))
}

/// `contributions_dsn` : les contributions declarees sous une adhesion Audiens.
fn contributions_dsn(ctx: &Contexte, ent: &Value, orgs: &Value, mois: &str, bar: &Value) -> Resultat<Vec<Value>> {
    let mut out = Vec::new();
    if let Some(c) = casc_svp(ctx, ent, orgs, mois)? {
        out.push(c);
    }
    let specs: Vec<&Value> = liste(g(bar, "contributions_dsn"))
        .iter()
        .filter(|c| c.is_object() && mois >= txt(g(c, "en_dsn_depuis")).as_str())
        .collect();
    if specs.is_empty() {
        return Ok(out);
    }
    let places = places_par_contrat(bar);
    let adhesions = liste(g(orgs, "adhesions_prevoyance"));
    let mut deja: HashMap<String, i64> = HashMap::new();
    for spec in specs {
        let nom = en_texte(req(spec, "contribution")?);
        let adh = adhesions.iter().find(|a| {
            txt(g(a, "contribution")).to_uppercase() == nom.to_uppercase() || txt(g(a, "libelle")).to_uppercase() == nom.to_uppercase()
        });
        let adh = match adh {
            Some(a) => a,
            None => {
                return Err(format!(
                    "Convention {} : le {} se declare en DSN depuis {}, sous une adhesion Audiens de la compagnie, et \
config/organismes.json n'en a pas (`adhesions_prevoyance`, une entree avec \"contribution\": \"{}\", sa \"reference\" et \
son \"libelle\" tels que la fiche de parametrage Audiens les donne). Elle ne se devine pas. Rien n'est ecrit.",
                    socle::date::forme_ecrite(g(ent, "idcc")).trim_matches('\''),
                    nom,
                    en_texte(g(spec, "en_dsn_depuis")),
                    nom
                ))
            }
        };
        let reference = en_texte(g(adh, "reference"));
        let mut natures = Map::new();
        if let Some(m) = g(orgs, "affiliations_par_nature").as_object() {
            for (n, sec) in m {
                let tous: Vec<String> = if sec.is_object() { liste(g(sec, "references")).iter().map(en_texte).collect() } else { vec![] };
                if tous.is_empty() || tous.contains(&reference) {
                    continue;
                }
                let refs = refs_du_mois(orgs, sec, mois)?.unwrap_or_default();
                let place = refs.len() as i64 + deja.get(n).copied().unwrap_or(0);
                if place >= places {
                    return Err(format!(
                        "Contrat '{}' : {} adhesions deja, plus de place pour le {} ({} identifiants d'affiliation par \
contrat). Rien n'est ecrit.",
                        n,
                        refs.len(),
                        nom,
                        places
                    ));
                }
                natures.insert(n.clone(), Value::String((202 + place).to_string()));
                *deja.entry(n.clone()).or_insert(0) += 1;
            }
        }
        let mut o = Map::new();
        o.insert("reference".into(), Value::String(reference));
        o.insert("libelle".into(), adh["libelle"].clone());
        o.insert("lignes".into(), req(spec, "lignes")?.clone());
        o.insert("natures".into(), Value::Object(natures));
        out.push(Value::Object(o));
    }
    Ok(out)
}

// ------------------------------------------------------------ les blocs

/// La rubrique est-elle admise pour cette nature dans le tableau des usages de la version du fichier ?
pub fn usage_admet(f: &Fichier, rubrique: &str, nature_dsn: &str) -> Resultat<bool> {
    f.norme()?.admet(rubrique, nature_dsn)
}

/// LA LICENCE D'ENTREPRENEUR DE SPECTACLES d'un contrat (S21.G00.40.049). « La licence est obligatoire pour toute
/// structure [...] dont l'activite principale est la production ou la diffusion de spectacle ou l'exploitation de lieu de
/// spectacle, a partir de 7 representations annuelles si l'activite principale de l'entreprise n'est pas d'organiser des
/// spectacles » ; « en l'absence de licence spectacle, cette rubrique doit etre renseignee par la reference portee sur le
/// recepisse » (cahier technique 2027.1.1, `norme-dsn/texte-2027/bloc-S21.G00.40.txt` l. 1418-1426 ; la meme phrase en
/// 2026.1, `texte/bloc-S21.G00.40.txt` l. 1367-1375). Ecrite quand la compagnie en a une ; sans elle, la DSN refuse
/// QUAND ELLE EST EXIGEE (`demarrage::licence_exigee`, que « Pour demarrer » lit aussi : la convention, ou un contrat de
/// spectacle, `spectacle`, dans une convention ou la case se montre) et ne l'ecrit pas sinon : une production
/// audiovisuelle n'en a pas, ni un lieu en 1790 qui n'engage pour aucun spectacle (#236). Une valeur vide ne s'ecrit
/// jamais : `X [1,100]`.
fn licence_du_contrat(f: &mut Fichier, nature_dsn: &str, ent: &Value, spectacle: bool) -> Resultat<()> {
    let licence = txt(req(ent, "licence_spectacle")?);
    if !licence.trim().is_empty() {
        return si_admise(f, nature_dsn, "S21.G00.40.049", Some(licence));
    }
    if crate::demarrage::licence_exigee(ent, spectacle) && usage_admet(f, "S21.G00.40.049", nature_dsn)? {
        return Err("La licence d'entrepreneur de spectacles manque : la saisir dans Configuration › Entreprise, « Licence d'entrepreneur de spectacles » (le numéro attribué par la DRAC, ou la référence du récépissé de déclaration). L'entrepreneur de spectacles vivants la déclare dans chaque contrat (S21.G00.40.049). Rien n'est écrit.".to_string());
    }
    Ok(())
}

fn si_admise(f: &mut Fichier, nature_dsn: &str, rubrique: &str, valeur: Option<String>) -> Resultat<()> {
    if usage_admet(f, rubrique, nature_dsn)? {
        f.add_opt(rubrique, valeur)?;
    }
    Ok(())
}

fn motif_recours(p: &Paie, nat: &Nature) -> Resultat<String> {
    let e = &p.engagement;
    match e.motif_recours.as_deref().filter(|m| !m.is_empty()) {
        Some(m) => Ok(m.to_string()),
        None => Err(format!(
            "Contrat {} ({}) : le motif de recours (S21.G00.40.021) n'est pas dit. Il est obligatoire pour un CDD \
(SIG-11) et c'est lui qui rend le contrat licite (art. L.1242-2 du code du travail) : remplacement d'un salarie, \
accroissement temporaire d'activite, emploi saisonnier... Il se saisit sur l'engagement (`motif_recours`).",
            e.numero.clone().unwrap_or_else(|| "None".into()),
            nat.libelle
        )),
    }
}

fn motif_rupture(p: &Paie, nat: &Nature) -> Resultat<String> {
    let e = &p.engagement;
    if let Some(m) = e.motif_rupture.as_deref().filter(|m| !m.is_empty()) {
        // CCH-11 de S21.G00.62.002 : un motif que la nature n'admet pas ne s'ecrit pas.
        if let Some(refus) = nat.motif_rupture_refuse(m, &e.numero.clone().unwrap_or_else(|| "None".into())) {
            return Err(refus);
        }
        return Ok(m.to_string());
    }
    if let Some(d) = nat.motif_rupture_par_defaut().filter(|d| !d.is_empty()) {
        return Ok(d);
    }
    let connus: Vec<String> = nat
        .motifs_rupture()
        .iter()
        .map(|m| format!("{} ({})", en_texte(g(m, "code")), en_texte(g(m, "libelle"))))
        .collect();
    Err(format!(
        "Contrat {} ({}) : il se termine le {}, et le motif de la rupture (S21.G00.62.002) n'est pas dit. Un {} ne \
finit pas tout seul, et le motif commande les droits du salarie a l'assurance chomage : il se saisit \
(`motif_rupture`). Motifs : {}.",
        e.numero.clone().unwrap_or_else(|| "None".into()),
        nat.libelle,
        e.fin.map(|f| f.jjmmaaaa()).unwrap_or_default(),
        nat.libelle,
        connus.join(", ")
    ))
}

fn qt(x: Decimal) -> String {
    fmt_f(x, 2)
}

fn qt_f(x: f64) -> Resultat<String> {
    Ok(qt(socle::num::dec_f64(x)?))
}

/// « Prenom NOM » d'une fiche, a defaut sa cle : qui une phrase de refus designe.
fn qui_fiche(s: &Value, cle: &str) -> String {
    let nom = [txt(g(s, "prenom")), txt(g(s, "nom"))].into_iter().filter(|x| !x.trim().is_empty()).collect::<Vec<_>>().join(" ");
    if nom.is_empty() { cle.to_string() } else { nom }
}

/// « Prenom NOM, contrat <numero> » : qui une phrase de refus du bloc 40 designe.
fn qui_contrat(p: &Paie) -> String {
    let nom = qui_fiche(&p.salarie, &cle_salarie(p));
    format!("{}, contrat {}", nom, p.engagement.numero.clone().unwrap_or_else(|| "sans numero".into()))
}

/// `S21.G00.86.003` d'une DSN mensuelle : l'anciennete dans l'entreprise (type 07, en jours), « connue a la fin de la
/// periode mensuelle declaree », jamais zero (cahier technique 2026.1, p. 322, `norme-dsn/texte/bloc-S21.G00.86.txt`,
/// l. 104-115). Elle part de `date_anciennete` de la fiche, posee a la creation de la fiche (l'ecran la pose sur toute
/// fiche neuve, l'import au premier contrat lu) et que les gens peuvent changer. Une fiche qui ne la porte pas ne dit
/// pas ce jour : elle refuse. Une date apres la fin du mois donnerait zero ou moins, que la CCH-12 interdit : elle
/// refuse aussi.
pub fn anciennete_du_mois(s: &Value, cle: &str, fin_m: &Jour) -> Resultat<i64> {
    let Some(v) = s.get("date_anciennete").filter(|v| vrai(v)) else {
        return Err(format!(
            "{} : la fiche ne porte pas sa date d'anciennete (`date_anciennete`, « Dans la compagnie depuis »), d'ou se \
compte l'anciennete declaree en S21.G00.86.003. C'est la date de la creation de la fiche (decide le 23/09/2026), que \
l'ecran pose sur toute fiche neuve et l'import au premier contrat lu ; cette fiche ne la dit pas, et elle ne se devine \
pas. La saisir sur la fiche : le jour ou la personne est entree dans la compagnie.",
            qui_fiche(s, cle)
        ));
    };
    let depuis = jour(v)?;
    let jours = fin_m.moins(&depuis) + 1;
    if jours < 1 {
        return Err(format!(
            "{} : la fiche date son anciennete du {} (`date_anciennete`), apres la fin du mois declare ({}). \
L'anciennete declaree en S21.G00.86.003 serait de {} jour(s), et la norme DSN interdit zero (CCH-12, cahier technique \
p. 322). Corriger la date sur la fiche : le jour ou la personne est entree dans la compagnie.",
            qui_fiche(s, cle),
            depuis.jj_mm_aaaa(),
            fin_m.jj_mm_aaaa(),
            jours
        ));
    }
    Ok(jours)
}

/// `S21.G00.40.002` (statut conventionnel) et `S21.G00.40.003` (statut categoriel retraite complementaire) d'un
/// contrat. Cahier technique DSN 2026.1, p. 182-183 (`atelier/reference/norme-dsn/texte/bloc-S21.G00.40.txt`, l. 133-198).
///
/// Ils se LISENT, jamais ne se devinent (regle 1) : d'abord la fiche telle que l'engagement l'a faite (`emploi_de` :
/// le metier choisi sur la ligne les a resolus, sinon la fiche les porte) ; a defaut, le metier de la fiche s'il est
/// au CATALOGUE (`emplois-base.json`), par la meme regle que `emplois::resoudre`. Un metier hors catalogue (la
/// nomenclature France Travail seule) ne dit rien du classement. Jusqu'au 10/10/2026, 05 et 04 se posaient par
/// defaut : faux pour un ouvrier (07), un employe (06), un cadre (04 / 01).
///
/// Un metier cadre au catalogue, sur une fiche qui ne dit pas cadre ou non cadre, refuse aussi : la paie s'est
/// calculee non cadre (`bareme::est_cadre` ne lit que la fiche), le catalogue dit cadre, et ni 01 ni 04 ne se lit.
///
/// La CCH-11 (p. 182, l. 156-160 : 40.003 = 01 => 40.002 dans 03, 04 ou 08) est un controle bloquant : verifiee ici,
/// avant de produire.
pub fn statuts_du_contrat(p: &Paie) -> Resultat<(String, String)> {
    let s = &p.salarie;
    let lu = |k: &str| Some(txt(g(s, k)).trim().to_string()).filter(|v| !v.is_empty());
    let code = txt(g(s, "code_emploi_ft")).trim().to_uppercase();
    let m = droit::emplois::metier(&code)?.filter(|m| m.get("catalogue") == Some(&Value::Bool(true)));
    let metier_dit = if code.is_empty() {
        "la fiche n'a pas de metier (code emploi)".to_string()
    } else if m.is_none() {
        format!("son metier ({}) n'est pas au catalogue des metiers, qui les dirait", code)
    } else {
        String::new()
    };
    let rc = match (lu("statut_categoriel_rc"), &m) {
        (Some(rc), _) => rc,
        (None, Some(m)) if !vrai(g(m, "cadre_par_defaut")) => "04".to_string(),
        (None, Some(m)) => {
            return Err(format!(
                "{} : le metier {} ({}) est cadre au catalogue, et la fiche ne dit pas cadre ou non cadre \
(S21.G00.40.003, `statut_categoriel_rc`). La paie s'est calculee non cadre ; ni l'un ni l'autre ne se devine. Saisir \
`statut_categoriel_rc` sur la fiche : 01 (cadre, et refaire la paie) ou 04 (non cadre).",
                qui_contrat(p),
                code,
                txt(g(m, "libelle"))
            ))
        }
        (None, None) => {
            let aussi = if lu("statut_conventionnel").is_none() {
                ", ni le classement conventionnel (S21.G00.40.002, `statut_conventionnel` : 04 cadre, 05 profession \
intermediaire, 06 employe, 07 ouvrier)"
            } else {
                ""
            };
            return Err(format!(
                "{} : la fiche ne dit pas le statut categoriel de retraite complementaire (S21.G00.40.003, \
`statut_categoriel_rc` : 01 cadre, 04 non cadre){}, et {}. Ils ne se devinent pas : choisir le metier dans la liste des \
metiers, ou saisir `statut_categoriel_rc` et `statut_conventionnel` sur la fiche.",
                qui_contrat(p),
                aussi,
                metier_dit
            ));
        }
    };
    let sc = match lu("statut_conventionnel") {
        Some(sc) => Some(sc),
        None => m.as_ref().and_then(|m| match rc.as_str() {
            "01" => Some("04".to_string()),
            "04" => droit::emplois::statut_non_cadre(m),
            _ => None,
        }),
    };
    let Some(sc) = sc else {
        let raison = if metier_dit.is_empty() { format!("le metier {} ne le dit pas pour le statut {}", code, rc) } else { metier_dit };
        return Err(format!(
            "{} : la fiche ne dit pas le classement conventionnel (S21.G00.40.002, `statut_conventionnel` : 04 cadre, 05 \
profession intermediaire, 06 employe, 07 ouvrier), et {}. Il ne se devine pas : le saisir sur la fiche.",
            qui_contrat(p),
            raison
        ));
    };
    if rc == "01" && !["03", "04", "08"].contains(&sc.as_str()) {
        return Err(format!(
            "{} : cadre (S21.G00.40.003 = 01) et classe {} en S21.G00.40.002. La norme DSN n'admet pour un cadre que 03 \
(cadre dirigeant), 04 (autres cadres) ou 08 (CCH-11, cahier technique p. 182) : la declaration serait rejetee. Corriger \
`statut_conventionnel` (04) ou `statut_categoriel_rc` sur la fiche.",
            qui_contrat(p),
            sc
        ));
    }
    Ok((sc, rc))
}

/// Les blocs 40 a 71 d'un contrat (`_bloc_contrat`). `emplois` : le code emplois multiples (`40.036`) du contrat,
/// que `emplois_multiples` tire des autres contrats de la periode.
#[allow(clippy::too_many_arguments)]
pub fn bloc_contrat(
    f: &mut Fichier, p: &Paie, ent: &Value, orgs: &Value, rang: i64, bar: &Value, fin_de_contrat: bool,
    arrets: &[&Arret], nature_dsn: &str, emplois: &str,
) -> Resultat<()> {
    bloc_contrat_fctu(f, p, ent, orgs, rang, bar, fin_de_contrat, arrets, nature_dsn, None, emplois)
}

/// `_bloc_contrat` avec `fin_fctu` : la fin du contrat d'un signalement de
/// fin de contrat (blocs 62 et 63, `fctu::bloc_62_63`) a la place de celle
/// d'une mensuelle.
#[allow(clippy::too_many_arguments)]
pub fn bloc_contrat_fctu(
    f: &mut Fichier, p: &Paie, ent: &Value, orgs: &Value, rang: i64, bar: &Value, fin_de_contrat: bool,
    arrets: &[&Arret], nature_dsn: &str, fin_fctu: Option<&crate::fctu::FinDeContrat>, emplois: &str,
) -> Resultat<()> {
    let e = &p.engagement;
    let s = &p.salarie;
    let nat = e.nat()?;
    let atmp = g(bar, "taux_atmp");
    let ecrit_atmp = usage_admet(f, "S21.G00.40.043", nature_dsn)?;
    if ecrit_atmp && (!vrai(g(atmp, "valeur")) || !vrai(g(atmp, "code_risque"))) {
        let annee = if bar.get("annee").is_some() { en_texte(&bar["annee"]) } else { "?".into() };
        return Err(format!(
            "Bareme {} : `taux_atmp` (valeur, code_risque) manque. C'est le taux AT/MP notifie a l'etablissement, \
declare en S21.G00.40.043 ; il ne se devine pas.",
            annee
        ));
    }
    // Les statuts se lisent, ou refusent (`statuts_du_contrat`), avant qu'une ligne du contrat soit ecrite.
    let (statut_conventionnel, statut_categoriel_rc) = statuts_du_contrat(p)?;
    f.add("S21.G00.40.001", e.debut.jjmmaaaa());
    f.add("S21.G00.40.002", statut_conventionnel);
    f.add("S21.G00.40.003", statut_categoriel_rc);
    f.add_v("S21.G00.40.004", req(s, "pcs_ese")?)?;
    // Le complement PCS-ESE : « 999SPT pour les emplois permanents du spectacle » (CCH-18, p. 185) quand la nature le
    // fixe (`contrats-base.json` : cdi, cdd) ; un CDDU declare le code de son metier dans la nomenclature des emplois
    // du spectacle, que France Travail Spectacle lit. La nature decide, pas le metier.
    match nat.code("S21.G00.40.005").filter(|c| !c.is_empty()) {
        Some(c) => f.add("S21.G00.40.005", c),
        None => f.add_v("S21.G00.40.005", req(s, "code_emploi_ft")?)?,
    }
    f.add_v("S21.G00.40.006", req(s, "emploi")?)?;
    f.add_opt("S21.G00.40.007", nat.code("S21.G00.40.007"))?;
    f.add_opt("S21.G00.40.008", nat.code("S21.G00.40.008"))?;
    f.add_opt("S21.G00.40.009", e.numero.clone())?;
    if !nat.duree_indeterminee || e.fin.is_some() {
        f.add_opt("S21.G00.40.010", e.fin.map(|j| j.jjmmaaaa()))?;
    }
    f.add("S21.G00.40.011", &nat.unite_quotite);
    let quotite = e.quotite()?;
    // Le plafond de la nature (`contrats-base.json`, `unite_quotite.plafond`) vaut pour 40.012 comme pour 40.013 :
    // CCH-12 du bloc 40 (journee, 31.00), CCH-13 (heure, 250.00) ; cahier technique 2026.1, p. 189.
    let depasse = |rubrique: &str, quoi: &str, valeur: String| -> Resultat<()> {
        let Some(plafond) = nat.plafond_quotite else { return Ok(()) };
        let q: f64 = valeur.parse().unwrap_or(0.0);
        if q <= plafond {
            return Ok(());
        }
        let cch = match nat.unite_quotite.as_str() {
            "12" | "20" => " CCH-12",
            "10" | "21" => " CCH-13",
            _ => "",
        };
        Err(format!(
            "Contrat {} du {} au {} : {} de {}, et la norme DSN la plafonne a {} en unite << {} >> \
({}, bloc 40{}). A trancher avant de le declarer.",
            e.numero.clone().unwrap_or_else(|| "None".into()),
            e.debut.jjmmaaaa(),
            e.fin.map(|j| j.jjmmaaaa()).unwrap_or_else(|| "?".into()),
            quoi,
            valeur,
            qt_f(plafond)?,
            nat.unite_quotite,
            rubrique,
            cch
        ))
    };
    depasse("S21.G00.40.013", "quotite", qt(quotite))?;
    match e.quotite_reference()? {
        Some(r) => {
            depasse("S21.G00.40.012", "quotite de reference", qt_f(r)?)?;
            quotite_de_reference_sans_heures_supplementaires(f, p, &nat, r)?;
            f.add("S21.G00.40.012", qt_f(r)?)
        }
        None => return Err("S21.G00.40.012 : la quotite de reference manque.".into()),
    }
    f.add("S21.G00.40.013", qt(quotite));
    f.add("S21.G00.40.014", nat.code("S21.G00.40.014").filter(|c| !c.is_empty()).map_or_else(|| e.modalite_temps_travail(), Ok)?);
    if bareme::regime_local(bar, s) {
        f.add("S21.G00.40.016", "01");
    } else {
        f.add_opt("S21.G00.40.016", nat.code("S21.G00.40.016"))?;
    }
    f.add("S21.G00.40.017", documents::bulletin::convention_du_contrat(ent, &p.spectacle, s)?.0);
    f.add_opt("S21.G00.40.018", nat.code("S21.G00.40.018"))?;
    f.add_v("S21.G00.40.019", req(ent, "siret")?)?;
    f.add_opt("S21.G00.40.020", nat.code("S21.G00.40.020"))?;
    if nat.ecrit("S21.G00.40.021") {
        let m = match nat.code("S21.G00.40.021").filter(|c| !c.is_empty()) {
            Some(c) => c,
            None => motif_recours(p, &nat)?,
        };
        f.add("S21.G00.40.021", m);
    }
    // « dans le cas ou les indemnites ne sont pas versees par l'employeur » (p. 195) : un contrat declare a la caisse
    // des Conges Spectacles (`caisse_conges::affilie`), et aucun autre -- ni un CDI, ni un contrat de douze mois ou plus.
    let caisse_conges = nat.ecrit("S21.G00.40.022") && p.declaree_a_la_caisse_des_conges()?;
    if caisse_conges {
        f.add("S21.G00.40.022", documents::conges_spectacles::code_dsn(ent)?);
    }
    // La deduction forfaitaire specifique que la fiche exerce (`calcul::deduction_forfaitaire`) : « Taux de deduction
    // forfaitaire specifique pour frais professionnels - S21.G00.40.023 », « Abattement pour frais professionnels applique
    // a la part de remuneration afferente a l'activite ouvrant droit a la deduction » (cahier technique 2026.1, p. 195,
    // `atelier/reference/norme-dsn/texte/bloc-S21.G00.40.txt` lignes 936-943 ; N [4,6], deux decimales).
    if let Some(d) = &p.deduction_forfaitaire {
        si_admise(f, nature_dsn, "S21.G00.40.023", Some(fmt_f(d.taux, 2)))?;
    }
    for r in ["S21.G00.40.024", "S21.G00.40.026"] {
        f.add_opt(r, nat.code(r))?;
    }
    // LES EMPLOIS MULTIPLES ne sont plus une constante de la nature : 02 quand le salarie tient plusieurs emplois
    // dans la periode de paie (p. 199, `emplois_multiples`).
    f.add("S21.G00.40.036", emplois);
    f.add_opt("S21.G00.40.037", nat.code("S21.G00.40.037"))?;
    si_admise(f, nature_dsn, "S21.G00.40.039", nat.code("S21.G00.40.039"))?;
    if ecrit_atmp {
        si_admise(f, nature_dsn, "S21.G00.40.040", Some(en_texte(&atmp["code_risque"])))?;
        si_admise(f, nature_dsn, "S21.G00.40.043", Some(fmt_f(dec_avec_echelle(&atmp["valeur"])?, 2)))?;
    }
    licence_du_contrat(f, nature_dsn, ent, nat.ecrit("S21.G00.40.050"))?;
    if nat.ecrit("S21.G00.40.050") && usage_admet(f, "S21.G00.40.050", nature_dsn)? {
        f.add_v("S21.G00.40.050", req(&p.spectacle, "numero_objet")?)?;
    }
    si_admise(f, nature_dsn, "S21.G00.40.051", nat.code("S21.G00.40.051"))?;
    if vrai(g(s, "statut_boeth")) {
        si_admise(f, nature_dsn, "S21.G00.40.072", Some(en_texte(&s["statut_boeth"])))?;
    }
    proratisation_du_plafond(f, p, nature_dsn)?;
    // S21.G00.40.086 (« Adhesion a une caisse de conges payes a titre obligatoire ») ne s'ecrit pas : « A ne pas
    // renseigner pour la version de norme P27V01 » (journal de maintenance 2027.1.2, evolution 17,
    // `atelier/reference/norme-dsn/dsn-journal-maintenance-2027.1.2-20260615.txt` l. 368-381), absente de P26V01. Le lot
    // V4b l'ecrivait sur le cahier 2027.1.1 ; `norme::verifier_les_valeurs` la refuse (`rubriques_a_ne_pas_renseigner`).
    for a in arrets {
        bloc_60(f, a, nature_dsn, g(ent, "banque"))?;
    }
    if let Some(fin) = fin_fctu {
        crate::fctu::bloc_62_63(f, fin, e)?;
    } else if e.dernier_mois()? && fin_de_contrat {
        let fin = e.fin.ok_or("S21.G00.62.001 : le contrat n'a pas de fin.")?;
        f.add("S21.G00.62.001", fin.jjmmaaaa());
        let motif = motif_rupture(p, &nat)?;
        f.add("S21.G00.62.002", &motif);
        // Le meme jour que le FCTU du contrat (`fctu::dernier_jour_paye`, cahier technique DSN p. 275 et § 2.5.2 p. 68).
        f.add("S21.G00.62.006", crate::fctu::dernier_jour_paye(e)?.jjmmaaaa());
        // 62.017 d'un CDDU suit le motif : 01 (circuit derogatoire) pour le 031, 02 sinon (CCH-12, `Nature::codes_fin`).
        for (rub, val) in nat.codes_fin(Some(&motif)) {
            f.add_v(&rub, &val)?;
        }
    }
    for s in &e.suspensions {
        suspension_sans_maintien(f, p, s)?;
        f.add_v("S21.G00.65.001", req(s, "motif")?)?;
        f.add("S21.G00.65.002", jj(req(s, "debut")?)?);
        f.add("S21.G00.65.003", jj(req(s, "fin")?)?);
    }
    let places = places_par_contrat(bar);
    let refs = affiliations(orgs, &nat, g(bar, "_contributions_du_mois"), (e.debut_periode()?, e.fin_periode()?))?;
    for a in rattachements(p, orgs, droit::bareme::entreprise_du(bar, ent), refs)? {
        if let Some(o) = a.option {
            si_admise(f, nature_dsn, "S21.G00.70.004", Some(o))?;
        }
        if let Some((conjoint, enfants)) = a.ayants_droit {
            si_admise(f, nature_dsn, "S21.G00.70.008", Some(conjoint.to_string()))?;
            si_admise(f, nature_dsn, "S21.G00.70.011", Some(enfants.to_string()))?;
        }
        si_admise(f, nature_dsn, "S21.G00.70.012", Some((202 + places * (rang - 1) + a.place as i64).to_string()))?;
        f.add("S21.G00.70.013", a.reference);
        if let Some(d) = a.debut {
            si_admise(f, nature_dsn, "S21.G00.70.014", Some(d.jjmmaaaa()))?;
        }
    }
    si_admise(f, nature_dsn, "S21.G00.71.002", nat.code("S21.G00.71.002"))?;
    Ok(())
}

/// Bloc S21.G00.65 en norme 2027 : « absence a temps plein du salarie sans maintien de sa remuneration normale par son
/// employeur » (la regle `bloc_65_sans_maintien_de_remuneration` de la version, texte dans `norme-dsn-base.json`). Le
/// moteur tire le bloc d'une suspension saisie comme imprevu (`imprevus::suspensions_du_mois`) ; un motif que
/// `permanents-base.json` dit remunere (637, conge pour evenement familial : C. trav. L. 3142-2) ne retire rien du salaire,
/// et la norme 2027 en fait une suspension qui ne se declare pas : refus, plutot que de taire un imprevu saisi. Ni le bloc 75
/// (liste p. 297) ni un autre ne recoit un conge paye : il est dans la remuneration du mois.
fn suspension_sans_maintien(f: &Fichier, p: &Paie, s: &Value) -> Resultat<()> {
    let Some(texte) = f.norme()?.regle("bloc_65_sans_maintien_de_remuneration")? else { return Ok(()) };
    let motif = txt(req(s, "motif")?);
    let regles = droit::permanents::regles()?;
    let m = liste(&regles["suspensions"]["motifs"]).into_iter().find(|m| txt(g(m, "code")) == motif);
    let Some(m) = m.filter(|m| vrai(g(m, "remunere"))) else { return Ok(()) };
    Err(format!(
        "{} : suspension au motif {} ({}), du {} au {}, saisie comme imprevu. Ce motif est un conge remunere \
(logiciel/donnees/permanents-base.json : {}). En norme {}, le bloc S21.G00.65 ne declare qu'une << absence a temps plein du \
salarie sans maintien de sa remuneration normale par son employeur >> : {} Un conge pour evenement familial se paie (le moteur \
ne retire rien du salaire) et ne se declare pas en suspension : retirer l'imprevu ; s'il n'est vraiment pas paye, le motif est a revoir. \
Rien n'est ecrit.",
        qui_contrat(p),
        motif,
        txt(g(m, "libelle")),
        txt(g(s, "debut")),
        txt(g(s, "fin")),
        txt(g(m, "_remunere")),
        f.norme()?.version,
        texte
    ))
}

/// S21.G00.40.012 en norme 2027 : la quotite de reference « ne doit pas inclure les heures supplementaires
/// structurelles » (la regle de la version, `Norme::regle`, texte dans `norme-dsn-base.json`). En heures, au-dela de la
/// duree legale (35 heures par semaine, C. trav. L. 3121-27 : 151,67 h par mois, `quotite_reference_defaut` de la
/// nature), une heure est une heure supplementaire (L. 3121-28) : une quotite de reference plus haute en inclut, et
/// refuse. Une duree collective plus courte (32 h) passe. Un contrat d'usage (en jours) n'est pas concerne.
fn quotite_de_reference_sans_heures_supplementaires(f: &Fichier, p: &Paie, nat: &droit::natures::Nature, reference: f64) -> Resultat<()> {
    let Some(texte) = f.norme()?.regle("quotite_reference_sans_heures_supplementaires")? else { return Ok(()) };
    let Some(legale) = nat.quotite_reference_defaut.filter(|_| nat.mensuelle()) else { return Ok(()) };
    if reference <= legale {
        return Ok(());
    }
    Err(format!(
        "{} : quotite de reference de {} h (S21.G00.40.012, `quotite_reference` de la fiche entreprise ou de l'engagement), \
au-dessus de la duree legale ({} h par mois, 35 heures par semaine). En norme {}, elle << ne doit pas inclure les heures \
supplementaires structurelles >> : {} Declarer la duree collective sans les heures supplementaires (une compagnie a 39 h \
declare {} h ; ses quatre heures sont des heures supplementaires, payees comme telles). Rien n'est ecrit.",
        qui_contrat(p),
        qt_f(reference)?,
        qt_f(legale)?,
        f.norme()?.version,
        texte,
        qt_f(legale)?
    ))
}

/// S21.G00.40.084, « Application de la proratisation du Plafond de Securite Sociale a hauteur de la quotite de
/// travail ». Norme 2027.1.2 (note differentielle CT 2026.1.2 -> 2027.1.2, § 4.5,
/// `atelier/reference/norme-dsn/dsn-note-differentielle-ct-2026.1.2-vers-2027.1.2-20251224.txt` l. 1737-1790) : « Pour un
/// salarie a temps partiel, cette rubrique doit etre renseignee avec le choix de l'employeur de proratiser le plafond
/// [...]. Pour un salarie en forfait jour reduit, [...] avec le choix du salarie » ; le journal de maintenance 2027.1.2
/// (`dsn-journal-maintenance-2027.1.2-20260615.txt`) la rend CONDITIONNELLE en DSN mensuelle (evolution 21, l. 433-452 :
/// O -> C) et retire sa CCH-12 (evolution 16, l. 347-366) : elle ne s'ecrit plus pour un temps plein ni pour un contrat
/// d'usage. Interdite en FCTU (I), conditionnelle en amorcage (C),
/// absente de la table P26V01 : la table de la version du fichier decide. Elle dit ce que le calcul a fait, tel que la
/// paie le porte (`Contenu::plafond_temps_partiel`) :
/// - temps partiel, plafond proratise (la regle, fiche net-entreprises 3369) : 01 ;
/// - temps partiel, l'etablissement renonce (fiche entreprise, `plafond_temps_partiel` = « entier ») : 02 (fiche 3369) ;
/// - temps partiel d'un artiste, exclu de la proratisation (L. 242-10, 1° ; BOSS § 1090) : 02. INFERENCE : la norme ne
///   dit rien d'un salarie que la loi exclut ; 02 (« Non ») est ce que le calcul fait, le plafond entier ;
/// - temps plein, contrat d'usage : rien.
/// Le forfait jour reduit (40.011 = 20) n'existe pas dans le moteur (aucune nature ne l'a) : un cas a porter.
fn proratisation_du_plafond(f: &mut Fichier, p: &Paie, nature_dsn: &str) -> Resultat<()> {
    use engagement::calcul::PlafondTempsPartiel as P;
    let r = "S21.G00.40.084";
    let usage = crate::usages::usage_en(f.norme()?, r, nature_dsn)?;
    if usage != "O" && usage != "C" {
        return Ok(());
    }
    let valeur = match p.plafond_temps_partiel {
        P::Proratise => Some("01"),
        P::Entier | P::Exclu => Some("02"),
        P::NonConcerne => None,
    };
    if let Some(v) = valeur {
        f.add(r, v);
    }
    Ok(())
}

/// Le bloc S21.G00.86 : l'anciennete dans l'entreprise, en jours.
pub fn bloc_anciennete(f: &mut Fichier, p: &Paie, jours: i64) -> Resultat<()> {
    let nat = p.engagement.nat()?;
    f.add_opt("S21.G00.86.001", nat.code("S21.G00.86.001"))?;
    f.add_opt("S21.G00.86.002", nat.code("S21.G00.86.002"))?;
    f.add("S21.G00.86.003", jours);
    f.add_opt("S21.G00.86.005", p.engagement.numero.clone())
}

/// (taux, type, identifiant, montant soumis) d'UN versement. L'abattement contrats courts n'a pas de rubrique : il se
/// lit dans l'ecart entre 50.002 et 50.013 (norme 2026.1, p. 245). S21.G00.50.012 est reservee aux abattements des
/// assistants maternels et familiaux, que seuls PAJEmploi et les departements declarent : le moteur ne l'ecrit pas.
fn pas_du_versement(
    taux_pas: Option<&Triplet>, bar: &Value, montant: Decimal, le: Jour, court: bool,
) -> Resultat<(Value, Value, Value, Decimal)> {
    let r = pas::du_versement(taux_pas, bar, montant, Some(le), court, ZONE_DEFAUT).map_err(|e| e.message().to_string())?;
    Ok((r.taux, r.type_taux, r.identifiant, r.montant_soumis))
}

pub fn court(p: &Paie, le: Jour) -> Resultat<bool> {
    let e = &p.engagement;
    Ok(contrat_court(&*e.nat()?, tuple(e.debut), e.fin.map(tuple), tuple(le)))
}

/// Les quatre blocs 51 d'une paie, et les activites 53 sous le 002.
pub fn remunerations(f: &mut Fichier, p: &Paie, debut: &Jour, fin: &Jour, signe: i64) -> Resultat<()> {
    let e = &p.engagement;
    let nat = e.nat()?;
    for code in nat.types_remuneration() {
        let code = en_texte(&code);
        let montant = match code.as_str() {
            "001" => brut_du_51(p),
            "002" => p.assiette_chomage,
            "003" => p.salaire_retabli,
            "010" => p.salaire_de_base,
            autre => return Err(format!("type de rémunération inconnu : {}", socle::fichiers::repr(autre))),
        };
        f.add("S21.G00.51.001", debut.jjmmaaaa());
        f.add("S21.G00.51.002", fin.jjmmaaaa());
        f.add_opt("S21.G00.51.010", e.numero.clone())?;
        f.add("S21.G00.51.011", &code);
        f.add("S21.G00.51.013", ms(montant, signe));
        if code == "002" {
            activites(f, p, signe)?;
        }
    }
    Ok(())
}

/// Les mesures d'activite (bloc 53) d'une paie, dans l'ordre ou elles s'ecrivent : (53.001, quantite, 53.003).
pub fn mesures_d_activite(p: &Paie) -> Resultat<Vec<(String, Decimal, String)>> {
    let e = &p.engagement;
    let nat = e.nat()?;
    let mut out = Vec::new();
    for u in nat.unites_activite() {
        let mesure = en_texte(req(&u, "mesure")?);
        let valeur = match mesure.as_str() {
            "jours" => Decimal::from(e.jours),
            "cachets" => Decimal::from(e.cachets),
            "heures" => match e.heures {
                Some(h) if h != 0.0 => socle::num::dec_f64(h)?,
                _ => Decimal::ZERO,
            },
            "heures_travaillees" => p.heures_travaillees,
            autre => return Err(format!("mesure inconnue : {}", socle::fichiers::repr(autre))),
        };
        if vrai(g(&u, "si_non_nul")) && valeur.is_zero() {
            continue;
        }
        let t = if u.get("type").is_some() { en_texte(&u["type"]) } else { "01".into() };
        out.push((t, valeur, en_texte(req(&u, "code")?)));
    }
    if let Some(a) = nat.unite_absence().filter(vrai) {
        if !p.heures_absence.is_zero() {
            out.push((en_texte(req(&a, "type")?), p.heures_absence, en_texte(req(&a, "code")?)));
        }
    }
    Ok(out)
}

fn activites(f: &mut Fichier, p: &Paie, signe: i64) -> Resultat<()> {
    for (t, valeur, code) in mesures_d_activite(p)? {
        f.add("S21.G00.53.001", t);
        f.add("S21.G00.53.002", ms(valeur, signe));
        f.add("S21.G00.53.003", code);
    }
    Ok(())
}

/// Le bloc 52 : les primes et indemnites qui ont leur code.
pub fn primes_et_indemnites(f: &mut Fichier, p: &Paie, debut: &Jour, fin: &Jour, signe: i64) -> Resultat<()> {
    for l in &p.remuneration {
        let code = match l.dsn_bloc52.as_deref().filter(|c| !c.is_empty()) {
            Some(c) => c,
            None => continue,
        };
        if let Some(m) = p.engagement.motif_rupture.as_deref().filter(|m| !m.is_empty()) {
            if let Some(pourquoi) = droit::permanents::code_52_permis(Some(code), Some(m))? {
                return Err(format!(
                    "Contrat {}, ligne << {} >> : {}",
                    p.engagement.numero.clone().unwrap_or_else(|| "None".into()),
                    l.libelle,
                    pourquoi
                ));
            }
        }
        f.add("S21.G00.52.001", code);
        f.add("S21.G00.52.002", ms(l.montant, signe));
        f.add("S21.G00.52.003", debut.jjmmaaaa());
        f.add("S21.G00.52.004", fin.jjmmaaaa());
        f.add_opt("S21.G00.52.006", p.engagement.numero.clone())?;
    }
    Ok(())
}

/// Les blocs 54 : les lignes qui portent `dsn_autres_bruts`, puis 92 et 93.
pub fn autre_revenu(f: &mut Fichier, p: &Paie, bar: &Value, debut: &Jour, fin: &Jour, signe: i64) -> Resultat<()> {
    let e = &p.engagement;
    for l in &p.remuneration {
        let code = match l.dsn_autres_bruts.as_deref().filter(|c| !c.is_empty()) {
            Some(c) => c,
            None => continue,
        };
        if l.montant.is_zero() {
            continue;
        }
        f.add("S21.G00.54.001", code);
        f.add("S21.G00.54.002", ms(l.montant, signe));
        f.add("S21.G00.54.003", debut.jjmmaaaa());
        f.add("S21.G00.54.004", fin.jjmmaaaa());
        f.add_opt("S21.G00.54.005", e.numero.clone())?;
    }
    // « 10 - Deduction forfaitaire specifique » (bloc 54, p. 257, `bloc-S21.G00.54.txt` ligne 56) : le bloc 54 « vehicule
    // [...] frais professionnels (y compris le montant de la Deduction Forfaitaire Specifique si applicable) » (chapitre
    // 2, p. 27, lignes 461-463), par contrat (CCH-16 de 50.001, p. 243, `bloc-S21.G00.50.txt` lignes 82-86). Le montant
    // est ce que la deduction retire du brut soumis : le brut moins le brut abattu du calcul (`brut_dfs`).
    if p.deduction_forfaitaire.is_some() {
        let deduction = sub(p.brut, brut_du_51(p));
        if !deduction.is_zero() {
            f.add("S21.G00.54.001", "10");
            f.add("S21.G00.54.002", ms(deduction, signe));
            f.add("S21.G00.54.003", debut.jjmmaaaa());
            f.add("S21.G00.54.004", fin.jjmmaaaa());
            f.add_opt("S21.G00.54.005", e.numero.clone())?;
        }
    }
    let bp = bareme_de(p, bar)?;
    for (type_54, cle) in [("92", "cotisation_sante_patronale"), ("93", "cotisation_prevoyance_retraite_supp")] {
        let supp = g(&bp, cle);
        if !vrai(supp) {
            continue;
        }
        let montant = if vrai(g(supp, "lignes_patronales")) {
            somme_lignes(p, &supp["lignes_patronales"], true)
        } else {
            let k = if supp.get("base").is_some() { en_texte(&supp["base"]) } else { "brut".into() };
            let base_54 = bases(p, &bp)?.get(&k).copied().unwrap_or(p.brut);
            pct(base_54, dec_avec_echelle(req(supp, "taux_sur_brut")?)?)
        };
        let montant = rond(montant);
        if montant.is_zero() {
            continue;
        }
        f.add("S21.G00.54.001", type_54);
        f.add("S21.G00.54.002", ms(montant, signe));
        f.add("S21.G00.54.003", debut.jjmmaaaa());
        f.add("S21.G00.54.004", fin.jjmmaaaa());
        f.add_opt("S21.G00.54.005", e.numero.clone())?;
    }
    Ok(())
}

/// Les dates (58.001, 58.002) d'un element de revenu calcule en net rattache a la periode [debut, fin] et verse le
/// `versement`, quand elles s'ecrivent : celles d'une correction (le 58 d'un mois anterieur, ou le rappel d'une
/// correction, `blocs_58_de_la_correction`). Norme 2026.1 (p. 265) : « en periode de versement », la periode. Norme
/// 2027 (cahier 2027.1.1, p. 267-268, la regle `net_social_en_date_de_versement` de la version) : « en date de
/// versement », la date du versement deux fois (CCH-14 : l'une ne va pas sans l'autre ; la fiche net-entreprises 2983
/// admet aussi le mois du versement). Le cas nominal de 2027 ne les ecrit pas : `dates_du_58_du_mois`.
fn dates_du_58(f: &Fichier, debut: &Jour, fin: &Jour, versement: &Jour) -> Resultat<(Jour, Jour)> {
    Ok(if f.norme()?.regle("net_social_en_date_de_versement")?.is_some() { (*versement, *versement) } else { (*debut, *fin) })
}

/// Les dates (58.001, 58.002) du 58 d'un versement du mois (la paie, ou le mois sans paie), ou `None` quand elles ne
/// s'ecrivent pas. Norme 2026.1 (p. 265) : la periode de versement, ecrite. Norme 2027 : la fiche net-entreprises 2983
/// (modifiee le 09/10/2026, `atelier/reference/sources/net-entreprises-base-connaissances/
/// net-entreprises-fiche2983-montant-net-social-20261009.txt`, l. 23-25 et l. 69-77) : « Le bloc S21.G00.58 heritant de la
/// datation du bloc Versement individu - S21.G00.50 (parent par defaut), les rubriques de datation [58.001 et 58.002]
/// ne sont pas obligatoires » ; elles servent « en cas d'erreur ou d'ecart sur un mois precedent » ; l'exemple du cas
/// nominal les laisse « Non renseignee ». Le cahier 2027.1.1 (p. 267) : « a renseigner des lors que sa datation est
/// differente de celle du Versement individu ». Le type 13 suit (« La meme regle de datation s'applique aux revenus
/// declares pour les types 10, 13, 14 et 15 », p. 268).
fn dates_du_58_du_mois(f: &Fichier, debut: &Jour, fin: &Jour) -> Resultat<Option<(Jour, Jour)>> {
    Ok(if f.norme()?.regle("net_social_en_date_de_versement")?.is_some() { None } else { Some((*debut, *fin)) })
}

fn dater_le_58(f: &mut Fichier, dates: Option<(Jour, Jour)>) {
    if let Some((d, fi)) = dates {
        f.add("S21.G00.58.001", d.jjmmaaaa());
        f.add("S21.G00.58.002", fi.jjmmaaaa());
    }
}

/// Norme 2027.1.1, S21.G00.58.003 type « 13 - Part exoneree de cotisations sociales des indemnites de rupture »
/// (p. 268 ; la regle `part_exoneree_des_indemnites_de_rupture` de la version) : la somme des indemnites que la paie
/// declare en bloc 52 (indemnites de rupture : 52.001 = 001, 003, 004, 007, 021...) et que le catalogue met hors de
/// l'assiette des cotisations (`soumis` faux, `logiciel/donnees/rubriques-base.json`). Le moteur exonere la ligne
/// entiere ou refuse (au-dela des seuils, l'indemnite n'est pas portee) : la part exoneree est le montant de la ligne.
/// `None` quand la version n'a pas le type ou que la paie n'a pas d'indemnite exoneree.
fn part_exoneree_des_indemnites_de_rupture(f: &Fichier, p: &Paie) -> Resultat<Option<Decimal>> {
    if f.norme()?.regle("part_exoneree_des_indemnites_de_rupture")?.is_none() {
        return Ok(None);
    }
    let part = p
        .remuneration
        .iter()
        .filter(|l| !l.soumis && l.dsn_bloc52.as_deref().map_or(false, |c| !c.is_empty()))
        .fold(Decimal::ZERO, |a, l| a + l.montant);
    Ok(if part.is_zero() { None } else { Some(part) })
}

/// Les blocs 58 de la paie : le type 03, le montant net social (`Paie::net_social`, calcule par `calcul::calculer`),
/// et en norme 2027 le type 13 (`part_exoneree_des_indemnites_de_rupture`), dates par `dates_du_58_du_mois` (en 2027 :
/// pas de dates, le 58 herite du versement).
pub fn net_social(f: &mut Fichier, p: &Paie, debut: &Jour, fin: &Jour, signe: i64) -> Resultat<()> {
    let dates = dates_du_58_du_mois(f, debut, fin)?;
    dater_le_58(f, dates);
    f.add("S21.G00.58.003", "03");
    f.add("S21.G00.58.004", ms(p.net_social, signe));
    if let Some(part) = part_exoneree_des_indemnites_de_rupture(f, p)? {
        dater_le_58(f, dates);
        f.add("S21.G00.58.003", "13");
        f.add("S21.G00.58.004", ms(part, signe));
    }
    Ok(())
}

/// Les blocs 78, 79 et 81 d'une paie.
#[allow(clippy::too_many_arguments)]
pub fn bases_et_cotisations(
    f: &mut Fichier, p: &Paie, bar: &Value, rang: i64, debut: &Jour, fin: &Jour, signe: i64, nature_dsn: &str,
    bases_exclues: &[&str],
) -> Resultat<()> {
    let e = &p.engagement;
    let mut courant: Option<(String, Option<String>, Decimal)> = None;
    let places = places_par_contrat(bar);
    let composants = usage_admet(f, "S21.G00.79.001", nature_dsn)?;
    for mut c in cotisations(p, bar)? {
        if bases_exclues.contains(&c.bloc78.as_str()) {
            continue;
        }
        if let Some(a) = c.affectation.as_deref().filter(|a| !a.is_empty()) {
            let n: i64 = a.trim().parse().map_err(|_| format!("affectation illisible : {}", a))?;
            c.affectation = Some((n + places * (rang - 1)).to_string());
        }
        let cle = (c.bloc78.clone(), c.affectation.clone(), c.base78);
        if courant.as_ref() != Some(&cle) {
            courant = Some(cle);
            f.add("S21.G00.78.001", &c.bloc78);
            f.add("S21.G00.78.002", debut.jjmmaaaa());
            f.add("S21.G00.78.003", fin.jjmmaaaa());
            f.add("S21.G00.78.004", ms(c.base78, signe));
            if let Some(a) = c.affectation.as_deref().filter(|a| !a.is_empty()) {
                si_admise(f, nature_dsn, "S21.G00.78.005", Some(a.to_string()))?;
            }
            if !f.contrat_unique {
                si_admise(f, nature_dsn, "S21.G00.78.006", e.numero.clone())?;
            }
            if !composants {
            } else if c.bloc78 == "03" {
                f.add("S21.G00.79.001", "01");
                let v = p.reduction_generale.as_ref().map_or(p.plafond, |rg| rg.smic_reference);
                f.add("S21.G00.79.004", ms(v, signe));
            } else if c.bloc78 == "31" {
                // Le composant que la source du taux donne (`composant79` : la sante des permanents 3090, base forfaitaire
                // 18 sur le plafond du mois, fiche Audiens 139), sinon la tranche A sur le brut.
                let (type_79, montant) = c.composant79.clone().unwrap_or_else(|| ("11".to_string(), p.brut));
                f.add("S21.G00.79.001", type_79);
                f.add("S21.G00.79.004", ms(montant, signe));
            }
        }
        let montant = match c.montant {
            Some(m) => m,
            None => continue,
        };
        let org = c.org.clone().filter(|o| !o.is_empty());
        // La CET patronale : « 146 » en P26V01 (guide Urssaf du 04/06/2026, p. 38) ; « 148 » en P27V01, le 146 y etant
        // « hors CET » (fiche net-entreprises 2556, l. 21 et 83-87). La version en decide (`cotisation_cet_patronale`).
        let code81 = if c.cet { f.norme()?.cotisation_cet_patronale()? } else { c.code81.clone() };
        f.add("S21.G00.81.001", &code81);
        if let Some(o) = &org {
            f.add("S21.G00.81.002", o);
        }
        if org.is_some() || c.cle_total.as_deref().map_or(false, |k| !k.is_empty()) {
            f.add("S21.G00.81.003", ms(c.base81.ok_or_else(|| socle::fichiers::absent("base81"))?, signe));
        }
        f.add("S21.G00.81.004", ms(montant, signe));
        if let Some(insee) = c.code_insee.as_deref().filter(|i| !i.is_empty()) {
            f.add("S21.G00.81.005", insee);
        }
        if let (Some(_), Some(t)) = (&org, &c.taux) {
            if !t.is_null() {
                f.add("S21.G00.81.007", fmt_f(dec_avec_echelle(t)?, 3));
            }
        }
    }
    Ok(())
}

/// S21.G00.50.003, le numero de versement : « Numéro valorisé par le débiteur permettant de distinguer deux versements au
/// même salarié » ; CCH-11 : « Pour un même bloc "Individu - S21.G00.30", il est interdit de déclarer plusieurs blocs
/// « Versement individu - S21.G00.50 » avec un même "Numéro de versement" » ; N [2,2], CSL 00 `[0]*[1-9][0-9]*` (norme
/// DSN 2026.1, bloc 50, p. 243-244 : `atelier/reference/norme-dsn/texte/bloc-S21.G00.50.txt`, lignes 111-126). La norme ne
/// dit rien de l'ordre : `n` est le rang du versement sous l'individu dans ce fichier, les versements du mois d'abord, puis
/// ses regularisations (`generer`). Ce n'est pas le rang du contrat, qui numerote les affiliations (70.012, 78.005) : une
/// regularisation d'un contrat aussi paye ce mois a le rang de ce contrat, et un numero de versement a elle.
fn numero_de_versement(n: i64) -> Resultat<String> {
    if !(1..=99).contains(&n) {
        return Err(format!(
            "Un individu porte un {}e versement dans cette DSN : le numero de versement (S21.G00.50.003) s'ecrit sur deux \
chiffres, de 01 a 99 (norme DSN 2026.1, p. 244), et deux versements du meme individu n'ont jamais le meme numero (CCH-11, \
p. 243). Porter une partie des corrections sur une DSN suivante. Rien n'est ecrit.",
            n
        ));
    }
    Ok(format!("{:02}", n))
}

fn bloc_versement(f: &mut Fichier, p: &Paie, bar: &Value, rang: i64, taux_pas: Option<&Triplet>) -> Resultat<()> {
    let e = &p.engagement;
    let debut = e.debut_periode()?;
    let fin = e.fin_periode()?;
    f.add("S21.G00.50.001", fin.jjmmaaaa());
    f.add("S21.G00.50.002", mt(p.net_imposable));
    // Le versement du mois a le rang de son contrat : un contrat paye ce mois, un versement (`generer`).
    f.add("S21.G00.50.003", numero_de_versement(rang)?);
    f.add("S21.G00.50.004", mt(p.net_verse()));
    // Le prelevement d'une paie validee est celui qu'elle a ecrit (`Paie::pas`) : la DSN ne le recalcule pas.
    let (taux, type_taux, identifiant, soumis) = match &p.pas {
        Some(v) => (v.taux.clone(), v.type_taux.clone(), v.identifiant.clone(), v.montant_soumis),
        None => pas_du_versement(taux_pas, bar, p.net_imposable, fin, court(p, fin)?)?,
    };
    f.add_v("S21.G00.50.006", &taux)?;
    f.add_v("S21.G00.50.007", &type_taux)?;
    if en_texte(&type_taux) == "01" {
        f.add_v("S21.G00.50.008", &identifiant)?;
    }
    f.add("S21.G00.50.009", mt(pct(soumis, dec_avec_echelle(&taux)?)));
    f.add("S21.G00.50.013", mt(soumis));

    // Un mois decoupe : les remunerations de chaque periode elementaire, datees de ses bornes (« La periode a
    // declarer dans le bloc Remuneration est cette periode elementaire de paie, lorsque le logiciel de paie produit
    // effectivement un decoupage », norme 2026.1, § 2.1.3.2, p. 28 ; les mesures d'activite aussi, § 2.1.3.5, p. 29).
    for q in p.elementaires() {
        remunerations(f, q, &q.engagement.debut_periode()?, &q.engagement.fin_periode()?, 1)?;
    }
    primes_et_indemnites(f, p, &debut, &fin, 1)?;
    if e.sans_paie() {
        let dates = dates_du_58_du_mois(f, &debut, &fin)?;
        dater_le_58(f, dates);
        f.add("S21.G00.58.003", "03");
        f.add("S21.G00.58.004", "0.00");
        let mut vues: Vec<Option<String>> = Vec::new();
        let places = places_par_contrat(bar);
        for c in cotisations(p, bar)? {
            if c.bloc78 != "31" || vues.contains(&c.affectation) {
                continue;
            }
            vues.push(c.affectation.clone());
            f.add("S21.G00.78.001", "31");
            f.add("S21.G00.78.002", debut.jjmmaaaa());
            f.add("S21.G00.78.003", fin.jjmmaaaa());
            f.add("S21.G00.78.004", "0.00");
            if let Some(a) = c.affectation.as_deref().filter(|a| !a.is_empty()) {
                let n: i64 = a.trim().parse().map_err(|_| format!("affectation illisible : {}", a))?;
                f.add("S21.G00.78.005", n + places * (rang - 1));
            }
            if !f.contrat_unique {
                f.add_opt("S21.G00.78.006", e.numero.clone())?;
            }
            f.add("S21.G00.79.001", c.composant79.as_ref().map_or("11", |(t, _)| t.as_str()));
            f.add("S21.G00.79.004", "0.00");
            f.add("S21.G00.81.001", &c.code81);
            f.add("S21.G00.81.004", "0.00");
        }
        return Ok(());
    }
    autre_revenu(f, p, bar, &debut, &fin, 1)?;
    // Le versement est date de la fin de la periode (50.001 ci-dessus).
    net_social(f, p, &debut, &fin, 1)?;
    // Les bases d'un mois decoupe, periode elementaire par periode elementaire, datees de ses bornes (78.002, 78.003) :
    // « Pour tout changement intervenant en cours de mois, il convient de ventiler les bases assujetties par periodes »
    // (fiche net-entreprises 1265, l. 52) ; FAQ Agirc-Arrco « Changement de situation en cours de mois ».
    for q in p.elementaires() {
        bases_et_cotisations(f, q, bar, rang, &q.engagement.debut_periode()?, &q.engagement.fin_periode()?, 1, "01", &[])?;
    }
    Ok(())
}

/// Un versement tel que la mensuelle l'ecrit pour un contrat : les affiliations du contrat (S21.G00.70.012, 70.013) et
/// les rubriques de son versement (blocs 50 a 81), dans l'ordre ou elles s'ecrivent.
pub struct VersementEcrit {
    pub affiliations: Vec<(String, String)>,
    pub rubriques: Vec<(String, String)>,
}

/// CE QU'UNE DSN A DECLARE POUR UN CONTRAT, ecrit depuis la paie qu'elle a declaree (`engagement::etats::paies_declarees`)
/// par les fonctions qui ecrivent la mensuelle de `mois` : ses affiliations (`rattachements`) et son versement
/// (`bloc_versement`), qu'une regularisation annule (`versement_declare::de_la_paie`).
pub fn versement_ecrit(ctx: &Contexte, p: &Paie, mois: &str) -> Resultat<VersementEcrit> {
    let ctx = &ctx.du_mois(mois)?;
    let ent = &ctx.entreprise;
    let nic = bareme::etablissement_du_salarie(ent, &p.salarie)?;
    let orgs = ctx.organismes_exiges()?;
    let bar = bar_du(ctx, ent, &orgs, &nic, mois)?;
    let ent_e = fiche_de(ent, &nic)?;
    let e = &p.engagement;
    let refs = affiliations(&orgs, &*e.nat()?, g(&bar, "_contributions_du_mois"), (e.debut_periode()?, e.fin_periode()?))?;
    let affiliations = rattachements(p, &orgs, bareme::entreprise_du(&bar, &ent_e), refs)?
        .into_iter()
        .map(|a| ((202 + a.place as i64).to_string(), a.reference))
        .collect();
    let mut f = Fichier::en_norme(crate::norme::de_la_mensuelle(mois)?);
    bloc_versement(&mut f, p, &bar, 1, None)?;
    Ok(VersementEcrit { affiliations, rubriques: f.rubriques() })
}

/// Les affiliations que CE fichier a deja ecrites pour le contrat `numero` de l'individu en cours :
/// (S21.G00.70.013, S21.G00.70.012).
fn affiliations_ecrites(f: &Fichier, numero: &str) -> Vec<(String, String)> {
    let depart = f.lignes.iter().rposition(|l| l.starts_with("S21.G00.30.001,")).unwrap_or(0);
    let mut out = Vec::new();
    let (mut du_contrat, mut id) = (false, None::<String>);
    for l in &f.lignes[depart..] {
        let (k, v) = l.split_once(',').unwrap_or((l.as_str(), ""));
        let v = v.trim_matches('\'');
        match k {
            "S21.G00.40.001" | "S21.G00.50.001" => du_contrat = false,
            "S21.G00.40.009" => du_contrat = v == numero,
            "S21.G00.70.012" if du_contrat => id = Some(v.to_string()),
            "S21.G00.70.013" if du_contrat => {
                if let Some(i) = id.take() {
                    out.push((v.to_string(), i));
                }
            }
            _ => {}
        }
    }
    out
}

/// L'annulation de ce qui a ete DECLARE : les lignes des `blocs` du versement declare, montants
/// changes de signe, le reste tel quel. 78.005 cite une affiliation (70.012) du versement declare : elle se retrouve dans
/// ce fichier par sa reference (70.013), son numero n'y est pas forcement le meme (le rang du contrat change).
fn annuler_le_declare(f: &mut Fichier, r: &Regularisation, blocs: &[&str]) -> Resultat<()> {
    let d = &r.declare;
    let numero = r.apres.engagement.numero.clone().ok_or_else(|| absente("S21.G00.40.009"))?;
    let ecrites = affiliations_ecrites(f, &numero);
    let lignes: Vec<(String, String)> = d.blocs(blocs).cloned().collect();
    for (k, v) in lignes {
        if k == "S21.G00.78.005" {
            let reference = d.affiliations.iter().find(|(id, _)| *id == v).map(|(_, rf)| rf.clone()).ok_or_else(|| {
                format!(
                    "Correction de {} (contrat {}) : {} cite l'affiliation {} en S21.G00.78.005 sans la \
declarer dans les blocs 70 du contrat. L'annulation ne sait pas a quoi la rattacher. Rien n'est ecrit.",
                    r.mois, numero, d.fichier, v
                )
            })?;
            let id = ecrites.iter().find(|(rf, _)| *rf == reference).map(|(_, i)| i.clone()).ok_or_else(|| {
                format!(
                    "Correction de {} (contrat {}) : {} rattachait une base a l'adhesion {} \
(S21.G00.70.013), que le contrat ne declare plus dans cette DSN. L'annulation ne sait pas a quelle affiliation la \
rattacher. Rien n'est ecrit.",
                    r.mois, numero, d.fichier, reference
                )
            })?;
            f.add(&k, id);
        } else if crate::versement_declare::est_un_montant(&k) == Some(true) {
            let m: Decimal = v.parse().map_err(|_| format!("{} illisible dans {} : {:?}", k, d.fichier, v))?;
            f.add(&k, ms(m, -1));
        } else {
            f.add(&k, v);
        }
    }
    Ok(())
}

/// Les blocs 58 de type 03 (montant net social) du versement d'une correction. Norme DSN 2026.1, S21.G00.58.003
/// (`atelier/reference/norme-dsn/texte/bloc-S21.G00.60.txt`, l. 14-18, p. 265) : pour le montant net social, « la date de
/// debut (S21.G00.58.001) et la date de fin (S21.G00.58.002) de periode de rattachement sont a renseigner en periode
/// de versement ». BOSS, montant net social, Q 35 et Q 41 (`atelier/reference/sources/montant-net-social/
/// boss-montant-net-social-20261001.txt`, l. 476-492 et 552-571) : un rappel ou une reprise se prend dans le montant net
/// social « du mois lors duquel ce rappel ou ce versement est effectue » ; si seul le montant net social declare etait
/// faux, la correction se rattache a la periode d'emploi concernee, « en precisant quel etait le montant net social
/// effectivement verse au titre de cette periode ». D'ou deux parts :
///
/// - **ce qui a ete verse ne change pas** (`apres.net_a_payer` = le net deja verse, lu sur le bulletin) : seul le
///   declare peut etre faux ; l'annulation du 58 depose et le 58 recalcule, aux dates de la periode d'origine (la
///   periode de son versement d'origine) ;
/// - **la correction verse ou reprend** (net a payer different) : le montant net social de ce qui a ete paye est le
///   net deja verse moins ce que la paie d'origine payait hors du montant net social (remboursements de frais,
///   acompte : `avant.net_a_payer - avant.net_social`, qui ne dependent que des lignes saisies). S'il differe du
///   declare (un depot qui portait le net a payer), le 58 depose s'annule et ce montant se declare,
///   aux dates d'origine ; la difference avec le montant net social corrige se declare aux dates du mois du versement,
///   du premier au dernier jour du mois de la correction (inference : la « periode de versement » est le mois dont le
///   versement porte la date, `S21.G00.50.001` = `fin_mois`).
///
/// Une correction deja portee a pu declarer un rappel aux dates de son propre mois (`declare.rappels_net_social`) :
/// le montant net social d'origine est ce qui a ete paye moins ces rappels.
fn blocs_58_de_la_correction(f: &mut Fichier, r: &Regularisation, debut: &Jour, fin: &Jour, fin_mois: Jour) -> Resultat<()> {
    let (avant, apres) = (&r.avant, &r.apres);
    // Ce que des corrections deja portees ont declare aux dates du mois de leur versement : le montant net social de la
    // periode d'origine ne le reprend pas.
    let rappels = r.declare.rappels_net_social;
    // Norme 2027 : le type 13 (indemnites de rupture exonerees) se declare avec le montant net social ; une correction
    // qui le porterait (avant ou apres) aurait a l'annuler et le redeclarer avec lui, ce qui n'est pas porte : refus.
    if part_exoneree_des_indemnites_de_rupture(f, avant)?.is_some() || part_exoneree_des_indemnites_de_rupture(f, apres)?.is_some() {
        return Err(format!(
            "Correction de {} (contrat {}) : la paie porte une indemnite de rupture hors de l'assiette des cotisations, que \
la norme {} declare en S21.G00.58 de type 13 (<< Part exoneree de cotisations sociales des indemnites de rupture >>, \
cahier 2027.1.1, p. 268). Le logiciel ne sait pas encore l'annuler et la redeclarer dans une correction. Rien n'est ecrit.",
            r.mois,
            apres.engagement.numero.clone().unwrap_or_else(|| "?".into()),
            f.norme()?.version
        ));
    }
    // Le 58 de la paie d'origine se date de son versement (sa fin de periode, 50.001 de la DSN d'origine) en norme 2027.
    let (d_o, f_o) = dates_du_58(f, debut, fin, fin)?;
    let origine = |f: &mut Fichier, montant: Decimal| -> Resultat<()> {
        annuler_les_58(f, r, (d_o, f_o))?;
        f.add("S21.G00.58.001", d_o.jjmmaaaa());
        f.add("S21.G00.58.002", f_o.jjmmaaaa());
        f.add("S21.G00.58.003", "03");
        f.add("S21.G00.58.004", ms(montant, 1));
        Ok(())
    };
    // UN CONTRAT AJOUTE (rien de declare, rien de verse : `produire_regularisations::ajout`) : pas de 58 d'origine a
    // annuler ni a redeclarer ; tout son montant net social est verse par la correction, aux dates du mois du versement
    // (BOSS, montant net social, Q 35 : « du mois lors duquel ce rappel ou ce versement est effectue »).
    let ajoute = r.declare.lignes.is_empty() && r.declare.fichier.is_empty() && r.net_deja_verse.is_zero();
    if !ajoute && apres.net_a_payer == r.net_deja_verse {
        return origine(f, sub(apres.net_social, rappels));
    }
    let paye = if ajoute { Decimal::ZERO } else { sub(r.net_deja_verse, sub(avant.net_a_payer, avant.net_social)) };
    if !ajoute && sub(paye, rappels) != r.declare.net_social {
        origine(f, sub(paye, rappels))?;
    }
    let debut_mois = Jour::nouveau(fin_mois.an, fin_mois.mois, 1).ok_or("mois de versement illisible")?;
    let (d_v, f_v) = dates_du_58(f, &debut_mois, &fin_mois, &fin_mois)?;
    f.add("S21.G00.58.001", d_v.jjmmaaaa());
    f.add("S21.G00.58.002", f_v.jjmmaaaa());
    f.add("S21.G00.58.003", "03");
    f.add("S21.G00.58.004", ms(sub(apres.net_social, paye), 1));
    Ok(())
}

/// L'annulation des blocs 58 du versement d'origine (`annuler_le_declare`), dans le versement d'une correction (date
/// du mois de la correction). Un 58 d'origine sans dates (le cas nominal de 2027, `dates_du_58_du_mois`) heritait de la
/// date de SON versement : annule tel quel, il heriterait de celle de la correction. Il recoit donc les dates du
/// versement d'origine (`origine`), comme le complement qui le suit (fiche net-entreprises 2983 : la correction d'un MNS
/// se rattache « a la date du mois du versement auquel ce MNS etait associe »).
fn annuler_les_58(f: &mut Fichier, r: &Regularisation, origine: (Jour, Jour)) -> Resultat<()> {
    let lignes: Vec<(String, String)> = r.declare.blocs(&["58"]).cloned().collect();
    let mut date = false;
    for (k, v) in lignes {
        match k.as_str() {
            "S21.G00.58.001" => {
                date = true;
                f.add(&k, v);
            }
            "S21.G00.58.003" => {
                if !date {
                    dater_le_58(f, Some(origine));
                }
                date = false;
                f.add(&k, v);
            }
            _ if crate::versement_declare::est_un_montant(&k) == Some(true) => {
                let m: Decimal = v.parse().map_err(|_| format!("{} illisible dans {} : {:?}", k, r.declare.fichier, v))?;
                f.add(&k, ms(m, -1));
            }
            _ => f.add(&k, v),
        }
    }
    Ok(())
}

/// `rang` : le rang du contrat sous l'individu (ses affiliations, 78.005) ; `numero` : celui du versement (50.003), qui
/// n'est jamais celui d'un autre versement de l'individu (`numero_de_versement`).
#[allow(clippy::too_many_arguments)]
fn bloc_regularisation(
    f: &mut Fichier, r: &Regularisation, bar_o: &Value, rang: i64, numero: i64, taux_pas: Option<&Triplet>, fin_mois: Jour,
    bar: &Value,
) -> Resultat<()> {
    // `avant` ne donne ici que l'engagement declare (ses bornes, s'il est sans paie) : les montants annules sont ceux
    // du versement declare (`r.declare`), pas ceux d'un recalcul.
    let (avant, apres) = (&r.avant, &r.apres);
    let debut = avant.engagement.debut_periode()?;
    let fin = avant.engagement.fin_periode()?;
    // La paie corrigee se rattache a la periode du contrat corrige : ses dates ont pu changer (21/10/2026, fiche
    // net-entreprises 1952 ; cahier technique DSN 2026.1, §2.1.3.6). L'annulation recopie le declare, a ses dates.
    let (debut_c, fin_c) = (apres.engagement.debut_periode()?, apres.engagement.fin_periode()?);
    let d_ni = sub(apres.net_imposable, r.declare.net_imposable);
    let d_net = sub(apres.net_verse(), r.declare.net_verse);
    f.add("S21.G00.50.001", fin_mois.jjmmaaaa());
    f.add("S21.G00.50.002", mt(d_ni));
    f.add("S21.G00.50.003", numero_de_versement(numero)?);
    f.add("S21.G00.50.004", mt(d_net));
    let (taux, type_taux, identifiant, soumis) = pas_du_versement(taux_pas, bar, d_ni, fin_mois, court(apres, fin_mois)?)?;
    f.add_v("S21.G00.50.006", &taux)?;
    f.add_v("S21.G00.50.007", &type_taux)?;
    if en_texte(&type_taux) == "01" {
        f.add_v("S21.G00.50.008", &identifiant)?;
    }
    let indu = d_ni < Decimal::ZERO;
    f.add("S21.G00.50.009", if indu { "0.00".into() } else { mt(pct(soumis, dec_avec_echelle(&taux)?)) });
    f.add("S21.G00.50.013", if indu { "0.00".into() } else { mt(soumis) });

    annuler_le_declare(f, r, &["51", "53"])?;
    remunerations(f, apres, &debut_c, &fin_c, 1)?;
    if !avant.engagement.sans_paie() {
        annuler_le_declare(f, r, &["54"])?;
    }
    if !apres.engagement.sans_paie() {
        autre_revenu(f, apres, bar_o, &debut_c, &fin_c, 1)?;
    }
    if indu {
        let taux_o = match r.taux_pas_origine.as_deref().filter(|t| !t.is_empty()) {
            Some(t) => t,
            None => {
                return Err(format!(
                    "Correction de {} pour {} : le taux de prelevement a la source applique ce mois-la est \
introuvable dans le versement declare (sorties/{}/{}, rubrique S21.G00.50.006). Le bloc 56 de regularisation du PAS \
doit le citer, et on ne le devine pas. Rien n'est ecrit.",
                    r.mois,
                    txt(g(&apres.salarie, "nom")),
                    r.mois,
                    r.declare.fichier
                ))
            }
        };
        f.add("S21.G00.56.001", format!("{}{}", &r.mois[5..7], &r.mois[..4]));
        f.add("S21.G00.56.002", "03");
        f.add("S21.G00.56.003", mt(d_ni));
        f.add("S21.G00.56.006", taux_o);
        f.add("S21.G00.56.007", mt(pct(d_ni, dec(&Value::String(taux_o.into()))?)));
        f.add("S21.G00.56.010", mt(d_ni));
    }
    blocs_58_de_la_correction(f, r, &debut, &fin, fin_mois)?;
    if !avant.engagement.sans_paie() {
        annuler_le_declare(f, r, &["78", "79", "81"])?;
    }
    if !apres.engagement.sans_paie() {
        bases_et_cotisations(f, apres, bar_o, rang, &debut_c, &fin_c, 1, "01", &[])?;
    }
    Ok(())
}

// ------------------------------------------- l'agrege de ce qui a ete declare
//
// Le bordereau Urssaf du mois d'origine (`lignes_regularisees`), les versements 20 (`cumuler_le_declare`) et le bloc 55
// d'une regularisation portent la MEME difference que le nominatif : « corrige moins declare »
// Le declare se recompose depuis les lignes 78 et 81 deposees du contrat (`r.declare`), lues
// par ce que la pile de bareme du contrat dit de chacune : quelle base porte un bloc 78 (78.004) ou une cotisation
// (81.003), a quelle cle de totalisation va une cotisation. Ce qui ne s'y retrouve pas sans ambiguite REFUSE (regle 1),
// en nommant le CTP ou la cotisation : on ne retombe pas sur la paie d'avant refaite.

/// Ce que le bareme dit d'une ligne 81, reconnue a son code (81.001) et a son organisme (81.002, vide sans lui).
#[derive(Clone, Debug, PartialEq)]
enum Role81 {
    /// Une cotisation : sa cle de totalisation, et la base que porte son 81.003 quand elle l'ecrit.
    Cotisation { cle: String, base: Option<String> },
    /// Une part de la reduction generale (« urssaf », « chomage », « retraite ») et sa cle de totalisation.
    Reduction { part: &'static str, cle: String },
}

impl Role81 {
    fn cle(&self) -> &str {
        match self {
            Role81::Cotisation { cle, .. } | Role81::Reduction { cle, .. } => cle,
        }
    }
}

/// La lecture que la pile de bareme d'un contrat donne de ses lignes deposees : (code, organisme, role) des lignes 81,
/// (bloc 78, base78) des blocs 78. Le meme bareme que `cotisations` : `bareme_de`, plus les contributions du mois et
/// les trois parts de la reduction generale.
struct Lecture {
    roles: Vec<(String, String, Role81)>,
    blocs: Vec<(String, String)>,
}

fn lecture(p: &Paie, bar: &Value) -> Resultat<Lecture> {
    let contribs = liste(g(bar, "_contributions_du_mois")).to_vec();
    let pile = bareme_de(p, bar)?;
    let mut l = Lecture { roles: Vec::new(), blocs: Vec::new() };
    for c in liste(req(&pile, "cotisations_dsn")?) {
        l.blocs.push((en_texte(req(c, "bloc78")?), en_texte(req(c, "base78")?)));
        if g(c, "code81").is_null() {
            continue;
        }
        let org = opt_txt(g(c, "org")).unwrap_or_default();
        let cle_total = if vrai(g(c, "cle_total")) { Some(en_texte(&c["cle_total"])) } else { None };
        // 81.003 s'ecrit avec un organisme ou une cle de totalisation (`bases_et_cotisations`) ; il porte la base du
        // taux, pas celle d'une cotisation faite de lignes du bulletin.
        let base = if (!org.is_empty() || cle_total.is_some()) && !vrai(g(c, "lignes")) && !g(c, "taux").is_null() {
            Some(en_texte(req(c, "base")?))
        } else {
            None
        };
        let code = en_texte(&c["code81"]);
        l.roles.push((code.clone(), org, Role81::Cotisation { cle: cle_total.unwrap_or(code), base }));
    }
    let nature = p.engagement.nat()?.bareme.clone().unwrap_or_else(|| "intermittent".into());
    for contrib in &contribs {
        if g(contrib, "natures").get(&nature).is_some() {
            l.blocs.push(("31".into(), "zero".into()));
            l.roles.push(("059".into(), String::new(), Role81::Cotisation { cle: "059".into(), base: None }));
        }
    }
    let dsn = g(g(&pile, "reduction_generale"), "dsn");
    let par = |k: &str, defaut: &str| -> String {
        let v = g(dsn, k);
        if v.is_null() { defaut.to_string() } else { en_texte(v) }
    };
    let org_de = |code: &str| -> String {
        liste(g(&pile, "cotisations_dsn")).iter().find(|c| txt(g(c, "code81")) == code).and_then(|c| opt_txt(g(c, "org"))).unwrap_or_default()
    };
    l.blocs.push(("03".into(), "brut".into()));
    for (code, org, part, cle) in [
        (par("code81_urssaf", "018"), org_de("075"), "urssaf", par("cle_total_urssaf", "018:urssaf")),
        (par("code81_chomage", "018"), org_de("040"), "chomage", par("cle_total_chomage", "018:chomage")),
        (par("code81_retraite", "106"), String::new(), "retraite", "106".to_string()),
    ] {
        l.roles.push((code, org, Role81::Reduction { part, cle }));
    }
    Ok(l)
}

/// Une ligne 81 deposee, avec le bloc 78 qui la porte.
struct Ligne81 {
    code: String,
    org: String,
    base: Option<Decimal>,
    montant: Option<Decimal>,
    /// 78.005 du bloc 78 en cours : l'affiliation (70.012) dans la DSN deposee.
    affiliation: Option<String>,
}

/// Les blocs 78 (78.001, 78.004) et les lignes 81 du versement declare.
fn lignes_declarees(d: &crate::versement_declare::VersementDeclare) -> Resultat<(Vec<(String, Decimal)>, Vec<Ligne81>)> {
    let lu = |k: &str, v: &str| -> Resultat<Decimal> { v.parse().map_err(|_| format!("{} illisible dans {} : {:?}", k, d.fichier, v)) };
    let (mut b78, mut l81): (Vec<(String, Decimal)>, Vec<Ligne81>) = (Vec::new(), Vec::new());
    let (mut bloc, mut affiliation) = (String::new(), None::<String>);
    for (k, v) in d.blocs(&["78", "81"]) {
        match k.as_str() {
            "S21.G00.78.001" => {
                bloc = v.clone();
                affiliation = None;
            }
            "S21.G00.78.004" => b78.push((bloc.clone(), lu(k, v)?)),
            "S21.G00.78.005" => affiliation = Some(v.clone()),
            "S21.G00.81.001" => l81.push(Ligne81 { code: v.clone(), org: String::new(), base: None, montant: None, affiliation: affiliation.clone() }),
            "S21.G00.81.002" | "S21.G00.81.003" | "S21.G00.81.004" => {
                let l = l81.last_mut().ok_or_else(|| format!("{} sans S21.G00.81.001 dans {}", k, d.fichier))?;
                match k.as_str() {
                    "S21.G00.81.002" => l.org = v.clone(),
                    "S21.G00.81.003" => l.base = Some(lu(k, v)?),
                    _ => l.montant = Some(lu(k, v)?),
                }
            }
            _ => {}
        }
    }
    Ok((b78, l81))
}

impl Lecture {
    fn roles_de(&self, code: &str, org: &str) -> Vec<&Role81> {
        self.roles.iter().filter(|(c, o, _)| c == code && o == org).map(|(_, _, r)| r).collect()
    }

    /// La base `nom` telle que les lignes deposees la portent : les 78.004 des blocs 78 dont le bareme dit qu'ils la
    /// portent (et rien d'autre), les 81.003 des cotisations dont il dit de meme. Elles doivent toutes dire la meme
    /// chose, et il en faut une.
    fn base(&self, nom: &str, b78: &[(String, Decimal)], l81: &[Ligne81]) -> Result<Decimal, String> {
        let bloc_sur = |b: &str| {
            let siens: Vec<&String> = self.blocs.iter().filter(|(x, _)| x == b).map(|(_, base)| base).collect();
            !siens.is_empty() && siens.iter().all(|x| *x == nom)
        };
        let cotisation_sur = |code: &str, org: &str| {
            let r = self.roles_de(code, org);
            !r.is_empty() && r.iter().all(|x| matches!(x, Role81::Cotisation { base: Some(b), .. } if b == nom))
        };
        let mut vus: Vec<(String, Decimal)> = Vec::new();
        for (b, v) in b78 {
            if bloc_sur(b) {
                vus.push((format!("S21.G00.78.004 du bloc {}", b), *v));
            }
        }
        for l in l81 {
            if let (Some(v), true) = (l.base, cotisation_sur(&l.code, &l.org)) {
                vus.push((format!("S21.G00.81.003 de la cotisation {}", l.code), v));
            }
        }
        let Some((_, premiere)) = vus.first() else {
            return Err(format!("aucune ligne deposee ne porte la base « {} » (ni bloc 78, ni cotisation que le bareme y rattache sans ambiguite)", nom));
        };
        if vus.iter().any(|(_, v)| v != premiere) {
            let detail: Vec<String> = vus.iter().map(|(q, v)| format!("{} = {}", q, mt(*v))).collect();
            return Err(format!("les lignes deposees portent la base « {} » a des montants differents ({})", nom, detail.join(", ")));
        }
        Ok(*premiere)
    }

    /// La reduction generale que l'Urssaf recouvre, en positif, telle que des lignes 81 la portent : l'oppose des 81.004
    /// des lignes declarees sous le code et l'organisme de la part Urssaf. Le bareme d'un permanent y declare aussi la
    /// part chomage (018 chez l'Urssaf, qui recouvre son assurance chomage : `cle_total_chomage`) ; celui d'un
    /// intermittent la declare chez France Travail, hors de ces lignes. C'est ce que le bordereau porte : « La somme des
    /// donnees individuelles (S21.G00.81.004 : code 018) doit etre egale aux donnees agregees (S21.G00.23.005) », « le
    /// montant agrege contient uniquement les montants Urssaf et assurance chomage » (guide declaratif Urssaf, § 8.2 ;
    /// atelier/reference/sources/editeur-dsn/guide-urssaf-regularisation.txt, lignes 1289-1290, 1300-1311 et 1355-1356).
    /// Les lignes n'ont donc pas a dire quelle part est laquelle. Sans ligne, rien n'a ete declare (`cotisations`
    /// n'ecrit pas une part nulle) ; une cotisation que le bareme declare sous le meme code et le meme organisme ne
    /// s'en separe pas : refus.
    fn reduction_a_l_urssaf(&self, l81: &[Ligne81]) -> Result<Decimal, String> {
        let urssaf = self.roles.iter().find(|(_, _, r)| matches!(r, Role81::Reduction { part: "urssaf", .. }));
        let Some((code, org, _)) = urssaf else {
            return Ok(zero());
        };
        if self.roles_de(code, org).iter().any(|r| matches!(r, Role81::Cotisation { .. })) {
            return Err(format!(
                "la reduction generale se declare en S21.G00.81.001 = {}{}, comme une cotisation du bareme : les lignes ne disent pas laquelle est laquelle",
                code,
                if org.is_empty() { String::new() } else { format!(" chez {}", org) }
            ));
        }
        let mut total = zero();
        for l in l81.iter().filter(|l| l.code == *code && l.org == *org) {
            total = sub(total, l.montant.ok_or_else(|| format!("la reduction generale {} n'a pas de S21.G00.81.004", code))?);
        }
        Ok(total)
    }
}

/// Les lignes 81 que le nominatif d'une paie declare (`cotisations`, comme `bases_et_cotisations` les ecrit), dans la
/// forme ou `lignes_declarees` relit celles d'une DSN deposee.
fn lignes_81(p: &Paie, bar: &Value) -> Resultat<Vec<Ligne81>> {
    Ok(cotisations(p, bar)?
        .into_iter()
        .filter(|c| c.montant.is_some())
        .map(|c| Ligne81 {
            base: if c.org.is_some() || c.cle_total.is_some() { c.base81 } else { None },
            code: c.code81,
            org: c.org.unwrap_or_default(),
            montant: c.montant,
            affiliation: c.affectation,
        })
        .collect())
}

/// La reduction generale que l'Urssaf recouvre pour une paie (CTP 671 d'un technicien intermittent, 668 d'un
/// permanent), lue dans son nominatif comme celle d'une DSN deposee (`Lecture::reduction_a_l_urssaf`) : une seule
/// lecture pour le bordereau courant et le bordereau regularise.
fn reduction_a_l_urssaf(p: &Paie, bar: &Value) -> Resultat<Decimal> {
    if p.reduction_generale.as_ref().map_or(true, |r| r.montant.is_zero()) {
        return Ok(zero());
    }
    lecture(p, bar)?.reduction_a_l_urssaf(&lignes_81(p, bar)?).map_err(|e| {
        format!("Contrat {} : {}. Rien n'est ecrit.", p.engagement.numero.as_deref().unwrap_or("sans numero"), e)
    })
}

/// La valeur que le contrat d'une regularisation apportait, tel qu'il a ete DECLARE, a l'assiette `cle` de la table des
/// CTP (celles de `assiettes_ctp`). La nature (permanent, intermittent) et le statut (artiste, technicien) sont ceux du
/// contrat d'avant (`r.avant`), comme ses bornes.
fn assiette_declaree(cle: &str, r: &Regularisation, bar: &Value) -> Resultat<Result<Decimal, String>> {
    // Un contrat ajoute apres la limite (`produire_regularisations::ajout`) : rien de declare, toutes ses assiettes sont
    // nulles -- le bordereau du mois d'origine porte tout le corrige.
    if r.declare.lignes.is_empty() && r.declare.fichier.is_empty() {
        let _ = (cle, bar);
        return Ok(Ok(zero()));
    }
    let p = &r.avant;
    let mensuelle = p.engagement.nat()?.mensuelle();
    // Un permanent declare a la caisse des Conges Spectacles a declare sa taxe d'apprentissage et son FNAL
    // sur des assiettes majorees ; les autres, sur le brut et la tranche 1.
    let caisse = mensuelle && p.declaree_a_la_caisse_des_conges()?;
    let tech = !mensuelle && statut(p) == "technicien";
    let art = !mensuelle && !tech;
    let l = lecture(p, bar)?;
    let (b78, l81) = lignes_declarees(&r.declare)?;
    let brut = r.declare.brut;
    let si = |oui: bool, v: &dyn Fn() -> Result<Decimal, String>| if oui { v() } else { Ok(zero()) };
    let base = |nom: &str| l.base(nom, &b78, &l81);
    Ok(match cle {
        "brut" => Ok(brut),
        "csg" => base("csg"),
        "brut_x115" => base("brut_x115"),
        "brut_x110" => base("brut_x110"),
        "brut_permanent" => si(mensuelle, &|| Ok(brut)),
        "csg_permanent" => si(mensuelle, &|| base("csg")),
        "t1_permanent" => si(mensuelle, &|| base("t1_mensuelle")),
        // La base 07 deposee (78.004 sous le bloc 07), comme `base_07` pour le corrige.
        "chomage_permanent" => si(mensuelle, &|| {
            let v: Vec<Decimal> = b78.iter().filter(|(b, _)| b == "07").map(|(_, v)| *v).collect();
            match v.first() {
                None => Ok(zero()),
                Some(x) if v.iter().all(|y| y == x) => Ok(*x),
                Some(_) => Err("la DSN deposee porte deux bases 07 differentes".to_string()),
            }
        }),
        "t1_x115_permanent" => si(mensuelle, &|| base(if caisse { "t1_mensuelle_x115" } else { "t1_mensuelle" })),
        "reduction_generale_permanent" => si(mensuelle, &|| l.reduction_a_l_urssaf(&l81)),
        // Les 81.003 de la cotisation 129 deposee (aucune, pour un mois depose sans elles : la correction declare
        // alors toute la contribution, qui etait due).
        "cpf_cdd" => Ok(l81.iter().filter(|x| x.code == CODE81_CPF_CDD).fold(zero(), |s, x| add(s, x.base.unwrap_or_else(zero)))),
        // Les 81.003 de la cotisation 128 deposee d'un permanent, comme `assiettes_ctp`.
        ASSIETTE_CFP_PERMANENT => si(mensuelle, &|| Ok(l81.iter().filter(|x| x.code == CODE81_CFP).fold(zero(), |s, x| add(s, x.base.unwrap_or_else(zero))))),
        "brut_intermittent" => si(!mensuelle, &|| Ok(brut)),
        "csg_intermittent" => si(!mensuelle, &|| base("csg")),
        "ta_principale" => if mensuelle && !caisse { Ok(brut) } else { base("brut_x110") },
        "brut_technicien" => si(tech, &|| Ok(brut)),
        "brut_x115_technicien" => si(tech, &|| base("brut_x115")),
        "brut_artiste" => si(art, &|| Ok(brut)),
        "brut_x115_artiste" => si(art, &|| base("brut_x115")),
        "reduction_generale_urssaf" => si(tech, &|| l.reduction_a_l_urssaf(&l81)),
        // Sous 921, l'assiette plafonnee (`assiettes_ctp`) : ce que la DSN deposee a declare en base 02 et sous le FNAL.
        k if k == plafonnee("t1_permanent") => si(mensuelle, &|| base("t1_mensuelle")),
        k if k == plafonnee("t1_x115_permanent") => {
            si(mensuelle, &|| base(if caisse { "t1_mensuelle_x115" } else { "t1_mensuelle" }))
        }
        k if k == plafonnee("brut_artiste") => si(art, &|| base("brut_plafonne")),
        k if k == plafonnee("brut_x115_artiste") => si(art, &|| base("brut_x115_plafonne")),
        k if k == plafonnee("brut_technicien") => si(tech, &|| base("brut_plafonne")),
        k if k == plafonnee("brut_x115_technicien") => si(tech, &|| base("brut_x115_plafonne")),
        autre => return Err(format!("assiette inconnue : {}", socle::fichiers::repr(autre))),
    })
}

/// La cotisation d'un code a-t-elle un versement 20 hors Urssaf (le bordereau Urssaf se fait par les CTP) ?
fn sert_a_un_versement(orgs: &Value, code: &str) -> bool {
    liste(g(orgs, "versements")).iter().filter(|v| txt(g(v, "libelle")) != "URSSAF").any(|v| {
        codes_du_versement(v).iter().any(|c| c == code || c.split(':').next() == Some(code))
    })
}

/// Les cles de totalisation d'un versement 20 (`codes81` de `config/organismes.json`). Agirc-Arrco y porte 131 et 132
/// (« Cotisation regime unifie Agirc-Arrco », « Cotisation Apec »), et 105 (« y compris Apec »), que la norme 2026.1 ne
/// garde que « pour regularisation des periodes anterieures a 2025 » (cahier technique 2026.1, p. 302 et 305 ;
/// `atelier/reference/norme-dsn/texte/bloc-S21.G00.81.txt`, lignes 79-81).
pub fn codes_du_versement(v: &Value) -> Vec<String> {
    liste(g(v, "codes81")).iter().map(en_texte).collect()
}

/// `cumuler_totaux` de ce qui a ete DECLARE pour le contrat d'une regularisation : ses lignes 81 deposees, chacune a sa
/// cle de totalisation (le bareme du contrat la dit, par le code et l'organisme). Les composants du bloc 55 ne se
/// cumulent plus ici : ils se lisent dans le nominatif ecrit (`composants_de_versement`), ou `annuler_le_declare` a deja
/// rattache chaque base 31 deposee a son adhesion (70.013).
fn cumuler_le_declare(
    totaux: &mut Vec<(String, Decimal)>, r: &Regularisation, bar: &Value, orgs: &Value, signe: i64,
) -> Resultat<()> {
    let d = &r.declare;
    let numero = r.apres.engagement.numero.clone().unwrap_or_default();
    let refus = |quoi: String| {
        format!(
            "Correction de {} (contrat {}) : {}. Les versements de la regularisation portent la difference « corrige moins declare », comme le nominatif ; elle ne se devine pas. Rien n'est ecrit.",
            r.mois, numero, quoi
        )
    };
    let l = lecture(&r.avant, bar)?;
    let (_, l81) = lignes_declarees(d)?;
    let s = Decimal::from(signe);
    for x in &l81 {
        let montant = x.montant.ok_or_else(|| refus(format!("la cotisation {} de {} n'a pas de S21.G00.81.004", x.code, d.fichier)))?;
        let mut cles: Vec<&str> = l.roles_de(&x.code, &x.org).iter().map(|r| r.cle()).collect();
        cles.sort();
        cles.dedup();
        let cle = match cles.as_slice() {
            [une] => une.to_string(),
            _ if !sert_a_un_versement(orgs, &x.code) => continue,
            _ => {
                return Err(refus(format!(
                    "{} declare la cotisation {}{}, que le bareme de {} {} ; a quel versement elle revient ne se lit pas",
                    d.fichier,
                    x.code,
                    if x.org.is_empty() { String::new() } else { format!(" chez {}", x.org) },
                    r.mois,
                    if cles.is_empty() { "ne connait pas pour ce contrat".to_string() } else { format!("rattache a plusieurs totaux ({})", cles.join(", ")) }
                )))
            }
        };
        ajouter(totaux, &cle, mul(s, montant));
        if x.code == "059" && x.affiliation.is_none() {
            return Err(refus(format!(
                "une cotisation 059 de {} n'est rattachee a aucune affiliation (S21.G00.78.005) : son composant de versement (S21.G00.55) ne se lit pas",
                d.fichier
            )));
        }
    }
    Ok(())
}

// --------------------------------------------- totaux, bordereau, versements

fn cumuler_totaux(totaux: &mut Vec<(String, Decimal)>, paies: &[&Paie], bar: &Value, signe: i64) -> Resultat<()> {
    let s = Decimal::from(signe);
    for p in paies {
        for c in cotisations(p, bar)? {
            let m = match c.montant {
                Some(m) => m,
                None => continue,
            };
            let cle = c.cle_total.clone().unwrap_or_else(|| c.code81.clone());
            ajouter(totaux, &cle, mul(s, m));
        }
    }
    Ok(())
}

fn ajouter(v: &mut Vec<(String, Decimal)>, cle: &str, m: Decimal) {
    match v.iter_mut().find(|(k, _)| k == cle) {
        Some((_, x)) => *x = add(*x, m),
        None => v.push((cle.to_string(), add(zero(), m))),
    }
}

fn lire(v: &[(String, Decimal)], cle: &str) -> Option<Decimal> {
    v.iter().find(|(k, _)| k == cle).map(|(_, x)| *x)
}

fn taux_at_du_ctp(ctp: &Value, bar: &Value) -> Resultat<String> {
    if !vrai(g(ctp, "taux_at")) {
        return Ok(String::new());
    }
    // LE TAUX DU BAREME DU MOIS DECLARE (09/10/2026), et non celui recopie dans config/organismes.json : celui-la
    // restait fige quand le taux changeait a l'ecran (1,25 garde, 1,40 pose le 03/10), ou en cours d'annee (1,69 puis
    // 1,65 au 01/06/2025). `taux_at` du CTP ne dit plus que « cette ligne porte le taux AT » ("bareme").
    if !vrai(g(g(bar, "taux_atmp"), "valeur")) {
        if txt(g(ctp, "taux_at")) == "bareme" {
            return Err("Le bareme du mois n'a pas de taux AT/MP (`taux_atmp.valeur`), et un CTP de la DSN le porte. C'est \
le taux notifie par l'Assurance Maladie (Configuration > Entreprise). Rien n'est ecrit."
                .to_string());
        }
        return Ok(en_texte(&ctp["taux_at"]));
    }
    let mut valeur = dec_avec_echelle(g(g(bar, "taux_atmp"), "valeur"))?;
    if txt(g(ctp, "statut")) == "artiste" {
        valeur = mul(valeur, Decimal::new(70, 2));
    }
    let q = valeur.round_dp_with_strategy(2, RoundingStrategy::MidpointNearestEven);
    Ok(format!("{:05.2}", q.to_string().parse::<f64>().unwrap_or(0.0)))
}

fn ligne(code: String, qualifiant: String, taux_at: String, assiette: Option<Decimal>, montant: Option<Decimal>, commune: Option<String>) -> LigneBordereau {
    LigneBordereau { code, qualifiant, taux_at, assiette, montant, commune }
}

fn ecrire_lignes_23(f: &mut Fichier, lignes: &[LigneBordereau]) {
    for l in lignes {
        f.add("S21.G00.23.001", &l.code);
        f.add("S21.G00.23.002", &l.qualifiant);
        if !l.taux_at.is_empty() {
            f.add("S21.G00.23.003", &l.taux_at);
        }
        match l.montant {
            Some(m) => f.add("S21.G00.23.005", mt(m)),
            None => f.add("S21.G00.23.004", mt(l.assiette.unwrap_or_default())),
        }
        if let Some(c) = l.commune.as_deref().filter(|c| !c.is_empty()) {
            f.add("S21.G00.23.006", c);
        }
    }
}

fn statut(p: &Paie) -> String {
    let s = txt(g(&p.salarie, "statut"));
    if s.is_empty() { "artiste".into() } else { s }
}

/// `_assiettes_ctp` : les assiettes agregees du bordereau Urssaf.
fn assiettes_ctp(paies: &[&Paie], bar: &Value) -> Resultat<HashMap<String, Decimal>> {
    // Sous deduction forfaitaire specifique, les assiettes de la securite sociale sont celles des bases abattues
    // (`cle_abattue`, la base 03 ; le 51 de type 001, `brut_du_51`) ; la CSG garde la sienne.
    let mut b = Vec::new();
    for p in paies {
        let mut x = bases(p, bar)?;
        for cle in ["brut", "brut_x115", "brut_x110", "brut_plafonne", "brut_x115_plafonne"] {
            if x.contains_key(cle) {
                let k = cle_abattue(p, "03", cle)?;
                let v = x[&k];
                x.insert(cle.to_string(), v);
            }
        }
        b.push(x);
    }
    let somme = |idx: &dyn Fn(usize) -> bool, f: &dyn Fn(usize) -> Decimal| -> Decimal {
        (0..paies.len()).filter(|i| idx(*i)).fold(zero(), |s, i| add(s, f(i)))
    };
    let tous = |_: usize| true;
    let mut out = HashMap::new();
    for k in ["brut", "csg", "brut_x115", "brut_x110"] {
        out.insert(k.to_string(), somme(&tous, &|i| b[i][k]));
    }
    let mensuelle: Vec<bool> = paies.iter().map(|p| p.engagement.nat().map(|n| n.mensuelle())).collect::<Resultat<_>>()?;
    let perm = |i: usize| mensuelle[i];
    let inter = |i: usize| !mensuelle[i];
    out.insert("brut_permanent".into(), somme(&perm, &|i| b[i]["brut"]));
    out.insert("csg_permanent".into(), somme(&perm, &|i| b[i]["csg"]));
    out.insert("t1_permanent".into(), somme(&perm, &|i| paies[i].bases.get("t1_mensuelle").copied().unwrap_or(zero())));
    // Le chomage et l'AGS des permanents (CTP 772 et 937, qualifiant 920) : la somme des bases 07 que leur nominatif
    // declare (`base_07`). Le tableau d'equivalence de l'Urssaf rattache ces deux CTP a la base 07 (040 et 048) sous le
    // qualifiant 920 (atelier/reference/sources/chomage-permanents/urssaf-equivalence-dida-20260611.csv, lignes 1640 et
    // 280) ; c'est le brut dans la limite de quatre plafonds
    // de la periode.
    let mut chomage = zero();
    for i in (0..paies.len()).filter(|i| perm(*i)) {
        chomage = add(chomage, base_07(paies[i], bar)?);
    }
    out.insert("chomage_permanent".into(), chomage);
    // La contribution CPF-CDD (CTP 987, qualifiant 920) : la somme des 81.003 de la cotisation 129 que le nominatif de
    // chaque contrat declare (DIDA : 987 -> 129, base 03).
    let mut cpf_cdd = zero();
    for p in paies {
        for c in cotisations(p, bar)?.into_iter().filter(|c| c.code81 == CODE81_CPF_CDD) {
            cpf_cdd = add(cpf_cdd, c.base81.unwrap_or(c.base78));
        }
    }
    out.insert("cpf_cdd".into(), cpf_cdd);
    // La contribution a la formation professionnelle des permanents (CTP 959 ou 971, qualifiant 920) : la somme des
    // 81.003 de leur cotisation 128, l'assiette majoree de 11,5 % d'un contrat declare a la caisse des Conges Spectacles
    // (guide Urssaf des contributions de formation, 12/03/2026 ; fiche net-entreprises 2503 :
    // atelier/reference/sources/majoration-caisses-conges/LISEZ-MOI.md).
    let mut cfp = zero();
    for i in (0..paies.len()).filter(|i| perm(*i)) {
        for c in cotisations(paies[i], bar)?.into_iter().filter(|c| c.code81 == CODE81_CFP) {
            cfp = add(cfp, c.base81.unwrap_or(c.base78));
        }
    }
    out.insert(ASSIETTE_CFP_PERMANENT.into(), cfp);
    // La reduction generale que l'Urssaf recouvre (`reduction_a_l_urssaf`) : pour un permanent, CTP 668 « reduction
    // generale etendue », sa part securite sociale ET sa part chomage ; pour un technicien intermittent, CTP 671, la
    // seule part securite sociale (France Travail recouvre son chomage).
    let (mut rg_perm, mut rg_tech) = (zero(), zero());
    for i in 0..paies.len() {
        if perm(i) {
            rg_perm = add(rg_perm, reduction_a_l_urssaf(paies[i], bar)?);
        } else if statut(paies[i]) == "technicien" {
            rg_tech = add(rg_tech, reduction_a_l_urssaf(paies[i], bar)?);
        }
    }
    out.insert("reduction_generale_permanent".into(), rg_perm);
    out.insert("brut_intermittent".into(), somme(&inter, &|i| b[i]["brut"]));
    out.insert("csg_intermittent".into(), somme(&inter, &|i| b[i]["csg"]));
    // La taxe d'apprentissage sur l'assiette majoree de chaque contrat declare a la caisse (D. 243-0-2), le brut des
    // autres (une majoration de 1,0) : intermittents et permanents ensemble.
    out.insert("ta_principale".into(), somme(&tous, &|i| b[i]["brut_x110"]));
    // Le FNAL plafonne des permanents (CTP 332 sous 921) : la tranche 1, majoree pour un contrat declare a la caisse
    // (`calcul::calculer`, `t1_mensuelle_x115`). Le 100 sous 921 garde `t1_permanent`.
    out.insert(
        "t1_x115_permanent".into(),
        somme(&perm, &|i| paies[i].bases.get("t1_mensuelle_x115").or(paies[i].bases.get("t1_mensuelle")).copied().unwrap_or(zero())),
    );
    let tech = |i: usize| inter(i) && statut(paies[i]) == "technicien";
    let art = |i: usize| inter(i) && !tech(i);
    out.insert("brut_technicien".into(), somme(&tech, &|i| b[i]["brut"]));
    out.insert("brut_x115_technicien".into(), somme(&tech, &|i| b[i]["brut_x115"]));
    out.insert("brut_artiste".into(), somme(&art, &|i| b[i]["brut"]));
    out.insert("brut_x115_artiste".into(), somme(&art, &|i| b[i]["brut_x115"]));
    out.insert("reduction_generale_urssaf".into(), rg_tech);
    // Sous le qualifiant 921 (« Assiette plafonnee », cahier technique 2026.1, p. 168), une population porte son assiette
    // PLAFONNEE : celle du bulletin et de la base 02 de chaque contrat (point 22). La cle de config/organismes.json reste
    // celle de la population (`brut_artiste` sous 312/921) ; c'est le qualifiant qui dit plafonnee.
    let base = |i: usize, k: &str| b[i].get(k).copied().ok_or_else(|| socle::fichiers::absent(k));
    let plaf = |f: &dyn Fn(usize) -> bool, k: &str| -> Resultat<Decimal> {
        (0..paies.len()).filter(|i| f(*i)).try_fold(zero(), |s, i| Ok(add(s, base(i, k)?)))
    };
    out.insert(plafonnee("t1_permanent"), out["t1_permanent"]);
    out.insert(plafonnee("t1_x115_permanent"), out["t1_x115_permanent"]);
    out.insert(plafonnee("brut_artiste"), plaf(&art, "brut_plafonne")?);
    out.insert(plafonnee("brut_x115_artiste"), plaf(&art, "brut_x115_plafonne")?);
    out.insert(plafonnee("brut_technicien"), plaf(&tech, "brut_plafonne")?);
    out.insert(plafonnee("brut_x115_technicien"), plaf(&tech, "brut_x115_plafonne")?);
    Ok(out)
}

/// La base 07 (« Assiette des contributions d'Assurance Chomage », norme 2026.1 p. 294) que le nominatif du contrat
/// declare : le 78.004 des lignes de `cotisations` sous le bloc 07, le meme montant que le bulletin (la base de la ligne
/// chomage, `brut_chomage` dans les baremes). Zero sans bloc 07 ; deux montants differents refusent.
fn base_07(p: &Paie, bar: &Value) -> Resultat<Decimal> {
    let mut vu: Option<Decimal> = None;
    for c in cotisations(p, bar)?.into_iter().filter(|c| c.bloc78 == "07") {
        match vu {
            Some(v) if v != c.base78 => {
                return Err(format!(
                    "Contrat {} : le bareme declare deux bases 07 differentes ({} et {}). Rien n'est ecrit.",
                    p.engagement.numero.as_deref().unwrap_or("sans numero"),
                    mt(v),
                    mt(c.base78)
                ))
            }
            _ => vu = Some(c.base78),
        }
    }
    Ok(vu.unwrap_or_else(zero))
}

/// Le qualifiant d'assiette plafonnee de la table des CTP (S21.G00.23.002, p. 168).
const QUALIFIANT_PLAFONNEE: &str = "921";

/// La cle, dans `assiettes_ctp`, de l'assiette plafonnee d'une population.
fn plafonnee(cle: &str) -> String {
    format!("{}:{}", QUALIFIANT_PLAFONNEE, cle)
}

/// La cle d'une ligne de la table des CTP dans `assiettes_ctp` : sa cle de montant, ou son assiette, plafonnee sous 921.
fn cle_ctp(ctp: &Value) -> Resultat<(String, bool)> {
    let par_montant = vrai(g(ctp, "montant"));
    if par_montant {
        return Ok((en_texte(&ctp["montant"]), true));
    }
    let k = en_texte(req(ctp, "assiette")?);
    Ok((if txt(g(ctp, "qualifiant")) == QUALIFIANT_PLAFONNEE { plafonnee(&k) } else { k }, false))
}

fn valeur_ctp(somme: &HashMap<String, Decimal>, ctp: &Value) -> Resultat<(Decimal, bool)> {
    let (k, par_montant) = cle_ctp(ctp)?;
    match somme.get(&k) {
        Some(v) => Ok((*v, par_montant)),
        None if k.starts_with(&plafonnee("")) => Err(format!(
            "config/organismes.json : le CTP {} sous le qualifiant {} (assiette plafonnee, cahier technique 2026.1, p. 168) \
lit l'assiette « {} », qui n'a pas de forme plafonnee. Rien n'est ecrit.",
            txt(g(ctp, "code")),
            QUALIFIANT_PLAFONNEE,
            &k[plafonnee("").len()..]
        )),
        None => Err(format!("assiette inconnue : {}", socle::fichiers::repr(&k))),
    }
}

/// La contribution CPF-CDD : sa cotisation au nominatif (norme DSN 2026.1, S21.G00.81.001 : « 129 - Contribution dediee
/// au financement du Compte Personnel de Formation pour les titulaires de CDD (CPF-CDD) »).
const CODE81_CPF_CDD: &str = "129";
/// La contribution a la formation professionnelle au nominatif (« 128 - Contribution a la formation professionnelle
/// (CFP) », fiche net-entreprises 2503) ; l'assiette de ses CTP de droit commun (959 sous onze salaries, 971 au-dela)
/// est la somme de ses 81.003 chez les permanents (`ASSIETTE_CFP_PERMANENT`).
const CODE81_CFP: &str = "128";
const ASSIETTE_CFP_PERMANENT: &str = "cfp_permanent";

/// La table des CTP de `config/organismes.json`, telle que `organismes::composer` l'ecrit depuis
/// `donnees/organismes-base.json`.
fn ctp_urssaf(orgs: &Value) -> Resultat<Vec<Value>> {
    Ok(liste(req(orgs, "ctp_urssaf")?).to_vec())
}

/// Les seuils d'effectif d'un mois, pour les CTP qui en dependent (`effectif_au_moins`, `effectif_sous` d'une ligne de
/// `ctp_urssaf`) : l'effectif A01 que l'Urssaf renvoie en CRM 118 (`config/effectifs.json`, regle 8), la regle des cinq
/// annees civiles consecutives (L. 130-1, II, CSS ; `Effectif::atteint`), pour payer l'annee du mois. Comme
/// l'echeance de la DSN (`Contexte::echeance_dsn`), un effectif que le moteur ne connait pas tient le seuil pour non
/// franchi : le bareme du mois l'a deja exige s'il porte `seuils_effectif` (`contexte::controler_effectif` refuse alors
/// de payer), et seul un bareme complet pris tel quel (sans `seuils_effectif`) passe sans lui.
pub struct Seuils {
    effectif: droit::contexte::Effectif,
    an: i32,
}

impl Seuils {
    fn du_mois(ctx: &Contexte, mois: &str) -> Resultat<Seuils> {
        let an: i32 = mois.get(..4).and_then(|a| a.parse().ok()).ok_or_else(|| format!("mois illisible : {}", mois))?;
        Ok(Seuils { effectif: ctx.effectif()?, an })
    }

    fn atteint(&self, seuil: i64) -> bool {
        matches!(self.effectif.atteint(seuil, self.an), Ok(true))
    }

    /// La ligne `ctp` du fichier des organismes s'ecrit-elle a l'effectif de ce mois ?
    fn retient(&self, ctp: &Value) -> Resultat<bool> {
        let seuil = |k: &str| -> Resultat<Option<i64>> {
            match g(ctp, k) {
                Value::Null => Ok(None),
                v => v.as_i64().map(Some).ok_or_else(|| format!("config/organismes.json : `{}` du CTP {} n'est pas un entier. Rien n'est ecrit.", k, txt(g(ctp, "code")))),
            }
        };
        Ok(seuil("effectif_au_moins")?.map_or(true, |s| self.atteint(s)) && seuil("effectif_sous")?.map_or(true, |s| !self.atteint(s)))
    }
}

/// `assujettissement::taxe_apprentissage_non_due_au_mois` pour un mois `AAAA-MM`.
fn taxe_apprentissage_non_due_du_mois(ent: &Value, mois: &str) -> Resultat<bool> {
    let m: u32 = mois.get(5..7).and_then(|m| m.parse().ok()).ok_or_else(|| format!("mois illisible : {}", mois))?;
    droit::apprentissage::taxe_apprentissage_non_due_au_mois(ent, annee_du_mois(mois)?, m)
}

fn lignes_bordereau(
    paies: &[&Paie], orgs: &Value, ent: &Value, bar: &Value, solde_ta: Option<&SoldeTa>, doeth: Option<&crate::oeth::Declaration>,
    csa: Option<&crate::csa::Csa>, mois: &str, effectif: &Seuils,
) -> Resultat<Vec<LigneBordereau>> {
    let mut lignes = Vec::new();
    let somme = assiettes_ctp(paies, bar)?;
    let ctps = crate::assujettissement::ctp_sans_la_taxe_non_due(ctp_urssaf(orgs)?, taxe_apprentissage_non_due_du_mois(ent, mois)?);
    let ctps = ctps.as_slice();
    if !somme["brut_permanent"].is_zero() && !ctps.iter().any(|c| txt(g(c, "nature")) == "permanent") {
        return Err(format!(
            "La DSN de {} porte {} EUR de brut de salaries permanents, et config/organismes.json n'a aucun CTP marque \
\"nature\": \"permanent\". Le bordereau Urssaf serait faux : un permanent releve du regime general (CTP 100 et \
suivants) et son assurance chomage est recouvree par l'Urssaf, contrairement a celle d'un intermittent. Relever les \
CTP dans la table de l'Urssaf et les ajouter avant de declarer. Rien n'est ecrit.",
            mois,
            mt(somme["brut_permanent"])
        ));
    }
    for ctp in ctps {
        if !effectif.retient(ctp)? {
            continue;
        }
        let (valeur, par_montant) = valeur_ctp(&somme, ctp)?;
        if (vrai(g(ctp, "statut")) || vrai(g(ctp, "nature"))) && valeur.is_zero() {
            continue;
        }
        let v = rond(valeur);
        lignes.push(ligne(
            en_texte(req(ctp, "code")?),
            en_texte(req(ctp, "qualifiant")?),
            taux_at_du_ctp(ctp, bar)?,
            if par_montant { None } else { Some(v) },
            if par_montant { Some(v) } else { None },
            None,
        ));
    }
    if let Some(vm) = bareme::versement_mobilite(bar)? {
        // L'assiette majoree de l'intermittent (CSS L. 243-1-3 ; `bareme::bareme_vm`), le brut du permanent : celle du
        // bloc 81 de chacun. « Pour le versement de transport, le CTP 900 » (Tissot, 2017 ;
        // atelier/reference/sources/majoration-caisses-conges/).
        let (mut artistes, mut autres) = (zero(), zero());
        for p in paies {
            let a = bases(p, bar)?["brut_x115"];
            if statut(p) == "artiste" {
                artistes = add(artistes, a);
            } else {
                autres = add(autres, a);
            }
        }
        for (code, assiette) in [("900", autres), ("921", artistes)] {
            if assiette.is_zero() {
                continue;
            }
            lignes.push(ligne(code.into(), "920".into(), fmt_f(vm.taux, 2), Some(rond(assiette)), None, Some(vm.code_insee.clone())));
        }
    }
    let mut local = zero();
    for p in paies {
        if bareme::regime_local(bar, &p.salarie) {
            local = add(local, bases(p, bar)?["brut"]);
        }
    }
    if !local.is_zero() {
        let ctp = g(g(bar, "regime_local_alsace_moselle"), "ctp");
        let code = if vrai(ctp) { en_texte(ctp) } else { "381".into() };
        lignes.push(ligne(code, "920".into(), String::new(), Some(rond(local)), None, None));
    }
    if let Some(s) = solde_ta {
        lignes.push(ligne("995".into(), "920".into(), String::new(), Some(s.assiette), None, None));
    }
    if let Some(d) = doeth {
        for (ctp, montant) in d.bordereau() {
            lignes.push(ligne(ctp, "920".into(), String::new(), Some(montant), None, None));
        }
    }
    // La contribution supplementaire a l'apprentissage de mars : le CTP 998, son montant en 23.004 (taux de 100 %),
    // comme le CTP 730 de l'OETH (guide Urssaf du 12/03/2026, l. 2453-2458 ; `csa`).
    if let Some((ctp, montant)) = csa.and_then(crate::csa::Csa::bordereau) {
        lignes.push(ligne(ctp.into(), "920".into(), String::new(), Some(montant), None, None));
    }
    Ok(lignes)
}

/// LE BORDEREAU URSSAF D'UN MOIS, tel que la DSN le porte, tire des paies `paies` par les fonctions qui l'y ecrivent
/// (`lignes_bordereau`, `bordereau::montant`) : ses lignes 23, valorisees a la table des CTP, et son total (22.005). Les
/// corrections des mois deja declares n'y sont pas : elles ont leurs propres bordereaux. `etablissement` : le NIC, le
/// principal sans lui.
pub fn bordereau_du_mois(ctx: &Contexte, paies: &[Paie], mois: &str, etablissement: Option<&str>) -> Resultat<(Vec<LigneBordereau>, Decimal)> {
    let ctx = &ctx.du_mois(mois)?;
    let ent = &ctx.entreprise;
    let principal = bareme::nic_principal(ent);
    let nic = etablissement.filter(|n| !n.is_empty()).map_or_else(|| principal.clone(), str::to_string);
    let orgs = ctx.organismes_exiges()?;
    let debut = premier(mois)?;
    let bar = bar_du(ctx, ent, &orgs, &nic, mois)?;
    let doeth = if nic == principal { crate::oeth::declaration(ctx, mois)? } else { None };
    let csa = if nic == principal { crate::csa::declaration(ctx, mois)? } else { None };
    let solde_ta = solde_taxe_apprentissage(ctx, mois, &orgs, &bar, &nic)?;
    let refs: Vec<&Paie> = paies.iter().flat_map(|p| p.elementaires()).collect();
    let effectif = Seuils::du_mois(ctx, mois)?;
    let lignes = lignes_bordereau(&refs, &orgs, ent, &bar, solde_ta.as_ref(), doeth.as_ref(), csa.as_ref(), mois, &effectif)?;
    let total = bordereau::montant(&lignes, &debut)?;
    Ok((lignes, total))
}

/// Le total du bordereau Urssaf d'un mois (22.005), pour l'apercu, avant de produire (`bordereau_du_mois`).
pub fn montant_urssaf(ctx: &Contexte, paies: &[Paie], mois: &str, etablissement: Option<&str>) -> Resultat<Decimal> {
    Ok(bordereau_du_mois(ctx, paies, mois, etablissement)?.1)
}

/// Le bordereau Urssaf d'un mois d'origine que la DSN courante regularise : par CTP, la difference « corrige moins
/// declare » (`assiette_declaree`), la meme que celle que porte le nominatif.
///
/// `bar_o` : le bareme du mois d'origine POUR L'ETABLISSEMENT de la DSN (`bar_du`), le meme que celui du nominatif de la
/// correction et du bordereau courant. Point 25 : il etait pris sans `surcharger`, et un etablissement
/// secondaire valorisait son bordereau regularise au taux AT/MP du principal (23.003, `taux_at_du_ctp`) ; regle 8,
/// chaque etablissement a le sien.
fn lignes_regularisees(
    orgs: &Value, ent: &Value, regs: &[Regularisation], mois_o: &str, bar_o: &Value, effectif: &Seuils,
) -> Resultat<Vec<LigneBordereau>> {
    let siens: Vec<&Regularisation> = regs.iter().filter(|r| r.mois == mois_o).collect();
    let apres = assiettes_ctp(&siens.iter().map(|r| &r.apres).collect::<Vec<_>>(), bar_o)?;
    let mut lignes = Vec::new();
    let ctps = crate::assujettissement::ctp_sans_la_taxe_non_due(ctp_urssaf(orgs)?, taxe_apprentissage_non_due_du_mois(ent, mois_o)?);
    for ctp in &ctps {
        if !effectif.retient(ctp)? {
            continue;
        }
        let (corrige, par_montant) = valeur_ctp(&apres, ctp)?;
        let (cle, _) = cle_ctp(ctp)?;
        let mut declare = zero();
        for r in &siens {
            match assiette_declaree(&cle, r, bar_o)? {
                Ok(v) => declare = add(declare, v),
                Err(e) => {
                    return Err(format!(
                        "Correction de {} (contrat {}) : la ligne {} ({}) du bordereau Urssaf de ce mois, assiette « {} », ne se recompose pas depuis ce que {} a declare pour le contrat : {}. Le bordereau d'une regularisation porte la difference « corrige moins declare », comme le nominatif ; elle ne se devine pas. Rien n'est ecrit.",
                        r.mois,
                        r.apres.engagement.numero.as_deref().unwrap_or("sans numero"),
                        en_texte(req(ctp, "code")?),
                        en_texte(req(ctp, "qualifiant")?),
                        cle,
                        r.declare.fichier,
                        e
                    ))
                }
            }
        }
        let v = rond(sub(corrige, declare));
        if v.is_zero() {
            continue;
        }
        let taux_at = taux_at_du_ctp(ctp, bar_o)?;
        let code = en_texte(req(ctp, "code")?);
        if par_montant && v.is_sign_negative() {
            // Une reduction qui BAISSE (un salaire corrige a la hausse) : son CTP de regularisation, en assiette (23.004)
            // et sans signe, et non le CTP de la reduction en montant negatif. Guide declaratif Urssaf 5.1, § 8.2,
            // « Schema de l'attendu declaratif » : « Ctp 668/671 : […] Montant de cotisation (reduction) XX ; Ctp
            // 669/801 : Montant d'assiette XX », « les montants ne doivent pas comporter de signe ; les CTP comprennent
            // les signes necessaires » (atelier/reference/sources/editeur-dsn/guide-urssaf-regularisation.txt, lignes
            // 1341-1362). La table des CTP valorise 669 et 801 (format E) a 100 % : le montant du bordereau est celui
            // qu'aurait deduit le montant negatif.
            let qualifiant = en_texte(req(ctp, "qualifiant")?);
            let regularisation = ctp_de_regularisation(&code, &siens, bar_o)?.ok_or_else(|| {
                format!(
                    "Correction de {} : la ligne {} ({}) du bordereau Urssaf baisse de {} EUR, et le bareme du mois ne dit pas sous quel CTP se regularise cette reduction (`reduction_generale.dsn.ctp_regularisation`) : un montant de reduction ne s'ecrit pas negatif (guide declaratif Urssaf, § 8.2). Rien n'est ecrit.",
                    mois_o,
                    code,
                    qualifiant,
                    mt(-v)
                )
            })?;
            lignes.push(ligne(regularisation, qualifiant, taux_at, Some(-v), None, None));
            continue;
        }
        lignes.push(ligne(
            code,
            en_texte(req(ctp, "qualifiant")?),
            taux_at,
            if par_montant { None } else { Some(v) },
            if par_montant { Some(v) } else { None },
            None,
        ));
    }
    Ok(lignes)
}

/// Le CTP qui regularise le CTP de reduction `code` (668 -> 669, 671 -> 801) : `reduction_generale.dsn.ctp_regularisation`
/// de la pile de bareme d'un des contrats corriges, celle dont `reduction_generale.dsn.ctp` est ce code (le bareme livre :
/// `par_nature.permanent` et `par_statut.technicien`). Aucun : `None`.
fn ctp_de_regularisation(code: &str, siens: &[&Regularisation], bar: &Value) -> Resultat<Option<String>> {
    for r in siens {
        for p in [&r.apres, &r.avant] {
            let pile = bareme_de(p, bar)?;
            let dsn = g(g(&pile, "reduction_generale"), "dsn");
            if txt(g(dsn, "ctp")) == code {
                if let Some(x) = opt_txt(g(dsn, "ctp_regularisation")).filter(|x| !x.is_empty()) {
                    return Ok(Some(x));
                }
            }
        }
    }
    Ok(None)
}

/// S21.G00.20.002 d'un versement : la reference de l'organisme.
pub fn reference_versement(v: &Value, ent: &Value) -> Resultat<Option<String>> {
    let reference = if vrai(g(v, "reference")) { Some(en_texte(&v["reference"])) } else { None };
    if txt(g(v, "siret")) != SIRET_FRANCE_TRAVAIL {
        return Ok(reference);
    }
    let fiche = Some(txt(g(ent, "france_travail_dossier_recouvrement")).trim().to_string()).filter(|s| !s.is_empty());
    if let (Some(fi), Some(r)) = (&fiche, &reference) {
        if fi != r {
            return Err(format!(
                "Deux dossiers de recouvrement France Travail : {} dans la fiche Entreprise, {} dans \
config/organismes.json. Rien n'est ecrit : corrigez celui qui est faux.",
                fi, r
            ));
        }
    }
    Ok(fiche.or(reference))
}

fn montant_versement(v: &Value, totaux: &[(String, Decimal)], brut_mois: Decimal) -> Resultat<Decimal> {
    let regle = g(v, "montant");
    if vrai(regle) && txt(g(regle, "mode")) == "taux_sur_brut" {
        let m = div(mul(brut_mois, dec_avec_echelle(req(regle, "taux")?)?), Decimal::ONE_HUNDRED);
        return Ok(if txt(g(regle, "arrondi")) == "euro" { rond(m) } else { eur(m) });
    }
    req(v, "codes81")?;
    Ok(codes_du_versement(v).iter().fold(zero(), |s, c| add(s, lire(totaux, c).unwrap_or(zero()))))
}

// ------------------------------------ le versement a un organisme complementaire
//
// Les composants d'un versement de prevoyance (bloc S21.G00.55, norme 2026.1, `atelier/reference/norme-dsn/texte/
// bloc-S21.G00.55.txt`) se LISENT dans ce que le fichier declare, une fois les individus ecrits : chaque cotisation
// d'une base rattachee a une affiliation (78.005) va au contrat que cette affiliation reference (70.012 -> 70.013 ->
// 15.005), et le code d'affectation est la reference de ce contrat (« Le contrat mentionne doit faire l'objet d'un bloc
// Adhesion Prevoyance S21.G00.15 dans la declaration », p. 260, lignes 72-74 ; CCH-11, lignes 75-79 : le couple
// 20.001 / 55.003 se retrouve en 15.002 / 15.001). Le montant d'un composant est « la part du versement affectee au
// contrat [...] pour la periode d'affectation » (p. 259, lignes 44-46) ; la periode est « une periode civile complete
// (mois, trimestre, semestre ou annee) contenant la ou les periodes cotisees » (p. 260, lignes 85-87). Le versement
// (20.005) est la somme de ses composants, ou zero si elle est negative (bloc-S21.G00.20.txt, p. 157, lignes 191-193
// et CCH-12 lignes 211-213).

/// Un composant de versement (bloc S21.G00.55), lu dans le fichier.
#[derive(Clone, Debug, PartialEq)]
pub struct Composant {
    /// L'organisme de l'adhesion (S21.G00.15.002), celui du versement (S21.G00.20.001).
    pub organisme: String,
    /// La reference du contrat chez l'organisme (S21.G00.15.001), le code d'affectation (S21.G00.55.003).
    pub code: String,
    /// AAAAPNN (S21.G00.55.004).
    pub periode: String,
    pub montant: Decimal,
}

/// La plus petite periode civile (trimestre, semestre, annee) qui contient les deux dates JJMMAAAA d'une base 78. Le
/// trimestre etait deja la periode ecrite pour un mois courant ; la norme l'admet (p. 260, ligne 89).
fn periode_d_affectation(debut: &str, fin: &str) -> Resultat<String> {
    let lire = |d: &str| -> Option<(u32, u32)> {
        if d.len() != 8 {
            return None;
        }
        Some((d[4..].parse().ok()?, d[2..4].parse().ok()?))
    };
    let ((a1, m1), (a2, m2)) = match (lire(debut), lire(fin)) {
        (Some(x), Some(y)) if (1..=12).contains(&x.1) && (1..=12).contains(&y.1) => (x, y),
        _ => return Err(format!("S21.G00.55.004 : periode cotisee illisible ({} - {}).", debut, fin)),
    };
    if a1 != a2 {
        return Err(format!(
            "S21.G00.55.004 : une base de prevoyance couvre du {} au {}, a cheval sur deux annees civiles ; la periode \
d'affectation est « une periode civile complete [...] contenant la ou les periodes cotisees » (norme p. 260) et aucune \
ne les contient toutes deux. Rien n'est ecrit.",
            debut, fin
        ));
    }
    let (t1, t2) = ((m1 - 1) / 3 + 1, (m2 - 1) / 3 + 1);
    let (s1, s2) = ((m1 - 1) / 6 + 1, (m2 - 1) / 6 + 1);
    Ok(if t1 == t2 {
        format!("{}T{:02}", a1, t1)
    } else if s1 == s2 {
        format!("{}S{:02}", a1, s1)
    } else {
        format!("{}A00", a1)
    })
}

/// Les composants de versement que declarent les lignes `lignes` (le fichier en cours, individus ecrits), dans l'ordre
/// ou ils apparaissent, cumules par organisme, code d'affectation et periode.
pub fn composants_de_versement(lignes: &[String]) -> Resultat<Vec<Composant>> {
    let lire = |l: &str| -> (String, String) {
        let (k, v) = l.split_once(',').unwrap_or((l, ""));
        (k.to_string(), v.trim().trim_matches('\'').to_string())
    };
    // 15.005 -> (15.001, 15.002)
    let mut adhesions: HashMap<String, (String, String)> = HashMap::new();
    let (mut lib15, mut org15) = (String::new(), String::new());
    // 70.012 -> 70.013, pour l'individu en cours (les identifiants y sont uniques, `bloc_contrat`).
    let mut ids: HashMap<String, String> = HashMap::new();
    let mut id70: Option<String> = None;
    let mut nom = String::new();
    // La base 78 en cours : (78.002, 78.003, 78.005) ; et le code de la cotisation 81 en cours.
    let mut base: Option<(String, String, Option<String>)> = None;
    let mut code81 = String::new();
    let mut out: Vec<Composant> = Vec::new();
    for l in lignes {
        let (k, v) = lire(l);
        let bloc = k.get(..10).unwrap_or("");
        if !matches!(bloc, "S21.G00.78" | "S21.G00.79" | "S21.G00.81") {
            base = None;
        }
        match k.as_str() {
            "S21.G00.15.001" => lib15 = v,
            "S21.G00.15.002" => org15 = v,
            "S21.G00.15.005" => {
                adhesions.insert(v, (lib15.clone(), org15.clone()));
            }
            "S21.G00.30.001" => {
                ids.clear();
                nom.clear();
            }
            "S21.G00.30.002" => nom = v,
            "S21.G00.70.012" => id70 = Some(v),
            "S21.G00.70.013" => {
                if let Some(i) = id70.take() {
                    ids.insert(i, v);
                }
            }
            "S21.G00.78.001" => base = Some((String::new(), String::new(), None)),
            "S21.G00.78.002" => {
                if let Some(b) = base.as_mut() {
                    b.0 = v;
                }
            }
            "S21.G00.78.003" => {
                if let Some(b) = base.as_mut() {
                    b.1 = v;
                }
            }
            "S21.G00.78.005" => {
                if let Some(b) = base.as_mut() {
                    b.2 = Some(v);
                }
            }
            "S21.G00.81.001" => code81 = v,
            "S21.G00.81.004" => {
                let (debut, fin, aff) = match &base {
                    Some((d, f, Some(a))) => (d, f, a),
                    _ => continue,
                };
                let reference = ids.get(aff).ok_or_else(|| {
                    format!(
                        "Salarie {} : la cotisation {} est rattachee a l'affiliation {} (S21.G00.78.005), qu'aucun bloc 70 \
du contrat ne declare. Le code d'affectation de son composant de versement (S21.G00.55.003) ne se lit nulle part. Rien \
n'est ecrit.",
                        nom, code81, aff
                    )
                })?;
                let (code, organisme) = adhesions.get(reference).cloned().ok_or_else(|| {
                    format!(
                        "Salarie {} : le contrat se rattache a l'adhesion {} (S21.G00.70.013), que la declaration ne declare \
pas en bloc S21.G00.15. Le code d'affectation (S21.G00.55.003) doit etre un contrat declare en bloc 15 (norme p. 260, \
CCH-11) : il ne se devine pas. Voir config/organismes.json, `adhesions_prevoyance` (et sa date `depuis`). Rien n'est \
ecrit.",
                        nom, reference
                    )
                })?;
                let montant: Decimal =
                    v.parse().map_err(|_| format!("S21.G00.81.004 illisible pour la cotisation {} : {:?}", code81, v))?;
                let periode = periode_d_affectation(debut, fin)?;
                match out.iter_mut().find(|c| c.organisme == organisme && c.code == code && c.periode == periode) {
                    Some(c) => c.montant = add(c.montant, montant),
                    None => out.push(Composant { organisme, code, periode, montant: add(zero(), montant) }),
                }
            }
            _ => {}
        }
    }
    Ok(out)
}

/// Le bloc 20 d'un organisme complementaire et ses composants 55, ou rien si aucun composant ne porte de montant.
fn versement_complementaire(
    v: &Value, ent: &Value, banque: &Value, composants: &[&Composant], debut: Jour, fin: Jour, echeance: Option<Jour>,
) -> Resultat<Vec<String>> {
    if composants.iter().all(|c| c.montant.is_zero()) {
        return Ok(Vec::new());
    }
    let somme = composants.iter().fold(zero(), |s, c| add(s, c.montant));
    // « sauf si cette somme fait ressortir un montant negatif. Auquel cas un montant nul doit etre renseigne » (p. 157).
    let montant = bordereau::a_verser(somme).montant;
    let mut f = Fichier::nouveau();
    f.add("S21.G00.20.001", en_texte(req(v, "siret")?));
    if let Some(r) = reference_versement(v, ent)? {
        f.add("S21.G00.20.002", r);
    }
    f.add_v("S21.G00.20.003", req(banque, "bic")?)?;
    f.add_v("S21.G00.20.004", req(banque, "iban")?)?;
    f.add("S21.G00.20.005", mt(montant));
    f.add("S21.G00.20.006", debut.jjmmaaaa());
    f.add("S21.G00.20.007", fin.jjmmaaaa());
    f.add("S21.G00.20.010", "05");
    if let Some(j) = echeance {
        f.add("S21.G00.20.011", j.jjmmaaaa());
    }
    for c in composants {
        f.add("S21.G00.55.001", mt(c.montant));
        f.add("S21.G00.55.003", &c.code);
        f.add("S21.G00.55.004", &c.periode);
    }
    Ok(f.lignes)
}

// ------------------------------------------------- le versement a la DGFiP
//
// Le prelevement a la source retenu se reverse par la DSN, dans un bloc 20 a part (norme 2026.1, bloc S21.G00.20,
// `atelier/reference/norme-dsn/texte/bloc-S21.G00.20.txt`) : 20.001 = « DGFIP » (p. 154, ligne 47), 20.002 =
// « DGFIP_PAS » (p. 155, ligne 104 ; CCH-11 p. 156, lignes 125-127), 20.005 « arrondi a l'entier le plus proche »
// (p. 157, lignes 194-199), 20.006 et 20.007 le premier et le dernier jour du mois principal declare (p. 158 ligne
// 248, p. 159 ligne 301), 20.010 « 05 » (CCH-14 p. 161, lignes 428-430), 20.011 « non concerne » (p. 161, ligne 466),
// 20.012 interdite en prelevement SEPA (CCH-13 p. 162, lignes 527-529). Aucun bloc 55 : il est « a destination des
// organismes complementaires » (bloc-S21.G00.55.txt, ligne 34).

/// S21.G00.20.001 et S21.G00.20.002 du versement du prelevement a la source.
pub const DGFIP: &str = "DGFIP";
pub const DGFIP_PAS: &str = "DGFIP_PAS";

/// Les coordonnees bancaires du compte preleve (S21.G00.20.003 et .004) : la fiche entreprise, `banque`. Absentes, la
/// DSN ne s'ecrit pas : un versement en prelevement SEPA (20.010 = 05) les exige (CCH-12 et SIG-13, p. 161).
fn banque_du_prelevement(ent: &Value) -> Resultat<&Value> {
    let banque = g(ent, "banque");
    let manque: Vec<&str> = [("bic", "le BIC (S21.G00.20.003)"), ("iban", "l'IBAN (S21.G00.20.004)")]
        .iter()
        .filter(|(k, _)| txt(g(banque, k)).trim().is_empty())
        .map(|(_, l)| *l)
        .collect();
    if !manque.is_empty() {
        return Err(format!(
            "Coordonnees bancaires absentes de la fiche entreprise : {}. Chaque versement de la DSN (Urssaf, DGFiP, \
caisses) se paie par prelevement SEPA sur ce compte, et la norme exige les deux (S21.G00.20.010 = 05 : CCH-12 et \
SIG-13). Les saisir dans Configuration › Paiement des cotisations (fiche entreprise, `banque.iban` et `banque.bic`, \
sur le RIB de la compagnie). Rien n'est ecrit.",
            manque.join(" et ")
        ));
    }
    Ok(banque)
}

/// Le prelevement a la source que CE fichier retient : la somme des S21.G00.50.009 (versements du mois et
/// regularisations positives) et des S21.G00.56.007 (regularisations d'un indu, negatives). INFERENCE : la norme ne
/// nomme pas cette somme ; le versement DGFiP est le « montant des cotisations reglees » (20.005, p. 156) et le PAS
/// precompte est celui des 50.009 (p. 245) corrige des 56.007 (p. 263).
pub fn pas_retenu(lignes: &[String]) -> Resultat<Decimal> {
    let mut s = zero();
    for l in lignes {
        let (k, v) = l.split_once(',').unwrap_or((l.as_str(), ""));
        if k == "S21.G00.50.009" || k == "S21.G00.56.007" {
            let v = v.trim_matches('\'');
            let m: Decimal = v.parse().map_err(|_| format!("{} illisible : {:?}", k, v))?;
            s = add(s, m);
        }
    }
    Ok(s)
}

/// Le bloc 20 de la DGFiP, ou rien. `du` : le PAS retenu par le fichier ; `remplace` : la DSN est un annule et
/// remplace (type 03 ou 05), et `remplacee_versait_pas` dit si la declaration remplacee portait un versement DGFiP.
#[allow(clippy::too_many_arguments)]
fn versement_dgfip(
    du: Decimal, mois: &str, debut: Jour, fin: Jour, banque: &Value, remplace: bool, remplacee_versait_pas: Option<bool>,
) -> Resultat<Vec<String>> {
    // « arrondi a l'entier le plus proche (les deux decimales etant valorisees a zero) » (p. 157, ligne 194) ; le demi
    // va a l'euro superieur (`bordereau::euro`) : CGI art. 1724, « La liquidation de toutes sommes a recevoir [...] est
    // arrondie a l'euro le plus proche. La fraction d'euro egale a 0,50 est comptee pour 1 »
    // (`atelier/reference/sources/code-general-des-impots/articles-20261013.md`). INFERENCE : la norme ne dit pas le demi,
    // et L. 133-10 CSS ne vise pas la DGFiP ; le versement du prelevement a la source est une somme a recevoir du fisc.
    let mut montant = bordereau::euro(du);
    montant.rescale(2);
    // UN PAS NEGATIF (les regularisations rendent plus que le mois ne retient) : « ces rubriques doivent etre renseignees
    // a 0.00 », et le bloc, que la fiche admet « par commodite » sur une DSN initiale, s'ecrit a zero pour que l'ecran
    // dise le remboursement a demander (fiche net-entreprises 1931, l. 4 et 13 ; `bordereau::a_verser`).
    let credit = du < Decimal::ZERO;
    let montant = bordereau::a_verser(montant).montant;
    let ecrire = if !montant.is_zero() || credit {
        true
    } else if remplace {
        // « Pour une declaration de type annule et remplace sans paiement de Prelevement a la source et deposee en
        // substitution d'une declaration pourvue d'un bloc Versement pour le Prelevement a la source, il est
        // obligatoire de declarer un bloc de Versement [...] a zero » (p. 157, lignes 195-199).
        match remplacee_versait_pas {
            Some(v) => v,
            None => {
                return Err(format!(
                    "Annule et remplace de {} sans prelevement a la source a reverser : si la declaration remplacee \
portait un versement DGFiP, celle-ci doit en porter un a 0.00 (S21.G00.20.005, norme p. 157). La DSN remplacee n'a \
pas pu etre relue (sorties/{}/, le fichier du dernier depot accepte) : on ne devine pas. Rien n'est ecrit.",
                    mois, mois
                ))
            }
        }
    } else {
        // Rien a reverser : aucune rubrique ne l'exige, et le bloc Versement « doit etre renseigne seulement a
        // l'echeance de paiement » (chapitre 2, §2.2.2.6, p. 56, ligne 1900 du texte).
        false
    };
    if !ecrire {
        return Ok(Vec::new());
    }
    let mut f = Fichier::nouveau();
    f.add("S21.G00.20.001", DGFIP);
    f.add("S21.G00.20.002", DGFIP_PAS);
    f.add_v("S21.G00.20.003", req(banque, "bic")?)?;
    f.add_v("S21.G00.20.004", req(banque, "iban")?)?;
    f.add("S21.G00.20.005", mt(montant));
    f.add("S21.G00.20.006", debut.jjmmaaaa());
    f.add("S21.G00.20.007", fin.jjmmaaaa());
    f.add("S21.G00.20.010", "05");
    Ok(f.lignes)
}

/// L'annee d'un mois `AAAA-MM`.
fn annee_du_mois(mois: &str) -> Resultat<i32> {
    mois.get(..4).and_then(|a| a.parse().ok()).ok_or_else(|| format!("mois illisible : {}", mois))
}

/// Le solde de la taxe d'apprentissage d'avril.
pub struct SoldeTa {
    pub annee: i32,
    pub assiette: Decimal,
    pub montant: Decimal,
    pub org: String,
}

/// CE QUI A ETE DECLARE D'UN MOIS : pour chaque engagement valide de `mois`, dans l'ordre de l'ecran, la paie que la
/// derniere DSN qui a declare ce mois a portee (`engagement::etats::paie_declaree`, regle 4 : ce qui est declare se lit
/// dans la version sur laquelle la DSN est partie), ou sa paie ecrite si aucune DSN ne l'a encore declaree. Un
/// engagement en saisie n'y est pas : rien ne part de lui. `nic` : les seuls salaries de cet etablissement.
pub fn paies_declarees_du_mois(ctx: &Contexte, mois: &str, nic: Option<&str>) -> Resultat<Vec<Paie>> {
    let Some(data) = engagement::stock::vue_du_mois(&ctx.racine, mois)? else { return Ok(Vec::new()) };
    let departs = engagement::departs::lire(&ctx.racine)?;
    let ctx_m = ctx.du_mois(mois)?;
    let mut out = Vec::new();
    for (i, e) in engagement::etats::du_mois(ctx, mois)?.iter().enumerate() {
        let Some(v) = e.valide() else { continue };
        let p = match engagement::etats::paie_declaree(&ctx_m, v.objet(), &departs, mois)? {
            Some(p) => p,
            None => v.paie(ctx, &data, i)?,
        };
        if let Some(n) = nic {
            if bareme::etablissement_du_salarie(&ctx_m.entreprise, &p.salarie)? != n {
                continue;
            }
        }
        out.push(p);
    }
    Ok(out)
}

/// LES ASSIETTES DES CTP `codes` que le bordereau Urssaf de `mois` porte pour l'etablissement `nic` sur les paies
/// `paies`, une par code, tirees par la fonction qui ecrit le bordereau de la DSN (`lignes_bordereau`) : la taxe
/// d'apprentissage (992, 993) que lisent le solde d'avril et la contribution supplementaire de mars (`csa`).
pub fn assiettes_des_ctp(ctx: &Contexte, paies: &[Paie], mois: &str, nic: &str, codes: &[&str]) -> Resultat<Vec<Decimal>> {
    if paies.is_empty() {
        return Ok(vec![zero(); codes.len()]);
    }
    let ctx = &ctx.du_mois(mois)?;
    let ent = &ctx.entreprise;
    let orgs = ctx.organismes_exiges()?;
    let bar = bar_du(ctx, ent, &orgs, nic, mois)?;
    let refs: Vec<&Paie> = paies.iter().flat_map(|p| p.elementaires()).collect();
    let lignes = lignes_bordereau(&refs, &orgs, ent, &bar, None, None, None, mois, &Seuils::du_mois(ctx, mois)?)?;
    Ok(codes.iter().map(|c| lignes.iter().filter(|l| l.code == *c).filter_map(|l| l.assiette).fold(zero(), add)).collect())
}

/// Les assiettes des CTP `codes` que les DSN de l'annee `annee` declarent pour l'etablissement `nic`, une par code.
pub fn assiettes_de_l_annee(ctx: &Contexte, annee: i32, nic: &str, codes: &[&str]) -> Resultat<Vec<Decimal>> {
    let mut total = vec![zero(); codes.len()];
    for m in 1..=12 {
        let mois = format!("{}-{:02}", annee, m);
        let paies = paies_declarees_du_mois(ctx, &mois, Some(nic))?;
        for (t, a) in total.iter_mut().zip(assiettes_des_ctp(ctx, &paies, &mois, nic, codes)?) {
            *t = add(*t, a);
        }
    }
    Ok(total)
}

fn solde_taxe_apprentissage(ctx: &Contexte, mois: &str, orgs: &Value, bar: &Value, nic: &str) -> Resultat<Option<SoldeTa>> {
    if &mois[5..7] != "04" {
        return Ok(None);
    }
    let sec = g(bar, "solde_taxe_apprentissage");
    if g(sec, "taux").is_null() || !liste(g(orgs, "ctp_urssaf")).iter().any(|c| txt(g(c, "code")) == "992") {
        return Ok(None);
    }
    let annee: i32 = mois[..4].parse::<i32>().map_err(|e| e.to_string())? - 1;
    // Pas de solde d'une taxe que la fiche dit non due pour l'annee (R12, `assujettissement::taxe_apprentissage_non_due`).
    if droit::apprentissage::taxe_apprentissage_non_due(&ctx.entreprise, annee)? {
        return Ok(None);
    }
    let assiette = assiettes_de_l_annee(ctx, annee, nic, &["992"])?[0];
    if assiette.is_zero() {
        return Ok(None);
    }
    let org = liste(g(bar, "cotisations_dsn"))
        .iter()
        .find(|c| txt(g(c, "code81")) == "075" && vrai(g(c, "org")))
        .map(|c| en_texte(&c["org"]))
        .unwrap_or_else(|| "78861779300013".into());
    let montant = div(mul(assiette, dec_avec_echelle(&sec["taux"])?), Decimal::ONE_HUNDRED)
        .round_dp_with_strategy(2, RoundingStrategy::MidpointNearestEven);
    Ok(Some(SoldeTa { annee, assiette, montant, org }))
}

// ------------------------------------------------------- les controles

/// CCH-13 et SIG-23, verifies ici plutot qu'au point de depot.
fn verifier_affiliations(texte: &str) -> Resultat<()> {
    let mut affiliations: BTreeSet<(String, String)> = BTreeSet::new();
    let mut bases: BTreeSet<(String, String)> = BTreeSet::new();
    let mut vus: HashSet<(String, String, String, String)> = HashSet::new();
    let mut doublons: BTreeSet<String> = BTreeSet::new();
    let mut contrat = String::new();
    let mut courant: Option<[String; 4]> = None;
    for ligne in texte.lines() {
        let (rubrique, valeur) = match ligne.split_once(',') {
            Some((r, v)) => (r, v.trim().trim_matches('\'')),
            None => (ligne, ""),
        };
        match rubrique {
            "S21.G00.30.002" => vus.clear(),
            "S21.G00.40.009" => contrat = valeur.to_string(),
            "S21.G00.70.012" => {
                affiliations.insert((contrat.clone(), valeur.to_string()));
            }
            "S21.G00.78.001" => {
                // Sans 78.006 (contrat unique, p. 298), la base se rattache au contrat de l'individu : le dernier 40.009.
                courant = if valeur == "31" { Some([String::new(), String::new(), String::new(), contrat.clone()]) } else { None };
            }
            _ => {
                if let Some(c) = courant.as_mut() {
                    match rubrique {
                        "S21.G00.78.002" => c[0] = valeur.to_string(),
                        "S21.G00.78.003" => c[1] = valeur.to_string(),
                        "S21.G00.78.005" => c[2] = valeur.to_string(),
                        "S21.G00.78.006" => c[3] = valeur.to_string(),
                        "S21.G00.81.004" => {
                            bases.insert((c[3].clone(), c[2].clone()));
                            let signe = if valeur.starts_with('-') { "-" } else { "+" };
                            let cle = (c[2].clone(), c[0].clone(), c[1].clone(), signe.to_string());
                            if vus.contains(&cle) {
                                doublons.insert(c[2].clone());
                            }
                            vus.insert(cle);
                            courant = None;
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    if !doublons.is_empty() {
        return Err(format!(
            "Affiliation(s) prevoyance declarees DEUX FOIS en base 31 pour un meme individu, meme periode et meme \
signe : {} (S21.G00.78.001/SIG-23).\n  Ce n'est admis que sur un contrat de mission (S21.G00.40.007 = 03). Les \
`cotisations_dsn` du bareme en declarent deux pour la meme affiliation.",
            doublons.into_iter().collect::<Vec<_>>().join(", ")
        ));
    }
    let mut manquantes: Vec<String> = affiliations
        .iter()
        .filter(|(c, a)| !bases.contains(&(c.clone(), a.clone())))
        .map(|(c, a)| format!("{} (contrat {})", a, c))
        .collect();
    manquantes.sort();
    if !manquantes.is_empty() {
        let mut dites: Vec<String> = bases.iter().map(|(_, a)| a.clone()).collect();
        dites.sort();
        return Err(format!(
            "Affiliation(s) prevoyance sans base assujettie de code 31 : {} (S21.G00.78.001/CCH-13).\n  Le bloc 70 \
declare {} affiliation(s) et le bareme ne donne de cotisation 31 que pour {}. La plate-forme refuse la declaration \
entiere.\n  Une base 31 par affiliation, meme a 0.00, et sous chacune une et une seule cotisation 81 de code 059 \
(CCH-15) : c'est aux `cotisations_dsn` du bareme de les dire.",
            manquantes.join(", "),
            if affiliations.is_empty() { "aucune".to_string() } else { affiliations.len().to_string() },
            if dites.is_empty() { "aucune".to_string() } else { dites.join(", ") }
        ));
    }
    Ok(())
}

// ----------------------------------------------------------- la generation

/// UN ORDRE DE PAIEMENT (bloc S21.G00.20) tel que la DSN l'ecrit : a qui (20.001), combien (20.005), pour quelle periode
/// (20.006, 20.007), pour quand (20.011), et le credit que la somme due laisse (`bordereau::credit`) : ce que la compagnie
/// a verse en trop et que l'ordre ne reprend pas (`reste`), ce qu'il reprend deja (`deduit`, l'Urssaf seulement).
#[derive(Clone, Debug)]
pub struct Ordre {
    pub organisme: String,
    pub reference: Option<String>,
    pub montant: Decimal,
    pub debut: Jour,
    pub fin: Jour,
    pub date_paiement: Option<Jour>,
    pub destinataire: Destinataire,
    pub reste: Decimal,
    pub deduit: Decimal,
}

/// UN BORDEREAU URSSAF (blocs S21.G00.22 et 23) tel que la DSN l'ecrit : la periode qu'il couvre (22.003, 22.004), son
/// total (22.005) et ses lignes 23. Le premier est celui du mois declare ; les suivants, ceux des mois que ses
/// corrections regularisent.
#[derive(Clone, Debug)]
pub struct BordereauEcrit {
    pub debut: Jour,
    pub fin: Jour,
    pub total: Decimal,
    pub lignes: Vec<LigneBordereau>,
}

/// Une DSN fabriquee : son texte, ses ordres de paiement dans l'ordre du fichier, et ses bordereaux Urssaf.
pub struct Fabriquee {
    pub texte: String,
    pub ordres: Vec<Ordre>,
    pub bordereaux: Vec<BordereauEcrit>,
}

/// Le texte de la DSN (`fabriquer`).
pub fn generer(ctx: &Contexte, paies: &[Paie], mois: &str, regs: &[Regularisation], arrets: &[Arret], o: &Options) -> Resultat<String> {
    Ok(fabriquer(ctx, paies, mois, regs, arrets, o)?.texte)
}

fn echeance_mois_suivant(an: i32, mo: u32, jour: u32) -> Jour {
    Jour::nouveau(an + (mo == 12) as i32, mo % 12 + 1, jour).unwrap_or_else(|| fin_mois(an + (mo == 12) as i32, mo % 12 + 1))
}

/// LE BAREME D'UN MOIS POUR UN ETABLISSEMENT : le bareme empile (`Contexte::bareme`) surcharge par ce que la fiche
/// entreprise dit de l'etablissement (`bareme::surcharger` : son taux AT/MP, son versement mobilite). Dans la DSN, la
/// seule facon de l'obtenir (regle 8 : chaque etablissement a son taux AT/MP) : le bordereau courant, le bordereau
/// regularise et le nominatif (`bar_du`), et l'apercu du montant Urssaf (`montant_urssaf`), passent par elle. (La pile
/// d'un salarie, `bareme::bareme_applique`, surcharge de meme par l'etablissement du salarie.)
fn bareme_du_nic(ctx: &Contexte, ent: &Value, nic: &str, mois: &str) -> Resultat<Value> {
    bareme::surcharger(ctx.bareme(&premier(mois)?)?, ent, nic)
}

/// LE BLOC 40 DECLARE du contrat de `p` (regle 4 : il se lit, il ne se recalcule pas), pour le bloc Changements contrat
/// de la mensuelle de `mois` : la DSN du mois le plus recent avant `mois` qui est partie pour son engagement, et la paie
/// de ce mois telle que la version sur laquelle elle est partie l'a ecrite (`engagement::etats::paie_declaree`) ; son
/// bloc 40 s'ecrit comme il s'est ecrit ce mois-la, de la fiche du salarie telle que ce calcul l'a lue. `etats` : les
/// engagements de `mois` ; `None` : rien de declare.
fn bloc_40_declare(ctx: &Contexte, p: &Paie, mois: &str, etats: &[engagement::etats::Etat], departs: &engagement::departs::Departs, orgs: &Value) -> Resultat<Option<crate::changements_contrat::Declare>> {
    let (cle, numero) = (cle_salarie(p), p.engagement.numero.clone().unwrap_or_default());
    let sienne = |e: &engagement::etats::Etat| {
        let b = Value::Object(e.objet().brut.clone());
        engagement::cle_de_l_engagement(ctx, &b).as_deref() == Some(cle.as_str()) && b.get("numero").map(en_texte).as_deref() == Some(numero.as_str())
    };
    let Some(v) = etats.iter().filter(|e| sienne(e)).find_map(engagement::etats::Etat::valide) else { return Ok(None) };
    let premier_mois = p.engagement.debut.mois_iso();
    let mut m = premier(mois)?.plus_jours(-1)?.mois_iso();
    while m >= premier_mois {
        // La compagnie telle qu'a la fin du mois declare (l'entreprise, ses etablissements) ; la fiche, celle que la paie declaree a lue.
        let ctx_m = Contexte::nouveau(&ctx.racine, Some(socle::date::dernier_du_mois(&m)?))?;
        if let Some(declaree) = engagement::etats::paie_declaree(&ctx_m, v.objet(), departs, &m)? {
            // Les autres contrats du salarie declares ce mois-la : le code emplois multiples (40.036) s'ecrit avec eux.
            let mut autres = Vec::new();
            for e in engagement::etats::tels_qu_ecrits(ctx, &m, departs)? {
                let Some(w) = e.valide() else { continue };
                if w.objet().fichier == v.objet().fichier && w.objet().rang == v.objet().rang {
                    continue;
                }
                if let Some(q) = engagement::etats::paie_declaree(&ctx_m, w.objet(), departs, &m)? {
                    autres.push(q);
                }
            }
            let nic = bareme::etablissement_du_salarie(&ctx_m.entreprise, &declaree.salarie)?;
            let bar = bar_du(&ctx_m, &ctx_m.entreprise, orgs, &nic, &m)?;
            let mut ent = ctx_m.entreprise.as_object().cloned().unwrap_or_default();
            ent.insert("licence_spectacle".into(), Value::String(ctx.licence(&premier(&m)?)?));
            let ent_e = fiche_de(&Value::Object(ent), &nic)?;
            let mut f = Fichier::en_norme(crate::norme::de_la_mensuelle(&m)?);
            let refs: Vec<&Paie> = autres.iter().collect();
            bloc_contrat(&mut f, &declaree, &ent_e, orgs, 1, &bar, true, &[], "01", emplois_multiples(&declaree, &refs))?;
            let bloc: Vec<(String, String)> = f
                .lignes
                .iter()
                .filter_map(|l| l.split_once(','))
                .filter(|(k, _)| k.starts_with("S21.G00.40."))
                .map(|(k, x)| (k.to_string(), x.trim_matches('\'').to_string()))
                .collect();
            return Ok(Some(crate::changements_contrat::Declare { mois: m, siret: txt(g(&ctx_m.entreprise, "siren")) + &nic, nic, bloc }));
        }
        m = premier(&m)?.plus_jours(-1)?.mois_iso();
    }
    Ok(None)
}

/// Le bareme d'un mois pour l'etablissement, avec ses contributions.
pub fn bar_du(ctx: &Contexte, ent: &Value, orgs: &Value, nic: &str, m: &str) -> Resultat<Value> {
    let b = bareme_du_nic(ctx, ent, nic, m)?;
    let c = contributions_dsn(ctx, ent, orgs, m, &b)?;
    if c.is_empty() {
        return Ok(b);
    }
    let mut o = b.as_object().cloned().unwrap_or_default();
    o.insert("_contributions_du_mois".into(), Value::Array(c));
    Ok(Value::Object(o))
}

fn ordre_salaries(paies: &[Paie]) -> Vec<String> {
    let mut vus: Vec<String> = Vec::new();
    for p in paies {
        let c = cle_salarie(p);
        if !vus.contains(&c) {
            vus.push(c);
        }
    }
    vus
}

/// La DSN mensuelle de `mois` (AAAA-MM) : toutes les paies du mois de cet etablissement.
///
/// Ce que la norme et les depots de test nous ont appris. Les parametres sont `o.etablissement`, `arrets`,
/// `regs`, `o.pas_par_salarie`, `o.envoi_test`, `o.remplace`, `o.ordre`.
///
/// `mois` au format AAAA-MM ; `paies` : toutes les paies du mois.
///
/// `etablissement` : le NIC de l'etablissement que CETTE declaration porte. Une compagnie qui en a
/// plusieurs depose une DSN par etablissement : le bloc 11, le lieu de
/// travail (40.019), le taux AT/MP et le versement mobilite sont les siens,
/// et `paies` ne contient que ses salaries -- un salarie d'un autre
/// etablissement est refuse. Le bloc 06 reste celui de l'entreprise (le
/// siege), et l'OETH ne part qu'avec le principal (D5212-8). Sans
/// `etablissement`, c'est le principal.
///
/// `arrets` : les arrets de travail (`signalement.Arret`) qui ont une periode
/// commune avec ce mois. La DSN mensuelle doit les REPORTER, meme quand un
/// signalement les a deja annonces : cahier technique 2026.1 §2.5.2, << il
/// est necessaire de reporter les informations d'un signalement dans la DSN
/// mensuelle consecutive [...] afin que l'evenement signale soit pris en
/// compte dans le traitement d'un evenement futur >>. Et bloc 60 : << en cas
/// de suspension supra-mensuelle, la suspension doit etre declaree dans
/// TOUTES les DSN mensuelles dont le mois principal declare presente une
/// periode commune avec la suspension >>. Un arret non reporte, c'est une
/// mensuelle qui contredit le signalement -- et c'est elle qui fait
/// l'historique de vie professionnelle du salarie.
///
/// `regularisations` : les corrections de mois DEPOSES dont l'echeance est
/// passee, a porter par CETTE declaration (operations_regularisation.
/// Regularisation : mois d'origine, paie AVANT telle que deposee, paie
/// APRES corrigee). Cahier technique 2026.1 §2.2.2.5 : « une fois passee
/// la date d'exigibilite, les corrections doivent etre portees par une DSN
/// mensuelle ulterieure », par les memes blocs DATES DE LA PERIODE
/// D'ORIGINE, et « une unique modalite de regularisation » pour toute la
/// declaration. La modalite retenue est l'ANNULATION-REMPLACEMENT : pour
/// chaque correction, les blocs de la paie d'avant en negatif, puis ceux
/// de la paie d'apres en positif -- deux ecritures que n'importe qui peut
/// relire, la ou une difference ne se verifie qu'en refaisant le calcul.
/// Voir `bloc_regularisation` et `lignes_regularisees` ci-dessous.
///
/// `pas_par_salarie` : {cle_salarie: (taux, type, identifiant)} -- LE TAUX
/// REELLEMENT DECLARE, impose de l'exterieur. C'est par la qu'un rejeu
/// des mois deposes repasse chaque mois
/// avec le taux qui etait en vigueur CE MOIS-LA : un triplet fourni s'ecrit
/// tel quel, sans abattement ni grille, et le fichier reproduit se compare
/// octet pour octet.
/// A defaut, le magasin config/pas.json fait foi -- mais seulement tant que
/// son taux est VALIDE (`pas.situation`). Le moteur ne fabrique jamais un
/// taux DGFiP : il vient du compte-rendu metier . Faute
/// d'un taux valide, ce n'est pas un refus, c'est la loi qui prend la main :
/// la grille de taux par defaut de l'article 204 H du CGI, calculee
/// versement par versement (`pas_du_versement`, ci-dessous).
///
/// `envoi_test` met S10.G00.00.005 a '01' : controles executes et bilan rendu,
/// donnees transmises aux organismes pour analyse mais NON CONSERVEES. Un
/// fichier code '02' est traite comme reel, quoi qu'on en dise par ailleurs.
///
/// ANNULE ET REMPLACE. `type_declaration='03'` et un `ordre` superieur a
/// celui de la declaration remplacee. C'est TOUT ce qu'il faut : la
/// plate-forme retrouve la declaration a remplacer par son mois, sa fraction
/// et son SIREN.
///
/// LA RUBRIQUE S20.G00.05.006 EST INTERDITE ICI. C'est ecrit noir sur blanc
/// dans le « tableau des usages » du cahier technique 2026.1, p. 362-363 :
/// par colonne, pour cette rubrique, on lit « I C C C C I C », et la premiere
/// colonne est « 01 - DSN mensuelle ». I = interdite. Elle n'est
/// conditionnelle que pour les signalements (arret de travail, reprise, fin
/// de contrat, amorcage) et la DPAE.
/// Le cahier technique la decrit comme « l'identifiant de la derniere
/// declaration, relative au meme EVENEMENT, validee par le point de depot ».
/// « Evenement » est le vocabulaire des SIGNALEMENTS (fin de contrat, arret
/// de travail), pas celui d'une DSN mensuelle. Nous avons lu la definition
/// trop vite, rempli la rubrique, et depose en test. Reponse de la
/// plate-forme, deux anomalies bloquantes :
///
/// ```text
/// CST-04      Presence de la rubrique interdite S20.G00.05.006.
/// B1-208-08   'annule et remplace' ou 'annule' sans normale prealable.
/// ```
///
/// D'ou `remplace` : on peut le passer, mais uniquement pour un signalement,
/// et le moteur n'en emet pas encore. Il est donc REFUSE pour une mensuelle.
/// Le jour ou un signalement sera ecrit, cette porte est deja la -- et le
/// commentaire dit pourquoi elle etait fermee.
///
/// LE NUMERO D'ORDRE EST LE SEUL RATTACHEMENT -- etabli le 10/09/2026 en
/// deposant en test, et lisible dans une troisieme anomalie :
///
/// ```text
/// B1-208-11   'annule et remplace' d'ordre 2026092 inferieur (ou egal)
///             au dernier ordre accepte 2026092.
/// ```
///
/// La plate-forme construit donc un rang « AAAAMM + ordre » a partir du MOIS
/// DE CONSTITUTION, et elle exige qu'un annule-et-remplace soit STRICTEMENT
/// superieur au dernier rang accepte pour cet etablissement. C'est ce rang,
/// et lui seul, qui designe la declaration remplacee : il n'y a rien d'autre
/// a fournir, et c'est pour ca que S20.G00.05.006 est interdite.
///
/// Consequence pratique : `ordre` ne se met pas a 2 par habitude. Il se
/// choisit au-dessus du dernier ordre ACCEPTE du mois civil en cours, qui se
/// lit sur le certificat de conformite du depot precedent (`moteur retours`).
///
/// IL REMPLACE, IL N'AJOUTE PAS -- Y COMPRIS LES ORDRES DE TELEPAIEMENT.
/// Question posee le 10/09/2026, tranchee le 11 par le cahier technique
/// 2026.1, p. 15 : « lorsque la derniere DSN mensuelle transmise pour une date
/// d'exigibilite donnee est de type "Annule et remplace integral", elle doit
/// porter les elements de cotisations destines aux organismes de recouvrement,
/// MEME SI ces elements ont deja ete transmis dans une DSN mensuelle relative
/// a la meme date d'exigibilite. » C'est la DERNIERE qui vaut. Exiger qu'elle
/// re-porte ce qui a deja ete transmis n'aurait aucun sens si l'on
/// additionnait. Le mecanisme le confirme : le telepaiement s'execute a la
/// DATE D'EXIGIBILITE, pas au depot -- l'ordre precedent n'a jamais ete
/// execute, il est remplace en attente.
///
/// ET SON ECHEANCE N'EST PAS CELLE D'UNE DSN NORMALE -- cahier technique,
/// p. 13. Une DSN « normale » se depose jusqu'au jour de l'echeance (5 ou 15)
/// a midi ; un annule-et-remplace, lui, « au plus tard la veille a minuit ».
/// Au-dela, la rectification se porte dans les paies des mois suivants
/// (`paie corriger`). Le moteur connait l'echeance de l'etablissement
/// (`Contexte::echeance_dsn` : le 5 a cinquante salaries, le 15 sinon --
/// critere infere, la norme ne le donne pas, fiche N7 § 1.4.2) et sa veille
/// (`routes_mois::Moteur::limite`) : passe cette veille, il refuse de produire
/// l'annule-et-remplace (`depart::forme_a_produire`) comme de le
/// deposer (`depot_acte`), « AnnuleRemplaceHorsDelai ». Ce n'est pas ici, dans
/// `generer`, qui ecrit ce qu'on lui donne, mais avant lui.
///
/// ET EN TEST, UN ANNULE-ET-REMPLACE SE PREPARE PAR UNE REPETITION. Le meme
/// jour, la premiere tentative avait rendu :
///
/// ```text
/// B1-208-08   'annule et remplace' ou 'annule' sans normale prealable.
/// ```
///
/// alors que la declaration d'aout existait bel et bien, deposee le 03/09 et
/// certifiee conforme. L'anomalie a disparu des qu'une declaration NORMALE a
/// ete deposee EN TEST juste avant. Le mode test a sa propre base : il ne
/// voit pas les declarations reelles. On teste donc un annule-et-remplace
/// comme on le deposera : la normale en test d'abord, puis l'annule-et-
/// remplace en test (la repetition). Il passe alors les memes controles
/// qu'en reel, rattachement compris : le test vaut le reel. Ce que le test ne controle pas, c'est
/// le montant.
pub fn fabriquer(ctx: &Contexte, paies: &[Paie], mois: &str, regs: &[Regularisation], arrets: &[Arret], o: &Options) -> Resultat<Fabriquee> {
    let ctx = &ctx.du_mois(mois)?;
    let ent0 = ctx.entreprise.clone();
    let principal_nic = bareme::nic_principal(&ent0);
    let nic = match o.etablissement.as_deref().filter(|n| !n.is_empty()) {
        Some(n) => n.to_string(),
        None => principal_nic.clone(),
    };
    let principal = nic == principal_nic;
    if !principal && bareme::entree(&ent0, &nic).is_none() {
        return Err(format!("Etablissement {} inconnu de la fiche entreprise (connus : {}).", nic, nics(&ent0).join(", ")));
    }
    let mut egares = BTreeSet::new();
    for s in paies.iter().map(|p| &p.salarie).chain(regs.iter().map(|r| &r.apres.salarie)) {
        if bareme::etablissement_du_salarie(&ent0, s)? != nic {
            egares.insert(txt(g(s, "_cle")));
        }
    }
    if !egares.is_empty() {
        let n = egares.len();
        return Err(format!(
            "DSN de l'etablissement {} : {} n'y appartien{} pas (fiche salarie, `etablissement`). Une DSN par \
etablissement, chacun avec ses salaries. Rien n'est ecrit.",
            nic,
            egares.into_iter().collect::<Vec<_>>().join(", "),
            if n > 1 { "nent" } else { "t" }
        ));
    }
    // La correction d'un mois decoupe en periodes elementaires : le declare relu (`versement_declare`) et le
    // corrige se comparent base par base, et un mois decoupe en a une par periode. Un chantier a part.
    if let Some(r) = regs.iter().find(|r| !r.apres.periodes.is_empty() || !r.avant.periodes.is_empty()) {
        return Err(format!(
            "Correction de {} (contrat {}) : la paie de ce mois se decoupe en periodes elementaires (un changement de \
statut, de categorie ou de duree du travail en cours de mois). La regularisation d'un tel mois, base par base et periode \
par periode, n'est pas faite par le logiciel. Rien n'est ecrit.",
            r.mois,
            r.apres.engagement.numero.clone().unwrap_or_else(|| "?".into())
        ));
    }
    // Les etablissements, celui de cette DSN d'abord : un contrat declare ailleurs a change d'etablissement (bloc 41).
    let nics_courant_d_abord: Vec<String> = std::iter::once(nic.clone()).chain(nics(&ent0).into_iter().filter(|n| *n != nic)).collect();
    // Ce qui est parti, et les engagements du mois : le bloc 40 declare de chaque contrat se lit dans sa version (`bloc_40_declare`).
    let departs = engagement::departs::lire(&ctx.racine)?;
    let etats_du_mois = engagement::etats::tels_qu_ecrits(ctx, mois, &departs)?;
    let orgs = ctx.organismes_exiges()?;
    let an: i32 = mois[..4].parse().map_err(|_| format!("mois illisible : {}", mois))?;
    let mo: u32 = mois[5..7].parse().map_err(|_| format!("mois illisible : {}", mois))?;
    let debut_mois = premier(mois)?;
    // La version de la norme suit le mois principal declare (`norme-dsn-base.json`, une inference : `_inference`) ; un
    // mois que la donnee ne couvre pas refuse ici, avant tout calcul. Un annule-et-remplace prend celle de son mois.
    let norme = crate::norme::de_la_mensuelle(&debut_mois.mois_iso())?;
    // En P27V01, la fiche entreprise est en NAF 2025 (`naf::verifier_pour`), avant tout calcul.
    crate::naf::verifier_pour(&norme, &ctx.entreprise)?;
    let fin_m = fin_mois(an, mo);
    let mut bars: HashMap<String, Value> = HashMap::new();
    let mut bar_de = |m: &str| -> Resultat<Value> {
        if let Some(b) = bars.get(m) {
            return Ok(b.clone());
        }
        let b = bar_du(ctx, &ent0, &orgs, &nic, m)?;
        bars.insert(m.to_string(), b.clone());
        Ok(b)
    };
    let bar = bar_de(mois)?;
    let contribs = g(&bar, "_contributions_du_mois").clone();
    let mut ent = ent0.as_object().cloned().unwrap_or_default();
    ent.insert("licence_spectacle".into(), Value::String(ctx.licence(&debut_mois)?));
    let ent = Value::Object(ent);
    let ent_e = fiche_de(&ent, &nic)?;

    let mut arrets_du_mois = Vec::new();
    for a in arrets {
        if a.couvre(&debut_mois, &fin_m)? {
            arrets_du_mois.push(a);
        }
    }
    let orphelins: Vec<&&Arret> = arrets_du_mois.iter().filter(|a| !paies.iter().any(|p| a.du_contrat(p))).collect();
    if !orphelins.is_empty() {
        let liste_o: Vec<String> = orphelins
            .iter()
            .map(|a| {
                let fin = if vrai(g(&a.v, "reprise")) { a.s("reprise") } else { a.s("fin_previsionnelle") };
                format!("{} contrat {} ({} -> {})", a.s("salarie"), a.s("contrat_numero"), a.s("dernier_jour_travaille"), fin)
            })
            .collect();
        return Err(format!(
            "Arret de travail a reporter dans la DSN de {}, mais son contrat n'est pas declare ce mois-ci : {}. Le \
bloc « Arret de travail - S21.G00.60 » se declare SOUS le contrat suspendu. Reprendre le contrat dans la saisie du \
mois (il y figure sans cachet), ou corriger les dates de l'arret. Rien n'est ecrit.",
            mois,
            liste_o.join(", ")
        ));
    }
    let manquants: Vec<String> = paies
        .iter()
        .filter(|p| p.remuneration.iter().any(|l| l.arret_de_travail) && !arrets_du_mois.iter().any(|a| a.du_contrat(p)))
        .map(|p| format!("{} contrat {}", cle_salarie(p), p.engagement.numero.clone().unwrap_or_else(|| "None".into())))
        .collect();
    if !manquants.is_empty() {
        return Err(format!(
            "Absence pour arret de travail dans la paie de {}, mais aucun arret saisi en face : {}. La ligne retire \
le salaire des heures non travaillees ; c'est le bloc « Arret de travail - S21.G00.60 » qui ouvre les indemnites \
journalieres, et il se construit depuis l'arret, pas depuis la ligne. Saisir l'arret (`paie arret \
saisir`), ou retirer la ligne. Rien n'est ecrit.",
            mois,
            manquants.join(", ")
        ));
    }
    // LA SUSPENSION D'UN CONTRAT QUE CE MOIS NE DECLARE PAS (15/10/2026) : le bloc 65 se declare SOUS le contrat suspendu,
    // dans chaque DSN dont le mois croise la suspension (bloc S21.G00.65, cahier technique DSN 2026.1, p. 281,
    // `atelier/reference/norme-dsn/texte/bloc-S21.G00.65.txt`) ; il se tire du magasin des imprevus pour le contrat de
    // chaque paie (`imprevus::suspensions_du_mois`). Une suspension de cet etablissement dont le contrat manque ne serait
    // declaree nulle part : refus, comme pour l'arret.
    let mut suspendus_absents = Vec::new();
    for imp in engagement::imprevus::du_mois(&ctx.racine, mois, Some(engagement::imprevus::SUSPENSION))? {
        let fiche = ctx.salarie(&txt(g(&imp, "salarie")))?;
        if bareme::etablissement_du_salarie(&ent0, &fiche)? != nic {
            continue;
        }
        let mut declare = false;
        for p in paies {
            if engagement::imprevus::du_contrat(&imp, &cle_salarie(p), p.engagement.numero.as_deref(), &p.engagement.debut)? {
                declare = true;
            }
        }
        if !declare {
            suspendus_absents.push(format!(
                "{} contrat {} (suspension {} du {} au {})",
                txt(g(&imp, "salarie")),
                txt(g(&imp, "contrat_numero")),
                txt(g(&imp, "motif")),
                txt(g(&imp, "debut")),
                txt(g(&imp, "fin"))
            ));
        }
    }
    if !suspendus_absents.is_empty() {
        return Err(format!(
            "Suspension a declarer dans la DSN de {}, mais son contrat n'est pas declare ce mois-ci : {}. Le bloc « Autre \
suspension de l'execution du contrat - S21.G00.65 » se declare SOUS le contrat suspendu, dans chaque mois qu'elle \
traverse. Reprendre le contrat dans la saisie du mois, ou corriger les dates de la suspension. Rien n'est ecrit.",
            mois,
            suspendus_absents.join(", ")
        ));
    }
    let sans_individu = paies.is_empty() && regs.is_empty();
    let constitution = match o.date_constitution {
        Some(d) => d,
        // La date de constitution (S20.G00.05.007) est le JOUR OU LE FICHIER SE FABRIQUE, comme celle des
        // signalements et de la FCTU : une DSN produite le 07/10 est « constituee le 07/10 ».
        None => aujourdhui()?,
    };
    let mut type_declaration = o.type_declaration.clone();
    if sans_individu {
        type_declaration = match type_declaration.as_str() {
            "01" => "02".into(),
            "03" => "05".into(),
            t => t.into(),
        };
    }
    if o.remplace.as_deref().map_or(false, |r| !r.is_empty()) {
        return Err("S20.G00.05.006 est interdite sur une DSN mensuelle : la plate-forme a repondu CST-04 le \
10/09/2026. Un annule et remplace se code par type_declaration='03' et un numero d'ordre superieur, sans \
identifiant de declaration remplacee."
            .into());
    }
    if (type_declaration == "03" || type_declaration == "05") && o.ordre < 2 {
        return Err(format!(
            "Un annule et remplace doit porter un numero d'ordre STRICTEMENT superieur au dernier ordre accepte du \
mois civil en cours (recu ordre={}). La plate-forme rend B1-208-11 sinon ; le dernier ordre accepte se lit sur le \
certificat de conformite du depot precedent.",
            o.ordre
        ));
    }

    let mut pas_par_salarie = o.pas_par_salarie.clone();
    for cle in paies.iter().map(cle_salarie).chain(regs.iter().map(|r| cle_salarie(&r.apres))) {
        if !pas_par_salarie.contains_key(&cle) {
            let t = pas::taux_dgfip(ctx, &cle, None)?;
            pas_par_salarie.insert(cle, t);
        }
    }
    let mut f = Fichier::en_norme(norme);
    let e = |k: &str| -> Resultat<&Value> { req(&ent, k) };

    f.add("S10.G00.00.001", LOGICIEL);
    f.add("S10.G00.00.002", LOGICIEL);
    f.add("S10.G00.00.003", VERSION);
    f.add("S10.G00.00.005", if o.envoi_test { "01" } else { "02" });
    f.ecrire_la_version()?;
    f.add("S10.G00.00.007", "01");
    f.add("S10.G00.00.008", if sans_individu { "02" } else { "01" });
    f.add_v("S10.G00.01.001", e("siren")?)?;
    f.add_v("S10.G00.01.002", e("nic")?)?;
    f.add_v("S10.G00.01.003", e("raison_sociale")?)?;
    f.add_v("S10.G00.01.004", e("adresse")?)?;
    f.add_v("S10.G00.01.005", e("code_postal")?)?;
    f.add_v("S10.G00.01.006", e("commune")?)?;
    contact_emetteur(&mut f, &ent)?;
    let decl = e("declarant_dsn")?;

    f.add("S20.G00.05.001", "01");
    f.add("S20.G00.05.002", &type_declaration);
    f.add("S20.G00.05.003", &o.fraction);
    f.add("S20.G00.05.004", o.ordre);
    f.add("S20.G00.05.005", debut_mois.jjmmaaaa());
    f.add("S20.G00.05.007", constitution.jjmmaaaa());
    f.add("S20.G00.05.008", "01");
    f.add("S20.G00.05.010", "01");
    f.add_v("S20.G00.07.001", req(decl, "nom_prenom")?)?;
    f.add_v("S20.G00.07.002", req(decl, "tel")?)?;
    f.add_v("S20.G00.07.003", req(decl, "email")?)?;
    f.add_v("S20.G00.07.004", &crate::dsn::type_du_contact(decl))?;
    contact_sante_travail(&mut f, &ent_e)?;
    if sans_individu {
        for code in liste(req(req(&orgs, "sans_individu")?, "absence_de_rattachement")?) {
            f.add_v("S20.G00.08.001", code)?;
        }
    }

    f.add_v("S21.G00.06.001", e("siren")?)?;
    f.add_v("S21.G00.06.002", e("nic")?)?;
    // Le code APEN (06.003) et le code APET (11.002) : obligatoires en P26V01, retires de la norme P27V01 (cahier
    // 2027.1.1, p. 142 et 145 ; usages p. 371 ). Le tableau des usages de la version du fichier en decide.
    if usage_admet(&f, "S21.G00.06.003", "01")? {
        f.add_v("S21.G00.06.003", e("naf")?)?;
    }
    f.add_v("S21.G00.06.004", e("adresse")?)?;
    f.add_v("S21.G00.06.005", e("code_postal")?)?;
    f.add_v("S21.G00.06.006", e("commune")?)?;
    let doeth = if principal { crate::oeth::declaration(ctx, mois)? } else { None };
    // La contribution supplementaire a l'apprentissage, sur l'etablissement principal seul (fiches 2588 et 2963 :
    // « sur un seul établissement »).
    let csa = if principal { crate::csa::declaration(ctx, mois)? } else { None };
    if let Some(d) = &doeth {
        if let Some(a) = d.accord.as_deref().filter(|a| !a.is_empty()) {
            f.add("S21.G00.13.001", a);
            f.add("S21.G00.13.004", d.millesime);
        }
        for (type_boeth, nombre) in &d.externes {
            f.add("S21.G00.13.002", type_boeth);
            f.add("S21.G00.13.003", mt(*nombre));
            f.add("S21.G00.13.004", d.millesime);
        }
    }
    for k in [("S21.G00.11.001", "nic"), ("S21.G00.11.002", "naf"), ("S21.G00.11.003", "adresse"), ("S21.G00.11.004", "code_postal"), ("S21.G00.11.005", "commune"), ("S21.G00.11.022", "code_convention_dsn")] {
        if k.0 == "S21.G00.11.002" && !usage_admet(&f, k.0, "01")? {
            continue;
        }
        f.add_v(k.0, req(&ent_e, k.1)?)?;
    }
    if let Some(spst) = spst_ecrit(&f, &ent_e)? {
        f.add("S21.G00.11.025", spst);
    }
    let (premier_du_mois, dernier_du_mois) = bornes_du_mois(mois)?;
    let mut couvertes: HashSet<String> = HashSet::new();
    // Les adhesions qu'au moins une affiliation du fichier reference : celles des salaries affilies ce mois-la
    // (`rattachements`), pas toutes celles de leur nature (CCH-13 et CCH-14 de 15.004, p. 151).
    for p in paies {
        let refs = affiliations(&orgs, &*p.engagement.nat()?, &contribs, (p.engagement.debut_periode()?, p.engagement.fin_periode()?))?;
        couvertes.extend(rattachements(p, &orgs, droit::bareme::entreprise_du(&bar, &ent_e), refs)?.into_iter().map(|a| a.reference));
    }
    for r in regs {
        let b = bar_de(&r.mois)?;
        let refs = affiliations(&orgs, &*r.apres.engagement.nat()?, g(&b, "_contributions_du_mois"), (r.apres.engagement.debut_periode()?, r.apres.engagement.fin_periode()?))?;
        couvertes.extend(rattachements(&r.apres, &orgs, droit::bareme::entreprise_du(&b, &ent_e), refs)?.into_iter().map(|a| a.reference));
    }
    for a in liste(req(&orgs, "adhesions_prevoyance")?) {
        let reference = en_texte(req(a, "reference")?);
        if !couvertes.contains(&reference) && !adhesion_en_vigueur(&orgs, &reference, &premier_du_mois, &dernier_du_mois)? {
            continue;
        }
        f.add_v("S21.G00.15.001", req(a, "libelle")?)?;
        f.add_v("S21.G00.15.002", req(a, "delegataire")?)?;
        f.add("S21.G00.15.004", if sans_individu || !couvertes.contains(&reference) { "02" } else { "01" });
        f.add_v("S21.G00.15.005", &a["reference"])?;
    }

    let solde_ta = solde_taxe_apprentissage(ctx, mois, &orgs, &bar, &nic)?;
    if let Some(s) = &solde_ta {
        f.add("S21.G00.82.001", mt(s.montant));
        f.add("S21.G00.82.002", "076");
        f.add("S21.G00.82.003", format!("0101{}", s.annee));
        f.add("S21.G00.82.004", format!("3112{}", s.annee));
        f.add("S21.G00.82.005", &s.org);
    }
    let versements = liste(req(&orgs, "versements")?);
    if let Some(d) = &doeth {
        let org_urssaf = versements
            .iter()
            .find(|v| txt(g(v, "libelle")) == "URSSAF")
            .map(|v| en_texte(g(v, "siret")))
            .ok_or("StopIteration")?;
        for (code, montant) in d.blocs_82() {
            f.add("S21.G00.82.001", mt(montant));
            f.add("S21.G00.82.002", code);
            f.add("S21.G00.82.003", format!("0101{}", d.millesime));
            f.add("S21.G00.82.004", format!("3112{}", d.millesime));
            f.add("S21.G00.82.005", &org_urssaf);
        }
    }
    if let Some(c) = &csa {
        let org_urssaf = versements
            .iter()
            .find(|v| txt(g(v, "libelle")) == "URSSAF")
            .map(|v| en_texte(g(v, "siret")))
            .ok_or("StopIteration")?;
        let (code, montant) = c.bloc_82();
        f.add("S21.G00.82.001", mt(montant));
        f.add("S21.G00.82.002", code);
        f.add("S21.G00.82.003", format!("0101{}", c.millesime));
        f.add("S21.G00.82.004", format!("3112{}", c.millesime));
        f.add("S21.G00.82.005", &org_urssaf);
    }

    let mut totaux = Vec::new();
    // Les totaux et le bordereau cumulent ce que le nominatif declare : les periodes elementaires d'un mois decoupe.
    let refs: Vec<&Paie> = paies.iter().flat_map(|p| p.elementaires()).collect();
    cumuler_totaux(&mut totaux, &refs, &bar, 1)?;
    let mut origines: Vec<String> = Vec::new();
    let mut par_origine: HashMap<String, Vec<(String, Decimal)>> = HashMap::new();
    for r in regs {
        let bar_o = bar_de(&r.mois)?;
        if !origines.contains(&r.mois) {
            origines.push(r.mois.clone());
        }
        let t_o = par_origine.entry(r.mois.clone()).or_default();
        cumuler_totaux(t_o, &[&r.apres], &bar_o, 1)?;
        cumuler_le_declare(t_o, r, &bar_o, &orgs, -1)?;
    }
    let mut totaux_avec_regs = totaux.clone();
    for m in &origines {
        for (k, v) in &par_origine[m] {
            ajouter(&mut totaux_avec_regs, k, *v);
        }
    }

    let echeance_dsn = ctx.echeance_dsn()?;
    let echeances: HashMap<&str, Jour> = [
        ("13000548100010", echeance_mois_suivant(an, mo, echeance_dsn)),
        // AUDIENS SANTE PREVOYANCE (ticket #223) : le 25 du mois qui suit le mois declare, date qui « doit etre
        // renseignee dans le bloc paiement a destination d'Audiens Sante Prevoyance (P0983) » (Audiens, dossier DSN,
        // `atelier/reference/sources/audiens-dsn/` ; `jour_de_paiement` de `donnees/organismes-base.json`). Elle etait
        // la fin du mois suivant le trimestre, pour un versement pourtant mensuel.
        (
            "P0983",
            echeance_mois_suivant(
                an,
                mo,
                crate::ordres_paiement::jour_de_paiement("P0983")?.ok_or("donnees/organismes-base.json : P0983 n'a pas de jour_de_paiement")?,
            ),
        ),
    ]
    .into_iter()
    .collect();
    let brut_mois = paies.iter().fold(zero(), |s, p| add(s, p.brut));
    let urssaf = versements.iter().find(|v| txt(g(v, "libelle")) == "URSSAF").ok_or("StopIteration")?;
    let lignes_23 = lignes_bordereau(&refs, &orgs, &ent0, &bar, solde_ta.as_ref(), doeth.as_ref(), csa.as_ref(), mois, &Seuils::du_mois(ctx, mois)?)?;
    let mut lignes_o = Vec::new();
    let mut montants_o = Vec::new();
    for m in &origines {
        // Le bareme du mois d'origine pour CET etablissement, comme celui du bordereau courant (`bar`) : point 25.
        let bar_o = bar_de(m)?;
        let l = lignes_regularisees(&orgs, &ent0, regs, m, &bar_o, &Seuils::du_mois(ctx, m)?)?;
        montants_o.push((m.clone(), bordereau::montant(&l, &premier(m)?)?));
        lignes_o.push((m.clone(), l));
    }
    // LE TROP-VERSE A L'URSSAF (les bordereaux negatifs des mois corriges) se reprend sur le versement du mois, dans la
    // limite de son bordereau ; le reste demeure a l'Urssaf (`bordereau::reprise_urssaf`, guide Urssaf § 6.4).
    let credit = -montants_o.iter().filter(|(_, m)| *m < Decimal::ZERO).fold(zero(), |s, (_, m)| add(s, *m));
    let mt_urssaf = bordereau::montant(&lignes_23, &debut_mois)?;
    let (verse_urssaf, deduit_urssaf) = bordereau::reprise_urssaf(mt_urssaf, credit);
    let banque = banque_du_prelevement(&ent)?;
    // Le versement DGFiP s'ecrit ici, en tete des blocs 20, quand les individus ont dit ce qu'ils retiennent.
    let place_dgfip = f.lignes.len();
    // Les ordres de paiement, dans l'ordre du fichier ; ceux des organismes complementaires a leur place, ecrits apres
    // les individus.
    let mut ordres: Vec<Option<Ordre>> = Vec::new();
    let mut places_ordres_oc: Vec<(String, usize)> = Vec::new();
    // Un organisme complementaire : celui d'une adhesion prevoyance (S21.G00.15.002 = S21.G00.20.001).
    let organismes_complementaires: HashSet<String> =
        liste(g(&orgs, "adhesions_prevoyance")).iter().map(|a| en_texte(g(a, "delegataire"))).collect();
    let mut places_oc: Vec<(usize, &Value)> = Vec::new();
    for v in versements {
        let libelle = txt(g(v, "libelle"));
        if sans_individu && libelle != "URSSAF" {
            continue;
        }
        let siret = en_texte(req(v, "siret")?);
        if organismes_complementaires.contains(&siret) {
            // Le versement d'un organisme complementaire s'ecrit ici, ses composants lus dans les individus ecrits.
            places_oc.push((f.lignes.len(), v));
            places_ordres_oc.push((siret.clone(), ordres.len()));
            ordres.push(None);
            continue;
        }
        let (montant, destinataire, reste, deduit) = if libelle == "URSSAF" {
            (verse_urssaf.montant, Destinataire::Urssaf, sub(credit.max(zero()), deduit_urssaf), deduit_urssaf)
        } else {
            let m = montant_versement(v, &totaux_avec_regs, brut_mois)?;
            if m.is_zero() {
                continue;
            }
            // Une correction a la baisse peut faire la somme negative : le versement est alors a zero, le credit reste
            // declare dans les cotisations individuelles (`bordereau::a_verser`).
            let a = bordereau::a_verser(m);
            (a.montant, Destinataire::Autre, a.credit, zero())
        };
        ordres.push(Some(Ordre {
            organisme: siret.clone(),
            reference: reference_versement(v, &ent)?,
            montant,
            debut: debut_mois,
            fin: fin_m,
            date_paiement: echeances.get(siret.as_str()).copied(),
            destinataire,
            reste,
            deduit,
        }));
        f.add("S21.G00.20.001", &siret);
        if let Some(r) = reference_versement(v, &ent)? {
            f.add("S21.G00.20.002", r);
        }
        f.add_v("S21.G00.20.003", req(banque, "bic")?)?;
        f.add_v("S21.G00.20.004", req(banque, "iban")?)?;
        f.add("S21.G00.20.005", mt(montant));
        f.add("S21.G00.20.006", debut_mois.jjmmaaaa());
        f.add("S21.G00.20.007", fin_m.jjmmaaaa());
        f.add("S21.G00.20.010", "05");
        if let Some(j) = echeances.get(siret.as_str()) {
            f.add("S21.G00.20.011", j.jjmmaaaa());
        }
    }
    let siret_urssaf = en_texte(req(urssaf, "siret")?);
    for (mois_o, mt_o) in &montants_o {
        if *mt_o < Decimal::ZERO {
            continue;
        }
        let d_o = premier(mois_o)?;
        ordres.push(Some(Ordre {
            organisme: siret_urssaf.clone(),
            reference: vrai(g(urssaf, "reference")).then(|| en_texte(&urssaf["reference"])),
            montant: *mt_o,
            debut: d_o,
            fin: fin_mois(d_o.an, d_o.mois),
            date_paiement: echeances.get(siret_urssaf.as_str()).copied(),
            destinataire: Destinataire::Urssaf,
            reste: zero(),
            deduit: zero(),
        }));
        f.add("S21.G00.20.001", &siret_urssaf);
        if vrai(g(urssaf, "reference")) {
            f.add("S21.G00.20.002", en_texte(&urssaf["reference"]));
        }
        f.add_v("S21.G00.20.003", req(banque, "bic")?)?;
        f.add_v("S21.G00.20.004", req(banque, "iban")?)?;
        f.add("S21.G00.20.005", mt(*mt_o));
        f.add("S21.G00.20.006", d_o.jjmmaaaa());
        f.add("S21.G00.20.007", fin_mois(d_o.an, d_o.mois).jjmmaaaa());
        f.add("S21.G00.20.010", "05");
        if let Some(j) = echeances.get(siret_urssaf.as_str()) {
            f.add("S21.G00.20.011", j.jjmmaaaa());
        }
    }

    f.add("S21.G00.22.001", &siret_urssaf);
    f.add("S21.G00.22.003", debut_mois.jjmmaaaa());
    f.add("S21.G00.22.004", fin_m.jjmmaaaa());
    f.add("S21.G00.22.005", mt(mt_urssaf));
    ecrire_lignes_23(&mut f, &lignes_23);
    let mut bordereaux = vec![BordereauEcrit { debut: debut_mois, fin: fin_m, total: mt_urssaf, lignes: lignes_23.clone() }];
    for ((mois_o, lignes), (_, montant)) in lignes_o.iter().zip(&montants_o) {
        let d_o = premier(mois_o)?;
        f.add("S21.G00.22.001", &siret_urssaf);
        f.add("S21.G00.22.003", d_o.jjmmaaaa());
        f.add("S21.G00.22.004", fin_mois(d_o.an, d_o.mois).jjmmaaaa());
        f.add("S21.G00.22.005", mt(*montant));
        ecrire_lignes_23(&mut f, lignes);
        bordereaux.push(BordereauEcrit { debut: d_o, fin: fin_mois(d_o.an, d_o.mois), total: *montant, lignes: lignes.clone() });
    }
    // L'assujettissement fiscal : une fois l'an, dans la DSN de decembre, apres les bordereaux et avant les
    // individus (arborescence de la norme, p. 118, `chapitre-8-arborescences.txt` : 20, 55, 22, 23, 44, 30) ;
    // `assujettissement.rs` dit d'ou vient chaque code.
    for b in crate::assujettissement::blocs(ctx, &ent0, &ent_e, &orgs, &bar, &nic, mois, paies)? {
        f.add("S21.G00.44.001", b.code);
        if let Some(m) = b.montant {
            f.add("S21.G00.44.002", mt(m));
        }
        f.add("S21.G00.44.003", an);
        if let Some(m) = &b.motif {
            f.add("S21.G00.44.004", m);
        }
    }

    let mut ordre = ordre_salaries(paies);
    for r in regs {
        let c = cle_salarie(&r.apres);
        if !ordre.contains(&c) {
            ordre.push(c);
        }
    }
    let date_ref = premier(mois)?;
    // LES CONTRATS DE CHAQUE PERIODE DE PAIE, pour les emplois multiples (`40.036`) : ceux de la declaration et ceux de
    // la saisie du mois ; pour un contrat regularise, ceux de son mois d'origine.
    let du_mois = contrats_du_mois(ctx, mois)?;
    let periode: Vec<&Paie> = paies.iter().chain(du_mois.iter()).collect();
    let mut des_mois_regularises: HashMap<String, Vec<Paie>> = HashMap::new();
    for r in regs {
        if !des_mois_regularises.contains_key(&r.mois) {
            des_mois_regularises.insert(r.mois.clone(), contrats_du_mois(ctx, &r.mois)?);
        }
    }
    let mut declarations = crate::changements_individu::Declarations::avant(ctx, mois)?;
    for cle in &ordre {
        let lot: Vec<&Paie> = paies.iter().filter(|p| &cle_salarie(p) == cle).collect();
        let corrections: Vec<&Regularisation> = regs.iter().filter(|r| &cle_salarie(&r.apres) == cle).collect();
        let s = match lot.first() {
            Some(p) => &p.salarie,
            None => &corrections[0].apres.salarie,
        };
        // LES IDENTIFIANTS DU MOIS DECLARE : NIR, nom, prenoms et date de naissance tels qu'au dernier jour du
        // mois, pour qu'un changement date apres lui n'entre pas dans sa DSN (`changements_individu::identite_du_mois`).
        let brute = ctx.salarie_brut(&txt(g(s, "_cle"))).cloned().unwrap_or(Value::Null);
        let ident = crate::changements_individu::identite_du_mois(s, &brute, &fin_m)?;
        crate::ntt::poser_nir(&mut f, &ident)?;
        for (rub, k) in [
            ("S21.G00.30.002", "nom"),
            ("S21.G00.30.004", "prenom"),
        ] {
            f.add_v(rub, req(&ident, k)?)?;
        }
        f.add("S21.G00.30.006", jj(req(&ident, "date_naissance")?)?);
        for (rub, k) in [
            ("S21.G00.30.007", "lieu_naissance"),
            ("S21.G00.30.008", "adresse"),
            ("S21.G00.30.009", "code_postal"),
            ("S21.G00.30.010", "commune"),
        ] {
            f.add_v(rub, req(s, k)?)?;
        }
        // La codification UE se lit dans la fiche par `codification_ue` : quatre valeurs, et un ancien 03 se relit.
        f.add("S21.G00.30.013", socle::codification_ue::codification_ue(s)?);
        for (rub, k) in [
            ("S21.G00.30.014", "departement_naissance"),
            ("S21.G00.30.015", "pays_naissance"),
            ("S21.G00.30.018", "email"),
            ("S21.G00.30.019", "code_salarie"),
        ] {
            // L'adresse mel est conditionnelle (usage « C » en DSN mensuelle, et « conditionne a l'accord explicite » de
            // l'individu : `atelier/reference/norme-dsn/texte/tableaux-des-usages.txt`, `texte/bloc-S21.G00.30.txt`) :
            // une fiche sans courriel n'en declare pas.
            if k == "email" {
                if let Some(v) = s.get(k).filter(|v| vrai(v)) {
                    f.add_v(rub, v)?;
                }
                continue;
            }
            // Le pays de naissance se lit dans la liste livree (`pays.rs`) : un code inconnu refuse.
            let v = if k == "pays_naissance" { socle::pays::de_naissance(s)? } else { req(s, k)?.clone() };
            f.add_v(rub, &v)?;
        }
        // L'identite DECLAREE, relue dans la derniere paie declaree qui le porte. Declare sans NIR, sous un NTT :
        // la premiere DSN qui porte son NIR porte aussi ce NTT, quel que soit le mois ou le NIR est arrive (fiche N4).
        let declaree = declarations.derniere(&ident, &brute)?;
        let ntt_declare = declaree.as_ref().filter(|d| d.nir.is_empty()).map(|d| d.ntt.as_str());
        crate::ntt::poser_ntt_declare(&mut f, &ent_e, &ident, &brute, &date_ref, ntt_declare)?;
        // LE BLOC 31 : ce qui differe de l'identite declaree, a l'ancienne valeur, date par l'historique de la fiche ; un
        // changement que la fiche ne date pas refuse.
        crate::changements_individu::ecrire(&mut f, &ident, &brute, declaree.as_ref(), mois, &fin_m)?;
        let taux_pas = pas_par_salarie.get(cle).cloned().flatten();

        for (i, p) in lot.iter().enumerate() {
            let siens: Vec<&Arret> = arrets_du_mois.iter().copied().filter(|a| a.du_contrat(p)).collect();
            let debut_bloc = f.lignes.len();
            bloc_contrat(&mut f, p, &ent_e, &orgs, i as i64 + 1, &bar, true, &siens, "01", emplois_multiples(p, &periode))?;
            // Ce qui a change depuis la derniere DSN deposee du contrat : le bloc S21.G00.41, sous son bloc 40.
            let declare = bloc_40_declare(ctx, p, mois, &etats_du_mois, &departs, &orgs)?;
            crate::changements_contrat::poser(&mut f.lignes, debut_bloc, ctx, p, mois, &nics_courant_d_abord, declare.as_ref())?;
        }
        let mut rangs_regs = Vec::new();
        let mut ajoutes: Vec<&Regularisation> = Vec::new();
        for r in &corrections {
            let deja = lot.iter().position(|p| p.engagement.numero == r.apres.engagement.numero).map(|i| i as i64 + 1);
            // UN CONTRAT SUR PLUSIEURS MOIS corrige pour plusieurs mois (21/10/2026) : un seul bloc 40 sous l'individu,
            // pose par la premiere de ses corrections ; chaque mois garde son versement, rattache a ce mois (cahier
            // technique DSN 2026.1, §2.2.2.5 : « valorisees mois civil par mois civil »).
            let meme_contrat = ajoutes.iter().position(|a| a.apres.engagement.numero == r.apres.engagement.numero);
            let rang = match (deja, meme_contrat) {
                (Some(rg), _) => rg,
                (None, Some(k)) => (lot.len() + k + 1) as i64,
                (None, None) => {
                    let rg = (lot.len() + ajoutes.len() + 1) as i64;
                    ajoutes.push(r);
                    // Le bloc 40 est celui du contrat tel qu'il est corrige, lu dans la correction de son dernier mois :
                    // c'est elle qui sait s'il finit (bloc 62).
                    let groupe: Vec<&Regularisation> =
                        corrections.iter().copied().filter(|x| x.apres.engagement.numero == r.apres.engagement.numero).collect();
                    let dernier = groupe.iter().copied().fold(*r, |a, x| if x.mois > a.mois { x } else { a });
                    let b = bar_de(&dernier.mois)?;
                    let autres: Vec<&Paie> =
                        regs.iter().filter(|x| x.mois == dernier.mois).map(|x| &x.apres).chain(des_mois_regularises[&dernier.mois].iter()).collect();
                    // UNE FIN CORRIGEE (21/10/2026) : « une correction de fin de contrat s'opere par l'emission d'un bloc
                    // " Fin du contrat - S21.G00.62 " dans la DSN mensuelle suivante portant les informations correctes
                    // qui ecraseront celles precedemment declarees » (fiche net-entreprises 1722,
                    // `atelier/reference/sources/net-entreprises-base-connaissances/`). UN CONTRAT AJOUTE apres la limite
                    // se declare en entier : sa fin aussi, comme celle de tout contrat termine (bloc 62, « Evenement de fin du
                    // contrat de travail », `norme-dsn/texte/bloc-S21.G00.62.txt`). `bloc_contrat` ne l'ecrit que si le contrat finit dans le mois
                    // de la correction qui le pose.
                    let ajoute = |x: &Regularisation| x.declare.lignes.is_empty() && x.declare.fichier.is_empty();
                    let fin = groupe.iter().any(|x| ajoute(x) || x.apres.engagement.fin != x.avant.engagement.fin);
                    let debut_bloc = f.lignes.len();
                    bloc_contrat(&mut f, &dernier.apres, &ent_e, &orgs, rg, &b, fin, &[], "01", emplois_multiples(&dernier.apres, &autres))?;
                    // UNE DATE DE DEBUT CORRIGEE : l'ancienne, lue dans ce qui a ete declare, sous le bloc 40 (fiche 1952) --
                    // une fois, depuis la correction du dernier mois qui a declare le contrat.
                    let declarant = groupe.iter().copied().filter(|x| !ajoute(x)).fold(None, |a: Option<&Regularisation>, x| match a {
                        Some(a) if a.mois >= x.mois => Some(a),
                        _ => Some(x),
                    });
                    if let Some(d) = declarant {
                        crate::changements_contrat::ancienne_date_de_debut(&mut f.lignes, debut_bloc, d, &dernier.apres.engagement.debut, fin_m)?;
                    }
                    rg
                }
            };
            rangs_regs.push(rang);
        }
        // Un seul bloc 40 sous l'individu (les contrats du mois, plus ceux qu'une regularisation ajoute) : 78.006 ne
        // s'ecrit pas sur ses bases (p. 298). Les lignes d'annulation recopient le depose tel quel.
        f.contrat_unique = lot.len() + ajoutes.len() == 1;
        for (i, p) in lot.iter().enumerate() {
            bloc_versement(&mut f, p, &bar, i as i64 + 1, taux_pas.as_ref())?;
        }
        // POINT 18 : une regularisation d'un contrat aussi paye ce mois garde le rang de ce contrat (ses
        // affiliations), mais pas son numero de versement -- deux blocs 50 au meme numero sous un individu, la CCH-11 du
        // bloc 50 l'interdit (p. 243). Les versements du mois portent 01 a n, ses regularisations n+1 et suivants.
        for (k, (rang, r)) in rangs_regs.iter().zip(&corrections).enumerate() {
            let b = bar_de(&r.mois)?;
            let numero = (lot.len() + k + 1) as i64;
            bloc_regularisation(&mut f, r, &b, *rang, numero, taux_pas.as_ref(), fin_m, &bar)?;
        }
        f.contrat_unique = false;
        let anciennete = anciennete_du_mois(s, cle, &fin_m)?;
        for p in &lot {
            bloc_anciennete(&mut f, p, anciennete)?;
        }
        for r in &ajoutes {
            bloc_anciennete(&mut f, &r.apres, anciennete)?;
        }
    }

    // Les versements aux organismes complementaires, leurs composants lus dans le nominatif ecrit.
    let composants = composants_de_versement(&f.lignes)?;
    let somme_composants = composants.iter().fold(zero(), |s, c| add(s, c.montant));
    let somme_059 = lire(&totaux_avec_regs, "059").unwrap_or(zero());
    if somme_composants != somme_059 {
        return Err(format!(
            "Les cotisations 059 du mois ({} EUR, regularisations comprises) et les composants de versement que le \
nominatif rattache a une adhesion ({} EUR) ne font pas la meme somme : une base de prevoyance n'est pas rattachee a son \
affiliation (S21.G00.78.005). Rien n'est ecrit.",
            mt(somme_059),
            mt(somme_composants)
        ));
    }
    if let Some(c) = composants.iter().find(|c| !places_oc.iter().any(|(_, v)| en_texte(g(v, "siret")) == c.organisme)) {
        if !sans_individu {
            return Err(format!(
                "Des cotisations sont declarees sous l'adhesion {} de l'organisme {} (S21.G00.15.002), et \
config/organismes.json n'a pas de versement a cet organisme (`versements`, \"siret\": \"{}\") : le bloc 20 qui les \
paie ne se devine pas. Rien n'est ecrit.",
                c.code, c.organisme, c.organisme
            ));
        }
    }
    for (place, v) in places_oc.iter().rev() {
        let siret = en_texte(req(v, "siret")?);
        let siens: Vec<&Composant> = composants.iter().filter(|c| c.organisme == siret).collect();
        let bloc = versement_complementaire(v, &ent, banque, &siens, debut_mois, fin_m, echeances.get(siret.as_str()).copied())?;
        if !bloc.is_empty() {
            let a = bordereau::a_verser(siens.iter().fold(zero(), |s, c| add(s, c.montant)));
            if let Some((_, i)) = places_ordres_oc.iter().find(|(s, _)| *s == siret) {
                ordres[*i] = Some(Ordre {
                    organisme: siret.clone(),
                    reference: reference_versement(v, &ent)?,
                    montant: a.montant,
                    debut: debut_mois,
                    fin: fin_m,
                    date_paiement: echeances.get(siret.as_str()).copied(),
                    destinataire: Destinataire::Complementaire,
                    reste: a.credit,
                    deduit: zero(),
                });
            }
        }
        f.lignes.splice(*place..*place, bloc);
    }

    let du = pas_retenu(&f.lignes)?;
    let remplace = type_declaration == "03" || type_declaration == "05";
    let dgfip = versement_dgfip(du, mois, debut_mois, fin_m, banque, remplace, o.remplacee_versait_pas)?;
    if !dgfip.is_empty() {
        let mut arrondi = bordereau::euro(du);
        arrondi.rescale(2);
        let montant = bordereau::a_verser(arrondi).montant;
        let reste = if montant.is_zero() { (-du).max(zero()) } else { zero() };
        ordres.insert(0, Some(Ordre { organisme: DGFIP.to_string(), reference: Some(DGFIP_PAS.to_string()), montant, debut: debut_mois, fin: fin_m, date_paiement: None, destinataire: Destinataire::Dgfip, reste, deduit: zero() }));
    }
    f.lignes.splice(place_dgfip..place_dgfip, dgfip);

    let n = f.lignes.len() + 2;
    f.add("S90.G00.90.001", n);
    f.add("S90.G00.90.002", "1");
    let texte = f.texte();
    verifier_affiliations(&texte)?;
    crate::ntt::verifier_fichier(&texte)?;
    crate::usages::verifier_fichier(&texte)?;
    crate::formats::verifier_fichier(&texte)?;
    Ok(Fabriquee { texte, ordres: ordres.into_iter().flatten().collect(), bordereaux })
}

/// Le texte en latin-1, comme `dsn.ecrire` l'ecrit.
pub fn en_latin1(texte: &str) -> Resultat<Vec<u8>> {
    texte
        .chars()
        .map(|c| if (c as u32) < 256 { Ok(c as u8) } else { Err(format!("'latin-1' codec can't encode character {:?}", c)) })
        .collect()
}

pub fn ecrire(texte: &str, chemin: &Path) -> Resultat<()> {
    if let Some(d) = chemin.parent() {
        std::fs::create_dir_all(d).map_err(|e| format!("{} : {}", d.display(), e))?;
    }
    std::fs::write(chemin, en_latin1(texte)?).map_err(|e| format!("{} : {}", chemin.display(), e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use socle::date::d;
    use serde_json::json;
    use std::path::PathBuf;

    /// Un dossier de compagnie temporaire : fiches vides, `config/casc-svp.json` des mois regularises.
    fn ctx_dsn(nom: &str, regularises: &[&str]) -> Contexte {
        let racine: PathBuf = std::env::temp_dir().join(format!("puppetpaie-dsn-{}-{}", std::process::id(), nom));
        let _ = std::fs::remove_dir_all(&racine);
        let cfg = racine.join("config");
        std::fs::create_dir_all(&cfg).unwrap();
        for f in ["entreprise.json", "salaries.json", "spectacles.json"] {
            std::fs::write(cfg.join(f), "{}").unwrap();
        }
        std::fs::write(cfg.join("casc-svp.json"), json!({"regularisations": [{"mois": regularises}]}).to_string()).unwrap();
        Contexte::nouveau(&racine, Some(d(&json!("2026-09-27")).unwrap())).unwrap()
    }

    fn orgs_casc() -> Value {
        json!({"adhesions_prevoyance": [{"libelle": "CASCSVP", "reference": "100"},
                                        {"libelle": "INTERBRANCHE", "reference": "102"}],
               "affiliations_par_nature": {"intermittent": {"references": ["103", "102"]},
                                           "permanent": {"references": ["100", "101"]}}})
    }

    // La CASC-SVP de la 3090 (`atelier/reference/sources/casc-svp-fiche-codification-dsn-202606.txt`).
    #[test]
    fn la_dsn_de_la_3090_porte_la_casc_en_troisieme_affiliation() {
        let c = casc_svp(&ctx_dsn("a", &[]), &json!({"idcc": "3090"}), &orgs_casc(), "2026-09").unwrap().unwrap();
        assert_eq!(c["reference"], json!("100"));
        assert_eq!(c["lignes"], json!(["ccn"]));
        // l'intermittent : apres 103 et 102, donc 204 ; le permanent l'a deja.
        assert_eq!(c["natures"], json!({"intermittent": "204"}));
    }

    // `atelier/reference/sources/casc-svp-fiche-codification-dsn-202606.txt`.
    #[test]
    fn pas_de_casc_hors_3090_avant_2025_ni_sur_un_mois_regularise() {
        let o = orgs_casc();
        assert!(casc_svp(&ctx_dsn("b1", &[]), &json!({"idcc": "1285"}), &o, "2026-09").unwrap().is_none());
        assert!(casc_svp(&ctx_dsn("b2", &[]), &json!({"idcc": "3090"}), &o, "2024-12").unwrap().is_none());
        assert!(casc_svp(&ctx_dsn("b3", &["2026-08"]), &json!({"idcc": "3090"}), &o, "2026-08").unwrap().is_none());
    }

    #[test]
    fn une_3090_sans_adhesion_casc_refuse() {
        let mut o = orgs_casc();
        o["adhesions_prevoyance"] = json!([{"libelle": "INTERBRANCHE", "reference": "102"}]);
        let e = casc_svp(&ctx_dsn("c", &[]), &json!({"idcc": "3090"}), &o, "2026-09").unwrap_err();
        assert!(e.contains("CASCSVP"), "{}", e);
    }

    // S21.G00.20.002 de France Travail (`atelier/reference/sources/francetravail-100-dsn/net-entreprises-fiche-3344-intermittents-20251113.html`).
    #[test]
    fn le_dossier_france_travail_de_la_fiche_part_dans_la_dsn() {
        let ft = json!({"siret": SIRET_FRANCE_TRAVAIL, "reference": "CS00012345"});
        assert_eq!(reference_versement(&ft, &json!({})).unwrap().as_deref(), Some("CS00012345"));
        assert_eq!(
            reference_versement(&json!({"siret": SIRET_FRANCE_TRAVAIL}), &json!({"france_travail_dossier_recouvrement": "CS00000001"}))
                .unwrap()
                .as_deref(),
            Some("CS00000001")
        );
        assert_eq!(
            reference_versement(
                &json!({"siret": "78861779300013", "reference": "X"}),
                &json!({"france_travail_dossier_recouvrement": "CS00000001"})
            )
            .unwrap()
            .as_deref(),
            Some("X")
        );
        let e = reference_versement(&ft, &json!({"france_travail_dossier_recouvrement": "CS00000002"})).unwrap_err();
        assert!(e.contains("CS00000002") && e.contains("CS00012345"), "{}", e);
        assert_eq!(SIRET_FRANCE_TRAVAIL, "13000548100010");
    }

    // Le tableau des usages (`atelier/reference/norme-dsn/dsn-cahier-technique-2026.1-usages.txt`).
    #[test]
    fn statut_boeth_admis_en_mensuelle() {
        assert!(usage_admet(&Fichier::en_norme(crate::norme::Norme::connue("P26V01").unwrap()), "S21.G00.40.072", "01").unwrap());
    }

    // `atelier/reference/norme-dsn/texte/bloc-S21.G00.50.txt`, `bloc-S21.G00.56.txt` (la somme est une inference, `pas_retenu`).
    #[test]
    fn le_pas_retenu_compte_les_50_009_et_les_regularisations_d_indu() {
        let l: Vec<String> = ["S21.G00.50.009,'187.18'", "S21.G00.50.013,'2000.00'", "S21.G00.50.009,'12.40'", "S21.G00.56.007,'-30.08'"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(pas_retenu(&l).unwrap(), Decimal::from_str_exact("169.50").unwrap());
    }

    // `atelier/reference/sources/net-entreprises-base-connaissances/net-entreprises-fiche1931-situations-creditrices-versement-20250323.txt`.
    #[test]
    fn le_versement_dgfip_s_arrondit_a_l_euro_et_verse_zero_pour_un_pas_negatif() {
        let banque = json!({"bic": "BANQFRPPXXX", "iban": "FR7630001007941234567890185"});
        let (debut, fin) = (Jour::nouveau(2026, 7, 1).unwrap(), Jour::nouveau(2026, 7, 31).unwrap());
        let montant = |du: &str| -> Option<String> {
            let l = versement_dgfip(Decimal::from_str_exact(du).unwrap(), "2026-07", debut, fin, &banque, false, None).unwrap();
            l.iter().find_map(|x| x.strip_prefix("S21.G00.20.005,").map(str::to_string))
        };
        assert_eq!(montant("187.18"), Some("'187.00'".into()));
        assert_eq!(montant("187.50"), Some("'188.00'".into()));
        assert_eq!(montant("169.49"), Some("'169.00'".into()));
        // Moins de 50 centimes : rien a reverser, rien a ecrire sur une DSN normale.
        assert_eq!(montant("0.40"), None);
        // Un PAS negatif : un versement a 0.00, que la fiche net-entreprises 1931 admet « par commodite » (l. 13).
        assert_eq!(montant("-3.10"), Some("'0.00'".into()));
    }

    // ------------------------------------------------------------ les composants du versement (bloc 55)

    fn l(k: &str, v: &str) -> String {
        format!("{},'{}'", k, v)
    }

    /// Un individu, une adhesion, une base 31 rattachee : le minimum que `composants_de_versement` lit.
    fn fichier_55(debut: &str, fin: &str, aff: &str, avec_15: bool) -> Vec<String> {
        let mut f = Vec::new();
        if avec_15 {
            f.extend([l("S21.G00.15.001", "CONTRAT A"), l("S21.G00.15.002", "P0983"), l("S21.G00.15.004", "01"), l("S21.G00.15.005", "100")]);
        }
        f.extend([
            l("S21.G00.30.001", "1"),
            l("S21.G00.30.002", "MARTIN"),
            l("S21.G00.40.009", "C1"),
            l("S21.G00.70.012", "202"),
            l("S21.G00.70.013", "100"),
            l("S21.G00.78.001", "31"),
            l("S21.G00.78.002", debut),
            l("S21.G00.78.003", fin),
            l("S21.G00.78.004", "0.00"),
            l("S21.G00.78.005", aff),
            l("S21.G00.81.001", "059"),
            l("S21.G00.81.004", "12.50"),
            l("S21.G00.78.001", "31"),
            l("S21.G00.78.002", debut),
            l("S21.G00.78.003", fin),
            l("S21.G00.78.005", aff),
            l("S21.G00.81.001", "059"),
            l("S21.G00.81.004", "-2.50"),
        ]);
        f
    }

    /// 55.004 : « une periode civile complete [...] contenant la ou les periodes cotisees » (cahier technique 2026.1,
    /// `bloc-S21.G00.55.txt`, p. 260, l. 85-87).
    #[test]
    fn la_periode_d_affectation_contient_les_periodes_cotisees() {
        let c = composants_de_versement(&fichier_55("01042026", "30042026", "202", true)).unwrap();
        assert_eq!(c.len(), 1, "deux bases du meme contrat et de la meme periode : un composant");
        assert_eq!((c[0].code.as_str(), c[0].organisme.as_str(), c[0].periode.as_str(), c[0].montant), ("CONTRAT A", "P0983", "2026T02", "10.00".parse::<Decimal>().unwrap()));
        assert_eq!(composants_de_versement(&fichier_55("15032026", "10042026", "202", true)).unwrap()[0].periode, "2026S01");
        assert_eq!(composants_de_versement(&fichier_55("15062026", "10072026", "202", true)).unwrap()[0].periode, "2026A00");
        let e = composants_de_versement(&fichier_55("15122025", "10012026", "202", true)).unwrap_err();
        assert!(e.contains("deux annees civiles"), "{}", e);
    }

    /// 55.003 : « Le contrat mentionne doit faire l'objet d'un bloc Adhesion Prevoyance S21.G00.15 », CCH-11 (p. 260,
    /// l. 72-79).
    #[test]
    fn un_code_d_affectation_qui_ne_se_lit_nulle_part_refuse() {
        let e = composants_de_versement(&fichier_55("01042026", "30042026", "203", true)).unwrap_err();
        assert!(e.contains("MARTIN") && e.contains("203") && e.contains("ne se lit nulle part"), "{}", e);
        let e = composants_de_versement(&fichier_55("01042026", "30042026", "202", false)).unwrap_err();
        assert!(e.contains("100") && e.contains("S21.G00.15"), "{}", e);
    }

    // ------------------------------------------------------------ l'arret de travail

    fn arret(reprise: Option<&str>, fin_previsionnelle: &str) -> Arret {
        Arret {
            v: json!({
                "salarie": "Audinot", "contrat_debut": "2025-09-01", "contrat_numero": "AU.25.01", "motif": "01",
                "dernier_jour_travaille": "2026-06-20", "fin_previsionnelle": fin_previsionnelle, "subrogation": "02",
                "reprise": reprise, "reprise_motif": reprise.map(|_| "01"), "date_accident": null, "envois": [],
            }),
        }
    }

    /// La suspension se declare dans les DSN dont le mois principal a une periode commune avec elle (bloc S21.G00.60,
    /// cahier technique DSN 2026.1, p. 265) ; la reprise est le « premier jour travaille et paye suite a l'arret de
    /// travail » (S21.G00.60.010, p. 269) : le dernier jour d'arret est la veille.
    #[test]
    fn un_arret_repris_le_premier_du_mois_ne_couvre_pas_ce_mois() {
        let j = |s: &str| Jour::lire(s).unwrap();
        let (juin, juillet) = ((j("2026-06-01"), j("2026-06-30")), (j("2026-07-01"), j("2026-07-31")));
        let repris_le_1er = arret(Some("2026-07-01"), "2026-07-03");
        assert!(repris_le_1er.couvre(&juin.0, &juin.1).unwrap());
        assert!(!repris_le_1er.couvre(&juillet.0, &juillet.1).unwrap(), "aucun jour d'arret en juillet");
        assert!(arret(Some("2026-07-02"), "2026-07-03").couvre(&juillet.0, &juillet.1).unwrap(), "le 1er juillet est arrete");
        assert!(arret(None, "2026-07-01").couvre(&juillet.0, &juillet.1).unwrap(), "la fin previsionnelle est un jour d'arret");
        assert!(!arret(None, "2026-06-30").couvre(&juillet.0, &juillet.1).unwrap());
    }
}

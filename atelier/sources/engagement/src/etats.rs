//! LES TROIS ETATS D'UN ENGAGEMENT, pour un mois :
//!
//! - EN SAISIE (`EnSaisie`) : il se calcule a mesure pour l'ecran ; rien ne peut partir de lui ;
//! - VALIDE (`Valide`) : « Valider l'engagement » a ecrit, dans l'engagement, `valide` : la date, le numero de sa
//!   version, sa saisie telle qu'elle etait, ses paies calculees, une par mois qu'il couvre, et pour chacune l'empreinte
//!   de ce dont son calcul depend (`empreinte`). Tout ce qui part part d'elles (`paies_du_mois`, `Valide::paie`) ;
//! - DOCUMENTS PARTIS (`Partis`) : les documents du mois (bulletin, certificat Conges Spectacles, AER) sont partis au
//!   salarie ; ce mois ne se recalcule plus de lui-meme, sauf par une decision de l'editeur. La couche des envois le dit
//!   (`departs`).
//!
//! Trois types, et seul un engagement valide (ou parti, qui l'est) donne une paie a qui fabrique un document : un
//! engagement en saisie n'a pas de `paie`.
//!
//! LES VERSIONS DATEES. Tant que rien n'est parti d'un engagement, il se modifie librement, sans historique. Des qu'un
//! envoi est parti sur sa version (`departs`), une revalidation qui le change garde dans l'engagement la version d'avant
//! (`versions` : sa saisie et ses paies telles qu'elles etaient, la date de sa validation et celle ou elle a ete
//! remplacee), et le present prend le numero suivant. Une revalidation qui ne change rien ne fait pas de version. Une
//! version passee ne se reecrit jamais. Ce qui est a refaire (`a_refaire`) se tire de la comparaison entre le present et
//! la version sur laquelle chaque envoi est parti.
//!
//! LE RECALCUL DE LUI-MEME (`rafraichir`) : a chaque lecture d'un mois (`du_mois`, par ou passe tout ce qui lit une
//! paie validee : l'ecran, les exports, les declarations, les documents, les envois), la paie de chaque engagement
//! valide se compare a son empreinte. Si elle ne tient plus, la paie se recalcule et se reecrit ; si un chiffre a change,
//! elle dit pourquoi (`raison`) : « taux de PAS reçu » (seul le prelevement change), « règle du JJ/MM/AAAA » (une regle
//! datee s'applique : du bareme, ou une valeur datee de la compagnie, son taux AT/MP de l'annee), « calcul corrigé » (le code qui calcule a change), « un autre engagement du mois a changé » (un
//! engagement que son calcul lit), « saisie du mois modifiée » (le taux du cachet, la date de paiement), « la paie de
//! <mois> a changé » (une paie validee d'un autre mois que son calcul lit : conges, cumuls), « configuration de la
//! compagnie modifiée » (une fiche corrigee sans date nouvelle, les contrats d'organismes). Un changement qui ne change aucun chiffre ne laisse aucune trace.
//! Un mois dont les documents sont partis ne se recalcule que par une decision de l'editeur (« calcul corrigé », une
//! regle datee) : le recalcul fait alors une version datee, comme une modification, et la paie refaite rectifie le
//! bulletin parti.
//!
//! MODIFIER UN ENGAGEMENT VALIDE se fait d'un geste, dans une seule ecriture de la route : il repasse en saisie
//! (`modifier`), sa validation mise de cote (`precedente`), prend sa saisie nouvelle (`stock::poser_la_saisie`) et se
//! revalide aussitot ; la validation reprend telles quelles les paies des mois dont les documents sont partis tant que la
//! saisie de leur mois n'a pas change, et fait une version si quelque chose est parti sur elle. Il ne reste jamais
//! rouvert.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::Path;

use serde_json::{json, Map, Value};

use crate::calcul::Paie;
use crate::departs::{Depart, Departs, Envoi};
use crate::stock::{self, Objet};
use crate::{empreinte, relire};
use droit::contexte::Contexte;
use socle::date::{en_texte, Jour};
use socle::fichiers::vrai;
use socle::num::{dec_avec_echelle, pct, Decimal, Resultat};

/// La raison d'un recalcul : le taux de prelevement a la source a change.
pub const TAUX_DE_PAS: &str = "taux de PAS reçu";
/// La raison d'un recalcul : le code qui calcule a change (le moteur, ou les baremes et catalogues qu'il livre).
pub const CALCUL_CORRIGE: &str = "calcul corrigé";
/// La raison d'un recalcul : la saisie d'un autre engagement que le calcul lit a change (plafond, cumuls, conges).
pub const AUTRE_ENGAGEMENT: &str = "un autre engagement du mois a changé";
/// La raison d'un recalcul : ce que la saisie du mois pose pour tous ses engagements (taux du cachet, date de paiement) a
/// change.
pub const SAISIE_DU_MOIS: &str = "saisie du mois modifiée";
/// La raison d'un recalcul : la configuration de la compagnie a change sans valeur datee nouvelle (une fiche corrigee, les
/// contrats d'organismes).
pub const CONFIGURATION: &str = "configuration de la compagnie modifiée";

/// Dans l'engagement : la validation qu'une modification va remplacer, et les versions passees.
const PRECEDENTE: &str = "precedente";
const VERSIONS: &str = "versions";

// ------------------------------------------------------------------ ce qui est parti

/// Ce qui est parti de l'engagement `o` (`departs`), d'apres la cle de son salarie et son numero.
pub(crate) fn ses_departs<'a>(ctx: &Contexte, o: &Objet, departs: &'a Departs) -> &'a [Depart] {
    match (cle_du_salarie(ctx, o), o.brut.get("numero").filter(|n| vrai(n)).map(en_texte)) {
        (Some(c), Some(n)) => departs.de(&c, &n),
        _ => &[],
    }
}

/// Quand les documents de `mois` de cet engagement sont partis (le premier envoi) ; `None` s'ils ne le sont pas.
fn documents_partis<'a>(siens: &'a [Depart], mois: &str) -> Option<&'a str> {
    siens.iter().find(|d| d.envoi == Envoi::Documents && d.mois.as_deref() == Some(mois)).map(|d| d.le.as_str())
}

// ------------------------------------------------------------------ les trois etats

/// Un engagement en saisie.
#[derive(Debug, Clone)]
pub struct EnSaisie {
    pub objet: Objet,
}

/// Un engagement valide : ses paies sont ecrites. Seul ce module en fait un.
#[derive(Debug, Clone)]
pub struct Valide {
    objet: Objet,
    le: String,
    /// Pourquoi la paie du mois lu ne s'est pas ecrite : le refus du recalcul (`du_mois`), quand elle manque.
    refus: Option<String>,
}

/// Un engagement valide dont les documents du mois `mois` sont partis le `le`.
#[derive(Debug, Clone)]
pub struct Partis {
    valide: Valide,
    pub mois: String,
    pub le: String,
}

/// Ce qu'est un engagement pour un mois.
#[derive(Debug, Clone)]
pub enum Etat {
    EnSaisie(EnSaisie),
    Valide(Valide),
    Partis(Partis),
}

impl Valide {
    fn de(objet: Objet) -> Option<Valide> {
        let le = objet.brut.get("valide")?.get("le").map(en_texte)?;
        Some(Valide { objet, le, refus: None })
    }

    /// L'engagement tel qu'il est ecrit.
    pub fn objet(&self) -> &Objet {
        &self.objet
    }

    /// Quand il a ete valide.
    pub fn le(&self) -> &str {
        &self.le
    }

    /// Le numero de sa version (1 tant qu'aucune modification n'a suivi un envoi) ; `None` pour une validation qui ne le
    /// porte pas (`a_revalider`).
    pub fn version(&self) -> Option<u64> {
        version_de(self.objet.brut.get("valide"))
    }

    /// CE QUI PART SE LIT DANS LES VERSIONS : une validation sans son numero de version ou sans sa saisie, ou une paie
    /// ecrite sans la fiche ni la saisie du mois que son calcul a lues (`fiches`, `MOIS_LU`), ne dit pas ce qu'un envoi
    /// emporterait. Rien ne part d'elle et rien ne s'y relit : la phrase du refus ; `None` quand elle dit tout.
    pub fn a_revalider(&self, ctx: &Contexte) -> Option<String> {
        a_revalider(ctx, &self.objet, self.objet.brut.get("valide")?)
    }

    /// Ses versions passees, de la plus ancienne a la plus recente : `{"version", "valide_le", "remplacee_le", "saisie",
    /// "paies"}`.
    pub fn versions(&self) -> &[Value] {
        self.objet.brut.get(VERSIONS).and_then(Value::as_array).map_or(&[], Vec::as_slice)
    }

    /// La version `n`, passee ou presente, telle qu'elle est ecrite (`saisie`, `paies`).
    pub fn version_numero(&self, n: u64) -> Option<&Value> {
        if Some(n) == self.version() {
            return self.objet.brut.get("valide");
        }
        self.versions().iter().find(|v| version_de(Some(v)) == Some(n))
    }

    /// La paie ecrite de `mois` (`{"paie", "fiches", "pas"?, "empreinte", "recalcul"?}`), si elle l'est.
    pub fn paie_ecrite(&self, mois: &str) -> Option<&Value> {
        self.objet.brut.get("valide")?.get("paies")?.get(mois)
    }

    /// Pourquoi la paie de `mois` s'est recalculee d'elle-meme, et quand : `{"le", "raison"}`.
    pub fn recalcul(&self, mois: &str) -> Option<&Value> {
        self.paie_ecrite(mois)?.get("recalcul").filter(|r| r.is_object())
    }

    /// LA PAIE DE `mois`, TELLE QU'ELLE A ETE VALIDEE : ce que lit ce qui fabrique un document. `data` : la saisie du mois
    /// (`stock::vue_du_mois`), `i` : le rang de l'engagement dans cette saisie.
    pub fn paie(&self, ctx: &Contexte, data: &Value, i: usize) -> Resultat<Paie> {
        let mois = data.get("mois").and_then(Value::as_str).ok_or_else(|| socle::fichiers::absent("mois"))?;
        let ecrite = self.paie_ecrite(mois).and_then(|p| p.get("paie")).ok_or_else(|| match &self.refus {
            Some(refus) => format!("{} : la paie de {} ne se calcule pas. {}", qui_de(ctx, &self.objet), socle::date::libelle_du_mois_ou_tel_quel(mois, socle::date::Graphie::Accentuee), refus),
            None => format!(
                "{} : la paie de {} n'est pas calculée. Revalidez l'engagement : ouvrez sa carte, puis « Modifier l'engagement ».",
                qui_de(ctx, &self.objet),
                socle::date::libelle_du_mois_ou_tel_quel(mois, socle::date::Graphie::Accentuee)
            ),
        })?;
        let une = relire::saisie_d_un_engagement(data, i);
        let mut paie = relire::paies(ctx, &une, &[ecrite])?.pop().ok_or("paie ecrite illisible")?;
        let p = paie.ecrire();
        p.pas = self.paie_ecrite(mois).and_then(|x| x.get("pas")).filter(|x| x.is_object()).map(droit::pas::Versement::lire).transpose()?;
        p.conges = self.paie_ecrite(mois).and_then(|x| x.get(CONGES)).filter(|x| x.is_object()).map(crate::conges::CompteurConges::lire).transpose()?;
        p.conges_avant = self.paie_ecrite(mois).and_then(|x| x.get(CONGES_AVANT)).filter(|x| x.is_object()).map(crate::conges::Avant::lire).transpose()?;
        p.numero_bulletin = self.bulletin(mois).map(String::from);
        Ok(paie)
    }

    /// Le numero du bulletin de `mois` (`N/AAAA`), ecrit quand ses documents sont partis (`poser_numeros_des_documents`).
    pub fn bulletin(&self, mois: &str) -> Option<&str> {
        self.paie_ecrite(mois)?.get(BULLETIN)?.as_str()
    }

    /// Le premier N CE du certificat Conges Spectacles de `mois`, ecrit quand ses documents sont partis.
    pub fn premier_ce(&self, mois: &str) -> Option<i64> {
        self.paie_ecrite(mois)?.get(CE)?.as_i64()
    }

    /// Le numero du bulletin que la paie de `mois` rectifie : celui d'une paie partie, refaite depuis.
    pub fn rectifie(&self, mois: &str) -> Option<&str> {
        self.paie_ecrite(mois)?.get(RECTIFIE)?.as_str()
    }
}

/// Dans une paie ecrite : le numero de son bulletin, le premier N CE de son certificat, et le bulletin qu'elle rectifie.
pub const BULLETIN: &str = "bulletin";
pub const CE: &str = "ce";
pub const RECTIFIE: &str = "rectifie";
/// Dans une paie ecrite : ce que la saisie du mois posait pour tous ses engagements quand elle a ete calculee (le taux du
/// cachet, la date de paiement), tel que son calcul l'a lu.
pub const MOIS_LU: &str = "saisie_du_mois";
/// Dans une paie ecrite : le compteur de conges payes que son bulletin montre (`conges::compteur_au_bulletin`), calcule
/// avec elle.
pub const CONGES: &str = "conges";
/// Dans une paie ecrite : le compteur de conges payes avant ce mois (`conges::Avant`), calcule avec elle. Le mois suivant
/// le reprend et n'y ajoute que ce mois-ci, sans relire les mois d'avant.
pub const CONGES_AVANT: &str = "conges_avant";

/// LES NUMEROS DES DOCUMENTS QUI PARTENT, ecrits dans la paie de `mois` de l'engagement du salarie `cle` au contrat
/// `numero` : le numero de son bulletin, le premier N CE de son certificat. Ils se relisent ensuite (`Valide::bulletin`,
/// `Valide::premier_ce`) : un document reparti a l'identique garde ses numeros. Une paie deja numerotee ne change pas de
/// numero.
pub fn poser_numeros_des_documents(ctx: &Contexte, mois: &str, cle: &str, numero: &str, bulletin: Option<&str>, ce: Option<i64>) -> Resultat<()> {
    let mut o = stock::objets_du_mois(&ctx.racine, mois)?
        .into_iter()
        .find(|o| {
            let e = Value::Object(o.brut.clone());
            crate::cle_de_l_engagement(ctx, &e).as_deref() == Some(cle) && e.get("numero").and_then(Value::as_str) == Some(numero)
        })
        .ok_or_else(|| format!("{} : pas d'engagement de {} au contrat {}", mois, cle, numero))?;
    let Some(p) = o.brut.get_mut("valide").and_then(|v| v.get_mut("paies")).and_then(|p| p.get_mut(mois)).and_then(Value::as_object_mut) else {
        return Err(format!("{} : l'engagement de {} au contrat {} n'a pas de paie validee", mois, cle, numero));
    };
    let mut change = false;
    if let Some(b) = bulletin {
        if !p.contains_key(BULLETIN) {
            p.insert(BULLETIN.into(), json!(b));
            change = true;
        }
    }
    if let Some(n) = ce {
        if !p.contains_key(CE) {
            p.insert(CE.into(), json!(n));
            change = true;
        }
    }
    if change {
        stock::reecrire(&ctx.racine, &o)?;
    }
    Ok(())
}

impl Partis {
    /// L'engagement valide dont les documents sont partis.
    pub fn valide(&self) -> &Valide {
        &self.valide
    }
}

impl Etat {
    /// L'engagement tel qu'il est ecrit.
    pub fn objet(&self) -> &Objet {
        match self {
            Etat::EnSaisie(e) => &e.objet,
            Etat::Valide(v) => &v.objet,
            Etat::Partis(p) => &p.valide.objet,
        }
    }

    /// L'engagement valide, qu'il soit parti ou non ; `None` en saisie.
    pub fn valide(&self) -> Option<&Valide> {
        match self {
            Etat::EnSaisie(_) => None,
            Etat::Valide(v) => Some(v),
            Etat::Partis(p) => Some(&p.valide),
        }
    }
}

/// La cle de la fiche du salarie d'un engagement (`None` s'il ne nomme aucune fiche).
fn cle_du_salarie(ctx: &Contexte, o: &Objet) -> Option<String> {
    crate::cle_de_l_engagement(ctx, &Value::Object(o.brut.clone()))
}

/// Qui est l'engagement `o`, pour une phrase de l'ecran : « MARTIN Lou (contrat du 20/11/2026) » (`crate::salarie_nomme`).
pub fn qui_de(ctx: &Contexte, o: &Objet) -> String {
    let debut = crate::debut_brut(&Value::Object(o.brut.clone()));
    let debut = Jour::lire(&debut).map(|j| j.jj_mm_aaaa()).unwrap_or(debut);
    format!("{} (contrat du {})", crate::salarie_nomme(ctx, o.brut.get("salarie")), debut)
}

/// Ce qu'est l'engagement `o` pour le mois `mois`, tel qu'il est ecrit.
fn etat(ctx: &Contexte, o: Objet, mois: &str, departs: &Departs) -> Etat {
    let parti = documents_partis(ses_departs(ctx, &o, departs), mois).map(String::from);
    match Valide::de(o.clone()) {
        None => Etat::EnSaisie(EnSaisie { objet: o }),
        Some(v) => match parti {
            Some(le) => Etat::Partis(Partis { valide: v, mois: mois.to_string(), le }),
            None => Etat::Valide(v),
        },
    }
}

/// L'etat de chaque engagement que `mois` montre, tel qu'il est ecrit, sans le recalcul de lui-meme, d'apres ce qui est
/// parti (`departs`, que la couche du dessus lit) : ce que lit un bilan qui ne lit aucune paie.
pub fn tels_qu_ecrits(ctx: &Contexte, mois: &str, departs: &Departs) -> Resultat<Vec<Etat>> {
    Ok(stock::objets_du_mois(&ctx.racine, mois)?.into_iter().map(|o| etat(ctx, o, mois, departs)).collect())
}

/// Le numero de version d'une validation ou d'une version ecrite.
fn version_de(v: Option<&Value>) -> Option<u64> {
    v.and_then(|v| v.get("version")).and_then(Value::as_u64)
}

/// Ce qui manque a la validation (ou a la version ecrite) `v` de `o` pour qu'un envoi en parte et qu'on y relise ce qui
/// est parti, dit en une phrase ; `None` quand rien ne manque.
fn a_revalider(ctx: &Contexte, o: &Objet, v: &Value) -> Option<String> {
    let mut manque = Vec::new();
    if version_de(Some(v)).is_none() {
        manque.push("son numero de version".to_string());
    }
    if !v.get("saisie").is_some_and(Value::is_object) {
        manque.push("sa saisie".to_string());
    }
    for (m, p) in v.get("paies").and_then(Value::as_object).into_iter().flatten() {
        if !p.get("fiches").is_some_and(Value::is_object) || !p.get(MOIS_LU).is_some_and(Value::is_object) {
            manque.push(format!("la fiche et la saisie du mois que la paie de {} a lues", m));
        }
    }
    (!manque.is_empty()).then(|| {
        format!(
            "{} : sa validation ne porte pas {}. Cet engagement est a revalider : ouvrir sa carte, puis « Modifier l'engagement » ; rien n'est ecrit.",
            qui_de(ctx, o),
            manque.join(", ni ")
        )
    })
}

/// LA LECTURE DE L'ENGAGEMENT VALIDE : l'etat de chaque engagement que `mois` montre, dans l'ordre de l'ecran, apres le
/// recalcul de lui-meme (`rafraichir`). C'est le seul chemin vers une paie validee : l'ecran, les exports, les
/// declarations, les documents et les envois lisent tous par ici, et lisent donc une paie juste.
///
/// UN RECALCUL QUE LE MOTEUR REFUSE n'est pas l'affaire de l'utilisateur : ce qui releve de lui (une saisie incomplete)
/// se refuse a la validation, avant tout calcul. L'engagement garde la paie validee, et le refus va au temoin que la
/// couche du dessus a pose (`poser_temoin`), qui le note pour l'editeur. Une paie que ce refus empeche d'ecrire (le mois
/// qu'un contrat sans terme n'avait pas encore) le porte : ce qui la lit dit pourquoi elle manque (le taux AT/MP de
/// l'annee, un bareme), et quoi faire.
pub fn du_mois(ctx: &Contexte, mois: &str, departs: &Departs) -> Resultat<Vec<Etat>> {
    match rafraichir(ctx, mois, departs) {
        // Rien ne s'est reecrit : les etats lus pour la verification sont ceux du mois.
        Ok(Some(etats)) => return Ok(etats),
        Ok(None) => {}
        Err(refus) => {
            if let Some(temoin) = TEMOIN.get() {
                temoin(&ctx.racine, mois, &refus);
            }
            // Une paie qui manque (un contrat sans terme qui entre dans le mois) ne s'est pas ecrite a cause de ce refus :
            // qui la lit le dit, avec sa cause (`Valide::paie`).
            let mut etats = tels_qu_ecrits(ctx, mois, departs)?;
            for e in etats.iter_mut() {
                let v = match e {
                    Etat::Valide(v) => v,
                    Etat::Partis(p) => &mut p.valide,
                    Etat::EnSaisie(_) => continue,
                };
                if v.paie_ecrite(mois).is_none() {
                    v.refus = Some(refus.clone());
                }
            }
            return Ok(etats);
        }
    }
    tels_qu_ecrits(ctx, mois, departs)
}

/// Qui recoit un recalcul refuse : `(racine de la compagnie, mois, refus du moteur)`.
pub type Temoin = fn(&Path, &str, &str);

static TEMOIN: std::sync::OnceLock<Temoin> = std::sync::OnceLock::new();

/// Pose le temoin des recalculs refuses (`du_mois`), une fois pour le processus.
pub fn poser_temoin(t: Temoin) {
    let _ = TEMOIN.set(t);
}

/// Pour chaque engagement que `mois` montre, dans l'ordre de l'ecran : est-il valide ? Ne lit aucune paie, donc ne
/// recalcule rien : ce que comptent l'accueil et le bilan d'un mois.
pub fn valides_du_mois(ctx: &Contexte, mois: &str) -> Resultat<Vec<bool>> {
    Ok(stock::objets_du_mois(&ctx.racine, mois)?.iter().map(|o| o.brut.get("valide").is_some_and(|v| v.get("le").is_some())).collect())
}

/// La paie ecrite de `mois` de l'engagement du salarie `cle` (la cle de sa fiche) qui commence le `debut` (AAAA-MM-JJ),
/// lue par `du_mois` ; `None` s'il n'est pas valide ou n'a pas de paie ecrite pour ce mois.
pub fn paie_ecrite_de(ctx: &Contexte, mois: &str, cle: &str, debut: &str, departs: &Departs) -> Resultat<Option<Value>> {
    Ok(du_mois(ctx, mois, departs)?.into_iter().find_map(|e| {
        let b = Value::Object(e.objet().brut.clone());
        let sien = crate::cle_de_l_engagement(ctx, &b).as_deref() == Some(cle) && crate::debut_brut(&b) == debut;
        if sien { e.valide().and_then(|v| v.paie_ecrite(mois)).cloned() } else { None }
    }))
}

/// RIEN NE PART SANS VALIDATION : les paies validees de `mois`, une par engagement qu'il montre, dans l'ordre de
/// l'ecran, telles qu'elles ont ete ecrites. Refus si un engagement du mois est encore en saisie : il se valide d'abord.
pub fn paies_du_mois(ctx: &Contexte, mois: &str, departs: &Departs) -> Resultat<Vec<Paie>> {
    let etats = du_mois(ctx, mois, departs)?;
    let Some(data) = stock::vue_du_mois(&ctx.racine, mois)? else { return Ok(Vec::new()) };
    let en_saisie: Vec<String> = etats.iter().filter(|e| e.valide().is_none()).map(|e| qui_de(ctx, e.objet())).collect();
    if !en_saisie.is_empty() {
        return Err(format!(
            "{} : {} engagement(s) ne sont pas encore valides ({}). Rien ne part d'un engagement en saisie : le valider \
             d'abord (« Valider l'engagement »).",
            crate::libelle_mois(mois),
            en_saisie.len(),
            en_saisie.join(", ")
        ));
    }
    etats.iter().enumerate().map(|(i, e)| e.valide().ok_or("engagement en saisie")?.paie(ctx, &data, i)).collect()
}

/// CE QUI PEUT PARTIR DE `mois` : sa saisie reduite aux engagements valides (dans l'ordre de l'ecran), et la paie ecrite
/// de chacun, au meme rang. Un engagement en saisie n'y est pas : aucun document ne part de lui. `None` : aucune saisie.
pub fn validees_du_mois(ctx: &Contexte, mois: &str, departs: &Departs) -> Resultat<Option<(Value, Vec<Paie>)>> {
    let etats = du_mois(ctx, mois, departs)?;
    let Some(data) = stock::vue_du_mois(&ctx.racine, mois)? else { return Ok(None) };
    let tous = data.get("engagements").and_then(Value::as_array).cloned().unwrap_or_default();
    let mut gardes = Vec::new();
    let mut paies = Vec::new();
    for (i, e) in etats.iter().enumerate() {
        let Some(v) = e.valide() else { continue };
        paies.push(v.paie(ctx, &data, i)?);
        gardes.push(tous.get(i).cloned().unwrap_or(Value::Null));
    }
    let mut reduite = data.clone();
    reduite["engagements"] = Value::Array(gardes);
    Ok(Some((reduite, paies)))
}

/// LES PAIES VALIDEES DE `mois` QU'UN CALCUL LIT (le compteur de conges, le brut de sa periode, la deduction forfaitaire
/// de l'annee) : la paie ecrite de chaque engagement valide du mois, telle qu'elle est ecrite, dans l'ordre de l'ecran.
/// Un engagement en saisie n'y est pas : ce qui n'est pas valide n'est pas paye. Rien ne se calcule ni ne se reecrit.
/// Pendant une validation, les mois que son engagement a deja calcules s'y lisent comme ecrits (`EnValidation`).
///
/// Le calcul qui les lit note leur empreinte (`paies <mois>`, `empreinte_des_paies_validees`) : une paie validee qui
/// change fait recalculer les mois qui l'ont lue.
pub fn paies_validees(ctx: &Contexte, mois: &str) -> Resultat<Vec<Paie>> {
    let Some(data) = stock::vue_du_mois(&ctx.racine, mois)? else { return Ok(Vec::new()) };
    let valides = valides_lues(ctx, mois)?;
    socle::fichiers::noter_valeur(&format!("paies {}", mois), empreinte_des_paies(&valides, mois));
    valides.iter().map(|(i, v)| v.paie(ctx, &data, *i)).collect()
}

/// L'empreinte des paies validees de `mois` telles qu'un calcul les lit (`paies_validees`) ; `None` si elles ne se lisent pas.
pub fn empreinte_des_paies_validees(ctx: &Contexte, mois: &str) -> Option<String> {
    valides_lues(ctx, mois).ok().map(|v| empreinte_des_paies(&v, mois))
}

/// Ce qu'un calcul lit d'une paie validee d'un autre mois : la paie, et le compteur de conges avant son mois.
fn empreinte_des_paies(valides: &[(usize, Valide)], mois: &str) -> String {
    let lu = |p: &Value| json!([p.get("paie"), p.get(CONGES_AVANT)]);
    let paies: Vec<Value> = valides.iter().map(|(_, v)| v.paie_ecrite(mois).map(lu).unwrap_or(Value::Null)).collect();
    socle::fichiers::sha256_hex(Value::Array(paies).to_string().as_bytes())
}

thread_local! {
    /// Les engagements en cours de validation (`fichier`, `rang`) et les paies deja calculees de leurs mois.
    static EN_VALIDATION: RefCell<Option<Vec<(String, usize, Map<String, Value>)>>> = const { RefCell::new(None) };
}

/// UNE VALIDATION EN COURS : les mois d'un engagement se calculent du premier au dernier, et chacun lit ceux d'avant
/// (`paies_validees`) tels qu'elle vient de les calculer. Elle se ferme quand elle tombe.
struct EnValidation(Option<Vec<(String, usize, Map<String, Value>)>>);

impl EnValidation {
    fn ouvrir(o: &Objet) -> EnValidation {
        EnValidation(EN_VALIDATION.with(|e| e.replace(Some(vec![(o.fichier.clone(), o.rang, Map::new())]))))
    }

    fn calculee(&self, mois: &str, paie: &Value) {
        noter_calculee(mois, paie);
    }
}

/// La paie de `mois` que le dernier engagement ouvert dans la validation en cours vient de calculer.
fn noter_calculee(mois: &str, paie: &Value) {
    EN_VALIDATION.with(|e| {
        if let Some((_, _, paies)) = e.borrow_mut().as_mut().and_then(|v| v.last_mut()) {
            paies.insert(mois.to_string(), paie.clone());
        }
    });
}

impl Drop for EnValidation {
    fn drop(&mut self) {
        let avant = self.0.take();
        EN_VALIDATION.with(|e| *e.borrow_mut() = avant);
    }
}

/// UN ENGAGEMENT EN SAISIE SE CALCULE POUR L'ECRAN COMME SA VALIDATION LE CALCULERA : le calcul de `mois` y lit les mois
/// d'avant de chaque engagement en saisie qui les couvre tels que sa validation les calculerait (`calculer`, du premier
/// au dernier, comme `paies_a_valider`), si bien que l'indemnite de conges montree est celle qui sera retenue. Rien ne
/// s'ecrit. Pendant une validation, rien de plus : elle tient deja ses mois. Un mois d'avant qui ne se calcule pas ne
/// compte pas (sa validation le refusera).
pub(crate) struct CommeALaValidation {
    _garde: Option<EnValidation>,
}

impl CommeALaValidation {
    pub(crate) fn ouvrir(ctx: &Contexte, mois: &str) -> CommeALaValidation {
        if EN_VALIDATION.with(|e| e.borrow().is_some()) {
            return CommeALaValidation { _garde: None };
        }
        let Ok(objets) = stock::objets_du_mois(&ctx.racine, mois) else { return CommeALaValidation { _garde: None } };
        let en_saisie: Vec<Objet> = objets.into_iter().filter(|o| !o.est_valide() && o.fichier.as_str() < mois).collect();
        if en_saisie.is_empty() {
            return CommeALaValidation { _garde: None };
        }
        let garde = EnValidation(EN_VALIDATION.with(|e| e.replace(Some(Vec::new()))));
        for o in &en_saisie {
            EN_VALIDATION.with(|e| {
                if let Some(v) = e.borrow_mut().as_mut() {
                    v.push((o.fichier.clone(), o.rang, Map::new()));
                }
            });
            for m in o.mois_couverts(mois).unwrap_or_default().into_iter().filter(|m| m.as_str() < mois) {
                let Ok(p) = calculer(ctx, o, &m) else { break };
                noter_calculee(&m, &p);
            }
        }
        CommeALaValidation { _garde: Some(garde) }
    }
}

/// Ce qui distingue l'etat de la validation en cours (les engagements et les mois deja calcules) : ce que lit
/// `paies_validees` en depend.
pub(crate) fn marque_de_validation() -> String {
    EN_VALIDATION.with(|e| match e.borrow().as_ref() {
        Some(v) => v.iter().map(|(f, r, paies)| format!("{}#{}#{}", f, r, paies.len())).collect::<Vec<_>>().join(";"),
        None => String::new(),
    })
}

/// Les engagements valides de `mois` qui ont une paie ecrite pour lui, avec leur rang a l'ecran ; un engagement en cours
/// de validation y porte les paies qu'elle a deja calculees, et seulement elles. Ce qui est parti n'y change rien (une
/// paie partie se lit comme une paie validee) : le journal des envois ne se lit pas, et un calcul qui lit ces paies ne
/// depend que d'elles (`paies <mois>`), pas de chaque depot.
fn valides_lues(ctx: &Contexte, mois: &str) -> Resultat<Vec<(usize, Valide)>> {
    let en_cours = EN_VALIDATION.with(|e| e.borrow().clone()).unwrap_or_default();
    let mut out = Vec::new();
    for (i, o) in stock::objets_du_mois(&ctx.racine, mois)?.into_iter().enumerate() {
        let v = match en_cours.iter().find(|(f, r, _)| o.fichier == *f && o.rang == *r) {
            Some((_, _, paies)) => {
                let Some(p) = paies.get(mois) else { continue };
                let mut lue = o.clone();
                lue.brut.insert("valide".into(), json!({ "le": "", "paies": { mois: p } }));
                Valide::de(lue)
            }
            None => Valide::de(o),
        };
        if let Some(v) = v.filter(|v| v.paie_ecrite(mois).is_some_and(|p| p.get("paie").is_some())) {
            out.push((i, v));
        }
    }
    Ok(out)
}

/// Les numeros des documents de chaque engagement valide de `mois`, au meme rang que `validees_du_mois` : le numero de
/// son bulletin, le premier N CE de son certificat, et le bulletin que sa paie rectifie, tels que la paie ecrite les porte.
pub fn numeros_des_documents(ctx: &Contexte, mois: &str, departs: &Departs) -> Resultat<Vec<NumerosEcrits>> {
    Ok(tels_qu_ecrits(ctx, mois, departs)?
        .iter()
        .filter_map(Etat::valide)
        .map(|v| NumerosEcrits { bulletin: v.bulletin(mois).map(String::from), premier_ce: v.premier_ce(mois), rectifie: v.rectifie(mois).map(String::from) })
        .collect())
}

/// Ce qu'une paie ecrite porte des documents partis d'elle.
#[derive(Debug, Clone, Default)]
pub struct NumerosEcrits {
    pub bulletin: Option<String>,
    pub premier_ce: Option<i64>,
    pub rectifie: Option<String>,
}

// ------------------------------------------------------------------ calculer et ecrire

/// Le rang de l'engagement `o` parmi ceux que `mois` montre.
fn rang_dans(ctx: &Contexte, o: &Objet, mois: &str) -> Resultat<usize> {
    stock::objets_du_mois(&ctx.racine, mois)?
        .iter()
        .position(|x| x.fichier == o.fichier && x.rang == o.rang)
        .ok_or_else(|| format!("{} ne touche pas {}", qui_de(ctx, o), mois))
}

/// Le prelevement a la source de la paie `p`, au taux que la DGFiP a transmis et qui vaut le jour de son paiement
/// (`droit::pas::taux_dgfip`), sinon a la grille de l'article 204 H ; `None` pour un mois sans paie. Un bareme sans
/// grille refuse le calcul : un prelevement ne se devine pas (regle 1), et aucun document ne le calcule a la place.
pub(crate) fn prelevement(ctx: &Contexte, p: &Paie) -> Resultat<Option<droit::pas::Versement>> {
    let e = &p.engagement;
    if e.sans_paie() {
        return Ok(None);
    }
    let cle = en_texte(p.salarie.get("_cle").ok_or_else(|| socle::fichiers::absent("_cle"))?);
    let fin = e.fin_periode()?;
    // Le taux qui vaut le jour du paiement (CGI, art. 204 A, 2, 1° : la retenue est faite « lors du paiement ») ; sans
    // date de paiement dite, la fin de la periode.
    let paye_le = e.date_paiement.as_deref().and_then(|d| Jour::lire(d.get(..10).unwrap_or(d)).ok()).unwrap_or(fin);
    let taux = droit::pas::taux_dgfip(ctx, &cle, Some(paye_le))?;
    let bareme = ctx.bareme(&Jour { an: fin.an, mois: fin.mois, jour: 1 })?;
    let t = |j: Jour| (j.an, j.mois, j.jour);
    let court = droit::natures::contrat_court(&*e.nat()?, t(e.debut), e.fin.map(t), t(fin));
    droit::pas::du_versement(taux.as_ref(), &bareme, p.net_imposable, Some(fin), court, droit::pas::ZONE_DEFAUT)
        .map(Some)
        .map_err(|e| e.message().to_string())
}

/// Le prelevement a la source du versement d'une correction, verse avec le mois `mois_du_versement` : la difference de
/// net imposable entre la paie corrigee `apres` et ce qui a ete declare (`net_imposable_declare`).
///
/// BOFiP, BOI-IR-PAS-30-10-50 (`atelier/reference/sources/bofip/BOI-IR-PAS-30-10-50-extrait-regularisation.md`) :
/// - § 1 : un rappel de paie « ne donne pas lieu a regularisation, mais doit etre soumis au prelevement a la source et
///   porte dans la declaration souscrite au titre du mois au cours duquel est intervenu le versement du rappel » : la
///   difference qui monte se preleve comme un versement de ce mois-la, datee de sa fin (S21.G00.50.001), au taux DGFiP
///   qui vaut alors, sinon a la grille de l'article 204 H, avec l'abattement d'un contrat court ;
/// - § 90 : un trop-verse qui ne se compense pas (le versement de la correction est negatif) se regularise « au taux de
///   prelevement initialement utilise » : `taux_origine`, le taux du versement declare (S21.G00.56.006). Sans lui, pas
///   de prelevement (`None`) : la DSN refuse, le bulletin le dit. Son versement porte un montant soumis nul (fiche
///   1810, `droit::pas::du_versement`).
///
/// Un bareme sans grille refuse, comme pour une paie (regle 1).
pub fn prelevement_de_la_correction(
    ctx: &Contexte, apres: &Paie, net_imposable_declare: Decimal, taux_origine: Option<&str>, mois_du_versement: &str,
) -> Resultat<PrelevementDeLaCorrection> {
    let debut = Jour::lire(&format!("{}-01", mois_du_versement))?;
    let fin = Jour::nouveau(debut.an, debut.mois, socle::date::jours_du_mois(debut.an, debut.mois))
        .ok_or_else(|| socle::date::date_impossible(debut.an, debut.mois, 1))?;
    let difference = socle::num::sub(apres.net_imposable, net_imposable_declare);
    let e = &apres.engagement;
    let cle = en_texte(apres.salarie.get("_cle").ok_or_else(|| socle::fichiers::absent("_cle"))?);
    let taux = droit::pas::taux_dgfip(ctx, &cle, Some(fin))?;
    let bareme = ctx.bareme(&debut)?;
    let t = |j: Jour| (j.an, j.mois, j.jour);
    let court = droit::natures::contrat_court(&*e.nat()?, t(e.debut), e.fin.map(t), t(fin));
    let versement = droit::pas::du_versement(taux.as_ref(), &bareme, difference, Some(fin), court, droit::pas::ZONE_DEFAUT)
        .map_err(|e| e.message().to_string())?;
    let prelevement = if difference < Decimal::ZERO {
        match taux_origine.filter(|t| !t.is_empty()) {
            Some(t) => Some(pct(difference, dec_avec_echelle(&Value::String(t.to_string()))?)),
            None => None,
        }
    } else {
        Some(pct(versement.montant_soumis, dec_avec_echelle(&versement.taux)?))
    };
    Ok(PrelevementDeLaCorrection { difference, versement, prelevement })
}

/// Ce que `prelevement_de_la_correction` rend.
pub struct PrelevementDeLaCorrection {
    /// La difference de net imposable (S21.G00.50.002 du versement de la correction).
    pub difference: Decimal,
    /// Le taux, son type, son identifiant et le montant soumis du versement (S21.G00.50.006 a .008, .013).
    pub versement: droit::pas::Versement,
    /// Ce que la correction preleve (positif) ou rend (negatif) ; `None` : un trop-verse sans taux d'origine.
    pub prelevement: Option<Decimal>,
}

impl PrelevementDeLaCorrection {
    /// Un trop-verse : la difference baisse.
    pub fn indu(&self) -> bool {
        self.difference < Decimal::ZERO
    }
}

/// La paie de l'engagement `o` pour `mois`, calculee maintenant : `{"paie", "fiches", "pas", "empreinte"}`.
pub(crate) fn calculer(ctx: &Contexte, o: &Objet, mois: &str) -> Resultat<Value> {
    let i = rang_dans(ctx, o, mois)?;
    calculer_les_rangs(ctx, mois, &[i])?.pop().ok_or_else(|| "le calcul du mois n'a pas cette paie".to_string())
}

/// Les paies des engagements de rangs `rangs` dans l'ecran de `mois`, d'un seul calcul du mois : chacune
/// `{"paie", "fiches", "saisie_du_mois", "pas", "empreinte"}`, dans l'ordre de `rangs`. Le calcul d'un mois lit tous ses engagements (plafonds,
/// cumuls, conges) : il se fait une fois, et chaque paie en garde l'empreinte et les fiches du salarie telles qu'il les a lues.
/// Ce que la saisie `data` d'un mois pose pour tous ses engagements : elle-meme, sans ses engagements ni ce que le moteur
/// tient (comme `stock::empreinte_de_saisie` la separe).
fn mois_lu(data: &Value) -> Value {
    let mut m = data.as_object().cloned().unwrap_or_default();
    for k in ["engagements", "_origine", "numeros_figes", "mois"] {
        m.remove(k);
    }
    Value::Object(m)
}

///
/// UNE PAIE ECRITE NE LIT QUE DES ENGAGEMENTS VALIDES : le calcul se fait sur les engagements valides du mois et ceux
/// de `rangs`, comptes comme valides (`stock::empreinte_de_saisie` ne compte que ceux-la) ; un engagement en saisie
/// n'entre ni dans le calcul ni dans l'empreinte d'une paie ecrite.
fn calculer_les_rangs(ctx: &Contexte, mois: &str, rangs: &[usize]) -> Resultat<Vec<Value>> {
    let mut data = stock::vue_du_mois(&ctx.racine, mois)?.ok_or_else(|| format!("{} : aucune saisie", mois))?;
    let tous: Vec<Value> = data.get("engagements").and_then(Value::as_array).cloned().unwrap_or_default();
    let mut gardes = Vec::new();
    let mut places = vec![None; tous.len()];
    for (i, mut e) in tous.into_iter().enumerate() {
        if rangs.contains(&i) && e.get("valide").is_none() {
            e["valide"] = json!({});
        }
        if e.get("valide").is_some() {
            places[i] = Some(gardes.len());
            gardes.push(e);
        }
    }
    data["engagements"] = Value::Array(gardes);
    let (calculees, entrees) = empreinte::suivre(ctx, &data, || {
        let s = crate::charger_donnees(ctx, &data)?;
        rangs
            .iter()
            .map(|i| {
                let p = places.get(*i).copied().flatten().and_then(|j| s.paies.get(j)).ok_or("le calcul du mois n'a pas cette paie")?;
                let conges = crate::conges::compteur_ecrit(ctx, p, mois)?;
                let config = empreinte::configuration(ctx, &p.engagement.fin_periode()?, &p.engagement.salarie, p.engagement.spectacle.as_deref())?;
                Ok((relire::ecrire(ctx, p)?, relire::fiches_du_calcul(p), prelevement(ctx, p)?.map(|v| v.canon()), conges, p.engagement.fin_periode()?, config))
            })
            .collect::<Resultat<Vec<_>>>()
    })?;
    let mois_lu = mois_lu(&data);
    calculees
        .into_iter()
        .map(|(paie, fiches, pas, conges, fin, config)| {
            let regles = empreinte::regles(ctx, &fin)?;
            let mut v = json!({ "paie": paie, "fiches": fiches, MOIS_LU: mois_lu.clone(), "empreinte": empreinte::en_json(&entrees, &regles, &config) });
            if let Some(pas) = pas {
                v["pas"] = pas;
            }
            if let Some((c, avant)) = conges {
                v[CONGES] = c.en_json();
                v[CONGES_AVANT] = avant.en_json();
            }
            Ok(v)
        })
        .collect()
}

/// Les chiffres d'une paie ecrite : tout sauf ce qu'elle dit en mots.
fn chiffres(v: &Value) -> Value {
    let champ = |k: &str| v.get(k).cloned().unwrap_or(Value::Null);
    let mut x = json!({ "paie": champ("paie"), "pas": champ("pas"), CONGES: champ(CONGES) });
    fn sans_avis(v: &mut Value) {
        if let Some(m) = v.as_object_mut() {
            m.remove("avertissements");
            for (_, x) in m.iter_mut() {
                sans_avis(x);
            }
        } else if let Some(a) = v.as_array_mut() {
            a.iter_mut().for_each(sans_avis);
        }
    }
    sans_avis(&mut x["paie"]);
    sans_avis(&mut x[CONGES]);
    x
}

/// Pourquoi la paie a change de `avant` a `apres` : son prelevement seul, une regle datee nouvelle, la saisie d'un autre
/// engagement ou du mois, la paie d'un autre mois, une valeur datee de la compagnie, le code qui calcule (le moteur ou ce
/// qu'il livre), ou la configuration de la compagnie.
///
/// UNE SEULE RAISON, LA MEME QUELLE QUE SOIT L'HEURE DE LA LECTURE. Une paie dont l'empreinte ne tient plus sans qu'un
/// chiffre change renouvelle son empreinte sans rien dire (`rafraichir`) : selon que l'ecran a lu le mois entre deux
/// gestes ou non, l'empreinte ecrite porte deja le code present ou encore l'ancien. Ce que la compagnie a fait (une
/// saisie, une paie d'un autre mois, une valeur datee) dit donc pourquoi avant le code, qui ne se nomme que lorsque rien
/// d'autre n'a change. Une paie partie (`parti`) ne bouge que par une decision de l'editeur : le code et les regles
/// datees s'y nomment d'abord, puisque ce sont eux qui la refont.
fn raison(avant: &Value, apres: &Value, parti: bool) -> String {
    let (a, b) = (chiffres(avant), chiffres(apres));
    if a["paie"] == b["paie"] && a[CONGES] == b[CONGES] && a["pas"] != b["pas"] {
        return TAUX_DE_PAS.to_string();
    }
    let vieilles = empreinte::regles_de(avant.get("empreinte").unwrap_or(&Value::Null));
    let nouvelles: Vec<String> = empreinte::regles_de(apres.get("empreinte").unwrap_or(&Value::Null)).into_iter().filter(|r| !vieilles.contains(r)).collect();
    if let Some(r) = nouvelles.iter().max() {
        let jour = Jour::lire(r).map(|j| j.jj_mm_aaaa()).unwrap_or_else(|_| r.clone());
        return format!("règle du {}", jour);
    }
    let entrees = |v: &Value| v.get("empreinte").and_then(empreinte::entrees_de).unwrap_or_default();
    let (ea, eb) = (entrees(avant), entrees(apres));
    let changees: Vec<&String> = ea.keys().chain(eb.keys()).filter(|k| ea.get(*k) != eb.get(*k)).collect();
    let code = changees.iter().any(|k| *k == "moteur" || k.split_once(' ').is_some_and(|(_, c)| c.starts_with("code:")));
    if code && parti {
        return CALCUL_CORRIGE.to_string();
    }
    // La saisie d'un mois : `<le mois>:<ses engagements>` (`stock::empreinte_de_saisie`).
    let saisies: Vec<&String> = changees.iter().copied().filter(|k| k.starts_with("saisie ")).collect();
    let part = |e: &BTreeMap<String, String>, k: &str, i: usize| e.get(k).and_then(|v| v.split(':').nth(i).map(String::from));
    if saisies.iter().any(|k| part(&ea, k, 1) != part(&eb, k, 1)) {
        return AUTRE_ENGAGEMENT.to_string();
    }
    if !saisies.is_empty() {
        return SAISIE_DU_MOIS.to_string();
    }
    // Les paies validees d'un autre mois que le calcul lit (conges, cumuls) : `paies <le mois>` (`paies_validees`).
    let mut mois: Vec<&str> = changees.iter().filter_map(|k| k.strip_prefix("paies ")).collect();
    mois.sort();
    if let Some(m) = mois.first() {
        return format!("la paie {} a changé", de_mois(m));
    }
    // La configuration de la compagnie : une valeur datee de ses fiches qui s'applique, ou son bareme de l'annee
    // (`config/baremes/<annee>.json` : le taux AT/MP, qui vaut pour toute l'annee).
    let config_avant = empreinte::configuration_de(avant.get("empreinte").unwrap_or(&Value::Null));
    let config_nouvelle = empreinte::configuration_de(apres.get("empreinte").unwrap_or(&Value::Null)).into_iter().filter(|r| !config_avant.contains(r)).max();
    let annee_du_bareme = changees.iter().find_map(|k| {
        k.split_once(' ').and_then(|(_, c)| c.strip_prefix("racine:config/baremes/")).and_then(|f| f.strip_suffix(".json")).filter(|an| an.len() == 4 && an.bytes().all(|b| b.is_ascii_digit()))
    });
    if let Some(jour) = config_nouvelle.or_else(|| annee_du_bareme.map(|an| format!("{}-01-01", an))) {
        return format!("règle du {}", Jour::lire(&jour).map(|j| j.jj_mm_aaaa()).unwrap_or(jour));
    }
    if code {
        return CALCUL_CORRIGE.to_string();
    }
    CONFIGURATION.to_string()
}

/// « de septembre 2026 », « d'octobre 2026 ».
fn de_mois(mois: &str) -> String {
    let nom = socle::date::libelle_du_mois_ou_tel_quel(mois, socle::date::Graphie::Accentuee);
    if nom.starts_with(['a', 'o']) {
        format!("d'{}", nom)
    } else {
        format!("de {}", nom)
    }
}

/// Le moment d'un geste, a l'heure du moteur.
fn maintenant() -> String {
    socle::paris::iso_secondes(socle::temps::secondes())
}


/// La saisie d'un engagement telle qu'une version la garde : l'objet, sans ce que le moteur tient (sa suite, qui est de
/// la saisie, y reste).
fn saisie_de(o: &Objet) -> Value {
    Value::Object(
        o.brut
            .iter()
            .filter(|(k, _)| k.as_str() == stock::SUITE || !stock::TENUS_PAR_LE_MOTEUR.contains(&k.as_str()))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect(),
    )
}

/// Ce qu'une paie ecrite a lu hors de la saisie de son engagement : la fiche du salarie (`fiches`) et la saisie du mois
/// (`MOIS_LU`). Ce qui a ete declare se relit dessus (`paie_declaree`) : qu'il change, et la version change.
fn lu(p: &Value) -> (Option<&Value>, Option<&Value>) {
    (p.get("fiches"), p.get(MOIS_LU))
}

/// Les paies `b` (`{<mois>: <paie ecrite>}`) disent-elles, pour chaque mois de `a`, les memes chiffres, lus sur les memes
/// fiches et la meme saisie du mois ? Un mois que `b` ajoute (un contrat sans terme qui entre dans un mois de plus) ne
/// change rien de ce que disait `a`.
fn memes_chiffres(a: Option<&Value>, b: Option<&Value>) -> bool {
    let vide = Map::new();
    let (a, b) = (a.and_then(Value::as_object).unwrap_or(&vide), b.and_then(Value::as_object).unwrap_or(&vide));
    a.iter().all(|(m, p)| b.get(m).is_some_and(|q| chiffres(p) == chiffres(q) && lu(p) == lu(q)))
}

/// LA VERSION QUE PREND LE PRESENT quand la validation `actuelle` de `o` est remplacee le `le` : si un envoi est parti
/// sur elle (`siens`), elle se garde, datee, dans les versions de l'engagement, et le present prend le numero suivant ;
/// sinon le present garde son numero `n`, sans historique.
fn nouvelle_version(o: &mut Objet, actuelle: &Value, n: u64, siens: &[Depart], le: &str) -> u64 {
    if !siens.iter().any(|d| d.version == n) {
        return n;
    }
    let passee = json!({
        "version": n, "valide_le": actuelle.get("le").cloned().unwrap_or(Value::Null), "remplacee_le": le,
        "saisie": actuelle["saisie"], "paies": actuelle["paies"],
    });
    match o.brut.get_mut(VERSIONS).and_then(Value::as_array_mut) {
        Some(v) => v.push(passee),
        None => {
            o.brut.insert(VERSIONS.into(), json!([passee]));
        }
    }
    n + 1
}

/// « VALIDER L'ENGAGEMENT » : l'engagement de rang `rang` dans l'ecran de `mois` ecrit ses paies, une par mois qu'il
/// couvre (jusqu'au mois de l'ecran pour un contrat sans terme), chacune avec son empreinte, et sa saisie telle qu'elle
/// est. La paie d'un mois dont les documents sont partis se reprend de la validation d'avant (`precedente`) si la saisie
/// de son mois n'a pas change. Une validation qui change ce qu'un envoi a emporte fait une version (`nouvelle_version`) ;
/// une validation qui ne change rien garde le numero. Refus : un mois qui ne se calcule pas ; rien n'est ecrit.
pub fn valider(ctx: &Contexte, mois: &str, rang: usize, departs: &Departs) -> Resultat<Valide> {
    let mut o = stock::objet_du_mois(&ctx.racine, mois, rang)?.ok_or_else(|| format!("{} : pas d'engagement de rang {}", mois, rang + 1))?;
    // La saisie que l'ecran montre lui fait manquer un fait (`stock::A_COMPLETER`) : c'est elle qui se valide, et elle
    // ne se calcule pas. Le refus est ce qui manque.
    if let Some(manque) = o.a_completer() {
        return Err(manque);
    }
    // Le numero du contrat d'abord : la paie de chaque mois le porte, et le calcul lit l'engagement ecrit. Un calcul
    // refuse rend l'engagement tel qu'il etait.
    let avant = o.clone();
    let le = maintenant();
    crate::numeros::a_la_validation(ctx, &mut o, &le, departs)?;
    let numerote = o.brut != avant.brut;
    if numerote {
        stock::reecrire(&ctx.racine, &o)?;
    }
    let siens = ses_departs(ctx, &o, departs).to_vec();
    // Une validation qui ne dit pas sa version, sa saisie ou ce que ses paies ont lu (`a_revalider`) ne se reprend pas :
    // celle-ci s'ecrit entiere, en version 1.
    let precedente = o.brut.get(PRECEDENTE).or_else(|| o.brut.get("valide")).filter(|p| a_revalider(ctx, &o, p).is_none()).cloned();
    let paies = match paies_a_valider(ctx, &o, mois, precedente.as_ref(), &siens) {
        Ok(p) => p,
        Err(e) => {
            if numerote {
                stock::reecrire(&ctx.racine, &avant)?;
            }
            return Err(e);
        }
    };
    let saisie = saisie_de(&o);
    let paies = Value::Object(paies);
    // Une validation qui ne change rien garde sa version et la date de celle-ci : la version est la meme.
    let (version, le) = match precedente.as_ref().and_then(|p| Some((p, version_de(Some(p))?))) {
        Some((p, n)) if p.get("saisie") != Some(&saisie) || !memes_chiffres(p.get("paies"), Some(&paies)) => (nouvelle_version(&mut o, p, n, &siens, &le), le),
        Some((p, n)) => (n, p.get("le").and_then(Value::as_str).map_or(le, String::from)),
        None => (1, le),
    };
    o.brut.remove(PRECEDENTE);
    o.brut.insert("valide".into(), json!({ "le": le, "version": version, "saisie": saisie, "paies": paies }));
    stock::reecrire(&ctx.racine, &o)?;
    Valide::de(o).ok_or_else(|| "validation illisible".to_string())
}

/// Les paies que la validation de `o` ecrit, une par mois qu'il couvre jusqu'a `mois`. `precedente` : la validation
/// qu'elle remplace ; `siens` : ce qui est parti de l'engagement.
fn paies_a_valider(ctx: &Contexte, o: &Objet, mois: &str, precedente: Option<&Value>, siens: &[Depart]) -> Resultat<Map<String, Value>> {
    let anciennes = precedente.and_then(|p| p.get("paies")).and_then(Value::as_object).cloned().unwrap_or_default();
    let mut paies = Map::new();
    let en_cours = EnValidation::ouvrir(o);
    for m in o.mois_couverts(mois)? {
        let partis = documents_partis(siens, &m).is_some();
        let garde = anciennes.get(&m).filter(|f| {
            let saisie = stock::empreinte_de_la_saisie_du_mois(&ctx.racine, &m).ok().flatten();
            let ecrite = f.get("empreinte").and_then(empreinte::entrees_de).and_then(|e| e.get(&stock::cle_de_saisie(&m)).cloned());
            partis && saisie.is_some() && saisie == ecrite
        });
        let p = match garde {
            Some(f) => f.clone(),
            None => {
                let mut p = calculer(ctx, o, &m)?;
                // Une paie partie, refaite : son bulletin sera un rectificatif de celui qui est parti.
                if partis {
                    if let Some(parti) = anciennes.get(&m).and_then(|f| f.get(BULLETIN).or_else(|| f.get(RECTIFIE))).filter(|b| b.is_string()) {
                        p[RECTIFIE] = parti.clone();
                    }
                }
                p
            }
        };
        en_cours.calculee(&m, &p);
        paies.insert(m, p);
    }
    Ok(paies)
}
/// L'engagement de rang `rang` de l'ecran de `mois`, rouvert (`modifier`), est-il a revalider ? Sa saisie dit autre
/// chose que sa validation mise de cote, ou celle-ci ne dit pas tout ce qu'un envoi emporte (`a_revalider`). Faux pour un
/// engagement qui n'a pas ete rouvert.
pub fn a_revalider_rouvert(ctx: &Contexte, mois: &str, rang: usize) -> Resultat<bool> {
    let Some(o) = stock::objet_du_mois(&ctx.racine, mois, rang)? else { return Ok(false) };
    let Some(p) = o.brut.get(PRECEDENTE) else { return Ok(false) };
    Ok(p.get("saisie") != Some(&saisie_de(&o)) || a_revalider(ctx, &o, p).is_some())
}

/// « A PARTIR DE QUEL MOIS ? » : ce que le moteur demande avant de revalider un contrat paye au mois (CDD, CDI) de
/// plusieurs mois dont un mois est parti (ses documents ou sa DSN), quand ce que le contrat dit (`stock::RECONDUITS` :
/// salaire, duree du travail, emploi...) change dans un de ses mois. `partis` : les mois partis ; `choix` : les mois
/// d'ou le changement peut valoir, du premier ou il change au dernier que la validation ecrit.
#[derive(Debug, Clone)]
pub struct APartirDu {
    pub qui: String,
    pub partis: Vec<String>,
    pub choix: Vec<String>,
}

impl APartirDu {
    /// La question, en mots ; l'avertissement nomme l'engagement (`qui`).
    pub fn phrase(&self) -> String {
        let partis: Vec<String> = self.partis.iter().map(|m| crate::libelle_mois(m)).collect();
        format!(
            "Les documents ou la DSN de {} sont déjà partis. À partir de quel mois ce changement vaut-il ? Les mois \
d'avant gardent leurs chiffres ; à partir du mois choisi, la paie prend le nouveau chiffre, et l'avenant le dit à compter \
du jour dit, le 1er si rien d'autre n'est dit. Un changement en cours de mois coupe ce mois en deux : l'ancien chiffre \
jusqu'à la veille, le nouveau à partir de ce jour, chaque part payée pour ses heures.",
            partis.join(", ")
        )
    }
}

/// La question « a partir de quel mois ? » pour l'engagement de rang `rang` de l'ecran de `mois` (voir `APartirDu`) ;
/// `None` quand le fait ne l'appelle pas : un contrat d'usage, un engagement d'un seul mois, jamais valide, dont rien
/// n'est parti, dont les termes ne changent pas, ou dont les bornes changent (le contrat entier change).
pub fn question_a_partir_du(ctx: &Contexte, mois: &str, rang: usize, departs: &Departs) -> Resultat<Option<APartirDu>> {
    let Some(o) = stock::objet_du_mois(&ctx.racine, mois, rang)? else { return Ok(None) };
    let Some((ancien, partis, couverts)) = contrat_au_mois_parti(ctx, &o, &o, mois, departs)? else { return Ok(None) };
    let mut premier = None;
    for m in &couverts {
        if o.termes_du_mois(m)? != ancien.termes_du_mois(m)? {
            premier = Some(m.clone());
            break;
        }
    }
    let Some(premier) = premier else { return Ok(None) };
    let choix = couverts.into_iter().filter(|m| *m >= premier).collect();
    Ok(Some(APartirDu { qui: qui_de(ctx, &o), partis, choix }))
}

/// L'engagement `ecrit` (rouvert, tel qu'il est ecrit) devenu `nouveau` : un contrat paye au mois, avant et apres, dont
/// le salarie et les bornes ne changent pas (`stock::PARTAGES`), dont un mois est parti (documents ou DSN) et qui couvre
/// au moins deux mois. Rend ce que sa validation disait, les mois partis et les mois couverts ; `None` sinon.
fn contrat_au_mois_parti(ctx: &Contexte, ecrit: &Objet, nouveau: &Objet, mois: &str, departs: &Departs) -> Resultat<Option<(Objet, Vec<String>, Vec<String>)>> {
    let Some(ancien) = ecrit.brut.get(PRECEDENTE).and_then(|p| p.get("saisie")).and_then(Value::as_object).map(|s| ecrit.d_apres_la_saisie(s)) else {
        return Ok(None);
    };
    if !nouveau.mensuel() || !ancien.mensuel() {
        return Ok(None);
    }
    if stock::PARTAGES.iter().any(|k| nouveau.brut.get(*k).map(stock::canonique) != ancien.brut.get(*k).map(stock::canonique)) {
        return Ok(None);
    }
    let mut partis: Vec<String> =
        ses_departs(ctx, ecrit, departs).iter().filter(|d| matches!(d.envoi, Envoi::Documents | Envoi::Dsn)).filter_map(|d| d.mois.clone()).collect();
    partis.sort();
    partis.dedup();
    if partis.is_empty() {
        return Ok(None);
    }
    // Un contrat sans terme court au moins jusqu'au mois d'aujourd'hui.
    let couverts = nouveau.mois_couverts(&socle::temps::jour_de_paris().mois_iso().max(mois.to_string()))?;
    if couverts.len() < 2 {
        return Ok(None);
    }
    Ok(Some((ancien, partis, couverts)))
}

/// La reponse : le changement vaut a partir de `depuis` (`stock::Objet::a_partir_du`), le jour `le` de ce mois quand il
/// est dit (le 1er sinon), ecrit dans l'engagement. Refus : la question ne se pose pas, `depuis` n'est pas un de ses
/// choix, ou le jour n'est pas un jour du contrat dans ce mois ; rien n'est ecrit.
pub fn poser_a_partir_du(ctx: &Contexte, mois: &str, rang: usize, depuis: &str, le: Option<u32>, departs: &Departs) -> Resultat<()> {
    let q = question_a_partir_du(ctx, mois, rang, departs)?.ok_or_else(|| "Rien ne change d'un mois à l'autre dans cet engagement : la question « à partir de quel mois ? » ne se pose pas. Rien n'est écrit.".to_string())?;
    if !q.choix.iter().any(|m| m == depuis) {
        return Err(format!(
            "{} : « à partir de {} » n'est pas un des mois possibles ({}). Rien n'est écrit.",
            q.qui,
            depuis,
            q.choix.iter().map(|m| crate::libelle_mois(m)).collect::<Vec<_>>().join(", ")
        ));
    }
    let o = stock::objet_du_mois(&ctx.racine, mois, rang)?.ok_or_else(|| format!("{} : pas d'engagement de rang {}", mois, rang + 1))?;
    let jour = match le {
        None => None,
        Some(n) => {
            let premier = socle::date::premier_du_mois(depuis)?;
            let j = socle::date::Jour::nouveau(premier.an, premier.mois, n).ok_or_else(|| {
                format!("{} : « le {} {} » n'est pas un jour de ce mois. Rien n'est écrit.", q.qui, n, crate::libelle_mois(depuis))
            })?;
            let debut = socle::date::d(o.brut.get("debut").unwrap_or(&Value::Null))?;
            let fin = o.brut.get("fin").filter(|f| vrai(f)).map(socle::date::d).transpose()?;
            if j < debut || fin.is_some_and(|f| j > f) {
                return Err(format!(
                    "{} : le contrat court du {}{} ; un changement au {} n'en est pas un jour. Rien n'est écrit.",
                    q.qui,
                    debut.jj_mm_aaaa(),
                    fin.map(|f| format!(" au {}", f.jj_mm_aaaa())).unwrap_or_default(),
                    j.jj_mm_aaaa()
                ));
            }
            // Le premier jour du contrat dans le mois : le changement couvre tout ce que le mois en paie.
            (j > debut).then_some(j).or(Some(premier))
        }
    };
    let saisie = o.brut.get(PRECEDENTE).and_then(|p| p.get("saisie")).and_then(Value::as_object).cloned().unwrap_or_default();
    let r = o.a_partir_du(&o.d_apres_la_saisie(&saisie), depuis, jour)?;
    stock::reecrire(&ctx.racine, &r)?;
    Ok(())
}

/// LE MOIS OU LE CONTRAT CHANGE entre la version `passee` d'un engagement valide (sa saisie gardee) et son present : le
/// premier mois dont les clauses (`CLAUSES`) different. C'est de ce mois que l'avenant prend effet (R1221-40 : « au plus
/// tard a la date de prise d'effet de cette modification », `atelier/reference/sources/code-du-travail/
/// articles-20261029.md`, l. 856). `None` : aucun mois ne change.
pub fn mois_du_changement(v: &Valide, passee: &Value) -> Resultat<Option<String>> {
    let Some(saisie) = passee.get("saisie").and_then(Value::as_object) else { return Ok(None) };
    let ancien = v.objet.d_apres_la_saisie(saisie);
    let mensuel = v.objet.mensuel();
    let mut mois: Vec<String> = [&v.objet.brut.get("valide").cloned().unwrap_or(Value::Null), passee]
        .iter()
        .filter_map(|x| x.get("paies").and_then(Value::as_object))
        .flat_map(|p| p.keys().cloned())
        .chain(v.objet.mois_saisis())
        .chain(ancien.mois_saisis())
        .collect();
    mois.sort();
    mois.dedup();
    let garder = |x: Option<Map<String, Value>>| -> Option<Map<String, Value>> {
        x.map(|m| m.into_iter().filter(|(k, _)| CLAUSES.contains(&k.as_str()) && !(mensuel && k.as_str() == "lignes")).collect())
    };
    for m in mois {
        if garder(v.objet.vue(&m)?) != garder(ancien.vue(&m)?) {
            return Ok(Some(m));
        }
    }
    Ok(None)
}

/// « MODIFIER… » : l'engagement repasse en saisie. Sa validation se garde (`precedente`) pour la suivante.
pub fn modifier(ctx: &Contexte, mois: &str, rang: usize) -> Resultat<EnSaisie> {
    let mut o = stock::objet_du_mois(&ctx.racine, mois, rang)?.ok_or_else(|| format!("{} : pas d'engagement de rang {}", mois, rang + 1))?;
    if let Some(v) = o.brut.remove("valide") {
        o.brut.insert(PRECEDENTE.into(), v);
        stock::reecrire(&ctx.racine, &o)?;
    }
    Ok(EnSaisie { objet: o })
}

// ------------------------------------------------------------------ supprimer, ou ne plus etre actif

/// CE QUI EST PARTI de l'engagement `o`, dit en mots (« la DPAE, le 02/03/2026 ») : ce que la couche des envois dit
/// (`departs`), et une version gardee parce qu'un envoi l'avait emportee. Vide : rien n'est parti, il se supprime.
pub fn ce_qui_est_parti(ctx: &Contexte, o: &Objet, departs: &Departs) -> Vec<String> {
    let jour = |le: &str| Jour::lire(le.get(..10).unwrap_or("")).map(|j| j.jj_mm_aaaa()).unwrap_or_else(|_| le.to_string());
    let mut out: Vec<String> = ses_departs(ctx, o, departs)
        .iter()
        .map(|d| format!("{}, le {}", crate::versions::quoi_du_depart(d), jour(&d.le)))
        .collect();
    out.dedup();
    if out.is_empty() && o.brut.get(VERSIONS).and_then(Value::as_array).is_some_and(|v| !v.is_empty()) {
        out.push("une version qu'un envoi a emportee".to_string());
    }
    out
}

/// SUPPRIMER L'ENGAGEMENT `o` (celui qu'une carte montre, en saisie ou valide) : tant que rien n'est parti de lui
/// (`ce_qui_est_parti` vide), il s'efface, meme valide, et rend son numero (`numeros` : le suivant se tire de ce que portent les
/// engagements et ce qui est parti). Refus des qu'un envoi est parti : il ne se supprime plus, il peut ne plus etre
/// actif (`desactiver`) ; rien n'est ecrit.
pub fn supprimer(ctx: &Contexte, o: &Objet, departs: &Departs) -> Resultat<()> {
    let partis = ce_qui_est_parti(ctx, o, departs);
    if !partis.is_empty() {
        return Err(format!(
            "{} : déjà parti : {}. Il ne se supprime plus ; il peut devenir non actif : il reste dans la compagnie, avec ses versions, et ne compte plus dans aucune paie ni aucune déclaration. Rien n'est effacé.",
            qui_de(ctx, o),
            partis.join(" ; "),
        ));
    }
    stock::effacer(&ctx.racine, o)?;
    Ok(())
}

/// Un engagement qui n'est plus actif (`stock::NON_ACTIF`) : il garde ses versions, ne se calcule plus, n'entre dans
/// aucun document ni aucune declaration ; il se lit.
#[derive(Debug, Clone)]
pub struct NonActif {
    objet: Objet,
    le: String,
    version: u64,
}

impl NonActif {
    /// L'engagement non actif `o`, tel qu'il est ecrit ; `None` s'il est actif.
    pub fn de(objet: Objet) -> Option<NonActif> {
        let na = objet.brut.get(stock::NON_ACTIF)?;
        let le = na.get("le").map(en_texte)?;
        let version = na.get("version").and_then(Value::as_u64)?;
        Some(NonActif { objet, le, version })
    }

    /// Sa version : celle qui suit la derniere validee. Son present est : rien de paye ; une correction qui le declare
    /// part sur elle.
    pub fn version(&self) -> u64 {
        self.version
    }

    pub fn objet(&self) -> &Objet {
        &self.objet
    }

    /// Depuis quand.
    pub fn le(&self) -> &str {
        &self.le
    }

    /// CE QUE SA DESACTIVATION REND FAUX, tire de ce qui est parti (`departs`) comparé a son present, qui est : rien.
    /// Chaque mois que la derniere DSN qui l'a declare a declare sur une autre version que la sienne est a corriger
    /// (`Dsn`) : la couche des declarations dit comment (l'annule-et-remplace du mois qui ne le porte plus, ou la
    /// correction qui annule ce qui a ete declare). Un contrat, des documents, un signalement partis ne demandent rien.
    /// Une DPAE partie non plus : aucun texte ne l'annule, une DPAE se corrige par une nouvelle declaration (fiche
    /// net-entreprises 3300, `atelier/reference/sources/net-entreprises-base-connaissances/
    /// net-entreprises-fiche3300-dpae-en-dsn-20261015.txt`, l. 451 ; guide de l'API DPAE,
    /// `atelier/reference/norme-dpae/5491-API-DPAE-Fonctionnement.pdf`, p. 3 : pas d'annule-et-remplace).
    pub fn a_refaire(&self, ctx: &Contexte, departs: &Departs) -> Vec<ARefaire> {
        let siens = ses_departs(ctx, &self.objet, departs);
        let mut out = Vec::new();
        let mut mois: BTreeMap<String, u64> = BTreeMap::new();
        for d in siens.iter().filter(|d| d.envoi == Envoi::Dsn) {
            if let Some(m) = &d.mois {
                mois.insert(m.clone(), d.version);
            }
        }
        out.extend(mois.into_iter().filter(|(_, depuis)| *depuis != self.version).map(|(mois, depuis)| ARefaire::Dsn { mois, depuis }));
        out
    }

    /// LA CORRECTION de `mois` que la DSN de `porteur` doit porter pour lui : ce qui a ete declare pour ce mois par les
    /// DSN anterieures a `porteur` (`paies_declarees`), quand la derniere ne l'a pas declare non actif ; `None` sinon.
    pub fn correction_due(&self, ctx: &Contexte, departs: &Departs, mois: &str, porteur: &str) -> Resultat<Option<Vec<Declaree>>> {
        let declarees = paies_declarees(ctx, &self.objet, departs, mois, porteur)?;
        let due = declarees.last().is_some_and(|d| d.version != self.version);
        Ok(due.then_some(declarees))
    }
}

/// « NE PLUS ETRE ACTIF » : l'engagement de rang `rang` dans l'ecran de `mois`, dont quelque chose est parti, garde sa
/// validation parmi ses versions (datee du `le` de la desactivation) et devient non actif. Refus quand rien n'est parti :
/// il se supprime (`supprimer`) ; rien n'est ecrit.
pub fn desactiver(ctx: &Contexte, mois: &str, rang: usize, departs: &Departs) -> Resultat<NonActif> {
    let mut o = stock::objet_du_mois(&ctx.racine, mois, rang)?.ok_or_else(|| format!("{} : pas d'engagement de rang {}", mois, rang + 1))?;
    if ce_qui_est_parti(ctx, &o, departs).is_empty() {
        return Err(format!("{} : rien n'est parti de cet engagement ; il se supprime, il ne devient pas non actif. Rien n'est écrit.", qui_de(ctx, &o)));
    }
    let le = maintenant();
    // La validation presente : la derniere version, gardee telle quelle.
    let derniere = o.brut.remove("valide");
    // Sa version suit la derniere validee : son present, rien de paye, est une version de plus.
    let version = o.brut.get(VERSIONS).and_then(Value::as_array).into_iter().flatten().chain(derniere.as_ref()).filter_map(|v| version_de(Some(v))).max().unwrap_or(0) + 1;
    if let Some(v) = derniere.filter(|v| version_de(Some(v)).is_some()) {
        let gardee = json!({
            "version": v["version"], "valide_le": v.get("le").cloned().unwrap_or(Value::Null), "remplacee_le": le,
            "saisie": v.get("saisie").cloned().unwrap_or(Value::Null), "paies": v.get("paies").cloned().unwrap_or(Value::Null),
        });
        match o.brut.get_mut(VERSIONS).and_then(Value::as_array_mut) {
            Some(vs) => vs.push(gardee),
            None => {
                o.brut.insert(VERSIONS.into(), json!([gardee]));
            }
        }
    }
    o.brut.insert(stock::NON_ACTIF.into(), json!({ "le": le, "version": version }));
    stock::reecrire(&ctx.racine, &o)?;
    NonActif::de(o).ok_or_else(|| "desactivation illisible".to_string())
}

/// Les engagements non actifs que `mois` montrerait, tels qu'ils sont ecrits.
pub fn non_actifs_du_mois(ctx: &Contexte, mois: &str) -> Resultat<Vec<NonActif>> {
    Ok(stock::non_actifs_du_mois(&ctx.racine, mois)?.into_iter().filter_map(NonActif::de).collect())
}

/// Une raison de recalcul qui est une decision de l'editeur : le code qui calcule, ou une regle datee.
fn decision_de_l_editeur(raison: &str) -> bool {
    raison == CALCUL_CORRIGE || raison.starts_with("règle du ")
}

/// LE RECALCUL DE LUI-MEME, a la lecture de `mois` (`du_mois`) : chaque engagement valide du mois dont la paie de ce mois
/// n'est plus juste (son empreinte ne tient plus, ou elle manque : un contrat sans terme qui entre dans un nouveau mois)
/// se recalcule et se reecrit. Un chiffre qui change note pourquoi, et quand ; rien d'autre ne change. Si un envoi a
/// emporte la paie de ce mois (sa DSN, ses documents), le chiffre qui change fait une version datee (`nouvelle_version`) :
/// la version d'avant garde ce qui est parti.
///
/// Un mois dont les documents sont partis ne bouge que par une decision de l'editeur (le code qui calcule, une regle
/// datee : `empreinte::code_ou_regle_change`, puis la raison du recalcul), et la paie refaite rectifie le bulletin parti.
/// Un taux de PAS recu, une configuration, une autre saisie ne touchent pas un mois parti.
fn rafraichir(ctx: &Contexte, mois: &str, departs: &Departs) -> Resultat<Option<Vec<Etat>>> {
    // Ce qui ne tient plus : le rang dans l'ecran, l'engagement, sa paie ecrite du mois, et si ses documents sont partis.
    let mut a_refaire: Vec<(usize, Objet, Option<Value>, bool)> = Vec::new();
    let mut relues = empreinte::Relues::default();
    let mut reecrit = false;
    let etats = tels_qu_ecrits(ctx, mois, departs)?;
    for (i, e) in etats.iter().enumerate() {
        let (v, parti) = match e {
            Etat::Valide(v) => (v, false),
            // Une paie qui manque (un contrat sans terme qui entre dans le mois) s'ecrit.
            Etat::Partis(p) if p.valide().paie_ecrite(mois).is_none() => (p.valide(), false),
            Etat::Partis(p) => (p.valide(), true),
            Etat::EnSaisie(_) => continue,
        };
        let ancienne = v.paie_ecrite(mois);
        let empreinte_ecrite = ancienne.and_then(|a| a.get("empreinte"));
        // Partie, elle ne bouge que par une decision de l'editeur : on la cherche d'abord (quelques entrees), et
        // l'empreinte entiere ne se verifie que si elle a lieu.
        if parti && !empreinte_ecrite.is_some_and(|e| empreinte::code_ou_regle_change(ctx, e, mois, &mut relues)) {
            // Partie, elle ne bouge pas ; le compteur avant le mois, que le mois suivant reprend, suit les mois d'avant.
            if let Some(a) = ancienne.filter(|a| a.get(CONGES_AVANT).is_some()) {
                if let Some(nouvelle) = conges_avant_a_jour(ctx, &v.objet, a, mois, &mut relues)? {
                    let mut o = v.objet.clone();
                    if let Some(Value::Object(paies)) = o.brut.get_mut("valide").and_then(|x| x.get_mut("paies")) {
                        paies.insert(mois.to_string(), nouvelle);
                    }
                    stock::reecrire(&ctx.racine, &o)?;
                    reecrit = true;
                }
            }
            continue;
        }
        let tient = empreinte_ecrite.and_then(empreinte::entrees_de).is_some_and(|en| empreinte::tient(ctx, &en, &mut relues));
        if tient {
            continue;
        }
        a_refaire.push((i, v.objet.clone(), ancienne.cloned(), parti));
    }
    if a_refaire.is_empty() {
        return Ok((!reecrit).then_some(etats));
    }
    let rangs: Vec<usize> = a_refaire.iter().map(|(i, _, _, _)| *i).collect();
    let nouvelles = calculer_les_rangs(ctx, mois, &rangs)?;
    for ((_, mut o, ancienne, parti), mut nouvelle) in a_refaire.into_iter().zip(nouvelles) {
        let raison = match &ancienne {
            Some(a) if chiffres(a) != chiffres(&nouvelle) => Some(raison(a, &nouvelle, parti)),
            Some(a) => {
                if let Some(r) = a.get("recalcul").filter(|r| r.is_object()) {
                    nouvelle["recalcul"] = r.clone();
                }
                None
            }
            None => None,
        };
        let le = maintenant();
        if let Some(r) = &raison {
            nouvelle["recalcul"] = json!({ "le": le, "raison": r });
        }
        let ancienne = ancienne.unwrap_or(Value::Null);
        match (&raison, parti) {
            // Partie, sans chiffre qui change : l'empreinte se renouvelle, la paie garde ses numeros.
            (None, true) => {
                for k in [BULLETIN, CE, RECTIFIE] {
                    if let Some(x) = ancienne.get(k) {
                        nouvelle[k] = x.clone();
                    }
                }
            }
            // Partie, et une decision de l'editeur la change : un bulletin rectificatif.
            (Some(r), true) if decision_de_l_editeur(r) => {
                if let Some(b) = ancienne.get(BULLETIN).or_else(|| ancienne.get(RECTIFIE)).filter(|b| b.is_string()) {
                    nouvelle[RECTIFIE] = b.clone();
                }
            }
            // Partie, et autre chose que l'editeur la changerait : elle ne bouge pas.
            (Some(_), true) => continue,
            (_, false) => {
                if let Some(r) = ancienne.get(RECTIFIE) {
                    nouvelle[RECTIFIE] = r.clone();
                }
            }
        }
        // Un chiffre, ou ce que la paie a lu (la fiche, la saisie du mois), qui change sur une paie qu'un envoi a emportee
        // (la DSN du mois, ses documents) : la version sur laquelle il est parti se garde, datee, avec ce qu'il a emporte.
        if raison.is_some() || lu(&ancienne) != lu(&nouvelle) {
            let emportee: Vec<Depart> = ses_departs(ctx, &o, departs)
                .iter()
                .filter(|d| matches!(d.envoi, Envoi::Documents | Envoi::Dsn) && d.mois.as_deref() == Some(mois))
                .cloned()
                .collect();
            // Une validation a revalider (`a_revalider`) n'a rien emporte : sa paie se refait, sans version.
            let actuelle = o.brut.get("valide").cloned().filter(|v| a_revalider(ctx, &o, v).is_none());
            if let Some((actuelle, n)) = actuelle.as_ref().and_then(|a| Some((a, version_de(Some(a))?))) {
                let n = nouvelle_version(&mut o, actuelle, n, &emportee, &le);
                if let Some(v) = o.brut.get_mut("valide").and_then(Value::as_object_mut) {
                    v.insert("version".into(), json!(n));
                }
            }
        }
        if let Some(Value::Object(paies)) = o.brut.get_mut("valide").and_then(|x| x.get_mut("paies")) {
            paies.insert(mois.to_string(), nouvelle);
        }
        stock::reecrire(&ctx.racine, &o)?;
    }
    Ok(None)
}

/// LE COMPTEUR AVANT LE MOIS D'UNE PAIE PARTIE, a jour des mois d'avant : la paie partie ne bouge pas, mais l'etat
/// qu'elle tient pour le mois suivant (`CONGES_AVANT`) suit ce dont il depend (les paies validees des mois qu'il lit,
/// les fiches). `None` : il tient. Sinon, la paie ecrite avec l'etat refait, et ce qu'il a lu dans son empreinte.
fn conges_avant_a_jour(ctx: &Contexte, o: &Objet, ancienne: &Value, mois: &str, relues: &mut empreinte::Relues) -> Resultat<Option<Value>> {
    let entrees = ancienne.get("empreinte").and_then(empreinte::entrees_de).unwrap_or_default();
    let de_lui = |k: &str| k.starts_with("paies ") || k == "fiches";
    let siennes: BTreeMap<String, String> = entrees.iter().filter(|(k, _)| de_lui(k)).map(|(k, v)| (k.clone(), v.clone())).collect();
    if empreinte::tient(ctx, &siennes, relues) {
        return Ok(None);
    }
    let cle = o.brut.get("salarie").map(en_texte).unwrap_or_default();
    let data = stock::vue_du_mois(&ctx.racine, mois)?.ok_or_else(|| format!("{} : aucune saisie", mois))?;
    let fin = socle::date::dernier_du_mois(mois)?;
    let (avant, lues) = empreinte::suivre(ctx, &data, || crate::conges::avant_le_mois(ctx, &cle, &fin, mois))?;
    let mut nouvelle = ancienne.clone();
    nouvelle[CONGES_AVANT] = avant.en_json();
    let mut entrees: Map<String, Value> = entrees.into_iter().filter(|(k, _)| !k.starts_with("paies ")).map(|(k, v)| (k, Value::String(v))).collect();
    for (k, v) in lues.into_iter().filter(|(k, _)| de_lui(k)) {
        entrees.insert(k, Value::String(v));
    }
    nouvelle["empreinte"]["entrees"] = Value::Object(entrees);
    Ok(Some(nouvelle))
}

// ------------------------------------------------------------------ ce qui est a refaire

/// Ce que dit le contrat, dans la saisie d'un engagement : ses parties, son objet, ses dates, sa remuneration, sa nature
/// et son emploi. INFERENCE : la liste suit les marqueurs du modele de contrat (`donnees/modeles/`) ; les lignes d'un
/// contrat paye au mois se composent chaque mois et ne sont pas des clauses.
const CLAUSES: [&str; 18] = [
    "salarie", "spectacle", "debut", "fin", "numero", "lignes", "lieu", "nature", "motif_recours", "quotite_contrat",
    "quotite_reference", "modalite_temps_travail", "salaire_mensuel", "emploi", "metier", "cadre", "statut", "planning",
];

/// Les clauses d'une saisie gardee (`saisie_de`) : celles de son premier mois et celles que chaque mois d'apres change.
fn clauses(saisie: &Value, mensuel: bool) -> Value {
    let garder = |m: &Map<String, Value>| -> Value {
        Value::Object(m.iter().filter(|(k, _)| CLAUSES.contains(&k.as_str()) && !(mensuel && k.as_str() == "lignes")).map(|(k, v)| (k.clone(), v.clone())).collect())
    };
    let vide = Map::new();
    let suite: Map<String, Value> = saisie
        .get(stock::SUITE)
        .and_then(Value::as_object)
        .unwrap_or(&vide)
        .iter()
        .map(|(m, x)| (m.clone(), garder(x.as_object().unwrap_or(&vide))))
        .collect();
    json!({ "premier": garder(saisie.as_object().unwrap_or(&vide)), "suite": suite })
}

/// Ce qu'un envoi parti sur une version passee demande de refaire, le present ayant change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ARefaire {
    /// Le contrat (ou son dernier avenant) est parti sur la version `depuis`, et ses clauses ont change : un avenant a
    /// faire signer.
    Avenant { depuis: u64 },
    /// La DPAE est partie sur la version `depuis` avec la date d'embauche `debut_declare`, qui a change.
    Dpae { depuis: u64, debut_declare: String },
    /// Les documents de `mois` sont partis sur la version `depuis`, et sa paie a change : un bulletin rectificatif.
    Bulletin { mois: String, depuis: u64 },
    /// La DSN de `mois` est partie sur la version `depuis`, et sa paie a change : un annule-et-remplace, ou la correction
    /// dans une DSN suivante (la couche des declarations tranche, a l'echeance).
    Dsn { mois: String, depuis: u64 },
}

/// LA VERSION EN VIGUEUR AU DEPART de chaque envoi, comparee au present de l'engagement valide `v` : ce qui est a
/// refaire. Pour chaque sorte d'envoi (et chaque mois des documents et des DSN), le dernier parti fait foi ; parti sur la
/// version presente, il est a jour.
pub fn a_refaire(ctx: &Contexte, v: &Valide, departs: &Departs) -> Vec<ARefaire> {
    let siens = ses_departs(ctx, &v.objet, departs);
    let present = v.version();
    let actuelle = v.objet.brut.get("valide").cloned().unwrap_or(Value::Null);
    let mut derniers: BTreeMap<(u8, Option<String>), &Depart> = BTreeMap::new();
    for d in siens {
        let sorte = match d.envoi {
            Envoi::Contrat => 0,
            Envoi::Dpae => 1,
            Envoi::Documents => 2,
            Envoi::Dsn => 3,
            Envoi::Signalement => 4,
        };
        derniers.insert((sorte, d.mois.clone()), d);
    }
    let mensuel = v.objet.mensuel();
    let mut out = Vec::new();
    for d in derniers.into_values() {
        if Some(d.version) == present {
            continue;
        }
        let Some(passee) = v.version_numero(d.version) else { continue };
        let saisie = |x: &Value| x.get("saisie").cloned().unwrap_or(Value::Null);
        let paie = |x: &Value, m: &str| x.get("paies").and_then(|p| p.get(m)).map(chiffres);
        match d.envoi {
            Envoi::Contrat => {
                if clauses(&saisie(passee), mensuel) != clauses(&saisie(&actuelle), mensuel) {
                    out.push(ARefaire::Avenant { depuis: d.version });
                }
            }
            Envoi::Dpae => {
                let (avant, maintenant) = (crate::debut_brut(&saisie(passee)), crate::debut_brut(&saisie(&actuelle)));
                if avant != maintenant {
                    out.push(ARefaire::Dpae { depuis: d.version, debut_declare: avant });
                }
            }
            // Un signalement (arret, reprise, fin de contrat) part sur sa version ; il ne porte pas de paie.
            Envoi::Signalement => {}
            Envoi::Documents | Envoi::Dsn => {
                let Some(m) = d.mois.as_deref() else { continue };
                if paie(passee, m) != paie(&actuelle, m) {
                    out.push(if d.envoi == Envoi::Documents {
                        ARefaire::Bulletin { mois: m.to_string(), depuis: d.version }
                    } else {
                        ARefaire::Dsn { mois: m.to_string(), depuis: d.version }
                    });
                }
            }
        }
    }
    out
}

/// La version (`saisie`, `paies`) sur laquelle le dernier envoi `envoi` de l'engagement `v` est parti, telle qu'elle est
/// ecrite ; `None` si rien de tel n'est parti.
pub fn version_au_depart<'a>(ctx: &Contexte, v: &'a Valide, departs: &Departs, envoi: Envoi) -> Option<&'a Value> {
    let d = ses_departs(ctx, &v.objet, departs).iter().rev().find(|d| d.envoi == envoi)?;
    v.version_numero(d.version)
}

/// La version `n` de l'engagement `o`, telle qu'elle est ecrite (`saisie`, `paies`) : le present, la validation que
/// « Modifier… » a mise de cote, ou une version passee ; `None` si l'engagement n'a pas de version `n`. Refus : une
/// version qui ne dit pas tout ce qu'un envoi a emporte (`a_revalider`).
fn version_ecrite<'a>(ctx: &Contexte, o: &'a Objet, n: u64) -> Resultat<Option<&'a Value>> {
    let passees = o.brut.get(VERSIONS).and_then(Value::as_array).into_iter().flatten();
    let Some(v) = [o.brut.get("valide"), o.brut.get(PRECEDENTE)].into_iter().flatten().chain(passees).find(|v| version_de(Some(v)) == Some(n)) else {
        return Ok(None);
    };
    match a_revalider(ctx, o, v) {
        Some(phrase) => Err(phrase),
        None => Ok(Some(v)),
    }
}

/// La version de l'engagement `o` sur laquelle la derniere DSN qui a declare `mois` est partie (celle du mois, ou celle
/// d'un mois suivant qui a porte sa correction), telle qu'elle est ecrite ; `None` si aucune DSN n'a declare ce mois
/// pour lui.
fn version_de_la_dsn<'a>(ctx: &Contexte, o: &'a Objet, departs: &Departs, mois: &str) -> Resultat<Option<&'a Value>> {
    match ses_departs(ctx, o, departs).iter().rev().find(|d| d.envoi == Envoi::Dsn && d.mois.as_deref() == Some(mois)) {
        Some(d) => version_ecrite(ctx, o, d.version),
        None => Ok(None),
    }
}

/// Ce que la version `version` dit de `mois` (`Objet::vue` de sa saisie).
fn vue_de_la_version(o: &Objet, version: &Value, mois: &str) -> Resultat<Option<Map<String, Value>>> {
    let saisie = version.get("saisie").and_then(Value::as_object).ok_or("version ecrite illisible : `saisie`")?;
    let mut declare = o.clone();
    declare.brut = saisie.clone();
    declare.vue(mois)
}

/// L'ENGAGEMENT DECLARE pour `mois` : ce que la saisie de la version sur laquelle la derniere DSN qui a declare ce mois
/// est partie dit de ce mois (`Objet::vue`) ; `None` si aucune DSN n'a declare ce mois pour l'engagement `o`.
pub fn engagement_declare(ctx: &Contexte, o: &Objet, departs: &Departs, mois: &str) -> Resultat<Option<Map<String, Value>>> {
    match version_de_la_dsn(ctx, o, departs, mois)? {
        Some(v) => vue_de_la_version(o, v, mois),
        None => Ok(None),
    }
}

/// LA SAISIE DU MOIS DECLAREE : ce que la saisie de `mois` posait pour tous ses engagements (le taux du cachet, la date
/// de paiement) quand la paie que la derniere DSN de ce mois a declaree pour `o` a ete calculee (`MOIS_LU`) ; `None` si
/// aucune DSN n'a declare ce mois pour lui.
pub fn mois_declare(ctx: &Contexte, o: &Objet, departs: &Departs, mois: &str) -> Resultat<Option<Value>> {
    let Some(v) = version_de_la_dsn(ctx, o, departs, mois)? else { return Ok(None) };
    Ok(v.get("paies").and_then(|p| p.get(mois)).and_then(|p| p.get(MOIS_LU)).cloned())
}

/// La paie de `mois` que la version `version` de `o` a ecrite, relue (`relire`) sur la saisie de cette version, la fiche
/// du salarie et la saisie du mois telles que son calcul les a lues (`fiches`, `MOIS_LU`) ; `None` si cette version n'a
/// pas de paie pour ce mois. Elle porte le numero de son bulletin quand ses documents sont partis.
fn paie_de_la_version(ctx: &Contexte, o: &Objet, version: &Value, mois: &str) -> Resultat<Option<Paie>> {
    let Some(ecrite) = version.get("paies").and_then(|p| p.get(mois)) else { return Ok(None) };
    let paie = ecrite.get("paie").ok_or("paie ecrite illisible : `paie`")?;
    let Some(vue) = vue_de_la_version(o, version, mois)? else { return Ok(None) };
    let mut data = ecrite.get(MOIS_LU).filter(|m| m.is_object()).cloned().ok_or_else(|| format!("paie ecrite illisible : `{}`", MOIS_LU))?;
    data["mois"] = json!(mois);
    data["engagements"] = json!([vue]);
    let eng = relire::engagements_ecrits(ctx, &data, &[paie])?.pop().ok_or("paie ecrite illisible")?;
    let fiches = ecrite.get("fiches").filter(|f| f.is_object()).ok_or("paie ecrite illisible : `fiches`")?;
    let mut lue = relire::paie_sur_ses_fiches(ctx, eng, paie, fiches)?;
    let p = lue.ecrire();
    p.pas = ecrite.get("pas").filter(|x| x.is_object()).map(droit::pas::Versement::lire).transpose()?;
    p.conges = ecrite.get(CONGES).filter(|x| x.is_object()).map(crate::conges::CompteurConges::lire).transpose()?;
    p.conges_avant = ecrite.get(CONGES_AVANT).filter(|x| x.is_object()).map(crate::conges::Avant::lire).transpose()?;
    p.numero_bulletin = ecrite.get(BULLETIN).and_then(Value::as_str).map(String::from);
    Ok(Some(lue))
}

/// LA PAIE DECLAREE de `mois` : celle que la version sur laquelle la derniere DSN qui a declare ce mois est partie a
/// ecrite (`paie_de_la_version`) (regle 4 : ce qui est declare se lit, il ne se recalcule pas) ; `None` si aucune DSN
/// n'a declare ce mois pour l'engagement `o`.
pub fn paie_declaree(ctx: &Contexte, o: &Objet, departs: &Departs, mois: &str) -> Resultat<Option<Paie>> {
    match version_de_la_dsn(ctx, o, departs, mois)? {
        Some(v) => paie_de_la_version(ctx, o, v, mois),
        None => Ok(None),
    }
}

/// LA PAIE PARTIE de `mois` : celle que la version sur laquelle les derniers documents de ce mois sont partis au salarie
/// a ecrite (`paie_de_la_version`), avec le numero de son bulletin ; `None` si les documents de ce mois ne sont pas partis
/// pour l'engagement `o`. C'est ce que son bulletin a paye.
pub fn paie_partie(ctx: &Contexte, o: &Objet, departs: &Departs, mois: &str) -> Resultat<Option<Paie>> {
    let Some(d) = ses_departs(ctx, o, departs).iter().rev().find(|d| d.envoi == Envoi::Documents && d.mois.as_deref() == Some(mois)) else {
        return Ok(None);
    };
    match version_ecrite(ctx, o, d.version)? {
        Some(v) => paie_de_la_version(ctx, o, v, mois),
        None => Ok(None),
    }
}

/// Une paie declaree pour un mois, et le mois de la DSN qui l'a portee quand ce n'est pas la sienne (une correction).
pub struct Declaree {
    pub par: Option<String>,
    /// La version de l'engagement sur laquelle la DSN est partie.
    pub version: u64,
    pub paie: Paie,
}

/// CE QUI A ETE DECLARE POUR `mois`, DANS L'ORDRE, par les DSN des mois anterieurs a `avant` : la paie de la version sur
/// laquelle la DSN de ce mois est partie (la derniere : un annule-et-remplace remplace la precedente), puis celle de
/// chaque correction qu'une DSN d'un mois suivant a portee (la derniere DSN de ce mois-la fait foi). Vide si aucune DSN
/// n'a declare ce mois pour l'engagement `o`. Refus : une correction portee par la DSN d'un mois posterieur a `avant`
/// (une correction se porte apres la derniere qui a declare le contrat), ou une version qui n'a pas de paie de ce mois.
pub fn paies_declarees(ctx: &Contexte, o: &Objet, departs: &Departs, mois: &str, avant: &str) -> Resultat<Vec<Declaree>> {
    let mut chaine: Vec<(Option<String>, u64)> = Vec::new();
    for d in ses_departs(ctx, o, departs).iter().filter(|d| d.envoi == Envoi::Dsn && d.mois.as_deref() == Some(mois)) {
        match d.par.as_deref() {
            None => chaine = vec![(None, d.version)],
            Some(p) if p > avant => {
                return Err(format!(
                    "{} : une correction de {} est deja partie dans la DSN de {}, posterieure a {}. Une correction se porte dans un mois posterieur au dernier qui a declare le contrat.",
                    qui_de(ctx, o),
                    mois,
                    p,
                    avant
                ))
            }
            Some(p) if p == avant => {}
            Some(p) => match chaine.last_mut() {
                Some((Some(q), v)) if q == p => *v = d.version,
                _ => chaine.push((Some(p.to_string()), d.version)),
            },
        }
    }
    let mut out: Vec<Declaree> = Vec::new();
    let non_actif = o.brut.get(stock::NON_ACTIF).and_then(|n| n.get("version")).and_then(Value::as_u64);
    for (par, n) in chaine {
        // Une correction partie sur la version d'un non actif a declare la paie d'avant, annulee.
        if Some(n) == non_actif {
            if let Some(avant) = out.last().map(|d| d.paie.annulee()) {
                out.push(Declaree { par, version: n, paie: avant });
                continue;
            }
        }
        let paie = version_ecrite(ctx, o, n)?
            .map(|v| paie_de_la_version(ctx, o, v, mois))
            .transpose()?
            .flatten()
            .ok_or_else(|| format!("{} : la version {} sur laquelle une DSN a declare {} n'a pas de paie de ce mois", qui_de(ctx, o), n, mois))?;
        out.push(Declaree { par, version: n, paie });
    }
    Ok(out)
}

/// LA CORRECTION DUE pour `mois` de l'engagement valide `v`, dans la DSN de `avant` : ce qui a ete declare pour ce mois
/// par les DSN anterieures a `avant` (`paies_declarees`) quand la paie presente de `mois` n'est plus celle que la
/// derniere a declaree (la paie elle-meme, pas le prelevement ni le compteur de conges, que la DSN ne corrige pas) ; vide
/// quand aucune DSN n'a declare ce mois pour lui. `None` : rien a corriger, ou pas de paie presente de ce mois. La couche
/// des declarations dit si le mois se corrige ainsi (sa DSN partie, son annule-et-remplace passe).
pub fn correction_due(ctx: &Contexte, v: &Valide, departs: &Departs, mois: &str, avant: &str) -> Resultat<Option<Vec<Declaree>>> {
    let Some(presente) = v.paie_ecrite(mois).filter(|p| p.get("paie").is_some()) else { return Ok(None) };
    let declarees = paies_declarees(ctx, &v.objet, departs, mois, avant)?;
    let Some(derniere) = declarees.last() else { return Ok(Some(declarees)) };
    if Some(derniere.version) == v.version() {
        return Ok(None);
    }
    let ecrite = version_ecrite(ctx, &v.objet, derniere.version)?.and_then(|x| x.get("paies")).and_then(|p| p.get(mois));
    if ecrite.is_some_and(|e| chiffres(e)["paie"] == chiffres(presente)["paie"]) {
        return Ok(None);
    }
    Ok(Some(declarees))
}

/// LA DERNIERE DATE DE DEBUT DECLAREE de l'engagement `o` (AAAA-MM-JJ) par une DSN d'un mois anterieur a `avant` : celle
/// de la saisie de la version sur laquelle la derniere est partie, quel que soit le mois qu'elle declarait (la correction
/// d'un autre mois d'un contrat sur plusieurs mois a pu la changer). `None` : aucune.
pub fn debut_declare(ctx: &Contexte, o: &Objet, departs: &Departs, avant: &str) -> Resultat<Option<String>> {
    let Some(d) = ses_departs(ctx, o, departs).iter().rev().find(|d| d.envoi == Envoi::Dsn && d.par.as_deref().or(d.mois.as_deref()).is_some_and(|m| m < avant)) else {
        return Ok(None);
    };
    Ok(version_ecrite(ctx, o, d.version)?.and_then(|v| v.get("saisie")).map(crate::debut_brut).filter(|d| !d.is_empty()))
}

/// La phrase d'un recalcul, pour l'ecran : « Recalculé le JJ/MM/AAAA : taux de PAS reçu. »
pub fn phrase_du_recalcul(r: &Value) -> Option<String> {
    let raison = r.get("raison").and_then(Value::as_str)?;
    let le = r.get("le").map(en_texte).unwrap_or_default();
    let jour = Jour::lire(le.get(..10).unwrap_or("")).map(|j| j.jj_mm_aaaa()).unwrap_or(le);
    Some(format!("Recalculé le {} : {}.", jour, raison))
}

/// La paie de l'engagement de rang `i` dans la saisie `data` est-elle validee ? (Une saisie de l'ecran, pas encore
/// ecrite, n'a pas de rang sur le disque : on la compare a ce qui est ecrit.)
pub fn est_validee(v: &Value) -> bool {
    v.get("valide").is_some_and(vrai)
}

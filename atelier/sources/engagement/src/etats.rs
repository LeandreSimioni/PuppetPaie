//! LES TROIS ETATS D'UN ENGAGEMENT, pour un mois :
//!
//! - EN SAISIE (`EnSaisie`) : il se calcule a mesure pour l'ecran ; rien ne peut partir de lui ;
//! - VALIDE (`Valide`) : « Valider l'engagement » a ecrit, dans l'engagement, `valide` : la date, le numero de sa
//!   version et sa saisie telle qu'elle etait ; et, dans le fichier de chaque mois qu'il couvre (`paies`), sa paie
//!   calculee, avec l'empreinte de ce dont son calcul depend (`empreinte`). Tout ce qui part part d'elles
//!   (`validees_du_mois`, `Valide::paie`) ;
//! - DOCUMENTS PARTIS (`Partis`) : les documents du mois (bulletin, certificat Conges Spectacles, AER) sont partis au
//!   salarie ; ce mois ne se recalcule plus de lui-meme, sauf par une decision de l'editeur. La couche des envois le dit
//!   (`departs`).
//!
//! Trois types, et seul un engagement valide (ou parti, qui l'est) donne une paie a qui fabrique un document : un
//! engagement en saisie n'a pas de `paie`.
//!
//! LES VERSIONS DATEES. Tant que rien n'est parti d'un engagement, il se modifie librement, sans historique. Des qu'un
//! envoi est parti sur sa version (`departs`), une revalidation qui le change garde dans l'engagement la version d'avant
//! (`versions` : sa saisie telle qu'elle etait, la date de sa validation et celle ou elle a ete remplacee ; ses paies se
//! gardent sous son numero dans le fichier de leur mois, `paies::de_la_version`), et le present prend le numero suivant. Une
//! revalidation qui ne change rien ne fait pas de version. Une
//! version passee ne se reecrit jamais. Ce qui est a refaire (`a_refaire`) se tire de la comparaison entre le present et
//! la version sur laquelle chaque envoi est parti.
//!
//! LE RECALCUL DE LUI-MEME (`rafraichir`) : a chaque lecture d'un mois (`du_mois`, par ou passe tout ce qui lit une
//! paie validee : l'ecran, les exports, les declarations, les documents, les envois), la paie de chaque engagement
//! valide se compare a son empreinte. Si elle ne tient plus, la paie se recalcule et se reecrit ; si un chiffre a change,
//! elle dit pourquoi (`raison`) : « taux de PAS reçu » (seul le prelevement change), « règle du JJ/MM/AAAA » (une regle
//! datee s'applique : du bareme, ou une valeur datee de la compagnie, son taux AT/MP de l'annee), « calcul corrigé » (le code qui calcule a change), « un autre engagement du salarié a changé » (un
//! engagement de son salarie dans le mois), « saisie du mois modifiée » (le taux du cachet, la date de paiement), « un arrêt
//! ou une absence a changé » (un imprevu du salarie que le calcul lit), « la paie de
//! <mois> a changé » (une paie validee d'un autre mois que son calcul lit : conges, cumuls), « configuration de la
//! compagnie modifiée » (une fiche corrigee sans date nouvelle, les contrats d'organismes). Un changement qui ne change aucun chiffre ne laisse aucune trace.
//! Un mois dont les documents sont partis ne se recalcule que par une decision de l'editeur (« calcul corrigé », une
//! regle datee) : le recalcul fait alors une version datee, comme une modification, et la paie refaite rectifie le
//! bulletin parti.
//!
//! LE PRESENT SEUL. Un mois est EN COURS ou FINI (`fini` : ses documents et sa DSN sont partis). Seul un mois en cours se
//! recalcule a sa lecture ; un mois fini se lit tel qu'il est ecrit, et une decision de l'editeur ne l'atteint qu'au
//! passage de la version qui la porte (`corriger_les_mois_finis`). Aucune paie ne s'ecrit pour un mois d'apres le present
//! (`mois_present`). Ce qu'une paie lit d'un autre mois (le compteur de conges que la paie d'avant tient pour elle) se
//! lit tel qu'il est ecrit ; une validation qui refait un mois remet a jour le report des paies parties qui le suivent
//! (`reporter`).
//!
//! MODIFIER UN ENGAGEMENT VALIDE se fait d'un geste, dans une seule ecriture de la route : il repasse en saisie
//! (`modifier`), sa validation mise de cote (`precedente`), prend sa saisie nouvelle (`stock::poser_la_saisie`) et se
//! revalide aussitot ; la validation reprend telles quelles les paies des mois dont les documents sont partis tant que la
//! saisie de leur mois n'a pas change, et fait une version si quelque chose est parti sur elle. Il ne reste jamais
//! rouvert.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

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
/// La raison d'un recalcul : un autre engagement valide du meme salarie dans le mois, que le calcul lit, a change (la
/// deduction forfaitaire specifique deja retenue, `calcul::dfs_deja_deduite`).
pub const AUTRE_ENGAGEMENT: &str = "un autre engagement du salarié a changé";
/// La raison d'un recalcul : ce que la saisie du mois pose pour tous ses engagements (taux du cachet, date de paiement) a
/// change.
pub const SAISIE_DU_MOIS: &str = "saisie du mois modifiée";
/// La raison d'un recalcul : un arret ou une suspension du salarie (le magasin des imprevus, `imprevus::DOSSIER`), que le
/// calcul lit, a change.
pub const IMPREVU: &str = "un arrêt ou une absence a changé";
/// La raison d'un recalcul : la configuration de la compagnie a change sans valeur datee nouvelle (une fiche corrigee, les
/// contrats d'organismes).
pub const CONFIGURATION: &str = "configuration de la compagnie modifiée";

/// Dans l'engagement : la validation qu'une modification va remplacer, et les versions passees.
pub(crate) const PRECEDENTE: &str = "precedente";
const VERSIONS: &str = "versions";

// ------------------------------------------------------------------ ce qui est parti

/// Ce qui est parti de l'engagement `o` (`departs`), d'apres la cle de son salarie et son numero.
pub(crate) fn ses_departs<'a>(ctx: &Contexte, o: &Objet, departs: &'a Departs) -> &'a [Depart] {
    match (crate::cle_de_l_engagement(ctx, &o.brut), o.brut.get("numero").filter(|n| vrai(n)).map(en_texte)) {
        (Some(c), Some(n)) => departs.de(&c, &n),
        _ => &[],
    }
}

/// LE MOIS DU PRESENT : celui de l'horloge (`Contexte::aujourdhui` : la date du geste, la veille ou le jour). Aucune paie
/// ne s'ecrit pour un mois d'apres : elle se calcule quand il devient le present, avec les regles qui le regissent.
pub fn mois_present(ctx: &Contexte) -> String {
    ctx.aujourdhui.mois_civil().to_string()
}

/// UN MOIS FINI, pour l'engagement dont `siens` est ce qui est parti : ses documents de `mois` sont partis au salarie et
/// une DSN partie l'a declare pour ce mois (une DSN refusee au depot ou au bilan n'est pas partie). Sa paie se lit telle
/// qu'elle est ecrite ; seule une decision de l'editeur la refait, une fois, au passage de la version qui la porte
/// (`corriger_les_mois_finis`). Un mois qui n'est pas fini est EN COURS : il se recalcule de lui-meme a sa lecture.
pub fn fini(siens: &[Depart], mois: &str) -> bool {
    documents_partis(siens, mois).is_some() && siens.iter().any(|d| d.envoi == Envoi::Dsn && d.mois.as_deref() == Some(mois))
}

/// LE PASSAGE D'UNE VERSION SUR LES MOIS FINIS : chaque mois qui a des paies, du plus ancien au plus recent, se relit
/// une fois comme un mois dont les documents sont partis (`rafraichir`) : une decision de l'editeur (le code qui calcule,
/// une regle datee) refait sa paie, en rectificatif, avec une version ; le reste ne bouge pas. La couche du dessus
/// l'appelle une fois par version installee. Rend les mois dont une paie s'est refaite.
pub fn corriger_les_mois_finis(ctx: &Contexte, departs: &Departs) -> Resultat<Vec<String>> {
    let mut refaits = Vec::new();
    for mois in crate::paies::mois_payes(&ctx.racine) {
        let avant: Vec<Option<Arc<Value>>> = tels_qu_ecrits(ctx, &mois, departs)?.iter().map(|e| e.valide().and_then(|v| v.paie_ecrite(&mois))).collect();
        let apres: Vec<Option<Arc<Value>>> = lu_et_rafraichi(ctx, &mois, departs, true)?.iter().map(|e| e.valide().and_then(|v| v.paie_ecrite(&mois))).collect();
        let change = |a: &Option<Arc<Value>>, b: &Option<Arc<Value>>| match (a, b) {
            (Some(x), Some(y)) => !chiffres_egaux(x, y),
            (x, y) => x.is_some() != y.is_some(),
        };
        if avant.len() != apres.len() || avant.iter().zip(&apres).any(|(a, b)| change(a, b)) {
            refaits.push(mois);
        }
    }
    Ok(refaits)
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
    /// Pourquoi la paie du mois lu ne s'est pas ecrite : le refus du recalcul (`du_mois`), quand elle manque, ou quand il
    /// dit une saisie a finir (`saisie_incomplete`) : la paie ecrite ne tient plus, et elle ne se lit pas.
    refus: Option<String>,
    /// La paie d'un mois qu'une validation en cours vient de calculer (`valides_lues`) : elle se lit en place de l'ecrite.
    en_cours: Option<(String, Arc<Value>)>,
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
        Some(Valide { objet, le, refus: None, en_cours: None })
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

    /// CE QUI PART SE LIT DANS LES VERSIONS : une validation sans son numero de version ou sans sa saisie ne dit pas ce
    /// qu'un envoi emporterait. Rien ne part d'elle et rien ne s'y relit : la phrase du refus ; `None` quand elle dit tout.
    pub fn a_revalider(&self, ctx: &Contexte) -> Option<String> {
        a_revalider(ctx, &self.objet, self.objet.brut.get("valide")?)
    }

    /// Ses versions passees, de la plus ancienne a la plus recente : `{"version", "valide_le", "remplacee_le", "saisie"}`.
    pub fn versions(&self) -> &[Value] {
        self.objet.brut.get(VERSIONS).and_then(Value::as_array).map_or(&[], Vec::as_slice)
    }

    /// La version `n`, passee ou presente, telle que l'engagement l'ecrit (`saisie`) ; ses paies se lisent par
    /// `paie_ecrite_de_la_version`.
    pub fn version_numero(&self, n: u64) -> Option<&Value> {
        if Some(n) == self.version() {
            return self.objet.brut.get("valide");
        }
        self.versions().iter().find(|v| version_de(Some(v)) == Some(n))
    }

    /// La paie ecrite de `mois` (`{"paie", "fiches", "pas"?, "empreinte", "recalcul"?}`), si elle l'est.
    pub fn paie_ecrite(&self, mois: &str) -> Option<Arc<Value>> {
        self.ecrite(mois).ok().flatten()
    }

    fn ecrite(&self, mois: &str) -> Resultat<Option<Arc<Value>>> {
        if self.refus.as_deref().is_some_and(crate::saisie_incomplete) {
            return Ok(None);
        }
        match &self.en_cours {
            Some((m, p)) if m == mois => Ok(Some(p.clone())),
            _ => crate::paies::presente(&self.objet, mois),
        }
    }

    /// Pourquoi la paie de `mois` s'est recalculee d'elle-meme, et quand : `{"le", "raison"}`.
    pub fn recalcul(&self, mois: &str) -> Option<Value> {
        self.paie_ecrite(mois)?.get("recalcul").filter(|r| r.is_object()).cloned()
    }

    /// LA PAIE DE `mois`, TELLE QU'ELLE A ETE VALIDEE : ce que lit ce qui fabrique un document. `data` : la saisie du mois
    /// (`stock::vue_du_mois`), `i` : le rang de l'engagement dans cette saisie.
    pub fn paie(&self, ctx: &Contexte, data: &Value, i: usize) -> Resultat<Paie> {
        let mois = data.get("mois").and_then(Value::as_str).ok_or_else(|| socle::fichiers::absent("mois"))?;
        let x = self.ecrite(mois)?.filter(|p| p.get("paie").is_some()).ok_or_else(|| match &self.refus {
            Some(refus) => format!("{} : la paie de {} ne se calcule pas. {}", qui_de(ctx, &self.objet), socle::mois::Mois::en_clair(mois), refus),
            None => format!(
                "{} : la paie de {} n'est pas calculée. Revalidez l'engagement : ouvrez sa carte, puis « Modifier l'engagement ».",
                qui_de(ctx, &self.objet),
                socle::mois::Mois::en_clair(mois)
            ),
        })?;
        let une = relire::saisie_d_un_engagement(data, i);
        let mut paie = relire::paies(ctx, &une, &[&x["paie"]])?.pop().ok_or_else(|| socle::echec::cle(socle::echec::PAIE_ILLISIBLE, "paie"))?;
        let p = paie.ecrire();
        p.pas = x.get("pas").filter(|x| x.is_object()).map(droit::pas::Versement::lire).transpose()?;
        p.conges = x.get(CONGES).filter(|x| x.is_object()).map(crate::conges::CompteurConges::lire).transpose()?;
        p.conges_avant = x.get(CONGES_AVANT).filter(|x| x.is_object()).map(crate::conges::Avant::lire).transpose()?;
        p.numero_bulletin = x.get(BULLETIN).and_then(Value::as_str).map(String::from);
        Ok(paie)
    }

    /// Le numero du bulletin de `mois` (`N/AAAA`), ecrit quand ses documents sont partis (`poser_numeros_des_documents`).
    pub fn bulletin(&self, mois: &str) -> Option<String> {
        self.paie_ecrite(mois)?.get(BULLETIN)?.as_str().map(String::from)
    }

    /// Le premier N CE du certificat Conges Spectacles de `mois`, ecrit quand ses documents sont partis.
    pub fn premier_ce(&self, mois: &str) -> Option<i64> {
        self.paie_ecrite(mois)?.get(CE)?.as_i64()
    }

    /// Le numero du bulletin que la paie de `mois` rectifie : celui d'une paie partie, refaite depuis.
    pub fn rectifie(&self, mois: &str) -> Option<String> {
        self.paie_ecrite(mois)?.get(RECTIFIE)?.as_str().map(String::from)
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
    let o = stock::objets_du_mois(&ctx.racine, mois)?
        .into_iter()
        .find(|o| {
            let e = &*o.brut;
            crate::cle_de_l_engagement(ctx, &e).as_deref() == Some(cle) && e.get("numero").and_then(Value::as_str) == Some(numero)
        })
        .ok_or_else(|| format!("{} : pas d'engagement de {} au contrat {}", mois, cle, numero))?;
    let Some(Value::Object(mut p)) = crate::paies::presente(&o, mois)?.filter(|_| o.est_valide()).map(|p| (*p).clone()) else {
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
        crate::paies::poser(&o, mois, Some(Value::Object(p)))?;
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

/// Le contrat de l'engagement `o` a-t-il ete signe hors du logiciel (`crate::signe_ailleurs_de`) ? Faux s'il ne se lit pas.
pub fn signe_ailleurs(ctx: &Contexte, o: &Objet) -> bool {
    let texte = |k: &str| o.brut.get(k).filter(|x| vrai(x)).map(en_texte).unwrap_or_default();
    let Ok(nat) = droit::natures::nature(o.brut.get("nature").and_then(Value::as_str)) else { return false };
    crate::signe_ailleurs_de(&ctx.racine, &nat, &texte("salarie"), &crate::debut_brut(&ctx.racine, &o.brut), &texte("numero"), &o.fichier).unwrap_or(false)
}

/// Qui est l'engagement `o`, pour une phrase de l'ecran : « MARTIN Lou (contrat du 20/11/2026) » (`crate::salarie_nomme`).
pub fn qui_de(ctx: &Contexte, o: &Objet) -> String {
    let debut = crate::debut_brut(&ctx.racine, &o.brut);
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
    lu_et_rafraichi(ctx, mois, departs, false)
}

/// Ce que `rafraichir` a fait : les etats lus, quand rien ne s'est reecrit ; et chaque engagement dont le recalcul a
/// refuse (son rang dans l'ecran), avec son refus.
struct Rafraichi {
    etats: Option<Vec<Etat>>,
    refus: Vec<(usize, String)>,
}

fn lu_et_rafraichi(ctx: &Contexte, mois: &str, departs: &Departs, passage: bool) -> Resultat<Vec<Etat>> {
    let refus: Vec<(Option<usize>, String)> = match rafraichir(ctx, mois, departs, passage) {
        // Rien ne s'est reecrit : les etats lus pour la verification sont ceux du mois.
        Ok(Rafraichi { etats: Some(etats), .. }) => return Ok(etats),
        Ok(Rafraichi { refus, .. }) => refus.into_iter().map(|(i, m)| (Some(i), m)).collect(),
        // Ce qui refuse avant tout calcul refuse pour tout le mois.
        Err(m) => vec![(None, m)],
    };
    let mut etats = tels_qu_ecrits(ctx, mois, departs)?;
    let mut temoins: Vec<&String> = Vec::new();
    for (_, m) in &refus {
        // Une saisie a finir est l'affaire de l'utilisateur : ce qui lit la paie la lui dit, l'editeur n'en est pas averti.
        if !temoins.contains(&m) && !crate::saisie_incomplete(m) {
            temoins.push(m);
            if let Some(temoin) = TEMOIN.get() {
                temoin(&ctx.racine, mois, m);
            }
        }
    }
    // Une paie qui manque (un contrat sans terme qui entre dans le mois) ne s'est pas ecrite a cause de son refus : qui la
    // lit le dit, avec sa cause (`Valide::paie`).
    // Une saisie a finir que le recalcul attend (les indemnites journalieres d'un arret, `maintien`) : la paie ecrite ne
    // tient plus, et ce qui la lit dit quoi saisir plutot que de lire une paie fausse.
    for (i, e) in etats.iter_mut().enumerate() {
        let (v, parti) = match e {
            Etat::Valide(v) => (v, false),
            Etat::Partis(p) => (&mut p.valide, true),
            Etat::EnSaisie(_) => continue,
        };
        let sien = refus.iter().find(|(r, _)| r.is_none_or(|r| r == i));
        if let Some((_, m)) = sien.filter(|(_, m)| v.paie_ecrite(mois).is_none() || (!parti && crate::saisie_incomplete(m))) {
            v.refus = Some(m.clone());
        }
    }
    Ok(etats)
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

/// La paie ecrite de `mois` de l'engagement du salarie `cle` (la cle de sa fiche) au contrat `numero`, lue par
/// `du_mois` ; `None` s'il n'est pas valide ou n'a pas de paie ecrite pour ce mois.
pub fn paie_ecrite_de(ctx: &Contexte, mois: &str, cle: &str, numero: &str, departs: &Departs) -> Resultat<Option<Value>> {
    Ok(du_mois(ctx, mois, departs)?.into_iter().find_map(|e| {
        let b = &*e.objet().brut;
        let sien = crate::cle_de_l_engagement(ctx, &b).as_deref() == Some(cle) && b.get("numero").and_then(Value::as_str) == Some(numero);
        if sien { e.valide().and_then(|v| v.paie_ecrite(mois)).map(|p| (*p).clone()) } else { None }
    }))
}

/// CE QUI PEUT PARTIR DE `mois` : sa saisie reduite aux engagements valides (dans l'ordre de l'ecran), et la paie ecrite
/// de chacun, au meme rang. Un engagement en saisie n'y est pas : aucun document ne part de lui. Un mois d'apres le
/// present n'a pas encore de paie ecrite : celle de chaque engagement valide s'y calcule, sans s'ecrire, pour ce qui part
/// avant lui (la DPAE, le contrat). `None` : aucune saisie.
pub fn validees_du_mois(ctx: &Contexte, mois: &str, departs: &Departs) -> Resultat<Option<(Value, Vec<Paie>)>> {
    validees_du_mois_parmi(ctx, mois, departs, &|_| true)
}

/// `validees_du_mois`, reduite aux engagements valides que `garder` retient : la paie d'un engagement que l'appelant ne
/// lit pas ne se relit pas.
pub fn validees_du_mois_parmi(ctx: &Contexte, mois: &str, departs: &Departs, garder: &dyn Fn(&Valide) -> bool) -> Resultat<Option<(Value, Vec<Paie>)>> {
    let etats = du_mois(ctx, mois, departs)?;
    let Some(data) = stock::vue_du_mois(&ctx.racine, mois)? else { return Ok(None) };
    let tous = data.get("engagements").and_then(Value::as_array).cloned().unwrap_or_default();
    let mut gardes = Vec::new();
    let mut paies = Vec::new();
    let a_venir: Vec<usize> = if mois > mois_present(ctx).as_str() {
        etats.iter().enumerate().filter(|(_, e)| e.valide().is_some_and(|v| garder(v) && v.paie_ecrite(mois).is_none())).map(|(i, _)| i).collect()
    } else {
        Vec::new()
    };
    let mut calculees = if a_venir.is_empty() { Vec::new() } else { calculer_les_rangs(ctx, mois, &a_venir)? }.into_iter();
    for (i, e) in etats.iter().enumerate() {
        let Some(v) = e.valide().filter(|v| garder(v)) else { continue };
        let paie = match a_venir.contains(&i).then(|| calculees.next()).flatten() {
            Some(p) => Valide { en_cours: Some((mois.to_string(), Arc::new(p))), ..v.clone() }.paie(ctx, &data, i)?,
            None => v.paie(ctx, &data, i)?,
        };
        paies.push(paie);
        gardes.push(tous.get(i).cloned().unwrap_or(Value::Null));
    }
    let mut reduite = data.clone();
    reduite["engagements"] = Value::Array(gardes);
    Ok(Some((reduite, paies)))
}

/// LES PAIES DE `mois`, CE QUE LISENT LES ETATS : la paie ecrite de chaque engagement valide de la saisie qui en a une
/// (`validees_du_mois`), puis celle de chaque engagement importe paye ce mois (`importe::paies_du_mois` : ce que les
/// pieces de l'ancien logiciel en disent, dans les mots du moteur). Un engagement en saisie n'est pas paye ; rien ne se
/// calcule. Ce qui part (DSN, documents) ne lit que la saisie (`validees_du_mois`) : un mois importe a deja ete declare.
pub fn paies_du_mois(ctx: &Contexte, mois: &str, departs: &Departs) -> Resultat<Vec<Paie>> {
    let mut paies: Vec<Paie> = validees_du_mois(ctx, mois, departs)?.map(|(_, p)| p).unwrap_or_default().into_iter().filter(|p| !p.engagement.sans_paie()).collect();
    paies.extend(crate::importe::paies_du_mois(ctx, mois)?);
    Ok(paies)
}

/// LES PAIES VALIDEES DU SALARIE `salarie` (la cle de sa fiche) EN `mois` QU'UN CALCUL LIT (le compteur de conges, le
/// brut de sa periode, la remuneration du contrat, la deduction forfaitaire de l'annee) : la paie de chacun de ses
/// engagements valides du mois, dans l'ordre de l'ecran -- ecrite, ou calculee sans s'ecrire quand son mois ne l'a pas
/// encore ecrite (`valides_lues`). Un engagement en saisie n'y est pas : ce qui n'est pas valide n'est pas paye. Rien ne
/// se reecrit. Pendant une validation, les mois que son engagement a deja calcules s'y lisent comme ecrits
/// (`EnValidation`).
///
/// Le calcul qui les lit note leur empreinte (`paies <mois> <salarie>`, `empreinte_des_paies_validees`), et elle seule :
/// une paie validee de ce salarie qui change fait recalculer ses mois qui l'ont lue ; celle d'un autre salarie, non.
pub fn paies_validees(ctx: &Contexte, mois: &str, salarie: &str) -> Resultat<Vec<Paie>> {
    let Some(saisie) = stock::vue_et_empreinte(&ctx.racine, mois)? else { return Ok(Vec::new()) };
    let data = &saisie.0;
    let valides = valides_lues(ctx, mois, Some(salarie))?;
    socle::fichiers::noter_valeur(&format!("paies {} {}", mois, salarie), empreinte_des_paies(&valides, mois));
    valides.iter().map(|(i, v)| v.paie(ctx, data, *i)).collect()
}

/// L'empreinte des paies validees de `mois` telles qu'un calcul les lit (`paies_validees`) : celles du salarie `salarie`
/// (la cle de sa fiche), ou toutes avec `None` (ce que la DSN du mois declare) ; `None` si elles ne se lisent pas.
pub fn empreinte_des_paies_validees(ctx: &Contexte, mois: &str, salarie: Option<&str>) -> Option<String> {
    valides_lues(ctx, mois, salarie).ok().map(|v| empreinte_des_paies(&v, mois))
}

/// Ce qu'un calcul lit d'une paie validee d'un autre mois : la paie, et le compteur de conges avant son mois.
fn empreinte_des_paies(valides: &[(usize, Valide)], mois: &str) -> String {
    let lu = |p: &Value| json!([p.get("paie"), p.get(CONGES_AVANT)]);
    let paies: Vec<Value> = valides.iter().map(|(_, v)| v.paie_ecrite(mois).map(|p| lu(&p)).unwrap_or(Value::Null)).collect();
    socle::fichiers::sha256_hex(Value::Array(paies).to_string().as_bytes())
}

thread_local! {
    /// Les engagements en cours de validation (`fichier`, `rang`) et les paies deja calculees de leurs mois.
    static EN_VALIDATION: RefCell<Option<Vec<(String, usize, BTreeMap<String, Arc<Value>>)>>> = const { RefCell::new(None) };
}

/// UNE VALIDATION EN COURS : les mois d'un engagement se calculent du premier au dernier, et chacun lit ceux d'avant
/// (`paies_validees`) tels qu'elle vient de les calculer. Elle se ferme quand elle tombe.
struct EnValidation(Option<Vec<(String, usize, BTreeMap<String, Arc<Value>>)>>);

impl EnValidation {
    fn ouvrir(o: &Objet) -> EnValidation {
        EnValidation(EN_VALIDATION.with(|e| e.replace(Some(vec![(o.fichier.clone(), o.rang, BTreeMap::new())]))))
    }

    fn calculee(&self, mois: &str, paie: &Value) {
        noter_calculee(mois, paie);
    }
}

/// La paie de `mois` que le dernier engagement ouvert dans la validation en cours vient de calculer.
fn noter_calculee(mois: &str, paie: &Value) {
    EN_VALIDATION.with(|e| {
        if let Some((_, _, paies)) = e.borrow_mut().as_mut().and_then(|v| v.last_mut()) {
            paies.insert(mois.to_string(), Arc::new(paie.clone()));
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
/// compte pas (sa validation le refusera). Un engagement valide se calcule de meme dans les mois d'apres le present, qui
/// n'ont pas encore sa paie ; ses mois ecrits se lisent tels qu'ils sont (`valides_lues`).
pub(crate) struct CommeALaValidation {
    _garde: Option<EnValidation>,
}

impl CommeALaValidation {
    pub(crate) fn ouvrir(ctx: &Contexte, mois: &str) -> CommeALaValidation {
        if EN_VALIDATION.with(|e| e.borrow().is_some()) {
            return CommeALaValidation { _garde: None };
        }
        let Ok(objets) = stock::objets_du_mois(&ctx.racine, mois) else { return CommeALaValidation { _garde: None } };
        let present = mois_present(ctx);
        let a_calculer: Vec<Objet> = objets.into_iter().filter(|o| o.fichier.as_str() < mois && (!o.est_valide() || mois > present.as_str())).collect();
        if a_calculer.is_empty() {
            return CommeALaValidation { _garde: None };
        }
        let garde = EnValidation(EN_VALIDATION.with(|e| e.replace(Some(Vec::new()))));
        for o in &a_calculer {
            EN_VALIDATION.with(|e| {
                if let Some(v) = e.borrow_mut().as_mut() {
                    v.push((o.fichier.clone(), o.rang, BTreeMap::new()));
                }
            });
            for m in o.mois_couverts(mois).unwrap_or_default().into_iter().filter(|m| m.as_str() < mois) {
                if o.est_valide() && (m <= present || crate::paies::presente(o, &m).ok().flatten().is_some()) {
                    continue;
                }
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

/// Les engagements valides de `mois` (de `salarie`, la cle de sa fiche, s'il est dit) et leur paie de ce mois, avec leur
/// rang a l'ecran : la paie ecrite, ou celle qui se calcule quand son mois ne l'a pas encore ecrite ; un engagement en
/// cours de validation y porte les paies qu'elle a deja calculees, et seulement elles (un engagement deja valide, ses
/// paies ecrites a cote). Ce qui est parti n'y change rien (une paie partie se lit comme une paie validee) : le journal
/// des envois ne se lit pas, et un calcul qui lit ces paies ne depend que d'elles (`paies <mois> <salarie>`).
fn valides_lues(ctx: &Contexte, mois: &str, salarie: Option<&str>) -> Resultat<Vec<(usize, Valide)>> {
    let en_cours = EN_VALIDATION.with(|e| e.borrow().clone()).unwrap_or_default();
    let mut out = Vec::new();
    let mut a_calculer: Vec<(usize, Valide)> = Vec::new();
    for (i, o) in stock::objets_du_mois(&ctx.racine, mois)?.into_iter().enumerate() {
        if salarie.is_some_and(|s| crate::cle_de_l_engagement(ctx, &o.brut).as_deref() != Some(s)) {
            continue;
        }
        let v = match en_cours.iter().find(|(f, r, paies)| o.fichier == *f && o.rang == *r && (paies.contains_key(mois) || !o.est_valide())) {
            Some((_, _, paies)) => {
                let Some(p) = paies.get(mois) else { continue };
                let mut lue = o.clone();
                lue.brut_mut().insert("valide".into(), json!({ "le": "" }));
                Valide::de(lue).map(|v| Valide { en_cours: Some((mois.to_string(), p.clone())), ..v })
            }
            None => Valide::de(o),
        };
        match v {
            Some(v) if v.paie_ecrite(mois).is_some_and(|p| p.get("paie").is_some()) => out.push((i, v)),
            Some(v) if v.paie_ecrite(mois).is_none() => a_calculer.push((i, v)),
            _ => {}
        }
    }
    // UNE PAIE VALIDEE QUE SON MOIS N'A PAS ENCORE ECRITE (personne n'a lu ce mois) se calcule ici, sans s'ecrire : ce
    // qui la lit (la remuneration du contrat d'un mois suivant, ses conges) depend de sa saisie, et son calcul note ce
    // qu'il a lu dans le registre de celui qui la demande (`socle::fichiers::rendre`). Ecrite plus tard, elle porte les
    // memes chiffres, et la meme empreinte (`empreinte_des_paies`).
    if !a_calculer.is_empty() {
        let rangs: Vec<usize> = a_calculer.iter().map(|(i, _)| *i).collect();
        for ((i, v), p) in a_calculer.into_iter().zip(calculer_les_rangs(ctx, mois, &rangs)?) {
            out.push((i, Valide { en_cours: Some((mois.to_string(), Arc::new(p))), ..v }));
        }
        out.sort_by_key(|(i, _)| *i);
    }
    Ok(out)
}

/// Les numeros des documents de chaque engagement valide de `mois`, au meme rang que `validees_du_mois` : le numero de
/// son bulletin, le premier N CE de son certificat, et le bulletin que sa paie rectifie, tels que la paie ecrite les porte.
pub fn numeros_des_documents(ctx: &Contexte, mois: &str, departs: &Departs) -> Resultat<Vec<NumerosEcrits>> {
    Ok(tels_qu_ecrits(ctx, mois, departs)?
        .iter()
        .filter_map(Etat::valide)
        .map(|v| {
            let p = v.paie_ecrite(mois).map_or(Value::Null, |p| (*p).clone());
            NumerosEcrits { bulletin: p.get(BULLETIN).and_then(Value::as_str).map(String::from), premier_ce: p.get(CE).and_then(Value::as_i64), rectifie: p.get(RECTIFIE).and_then(Value::as_str).map(String::from) }
        })
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
    let cle = p.salarie.id.to_string();
    let fin = e.fin_periode()?;
    let taux = droit::pas::taux_dgfip(ctx, &cle, Some(e.date_du_versement()?), Some(&e.mois))?;
    let bareme = ctx.bareme(&Jour { an: fin.an, mois: fin.mois, jour: 1 })?;
    let t = |j: Jour| (j.an, j.mois, j.jour);
    let court = droit::natures::contrat_court(&*e.nat()?, t(e.debut), e.fin.map(t), t(fin));
    // « Dans le cas d'une avance faite par un employeur en cas de subrogation des indemnites journalieres : le montant de
    // l'avance s'ajoute a la RNF pour constituer l'assiette du PAS » (S21.G00.50.013, `norme-dsn/texte/bloc-S21.G00.50.txt`) ;
    // cette avance-la, ce que le BOFiP en soumet (`maintien`, `IJ_ASSIETTE_PAS`).
    let avance = p.bases.get(crate::maintien::IJ_ASSIETTE_PAS).copied().unwrap_or(Decimal::ZERO);
    droit::pas::du_versement(taux.as_ref(), &bareme, p.net_imposable + avance, Some(fin), court, droit::pas::ZONE_DEFAUT)
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
    let versement = socle::mois::Mois::lire(mois_du_versement)?;
    let (debut, fin) = (versement.premier_jour(), versement.dernier_jour());
    let difference = socle::num::sub(apres.net_imposable, net_imposable_declare);
    let e = &apres.engagement;
    let cle = apres.salarie.id.to_string();
    let taux = droit::pas::taux_dgfip(ctx, &cle, Some(fin), Some(mois_du_versement))?;
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

/// Ce que la saisie `data` d'un mois pose pour tous ses engagements : elle-meme, sans ses engagements ni ce que le moteur
/// tient (comme `stock::empreinte_de_saisie` la separe).
fn mois_lu(data: &Value) -> Value {
    let mut m = data.as_object().cloned().unwrap_or_default();
    for k in ["engagements", "_origine", "mois"] {
        m.remove(k);
    }
    Value::Object(m)
}

/// Les paies des engagements de rangs `rangs` dans l'ecran de `mois`, d'un seul calcul du mois, au contexte du mois :
/// chacune `{"paie", "fiches", "saisie_du_mois", "pas", "empreinte"}`, dans l'ordre de `rangs`, avec l'empreinte de ce
/// dont son calcul depend.
///
/// UNE PAIE ECRITE NE LIT QUE DES ENGAGEMENTS VALIDES : le calcul se fait sur les engagements valides du mois et ceux
/// de `rangs`, comptes comme valides (`stock::empreinte_de_saisie` ne compte que ceux-la) ; un engagement en saisie
/// n'entre ni dans le calcul ni dans l'empreinte d'une paie ecrite. DES AUTRES ENGAGEMENTS DU MOIS, ELLE NE LIT QUE CEUX
/// DE SON SALARIE : le calcul ne prend qu'eux, et ce qui refuse la paie d'un salarie n'empeche pas celle d'un autre.
fn calculer_les_rangs(ctx: &Contexte, mois: &str, rangs: &[usize]) -> Resultat<Vec<Value>> {
    let ctx = &ctx.du_mois(mois)?;
    let mut data = stock::vue_du_mois(&ctx.racine, mois)?.ok_or_else(|| format!("{} : aucune saisie", mois))?;
    let tous: Vec<Value> = data.get("engagements").and_then(Value::as_array).cloned().unwrap_or_default();
    let salaries: Vec<String> = rangs.iter().map(|i| tous.get(*i).and_then(|e| e.get("salarie")).map(en_texte).unwrap_or_default()).collect();
    let mut gardes = Vec::new();
    let mut places = vec![None; tous.len()];
    for (i, mut e) in tous.into_iter().enumerate() {
        if rangs.contains(&i) && e.get("valide").is_none() {
            e["valide"] = json!({});
        }
        let des_siens = e.get("salarie").map(en_texte).is_some_and(|s| salaries.contains(&s));
        if des_siens && e.get("valide").is_some() {
            places[i] = Some(gardes.len());
            gardes.push(e);
        }
    }
    data["engagements"] = Value::Array(gardes);
    let (calculees, mut entrees) = empreinte::suivre(ctx, || {
        let s = crate::charger_donnees(ctx, &data)?;
        rangs
            .iter()
            .map(|i| {
                let p = places.get(*i).copied().flatten().and_then(|j| s.paies.get(j)).ok_or("le calcul du mois n'a pas cette paie")?;
                let conges = crate::conges::compteur_ecrit(ctx, p, mois)?;
                let (salarie, spectacle) = (&p.engagement.salarie, p.engagement.spectacle.as_deref());
                let config = empreinte::configuration(ctx, &p.engagement.fin_periode()?, salarie, spectacle)?;
                let entree_fiches = empreinte::fiches(ctx, mois, salarie, spectacle)?;
                Ok((relire::ecrire(ctx, p)?, relire::fiches_du_calcul(p), prelevement(ctx, p)?.map(|v| v.canon()), conges, p.engagement.fin_periode()?, config, entree_fiches))
            })
            .collect::<Resultat<Vec<_>>>()
    })?;
    // DES AUTRES ENGAGEMENTS DE SON MOIS, UNE PAIE NE LIT QUE CEUX DE SON SALARIE (`calcul::dfs_deja_deduite`, sur
    // `precedentes`) ; `un_numero_un_salarie` refuse sans changer un chiffre. Des autres mois, elle ne lit que les paies
    // de son salarie (`paies <mois> <salarie>`) : le calcul du mois les a lues pour tous.
    entrees.remove(&stock::cle_de_saisie(mois, None));
    let mois_lu = mois_lu(&data);
    calculees
        .into_iter()
        .zip(salaries)
        .map(|((paie, fiches, pas, conges, fin, config, (cle_fiches, entree_fiches)), salarie)| {
            let regles = empreinte::regles(ctx, &fin)?;
            let cle = ctx.cle_du_salarie(&salarie).map_or_else(|_| salarie.clone(), String::from);
            let mut siennes = entrees.clone();
            siennes.retain(|k, _| k.strip_prefix("paies ").and_then(|r| r.split_once(' ')).is_none_or(|(_, s)| s == cle));
            siennes.retain(|k, _| k.strip_prefix(crate::conges::MENSUELS).and_then(|r| r.split_once(' ')).is_none_or(|(_, s)| s == cle));
            // Le taux de PAS d'un autre salarie n'est pas de son calcul.
            siennes.retain(|k, _| k.strip_prefix("pas ").and_then(|r| r.split_once(' ')).is_none_or(|(s, _)| s == cle));
            siennes.insert(stock::cle_de_saisie(mois, Some(&salarie)), stock::empreinte_de_saisie(&data, Some(&salarie)));
            siennes.insert(cle_fiches, entree_fiches);
            let mut v = json!({ "paie": paie, "fiches": fiches, MOIS_LU: mois_lu.clone(), "empreinte": empreinte::en_json(&siennes, &regles, &config) });
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

/// Deux paies ecrites disent-elles les memes chiffres : tout sauf ce qu'elles disent en mots ? Rien ne se recopie : ce
/// qui est a refaire se compare a chaque lecture de l'etat d'un mois, pour chaque envoi parti.
fn chiffres_egaux(a: &Value, b: &Value) -> bool {
    egales_sans_avis(champ(a, "paie"), champ(b, "paie")) && champ(a, "pas") == champ(b, "pas") && egales_sans_avis(champ(a, CONGES), champ(b, CONGES))
}

/// Le champ `k` d'une paie ecrite, `null` s'il manque.
fn champ<'a>(v: &'a Value, k: &str) -> &'a Value {
    v.get(k).unwrap_or(&Value::Null)
}

/// Deux valeurs egales hors de leurs `avertissements`, a toute profondeur.
fn egales_sans_avis(a: &Value, b: &Value) -> bool {
    const AVIS: &str = "avertissements";
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            let cles = |m: &Map<String, Value>| m.keys().filter(|k| *k != AVIS).count();
            cles(x) == cles(y) && x.iter().filter(|(k, _)| *k != AVIS).all(|(k, v)| y.get(k).is_some_and(|w| egales_sans_avis(v, w)))
        }
        (Value::Array(x), Value::Array(y)) => x.len() == y.len() && x.iter().zip(y).all(|(v, w)| egales_sans_avis(v, w)),
        _ => a == b,
    }
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
    let sans_avis = |k: &str| egales_sans_avis(champ(avant, k), champ(apres, k));
    if sans_avis("paie") && sans_avis(CONGES) && champ(avant, "pas") != champ(apres, "pas") {
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
    // Le code a change : le moteur, ou un fichier livre que les deux calculs ont lu. Un fichier livre que seul l'un des
    // deux a lu dit que le calcul a pris un autre chemin (un arret saisi lit le bareme des arrets), pas que le code a change.
    let code = changees.iter().any(|k| *k == "moteur" || (k.split_once(' ').is_some_and(|(_, c)| c.starts_with("code:")) && ea.contains_key(*k) && eb.contains_key(*k)));
    if code && parti {
        return CALCUL_CORRIGE.to_string();
    }
    // La saisie dont la paie depend : `<le mois>:<les engagements de son salarie>` (`stock::empreinte_de_saisie`), lue
    // des deux cotes ; une saisie que l'une des deux paies n'a pas lue ne dit pas qu'un engagement a change.
    let saisies: Vec<&String> = changees.iter().copied().filter(|k| k.starts_with("saisie ") && ea.contains_key(*k) && eb.contains_key(*k)).collect();
    let part = |e: &BTreeMap<String, String>, k: &str, i: usize| e.get(k).and_then(|v| v.split(':').nth(i).map(String::from));
    if saisies.iter().any(|k| part(&ea, k, 1) != part(&eb, k, 1)) || changees.iter().any(|k| k.starts_with(crate::conges::MENSUELS)) {
        return AUTRE_ENGAGEMENT.to_string();
    }
    if !saisies.is_empty() {
        return SAISIE_DU_MOIS.to_string();
    }
    let imprevus = format!("racine:{}", crate::imprevus::DOSSIER);
    if changees.iter().any(|k| k.split_once(' ').is_some_and(|(_, c)| c.starts_with(&imprevus))) {
        return IMPREVU.to_string();
    }
    // L'indemnite de mission d'un jour a l'etranger que la table livree dit autrement : le code qui calcule.
    if changees.iter().any(|k| k.starts_with("mission ")) {
        return CALCUL_CORRIGE.to_string();
    }
    // Les paies validees d'un autre mois que le calcul lit (conges, cumuls) : `paies <le mois> <son salarie>`.
    let mut mois: Vec<&str> = changees.iter().filter_map(|k| k.strip_prefix("paies ")?.split(' ').next()).collect();
    mois.sort();
    if let Some(m) = mois.first() {
        return format!("la paie {} a changé", socle::texte::de(&socle::mois::Mois::en_clair(m)));
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

/// Le moment d'un geste, a l'heure du moteur.
fn maintenant() -> String {
    socle::paris::iso_secondes(socle::temps::secondes())
}


/// La saisie d'un engagement telle qu'une version la garde : l'objet, sans ce que le moteur tient (ses termes dates et
/// les faits de ses mois, qui sont de la saisie, y restent).
fn saisie_de(o: &Objet) -> Value {
    Value::Object(
        o.champs()
            .iter()
            .filter(|(k, _)| stock::SAISIS_PAR_MOIS.contains(&k.as_str()) || !stock::TENUS_PAR_LE_MOTEUR.contains(&k.as_str()))
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
fn memes_chiffres(a: &Map<String, Value>, b: &Map<String, Value>) -> bool {
    a.iter().all(|(m, p)| b.get(m).is_some_and(|q| chiffres_egaux(p, q) && lu(p) == lu(q)))
}

/// LA VERSION QUE PREND LE PRESENT quand la validation `actuelle` de `o` est remplacee le `le` : si un envoi est parti
/// sur elle (`siens`), elle se garde, datee, dans les versions de l'engagement, ses paies sous son numero dans le fichier
/// de leur mois (`paies::garder_la_version`), et le present prend le numero suivant ; sinon le present garde son numero
/// `n`, sans historique.
fn nouvelle_version(o: &mut Objet, actuelle: &Value, n: u64, siens: &[Depart], le: &str) -> Resultat<u64> {
    if !siens.iter().any(|d| d.version == n) {
        return Ok(n);
    }
    crate::paies::garder_la_version(o, n)?;
    let passee = json!({
        "version": n, "valide_le": actuelle.get("le").cloned().unwrap_or(Value::Null), "remplacee_le": le, "saisie": actuelle["saisie"],
    });
    match o.brut_mut().get_mut(VERSIONS).and_then(Value::as_array_mut) {
        Some(v) => v.push(passee),
        None => {
            o.brut_mut().insert(VERSIONS.into(), json!([passee]));
        }
    }
    Ok(n + 1)
}

/// « VALIDER L'ENGAGEMENT » : l'engagement de rang `rang` dans l'ecran de `mois` ecrit sa saisie telle qu'elle est, et
/// ses paies, une par mois qu'il couvre jusqu'au present (`mois_present`), ou court un contrat sans terme ; un mois
/// d'apres aura la sienne quand il sera le present. Chacune avec son
/// empreinte, dans le fichier de son mois (`paies`) ; une paie de la validation d'avant dont le mois n'est plus couvert
/// s'efface. La paie d'un mois dont les documents sont partis se reprend de la validation d'avant (`precedente`) si la
/// saisie de son mois n'a pas change. Une validation qui change ce qu'un envoi a emporte fait une version
/// (`nouvelle_version`) ; une validation qui ne change rien garde le numero. Refus : un mois qui ne se calcule pas ; rien
/// n'est ecrit.
pub fn valider(ctx: &Contexte, mois: &str, rang: usize, departs: &Departs) -> Resultat<Valide> {
    let mut o = stock::objet_du_mois(&ctx.racine, mois, rang)?.ok_or_else(|| format!("{} : pas d'engagement de rang {}", mois, rang + 1))?;
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
    // Une validation qui ne dit pas sa version ou sa saisie (`a_revalider`) ne se reprend pas : celle-ci s'ecrit entiere,
    // en version 1.
    let deja = o.brut.get(PRECEDENTE).or_else(|| o.brut.get("valide")).cloned();
    let precedente = deja.clone().filter(|p| a_revalider(ctx, &o, p).is_none());
    // Les paies de la validation d'avant, sous le numero qu'elle portait.
    let anciennes = if deja.is_some() { crate::paies::presentes(&avant)? } else { Map::new() };
    let gardees = if precedente.is_some() { anciennes.clone() } else { Map::new() };
    let paies = match paies_a_valider(ctx, &o, mois, &gardees, &siens) {
        Ok(p) => p,
        Err(e) => {
            if numerote {
                stock::reecrire(&ctx.racine, &avant)?;
            }
            return Err(e);
        }
    };
    let saisie = saisie_de(&o);
    // Une validation qui ne change rien garde sa version et la date de celle-ci : la version est la meme.
    let (version, le) = match precedente.as_ref().and_then(|p| Some((p, version_de(Some(p))?))) {
        Some((p, n)) if p.get("saisie") != Some(&saisie) || !memes_chiffres(&gardees, &paies) => (nouvelle_version(&mut o, p, n, &siens, &le)?, le),
        Some((p, n)) => (n, p.get("le").and_then(Value::as_str).map_or(le, String::from)),
        None => (1, le),
    };
    // Sous un autre numero, la validation d'avant n'a plus de paies ; sous le meme, ses mois que celle-ci ne couvre plus.
    if deja.is_some() && avant.brut.get("numero") != o.brut.get("numero") {
        crate::paies::effacer(&avant)?;
    } else {
        for m in anciennes.keys().filter(|m| !paies.contains_key(*m)) {
            crate::paies::poser(&o, m, None)?;
        }
    }
    // Le premier mois que la validation a refait (ou ecrit) : ce qui le suit reprend son report.
    let premier_refait = paies.iter().find(|(m, p)| anciennes.get(*m) != Some(*p)).map(|(m, _)| m.clone());
    for (m, p) in paies {
        crate::paies::poser(&o, &m, Some(p))?;
    }
    o.brut_mut().remove(PRECEDENTE);
    o.brut_mut().insert("valide".into(), json!({ "le": le, "version": version, "saisie": saisie }));
    stock::reecrire(&ctx.racine, &o)?;
    if let Some(premier) = premier_refait {
        reporter(ctx, &o, &premier, departs)?;
    }
    Valide::de(o).ok_or_else(|| "validation illisible".to_string())
}

/// Les paies que la validation de `o` ecrit, une par mois qu'il couvre jusqu'au present (`mois_present`). `anciennes` : celles de la
/// validation qu'elle remplace ; `siens` : ce qui est parti de l'engagement.
fn paies_a_valider(ctx: &Contexte, o: &Objet, mois: &str, anciennes: &Map<String, Value>, siens: &[Depart]) -> Resultat<Map<String, Value>> {
    let mut paies = Map::new();
    let en_cours = EnValidation::ouvrir(o);
    let present = mois_present(ctx);
    // Un contrat sans terme court jusqu'au present au moins.
    for m in o.mois_couverts(&present.clone().max(mois.to_string()))?.into_iter().filter(|m| *m <= present) {
        let partis = documents_partis(siens, &m).is_some();
        let garde = anciennes.get(&m).filter(|f| {
            let salarie = o.brut.get("salarie").map(en_texte);
            let saisie = saisie_comptee_valide(ctx, o, &m, salarie.as_deref());
            let ecrite = f.get("empreinte").and_then(empreinte::entrees_de).and_then(|e| e.get(&stock::cle_de_saisie(&m, salarie.as_deref())).cloned());
            partis && saisie.is_some() && saisie == ecrite && imprevus_tiennent(ctx, f)
        });
        let p = match garde {
            Some(f) => f.clone(),
            None => {
                let mut p = calculer(ctx, o, &m)?;
                match anciennes.get(&m).filter(|_| partis) {
                    // Une paie partie, refaite aux memes chiffres : le bulletin parti reste juste, elle garde ses numeros.
                    Some(f) if chiffres_egaux(f, &p) => {
                        for k in [BULLETIN, CE, RECTIFIE] {
                            if let Some(x) = f.get(k) {
                                p[k] = x.clone();
                            }
                        }
                    }
                    // Une paie partie, refaite : son bulletin sera un rectificatif de celui qui est parti.
                    Some(f) => {
                        if let Some(parti) = f.get(BULLETIN).or_else(|| f.get(RECTIFIE)).filter(|b| b.is_string()) {
                            p[RECTIFIE] = parti.clone();
                        }
                    }
                    None => {}
                }
                p
            }
        };
        en_cours.calculee(&m, &p);
        paies.insert(m, p);
    }
    Ok(paies)
}
/// Les imprevus que la paie ecrite `f` a lus (le magasin, `imprevus::DOSSIER`) valent-ils encore ce qu'ils valaient ?
/// Un arret saisi ou corrige apres le depart du mois les change : la paie partie se refait alors a la revalidation, et
/// l'ecart passe en regularisation (`apres_un_imprevu`).
fn imprevus_tiennent(ctx: &Contexte, f: &Value) -> bool {
    let magasin = format!("racine:{}", crate::imprevus::DOSSIER);
    let entrees: BTreeMap<String, String> = f
        .get("empreinte")
        .and_then(empreinte::entrees_de)
        .unwrap_or_default()
        .into_iter()
        .filter(|(k, _)| k.split_once(' ').is_some_and(|(_, c)| c.starts_with(&magasin)))
        .collect();
    empreinte::tient(ctx, &entrees, &mut empreinte::Relues::default())
}

/// Ce que la revalidation d'un engagement, apres un imprevu change (`apres_un_imprevu`), a donne.
pub enum Revalidation {
    /// Une paie a change : l'ecart d'une paie partie se regularise
    /// (`a_refaire` : le bulletin rectificatif, la DSN qui le porte).
    Changee(Valide),
    /// La paie partie ne se calcule pas encore (une indemnite a dire...) : qui, et le refus du calcul. Rien n'est ecrit
    /// pour lui ; la revalidation se refait au geste suivant sur l'imprevu.
    Attend(String, String),
}

/// UN IMPREVU CHANGE APRES LE DEPART DU MOIS (un arret saisi, ses indemnites corrigees, une absence) : chaque engagement
/// valide de `salarie` dont des documents sont partis se revalide (`valider`), comme une modification, dans son
/// brouillon. Une paie partie dont les imprevus ont change se refait (`imprevus_tiennent`) : si un chiffre change,
/// l'engagement prend une version nouvelle, la version d'avant garde ce qui est parti, et l'ecart suit le chemin de
/// toute correction d'un mois parti (un bulletin rectificatif verse avec une paie suivante ; la DSN du mois tant que
/// l'annule-et-remplace est admis, sinon la DSN qui porte la correction). Le mois parti ne se redepose pas. Rend les
/// engagements dont une paie a change, et ceux qui attendent.
pub fn apres_un_imprevu(ctx: &Contexte, salarie: &str, departs: &Departs) -> Resultat<Vec<Revalidation>> {
    let mut out = Vec::new();
    for o in stock::objets(&ctx.racine)? {
        if !o.est_valide() || o.est_non_actif() || o.est_importe() || crate::cle_de_l_engagement(ctx, &o.brut).as_deref() != Some(salarie) {
            continue;
        }
        if !ses_departs(ctx, &o, departs).iter().any(|d| d.envoi == Envoi::Documents) {
            continue;
        }
        let mois = o.premier_mois().to_string();
        let rang = rang_dans(ctx, &o, &mois)?;
        let avant = crate::paies::presentes(&o)?;
        let brouillon = stock::Brouillon::ouvrir(&ctx.racine);
        match valider(ctx, &mois, rang, departs) {
            Ok(v) => {
                brouillon.ecrire()?;
                if !memes_chiffres(&avant, &crate::paies::presentes(v.objet())?) {
                    out.push(Revalidation::Changee(v));
                }
            }
            Err(m) => out.push(Revalidation::Attend(qui_de(ctx, &o), m)),
        }
    }
    Ok(out)
}

/// L'empreinte de la saisie de `mois` dont la paie de l'engagement `o` (de `salarie`) depend, `o` compte comme valide,
/// comme son calcul le compte (`calculer_les_rangs`) : celle qu'une paie ecrite de `o` porte si ce que le mois dit d'elle
/// n'a pas change.
fn saisie_comptee_valide(ctx: &Contexte, o: &Objet, mois: &str, salarie: Option<&str>) -> Option<String> {
    let mut data = stock::vue_du_mois(&ctx.racine, mois).ok().flatten()?;
    let i = rang_dans(ctx, o, mois).ok()?;
    let e = data.get_mut("engagements").and_then(|es| es.get_mut(i))?;
    if e.get("valide").is_none() {
        e["valide"] = json!({});
    }
    Some(stock::empreinte_de_saisie(&data, salarie))
}

/// L'engagement de rang `rang` de l'ecran de `mois`, rouvert (`modifier`), est-il a revalider ? Sa saisie dit autre
/// chose que sa validation mise de cote, ou celle-ci ne dit pas tout ce qu'un envoi emporte (`a_revalider`). Faux pour un
/// engagement qui n'a pas ete rouvert.
pub fn a_revalider_rouvert(ctx: &Contexte, mois: &str, rang: usize) -> Resultat<bool> {
    let Some(o) = stock::objet_du_mois(&ctx.racine, mois, rang)? else { return Ok(false) };
    let Some(p) = o.brut.get(PRECEDENTE) else { return Ok(false) };
    Ok(p.get("saisie") != Some(&saisie_de(&o)) || a_revalider(ctx, &o, p).is_some())
}

/// LE MOIS OU LE CONTRAT CHANGE entre la version `passee` d'un engagement valide (sa saisie gardee) et son present : le
/// premier mois dont les clauses (`CLAUSES`) different. C'est de ce mois que l'avenant prend effet (R1221-40 : « au plus
/// tard a la date de prise d'effet de cette modification », `atelier/reference/sources/code-du-travail/
/// articles-20261029.md`, l. 856). `None` : aucun mois ne change.
pub fn mois_du_changement(v: &Valide, passee: &Value) -> Resultat<Option<String>> {
    let Some(saisie) = passee.get("saisie").and_then(Value::as_object) else { return Ok(None) };
    let ancien = v.objet.d_apres_la_saisie(saisie);
    let mensuel = v.objet.mensuel();
    // Les mois ou l'une des versions a une paie (`paies::mois_ecrits`), et ceux que l'une des saisies a touches.
    let mut mois: Vec<String> = crate::paies::mois_ecrits(&v.objet)?
        .into_iter()
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
    if let Some(v) = o.brut_mut().remove("valide") {
        o.brut_mut().insert(PRECEDENTE.into(), v);
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
    crate::paies::effacer(o)?;
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
    let derniere = o.brut_mut().remove("valide");
    // Sa version suit la derniere validee : son present, rien de paye, est une version de plus.
    let version = o.brut.get(VERSIONS).and_then(Value::as_array).into_iter().flatten().chain(derniere.as_ref()).filter_map(|v| version_de(Some(v))).max().unwrap_or(0) + 1;
    if let Some(v) = derniere.filter(|v| version_de(Some(v)).is_some()) {
        if let Some(n) = version_de(Some(&v)) {
            crate::paies::garder_la_version(&o, n)?;
        }
        let gardee = json!({
            "version": v["version"], "valide_le": v.get("le").cloned().unwrap_or(Value::Null), "remplacee_le": le,
            "saisie": v.get("saisie").cloned().unwrap_or(Value::Null),
        });
        match o.brut_mut().get_mut(VERSIONS).and_then(Value::as_array_mut) {
            Some(vs) => vs.push(gardee),
            None => {
                o.brut_mut().insert(VERSIONS.into(), json!([gardee]));
            }
        }
    }
    // Rien de paye : il n'a plus de paie presente.
    for m in crate::paies::mois_ecrits(&o)? {
        crate::paies::poser(&o, &m, None)?;
    }
    o.brut_mut().insert(stock::NON_ACTIF.into(), json!({ "le": le, "version": version }));
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
/// Un taux de PAS recu, une configuration, une autre saisie ne touchent pas un mois parti : un premier taux personnalise
/// recu apres la grille « ne s'applique que pour l'avenir », sans regularisation (BOI-IR-PAS-30-10-50, § 20,
/// `atelier/reference/sources/bofip/BOI-IR-PAS-30-10-50-20190227.html`). Un mois fini (`fini`) ne se regarde qu'au
/// passage d'une version (`passage`, `corriger_les_mois_finis`) ; un mois d'apres le present n'a pas de paie
/// (`mois_present`).
fn rafraichir(ctx: &Contexte, mois: &str, departs: &Departs, passage: bool) -> Resultat<Rafraichi> {
    // Au contexte du mois, quel que soit le jour de celui qui lit : ce que l'empreinte relit (les fiches) vaut au mois paye.
    let present = mois_present(ctx);
    let ctx = &ctx.du_mois(mois)?;
    // Ce qui ne tient plus : le rang dans l'ecran, l'engagement, sa paie ecrite du mois, et si ses documents sont partis.
    let mut a_refaire: Vec<(usize, Objet, Option<Value>, bool)> = Vec::new();
    let mut relues = empreinte::Relues::default();
    let mut reecrit = false;
    // CE DONT UNE PAIE DEPEND SE LIT TEL QU'IL EST ECRIT : la paie d'un mois d'avant qu'elle a lue (`paies <mois>
    // <salarie>`) se compare a son empreinte sans se recalculer d'abord. Qu'elle se reecrive, et l'empreinte de celle-ci
    // ne tient plus : elle se refait a sa prochaine lecture.
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
        // UN MOIS FINI NE SE RELIT PAS : sa paie se lit telle qu'elle est ecrite (`fini`), sauf au passage d'une version
        // (`corriger_les_mois_finis`), ou il est un mois parti comme un autre.
        if ancienne.is_some() && !passage && fini(ses_departs(ctx, &v.objet, departs), mois) {
            continue;
        }
        // AUCUNE PAIE POUR UN MOIS QUI N'EST PAS ENCORE LA : elle s'ecrira quand il sera le present, avec ses regles.
        if ancienne.is_none() && mois > present.as_str() {
            continue;
        }
        let empreinte_ecrite = ancienne.as_ref().and_then(|a| a.get("empreinte"));
        // Partie, elle ne bouge que par une decision de l'editeur : on la cherche d'abord (quelques entrees), et
        // l'empreinte entiere ne se verifie que si elle a lieu.
        if parti && !empreinte_ecrite.is_some_and(|e| empreinte::code_ou_regle_change(ctx, e, mois, &mut relues)) {
            // Partie, elle ne bouge pas ; le compteur avant le mois, que le mois suivant reprend, suit les mois d'avant.
            if let Some(a) = ancienne.as_ref().filter(|a| a.get(CONGES_AVANT).is_some()) {
                if let Some(nouvelle) = conges_avant_a_jour(ctx, &v.objet, a, mois, &mut relues)? {
                    crate::paies::poser(&v.objet, mois, Some(nouvelle))?;
                    reecrit = true;
                }
            }
            continue;
        }
        let tient = empreinte_ecrite.and_then(empreinte::entrees_de).is_some_and(|en| empreinte::tient(ctx, &en, &mut relues));
        if tient {
            continue;
        }
        a_refaire.push((i, v.objet.clone(), ancienne.as_deref().cloned(), parti));
    }
    if a_refaire.is_empty() {
        return Ok(Rafraichi { etats: (!reecrit).then_some(etats), refus: Vec::new() });
    }
    // UN CALCUL PAR SALARIE (`calculer_les_rangs`) : le refus de l'un est le sien, les paies des autres s'ecrivent.
    let mut salaries: Vec<String> = Vec::new();
    for (_, o, _, _) in &a_refaire {
        let s = o.brut.get("salarie").map(en_texte).unwrap_or_default();
        if !salaries.contains(&s) {
            salaries.push(s);
        }
    }
    let mut refus: Vec<(usize, String)> = Vec::new();
    let mut calculees: Vec<((usize, Objet, Option<Value>, bool), Value)> = Vec::new();
    for s in &salaries {
        let siens: Vec<(usize, Objet, Option<Value>, bool)> =
            a_refaire.iter().filter(|(_, o, _, _)| o.brut.get("salarie").map(en_texte).as_deref() == Some(s.as_str())).cloned().collect();
        let rangs: Vec<usize> = siens.iter().map(|(i, _, _, _)| *i).collect();
        match calculer_les_rangs(ctx, mois, &rangs) {
            Ok(nouvelles) => calculees.extend(siens.into_iter().zip(nouvelles)),
            Err(m) => refus.extend(rangs.into_iter().map(|i| (i, m.clone()))),
        }
    }
    for ((_, mut o, ancienne, parti), mut nouvelle) in calculees {
        let raison = match &ancienne {
            Some(a) if !chiffres_egaux(a, &nouvelle) => Some(raison(a, &nouvelle, parti)),
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
                let suivante = nouvelle_version(&mut o, actuelle, n, &emportee, &le)?;
                if suivante != n {
                    if let Some(v) = o.brut_mut().get_mut("valide").and_then(Value::as_object_mut) {
                        v.insert("version".into(), json!(suivante));
                    }
                    stock::reecrire(&ctx.racine, &o)?;
                }
            }
        }
        crate::paies::poser(&o, mois, Some(nouvelle))?;
    }
    Ok(Rafraichi { etats: None, refus })
}

/// LE REPORT SUIT CE QUI LE PRECEDE, A L'ECRITURE : la validation de l'engagement `o` a refait sa paie de `premier` ;
/// chaque paie partie de son salarie dans les mois d'apres, jusqu'au present, du plus ancien au plus recent, remet a jour
/// l'etat qu'elle tient pour le mois suivant (`conges_avant_a_jour`) : un mois fini ne se relit pas, et le premier mois
/// en cours reprend un etat juste. Une paie qui n'est pas partie se refait a sa lecture (son empreinte ne tient plus).
fn reporter(ctx: &Contexte, o: &Objet, premier: &str, departs: &Departs) -> Resultat<()> {
    let Some(cle) = crate::cle_de_l_engagement(ctx, &o.brut) else { return Ok(()) };
    let present = mois_present(ctx);
    for mois in crate::paies::mois_payes(&ctx.racine).into_iter().filter(|m| m.as_str() > premier && *m <= present) {
        let ctx = &ctx.du_mois(&mois)?;
        let mut relues = empreinte::Relues::default();
        for e in tels_qu_ecrits(ctx, &mois, departs)? {
            let Etat::Partis(p) = e else { continue };
            let v = p.valide();
            if crate::cle_de_l_engagement(ctx, &v.objet.brut).as_ref() != Some(&cle) {
                continue;
            }
            if let Some(a) = v.paie_ecrite(&mois).filter(|a| a.get(CONGES_AVANT).is_some()) {
                if let Some(nouvelle) = conges_avant_a_jour(ctx, &v.objet, &a, &mois, &mut relues)? {
                    crate::paies::poser(&v.objet, &mois, Some(nouvelle))?;
                }
            }
        }
    }
    Ok(())
}

/// LE COMPTEUR AVANT LE MOIS D'UNE PAIE PARTIE, a jour des mois d'avant : la paie partie ne bouge pas, mais l'etat
/// qu'elle tient pour le mois suivant (`CONGES_AVANT`) suit ce dont il depend (les paies validees des mois qu'il lit,
/// les fiches). `None` : il tient. Sinon, la paie ecrite avec l'etat refait, et ce qu'il a lu dans son empreinte.
fn conges_avant_a_jour(ctx: &Contexte, o: &Objet, ancienne: &Value, mois: &str, relues: &mut empreinte::Relues) -> Resultat<Option<Value>> {
    let entrees = ancienne.get("empreinte").and_then(empreinte::entrees_de).unwrap_or_default();
    let de_lui = |k: &str| k.starts_with("paies ") || k.starts_with("fiches ");
    let siennes: BTreeMap<String, String> = entrees.iter().filter(|(k, _)| de_lui(k)).map(|(k, v)| (k.clone(), v.clone())).collect();
    if empreinte::tient(ctx, &siennes, relues) {
        return Ok(None);
    }
    let cle = o.brut.get("salarie").map(en_texte).unwrap_or_default();
    let fin = socle::mois::Mois::lire(mois)?.dernier_jour();
    let (avant, mut lues) = empreinte::suivre(ctx, || crate::conges::avant_le_mois(ctx, &cle, &fin, mois))?;
    let spectacle = o.brut.get("spectacle").and_then(Value::as_str);
    lues.extend([empreinte::fiches(ctx, mois, &cle, spectacle)?]);
    let mut nouvelle = ancienne.clone();
    nouvelle[CONGES_AVANT] = avant.en_json();
    let mut entrees: Map<String, Value> = entrees.into_iter().filter(|(k, _)| !de_lui(k)).map(|(k, v)| (k, Value::String(v))).collect();
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
pub(crate) const CLAUSES: [&str; 18] = [
    "salarie", "spectacle", "debut", "fin", "numero", "lignes", "lieu", "nature", "motif_recours", "quotite_contrat",
    "quotite_reference", "modalite_temps_travail", "salaire_mensuel", "emploi", "metier", "cadre", "statut", "planning",
];

/// Les clauses d'une saisie gardee (`saisie_de`) : celles de son premier mois, chaque changement d'une clause, et
/// celles que les faits de chaque mois d'apres disent.
fn clauses(saisie: &Value, mensuel: bool) -> Value {
    let garder = |m: &Map<String, Value>| -> Map<String, Value> {
        m.iter().filter(|(k, _)| CLAUSES.contains(&k.as_str()) && !(mensuel && k.as_str() == "lignes")).map(|(k, v)| (k.clone(), v.clone())).collect()
    };
    let vide = Map::new();
    let par_mois = |k: &str| saisie.get(k).and_then(Value::as_object).unwrap_or(&vide);
    let faits: Map<String, Value> = par_mois(stock::FAITS).iter().map(|(m, x)| (m.clone(), Value::Object(garder(x.as_object().unwrap_or(&vide))))).collect();
    json!({ "premier": garder(saisie.as_object().unwrap_or(&vide)), "termes": garder(par_mois(stock::DATES)), "faits": faits })
}

/// Ce qu'un envoi parti sur une version passee demande de refaire, le present ayant change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ARefaire {
    /// Le contrat (ou son dernier avenant) est parti sur la version `depuis`, et ses clauses ont change : un avenant a
    /// faire signer. `None` : le contrat a ete signe hors du logiciel (`crate::signe_ailleurs`), aucun avenant n'est
    /// parti, et un de ses termes a change depuis le premier mois (`stock::DATES`).
    Avenant { depuis: Option<u64> },
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
    a_refaire_parmi(ctx, v, departs, &|_| true)
}

/// `a_refaire`, pour les seules sortes d'envoi que `sorte` retient : ce qui ne se demande pas ne se compare pas (les paies
/// des documents et des DSN partis, pour qui ne cherche que l'avenant d'un contrat).
pub fn a_refaire_parmi(ctx: &Contexte, v: &Valide, departs: &Departs, sorte: &dyn Fn(Envoi) -> bool) -> Vec<ARefaire> {
    let siens = ses_departs(ctx, &v.objet, departs);
    let present = v.version();
    let actuelle = v.objet.brut.get("valide").cloned().unwrap_or(Value::Null);
    let mut derniers: BTreeMap<(u8, Option<String>), &Depart> = BTreeMap::new();
    for d in siens.iter().filter(|d| sorte(d.envoi)) {
        let rang = match d.envoi {
            Envoi::Contrat => 0,
            Envoi::Dpae => 1,
            Envoi::Documents => 2,
            Envoi::Dsn => 3,
            Envoi::Signalement => 4,
        };
        derniers.insert((rang, d.mois.clone()), d);
    }
    let mensuel = v.objet.mensuel();
    let mut out = Vec::new();
    // UN CONTRAT SIGNE AILLEURS, dont aucun avenant n'est parti : ce qu'il dit est ce que dit le premier mois ; un terme
    // date apres lui (`stock::DATES`) est un changement que le salarie n'a pas signe.
    let contrat_parti = derniers.keys().any(|(sorte, _)| *sorte == 0);
    if sorte(Envoi::Contrat) && !contrat_parti && signe_ailleurs(ctx, &v.objet) {
        let presente = actuelle.get("saisie").cloned().unwrap_or(Value::Null);
        if clauses(&presente, mensuel)["termes"].as_object().is_some_and(|t| !t.is_empty()) {
            out.push(ARefaire::Avenant { depuis: None });
        }
    }
    for d in derniers.into_values() {
        if Some(d.version) == present {
            continue;
        }
        let Some(passee) = v.version_numero(d.version) else { continue };
        let saisie = |x: &Value| x.get("saisie").cloned().unwrap_or(Value::Null);
        let paie = |x: &Value, m: &str| paie_ecrite_de_la_version(&v.objet, x, m).ok().flatten();
        match d.envoi {
            Envoi::Contrat => {
                let (partie, presente) = (saisie(passee), saisie(&actuelle));
                if clauses(&partie, mensuel) != clauses(&presente, mensuel) && !seule_la_rupture(&partie, &presente) {
                    out.push(ARefaire::Avenant { depuis: Some(d.version) });
                }
            }
            Envoi::Dpae => {
                let (avant, maintenant) = (crate::debut_brut(&ctx.racine, &saisie(passee)), crate::debut_brut(&ctx.racine, &saisie(&actuelle)));
                if avant != maintenant {
                    out.push(ARefaire::Dpae { depuis: d.version, debut_declare: avant });
                }
            }
            // Un signalement (arret, reprise, fin de contrat) part sur sa version ; il ne porte pas de paie.
            Envoi::Signalement => {}
            Envoi::Documents | Envoi::Dsn => {
                let Some(m) = d.mois.as_deref() else { continue };
                let change = match (paie(passee, m), paie(&actuelle, m)) {
                    (Some(a), Some(b)) => !Arc::ptr_eq(&a, &b) && !chiffres_egaux(&a, &b),
                    (a, b) => a.is_some() != b.is_some(),
                };
                if change {
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

/// Ce qu'une rupture ecrit dans la saisie d'un engagement : sa fin, son motif, son preavis et ses dates.
const DE_LA_RUPTURE: &[&str] =
    &["debut", "fin", "motif_rupture", "preavis", "notification_rupture", "signature_convention", "engagement_licenciement", "transaction", "numero", "lignes"];

/// UN CONTRAT ROMPU N'A PAS D'AVENANT. Code du travail L. 1243-1 (`atelier/reference/sources/code-du-travail/
/// articles-20261028.md`) : le CDD se rompt avant l'echeance de son terme (accord des parties, faute grave, force
/// majeure, inaptitude) ; la rupture met fin au contrat signe, elle ne le modifie pas, et rien ne se refait signer. Vrai
/// quand la saisie `presente` ne differe de la saisie `partie` (celle sur laquelle le contrat est parti) que par la rupture
/// qu'elle porte : son motif (`motif_rupture`, absent de la partie), la fin et le preavis qu'il appelle, et les jours de
/// travail d'apres la fin retires (une ligne garde ses jours jusqu'a la fin, et sa quantite ne croit pas).
fn seule_la_rupture(partie: &Value, presente: &Value) -> bool {
    let texte = |v: &Value, k: &str| v.get(k).filter(|x| vrai(x)).map(en_texte).unwrap_or_default();
    let fin = texte(presente, "fin");
    if texte(presente, "motif_rupture").is_empty() || !texte(partie, "motif_rupture").is_empty() || fin.len() != 10 {
        return false;
    }
    let reste = |v: &Value| -> Map<String, Value> {
        v.as_object().map(|o| o.iter().filter(|(k, _)| !DE_LA_RUPTURE.contains(&k.as_str())).map(|(k, x)| (k.clone(), x.clone())).collect()).unwrap_or_default()
    };
    if reste(partie) != reste(presente) {
        return false;
    }
    let dates = |l: &Value| -> Vec<String> { l.get("dates").and_then(Value::as_array).map(|a| a.iter().map(en_texte).collect()).unwrap_or_default() };
    let quantite = |l: &Value| l.get("quantite").and_then(Value::as_f64).unwrap_or(0.0);
    let sans = |l: &Value, k: &[&str]| -> Map<String, Value> {
        l.as_object().map(|o| o.iter().filter(|(c, _)| !k.contains(&c.as_str())).map(|(c, x)| (c.clone(), x.clone())).collect()).unwrap_or_default()
    };
    let lignes = |v: &Value| v.get("lignes").and_then(Value::as_array).cloned().unwrap_or_default();
    // Les lignes de la partie, arretees a la fin : celles qui n'ont plus aucun jour disparaissent.
    let mut attendues: Vec<(Value, bool)> = Vec::new();
    for l in lignes(partie) {
        let jours = dates(&l);
        let gardes: Vec<String> = jours.iter().filter(|j| **j <= fin).cloned().collect();
        if !jours.is_empty() && gardes.is_empty() {
            continue;
        }
        let mut l = l.clone();
        let coupee = gardes.len() != jours.len();
        if coupee {
            l["dates"] = json!(gardes);
        }
        attendues.push((l, coupee));
    }
    let presentes = lignes(presente);
    presentes.len() == attendues.len()
        && presentes.iter().zip(&attendues).all(|(p, (a, coupee))| {
            if *coupee {
                sans(p, &["quantite"]) == sans(a, &["quantite"]) && quantite(p) <= quantite(a)
            } else {
                p == a
            }
        })
}

#[cfg(test)]
mod tests_rupture {
    use super::*;

    /// Code du travail L. 1243-1 (`atelier/reference/sources/code-du-travail/articles-20261028.md`) : rompre un CDD avant
    /// son terme ne modifie pas le contrat signe, rien ne se refait signer ; changer en meme temps ce qu'il paie, si.
    #[test]
    fn un_contrat_rompu_n_a_pas_d_avenant() {
        let jours = |n: usize| json!(["2026-05-04", "2026-05-05", "2026-05-06", "2026-05-07", "2026-05-08"][..n].to_vec());
        let ligne = |n: usize, taux: i64| json!({"rubrique": "cachet", "quantite": n, "taux": taux, "lieu": "Salle", "dates": jours(n)});
        let partie = json!({"salarie": "X", "spectacle": "s", "lignes": [ligne(5, 150)], "numero": "X1.26.01"});
        let rompu = |taux: i64| json!({"salarie": "X", "spectacle": "s", "debut": "2026-05-04", "fin": "2026-05-06", "motif_rupture": "084",
            "preavis": {"type": "90"}, "lignes": [ligne(3, taux)], "numero": "X1.26.01"});
        assert!(seule_la_rupture(&partie, &rompu(150)));
        assert!(!seule_la_rupture(&partie, &rompu(180)), "un taux change avec la rupture se refait signer");
        let raccourci = json!({"salarie": "X", "spectacle": "s", "lignes": [ligne(3, 150)], "numero": "X1.26.01"});
        assert!(!seule_la_rupture(&partie, &raccourci), "sans motif de rupture, des jours retires sont un avenant");
    }
}

/// La version (`saisie` ; ses paies : `paie_ecrite_de_la_version`) sur laquelle le dernier envoi `envoi` de l'engagement
/// `v` est parti, telle qu'elle est ecrite ; `None` si rien de tel n'est parti.
pub fn version_au_depart<'a>(ctx: &Contexte, v: &'a Valide, departs: &Departs, envoi: Envoi) -> Option<&'a Value> {
    let d = ses_departs(ctx, &v.objet, departs).iter().rev().find(|d| d.envoi == envoi)?;
    v.version_numero(d.version)
}

/// La version `n` de l'engagement `o`, telle qu'elle est ecrite (`saisie`) : le present, la validation que
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
    let saisie = version.get("saisie").and_then(Value::as_object).ok_or_else(|| socle::echec::cle(socle::echec::PAIE_ILLISIBLE, "saisie"))?;
    let mut declare = o.clone();
    declare.brut = std::sync::Arc::new(Value::Object(saisie.clone()));
    declare.vue(mois)
}

/// LA PAIE ECRITE DE `mois` DE LA VERSION `version` de l'engagement `o` (une validation, celle qu'une modification va
/// remplacer, ou une version passee, telles que l'engagement les ecrit) : la paie presente du mois pour la version presente,
/// celle que la version passee a gardee (`paies::de_la_version`) sinon ; `None` si elle n'en a pas pour ce mois.
pub fn paie_ecrite_de_la_version(o: &Objet, version: &Value, mois: &str) -> Resultat<Option<Arc<Value>>> {
    let n = version_de(Some(version));
    let presente = [o.brut.get("valide"), o.brut.get(PRECEDENTE)].into_iter().flatten().any(|v| version_de(Some(v)) == n);
    match n {
        Some(n) if !presente => Ok(crate::paies::de_la_version(o, n, mois)?.map(Arc::new)),
        _ => crate::paies::presente(o, mois),
    }
}

/// La paie de `mois` que la version `version` de `o` a ecrite, relue (`relire`) sur la saisie de cette version, la fiche
/// du salarie et la saisie du mois telles que son calcul les a lues (`fiches`, `MOIS_LU`) ; `None` si cette version n'a
/// pas de paie pour ce mois. Elle porte le numero de son bulletin quand ses documents sont partis.
fn paie_de_la_version(ctx: &Contexte, o: &Objet, version: &Value, mois: &str) -> Resultat<Option<Paie>> {
    let Some(ecrite) = paie_ecrite_de_la_version(o, version, mois)? else { return Ok(None) };
    let paie = ecrite.get("paie").ok_or_else(|| socle::echec::cle(socle::echec::PAIE_ILLISIBLE, "paie"))?;
    let Some(vue) = vue_de_la_version(o, version, mois)? else { return Ok(None) };
    let mut data = ecrite.get(MOIS_LU).filter(|m| m.is_object()).cloned().ok_or_else(|| socle::echec::cle(socle::echec::PAIE_ILLISIBLE, MOIS_LU))?;
    data["mois"] = json!(mois);
    data["engagements"] = json!([vue]);
    let eng = relire::engagements_ecrits(ctx, &data, &[paie])?.pop().ok_or_else(|| socle::echec::cle(socle::echec::PAIE_ILLISIBLE, "paie"))?;
    let fiches = ecrite.get("fiches").filter(|f| f.is_object()).ok_or_else(|| socle::echec::cle(socle::echec::PAIE_ILLISIBLE, "fiches"))?;
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

/// La paie de `mois` que la version n° `n` de l'engagement `o` a ecrite (`version_ecrite`, `paie_de_la_version`) ; `None`
/// si `o` n'a pas cette version ou qu'elle n'a pas de paie pour ce mois.
pub fn paie_de_la_version_n(ctx: &Contexte, o: &Objet, n: u64, mois: &str) -> Resultat<Option<Paie>> {
    match version_ecrite(ctx, o, n)? {
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
    let mut out: Vec<Declaree> = Vec::new();
    let non_actif = o.brut.get(stock::NON_ACTIF).and_then(|n| n.get("version")).and_then(Value::as_u64);
    for (par, n) in versions_declarees(ctx, o, departs, mois, avant)? {
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

/// Les versions que `paies_declarees` relit, dans le meme ordre, chacune avec le mois de la DSN qui l'a portee : ce qui
/// a ete declare, sans en relire les paies.
fn versions_declarees(ctx: &Contexte, o: &Objet, departs: &Departs, mois: &str, avant: &str) -> Resultat<Vec<(Option<String>, u64)>> {
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
    Ok(chaine)
}

/// LA CORRECTION DUE pour `mois` de l'engagement valide `v`, dans la DSN de `avant` : ce qui a ete declare pour ce mois
/// par les DSN anterieures a `avant` (`paies_declarees`) quand la paie presente de `mois` n'est plus celle que la
/// derniere a declaree (la paie elle-meme, pas le prelevement ni le compteur de conges, que la DSN ne corrige pas) ; vide
/// quand aucune DSN n'a declare ce mois pour lui. `None` : rien a corriger, ou pas de paie presente de ce mois. La couche
/// des declarations dit si le mois se corrige ainsi (sa DSN partie, son annule-et-remplace passe). Les paies declarees ne
/// se relisent que quand la correction est due.
pub fn correction_due(ctx: &Contexte, v: &Valide, departs: &Departs, mois: &str, avant: &str) -> Resultat<Option<Vec<Declaree>>> {
    let Some(presente) = v.paie_ecrite(mois).filter(|p| p.get("paie").is_some()) else { return Ok(None) };
    let Some(&(_, derniere)) = versions_declarees(ctx, &v.objet, departs, mois, avant)?.last() else { return Ok(Some(Vec::new())) };
    if Some(derniere) == v.version() {
        return Ok(None);
    }
    let ecrite = version_ecrite(ctx, &v.objet, derniere)?.map(|x| paie_ecrite_de_la_version(&v.objet, x, mois)).transpose()?.flatten();
    if ecrite.is_some_and(|e| egales_sans_avis(champ(&e, "paie"), champ(&presente, "paie"))) {
        return Ok(None);
    }
    Ok(Some(paies_declarees(ctx, &v.objet, departs, mois, avant)?))
}

/// LA DERNIERE DATE DE DEBUT DECLAREE de l'engagement `o` (AAAA-MM-JJ) par une DSN d'un mois anterieur a `avant` : celle
/// de la saisie de la version sur laquelle la derniere est partie, quel que soit le mois qu'elle declarait (la correction
/// d'un autre mois d'un contrat sur plusieurs mois a pu la changer). `None` : aucune.
pub fn debut_declare(ctx: &Contexte, o: &Objet, departs: &Departs, avant: &str) -> Resultat<Option<String>> {
    let Some(d) = ses_departs(ctx, o, departs).iter().rev().find(|d| d.envoi == Envoi::Dsn && d.par.as_deref().or(d.mois.as_deref()).is_some_and(|m| m < avant)) else {
        return Ok(None);
    };
    Ok(version_ecrite(ctx, o, d.version)?.and_then(|v| v.get("saisie")).map(|s| crate::debut_brut(&ctx.racine, s)).filter(|d| !d.is_empty()))
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

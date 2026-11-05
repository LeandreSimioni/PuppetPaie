//! Les taux de prelevement a la source que la DGFiP a fait connaitre (un compte rendu de nature 94, `suites`) ou que les
//! DSN deposees declarent (l'import) : chacun s'ecrit dans la fiche de son salarie (`droit::pas::CHAMP`,
//! `routes::fiches::poser_taux_recu`), et les engagements qu'il touche se recalculent. `droit::pas` les lit.

use serde_json::{json, Value};

use droit::contexte::Contexte;
use socle::date::Jour;
use socle::echec::Echec;

pub(crate) fn ko(m: String) -> Echec {
    Echec { classe: None, message: m }
}

/// Ecrit des taux recus ; rend les changements lisibles (`CLE : ...`). Un releve :
/// `{"salarie", "taux", "type", "identifiant", "mis_a_disposition", "en_retour_de"?}` (AAAAMMJJHHMMSS ; le mois de la DSN
/// a laquelle repond son compte rendu, `droit::pas::EN_RETOUR_DE`), ecrit a sa date d'effet (`droit::pas::date_d_effet`).
/// Un taux deja recu (meme
/// identifiant, meme taux, meme instant) ne s'ecrit pas deux fois.
pub fn enregistrer(ctx: &Contexte, releves: &[Value], source: &str) -> Result<Vec<String>, Echec> {
    let mut changements = Vec::new();
    let mut ecrits: Vec<(String, droit::fiche::TauxPas)> = Vec::new();
    for r in releves {
        let cle = r.get("salarie").and_then(Value::as_str).unwrap_or("").to_string();
        let get = |k: &str| r.get(k).cloned().unwrap_or(Value::Null);
        let mut valeur = json!({"taux": get("taux"), "type": get("type"), "identifiant": get("identifiant"), "mis_a_disposition": get("mis_a_disposition")});
        let en_retour_de = r.get(droit::pas::EN_RETOUR_DE).and_then(Value::as_str).filter(|m| !m.is_empty());
        if let Some(m) = en_retour_de {
            valeur[droit::pas::EN_RETOUR_DE] = json!(m);
        }
        let lu: droit::fiche::TauxPas = droit::fiche::lire_champs(&valeur, "Le taux de PAS reçu").map_err(ko)?;
        let Some(mad) = droit::pas::mise_a_disposition(&lu) else {
            return Err(ko(format!("{} : un taux de PAS recu sans sa date de mise a disposition ne s'ecrit pas ({}).", cle, valeur)));
        };
        let depuis = droit::pas::date_d_effet(mad, en_retour_de);
        let deja = droit::pas::recus(ctx, &cle).map_err(ko)?.into_iter().chain(ecrits.iter().filter(|(c, _)| *c == cle).map(|(_, v)| v.clone())).any(|v| v == lu);
        if deja {
            continue;
        }
        crate::routes::fiches::poser_taux_recu(ctx, &cle, valeur.clone(), depuis, source)?;
        let t = |k: &str| crate::commandes::rendu::champ(&valeur, k);
        changements.push(format!("{} : taux {}, type {}, identifiant {}, mis a disposition le {}", cle, t("taux"), t("type"), t("identifiant"), mad.jj_mm_aaaa()));
        ecrits.push((cle, lu));
    }
    if !changements.is_empty() {
        refaire_les_engagements(ctx)?;
    }
    Ok(changements)
}

/// UN TAUX RECU REFAIT, DANS LE MEME GESTE, LES ENGAGEMENTS QU'IL TOUCHE : chaque mois saisi se relit
/// (`engagement::etats::du_mois`) dans la compagnie telle qu'elle est maintenant, et ses engagements valides dont
/// l'empreinte ne tient plus (le taux de leur salarie au jour de leur paiement, `droit::pas::taux_dgfip`) se reecrivent.
/// Un mois dont les documents sont partis ne bouge pas.
fn refaire_les_engagements(ctx: &Contexte) -> Result<(), Echec> {
    let ctx = Contexte::nouveau(&ctx.racine, Some(ctx.date)).map_err(ko)?;
    let departs = envois::journal::lu(&ctx.racine).map_err(ko)?.departs;
    for mois in engagement::stock::mois_saisis(&ctx.racine).map_err(ko)?.into_iter().map(String::from) {
        engagement::etats::du_mois(&ctx, &mois, &departs).map_err(ko)?;
    }
    Ok(())
}

/// `AAAAMMJJ000000` du jour `j`.
fn instant_du_jour(j: Jour) -> String {
    format!("{}000000", j.aaaammjj())
}

/// LE DERNIER TAUX TRANSMIS PAR LA DGFiP QUE LES DSN DEJA DEPOSEES DE `dossier` DECLARENT, par salarie (un releve tel que
/// `enregistrer` le prend). Une DSN qui declare un taux de type 01 avec son identifiant (S21.G00.50.006 a .008,
/// `norme-dsn/texte/bloc-S21.G00.50.txt` : « Identifiant du taux porté par le CRM PAS […] transmis par la DGFIP ») dit
/// que la DGFiP a transmis ce taux a l'employeur, qui l'applique (CGI, art. 1671, 2,
/// `atelier/reference/sources/code-general-des-impots/articles-20261021.md`). Sa validite court de sa mise a disposition
/// (`droit::pas::VALIDITE_MOIS`), que la DSN ne porte pas. Ce que les textes en disent : la retenue se fait « lors du
/// paiement » (CGI, art. 1671, 1) avec un taux « toujours valide à la date de son application » (BOI-IR-PAS-30-10-20,
/// § 80, `atelier/reference/sources/bofip/BOI-IR-PAS-30-10-20-20181203.html`) : le taux etait donc mis a disposition au
/// plus tard le jour du versement (S21.G00.50.001) et au plus tot deux mois pleins avant ; le compte rendu « est
/// transmis le mois au cours duquel la DSN […] a été déposée » (meme page, § 30), mais la DSN qui applique le
/// taux n'en porte pas la date. INFERENCE (aucun texte ne date un taux lu dans une DSN) : le releve est date du jour du versement
/// (`mis_a_disposition`), la mise a disposition la plus tardive que les textes permettent ; le compte rendu de la DSN suivante
/// le remplace. Chaque DSN se lit par `declarations::dsn_lue::lire_une_dsn` ; une DSN fautive refuse.
pub(crate) fn derniers_taux_des_dsn(ctx: &Contexte, dossier: &std::path::Path) -> Result<Vec<Value>, Echec> {
    let par_nir: std::collections::HashMap<String, String> = ctx
        .salaries
        .iter()
        .filter_map(|(cle, s)| s.nir.dite().filter(|n| !n.is_empty()).map(|n| (n.to_string(), cle.to_string())))
        .collect();
    let fichiers = declarations::dsn_lue::fichiers(dossier);
    // salarie -> mois -> releve (dans l'ordre d'insertion des salaries, mois tries).
    let mut trouve: Vec<(String, std::collections::BTreeMap<socle::mois::Mois, Value>)> = Vec::new();
    for f in fichiers {
        let Some(d) = declarations::dsn_lue::lire_une_dsn(&f)? else { continue };
        let Some(mois) = d.mois else { continue };
        for v in &d.versements {
            // « -1 » : un CDD court sans taux personnalise (`norme-dsn/texte/bloc-S21.G00.50.txt`, S21.G00.50.008) ;
            // aucun taux n'a ete transmis. Sans la date du versement (S21.G00.50.001), le taux ne se date pas.
            let (Some(id), Some(j), Some(cle)) = (v.identifiant_pas.as_ref().filter(|i| *i != "-1"), v.verse_le, par_nir.get(&v.nir)) else {
                continue;
            };
            let rel = json!({
                "taux": v.taux_pas.map(|t| t.to_string()),
                "type": v.type_pas,
                "identifiant": id,
                "mis_a_disposition": instant_du_jour(j),
            });
            match trouve.iter_mut().find(|(k, _)| k == cle) {
                Some((_, mm)) => {
                    mm.insert(mois, rel);
                }
                None => trouve.push((cle.clone(), [(mois, rel)].into_iter().collect())),
            }
        }
    }
    let mut releves = Vec::new();
    for (cle, mm) in trouve {
        if let Some(Value::Object(mut r)) = mm.into_values().next_back() {
            r.insert("salarie".into(), json!(cle));
            releves.push(Value::Object(r));
        }
    }
    Ok(releves)
}

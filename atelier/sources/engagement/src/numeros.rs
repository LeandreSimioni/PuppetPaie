//! LE NUMERO D'UN CONTRAT (regle 4 : il ne se recalcule pas, il se lit). Il identifie le contrat en DSN
//! (S21.G00.40.009), unique par individu et par etablissement dans une declaration (CCH-12,
//! `atelier/reference/norme-dsn/texte-2027/bloc-S21.G00.40.txt`) ; aucune suite continue n'est exigee.
//!
//! Il s'ecrit dans l'engagement a sa validation (`a_la_validation`, que seule `etats::valider` appelle), puis tout ce qui
//! part -- le contrat a la signature ou imprime, la DPAE, la DSN, le bulletin -- le lit la. Le numero donne est le suivant
//! du plus haut que portent, pour le meme salarie et la meme annee, les engagements et ce qui est parti (`departs`)
//! (`derniers_pris`) : un engagement supprime avant tout envoi rend le sien.
//!
//! Une revalidation garde le numero. Tant que rien n'est parti sous lui, un engagement dont le salarie ou le debut a
//! change en recoit un nouveau ; des qu'un envoi l'a porte, il ne change plus. Un numero venu avec la saisie (un contrat
//! repris d'un autre logiciel) reste le sien, sauf sur la copie d'un engagement plus ancien qui le porte deja.

use std::collections::HashMap;


use serde_json::{json, Value};

use droit::contexte::Contexte;
use socle::date::en_texte;
use socle::fichiers::vrai;
use socle::num::Resultat;

use crate::stock::{self, Objet};

/// Pour qui le numero a ete donne : la cle du salarie et le debut du contrat a ce moment-la, et quand.
const POUR: &str = "numero_pour";

/// L'annee (2000 + les deux chiffres) et le rang d'un numero `FA1.26.16` ; `None` s'il n'a pas cette forme.
pub fn annee_et_rang(n: &str) -> Option<(i32, i64)> {
    let (tete, rang) = n.rsplit_once('.')?;
    if rang.is_empty() || !rang.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let (_, yy) = tete.rsplit_once('.')?;
    if yy.len() != 2 || !yy.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some((2000 + yy.parse::<i32>().ok()?, rang.parse().ok()?))
}

/// Le numero de rang `seq` de l'annee `annee`, pour le salarie de cette fiche : `<initiales><code>.<AA>.<rang>`.
pub fn former(fiche: &Value, annee: i32, seq: i64) -> Resultat<String> {
    let champ = |k: &str| fiche.get(k).map(en_texte).ok_or_else(|| socle::fichiers::absent(k));
    let an = annee.to_string();
    Ok(format!("{}{}.{}.{:02}", champ("initiales_contrat")?, champ("code_salarie")?, &an[2.min(an.len())..], seq))
}

/// Le dernier rang pris en `annee`, par salarie (la cle de sa fiche) : le plus grand des numeros de cette annee que
/// portent ses engagements, ou sous lesquels quelque chose est parti pour lui (`departs`).
pub fn derniers_pris(ctx: &Contexte, annee: i32) -> Resultat<HashMap<String, i64>> {
    derniers_pris_sauf(ctx, annee, None)
}

/// La meme, sans l'engagement `sauf` (son fichier et son rang) : celui qui recoit un nouveau numero.
fn derniers_pris_sauf(ctx: &Contexte, annee: i32, sauf: Option<(&str, usize)>) -> Resultat<HashMap<String, i64>> {
    let mut pris: HashMap<String, i64> = HashMap::new();
    let mut noter = |cle: &str, numero: &str| {
        if let Some((a, seq)) = annee_et_rang(numero) {
            if a == annee {
                let v = pris.entry(cle.to_string()).or_insert(0);
                *v = (*v).max(seq);
            }
        }
    };
    for o in stock::objets(&ctx.racine)? {
        if sauf == Some((o.fichier.as_str(), o.rang)) {
            continue;
        }
        let e = Value::Object(o.brut.clone());
        let (Some(cle), Some(n)) = (crate::cle_de_l_engagement(ctx, &e), o.brut.get("numero").filter(|n| vrai(n))) else { continue };
        noter(&cle, &en_texte(n));
    }
    for (cle, n) in crate::departs::lire(&ctx.racine)?.engagements() {
        noter(cle, n);
    }
    Ok(pris)
}

/// Un envoi a-t-il porte le numero `n` de l'engagement `o` (du salarie `cle`) ? Ce qui est parti sous lui (`departs`),
/// ou une version de lui gardee parce qu'un envoi l'avait emportee.
fn porte_par_un_envoi(ctx: &Contexte, o: &Objet, cle: &str, n: &str) -> Resultat<bool> {
    if o.brut.get("versions").and_then(Value::as_array).is_some_and(|v| !v.is_empty()) {
        return Ok(true);
    }
    Ok(!crate::departs::lire(&ctx.racine)?.de(cle, n).is_empty())
}


/// Un engagement ecrit avant `o` (plus ancien fichier, ou plus haut dans le meme) porte-t-il deja le numero `n` ? Une
/// copie d'engagement emporte le numero de l'original : il reste a l'original.
/// Le contrat importe que `o` continue (`importe::continue_par`) porte le sien : c'est le meme contrat, pas une copie.
fn porte_par_un_autre(ctx: &Contexte, o: &Objet, n: &str) -> Resultat<bool> {
    let continue_ = crate::importe::continue_par(&ctx.racine, &o.brut)?.map(|i| (i.objet().fichier.clone(), i.objet().rang));
    Ok(stock::objets(&ctx.racine)?.iter().any(|x| {
        (x.fichier.as_str(), x.rang) < (o.fichier.as_str(), o.rang)
            && continue_.as_ref() != Some(&(x.fichier.clone(), x.rang))
            && x.brut.get("numero").map(en_texte).as_deref() == Some(n)
    }))
}

/// LE NUMERO A LA VALIDATION de l'engagement `o` (`etats::valider`), pose sur lui ; `le` : l'instant de la validation.
/// Il garde celui qu'il porte, sauf si son salarie ou son debut a change depuis qu'il l'a recu et qu'aucun envoi ne l'a
/// porte : il en recoit alors un nouveau, comme un engagement qui n'en a pas.
pub(crate) fn a_la_validation(ctx: &Contexte, o: &mut Objet, le: &str) -> Resultat<()> {
    let e = Value::Object(o.brut.clone());
    let fiche = ctx.salarie(&e.get("salarie").map(en_texte).unwrap_or_default())?;
    let cle = fiche.get("_cle").map(en_texte).ok_or_else(|| socle::fichiers::absent("_cle"))?;
    let debut = crate::debut_brut(&e);
    if let Some(n) = o.brut.get("numero").filter(|n| vrai(n)).map(en_texte) {
        let garde = match o.brut.get(POUR) {
            Some(p) => {
                let meme = p.get("salarie").map(en_texte).as_deref() == Some(cle.as_str()) && p.get("debut").map(en_texte) == Some(debut.clone());
                meme || porte_par_un_envoi(ctx, o, &cle, &n)?
            }
            // Un numero venu avec la saisie (un contrat repris d'un autre logiciel, sous la forme qu'il avait) : le sien,
            // sauf s'il est la copie d'un engagement plus ancien.
            None => !porte_par_un_autre(ctx, o, &n)?,
        };
        if garde {
            return Ok(());
        }
    }
    let annee: i32 = debut.get(..4).and_then(|a| a.parse().ok()).ok_or_else(|| format!("date illisible : {}", debut))?;
    let dernier = derniers_pris_sauf(ctx, annee, Some((o.fichier.as_str(), o.rang)))?.get(&cle).copied().unwrap_or(0);
    o.brut.insert("numero".into(), Value::String(former(&fiche, annee, dernier + 1)?));
    o.brut.insert(POUR.into(), json!({"salarie": cle, "debut": debut, "le": le}));
    Ok(())
}

/// Le numero de contrat que la paie `p`, sans numero, porterait : la suite des numeros `pris` pour son salarie
/// (`derniers_pris`, lus une fois), en memoire.
fn proposer(ctx: &Contexte, p: &mut crate::calcul::Contenu, annee: i32, pris: &mut Option<HashMap<String, i64>>) -> Resultat<()> {
    let pris = match pris {
        Some(x) => x,
        None => pris.insert(derniers_pris(ctx, annee)?),
    };
    let cle = p.salarie.get("_cle").map(en_texte).ok_or_else(|| socle::fichiers::absent("_cle"))?;
    let dernier = pris.entry(cle).or_insert(0);
    *dernier += 1;
    p.engagement.numero = Some(former(&p.salarie, annee, *dernier)?);
    Ok(())
}

/// LES NUMEROS DE CONTRAT QUE DES PAIES PORTERAIENT, en memoire : un engagement qui porte deja le sien le garde ; au
/// premier mois de son contrat, un autre prend la suite des numeros pris pour son salarie (`derniers_pris`). Un mois qui
/// n'est pas le premier n'en recoit pas : son numero est celui de l'engagement, ecrit a sa validation.
pub fn numeroter_les_contrats(ctx: &Contexte, paies: &mut [crate::calcul::Paie], annee: i32) -> Resultat<()> {
    let mut pris = None;
    for paie in paies.iter_mut() {
        if paie.engagement.numero.as_deref().is_some_and(|n| !n.is_empty()) || !paie.engagement.premier_mois()? {
            continue;
        }
        proposer(ctx, paie.ecrire(), annee, &mut pris)?;
    }
    Ok(())
}

/// LES NUMEROS QUE DES PAIES PORTERAIENT, en memoire : les numeros de contrat (`numeroter_les_contrats`, et un mois qui
/// n'est pas le premier de son contrat, sans numero, refuse : `refus_sans_numero`) ; une paie dont le bulletin est parti
/// garde son numero de bulletin, une autre recoit le `prochain` (`N/AAAA`), puis le suivant. Un mois sans paie n'a pas de
/// bulletin. Rend le prochain numero de bulletin libre.
pub fn numeroter(ctx: &Contexte, paies: &mut [crate::calcul::Paie], annee: i32, prochain: i64) -> Resultat<i64> {
    let mut n_bull = prochain;
    let mut pris = None;
    for paie in paies.iter_mut() {
        let p = paie.ecrire();
        if p.engagement.numero.as_deref().map_or(true, str::is_empty) {
            crate::refus_sans_numero(ctx, &p.engagement)?;
            proposer(ctx, p, annee, &mut pris)?;
        }
        if p.engagement.sans_paie() || p.numero_bulletin.as_deref().is_some_and(|n| !n.is_empty()) {
            continue;
        }
        p.numero_bulletin = Some(format!("{}/{}", n_bull, annee));
        n_bull += 1;
    }
    Ok(n_bull)
}

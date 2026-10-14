//! `S21.G00.30.013` (codification UE) : quatre valeurs, et la table qui les donne depuis le pays de nationalite.
//!
//! LA NORME (cahier technique DSN 2026.1, p. 173, `prive/reference/norme-dsn/texte/bloc-S21.G00.30.txt`, l. 267-277) :
//! « Classification de l'origine du salarié au vu des frontières françaises et des limites de l'Union Européenne. Il
//! faut renseigner dans cette rubrique le code le plus précis applicable à l'individu » ; « 01 - France / 02 - UE /
//! 03 - EEE / 04 - Reste du Monde ». « Le plus precis » : la France est dans l'Union, l'Union dans l'EEE ; 03 ne vaut
//! donc que pour un Etat de l'EEE hors de l'Union. Lire « origine » comme la nationalite est une INFERENCE (le moteur
//! le fait depuis le 23/09/2026 ; la norme ne dit pas « nationalite »).
//!
//! LES ETATS (`prive/reference/sources/union-europeenne/LISEZ-MOI.md`) : l'Union, traite sur l'Union europeenne,
//! art. 52 § 1, version consolidee (JO C 202 du 07/06/2016), moins le Royaume-Uni, sorti par l'accord de retrait
//! (JO L 29 du 31/01/2020 ; fin de la periode de transition le 31/12/2020, art. 126) ; l'EEE hors Union : Islande,
//! Liechtenstein, Norvege (synthese EUR-Lex de l'accord EEE, qui dit aussi que la Suisse l'a rejete). Le Royaume-Uni
//! et la Suisse ne sont ni dans l'une ni dans l'autre liste : 04 (INFERENCE par absence, la norme ne les nomme pas).
//! Les iles Feroe, dont le pays de rattachement est le Danemark, sont hors de l'Union par le texte (TFUE art. 355 § 5 a) : 04.
//!
//! JUSQU'AU 10/10/2026, LE MOTEUR N'AVAIT QUE TROIS VALEURS, 03 voulant dire « un autre pays ». Une fiche en 03 sans
//! pays de nationalite est donc ambigue : ancienne (« reste du monde », 04 aujourd'hui) ou neuve (EEE). Elle ne se
//! convertit pas en silence : le moteur la recode quand la fiche porte le pays (`pays_nationalite`), il refuse sinon.
//!
//! UN PAYS HORS DE LA LISTE (vague 2h, Leandre, 10/10/2026 : « Si le pays n'existe pas, on laisse l'administrateur
//! choisir. ») : la fiche l'ecrit en `pays_nationalite_hors_liste` (le nom, en texte), et la codification est celle que
//! l'administrateur choisit en `code_nationalite_dsn`, sans pays de la liste a cote. La DSN ne declare pas le pays de
//! nationalite, seulement `30.013` : rien a inventer. Un pays de la liste ecrit la ne compte pas comme hors liste (il
//! se choisit dans la liste, la codification s'en deduit) ; la France non plus (01 refuse).

use serde_json::Value;

use crate::fichiers::vrai;
use crate::num::Resultat;

/// Les Etats membres de l'Union (TUE art. 52 § 1, sans le Royaume-Uni), en codes ISO a deux lettres, la France a part.
pub const UNION: [&str; 26] = [
    "BE", "BG", "CZ", "DK", "DE", "EE", "IE", "GR", "ES", "HR", "IT", "CY", "LV", "LT", "LU", "HU", "MT", "NL", "AT",
    "PL", "PT", "RO", "SI", "SK", "FI", "SE",
];

/// Les territoires que les traites excluent de l'Union alors que leur pays de rattachement en est : les Feroe (TFUE art. 355
/// § 5 a), « les traites ne s'appliquent pas aux iles Feroe » ; protocole n° 2 de l'acte d'adhesion de 1972, art. 4 : les
/// ressortissants danois qui y resident ne sont pas des ressortissants d'un Etat membre ; hors de l'EEE par l'art. 126 § 1
/// de l'accord : `prive/reference/sources/union-europeenne/LISEZ-MOI.md`, « Les iles Feroe »).
pub const TERRITOIRES_HORS_UNION: [&str; 1] = ["FO"];

/// Les Etats parties a l'accord EEE qui ne sont pas dans l'Union.
pub const EEE_HORS_UNION: [&str; 3] = ["IS", "LI", "NO"];

/// Le code de `S21.G00.30.013` d'un pays (code ISO a deux lettres, majuscules).
pub fn code_du_pays(iso: &str) -> &'static str {
    if iso == "FR" {
        "01"
    } else if UNION.contains(&iso) {
        "02"
    } else if EEE_HORS_UNION.contains(&iso) {
        "03"
    } else {
        "04"
    }
}

fn texte(fiche: &Value, cle: &str) -> Option<String> {
    fiche.get(cle).filter(|v| vrai(v)).map(|v| crate::etats::py_str(v).trim().to_string()).filter(|t| !t.is_empty())
}

fn qui(fiche: &Value) -> String {
    let n = [texte(fiche, "prenom"), texte(fiche, "nom")].into_iter().flatten().collect::<Vec<_>>().join(" ");
    if n.is_empty() {
        "ce salarié".to_string()
    } else {
        n
    }
}

/// Ce qui ne va pas dans le pays de nationalite ecrit hors de la liste (`pays_nationalite_hors_liste`), avant toute
/// codification : un pays de la liste a cote, ou un pays que la liste connait (son nom ou son code). `None` : rien a
/// dire (pas de pays hors liste, ou un pays que la liste ne connait pas).
pub fn faute_hors_liste(fiche: &Value) -> Resultat<Option<String>> {
    let Some(hors) = texte(fiche, "pays_nationalite_hors_liste") else { return Ok(None) };
    if let Some(pays) = texte(fiche, "pays_nationalite") {
        return Ok(Some(format!(
            "La fiche de {} dit deux pays de nationalité : {} dans la liste, et « {} » hors de la liste. Garder l'un ou \
             l'autre : le pays de la liste, dont la nationalité DSN se déduit, ou le pays hors de la liste avec la \
             nationalité choisie.",
            qui(fiche),
            pays,
            hors
        )));
    }
    let connu = match crate::pays::du_nom(&hors)? {
        Some(p) => Some(p),
        None if hors.chars().count() == 2 => crate::pays::du_code(&hors)?,
        None => None,
    };
    Ok(connu.map(|p| {
        format!(
            "Le pays de nationalité de {} (« {} ») est dans la liste ({}, {}) : le choisir dans « Pays de nationalité » ; \
             la nationalité que la DSN déclare s'en déduit et ne se choisit pas.",
            qui(fiche),
            hors,
            p.nom,
            p.code
        )
    }))
}

/// Le code a declarer en `S21.G00.30.013` pour une fiche salarie. Refus : pas de code ; un 03 sans pays (ancienne
/// valeur, ambigue) ; un pays et un code qui se contredisent sans que l'ancienne codification l'explique. Un pays de
/// nationalite hors de la liste livree (`pays.rs`, vague 1k) refuse aussi, sauf ecrit comme tel
/// (`pays_nationalite_hors_liste`, vague 2h) : la codification est alors celle que l'administrateur a choisie.
pub fn codification_ue(fiche: &Value) -> Resultat<String> {
    let code = texte(fiche, "code_nationalite_dsn");
    if let Some(m) = faute_hors_liste(fiche)? {
        return Err(m);
    }
    if let Some(hors) = texte(fiche, "pays_nationalite_hors_liste") {
        return match code.as_deref() {
            None => Err(format!(
                "Le pays de nationalité de {} (« {} ») n'est pas dans la liste du logiciel : la nationalité que la DSN \
                 déclare ne s'en déduit pas. Dans sa fiche, la choisir (S21.G00.30.013, cahier technique DSN p. 173 : \
                 02 Union européenne, 03 Espace économique européen hors Union, 04 reste du monde).",
                qui(fiche),
                hors
            )),
            Some("01") => Err(format!(
                "La fiche de {} dit la nationalité française (01) et un pays de nationalité hors de la liste (« {} ») : \
                 la France est dans la liste. Choisir 02, 03 ou 04, ou la France dans « Pays de nationalité ».",
                qui(fiche),
                hors
            )),
            Some(c) => Ok(c.to_string()),
        };
    }
    let Some(pays) = texte(fiche, "pays_nationalite") else {
        return match code.as_deref() {
            None => Err(crate::fichiers::absent("code_nationalite_dsn")),
            Some("03") => Err(format!(
                "La nationalité de {} est codée 03. Jusqu'au 10/10/2026, 03 voulait dire « un autre pays » ; la norme DSN \
                 en fait l'Espace économique européen hors Union (Islande, Liechtenstein, Norvège), et le reste du monde \
                 est 04 (S21.G00.30.013, cahier technique p. 173). Dans sa fiche, indiquer son pays de nationalité, ou \
                 choisir de nouveau la nationalité (04 pour un pays hors Union et hors EEE).",
                qui(fiche)
            )),
            Some(c) => Ok(c.to_string()),
        };
    };
    // LE PAYS SE LIT DANS LA LISTE LIVREE (vague 1k, `pays.rs`) : un code qu'elle ne connait pas refuse ; un territoire
    // prend la codification de son pays de rattachement.
    let Some(p) = crate::pays::du_code(&pays)? else {
        return Err(crate::pays::inconnu_nationalite(&format!("Le pays de nationalité de {}", qui(fiche)), &pays));
    };
    let iso = p.code.clone();
    let attendu = crate::pays::codification(&p);
    match code.as_deref() {
        // L'ancienne codification : 03 « autre pays » (aujourd'hui 04), et 02 pour l'Islande, le Liechtenstein et la
        // Norvege (aujourd'hui 03). Le pays tranche.
        Some(c) if c != attendu && !(c == "03" && attendu == "04") && !(c == "02" && attendu == "03") => Err(format!(
            "La fiche de {} dit la nationalité {} et le pays de nationalité {}, qui se déclare {} (S21.G00.30.013 : 01 \
             France, 02 Union européenne, 03 EEE hors Union, 04 reste du monde). Corriger l'un ou l'autre dans sa fiche.",
            qui(fiche),
            c,
            iso,
            attendu
        )),
        _ => Ok(attendu.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn la_table_a_vingt_sept_etats_et_trois_de_l_eee() {
        assert_eq!(UNION.len() + 1, 27);
        assert_eq!(code_du_pays("FR"), "01");
        assert_eq!(code_du_pays("IT"), "02");
        assert_eq!(code_du_pays("NO"), "03");
        for p in ["GB", "CH", "MA", "US"] {
            assert_eq!(code_du_pays(p), "04", "{}", p);
        }
    }

    #[test]
    fn un_ancien_03_sans_pays_refuse() {
        let e = codification_ue(&json!({"prenom": "Ali", "nom": "X", "code_nationalite_dsn": "03"})).unwrap_err();
        assert!(e.contains("Ali X") && e.contains("04"), "{}", e);
        assert_eq!(codification_ue(&json!({"code_nationalite_dsn": "03", "pays_nationalite": "MA"})).unwrap(), "04");
        assert_eq!(codification_ue(&json!({"code_nationalite_dsn": "02", "pays_nationalite": "NO"})).unwrap(), "03");
        assert!(codification_ue(&json!({"code_nationalite_dsn": "02", "pays_nationalite": "US"})).is_err());
    }
}

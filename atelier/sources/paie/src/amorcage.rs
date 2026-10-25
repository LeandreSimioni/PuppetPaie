//! Le signalement d'amorcage des donnees variables (SADV), nature 08
//! (`moteur/amorcage.py`).
//!
//! La seule facon de DEMANDER a la DGFiP le taux de prelevement a la source
//! d'un salarie. Un signalement, un individu : l'envoi, la declaration,
//! l'entreprise, l'etablissement, l'individu, le contrat -- et rien de ce qui
//! se paye. Les refus gardent les phrases du Python ; `Echec::Impossible` est
//! ce qui devient `AmorcageImpossible` cote Python.

use std::collections::HashSet;

use serde_json::Value;

use crate::calcul::Paie;
use droit::contexte::{aujourdhui, Contexte};
use socle::date::{en_texte, Jour};
use crate::dsn::{
    affiliations, bloc_anciennete, bloc_contrat, g, jj, jour, liste, req, usage_admet, Fichier, LOGICIEL, VERSION,
};
use crate::fctu::Echec;
use socle::fichiers::{dossier_du_code, json_en_cache, lire_json, repr, vrai};
use socle::num::Resultat;

pub const NATURE: &str = "08";
pub const EMBAUCHE: &str = "01";
pub const CHANGEMENT_DE_LOGICIEL: &str = "07";

pub const NORMALE: &str = "01";
pub const ANNULE: &str = "04";
/// Les types qu'un amorcage admet. Pas de « 03 - annule et remplace integral » : « CCH-15 : Le type de declaration "03 -
/// declaration annule et remplace integral" est interdit pour une nature de declaration (S20.G00.05.001) "08 -
/// Signalement amorcage des donnees variables". » (cahier technique 2026.1, p. 135,
/// `atelier/reference/norme-dsn/texte/bloc-S20.G00.05.txt`). Un amorcage se corrige en l'annulant, puis en en deposant
/// un nouveau (§ 1.4.3.4, p. 15). Reste R1 #52.
const TYPES: [(&str, &str); 2] = [(NORMALE, "normale"), (ANNULE, "annule")];

fn motifs_de_la_norme() -> Resultat<Vec<Value>> {
    let n = json_en_cache(&dossier_du_code().join("amorcage-base.json"))?;
    Ok(liste(&n["motifs"]["codes"]).to_vec())
}

/// `amorcage.libelle_motif` : le libelle du motif, « ? » s'il est inconnu.
pub fn libelle_motif(motif: &str) -> Resultat<String> {
    Ok(motifs_de_la_norme()?
        .iter()
        .find(|c| en_texte(&c["code"]) == motif)
        .and_then(|c| c.get("libelle").map(en_texte))
        .unwrap_or_else(|| "?".to_string()))
}

/// Une embauche (`operations_paies.Embauche`) : un engagement et sa fiche
/// salarie, sans paie. C'est ce que la DPAE declare, et tout ce que lit un
/// signalement d'amorcage ; les montants de la `Paie` rendue sont a zero.
pub fn embauche(ctx: &Contexte, eng: &crate::engagement::Engagement) -> Resultat<Paie> {
    let nat = eng.nat()?;
    // La fiche telle que la paie la lit (`calcul::fiche_de_l_engagement`) : au premier jour d'un contrat d'usage, au
    // mois paye d'un contrat paye au mois.
    let sal = crate::calcul::fiche_de_l_engagement(ctx, eng)?;
    let spec = if nat.mensuelle() {
        Value::Null
    } else {
        ctx.spectacle(eng.spectacle.as_deref().ok_or_else(|| socle::fichiers::absent("spectacle"))?)?
    };
    let zero = socle::num::Decimal::new(0, 2);
    Ok(Paie::nouveau(eng.clone(), sal, spec, zero, zero, zero, 0.0, zero))
}

/// Les parametres de `generer`, avec les defauts du Python.
pub struct Options {
    pub motif: String,
    pub type_declaration: String,
    pub ordre: i64,
    pub remplace: Option<String>,
    pub envoi_test: bool,
    pub date_constitution: Option<Jour>,
}

/// `verifier` : refuse ce que la norme -- ou le bon sens de la demande -- refuse.
pub fn verifier(paie: &Paie, motif: &str, type_declaration: &str, remplace: Option<&str>) -> Result<(), Echec> {
    let impossible = |m: String| Err(Echec::Impossible(m));
    let tous = motifs_de_la_norme()?;
    let m = match tous.iter().find(|c| en_texte(&c["code"]) == motif) {
        Some(m) => m,
        None => {
            return impossible(format!(
                "Nature d'evenement declencheur {} inconnue de la norme (S20.G00.05.011). Les sept valeurs sont \
dans amorcage-base.json.",
                repr(motif)
            ))
        }
    };
    if !vrai(g(m, "saisissable")) {
        let portes: Vec<String> = tous
            .iter()
            .filter(|c| vrai(g(c, "saisissable")))
            .map(|c| format!("{} ({})", en_texte(&c["code"]), en_texte(&c["libelle"])))
            .collect();
        return impossible(format!(
            "Le motif {} ({}) existe dans la norme, mais ce moteur ne l'emet pas : il sert l'affiliation aux \
organismes complementaires, pas la demande d'un taux de PAS. Les deux portes d'un salarie sont {}.",
            motif,
            en_texte(&m["libelle"]),
            portes.join(" et ")
        ));
    }
    if type_declaration == "03" {
        return impossible(
            "Un signalement d'amorcage ne s'annule et ne se remplace pas d'un seul envoi : le type << 03 - annule et \
remplace integral >> lui est interdit (CCH-15, cahier technique DSN p. 135). Pour le corriger, deposer un << 04 - \
annule >> qui vise le dernier envoi valide, puis un nouveau signalement << 01 - normale >> (paragraphe 1.4.3.4, p. 15)."
                .into(),
        );
    }
    let libelle_type = match TYPES.iter().find(|(c, _)| *c == type_declaration) {
        Some((_, l)) => *l,
        None => {
            return impossible(format!(
                "Type de declaration {}. Un signalement d'amorcage est {}.",
                repr(type_declaration),
                TYPES.iter().map(|(k, v)| format!("<< {} - {} >>", k, v)).collect::<Vec<_>>().join(", ")
            ))
        }
    };
    let remplace = remplace.filter(|r| !r.is_empty());
    if type_declaration == ANNULE && remplace.is_none() {
        return impossible(format!(
            "Un signalement << {} >> doit viser la declaration qu'il reprend (S20.G00.05.006) : l'identifiant du \
dernier envoi VALIDE par le point de depot, lu sur son certificat de conformite. Sans lui, la plate-forme rend \
B1-208-08.",
            libelle_type
        ));
    }
    if type_declaration == NORMALE && remplace.is_some() {
        return impossible(
            "S20.G00.05.006 est interdite sur un signalement normal (CCH-12) : elle ne s'ecrit que sur un \
annule-et-remplace ou un annule."
                .into(),
        );
    }

    // Le NIR, et pas un numero technique temporaire : sans NIR, la DGFiP n'a
    // personne a qui repondre.
    let s = &paie.salarie;
    if crate::ntt::nir_de(s).is_empty() {
        let qui = ["_cle", "nom"].iter().map(|k| g(s, k)).find(|v| vrai(v)).map(en_texte).unwrap_or_else(|| "Ce salarie".into());
        return impossible(format!(
            "{} n'a pas de NIR : il se declare sous numero technique temporaire, et la DGFiP n'a alors personne a \
qui repondre. Son taux de PAS restera le bareme de l'article 204 H jusqu'a son immatriculation -- c'est la loi, pas \
un manque du logiciel.",
            qui
        ));
    }
    Ok(())
}

/// Le fichier d'un signalement d'amorcage, pret a deposer.
pub fn generer(ctx: &Contexte, paie: &Paie, o: &Options) -> Result<String, Echec> {
    verifier(paie, &o.motif, &o.type_declaration, o.remplace.as_deref())?;
    let e = &paie.engagement;
    let s = &paie.salarie;
    let ent = &ctx.entreprise;
    let constitution = match o.date_constitution {
        Some(j) => j,
        None => aujourdhui()?,
    };
    // Le bareme ne sert a rien ici : le taux AT/MP est interdit en nature 08.
    let bar = Value::Object(Default::default());
    let remplace = o.remplace.as_deref().filter(|r| !r.is_empty());

    let ent_ = |k: &str| -> Resultat<&Value> { req(ent, k) };
    // La version de la norme d'un signalement suit le jour ou il est constitue (`norme-dsn-base.json`, une inference).
    let norme = crate::norme::du_signalement(&constitution)?;
    crate::naf::verifier_pour(&norme, ent)?;
    let mut f = Fichier::en_norme(norme);
    // ------------------------------------------------------------ S10 envoi
    f.add("S10.G00.00.001", LOGICIEL);
    f.add("S10.G00.00.002", LOGICIEL);
    f.add("S10.G00.00.003", VERSION);
    f.add("S10.G00.00.005", if o.envoi_test { "01" } else { "02" });
    f.ecrire_la_version()?;
    f.add("S10.G00.00.007", "01");
    // << 02 - envoi neant >> n'a pas de sens ici.
    f.add("S10.G00.00.008", "01");
    f.add_v("S10.G00.01.001", ent_("siren")?)?;
    f.add_v("S10.G00.01.002", ent_("nic")?)?;
    f.add_v("S10.G00.01.003", ent_("raison_sociale")?)?;
    f.add_v("S10.G00.01.004", ent_("adresse")?)?;
    f.add_v("S10.G00.01.005", ent_("code_postal")?)?;
    f.add_v("S10.G00.01.006", ent_("commune")?)?;
    crate::dsn::contact_emetteur(&mut f, ent)?;
    let decl = ent_("declarant_dsn")?;

    // ---------------------------------------------------------- S20 declaration
    f.add("S20.G00.05.001", NATURE);
    f.add("S20.G00.05.002", &o.type_declaration);
    f.add("S20.G00.05.003", "11");
    f.add("S20.G00.05.004", o.ordre);
    if let Some(r) = remplace {
        f.add("S20.G00.05.006", r);
    }
    f.add("S20.G00.05.007", constitution.jjmmaaaa());
    // La rubrique qui fait tout le message : sans elle, le point de depot ne
    // sait pas a qui transmettre le signalement (fiche 2984).
    f.add("S20.G00.05.011", &o.motif);
    // L'etablissement est celui du salarie, comme dans ses mensuelles (`dsn::etablissement_du_salarie`) : le NIC est
    // une donnee identifiante (cahier technique DSN 2026.1, § 2.5.1, p. 67).
    let etab = crate::dsn::etablissement_du_salarie(ent, s)?;
    // Le changement de logiciel nomme l'etablissement qui declarait le dernier contrat connu du salarie (S20.G00.05.012,
    // p. 138) : le sien.
    let changement = o.motif == CHANGEMENT_DE_LOGICIEL;
    if changement {
        let siret = if vrai(g(&etab, "siret")) {
            en_texte(&etab["siret"])
        } else {
            format!("{}{}", en_texte(req(&etab, "siren")?), en_texte(req(&etab, "nic")?))
        };
        f.add("S20.G00.05.012", siret);
    }
    f.add_v("S20.G00.07.001", req(decl, "nom_prenom")?)?;
    f.add_v("S20.G00.07.002", req(decl, "tel")?)?;
    f.add_v("S20.G00.07.003", req(decl, "email")?)?;
    f.add_v("S20.G00.07.004", &crate::dsn::type_du_contact(decl))?;

    // ------------------------------------------------------------ S21 entreprise
    f.add_v("S21.G00.06.001", ent_("siren")?)?;
    f.add_v("S21.G00.06.002", ent_("nic")?)?;
    for (rub, k) in [
        ("S21.G00.11.001", "nic"),
        ("S21.G00.11.002", "naf"),
        ("S21.G00.11.003", "adresse"),
        ("S21.G00.11.004", "code_postal"),
        ("S21.G00.11.005", "commune"),
    ] {
        // Le code APET : obligatoire en amorcage en P26V01, retire de la norme P27V01 (cahier 2027.1.1, p. 145).
        if rub == "S21.G00.11.002" && !usage_admet(&f, rub, NATURE)? {
            continue;
        }
        f.add_v(rub, req(&etab, k)?)?;
    }

    // Les adhesions prevoyance que le contrat designe, et elles seules -- sauf
    // au changement de logiciel, qui n'affilie rien.
    let orgs = lire_json(&ctx.racine.join("config").join("organismes.json"))?;
    let rattachees: HashSet<String> = if changement {
        HashSet::new()
    } else {
        // Celles auxquelles le salarie est affilie a l'embauche (`dsn::rattachements`) : un bloc 15 que nul bloc 70 ne
        // cite n'aurait personne de couvert.
        let refs = affiliations(&orgs, &*e.nat()?, &Value::Null, None)?;
        crate::dsn::rattachements(paie, &orgs, ent, refs)?.into_iter().map(|a| a.reference).collect()
    };
    for a in liste(req(&orgs, "adhesions_prevoyance")?) {
        if !rattachees.contains(&en_texte(req(a, "reference")?)) {
            continue;
        }
        f.add_v("S21.G00.15.001", req(a, "libelle")?)?;
        f.add_v("S21.G00.15.002", req(a, "delegataire")?)?;
        f.add_v("S21.G00.15.005", req(a, "reference")?)?;
    }

    // ------------------------------------------------------------ S21 individu
    crate::ntt::poser_nir(&mut f, s)?;
    f.add_v("S21.G00.30.002", req(s, "nom")?)?;
    f.add_v("S21.G00.30.004", req(s, "prenom")?)?;
    f.add("S21.G00.30.006", jj(req(s, "date_naissance")?)?);
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
        // Le pays de naissance se lit dans la liste livree (`pays.rs`, vague 1k) : un code inconnu refuse.
        let v = if k == "pays_naissance" { socle::pays::de_naissance(s)? } else { req(s, k)?.clone() };
        f.add_v(rub, &v)?;
    }
    crate::ntt::poser_ntt(&mut f, ent, s, &e.debut)?;

    // ------------------------------------------------------------ S21 contrat
    if !changement {
        // Les emplois multiples (`40.036`) : les autres contrats du salarie dans le mois de l'embauche.
        let du_mois = crate::dsn::contrats_du_mois(ctx, &e.debut_periode()?.mois_iso())?;
        let emplois = crate::dsn::emplois_multiples(paie, &du_mois.iter().collect::<Vec<_>>());
        bloc_contrat(&mut f, paie, &etab, &orgs, 1, &bar, false, &[], NATURE, emplois)?;
        // L'anciennete, due des qu'une adhesion prevoyance est declaree.
        let depuis = if vrai(g(s, "date_anciennete")) { jour(&s["date_anciennete"])? } else { e.debut };
        bloc_anciennete(&mut f, paie, 1.max(e.debut.moins(&depuis) + 1))?;
    }

    let n = f.lignes.len() + 2;
    f.add("S90.G00.90.001", n);
    f.add("S90.G00.90.002", "1");
    let texte = f.texte();
    crate::usages::verifier_fichier(&texte)?;
    crate::formats::verifier_fichier(&texte)?;
    crate::ntt::verifier_fichier(&texte)?;
    Ok(texte)
}

//! Le signalement d'amorcage des donnees variables (SADV), nature 08.
//!
//! La seule facon de DEMANDER a la DGFiP le taux de prelevement a la source
//! d'un salarie. Un signalement, un individu : l'envoi, la declaration,
//! l'entreprise, l'etablissement, l'individu, le contrat -- et rien de ce qui
//! se paye. `Echec::Impossible` est la regle de la norme qui s'oppose.

use droit::fiche::Lue as _;
use std::collections::HashSet;

use serde_json::Value;

use engagement::calcul::Paie;
use droit::contexte::{aujourdhui, Contexte};
use socle::date::Jour;
use documents::valeur::{en_texte, json_en_cache, vrai};
use crate::dsn::{
    affiliations, bloc_anciennete, bloc_contrat, g, liste, usage_admet, Fichier, LOGICIEL, VERSION,
};
use crate::fctu::Echec;
use socle::fichiers::{dossier_du_code, repr};
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
/// un nouveau (§ 1.4.3.4, p. 15).
const TYPES: [(&str, &str); 2] = [(NORMALE, "normale"), (ANNULE, "annule")];

fn motifs_de_la_norme() -> Resultat<Vec<Value>> {
    let n = json_en_cache(&dossier_du_code().join("amorcage-base.json"))?;
    Ok(liste(&n["motifs"]["codes"]).to_vec())
}

/// Le libelle du motif, « ? » s'il est inconnu.
pub fn libelle_motif(motif: &str) -> Resultat<String> {
    Ok(motifs_de_la_norme()?
        .iter()
        .find(|c| en_texte(&c["code"]) == motif)
        .and_then(|c| c.get("libelle").map(en_texte))
        .unwrap_or_else(|| "?".to_string()))
}

/// Les parametres de `generer`, avec leurs defauts.
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
    if crate::ntt::nir_de(&paie.salarie).is_none() {
        let qui = paie.salarie.id.to_string();
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
pub fn generer(ctx: &Contexte, paie: &Paie, o: &Options, departs: &engagement::departs::Departs) -> Result<String, Echec> {
    verifier(paie, &o.motif, &o.type_declaration, o.remplace.as_deref())?;
    let ctx = &ctx.au(&paie.engagement.debut)?;
    let e = &paie.engagement;
    let s = &paie.salarie;
    let ent = &ctx.entreprise;
    crate::dsn::exiges(ent, std::iter::once(&paie.salarie), paie.declaree_a_la_caisse_des_conges()?)?;
    let constitution = match o.date_constitution {
        Some(j) => j,
        None => aujourdhui()?,
    };
    // Le bareme ne sert a rien ici : le taux AT/MP est interdit en nature 08.
    let bar = crate::dsn::BaremeDuMois::nouveau(droit::forme_bareme::Pile::default(), droit::bareme::EntrepriseLue { fiche: ent.clone(), effectif: None, taux_lus: false });
    let remplace = o.remplace.as_deref().filter(|r| !r.is_empty());

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
    f.add_case("S10.G00.01.001", &ent.siren, "siren")?;
    f.add_case("S10.G00.01.002", &ent.nic, "nic")?;
    f.add_case("S10.G00.01.003", &ent.raison_sociale, "raison_sociale")?;
    f.add_case("S10.G00.01.004", &ent.adresse, "adresse")?;
    f.add_case("S10.G00.01.005", &ent.code_postal, "code_postal")?;
    f.add_case("S10.G00.01.006", &ent.commune, "commune")?;
    f.add_complement("S10.G00.01.009", &ent.complement_adresse);
    crate::dsn::contact_emetteur(&mut f, ent)?;

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
    // L'etablissement est celui du salarie, comme dans ses mensuelles (`droit::bareme::entreprise_du_salarie`) : le NIC est
    // une donnee identifiante (cahier technique DSN 2026.1, § 2.5.1, p. 67).
    let etab = droit::bareme::entreprise_du_salarie(ent, &paie.salarie)?;
    // Le changement de logiciel nomme l'etablissement qui declarait le dernier contrat connu du salarie (S20.G00.05.012,
    // p. 138) : le sien.
    let changement = o.motif == CHANGEMENT_DE_LOGICIEL;
    if changement {
        let siret = match etab.siret.remplie() {
            Some(s) => s.to_string(),
            None => format!("{}{}", etab.siren.ecrite("siren")?, etab.nic.ecrite("nic")?),
        };
        f.add("S20.G00.05.012", siret);
    }
    crate::dsn::contact_chez_le_declare(&mut f, ent)?;

    // ------------------------------------------------------------ S21 entreprise
    f.add_case("S21.G00.06.001", &ent.siren, "siren")?;
    f.add_case("S21.G00.06.002", &ent.nic, "nic")?;
    for (rub, case, cle) in [
        ("S21.G00.11.001", &etab.nic, "nic"),
        ("S21.G00.11.002", &etab.naf.en_texte(), "naf"),
        ("S21.G00.11.003", &etab.adresse.en_texte(), "adresse"),
        ("S21.G00.11.004", &etab.code_postal.en_texte(), "code_postal"),
        ("S21.G00.11.005", &etab.commune.en_texte(), "commune"),
    ] {
        // Le code APET : obligatoire en amorcage en P26V01, retire de la norme P27V01 (cahier 2027.1.1, p. 145).
        if rub == "S21.G00.11.002" && !usage_admet(&f, rub, NATURE)? {
            continue;
        }
        f.add_case(rub, case, cle)?;
        if rub == "S21.G00.11.005" {
            f.add_complement("S21.G00.11.006", &etab.complement_adresse);
        }
    }

    // Les adhesions prevoyance que le contrat designe, et elles seules -- sauf
    // au changement de logiciel, qui n'affilie rien.
    let orgs = ctx.organismes()?;
    let rattachees: HashSet<String> = if changement {
        HashSet::new()
    } else {
        // Celles auxquelles le salarie est affilie a l'embauche (`dsn::rattachements`) : un bloc 15 que nul bloc 70 ne
        // cite n'aurait personne de couvert.
        let refs = affiliations(&orgs, &*e.nat()?, &[], (e.debut_periode()?, e.fin_periode()?))?;
        crate::dsn::rattachements(paie, &orgs, ent, refs)?.into_iter().map(|a| a.reference).collect()
    };
    for a in &orgs.adhesions_prevoyance {
        if !rattachees.contains(&a.reference) {
            continue;
        }
        f.add("S21.G00.15.001", &a.libelle);
        f.add("S21.G00.15.002", &a.delegataire);
        f.add("S21.G00.15.005", &a.reference);
    }

    // ------------------------------------------------------------ S21 individu
    crate::dsn::bloc_individu(&mut f, s, true)?;
    crate::ntt::poser_ntt(&mut f, ent, s, crate::changements_individu::derniere_declaree(ctx, &paie.salarie, &e.debut, departs)?.as_ref())?;

    // ------------------------------------------------------------ S21 contrat
    if !changement {
        // Les emplois multiples (`40.036`) : les autres contrats du salarie dans le mois de l'embauche.
        let du_mois = crate::dsn::contrats_du_mois(ctx, &e.debut_periode()?.mois_civil().to_string(), departs)?;
        let emplois = crate::dsn::emplois_multiples(paie, &du_mois.iter().collect::<Vec<_>>());
        bloc_contrat(&mut f, paie, &etab, &orgs, 1, &bar, false, &[], NATURE, emplois)?;
        // L'anciennete, due des qu'une adhesion prevoyance est declaree.
        let depuis = s.date_anciennete.dite().copied().unwrap_or(e.debut);
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

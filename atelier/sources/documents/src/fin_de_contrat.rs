//! Le certificat de travail et le reçu pour solde de tout compte d'un contrat qui se termine : deux papiers que la
//! compagnie remet au salarié, rien ne part nulle part. Ils se fabriquent depuis la paie validée de l'engagement.
//!
//! Le certificat se délivre à l'expiration du contrat (L1234-19) et ne porte que les mentions de D1234-6 ; le reçu
//! fait l'inventaire des sommes versées (L1234-20), ligne à ligne, et s'établit en double exemplaire (D1234-7).

use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};

use crate::bulletin::{fmt_eur, identite_de, jj_mm_aaaa};
use droit::contexte::Contexte;
use engagement::calcul::Paie;
use socle::date::{en_texte, Jour};
use socle::echec::{erreur, R};
use socle::num::eur2;

/// Ce qu'il faut savoir du reçu et du certificat en les remettant.
pub const AVIS: [&str; 3] = [
    "Le reçu pour solde de tout compte s'établit en double exemplaire (D1234-7) : un pour le salarié, un pour la compagnie. Il se signe de sa main, avec la mention « pour solde de tout compte ».",
    "Il peut être dénoncé par lettre recommandée dans les six mois (L1234-20 et D1234-8) ; passé ce délai, il est libératoire pour les seules sommes qui y sont inscrites.",
    "Le certificat de travail ne contient que les mentions de D1234-6 : dates d'entrée et de sortie, emplois occupés et leurs périodes.",
];

/// Écrit les deux papiers de la paie `p` (dont l'engagement finit), établis le `date`, dans `sortie` : le certificat
/// puis le reçu ; les deux ou aucun (un reçu qui ne s'écrit pas emporte le certificat déjà posé).
pub fn ecrire(ctx: &Contexte, p: &Paie, date: Jour, sortie: &Path) -> R<[PathBuf; 2]> {
    let e = &p.engagement;
    let numero = e.numero.clone().unwrap_or_else(|| "?".to_string());
    let Some(fin) = e.fin else {
        return Err(erreur(
            "ValueError",
            format!("Certificat de travail : le contrat {} n'a pas de date de fin. Il se délivre à l'expiration du contrat (L1234-19).", numero),
        ));
    };
    let cle = p.salarie.get("_cle").map(en_texte).unwrap_or_default();
    let ent = ctx.entreprise_et_gabarits();
    let poste = ctx.gabarits();
    let certificat = sortie.join(format!("CertificatTravail_{}_{}.pdf", cle, fin.aaaammjj()));
    let recu = sortie.join(format!("RecuSoldeToutCompte_{}_{}.pdf", cle, fin.aaaammjj()));
    let valeurs_cert = marqueurs_communs(&date, p, &ent)?;
    let mut valeurs = valeurs_cert.clone();
    valeurs.insert("solde.inventaire".into(), json!(inventaire(p)));
    valeurs.insert("solde.net".into(), json!(fmt_eur(eur2(p.net_a_payer))));
    ecrire_papier(&socle::fichiers::modele_de("certificat-travail.modele", &ent, &poste), valeurs_cert, &certificat, &ent, true)?;
    if let Err(e) = ecrire_papier(&socle::fichiers::modele_de("recu-solde-de-tout-compte.modele", &ent, &poste), valeurs, &recu, &ent, false) {
        let _ = std::fs::remove_file(&certificat);
        return Err(e);
    }
    Ok([certificat, recu])
}

/// Les mentions communes du certificat de travail et du reçu.
fn marqueurs_communs(date: &Jour, p: &Paie, ent: &Value) -> R<Map<String, Value>> {
    let s = &p.salarie;
    let e = &p.engagement;
    let req = |v: &Value, k: &str| -> R<String> { v.get(k).map(en_texte).ok_or_else(|| erreur("KeyError", socle::fichiers::absent(k))) };
    let ou_vide = |v: &Value, k: &str| -> String { v.get(k).map(en_texte).unwrap_or_default() };
    let sig = ent.get("signataire").ok_or_else(|| erreur("KeyError", socle::fichiers::absent("signataire")))?;
    let aujourdhui = date.jj_mm_aaaa();
    let mut m = Map::new();
    let mut poser = |k: &str, v: String| {
        m.insert(k.to_string(), Value::String(v));
    };
    poser("employeur.raison_sociale", req(ent, "raison_sociale")?);
    poser("employeur.adresse", req(ent, "adresse")?);
    poser("employeur.code_postal", req(ent, "code_postal")?);
    poser("employeur.commune", req(ent, "commune")?);
    poser("employeur.siret", req(ent, "siret")?);
    poser("employeur.naf", req(ent, "naf")?);
    poser("employeur.signataire", format!("{} {}", req(sig, "prenom")?, req(sig, "nom")?));
    poser("employeur.signataire_fonction", ou_vide(sig, "fonction"));
    poser("salarie.nom_complet", format!("{} {}", req(s, "prenom")?, req(s, "nom")?));
    poser(
        "salarie.date_naissance",
        jj_mm_aaaa(s.get("date_naissance").ok_or_else(|| erreur("KeyError", socle::fichiers::absent("date_naissance")))?).map_err(|m| erreur("ValueError", m))?,
    );
    poser("salarie.lieu_naissance", ou_vide(s, "lieu_naissance"));
    poser("salarie.adresse", req(s, "adresse")?);
    poser("salarie.code_postal", req(s, "code_postal")?);
    poser("salarie.commune", req(s, "commune")?);
    poser("contrat.numero", e.numero.clone().unwrap_or_default());
    poser("contrat.debut", e.debut.jj_mm_aaaa());
    poser("contrat.fin", e.fin.map(|f| f.jj_mm_aaaa()).unwrap_or_default());
    poser("contrat.emploi", ou_vide(s, "emploi"));
    poser("contrat.nature", e.nat().map_err(|m| erreur("ValueError", m))?.libelle.clone());
    poser("certificat.date", aujourdhui.clone());
    poser("recu.date", aujourdhui);
    Ok(m)
}

/// Le détail des sommes, ligne à ligne -- pas un total.
fn inventaire(p: &Paie) -> String {
    let mut lignes = Vec::new();
    for l in &p.remuneration {
        lignes.push(format!("{} : {}", l.libelle, fmt_eur(eur2(l.montant))));
    }
    for l in &p.lignes {
        let m = eur2(l.montant_sal);
        if m != socle::num::Decimal::ZERO {
            lignes.push(format!("{} : -{}", l.libelle, fmt_eur(m)));
        }
    }
    lignes.join("\n")
}

/// Le papier de `modele`, par la fabrique (style « attestation »). `signe` : le certificat s'établit signé de
/// l'employeur.
fn ecrire_papier(modele: &Path, valeurs: Map<String, Value>, chemin: &Path, ent: &Value, signe: bool) -> R<()> {
    let texte = crate::contrat::lire_modele_texte(modele)?;
    let mut d = crate::contrat::enveloppe("attestation", ent);
    let mut un = Map::new();
    un.insert("texte".into(), json!(texte));
    un.insert("valeurs".into(), Value::Object(valeurs));
    if signe {
        if let Some(s) = identite_de(ent, "signature") {
            un.insert("signature_employeur".into(), json!(crate::conges_spectacles::chemin_texte(&s)));
        }
    }
    d.insert("documents".into(), json!([{ "modele": Value::Object(un) }]));
    crate::fabrique::ecrire(&Value::Object(d), chemin).map_err(|m| erreur("FabriqueRefus", m.replace('\n', " ").trim().to_string()))?;
    Ok(())
}

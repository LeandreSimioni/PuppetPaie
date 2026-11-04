//! Le certificat de travail et le reçu pour solde de tout compte d'un contrat qui se termine : deux papiers que la
//! compagnie remet au salarié, rien ne part nulle part. Ils se fabriquent depuis la paie validée de l'engagement.
//!
//! Le certificat se délivre à l'expiration du contrat (L1234-19) et ne porte que les mentions de D1234-6 ; le reçu
//! fait l'inventaire des sommes versées (L1234-20), ligne à ligne, et s'établit en double exemplaire (D1234-7).

use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};

use crate::bulletin::{fmt_eur, identite_de};
use crate::marqueurs::{dans_la_description, Marqueur, ModeleLu, Valeurs};
use droit::contexte::Contexte;
use engagement::calcul::Paie;
use socle::date::Jour;
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
    let numero = e.numero.as_deref().unwrap_or("?").to_string();
    let Some(fin) = e.fin else {
        return Err(erreur(
            "ValueError",
            format!("Certificat de travail : le contrat {} n'a pas de date de fin. Il se délivre à l'expiration du contrat (L1234-19).", numero),
        ));
    };
    let cle = p.salarie.id.to_string();
    let ent = ctx.entreprise_et_gabarits();
    let poste = ctx.gabarits();
    let certificat = sortie.join(format!("CertificatTravail_{}_{}.pdf", cle, fin.aaaammjj()));
    let recu = sortie.join(format!("RecuSoldeToutCompte_{}_{}.pdf", cle, fin.aaaammjj()));
    let valeurs_cert = marqueurs_communs(&date, p, &ent)?;
    let mut valeurs = valeurs_cert.clone();
    valeurs.insert(Marqueur::SoldeInventaire, inventaire(p));
    valeurs.insert(Marqueur::SoldeNet, fmt_eur(eur2(p.net_a_payer)));
    ecrire_papier(&socle::fichiers::modele_de("certificat-travail.modele", Some(&ent.gabarits), &poste), valeurs_cert, &certificat, &ent, true)?;
    if let Err(e) = ecrire_papier(&socle::fichiers::modele_de("recu-solde-de-tout-compte.modele", Some(&ent.gabarits), &poste), valeurs, &recu, &ent, false) {
        let _ = std::fs::remove_file(&certificat);
        return Err(e);
    }
    Ok([certificat, recu])
}

/// Les mentions communes du certificat de travail et du reçu.
fn marqueurs_communs(date: &Jour, p: &Paie, ent: &droit::fiche::ChampsEntreprise) -> R<Valeurs> {
    let s = &p.salarie;
    let e = &p.engagement;
    fn ecrite<T: AsRef<str>>(c: &droit::fiche::Case<T>, k: &str) -> R<String> {
        c.ecrite(k).map(str::to_string).map_err(|m| erreur("KeyError", m))
    }
    let sig = ent.signataire.exigee("signataire").map_err(|m| erreur("KeyError", m))?;
    let aujourdhui = date.jj_mm_aaaa();
    let mut m = Valeurs::new();
    let mut poser = |k: Marqueur, v: String| {
        m.insert(k, v);
    };
    poser(Marqueur::EmployeurRaisonSociale, ecrite(&ent.raison_sociale, "raison_sociale")?);
    poser(Marqueur::EmployeurAdresse, droit::fiche::avec_complement(ecrite(&ent.adresse.en_texte(), "adresse")?, ent.complement_adresse.remplie()));
    poser(Marqueur::EmployeurCodePostal, ecrite(&ent.code_postal, "code_postal")?);
    poser(Marqueur::EmployeurCommune, ecrite(&ent.commune, "commune")?);
    poser(Marqueur::EmployeurSiret, ecrite(&ent.siret, "siret")?);
    poser(Marqueur::EmployeurNaf, ecrite(&ent.naf, "naf")?);
    poser(Marqueur::EmployeurSignataire, format!("{} {}", ecrite(&sig.prenom, "prenom")?, ecrite(&sig.nom, "nom")?));
    poser(Marqueur::EmployeurSignataireFonction, sig.fonction.texte().to_string());
    poser(Marqueur::SalarieNomComplet, format!("{} {}", ecrite(&s.prenom, "prenom")?, ecrite(&s.nom, "nom")?));
    poser(Marqueur::SalarieDateNaissance, s.date_naissance.exigee("date_naissance").map_err(|m| erreur("KeyError", m))?.jj_mm_aaaa());
    poser(Marqueur::SalarieLieuNaissance, s.lieu_naissance.texte().to_string());
    poser(Marqueur::SalarieAdresse, ecrite(&s.adresse, "adresse")?);
    poser(Marqueur::SalarieCodePostal, ecrite(&s.code_postal, "code_postal")?);
    poser(Marqueur::SalarieCommune, ecrite(&s.commune, "commune")?);
    poser(Marqueur::ContratNumero, e.numero.as_deref().unwrap_or_default().to_string());
    poser(Marqueur::ContratDebut, e.debut.jj_mm_aaaa());
    poser(Marqueur::ContratFin, e.fin.map(|f| f.jj_mm_aaaa()).unwrap_or_default());
    poser(Marqueur::ContratEmploi, s.emploi.texte().to_string());
    poser(Marqueur::ContratNature, e.nat().map_err(|m| erreur("ValueError", m))?.libelle.clone());
    poser(Marqueur::CertificatDate, aujourdhui.clone());
    poser(Marqueur::RecuDate, aujourdhui);
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
fn ecrire_papier(modele: &Path, valeurs: Valeurs, chemin: &Path, ent: &droit::contexte::EntrepriseDuDocument, signe: bool) -> R<()> {
    let lu = ModeleLu::du_fichier(modele)?;
    let mut d = crate::contrat::enveloppe("attestation", ent);
    let mut un = Map::new();
    un.insert("texte".into(), json!(lu.texte));
    un.insert("valeurs".into(), dans_la_description(&valeurs));
    if signe {
        if let Some(s) = identite_de(&ent.gabarits, "signature") {
            un.insert("signature_employeur".into(), json!(crate::conges_spectacles::chemin_texte(&s)));
        }
    }
    d.insert("documents".into(), json!([{ "modele": Value::Object(un) }]));
    crate::fabrique::ecrire(&Value::Object(d), chemin).map_err(|m| erreur("FabriqueRefus", m.replace('\n', " ").trim().to_string()))?;
    Ok(())
}

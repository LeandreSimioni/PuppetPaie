//! `moteur aer [--mois AAAA-MM] [--forcer]` : ranger les attestations employeur de France Travail deja recues.
//!
//! Cette commande ne parle a aucun service : `retours` a deja telecharge et archive les fichiers (nature 46). Ici on les ouvre et
//! l'on ecrit un PDF par salarie dans `sorties/<mois>/envoi/`, sous le meme nom de famille que le bulletin et le certificat CS --
//! c'est ce qui permet a `envoi` de les ramasser. On ne touche pas aux CHIFFRES du PDF (ceux de France Travail font foi) : on remplit
//! le cadre 5, a la charge de l'employeur. Et parce qu'on signe, on verifie d'abord : si l'attestation n'affiche pas les memes
//! cachets et les memes heures que la paie du mois, la commande REFUSE de signer et dit lequel (art. L. 5429-1). `--forcer` signe
//! quand meme : l'ecart est alors connu et assume.
//!
//! Le travail est `crate::routes::documents::ranger_aer_detail` (la meme version que la releve des retours, qui ne force jamais).
//! `--json` est sans effet ici.
//!
//! Une erreur d'un autre genre qu'un ecart (PDF illisible, fabrique) : `! phrase`, code 2.

use super::analyse::{Args, Opt, Spec};
use super::Cadre;
use envois::retours::{NATURE_AER_SPECTACLE};
use socle::echec::Echec;

pub static SPEC: Spec = Spec {
    commande: "aer",
    positionnels: &[],
    options: &[Opt::valeur("mois"), Opt::drapeau("forcer"), Opt::drapeau("json")],
};

pub fn executer(cadre: &Cadre, args: &Args) -> i32 {
    match faire(cadre, args) {
        Ok(c) => c,
        Err(e) => super::actes::reponse_metier("aer", false, &e, &[]),
    }
}

fn faire(cadre: &Cadre, args: &Args) -> Result<i32, Echec> {
    let archives = cadre.racine.join("retours");
    let motif = format!("_nature{}_", NATURE_AER_SPECTACLE);
    let il_y_en_a = std::fs::read_dir(&archives).map(|l| l.flatten().any(|e| e.file_name().to_string_lossy().contains(&motif))).unwrap_or(false);
    if !il_y_en_a {
        dire!(
            "\nAucun retour de nature {} (AER « cinema, spectacle ») dans {}.\n\nDepuis le passage en 100 % DSN, l'attestation employeur est editee par France\nTravail a partir de notre DSN, quelques heures apres le depot. La releve la\nrecupere et la range seule ; a la main :\n\n    moteur retours\n",
            NATURE_AER_SPECTACLE,
            archives.display()
        );
        return Ok(2);
    }

    let mois = args.texte("mois");
    let fait = crate::routes::documents::ranger_aer_detail(&cadre.ctx, mois, None, args.drapeau("forcer"))?;
    if fait.ecrits.is_empty() && fait.a_ranger.is_empty() && fait.refusees.is_empty() {
        dire!(
            "Aucune attestation{} dans les archives de nature {}.",
            mois.map(|m| format!(" pour {}", m)).unwrap_or_default(),
            NATURE_AER_SPECTACLE
        );
        return Ok(2);
    }

    let mut periodes: Vec<&String> = fait.ecrits.iter().map(|(p, _, _)| p).collect();
    periodes.sort();
    periodes.dedup();
    for periode in &periodes {
        let lot: Vec<_> = fait.ecrits.iter().filter(|(p, _, _)| p == *periode).collect();
        let dossier = lot[0].2.parent().map(|p| p.display().to_string()).unwrap_or_default();
        dire!("{} : {} attestation(s) dans {}", periode, lot.len(), dossier);
        for (_, cle, chemin) in lot {
            dire!("   {:<12} {}", cle, chemin.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default());
        }
    }

    if !fait.a_ranger.is_empty() {
        // On ne devine pas : c'est a un humain de trancher.
        let dossier = fait.a_ranger[0].1.parent().map(|p| p.display().to_string()).unwrap_or_default();
        dire!("\n{} attestation(s) non rattachee(s) -- deposees dans {} :", fait.a_ranger.len(), dossier);
        for (qui, chemin) in &fait.a_ranger {
            dire!("   {}  ({})", qui, chemin.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default());
        }
        dire!("Verifier le nom et le prenom dans config/salaries.json, puis relancer, ou les renommer a la main.");
    }

    if !fait.refusees.is_empty() {
        dire!("\n{} attestation(s) NON SIGNEE(S) -- elles ne disent pas ce que la paie dit :", fait.refusees.len());
        for (cle, periode, ecarts) in &fait.refusees {
            dire!("   {}, {}", cle, periode);
            for e in ecarts {
                dire!("      {}", e);
            }
        }
        dire!("\nSigner reviendrait a certifier exacts des renseignements qui ne le sont pas.\nLa correction ne se fait PAS sur le PDF : elle se fait sur la DSN, et France\nTravail reedite l'attestation. Une DSN mensuelle « annule et remplace » est\npossible tant que la date d'exigibilite n'est pas depassee ; a defaut, la\nrectification passe par la DSN du mois suivant.\n\nSi l'ecart est connu et assume, --forcer signe quand meme.");
    }

    if let Some(dernier) = periodes.last() {
        dire!("\nElles partiront avec le message aux salaries :");
        dire!("   moteur envoi {}", dernier);
    }
    Ok(if fait.refusees.is_empty() { 0 } else { 2 })
}

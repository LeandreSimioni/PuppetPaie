//! Les petits outils des commandes qui AGISSENT : ecrire un fichier de saisie, produire un mois, declarer,
//! deposer. Ce qu'elles ont en commun avec la consultation (J2a) vit dans `mod.rs`, `analyse.rs` et `rendu.rs` ; ici, ce
//! que seuls les actes demandent.
//!
//! UNE ERREUR METIER EST UNE REPONSE : une `ErreurOperation` (et ses filles) s'ecrit
//! en une phrase, `! phrase` et le code 2 -- ou, avec `--json`, `sortie_json.erreur`. Une erreur d'un autre genre est un
//! plantage (`Cadre::plantage`, code 1). Le Rust ne distingue les classes que par leur nom, rendu par les routes.

use serde_json::Value;

use super::rendu::{erreur, rendre};
use socle::echec::Echec;

/// Les classes d'un refus metier : `ErreurOperation` et ses filles.
const OPERATION: [&str; 28] = [
    "ErreurOperation", "SaisieIncomplete", "PreuveInsuffisante", "FicheRefusee", "MoisPasPret", "CompteursVerrouilles",
    "VersionExigee", "DsnDejaDeposee", "DpaeDejaDeclaree", "RepetitionManquante", "OrdreTropPetit", "AnnuleRemplaceHorsDelai",
    "MoisNonRegularisable", "ExerciceClos", "ExerciceOuvert", "ClotureRefusee", "MoisHorsDelai", "SortieAbsente", "BoiteAbsente",
    "ArretIntrouvable", "ContratIntrouvable", "FichierExistant", "AucunDocument", "FormatIndisponible", "ExportIntrouvable",
    "ActeReelRefuse", "ModificationApresEcheance", "ModificationNonDeclarable",
];

/// Cette classe est-elle une `ErreurOperation` (ou une de ses filles) ?
pub fn est_operation(classe: Option<&str>) -> bool {
    classe.is_some_and(|c| OPERATION.contains(&c))
}

/// La phrase d'un refus : celle du message, sans la donnee qu'un refus de depot ou de mois porte a cote (`avec_donnee`).
pub fn phrase(e: &Echec) -> String {
    crate::compagnie::depot_acte::phrase_de(e)
}

/// Une erreur metier rendue : `! phrase` (code 2), ou `sortie_json.erreur` avec `--json`.
/// `extra` s'ajoute au corps de l'erreur JSON (`manques`, `chemin`...).
pub fn reponse_metier(commande: &str, json: bool, e: &Echec, extra: &[(&str, Value)]) -> i32 {
    let message = phrase(e);
    if json {
        return rendre(&erreur(commande, e.classe.unwrap_or("ErreurOperation"), &message, extra));
    }
    dire!("! {}", message);
    2
}

/// `AAAA-MM-JJ`, un jour qui existe. Les autres formes ne sont pas portees.
pub fn jour_iso(t: &str) -> Option<socle::date::Jour> {
    let t = t.trim();
    if t.len() != 10 {
        return None;
    }
    let b = t.as_bytes();
    if b[4] != b'-' || b[7] != b'-' || !b[..4].iter().chain(&b[5..7]).chain(&b[8..]).all(u8::is_ascii_digit) {
        return None;
    }
    socle::date::Jour::lire(t).ok()
}

/// `JJ/MM/AAAA` d'un `AAAA-MM-JJ` (`util.jj_mm_aaaa`) ; le texte tel quel s'il ne se lit pas.
pub fn jj_mm_aaaa(iso: &str) -> String {
    jour_iso(&iso.chars().take(10).collect::<String>()).map(|j| j.jj_mm_aaaa()).unwrap_or_else(|| iso.to_string())
}

/// `AAAA-MM` : un mois de saisie bien forme (quatre chiffres, un tiret, un mois de 01 a 12).
pub fn mois_valide(t: &str) -> bool {
    let b = t.as_bytes();
    if b.len() != 7 || b[4] != b'-' || !b[..4].iter().chain(&b[5..]).all(u8::is_ascii_digit) {
        return false;
    }
    matches!(t[5..7].parse::<u32>(), Ok(1..=12))
}

/// Une ligne de `etapes` (`A faire ensuite :`) : `  N. TITRE     detail`, puis la commande s'il y en a une.
pub fn afficher_etapes(etapes: &[Value], titre: &str) {
    dire!("\n{}", titre);
    for (i, e) in etapes.iter().enumerate() {
        dire!("  {}. {:<9} {}", i + 1, super::rendu::champ(e, "titre"), super::rendu::champ(e, "detail"));
        if e.get("commande").is_some_and(socle::fichiers::vrai) {
            dire!("     {}", super::rendu::champ(e, "commande"));
        }
    }
}

/// `Fichiers produits dans DOSSIER :` puis `  nom  (N o)` pour chacun.
pub fn afficher_fichiers(fichiers: &[Value], dossier: &str) {
    dire!("\nFichiers produits dans {} :", dossier);
    for f in fichiers {
        let octets = match f.get("octets") {
            Some(Value::Number(n)) => n.to_string(),
            _ => "None".to_string(),
        };
        dire!("  {}  ({} o)", super::rendu::champ(f, "nom"), octets);
    }
}

/// Un refus que la commande rend en une phrase : une `ErreurOperation`, ou une `ValueError` (la route rend ainsi ce que
/// la norme refuse -- `SignalementImpossible`, `FctuImpossible`, une date qui ne se lit pas --).
pub fn est_refus(classe: Option<&str>) -> bool {
    est_operation(classe) || classe == Some("ValueError")
}

/// `--le AAAA-MM-JJ` : le jour tel que la route le lit (AAAA-MM-JJ), ou le refus d'une phrase.
pub fn jour_option(nom: &str, texte: &str) -> Result<socle::date::Jour, Echec> {
    jour_iso(texte).ok_or_else(|| Echec {
        classe: Some("ErreurOperation"),
        message: format!("--{} attend une date AAAA-MM-JJ, recu {}.", nom, socle::fichiers::repr(texte)),
    })
}

/// Des OPTIONS obligatoires : `moteur COMMANDE : à donner : --a, --b`
/// (usage sur la sortie d'erreur, code 2). `None` quand toutes sont la.
pub fn options_requises(spec: &super::analyse::Spec, args: &super::analyse::Args, noms: &[&str]) -> Option<i32> {
    let manquantes: Vec<String> = noms.iter().filter(|n| !args.drapeau(n)).map(|n| format!("--{}", n)).collect();
    if manquantes.is_empty() {
        return None;
    }
    eprintln!("{}\nmoteur {} : à donner : {}", super::analyse::usage(spec), spec.commande, manquantes.join(", "));
    Some(2)
}

/// Un envoi qui ne part pas : un refus ordinaire (`ErreurOperation` et filles, `ValueError`), ou la boite qui casse
/// (`ErreurCourriel`) et le `RuntimeError` d'un envoi interrompu apres des messages deja partis.
/// Rendus en une phrase.
pub fn est_refus_d_envoi(classe: Option<&str>) -> bool {
    est_refus(classe) || matches!(classe, Some("ErreurCourriel") | Some("RuntimeError"))
}

/// Avant de toucher a la boite courriel : son mot de passe d'application est au coffre, que la cle du poste ouvre ou, a defaut,
/// une seance (`Cadre::ouvrir_le_coffre`). La compagnie fictive n'a ni boite ni coffre a ouvrir. Ceci n'AUTORISE rien : l'envoi reel
/// reste soumis a la preuve du moteur (`depot_acte::verifier_preuve`), qui demande elle-meme le mot de passe a un humain.
pub fn ouvrir_la_boite(cadre: &super::Cadre) {
    if !crate::compagnie::bridage::branchement(&cadre.racine).joue() {
        cadre.ouvrir_le_coffre();
    }
}

/// Liste de textes d'une cle de reponse.
pub fn textes_de(v: &Value, cle: &str) -> Vec<String> {
    v.get(cle).and_then(Value::as_array).map(|l| l.iter().map(super::rendu::texte).collect()).unwrap_or_default()
}

/// Le mois que la commande vise : `AAAA-MM`, ou le fichier `engagements/AAAA-MM.json` des donnees. Refus (`sujet` + la phrase
/// qui le dit) : un autre fichier, ou un mois qu'aucun engagement ne touche.
pub fn mois_du_fichier(cadre: &super::Cadre, texte: &str, sujet: &str) -> Result<String, Echec> {
    let refus = |m: String| Echec { classe: Some("ErreurOperation"), message: m };
    let est_un_mois = |s: &str| s.len() == 7 && s.as_bytes()[4] == b'-' && s.chars().enumerate().all(|(i, c)| i == 4 || c.is_ascii_digit());
    let mois = if est_un_mois(texte) {
        texte.to_string()
    } else {
        let chemin = std::path::PathBuf::from(crate::routes::depots::resoudre(&cadre.racine, &serde_json::json!(texte))?);
        let mois = chemin.file_stem().map(|s| s.to_string_lossy().into_owned()).filter(|s| est_un_mois(s)).unwrap_or_default();
        let attendu = engagement::stock::chemin(&cadre.racine, &mois);
        let meme = matches!((std::fs::canonicalize(&chemin), std::fs::canonicalize(&attendu)), (Ok(a), Ok(b)) if a == b);
        if !meme {
            return Err(refus(format!(
                "{} depuis les engagements du mois (AAAA-MM, ou {}/AAAA-MM.json dans les donnees) ; {} n'est pas ce fichier.",
                sujet,
                engagement::stock::DOSSIER,
                chemin.display()
            )));
        }
        mois
    };
    if !engagement::stock::a_une_saisie(&cadre.racine, &mois).map_err(Echec::from)? {
        return Err(refus(format!("{} depuis les engagements du mois : aucun ne touche {}.", sujet, mois)));
    }
    Ok(mois)
}

// ------------------------------------------------------------------ le binaire de l'atelier

/// Ce que la redirection de l'atelier tient : net-entreprises, l'Urssaf et Google (`atelier::redirection`). Le reste -- la boite courriel --
/// n'est jamais redirige.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Organisme {
    Dsn,
    Dpae,
    Google,
}

/// Dans `moteur-atelier`, HORS COMPAGNIE FICTIVE : l'organisme n'est-il PAS redirige vers un faux serveur local ? Alors une commande qui lui
/// parlerait irait au vrai, avec les vrais mots de passe du poste : elle doit s'arreter. Dans `moteur` : jamais (c'est le vrai
/// serveur, et c'est son role ; les garde-fous des actes reels tiennent seuls).
#[cfg(feature = "atelier")]
pub fn pas_redirige(cadre: &super::Cadre, organisme: Organisme) -> bool {
    let variable = match organisme {
        Organisme::Dsn => crate::atelier::redirection::VARIABLE_DSN,
        Organisme::Dpae => crate::atelier::redirection::VARIABLE_DPAE,
        Organisme::Google => crate::atelier::redirection::VARIABLE_GOOGLE,
    };
    crate::atelier::redirection::base(variable).is_none() && !crate::compagnie::bridage::branchement(&cadre.racine).joue()
}

#[cfg(not(feature = "atelier"))]
pub fn pas_redirige(_cadre: &super::Cadre, _organisme: Organisme) -> bool {
    false
}

/// Dans `moteur-atelier`, hors compagnie fictive : la boite courriel n'est pas redirigee, donc rien ne la lit ni ne l'ouvre.
#[cfg(feature = "atelier")]
pub fn boite_interdite(cadre: &super::Cadre) -> bool {
    !crate::compagnie::bridage::branchement(&cadre.racine).joue()
}

#[cfg(not(feature = "atelier"))]
pub fn boite_interdite(_cadre: &super::Cadre) -> bool {
    false
}

/// `moteur-atelier` refuse, hors compagnie fictive, ce qui passe par le mot de passe du logiciel (`filet::exiger_mot_de_passe` : poser ou
/// retirer un mot de passe, le changer, oublier Google, activer ou desactiver le filet) : ce sont des gestes sur les secrets du
/// poste, que l'atelier ne touche pas. `quoi` dit ce qui est refuse (« de poser un mot de passe »). Dans `moteur` : rien n'est refuse ici.
#[cfg(feature = "atelier")]
pub fn refuser_dans_l_atelier(cadre: &super::Cadre, quoi: &str) -> Result<(), Echec> {
    if crate::compagnie::bridage::branchement(&cadre.racine).joue() {
        return Ok(());
    }
    Err(Echec {
        classe: Some("ActeReelRefuse"),
        message: format!("Ceci est le binaire de l'atelier (moteur-atelier) : il refuse {} hors compagnie fictive. Rien n'a ete modifie.", quoi),
    })
}

#[cfg(not(feature = "atelier"))]
pub fn refuser_dans_l_atelier(_cadre: &super::Cadre, _quoi: &str) -> Result<(), Echec> {
    Ok(())
}

/// Le refus de l'atelier quand `organisme` n'y est pas redirige (`pas_redirige`) ; `quoi` : ce qui irait au vrai serveur.
#[cfg(feature = "atelier")]
pub fn refuser_si_pas_redirige(cadre: &super::Cadre, organisme: Organisme, quoi: &str) -> Result<(), Echec> {
    if !pas_redirige(cadre, organisme) {
        return Ok(());
    }
    let variable = match organisme {
        Organisme::Dsn => "PUPPETPAIE_ATELIER_DSN",
        Organisme::Dpae => "PUPPETPAIE_ATELIER_DPAE",
        Organisme::Google => "PUPPETPAIE_ATELIER_GOOGLE",
    };
    Err(Echec {
        classe: Some("ActeReelRefuse"),
        message: format!(
            "Ceci est le binaire de l'atelier (moteur-atelier) : {} irait au vrai serveur, que l'atelier ne tient pas. Poser {} sur un faux serveur local. Rien n'a ete modifie.",
            quoi, variable
        ),
    })
}

/// Dans `moteur` : rien n'est redirige, rien n'est refuse ici (le texte de l'atelier n'est pas compile dans le binaire livre).
#[cfg(not(feature = "atelier"))]
pub fn refuser_si_pas_redirige(_cadre: &super::Cadre, _organisme: Organisme, _quoi: &str) -> Result<(), Echec> {
    Ok(())
}

/// Les services du coffre, dans l'ordre alphabetique (`sorted(coffre.SERVICES)`).
pub fn services_connus() -> Vec<&'static str> {
    let mut noms: Vec<&str> = crate::compagnie::coffre::SERVICES.iter().map(|s| s.nom).collect();
    noms.sort_unstable();
    noms
}

/// `datetime.now().isoformat(timespec="minutes")` a l'heure de Paris : ce que le coffre note comme date de pose.
pub fn maintenant_minutes() -> String {
    let t = socle::temps::secondes();
    socle::paris::iso_minutes(t)
}

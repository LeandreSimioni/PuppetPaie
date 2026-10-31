//! Les petits outils des commandes : les classes d'un refus, les garde-fous du binaire de l'atelier, les services du coffre.

use socle::echec::Echec;

/// Les classes d'un refus metier : `ErreurOperation` et ses filles.
const OPERATION: [&str; 17] = [
    "ErreurOperation", "SaisieIncomplete", "PreuveInsuffisante", "FicheRefusee", "CompteursVerrouilles",
    "VersionExigee", "DsnDejaDeposee", "RepetitionManquante", "OrdreTropPetit", "AnnuleRemplaceHorsDelai",
    "MoisNonRegularisable", "ExerciceClos", "SortieAbsente", "BoiteAbsente", "AucunDocument", "ActeReelRefuse",
    "ModificationNonDeclarable",
];

/// Cette classe est-elle une `ErreurOperation` (ou une de ses filles) ?
pub fn est_operation(classe: Option<&str>) -> bool {
    classe.is_some_and(|c| OPERATION.contains(&c))
}

/// Un refus que la commande rend en une phrase : une `ErreurOperation`, ou une `ValueError` (la route rend ainsi ce que
/// la norme refuse -- `SignalementImpossible`, `FctuImpossible`, une date qui ne se lit pas --).
pub fn est_refus(classe: Option<&str>) -> bool {
    est_operation(classe) || classe == Some("ValueError")
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

/// Les services du coffre, dans l'ordre alphabétique.
pub fn services_connus() -> Vec<&'static str> {
    let mut noms: Vec<&str> = crate::compagnie::coffre::SERVICES.iter().map(|s| s.nom).collect();
    noms.sort_unstable();
    noms
}

/// L'heure de Paris, a la minute : ce que le coffre note comme date de pose.
pub fn maintenant_minutes() -> String {
    let t = socle::temps::secondes();
    socle::paris::iso_minutes(t)
}

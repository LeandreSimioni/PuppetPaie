//! LA CARTE D'UN ENGAGEMENT : ce que la personne tape pour un mois, et ce que l'engagement dit de ce mois (sa vue,
//! `stock::Objet::vue`). Une seule forme, declaree ici : la charge d'une route qui ecrit ou calcule une carte, la vue que
//! l'ecran ouvre et que le calcul lit, chacune lue par `Carte::lire` ; une cle inconnue refuse en se nommant.
//!
//! Ce que tous les mois de l'engagement partagent (`stock::PARTAGES`), ses termes (`stock::TERMES`), les faits d'un mois
//! (ses lignes, ses conges, ses primes, ses heures en plus, sa fin : `stock::DE_LA_FIN`), et ce que la vue deduit
//! (`stock::DEDUITS`). Un engagement valide le dit dans la vue (`valide`). Ce que le moteur tient seul (sa validation
//! ecrite, ses versions, ses termes dates : `stock::TENUS_PAR_LE_MOTEUR`) n'est pas d'une carte.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Number, Value};

use std::path::Path;

use socle::num::Resultat;

/// UNE CARTE. Absent et `null` se confondent : rien n'est dit.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Carte {
    // ---- qui, et les bornes du contrat (`stock::PARTAGES`)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub salarie: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spectacle: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nature: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub debut: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fin: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub numero: Option<String>,
    // ---- les termes du contrat (`stock::TERMES`)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub etablissement: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metier: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cadre: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lieu: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub motif_recours: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quotite_contrat: Option<Number>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quotite_reference: Option<Number>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modalite_temps_travail: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub salaire_mensuel: Option<Number>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emploi: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pcs_ese: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code_emploi_ft: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub statut: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub statut_conventionnel: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub statut_categoriel_rc: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub periode_essai_jours: Option<Number>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub personne_remplacee: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repartition_horaire: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clauses_particulieres: Option<String>,
    // ---- les faits du mois
    /// Les lignes de paie de la carte. TRAVAIL A FAIRE (chaine A) : la ligne a sa forme (`calcul::lignes_brut`,
    /// `Engagement::lignes`, `frais_au_dela` la lisent par ses cles).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lignes: Option<Vec<Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conges_pris: Option<Vec<CongePris>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primes: Option<Vec<Prime>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date_paiement: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jours: Option<Number>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub taux_cachet: Option<Number>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heures_supplementaires: Option<Number>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heures_supplementaires_a_50: Option<Number>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heures_complementaires: Option<Number>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub indemnite_licenciement: Option<Number>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub faute_grave: Option<bool>,
    // ---- la fin du contrat (`stock::DE_LA_FIN`)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub motif_rupture: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notification_rupture: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engagement_licenciement: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature_convention: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preavis: Option<Preavis>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transaction: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dernier_jour_paye: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub en_droit_a_la_retraite: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preavis_non_effectue: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preavis_mois: Option<Number>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub precarite_due: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub precarite_pourquoi: Option<String>,
    // ---- ce que la vue deduit (`stock::DEDUITS`)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jours_contrat: Option<Number>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date_modification: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avant: Option<Avant>,
    /// Dans la vue : l'engagement est valide, et depuis quand.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valide: Option<Validee>,
    /// La paie d'un bulletin de l'ancien logiciel, recalculee pour s'y comparer (`ecarts`) : ce qu'il a verse est un fait,
    /// rien ne s'y juge (le jour du versement, les unites du statut).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub importe: Option<bool>,
}

/// Des conges pris dans le mois, d'un jour a l'autre (AAAA-MM-JJ ; la page envoie une ligne commencee telle quelle).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CongePris {
    #[serde(default)]
    pub debut: String,
    #[serde(default)]
    pub fin: String,
}

/// Une prime du mois : son montant, sa rubrique (`prime` sans elle), son libelle.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Prime {
    pub montant: Number,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rubrique: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub libelle: Option<String>,
}

/// Le preavis de la fin du contrat : son genre (`droit::preavis`), et ses jours quand ils sont dits.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Preavis {
    #[serde(rename = "type")]
    pub genre: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub debut: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fin: Option<String>,
}

/// Les termes d'avant l'avenant qui change en cours de mois (`droit::periodes::TERMES_D_AVANT`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Avant {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub salaire_mensuel: Option<Number>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quotite_contrat: Option<Number>,
}

/// Un engagement valide, dans la vue de son mois : le jour de sa validation (`None` : compte comme valide par le calcul
/// qui l'ecrit, `etats::calculer_les_rangs`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Validee {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub le: Option<String>,
}

impl Carte {
    /// LA SEULE LECTURE D'UNE CARTE.
    pub fn lire(v: &Value) -> Resultat<Carte> {
        socle::forme::lire(v, "La carte d'un engagement ne se lit pas.")
    }

    /// La carte telle qu'un fichier d'engagements l'ecrit. TRAVAIL A FAIRE (chaine A) : l'engagement ecrit a sa forme
    /// (`stock::Objet::brut`), et la carte s'y pose sans passer par une `Map`.
    pub fn ecrite(&self) -> Map<String, Value> {
        match serde_json::to_value(self) {
            Ok(Value::Object(m)) => m,
            _ => Map::new(),
        }
    }

    /// TRAVAIL A FAIRE (chaine A) : la carte telle que les lecteurs qui la lisent encore par ses cles la recoivent
    /// (`engagements`, `conges::lignes_du_mois`, `frais_au_dela`, `duree`, `solde`, `maintien`, `imprevus`).
    pub fn en_valeur(&self) -> Value {
        Value::Object(self.ecrite())
    }

    /// Le salarie que la carte nomme, s'il est dit.
    pub fn salarie_dit(&self) -> Option<&str> {
        self.salarie.as_deref().filter(|s| !s.trim().is_empty())
    }

    /// Le numero du contrat, s'il est dit.
    pub fn numero_dit(&self) -> Option<&str> {
        self.numero.as_deref().filter(|s| !s.is_empty())
    }

    /// Les lignes de la carte (aucune si elle n'en dit pas).
    pub fn lignes(&self) -> &[Value] {
        self.lignes.as_deref().unwrap_or(&[])
    }

    /// La fiche que la carte nomme, lue comme le calcul la lit (`Contexte::cle_du_salarie`) ; `None` sans elle.
    pub fn cle(&self, ctx: &droit::contexte::Contexte) -> Option<droit::contexte::IdSalarie> {
        ctx.cle_du_salarie(self.salarie.as_deref()?).ok()
    }

    /// Le debut, comme `Engagement` le deduirait : celui qu'elle dit, sinon son premier jour joue ; vide sans aucun.
    pub fn debut_lu(&self, racine: &Path) -> String {
        crate::debut_de(racine, self.debut.clone(), self.lignes())
    }

    /// Les jours joues de ses lignes de travail (`crate::jours_des_lignes`).
    pub fn dates(&self, racine: &Path) -> Vec<Value> {
        crate::jours_des_lignes(racine, self.lignes(), "dates")
    }

    /// Les jours que ses lignes de travail mettent au planning.
    pub fn planning(&self, racine: &Path) -> Vec<Value> {
        crate::jours_des_lignes(racine, self.lignes(), "planning")
    }

    /// Les jours du contrat entier que la carte connait.
    pub fn jours_du_contrat(&self, racine: &Path) -> usize {
        crate::jours_de_lignes(racine, self.lignes())
    }

    /// La nature de son contrat (la nature par defaut quand elle n'en dit pas).
    pub fn nature_lue(&self) -> Resultat<std::sync::Arc<droit::natures::Nature>> {
        droit::natures::nature(self.nature.as_deref())
    }

    /// Valide, dans la vue de son mois (un engagement que le calcul compte seulement comme valide ne l'est pas).
    pub fn est_validee(&self) -> bool {
        self.valide.as_ref().is_some_and(|v| v.le.as_deref().is_some_and(|l| !l.is_empty()))
    }

    /// Un contrat paye au mois ?
    pub fn mensuelle(&self) -> bool {
        self.nature_lue().is_ok_and(|n| n.mensuelle())
    }
}

/// Ce que la saisie d'un mois pose pour tous ses engagements (le cachet par defaut), tel que la paie ecrite le garde
/// (`etats::MOIS_LU`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CeQueLeMoisDit {
    #[serde(default)]
    pub taux_cachet: Option<Number>,
}

/// LE MOIS DE L'ECRAN (`stock::vue_du_mois`) : le mois, le cachet qu'il dit par defaut, et la vue de chaque engagement qui
/// le touche, dans l'ordre de l'ecran.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VueDuMois {
    pub mois: socle::mois::Mois,
    #[serde(default)]
    pub taux_cachet: Option<Number>,
    #[serde(default)]
    pub engagements: Vec<Carte>,
}

impl VueDuMois {
    /// Un mois sans engagement.
    pub fn vide(mois: socle::mois::Mois) -> VueDuMois {
        VueDuMois { mois, taux_cachet: None, engagements: Vec::new() }
    }

    /// TRAVAIL A FAIRE (chaine A) : la vue telle que les lecteurs qui la lisent encore par ses cles la recoivent
    /// (`engagements`, `imprevus::avertir_les_jours_payes`).
    pub fn en_valeur(&self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Null)
    }
}

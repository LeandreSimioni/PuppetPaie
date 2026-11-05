//! LA CARTE D'UN ENGAGEMENT : ce que la personne tape pour un mois, et ce que l'engagement dit de ce mois (sa vue,
//! `stock::Objet::vue`). Une seule forme, declaree ici : la charge d'une route qui ecrit ou calcule une carte, la vue que
//! l'ecran ouvre et que le calcul lit, chacune lue par `Carte::lire` ; une cle inconnue refuse en se nommant.
//!
//! Chaque champ appartient a un groupe (`Groupe`) : ce que tous les mois de l'engagement partagent, ses termes, les faits
//! d'un mois, sa fin, ce que la vue deduit, ce que la vue dit de lui. Le stock compose et ecrit les cartes par leurs
//! groupes et par leurs champs (`Champ`), jamais par le nom d'une cle.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Number, Value};

use std::path::Path;

use socle::num::Resultat;

/// LES GROUPES DES CHAMPS D'UNE CARTE.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Groupe {
    /// Ce que tous les mois de l'engagement partagent : le salarie, les bornes du contrat, son numero.
    Partage,
    /// LES TERMES DU CONTRAT, qui valent jusqu'a ce qu'ils changent (`stock::Avenant`) : sa nature, son motif, sa duree du
    /// travail, son salaire, son emploi, ce que son contrat ecrit dit en plus (`CHAMPS_DU_CONTRAT_ECRIT`), son
    /// spectacle, son lieu, son etablissement et son metier.
    Terme,
    /// Ce qui n'appartient qu'a un mois : ses lignes (celles d'un contrat paye au mois se composent chaque mois), ses
    /// conges pris, ses primes, ses heures en plus, la date de son paiement.
    Fait,
    /// CE QUE LA FIN DU CONTRAT DIT D'ELLE : son motif, ses dates (notifiee, engagee, signee), son preavis, la
    /// transaction, le dernier jour paye, et ce que le motif appelle. Ce sont les faits du mois ou le contrat finit : la
    /// paie de ce mois-la et le signalement de fin de contrat les lisent (S21.G00.62, S21.G00.63), aucun autre mois.
    Fin,
    /// Ce que la vue d'un mois porte et que le moteur deduit, qui ne s'ecrit pas : les jours du contrat entier, que la DSN
    /// declare chaque mois (S21.G00.40.013) ; la date de l'avenant du mois et les termes d'avant (`stock::Objet::vue`).
    Deduit,
    /// Ce que la vue dit de l'engagement ecrit : qu'il est valide, qu'il est importe.
    Vue,
}

/// LA CARTE ET SES CHAMPS, declares une fois : la structure telle qu'elle s'ecrit, et, de chaque champ marque
/// `#[champ(Groupe, Champ)]`, son nom (`Champ`), son groupe, et ce qui le lit et l'ecrit sans passer par son nom de cle.
macro_rules! carte {
    (
        $(#[$sattr:meta])*
        pub struct $carte:ident {
            $( #[champ($groupe:ident, $variante:ident)] $(#[$attr:meta])* pub $champ:ident : Option<$t:ty>, )*
        }
    ) => {
        $(#[$sattr])*
        pub struct $carte {
            $( $(#[$attr])* pub $champ: Option<$t>, )*
        }

        /// UN CHAMP DE LA CARTE, tel qu'un avenant le retire (`stock::Avenant::retires`) ; il s'ecrit par son nom de cle.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum Champ { $( $variante, )* }

        impl Champ {
            /// Tous les champs, dans l'ordre de la carte.
            pub const TOUS: &'static [Champ] = &[ $( Champ::$variante, )* ];

            /// Son nom de cle.
            pub fn nom(self) -> &'static str {
                match self { $( Champ::$variante => stringify!($champ), )* }
            }

            /// Son groupe.
            pub fn groupe(self) -> Groupe {
                match self { $( Champ::$variante => Groupe::$groupe, )* }
            }
        }

        impl $carte {
            /// Le champ `c` de `de` pose sur la carte, tel quel (dit ou non).
            pub fn prendre(&mut self, de: &$carte, c: Champ) {
                match c { $( Champ::$variante => self.$champ = de.$champ.clone(), )* }
            }

            /// Le champ `c` ote.
            pub fn ote(&mut self, c: Champ) {
                match c { $( Champ::$variante => self.$champ = None, )* }
            }

            /// Le champ `c` est-il dit ?
            pub fn dit(&self, c: Champ) -> bool {
                match c { $( Champ::$variante => self.$champ.is_some(), )* }
            }

            /// Le champ `c` tel qu'il se compare (`socle::ecriture::comparable`).
            pub fn comparable_du(&self, c: Champ) -> String {
                match c { $( Champ::$variante => socle::ecriture::comparable(&self.$champ), )* }
            }

            /// La valeur du champ `c` quand elle est un texte, un nombre ou un oui-non (un terme, une clause) ; `None`
            /// sinon, ou s'il n'est pas dit.
            pub fn valeur_du(&self, c: Champ) -> Option<Valeur> {
                match c { $( Champ::$variante => self.$champ.as_ref().and_then(Simple::simple), )* }
            }
        }
    };
}

carte! {
    /// UNE CARTE. Absent et `null` se confondent : rien n'est dit. Chaque champ dit son groupe et son nom de champ
    /// (`#[champ(Groupe, Champ)]`).
    #[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Carte {
        #[champ(Partage, Salarie)]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub salarie: Option<String>,
        #[champ(Terme, Spectacle)]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub spectacle: Option<String>,
        #[champ(Terme, Nature)]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub nature: Option<String>,
        #[champ(Partage, Debut)]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub debut: Option<String>,
        #[champ(Partage, Fin)]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub fin: Option<String>,
        #[champ(Partage, Numero)]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub numero: Option<String>,
        #[champ(Terme, Etablissement)]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub etablissement: Option<String>,
        #[champ(Terme, Metier)]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub metier: Option<String>,
        #[champ(Terme, Cadre)]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub cadre: Option<bool>,
        #[champ(Terme, Lieu)]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub lieu: Option<String>,
        #[champ(Terme, MotifRecours)]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub motif_recours: Option<String>,
        #[champ(Terme, QuotiteContrat)]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub quotite_contrat: Option<Number>,
        #[champ(Terme, QuotiteReference)]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub quotite_reference: Option<Number>,
        #[champ(Terme, ModaliteTempsTravail)]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub modalite_temps_travail: Option<String>,
        #[champ(Terme, SalaireMensuel)]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub salaire_mensuel: Option<Number>,
        #[champ(Terme, Emploi)]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub emploi: Option<String>,
        #[champ(Terme, PcsEse)]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub pcs_ese: Option<String>,
        #[champ(Terme, CodeEmploiFt)]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub code_emploi_ft: Option<String>,
        #[champ(Terme, Statut)]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub statut: Option<String>,
        #[champ(Terme, StatutConventionnel)]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub statut_conventionnel: Option<String>,
        #[champ(Terme, StatutCategorielRc)]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub statut_categoriel_rc: Option<String>,
        #[champ(Terme, PeriodeEssaiJours)]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub periode_essai_jours: Option<Number>,
        #[champ(Terme, PersonneRemplacee)]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub personne_remplacee: Option<String>,
        #[champ(Terme, RepartitionHoraire)]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub repartition_horaire: Option<String>,
        #[champ(Terme, ClausesParticulieres)]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub clauses_particulieres: Option<String>,
        #[champ(Fait, Lignes)]
        /// Les lignes de paie de la carte. TRAVAIL A FAIRE (chaine A) : la ligne a sa forme (`calcul::lignes_brut`,
        /// `Engagement::lignes`, `frais_au_dela`, `conges::lignes_du_mois` la lisent par ses cles).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub lignes: Option<Vec<Value>>,
        #[champ(Fait, CongesPris)]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub conges_pris: Option<Vec<CongePris>>,
        #[champ(Fait, Primes)]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub primes: Option<Vec<Prime>>,
        #[champ(Fait, DatePaiement)]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub date_paiement: Option<String>,
        #[champ(Fait, Jours)]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub jours: Option<Number>,
        #[champ(Fait, TauxCachet)]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub taux_cachet: Option<Number>,
        #[champ(Fait, HeuresSupplementaires)]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub heures_supplementaires: Option<Number>,
        #[champ(Fait, HeuresSupplementairesA50)]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub heures_supplementaires_a_50: Option<Number>,
        #[champ(Fait, HeuresComplementaires)]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub heures_complementaires: Option<Number>,
        #[champ(Fait, IndemniteLicenciement)]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub indemnite_licenciement: Option<Number>,
        #[champ(Fait, FauteGrave)]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub faute_grave: Option<bool>,
        #[champ(Fin, MotifRupture)]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub motif_rupture: Option<String>,
        #[champ(Fin, NotificationRupture)]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub notification_rupture: Option<String>,
        #[champ(Fin, EngagementLicenciement)]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub engagement_licenciement: Option<String>,
        #[champ(Fin, SignatureConvention)]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub signature_convention: Option<String>,
        #[champ(Fin, Preavis)]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub preavis: Option<Preavis>,
        #[champ(Fin, Transaction)]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub transaction: Option<String>,
        #[champ(Fin, DernierJourPaye)]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub dernier_jour_paye: Option<String>,
        #[champ(Fin, EnDroitALaRetraite)]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub en_droit_a_la_retraite: Option<bool>,
        #[champ(Fin, PreavisNonEffectue)]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub preavis_non_effectue: Option<bool>,
        #[champ(Fin, PreavisMois)]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub preavis_mois: Option<Number>,
        #[champ(Fin, PrecariteDue)]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub precarite_due: Option<bool>,
        #[champ(Fin, PrecaritePourquoi)]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub precarite_pourquoi: Option<String>,
        #[champ(Deduit, JoursContrat)]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub jours_contrat: Option<Number>,
        #[champ(Deduit, DateModification)]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub date_modification: Option<String>,
        #[champ(Deduit, Avant)]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub avant: Option<Avant>,
        #[champ(Vue, Valide)]
        /// Dans la vue : l'engagement est valide, et depuis quand.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub valide: Option<Validee>,
        #[champ(Vue, Importe)]
        /// La paie d'un bulletin de l'ancien logiciel, recalculee pour s'y comparer (`ecarts`) : ce qu'il a verse est un fait,
        /// rien ne s'y juge (le jour du versement, les unites du statut).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub importe: Option<bool>,
    }
}

impl Champ {
    /// Les champs des groupes `g`, dans l'ordre de la carte.
    pub fn des(g: &[Groupe]) -> impl Iterator<Item = Champ> + '_ {
        Champ::TOUS.iter().copied().filter(move |c| g.contains(&c.groupe()))
    }
}

/// LA VALEUR SIMPLE D'UN CHAMP (`Carte::valeur_du`) : un texte, un nombre, un oui-non ; elle s'ecrit telle quelle.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum Valeur {
    Texte(String),
    Nombre(Number),
    Oui(bool),
}

/// Ce qu'un champ a de simple (`Valeur`).
trait Simple {
    fn simple(&self) -> Option<Valeur>;
}

impl Simple for String {
    fn simple(&self) -> Option<Valeur> {
        Some(Valeur::Texte(self.clone()))
    }
}

impl Simple for Number {
    fn simple(&self) -> Option<Valeur> {
        Some(Valeur::Nombre(self.clone()))
    }
}

impl Simple for bool {
    fn simple(&self) -> Option<Valeur> {
        Some(Valeur::Oui(*self))
    }
}

macro_rules! rien_de_simple {
    ($($t:ty),*) => { $( impl Simple for $t { fn simple(&self) -> Option<Valeur> { None } } )* };
}
rien_de_simple!(Vec<Value>, Vec<CongePris>, Vec<Prime>, Preavis, Avant, Validee);

impl Serialize for Champ {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.nom())
    }
}

impl<'de> Deserialize<'de> for Champ {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Champ, D::Error> {
        let nom = String::deserialize(d)?;
        Champ::TOUS.iter().copied().find(|c| c.nom() == nom).ok_or_else(|| serde::de::Error::custom(format!("« {} » n'est pas un champ d'une carte", nom)))
    }
}

impl Carte {
    /// LA SEULE LECTURE D'UNE CARTE.
    pub fn lire(v: &Value) -> Resultat<Carte> {
        socle::forme::lire(v, "La carte d'un engagement ne se lit pas.")
    }

    /// TRAVAIL A FAIRE (chaine A) : la carte telle que les lecteurs qui la lisent encore par ses cles la recoivent
    /// (`conges::lignes_du_mois`, `solde`, `maintien`, `imprevus`, `duree`, `heures_en_plus`, `calcul::saisie_declaree_a_la_caisse_des_conges`).
    pub fn en_valeur(&self) -> Value {
        match serde_json::to_value(self) {
            Ok(Value::Object(m)) => Value::Object(m),
            _ => Value::Object(Map::new()),
        }
    }

    /// La carte des seuls champs des groupes `g`.
    pub fn du_groupe(&self, g: &[Groupe]) -> Carte {
        let mut c = Carte::default();
        for x in Champ::des(g) {
            c.prendre(self, x);
        }
        c
    }

    /// La carte sans les champs des groupes `g`.
    pub fn sans(&self, g: &[Groupe]) -> Carte {
        let mut c = self.clone();
        for x in Champ::des(g) {
            c.ote(x);
        }
        c
    }

    /// Chaque champ que `de` dit se pose sur la carte ; les autres restent ce qu'ils sont.
    pub fn recouvrir(&mut self, de: &Carte) {
        for x in Champ::TOUS.iter().copied().filter(|x| de.dit(*x)) {
            self.prendre(de, x);
        }
    }

    /// Une carte qui ne dit rien (une liste vide ne dit rien : `socle::ecriture::comparable`) ?
    pub fn muette(&self) -> bool {
        socle::ecriture::comparable(self) == socle::ecriture::comparable(&Carte::default())
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

    /// Le texte d'un champ texte dit et non vide ; vide sinon.
    pub fn texte(x: &Option<String>) -> String {
        x.clone().filter(|t| !t.is_empty()).unwrap_or_default()
    }
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

//! Les pieces de `calculer` : les
//! lignes de remuneration et leur designation, les structs de sortie
//! (`LigneCotisation`, `Paie`), la reduction generale, le seuil de SMIC et la
//! deduction forfaitaire, puis `calculer` lui-meme (§7).

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex, OnceLock};


use droit::bareme::{bareme_applique, SANTE_SUSPENSION, SANTE_SUSPENSION_MAINTENUE, SANTE_SUSPENSION_SUSPENDUE};
use droit::natures::Nature;
use droit::fiche::{AccordDfs, Case, ChampsEntreprise, ChampsSalarie, FicheSalarie, OptionDfs as DfsEmployeur, SalarieDuContrat, Statut};
use droit::catalogue::{code_54, rubrique, HorsAssiette};
use droit::contexte::Contexte;
use socle::date::Jour;
use socle::texte::euros;
use droit::emplois::{emploi_de, EmploiEngagement};
use crate::{Avis, Engagement};
use crate::paie_ecrite::{Cle, FormeDeCotisation, FormeDeLaDeduction, FormeDePaie, FormeDeRemuneration};
use socle::num::{add, dec_f64, div, eur, mul, pct, round3, sub, Decimal, Resultat};
pub use droit::reduction::ReductionGenerale;
use droit::bareme::somme_pct;
use droit::forme_bareme::{Pile, Rubrique};
use droit::reduction::{au_dela_du_seuil, reduction_generale, reduction_generale_sous_dfs};
use droit::plafond::{plafond_permanent, pour_paie, EngagementPlafond};



/// Le flottant d'un decimal, correctement arrondi, via le texte.
fn en_f64(x: Decimal) -> f64 {
    crate::conges::flot(x)
}

// ------------------------------------------------------ les lignes de brut

/// Une ligne de remuneration : une rubrique, une quantite, un taux.
#[derive(Debug, Clone, PartialEq)]
pub struct LigneBrut {
    pub code: String,
    pub libelle: String,
    pub unite: String,
    pub quantite: Decimal,
    pub taux: Decimal,
    pub montant: Decimal,
    pub heures_annexes: Decimal,
    pub soumis: bool,
    pub imposable: bool,
    pub conges_spectacles: bool,
    pub salaire_de_base: bool,
    pub retabli: bool,
    pub conges_pris: bool,
    pub dsn_bloc52: Option<String>,
    pub arret_de_travail: bool,
    pub dsn_autres_bruts: Option<String>,
    /// Ce que la ligne dit d'elle-meme sur le bulletin quand sa quantite ne suffit pas : la part payee d'un mois incomplet
    /// (`droit::periodes::PartDuMois::sur_le_bulletin`).
    pub precision: Option<String>,
}

impl LigneBrut {
    /// La ligne en francais, telle qu'une personne la lit : « 2 Cachets », « 1 Journée de travail », « 6 h de
    /// masterclass / action culturelle », « 154 h d'absence non rémunérée », « Indemnité compensatrice de congés payés
    /// (4,92 jours) », « Indemnité de fin de contrat (précarité) ». Un nombre se dit au centieme. La quantite se compte
    /// devant le libelle quand son premier mot nomme l'unite (un cachet, une journee, un service, une semaine) ; sinon
    /// elle se dit apres lui, dans son unite. Un forfait et un pourcentage se disent par leur seul libelle : leur
    /// quantite n'est pas un nombre de choses.
    pub fn designation(&self) -> String {
        let q = self.quantite.round_dp(2).normalize();
        let nb = q.to_string().replace('.', ",");
        if matches!(self.unite.as_str(), "forfait" | "pourcentage") {
            return self.libelle.clone();
        }
        // UN SALAIRE MENSUEL se paie au mois : sa quantite est la fraction du mois payee (`rubriques-base.json`, `mois`).
        // Un mois entier se dit seul ; une fraction se dit en part du mois, pas en decimales.
        if self.unite == "mois" {
            if self.quantite == Decimal::ONE {
                return self.libelle.clone();
            }
            if let Some(p) = &self.precision {
                return format!("{} ({})", self.libelle, p);
            }
            let part = (self.quantite * Decimal::from(100)).round_dp(2).normalize().to_string().replace('.', ",");
            return format!("{} ({} % du mois)", self.libelle, part);
        }
        let (tete, reste) = match self.libelle.split_once(' ') {
            Some((t, r)) => (t, Some(r)),
            None => (self.libelle.as_str(), None),
        };
        let compte = match self.unite.as_str() {
            "heure" => ["heure"].as_slice(),
            "jour" => ["jour", "journée"].as_slice(),
            "cachet" => ["cachet"].as_slice(),
            "service" => ["service"].as_slice(),
            "semaine" => ["semaine"].as_slice(),
            _ => [].as_slice(),
        };
        // Des heures qui se nomment au pluriel (« Heures supplémentaires ») : leur nombre, et ce qui les distingue
        // (leur majoration), entre parentheses.
        if self.unite == "heure" && tete.to_lowercase() == "heures" {
            return match &self.precision {
                Some(p) => format!("{} ({} h, {})", self.libelle, nb, p),
                None => format!("{} ({} h)", self.libelle, nb),
            };
        }
        // Au singulier jusqu'a deux exclus (« 1,5 journée »).
        let pluriel = q >= Decimal::TWO;
        if !compte.contains(&tete.to_lowercase().as_str()) {
            if self.unite == "heure" {
                let mut l = self.libelle.chars();
                let minuscule: String = l.next().into_iter().flat_map(|c| c.to_lowercase()).chain(l).collect();
                return format!("{} h {}", nb, socle::texte::de(&minuscule));
            }
            return format!("{} ({} {}{})", self.libelle, nb, self.unite, if pluriel { "s" } else { "" });
        }
        // Le pluriel porte sur le premier mot : il nomme ce qui se compte.
        let tete = if pluriel && !tete.ends_with('s') { format!("{}s", tete) } else { tete.to_string() };
        match reste {
            Some(r) => format!("{} {} {}", nb, tete, r),
            None => format!("{} {}", nb, tete),
        }
    }

    /// Sa forme ecrite, avec sa designation au bulletin.
    pub fn forme(&self) -> FormeDeRemuneration {
        FormeDeRemuneration {
            code: self.code.clone(),
            libelle: self.libelle.clone(),
            unite: self.unite.clone(),
            quantite: self.quantite,
            taux: self.taux,
            montant: self.montant,
            heures_annexes: self.heures_annexes,
            soumis: self.soumis,
            imposable: self.imposable,
            conges_spectacles: self.conges_spectacles,
            salaire_de_base: self.salaire_de_base,
            retabli: self.retabli,
            conges_pris: self.conges_pris,
            dsn_bloc52: self.dsn_bloc52.clone(),
            arret_de_travail: self.arret_de_travail,
            dsn_autres_bruts: self.dsn_autres_bruts.clone(),
            precision: self.precision.clone(),
            designation: self.designation(),
        }
    }

    /// Relue de sa forme ecrite.
    pub(crate) fn de_la_forme(f: &FormeDeRemuneration) -> LigneBrut {
        LigneBrut {
            code: f.code.clone(),
            libelle: f.libelle.clone(),
            unite: f.unite.clone(),
            quantite: f.quantite,
            taux: f.taux,
            montant: f.montant,
            heures_annexes: f.heures_annexes,
            soumis: f.soumis,
            imposable: f.imposable,
            conges_spectacles: f.conges_spectacles,
            salaire_de_base: f.salaire_de_base,
            retabli: f.retabli,
            conges_pris: f.conges_pris,
            dsn_bloc52: f.dsn_bloc52.clone(),
            arret_de_travail: f.arret_de_travail,
            dsn_autres_bruts: f.dsn_autres_bruts.clone(),
            precision: f.precision.clone(),
        }
    }
}

/// Les lignes de remuneration d'un engagement : une par ligne de paie, le
/// montant arrondi ligne par ligne.
pub(crate) fn lignes_brut(eng: &Engagement, ctx: &Contexte) -> Resultat<Vec<LigneBrut>> {
    let mut out = Vec::new();
    for l in &eng.lignes {
        let requis = |x: Option<Decimal>, cle: &str| x.ok_or_else(|| socle::fichiers::absent(cle));
        let r = rubrique(&ctx.racine, l.rubrique(), &ctx.lue_au)?;
        let q = requis(l.quantite(), "quantite")?;
        let taux = requis(l.taux(), "taux")?;
        let heures = l.heures_lues();
        let signe = if r.sens == "-" { -Decimal::ONE } else { Decimal::ONE };
        // Le signe porte aussi sur les heures : une absence en retire.
        let h = dec_f64(r.heures(en_f64(q), heures))?;
        let dsn_autres_bruts = code_54(&r, l.genre())?;
        out.push(LigneBrut {
            code: r.code,
            libelle: r.libelle,
            unite: r.unite,
            quantite: q,
            taux,
            montant: signe * eur(mul(taux, q)),
            // Des heures nulles gardent le signe de la ligne et leur echelle.
            heures_annexes: if h.is_zero() { if signe.is_sign_negative() { -h } else { h } } else { signe * h },
            soumis: r.soumis,
            imposable: r.imposable,
            conges_spectacles: r.conges_spectacles,
            salaire_de_base: r.salaire_de_base,
            retabli: r.retabli,
            conges_pris: r.conges_pris,
            dsn_bloc52: r.dsn_bloc52,
            arret_de_travail: r.arret_de_travail,
            dsn_autres_bruts,
            precision: l.precision().map(String::from),
        });
    }
    Ok(out)
}

// ----------------------------------------------------------------- sortie

#[derive(Debug, Clone, PartialEq)]
pub struct LigneCotisation {
    pub id: String,
    pub famille: String,
    pub libelle: String,
    pub base: Decimal,
    pub taux_pat: Option<f64>,
    pub montant_pat: Decimal,
    pub taux_sal: Option<f64>,
    pub montant_sal: Decimal,
    pub masquer_base_et_taux: bool,
}

impl LigneCotisation {
    /// Sa forme ecrite : un taux en decimal par sa forme la plus courte.
    pub fn forme(&self) -> Resultat<FormeDeCotisation> {
        let taux = |t: Option<f64>| t.map(dec_f64).transpose();
        Ok(FormeDeCotisation {
            id: self.id.clone(),
            famille: self.famille.clone(),
            libelle: self.libelle.clone(),
            base: self.base,
            taux_pat: taux(self.taux_pat)?,
            montant_pat: self.montant_pat,
            taux_sal: taux(self.taux_sal)?,
            montant_sal: self.montant_sal,
            masquer_base_et_taux: self.masquer_base_et_taux,
        })
    }

    /// Relue de sa forme ecrite.
    pub(crate) fn de_la_forme(f: &FormeDeCotisation) -> LigneCotisation {
        LigneCotisation {
            id: f.id.clone(),
            famille: f.famille.clone(),
            libelle: f.libelle.clone(),
            base: f.base,
            taux_pat: f.taux_pat.map(en_f64),
            montant_pat: f.montant_pat,
            taux_sal: f.taux_sal.map(en_f64),
            montant_sal: f.montant_sal,
            masquer_base_et_taux: f.masquer_base_et_taux,
        }
    }
}

/// La deduction forfaitaire specifique (DFS) appliquee a un contrat : arrete du 4 septembre 2025
/// relatif aux frais professionnels, art. 9 (`atelier/reference/sources/dfs/LISEZ-MOI.md`).
#[derive(Debug, Clone, PartialEq)]
pub struct DeductionForfaitaire {
    /// « 20 » ou « 25 » : le taux de l'article 5 de l'annexe IV du CGI (redaction du 31/12/2000) que la profession porte.
    pub categorie: String,
    /// La profession de la fiche (`deduction_forfaitaire.professions` du bareme).
    pub profession: String,
    /// Le taux de l'annee (bareme), en pourcent.
    pub taux: Decimal,
    /// Les lignes du bulletin dont l'assiette est abattue.
    pub rubriques: Vec<String>,
    /// La deduction retenue sur ce contrat : le brut moins l'assiette abattue, apres le plafond annuel et l'assiette
    /// minimum. C'est ce que la DSN declare en bloc 54 de type 10.
    pub montant: Decimal,
    /// Ce que l'annee civile a deja deduit pour ce salarie avant ce contrat (les mois anterieurs, les contrats
    /// precedents du mois).
    pub deja: Decimal,
}

impl DeductionForfaitaire {
    /// Sa forme ecrite.
    pub fn forme(&self) -> FormeDeLaDeduction {
        FormeDeLaDeduction {
            categorie: self.categorie.clone(),
            profession: self.profession.clone(),
            taux: self.taux,
            rubriques: self.rubriques.clone(),
            montant: self.montant,
            deja: self.deja,
        }
    }

    /// Relue de sa forme ecrite.
    pub(crate) fn de_la_forme(f: &FormeDeLaDeduction) -> DeductionForfaitaire {
        DeductionForfaitaire {
            categorie: f.categorie.clone(),
            profession: f.profession.clone(),
            taux: f.taux,
            rubriques: f.rubriques.clone(),
            montant: f.montant,
            deja: f.deja,
        }
    }
}

/// Ce que la remuneration nette fiscale reintegre au net des cotisations : la CSG et la CRDS non deductibles (part
/// salariale de `csg_crds_nd`) et la part patronale des garanties frais de sante (`mutuelle`) ; norme DSN 2026.1,
/// p. 60-61 (« AJOUTER ») et p. 244 (50.004). Un seul endroit : `calculer` en tire 50.002, `Paie::net_verse` 50.004.
fn reintegrations(lignes: &[LigneCotisation]) -> Decimal {
    let zero = Decimal::new(0, 2);
    let csg_nd = lignes.iter().filter(|l| l.id == "csg_crds_nd").take(1).fold(zero, |a, l| a + l.montant_sal);
    csg_nd + lignes.iter().filter(|l| l.id == "mutuelle").fold(zero, |a, l| a + l.montant_pat)
}

/// Une paie d'un engagement : ce que son calcul a produit, ou sa forme ecrite relue (`relire`). Elle se lit partout, ce
/// qu'elle porte a travers `Contenu` ; elle ne se construit et ne s'ecrit que dans cette crate, qui seule calcule.
#[derive(Debug, Clone)]
pub struct Paie(Contenu);

impl std::ops::Deref for Paie {
    type Target = Contenu;
    fn deref(&self) -> &Contenu {
        &self.0
    }
}

/// Ce qu'une paie porte. Se lit a travers `Paie` ; un `Contenu` construit hors de cette crate ne devient jamais une paie.
#[derive(Debug, Clone)]
pub struct Contenu {
    pub engagement: Engagement,
    pub salarie: SalarieDuContrat,
    /// La fiche du spectacle telle que le calcul l'a lue ; `None` pour un contrat paye au mois.
    pub spectacle: Option<droit::fiche::Spectacle>,
    pub brut: Decimal,
    pub base_csg: Decimal,
    pub plafond: Decimal,
    pub smic: f64,
    pub heures: Decimal,
    pub lignes: Vec<LigneCotisation>,
    pub total_pat: Decimal,
    pub total_sal: Decimal,
    /// Ce qui se paie au salarie avant impot : le brut, moins les cotisations salariales, plus ou moins TOUTES les
    /// lignes hors assiette (defraiements, acompte retenu, indemnites).
    pub net_a_payer: Decimal,
    /// Le montant net social (S21.G00.58.004 de type 03, « Montant net social » du bulletin) : voir `calculer`
    /// (7.10). Le brut, moins les cotisations salariales, plus les lignes hors assiette qui sont un revenu ; ni
    /// remboursement de frais, ni mouvement de tresorerie (acompte).
    pub net_social: Decimal,
    /// La remuneration nette fiscale (S21.G00.50.002, « Net imposable » du bulletin, base du PAS) : voir
    /// `calculer` (7.10) ; les lignes hors assiette n'y entrent que si le catalogue les dit imposables.
    pub net_imposable: Decimal,
    pub cout_employeur: Decimal,
    pub numero_bulletin: Option<String>,
    /// Les assiettes du calcul, que la DSN declare telles quelles.
    pub bases: BTreeMap<String, Decimal>,
    /// Les lignes de remuneration (le haut du bulletin), pas les cotisations.
    pub remuneration: Vec<LigneBrut>,
    pub reduction_generale: Option<ReductionGenerale>,
    pub deduction_forfaitaire: Option<DeductionForfaitaire>,
    pub avertissements: Vec<Avis>,
    pub salaire_de_base: Decimal,
    pub salaire_retabli: Decimal,
    pub assiette_chomage: Decimal,
    pub heures_travaillees: Decimal,
    pub heures_absence: Decimal,
    /// Les periodes elementaires de paie d'un mois decoupe (`periodes`), chacune une paie calculee sur ses
    /// bornes, sa fiche et sa pile ; vide pour un mois d'un seul tenant. La paie du mois en est la somme ; la DSN declare
    /// leurs remunerations (51) et leurs bases (78, 79, 81) une par une.
    pub periodes: Vec<Paie>,
    /// Le prelevement a la source de la paie, tel que la validation l'a calcule au taux qu'elle avait (`etats::valider`) ;
    /// `None` pour une paie que rien n'a encore validee.
    pub pas: Option<droit::pas::Versement>,
    /// Le compteur de conges payes que le bulletin montre (`conges::compteur_au_bulletin`), tel que la validation l'a
    /// ecrit ; `None` pour une paie que rien n'a encore validee, et pour un contrat sans compteur de l'employeur.
    pub conges: Option<crate::conges::CompteurConges>,
    /// Le compteur de conges avant le mois (`conges::Avant`), que la validation ecrit avec le compteur du bulletin et que
    /// le mois suivant reprend ; `None` sans compteur.
    pub conges_avant: Option<crate::conges::Avant>,
    /// Le plafond d'un salarie a temps partiel (`plafond_temps_partiel`), tel que le calcul l'a applique : ce que la DSN
    /// declare en S21.G00.40.084.
    pub plafond_temps_partiel: PlafondTempsPartiel,
    /// LA PAIE D'UN ENGAGEMENT IMPORTE (`importe::Importe::paie`) : ce que les pieces de l'ancien logiciel en disent, rien
    /// de calcule ; `None` pour une paie calculee ici.
    pub importee: Option<Importee>,
}

/// Ce qu'une paie importee dit, et ce qu'elle tait.
#[derive(Debug, Clone, PartialEq)]
pub struct Importee {
    /// Le jour de l'import.
    pub le: String,
    /// Les montants que les pieces ne disent pas (`brut`, `cotisations_salariales`, `net_a_payer`, `plafond`...) : ils
    /// valent zero dans la paie, et un etat les laisse vides.
    pub non_dits: Vec<&'static str>,
}

impl Paie {
    /// Le montant `champ` de la paie est-il dit ? Toujours pour une paie calculee ; pour une paie importee, quand ses
    /// pieces le disent.
    pub fn dit(&self, champ: &str) -> bool {
        self.importee.as_ref().is_none_or(|i| !i.non_dits.contains(&champ))
    }
}

impl Paie {
    /// Les champs sans valeur par defaut ; les autres partent a zero ou vides.
    pub(crate) fn nouveau(
        engagement: Engagement, salarie: SalarieDuContrat, spectacle: Option<droit::fiche::Spectacle>, brut: Decimal,
        base_csg: Decimal, plafond: Decimal, smic: f64, heures: Decimal,
        plafond_temps_partiel: PlafondTempsPartiel,
    ) -> Paie {
        let zero = Decimal::new(0, 2);
        Paie(Contenu {
            engagement, salarie, spectacle, brut, base_csg, plafond, smic, heures,
            lignes: Vec::new(), total_pat: zero, total_sal: zero, net_a_payer: zero, net_social: zero,
            net_imposable: zero, cout_employeur: zero, numero_bulletin: None,
            bases: BTreeMap::new(), remuneration: Vec::new(), reduction_generale: None,
            deduction_forfaitaire: None, avertissements: Vec::new(), salaire_de_base: zero,
            salaire_retabli: zero, assiette_chomage: zero, heures_travaillees: zero,
            heures_absence: zero, periodes: Vec::new(), pas: None, conges: None, conges_avant: None, plafond_temps_partiel, importee: None,
        })
    }

    /// LA MEME PAIE, ANNULEE : son contrat, sa fiche et son spectacle, sans aucune ligne, aucune activite ni aucun
    /// montant. Ce qu'est la paie d'un engagement qui n'est plus actif (`etats::NonActif`) : une correction qui la porte
    /// annule tout ce qui a ete declare, et ne declare rien a la place.
    pub fn annulee(&self) -> Paie {
        let mut engagement = self.engagement.clone();
        engagement.lignes.clear();
        (engagement.jours, engagement.cachets, engagement.heures) = (0, 0, None);
        let zero = Decimal::new(0, 2);
        let mut p = Paie::nouveau(engagement, self.salarie.clone(), self.spectacle.clone(), zero, zero, zero, self.smic, zero, self.plafond_temps_partiel);
        p.0.bases = self.bases.keys().map(|k| (k.clone(), zero)).collect();
        p
    }

    /// Ce que la paie porte, a ecrire : le calcul, la relecture, la validation.
    pub(crate) fn ecrire(&mut self) -> &mut Contenu {
        &mut self.0
    }

    /// S21.G00.50.004, le montant net verse : « la remuneration nette fiscale (S21.G00.50.002), de laquelle sont
    /// deduits le montant de la CSG non deductible, le montant de la CRDS et le montant des contributions des employeurs
    /// destinees a financer des garanties « frais de sante » reintegrees dans la base fiscale » (norme DSN 2026.1,
    /// p. 244). Ce n'est PAS le net a payer : ni defraiement, ni acompte, ni indemnite non imposable n'y entrent.
    pub fn net_verse(&self) -> Decimal {
        self.net_imposable - reintegrations(&self.lignes)
    }

    /// Le contrat de cette paie est-il declare a la caisse des Conges Spectacles (`caisse_conges::affilie`, D. 7121-41).
    pub fn declaree_a_la_caisse_des_conges(&self) -> Resultat<bool> {
        declaree_a_la_caisse_des_conges(self)
    }

    /// Les lignes saisies de cette paie, une par ligne de remuneration (`minima::saisies_de`).
    #[cfg(feature = "ecran")]
    pub fn saisies(&self) -> Vec<Option<&crate::LignePaie>> {
        crate::minima::saisies_de(self)
    }

    /// Le minimum conventionnel de la ligne `i` de la remuneration (`minima::minimum_de_la_ligne`).
    #[cfg(feature = "ecran")]
    pub fn minimum_de_la_ligne(&self, ctx: &Contexte, i: usize) -> Resultat<Option<droit::minima::Minimum>> {
        crate::minima::minimum_de_la_ligne(ctx, self, i)
    }

    /// Le cachet de repetition de la convention pour ce metier, s'il en a un (`minima::cachet_de_repetition`).
    #[cfg(feature = "ecran")]
    pub fn cachet_de_repetition(&self, ctx: &Contexte) -> Resultat<Option<droit::minima::Minimum>> {
        crate::minima::cachet_de_repetition(ctx, self)
    }

    /// Le titre du spectacle, ou pour un permanent le libelle de sa nature de contrat.
    pub fn titre(&self) -> Resultat<String> {
        match &self.spectacle {
            Some(s) => s.titre.portee().map(str::to_string).ok_or_else(|| socle::fichiers::absent("titre")),
            None => Ok(self.engagement.nat()?.libelle.clone()),
        }
    }

    /// Les paies que la DSN declare une par une : les periodes elementaires d'un mois decoupe, sinon la paie elle-meme.
    pub fn elementaires(&self) -> Vec<&Paie> {
        if self.periodes.is_empty() {
            vec![self]
        } else {
            self.periodes.iter().collect()
        }
    }

    /// La paie est-elle decoupee au jour `j` (une periode elementaire commence ce jour-la, apres le debut du mois) ?
    pub fn decoupee_le(&self, j: &Jour) -> Resultat<bool> {
        for p in self.periodes.iter().skip(1) {
            if p.engagement.debut_periode()? == *j {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub fn ligne(&self, id: &str) -> Option<&LigneCotisation> {
        self.lignes.iter().find(|l| l.id == id)
    }

    /// Ce que la paie paye, en une ligne, la plus courte qu'une personne reconnaisse (`en_bref`).
    pub fn paye_en_bref(&self) -> String {
        en_bref(&self.remuneration)
    }
}

/// Des lignes de remuneration en une ligne, la plus courte qu'une personne reconnaisse : « 4 cachets, 8 repas,
/// 4 nuitées », « 28 h de travail », « salaire mensuel de base, 119 h d'absence maladie, maintien de salaire,
/// indemnités journalières (22 jours) ». Le detail ligne a ligne est celui du bulletin (`LigneBrut::designation`).
/// - Les lignes d'une meme rubrique s'additionnent.
/// - La part d'un repas ou d'une nuitee au-dela de sa limite (`REPAS_AU_DELA`, `NUITEE_AU_DELA`) est celle d'une ligne deja comptee.
/// - Deux rubriques qui ne different que par leur precision entre parentheses, sur la meme mesure, se disent une fois :
///   les indemnites journalieres deduites puis versees.
/// - Ce qui rend, juste apres elle, ce qu'une deduction a retire (la meme mesure) se dit sans la repeter : le maintien
///   de salaire d'une absence, l'indemnite des conges pris.
pub fn en_bref(lignes: &[LigneBrut]) -> String {
    let mut groupes: Vec<LigneBrut> = Vec::new();
    for l in lignes.iter().filter(|l| l.code != crate::NUITEE_AU_DELA && l.code != crate::REPAS_AU_DELA) {
        match groupes.iter_mut().find(|g| g.code == l.code && g.precision == l.precision) {
            Some(g) => {
                g.quantite += l.quantite;
                g.montant += l.montant;
            }
            None => groupes.push(l.clone()),
        }
    }
    let base = |l: &LigneBrut| l.libelle.split_once(" (").map(|(b, _)| b.to_string());
    let meme_mesure = |a: &LigneBrut, b: &LigneBrut| a.unite == b.unite && a.quantite == b.quantite;
    let mut dits = Vec::new();
    let mut i = 0;
    while i < groupes.len() {
        let g = &groupes[i];
        let jumelle = groupes.get(i + 1).filter(|s| meme_mesure(g, s) && base(g).is_some() && base(g) == base(s));
        if let (Some(_), Some(commune)) = (jumelle, base(g)) {
            let mut une = g.clone();
            une.libelle = commune;
            une.precision = None;
            dits.push(bref(&une));
            i += 2;
            continue;
        }
        let rend = i > 0 && groupes[i - 1].montant.is_sign_negative() && g.montant.is_sign_positive() && meme_mesure(&groupes[i - 1], g);
        dits.push(if rend { g.libelle.to_lowercase() } else { bref(g) });
        i += 1;
    }
    dits.join(", ")
}

/// Une ligne (deja additionnee) dite en bref : un forfait qui se compte dit son nombre (« 8 repas »), des heures de
/// travail se disent en heures (« 28 h de travail ») ; le reste comme au bulletin.
fn bref(l: &LigneBrut) -> String {
    let q = l.quantite.round_dp(2).normalize();
    let (tete, reste) = match l.libelle.split_once(' ') {
        Some((t, r)) => (t, r),
        None => (l.libelle.as_str(), ""),
    };
    let reste = if reste.is_empty() { String::new() } else { format!(" {}", reste) };
    let dit = if l.unite == "forfait" && q.fract().is_zero() && q >= Decimal::TWO {
        let tete = if tete.ends_with('s') || tete.ends_with('x') { tete.to_string() } else { format!("{}s", tete) };
        format!("{} {}{}", q, tete, reste)
    } else if l.unite == "heure" && tete.to_lowercase() == "heure" {
        format!("{} h{}", q.to_string().replace('.', ","), reste)
    } else {
        l.designation()
    };
    dit.to_lowercase()
}

// ------------------------------------------------------ les allegements

/// La ligne du bulletin qui porte la reduction generale `rg` d'un brut `brut`.
pub(crate) fn ligne_de_reduction(rg: &ReductionGenerale, brut: Decimal) -> LigneCotisation {
    LigneCotisation {
        id: "reduction_generale".into(),
        famille: FAMILLE_ALLEGEMENTS.into(),
        libelle: "Réduction générale dégressive unique".into(),
        base: brut,
        taux_pat: None,
        montant_pat: -rg.montant,
        taux_sal: None,
        montant_sal: Decimal::new(0, 2),
        masquer_base_et_taux: true,
    }
}

/// Les bases de l'option de l'employeur (fiche entreprise, `deduction_forfaitaire_option`) : arrete du 4 septembre 2025
/// relatif aux frais professionnels, art. 9, I, al. 2 et 3 (`atelier/reference/sources/dfs/`). « L'employeur peut opter
/// pour la deduction forfaitaire specifique lorsqu'une convention ou un accord collectif l'a explicitement prevu ou
/// lorsque le comite d'entreprise, le comite social et economique ou les delegues du personnel ont donne leur accord. A
/// defaut, il appartient a chaque salarie d'accepter ou non cette option. » (`droit::fiche::OptionDfs`.)
///
/// L'accord du salarie, quand l'option repose sur lui (fiche salarie, `deduction_forfaitaire_accord`) : art. 9, I,
/// al. 3, « Celle-ci peut alors figurer soit dans le contrat de travail ou soit dans un avenant au contrat de travail. A
/// defaut, l'employeur informe et recueille le consentement annuel du salarie par tout moyen. Lorsque le travailleur
/// salarie et assimile ne repond pas dans un delai raisonnable a cette consultation, son silence vaut accord. Le salarie
/// peut toutefois demander a tout moment a son employeur a beneficier ou ne plus beneficier de la deduction, avec
/// application au 1er janvier de l'annee suivante. » Le consentement est annuel : la fiche le date (son historique).
pub(crate) const DFS_SALARIE_CONTRAT: &str = "contrat";
pub(crate) const DFS_SALARIE_CONSENTEMENT: &str = "consentement_annuel";
pub(crate) const DFS_SALARIE_REFUS: &str = "refus";

/// L'option de DFS qui vaut pour ce salarie : sa profession, son taux de l'annee, les lignes abattues, le plafond annuel,
/// les remboursements de frais que l'annexe 2 de l'arrete laisse hors de l'assiette et la tolerance du BOSS qui y laisse
/// les autres.
#[derive(Debug, Clone, PartialEq)]
pub struct OptionDfs {
    pub profession: String,
    pub categorie: String,
    pub taux: Decimal,
    pub rubriques: Vec<String>,
    pub plafond_annuel: Decimal,
    /// Les rubriques de frais (`defraiement_forfait`, `repas`, `nuitee`, `defraiement_reel`) que l'annexe 2 de l'arrete (I, 2° et 5°) laisse
    /// hors de l'assiette pour cette profession ; tout autre remboursement de frais se reintegrerait (art. 9, I, al. 4).
    pub frais_hors_assiette: Vec<String>,
    /// La tolerance du BOSS, Frais professionnels, § 2320 (bareme, `frais_cumulables`) : pour les huit secteurs, dont le
    /// spectacle vivant et le spectacle enregistre, << l'ensemble des remboursements de frais professionnels definis par
    /// l'arrete du 4 septembre 2025 peut faire l'objet d'un cumul avec la deduction forfaitaire specifique >>
    /// (`atelier/reference/sources/boss/boss-frais-professionnels-20261013.txt`, l. 2917-2921).
    pub frais_cumulables: bool,
}


/// La DFS que la fiche exerce, ou `None`. La decision vit ici (regle 2) : la profession de la fiche salarie
/// (`deduction_forfaitaire`), l'option de l'employeur (fiche entreprise, `deduction_forfaitaire_option`) et, quand
/// l'option repose sur chaque salarie, son accord (`deduction_forfaitaire_accord`). Arrete du 4 septembre 2025, art. 9
/// (`atelier/reference/sources/dfs/arrete-2025-frais-professionnels/`) ; les professions, l'article 5 de l'annexe IV du
/// CGI dans sa redaction en vigueur au 31/12/2000 (`atelier/reference/sources/dfs/cgi-annexe4-art5/`) et l'annexe 1 de
/// l'arrete ; leur taux de l'annee, le bareme (art. 9, II et III). Une section du bareme non marquee verifiee refuse.
pub(crate) fn deduction_forfaitaire(bar: &Pile, sal: &ChampsSalarie, ent: &ChampsEntreprise) -> Resultat<Option<OptionDfs>> {
    let Some(profession) = sal.deduction_forfaitaire.dite().map(|p| p.as_str().to_string()) else {
        return Ok(None);
    };
    let qui = crate::qui_de(sal)?;
    let annee = bar.annee;
    let dfs = bar.deduction_forfaitaire.as_ref();
    let professions = dfs.map(|d| &d.professions);
    let connues = || match professions {
        Some(p) => p.keys().cloned().collect::<Vec<_>>().join(", "),
        None => "aucune".to_string(),
    };
    let Some(fiche_prof) = professions.and_then(|p| p.get(&profession)) else {
        return Err(format!(
            "Deduction forfaitaire specifique de {} : la fiche dit << {} >>, que le bareme {} ne connait pas. La deduction \
est attachee a une PROFESSION (arrete du 4 septembre 2025, art. 9, I : << les professions, prevues a l'article 5 de \
l'annexe IV du code general des impots dans sa redaction en vigueur au 31 decembre 2000 >>), et c'est elle qui dit le \
taux et les frais que l'annexe 2 de l'arrete laisse hors de l'assiette : choisir la profession ({}).",
            qui, profession, annee, connues()
        ));
    };
    let dfs = dfs.ok_or_else(|| socle::echec::cle("Le barème de l'année ne porte pas la déduction forfaitaire spécifique : le signaler au développeur.", "deduction_forfaitaire"))?;
    // L'option de l'employeur, et l'accord qu'elle demande.
    match ent.deduction_forfaitaire_option.dite() {
        None => {
            return Err(format!(
                "Deduction forfaitaire specifique de {} : sa fiche dit qu'il l'exerce ({}), mais la compagnie n'a pas \
opte pour elle (fiche entreprise, << Deduction forfaitaire specifique >>). C'est une option de l'EMPLOYEUR (arrete du \
4 septembre 2025, art. 9, I, al. 2) : par un accord collectif qui l'a explicitement prevue, avec l'accord du comite \
social et economique, ou, a defaut, salarie par salarie. Rien n'est calcule.",
                qui, profession
            ))
        }
        Some(DfsEmployeur::AccordCollectif | DfsEmployeur::AccordCse) => {}
        Some(DfsEmployeur::AccordIndividuel) => match sal.deduction_forfaitaire_accord.dite() {
            Some(AccordDfs::Contrat | AccordDfs::ConsentementAnnuel) => {}
            Some(AccordDfs::Refus) => return Ok(None),
            None => {
                return Err(format!(
                    "Deduction forfaitaire specifique de {} : la compagnie l'exerce salarie par salarie, et la fiche ne \
dit pas l'accord de celui-ci. L'arrete du 4 septembre 2025 (art. 9, I, al. 3) veut qu'il l'accepte, << soit dans le \
contrat de travail ou soit dans un avenant >>, a defaut par un consentement annuel recueilli << par tout moyen >> (<< son \
silence vaut accord >>). Dire lequel ({}, {}) ou son refus ({}). Rien n'est calcule.",
                    qui, DFS_SALARIE_CONTRAT, DFS_SALARIE_CONSENTEMENT, DFS_SALARIE_REFUS
                ))
            }
        },
    }
    if !dfs.verifie {
        return Err(socle::echec::avec("Le barème de l'année ne paie pas encore la déduction forfaitaire spécifique : ses taux ne sont pas vérifiés.", crate::Detail::du(None, format!(
            "Deduction forfaitaire specifique de {} ({}) : la section deduction_forfaitaire du bareme {} n'est pas \
marquee verifiee (\"verifie\": true). Le moteur refuse de payer plutot que d'inventer -- comme pour un statut sans \
section verifiee.",
            qui, profession, annee
        ))));
    }
    let categorie = fiche_prof.categorie.clone();
    // Le taux de l'annee : celui de la profession quand l'arrete lui donne un calendrier a part (art. 9, III, pour les
    // professions hors spectacle vivant), sinon celui de sa categorie (art. 9, II).
    let taux = fiche_prof.taux.or_else(|| dfs.taux.get(&categorie).map(|n| n.0)).ok_or_else(|| {
        format!(
            "Deduction forfaitaire specifique de {} ({}) : le bareme {} n'a pas de taux pour la categorie << {} >>.",
            qui, profession, annee, categorie
        )
    })?;
    Ok(Some(OptionDfs {
        profession,
        categorie,
        taux,
        rubriques: dfs.rubriques_abattues.clone(),
        plafond_annuel: dfs.plafond_annuel,
        frais_hors_assiette: fiche_prof.frais_hors_assiette.clone(),
        frais_cumulables: dfs.frais_cumulables,
    }))
}

/// Ce que le moteur sait calculer sous DFS, et ce qu'il refuse en disant pourquoi.
/// - un contrat paye au mois (permanent, CDD de droit commun) : l'assiette de son chomage (reglement general, art. 49,
///   << entrant dans l'assiette des cotisations de securite sociale >>, sans le << avant application de l'abattement >>
///   des annexes VIII et X), sa prevoyance conventionnelle (3090 : << servant de base aux cotisations de la securite
///   sociale >> ; 1285 : rien), ses tranches de retraite et son assiette minimum mensuelle ne sont pas portees ;
/// - un cadre : l'Apec n'a pas de texte depose qui dise son assiette sous DFS ;
/// - un remboursement de frais que l'annexe 2 de l'arrete ne laisse pas hors de l'assiette, quand le bareme ne porte pas
///   la tolerance du BOSS (`frais_cumulables`) : l'article 9 (I, al. 4) le reintegre << y compris, le cas echeant, les
///   indemnites versees [...] a titre de remboursement des frais professionnels >>, et ce que la reintegration fait a
///   l'assiette chomage et a la DSN (type 35) n'est pas porte. Avec la tolerance (BOSS, Frais professionnels, § 2320,
///   `atelier/reference/sources/boss/boss-frais-professionnels-20261013.txt`, l. 2917-2921 : << l'ensemble des
///   remboursements de frais professionnels definis par l'arrete du 4 septembre 2025 peut faire l'objet d'un cumul avec
///   la deduction forfaitaire specifique >>), tout remboursement de frais reste hors de l'assiette abattue.
/// Une ligne hors assiette se classe par ce qu'elle est, la nature de sa rubrique (`Rubrique::hors_assiette`), pas par
/// son code : un remboursement de frais d'une rubrique de la compagnie est un remboursement de frais.
/// - une ligne hors assiette qui n'est pas un remboursement de frais (une indemnite) : la tolerance du BOSS ne la couvre
///   pas, et le rescrit BOSS-RES-000010 (meme page, § 2325, l. 2923-2934) veut une indemnite de transport conventionnelle
///   << dans l'assiette des cotisations et contributions sociales des le 1er euro prealablement a l'application de
///   l'abattement >> ; l'art. 9, I, al. 4 compose l'assiette du << montant global des remunerations, indemnites, primes
///   [...] >>. Aucun texte depose ne dit si une autre indemnite hors assiette (une indemnite de rupture exoneree par sa
///   propre regle) s'y reintegre : refus (regle 1). Un acompte, un mouvement de tresorerie, n'est rien de tout cela.
fn dfs_applicable(
    eng: &Engagement, nat: &Nature, o: &OptionDfs, lb: &[LigneBrut], rubriques_bulletin: &[Rubrique], racine: &std::path::Path,
    jour: &Jour,
) -> Resultat<()> {
    let qui = eng.qui();
    if nat.mensuelle() {
        return Err(socle::echec::avec(format!("{} : la déduction forfaitaire spécifique ne se porte que sur un contrat d'usage, pas sur un contrat payé au mois.", qui), crate::Detail::du(None, format!(
            "{} : une deduction forfaitaire specifique sur un contrat paye au mois ({}). Le logiciel ne la porte que sur \
un contrat d'usage : pour un salarie au mois, l'assiette du chomage (reglement general d'assurance chomage, art. 49), sa \
prevoyance conventionnelle, ses tranches de retraite et son assiette minimum ne sont pas encore calculees sous deduction \
(atelier/reference/sources/dfs/LISEZ-MOI.md, << Ce qui manque >>). Rien n'est calcule.",
            qui, nat.libelle
        ))));
    }
    if let Some(r) = rubriques_bulletin.iter().find(|r| r.id.starts_with("apec")) {
        return Err(socle::echec::avec(format!("{} : la déduction forfaitaire spécifique ne se porte pas sur la paie d'un cadre : aucun texte ne dit l'assiette de l'Apec sous déduction.", qui), crate::Detail::du(None, format!(
            "{} : une deduction forfaitaire specifique sur la paie d'un cadre (ligne << {} >>). L'assiette de l'Apec sous \
deduction n'est dans aucun texte depose (atelier/reference/sources/dfs/LISEZ-MOI.md, << Ce qui manque >>) : le logiciel \
ne la devine pas. Rien n'est calcule.",
            qui,
            r.libelle.iter().cloned().collect::<String>()
        ))));
    }
    for l in lb.iter().filter(|l| !l.soumis && !l.montant.is_zero()) {
        let genre = rubrique(racine, &l.code, jour)?.hors_assiette()?;
        if genre == HorsAssiette::Revenu {
            return Err(socle::echec::avec(format!("{} : la ligne « {} » est hors assiette sans être un remboursement de frais, et il est sous déduction forfaitaire spécifique : la soumettre, ou en faire un remboursement de frais.", qui, l.libelle), crate::Detail::du(None, format!(
                "{} : la ligne << {} >> est hors assiette et n'est pas un remboursement de frais, pour un salarie sous \
deduction forfaitaire specifique ({}). La tolerance du BOSS (Frais professionnels, § 2320) ne laisse hors de l'assiette \
que les remboursements de frais au sens de l'arrete du 4 septembre 2025 ; le rescrit BOSS-RES-000010 (§ 2325) veut une \
indemnite de transport conventionnelle dans l'assiette << des le 1er euro prealablement a l'application de \
l'abattement >>, et aucun texte depose ne dit ce que devient une autre indemnite hors assiette. Le logiciel ne le \
devine pas : si c'est un remboursement de frais, sa rubrique est de nature `frais` ; si elle entre dans l'assiette, \
elle est soumise. Rien n'est calcule.",
                qui, l.libelle, o.profession
            ))));
        }
        let frais = genre == HorsAssiette::Frais;
        if frais && !o.frais_cumulables && !o.frais_hors_assiette.contains(&l.code) {
            return Err(socle::echec::avec(format!("{} : la ligne « {} » rembourse des frais sous déduction forfaitaire spécifique, et le logiciel ne porte pas leur réintégration dans l'assiette.", qui, l.libelle), crate::Detail::du(None, format!(
                "{} : la ligne << {} >> rembourse des frais a un salarie sous deduction forfaitaire specifique ({}). \
L'arrete du 4 septembre 2025 (art. 9, I, al. 4) les reintegre alors dans l'assiette avant l'abattement, sauf ceux de \
la liste de son annexe 2 : pour cette profession, {} ; et le bareme ne porte pas la tolerance du BOSS (Frais \
professionnels, § 2320, `frais_cumulables`) qui les laisserait tous hors de l'assiette. Le logiciel ne porte pas \
cette reintegration (assiette du chomage, bloc 54 de type 35) : rien n'est calcule.",
                qui,
                l.libelle,
                o.profession,
                if o.frais_hors_assiette.is_empty() {
                    "aucun".to_string()
                } else {
                    o.frais_hors_assiette.join(", ")
                }
            ))));
        }
    }
    Ok(())
}

/// Ce que l'annee civile a deja deduit pour ce salarie avant ce contrat : les mois anterieurs de la meme annee
/// (`conges::dfs_du_mois`, recalcules depuis leurs saisies) et les contrats precedents du meme mois (`precedentes`).
/// Arrete du 4 septembre 2025, art. 9, I, al. 1 : << dans la limite de 7 600 euros par annee civile >>. INFERENCE, ecrite comme
/// telle : l'annee est celle de la periode de paie, et les contrats d'un mois se suivent dans l'ordre de la saisie.
fn dfs_deja_deduite(ctx: &Contexte, eng: &Engagement, sal: &FicheSalarie, precedentes: &[Paie]) -> Resultat<Decimal> {
    let cle = sal.id.to_string();
    let mois = eng.fin_periode()?.mois_civil().to_string();
    let mut deja = crate::conges::dfs_des_mois_anterieurs(ctx, &mois, &cle)?;
    for p in precedentes {
        let sienne = p.salarie.id == cle || p.engagement.salarie == eng.salarie;
        if sienne {
            if let Some(d) = &p.deduction_forfaitaire {
                deja += d.montant;
            }
        }
    }
    Ok(deja)
}

/// Ce que la deduction retire d'un contrat, et les assiettes abattues de ses rubriques de salaire.
#[derive(Debug, Clone, PartialEq)]
pub struct AbattementDfs {
    pub deduction: Decimal,
    pub assiettes: Vec<Decimal>,
    pub avis: Vec<String>,
}

/// La deduction d'un contrat : chaque rubrique de salaire abattue du taux de l'annee (arrondie au centime),
/// puis
/// - le plafond annuel : << dans la limite de 7 600 euros par annee civile >> (arrete du 4 septembre 2025, art. 9, I,
///   al. 1) ;
///   `deja` est ce que l'annee a deja deduit ;
/// - l'assiette minimum : << L'application de ces dispositions s'entend sans prejudice du sixieme alinea de l'article
///   R. 242-1 du code de la securite sociale >> (art. 9, I, dernier alinea) ; R. 242-1, al. 6 : la base << ne peut etre inferieure,
///   en aucun cas, au montant cumule [...] du salaire minimum de croissance >> ; BOSS, assiette generale, § 410 : le
///   SMIC, sans les elements conventionnels. `assiette_minimale` est le SMIC du contrat, celui que la DSN retient (SMIC
///   horaire x heures, `calculer` 7.5). INFERENCE pour un cachet : ses heures sont celles que le moteur lui donne pour
///   la DSN (`heures_par_cachet`), aucun texte depose ne dit les heures travaillees d'un cachet.
/// Quand l'une ou l'autre ramene la deduction, l'assiette abattue se repartit entre les rubriques au prorata de leur
/// montant, le dernier centime sur la derniere.
pub(crate) fn abattement_dfs(o: &OptionDfs, montants: &[Decimal], brut: Decimal, deja: Decimal, assiette_minimale: Decimal) -> AbattementDfs {
    let zero = Decimal::new(0, 2);
    let facteur = sub(Decimal::ONE, div(o.taux, Decimal::ONE_HUNDRED));
    let theoriques: Vec<Decimal> = montants.iter().map(|m| eur(mul(*m, facteur))).collect();
    let pleine = brut - theoriques.iter().fold(zero, |a, b| a + *b);
    let mut avis = Vec::new();
    let reste = zero.max(o.plafond_annuel - deja);
    let mut retenue = pleine.min(reste);
    if retenue < pleine {
        avis.push(format!(
            "Déduction forfaitaire spécifique ramenée de {} à {} : le plafond de {} par année civile (arrêté du \
4 septembre 2025, art. 9) est atteint, {} déjà déduits cette année.",
            euros(pleine), euros(retenue), euros(o.plafond_annuel), euros(deja)
        ));
    }
    if brut - retenue < assiette_minimale {
        let plancher = zero.max(brut - assiette_minimale).min(retenue);
        avis.push(format!(
            "Déduction forfaitaire spécifique ramenée de {} à {} : l'assiette abattue ne descend pas sous le SMIC du \
contrat, {} (CSS R. 242-1, al. 6 ; arrêté du 4 septembre 2025, art. 9, I, dernier alinéa).",
            euros(retenue), euros(plancher), euros(assiette_minimale)
        ));
        retenue = plancher;
    }
    let assiettes = if retenue == pleine || brut <= Decimal::ZERO {
        theoriques
    } else {
        let cible = brut - retenue;
        let mut out = Vec::with_capacity(montants.len());
        let mut cumul = zero;
        for (i, m) in montants.iter().enumerate() {
            let a = if i + 1 == montants.len() { cible - cumul } else { eur(div(mul(*m, cible), brut)) };
            cumul += a;
            out.push(a);
        }
        out
    };
    let deduction = brut - assiettes.iter().fold(zero, |a, b| a + *b);
    AbattementDfs { deduction, assiettes, avis }
}

// ---------------------------------------------------------------- calculer

pub(crate) const FAMILLE_ALLEGEMENTS: &str = "EXONÉRATIONS ET ALLÈGEMENTS DE COTISATIONS";

/// Les assiettes que le calcul relit par leur nom dans les bases de la paie (`Contenu::bases`) : celle des Conges Spectacles,
/// le brut majore de 15 %, le brut apres la deduction forfaitaire, la tranche de la CET, l'assiette de la CSG.
pub(crate) const BRUT_CS: &str = "brut_cs";
pub(crate) const BRUT_X115: &str = "brut_x115";
pub(crate) const BRUT_DFS: &str = "brut_dfs";
pub(crate) const CET: &str = "cet";
pub(crate) const CSG: &str = "csg";

/// Les bases lues par cle : une valeur par rubrique de salaire, sinon globale.
const GLOBALES: [&str; 20] = [
    "csg", "t1_journaliere", "t2_journaliere", "t2_prevoyance_journaliere", "apec", "apec_t2_journaliere", "cet", "t1_mensuelle",
    "t2_mensuelle", "apec_mensuelle", "cet_mensuelle", "pss_periode", "t2_conventionnelle", "t1_t2_mensuelle",
    "brut_plafonne", "brut_x115_plafonne", "brut_chomage", "brut_plafonne_dfs", "brut_x115_plafonne_dfs",
    "t1_mensuelle_x115",
];

/// Les assiettes d'un montant de salaire (§7.4).
struct Assiettes {
    brut: Decimal,
    brut_cs: Decimal,
    csg: Decimal,
    x115: Decimal,
    x110: Decimal,
    dfs: Option<Decimal>,
    /// Les assiettes majorees apres la deduction forfaitaire specifique : la majoration s'applique au brut abattu
    /// (lettre circulaire Acoss 2013-019, annexe 1, R6 ; fiche Audiens « Les Conges Spectacles », 2023 : 334,50 =
    /// 300 x 1,115 ; atelier/reference/sources/majoration-caisses-conges/).
    x115_dfs: Option<Decimal>,
    x110_dfs: Option<Decimal>,
}

impl Assiettes {
    fn lire(&self, cle: &str) -> Option<Decimal> {
        match cle {
            "brut" => Some(self.brut),
            "brut_cs" => Some(self.brut_cs),
            "brut_x115" => Some(self.x115),
            "brut_x110" => Some(self.x110),
            "brut_dfs" => self.dfs,
            "brut_x115_dfs" => self.x115_dfs,
            "brut_x110_dfs" => self.x110_dfs,
            _ => None,
        }
    }
}

fn pss_du_contexte(ctx: &Contexte) -> impl Fn(i32) -> Resultat<(Decimal, Decimal)> + '_ {
    move |a| ctx.plafond_securite_sociale(a).map(|(m, h)| (Decimal::from(m), Decimal::from(h)))
}

fn tuple(j: &Jour) -> (i32, u32, u32) {
    (j.an, j.mois, j.jour)
}

/// `bareme_applique(&ctx.bareme(fin), sal, nat, fin, caisse)`, garde : la pile ne
/// depend que des fichiers du bareme, de la fiche entreprise, du salarie, de
/// la nature, de la caisse de conges et du jour. Un mois et ses mois anterieurs la recomposaient a
/// chaque paie, et c'etait l'essentiel du temps de calcul.
pub(crate) fn bareme_du_salarie(ctx: &Contexte, sal: &SalarieDuContrat, nat: &Nature, fin: &Jour, caisse: bool, cpf_cdd: bool, majoration: bool) -> Resultat<Arc<Pile>> {
    pile(ctx, sal, nat, fin, caisse, cpf_cdd, majoration, false)
}

/// La pile d'une paie IMPORTEE, qui ne range que ses bases : ses taux se lisent sur ses pieces
/// (`bareme::EntrepriseLue::taux_lus`).
pub(crate) fn bareme_pour_ranger(ctx: &Contexte, sal: &SalarieDuContrat, nat: &Nature, fin: &Jour, caisse: bool, majoration: bool) -> Resultat<Arc<Pile>> {
    pile(ctx, sal, nat, fin, caisse, false, majoration, true)
}

#[allow(clippy::too_many_arguments)]
fn pile(ctx: &Contexte, sal: &SalarieDuContrat, nat: &Nature, fin: &Jour, caisse: bool, cpf_cdd: bool, majoration: bool, taux_lus: bool) -> Resultat<Arc<Pile>> {
    type Cache = Mutex<HashMap<String, Arc<Pile>>>;
    static CACHE: OnceLock<Cache> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let cle = format!("{}|{}|{}|{:?}|{}|{}|{}|{}|{}", ctx.empreinte_bareme(fin)?, fin.iso(), nat.cle, sal, nat.mensuelle(), caisse, cpf_cdd, majoration, taux_lus);
    if let Some(v) = cache.lock().unwrap().get(&cle) {
        return Ok(v.clone());
    }
    let ent = droit::bareme::EntrepriseLue { taux_lus, ..ctx.entreprise_lue(fin)? };
    let bar = Arc::new(bareme_applique(&*ctx.bareme(fin)?, &ent, sal, nat, Some(fin), caisse, cpf_cdd, majoration)?);
    socle::fichiers::ranger(cache, cle, bar.clone());
    Ok(bar)
}

/// La fiche salarie telle qu'elle vaut pour l'engagement `eng`, emploi compris (`emplois::emploi_de`). Un contrat
/// d'usage la lit telle qu'au premier jour du contrat (un nouveau contrat est un nouveau contrat) ; un contrat paye au
/// mois, telle qu'a la fin de la periode payee (`emplois::emploi_du_mois`). La paie (`calculer`) et ce qui lit un contrat
/// sans le payer (`amorcage::embauche` : la DPAE, le signalement d'amorcage, les emplois multiples de la DSN) la
/// lisent ici, et pas autrement : deux lectures d'un meme contrat feraient deux emplois.
pub(crate) fn fiche_de_l_engagement(ctx: &Contexte, eng: &Engagement) -> Resultat<SalarieDuContrat> {
    let nat = eng.nat()?;
    let cle = ctx.cle_du_salarie(&eng.salarie)?;
    let emploi = EmploiEngagement {
        emploi: eng.emploi.clone(),
        emploi_affichage: eng.emploi_affichage.clone(),
        pcs_ese: eng.pcs_ese.clone(),
        code_emploi_ft: eng.code_emploi_ft.clone(),
        statut: eng.statut.clone(),
        statut_conventionnel: eng.statut_conventionnel.clone(),
        statut_categoriel_rc: eng.statut_categoriel_rc.clone(),
    };
    let fiche = if nat.mensuelle() {
        // La fiche de la derniere periode elementaire : un mois decoupe declare la derniere valeur connue
        // (norme DSN 2026.1, § 2.1.2.4, p. 22) ; un mois d'un seul tenant, la periode de paie entiere. La dispense de
        // sante se lit sur ces memes bornes (`dispense_de_la_periode`).
        let (debut, fin) = *crate::periodes::bornes(ctx, eng)?.last().ok_or("aucune periode de paie")?;
        let du_mois = droit::emplois::emploi_du_mois(ctx, &cle, &debut, &fin)?;
        let fiche = dispense_de_la_periode(emploi_de(&du_mois, &emploi)?, &debut, &fin)?;
        sante_de_la_suspension(ctx, eng, convention_de_l_employeur(ctx, fiche))?
    } else {
        convention_de_l_employeur(ctx, emploi_de(&ctx.salarie_au(&cle, &eng.debut)?, &emploi)?)
    };
    Ok(anciennete_a_l_embauche(etablissement_du_contrat(fiche, eng), eng))
}

/// LA CONVENTION DE L'EMPLOYEUR sur la fiche que la paie lit (`droit::caisse_conges::IDCC_DE_L_EMPLOYEUR`) : l'IDCC de
/// l'entreprise, telle que `ctx` la lit. Le domaine d'activite de l'annexe 8 se lit a cette convention
/// (`droit::caisse_conges::intermittent`).
pub(crate) fn convention_de_l_employeur(ctx: &Contexte, fiche: FicheSalarie) -> SalarieDuContrat {
    let idcc = ctx.entreprise.idcc.dite().filter(|i| !i.is_empty()).cloned();
    SalarieDuContrat { idcc_de_l_employeur: idcc, ..SalarieDuContrat::de(fiche) }
}

/// L'ETABLISSEMENT DE RATTACHEMENT du salarie pour ce contrat (S21.G00.11.001, « l'etablissement de rattachement
/// administratif du salarie », `norme-dsn/texte/bloc-S21.G00.11.txt`) : celui que l'engagement dit, quand la compagnie en a
/// plusieurs ; sinon le principal. La fiche le porte en `etablissement`, que la DSN, les signalements et le bareme lisent
/// (`droit::bareme::etablissement_du_salarie`).
pub fn etablissement_du_contrat(mut fiche: SalarieDuContrat, eng: &Engagement) -> SalarieDuContrat {
    fiche.etablissement = eng.etablissement.clone();
    fiche
}

/// L'ANCIENNETE DANS L'ENTREPRISE (S21.G00.86.003, `norme-dsn/texte/bloc-S21.G00.86.txt`) d'une fiche qui ne la date pas
/// encore : elle part de l'embauche, le premier jour de ce contrat, qui est le premier du salarie (la validation de son
/// premier engagement la pose sur sa fiche, `moteur::routes::fiches::poser_anciennete_a_l_embauche`, ou elle se change).
/// INFERENCE : l'anciennete dans l'entreprise commence a l'entree dans l'entreprise.
fn anciennete_a_l_embauche(mut fiche: SalarieDuContrat, eng: &Engagement) -> SalarieDuContrat {
    if !fiche.date_anciennete.est_dite() {
        fiche.fiche.champs.date_anciennete = Case::Dite(eng.debut);
    }
    fiche
}

/// LA DATE D'ANCIENNETE DANS L'ENTREPRISE de `fiche` pour un contrat qui commence le `debut` : celle que la fiche dit,
/// sinon l'embauche, le premier jour de ce contrat (`anciennete_a_l_embauche`).
pub(crate) fn date_d_anciennete(fiche: &ChampsSalarie, debut: &Jour) -> Jour {
    match fiche.date_anciennete.dite() {
        Some(d) => *d,
        None => *debut,
    }
}

/// LE STATUT DU SALARIE, artiste ou technicien, que sa fiche dit : il choisit la section du bareme (l'abattement de
/// l'arrete du 24/01/1975 ne vise que les artistes), le plafond, la regle d'un mois incomplet. Une fiche qui ne le dit
/// pas se refuse : rien ne le devine.
pub(crate) fn statut_de(sal: &ChampsSalarie) -> Resultat<&'static str> {
    sal.statut.dite().map(|s| s.as_str()).ok_or_else(|| {
        socle::echec::cle("La fiche du salarié ne dit pas son statut, artiste ou technicien : le dire sur sa fiche.", "statut")
    })
}


/// La paie d'un engagement, les paies deja calculees du meme mois en main (`engagement::charger_donnees`, dans l'ordre de la
/// saisie) : la deduction forfaitaire specifique d'un salarie se plafonne par annee civile, contrats precedents du mois
/// compris (`dfs_deja_deduite`). Un mois d'un seul tenant se calcule en une fois (`calculer_periode`) ; un mois ou
/// le statut, la categorie ou la quotite d'un contrat paye au mois change en cours de periode se decoupe en periodes
/// elementaires (`periodes::bornes`), chacune calculee de meme, les periodes deja calculees comptant parmi les
/// precedentes, et la paie du mois en est la somme (`periodes::assembler`).
///
/// Un maintien de salaire garanti en net (`maintien::au_net`) se reconstitue par ce meme calcul, refait sur les lignes du
/// mois jusqu'a ce que le net garanti soit atteint au centime ; la paie est alors celle de ce montant.
pub(crate) fn calculer_apres(ctx: &Contexte, eng: &Engagement, precedentes: &[Paie]) -> Resultat<Paie> {
    let mut paie_des = |lignes: Vec<crate::LignePaie>| -> Resultat<Paie> {
        let mut e = eng.clone();
        e.lignes = lignes;
        calculer_du_mois(ctx, &e, precedentes)
    };
    match crate::maintien::au_net(&eng.lignes, &mut paie_des)? {
        None => calculer_du_mois(ctx, eng, precedentes),
        Some((montant, avis)) => {
            let mut e = eng.clone();
            e.lignes = crate::maintien::avec_le_montant(&eng.lignes, montant)?;
            let mut p = calculer_du_mois(ctx, &e, precedentes)?;
            p.ecrire().avertissements.push(avis);
            Ok(p)
        }
    }
}

/// La paie d'un mois, d'un seul tenant ou en periodes elementaires.
fn calculer_du_mois(ctx: &Contexte, eng: &Engagement, precedentes: &[Paie]) -> Resultat<Paie> {
    let bornes = crate::periodes::bornes(ctx, eng)?;
    if bornes.len() <= 1 {
        return calculer_periode(ctx, eng, precedentes);
    }
    let mut parts: Vec<Paie> = Vec::new();
    for e in crate::periodes::engagements_des_periodes(ctx, eng, &bornes)? {
        let avant: Vec<Paie> = precedentes.iter().chain(parts.iter()).cloned().collect();
        parts.push(calculer_periode(ctx, &e, &avant)?);
    }
    crate::periodes::assembler(ctx, eng, parts)
}

/// L'assiette d'une rubrique : le calcul ne cotise pas sur une assiette que la rubrique ne dit pas.
fn base_de(r: &Rubrique) -> Resultat<&str> {
    r.base.as_deref().ok_or_else(|| socle::fichiers::absent("base"))
}

/// Le taux patronal d'une rubrique, s'il n'est pas nul.
fn part_patronale(r: &Rubrique) -> Option<f64> {
    r.taux_pat.filter(|t| *t != 0.0)
}

/// La pile de bareme sur laquelle se calcule la paie `p` (celle de `calculer_periode`, du meme cache).
pub(crate) fn pile_de_la_paie(ctx: &Contexte, p: &Paie) -> Resultat<Arc<Pile>> {
    let eng = &p.engagement;
    let nat = eng.nat()?;
    let caisse = droit::caisse_conges::affilie(&nat, &p.salarie, &eng.debut, eng.fin.as_ref())?;
    let cpf_cdd = droit::cpf_cdd::contrat_ouvre_cpf_cdd(
        &nat,
        eng.motif_recours.as_deref(),
        &droit::cpf_cdd::le_contrat(eng.numero.as_deref(), &eng.salarie),
    )?;
    let majoration = droit::majoration_chomage::due(&nat, eng.motif_recours.as_deref(), &eng.debut, eng.fin.as_ref());
    bareme_du_salarie(ctx, &p.salarie, &nat, &eng.fin_periode()?, caisse, cpf_cdd, majoration)
}

/// LA DISPENSE DE SANTE A DES BORNES. Les accords de branche deposes en
/// donnent : la dispense de l'assure individuel « ne peut jouer que jusqu'a l'echeance du contrat individuel », celle
/// d'une couverture collective se justifie chaque annee, et le salarie « cessant de justifier » de son cas adhere
/// (CCN 3090, art. 9.2 et 9.2.2 b et d, `atelier/reference/sources/kali-3090-accord-sante-permanents-20240719-20261011.md` ;
/// CCNEAC XII.2.1.8.1 b, `kali-1285-accord-frais-de-sante-20260130-20261011.md` l. 78). La fiche porte le motif (`sante_dispense`) et, s'il y a lieu, `sante_dispense_depuis` et
/// `sante_dispense_jusqu_au` ; hors de ces bornes, le salarie est affilie : la fiche que la paie lit perd le motif, et
/// le bareme (`bareme_sante`) comme la DSN (`dsn::affiliations_du_permanent`) le traitent en affilie.
///
/// La periode payee entiere dans la dispense : dispense ; entiere dehors : affilie. Une borne en cours de periode
/// (une dispense qui commence avec le contrat n'en est pas une) refuse : le partage de la cotisation du mois entre
/// dispense et affiliation n'est dit par aucun texte depose (regle 1).
pub(crate) fn dispense_de_la_periode(mut fiche: FicheSalarie, debut: &Jour, fin: &Jour) -> Resultat<FicheSalarie> {
    if fiche.sante_dispense.texte().is_empty() {
        return Ok(fiche);
    }
    let depuis = fiche.sante_dispense_depuis.dite().copied();
    let jusqu_au = fiche.sante_dispense_jusqu_au.dite().copied();
    if depuis.is_some_and(|d| d > *fin) || jusqu_au.is_some_and(|j| j < *debut) {
        fiche.champs.sante_dispense = Case::Absente;
        return Ok(fiche);
    }
    let borne = match (depuis.filter(|d| d > debut), jusqu_au.filter(|j| j < fin)) {
        (Some(d), _) => Some(format!("commence le {}", d.jj_mm_aaaa())),
        (None, Some(j)) => Some(format!("finit le {}", j.jj_mm_aaaa())),
        (None, None) => None,
    };
    if let Some(borne) = borne {
        let qui = [fiche.prenom.texte(), fiche.nom.texte()].into_iter().filter(|v| !v.is_empty()).collect::<Vec<_>>().join(" ");
        return Err(format!(
            "{} : sa dispense de complementaire sante {}, en cours de periode payee (du {} au {}). Le moteur ne partage \
pas la cotisation sante d'un mois entre dispense et affiliation : aucun texte depose ne dit comment. Dater la borne de \
la dispense au premier jour d'un mois (fiche salarie, « Dispense depuis ») ou au dernier jour d'un mois (« Dispense \
jusqu'au »), si l'organisme et la convention le permettent ; sinon c'est a trancher. Rien n'est calcule.",
            if qui.is_empty() { "Ce salarie".to_string() } else { qui },
            borne,
            debut.jj_mm_aaaa(),
            fin.jj_mm_aaaa()
        ));
    }
    Ok(fiche)
}

/// LA COMPLEMENTAIRE SANTE D'UN MOIS ENTIEREMENT SUSPENDU SANS INDEMNISATION. Un permanent dont toute la
/// periode payee est couverte par des suspensions non remunerees (conge parental 632, sabbatique 639, sans solde 501... :
/// `imprevus::suspendu_sans_remuneration` ; un arret de travail n'en est pas une) :
///
/// - **3090** : « Le benefice des garanties [...] est suspendu pour les salaries permanents dont le contrat de travail est
///   suspendu et ne donne lieu a aucune indemnisation » ; le salarie qui veut rester affilie paie « integralement » les
///   deux parts, que « l'organisme assureur preleve [...] directement aupres du salarie » (accord du 19/07/2024, etendu,
///   art. 4.2 et 15, `atelier/reference/sources/kali-3090-accord-sante-permanents-20240719-20261011.md`). Ni cotisation ni
///   part patronale sur la paie de ce mois, donc rien a reintegrer au net fiscal : la fiche porte `sante_suspendue`.
/// - **toute autre convention** (1285 : l'accord du 30/01/2026 ne parle que de la suspension « indemnisee » ; un contrat
///   d'entreprise) : AUCUN TEXTE DEPOSE ne dit ce que devient la sante ; le droit commun laisse le maintien facultatif
///   (instruction DSS/3C/5B/2021/127, II.B, `atelier/reference/sources/suspension-non-remuneree/dss-instruction-3c5b-2021-
///   127-suspension-contrat-prevoyance-20210617.txt`) et c'est le contrat d'adhesion de la compagnie qui tranche. La fiche
///   entreprise le dit (`SANTE_SUSPENSION`) : « suspendue », comme la 3090 ; « maintenue », la paie cotise comme un mois
///   ordinaire (la part patronale se reintegre alors au net fiscal, CGI 83, 1° quater, et le montant soumis au PAS reste a
///   0.00 : `pas::du_versement`). Sans reponse, refus (regle 1).
///
/// La prevoyance n'a rien a suspendre ici : 3090 et 1285 l'assoient sur la remuneration (tranche A, tranche 1 ;
/// `legifrance-3090-prevoyance-titre12-20260919.md`, `legifrance-1285-prevoyance-titre12-20260919.md`), nulle ce
/// mois-la -- inference : aucun texte depose ne traite de la suspension non indemnisee pour la prevoyance.
fn sante_de_la_suspension(ctx: &Contexte, eng: &Engagement, mut fiche: SalarieDuContrat) -> Resultat<SalarieDuContrat> {
    if !fiche.sante_dispense.texte().is_empty() {
        return Ok(fiche);
    }
    let cle = fiche.id.to_string();
    let (debut, fin) = (eng.debut_periode()?, eng.fin_periode()?);
    let suspensions = crate::imprevus::suspendu_sans_remuneration(&ctx.racine, &cle, eng.numero.as_deref(), &eng.debut, &debut, &fin)?;
    let Some(premiere) = suspensions.first() else { return Ok(fiche) };
    let motifs: Vec<String> = suspensions.iter().map(|s| s.motif().to_string()).collect();
    let ent = ctx.entreprise_au(&fin);
    let idcc = ent.idcc.texte().to_string();
    let pourquoi = if idcc == "3090" {
        format!(
            "contrat suspendu sans indemnisation tout le mois (motif {}) : les garanties de sante sont suspendues (CCN 3090, \
accord sante du 19/07/2024, art. 4.2 et 15)",
            motifs.join(", ")
        )
    } else {
        match ent.sante_suspension_non_indemnisee.code() {
            SANTE_SUSPENSION_SUSPENDUE => format!(
                "contrat suspendu sans indemnisation tout le mois (motif {}) : les garanties de sante sont suspendues (fiche \
entreprise, « {} »)",
                motifs.join(", "),
                SANTE_SUSPENSION_SUSPENDUE
            ),
            SANTE_SUSPENSION_MAINTENUE => return Ok(fiche),
            autre => {
                let qui = [fiche.prenom.texte(), fiche.nom.texte()].into_iter().filter(|v| !v.is_empty()).collect::<Vec<_>>().join(" ");
                return Err(socle::echec::avec(format!("{} : son contrat est suspendu sans indemnisation, et la fiche entreprise ne dit pas ce que devient sa complémentaire santé : le dire dans Configuration › Entreprise, « Santé d'un permanent suspendu sans indemnisation ».", if qui.is_empty() { cle.clone() } else { qui.clone() }), crate::Detail::du(Some(SANTE_SUSPENSION), format!(
                    "{} : son contrat est suspendu sans indemnisation sur toute la periode payee (du {} au {} ; suspension {} \
depuis le {}). Que devient sa complementaire sante ce mois-la ? Sous la convention {}, aucun texte ne le dit : c'est le \
contrat de sante de la compagnie qui tranche (le maintien est facultatif, instruction DSS du 17/06/2021, II.B). {}A dire \
une fois pour toutes : Configuration › Entreprise, « Santé d'un permanent suspendu sans indemnisation » (`{}`) -- « {} » \
si le contrat suspend les garanties (rien sur la paie), « {} » s'il les maintient avec les cotisations. Rien n'est calcule.",
                    if qui.is_empty() { cle.clone() } else { qui },
                    debut.jj_mm_aaaa(),
                    fin.jj_mm_aaaa(),
                    motifs.join(", "),
                    Jour::lire(premiere.debut())?.jj_mm_aaaa(),
                    if idcc.is_empty() { "de la compagnie".to_string() } else { idcc.clone() },
                    if autre.is_empty() { String::new() } else { format!("La fiche dit « {} », qui n'est pas une reponse. ", autre) },
                    SANTE_SUSPENSION,
                    SANTE_SUSPENSION_SUSPENDUE,
                    SANTE_SUSPENSION_MAINTENUE
                ))));
            }
        }
    };
    fiche.sante_suspendue = Some(pourquoi);
    Ok(fiche)
}

/// Le contrat de cette paie est-il declare a la caisse des Conges Spectacles (`caisse_conges::affilie`, D. 7121-41) :
/// son contrat et la fiche qu'elle a lue.
pub(crate) fn declaree_a_la_caisse_des_conges(p: &Paie) -> Resultat<bool> {
    let e = &p.engagement;
    droit::caisse_conges::affilie(&*e.nat()?, &p.salarie, &e.debut, e.fin.as_ref())
}

/// La meme question pour la saisie `e` d'un contrat paye au mois, avant que son engagement ne soit construit (le solde de
/// tout compte de `conges::lignes_du_mois`) : sa fiche telle que sa paie la lira au dernier jour du contrat, le metier que
/// la saisie dit l'emportant (`fiche_de_l_engagement`, `emploi_de`).
pub(crate) fn saisie_declaree_a_la_caisse_des_conges(ctx: &Contexte, e: &crate::saisie::Carte, fin: &Jour) -> Resultat<bool> {
    let nat = e.nature_lue()?;
    let debut = crate::conges::debut_de(e)?;
    let cle = ctx.cle_du_salarie(e.salarie.as_deref().ok_or_else(|| socle::fichiers::absent("salarie"))?)?;
    let dit = |c: &Option<String>| c.clone().filter(|x| !x.is_empty());
    let emploi = EmploiEngagement { code_emploi_ft: dit(&e.code_emploi_ft).or_else(|| dit(&e.metier)), ..Default::default() };
    let fiche = convention_de_l_employeur(ctx, emploi_de(&ctx.salarie_au(&cle, fin)?, &emploi)?);
    droit::caisse_conges::affilie(&nat, &fiche, &debut, Some(fin))
}

/// LA PERIODE D'ESSAI QUE L'ENGAGEMENT STIPULE ne depasse pas la duree maximale de son contrat (`droit::essai` : C. trav.
/// L. 1242-10 pour un CDD, L. 1221-19 pour un CDI, la convention 3090 art. 7.4 pour un artiste en CDI). Sa categorie se lit
/// sur la fiche du salarie telle que l'engagement la voit (`fiche_de_l_engagement`).
fn essai_dans_son_plafond(ctx: &Contexte, eng: &Engagement, nat: &Nature, sal: &ChampsSalarie) -> Resultat<()> {
    let Some(v) = crate::ligne::dite(&eng.ecrit.periode_essai_jours) else { return Ok(()) };
    // Une virgule decimale se lit comme un point ; un texte qui n'est pas un nombre se refuse.
    let lu = match v {
        crate::Chiffre::Texte(t) => t.trim().replace(',', ".").parse::<f64>().map_err(|_| socle::num::illisible(t))?,
        nombre => nombre.flottant()?,
    };
    let jours = lu.ceil() as i64;
    let idcc = ctx.entreprise.idcc.texte().to_string();
    let artiste = sal.statut.dite() == Some(&Statut::Artiste);
    let plafond = droit::essai::plafond(nat, &eng.debut, eng.fin.as_ref(), sal.statut_conventionnel.dite().map(|s| s.as_str()), artiste, &idcc)?;
    droit::essai::refuser_au_dela(jours, plafond.as_ref(), &format!("{} ({})", crate::qui_de(sal)?, nat.libelle))
}

/// La paie d'un engagement sur sa periode (la periode de paie, ou une periode elementaire), dans l'ordre du §7. La
/// configuration de la compagnie se lit a la fin de cette periode (`Contexte::au`).
/// Les cles de `Paie::bases` qui portent, un mois d'absence, les deux termes du rapport de `smic_d_une_absence` : la DSN
/// les declare en S21.G00.51, types 028 et 029.
pub const PERCUE_HORS_ENA: &str = "remuneration_percue_hors_ena";
pub const HABITUELLE_HORS_ENA: &str = "remuneration_habituelle_hors_ena";

/// Le SMIC de la reduction generale d'un mois ou une absence retire du salaire.
pub(crate) struct SmicDUneAbsence {
    /// La remuneration percue hors elements non affectes par l'absence (S21.G00.51.011 = 028).
    pub percue: Decimal,
    /// La remuneration habituelle du mois complet hors ces elements (S21.G00.51.011 = 029).
    pub habituelle: Decimal,
    /// Les heures dont le SMIC horaire fait le SMIC du mois, quand le bareme dit le SMIC horaire de la reduction.
    pub heures: Option<Decimal>,
}

/// CSS D. 241-7, IV, dernier alinea (`atelier/reference/sources/code-de-la-securite-sociale/xml/D241-7.xml`) : quand le
/// contrat est suspendu sans paiement de la remuneration ou avec un paiement partiel, « la fraction du montant du salaire
/// minimum a retenir pour le mois ou a lieu l'absence est corrigee selon le rapport entre les revenus d'activite [...] dus
/// et ceux qui auraient ete dus si le salarie avait ete present tout le mois, hors elements de remuneration qui ne sont pas
/// affectes par l'absence », l'affectation s'entendant « d'une proratisation strictement proportionnelle au temps
/// d'absence » ; un paiement integral garde le SMIC entier (alinea precedent). La fiche net-entreprises 2681
/// (`atelier/reference/sources/net-entreprises-base-connaissances/net-entreprises-fiche2681-reduction-generale-absence-
/// proratisation-smic-types-028-029-20260609.txt`, exemples « Prime d'anciennete et maladie » et « garantie sur le net »)
/// met au numerateur le salaire, l'absence, le maintien et les IJSS brutes deduites, hors IJSS subrogees non soumises, et
/// au denominateur le salaire du mois complet ; « SMIC = 1766.92 x 1 104.27 / 1 820.04 ».
///
/// Dans ce calcul, une absence ne proratise que le salaire de base : les lignes `retabli` (l'absence, le maintien, les IJSS
/// brutes) se calculent sur lui (`maintien`, `imprevus`). Le numerateur est donc le salaire de base et ces lignes, le
/// denominateur le salaire de base. Le SMIC du mois du salaire de base (`smic_horaire` x ses heures, au centime) prend le
/// rapport, comme la fiche l'ecrit ; les autres heures payees (en plus du mois) restent entieres. `heures` est ce SMIC
/// rapporte au SMIC horaire : la reduction le retrouve (`droit::reduction`). Sans ligne `retabli` non nulle : aucun rapport.
pub(crate) fn smic_d_une_absence(lb: &[LigneBrut], smic_horaire: Option<Decimal>) -> Option<SmicDUneAbsence> {
    let somme = |f: &dyn Fn(&LigneBrut) -> bool, v: &dyn Fn(&LigneBrut) -> Decimal| lb.iter().filter(|l| f(l)).fold(Decimal::ZERO, |a, l| a + v(l));
    if !lb.iter().any(|l| l.retabli && !l.montant.is_zero()) {
        return None;
    }
    let habituelle = somme(&|l| l.salaire_de_base, &|l| l.montant);
    if habituelle <= Decimal::ZERO {
        return None;
    }
    let percue = Decimal::ZERO.max(habituelle + somme(&|l| l.retabli && l.soumis, &|l| l.montant));
    let heures = smic_horaire.filter(|s| *s > Decimal::ZERO).map(|s| {
        let smic_du_salaire = eur(mul(s, somme(&|l| l.salaire_de_base, &|l| l.heures_annexes)));
        let autres = somme(&|l| !l.salaire_de_base && !l.retabli, &|l| l.heures_annexes);
        Decimal::ZERO.max(div(div(mul(smic_du_salaire, percue), habituelle), s) + autres)
    });
    Some(SmicDUneAbsence { percue, habituelle, heures })
}

pub(crate) fn calculer_periode(ctx: &Contexte, eng: &Engagement, precedentes: &[Paie]) -> Resultat<Paie> {
    let nat = eng.nat()?;
    let mut avis_dfs: Vec<String> = Vec::new();
    let fin_periode = eng.fin_periode()?;
    let ctx = &ctx.au(&fin_periode)?;
    let sal = fiche_de_l_engagement(ctx, eng)?;
    essai_dans_son_plafond(ctx, eng, &nat, &sal)?;
    let spec = if nat.mensuelle() {
        None
    } else {
        Some(ctx.spectacle(eng.spectacle.as_deref().ok_or_else(|| socle::fichiers::absent("spectacle"))?)?)
    };
    // Le contrat est-il declare a la caisse des Conges Spectacles ? (`caisse_conges::affilie` : un CDDU de moins de
    // douze mois, un CDD de droit commun de moins de douze mois d'un artiste.)
    let caisse = droit::caisse_conges::affilie(&nat, &sal, &eng.debut, eng.fin.as_ref())?;
    // Le contrat ouvre-t-il la contribution CPF-CDD (C. trav. L. 6331-6) ? Un CDD de droit commun, ni saisonnier,
    // ni contrat aide (`droit::cpf_cdd`, la regle du bloc 44).
    let cpf_cdd = droit::cpf_cdd::contrat_ouvre_cpf_cdd(
        &nat,
        eng.motif_recours.as_deref(),
        &droit::cpf_cdd::le_contrat(eng.numero.as_deref(), &eng.salarie),
    )?;
    let majoration = droit::majoration_chomage::due(&nat, eng.motif_recours.as_deref(), &eng.debut, eng.fin.as_ref());
    let bar = bareme_du_salarie(ctx, &sal, &nat, &fin_periode, caisse, cpf_cdd, majoration)?;
    let rubriques_bulletin = &bar.rubriques_bulletin;
    let jours = Decimal::from(eng.jours);

    // 7.2 : les lignes de remuneration.
    let lb = lignes_brut(eng, ctx)?;
    let soumises: Vec<&LigneBrut> = lb.iter().filter(|l| l.soumis).collect();
    if caisse {
        // La caisse paie ses conges (D. 7121-41) : l'employeur ne les paie pas en plus.
        if let Some(l) = lb.iter().find(|l| matches!(l.code.as_str(), "absence_conges_payes" | "indemnite_conges_payes" | "indemnite_compensatrice_conges")) {
            return Err(socle::echec::avec("Cette ligne paie des congés à un salarié déclaré à la caisse des Congés Spectacles : c'est la caisse qui les lui verse ; retirer la ligne.", crate::Detail::du(None, format!(
                "{} : la ligne << {} >> paie des conges a un salarie que l'employeur declare a la caisse des \
Conges Spectacles (code du travail D. 7121-41 ; `caisse_conges::affilie`) : c'est la caisse qui les lui verse. Les payer aussi au bulletin serait les payer deux fois. Refus.",
                eng.qui(),
                l.code
            ))));
        }
    }
    let rubriques: Vec<Decimal> = soumises.iter().map(|l| l.montant).collect();
    let brut = rubriques.iter().fold(Decimal::new(0, 2), |a, b| a + *b);

    // 7.3 : la prevoyance dans l'assiette CSG.
    let (prev, taux_prev) = match bar.rubrique("prevoyance") {
        Some(p @ Rubrique { taux_pat: Some(t), .. }) => (p, dec_f64(*t)?),
        _ => {
            return Err(socle::echec::avec("Le barème de l'année n'a pas de prévoyance avec un taux patronal : le signaler au développeur.", crate::Detail::du(None, format!(
                "Bareme {}{} : aucune rubrique `prevoyance` avec un taux \
                 patronal. Elle entre dans l'assiette CSG (98,25 % du brut + \
                 part patronale de prevoyance) : sans elle la CSG serait \
                 sous-declaree sans que rien ne le signale.",
                bar.annee,
                if nat.bareme().is_some() { format!(", nature '{}'", nat.cle) } else { String::new() }
            ))))
        }
    };
    let plafond_journalier = bar.plafond_journalier.filter(|p| !p.is_zero());
    let t1_du_contrat = |p: Decimal| -> Resultat<Decimal> { Ok(eur(mul(p, jours))) };
    let mut plafond_prev = None;
    if prev.base.as_deref() == Some("t1_journaliere") {
        if let Some(p) = plafond_journalier {
            plafond_prev = Some(t1_du_contrat(p)?);
        }
    }
    let mut pss_prev = None;
    if nat.mensuelle() {
        // Le plafond de la periode, le meme partout (`plafond_de_l_engagement`) : corrige a la quotite d'un temps
        // partiel, sauf renonciation de l'etablissement ou artiste (`plafond_temps_partiel`).
        let p = plafond_de_l_engagement(ctx, eng, &sal)?;
        pss_prev = Some(p);
        if prev.base.as_deref() == Some("t1_mensuelle") {
            plafond_prev = Some(p);
        }
    }
    // Les autres lignes de prevoyance, sur une tranche 2 : leur part patronale entre aussi dans l'assiette.
    let mut tranches_prev = Decimal::new(0, 2);
    for r in rubriques_bulletin {
        let (id, Some(taux)) = (&r.id, part_patronale(r)) else { continue };
        if id == "prevoyance" || !id.starts_with("prevoyance") {
            continue;
        }
        let base = base_de(r)?;
        let t1 = if base == "t2_prevoyance_journaliere" && plafond_journalier.is_some() {
            Some(t1_du_contrat(plafond_journalier.unwrap())?)
        } else if base == "t2_conventionnelle" {
            pss_prev
        } else {
            None
        };
        let Some(t1) = t1 else {
            return Err(socle::echec::avec("Le barème de l'année porte une prévoyance que le calcul de la CSG ne sait pas lire : le signaler au développeur.", crate::Detail::du(None, format!(
                "Bareme {} : la ligne de prevoyance `{}` porte l'assiette \
                 `{}`, que le calcul de l'assiette CSG ne sait pas lire. Sa \
                 part patronale doit y entrer : sans elle la CSG serait \
                 sous-declaree.",
                bar.annee, id, base
            ))));
        };
        let base_t2 = Decimal::new(0, 2).max(brut.min(Decimal::from(4) * t1) - t1);
        tranches_prev = add(tranches_prev, div(mul(base_t2, dec_f64(taux)?), Decimal::ONE_HUNDRED));
    }
    // La part patronale de la mutuelle : un forfait sur le plafond MENSUEL de la periode, sans la correction du temps
    // partiel (`plafond_de_la_sante`).
    let mutuelle = bar.rubrique("mutuelle").and_then(part_patronale);
    let pss_sante = if nat.mensuelle() { Some(plafond_de_la_sante(ctx, eng)?) } else { None };
    if let (Some(m), Some(pss)) = (mutuelle, pss_sante) {
        tranches_prev += pct(pss, dec_f64(m)?);
    }

    // 7.4 : les assiettes, par rubrique de salaire.
    let (abattement, maj115, maj110) = (bar.abattement_csg, bar.majoration_conges_115, bar.majoration_conges_110);
    let assiettes = |montant: Decimal, cs: bool| -> Assiettes {
        let mut base_prev = montant;
        if let Some(pp) = plafond_prev {
            if brut > Decimal::ZERO {
                base_prev = div(mul(brut.min(pp), montant), brut);
            }
        }
        let base_prev_t2 = if !tranches_prev.is_zero() && brut > Decimal::ZERO {
            div(mul(tranches_prev, montant), brut)
        } else {
            Decimal::ZERO
        };
        Assiettes {
            brut: montant,
            brut_cs: if cs { montant } else { Decimal::new(0, 2) },
            csg: eur(add(add(mul(montant, abattement), div(mul(base_prev, taux_prev), Decimal::ONE_HUNDRED)), base_prev_t2)),
            x115: eur(mul(montant, maj115)),
            x110: eur(mul(montant, maj110)),
            dfs: None,
            x115_dfs: None,
            x110_dfs: None,
        }
    };
    // L'assiette des Conges Spectacles : les rubriques que le catalogue y met, d'un contrat que l'employeur declare a la
    // caisse -- et d'aucun autre (un CDI, un contrat de douze mois ou plus : `caisse_conges::affilie`).
    let mut par_rubrique: Vec<Assiettes> = soumises.iter().map(|l| assiettes(l.montant, l.conges_spectacles && caisse)).collect();
    // UN MOIS SANS SALAIRE OU LA PART PATRONALE RESTE DUE (une sante maintenue pendant une suspension non indemnisee :
    // `sante_de_la_suspension`, « maintenue »). Les contributions de l'employeur aux garanties collectives entrent dans
    // l'assiette de la CSG (CSS L136-1-1, I, « toutes les sommes [...] dues [...] a l'occasion d'un travail », dont le III, 2°,
    // n'exclut pas les contributions aux garanties collectives : `atelier/reference/sources/suspension-non-remuneree/legi-extrait-code-de-la-
    // securite-sociale-20261015.md`) ; elles se repartissent au prorata du brut de chaque rubrique, et un brut nul ne les
    // repartit nulle part : elles vont entieres a la premiere rubrique soumise, plutot que de sortir de l'assiette.
    if brut.is_zero() && !tranches_prev.is_zero() {
        if let Some(premiere) = par_rubrique.first_mut() {
            premiere.csg = eur(add(premiere.csg, tranches_prev));
        }
    }
    let zero = Decimal::new(0, 2);
    let base_csg = par_rubrique.iter().fold(zero, |a, b| a + b.csg);

    // 7.5 : le SMIC retenu pour la DSN.
    let smic = ctx.smic(&fin_periode)?;
    let hpc = bar.heures_par_cachet;
    let heures_annexes = lb.iter().fold(Decimal::ZERO, |a, l| a + l.heures_annexes);
    let heures = if nat.mensuelle() { Decimal::ZERO.max(heures_annexes) } else { heures_annexes.min(mul(hpc, jours)) };
    let plafond = eur(mul(dec_f64(smic)?, heures));

    // 7.5 bis : la deduction forfaitaire specifique (arrete du 4 septembre 2025, art. 9 ; `abattement_dfs`). Elle abat
    // l'assiette des lignes que le bareme nomme, rubrique de salaire par rubrique de salaire, apres le plafond annuel
    // et sans descendre sous l'assiette minimum ; les assiettes majorees partent du brut abattu (Acoss 2013-019, R6).
    let mut dfs: Option<DeductionForfaitaire> = None;
    if let Some(o) = deduction_forfaitaire(&bar, &sal, &ctx.entreprise)? {
        dfs_applicable(eng, &nat, &o, &lb, rubriques_bulletin, &ctx.racine, &ctx.lue_au)?;
        let deja = dfs_deja_deduite(ctx, eng, &sal, precedentes)?;
        let montants: Vec<Decimal> = soumises.iter().map(|l| l.montant).collect();
        let a = abattement_dfs(&o, &montants, brut, deja, plafond);
        for (r, abattu) in par_rubrique.iter_mut().zip(&a.assiettes) {
            r.dfs = Some(*abattu);
            r.x115_dfs = Some(eur(mul(*abattu, maj115)));
            r.x110_dfs = Some(eur(mul(*abattu, maj110)));
        }
        avis_dfs.extend(a.avis);
        dfs = Some(DeductionForfaitaire {
            categorie: o.categorie.clone(),
            profession: o.profession.clone(),
            taux: o.taux,
            rubriques: o.rubriques.clone(),
            montant: a.deduction,
            deja,
        });
    }

    // 7.6 : les bases globales.
    let mut plafond_intermittent = None;
    let mut bases: BTreeMap<String, Decimal> = BTreeMap::new();
    let somme_de = |f: &dyn Fn(&Assiettes) -> Decimal| par_rubrique.iter().fold(zero, |a, b| a + f(b));
    bases.insert("brut".into(), brut);
    bases.insert(BRUT_CS.into(), somme_de(&|b| b.brut_cs));
    bases.insert(CSG.into(), base_csg);
    bases.insert(BRUT_X115.into(), somme_de(&|b| b.x115));
    bases.insert("brut_x110".into(), somme_de(&|b| b.x110));
    // Les tranches journalieres d'un intermittent (Audiens, Parametres : « le plafond applicable est le plafond journalier »).
    if let Some(pj) = plafond_journalier.filter(|_| !nat.mensuelle()) {
        let t1 = t1_du_contrat(pj)?;
        bases.insert("t1_journaliere".into(), brut.min(t1));
    }
    // Les tranches d'un cadre intermittent, au-dela de la T1 (`bareme_cadre`).
    if let Some(pj) = plafond_journalier.filter(|_| !nat.mensuelle() && droit::bareme::est_cadre(&sal)) {
        let t1 = t1_du_contrat(pj)?;
        let mult = |m: Option<Decimal>, cle: &str| -> Resultat<Decimal> { Ok(m.ok_or_else(|| socle::fichiers::absent(cle))?.trunc()) };
        let (t2_mult, apec_mult) = (mult(bar.t2_multiple, "t2_multiple")?, mult(bar.apec_multiple, "apec_multiple")?);
        bases.insert("t2_journaliere".into(), zero.max(brut.min(t2_mult * t1) - t1));
        // L'Apec, sur le brut dans la limite de quatre plafonds (Audiens, Parametres ; Agirc-Arrco, aide DSN) : en tout,
        // et au-dela de la T1.
        bases.insert("apec".into(), brut.min(apec_mult * t1));
        bases.insert("apec_t2_journaliere".into(), zero.max(brut.min(apec_mult * t1) - t1));
        bases.insert("t2_prevoyance_journaliere".into(), zero.max(brut.min(Decimal::from(4) * t1) - t1));
        // La CET, due quand le brut depasse la T1, sur T1 + T2 : huit plafonds au plus (Agirc-Arrco, « jusqu'a 8 PSS »).
        bases.insert(CET.into(), if brut > t1 { brut.min(t2_mult * t1) } else { zero });
    }
    if nat.mensuelle() {
        let pss = plafond_de_l_engagement(ctx, eng, &sal)?;
        // `pss_periode` : l'assiette du forfait de sante (rubrique `mutuelle`, base 18 du bloc 79), le plafond de la
        // periode sans la correction du temps partiel (`plafond_de_la_sante`). Les tranches (`t1_mensuelle`...) suivent,
        // elles, le plafond corrige.
        bases.insert("pss_periode".into(), pss_sante.ok_or("le plafond de la santé d'un contrat payé au mois manque : le signaler au développeur")?);
        bases.insert("t1_mensuelle".into(), brut.min(pss));
        // Le FNAL plafonne d'un permanent declare a la caisse (CDD de moins de douze mois d'un
        // artiste) : seul le montant plafonne se majore (lettre circulaire Acoss 2013-019, annexe 1, R2), comme
        // `brut_x115_plafonne` d'un intermittent. Sans majoration (1,0), c'est la tranche 1.
        let x115 = bases[BRUT_X115];
        bases.insert("t1_mensuelle_x115".into(), if brut <= pss { x115 } else { eur(mul(pss, maj115)) });
        bases.insert("t2_mensuelle".into(), zero.max(brut.min(Decimal::from(8) * pss) - pss));
        // Quatre plafonds de la periode : l'Apec d'un cadre, et le chomage et l'AGS d'un permanent. Les contributions
        // d'assurance chomage « sont assises sur les remunerations brutes dans la limite d'un plafond » (C. trav.
        // L. 5422-9), « fixe a quatre fois le plafond mentionne a l'article L. 241-3 du code de la securite sociale »
        // (reglement general annexe a la convention du 15/11/2024, art. 49, al. 2) ; l'AGS suit la meme assiette
        // (L. 3253-18). Le plafond est celui de la periode de paie, reduit au prorata des jours d'emploi quand le contrat
        // ne couvre pas le mois (CSS R. 242-2, al. 3) : `pss`. Textes : atelier/reference/sources/chomage-permanents/.
        let quatre_plafonds = brut.min(Decimal::from(4) * pss);
        bases.insert("apec_mensuelle".into(), quatre_plafonds);
        bases.insert("brut_chomage".into(), quatre_plafonds);
        bases.insert("t2_conventionnelle".into(), zero.max(brut.min(Decimal::from(4) * pss) - pss));
        // La CET des permanents : due au-dela de la T1, sur le brut dans la limite de huit plafonds (Agirc-Arrco, « jusqu'a 8 PSS » ;
        // Audiens, « dans la limite de la T1 + T2 »).
        bases.insert("cet_mensuelle".into(), if brut > pss { brut.min(Decimal::from(8) * pss) } else { zero });
        bases.insert("t1_t2_mensuelle".into(), brut.min(Decimal::from(8) * pss));
    } else {
        // Le plafond d'un intermittent : celui que le bas du bulletin imprime (`plafond_de_l_engagement`), et la seule
        // source. Textes : atelier/reference/sources/plafond-intermittents/LISEZ-MOI.md. La vieillesse plafonnee et le
        // FNAL s'assoient « dans la limite » du plafond de L. 241-3 CSS ; le chomage des annexes VIII et X « dans la
        // limite d'un plafond fixe a quatre fois le plafond mentionne a l'article L. 241-3 » (annexes VIII et X du
        // 15/11/2024, art. 49). Sous le plafond, rien ne bouge : les bases sont le brut, et le brut majore tel qu'il
        // etait.
        let plafond_ss = plafond_de_l_engagement(ctx, eng, &sal)?;
        plafond_intermittent = Some(plafond_ss);
        let x115 = bases[BRUT_X115];
        bases.insert("brut_plafonne".into(), brut.min(plafond_ss));
        // La majoration de 11,5 % (CSS L. 243-1-3, D. 243-0-2) s'applique a l'assiette plafonnee : au-dela du plafond,
        // c'est le plafond qu'elle majore (lettre circulaire Acoss 2013-019, annexe 1, R2 ;
        // atelier/reference/sources/majoration-caisses-conges/LISEZ-MOI.md).
        bases.insert("brut_x115_plafonne".into(), if brut <= plafond_ss { x115 } else { eur(mul(plafond_ss, maj115)) });
        bases.insert("brut_chomage".into(), brut.min(Decimal::from(4) * plafond_ss));
    }
    if dfs.is_some() {
        let s = par_rubrique.iter().filter_map(|b| b.dfs).fold(zero, |a, x| a + x);
        bases.insert(BRUT_DFS.into(), s);
        let x115 = par_rubrique.iter().filter_map(|b| b.x115_dfs).fold(zero, |a, x| a + x);
        bases.insert("brut_x115_dfs".into(), x115);
        bases.insert("brut_x110_dfs".into(), par_rubrique.iter().filter_map(|b| b.x110_dfs).fold(zero, |a, x| a + x));
        if let Some(plafond_ss) = plafond_intermittent {
            // Le plafond limite l'assiette abattue : la vieillesse plafonnee et la base 02 sur le brut abattu dans la
            // limite du plafond du contrat (V1n l'avait laisse sur le brut entier).
            bases.insert("brut_plafonne_dfs".into(), s.min(plafond_ss));
            bases.insert("brut_x115_plafonne_dfs".into(), if s <= plafond_ss { x115 } else { eur(mul(plafond_ss, maj115)) });
        }
    }

    // 7.7 : la boucle des cotisations.
    let mut lignes: Vec<LigneCotisation> = Vec::new();
    let (mut tot_pat, mut tot_sal) = (zero, zero);
    for r in rubriques_bulletin {
        let id = r.id.clone();
        let mut cle_base = base_de(r)?.to_string();
        if let Some(d) = &dfs {
            // Une rubrique abattue s'assoit sur le brut abattu ; ses assiettes majorees aussi (Acoss 2013-019, R6).
            if d.rubriques.contains(&id) {
                if let Some(abattue) = match cle_base.as_str() {
                    "brut" => Some("brut_dfs"),
                    "brut_x115" => Some("brut_x115_dfs"),
                    "brut_x110" => Some("brut_x110_dfs"),
                    "brut_plafonne" if bases.contains_key("brut_plafonne_dfs") => Some("brut_plafonne_dfs"),
                    "brut_x115_plafonne" if bases.contains_key("brut_x115_plafonne_dfs") => Some("brut_x115_plafonne_dfs"),
                    _ => None,
                } {
                    cle_base = abattue.into();
                }
            }
        }
        let base = *bases.get(&cle_base).ok_or_else(|| socle::fichiers::absent(&cle_base))?;
        let assiettes_r: Vec<Decimal> = if GLOBALES.contains(&cle_base.as_str()) {
            vec![base]
        } else {
            par_rubrique
                .iter()
                .map(|b| b.lire(&cle_base).ok_or_else(|| socle::fichiers::absent(&cle_base)))
                .collect::<Resultat<_>>()?
        };
        let (mut m_pat, mut m_sal) = (zero, zero);
        let (mut taux_pat, mut taux_sal) = (r.taux_pat, r.taux_sal);
        let non_vide = |c: &Vec<f64>| Some(c.clone()).filter(|c| !c.is_empty());
        let (mut comp_pat, mut comp_sal) = (non_vide(&r.composants_pat), non_vide(&r.composants_sal));
        let manque = || "taux absent (taux_pat ou taux_sal) : le complément ne s'ajoute à rien".to_string();
        // Le complement au-dela d'un seuil : le taux reduit, plus le complement.
        if let Some(compl) = &r.complement {
            if au_dela_du_seuil(bar.reduction_generale.as_ref(), brut, Decimal::ZERO.max(heures_annexes), compl.seuil_smic) {
                taux_pat = Some(round3(taux_pat.ok_or_else(manque)? + compl.taux));
            }
        }
        // La CET des cadres, fondue dans T1 et T2 : l'arrondi se fait en deux parts.
        if let Some(cet) = &r.cet_si_depasse {
            if bases.get(CET).is_some_and(|c| !c.is_zero()) {
                let (cet_pat, cet_sal) = (cet.pat, cet.sal);
                let mut cp = match comp_pat.take() {
                    Some(c) => c,
                    None => vec![taux_pat.ok_or_else(manque)?],
                };
                *cp.last_mut().unwrap() += cet_pat;
                let mut cs = match comp_sal.take() {
                    Some(c) => c,
                    None => vec![taux_sal.ok_or_else(manque)?],
                };
                *cs.last_mut().unwrap() += cet_sal;
                comp_pat = Some(cp);
                comp_sal = Some(cs);
                taux_pat = Some(round3(taux_pat.ok_or_else(manque)? + cet_pat));
                taux_sal = Some(round3(taux_sal.ok_or_else(manque)? + cet_sal));
            }
        }
        let composants = non_vide(&r.composants);
        for a in &assiettes_r {
            if let Some(c) = &composants {
                m_pat += somme_pct(*a, c)?;
            } else if let Some(c) = &comp_pat {
                m_pat += somme_pct(*a, c)?;
            } else if let Some(t) = taux_pat {
                m_pat += pct(*a, dec_f64(t)?);
            }
            if let Some(c) = &comp_sal {
                m_sal += somme_pct(*a, c)?;
            } else if let Some(t) = taux_sal {
                m_sal += pct(*a, dec_f64(t)?);
            }
        }
        lignes.push(LigneCotisation {
            id,
            famille: r.famille.clone().ok_or_else(|| socle::fichiers::absent("famille"))?,
            libelle: r.libelle.clone().ok_or_else(|| socle::fichiers::absent("libelle"))?,
            base,
            taux_pat,
            montant_pat: m_pat,
            taux_sal,
            montant_sal: m_sal,
            masquer_base_et_taux: r.masquer_base_et_taux,
        });
        tot_pat += m_pat;
        tot_sal += m_sal;
    }
    lignes.retain(|l| {
        !(l.base.is_zero() && l.montant_pat.is_zero() && l.montant_sal.is_zero() && (l.id == "retraite_t2" || l.id == "apec_t2"))
    });

    // 7.9 : la reduction generale, apres les cotisations.
    let statut = sal.statut.dite().map(|s| s.as_str());
    let caisse_conges = bases[BRUT_CS] > Decimal::ZERO;
    // Sous DFS, la reduction se calcule sur le brut abattu, dans la limite de 130 % de celle du brut entier
    // (`reduction_generale_sous_dfs`).
    // Le SMIC horaire de la reduction (`smic_horaire_reference`) ; sans lui, pas de reduction.
    let smic_horaire_rg = bar.reduction_generale.as_ref().and_then(|r| r.smic_horaire_reference).filter(|s| !s.is_zero());
    let absence = smic_d_une_absence(&lb, smic_horaire_rg);
    let heures_rg = match absence.as_ref().and_then(|a| a.heures) {
        Some(h) => h,
        None => heures_annexes,
    };
    if let Some(a) = &absence {
        bases.insert(PERCUE_HORS_ENA.into(), a.percue);
        bases.insert(HABITUELLE_HORS_ENA.into(), a.habituelle);
    }
    let rg = match bases.get(BRUT_DFS).filter(|_| dfs.is_some()) {
        Some(abattu) => reduction_generale_sous_dfs(bar.reduction_generale.as_ref(), statut, brut, *abattu, heures_rg, caisse_conges)?,
        None => reduction_generale(bar.reduction_generale.as_ref(), statut, brut, heures_rg, caisse_conges)?,
    };
    // Le SMIC se calcule sur les heures exactes du rapport ; le bulletin les dit au centieme.
    let rg = rg.map(|mut r| {
        if absence.as_ref().is_some_and(|a| a.heures.is_some()) {
            r.heures = r.heures.round_dp(2);
        }
        r
    });
    if let Some(rg) = rg.as_ref().filter(|rg| !rg.montant.is_zero()) {
        lignes.push(ligne_de_reduction(rg, brut));
        tot_pat -= rg.montant;
    }

    // 7.9 bis : les heures en plus d'un salarie paye au mois (`heures_en_plus`) : la reduction de cotisations salariales,
    // la deduction patronale, et le net exonere d'impot, que la remuneration nette fiscale ne prend pas.
    let mois_paye = fin_periode.mois_civil().to_string();
    let cle_sal = sal.id.to_string();
    let heures_en_plus = crate::heures_en_plus::apres_les_cotisations(ctx, &bar, &lb, &lignes, brut, tot_pat, || {
        crate::conges::exonere_des_mois_anterieurs(ctx, &mois_paye, &cle_sal)
    })?;
    let mut exonere = zero;
    if let Some(h) = &heures_en_plus {
        lignes.extend(h.lignes.iter().cloned());
        tot_sal -= h.reduction;
        tot_pat -= h.deduction;
        exonere = h.exonere;
        bases.insert(crate::heures_en_plus::REMUNERATION.into(), h.remuneration);
        bases.insert(crate::heures_en_plus::HEURES.into(), h.heures);
        bases.insert(crate::heures_en_plus::EXONERE.into(), h.exonere);
    }
    // Les indemnites journalieres que la compagnie subrogee reverse (`maintien::lignes`) : le montant net social les
    // compte, la DSN les dit en S21.G00.58 de type 10, et le prelevement a la source les prend pour assiette, a cote du
    // net imposable (`etats::prelevement`).
    for (l, b) in eng.lignes.iter().zip(lb.iter()) {
        if b.code == crate::maintien::IJ_NETTES {
            *bases.entry(crate::maintien::IJ_SUBROGEES.into()).or_insert(zero) += b.montant;
            *bases.entry(crate::maintien::IJ_ASSIETTE_PAS.into()).or_insert(zero) += l.assiette_pas().ok_or_else(|| socle::fichiers::absent("assiette_pas"))?;
        }
    }

    // 7.10 : la sortie. Ce qui est hors assiette se paie ou se retient quand meme.
    let hors_assiette = lb.iter().filter(|l| !l.soumis).fold(zero, |a, l| a + l.montant);
    let net = brut - tot_sal + hors_assiette;
    // La remuneration nette fiscale (S21.G00.50.002) part du brut soumis, deduit les cotisations salariales et
    // reintegre la CSG/CRDS non deductible et la sante patronale (`reintegrations`). Des lignes hors assiette, elle ne
    // prend que celles que le catalogue dit imposables : la norme (2026.1, § 2.3.2, p. 60-61) fait « DEDUIRE les sommes
    // versees a titre de remboursement de frais professionnels » et « AJOUTER les indemnites imposables a l'impot sur
    // le revenu ». Un defraiement, une indemnite exoneree (CGI 80 duodecies) n'y entrent pas ; un acompte non plus : il
    // avance le net du mois, il ne change pas le revenu.
    let hors_assiette_imposable = lb.iter().filter(|l| !l.soumis && l.imposable).fold(zero, |a, l| a + l.montant);
    // Le montant net social (S21.G00.58.004, type 03). C. trav. R. 3243-1, 9° bis, qui renvoie au CSS R. 844-1, II
    // (redaction du decret n° 2023-1378, que le decret n° 2026-535 n'a pas touchee) ; page du BOSS « Montant net
    // social » du 01/10/2026, II.A.1 et Q 20, 27 (textes : atelier/reference/sources/montant-net-social/LISEZ-MOI.md). La
    // totalite des sommes versees en contrepartie ou a l'occasion du travail, exonerees ou non, moins les cotisations et
    // contributions sociales du salarie (`tot_sal`, parts salariales de sante et de prevoyance comprises). Le
    // financement patronal des garanties collectives (sante, prevoyance, Fonds sante des intermittents) n'y entre pas :
    // R. 844-1, II, 1° l'excepte. La page du BOSS l'y range (II.A.1, Q 13, 14, 29) sans texte qui l'appuie ; le decret
    // prime (meme LISEZ-MOI, derniere section). Des lignes hors assiette, seules
    // celles qui sont un revenu (indemnites, imposables ou non) ; ni remboursement de frais dans ses limites, ni
    // mouvement de tresorerie (acompte) : la nature de la rubrique le dit, et une nature qui ne le dit pas refuse
    // (`Rubrique::hors_assiette`). Pas de prelevement a la source : un net SOCIAL.
    let mut hors_assiette_revenu = zero;
    for l in lb.iter().filter(|l| !l.soumis) {
        if rubrique(&ctx.racine, &l.code, &ctx.lue_au)?.hors_assiette()? == HorsAssiette::Revenu {
            hors_assiette_revenu += l.montant;
        }
    }
    lignes.iter().find(|l| l.id == "csg_crds_nd").ok_or("StopIteration")?;
    let (base_51, retabli, chomage_51) = if nat.remunerations_au_brut() {
        (brut, brut, brut)
    } else {
        let base_51 = lb.iter().filter(|l| l.salaire_de_base).fold(zero, |a, l| a + l.montant);
        // S21.G00.51.011 = 003, « Salaire rétabli – reconstitué », qui « doit toujours être déclaré pour la période de paie
        // courante dans la DSN mensuelle » (cahier technique 2026.1, bloc S21.G00.51, Type :
        // `atelier/reference/norme-dsn/texte/bloc-S21.G00.51.txt`) : le salaire RECONSTITUE, le brut sans les absences qui
        // l'ont reduit (les rubriques `retabli`). Un mois entierement suspendu declare donc 001 a 0.00 et 003 au salaire du
        // mois.
        let retabli = brut - lb.iter().filter(|l| l.retabli).fold(zero, |a, l| a + l.montant);
        let chomage = brut - lb.iter().filter(|l| l.dsn_bloc52.is_some() && l.soumis).fold(zero, |a, l| a + l.montant);
        (base_51, retabli, chomage)
    };
    let tp = plafond_temps_partiel(&ctx.entreprise, eng, &sal)?;
    let mut paie = Paie::nouveau(eng.clone(), sal, spec, brut, base_csg, plafond, smic, heures, tp);
    let p = paie.ecrire();
    p.heures_absence = -lb.iter().filter(|l| l.heures_annexes < Decimal::ZERO).fold(Decimal::ZERO, |a, l| a + l.heures_annexes);
    p.lignes = lignes;
    p.total_pat = tot_pat;
    p.total_sal = tot_sal;
    p.net_a_payer = net;
    p.net_social = brut - tot_sal + hors_assiette_revenu;
    p.net_imposable = brut - tot_sal + hors_assiette_imposable + reintegrations(&p.lignes) - exonere;
    p.cout_employeur = brut + tot_pat;
    p.bases = bases;
    p.reduction_generale = rg;
    p.deduction_forfaitaire = dfs;
    p.avertissements.extend(avis_dfs.into_iter().map(|texte| Avis { info: true, texte }));
    if let Some(h) = heures_en_plus {
        p.avertissements.extend(h.avis.into_iter().map(|texte| Avis { info: false, texte }));
    }
    p.salaire_de_base = base_51;
    p.salaire_retabli = retabli;
    p.assiette_chomage = chomage_51;
    p.heures_travaillees = Decimal::ZERO.max(heures_annexes);
    p.remuneration = lb;
    Ok(paie)
}

/// Le plafond de securite sociale d'un engagement (`plafond::pour_paie`) : celui que le bas du bulletin imprime, et
/// celui sur lequel s'assoient la vieillesse plafonnee, le FNAL et le chomage d'un intermittent (`calculer`, 7.6).
/// `sal` : la fiche du salarie telle que la paie la lit (`emploi_de`), pour son statut : l'arrete du 24/01/1975 ne
/// vise que les artistes ; un technicien, comme un artiste engage cinq jours ou plus, a le plafond proratise sur les
/// jours remuneres de la paie (BOSS, assiette generale, § 1090, `atelier/reference/sources/boss/`). Textes :
/// atelier/reference/sources/plafond-intermittents/LISEZ-MOI.md.
pub(crate) fn plafond_de_l_engagement(ctx: &Contexte, e: &Engagement, sal: &ChampsSalarie) -> Resultat<Decimal> {
    let nat = e.nat()?;
    let artiste = match statut_de(sal)? {
        "artiste" => true,
        "technicien" => false,
        autre if !nat.mensuelle() => {
            return Err(format!(
                "Statut << {} >> : le plafond de securite sociale d'un intermittent depend de son statut (artiste : \
                 arrete du 24/01/1975, art. 3 ; technicien : R. 242-2 CSS), et ce statut n'est ni l'un ni l'autre.",
                autre))
        }
        _ => false,
    };
    if nat.mensuelle() {
        let tp = match plafond_temps_partiel(&ctx.entreprise, e, sal)? {
            PlafondTempsPartiel::Proratise => Some(duree_du_temps_partiel(e)?),
            _ => None,
        };
        return plafond_permanent(tuple(&e.debut_periode()?), tuple(&e.fin_periode()?), tp, &pss_du_contexte(ctx));
    }
    let ep = EngagementPlafond {
        mensuelle: nat.mensuelle(),
        artiste,
        debut_periode: tuple(&e.debut_periode()?),
        fin_periode: tuple(&e.fin_periode()?),
        debut: tuple(&e.debut),
        // Un contrat paye a l'engagement a son terme (`Engagement::fin` : `None` seulement pour un permanent ouvert).
        fin: tuple(&e.fin.ok_or_else(|| socle::fichiers::absent("fin"))?),
        dates_travail: e.dates_travail.iter().map(tuple).collect(),
        jours: e.jours,
    };
    pour_paie(&ep, &pss_du_contexte(ctx))
}

/// Le plafond sur lequel se pose le forfait de sante d'un salarie au mois (`pss_periode`, la rubrique `mutuelle` et la
/// base 18 du bloc 79) : celui de la periode de paie, reduit au prorata des jours quand le contrat ne la couvre pas
/// (`plafond::plafond_permanent`), mais SANS la correction du temps partiel.
///
/// LES TEXTES. Les accords de sante fixent une cotisation « au titre de la couverture du seul salarie » en pourcentage
/// du « plafond mensuel de la securite sociale » (IDCC 1285, accord du 30/01/2026, art. 6 ; IDCC 3090, accord du
/// 19/07/2024, art. 13 : `atelier/reference/sources/kali-1285-accord-frais-de-sante-20260130-20261011.md`,
/// `kali-3090-accord-sante-permanents-20240719-20261011.md`) : c'est la valeur mensuelle que l'arrete fixe chaque annee
/// (CSS L. 241-3, D. 242-17, D. 242-18). La correction du temps partiel est autre chose : R. 242-2, I, dernier alinea,
/// corrige « le plafond » retenu pour le calcul des cotisations plafonnees du salarie, a la duree de son contrat ; un
/// forfait n'est pas plafonne. Et les memes accords, apres CSS R. 242-1-6, 2°, c, ouvrent une dispense aux « salaries a
/// temps partiel [...] dont l'adhesion au systeme de garanties les conduirait a s'acquitter d'une cotisation au moins
/// egale a 10 % de leur remuneration brute » : la cotisation d'un temps partiel ne baisse donc pas avec son temps de
/// travail. Le prorata des jours d'un contrat qui ne couvre pas le mois reste une INFERENCE plus ancienne (V3e), qu'aucun
/// de ces textes ne tranche. La prevoyance, elle, porte sur des tranches : « la tranche 1 porte sur la remuneration egale
/// au plafond de la securite sociale vise a l'article L. 241-3 » (IDCC 1285, titre XII, `kali-1285-titres-ii-iii-x-xi-
/// xii-20260923.md`, l. 73), le plafond que R. 242-2 corrige ; elle suit le plafond corrige, comme la retraite
/// complementaire (`plafond_de_l_engagement`). Textes : `atelier/reference/sources/plafond-temps-partiel/LISEZ-MOI.md`.
pub(crate) fn plafond_de_la_sante(ctx: &Contexte, e: &Engagement) -> Resultat<Decimal> {
    plafond_permanent(tuple(&e.debut_periode()?), tuple(&e.fin_periode()?), None, &pss_du_contexte(ctx))
}

/// Le plafond d'un salarie a temps partiel, et ce que S21.G00.40.084 en declare.
///
/// LA PRORATISATION EST LA REGLE, Y RENONCER EST LE CHOIX.
/// - CSS L. 242-8 : pour un salarie a temps partiel (L. 3123-1 C. trav.), « il est opere un abattement d'assiette »
///   destine a compenser l'ecart avec un temps complet -- un imperatif ; R. 242-2, I, dernier alinea, en donne la
///   mesure (`plafond::plafond_permanent`) ;
/// - fiche net-entreprises 3369 (03/04/2026) : « Par defaut, le salarie a temps partiel cotise uniquement a hauteur de
///   la quotite de travail de son contrat. L'employeur peut choisir de ne pas proratiser le plafond de Securite sociale
///   pour tous les salaries a temps partiel de son etablissement » ; BOSS, assiette generale, § 760 : « Le salarie a
///   temps partiel et l'employeur ont la possibilite de renoncer a cet ajustement » ;
/// - L. 242-10, 1° : L. 242-8 « n'est pas applicable » aux salaries dont l'emploi donne lieu a des taux, assiettes ou
///   montants specifiques ; BOSS § 1090 : ils « ne sont pas eligibles a la reduction du plafond », et « sont notamment
///   vises les artistes du spectacle ».
/// Textes : `atelier/reference/sources/plafond-temps-partiel/LISEZ-MOI.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlafondTempsPartiel {
    /// Un intermittent (contrat d'usage, sans duree du travail : 40.014 = 99), ou un temps plein : la question ne se
    /// pose pas, le plafond n'est pas corrige a la quotite, et 40.084 ne s'ecrit pas (norme 2027.1.2 : conditionnelle).
    NonConcerne,
    /// Temps partiel : le plafond se corrige a la duree du contrat (la regle).
    Proratise,
    /// Temps partiel, et l'etablissement a renonce a la proratisation (la fiche entreprise le dit) : le plafond entier.
    Entier,
    /// Temps partiel d'un artiste du spectacle : exclu de la proratisation (L. 242-10, 1° ; BOSS § 1090), le plafond
    /// entier quel que soit le choix de l'etablissement.
    Exclu,
}

impl PlafondTempsPartiel {
    const CODES: [(PlafondTempsPartiel, &'static str); 4] = [
        (PlafondTempsPartiel::NonConcerne, "non_concerne"),
        (PlafondTempsPartiel::Proratise, "proratise"),
        (PlafondTempsPartiel::Entier, "entier"),
        (PlafondTempsPartiel::Exclu, "exclu"),
    ];

    /// Sa forme ecrite dans une paie (`relire::PLAFOND_TEMPS_PARTIEL`).
    pub fn code(self) -> &'static str {
        match self {
            PlafondTempsPartiel::NonConcerne => Self::CODES[0].1,
            PlafondTempsPartiel::Proratise => Self::CODES[1].1,
            PlafondTempsPartiel::Entier => Self::CODES[2].1,
            PlafondTempsPartiel::Exclu => Self::CODES[3].1,
        }
    }

    /// Depuis sa forme ecrite ; une autre valeur est une paie illisible.
    pub(crate) fn lire(code: &str) -> Resultat<PlafondTempsPartiel> {
        Self::CODES
            .iter()
            .find(|(_, c)| *c == code)
            .map(|(p, _)| *p)
            .ok_or_else(|| socle::echec::a_la_cle(socle::echec::PAIE_ILLISIBLE, "plafond_temps_partiel", code))
    }
}

/// Le plafond du salarie `sal` (sa fiche telle que la paie la lit, pour son statut) a temps partiel de l'engagement `e`,
/// d'apres la fiche entreprise `ent` (`plafond_temps_partiel`). Le temps partiel est celui que la DSN declare en
/// S21.G00.40.014 (20) : une duree du contrat inferieure a la duree de reference (L. 3123-1 du code du travail,
/// `engagement::modalite_temps_travail`). Sans reponse dans la fiche, la regle : proratise (L. 242-8, fiche 3369). Un
/// artiste est exclu (L. 242-10, 1° ; BOSS § 1090) : la reponse de la fiche ne le concerne pas. Le calcul l'ecrit dans
/// la paie (`Contenu::plafond_temps_partiel`), que la DSN lit.
pub(crate) fn plafond_temps_partiel(ent: &ChampsEntreprise, e: &Engagement, sal: &ChampsSalarie) -> Resultat<PlafondTempsPartiel> {
    if !e.nat()?.mensuelle() || e.modalite_temps_travail()? != "20" {
        return Ok(PlafondTempsPartiel::NonConcerne);
    }
    // « Entier » : l'etablissement renonce a la proratisation pour tous ses salaries a temps partiel (fiche 3369 ; BOSS,
    // assiette generale, § 760).
    let renonce = ent.plafond_temps_partiel.dite() == Some(&droit::fiche::PlafondTempsPartiel::Entier);
    if statut_de(sal)? == "artiste" {
        return Ok(PlafondTempsPartiel::Exclu);
    }
    Ok(if renonce { PlafondTempsPartiel::Entier } else { PlafondTempsPartiel::Proratise })
}

/// (duree du contrat, duree de reference) de la correction du plafond d'un temps partiel (R. 242-2, I, dernier alinea) :
/// la duree inscrite au contrat (S21.G00.40.013), rapportee a « la plus courte des durees mentionnees au 1° de
/// l'article L. 3123-1 du code du travail » -- la duree legale (151,67 h, `contrats-base.json`, L. 3121-27) ou, si
/// elle est plus courte, la duree collective de l'entreprise (S21.G00.40.012). Le texte ajoute a la duree du contrat
/// « le nombre d'heures complementaires » du mois : celles que le moteur compose du fait de la carte (`heures_en_plus`) ;
/// une ligne d'heures tapee a la main ne dit pas si elle en est, et refuse plutot que de le deviner.
pub(crate) fn duree_du_temps_partiel(e: &Engagement) -> Resultat<(Decimal, Decimal)> {
    let nat = e.nat()?;
    if let Some(l) = e.lignes.iter().find(|l| l.rubrique() == "heure") {
        return Err(format!(
            "{} : le plafond de Securite sociale de ce salarie a temps partiel se corrige a la duree de son \
contrat << majoree du nombre d'heures complementaires >> du mois (CSS R. 242-2, I, dernier alinea), et la paie porte des \
heures en plus du salaire mensuel ({} h) : heures complementaires ou non, le logiciel ne le sait pas. Refus : il ne les \
devine pas.",
            e.qui(),
            l.quantite().iter().map(Decimal::to_string).collect::<String>()
        ));
    }
    let legale = dec_f64(nat.quotite_reference_defaut().ok_or_else(|| socle::echec::livree("contrats-base.json", &format!("{}.quotite_reference_defaut", nat.libelle)))?)?;
    let reference = match e.quotite_reference()? {
        Some(r) => legale.min(dec_f64(r)?),
        None => legale,
    };
    // « majoree du nombre d'heures complementaires » : celles que le moteur compose du fait du mois (`heures_en_plus`).
    let complementaires = e
        .lignes
        .iter()
        .filter(|l| l.rubrique() == crate::heures_en_plus::HEURE_COMPLEMENTAIRE)
        .map(|l| l.quantite().ok_or_else(|| "nombre attendu, lu null".to_string()))
        .sum::<Resultat<Decimal>>()?;
    Ok((e.quotite()? + complementaires, reference))
}

impl Paie {
    /// Le « Plafond Urssaf » du bas de bulletin.
    pub fn plafond_urssaf(&self, ctx: &Contexte) -> Resultat<Decimal> {
        // Un mois decoupe : la somme des plafonds de ses periodes (BOSS, assiette generale, § 730 : « les differentes
        // regles d'ajustement du plafond sont cumulables »).
        if !self.periodes.is_empty() {
            let mut total = Decimal::new(0, 2);
            for p in &self.periodes {
                total += p.plafond_urssaf(ctx)?;
            }
            return Ok(total);
        }
        plafond_de_l_engagement(ctx, &self.engagement, &self.salarie)
    }

    /// La forme canonique d'une paie, ecriture stable pour les empreintes et les oracles (`ctx` pour le plafond Urssaf) :
    /// chaque avis par son seul texte.
    pub fn canon(&self, ctx: &Contexte) -> Resultat<FormeDePaie<String>> {
        self.forme(ctx, &|a: &Avis| a.texte.clone(), false)
    }

    /// Sa forme (`paie_ecrite::FormeDePaie`), chaque avis par `avis` ; `ecrite` : celle que la validation ecrit
    /// (`relire::ecrire`), les lignes de l'engagement telles quelles et le plafond d'un temps partiel.
    pub(crate) fn forme<A>(&self, ctx: &Contexte, avis: &dyn Fn(&Avis) -> A, ecrite: bool) -> Resultat<FormeDePaie<A>> {
        let mut engagement = self.engagement.canon()?;
        // Le jour du versement que la paie declare et imprime (`Engagement::date_du_versement`) : le contrat le dit.
        engagement.date_paiement = Some(self.engagement.date_du_versement()?.iso());
        if ecrite {
            // Telles qu'elles s'ecrivent : sans ce que le moteur pose pour l'ecran (`ligne::Marques`), qui ne s'ecrit pas.
            engagement.lignes = self.engagement.lignes.iter().map(crate::LignePaie::sans_marques).collect();
        }
        Ok(FormeDePaie {
            assiette_chomage: self.assiette_chomage,
            avertissements: self.avertissements.iter().map(avis).collect(),
            base_csg: self.base_csg,
            bases: self.bases.clone(),
            brut: self.brut,
            cout_employeur: self.cout_employeur,
            deduction_forfaitaire: self.deduction_forfaitaire.as_ref().map(DeductionForfaitaire::forme),
            engagement,
            heures: self.heures,
            heures_absence: self.heures_absence,
            heures_travaillees: self.heures_travaillees,
            lignes: self.lignes.iter().map(LigneCotisation::forme).collect::<Resultat<_>>()?,
            net_a_payer: self.net_a_payer,
            net_imposable: self.net_imposable,
            net_social: self.net_social,
            numero_bulletin: self.numero_bulletin.clone(),
            reduction_generale: self.reduction_generale.clone(),
            remuneration: self.remuneration.iter().map(LigneBrut::forme).collect(),
            salaire_de_base: self.salaire_de_base,
            salaire_retabli: self.salaire_retabli,
            salarie: Cle { cle: self.salarie.id.to_string() },
            smic: dec_f64(self.smic)?,
            spectacle: self.spectacle.as_ref().map(|s| Cle { cle: s.cle.clone() }),
            total_pat: self.total_pat,
            total_sal: self.total_sal,
            plafond: self.plafond,
            plafond_urssaf: self.plafond_urssaf(ctx)?,
            periodes: self.periodes.iter().map(|p| p.forme(ctx, avis, ecrite)).collect::<Resultat<_>>()?,
            plafond_temps_partiel: ecrite.then(|| self.plafond_temps_partiel.code().to_string()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ligne(code: &str, montant: &str, heures: &str, base: bool, retabli: bool, soumis: bool) -> LigneBrut {
        LigneBrut {
            code: code.into(), libelle: code.into(), unite: "mois".into(), quantite: Decimal::ONE, taux: dec_de(montant),
            montant: dec_de(montant), heures_annexes: dec_de(heures), soumis, imposable: soumis, conges_spectacles: false,
            salaire_de_base: base, retabli, conges_pris: false, dsn_bloc52: None, arret_de_travail: retabli, dsn_autres_bruts: None,
            precision: None,
        }
    }

    fn dec_de(t: &str) -> Decimal {
        t.parse().unwrap()
    }

    /// CSS D. 241-7, IV, dernier alinea, et la fiche net-entreprises 2681 (`atelier/reference/sources/
    /// net-entreprises-base-connaissances/net-entreprises-fiche2681-reduction-generale-absence-proratisation-smic-types-028-029-
    /// 20260609.txt`), premier exemple : un salaire de 2 000 EUR, une absence qui en retire la moitie, une prime annuelle
    /// non affectee par l'absence ; « 028 » 1 000, « 029 » 2 000, « Smic = 1766.92 x 1000 /2000 = 883.46 » (SMIC horaire
    /// 11,65). Une IJSS subrogee, non soumise, n'entre pas au rapport ; un maintien integral garde le SMIC entier.
    #[test]
    fn le_smic_d_un_mois_d_absence_suit_le_rapport_des_remunerations() {
        let smic = Some(dec_de("11.65"));
        // 35 x 52 / 12 heures, comme la fiche compte le SMIC du mois (1 766,92).
        let mois = ligne("mois", "2000", "151.6667", true, false, true);
        let prime = ligne("prime", "3833.33", "0", false, false, true);
        let absence = ligne("absence_maladie", "-1000", "-75.83", false, true, true);
        let a = smic_d_une_absence(&[mois.clone(), absence.clone(), prime], smic).unwrap();
        assert_eq!((a.percue, a.habituelle), (dec_de("1000"), dec_de("2000")));
        assert_eq!(eur(mul(dec_de("11.65"), a.heures.unwrap())), dec_de("883.46"));
        let subrogee = ligne("ijss_nettes", "300", "0", false, false, false);
        assert_eq!(smic_d_une_absence(&[mois.clone(), absence.clone(), subrogee], smic).unwrap().percue, dec_de("1000"));
        let maintien = ligne("maintien_salaire", "1000", "0", false, true, true);
        let entier = smic_d_une_absence(&[mois.clone(), absence, maintien], smic).unwrap();
        assert_eq!(eur(mul(dec_de("11.65"), entier.heures.unwrap())), dec_de("1766.92"));
        assert!(smic_d_une_absence(&[mois], smic).is_none());
    }
}

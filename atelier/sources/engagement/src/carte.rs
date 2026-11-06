//! LA CARTE VALIDE : ce qu'un engagement dit, une variante par nature de contrat (`logiciel/donnees/contrats-base.json`,
//! `natures` : `cddu_spectacle`, `cdi`, `cdd`). Chaque variante ne porte que ses cases : une case obligatoire y est une
//! valeur, une case facultative un `Option` lu present (`socle::forme::presente`, ecrit `null` quand rien n'est dit) ; ce
//! qui n'appartient pas a une nature n'existe pas dans sa variante. Le motif de recours d'un CDD est une enum, et seul
//! un motif de remplacement porte la personne remplacee (L. 1242-12, 1°).
//!
//! LE BROUILLON (`Brouillon`) est ce que la page enregistre tant que l'engagement n'est pas valide : une variante par
//! nature aussi, chacune de ses cases un `Option` ecrit `null` tant qu'elle n'est pas remplie. Il change par une liste de
//! changements de case, tout ou rien (`Changements`, routes `enregistrer-saisie` et `modifier-engagement`, et l'apercu
//! d'une modification pas encore ecrite), et s'ecrit entier. La carte qu'il ecrit (`saisie::Carte`) se lit en
//! carte valide (`CarteValide::depuis`) : il lui manque une case obligatoire, ou elle porte une case que sa nature n'a
//! pas, le refus nomme chacune et rien ne s'ecrit. Seule une carte valide entre dans les calculs (`crate::engagements`).
//!
//! Ce qui fonde l'obligation de chaque case : le salarie (R. 1221-34, 1°) ; le spectacle d'un CDDU (son numero d'objet,
//! S21.G00.40.050) ; ses lignes (les dates jouees, R. 1221-34, 12°, et la remuneration, L. 1242-12, 7°) ; le debut d'un
//! contrat paye au mois (R. 1221-34, 4°, S21.G00.40.001), son salaire (L. 1242-12, 7°) et ses heures par mois
//! (S21.G00.40.013) ; la fin d'un CDD (L. 1242-12, 2°, S21.G00.40.010 CCH-12) et son motif (S21.G00.40.021, SIG-11,
//! `cdd.champs."S21.G00.40.021".obligatoire`). Le reste est facultatif : aucun fait ne l'exige.

use serde::{Deserialize, Serialize};

use socle::forme::NombreLu;
use socle::num::Resultat;

use crate::ligne::LignePaie;
use crate::saisie::{Carte, Champ, CongePris, Groupe, Preavis, Prime, Valeur};

/// Ce qu'une case de la carte est pour sa nature.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Exige {
    Obligatoire,
    Facultatif,
}

/// UNE NATURE DE CONTRAT ET SES CASES, declarees une fois : la structure (chaque case obligatoire une valeur, chaque case
/// facultative un `Option` present), la liste de ses cases (`CASES`), et sa lecture depuis la carte du mois. Une case se
/// lit de la case de meme nom de la carte, ou par la fonction que `par` nomme.
///
/// Elle declare aussi SON BROUILLON (`$brouillon`) : les memes cases, chacune un `Option` lu present (une case pas encore
/// remplie s'ecrit `null`), celle d'un motif en texte (`| String`, son code), et ce que porte un motif (`du_motif`).
macro_rules! nature {
    (
        $(#[$a:meta])*
        $nom:ident / $brouillon:ident {
            obligatoire { $( $o:ident : $to:ty $(| $bo:ty)? = $co:ident $(par $lo:path)? ),* $(,)? }
            facultatif { $( $f:ident : $tf:ty = $cf:ident ),* $(,)? }
            du_motif { $( $m:ident : $tm:ty = $cm:ident ),* $(,)? }
        }
    ) => {
        /// LE BROUILLON de cette nature : chacune de ses cases, remplie ou non.
        #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
        #[serde(deny_unknown_fields)]
        pub struct $brouillon {
            $( #[serde(deserialize_with = "socle::forme::presente")] pub $o: Option<nature!(@type $to $(| $bo)?)>, )*
            $( #[serde(deserialize_with = "socle::forme::presente")] pub $f: Option<$tf>, )*
            $( #[serde(deserialize_with = "socle::forme::presente")] pub $m: Option<$tm>, )*
        }

        impl $brouillon {
            /// Ses cases, dans l'ordre de la structure.
            pub const CASES: &'static [Champ] = &[ $( Champ::$co, )* $( Champ::$cf, )* $( Champ::$cm, )* ];

            /// Aucune case remplie.
            fn vide() -> $brouillon {
                $brouillon { $( $o: None, )* $( $f: None, )* $( $m: None, )* }
            }

            /// Ses cases, telles que la carte `c` les dit.
            fn de_la_carte(c: &Carte) -> $brouillon {
                $brouillon { $( $o: c.$o.clone(), )* $( $f: c.$f.clone(), )* $( $m: c.$m.clone(), )* }
            }

            /// Ses cases posees sur la carte `c`.
            fn sur_la_carte(&self, c: &mut Carte) {
                $( c.$o = self.$o.clone(); )*
                $( c.$f = self.$f.clone(); )*
                $( c.$m = self.$m.clone(); )*
            }
        }

        $(#[$a])*
        #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
        #[serde(deny_unknown_fields)]
        pub struct $nom {
            $( pub $o: $to, )*
            $( #[serde(deserialize_with = "socle::forme::presente")] pub $f: Option<$tf>, )*
        }

        impl $nom {
            /// Ses cases, dans l'ordre de la structure.
            pub const CASES: &'static [(Champ, Exige)] = &[ $( (Champ::$co, Exige::Obligatoire), )* $( (Champ::$cf, Exige::Facultatif), )* ];

            /// Ce que la carte `c` dit de cette nature ; `None` s'il lui manque une case obligatoire (`manques` les nomme).
            fn tiree(c: &Carte) -> Resultat<Option<$nom>> {
                Ok(Some($nom {
                    $( $o: match nature!(@lit c, $o $(, $lo)?) { Some(v) => v, None => return Ok(None) }, )*
                    $( $f: c.$f.clone(), )*
                }))
            }
        }
    };
    (@type $t:ty) => { $t };
    (@type $t:ty | $b:ty) => { $b };
    (@lit $c:ident, $o:ident) => { $c.$o.clone() };
    (@lit $c:ident, $o:ident, $l:path) => { $l($c)? };
}

nature! {
    /// UN CDDU DU SPECTACLE (L. 1242-2, 3°) : l'artiste ou le technicien engage pour un spectacle, paye a ses lignes
    /// (cachets, heures). Son debut et sa fin se tirent de ses jours joues quand la carte ne les dit pas.
    Cddu / BrouillonCddu {
        obligatoire {
            salarie: String = Salarie,
            spectacle: String = Spectacle,
            lignes: Vec<LignePaie> = Lignes,
        }
        facultatif {
            debut: String = Debut,
            fin: String = Fin,
            numero: String = Numero,
            etablissement: String = Etablissement,
            metier: String = Metier,
            cadre: bool = Cadre,
            lieu: String = Lieu,
            emploi: String = Emploi,
            pcs_ese: String = PcsEse,
            code_emploi_ft: String = CodeEmploiFt,
            statut: String = Statut,
            statut_conventionnel: String = StatutConventionnel,
            statut_categoriel_rc: String = StatutCategorielRc,
            periode_essai_jours: NombreLu = PeriodeEssaiJours,
            clauses_particulieres: String = ClausesParticulieres,
            primes: Vec<Prime> = Primes,
            date_paiement: String = DatePaiement,
            jours: NombreLu = Jours,
            taux_cachet: NombreLu = TauxCachet,
            indemnite_licenciement: NombreLu = IndemniteLicenciement,
            faute_grave: bool = FauteGrave,
            motif_rupture: String = MotifRupture,
            notification_rupture: String = NotificationRupture,
            engagement_licenciement: String = EngagementLicenciement,
            signature_convention: String = SignatureConvention,
            preavis: Preavis = Preavis,
            transaction: String = Transaction,
            dernier_jour_paye: String = DernierJourPaye,
            en_droit_a_la_retraite: bool = EnDroitALaRetraite,
            preavis_non_effectue: bool = PreavisNonEffectue,
            preavis_mois: NombreLu = PreavisMois,
            precarite_due: bool = PrecariteDue,
            precarite_pourquoi: String = PrecaritePourquoi,
        }
        du_motif {}
    }
}

nature! {
    /// UN CDI, paye au mois : son debut, son salaire et ses heures par mois. Sa fin est sa rupture.
    Cdi / BrouillonCdi {
        obligatoire {
            salarie: String = Salarie,
            debut: String = Debut,
            salaire_mensuel: NombreLu = SalaireMensuel,
            quotite_contrat: NombreLu = QuotiteContrat,
        }
        facultatif {
            fin: String = Fin,
            numero: String = Numero,
            etablissement: String = Etablissement,
            metier: String = Metier,
            cadre: bool = Cadre,
            lieu: String = Lieu,
            quotite_reference: NombreLu = QuotiteReference,
            modalite_temps_travail: String = ModaliteTempsTravail,
            emploi: String = Emploi,
            pcs_ese: String = PcsEse,
            code_emploi_ft: String = CodeEmploiFt,
            statut: String = Statut,
            statut_conventionnel: String = StatutConventionnel,
            statut_categoriel_rc: String = StatutCategorielRc,
            periode_essai_jours: NombreLu = PeriodeEssaiJours,
            repartition_horaire: String = RepartitionHoraire,
            clauses_particulieres: String = ClausesParticulieres,
            lignes: Vec<LignePaie> = Lignes,
            conges_pris: Vec<CongePris> = CongesPris,
            primes: Vec<Prime> = Primes,
            date_paiement: String = DatePaiement,
            jours: NombreLu = Jours,
            heures_supplementaires: NombreLu = HeuresSupplementaires,
            heures_supplementaires_a_50: NombreLu = HeuresSupplementairesA50,
            heures_complementaires: NombreLu = HeuresComplementaires,
            indemnite_licenciement: NombreLu = IndemniteLicenciement,
            faute_grave: bool = FauteGrave,
            motif_rupture: String = MotifRupture,
            notification_rupture: String = NotificationRupture,
            engagement_licenciement: String = EngagementLicenciement,
            signature_convention: String = SignatureConvention,
            preavis: Preavis = Preavis,
            transaction: String = Transaction,
            dernier_jour_paye: String = DernierJourPaye,
            en_droit_a_la_retraite: bool = EnDroitALaRetraite,
            preavis_non_effectue: bool = PreavisNonEffectue,
            preavis_mois: NombreLu = PreavisMois,
            precarite_due: bool = PrecariteDue,
            precarite_pourquoi: String = PrecaritePourquoi,
        }
        du_motif {}
    }
}

nature! {
    /// UN CDD ORDINAIRE (L. 1242-2, hors 3°), paye au mois : son debut et sa fin, son motif, son salaire et ses heures
    /// par mois.
    Cdd / BrouillonCdd {
        obligatoire {
            salarie: String = Salarie,
            debut: String = Debut,
            fin: String = Fin,
            motif_recours: Recours | String = MotifRecours par Recours::de_la_carte,
            salaire_mensuel: NombreLu = SalaireMensuel,
            quotite_contrat: NombreLu = QuotiteContrat,
        }
        facultatif {
            numero: String = Numero,
            etablissement: String = Etablissement,
            metier: String = Metier,
            cadre: bool = Cadre,
            lieu: String = Lieu,
            quotite_reference: NombreLu = QuotiteReference,
            modalite_temps_travail: String = ModaliteTempsTravail,
            emploi: String = Emploi,
            pcs_ese: String = PcsEse,
            code_emploi_ft: String = CodeEmploiFt,
            statut: String = Statut,
            statut_conventionnel: String = StatutConventionnel,
            statut_categoriel_rc: String = StatutCategorielRc,
            periode_essai_jours: NombreLu = PeriodeEssaiJours,
            repartition_horaire: String = RepartitionHoraire,
            clauses_particulieres: String = ClausesParticulieres,
            lignes: Vec<LignePaie> = Lignes,
            conges_pris: Vec<CongePris> = CongesPris,
            primes: Vec<Prime> = Primes,
            date_paiement: String = DatePaiement,
            jours: NombreLu = Jours,
            heures_supplementaires: NombreLu = HeuresSupplementaires,
            heures_supplementaires_a_50: NombreLu = HeuresSupplementairesA50,
            heures_complementaires: NombreLu = HeuresComplementaires,
            indemnite_licenciement: NombreLu = IndemniteLicenciement,
            faute_grave: bool = FauteGrave,
            motif_rupture: String = MotifRupture,
            notification_rupture: String = NotificationRupture,
            engagement_licenciement: String = EngagementLicenciement,
            signature_convention: String = SignatureConvention,
            preavis: Preavis = Preavis,
            transaction: String = Transaction,
            dernier_jour_paye: String = DernierJourPaye,
            en_droit_a_la_retraite: bool = EnDroitALaRetraite,
            preavis_non_effectue: bool = PreavisNonEffectue,
            preavis_mois: NombreLu = PreavisMois,
            precarite_due: bool = PrecariteDue,
            precarite_pourquoi: String = PrecaritePourquoi,
        }
        du_motif {
            personne_remplacee: String = PersonneRemplacee,
        }
    }
}

/// LE MOTIF DE RECOURS D'UN CDD (S21.G00.40.021), tel que `contrats-base.json` les offre (`cdd.champs."S21.G00.40.021"
/// .valeurs`) : seul un motif de remplacement nomme la personne remplacee (L. 1242-12, 1° ; facultative, comme le contrat
/// ecrit l'imprime, `si_dite`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "code", deny_unknown_fields)]
pub enum Recours {
    #[serde(rename = "01")]
    Remplacement {
        #[serde(deserialize_with = "socle::forme::presente")]
        personne_remplacee: Option<String>,
    },
    #[serde(rename = "02")]
    Accroissement,
    #[serde(rename = "03")]
    Saisonnier,
    #[serde(rename = "06")]
    ObjetDefini,
    #[serde(rename = "09")]
    Insertion,
    #[serde(rename = "10")]
    ComplementDeFormation,
    #[serde(rename = "12")]
    RemplacementATempsPartiel {
        #[serde(deserialize_with = "socle::forme::presente")]
        personne_remplacee: Option<String>,
    },
    #[serde(rename = "13")]
    AttenteDeSuppression {
        #[serde(deserialize_with = "socle::forme::presente")]
        personne_remplacee: Option<String>,
    },
}

impl Recours {
    /// Les cases qu'un motif porte en plus de celles du CDD.
    pub const CASES: &'static [Champ] = &[Champ::PersonneRemplacee];

    /// Chaque motif : son code, et s'il nomme la personne remplacee.
    pub const TOUS: &'static [(&'static str, bool)] =
        &[("01", true), ("02", false), ("03", false), ("06", false), ("09", false), ("10", false), ("12", true), ("13", true)];

    /// Le motif que la carte dit (`None` : elle n'en dit pas) ; un code que la liste n'offre pas se refuse.
    fn de_la_carte(c: &Carte) -> Resultat<Option<Recours>> {
        let Some(code) = c.motif_recours.as_deref().filter(|m| !m.trim().is_empty()) else { return Ok(None) };
        let p = c.personne_remplacee.clone();
        Ok(Some(match code {
            "01" => Recours::Remplacement { personne_remplacee: p },
            "02" => Recours::Accroissement,
            "03" => Recours::Saisonnier,
            "06" => Recours::ObjetDefini,
            "09" => Recours::Insertion,
            "10" => Recours::ComplementDeFormation,
            "12" => Recours::RemplacementATempsPartiel { personne_remplacee: p },
            "13" => Recours::AttenteDeSuppression { personne_remplacee: p },
            autre => {
                let codes: Vec<&str> = Recours::TOUS.iter().map(|(c, _)| *c).collect();
                return Err(format!("« {} » n'est pas un motif de recours d'un CDD : {}.", autre, codes.join(", ")));
            }
        }))
    }

    /// Son code.
    pub fn code(&self) -> &'static str {
        match self {
            Recours::Remplacement { .. } => "01",
            Recours::Accroissement => "02",
            Recours::Saisonnier => "03",
            Recours::ObjetDefini => "06",
            Recours::Insertion => "09",
            Recours::ComplementDeFormation => "10",
            Recours::RemplacementATempsPartiel { .. } => "12",
            Recours::AttenteDeSuppression { .. } => "13",
        }
    }

    /// La personne remplacee, pour un motif de remplacement qui la nomme.
    pub fn personne_remplacee(&self) -> Option<&String> {
        match self {
            Recours::Remplacement { personne_remplacee } | Recours::RemplacementATempsPartiel { personne_remplacee } | Recours::AttenteDeSuppression { personne_remplacee } => {
                personne_remplacee.as_ref()
            }
            Recours::Accroissement | Recours::Saisonnier | Recours::ObjetDefini | Recours::Insertion | Recours::ComplementDeFormation => None,
        }
    }

    /// Le motif nomme-t-il la personne remplacee ?
    fn remplace(&self) -> bool {
        Recours::TOUS.iter().any(|(c, p)| *c == self.code() && *p)
    }
}

/// LA CARTE VALIDE : une variante par nature de contrat, la nature dite (`nature`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "nature", deny_unknown_fields)]
pub enum CarteValide {
    #[serde(rename = "cddu_spectacle")]
    CdduSpectacle(Cddu),
    #[serde(rename = "cdi")]
    Cdi(Cdi),
    #[serde(rename = "cdd")]
    Cdd(Cdd),
}

/// UNE NATURE DECRITE pour l'appelant (`natures` de la fiche des routes qui ecrivent une carte, et de la route etat) :
/// sa cle, chacune de ses cases (obligatoire ou facultative), et, pour un CDD, ses motifs et ce que chacun porte en plus.
#[derive(Debug, Clone, Serialize)]
pub struct NatureDecrite {
    pub nature: &'static str,
    pub cases: Vec<CaseDecrite>,
    pub motifs_recours: Vec<MotifDecrit>,
}

/// Une case d'une nature : sa cle sur la carte, son libelle, ce qu'elle est.
#[derive(Debug, Clone, Serialize)]
pub struct CaseDecrite {
    pub cle: &'static str,
    pub libelle: &'static str,
    pub exige: Exige,
}

/// Un motif de recours : son code, et les cases qu'il porte en plus de celles de sa nature.
#[derive(Debug, Clone, Serialize)]
pub struct MotifDecrit {
    pub code: &'static str,
    pub cases: Vec<CaseDecrite>,
}

/// La nature d'un contrat d'usage du spectacle.
pub const CDDU: &str = "cddu_spectacle";

/// Les natures et leurs cases, dans l'ordre de `contrats-base.json`.
pub const NATURES: [(&str, &[(Champ, Exige)]); 3] = [("cddu_spectacle", Cddu::CASES), ("cdi", Cdi::CASES), ("cdd", Cdd::CASES)];

impl CarteValide {
    /// Les cases de la nature `cle` ; `None` : ce n'est pas une nature.
    pub fn cases_de(cle: &str) -> Option<&'static [(Champ, Exige)]> {
        NATURES.iter().find(|(n, _)| *n == cle).map(|(_, c)| *c)
    }

    /// Chaque nature et ses cases, tirees du type.
    pub fn natures() -> Vec<NatureDecrite> {
        let decrire = |cases: &'static [(Champ, Exige)]| -> Vec<CaseDecrite> {
            cases.iter().map(|(c, e)| CaseDecrite { cle: c.nom(), libelle: libelle(*c), exige: *e }).collect()
        };
        NATURES
            .iter()
            .map(|(n, cases)| NatureDecrite {
                nature: n,
                cases: decrire(cases),
                motifs_recours: if *n == "cdd" {
                    Recours::TOUS
                        .iter()
                        .map(|(code, remplace)| MotifDecrit {
                            code,
                            cases: if *remplace {
                                Recours::CASES.iter().map(|c| CaseDecrite { cle: c.nom(), libelle: libelle(*c), exige: Exige::Facultatif }).collect()
                            } else {
                                Vec::new()
                            },
                        })
                        .collect()
                } else {
                    Vec::new()
                },
            })
            .collect()
    }

    /// LA CARTE DU MOIS `c`, VALIDE : sa nature dite, chaque case obligatoire remplie, aucune case que sa nature n'a pas.
    /// Le refus nomme chaque case a remplir (`A_REMPLIR`), ou chaque case a retirer ; `qui` : le salarie qu'il nomme. Ce
    /// que la vue deduit (`Groupe::Deduit`) et ce qu'elle dit de l'engagement ecrit (`Groupe::Vue`) ne sont pas des cases.
    pub fn depuis(c: &Carte, qui: &str) -> Resultat<CarteValide> {
        let Some(nature) = c.nature.as_deref().filter(|n| !n.trim().is_empty()) else {
            return Err(format!("{}{}", qui, crate::PAS_DE_NATURE));
        };
        let Some(cases) = CarteValide::cases_de(nature) else {
            let cles: Vec<&str> = NATURES.iter().map(|(n, _)| *n).collect();
            return Err(format!("{} : « {} » n'est pas une nature de contrat : {}.", qui, nature, cles.join(", ")));
        };
        let nat = droit::natures::nature(Some(nature))?;
        let libelle_nature = nat.libelle.clone();
        // Un motif que la nature n'offre pas se refuse en disant ceux qu'elle offre (`contrats-base.json`).
        if let Some(refus) = c.motif_recours.as_deref().and_then(|m| nat.motif_recours_refuse(m, qui)) {
            return Err(refus);
        }
        // Ce que porte un motif de recours est une case du CDD (son motif dit ensuite s'il la porte).
        let du_motif: &[Champ] = if nature == "cdd" { Recours::CASES } else { &[] };
        let a_elle = |x: Champ| x == Champ::Nature || du_motif.contains(&x) || cases.iter().any(|(k, _)| *k == x);
        let de_trop: Vec<&str> = Champ::TOUS
            .iter()
            .copied()
            .filter(|x| !matches!(x.groupe(), Groupe::Deduit | Groupe::Vue) && c.dit(*x) && !a_elle(*x))
            .map(libelle)
            .collect();
        if !de_trop.is_empty() {
            return Err(format!(
                "{} : {} {} pas une case d'un {} ; retirez-{} de sa carte. Rien n'est écrit.",
                qui,
                guillemets(&de_trop),
                if de_trop.len() > 1 { "ne sont" } else { "n'est" },
                libelle_nature,
                if de_trop.len() > 1 { "les" } else { "la" }
            ));
        }
        let manques: Vec<&str> = cases.iter().filter(|(x, e)| *e == Exige::Obligatoire && !remplie(c, *x)).map(|(x, _)| libelle(*x)).collect();
        if !manques.is_empty() {
            return Err(format!("{}{}{} ({}).", qui, A_REMPLIR, guillemets(&manques), libelle_nature));
        }
        let manquante = || format!("{}{}({})", qui, A_REMPLIR, libelle_nature);
        Ok(match nature {
            "cddu_spectacle" => CarteValide::CdduSpectacle(Cddu::tiree(c)?.ok_or_else(manquante)?),
            "cdi" => CarteValide::Cdi(Cdi::tiree(c)?.ok_or_else(manquante)?),
            _ => {
                let v = Cdd::tiree(c)?.ok_or_else(manquante)?;
                if c.personne_remplacee.is_some() && !v.motif_recours.remplace() {
                    return Err(format!(
                        "{} : {} n'est pas une case d'un CDD conclu pour ce motif ({}) ; retirez-la de sa carte. Rien n'est écrit.",
                        qui,
                        guillemets(&[libelle(Champ::PersonneRemplacee)]),
                        v.motif_recours.code()
                    ));
                }
                CarteValide::Cdd(v)
            }
        })
    }

    /// Sa nature, telle que `contrats-base.json` la nomme.
    pub fn nature(&self) -> &'static str {
        match self {
            CarteValide::CdduSpectacle(_) => "cddu_spectacle",
            CarteValide::Cdi(_) => "cdi",
            CarteValide::Cdd(_) => "cdd",
        }
    }
}

/// UNE NATURE DE CONTRAT, telle que `contrats-base.json` la nomme : celle d'un brouillon, des sa naissance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NatureDeContrat {
    #[serde(rename = "cddu_spectacle")]
    CdduSpectacle,
    #[serde(rename = "cdi")]
    Cdi,
    #[serde(rename = "cdd")]
    Cdd,
}

impl NatureDeContrat {
    /// Sa cle.
    pub fn cle(self) -> &'static str {
        match self {
            NatureDeContrat::CdduSpectacle => "cddu_spectacle",
            NatureDeContrat::Cdi => "cdi",
            NatureDeContrat::Cdd => "cdd",
        }
    }
}

/// LE BROUILLON D'UNE CARTE : ce que la page enregistre tant que l'engagement n'est pas valide, peut-etre incomplet. Il
/// nait avec sa nature (une carte neuve s'ouvre dans celle que dit la route etat, `nouvelle_carte`) et n'existe jamais
/// sans elle ; chacune des cases de sa nature y est, remplie ou `null`. Il change d'une case a la fois (`Changement`) ;
/// changer de nature le fait changer de variante. Valider, c'est le lire en carte valide (`CarteValide::depuis`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "nature", deny_unknown_fields)]
pub enum Brouillon {
    #[serde(rename = "cddu_spectacle")]
    CdduSpectacle(BrouillonCddu),
    #[serde(rename = "cdi")]
    Cdi(BrouillonCdi),
    #[serde(rename = "cdd")]
    Cdd(BrouillonCdd),
}

impl Brouillon {
    /// Un brouillon neuf de la nature `n` : aucune case remplie.
    pub fn neuf(n: NatureDeContrat) -> Brouillon {
        match n {
            NatureDeContrat::CdduSpectacle => Brouillon::CdduSpectacle(BrouillonCddu::vide()),
            NatureDeContrat::Cdi => Brouillon::Cdi(BrouillonCdi::vide()),
            NatureDeContrat::Cdd => Brouillon::Cdd(BrouillonCdd::vide()),
        }
    }

    /// Sa nature.
    pub fn nature(&self) -> NatureDeContrat {
        match self {
            Brouillon::CdduSpectacle(_) => NatureDeContrat::CdduSpectacle,
            Brouillon::Cdi(_) => NatureDeContrat::Cdi,
            Brouillon::Cdd(_) => NatureDeContrat::Cdd,
        }
    }

    /// Les cases de sa nature (et, pour un CDD, ce que porte un motif).
    pub fn cases(n: NatureDeContrat) -> &'static [Champ] {
        match n {
            NatureDeContrat::CdduSpectacle => BrouillonCddu::CASES,
            NatureDeContrat::Cdi => BrouillonCdi::CASES,
            NatureDeContrat::Cdd => BrouillonCdd::CASES,
        }
    }

    /// LE BROUILLON QUE LA CARTE `c` ECRIT (la vue d'un mois d'un engagement en saisie, `stock::Objet::vue`) : sa nature
    /// dite, et aucune case que sa nature n'a pas (le refus les nomme). Ce que la vue deduit (`Groupe::Deduit`) et ce
    /// qu'elle dit de l'engagement ecrit (`Groupe::Vue`) ne sont pas des cases. `qui` : le salarie que le refus nomme.
    pub fn de_la_carte(c: &Carte, qui: &str) -> Resultat<Brouillon> {
        let Some(cle) = c.nature.as_deref().filter(|n| !n.trim().is_empty()) else {
            return Err(format!("{}{}", qui, crate::PAS_DE_NATURE));
        };
        let n = match cle {
            "cddu_spectacle" => NatureDeContrat::CdduSpectacle,
            "cdi" => NatureDeContrat::Cdi,
            "cdd" => NatureDeContrat::Cdd,
            autre => {
                let cles: Vec<&str> = NATURES.iter().map(|(n, _)| *n).collect();
                return Err(format!("{} : « {} » n'est pas une nature de contrat : {}.", qui, autre, cles.join(", ")));
            }
        };
        let cases = Brouillon::cases(n);
        let de_trop: Vec<&str> = Champ::TOUS
            .iter()
            .copied()
            .filter(|x| !matches!(x.groupe(), Groupe::Deduit | Groupe::Vue) && *x != Champ::Nature && c.dit(*x) && !cases.contains(x))
            .map(libelle)
            .collect();
        if !de_trop.is_empty() {
            return Err(format!(
                "{} : {} {} pas une case d'un {}. Rien n'est écrit.",
                qui,
                guillemets(&de_trop),
                if de_trop.len() > 1 { "ne sont" } else { "n'est" },
                droit::natures::nature(Some(cle))?.libelle
            ));
        }
        Ok(match n {
            NatureDeContrat::CdduSpectacle => Brouillon::CdduSpectacle(BrouillonCddu::de_la_carte(c)),
            NatureDeContrat::Cdi => Brouillon::Cdi(BrouillonCdi::de_la_carte(c)),
            NatureDeContrat::Cdd => Brouillon::Cdd(BrouillonCdd::de_la_carte(c)),
        })
    }

    /// La carte qu'il ecrit : sa nature et ses cases remplies.
    pub fn en_carte(&self) -> Carte {
        let mut c = Carte { nature: Some(self.nature().cle().to_string()), ..Carte::default() };
        match self {
            Brouillon::CdduSpectacle(b) => b.sur_la_carte(&mut c),
            Brouillon::Cdi(b) => b.sur_la_carte(&mut c),
            Brouillon::Cdd(b) => b.sur_la_carte(&mut c),
        }
        c
    }

    /// LE CHANGEMENT D'UNE CASE `ch`, applique. Changer de nature change le brouillon de variante : les cases propres a
    /// l'ancienne disparaissent, celles propres a la nouvelle naissent vides, et une case des deux (le salarie, le metier,
    /// les bornes, les lignes...) garde ce qu'elle dit : c'est la meme case, et l'ecran la montre telle quelle. Refus :
    /// une case que sa nature n'a pas, un motif de recours que sa nature n'offre pas ; rien ne change.
    pub fn changer(&mut self, ch: Changement, qui: &str) -> Resultat<()> {
        let mut c = self.en_carte();
        if let Changement::Nature(n) = ch {
            let mut neuve = Carte { nature: Some(n.cle().to_string()), ..Carte::default() };
            for x in Brouillon::cases(n) {
                neuve.prendre(&c, *x);
            }
            *self = Brouillon::de_la_carte(&neuve, qui)?;
            return Ok(());
        }
        let x = ch.champ();
        if !Brouillon::cases(self.nature()).contains(&x) {
            let nat = droit::natures::nature(Some(self.nature().cle()))?;
            return Err(format!("{} : {} n'est pas une case d'un {}. Rien n'est écrit.", qui, guillemets(&[libelle(x)]), nat.libelle));
        }
        if let Changement::MotifRecours(Some(m)) = &ch {
            let nat = droit::natures::nature(Some(self.nature().cle()))?;
            if let Some(refus) = nat.motif_recours_refuse(m, qui) {
                return Err(refus);
            }
        }
        ch.poser(&mut c);
        *self = Brouillon::de_la_carte(&c, qui)?;
        Ok(())
    }
}

/// LES CHANGEMENTS D'UNE CASE, declares une fois : chaque case qui se saisit (ni ce que la vue deduit, ni ce qu'elle dit
/// de l'engagement ecrit), sa cle, son type (`null` : la case se vide) et son champ de la carte.
macro_rules! changements {
    ( $( $v:ident ( $t:ty ) = $cle:tt => $champ:ident ),* $(,)? ) => {
        /// UN CHANGEMENT D'UNE CASE DU BROUILLON : `{"case": "salarie", "valeur": "FULLER"}` ; `{"case": "nature",
        /// "valeur": "cdi"}` change sa nature, qui ne se vide jamais.
        #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
        #[serde(tag = "case", content = "valeur", deny_unknown_fields)]
        pub enum Changement {
            #[serde(rename = "nature")]
            Nature(NatureDeContrat),
            $( #[serde(rename = $cle)] $v(Option<$t>), )*
        }

        impl Changement {
            /// La case qu'il change.
            pub fn champ(&self) -> Champ {
                match self {
                    Changement::Nature(_) => Champ::Nature,
                    $( Changement::$v(_) => Champ::$v, )*
                }
            }

            /// Pose sur la carte `c` la valeur qu'il dit.
            fn poser(self, c: &mut Carte) {
                match self {
                    Changement::Nature(n) => c.nature = Some(n.cle().to_string()),
                    $( Changement::$v(x) => c.$champ = x, )*
                }
            }
        }
    };
}

changements! {
    Salarie(String) = "salarie" => salarie,
    Spectacle(String) = "spectacle" => spectacle,
    Debut(String) = "debut" => debut,
    Fin(String) = "fin" => fin,
    Numero(String) = "numero" => numero,
    Etablissement(String) = "etablissement" => etablissement,
    Metier(String) = "metier" => metier,
    Cadre(bool) = "cadre" => cadre,
    Lieu(String) = "lieu" => lieu,
    MotifRecours(String) = "motif_recours" => motif_recours,
    QuotiteContrat(NombreLu) = "quotite_contrat" => quotite_contrat,
    QuotiteReference(NombreLu) = "quotite_reference" => quotite_reference,
    ModaliteTempsTravail(String) = "modalite_temps_travail" => modalite_temps_travail,
    SalaireMensuel(NombreLu) = "salaire_mensuel" => salaire_mensuel,
    Emploi(String) = "emploi" => emploi,
    PcsEse(String) = "pcs_ese" => pcs_ese,
    CodeEmploiFt(String) = "code_emploi_ft" => code_emploi_ft,
    Statut(String) = "statut" => statut,
    StatutConventionnel(String) = "statut_conventionnel" => statut_conventionnel,
    StatutCategorielRc(String) = "statut_categoriel_rc" => statut_categoriel_rc,
    PeriodeEssaiJours(NombreLu) = "periode_essai_jours" => periode_essai_jours,
    PersonneRemplacee(String) = "personne_remplacee" => personne_remplacee,
    RepartitionHoraire(String) = "repartition_horaire" => repartition_horaire,
    ClausesParticulieres(String) = "clauses_particulieres" => clauses_particulieres,
    Lignes(Vec<LignePaie>) = "lignes" => lignes,
    CongesPris(Vec<CongePris>) = "conges_pris" => conges_pris,
    Primes(Vec<Prime>) = "primes" => primes,
    DatePaiement(String) = "date_paiement" => date_paiement,
    Jours(NombreLu) = "jours" => jours,
    TauxCachet(NombreLu) = "taux_cachet" => taux_cachet,
    HeuresSupplementaires(NombreLu) = "heures_supplementaires" => heures_supplementaires,
    HeuresSupplementairesA50(NombreLu) = "heures_supplementaires_a_50" => heures_supplementaires_a_50,
    HeuresComplementaires(NombreLu) = "heures_complementaires" => heures_complementaires,
    IndemniteLicenciement(NombreLu) = "indemnite_licenciement" => indemnite_licenciement,
    FauteGrave(bool) = "faute_grave" => faute_grave,
    MotifRupture(String) = "motif_rupture" => motif_rupture,
    NotificationRupture(String) = "notification_rupture" => notification_rupture,
    EngagementLicenciement(String) = "engagement_licenciement" => engagement_licenciement,
    SignatureConvention(String) = "signature_convention" => signature_convention,
    Preavis(Preavis) = "preavis" => preavis,
    Transaction(String) = "transaction" => transaction,
    DernierJourPaye(String) = "dernier_jour_paye" => dernier_jour_paye,
    EnDroitALaRetraite(bool) = "en_droit_a_la_retraite" => en_droit_a_la_retraite,
    PreavisNonEffectue(bool) = "preavis_non_effectue" => preavis_non_effectue,
    PreavisMois(NombreLu) = "preavis_mois" => preavis_mois,
    PrecariteDue(bool) = "precarite_due" => precarite_due,
    PrecaritePourquoi(String) = "precarite_pourquoi" => precarite_pourquoi,
}

/// LES CHANGEMENTS D'UN APPEL (`changements` des routes `enregistrer-saisie` et `modifier-engagement`) : au moins un,
/// appliques dans l'ordre, tout ou rien. La page en envoie ce qui a change depuis sa derniere ecriture (une case, le plus
/// souvent) ; un appelant en ligne de commande, toutes les cases d'une carte d'un coup.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Changements(Vec<Changement>);

impl<'de> Deserialize<'de> for Changements {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Changements, D::Error> {
        let v = Vec::<Changement>::deserialize(d)?;
        if v.is_empty() {
            return Err(serde::de::Error::custom("`changements` : au moins un changement, {\"case\": …, \"valeur\": …}"));
        }
        Ok(Changements(v))
    }
}

impl Changements {
    /// CHAQUE CHANGEMENT APPLIQUE AU BROUILLON `b`, dans l'ordre (`Brouillon::changer`), tout ou rien : le premier refus
    /// (il nomme sa case) laisse `b` tel qu'il etait. `qui` : le salarie que le refus nomme, tel que le brouillon le dit
    /// au moment du changement.
    pub fn appliquer(self, b: &mut Brouillon, qui: impl Fn(&Brouillon) -> String) -> Resultat<()> {
        let mut essai = b.clone();
        for ch in self.0 {
            let q = qui(&essai);
            essai.changer(ch, &q)?;
        }
        *b = essai;
        Ok(())
    }
}

/// Le debut du refus d'une carte a qui il manque des cases (`crate::SAISIE_INCOMPLETE`).
pub const A_REMPLIR: &str = " : à remplir sur sa carte avant de valider : ";

/// Une case obligatoire est remplie : dite, et un texte n'y est pas vide.
fn remplie(c: &Carte, x: Champ) -> bool {
    match c.valeur_du(x) {
        Some(Valeur::Texte(t)) => !t.trim().is_empty(),
        _ => c.dit(x),
    }
}

/// « a », « b » et « c ».
fn guillemets(noms: &[&str]) -> String {
    let g: Vec<String> = noms.iter().map(|n| format!("« {} »", n)).collect();
    match g.split_last() {
        Some((dernier, [])) => dernier.clone(),
        Some((dernier, reste)) => format!("{} et {}", reste.join(", "), dernier),
        None => String::new(),
    }
}

/// LE LIBELLE D'UNE CASE DE LA CARTE, tel que l'ecran l'ecrit.
pub fn libelle(c: Champ) -> &'static str {
    match c {
        Champ::Salarie => "Salarié",
        Champ::Spectacle => "Spectacle",
        Champ::Nature => "Nature",
        Champ::Debut => "Début",
        Champ::Fin => "Fin",
        Champ::Numero => "Numéro du contrat",
        Champ::Etablissement => "Établissement",
        Champ::Metier => "Métier",
        Champ::Cadre => "Cadre",
        Champ::Lieu => "Lieu",
        Champ::MotifRecours => "Motif du CDD",
        Champ::QuotiteContrat => "Heures par mois",
        Champ::QuotiteReference => "Heures de référence",
        Champ::ModaliteTempsTravail => "Temps de travail",
        Champ::SalaireMensuel => "Salaire mensuel brut",
        Champ::Emploi => "Emploi",
        Champ::PcsEse => "PCS-ESE",
        Champ::CodeEmploiFt => "Code emploi France Travail",
        Champ::Statut => "Statut",
        Champ::StatutConventionnel => "Statut conventionnel",
        Champ::StatutCategorielRc => "Statut catégoriel",
        Champ::PeriodeEssaiJours => "Période d'essai",
        Champ::PersonneRemplacee => "Personne remplacée",
        Champ::RepartitionHoraire => "Répartition des horaires",
        Champ::ClausesParticulieres => "Clauses particulières",
        Champ::Lignes => "Lignes de paie",
        Champ::CongesPris => "Congés pris",
        Champ::Primes => "Primes",
        Champ::DatePaiement => "Payé le",
        Champ::Jours => "Jours travaillés",
        Champ::TauxCachet => "Cachet",
        Champ::HeuresSupplementaires => "Heures supplémentaires",
        Champ::HeuresSupplementairesA50 => "Dont à 50 %",
        Champ::HeuresComplementaires => "Heures complémentaires",
        Champ::IndemniteLicenciement => "Indemnité de licenciement",
        Champ::FauteGrave => "Faute grave",
        Champ::MotifRupture => "Motif de la fin",
        Champ::NotificationRupture => "Notifiée le",
        Champ::EngagementLicenciement => "Procédure engagée le",
        Champ::SignatureConvention => "Convention signée le",
        Champ::Preavis => "Préavis",
        Champ::Transaction => "Transaction",
        Champ::DernierJourPaye => "Dernier jour payé",
        Champ::EnDroitALaRetraite => "En droit à la retraite",
        Champ::PreavisNonEffectue => "Préavis non effectué",
        Champ::PreavisMois => "Mois de préavis",
        Champ::PrecariteDue => "Indemnité de fin de contrat",
        Champ::PrecaritePourquoi => "Pourquoi elle n'est pas due",
        Champ::JoursContrat => "Jours du contrat",
        Champ::DateModification => "Date de la modification",
        Champ::Avant => "Termes d'avant",
        Champ::Valide => "Validé",
        Champ::Importe => "Importé",
    }
}

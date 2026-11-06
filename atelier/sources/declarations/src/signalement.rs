//! Les signalements d'evenements, dans la version de la norme NEODeS que leur jour de
//! constitution appelle (`norme::du_signalement`) :
//! la nature 04 (arret de travail) et la nature 05 (reprise suite a arret).
//!
//! Un signalement ne porte ni paie, ni cotisation : un individu, un contrat,
//! un evenement. Chaque rubrique ecrite ici est une rubrique que le tableau
//! des usages admet pour la nature produite. Les refus de la norme
//! (`SignalementImpossible`) gardent leurs phrases ; les autres refus (fiche
//! introuvable, valeur absente, usage ou format hors norme) sont des
//! `ValueError`/`KeyError`, ici `Echec::Autre`.

use serde_json::Value;

use droit::contexte::{aujourdhui, Contexte};
use socle::date::{d, en_texte, Jour};
use crate::dsn::{self, Arret, Fichier, LOGICIEL, VERSION};
use socle::fichiers::{repr, repr_valeur, vrai};
use socle::num::Resultat;

pub const ARRET: &str = "04";
pub const REPRISE: &str = "05";
pub const NORMALE: &str = "01";
pub const ANNULE_ET_REMPLACE: &str = "03";

/// Un refus : `Impossible` est `SignalementImpossible`, `Autre` le reste.
#[derive(Debug, Clone, PartialEq)]
pub enum Echec {
    Impossible(String),
    Autre(String),
}

impl From<String> for Echec {
    fn from(m: String) -> Echec {
        Echec::Autre(m)
    }
}

type R<T> = Result<T, Echec>;

fn impossible<T>(m: String) -> R<T> {
    Err(Echec::Impossible(m))
}

const NUL: Value = Value::Null;

fn g<'a>(v: &'a Value, k: &str) -> &'a Value {
    v.get(k).unwrap_or(&NUL)
}

fn s(v: &Value, k: &str) -> String {
    en_texte(g(v, k))
}

/// L'arret est-il subroge ? Son code de subrogation est le « oui » de la norme (`droit::arrets::Bloc60`).
pub fn avec_subrogation(a: &Arret) -> Resultat<bool> {
    let sub = if vrai(g(&a.v, "subrogation")) { s(&a.v, "subrogation") } else { String::new() };
    Ok(sub == droit::arrets::bloc_60()?.subrogation_oui()?)
}

fn nature_inconnue(nature: &str) -> String {
    format!("Nature {} : ce module n'ecrit que {} et {}.", repr(nature), ARRET, REPRISE)
}

fn jours(a: &Arret, k: &str) -> Resultat<Jour> {
    d(g(&a.v, k))
}

/// Ce que la norme refuse (`Echec::Impossible`) ; rend les avertissements.
pub fn verifier(a: &Arret, nature: &str, constitution: Option<&Jour>) -> R<Vec<String>> {
    let v = &a.v;
    if nature != ARRET && nature != REPRISE {
        return impossible(nature_inconnue(nature));
    }
    let bloc = droit::arrets::bloc_60()?;
    let motif = match bloc.motifs_arret.code(g(v, "motif").as_str().unwrap_or_default()) {
        Some(m) => m.clone(),
        None => {
            return impossible(format!(
                "Motif d'arret {} inconnu de la norme (S21.G00.60.001). Les motifs sont dans arrets-base.json.",
                repr_valeur(g(v, "motif"))
            ))
        }
    };
    let m_s = s(v, "motif");
    let libelle = motif.libelle.clone();

    if motif.annulation {
        return impossible(
            "Le motif « 99 - annulation » est interdit dans un signalement d'arret ou de reprise (CCH-12). Un \
signalement deja depose se reprend par un annule-et-remplace : meme salarie, meme contrat, type 03, et \
l'identifiant du dernier envoi valide."
                .into(),
        );
    }
    if motif.temps_partiel_therapeutique {
        return impossible(format!(
            "Le motif {} (temps partiel therapeutique) est interdit dans un signalement (CCH-14) : il ne se \
declare qu'en DSN mensuelle ou en FCTU, et impose un bloc « Temps partiel Therapeutique - S21.G00.66 » que le \
moteur n'ecrit pas.",
            m_s
        ));
    }
    if motif.reserve {
        return impossible(format!(
            "Le motif {} est une valeur de reserve de la norme : elle n'a pas encore de sens declare.",
            m_s
        ));
    }

    let travaille = jours(a, "dernier_jour_travaille")?;
    let debut = jours(a, "debut")?;
    let fin_prev = jours(a, "fin_previsionnelle")?;
    let debut_contrat = jours(a, "contrat_debut")?;

    // Le dernier jour travaille est la veille du debut prescrit, ou ce jour meme quand il a ete travaille
    // (S21.G00.60.002) : un arret ne commence pas avant un jour travaille.
    if debut < travaille {
        return impossible(format!(
            "Début de l'arrêt ({}) avant le dernier jour travaillé ({}) : corriger l'une des deux dates. L'arrêt commence le \
jour que l'avis prescrit, le dernier jour travaillé est la veille ou ce jour même (S21.G00.60.002).",
            debut.jj_mm_aaaa(),
            travaille.jj_mm_aaaa()
        ));
    }
    if debut > fin_prev {
        return impossible(format!(
            "Début de l'arrêt ({}) après sa fin prescrite ({}) : corriger l'une des deux dates de l'avis.",
            debut.jj_mm_aaaa(),
            fin_prev.jj_mm_aaaa()
        ));
    }
    let dernier = engagement::imprevus::dernier_jour_declare(&a.forme()?)?;
    if dernier > fin_prev {
        return impossible(format!(
            "Dernier jour travaille ({}) posterieur a la fin previsionnelle de l'arret ({}) -- CCH-12.",
            dernier.jj_mm_aaaa(),
            s(v, "fin_previsionnelle")
        ));
    }
    if dernier < debut_contrat {
        return impossible(format!(
            "Dernier jour travaille ({}) anterieur au debut du contrat ({}) -- CCH-18. Ce n'est pas ce \
contrat-la qui est suspendu.",
            dernier.jj_mm_aaaa(),
            s(v, "contrat_debut")
        ));
    }

    if motif.date_accident {
        if !vrai(g(v, "date_accident")) {
            return impossible(format!(
                "Motif {} ({}) : la date de l'accident ou de la premiere constatation (S21.G00.60.012) est \
obligatoire -- CCH-11.",
                m_s, libelle
            ));
        }
        if jours(a, "date_accident")? < debut_contrat {
            return impossible(format!(
                "Date de l'accident ({}) anterieure au debut du contrat ({}) -- SIG-14.",
                s(v, "date_accident"),
                s(v, "contrat_debut")
            ));
        }
    } else if vrai(g(v, "date_accident")) {
        return impossible(format!(
            "Motif {} ({}) : la date de l'accident (S21.G00.60.012) est INTERDITE -- CCH-11. Elle n'est due que \
pour un accident de trajet, un accident du travail ou une maladie professionnelle.",
            m_s, libelle
        ));
    }

    if nature == ARRET {
        if !vrai(g(v, "subrogation")) {
            return impossible(
                "La subrogation (S21.G00.60.004) est obligatoire dans un signalement d'arret de travail : dire \
oui ou non. Elle ne se devine pas -- repondre oui engage la compagnie a percevoir les indemnites journalieres \
et a maintenir le salaire."
                    .into(),
            );
        }
        let subs = &bloc.subrogation.codes;
        if bloc.subrogation.code(g(v, "subrogation").as_str().unwrap_or_default()).is_none() {
            let liste: Vec<String> = subs
                .iter()
                .map(|c| format!("{} ({})", c.code, c.libelle))
                .collect();
            return impossible(format!(
                "Subrogation {} : la norme ne connait que {}.",
                repr_valeur(g(v, "subrogation")),
                liste.join(", ")
            ));
        }
        if avec_subrogation(a)? {
            if !(vrai(g(v, "subrogation_debut")) && vrai(g(v, "subrogation_fin"))) {
                return impossible(
                    "Subrogation demandee : ses dates de debut et de fin (S21.G00.60.005 et .006) sont \
obligatoires -- CCH-11."
                        .into(),
                );
            }
            if jours(a, "subrogation_fin")? < jours(a, "subrogation_debut")? {
                return impossible(format!(
                    "Fin de subrogation ({}) anterieure a son debut ({}) -- CCH-12.",
                    s(v, "subrogation_fin"),
                    s(v, "subrogation_debut")
                ));
            }
        } else if vrai(g(v, "subrogation_debut")) || vrai(g(v, "subrogation_fin")) {
            return impossible(
                "Sans subrogation, ses dates sont INTERDITES (CCH-11 des rubriques S21.G00.60.005 et .006)."
                    .into(),
            );
        }
    }

    if vrai(g(v, "reprise")) {
        let rep = jours(a, "reprise")?;
        if rep < dernier || rep < debut_contrat {
            return impossible(format!(
                "Date de reprise ({}) anterieure au dernier jour travaille ({}) ou au debut du contrat ({}) -- \
CCH-11.",
                s(v, "reprise"),
                dernier.jj_mm_aaaa(),
                s(v, "contrat_debut")
            ));
        }
        if rep > fin_prev {
            return impossible(format!(
                "Date de reprise ({}) posterieure a la fin previsionnelle de l'arret ({}) : ce n'est pas une \
reprise anticipee, et la rubrique ne doit alors pas etre valorisee (S21.G00.60.010). Si l'arret s'est \
prolonge, c'est sa fin previsionnelle qui change (S21.G00.60.003, sans nouveau signalement : fiche net-entreprises \
1983) : prolonger l'arret d'abord.",
                s(v, "reprise"),
                s(v, "fin_previsionnelle")
            ));
        }
    }
    if nature == REPRISE {
        if !vrai(g(v, "reprise")) {
            return impossible(
                "Un signalement de reprise sans date de reprise (S21.G00.60.010, usage O). Et il ne s'emet \
qu'en cas de reprise ANTICIPEE par rapport a la fin previsionnelle."
                    .into(),
            );
        }
        if jours(a, "reprise")? == fin_prev {
            return impossible(format!(
                "Reprise le {}, soit la fin previsionnelle de l'arret : le signalement de reprise ne s'emet \
qu'en cas de reprise ANTICIPEE (cahier technique §5.4). Rien n'est a declarer ; la DSN mensuelle portera la \
reprise.",
                s(v, "reprise")
            ));
        }
        if !vrai(g(v, "reprise_motif")) {
            return impossible(
                "Un signalement de reprise sans motif de reprise (S21.G00.60.011, usage O).".into(),
            );
        }
    }
    if vrai(g(v, "reprise_motif")) {
        let mr = match bloc.motifs_reprise.code(g(v, "reprise_motif").as_str().unwrap_or_default()) {
            Some(m) => m,
            None => {
                return impossible(format!(
                    "Motif de reprise {} inconnu de la norme (S21.G00.60.011).",
                    repr_valeur(g(v, "reprise_motif"))
                ))
            }
        };
        if mr.temps_partiel_therapeutique {
            return impossible(format!(
                "Le motif de reprise {} (temps partiel therapeutique) est interdit dans un signalement -- \
CCH-12.",
                s(v, "reprise_motif")
            ));
        }
        if !vrai(g(v, "reprise")) {
            return impossible(
                "Un motif de reprise sans date de reprise : les deux vont ensemble.".into(),
            );
        }
    }

    // Ce qui avertit sans bloquer (§4.4.13).
    let mut avertissements = Vec::new();
    let jour = match constitution {
        Some(j) => *j,
        None => aujourdhui()?,
    };
    for (libelle, cle) in [
        ("Dernier jour travaillé", "dernier_jour_travaille"),
        ("Début de subrogation", "subrogation_debut"),
        ("Date de reprise", "reprise"),
        ("Date de l'accident", "date_accident"),
    ] {
        if vrai(g(v, cle)) && jours(a, cle)? > jour {
            avertissements.push(format!(
                "{} le {}, après le jour où le fichier est fait ({}) : le cahier technique (§4.4.13) le signale sans bloquer, \
un fait déjà survenu ne se date pas dans le futur.",
                libelle,
                jours(a, cle)?.jj_mm_aaaa(),
                jour.jj_mm_aaaa()
            ));
        }
    }
    // UN MOTIF QUE LA DSN NE COUVRE PAS (le deuil d'enfant, 19) : la norme l'admet dans un signalement (S21.G00.60.001,
    // aucune regle CCH ne l'en exclut), donc rien ne se refuse (regle 7) ; mais l'Assurance Maladie ne le traite pas, et
    // l'attestation de salaire se fait sur net-entreprises. La phrase est dans `obligations-base.json` (`sauf_motifs`).
    if let Some(a) = avertissement_hors_dsn(nature, &m_s)? {
        avertissements.push(a);
    }
    Ok(avertissements)
}

/// Les options de `generer`.
pub struct Options {
    pub nature: String,
    pub type_declaration: String,
    pub ordre: i64,
    pub remplace: Option<String>,
    pub envoi_test: bool,
    pub date_constitution: Option<Jour>,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            nature: ARRET.into(),
            type_declaration: NORMALE.into(),
            ordre: 1,
            remplace: None,
            envoi_test: false,
            date_constitution: None,
        }
    }
}

/// Le fichier d'un signalement, pret a deposer. `departs` : ce qui est parti, pour l'identite declaree (`ntt::poser_ntt`).
pub fn generer(ctx: &Contexte, a: &Arret, o: &Options, departs: &engagement::departs::Departs) -> R<String> {
    let nature = o.nature.as_str();
    if nature != ARRET && nature != REPRISE {
        return impossible(nature_inconnue(nature));
    }
    let td = o.type_declaration.as_str();
    if td != NORMALE && td != ANNULE_ET_REMPLACE {
        return impossible(format!(
            "Type de declaration {}. Un signalement d'arret ou de reprise est « 01 - normale » ou « 03 - annule \
et remplace integral » ; le « 04 - annule » n'existe que pour un FCTU et un signalement d'amorcage (cahier \
technique §1.4.3.4).",
            repr(td)
        ));
    }
    let remplace = o.remplace.as_deref().filter(|r| !r.is_empty());
    if td == ANNULE_ET_REMPLACE && remplace.is_none() {
        return impossible(
            "Un annule-et-remplace doit viser la declaration qu'il remplace (S20.G00.05.006) : l'identifiant du \
dernier envoi VALIDE par le point de depot, lu sur son certificat de conformite. Sans lui, la plate-forme rend \
B1-208-08."
                .into(),
        );
    }
    if td == NORMALE && remplace.is_some() {
        return impossible(
            "S20.G00.05.006 est interdite sur un signalement normal (CCH-12) : elle ne s'ecrit que sur un \
annule-et-remplace (type 03)."
                .into(),
        );
    }

    // La configuration du dernier jour travaille : celle de la periode que le signalement declare.
    let arret = a.forme()?;
    let ctx = &ctx.au(&engagement::imprevus::dernier_jour_declare(&arret)?)?;
    let ent = &ctx.entreprise;
    let fiche = engagement::imprevus::fiche_du_salarie(ctx, &arret)?;
    let constitution = match o.date_constitution {
        Some(j) => j,
        None => aujourdhui()?,
    };
    verifier(a, nature, Some(&constitution))?;

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
    f.add("S10.G00.00.008", "01");
    f.add_case("S10.G00.01.001", &ent.siren, "siren")?;
    f.add_case("S10.G00.01.002", &ent.nic, "nic")?;
    f.add_case("S10.G00.01.003", &ent.raison_sociale, "raison_sociale")?;
    f.add_case("S10.G00.01.004", &ent.adresse, "adresse")?;
    f.add_case("S10.G00.01.005", &ent.code_postal, "code_postal")?;
    f.add_case("S10.G00.01.006", &ent.commune, "commune")?;
    f.add_complement("S10.G00.01.009", &ent.complement_adresse);
    dsn::contact_emetteur(&mut f, ent)?;

    // ---------------------------------------------------------- S20 declar.
    f.add("S20.G00.05.001", nature);
    f.add("S20.G00.05.002", td);
    f.add("S20.G00.05.003", "11");
    f.add("S20.G00.05.004", o.ordre.to_string());
    if let Some(r) = remplace {
        f.add("S20.G00.05.006", r);
    }
    f.add("S20.G00.05.007", constitution.jjmmaaaa());
    crate::dsn::contact_chez_le_declare(&mut f, ent)?;

    // ------------------------------------------------------ S21 entreprise
    f.add_case("S21.G00.06.001", &ent.siren, "siren")?;
    f.add_case("S21.G00.06.002", &ent.nic, "nic")?;
    // L'etablissement est celui du salarie, comme dans ses mensuelles (`droit::bareme::entreprise_du_salarie`) : le NIC est
    // une donnee identifiante (cahier technique DSN 2026.1, § 2.5.1, p. 67).
    let etab = droit::bareme::entreprise_du_salarie(ent, &fiche)?;
    f.add_case("S21.G00.11.001", &etab.nic, "nic")?;
    f.add_case("S21.G00.11.003", &etab.adresse, "adresse")?;
    f.add_case("S21.G00.11.004", &etab.code_postal, "code_postal")?;
    f.add_case("S21.G00.11.005", &etab.commune, "commune")?;
    f.add_complement("S21.G00.11.006", &etab.complement_adresse);

    // Les adhesions qui valent le dernier jour travaille (`droit::adhesions`).
    let orgs = ctx.organismes()?;
    let jour = engagement::imprevus::dernier_jour_declare(&arret)?;
    for ad in orgs.adhesions_prevoyance.iter().filter(|ad| ad.en_vigueur(&jour, &jour)) {
        f.add("S21.G00.15.001", &ad.libelle);
        f.add("S21.G00.15.002", &ad.delegataire);
        f.add("S21.G00.15.005", &ad.reference);
    }

    // ------------------------------------------------------- S21 individu
    crate::ntt::poser_nir(&mut f, &fiche)?;
    f.add_case("S21.G00.30.002", &fiche.nom, "nom")?;
    f.add_case("S21.G00.30.004", &fiche.prenom, "prenom")?;
    f.add("S21.G00.30.006", fiche.date_naissance.exigee("date_naissance")?.jjmmaaaa());
    f.add_case("S21.G00.30.019", &fiche.code_salarie, "code_salarie")?;
    crate::ntt::poser_ntt(&mut f, ent, &fiche,crate::changements_individu::derniere_declaree(ctx, &fiche, &engagement::imprevus::dernier_jour_declare(&arret)?, departs)?.as_ref())?;

    // -------------------------------------------------------- S21 contrat
    f.add("S21.G00.40.001", jours(a, "contrat_debut")?.jjmmaaaa());
    f.add_v("S21.G00.40.009", g(&a.v, "contrat_numero"))?;
    f.add_case("S21.G00.40.019", &etab.siret, "siret")?;

    if ent.banque.est_absente() {
        return Err(socle::fichiers::absent("banque").into());
    }
    let banque = ent.banque.dite();
    if nature != REPRISE && vrai(g(&a.v, "subrogation")) && avec_subrogation(a)? && !banque.is_some_and(|b| b.iban.remplie().is_some() && b.bic.remplie().is_some()) {
        return impossible(
            "Subrogation demandee : l'IBAN et le BIC de la compagnie (S21.G00.60.007 et .008) sont obligatoires \
-- CCH-11. Ils se lisent dans la fiche entreprise, ils ne se saisissent pas dans l'arret."
                .into(),
        );
    }
    dsn::bloc_60(&mut f, a, nature, banque)?;

    let n = f.lignes.len() + 2;
    f.add("S90.G00.90.001", n);
    f.add("S90.G00.90.002", "1");
    let texte = f.texte();
    crate::usages::verifier_fichier(&texte)?;
    crate::formats::verifier_fichier(&texte)?;
    Ok(texte)
}

/// L'avertissement d'un signalement de nature `nature` (04 ou 05) dont le motif n'est pas couvert par la DSN
/// (`sauf_motifs.avertissement`) : la norme l'admet, le moteur ne le refuse pas (regle 7), il dit que l'Assurance Maladie
/// ne le traite pas et ce qui se fait a la place. `None` pour un motif couvert. Appele par `verifier`.
pub fn avertissement_hors_dsn(nature: &str, motif: &str) -> Result<Option<String>, String> {
    let base = crate::obligations::base()?;
    let o = match nature {
        ARRET => base.obligations.signalement_arret,
        REPRISE => base.obligations.signalement_reprise,
        _ => return Ok(None),
    };
    Ok(o.sauf_motifs.filter(|s| s.codes.iter().any(|m| m == motif)).map(|s| s.avertissement).filter(|t| !t.is_empty()))
}

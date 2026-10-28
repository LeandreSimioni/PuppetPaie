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
use socle::fichiers::{dossier_du_code, json_en_cache, repr, repr_valeur, vrai};
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

/// `v[k]` : la cle doit exister (`KeyError`).
fn req<'a>(v: &'a Value, k: &str) -> Resultat<&'a Value> {
    v.get(k).ok_or_else(|| socle::fichiers::absent(k))
}

fn s(v: &Value, k: &str) -> String {
    en_texte(g(v, k))
}

fn nomenclatures() -> Resultat<std::sync::Arc<Value>> {
    json_en_cache(&dossier_du_code().join("arrets-base.json"))
}

/// Les codes de `arrets-base.json[quoi]`, dans l'ordre du fichier.
fn codes(quoi: &str) -> Resultat<Vec<Value>> {
    let n = nomenclatures()?;
    Ok(n[quoi]["codes"].as_array().cloned().unwrap_or_default())
}

fn code<'a>(liste: &'a [Value], c: &Value) -> Option<&'a Value> {
    let c = c.as_str()?;
    liste.iter().find(|x| x["code"].as_str() == Some(c))
}

fn code_subrogation_oui() -> Resultat<String> {
    for c in codes("subrogation")? {
        if c["libelle"].as_str() == Some("oui") {
            return Ok(en_texte(&c["code"]));
        }
    }
    Err("subrogation : pas de code « oui » dans arrets-base.json".into())
}

pub fn avec_subrogation(a: &Arret) -> Resultat<bool> {
    let sub = if vrai(g(&a.v, "subrogation")) { s(&a.v, "subrogation") } else { String::new() };
    Ok(sub == code_subrogation_oui()?)
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
    let motifs = codes("motifs_arret")?;
    let motif = match code(&motifs, g(v, "motif")) {
        Some(m) => m.clone(),
        None => {
            return impossible(format!(
                "Motif d'arret {} inconnu de la norme (S21.G00.60.001). Les motifs sont dans arrets-base.json.",
                repr_valeur(g(v, "motif"))
            ))
        }
    };
    let m_s = s(v, "motif");
    let libelle = en_texte(g(&motif, "libelle"));

    if vrai(g(&motif, "annulation")) {
        return impossible(
            "Le motif « 99 - annulation » est interdit dans un signalement d'arret ou de reprise (CCH-12). Un \
signalement deja depose se reprend par un annule-et-remplace : meme salarie, meme contrat, type 03, et \
l'identifiant du dernier envoi valide."
                .into(),
        );
    }
    if vrai(g(&motif, "temps_partiel_therapeutique")) {
        return impossible(format!(
            "Le motif {} (temps partiel therapeutique) est interdit dans un signalement (CCH-14) : il ne se \
declare qu'en DSN mensuelle ou en FCTU, et impose un bloc « Temps partiel Therapeutique - S21.G00.66 » que le \
moteur n'ecrit pas.",
            m_s
        ));
    }
    if vrai(g(&motif, "reserve")) {
        return impossible(format!(
            "Le motif {} est une valeur de reserve de la norme : elle n'a pas encore de sens declare.",
            m_s
        ));
    }

    let dernier = jours(a, "dernier_jour_travaille")?;
    let fin_prev = jours(a, "fin_previsionnelle")?;
    let debut_contrat = jours(a, "contrat_debut")?;

    if dernier > fin_prev {
        return impossible(format!(
            "Dernier jour travaille ({}) posterieur a la fin previsionnelle de l'arret ({}) -- CCH-12.",
            s(v, "dernier_jour_travaille"),
            s(v, "fin_previsionnelle")
        ));
    }
    if dernier < debut_contrat {
        return impossible(format!(
            "Dernier jour travaille ({}) anterieur au debut du contrat ({}) -- CCH-18. Ce n'est pas ce \
contrat-la qui est suspendu.",
            s(v, "dernier_jour_travaille"),
            s(v, "contrat_debut")
        ));
    }

    if vrai(g(&motif, "date_accident")) {
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
        let subs = codes("subrogation")?;
        if code(&subs, g(v, "subrogation")).is_none() {
            let liste: Vec<String> = subs
                .iter()
                .map(|c| format!("{} ({})", en_texte(&c["code"]), en_texte(&c["libelle"])))
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
                s(v, "dernier_jour_travaille"),
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
        let reprises = codes("motifs_reprise")?;
        let mr = match code(&reprises, g(v, "reprise_motif")) {
            Some(m) => m,
            None => {
                return impossible(format!(
                    "Motif de reprise {} inconnu de la norme (S21.G00.60.011).",
                    repr_valeur(g(v, "reprise_motif"))
                ))
            }
        };
        if vrai(g(mr, "temps_partiel_therapeutique")) {
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
    for (libelle, rubrique, cle) in [
        ("dernier jour travaille", "S21.G00.60.002", "dernier_jour_travaille"),
        ("debut de subrogation", "S21.G00.60.005", "subrogation_debut"),
        ("date de reprise", "S21.G00.60.010", "reprise"),
        ("date de l'accident", "S21.G00.60.012", "date_accident"),
    ] {
        if vrai(g(v, cle)) && jours(a, cle)? > jour {
            avertissements.push(format!(
                "{} ({}) au {} : posterieur a la constitution du fichier ({}). Le cahier technique (§4.4.13) le \
signale sans bloquer -- un evenement deja survenu ne se date pas dans le futur.",
                libelle,
                rubrique,
                s(v, cle),
                jour.iso()
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

/// Le fichier d'un signalement, pret a deposer.
pub fn generer(ctx: &Contexte, a: &Arret, o: &Options) -> R<String> {
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
    let ctx = &ctx.au(&jours(a, "dernier_jour_travaille")?)?;
    let ent = &ctx.entreprise;
    let salarie = ctx.salarie(&en_texte(g(&a.v, "salarie")))?;
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
    f.add_v("S10.G00.01.001", req(ent, "siren")?)?;
    f.add_v("S10.G00.01.002", req(ent, "nic")?)?;
    f.add_v("S10.G00.01.003", req(ent, "raison_sociale")?)?;
    f.add_v("S10.G00.01.004", req(ent, "adresse")?)?;
    f.add_v("S10.G00.01.005", req(ent, "code_postal")?)?;
    f.add_v("S10.G00.01.006", req(ent, "commune")?)?;
    dsn::contact_emetteur(&mut f, ent)?;
    let decl = req(ent, "declarant_dsn")?;

    // ---------------------------------------------------------- S20 declar.
    f.add("S20.G00.05.001", nature);
    f.add("S20.G00.05.002", td);
    f.add("S20.G00.05.003", "11");
    f.add("S20.G00.05.004", o.ordre.to_string());
    if let Some(r) = remplace {
        f.add("S20.G00.05.006", r);
    }
    f.add("S20.G00.05.007", constitution.jjmmaaaa());
    f.add_v("S20.G00.07.001", req(decl, "nom_prenom")?)?;
    f.add_v("S20.G00.07.002", req(decl, "tel")?)?;
    f.add_v("S20.G00.07.003", req(decl, "email")?)?;
    f.add_v("S20.G00.07.004", &crate::dsn::type_du_contact(decl))?;

    // ------------------------------------------------------ S21 entreprise
    f.add_v("S21.G00.06.001", req(ent, "siren")?)?;
    f.add_v("S21.G00.06.002", req(ent, "nic")?)?;
    // L'etablissement est celui du salarie, comme dans ses mensuelles (`dsn::etablissement_du_salarie`) : le NIC est
    // une donnee identifiante (cahier technique DSN 2026.1, § 2.5.1, p. 67).
    let etab = dsn::etablissement_du_salarie(ent, &salarie)?;
    f.add_v("S21.G00.11.001", req(&etab, "nic")?)?;
    f.add_v("S21.G00.11.003", req(&etab, "adresse")?)?;
    f.add_v("S21.G00.11.004", req(&etab, "code_postal")?)?;
    f.add_v("S21.G00.11.005", req(&etab, "commune")?)?;

    // Les adhesions qui valent le dernier jour travaille (`droit::adhesions`).
    let orgs = ctx.organismes_exiges()?;
    let jour = jours(a, "dernier_jour_travaille")?;
    for ad in req(&orgs, "adhesions_prevoyance")?.as_array().map(Vec::as_slice).unwrap_or(&[]) {
        if !droit::adhesions::en_vigueur(ad, &jour, &jour)? {
            continue;
        }
        f.add_v("S21.G00.15.001", req(ad, "libelle")?)?;
        f.add_v("S21.G00.15.002", req(ad, "delegataire")?)?;
        f.add_v("S21.G00.15.005", req(ad, "reference")?)?;
    }

    // ------------------------------------------------------- S21 individu
    crate::ntt::poser_nir(&mut f, &salarie)?;
    f.add_v("S21.G00.30.002", req(&salarie, "nom")?)?;
    f.add_v("S21.G00.30.004", req(&salarie, "prenom")?)?;
    f.add("S21.G00.30.006", d(req(&salarie, "date_naissance")?)?.jjmmaaaa());
    f.add_v("S21.G00.30.019", req(&salarie, "code_salarie")?)?;
    crate::ntt::poser_ntt(&mut f, ent, &salarie, &crate::ntt::brute(ctx, &salarie), &jours(a, "dernier_jour_travaille")?)?;

    // -------------------------------------------------------- S21 contrat
    f.add("S21.G00.40.001", jours(a, "contrat_debut")?.jjmmaaaa());
    f.add_v("S21.G00.40.009", g(&a.v, "contrat_numero"))?;
    f.add_v("S21.G00.40.019", req(&etab, "siret")?)?;

    let banque = req(ent, "banque")?;
    if nature != REPRISE && vrai(g(&a.v, "subrogation")) && avec_subrogation(a)? && !(vrai(g(banque, "iban")) && vrai(g(banque, "bic"))) {
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

/// Les motifs pour lesquels un signalement DSN ne nait pas (`sauf_motifs.codes`) : ceux que la DSN ne couvre pas.
pub fn sauf_motifs(o: &Value) -> Vec<String> {
    o.get("sauf_motifs").and_then(|s| s.get("codes")).and_then(Value::as_array).map(|a| a.iter().map(en_texte).collect()).unwrap_or_default()
}

/// L'avertissement d'un signalement de nature `nature` (04 ou 05) dont le motif n'est pas couvert par la DSN
/// (`sauf_motifs.avertissement`) : la norme l'admet, le moteur ne le refuse pas (regle 7), il dit que l'Assurance Maladie
/// ne le traite pas et ce qui se fait a la place. `None` pour un motif couvert. Appele par `verifier`.
pub fn avertissement_hors_dsn(nature: &str, motif: &str) -> Result<Option<String>, String> {
    let cle = match nature {
        ARRET => "signalement_arret",
        REPRISE => "signalement_reprise",
        _ => return Ok(None),
    };
    let n = json_en_cache(&dossier_du_code().join("obligations-base.json"))?;
    let o = n.get("obligations").and_then(|o| o.get(cle)).ok_or_else(|| format!("obligations-base.json : l'obligation {} manque", repr(cle)))?;
    if !sauf_motifs(o).iter().any(|m| m == motif) {
        return Ok(None);
    }
    Ok(o["sauf_motifs"].get("avertissement").and_then(Value::as_str).filter(|t| !t.is_empty()).map(str::to_string))
}

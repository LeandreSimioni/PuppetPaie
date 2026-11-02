//! Les routes des IMPREVUS d'un salarie : la liste (`imprevus`, `arrets`, `fins-de-contrat`, `arret-contrats`), l'arret
//! saisi, repris, prolonge, signale ou oublie (`arret-enregistrer`, `arret-reprise`, `arret-prolonger`,
//! `arret-signaler`, `arret-oublier`), ses indemnites journalieres (`arret-indemnites`), la suspension du bloc 65
//! (`suspension-enregistrer`, `-modifier`, `-oublier`), et la fin d'un contrat signalee, avec ses documents
//! (`fin-contrat-signaler`, `fin-contrat-documents`).
//!
//! RIEN NE PART D'ICI : un imprevu s'ecrit dans `imprevus/AAAA-MM.json`, et c'est la production du mois qui en tire la
//! ligne de paie et le bloc de la DSN ; un signalement s'ecrit en fichier, il ne s'envoie pas. Les refus sont des
//! `ErreurOperation` (la page les montre tels quels) ; un refus n'ecrit rien.
//!
//! CE QUI S'Y SAISIT, ET CE QUI L'APPELLE (rien de remplissable s'il n'y a pas a le remplir). Le moteur dit, sur chaque
//! imprevu resume (`resume`), ce que ses faits ouvrent : `ij_attendues` (l'arret ouvre le maintien de salaire, par la loi
//! ou la convention : ses indemnites journalieres se disent, `arret-indemnites`), `ij_majoree_attendue` et
//! `jours_deja_indemnises_attendus`. La page n'ouvre que cela.

use std::collections::HashMap;

use serde_json::{json, Map, Value};

use crate::routes::{brut, maintenant, qui_de, salarie};
use declarations::signalement;
use droit::contexte::Contexte;
use engagement::calcul::Paie;
use engagement::imprevus::{self, ARRET, SUSPENSION};
use envois::etats::{get, getd, items, objet_vide, ou, Dico};
use socle::date::{d, en_texte, repr_texte, Jour};
use socle::echec::{erreur, Echec, R};
use socle::fichiers::{dossier_du_code, json_en_cache, repr, vrai};
use socle::num::Decimal;
use socle::valeur::{nom_type, valeur_ecrite, valeur_en_texte, valeurs_egales};


use crate::routes::catalogue::{Acte, Affiche, Champ, Forme, Fiche, FORME, ILLISIBLE, MANQUE, MOIS, OPERATION, SALARIE};
use crate::routes::catalogue::{Ecran, Place};

const DERNIER_JOUR: Champ = Champ { nom: "dernier_jour", libelle: Affiche::new("Dernier jour travaillé"), forme: Forme::Date, sens: "le dernier jour travaillé avant l'arrêt, qui le désigne", avance: false };
const DEBUT: Champ = Champ { nom: "debut", libelle: Affiche::new("Premier jour d'absence"), forme: Forme::Date, sens: "le premier jour de l'absence, qui la désigne", avance: false };
const UN_ARRET: &str = r#"{"salarie":"FULLER","dernier_jour":"2026-11-27"}"#;

/// Une fiche de ce module : `rend` et les refus que toutes partagent.
const fn fiche(nom: &'static str, geste: &'static str, charge: &'static [Champ], exemple: &'static str, rend: &'static str, acte: Acte, avance: bool) -> Fiche {
    Fiche { nom, geste, charge, exemple, rend, refus: &[OPERATION, MANQUE, ILLISIBLE, FORME], acte, avance, phrase: 0, ecran: ECRAN }
}

const ORDRE: Champ = Champ { nom: "ordre", libelle: Affiche::new("Numéro d'ordre"), forme: Forme::Nombre, sens: "son numéro d'ordre ; sinon le suivant de ceux déjà partis", avance: true };
const REMPLACE: Champ = Champ { nom: "remplace", libelle: Affiche::new("Remplace"), forme: Forme::Texte, sens: "le signalement qu'il remplace", avance: true };
const TEST: Champ = Champ { nom: "test", libelle: Affiche::new("Fichier d'essai"), forme: Forme::OuiNon, sens: "vrai : un fichier d'essai, marqué comme tel", avance: true };

/// Ou ces routes se voient a l'ecran (une place par module).
const ECRAN: Ecran = Ecran::Dessine(Place::Paie);

/// Les routes de ce module, et leurs fiches, dans l'ordre d'un arrêt : il s'enregistre, se suit, se signale, s'oublie.
pub const ROUTES: &[Fiche] = &[
    fiche("imprevus", "Lire les imprévus d'un mois ou d'un salarié, et ce qu'on peut en saisir.",
        &[MOIS, Champ { nom: "salarie", libelle: Affiche::new("Salarié"), forme: Forme::Texte, sens: "à la place du mois : les imprévus de ce salarié", avance: true }],
        r#"{"mois":"2026-11"}"#, "imprevus (chacun résumé, un arrêt avec ses obligations), nomenclatures", Acte::Lit, false),
    Fiche { ecran: Ecran::Auto(Place::Exports, Affiche::new("Lire tous les arrêts")), ..fiche("arrets", "Lire tous les arrêts de la compagnie.", &[], "{}", "phrase, phrases (qui, quand, le motif, ce qui reste à signaler), arrets : chacun résumé", Acte::Lit, true) },
    fiche("fins-de-contrat", "Lire les fins de contrat dues ce mois, et ce qui manque à chacune.", &[MOIS],
        r#"{"mois":"2026-11"}"#, "fins : chacune avec ce qui est fait et ce qui manque", Acte::Lit, false),
    fiche("arret-contrats", "Lire les contrats qu'un arrêt suspendrait à ce jour.",
        &[SALARIE, Champ { nom: "jour", libelle: Affiche::new("Dernier jour travaillé"), forme: Forme::Date, sens: "le dernier jour travaillé", avance: false }],
        r#"{"salarie":"FULLER","jour":"2026-11-27"}"#, "contrats : ceux du salarié qui couvrent ce jour", Acte::Lit, false),
    Fiche { phrase: 40, ..fiche("arret-enregistrer", "Enregistrer un arrêt de travail.",
        &[
            SALARIE, DERNIER_JOUR,
            Champ { nom: "debut", libelle: Affiche::new("Début de l'arrêt"), forme: Forme::Date, sens: "le premier jour que l'avis prescrit ; le dernier jour travaillé s'il a été prescrit ce jour-là", avance: false },
            Champ { nom: "fin", libelle: Affiche::new("Fin prescrite"), forme: Forme::Date, sens: "la fin prescrite", avance: false },
            Champ { nom: "motif", libelle: Affiche::new("Motif"), forme: Forme::Texte, sens: "le code du motif d'arrêt (nomenclatures de imprevus)", avance: false },
            Champ { nom: "date_accident", libelle: Affiche::new("Jour de l'accident"), forme: Forme::Date, sens: "le jour de l'accident, quand le motif l'appelle", avance: true },
            Champ { nom: "connu", libelle: Affiche::new("Su le"), forme: Forme::Objet, sens: "{jour, heure} où la compagnie a su l'accident, quand le motif l'appelle", avance: true },
            Champ { nom: "contrat", libelle: Affiche::new("Contrat"), forme: Forme::Texte, sens: "le contrat suspendu, quand plusieurs couvrent le jour", avance: true },
            Champ { nom: "jours_deja_indemnises", libelle: Affiche::new("Jours déjà indemnisés"), forme: Forme::Nombre, sens: "pour le maintien d'un permanent", avance: true },
            Champ { nom: "note", libelle: Affiche::new("Note"), forme: Forme::Texte, sens: "une note libre", avance: true },
        ],
        r#"{"salarie":"FULLER","dernier_jour":"2026-11-27","debut":"2026-11-30","fin":"2026-11-30","motif":"01"}"#,
        "arret (l'arrêt résumé), phrase", Acte::Ecrit, false) },
    Fiche { phrase: 700, ..fiche("arret-prolonger", "Prolonger un arrêt : sa nouvelle fin.",
        &[SALARIE, DERNIER_JOUR, Champ { nom: "fin", libelle: Affiche::new("Nouvelle fin prescrite"), forme: Forme::Date, sens: "la nouvelle fin prescrite", avance: false }],
        r#"{"salarie":"FULLER","dernier_jour":"2026-11-27","fin":"2026-12-04"}"#,
        "arret (l'arrêt résumé), phrase", Acte::Ecrit, false) },
    Fiche { phrase: 20, ..fiche("arret-reprise", "Poser ou effacer la reprise d'un arrêt.",
        &[SALARIE, DERNIER_JOUR, Champ { nom: "reprise", libelle: Affiche::new("Reprise le"), forme: Forme::Date, sens: "le jour de la reprise ; vide l'efface", avance: false }, Champ { nom: "motif_reprise", libelle: Affiche::new("Motif de la reprise"), forme: Forme::Texte, sens: "le motif de la reprise", avance: true }],
        r#"{"salarie":"FULLER","dernier_jour":"2026-11-27","reprise":"2026-12-03"}"#, "arret (l'arrêt résumé), phrase", Acte::Ecrit, false) },
    Fiche { phrase: 50, ..fiche("arret-indemnites", "Dire les indemnités journalières d'un arrêt, par jour.",
        &[SALARIE, DERNIER_JOUR,
            Champ { nom: "ij_brute", libelle: Affiche::new("Indemnité brute d'un jour"), forme: Forme::Nombre, sens: "l'indemnité d'un jour, avant CSG et CRDS (relevé de la CPAM)", avance: false },
            Champ { nom: "ij_nette", libelle: Affiche::new("Indemnité nette d'un jour"), forme: Forme::Nombre, sens: "la même, nette de CSG et CRDS", avance: false },
            Champ { nom: "ij_brute_majoree", libelle: Affiche::new("Indemnité brute majorée"), forme: Forme::Nombre, sens: "accident du travail, dès le 29e jour : brute", avance: true },
            Champ { nom: "ij_nette_majoree", libelle: Affiche::new("Indemnité nette majorée"), forme: Forme::Nombre, sens: "la même, nette", avance: true },
            Champ { nom: "jours_deja_indemnises", libelle: Affiche::new("Jours déjà indemnisés"), forme: Forme::Nombre, sens: "les jours indemnisés dans les douze mois qu'aucun arrêt saisi ne porte", avance: true }],
        r#"{"salarie":"SVOBODA","dernier_jour":"2026-11-02","ij_brute":"40.89","ij_nette":"38.15"}"#, "arret (l'arrêt résumé), phrase", Acte::Ecrit, false) },
    Fiche { phrase: 600, ..fiche("arret-signaler", "Fabriquer le signalement d'un arrêt, sans le déposer.",
        &[SALARIE, DERNIER_JOUR, Champ { nom: "nature", libelle: Affiche::new("Nature"), forme: Forme::Texte, sens: "04 arrêt (par défaut), ou 05 reprise", avance: true }, REMPLACE, ORDRE, TEST,
            Champ { nom: "le", libelle: Affiche::new("Constitué le"), forme: Forme::Date, sens: "le jour de constitution ; sinon aujourd'hui", avance: true }],
        UN_ARRET, "fichier (écrit dans sorties/AAAA-MM/), phrase, nature, type, ordre, avertissements", Acte::Ecrit, false) },
    Fiche { phrase: 60, ..fiche("arret-oublier", "Retirer un arrêt qui n'a pas été déclaré.", &[SALARIE, DERNIER_JOUR],
        UN_ARRET, "arret (l'arrêt retiré, résumé), phrase", Acte::Ecrit, false) },
    Fiche { phrase: 80, ..fiche("suspension-enregistrer", "Enregistrer une absence qui suspend un contrat.",
        &[SALARIE, DEBUT, Champ { nom: "fin", libelle: Affiche::new("Dernier jour"), forme: Forme::Date, sens: "son dernier jour", avance: false }, Champ { nom: "motif", libelle: Affiche::new("Motif"), forme: Forme::Texte, sens: "le code du motif (nomenclatures de imprevus)", avance: false }, Champ { nom: "contrat", libelle: Affiche::new("Contrat"), forme: Forme::Texte, sens: "le contrat, quand plusieurs couvrent le jour", avance: true }, Champ { nom: "note", libelle: Affiche::new("Note"), forme: Forme::Texte, sens: "une note libre", avance: true }],
        r#"{"salarie":"FULLER","debut":"2026-11-27","fin":"2026-11-27","motif":"501"}"#, "suspension (l'absence résumée), phrase", Acte::Ecrit, true) },
    Fiche { phrase: 20, ..fiche("suspension-modifier", "Changer la fin ou le motif d'une absence.",
        &[SALARIE, DEBUT, Champ { nom: "fin", libelle: Affiche::new("Nouvelle fin"), forme: Forme::Date, sens: "sa nouvelle fin", avance: false }, Champ { nom: "motif", libelle: Affiche::new("Nouveau motif"), forme: Forme::Texte, sens: "son nouveau motif", avance: true }],
        r#"{"salarie":"FULLER","debut":"2026-11-27","fin":"2026-11-27"}"#, "suspension (l'absence résumée), phrase", Acte::Ecrit, true) },
    Fiche { phrase: 60, ..fiche("suspension-oublier", "Retirer une absence qui n'a pas été déclarée.", &[SALARIE, DEBUT],
        r#"{"salarie":"FULLER","debut":"2026-11-27"}"#, "suspension (l'absence retirée, résumée), phrase", Acte::Ecrit, true) },
    Fiche { phrase: 600, ..fiche("fin-contrat-signaler", "Fabriquer le signalement d'une fin de contrat, sans le déposer.",
        &[MOIS, SALARIE, REMPLACE, ORDRE, TEST, Champ { nom: "le", libelle: Affiche::new("Constitué le"), forme: Forme::Date, sens: "le jour de constitution ; sinon aujourd'hui", avance: true }],
        r#"{"mois":"2026-11","salarie":"FULLER"}"#, "fichier (écrit dans sorties/AAAA-MM/), phrase, type, ordre, preavis, avertissements", Acte::Ecrit, true) },
    Fiche { phrase: 160, ..fiche("fin-contrat-documents", "Fabriquer le certificat de travail et le reçu pour solde de tout compte.",
        &[MOIS, SALARIE, Champ { nom: "le", libelle: Affiche::new("Établis le"), forme: Forme::Date, sens: "le jour où ils s'établissent ; sinon aujourd'hui", avance: true }],
        r#"{"mois":"2026-11","salarie":"FULLER"}"#, "phrase, certificat et recu (leurs chemins), dossier, avertissements (ce qu'il faut savoir en les remettant)", Acte::Ecrit, false) },
];

/// Une route de ce module.
pub fn route(_o: &HashMap<String, String>, ctx: &Contexte, nom: &str, charge: &Value) -> R<Value> {
    match nom {
        "imprevus" => imprevus(ctx, charge).map(|mut v| {
            let n = super::nombre(&v["imprevus"]);
            let lignes: Vec<Value> = v["imprevus"].as_array().into_iter().flatten().flat_map(|i| {
                let t = |k: &str| super::mot(&i[k]);
                let etat = match t("etat").as_str() {
                    "a signaler" => "à signaler".to_string(),
                    "signale" => "signalé".to_string(),
                    autre => autre.to_string(),
                };
                let mut l = vec![json!(format!("{} ({}) : {}, du {} au {} — {}", t("salarie"), t("contrat"), t("motif_libelle"), super::quand(&t("premier_jour")), super::quand(&t("dernier_jour")), etat))];
                l.extend(crate::obligations::en_phrases(&i["obligations"]).into_iter().map(|o| json!(format!("  {}", o))));
                l
            }).collect();
            let p = format!("Imprévus : {}.", n);
            v["phrases"] = Value::Array(lignes);
            super::phrase_en_tete(v, p)
        }),
        "arrets" => arrets(ctx),
        "fins-de-contrat" => fins_de_contrat(ctx, charge).map(|v| {
            let p = format!("Fins de contrat du mois : {}.", super::nombre(&v["fins"]));
            super::phrase_en_tete(v, p)
        }),
        "arret-contrats" => arret_contrats(ctx, charge).map(|v| {
            let cle = super::mot(&charge["salarie"]);
            let nom = ctx.salaries.get(&cle).and_then(socle::texte::salarie_nomme).unwrap_or(cle);
            let p = match super::nombre(&v["contrats"]) {
                0 => format!("Aucun contrat de {} ne couvre le {} : il n'y a rien à suspendre.", nom, super::jour_en_clair(&charge["jour"])),
                n => format!("Contrats qu'un arrêt peut toucher : {}.", n),
            };
            // Chaque contrat se dit en une ligne : son numéro et sa période.
            let mut v = v;
            for c in v["contrats"].as_array_mut().into_iter().flatten() {
                let numero = if vrai(&c["numero"]) { valeur_en_texte(&c["numero"]) } else { "sans numéro".to_string() };
                c["dit"] = json!(format!("{}, {}", numero, super::periode_en_clair(&c["debut"], &c["fin"])));
            }
            super::phrase_en_tete(v, p)
        }),
        "arret-enregistrer" => arret_enregistrer(ctx, charge),
        "arret-reprise" => arret_reprise(ctx, charge),
        "arret-oublier" => arret_oublier(ctx, charge),
        "arret-indemnites" => arret_indemnites(ctx, charge),
        "arret-prolonger" => arret_prolonger(ctx, charge),
        "arret-signaler" => arret_signaler(ctx, charge),
        "suspension-enregistrer" => suspension_enregistrer(ctx, charge),
        "suspension-modifier" => suspension_modifier(ctx, charge),
        "suspension-oublier" => suspension_oublier(ctx, charge),
        "fin-contrat-signaler" => fin_contrat_signaler(ctx, charge),
        "fin-contrat-documents" => fin_contrat_documents(ctx, charge),
        autre => Err(Echec::from(format!("route inconnue : {}", autre))),
    }
}

// ------------------------------------------------------------------ outils

#[track_caller]
pub(crate) fn refus(m: impl Into<String>) -> Echec {
    erreur("ErreurOperation", m)
}

fn exiger<'a>(charge: &'a Value, cle: &str) -> R<&'a Value> {
    charge.get(cle).ok_or_else(|| erreur("KeyError", socle::fichiers::absent(cle)))
}

fn optionnel<'a>(charge: &'a Value, cle: &str) -> &'a Value {
    charge.get(cle).unwrap_or(&Value::Null)
}

pub(crate) fn jour(v: &Value) -> R<Jour> {
    d(v).map_err(|m| erreur("ValueError", m))
}

fn magasin<T>(r: Result<T, String>) -> R<T> {
    r.map_err(refus_magasin)
}

/// La cle de la fiche que `nom` designe (`KeyError` si aucune).
pub(crate) fn cle_du_salarie(ctx: &Contexte, nom: &Value) -> R<String> {
    let fiche = crate::routes::salarie(ctx, nom)?;
    Ok(valeur_en_texte(fiche.get("_cle").ok_or_else(|| erreur("KeyError", socle::fichiers::absent("_cle")))?))
}

/// LE CONTRAT QU'UN IMPREVU SUSPEND : parmi ceux du salarie qui couvrent `jour` (`crate::routes::imprevus::contrats_au`), celui
/// que la charge nomme (`contrat`), sinon le seul. Un imprevu designe son contrat par son numero et sa date de debut
/// (S21.G00.40.009 et .001) ; sans contrat, il n'y a rien a suspendre, et a deux, le cahier technique laisse le choix au
/// declarant : on le lui demande.
pub(crate) fn contrat_suspendu(ctx: &Contexte, cle: &str, jour: &Jour, voulu: &Value) -> R<Value> {
    let candidats = crate::routes::imprevus::contrats_au(ctx, &json!(cle), &jour.iso())?;
    let texte_de = |c: &Value, k: &str| valeur_en_texte(&c[k]);
    if vrai(voulu) {
        return match candidats.iter().find(|c| valeurs_egales(&c["numero"], voulu)) {
            Some(c) => Ok(c.clone()),
            None => {
                let liste: Vec<String> = candidats.iter().map(|c| texte_de(c, "numero")).collect();
                Err(refus(format!(
                    "Contrat {} introuvable parmi ceux de {} qui couvrent le {} : {}",
                    valeur_en_texte(voulu),
                    cle,
                    jour.jj_mm_aaaa(),
                    if liste.is_empty() { "aucun".to_string() } else { liste.join(", ") }
                )))
            }
        };
    }
    match candidats.len() {
        1 => Ok(candidats[0].clone()),
        0 => Err(refus(format!(
            "Aucun contrat de {} ne couvre le {}. Un arret ou une suspension designe le contrat suspendu par son numero et sa \
date de debut (S21.G00.40.009 et .001) : sans contrat declare, il n'y a rien a suspendre. Verifier la date, ou saisir le \
contrat.",
            cle,
            jour.jj_mm_aaaa()
        ))),
        _ => {
            let liste: Vec<String> =
                candidats.iter().map(|c| format!("{} du {} au {}", texte_de(c, "numero"), texte_de(c, "debut"), texte_de(c, "fin"))).collect();
            Err(refus(format!(
                "Deux contrats de {} couvrent le {} ({}). Le cahier technique laisse le choix au declarant : dire lequel.",
                cle,
                jour.jj_mm_aaaa(),
                liste.join(", ")
            )))
        }
    }
}

/// UN IMPREVU NE CHEVAUCHE PAS UN AUTRE IMPREVU DU MEME CONTRAT : un jour d'absence ne se retire qu'une fois du salaire,
/// et deux periodes qui se recouvrent sur le meme contrat sont deux declarations qui se contredisent (un salarie ne
/// travaille pas le dernier jour travaille d'un arret qui court deja). `sauf` : l'imprevu lui-meme, quand il change.
pub(crate) fn sans_chevauchement(ctx: &Contexte, nouveau: &Value, sauf: Option<&Value>) -> R<()> {
    let (p, dj) = (magasin(imprevus::premier_jour(nouveau))?, magasin(imprevus::dernier_jour(nouveau))?);
    let cle = valeur_en_texte(&nouveau["salarie"]);
    let debut = jour(&nouveau["contrat_debut"])?;
    let numero = valeur_en_texte(&nouveau["contrat_numero"]);
    for autre in magasin(imprevus::tous(&ctx.racine))? {
        if sauf.is_some_and(|s| meme_imprevu(s, &autre)) || meme_imprevu(nouveau, &autre) {
            continue;
        }
        if !magasin(imprevus::du_contrat(&autre, &cle, Some(numero.as_str()), &debut))? {
            continue;
        }
        let (ap, ad) = (magasin(imprevus::premier_jour(&autre))?, magasin(imprevus::dernier_jour(&autre))?);
        if ap <= dj && ad >= p {
            return Err(refus(format!(
                "{} : du {} au {}, cet imprevu chevauche {} du {} au {} sur le meme contrat ({}). Un jour d'absence ne se \
declare et ne se retire qu'une fois : corriger les dates, ou reprendre l'autre imprevu.",
                cle,
                p.jj_mm_aaaa(),
                dj.jj_mm_aaaa(),
                if imprevus::est_un_arret(&autre) { "l'arret de travail" } else { "la suspension" },
                ap.jj_mm_aaaa(),
                ad.jj_mm_aaaa(),
                if numero.is_empty() { debut.jj_mm_aaaa() } else { numero.clone() }
            )));
        }
    }
    Ok(())
}

/// Le meme imprevu : meme genre, meme salarie, meme jour de rangement.
fn meme_imprevu(a: &Value, b: &Value) -> bool {
    imprevus::genre(a) == imprevus::genre(b)
        && valeurs_egales(&a["salarie"], &b["salarie"])
        && imprevus::jour_de_rangement(a).ok() == imprevus::jour_de_rangement(b).ok()
}

/// L'imprevu de ce genre, de ce salarie, a ce jour de rangement (le dernier jour travaille d'un arret, le debut d'une
/// suspension) : son indice et le fichier de son mois, tous genres compris.
pub(crate) fn trouver(ctx: &Contexte, genre: &str, cle: &str, jour: &Jour) -> R<Option<(usize, Vec<Value>)>> {
    let liste = magasin(imprevus::du_fichier(&ctx.racine, &jour.mois_iso()))?;
    let i = liste.iter().position(|x| {
        imprevus::genre(x) == genre && valeur_en_texte(&x["salarie"]) == cle && imprevus::jour_de_rangement(x).ok() == Some(*jour)
    });
    Ok(i.map(|i| (i, liste)))
}

/// Ecrit le fichier d'un mois du magasin.
pub(crate) fn ecrire(ctx: &Contexte, mois: &str, liste: Vec<Value>) -> R<()> {
    imprevus::ecrire(&ctx.racine, mois, liste).map_err(|m| erreur("OSError", m))
}

/// Les DSN mensuelles qui ont reporte l'imprevu (`obligations::avec_ce_qui_est_parti`, nature 01).
fn reporte_par(ctx: &Contexte, imp: &Value) -> R<Vec<Value>> {
    let imp = crate::obligations::avec_ce_qui_est_parti(ctx, imp)?;
    Ok(imp["envois"].as_array().map_or_else(Vec::new, |e| {
        e.iter().filter(|x| x.get("nature").and_then(Value::as_str) == Some("01")).map(|x| x.get("mois").cloned().unwrap_or(Value::Null)).collect()
    }))
}

/// Un signalement de cette nature (04, 05) est-il parti pour cet arret (`obligations::avec_ce_qui_est_parti`) ?
fn envoye(ctx: &Contexte, imp: &Value, nature: &str) -> R<bool> {
    let imp = crate::obligations::avec_ce_qui_est_parti(ctx, imp)?;
    Ok(imp["envois"].as_array().is_some_and(|e| e.iter().any(|x| x.get("nature").and_then(Value::as_str) == Some(nature))))
}

/// UN ARRET EN COURS : celui dont on peut encore apprendre qu'il se prolonge ou que le salarie a repris plus
/// tot. Aucune reprise n'est saisie, et :
///
/// - sa fin previsionnelle n'est pas passee : le salarie est arrete aujourd'hui ;
/// - ou elle est passee, mais la DSN mensuelle du mois ou elle tombe n'est pas encore partie : l'avis de prolongation peut
///   arriver apres la fin prescrite, et une reprise anticipee se signale encore. Une fois cette DSN partie (`reporte_par`),
///   l'arret y est declare fini a sa fin previsionnelle -- « la reprise du travail de votre salarie a la date prevue doit
///   etre signalee dans la DSN mensuelle » (ameli, `atelier/reference/sources/ameli-arret-de-travail/`) --, et il n'est
///   plus en cours.
///
/// Les routes elles-memes ne s'en servent pas pour refuser : une prolongation se saisit tant qu'aucune reprise ne l'est
/// (`arret_prolonger`). C'est ce que la page offre, pas ce que le moteur admet.
pub(crate) fn en_cours(ctx: &Contexte, imp: &Value) -> R<bool> {
    if vrai(&imp["reprise"]) {
        return Ok(false);
    }
    let fin = jour(&imp["fin_previsionnelle"])?;
    if fin >= ctx.aujourdhui {
        return Ok(true);
    }
    Ok(!reporte_par(ctx, imp)?.iter().any(|m| valeur_en_texte(m) == fin.mois_iso()))
}

// ------------------------------------------------------------------ le resume

/// UN IMPREVU TEL QUE LA PAGE LE LIT. Un arret : le resume de toujours (`crate::routes::imprevus::resume_arret` : salarie, contrat,
/// motif et son libelle, les dates de la norme, signale, signalements, reporte_par, etat), plus `genre`, `premier_jour`,
/// `dernier_jour`, `jours_deja_indemnises`, les indemnites par jour (`ij_brute`, `ij_nette`, `ij_brute_majoree`,
/// `ij_nette_majoree`), `origine`, `ij_attendues` (l'arret ouvre le maintien), `ij_majoree_attendue` (un accident du travail
/// au-dela du vingt-huitieme jour, CSS R433-3), `jours_deja_indemnises_attendus`, `cachets`. Une suspension : `genre`, `salarie`, `contrat`,
/// `contrat_debut`, `motif`, `motif_libelle`, `remunere`, `debut`, `fin`, `premier_jour`, `dernier_jour`, `note`,
/// `saisi_le`, `reporte_par`, `cachets`.
pub(crate) fn resume(ctx: &Contexte, n: &Value, imp: &Value) -> R<Value> {
    // Chaque cachet dit son mois tel que l'écran le lit (« de novembre 2026 »).
    let cachets: Vec<Value> = magasin(imprevus::cachets_dans(ctx, imp))?.into_iter().map(|mut c| { c["mois_dit"] = json!(crate::regularisation::de_mois_en_clair(&valeur_en_texte(&c["mois"]))); c }).collect();
    let premier = magasin(imprevus::premier_jour(imp))?.iso();
    let dernier = magasin(imprevus::dernier_jour(imp))?.iso();
    if imprevus::est_un_arret(imp) {
        let mut r = crate::routes::imprevus::resume_arret(ctx, n, imp)?;
        let motif = magasin(imprevus::motif_d_arret(&valeur_en_texte(&imp["motif"])))?.unwrap_or(Value::Null);
        let ouvert = ouvre_le_maintien(ctx, imp)?;
        let majoree = ouvert
            && engagement::maintien::origine(imp).is_some_and(|o| o.professionnelle() || o == droit::maintien::Origine::Trajet)
            && magasin(imprevus::dernier_jour(imp))?.moins(&magasin(imprevus::premier_jour(imp))?) >= 28;
        let o = r.as_object_mut().ok_or_else(|| erreur("TypeError", "resume"))?;
        o.insert("genre".into(), json!(ARRET));
        o.insert("premier_jour".into(), json!(premier));
        o.insert("dernier_jour".into(), json!(dernier));
        o.insert("jours_deja_indemnises".into(), imp["jours_deja_indemnises"].clone());
        for k in ["ij_brute", "ij_nette", "ij_brute_majoree", "ij_nette_majoree"] {
            o.insert(k.into(), imp[k].clone());
        }
        o.insert("origine".into(), motif.get("origine").cloned().unwrap_or(Value::Null));
        o.insert("ij_attendues".into(), json!(ouvert));
        o.insert("ij_majoree_attendue".into(), json!(majoree));
        o.insert("jours_deja_indemnises_attendus".into(), json!(ouvert));
        o.insert("cachets".into(), json!(cachets));
        // CE QUE L'ARRET OUVRE A LA PAGE (rien de remplissable s'il n'y a pas a le remplir) :
        // un arret en cours se prolonge ou se reprend plus tot ; une reprise saisie se corrige tant que son signalement
        // n'est pas parti.
        let cours = en_cours(ctx, imp)?;
        o.insert("prolongations".into(), imp["prolongations"].clone());
        o.insert("en_cours".into(), json!(cours));
        o.insert("peut_prolonger".into(), json!(cours));
        o.insert("peut_reprendre_plus_tot".into(), json!(cours));
        o.insert("reprise_corrigeable".into(), json!(vrai(&imp["reprise"]) && !envoye(ctx, imp, declarations::signalement::REPRISE)?));
        return Ok(dire_l_imprevu(r));
    }
    let reportee = reporte_par(ctx, imp)?;
    let motif = magasin(imprevus::motif_de_suspension(&valeur_en_texte(&imp["motif"])))?.unwrap_or(Value::Null);
    Ok(dire_l_imprevu(json!({
        "genre": SUSPENSION,
        "salarie": imp["salarie"], "contrat": imp["contrat_numero"], "contrat_debut": imp["contrat_debut"],
        "motif": imp["motif"], "motif_libelle": motif.get("libelle").cloned().unwrap_or(json!("?")),
        "remunere": motif.get("remunere").is_some_and(vrai),
        "debut": imp["debut"], "fin": imp["fin"], "premier_jour": premier, "dernier_jour": dernier,
        "note": imp["note"], "saisi_le": imp["saisi_le"], "reporte_par_dit": mois_de_la_dsn(&reportee), "reporte_par": reportee,
        "cachets": cachets,
    })))
}

/// Les motifs d'une suspension qui se DECLARENT (`permanents-base.json`, section `suspensions`, sans le 200).
fn motifs_de_suspension() -> R<Vec<Value>> {
    let regles = droit::permanents::regles().map_err(refus_magasin)?;
    Ok(regles["suspensions"]["motifs"]
        .as_array()
        .map_or(&[][..], |a| &a[..])
        .iter()
        .filter(|m| !m.get("_ne_pas_declarer").is_some_and(vrai))
        .map(|m| {
            let remunere = m.get("remunere").is_some_and(vrai);
            let dit = format!("{}{}", valeur_en_texte(&m["libelle"]), if remunere { " (payé)" } else { "" });
            json!({"code": m["code"], "libelle": m["libelle"], "libelle_dit": dit, "remunere": remunere})
        })
        .collect())
}

// ------------------------------------------------------------------ imprevus

/// La route `imprevus` : `{salarie?, mois?}` -> les imprevus (d'un salarie, qui croisent un mois), resumes -- un arret
/// avec ses `obligations` --, et les nomenclatures pour en saisir.
fn imprevus(ctx: &Contexte, charge: &Value) -> R<Value> {
    let cle = match charge.get("salarie").filter(|v| vrai(v)) {
        Some(nom) => Some(cle_du_salarie(ctx, nom)?),
        None => None,
    };
    let mois = charge.get("mois").filter(|v| vrai(v)).map(valeur_en_texte);
    let bornes = match &mois {
        Some(m) => Some((
            socle::date::premier_du_mois(m).map_err(|e| erreur("ValueError", e))?,
            socle::date::dernier_du_mois(m).map_err(|e| erreur("ValueError", e))?,
        )),
        None => None,
    };
    let n = crate::routes::imprevus::nomenclatures(ctx)?;
    let suivi = crate::obligations::Suivi::lire(ctx)?;
    let mut out = Vec::new();
    for imp in magasin(imprevus::tous(&ctx.racine))? {
        if cle.as_deref().is_some_and(|c| valeur_en_texte(&imp["salarie"]) != c) {
            continue;
        }
        if let Some((debut, fin)) = &bornes {
            if !magasin(imprevus::croise(&imp, debut, fin))? {
                continue;
            }
        }
        let mut r = resume(ctx, &n, &imp)?;
        // CE QU'UN ARRET FAIT NAITRE, SUR L'ARRET LUI-MEME : la page des imprevus (la carte d'un salarie, la
        // colonne du mois, son dossier) lit une seule liste, et chaque arret y porte ses obligations, calculees au meme
        // endroit que pour la route `arrets` (`obligations::de_l_arret`).
        if imprevus::est_un_arret(&imp) {
            r["obligations"] = Value::Array(crate::obligations::de_l_arret(ctx, &imp, &suivi, ctx.aujourdhui)?);
        }
        out.push(dire_ce_qui_est_parti(r));
    }
    Ok(json!({
        "imprevus": out,
        "motifs_arret": crate::routes::imprevus::saisissables(&n, "motifs_arret")?,
        "motifs_reprise": crate::routes::imprevus::saisissables(&n, "motifs_reprise")?,
        "motifs_suspension": motifs_de_suspension()?,
    }))
}

// ------------------------------------------------------------------ le maintien

/// Un entier positif ou nul, ou rien (`null`, `""`).
pub(crate) fn jours_ou_rien(v: &Value, quoi: &str) -> R<Value> {
    if v.is_null() || v.as_str().is_some_and(|t| t.trim().is_empty()) {
        return Ok(Value::Null);
    }
    let n = match v {
        Value::Number(x) => x.as_i64().filter(|_| x.as_f64().is_some_and(|f| f.fract() == 0.0)),
        Value::String(t) => t.trim().parse::<i64>().ok(),
        _ => None,
    };
    match n {
        Some(k) if k >= 0 => Ok(json!(k)),
        _ => Err(refus(format!("{} : un nombre entier de jours, positif ou nul, est attendu ({}).", quoi, valeur_en_texte(v)))),
    }
}

/// `arret-indemnites` : `{salarie, dernier_jour, ij_brute?, ij_nette?, ij_brute_majoree?, ij_nette_majoree?,
/// jours_deja_indemnises?}`. Les indemnites journalieres d'un arret, par jour, telles que les ecrit le releve de la caisse ou
/// son bordereau de paiement (BPIJ : le prix unitaire de l'« I.J. normale » ou « majoree », puis les retenues CSG et RDS,
/// `atelier/reference/sources/subrogation-indemnites-journalieres/bpij-edi/`) : brutes, et nettes de CSG et CRDS. Le
/// maintien de salaire s'en deduit (`engagement::maintien`). Un champ vide l'efface. La page ne les demande que pour un arret qui
/// ouvre le maintien (`ij_attendues` du resume) ; ailleurs, elles ne se deduisent de rien.
fn arret_indemnites(ctx: &Contexte, charge: &Value) -> R<Value> {
    let cle = cle_du_salarie(ctx, exiger(charge, "salarie")?)?;
    let dernier = jour(exiger(charge, "dernier_jour")?)?;
    let Some((i, mut liste)) = trouver(ctx, ARRET, &cle, &dernier)? else {
        return Err(refus(format!("Aucun arret de {} dont le dernier jour travaille soit le {}.", cle, dernier.jj_mm_aaaa())));
    };
    let a = liste[i].clone();
    let champs = ["ij_brute", "ij_nette", "ij_brute_majoree", "ij_nette_majoree", "jours_deja_indemnises"];
    if !champs.iter().any(|k| charge.get(*k).is_some()) {
        return Err(refus("Dire les indemnités journalières (brute et nette, par jour) ou les jours déjà indemnisés : rien n'est changé."));
    }
    let mut nouveau = a.clone();
    for k in &champs[..4] {
        if let Some(v) = charge.get(*k) {
            let vide = v.is_null() || v.as_str().is_some_and(|t| t.trim().is_empty());
            nouveau[*k] = if vide {
                Value::Null
            } else {
                let x = socle::num::dec_avec_echelle(v).map_err(|e| refus(format!("Indemnité journalière ({}) : {}", k, e)))?;
                if x <= Decimal::ZERO {
                    return Err(refus("Une indemnité journalière est un montant par jour, positif : rien n'est changé."));
                }
                json!(socle::num::eur2(x).to_string())
            };
        }
    }
    if let Some(j) = charge.get("jours_deja_indemnises") {
        nouveau["jours_deja_indemnises"] = jours_ou_rien(j, "Jours deja indemnises")?;
    }
    imprevus::complet(&nouveau).map_err(refus)?;
    liste[i] = nouveau.clone();
    ecrire(ctx, &dernier.mois_iso(), liste)?;
    let n = crate::routes::imprevus::nomenclatures(ctx)?;
    Ok(json!({"arret": resume(ctx, &n, &nouveau)?, "phrase": "Indemnités journalières notées."}))
}

/// La nature du contrat qu'un imprevu suspend, lue dans la saisie qui le porte ; `None` s'il n'y est plus.
pub(crate) fn nature_du_contrat(ctx: &Contexte, imp: &Value) -> R<Option<std::sync::Arc<droit::natures::Nature>>> {
    let debut = valeur_en_texte(&imp["contrat_debut"]);
    let contrats = crate::routes::imprevus::contrats_au(ctx, &imp["salarie"], &debut)?;
    let Some(c) = contrats.iter().find(|c| valeur_en_texte(&c["debut"]) == debut) else { return Ok(None) };
    let cle = c["nature"].as_str().filter(|n| !n.is_empty());
    Ok(Some(droit::natures::nature(cle).map_err(refus)?))
}

/// L'arret ouvre-t-il le maintien de salaire, par la loi ou la convention (`engagement::maintien::droits`) ?
fn ouvre_le_maintien(ctx: &Contexte, imp: &Value) -> R<bool> {
    let Some(nat) = nature_du_contrat(ctx, imp)? else { return Ok(false) };
    Ok(magasin(engagement::maintien::droits(ctx, imp, &nat))?.is_some_and(|d| d.du()))
}

/// LA SUBROGATION D'UN ARRET, telle que le moteur la tire de la convention et du choix de la compagnie
/// (`engagement::maintien::subrogation`) : `01` et ses dates, ou `02`.
pub(crate) fn subrogation_de(ctx: &Contexte, imp: &Value) -> R<(Value, Value, Value)> {
    let sub = match nature_du_contrat(ctx, imp)? {
        Some(nat) => magasin(engagement::maintien::subrogation(ctx, imp, &nat))?,
        None => None,
    };
    Ok(match sub {
        Some((d, f)) => (json!("01"), json!(d.iso()), json!(f.iso())),
        None => (json!("02"), Value::Null, Value::Null),
    })
}

// ------------------------------------------------------------------ la prolongation

/// `arret-prolonger` : `{salarie, dernier_jour, fin}` -> `{"arret": resume, "phrase"}`. L'AVIS DE PROLONGATION D'UN ARRET EN COURS.
///
/// CE QUI PART, ET CE QUI NE PART PAS. « En cas de prolongation d'arret, il ne faut pas envoyer de nouveau signalement de
/// nature " arret de travail ". Il suffit de declarer la nouvelle date de fin de l'arret en DSN mensuelle, au niveau de la
/// rubrique " Date de fin previsionnelle - S21.G00.60.003 " » (fiche net-entreprises 1983, `atelier/reference/sources/
/// net-entreprises-base-connaissances/net-entreprises-fiche1983-prolongation-arret-dates-de-subrogation-20250322.txt`, l.
/// 13) ; ameli dit de meme a l'employeur en DSN : « modifier la date de fin previsionnelle de l'arret de travail dans votre
/// logiciel de paie » (`atelier/reference/sources/ameli-arret-de-travail/`). La route ne change donc que
/// `fin_previsionnelle` : le bloc S21.G00.60 de chaque DSN mensuelle que l'arret traverse la porte (`dsn::bloc_60`,
/// `imprevus::du_mois`, qui suit la nouvelle fin), la paie d'un permanent retire les jours ajoutes et le maintien s'ouvre
/// aux mois ajoutes (`imprevus::lignes_du_mois`, `maintien`), les cachets payes dans la prolongation s'avertissent
/// (`imprevus::cachets_dans`) -- par le mecanisme de toujours, sans second. Un signalement d'arret pas encore parti partira
/// avec la nouvelle fin.
///
/// LA SUBROGATION. Elle n'est portee que par le signalement d'arret (60.004 a 60.008 : C en 01, mais le moteur ne l'ecrit
/// qu'en 04 -- `dsn::bloc_60`, tableau des usages) ; sa fin « maximale prevue dans le cadre de la convention collective ou
/// de l'accord de branche » se declare d'emblee (fiche 1983, l. 25). Si elle est « insuffisante ou erronee », un nouveau
/// signalement d'arret de type annule et remplace la porte (l. 37), « le plus rapidement possible des connaissance de la
/// prolongation » (l. 66) ; si elle « court toujours », rien (l. 110). Le moteur tire ce maximum du droit
/// (`engagement::maintien::subrogation`) : il ne bouge qu'avec une subrogation qui dure tant que la caisse indemnise (1285,
/// X.4.7), et suit alors la nouvelle fin. Une fin de subrogation changee apres le
/// depot du signalement d'arret fait naitre le signalement rectificatif (`obligations::de_l_arret`,
/// `signalement_arret_rectificatif`), d'ou la trace de chaque avis dans `prolongations`.
///
/// CE QUI REFUSE, sans rien ecrire : un arret deja repris (« se prolonge sans interruption », ameli : un arret apres la
/// reprise est un nouvel arret) ; une fin qui n'est pas apres la fin prescrite (une fin plus tot est une reprise
/// anticipee, qui se signale : `arret-reprise`) ; ce que la norme refuse au signalement d'arret (`crate::routes::imprevus::verifier`) ; un chevauchement.
fn arret_prolonger(ctx: &Contexte, charge: &Value) -> R<Value> {
    let cle = cle_du_salarie(ctx, exiger(charge, "salarie")?)?;
    let dernier = jour(exiger(charge, "dernier_jour")?)?;
    let Some((i, mut liste)) = trouver(ctx, ARRET, &cle, &dernier)? else {
        return Err(refus(format!("Aucun arret de {} dont le dernier jour travaille soit le {}.", cle, dernier.jj_mm_aaaa())));
    };
    let avant = liste[i].clone();
    if vrai(&avant["reprise"]) {
        return Err(refus(format!(
            "L'arrêt de {} a pris fin : le salarié a repris le {}. Une prolongation continue un arrêt en cours, sans \
interruption ; un arrêt qui commence après la reprise est un nouvel arrêt, qui se saisit et se signale comme tel. Rien \
n'est changé.",
            cle,
            jour(&avant["reprise"])?.jj_mm_aaaa()
        )));
    }
    let f = jour(exiger(charge, "fin")?)?;
    let fin_avant = jour(&avant["fin_previsionnelle"])?;
    let mut a = avant.clone();
    {
        if f < fin_avant {
            return Err(refus(format!(
                "Le {} est avant la fin prescrite de l'arrêt (le {}) : ce n'est pas une prolongation. Si le salarié a repris \
plus tôt, c'est une reprise anticipée, qui se saisit comme telle et se signale (nature 05). Rien n'est changé.",
                f.jj_mm_aaaa(),
                fin_avant.jj_mm_aaaa()
            )));
        }
        if f == fin_avant {
            return Err(refus(format!("L'arrêt est déjà prescrit jusqu'au {} : rien n'est changé.", f.jj_mm_aaaa())));
        }
        a["fin_previsionnelle"] = json!(f.iso());
    }
    // Une subrogation qui dure tant que la caisse indemnise (1285, X.4.7) suit la fin de l'arret ; les autres vont deja
    // jusqu'au maximum du maintien (`engagement::maintien::subrogation`).
    if signalement_subroge(&a)? {
        let (sub, _, sub_fin) = crate::routes::imprevus::subrogation_de(ctx, &a)?;
        if sub == json!("01") {
            a["subrogation_fin"] = sub_fin;
        }
    }
    crate::routes::imprevus::verifier(&a)?;
    sans_chevauchement(ctx, &a, Some(&avant))?;

    // La trace de l'avis : le jour ou la compagnie l'a su (le jour du moteur), l'heure de la saisie, et ce qui a change.
    let trace = json!({
        "le": ctx.aujourdhui.iso(),
        "saisi_le": crate::routes::maintenant(),
        "qui": crate::routes::qui_de(charge),
        "fin_avant": avant["fin_previsionnelle"], "fin": a["fin_previsionnelle"],
        "subrogation_fin_avant": avant["subrogation_fin"], "subrogation_fin": a["subrogation_fin"],
    });
    match a["prolongations"].as_array_mut() {
        Some(l) => l.push(trace),
        None => a["prolongations"] = json!([trace]),
    }
    liste[i] = a.clone();
    ecrire(ctx, &dernier.mois_iso(), liste)?;

    let nouvelle_fin = jour(&a["fin_previsionnelle"])?;
    let mut avertissements = Vec::new();
    if nouvelle_fin > fin_avant {
        avertissements.push(json!(format!(
            "Prolongé jusqu'au {}. Rien ne part : pas de nouveau signalement d'arrêt ; la DSN mensuelle de chaque mois que \
l'arrêt traverse porte la nouvelle fin prévisionnelle (S21.G00.60.003, fiche net-entreprises 1983).",
            nouvelle_fin.jj_mm_aaaa()
        )));
        // Les jours ajoutes qui tombent dans un mois dont la DSN reelle est deja partie : elle portait l'ancienne fin.
        let declares = reporte_par(ctx, &avant)?;
        for m in socle::date::mois_entre(&fin_avant.plus_jours(1).map_err(|e| erreur("ValueError", e))?, &nouvelle_fin) {
            if declares.iter().any(|x| valeur_en_texte(x) == m) {
                avertissements.push(json!(format!(
                    "La DSN de {} est déjà partie avec l'ancienne fin (le {}) : la paie de ce mois, où les jours ajoutés \
tombent, est à corriger.",
                    engagement::libelle_mois(&m),
                    fin_avant.jj_mm_aaaa()
                )));
            }
        }
    }
    let sub_change = a["subrogation_fin"] != avant["subrogation_fin"];
    if sub_change && envoye(ctx, &avant, declarations::signalement::ARRET)? {
        avertissements.push(json!(
            "Le signalement d'arrêt est déjà parti avec l'ancienne fin de subrogation : un signalement d'arrêt annule et \
remplace porte la nouvelle, le plus vite possible (fiche net-entreprises 1983). Il paraît dans ce qui est dû."
        ));
    }
    let phrase = if !avertissements.is_empty() {
        avertissements.iter().map(valeur_en_texte).collect::<Vec<_>>().join("  ·  ")
    } else {
        "Arrêt prolongé.".to_string()
    };
    let n = crate::routes::imprevus::nomenclatures(ctx)?;
    Ok(json!({
        "arret": resume(ctx, &n, &a)?,
        "phrase": phrase,
    }))
}

/// L'arret est-il subroge (S21.G00.60.004 = oui) ?
fn signalement_subroge(a: &Value) -> R<bool> {
    declarations::signalement::avec_subrogation(&declarations::dsn::Arret { v: a.clone() }).map_err(|m| erreur("ValueError", m))
}

// ------------------------------------------------------------------ les suspensions

/// Ce qu'une suspension doit tenir avant de s'ecrire : son motif se declare (`imprevus::verifier_motif_de_suspension` :
/// connu, pas le 200), elle finit apres avoir commence, et ne deborde pas de son contrat.
fn verifier_suspension(s: &Value, contrat_fin: &str) -> R<()> {
    imprevus::verifier_motif_de_suspension(&valeur_en_texte(&s["motif"]), &valeur_en_texte(&s["salarie"])).map_err(refus)?;
    let (debut, fin) = (jour(&s["debut"])?, jour(&s["fin"])?);
    if fin < debut {
        return Err(refus(format!(
            "Suspension du {} au {} : elle finit avant de commencer (S21.G00.65.002 et .003).",
            debut.jj_mm_aaaa(),
            fin.jj_mm_aaaa()
        )));
    }
    if !contrat_fin.is_empty() && fin.iso().as_str() > contrat_fin {
        return Err(refus(format!(
            "Suspension jusqu'au {}, mais le contrat {} finit le {} : une suspension ne survit pas a son contrat.",
            fin.jj_mm_aaaa(),
            valeur_en_texte(&s["contrat_numero"]),
            jour(&json!(contrat_fin))?.jj_mm_aaaa()
        )));
    }
    Ok(())
}

/// La fin du contrat d'une suspension deja ecrite, lue dans les saisies (vide : un contrat qui court).
fn fin_du_contrat(ctx: &Contexte, s: &Value) -> R<String> {
    let debut = valeur_en_texte(&s["contrat_debut"]);
    let contrats = crate::routes::imprevus::contrats_au(ctx, &s["salarie"], &debut)?;
    Ok(contrats.iter().find(|c| valeur_en_texte(&c["debut"]) == debut).map(|c| valeur_en_texte(&c["fin"])).unwrap_or_default())
}

fn note_de(charge: &Value) -> Value {
    let n = optionnel(charge, "note");
    if vrai(n) {
        n.clone()
    } else {
        json!("")
    }
}

/// `suspension-enregistrer` : `{salarie, debut, fin, motif, contrat?, note?}` -> `{"suspension": resume}`.
fn suspension_enregistrer(ctx: &Contexte, charge: &Value) -> R<Value> {
    let cle = cle_du_salarie(ctx, exiger(charge, "salarie")?)?;
    let debut = jour(exiger(charge, "debut")?)?;
    let fin = jour(exiger(charge, "fin")?)?;
    let motif = valeur_en_texte(exiger(charge, "motif")?);
    let contrat = contrat_suspendu(ctx, &cle, &debut, optionnel(charge, "contrat"))?;
    let mut s = Map::new();
    s.insert("genre".into(), json!(SUSPENSION));
    s.insert("salarie".into(), json!(cle));
    s.insert("contrat_debut".into(), contrat["debut"].clone());
    s.insert("contrat_numero".into(), contrat["numero"].clone());
    s.insert("motif".into(), json!(motif));
    s.insert("debut".into(), json!(debut.iso()));
    s.insert("fin".into(), json!(fin.iso()));
    s.insert("saisi_le".into(), json!(crate::routes::maintenant()));
    s.insert("qui".into(), json!(crate::routes::qui_de(charge)));
    s.insert("note".into(), note_de(charge));
    let s = Value::Object(s);
    verifier_suspension(&s, &valeur_en_texte(&contrat["fin"]))?;
    if trouver(ctx, SUSPENSION, &cle, &debut)?.is_some() {
        return Err(refus(format!(
            "Une suspension de {} au {} est deja enregistree : la modifier plutot que d'en saisir une seconde.",
            cle,
            debut.jj_mm_aaaa()
        )));
    }
    sans_chevauchement(ctx, &s, None)?;
    let mois = debut.mois_iso();
    let mut liste = magasin(imprevus::du_fichier(&ctx.racine, &mois))?;
    liste.push(s.clone());
    ecrire(ctx, &mois, liste)?;
    let n = crate::routes::imprevus::nomenclatures(ctx)?;
    Ok(json!({"suspension": resume(ctx, &n, &s)?, "phrase": "Absence enregistrée. Chaque mois qu'elle traverse la déclarera."}))
}

/// `suspension-modifier` : `{salarie, debut, fin?, motif?, note?}` -> `{"suspension": resume}`. Le debut designe la
/// suspension et ne change pas : une suspension qui commence un autre jour s'oublie et se saisit de nouveau.
fn suspension_modifier(ctx: &Contexte, charge: &Value) -> R<Value> {
    let cle = cle_du_salarie(ctx, exiger(charge, "salarie")?)?;
    let debut = jour(exiger(charge, "debut")?)?;
    let Some((i, mut liste)) = trouver(ctx, SUSPENSION, &cle, &debut)? else {
        return Err(refus(format!("Aucune suspension de {} qui commence le {}.", cle, debut.jj_mm_aaaa())));
    };
    let avant = liste[i].clone();
    let mut s = avant.clone();
    if let Some(f) = charge.get("fin") {
        s["fin"] = json!(jour(f)?.iso());
    }
    if let Some(m) = charge.get("motif") {
        s["motif"] = json!(valeur_en_texte(m));
    }
    if charge.get("note").is_some() {
        s["note"] = note_de(charge);
    }
    verifier_suspension(&s, &fin_du_contrat(ctx, &s)?)?;
    sans_chevauchement(ctx, &s, Some(&avant))?;
    liste[i] = s.clone();
    ecrire(ctx, &debut.mois_iso(), liste)?;
    let n = crate::routes::imprevus::nomenclatures(ctx)?;
    Ok(json!({"suspension": resume(ctx, &n, &s)?, "phrase": "Absence changée."}))
}

/// `suspension-oublier` : `{salarie, debut}` -> `{"suspension": resume}`. Une suspension qu'une DSN reelle a deja
/// declaree ne se retire pas : le magasin garderait moins que ce qui est parti.
fn suspension_oublier(ctx: &Contexte, charge: &Value) -> R<Value> {
    let cle = cle_du_salarie(ctx, exiger(charge, "salarie")?)?;
    let debut = jour(exiger(charge, "debut")?)?;
    let Some((i, mut liste)) = trouver(ctx, SUSPENSION, &cle, &debut)? else {
        return Err(refus(format!("Aucune suspension de {} qui commence le {}.", cle, debut.jj_mm_aaaa())));
    };
    let envois = crate::obligations::avec_ce_qui_est_parti(ctx, &liste[i])?["envois"].as_array().map_or(0, Vec::len);
    if envois > 0 {
        return Err(refus(format!(
            "Cette suspension a deja ete declaree ({} DSN). La retirer du magasin n'annule rien aupres des organismes : la \
reprendre (une fin plus tot), ou corriger les DSN deposees.",
            envois
        )));
    }
    let s = liste.remove(i);
    ecrire(ctx, &debut.mois_iso(), liste)?;
    let n = crate::routes::imprevus::nomenclatures(ctx)?;
    Ok(json!({"suspension": resume(ctx, &n, &s)?, "phrase": "Absence retirée : aucune déclaration ne l'avait portée."}))
}

/// `x or None`.
fn ou_rien(v: &Value) -> Value {
    if vrai(v) {
        v.clone()
    } else {
        Value::Null
    }
}


/// Ecrit le fichier d'un mois du magasin des imprevus (`imprevus/AAAA-MM.json`) : les arrets ET les suspensions qu'il
/// range, tels que la liste les donne.
pub(crate) fn ecrire_arrets(ctx: &Contexte, mois: &str, imprevus: Vec<Value>) -> R<()> {
    crate::routes::imprevus::ecrire(ctx, mois, imprevus)
}

/// Ce que la norme refuse (`SignalementImpossible`, une
/// `ValueError`) ; les avertissements ne servent pas ici.
pub(crate) fn verifier(arret: &Value) -> R<()> {
    match signalement::verifier(&declarations::dsn::Arret { v: arret.clone() }, signalement::ARRET, None) {
        Ok(_) => Ok(()),
        Err(signalement::Echec::Impossible(m)) => Err(erreur("ValueError", m)),
        Err(signalement::Echec::Autre(m)) => Err(Echec { classe: None, message: m }),
    }
}

/// `ctx.salarie(salarie)["_cle"]`.
fn cle_en_json(ctx: &Contexte, nom: &Value) -> R<Value> {
    salarie(ctx, nom)?.get("_cle").cloned().ok_or_else(|| erreur("KeyError", socle::fichiers::absent("_cle")))
}

/// L'arret de ce salarie a ce dernier jour travaille : (son indice, le fichier de son mois, tous imprevus compris).
pub(crate) fn trouver_l_arret(ctx: &Contexte, nom: &Value, dernier_jour: &Value) -> R<(usize, Vec<Value>)> {
    let cle = cle_en_json(ctx, nom)?;
    if vrai(dernier_jour) {
        if let Some(t) = crate::routes::imprevus::trouver(ctx, engagement::imprevus::ARRET, &valeur_en_texte(&cle), &jour(dernier_jour)?)? {
            return Ok(t);
        }
    }
    let quand = if vrai(dernier_jour) { documents::bulletin::jj_mm_aaaa(dernier_jour).map_err(|m| erreur("ValueError", m))? } else { "?".to_string() };
    Err(refus(format!(
        "Aucun arret de {} dont le dernier jour travaille soit le {}. La route `arrets` les liste.",
        valeur_en_texte(&cle),
        quand
    )))
}

/// Le dernier jour travaille saisi de l'arret de ce salarie dont un signalement a declare `declare` (S21.G00.60.002,
/// `imprevus::dernier_jour_declare`) : ce qui designe l'arret aux routes ; `declare` lui-meme si aucun arret ne le porte.
pub(crate) fn dernier_jour_saisi(ctx: &Contexte, salarie: &str, declare: &str) -> R<String> {
    for a in tous_les_arrets(ctx)? {
        if valeur_en_texte(&a["salarie"]) == salarie && magasin(imprevus::dernier_jour_declare(&a))?.iso() == declare {
            return Ok(valeur_en_texte(&a["dernier_jour_travaille"]));
        }
    }
    Ok(declare.to_string())
}

/// `{"arret": resume, "phrase"}` : l'arret tel que la page le lit (`crate::routes::imprevus::resume`, avec les cachets qui tombent dedans), et ce qui se lit du geste.
fn reponse_arret(ctx: &Contexte, arret: &Value, phrase: &str) -> R<Value> {
    let n = nomenclatures(ctx)?;
    Ok(json!({"arret": crate::routes::imprevus::resume(ctx, &n, arret)?, "phrase": phrase}))
}

/// `arret-enregistrer` : `{salarie, dernier_jour, debut, fin, motif,
/// date_accident?, jours_deja_indemnises?, contrat?, note?}` -> `{"arret": resume}`. Les controles de la norme sont ceux
/// du signalement (`signalement::verifier`, nature 04), avant toute ecriture ; un arret ne chevauche pas un autre imprevu
/// du meme contrat (`crate::routes::imprevus::sans_chevauchement`).
fn arret_enregistrer(ctx: &Contexte, charge: &Value) -> R<Value> {
    let nom = exiger(charge, "salarie")?;
    let dernier_jour = exiger(charge, "dernier_jour")?;
    let debut = jour(exiger(charge, "debut")?)?;
    let fin = exiger(charge, "fin")?;
    let motif = exiger(charge, "motif")?;
    let cle = cle_en_json(ctx, nom)?;
    let dernier = jour(dernier_jour)?;
    let choisi = crate::routes::imprevus::contrat_suspendu(ctx, &valeur_en_texte(&cle), &dernier, &ou_rien(optionnel(charge, "contrat")))?;

    let jour_ou_rien = |k: &str| -> R<Value> {
        let v = optionnel(charge, k);
        if vrai(v) {
            Ok(json!(jour(v)?.iso()))
        } else {
            Ok(Value::Null)
        }
    };
    let note = {
        let n = optionnel(charge, "note");
        if vrai(n) {
            n.clone()
        } else {
            json!("")
        }
    };
    let deja = crate::routes::imprevus::jours_ou_rien(optionnel(charge, "jours_deja_indemnises"), "Jours deja indemnises")?;
    let mut a = Map::new();
    a.insert("genre".into(), json!(engagement::imprevus::ARRET));
    a.insert("salarie".into(), cle.clone());
    a.insert("contrat_debut".into(), choisi["debut"].clone());
    a.insert("contrat_numero".into(), choisi["numero"].clone());
    a.insert("motif".into(), json!(valeur_en_texte(motif)));
    a.insert("dernier_jour_travaille".into(), json!(dernier.iso()));
    a.insert("debut".into(), json!(debut.iso()));
    a.insert("fin_previsionnelle".into(), json!(jour(fin)?.iso()));
    a.insert("subrogation".into(), Value::Null);
    a.insert("subrogation_debut".into(), Value::Null);
    a.insert("subrogation_fin".into(), Value::Null);
    a.insert("reprise".into(), Value::Null);
    a.insert("reprise_motif".into(), Value::Null);
    a.insert("date_accident".into(), jour_ou_rien("date_accident")?);
    a.insert("jours_deja_indemnises".into(), deja);
    a.insert("prolongations".into(), json!([]));
    a.insert("saisi_le".into(), json!(maintenant()));
    a.insert("qui".into(), json!(qui_de(charge)));
    a.insert("note".into(), note);
    let mut arret = Value::Object(a);
    // Les jours deja indemnises (D1226-4) ne se disent que pour un arret qui ouvre le maintien de salaire.
    if !arret["jours_deja_indemnises"].is_null() && !ouvre_le_maintien(ctx, &arret)? {
        return Err(refus(
            "Les jours deja indemnises (D1226-4) ne servent qu'au maintien de salaire, que cet arret n'ouvre pas. Rien n'est ecrit.",
        ));
    }
    // LA SUBROGATION SE TIRE DU DROIT : la convention qui la donne, ou le choix de la compagnie, et la duree maximale du
    // maintien pour ses dates (`engagement::maintien::subrogation`).
    let (sub, sub_debut, sub_fin) = crate::routes::imprevus::subrogation_de(ctx, &arret)?;
    arret["subrogation"] = sub;
    arret["subrogation_debut"] = sub_debut;
    arret["subrogation_fin"] = sub_fin;
    // On verifie AVANT d'ecrire, avec les regles de la nature 04 -- celles qui partiront.
    verifier(&arret)?;
    // Le jour où la compagnie a su l'accident, point de départ des 48 heures de la déclaration (R441-3) : contrôlé ici,
    // écrit avec l'arrêt.
    let connu = match optionnel(charge, "connu") {
        v if vrai(v) => Some(crate::obligations::connaissance_saisie(ctx, &arret, "accident", v)?),
        _ => None,
    };

    let mois = dernier.mois_iso();
    if crate::routes::imprevus::trouver(ctx, engagement::imprevus::ARRET, &valeur_en_texte(&cle), &dernier)?.is_some() {
        return Err(refus(format!(
            "Un arret de {} au {} est deja enregistre. Le modifier plutot que d'en saisir un second : deux arrets au meme dernier jour travaille, ce sont deux declarations qui se contredisent.",
            valeur_en_texte(&cle),
            dernier.jj_mm_aaaa()
        )));
    }
    crate::routes::imprevus::sans_chevauchement(ctx, &arret, None)?;
    let mut liste = engagement::imprevus::du_fichier(&ctx.racine, &mois).map_err(crate::routes::imprevus::refus_magasin)?;
    liste.push(arret.clone());
    ecrire_arrets(ctx, &mois, liste)?;
    if let Some((champ, valeur)) = connu {
        crate::obligations::connaissance_ecrire(ctx, &arret, champ, valeur)?;
    }
    reponse_arret(ctx, &arret, "Arrêt enregistré.")
}

/// `arret-reprise` : `{salarie, dernier_jour, reprise, motif_reprise?}` -> `{"arret": resume}` ; une reprise vide l'efface.
fn arret_reprise(ctx: &Contexte, charge: &Value) -> R<Value> {
    let nom = exiger(charge, "salarie")?;
    let dernier_jour = exiger(charge, "dernier_jour")?;
    let reprise = ou_rien(exiger(charge, "reprise")?);
    let motif = ou_rien(optionnel(charge, "motif_reprise"));
    let (i, mut liste) = trouver_l_arret(ctx, nom, dernier_jour)?;
    if let Value::Object(a) = &mut liste[i] {
        a.insert("reprise".into(), if vrai(&reprise) { json!(jour(&reprise)?.iso()) } else { reprise });
        a.insert("reprise_motif".into(), motif);
    }
    let arret = liste[i].clone();
    verifier(&arret)?;
    let mois: String = valeur_en_texte(&arret["dernier_jour_travaille"]).chars().take(7).collect();
    ecrire_arrets(ctx, &mois, liste)?;
    reponse_arret(ctx, &arret, if vrai(&arret["reprise"]) { "Reprise notée." } else { "Reprise effacée." })
}

/// `arret-oublier` : `{salarie, dernier_jour}` -> `{"arret": resume}`. Un arret deja declare ne se retire pas.
fn arret_oublier(ctx: &Contexte, charge: &Value) -> R<Value> {
    let nom = exiger(charge, "salarie")?;
    let dernier_jour = exiger(charge, "dernier_jour")?;
    let (i, mut liste) = trouver_l_arret(ctx, nom, dernier_jour)?;
    let envois = crate::obligations::avec_ce_qui_est_parti(ctx, &liste[i])?["envois"].as_array().map_or(0, Vec::len);
    if envois > 0 {
        return Err(refus(format!(
            "Cet arret a deja ete declare ({} envoi(s)). Le retirer du magasin n'annule rien aupres des organismes et ferait disparaitre la trace de ce qui est parti : emettre un annule-et-remplace.",
            envois
        )));
    }
    let arret = liste.remove(i);
    let mois: String = valeur_en_texte(&arret["dernier_jour_travaille"]).chars().take(7).collect();
    ecrire_arrets(ctx, &mois, liste)?;
    reponse_arret(ctx, &arret, "Arrêt retiré : aucune déclaration ne l'avait porté.")
}

/// `charge.get(cle)` quand il est vrai (`or None`).
fn facultatif<'a>(charge: &'a Value, cle: &str) -> Option<&'a Value> {
    charge.get(cle).filter(|v| vrai(v))
}

/// `charge["le"]` : un jour de constitution, quand la ligne de commande en donne un (`--le AAAA-MM-JJ`). Absent : le jour du
/// contexte, comme la fenetre.
fn jour_le(charge: &Value) -> R<Option<Jour>> {
    match facultatif(charge, "le") {
        None => Ok(None),
        Some(v) => Jour::lire(&en_texte(v)).map(Some).map_err(|m| erreur("ValueError", m)),
    }
}

/// Ce qui peut partir du mois : la saisie de ses engagements valides (`engagement::etats::validees_du_mois`), et leurs
/// paies ecrites ; refus si aucun engagement ne le touche.
fn validees(ctx: &Contexte, mois: &str) -> R<(Value, Vec<Paie>)> {
    let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs).map_err(plain)?;
    engagement::etats::validees_du_mois(ctx, mois, &departs).map_err(plain)?.ok_or_else(|| refus(format!("Aucune saisie pour {}.", mois)))
}

/// Le mois en toutes lettres (« mai 2026 ») pour une phrase ; tel quel s'il n'a pas la forme AAAA-MM.
fn mois_dit(mois: &str) -> String {
    match mois.get(..4) {
        Some(_) if mois.len() == 7 => engagement::libelle_mois(mois),
        _ => mois.to_string(),
    }
}

/// La cle de la fiche que `nom` designe, comme partout dans le moteur (`Contexte::salarie` : la casse, les accents, le
/// prenom, le NIR ne comptent pas). Un nom qui ne designe personne se refuse avec la phrase de `Contexte::salarie`.
fn cle_du_nom(ctx: &Contexte, nom: &str) -> R<String> {
    let fiche = crate::routes::salarie(ctx, &json!(nom))?;
    fiche.get("_cle").map(valeur_en_texte).ok_or_else(|| erreur("KeyError", socle::fichiers::absent("_cle")))
}

/// L'engagement brut avec ses bornes : un contrat aux cachets ne les ecrit pas, il commence son premier jour travaille et
/// finit son dernier (`engagement::debut_brut`, `engagement::dates_engagement`).
pub(crate) fn avec_ses_bornes(racine: &std::path::Path, e: &Value) -> Value {
    let mut e = e.clone();
    let jour = |j: &Value| j.get("jour").map_or_else(|| valeur_en_texte(j), valeur_en_texte);
    if !e.get("debut").is_some_and(vrai) {
        let debut = engagement::debut_brut(racine, &e);
        if !debut.is_empty() {
            e["debut"] = json!(debut);
        }
    }
    if !e.get("fin").is_some_and(vrai) {
        if let Some(fin) = engagement::dates_engagement(racine, &e).iter().map(jour).max() {
            e["fin"] = json!(fin);
        }
    }
    e
}

/// L'engagement de ce salarie (sa cle) qui se termine ce mois, avec ses bornes (`avec_ses_bornes`), et son rang dans la
/// saisie. Un engagement se rattache a sa fiche comme sa paie (`engagement::cle_de_l_engagement`), quelle que soit la
/// casse du nom saisi. `nom` : comme on l'a dit, pour la phrase. Un contrat qui a un motif de rupture passe avant les
/// autres. Un salarie sans engagement ce mois-la n'a pas de paie : la phrase le dit, avec le mois en toutes lettres.
fn engagement_qui_finit(ctx: &Contexte, saisie: &Value, cle: &str, nom: &str, mois: &str) -> R<(usize, Value)> {
    let eng = saisie.get("engagements").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]);
    let siens: Vec<(usize, Value)> = eng
        .iter()
        .enumerate()
        .filter(|(_, x)| engagement::cle_de_l_engagement(ctx, x).as_deref() == Some(cle))
        .map(|(i, x)| (i, avec_ses_bornes(&ctx.racine, x)))
        .collect();
    if siens.is_empty() {
        return Err(refus(format!("{} n'a aucun engagement en {} : aucun contrat ne s'y termine.", nom, mois_dit(mois))));
    }
    let finit = |x: &Value| x.get("fin").and_then(Value::as_str).is_some_and(|f| f.starts_with(mois));
    let rompu = siens.iter().position(|(_, x)| finit(x) && x.get("motif_rupture").is_some_and(vrai));
    let i = rompu.or_else(|| siens.iter().position(|(_, x)| finit(x))).ok_or_else(|| {
        refus(format!("{} n'a aucun contrat qui se termine en {}.", nom, mois_dit(mois)))
    })?;
    Ok(siens.into_iter().nth(i).expect("rang lu dans la meme liste"))
}

/// La paie validee de l'engagement de rang `rang` (`paies` : celles de `validees`, au meme rang que sa saisie).
fn paie_de_l_engagement(paies: &[Paie], rang: usize, cle: &str, nom: &str, mois: &str) -> R<Paie> {
    paies
        .get(rang)
        .filter(|p| p.salarie.get("_cle").map(valeur_en_texte).as_deref() == Some(cle))
        .cloned()
        .ok_or_else(|| refus(format!("{} n'a aucune paie validee en {} pour ce contrat.", nom, mois_dit(mois))))
}

/// `fin-contrat-documents` : le certificat de travail et le recu pour solde de tout compte (`documents::fin_de_contrat`)
/// de l'engagement du salarie qui finit ce mois.
fn fin_contrat_documents(ctx: &Contexte, charge: &Value) -> R<Value> {
    let (mois, salarie) = (texte_requis(charge, "mois")?, texte_requis(charge, "salarie")?);
    let (saisie, paies) = validees(ctx, mois)?;
    let cle = cle_du_nom(ctx, salarie)?;
    let (rang, _) = engagement_qui_finit(ctx, &saisie, &cle, salarie, mois)?;
    let p = paie_de_l_engagement(&paies, rang, &cle, salarie, mois)?;
    let date: Jour = jour_le(charge)?.unwrap_or(ctx.aujourdhui);
    let [certificat, recu] = documents::fin_de_contrat::ecrire(ctx, &p, date, &ctx.racine.join("sorties").join(mois))?;
    Ok(json!({
        "phrase": format!("Écrits pour {} : le certificat de travail et le reçu pour solde de tout compte.", crate::routes::nom_a_l_ecran(&p.salarie)),
        "certificat": certificat.to_string_lossy(),
        "recu": recu.to_string_lossy(),
        "dossier": ctx.racine.join("sorties").join(mois).to_string_lossy(),
        "avertissements": documents::fin_de_contrat::AVIS,
    }))
}

/// `fctu.FinDeContrat` (en dictionnaire) que le signalement declarera, batie sur la saisie
/// (`fin-contrat-signaler`).
fn fin_de_contrat_declaree(ctx: &Contexte, e: &Value, mois: &str) -> R<declarations::fctu::FinDeContrat> {
    let manque = crate::routes::imprevus::ce_qui_manque(ctx, e)?;
    let e = &crate::routes::imprevus::fin_au_terme(e)?;
    if !manque.is_empty() {
        let numero = e.get("numero").filter(|n| vrai(n)).map(en_texte).unwrap_or_else(|| "?".to_string());
        let liste: Vec<String> = manque.iter().map(crate::routes::imprevus::phrase_du_manque).collect();
        return Err(refus(format!(
            "Contrat {} : le signalement de fin de contrat ne s'écrit pas encore ; il manque, dans la carte de l'engagement, sous « Contrat » :\n  {}",
            numero,
            liste.join("\n  ")
        )));
    }
    let preavis = e.get("preavis").filter(|p| vrai(p)).cloned().unwrap_or_else(|| json!({}));
    let cle = |k: &str| e.get(k).cloned().unwrap_or(Value::Null);
    let fin = e.get("fin").ok_or_else(|| erreur("KeyError", socle::fichiers::absent("fin")))?.clone();
    let motif = e.get("motif_rupture").ok_or_else(|| erreur("KeyError", socle::fichiers::absent("motif_rupture")))?.clone();
    let type_preavis = preavis.get("type").ok_or_else(|| erreur("KeyError", socle::fichiers::absent("type")))?.clone();
    let v = json!({
        "fin": fin, "motif": motif, "mois_rattachement": mois,
        "type_preavis": type_preavis,
        "preavis_debut": preavis.get("debut").cloned().unwrap_or(Value::Null),
        "preavis_fin": preavis.get("fin").cloned().unwrap_or(Value::Null),
        "notification": cle("notification_rupture"),
        "signature_convention": cle("signature_convention"),
        "engagement_licenciement": cle("engagement_licenciement"),
        "transaction": cle("transaction"),
    });
    declarations::fctu::FinDeContrat::lire(&v).map_err(plain)
}

/// L'echec d'un signalement de fin de contrat ou d'amorcage : une regle de la norme qui s'oppose
/// (`FctuImpossible`, `AmorcageImpossible`, des `ValueError`) ou autre chose.
fn echec_fctu(e: declarations::fctu::Echec) -> Echec {
    match e {
        declarations::fctu::Echec::Impossible(m) | declarations::fctu::Echec::Autre(m) => erreur("ValueError", m),
    }
}

/// Ecrit le signalement dans `sorties/AAAA-MM/`. Rien n'est
/// depose : le depot est un acte a part.
fn fin_contrat_signaler(ctx: &Contexte, charge: &Value) -> R<Value> {
    let (mois, salarie) = (texte_requis(charge, "mois")?, texte_requis(charge, "salarie")?);
    let remplace: Option<String> = facultatif(charge, "remplace").map(en_texte);
    // LE NUMERO D'ORDRE : celui qu'on donne, sinon (0) le suivant de tous les signalements deja partis (p. 136), lu une
    // fois la saisie verifiee : la plate-forme ne se consulte pas pour un refus.
    let ordre: i64 = match facultatif(charge, "ordre") {
        None => 0,
        Some(Value::Number(n)) => n.as_i64().or_else(|| n.as_f64().map(|f| f as i64)).unwrap_or(1),
        Some(v) => {
            let t = en_texte(v);
            t.trim().parse().map_err(|_| erreur("ValueError", format!("nombre entier illisible : {}", repr_texte(&t))))?
        }
    };
    let envoi_test = facultatif(charge, "test").is_some();

    let (saisie, paies) = validees(ctx, mois)?;
    let cle = cle_du_nom(ctx, salarie)?;
    let (rang, e) = engagement_qui_finit(ctx, &saisie, &cle, salarie, mois)?;
    let fin = fin_de_contrat_declaree(ctx, &e, mois)?;
    let paie = paie_de_l_engagement(&paies, rang, &cle, salarie, mois)?;
    let type_declaration = if remplace.is_some() { declarations::fctu::ANNULE_ET_REMPLACE } else { declarations::fctu::NORMALE };
    let mut avertissements = declarations::fctu::verifier(&fin, &paie.engagement, type_declaration).map_err(echec_fctu)?;
    let (ordre, plate_forme) = if ordre != 0 { (ordre, Vec::new()) } else { prochain_ordre(ctx)? };
    avertissements.extend(plate_forme);
    let nom = format!("SignalementFinContrat_{}_{}.dsn", cle, fin.fin.replace('-', ""));
    let cible = ctx.racine.join("sorties").join(mois).join(&nom);
    let options = declarations::fctu::Options {
        type_declaration: type_declaration.to_string(),
        ordre,
        remplace: remplace.clone(),
        envoi_test,
        date_constitution: jour_le(charge)?,
    };
    // Les arrets du contrat qui croisent la periode du signalement : le FCTU les reporte (bloc 60), comme la mensuelle,
    // tires du magasin des imprevus ; ses suspensions (bloc 65) en viennent deja (`imprevus::suspensions_du_mois`).
    let (debut_p, fin_p) = (paie.engagement.debut_periode().map_err(plain)?, paie.engagement.fin_periode().map_err(plain)?);
    let arrets: Vec<declarations::dsn::Arret> = engagement::imprevus::arrets_du_contrat(ctx, &paie, &debut_p, &fin_p)
        .map_err(crate::routes::imprevus::refus_magasin)?
        .into_iter()
        .map(|v| declarations::dsn::Arret { v })
        .collect();
    let texte = declarations::fctu::generer(ctx, &paie, &fin, &options, &arrets, &envois::journal::lu(&ctx.racine).map(|j| j.departs).map_err(plain)?).map_err(echec_fctu)?;
    declarations::dsn::ecrire(&texte, &cible).map_err(plain)?;
    crate::compagnie::depart::noter_piece_ecrite(&ctx.racine, &crate::compagnie::depart::cle_de_piece(&cible), None, None)?;
    let preavis = declarations::fctu::libelle_preavis(&fin.type_preavis).map_err(plain)?;
    Ok(json!({
        "fichier": cible.to_string_lossy(),
        "phrase": phrase_du_signalement(&format!("Signalement de fin de contrat écrit (préavis : {}). Rien n'est parti.", preavis), &avertissements),
        "type": type_declaration,
        "ordre": ordre,
        "preavis": preavis,
        "avertissements": avertissements,
    }))
}

/// Ce qui se lit d'un signalement écrit : ce qui est fait, puis ses avis, sur la même ligne.
fn phrase_du_signalement<T: std::fmt::Display>(fait: &str, avertissements: &[T]) -> String {
    std::iter::once(fait.to_string()).chain(avertissements.iter().map(|a| a.to_string())).collect::<Vec<_>>().join("  ·  ")
}

/// L'arret de ce salarie dont c'est le dernier jour travaille, dans le magasin des imprevus.
fn trouver_arret(ctx: &Contexte, salarie: &str, dernier_jour: &Value) -> R<Value> {
    let cle = ctx.salarie(salarie).map_err(|m| erreur("KeyError", m))?;
    let cle = cle.get("_cle").map(valeur_en_texte).ok_or_else(|| erreur("KeyError", socle::fichiers::absent("_cle")))?;
    if vrai(dernier_jour) {
        let jour = crate::routes::imprevus::jour(dernier_jour)?;
        if let Some((i, liste)) = crate::routes::imprevus::trouver(ctx, engagement::imprevus::ARRET, &cle, &jour)? {
            return Ok(liste[i].clone());
        }
    }
    let lisible = if vrai(dernier_jour) {
        documents::bulletin::jj_mm_aaaa(dernier_jour).map_err(|m| erreur("ValueError", m))?
    } else {
        "?".to_string()
    };
    Err(refus(format!(
        "Aucun arret de {} dont le dernier jour travaille soit le {}. La route `arrets` les liste.",
        cle, lisible
    )))
}

/// Le numero d'ordre (`S20.G00.05.004`) du prochain signalement : un de plus que le plus grand de tous ceux que
/// la compagnie a deja envoyes, arrets, reprises, fins de contrat et amorcages confondus. Cahier technique
/// DSN, p. 136 : « Pour un signalement, ce numero ne doit pas etre remis a zero afin qu'il soit unique quel que
/// soit le mois d'emission » (et non plus le rang parmi les envois de CET arret, qui repartait a 1). LA PLATE-FORME
/// FAIT FOI : `crate::compagnie::releve::dernier_ordre_connu` la consulte ; ce qu'il faut en dire (elle n'a pas
/// repondu, elle connaissait plus que le journal) part avec les avertissements.
pub(crate) fn prochain_ordre(ctx: &Contexte) -> R<(i64, Vec<String>)> {
    let connu = crate::compagnie::releve::dernier_ordre_connu(ctx, None)?;
    Ok((connu.dernier + 1, connu.avertissements))
}

/// Ecrit le fichier du signalement dans `sorties/AAAA-MM/`. Rien
/// n'est depose : le depot est un acte a part.
fn arret_signaler(ctx: &Contexte, charge: &Value) -> R<Value> {
    use declarations::signalement::{self, Echec as Ech};
    let salarie = en_texte(crate::routes::fiches::exiger(charge, "salarie")?);
    let dernier_jour = crate::routes::fiches::exiger(charge, "dernier_jour")?;
    let nature = facultatif(charge, "nature").map(en_texte).unwrap_or_else(|| signalement::ARRET.to_string());
    let remplace: Option<String> = facultatif(charge, "remplace").map(en_texte);
    let envoi_test = facultatif(charge, "test").is_some();

    let arret = trouver_arret(ctx, &salarie, dernier_jour)?;
    let type_declaration = if remplace.is_some() { signalement::ANNULE_ET_REMPLACE } else { signalement::NORMALE };
    // `ordre` (`--ordre N`) : le rang voulu ; sinon le suivant. `le` (`--le`) : la date de constitution ; sinon le jour du contexte.
    let ordre = match facultatif(charge, "ordre") {
        Some(Value::Number(n)) => n.as_i64().or_else(|| n.as_f64().map(|f| f as i64)).unwrap_or(0),
        Some(v) => {
            let t = en_texte(v);
            t.trim().parse().map_err(|_| erreur("ValueError", format!("nombre entier illisible : {}", repr_texte(&t))))?
        }
        None => 0,
    };
    let constitution: Jour = jour_le(charge)?.unwrap_or(ctx.aujourdhui);
    let dsn_arret = declarations::dsn::Arret { v: arret.clone() };
    let echec = |e: Ech| match e {
        Ech::Impossible(m) | Ech::Autre(m) => erreur("ValueError", m),
    };
    let mut avertissements = signalement::verifier(&dsn_arret, &nature, Some(&constitution)).map_err(echec)?;
    let (ordre, plate_forme) = if ordre != 0 { (ordre, Vec::new()) } else { prochain_ordre(ctx)? };
    avertissements.extend(plate_forme);

    let jour = en_texte(&arret["dernier_jour_travaille"]);
    let mois: String = jour.chars().take(7).collect();
    let nom = format!(
        "Signalement{}_{}_{}.dsn",
        if nature == signalement::ARRET { "Arret" } else { "Reprise" },
        en_texte(&arret["salarie"]),
        jour.replace('-', "")
    );
    let cible = ctx.racine.join("sorties").join(&mois).join(&nom);
    let options = signalement::Options {
        nature: nature.clone(),
        type_declaration: type_declaration.to_string(),
        ordre,
        remplace,
        envoi_test,
        date_constitution: Some(constitution),
    };
    let texte = signalement::generer(ctx, &dsn_arret, &options, &envois::journal::lu(&ctx.racine).map(|j| j.departs).map_err(plain)?).map_err(echec)?;
    declarations::dsn::ecrire(&texte, &cible).map_err(plain)?;
    crate::compagnie::depart::noter_piece_ecrite(&ctx.racine, &crate::compagnie::depart::cle_de_piece(&cible), None, None)?;
    Ok(json!({
        "fichier": cible.to_string_lossy(),
        "phrase": phrase_du_signalement(if nature == signalement::ARRET { "Signalement d'arrêt écrit. Rien n'est parti." } else { "Signalement de reprise écrit. Rien n'est parti." }, &avertissements),
        "nature": nature,
        "type": type_declaration,
        "ordre": ordre,
        "avertissements": avertissements,
    }))
}

#[track_caller]
fn plain(m: String) -> Echec {
    socle::echec::noter_et(None, m)
}

/// `charge["cle"]` : un texte requis.
fn texte_requis<'a>(charge: &'a Value, cle: &str) -> R<&'a str> {
    crate::routes::fiches::exiger(charge, cle)?.as_str().ok_or_else(|| erreur("TypeError", socle::echec::charge(cle, "un texte")))
}

/// Le nombre d'un morceau de nom de mois ; refus s'il ne se lit pas.
fn entier(t: &str) -> R<i64> {
    t.trim().parse::<i64>().map_err(|_| erreur("ValueError", format!("nombre entier illisible : {}", repr(t))))
}

/// Le champ `k` d'une valeur, `defaut` s'il manque.
fn champ(v: &Value, k: &str, defaut: Value) -> Value {
    v.get(k).cloned().unwrap_or(defaut)
}

// -------------------------------------------------------------------- arrets

/// Un refus du magasin des imprevus (`imprevus`) : un fichier abime, un genre inconnu, le dossier `arrets/` d'avant.
#[track_caller]
pub(crate) fn refus_magasin(m: String) -> Echec {
    erreur("ErreurOperation", m)
}

/// Les arrets de travail du magasin des imprevus (`imprevus/AAAA-MM.json`, genre `arret`), dans l'ordre des mois du
/// magasin, chacun complet.
pub(crate) fn tous_les_arrets(ctx: &Contexte) -> R<Vec<Value>> {
    Ok(engagement::imprevus::tous(&ctx.racine).map_err(refus_magasin)?.into_iter().filter(engagement::imprevus::est_un_arret).collect())
}

/// `signalement.nomenclatures()`.
pub(crate) fn nomenclatures(ctx: &Contexte) -> R<std::sync::Arc<Value>> {
    let _ = ctx;
    json_en_cache(&dossier_du_code().join("arrets-base.json")).map_err(plain)
}

/// Les codes de `nomenclatures()[quoi]["codes"]`.
fn codes(n: &Value, quoi: &str) -> R<Vec<Value>> {
    let section = n.get(quoi).ok_or_else(|| erreur("KeyError", socle::fichiers::absent(quoi)))?;
    let liste = section.get("codes").ok_or_else(|| erreur("KeyError", socle::fichiers::absent("codes")))?;
    Ok(items(liste)?.into_iter().map(|c| c.into_owned()).collect())
}

/// `signalement.motifs_arret()` et `motifs_reprise()` : les codes saisissables.
pub(crate) fn saisissables(n: &Value, quoi: &str) -> R<Vec<Value>> {
    let mut out = Vec::new();
    for c in codes(n, quoi)? {
        if vrai(get(&c, "saisissable")?) {
            out.push(c);
        }
    }
    Ok(out)
}

/// Le libelle du motif d'arret dans les codes de la norme, `?` s'il n'y est pas.
fn libelle_motif(n: &Value, motif: &Value) -> R<Value> {
    if matches!(motif, Value::Array(_) | Value::Object(_)) {
        return Err(erreur("TypeError", socle::fichiers::pas_une_cle(nom_type(motif))));
    }
    let Some(m) = motif.as_str() else { return Ok(json!("?")) };
    let mut trouve: Option<Value> = None;
    for c in codes(n, "motifs_arret")? {
        if get(&c, "code")?.as_str() == Some(m) {
            trouve = Some(c);
        }
    }
    match trouve {
        Some(c) => Ok(getd(&c, "libelle", &json!("?"))?.clone()),
        None => Ok(json!("?")),
    }
}

/// Les envois d'un arret (`arret.envois`), a parcourir.
fn envois(a: &Value) -> R<Vec<std::borrow::Cow<'_, Value>>> {
    match a.get("envois") {
        None => Ok(Vec::new()),
        Some(v) => items(v),
    }
}

/// Un arret resume pour la liste des imprevus.
pub(crate) fn resume_arret(ctx: &Contexte, n: &Value, a: &Value) -> R<Value> {
    let a = &crate::obligations::avec_ce_qui_est_parti(ctx, a)?;
    let mut signalements = Vec::new();
    for e in envois(a)? {
        if matches!(get(&e, "nature")?.as_str(), Some("04") | Some("05")) {
            signalements.push(e.into_owned());
        }
    }
    let mut reports = Vec::new();
    for e in envois(a)? {
        if get(&e, "nature")?.as_str() == Some("01") {
            reports.push(e.into_owned());
        }
    }
    let motif = champ(a, "motif", Value::Null);
    let libelle = libelle_motif(n, &motif)?;
    let mut mois = Vec::new();
    for r in &reports {
        mois.push(get(r, "mois")?.clone());
    }
    let signale = !signalements.is_empty();
    // UN SIGNALEMENT EST-IL DU ? Pas pour un motif que la DSN ne couvre pas (le deuil d'enfant : `obligations-base.json`,
    // `signalement_arret.sauf_motifs`) ; son obligation est alors l'attestation de salaire hors DSN.
    let signalement_du = crate::obligations::signalement_du(&socle::date::en_texte(&motif))?;
    Ok(json!({
        "salarie": champ(a, "salarie", Value::Null),
        "contrat": champ(a, "contrat_numero", Value::Null),
        "contrat_debut": champ(a, "contrat_debut", Value::Null),
        "motif": motif,
        "motif_libelle": libelle,
        "dernier_jour_travaille": champ(a, "dernier_jour_travaille", Value::Null),
        "debut": champ(a, "debut", Value::Null),
        "fin_previsionnelle": champ(a, "fin_previsionnelle", Value::Null),
        "subrogation": champ(a, "subrogation", Value::Null),
        "subrogation_debut": champ(a, "subrogation_debut", Value::Null),
        "subrogation_fin": champ(a, "subrogation_fin", Value::Null),
        "date_accident": champ(a, "date_accident", Value::Null),
        "reprise": champ(a, "reprise", Value::Null),
        "reprise_motif": champ(a, "reprise_motif", Value::Null),
        "note": champ(a, "note", json!("")),
        "saisi_le": champ(a, "saisi_le", Value::Null),
        "signale": signale,
        "signalement_du": signalement_du,
        "signalements": signalements,
        "reporte_par_dit": mois_de_la_dsn(&mois),
        "reporte_par": mois,
        "etat": if signale { "signale" } else if signalement_du { "a signaler" } else { "sans signalement" },
    }))
}

/// La route `arrets` : les arrets de travail du magasin des imprevus, chacun resume comme `imprevus` le resume
/// (`crate::routes::imprevus::resume`), ceux qu'aucun signalement n'a annonces, et les nomenclatures du bloc 60.
fn arrets(ctx: &Contexte) -> R<Value> {
    let tous = tous_les_arrets(ctx)?;
    let n = nomenclatures(ctx)?;
    // LES OBLIGATIONS de chaque arret (`obligations::de_l_arret`) : un champ de plus a son resume.
    let suivi = crate::obligations::Suivi::lire(ctx)?;
    let mut resumes = Vec::new();
    let mut a_signaler = Vec::new();
    for a in &tous {
        let mut r = crate::routes::imprevus::resume(ctx, &n, a)?;
        r["obligations"] = Value::Array(crate::obligations::de_l_arret(ctx, a, &suivi, ctx.aujourdhui)?);
        let r = dire_ce_qui_est_parti(r);
        if !r["signale"].as_bool().unwrap_or(false) && r["signalement_du"].as_bool().unwrap_or(true) {
            a_signaler.push(r.clone());
        }
        resumes.push(r);
    }
    let subrogation = n.get("subrogation").ok_or_else(|| erreur("KeyError", socle::fichiers::absent("subrogation")))?;
    // Ce que l'ecran lit (Exports › Avancé) : qui, quand, le motif, ce qui reste a signaler.
    let phrases: Vec<String> = resumes
        .iter()
        .flat_map(|r| {
            let t = |k: &str| r.get(k).filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default();
            let fin = if t("fin_previsionnelle").is_empty() { String::new() } else { format!(", fin prévue le {}", crate::routes::quand(&t("fin_previsionnelle"))) };
            let reste = match t("etat").as_str() {
                "a signaler" => "à signaler",
                "signale" => "signalé",
                _ => "sans signalement",
            };
            // Puis ses obligations, une ligne chacune.
            let mut l = vec![format!("{} : dernier jour travaillé le {}{} — {} — {}", t("salarie"), crate::routes::quand(&t("dernier_jour_travaille")), fin, t("motif_libelle"), reste)];
            l.extend(crate::obligations::en_phrases(&r["obligations"]).into_iter().map(|o| format!("  {}", o)));
            l
        })
        .collect();
    Ok(json!({
        "phrase": format!("{}, {} à signaler.", crate::routes::compte(resumes.len(), "arrêt de travail", "arrêts de travail"), a_signaler.len()),
        "phrases": phrases,
        "arrets": resumes,
        "a_signaler": a_signaler,
        "motifs": saisissables(&n, "motifs_arret")?,
        "motifs_reprise": saisissables(&n, "motifs_reprise")?,
        "subrogation": subrogation.get("codes").ok_or_else(|| erreur("KeyError", socle::fichiers::absent("codes")))?,
    }))
}

// ----------------------------------------------------------- fins de contrat

/// Le dernier jour du mois.
fn fin_du_mois(mois: &str) -> R<Jour> {
    let an = entier(&mois.chars().take(4).collect::<String>())?;
    let mo = entier(&mois.chars().skip(5).take(2).collect::<String>())?;
    if !(1..=12).contains(&mo) {
        return Err(erreur("ValueError", socle::date::mois_impossible(mo)));
    }
    if !(1..=9999).contains(&an) {
        return Err(erreur("ValueError", socle::date::annee_impossible(an)));
    }
    let dernier = socle::date::jours_du_mois(an as i32, mo as u32);
    Jour::nouveau(an as i32, mo as u32, dernier).ok_or_else(|| erreur("ValueError", socle::date::jour_impossible(an as i64, mo as i64, dernier)))
}

/// `KeyError` quand elle est inconnue.
fn nature_de(cle: &Value) -> R<std::sync::Arc<droit::natures::Nature>> {
    let texte = match cle {
        Value::String(s) => Some(s.clone()),
        v if !vrai(v) => None,
        Value::Array(_) | Value::Object(_) => {
            return Err(erreur("TypeError", socle::fichiers::pas_une_cle(nom_type(cle))));
        }
        autre => {
            // Une cle qui n'est pas un texte ne peut etre aucune nature.
            let mut connues: Vec<String> = droit::natures::natures().map_err(plain)?.iter().map(|n| n.cle.clone()).collect();
            connues.sort();
            return Err(erreur("KeyError", format!("Nature de contrat inconnue : {}. Connues : {}.", valeur_ecrite(autre), connues.join(", "))));
        }
    };
    droit::natures::nature(texte.as_deref()).map_err(|m| erreur("KeyError", m))
}

/// Ce qui manque a un signalement, tel que l'ecran le dit : `champ` (la donnee de l'engagement, `data-fait` de sa carte),
/// `case` (le nom de la case, sous « Contrat »), `dire` (ce qu'on y met, en mots d'utilisateur, visible sans rien
/// deplier) et `pourquoi` (la regle de la norme).
fn manque(champ: &str, case: &str, dire: impl Into<String>, pourquoi: impl Into<String>) -> Value {
    json!({"champ": champ, "case": case, "dire": dire.into(), "pourquoi": pourquoi.into()})
}

/// Le type de preavis « 90 - pas de clause de preavis applicable » (`fctu-base.json`, S21.G00.63.001).
const SANS_PREAVIS: &str = "90";

/// UN CONTRAT A DUREE DETERMINEE QUI VA A SON TERME n'a rien a dire de sa fin : son motif est celui de sa nature
/// (`contrats-base.json`, `motif_par_defaut` : 031, « la fin normale d'un CDD arrive a son terme »), et il n'a pas de
/// preavis -- il « cesse de plein droit a l'echeance du terme » (L1243-5, `permanents-base.json`, `motifs_sans_preavis`,
/// 031) --, ce que le bloc S21.G00.63 declare en type 90. L'engagement tel que le signalement le lit : sans motif de
/// rupture saisi, ces deux faits ; avec un motif saisi, tel quel.
pub(crate) fn fin_au_terme(e: &Value) -> R<Value> {
    let mut e = e.clone();
    if vrai(get(&e, "motif_rupture")?) {
        return Ok(e);
    }
    if let Some(m) = nature_de(get(&e, "nature")?)?.motif_rupture_par_defaut() {
        e["motif_rupture"] = json!(m);
        if !vrai(get(ou(get(&e, "preavis")?, objet_vide()), "type")?) {
            e["preavis"] = json!({"type": SANS_PREAVIS});
        }
    }
    Ok(e)
}

/// La phrase d'un manque (`ce_qui_manque`) : sa case, son nom dans la saisie de la carte, et ce qu'on y met.
pub(crate) fn phrase_du_manque(m: &Value) -> String {
    format!("« {} » (`{}`) : {}", valeur_en_texte(&m["case"]), valeur_en_texte(&m["champ"]), valeur_en_texte(&m["dire"]))
}

/// Ce qu'un signalement exige et que la saisie ne dit pas encore, un manque par case
/// de la carte (`manque`).
pub(crate) fn ce_qui_manque(ctx: &Contexte, e: &Value) -> R<Vec<Value>> {
    let e = &fin_au_terme(e)?;
    let mut manque = Vec::new();
    let motif = get(e, "motif_rupture")?;
    if !vrai(motif) {
        manque.push(self::manque(
            "motif_rupture",
            "Motif de la rupture",
            "pourquoi le contrat a pris fin.",
            "Le motif de la rupture (S21.G00.62.002) est obligatoire, et un CDI n'en a aucun par défaut.",
        ));
    }
    // CCH-12 de S21.G00.62.003 : la date de notification, obligatoire pour ces motifs (`fctu::notification_exigee`).
    if vrai(motif) && !vrai(get(e, "notification_rupture")?) && declarations::fctu::notification_exigee(&valeur_en_texte(motif)).map_err(plain)? {
        manque.push(self::manque(
            "notification_rupture",
            "Notifiée le",
            "le jour où la rupture a été notifiée : la démission remise, la lettre de rupture ou de licenciement envoyée, la \
fin de la période d'essai notifiée.",
            format!(
                "La date de notification de la rupture (S21.G00.62.003) est obligatoire pour le motif {} (CCH-12, cahier \
technique DSN p. 275). Le signalement ne s'écrit pas sans elle.",
                valeur_en_texte(motif)
            ),
        ));
    }
    let preavis = ou(get(e, "preavis")?, objet_vide());
    if !vrai(get(preavis, "type")?) {
        let requis = |k: &str| e.get(k).ok_or_else(|| erreur("KeyError", socle::fichiers::absent(k)));
        let (debut, fin) = (d(requis("debut")?).map_err(|m| erreur("ValueError", m))?, d(requis("fin")?).map_err(|m| erreur("ValueError", m))?);
        let anciennete = droit::permanents::anciennete_mois(&debut, &fin);
        let motif_texte = if vrai(motif) { Some(valeur_en_texte(motif)) } else { None };
        let cdd = !nature_de(get(e, "nature")?)?.duree_indeterminee;
        let duree = droit::permanents::preavis_mois(anciennete, motif_texte.as_deref(), cdd).map_err(plain)?;
        let regle = droit::permanents::regle_du_preavis(motif_texte.as_deref(), cdd).map_err(plain)?;
        let mut dit = match (duree, &regle) {
            (Some(m), _) if m != Decimal::ZERO => format!("La loi en donne {} mois (L1234-1).", m),
            (Some(_), _) => format!(
                "La loi ne donne aucun préavis pour ce motif : {}",
                droit::permanents::raison_sans_preavis(motif_texte.as_deref(), cdd).map_err(plain)?.unwrap_or_default()
            ),
            (None, Some(r)) => format!("La loi en donne un que le moteur ne compte pas : {}", r),
            (None, None) => "Sous six mois d'ancienneté, la loi renvoie à la convention ou aux usages (L1234-1) : elle ne dit pas de durée.".to_string(),
        };
        // Ce que dit la convention, a cote, par le moteur (le calcul de `moteur preavis`) ; pas pour un motif sans preavis
        // ni pour la rupture d'un CDD par le salarie (L1243-2), que la convention du licenciement ne regle pas.
        if duree != Some(Decimal::ZERO) && regle.is_none() {
            let tout = engagement::preavis(ctx, e, anciennete, e.get("motif_rupture").and_then(Value::as_str)).map_err(plain)?;
            if let (Some(conv), true) = (tout.convention, tout.qui != "la loi (L1234-1)") {
                dit.push_str(&format!(" {} en donne {} : c'est elle qui s'applique.", tout.qui, conv));
            } else if tout.mois.is_none() {
                if let Some(pourquoi) = tout.pourquoi.as_ref().filter(|p| !p.is_empty()) {
                    dit.push_str(&format!(" Et {}.", pourquoi.trim_end_matches('.')));
                }
            }
        }
        manque.push(self::manque(
            "preavis",
            "Préavis",
            format!("le préavis de la rupture, ses dates s'il y en a un, ou « pas de clause de préavis ». {}", dit),
            "Le bloc S21.G00.63 est obligatoire dans un signalement de fin de contrat (cardinalité 1,*) : même sans préavis, il \
se déclare, avec le type « 90 - pas de clause de préavis applicable ».",
        ));
    } else if declarations::fctu::preavis_non_effectue(&valeur_en_texte(get(preavis, "type")?)).map_err(plain)? && !vrai(get(e, "dernier_jour_paye")?) {
        manque.push(self::manque(
            "dernier_jour_paye",
            "Dernier jour payé",
            "le dernier jour travaillé et payé au salaire habituel, avant le préavis qui n'est pas effectué.",
            "Sur un préavis non effectué, le dernier jour travaillé et payé au salaire habituel (S21.G00.62.006) n'est pas la fin \
du contrat, il précède le début du préavis (CCH-12, cahier technique DSN p. 275). La mensuelle et le signalement le \
déclarent tous deux.",
        ));
    } else if let Some(dernier) = e.get("dernier_jour_paye").filter(|j| vrai(j)).map(valeur_en_texte) {
        // Hors preavis non effectue, aucun jour de la paie ne vient apres le dernier jour paye.
        let jour = |j: &Value| j.get("jour").map_or_else(|| valeur_en_texte(j), valeur_en_texte);
        let apres: Vec<String> = engagement::dates_engagement(&ctx.racine, e).iter().map(jour).filter(|j| *j > dernier).collect();
        if !apres.is_empty() {
            let dire = |j: &str| socle::date::jour_iso(j).map_or_else(|| j.to_string(), |x| x.jj_mm());
            manque.push(self::manque(
                "dernier_jour_paye",
                "Dernier jour payé",
                format!(
                    "{} {} après le dernier jour payé, le {} : retirez-le de la paie, ou dites un autre dernier jour payé.",
                    apres.iter().map(|j| format!("le {}", dire(j))).collect::<Vec<_>>().join(", "),
                    if apres.len() > 1 { "sont" } else { "est" },
                    dire(&dernier)
                ),
                "Le dernier jour travaillé et payé (S21.G00.62.006) clôt la paie du contrat rompu : un jour payé après lui \
contredit le signalement.",
            ));
        }
    }
    Ok(manque)
}

/// Les contrats qui finissent dans `mois` et dont la fin SE SIGNALE, chacun avec
/// son obligation (`obligations::de_la_fin`) comptee au jour `aujourdhui`. UN SEUL ENDROIT : la route `fins-de-contrat` (la
/// colonne du mois) et les rappels de l'ouverture (`rappels`) la lisent.
pub(crate) fn fins_a_signaler(ctx: &Contexte, mois: &str, aujourdhui: Jour) -> R<Vec<Value>> {
    // Un signalement de fin ne se doit que pour un engagement valide, lu tel qu'il est ecrit (`tels_qu_ecrits`) : un
    // brouillon ne part pas, et ce qui ne fait que lire ne recalcule rien.
    let Some(saisie) = engagement::stock::vue_du_mois(&ctx.racine, mois).map_err(Echec::from)? else {
        return Ok(Vec::new());
    };
    let journal = envois::journal::lu(&ctx.racine)?;
    let etats = engagement::etats::tels_qu_ecrits(ctx, mois, &journal.departs).map_err(Echec::from)?;
    let fin_mois = fin_du_mois(mois)?;
    let engagements = getd(&saisie, "engagements", &Value::Null)?;
    let liste = if vrai(engagements) { items(engagements)? } else { Vec::new() };
    let mut out = Vec::new();
    for (i, e) in liste.into_iter().enumerate() {
        if etats.get(i).and_then(|x| x.valide()).is_none() {
            continue;
        }
        let e: &Value = &avec_ses_bornes(&ctx.racine, &e);
        let nat = nature_de(get(e, "nature")?)?;
        // La regle est celle de la nature et du motif (`Nature::signalement_fctu`) : un CDDU rompu avant son terme se signale.
        let motif = e.get("motif_rupture").filter(|m| vrai(m)).map(valeur_en_texte);
        if !nat.signalement_fctu(motif.as_deref()) || !vrai(get(e, "fin")?) {
            continue;
        }
        let debut_du_mois = d(&json!(format!("{}-01", mois))).map_err(|m| erreur("ValueError", m))?;
        let fin = d(&e["fin"]).map_err(|m| erreur("ValueError", m))?;
        if !(debut_du_mois <= fin && fin <= fin_mois) {
            continue;
        }
        let salarie = e.get("salarie").ok_or_else(|| erreur("KeyError", socle::fichiers::absent("salarie")))?;
        // La cle de la fiche : celle que porte le nom du fichier du signalement (`fin-contrat-signaler`).
        let cle = ctx.cle_du_salarie(&valeur_en_texte(salarie)).ok();
        let a_saisir = ce_qui_manque(ctx, e)?;
        // LE DELAI ET OU ON EN EST (`obligations::de_la_fin`).
        let obligation = crate::obligations::de_la_fin(ctx, cle.as_deref(), fin, &journal.lignes, aujourdhui)?;
        // Due : il manque de quoi signaler, aucun delai ne se lit, ou le delai court. Sa pastille : celle du delai passe
        // (« en retard » l'emporte : la date butoir ne se tait pas parce qu'une case manque), sinon ce qui manque, sinon
        // celle du delai.
        let incomplet = !a_saisir.is_empty();
        let du = incomplet || obligation.is_null() || obligation.get("du").is_some_and(vrai);
        let en_retard = obligation.get("etat").and_then(Value::as_str) == Some(crate::obligations::EN_RETARD);
        let pastille = if incomplet && !en_retard {
            super::pastille("a_faire", "à compléter")
        } else {
            obligation.get("pastille").cloned().unwrap_or(Value::Null)
        };
        out.push(json!({
            "salarie": salarie,
            "mois": mois,
            "debut": e["debut"],
            "en_retard": en_retard,
            "numero": get(e, "numero")?,
            "fin": e["fin"],
            "nature": nat.libelle,
            "motif_rupture": get(e, "motif_rupture")?,
            "preavis": get(e, "preavis")?,
            "a_saisir": a_saisir,
            "du": du,
            "pastille": pastille,
            "obligation": obligation,
        }));
    }
    Ok(out)
}

/// La route `fins-de-contrat`.
fn fins_de_contrat(ctx: &Contexte, charge: &Value) -> R<Value> {
    let brut = charge.get("mois").ok_or_else(|| erreur("KeyError", socle::fichiers::absent("mois")))?;
    let mois = valeur_en_texte(brut);
    let mut fins = fins_en_retard_avant(ctx, &mois, ctx.aujourdhui)?;
    fins.extend(fins_a_signaler(ctx, &mois, ctx.aujourdhui)?);
    // Ce que l'ecran lit de chaque fin : qui, quelle nature, quel jour (et le mois d'une fin d'un mois passe), et son
    // contrat.
    for f in &mut fins {
        let fin = crate::routes::jour_en_clair(&f["fin"]);
        let nom = ctx.salarie(&valeur_en_texte(&f["salarie"])).map(|s| crate::routes::nom_a_l_ecran(&s)).unwrap_or_else(|_| valeur_en_texte(&f["salarie"]));
        let ailleurs = match f["mois"].as_str().filter(|m| !m.is_empty() && *m != mois) {
            Some(m) => format!(" ({})", engagement::libelle_mois(m)),
            None => String::new(),
        };
        f["qui_dit"] = json!(format!("{}, {}, fin le {}{}", nom, valeur_en_texte(&f["nature"]), fin, ailleurs));
        let numero = if vrai(&f["numero"]) { valeur_en_texte(&f["numero"]) } else { "?".to_string() };
        f["contrat_dit"] = json!(format!("contrat {}, fin le {}", numero, fin));
    }
    Ok(json!({"mois": brut, "fins": fins}))
}

/// Combien de mois en arriere un signalement de fin en retard se rappelle : une annee de paies.
pub(crate) const MOIS_EN_ARRIERE: u32 = 12;

/// LES FINS D'AVANT, EN RETARD : celles des mois qui precedent `mois` (sur `MOIS_EN_ARRIERE`) dont le signalement est du
/// et sa date butoir passee. Une echeance manquee reste due -- le signalement se fait « dans les cinq jours ouvres qui
/// suivent la survenance de la fin du contrat » (cahier technique DSN 2026.1, p. 107, `fctu-base.json`, `_delai`) -- et
/// elle se dit sur le mois qu'on regarde, pas seulement sur celui ou le contrat a fini.
pub(crate) fn fins_en_retard_avant(ctx: &Contexte, mois: &str, aujourdhui: Jour) -> R<Vec<Value>> {
    let mut out = Vec::new();
    let mut m = mois.to_string();
    for _ in 0..MOIS_EN_ARRIERE {
        m = socle::date::premier_du_mois(&m).and_then(|j| j.plus_jours(-1)).map_err(|e| erreur("ValueError", e))?.mois_iso();
        let mut celles: Vec<Value> = fins_a_signaler(ctx, &m, aujourdhui)?
            .into_iter()
            .filter(|f| f["du"] == json!(true) && f["en_retard"] == json!(true))
            .collect();
        celles.extend(out);
        out = celles;
    }
    Ok(out)
}

// ------------------------------------------------------------ regularisations





/// `{"contrats": [...]}` : les
/// contrats d'un salarie qui couvrent un jour, lus dans les saisies.
fn arret_contrats(ctx: &Contexte, charge: &Value) -> R<Value> {
    let sal = exiger(charge, "salarie")?;
    let jour = exiger(charge, "jour")?;
    let cle = match salarie(ctx, sal)?.get("_cle") {
        Some(c) => c.clone(),
        None => return Err(erreur("KeyError", socle::fichiers::absent("_cle"))),
    };
    let jour = d(jour).map_err(|m| erreur("ValueError", m))?.iso();
    Ok(json!({"contrats": contrats_au(ctx, &cle, &jour)?}))
}

/// Les contrats de la fiche `cle` qui couvrent `jour` (AAAA-MM-JJ),
/// lus dans les saisies, du plus ancien debut au plus recent. Chacun : salarie, nom, numero, debut,
/// fin, mois, spectacle.
pub(crate) fn contrats_au(ctx: &Contexte, cle: &Value, jour: &str) -> R<Vec<Value>> {
    let cle_texte = valeur_en_texte(cle);
    let mut sorties: Vec<(String, Value)> = Vec::new();
    let mut vus: Dico<()> = Dico::nouveau();
    let mut mois = engagement::stock::mois_saisis(&ctx.racine).map_err(socle::echec::Echec::from)?;
    mois.reverse();
    for m in mois {
        for (c, e) in crate::routes::mois::engagements_du_mois(ctx, &m)? {
            if c != cle_texte {
                continue;
            }
            let (debut, fin) = crate::routes::mois::bornes_brutes(ctx, &e);
            // Un contrat paye au mois sans fin ecrite (un CDI qui court) couvre tout jour depuis son debut : c'est sur lui
            // qu'un arret ou une suspension se pose. Le mois le plus recent qui le saisit fait foi (une fin ecrite plus
            // tard l'emporte) : il est retenu, couvrant ou non, avant les mois plus anciens.
            let mensuel = droit::natures::nature(e.get("nature").and_then(Value::as_str)).map(|n| n.mensuelle()).unwrap_or(false);
            let numero = e.get("numero").cloned().unwrap_or(Value::Null);
            if debut.is_empty() || (fin.is_empty() && !mensuel) {
                continue;
            }
            if vus.contient(&[json!(c), json!(debut), numero.clone()])? {
                continue;
            }
            vus.poser(vec![json!(c), json!(debut), numero.clone()], ())?;
            if !(debut.as_str() <= jour && (fin.is_empty() || jour <= fin.as_str())) {
                continue;
            }
            salarie(ctx, cle)?;
            let nom = ctx.nomme(&valeur_en_texte(cle));
            let ou_vide = |k: &str| e.get(k).filter(|x| vrai(x)).cloned().unwrap_or_else(|| json!(""));
            let mut o = Map::new();
            o.insert("salarie".into(), cle.clone());
            o.insert("nom".into(), json!(nom));
            o.insert("numero".into(), brut(&ou_vide("numero")));
            o.insert("debut".into(), json!(debut));
            o.insert("fin".into(), json!(fin));
            o.insert("mois".into(), json!(m));
            o.insert("spectacle".into(), brut(&ou_vide("spectacle")));
            // Paye au mois (`Nature::mensuelle`) : son mois de paie se tire du moteur, et un arret y appelle le maintien de
            // salaire (L1226-1 : pas pour un intermittent).
            o.insert("mensuel".into(), json!(mensuel));
            o.insert("nature".into(), e.get("nature").cloned().unwrap_or(Value::Null));
            sorties.push((debut, Value::Object(o)));
        }
    }
    sorties.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(sorties.into_iter().map(|(_, v)| v).collect())
}

/// Les mois des DSN qui ont porte un imprevu, tels que l'ecran les lit (« de novembre 2026 »).
fn mois_de_la_dsn(mois: &[Value]) -> Vec<String> {
    mois.iter().map(|m| crate::regularisation::de_mois_en_clair(&valeur_en_texte(m))).collect()
}

/// CE QUE L'ECRAN LIT D'UN IMPREVU, en phrases que la page ne compose pas : ce qu'il est (`quoi`), sa periode
/// (`periode` ; un arret sans reprise dit sa fin prescrite), son contrat (`contrat_dit`), sa reprise (`reprise_dit`), le
/// debut d'une absence (`depuis_dit`), ce qui est paye pendant (`paye_pendant`, et chaque jour d'un cachet : `jours`), et
/// les signalements deja partis (`deja_signale`, par nature).
fn dire_l_imprevu(mut r: Value) -> Value {
    let jour = |v: &Value| crate::routes::jour_en_clair(v);
    let arret = r["genre"] == json!(ARRET);
    let libelle = valeur_en_texte(&r["motif_libelle"]);
    r["quoi"] = json!(if arret { format!("Arrêt de travail ({})", libelle) } else { libelle });
    r["periode"] = json!(if arret && !vrai(&r["reprise"]) {
        format!("depuis le {}, prescrit jusqu'au {}", jour(&r["premier_jour"]), jour(&r["dernier_jour"]))
    } else {
        format!("du {} au {}", jour(&r["premier_jour"]), jour(&r["dernier_jour"]))
    });
    r["contrat_dit"] = if vrai(&r["contrat"]) { json!(format!("contrat {}", valeur_en_texte(&r["contrat"]))) } else { Value::Null };
    r["reprise_dit"] = if vrai(&r["reprise"]) { json!(format!("reprise le {}", jour(&r["reprise"]))) } else { Value::Null };
    r["depuis_dit"] = if vrai(&r["debut"]) { json!(format!("Depuis le {}", jour(&r["debut"]))) } else { Value::Null };
    r["paye_pendant"] = json!(if arret { "Payé pendant l'arrêt" } else { "Payé pendant l'absence" });
    r["reprise_titre"] = json!(if vrai(&r["reprise"]) { "La reprise" } else { "Reprise plus tôt" });
    r["signaler_titre"] = json!({"04": "Signaler l'arrêt", "05": "Signaler la reprise"});
    if let Some(cachets) = r["cachets"].as_array_mut() {
        for c in cachets {
            let libelle = valeur_en_texte(&c["libelle"]);
            let mois_dit = valeur_en_texte(&c["mois_dit"]);
            let jours: Vec<Value> = c["dates"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|iso| {
                    let le = format!("{} le {}", libelle, jour(iso));
                    json!({"iso": iso, "phrase": le, "phrase_du_mois": format!("{}, {}", le, mois_dit)})
                })
                .collect();
            c["jours"] = Value::Array(jours);
        }
    }
    let mut deja = Map::new();
    for nature in ["04", "05"] {
        let n = r["signalements"].as_array().map_or(0, |l| l.iter().filter(|s| s["nature"] == json!(nature)).count());
        if n > 0 {
            deja.insert(nature.into(), json!(format!("Déjà signalé {} fois", n)));
        }
    }
    r["deja_signale"] = Value::Object(deja);
    r
}

/// CE QUI EST PARTI D'UN IMPREVU, ET CE QUI EN RESTE DU : une ligne courte par morceau (`parti`), dans l'ordre ou l'ecran
/// les lit -- les prolongations, les signalements, la DSN qui l'a declare, puis chaque obligation, faite ou due (avec sa
/// pastille) ; « rien n'est encore parti » sinon. Se pose une fois les obligations de l'arret posees.
pub(crate) fn dire_ce_qui_est_parti(mut r: Value) -> Value {
    let jour = |v: &Value| crate::routes::jour_en_clair(v);
    let minuscule = |t: String| -> String { t.chars().take(1).flat_map(char::to_lowercase).chain(t.chars().skip(1)).collect() };
    let mut parti: Vec<Value> = Vec::new();
    for p in r["prolongations"].as_array().into_iter().flatten().filter(|p| p["fin"] != p["fin_avant"]) {
        parti.push(json!({"phrase": format!("prolongé jusqu'au {} (avis du {})", jour(&p["fin"]), jour(&p["le"]))}));
    }
    for s in r["signalements"].as_array().into_iter().flatten() {
        let quoi = if s["nature"] == json!("05") { "reprise signalée" } else if s["type"] == json!("03") { "signalement rectifié" } else { "signalé" };
        parti.push(json!({"phrase": format!("{} le {}", quoi, crate::routes::quand(&valeur_en_texte(&s["quand"])))}));
    }
    if r["reporte_par"].as_array().is_some_and(|l| !l.is_empty()) {
        let dsn: Vec<String> = r["reporte_par_dit"].as_array().into_iter().flatten().map(valeur_en_texte).collect();
        parti.push(json!({"phrase": format!("déclaré par la DSN {}", dsn.join(", "))}));
    }
    for o in r["obligations"].as_array().into_iter().flatten() {
        let libelle = minuscule(valeur_en_texte(&o["libelle"]));
        if vrai(&o["fait"]["le"]) && o["ou"].is_array() {
            parti.push(json!({"phrase": format!("{}, faite le {}", libelle, jour(&o["fait"]["le"]))}));
        } else if o["du"] == json!(true) {
            parti.push(json!({"phrase": format!("{}, {}", libelle, valeur_en_texte(&o["butoir_dit"])), "pastille": o["pastille"]}));
        }
    }
    if parti.is_empty() {
        parti.push(json!({"phrase": "rien n'est encore parti", "discret": true}));
    }
    r["parti"] = Value::Array(parti);
    r
}

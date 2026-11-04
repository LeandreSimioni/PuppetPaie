//! Les routes des contrats : ce que le contrat a signer et l'avenant montrent, les envoyer a la signature ou les
//! imprimer, les relancer, les noter revenus signes, et les pieces parties d'un engagement.

use crate::compagnie::signature::jj;
use std::collections::HashMap;
use std::path::Path;

use serde_json::{json, Map, Value};

use crate::compagnie::depot_acte::{verifier_preuve, Preuve};
use crate::routes::catalogue::{Acte, Affiche, Champ, Forme, Fiche, Refus, ILLISIBLE, MANQUE, MOIS, OPERATION, SALARIE};
use crate::routes::catalogue::Route;
use crate::routes::documents::{charge_texte, client_courriel, cle, liste_de, nul, preuve_de, refus, requis, vrai_ou, Ouvrir};
use crate::routes::engagement::{booleen, en_echec, maintenant, texte_requis};
use crate::routes::etapes::Moteur;
use droit::contexte::Contexte;
use crate::compagnie::signature::{Choisis, UnAvenant};
use envois::etats::{self, est, get, getd, items, objet_vide, ou, DPAE, SIGNATURE};
use envois::journal;
use socle::date::jour_iso;
use socle::echec::{erreur, Echec, R};
use socle::fichiers::{repr, vrai};
use socle::valeur::{nom_type, valeur_en_texte, valeurs_egales};
use crate::routes::catalogue::{Ecran, Place};



const CONTRATS: Champ = Champ { nom: "contrats", libelle: Affiche::new("Contrats"), forme: Forme::Objet, sens: "les contrats choisis : [{salarie, debut, numero}]", avance: false };
const DEBUT: Champ = Champ { nom: "debut", libelle: Affiche::new("Début de l'engagement"), forme: Forme::Date, sens: "le premier jour de l'engagement, AAAA-MM-JJ", avance: false };
const NUMERO: Champ = Champ { nom: "numero", libelle: Affiche::new("Numéro du contrat"), forme: Forme::Texte, sens: "le numéro du contrat, quand deux commencent le même jour", avance: true };
/// Un contrat ecrit refuse de se faire quand une case qu'il appelle est vide : elle se dit dans la saisie de l'engagement.
const CONTRAT_ECRIT: Refus = Refus {
    classe: "ErreurOperation",
    corriger: "ce que la phrase dit ; une case du contrat écrit vide (`contrat.ecrits` de la route apercu) se remplit dans la saisie de l'engagement, par modifier-engagement ; rien n'est écrit",
};

/// Ou ces routes se voient a l'ecran (une place par module).
const ECRAN: Ecran = Ecran::Dessine(Place::Paie);

/// Les routes de ce module, et leurs fiches.
pub const ROUTES: &[Fiche] = &[
    Fiche {
        route: Route::SignatureApercu,
        geste: "Montrer les contrats du mois : à envoyer, en attente de signature, en avenant.",
        charge: &[MOIS],
        exemple: r#"{"mois":"2026-11"}"#,
        rend: "contrats (chacun : son état, et ses gestes recu et relancer, route et charge), boite_liee, signature (l'encart de la signature d'un clic), rien_a_envoyer, rien_en_attente",
        refus: &[MANQUE, ILLISIBLE],
        acte: Acte::Lit,
        avance: false,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::DemanderSignature,
        geste: "Faire signer par courriel les contrats et les avenants cochés.",
        charge: &[
            MOIS,
            CONTRATS,
            Champ { nom: "avenants", libelle: Affiche::new("Avenants"), forme: Forme::Objet, sens: "les avenants cochés : [{salarie, debut, numero, mois, effet}]", avance: false },
            Champ { nom: "salaries", libelle: Affiche::new("Salariés"), forme: Forme::Liste, sens: "tous les contrats de ces salariés", avance: true },
            Champ { nom: "relance", libelle: Affiche::new("Relancer"), forme: Forme::OuiNon, sens: "vrai : relancer ceux qui attendent leur signature", avance: true },
            Champ { nom: "remplace", libelle: Affiche::new("Annule et remplace"), forme: Forme::OuiNon, sens: "vrai : un contrat refait qui annule et remplace le précédent", avance: true },
        ],
        exemple: r#"{"mois":"2026-11","contrats":[{"salarie":"FULLER","debut":"2026-11-27"}]}"#,
        rend: "phrase, partis, contrats (les fichiers partis), avertissements (ceux sans courriel), signatures (compagnie fictive : revenues aussitôt)",
        refus: &[
            Refus { classe: "PreuveInsuffisante", corriger: "le mot de passe du logiciel, tapé par un humain au moment de l'envoi ; rien n'est parti" },
            Refus { classe: "BoiteAbsente", corriger: "poser la boîte courriel (Configuration, Mots de passe), ou imprimer" },
            CONTRAT_ECRIT,
            MANQUE,
        ],
        acte: Acte::Engage,
        avance: false,
        phrase: 600,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::PiecesParties,
        geste: "Lister ce qui est parti pour un contrat : DPAE, contrat envoyé, signé, preuve.",
        charge: &[
            Champ { nom: "numero", libelle: Affiche::new("Numéro du contrat"), forme: Forme::Texte, sens: "le numéro du contrat, tel que signature-apercu ou etat-par-engagement le rend", avance: false },
            Champ { nom: "salarie", libelle: Affiche::new("Salarié"), forme: Forme::Texte, sens: "la clé de la fiche du salarié ; le numéro suffit", avance: true },
        ],
        exemple: r#"{"salarie":"FULLER","numero":"FU1.26.01"}"#,
        rend: "phrase (chaque pièce partie et son jour), dpae, contrat : les fichiers partis, lus au journal",
        refus: &[MANQUE],
        acte: Acte::Lit,
        avance: false,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::AvenantApercu,
        geste: "Montrer l'avenant d'un contrat : ce qui change, par article ou en clair.",
        charge: &[
            SALARIE,
            DEBUT,
            Champ { nom: "numero", libelle: Affiche::new("Numéro du contrat"), forme: Forme::Texte, sens: "le numéro du contrat, tel que signature-apercu le rend", avance: false },
            Champ { nom: "mois", libelle: Affiche::new("Mois"), forme: Forme::Mois, sens: "le mois de l'écran, où se lit un contrat signé hors du logiciel (le mois présent sinon)", avance: true },
        ],
        exemple: r#"{"salarie":"FULLER","debut":"2026-11-27","numero":"FU1.26.01"}"#,
        rend: "le contrat (contrat, date_contrat, signe ; ailleurs : signé hors du logiciel, et contrat_signe : chemin du dossier de son PDF, deposes, phrase), numero de l'avenant, effet, modifications (numero et article, avant, apres ; phrase en clair sans numero), avenants (et la phrase de chacun), destinataire, titre (le contrat comme l'avenant le nomme), situation, encarts",
        refus: &[Refus { classe: "ErreurOperation", corriger: "le salarié, le début et le numéro d'un contrat parti à la signature ou signé hors du logiciel" }, MANQUE, ILLISIBLE],
        acte: Acte::Lit,
        avance: false,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::ContratsImprimer,
        geste: "Réunir en un PDF les contrats et avenants cochés, à remettre en main propre.",
        charge: &[MOIS, CONTRATS, Champ { nom: "avenants", libelle: Affiche::new("Avenants"), forme: Forme::Objet, sens: "les avenants cochés : [{salarie, debut, numero, mois, effet}]", avance: false }],
        exemple: r#"{"mois":"2026-11","contrats":[{"salarie":"SVOBODA","debut":"2026-11-02"}]}"#,
        rend: "impression (le PDF) et son chemin, contrats",
        refus: &[CONTRAT_ECRIT, MANQUE],
        acte: Acte::Ecrit,
        avance: false,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::ContratRecu,
        geste: "Noter qu'un contrat (ou un avenant) est revenu signé, ou retirer la note.",
        charge: &[
            MOIS,
            SALARIE,
            DEBUT,
            NUMERO,
            Champ { nom: "recu", libelle: Affiche::new("Reçu"), forme: Forme::OuiNon, sens: "faux : retirer la note", avance: true },
            Champ { nom: "avenant", libelle: Affiche::new("Avenant signé"), forme: Forme::Texte, sens: "le numéro de l'avenant revenu signé", avance: true },
        ],
        exemple: r#"{"mois":"2026-11","salarie":"FULLER","debut":"2026-11-27"}"#,
        rend: "phrase, recu (quand), annuler (la route et la charge qui défont la note)",
        refus: &[OPERATION, MANQUE],
        acte: Acte::Ecrit,
        avance: false,
        phrase: 200,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::PoserCourriel,
        geste: "Poser le courriel d'un salarié, là où il manque.",
        charge: &[SALARIE, Champ { nom: "email", libelle: Affiche::new("Courriel"), forme: Forme::Texte, sens: "son adresse de courriel", avance: false }],
        exemple: r#"{"salarie":"FULLER","email":"loie.fuller@exemple.invalid"}"#,
        rend: "salarie, email",
        refus: &[crate::routes::fiches::REFUSEE, OPERATION, MANQUE],
        acte: Acte::Ecrit,
        avance: false,
        phrase: 0,
        ecran: ECRAN,
    },
];

/// Une route de ce module.
pub fn route(_o: &HashMap<String, String>, ctx: &Contexte, nom: Route, charge: &Value) -> R<Value> {
    let t = |v: &Value, k: &str| super::mot(&v[k]);
    match nom {
        Route::PoserCourriel => crate::routes::fiches::poser_courriel(ctx, charge).map(|v| {
            let p = format!("Courriel de {} : {}.", t(&v, "salarie"), t(&v, "email"));
            super::phrase_en_tete(v, p)
        }),
        Route::SignatureApercu => signature_apercu(ctx, charge).map(|mut v| {
            let mut lignes: Vec<Value> = v["contrats"].as_array().into_iter().flatten().map(|c| {
                let a = if t(c, "destinataire").is_empty() { "pas de courriel : il s'imprime".to_string() } else { t(c, "destinataire") };
                json!(format!("{} ({}) : {}", t(c, "nom"), t(c, "numero"), a))
            }).collect();
            // Ce que ses deux boites disent : la signature d'un clic, quand une boite courriel est liee ; leur liste vide.
            let aucun = |v: &Value, k: &str| !v["contrats"].as_array().into_iter().flatten().any(|c| vrai(&c[k]));
            if vrai(&v["boite_liee"]) {
                v["signature"] = super::encart("information", "Signature d'un clic", "Le contrat part par courriel avec un bouton « Je signe » ; la réponse du salarié est relevée toute seule, et il reçoit son contrat contresigné.");
            }
            if aucun(&v, "a_envoyer") {
                v["rien_a_envoyer"] = json!("Aucun contrat à faire partir ce mois-ci.");
            }
            if aucun(&v, "en_attente") {
                v["rien_en_attente"] = json!("Aucun contrat n'attend sa signature.");
            }
            lignes.extend(v["avertissements"].as_array().into_iter().flatten().cloned());
            let p = format!("Contrats à faire signer : {}.", super::nombre(&v["contrats"]));
            v["phrases"] = Value::Array(lignes);
            super::phrase_en_tete(v, p)
        }),
        Route::AvenantApercu => avenant_apercu(ctx, charge).map(|mut v| {
            let n = super::nombre(&v["modifications"]);
            let quand = |k: &str| super::quand(&t(&v, k));
            let ailleurs = vrai(&v["ailleurs"]);
            v["situation"] = json!(format!(
                "{} — {}, {} ; {}",
                t(&v, "nom"), t(&v, "spectacle"), t(&v, "periode"),
                if ailleurs {
                    "signé hors de PuppetPaie".to_string()
                } else if vrai(&v["signe"]) {
                    format!("signé le {}", quand("signe"))
                } else {
                    format!("envoyé le {}, pas encore signé", super::jour_en_clair(&v["date_contrat"]))
                }
            ));
            for a in v["avenants"].as_array_mut().into_iter().flatten() {
                let signe = if vrai(&a["signe"]) { format!("signé le {}", super::quand(&t(a, "signe"))) } else { "pas encore revenu signé".to_string() };
                a["phrase"] = json!(format!("Avenant n° {} envoyé le {} — {}", t(a, "numero"), super::quand(&t(a, "quand")), signe));
            }
            let mut encarts = vec![if ailleurs && n > 0 {
                super::encart("information", "", format!(
                    "Le contrat a été signé hors de PuppetPaie : l'avenant n° {} ne cite aucun de ses articles, il dit en clair ce qui change, l'ancienne valeur et la nouvelle ; le reste du contrat ne change pas.",
                    t(&v, "numero")
                ))
            } else if ailleurs {
                super::encart("information", "", "Aucun terme du contrat signé hors de PuppetPaie n'a changé : il n'y a rien à modifier.")
            } else if n > 0 {
                super::encart("information", "", format!(
                    "Le contrat parti se compare à l'engagement d'aujourd'hui : {}. L'avenant n° {} cite l'ancien texte et le nouveau ; le reste du contrat ne change pas.",
                    if n > 1 { format!("{} articles ont changé", n) } else { "un article a changé".to_string() }, t(&v, "numero")
                ))
            } else {
                super::encart("information", "", "Le contrat parti dit déjà tout ce que dit l'engagement : il n'y a rien à modifier.")
            }];
            if n > 0 && vrai(&v["remplacable"]) && vrai(&v["boite_liee"]) {
                encarts.push(super::encart("information", "", format!(
                    "Le contrat n'a pas commencé : il peut aussi être refait en entier. Le nouveau porte en tête « annule et remplace le contrat n° {} du {} ».",
                    t(&v, "contrat"), super::jour_en_clair(&v["date_contrat"])
                )));
            }
            let dits: Vec<Value> = v["modifications"].as_array().into_iter().flatten().filter_map(|m| m.get("phrase").filter(|x| vrai(x)).cloned()).collect();
            let dossier = v["contrat_signe"].get("phrase").filter(|x| vrai(x)).map(|p| json!(format!("{} Son dossier : {}", valeur_en_texte(p), t(&v["contrat_signe"], "chemin"))));
            v["phrases"] = Value::Array(std::iter::once(v["situation"].clone()).chain(super::encarts_en_lignes(&encarts)).chain(dits).chain(dossier).collect());
            v["encarts"] = Value::Array(encarts);
            // Le contrat comme l'avenant le nomme (`{{avenant.reference}}`) : signe hors du logiciel, le papier ne porte
            // pas le numero que le logiciel lui a donne.
            let reference = if ailleurs { format!("commencé le {}", super::jour_en_clair(&v["debut"])) } else { format!("n° {}", t(&v, "contrat")) };
            v["titre"] = json!(format!("Avenant au contrat {}", reference));
            let p = format!("Avenant {} au contrat {} de {}, à effet du {} : {}.", t(&v, "numero"), reference, t(&v, "nom"), super::jour_en_clair(&v["effet"]), super::compte(n, "modification", "modifications"));
            super::phrase_en_tete(v, p)
        }),
        Route::PiecesParties => pieces_parties(ctx, charge),
        Route::DemanderSignature => demander_signature(ctx, charge, &mut || client_courriel(ctx)),
        Route::ContratsImprimer => contrats_imprimer_par(ctx, charge, "interface").map(|v| {
            // Chaque piece imprimee par son numero, comme l'ecran la coche.
            let quoi: Vec<String> = v["contrats"].as_array().into_iter().flatten().map(|c| format!("contrat n° {}", t(c, "numero")))
                .chain(v["avenants"].as_array().into_iter().flatten().map(|c| format!("avenant n° {} au contrat n° {}", t(c, "avenant"), t(c, "numero"))))
                .collect();
            let p = format!("À faire signer en main propre : {} ; imprimés.", quoi.join(", "));
            super::phrase_en_tete(v, p)
        }),
        Route::ContratRecu => contrat_recu(ctx, charge),
        autre => Err(Echec::from(format!("route inconnue : {}", autre))),
    }
}

/// Un humain dit que le contrat signe est revenu (ou retire la note).
/// Une ligne de journal, pas un etat de la saisie : elle date un fait du monde. Rien ne part, aucun compteur ne
/// bouge.
fn contrat_recu(ctx: &Contexte, charge: &Value) -> R<Value> {
    let (mois, salarie, debut) = (texte_requis(charge, "mois")?, texte_requis(charge, "salarie")?, texte_requis(charge, "debut")?);
    let recu = booleen(charge, "recu", true);
    // `numero` (facultatif) dit QUEL contrat du jour : deux contrats du meme salarie peuvent commencer le meme jour.
    let numero = charge.get("numero").filter(|v| vrai(v)).map(valeur_en_texte);
    // `avenant` (facultatif) : le numero de l'avenant revenu signe, quand c'est lui et non le contrat.
    let avenant = match charge.get("avenant").filter(|v| vrai(v)) {
        None => Value::Null,
        Some(v) => json!(valeur_en_texte(v).trim().parse::<i64>().map_err(|_| refus(format!("Numéro d'avenant illisible : {}.", valeur_en_texte(v))))?),
    };
    contrat_recu_avec(ctx, mois, salarie, debut, numero.as_deref(), recu, &avenant, "interface", None)
}

/// `contrat_recu(ctx, mois, salarie, debut, recu=..., qui=...)` : la meme note, `qui` donne (l'horloge d'une
/// compagnie fictive passe `horloge`).
pub(crate) fn contrat_recu_par(
    ctx: &Contexte,
    mois: &str,
    salarie: &str,
    debut: &str,
    numero: Option<&str>,
    recu: bool,
    qui: &str,
) -> R<Value> {
    contrat_recu_avec(ctx, mois, salarie, debut, numero, recu, &Value::Null, qui, None)
}

/// `contrat_recu(..., courriel=...)` : quand la releve des signatures a trouve la reponse « Je signe », elle passe
/// `courriel` (code, adresse, date, identifiant du message, fichier archive, ...). La ligne est alors datee de la
/// SIGNATURE (`courriel["quand"]`), pas du releve, avec le message pour origine, et son detail porte `origine:
/// "courriel"` puis les champs de `courriel` dans l'ordre ou ils sont donnes.
#[allow(clippy::too_many_arguments)]
pub(crate) fn contrat_recu_avec(
    ctx: &Contexte,
    mois: &str,
    salarie: &str,
    debut: &str,
    numero: Option<&str>,
    recu: bool,
    avenant: &Value,
    qui: &str,
    courriel: Option<&Map<String, Value>>,
) -> R<Value> {
    // Les paies telles que la DSN les a declarees, sans rien ecrire.
    let paies = documents::ecrire_documents::paies_aux_numeros_declares(ctx, mois, None, &envois::journal::lu(&ctx.racine).map(|j| j.departs)?).map_err(en_echec)?.1;
    // LE CONTRAT, pas le salarie : son numero, celui que l'ecran affiche (ou que la releve a lu) ; sans numero, l'engagement
    // de ce salarie qui commence ce jour-la.
    let numero = numero.filter(|n| !n.is_empty());
    let cle_de = |p: &engagement::calcul::Paie| p.salarie.id.to_string();
    let trouvee = paies.iter().position(|p| {
        cle_de(p) == salarie
            && match numero {
                Some(n) => p.engagement.numero.as_deref() == Some(n),
                None => p.engagement.debut.iso() == debut,
            }
    });
    let Some(rang) = trouvee else {
        return Err(refus(format!(
            "Aucun engagement de {} commençant le {}{} dans la saisie enregistrée de {}.",
            salarie,
            debut,
            numero.map_or(String::new(), |n| format!(" (n° {})", n)),
            mois
        )));
    };
    let p = &paies[rang];
    let numero_note = p.engagement.numero.clone().map_or(Value::Null, Value::from);
    let nom = p.salarie.nomme().unwrap_or_else(|| (&salarie).to_string());
    let mut detail = json!({
        "salarie": salarie, "debut": p.engagement.debut.iso(), "recu": recu,
        "numero": numero_note, "nom": nom,
    });
    if vrai(avenant) {
        detail["avenant"] = avenant.clone();
    }
    let ce = if vrai(avenant) { format!("avenant n° {}", valeur_en_texte(avenant)) } else { "contrat".to_string() };
    let courriel = courriel.filter(|c| !c.is_empty());
    let texte_du_courriel = |cle: &str, defaut: &str| courriel.and_then(|c| c.get(cle)).map_or_else(|| defaut.to_string(), valeur_en_texte);
    let (reponse, quand) = match courriel {
        Some(c) => {
            detail["origine"] = json!("courriel");
            for (cle, valeur) in c {
                detail[cle.as_str()] = valeur.clone();
            }
            (
                format!("contrat signe par courriel ({}, {})", texte_du_courriel("code", "?"), texte_du_courriel("adresse", "?")),
                c.get("quand").filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default(),
            )
        }
        None => (
            if recu { format!("{} signe recu, range a la main", ce) } else { format!("mention retiree : le {} signe n'est pas la", ce) },
            String::new(),
        ),
    };
    // CE QUI REVIENT SE RANGE SUR LA LIGNE DE L'ENVOI : la derniere qui a fait partir ce contrat (ou cet avenant).
    detail["quand"] = json!(if quand.is_empty() { maintenant() } else { quand });
    detail["qui"] = json!(qui);
    detail["reponse"] = json!(reponse);
    let av = detail.get("avenant").filter(|a| vrai(a)).cloned();
    let porte = |l: &Value| -> bool {
        est(&l["quoi"], SIGNATURE)
            && l.get("accepte").is_some_and(vrai)
            && items(&l["detail"]["contrats"]).unwrap_or_default().iter().any(|c| {
                c["salarie"] == json!(salarie)
                    && valeur_en_texte(&etats::numero_de_cle(&c["numero"])) == valeur_en_texte(&etats::numero_de_cle(&numero_note))
                    && match &av {
                        Some(a) => est(&c["acte"], "avenant") && valeurs_egales(&c["avenant"], a),
                        None => !est(&c["acte"], "avenant"),
                    }
            })
    };
    let Some(ligne) = journal::rattacher(&ctx.racine, porte, detail.clone()).map_err(Echec::from)? else {
        return Err(refus(format!("Le {} de {} n'est jamais parti : rien ne peut en revenir. Rien n'est écrit.", ce, nom)));
    };
    // Ce que l'ecran dit de la note.
    let phrase = format!(
        "{} de {} {}.",
        socle::texte::capitaliser(&ce),
        nom,
        if recu { "noté reçu signé" } else { "de nouveau en attente" }
    );
    Ok(json!({
        "salarie": salarie, "debut": debut,
        "phrase": phrase,
        "recu": if recu { detail["quand"].clone() } else { Value::Null },
        // De quoi revenir sur la note : la meme route, le mot contraire.
        "annuler": {"route": Route::ContratRecu, "charge": {"mois": mois, "salarie": salarie, "debut": debut, "numero": numero_note, "avenant": avenant, "recu": !recu}},
        "ligne": ligne,
    }))
}

/// Les contrats que la charge choisit : `salaries` (tous leurs contrats) et `contrats` ([{"salarie", "debut", "numero"}]).
fn choisis_de(charge: &Value) -> R<Choisis> {
    let salaries = liste_de(charge, "salaries")?.map(|l| l.iter().map(valeur_en_texte).collect());
    let contrats = match liste_de(charge, "contrats")? {
        None => None,
        Some(l) => {
            let mut v = Vec::new();
            for c in l {
                let numero = c.get("numero").filter(|n| vrai(n)).map(valeur_en_texte);
                v.push((valeur_en_texte(requis(c, "salarie")?), valeur_en_texte(requis(c, "debut")?), numero));
            }
            Some(v)
        }
    };
    Ok(Choisis { salaries, contrats })
}

/// UN CONTRAT PARTI PUIS CHANGE NE REPART PAS EN CONTRAT : signe ou non, commence ou non, il se modifie par un avenant
/// (`signature-apercu` le dit, `en_avenant`). Refus quand un contrat vise expressement (`contrats`, `salaries`) en est un ;
/// rien n'est ecrit.
fn refuser_les_contrats_partis(ctx: &Contexte, mois: &str, choisis: &crate::compagnie::signature::Choisis) -> R<()> {
    if choisis.contrats.is_none() && choisis.salaries.is_none() {
        return Ok(());
    }
    let apercu = signature_apercu(ctx, &json!({"mois": mois}))?;
    for c in apercu["contrats"].as_array().into_iter().flatten().filter(|c| vrai(&c["en_avenant"])) {
        let (salarie, debut) = (valeur_en_texte(&c["salarie"]), valeur_en_texte(&c["debut"]));
        let numero = valeur_en_texte(&etats::numero_de_cle(&c["numero"]));
        let vise = choisis.salaries.as_ref().is_some_and(|l| l.contains(&salarie))
            || choisis.contrats.as_ref().is_some_and(|l| {
                l.iter().any(|(s, d, n)| *s == salarie && *d == debut && n.as_ref().map_or(true, |n| *n == numero))
            });
        if vise {
            return Err(refus(format!(
                "Le contrat de {} est déjà parti et l'engagement a changé depuis : il se modifie par un avenant, que la boîte « Faire signer les contrats » propose. Rien n'est parti.",
                valeur_en_texte(&c["nom"])
            )));
        }
    }
    Ok(())
}

/// La route `contrats-imprimer` (`signature::contrats_imprimer`), `qui` donne (l'horloge d'une compagnie fictive
/// passe le sien).
/// `avenants` : les avenants des contrats signes qui ont change ([{"salarie", "debut", "numero", "mois", "effet"}]),
/// imprimes avec eux ; sans `contrats` ni `salaries`, seuls les avenants s'impriment.
/// Une charge qui coche une liste vide (`contrats` ou `salaries` donnes vides, sans avenant) : rien n'est choisi.
fn aucun_coche(charge: &Value) -> R<()> {
    let vide = |k: &str| charge.get(k).and_then(Value::as_array).is_none_or(Vec::is_empty);
    let donne = ["contrats", "salaries"].iter().any(|k| charge.get(*k).is_some());
    if donne && vide("contrats") && vide("salaries") && vide("avenants") {
        return Err(refus("Aucun contrat coché. Rien n'est parti."));
    }
    Ok(())
}

pub(crate) fn contrats_imprimer_par(ctx: &Contexte, charge: &Value, qui: &str) -> R<Value> {
    aucun_coche(charge)?;
    let voulus = liste_de(charge, "avenants")?.cloned().unwrap_or_default();
    let avenants = avenants_voulus(ctx, &voulus)?;
    if avenants.iter().any(|(_, aa)| aa.iter().any(UnAvenant::relance)) {
        return Err(refus("Une relance ne s'imprime pas : l'avenant imprimé se remet en main propre. Rien n'est parti."));
    }
    let contrats_choisis = ["contrats", "salaries"].iter().any(|k| charge.get(*k).is_some_and(vrai));
    let choisis = choisis_de(charge)?;
    refuser_les_contrats_partis(ctx, &charge_texte(charge, "mois")?, &choisis)?;
    let choisis = (contrats_choisis || avenants.is_empty()).then_some(&choisis);
    crate::compagnie::signature::contrats_imprimer(ctx, &charge_texte(charge, "mois")?, choisis, &avenants, qui)
}

/// La route `demander-signature` : faire signer, en un geste, les contrats choisis (`contrats`, `salaries`) et les
/// avenants coches (`avenants`). Tous les refus d'abord, puis la boite et UNE preuve, puis les contrats
/// (`signature::demander_signature`) et les avenants (`envoyer_avenants`, un courriel par salarie). `relance` : l'etat des
/// contrats du mois (`signature-apercu`) dit lesquels peuvent se relancer.
pub(crate) fn demander_signature(ctx: &Contexte, charge: &Value, ouvrir: Ouvrir) -> R<Value> {
    aucun_coche(charge)?;
    let mois = charge_texte(charge, "mois")?;
    let relance = charge.get("relance").is_some_and(vrai);
    let remplace = charge.get("remplace").is_some_and(vrai);
    let avenants = liste_de(charge, "avenants")?.cloned().unwrap_or_default();
    let des_contrats = avenants.is_empty() || ["contrats", "salaries"].iter().any(|k| charge.get(*k).is_some_and(vrai));
    // Un avenant sans objet ou une date d'effet illisible refuse le tout, avant que rien ne parte.
    avenants_voulus(ctx, &avenants)?;
    let mut preuve = preuve_de(charge);
    let mut r = json!({"partis": [], "contrats": [], "avertissements": []});
    if des_contrats {
        let choisis = choisis_de(charge)?;
        let a_qui = match &choisis.salaries {
            Some(l) if !l.is_empty() => format!("{} salarie(s) : {}", l.len(), l.join(", ")),
            _ => "les salaries du mois qui ont un contrat a signer".to_string(),
        };
        let etats_des_contrats = if relance {
            let apercu = crate::routes::contrats::signature_apercu(ctx, &json!({"mois": mois}))?;
            apercu.get("contrats").and_then(Value::as_array).cloned().unwrap_or_default()
        } else {
            Vec::new()
        };
        if !relance && !remplace {
            refuser_les_contrats_partis(ctx, &mois, &choisis)?;
        }
        let demande = crate::compagnie::signature::DemandeDeSignature { mois: &mois, relance, remplace, choisis, etats_des_contrats };
        // Un refus se dit d'abord : la boite et la preuve ne se demandent que pour ce qui part.
        crate::compagnie::signature::verifier_demande(ctx, &demande)?;
        let mut client = ouvrir()?;
        let quoi = if relance { "relance de demande de signature de contrat" } else if remplace { "contrat refait, a signer (il annule et remplace le precedent)" } else { "demande de signature de contrat" };
        preuve = Preuve { donnee: Some(verifier_preuve(ctx, Some(&preuve), &mois, quoi, &a_qui)?), ..Preuve::default() };
        r = crate::compagnie::signature::demander_signature(ctx, &demande, client.as_mut())?;
    }
    let n_contrats = r["contrats"].as_array().map_or(0, Vec::len);
    let mut n_avenants = 0;
    if !avenants.is_empty() {
        let (av, avertissements) = envoyer_avenants(ctx, &avenants, &preuve, ouvrir)?;
        n_avenants = av.avenants.len();
        if let (Some(p), Some(c), Some(a)) = (r["partis"].as_array().cloned(), r["contrats"].as_array().cloned(), r["avertissements"].as_array().cloned()) {
            r["partis"] = json!(p.into_iter().chain(av.partis.iter().map(|x| json!(x))).collect::<Vec<_>>());
            r["contrats"] = json!(c.into_iter().chain(av.avenants.iter().map(|x| json!({"fichier": x["fichier"]}))).collect::<Vec<_>>());
            r["avertissements"] = json!(a.into_iter().chain(avertissements.into_iter().map(Value::String)).collect::<Vec<_>>());
        }
    }
    let quoi = [(n_contrats, "contrat", "contrats"), (n_avenants, "avenant", "avenants")].iter().filter(|(n, _, _)| *n > 0).map(|(n, un, des)| super::compte(*n, un, des)).collect::<Vec<_>>().join(" et ");
    let avertissements = r["avertissements"].as_array().into_iter().flatten().filter_map(Value::as_str).collect::<Vec<_>>().join(" ");
    let relancee = relance || (!des_contrats && avenants.iter().all(|a| a.get("relance").is_some_and(vrai)));
    let s = if n_contrats + n_avenants > 1 { "s" } else { "" };
    let fait = if relancee { format!("relancé{}", s) } else { format!("envoyé{} à la signature", s) };
    let mut phrase = format!("{} {}.{}{}", quoi, fait, if avertissements.is_empty() { "" } else { " " }, avertissements);
    // LE SIMULATEUR REPOND DANS LE GESTE : les salaries d'une compagnie fictive signent aussitot, et la vraie releve lit
    // leur reponse (`releve_signatures::boite_des_signatures`).
    if crate::compagnie::bridage::branchement(&ctx.racine).joue() {
        let releve = crate::compagnie::releve_signatures::relever_signatures(ctx, None, None, "interface")?;
        let resume = releve.resume();
        if !resume.is_empty() {
            phrase.push_str(&format!(" {}.", resume.trim_end_matches('.')));
        }
        r["signatures"] = releve.en_json();
    }
    r["phrase"] = json!(phrase);
    Ok(r)
}

/// LES AVENANTS VOULUS, verifies avant que rien ne parte : chacun a son apercu (`avenant-apercu`), qui dit ce qui change
/// (un avenant sans objet ou une date d'effet illisible refuse le tout) ; une relance (`relance`) reprend l'avenant parti
/// et pas revenu, tel quel. Regroupes par salarie.
fn avenants_voulus(ctx: &Contexte, voulus: &[Value]) -> R<Vec<(String, Vec<UnAvenant>)>> {
    let mut par_salarie: Vec<(String, Vec<UnAvenant>)> = Vec::new();
    for v in voulus {
        let salarie_v = requis(v, "salarie")?;
        let debut_v = requis(v, "debut")?;
        let un = if v.get("relance").is_some_and(vrai) {
            avenant_a_relancer(ctx, v)?
        } else {
            // `numero` (celui que `signature-apercu` rend) et `mois` (celui de l'ecran) disent QUEL contrat du jour : facultatifs.
            let apercu = crate::routes::contrats::avenant_apercu(
                ctx,
                &json!({"salarie": salarie_v, "debut": debut_v, "numero": v.get("numero").cloned().unwrap_or(Value::Null), "mois": v.get("mois").cloned().unwrap_or(Value::Null)}),
            )?;
            let modifications = apercu["modifications"].as_array().map_or(0, Vec::len);
            let nom = apercu.get("nom").map(valeur_en_texte).unwrap_or_default();
            if modifications == 0 {
                return Err(refus(format!(
                    "{}, contrat du {} : le contrat parti dit déjà ce que dit l'engagement, pas d'avenant à faire. Rien n'est parti.",
                    nom,
                    jj(&valeur_en_texte(debut_v))?
                )));
            }
            let effet = vrai_ou(v, "effet").unwrap_or_else(|| apercu.get("effet").map(valeur_en_texte).unwrap_or_default());
            if jour_iso(&effet).is_none() {
                return Err(refus(format!("Date d'effet illisible : {}.", repr(&effet))));
            }
            UnAvenant { apercu, effet }
        };
        let cle = valeur_en_texte(salarie_v);
        match par_salarie.iter_mut().find(|(k, _)| *k == cle) {
            Some((_, l)) => l.push(un),
            None => par_salarie.push((cle, vec![un])),
        }
    }
    Ok(par_salarie)
}

/// L'AVENANT A RELANCER : celui que `signature-apercu` montre en attente pour ce contrat, parti par courriel. Il repart
/// tel qu'il est parti (son PDF, ses modifications). Un contrat dont aucun avenant n'attend, ou dont l'avenant est parti
/// imprime, se refuse : rien n'est ecrit.
fn avenant_a_relancer(ctx: &Contexte, v: &Value) -> R<UnAvenant> {
    let (salarie, debut) = (valeur_en_texte(requis(v, "salarie")?), valeur_en_texte(requis(v, "debut")?));
    let qui = ctx.nomme(&salarie);
    let numero = vrai_ou(v, "numero").unwrap_or_default();
    let apercu = crate::routes::contrats::signature_apercu(ctx, &json!({"mois": requis(v, "mois")?}))?;
    let c = apercu["contrats"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|c| c["salarie"].as_str() == Some(&salarie) && c["debut"].as_str() == Some(&debut) && (numero.is_empty() || valeur_en_texte(&c["numero"]) == numero))
        .ok_or_else(|| refus(format!("Aucun contrat de {} commençant le {} dans {}. Rien n'est parti.", qui, debut, valeur_en_texte(&v["mois"]))))?;
    let a = &c["avenant"];
    if !vrai(&c["avenant_attendu"]) || !vrai(&c["en_attente"]) {
        return Err(refus(format!("Le contrat de {} du {} n'a aucun avenant en attente de signature : il n'y a rien à relancer. Rien n'est parti.", qui, jj(&debut)?)));
    }
    if c["par"].as_str() != Some("courriel") {
        return Err(refus(format!("L'avenant n° {} de {} est parti imprimé, à remettre en main propre : il ne se relance pas. Rien n'est parti.", valeur_en_texte(&a["numero"]), qui)));
    }
    let apercu = json!({
        "salarie": salarie, "debut": debut, "fin": c["fin"], "spectacle": c["spectacle"], "mois": a["mois"],
        "contrat": a["contrat"], "numero": a["numero"], "fichier": a["fichier"], "modifications": a["modifications"],
        "destinataire": c["destinataire"], "relance": true,
    });
    Ok(UnAvenant { apercu, effet: valeur_en_texte(&a["effet"]) })
}

/// Les avenants voulus, verifies, puis la boite, la preuve, et `signature::envoyer_avenants`. UN SALARIE SANS
/// COURRIEL n'empeche pas les autres : ses avenants ne partent pas, ils se nomment dans le second terme (la phrase de
/// `signature::avenants_sans_courriel_dit`) ; refus si aucun avenant coche n'a de courriel.
pub(crate) fn envoyer_avenants(ctx: &Contexte, voulus: &[Value], preuve: &Preuve, ouvrir: Ouvrir) -> R<(crate::compagnie::signature::Avenants, Vec<String>)> {
    if voulus.is_empty() {
        return Err(refus("Aucun avenant coché. Rien n'est parti."));
    }
    // La porte du branchement, avant de rien lire.
    crate::compagnie::bridage::branchement(&ctx.racine).ouvrir()?;
    let (par_salarie, sans): (Vec<_>, Vec<_>) =
        avenants_voulus(ctx, voulus)?.into_iter().partition(|(_, aa)| aa.iter().all(|a| !a.texte("destinataire").trim().is_empty()));
    let noms_sans: Vec<String> = sans.iter().map(|(k, _)| ctx.nomme(k)).collect();
    if par_salarie.is_empty() {
        return Err(refus(format!("{} Rien n'est parti.", crate::compagnie::signature::avenants_sans_courriel_dit(&noms_sans))));
    }
    let avertissements = if noms_sans.is_empty() { Vec::new() } else { vec![crate::compagnie::signature::avenants_sans_courriel_dit(&noms_sans)] };
    crate::compagnie::signature::controler_avenants(ctx, &par_salarie)?;
    let mut client = ouvrir()?;
    let mut mois_vus: Vec<String> = par_salarie.iter().flat_map(|(_, aa)| aa.iter().map(|a| a.texte("mois"))).collect();
    mois_vus.sort();
    let a_qui = format!("{} salarie(s) : {}", par_salarie.len(), par_salarie.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>().join(", "));
    let quoi = if par_salarie.iter().any(|(_, aa)| aa.iter().any(UnAvenant::relance)) { "relance d'avenant(s) a signer" } else { "envoi d'avenant(s) a signer" };
    verifier_preuve(ctx, Some(preuve), mois_vus.first().map_or("", String::as_str), quoi, &a_qui)?;
    Ok((crate::compagnie::signature::envoyer_avenants(ctx, &par_salarie, client.as_mut())?, avertissements))
}

/// `apercu_signature` : les contrats du mois et l'etat de leur demande de signature.
pub(crate) fn signature_apercu(ctx: &Contexte, charge: &Value) -> R<Value> {
    let mois_v = cle(charge, "mois")?;
    let mois = valeur_en_texte(mois_v);
    let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs)?;
    let (_data, paies) = documents::ecrire_documents::paies_aux_numeros_declares(ctx, &mois, None, &departs)?;
    let mut m = Moteur::nouveau(ctx);
    let g = m.globales()?;
    let rangs: Vec<(String, String)> = paies
        .iter()
        .map(|p| Ok((p.salarie.id.to_string(), p.engagement.debut.iso())))
        .collect::<R<_>>()?;
    let avenants = crate::routes::etapes::avenants_a_faire(ctx, &mois)?;

    let boite_liee = crate::routes::ouverture::boite_liee(ctx)?;
    let mut contrats = Vec::new();
    for (i, p) in paies.iter().enumerate() {
        // SEUL UN CONTRAT ECRIT SE FAIT SIGNER ICI (`engagement::contrat_a_signer`) ; un contrat signe hors du logiciel
        // (`engagement::signe_ailleurs`) n'y vient que pour son avenant, a faire ou attendu.
        let ailleurs = engagement::signe_ailleurs(&ctx.racine, &p.engagement).map_err(Echec::from)?;
        if !ailleurs && !engagement::contrat_a_signer(&ctx.racine, &p.engagement).map_err(Echec::from)? {
            continue;
        }
        let (cle_s, debut) = rangs[i].clone();
        let k = (cle_s.clone(), p.engagement.numero.as_deref().unwrap_or_default().to_string());
        let etat = crate::routes::etapes::etat_signature(
            crate::routes::etapes::contrat_parti_le(&departs, &cle_s, p.engagement.numero.as_deref()),
            g.par.get(&k).unwrap_or(nul()),
            g.recus.get(&k).unwrap_or(nul()),
            g.renvoyes.contains(&k),
            g.avenants.get(&k).unwrap_or(nul()),
            avenants.contains(&(cle_s.clone(), p.engagement.numero.as_deref().unwrap_or_default().to_string())),
            ailleurs,
        );
        if ailleurs && !etat.caduc && !etat.avenant_attendu {
            continue;
        }
        let sal = &p.salarie;
        let numero = p.engagement.numero.clone().map_or(Value::Null, Value::from);
        let fin = p.engagement.fin.map(|f| f.iso());
        let mut l = Map::new();
        l.insert("salarie".into(), json!(cle_s));
        l.insert("nom".into(), json!(sal.nomme().unwrap_or_else(|| (&cle_s).to_string())));
        l.insert("destinataire".into(), sal.email.dite().map_or(json!(""), |e| json!(e)));
        l.insert("spectacle".into(), json!(p.titre()?));
        l.insert("debut".into(), json!(debut));
        l.insert("fin".into(), json!(fin));
        l.insert("numero".into(), numero);
        l.insert("demande_le".into(), json!(etat.fait));
        // CE QUE LA BOITE EN FAIT : un contrat a envoyer (jamais parti, ou caduc) se coche, et part en avenant s'il est
        // deja parti, signe ou non (`en_avenant`) ; un contrat ou un avenant parti et pas revenu attend : il se note reçu signe
        // (`recu`), et seul celui parti par courriel se relance (`relancer`), quand la boite est posee. Les deux gestes
        // sont des routes et leurs charges : la page n'en compose aucun.
        let parti = l["demande_le"].clone();
        let (recu, caduc) = (vrai(&etat.recu), etat.caduc);
        let avenant_en_attente = !caduc && etat.avenant_attendu;
        let en_attente = !caduc && ((vrai(&parti) && !recu) || avenant_en_attente);
        let a_envoyer = caduc || (!recu && !vrai(&parti));
        let par = etat.par.clone();
        let (ce, depuis) = if avenant_en_attente {
            (format!("avenant n° {} ", valeur_en_texte(&etat.avenant["numero"])), etat.avenant["quand"].clone())
        } else {
            (String::new(), parti.clone())
        };
        let jour = valeur_en_texte(&depuis);
        let jj_mm = if jour.len() >= 10 { format!("{}/{}", &jour[8..10], &jour[5..7]) } else { jour };
        let attente = format!("{}{} le {}", ce, if est(&par, "imprime") { "imprimé" } else { "envoyé" }, jj_mm);
        // Ce que la demande du mot de passe dit avant une relance.
        let pourquoi = format!("Relancer la signature : {}, {}.", valeur_en_texte(&l["nom"]), attente);
        let numero_charge = p.engagement.numero.clone().map_or(json!(""), Value::from);
        let (recu_g, relancer) = if en_attente {
            let avenant = if avenant_en_attente { etat.avenant["numero"].clone() } else { Value::Null };
            let recu_g = json!({"route": Route::ContratRecu, "charge": {"mois": mois, "salarie": cle_s, "debut": debut, "numero": numero_charge, "avenant": avenant, "recu": true}});
            let relancer = if !(est(&par, "courriel") && boite_liee) {
                Value::Null
            } else if avenant_en_attente {
                json!({"route": Route::DemanderSignature, "pourquoi": pourquoi, "charge": {"mois": mois, "avenants": [{"salarie": cle_s, "debut": debut, "numero": numero_charge, "mois": mois, "relance": true}]}})
            } else {
                json!({"route": Route::DemanderSignature, "pourquoi": pourquoi, "charge": {"mois": mois, "relance": true, "contrats": [{"salarie": cle_s, "debut": debut, "numero": numero_charge}]}})
            };
            (recu_g, relancer)
        } else {
            (Value::Null, Value::Null)
        };
        // Sa pastille : un contrat parti puis change part en avenant, signe ou non, commence ou non ; parti et pas revenu,
        // il attend sa signature. Toutes a faire : la compagnie le peut.
        let pourquoi_dit = Some(etat.pourquoi).filter(|t| !t.is_empty()).map(String::from);
        let pastille = if caduc {
            let quand = if recu { " (signé)".to_string() } else { l["demande_le"].as_str().map_or(String::new(), |q| format!(" (parti le {})", super::quand(q))) };
            super::pastille("a_faire", &format!("{}{}", pourquoi_dit.unwrap_or_else(|| crate::routes::etapes::CONTRAT_A_REFAIRE.into()), quand))
        } else if en_attente {
            super::pastille("en_attente", &attente)
        } else {
            Value::Null
        };
        l.insert("pastille".into(), pastille);
        l.insert("par".into(), par.clone());
        l.insert("a_envoyer".into(), json!(a_envoyer));
        l.insert("en_avenant".into(), json!(caduc));
        l.insert("en_attente".into(), json!(en_attente));
        l.insert("recu".into(), recu_g);
        l.insert("relancer".into(), relancer);
        l.insert("recu_le".into(), etat.recu.clone());
        l.insert("caduc".into(), json!(etat.caduc));
        let champ = |k: &str| l.get(k).cloned().unwrap_or(Value::Null);
        let (debut, fin, numero) = (champ("debut"), champ("fin"), champ("numero"));
        l.insert("periode_dit".into(), json!(super::periode_en_clair(&debut, &fin)));
        l.insert("numero_dit".into(), if vrai(&numero) { json!(format!("contrat n° {}", valeur_en_texte(&numero))) } else { Value::Null });
        l.insert("pourquoi".into(), json!(etat.pourquoi));
        l.insert("avenant".into(), etat.avenant.clone());
        l.insert("avenant_attendu".into(), json!(etat.avenant_attendu));
        contrats.push(Value::Object(l));
    }
    // Les contrats et les avenants a faire partir dont le salarie n'a pas de courriel : ce que l'envoi en dira.
    let (mut sans, mut sans_avenant): (Vec<String>, Vec<String>) = (Vec::new(), Vec::new());
    for c in &contrats {
        if boite_liee && vrai(&c["a_envoyer"]) && valeur_en_texte(&c["destinataire"]).trim().is_empty() {
            let l = if vrai(&c["en_avenant"]) { &mut sans_avenant } else { &mut sans };
            let nom = valeur_en_texte(&c["nom"]);
            if !l.contains(&nom) {
                l.push(nom);
            }
        }
    }
    let mut avertissements: Vec<String> = Vec::new();
    if !sans.is_empty() {
        avertissements.push(crate::compagnie::signature::sans_courriel_dit(&sans));
    }
    if !sans_avenant.is_empty() {
        avertissements.push(crate::compagnie::signature::avenants_sans_courriel_dit(&sans_avenant));
    }
    Ok(json!({
        "mois": mois_v,
        "contrats": contrats,
        "boite_liee": boite_liee,
        "avertissements": avertissements,
    }))
}

/// Seuls les quatre champs d'identite que la demande affiche passent a `signatures_attendues` (`journal.signatures_attendues`).
pub(crate) fn salaries_d_identite(ctx: &Contexte) -> Value {
    let mut salaries = Map::new();
    for (k, f) in &ctx.salaries {
        let mut garde = Map::new();
        for (c, x) in [("prenom_affichage", &f.prenom_affichage), ("prenom", &f.prenom.en_texte()), ("nom_affichage", &f.nom_affichage), ("nom", &f.nom.en_texte())] {
            if !x.est_absente() {
                garde.insert(c.to_string(), x.en_valeur());
            }
        }
        salaries.insert(k.to_string(), Value::Object(garde));
    }
    Value::Object(salaries)
}

/// `apercu_avenant` : ce que l'avenant d'un contrat parti modifierait, article par article, l'ancien
/// texte (celui de la version sur laquelle il est parti) et le nouveau (la paie validee). Rien n'est ecrit.
pub(crate) fn avenant_apercu(ctx: &Contexte, charge: &Value) -> R<Value> {
    let salarie_v = cle(charge, "salarie")?;
    let debut_v = cle(charge, "debut")?;
    let mut m = Moteur::nouveau(ctx);
    let lignes = m.lignes()?;
    // Le contrat que la page vise : son numero (que `signature-apercu` rend).
    let numero_voulu = charge.get("numero").filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default();
    let parti = match salarie_v.as_str() {
        Some(s) if !numero_voulu.is_empty() => envois::etats::contrat_parti(&lignes, s, &numero_voulu)?,
        _ => None,
    };
    let Some(parti) = parti else {
        if let Some(v) = avenant_d_un_contrat_signe_ailleurs(ctx, charge, &numero_voulu)? {
            return Ok(v);
        }
        if numero_voulu.is_empty() {
            return Err(erreur("KeyError", socle::echec::cle("Le numéro du contrat désigne le contrat parti dont l'avenant se fait, et la demande ne le dit pas. Rien n'est écrit.", "numero")));
        }
        return Err(erreur(
            "ErreurOperation",
            format!("Aucun contrat n'est parti à la signature pour {} du {} : il n'y a rien à modifier.", valeur_en_texte(salarie_v), documents::bulletin::jj_mm_aaaa(debut_v).map_err(|m| erreur("ValueError", m))?),
        ));
    };
    let (salarie, debut) = (valeur_en_texte(salarie_v), valeur_en_texte(debut_v));
    let mois_parti = valeur_en_texte(&parti.mois);
    let (_data, paies) = documents::ecrire_documents::paies_aux_numeros_declares(ctx, &mois_parti, None, &envois::journal::lu(&ctx.racine).map(|j| j.departs)?)?;
    // La paie de CE contrat : celle qui porte son numero.
    let numero_parti = valeur_en_texte(&etats::numero_de_cle(&parti.numero));
    let trouvee = etats::paie_du_contrat(&paies, &salarie, &numero_parti)?;
    let Some(i) = trouvee else {
        return Err(erreur(
            "ErreurOperation",
            format!("Aucun contrat de {} commençant le {} dans les engagements de {}.", salarie, documents::bulletin::jj_mm_aaaa(debut_v).map_err(|m| erreur("ValueError", m))?, mois_parti),
        ));
    };
    // L'ANCIEN TEXTE : celui que le contrat disait sur la version de l'engagement sur laquelle il (ou son dernier avenant)
    // est parti. Ce qui change se compare au mois ou le contrat change (`engagement::etats::mois_du_changement`) : un
    // contrat au mois qui change a partir d'un mois d'apres celui ou il est parti se compare ce mois-la, et l'avenant
    // prend effet le premier jour de ce mois (ou a la date de l'avenant que la saisie de ce mois dit).
    let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs)?;
    let siens = engagement::etats::du_mois(ctx, &mois_parti, &envois::journal::lu(&ctx.racine).map(|j| j.departs)?)?.into_iter().find_map(|e| {
        let v = e.valide()?.clone();
        let b = &*v.objet().brut;
        (engagement::cle_de_l_engagement(ctx, &b).as_deref() == Some(salarie.as_str()) && b.get("numero").map(valeur_en_texte).as_deref() == paies[i].engagement.numero.as_deref()).then_some(v)
    });
    let passee = siens.as_ref().and_then(|v| engagement::etats::version_au_depart(ctx, v, &departs, engagement::departs::Envoi::Contrat));
    let mois_effet = match (siens.as_ref(), passee) {
        (Some(v), Some(x)) => engagement::etats::mois_du_changement(v, x).map_err(Echec::from)?.filter(|m| *m > mois_parti),
        _ => None,
    };
    let (paies, i, mois_compare) = match &mois_effet {
        Some(me) => {
            let (_d, ps) = documents::ecrire_documents::paies_aux_numeros_declares(ctx, me, None, &envois::journal::lu(&ctx.racine).map(|j| j.departs)?)?;
            let j = etats::paie_du_contrat(&ps, &salarie, &numero_parti)?.ok_or_else(|| {
                erreur("ErreurOperation", format!("Aucune paie de {} en {} pour le contrat n° {}.", salarie, socle::mois::Mois::en_clair(me), numero_parti))
            })?;
            (ps, j, me.clone())
        }
        None => (paies, i, mois_parti.clone()),
    };
    let effet = match (&mois_effet, siens.as_ref()) {
        (Some(me), Some(v)) => {
            let dite = v.objet().vue(me).map_err(Echec::from)?.and_then(|x| x.date_modification.filter(|d| !d.is_empty()));
            match dite {
                Some(d) => d,
                None => engagement::stock::premier_jour(me).map_err(Echec::from)?.iso(),
            }
        }
        _ => valeur_en_texte(debut_v),
    };
    let p = &paies[i];
    let g = m.globales()?;
    let deja = envois::etats::avenants_du_contrat(g.avenants.get(&(salarie.clone(), numero_parti.clone())), &parti);
    let ent = documents::avenant::ent_du_mois(ctx, &mois_parti)?;
    let modele = documents::avenant::modele_du_contrat(&ent.gabarits, &*p.engagement.nat().map_err(Echec::from)?)?;
    let paie_json = engagement::relire::ecrire(ctx, p)?;
    let numero_porte = Some(p.engagement.numero.as_deref().unwrap_or("None").to_string());
    let vals = documents::avenant::valeurs_de_la_paie(ctx, &ent, &paie_json, Some(&numero_porte))?;
    // L'ancien texte s'ecrit du modele avec les valeurs de la version partie, au mois compare (ou, pour un contrat sans
    // terme dont cette version n'avait pas encore ecrit ce mois, a son dernier mois d'avant : ses termes s'y reconduisent).
    let ancienne = match (siens.as_ref(), passee) {
        (Some(v), Some(x)) => {
            let o = v.objet();
            let mut mois = engagement::paies::mois_ecrits(o).map_err(Echec::from)?;
            mois.retain(|m| *m <= mois_compare);
            let mut trouvee = None;
            for m in mois.iter().rev() {
                if let Some(p) = engagement::etats::paie_ecrite_de_la_version(o, x, m).map_err(Echec::from)? {
                    trouvee = Some(p.paie.clone());
                    break;
                }
            }
            trouvee
        }
        _ => None,
    };
    let ancienne = ancienne
        .ok_or_else(|| {
            erreur(
                "ErreurOperation",
                format!("Le journal ne dit pas sur quelle version de l'engagement le contrat n° {} est parti : l'avenant ne peut pas citer son ancien texte.", numero_parti),
            )
        })?;
    let avant = documents::avenant::valeurs_de_la_paie(ctx, &ent, &ancienne, Some(&numero_porte))?;
    let modifs = documents::avenant::modifications(&modele, &avant, &vals)?;
    let mut signes: Vec<i64> = Vec::new();
    for a in deja.iter().filter(|a| a.get("signe").is_some_and(vrai)) {
        signes.push(a.get("numero").and_then(Value::as_i64).ok_or_else(|| erreur("TypeError", "un numero d'avenant n'est pas un entier"))?);
    }
    let sal = &p.salarie;
    let modifications: Vec<Value> = modifs
        .iter()
        .map(|x| json!({"numero": x.numero, "article": x.article, "titre_dit": format!("Article {} — {}", x.numero, x.article), "avant": x.avant, "apres": x.apres, "blocs": x.blocs}))
        .collect();
    Ok(json!({
        "salarie": salarie_v,
        "debut": debut_v,
        "fin": p.engagement.fin.map(|f| f.iso()),
        "nom": sal.nomme().unwrap_or_else(|| (&valeur_en_texte(salarie_v)).to_string()),
        "spectacle": p.titre()?,
        // CE QUE LE COURRIEL DIT DU CONTRAT : son nom (`contrat_ecrit.nom` de sa nature), son spectacle s'il en a un, sa
        // periode (« a partir du » pour un contrat sans terme).
        "nom_du_contrat": p.engagement.nat().map_err(Echec::from)?.nom_du_contrat(),
        "pour": if vrai(&p.spectacle) { format!(" pour {}", p.titre()?) } else { String::new() },
        "periode": match p.engagement.fin { Some(f) => format!("du {} au {}", p.engagement.debut.jj_mm_aaaa(), f.jj_mm_aaaa()), None => format!("à partir du {}", p.engagement.debut.jj_mm_aaaa()) },
        "mois": parti.mois,
        "contrat": parti.numero,
        "date_contrat": parti.date(),
        "signe": parti.signe,
        "numero": signes.iter().max().copied().unwrap_or(0) + 1,
        "effet": effet,
        "modifications": modifications,
        "avenants": deja,
        "remplacable": debut > ctx.aujourdhui.iso(),
        // Sans boite courriel, l'avenant s'imprime avec les contrats (`contrats-imprimer`) : rien ne part d'ici.
        "boite_liee": crate::routes::ouverture::boite_liee(ctx)?,
        "destinataire": sal.email.dite().map_or(json!(""), |e| json!(e)),
    }))
}

/// L'AVENANT D'UN CONTRAT SIGNE HORS DU LOGICIEL (`engagement::signe_ailleurs`) : le contrat n'est pas parti d'ici, le
/// logiciel ne connait pas son texte. Ce qu'il disait est ce que disait l'engagement sur la version de son dernier avenant
/// parti, ou, sans avenant, son premier mois ; ce qui change, chaque terme date depuis (`engagement::versions::termes_changes`),
/// se dit en clair, sans article (`documents::avenant::changements_en_clair`). `None` : ce n'est pas un contrat signe
/// ailleurs (le contrat du mois de la charge, ou du mois present, qui porte ce numero).
fn avenant_d_un_contrat_signe_ailleurs(ctx: &Contexte, charge: &Value, numero: &str) -> R<Option<Value>> {
    let salarie = valeur_en_texte(cle(charge, "salarie")?);
    let mois = charge.get("mois").filter(|m| vrai(m)).map(valeur_en_texte).unwrap_or_else(|| ctx.aujourdhui.mois_civil().to_string());
    if numero.is_empty() || !socle::mois::Mois::lire(&mois).is_ok() {
        return Ok(None);
    }
    let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs)?;
    let (_data, paies) = documents::ecrire_documents::paies_aux_numeros_declares(ctx, &mois, None, &departs)?;
    let Some(i) = etats::paie_du_contrat(&paies, &salarie, numero)? else { return Ok(None) };
    let p = &paies[i];
    if !engagement::signe_ailleurs(&ctx.racine, &p.engagement).map_err(Echec::from)? {
        return Ok(None);
    }
    let debut = p.engagement.debut.iso();
    let siens = engagement::etats::du_mois(ctx, &mois, &departs)?.into_iter().find_map(|e| {
        let v = e.valide()?.clone();
        let b = &*v.objet().brut;
        (engagement::cle_de_l_engagement(ctx, b).as_deref() == Some(salarie.as_str()) && b.get("numero").map(valeur_en_texte).as_deref() == Some(numero)).then_some(v)
    });
    let Some(v) = siens else { return Ok(None) };
    let presente = v.objet().brut.get("valide").and_then(|x| x.get("saisie")).cloned().unwrap_or(Value::Null);
    let convenue = match engagement::etats::version_au_depart(ctx, &v, &departs, engagement::departs::Envoi::Contrat) {
        Some(x) => x.get("saisie").cloned().unwrap_or(Value::Null),
        None => {
            let mut s = presente.clone();
            if let Some(o) = s.as_object_mut() {
                o.remove(engagement::stock::DATES);
            }
            s
        }
    };
    let nat = p.engagement.nat().map_err(Echec::from)?;
    let changes = engagement::versions::termes_changes(&convenue, &presente);
    let modifications = documents::avenant::changements_en_clair(ctx, &nat, &changes);
    let effet = changes.first().map_or_else(|| debut.clone(), |c| c.jour.iso());
    let g = Moteur::nouveau(ctx).globales()?;
    let deja: Vec<Value> = g.avenants.get(&(salarie.clone(), numero.to_string())).and_then(Value::as_array).cloned().unwrap_or_default();
    let signes = deja.iter().filter(|a| a.get("signe").is_some_and(vrai)).filter_map(|a| a.get("numero").and_then(Value::as_i64)).max().unwrap_or(0);
    let sal = &p.salarie;
    Ok(Some(json!({
        "salarie": salarie,
        "debut": debut,
        "fin": p.engagement.fin.map(|f| f.iso()),
        "nom": sal.nomme().unwrap_or_else(|| (&salarie).to_string()),
        "spectacle": p.titre()?,
        "nom_du_contrat": nat.nom_du_contrat(),
        "pour": if vrai(&p.spectacle) { format!(" pour {}", p.titre()?) } else { String::new() },
        "periode": match p.engagement.fin { Some(f) => format!("du {} au {}", p.engagement.debut.jj_mm_aaaa(), f.jj_mm_aaaa()), None => format!("à partir du {}", p.engagement.debut.jj_mm_aaaa()) },
        "mois": mois,
        "contrat": numero,
        "date_contrat": debut,
        "signe": Value::Null,
        "ailleurs": true,
        "contrat_signe": crate::compagnie::signature::contrat_signe_dit(ctx, &salarie, &debut),
        "numero": signes + 1,
        "effet": effet,
        "modifications": modifications,
        "avenants": deja,
        "remplacable": false,
        "boite_liee": crate::routes::ouverture::boite_liee(ctx)?,
        "destinataire": sal.email.dite().map_or(json!(""), |e| json!(e)),
    })))
}

/// Le fichier d'une piece partie existe-t-il, dans
/// `sorties/<mois>/` ou son `envoi/` ? Un NOM, jamais un chemin.
fn piece_existe(racine: &Path, mois: &Value, fichier: &Value) -> R<bool> {
    if !vrai(mois) || !vrai(fichier) {
        return Ok(false);
    }
    let nom = valeur_en_texte(fichier);
    if ["/", "\\", ".."].iter().any(|x| nom.contains(x)) {
        return Ok(false);
    }
    let Value::String(f) = fichier else {
        return Err(erreur("TypeError", format!("'fichier' : {}", socle::fichiers::attendu("un texte", nom_type(fichier)))));
    };
    let base = racine.join("sorties").join(valeur_en_texte(mois));
    Ok([base.join("envoi").join(f), base.join(f)].iter().any(|c| c.is_file()))
}

fn piece(racine: &Path, quoi: &str, quand: &Value, mois: &Value, fichier: &Value) -> R<Value> {
    Ok(json!({"quoi": quoi, "quand": quand, "quand_dit": super::quand(&valeur_en_texte(&json!(quand))), "mois": mois, "fichier": fichier, "existe": piece_existe(racine, mois, fichier)?}))
}

/// La ligne `c` (une embauche d'une DPAE, un contrat parti, un contrat revenu) est-elle celle du contrat n° `numero` (de ce
/// salarie, quand il est dit) ?
fn est_ce_contrat(c: &Value, salarie: &Value, numero: &Value) -> R<bool> {
    Ok(vrai(numero) && (salarie.is_null() || valeurs_egales(get(c, "salarie")?, salarie)) && valeurs_egales(getd(c, "numero", &Value::Null)?, numero))
}

/// `pieces_parties` : les FICHIERS partis pour le contrat n° `numero` -- la DPAE deposee, le
/// contrat envoye a la signature, le contrat revenu signe et la preuve de la
/// signature. Lu au journal ; rien ne part. La phrase dit chaque piece et son jour, comme la boite de l'ecran.
fn pieces_parties(ctx: &Contexte, charge: &Value) -> R<Value> {
    let numero = &charge.get("numero").filter(|n| vrai(n)).map(|n| json!(valeur_en_texte(n))).ok_or_else(|| {
        erreur("KeyError", "Quel contrat ? Donnez son numéro (« numero »), tel que signature-apercu ou etat-par-engagement le rend. Rien n'est écrit.")
    })?;
    let salarie = &charge.get("salarie").filter(|s| vrai(s)).cloned().unwrap_or(Value::Null);
    let mut qui: Option<Value> = salarie.as_str().map(|_| salarie.clone());
    let racine = ctx.racine.as_path();
    let m = Moteur::nouveau(ctx);
    let lignes = m.lignes()?;

    let mut dpae = Vec::new();
    for l in etats::filtrer(&lignes, Some(DPAE), None, true)? {
        if !vrai(get(l, "accepte")?) || !vrai(get(l, "fichier")?) {
            continue;
        }
        let embauches = items(getd(ou(get(l, "detail")?, objet_vide()), "embauches", &etats::LISTE_VIDE)?)?;
        let mut trouve = false;
        for e in &embauches {
            if est_ce_contrat(e, salarie, numero)? {
                qui.get_or_insert_with(|| e["salarie"].clone());
                trouve = true;
                break;
            }
        }
        if trouve {
            dpae.push(piece(racine, "DPAE déposée", get(l, "quand")?, get(l, "mois")?, get(l, "fichier")?)?);
        }
    }

    let mut contrat: Vec<Value> = Vec::new();
    for l in etats::filtrer(&lignes, Some(SIGNATURE), None, false)? {
        if !vrai(get(l, "accepte")?) {
            continue;
        }
        let parti = if l.pointer("/detail/main_propre").is_some_and(vrai) { "imprimé" } else { "envoyé à la signature" };
        for c in items(getd(ou(get(l, "detail")?, objet_vide()), "contrats", &etats::LISTE_VIDE)?)? {
            if !(est_ce_contrat(&c, salarie, numero)? && vrai(get(&c, "fichier")?)) {
                continue;
            }
            qui.get_or_insert_with(|| c["salarie"].clone());
            let quoi = if est(get(&c, "acte")?, "avenant") {
                format!("Avenant n° {} {}", valeur_en_texte(get(&c, "avenant")?), parti)
            } else if est(get(&c, "acte")?, "remplace") {
                format!("Contrat qui annule et remplace le n° {}, {}", valeur_en_texte(get(&c, "remplace")?), parti)
            } else {
                format!("Contrat {}", parti)
            };
            let mois = ou(get(&c, "mois")?, get(l, "mois")?);
            contrat.push(piece(racine, &quoi, get(l, "quand")?, mois, get(&c, "fichier")?)?);
        }
    }
    // CE QUI EN EST REVENU, sur la ligne de chaque envoi.
    for l in etats::filtrer(&lignes, Some(SIGNATURE), None, false)? {
        if !vrai(get(l, "accepte")?) {
            continue;
        }
        for d in items(getd(l, "retours", &etats::LISTE_VIDE)?)? {
            if !est_ce_contrat(&d, salarie, numero)? {
                continue;
            }
            if vrai(get(&d, "fichier_signe")?) {
                let quoi = if est(get(&d, "acte")?, "avenant") {
                    format!("Avenant n° {} signé, contresigné", valeur_en_texte(get(&d, "avenant")?))
                } else {
                    "Contrat signé, contresigné".to_string()
                };
                contrat.push(piece(racine, &quoi, get(&d, "quand")?, get(l, "mois")?, get(&d, "fichier_signe")?)?);
            }
            if vrai(get(&d, "preuve")?) {
                contrat.push(piece(racine, "Réponse « Je signe » du salarié (preuve)", get(&d, "quand")?, get(l, "mois")?, get(&d, "preuve")?)?);
            }
        }
    }
    contrat.sort_by_key(|x| if vrai(&x["quand"]) { valeur_en_texte(&x["quand"]) } else { String::new() });
    let de = match &qui {
        Some(cle) => format!(" de {}", super::salarie(ctx, cle).map(|s| super::nom_a_l_ecran(&s)).unwrap_or_else(|_| valeur_en_texte(cle))),
        None => String::new(),
    };
    let dites: Vec<String> = dpae.iter().chain(contrat.iter()).map(|p| {
        let quoi = valeur_en_texte(&p["quoi"]);
        let quoi = if quoi.starts_with("DPAE") { quoi } else { quoi[..1].to_lowercase() + &quoi[1..] };
        if vrai(&p["quand"]) { format!("{} le {}", quoi, super::quand(&valeur_en_texte(&p["quand"]))) } else { quoi }
    }).collect();
    let phrase = if dites.is_empty() {
        format!("Contrat {}{} : rien n'est parti.", valeur_en_texte(numero), de)
    } else {
        format!("Contrat {}{} : {}.", valeur_en_texte(numero), de, dites.join(" ; "))
    };
    Ok(json!({"phrase": phrase, "dpae": dpae, "contrat": contrat}))
}

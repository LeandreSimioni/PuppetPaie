//! Les routes des contrats : ce que le contrat a signer et l'avenant montrent, les envoyer a la signature ou les
//! imprimer, les relancer, les noter revenus signes, et les pieces parties d'un engagement.

use crate::compagnie::signature::jj;
use std::collections::HashMap;
use std::path::Path;

use serde_json::{json, Map, Value};

use crate::compagnie::depot_acte::{verifier_preuve, Preuve};
use crate::routes::catalogue::{Acte, Champ, Fiche, Refus, ILLISIBLE, MANQUE, MOIS, OPERATION, SALARIE};
use crate::routes::documents::{charge_texte, client_courriel, cle, liste_de, nul, preuve_de, refus, requis, un_nom_commence_et_finit, vrai_ou, Ouvrir};
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



const CONTRATS: Champ = Champ { nom: "contrats", sens: "les contrats choisis : [{salarie, debut, numero}]", avance: false };
const DEBUT: Champ = Champ { nom: "debut", sens: "le premier jour de l'engagement, AAAA-MM-JJ", avance: false };
const NUMERO: Champ = Champ { nom: "numero", sens: "le numéro du contrat, quand deux commencent le même jour", avance: true };

/// Les routes de ce module, et leurs fiches.
pub const ROUTES: &[Fiche] = &[
    Fiche {
        nom: "signature-apercu",
        geste: "Montrer les contrats du mois : à envoyer, en attente de signature, en avenant.",
        charge: &[MOIS],
        exemple: r#"{"mois":"2026-11"}"#,
        rend: "contrats (chacun : son état, et ses gestes recu et relancer, route et charge), boite_liee",
        refus: &[MANQUE, ILLISIBLE],
        acte: Acte::Lit,
        avance: false,
        phrase: 0,
    },
    Fiche {
        nom: "demander-signature",
        geste: "Faire signer par courriel les contrats et les avenants cochés.",
        charge: &[
            MOIS,
            CONTRATS,
            Champ { nom: "avenants", sens: "les avenants cochés : [{salarie, debut, numero, mois, effet}]", avance: false },
            Champ { nom: "salaries", sens: "tous les contrats de ces salariés", avance: true },
            Champ { nom: "relance", sens: "vrai : relancer ceux qui attendent leur signature", avance: true },
            Champ { nom: "remplace", sens: "vrai : un contrat refait qui annule et remplace le précédent", avance: true },
        ],
        exemple: r#"{"mois":"2026-11","contrats":[{"salarie":"FULLER","debut":"2026-11-27"}]}"#,
        rend: "phrase, partis, contrats (les fichiers partis), avertissements (ceux sans courriel)",
        refus: &[
            Refus { classe: "PreuveInsuffisante", corriger: "le mot de passe du logiciel, tapé par un humain au moment de l'envoi ; rien n'est parti" },
            Refus { classe: "BoiteAbsente", corriger: "poser la boîte courriel (Configuration, Mots de passe), ou imprimer" },
            OPERATION,
            MANQUE,
        ],
        acte: Acte::Engage,
        avance: false,
        phrase: 600,
    },
    Fiche {
        nom: "pieces-parties",
        geste: "Lister ce qui est parti pour un contrat : DPAE, contrat envoyé, signé, preuve.",
        charge: &[SALARIE, NUMERO],
        exemple: r#"{"salarie":"FULLER"}"#,
        rend: "dpae, contrat, signe, preuve : les fichiers partis, lus au journal",
        refus: &[MANQUE],
        acte: Acte::Lit,
        avance: false,
        phrase: 0,
    },
    Fiche { acte: Acte::Lit, ..Fiche::a_ecrire("avenant-apercu") },
    Fiche {
        nom: "contrats-imprimer",
        geste: "Réunir en un PDF les contrats et avenants cochés, à remettre en main propre.",
        charge: &[MOIS, CONTRATS, Champ { nom: "avenants", sens: "les avenants cochés : [{salarie, debut, numero, mois, effet}]", avance: false }],
        exemple: r#"{"mois":"2026-11","contrats":[{"salarie":"SVOBODA","debut":"2026-11-02"}]}"#,
        rend: "impression (le PDF), contrats",
        refus: &[OPERATION, MANQUE],
        acte: Acte::Ecrit,
        avance: false,
        phrase: 0,
    },
    Fiche {
        nom: "contrat-recu",
        geste: "Noter qu'un contrat (ou un avenant) est revenu signé, ou retirer la note.",
        charge: &[
            MOIS,
            SALARIE,
            DEBUT,
            NUMERO,
            Champ { nom: "recu", sens: "faux : retirer la note", avance: true },
            Champ { nom: "avenant", sens: "le numéro de l'avenant revenu signé", avance: true },
        ],
        exemple: r#"{"mois":"2026-11","salarie":"FULLER","debut":"2026-11-27"}"#,
        rend: "phrase, recu (quand), annuler (la route et la charge qui défont la note)",
        refus: &[OPERATION, MANQUE],
        acte: Acte::Ecrit,
        avance: false,
        phrase: 200,
    },
];

/// Une route de ce module.
pub fn route(_o: &HashMap<String, String>, ctx: &Contexte, nom: &str, charge: &Value) -> R<Value> {
    match nom {
        "signature-apercu" => signature_apercu(ctx, charge),
        "avenant-apercu" => avenant_apercu(ctx, charge),
        "pieces-parties" => pieces_parties(ctx, charge),
        "demander-signature" => demander_signature(ctx, charge, &mut || client_courriel(ctx)),
        "contrats-imprimer" => contrats_imprimer_par(ctx, charge, "interface"),
        "contrat-recu" => contrat_recu(ctx, charge),
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
    let cle_de = |p: &engagement::calcul::Paie| p.salarie.get("_cle").map(valeur_en_texte).unwrap_or_default();
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
    let numero_note = p.engagement.numero.clone().map_or(Value::Null, Value::String);
    let nom = socle::texte::salarie_nomme_ou(Some(&p.salarie), &salarie);
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
        "annuler": {"route": "contrat-recu", "charge": {"mois": mois, "salarie": salarie, "debut": debut, "numero": numero_note, "avenant": avenant, "recu": !recu}},
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
pub(crate) fn contrats_imprimer_par(ctx: &Contexte, charge: &Value, qui: &str) -> R<Value> {
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
    let quoi = [(n_contrats, "contrat(s)"), (n_avenants, "avenant(s)")].iter().filter(|(n, _)| *n > 0).map(|(n, m)| format!("{} {}", n, m)).collect::<Vec<_>>().join(" et ");
    let avertissements = r["avertissements"].as_array().into_iter().flatten().filter_map(Value::as_str).collect::<Vec<_>>().join(" ");
    let relancee = relance || (!des_contrats && avenants.iter().all(|a| a.get("relance").is_some_and(vrai)));
    let fait = if relancee { "relancé(s)" } else { "envoyé(s) à la signature" };
    r["phrase"] = json!(format!("{} {}.{}{}", quoi, fait, if avertissements.is_empty() { "" } else { " " }, avertissements));
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
        .map(|p| Ok((valeur_en_texte(get(&p.salarie, "_cle")?), p.engagement.debut.iso())))
        .collect::<R<_>>()?;
    let avenants = crate::routes::etapes::avenants_a_faire(ctx, &mois)?;

    let aujourdhui = ctx.aujourdhui.iso();
    let boite_liee = crate::routes::ouverture::boite_liee(ctx)?;
    let mut contrats = Vec::new();
    for (i, p) in paies.iter().enumerate() {
        // SEUL UN CONTRAT ECRIT SE FAIT SIGNER ICI (`engagement::contrat_a_signer`).
        if !engagement::contrat_a_signer(&ctx.racine, &p.engagement).map_err(Echec::from)? {
            continue;
        }
        let (cle_s, debut) = rangs[i].clone();
        let k = (cle_s.clone(), p.engagement.numero.clone().unwrap_or_default());
        let etat = crate::routes::etapes::etat_signature(
            &crate::routes::etapes::contrat_parti_le(&departs, &cle_s, p.engagement.numero.as_deref()),
            g.par.get(&k).unwrap_or(nul()),
            g.recus.get(&k).unwrap_or(nul()),
            g.recus_par.get(&k).unwrap_or(nul()),
            g.renvoyes.contains(&k),
            g.avenants.get(&k).unwrap_or(nul()),
            avenants.contains(&(cle_s.clone(), p.engagement.numero.clone().unwrap_or_default())),
        );
        let sal = &p.salarie;
        let numero = p.engagement.numero.clone().map_or(Value::Null, Value::String);
        let fin = p.engagement.fin.map_or("None".to_string(), |f| f.iso());
        let mut l = Map::new();
        l.insert("salarie".into(), json!(cle_s));
        l.insert("nom".into(), json!(socle::texte::salarie_nomme_ou(Some(sal), &cle_s)));
        l.insert("destinataire".into(), sal.get("email").cloned().unwrap_or_else(|| json!("")));
        l.insert("spectacle".into(), json!(p.titre()?));
        l.insert("debut".into(), json!(debut));
        l.insert("fin".into(), json!(fin));
        l.insert("numero".into(), numero);
        l.insert("demande_le".into(), etat["fait"].clone());
        // CE QUE LA BOITE EN FAIT : un contrat a envoyer (jamais parti, ou caduc) se coche, et part en avenant s'il est
        // deja parti, signe ou non (`en_avenant`) ; un contrat ou un avenant parti et pas revenu attend : il se note reçu signe
        // (`recu`), et seul celui parti par courriel se relance (`relancer`), quand la boite est posee. Les deux gestes
        // sont des routes et leurs charges : la page n'en compose aucun.
        let parti = l["demande_le"].clone();
        let (recu, caduc) = (vrai(&etat["recu"]), vrai(&etat["caduc"]));
        let avenant_en_attente = !caduc && vrai(&etat["avenant_attendu"]);
        let en_attente = !caduc && ((vrai(&parti) && !recu) || avenant_en_attente);
        let a_envoyer = caduc || (!recu && !vrai(&parti));
        let par = etat["par"].clone();
        let (ce, depuis) = if avenant_en_attente {
            (format!("avenant n° {} ", valeur_en_texte(&etat["avenant"]["numero"])), etat["avenant"]["quand"].clone())
        } else {
            (String::new(), parti.clone())
        };
        let jour = valeur_en_texte(&depuis);
        let jj_mm = if jour.len() >= 10 { format!("{}/{}", &jour[8..10], &jour[5..7]) } else { jour };
        let attente = format!("{}{} le {}", ce, if est(&par, "imprime") { "imprimé" } else { "envoyé" }, jj_mm);
        // Ce que la demande du mot de passe dit avant une relance.
        let pourquoi = format!("relancer la signature : {}, {}", valeur_en_texte(&l["nom"]), attente);
        let numero_charge = p.engagement.numero.clone().map_or(json!(""), Value::String);
        let (recu_g, relancer) = if en_attente {
            let avenant = if avenant_en_attente { etat["avenant"]["numero"].clone() } else { Value::Null };
            let recu_g = json!({"route": "contrat-recu", "charge": {"mois": mois, "salarie": cle_s, "debut": debut, "numero": numero_charge, "avenant": avenant, "recu": true}});
            let relancer = if !(est(&par, "courriel") && boite_liee) {
                Value::Null
            } else if avenant_en_attente {
                json!({"route": "demander-signature", "pourquoi": pourquoi, "charge": {"mois": mois, "avenants": [{"salarie": cle_s, "debut": debut, "numero": numero_charge, "mois": mois, "relance": true}]}})
            } else {
                json!({"route": "demander-signature", "pourquoi": pourquoi, "charge": {"mois": mois, "relance": true, "contrats": [{"salarie": cle_s, "debut": debut, "numero": numero_charge}]}})
            };
            (recu_g, relancer)
        } else {
            (Value::Null, Value::Null)
        };
        // Sa pastille : un contrat parti puis change part en avenant, signe ou non, commence ou non ; parti et pas revenu,
        // il attend sa signature. Toutes a faire : la compagnie le peut.
        let pourquoi_dit = etat["pourquoi"].as_str().filter(|t| !t.is_empty()).map(String::from);
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
        l.insert(
            "attente".into(),
            if en_attente { json!(attente) } else { Value::Null },
        );
        l.insert("recu".into(), recu_g);
        l.insert("relancer".into(), relancer);
        l.insert("recu_le".into(), etat["recu"].clone());
        l.insert("recu_par".into(), etat["recu_par"].clone());
        l.insert("caduc".into(), etat["caduc"].clone());
        l.insert("pourquoi".into(), etat["pourquoi"].clone());
        l.insert("avenant".into(), etat["avenant"].clone());
        l.insert("avenant_attendu".into(), etat["avenant_attendu"].clone());
        l.insert("commence".into(), json!(debut <= aujourdhui));
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
    let dossier = ctx.racine.join("sorties").join(&mois);
    let envoi = dossier.join("envoi");
    let produit = dossier.exists()
        && if envoi.exists() {
            un_nom_commence_et_finit(&dossier, "Contrats_", ".pdf") || un_nom_commence_et_finit(&envoi, "Contrat_", ".pdf")
        } else {
            un_nom_commence_et_finit(&dossier, "Contrats_", ".pdf")
        };
    let salaries = salaries_d_identite(ctx);
    let attendues = etats::signatures_attendues(&m.lignes()?, Some(&mois), &salaries)?;
    let n_attendues = attendues.as_array().map_or(0, Vec::len);
    Ok(json!({
        "mois": mois_v,
        "contrats": contrats,
        "boite_liee": boite_liee,
        "avertissements": avertissements,
        "attendues": n_attendues,
        "dossier": dossier.to_string_lossy(),
        "produit": produit,
    }))
}

/// Seuls les quatre champs d'identite que la demande affiche passent a `signatures_attendues` (`journal.signatures_attendues`).
pub(crate) fn salaries_d_identite(ctx: &Contexte) -> Value {
    let mut salaries = Map::new();
    for (k, v) in &ctx.salaries {
        if let Value::Object(f) = v {
            let garde: Map<String, Value> = ["prenom_affichage", "prenom", "nom_affichage", "nom"]
                .iter()
                .filter_map(|c| f.get(*c).map(|x| (c.to_string(), x.clone())))
                .collect();
            salaries.insert(k.clone(), Value::Object(garde));
        }
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
        let b = Value::Object(v.objet().brut.clone());
        (engagement::cle_de_l_engagement(ctx, &b).as_deref() == Some(salarie.as_str()) && b.get("numero").map(valeur_en_texte) == paies[i].engagement.numero).then_some(v)
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
                erreur("ErreurOperation", format!("Aucune paie de {} en {} pour le contrat n° {}.", salarie, engagement::libelle_mois(me), numero_parti))
            })?;
            (ps, j, me.clone())
        }
        None => (paies, i, mois_parti.clone()),
    };
    let effet = match (&mois_effet, siens.as_ref()) {
        (Some(me), Some(v)) => {
            let dite = v.objet().vue(me).map_err(Echec::from)?.and_then(|x| x.get("date_modification").filter(|d| vrai(d)).map(valeur_en_texte));
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
    let modele = documents::avenant::modele_du_contrat(&ent, &ctx.gabarits(), &*p.engagement.nat().map_err(Echec::from)?)?;
    let paie_json = engagement::relire::ecrire(ctx, p)?;
    let numero_porte = Some(p.engagement.numero.clone().unwrap_or_else(|| "None".to_string()));
    let vals = documents::avenant::valeurs_de_la_paie(ctx, &ent, &paie_json, Some(&numero_porte))?;
    // L'ancien texte s'ecrit du modele avec les valeurs de la version partie, au mois compare (ou, pour un contrat sans
    // terme dont cette version n'avait pas encore ecrit ce mois, a son dernier mois d'avant : ses termes s'y reconduisent).
    let ancienne = passee
        .and_then(|x| {
            let ps = x.get("paies")?.as_object()?;
            ps.get(&mois_compare).or_else(|| ps.iter().filter(|(k, _)| **k <= mois_compare).last().map(|(_, v)| v))?.get("paie").cloned()
        })
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
        .map(|x| json!({"numero": x.numero, "article": x.article, "avant": x.avant, "apres": x.apres, "blocs": x.blocs}))
        .collect();
    Ok(json!({
        "salarie": salarie_v,
        "debut": debut_v,
        "fin": p.engagement.fin.map_or("None".to_string(), |f| f.iso()),
        "nom": socle::texte::salarie_nomme_ou(Some(sal), &valeur_en_texte(salarie_v)),
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
        "fichier_parti": parti.fichier,
        "numero": signes.iter().max().copied().unwrap_or(0) + 1,
        "effet": effet,
        "modifications": modifications,
        "avenants": deja,
        "remplacable": debut > ctx.aujourdhui.iso(),
        // Sans boite courriel, l'avenant s'imprime avec les contrats (`contrats-imprimer`) : rien ne part d'ici.
        "boite_liee": crate::routes::ouverture::boite_liee(ctx)?,
        "destinataire": sal.get("email").cloned().unwrap_or_else(|| json!("")),
    }))
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
    Ok(json!({"quoi": quoi, "quand": quand, "mois": mois, "fichier": fichier, "existe": piece_existe(racine, mois, fichier)?}))
}

/// La ligne `c` (une embauche d'une DPAE, un contrat parti, un contrat revenu) est-elle celle du contrat n° `numero` de ce
/// salarie ?
fn est_ce_contrat(c: &Value, salarie: &Value, numero: &Value) -> R<bool> {
    Ok(vrai(numero) && valeurs_egales(get(c, "salarie")?, salarie) && valeurs_egales(getd(c, "numero", &Value::Null)?, numero))
}

/// `pieces_parties` : les FICHIERS partis pour le contrat n° `numero` d'un salarie -- la DPAE deposee, le
/// contrat envoye a la signature, le contrat revenu signe et la preuve de la
/// signature. Lu au journal ; rien ne part.
fn pieces_parties(ctx: &Contexte, charge: &Value) -> R<Value> {
    let cle = |k: &str| charge.get(k).ok_or_else(|| erreur("KeyError", socle::fichiers::absent(k)));
    let salarie = cle("salarie")?;
    let numero = &charge.get("numero").filter(|n| vrai(n)).map(|n| json!(valeur_en_texte(n))).unwrap_or(Value::Null);
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
    Ok(json!({"dpae": dpae, "contrat": contrat}))
}

// ------------------------------------------------------------------ la rupture n'est pas un avenant

/// Ce qu'une rupture ecrit dans la saisie d'un engagement : sa fin, son motif, son preavis et ses dates.
const DE_LA_RUPTURE: &[&str] =
    &["debut", "fin", "motif_rupture", "preavis", "notification_rupture", "signature_convention", "engagement_licenciement", "transaction", "numero", "lignes"];

/// UN CONTRAT ROMPU N'A PAS D'AVENANT. Code du travail L. 1243-1 (`atelier/reference/sources/code-du-travail/
/// articles-20261028.md`) : le CDD se rompt avant l'echeance de son terme (accord des parties, faute grave, force
/// majeure, inaptitude) ; la rupture met fin au contrat signe, elle ne le modifie pas, et rien ne se refait signer. Vrai
/// quand la saisie `presente` ne differe de la saisie `partie` (celle sur laquelle le contrat est parti) que par la rupture
/// qu'elle porte : son motif (`motif_rupture`, absent de la partie), la fin et le preavis qu'il appelle, et les jours de
/// travail d'apres la fin retires (une ligne garde ses jours jusqu'a la fin, et sa quantite ne croit pas).
pub(crate) fn seule_la_rupture(partie: &Value, presente: &Value) -> bool {
    let texte = |v: &Value, k: &str| v.get(k).filter(|x| vrai(x)).map(valeur_en_texte).unwrap_or_default();
    let fin = texte(presente, "fin");
    if texte(presente, "motif_rupture").is_empty() || !texte(partie, "motif_rupture").is_empty() || fin.len() != 10 {
        return false;
    }
    let reste = |v: &Value| -> Map<String, Value> {
        v.as_object().map(|o| o.iter().filter(|(k, _)| !DE_LA_RUPTURE.contains(&k.as_str())).map(|(k, x)| (k.clone(), x.clone())).collect()).unwrap_or_default()
    };
    if reste(partie) != reste(presente) {
        return false;
    }
    let dates = |l: &Value| -> Vec<String> { l.get("dates").and_then(Value::as_array).map(|a| a.iter().map(valeur_en_texte).collect()).unwrap_or_default() };
    let quantite = |l: &Value| l.get("quantite").and_then(Value::as_f64).unwrap_or(0.0);
    let sans = |l: &Value, k: &[&str]| -> Map<String, Value> {
        l.as_object().map(|o| o.iter().filter(|(c, _)| !k.contains(&c.as_str())).map(|(c, x)| (c.clone(), x.clone())).collect()).unwrap_or_default()
    };
    let lignes = |v: &Value| v.get("lignes").and_then(Value::as_array).cloned().unwrap_or_default();
    // Les lignes de la partie, arretees a la fin : celles qui n'ont plus aucun jour disparaissent.
    let mut attendues: Vec<(Value, bool)> = Vec::new();
    for l in lignes(partie) {
        let jours = dates(&l);
        let gardes: Vec<String> = jours.iter().filter(|j| **j <= fin).cloned().collect();
        if !jours.is_empty() && gardes.is_empty() {
            continue;
        }
        let mut l = l.clone();
        let coupee = gardes.len() != jours.len();
        if coupee {
            l["dates"] = json!(gardes);
        }
        attendues.push((l, coupee));
    }
    let presentes = lignes(presente);
    presentes.len() == attendues.len()
        && presentes.iter().zip(&attendues).all(|(p, (a, coupee))| {
            if *coupee {
                sans(p, &["quantite"]) == sans(a, &["quantite"]) && quantite(p) <= quantite(a)
            } else {
                p == a
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Code du travail L. 1243-1 (`atelier/reference/sources/code-du-travail/articles-20261028.md`) : rompre un CDD avant
    /// son terme ne modifie pas le contrat signe, rien ne se refait signer ; changer en meme temps ce qu'il paie, si.
    #[test]
    fn un_contrat_rompu_n_a_pas_d_avenant() {
        let jours = |n: usize| json!(["2026-05-04", "2026-05-05", "2026-05-06", "2026-05-07", "2026-05-08"][..n].to_vec());
        let ligne = |n: usize, taux: i64| json!({"rubrique": "cachet", "quantite": n, "taux": taux, "lieu": "Salle", "dates": jours(n)});
        let partie = json!({"salarie": "X", "spectacle": "s", "lignes": [ligne(5, 150)], "numero": "X1.26.01"});
        let rompu = |taux: i64| json!({"salarie": "X", "spectacle": "s", "debut": "2026-05-04", "fin": "2026-05-06", "motif_rupture": "084",
            "preavis": {"type": "90"}, "lignes": [ligne(3, taux)], "numero": "X1.26.01"});
        assert!(seule_la_rupture(&partie, &rompu(150)));
        assert!(!seule_la_rupture(&partie, &rompu(180)), "un taux change avec la rupture se refait signer");
        let raccourci = json!({"salarie": "X", "spectacle": "s", "lignes": [ligne(3, 150)], "numero": "X1.26.01"});
        assert!(!seule_la_rupture(&partie, &raccourci), "sans motif de rupture, des jours retires sont un avenant");
    }
}

//! POUR DEMARRER : ce qui manque a une compagnie neuve, d'un coup.
//!
//! Une compagnie neuve ne decouvre pas ce qui lui manque un refus apres l'autre (le taux AT/MP, l'effectif, la medecine
//! du travail, le compte Urssaf...) : tout ce qui manque se dit d'un coup, ce que chaque manque BLOQUE, OU il se saisit, et ou la
//! compagnie TROUVE la valeur (l'aide du champ, `champs-base.json`).
//!
//! Les regles ne sont pas recopiees : le bareme de l'annee se demande au contexte (`Contexte::bareme` : le taux AT/MP,
//! l'effectif, la premiere embauche), l'employeur de la DPAE a la DPAE (`dpae::employeur_manques`). La table `FICHE` ne fait
//! que nommer les champs que la DSN et le bulletin lisent sans valeur par defaut (`dsn.rs`, en-tete S10/S20 et bloc 40 ;
//! `bulletin.rs`) : un champ qu'on y ajoute sans qu'un document le lise serait un faux manque.

use serde_json::{json, Value};

use droit::contexte::Contexte;
use socle::fichiers::vrai;
use socle::echec::Echec;

type R<T> = Result<T, Echec>;

pub const ROUTE: &str = "pour-demarrer";

const ENTREPRISE: &str = "Configuration › Entreprise";

/// (chemin dans la fiche entreprise, ce que son absence bloque). Le SIRET avant le SIREN et le NIC : ils s'en deduisent
/// (`moteur::routes::fiches::deriver`) et se disent sur sa ligne. La qualite du contact (S20.G00.07.004) n'y est pas : le moteur
/// l'ecrit seul (`dsn::type_du_contact`).
const FICHE: [(&str, &str); 22] = [
    ("raison_sociale", "la DSN"),
    ("siret", "la DSN et la DPAE"),
    ("siren", "la DSN"),
    ("nic", "la DSN"),
    ("naf", "la DSN et la DPAE"),
    ("adresse", "la DSN et la DPAE"),
    ("code_postal", "la DSN et la DPAE"),
    ("commune", "la DSN et la DPAE"),
    ("idcc", "toute paie"),
    ("urssaf_compte", "les bulletins"),
    ("signataire.nom", "la DSN et les contrats"),
    ("signataire.prenom", "la DSN et les contrats"),
    ("declarant_dsn.civilite", "la DSN"),
    ("declarant_dsn.nom_prenom", "la DSN"),
    ("declarant_dsn.tel", "la DSN"),
    ("declarant_dsn.email", "la DSN"),
    // Seulement quand la medecine du travail est interne (code MT01) : le champ est alors exige (`exige_si`), et la DSN
    // de 2027 l'ecrit en S20.G00.07 de type 17 (`dsn::contact_sante_travail`).
    ("contact_sante_travail.nom_prenom", "la DSN"),
    ("contact_sante_travail.tel", "la DSN"),
    ("contact_sante_travail.email", "la DSN"),
    ("banque.iban", "la DSN (le paiement des cotisations)"),
    ("banque.bic", "la DSN (le paiement des cotisations)"),
    // Le SEUL numero Conges Spectacles demande : la DSN le declare precede de 97 (`conges_spectacles::code_dsn`, fiche de
    // specificites d'Audiens, l. 80-81), les certificats l'impriment dans leur case COMPTE.
    ("congesspectacles_adherent", "la DSN des intermittents et les certificats Congés Spectacles"),
];

/// LE CHAMP QUE L'ECRAN MONTRE pour une valeur de la fiche : chaque chose demandee se remplit la ou elle
/// envoie, sans champ cache. Une valeur que l'on deduit d'une autre se demande par celle-la : le SIREN et le NIC par
/// le SIRET (`moteur::routes::fiches::deriver`), la forme declaree (`raison_sociale`, `adresse`, `commune`) par sa forme lisible
/// (`derive_de` de `champs-base.json`).
pub fn a_l_ecran(racine: &str) -> String {
    match racine {
        "siren" | "nic" => return "siret".to_string(),
        _ => {}
    }
    crate::champs::fiche("entreprise")
        .ok()
        .and_then(|v| v.into_iter().find(|c| c.cle == racine))
        .and_then(|c| c.derive_de.filter(|d| !d.is_empty()))
        .unwrap_or_else(|| racine.to_string())
}

/// Le nom en clair d'une case d'un champ a plusieurs cases.
fn en_clair(sous: &str) -> &str {
    match sous {
        "civilite" => "civilité",
        "nom_prenom" => "nom et prénom",
        "nom" => "nom",
        "prenom" => "prénom",
        "tel" => "téléphone",
        "email" => "courriel",
        "qualite" => "qualité",
        "iban" => "IBAN",
        "bic" => "BIC",
        autre => autre,
    }
}

fn valeur<'a>(ent: &'a Value, chemin: &str) -> Option<&'a Value> {
    chemin.split('.').try_fold(ent, |v, k| v.get(k))
}

/// Le libelle et l'aide du champ (`champs-base.json`), le premier segment du chemin.
fn champ(cle: &str) -> (String, String) {
    let racine = cle.split('.').next().unwrap_or(cle);
    let c = crate::champs::fiche("entreprise").ok().and_then(|v| v.into_iter().find(|c| c.cle == racine));
    match c {
        Some(c) => {
            let precision = match cle.split_once('.') {
                Some((_, sous)) => format!(" ({})", en_clair(sous)),
                None => String::new(),
            };
            (format!("{}{}", c.libelle, precision), c.aide)
        }
        None => (cle.to_string(), String::new()),
    }
}

fn manque(cle: &str, libelle: String, bloque: &str, ou: &str, champ: Option<&str>, trouver: String) -> Value {
    json!({ "cle": cle, "libelle": libelle, "bloque": bloque, "ou": ou, "champ": champ, "trouver": trouver })
}

/// LA LICENCE D'ENTREPRENEUR DE SPECTACLES est-elle exigee de la compagnie `ent` ? « Est entrepreneur de spectacles
/// vivants toute personne qui exerce une activite d'exploitation de lieux de spectacles, de production ou de diffusion de
/// spectacles » (L7122-2) ; il exerce « sous reserve de [...] declarer son activite », declaration qui « donne lieu a la
/// delivrance d'un recepisse [...] valant licence » (L7122-3) ; celui dont ce n'est pas l'activite principale declare ses
/// artistes et techniciens par le guichet unique, pas en DSN (L7122-22, L7122-23 ; `atelier/reference/sources/
/// code-du-travail/articles-20261023.md`, « L'entrepreneur de spectacles vivants et sa declaration »). La norme DSN
/// l'ecrit dans chaque contrat (S21.G00.40.049, `norme-dsn/texte-2027/bloc-S21.G00.40.txt` l. 1418-1426).
///
/// Deux faits la rendent exigee, lus ici pour « Pour demarrer » et pour la DSN :
/// - la convention dont c'est le champ (3090, 1285 ; et la compagnie qui n'a pas dit la sienne) : `exige_si` du champ
///   `licence_spectacle` (`champs-base.json`) ;
/// - dans une convention ou la case se montre (`si` : 1790, 3252), la compagnie engage pour un spectacle
///   (`spectacle` : un spectacle configure pour « Pour demarrer », un contrat de spectacle pour la DSN). Un lieu en 1790
///   sans spectacle n'a rien a dire ; l'audiovisuel ne voit pas la case.
pub fn licence_exigee(ent: &Value, spectacle: bool) -> bool {
    crate::champs::champ("entreprise", "licence_spectacle")
        .ok()
        .flatten()
        .is_some_and(|c| c.exige(ent) || (spectacle && crate::champs::condition_remplie(&c.si, ent)))
}

/// La compagnie emploie-t-elle des intermittents ? Un spectacle actif le dit : un CDDU se rattache a un spectacle.
fn a_des_spectacles(ctx: &Contexte) -> bool {
    std::fs::read(ctx.racine.join("config").join("spectacles.json"))
        .ok()
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        .and_then(|v| v.as_object().map(|o| o.iter().any(|(k, s)| !k.starts_with('_') && s.get("actif") != Some(&json!(false)))))
        .unwrap_or(false)
}

/// Une suspension saisie (un imprevu du bloc 65) dont le motif n'est pas remunere. Un magasin illisible ne dit rien ici :
/// la paie le refusera elle-meme.
fn une_suspension_non_remuneree(ctx: &Contexte) -> bool {
    engagement::imprevus::tous(&ctx.racine).unwrap_or_default().iter().any(|imp| {
        engagement::imprevus::est_une_suspension(imp)
            && imp
                .get("motif")
                .map(socle::date::en_texte)
                .and_then(|m| engagement::imprevus::motif_de_suspension(&m).ok().flatten())
                .is_some_and(|m| !m.get("remunere").is_some_and(vrai))
    })
}

/// Le champ `cle` de la fiche entreprise est-il exige de la compagnie `ent` (`exige_si` de `champs-base.json`) ?
fn champ_exige(ent: &Value, cle: &str) -> bool {
    crate::champs::champ("entreprise", cle).ok().flatten().is_some_and(|c| c.exige(ent))
}

/// Tout ce qui manque a la compagnie pour ses premiers actes. Vide : rien ne manque. Une ligne dit ce qu'elle bloque
/// (`bloque`), ou, quand rien ne se refuse sans elle, ce qui se passe sinon (`sinon`).
pub fn manques(ctx: &Contexte) -> Vec<Value> {
    let ent = &ctx.entreprise;
    let mut out: Vec<Value> = Vec::new();
    let intermittents = a_des_spectacles(ctx);
    // Un champ a plusieurs cases (le declarant : civilite, nom, telephone, courriel) fait UNE ligne, qui nomme ce qui manque.
    let mut vus: Vec<String> = Vec::new();
    for (cle, bloque) in FICHE {
        if cle == documents::conges_spectacles::CLE_ADHERENT && !intermittents {
            continue;
        }
        if cle.starts_with("contact_sante_travail.") && !champ_exige(ent, "contact_sante_travail") {
            continue;
        }
        if valeur(ent, cle).is_some_and(vrai) {
            continue;
        }
        // Ce qui manque se dit par le champ que l'ecran montre (`a_l_ecran`) : SIREN, NIC et SIRET font une ligne.
        let racine = a_l_ecran(cle.split('.').next().unwrap_or(cle));
        let sous_cle = cle.split_once('.').map(|(_, s)| s);
        let cle_ecran = match sous_cle {
            Some(s) => format!("{}.{}", racine, s),
            None => racine.clone(),
        };
        let sous = sous_cle.map(|s| en_clair(s).to_string());
        if let Some(i) = vus.iter().position(|r| *r == racine) {
            if let (Some(sous), Some(l)) = (sous, out[i]["libelle"].as_str().map(str::to_string)) {
                let l = match l.strip_suffix(')') {
                    Some(debut) => format!("{}, {})", debut, sous),
                    None => format!("{} ({})", l, sous),
                };
                out[i]["libelle"] = json!(l);
            }
            continue;
        }
        let (libelle, aide) = champ(&cle_ecran);
        out.push(manque(&cle_ecran, libelle, bloque, ENTREPRISE, Some(&racine), aide));
        vus.push(racine);
    }
    // La licence d'entrepreneur de spectacles : la DSN l'ecrit dans chaque contrat (S21.G00.40.049) et refuse sans elle
    // QUAND ELLE EST EXIGEE de la compagnie (`exige_si` de `champs-base.json`, lu par `dsn::licence_du_contrat` aussi) :
    // en 1790 et en 3252 la case se montre sans etre exigee, dans l'audiovisuel elle ne se montre pas.
    // Elle peut etre datee (une par periode) : le contexte la resout au jour.
    match ctx.licence(&ctx.aujourdhui) {
        Ok(l) if !l.trim().is_empty() => {}
        _ if !licence_exigee(ent, intermittents) => {}
        autre => {
            let (libelle, aide) = champ("licence_spectacle");
            let trouver = match autre {
                Err(m) => m,
                Ok(_) => aide,
            };
            out.push(manque("licence_spectacle", libelle, "la DSN", ENTREPRISE, Some("licence_spectacle"), trouver));
        }
    }
    // L'effectif, puis le reste du bareme de l'annee : le contexte refuse lui-meme, avec la phrase qui dit ou trouver ce
    // qui manque. L'effectif a part : le bareme s'arrete au premier refus (le taux AT/MP), et tout doit sortir d'un coup.
    let an = ctx.aujourdhui.an;
    if let Err(m) = ctx.effectif().and_then(|e| e.atteint(11, an).map(|_| ())) {
        let (libelle, _) = champ("premiere_embauche");
        out.push(manque("premiere_embauche", libelle, "toute paie", ENTREPRISE, Some("premiere_embauche"), m));
    }
    // EN ALSACE-MOSELLE, le taux pose a la creation (celui de l'annexe 1) ne vaut pas : l'annexe 3 en fixe d'autres (art. 4
    // de l'arrete, `atelier/reference/sources/tarification-atmp/`), et le taux se saisit (`droit::bareme::taux_de_l_arrete`).
    if en_alsace_moselle(ctx) && pose_a_la_creation(ctx) {
        out.push(manque(
            "taux_atmp",
            format!("Le taux AT/MP {}", an),
            "toute paie",
            ENTREPRISE,
            Some("taux_atmp"),
            "L'établissement est en Alsace-Moselle : ses taux AT/MP sont ceux de l'annexe 3 de l'arrêté de l'année, pas celui posé à la création. Saisissez le code et le taux de votre notification (compte AT/MP sur net-entreprises).".into(),
        ));
    }
    if let Err(m) = ctx.bareme(&ctx.aujourdhui) {
        if m.contains("AT/MP") {
            out.push(manque("taux_atmp", format!("Le taux AT/MP {}", an), "toute paie", ENTREPRISE, Some("taux_atmp"), m));
        } else if !m.contains("Effectif") {
            out.push(manque("bareme", format!("Le barème {}", an), "toute paie", ENTREPRISE, None, m));
        }
    }
    // L'employeur de la DPAE : la DPAE elle-meme dit ce qui lui manque, champ par champ (`dpae::employeur_manques`). Un
    // SIRET deja demande plus haut ne se redemande pas ; il dit seulement qu'il bloque aussi la DPAE.
    match crate::dpae::employeur_manques(ent, &ctx.gabarits()) {
        Ok(manques_dpae) => {
            for (cle, phrase) in manques_dpae {
                if out.iter().any(|m| m["champ"] == json!(cle)) {
                    continue;
                }
                let (libelle, aide) = champ(cle);
                let trouver = if aide.is_empty() { phrase } else { aide };
                out.push(manque(cle, libelle, "la DPAE", ENTREPRISE, Some(cle), trouver));
            }
        }
        Err(m) => out.push(manque("dpae", "L'employeur de la DPAE".into(), "la DPAE", ENTREPRISE, None, m)),
    }
    // La sante d'un permanent suspendu sans indemnisation tout un mois : hors de la 3090, aucun texte ne la dit, et le mois
    // refuse tant que la fiche ne repond pas (`calcul::sante_de_la_suspension`). Elle ne se demande que si une suspension
    // non remuneree est saisie : un champ n'apparait que parce qu'un fait l'exige.
    let idcc = ent.get("idcc").map(socle::date::en_texte).unwrap_or_default();
    if idcc != "3090" && !valeur(ent, droit::bareme::SANTE_SUSPENSION).is_some_and(vrai) && une_suspension_non_remuneree(ctx) {
        let cle = droit::bareme::SANTE_SUSPENSION;
        let (libelle, aide) = champ(cle);
        out.push(manque(cle, libelle, "le mois d'un permanent suspendu sans indemnisation tout le mois", ENTREPRISE, Some(cle), aide));
    }
    // LA TVA ET LA TAXE SUR LES SALAIRES : la DSN de decembre declare chaque annee si la compagnie la doit (bloc
    // S21.G00.44) et ne se fait pas sans reponse (`taxe_sur_les_salaires::regime_taxe_sur_les_salaires`). Une compagnie qui
    // demarre en novembre la rencontrait au premier mois de decembre, sans que rien ne l'ait dit d'avance.
    if let Err(m) = documents::taxe_sur_les_salaires::regime_taxe_sur_les_salaires(ent, None).decision {
        let cle = documents::taxe_sur_les_salaires::CHAMP_TAXE_SUR_LES_SALAIRES;
        let (libelle, _) = champ(cle);
        out.push(manque(cle, libelle, "la DSN de décembre", ENTREPRISE, Some(cle), m));
    }
    // LES CONTRATS DES PERMANENTS : c'est tout ce que la compagnie a a saisir des organismes (le reste, les textes le
    // donnent : `droit::organismes`). Sans eux, le mois d'un permanent se refuse (`dsn::affiliations`).
    if let (true, Ok(orgs)) = (ctx.emploie_des_permanents(), ctx.organismes()) {
        if !orgs.pointer("/affiliations_par_nature/permanent/references").is_some_and(vrai) {
            out.push(manque(
                "organismes_permanents",
                "Les contrats de prévoyance et de santé des permanents".into(),
                "le mois d'un salarié au mois",
                "Configuration › Organismes et contrats",
                None,
                "La compagnie emploie des salariés au mois : leurs contrats de prévoyance et de santé se disent avec ceux des intermittents. La référence du contrat se lit sur le certificat d'adhésion de l'assureur.".into(),
            ));
        }
    }
    // LES CONGES PAYES ACQUIS AVANT PUPPETPAIE : un permanent dont le contrat commence avant le premier mois ou PuppetPaie
    // le paie a acquis des jours ailleurs (L3141-3 du code du travail, `atelier/reference/sources/code-du-travail/
    // articles-20260919.md`) ; un solde ne se devine pas (regle 1). Sa fiche les dit (`conges_reprise`, que
    // `engagement::conges::reprise_conges` lit). Rien ne se refuse sans eux : la ligne dit ce qui se passe sinon (`sinon`),
    // et non ce qu'elle bloque. C'est ici seulement que la question se pose, contrat repris d'un import compris.
    for (salarie, nom, depuis) in conges_a_reprendre(ctx) {
        out.push(json!({
            "cle": format!("conges_reprise.{}", salarie),
            "libelle": format!("Les congés payés de {} acquis avant le {}", nom, socle::date::Jour::lire(&depuis).map(|j| j.jj_mm_aaaa()).unwrap_or(depuis.clone())),
            "sinon": "son compteur de congés payés part de zéro",
            "ou": format!("la fiche de {}, « Congés payés à la reprise »", nom),
            "champ": "conges_reprise",
            "salarie": salarie,
            "trouver": format!("Son contrat a commencé avant que PuppetPaie le paie : recopiez de son dernier bulletin de votre ancien logiciel les jours acquis et pris, avec le {} pour date de reprise.", socle::date::Jour::lire(&depuis).map(|j| j.jj_mm_aaaa()).unwrap_or(depuis.clone())),
        }));
    }
    out
}

/// Les permanents dont un contrat commence avant le premier mois ou PuppetPaie le paie, et dont la fiche ne dit pas les
/// conges a la reprise (`conges_reprise.date`) : `(salarie, nom, premier jour paye ici)`. Le premier mois paye ici est,
/// pour un contrat repris d'un ancien logiciel (`engagement::importe`), celui qui suit le dernier mois repris (le contrat
/// court encore ce mois-la, et sa nature est dite) ; sinon le premier `engagements/AAAA-MM.json` qui porte le salarie.
/// Un intermittent n'en est pas (ses conges sont ceux des Conges Spectacles) ; un contrat ne d'ici n'a rien d'avant.
fn conges_a_reprendre(ctx: &Contexte) -> Vec<(String, String, String)> {
    let t = |e: &Value, k: &str| e.get(k).filter(|v| vrai(v)).map(socle::date::en_texte).unwrap_or_default();
    let mensuel = |nature: &str| droit::natures::nature(Some(nature)).is_ok_and(|n| n.mensuelle());
    // (salarie, debut du contrat, premier jour paye ici)
    let mut contrats: Vec<(String, String, String)> = Vec::new();
    let mut vus: Vec<String> = Vec::new();
    let repris = engagement::importe::contrats_lus(&ctx.racine).unwrap_or_default();
    if let Some(dernier) = repris.iter().map(|e| t(e, "fin")).filter(|f| f.len() >= 7).map(|f| f[..7].to_string()).max() {
        let suite = socle::date::dernier_du_mois(&dernier).and_then(|j| j.plus_jours(1)).map(|j| j.iso()).unwrap_or_default();
        for e in &repris {
            let (salarie, debut, fin) = (t(e, "salarie"), t(e, "contrat_debut"), t(e, "contrat_fin"));
            if salarie.is_empty() || debut.is_empty() || suite.is_empty() || !mensuel(&t(e, "nature")) {
                continue;
            }
            vus.push(salarie.clone());
            if debut < suite && (fin.is_empty() || fin >= suite) && !contrats.iter().any(|c| c.0 == salarie) {
                contrats.push((salarie, debut, suite.clone()));
            }
        }
    }
    let mois: Vec<String> = std::fs::read_dir(ctx.racine.join("engagements"))
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter_map(|n| n.strip_suffix(".json").filter(|m| m.len() == 7 && m.as_bytes()[4] == b'-').map(str::to_string))
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    for m in mois {
        let Ok(f) = socle::fichiers::lire_json(&ctx.racine.join("engagements").join(format!("{}.json", m))) else { continue };
        for e in f.get("engagements").and_then(Value::as_array).into_iter().flatten() {
            let (salarie, debut) = (t(e, "salarie"), t(e, "debut"));
            if salarie.is_empty() || debut.is_empty() || vus.contains(&salarie) || !mensuel(&t(e, "nature")) {
                continue;
            }
            vus.push(salarie.clone());
            contrats.push((salarie, debut, format!("{}-01", m)));
        }
    }
    let mut out = Vec::new();
    for (salarie, debut, premier) in contrats {
        let fiche = ctx.salaries.get(&salarie);
        let dit = fiche.and_then(|f| f.get("conges_reprise")).and_then(|c| c.get("date")).is_some_and(vrai);
        if debut >= premier || dit {
            continue;
        }
        let nom = fiche
            .map(|f| {
                let v = |k: &str| f.get(k).filter(|x| vrai(x)).map(socle::date::en_texte).unwrap_or_default();
                let prenom = if v("prenom_affichage").is_empty() { v("prenom") } else { v("prenom_affichage") };
                format!("{} {}", v("nom"), prenom).trim().to_string()
            })
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| salarie.clone());
        out.push((salarie, nom, premier));
    }
    out
}

/// CE QUE LE MOTEUR A POSE DE LUI-MEME A LA CREATION, et qui se dit avec ce qui manque : le code risque et le taux AT/MP de
/// l'annee (`moteur::routes::atmp::poser_a_la_creation`), tant que personne ne les a changes. Le libelle est celui de l'arrete
/// de l'annee (`droit::bareme::risques_atmp`, `atelier/reference/sources/tarification-atmp/`).
pub fn poses(ctx: &Contexte) -> Vec<Value> {
    let an = ctx.aujourdhui.an;
    let Some(Ok(t)) = ctx.taux_atmp_de(an) else { return Vec::new() };
    let reporte_de = t.get("_reporte_de").and_then(Value::as_i64);
    if !(t.get("_pose_a_la_creation").is_some_and(vrai) || reporte_de.is_some()) || en_alsace_moselle(ctx) {
        return Vec::new();
    }
    let code = t.get("code_risque").map(socle::date::en_texte).unwrap_or_default();
    let socle = ctx.socle_livre(an).ok().flatten().unwrap_or(Value::Null);
    let Some(r) = droit::bareme::risques_atmp(&socle).into_iter().find(|r| r.code == code) else { return Vec::new() };
    let taux = t.get("valeur").and_then(Value::as_f64).map(|v| format!("{:.2} %", v).replace('.', ",")).unwrap_or_default();
    vec![json!({
        "cle": "taux_atmp",
        "libelle": format!("Accidents du travail : code risque {}, taux {} en {}", r.code_arrete.as_deref().unwrap_or(&r.code), taux, an),
        "texte": match reporte_de {
            Some(de) => format!("« {} » Le code risque est celui que la compagnie a dit pour {} ; son taux de {} est celui de l'arrêté de tarification, le même pour toutes les compagnies qui y sont classées. Votre notification AT/MP (compte AT/MP sur net-entreprises) dit le vôtre ; s'il est autre, changez-le dans Configuration › Entreprise, « Avancé ».", r.libelle, de, an),
            None => format!("« {} » Le logiciel l'a posé pour vous : c'est le risque du spectacle vivant, et son taux est le même pour toutes les compagnies qui y sont classées. Votre notification AT/MP (compte AT/MP sur net-entreprises) dit le vôtre ; s'il est autre, changez-le dans Configuration › Entreprise, « Avancé ».", r.libelle),
        },
        "ou": ENTREPRISE,
        "champ": "taux_atmp",
    })]
}

/// `pour-demarrer` : `{"manques": [...], "poses": [...]}`.
pub fn route(ctx: &Contexte) -> R<Value> {
    Ok(json!({ "manques": manques(ctx), "poses": poses(ctx) }))
}

/// Ce qui suit le refus d'un acte quand la compagnie n'a pas fini de demarrer : toute la liste, d'un coup, sans ce que
/// le refus `deja_dit` dit deja (un manque dont l'explication est dans le refus ne se redit pas).
pub fn a_la_suite(ctx: &Contexte, deja_dit: &str) -> Option<String> {
    let lignes: Vec<String> = manques(ctx)
        .iter()
        .filter(|x| x.get("bloque").is_some())
        .filter(|x| !x["trouver"].as_str().map(str::trim).is_some_and(|t| !t.is_empty() && deja_dit.contains(t)))
        .map(|x| format!("  - {} : bloque {} ({})", x["libelle"].as_str().unwrap_or(""), x["bloque"].as_str().unwrap_or(""), x["ou"].as_str().unwrap_or("")))
        .collect();
    if lignes.is_empty() {
        return None;
    }
    Some(format!("\n\nPour démarrer, il manque encore :\n{}", lignes.join("\n")))
}

/// L'etablissement de la fiche est-il en Alsace-Moselle (`droit::bareme::en_alsace_moselle`) ?
fn en_alsace_moselle(ctx: &Contexte) -> bool {
    ctx.entreprise.get("code_postal").map(socle::date::en_texte).is_some_and(|c| droit::bareme::en_alsace_moselle(&c))
}

/// Le taux AT/MP de l'annee est-il encore celui que le moteur a pose a la creation ?
fn pose_a_la_creation(ctx: &Contexte) -> bool {
    matches!(ctx.taux_atmp_de(ctx.aujourdhui.an), Some(Ok(t)) if t.get("_pose_a_la_creation").is_some_and(vrai))
}

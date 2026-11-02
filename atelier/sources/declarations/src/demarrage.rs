
//! POUR DEMARRER : ce qu'une compagnie neuve peut remplir d'avance, d'un coup.
//!
//! Une compagnie neuve ne decouvre pas ce qui lui manque un refus apres l'autre (le taux AT/MP, l'effectif, la medecine
//! du travail, le compte Urssaf...) : tout se dit d'un coup, ce que chaque case servira, OU elle vit, et ou la compagnie
//! TROUVE la valeur (l'aide du champ, `champs-base.json`). Une case y est exigee pour un geste ou utile pour un geste ; le refus reste a l'acte.
//!
//! Les regles ne sont pas recopiees : le bareme de l'annee se demande au contexte (`Contexte::bareme` : le taux AT/MP,
//! l'effectif, la premiere embauche), ce que chaque acte exige des fiches a `champs-base.json` (`exige_par`), que l'acte
//! lit aussi (`champs::refus_des_manques`) : un champ qu'on y marque sans qu'un acte le lise serait un faux manque.

use serde_json::{json, Value};

use droit::contexte::Contexte;
use socle::fichiers::vrai;
use socle::echec::Echec;

type R<T> = Result<T, Echec>;

pub const ROUTE: &str = "pour-demarrer";

/// Dans la fiche d'un spectacle : le titre que l'import a lu dans les bulletins de l'ancien logiciel, propose, et ou il
/// l'a lu (la ligne « Spectacle », ou la case « Analytique »). « Pour démarrer » le propose dans la case du titre.
pub const TITRE_PROPOSE: &str = "_titre_propose";
pub const TITRE_LU_DANS: &str = "_titre_lu_dans";
/// Le titre d'un spectacle que l'import n'a pas ecrit.
const TITRE_A_COMPLETER: &str = "A COMPLETER";

const ENTREPRISE: &str = "Configuration › Entreprise";

fn valeur<'a>(ent: &'a Value, chemin: &str) -> Option<&'a Value> {
    chemin.split('.').try_fold(ent, |v, k| v.get(k))
}

/// Le libelle et l'aide du champ `cle` de la fiche entreprise (`champs-base.json`).
fn champ(cle: &str) -> (String, String) {
    match crate::champs::champ("entreprise", cle).ok().flatten() {
        Some(c) => (c.libelle, c.aide),
        None => (cle.to_string(), String::new()),
    }
}

fn manque(cle: &str, libelle: String, bloque: &str, ou: &str, champ: Option<&str>, trouver: String) -> Value {
    let mut l = json!({ "cle": cle, "libelle": libelle, "bloque": bloque, "ou": ou, "champ": champ, "case": champ, "trouver": trouver });
    // UNE STRUCTURE DIT SES CASES (leur nom, leur forme, leurs valeurs), d'apres la table qui les decrit.
    if let Some(c) = champ.and_then(|k| crate::champs::champ("entreprise", k).ok().flatten()).filter(|c| !c.champs.is_empty()) {
        l["cases"] = json!(crate::champs::cases_dites(&c));
    }
    l
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

/// Les spectacles (leur cle) que porte un engagement actif, ni importe, dont la nature declare le numero d'objet.
fn spectacles_declares(ctx: &Contexte) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for o in engagement::stock::objets(&ctx.racine).unwrap_or_default() {
        if o.est_non_actif() || o.importe_seul() {
            continue;
        }
        let nature = o.brut.get("nature").and_then(Value::as_str);
        let Some(sp) = o.brut.get("spectacle").and_then(Value::as_str).filter(|s| !s.trim().is_empty()) else { continue };
        let Some(cle) = ctx.spectacle(sp).ok().and_then(|f| f.get("_cle").and_then(Value::as_str).map(str::to_string)) else { continue };
        if droit::natures::nature(nature).is_ok_and(|n| n.ecrit("S21.G00.40.050")) && !out.contains(&cle) {
            out.push(cle);
        }
    }
    out
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

/// Tout ce qui manque a la compagnie pour ses premiers actes. Vide : rien ne manque. Une ligne dit ce qu'elle bloque
/// (`bloque`), ou, quand rien ne se refuse sans elle, ce qu'elle sert (`utile`) et ce qui se passe sinon (`sinon`).
pub fn manques(ctx: &Contexte) -> Vec<Value> {
    let ent = &ctx.entreprise;
    let mut out: Vec<Value> = Vec::new();
    let intermittents = a_des_spectacles(ctx);
    // CE QUE LES ACTES EXIGENT DES FICHES (`exige_par` de `champs-base.json`, que chaque acte lit aussi :
    // `champs::refus_des_manques`) : l'entreprise, puis chaque salarie actif ; ce qui ne bloque rien dit ce qui se passe
    // sans lui (`sinon`).
    // Une ligne dit la case a remplir (`case`, son chemin dans la fiche, que l'ecran dessine) ; celle d'un salarie dit
    // aussi qui (`qui`), sous le nom duquel elle se range.
    let ligne = |m: crate::champs::Manquant, ou: String, salarie: Option<(&str, &str)>| {
        let racine = m.ecran.split('.').next().unwrap_or(&m.ecran).to_string();
        let mut l = json!({ "cle": m.ecran, "case": m.ecran, "libelle": m.libelle, "ou": ou, "champ": racine, "trouver": m.aide });
        if m.actes.is_empty() {
            l["utile"] = json!(crate::champs::actes_en_clair(&m.utile));
            l["sinon"] = json!(m.sinon);
        } else {
            l["bloque"] = json!(crate::champs::actes_en_clair(&m.actes));
        }
        // UNE STRUCTURE DIT SES CASES (leur nom, leur forme, leurs valeurs), d'apres la table qui les decrit.
        if !m.cases.is_empty() {
            l["cases"] = json!(m.cases);
        }
        if let Some((s, nom)) = salarie {
            l["salarie"] = json!(s);
            l["qui"] = json!(nom);
            l["cle"] = json!(format!("{}.{}", m.ecran, s));
        }
        l
    };
    for m in crate::champs::manquants("entreprise", ent).unwrap_or_default() {
        if m.cle == documents::conges_spectacles::CLE_ADHERENT && !intermittents {
            continue;
        }
        out.push(ligne(m, ENTREPRISE.to_string(), None));
    }
    for (cle, s) in ctx.salaries.iter().filter(|(_, s)| s.get("actif") != Some(&json!(false))) {
        let nom = socle::texte::salarie_nomme_ou(Some(s), cle);
        for m in crate::champs::manquants("salarie", s).unwrap_or_default() {
            out.push(ligne(m, format!("Configuration › Salariés, la fiche de {}", nom), Some((cle, &nom))));
        }
    }
    // Un spectacle ne se dit que si un engagement dont la nature declare le numero d'objet (S21.G00.40.050, `champs` de
    // sa nature dans `contrats-base.json`) le porte : rien de remplissable s'il n'y a pas a le remplir.
    for cle in spectacles_declares(ctx) {
        let Some(sp) = ctx.spectacles.get(&cle) else { continue };
        let titre = sp.get("titre").filter(|t| vrai(t)).map(socle::date::en_texte).unwrap_or_else(|| cle.clone());
        for m in crate::champs::manquants("spectacle", sp).unwrap_or_default() {
            let mut l = ligne(m, format!("Configuration › Spectacles, la fiche de {}", titre), None);
            l["spectacle"] = json!(cle);
            l["qui"] = json!(titre);
            l["cle"] = json!(format!("{}.{}", l["case"].as_str().unwrap_or(""), cle));
            out.push(l);
        }
    }
    // LE TITRE D'UN SPECTACLE QUE L'IMPORT N'A PAS ECRIT (« A COMPLETER » sur une fiche active : le contrat et le bulletin
    // l'imprimeraient tel quel) : la case se demande, avec le titre que les bulletins de l'ancien logiciel portent
    // (`TITRE_PROPOSE`), a valider ou a corriger, et le numero d'objet auquel il correspond.
    for (cle, sp) in ctx.spectacles.iter().filter(|(k, s)| !k.starts_with('_') && s.get("actif") != Some(&json!(false))) {
        let lu = |k: &str| sp.get(k).filter(|t| vrai(t)).map(socle::date::en_texte).map(|t| t.trim().to_string()).filter(|t| !t.is_empty());
        if !lu("titre").is_none_or(|t| t.eq_ignore_ascii_case(TITRE_A_COMPLETER)) {
            continue;
        }
        let objet = lu("numero_objet").unwrap_or_else(|| cle.clone());
        let propose = lu(TITRE_PROPOSE);
        let trouver = match &propose {
            Some(t) => format!(
                "Proposé : « {} », lu dans la case « {} » des bulletins de l'ancien logiciel pour le numéro d'objet {}. Validez-le ou corrigez-le.",
                t,
                lu(TITRE_LU_DANS).unwrap_or_else(|| "Spectacle".into()),
                objet
            ),
            None => format!("Aucune pièce reprise ne porte le titre du numéro d'objet {} : celui que vous avez déclaré à France Travail.", objet),
        };
        let mut l = json!({
            "cle": format!("titre.{}", cle), "case": "titre", "libelle": "Titre", "champ": "titre",
            "ou": format!("Configuration › Spectacles, la fiche du numéro d'objet {}", objet),
            "trouver": trouver, "utile": "les contrats et les bulletins", "sinon": "ils impriment « A COMPLETER »",
            "spectacle": cle, "qui": format!("Numéro d'objet {}", objet),
        });
        if let Some(t) = propose {
            l["propose"] = json!(t);
        }
        out.push(l);
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
    // L'EFFECTIF, puis le reste du bareme de l'annee : le contexte refuse lui-meme, avec la phrase qui dit ou trouver ce
    // qui manque. L'effectif a part : le bareme s'arrete au premier refus (le taux AT/MP), et tout doit sortir d'un coup.
    // La date de la premiere embauche n'est demandee que quand l'effectif ne se connait pas autrement : un seuil d'effectif
    // n'est franchi qu'apres « cinq années civiles consécutives » (CSS, art. L. 130-1, II,
    // `atelier/reference/sources/code-de-la-securite-sociale/articles-20261011.md` l. 296), et des taux de chaque paie en
    // dependent (onze et cinquante salaries) : elle est exigee pour l'effectif de la paie.
    // Une compagnie plus ancienne dit l'effectif de chaque annee qui manque (`Effectif::a_dire`), tel que l'Urssaf l'a
    // calcule (L. 130-1, I) : celle qui arrive d'un autre logiciel n'a pas ses comptes rendus de nature 118.
    let an = ctx.aujourdhui.an;
    if let Ok(e) = ctx.effectif() {
        let manquent = e.a_dire(an);
        if !manquent.is_empty() && e.premiere_embauche.is_none() {
            let (libelle, _) = champ("premiere_embauche");
            let m = e.atteint(11, an).err().unwrap_or_else(|| format!("L'effectif de {} n'est pas connu.", an - 1));
            out.push(manque("premiere_embauche", libelle, "l'effectif de toute paie (seuils de 11 et 50 salariés)", ENTREPRISE, Some("premiere_embauche"), m));
        } else if !manquent.is_empty() {
            let annees = manquent.iter().map(i32::to_string).collect::<Vec<_>>().join(", ");
            out.push(manque(
                "effectif",
                format!("L'effectif de {}", annees),
                "l'effectif de toute paie (seuils de 11 et 50 salariés)",
                ENTREPRISE,
                None,
                "Ces années sont d'avant PuppetPaie : leur effectif se dit tel que l'Urssaf l'a calculé (l'effectif annuel moyen de l'année, sur le compte rendu « Effectifs » de nature 118 reçu au début de l'année suivante), sous « Avancé », une année à la fois. Le compte rendu 118 de l'Urssaf le remplace à son arrivée.".to_string(),
            ));
        }
    }
    // LE CSE, la ou la convention en fait dependre une contribution (`par_representation` : le FNAS de la CCNEAC,
    // article III.3.1) et seulement quand l'effectif de la paie compte au moins onze salaries : en dessous, la question
    // ne se pose pas (III.3.1.b, `droit::bareme::representation_du_personnel`).
    let cle = "representation_personnel";
    let par_representation = |b: &Value| b["contributions_conventionnelles"].as_array().is_some_and(|l| l.iter().any(|c| c.get("par_representation").is_some()));
    // Sans CSE dit, c'est l'effectif de la paie (celui de l'annee d'avant, L. 130-1, I) qui decide (III.3.1.b) : inconnu,
    // c'est lui que la contribution demande, pas le CSE.
    if !valeur(ent, cle).is_some_and(vrai) && ctx.bareme(&ctx.aujourdhui).is_ok_and(|b| par_representation(&b)) {
        match ctx.effectif().ok().and_then(|e| e.valeur(an)) {
            Some(e) if e >= 11.0 => {
                let (libelle, aide) = champ(cle);
                out.push(manque(cle, libelle, "la contribution au FNAS de chaque paie", ENTREPRISE, Some(cle), aide));
            }
            Some(_) => {}
            None => out.push(manque(
                "effectif",
                format!("L'effectif de {}", an - 1),
                "la contribution au FNAS de chaque paie",
                ENTREPRISE,
                None,
                format!("Sans comité social et économique dit, la CCNEAC fixe la contribution au FNAS par l'effectif (article III.3.1) : celui de {} tel que l'Urssaf l'a calculé (compte rendu « Effectifs » de nature 118), sous « Avancé », ou le comité dans Configuration › Entreprise.", an - 1),
            )),
        }
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
        out.push(conges_a_dire(&salarie, &nom, &depuis));
    }
    out
}

/// LA LIGNE QUI DEMANDE LES CONGES ACQUIS AVANT PUPPETPAIE d'un salarie au mois (`salarie`, `nom` tel que l'ecran le dit),
/// paye ici depuis `depuis` (`AAAA-MM-JJ`) : la case `conges_reprise` de sa fiche, ou elle mene. « Pour démarrer » la dit,
/// et la carte d'un contrat commence avant le logiciel la montre avant sa validation (`moteur::routes::mois`).
pub fn conges_a_dire(salarie: &str, nom: &str, depuis: &str) -> Value {
    let le = socle::date::Jour::lire(depuis).map(|j| j.jj_mm_aaaa()).unwrap_or(depuis.to_string());
    json!({
        "cle": format!("conges_reprise.{}", salarie),
        "libelle": format!("Ses congés payés acquis avant le {}", le),
        "utile": "le compteur de ses congés payés",
        "sinon": "il part de zéro",
        "ou": format!("la fiche de {}, « Congés payés à la reprise »", nom),
        "champ": "conges_reprise",
        "case": "conges_reprise",
        "qui": nom,
        "salarie": salarie,
        "trouver": format!("Son contrat a commencé avant que PuppetPaie le paie : recopiez de son dernier bulletin de votre ancien logiciel les jours acquis et pris, avec le {} pour date de reprise.", le),
    })
}

/// La reprise des conges de ce salarie est-elle deja dite sur sa fiche (`conges_reprise.date`) ?
pub fn conges_repris(ctx: &Contexte, salarie: &str) -> bool {
    ctx.salaries.get(salarie).and_then(|f| f.get("conges_reprise")).and_then(|c| c.get("date")).is_some_and(vrai)
}

/// Les permanents dont un contrat commence avant le premier mois ou PuppetPaie le paie, et dont la fiche ne dit pas les
/// conges a la reprise (`conges_reprise.date`) : `(salarie, nom, premier jour paye ici)`. Le premier mois paye ici est
/// celui du fichier de l'engagement ; un contrat repris d'un ancien logiciel qui continue ici y est des le premier mois
/// neuf (`engagement::importe`), valide ou non. Un intermittent n'en est pas (ses conges sont ceux des Conges
/// Spectacles) ; un contrat ne d'ici n'a rien d'avant.
fn conges_a_reprendre(ctx: &Contexte) -> Vec<(String, String, String)> {
    let t = |e: &Value, k: &str| e.get(k).filter(|v| vrai(v)).map(socle::date::en_texte).unwrap_or_default();
    let mensuel = |nature: &str| droit::natures::nature(Some(nature)).is_ok_and(|n| n.mensuelle());
    // (salarie, debut du contrat, premier jour paye ici)
    let mut contrats: Vec<(String, String, String)> = Vec::new();
    let mut vus: Vec<String> = Vec::new();
    // Les engagements valides et actifs, chacun dans le mois de son fichier (`engagement::stock::objets`) : un brouillon
    // n'engage rien, un contrat repris qui continue ici si.
    for o in engagement::stock::objets(&ctx.racine).unwrap_or_default() {
        if !(o.est_valide() || o.est_importe()) || o.est_non_actif() || o.importe_seul() {
            continue;
        }
        let e = &*o.brut;
        let (salarie, debut) = (t(&e, "salarie"), t(&e, "debut"));
        if salarie.is_empty() || debut.is_empty() || vus.contains(&salarie) || !mensuel(&t(&e, "nature")) {
            continue;
        }
        vus.push(salarie.clone());
        if engagement::stock::commence_avant_le_logiciel(&ctx.racine, &debut, o.premier_mois()).unwrap_or(false) {
            contrats.push((salarie, debut, format!("{}-01", o.premier_mois())));
        }
    }
    let mut out = Vec::new();
    for (salarie, debut, premier) in contrats {
        let fiche = ctx.salaries.get(&salarie);
        if debut >= premier || conges_repris(ctx, &salarie) {
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
/// l'annee (`moteur::routes::atmp::poser_a_la_creation`), tant que personne ne les a changes, ou ceux que la notification
/// de la CARSAT a poses, avec sa date (`moteur::routes::atmp::poser_notifie`). Le libelle est celui de l'arrete
/// de l'annee (`droit::bareme::risques_atmp`, `atelier/reference/sources/tarification-atmp/`).
pub fn poses(ctx: &Contexte) -> Vec<Value> {
    let an = ctx.aujourdhui.an;
    let Some(Ok(t)) = ctx.taux_atmp_de(an) else { return Vec::new() };
    let reporte_de = t.get("_reporte_de").and_then(Value::as_i64);
    let notification = t.get("_notification").filter(|n| vrai(n));
    let a_la_creation = t.get("_pose_a_la_creation").is_some_and(vrai);
    if notification.is_none() && (!(a_la_creation || reporte_de.is_some()) || en_alsace_moselle(ctx)) {
        return Vec::new();
    }
    let code = t.get("code_risque").map(socle::date::en_texte).unwrap_or_default();
    let socle = ctx.socle_livre(an).ok().flatten().unwrap_or(Value::Null);
    let risque = droit::bareme::risques_atmp(&socle).into_iter().find(|r| r.code == code);
    let taux = t
        .get("valeur")
        .and_then(|v| droit::bareme::taux_atmp_au(v, &ctx.aujourdhui))
        .map(|v| format!("{:.2} %", v.round_dp(2)).replace('.', ","))
        .unwrap_or_default();
    let nom = risque.as_ref().map(|r| r.code_arrete.clone().unwrap_or_else(|| r.code.clone())).unwrap_or_else(|| code.clone());
    let libelle = format!("Accidents du travail : code risque {}, taux {} en {}", nom, taux, an);
    // Le taux que la notification de la CARSAT a pose (`moteur::routes::atmp::poser_notifie`) : d'ou il vient.
    if let Some(n) = notification {
        let date = |k: &str| n.get(k).and_then(Value::as_str).and_then(|d| socle::date::Jour::lire(d).ok()).map(|j| j.jj_mm_aaaa());
        return vec![json!({
            "cle": "taux_atmp",
            "libelle": libelle,
            "texte": format!(
                "{}Le code risque et le taux sont ceux de la notification de la CARSAT{}, en vigueur depuis le {} : le logiciel les a lus dans les retours de la DSN et posés de lui-même. Rien à recopier.",
                risque.as_ref().map(|r| format!("« {} » ", r.libelle)).unwrap_or_default(),
                date("calcul").or_else(|| date("le")).map(|d| format!(" du {}", d)).unwrap_or_default(),
                date("effet").unwrap_or_default()
            ),
            "ou": ENTREPRISE,
            "champ": "taux_atmp",
        })];
    }
    let Some(r) = risque else { return Vec::new() };
    vec![json!({
        "cle": "taux_atmp",
        "libelle": libelle,
        "texte": match reporte_de {
            Some(de) => format!("« {} » Le code risque est celui que la compagnie a dit pour {} ; son taux de {} est celui de l'arrêté de tarification, le même pour toutes les compagnies qui y sont classées. Votre notification AT/MP (compte AT/MP sur net-entreprises) dit le vôtre ; s'il est autre, changez-le dans Configuration › Entreprise, « Avancé ».", r.libelle, de, an),
            None => format!("« {} » Le logiciel l'a posé pour vous : c'est le risque du spectacle vivant, et son taux est le même pour toutes les compagnies qui y sont classées. Votre notification AT/MP (compte AT/MP sur net-entreprises) dit le vôtre ; s'il est autre, changez-le dans Configuration › Entreprise, « Avancé ».", r.libelle),
        },
        "ou": ENTREPRISE,
        "champ": "taux_atmp",
    })]
}

/// `pour-demarrer` : `{"phrase", "titre", "note", "manques": [...], "poses": [...]}`. `proposes` : les propositions qui
/// ne se tirent pas des fiches (les acces aux organismes, que le moteur lit dans le coffre), a la suite ; `pose` : ce qui
/// se dit en information a leur place. Une case est EXIGEE POUR un geste (sans elle, il ne se fait pas : `bloque`) ou
/// UTILE POUR un geste (sans elle, il se fait autrement : `utile`, `sinon`) ; chaque ligne le dit (`pour`), et sa phrase
/// entiere dit aussi ou elle vit et de qui (`phrase`). Le refus reste a l'acte (`champs::refus_des_manques`).
pub fn route(ctx: &Contexte, proposes: Vec<Value>, pose: Option<Value>) -> R<Value> {
    let mut lignes = manques(ctx);
    lignes.extend(proposes);
    for l in lignes.iter_mut() {
        let t = |k: &str| l.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        let pour = match l.get("bloque").and_then(Value::as_str) {
            Some(b) => format!("case exigée pour {}", b),
            None => format!("utile pour {} ; sinon, {}", t("utile"), t("sinon")),
        };
        // Une valeur que le moteur propose (`propose`) se dit dans la phrase : la ligne de commande la lit comme l'ecran.
        let propose = l.get("propose").and_then(Value::as_str).map(|p| format!(" Proposé : « {} ».", p)).unwrap_or_default();
        l["phrase"] = json!(format!("{} › {} : {}.{}", t("ou"), t("libelle"), pour, propose));
        let mut c = pour.chars();
        l["pour"] = json!(format!("{}{}.", c.next().map(|p| p.to_uppercase().collect::<String>()).unwrap_or_default(), c.as_str()));
    }
    let n = lignes.iter().filter(|l| l.get("bloque").is_some()).count();
    let (titre, note) = match (n, lignes.is_empty()) {
        (0, true) => ("Pour démarrer : rien à remplir".to_string(), ""),
        (0, false) => ("Pour démarrer : tout ce qui est exigé est rempli".to_string(), "Ce qui suit est utile : sans ces cases, les gestes se font autrement."),
        (n, _) => (
            format!("Pour démarrer : {} à remplir d'avance", if n == 1 { "une case exigée".to_string() } else { format!("{} cases exigées", n) }),
            "Une case exigée bloque son geste tant qu'elle est vide ; ce qui est rempli d'avance ne se demandera plus au moment de l'acte.",
        ),
    };
    let phrase = if note.is_empty() { format!("{}.", titre) } else { format!("{}. {}", titre, note) };
    let titre = if lignes.is_empty() { String::new() } else { titre };
    let mut poses = poses(ctx);
    poses.extend(pose);
    // Chaque ligne en phrase entiere, puis ce que le moteur a pose de lui-meme : ce que la ligne de commande lit.
    let mut phrases: Vec<Value> = lignes.iter().map(|l| l["phrase"].clone()).collect();
    phrases.extend(poses.iter().map(|p| json!(format!("{} : {}", p["libelle"].as_str().unwrap_or(""), p["texte"].as_str().unwrap_or("")))));
    Ok(json!({ "phrase": phrase, "phrases": phrases, "titre": titre, "note": note, "manques": lignes, "poses": poses }))
}

/// Ce qui suit le refus d'un acte quand la compagnie n'a pas fini de demarrer : toute la liste, d'un coup, sans ce que
/// le refus `deja_dit` dit deja (un manque que le refus nomme, ou dont il donne l'explication, ne se redit pas).
pub fn a_la_suite(ctx: &Contexte, deja_dit: &str) -> Option<String> {
    let lignes: Vec<String> = manques(ctx)
        .iter()
        .filter(|x| x.get("bloque").is_some())
        .filter(|x| !x["trouver"].as_str().map(str::trim).is_some_and(|t| !t.is_empty() && deja_dit.contains(t)))
        .filter(|x| {
            !(x["libelle"].as_str().is_some_and(|l| deja_dit.contains(l)) && x["qui"].as_str().is_none_or(|q| deja_dit.contains(q)))
        })
        .map(|x| {
            let t = |k: &str| x[k].as_str().unwrap_or("").to_string();
            format!("« {} » pour {}, dans {}", t("libelle"), t("bloque"), t("ou"))
        })
        .collect();
    if lignes.is_empty() {
        return None;
    }
    Some(format!(" Pour démarrer, il manque encore : {}.", lignes.join(" ; ")))
}

/// L'etablissement de la fiche est-il en Alsace-Moselle (`droit::bareme::en_alsace_moselle`) ?
fn en_alsace_moselle(ctx: &Contexte) -> bool {
    ctx.entreprise.get("code_postal").map(socle::date::en_texte).is_some_and(|c| droit::bareme::en_alsace_moselle(&c))
}

/// Le taux AT/MP de l'annee est-il encore celui que le moteur a pose a la creation ?
fn pose_a_la_creation(ctx: &Contexte) -> bool {
    matches!(ctx.taux_atmp_de(ctx.aujourdhui.an), Some(Ok(t)) if t.get("_pose_a_la_creation").is_some_and(vrai))
}

//! UN MOIS IMPORTE EST UN MOIS COMME LES AUTRES (ticket #215 du depot prive ; Leandre, 20/10/2026 : « j'ai importe les
//! bulletins, mais je pensais que les paies allaient avoir toutes les infos pour avoir les cases, qu'est-ce qui manque ? » ;
//! « un mois importe doit etre un mois comme les autres, avec ses cases » ; « il se dit en retard alors qu'il est deja
//! declare »).
//!
//! Un mois tenu AVANT ce logiciel n'a aucun engagement : il vit dans l'historique que l'import a ecrit
//! (`historique/paies.json`, une ligne par paie, de la DSN et/ou du bulletin ; `historique/engagements.json`, un contrat
//! par AEM ou par bulletin : `importation_ecrite`). Ce module le rend tel que la page Paie montre un mois saisi : une
//! CARTE PAR PAIE, avec ses cases (salarie, spectacle, debut et fin, jours travailles, cachets et heures, montants, etats),
//! remplies de ce que les pieces importees portent, et rapprochees de ce que le moteur sait par ailleurs -- la fiche du
//! salarie (son nom), celle du spectacle (son titre), le contrat qu'une AEM atteste (cachets, heures, numero d'objet), le
//! journal des depots et les certificats DPAE (`retours/dpae/`).
//!
//! RIEN NE SE DEVINE (regle 1) : une case qu'aucune piece ne porte reste vide, et la carte DIT pourquoi (`manque` : quel
//! champ, et ce qui l'aurait donne). Rien ne s'ecrit ici : c'est une lecture.
//!
//! LA DSN D'UN MOIS TENU AVANT (`dsn_avant`) : le journal des depots d'abord (un depot que les retours attestent y entre
//! de lui-meme : `reprise_journal::reprendre_les_depots`) ; a defaut, la DSN que l'import a lue pour ce mois (le fichier
//! de l'ancien logiciel : `declare_par: "DSN"`) -- le mois est declare, rien ne propose de la refaire (regle 4 : une
//! declaration deposee ne se regenere pas, et PuppetPaie n'a pas calcule ce mois). Sans aucune trace, il le dit, sans
//! le dire « en retard » ni proposer de la produire.

use std::collections::BTreeSet;
use std::path::Path;

use serde_json::{json, Map, Value};

use droit::contexte::Contexte;
use envois::etats;
use socle::valeur::valeur_en_texte;
use socle::echec::R;
use socle::fichiers::vrai;
use crate::importation_completer::A_COMPLETER;
use crate::routes_saisie::lire_saisie;

/// `historique/<nom>.json`, sa liste `cle` ; vide si le fichier n'existe pas. Un fichier illisible refuse.
fn liste(racine: &Path, nom: &str, cle: &str) -> R<Vec<Value>> {
    let p = racine.join("historique").join(nom);
    if !p.exists() {
        return Ok(Vec::new());
    }
    let v = lire_saisie(&p)?;
    Ok(v.get(cle).and_then(Value::as_array).cloned().unwrap_or_default())
}

fn texte(v: &Value, k: &str) -> String {
    v.get(k).filter(|x| vrai(x)).map(valeur_en_texte).map(|t| t.trim().to_string()).unwrap_or_default()
}

/// Le contrat touche-t-il `mois` (`AAAA-MM`) ? Ses bornes en ISO ; une fin absente vaut le debut.
fn touche(debut: &str, fin: &str, mois: &str) -> bool {
    if debut.len() < 7 {
        return false;
    }
    let fin = if fin.len() < 7 { debut } else { fin };
    &debut[..7] <= mois && mois <= &fin[..7]
}

/// Les paies de l'historique importe pour `mois`.
fn paies_du_mois(racine: &Path, mois: &str) -> R<Vec<Value>> {
    Ok(liste(racine, "paies.json", "paies")?.into_iter().filter(|p| texte(p, "mois") == mois).collect())
}

/// Les contrats attestes (AEM, bulletins) qui touchent `mois`.
fn contrats_du_mois(racine: &Path, mois: &str) -> R<Vec<Value>> {
    Ok(liste(racine, "engagements.json", "engagements")?
        .into_iter()
        .filter(|e| touche(&texte(e, "debut"), &texte(e, "fin"), mois))
        .collect())
}

/// Le mois est-il tenu avant ce logiciel : aucun engagement ne le touche, et l'historique importe en parle.
pub(crate) fn importe(racine: &Path, mois: &str) -> R<bool> {
    if engagement::stock::a_une_saisie(racine, mois).map_err(socle::echec::Echec::from)? {
        return Ok(false);
    }
    Ok(!paies_du_mois(racine, mois)?.is_empty() || !contrats_du_mois(racine, mois)?.is_empty())
}

/// Les mois que l'historique importe porte (une paie, ou un contrat qui les touche), qu'aucun engagement ne touche.
pub(crate) fn mois_importes(racine: &Path) -> R<Vec<String>> {
    let mut mois: BTreeSet<String> = BTreeSet::new();
    for p in liste(racine, "paies.json", "paies")? {
        let m = texte(&p, "mois");
        if m.len() == 7 {
            mois.insert(m);
        }
    }
    for e in liste(racine, "engagements.json", "engagements")? {
        let (debut, fin) = (texte(&e, "debut"), texte(&e, "fin"));
        if debut.len() < 7 {
            continue;
        }
        let fin = if fin.len() < 7 { debut.clone() } else { fin };
        let mut m = debut[..7].to_string();
        while m.as_str() <= &fin[..7] {
            mois.insert(m.clone());
            m = mois_suivant(&m);
        }
    }
    Ok(mois.into_iter().filter(|m| !engagement::stock::a_une_saisie(racine, m).unwrap_or(false)).collect())
}

fn mois_suivant(m: &str) -> String {
    let an: i32 = m[..4].parse().unwrap_or(0);
    let mo: i32 = m[5..7].parse().unwrap_or(12);
    if mo >= 12 {
        format!("{:04}-01", an + 1)
    } else {
        format!("{:04}-{:02}", an, mo + 1)
    }
}

/// LA DSN QUE L'IMPORT A LUE POUR `mois` : le fichier de l'ancien logiciel (`declare_par: "DSN"`), ou `None`. Rend
/// `{"importee": true, "etablie_le", "fichier"}` -- la date est celle que le fichier porte (S20.G00.05.007, sa
/// constitution), pas celle d'un depot : aucun bilan de la plate-forme n'est lu pour lui.
pub(crate) fn dsn_importee(racine: &Path, mois: &str) -> R<Option<Value>> {
    if engagement::stock::a_une_saisie(racine, mois).map_err(socle::echec::Echec::from)? {
        return Ok(None);
    }
    let p = paies_du_mois(racine, mois)?;
    Ok(p.iter().find(|x| texte(x, "declare_par") == "DSN").map(|x| {
        json!({"importee": true, "passee": false, "refusee": false, "etablie_le": texte(x, "depose_le"), "fichier": texte(x, "fichier")})
    }))
}

/// L'etat de la DSN d'un mois tenu avant ce logiciel, en mots : `etat` (« passee », « attente », « refusee » : le
/// journal ; « importee » : la DSN de l'ancien logiciel ; « sans_trace »), `courte`, `phrase`, `quand`.
pub(crate) fn dsn_avant(ctx: &Contexte, mois: &str) -> R<Value> {
    let lignes = etats::charger(&ctx.racine)?;
    let e = etats::dsn_du_mois(&lignes, &ctx.entreprise, mois)?;
    let quand = |e: &Value| etats::quand_lisible(e.get("quand").unwrap_or(&Value::Null));
    if vrai(&e) {
        let refusee = e.get("refusee").is_some_and(vrai);
        let passee = e.get("passee").is_some_and(vrai);
        let (etat, courte, phrase) = if refusee {
            ("refusee", "refusée".to_string(), format!("DSN déposée le {} — REFUSÉE par la plate-forme", quand(&e)))
        } else if passee {
            ("passee", "déposée — passée".to_string(), format!("DSN déposée le {} — passée", quand(&e)))
        } else {
            ("attente", "déposée — retour attendu".to_string(), format!("DSN déposée le {} — retour attendu", quand(&e)))
        };
        return Ok(json!({"etat": etat, "courte": courte, "phrase": phrase, "quand": e.get("quand").cloned().unwrap_or(Value::Null)}));
    }
    if let Some(d) = dsn_importee(&ctx.racine, mois)? {
        let etablie = texte(&d, "etablie_le");
        let jour = if etablie.len() == 10 { format!(" (établie le {}/{}/{})", &etablie[8..10], &etablie[5..7], &etablie[..4]) } else { String::new() };
        return Ok(json!({
            "etat": "importee",
            "courte": "déclarée avant PuppetPaie",
            "phrase": format!(
                "DSN déclarée avant PuppetPaie : celle de l'ancien logiciel, importée{} — aucun bilan de la plate-forme n'est lu pour elle. Rien n'est à refaire ici.",
                jour
            ),
            "quand": Value::Null,
        }));
    }
    Ok(json!({
        "etat": "sans_trace",
        "courte": "aucune trace de sa DSN",
        "phrase": "Mois tenu avant PuppetPaie : aucune trace de sa DSN — ni au journal, ni dans les retours relevés, ni parmi les DSN importées. Si elle est partie, le relevé des retours ou l'import de la DSN de l'ancien logiciel la retrouvera ; si elle n'est jamais partie, elle se fait dans le logiciel qui a calculé ces paies.",
        "quand": Value::Null,
    }))
}

// ------------------------------------------------------------------ les cartes

/// Le nom d'un salarie, comme les cartes le montrent : « prenom NOM » de sa fiche, ou `None` sans fiche.
fn nom_du_salarie(ctx: &Contexte, cle: &str) -> Option<String> {
    let s = ctx.salaries.get(cle).filter(|s| vrai(s))?;
    let prenom = if vrai(s.get("prenom_affichage").unwrap_or(&Value::Null)) { texte(s, "prenom_affichage") } else { texte(s, "prenom") };
    let nom = texte(s, "nom");
    let tout = format!("{} {}", prenom, nom).trim().to_string();
    (!tout.is_empty()).then_some(tout)
}

/// La fiche du spectacle de ce numero d'objet : `(cle, titre)` ; le titre est vide s'il est « A COMPLETER ».
fn spectacle_de(ctx: &Contexte, numero: &str) -> Option<(String, String)> {
    if numero.is_empty() {
        return None;
    }
    ctx.spectacles.iter().filter(|(k, _)| !k.starts_with('_')).find(|(_, s)| texte(s, "numero_objet").eq_ignore_ascii_case(numero)).map(|(k, s)| {
        let t = texte(s, "titre");
        (k.clone(), if t.eq_ignore_ascii_case(A_COMPLETER) { String::new() } else { t })
    })
}

/// Les jours d'intervalles `[{"debut", "fin"}]` (les dates travaillees d'un bulletin), dans `mois`.
fn jours_du_mois(dates: &Value, mois: &str) -> Vec<String> {
    let mut out = Vec::new();
    for d in dates.as_array().map_or(&[][..], |a| &a[..]) {
        let (Ok(a), Ok(b)) = (socle::date::Jour::lire(&texte(d, "debut")), socle::date::Jour::lire(&texte(d, "fin"))) else { continue };
        let mut j = a;
        while j <= b {
            let iso = j.iso();
            if iso.starts_with(mois) && !out.contains(&iso) {
                out.push(iso);
            }
            match j.plus_jours(1) {
                Ok(n) => j = n,
                Err(_) => break,
            }
        }
    }
    out.sort();
    out
}

fn manque(champ: &str, libelle: &str, phrase: String) -> Value {
    json!({"champ": champ, "libelle": libelle, "phrase": phrase})
}

/// L'etat de la DPAE d'une embauche tenue avant : le journal (reel accepte), puis les certificats de `retours/dpae/`.
fn dpae_de(journal: &etats::Dico<Value>, certifiees: &[Value], salarie: &str, debut: &str) -> Value {
    if let Ok(Some(q)) = journal.get(&[json!(salarie), json!(debut)]) {
        return json!({"couleur": "bien", "courte": "DPAE faite", "phrase": format!("DPAE déposée le {}", etats::quand_lisible(q))});
    }
    let certifiee = certifiees.iter().find(|c| {
        c.get("embauches").and_then(Value::as_array).is_some_and(|l| l.iter().any(|e| texte(e, "salarie") == salarie && texte(e, "debut") == debut))
    });
    if let Some(c) = certifiee {
        return json!({"couleur": "bien", "courte": "DPAE faite", "phrase": format!("DPAE certifiée par l'Urssaf ({})", etats::quand_lisible(c.get("quand").unwrap_or(&Value::Null)))});
    }
    json!({"couleur": "", "courte": "avant PuppetPaie", "phrase": "DPAE faite avant PuppetPaie : aucun certificat de l'Urssaf n'est relevé pour cette embauche"})
}

/// Une carte : une paie importee (et le contrat atteste qui va avec), ou un contrat atteste seul.
fn carte(ctx: &Contexte, mois: &str, paie: Option<&Value>, contrat: Option<&Value>, dpae: Value) -> Value {
    let vide = json!({});
    let p = paie.unwrap_or(&vide);
    let c = contrat.unwrap_or(&vide);
    let lire = |k: &str| {
        let t = texte(p, k);
        if t.is_empty() { texte(c, k) } else { t }
    };
    let salarie = lire("salarie");
    let (debut, fin) = (lire("debut"), lire("fin"));
    let numero = lire("numero_objet");
    let spectacle = spectacle_de(ctx, &numero);
    let titre_bulletin = texte(p, "spectacle");
    let titre = match &spectacle {
        Some((_, t)) if !t.is_empty() => t.clone(),
        _ => titre_bulletin.clone(),
    };
    let nom = nom_du_salarie(ctx, &salarie);
    let dates = jours_du_mois(p.get("dates").unwrap_or(&Value::Null), mois);
    let nombre = |k: &str| c.get(k).filter(|x| !x.is_null()).cloned();
    let bulletin = p.get("bulletin").filter(|b| b.is_object()).cloned();
    let declare_par = texte(p, "declare_par");

    // Les montants, et d'ou chacun vient : le bulletin fait foi sur ceux qu'il imprime (`importation_bulletins::completer`).
    let mut montants = Map::new();
    let mut sources = Map::new();
    for k in ["brut", "net_imposable", "net_a_payer", "pas", "net_verse", "cotisations_salariales", "cotisations_patronales", "cout_employeur", "net_avant_impot"] {
        let mut v = texte(p, k);
        let mut source = if bulletin.is_some() && !matches!(k, "net_verse") { "bulletin" } else { declare_par.as_str() };
        if v.is_empty() && k == "brut" && paie.is_none() {
            v = texte(c, "brut");
            source = "AEM";
        }
        if !v.is_empty() {
            montants.insert(k.into(), json!(v));
            sources.insert(k.into(), json!(if source == "DSN" { "DSN" } else if source.is_empty() { "historique" } else { source }));
        }
    }

    // CE QUI MANQUE, case par case, et ce qui l'aurait donne.
    let mut manques: Vec<Value> = Vec::new();
    if salarie.is_empty() {
        manques.push(manque("salarie", "salarié", "aucune pièce importée ne rattache cette paie à une fiche".into()));
    } else if nom.is_none() {
        manques.push(manque("salarie", "salarié", format!("la fiche « {} » n'existe pas (ou plus) : à créer dans Configuration › Salariés", salarie)));
    }
    if numero.is_empty() && titre.is_empty() {
        manques.push(manque(
            "spectacle",
            "spectacle",
            "ni la DSN, ni l'AEM, ni le bulletin ne portent de numéro d'objet pour cette paie (un permanent n'en a pas)".into(),
        ));
    } else if !numero.is_empty() && titre.is_empty() {
        manques.push(manque(
            "spectacle",
            "titre du spectacle",
            format!("la DSN ne porte que le numéro d'objet {} : son titre se complète dans Configuration › Spectacles", numero),
        ));
    }
    if dates.is_empty() {
        let pourquoi = match (bulletin.is_some(), declare_par.as_str(), texte(c, "atteste_par").as_str()) {
            (true, _, _) => "le bulletin n'imprime pas ses dates travaillées",
            (false, "DSN", _) => "la DSN ne déclare que le début et la fin du contrat ; le bulletin de cette paie les donnerait",
            (false, _, "AEM") => "l'AEM ne porte que le début, la fin et le nombre de cachets ; le bulletin de cette paie les donnerait",
            _ => "aucune pièce importée ne les porte",
        };
        manques.push(manque("dates", "jours travaillés", pourquoi.into()));
    }
    if contrat.is_none() {
        manques.push(manque(
            "cachets",
            "cachets et heures",
            if bulletin.is_some() {
                "le bulletin ne les donne pas en nombres entiers lisibles, et aucune AEM n'atteste ce contrat".into()
            } else {
                "la DSN ne les déclare pas, et aucune AEM ni aucun bulletin n'atteste ce contrat".into()
            },
        ));
    }
    for (k, libelle, pourquoi) in [
        ("brut", "brut", "aucune pièce importée ne le porte"),
        ("net_imposable", "net imposable", "aucune pièce importée ne le porte"),
        (
            "net_a_payer",
            "net à payer",
            if declare_par == "DSN" {
                "la DSN ne le déclare pas (son 50.004 est le net versé) ; le bulletin de cette paie le donnerait"
            } else {
                "aucune pièce importée ne le porte ; le bulletin de cette paie le donnerait"
            },
        ),
        ("pas", "prélèvement à la source", "aucune pièce importée ne le porte"),
    ] {
        if !montants.contains_key(k) {
            manques.push(manque(k, libelle, pourquoi.into()));
        }
    }

    let documents = match &bulletin {
        Some(b) => json!({"couleur": "bien", "courte": "bulletin importé", "phrase": format!("bulletin de l'ancien logiciel{}, copié dans historique/bulletins/", {
            let n = texte(b, "numero");
            if n.is_empty() { String::new() } else { format!(" n° {}", n) }
        })}),
        None => json!({"couleur": "", "courte": "avant PuppetPaie", "phrase": "documents remis avant PuppetPaie : aucun bulletin importé pour cette paie"}),
    };
    json!({
        "salarie": salarie,
        "nom": nom.clone().unwrap_or_else(|| salarie.clone()),
        "spectacle": {
            "cle": spectacle.as_ref().map(|s| s.0.clone()),
            "numero_objet": numero,
            "titre": titre,
        },
        "contrat": texte(p, "contrat"),
        "debut": debut,
        "fin": fin,
        "dates": dates,
        "cachets": nombre("cachets"),
        "heures": nombre("heures"),
        "jours": nombre("jours"),
        "montants": montants,
        "sources": sources,
        "lignes": p.get("lignes").filter(|l| l.is_array()).cloned().unwrap_or(json!([])),
        "declare_par": if declare_par.is_empty() { Value::Null } else { json!(declare_par) },
        "atteste_par": c.get("atteste_par").cloned().unwrap_or(Value::Null),
        "bulletin": bulletin.unwrap_or(Value::Null),
        "etats": {
            "dpae": dpae,
            "signature": {"couleur": "", "courte": "avant PuppetPaie", "phrase": "contrat fait avant PuppetPaie : sa signature n'est pas suivie ici"},
            "documents": documents,
        },
        "manque": manques,
    })
}

/// LES ETAPES D'UN MOIS TENU AVANT (`bilan-du-mois`, la colonne de droite de la page Paie) : tout s'est fait ailleurs,
/// aucune etape n'a de bouton (`vide`) -- ni « Fermer le mois », ni « Deposer la DSN » (regle 4). La DSN dit ce qu'on en
/// sait (`dsn_avant`) ; sans trace, elle reste en alerte, sans geste.
pub(crate) fn etapes(vue: &Value) -> Value {
    let cartes = vue["paies"].as_array().map_or(&[][..], |a| &a[..]);
    let n = cartes.len();
    let pl = |n: usize| if n > 1 { "s" } else { "" };
    let dpae_faites = cartes.iter().filter(|c| c["etats"]["dpae"]["couleur"] == json!("bien")).count();
    let bulletins = cartes.iter().filter(|c| c["bulletin"].is_object()).count();
    let dpae = if dpae_faites == n {
        format!("Mois tenu avant PuppetPaie — {} DPAE retrouvée{} dans le journal ou les certificats de l'Urssaf", n, pl(n))
    } else {
        format!(
            "Mois tenu avant PuppetPaie — {} DPAE sur {} retrouvée{} dans le journal ou les certificats de l'Urssaf ; les autres ont été faites ailleurs, sans trace relevée ici",
            dpae_faites,
            n,
            pl(dpae_faites)
        )
    };
    let dsn = &vue["dsn"];
    let sans_trace = dsn["etat"] == json!("sans_trace") || dsn["etat"] == json!("refusee");
    json!({
        "dpae": {"couleur": "bien", "vide": true, "courte": "avant PuppetPaie", "phrase": dpae},
        "contrat-signature": {"couleur": "bien", "vide": true, "courte": "avant PuppetPaie",
            "phrase": format!("Mois tenu avant PuppetPaie — {} contrat{} fait{} ailleurs : leur signature n'est pas suivie ici", n, pl(n), pl(n))},
        "dsn": {
            "couleur": if dsn["etat"] == json!("refusee") { "refus" } else if sans_trace { "alerte" } else { "bien" },
            "vide": true,
            "importe": true,
            "etat": dsn["etat"].clone(),
            "courte": dsn["courte"].clone(),
            "phrase": dsn["phrase"].clone(),
        },
        "documents": {"couleur": "bien", "vide": true, "courte": "avant PuppetPaie",
            "phrase": format!("Mois tenu avant PuppetPaie — {} bulletin{} de l'ancien logiciel importé{} sur {} paie{}", bulletins, pl(bulletins), pl(bulletins), n, pl(n))},
    })
}

/// LE MOIS TENU AVANT, comme la page Paie le montre : `None` si un engagement touche le mois ou si l'historique
/// importe n'en dit rien. Sinon `{"dsn", "paies", "totaux", "manques"}` : une carte par paie de l'historique (rapprochee
/// du contrat atteste des memes salarie, debut et fin), puis une par contrat atteste qu'aucune paie ne porte.
pub(crate) fn vue(ctx: &Contexte, mois: &str) -> R<Option<Value>> {
    if !importe(&ctx.racine, mois)? {
        return Ok(None);
    }
    let paies = paies_du_mois(&ctx.racine, mois)?;
    let contrats = contrats_du_mois(&ctx.racine, mois)?;
    let lignes = etats::charger(&ctx.racine)?;
    let embauches = etats::embauches_par_depot(&lignes)?;
    let (certifiees, _) = envois::reprise_journal::embauches_attestees(ctx);
    let mut pris = vec![false; contrats.len()];
    let mut cartes = Vec::new();
    for p in &paies {
        let (s, d, f, o) = (texte(p, "salarie"), texte(p, "debut"), texte(p, "fin"), texte(p, "numero_objet"));
        let candidats: Vec<usize> =
            (0..contrats.len()).filter(|&i| !pris[i] && texte(&contrats[i], "salarie") == s && texte(&contrats[i], "debut") == d && texte(&contrats[i], "fin") == f).collect();
        // Plusieurs contrats aux memes bornes : celui du meme numero d'objet ; sinon aucun (on ne choisit pas au hasard).
        let i = match candidats.len() {
            1 => Some(candidats[0]),
            0 => None,
            _ => {
                let memes: Vec<usize> = candidats.iter().copied().filter(|&i| texte(&contrats[i], "numero_objet").eq_ignore_ascii_case(&o)).collect();
                (memes.len() == 1).then(|| memes[0])
            }
        };
        if let Some(i) = i {
            pris[i] = true;
        }
        let dpae = dpae_de(&embauches, &certifiees, &s, &d);
        cartes.push(carte(ctx, mois, Some(p), i.map(|i| &contrats[i]), dpae));
    }
    for (i, c) in contrats.iter().enumerate() {
        if pris[i] {
            continue;
        }
        let dpae = dpae_de(&embauches, &certifiees, &texte(c, "salarie"), &texte(c, "debut"));
        cartes.push(carte(ctx, mois, None, Some(c), dpae));
    }
    cartes.sort_by(|a, b| (texte(a, "debut"), texte(a, "nom")).cmp(&(texte(b, "debut"), texte(b, "nom"))));

    // Les totaux du mois, sur ce qui est connu ; ce qui manque se compte a part.
    let mut totaux = Map::new();
    for k in ["brut", "net_a_payer", "cout_employeur"] {
        let mut somme = socle::num::Decimal::ZERO;
        let mut connus = 0usize;
        for c in &cartes {
            if let Some(v) = c["montants"].get(k).and_then(Value::as_str).and_then(socle::num::decimal_py) {
                somme += v;
                connus += 1;
            }
        }
        totaux.insert(k.into(), json!({"montant": if connus > 0 { json!(somme.round_dp(2).to_string()) } else { Value::Null }, "connus": connus}));
    }
    let manques = cartes.iter().filter(|c| c["manque"].as_array().is_some_and(|l| !l.is_empty())).count();
    Ok(Some(json!({
        "mois": mois,
        "dsn": dsn_avant(ctx, mois)?,
        "paies": cartes,
        "totaux": totaux,
        "incompletes": manques,
    })))
}

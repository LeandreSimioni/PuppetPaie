//! Les etats du personnel et du controle de `moteur/operations_etats.py` : la liste des contrats, le registre
//! unique du personnel (L1221-13, D1221-23), le fichier du personnel, le journal des declarations.
//!
//! Le registre et le fichier du personnel SONT des fichiers du personnel : ils portent le NIR, la date de
//! naissance et l'adresse, et l'avertissement le dit.

use std::collections::BTreeSet;

use serde_json::Value;

use droit::contexte::Contexte;
use socle::valeur::valeur_en_texte;
use socle::echec::erreur;
use crate::etats_paie::{
    dossier_exports, ecrire_fini, fiche_texte, finir, titre_spectacle, Demande, Lecture, Periode,
};
use socle::fichiers::vrai;
use socle::num::{eur2, Decimal};
use crate::tableaux_export::{col_m, col_t, jj, libelle_mois, qui, qui_souple, refus, sections_de, Ligne, Section, Tableau, Val, R};

/// `fiche.get(a) or fiche.get(b) or ...`, la valeur telle que le fichier la porte (un nombre reste un nombre).
fn premiere(fiche: &Value, cles: &[&str]) -> Option<Val> {
    for c in cles {
        if let Some(v) = fiche.get(*c).filter(|v| vrai(v)) {
            return Some(match v {
                Value::String(s) => Val::Txt(s.clone()),
                Value::Number(_) => Val::Num(v.clone()),
                autre => Val::Txt(valeur_en_texte(autre)),
            });
        }
    }
    None
}

// ============================================================ 13. les contrats

/// Un contrat par mois de paie, vu des etats du personnel.
struct ContratMois {
    mois: String,
    cle: String,
    fiche: Value,
    salarie: String,
    contrat: String,
    nature: String,
    emploi: String,
    spectacle: String,
    objet: String,
    debut: String,
    fin: String,
    brut: Option<Decimal>,
}

/// `_contrats` : chaque contrat de la periode, une ligne par mois de paie, lu dans la paie ecrite de son engagement
/// valide.
fn contrats_de(p: &Periode, lecture: &mut Lecture) -> R<Vec<ContratMois>> {
    let mut out = Vec::new();
    for m in p.saisis() {
        for paie in lecture.paies(&m)?.iter() {
            let e = &paie.engagement;
            let dates: Vec<String> = e.dates_travail.iter().map(|j| j.iso()).collect();
            let emploi = match e.emploi.clone().filter(|x| !x.is_empty()) {
                Some(x) => x,
                None => fiche_texte(&paie.salarie, "emploi"),
            };
            out.push(ContratMois {
                mois: m.clone(),
                cle: paie.salarie.get("_cle").map(valeur_en_texte).unwrap_or_default(),
                fiche: paie.salarie.clone(),
                salarie: qui(&paie.salarie)?,
                contrat: e.numero.clone().unwrap_or_default(),
                nature: e.nat().map_err(|x| erreur("ErreurOperation", x))?.libelle.clone(),
                emploi,
                spectacle: titre_spectacle(paie),
                objet: fiche_texte(&paie.spectacle, "numero_objet"),
                debut: e.debut.iso(),
                fin: match e.fin {
                    Some(f) => f.iso(),
                    None => dates.last().cloned().unwrap_or_default(),
                },
                brut: Some(eur2(paie.brut)),
            });
        }
    }
    Ok(out)
}

/// `operations_etats.liste_des_contrats`.
pub fn liste_des_contrats(ctx: &Contexte, d: &Demande) -> R<Tableau> {
    let p = Periode::de(ctx, d, true)?;
    let mut avert = Vec::new();
    let mut lecture = Lecture::nouvelle(ctx);
    let contrats = contrats_de(&p, &mut lecture)?;
    let mut lignes = Vec::new();
    for c in &contrats {
        let mut l = Ligne::nouvelle();
        l.t("mois", c.mois.clone()).t("cle", c.cle.clone()).t("salarie", c.salarie.clone()).t("contrat", c.contrat.clone()).t("nature", c.nature.clone());
        l.t("emploi", c.emploi.clone()).t("spectacle", c.spectacle.clone()).t("objet", c.objet.clone()).t("debut", jj(&c.debut)).t("fin", jj(&c.fin));
        l.od("brut", c.brut);
        lignes.push(l);
    }
    if contrats.iter().any(|c| c.contrat.is_empty()) {
        avert.push("Contrat sans numéro : son mois n'a pas été produit, aucun numéro n'est figé. Rien n'est inventé.".to_string());
    }
    let mut t = Tableau::nouveau("contrats", "Liste des contrats", p.libelle.clone());
    t.mois = p.mois.clone();
    t.avertissements = avert;
    t.sections = sections_de(lignes, |l| l.texte("mois"), |k| libelle_mois(k).unwrap_or_else(|_| k.clone()), Some(&|l| vec![l.texte("salarie"), l.texte("debut"), l.texte("contrat")]));
    t.colonnes = vec![
        col_t("mois", "Mois de paie"),
        col_t("contrat", "Contrat"),
        col_t("salarie", "Salarié"),
        col_t("nature", "Nature"),
        col_t("emploi", "Emploi"),
        col_t("spectacle", "Spectacle"),
        col_t("objet", "N° d'objet"),
        col_t("debut", "Début"),
        col_t("fin", "Fin"),
        col_m("brut", "Brut du mois"),
    ];
    t.sources = vec!["paies écrites des engagements validés".to_string()];
    finir(ctx, &mut t, Some(&p), "contrats", &d.format, d.ecrire)?;
    Ok(t)
}

// ================================================ 14. le registre du personnel

/// `NATIONALITES`.
fn nationalite(code: &str) -> &'static str {
    match code {
        "01" => "française",
        "02" => "Union européenne",
        "03" => "EEE hors Union européenne",
        "04" => "hors Union européenne et EEE",
        _ => "",
    }
}

/// `_sexe` : le premier chiffre du NIR (1 homme, 2 femme) ; un NIR provisoire ne le dit pas.
fn sexe(nir: Option<&Value>) -> &'static str {
    let t = nir.filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default();
    match t.chars().next() {
        Some('1') => "M",
        Some('2') => "F",
        _ => "",
    }
}

/// `operations_etats.registre_du_personnel` : une ligne par embauche, dans l'ordre des embauches, avec les mentions
/// obligatoires. Un intermittent est embauche a chaque contrat : chaque CDDU est une entree et une sortie.
pub fn registre_du_personnel(ctx: &Contexte, d: &Demande) -> R<Tableau> {
    let p = Periode::de(ctx, d, true)?;
    let mut avert = Vec::new();
    let mut vus: BTreeSet<(String, String, String, String)> = BTreeSet::new();
    let mut lignes: Vec<Ligne> = Vec::new();
    let mut lecture = Lecture::nouvelle(ctx);
    for c in contrats_de(&p, &mut lecture)? {
        // Un contrat qui deborde du mois apparait une fois par mois de paie : c'est UNE embauche.
        if !vus.insert((c.cle.clone(), c.contrat.clone(), c.debut.clone(), c.fin.clone())) {
            continue;
        }
        let f = &c.fiche;
        let texte = |cles: &[&str], defaut: &str| -> String { cles.iter().find_map(|k| f.get(*k).filter(|v| vrai(v)).map(valeur_en_texte)).unwrap_or_else(|| defaut.to_string()) };
        let mut l = Ligne::nouvelle();
        l.t("nom", texte(&["nom_affichage", "nom"], &c.salarie)).t("prenom", texte(&["prenom_affichage", "prenom"], ""));
        // La codification relue comme la DSN la declare (`codification_ue`) : un ancien 03 sans pays ne se lit pas « EEE ».
        let code_ue = if texte(&["code_nationalite_dsn", "pays_nationalite", "pays_nationalite_hors_liste"], "").is_empty() {
            Ok(String::new())
        } else {
            socle::codification_ue::codification_ue(f)
        };
        // LE PAYS, quand la fiche le porte (vagues 1j et 2h, reste R1 #147) : le registre dit la nationalite (C. trav.,
        // art. D1221-23, 1°), la categorie DSN la suit entre parentheses ; sans pays, la categorie seule.
        let pays = match texte(&["pays_nationalite"], "") {
            c if c.is_empty() => texte(&["pays_nationalite_hors_liste"], ""),
            c => socle::pays::du_code(&c).ok().flatten().map_or(c, |p| p.nom),
        };
        let categorie = code_ue.map_or_else(|_| "à préciser dans la fiche".to_string(), |c| nationalite(&c).to_string());
        l.t(
            "nationalite",
            match (pays.is_empty(), categorie.is_empty()) {
                (true, _) => categorie,
                (false, true) => pays,
                (false, false) => format!("{} ({})", pays, categorie),
            },
        );
        l.t("naissance", jj(&texte(&["date_naissance"], ""))).t("sexe", sexe(f.get("nir")));
        l.t("emploi", c.emploi.clone()).t("qualification", texte(&["statut"], "")).t("contrat_type", c.nature.clone()).t("contrat", c.contrat.clone());
        l.t("entree", c.debut.clone()).t("sortie", c.fin.clone());
        lignes.push(l);
    }
    lignes.sort_by_cached_key(|l| (l.texte("entree"), l.texte("nom"), l.texte("contrat")));
    for (i, l) in lignes.iter_mut().enumerate() {
        l.n("ordre", (i + 1) as i64);
        let (e, s) = (jj(&l.texte("entree")), jj(&l.texte("sortie")));
        l.t("entree", e).t("sortie", s);
    }
    avert.push("La nationalité est le pays de nationalité de la fiche, suivi de la catégorie déclarée en DSN (française, Union européenne, EEE hors Union, reste du monde) ; une fiche sans pays n'en montre que la catégorie. Pour un salarié étranger hors UE, le registre doit aussi porter le type et le numéro du titre de travail (art. D1221-24) — à compléter à la main.".to_string());
    avert.push("Registre unique du personnel : il porte des données personnelles et se conserve cinq ans après le départ de chaque salarié. Il se présente à l'inspection du travail sur demande.".to_string());
    let mut t = Tableau::nouveau("registre-du-personnel", "Registre unique du personnel", p.libelle.clone());
    t.mois = p.mois.clone();
    t.avertissements = avert;
    t.sections = vec![Section::nouvelle("", lignes)];
    t.avec_total = false;
    t.donnees_personnelles = true;
    t.colonnes = vec![
        col_t("ordre", "N°"),
        col_t("nom", "Nom"),
        col_t("prenom", "Prénom"),
        col_t("nationalite", "Nationalité"),
        col_t("naissance", "Date de naissance"),
        col_t("sexe", "Sexe"),
        col_t("emploi", "Emploi"),
        col_t("qualification", "Qualification"),
        col_t("contrat_type", "Type de contrat"),
        col_t("contrat", "Contrat"),
        col_t("entree", "Entrée"),
        col_t("sortie", "Sortie"),
    ];
    t.sources = vec!["Code du travail, art. L1221-13 (registre, ordre des embauches), D1221-23 (mentions : nationalité, date de naissance, sexe, emploi, qualification, dates d'entrée et de sortie), D1221-24 (type de contrat, titre de travail), Légifrance, relevé le 23/09/2026".to_string()];
    finir(ctx, &mut t, Some(&p), "registre-du-personnel", &d.format, d.ecrire)?;
    Ok(t)
}

// ============================================= 15. le fichier du personnel

/// `operations_etats.fichier_du_personnel` : chaque fiche de salarie, coordonnees comprises -- ce que le comptable
/// demande a l'ouverture du dossier. Pas de periode : ce sont les fiches d'aujourd'hui.
pub fn fichier_du_personnel(ctx: &Contexte, d: &Demande) -> R<Tableau> {
    let mut fiches: Vec<(&String, &Value)> = ctx.salaries.iter().collect();
    fiches.sort_by(|a, b| a.0.cmp(b.0));
    let mut lignes = Vec::new();
    for (cle, f) in fiches {
        if !f.is_object() {
            continue;
        }
        let mut l = Ligne::nouvelle();
        let ou_cle = |cles: &[&str]| premiere(f, cles).unwrap_or_else(|| Val::Txt(String::new()));
        l.poser("code", Some(premiere(f, &["code_salarie"]).unwrap_or_else(|| Val::Txt(cle.clone()))));
        l.poser("nom", Some(premiere(f, &["nom_affichage", "nom"]).unwrap_or_else(|| Val::Txt(cle.clone()))));
        l.poser("prenom", Some(ou_cle(&["prenom_affichage", "prenom"])));
        let nir = f.get("nir").filter(|v| vrai(v));
        l.t("nir", match nir {
            Some(n) => format!("{} {}", valeur_en_texte(n), f.get("nir_cle").filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default()),
            None => String::new(),
        });
        l.t("naissance", jj(&fiche_texte(f, "date_naissance")));
        l.poser("lieu", Some(ou_cle(&["lieu_naissance_affichage", "lieu_naissance"])));
        l.poser("adresse", Some(ou_cle(&["adresse_affichage", "adresse"])));
        l.poser("cp", Some(ou_cle(&["code_postal"])));
        l.poser("commune", Some(ou_cle(&["commune_affichage", "commune"])));
        l.poser("email", Some(ou_cle(&["email"])));
        l.poser("emploi", Some(ou_cle(&["emploi_affichage", "emploi"])));
        l.poser("statut", Some(ou_cle(&["statut"])));
        l.poser("cs", Some(ou_cle(&["num_conges_spectacles"])));
        l.t("actif", if f.get("actif").map_or(true, vrai) { "oui" } else { "archivé" });
        lignes.push(l);
    }
    if lignes.is_empty() {
        return Err(refus("Aucune fiche de salarié."));
    }
    let mut t = Tableau::nouveau("fichier-du-personnel", "Fichier du personnel", "fiches au jour de l'export");
    t.avec_total = false;
    t.donnees_personnelles = true;
    t.sections = vec![Section::nouvelle("", lignes)];
    t.avertissements = vec!["Ce fichier porte le NIR, la date de naissance et l'adresse des salariés : il reste dans prive/, et ne s'envoie qu'à qui en a besoin.".to_string()];
    t.colonnes = vec![
        col_t("code", "Code"),
        col_t("nom", "Nom"),
        col_t("prenom", "Prénom"),
        col_t("nir", "NIR"),
        col_t("naissance", "Né(e) le"),
        col_t("lieu", "à"),
        col_t("adresse", "Adresse"),
        col_t("cp", "Code postal"),
        col_t("commune", "Commune"),
        col_t("email", "Courriel"),
        col_t("emploi", "Emploi"),
        col_t("statut", "Statut"),
        col_t("cs", "N° Congés Spectacles"),
        col_t("actif", "Fiche"),
    ];
    t.sources = vec!["config/salaries.json".to_string()];
    finir(ctx, &mut t, None, "fichier-du-personnel", &d.format, d.ecrire)?;
    Ok(t)
}

// ===================================== 16. le journal des declarations

/// `ACTES_LIBELLES`.
fn acte_lisible(quoi: &str) -> Option<&'static str> {
    Some(match quoi {
        "dsn" => "DSN",
        "dpae" => "DPAE",
        "documents" => "Documents au salarié",
        "contrat-signature" => "Contrat envoyé à la signature",
        "contrat-recu" => "Contrat signé reçu",
        "contrat-contresigne" => "Contrat contresigné renvoyé",
        "regularisation-casc" => "Régularisation CASC-SVP",
        "dossier-salarie" => "Dossier envoyé au salarié",
        _ => return None,
    })
}

/// `_reponse` : une reponse lisible ; un rapport XML complet ne tient pas dans une case.
fn reponse(r: Option<&Value>) -> String {
    let t = r.filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default();
    let t = t.split(socle::texte::blanc).filter(|m| !m.is_empty()).collect::<Vec<_>>().join(" ");
    if t.starts_with('<') {
        return "rapport de l'organisme (XML, conservé au journal)".to_string();
    }
    t.chars().take(160).collect()
}

/// `operations_etats.journal_des_declarations` : chaque acte qui a engage la compagnie -- DSN, DPAE, envois,
/// signatures --, avec sa date, son mode, son issue et son fichier : la piste d'audit.
pub fn journal_des_declarations(ctx: &Contexte, d: &Demande, journal: &[Value]) -> R<Tableau> {
    let p = Periode::de(ctx, d, false)?;
    let mut lignes = Vec::new();
    for l in journal {
        // Un acte porte le mois qu'il declare ; un acte annuel se range au mois ou il a eu lieu.
        let texte = |k: &str| l.get(k).filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default();
        let mut m = texte("mois");
        if m.chars().count() != 7 {
            m = texte("quand").chars().take(7).collect();
        }
        if m.chars().count() != 7 || !p.contient(&m) {
            continue;
        }
        let quoi = texte("quoi");
        let mut x = Ligne::nouvelle();
        x.t("quand", texte("quand").replace('T', " ").chars().take(16).collect::<String>());
        let main_propre = quoi == "contrat-signature" && l.pointer("/detail/main_propre").is_some_and(vrai);
        x.t("acte", if main_propre { "Contrat imprimé, à remettre en main propre".to_string() } else { acte_lisible(&quoi).map(str::to_string).unwrap_or(quoi) });
        x.t("mois", m).t("mode", texte("mode")).t("issue", if l.get("accepte").is_some_and(vrai) { "accepté" } else { "refusé ou échoué" });
        x.t("fichier", texte("fichier")).t("destinataire", texte("destinataire")).t("reponse", reponse(l.get("reponse"))).t("origine", texte("origine"));
        lignes.push(x);
    }
    if lignes.is_empty() {
        return Err(refus(format!("Aucun acte au journal sur la période ({}).", p.libelle)));
    }
    let mut t = Tableau::nouveau("journal-des-declarations", "Journal des déclarations et envois", p.libelle.clone());
    t.mois = lignes.iter().map(|l| l.texte("mois")).collect::<BTreeSet<_>>().into_iter().collect();
    t.avec_total = false;
    t.sections = sections_de(lignes, |l| l.texte("mois"), |k| libelle_mois(k).unwrap_or_else(|_| k.clone()), Some(&|l| vec![l.texte("quand")]));
    t.colonnes = vec![
        col_t("quand", "Date"),
        col_t("acte", "Acte"),
        col_t("mois", "Mois"),
        col_t("mode", "Mode"),
        col_t("issue", "Issue"),
        col_t("fichier", "Fichier"),
        col_t("destinataire", "Destinataire"),
        col_t("reponse", "Réponse"),
        col_t("origine", "Reconstitué depuis"),
    ];
    t.sources = vec!["journal des actes de PuppetPaie (journal.py) : une ligne écrite après chaque acte, jamais avant".to_string()];
    finir(ctx, &mut t, Some(&p), "journal-des-declarations", &d.format, d.ecrire)?;
    Ok(t)
}

#[allow(dead_code)]
fn _inutilises() {
    let _ = dossier_exports;
    let _ = ecrire_fini;
    let _ = qui_souple;
}

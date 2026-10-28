//! Les routes du lot H1 : produire des fichiers, sans rien envoyer (`moteur/web.py`,
//! `operations_declarations.produire_dpae`, `operations_arrets.produire`,
//! `operations_permanents.produire_fctu` et `documents_de_fin`, `documents_fin`) :
//! `dpae`, `arret-signaler`, `fin-contrat-signaler` et `fin-contrat-documents`.
//!
//! Les formats sont ailleurs (`dpae`, `signalement`, `fctu`, `amorcage`, la fabrique de PDF) : ce
//! module ne fait que l'enchainement -- charger, choisir, refuser, nommer, ecrire. Memes fichiers
//! ecrits, memes reponses que le Python (ce qui compte, ce sont les VALEURS) ; une phrase de refus
//! garde sa forme quand elle est utile a la page, la classe de l'exception peut differer.
//!
//! AUCUN DEPOT, AUCUN RESEAU : rien ne part d'ici. Le jour est celui du contexte
//! (`PUPPETPAIE_AUJOURDHUI`, que `paie_rust` pose a la date du jour) ; l'horodatage d'un fichier
//! partiel de DPAE est la minute de l'horloge, a l'heure de Paris.

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};

use engagement::calcul::Paie;
use droit::contexte::Contexte;
use socle::date::{en_texte, repr_texte, Jour};
use declarations::dpae::{Chemins, Embauche, Parametres};
use envois::etats;
use socle::valeur::valeur_en_texte;
use socle::echec::R;
use socle::echec::erreur;
use socle::fichiers::{dossier_du_code, vrai};
use socle::num::eur2;
use socle::echec::Echec;
use socle::echec::Refus;

/// Les routes que ce module sert.
pub const ROUTES: [&str; 4] = ["dpae", "arret-signaler", "fin-contrat-signaler", "fin-contrat-documents"];

/// Une route de ce module.
pub fn route(ctx: &Contexte, nom: &str, charge: &Value) -> R<Value> {
    match nom {
        "dpae" => dpae(ctx, charge),
        "arret-signaler" => arret_signaler(ctx, charge),
        "fin-contrat-signaler" => fin_contrat_signaler(ctx, charge),
        "fin-contrat-documents" => fin_contrat_documents(ctx, charge),
        autre => Err(Echec::from(format!("route inconnue : {}", autre))),
    }
}

// ------------------------------------------------------------------ outils

#[track_caller]
fn refus(m: impl Into<String>) -> Echec {
    erreur("ErreurOperation", m)
}

fn en_echec(r: Refus) -> Echec {
    Echec { classe: r.classe, message: r.message }
}

#[track_caller]
fn plain(m: String) -> Echec {
    socle::echec::noter_et(None, m)
}

/// `charge["cle"]` : un texte requis.
fn texte_requis<'a>(charge: &'a Value, cle: &str) -> R<&'a str> {
    crate::routes::fiches::exiger(charge, cle)?.as_str().ok_or_else(|| erreur("TypeError", format!("`{}` : un texte est attendu", cle)))
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

/// Les secondes unix de l'horloge.
fn maintenant() -> f64 {
    socle::temps::secondes()
}

/// `datetime.now()` a l'heure de Paris, en `AAAA-MM-JJTHH:MM:SS`.
fn maintenant_iso() -> String {
    socle::paris::iso_secondes(maintenant())
}

fn mois_en_toutes_lettres(mois: &str) -> R<(String, String)> {
    let an = mois.get(..4).ok_or_else(|| erreur("ValueError", "mois illisible"))?;
    let rang: usize = mois
        .get(5..7)
        .and_then(|m| m.parse().ok())
        .ok_or_else(|| erreur("ValueError", format!("nombre entier illisible : {}", repr_texte(mois.get(5..7).unwrap_or("")))))?;
    let nom = socle::date::nom_du_mois(rang as i64, socle::date::Graphie::Titre).map_err(|e| erreur("IndexError", e))?;
    let annee: i64 = an.parse().map_err(|_| erreur("ValueError", format!("nombre entier illisible : {}", repr_texte(an))))?;
    Ok((format!("{}{}", nom, annee), format!("{} {}", nom, annee)))
}

/// Ce qui peut partir du mois : la saisie de ses engagements valides (`engagement::etats::validees_du_mois`), et leurs
/// paies ecrites ; refus si aucun engagement ne le touche.
fn saisie_du_mois(ctx: &Contexte, mois: &str) -> R<(Value, Vec<Paie>)> {
    engagement::etats::validees_du_mois(ctx, mois).map_err(plain)?.ok_or_else(|| refus(format!("Aucune saisie pour {}.", mois)))
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
fn cle_du_salarie(ctx: &Contexte, nom: &str) -> R<String> {
    let fiche = crate::routes::dossiers::salarie(ctx, &json!(nom))?;
    fiche.get("_cle").map(valeur_en_texte).ok_or_else(|| erreur("KeyError", socle::fichiers::absent("_cle")))
}

/// L'engagement de ce salarie (sa cle) qui se termine ce mois, et son rang dans la saisie. Un engagement se rattache a
/// sa fiche comme sa paie (`engagement::cle_de_l_engagement`), quelle que soit la casse du nom saisi. `nom` : comme on
/// l'a dit, pour la phrase.
/// Un salarie sans engagement ce mois-la n'a pas de paie : la phrase le dit, avec le mois en toutes lettres.
fn engagement_qui_finit<'a>(ctx: &Contexte, saisie: &'a Value, cle: &str, nom: &str, mois: &str) -> R<(usize, &'a Value)> {
    let eng = saisie.get("engagements").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]);
    let siens: Vec<(usize, &Value)> =
        eng.iter().enumerate().filter(|(_, x)| engagement::cle_de_l_engagement(ctx, x).as_deref() == Some(cle)).collect();
    if siens.is_empty() {
        return Err(refus(format!("{} n'a aucune paie en {} : aucun contrat ne s'y termine.", nom, mois_dit(mois))));
    }
    siens.into_iter().find(|(_, x)| x.get("fin").map_or(false, vrai)).ok_or_else(|| {
        refus(format!("{} n'a aucun contrat qui se termine en {} (la saisie ne dit pas de `fin`).", nom, mois_dit(mois)))
    })
}

/// La paie validee de l'engagement de rang `rang` (`paies` : celles de `saisie_du_mois`, au meme rang que sa saisie),
/// numerotee (les numeros ne sont que reserves).
fn paie_de_l_engagement(ctx: &Contexte, paies: &[Paie], rang: usize, cle: &str, nom: &str, mois: &str) -> R<Paie> {
    let annee = mois.chars().take(4).collect::<String>().parse::<i32>().map_err(|_| erreur("ValueError", format!("mois illisible : {}", repr_texte(mois))))?;
    let mut paies = paies.to_vec();
    documents::ecrire_documents::numeroter(ctx, &mut paies, annee).map_err(en_echec)?;
    paies
        .into_iter()
        .nth(rang)
        .filter(|p| p.salarie.get("_cle").map(valeur_en_texte).as_deref() == Some(cle))
        .ok_or_else(|| refus(format!("{} n'a aucune paie validee en {} pour ce contrat.", nom, mois_dit(mois))))
}

fn ecrire_texte(chemin: &Path, texte: &str) -> R<()> {
    if let Some(d) = chemin.parent() {
        std::fs::create_dir_all(d).map_err(|e| plain(format!("{} : {}", d.display(), e)))?;
    }
    std::fs::write(chemin, texte).map_err(|e| plain(format!("{} : {}", chemin.display(), e)))
}

// ------------------------------------------------------------------ fin-contrat-documents

/// Les mentions communes du certificat de travail et du recu (`documents_fin._marqueurs_communs`).
fn marqueurs_communs(date: &Jour, p: &Paie, ent: &Value) -> R<Map<String, Value>> {
    let s = &p.salarie;
    let e = &p.engagement;
    let req = |v: &Value, k: &str| -> R<String> { v.get(k).map(en_texte).ok_or_else(|| erreur("KeyError", socle::fichiers::absent(k))) };
    let ou_vide = |v: &Value, k: &str| -> String { v.get(k).map(en_texte).unwrap_or_default() };
    let sig = ent.get("signataire").ok_or_else(|| erreur("KeyError", socle::fichiers::absent("signataire")))?;
    let aujourdhui = date.jj_mm_aaaa();
    let mut m = Map::new();
    let mut poser = |k: &str, v: String| {
        m.insert(k.to_string(), Value::String(v));
    };
    poser("employeur.raison_sociale", req(ent, "raison_sociale")?);
    poser("employeur.adresse", req(ent, "adresse")?);
    poser("employeur.code_postal", req(ent, "code_postal")?);
    poser("employeur.commune", req(ent, "commune")?);
    poser("employeur.siret", req(ent, "siret")?);
    poser("employeur.naf", req(ent, "naf")?);
    poser("employeur.signataire", format!("{} {}", req(sig, "prenom")?, req(sig, "nom")?));
    poser("employeur.signataire_fonction", ou_vide(sig, "fonction"));
    poser("salarie.nom_complet", format!("{} {}", req(s, "prenom")?, req(s, "nom")?));
    poser(
        "salarie.date_naissance",
        documents::bulletin::jj_mm_aaaa(s.get("date_naissance").ok_or_else(|| erreur("KeyError", socle::fichiers::absent("date_naissance")))?).map_err(|m| erreur("ValueError", m))?,
    );
    poser("salarie.lieu_naissance", ou_vide(s, "lieu_naissance"));
    poser("salarie.adresse", req(s, "adresse")?);
    poser("salarie.code_postal", req(s, "code_postal")?);
    poser("salarie.commune", req(s, "commune")?);
    poser("contrat.numero", e.numero.clone().unwrap_or_default());
    poser("contrat.debut", e.debut.jj_mm_aaaa());
    poser("contrat.fin", e.fin.map(|f| f.jj_mm_aaaa()).unwrap_or_default());
    poser("contrat.emploi", ou_vide(s, "emploi"));
    poser("contrat.nature", e.nat().map_err(plain)?.libelle.clone());
    poser("certificat.date", aujourdhui.clone());
    poser("recu.date", aujourdhui);
    Ok(m)
}

/// `documents_fin._inventaire` : le detail des sommes, ligne a ligne -- pas un total.
fn inventaire(p: &Paie) -> String {
    let mut lignes = Vec::new();
    for l in &p.remuneration {
        lignes.push(format!("{} : {}", l.libelle, documents::bulletin::fmt_eur(eur2(l.montant))));
    }
    for l in &p.lignes {
        let m = eur2(l.montant_sal);
        if m != socle::num::Decimal::ZERO {
            lignes.push(format!("{} : -{}", l.libelle, documents::bulletin::fmt_eur(m)));
        }
    }
    lignes.join("\n")
}

/// `documents_fin._ecrire` : le papier de fin de contrat de `modele`, par la fabrique (style
/// « attestation »). `signe` : le certificat s'etablit SIGNE de l'employeur.
fn ecrire_papier(modele: &Path, valeurs: Map<String, Value>, chemin: &Path, ent: &Value, signe: bool) -> R<()> {
    let texte = documents::contrat::lire_modele_texte(modele).map_err(en_echec)?;
    let mut d = documents::contrat::enveloppe("attestation", ent);
    let mut un = Map::new();
    un.insert("texte".into(), json!(texte));
    un.insert("valeurs".into(), Value::Object(valeurs));
    if signe {
        if let Some(s) = documents::bulletin::identite_de(ent, "signature") {
            un.insert("signature_employeur".into(), json!(documents::conges_spectacles::chemin_texte(&s)));
        }
    }
    d.insert("documents".into(), json!([{ "modele": Value::Object(un) }]));
    documents::fabrique::ecrire(&Value::Object(d), chemin).map_err(|m| erreur("FabriqueRefus", m.replace('\n', " ").trim().to_string()))?;
    Ok(())
}

/// `operations_permanents.documents_de_fin` : le certificat de travail et le recu pour solde de
/// tout compte. Deux PAPIERS : rien ne part nulle part.
fn fin_contrat_documents(ctx: &Contexte, charge: &Value) -> R<Value> {
    let (mois, salarie) = (texte_requis(charge, "mois")?, texte_requis(charge, "salarie")?);
    let (saisie, paies) = saisie_du_mois(ctx, mois)?;
    let cle = cle_du_salarie(ctx, salarie)?;
    let (rang, e) = engagement_qui_finit(ctx, &saisie, &cle, salarie, mois)?;
    let p = paie_de_l_engagement(ctx, &paies, rang, &cle, salarie, mois)?;
    let ent = ctx.entreprise_et_gabarits();
    let sortie = ctx.racine.join("sorties").join(mois);
    let date: Jour = jour_le(charge)?.unwrap_or(ctx.aujourdhui);
    let fin_jj = en_texte(&e["fin"]).replace('-', "");
    let poste = ctx.gabarits();

    let numero = p.engagement.numero.clone().unwrap_or_else(|| "?".to_string());
    if p.engagement.fin.is_none() {
        return Err(erreur(
            "ValueError",
            format!(
                "Certificat de travail : le contrat {} n'a pas de date de fin. Il se delivre A L'EXPIRATION du contrat (L1234-19).",
                numero
            ),
        ));
    }
    let nom_cert = format!("CertificatTravail_{}_{}.pdf", cle, fin_jj);
    let modele = socle::fichiers::modele_de("certificat-travail.modele", &ent, &poste);
    ecrire_papier(&modele, marqueurs_communs(&date, &p, &ent)?, &sortie.join(&nom_cert), &ent, true)?;

    let nom_recu = format!("RecuSoldeToutCompte_{}_{}.pdf", cle, fin_jj);
    let mut valeurs = marqueurs_communs(&date, &p, &ent)?;
    valeurs.insert("solde.inventaire".into(), json!(inventaire(&p)));
    valeurs.insert("solde.net".into(), json!(documents::bulletin::fmt_eur(eur2(p.net_a_payer))));
    let modele = socle::fichiers::modele_de("recu-solde-de-tout-compte.modele", &ent, &poste);
    ecrire_papier(&modele, valeurs, &sortie.join(&nom_recu), &ent, false)?;

    Ok(json!({
        "certificat": sortie.join(&nom_cert).to_string_lossy(),
        "recu": sortie.join(&nom_recu).to_string_lossy(),
        "noms": [nom_cert, nom_recu],
        "avertissements": [
            "Le recu pour solde de tout compte s'etablit en DOUBLE EXEMPLAIRE (D1234-7) : un pour le salarie, un pour la compagnie. Il se signe de sa main, avec la mention << pour solde de tout compte >>.",
            "Il peut etre denonce par lettre recommandee dans les SIX MOIS (L1234-20 et D1234-8) ; passe ce delai il est liberatoire pour les seules sommes qui y sont inscrites.",
            "Le certificat de travail ne contient QUE les mentions de D1234-6 : dates d'entree et de sortie, emplois occupes et leurs periodes. Y ajouter autre chose, c'est sortir de ce que la loi autorise."
        ],
    }))
}

// ------------------------------------------------------------------ fin-contrat-signaler

/// `fctu.FinDeContrat` (en dictionnaire) que le signalement declarera, batie sur la saisie
/// (`operations_permanents.fin_de_contrat_declaree`).
fn fin_de_contrat_declaree(ctx: &Contexte, e: &Value, mois: &str) -> R<declarations::fctu::FinDeContrat> {
    let manque = crate::routes::listes::ce_qui_manque(ctx, e)?;
    if !manque.is_empty() {
        let numero = e.get("numero").filter(|n| vrai(n)).map(en_texte).unwrap_or_else(|| "?".to_string());
        let liste: Vec<String> = manque.iter().map(valeur_en_texte).collect();
        return Err(refus(format!(
            "Contrat {} : le signalement de fin de contrat ne peut pas etre produit, la saisie ne dit pas tout.\n  {}",
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

/// `operations_permanents.produire_fctu` : ecrit le signalement dans `sorties/AAAA-MM/`. Rien n'est
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

    let (saisie, paies) = saisie_du_mois(ctx, mois)?;
    let cle = cle_du_salarie(ctx, salarie)?;
    let (rang, e) = engagement_qui_finit(ctx, &saisie, &cle, salarie, mois)?;
    let fin = fin_de_contrat_declaree(ctx, e, mois)?;
    let paie = paie_de_l_engagement(ctx, &paies, rang, &cle, salarie, mois)?;
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
        .map_err(crate::routes::listes::refus_magasin)?
        .into_iter()
        .map(|v| declarations::dsn::Arret { v })
        .collect();
    let texte = declarations::fctu::generer(ctx, &paie, &fin, &options, &arrets).map_err(echec_fctu)?;
    declarations::dsn::ecrire(&texte, &cible).map_err(plain)?;
    crate::depart::noter_piece_ecrite(&ctx.racine, &crate::depart::cle_de_piece(&cible), None)?;
    Ok(json!({
        "fichier": cible.to_string_lossy(),
        "nom": nom,
        "type": type_declaration,
        "ordre": ordre,
        "preavis": declarations::fctu::libelle_preavis(&fin.type_preavis).map_err(plain)?,
        "avertissements": avertissements,
    }))
}

// ------------------------------------------------------------------ arret-signaler

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
        "Aucun arret de {} dont le dernier jour travaille soit le {}. `moteur arret` les liste.",
        cle, lisible
    )))
}

/// Le numero d'ordre (`S20.G00.05.004`) du prochain signalement : un de plus que le plus grand de tous ceux que
/// la compagnie a deja envoyes, arrets, reprises, fins de contrat et amorcages confondus. Cahier technique
/// DSN, p. 136 : « Pour un signalement, ce numero ne doit pas etre remis a zero afin qu'il soit unique quel que
/// soit le mois d'emission » (et non plus le rang parmi les envois de CET arret, qui repartait a 1). LA PLATE-FORME
/// FAIT FOI (lot O1) : `crate::releve::dernier_ordre_connu` la consulte ; ce qu'il faut en dire (elle n'a pas
/// repondu, elle connaissait plus que le journal) part avec les avertissements.
pub(crate) fn prochain_ordre(ctx: &Contexte) -> R<(i64, Vec<String>)> {
    let connu = crate::releve::dernier_ordre_connu(ctx, None)?;
    Ok((connu.dernier + 1, connu.avertissements))
}

/// `operations_arrets.produire` : ecrit le fichier du signalement dans `sorties/AAAA-MM/`. Rien
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
    let texte = signalement::generer(ctx, &dsn_arret, &options).map_err(echec)?;
    declarations::dsn::ecrire(&texte, &cible).map_err(plain)?;
    crate::depart::noter_piece_ecrite(&ctx.racine, &crate::depart::cle_de_piece(&cible), None)?;
    let nomenclatures = crate::routes::listes::nomenclatures(ctx)?;
    Ok(json!({
        "fichier": cible.to_string_lossy(),
        "nom": nom,
        "nature": nature,
        "type": type_declaration,
        "ordre": ordre,
        "avertissements": avertissements,
        "arret": crate::routes::listes::resume_arret(&nomenclatures, &arret)?,
    }))
}

// ------------------------------------------------------------------ dpae

/// La phrase qu'on lit en face d'une embauche deja declaree (`libelle_embauche_declaree`).
fn libelle_embauche_declaree(cle: &str, debut: &str, e: &Value) -> R<String> {
    let quand = {
        let t = e.get("quand").filter(|q| vrai(q)).map(valeur_en_texte).unwrap_or_default();
        let c: Vec<char> = t.chars().collect();
        if c.len() < 10 || c[4] != '-' || c[7] != '-' {
            t
        } else {
            let tr = |a: usize, b: usize| c[a..b].iter().collect::<String>();
            let jour = format!("{}/{}/{}", tr(8, 10), tr(5, 7), tr(0, 4));
            if c.len() >= 16 {
                format!("{} a {}", jour, tr(11, 16))
            } else {
                jour
            }
        }
    };
    let debut_lisible = documents::bulletin::jj_mm_aaaa(&json!(debut)).map_err(|m| erreur("ValueError", m))?;
    if e.get("refusee").map_or(false, vrai) {
        let mut vus: Vec<String> = Vec::new();
        for a in e.get("anomalies").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]) {
            let t = valeur_en_texte(a);
            if !vus.contains(&t) {
                vus.push(t);
            }
        }
        let motif = if vus.is_empty() { "motif non lu".to_string() } else { vus.join(" ; ") };
        return Ok(format!("{} au {} : REFUSEE le {} -- {} -- a corriger et a renvoyer", cle, debut_lisible, quand, motif));
    }
    let suite = if e.get("passee").map_or(false, vrai) {
        "certificat de conformite recu : la DPAE est passee, rien a faire de notre cote"
    } else {
        "retour pas encore arrive : on attend, on ne renvoie pas"
    };
    Ok(format!("{} au {} : declaree le {} -- {}", cle, debut_lisible, quand, suite))
}

/// Les noms `DPAE_<etiquette>_AAAAMMJJ-HHMM.txt` d'un dossier, du plus grand au plus petit
/// (`sorted(sortie.glob("DPAE_%s_????????-????.txt" % etiq), reverse=True)`).
fn fichiers_partiels(dossier: &Path, etiq: &str) -> Vec<PathBuf> {
    let debut = format!("DPAE_{}_", etiq);
    let Ok(lecture) = std::fs::read_dir(dossier) else { return Vec::new() };
    let mut out = Vec::new();
    for e in lecture.flatten() {
        let Some(nom) = e.file_name().to_str().map(str::to_string) else { continue };
        let Some(reste) = nom.strip_prefix(&debut) else { continue };
        let Some(reste) = reste.strip_suffix(".txt") else { continue };
        let c: Vec<char> = reste.chars().collect();
        // `AAAAMMJJ-HHMM`, et `AAAAMMJJ-HHMM-N` : le nom que prend un fichier partiel quand un autre porte deja le sien
        // dans la meme minute (`nom_libre_du_partiel`).
        let rang_ok = c.len() == 13 || (c.len() > 14 && c[13] == '-' && c[14..].iter().all(char::is_ascii_digit));
        if c.len() >= 13 && c[8] == '-' && rang_ok {
            out.push(e.path());
        }
    }
    out.sort();
    out.reverse();
    out
}

/// La marque `_AAAAMMJJ-HHMM` d'un fichier partiel, suivie de `-2`, `-3`... tant que `DPAE_<etiq><marque>.txt` existe deja.
pub(crate) fn nom_libre_du_partiel(dossier: &Path, etiq: &str, marque: &str) -> String {
    let mut essai = marque.to_string();
    let mut rang = 2;
    while dossier.join(format!("DPAE_{}{}.txt", etiq, essai)).exists() {
        essai = format!("{}-{}", marque, rang);
        rang += 1;
    }
    essai
}

fn latin1(octets: &[u8]) -> String {
    octets.iter().map(|&o| o as char).collect()
}

/// `urssaf.pour_entreprise` : le code Urssaf que la DPAE de cette compagnie porte.
fn code_urssaf(ent: &Value) -> R<String> {
    let ecrit = ent.get("code_urssaf").filter(|c| vrai(c)).map(valeur_en_texte).unwrap_or_default();
    let ecrit = ecrit.trim();
    if !ecrit.is_empty() {
        return Ok(ecrit.to_string());
    }
    let territoire = crate::routes::fiches::urssaf_du_code_postal(ent.get("code_postal"))?;
    Ok(if territoire.is_empty() { "117".to_string() } else { territoire })
}

/// `Embauche` de la DPAE, lue d'une paie sans montants (`paie_rust._embauche_dpae`).
fn embauche_dpae(p: &Paie, spectacle: bool) -> Embauche {
    Embauche {
        salarie: p.salarie.clone(),
        debut: json!(p.engagement.debut.iso()),
        fin: p.engagement.fin.map_or(Value::Null, |f| json!(f.iso())),
        nature: p.engagement.nature.clone(),
        spectacle: if spectacle && vrai(&p.spectacle) { p.spectacle.clone() } else { Value::Null },
    }
}

/// Ce qu'une production d'amorcage a laisse : les fichiers ecrits.
struct Amorcage {
    fichiers: Vec<PathBuf>,
    /// Qui est demande : (cle du salarie, ce que la situation de son taux dit).
    lignes: Vec<(String, String)>,
}

/// `operations_amorcage.produire(motif=EMBAUCHE, ...)` : un fichier de signalement par individu dont le taux manque (le travail est
/// celui de `amorcage_demande`, que `moteur pas --demander` generalise). L'echec (quel qu'il soit) est celui que la DPAE ecarte.
fn produire_amorcage(ctx: &Contexte, mois: &str, retenues: &[Paie], constitution: Jour, ecrits: &mut Vec<PathBuf>) -> Result<Amorcage, String> {
    let demande = crate::amorcage_demande::Demande {
        motif: declarations::amorcage::EMBAUCHE,
        mois,
        salaries: None,
        envoi_test: false,
        forcer: false,
        constitution,
    };
    let lot = crate::amorcage_demande::produire(ctx, retenues, &demande, ecrits)?;
    Ok(Amorcage { fichiers: lot.fichiers, lignes: lot.lignes.iter().map(|l| (l.salarie.clone(), l.libelle.clone())).collect() })
}

/// `{"nom", "chemin", "octets"}` d'un fichier (`sortie_json.fichier`).
fn fichier_json(f: &Path) -> Value {
    let octets = std::fs::metadata(f).ok().map(|m| m.len());
    json!({"nom": f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(), "chemin": f.to_string_lossy(), "octets": octets})
}

/// `operations_declarations.produire_dpae` : la DPAE seule, due AVANT le premier jour travaille.
/// Tout le mois, ou une sélection d embauches (`paies`) dans un fichier horodate a cote du complet. `forcer` : reecrire meme
/// une embauche deja declaree (un depot rejete, a refaire).
pub(crate) fn dpae(ctx: &Contexte, charge: &Value) -> R<Value> {
    dpae_detail(ctx, charge).map(|(v, _, _)| v)
}

/// `dpae`, avec ce que la reponse JSON ne porte pas et que la ligne de commande dit : qui est demande dans le signalement
/// d amorcage (`None` : il n y en a pas) et, sinon, pourquoi il est ecarte.
pub(crate) fn dpae_detail(ctx: &Contexte, charge: &Value) -> R<(Value, Option<Vec<(String, String)>>, String)> {
    let forcer = charge.get("forcer").is_some_and(vrai);
    let mois_demande = texte_requis(charge, "mois")?;
    let heure: Value = charge.get("heure").cloned().unwrap_or_else(|| json!("0800"));
    let selection: Option<&Vec<Value>> = match charge.get("paies") {
        None | Some(Value::Null) => None,
        Some(Value::Array(a)) => Some(a),
        Some(autre) => return Err(erreur("TypeError", socle::fichiers::attendu("une liste", socle::valeur::nom_type(autre)))),
    };

    // Les engagements valides du mois (rien ne part d'un engagement en saisie), vus comme des embauches, sans relire leur
    // paie.
    let data = engagement::etats::validees_du_mois(ctx, mois_demande).map_err(plain)?.map(|(d, _)| d)
        .ok_or_else(|| erreur("FileNotFoundError", format!("Aucune saisie pour {}.", mois_demande)))?;
    let paies: Vec<Paie> = {
        let engs = engagement::engagements_de_la_saisie(ctx, &data).map_err(plain)?;
        engs.iter().map(|e| engagement::embauche(ctx, e)).collect::<Result<_, _>>().map_err(plain)?
    };
    let mois = data.get("mois").map(en_texte).ok_or_else(|| erreur("KeyError", socle::fichiers::absent("mois")))?;
    let (etiq, libelle) = mois_en_toutes_lettres(&mois)?;
    let sortie = ctx.racine.join("sorties").join(&mois);
    let aujourdhui = ctx.aujourdhui;
    let cle_de = |p: &Paie| -> (String, String) { (p.salarie.get("_cle").map(valeur_en_texte).unwrap_or_default(), p.engagement.debut.iso()) };

    // Un contrat en cours depuis un mois precedent n'est pas une embauche de ce mois.
    let mut en_cours: Vec<&Paie> = Vec::new();
    let mut premiers: Vec<&Paie> = Vec::new();
    for p in &paies {
        if p.engagement.premier_mois().map_err(plain)? {
            premiers.push(p);
        } else {
            en_cours.push(p);
        }
    }
    let mut retenues: Vec<&Paie> = premiers;
    let mut marque = String::new();
    if let Some(sel) = selection {
        let mut voulues: BTreeSet<(String, String)> = BTreeSet::new();
        for x in sel {
            match x {
                Value::Object(_) => {
                    let g = |k: &str| x.get(k).map(en_texte).ok_or_else(|| erreur("KeyError", socle::fichiers::absent(k)));
                    voulues.insert((g("salarie")?, g("debut")?));
                }
                Value::Array(a) if a.len() == 2 => {
                    voulues.insert((en_texte(&a[0]), en_texte(&a[1])));
                }
                autre => return Err(erreur("TypeError", socle::fichiers::attendu("une liste", socle::valeur::nom_type(autre)))),
            }
        }
        retenues = paies.iter().filter(|p| voulues.contains(&cle_de(p))).collect();
        let trouvees: BTreeSet<(String, String)> = retenues.iter().map(|p| cle_de(p)).collect();
        let anciennes: BTreeSet<(String, String)> = en_cours.iter().map(|p| cle_de(p)).collect();
        let joint = |s: BTreeSet<&(String, String)>| s.into_iter().map(|(a, b)| format!("{} au {}", a, b)).collect::<Vec<_>>().join(", ");
        let deja_parties: BTreeSet<&(String, String)> = voulues.intersection(&anciennes).collect();
        if !deja_parties.is_empty() {
            return Err(refus(format!(
                "Contrat en cours depuis un mois precedent, sa DPAE est deja partie avec son premier mois : {}. Rien n'est ecrit.",
                joint(deja_parties)
            )));
        }
        let absentes: BTreeSet<&(String, String)> = voulues.difference(&trouvees).collect();
        if !absentes.is_empty() {
            return Err(refus(format!("Ces embauches ne sont pas dans la saisie de {} : {}. Rien n'est ecrit.", mois, joint(absentes))));
        }
        if retenues.is_empty() {
            return Err(refus("Aucune embauche retenue : il n'y a rien a declarer."));
        }
        let now = maintenant_iso();
        marque = format!("_{}{}{}-{}{}", &now[0..4], &now[5..7], &now[8..10], &now[11..13], &now[14..16]);
    }
    if retenues.is_empty() {
        return Err(refus(format!(
            "Aucune embauche en {} : {}. Rien n'est ecrit.",
            libelle,
            if en_cours.is_empty() {
                "la saisie est vide"
            } else {
                "les contrats du mois sont en cours depuis un mois precedent, leur DPAE est partie avec lui"
            }
        )));
    }

    // RETOURS WINDOWS, D : DEUX LIGNES IDENTIQUES d'une saisie declareraient deux fois la meme embauche. Le Python ne le
    // voyait pas (une embauche par paie retenue). Decision du 07/10/2026 : la DPAE ne se produit pas ; le refus dit
    // lesquelles (salarie, jour) et qu'il faut corriger la saisie. Ni le numero de contrat ni la validation ne distinguent
    // deux lignes.
    if let Some(brutes) = data.get("engagements").and_then(Value::as_array).filter(|b| b.len() == paies.len()) {
        let sans_numero = |v: &Value| -> Value {
            match v {
                Value::Object(o) => Value::Object(o.iter().filter(|(k, _)| *k != "numero" && *k != "valide").map(|(k, x)| (k.clone(), x.clone())).collect()),
                autre => autre.clone(),
            }
        };
        let retenue = |i: usize| retenues.iter().any(|r| std::ptr::eq(*r, &paies[i]));
        let mut doubles: Vec<String> = Vec::new();
        for i in 1..paies.len() {
            if retenue(i) && (0..i).any(|j| retenue(j) && sans_numero(&brutes[i]) == sans_numero(&brutes[j])) {
                let (c, d) = cle_de(&paies[i]);
                let texte = format!("{} au {}", c, d);
                if !doubles.contains(&texte) {
                    doubles.push(texte);
                }
            }
        }
        if !doubles.is_empty() {
            return Err(refus(format!(
                "Deux lignes identiques dans la saisie de {} declareraient deux fois la meme embauche : {}. Corrigez la saisie (une seule ligne par embauche). Rien n'est ecrit.",
                mois,
                doubles.join(", ")
            )));
        }
    }

    // PAS DEUX FOIS : une embauche deja partie en DPAE reelle ne se reecrit pas.
    let lignes_journal = etats::charger(&ctx.racine)?;
    let etat = etats::etat_des_embauches(&lignes_journal)?;
    let mut deja: HashMap<(String, String), Value> = HashMap::new();
    for entree in etat.as_array().map(Vec::as_slice).unwrap_or(&[]) {
        if let [Value::String(c), Value::String(d), info] = entree.as_array().map(Vec::as_slice).unwrap_or(&[]) {
            deja.insert((c.clone(), d.clone()), info.clone());
        }
    }
    let mut bloquees: Vec<(String, String)> = Vec::new();
    for p in &retenues {
        let k = cle_de(p);
        if let Some(info) = deja.get(&k) {
            if !info.get("refusee").map_or(false, vrai) && !bloquees.contains(&k) {
                bloquees.push(k);
            }
        }
    }
    if !bloquees.is_empty() && !forcer {
        let mut phrases = Vec::new();
        for (c, d) in &bloquees {
            phrases.push(libelle_embauche_declaree(c, d, &deja[&(c.clone(), d.clone())])?);
        }
        return Err(refus(format!("Deja declaree(s), rien n'est ecrit :\n  {}", phrases.join("\n  "))));
    }

    // LE MEME CONTENU, LE MEME FICHIER : une selection deja ecrite se reprend telle quelle.
    let mut ent = ctx.entreprise.clone();
    if let Some(o) = ent.as_object_mut() {
        o.insert("_gabarits".into(), Value::String(ctx.gabarits().to_string_lossy().into_owned()));
    }
    let code = code_urssaf(&ctx.entreprise)?;
    let chemins = Chemins {
        defaut: dossier_du_code().join("modeles").join("due_tel_gabarit.txt"),
        poste: ctx.gabarits(),
    };
    let heure_texte = en_texte(&heure);
    let embauches: Vec<Embauche> = retenues.iter().map(|p| embauche_dpae(p, false)).collect();
    let mut f: Option<PathBuf> = None;
    if !marque.is_empty() {
        let h = horodatage(&maintenant_iso());
        let voulu = declarations::dpae::contenu(&embauches, &parametres_de(&h, &heure_texte, &code, &ent, &chemins)).map_err(plain)?;
        let voulu = latin1(&voulu);
        let mut pareils = Vec::new();
        for x in fichiers_partiels(&sortie, &etiq) {
            let contenu = std::fs::read(&x).map_err(|e| erreur("OSError", format!("{} : {}", x.display(), e)))?;
            if declarations::dpae::meme_declaration(&latin1(&contenu), &voulu) {
                pareils.push(x);
            }
        }
        let mut repetes = Vec::new();
        for x in &pareils {
            let empreinte = envois::journal::empreinte_fichier(x).map_err(plain)?;
            let nom = x.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let r = etats::repetition(&lignes_journal, etats::DPAE, &empreinte, &nom)?;
            if vrai(&r["conforme"]) {
                repetes.push(x.clone());
            }
        }
        if !pareils.is_empty() {
            let choisi = repetes.first().unwrap_or(&pareils[0]).clone();
            let stem = choisi.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            marque = stem.chars().skip(format!("DPAE_{}", etiq).chars().count()).collect();
            f = Some(choisi);
        }
    }
    // JAMAIS PAR-DESSUS UN AUTRE (lot I1 : deux DPAE jouees dans la meme minute s'ecrasaient ; l'ancien Python aussi) : si le
    // nom a la minute est pris par une declaration DIFFERENTE, le fichier prend `-2`, `-3`... ; le nom ordinaire ne change pas.
    if f.is_none() && !marque.is_empty() {
        marque = nom_libre_du_partiel(&sortie, &etiq, &marque);
    }
    let recap = sortie.join(format!("DPAE_{}{}_recapitulatif.txt", etiq, marque));
    let f = match f {
        Some(f) => f,
        None => {
            let cible = sortie.join(format!("DPAE_{}{}.txt", etiq, marque));
            let h = horodatage(&maintenant_iso());
            declarations::dpae::ecrire(&embauches, &cible, &parametres_de(&h, &heure_texte, &code, &ent, &chemins), true).map_err(plain)?;
            crate::depart::noter_piece_ecrite(&ctx.racine, &crate::depart::cle_de_piece(&cible), None)?;
            let pour_recap: Vec<Embauche> = retenues.iter().map(|p| embauche_dpae(p, true)).collect();
            ecrire_texte(&recap, &declarations::dpae::recapitulatif(&pour_recap, &heure).map_err(plain)?)?;
            cible
        }
    };

    let mut lignes = Vec::new();
    for p in &retenues {
        let reste = p.engagement.debut.moins(&aujourdhui);
        let (etat, libelle_ligne) = if reste < 0 {
            ("retard", "EN RETARD (embauche passee)".to_string())
        } else if reste == 0 {
            ("aujourdhui", "AUJOURD'HUI - a deposer avant le debut du travail".to_string())
        } else {
            ("ok", format!("dans {} jour(s) - delai tenu", reste))
        };
        let nom = p.salarie.get("nom").ok_or_else(|| erreur("KeyError", socle::fichiers::absent("nom")))?;
        let prenom = p.salarie.get("prenom_affichage").ok_or_else(|| erreur("KeyError", socle::fichiers::absent("prenom_affichage")))?;
        lignes.push(json!({
            "salarie": format!("{} {}", valeur_en_texte(nom), valeur_en_texte(prenom)),
            "debut": p.engagement.debut.iso(),
            "heure": heure,
            "etat": etat,
            "libelle": libelle_ligne,
            "jours_restants": reste,
        }));
    }
    let retard = lignes.iter().any(|l| l["etat"] == "retard");

    let nom_f = f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let nom_recap = recap.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let mut etapes = vec![json!({
        "cle": "depot",
        "titre": "Depot",
        "detail": format!(
            "Deposer {}.zip sur net-entreprises.fr, rubrique DPAE, AVANT le debut du contrat. Pour la saisie en ligne, recopier {}. Le moteur depose lui-meme : en test, sans rien demander ; en reel, avec le mot de passe du logiciel, tape par l'utilisateur au moment du depot. Le geste : dans la fenetre, l'action « Envoyer les DPAE… » du mois (le test d'abord, puis l'envoi du fichier teste).",
            nom_f, nom_recap
        ),
        "commande": Value::Null,
    })];

    // LE SIGNALEMENT D'AMORCAGE, DANS LE MEME GESTE : il ne fait jamais echouer la DPAE ; ce qui
    // l'empeche est ecarte, et les fichiers deja ecrits restent.
    let retenues_owned: Vec<Paie> = retenues.iter().map(|p| (*p).clone()).collect();
    let mut ecrits = Vec::new();
    let (amorce, ecarte) = match produire_amorcage(ctx, &mois, &retenues_owned, aujourdhui, &mut ecrits) {
        Ok(a) => (Some(a), String::new()),
        Err(m) => (None, m),
    };
    let _ = ecrits;
    let mut fichiers: Vec<Value> = vec![fichier_json(&f), fichier_json(&PathBuf::from(format!("{}.zip", f.to_string_lossy()))), fichier_json(&recap)];
    if let Some(a) = &amorce {
        etapes.push(json!({
            "cle": "amorcage",
            "titre": "Demander les taux de PAS",
            "detail": format!(
                "Deposer aussi {} signalement(s) d'amorcage (nature 08), un par salarie : ils demandent a la DGFiP le taux de prelevement a la source de ces embauches, qui repond sous cinq jours ouvres. Meme site, meme mot de passe, rubrique DSN. Le geste : le depot depuis la fenetre (boite « Deposer la DSN », le test d'abord). Sans eux, la premiere paie partira sur la grille de l'article 204 H, qui preleve le plus souvent PLUS que le taux du salarie.",
                a.fichiers.len()
            ),
            "commande": Value::Null,
        }));
        fichiers.extend(a.fichiers.iter().map(|p| fichier_json(p)));
    }
    let json = json!({
        "mois": mois,
        "libelle": libelle,
        "dossier": sortie.to_string_lossy(),
        "retard": retard,
        "lignes": lignes,
        "fichiers": fichiers,
        "etapes": etapes,
    });
    let amorce_lignes = amorce.as_ref().map(|a| a.lignes.clone());
    Ok((json, amorce_lignes, ecarte))
}

/// Les parametres d'une ecriture de DPAE.
fn parametres_de<'a>(horodatage: &'a str, heure: &str, code: &str, ent: &'a Value, chemins: &'a Chemins) -> Parametres<'a> {
    Parametres { horodatage, heure_embauche: heure.to_string(), code_urssaf: json!(code), ent: Some(ent), chemins }
}

/// `AAAA-MM-JJTHH:MM:SS` -> `JJMMAAAAHHMM` (`strftime("%d%m%Y%H%M")`).
fn horodatage(iso: &str) -> String {
    format!("{}{}{}{}{}", &iso[8..10], &iso[5..7], &iso[0..4], &iso[11..13], &iso[14..16])
}

#[cfg(test)]
mod tests_dpae_partielles {
    use super::*;

    fn dossier() -> PathBuf {
        let r = crate::horloge::racine_d_essai(false).join("sorties").join("2026-10");
        std::fs::create_dir_all(&r).unwrap();
        r
    }

    #[test]
    fn deux_dpae_dans_la_meme_minute_ne_s_ecrasent_jamais() {
        // Defaut de l'ancien Python (lot I1) : le nom `DPAE_<mois>_<AAAAMMJJ-HHMM>.txt` est a la minute, et la seconde
        // declaration ecrasait la premiere ; le depot du premier jour n'etait plus relisible tel qu'il avait ete depose.
        let d = dossier();
        let marque = "_20261007-1815";
        // Cas ordinaire : le nom ne change pas.
        assert_eq!(nom_libre_du_partiel(&d, "Octobre2026", marque), marque);
        std::fs::write(d.join("DPAE_Octobre2026_20261007-1815.txt"), "premiere").unwrap();
        assert_eq!(nom_libre_du_partiel(&d, "Octobre2026", marque), "_20261007-1815-2");
        std::fs::write(d.join("DPAE_Octobre2026_20261007-1815-2.txt"), "seconde").unwrap();
        assert_eq!(nom_libre_du_partiel(&d, "Octobre2026", marque), "_20261007-1815-3");
        // Une autre minute, ou un autre mois, garde son nom.
        assert_eq!(nom_libre_du_partiel(&d, "Octobre2026", "_20261007-1816"), "_20261007-1816");
        assert_eq!(nom_libre_du_partiel(&d, "Novembre2026", marque), marque);
        // Les fichiers deja ecrits sont intacts.
        assert_eq!(std::fs::read_to_string(d.join("DPAE_Octobre2026_20261007-1815.txt")).unwrap(), "premiere");
    }

    #[test]
    fn les_fichiers_partiels_se_reconnaissent_avec_leur_rang() {
        let d = dossier();
        for nom in [
            "DPAE_Octobre2026_20261007-1815.txt",
            "DPAE_Octobre2026_20261007-1815-2.txt",
            "DPAE_Octobre2026_20261007-1815-13.txt",
            "DPAE_Octobre2026.txt",
            "DPAE_Octobre2026_20261007-1815_recapitulatif.txt",
            "DPAE_Octobre2026_20261007-1815-x.txt",
            "DPAE_Octobre2026_20261007-1815-.txt",
            "DPAE_Octobre2026_20261007-1815.txt.zip",
            "DPAE_Novembre2026_20261007-1815.txt",
        ] {
            std::fs::write(d.join(nom), "x").unwrap();
        }
        let noms: Vec<String> = fichiers_partiels(&d, "Octobre2026").iter().map(|p| p.file_name().unwrap().to_string_lossy().into_owned()).collect();
        // Tri decroissant des chemins.
        assert_eq!(noms, vec!["DPAE_Octobre2026_20261007-1815.txt", "DPAE_Octobre2026_20261007-1815-2.txt", "DPAE_Octobre2026_20261007-1815-13.txt"]);
    }
}

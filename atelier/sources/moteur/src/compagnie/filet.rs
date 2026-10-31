//! Le filet de securite DPAE, cote poste : s'inscrire aupres du serveur de rappels, relire si
//! l'adresse est confirmee, effacer le compte, armer un reveil la veille d'une
//! embauche et le desarmer des que la DPAE part.
//!
//! L'ETAT vit dans `config/filet-dpae.json` : le compte (serveur, adresse,
//! cle), les tests faits par embauche, les reveils armes. Une embauche
//! s'identifie par (salarie, premier jour), comme dans le journal des DPAE.
//!
//! CE QUI NE PART JAMAIS D'ICI : une DPAE REELLE. Le test automatique d'une
//! DPAE (produire le fichier et le deposer en TEST chez l'Urssaf)
//! suppose `produire_dpae` et `deposer_dpae` ; ici, `passage` ne fait qu'ecrire dans l'etat les
//! tests qu'il a faits.
//!
//! Un seul fichier d'etat, ecrit en entier : un verrou court
//! (`config/.filet.verrou`) exclut deux lectures-ecritures simultanees ; tenu,
//! l'appel dit de reessayer au lieu d'attendre.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use droit::contexte::Contexte;
use socle::ecriture::ecrire_json;
use crate::compagnie::editeur::Client;
use envois::etats::get;
use socle::valeur::valeur_en_texte;
use socle::echec::R;
use socle::echec::erreur;
use socle::fichiers::vrai;
use socle::verrou::Verrou;

pub const FICHIER: &str = "filet-dpae.json";
const DSN: &str = "dsn";

/// `HEURE_VEILLE` : le reveil sonne la veille de l'embauche, a 9 h ; ou le jour
/// meme a 7 h, si la veille est passee.
const HEURE_VEILLE: &str = "09:00";
const HEURE_JOUR_MEME: &str = "07:00";

fn chemin(racine: &Path) -> PathBuf {
    racine.join("config").join(FICHIER)
}

#[track_caller]
fn en_os(m: String) -> socle::echec::Echec {
    erreur("OSError", m)
}

/// `lire_etat` : l'etat, avec `compte`, `tests` et `reveils` toujours la.
pub(crate) fn lire_etat(racine: &Path) -> R<Value> {
    let mut e = crate::routes::fiches::lire_tolerant(&chemin(racine))?.unwrap_or_else(|| json!({}));
    let Value::Object(m) = &mut e else {
        // Un etat qui n'est pas un objet : un refus, avec sa phrase.
        return Err(erreur("ErreurOperation", format!("{} est illisible : ce n'est pas un objet JSON.\nLe deplacer suffit a repartir de zero (le filet sera a reactiver).", chemin(racine).display())));
    };
    for (cle, defaut) in [("compte", Value::Null), ("tests", json!({})), ("reveils", json!({}))] {
        m.entry(cle).or_insert(defaut);
    }
    Ok(e)
}

fn ecrire_etat(racine: &Path, e: &Value) -> R<()> {
    ecrire_json(&chemin(racine), e, 2).map_err(en_os)
}

/// Rien de change, rien d'ecrit.
fn ecrire_etat_si_change(racine: &Path, e: &Value) -> R<()> {
    if &lire_etat(racine)? != e {
        ecrire_etat(racine, e)?;
    }
    Ok(())
}

/// Le verrou court de l'etat ; tenu par un autre, un refus qui dit de reessayer.
fn verrouiller(racine: &Path) -> R<Verrou> {
    let p = racine.join("config").join(".filet.verrou");
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d).map_err(|e| en_os(format!("{} : {}", d.display(), e)))?;
    }
    let marque = socle::verrou::marque("filet", std::process::id(), &socle::paris::maintenant());
    match Verrou::prendre(&p, &marque, socle::verrou::DELAI_VERROU).map_err(en_os)? {
        Ok(v) => Ok(v),
        Err(_) => Err(erreur("ErreurOperation", "Un autre passage du filet est en cours : reessayer dans un instant. Rien n'a ete modifie.")),
    }
}

fn cle(salarie: &str, debut: &str) -> String {
    format!("{}|{}", salarie, debut)
}

fn cle_dsn(mois: &str) -> String {
    format!("dsn|{}", mois)
}

/// `_maintenant` puis `_iso` : `AAAA-MM-JJTHH:MM`, l'heure de Paris.
fn maintenant() -> String {
    socle::paris::maintenant()[..16].to_string()
}

/// `_client(compte)` : celui de ce compte (sa cle), sans l'identite du poste.
fn client_du_compte(compte: &Value) -> R<Client> {
    let serveur = get(compte, "serveur")?;
    let cle = get(compte, "cle")?;
    Ok(Client::nouveau(&if vrai(serveur) { valeur_en_texte(serveur) } else { String::new() }, &if vrai(cle) { valeur_en_texte(cle) } else { String::new() }))
}

/// `exiger_mot_de_passe` : le mot de passe du logiciel, s'il est pose, pour un
/// acte qui ENGAGE sans rien envoyer. Decide ICI, jamais par la page.
pub(crate) fn exiger_mot_de_passe(racine: &Path, motdepasse: &Value, quoi: &str) -> R<()> {
    let coffre = crate::compagnie::coffre::Coffre::du_poste(racine, "");
    let vers = crate::routes::vers_route;
    if !coffre.ouverture_exigee().map_err(vers)? {
        return Ok(());
    }
    if !vrai(motdepasse) {
        // Rien n'a ete tape pour cette charge : le moteur demande lui-meme a un humain (fenetre, terminal), ou il refuse.
        // Un « oui » ne laisse que l'ouverture du coffre pour la suite de la commande ; la seance ne vaut pas ici.
        let acte = crate::compagnie::demande::Acte::nouveau(quoi, "", "");
        return match crate::compagnie::demande::autoriser_acte(racine, &acte) {
            crate::compagnie::demande::Verdict::Accepte => Ok(()),
            crate::compagnie::demande::Verdict::Refuse(phrase) => Err(erreur(
                "PreuveInsuffisante",
                format!("Un mot de passe du logiciel est pose : il faut le taper pour {}. {}", quoi, phrase.replace("Rien n'est parti", "Rien n'a ete modifie")),
            )),
        };
    }
    let mdp = valeur_en_texte(motdepasse);
    if !coffre.verifier_ouverture(&mdp).map_err(vers)? {
        return Err(erreur("PreuveInsuffisante", "Mot de passe du logiciel refuse. Rien n'a ete modifie."));
    }
    Ok(())
}

// ----------------------------------------------------------------- l'option

/// `etat_filet` : ce que la page montre, relu du disque.
pub(crate) fn etat_filet(racine: &Path) -> R<Value> {
    crate::routes::filet::etat_filet(racine)
}

/// Le serveur du filet de cette compagnie : celui de son compte, sinon celui qu'elle a pose (`serveur` de
/// `config/filet-dpae.json`), sinon celui du moteur (`editeur::SERVEUR_PAR_DEFAUT`). `activer` s'y inscrit,
/// `filet-etat` le montre.
pub fn serveur(etat: &Value) -> String {
    let texte = |v: Option<&Value>| v.filter(|v| vrai(v)).map(valeur_en_texte);
    let compte = etat.get("compte").filter(|c| c.is_object());
    texte(compte.and_then(|c| c.get("serveur")))
        .or_else(|| texte(etat.get("serveur")))
        .unwrap_or_else(|| crate::compagnie::editeur::SERVEUR_PAR_DEFAUT.to_string())
}

/// `activer_filet` : s'inscrire aupres du serveur. Un courriel de confirmation
/// part a l'adresse ; tant qu'elle n'est pas confirmee, aucun rappel ne lui est
/// envoye. Remplace un compte precedent (qui est efface chez le serveur).
pub fn activer(racine: &Path, adresse: &Value, motdepasse: &Value) -> R<Value> {
    // UNE FICTIVE AJOUTEE : le serveur du filet ecrirait a l'adresse (confirmation, rappels) -- un envoi.
    crate::compagnie::bridage::branchement(racine).ouvrir()?;
    exiger_mot_de_passe(racine, motdepasse, "activer le filet de sécurité DPAE")?;
    let adresse = match adresse {
        Value::String(s) => s.trim_matches(|c: char| c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c)).to_lowercase(),
        a if !vrai(a) => String::new(),
        a => return Err(erreur("AttributeError", socle::fichiers::attendu("un texte", socle::valeur::nom_type(a)))),
    };
    if !adresse.contains('@') {
        return Err(erreur("ErreurOperation", "Une adresse courriel, pour recevoir le rappel."));
    }
    let _v = verrouiller(racine)?;
    let mut etat = lire_etat(racine)?;
    let ancien = get(&etat, "compte")?.clone();
    // Le serveur : celui du compte precedent, sinon celui que la compagnie a pose, sinon celui du moteur.
    if vrai(&ancien) {
        // Un compte qui n'est pas un objet : le refus de `get`.
        get(&ancien, "serveur")?;
    }
    let serveur = serveur(&etat);
    // `inscrire` est une action du poste (serveur version 7) : l'identite de ce poste chez ce serveur, demandee s'il ne l'a pas.
    let c = match crate::compagnie::incidents::client_du_poste(racine, &serveur)? {
        Ok(c) => c,
        Err(e) if e.est_filet() => return Err(erreur("ErreurOperation", e.message)),
        Err(e) => return Err(e.en_echec()),
    };
    let r = match c.inscrire(&adresse) {
        Ok(r) => r,
        Err(e) if e.est_filet() => return Err(erreur("ErreurOperation", e.message)),
        Err(e) => return Err(e.en_echec()),
    };
    if vrai(&ancien) && vrai(get(&ancien, "cle")?) {
        match client_du_compte(&ancien)?.desinscrire() {
            Ok(_) => {}
            Err(e) if e.est_filet() => {}
            Err(e) => return Err(e.en_echec()),
        }
    }
    let cle_du_compte = crate::routes::fiches::exiger(&r, "cle")?.clone();
    let confirme = vrai(get(&r, "confirme")?);
    etat["compte"] = json!({
        "serveur": serveur, "adresse": adresse, "cle": cle_du_compte,
        "confirme": confirme, "inscrit_le": maintenant(),
    });
    etat["reveils"] = json!({});
    ecrire_etat(racine, &etat)?;
    drop(_v);
    etat_filet(racine)
}

/// `desactiver_filet` : effacer le compte chez le serveur, et ses reveils avec lui.
pub fn desactiver(racine: &Path, motdepasse: &Value) -> R<Value> {
    exiger_mot_de_passe(racine, motdepasse, "désactiver le filet de sécurité DPAE")?;
    let _v = verrouiller(racine)?;
    let mut etat = lire_etat(racine)?;
    let compte = get(&etat, "compte")?.clone();
    if vrai(&compte) {
        match client_du_compte(&compte)?.desinscrire() {
            Ok(_) => {}
            Err(e) if e.est_filet() => {
                if e.statut != 404 {
                    return Err(erreur(
                        "ErreurOperation",
                        format!("{} -- rien n'est change : reessayer quand le serveur repondra.", e.message),
                    ));
                }
            }
            Err(e) => return Err(e.en_echec()),
        }
    }
    etat["compte"] = Value::Null;
    etat["reveils"] = json!({});
    ecrire_etat(racine, &etat)?;
    drop(_v);
    etat_filet(racine)
}

/// `verifier_filet` : relire chez le serveur si l'adresse est confirmee.
pub fn verifier(racine: &Path) -> R<Value> {
    let _v = verrouiller(racine)?;
    let mut etat = lire_etat(racine)?;
    let compte = get(&etat, "compte")?.clone();
    if !vrai(&compte) {
        drop(_v);
        return etat_filet(racine);
    }
    let Value::Object(_) = &compte else {
        return Err(erreur("AttributeError", socle::fichiers::attendu("un objet", socle::valeur::nom_type(&compte))));
    };
    match client_du_compte(&compte)?.etat() {
        Ok(r) => {
            let confirme = vrai(get(&r, "confirme")?);
            let c = etat["compte"].as_object_mut().unwrap();
            c.insert("confirme".into(), json!(confirme));
            c.shift_remove("erreur");
        }
        Err(e) if e.est_filet() => {
            etat["compte"].as_object_mut().unwrap().insert("erreur".into(), json!(e.message));
        }
        Err(e) => return Err(e.en_echec()),
    }
    ecrire_etat(racine, &etat)?;
    drop(_v);
    etat_filet(racine)
}

// ------------------------------------------------------- les embauches suivies

/// Le journal des depots, ou un refus qui le dit (un fichier qui ne se lit pas en JSON).
fn journal_lisible(racine: &Path) -> R<std::sync::Arc<Vec<Value>>> {
    envois::journal::lu(racine).map(|j| j.lignes).map_err(|e| {
        erreur("ErreurOperation", format!("Le journal des depots (journal/depots.json) est illisible : {e}.\nLe corriger, ou le deplacer, avant de relancer."))
    })
}
/// Les embauches suivies : les DPAE a faire (`etapes::Moteur::embauches_a_declarer`), le journal des depots lisible.
pub fn suivies(ctx: &Contexte) -> R<Vec<Value>> {
    journal_lisible(&ctx.racine)?;
    let a_faire = crate::routes::etapes::Moteur::nouveau(ctx).embauches_a_declarer(None)?;
    Ok(a_faire.as_array().cloned().unwrap_or_default())
}

/// `echeance_du_reveil` : la veille a 9 h ; passee, le jour meme a 7 h ; passe
/// aussi, rien -- l'embauche a commence, le reveil n'a plus d'objet.
/// `AAAA-MM-JJTHH:MM`, qui se compare comme un instant.
pub fn echeance_du_reveil(debut: &str, maintenant: &str) -> R<Option<String>> {
    let jour = socle::date::Jour::lire(debut).map_err(|m| erreur("ValueError", m))?;
    let veille = jour.plus_jours(-1).map_err(|m| erreur("ValueError", m))?;
    for t in [format!("{}T{}", veille.iso(), HEURE_VEILLE), format!("{}T{}", jour.iso(), HEURE_JOUR_MEME)] {
        if t.as_str() > maintenant {
            return Ok(Some(t));
        }
    }
    Ok(None)
}

// ------------------------------------------------------------- le passage

/// Ce que `_reveils` a fait.
#[derive(Default)]
pub struct Reveils {
    pub armes: Vec<String>,
    pub desarmes: Vec<String>,
    pub incidents: Vec<String>,
}

/// (mois, jour d'echeance ISO) des DSN dont la boite des
/// rappels parle -- aucun depot REEL accepte, ou un depot refuse.
fn dsn_a_rappeler(ctx: &Contexte) -> R<Vec<(String, String)>> {
    use crate::routes::rappels::{mois_a_declarer, Etat};
    let st = Etat { ctx, aujourdhui: ctx.aujourdhui, journal: journal_lisible(&ctx.racine)? };
    let mut out = Vec::new();
    for m in mois_a_declarer(&st).map_err(socle::echec::Echec::from)? {
        // Le jour de depot, reporte au jour ouvre (`echeance_dsn`) : le reveil sonne la veille et le jour meme, avant midi.
        let jour = declarations::echeance_dsn::de_la_compagnie(ctx, &m).map_err(socle::echec::Echec::from)?.jour;
        out.push((m, jour.iso()));
    }
    Ok(out)
}

fn nouveau_code() -> R<String> {
    let mut alea = [0u8; 16];
    getrandom::fill(&mut alea).map_err(|e| erreur("OSError", e.to_string()))?;
    Ok(alea.iter().map(|o| format!("{:02x}", o)).collect())
}

/// Armer ce qui doit sonner, desarmer ce qui ne doit plus. Une
/// panne de reseau arrete le passage (on ne martele pas un serveur absent) ; les
/// reveils restent notes tels quels, et le passage suivant reprend.
fn reveils(ctx: &Contexte, etat: &mut Value, suivies: &[Value], maintenant: &str, bilan: &mut Reveils) -> R<()> {
    let compte = get(etat, "compte")?.clone();
    let c = client_du_compte(&compte)?;
    // Le travail, avec ses pannes d'`ErreurFilet` rangees a part.
    let mut panne: Option<crate::compagnie::editeur::ErreurFilet> = None;
    let resultat: R<()> = (|| {
        if !vrai(get(&compte, "confirme")?) {
            match c.etat() {
                Ok(r) => {
                    etat["compte"]["confirme"] = json!(vrai(get(&r, "confirme")?));
                }
                Err(e) if e.est_filet() => {
                    panne = Some(e);
                    return Ok(());
                }
                Err(e) => return Err(e.en_echec()),
            }
        }
        // Ce qui doit sonner : (cle, jour de l'objet, echeance, objet).
        let mut voulus: Vec<(String, String, String, &'static str)> = Vec::new();
        let mut poser = |k: String, jour: String, quand: String, objet: &'static str| {
            match voulus.iter().position(|v| v.0 == k) {
                Some(i) => voulus[i] = (k, jour, quand, objet),
                None => voulus.push((k, jour, quand, objet)),
            }
        };
        for e in suivies {
            let debut = valeur_en_texte(get(e, "debut")?);
            if let Some(quand) = echeance_du_reveil(&debut, maintenant)? {
                poser(cle(&valeur_en_texte(get(e, "salarie")?), &debut), debut, quand, "dpae");
            }
        }
        // LA DSN PAS PARTIE : le meme reveil, la veille de l'echeance a 9 h.
        for (m, jour) in dsn_a_rappeler(ctx)? {
            if let Some(quand) = echeance_du_reveil(&jour, maintenant)? {
                poser(cle_dsn(&m), jour, quand, DSN);
            }
        }
        for (k, jour, quand, objet) in &voulus {
            let deja = etat["reveils"].get(k).cloned();
            if let Some(d) = &deja {
                if get(d, "echeance")?.as_str() == Some(quand.as_str()) {
                    continue;
                }
            }
            let code = match &deja {
                Some(d) if vrai(get(d, "code")?) => valeur_en_texte(get(d, "code")?),
                _ => nouveau_code()?,
            };
            if let Err(e) = c.armer(&code, quand, jour, objet) {
                if e.est_filet() {
                    panne = Some(e);
                    return Ok(());
                }
                return Err(e.en_echec());
            }
            etat["reveils"].as_object_mut().ok_or_else(|| erreur("TypeError", "'reveils' n'est pas un objet"))?.insert(
                k.clone(),
                json!({"code": code, "echeance": quand, "arme_le": maintenant}),
            );
            bilan.armes.push(k.clone());
        }
        let a_desarmer: Vec<String> = match etat["reveils"].as_object() {
            Some(o) => o.keys().filter(|k| !voulus.iter().any(|v| &v.0 == *k)).cloned().collect(),
            None => Vec::new(),
        };
        for k in a_desarmer {
            let code = match etat["reveils"].get(&k).and_then(|r| r.get("code")) {
                Some(c) => valeur_en_texte(c),
                None => return Err(erreur("KeyError", socle::fichiers::absent("code"))),
            };
            if let Err(e) = c.desarmer(&code) {
                if e.est_filet() {
                    panne = Some(e);
                    return Ok(());
                }
                return Err(e.en_echec());
            }
            etat["reveils"].as_object_mut().unwrap().shift_remove(&k);
            bilan.desarmes.push(k);
        }
        if let Some(c) = etat["compte"].as_object_mut() {
            c.shift_remove("erreur");
        }
        Ok(())
    })();
    resultat?;
    if let Some(e) = panne {
        if e.statut == 404 {
            // Le compte n'existe plus chez le serveur : les reveils notes ne
            // valent plus rien. On le dit, sans boucler.
            if let Some(c) = etat["compte"].as_object_mut() {
                c.insert("erreur".into(), json!(e.message));
                c.insert("confirme".into(), json!(false));
            }
            etat["reveils"] = json!({});
        }
        bilan.incidents.push(e.message);
    }
    Ok(())
}

/// Un passage du filet, sans le test automatique des DPAE (qui reste aux actes) :
/// `tests` (cle -> entree) s'ajoute a l'etat, `vivantes` (cles des embauches
/// encore suivies) retire les tests des autres, puis les reveils s'arment et
/// se desarment. L'etat n'est ecrit que s'il a change.
pub fn passage(ctx: &Contexte, tests: &Value, vivantes: &Value, maintenant_force: &Value) -> R<Value> {
    let racine = &ctx.racine;
    // `maintenant` : l'heure du passage (AAAA-MM-JJTHH:MM), que l'appelant peut fixer.
    let maintenant = match maintenant_force.as_str() {
        Some(m) if !m.is_empty() => m.to_string(),
        _ => maintenant(),
    };
    let _v = verrouiller(racine)?;
    let mut etat = lire_etat(racine)?;
    if let Value::Object(neufs) = tests {
        let t = etat["tests"].as_object_mut().ok_or_else(|| erreur("TypeError", "'tests' n'est pas un objet"))?;
        for (k, v) in neufs {
            t.insert(k.clone(), v.clone());
        }
    }
    if let Value::Array(v) = vivantes {
        let gardees: Vec<String> = v.iter().map(valeur_en_texte).collect();
        if let Some(t) = etat["tests"].as_object_mut() {
            let a_retirer: Vec<String> = t.keys().filter(|k| !gardees.contains(k)).cloned().collect();
            for k in a_retirer {
                t.shift_remove(&k);
            }
        }
    }
    let mut bilan = Reveils::default();
    if vrai(get(&etat, "compte")?) {
        let suivies = suivies(ctx)?;
        reveils(ctx, &mut etat, &suivies, &maintenant, &mut bilan)?;
    }
    ecrire_etat_si_change(racine, &etat)?;
    Ok(json!({"armes": bilan.armes, "desarmes": bilan.desarmes, "incidents": bilan.incidents}))
}

/// `desarmer` : desarme tout de suite le reveil d'une embauche (ou d'un mois de
/// DSN) -- apres son depot reel. Vrai si le serveur l'a efface ; une panne laisse
/// le reveil note, et le passage suivant le desarmera.
pub fn desarmer(racine: &Path, k: &str) -> R<bool> {
    let _v = verrouiller(racine)?;
    let mut etat = lire_etat(racine)?;
    let compte = get(&etat, "compte")?.clone();
    let r = match etat["reveils"].get(k) {
        Some(r) if vrai(r) => r.clone(),
        _ => return Ok(false),
    };
    if !vrai(&compte) {
        return Ok(false);
    }
    let code = r.get("code").map(valeur_en_texte).ok_or_else(|| erreur("KeyError", socle::fichiers::absent("code")))?;
    match client_du_compte(&compte)?.desarmer(&code) {
        Ok(_) => {}
        Err(e) if e.est_filet() => return Ok(false),
        Err(e) => return Err(e.en_echec()),
    }
    if let Some(o) = etat["reveils"].as_object_mut() {
        o.shift_remove(k);
    }
    ecrire_etat(racine, &etat)?;
    Ok(true)
}

/// La cle d'un reveil de DPAE ou de DSN.
pub fn cle_du_reveil(charge: &Value) -> String {
    match charge.get("cle").filter(|c| vrai(c)) {
        Some(c) => valeur_en_texte(c),
        None => cle(
            &charge.get("salarie").map(valeur_en_texte).unwrap_or_else(|| "None".into()),
            &charge.get("debut").map(valeur_en_texte).unwrap_or_else(|| "None".into()),
        ),
    }
}


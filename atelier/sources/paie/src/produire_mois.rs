//! La route `produire` (`moteur/web.py`, `operations_paies.fermer_mois`, `manques_pour_fermer`,
//! `controler_calcul_stocke`, `produire_mois`, `apercu_mois`, `figer_numeros`,
//! `operations_remplacement.forme_a_produire` et `noter_dsn_ecrite`, `sortie_json.mois_produit`) -- lot H2.
//!
//! ECRIT, NON PROUVE. Fermer le mois : on ne ferme qu'un mois pret (la DPAE de chaque embauche partie pour de
//! vrai, son contrat parti a la signature) ; on y fabrique bulletins, certificats et DSN -- jamais les contrats
//! ni les DPAE, deja partis (`sans_contrat=True`). Puis la saisie s'enregistre (`routes_ecritures::ecrire_saisie`,
//! ce que fait la route `enregistrer-saisie`). Le filet DPAE en fond n'est PAS ici : il se lance par la route
//! `filet-preparer`, au branchement. Aucun fil dans le moteur.
//!
//! Tout le mois, ou rien : les compteurs ne sont ecrits qu'une fois le dernier fichier ecrit (`Reserves::valider`),
//! et le verrou des compteurs tient pendant toute la fabrication.
//!
//! DEUX FACONS DE FAIRE UNE PAIE, COMME LE PYTHON. Ce que l'ecran montre (`vue_paie`, les totaux, le bordereau
//! affiche, les certificats) sort du calcul du mois (`paie_de_json`) ; ce que le bulletin et la DSN IMPRIMENT, le
//! Python le faisait recalculer par le moteur depuis la saisie : ici `engagement::charger_donnees`, aux numeros de
//! la paie du calcul.
//!
//! NUMEROTATION CONTINUE (Leandre, 06/10/2026). L'ancien Python reservait les numeros DEUX fois : `fermer_mois`
//! charge la saisie (numerotee) pour controler le mois, puis `produire_mois` la recharge (`apercu_mois`), et
//! `Contexte.prochain` lit d'abord les numeros deja reserves : les bulletins et les contrats neufs d'un mois
//! produit etaient decales du nombre de paies, avec des trous dans la suite. Ici le controle numerote dans des
//! reserves jetees ; la production repart des compteurs du disque et ne reserve qu'une fois.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::calcul::Paie;
use crate::conges_spectacles::{self, PaieCs};
use crate::contexte::Contexte;
use crate::depot_acte::avec_donnee;
use crate::ecriture::{ecrire_json, json_py};
use crate::engagement::{self, Avis};
use crate::etats::{erreur, py_str, R};
use crate::fichiers::{lire_json, vrai};
use crate::jumeaux;
use crate::produire_regularisations::{self as regularisations, Reg};
use crate::retours::Echec;
use crate::routes_mois::{Moteur, Paire};
use crate::routes_ouverture::q2;

/// Le rang d'essai de la DSN : en test ou en reel, `fermer_mois` ne pose jamais autre chose que 1.
const ESSAI_DSN: i64 = 1;

fn info(texte: impl Into<String>) -> Avis {
    Avis { info: true, texte: texte.into() }
}

fn avertissement(texte: impl Into<String>) -> Avis {
    Avis { info: false, texte: texte.into() }
}

fn os(chemin: &Path, e: std::io::Error) -> Echec {
    erreur("OSError", format!("{} : {}", chemin.display(), e))
}

fn valeur(m: String) -> Echec {
    erreur("ValueError", m)
}

fn cle_de(p: &Paie) -> String {
    p.salarie.get("_cle").map(py_str).unwrap_or_default()
}

fn prenom_de(sal: &Value) -> String {
    match sal.get("prenom_affichage") {
        Some(v) if vrai(v) => py_str(v),
        _ => py_str(sal.get("prenom").unwrap_or(&Value::Null)),
    }
}

// ------------------------------------------------------------- les numeros reserves

/// `Contexte._compteurs_en_attente` : les numeros reserves par ce processus, dans leur ordre d'arrivee. Rien n'est
/// ecrit avant `valider`.
#[derive(Default)]
pub(crate) struct Reserves {
    valeurs: HashMap<(String, String), i64>,
    ordre: Vec<(String, String)>,
}

impl Reserves {
    /// `Contexte.prochain` : le suivant de ce qui est reserve, sinon de ce que `compteurs.json` porte.
    pub(crate) fn prochain(&self, ctx: &Contexte, cle: &str, annee: i64) -> R<i64> {
        let an = annee.to_string();
        if let Some(v) = self.valeurs.get(&(an.clone(), cle.to_string())) {
            return Ok(v + 1);
        }
        let p = ctx.racine.join("config").join("compteurs.json");
        let c = if p.exists() { lire_json(&p).map_err(valeur)? } else { Value::Null };
        let lu = match c.get(&an).and_then(|a| a.get(cle)) {
            None => 0,
            Some(Value::Number(n)) => n.as_i64().or_else(|| n.as_f64().map(|f| f as i64)).unwrap_or(0),
            Some(Value::String(s)) => s.trim().parse().map_err(|_| valeur(format!("nombre entier illisible : {}", crate::fichiers::repr(s))))?,
            Some(Value::Bool(b)) => *b as i64,
            Some(_) => 0,
        };
        Ok(lu + 1)
    }

    /// `Contexte.enregistrer_compteur` : reserve, sans ecrire.
    pub(crate) fn enregistrer(&mut self, annee: i64, cle: &str, v: i64) {
        let k = (annee.to_string(), cle.to_string());
        if !self.valeurs.contains_key(&k) {
            self.ordre.push(k.clone());
        }
        self.valeurs.insert(k, v);
    }

    /// `engagement.numeroter` : les numeros de contrat et de bulletin des paies, reserves.
    pub(crate) fn numeroter(&mut self, ctx: &Contexte, paies: &mut [Paie], annee: i32) -> R<()> {
        crate::routes_ouverture::numeroter_en_attente(ctx, paies, annee, &mut self.valeurs)?;
        // L'ordre d'insertion d'un dictionnaire Python : `bulletin` d'abord, puis les contrats dans l'ordre des paies.
        let an = annee.to_string();
        let rang = |c: &str| -> usize {
            if c == "bulletin" {
                0
            } else {
                paies.iter().position(|p| format!("contrat_{}", cle_de(p)) == c).map_or(usize::MAX, |i| i + 1)
            }
        };
        let mut nouveaux: Vec<String> = self.valeurs.keys().filter(|k| k.0 == an && !self.ordre.contains(k)).map(|k| k.1.clone()).collect();
        nouveaux.sort_by_key(|c| (rang(c), c.clone()));
        for c in nouveaux {
            self.ordre.push((an.clone(), c));
        }
        Ok(())
    }

    /// `Contexte.valider_compteurs` : ecrit les numeros reserves (JSON a deux espaces, sans saut de ligne final).
    pub(crate) fn valider(&self, ctx: &Contexte) -> R<()> {
        if self.ordre.is_empty() {
            return Ok(());
        }
        let p = ctx.racine.join("config").join("compteurs.json");
        let mut c = if p.exists() { lire_json(&p).map_err(valeur)? } else { json!({}) };
        let lu = crate::etats::nom_type(&c);
        let racine = c.as_object_mut().ok_or_else(|| erreur("AttributeError", format!("config/compteurs.json : {}", crate::fichiers::attendu("un objet", lu))))?;
        for (an, cle) in &self.ordre {
            let annee = racine.entry(an.clone()).or_insert_with(|| json!({}));
            let annee = annee.as_object_mut().ok_or_else(|| erreur("AttributeError", "l'annee des compteurs n'est pas un dictionnaire"))?;
            annee.insert(cle.clone(), json!(self.valeurs[&(an.clone(), cle.clone())]));
        }
        std::fs::write(&p, json_py(&c, 2)).map_err(|e| os(&p, e))
    }
}

// ------------------------------------------------------------- charger

/// `paie_rust.calculer` : la reponse du calcul pour la saisie en memoire. Un refus garde le type d'une saisie
/// encore incomplete (`SaisieIncomplete`).
pub(crate) fn calculer(o: &HashMap<String, String>, ctx: &Contexte, data: &Value) -> R<Value> {
    let (rep, _entrees) = crate::calcul_du_mois::calculer_suivi(o, &json!({"saisie": data})).map_err(|m| Echec::from(crate::routes_ouverture::refus_du_calcul(ctx, data, m)))?;
    Ok(rep)
}

/// `charger` : les paies du calcul, numerotees (numeros RESERVES, rien d'ecrit) ; sans numeros, `reserves` reste
/// intact.
pub(crate) fn charger(ctx: &Contexte, data: &Value, rep: &Value, reserves: Option<&mut Reserves>) -> R<Vec<Paie>> {
    let engs = crate::routes_ouverture::engagements_calcules(ctx, data, rep)?;
    let paies_json = rep.get("paies").and_then(Value::as_array).ok_or_else(|| erreur("KeyError", crate::fichiers::absent("paies")))?;
    let mut paies = Vec::new();
    for (e, v) in engs.into_iter().zip(paies_json) {
        paies.push(crate::routes_ouverture::paie_de_json(ctx, e, v)?);
    }
    if let Some(r) = reserves {
        let mois = py_str(data.get("mois").ok_or_else(|| erreur("KeyError", crate::fichiers::absent("mois")))?);
        let annee: i32 = mois.get(..4).and_then(|a| a.parse().ok()).ok_or_else(|| valeur(format!("nombre entier illisible : {}", crate::fichiers::repr(&mois))))?;
        r.numeroter(ctx, &mut paies, annee)?;
    }
    Ok(paies)
}

/// Ce que le bulletin et la DSN impriment : les paies refaites depuis la saisie (comme le moteur les refait), aux
/// numeros des paies du calcul.
fn paies_a_imprimer(ctx: &Contexte, data: &Value, paies: &[Paie]) -> R<Vec<Paie>> {
    let s = engagement::charger_donnees(ctx, data).map_err(|m| Echec::from(crate::routes_ouverture::refus_du_calcul(ctx, data, m)))?;
    if s.paies.len() != paies.len() {
        return Err(valeur("le nombre de paies n'est pas celui du calcul".into()));
    }
    let mut out = s.paies;
    for (p, q) in out.iter_mut().zip(paies) {
        p.numero_bulletin = q.numero_bulletin.clone();
        p.engagement.numero = q.engagement.numero.clone();
    }
    Ok(out)
}

// ------------------------------------------------------------- fermer le mois

/// `manques_pour_fermer` : ce qui empeche de fermer le mois, embauche par embauche ; vide = pret.
///
/// DEUX CONDITIONS : la DPAE de chaque embauche est partie, POUR DE VRAI (un depot reel accepte, pas refuse) ; son
/// contrat est parti a la signature (ou revenu signe) -- un salarie au mois n'en a pas ici. Pas de condition de
/// date. Seuls les contrats qui COMMENCENT ce mois-ci ont une DPAE et un contrat a envoyer.
fn manques_pour_fermer(m: &mut Moteur, paies: &[Paie]) -> R<Vec<Value>> {
    let g = m.globales()?;
    let lignes = m.lignes()?;
    // Le contrat, pas le salarie : le jumeau parti ne dispense pas celui qui n'est pas parti (Retours Windows, B, suite).
    let cles_paies = crate::etats::cles_des_paies(&lignes, &crate::routes_mois::contrats_des_paies(paies))?;
    let mut manques = Vec::new();
    for (i, p) in paies.iter().enumerate() {
        let e = &p.engagement;
        let cle: Paire = (cle_de(p), e.debut.iso());
        let nom = format!(
            "{} {}",
            match p.salarie.get("prenom_affichage") {
                Some(v) if vrai(v) => py_str(v),
                _ => p.salarie.get("prenom").map(py_str).unwrap_or_default(),
            },
            p.salarie.get("nom").map(py_str).unwrap_or_default()
        );
        let nom = nom.trim().to_string();
        let mut manque = |quoi: &str, phrase: String| {
            manques.push(json!({"salarie": cle.0, "nom": nom, "debut": cle.1, "quoi": quoi, "phrase": phrase}));
        };
        if e.premier_mois()? {
            match g.dpae.get(&cle) {
                None => manque("dpae", format!("{} (contrat du {}) : DPAE pas encore deposee.", nom, e.debut.jj_mm_aaaa())),
                Some(etat) if etat.get("refusee").is_some_and(vrai) => {
                    manque("dpae", format!("{} (contrat du {}) : DPAE refusee par l'Urssaf, a corriger et renvoyer.", nom, e.debut.jj_mm_aaaa()))
                }
                Some(_) => {}
            }
            if !e.nat()?.mensuelle() && crate::routes_mois::premier(&g.contrats, &cles_paies[i]).is_none() && crate::routes_mois::premier(&g.recus, &cles_paies[i]).is_none() {
                manque("contrat", format!("{} (contrat du {}) : contrat pas encore envoye a la signature.", nom, e.debut.jj_mm_aaaa()));
            }
        }
    }
    Ok(manques)
}

/// `controler_calcul_stocke` : ce qui part se recalcule toujours ; `paies/AAAA-MM.calcul.json` ne sert qu'a
/// l'affichage. Au moment de fermer, on compare : memes entrees et autre resultat, c'est un fichier modifie a la
/// main ou une entree du calcul qui echappe a l'empreinte, et cela remonte en incident. Seulement quand la saisie
/// fermee est celle du disque ; ne leve jamais.
fn controler_calcul_stocke(o: &HashMap<String, String>, ctx: &Contexte, mois: &str, data: &Value, rep: &Value) {
    let p = ctx.racine.join("paies").join(format!("{}.json", mois));
    if !p.is_file() {
        return;
    }
    let Ok(sur_le_disque) = lire_json(&p) else { return };
    if !crate::etats::eq_py(&sur_le_disque, data) {
        return;
    }
    let demande = json!({"mois": mois, "resultat": rep}).to_string();
    let Ok(constat) = crate::calcul_du_mois::comparer(o, &demande) else { return };
    if constat.get("constat").and_then(Value::as_str) == Some("ecart") {
        let resume = format!("La paie stockee du mois {} differe du calcul a la fermeture, pour les memes entrees.", mois);
        let _ = crate::incidents::noter(
            &ctx.racine,
            &crate::incidents::Note {
                nature: crate::incidents::LOGICIEL,
                resume: &resume,
                ou: "paies/calcul.json",
                quoi: "calcul-stocke-different",
                detail: json!({"mois": mois}),
                ..Default::default()
            },
        );
    }
}

// ------------------------------------------------------------- l'annule-et-remplace

/// `forme_a_produire` : ce que doit etre la prochaine DSN REELLE de ce mois : `(type, ordre, remplace)`. Normale
/// d'ordre 1 si rien n'est passe ; sinon annule-et-remplace au rang suivant -- et un refus si son delai est passe.
fn forme_a_produire(ctx: &Contexte, m: &mut Moteur, mois: &str, nic: &Value) -> R<(String, i64, bool)> {
    let faits = m.depots_reels(mois, nic)?;
    let mut dernier: Option<i64> = None;
    for l in &faits {
        let n = m.ordre_de_la_ligne(mois, l)?;
        dernier = Some(dernier.map_or(n, |d| d.max(n)));
    }
    // CE QUE LA PLATE-FORME A VU EN REEL ce mois civil, lu en silence dans les bilans archives
    // (`ordre_signalements::dernier_ordre_des_mensuelles`) : le rang se compte « au cours du mois courant » (cahier
    // technique DSN, p. 136), toutes declarations du SIREN confondues, d'ou qu'elles soient parties (un autre poste).
    let constitution: String = ctx.aujourdhui.iso().chars().take(7).collect();
    let vues = crate::ordre_signalements::dernier_ordre_des_mensuelles(&ctx.racine, &crate::ordre_signalements::siren_de(ctx), false, &constitution, mois)?;
    let dernier = match dernier {
        Some(d) => d.max(vues.ordre),
        // Une mensuelle du mois acceptee en reel que le journal ignore : la suivante la remplace.
        None if vues.du_mois_acceptee => vues.ordre.max(1),
        None => return Ok(("01".into(), vues.ordre + 1, false)),
    };
    let limite = m.limite(mois)?;
    if ctx.aujourdhui > limite {
        return Err(erreur(
            "AnnuleRemplaceHorsDelai",
            format!(
                "La DSN de {} est deja partie, et l'annule-et-remplace n'est plus admis depuis le {} a minuit (veille de l'echeance, cahier technique DSN §1.4.2). La correction se porte dans la DSN suivante : Historique › Corriger ce mois (`paie corriger {}`). Rien n'est ecrit.",
                mois,
                limite.jj_mm_aaaa(),
                mois
            ),
        ));
    }
    Ok(("03".into(), dernier + 1, true))
}

/// La declaration qu'un annule-et-remplace remplace (le dernier depot reel accepte du mois, au plus grand ordre)
/// portait-elle un versement du prelevement a la source (S21.G00.20.002 = DGFIP_PAS) ? Elle se relit dans
/// `sorties/<mois>/` ; introuvable, `None` : `dsn::generer` refuse alors s'il n'y a rien a reverser (norme p. 157).
fn remplacee_versait_pas_de(ctx: &Contexte, m: &mut Moteur, mois: &str, nic: &Value) -> R<Option<bool>> {
    let faits = m.depots_reels(mois, nic)?;
    let mut dernier: Option<(i64, Value)> = None;
    for l in faits {
        let n = m.ordre_de_la_ligne(mois, &l)?;
        if dernier.as_ref().map_or(true, |(d, _)| n > *d) {
            dernier = Some((n, l));
        }
    }
    let Some((_, l)) = dernier else { return Ok(None) };
    let nom = l.get("fichier").filter(|v| vrai(v)).map(py_str).unwrap_or_default();
    if nom.is_empty() {
        return Ok(None);
    }
    let Ok(octets) = std::fs::read(ctx.racine.join("sorties").join(mois).join(&nom)) else { return Ok(None) };
    let cherche = format!("S21.G00.20.002,'{}'", crate::dsn::DGFIP_PAS);
    Ok(Some(octets.windows(cherche.len()).any(|w| w == cherche.as_bytes())))
}

/// `noter_dsn_ecrite` : la paie que ce fichier declare, numeros figes (`journal/dsn-ecrites.json`). L'ecran dira si
/// elle change avant l'envoi.
fn noter_dsn_ecrite(m: &Moteur, ctx: &Contexte, mois: &str, fichier: &Path) -> R<()> {
    let p = ctx.racine.join("journal").join("dsn-ecrites.json");
    let mut d: Value = if p.exists() {
        let t = std::fs::read_to_string(&p).map_err(|e| os(&p, e))?;
        serde_json::from_str(&t).ok().filter(Value::is_object).unwrap_or_else(|| json!({}))
    } else {
        json!({})
    };
    let nom = fichier.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let empreinte = m.empreinte_saisie(mois)?;
    d[format!("{}/{}", mois, nom)] = json!({"empreinte_saisie": empreinte, "quand": crate::incidents::maintenant()});
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).map_err(|e| os(parent, e))?;
    }
    ecrire_json(&p, &d, 2).map_err(|m| erreur("OSError", m))
}

// ------------------------------------------------------------- figer les numeros

/// Ce que `figer_numeros` ecrira : le fichier de saisie relu, et pour chaque engagement du fichier qui n'a pas
/// encore de numero, celui que SA paie a recu.
struct NumerosAFiger {
    chemin: PathBuf,
    data: Value,
    poser: Vec<(usize, String)>,
}

/// La premiere moitie de `figer_numeros`, AVANT d'ecrire quoi que ce soit : quel engagement du fichier recevra quel
/// numero. `saisie` est la saisie produite (une paie par engagement, dans son ordre), `paies` ses paies numerotees.
///
/// LE CONTRAT, PAS LE SALARIE DU JOUR : chaque paie retrouve son engagement dans le fichier par `jumeaux::apparier`
/// (le numero pose, sinon le rang parmi ceux du meme salarie et du meme jour) -- deux contrats qui commencent le
/// meme jour gardent chacun le leur. La route `produire` produit depuis ce fichier ; l'horloge aussi. S'il differait de la saisie produite
/// (un appel qui produit depuis la memoire), on ne toucherait qu'aux numeros. UN NUMERO DEJA POSE ET DIFFERENT N'EST JAMAIS ECRASE (regle 4) : c'est un
/// refus, avant la premiere ecriture. `None` si le fichier n'existe pas.
fn numeros_a_figer(ctx: &Contexte, mois: &str, saisie: &Value, paies: &[Paie]) -> R<Option<NumerosAFiger>> {
    let chemin = ctx.racine.join("paies").join(format!("{}.json", mois));
    if !chemin.exists() {
        return Ok(None);
    }
    let data = engagement::lire_saisie(&chemin).map_err(valeur)?;
    let produits = saisie.get("engagements").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]);
    let des_paies: Vec<Option<jumeaux::Ident>> =
        paies.iter().enumerate().map(|(i, p)| Some((cle_de(p), p.engagement.debut.iso(), produits.get(i).and_then(jumeaux::numero_pose)))).collect();
    let (taux, spectacle) = (data.get("taux_cachet").cloned().unwrap_or(Value::Null), data.get("spectacle").cloned().unwrap_or(Value::Null));
    let fichier = data.get("engagements").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]);
    let du_fichier: Vec<Option<jumeaux::Ident>> = fichier
        .iter()
        .map(|e| {
            // Un engagement qui ne se calcule pas seul est laisse tel quel (`except Exception: continue`).
            let fiche = ctx.salarie(e.get("salarie").and_then(Value::as_str)?).ok()?;
            let cle = py_str(fiche.get("_cle").unwrap_or(&Value::Null));
            let seule = json!({"mois": mois, "engagements": [e.clone()], "taux_cachet": taux, "spectacle": spectacle});
            let s = engagement::charger_donnees(ctx, &seule).ok()?;
            Some((cle, s.paies.first()?.engagement.debut.iso(), jumeaux::numero_pose(e)))
        })
        .collect();
    let mut poser = Vec::new();
    let mut conflits = Vec::new();
    for (i, paire) in jumeaux::apparier(&des_paies, &du_fichier).into_iter().enumerate() {
        let Some(j) = paire else { continue };
        let Some(n) = paies[i].engagement.numero.clone().filter(|n| !n.is_empty()) else { continue };
        match du_fichier[j].as_ref().and_then(|x| x.2.clone()) {
            None => poser.push((j, n)),
            Some(pose) if pose == n => {}
            Some(pose) => conflits.push(format!(
                "  - {} {} (contrat du {}) : le fichier porte le n° {}, cette production lui donnerait le n° {}.",
                prenom_de(&paies[i].salarie),
                cle_de(&paies[i]),
                paies[i].engagement.debut.jj_mm_aaaa(),
                pose,
                n
            )),
        }
    }
    if !conflits.is_empty() {
        return Err(erreur(
            "ErreurOperation",
            format!(
                "Le mois {} ne se produit pas : un numero de contrat deja pose dans paies/{}.json ne s'ecrase pas (il se lit, il ne se recalcule pas) :\n{}\nLa saisie produite ne porte pas ces numeros : recharger le mois depuis le fichier, puis produire. Rien n'est ecrit.",
                mois,
                mois,
                conflits.join("\n")
            ),
        ));
    }
    Ok(Some(NumerosAFiger { chemin, data, poser }))
}

/// `figer_numeros` : ecrit dans le fichier de saisie les numeros de contrat que le mois vient de declarer (ceux que
/// `numeros_a_figer` a apparies), et `numeros_figes: true`. Le fichier dit ce qui a ete declare, et le mois suivant
/// d'un contrat en cours le recopie. Apres `valider`, sous le verrou : un numero ecrit ici est un numero consomme.
fn figer_numeros(plan: Option<NumerosAFiger>) -> R<Option<PathBuf>> {
    let Some(NumerosAFiger { chemin, mut data, poser }) = plan else { return Ok(None) };
    let change = !poser.is_empty();
    if let Some(Value::Array(engagements)) = data.get_mut("engagements") {
        for (j, n) in poser {
            if let Some(Value::Object(o)) = engagements.get_mut(j) {
                o.insert("numero".into(), json!(n));
            }
        }
    }
    if change || !data.get("numeros_figes").is_some_and(vrai) {
        data["numeros_figes"] = json!(true);
        crate::routes_contrats::sauver_avant(&chemin)?;
        ecrire_json(&chemin, &data, 2).map_err(|m| erreur("OSError", m))?;
    }
    Ok(Some(chemin))
}

// ------------------------------------------------------------- les etapes

/// Une chose a faire apres la production : `{"cle", "titre", "detail", "commande"}`.
fn etape(cle: &str, titre: &str, detail: String, commande: Option<String>) -> Value {
    json!({"cle": cle, "titre": titre, "detail": detail, "commande": commande})
}

/// `_etapes_mois`, pour un mois dont les contrats et les DPAE sont deja partis (`sans_contrat=True`).
fn etapes_du_mois(etiq: &str, mois: &str, cs_premier: i64, cs_dernier: i64, certificats: bool) -> Vec<Value> {
    let mut etapes = vec![etape(
        "dsn",
        "DSN",
        format!("Deposer DSN_{}.dsn EN TEST (depot-dsn --test), lire le bilan, PUIS deposer en reel.", etiq),
        None,
    )];
    if certificats {
        etapes.push(etape(
            "cs",
            "Conges Spectacles",
            format!(
                "REMETTRE CertificatsCS_{}.pdf aux salaries (N CE {} a {}). Rien a envoyer a Audiens : la cotisation part par la DSN. Ces certificats ont valeur de cheque sur notre compte Conges Spectacles.",
                etiq, cs_premier, cs_dernier
            ),
            None,
        ));
    }
    etapes.push(etape(
        "aer",
        "Attestations France Travail",
        format!(
            "Quelques heures apres le depot de la DSN, France Travail renvoie les AER Cinema Spectacle. La releve de l'ouverture les signe et les range dans sorties/{}/envoi/ ; elles partiront avec le message aux salaries. A la main, si besoin :",
            mois
        ),
        Some(format!("paie retours ; paie aer --mois {}", mois)),
    ));
    etapes.push(etape(
        "envoi",
        "Salaries",
        "Preparer un message par salarie (bulletin, certificat CS, AER) et relire avant d'agir. A faire APRES l'etape precedente : sans l'AER, le message part en annoncant qu'elle suivra.".to_string(),
        None,
    ));
    etapes
}

// ------------------------------------------------------------- produire le mois

/// Ce que `produire_mois` rend : tout ce que `mois_produit` montre.
struct MoisProduit {
    mois: String,
    libelle: String,
    dossier: PathBuf,
    vues: Vec<Value>,
    total_brut: Value,
    total_cout: Value,
    urssaf: Value,
    envoi_test: bool,
    numeros: Value,
    fichiers: Vec<PathBuf>,
    avertissements: Vec<Avis>,
    etapes: Vec<Value>,
    /// Combien des `avertissements` viennent de l apercu (les premiers) : la ligne de commande les montre deux fois, comme le Python.
    n_apercu: usize,
}

/// `sortie_json.mois_produit`.
fn mois_produit(r: MoisProduit) -> Value {
    let fichiers: Vec<Value> = r
        .fichiers
        .iter()
        .map(|f| {
            json!({
                "nom": f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
                "chemin": f.to_string_lossy(),
                "octets": std::fs::metadata(f).ok().map(|m| m.len()),
            })
        })
        .collect();
    json!({
        "mois": r.mois,
        "libelle": r.libelle,
        "dossier": r.dossier.to_string_lossy(),
        "paies": r.vues,
        "total_brut": r.total_brut,
        "total_cout": r.total_cout,
        "urssaf": r.urssaf,
        "envoi_test": r.envoi_test,
        "numeros": r.numeros,
        "fichiers": fichiers,
        "avertissements": r.avertissements.iter().filter(|a| !a.info).map(|a| a.texte.clone()).collect::<Vec<_>>(),
        "informations": r.avertissements.iter().filter(|a| a.info).map(|a| a.texte.clone()).collect::<Vec<_>>(),
        "etapes": r.etapes,
    })
}

/// Un etablissement et ce qu'il declare ce mois-ci : `(nic, paies, corrections, arrets)`.
struct Lot {
    nic: Option<String>,
    paies: Vec<usize>,
    regs: Vec<usize>,
    arrets: Vec<Value>,
}

/// `produire_mois(sans_contrat=True)` : bulletins, certificats CS et DSN d'un mois, tout ou rien.
#[allow(clippy::too_many_arguments)]
fn produire_mois(
    o: &HashMap<String, String>,
    ctx: &Contexte,
    m: &mut Moteur,
    data: &Value,
    rep: &Value,
    envoi_test: bool,
    forcer: bool,
    qui: &str,
    sans_contrat: bool,
    constitution: Option<crate::date::Jour>,
    reserves: &mut Reserves,
) -> R<MoisProduit> {
    let _verrou = crate::ecrire_documents::prendre_le_verrou(ctx, qui)?;
    let mois = py_str(data.get("mois").ok_or_else(|| erreur("KeyError", crate::fichiers::absent("mois")))?);
    // apercu_mois : le mois calcule, rien d'ecrit (`charger` : la saisie numerotee, une deuxieme fois).
    let annee: i64 = mois.get(..4).and_then(|a| a.parse().ok()).ok_or_else(|| valeur(format!("nombre entier illisible : {}", crate::fichiers::repr(&mois))))?;
    let numero_mois: usize = mois.get(5..7).and_then(|a| a.parse().ok()).ok_or_else(|| valeur(format!("mois illisible : {}", crate::fichiers::repr(&mois))))?;
    let paies = charger(ctx, data, rep, Some(reserves))?;
    let (urssaf, avert) = crate::routes_ouverture::urssaf_de_l_apercu(ctx, &paies, &mois)?;
    let mut avertissements: Vec<Avis> = avert.into_iter().map(avertissement).collect();
    avertissements.extend(crate::routes_ouverture::archives_employees(&paies)?.into_iter().map(avertissement));
    for p in &paies {
        avertissements.extend(p.avertissements.iter().cloned());
    }
    let n_apercu = avertissements.len();
    let zero = crate::num::Decimal::new(0, 2);
    let total_brut = paies.iter().fold(zero, |a, p| a + q2(p.brut));
    let total_cout = paies.iter().fold(zero, |a, p| a + q2(p.cout_employeur));

    crate::routes_ecritures::refuser_si_clos(ctx, &mois)?; // un exercice cloture ne bouge plus
    crate::routes_ecritures::refuser_si_hors_delai(ctx, &mois, forcer)?; // DSN due le 15
    let fiche = ctx.entreprise.clone();
    let principal = crate::bareme::nic_principal(&fiche);
    let secondaires: Vec<String> = crate::bareme::secondaires(&fiche).iter().map(|e| py_str(e.get("nic").unwrap_or(&Value::Null))).collect();
    let plusieurs = !secondaires.is_empty();
    if !envoi_test && ESSAI_DSN == 1 {
        // Un mois deja declare dont l'annule-et-remplace n'est plus admis (veille de l'echeance passee) : on le dit
        // AVANT d'ecrire le moindre bulletin -- la correction passe par la DSN suivante.
        let nics: Vec<Value> = if plusieurs { std::iter::once(principal.clone()).chain(secondaires.iter().cloned()).map(Value::String).collect() } else { vec![Value::Null] };
        for nic in &nics {
            forme_a_produire(ctx, m, &mois, nic)?;
        }
    }
    // Quel engagement du fichier recevra quel numero : un numero deja pose et different se refuse ICI, avant la premiere
    // ecriture ; il ne s'ecrit qu'apres `valider`.
    let a_figer = numeros_a_figer(ctx, &mois, data, &paies)?;
    let etiq = crate::apercu_dsn::etiquette(&mois).map_err(valeur)?;
    // Ceux de l'apercu d'abord : ils portent sur ce qu'on produit, pas sur la facon de le produire.
    if envoi_test {
        avertissements.push(info(
            "DSN produite en ENVOI DE TEST (S10.G00.00.005 = 01) : controles executes et bilan rendu, donnees non conservees. Un fichier code '02' est traite comme REEL.",
        ));
    }
    let sortie = ctx.racine.join("sorties").join(&mois);
    std::fs::create_dir_all(&sortie).map_err(|e| os(&sortie, e))?;
    // Licence resolue une fois pour toutes sur le mois produit : bulletins et DSN doivent porter le meme numero.
    let ent = crate::oracle::entreprise_du_mois(ctx, &mois).map_err(valeur)?;
    let a_imprimer = paies_a_imprimer(ctx, data, &paies)?;

    // UN CONTRAT QUI DEBORDE DU MOIS n'a son certificat Conges Spectacles qu'au DERNIER mois, pour le contrat
    // entier ; le bulletin et la DSN, eux, sont mensuels. Un permanent n'a pas de certificat.
    let (pour_cs, avert_cs) = crate::routes_dossier_salarie::certificats_dus(ctx, &a_imprimer)?;
    // La phrase de `produire_mois` finit par un conseil que celle du dossier salarie n'a pas.
    avertissements.extend(avert_cs.into_iter().map(|a| avertissement(format!("{} Si ces mois ont ete payes, les saisir et reproduire.", a))));
    let paies_cs: Vec<&PaieCs> = pour_cs.iter().filter_map(|(_, _, v)| v.as_ref()).collect();
    for p in &paies {
        if p.engagement.premier_mois()? && p.engagement.nat()?.mensuelle() {
            avertissements.push(avertissement(format!(
                "{} {} : {} commence en {} -- le moteur n'imprime que le contrat d'un CDD d'usage ; celui-ci se redige a part.",
                py_str(p.salarie.get("nom").unwrap_or(&Value::Null)),
                prenom_de(&p.salarie),
                p.engagement.nat()?.libelle,
                engagement::libelle_mois(&mois)
            )));
        }
    }
    // Il y a d'abord le contrat et la DPAE (`sans_contrat` : ni l'un ni l'autre) ; puis les bulletins : un par paie, et
    // un mois du contrat sans cachet n'en a pas. Un mois ou personne n'est paye n'a pas de fichier.
    let mut produits: Vec<PathBuf> = Vec::new();
    if !sans_contrat {
        // `embauches` : les contrats qui COMMENCENT ce mois-ci ; le contrat s'imprime pour un CDD d'usage (un permanent
        // se redige a part, et le mois l'a dit plus haut). Le fichier DPAE du mois, avec son recapitulatif.
        let mut embauches: Vec<&Paie> = Vec::new();
        for p in &paies {
            if p.engagement.premier_mois()? {
                embauches.push(p);
            }
        }
        if !embauches.is_empty() {
            let mut a_rediger: Vec<Paie> = Vec::new();
            for p in &embauches {
                if !p.engagement.nat()?.mensuelle() {
                    a_rediger.push((*p).clone());
                }
            }
            if !a_rediger.is_empty() {
                let chemin = sortie.join(format!("Contrats_{}.pdf", etiq));
                crate::contrat::ecrire_et_garder(ctx, &a_rediger, &ent, &chemin, &[])?;
                produits.push(chemin);
            }
            produits.push(crate::routes_actes_h1::ecrire_dpae_du_mois(ctx, &ent, &embauches, &sortie, &etiq)?);
        }
    }
    let payes: Vec<usize> = (0..paies.len()).filter(|i| !paies[*i].engagement.sans_paie()).collect();
    if !payes.is_empty() {
        let imprimees: Vec<Paie> = payes.iter().map(|i| a_imprimer[*i].clone()).collect();
        let description = crate::bulletin::jeux_et_description(ctx, &imprimees, &ent, &mois, None, false, &HashMap::new()).map_err(valeur)?;
        let chemin = sortie.join(format!("Bulletins_{}.pdf", etiq));
        crate::routes_dossier_salarie::ecrire_pdf(&description, &chemin)?;
        produits.push(chemin);
    }
    if paies.is_empty() {
        avertissements.push(info(format!(
            "Aucun salarie en {} : la DSN part quand meme, sans individu (envoi neant) -- une DSN est due chaque mois.",
            engagement::libelle_mois(&mois)
        )));
    }
    for p in &paies {
        if p.engagement.sans_paie() {
            avertissements.push(info(format!(
                "{} {} : contrat {} en cours, aucun cachet en {} -- pas de bulletin ; la DSN porte le contrat avec un versement a zero.",
                py_str(p.salarie.get("nom").unwrap_or(&Value::Null)),
                prenom_de(&p.salarie),
                p.engagement.numero.as_deref().unwrap_or("None"),
                engagement::libelle_mois(&mois)
            )));
        }
    }

    // Certificats d'emploi Conges Spectacles : rien a deposer nulle part, ils sont dus AU SALARIE. Le N CE est une
    // sequence employeur continue, que `prochain_continu` lit sur toutes les annees a la fois.
    let n_cs = crate::ecrire_documents::prochain_continu(ctx, "certificat_cs", 1)?;
    let mut combien = 0i64;
    for p in &paies_cs {
        combien += conges_spectacles::decouper(&p.engagement)?.len() as i64;
    }
    // UN CERTIFICAT SANS NIR N'EST RATTACHABLE A PERSONNE : il sort quand meme, mais le mois le dit.
    let mut vues_cs: Vec<String> = Vec::new();
    for p in &paies_cs {
        let cle = py_str(p.salarie.get("_cle").unwrap_or(&Value::Null));
        if vues_cs.contains(&cle) {
            continue;
        }
        vues_cs.push(cle.clone());
        let s_cs = ctx.salarie(&cle).map_err(valeur)?;
        if s_cs.get("nir").filter(|n| vrai(n)).map(py_str).unwrap_or_default().trim().is_empty() {
            avertissements.push(avertissement(format!(
                "{} {} : certificat Conges Spectacles emis SANS numero de securite sociale -- Audiens ne pourra pas le rattacher. Le reproduire quand le numero sera connu.",
                py_str(s_cs.get("nom").unwrap_or(&Value::Null)),
                prenom_de(&s_cs)
            )));
        }
    }
    if !paies_cs.is_empty() {
        let mut paires: Vec<(&PaieCs, i64)> = Vec::new();
        let mut numero = n_cs;
        for p in &paies_cs {
            paires.push((p, numero));
            numero += conges_spectacles::decouper(&p.engagement)?.len() as i64;
        }
        let chemin = sortie.join(format!("CertificatsCS_{}.pdf", etiq));
        let description = conges_spectacles::description_numerotee(&paires, &ent).map_err(Echec::from)?;
        crate::routes_dossier_salarie::ecrire_pdf(&description, &chemin)?;
        produits.push(chemin);
        let recap = sortie.join(format!("CertificatsCS_{}_recapitulatif.txt", etiq));
        let texte = conges_spectacles::recapitulatif(&paires, &ent).map_err(Echec::from)?;
        std::fs::write(&recap, texte).map_err(|e| os(&recap, e))?;
        produits.push(recap);
        reserves.enregistrer(annee, "certificat_cs", n_cs + combien - 1);
    }

    // Un jeu de documents PAR SALARIE, pour l'envoi. Le fichier groupe porte la paie de tout le monde : il ne doit
    // jamais partir a un salarie.
    crate::ecrire_documents::pieces_individuelles(ctx, &a_imprimer, &ent, &sortie.join("envoi"), &etiq, &mois, n_cs, &pour_cs)?;

    // LES CORRECTIONS DES MOIS DEPOSES : la DSN de ce mois les porte, datees de leur mois d'origine. Le net qu'elles
    // changent se paie (ou se retient) CE mois-ci, par un bulletin rectificatif du mois d'origine, ecrit apres la DSN.
    let regs = regularisations::regularisations_pour(o, ctx, m, &mois)?;
    for ligne in regularisations::resume_regularisations(&regs)? {
        avertissements.push(info(format!(
            "Regularisation embarquee dans la DSN : {}. La difference de net se paie -- ou se retient -- ce mois-ci, par un bulletin rectificatif.",
            ligne
        )));
    }
    // LA DECLARATION OETH D'AVRIL : elle ajoute au bordereau Urssaf une contribution (CTP 730 ou 740).
    if let Some(doeth) = crate::oeth::declaration(ctx, &mois).map_err(valeur)? {
        avertissements.push(info(format!("{} Le bordereau Urssaf du mois en porte {} EUR.", doeth.resume(), doeth.a_verser())));
    }
    // LA CONTRIBUTION SUPPLEMENTAIRE A L'APPRENTISSAGE DE MARS : le bloc 82 (079 ou 080) et le CTP 998 (`csa`).
    if let Some(c) = crate::csa::declaration(ctx, &mois).map_err(valeur)? {
        avertissements.push(info(c.resume()));
    }
    // LES ARRETS DE TRAVAIL : la DSN mensuelle doit REPORTER ceux dont la periode croise le mois, signales ou non.
    let arrets = crate::marques_depot::arrets_pour(ctx, &mois)?;
    let nomenclature = crate::routes_listes::nomenclatures(ctx)?;
    let mut non_signales = 0;
    for a in &arrets {
        let resume = crate::routes_listes::resume_arret(&nomenclature, a)?;
        let jour = |v: &Value| -> R<String> { Ok(crate::bulletin::jj_mm_aaaa(v).map_err(valeur)?) };
        let reprise = a.get("reprise").filter(|r| vrai(r));
        let fin = match reprise {
            Some(r) => r.clone(),
            None => a.get("fin_previsionnelle").cloned().unwrap_or(Value::Null),
        };
        avertissements.push(info(format!(
            "Arret de travail reporte dans la DSN : {}, {}, du {} au {}{}.",
            py_str(a.get("salarie").unwrap_or(&Value::Null)),
            py_str(&resume["motif_libelle"]),
            jour(a.get("dernier_jour_travaille").unwrap_or(&Value::Null))?,
            jour(&fin)?,
            if reprise.is_some() { " (reprise)" } else { " (fin previsionnelle)" }
        )));
        if !resume.get("signale").is_some_and(vrai) {
            non_signales += 1;
        }
    }
    // LES SUSPENSIONS (bloc 65) : chacune se declare entiere dans la DSN de chaque mois qu'elle traverse, tiree du magasin
    // des imprevus sous son contrat (`imprevus::suspensions_du_mois`) ; dit ici, comme l'arret.
    for s in crate::imprevus::du_mois(&ctx.racine, &mois, Some(crate::imprevus::SUSPENSION)).map_err(crate::routes_listes::refus_magasin)? {
        let jour = |v: &Value| -> R<String> { Ok(crate::bulletin::jj_mm_aaaa(v).map_err(valeur)?) };
        avertissements.push(info(format!(
            "Suspension declaree dans la DSN : {}, motif {}, du {} au {}.",
            py_str(&s["salarie"]),
            py_str(&s["motif"]),
            jour(&s["debut"])?,
            jour(&s["fin"])?
        )));
    }
    if non_signales > 0 {
        avertissements.push(avertissement(format!(
            "{} arret(s) reporte(s) dans la DSN mais JAMAIS SIGNALE(S) : le signalement d'arret est du dans les cinq jours, la DSN du mois ne le remplace pas (Arrets de travail › Signaler).",
            non_signales
        )));
    }

    // UNE DSN PAR ETABLISSEMENT : chacun avec ses salaries, ses corrections et ses arrets ; celle du principal garde
    // son nom de toujours, les autres portent leur NIC. Un etablissement sans salarie ce mois-ci depose quand meme la
    // sienne, sans individu : une DSN est due chaque mois, par SIRET.
    let mut lots: Vec<Lot> = Vec::new();
    if plusieurs {
        for nic in std::iter::once(principal.clone()).chain(secondaires.iter().cloned()) {
            let du_nic = |sal: &Value| -> R<bool> { Ok(crate::bareme::etablissement_du_salarie(&fiche, sal).map_err(valeur)? == nic) };
            let mut lot = Lot { nic: Some(nic.clone()), paies: Vec::new(), regs: Vec::new(), arrets: Vec::new() };
            for (i, p) in paies.iter().enumerate() {
                if du_nic(&p.salarie)? {
                    lot.paies.push(i);
                }
            }
            for (i, r) in regs.iter().enumerate() {
                if du_nic(&r.apres.salarie)? {
                    lot.regs.push(i);
                }
            }
            for a in &arrets {
                let sal = ctx.salarie(&py_str(a.get("salarie").unwrap_or(&Value::Null))).map_err(valeur)?;
                if du_nic(&sal)? {
                    lot.arrets.push(a.clone());
                }
            }
            lots.push(lot);
        }
    } else {
        lots.push(Lot { nic: None, paies: (0..paies.len()).collect(), regs: (0..regs.len()).collect(), arrets: arrets.clone() });
    }
    for lot in &lots {
        let mut remplacee_versait_pas = None;
        let mut nom = match &lot.nic {
            Some(n) if *n != principal => format!("DSN_{}_{}.dsn", etiq, n),
            _ => format!("DSN_{}.dsn", etiq),
        };
        // Au-dela du premier essai, c'est un annule-et-remplace ; ici, le premier.
        let (mut type_dsn, mut ordre_dsn) = (if ESSAI_DSN > 1 { "03".to_string() } else { "01".to_string() }, ESSAI_DSN);
        if !envoi_test && ESSAI_DSN == 1 {
            // UN MOIS DEJA DECLARE REPART EN ANNULE-ET-REMPLACE, de lui-meme : type 03, a l'ordre du dernier depot reel + 1,
            // sous un nom a lui -- le fichier deja depose reste sur le disque tel qu'il est parti.
            let nic = lot.nic.clone().map_or(Value::Null, Value::String);
            let (t, ordre, remplace) = forme_a_produire(ctx, m, &mois, &nic)?;
            if remplace {
                type_dsn = t;
                ordre_dsn = ordre;
                remplacee_versait_pas = remplacee_versait_pas_de(ctx, m, &mois, &nic)?;
                nom = format!("{}_AR{}.dsn", &nom[..nom.len() - ".dsn".len()], ordre_dsn);
                avertissements.push(info(format!(
                    "La DSN de {} est deja partie : celle-ci est un ANNULE-ET-REMPLACE integral (ordre {}). Elle remplace la precedente, ordres de telepaiement compris, et s'envoie au plus tard le {} a minuit.",
                    engagement::libelle_mois(&mois),
                    ordre_dsn,
                    m.limite(&mois)?.jj_mm_aaaa()
                )));
            }
        }
        let paies_e: Vec<Paie> = lot.paies.iter().map(|i| a_imprimer[*i].clone()).collect();
        let regs_e: Vec<&Reg> = lot.regs.iter().map(|i| &regs[*i]).collect();
        let regs_dsn = regularisations::pour_la_dsn(ctx, &regs_e)?;
        let arrets_e: Vec<crate::dsn::Arret> = lot.arrets.iter().map(|v| crate::dsn::Arret { v: v.clone() }).collect();
        let options = crate::dsn::Options {
            // Retours Windows, F : le jour ou le fichier se fabrique (celui du contexte), plus la fin de periode du mois.
            date_constitution: Some(constitution.unwrap_or(ctx.aujourdhui)),
            envoi_test,
            type_declaration: type_dsn,
            ordre: ordre_dsn,
            etablissement: lot.nic.clone(),
            remplacee_versait_pas,
            ..Default::default()
        };
        let texte = crate::dsn::generer(ctx, &paies_e, &mois, &regs_dsn, &arrets_e, &options).map_err(valeur)?;
        let f_dsn = sortie.join(&nom);
        crate::dsn::ecrire(&texte, &f_dsn).map_err(|m| erreur("OSError", m))?;
        produits.push(f_dsn.clone());
        if !regs_e.is_empty() {
            let cles: Vec<(String, i64)> = regs_e.iter().map(|r| (r.mois.clone(), r.numero)).collect();
            crate::marques_depot::marquer_preparees(ctx, &cles, &mois, &f_dsn)?;
        }
    }
    if !regs.is_empty() {
        // Apres la DSN : un refus de la declaration n'aura rien numerote.
        let rectifs = regularisations::ecrire_rectificatifs(ctx, &regs, &mois, &sortie, reserves)?;
        produits.extend(rectifs.fichiers);
        avertissements.extend(rectifs.avertissements.into_iter().map(avertissement));
    }

    // Les numeros n'existent qu'a partir d'ici : tous les fichiers sont ecrits.
    reserves.valider(ctx)?;
    // Et ils s'ecrivent dans la saisie : le fichier dit ce qui a ete declare, et le mois suivant d'un contrat en cours
    // le recopie.
    let numeros_figes = figer_numeros(a_figer)?;
    // La paie que chaque DSN declare, numeros figes : l'ecran dira si elle change avant l'envoi.
    for f in &produits {
        if f.to_string_lossy().to_lowercase().ends_with(".dsn") {
            noter_dsn_ecrite(m, ctx, &mois, f)?;
        }
    }
    if numeros_figes.is_none() {
        avertissements.push(avertissement(format!(
            "Numeros de contrat non enregistres : la saisie de {} n'a pas pu etre relue. Un contrat qui se poursuit le mois suivant risque de ne pas retrouver son numero -- rouvrir le mois et l'enregistrer a nouveau.",
            crate::routes_ouverture::MOIS_TITRE[numero_mois.clamp(1, 12) - 1].to_string() + &format!(" {}", annee)
        )));
    }

    let numero_de = |i: usize| -> i64 { paies[i].numero_bulletin.as_deref().and_then(|n| n.split('/').next()).and_then(|n| n.parse().ok()).unwrap_or(0) };
    let numeros = json!({
        "bulletin_premier": payes.first().map_or(0, |i| numero_de(*i)),
        "bulletin_dernier": payes.last().map_or(0, |i| numero_de(*i)),
        "cs_premier": n_cs,
        "cs_dernier": n_cs + combien - 1,
    });
    let tous = data.get("engagements").and_then(Value::as_array).cloned().unwrap_or_default();
    let mut vues = Vec::new();
    for (i, p) in paies.iter().enumerate() {
        vues.push(crate::routes_ouverture::vue_paie(ctx, p, tous.get(i).unwrap_or(&Value::Null), data, false)?);
    }
    Ok(MoisProduit {
        libelle: format!("{} {}", crate::routes_ouverture::MOIS_TITRE[numero_mois.clamp(1, 12) - 1], annee),
        mois: mois.clone(),
        dossier: sortie,
        vues,
        total_brut: json!(total_brut.to_string()),
        total_cout: json!(total_cout.to_string()),
        urssaf,
        envoi_test,
        numeros,
        fichiers: produits,
        avertissements,
        etapes: etapes_du_mois(&etiq, &mois, n_cs, n_cs + combien - 1, !paies_cs.is_empty()),
        n_apercu,
    })
}

// ------------------------------------------------------------- la route

/// `produire` : `{"mois", "saisie", "test"?, "forcer"?}`. La saisie de l'ecran s'ENREGISTRE D'ABORD (la route
/// `enregistrer-saisie`, fusion comprise), puis le mois se produit depuis le FICHIER relu : la DSN declare ce que dit
/// `paies/AAAA-MM.json`, et rien ne le reecrit apres que son empreinte a ete notee (RELECTURE-COHERENCE, point 9).
/// Rend la reponse de `sortie_json.mois_produit`.
pub fn produire(o: &HashMap<String, String>, ctx: &Contexte, charge: &Value) -> R<Value> {
    produire_pour(o, ctx, charge, "interface", None).map(|(v, _)| v)
}

/// `produire`, pour qui l appelle sous son nom (`qui` : `cli` pour la ligne de commande) et avec la date de constitution de la
/// DSN qu il choisit (`--constitution` ; le jour du contexte sinon). Rend aussi ce que l apercu a dit (a regarder, puis informations) :
/// la ligne de commande le montre deux fois, comme le Python.
pub(crate) fn produire_pour(o: &HashMap<String, String>, ctx: &Contexte, charge: &Value, qui: &str, constitution: Option<crate::date::Jour>) -> R<(Value, (Vec<String>, Vec<String>))> {
    let saisie = charge.get("saisie").ok_or_else(|| erreur("KeyError", crate::fichiers::absent("saisie")))?;
    let Value::Object(brute) = saisie else {
        return Err(erreur("TypeError", "`saisie` : un objet est attendu"));
    };
    let mois = py_str(charge.get("mois").ok_or_else(|| erreur("KeyError", crate::fichiers::absent("mois")))?);
    let envoi_test = charge.get("test").is_some_and(vrai);
    let forcer = charge.get("forcer").is_some_and(vrai);
    // `enregistrer_saisie` D'ABORD : elle valide la saisie de l'ecran en la calculant, reprend du fichier ce que l'ecran
    // ne montre pas (`_origine`, `valide`, `numero`, `rouverte`, `numeros_figes`) et garde l'ancien dans `avant/`.
    // Produire depuis l'ecran puis reecrire le fichier depuis l'ecran changeait le JSON apres que l'empreinte de la DSN
    // etait notee (un ordre d'engagements, un champ que l'ecran ne renvoie pas) : le depot refusait « la paie a change ».
    let chemin = crate::routes_ecritures::ecrire_saisie(o, ctx, &mois, brute, forcer, true)?;
    // Puis tout part du FICHIER : le calcul, les manques, le controle du calcul stocke, la production et ses numeros.
    let mut data = crate::engagement::lire_saisie(&chemin).map_err(Echec::from)?;
    if let Value::Object(d) = &mut data {
        d.insert("mois".into(), Value::String(mois.clone()));
    }

    // fermer_mois : `charger(avec_entrees=True)`, les manques, le calcul stocke.
    let rep = calculer(o, ctx, &data)?;
    let mut reserves_du_controle = Reserves::default();
    let paies = charger(ctx, &data, &rep, Some(&mut reserves_du_controle))?;
    let mut m = Moteur::nouveau(o, ctx);
    let manques = manques_pour_fermer(&mut m, &paies)?;
    if !manques.is_empty() {
        let lignes: Vec<String> = manques.iter().map(|x| format!("  - {}", py_str(&x["phrase"]))).collect();
        return Err(avec_donnee("MoisPasPret", format!("Le mois {} ne peut pas encore se fermer :\n{}", mois, lignes.join("\n")), "manques", Value::Array(manques)));
    }
    controler_calcul_stocke(o, ctx, &mois, &data, &rep);
    // La production repart des compteurs du disque : les numeros du controle ne sont pas gardes.
    let mut reserves = Reserves::default();
    let produit = produire_mois(o, ctx, &mut m, &data, &rep, envoi_test, forcer, qui, true, constitution, &mut reserves)?;
    let (a_regarder, informations): (Vec<&Avis>, Vec<&Avis>) = produit.avertissements[..produit.n_apercu].iter().partition(|a| !a.info);
    let apercu_avertissements = (a_regarder.iter().map(|a| a.texte.clone()).collect::<Vec<_>>(), informations.iter().map(|a| a.texte.clone()).collect::<Vec<_>>());
    Ok((mois_produit(produit), apercu_avertissements))
}

// ------------------------------------------------------- l'horloge de la compagnie fictive

/// `operations_paies.produire_mois(ctx, saisie, qui=..., forcer_delai=True)` tel que l'HORLOGE de la compagnie fictive
/// l'appelle (`horloge.rs`) : le mois s'ecrit en entier -- contrats, DPAE du mois, bulletins, certificats, DSN --,
/// SANS passer par `fermer_mois` (la DPAE de chaque embauche n'est pas encore partie : l'horloge la fera le jour de
/// l'embauche), et SANS enregistrer la saisie (la route `produire` le fait ; l'horloge relit le fichier tel quel,
/// seuls les numeros de contrat s'y figent, comme dans le Python). Le delai de la DSN est force.
///
/// `ctx` porte la date de FIN DE MOIS (`Contexte(racine, date=_fin_de_mois(mois))`). Une seule reservation de
/// numeros, comme le Python qui n'a pas ici le controle de `fermer_mois`.
pub(crate) fn produire_pour_l_horloge(o: &HashMap<String, String>, ctx: &Contexte, mois: &str, qui: &str) -> R<()> {
    let chemin = ctx.racine.join("paies").join(format!("{}.json", mois));
    let mut data = crate::engagement::lire_saisie(&chemin).map_err(Echec::from)?;
    if let Value::Object(m) = &mut data {
        if !m.contains_key("mois") {
            m.insert("mois".into(), Value::String(mois.to_string()));
        }
    }
    let rep = calculer(o, ctx, &data)?;
    let mut m = Moteur::nouveau(o, ctx);
    let mut reserves = Reserves::default();
    produire_mois(o, ctx, &mut m, &data, &rep, false, true, qui, false, None, &mut reserves)?;
    Ok(())
}

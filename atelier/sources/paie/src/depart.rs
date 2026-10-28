//! CE QUI PART SE FABRIQUE A SON DEPART, depuis l'engagement valide (`engagement::etats::validees_du_mois`) :
//!
//! - la DSN du mois, quand on la depose (`fabriquer_dsn`, la route `dsn-a-deposer`) : des qu'un engagement du mois est
//!   valide, sans attendre ni DPAE ni contrat. Le moteur dit si c'est un depot initial ou un annule-et-remplace ; apres la
//!   veille de l'echeance, un mois deja declare ne se redepose plus (`AnnuleRemplaceHorsDelai`, cahier technique DSN
//!   §1.4.2) : sa correction part dans la DSN d'un mois suivant (`produire_regularisations::regularisations_pour`) ;
//! - le bulletin et le certificat Conges Spectacles, quand les documents partent au salarie (`fabriquer_documents`, la
//!   route `envoyer-documents`). Le numero du bulletin (`N/AAAA`, suite de l'annee) et le premier N CE du certificat
//!   s'ecrivent alors dans la paie de l'engagement (`DocumentsFabriques::engager`) et se relisent ensuite. Une paie partie
//!   puis refaite part sous un numero nouveau, comme un bulletin rectificatif de celui qui est parti, et l'original reste
//!   a cote.
//!
//! L'AER ne se fabrique pas : France Travail la renvoie apres la DSN, et elle part avec les documents quand elle est la.
//! Un numero de contrat s'ecrit dans l'engagement a sa validation (`engagement::numeros`) ; tout ce qui part le lit la.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};

use documents::conges_spectacles::{self, PaieCs};
use crate::produire_regularisations::{self as regularisations, Reg};
use crate::routes_mois::Moteur;
use droit::contexte::Contexte;
use engagement::calcul::Paie;
use engagement::Avis;
use socle::echec::erreur;
use socle::echec::Echec;
use socle::echec::R;
use socle::ecriture::ecrire_json;
use socle::fichiers::vrai;
use documents::ecrire_documents::Reserves;
use socle::valeur::valeur_en_texte;

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
    p.salarie.get("_cle").map(valeur_en_texte).unwrap_or_default()
}

fn prenom_de(sal: &Value) -> String {
    match sal.get("prenom_affichage") {
        Some(v) if vrai(v) => valeur_en_texte(v),
        _ => valeur_en_texte(sal.get("prenom").unwrap_or(&Value::Null)),
    }
}

fn annee_de(mois: &str) -> R<i64> {
    mois.get(..4).and_then(|a| a.parse().ok()).ok_or_else(|| valeur(format!("nombre entier illisible : {}", socle::fichiers::repr(mois))))
}

// ------------------------------------------------------------- le journal des pieces ecrites

/// LE JOURNAL DES PIECES ECRITES (`journal/dsn-ecrites.json`, #236) : chaque piece que le moteur ecrit -- une DSN, une
/// DPAE, un export compta -- y note QUAND, au temps du moteur (`temps.rs`, par `incidents::maintenant`). C'est ce que
/// l'ecran dit (« ecrit le ... ») : la date d'un fichier n'est pas celle de son ecriture (une fictive livree est
/// decompressee chez le visiteur, un banc simule le temps). `cle` : `<mois>/<fichier>`, ou `exercices/<fichier>` ;
/// `empreinte_saisie` : la paie que la piece declare -- pour une DSN, l'empreinte de la saisie du mois ; pour l'export compta
/// de l'annee, celles de ses saisies (`exercice_ecrit::empreintes_des_saisies`).
pub(crate) fn noter_piece_ecrite(racine: &Path, cle: &str, empreinte_saisie: Option<Value>) -> R<()> {
    let p = racine.join("journal").join("dsn-ecrites.json");
    let mut d = Value::Object(journal_des_ecritures(racine));
    let mut note = json!({"quand": crate::incidents::maintenant()});
    if let Some(e) = empreinte_saisie {
        note["empreinte_saisie"] = e;
    }
    d[cle] = note;
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).map_err(|e| os(parent, e))?;
    }
    ecrire_json(&p, &d, 2).map_err(|m| erreur("OSError", m))
}

/// La cle d'une piece au journal des ecritures : `<son dossier>/<son nom>` (`2026-11/DPAE_Novembre2026.txt`,
/// `exercices/Compta_2026.csv`).
pub(crate) fn cle_de_piece(fichier: &Path) -> String {
    let nom = |p: Option<&Path>| p.and_then(Path::file_name).map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    format!("{}/{}", nom(fichier.parent()), nom(Some(fichier)))
}

/// Le journal des pieces ecrites : `{"<mois>/<fichier>": {"quand", "empreinte_saisie"}}`. Un journal absent ou illisible
/// est vide.
pub(crate) fn journal_des_ecritures(racine: &Path) -> Map<String, Value> {
    std::fs::read(racine.join("journal").join("dsn-ecrites.json"))
        .ok()
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default()
}

/// LA DSN DU MOIS EST-ELLE ECRITE ? Une DSN mensuelle du mois notee au journal des ecritures, et toujours la ou le moteur
/// l'a ecrite (`sorties/<mois>/`). Un fichier DSN que le moteur n'a pas note (pose a la main) ne compte pas. C'est le
/// critere de l'accueil (`routes_accueil::bilan_des_mois`), des rappels (`routes_ouverture::dsn_deposable`) et de
/// l'etape de depot (`routes_mois`).
pub(crate) fn mois_ecrit(racine: &Path, mois: &str) -> bool {
    let prefixe = format!("{}/", mois);
    let sorties = racine.join("sorties");
    journal_des_ecritures(racine)
        .keys()
        .any(|k| k.strip_prefix(&prefixe).is_some_and(crate::routes_mois::est_une_dsn_mensuelle) && sorties.join(k).is_file())
}

/// QUAND LE MOTEUR A ECRIT CETTE PIECE (`AAAA-MM-JJTHH:MM`), lu au journal des ecritures (`noter_piece_ecrite`) ; `null`
/// s'il ne le dit pas. Jamais la date du fichier (#236).
pub(crate) fn ecrit_le(racine: &Path, fichier: &Path) -> Value {
    journal_des_ecritures(racine)
        .get(&cle_de_piece(fichier))
        .and_then(|n| n.get("quand"))
        .and_then(Value::as_str)
        .map_or(Value::Null, |q| json!(q.chars().take(16).collect::<String>()))
}

// ------------------------------------------------------------- la DSN, a son depot

/// Ce que doit etre la prochaine DSN REELLE de ce mois : `(type, ordre, remplace)`. Normale d'ordre 1 si rien n'est
/// passe ; sinon annule-et-remplace au rang suivant -- et un refus si son delai est passe.
fn forme_de_la_dsn(ctx: &Contexte, m: &mut Moteur, mois: &str, nic: &Value) -> R<(String, i64, bool)> {
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
                "La DSN de {} est deja partie, et l'annule-et-remplace n'est plus admis depuis le {} a minuit (veille de l'echeance, cahier technique DSN §1.4.2). Ce qui a change dans ce mois part dans la DSN d'un mois suivant, en correction. Rien n'est ecrit.",
                mois,
                limite.jj_mm_aaaa()
            ),
        ));
    }
    Ok(("03".into(), dernier + 1, true))
}

/// La declaration qu'un annule-et-remplace remplace (la derniere DSN mensuelle partie du mois, `dsn_parties`)
/// portait-elle un versement du prelevement a la source (S21.G00.20.002 = DGFIP_PAS) ? `None` : aucune n'est partie.
fn remplacee_versait_pas_de(ctx: &Contexte, m: &mut Moteur, mois: &str, nic: &Value) -> R<Option<bool>> {
    Ok(dsn_parties(ctx, m, mois, nic)?.last().map(verse_a_la_dgfip))
}

/// La DSN porte-t-elle un versement du prelevement a la source ?
fn verse_a_la_dgfip(f: &crate::dsn::Fabriquee) -> bool {
    f.ordres.iter().any(|o| o.reference.as_deref() == Some(crate::dsn::DGFIP_PAS))
}

/// Le jour de constitution (S20.G00.05.007, `JJMMAAAA`) d'une DSN deja ecrite.
fn constitution_de(chemin: &Path) -> Option<socle::date::Jour> {
    let octets = std::fs::read(chemin).ok()?;
    let t = crate::depot::lire(&octets).champ("constitution").to_string();
    let (j, m, a) = (t.get(0..2)?, t.get(2..4)?, t.get(4..8)?);
    socle::date::Jour::lire(&format!("{}-{}-{}", a, m, j)).ok()
}

// ------------------------------------------------------------- ce qu'une DSN porte

/// La forme d'une DSN mensuelle : son en-tete, l'etablissement qu'elle declare, et le jour de sa constitution.
struct Forme {
    type_dsn: String,
    ordre: i64,
    /// Le NIC de l'etablissement ; `None` : la compagnie n'en a qu'un.
    etablissement: Option<String>,
    constitution: socle::date::Jour,
    remplacee_versait_pas: Option<bool>,
}

impl Forme {
    fn options(&self) -> crate::dsn::Options {
        crate::dsn::Options {
            date_constitution: Some(self.constitution),
            envoi_test: false,
            type_declaration: self.type_dsn.clone(),
            ordre: self.ordre,
            etablissement: self.etablissement.clone(),
            remplacee_versait_pas: self.remplacee_versait_pas,
            ..Default::default()
        }
    }
}

/// LA DSN DE `mois` TELLE QU'ELLE PARTIRAIT, sous la forme `forme` : depuis les paies ecrites des engagements valides du
/// mois de son etablissement, avec ce que `fabriquer_dsn` lui donne (les corrections a porter, les arrets du mois).
fn fabriquer_du_present(ctx: &Contexte, mois: &str, forme: &Forme) -> R<(crate::dsn::Fabriquee, bool)> {
    let fiche = ctx.entreprise.clone();
    let nic = forme.etablissement.clone();
    let du_nic = |sal: &Value| -> R<bool> {
        match &nic {
            Some(n) => Ok(droit::bareme::etablissement_du_salarie(&fiche, sal).map_err(valeur)? == *n),
            None => Ok(true),
        }
    };
    let (_data, toutes) = documents::ecrire_documents::paies_aux_numeros_declares(ctx, mois, None)?;
    let mut paies: Vec<Paie> = Vec::new();
    for p in toutes {
        if du_nic(&p.salarie)? {
            paies.push(p);
        }
    }
    let regs = regularisations::regularisations_pour(ctx, mois)?;
    let mut regs_e: Vec<&Reg> = Vec::new();
    for r in &regs {
        if du_nic(&r.apres.salarie)? {
            regs_e.push(r);
        }
    }
    let regs_dsn = regularisations::pour_la_dsn(&regs_e);
    let mut arrets: Vec<crate::dsn::Arret> = Vec::new();
    for a in crate::marques_depot::arrets_pour(ctx, mois)? {
        let sal = ctx.salarie(&valeur_en_texte(a.get("salarie").unwrap_or(&Value::Null))).map_err(valeur)?;
        if du_nic(&sal)? {
            arrets.push(crate::dsn::Arret { v: a });
        }
    }
    let vide = paies.is_empty() && regs_dsn.is_empty();
    Ok((crate::dsn::fabriquer(ctx, &paies, mois, &regs_dsn, &arrets, &forme.options()).map_err(valeur)?, vide))
}

/// LA DSN QUE PORTE LE FICHIER `chemin` (mois principal `mois`, etablissement `nic` ; le principal sans lui), fabriquee
/// de nouveau depuis les paies des engagements qu'elle couvre (`fabriquer_du_present`), sous la forme que son en-tete
/// dit, au jour ou elle a ete constituee. `None` : le fichier ne dit plus ce que disent les paies, il se refait.
pub(crate) fn refabriquer(ctx: &Contexte, chemin: &Path, mois: &str, nic: Option<&str>) -> R<Option<crate::dsn::Fabriquee>> {
    let Ok(octets) = std::fs::read(chemin) else { return Ok(None) };
    let Some(constitution) = constitution_de(chemin) else { return Ok(None) };
    let entete = crate::depot::lire(&octets);
    let type_dsn = entete.champ("type_declaration").to_string();
    let Ok(ordre) = entete.champ("ordre").trim().parse::<i64>() else { return Ok(None) };
    let plusieurs = !droit::bareme::secondaires(&ctx.entreprise).is_empty();
    let nic = nic.filter(|n| !n.is_empty()).map(str::to_string).unwrap_or_else(|| droit::bareme::nic_principal(&ctx.entreprise));
    let nic_v = if plusieurs { Value::String(nic.clone()) } else { Value::Null };
    let remplacee_versait_pas = if type_dsn == "03" || type_dsn == "05" {
        let mut m = Moteur::nouveau(ctx);
        remplacee_versait_pas_de(ctx, &mut m, mois, &nic_v)?
    } else {
        None
    };
    let forme = Forme { type_dsn, ordre, etablissement: plusieurs.then_some(nic), constitution, remplacee_versait_pas };
    let (f, _) = fabriquer_du_present(ctx, mois, &forme)?;
    Ok((crate::dsn::en_latin1(&f.texte).ok().as_deref() == Some(octets.as_slice())).then_some(f))
}

/// L'engagement de `mois` du salarie `cle` au contrat `numero`.
fn engagement_de(ctx: &Contexte, mois: &str, cle: &str, numero: &str) -> R<engagement::stock::Objet> {
    engagement::stock::objets_du_mois(&ctx.racine, mois)?
        .into_iter()
        .find(|o| {
            let b = Value::Object(o.brut.clone());
            engagement::cle_de_l_engagement(ctx, &b).as_deref() == Some(cle) && b.get("numero").map(valeur_en_texte).as_deref() == Some(numero)
        })
        .ok_or_else(|| valeur(format!("{} : aucun engagement de {} ne porte le contrat {} qu'une DSN partie a declare.", cle, engagement::libelle_mois(mois), numero)))
}

/// La paie de `mois` que la version `version` de l'engagement `o` (salarie `cle`, contrat `numero`) a ecrite
/// (`engagement::etats::paie_declaree`, sur ce seul depart).
fn paie_de_la_version(ctx: &Contexte, o: &engagement::stock::Objet, cle: &str, numero: &str, mois: &str, version: u64) -> R<Paie> {
    use engagement::departs::{Depart, Departs, Envoi};
    let mut seul = Departs::default();
    seul.noter(cle, numero, Depart { envoi: Envoi::Dsn, mois: Some(mois.to_string()), par: None, version, le: String::new() });
    engagement::etats::paie_declaree(ctx, o, &seul, mois)?
        .ok_or_else(|| valeur(format!("{} : la version {} du contrat {} n'a pas de paie de {}.", cle, version, numero, engagement::libelle_mois(mois))))
}

/// Le jour ou la version `n` de l'engagement `o` a ete validee (`le` de la presente, `valide_le` d'une passee).
fn valide_le(o: &engagement::stock::Objet, n: u64) -> Option<socle::date::Jour> {
    let passees = o.brut.get("versions").and_then(Value::as_array).into_iter().flatten();
    let v = [o.brut.get("valide"), o.brut.get("precedente")].into_iter().flatten().chain(passees).find(|v| v.get("version").and_then(Value::as_u64) == Some(n))?;
    let le = v.get("valide_le").or_else(|| v.get("le")).and_then(Value::as_str)?;
    socle::date::Jour::lire(le.get(..10)?).ok()
}

/// LA DSN MENSUELLE PARTIE que dit la ligne `lignes[rang]` du journal, fabriquee de nouveau par la fonction qui l'a
/// fabriquee (`dsn::fabriquer`) depuis les versions qu'elle a emportees (`versions` de sa ligne) : les paies de son mois,
/// et les corrections des mois passes qu'elle portait -- leur paie d'apres, la version emportee ; ce qu'elles annulaient,
/// le dernier etat declare par ce qui etait parti avant elle (`versement_declare::dernier_declare` sur les lignes qui la
/// precedent). Les arrets n'y sont pas : ils ne portent aucun montant. `remplacee_versait_pas` : sur un
/// annule-et-remplace, si la DSN remplacee versait a la DGFiP.
fn dsn_partie(ctx: &Contexte, m: &mut Moteur, lignes: &[Value], rang: usize, remplacee_versait_pas: Option<bool>) -> R<crate::dsn::Fabriquee> {
    let l = &lignes[rang];
    let mois = valeur_en_texte(l.get("mois").unwrap_or(&Value::Null));
    let detail = l.get("detail").cloned().unwrap_or(Value::Null);
    let champ = |k: &str| detail.get(k).filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default();
    let type_dsn = Some(champ("type")).filter(|t| !t.is_empty()).unwrap_or_else(|| "01".into());
    let plusieurs = !droit::bareme::secondaires(&ctx.entreprise).is_empty();
    let nic = Some(champ("etablissement")).filter(|n| !n.is_empty()).unwrap_or_else(|| droit::bareme::nic_principal(&ctx.entreprise));
    let quand = valeur_en_texte(l.get("quand").unwrap_or(&Value::Null));
    let constitution = socle::date::Jour::lire(quand.get(..10).unwrap_or_default()).map_err(valeur)?;
    let forme = Forme { type_dsn, ordre: m.ordre_de_la_ligne(&mois, l)?, etablissement: plusieurs.then_some(nic), constitution, remplacee_versait_pas };
    let avant = crate::journal::departs_des_lignes(&lignes[..rang]);
    let ctx_m = ctx.du_mois(&mois).map_err(valeur)?;
    let mut paies: Vec<Paie> = Vec::new();
    let mut regs: Vec<crate::dsn::Regularisation> = Vec::new();
    for v in l.get("versions").and_then(Value::as_array).into_iter().flatten() {
        let texte = |k: &str| v.get(k).map(valeur_en_texte).unwrap_or_default();
        let (cle, numero) = (texte("salarie"), texte("numero"));
        let version = v.get("version").and_then(Value::as_u64).ok_or_else(|| valeur(format!("journal : une version illisible dans la DSN de {}", mois)))?;
        let corrige = Some(texte("mois")).filter(|c| !c.is_empty() && *c != mois);
        let Some(c) = corrige else {
            let o = engagement_de(ctx, &mois, &cle, &numero)?;
            paies.push(paie_de_la_version(&ctx_m, &o, &cle, &numero, &mois, version)?);
            continue;
        };
        let ctx_c = ctx.du_mois(&c).map_err(valeur)?;
        let o = engagement_de(ctx, &c, &cle, &numero)?;
        let apres = paie_de_la_version(&ctx_c, &o, &cle, &numero, &c, version)?;
        let dernier = crate::versement_declare::dernier_declare(&ctx_c, &o, &avant, &c, &mois).map_err(valeur)?;
        // Le jour de la correction : celui ou la version qu'elle emporte a ete validee.
        let decidee_le = valide_le(&o, version);
        regs.push(match dernier {
            Some(d) => crate::dsn::Regularisation {
                mois: c,
                avant: d.avant,
                taux_pas_origine: d.versement.taux_pas.clone(),
                declare: d.versement,
                apres,
                net_deja_verse: d.net_deja_verse,
                decidee_le,
            },
            // Un contrat ajoute apres la limite : rien n'etait declare ni verse pour lui.
            None => crate::dsn::Regularisation {
                mois: c.clone(),
                avant: apres.clone(),
                declare: crate::versement_declare::VersementDeclare {
                    fichier: String::new(),
                    net_imposable: socle::num::Decimal::ZERO,
                    net_verse: socle::num::Decimal::ZERO,
                    net_social: socle::num::Decimal::ZERO,
                    rappels_net_social: socle::num::Decimal::ZERO,
                    frais: socle::num::Decimal::ZERO,
                    brut: socle::num::Decimal::ZERO,
                    lignes: Vec::new(),
                    affiliations: Vec::new(),
                    taux_pas: None,
                    debut_contrat: None,
                },
                apres,
                taux_pas_origine: None,
                net_deja_verse: socle::num::Decimal::ZERO,
                decidee_le,
            },
        });
    }
    crate::dsn::fabriquer(ctx, &paies, &mois, &regs, &[], &forme.options()).map_err(valeur)
}

/// LES DSN MENSUELLES PARTIES de `mois` pour l'etablissement `nic` (`Null` : le principal), reelles et acceptees, dans
/// l'ordre de leur rang, chacune fabriquee de nouveau telle qu'elle est partie (`dsn_partie`) ; un annule-et-remplace
/// sait ainsi si celle qu'il remplacait versait a la DGFiP.
fn dsn_parties(ctx: &Contexte, m: &mut Moteur, mois: &str, nic: &Value) -> R<Vec<crate::dsn::Fabriquee>> {
    let lignes = m.lignes()?;
    let mut faits: Vec<(i64, usize)> = Vec::new();
    for l in m.depots_reels(mois, nic)? {
        let Some(rang) = lignes.iter().position(|x| *x == l) else { continue };
        faits.push((m.ordre_de_la_ligne(mois, &l)?, rang));
    }
    faits.sort();
    let mut out: Vec<crate::dsn::Fabriquee> = Vec::new();
    for (_, rang) in faits {
        let versait = out.last().map(verse_a_la_dgfip);
        out.push(dsn_partie(ctx, m, &lignes, rang, versait)?);
    }
    Ok(out)
}

/// CE QU'UNE DSN DE `mois` FAIT VERSER, pour l'etablissement `nic` (le principal sans lui) : la derniere DSN mensuelle
/// partie du mois, telle qu'elle est partie (ses paies et les corrections d'autres mois qu'elle portait, `dsn_parties`) ;
/// si aucune n'est partie, celle qui partirait aujourd'hui (`fabriquer_du_present`). Rend aussi si elle est partie.
/// `None` : rien n'est parti et rien n'est a declarer pour cet etablissement (ni paie validee, ni correction).
pub(crate) fn dsn_du_mois(ctx: &Contexte, mois: &str, nic: Option<&str>) -> R<Option<(crate::dsn::Fabriquee, bool)>> {
    let mut m = Moteur::nouveau(ctx);
    let plusieurs = !droit::bareme::secondaires(&ctx.entreprise).is_empty();
    let nic = nic.filter(|n| !n.is_empty()).map(str::to_string).unwrap_or_else(|| droit::bareme::nic_principal(&ctx.entreprise));
    let nic_v = if plusieurs { Value::String(nic.clone()) } else { Value::Null };
    if let Some(f) = dsn_parties(ctx, &mut m, mois, &nic_v)?.pop() {
        return Ok(Some((f, true)));
    }
    let forme = Forme { type_dsn: "01".into(), ordre: 1, etablissement: plusieurs.then_some(nic), constitution: ctx.aujourdhui, remplacee_versait_pas: None };
    let (f, vide) = fabriquer_du_present(ctx, mois, &forme)?;
    if vide {
        return Ok(None);
    }
    Ok(Some((f, false)))
}

struct Lot {
    nic: Option<String>,
    paies: Vec<usize>,
    regs: Vec<usize>,
    arrets: Vec<Value>,
}

/// La DSN du mois, telle que le depot la prend.
pub(crate) struct DsnFabriquee {
    /// Une DSN par etablissement, le principal d'abord.
    pub(crate) fichiers: Vec<PathBuf>,
    /// Les bulletins rectificatifs des corrections qu'elle porte.
    pub(crate) rectificatifs: Vec<PathBuf>,
    pub(crate) type_dsn: String,
    pub(crate) ordre: i64,
    /// Rien n'a change depuis la DSN deja ecrite : c'est elle qui part.
    pub(crate) reprise: bool,
    pub(crate) avertissements: Vec<Avis>,
}

impl DsnFabriquee {
    pub(crate) fn en_json(&self, mois: &str) -> Value {
        let decrire = |f: &PathBuf| {
            json!({
                "nom": f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
                "chemin": f.to_string_lossy(),
                "octets": std::fs::metadata(f).ok().map(|m| m.len()),
            })
        };
        json!({
            "mois": mois,
            "chemin": self.fichiers.first().map(|f| f.to_string_lossy().into_owned()),
            "fichiers": self.fichiers.iter().map(decrire).collect::<Vec<_>>(),
            "rectificatifs": self.rectificatifs.iter().map(decrire).collect::<Vec<_>>(),
            "type": self.type_dsn,
            "ordre": self.ordre,
            "remplace": self.type_dsn == "03",
            "reprise": self.reprise,
            "avertissements": self.avertissements.iter().filter(|a| !a.info).map(|a| a.texte.clone()).collect::<Vec<_>>(),
            "informations": self.avertissements.iter().filter(|a| a.info).map(|a| a.texte.clone()).collect::<Vec<_>>(),
        })
    }
}

/// La route `dsn-a-deposer` : `{"mois"}`. La DSN du mois, fabriquee depuis ses engagements valides, prete a deposer.
pub fn dsn_a_deposer(ctx: &Contexte, charge: &Value) -> R<Value> {
    let mois = valeur_en_texte(charge.get("mois").ok_or_else(|| erreur("KeyError", socle::fichiers::absent("mois")))?);
    Ok(fabriquer_dsn(ctx, &mois, "interface")?.en_json(&mois))
}

/// LA DSN DU MOIS, A SON DEPOT : une par etablissement, depuis les paies ecrites des engagements valides du mois, avec
/// les corrections des mois deja declares qu'elle porte, les arrets qu'elle reporte, les suspensions, l'OETH d'avril et
/// la CSA de mars. Un mois deja declare repart en annule-et-remplace, sous un nom a lui (`_AR<ordre>`) : le fichier
/// depose reste tel qu'il est parti. Un fichier deja ecrit qui dit exactement la meme chose est repris tel quel : le test
/// qu'il a passe vaut pour lui.
///
/// Refus : un mois dont aucun engagement n'est valide alors qu'il en a (rien ne part d'un engagement en saisie) ; un mois
/// deja declare dont l'annule-et-remplace n'est plus admis.
pub(crate) fn fabriquer_dsn(ctx: &Contexte, mois: &str, qui: &str) -> R<DsnFabriquee> {
    let _verrou = documents::ecrire_documents::prendre_le_verrou(ctx, qui)?;
    let numero_mois: usize = mois.get(5..7).and_then(|a| a.parse().ok()).ok_or_else(|| valeur(format!("mois illisible : {}", socle::fichiers::repr(mois))))?;
    socle::date::nom_du_mois(numero_mois as i64, socle::date::Graphie::Titre).map_err(valeur)?;
    crate::routes_ecritures::refuser_si_clos(ctx, mois)?;
    let mut avertissements: Vec<Avis> = Vec::new();

    // RIEN NE PART D'UN ENGAGEMENT EN SAISIE : la DSN declare les engagements valides du mois.
    let valides = engagement::etats::valides_du_mois(ctx, mois).map_err(Echec::from)?;
    let en_saisie = valides.iter().filter(|v| !**v).count();
    if en_saisie > 0 && en_saisie == valides.len() {
        return Err(erreur(
            "EngagementEnSaisie",
            format!(
                "{} : aucun engagement n'est valide. Rien ne part d'un engagement en saisie : le valider d'abord (« Valider l'engagement »). Rien n'est ecrit.",
                engagement::libelle_mois(mois)
            ),
        ));
    }
    if en_saisie > 0 {
        avertissements.push(avertissement(format!(
            "{} engagement(s) de {} encore en saisie : la DSN ne les declare pas. Une fois valides, la DSN se refait.",
            en_saisie,
            engagement::libelle_mois(mois)
        )));
    }
    let (_data, paies) = documents::ecrire_documents::paies_aux_numeros_declares(ctx, mois, None)?;
    let (_urssaf, avert) = crate::routes_ouverture::urssaf_de_l_apercu(ctx, &paies, mois)?;
    avertissements.extend(avert.into_iter().map(avertissement));
    avertissements.extend(crate::routes_ouverture::archives_employees(&paies)?.into_iter().map(avertissement));
    for p in &paies {
        avertissements.extend(p.avertissements.iter().cloned());
    }
    for p in &paies {
        if p.engagement.sans_paie() {
            avertissements.push(info(format!(
                "{} {} : contrat {} en cours, aucun cachet en {} -- la DSN porte le contrat avec un versement a zero.",
                valeur_en_texte(p.salarie.get("nom").unwrap_or(&Value::Null)),
                prenom_de(&p.salarie),
                p.engagement.numero.as_deref().unwrap_or("None"),
                engagement::libelle_mois(mois)
            )));
        }
    }

    let fiche = ctx.entreprise.clone();
    let principal = droit::bareme::nic_principal(&fiche);
    let secondaires: Vec<String> = droit::bareme::secondaires(&fiche).iter().map(|e| valeur_en_texte(e.get("nic").unwrap_or(&Value::Null))).collect();
    let plusieurs = !secondaires.is_empty();
    let mut m = Moteur::nouveau(ctx);
    // Un mois deja declare dont l'annule-et-remplace n'est plus admis : refuse AVANT d'ecrire quoi que ce soit.
    let nics: Vec<Value> = if plusieurs { std::iter::once(principal.clone()).chain(secondaires.iter().cloned()).map(Value::String).collect() } else { vec![Value::Null] };
    for nic in &nics {
        forme_de_la_dsn(ctx, &mut m, mois, nic)?;
    }
    let etiq = socle::date::etiquette_du_mois(mois).map_err(valeur)?;
    let sortie = ctx.racine.join("sorties").join(mois);

    // LES CORRECTIONS DES MOIS DEJA DECLARES : la DSN de ce mois les porte, datees de leur mois d'origine.
    let regs = regularisations::regularisations_pour(ctx, mois)?;
    for ligne in regularisations::resume_regularisations(&regs)? {
        avertissements.push(info(format!(
            "Regularisation embarquee dans la DSN : {}. La difference de net se paie -- ou se retient -- ce mois-ci, par un bulletin rectificatif.",
            ligne
        )));
    }
    if paies.is_empty() && regs.is_empty() {
        avertissements.push(info(format!(
            "Aucun salarie en {} : la DSN part quand meme, sans individu (envoi neant) -- une DSN est due chaque mois.",
            engagement::libelle_mois(mois)
        )));
    }
    // LA DECLARATION OETH D'AVRIL : elle ajoute au bordereau Urssaf une contribution (CTP 730 ou 740).
    if let Some(doeth) = crate::oeth::declaration(ctx, mois).map_err(valeur)? {
        avertissements.push(info(format!("{} Le bordereau Urssaf du mois en porte {} EUR.", doeth.resume(), doeth.a_verser())));
    }
    // LA CONTRIBUTION SUPPLEMENTAIRE A L'APPRENTISSAGE DE MARS : le bloc 82 (079 ou 080) et le CTP 998 (`csa`).
    if let Some(c) = crate::csa::declaration(ctx, mois).map_err(valeur)? {
        avertissements.push(info(c.resume()));
    }
    // LES ARRETS DE TRAVAIL : la DSN mensuelle doit REPORTER ceux dont la periode croise le mois, signales ou non.
    let arrets = crate::marques_depot::arrets_pour(ctx, mois)?;
    let nomenclature = crate::routes_listes::nomenclatures(ctx)?;
    let jour = |v: &Value| -> R<String> { documents::bulletin::jj_mm_aaaa(v).map_err(valeur) };
    let mut non_signales = 0;
    for a in &arrets {
        let resume = crate::routes_listes::resume_arret(&nomenclature, a)?;
        let reprise = a.get("reprise").filter(|r| vrai(r));
        let fin = match reprise {
            Some(r) => r.clone(),
            None => a.get("fin_previsionnelle").cloned().unwrap_or(Value::Null),
        };
        avertissements.push(info(format!(
            "Arret de travail reporte dans la DSN : {}, {}, du {} au {}{}.",
            valeur_en_texte(a.get("salarie").unwrap_or(&Value::Null)),
            valeur_en_texte(&resume["motif_libelle"]),
            jour(a.get("dernier_jour_travaille").unwrap_or(&Value::Null))?,
            jour(&fin)?,
            if reprise.is_some() { " (reprise)" } else { " (fin previsionnelle)" }
        )));
        if !resume.get("signale").is_some_and(vrai) {
            non_signales += 1;
        }
    }
    // LES SUSPENSIONS (bloc 65) : chacune se declare entiere dans la DSN de chaque mois qu'elle traverse.
    for s in engagement::imprevus::du_mois(&ctx.racine, mois, Some(engagement::imprevus::SUSPENSION)).map_err(crate::routes_listes::refus_magasin)? {
        avertissements.push(info(format!(
            "Suspension declaree dans la DSN : {}, motif {}, du {} au {}.",
            valeur_en_texte(&s["salarie"]),
            valeur_en_texte(&s["motif"]),
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
            let du_nic = |sal: &Value| -> R<bool> { Ok(droit::bareme::etablissement_du_salarie(&fiche, sal).map_err(valeur)? == nic) };
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
                let sal = ctx.salarie(&valeur_en_texte(a.get("salarie").unwrap_or(&Value::Null))).map_err(valeur)?;
                if du_nic(&sal)? {
                    lot.arrets.push(a.clone());
                }
            }
            lots.push(lot);
        }
    } else {
        lots.push(Lot { nic: None, paies: (0..paies.len()).collect(), regs: (0..regs.len()).collect(), arrets: arrets.clone() });
    }
    let mut a_ecrire: Vec<(PathBuf, String)> = Vec::new();
    let mut fichiers: Vec<PathBuf> = Vec::new();
    let (mut type_principal, mut ordre_principal) = ("01".to_string(), 1);
    for (rang, lot) in lots.iter().enumerate() {
        let mut nom = match &lot.nic {
            Some(n) if *n != principal => format!("DSN_{}_{}.dsn", etiq, n),
            _ => format!("DSN_{}.dsn", etiq),
        };
        let nic = lot.nic.clone().map_or(Value::Null, Value::String);
        let (type_dsn, ordre_dsn, remplace) = forme_de_la_dsn(ctx, &mut m, mois, &nic)?;
        let mut remplacee_versait_pas = None;
        if remplace {
            remplacee_versait_pas = remplacee_versait_pas_de(ctx, &mut m, mois, &nic)?;
            nom = format!("{}_AR{}.dsn", &nom[..nom.len() - ".dsn".len()], ordre_dsn);
            avertissements.push(info(format!(
                "La DSN de {} est deja partie : celle-ci est un ANNULE-ET-REMPLACE integral (ordre {}). Elle remplace la precedente, ordres de telepaiement compris, et s'envoie au plus tard le {} a minuit.",
                engagement::libelle_mois(mois),
                ordre_dsn,
                m.limite(mois)?.jj_mm_aaaa()
            )));
        }
        if rang == 0 {
            (type_principal, ordre_principal) = (type_dsn.clone(), ordre_dsn);
        }
        let paies_e: Vec<Paie> = lot.paies.iter().map(|i| paies[*i].clone()).collect();
        let regs_e: Vec<&Reg> = lot.regs.iter().map(|i| &regs[*i]).collect();
        let regs_dsn = regularisations::pour_la_dsn(&regs_e);
        let arrets_e: Vec<crate::dsn::Arret> = lot.arrets.iter().map(|v| crate::dsn::Arret { v: v.clone() }).collect();
        let f_dsn = sortie.join(&nom);
        let generer = |constitution: socle::date::Jour| -> R<String> {
            let options = crate::dsn::Options {
                date_constitution: Some(constitution),
                envoi_test: false,
                type_declaration: type_dsn.clone(),
                ordre: ordre_dsn,
                etablissement: lot.nic.clone(),
                remplacee_versait_pas,
                ..Default::default()
            };
            crate::dsn::generer(ctx, &paies_e, mois, &regs_dsn, &arrets_e, &options).map_err(valeur)
        };
        // LE FICHIER DEJA ECRIT QUI DIT LA MEME CHOSE se reprend : meme declaration, au jour ou il a ete constitue.
        let deja = constitution_de(&f_dsn).and_then(|jour| generer(jour).ok()).filter(|t| std::fs::read(&f_dsn).ok() == crate::dsn::en_latin1(t).ok());
        if deja.is_none() {
            a_ecrire.push((f_dsn.clone(), generer(ctx.aujourdhui)?));
        }
        fichiers.push(f_dsn);
    }
    let reprise = a_ecrire.is_empty();
    let mut rectificatifs = Vec::new();
    if !reprise {
        std::fs::create_dir_all(&sortie).map_err(|e| os(&sortie, e))?;
        for (f, texte) in &a_ecrire {
            crate::dsn::ecrire(texte, f).map_err(|m| erreur("OSError", m))?;
        }
        if !regs.is_empty() {
            let mut reserves = Reserves::default();
            let rectifs = regularisations::ecrire_rectificatifs(ctx, &regs, mois, &sortie, &mut reserves)?;
            rectificatifs = rectifs.fichiers;
            avertissements.extend(rectifs.avertissements.into_iter().map(avertissement));
            reserves.valider(ctx)?;
        }
        // La paie que chaque DSN declare : l'ecran dira si elle change avant l'envoi.
        let empreinte = m.empreinte_saisie(mois)?;
        for (f, _) in &a_ecrire {
            noter_piece_ecrite(&ctx.racine, &cle_de_piece(f), Some(json!(empreinte)))?;
        }
    }
    Ok(DsnFabriquee { fichiers, rectificatifs, type_dsn: type_principal, ordre: ordre_principal, reprise, avertissements })
}

// ------------------------------------------------------------- les documents, a leur envoi

/// Les documents d'un mois, fabriques pour ceux a qui ils partent : leurs numeros sont reserves, pas encore ecrits.
pub(crate) struct DocumentsFabriques {
    _verrou: socle::verrou::Verrou,
    reserves: Reserves,
    /// `(cle du salarie, debut, jumeau, numero de bulletin, premier N CE)` : ce qui s'ecrit dans les paies au depart.
    a_poser: Vec<(String, String, usize, Option<String>, Option<i64>)>,
    pub(crate) avertissements: Vec<String>,
}

impl DocumentsFabriques {
    /// LES DOCUMENTS SONT PARTIS : leurs numeros s'ecrivent dans les paies des engagements, et les compteurs passent
    /// au-dessus.
    pub(crate) fn engager(self, ctx: &Contexte, mois: &str) -> R<()> {
        self.reserves.valider(ctx)?;
        for (cle, debut, jumeau, bulletin, ce) in &self.a_poser {
            engagement::etats::poser_numeros_des_documents(ctx, mois, cle, debut, *jumeau, bulletin.as_deref(), *ce).map_err(Echec::from)?;
        }
        Ok(())
    }
}

/// Le numero d'un bulletin dans un nom de fichier : `12/2026` -> `12-2026`.
fn numero_en_nom(n: &str) -> String {
    n.replace('/', "-")
}

/// LES DOCUMENTS DU MOIS, FABRIQUES A LEUR ENVOI, pour les salaries `cles` : depuis les paies ecrites des engagements
/// valides, un `Bulletin_<cle>_<Mois>.pdf` (tous ses bulletins du mois) et un `CertificatCS_<cle>_<Mois>.pdf` par salarie,
/// dans `sorties/<mois>/envoi/`. Un bulletin parti garde son numero ; un bulletin qui part pour la premiere fois prend le
/// suivant de l'annee ; une paie partie puis refaite part sous un nouveau numero, imprimee comme le rectificatif de celle
/// qui est partie, et le bulletin d'origine se garde a cote (`sorties/<mois>/Bulletin_<cle>_<Mois>_<numero>.pdf`). Les N CE
/// suivent la sequence continue des certificats. L'archive du mois (`Bulletins_<Mois>.pdf`, `CertificatsCS_<Mois>.pdf` et
/// son recapitulatif) porte tous les documents numerotes du mois.
pub(crate) fn fabriquer_documents(ctx: &Contexte, mois: &str, cles: &[String], qui: &str) -> R<DocumentsFabriques> {
    let verrou = documents::ecrire_documents::prendre_le_verrou(ctx, qui)?;
    let mut faits = DocumentsFabriques { _verrou: verrou, reserves: Reserves::default(), a_poser: Vec::new(), avertissements: Vec::new() };
    let Some((_data, mut paies)) = engagement::etats::validees_du_mois(ctx, mois).map_err(Echec::from)? else { return Ok(faits) };
    let ecrits = engagement::etats::numeros_des_documents(ctx, mois).map_err(Echec::from)?;
    let an = annee_de(mois)?;
    // Les numeros de bulletin que le compteur propose ; chaque contrat porte deja le sien.
    documents::ecrire_documents::numeroter(ctx, &mut paies, an as i32)?;
    let etiq = socle::date::etiquette_du_mois(mois).map_err(valeur)?;
    let sortie = ctx.racine.join("sorties").join(mois);
    let dossier = sortie.join("envoi");
    std::fs::create_dir_all(&dossier).map_err(|e| os(&dossier, e))?;
    let ent = ctx.entreprise_du_mois(mois).map_err(valeur)?;
    let part = |p: &Paie| cles.contains(&cle_de(p));
    let jumeau = |i: usize| (0..i).filter(|j| cle_de(&paies[*j]) == cle_de(&paies[i]) && paies[*j].engagement.debut == paies[i].engagement.debut).count();
    let jumeaux: Vec<usize> = (0..paies.len()).map(jumeau).collect();
    let mut a_poser: HashMap<usize, (Option<String>, Option<i64>)> = HashMap::new();

    // LES BULLETINS : un numero par paie qui a des cachets, ecrit dans la paie a son depart.
    let mut numerotees: Vec<bool> = vec![false; paies.len()];
    for i in 0..paies.len() {
        if paies[i].engagement.sans_paie() {
            continue;
        }
        if let Some(b) = ecrits.get(i).and_then(|e| e.bulletin.clone()) {
            paies[i].numero_bulletin = Some(b);
            numerotees[i] = true;
        } else if part(&paies[i]) {
            let n = faits.reserves.prochain(ctx, "bulletin", an)?;
            faits.reserves.enregistrer(an, "bulletin", n);
            let numero = format!("{}/{}", n, an);
            paies[i].numero_bulletin = Some(numero.clone());
            a_poser.entry(i).or_default().0 = Some(numero);
            numerotees[i] = true;
        }
    }
    let rectificatifs: HashMap<usize, Value> = (0..paies.len())
        .filter_map(|i| {
            let r = ecrits.get(i)?.rectifie.clone()?;
            Some((i, json!({"remplace": true, "rectifie": r, "mois_origine": engagement::libelle_mois(mois)})))
        })
        .collect();

    // LES CERTIFICATS CONGES SPECTACLES : un par contrat qui finit ce mois-ci, numerotes dans la sequence continue.
    let (pour_cs, avert_cs) = crate::routes_dossier_salarie::certificats_dus(ctx, &paies)?;
    faits.avertissements.extend(avert_cs);
    let mut prochain_ce: Option<i64> = None;
    let mut certificats: Vec<(usize, &PaieCs, i64)> = Vec::new();
    for (cle, debut, pc) in &pour_cs {
        let Some(pc) = pc else { continue };
        let Some(i) = (0..paies.len()).find(|i| cle_de(&paies[*i]) == *cle && paies[*i].engagement.debut.iso() == *debut) else { continue };
        let n = match ecrits.get(i).and_then(|e| e.premier_ce) {
            Some(n) => n,
            None if part(&paies[i]) => {
                let n = match prochain_ce {
                    Some(n) => n,
                    None => documents::ecrire_documents::prochain_continu(ctx, "certificat_cs", 1)?,
                };
                let combien = conges_spectacles::decouper(&pc.engagement)?.len() as i64;
                prochain_ce = Some(n + combien);
                faits.reserves.enregistrer(an, "certificat_cs", n + combien - 1);
                a_poser.entry(i).or_default().1 = Some(n);
                if ctx.salarie(cle).map_err(valeur)?.get("nir").filter(|x| vrai(x)).map(valeur_en_texte).unwrap_or_default().trim().is_empty() {
                    faits.avertissements.push(format!(
                        "{} {} : certificat Conges Spectacles emis SANS numero de securite sociale -- Audiens ne pourra pas le rattacher.",
                        valeur_en_texte(paies[i].salarie.get("nom").unwrap_or(&Value::Null)),
                        prenom_de(&paies[i].salarie)
                    ));
                }
                n
            }
            None => continue,
        };
        certificats.push((i, pc, n));
    }

    // UN JEU DE DOCUMENTS PAR SALARIE QUI LES RECOIT.
    let mut vus: Vec<String> = Vec::new();
    for p in paies.iter().filter(|p| part(p)) {
        let cle = cle_de(p);
        if vus.contains(&cle) {
            continue;
        }
        vus.push(cle.clone());
        let siennes: Vec<usize> = (0..paies.len()).filter(|i| cle_de(&paies[*i]) == cle && numerotees[*i]).collect();
        if !siennes.is_empty() {
            let piece = dossier.join(crate::envoi::nom_fichier("Bulletin", &cle, &etiq));
            // LE BULLETIN D'ORIGINE RESTE : une paie refaite part sous un nouveau numero, et ce qui est parti se garde.
            for i in &siennes {
                if let (Some(r), true) = (ecrits[*i].rectifie.as_deref(), a_poser.get(i).is_some_and(|x| x.0.is_some())) {
                    let garde = sortie.join(format!("Bulletin_{}_{}_{}.pdf", cle, etiq, numero_en_nom(r)));
                    if piece.exists() && !garde.exists() {
                        std::fs::copy(&piece, &garde).map_err(|e| os(&garde, e))?;
                    }
                }
            }
            let imprimees: Vec<Paie> = siennes.iter().map(|i| paies[*i].clone()).collect();
            let rectifs: HashMap<usize, Value> = siennes.iter().enumerate().filter_map(|(k, i)| rectificatifs.get(i).map(|r| (k, r.clone()))).collect();
            let description = documents::bulletin::jeux_et_description(ctx, &imprimees, &ent, mois, Some(&paies), false, &rectifs).map_err(valeur)?;
            documents::fabrique::ecrire_pdf(&description, &piece)?;
        }
        let ses_certificats: Vec<(&PaieCs, i64)> = certificats.iter().filter(|(i, _, _)| cle_de(&paies[*i]) == cle).map(|(_, pc, n)| (*pc, *n)).collect();
        if !ses_certificats.is_empty() {
            let description = conges_spectacles::description_numerotee(&ses_certificats, &ent).map_err(Echec::from)?;
            documents::fabrique::ecrire_pdf(&description, &dossier.join(crate::envoi::nom_fichier("CertificatCS", &cle, &etiq)))?;
        }
    }

    // L'ARCHIVE DU MOIS : tous ses documents numerotes.
    let archivees: Vec<usize> = (0..paies.len()).filter(|i| numerotees[*i]).collect();
    if !archivees.is_empty() {
        let imprimees: Vec<Paie> = archivees.iter().map(|i| paies[*i].clone()).collect();
        let rectifs: HashMap<usize, Value> = archivees.iter().enumerate().filter_map(|(k, i)| rectificatifs.get(i).map(|r| (k, r.clone()))).collect();
        let description = documents::bulletin::jeux_et_description(ctx, &imprimees, &ent, mois, None, false, &rectifs).map_err(valeur)?;
        documents::fabrique::ecrire_pdf(&description, &sortie.join(format!("Bulletins_{}.pdf", etiq)))?;
    }
    if !certificats.is_empty() {
        let mut paires: Vec<(&PaieCs, i64)> = certificats.iter().map(|(_, pc, n)| (*pc, *n)).collect();
        paires.sort_by_key(|(_, n)| *n);
        let description = conges_spectacles::description_numerotee(&paires, &ent).map_err(Echec::from)?;
        documents::fabrique::ecrire_pdf(&description, &sortie.join(format!("CertificatsCS_{}.pdf", etiq)))?;
        let recap = sortie.join(format!("CertificatsCS_{}_recapitulatif.txt", etiq));
        let texte = conges_spectacles::recapitulatif(&paires, &ent).map_err(Echec::from)?;
        std::fs::write(&recap, texte).map_err(|e| os(&recap, e))?;
    }
    let mut poses: Vec<(usize, (Option<String>, Option<i64>))> = a_poser.into_iter().collect();
    poses.sort_by_key(|(i, _)| *i);
    faits.a_poser = poses.into_iter().map(|(i, (b, ce))| (cle_de(&paies[i]), paies[i].engagement.debut.iso(), jumeaux[i], b, ce)).collect();
    Ok(faits)
}

//! Le contrat ecrit, tel qu'il s'ecrit : le contrat d'engagement d'un spectacle, le CDD et le CDI d'un permanent, et
//! leurs avenants.
//!
//! Le texte du contrat n'est PAS dans ce module : il vit dans le modele de sa nature (`contrat_ecrit.modele` de
//! `contrats-base.json` : `contrat-cddu.modele`, `contrat-cdd.modele`, `contrat-cdi.modele`), un fichier de donnees, que la
//! compagnie remplace par le sien dans son dossier `gabarits/`. Ce module envoie a la fabrique le TEXTE du modele (lu
//! maintenant : `fabrique.figer`) et les VALEURS de la paie (`avenant::valeurs_de_la_paie`) ; c'est elle qui le met en
//! page. Un modele qui tairait une mention que la loi exige se refuse (`contrat_ecrit::refuser_les_mentions_absentes`).

use std::path::Path;

use serde_json::{json, Map, Value};

use crate::avenant::valeurs_de_la_paie;
use crate::bulletin::{identite_de, FORMAT, LOGO_LARGEUR, LOGO_MARGE_DROITE, LOGO_MARGE_HAUT};
use engagement::calcul::Paie;
use crate::conges_spectacles::chemin_texte;
use droit::contexte::Contexte;
use socle::echec::Refus;
use fabrique_pdf::modele::Valeurs;

type R<T> = Result<T, Refus>;


/// Le haut d'une description : le format, le style, le logo de la compagnie.
pub fn enveloppe(style: &str, ent: &Value) -> Map<String, Value> {
    let mut d = Map::new();
    d.insert("format".into(), json!(FORMAT));
    d.insert("style".into(), json!(style));
    if let Some(logo) = identite_de(ent, "logo") {
        d.insert(
            "logo".into(),
            json!({"chemin": chemin_texte(&logo), "largeur": LOGO_LARGEUR, "marge_haut": LOGO_MARGE_HAUT,
                   "marge_droite": LOGO_MARGE_DROITE}),
        );
    }
    d
}

/// `fabrique.figer` pour un modele : le fichier lu en texte.
pub fn lire_modele_texte(chemin: &Path) -> R<String> {
    let octets = std::fs::read(chemin).map_err(|e| {
        let classe = if e.kind() == std::io::ErrorKind::NotFound { "FileNotFoundError" } else { "OSError" };
        Refus { classe: Some(classe), message: socle::fichiers::panne_fichier(&e, chemin) }
    })?;
    String::from_utf8(octets).map_err(|e| Refus {
        classe: Some("UnicodeDecodeError"),
        message: format!("'utf-8' codec can't decode bytes: {}", e.utf8_error()),
    })
}

/// L'ordre des cles de `contrat.marqueurs` : celui du modele. `Valeurs` est trie ; la description
/// gardee a cote du PDF (`fabrique.garder_description`) se relit, et s'ecrit dans le meme ordre.
const ORDRE_MARQUEURS: [&str; 38] = [
    "employeur.raison_sociale",
    "employeur.adresse",
    "employeur.code_postal",
    "employeur.commune",
    "employeur.siret",
    "employeur.naf",
    "employeur.licence",
    "employeur.idcc",
    "employeur.convention",
    "employeur.congesspectacles",
    "employeur.signataire",
    "employeur.signataire_fonction",
    "employeur.qualite_de",
    "salarie.nom",
    "salarie.prenom",
    "salarie.nom_complet",
    "salarie.date_naissance",
    "salarie.lieu_naissance",
    "salarie.adresse",
    "salarie.code_postal",
    "salarie.commune",
    "salarie.nir",
    "salarie.emploi",
    "salarie.qualite_de",
    "contrat.numero",
    "contrat.debut",
    "contrat.fin",
    "contrat.dates_travail",
    "contrat.lieu",
    "contrat.date_paiement",
    "remuneration.detail",
    "remuneration.periode",
    "remuneration.cachets",
    "remuneration.taux_cachet",
    "remuneration.brut",
    "remuneration.net_estime",
    "spectacle.titre",
    "spectacle.numero_objet",
];

/// Les valeurs d'un modele en JSON, dans l'ordre du modele : celles de `marqueurs` d'abord, puis les autres
/// (l'avenant ajoute les siennes) dans l'ordre ou elles sont donnees.
pub fn valeurs_en_ordre(v: &Valeurs, ajouts: &[(&str, String)]) -> Value {
    let mut m = Map::new();
    for k in ORDRE_MARQUEURS.iter() {
        if let Some(x) = v.get(*k) {
            m.insert((*k).to_string(), json!(x));
        }
    }
    for (k, x) in v {
        if !m.contains_key(k) {
            m.insert(k.clone(), json!(x));
        }
    }
    for (k, x) in ajouts {
        m.insert((*k).to_string(), json!(x));
    }
    Value::Object(m)
}

/// Les valeurs que le modele lit dans cette paie. Les montants s'ecrivent comme
/// le modele les imprime (`ECHELLE_GARDEE`).
pub fn valeurs_de_paie(ctx: &Contexte, ent: &Value, p: &Paie) -> R<Valeurs> {
    socle::num::ECHELLE_GARDEE.with(|f| f.set(true));
    let r = (|| -> R<Valeurs> {
        let canon = p.canon(ctx)?;
        Ok(valeurs_de_la_paie(ctx, ent, &canon, None)?)
    })();
    socle::num::ECHELLE_GARDEE.with(|f| f.set(false));
    r
}

/// Les contrats qui en annulent un autre : `{(salarie, premier jour, numero du contrat): (numero remplace, date)}`.
pub type Remplacements = [((String, String, String), (String, String))];

/// `contrat.description` puis `fabrique.figer` : la description de ces contrats, un document par
/// paie, le modele en texte. `remplacements` : ces contrats-la annulent et remplacent celui qui est parti avant
/// eux, et le disent en tete.
pub fn description(ctx: &Contexte, paies: &[Paie], ent: &Value, remplacements: &Remplacements) -> R<Value> {
    let mut d = enveloppe("contrat", ent);
    let signature = identite_de(ent, "signature");
    let mut documents = Vec::new();
    for p in paies {
        let nat = p.engagement.nat()?;
        let modele = socle::fichiers::modele_de(&crate::contrat_ecrit::nom_du_modele(&nat)?, ent, &ctx.gabarits());
        let texte = lire_modele_texte(&modele)?;
        let v = valeurs_de_paie(ctx, ent, p)?;
        if v.values().any(|t| t.contains("VALEUR-SANS-TEXTE")) {
            return Err(Refus::from("description illisible : une valeur du modele de contrat n'est pas un texte"));
        }
        let mut un = Map::new();
        un.insert("valeurs".into(), valeurs_en_ordre(&v, &[]));
        let cle = (
            p.salarie.get("_cle").map(socle::valeur::valeur_en_texte).unwrap_or_default(),
            p.engagement.debut.iso(),
            p.engagement.numero.clone().unwrap_or_default(),
        );
        if let Some((_, (numero, date))) = remplacements.iter().find(|(k, _)| *k == cle) {
            un.insert("remplace".into(), json!({"numero": numero, "date": date}));
        }
        if let Some(s) = &signature {
            un.insert("signature".into(), json!(chemin_texte(s)));
        }
        refuser_ce_que_le_modele_tairait(&modele, &texte, &un["valeurs"])?;
        crate::contrat_ecrit::refuser_les_mentions_absentes(&modele, &texte, &nat, &un["valeurs"])
            .map_err(|m| Refus { classe: Some("ModeleIncomplet"), message: m })?;
        un.insert("texte".into(), json!(texte));
        documents.push(json!({ "modele": Value::Object(un) }));
    }
    d.insert("documents".into(), Value::Array(documents));
    Ok(Value::Object(d))
}

/// Les valeurs qu'un modele de contrat DOIT citer des qu'elles ne sont pas vides. `remuneration.detail` ne cite que
/// ce qui fait le brut ; ce qui est verse en plus (les defraiements) n'est que dans `remuneration.hors_brut`.
const A_CITER: [&str; 1] = ["remuneration.hors_brut"];

/// Le modele porte-t-il le marqueur `cle` hors de ses commentaires (les lignes en `#`, que la fabrique saute) ?
pub(crate) fn cite(texte: &str, cle: &str) -> bool {
    texte.lines().filter(|l| !l.trim_start().starts_with('#')).any(|l| {
        l.match_indices("{{").any(|(i, _)| fabrique_pdf::modele::marqueur_en(l, i).is_some_and(|(k, _)| k == cle))
    })
}

/// REGLE 1 : un gabarit de compagnie qui n'a pas le marqueur d'une valeur a citer ne fait pas un contrat qui la tait
/// (un marqueur INCONNU, la fabrique le refuse deja ; c'est l'inverse : une valeur sans sa place). Une valeur vide
/// passe : le modele qui l'omet dit alors tout ce qu'il y a a dire.
///
/// L'inverse aussi, pour le taux du cachet (REGLE 1) : un contrat qui ne paie aucun cachet, et dont ni
/// l'engagement ni le mois n'ecrivent de taux, n'en a pas ; le moteur prenait 130 EUR, un chiffre qu'aucun texte ne fixe.
/// Un modele qui cite `{{remuneration.taux_cachet}}` ne fait pas ce contrat-la (la fabrique dirait « marqueur
/// inconnu », sans dire pourquoi).
fn refuser_ce_que_le_modele_tairait(modele: &Path, texte: &str, valeurs: &Value) -> R<()> {
    const TAUX: &str = "remuneration.taux_cachet";
    if valeurs.get(TAUX).is_none() && cite(texte, TAUX) {
        return Err(Refus {
            classe: Some("ModeleIncomplet"),
            message: format!(
                "Le modèle de contrat {} cite {{{{{}}}}}, et le contrat de {} n'a aucun taux de cachet : il ne paie aucun cachet, et ni l'engagement ni le mois n'en écrivent. Le moteur n'en invente pas. Écrire la rémunération de ce modèle par {{{{remuneration.detail}}}}, qui la dit quelle que soit sa forme (le modèle livré le fait), ou porter le taux sur une ligne de cachet de l'engagement. Rien n'est écrit.",
                modele.display(),
                TAUX,
                valeurs.get("salarie.nom_complet").and_then(Value::as_str).unwrap_or("ce salarie"),
            ),
        });
    }
    for cle in A_CITER {
        let valeur = valeurs.get(cle).and_then(Value::as_str).unwrap_or_default();
        if !valeur.trim().is_empty() && !cite(texte, cle) {
            return Err(Refus {
                classe: Some("ModeleIncomplet"),
                message: format!(
                    "Le modèle de contrat {} n'a pas le marqueur {{{{{}}}}} : le contrat de {} tairait ce qui lui est versé hors brut ({}). Ajouter {{{{{}}}}} à l'article « Rémunération » de ce modèle (le modèle livré l'y place juste après {{{{remuneration.periode}}}}). Rien n'est écrit.",
                    modele.display(),
                    cle,
                    valeurs.get("salarie.nom_complet").and_then(Value::as_str).unwrap_or("ce salarie"),
                    valeur.trim().trim_start_matches(',').trim(),
                    cle
                ),
            });
        }
    }
    Ok(())
}

/// Ce que le logiciel depose dans la case de l'artiste, quatre lignes.
pub fn mention_signature(nom: &str, quand: &str, adresse: &str, code: &str) -> String {
    format!("Signé électroniquement le {}\npar {}\n{}\ncode de signature {}", quand, nom, adresse, code)
}

/// Le contrat renvoye au salarie une fois qu'il a signe d'un clic. LA REGENERATION, ET RIEN
/// D'AUTRE : le contrat est refait depuis la description gardee a l'envoi (`fabrique::description_gardee`), la mention
/// de l'artiste dans sa case, puis verrouille (AES-256, impression et copie seules). Sans description valide (absente,
/// illisible, ne decrivant plus ce PDF) on REFUSE au lieu de deviner : il faut renvoyer le contrat. Il refuse aussi si
/// la fabrique refuse la mention. `Err` : la phrase du refus.
pub fn contresigner(source: &Path, sortie: &Path, nom: &str, quand: &str, adresse: &str, code: &str) -> Result<(), String> {
    let fichier = source.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let texte = mention_signature(nom, quand, adresse, code);
    let Some(gardee) = crate::fabrique::description_gardee(source) else {
        return Err(format!(
            "{} ne peut pas etre contresigne : la description gardee a l'envoi est absente, illisible ou ne correspond plus a ce PDF. Renvoyez le contrat (ou l'avenant) a la signature.",
            fichier
        ));
    };
    let regenere = crate::fabrique::avec_mention_artiste(&gardee, &texte)
        .and_then(|d| crate::fabrique::fabriquer_octets(&d))
        .map_err(|e| format!("{} ne peut pas etre contresigne : la fabrique refuse la mention de signature ({}).", fichier, e))?;
    let verrouille = crate::fabrique::verrouiller(&regenere).map_err(|e| format!("{} : {}", fichier, e))?;
    if let Some(parent) = sortie.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{} : {}", parent.display(), e))?;
    }
    std::fs::write(sortie, verrouille).map_err(|e| format!("{} : {}", sortie.display(), e))
}

/// `contrat.ecrire` sans la description gardee : un contrat par page, dans `chemin`.
pub fn ecrire(ctx: &Contexte, paies: &[Paie], ent: &Value, chemin: &Path) -> R<()> {
    let d = description(ctx, paies, ent, &[])?;
    crate::fabrique::ecrire(&d, chemin)
        .map_err(|m| Refus { classe: Some("FabriqueRefus"), message: m.replace('\n', " ").trim().to_string() })?;
    Ok(())
}

/// Le contrat, puis sa description gardee a cote (`fabrique.garder_description`) : la
/// contresignature refait le contrat depuis elle, octet pour octet.
pub fn ecrire_et_garder(ctx: &Contexte, paies: &[Paie], ent: &Value, chemin: &Path, remplacements: &Remplacements) -> R<()> {
    let d = description(ctx, paies, ent, remplacements)?;
    crate::fabrique::ecrire(&d, chemin)
        .map_err(|m| Refus { classe: Some("FabriqueRefus"), message: m.replace('\n', " ").trim().to_string() })?;
    crate::fabrique::garder_description(&d, chemin).map_err(|m| Refus { classe: Some("OSError"), message: m })?;
    Ok(())
}


/// Le modele livre de l'avenant du contrat ecrit de cette nature (`contrat_ecrit.avenant` de `contrats-base.json`).
fn nom_de_l_avenant(nat: &droit::natures::Nature) -> R<String> {
    nat.modele_avenant().ok_or_else(|| Refus::from(format!("La nature {} n'a pas d'avenant écrit.", nat.libelle)))
}

/// `avenant.description` puis `fabrique.figer` : le modele de l'avenant en texte, les valeurs du contrat et les
/// siennes, et les articles modifies avec l'ancien texte et le nouveau.
fn description_avenant(ctx: &Contexte, p: &Paie, ent: &Value, modifs: &[Value], extra: &[(&str, String)]) -> R<Value> {
    let nat = p.engagement.nat()?;
    let modele = socle::fichiers::modele_de(&nom_de_l_avenant(&nat)?, ent, &ctx.gabarits());
    let mut d = enveloppe("avenant", ent);
    let v = valeurs_de_paie(ctx, ent, p)?;
    // Les memes refus que le contrat : ses articles modifies sont ceux du modele de CONTRAT de la compagnie,
    // et un modele qui n'a pas le marqueur d'une valeur a citer ferait un avenant qui la tait.
    if v.values().any(|t| t.contains("VALEUR-SANS-TEXTE")) {
        return Err(Refus::from("description illisible : une valeur du modele de contrat n'est pas un texte"));
    }
    let mut un = Map::new();
    un.insert("valeurs".into(), valeurs_en_ordre(&v, extra));
    let contrat = socle::fichiers::modele_de(&crate::contrat_ecrit::nom_du_modele(&nat)?, ent, &ctx.gabarits());
    let texte_contrat = lire_modele_texte(&contrat)?;
    refuser_ce_que_le_modele_tairait(&contrat, &texte_contrat, &un["valeurs"])?;
    crate::contrat_ecrit::refuser_les_mentions_absentes(&contrat, &texte_contrat, &nat, &un["valeurs"]).map_err(|m| Refus { classe: Some("ModeleIncomplet"), message: m })?;
    let articles: Vec<Value> = modifs
        .iter()
        .map(|m| json!({"numero": m["numero"], "article": m["article"], "avant": m["avant"], "apres": m["apres"]}))
        .collect();
    un.insert("modifications".into(), Value::Array(articles));
    if let Some(s) = identite_de(ent, "signature") {
        un.insert("signature".into(), json!(chemin_texte(&s)));
    }
    un.insert("texte".into(), json!(lire_modele_texte(&modele)?));
    d.insert("documents".into(), json!([{ "modele": Value::Object(un) }]));
    Ok(Value::Object(d))
}

/// L'avenant en PDF, a l'identite de la compagnie, la signature de l'employeur posee comme sur
/// le contrat, puis sa description gardee. Les dates en JJ/MM/AAAA. `modifs` : ce que `avenant-apercu` rend.
#[allow(clippy::too_many_arguments)]
pub fn ecrire_avenant(
    ctx: &Contexte,
    p: &Paie,
    ent: &Value,
    chemin: &Path,
    modifs: &[Value],
    numero: i64,
    date_contrat: &str,
    effet: &str,
    date: &str,
) -> R<()> {
    if modifs.is_empty() {
        return Err(Refus { classe: Some("ValueError"), message: "Rien n'a changé : pas d'avenant à écrire.".into() });
    }
    let nom = nom_de_l_avenant(&*p.engagement.nat()?)?;
    let fichier = socle::fichiers::modele_de(&nom, ent, &ctx.gabarits());
    let m = fabrique_pdf::modele::lire_fichier(&fichier.to_string_lossy()).map_err(|r| Refus { classe: Some("ValueError"), message: r.0 })?;
    if m.modification.is_empty() {
        return Err(Refus { classe: Some("ValueError"), message: format!("{} n'a pas de bloc MODIFICATION:.", nom) });
    }
    let extra = [
        ("avenant.numero", numero.to_string()),
        ("avenant.date_contrat", date_contrat.to_string()),
        ("avenant.effet", effet.to_string()),
        ("avenant.date", date.to_string()),
    ];
    let d = description_avenant(ctx, p, ent, modifs, &extra)?;
    crate::fabrique::ecrire(&d, chemin).map_err(|m| Refus { classe: Some("FabriqueRefus"), message: m.replace('\n', " ").trim().to_string() })?;
    crate::fabrique::garder_description(&d, chemin).map_err(|m| Refus { classe: Some("OSError"), message: m })?;
    Ok(())
}

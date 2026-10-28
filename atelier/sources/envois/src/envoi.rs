//! L'envoi aux salaries, un message par salarie et par mois.
//!
//! Bulletin, certificat d'emploi Conges Spectacles et attestation France Travail dans un seul message. Rien ne
//! part sans un clic : le garde-fou est a l'appelant (le mot de passe du logiciel, la page de relecture de
//! l'interface) ; ce module prepare (`preparer`), assemble (`en_mime`) et envoie un par un (`envoyer`) par le
//! client `courriel::Courriel` qu'on lui passe -- le vrai (IMAP/SMTP) ou la boite simulee de la compagnie fictive.
//!
//! L'OCTET PRES. `en_mime` rend toujours les memes octets, a `Date`, `Message-ID` et frontieres pres. Les regles :
//!
//! * ORDRE DES EN-TETES : `From`, `To`, `Subject`,
//!   `Date`, `Message-ID`, puis, texte seul : `Content-Type`, `Content-Transfer-Encoding`, `MIME-Version` ;
//!   multipart : `MIME-Version`, `Content-Type: multipart/...` (les en-tetes `Content-*` partent dans la premiere
//!   sous-partie, `MIME-Version` reste a la racine). Une sous-partie finit par
//!   `MIME-Version` ; la partie `alternative` imbriquee dans un `mixed` ne porte que son `Content-Type`.
//! * TEXTE : lignes d'octets coupees sur \n, \r\n, \r ; toutes <= 78 octets :
//!   `7bit` si ASCII, `8bit` sinon ; sinon on regarde les 10 premieres lignes : quoted-
//!   printable (78 colonnes) sauf si sa longueur depasse celle du base64 d'un seul
//!   bloc, auquel cas base64 a 76 colonnes (57 octets par ligne). Au plus 10
//!   lignes et quoted-printable : le corps de l'echantillon est le corps. Fin de ligne \n partout.
//! * PIECES : `Content-Type`, `Content-Transfer-Encoding: base64`,
//!   `Content-Disposition: attachment; filename=...` (toujours entre guillemets, `\` et `"` echappes ;
//!   non ASCII : RFC 2231, `filename*=utf-8''` et `%XX` pour tout octet hors des caracteres non reserves).
//! * ASSEMBLAGE : `--b\n` partie1 (`\n--b\n` partie)*
//!   `\n--b--\n` ; frontiere `=` x15, 19 chiffres, `==`, redemandee tant qu'une ligne `--b` ou `--b--` existe dans
//!   le texte (`.0`, `.1`...) ; les en-tetes d'une partie multipart s'ecrivent APRES son corps (la frontiere).
//! * PLIAGE DES EN-TETES (maxlen 78) : en-tete libre (`Subject`) :
//!   decoupe en blancs et mots, mots non ASCII en mots codes RFC 2047 (`q`
//!   sauf si `b` est plus court de 5 caracteres ou plus), recombinaison avec le mot code precedent de la ligne,
//!   un sujet ASCII qui tient sur une ligne seule passe en entier a la ligne ; en-tete a
//!   parametres (`Content-Type`, `Content-Disposition`) : en une ligne si le tout tient en 78, sinon un
//!   parametre par ligne, continuations RFC 2231 `nom*0*=` au-dela ; adresses et
//!   `Message-ID` : une ligne, ou la valeur seule sur la ligne suivante si elle y tient.
//! * `Date` : heure de Paris (`TZ=Europe/Paris` pose par le moteur) ; `Message-ID` :
//!   `<centiemes.pid.alea64@domaine>`.
//!
//! Limites : une adresse qui n'est pas une adresse simple (`nom@domaine`, ASCII) est refusee ; un sujet qui contient un
//! mot code RFC 2047 deja ecrit n'est pas decode ; le type d'une piece vient de `GENRES` puis de la table `MIMETYPES`
//! (sans les fichiers du systeme : un `.docx` y a son type, ici `octet-stream`) ; `Date` d'avant
//! 1996 : la regle de l'heure d'ete est celle de l'Union europeenne (`paris.rs`).

use std::collections::VecDeque;
use std::path::{Path, PathBuf};

use rust_decimal::Decimal;
use serde_json::Value;

use documents::bulletin::{fmt_eur, fmt_heures};
use engagement::calcul::Paie;
use crate::courriel::Courriel;
use socle::date::{en_texte, repr_flottant};
use socle::fichiers::vrai;

/// Un courriel, sans le piece jointe lue : `pieces` sont des chemins.
#[derive(Debug, Clone, Default)]
pub struct Message {
    pub cle: String,
    pub nom: String,
    pub destinataire: String,
    pub sujet: String,
    pub corps: String,
    pub pieces: Vec<PathBuf>,
    /// La meme lettre en HTML, quand elle porte un bouton (le « Je signe » des contrats). Le texte reste la version
    /// de reference : c'est lui que la page de relecture montre, et lui que lit une messagerie sans HTML.
    pub html: String,
}

impl Message {
    /// La taille des pieces qui existent.
    pub fn taille(&self) -> u64 {
        self.pieces.iter().filter_map(|p| std::fs::metadata(p).ok()).map(|m| m.len()).sum()
    }
}

/// « de » devant un nom de mois, elide devant une voyelle : « de mars », « d'avril », « d'août », « d'octobre ».
pub fn de_mois(nom: &str) -> String {
    if nom.starts_with(['a', 'e', 'é', 'i', 'o', 'u']) {
        format!("d'{}", nom)
    } else {
        format!("de {}", nom)
    }
}

/// Le seul texte de ce depot qui s'adresse a un humain : il porte ses accents. Les champs : `{prenom}`, `{mois}`,
/// `{annee}`, `{liste}`, `{notes}`, `{france_travail}`, `{signataire}`, `{raison_sociale}`.
pub const CORPS: &str = "Bonjour {prenom},\n\nTu trouveras ci-joint tes documents pour {mois} {annee} :\n\n{liste}\n\n{notes}{france_travail}\nBien à toi,\n{signataire}\n{raison_sociale}\n";

/// L'ordre des pieces, une fois pour toutes : il gouverne les pieces jointes, la liste du message et les notes.
pub const PIECES: [&str; 4] = ["bulletin", "rectificatif", "certificat_cs", "aer"];

pub const PREFIXES: [(&str, &str); 4] = [
    ("bulletin", "Bulletin"),
    ("rectificatif", "BulletinRectificatif"),
    ("certificat_cs", "CertificatCS"),
    ("aer", "AER"),
];

pub const LIBELLES: [(&str, &str); 4] = [
    ("bulletin", "le bulletin de paie"),
    ("rectificatif", "le bulletin rectificatif d'un mois précédent"),
    ("certificat_cs", "le certificat d'emploi Congés Spectacles"),
    ("aer", "l'attestation employeur France Travail (AER Cinéma Spectacle)"),
];

/// Pour l'objet du message, ou l'on n'a pas la place des libelles longs.
pub const COURTS: [(&str, &str); 4] = [
    ("bulletin", "bulletin"),
    ("rectificatif", "bulletin rectificatif"),
    ("certificat_cs", "certificat Congés Spectacles"),
    ("aer", "attestation France Travail"),
];

pub const NOTES: [(&str, &str); 4] = [
    ("bulletin", "Le bulletin est à conserver sans limitation de durée."),
    (
        "rectificatif",
        "Le bulletin rectificatif remplace, pour le mois qu'il indique, le bulletin remis alors : la différence est \
         versée (ou retenue) avec la paie de ce mois-ci. Garde les deux.",
    ),
    (
        "certificat_cs",
        "Le certificat d'emploi Congés Spectacles te sert à faire valoir tes droits à congé auprès d'Audiens : \
         garde-le, c'est la pièce qu'ils demandent.",
    ),
    (
        "aer",
        "L'attestation employeur est celle qui compte pour France Travail. Elle est éditée par France Travail à \
         partir de notre déclaration ; nous ne faisons que te la transmettre.",
    ),
];

/// Quand l'AER DUE ce mois-ci (`aer::due` : un intermittent dont le contrat finit dans le mois) n'est pas jointe. On le
/// DIT, plutot que d'envoyer un message qui laisse croire que le dossier est complet ; on ne promet rien de la suite.
/// Un salarie a qui aucune AER Cinema Spectacle n'est due (un permanent, un contrat d'usage qui continue le mois
/// suivant) n'a pas cette phrase.
pub const NOTE_SANS_AER: &str = "Ton attestation employeur France Travail (AER Cinéma Spectacle) de ce mois n'est pas \
    jointe : France Travail l'édite à partir de notre déclaration, et elle n'était pas encore revenue au moment de cet envoi.";

/// Quand le contrat finit ce mois-ci et que sa fin se signale (`aer::attestation_fctu` : CDD, CDI, CDDU rompu avant son
/// terme) : l'attestation employeur de droit commun revient de France Travail apres le signalement, a part de la paie.
/// Le courriel le dit, pour que son absence ne laisse pas croire le dossier complet ; il ne promet rien.
pub const NOTE_SANS_ATTESTATION_FCTU: &str = "Ton contrat se termine ce mois-ci : ton attestation employeur pour France \
    Travail n'est pas jointe à ce courriel. France Travail l'édite à partir du signalement de fin de contrat.";

fn chercher(table: &[(&'static str, &'static str)], cle: &str) -> &'static str {
    table.iter().find(|(c, _)| *c == cle).map(|(_, v)| *v).unwrap_or("")
}

/// `str.capitalize` : la premiere lettre en capitale, le reste en minuscules.
fn capitaliser(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(p) => p.to_uppercase().collect::<String>() + &c.as_str().to_lowercase(),
        None => String::new(),
    }
}

/// `str.format` a champs nommes : chaque `{nom}` est remplace, une seule fois (la valeur n'est pas relue).
pub fn formater(modele: &str, champs: &[(&str, String)]) -> String {
    let mut sortie = String::new();
    let mut reste = modele;
    while let Some(i) = reste.find('{') {
        sortie.push_str(&reste[..i]);
        let suite = &reste[i..];
        let Some(fin) = suite.find('}') else { break };
        let nom = &suite[1..fin];
        match champs.iter().find(|(n, _)| *n == nom) {
            Some((_, v)) => sortie.push_str(v),
            None => sortie.push_str(&suite[..=fin]),
        }
        reste = &suite[fin + 1..];
    }
    sortie.push_str(reste);
    sortie
}

fn cle_absente(cle: &str) -> String {
    socle::fichiers::absent(cle)
}

/// L'annee et le mois d'un « AAAA-MM » (`int(mois[:4]), int(mois[5:7])`).
fn an_mois(mois: &str) -> Result<(i32, usize), String> {
    let lire = |t: Option<&str>| -> Result<i64, String> {
        let t = t.unwrap_or("");
        t.trim().parse::<i64>().map_err(|_| format!("nombre entier illisible : '{}'", t))
    };
    let an = lire(mois.get(..4))?;
    let mo = lire(mois.get(5..7))?;
    Ok((an as i32, mo as usize))
}

/// Ce que le salarie declare a son actualisation France Travail du mois (`recap_france_travail`) : une ligne par
/// contrat, sur ce qui a ete fait DANS LE MOIS. Rien pour un permanent ni pour un salarie qui n'a qu'un
/// rectificatif ce mois-ci : chaine vide.
pub fn recap_france_travail(paies: &[&Paie], mois: &str, employeur: &str) -> Result<String, String> {
    let mut lignes: Vec<String> = Vec::new();
    for p in paies {
        let e = &p.engagement;
        if e.sans_paie() || e.nat()?.mensuelle() {
            continue;
        }
        let mut dates = e.dates_travail.clone();
        dates.sort();
        if dates.is_empty() {
            dates.push(e.debut);
        }
        let mut volume: Vec<String> = Vec::new();
        if e.cachets != 0 {
            volume.push(format!("{} cachet{}", e.cachets, if e.cachets > 1 { "s" } else { "" }));
        }
        if let Some(h) = e.heures.filter(|h| *h != 0.0) {
            let dec = heures_en_decimal(h)?;
            volume.push(format!("{} heure{}", fmt_heures(dec), if h > 1.0 { "s" } else { "" }));
        }
        let (premier, dernier) = (dates[0], dates[dates.len() - 1]);
        let periode = if premier == dernier {
            format!("le {}", premier.jj_mm_aaaa())
        } else {
            format!("du {} au {}", premier.jj_mm_aaaa(), dernier.jj_mm_aaaa())
        };
        let quoi = if volume.is_empty() {
            format!("{} jour{}", dates.len(), if dates.len() > 1 { "s" } else { "" })
        } else {
            volume.join(" et ")
        };
        let mut ligne = format!("  - {} : {}, {} € brut", periode, quoi, fmt_eur(p.brut));
        if e.a_cheval()? && !e.dernier_mois()? {
            // Septembre 2026 : France Travail n'a edite AUCUNE attestation pour le contrat du 29/09 au 16/10 -- elle
            // vient a la fin du contrat. Le salarie declare quand meme ses jours du mois.
            let fin = e.fin.ok_or("un contrat sans fin n'a pas de date de fin")?;
            ligne.push_str(&format!(" (contrat en cours jusqu'au {} : son attestation viendra à la fin)", fin.jj_mm_aaaa()));
        }
        lignes.push(ligne);
    }
    if lignes.is_empty() {
        return Ok(String::new());
    }
    let (an, mo) = an_mois(mois)?;
    let nom_mois = socle::date::nom_du_mois(mo as i64, socle::date::Graphie::Accentuee)?;
    Ok(format!(
        "Pour ton actualisation France Travail {} {}, employeur {} :\n{}\nCe sont les chiffres de ton bulletin ; \
         vérifie que ce que France Travail pré-remplit est identique.\n",
        de_mois(nom_mois),
        an,
        employeur,
        lignes.join("\n")
    ))
}

/// Un nombre d'heures en decimal : le flottant tel qu'il s'ecrit (`repr_flottant`).
fn heures_en_decimal(h: f64) -> Result<Decimal, String> {
    let t = repr_flottant(h);
    t.parse::<Decimal>().or_else(|_| Decimal::from_scientific(&t)).map_err(|e| format!("heures illisibles ({}) : {}", t, e))
}


pub fn nom_fichier(prefixe: &str, cle: &str, etiq: &str) -> String {
    format!("{}_{}_{}.pdf", prefixe, cle, etiq)
}

/// « a, b et c ». Suit les pieces REELLEMENT jointes : promettre dans l'objet une AER qui n'est pas la serait un
/// mensonge en premiere ligne.
fn enumerer(mots: &[&str]) -> String {
    match mots.len() {
        0 => "documents".to_string(),
        1 => mots[0].to_string(),
        n => format!("{} et {}", mots[..n - 1].join(", "), mots[n - 1]),
    }
}

// ------------------------------------------------------------------------------------------- le renvoi

/// LE RENVOI DES DOCUMENTS CORRIGES (des documents d'une paie changee ne sont plus valables, et le courriel doit
/// dire ce qui a change). Le salarie dont la paie a change depuis l'envoi de ses
/// documents recoit ses documents refaits avec ce message-ci, qui dit l'ancien et le nouveau, et que les documents
/// precedents sont remplaces. Les champs : ceux de `CORPS`, plus `{de_mois}`, `{envoye_le}`, `{changements}` et
/// `{declaration}` : vide, ou -- la DSN du mois deposee et son annule-et-remplace passe -- la phrase qui dit
/// avec quelle paie la correction sera declaree (`Renvoi::declaration`).
pub const CORPS_RENVOI: &str = "Bonjour {prenom},\n\nTa paie {de_mois} {annee} a été modifiée depuis l'envoi de tes documents le {envoye_le}. Ce qui a changé :\n\n{changements}\n\nTu trouveras ci-joint tes documents refaits :\n\n{liste}\n\nIls remplacent ceux que tu as reçus le {envoye_le} : ne garde que ceux-ci.\n\n{declaration}{notes}{france_travail}\nBien à toi,\n{signataire}\n{raison_sociale}\n";

/// Ce qu'un renvoi sait de l'envoi precedent : son jour (« 21/10/2026 ») et ce qui a change depuis
/// (`ce_qui_a_change`, une ligne par fait).
#[derive(Debug, Clone, Default)]
pub struct Renvoi {
    pub envoye_le: String,
    pub changements: Vec<String>,
    /// La phrase de la declaration de la correction, son paragraphe compris, ou rien (`moteur::routes::documents::choisir`).
    pub declaration: String,
}

/// Le resume de la paie d'UN salarie pour le mois, tel que ses documents la disent : par contrat, ses bornes, son
/// spectacle, ses jours travailles, ses cachets, ses heures, son brut et son net a payer ; et les totaux. Le journal le
/// garde a chaque envoi des documents (`moteur::routes::documents::expedier_envoi`) : c'est l'« ancien » que le courriel de renvoi
/// compare a la paie refaite. Les montants et les heures s'ecrivent comme le bulletin les ecrit.
pub fn resume_de(paies: &[&Paie]) -> Result<Value, String> {
    let mut contrats: Vec<Value> = Vec::new();
    let (mut brut, mut net) = (Decimal::ZERO, Decimal::ZERO);
    for p in paies {
        let e = &p.engagement;
        let mut dates = e.dates_travail.clone();
        dates.sort();
        let heures = match e.heures.filter(|h| *h != 0.0) {
            Some(h) => fmt_heures(heures_en_decimal(h)?),
            None => String::new(),
        };
        let spectacle = if vrai(&p.spectacle) { p.spectacle.get("titre").filter(|t| vrai(t)).map(en_texte) } else { None };
        contrats.push(serde_json::json!({
            "debut": e.debut.iso(),
            "fin": e.fin.map(|f| f.iso()),
            "spectacle": spectacle,
            "jours": dates.iter().map(|d| d.iso()).collect::<Vec<_>>(),
            "cachets": e.cachets,
            "heures": heures,
            "brut": fmt_eur(p.brut),
            "net": fmt_eur(p.net_a_payer),
        }));
        brut += p.brut;
        net += p.net_a_payer;
    }
    Ok(serde_json::json!({"contrats": contrats, "brut": fmt_eur(brut), "net": fmt_eur(net)}))
}

fn texte_de(v: &Value, cle: &str) -> String {
    v.get(cle).filter(|x| vrai(x)).map(en_texte).unwrap_or_default()
}

/// CE QUI A CHANGE depuis l'envoi des documents, en lignes pretes pour le courriel : ce qui a change dans les
/// engagements du salarie depuis la version sur laquelle ils sont partis (`faits`, dits par l'engagement :
/// « Cachets : 2 → 3 », « Jours : + 22/11 »), puis ce que sa paie en tire, du resume note a l'envoi (`ancien`) a celui
/// de la paie refaite (`nouveau`) : « Salaire brut : 576,00 € → 720,00 € ». Vide quand rien de cela n'a bouge (le
/// detail du bulletin seul a change).
pub fn ce_qui_a_change(faits: &[String], ancien: &Value, nouveau: &Value) -> Vec<String> {
    let mut out: Vec<String> = faits.iter().map(|f| format!("  - {}", f)).collect();
    if texte_de(ancien, "brut") != texte_de(nouveau, "brut") {
        out.push(format!("  - Salaire brut : {} € → {} €", texte_de(ancien, "brut"), texte_de(nouveau, "brut")));
    }
    if texte_de(ancien, "net") != texte_de(nouveau, "net") {
        out.push(format!("  - Net à payer : {} € → {} €", texte_de(ancien, "net"), texte_de(nouveau, "net")));
    }
    out
}

/// Ce que le courriel de renvoi dit quand le resume n'a pas bouge.
pub const RIEN_DANS_LE_RESUME: &str = "  - le détail de ton bulletin ; tes jours, tes cachets, tes heures, ton brut et ton net à payer restent les mêmes";

/// Un `Message` par salarie, a partir des pieces individuelles deja ecrites (`preparer`). Un salarie peut avoir
/// plusieurs contrats dans le mois : ses pieces sont ecrites une fois, sous sa cle, et il ne recoit qu'un seul
/// message. `fiche` (cle -> fiche salarie) : pour celui qui n'a AUCUNE paie ce mois-ci mais un bulletin
/// rectificatif d'un mois precedent.
pub fn preparer(
    paies: &[Paie],
    ent: &Value,
    mois: &str,
    dossier: &Path,
    fiche: Option<&dyn Fn(&str) -> Value>,
) -> Result<Vec<Message>, String> {
    preparer_avec(paies, ent, mois, dossier, fiche, &std::collections::HashMap::new())
}

/// La meme ; `renvois` (cle -> `Renvoi`) : ceux dont la paie a change depuis l'envoi de leurs documents recoivent le
/// courriel de renvoi (`CORPS_RENVOI`), qui dit ce qui a change ; les autres, le premier courriel (`CORPS`).
pub fn preparer_avec(
    paies: &[Paie],
    ent: &Value,
    mois: &str,
    dossier: &Path,
    fiche: Option<&dyn Fn(&str) -> Value>,
    renvois: &std::collections::HashMap<String, Renvoi>,
) -> Result<Vec<Message>, String> {
    let (an, mo) = an_mois(mois)?;
    let etiq = socle::date::etiquette_du_mois(mois)?;
    let nom_mois = socle::date::nom_du_mois(mo as i64, socle::date::Graphie::Accentuee)?;
    let mut vus: Vec<String> = Vec::new();
    let mut messages = Vec::new();
    let mut salaries: Vec<Value> = paies.iter().map(|p| p.salarie.clone()).collect();
    let cle_de = |s: &Value| -> Result<String, String> { Ok(en_texte(s.get("_cle").ok_or_else(|| cle_absente("_cle"))?)) };
    if let Some(fiche) = fiche {
        let prefixe = format!("{}_", chercher(&PREFIXES, "rectificatif"));
        let suffixe = format!("_{}.pdf", etiq);
        let mut noms: Vec<String> = match std::fs::read_dir(dossier) {
            Ok(l) => l.filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().into_owned()).collect(),
            Err(_) => Vec::new(),
        };
        noms.retain(|n| n.len() >= prefixe.len() + suffixe.len() && n.starts_with(&prefixe) && n.ends_with(&suffixe));
        noms.sort();
        for nom in noms {
            let cle = nom[prefixe.len()..nom.len() - suffixe.len()].to_string();
            let connues: Result<Vec<String>, String> = salaries.iter().map(&cle_de).collect();
            if !connues?.contains(&cle) {
                let mut f = fiche(&cle);
                if let Some(o) = f.as_object_mut() {
                    o.insert("_cle".into(), Value::String(cle));
                }
                salaries.push(f);
            }
        }
    }
    for s in &salaries {
        let cle = cle_de(s)?;
        if vus.contains(&cle) {
            continue;
        }
        vus.push(cle.clone());
        let texte = |k: &str| -> Result<String, String> { Ok(en_texte(s.get(k).ok_or_else(|| cle_absente(k))?)) };
        let email = s.get("email").filter(|v| vrai(v));
        let Some(email) = email else {
            return Err(format!(
                "Pas d'adresse e-mail pour {} {} : completer config/salaries.json avant d'envoyer.",
                texte("nom")?,
                texte("prenom")?
            ));
        };
        let mut pieces = Vec::new();
        let mut libelles = Vec::new();
        let mut presents: Vec<&str> = Vec::new();
        for champ in PIECES {
            let chemin = dossier.join(nom_fichier(chercher(&PREFIXES, champ), &cle, &etiq));
            if chemin.exists() {
                pieces.push(chemin);
                libelles.push(format!("  - {}", chercher(&LIBELLES, champ)));
                presents.push(champ);
            }
        }
        let mut notes: Vec<&str> = presents.iter().map(|c| chercher(&NOTES, c)).collect();
        let siennes: Vec<&Paie> = {
            let mut v = Vec::new();
            for p in paies {
                if cle_de(&p.salarie)? == cle {
                    v.push(p);
                }
            }
            v
        };
        let (mut aer_due, mut fctu) = (false, false);
        for p in &siennes {
            aer_due |= documents::aer::due(&p.engagement)?;
            fctu |= documents::aer::attestation_fctu(&p.engagement)?;
        }
        if aer_due && !presents.contains(&"aer") {
            notes.push(NOTE_SANS_AER);
        }
        if fctu {
            notes.push(NOTE_SANS_ATTESTATION_FCTU);
        }
        let nom_affichage = en_texte(ent.get("nom_affichage").ok_or_else(|| cle_absente("nom_affichage"))?);
        let mut ft = recap_france_travail(&siennes, mois, &nom_affichage)?;
        if !ft.is_empty() && !notes.is_empty() {
            ft = format!("\n{}", ft);
        }
        let signataire = ent.get("signataire").ok_or_else(|| cle_absente("signataire"))?;
        let nom_signataire = match signataire.get("prenom_affichage").filter(|v| vrai(v)) {
            Some(v) => en_texte(v),
            None => en_texte(signataire.get("prenom").ok_or_else(|| cle_absente("prenom"))?),
        };
        let courts: Vec<&str> = presents.iter().map(|c| chercher(&COURTS, c)).collect();
        let renvoi = renvois.get(&cle);
        let champs = [
            ("prenom", texte("prenom_affichage")?),
            ("mois", nom_mois.to_string()),
            ("de_mois", de_mois(nom_mois)),
            ("annee", an.to_string()),
            ("liste", libelles.join("\n")),
            ("notes", if notes.is_empty() { String::new() } else { format!("{}\n", notes.join("\n\n")) }),
            ("france_travail", ft),
            ("signataire", nom_signataire),
            ("raison_sociale", nom_affichage),
            ("envoye_le", renvoi.map(|r| r.envoye_le.clone()).unwrap_or_default()),
            ("declaration", renvoi.map(|r| r.declaration.clone()).unwrap_or_default()),
            (
                "changements",
                match renvoi {
                    Some(r) if !r.changements.is_empty() => r.changements.join("\n"),
                    _ => RIEN_DANS_LE_RESUME.to_string(),
                },
            ),
        ];
        let (sujet, corps) = match renvoi {
            Some(_) => (format!("{} {} — documents corrigés : {}", capitaliser(nom_mois), an, enumerer(&courts)), formater(CORPS_RENVOI, &champs)),
            None => (format!("{} {} — {}", capitaliser(nom_mois), an, enumerer(&courts)), formater(CORPS, &champs)),
        };
        messages.push(Message {
            cle: cle.clone(),
            nom: format!("{} {}", texte("prenom_affichage")?, texte("nom")?),
            destinataire: en_texte(email).to_lowercase(),
            sujet,
            corps,
            pieces,
            html: String::new(),
        });
    }
    Ok(messages)
}

// ----------------------------------------------------------------------------------------------------- l'envoi

/// Envoie pour de bon. Irreversible : un courriel parti ne revient pas. Un par un ; la liste rendue ne porte que
/// ceux qui sont reellement partis (leurs destinataires) : si le troisieme echoue, les deux premiers sont partis, et
/// l'erreur le dit.
///
/// `identifiants` : une liste a remplir, si l'appelant la passe, des identifiants des messages partis, dans
/// l'ordre (le `Message-ID` avec le vrai client). `copies` : de meme, les messages exactement tels qu'ils sont
/// partis (RFC 822, en octets), a archiver quand l'envoi fait preuve.
pub fn envoyer(
    messages: &[Message],
    expediteur: &str,
    client: &mut dyn Courriel,
    identifiants: Option<&mut Vec<String>>,
    copies: Option<&mut Vec<Vec<u8>>>,
) -> Result<Vec<String>, String> {
    envoyer_suivi(messages, expediteur, client, identifiants, copies).map_err(|e| e.message)
}

/// Un envoi qui a echoue en route : la phrase de l'erreur, les adresses ou les messages sont REELLEMENT partis, et
/// l'adresse du message qui n'est pas parti (retours Windows, C : le journal les note, jamais l'expediteur).
#[derive(Debug, Clone)]
pub struct ErreurEnvoi {
    pub message: String,
    pub partis: Vec<String>,
    pub echoue: String,
}

/// `envoyer`, avec ce qu'il faut pour noter un echec au journal : qui a ete servi, et ou l'envoi a echoue.
pub fn envoyer_suivi(
    messages: &[Message],
    expediteur: &str,
    client: &mut dyn Courriel,
    mut identifiants: Option<&mut Vec<String>>,
    mut copies: Option<&mut Vec<Vec<u8>>>,
) -> Result<Vec<String>, ErreurEnvoi> {
    let mut faits: Vec<String> = Vec::new();
    // LA SIGNATURE DE LA COMPAGNIE : tout courriel passe ici, il la prend ici. Trop lourde, rien ne part.
    let signature = match client.racine().map(crate::signature_courriel::du_poste) {
        Some(Ok(s)) => s,
        Some(Err(m)) => {
            return Err(ErreurEnvoi { message: m, partis: Vec::new(), echoue: messages.first().map(|m| m.destinataire.clone()).unwrap_or_default() })
        }
        None => None,
    };
    for message in messages {
        let signe;
        let (message, nom) = match &signature {
            Some(s) => {
                signe = crate::signature_courriel::signer(message, s);
                (&signe, s.nom.as_str())
            }
            None => (message, ""),
        };
        let brut = en_mime_nomme(message, expediteur, nom).map_err(|m| ErreurEnvoi { message: m, partis: faits.clone(), echoue: message.destinataire.clone() })?;
        let ident = match client.envoyer(&brut) {
            Ok(i) => i,
            Err(e) => {
                let phrase = if faits.is_empty() {
                    e.to_string()
                } else {
                    format!(
                        "{} : {}\nATTENTION, {} message(s) etaient deja partis : {}",
                        message.destinataire,
                        e,
                        faits.len(),
                        faits.join(", ")
                    )
                };
                return Err(ErreurEnvoi { message: phrase, partis: faits, echoue: message.destinataire.clone() });
            }
        };
        faits.push(message.destinataire.clone());
        if let Some(l) = identifiants.as_deref_mut() {
            l.push(ident);
        }
        if let Some(l) = copies.as_deref_mut() {
            l.push(brut);
        }
    }
    Ok(faits)
}

// ------------------------------------------------------------------------------------------- le message MIME

const MAXLEN: usize = 78;

/// Les types qui passent devant `mimetypes` (`envoi.GENRES`).
const GENRES: [(&str, &str); 2] = [
    (".pdf", "application/pdf"),
    (".xlsx", "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"),
];

/// La table des types de pieces, sans les fichiers du systeme (`/etc/mime.types`, le registre Windows) : 154 suffixes,
/// les memes sur tous les postes.
const MIMETYPES: [(&str, &str); 154] = [
    (".3g2", "audio/3gpp2"),
    (".3gp", "audio/3gpp"),
    (".3gpp", "audio/3gpp"),
    (".3gpp2", "audio/3gpp2"),
    (".a", "application/octet-stream"),
    (".aac", "audio/aac"),
    (".adts", "audio/aac"),
    (".ai", "application/postscript"),
    (".aif", "audio/x-aiff"),
    (".aifc", "audio/x-aiff"),
    (".aiff", "audio/x-aiff"),
    (".ass", "audio/aac"),
    (".au", "audio/basic"),
    (".avi", "video/x-msvideo"),
    (".avif", "image/avif"),
    (".bat", "text/plain"),
    (".bcpio", "application/x-bcpio"),
    (".bin", "application/octet-stream"),
    (".bmp", "image/bmp"),
    (".c", "text/plain"),
    (".cdf", "application/x-netcdf"),
    (".cpio", "application/x-cpio"),
    (".csh", "application/x-csh"),
    (".css", "text/css"),
    (".csv", "text/csv"),
    (".dll", "application/octet-stream"),
    (".doc", "application/msword"),
    (".dot", "application/msword"),
    (".dvi", "application/x-dvi"),
    (".eml", "message/rfc822"),
    (".eps", "application/postscript"),
    (".etx", "text/x-setext"),
    (".exe", "application/octet-stream"),
    (".gif", "image/gif"),
    (".gtar", "application/x-gtar"),
    (".h", "text/plain"),
    (".h5", "application/x-hdf5"),
    (".hdf", "application/x-hdf"),
    (".heic", "image/heic"),
    (".heif", "image/heif"),
    (".htm", "text/html"),
    (".html", "text/html"),
    (".ico", "image/vnd.microsoft.icon"),
    (".ief", "image/ief"),
    (".jpe", "image/jpeg"),
    (".jpeg", "image/jpeg"),
    (".jpg", "image/jpeg"),
    (".js", "text/javascript"),
    (".json", "application/json"),
    (".ksh", "text/plain"),
    (".latex", "application/x-latex"),
    (".loas", "audio/aac"),
    (".m1v", "video/mpeg"),
    (".m3u", "application/vnd.apple.mpegurl"),
    (".m3u8", "application/vnd.apple.mpegurl"),
    (".man", "application/x-troff-man"),
    (".markdown", "text/markdown"),
    (".md", "text/markdown"),
    (".me", "application/x-troff-me"),
    (".mht", "message/rfc822"),
    (".mhtml", "message/rfc822"),
    (".mif", "application/x-mif"),
    (".mjs", "text/javascript"),
    (".mov", "video/quicktime"),
    (".movie", "video/x-sgi-movie"),
    (".mp2", "audio/mpeg"),
    (".mp3", "audio/mpeg"),
    (".mp4", "video/mp4"),
    (".mpa", "video/mpeg"),
    (".mpe", "video/mpeg"),
    (".mpeg", "video/mpeg"),
    (".mpg", "video/mpeg"),
    (".ms", "application/x-troff-ms"),
    (".n3", "text/n3"),
    (".nc", "application/x-netcdf"),
    (".nq", "application/n-quads"),
    (".nt", "application/n-triples"),
    (".nws", "message/rfc822"),
    (".o", "application/octet-stream"),
    (".obj", "application/octet-stream"),
    (".oda", "application/oda"),
    (".opus", "audio/opus"),
    (".p12", "application/x-pkcs12"),
    (".p7c", "application/pkcs7-mime"),
    (".pbm", "image/x-portable-bitmap"),
    (".pdf", "application/pdf"),
    (".pfx", "application/x-pkcs12"),
    (".pgm", "image/x-portable-graymap"),
    (".pl", "text/plain"),
    (".png", "image/png"),
    (".pnm", "image/x-portable-anymap"),
    (".pot", "application/vnd.ms-powerpoint"),
    (".ppa", "application/vnd.ms-powerpoint"),
    (".ppm", "image/x-portable-pixmap"),
    (".pps", "application/vnd.ms-powerpoint"),
    (".ppt", "application/vnd.ms-powerpoint"),
    (".ps", "application/postscript"),
    (".pwz", "application/vnd.ms-powerpoint"),
    (".py", "text/x-python"),
    (".pyc", "application/x-python-code"),
    (".pyo", "application/x-python-code"),
    (".qt", "video/quicktime"),
    (".ra", "audio/x-pn-realaudio"),
    (".ram", "application/x-pn-realaudio"),
    (".ras", "image/x-cmu-raster"),
    (".rdf", "application/xml"),
    (".rgb", "image/x-rgb"),
    (".roff", "application/x-troff"),
    (".rst", "text/x-rst"),
    (".rtf", "text/rtf"),
    (".rtx", "text/richtext"),
    (".sgm", "text/x-sgml"),
    (".sgml", "text/x-sgml"),
    (".sh", "application/x-sh"),
    (".shar", "application/x-shar"),
    (".snd", "audio/basic"),
    (".so", "application/octet-stream"),
    (".src", "application/x-wais-source"),
    (".srt", "text/plain"),
    (".sv4cpio", "application/x-sv4cpio"),
    (".sv4crc", "application/x-sv4crc"),
    (".svg", "image/svg+xml"),
    (".swf", "application/x-shockwave-flash"),
    (".t", "application/x-troff"),
    (".tar", "application/x-tar"),
    (".tcl", "application/x-tcl"),
    (".tex", "application/x-tex"),
    (".texi", "application/x-texinfo"),
    (".texinfo", "application/x-texinfo"),
    (".tif", "image/tiff"),
    (".tiff", "image/tiff"),
    (".tr", "application/x-troff"),
    (".trig", "application/trig"),
    (".tsv", "text/tab-separated-values"),
    (".txt", "text/plain"),
    (".ustar", "application/x-ustar"),
    (".vcf", "text/x-vcard"),
    (".vtt", "text/vtt"),
    (".wasm", "application/wasm"),
    (".wav", "audio/x-wav"),
    (".webm", "video/webm"),
    (".webmanifest", "application/manifest+json"),
    (".webp", "image/webp"),
    (".wiz", "application/msword"),
    (".wsdl", "application/xml"),
    (".xbm", "image/x-xbitmap"),
    (".xlb", "application/vnd.ms-excel"),
    (".xls", "application/vnd.ms-excel"),
    (".xml", "text/xml"),
    (".xpdl", "application/xml"),
    (".xpm", "image/x-xpixmap"),
    (".xsl", "application/xml"),
    (".xwd", "image/x-xwindowdump"),
    (".zip", "application/zip"),
];

/// `mimetypes.suffix_map` : un suffixe qui en cache deux.
const SUFFIXES: [(&str, &str); 6] = [
    (".svgz", ".svg.gz"),
    (".tgz", ".tar.gz"),
    (".taz", ".tar.gz"),
    (".tz", ".tar.gz"),
    (".tbz2", ".tar.bz2"),
    (".txz", ".tar.xz"),
];

/// `mimetypes.encodings_map` (sensible a la casse) : un suffixe de compression, que la devinette saute.
const ENCODAGES: [&str; 5] = [".gz", ".Z", ".bz2", ".xz", ".br"];

/// `os.path.splitext` : le dernier point du nom, sauf s'il ne suit que des points.
fn couper_extension(nom: &str) -> (&str, &str) {
    let Some(point) = nom.rfind('.') else { return (nom, "") };
    if nom[..point].chars().all(|c| c == '.') {
        return (nom, "");
    }
    (&nom[..point], &nom[point..])
}

/// `mimetypes.guess_type(nom)[0]`, sans les fichiers du systeme.
pub fn type_devine(nom: &str) -> Option<&'static str> {
    let (mut base, mut ext) = couper_extension(nom);
    let mut dernier: String;
    loop {
        let minuscule = ext.to_lowercase();
        let Some((_, remplace)) = SUFFIXES.iter().find(|(s, _)| *s == minuscule) else { break };
        dernier = format!("{}{}", base, remplace);
        let (b, e) = couper_extension(&dernier);
        base = &dernier[..b.len()];
        ext = &dernier[dernier.len() - e.len()..];
    }
    if ENCODAGES.contains(&ext) {
        let (b, e) = couper_extension(base);
        let _ = b;
        ext = e;
    }
    let ext = ext.to_lowercase();
    MIMETYPES.iter().find(|(s, _)| *s == ext).map(|(_, g)| *g)
}

fn genre_de(nom: &str) -> &'static str {
    // `piece.suffix.lower()` : le suffixe de pathlib (rien pour un nom qui n'est qu'un suffixe, ni pour un point final).
    let suffixe = match nom.rfind('.') {
        Some(i) if i > 0 && i + 1 < nom.len() => nom[i..].to_lowercase(),
        _ => String::new(),
    };
    GENRES.iter().find(|(s, _)| *s == suffixe).map(|(_, g)| *g).or_else(|| type_devine(nom)).unwrap_or("application/octet-stream")
}

/// Ce que `en_mime` demande au hasard et a l'horloge : de quoi fixer `Date`, `Message-ID` et les frontieres dans
/// les tests.
pub struct Hasards<'a> {
    pub date: String,
    pub message_id: String,
    /// Une frontiere neuve a chaque appel (`=` x15, 19 chiffres, `==`).
    pub frontiere: &'a mut dyn FnMut() -> String,
}

/// Le message complet, pieces jointes comprises, tel que `EmailMessage.as_bytes()` l'ecrit.
pub fn en_mime(message: &Message, expediteur: &str) -> Result<Vec<u8>, String> {
    en_mime_nomme(message, expediteur, "")
}

/// `en_mime`, l'expediteur precede du nom que la compagnie affiche (« Compagnie du Fil <adresse> ») ; vide, l'adresse
/// seule.
pub fn en_mime_nomme(message: &Message, expediteur: &str, nom: &str) -> Result<Vec<u8>, String> {
    let mut frontiere = frontiere_au_hasard;
    let domaine = expediteur.rsplit('@').next().unwrap_or("");
    let hasards = Hasards {
        date: date_locale(),
        message_id: nouveau_message_id(if domaine.is_empty() { "localhost.localdomain" } else { domaine }),
        frontiere: &mut frontiere,
    };
    en_mime_complet(message, expediteur, nom, hasards)
}

pub fn en_mime_avec(message: &Message, expediteur: &str, hasards: Hasards) -> Result<Vec<u8>, String> {
    en_mime_complet(message, expediteur, "", hasards)
}

fn en_mime_complet(message: &Message, expediteur: &str, nom: &str, hasards: Hasards) -> Result<Vec<u8>, String> {
    let Hasards { date, message_id, frontiere } = hasards;
    let adresse = |a: &str| -> Result<(), String> {
        if adresse_simple(a) {
            Ok(())
        } else {
            Err(format!("Adresse illisible : {:?}", a))
        }
    };
    adresse(expediteur)?;
    adresse(&message.destinataire)?;
    if compte_lignes(&message.sujet) > 1 {
        return Err("Header values may not contain linefeed or carriage return characters".to_string());
    }
    let mut racine_entetes = vec![
        if nom.is_empty() { Entete::Adresse("From", expediteur.to_string()) } else { Entete::De(nom.to_string(), expediteur.to_string()) },
        Entete::Adresse("To", message.destinataire.clone()),
        Entete::Libre("Subject", message.sujet.clone()),
        Entete::Date(date),
        Entete::MessageId(message_id),
    ];
    let mime_version = || Entete::Simple("MIME-Version", "1.0".to_string());
    let (cte, charge) = encoder_texte(&message.corps);
    let partie_texte = |sous: &str, cte: &str, charge: Vec<u8>, version: bool| -> Noeud {
        let mut e = vec![
            Entete::Params {
                nom: "Content-Type",
                principal: format!("text/{}", sous),
                params: vec![("charset".to_string(), "utf-8".to_string())],
            },
            Entete::Simple("Content-Transfer-Encoding", cte.to_string()),
        ];
        if version {
            e.push(mime_version());
        }
        Noeud { entetes: e, corps: Corps::Texte(charge) }
    };
    let texte = partie_texte("plain", cte, charge, false);
    let mut pieces: Vec<Noeud> = Vec::new();
    for piece in &message.pieces {
        let octets = std::fs::read(piece).map_err(|e| format!("{} : {}", piece.display(), e))?;
        let nom = piece.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let genre = genre_de(&nom);
        pieces.push(Noeud {
            entetes: vec![
                Entete::Params { nom: "Content-Type", principal: genre.to_string(), params: vec![] },
                Entete::Simple("Content-Transfer-Encoding", "base64".to_string()),
                Entete::Params {
                    nom: "Content-Disposition",
                    principal: "attachment".to_string(),
                    params: vec![("filename".to_string(), nom)],
                },
                mime_version(),
            ],
            corps: Corps::Texte(base64_lignes(&octets, MAXLEN).into_bytes()),
        });
    }
    let racine = if message.html.is_empty() && pieces.is_empty() {
        // Texte seul : les en-tetes de contenu suivent ceux du message.
        let Noeud { entetes, corps } = texte;
        racine_entetes.extend(entetes);
        racine_entetes.push(mime_version());
        Noeud { entetes: racine_entetes, corps }
    } else {
        let alternative = if message.html.is_empty() {
            None
        } else {
            let (cte_h, charge_h) = encoder_texte(&message.html);
            Some(vec![texte.clone(), partie_texte("html", cte_h, charge_h, true)])
        };
        racine_entetes.push(mime_version());
        if pieces.is_empty() {
            racine_entetes.push(multipart("alternative"));
            Noeud { entetes: racine_entetes, corps: Corps::Parties(alternative.unwrap_or_default()) }
        } else {
            racine_entetes.push(multipart("mixed"));
            let mut parties = Vec::new();
            match alternative {
                Some(a) => parties.push(Noeud { entetes: vec![multipart("alternative")], corps: Corps::Parties(a) }),
                None => parties.push(texte),
            }
            parties.extend(pieces);
            Noeud { entetes: racine_entetes, corps: Corps::Parties(parties) }
        }
    };
    ecrire_noeud(&racine, frontiere)
}

fn multipart(sous: &str) -> Entete {
    Entete::Params { nom: "Content-Type", principal: format!("multipart/{}", sous), params: vec![] }
}

#[derive(Clone)]
enum Entete {
    /// `From`, `To` : une adresse simple.
    Adresse(&'static str, String),
    /// `From` avec le nom affiche : (nom, adresse).
    De(String, String),
    /// `Subject` : texte libre.
    Libre(&'static str, String),
    Date(String),
    MessageId(String),
    /// `Content-Transfer-Encoding`, `MIME-Version` : un mot.
    Simple(&'static str, String),
    /// `Content-Type`, `Content-Disposition` : une valeur et ses parametres.
    Params { nom: &'static str, principal: String, params: Vec<(String, String)> },
}

#[derive(Clone)]
enum Corps {
    Texte(Vec<u8>),
    Parties(Vec<Noeud>),
}

#[derive(Clone)]
struct Noeud {
    entetes: Vec<Entete>,
    corps: Corps,
}

/// `BytesGenerator._write` : le corps d'abord (la frontiere en depend), puis les en-tetes, une ligne vide, le corps.
fn ecrire_noeud(n: &Noeud, frontiere: &mut dyn FnMut() -> String) -> Result<Vec<u8>, String> {
    let mut entetes = n.entetes.clone();
    let corps: Vec<u8> = match &n.corps {
        Corps::Texte(t) => t.clone(),
        Corps::Parties(parties) => {
            let textes: Result<Vec<Vec<u8>>, String> = parties.iter().map(|p| ecrire_noeud(p, frontiere)).collect();
            let textes = textes?;
            let tout = textes.join(&b'\n');
            let b = frontiere_libre(frontiere(), &tout);
            let mut c: Vec<u8> = Vec::new();
            c.extend_from_slice(format!("--{}\n", b).as_bytes());
            for (i, t) in textes.iter().enumerate() {
                if i > 0 {
                    c.extend_from_slice(format!("\n--{}\n", b).as_bytes());
                }
                c.extend_from_slice(t);
            }
            c.extend_from_slice(format!("\n--{}--\n", b).as_bytes());
            for e in entetes.iter_mut() {
                if let Entete::Params { nom: "Content-Type", params, .. } = e {
                    params.push(("boundary".to_string(), b.clone()));
                }
            }
            c
        }
    };
    let mut sortie: Vec<u8> = Vec::new();
    for e in &entetes {
        sortie.extend_from_slice(plier_entete(e)?.as_bytes());
    }
    sortie.push(b'\n');
    sortie.extend_from_slice(&corps);
    Ok(sortie)
}

/// `_make_boundary(text)` : la frontiere ne doit apparaitre seule sur aucune ligne du texte.
fn frontiere_libre(base: String, texte: &[u8]) -> String {
    let mut b = base.clone();
    let mut compteur = 0;
    loop {
        let fermante = format!("--{}--", b);
        let ouvrante = format!("--{}", b);
        let presente = texte.split(|c| *c == b'\n').any(|l| {
            let l = l.strip_suffix(b"\r").unwrap_or(l);
            l == ouvrante.as_bytes() || l == fermante.as_bytes()
        });
        if !presente {
            return b;
        }
        b = format!("{}.{}", base, compteur);
        compteur += 1;
    }
}

fn frontiere_au_hasard() -> String {
    let mut o = [0u8; 8];
    // Sans alea du systeme, le temps : la frontiere n'a pas besoin d'etre imprevisible, seulement absente du texte.
    if socle::temps::aleas(&mut o).is_err() {
        o = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(1).to_le_bytes();
    }
    let jeton = u64::from_le_bytes(o) >> 1;
    format!("{}{:019}==", "=".repeat(15), jeton)
}

/// `utils.make_msgid(domain=...)`.
pub fn nouveau_message_id(domaine: &str) -> String {
    let maintenant = std::time::Duration::from_secs_f64(socle::temps::secondes());
    let centiemes = maintenant.as_millis() / 10;
    let mut o = [0u8; 8];
    let _ = socle::temps::aleas(&mut o);
    format!("<{}.{}.{}@{}>", centiemes, socle::temps::numero_de_processus(), u64::from_le_bytes(o), domaine)
}

/// `utils.formatdate(localtime=True)`, a l'heure de Paris.
fn date_locale() -> String {
    let t = socle::temps::secondes().floor() as i64;
    date_de(t)
}

/// La date RFC 5322 d'un instant (secondes unix), heure de Paris.
pub fn date_de(t: i64) -> String {
    const JOURS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    const MOIS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    let decalage = socle::paris::decalage(t);
    let local = t + decalage;
    let jours = local.div_euclid(86_400);
    let secondes = local.rem_euclid(86_400);
    let j = socle::date::Jour::depuis_jours_unix(jours).unwrap_or(socle::date::Jour { an: 1970, mois: 1, jour: 1 });
    // Le 01/01/1970 etait un jeudi.
    let semaine = (jours + 3).rem_euclid(7) as usize;
    format!(
        "{}, {:02} {} {:04} {:02}:{:02}:{:02} {}{:02}{:02}",
        JOURS[semaine],
        j.jour,
        MOIS[j.mois as usize - 1],
        j.an,
        secondes / 3600,
        secondes % 3600 / 60,
        secondes % 60,
        if decalage < 0 { '-' } else { '+' },
        decalage.abs() / 3600,
        decalage.abs() % 3600 / 60
    )
}

/// `len(value.splitlines())`, pour le refus d'un en-tete a plusieurs lignes.
fn compte_lignes(s: &str) -> usize {
    let mut n = 0;
    let mut en_cours = false;
    let mut cr = false;
    for c in s.chars() {
        if matches!(c, '\n' | '\r' | '\x0b' | '\x0c' | '\x1c' | '\x1d' | '\x1e' | '\u{85}' | '\u{2028}' | '\u{2029}') {
            if c == '\n' && cr {
                cr = false;
                continue;
            }
            cr = c == '\r';
            n += 1;
            en_cours = false;
        } else {
            cr = false;
            en_cours = true;
        }
    }
    n + usize::from(en_cours)
}

/// Une adresse `local@domaine` en ASCII, sans nom ni chevrons : la seule forme que ce logiciel ecrit.
fn adresse_simple(a: &str) -> bool {
    let Some((local, domaine)) = a.split_once('@') else { return false };
    let ok_local = !local.is_empty()
        && local.bytes().all(|c| c.is_ascii_alphanumeric() || b"!#$%&'*+/=?^_`{|}~-.".contains(&c));
    let ok_domaine = !domaine.is_empty() && domaine.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'.');
    ok_local && ok_domaine
}

// ------------------------------------------------------------------------------------------- le texte des parties

/// `contentmanager._encode_text` : (`Content-Transfer-Encoding`, charge).
fn encoder_texte(texte: &str) -> (&'static str, Vec<u8>) {
    let lignes = lignes_octets(texte.as_bytes());
    let normal = {
        let mut b = lignes.join(&b'\n');
        b.push(b'\n');
        b
    };
    let plus_longue = lignes.iter().map(|l| l.len()).max().unwrap_or(0);
    if plus_longue <= MAXLEN {
        return (if normal.is_ascii() { "7bit" } else { "8bit" }, normal);
    }
    let echantillon = {
        let mut b = lignes[..lignes.len().min(10)].join(&b'\n');
        b.push(b'\n');
        b
    };
    let qp = quoted_printable(&echantillon, MAXLEN);
    let longueur_base64 = echantillon.len().div_ceil(3) * 4 + 1;
    if qp.len() > longueur_base64 {
        let mut incorpore = lignes.join(&b'\n');
        incorpore.push(b'\n');
        ("base64", base64_lignes(&incorpore, MAXLEN).into_bytes())
    } else if lignes.len() <= 10 {
        ("quoted-printable", qp)
    } else {
        ("quoted-printable", quoted_printable(&normal, MAXLEN))
    }
}

/// `bytes.splitlines()` : coupe sur \n, \r\n et \r seulement.
fn lignes_octets(o: &[u8]) -> Vec<&[u8]> {
    let mut lignes = Vec::new();
    let mut debut = 0;
    let mut i = 0;
    while i < o.len() {
        match o[i] {
            b'\n' => {
                lignes.push(&o[debut..i]);
                i += 1;
                debut = i;
            }
            b'\r' => {
                lignes.push(&o[debut..i]);
                i += if o.get(i + 1) == Some(&b'\n') { 2 } else { 1 };
                debut = i;
            }
            _ => i += 1,
        }
    }
    if debut < o.len() {
        lignes.push(&o[debut..]);
    }
    lignes
}

/// `quoprimime.body_encode(body.decode('latin-1'), maxlinelen)` : chaque octet est un caractere, `=` et tout ce
/// qui n'est pas ASCII imprimable (hors tabulation) s'ecrit `=XX`, lignes coupees par `=\n`.
pub fn quoted_printable(corps: &[u8], max: usize) -> Vec<u8> {
    if corps.is_empty() {
        return Vec::new();
    }
    const SURES: &[u8] = b" !\"#$%&'()*+,-./0123456789:;<>?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`abcdefghijklmnopqrstuvwxyz{|}~\t";
    let mut t: Vec<u8> = Vec::with_capacity(corps.len());
    for &c in corps {
        if SURES.contains(&c) || c == b'\r' || c == b'\n' {
            t.push(c);
        } else {
            t.extend_from_slice(format!("={:02X}", c).as_bytes());
        }
    }
    let citer = |c: u8| format!("={:02X}", c).into_bytes();
    let max1 = max as i64 - 1;
    let mut codees: Vec<Vec<u8>> = Vec::new();
    for ligne in lignes_octets(&t) {
        let long = ligne.len() as i64;
        let mut debut: i64 = 0;
        let derniere = long - 1 - max as i64;
        while debut <= derniere {
            let fin = debut + max1;
            if ligne[(fin - 2) as usize] == b'=' {
                codees.push(ligne[debut as usize..(fin - 1) as usize].to_vec());
                debut = fin - 2;
            } else if ligne[(fin - 1) as usize] == b'=' {
                codees.push(ligne[debut as usize..fin as usize].to_vec());
                debut = fin - 1;
            } else {
                let mut l = ligne[debut as usize..fin as usize].to_vec();
                l.push(b'=');
                codees.push(l);
                debut = fin;
            }
        }
        match ligne.last() {
            Some(&dernier) if dernier == b' ' || dernier == b'\t' => {
                let place = debut - derniere;
                let q: Vec<u8> = if place >= 3 {
                    citer(dernier)
                } else if place == 2 {
                    vec![dernier, b'=', b'\n']
                } else {
                    let mut v = b"=\n".to_vec();
                    v.extend(citer(dernier));
                    v
                };
                let milieu = if (debut as usize) < ligne.len() - 1 { &ligne[debut as usize..ligne.len() - 1] } else { &[][..] };
                let mut l = milieu.to_vec();
                l.extend(q);
                codees.push(l);
            }
            _ => codees.push(ligne[(debut as usize).min(ligne.len())..].to_vec()),
        }
    }
    if matches!(t.last(), Some(b'\r') | Some(b'\n')) {
        codees.push(Vec::new());
    }
    codees.join(&b'\n')
}

const BASE64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn base64_brut(o: &[u8]) -> String {
    let mut s = String::with_capacity(o.len().div_ceil(3) * 4);
    for bloc in o.chunks(3) {
        let n = (bloc[0] as u32) << 16 | (*bloc.get(1).unwrap_or(&0) as u32) << 8 | *bloc.get(2).unwrap_or(&0) as u32;
        s.push(BASE64[(n >> 18) as usize & 63] as char);
        s.push(BASE64[(n >> 12) as usize & 63] as char);
        s.push(if bloc.len() > 1 { BASE64[(n >> 6) as usize & 63] as char } else { '=' });
        s.push(if bloc.len() > 2 { BASE64[n as usize & 63] as char } else { '=' });
    }
    s
}

/// `contentmanager._encode_base64` : `max // 4 * 3` octets par ligne, 76 colonnes, chaque ligne finie par \n.
fn base64_lignes(o: &[u8], max: usize) -> String {
    let par_ligne = max / 4 * 3;
    let mut s = String::new();
    for bloc in o.chunks(par_ligne) {
        s.push_str(&base64_brut(bloc));
        s.push('\n');
    }
    s
}

fn base64_lire(t: &str) -> Option<Vec<u8>> {
    let mut sortie = Vec::new();
    let mut tampon = 0u32;
    let mut bits = 0;
    for c in t.bytes() {
        if c == b'=' {
            break;
        }
        let v = BASE64.iter().position(|x| *x == c)? as u32;
        tampon = tampon << 6 | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            sortie.push((tampon >> bits) as u8);
            tampon &= (1 << bits) - 1;
        }
    }
    Some(sortie)
}

// ------------------------------------------------------------------------------------- le pliage des en-tetes

fn plier_entete(e: &Entete) -> Result<String, String> {
    Ok(match e {
        Entete::Adresse(nom, v) => plier_insecable(nom, v),
        Entete::De(nom, adresse) => plier_insecable("From", &de_nomme(nom, adresse)),
        Entete::MessageId(v) => plier_insecable("Message-ID", v),
        Entete::Date(v) => plier_insecable("Date", v),
        Entete::Simple(nom, v) => format!("{}: {}\n", nom, v),
        Entete::Libre(nom, v) => plier_libre(nom, v)?,
        Entete::Params { nom, principal, params } => plier_params(nom, principal, params),
    })
}

/// `Nom <adresse>` : un nom ASCII entre guillemets, un nom accentue en mots codes (RFC 2047, base64, 45 octets
/// au plus par mot pour rester sous 75 signes).
fn de_nomme(nom: &str, adresse: &str) -> String {
    if nom.is_ascii() {
        return format!("{} <{}>", guillemets(nom), adresse);
    }
    let mut mots: Vec<String> = Vec::new();
    let mut morceau = String::new();
    for c in nom.chars() {
        if morceau.len() + c.len_utf8() > 45 {
            mots.push(format!("=?utf-8?b?{}?=", base64_brut(morceau.as_bytes())));
            morceau.clear();
        }
        morceau.push(c);
    }
    if !morceau.is_empty() {
        mots.push(format!("=?utf-8?b?{}?=", base64_brut(morceau.as_bytes())));
    }
    format!("{} <{}>", mots.join(" "), adresse)
}

/// Une valeur d'un seul tenant : sur la ligne si elle y tient, sinon seule sur la suivante si elle y tient.
fn plier_insecable(nom: &str, valeur: &str) -> String {
    if nom.len() + 2 + valeur.len() <= MAXLEN {
        format!("{}: {}\n", nom, valeur)
    } else if valeur.len() < MAXLEN {
        format!("{}:\n {}\n", nom, valeur)
    } else {
        format!("{}: {}\n", nom, valeur)
    }
}

/// `utils.quote`.
fn guillemets(v: &str) -> String {
    format!("\"{}\"", v.replace('\\', "\\\\").replace('"', "\\\""))
}

/// `urllib.parse.quote(s, safe='')`.
fn pourcent(s: &str) -> String {
    let mut o = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"_.-~".contains(&b) {
            o.push(b as char);
        } else {
            o.push_str(&format!("%{:02X}", b));
        }
    }
    o
}

/// `Message.set_param` / `_formatparam` : le parametre tel qu'il est range dans l'en-tete.
fn parametre_range(nom: &str, valeur: &str) -> String {
    if valeur.is_empty() {
        nom.to_string()
    } else if !valeur.is_ascii() {
        format!("{}*=utf-8''{}", nom, pourcent(valeur))
    } else {
        format!("{}={}", nom, guillemets(valeur))
    }
}

fn plier_params(nom: &str, principal: &str, params: &[(String, String)]) -> String {
    let mut range = principal.to_string();
    for (n, v) in params {
        range.push_str("; ");
        range.push_str(&parametre_range(n, v));
    }
    // Un parametre RFC 2231 (non ASCII) pese un caractere de plus dans le pliage : le tout doit tenir en 77.
    let limite = if params.iter().any(|(_, v)| !v.is_ascii()) { MAXLEN - 1 } else { MAXLEN };
    if nom.len() + 2 + range.len() <= limite || params.is_empty() {
        return format!("{}: {}\n", nom, range);
    }
    // Le tout ne tient pas : `_fold_mime_parameters`, un parametre apres l'autre.
    let mut lignes: Vec<String> = vec![format!("{}: {};", nom, principal)];
    let mut max = MAXLEN;
    for (n, v) in params {
        let derniere = lignes.last().unwrap();
        if !derniere.trim_end().ends_with(';') {
            lignes.last_mut().unwrap().push(';');
        }
        let (charset, requis) = if v.is_ascii() { ("us-ascii", false) } else { ("utf-8", true) };
        let tstr = if requis {
            format!("{}*={}''{}", n, charset, pourcent(v))
        } else {
            format!("{}={}", n, guillemets(v))
        };
        if lignes.last().unwrap().len() + tstr.len() + 1 < max {
            let l = lignes.last_mut().unwrap();
            l.push(' ');
            l.push_str(&tstr);
            continue;
        } else if tstr.len() + 2 <= max {
            lignes.push(format!(" {}", tstr));
            continue;
        }
        // Plusieurs sections : on les encode toutes.
        let mut section = 0;
        let mut extra = format!("{}''", charset);
        let mut reste: Vec<char> = v.chars().collect();
        while !reste.is_empty() {
            let chrome = n.len() + section.to_string().len() + 3 + extra.len();
            if max <= chrome + 3 {
                max = 78;
            }
            let mut coupe = max - chrome - 2;
            let maxchars = coupe;
            loop {
                let morceau: String = reste[..coupe.min(reste.len())].iter().collect();
                let codee = pourcent(&morceau);
                if codee.len() <= maxchars || coupe <= 1 {
                    lignes.push(format!(" {}*{}*={}{}", n, section, extra, codee));
                    reste = reste[coupe.min(reste.len())..].to_vec();
                    break;
                }
                coupe -= 1;
            }
            extra.clear();
            section += 1;
            if !reste.is_empty() {
                lignes.last_mut().unwrap().push(';');
            }
        }
    }
    lignes.join("\n") + "\n"
}

// ----------------------------------------------------- un en-tete libre : `get_unstructured` et `_refold_parse_tree`

#[derive(Clone, Debug)]
enum Jeton {
    Blanc(String),
    Mot(String),
}

impl Jeton {
    fn texte(&self) -> &str {
        match self {
            Jeton::Blanc(t) | Jeton::Mot(t) => t,
        }
    }
    fn commence_par_blanc(&self) -> bool {
        matches!(self, Jeton::Blanc(_))
    }
}

fn est_blanc(c: char) -> bool {
    c == ' ' || c == '\t'
}

use socle::texte::blanc as est_espace_unicode;

/// `get_unstructured`, sans mots codes : des blancs et des mots. Un blanc COMMENCE par un espace ou une tabulation
/// et `get_fws` (`value.lstrip()`) l'etend a tous les blancs d'Unicode qui suivent, l'espace insecable compris ;
/// un mot va jusqu'au premier espace ou a la premiere tabulation (`_wsp_splitter`), l'espace insecable y compris.
fn jetons_de(valeur: &str) -> Vec<Jeton> {
    let mut jetons = Vec::new();
    let mut reste = valeur;
    while !reste.is_empty() {
        let (blanc, fin) = if reste.starts_with(est_blanc) {
            (true, reste.find(|c: char| !est_espace_unicode(c)).unwrap_or(reste.len()))
        } else {
            (false, reste.find(est_blanc).unwrap_or(reste.len()))
        };
        let (t, suite) = reste.split_at(fin);
        jetons.push(if blanc { Jeton::Blanc(t.to_string()) } else { Jeton::Mot(t.to_string()) });
        reste = suite;
    }
    jetons
}

/// `_steal_trailing_WSP_if_exists`.
fn voler_blanc(lignes: &mut Vec<String>) -> String {
    let mut blanc = String::new();
    if let Some(l) = lignes.last_mut() {
        if let Some(c) = l.chars().last().filter(|c| est_blanc(*c)) {
            blanc.push(c);
            l.pop();
            if l.is_empty() {
                lignes.pop();
            }
        }
    }
    blanc
}

/// `_encoded_words.encode(s, 'utf-8')` : `q` sauf si `b` est plus court de 5 caracteres ou plus.
fn mot_code(s: &[char]) -> String {
    let texte: String = s.iter().collect();
    let octets = texte.as_bytes();
    let q = |b: u8| if b.is_ascii_alphanumeric() || b"-!*+/".contains(&b) { 1 } else if b == b' ' { 1 } else { 3 };
    let longueur_q: usize = octets.iter().map(|b| q(*b)).sum();
    let longueur_b = octets.len().div_ceil(3) * 4;
    if (longueur_q as i64) - (longueur_b as i64) < 5 {
        let mut c = String::new();
        for &b in octets {
            if b == b' ' {
                c.push('_');
            } else if q(b) == 1 {
                c.push(b as char);
            } else {
                c.push_str(&format!("={:02X}", b));
            }
        }
        format!("=?utf-8?q?{}?=", c)
    } else {
        format!("=?utf-8?b?{}?=", base64_brut(octets))
    }
}

/// `str(get_unstructured(texte))` : les mots codes sont decodes, le blanc entre deux mots codes s'efface.
fn decoder_libre(s: &str) -> String {
    enum Piece {
        Blanc(String, bool),
        Mot(String),
        Code(String),
    }
    let mut pieces: Vec<Piece> = Vec::new();
    let mut reste = s;
    while !reste.is_empty() {
        if reste.starts_with(est_blanc) {
            let fin = reste.find(|c: char| !est_espace_unicode(c)).unwrap_or(reste.len());
            pieces.push(Piece::Blanc(reste[..fin].to_string(), false));
            reste = &reste[fin..];
            continue;
        }
        if reste.starts_with("=?") {
            if let Some((texte, suite)) = lire_mot_code(reste) {
                let n = pieces.len();
                if n > 1 && matches!(pieces[n - 1], Piece::Blanc(..)) && matches!(pieces[n - 2], Piece::Code(_)) {
                    if let Piece::Blanc(_, efface) = &mut pieces[n - 1] {
                        *efface = true;
                    }
                }
                pieces.push(Piece::Code(texte));
                reste = suite;
                continue;
            }
        }
        let fin = reste.find(est_blanc).unwrap_or(reste.len());
        pieces.push(Piece::Mot(reste[..fin].to_string()));
        reste = &reste[fin..];
    }
    pieces
        .iter()
        .map(|p| match p {
            Piece::Blanc(_, true) => String::new(),
            Piece::Blanc(t, false) | Piece::Mot(t) | Piece::Code(t) => t.clone(),
        })
        .collect()
}

fn lire_mot_code(v: &str) -> Option<(String, &str)> {
    let apres = &v[2..];
    let fin = apres.find("?=")?;
    let mut tok = apres[..fin].to_string();
    let mut suite = &apres[fin + 2..];
    // Le « ?= » trouve peut etre celui de « q?=C3 » : le mot code continue (`get_encoded_word`).
    let b = suite.as_bytes();
    if b.len() > 1 && b[0].is_ascii_hexdigit() && b[1].is_ascii_hexdigit() && tok.matches('?').count() < 2 {
        match suite.find("?=") {
            Some(f2) => {
                tok = format!("{}?={}", tok, &suite[..f2]);
                suite = &suite[f2 + 2..];
            }
            None => {
                tok = format!("{}?={}", tok, suite);
                suite = "";
            }
        }
    }
    let tok = tok.as_str();
    let morceaux: Vec<&str> = tok.split('?').collect();
    if morceaux.len() != 3 {
        return None;
    }
    let octets = match morceaux[1].to_ascii_lowercase().as_str() {
        "q" => {
            let mut o = Vec::new();
            let b = morceaux[2].as_bytes();
            let mut i = 0;
            while i < b.len() {
                if b[i] == b'_' {
                    o.push(b' ');
                    i += 1;
                } else if b[i] == b'=' && i + 2 < b.len() {
                    match u8::from_str_radix(std::str::from_utf8(&b[i + 1..i + 3]).ok()?, 16) {
                        Ok(x) => {
                            o.push(x);
                            i += 3;
                        }
                        Err(_) => {
                            o.push(b'=');
                            i += 1;
                        }
                    }
                } else {
                    o.push(b[i]);
                    i += 1;
                }
            }
            o
        }
        "b" => base64_lire(morceaux[2])?,
        _ => return None,
    };
    Some((String::from_utf8_lossy(&octets).into_owned(), suite))
}

/// `_fold_as_ew` : ecrit `a_coder` en mots codes sur les lignes, en le recombinant avec les mots codes qui
/// terminent deja la ligne. Rend la nouvelle position du premier mot code de la ligne.
fn plier_en_mot_code(
    a_coder: &str,
    lignes: &mut Vec<String>,
    dernier_mot_code: Option<usize>,
    dernier_est_code: bool,
) -> Result<usize, String> {
    let mut a_coder = a_coder.to_string();
    if let Some(pos) = dernier_mot_code {
        let segment = lignes.last().unwrap()[pos..].to_string();
        a_coder = decoder_libre(&format!("{}{}", segment, a_coder));
        lignes.last_mut().unwrap().truncate(pos);
    } else if dernier_est_code {
        let l = lignes.last().unwrap().clone();
        let sans = l.trim_end_matches(est_blanc).len();
        let blanc = l[sans..].to_string();
        *lignes.last_mut().unwrap() = format!("{}{}", &l[..sans], if blanc.is_empty() { "" } else { " " });
        a_coder = format!("{}{}", blanc, a_coder);
    } else if a_coder.starts_with(est_blanc) {
        let c = a_coder.remove(0);
        if lignes.last().unwrap().len() == MAXLEN {
            let v = voler_blanc(lignes);
            lignes.push(v);
        }
        lignes.last_mut().unwrap().push(c);
    }
    let mut blanc_final = String::new();
    if a_coder.ends_with(est_blanc) {
        blanc_final = a_coder.pop().map(String::from).unwrap_or_default();
    }
    let nouveau = dernier_mot_code.unwrap_or_else(|| lignes.last().unwrap().len());
    let mut nouveau = nouveau;
    let chrome = "utf-8".len() + 7;
    let mut reste: Vec<char> = a_coder.chars().collect();
    while !reste.is_empty() {
        let place = MAXLEN as i64 - lignes.last().unwrap().len() as i64;
        let place_texte = place - chrome as i64;
        if place_texte <= 0 {
            let v = voler_blanc(lignes);
            lignes.push(if v.is_empty() { " ".to_string() } else { v });
            nouveau = lignes.last().unwrap().len();
            continue;
        }
        let mut n = (place_texte as usize).min(reste.len());
        let mut mot = mot_code(&reste[..n]);
        while mot.len() as i64 - place > 0 && n > 0 {
            n -= 1;
            mot = mot_code(&reste[..n]);
        }
        lignes.last_mut().unwrap().push_str(&mot);
        reste = reste[n..].to_vec();
        if !reste.is_empty() {
            lignes.push(" ".to_string());
            nouveau = lignes.last().unwrap().len();
        }
    }
    lignes.last_mut().unwrap().push_str(&blanc_final);
    Ok(nouveau)
}

enum Morceau {
    Liste(Vec<Jeton>),
    Seul(Jeton),
}

/// L'en-tete `nom: valeur` d'un texte libre, plie comme `_refold_parse_tree` (nom et `: ` compris, \n final).
fn plier_libre(nom: &str, valeur: &str) -> Result<String, String> {
    let mut lignes: Vec<String> = vec![format!("{}:", nom)];
    let jetons = jetons_de(valeur);
    if !jetons.is_empty() {
        lignes[0].push(' ');
    }
    let mut dernier_est_code = false;
    let mut dernier_mot_code: Option<usize> = None;
    let mut dernier_charset: Option<&str> = None;
    let mut veut_coder = false;
    let mut parts: VecDeque<Morceau> = VecDeque::new();
    parts.push_back(Morceau::Liste(jetons));
    while let Some(part) = parts.pop_front() {
        let (tstr, commence_par_blanc, est_liste) = match &part {
            Morceau::Liste(l) => (l.iter().map(|j| j.texte()).collect::<String>(), l.first().map_or(false, |j| j.commence_par_blanc()), true),
            Morceau::Seul(j) => (j.texte().to_string(), j.commence_par_blanc(), false),
        };
        let charset = if tstr.is_ascii() {
            "us-ascii"
        } else {
            veut_coder = true;
            "utf-8"
        };
        if veut_coder {
            if est_liste {
                // Pas une feuille : on traite chaque jeton.
                if let Morceau::Liste(l) = part {
                    for j in l.into_iter().rev() {
                        parts.push_front(Morceau::Seul(j));
                    }
                }
                veut_coder = false;
                continue;
            }
            if dernier_mot_code.is_some()
                && Some(charset) != dernier_charset
                && (dernier_charset == Some("unknown-8bit") || dernier_charset == Some("utf-8") && charset != "us-ascii")
            {
                dernier_mot_code = None;
            }
            dernier_mot_code = Some(plier_en_mot_code(&tstr, &mut lignes, dernier_mot_code, dernier_est_code)?);
            dernier_est_code = true;
            dernier_charset = Some(charset);
            veut_coder = false;
            continue;
        }
        let courante = lignes.last().map_or(0, |l| l.len());
        if tstr.len() <= MAXLEN.saturating_sub(courante) {
            if lignes.is_empty() {
                lignes.push(String::new());
            }
            lignes.last_mut().unwrap().push_str(&tstr);
            dernier_est_code = dernier_est_code && tstr.trim_matches(est_blanc).is_empty();
            continue;
        }
        if tstr.len() < MAXLEN {
            let nouvelle = voler_blanc(&mut lignes);
            if !nouvelle.is_empty() || commence_par_blanc {
                lignes.push(format!("{}{}", nouvelle, tstr));
                dernier_est_code = dernier_est_code && lignes.last().unwrap().trim_matches(est_blanc).is_empty();
                dernier_mot_code = None;
                continue;
            }
        }
        if est_liste {
            if let Morceau::Liste(l) = part {
                for j in l.into_iter().rev() {
                    parts.push_front(Morceau::Seul(j));
                }
            }
            continue;
        }
        // Une feuille qui ne tient pas : on la code pour pouvoir la couper.
        if let Morceau::Seul(j) = part {
            parts.push_front(Morceau::Seul(j));
        }
        veut_coder = true;
    }
    Ok(lignes.join("\n") + "\n")
}

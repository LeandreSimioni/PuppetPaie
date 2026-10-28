//! `courriel` sans reseau : `serveurs` avec et sans `config/courriel.json`, la boite simulee de la compagnie fictive,
//! le dialogue IMAP contre un faux serveur en memoire (un flux qui rend des reponses ecrites d'avance : les litteraux
//! `{n}` se lisent par leur longueur, jamais par ce qu'ils contiennent), la traduction des refus, les destinataires
//! d'un message. Rien n'ouvre de connexion, rien n'envoie : `Client` n'est jamais appele. L'adresse et le secret lus dans
//! l'environnement : `tests/courriel_environnement.rs`.

use std::cell::RefCell;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::rc::Rc;

use envois::courriel::{self, BoiteSimulee, Courriel, Imap, Serveurs};
use serde_json::{json, Value};

fn racine(nom: &str) -> PathBuf {
    let r = crate::outils::dossier_de_test("courriel", nom);
    std::fs::create_dir_all(r.join("config")).unwrap();
    r
}

// ------------------------------------------------------------------------------------------------ les serveurs

#[test]
fn serveurs_gmail_par_defaut() {
    let r = racine("sans-fichier");
    assert_eq!(courriel::serveurs(&r).unwrap(), Serveurs::gmail());
    let g = Serveurs::gmail();
    assert_eq!((g.imap.as_str(), g.port_imap, g.smtp.as_str(), g.port_smtp), ("imap.gmail.com", 993, "smtp.gmail.com", 465));
}

#[test]
fn serveurs_selon_courriel_json() {
    let r = racine("avec-fichier");
    std::fs::write(
        r.join("config").join("courriel.json"),
        r#"{"imap": "imap.exemple.fr", "smtp": "smtp.exemple.fr", "port_smtp": 587, "inconnue": 1}"#,
    )
    .unwrap();
    let s = courriel::serveurs(&r).unwrap();
    assert_eq!(s, Serveurs { imap: "imap.exemple.fr".into(), port_imap: 993, smtp: "smtp.exemple.fr".into(), port_smtp: 587 });
    // Un port ecrit en texte se lit aussi.
    std::fs::write(r.join("config").join("courriel.json"), r#"{"port_imap": "143", "port_smtp": "25"}"#).unwrap();
    let s = courriel::serveurs(&r).unwrap();
    assert_eq!((s.imap.as_str(), s.port_imap, s.smtp.as_str(), s.port_smtp), ("imap.gmail.com", 143, "smtp.gmail.com", 25));
}

#[test]
fn serveurs_fichier_illisible() {
    let r = racine("illisible");
    std::fs::write(r.join("config").join("courriel.json"), "{ pas du json").unwrap();
    let e = courriel::serveurs(&r).unwrap_err();
    assert!(e.0.contains("courriel.json") && e.0.contains("illisible"), "{}", e);
    std::fs::write(r.join("config").join("courriel.json"), r#"{"port_smtp": "abc"}"#).unwrap();
    assert!(courriel::serveurs(&r).unwrap_err().0.contains("port"));
}

// ---------------------------------------------------------------------------------------- l'adresse et le secret

#[test]
fn traduction_des_refus() {
    for t in [
        "NO [AUTHENTICATIONFAILED] Invalid credentials (Failure)",
        "permanent error (535): 5.7.8 Username and Password not accepted",
    ] {
        assert!(courriel::traduire(t).contains("mot de passe D'APPLICATION"), "{}", t);
    }
    assert_eq!(courriel::traduire("autre chose"), "autre chose");
    assert_eq!(courriel::traduire("Message 1535 introuvable."), "Message 1535 introuvable.");
}

// ------------------------------------------------------------------------------------------- la boite simulee

fn fiche(signataire: Value) -> Value {
    json!({"signataire": signataire})
}

#[test]
fn boite_simulee_le_compte() {
    let r = racine("boite-compte");
    assert_eq!(BoiteSimulee::nouvelle(&r, &fiche(json!({"email": "Direction@Demo.INVALID"}))).compte(), "direction@demo.invalid");
    assert_eq!(BoiteSimulee::nouvelle(&r, &fiche(json!({}))).compte(), "fictive@puppetpaie.invalid");
    assert_eq!(BoiteSimulee::nouvelle(&r, &fiche(json!({"email": ""}))).compte(), "fictive@puppetpaie.invalid");
    assert_eq!(BoiteSimulee::nouvelle(&r, &json!({})).compte(), "fictive@puppetpaie.invalid");
}

#[test]
fn boite_simulee_garde_sans_envoyer() {
    let r = racine("boite-envoi");
    let mut b = BoiteSimulee::nouvelle(&r, &fiche(json!({"email": "direction@demo.invalid"})));
    let m1 = b"From: a@b.fr\nMessage-ID: <un.deux@demo.invalid>\nSubject: un\n\ncorps\n".to_vec();
    // Un Message-ID plie sur la ligne suivante se lit aussi (`\s*` couvre la fin de ligne), la casse ne compte pas.
    let m2 = b"From: a@b.fr\nmessage-id:\n <plie@demo.invalid>\nSubject: deux\n\ncorps\n".to_vec();
    let m3 = b"From: a@b.fr\nSubject: sans identifiant\n\ncorps\n".to_vec();
    assert_eq!(b.envoyer(&m1).unwrap(), "<un.deux@demo.invalid>");
    assert_eq!(b.envoyer(&m2).unwrap(), "<plie@demo.invalid>");
    let i3 = b.envoyer(&m3).unwrap();
    // Sans identifiant, la boite en donne un (un Message-ID : `<...@...>`), comme un serveur.
    assert!(i3.starts_with('<') && i3.ends_with('>') && i3.contains('@'), "{}", i3);
    // Chaque message est ecrit tel quel dans `courriels-simules/`.
    let mut noms: Vec<String> = std::fs::read_dir(r.join("courriels-simules"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    noms.sort();
    assert_eq!(noms.len(), 3);
    let ecrits: Vec<Vec<u8>> = noms.iter().map(|n| std::fs::read(r.join("courriels-simules").join(n)).unwrap()).collect();
    for m in [&m1, &m2, &m3] {
        assert!(ecrits.contains(m));
    }
    // Elle ne retrouve rien : la compagnie fictive releve ses signatures a sa maniere.
    assert!(b.rechercher("Je signe", 50).unwrap().is_empty());
    assert!(b.message_brut("1").unwrap().is_empty());
    b.fermer();
}

// ------------------------------------------------------------------------------------------------ l'IMAP

/// Un flux en memoire : ce que le serveur dira (par morceaux de `par` octets au plus), et tout ce qu'on lui ecrit.
struct Faux {
    a_lire: Vec<u8>,
    pos: usize,
    par: usize,
    ecrit: Rc<RefCell<Vec<u8>>>,
}

impl Read for Faux {
    fn read(&mut self, tampon: &mut [u8]) -> std::io::Result<usize> {
        let n = tampon.len().min(self.par).min(self.a_lire.len() - self.pos);
        tampon[..n].copy_from_slice(&self.a_lire[self.pos..self.pos + n]);
        self.pos += n;
        Ok(n)
    }
}

impl Write for Faux {
    fn write(&mut self, o: &[u8]) -> std::io::Result<usize> {
        self.ecrit.borrow_mut().extend_from_slice(o);
        Ok(o.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn faux(serveur: &[u8], par: usize) -> (Faux, Rc<RefCell<Vec<u8>>>) {
    let ecrit = Rc::new(RefCell::new(Vec::new()));
    (Faux { a_lire: serveur.to_vec(), pos: 0, par, ecrit: ecrit.clone() }, ecrit)
}

const ACCUEIL: &[u8] = b"* OK Gimap ready for requests from 192.0.2.1 abc\r\n";
const CONNEXION: &[u8] = b"* CAPABILITY IMAP4rev1 UNSELECT IDLE\r\nA0001 OK compagnie@gmail.com authenticated (Success)\r\n\
* FLAGS (\\Answered \\Flagged \\Draft \\Deleted \\Seen)\r\n* OK [PERMANENTFLAGS ()] Flags permitted.\r\n\
* OK [UIDVALIDITY 1] UIDs valid.\r\n* 42 EXISTS\r\n* 0 RECENT\r\n* OK [UIDNEXT 99] Predicted next UID.\r\n\
A0002 OK [READ-ONLY] INBOX selected. (Success)\r\n";

fn ouvert(suite: &[u8], par: usize) -> (Imap<Faux>, Rc<RefCell<Vec<u8>>>) {
    let mut dit = ACCUEIL.to_vec();
    dit.extend_from_slice(CONNEXION);
    dit.extend_from_slice(suite);
    let (f, ecrit) = faux(&dit, par);
    let imap = Imap::ouvrir(f, "compagnie@gmail.com", "abcdefghijklmnop", "INBOX").unwrap();
    (imap, ecrit)
}

fn ecrit(e: &Rc<RefCell<Vec<u8>>>) -> String {
    String::from_utf8_lossy(&e.borrow()).into_owned()
}

#[test]
fn imap_connexion_login_et_examine() {
    let (_, e) = ouvert(b"", 8192);
    // LOGIN, puis EXAMINE (lecture seule) : jamais SELECT.
    assert_eq!(ecrit(&e), "A0001 LOGIN \"compagnie@gmail.com\" \"abcdefghijklmnop\"\r\nA0002 EXAMINE \"INBOX\"\r\n");
}

#[test]
fn imap_recherche_les_plus_recents_d_abord() {
    let (mut imap, e) = ouvert(b"* SEARCH 4 7 9 12\r\nA0003 OK SEARCH completed (Success)\r\n", 8192);
    // Les guillemets de la syntaxe Gmail sont toleres et retires.
    assert_eq!(imap.rechercher("  \"Je signe 12345\"  ", 3).unwrap(), vec!["12", "9", "7"]);
    assert!(ecrit(&e).ends_with("A0003 UID SEARCH TEXT \"Je signe 12345\"\r\n"), "{}", ecrit(&e));
}

#[test]
fn imap_recherche_sans_resultat() {
    let (mut imap, _) = ouvert(b"* SEARCH\r\nA0003 OK SEARCH completed (Success)\r\n", 8192);
    assert!(imap.rechercher("rien", 50).unwrap().is_empty());
    let (mut imap, _) = ouvert(b"A0003 OK SEARCH completed (Success)\r\n", 8192);
    assert!(imap.rechercher("rien", 50).unwrap().is_empty());
}

#[test]
fn imap_recherche_echappe_ce_qui_casserait_la_commande() {
    let (mut imap, e) = ouvert(b"* SEARCH\r\nA0003 OK done\r\n", 8192);
    imap.rechercher("a\"b\\c", 5).unwrap();
    assert!(ecrit(&e).ends_with("A0003 UID SEARCH TEXT \"a\\\"b\\\\c\"\r\n"), "{}", ecrit(&e));
    // Une fin de ligne dans la recherche ne passe pas : rien n'est ecrit.
    let (mut imap, e) = ouvert(b"", 8192);
    let avant = e.borrow().len();
    assert!(imap.rechercher("a\r\nA0009 LOGOUT", 5).is_err());
    assert_eq!(e.borrow().len(), avant);
}

/// Un message dont le corps ressemble a une reponse du serveur : seule la longueur annoncee compte.
fn message_piege() -> Vec<u8> {
    let mut m = b"From: a@b.fr\r\nSubject: pi\xc3\xa8ge {5}\r\n\r\n".to_vec();
    m.extend_from_slice(b")\r\nA0004 OK FETCH completed\r\n* 9 EXISTS\r\n{3}\r\n");
    m.extend_from_slice(&(0u8..=255).collect::<Vec<u8>>());
    m.extend_from_slice(b"\r\nfin sans retour");
    m
}

fn fetch_de(corps: &[u8]) -> Vec<u8> {
    let mut s = format!("* 2 FETCH (UID 7 BODY[] {{{}}}\r\n", corps.len()).into_bytes();
    s.extend_from_slice(corps);
    s.extend_from_slice(b")\r\nA0003 OK Success\r\n");
    s
}

#[test]
fn imap_litteral_lu_par_sa_longueur() {
    let corps = message_piege();
    for par in [8192, 1, 7, 64] {
        let (mut imap, e) = ouvert(&fetch_de(&corps), par);
        let lu = imap.message_brut("7").unwrap();
        assert_eq!(lu, corps, "par morceaux de {}", par);
        // BODY.PEEK : le message n'est pas marque lu.
        assert!(ecrit(&e).ends_with("A0003 UID FETCH 7 (BODY.PEEK[])\r\n"), "{}", ecrit(&e));
    }
}

#[test]
fn imap_message_introuvable() {
    let (mut imap, _) = ouvert(b"A0003 OK Success\r\n", 8192);
    assert!(imap.message_brut("7").unwrap_err().0.contains('7'), "le refus nomme le message");
    // Un identifiant qui n'est pas un UID n'est jamais ecrit sur le fil.
    let (mut imap, e) = ouvert(b"", 8192);
    let avant = e.borrow().len();
    for mauvais in ["", "7 8", "7\r\nA0009 LOGOUT", "*", "1:*", "-1"] {
        assert!(imap.message_brut(mauvais).is_err(), "{:?}", mauvais);
    }
    assert_eq!(e.borrow().len(), avant);
}

#[test]
fn imap_plusieurs_commandes_sur_la_meme_connexion() {
    let mut suite = b"* SEARCH 5\r\nA0003 OK done\r\n".to_vec();
    suite.extend_from_slice(&fetch_de(b"un message\r\n"));
    let suite = String::from_utf8(suite).unwrap().replace("A0003 OK Success", "A0004 OK Success");
    let (mut imap, e) = ouvert(suite.as_bytes(), 3);
    assert_eq!(imap.rechercher("x", 50).unwrap(), vec!["5"]);
    assert_eq!(imap.message_brut("5").unwrap(), b"un message\r\n");
    imap.deconnecter();
    assert!(ecrit(&e).ends_with("A0005 LOGOUT\r\n"));
}

#[test]
fn imap_refus_du_mot_de_passe_en_francais() {
    let mut dit = ACCUEIL.to_vec();
    dit.extend_from_slice(b"A0001 NO [AUTHENTICATIONFAILED] Invalid credentials (Failure)\r\n");
    let (f, e) = faux(&dit, 8192);
    let err = Imap::ouvrir(f, "compagnie@gmail.com", "mauvais", "INBOX").err().unwrap();
    assert!(err.0.contains("mot de passe D'APPLICATION"), "{}", err);
    // La suite n'a pas ete tentee.
    assert!(!ecrit(&e).contains("EXAMINE"));
    // Le mot de passe n'est jamais dans l'erreur.
    assert!(!err.0.contains("mauvais"));
}

#[test]
fn imap_le_serveur_raccroche() {
    let (f, _) = faux(b"* BYE Autologout; idle for too long\r\n", 8192);
    assert!(Imap::ouvrir(f, "a@b.fr", "x", "INBOX").is_err());
    // Coupe en pleine reponse.
    let (f, _) = faux(b"* OK pret\r\nA0001 OK", 8192);
    let e = Imap::ouvrir(f, "a@b.fr", "x", "INBOX").err().unwrap();
    assert!(e.0.contains("fermee"), "{}", e);
    // Un litteral annonce plus long que ce qui arrive.
    let (f, _) = faux(b"* OK pret\r\nA0001 OK ok\r\nA0002 OK ok\r\n", 8192);
    let mut imap = Imap::ouvrir(f, "a@b.fr", "x", "INBOX").unwrap();
    assert!(imap.commande("UID FETCH 1 (BODY.PEEK[])").is_err());
}

#[test]
fn imap_le_mot_de_passe_se_cite_et_les_controles_se_refusent() {
    let mut dit = ACCUEIL.to_vec();
    dit.extend_from_slice(b"A0001 OK ok\r\nA0002 OK ok\r\n");
    let (f, e) = faux(&dit, 8192);
    Imap::ouvrir(f, "a@b.fr", "un\"deux\\trois", "INBOX").unwrap();
    assert!(ecrit(&e).starts_with("A0001 LOGIN \"a@b.fr\" \"un\\\"deux\\\\trois\"\r\n"), "{}", ecrit(&e));
    let (f, e) = faux(&dit, 8192);
    assert!(Imap::ouvrir(f, "a@b.fr", "un\r\nA0009 LOGOUT", "INBOX").is_err());
    assert_eq!(ecrit(&e), "");
}

// ------------------------------------------------------------------------------------------- les destinataires

#[test]
fn destinataires_de_to_cc_bcc() {
    let brut = b"From: a@b.fr\nTo: Marie Dupont <marie@exemple.fr>, \"Durand, Paul\" <paul@exemple.fr>\n\
Cc: cc@exemple.fr\nBcc: cache@exemple.fr, equipe: x@y.fr, z@y.fr;\nSubject: s\n\nc\n";
    let (entetes, _) = mailparse::parse_headers(brut).unwrap();
    assert_eq!(
        courriel::destinataires(&entetes),
        vec!["marie@exemple.fr", "paul@exemple.fr", "cc@exemple.fr", "cache@exemple.fr", "x@y.fr", "z@y.fr"]
    );
    let (entetes, _) = mailparse::parse_headers(b"To: a@b.fr\nTo: autre@b.fr\n\n").unwrap();
    // Le premier en-tete de chaque nom.
    assert_eq!(courriel::destinataires(&entetes), vec!["a@b.fr"]);
}


// ------------------------------------------------------------------------ ce qui part sur le fil SMTP (lot H3a, la preuve)

/// Ce que `lettre` ecrira : ses donnees, puis `\r\n.\r\n`.
fn sur_le_fil(brut: &[u8]) -> Vec<u8> {
    let mut o = courriel::donnees_du_fil(brut);
    o.extend_from_slice(b"\r\n.\r\n");
    o
}

/// Le message tel quel, sa derniere ligne terminee, puis le point.
fn termine(brut: &[u8]) -> Vec<u8> {
    let mut o = brut.to_vec();
    if !o.ends_with(b"\r\n") {
        o.extend_from_slice(b"\r\n");
    }
    o.extend_from_slice(b".\r\n");
    o
}

#[test]
fn les_donnees_du_fil_sont_le_message_termine_sans_lf_nu() {
    for brut in [
        &b"a\nb\n"[..],
        b"a\nb",
        b"a\nb\n\n\n",
        b"",
        b"x\r\n",
        b"x\r\ny\r\n",
        b"--fin--\n",
    ] {
        let attendu = termine(brut);
        let obtenu = sur_le_fil(brut);
        let normal = |o: &[u8]| String::from_utf8_lossy(o).replace("\r\n", "\n");
        assert_eq!(normal(&obtenu), normal(&attendu), "{:?}", String::from_utf8_lossy(brut));
        // Aucune fin de ligne nue ne part sur le fil.
        for (i, o) in obtenu.iter().enumerate() {
            assert!(*o != b'\n' || (i > 0 && obtenu[i - 1] == b'\r'), "LF nu dans {:?}", obtenu);
        }
    }
    // Un message d'`en_mime` (fin de ligne LF) : sa derniere fin de ligne est celle qui precede le point.
    assert_eq!(courriel::donnees_du_fil(b"a\n"), b"a\r\n");
    assert_eq!(courriel::donnees_du_fil(b"a\r\n"), b"a");
    assert_eq!(courriel::donnees_du_fil(b"a"), b"a");
    assert_eq!(courriel::donnees_du_fil(b"a\rb\n"), b"a\r\nb\r\n");
}

#[test]
fn imap_des_reponses_non_etiquetees_avant_et_apres_le_litteral() {
    // `* 1 FETCH (FLAGS (\Seen))` avant le message : imaplib le prenait pour le message (« introuvable ») ; ici il se lit.
    let corps = message_piege();
    let mut suite = b"* 1 FETCH (FLAGS (\\Seen))\r\n* 6 EXISTS\r\n".to_vec();
    suite.extend_from_slice(b"* 1 FETCH (UID 7 RFC822.SIZE ");
    suite.extend_from_slice(format!("{} BODY[] {{{}}}\r\n", corps.len(), corps.len()).as_bytes());
    suite.extend_from_slice(&corps);
    suite.extend_from_slice(b")\r\n* OK [ALERT] bonjour\r\nA0003 OK Success\r\n");
    for par in [8192, 5] {
        let (mut imap, _) = ouvert(&suite, par);
        assert_eq!(imap.message_brut("7").unwrap(), corps, "par {}", par);
    }
    // Les attributs peuvent suivre le litteral.
    let mut suite = format!("* 1 FETCH (BODY[] {{{}}}\r\n", corps.len()).into_bytes();
    suite.extend_from_slice(&corps);
    suite.extend_from_slice(b" UID 7 FLAGS (\\Seen))\r\nA0003 OK Success\r\n");
    let (mut imap, _) = ouvert(&suite, 64);
    assert_eq!(imap.message_brut("7").unwrap(), corps);
}

#[test]
fn imap_un_litteral_de_plusieurs_megaoctets() {
    let corps: Vec<u8> = (0..6_000_000u32).map(|i| (i.wrapping_mul(2654435761) >> 24) as u8).collect();
    let mut suite = format!("* 1 FETCH (UID 7 BODY[] {{{}}}\r\n", corps.len()).into_bytes();
    suite.extend_from_slice(&corps);
    suite.extend_from_slice(b")\r\nA0003 OK Success\r\n");
    let (mut imap, _) = ouvert(&suite, 65536);
    assert_eq!(imap.message_brut("7").unwrap().len(), corps.len());
}

#[test]
fn boite_simulee_les_noms_se_trient_dans_l_ordre_des_envois() {
    let r = racine("boite-ordre");
    let mut b = BoiteSimulee::nouvelle(&r, &json!({}));
    let messages: Vec<Vec<u8>> = (0..25).map(|i| format!("From: a@b.fr\nMessage-ID: <n{}@demo.invalid>\nSubject: {}\n\ncorps\n", i, i).into_bytes()).collect();
    for m in &messages {
        b.envoyer(m).unwrap();
    }
    let mut noms: Vec<String> = std::fs::read_dir(r.join("courriels-simules")).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    noms.sort();
    let ecrits: Vec<Vec<u8>> = noms.iter().map(|n| std::fs::read(r.join("courriels-simules").join(n)).unwrap()).collect();
    assert_eq!(ecrits, messages);
}

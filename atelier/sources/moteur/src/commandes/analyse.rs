//! L'analyse des arguments d'une commande, a la facon d'`argparse` (la ligne de commande Python, `moteur/cli.py`) :
//! options longues, abreviations non ambigues (`--arch` pour `--archives`), `--option=valeur`, positionnels
//! facultatifs, choix fermes, entiers. Un refus d'analyse a le code 2 et la forme d'`argparse` :
//! `usage: ...` puis `moteur COMMANDE: error: ...` sur la sortie d'erreur.
//!
//! Rien ici ne sait ce que fait une commande : une commande declare ses options (`Spec`), l'analyse rend ce qui a
//! ete tape (`Args`).

use std::collections::HashMap;

/// Ce qu'une option attend.
#[derive(Clone, Copy, PartialEq)]
pub enum Genre {
    /// `--archives` : un drapeau.
    Drapeau,
    /// `--depuis JJ/MM/AAAA` : une valeur.
    Valeur,
    /// `--jours 30` : un entier.
    Entier,
    /// `--quoi dsn` : une valeur parmi des choix fermes.
    Choix(&'static [&'static str]),
    /// `--salarie A --salarie B` : la valeur s'ajoute a chaque fois.
    Ajout,
    /// `--seulement CAT --seulement CAT` (`action="append"`, `choices=...`) : une valeur parmi des choix fermes, a repeter.
    AjoutParmi(&'static [&'static str]),
    /// `--seulement A B C` (`nargs="+"`) : une ou plusieurs valeurs a la suite, jusqu'a la prochaine option.
    Plusieurs,
}

pub struct Opt {
    pub nom: &'static str,
    pub genre: Genre,
    pub defaut: Option<&'static str>,
}

impl Opt {
    pub const fn drapeau(nom: &'static str) -> Opt {
        Opt { nom, genre: Genre::Drapeau, defaut: None }
    }

    pub const fn valeur(nom: &'static str) -> Opt {
        Opt { nom, genre: Genre::Valeur, defaut: None }
    }

    pub const fn avec_defaut(nom: &'static str, defaut: &'static str) -> Opt {
        Opt { nom, genre: Genre::Valeur, defaut: Some(defaut) }
    }

    pub const fn entier(nom: &'static str) -> Opt {
        Opt { nom, genre: Genre::Entier, defaut: None }
    }

    pub const fn choix(nom: &'static str, choix: &'static [&'static str], defaut: Option<&'static str>) -> Opt {
        Opt { nom, genre: Genre::Choix(choix), defaut }
    }

    pub const fn ajout(nom: &'static str) -> Opt {
        Opt { nom, genre: Genre::Ajout, defaut: None }
    }

    pub const fn ajout_parmi(nom: &'static str, choix: &'static [&'static str]) -> Opt {
        Opt { nom, genre: Genre::AjoutParmi(choix), defaut: None }
    }

    pub const fn plusieurs(nom: &'static str) -> Opt {
        Opt { nom, genre: Genre::Plusieurs, defaut: None }
    }
}

/// Un argument sans tiret (`moteur coffre etat`, `moteur filet activer ADRESSE`).
pub struct Pos {
    pub nom: &'static str,
    pub choix: Option<&'static [&'static str]>,
    pub defaut: Option<&'static str>,
    /// Sans `nargs="?"` : `argparse` le reclame (`the following arguments are required: mois`).
    pub obligatoire: bool,
    /// `nargs="*"` : tout ce qui reste (aucun, un ou plusieurs).
    pub multiple: bool,
}

impl Pos {
    pub const fn facultatif(nom: &'static str, defaut: Option<&'static str>) -> Pos {
        Pos { nom, choix: None, defaut, obligatoire: false, multiple: false }
    }

    pub const fn parmi(nom: &'static str, choix: &'static [&'static str], defaut: Option<&'static str>) -> Pos {
        Pos { nom, choix: Some(choix), defaut, obligatoire: false, multiple: false }
    }

    /// Des positionnels en nombre libre (`dossier CLE CLE ...`, `nargs="*"`), aucun permis.
    pub const fn plusieurs(nom: &'static str) -> Pos {
        Pos { nom, choix: None, defaut: None, obligatoire: false, multiple: true }
    }

    /// Un positionnel que la commande exige (`fichier`, `mois`).
    pub const fn requis(nom: &'static str) -> Pos {
        Pos { nom, choix: None, defaut: None, obligatoire: true, multiple: false }
    }
}

/// Les options et les positionnels d'une commande.
pub struct Spec {
    pub commande: &'static str,
    pub positionnels: &'static [Pos],
    pub options: &'static [Opt],
}

/// Ce qui a ete tape.
#[derive(Default, Debug)]
pub struct Args {
    valeurs: HashMap<String, Vec<String>>,
}

impl Args {
    pub fn drapeau(&self, nom: &str) -> bool {
        self.valeurs.contains_key(nom)
    }

    pub fn texte(&self, nom: &str) -> Option<&str> {
        self.valeurs.get(nom).and_then(|v| v.last()).map(String::as_str)
    }

    pub fn entier(&self, nom: &str) -> Option<i64> {
        self.texte(nom).and_then(|t| t.parse().ok())
    }

    pub fn liste(&self, nom: &str) -> Vec<String> {
        self.valeurs.get(nom).cloned().unwrap_or_default()
    }
}

/// Un refus d'analyse : la phrase d'`argparse` (`error: ...`).
pub struct Refus(pub String);

pub fn usage(spec: &Spec) -> String {
    let mut u = format!("usage: moteur {}", spec.commande);
    for o in spec.options {
        u.push_str(&match o.genre {
            Genre::Drapeau => format!(" [--{}]", o.nom),
            Genre::Entier => format!(" [--{} N]", o.nom),
            _ => format!(" [--{} {}]", o.nom, o.nom.to_uppercase().replace('-', "_")),
        });
    }
    for p in spec.positionnels {
        if p.multiple {
            u.push_str(&format!(" [{} ...]", p.nom));
        } else if p.obligatoire {
            u.push_str(&format!(" {}", p.nom));
        } else {
            u.push_str(&format!(" [{}]", p.nom));
        }
    }
    u
}

/// `int(texte)` de Python pour un argument : espaces et signe permis.
fn entier_de(t: &str) -> Option<i64> {
    t.trim().parse::<i64>().ok()
}

/// Analyse `args` (ce qui suit le nom de la commande). Rend ce qui a ete tape, ou le refus.
pub fn analyser(spec: &Spec, args: &[String]) -> Result<Args, Refus> {
    let mut r = Args::default();
    for o in spec.options {
        if let Some(d) = o.defaut {
            r.valeurs.insert(o.nom.to_string(), vec![d.to_string()]);
        }
    }
    let mut pos: Vec<&String> = Vec::new();
    let mut i = 0;
    let mut fin_des_options = false;
    while i < args.len() {
        let a = &args[i];
        i += 1;
        if fin_des_options || !a.starts_with("--") || a.len() == 2 {
            if !fin_des_options && a == "--" {
                fin_des_options = true;
                continue;
            }
            pos.push(a);
            continue;
        }
        let (brut, valeur_collee) = match a[2..].split_once('=') {
            Some((n, v)) => (n, Some(v.to_string())),
            None => (&a[2..], None),
        };
        // Le nom exact d'abord, puis l'abreviation si une seule option y repond.
        let opt = match spec.options.iter().find(|o| o.nom == brut) {
            Some(o) => o,
            None => {
                let candidates: Vec<&Opt> = spec.options.iter().filter(|o| o.nom.starts_with(brut)).collect();
                match candidates.len() {
                    1 => candidates[0],
                    0 => return Err(Refus(format!("unrecognized arguments: {}", a))),
                    _ => {
                        let noms: Vec<String> = candidates.iter().map(|o| format!("--{}", o.nom)).collect();
                        return Err(Refus(format!("ambiguous option: --{} could match {}", brut, noms.join(", "))));
                    }
                }
            }
        };
        match opt.genre {
            Genre::Drapeau => {
                if valeur_collee.is_some() {
                    return Err(Refus(format!("argument --{}: ignored explicit argument", opt.nom)));
                }
                r.valeurs.insert(opt.nom.to_string(), vec![String::new()]);
            }
            Genre::Plusieurs => {
                // `nargs="+"` : au moins une valeur, puis tout ce qui ne ressemble pas a une option.
                let mut vs: Vec<String> = Vec::new();
                if let Some(v) = valeur_collee {
                    vs.push(v);
                }
                while let Some(v) = args.get(i) {
                    if v.starts_with("--") && v.len() > 2 {
                        break;
                    }
                    vs.push(v.clone());
                    i += 1;
                }
                if vs.is_empty() {
                    return Err(Refus(format!("argument --{}: expected at least one argument", opt.nom)));
                }
                r.valeurs.insert(opt.nom.to_string(), vs);
            }
            _ => {
                let v = match valeur_collee {
                    Some(v) => v,
                    None => match args.get(i) {
                        Some(v) if !(v.starts_with("--") && v.len() > 2) => {
                            i += 1;
                            v.clone()
                        }
                        _ => return Err(Refus(format!("argument --{}: expected one argument", opt.nom))),
                    },
                };
                match opt.genre {
                    Genre::Entier if entier_de(&v).is_none() => {
                        return Err(Refus(format!("argument --{}: invalid int value: '{}'", opt.nom, v)));
                    }
                    Genre::Choix(c) | Genre::AjoutParmi(c) if !c.contains(&v.as_str()) => {
                        let liste: Vec<String> = c.iter().map(|x| format!("'{}'", x)).collect();
                        return Err(Refus(format!("argument --{}: invalid choice: '{}' (choose from {})", opt.nom, v, liste.join(", "))));
                    }
                    _ => {}
                }
                let e = r.valeurs.entry(opt.nom.to_string()).or_default();
                if opt.genre == Genre::Ajout || matches!(opt.genre, Genre::AjoutParmi(_)) {
                    e.push(v);
                } else {
                    *e = vec![v];
                }
            }
        }
    }
    if pos.len() > spec.positionnels.len() && !spec.positionnels.iter().any(|p| p.multiple) {
        let en_trop: Vec<&str> = pos[spec.positionnels.len()..].iter().map(|s| s.as_str()).collect();
        return Err(Refus(format!("unrecognized arguments: {}", en_trop.join(" "))));
    }
    // La distribution d'`argparse` : chaque positionnel prend un argument, sauf un facultatif quand il n'en reste que
    // pour les obligatoires qui le suivent (`fin-contrat 2026-10` : `quoi` garde son defaut, `mois` prend l'argument).
    let mut suivant = 0usize;
    let mut manquants: Vec<&str> = Vec::new();
    for (k, p) in spec.positionnels.iter().enumerate() {
        let restants = pos.len() - suivant;
        let obligatoires_apres = spec.positionnels[k + 1..].iter().filter(|q| q.obligatoire).count();
        if p.multiple {
            let n = restants.saturating_sub(obligatoires_apres);
            r.valeurs.insert(p.nom.to_string(), pos[suivant..suivant + n].iter().map(|s| (*s).clone()).collect());
            suivant += n;
            continue;
        }
        let prend = if p.obligatoire { restants >= 1 } else { restants > obligatoires_apres };
        if prend {
            let v = pos[suivant];
            suivant += 1;
            if let Some(c) = p.choix {
                if !c.contains(&v.as_str()) {
                    let liste: Vec<String> = c.iter().map(|x| format!("'{}'", x)).collect();
                    return Err(Refus(format!("argument {}: invalid choice: '{}' (choose from {})", p.nom, v, liste.join(", "))));
                }
            }
            r.valeurs.insert(p.nom.to_string(), vec![v.clone()]);
        } else if p.obligatoire {
            manquants.push(p.nom);
        } else if let Some(d) = p.defaut {
            r.valeurs.insert(p.nom.to_string(), vec![d.to_string()]);
        }
    }
    if !manquants.is_empty() {
        return Err(Refus(format!("the following arguments are required: {}", manquants.join(", "))));
    }
    Ok(r)
}

/// Analyse `args` ; sur un refus, ecrit `usage` puis la phrase sur la sortie d'erreur et rend le code 2.
pub fn analyser_ou_sortir(spec: &Spec, args: &[String]) -> Result<Args, i32> {
    analyser(spec, args).map_err(|Refus(m)| {
        eprintln!("{}\npaie {}: error: {}", usage(spec), spec.commande, m);
        2
    })
}

//! La fiche recapitulative du dossier d'un salarie : les deux etats de l'onglet Exports reduits a lui -- les heures et
//! les cachets, le recapitulatif annuel (`etats_paie`) -- en un PDF ; et ce qu'un spectacle ou un salarie a coute dans
//! l'annee.
//!
//! Elle lit ce que les etats lisent, et rien d'autre : les paies ecrites des engagements valides et celles des
//! engagements importes. Le recapitulatif annuel et le cout de l'annee portent les paies importees avec ce que les
//! pieces de l'ancien logiciel en disaient ; les heures et les cachets ne portent que les paies calculees ici, comme
//! l'etat. Un etat qui n'a rien sur la periode (`ErreurOperation`) est LAISSE DE COTE ; tout autre refus remonte.

use std::path::{Path, PathBuf};
use std::rc::Rc;

use serde_json::{json, Map, Value};

use crate::etats_paie::{self, Demande, Periode};
use crate::tableaux_export::{pdf_tableau, qui, totaliser, Section, Tableau, R, TOTAL_GENERAL};
use droit::contexte::Contexte;
use socle::echec::{erreur, Echec};
use socle::num::Decimal;

/// Un refus qui dit « rien sur la periode » : l'etat est laisse de cote.
fn est_erreur_operation(e: &Echec) -> bool {
    matches!(e.classe, None | Some("ErreurOperation") | Some("SaisieIncomplete"))
}

fn copie(e: &Echec) -> Echec {
    Echec { classe: e.classe, message: e.message.clone() }
}

/// Les deux etats de la periode, lus une fois chacun pour tous les salaries du dossier.
pub struct Recapitulatifs<'a> {
    ctx: &'a Contexte,
    demande: Demande,
    etats: [Option<Result<Rc<Tableau>, Echec>>; 2],
}

impl<'a> Recapitulatifs<'a> {
    pub fn nouveau(ctx: &'a Contexte, mois: Option<String>, annee: Option<String>, toutes: bool) -> Self {
        let demande = Demande { mois, annee, toutes, format: "pdf".to_string(), ..Demande::default() };
        Recapitulatifs { ctx, demande, etats: [None, None] }
    }

    /// Ce qu'un spectacle, ou un salarie, a coute dans l'annee demandee, jusqu'au mois de `ctx.aujourdhui` ; ce qui
    /// est seulement saisi pour plus tard se dit a part (`prevu`). Une paie importee dont les pieces ne disaient pas le
    /// cout employeur rend le cumul `incomplet`. Sans annee, ou sans mois cette annee-la : rien.
    pub fn couts_de_l_annee(&mut self) -> R<Value> {
        let annee = self.demande.annee.clone().unwrap_or_default();
        let courant: String = self.ctx.aujourdhui.iso().chars().take(7).collect();
        let mut out = Map::new();
        out.insert("annee".into(), json!(annee));
        out.insert("jusqu_a".into(), json!(courant));
        out.insert("spectacles".into(), json!({}));
        out.insert("salaries".into(), json!({}));
        if annee.is_empty() {
            return Ok(Value::Object(out));
        }
        let p = Periode::nouvelle(self.ctx, None, Some(&annee), false, false)?;
        if p.mois.is_empty() {
            return Ok(Value::Object(out));
        }
        struct Cumul {
            bulletins: i64,
            brut: Decimal,
            charges: Decimal,
            cout: Decimal,
            prevu: Decimal,
            incomplet: bool,
        }
        let zero = socle::num::zero2();
        let mut cotes: [Vec<(String, Cumul)>; 2] = [Vec::new(), Vec::new()];
        for l in etats_paie::lignes_livre(self.ctx, &p, &mut Vec::new())? {
            for (i, cle) in [&l.spectacle, &l.cle_salarie].into_iter().enumerate() {
                if cle.is_empty() {
                    continue;
                }
                let pos = match cotes[i].iter().position(|(k, _)| k == cle) {
                    Some(n) => n,
                    None => {
                        cotes[i].push((
                            cle.clone(),
                            Cumul { bulletins: 0, brut: zero, charges: zero, cout: zero, prevu: zero, incomplet: false },
                        ));
                        cotes[i].len() - 1
                    }
                };
                let d = &mut cotes[i][pos].1;
                if l.mois > courant {
                    d.prevu += l.cout_employeur.unwrap_or(zero);
                    continue;
                }
                d.bulletins += 1;
                d.brut += l.brut.unwrap_or(zero);
                d.charges += l.cotisations_patronales.unwrap_or(zero);
                if l.cout_employeur.is_none() {
                    d.incomplet = true;
                }
                d.cout += l.cout_employeur.unwrap_or(zero);
            }
        }
        for (i, nom) in ["spectacles", "salaries"].into_iter().enumerate() {
            let mut m = Map::new();
            for (cle, d) in std::mem::take(&mut cotes[i]) {
                m.insert(
                    cle,
                    json!({
                        "bulletins": d.bulletins,
                        "brut": d.brut.to_string(),
                        "charges": d.charges.to_string(),
                        "cout": d.cout.to_string(),
                        "prevu": d.prevu.to_string(),
                        "incomplet": d.incomplet,
                    }),
                );
            }
            out.insert(nom.into(), Value::Object(m));
        }
        Ok(Value::Object(out))
    }

    /// L'etat `i` de la periode : 0, les heures et les cachets ; 1, le recapitulatif annuel.
    fn etat(&mut self, i: usize) -> R<Rc<Tableau>> {
        if self.etats[i].is_none() {
            let r = if i == 0 {
                etats_paie::heures_et_cachets(self.ctx, &self.demande)
            } else {
                etats_paie::recapitulatif_annuel(self.ctx, &self.demande)
            };
            self.etats[i] = Some(r.map(Rc::new));
        }
        match self.etats[i].as_ref().expect("lu") {
            Ok(t) => Ok(Rc::clone(t)),
            Err(e) => Err(copie(e)),
        }
    }

    /// Les deux etats reduits au salarie `cle` (les lignes qui portent sa cle : deux salaries de meme nom et de meme
    /// prenom ont chacun la leur), en un PDF dans `tmp`. `None` quand aucun des deux n'a rien a dire de
    /// lui. Les avertissements des etats nomment d'autres salaries : la fiche ne les porte pas.
    pub fn pdf(&mut self, cle: &str, tmp: &Path) -> R<Option<PathBuf>> {
        let fiche = self.ctx.salaries.get(cle).ok_or_else(|| erreur("KeyError", socle::fichiers::absent(cle)))?;
        let lui = qui(fiche)?;
        let mut morceaux: Vec<Vec<u8>> = Vec::new();
        for i in 0..2 {
            let t = match self.etat(i) {
                Ok(t) => t,
                Err(e) if est_erreur_operation(&e) => continue,
                Err(e) => return Err(e),
            };
            let sections: Vec<Section> = t
                .sections
                .iter()
                .map(|s| Section::nouvelle(s.libelle.clone(), s.lignes.iter().filter(|l| l.texte("cle_salarie") == cle).cloned().collect()))
                .filter(|s| !s.lignes.is_empty())
                .collect();
            if sections.is_empty() {
                continue;
            }
            let mut reduit = Tableau::nouveau(&t.etat, format!("{} — {}", t.titre, lui), t.periode.clone());
            reduit.colonnes = t.colonnes.clone();
            reduit.sections = sections;
            reduit.sources = t.sources.clone();
            reduit.mois = t.mois.clone();
            totaliser(&mut reduit, TOTAL_GENERAL)?;
            morceaux.push(pdf_tableau(&reduit, Some(self.ctx))?);
        }
        if morceaux.is_empty() {
            return Ok(None);
        }
        let chemin = tmp.join(format!("R_{}.pdf", cle));
        let octets = crate::fabrique::fusion_octets(&morceaux, tmp).map_err(Echec::from)?;
        std::fs::write(&chemin, octets).map_err(|e| Echec::from(format!("ecriture impossible de {} : {}", chemin.display(), e)))?;
        Ok(Some(chemin))
    }
}

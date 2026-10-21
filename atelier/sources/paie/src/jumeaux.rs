//! LES JUMEAUX : deux contrats du meme salarie qui commencent le meme jour (deux CDDU sur deux spectacles, un
//! cachet et une masterclass...). Ils ne se confondent pas sur (salarie, debut).
//!
//! L'IDENTITE D'UN ENGAGEMENT DANS UN FICHIER DE MOIS (decide le 09/10/2026) : (salarie, debut, numero) quand le
//! numero est pose ; sans numero (premier mois, avant production), son RANG parmi les engagements de meme
//! (salarie, debut) du fichier. C'est ici, et seulement ici, que cette regle s'ecrit :
//! - `cles` : la cle unique de chaque engagement d'UNE liste ;
//! - `apparier` : d'une liste a l'autre (la saisie de l'ecran et le fichier, un mois et le suivant), quel engagement
//!   est le meme contrat.
//!
//! Un numero de contrat se LIT, il ne se recalcule pas (regle 4) : deux numeros poses et differents sont deux
//! contrats, jamais le meme.

use serde_json::Value;

use crate::etats::py_str;
use crate::fichiers::vrai;

/// Un engagement tel qu'il s'identifie : (cle du salarie, premier jour ISO, numero de contrat s'il est pose).
pub(crate) type Ident = (String, String, Option<String>);

/// Le numero de contrat POSE sur un engagement brut (`"numero"` non vide), et lui seul.
pub(crate) fn numero_pose(e: &Value) -> Option<String> {
    numero_du_champ(e.get("numero"))
}

/// Le meme, depuis le champ `"numero"` lui-meme (un engagement deja deplie en dictionnaire).
pub(crate) fn numero_du_champ(n: Option<&Value>) -> Option<String> {
    n.filter(|n| vrai(n)).map(py_str)
}

/// La cle unique de chaque engagement d'une liste : `(salarie, debut, numero)` quand le numero est pose, sinon
/// `(salarie, debut, "#rang")`, le rang (0, 1...) etant celui de l'engagement parmi ceux de meme (salarie, debut).
pub(crate) fn cles(ids: &[Ident]) -> Vec<(String, String, String)> {
    ids.iter()
        .enumerate()
        .map(|(i, (s, d, n))| {
            let marque = match n {
                Some(n) => n.clone(),
                None => format!("#{}", ids[..i].iter().filter(|(s2, d2, _)| s2 == s && d2 == d).count()),
            };
            (s.clone(), d.clone(), marque)
        })
        .collect()
}

/// Pour chaque engagement de `a`, l'indice dans `b` du MEME contrat, ou `None`. Un engagement qui ne s'identifie
/// pas (`None` dans la liste) ne s'apparie a rien.
///
/// Parmi les engagements de meme (salarie, debut) :
/// 1. un numero pose retrouve le meme numero pose ;
/// 2. les restants se suivent dans l'ordre de leur liste (le rang), un a un -- c'est ce qui relie un engagement
///    encore sans numero a celui qui en a recu un depuis (le fichier fige apres production, le contrat parti a la
///    signature) ; deux numeros poses et differents ne s'apparient jamais.
pub(crate) fn apparier(a: &[Option<Ident>], b: &[Option<Ident>]) -> Vec<Option<usize>> {
    let mut sortie: Vec<Option<usize>> = vec![None; a.len()];
    let mut pris = vec![false; b.len()];
    let meme_jour = |x: &Ident, y: &Ident| x.0 == y.0 && x.1 == y.1;
    // 1. le numero
    for (i, x) in a.iter().enumerate() {
        let Some(x) = x else { continue };
        let Some(n) = &x.2 else { continue };
        if let Some(j) = (0..b.len()).find(|&j| !pris[j] && b[j].as_ref().is_some_and(|y| meme_jour(x, y) && y.2.as_ref() == Some(n))) {
            sortie[i] = Some(j);
            pris[j] = true;
        }
    }
    // 2. le rang, parmi ceux qui restent
    for (i, x) in a.iter().enumerate() {
        let Some(x) = x else { continue };
        if sortie[i].is_some() {
            continue;
        }
        if let Some(j) = (0..b.len()).find(|&j| !pris[j] && b[j].as_ref().is_some_and(|y| meme_jour(x, y) && (x.2.is_none() || y.2.is_none()))) {
            sortie[i] = Some(j);
            pris[j] = true;
        }
    }
    sortie
}

/// `apparier`, puis LE MEME CONTRAT A D'AUTRES DATES : un engagement reste sans jumeau retrouve celui du meme salarie
/// qui porte le meme numero de contrat pose, quel que soit son premier jour (21/10/2026). Un numero de contrat se lit et
/// ne se recalcule pas (regle 4) : deux engagements du meme salarie au meme numero sont un seul contrat, dont la date de
/// debut a change -- ce que la DSN declare par l'ancienne date dans un bloc « Changements contrat » (S21.G00.41.021,
/// fiche net-entreprises 1952, `atelier/reference/sources/net-entreprises-base-connaissances/`). Seulement pour comparer
/// une saisie declaree a sa modification (`routes_regularisation`) : ailleurs, l'identite reste (salarie, debut, numero).
pub(crate) fn apparier_meme_numero(a: &[Option<Ident>], b: &[Option<Ident>]) -> Vec<Option<usize>> {
    let mut sortie = apparier(a, b);
    let mut pris = vec![false; b.len()];
    for j in sortie.iter().flatten() {
        pris[*j] = true;
    }
    for (i, x) in a.iter().enumerate() {
        let (Some(x), None) = (x, sortie[i]) else { continue };
        let Some(n) = &x.2 else { continue };
        if let Some(j) = (0..b.len()).find(|&j| !pris[j] && b[j].as_ref().is_some_and(|y| y.0 == x.0 && y.2.as_ref() == Some(n))) {
            sortie[i] = Some(j);
            pris[j] = true;
        }
    }
    sortie
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(s: &str, d: &str, n: Option<&str>) -> Option<Ident> {
        Some((s.into(), d.into(), n.map(String::from)))
    }

    #[test]
    fn deux_jumeaux_sans_numero_se_distinguent_par_le_rang() {
        let ids: Vec<Ident> = [id("A", "2026-10-06", None), id("A", "2026-10-06", None), id("B", "2026-10-06", None)].into_iter().flatten().collect();
        let c = cles(&ids);
        assert_eq!(c[0].2, "#0");
        assert_eq!(c[1].2, "#1");
        assert_eq!(c[2].2, "#0");
    }

    #[test]
    fn le_numero_d_abord_puis_le_rang_et_jamais_deux_numeros_differents() {
        // l'ecran sans numeros, le fichier fige : chacun retrouve le sien, dans l'ordre
        let ecran = [id("A", "d", None), id("A", "d", None)];
        let fichier = [id("A", "d", Some("N1")), id("A", "d", Some("N2"))];
        assert_eq!(apparier(&ecran, &fichier), vec![Some(0), Some(1)]);
        // reordonnes, mais numerotes : le numero gagne sur l'ordre
        let ecran = [id("A", "d", Some("N2")), id("A", "d", None)];
        assert_eq!(apparier(&ecran, &fichier), vec![Some(1), Some(0)]);
        // deux numeros differents ne sont pas le meme contrat
        assert_eq!(apparier(&[id("A", "d", Some("N3"))], &fichier), vec![None]);
        // un autre jour, un autre salarie : rien
        assert_eq!(apparier(&[id("A", "e", None), id("B", "d", None)], &fichier), vec![None, None]);
    }

    #[test]
    fn le_meme_numero_retrouve_le_contrat_dont_la_date_de_debut_a_change() {
        let declare = [id("A", "d", Some("N1")), id("A", "d", Some("N2"))];
        let saisie = [id("A", "c", Some("N2")), id("A", "d", Some("N1"))];
        assert_eq!(apparier(&saisie, &declare), vec![None, Some(0)]);
        assert_eq!(apparier_meme_numero(&saisie, &declare), vec![Some(1), Some(0)]);
        // sans numero, ou chez un autre salarie : rien
        assert_eq!(apparier_meme_numero(&[id("A", "c", None), id("B", "c", Some("N2"))], &declare), vec![None, None]);
    }
}

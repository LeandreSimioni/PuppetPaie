//! L'heure locale du poste : Europe/Paris, sans lire le fuseau du systeme.
//!
//! Le Python pose `TZ=Europe/Paris` (`moteur/__init__.py`), donc
//! `datetime.fromtimestamp(mtime)` rend l'heure de Paris. Le moteur n'a pas de
//! fuseau : il applique la regle de l'heure d'ete de l'Union europeenne
//! (directive 2000/84/CE, art. 2 et 3, en vigueur a compter de 2002 ;
//! `atelier/reference/sources/union-europeenne/eurlex-directive-2000-84-ce-heure-d-ete-20010202.txt`) :
//! UTC+2 du dernier dimanche de mars (1 h UTC) au dernier dimanche d'octobre
//! (1 h UTC), UTC+1 le reste de l'annee. Avant 2002, d'autres directives
//! reglaient la periode ; aucune date de fichier du logiciel n'y remonte.

use crate::date::Jour;

/// Le jour unix (nombre de jours depuis le 01/01/1970) du dernier dimanche du
/// mois `mois` de l'annee `an`.
fn dernier_dimanche(an: i32, mois: u32) -> i64 {
    let dernier = Jour::nouveau(an, mois, crate::date::jours_du_mois(an, mois)).expect("fin de mois");
    let n = dernier.ordinal() - 719_163;
    // Le 01/01/1970 etait un jeudi : (n + 4) % 7 vaut 0 pour un dimanche.
    n - (n + 4).rem_euclid(7)
}

/// Le decalage de Paris sur UTC, en secondes, a cet instant (secondes unix).
pub fn decalage(t: i64) -> i64 {
    let jours = t.div_euclid(86_400);
    let an = Jour::depuis_jours_unix(jours).map_or(1970, |j| j.an);
    let debut = dernier_dimanche(an, 3) * 86_400 + 3_600;
    let fin = dernier_dimanche(an, 10) * 86_400 + 3_600;
    if t >= debut && t < fin {
        7_200
    } else {
        3_600
    }
}

/// `datetime.fromtimestamp(t).isoformat(timespec="seconds")` a l'heure de
/// Paris : `AAAA-MM-JJTHH:MM:SS`.
pub fn iso_secondes(t: f64) -> String {
    let (local, _) = local_de(t);
    let jours = local.div_euclid(86_400);
    let reste = local.rem_euclid(86_400);
    let j = Jour::depuis_jours_unix(jours).unwrap_or(Jour { an: 1970, mois: 1, jour: 1 });
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}", j.an, j.mois, j.jour, reste / 3_600, (reste % 3_600) / 60, reste % 60)
}

/// Les secondes locales de Paris d'un instant, apres l'arrondi a la
/// microseconde que Python fait (au pair le plus proche).
fn local_de(t: f64) -> (i64, i64) {
    let mut entier = t.trunc();
    let mut us = ((t - entier) * 1e6).round_ties_even();
    if us >= 1e6 {
        entier += 1.0;
        us -= 1e6;
    } else if us < 0.0 {
        entier -= 1.0;
        us += 1e6;
    }
    let _ = us;
    let utc = entier as i64;
    (utc + decalage(utc), utc)
}

/// `datetime.fromtimestamp(t).isoformat(timespec="minutes")` a l'heure de
/// Paris : `AAAA-MM-JJTHH:MM`. Python arrondit la fraction de seconde a la
/// microseconde (au pair le plus proche) avant de tronquer aux minutes.
pub fn iso_minutes(t: f64) -> String {
    let mut entier = t.trunc();
    let mut us = ((t - entier) * 1e6).round_ties_even();
    if us >= 1e6 {
        entier += 1.0;
        us -= 1e6;
    } else if us < 0.0 {
        entier -= 1.0;
        us += 1e6;
    }
    let _ = us;
    let utc = entier as i64;
    let local = utc + decalage(utc);
    let jours = local.div_euclid(86_400);
    let reste = local.rem_euclid(86_400);
    let j = Jour::depuis_jours_unix(jours).unwrap_or(Jour { an: 1970, mois: 1, jour: 1 });
    format!("{:04}-{:02}-{:02}T{:02}:{:02}", j.an, j.mois, j.jour, reste / 3_600, (reste % 3_600) / 60)
}

/// `st_mtime` d'un fichier, comme Python le lit : les secondes et la fraction
/// en nanosecondes, additionnees en flottant.
/// Hors Unix (Windows), la meme valeur par `modified()`.
#[cfg(unix)]
pub fn mtime(m: &std::fs::Metadata) -> f64 {
    use std::os::unix::fs::MetadataExt;
    m.mtime() as f64 + m.mtime_nsec() as f64 * 1e-9
}

#[cfg(not(unix))]
pub fn mtime(m: &std::fs::Metadata) -> f64 {
    let d = m.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).unwrap_or_default();
    d.as_secs() as f64 + d.subsec_nanos() as f64 * 1e-9
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l_heure_d_ete_commence_le_dernier_dimanche_de_mars_a_1_h_utc() {
        // 2026 : dimanche 29 mars. 00:59:59 UTC encore l'hiver, 01:00:00 l'ete.
        let t = Jour::nouveau(2026, 3, 29).unwrap().ordinal() - 719_163;
        assert_eq!(decalage(t * 86_400 + 3_599), 3_600);
        assert_eq!(decalage(t * 86_400 + 3_600), 7_200);
        assert_eq!(iso_minutes((t * 86_400 + 3_599) as f64), "2026-03-29T01:59");
        assert_eq!(iso_minutes((t * 86_400 + 3_600) as f64), "2026-03-29T03:00");
    }

    #[test]
    fn l_heure_d_ete_finit_le_dernier_dimanche_d_octobre_a_1_h_utc() {
        // 2026 : dimanche 25 octobre.
        let t = Jour::nouveau(2026, 10, 25).unwrap().ordinal() - 719_163;
        assert_eq!(decalage(t * 86_400 + 3_599), 7_200);
        assert_eq!(decalage(t * 86_400 + 3_600), 3_600);
        assert_eq!(iso_minutes((t * 86_400 + 3_599) as f64), "2026-10-25T02:59");
        assert_eq!(iso_minutes((t * 86_400 + 3_600) as f64), "2026-10-25T02:00");
    }

}

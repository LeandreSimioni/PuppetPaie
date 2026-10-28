//! Le courriel de renvoi d'un contrat modifie (`envoi::ce_qui_a_change`) : ce qui a change, contrat par contrat.

use serde_json::{json, Value};

#[test]
fn le_courriel_de_renvoi_dit_ce_qui_a_change_contrat_par_contrat() {
    use envois::envoi::ce_qui_a_change;
    let contrat = |debut: &str, jours: &[&str], cachets: i64, heures: &str, brut: &str, net: &str| {
        json!({"debut": debut, "fin": jours.last(), "spectacle": "Le Spectacle", "jours": jours, "cachets": cachets, "heures": heures, "brut": brut, "net": net})
    };
    // Un seul contrat : une ligne par fait qui a change.
    let a = json!({"contrats": [contrat("2026-10-05", &["2026-10-05", "2026-10-06"], 1, "15", "300,00", "230,00")], "brut": "300,00", "net": "230,00"});
    let n = json!({"contrats": [contrat("2026-10-05", &["2026-10-05", "2026-10-06"], 1, "17,5", "340,00", "260,00")], "brut": "340,00", "net": "260,00"});
    assert_eq!(
        ce_qui_a_change(&a, &n),
        ["  - heures : 15 → 17,5", "  - salaire brut : 300,00 € → 340,00 €", "  - net à payer : 230,00 € → 260,00 €"]
    );
    // Rien de ce que le resume porte n'a bouge : rien a dire (le courriel le dit autrement).
    assert!(ce_qui_a_change(&a, &a).is_empty());
    // Deux contrats : celui qui a change, nomme, puis les totaux.
    let deux = |x: &Value, y: &Value, brut: &str| json!({"contrats": [x, y], "brut": brut, "net": "0,00"});
    let premier = contrat("2026-10-05", &["2026-10-05"], 1, "", "200,00", "0,00");
    let second = contrat("2026-10-20", &["2026-10-20"], 1, "", "200,00", "0,00");
    let second_change = contrat("2026-10-20", &["2026-10-20", "2026-10-21"], 2, "", "400,00", "0,00");
    assert_eq!(
        ce_qui_a_change(&deux(&premier, &second, "400,00"), &deux(&premier, &second_change, "600,00")),
        [
            "  - contrat du 20/10/2026 au 21/10/2026 (Le Spectacle) :",
            "      contrat du 20/10/2026 (Le Spectacle) → contrat du 20/10/2026 au 21/10/2026 (Le Spectacle)",
            "      jours travaillés : 20/10 → 20/10 et 21/10",
            "      cachets : 1 → 2",
            "      salaire brut : 200,00 € → 400,00 €",
            "  - total brut du mois : 400,00 € → 600,00 €",
        ]
    );
    // Un contrat de plus : avant, maintenant.
    let c = ce_qui_a_change(&json!({"contrats": [premier.clone()], "brut": "200,00", "net": "0,00"}), &deux(&premier, &second, "400,00"));
    assert_eq!(c[0], "  - avant : contrat du 05/10/2026 (Le Spectacle), 1 cachet, 200,00 € brut, 0,00 € net à payer");
    assert_eq!(c.len(), 4, "{:?}", c);
}

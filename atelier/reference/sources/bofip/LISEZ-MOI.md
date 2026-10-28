# BOFiP — ce que la paie lit de la doctrine fiscale

Chaque page est déposée **brute** (`.html`, telle que servie, nom = identifiant BOFiP + date de version) ; quand
seuls quelques paragraphes servent, un `-extrait-*.md` à côté les transcrit, dit la date du relevé et où le moteur
s'en sert. Le BOFiP se laisse lire par un robot (contrairement à Légifrance) : une page se redépose à chaque version.

| fichier | ce que c'est | où ça sert |
|---|---|---|
| `BOI-BAREME-000037-*.html` | barème kilométrique | défraiements |
| `BOI-RSA-BASE-30-10-20-20260810.html`, `-extrait-prevoyance.md` | charges déductibles du salaire : prévoyance complémentaire (§ 200-240) | net imposable : part patronale santé réintégrée, prévoyance non ; Fonds santé des intermittents |
| `BOI-IR-PAS-30-10-30-20-20190329.html`, `BOI-IR-PAS-30-10-30-20-extrait-reversement.md` | prélèvement à la source : la date du reversement (§ 50-60 : le 15 ou le 5 du mois suivant ; option trimestrielle sous onze salariés) | boîte de dépôt de la DSN, date du versement à la DGFiP (`paie/src/ordres_paiement.rs`, ticket #223) |
| `BOI-IR-PAS-30-10-50-20190227.html`, `BOI-IR-PAS-30-10-50-extrait-regularisation.md` | prélèvement à la source : un rappel de paie se prélève au mois de son versement (§ 1) ; un trop-versé non compensé se régularise au taux initialement utilisé (§ 90) | le prélèvement du versement d'une correction (`engagement/src/etats.rs`, `prelevement_de_la_correction`), lu par la DSN et le bulletin rectificatif |
| `BOI-TPS-TS-30-20200624.html`, `BOI-TPS-PEEC-10-20141218.html`, `BOI-TPS-PEEC-20-20141218.html`, `BOI-TPS-TS-30-et-PEEC-extrait-bloc-44.md` | taxe sur les salaires : liquidation, taux majorés et part assujettie (§ 40, 110, 240) ; PEEC : qui la doit (§ 40-50), sa base (§ 1-20) | DSN de décembre, bloc S21.G00.44 (`paie/src/assujettissement.rs`, lot V3b) |

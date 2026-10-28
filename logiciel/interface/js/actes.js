/* Ce qui part : actions, DSN, DPAE, depot, repetition, envoi, signature, avenant. */
"use strict";

/* =======================================================================
   LE MOIS : CE QUI PART D'ICI
   =======================================================================
   Quatre actes, un seul écran pour les quatre : déposer la DSN, déposer la
   DPAE, envoyer les documents aux salariés, demander la signature des
   contrats. Le moteur sait les faire depuis le 11/09/2026 ; ils n'avaient
   pas d'écran, et un bouton qui n'existe pas est une fonction qui n'existe
   pas.

   TROIS RÈGLES, ET AUCUNE N'EST TENUE ICI — c'est tout le sujet :

   1. La phrase d'état vient du JOURNAL, par le moteur (`journal.resume`).
      Elle distingue trois choses qu'un écran confondrait vite : jamais
      parti, parti sans réponse, parti et répondu. « En attente » affiché
      comme « conforme », c'est une déclaration qu'on croit faite.
   2. Rien ne part d'un clic. Chaque bouton ouvre un RÉCAPITULATIF, et le
      récapitulatif demande le mot de passe DU LOGICIEL — pas celui de
      l'organisme, que cette page ne voit jamais, ne demande jamais et ne
      reçoit jamais. Le moteur va le chercher lui-même au coffre.
   3. Le mois déclaré y est écrit EN GROS. Ce n'est pas de la décoration :
      recopier le mois était le seul geste qu'on ne peut pas faire sans
      regarder l'écran, et il a été retiré du parcours. Un mot de passe se
      tape de mémoire, les yeux ailleurs — donc c'est ce bloc-là qui porte
      désormais toute la lecture (docstring d'`operations.Preuve`).

   La séance se voit AVANT le clic, et pourtant les boutons restent actifs.
   Un garde-fou posé dans un front est un garde-fou qu'on contourne par
   l'autre face : c'est `operations` qui refuse, et lui seul. La page
   prévient, parce que découvrir un refus après avoir tapé son mot de passe
   est une mauvaise façon d'apprendre qu'il fallait ouvrir la séance.
*/
let MOIS_ACTES = null;          // le dernier /api/etat-du-mois lu

/* LE FIL DES ENVOIS N'EST PLUS ICI (20/10/2026, tickets #219, #220) : tous les envois du mois et tous leurs retours, lus,
 * sont dans l'Historique (js/historique.js, route `historique-mois`). L'étape garde son état et son bouton ; un lien
 * mène au mois dans l'Historique. */
/* LES ÉTAPES DU MOIS (#257) : le moteur rend, étape par étape, les groupes à dessiner (`etat-du-mois`, `etapes`) —
 * chacun son état (fait, a_faire, en_attente, refuse), son compte, sa phrase courte, son échéance et le geste qu'ouvre
 * un clic — et laquelle est la suivante. La page ne décide rien : elle dessine ce que disent les groupes. */

/** Le geste d'un groupe, tel que le moteur le nomme. */
function ouvrirGeste(action, mois) {
  if (!action) return;
  const m = action.mois || mois;
  switch (action.geste) {
    case "declarer_embauches": return boiteDpae(m);
    case "faire_signer": return boiteSignature(m);
    case "contrats_en_attente": return boiteContratsEnAttente(m);
    case "deposer_dsn": return deposerLaDsn(m);
    case "deposer_fichier": return boiteDepot(action.quoi, action.fichier);
    case "envoyer_documents": return boiteEnvoi(m);
    case "historique": return ouvrirMoisHistorique(m);
  }
}

/** La classe d'un état, pour la couleur que la feuille de style donne déjà. */
const COULEUR_ETAT = { fait: "bien", a_faire: "alerte", en_attente: "alerte", refuse: "refus" };

/* -------------------------------------------- la colonne des quatre étapes */
async function dessinerActions(mois) {
  const cible = document.getElementById("actions-mois");
  if (!mois) { cible.innerHTML = ""; return; }
  cible.innerHTML = `<p class="discret">Lecture du journal…</p>`;
  let e;
  try {
    e = await api("etat-du-mois", { mois });
  } catch (err) {
    cible.innerHTML = `<p class="discret">${echapper(err.message)}</p>`;
    return;
  }
  MOIS_ACTES = e;
  const gestes = [];
  cible.innerHTML = `<ol class="etapes">` + (e.etapes || []).map((et, i) => {
    const groupes = et.groupes || [];
    const ouverts = groupes.filter((g) => g.etat !== "fait");
    const fini = ouverts.length === 0;
    const pire = ouverts.find((g) => g.etat === "refuse") || ouverts[0];
    const couleur = pire ? COULEUR_ETAT[pire.etat] : "bien";
    const courte = ouverts.map((g) => g.phrase).filter(Boolean).join(" · ");
    const echeance = ouverts.map((g) => g.echeance && g.echeance.texte).find(Boolean);
    const nEnvois = (e.lignes || []).filter((x) => x.quoi === et.cle).length;
    const boutons = ouverts.filter((g) => g.action).map((g) => {
      gestes.push(g.action);
      return `<button class="${et.suivante ? "primaire" : "secondaire"}" data-geste="${gestes.length - 1}">${echapper(g.action.libelle)}</button>`;
    });
    if (et.second) {
      gestes.push(et.second);
      boutons.push(`<button class="secondaire" data-geste="${gestes.length - 1}">${echapper(et.second.libelle)}</button>`);
    }
    return `
      <li class="etape ${echapper(couleur)} ${fini ? "faite" : ""} ${et.suivante ? "suivante" : ""}">
        <details ${et.suivante ? "open" : ""}>
          <summary>
            <span class="etape-rond">${fini ? "✓" : i + 1}</span>
            <span class="etape-nom">${echapper(et.nom)}</span>
            <span class="etape-court">${echapper(courte)}</span>${echeance
              ? ` <span class="etape-limite"><b>${echapper(echeance)}</b></span>` : ""}
          </summary>
          <div class="etape-corps">
            ${et.phrase ? `<div class="etape-phrase">${echapper(et.phrase)}</div>` : ""}
            ${et.detail ? `<div class="discret etape-detail">${echapper(et.detail)}</div>` : ""}
            ${boutons.join("")}
            ${nEnvois ? `<button class="lien etape-envois" data-historique="${echapper(mois)}">voir ${nEnvois > 1
              ? `les ${nEnvois} envois` : "l'envoi"} et leurs retours dans l'Historique</button>` : ""}
          </div>
        </details>
      </li>`;
  }).join("") + `</ol>`;

  cible.querySelectorAll("[data-geste]").forEach((b) => {
    b.onclick = () => ouvrirGeste(gestes[Number(b.dataset.geste)], mois);
  });
  cible.querySelectorAll("[data-historique]").forEach((b) => {
    b.onclick = () => ouvrirMoisHistorique(b.dataset.historique);
  });
}

/* Ce qu'il faut relire après avoir déposé ou envoyé : l'état du mois affiché,
 * et la liste des embauches, qu'un dépôt de DPAE vient de raccourcir. */
async function rafraichirActions() {
  const mois = document.getElementById("mois").value;
  await dessinerActions(mois);
  await dessinerEmbauches(mois);
  await dessinerEtatsDesPaies(mois);
}

/* ---------------------------------------- le récapitulatif d'un dépôt */

/** CE QUE LA DSN DÉCLARE, contrat par contrat, LU DANS LE FICHIER
 *  (30/09/2026, l'éditeur : « ce que l'utilisateur veut voir et vérifier,
 *  c'est que tous les contrats sont dans la DSN »). Comparé aux contrats
 *  saisis du mois : un contrat absent du fichier, ou que la paie dit
 *  autrement, se dit en tête. */
function recapitulatifDsn(r) {
  if (!r) return "";
  if (r.erreur) return `<div class="encart attention"><b>Récapitulatif illisible</b>
    ${echapper(r.erreur)}</div>`;
  const ecarts = r.salaries.reduce((n, s) => n + s.contrats.filter((c) => c.ecarts.length).length, 0);
  // Un nombre du fichier, à la française ; une correction le signe d'un vrai
  // moins (« −184 h » : ce qui s'ajoute à ce qui a déjà été déclaré).
  const signe = (x) => String(x).replace(/^-/, "−");
  const nombre = (x) => signe(Number(x).toLocaleString("fr-FR", { maximumFractionDigits: 2 }));
  const vide = (x) => !x || Number(x) === 0;
  // L'activité en une colonne : « 4 cachets · 4 j », « 151,67 h ».
  const activite = (c) => [
    vide(c.cachets) ? "" : `${nombre(c.cachets)} cachet${Math.abs(Number(c.cachets)) > 1 ? "s" : ""}`,
    vide(c.heures) ? "" : `${nombre(c.heures)} h`,
    vide(c.jours) ? "" : `${nombre(c.jours)} j`,
  ].filter(Boolean).join(" · ") || "—";
  // Les dates sur une ligne : « 06/10 → 09/10/2026 » dans une même année.
  const dates = (c) => {
    if (!c.fin || c.fin === c.debut) return jjmmaaaa(c.debut);
    const d = jjmmaaaa(c.debut), f = jjmmaaaa(c.fin);
    return `${c.debut.slice(0, 4) === c.fin.slice(0, 4) ? d.slice(0, 5) : d} → ${f}`;
  };
  const lignes = r.salaries.map((s) => s.contrats.map((c, i) => `
    <tr class="${c.ecarts.length ? "ecart" : ""}${c.correction ? " correction" : ""}">
      <td>${i === 0 ? `<b>${echapper(s.nom)}</b> ${echapper(s.prenoms)}` : ""}</td>
      <td>${echapper(c.numero)}</td>
      <td class="dates">${echapper(dates(c))}</td>
      <td>${echapper(c.emploi)}</td>
      <td class="num">${c.correction ? "correction : " : ""}${echapper(activite(c))}</td>
      <td class="num">${c.remunere ? echapper(signe(euros(c.brut))) : "—"}</td>
      <td class="fin">${c.correction ? "mois déjà déclaré"
        : c.fin_declaree ? "fin le " + echapper(jjmmaaaa(c.fin_declaree)) : "en cours"}</td>
    </tr>
    ${c.ecarts.length ? `<tr class="ecart"><td></td><td colspan="6">
      <span class="pastille alerte">${echapper(c.ecarts.join(" ; "))}</span></td></tr>` : ""}`).join("")).join("");
  return `
    <div class="encart ${r.conforme ? "bien" : r.compare ? "attention" : ""}">
      <b>${r.contrats} contrat${r.contrats > 1 ? "s" : ""} dans ce fichier${r.corrections
        ? ` (et ${r.corrections} correction${r.corrections > 1 ? "s" : ""} d'un mois déjà déclaré)` : ""}${r.compare
        ? `, ${r.contrats_saisis} saisi${r.contrats_saisis > 1 ? "s" : ""} dans la paie du mois` : ""}</b>
      ${r.conforme ? "Tous les contrats de la paie sont dans la DSN, tels que la paie les dit."
        : r.compare ? `${r.absents.length ? r.absents.length + " contrat(s) de la paie manquent au fichier. " : ""}${ecarts
          ? ecarts + " contrat(s) du fichier ne disent pas ce que dit la paie : réécrire le mois." : ""}`
        : "Pas de paie saisie à comparer : le fichier se lit seul."}
    </div>
    ${r.absents.length ? `<ul class="absents">${r.absents.map((x) =>
      `<li><span class="pastille alerte">absent du fichier</span> ${echapper(x.nom)} — contrat
       ${echapper(x.numero)}, du ${echapper(jjmmaaaa(x.debut))}</li>`).join("")}</ul>` : ""}
    <table class="recap-dsn">
      <tr><th>Salarié</th><th>Contrat</th><th>Dates</th><th>Emploi</th>
        <th class="num">Activité</th><th class="num">Brut</th><th>Fin</th></tr>
      ${lignes}
    </table>`;
}

function corpsDsn(a) {
  const v = a.ordres || [];
  return `
    <p class="discret">${echapper(a.nom)} — ${echapper(a.libelle_type)}
      (type ${echapper(a.type_declaration)}), ordre ${echapper(a.ordre)},
      fraction ${echapper(a.fraction)}, constituée le ${echapper(a.constitution)}.
      ${a.salaries} salarié(s), ${a.lignes} lignes, SIREN ${echapper(a.siren)}.</p>
    ${a.remplace ? `<div class="encart"><b>Annule et remplace</b>
      Ce fichier remplace la déclaration ${echapper(a.remplace)}.</div>` : ""}
    ${a.remplacement?.fichier_remplace ? `<div class="encart attention">
      <b>Annule-et-remplace intégral — ordre ${echapper(a.ordre)}</b>
      Il remplace la DSN réelle déjà partie pour ce mois (ordre
      ${echapper(a.remplacement.dernier_ordre)}), ordres de télépaiement
      compris : c'est lui qui vaudra. Admis jusqu'au
      ${echapper(jjmmaaaa(a.remplacement.limite))} à minuit.</div>` : ""}
    ${a.pas_bareme ? `<div class="encart attention">
      <b>Prélèvement à la source : ${a.pas_bareme} versement(s) sur
      ${a.pas_versements} partent sur la grille par défaut</b>
      Faute de taux transmis par la DGFiP, c'est la grille de l'article 204 H
      du CGI qui s'applique — c'est ce que la loi prévoit, et le dépôt n'est
      pas bloqué. Mais elle prélève le plus souvent <b>plus</b> que le taux
      réel du salarié, qui ne récupère la différence qu'à sa déclaration de
      revenus l'année suivante. Relever les retours avant de déposer peut
      encore changer cela.</div>` : ""}
    ${recapitulatifDsn(a.recapitulatif)}
    <details class="replie">
      <summary>Ordres de télépaiement (${v.length}) — la conséquence de ce que
        la DSN déclare ; un dépôt réel les déclenche tous</summary>
      <div class="encart ${enFictive() ? "info" : "attention"}">
      ${a.ordres_erreur ? `<p><b>${echapper(a.ordres_erreur)}</b></p>` : ""}
      ${v.length
        ? `<table class="ordres">
             <tr><th>À qui</th><th>Pour quoi</th><th class="nombre">Montant</th><th>Prélevé</th></tr>
             ${v.map((x) => `<tr>
               <td><b>${echapper(x.organisme)}</b></td>
               <td>${echapper(x.objet)}</td>
               <td class="nombre"><b>${echapper(euros(x.montant))}</b></td>
               <td title="${echapper(x.source)}">${echapper(x.quand)}</td></tr>
             ${x.credit ? `<tr class="credit"><td></td><td colspan="3">
               <div class="encart" title="${echapper(x.credit.source)}">
               <b>${echapper(x.credit.titre)}</b>
               ${echapper(x.credit.que_faire)}</div></td></tr>` : ""}`).join("")}
           </table>
           Chaque organisme prélèvera ce montant sur le compte de la compagnie. Ce ne sont pas des indications.
           <span class="discret">La date se lit dans la DSN ou dans le texte qui la fixe (au survol).</span>`
        : "Aucun ordre de télépaiement dans ce fichier."}
      </div>
    </details>`;
}

function corpsDpae(a) {
  const deja = (a.embauches || []).filter((x) => x.deja_declaree).length;
  return `
    <p class="discret">${echapper(a.nom)} — ${a.enregistrements}
      enregistrement(s), ${a.octets} octets, indicateur
      ${echapper(a.indicateur)}.</p>
    <table>
      <tr><th>Salarié</th><th>Contrat</th><th>Du</th><th>Au</th>
        <th>Trace d'envoi</th></tr>
      ${(a.embauches || []).map((x) => `
        <tr>
          <td>${echapper(x.nom)}</td>
          <td>${echapper(x.type_contrat || "")}</td>
          <td>${echapper(jjmmaaaa(x.debut))}</td>
          <td>${x.fin ? echapper(jjmmaaaa(x.fin))
                      : `<span class="discret">sans terme</span>`}</td>
          <td>${x.deja_declaree
                ? `<span class="pastille alerte">déjà déclarée</span>`
                : `<span class="pastille bien">à déclarer</span>`}</td>
        </tr>`).join("")}
    </table>
    ${deja ? `<div class="encart attention">
      <b>${deja} embauche(s) de ce fichier ont déjà une trace d'envoi</b>
      Le fichier porte TOUT le mois : le déposer les redéclare. Déclarer deux
      fois la même embauche n'est pas anodin — l'Urssaf reçoit deux
      déclarations pour un seul contrat.</div>` : ""}`;
}

/** LA DSN DU MOIS SE FABRIQUE À SON DÉPÔT : le moteur l'écrit depuis les
 *  engagements validés (`dsn-a-deposer`), puis la boîte de dépôt s'ouvre sur
 *  elle, en test d'abord. Ce que le moteur en dit se lit sous la barre. */
async function deposerLaDsn(mois) {
  let r;
  try {
    r = await api("dsn-a-deposer", { mois });
  } catch (e) { dire(e.message, "erreur"); return; }
  const dit = (r.avertissements || []).concat(r.informations || []);
  if (dit.length) dire(dit.join("  ·  "));
  await rafraichirActions();
  if (r.chemin) await boiteDepot("dsn", r.chemin, { testerDabord: true });
}

async function boiteDepot(quoi, fichier, { testerDabord = false } = {}) {
  let a;
  try {
    a = await api("depot-apercu", { quoi, fichier });
  } catch (e) { dire(e.message, "erreur"); return; }

  ouvrirBoite(`
    <h3>Déposer ${quoi === "dsn" ? "la DSN" : "la DPAE"}</h3>
    ${moisEnGros(a.mois)}
    ${quoi === "dsn" ? corpsDsn(a) : corpsDpae(a)}
    ${a.deja_depose ? `<div class="encart attention">
      <b>Ce fichier a déjà été déposé</b>
      Le ${echapper(quand(a.deja_depose.quand))}, en mode
      ${echapper(a.deja_depose.mode)}. Redéposer le même fichier ne remplace
      rien : un annule-et-remplace se produit, il ne se redépose pas.</div>` : ""}
    <div id="d-bilan">${encartRepetition(a.repetition, quoi, a)}</div>
    ${enFictive() ? noteFictive(quoi === "dsn" ? "net-entreprises" : "l'Urssaf")
      : `<p class="note">Un dépôt réel demande le mot de passe <b>du logiciel</b>,
      qui ne sert qu'à prouver que quelqu'un est là. Celui de l'organisme ne
      passe pas par cette page : le moteur va le chercher au coffre.</p>`}
    <div class="actions">
      <button class="secondaire" onclick="fermerBoite()">Annuler</button>
      <button class="primaire" id="d-test">Envoyer le test</button>
      <button class="secondaire" id="d-reel" disabled>
        ${enFictive() ? `Envoyer — simulé (${echapper(a.mois)})` : `Envoyer le réel — ${echapper(a.mois)}`}</button>
    </div>
    ${a.reel ? `<p class="discret"><button type="button" class="lien" id="d-sans-test">envoyer
      ${enFictive() ? "sans attendre le test — simulé…" : "en réel sans attendre le test…"}</button></p>` : ""}
    ${a.reel ? "" : `<p class="note">Le fichier est codé <b>envoi de test</b> :
      déposé « pour de vrai », il ne déclarerait rien et rien ne serait
      conservé.</p>`}`);

  // Le mot de passe tapé pour le dépôt sert au renvoi (boiteRenvoiDsn) :
  // on ne le redemande pas. Une erreur l'oublie, un refus aussi.
  let mdpConnu = null;
  const lancer = async (reel, forcer = false, contournement = "") => {
    let mdp = "";
    if (reel) {
      mdp = mdpConnu || await demanderMotDePasse("le dépôt réel de " + (quoi === "dsn" ? "la DSN" : "la DPAE"), { envoi: true });
      if (!mdp) return;
      mdpConnu = mdp;
    }
    document.getElementById("d-test").disabled = true;
    document.getElementById("d-reel").disabled = true;
    const rendre = occupe(document.getElementById(reel ? "d-reel" : "d-test"),
                          reel ? "Dépôt réel en cours…" : "Envoi du test en cours…");
    try {
      const r = await api("deposer", { quoi, fichier, reel, motdepasse: mdp,
                                       forcer, contournement });
      rendre();
      if (!reel) {
        // LE TEST NE FERME PLUS LA BOÎTE (25/09/2026) : « ça a quitté la
        // fenêtre au lieu de rester et d'attendre les retours pour proposer
        // d'envoyer direct ». On reste, on attend le compte rendu, et le
        // bouton du réel s'allume ici même.
        dire("Dépôt de test envoyé — on attend son compte rendu.", "envoye");
        attendreBilan();
        rafraichirActions();
        return;
      }
      fermerBoite();
      dire("Dépôt réel effectué — on relève ses retours.", "envoye");
      if (quoi === "dsn") avisDocumentsApresDsn(r.mois);
      await rafraichirActions();
      suivreDepot(quoi, r.flux, r.mois);
    } catch (e) {
      rendre();
      const test = document.getElementById("d-test"), reelB = document.getElementById("d-reel");
      if (test) test.disabled = false;
      if (reelB) reelB.disabled = !a.reel;
      if (e.detail?.type === "RepetitionManquante") {
        // « Aucun acte irréversible sans une répétition réussie du même
        // contenu. » On ne renvoie pas l'utilisateur chercher une commande :
        // le dépôt de test est un bouton, ici, tout de suite.
        return boiteRepetition(e.detail.etat, quoi,
                               () => lancer(false),
                               (motif) => lancer(true, false, motif));
      }
      if (e.detail?.type === "DsnDejaDeposee") {
        // « On attend le retour, et si vraiment on y tient, après un
        // avertissement, on peut renvoyer ». Le mot de
        // passe déjà tapé sert au renvoi : on ne le redemande pas.
        return boiteRenvoiDsn(e.detail.etat, () => lancer(true, true));
      }
      mdpConnu = null;
      dire(e.message, "erreur");
    }
  };
  // LE COMPTE RENDU DU TEST, ATTENDU DANS LA BOÎTE. Toutes les dix secondes,
  // cinq minutes au plus : il arrive le plus souvent dans la minute. Le réel
  // reste éteint tant qu'on attend ; conforme, il devient LE bouton ; refusé,
  // on dit pourquoi et il reste éteint. Passé cinq minutes, on cesse de
  // demander (chaque tour relève les retours chez l'organisme : rien ne doit
  // tourner sans fin) et le réel reste éteint ; le relevé ordinaire rangera le
  // compte rendu, et l'étape du mois ouvrira alors le réel.
  const zone = () => document.getElementById("d-bilan");
  const boutonReel = (principal) => {
    const b = document.getElementById("d-reel"), t = document.getElementById("d-test");
    if (!b) return;
    b.disabled = !a.reel || !principal;
    b.className = principal ? "primaire" : "secondaire";
    if (t) t.className = principal ? "secondaire" : "primaire";
    if (principal && a.reel) b.focus();
    const sansTest = document.getElementById("d-sans-test");
    if (sansTest && principal) sansTest.hidden = true;
  };
  // La boîte peut se fermer pendant l'attente : l'écoute continue, et le dit
  // en bas de page quand le compte rendu arrive (« après un test aussi »).
  // `el` est CETTE boîte : une autre ouverte entre-temps n'est pas touchée.
  const attendreBilan = async () => {
    const el = zone();
    const ici = () => el && el.isConnected;
    const ecrire = (html) => { if (ici()) el.innerHTML = html; };
    const test = document.getElementById("d-test");
    boutonReel(false);
    if (test) test.disabled = true;
    const libre = () => { if (ici() && test) test.disabled = false; };
    const nom = quoi === "dsn" ? "La DSN" : "La DPAE";
    // Seul le bilan du test (le certificat, ou le bilan d'anomalies) ouvre le
    // réel ou le ferme ; l'accusé d'enregistrement dit « envoi reçu », pas
    // « conforme » (tickets #221, #222 ; le moteur refuse le réel sans lui, DSN
    // comme DPAE).
    for (let tour = 0; tour < 30; tour++) {
      let r = null;
      try { r = await api("bilan-repetition", { quoi, fichier }); } catch (e) { /* on réessaie */ }
      if (r && r.conforme && r.compte_rendu === "ok") {
        ecrire(encartRepetition(r, quoi, a));
        libre();
        if (ici()) boutonReel(true);
        else dire(`${nom} est testée — conforme. Le dépôt réel est prêt.`);
        rafraichirActions();
        return;
      }
      if (r && !r.conforme && (r.ratees || []).length) {
        const l = r.ratees[r.ratees.length - 1];
        const anos = (l.retours || []).flatMap((x) => x.anomalies || []);
        const motif = anos.join(" ; ") || l.reponse || "motif non lu";
        ecrire(`<div class="encart attention"><b>Le test est refusé</b>
          ${echapper(motif)}
          Le même refus attendrait le dépôt réel : c'est cela qu'il faut corriger.</div>`);
        libre();
        if (!ici()) dire(`Le test de ${nom.toLowerCase()} est refusé : ${motif}`, "erreur");
        rafraichirActions();
        return;
      }
      ecrire((r && r.conforme ? encartRepetition(r, quoi, a)
        : `<div class="encart"><b>Test envoyé — son compte rendu est attendu</b> Le dépôt réel se fait
            ici, une fois le test revenu conforme.</div>`)
        + (r && r.incident ? `<p class="discret">${echapper(r.incident)}</p>` : ""));
      await new Promise((ok) => setTimeout(ok, enFictive() ? 3000 : 10000));
    }
    ecrire(`<div class="encart attention"><b>Le compte rendu n'est
      pas arrivé en cinq minutes</b> L'envoi est reçu, mais rien ne dit encore
      qu'il est conforme : le dépôt réel attend ce compte rendu. Il se rangera
      au prochain relevé, et l'étape du mois ouvrira alors le dépôt réel.</div>`);
    libre();
  };

  document.getElementById("d-test").onclick = () => lancer(false);
  document.getElementById("d-reel").onclick = () => lancer(true);
  // LE TEST D'ABORD, TOUJOURS (ticket #103, 02/10/2026 : « on propose d'abord
  // un test, on regarde si tout est ok et après on propose le réel »). Le
  // réel reste éteint tant que le test n'est pas revenu conforme. Passer
  // outre reste possible, mais c'est un petit lien, et le moteur demande
  // alors un motif écrit (RepetitionManquante → boiteRepetition).
  const sansTest = document.getElementById("d-sans-test");
  if (sansTest) sansTest.onclick = () => lancer(true);
  // Un test déjà fait pour ce contenu : s'il attend encore son compte rendu,
  // on l'attend ici ; s'il est arrivé, le réel est le bouton principal.
  const rep = a.repetition;
  if (rep && rep.conforme) {
    if (rep.compte_rendu === "ok") boutonReel(true);
    else attendreBilan();
  } else {
    // Pas de test pour ce contenu : c'est lui qui vient d'abord.
    document.getElementById("d-test").className = "primaire";
    document.getElementById("d-reel").className = "secondaire";
    // LE MOIS QU'ON VIENT DE FERMER PART EN TEST TOUT SEUL (29/09/2026) :
    // « on produit la DSN de test complète et on attend le retour du test,
    // ensuite on propose d'envoyer la DSN réelle ». Un test n'engage rien.
    if (testerDabord) lancer(false);
  }
}

/** APRÈS LA DSN RÉELLE, LES DOCUMENTS DES SALARIÉS. Rien ne s'annonce ici
 *  (« on ne promet rien ») : ce qui manque encore aux documents -- l'AER de
 *  France Travail, quand elle est due -- c'est le moteur qui le dit, dans
 *  l'étape 4 (`documents_prets`). */
function avisDocumentsApresDsn(mois) {
  ouvrirBoite(`
    <h3>DSN envoyée</h3>
    ${moisEnGros(mois)}
    <div class="encart info"><b>Les documents des salariés</b> Ils partent de
      l'étape 4, « Envoyer les documents… », qui dit ce qu'il leur manque
      encore.</div>
    <div class="actions">
      <button class="primaire" onclick="fermerBoite()">Compris</button>
    </div>`);
}

/** L'ÉCOUTE DES RETOURS, JUSTE APRÈS UN DÉPÔT RÉEL (25/09/2026 : « pour que
 *  ça devienne vert le plus vite possible »). Toutes les quinze secondes,
 *  dix minutes au plus, sans attendre la veille de vingt minutes ; l'écran
 *  se repeint dès que le dépôt est tranché. */
async function suivreDepot(quoi, flux, mois) {
  if (!flux) return;
  const nom = quoi === "dsn" ? "La DSN" : "La DPAE";
  for (let tour = 0; tour < 40; tour++) {
    await new Promise((ok) => setTimeout(ok, enFictive() ? 4000 : 15000));
    let r = null;
    try { r = await api("suivre-depot", { quoi, flux, mois }); } catch (e) { continue; }
    if (r && r.fini) {
      dire(r.passe ? `${nom} est passée — retour de l'organisme reçu.`
                   : `${nom} est refusée — voir le motif dans l'étape.`, r.passe ? "" : "erreur");
      await rafraichirActions();
      return;
    }
  }
}

/** Où en est la répétition de CE contenu — avant de proposer le dépôt réel.
 *  Un bouton qui refusera au clic est un bouton qui ment. */
/** Les embauches que porte une DPAE, nommées (ticket #105, 02/10/2026 :
 *  deux salariés partis dans un seul dépôt, et l'écran disait « Cette DPAE »
 *  au singulier, sans dire pour qui -- on a cru qu'une seule était partie). */
function embauchesDe(a) {
  const l = (a && a.embauches) || [];
  if (!l.length) return "";
  return ` (${l.length} embauche${l.length > 1 ? "s" : ""} : ${l.map((x) => x.nom).join(", ")})`;
}

/** Ce que le certificat de conformité (nature 41) dit de chaque personne. */
function certificatParPersonne(ligne) {
  const dpae = ((ligne && ligne.retours) || []).flatMap((x) => x.dpae || []);
  if (!dpae.length) return "";
  return `<ul class="liste-certificat">${dpae.map((x) => `
    <li>${echapper([x.nom, x.prenom].filter(Boolean).join(" "))} :
      ${x.enregistree ? `<span class="pastille bien">enregistrée${x.dossier
          ? ` — dossier ${echapper(x.dossier)}` : ""}</span>`
        : x.deja_existante ? `<span class="pastille">déjà existante</span>`
        : `<span class="pastille erreur">${echapper(x.libelle || x.code || "refusée")}</span>`}</li>`).join("")}</ul>`;
}

function encartRepetition(r, quoi, a) {
  if (!r) return "";
  const cette = (quoi === "dsn" ? "Cette DSN" : "Cette DPAE")
    + (quoi === "dsn" ? "" : echapper(embauchesDe(a)));
  // Les deux phrases de l'éditeur (25/09/2026) : « cette DPAE est déjà
  // testée, voulez-vous l'envoyer » — ou « elle est déjà envoyée en test,
  // on attend les retours, qui arrivent bientôt ».
  if (r.conforme && r.compte_rendu === "ok") {
    return `<div class="encart bien">
      <b>${cette} est testée — conforme. Voulez-vous l'envoyer ?</b>
      Testée le ${echapper(quand(r.conforme.quand))} : le même contenu est
      passé au bac à sable sans anomalie. ${enFictive() ? "L'envoi, simulé, est le bouton « Envoyer — simulé »." : "Le dépôt réel est le bouton « Envoyer le réel »."}
      ${certificatParPersonne(r.conforme)}</div>`;
  }
  if (r.conforme) {
    return `<div class="encart">
      <b>${cette} est en test — son compte rendu est attendu</b>
      Envoyée en test le ${echapper(quand(r.conforme.quand))}. ${enFictive() ? "L'envoi simulé se" : "Le dépôt réel se"}
      fait ici, une fois ce compte rendu revenu conforme.</div>`;
  }
  // Jamais essayé : ce n'est pas une mise en garde, c'est l'étape suivante
  // (26/09/2026 : l'orange se garde pour ce qui demande de corriger).
  const rate = (r.ratees || []).length || (r.perimees || []).length;
  return `<div class="encart ${rate ? "attention" : ""}">
    <b>${rate ? "Pas de test valable pour ce contenu" : "D'abord, le dépôt de test"}</b>
    ${echapper(situationRepetition(r))} ${enFictive() ? "L'envoi simulé se fait une fois le" : "Le dépôt réel se fait une fois le"}
    test revenu conforme.</div>`;
}

function situationRepetition(etat) {
  const ratees = etat.ratees || [], perimees = etat.perimees || [];
  if (ratees.length) {
    const l = ratees[ratees.length - 1];
    const anos = (l.retours || []).filter((x) => x.verdict === "KO").flatMap((x) => x.anomalies || []);
    return `Ce contenu a été essayé en test le ${quand(l.quand)}`
      + ` et l'essai a échoué${anos.length ? ` : ${anos.join(" ; ").replace(/\.$/, "")}` : ""}.`
      + ` C'est cela qu'il faut corriger, le même refus attend en réel.`;
  }
  if (perimees.length) {
    return `Un fichier du même nom a bien été essayé le ${quand(perimees[perimees.length - 1].quand)},`
      + ` mais son contenu a changé depuis : un octet qui bouge rend le test caduc.`;
  }
  return "Ce contenu n'a jamais été déposé en test.";
}

/** Le verrou test → réel. Deux issues, et la première est la bonne : faire
 *  la répétition. La seconde existe parce que le service de test sera
 *  indisponible un 5 du mois à 23 h — et elle demande un motif écrit, qui
 *  part au journal des dépôts. */
function boiteRepetition(etat, quoi, repeter, quandMeme) {
  ouvrirBoite(`
    <h3>Ce contenu n'a pas été répété</h3>
    <div class="encart attention">
      <b>Aucun dépôt de test ne répond de ce fichier</b>
      ${echapper(situationRepetition(etat))}
    </div>
    <p class="note">Un dépôt de test exécute les mêmes contrôles et rend le
      même bilan, sans rien conserver et sans rien déclarer. C'est la seule
      façon de savoir qu'un dépôt réel passera — et une déclaration ne se
      rattrape pas, elle s'annule-et-remplace.</p>
    <div class="actions">
      <button class="secondaire" onclick="fermerBoite()">Annuler</button>
      <button class="primaire" id="r-tester">Faire le dépôt de test</button>
    </div>
    <p class="note" style="margin-top:18px"><a href="#" id="r-outre">Déposer
      sans répétition</a> — si le service de test est indisponible et que
      l'échéance ne peut pas attendre.</p>
    <div id="r-motif" class="cachee">
      <p class="note">Dire ce qui empêchait le test, en une phrase : c'est ce
        qu'on relira le jour où ce dépôt posera question. Le motif est inscrit
        au journal des dépôts.</p>
      <textarea id="r-raison" rows="2" placeholder="le service de test de net-entreprises répond 503 depuis ce matin"></textarea>
      <div class="actions">
        <button class="secondaire" id="r-quand-meme">${enFictive() ? "Déposer sans répétition — simulé" : "Déposer sans répétition"}</button>
      </div>
    </div>`);
  document.getElementById("r-tester").onclick = () => { fermerBoite(); repeter(); };
  document.getElementById("r-outre").onclick = (e) => {
    e.preventDefault();
    document.getElementById("r-motif").classList.remove("cachee");
    document.getElementById("r-raison").focus();
  };
  document.getElementById("r-quand-meme").onclick = () => {
    const motif = document.getElementById("r-raison").value.trim();
    if (motif.length < 10) {
      dire("Le motif se lit en une phrase : dire ce qui empêchait le test.");
      return;
    }
    fermerBoite();
    quandMeme(motif);
  };
}

/** Une DSN réelle est déjà partie pour ce mois. Dire où elle en est, et ne
 *  renvoyer que si on le veut vraiment. */
function boiteRenvoiDsn(etat, renvoyer) {
  const quand = etat && etat.quand ? quand_(etat.quand) : "?";
  const situation = etat.passee
    ? `Le bilan de conformité est arrivé : <b>elle est passée</b>, il n'y a
       rien à faire de notre côté.`
    : `<b>Son retour n'est pas encore arrivé</b> : on attend, on ne renvoie pas.`;
  ouvrirBoite(`
    <h3>Une DSN est déjà partie pour ce mois</h3>
    <div class="encart attention">
      <b>Déposée le ${echapper(quand)}${etat.flux ? ` — flux ${echapper(etat.flux)}` : ""}</b>
      ${situation}
    </div>
    <p class="note">Renvoyer quand même fait un <b>annule-et-remplace</b> à
      net-entreprises : la nouvelle DSN remplace la précédente, avec ses ordres
      de télépaiement. À ne faire que si c'est voulu — une paie corrigée, par
      exemple — et jamais parce qu'on n'a pas encore vu le retour.</p>
    <div class="actions">
      <button class="primaire" onclick="fermerBoite()">Ne pas renvoyer</button>
      <button class="secondaire" id="d-renvoyer">${enFictive() ? "Renvoyer quand même — simulé" : "Renvoyer quand même"}</button>
    </div>`);
  document.getElementById("d-renvoyer").onclick = () => { fermerBoite(); renvoyer(); };
}
const quand_ = (iso) => quand(iso);

/* ------------------------------------------ les documents aux salariés */
/** LES DOCUMENTS DU MOIS : chaque salarié à qui ils partent, coché ; on
 *  décoche. Deux gestes : Imprimer (un seul PDF, salarié par salarié : bulletin,
 *  certificat, AER) et Envoyer par courriel quand une boîte est posée. Les deux
 *  sont un départ : le moteur numérote et fige la paie. */
async function boiteEnvoi(mois) {
  let a;
  try {
    a = await api("envoi-apercu", { mois });
  } catch (e) { dire(e.message, "erreur"); return; }

  const peut = a.pret && a.messages.length;
  ouvrirBoite(`
    <h3>Les documents</h3>
    ${moisEnGros(mois)}
    ${a.boite_liee && a.expediteur ? `<p class="discret">Courriels depuis ${echapper(a.expediteur)}.</p>` : ""}
    ${(a.empechements || []).map((x) =>
      `<div class="encart attention">${echapper(x)}</div>`).join("")}
    ${(a.avertissements || []).concat(a.informations || []).map((x) =>
      `<div class="encart info">${echapper(x)}</div>`).join("")}
    ${!a.messages.length ? "" : `<table class="table-envoi">
      <colgroup><col class="c-coche"><col class="c-salarie">${a.boite_liee ? `<col class="c-adresse">` : ""}<col class="c-pieces"></colgroup>
      <tr><th></th><th>Salarié</th>${a.boite_liee ? "<th>Adresse</th>" : ""}<th>Pièces</th></tr>
      ${a.messages.map((m) => `
        <tr>
          <td><input type="checkbox" class="e-qui" value="${echapper(m.cle)}" checked
                     aria-label="${echapper(m.nom)}"></td>
          <td>${echapper(m.nom)}${m.renvoi ? `<br><span class="pastille alerte">documents corrigés —
            remplacent ceux du ${echapper(m.envoye_le)}</span>` : ""}</td>
          ${a.boite_liee ? `<td>${echapper(m.destinataire)}</td>` : ""}
          <td class="pieces">${m.pieces.map((x) => `<div>${echapper(x)}</div>`).join("")}</td>
        </tr>
        ${m.renvoi ? `<tr><td></td><td colspan="${a.boite_liee ? 3 : 2}" class="discret">Ce qui a changé :
          <ul>${m.changements.map((c) => `<li>${echapper(c)}</li>`).join("")}</ul></td></tr>` : ""}`).join("")}
    </table>`}
    ${a.deja ? `<div class="encart attention"><b>Déjà partis</b>
      Le ${echapper(quand(a.deja.quand))} — ${echapper(a.deja.reponse || "")}.</div>` : ""}
    ${peut && a.boite_liee && enFictive() ? noteFictive("les salariés", false) : ""}
    <div class="actions">
      <button class="secondaire" onclick="fermerBoite()">Fermer</button>
      ${peut ? `<button class="${a.boite_liee ? "secondaire" : "primaire"}" id="e-imprimer"
        title="Les documents des salariés cochés, en un seul PDF, à imprimer et à remettre en main propre">Imprimer</button>` : ""}
      ${peut && a.boite_liee ? `<button class="primaire" id="e-envoyer">Envoyer par courriel${enFictive() ? " — simulé" : ""}</button>` : ""}
    </div>`);

  const coches = () => [...document.querySelectorAll(".e-qui:checked")].map((c) => c.value);
  const imprimer = document.getElementById("e-imprimer");
  if (imprimer) imprimer.onclick = async () => {
    const salaries = coches();
    if (!salaries.length) { dire("Aucun salarié coché."); return; }
    const rendre = occupe(imprimer, "Impression…");
    try {
      const r = await api("documents-imprimer", { mois, salaries });
      fermerBoite();
      dire("Documents de " + r.salaries.length + " salarié(s) à imprimer.", "envoye");
      await ouvrirImpression(mois, r.impression);
      await rafraichirActions();
    } catch (e) {
      rendre();
      dire(e.message, "erreur");
    }
  };
  const envoyer = document.getElementById("e-envoyer");
  if (envoyer) envoyer.onclick = async () => {
    const salaries = coches();
    if (!salaries.length) { dire("Aucun salarié coché."); return; }
    const mdp = await demanderMotDePasse("envoyer les documents à " + salaries.length + " salarié(s)", { envoi: true });
    if (!mdp) return;
    const rendre = occupe(envoyer, "Envoi des documents en cours…");
    const repere = await repereCourrielsFictifs();
    try {
      const r = await api("envoyer-documents", { mois, salaries, motdepasse: mdp });
      fermerBoite();
      dire(r.partis.length + " message(s) envoyé(s).", "envoye");
      afficherEnvoi("Documents aux salariés", mois, r.partis, r.avertissements);
      fenetreCourrielsFictifs("documents", repere);
      await rafraichirActions();
    } catch (e) {
      rendre();
      dire(e.message, "erreur");
    }
  };
}

/* --------------------------------------------- les contrats à faire signer */

/** « signé par courriel le … » ou « signé, reçu le … » : le journal sait
 *  d'où vient la mention (contrats_recus_comment), l'écran ne fait que
 *  l'écrire. */
function signeLe(quandRecu, par) {
  return (par === "courriel" ? "signé d'un clic le " : "signé, reçu le ") + quand(quandRecu);
}

/** LE PDF À IMPRIMER s'ouvre : le moteur l'a écrit (`impression`), la fenêtre l'ouvre. */
async function ouvrirImpression(mois, fichier) {
  try {
    await api("ouvrir-piece", { impression: mois, fichier });
  } catch (e) { dire(e.message, "erreur"); }
}

/** LES CONTRATS À FAIRE PARTIR : ceux que le moteur dit à envoyer
 *  (`a_envoyer` : jamais partis, ou changés depuis), tous cochés ; on décoche
 *  ce qu'on ne fait pas partir. `seulement` : les salariés choisis ailleurs
 *  (la pastille d'un engagement) ; les autres restent dans la liste, décochés.
 *  Un contrat signé qui a changé part en avenant. Deux gestes : Imprimer (un
 *  seul PDF de tout ce qui est coché), et Envoyer à la signature quand une
 *  boîte courriel est posée. */
async function boiteSignature(mois, seulement) {
  let a;
  try {
    a = await api("signature-apercu", { mois });
  } catch (e) { dire(e.message, "erreur"); return; }

  const contrats = a.contrats.filter((c) => c.a_envoyer);
  const coche = (c) => !seulement || seulement.includes(c.salarie);
  const avenantAFaire = (c) => c.caduc && c.recu_le;
  ouvrirBoite(`
    <h3>Faire signer les contrats</h3>
    ${moisEnGros(mois)}
    ${a.boite_liee ? `<div class="encart">
      <b>Signature d'un clic</b>
      Le contrat part par courriel avec un bouton « Je signe ». Le salarié
      appuie dessus et renvoie la réponse toute écrite ; elle est relevée toute
      seule et le salarié reçoit son contrat contresigné.
    </div>` : ""}
    ${a.boite_liee && enFictive() ? noteFictive("les salariés").replace("sa réponse arrive comme la vraie",
      "la signature revient comme la vraie") : ""}
    ${noteDpaeAvance()}
    ${!contrats.length ? `<p class="discret">Aucun contrat à faire partir ce mois-ci.</p>` : `<div class="liste-choix">
      ${contrats.map((c) => `
        <label class="choix">
          <input type="checkbox" class="s-qui" value="${echapper(c.salarie)}"
                 data-debut="${echapper(c.debut)}" data-numero="${echapper(c.numero || "")}" ${avenantAFaire(c) ? 'data-avenant="1"' : ""}
                 ${coche(c) ? "checked" : ""}>
          <span><b>${echapper(c.nom)}</b> — ${echapper(c.spectacle)},
            ${echapper(periodeEnClair(c.debut, c.fin))}
            ${c.numero ? `<span class="discret">(contrat n° ${echapper(c.numero)})</span>` : ""}
            ${a.boite_liee ? `<br><span class="discret">${echapper(c.destinataire)
              || "aucun courriel dans la fiche — l'envoi sera refusé"}</span>` : ""}
            ${avenantAFaire(c) ? `<br><span class="pastille alerte">${echapper(c.pourquoi
              || "la paie a changé depuis la signature")} — signé : partira en avenant</span>
              <button class="lien s-avenant" data-salarie="${echapper(c.salarie)}"
                data-debut="${echapper(c.debut)}" data-numero="${echapper(c.numero || "")}">voir l'avenant</button>`
              : c.caduc ? `<br><span class="pastille alerte">${echapper(c.pourquoi
              || "la paie a changé depuis l'envoi")}${c.demande_le ? ` (parti le ${echapper(quand(c.demande_le))})` : ""} —
              ce contrat est à refaire partir</span>` : ""}</span>
        </label>`).join("")}
    </div>`}
    <div class="actions">
      <button class="secondaire" onclick="fermerBoite()">Fermer</button>
      ${contrats.length ? `<button class="${a.boite_liee ? "secondaire" : "primaire"}" id="s-imprimer"
        title="Les contrats cochés, en un seul PDF, à imprimer et à faire signer à la main : ils partent sous leur numéro">Imprimer</button>` : ""}
      ${contrats.length && a.boite_liee ? `<button class="primaire" id="s-envoyer">${enFictive()
        ? "Envoyer à la signature — simulé" : "Envoyer à la signature"}</button>` : ""}
    </div>`);

  // IMPRIMER : les contrats partent à la main, sous leur numéro, et le
  // journal le note ; le PDF de tous les contrats cochés s'ouvre.
  const imprimer = document.getElementById("s-imprimer");
  if (imprimer) imprimer.onclick = async () => {
    const coches = [...document.querySelectorAll(".s-qui:checked")].filter((c) => !c.dataset.avenant);
    if (!coches.length) { dire("Aucun contrat coché."); return; }
    const choisis = coches.map((c) => ({ salarie: c.value, debut: c.dataset.debut, numero: c.dataset.numero }));
    const rendre = occupe(imprimer, "Impression…");
    try {
      const r = await api("contrats-imprimer", { mois, contrats: choisis });
      fermerBoite();
      dire(r.contrats.length + " contrat(s) à imprimer : " + r.contrats.map((c) => "n° " + c.numero).join(", ") + ".", "envoye");
      await ouvrirImpression(mois, r.impression);
      await rafraichirActions();
    } catch (e) {
      rendre();
      dire(e.message, "erreur");
    }
  };
  document.querySelectorAll(".s-avenant").forEach((b) => {
    b.onclick = (ev) => {
      ev.preventDefault();                 // le bouton est dans un <label>
      fermerBoite();
      boiteAvenant(mois, b.dataset.salarie, b.dataset.debut, b.dataset.numero);
    };
  });
  const envoyer = document.getElementById("s-envoyer");
  if (envoyer) envoyer.onclick = async () => {
    // Par CONTRAT : un salarié peut avoir un contrat coché et un autre non.
    const coches = [...document.querySelectorAll(".s-qui:checked")];
    // Les contrats signés qui ont changé partent en AVENANT, les autres en
    // contrat : un seul bouton, un seul mot de passe.
    const avenants = coches.filter((c) => c.dataset.avenant)
      .map((c) => ({ salarie: c.value, debut: c.dataset.debut, numero: c.dataset.numero, mois }));
    // Le numéro distingue deux contrats du même salarié qui commencent le même jour.
    const choisis = coches.filter((c) => !c.dataset.avenant)
      .map((c) => ({ salarie: c.value, debut: c.dataset.debut, numero: c.dataset.numero }));
    if (!coches.length) { dire("Aucun contrat coché."); return; }
    const quoi = [choisis.length ? choisis.length + " contrat(s)" : "",
                  avenants.length ? avenants.length + " avenant(s)" : ""].filter(Boolean).join(" et ");
    const mdp = await demanderMotDePasse("envoyer " + quoi + " à la signature", { envoi: true });
    if (!mdp) return;
    const rendre = occupe(envoyer, "Envoi de " + quoi + " en cours…");
    const repere = await repereCourrielsFictifs();
    try {
      const r = choisis.length
        ? await api("demander-signature", { mois, contrats: choisis, motdepasse: mdp })
        : { partis: [], contrats: [] };
      // Les avenants d'un même salarié partent ensemble, dans un seul courriel, comme ses contrats.
      if (avenants.length) {
        const av = await api("avenants-envoyer", { avenants, motdepasse: mdp });
        r.partis.push(...av.partis);
        r.contrats.push(...av.avenants.map((x) => ({ fichier: x.fichier })));
      }
      fermerBoite();
      dire(quoi + " envoyé(s) à la signature.", "envoye");
      afficherEnvoi("Contrats à signer", mois, [...new Set(r.partis)],
                    r.contrats.map((c) => c.fichier));
      if (enFictive()) {
        attendreSignaturesFictives();
        fenetreCourrielsFictifs("signature", repere);
      }
      await rafraichirActions();
    } catch (e) {
      rendre();
      dire(e.message, "erreur");
    }
  };
}

/** LES CONTRATS EN ATTENTE : partis, pas revenus signés. Chacun se note
 *  « Reçu signé » (un contrat parti par courriel peut revenir signé sur
 *  papier), et celui que le moteur dit relançable (`relancer` : parti par
 *  courriel, boîte posée) se relance. Une note se défait : le moteur rend
 *  de quoi l'annuler (`annuler`). */
async function boiteContratsEnAttente(mois, fait) {
  let a;
  try {
    a = await api("signature-apercu", { mois });
  } catch (e) { dire(e.message, "erreur"); return; }
  const contrats = a.contrats.filter((c) => c.en_attente);
  ouvrirBoite(`
    <h3>Les contrats en attente</h3>
    ${moisEnGros(mois)}
    ${fait ? `<div class="encart info" id="a-fait">${echapper(fait.texte)}
      <button class="lien" id="a-annuler">Annuler</button></div>` : ""}
    ${!contrats.length ? `<p class="discret">Aucun contrat n'attend sa signature.</p>` : `<div class="liste-choix">
      ${contrats.map((c, i) => `
        <div class="choix">
          <span><b>${echapper(c.nom)}</b> — ${echapper(c.spectacle)},
            ${echapper(periodeEnClair(c.debut, c.fin))}
            ${c.numero ? `<span class="discret">(contrat n° ${echapper(c.numero)})</span>` : ""}
            <br><span class="pastille alerte">${c.par === "imprime" ? "imprimé" : "envoyé"} le
              ${echapper(quand(c.demande_le))} — pas encore revenu signé</span></span>
          <span class="gestes-contrat">
            <button class="primaire petit a-recu" data-i="${i}">Reçu signé</button>
            ${c.relancer ? `<button class="secondaire petit a-relancer" data-i="${i}"
              title="Renvoie le même contrat, avec un mot de rappel">Relancer</button>` : ""}
          </span>
        </div>`).join("")}
    </div>`}
    <div class="actions">
      <button class="secondaire" onclick="fermerBoite()">Fermer</button>
    </div>`);

  const annuler = document.getElementById("a-annuler");
  if (annuler) annuler.onclick = async () => {
    const rendre = occupe(annuler, "…");
    try {
      await api(fait.annuler.route, fait.annuler.charge);
      await rafraichirActions();
      boiteContratsEnAttente(mois);
    } catch (e) { rendre(); dire(e.message, "erreur"); }
  };
  document.querySelectorAll(".a-recu").forEach((b) => {
    b.onclick = async () => {
      const c = contrats[Number(b.dataset.i)];
      const rendre = occupe(b, "…");
      try {
        const r = await api("contrat-recu", { mois, salarie: c.salarie, debut: c.debut, numero: c.numero || "", recu: true });
        await rafraichirActions();
        boiteContratsEnAttente(mois, { texte: "Contrat de " + c.nom + " noté reçu signé.", annuler: r.annuler });
      } catch (e) { rendre(); dire(e.message, "erreur"); }
    };
  });
  document.querySelectorAll(".a-relancer").forEach((b) => {
    b.onclick = async () => {
      const c = contrats[Number(b.dataset.i)];
      const mdp = await demanderMotDePasse("relancer la signature de ce contrat", { envoi: true });
      if (!mdp) return;
      const rendre = occupe(b, "Relance…");
      try {
        const r = await api("demander-signature", { mois, motdepasse: mdp, relance: true,
          contrats: [{ salarie: c.salarie, debut: c.debut, numero: c.numero || "" }] });
        fermerBoite();
        dire("Relance partie : " + r.partis.join(", ") + ".", "envoye");
        await rafraichirActions();
      } catch (e) { rendre(); dire(e.message, "erreur"); }
    };
  });
}

/** L'AVENANT (30/09/2026). Le contrat parti se lit (son PDF), la paie
 *  d'aujourd'hui s'y compare, article par article : ce qui a changé se
 *  montre, l'ancien texte à côté du nouveau. L'avenant part à la signature
 *  par le même circuit « Je signe » que le contrat. Un contrat qui n'a pas
 *  commencé peut aussi être refait en entier (« annule et remplace »). */
async function boiteAvenant(mois, salarie, debut, numero) {
  let a;
  try {
    // Le numéro et le mois disent QUEL contrat, si le salarié en a deux qui commencent le même jour (retours Windows, B).
    a = await api("avenant-apercu", { salarie, debut, numero: numero || "", mois });
  } catch (e) { dire(e.message, "erreur"); return; }
  const mods = a.modifications || [];
  const cite = (textes) => textes.map((x) => `<p class="citation">« ${echapper(x)} »</p>`).join("");
  ouvrirBoite(`
    <h3>Avenant au contrat n° ${echapper(a.contrat || "")}</h3>
    <p class="discret">${echapper(a.nom)} — ${echapper(a.spectacle)}, du
      ${echapper(jjmmaaaa(a.debut))} au ${echapper(jjmmaaaa(a.fin))} ;
      ${a.signe ? "signé le " + echapper(quand(a.signe))
                : "envoyé le " + echapper(jjmmaaaa(a.date_contrat)) + ", pas encore signé"}</p>
    ${(a.avenants || []).map((x) => `<p class="discret">Avenant n° ${echapper(String(x.numero))}
      envoyé le ${echapper(quand(x.quand))} — ${x.signe ? "signé le " + echapper(quand(x.signe))
      : "pas encore revenu signé"}</p>`).join("")}
    ${mods.length ? `
      <div class="encart">Le contrat parti se compare à la paie d'aujourd'hui :
        ${mods.length > 1 ? mods.length + " articles ont changé" : "un article a changé"}.
        L'avenant n° ${a.numero} cite l'ancien texte et le nouveau ; le reste du
        contrat ne change pas.</div>
      ${mods.map((m) => `<div class="modif-avenant">
        <b>Article ${m.numero} — ${echapper(m.article)}</b>
        <div class="discret">Avant</div>${cite(m.avant)}
        <div class="discret">Désormais</div>${cite(m.apres)}
      </div>`).join("")}
      <label>Date d'effet de l'avenant
        <input type="date" id="av-effet" value="${echapper(a.effet)}"></label>`
    : `<div class="encart">Le contrat parti dit déjà tout ce que dit la paie :
        il n'y a rien à modifier.</div>`}
    ${mods.length && a.remplacable ? `<p class="discret">Le contrat n'a pas commencé : il peut aussi être
      refait en entier. Le nouveau porte en tête « annule et remplace le contrat
      n° ${echapper(a.contrat || "")} du ${echapper(jjmmaaaa(a.date_contrat))} ».</p>` : ""}
    <p class="discret"><a href="#" id="av-pieces">Voir ce qui est parti (contrat envoyé, signé, preuve)</a></p>
    <div class="actions">
      <button class="secondaire" onclick="fermerBoite()">Fermer</button>
      ${mods.length && a.remplacable ? `<button class="secondaire" id="av-remplace">${enFictive() ? "Contrat qui annule et remplace — simulé…" : "Contrat qui annule et remplace…"}</button>` : ""}
      ${mods.length ? `<button class="primaire" id="av-envoyer">${enFictive() ? "Envoyer l'avenant à la signature — simulé…" : "Envoyer l'avenant à la signature…"}</button>` : ""}
    </div>`);
  document.getElementById("av-pieces").onclick = (ev) => {
    ev.preventDefault();
    fermerBoite();
    boitePieces({ salarie, debut, numero, nom: a.nom, fin: a.fin, spectacle: a.spectacle,
                  etats: { signature: { fait: a.date_contrat, recu: a.signe } } }, "signature");
  };
  const envoyer = document.getElementById("av-envoyer");
  if (envoyer) envoyer.onclick = async () => {
    const effet = document.getElementById("av-effet").value || a.effet;
    const mdp = await demanderMotDePasse("envoyer l'avenant n° " + a.numero + " à " + a.nom, { envoi: true });
    if (!mdp) return;
    const rendre = occupe(envoyer, "Envoi de l'avenant…");
    try {
      const r = await api("avenant-envoyer", { salarie, debut, numero: numero || "", mois, effet, motdepasse: mdp });
      fermerBoite();
      dire("Avenant n° " + r.numero + " envoyé à la signature : " + r.partis.join(", ") + ".", "envoye");
      afficherEnvoi("Avenant à signer", a.mois, r.partis, [r.fichier]);
      await rafraichirActions();
    } catch (e) {
      rendre();
      dire(e.message, "erreur");
    }
  };
  const remplace = document.getElementById("av-remplace");
  if (remplace) remplace.onclick = async () => {
    const mdp = await demanderMotDePasse("envoyer le contrat qui annule et remplace le n° " + (a.contrat || ""), { envoi: true });
    if (!mdp) return;
    const rendre = occupe(remplace, "Envoi du contrat…");
    try {
      const r = await api("demander-signature", { mois: a.mois, contrats: [{ salarie, debut, numero: numero || a.contrat || "" }],
                                                  motdepasse: mdp, remplace: true });
      fermerBoite();
      dire("Contrat qui annule et remplace le n° " + (a.contrat || "") + " envoyé : "
           + r.partis.join(", ") + ".", "envoye");
      afficherEnvoi("Contrat à signer", a.mois, r.partis, r.contrats.map((c) => c.fichier));
      await rafraichirActions();
    } catch (e) {
      rendre();
      dire(e.message, "erreur");
    }
  };
}

function afficherEnvoi(titre, mois, partis, details) {
  document.getElementById("resultat").innerHTML = `
    <div class="carte" style="margin-top:20px">
      <h2>${echapper(titre)} — ${echapper(moisEnClair(mois))}</h2>
      <div class="encart bien"><b>${partis.length} message(s) parti(s)</b>
        ${echapper(partis.join(", "))}</div>
      ${(details || []).length
        ? `<p class="note">${echapper(details.join(" · "))}</p>` : ""}
    </div>`;
}


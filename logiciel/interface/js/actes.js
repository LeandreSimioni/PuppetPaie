/* Ce qui part : les étapes du mois, et les boîtes qui déposent, envoient et font signer. */
"use strict";

/* =======================================================================
   LES ÉTAPES DU MOIS
   =======================================================================
   Le moteur rend (`etat-du-mois`) les colonnes du mois et, étape par étape
   (DPAE, Contrats, DSN, Documents), les groupes à dessiner : chacun son état
   (`fait`, `a_faire`, `en_attente`, `refuse`), les colonnes qu'il couvre, ce
   qu'il écrit (`compte_texte` en gras, `quoi`, `echeance.court`) et le geste
   qu'ouvre un clic. La page en fait des cases alignées sur les colonnes — la
   première, la grande, porte ce qui est fait ; ce qui reste sort à côté — et
   relie les cases d'une étape à la suivante par des traits droits (l'arbre).
   Elle ne choisit ni une couleur hors de l'état, ni une phrase, ni un geste.

   RIEN NE PART D'UN CLIC : une case ouvre un récapitulatif, qui demande le
   mot de passe DU LOGICIEL, jamais celui de l'organisme (le moteur va le
   chercher lui-même au coffre). C'est le moteur qui refuse un acte, et lui
   seul : un garde-fou posé dans un front se contourne par l'autre.
*/

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

/** La classe d'une case : la couleur que le moteur donne au groupe (`couleur`) ; une attente se dessine en deux moitiés. */
const classeDeCase = (g) => g.coupee ? "en-attente" : (g.couleur || "gris");

/** Les colonnes déjà dessinées pour un mois : une colonne qui apparaît entre en glissant. */
const COLONNES_VUES = new Map();

/** Une case : un groupe d'une étape (`k`, son rang), placé sur ses colonnes. */
function dessinerCase(et, k, g, colonnes, neuves, gestes) {
  const de = colonnes.indexOf(g.colonnes[0]) + 1;
  const a = colonnes.indexOf(g.colonnes[g.colonnes.length - 1]) + 2;
  const classe = classeDeCase(g);
  const fini = g.etat === "fait";
  const cote = g.colonnes[0] !== colonnes[0];
  const neuf = g.colonnes.some((c) => neuves.has(c));
  const ech = g.echeance && g.echeance.court;
  const titre = [et.nom, g.phrase, g.echeance && g.echeance.texte && `— ${g.echeance.texte}`, et.phrase]
    .filter(Boolean).join(" ");
  let bouton = "";
  if (!fini && g.action) {
    gestes.push(g.action);
    bouton = `data-geste="${gestes.length - 1}" tabindex="0" role="button"`;
  }
  const quoi = `<span class="quoi">${g.compte_texte
    ? `<span class="compte">${echapper(g.compte_texte)}</span> ` : ""}${echapper(g.quoi || "")}</span>`;
  const limite = ech ? `<span class="ech">${echapper(ech)}</span>` : "";
  const corps = g.coupee ? `
      <span class="moitie faite"><span class="etape-rond">✓</span>${cote ? "" : `<b>${echapper(et.nom)}</b>`}</span>
      <span class="moitie attend"><span class="kase-texte">${quoi}${limite}</span></span>` : `
      <span class="etape-rond">${fini ? "✓" : k + 1}</span>
      <span class="kase-texte">
        <span class="ligne1"><b>${echapper(et.nom)}</b></span>${fini ? "" : quoi}${limite}
      </span>`;
  return `<div class="kase ${classe}${cote ? " cote" : ""}${neuf ? " neuf" : ""}${bouton ? " cliquable" : ""}"
      data-k="${k}" style="grid-column:${de} / ${a}" title="${echapper(titre)}" ${bouton}>${corps}</div>`;
}

/* -------------------------------------------- le panneau des quatre étapes */
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
  const colonnes = e.colonnes && e.colonnes.length ? e.colonnes : ["G"];
  const vues = COLONNES_VUES.get(mois);
  const neuves = new Set(vues ? colonnes.filter((c) => !vues.includes(c)) : []);
  COLONNES_VUES.set(mois, colonnes);
  const gabarit = colonnes.map((c, i) => i === 0 ? "minmax(0, 1.4fr)" : "minmax(0, 1fr)").join(" ");
  const gestes = [];
  const rangees = (e.etapes || []).map((et, k) => {
    const cases = (et.groupes || []).map((g) => dessinerCase(et, k, g, colonnes, neuves, gestes)).join("");
    return `<li class="rangee"><div class="kases" style="grid-template-columns:${gabarit}">${cases}</div></li>`;
  }).join("");
  // Le geste second d'une étape (l'annule-et-remplace sans nécessité) : un lien sous le panneau, hors de l'arbre.
  const seconds = (e.etapes || []).filter((et) => et.second).map((et) => {
    gestes.push(et.second);
    return `<button class="lien etape-second" data-geste="${gestes.length - 1}">${echapper(et.second.libelle)}</button>`;
  }).join("");
  cible.innerHTML = `<ol class="etapes">${rangees}<svg class="arbre" aria-hidden="true"></svg></ol>${seconds}`;

  cible.querySelectorAll("[data-geste]").forEach((d) => {
    const ouvrir = (ev) => { ev.preventDefault(); ouvrirGeste(gestes[Number(d.dataset.geste)], mois); };
    d.addEventListener("click", ouvrir);
    if (d.getAttribute("role") === "button") {
      d.addEventListener("keydown", (ev) => { if (ev.key === "Enter" || ev.key === " ") ouvrir(ev); });
    }
  });
  dessinerArbre(cible.querySelector(".etapes"));
  if (!dessinerActions.observe) {
    // L'arbre suit la largeur du panneau (fenêtre redimensionnée, colonne qui réapparaît).
    dessinerActions.observe = new ResizeObserver(() => dessinerArbre(cible.querySelector(".etapes")));
    dessinerActions.observe.observe(cible);
  }
}

/* L'ARBRE : un trait droit relie une case d'une étape à une case de la suivante. Chaque case du haut a le sien vers le
 * bas si son milieu tombe dans une case ; chaque case du bas qu'aucun trait n'atteint a le sien vers le haut. Un trait
 * est orange si ses deux bouts restent à faire (le rouge compte comme l'orange, l'attente aussi), vert sinon. */
function dessinerArbre(liste) {
  const svg = liste && liste.querySelector(".arbre");
  if (!svg) return;
  const base = liste.getBoundingClientRect();
  if (!base.width) return;
  const rangs = [...new Set([...liste.querySelectorAll(".kase")].map((el) => el.dataset.k))];
  const boites = (k) => [...liste.querySelectorAll(`.kase[data-k="${k}"]`)].map((el) => {
    // Le trait d'une case coupée tombe dans sa moitié qui attend.
    const r = el.getBoundingClientRect();
    const attend = el.querySelector(".moitie.attend");
    const a = attend ? attend.getBoundingClientRect() : r;
    return { el, r, x: a.left + a.width / 2 };
  });
  const ouvert = (el) => !el.classList.contains("vert") && !el.classList.contains("gris");
  const couleur = (h, b) => ouvert(h) && ouvert(b) ? "orange" : "vert";
  const traits = [];
  for (let i = 0; i < rangs.length - 1; i++) {
    const dessus = boites(rangs[i]), dessous = boites(rangs[i + 1]);
    const dedans = (x, b) => x >= b.r.left && x <= b.r.right;
    dessous.forEach((d) => {
      const haut = dessus.find((h) => dedans(d.x, h));
      const deja = dessus.some((h) => dedans(h.x, d));
      if (haut && !deja) traits.push({ c: couleur(haut.el, d.el), x: d.x, y1: haut.r.bottom, y2: d.r.top });
    });
    dessus.forEach((h) => {
      const bas = dessous.find((d) => dedans(h.x, d));
      if (bas) traits.push({ c: couleur(h.el, bas.el), x: h.x, y1: h.r.bottom, y2: bas.r.top });
    });
  }
  const ordre = { vert: 1, orange: 2 };
  svg.innerHTML = traits.sort((a, b) => ordre[a.c] - ordre[b.c]).map((t) =>
    `<path class="${t.c}" d="M${t.x - base.left},${t.y1 - base.top} V${t.y2 - base.top}"/>`).join("");
}

/* Ce qu'il faut relire après avoir déposé ou envoyé : l'état du mois affiché,
 * et la liste des embauches, qu'un dépôt de DPAE vient de raccourcir. */
async function rafraichirActions() {
  const mois = document.getElementById("mois").value;
  await dessinerActions(mois);
  await dessinerEmbauches(mois);
  await dessinerEtatsDesEngagements(mois);
}

/* ---------------------------------------- le récapitulatif d'un dépôt */

/** CE QUE LA DSN DÉCLARE, contrat par contrat, LU DANS LE FICHIER
 *  (30/09/2026, l'éditeur : « ce que l'utilisateur veut voir et vérifier,
 *  c'est que tous les contrats sont dans la DSN »). Comparé aux contrats
 *  saisis du mois : un contrat absent du fichier, ou que l'engagement dit
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
      ${pastilleDeLEtat(c.pastille_ecarts)}</td></tr>` : ""}`).join("")).join("");
  return `
    <div class="encart ${ENCART_DE_COULEUR[r.couleur] ?? ""}">
      <b>${r.contrats} contrat${r.contrats > 1 ? "s" : ""} dans ce fichier${r.corrections
        ? ` (et ${r.corrections} correction${r.corrections > 1 ? "s" : ""} d'un mois déjà déclaré)` : ""}${r.compare
        ? `, ${r.contrats_saisis} saisi${r.contrats_saisis > 1 ? "s" : ""} dans la paie du mois` : ""}</b>
      ${r.conforme ? "Tous les contrats de la paie sont dans la DSN, tels que la paie les dit."
        : r.compare ? `${r.absents.length ? r.absents.length + " contrat(s) de la paie manquent au fichier. " : ""}${ecarts
          ? ecarts + " contrat(s) du fichier ne disent pas ce que dit la paie : réécrire le mois." : ""}`
        : "Pas de paie saisie à comparer : le fichier se lit seul."}
    </div>
    ${r.absents.length ? `<ul class="absents">${r.absents.map((x) =>
      `<li>${pastilleDeLEtat(x.pastille)} ${echapper(x.nom)} — contrat
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
      pas bloqué. Mais elle prélève le plus souvent <em>plus</em> que le taux
      réel du salarié, qui ne récupère la différence qu'à sa déclaration de
      revenus l'année suivante. Relever les retours avant de déposer peut
      encore changer cela.</div>` : ""}
    ${recapitulatifDsn(a.recapitulatif)}
    <details class="replie">
      <summary>Ordres de télépaiement (${v.length}) — la conséquence de ce que
        la DSN déclare ; un dépôt réel les déclenche tous</summary>
      <div class="encart ${ENCART_DE_COULEUR[a.couleur_ordres] ?? ""}">
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
          <td>${pastilleDeLEtat(x.pastille)}</td>
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
  if (r.pas_avant) {
    // Ce que le moteur dit d'une DSN que la norme ne laisse pas encore partir.
    ouvrirBoite(`
      <h3>Déposer la DSN</h3>
      ${moisEnGros(r.mois)}
      <p>${echapper(r.pas_avant.phrase)}</p>
      <div class="actions"><button class="primaire" onclick="fermerBoite()">Fermer</button></div>`);
    return;
  }
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
        // LE TEST NE FERME PLUS LA BOÎTE : « ça a quitté la
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
    // « conforme » (le moteur refuse le réel sans lui, DSN comme DPAE).
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
        ecrire(`<div class="encart ${ENCART_DE_COULEUR[r.couleur] ?? ""}"><b>Le test est refusé</b>
          ${echapper(motif)}
          Le même refus attendrait le dépôt réel : c'est cela qu'il faut corriger.</div>`);
        libre();
        if (!ici()) dire(`Le test de ${nom.toLowerCase()} est refusé : ${motif}`, r.couleur);
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
  // LE TEST D'ABORD, TOUJOURS : on propose d'abord un test, puis le réel. Le
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
    // LE MOIS QU'ON VIENT DE FERMER PART EN TEST TOUT SEUL :
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
  for (let tour = 0; tour < 40; tour++) {
    await new Promise((ok) => setTimeout(ok, enFictive() ? 4000 : 15000));
    let r = null;
    try { r = await api("suivre-depot", { quoi, flux, mois }); } catch (e) { continue; }
    if (r && r.fini) {
      dire(r.phrase, r.couleur);
      await rafraichirActions();
      return;
    }
  }
}

/** Où en est la répétition de CE contenu — avant de proposer le dépôt réel.
 *  Un bouton qui refusera au clic est un bouton qui ment. */
/** Les embauches que porte une DPAE, nommées : un seul dépôt peut en porter
 *  plusieurs, et l'écran dit pour qui chacune est partie. */
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
      ${pastilleDeLEtat(x.pastille)}</li>`).join("")}</ul>`;
}

function encartRepetition(r, quoi, a) {
  if (!r) return "";
  const cette = (quoi === "dsn" ? "Cette DSN" : "Cette DPAE")
    + (quoi === "dsn" ? "" : echapper(embauchesDe(a)));
  // Les deux phrases de l'éditeur : « cette DPAE est déjà
  // testée, voulez-vous l'envoyer » — ou « elle est déjà envoyée en test,
  // on attend les retours, qui arrivent bientôt ».
  if (r.conforme && r.compte_rendu === "ok") {
    return `<div class="encart ${ENCART_DE_COULEUR[r.couleur] ?? ""}">
      <b>${cette} est testée — conforme. Voulez-vous l'envoyer ?</b>
      Testée le ${echapper(quand(r.conforme.quand))} : le même contenu est
      passé au bac à sable sans anomalie. ${enFictive() ? "L'envoi, simulé, est le bouton « Envoyer — simulé »." : "Le dépôt réel est le bouton « Envoyer le réel »."}
      ${certificatParPersonne(r.conforme)}</div>`;
  }
  if (r.conforme) {
    return `<div class="encart ${ENCART_DE_COULEUR[r.couleur] ?? ""}">
      <b>${cette} est en test — son compte rendu est attendu</b>
      Envoyée en test le ${echapper(quand(r.conforme.quand))}. ${enFictive() ? "L'envoi simulé se" : "Le dépôt réel se"}
      fait ici, une fois ce compte rendu revenu conforme.</div>`;
  }
  // Jamais essayé : ce n'est pas une mise en garde, c'est l'étape suivante
  // (26/09/2026 : l'orange se garde pour ce qui demande de corriger).
  const rate = (r.ratees || []).length || (r.perimees || []).length;
  return `<div class="encart ${ENCART_DE_COULEUR[r.couleur] ?? ""}">
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
          <td>${echapper(m.nom)}${m.pastille ? `<br>${pastilleDeLEtat(m.pastille)}` : ""}</td>
          ${a.boite_liee ? `<td>${echapper(m.destinataire)}</td>` : ""}
          <td class="pieces">${m.pieces.map((x) => `<div>${echapper(x)}</div>`).join("")}</td>
        </tr>
        ${m.renvoi ? `<tr><td></td><td colspan="${a.boite_liee ? 3 : 2}" class="discret">Ce qui a changé :
          <ul>${m.changements.map((c) => `<li>${echapper(c)}</li>`).join("")}</ul></td></tr>` : ""}`).join("")}
    </table>`}
    ${a.deja ? `<div class="encart attention"><b>Déjà partis</b>
      Le ${echapper(quand(a.deja.quand))}.</div>` : ""}
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
      await ouvrirImpression(mois, r.impression, "Documents de " + r.salaries.length + " salarié(s) à imprimer.");
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

/** LE PDF À IMPRIMER s'ouvre : le moteur l'a écrit (`impression`), la fenêtre l'ouvre,
 *  et le message dit ce qui est à imprimer (`quoi`) et où est le PDF (la phrase du moteur). */
async function ouvrirImpression(mois, fichier, quoi) {
  try {
    const r = await api("ouvrir-piece", { impression: mois, fichier });
    dire([quoi, r && r.phrase].filter(Boolean).join(" "), "envoye");
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
                 data-debut="${echapper(c.debut)}" data-numero="${echapper(c.numero || "")}" ${c.en_avenant ? 'data-avenant="1"' : ""}
                 ${coche(c) ? "checked" : ""}>
          <span><b>${echapper(c.nom)}</b> — ${echapper(c.spectacle)},
            ${echapper(periodeEnClair(c.debut, c.fin))}
            ${c.numero ? `<span class="discret">(contrat n° ${echapper(c.numero)})</span>` : ""}
            ${a.boite_liee ? (c.destinataire ? `<br><span class="discret">${echapper(c.destinataire)}</span>`
              : `<br><span class="s-courriel">pas de courriel dans sa fiche :
                  <input type="email" class="s-courriel-saisi" placeholder="son courriel" data-salarie="${echapper(c.salarie)}">
                  <button type="button" class="lien s-courriel-poser" data-salarie="${echapper(c.salarie)}">Enregistrer</button></span>`) : ""}
            ${c.pastille ? `<br>${pastilleDeLEtat(c.pastille)}` : ""}${c.en_avenant ? `
              <button class="lien s-avenant" data-salarie="${echapper(c.salarie)}"
                data-debut="${echapper(c.debut)}" data-numero="${echapper(c.numero || "")}">voir l'avenant</button>` : ""}</span>
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
    const coches = [...document.querySelectorAll(".s-qui:checked")];
    if (!coches.length) { dire("Aucun contrat coché."); return; }
    // Un contrat signé qui a changé s'imprime en avenant, dans le même PDF.
    const choisis = coches.filter((c) => !c.dataset.avenant)
      .map((c) => ({ salarie: c.value, debut: c.dataset.debut, numero: c.dataset.numero }));
    const avenants = coches.filter((c) => c.dataset.avenant)
      .map((c) => ({ salarie: c.value, debut: c.dataset.debut, numero: c.dataset.numero, mois }));
    const rendre = occupe(imprimer, "Impression…");
    try {
      const r = await api("contrats-imprimer", { mois, contrats: choisis, avenants });
      fermerBoite();
      await ouvrirImpression(mois, r.impression, "À imprimer : " + [...r.contrats.map((c) => "contrat n° " + c.numero),
        ...r.avenants.map((c) => "avenant n° " + c.avenant + " au contrat n° " + c.numero)].join(", ") + ".");
      await rafraichirActions();
    } catch (e) {
      rendre();
      dire(e.message, "erreur");
    }
  };
  // LE COURRIEL QUI MANQUE se saisit ici : il s'enregistre dans la fiche du
  // salarié (`poser-courriel`), et la boîte se rouvre avec lui.
  document.querySelectorAll(".s-courriel-poser").forEach((b) => {
    b.onclick = async (ev) => {
      ev.preventDefault();                 // le bouton est dans un <label>
      const champ = document.querySelector(`.s-courriel-saisi[data-salarie="${CSS.escape(b.dataset.salarie)}"]`);
      try {
        await api("poser-courriel", { salarie: b.dataset.salarie, email: champ ? champ.value : "" });
      } catch (e) { dire(e.message, "erreur"); return; }
      ETAT = await api("etat");
      const gardes = [...document.querySelectorAll(".s-qui:checked")].map((c) => c.value);
      fermerBoite();
      dire("Courriel enregistré dans la fiche.", "envoye");
      boiteSignature(mois, gardes);
    };
  });
  document.querySelectorAll(".s-courriel-saisi").forEach((c) => {
    c.onclick = (ev) => ev.preventDefault();   // dans un <label> : ne pas cocher la case
  });
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

/** LES CONTRATS EN ATTENTE : un contrat ou un avenant parti, pas revenu
 *  signé. Le moteur dit, pour chacun, ce qui est parti et quand (`attente`),
 *  et ses gestes, route et charge : « Reçu signé » (`recu`, pour tous : ce
 *  qui part par courriel peut revenir signé sur papier) et « Relancer »
 *  (`relancer`, parti par courriel, boîte posée). Une note se défait : le
 *  moteur rend de quoi l'annuler (`annuler`) et la phrase qui la dit. */
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
            <br>${pastilleDeLEtat(c.pastille)}</span>
          <span class="gestes-contrat">
            <button class="primaire petit a-recu" data-i="${i}">Reçu signé</button>
            ${c.relancer ? `<button class="secondaire petit a-relancer" data-i="${i}"
              title="Renvoie le même document, avec un mot de rappel">Relancer</button>` : ""}
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
        const r = await api(c.recu.route, c.recu.charge);
        await rafraichirActions();
        boiteContratsEnAttente(mois, { texte: r.phrase, annuler: r.annuler });
      } catch (e) { rendre(); dire(e.message, "erreur"); }
    };
  });
  document.querySelectorAll(".a-relancer").forEach((b) => {
    b.onclick = async () => {
      const c = contrats[Number(b.dataset.i)];
      const mdp = await demanderMotDePasse(c.relancer.pourquoi, { envoi: true });
      if (!mdp) return;
      const rendre = occupe(b, "Relance…");
      try {
        const r = await api(c.relancer.route, { ...c.relancer.charge, motdepasse: mdp });
        fermerBoite();
        dire("Relance partie : " + r.partis.join(", ") + ".", "envoye");
        await rafraichirActions();
      } catch (e) { rendre(); dire(e.message, "erreur"); }
    };
  });
}

/** L'AVENANT. Le contrat parti se lit (son PDF), l'engagement
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
      <div class="encart">Le contrat parti se compare à l'engagement d'aujourd'hui :
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
    : `<div class="encart">Le contrat parti dit déjà tout ce que dit l'engagement :
        il n'y a rien à modifier.</div>`}
    ${mods.length && a.remplacable && a.boite_liee ? `<p class="discret">Le contrat n'a pas commencé : il peut aussi être
      refait en entier. Le nouveau porte en tête « annule et remplace le contrat
      n° ${echapper(a.contrat || "")} du ${echapper(jjmmaaaa(a.date_contrat))} ».</p>` : ""}
    <p class="discret"><a href="#" id="av-pieces">Voir ce qui est parti (contrat envoyé, signé, preuve)</a></p>
    <div class="actions">
      <button class="secondaire" onclick="fermerBoite()">Fermer</button>
      ${mods.length && a.remplacable && a.boite_liee ? `<button class="secondaire" id="av-remplace">${enFictive() ? "Contrat qui annule et remplace — simulé…" : "Contrat qui annule et remplace…"}</button>` : ""}
      ${mods.length && a.boite_liee ? `<button class="primaire" id="av-envoyer">${enFictive() ? "Envoyer l'avenant à la signature — simulé…" : "Envoyer l'avenant à la signature…"}</button>` : ""}
    </div>`);
  document.getElementById("av-pieces").onclick = (ev) => {
    ev.preventDefault();
    fermerBoite();
    boitePieces({ salarie, debut, numero, nom: a.nom, fin: a.fin, spectacle: a.spectacle,
                }, "contrat-signature");
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


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
    case "deposer_dsn": return boiteDepot({ quoi: "dsn", mois: m }, { testerDabord: true });
    case "deposer": return boiteDepot(action.charge);
    case "envoyer_documents": return boiteEnvoi(m);
    case "historique": return ouvrirMoisHistorique(m);
    case "dossier": return ouvrirEtDire(action.chemin, action.phrase, true);
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
  const titre = [g.phrase || et.nom, et.phrase].filter(Boolean).join(" — ");
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
        <span class="ligne1"><b>${echapper(et.nom)}</b></span>${fini && !g.quoi ? "" : quoi}${limite}
      </span>`;
  return `<div class="kase ${classe}${cote ? " cote" : ""}${neuf ? " neuf" : ""}${bouton ? " cliquable" : ""}"
      data-k="${k}" style="grid-column:${de} / ${a}" title="${echapper(titre)}" ${bouton}>${corps}</div>`;
}

/* -------------------------------------------- le panneau des quatre étapes */
async function dessinerActions(mois) {
  const cible = document.getElementById("actions-mois");
  if (!mois) { cible.innerHTML = ""; return; }
  cible.innerHTML = `<p class="discret">Lecture…</p>`;
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
  cible.innerHTML = `<ol class="etapes">${rangees}<svg class="arbre" aria-hidden="true"></svg></ol>`;

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

/* Ce qu'il faut relire après avoir déposé ou envoyé : l'état du mois affiché, l'état de chaque engagement, et ses
 * chiffres (un acte peut refaire une paie : le taux de PAS revenu avec la DPAE), lus dans la même lecture du mois.
 * Les chiffres ne s'attendent pas : appelée pendant un enregistrement, l'aperçu prend son tour après lui. */
async function rafraichirActions() {
  const mois = document.getElementById("mois").value;
  await dessinerActions(mois);
  await dessinerEtatsDesEngagements(mois);
  if (mois && toutEstEcrit()) lireLeMois(mois).then((lu) => calculerApercu(lu.lectures.apercu)).catch(() => {});
}

/* ---------------------------------------- le récapitulatif d'un dépôt */

/** CE QUE LA DSN DÉCLARE, contrat par contrat, LU DANS LE FICHIER
 *  (ce que l'utilisateur veut voir et vérifier : que tous les contrats sont dans la DSN). Comparé aux contrats
 *  saisis du mois : un contrat absent du fichier, ou que l'engagement dit
 *  autrement, se dit en tête. */
function recapitulatifDsn(r) {
  if (!r) return "";
  if (r.erreur) return `<div class="encart attention"><b>Récapitulatif illisible</b>
    ${echapper(r.erreur)}</div>`;
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
      <b>${echapper(r.titre)}</b> ${echapper(r.phrase)}
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
    <p class="discret">${echapper(a.description)}</p>
    ${recapitulatifDsn(a.recapitulatif)}
    <details class="replie">
      <summary>Ordres de télépaiement (${v.length})</summary>
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
           </table>` : ""}
      ${echapper(a.ordres_phrase)}
      </div>
    </details>`;
}

function corpsDpae(a) {
  return `
    <p class="discret">${echapper(a.description)}</p>
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
    </table>`;
}

/** LA DÉCLARATION SE DÉSIGNE, ELLE NE SE NOMME PAS PAR SON FICHIER : `charge`
 *  est ce que le moteur rend pour elle (`{quoi, mois, etablissement?,
 *  engagements?}`, ou le signalement écrit). Le moteur l'écrit ou la reprend
 *  (`depot-apercu`), et ce qu'il en dit se lit sous la barre. */
async function boiteDepot(charge, { testerDabord = false } = {}) {
  const quoi = charge.quoi;
  let a;
  try {
    a = await api("depot-apercu", charge);
  } catch (e) { dire(e.message, "erreur"); return; }
  const dit = [a.phrase, ...(a.avertissements || []), ...(a.informations || [])].filter(Boolean);
  if (dit.length) dire(dit.join("  ·  "));
  rafraichirActions();

  ouvrirBoite(`
    <h3>Déposer ${quoi === "dsn" ? "la DSN" : "la DPAE"}</h3>
    ${moisEnGros(a.mois)}
    ${quoi === "dsn" ? corpsDsn(a) : corpsDpae(a)}
    ${encarts(a.encarts)}
    <div id="d-bilan">${encartRepetition(a.repetition)}</div>
    ${noteFictive()}
    <div class="actions">
      <button class="secondaire" onclick="fermerBoite()">Annuler</button>
      <button class="primaire" id="d-test">Envoyer le test</button>
      <button class="secondaire envoi" id="d-reel" disabled>Envoyer le réel</button>
    </div>
    ${a.reel ? `<p class="discret"><button type="button" class="lien envoi" id="d-sans-test">envoyer sans test</button></p>` : ""}`);

  // Le mot de passe tapé pour le dépôt sert au renvoi (boiteRenvoiDsn) :
  // on ne le redemande pas. Une erreur l'oublie, un refus aussi.
  let mdpConnu = null;
  const lancer = async (reel, forcer = false, contournement = "") => {
    let mdp = "";
    if (reel) {
      mdp = mdpConnu || await demanderMotDePasse(quoi === "dsn" ? "Déposer la DSN" : "Déposer la DPAE", { envoi: true });
      if (!mdp) return;
      mdpConnu = mdp;
    }
    document.getElementById("d-test").disabled = true;
    document.getElementById("d-reel").disabled = true;
    const rendre = occupe(document.getElementById(reel ? "d-reel" : "d-test"),
                          reel ? "Dépôt…" : "Envoi…");
    try {
      const r = await api("deposer", { ...charge, reel, motdepasse: mdp,
                                       forcer, contournement });
      rendre();
      if (!reel) {
        // LE TEST NE FERME PLUS LA BOÎTE : « ça a quitté la
        // fenêtre au lieu de rester et d'attendre les retours pour proposer
        // d'envoyer direct ». On reste, on attend le compte rendu, et le
        // bouton du réel s'allume ici même.
        dire(r.phrase, "envoye");
        attendreBilan();
        rafraichirActions();
        return;
      }
      fermerBoite();
      dire(r.phrase, "envoye");
      if (r.avis) avisApresLeDepot(r);
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
        return boiteRepetition(e.message,
                               () => lancer(false),
                               (motif) => lancer(true, false, motif));
      }
      if (e.detail?.type === "DsnDejaDeposee") {
        // « On attend le retour, et si vraiment on y tient, après un
        // avertissement, on peut renvoyer ». Le mot de
        // passe déjà tapé sert au renvoi : on ne le redemande pas.
        return boiteRenvoiDsn(e.message, () => lancer(true, true));
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
    b.className = (principal ? "primaire" : "secondaire") + " envoi";
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
    // Seul le bilan du test (le certificat, ou le bilan d'anomalies) ouvre le
    // réel ou le ferme ; l'accusé d'enregistrement dit « envoi reçu », pas
    // « conforme » (le moteur refuse le réel sans lui, DSN comme DPAE).
    for (let tour = 0; tour < 30; tour++) {
      let r = null;
      try { r = await api("bilan-repetition", charge); } catch (e) { /* on réessaie */ }
      if (r && (r.etat === "testee" || r.etat === "test_refuse")) {
        ecrire(encartRepetition(r));
        libre();
        if (!ici()) dire(`${r.titre} · ${r.phrase}`, r.couleur);
        else if (r.etat === "testee") boutonReel(true);
        rafraichirActions();
        return;
      }
      if (r) ecrire(encartRepetition(r) + (r.incident ? `<p class="discret">${echapper(r.incident)}</p>` : ""));
      await new Promise((ok) => setTimeout(ok, enFictive() ? 3000 : 10000));
    }
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
  if (rep && rep.etat === "testee") boutonReel(true);
  else if (rep && rep.etat === "en_test") attendreBilan();
  else {
    // Pas de test pour ce contenu : c'est lui qui vient d'abord.
    document.getElementById("d-test").className = "primaire";
    document.getElementById("d-reel").className = "secondaire envoi";
    // LE MOIS QU'ON VIENT DE FERMER PART EN TEST TOUT SEUL :
    // « on produit la DSN de test complète et on attend le retour du test,
    // ensuite on propose d'envoyer la DSN réelle ». Un test n'engage rien.
    if (testerDabord) lancer(false);
  }
}

/** APRÈS LE DÉPÔT RÉEL, L'AVIS QUE LE MOTEUR REND (`avis` de `deposer`) : il le rend quand il y a quelque chose à dire
 *  (les documents des salariés d'une DSN qui en a). */
function avisApresLeDepot(r) {
  ouvrirBoite(`
    <h3>DSN envoyée</h3>
    ${moisEnGros(r.mois)}
    ${encart(r.avis)}
    <div class="actions">
      <button class="primaire" onclick="fermerBoite()">Compris</button>
    </div>`);
}

/** L'ÉCOUTE DES RETOURS, JUSTE APRÈS UN DÉPÔT RÉEL. Toutes les quinze secondes,
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

/** Ce que le certificat de conformité (nature 41) dit de chaque personne. */
function certificatParPersonne(ligne) {
  const dpae = ((ligne && ligne.retours) || []).flatMap((x) => x.dpae || []);
  if (!dpae.length) return "";
  return `<ul class="liste-certificat">${dpae.map((x) => `
    <li>${echapper([x.nom, x.prenom].filter(Boolean).join(" "))} :
      ${pastilleDeLEtat(x.pastille)}</li>`).join("")}</ul>`;
}

/** Où en est le test de CE contenu, tel que le moteur le dit (`titre`, `phrase`, `couleur` de la répétition). */
function encartRepetition(r) {
  if (!r) return "";
  return `<div class="encart ${ENCART_DE_COULEUR[r.couleur] ?? ""}"><b>${echapper(r.titre)}</b>
    ${echapper(r.phrase)}${r.etat === "testee" ? certificatParPersonne(r.conforme) : ""}</div>`;
}

/** Le verrou test → réel : le refus du moteur (`RepetitionManquante`) dit pourquoi et la sortie de secours. Deux
 *  issues, et la première est la bonne : faire la répétition. La seconde part avec un motif écrit, que le moteur exige
 *  et inscrit au journal des dépôts. */
function boiteRepetition(phrase, repeter, quandMeme) {
  ouvrirBoite(`
    <h3>Sans dépôt de test</h3>
    ${encart({ couleur: "orange", phrase })}
    <div class="actions">
      <button class="secondaire" onclick="fermerBoite()">Annuler</button>
      <button class="primaire" id="r-tester">Déposer en test</button>
    </div>
    <p class="note"><a href="#" id="r-outre">Déposer sans répétition</a></p>
    <div id="r-motif" class="cachee">
      <label>Le motif <textarea id="r-raison" rows="2"></textarea></label>
      <div class="actions">
        <button class="secondaire envoi" id="r-quand-meme">Déposer sans répétition</button>
      </div>
    </div>`);
  document.getElementById("r-tester").onclick = () => { fermerBoite(); repeter(); };
  document.getElementById("r-outre").onclick = (e) => {
    e.preventDefault();
    document.getElementById("r-motif").classList.remove("cachee");
    document.getElementById("r-raison").focus();
  };
  document.getElementById("r-quand-meme").onclick = () => {
    fermerBoite();
    quandMeme(document.getElementById("r-raison").value.trim());
  };
}

/** Une DSN réelle est déjà partie pour ce mois : le refus du moteur (`DsnDejaDeposee`) dit où elle en est ; on ne
 *  renvoie que si on le veut vraiment. */
function boiteRenvoiDsn(phrase, renvoyer) {
  ouvrirBoite(`
    <h3>DSN déjà partie</h3>
    ${encart({ couleur: "orange", phrase })}
    <div class="actions">
      <button class="primaire" onclick="fermerBoite()">Ne pas renvoyer</button>
      <button class="secondaire envoi" id="d-renvoyer">Renvoyer quand même</button>
    </div>`);
  document.getElementById("d-renvoyer").onclick = () => { fermerBoite(); renvoyer(); };
}

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
    ${a.boite_liee && a.expediteur ? `<p class="discret">Courriels depuis ${echapper(a.expediteur)}</p>` : ""}
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
          <td class="pieces">${m.sortes.map((x) => `<div>${echapper(x)}</div>`).join("")}</td>
        </tr>
        ${m.renvoi ? `<tr><td></td><td colspan="${a.boite_liee ? 3 : 2}" class="discret">Ce qui a changé
          <ul>${m.changements.map((c) => `<li>${echapper(c)}</li>`).join("")}</ul></td></tr>` : ""}`).join("")}
    </table>`}
    ${a.deja ? `<div class="encart attention"><b>Déjà partis</b>
      Le ${echapper(quand(a.deja.quand))}.</div>` : ""}
    ${peut && a.boite_liee ? noteFictive() : ""}
    <div class="actions">
      <button class="secondaire" onclick="fermerBoite()">Fermer</button>
      ${peut ? `<button class="${a.boite_liee ? "secondaire" : "primaire"}" id="e-imprimer"
        >Imprimer</button>` : ""}
      ${peut && a.boite_liee ? `<button class="primaire envoi" id="e-envoyer">Envoyer par courriel</button>` : ""}
    </div>`);

  const coches = () => [...document.querySelectorAll(".e-qui:checked")].map((c) => c.value);
  const imprimer = document.getElementById("e-imprimer");
  if (imprimer) imprimer.onclick = async () => {
    const salaries = coches();
    const rendre = occupe(imprimer, "Impression…");
    try {
      const r = await api("documents-imprimer", { mois, salaries });
      fermerBoite();
      await ouvrirEtDire(r.chemin, r.phrase);
      await rafraichirActions();
    } catch (e) {
      rendre();
      dire(e.message, "erreur");
    }
  };
  const envoyer = document.getElementById("e-envoyer");
  if (envoyer) envoyer.onclick = async () => {
    const salaries = coches();
    const mdp = await demanderMotDePasse("Envoyer les documents", { envoi: true });
    if (!mdp) return;
    const rendre = occupe(envoyer, "Envoi…");
    const repere = await repereCourrielsFictifs();
    try {
      const r = await api("envoyer-documents", { mois, salaries, motdepasse: mdp });
      fermerBoite();
      dire(r.phrase, "envoye");
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
    ${encart(a.signature)}
    ${a.boite_liee ? noteFictive() : ""}
    ${(a.avertissements || []).map((x) => `<div class="encart info">${echapper(x)}</div>`).join("")}
    ${!contrats.length ? `<p class="discret">${echapper(a.rien_a_envoyer)}</p>` : `<div class="liste-choix">
      ${contrats.map((c) => `
        <label class="choix">
          <input type="checkbox" class="s-qui" value="${echapper(c.salarie)}"
                 data-debut="${echapper(c.debut)}" data-numero="${echapper(c.numero || "")}" ${c.en_avenant ? 'data-avenant="1"' : ""}
                 ${coche(c) ? "checked" : ""}>
          <span><b>${echapper(c.nom)}</b> — ${echapper(c.spectacle)},
            ${echapper(periodeEnClair(c.debut, c.fin))}
            ${c.numero ? `<span class="discret">(contrat n° ${echapper(c.numero)})</span>` : ""}
            ${a.boite_liee ? (c.destinataire ? `<br><span class="discret">${echapper(c.destinataire)}</span>`
              : `<br><span class="s-courriel">Son courriel
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
        >Imprimer</button>` : ""}
      ${contrats.length && a.boite_liee ? `<button class="primaire envoi" id="s-envoyer">Envoyer à la signature</button>` : ""}
    </div>`);

  // IMPRIMER : les contrats partent à la main, sous leur numéro, et le
  // journal le note ; le PDF de tous les contrats cochés s'ouvre.
  const imprimer = document.getElementById("s-imprimer");
  if (imprimer) imprimer.onclick = async () => {
    const coches = [...document.querySelectorAll(".s-qui:checked")];
    // Un contrat signé qui a changé s'imprime en avenant, dans le même PDF.
    const choisis = coches.filter((c) => !c.dataset.avenant)
      .map((c) => ({ salarie: c.value, debut: c.dataset.debut, numero: c.dataset.numero }));
    const avenants = coches.filter((c) => c.dataset.avenant)
      .map((c) => ({ salarie: c.value, debut: c.dataset.debut, numero: c.dataset.numero, mois }));
    const rendre = occupe(imprimer, "Impression…");
    try {
      const r = await api("contrats-imprimer", { mois, contrats: choisis, avenants });
      fermerBoite();
      await ouvrirEtDire(r.chemin, r.phrase);
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
        var pose = await api("poser-courriel", { salarie: b.dataset.salarie, email: champ ? champ.value : "" });
      } catch (e) { dire(e.message, "erreur"); return; }
      await relireEtat();
      const gardes = [...document.querySelectorAll(".s-qui:checked")].map((c) => c.value);
      fermerBoite();
      dire(pose.phrase, "envoye");
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
    const mdp = await demanderMotDePasse("Envoyer à la signature", { envoi: true });
    if (!mdp) return;
    const rendre = occupe(envoyer, "Envoi…");
    const repere = await repereCourrielsFictifs();
    try {
      const r = await api("demander-signature", { mois, contrats: choisis, avenants, motdepasse: mdp });
      fermerBoite();
      dire(r.phrase, "envoye");
      afficherEnvoi("Contrats à signer", mois, [...new Set(r.partis)],
                    [...(r.avertissements || []), ...r.contrats.map((c) => c.fichier)]);
      if (enFictive()) {
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
 *  signé. Le moteur dit, pour chacun, où il en est (`pastille`),
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
    ${!contrats.length ? `<p class="discret">${echapper(a.rien_en_attente)}</p>` : `<div class="liste-choix">
      ${contrats.map((c, i) => `
        <div class="choix">
          <span><b>${echapper(c.nom)}</b> — ${echapper(c.spectacle)},
            ${echapper(periodeEnClair(c.debut, c.fin))}
            ${c.numero ? `<span class="discret">(contrat n° ${echapper(c.numero)})</span>` : ""}
            <br>${pastilleDeLEtat(c.pastille)}</span>
          <span class="gestes-contrat">
            <button class="primaire petit a-recu" data-i="${i}">Reçu signé</button>
            ${c.relancer ? `<button class="secondaire petit a-relancer" data-i="${i}"
              >Relancer</button>` : ""}
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
        dire(r.phrase, "envoye");
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
    <h3>${echapper(a.titre)}</h3>
    <p class="discret">${echapper(a.situation)}</p>
    ${(a.avenants || []).map((x) => `<p class="discret">${echapper(x.phrase)}</p>`).join("")}
    ${encarts(a.encarts)}
    ${mods.length ? `
      ${mods.map((m) => m.phrase ? `<div class="modif-avenant">
        <b>${echapper(m.article)}</b>
        <p>${echapper(m.phrase)}</p>
      </div>` : `<div class="modif-avenant">
        <b>Article ${m.numero} — ${echapper(m.article)}</b>
        <div class="discret">Avant</div>${cite(m.avant)}
        <div class="discret">Désormais</div>${cite(m.apres)}
      </div>`).join("")}
      <label>Date d'effet de l'avenant
        <input type="date" id="av-effet" value="${echapper(a.effet)}"></label>` : ""}
    ${a.contrat_signe ? `<div class="encart info">${echapper(a.contrat_signe.phrase)}
      <button class="lien" id="av-dossier">Ouvrir son dossier</button></div>` : ""}
    <p class="discret"><a href="#" id="av-pieces">Ce qui est parti</a></p>
    <div class="actions">
      <button class="secondaire" onclick="fermerBoite()">Fermer</button>
      ${mods.length && a.remplacable && a.boite_liee ? `<button class="secondaire envoi" id="av-remplace">Annule et remplace</button>` : ""}
      ${mods.length && a.boite_liee ? `<button class="primaire envoi" id="av-envoyer">Envoyer l'avenant</button>` : ""}
    </div>`);
  // LE CONTRAT SIGNÉ HORS DU LOGICIEL : la route rend le chemin de son dossier, la fenêtre l'ouvre (et le crée).
  const dossier = document.getElementById("av-dossier");
  if (dossier) dossier.onclick = (ev) => {
    ev.preventDefault();
    ouvrirEtDire(a.contrat_signe.chemin, "", true);
  };
  document.getElementById("av-pieces").onclick = (ev) => {
    ev.preventDefault();
    fermerBoite();
    boitePieces({ salarie, debut, numero, nom: a.nom, fin: a.fin, spectacle: a.spectacle,
                }, "contrat-signature");
  };
  const envoyer = document.getElementById("av-envoyer");
  if (envoyer) envoyer.onclick = async () => {
    const effet = document.getElementById("av-effet").value || a.effet;
    const mdp = await demanderMotDePasse("Envoyer l'avenant", { envoi: true });
    if (!mdp) return;
    const rendre = occupe(envoyer, "Envoi…");
    try {
      const r = await api("demander-signature", { mois, avenants: [{ salarie, debut, numero: numero || "", mois, effet }], motdepasse: mdp });
      fermerBoite();
      dire(r.phrase, "envoye");
      afficherEnvoi("Avenant à signer", a.mois, r.partis, r.contrats.map((c) => c.fichier));
      await rafraichirActions();
    } catch (e) {
      rendre();
      dire(e.message, "erreur");
    }
  };
  const remplace = document.getElementById("av-remplace");
  if (remplace) remplace.onclick = async () => {
    const mdp = await demanderMotDePasse("Annule et remplace", { envoi: true });
    if (!mdp) return;
    const rendre = occupe(remplace, "Envoi…");
    try {
      const r = await api("demander-signature", { mois: a.mois, contrats: [{ salarie, debut, numero: numero || a.contrat || "" }],
                                                  motdepasse: mdp, remplace: true });
      fermerBoite();
      dire(r.phrase, "envoye");
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


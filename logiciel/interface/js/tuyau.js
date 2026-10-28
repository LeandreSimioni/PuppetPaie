/* L'interface : elle affiche et elle demande, elle ne calcule jamais.
 *
 * Tout chiffre visible ici vient du moteur, par /api. Pas un brut, pas un
 * net, pas un plafond n'est recalcule en JavaScript -- c'est la meme regle
 * que pour Claude : le calcul est du code Rust teste et cale au centime,
 * et deux calculateurs, c'est deux resultats le jour ou l'un derive.
 *
 * Les erreurs metier arrivent avec un `type` : c'est sur lui qu'on branche,
 * jamais sur le texte du message.
 */
"use strict";

const JETON = document.body.dataset.jeton;
let ETAT = null;            // configuration + historique, rafraichis a la demande
let SAISIE = null;          // le mois en cours d'edition
let MINUTEUR = null;        // anti-rebond de l'apercu

/* ------------------------------------------------------------- le tuyau */

/* « Failed to fetch » : le serveur ne répond plus.
 *
 * C'est arrivé le 11/09/2026, et le message du navigateur n'apprenait rien —
 * une fiche en cours d'édition n'a pas été enregistrée, sans qu'on sache
 * pourquoi. La page survit à son serveur : elle a l'air de marcher, les
 * champs se remplissent, et seul l'enregistrement échoue.
 *
 * On le dit donc en toutes lettres, et on le dit de façon PERSISTANTE : un
 * message qui s'efface au bout de six secondes n'aide pas quelqu'un qui
 * était en train de taper. */
let SERVEUR_PERDU = false;

function serveurPerdu() {
  if (SERVEUR_PERDU) return;
  SERVEUR_PERDU = true;
  const zone = document.getElementById("veille");
  zone.hidden = false;
  zone.classList.add("souci");
  document.getElementById("veille-texte").innerHTML =
    "<b>Le serveur ne répond plus.</b> La fenêtre PuppetPaie a été fermée, ou le "
    + "terminal qui la faisait tourner. <b>Rien de ce qui est à l'écran n'est "
    + "enregistré.</b> Relancer PuppetPaie, puis recharger cette page.";
  document.getElementById("veille-maintenant").hidden = true;
}

/* L'ÉCRAN D'UN MOIS SE LIT D'UN COUP (Leandre, 09/10/2026 : « juste de la
 * lecture » ; PLAN-MOIS-INSTANTANE.md). Les lectures d'un mois (la saisie, les
 * contrats en cours, les états, les embauches, les fins, les imprévus,
 * l'aperçu) ne partent plus une à une au moteur : une
 * seule demande `mois` les rend toutes, gardées en mémoire
 * par la fenêtre elle-même, sans calcul. Le calcul s'est fait avant, quand
 * quelque chose a changé. Chaque lecture garde sa forme (résultat ou refus) :
 * les fonctions qui dessinent n'ont pas changé.
 *
 * Toute autre route peut écrire : ce qui a été lu part alors, avant ET après
 * elle — on n'affiche jamais un état qu'on aurait pu changer soi-même. */
const LECTURES_DU_MOIS = new Set(["saisie", "contrats-en-cours", "etat-du-mois",
  "embauches-a-declarer", "fins-de-contrat", "etat-par-engagement", "imprevus"]);
// Des lectures que la page fait souvent, sans rien changer : elles ne vident pas ce qui a été lu.
const ROUTES_SANS_EFFET = new Set(["veille", "rappels", "pour-demarrer", "organismes-contrats", "incident-navigateur", "diagnostic-etat",
  "verifier-fiche", "emploi", "signalements", "apercu", "importation-incident", "importation-tri", "arret-obligations",
  // Les imprévus d'un salarié (son dossier), les contrats qu'un imprévu suspendrait : des lectures.
  "imprevus", "arret-contrats",
  // Les contrats déclarés avant PuppetPaie (l'historique importé) : une lecture, que l'écran d'un mois passé fait en se peignant.
  "contrats-passes",
  // L'Historique d'un mois (ses documents, ses envois, leurs retours lus) et la pièce qu'on y ouvre : des lectures.
  "historique-mois", "ouvrir-piece",
  // Ce que la boîte « Supprimer cette compagnie » demande (ticket #236) : une lecture.
  "compagnie-suppression",
  // L'étape du relevé en cours, lue toutes les secondes le temps qu'il tourne (ticket #220).
  "veille-etape",
  // Les conseils « Le saviez-vous ? » (ticket 224) : le conseil du jour, ceux d'un salarié, le résumé de l'accueil.
  "conseil-du-jour", "conseils-salarie", "conseils-resume",
  // Les lectures de l'ouverture (ticket #244) : l'accueil, le mois à ouvrir, l'échéancier, et l'ouverture d'un coup.
  // Elles vidaient ce qui avait été lu du mois : l'écran, servi tout de suite, attendait alors l'état frais (~1 s).
  "etat", "mois-a-ouvrir", "echeancier", "ouverture"]);
let LU = null;            // { mois, lectures, perime } : la dernière lecture `mois`
let LU_EN_COURS = null;   // { mois, promesse, perimeAdmis } : celle qui est en route
let LU_GENERATION = 0;

/** Une réponse du moteur, telle que `appelerMoteur` la rend : son résultat, ou son refus levé. */
function lireReponse(d) {
  if (!d) throw new Error("Lecture absente de l'état du mois.");
  if (!d.ok) { const e = new Error(d.erreur.message); e.detail = d.erreur; throw e; }
  return d.resultat;
}

/** Tout ce que l'écran de `mois` lit, en une demande (partagée si elle est déjà en route).
 *  `perimeAdmis` (le changement de mois, ticket #212) : le dernier état connu, même périmé — il arrive tout de suite,
 *  marqué `perime`, et `verifierLeMois` le remplace par l'état frais. Sans lui (une relecture après une écriture) :
 *  l'état frais seulement. */
function lireLeMois(mois, perimeAdmis = false) {
  if (LU && LU.mois === mois) return Promise.resolve(LU);
  if (LU_EN_COURS && LU_EN_COURS.mois === mois && (perimeAdmis || !LU_EN_COURS.perimeAdmis)) return LU_EN_COURS.promesse;
  const generation = LU_GENERATION;
  const promesse = appelerMoteur("mois", perimeAdmis ? { mois } : { mois, attendre: true }).then((r) => {
    const lu = { mois, lectures: r.lectures || {}, perime: !!r.perime };
    if (generation === LU_GENERATION) LU = lu;
    if (LU_EN_COURS && LU_EN_COURS.promesse === promesse) LU_EN_COURS = null;
    return lu;
  }, (e) => {
    if (LU_EN_COURS && LU_EN_COURS.promesse === promesse) LU_EN_COURS = null;
    throw e;
  });
  LU_EN_COURS = { mois, promesse, perimeAdmis };
  return promesse;
}

function oublierLeMois() { LU = null; LU_EN_COURS = null; LU_GENERATION++; }

/** L'écran de `mois` arrivé avec l'ouverture (ticket #244, js/demarrage.js) : c'est la lecture `mois` que la page
 *  aurait faite, servie de même (dernier état connu, `perime` s'il l'est). */
function poserLeMois(mois, ecran) {
  LU = { mois, lectures: (ecran && ecran.lectures) || {}, perime: !!(ecran && ecran.perime) };
  LU_EN_COURS = null;
}

/* L'ÉTAT AFFICHÉ SE VÉRIFIE DERRIÈRE (ticket #212, Leandre, 16/10/2026 :
 * « quand je change de mois c'est pas instantané »). Un mois s'affiche avec
 * son dernier état connu, même périmé ; le moteur le refait en fond, et
 * `verifierLeMois` attend l'état frais (`attendre`) pour repeindre l'écran,
 * s'il a changé, sans toucher à une saisie en cours (`repeindreLeMois`). Tant
 * que ce n'est pas fait, la case du mois dit « mise à jour… », et les gestes
 * qui engagent attendent (`GESTES_QUI_ENGAGENT`) : on ne valide, ne fabrique,
 * ne dépose ni n'envoie sur un écran que le moteur n'a pas confirmé. */
let VERIF = null;         // { mois, promesse } : la vérification en route
const GESTES_QUI_ENGAGENT = new Set(["valider-engagement", "contrat-recu", "contrats-imprimer", "dsn-a-deposer", "deposer", "dpae",
  "demander-signature", "envoyer-documents", "documents-imprimer", "avenant-envoyer", "avenants-envoyer",
  "dossier-envoyer", "casc-envoyer", "arret-signaler", "fin-contrat-signaler", "fin-contrat-documents",
  "cloture"]);

function marquerMiseAJour(oui) {
  const el = document.getElementById("mois-maj");
  if (el) el.hidden = !oui;
}

/** L'état frais de `mois`, puis l'écran repeint s'il a changé. `peint` : la fin de la peinture en cours (on ne repeint
 *  pas par-dessus un écran qui se dessine). Rend une promesse qui ne lève jamais. */
function verifierLeMois(mois, peint) {
  marquerMiseAJour(true);
  const generation = LU_GENERATION;
  const promesse = appelerMoteur("mois", { mois, attendre: true }).then(async (r) => {
    await peint;
    // Le mois a été quitté, une écriture de la page l'a relu depuis, ou une vérification plus récente est partie (après
    // une écriture) : rien à repeindre ici.
    if (generation !== LU_GENERATION || MOIS_AFFICHE !== mois || DEMANDE_MOIS !== mois) return;
    if (!VERIF || VERIF.promesse !== promesse) return;
    const ancien = LU && LU.mois === mois ? LU : null;
    const frais = { mois, lectures: r.lectures || {}, perime: false };
    LU = frais;
    if (ancien) await repeindreLeMois(mois, ancien, frais);
  }).catch((e) => {
    if (MOIS_AFFICHE === mois) dire("L'état du mois ne s'est pas mis à jour : " + e.message, "erreur");
  }).finally(() => {
    if (VERIF && VERIF.promesse === promesse) { VERIF = null; marquerMiseAJour(false); }
  });
  VERIF = { mois, promesse };
  return promesse;
}

/** Le mois affiché est confirmé : plus rien à attendre (on a changé de mois vers un état juste). */
function moisConfirme(mois) {
  if (VERIF && VERIF.mois !== mois) { VERIF = null; marquerMiseAJour(false); }
}

/** Ce qu'un geste qui engage attend : la vérification du mois affiché, s'il y en a une en route. */
function attendreLeMoisFrais() { return VERIF ? VERIF.promesse : Promise.resolve(); }

/** Le mois dont `route` est une lecture, ou `null` : `{mois}` pour une lecture du mois. */
function moisDeLaLecture(route, charge) {
  const cles = Object.keys(charge || {});
  if (LECTURES_DU_MOIS.has(route) && cles.length === 1 && cles[0] === "mois" && charge.mois) return charge.mois;
  return null;
}

async function api(route, charge = {}) {
  const mois = moisDeLaLecture(route, charge);
  if (mois) {
    const lu = await lireLeMois(mois);
    return structuredClone(lireReponse(lu.lectures[route]));
  }
  if (ROUTES_SANS_EFFET.has(route)) return appelerMoteur(route, charge);
  if (GESTES_QUI_ENGAGENT.has(route)) await attendreLeMoisFrais();
  oublierLeMois();
  try { return await appelerMoteur(route, charge); } finally { oublierLeMois(); }
}

/* LES GESTES DE LA PAGE (Léandre, 23/10/2026 : « un petit historique de toutes
 * les actions sur la machine, joint au ticket… les 3 minutes d'avant »). La
 * page note ce qu'on y fait — un clic (sur quel bouton, lien, carte), un champ
 * modifié (son nom et ce qu'on y a mis), une boîte qui s'ouvre, ce que la
 * console dit d'une erreur — et le joint à sa prochaine demande au moteur
 * (l'en-tête `X-Gestes`). C'est le moteur qui le met en forme (`diagnostic.rs` :
 * rien qui désigne une personne ne passe) et qui le garde trois minutes, en
 * mémoire : ce fil part avec un retour et avec chaque incident. La page ne
 * garde que ce qui n'est pas encore parti. */
const GESTES = [];
let GESTES_PERDUS = 0;
let PAGE_DITE = false;
const GESTES_MAX = 80;

function noterGeste(g) {
  try {
    if (GESTES.length >= GESTES_MAX) { GESTES.shift(); GESTES_PERDUS += 1; }
    GESTES.push({ t: Date.now(), ...g });
  } catch (e) { /* un guetteur ne lève jamais */ }
}

/** Ce que la page dit d'elle-même, une fois : l'écran, l'échelle, la langue, le fuseau, le moteur de rendu (WebView2). */
function ditDeLaPage() {
  let fuseau = "";
  try { fuseau = Intl.DateTimeFormat().resolvedOptions().timeZone || ""; } catch (e) { /* rien */ }
  return {
    ecran: `${screen.width}×${screen.height}`, fenetre: `${innerWidth}×${innerHeight}`,
    echelle: window.devicePixelRatio || 1, langue: navigator.language || "",
    langues: (navigator.languages || []).join(" "), fuseau,
    decalage_minutes: -new Date().getTimezoneOffset(), agent: navigator.userAgent || "",
    memoire_go: navigator.deviceMemory ?? null, coeurs: navigator.hardwareConcurrency ?? null,
  };
}

/** L'en-tête qui emporte les gestes en attente (et les vide), ou `null`. */
function enteteGestes() {
  if (!GESTES.length && PAGE_DITE && !GESTES_PERDUS) return null;
  const lot = { gestes: GESTES.splice(0) };
  if (!PAGE_DITE) { lot.page = ditDeLaPage(); PAGE_DITE = true; }
  if (GESTES_PERDUS) { lot.perdus = GESTES_PERDUS; GESTES_PERDUS = 0; }
  return encodeURIComponent(sansJeton(JSON.stringify(lot)));
}

/** Ce sur quoi on a cliqué : la balise, l'identifiant, la classe, le texte lisible, l'écran ouvert. */
function cibleDuClic(el) {
  const c = el.closest("button, a, summary, label, select, input, textarea, [role=button], [data-vue], .carte, .choix, tr, li") || el;
  const texte = (c.getAttribute("aria-label") || c.title || c.innerText || c.value || "").replace(/\s+/g, " ").trim();
  return {
    balise: c.tagName.toLowerCase(), id: c.id || "", classe: String(c.className || "").split(/\s+/).slice(0, 2).join("."),
    texte: c.matches("input, textarea, select")
      ? (c.name || c.id || (c.closest("label")?.innerText || "").replace(/\s+/g, " ").trim()).slice(0, 80) : texte.slice(0, 80),
  };
}

document.addEventListener("click", (e) => {
  if (!(e.target instanceof Element)) return;
  noterGeste({ quoi: "clic", cible: cibleDuClic(e.target), vue: ouDansLaPage() });
}, true);

document.addEventListener("change", (e) => {
  const el = e.target;
  if (!(el instanceof HTMLInputElement || el instanceof HTMLSelectElement || el instanceof HTMLTextAreaElement)) return;
  // Le nom du champ : la clé de fiche qu'il remplit (`data-cle` : `nir`, `adresse`…), son `name`, son `id`, son libellé, sinon sa première classe (`e-spectacle`, `r-taux`).
  const nom = el.dataset.cle || el.name || el.id || el.getAttribute("aria-label") || el.placeholder || String(el.className || "").split(/\s+/)[0] || "";
  const type = el.type || el.tagName.toLowerCase();
  // Un mot de passe ne voyage jamais, même jusqu'au moteur : seulement qu'il a été tapé.
  if (type === "password") return noterGeste({ quoi: "champ", nom, type, secret: true });
  const valeur = type === "checkbox" || type === "radio" ? el.checked
    : type === "file" ? `${(el.files || []).length} fichier(s)` : String(el.value).slice(0, 300);
  // Une case dit aussi ce qu'elle coche (sa `value` : un salarié, un jour) ; le moteur le met en forme.
  noterGeste({ quoi: "champ", nom, type, valeur, ...(type === "checkbox" || type === "radio" ? { de: String(el.value).slice(0, 80) } : {}) });
}, true);

// Une boîte qui s'ouvre (`ouvrirBoite`) : son titre.
let BOITE_DITE = "";
new MutationObserver(() => {
  const voile = document.getElementById("voile");
  if (!voile || voile.classList.contains("cachee")) { BOITE_DITE = ""; return; }
  const h = voile.querySelector("h3, h2");
  const titre = h ? h.textContent.trim().slice(0, 100) : "";
  if (titre && titre !== BOITE_DITE) { BOITE_DITE = titre; noterGeste({ quoi: "boite", titre }); }
}).observe(document.body, { subtree: true, childList: true, attributes: true, attributeFilter: ["class"] });

// Ce que la console dit d'une erreur ou d'un avertissement.
for (const niveau of ["error", "warn"]) {
  const avant = console[niveau].bind(console);
  console[niveau] = (...args) => {
    noterGeste({ quoi: "console", niveau, message: args.map((a) => (a instanceof Error ? `${a.name}: ${a.message}` : String(a))).join(" ").slice(0, 300) });
    avant(...args);
  };
}

async function appelerMoteur(route, charge = {}) {
  let r;
  try {
    const entetes = { "Content-Type": "application/json", "X-Jeton": JETON };
    const gestes = enteteGestes();
    if (gestes) entetes["X-Gestes"] = gestes;
    r = await fetch("/api/" + route + "?jeton=" + encodeURIComponent(JETON), {
      method: "POST",
      headers: entetes,
      body: JSON.stringify(charge),
    });
  } catch (e) {
    // `fetch` ne rejette que sur un échec réseau — le serveur est parti.
    serveurPerdu();
    const perdu = new Error("Le serveur PuppetPaie ne répond plus. Rien n'a été enregistré.");
    // Ce n'est pas un défaut de la page, et de toute façon le journal des
    // incidents vit derrière ce serveur-là : le rapport n'arriverait nulle
    // part. Voir `noterIncident`.
    perdu.reseau = true;
    throw perdu;
  }
  // UN CORPS QUI N'EST PAS DU JSON (#141) : un refus du serveur avant la
  // route (corps trop gros) le dit désormais en JSON, mais une réponse vide
  // ou coupée ne doit pas lever une SyntaxError sans phrase.
  const texte = await r.text();
  let d;
  try { d = JSON.parse(texte); } catch (e) {
    const vide = new Error(`PuppetPaie a répondu ${r.status} sans dire pourquoi (route ${route}). Rien n'est sûr d'avoir été enregistré.`);
    vide.detail = { type: "ReponseIllisible", message: vide.message };
    throw vide;
  }
  if (!d.ok) {
    const erreur = typeof d.erreur === "object" && d.erreur ? d.erreur : { message: String(d.erreur || "Refusé.") };
    if (erreur.type === "EnvoiBride") montrerEnvoiRefuse(erreur.message);
    const e = new Error(erreur.message); e.detail = erreur; throw e;
  }
  return d.resultat;
}

/* L'ENVOI REFUSÉ D'UNE FICTIVE AJOUTÉE (ticket #236) : le moteur refuse tout
 * geste qui ferait partir quelque chose (bridage::refus_d_envoi) et rend sa
 * phrase, toujours la même ; la page la montre dans une seule boîte, au-dessus
 * de tout, quel que soit le geste. L'adresse de la phrase devient un lien
 * mailto:. Le geste qui l'a demandé reçoit aussi le refus (il rallume son
 * bouton) : sa ligne d'erreur ne répète pas la phrase (`dire`). */
let ENVOI_REFUSE_MONTRE = null;
function montrerEnvoiRefuse(message) {
  ENVOI_REFUSE_MONTRE = message;
  const voile = document.getElementById("voile-envoi-refuse");
  if (!voile) return;
  const texte = String(message).replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c]));
  document.getElementById("envoi-refuse-texte").innerHTML = texte.replace(/([\w.+-]+@[\w-]+(\.[\w-]+)+)/g,
    (a) => `<a href="mailto:${a}">${a}</a>`);
  voile.classList.remove("cachee");
  const fermer = () => { voile.classList.add("cachee"); voile.onclick = null; };
  document.getElementById("envoi-refuse-ok").onclick = fermer;
  voile.onclick = (e) => { if (e.target === voile) fermer(); };
  setTimeout(() => document.getElementById("envoi-refuse-ok").focus(), 30);
}

/* ------------------------------ ce qui casse ICI, et qui mourait en silence
 *
 * Cinq mille lignes de JavaScript dont les exceptions s'imprimaient dans la
 * console du navigateur — que personne n'ouvre, et surtout pas une compagnie
 * à trois cents kilomètres. Le moteur, lui, note tout ce qui casse chez lui
 * depuis le 20/09/2026 ; la page était le dernier endroit d'où une erreur
 * pouvait disparaître sans laisser de trace.
 *
 * Elle poste donc ce qui casse à `/api/incident-navigateur`, et c'est
 * `incidents.depuis_navigateur` qui décide de la nature, regroupe et
 * caviarde. La page n'écrit pas le journal : elle le nourrit.
 *
 * DEUX GUETTEURS, PARCE QU'IL EN FAUT DEUX. `window.onerror` ne voit pas le
 * rejet d'une promesse, et cette page est asynchrone de bout en bout : un
 * `await` qui lève dans un gestionnaire de bouton non attendu ne passe QUE
 * par `unhandledrejection`.
 *
 * TROIS CHOSES NE REMONTENT PAS, et chacune pour sa raison :
 *   - une erreur métier venue de `/api` (elle porte `detail`) — le serveur
 *     l'a déjà notée, du côté où elle est née ; la reposter la compterait
 *     deux fois ;
 *   - le serveur parti (`reseau`) — ce n'est pas un défaut, et le journal
 *     vit derrière ce serveur-là ;
 *   - le même défaut deux fois, et au-delà de vingt en tout : une boucle qui
 *     lève à chaque image remplirait le journal d'une seule erreur. */
const INCIDENTS_VUS = new Set();
let INCIDENTS_POSTES = 0;
const INCIDENTS_MAX = 20;

/* Le jeton ouvre le serveur local : il ne voyage pas, même dans une pile
 * d'appels qui cite l'adresse d'un `fetch`. Le moteur le caviarde aussi —
 * deux serrures, comme pour le NIR. */
const sansJeton = (t) => String(t ?? "").replace(/jeton=[^&\s"']+/gi, "jeton=[retiré]");

/** L'onglet ouvert : ce que l'utilisatrice était en train de faire. */
function ouDansLaPage() {
  const b = document.querySelector(".onglets button.actif");
  return b ? String(b.dataset.vue || "") : "";
}

function noterIncident(rapport) {
  try {
    if (INCIDENTS_POSTES >= INCIDENTS_MAX) return;
    // Le message entre dans la clé, et pas seulement l'endroit : un rejet
    // de promesse n'a ni fichier ni ligne, et deux défauts différents s'y
    // confondraient. Le regroupement, lui, se fait à la lecture du journal.
    const cle = [rapport.genre, rapport.fichier, rapport.ligne,
                 String(rapport.message || "").slice(0, 80)].join("|");
    if (INCIDENTS_VUS.has(cle)) return;
    INCIDENTS_VUS.add(cle);
    INCIDENTS_POSTES += 1;
    // `fetch` nu, et non `api()` : `api()` lève, et une exception levée
    // depuis le guetteur d'exceptions rentrerait dans sa propre bouche.
    fetch("/api/incident-navigateur?jeton=" + encodeURIComponent(JETON), {
      method: "POST",
      headers: (() => { const h = { "Content-Type": "application/json", "X-Jeton": JETON }; const g = enteteGestes(); if (g) h["X-Gestes"] = g; return h; })(),
      body: JSON.stringify({
        genre: rapport.genre,
        message: sansJeton(rapport.message),
        fichier: rapport.fichier,
        ligne: rapport.ligne,
        colonne: rapport.colonne,
        pile: sansJeton(rapport.pile),
        canal: rapport.canal,
        ou_dans_la_page: ouDansLaPage(),
      }),
    }).catch(() => {});      // le serveur est parti : il n'y a rien à faire
  } catch (e) {
    /* Un guetteur qui lève remplacerait l'erreur qu'on cherche par la
     * sienne. Il se tait. */
  }
}

/** Ce qu'on ne remonte pas : voir l'en-tête. */
function incidentAIgnorer(e) {
  return !!(e && (e.detail || e.reseau));
}

window.addEventListener("error", (ev) => {
  // Une ressource qui ne se charge pas (image, feuille de style) déclenche
  // le même événement, sans `error` : ce n'est pas une exception, et
  // `ev.target` est l'élément fautif.
  if (!ev.error && ev.target && ev.target !== window) {
    return noterIncident({
      genre: "RessourceAbsente",
      message: String(ev.target.tagName || "?") + " n'a pas pu être chargé",
      fichier: ev.target.src || ev.target.href || "",
      ligne: 0, canal: "ressource",
    });
  }
  if (incidentAIgnorer(ev.error)) return;
  noterIncident({
    genre: (ev.error && ev.error.name) || "Erreur",
    message: ev.message || String(ev.error || ""),
    fichier: ev.filename, ligne: ev.lineno, colonne: ev.colno,
    pile: ev.error && ev.error.stack, canal: "erreur",
  });
// `true` — la PHASE DE CAPTURE. Une ressource qui ne se charge pas émet un
// événement qui NE REMONTE PAS : sans capture, `window` ne le voit jamais.
}, true);

/* D'OÙ VIENT UN REJET (09/10/2026). L'événement d'une promesse rejetée ne dit
 * ni fichier ni ligne ; une Error, si : le premier script que cite sa pile,
 * là où elle a été levée. Chromium (WebView2) écrit « at f (http://hôte/js/
 * x.js?v=…:12:5) », WebKit « f@http://hôte/js/x.js?v=…:12:5 ». La requête
 * reste derrière (le moteur la coupe aussi, `fichier_court`). Sans pile
 * lisible, rien : le moteur écrit alors « interface:? ». Il écrivait
 * « interface.js », un fichier qui n'existe plus depuis que la page est
 * coupée en vingt (lot L), et tous les rejets s'y regroupaient. */
function sourceDeLaPile(pile) {
  const m = String(pile || "").match(/(https?:\/\/[^\s()]+?\.js)(?:\?[^\s():]*)?:(\d+):(\d+)/);
  return m ? { fichier: m[1], ligne: +m[2], colonne: +m[3] } : { fichier: "", ligne: 0 };
}

window.addEventListener("unhandledrejection", (ev) => {
  const r = ev.reason;
  if (incidentAIgnorer(r)) return;
  // Une promesse peut être rejetée avec n'importe quoi — une chaîne, un
  // objet. Seule une vraie Error porte une pile et un nom.
  const estErreur = r instanceof Error;
  noterIncident({
    genre: estErreur ? (r.name || "Erreur") : "RejetNonRattrape",
    message: estErreur ? r.message : String(r),
    ...sourceDeLaPile(estErreur ? r.stack : ""),
    pile: estErreur ? r.stack : "", canal: "promesse",
  });
});

/** Ouvre un dossier du poste dans l'explorateur, et dit lequel (#141) :
 *  l'explorateur peut s'ouvrir derrière la fenêtre, et le chemin, c'est le
 *  moteur qui le rend (`dossier`). */
async function ouvrirEtDire(route, charge = {}) {
  try {
    const r = await api(route, charge);
    if (r && r.dossier) dire(`Dossier ouvert : ${r.dossier}`);
  } catch (e) { dire(e.message, "erreur"); }
}

/** `type` : "erreur" (en rouge), "envoye" (ce qui est parti : en vert, coché — ticket #220, Léandre, 20/10/2026 :
 *  « un petit "envoi en cours" et "message envoyé" joli »), rien pour une simple nouvelle. */
function dire(texte, type) {
  // Le refus d'un envoi d'une fictive ajoutée est déjà dans sa boîte (`montrerEnvoiRefuse`).
  if (type === "erreur" && ENVOI_REFUSE_MONTRE !== null && texte === ENVOI_REFUSE_MONTRE) return;
  const el = document.getElementById("message");
  el.textContent = texte;
  el.className = "message" + (type === "erreur" ? " erreur" : type === "envoye" ? " envoye" : "");
  clearTimeout(dire._t);
  dire._t = setTimeout(() => el.classList.add("cachee"), 6000);
}

/** UN ACTE EN COURS SE VOIT (26/09/2026 : « quand on met envoyer les
 *  contrats, il faut un petit message pour dire envoi des contrats en
 *  cours »). Le bouton cliqué s'éteint et dit ce qui se passe, là où l'on
 *  regarde — dans la boîte, que le bandeau du bas ne traverse pas. Rend
 *  la fonction qui le rallume (une erreur : on peut réessayer). */
function occupe(bouton, texte) {
  if (!bouton) return () => {};
  const avant = bouton.innerHTML;
  bouton.disabled = true;
  bouton.classList.add("occupe");
  bouton.textContent = texte;
  return () => {
    if (!bouton.isConnected) return;
    bouton.disabled = false;
    bouton.classList.remove("occupe");
    bouton.innerHTML = avant;
  };
}

const echapper = (s) => String(s ?? "").replace(/[&<>"]/g,
  (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c]));

/* Une phrase du moteur qui porte un lien (le texte d'une convention, journal
 * §161) : echappee d'abord, puis l'adresse https devient cliquable. Le point
 * qui clot la phrase reste dehors. */
const echapperLiens = (s) => echapper(s).replace(
  /(https:\/\/[^\s<]+?)([.,;]?)(?=\s|$)/g,
  (_, url, fin) => `<a href="${url}" target="_blank" rel="noopener">texte de la convention</a>${fin}`);

/* ------------------------------------------------------------ navigation */
document.querySelectorAll(".onglets button").forEach((b) => {
  b.onclick = async () => {
    // Quitter une fiche en cours de saisie sans rien dire, c'est perdre ce
    // qui vient d'etre tape. La boite ne s'ouvre que si quelque chose a
    // change -- voir `quitterLaFiche`.
    if (!(await quitterLaFiche())) return;
    document.querySelectorAll(".onglets button").forEach((x) => x.classList.remove("actif"));
    b.classList.add("actif");
    document.querySelectorAll(".vue").forEach((v) => v.classList.add("cachee"));
    document.getElementById("vue-" + b.dataset.vue).classList.remove("cachee");
    if (b.dataset.vue === "historique") { dessinerHistorique(); rafraichirHistorique(); }
    if (b.dataset.vue === "configuration") dessinerConfig();
    if (b.dataset.vue === "exports") dessinerExports();
    if (b.dataset.vue === "paie") {
      dessinerConseils();
      // REVENIR SUR PAIE REPREND CE QUI N'A PAS PU S'ÉCRIRE (#236) : une saisie
      // que le moteur a refusée faute d'une fiche (le taux AT/MP, l'effectif…)
      // restait à l'écran, « PAS enregistré », jusqu'à une frappe. La fiche
      // vient peut-être d'être corrigée : on la redonne au moteur tout de
      // suite ; c'est lui qui dit si elle passe. Rien d'autre à écrire : on
      // relit seulement ce que le moteur dit du mois.
      if (MOIS_AFFICHE) {
        const enAttente = JSON.stringify(saisieCourante()) !== DERNIERE_ECRITE;
        if (enAttente) await enregistrerSaisie(true);
        else await rafraichirActions();
      }
    }
  };
});


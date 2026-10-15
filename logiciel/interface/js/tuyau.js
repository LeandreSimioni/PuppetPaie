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
 * contrats en cours, les états, les embauches, les fins, les arrêts, la
 * reprise du journal, l'aperçu) ne partent plus une à une au moteur : une
 * seule demande `mois` les rend toutes, lues dans `paies/AAAA-MM.etat.json`
 * par la fenêtre elle-même, sans calcul. Le calcul s'est fait avant, quand
 * quelque chose a changé. Chaque lecture garde sa forme (résultat ou refus) :
 * les fonctions qui dessinent n'ont pas changé.
 *
 * Toute autre route peut écrire : ce qui a été lu part alors, avant ET après
 * elle — on n'affiche jamais un état qu'on aurait pu changer soi-même. */
const LECTURES_DU_MOIS = new Set(["saisie", "contrats-en-cours", "etat-du-mois",
  "embauches-a-declarer", "fins-de-contrat", "etat-par-paie", "imprevus"]);
// Elles ne dépendent pas du mois : n'importe quelle lecture `mois` les porte.
const LECTURES_DE_LA_COMPAGNIE = new Set(["arrets", "journal-reprise"]);
// Des lectures que la page fait souvent, sans rien changer : elles ne vident pas ce qui a été lu.
const ROUTES_SANS_EFFET = new Set(["veille", "rappels", "pour-demarrer", "organismes-contrats", "incident-navigateur", "diagnostic-etat",
  "verifier-fiche", "emploi", "signalements", "apercu", "importation-incident", "importation-tri", "arret-obligations"]);
let LU = null;            // { mois, lectures } : la dernière lecture `mois`
let LU_EN_COURS = null;   // { mois, promesse } : celle qui est en route
let LU_GENERATION = 0;

/** Une réponse du moteur, telle que `appelerMoteur` la rend : son résultat, ou son refus levé. */
function lireReponse(d) {
  if (!d) throw new Error("Lecture absente de l'état du mois.");
  if (!d.ok) { const e = new Error(d.erreur.message); e.detail = d.erreur; throw e; }
  return d.resultat;
}

/** Tout ce que l'écran de `mois` lit, en une demande (partagée si elle est déjà en route). */
function lireLeMois(mois) {
  if (LU && LU.mois === mois) return Promise.resolve(LU);
  if (LU_EN_COURS && LU_EN_COURS.mois === mois) return LU_EN_COURS.promesse;
  const generation = LU_GENERATION;
  const promesse = appelerMoteur("mois", { mois }).then((r) => {
    const lu = { mois, lectures: r.lectures || {} };
    if (generation === LU_GENERATION) LU = lu;
    if (LU_EN_COURS && LU_EN_COURS.promesse === promesse) LU_EN_COURS = null;
    return lu;
  }, (e) => {
    if (LU_EN_COURS && LU_EN_COURS.promesse === promesse) LU_EN_COURS = null;
    throw e;
  });
  LU_EN_COURS = { mois, promesse };
  return promesse;
}

function oublierLeMois() { LU = null; LU_EN_COURS = null; LU_GENERATION++; }

/** Le mois dont `route` est une lecture, ou `null` : `{mois}` pour une lecture du mois, `{}` pour une de la compagnie. */
function moisDeLaLecture(route, charge) {
  const cles = Object.keys(charge || {});
  if (LECTURES_DU_MOIS.has(route) && cles.length === 1 && cles[0] === "mois" && charge.mois) return charge.mois;
  if (LECTURES_DE_LA_COMPAGNIE.has(route) && !cles.length) return MOIS_AFFICHE || document.getElementById("mois").value || null;
  return null;
}

async function api(route, charge = {}) {
  const mois = moisDeLaLecture(route, charge);
  if (mois) {
    const lu = (LECTURES_DE_LA_COMPAGNIE.has(route) && LU) ? LU : await lireLeMois(mois);
    return structuredClone(lireReponse(lu.lectures[route]));
  }
  if (ROUTES_SANS_EFFET.has(route)) return appelerMoteur(route, charge);
  oublierLeMois();
  try { return await appelerMoteur(route, charge); } finally { oublierLeMois(); }
}

async function appelerMoteur(route, charge = {}) {
  let r;
  try {
    r = await fetch("/api/" + route + "?jeton=" + encodeURIComponent(JETON), {
      method: "POST",
      headers: { "Content-Type": "application/json", "X-Jeton": JETON },
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
    const e = new Error(erreur.message); e.detail = erreur; throw e;
  }
  return d.resultat;
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
      headers: { "Content-Type": "application/json", "X-Jeton": JETON },
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

function dire(texte, type) {
  const el = document.getElementById("message");
  el.textContent = texte;
  el.className = "message" + (type === "erreur" ? " erreur" : "");
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
    if (b.dataset.vue === "historique") dessinerHistorique();
    if (b.dataset.vue === "configuration") dessinerConfig();
    if (b.dataset.vue === "exports") dessinerExports();
  };
});


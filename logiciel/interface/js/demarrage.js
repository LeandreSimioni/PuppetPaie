/* Le demarrage : demarrer, finir le chargement ; la fenetre PuppetPaie, le bouton Accueil. Charge en dernier. */
"use strict";

/* =======================================================================
   DEMARRAGE
   =======================================================================
   L'OUVERTURE D'UN COUP (ticket #244, Léandre, 22/10/2026 : « pas 1
   seconde, une demi max » ; « je ne veux pas que la page s'affiche petit à
   petit » ; « ne pas poireauter 5 secondes devant une roue qui tourne pour
   on ne sait pas quoi »). UNE demande, `ouverture`, que la fenêtre sert
   sans processus ni calcul (`etat_garde::FileDesEtats::ouverture`) : le
   dernier état connu de tout ce que la page lit en s'ouvrant — l'accueil,
   le mois à ouvrir et son écran, l'échéancier, les rappels, ce qui manque
   pour démarrer, les conseils, la veille, la mise à jour, les signalements.
   La page se peint d'un coup avec, et le voile s'en va.

   Ce qui était périmé (le moteur le dit : `perime`, `ecran.perime`) se
   refait déjà en fond ; l'état frais arrive ensuite et se pose EN PLACE
   (`rafraichirOuverture` ; le mois, par `verifierLeMois`) : une valeur, un
   état, un badge — la place de chaque bloc est déjà prise.

   Sans état connu (compagnie neuve, état illisible), le moteur le fait et
   dit ce qu'il fait (`phrase`) : le voile l'affiche, et la page redemande. */

/** Une lecture de l'ouverture : son résultat, ou son refus levé (comme `api`). */
function lectureDe(o, route) {
  return structuredClone(lireReponse(o.lectures[route]));
}

/** Le bandeau : les erreurs de configuration et les compteurs. */
function peindreEtatGlobal() {
  // Une option VIDE en tete, et c'est elle le defaut. Un deroulant montre
  // toujours quelque chose : sans elle, le premier spectacle de la liste a
  // l'air choisi alors que personne ne l'a choisi -- et il porte le numero
  // d'objet qui part en DSN. Un mois porte d'ailleurs plusieurs spectacles :
  // c'est l'engagement qui dit le sien, pas le mois.
  const c = ETAT.configuration;
  const err = [c.entreprise, ...c.salaries, ...c.spectacles]
    .reduce((n, f) => n + f.anomalies.filter((a) => a.gravite === "erreur" && !aRemplir(a)).length, 0);
  const annees = Object.entries(c.compteurs).filter(([k]) => /^\d{4}$/.test(k));
  const compteurs = annees.slice(-1)[0];
  const html = `
    ${err ? `<span class="pastille erreur">${err} erreur(s) de configuration</span>` : ""}
    ${compteurs ? `<div>compteurs ${echapper(compteurs[0])} :
       bulletin <b>${compteurs[1].bulletin ?? "?"}</b> ·
       AEM <b>${compteurs[1].aem ?? "?"}</b> ·
       CS <b>${compteurs[1].certificat_cs ?? "?"}</b></div>` : ""}`;
  const el = document.getElementById("etat-global");
  if (el.dataset.peint !== html) { el.dataset.peint = html; el.innerHTML = html; }
}

/** Les conseils de l'ouverture, lus : `{jour, resume}` (un refus : rien à montrer). */
function conseilsDe(o) {
  const lu = (r) => { try { return lectureDe(o, r); } catch (e) { return null; } };
  return { jour: lu("conseil-du-jour"), resume: lu("conseils-resume") };
}

let OUVERTURE = null;   // { mois } : ce que l'ouverture a peint

async function demarrer() {
  let o = await appelerMoteur("ouverture");
  while (!o.pret) {
    // LA SEULE ATTENTE : rien n'est encore connu. Le moteur dit ce qu'il fait.
    document.getElementById("chargement-phrase").textContent = o.phrase || "";
    o = await appelerMoteur("ouverture", { patienter: 400 });
  }
  AUJOURDHUI = o.aujourdhui || "";
  ETAT = lectureDe(o, "etat");
  peindreEtatGlobal();
  // L'échéancier sert à la colonne des actes (la date limite de la DSN) : il est déjà là.
  try { ECHEANCIER = lectureDe(o, "echeancier"); } catch (e) { ECHEANCIER = null; }
  dessinerVeille(o.veille);
  try { dessinerPastilleReponses(lireReponse(o.signalements)); } catch (e) { /* la pastille est un plus */ }
  if (window.miseAJourMontrer) window.miseAJourMontrer(o.maj);
  // Le conseil du jour, quand la compagnie a coché l'option (ticket 224) : il ne retient rien.
  dessinerConseils(conseilsDe(o));

  // LE MOIS À OUVRIR, le moteur le dit (`mois-a-ouvrir`, et le mois du jour
  // sans lui) : le mois en cours, sauf un mois passé dont une étape n'est pas
  // verte (01/10/2026 : « on doit attendre que tout soit vert pour passer au
  // mois suivant »). Son écran est arrivé avec l'ouverture.
  OUVERTURE = { mois: o.mois };
  poserLeMois(o.mois, o.ecran);
  await chargerMois(o.mois);
  finirChargement();

  // Ce qui attend : la DSN du mois d'avant, la clôture en janvier. Une
  // boîte, avec la page, tant que ce n'est pas fait.
  montrerRappels((r) => lectureDe(o, r));

  if (o.perime) rafraichirOuverture();

  // Le travail de fond : on le regarde toutes les minutes. C'est une lecture
  // d'etat en memoire cote serveur, elle ne coute rien -- surtout pas un
  // appel a Google ou a net-entreprises, qui n'ont lieu que dans le fil de
  // la veille, a son rythme a lui.
  setInterval(suivreVeille, 60000);
}

/** L'ÉTAT FRAIS DE L'OUVERTURE (ce qui était peint était périmé) : chaque
 *  chose se repose à sa place, rien ne s'ajoute ni ne se retire autour. Le
 *  mois à ouvrir qui change (un nouveau mois a commencé, un mois passé est
 *  redevenu incomplet) n'emmène ailleurs que si l'on n'a encore touché à
 *  rien. */
async function rafraichirOuverture() {
  let o;
  try { o = await appelerMoteur("ouverture", { frais: true }); } catch (e) { return; }
  if (o.aujourdhui) AUJOURDHUI = o.aujourdhui;
  try { ETAT = lectureDe(o, "etat"); peindreEtatGlobal(); } catch (e) { /* l'état d'avant reste */ }
  try { ECHEANCIER = lectureDe(o, "echeancier"); } catch (e) { /* idem */ }
  dessinerConseils(conseilsDe(o));
  rafraichirRappels((r) => lectureDe(o, r));
  const intact = MOIS_AFFICHE === OUVERTURE.mois && DEMANDE_MOIS === OUVERTURE.mois
    && JSON.stringify(saisieCourante()) === DERNIERE_ECRITE;
  if (o.mois !== OUVERTURE.mois && intact) allerAuMois(o.mois);
  OUVERTURE = { mois: o.mois };
}

/** Le voile du demarrage (index.html, #chargement) : rien que le fond tant
 *  que la page n'est pas peinte, et la phrase du moteur s'il faut attendre ;
 *  puis la page, d'un coup. Retire aussi en cas d'echec : un voile qui
 *  resterait cacherait le message qui dit pourquoi. */
function finirChargement() {
  const v = document.getElementById("chargement");
  if (v) v.remove();
}

demarrer().catch((e) => {
  finirChargement();
  dire("Démarrage impossible : " + e.message, "erreur");
});

/* LA FENETRE PUPPETPAIE, SANS BARRE NOIRE (ticket #195, 15/10/2026). La
   fenetre n'a pas de bordure systeme : le bandeau EST sa barre de titre. Son
   cadre (`fenetre/src/cadre.js`, pose par la fenetre dans chaque page) donne
   `window.fenetrePuppetPaie` : les trois boutons lui parlent, le bandeau se
   tire (`fenetre-tire`) et un double-clic agrandit. Dans un navigateur, il
   n'existe pas : `body` n'a pas la classe `fenetre` et rien ne s'affiche. */
function fenetreNative() {
  const api = window.pywebview && window.pywebview.api;
  // L'ACCUEIL DES COMPAGNIES (ticket #113) : le serveur de la fenetre le
  // propose (`pywebview.api.accueil`, la cale du moteur), dans la fenetre
  // comme dans le navigateur.
  if (api && typeof api.accueil === "function" && !document.body.classList.contains("accueil-possible")) {
    boutonAccueil(api);
  }
  const f = window.fenetrePuppetPaie;
  if (!f || document.body.classList.contains("fenetre")) return;
  document.body.classList.add("fenetre");
  document.querySelector(".bandeau").classList.add("fenetre-tire");
  document.getElementById("fenetre-reduire").onclick = () => f.reduire();
  document.getElementById("fenetre-agrandir").onclick = () => f.agrandir();
  // Fermer enregistre d'abord ce qui attend encore (la saisie s'enregistre
  // une seconde apres la derniere frappe), puis ferme.
  document.getElementById("fenetre-fermer").onclick = async () => {
    clearTimeout(MINUTEUR_SAISIE);
    await enregistrerSaisie(false).catch(() => false);
    f.fermer();
  };
  // Agrandie, le bouton du milieu restaure : il le montre.
  const milieu = document.getElementById("fenetre-agrandir");
  const suivre = async () => {
    const agrandie = await f.agrandie();
    milieu.innerHTML = agrandie ? "&#x2750;" : "&#x25A1;";
    milieu.title = agrandie ? "Restaurer" : "Agrandir";
  };
  window.addEventListener("resize", suivre);
  suivre();
}
/* Avant de quitter la page (revenir aux compagnies, redémarrer pour mettre à
   jour : js/mise-a-jour.js) : ce qui attend encore s'enregistre. Faux si
   l'enregistrement est refusé : on reste, le message est à l'écran, et partir
   perdrait ce qu'on vient de taper. */
async function avantDeQuitter() {
  clearTimeout(MINUTEUR_SAISIE);
  await enregistrerSaisie(false);
  const s = saisieCourante();
  const ecrivable = s.mois && !(MOIS_FERME && MOIS_FORCE !== s.mois);
  return !(ecrivable && JSON.stringify(s) !== DERNIERE_ECRITE);
}
window.avantDeQuitter = avantDeQuitter;
function boutonAccueil(api) {
  document.body.classList.add("accueil-possible");
  document.getElementById("fenetre-accueil").onclick = async () => {
    if (!(await avantDeQuitter())) return;
    api.accueil();
  };
}
window.addEventListener("pywebviewready", fenetreNative);
fenetreNative();


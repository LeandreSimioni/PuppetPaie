/* Le demarrage : demarrer, finir le chargement ; la fenetre PuppetPaie, le bouton Accueil. Charge en dernier. */
"use strict";

/* =======================================================================
   DEMARRAGE
   =======================================================================
   L'OUVERTURE D'UN COUP : la page entière d'un seul geste, jamais petit à
   petit ni derrière une roue qui tourne sans dire pourquoi. UNE demande,
   `ouverture`, que la fenêtre sert sans processus
   (`etat_garde::FileDesEtats::ouverture`) : tout ce que la page lit en
   s'ouvrant, juste — l'accueil, le mois à ouvrir et son écran,
   l'échéancier, les rappels, ce qui manque pour démarrer, les conseils, la
   veille, la mise à jour, les signalements. La page se peint d'un coup
   avec, et le voile s'en va. L'écran du mois peut arriver périmé
   (`ecran.perime`) : l'état frais le remplace en place (`verifierLeMois`).

   Ce qui se refait (compagnie neuve, une écriture depuis), le moteur le
   fait et dit ce qu'il fait (`phrase`) : le voile l'affiche, et la page
   redemande. */

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
  const annees = Object.entries(c.compteurs).filter(([k]) => /^\d{4}$/.test(k));
  const compteurs = annees.slice(-1)[0];
  // Les erreurs de configuration, comptées par le moteur (`configuration`, `erreurs`).
  const html = `
    ${pastilleDeLEtat(c.erreurs)}
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

async function demarrer() {
  let o = await appelerMoteur("ouverture");
  while (!o.pret) {
    // LA SEULE ATTENTE : ce que la page lit se refait. Le moteur dit ce qu'il fait.
    document.getElementById("chargement-phrase").textContent = o.phrase || "";
    o = await appelerMoteur("ouverture", { patienter: 400 });
  }
  AUJOURDHUI = o.aujourdhui || "";
  ETAT = lectureDe(o, "etat");
  peindreEtatGlobal();
  dessinerVeille(o.veille);
  try { dessinerPastilleReponses(lireReponse(o.signalements)); } catch (e) { /* la pastille est un plus */ }
  if (window.miseAJourMontrer) window.miseAJourMontrer(o.maj);
  // Le conseil du jour, quand la compagnie a coché l'option : il ne retient rien.
  dessinerConseils(conseilsDe(o));

  // LE MOIS À OUVRIR, le moteur le dit (`mois-a-ouvrir`, et le mois du jour
  // sans lui) : le mois en cours, sauf un mois passé dont une étape n'est pas
  // verte (01/10/2026 : « on doit attendre que tout soit vert pour passer au
  // mois suivant »). Son écran est arrivé avec l'ouverture.
  poserLeMois(o.mois, o.ecran);
  await chargerMois(o.mois);
  finirChargement();

  // UNE COMPAGNIE QUI COMMENCE PAR L'IMPORT (`commencer_par_l_import`, du
  // moteur) : la page s'ouvre sur lui et son guide ; ce qui manque pour
  // démarrer se dit après lui, sous ce qu'il a repris.
  if (ETAT.commencer_par_l_import) {
    document.querySelector('.onglets button[data-vue="configuration"]').click();
    const sous = document.querySelector('.sous-onglets button[data-fiche="importer"]');
    if (sous) sous.click();
  } else {
    // Ce qui attend : la DSN du mois d'avant, la clôture en janvier. Une
    // boîte, avec la page, tant que ce n'est pas fait.
    montrerRappels((r) => lectureDe(o, r));
  }

  // Le travail de fond : on le regarde toutes les minutes. C'est une lecture
  // d'etat en memoire cote serveur, elle ne coute rien -- surtout pas un
  // appel a Google ou a net-entreprises, qui n'ont lieu que dans le fil de
  // la veille, a son rythme a lui.
  setInterval(suivreVeille, 60000);
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

/* LA FENETRE PUPPETPAIE, SANS BARRE NOIRE. La
   fenetre n'a pas de bordure systeme : le bandeau EST sa barre de titre. Son
   cadre (`fenetre/src/cadre.js`, pose par la fenetre dans chaque page) donne
   `window.fenetrePuppetPaie` : les trois boutons lui parlent, le bandeau se
   tire (`fenetre-tire`) et un double-clic agrandit. Dans un navigateur, il
   n'existe pas : `body` n'a pas la classe `fenetre` et rien ne s'affiche. */
function fenetreNative() {
  const api = window.pywebview && window.pywebview.api;
  // L'ACCUEIL DES COMPAGNIES : le serveur de la fenetre le
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
  return toutEstEcrit();
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


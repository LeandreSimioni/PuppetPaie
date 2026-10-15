/* Le demarrage : demarrer, finir le chargement ; la fenetre PuppetPaie, le bouton Accueil. Charge en dernier. */
"use strict";

/* =======================================================================
   DEMARRAGE
   ===================================================================== */
async function demarrer() {
  ETAT = await api("etat");

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
  document.getElementById("etat-global").innerHTML = `
    ${err ? `<span class="pastille erreur">${err} erreur(s) de configuration</span>` : ""}
    ${compteurs ? `<div>compteurs ${echapper(compteurs[0])} :
       bulletin <b>${compteurs[1].bulletin ?? "?"}</b> ·
       AEM <b>${compteurs[1].aem ?? "?"}</b> ·
       CS <b>${compteurs[1].certificat_cs ?? "?"}</b></div>` : ""}`;

  // LE MOIS EN COURS, et rien d'autre. « Le mois doit être le mois en cours de
  // base quand j'ouvre l'app ». La version d'avant
  // ouvrait sur « le premier mois non produit », ce qui paraissait serviable
  // et ne l'était pas : une saisie d'essai laissée en octobre faisait ouvrir
  // le logiciel en octobre, en septembre. On ouvre sur aujourd'hui ; le mois
  // se change d'une case, et l'Historique liste les autres.
  // SAUF UN MOIS PAS FINI (01/10/2026) : « on doit attendre que tout soit
  // vert pour passer au mois suivant, pour éviter d'oublier la DSN ou
  // l'envoi des documents ». Le moteur rend le plus ancien mois passé dont
  // une étape n'est pas verte ; seuls les mois saisis et passés comptent,
  // une saisie d'essai en avance n'y change rien.
  let ouvrir = moisCourant();
  try { ouvrir = (await api("mois-a-ouvrir")).mois || ouvrir; } catch (e) { /* le calendrier, alors */ }
  await chargerMois(ouvrir);
  finirChargement();

  // Ce qui attend : la DSN du mois d'avant, la clôture en janvier. Une
  // boîte, tout de suite, tant que ce n'est pas fait.
  await montrerRappels();

  // Le travail de fond : on le regarde toutes les minutes. C'est une lecture
  // d'etat en memoire cote serveur, elle ne coute rien -- surtout pas un
  // appel a Google ou a net-entreprises, qui n'ont lieu que dans le fil de
  // la veille, a son rythme a lui.
  suivreVeille();
  setInterval(suivreVeille, 60000);
}

/** Le voile du demarrage (index.html, #chargement) : une roue tant que la
 *  page se peint, puis la page d'un coup. Retire aussi en cas d'echec -- un
 *  voile qui resterait cacherait le message qui dit pourquoi -- et au bout de
 *  vingt secondes quoi qu'il arrive : une lecture qui traine ne doit pas
 *  bloquer l'ecran. */
function finirChargement() {
  const v = document.getElementById("chargement");
  if (!v || v.classList.contains("fini")) return;
  v.classList.add("fini");
  setTimeout(() => v.remove(), 300);
}
setTimeout(finirChargement, 20000);

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
function boutonAccueil(api) {
  document.body.classList.add("accueil-possible");
  document.getElementById("fenetre-accueil").onclick = async () => {
    clearTimeout(MINUTEUR_SAISIE);
    await enregistrerSaisie(false);
    // Un enregistrement refusé : on reste, le message est à l'écran.
    // Partir perdrait ce qu'on vient de taper.
    const s = saisieCourante();
    const ecrivable = s.mois && !(MOIS_FERME && MOIS_FORCE !== s.mois);
    if (ecrivable && JSON.stringify(s) !== DERNIERE_ECRITE) return;
    api.accueil();
  };
}
window.addEventListener("pywebviewready", fenetreNative);
fenetreNative();


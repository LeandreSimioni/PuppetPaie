/* Les rappels de l'ouverture, « pour demarrer », l'echeancier. */
"use strict";

/* =======================================================================
   LES RAPPELS DE L'OUVERTURE, ET L'EXERCICE
   =======================================================================
   « Quand le mois est fini, le logiciel s'ouvre sur le mois suivant, et là
   il faut un pop-up : il est temps d'envoyer la DSN du mois précédent. Avez-
   vous bien fini les paies du mois précédent ? Et un bouton vérifier et
   envoyer. ». Puis, en janvier : clôturer l'exercice, exporter
   les chiffres pour la compta, saisir les numéros d'objet de l'année.

   La page ne décide de rien : /api/rappels rend la liste, déjà dans l'ordre,
   et chaque rappel dit où en est le mois (à saisir, à écrire, à déposer, à
   corriger). « Vérifier et envoyer » ne fait qu'ouvrir le mois et enchaîner
   les boîtes qui existent déjà — écrire le mois, puis le récapitulatif de
   dépôt avec son mot de passe. Rien ne part d'ici. « Plus tard » ferme la
   boîte ; elle reviendra à la prochaine ouverture, tant que le journal des
   dépôts ne dit pas que c'est fait. */
/* `lu(route)` : la lecture de l'ouverture (js/demarrage.js : tout arrive d'un
 * coup, et la boîte s'ouvre avec la page) — elle rend le résultat ou lève le
 * refus, comme `api`. Sans elle, on demande. */
let RAPPELS_MONTRES = null;   // ce que la boîte de l'ouverture a montré : `{liste, ech, manques}`, ou null
async function montrerRappels(lu = api) {
  let r, ech = null;
  try { r = await lu("rappels"); } catch (e) { return; }
  try { ech = await lu("echeancier"); } catch (e) { ech = null; }
  let manques = [];
  try { const pd = await lu("pour-demarrer"); manques = pd.manques || []; POSES_DEMARRAGE = pd.poses || []; } catch (e) { manques = []; }
  const liste = r.rappels || [];
  // L'ÉCHÉANCIER : « un pop-up au démarrage : on est le…,
  // bienvenue sur PuppetPaie, n'oublie pas de faire la DSN ». Elle s'ouvrait
  // aussi une fois par jour quand rien n'attendait ; l'éditeur, 01/10/2026 :
  // « elle apparaît seulement s'il y a des trucs à faire ».
  if (!liste.length && !(ech && ech.a_faire) && !manques.length) return;
  RAPPELS_MONTRES = { liste, ech, manques };
  if (dejaMontres(RAPPELS_MONTRES)) return;
  boiteRappels(liste, ech, manques);
}

/* UNE FOIS, PUIS PLUS : la boîte s'ouvre à l'ouverture du logiciel ; la page
 * rechargée pendant qu'il tourne ne la rouvre pas tant qu'elle dirait la même
 * chose (le même jour, les mêmes choses à faire). La mémoire est celle de la
 * fenêtre (`sessionStorage`) : le logiciel rouvert la montre de nouveau. */
function dejaMontres(montres) {
  const cle = "rappels-montres:" + ((montres.ech && montres.ech.compagnie) || "");
  const empreinte = JSON.stringify(montres);
  try {
    if (sessionStorage.getItem(cle) === empreinte) return true;
    sessionStorage.setItem(cle, empreinte);
  } catch (e) { /* sans mémoire, la boîte s'ouvre */ }
  return false;
}

/* CE QUE LE MOTEUR A POSÉ DE LUI-MÊME (`pour-demarrer`, `poses`) : dit avec
 * ce qui manque, en information, avec le lien vers sa case. */
let POSES_DEMARRAGE = [];
function htmlPoses() {
  if (!POSES_DEMARRAGE.length) return "";
  return `<div class="rappel demarrage-poses">
    <h3>Posé pour vous</h3>
    <ul class="rappel-liste">${POSES_DEMARRAGE.map((p) => `
      <li>${pastilleDeLEtat(p.pastille)}
        <button type="button" class="lien" data-manque-champ="${echapper(p.champ || "")}"
          data-manque-ou="${echapper(p.ou || "")}"><b>${echapper(p.libelle)}</b></button>
        <div class="note">${echapper(p.texte)}</div></li>`).join("")}</ul>
  </div>`;
}

/* POUR DÉMARRER. Une compagnie neuve découvrait ce qui lui
 * manquait un refus après l'autre. Le moteur (`pour-demarrer`) rend tout
 * d'un coup : ce que chaque manque bloque, où il se saisit, où la compagnie
 * trouve la valeur. La page ne fait que l'afficher. */
function htmlDemarrage(manques) {
  if (!manques || !manques.length) return htmlPoses();
  // CHAQUE CHOSE MÈNE À SA CASE : le libellé conduit au champ que le
  // moteur nomme (`champ`, celui que l'écran montre : `demarrage::a_l_ecran`),
  // ou à l'onglet qu'il nomme (`ou`) quand ce n'est pas une case.
  const ligne = (m) => `
    <li>
      <button type="button" class="lien" data-manque-champ="${echapper(m.champ || "")}"
        data-manque-ou="${echapper(m.ou || "")}" data-manque-salarie="${echapper(m.salarie || "")}"><b>${echapper(m.libelle)}</b></button>
      <span class="discret">— ${m.bloque ? `bloque ${echapper(m.bloque)}` : `sinon, ${echapper(m.sinon || "")}`}</span>
      <div class="note">À saisir dans ${echapper(m.ou)}.</div>
      ${m.trouver ? `<details><summary>Où le trouver</summary><p class="note">${echapper(m.trouver)}</p></details>` : ""}
    </li>`;
  return `
    <div class="rappel retard demarrage">
      <h3>Pour démarrer, il manque encore ${manques.length === 1 ? "une chose" : manques.length + " choses"}</h3>
      ${manques.some((m) => m.bloque) ? `<p class="note">Tant qu'elles manquent, ce qu'elles bloquent sera refusé.</p>` : ""}
      <ul class="rappel-liste">${manques.map(ligne).join("")}</ul>
      <div class="actions"><button class="petit" data-aller-configuration>Ouvrir la configuration</button></div>
    </div>${htmlPoses()}`;
}

/** Aller à ce qui manque : l'onglet, puis la case (son tiroir ouvert, le
 *  curseur dedans). Le taux AT/MP n'est pas une case de la fiche
 *  (`ATMP_HTML`) ; les organismes ont leur onglet ; ce qui se dit sur la
 *  fiche d'un salarié (`salarie`, du moteur) ouvre sa fiche. */
async function allerAuManque(champ, ou, salarie) {
  if (!document.getElementById("voile").classList.contains("cachee")) fermerBoite();
  // Déjà dans la configuration : on n'y retourne pas. Recliquer l'onglet
  // redessinait la fiche et, sur une fiche en cours de saisie, ouvrait « Vous
  // avez modifié cette fiche » — suivre un manque après l'autre sans
  // enregistrer entre deux ne doit rien demander.
  if (document.getElementById("vue-configuration").classList.contains("cachee")) {
    document.querySelector('.onglets button[data-vue="configuration"]').click();
  }
  const fiche = salarie ? "salarie" : /Organismes/.test(ou) ? "organismes" : "entreprise";
  if (salarie) await ouvrirFicheImportee("salarie", salarie);
  else {
    const sous = document.querySelector(`.sous-onglets button[data-fiche="${fiche}"]`);
    if (sous && !sous.classList.contains("actif")) sous.click();
  }
  if (fiche === "organismes" || !champ) return;
  const trouver = () => champ === "taux_atmp" ? document.getElementById("atmp-valeur")
    : document.querySelector(`#config-formulaire [data-cle="${CSS.escape(champ)}"]:not([type="hidden"])`)
      || document.querySelector(`#config-formulaire [data-sous-de="${CSS.escape(champ)}"]:not([type="hidden"])`);
  let essais = 0;
  const viser = () => {
    const c = trouver();
    if (!c) { if (++essais < 20) setTimeout(viser, 100); return; }
    const tiroir = c.closest("details");
    if (tiroir) tiroir.open = true;
    c.scrollIntoView({ block: "center" });
    c.focus({ preventScroll: true });
  };
  // La liste « Pour démarrer » se peint au-dessus de la fiche quand le moteur
  // a répondu : viser après elle, sinon elle repousse la case hors de la vue.
  Promise.resolve(DEMARRAGE_PEINT).then(viser, viser);
}
document.addEventListener("click", (e) => {
  const b = e.target.closest && e.target.closest("[data-manque-champ]");
  if (!b) return;
  e.preventDefault();
  allerAuManque(b.dataset.manqueChamp, b.dataset.manqueOu, b.dataset.manqueSalarie);
});

/** La liste « Pour démarrer », en tête de Configuration › Entreprise. */
let DEMARRAGE_PEINT = null;   // la peinture en cours de la liste, pour `allerAuManque`
function peindreDemarrage(conteneur) {
  DEMARRAGE_PEINT = peindreDemarrageDans(conteneur);
  return DEMARRAGE_PEINT;
}
async function peindreDemarrageDans(conteneur) {
  let manques = [];
  try { const pd = await api("pour-demarrer"); manques = pd.manques || []; POSES_DEMARRAGE = pd.poses || []; } catch (e) { return; }
  conteneur.innerHTML = htmlDemarrage(manques);
  const b = conteneur.querySelector("[data-aller-configuration]");
  if (b) b.remove();
}

/* L'en-tête de la boîte : la date, la bienvenue, ce qui est à faire ce
 * mois-ci, et l'année qui vient repliée. Tout vient du moteur
 * : la page ne sait pas ce qu'est une échéance. */
function htmlEcheancier(ech) {
  if (!ech) return "";
  const pastille = (x) => pastilleDeLEtat(x.pastille);
  const avant = (x) => x.date && x.etat !== "fait"
    ? ` <span class="discret">— avant le ${echapper(jjmmaaaa(String(x.date || "").slice(0, 10)))}</span>` : "";
  const ligne = (x) => `
    <li class="echeance ${echapper(x.etat)}">
      <div class="echeance-titre">${pastille(x)} ${echapper(x.titre)}${avant(x)}</div>
      ${x.etat === "fait" ? "" : `<div class="note">${echapper(x.detail)}</div>`}
      ${x.cochable ? `<button class="secondaire petit" data-echeance="${echapper(x.cle)}"
        data-fait="${x.etat === "fait" ? "non" : "oui"}">${x.etat === "fait" ? "Pas encore fait" : "C'est fait"}</button>` : ""}
    </li>`;
  const parMois = {};
  (ech.a_venir || []).forEach((x) => { (parMois[x.mois] = parMois[x.mois] || []).push(x); });
  const annee = Object.keys(parMois).map((m) => `
    <li><b>${echapper(moisEnClair(m))}</b>
      <ul>${parMois[m].map((x) => `<li>${echapper(x.titre)}${x.etat === "fait" ? " ✓" : ""}</li>`).join("")}</ul>
    </li>`).join("");
  // CE QUI EST FAIT NE SE RAPPELLE PAS (01/10/2026, « pourquoi il me dit
  // ça alors que je l'ai déjà fait, pas besoin ») : la DSN de septembre,
  // déposée, s'affichait encore « fait ». Une échéance cochable reste, pour
  // pouvoir la décocher ; les autres faites se taisent.
  // UNE CHOSE SE DIT UNE FOIS : la DSN qu'un rappel réclame
  // (`rappel`, le moteur le dit) se dit dans ce rappel, sous l'en-tête, avec
  // sa date et son geste ; ici elle se dirait en double.
  const ceMois = (ech.ce_mois || []).filter((x) => (x.etat !== "fait" || x.cochable) && !x.rappel);
  const ditAilleurs = (ech.ce_mois || []).some((x) => x.rappel);
  return `
    <div class="accueil">
      <p class="accueil-date">Nous sommes le ${echapper(ech.date)}</p>
      <h2 class="accueil-titre">${echapper(ech.bienvenue)}</h2>
      ${ceMois.length
        ? `<h3>Ce mois-ci</h3><ul class="echeances">${ceMois.map(ligne).join("")}</ul>`
        : ditAilleurs ? "" : `<h3>Ce mois-ci</h3><p class="discret">Rien à faire ce mois-ci.</p>`}
      ${annee ? `<details class="accueil-annee"><summary>L'année qui vient</summary>
        <ul class="echeances-annee">${annee}</ul></details>` : ""}
    </div>`;
}

function boiteRappels(liste, ech, manques = []) {
  const html = htmlDemarrage(manques) + htmlEcheancier(ech) + liste.map((r, i) => `
    <div class="rappel ${r.en_retard ? "retard" : ""}">
      <h3>${echapper(r.titre)}${r.pastille ? ` ${pastilleDeLEtat(r.pastille)}` : ""}${r.echeance_depot && r.echeance_depot.jour
        ? ` <span class="discret">— avant le ${echapper(jjmmaaaa(r.echeance_depot.jour))}</span>` : ""}</h3>
      <p class="rappel-question">${echapper(r.question)}</p>
      ${r.detail ? `<p class="note">${echapper(r.detail)}</p>` : ""}
      ${r.spectacles && r.spectacles.length ? `<ul class="rappel-liste">${r.spectacles.map((s) =>
        `<li>${echapper(s.titre)} <span class="discret">— ${echapper(s.numero_objet || "aucun numéro")}</span></li>`).join("")}</ul>` : ""}
      <div class="actions">${boutonRappel(r, i)}</div>
    </div>`).join("");
  ouvrirBoite(`
    <div class="boite-rappels"></div>${html}
    <div class="actions">
      <button class="secondaire" onclick="fermerBoite()">${liste.length ? "Plus tard" : "Fermer"}</button>
    </div>`, () => {
    // FERMÉE, ELLE SORT DE LA PAGE : cachée sous le voile, elle gardait
    // ses boutons `[data-manque-champ]`, périmés dès que la fiche change, et
    // qu'on retrouvait en cherchant ceux de « Pour démarrer ».
    const boite = document.getElementById("boite");
    if (boite.querySelector(".boite-rappels")) boite.innerHTML = "";
  });
  liste.forEach((r, i) => {
    const b = document.getElementById("rappel-" + i);
    if (b) b.onclick = () => agirSurRappel(r);
  });
  const conf = document.querySelector("[data-aller-configuration]");
  if (conf) conf.onclick = () => {
    fermerBoite();
    document.querySelector('.onglets button[data-vue="configuration"]').click();
    const sous = document.querySelector('.sous-onglets button[data-fiche="entreprise"]');
    if (sous && !sous.classList.contains("actif")) sous.click();
  };
  document.querySelectorAll("[data-echeance]").forEach((b) => {
    b.onclick = async () => {
      try {
        await api("echeance-faite", { cle: b.dataset.echeance, fait: b.dataset.fait === "oui" });
        const ech2 = await api("echeancier");
        boiteRappels(liste, ech2, manques);
      } catch (e) { dire(e.message, "erreur"); }
    };
  });
}

/* CE QUI EST DÛ : une obligation à faire ou en retard
   (`obligations.rs`) — la fin d'un contrat qui se signale, la déclaration
   d'accident du travail, le signalement d'un arrêt, l'attestation de salaire
   du deuil. Le moteur rend l'obligation et ce sur quoi elle porte (`objet`) ;
   le geste est celui de la colonne « Ce qui est dû » du mois, tiré des mêmes
   champs (`imprevus.js` : `SIGNALEMENTS_DSN`, `ROUTES_OBLIGATION_FAITE`). */
function gesteObligation(r) {
  const o = r.obligation || {};
  if (o.cle === "signalement_fin_contrat") return (r.a_saisir || []).length ? "carte" : "signaler-fin";
  if (SIGNALEMENTS_DSN[o.cle]) return "signaler";
  if (Array.isArray(o.ou) && ROUTES_OBLIGATION_FAITE[o.cle]) return "faite";
  return "mois";
}

function boutonRappel(r, i) {
  if (r.quoi === "obligation") {
    const libelle = {
      "signaler-fin": "Signaler…", signaler: "Signaler…", faite: "Faite le…",
      carte: "Compléter le contrat", mois: "Ouvrir le mois",
    }[gesteObligation(r)];
    return `<button class="primaire" id="rappel-${i}">${echapper(libelle)}</button>`;
  }
  const libelle = {
    "dsn": "Vérifier et envoyer",
    "cloture": "Clôturer " + (r.annee || ""),
    "export-compta": "Exporter pour la compta",
    "revue-spectacles": "Choisir les spectacles",
    "dpae": r.fichier ? "Envoyer la DPAE" : "Préparer la DPAE",
  }[r.quoi] || "Voir";
  return `<button class="primaire" id="rappel-${i}">${echapper(libelle)}</button>`;
}

async function agirSurRappel(r) {
  fermerBoite();
  if (r.quoi === "dsn") {
    // On va sur le mois, on le regarde, et le dépôt s'enchaîne : c'est la
    // même suite que le bouton DSN de la colonne de droite : la DSN se
    // fabrique à son dépôt, même vide — la DSN « néant » d'un mois sans
    // cachet passe par là aussi.
    document.querySelector('.onglets button[data-vue="paie"]').click();
    await chargerMois(r.mois);
    return deposerLaDsn(r.mois);
  }
  if (r.quoi === "dpae") {
    // LA DPAE PRÉPARÉE D'AVANCE : son test est passé, le bouton
    // ouvre le dépôt de CE fichier — le réel, avec le mot de passe. Sans
    // fichier testé, on va au mois et à la boîte des embauches.
    if (r.fichier) return boiteDepot("dpae", r.fichier);
    document.querySelector('.onglets button[data-vue="paie"]').click();
    await chargerMois(r.mois);
    return boiteDpae(r.mois);
  }
  if (r.quoi === "obligation") {
    const geste = gesteObligation(r);
    if (geste === "signaler-fin") return boiteSignalerFin(r.mois, r.objet);
    if (geste === "signaler") return boiteSignalerArret(r.objet, SIGNALEMENTS_DSN[r.obligation.cle]);
    if (geste === "faite") return boiteObligationFaite(r.objet, r.obligation);
    document.querySelector('.onglets button[data-vue="paie"]').click();
    // Ce qui manque au contrat se tape dans sa carte, sous « Contrat » : la carte s'ouvre sur la première case à dire.
    const fin = r.objet || {};
    await chargerMois(fin.mois || r.mois);
    if (geste === "carte" && (fin.a_saisir || []).length) return allerALaCase({ ...fin, mois: fin.mois || r.mois }, fin.a_saisir[0].champ);
  }
  if (r.quoi === "cloture") return boiteCloture(r.annee);
  if (r.quoi === "export-compta") return lancerExportCompta(r.annee);
  if (r.quoi === "revue-spectacles") return boiteRevueSpectacles(r);
}

/* ---- La revue des spectacles, en début d'année. Un numéro d'objet vaut
 * tant que le spectacle se joue : on
 * demande seulement lesquels continuent. Tout est coché ; décocher archive,
 * et une fiche archivée se réactive d'un clic dans Configuration. */
function boiteRevueSpectacles(r) {
  const lignes = (r.spectacles || []).map((s, i) => `
    <label class="coche"><input type="checkbox" class="rv-garde" data-cle="${echapper(s.cle)}" checked>
      ${echapper(s.titre)}${s.numero_objet ? ` <span class="discret">${echapper(s.numero_objet)}</span>` : ""}</label>`).join("");
  ouvrirBoite(`
    <h3>Les spectacles de ${echapper(r.annee)}</h3>
    <p>${echapper(r.question)}</p>
    ${lignes}
    <p class="note">Décochés, ils s'archivent : ils restent lisibles et se réactivent
      d'un clic (Configuration › Spectacles). Ceux qui continuent gardent leur numéro d'objet.</p>
    <div class="actions">
      <button class="secondaire" onclick="fermerBoite()">Plus tard</button>
      <button class="primaire" id="rv-valider">Enregistrer</button>
    </div>`);
  document.getElementById("rv-valider").onclick = async () => {
    const bouton = document.getElementById("rv-valider");
    bouton.disabled = true;
    const continuent = [...document.querySelectorAll(".rv-garde")]
      .filter((c) => c.checked).map((c) => c.dataset.cle);
    try {
      const res = await api("revue-spectacles", { annee: r.annee, continuent });
      ETAT = await api("etat");
      fermerBoite();
      dire(res.archives.length
        ? `${res.archives.length} spectacle${res.archives.length > 1 ? "s" : ""} archivé${res.archives.length > 1 ? "s" : ""} ; ${res.continuent.length} continue${res.continuent.length > 1 ? "nt" : ""}.`
        : "Tous les spectacles continuent.");
    } catch (e) {
      bouton.disabled = false;
      dire(e.message, "erreur");
    }
  };
}

/* ---- L'exercice, dans l'Historique : par année, ce qui est fait, ce qui
 * se fait. Clôturer fige l'année et écrit le zip ; l'export compta écrit le
 * CSV du journal de paie. Les deux sont locaux — rien ne part. */
async function dessinerExercices() {
  const cible = document.getElementById("exercices");
  let r;
  try { r = await api("exercices"); }
  catch (e) { cible.innerHTML = `<p class="discret">${echapper(e.message)}</p>`; return; }
  if (!r.exercices.length) { cible.innerHTML = `<p class="discret">Aucune année pour l'instant.</p>`; return; }
  cible.innerHTML = r.exercices.map((x) => {
    const clos = x.clos;
    return `
    <div class="rang exercice">
      <span class="rang-nom">${echapper(x.annee)}</span>
      ${pastilleDeLEtat(x.pastille)}
      <span class="rang-infos">
        ${x.mois.length} mois · ${x.dsn_acceptees.length} DSN acceptée${x.dsn_acceptees.length === 1 ? "" : "s"}
        ${clos ? ` · clos le ${echapper(quand(clos.le))}, ${echapper(clos.archive)}` : ""}
        ${x.export ? ` · export compta${x.export.ecrit ? ` du ${echapper(quand(x.export.ecrit))}` : ""}` : " · pas d'export compta"}
      </span>
      <button class="secondaire" data-export="${echapper(x.annee)}">Export compta</button>
      ${clos
        ? `<button class="secondaire" data-rouvrir="${echapper(x.annee)}">Rouvrir</button>`
        : `<button class="secondaire" data-cloturer="${echapper(x.annee)}">Clôturer…</button>`}
    </div>`;
  }).join("") + `<p class="note"><button class="lien" id="ouvrir-exercices"
    title="${echapper(r.dossier)}">Ouvrir le dossier des archives</button></p>`;
  cible.querySelectorAll("[data-export]").forEach((b) => {
    b.onclick = () => lancerExportCompta(b.dataset.export);
  });
  cible.querySelectorAll("[data-cloturer]").forEach((b) => {
    b.onclick = () => boiteCloture(b.dataset.cloturer);
  });
  cible.querySelectorAll("[data-rouvrir]").forEach((b) => {
    b.onclick = async () => {
      try {
        const x = await api("cloture", { annee: b.dataset.rouvrir, rouvrir: true });
        dire("L'année " + x.annee + " est rouverte — son archive reste.");
        await dessinerExercices();
      } catch (e) { dire(e.message, "erreur"); }
    };
  });
  const o = document.getElementById("ouvrir-exercices");
  if (o) o.onclick = async () => {
    await ouvrirEtDire("ouvrir-exercices");
  };
}


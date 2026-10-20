/* Les rappels de l'ouverture, « pour demarrer », l'echeancier. */
"use strict";

/* =======================================================================
   LES RAPPELS DE L'OUVERTURE, ET L'EXERCICE
   =======================================================================
   « Quand le mois est fini, le logiciel s'ouvre sur le mois suivant, et là
   il faut un pop-up : il est temps d'envoyer la DSN du mois précédent. Avez-
   vous bien fini les paies du mois précédent ? Et un bouton vérifier et
   envoyer. » (16/09/2026). Puis, en janvier : clôturer l'exercice, exporter
   les chiffres pour la compta, saisir les numéros d'objet de l'année.

   La page ne décide de rien : /api/rappels rend la liste, déjà dans l'ordre,
   et chaque rappel dit où en est le mois (à saisir, à écrire, à déposer, à
   corriger). « Vérifier et envoyer » ne fait qu'ouvrir le mois et enchaîner
   les boîtes qui existent déjà — écrire le mois, puis le récapitulatif de
   dépôt avec son mot de passe. Rien ne part d'ici. « Plus tard » ferme la
   boîte ; elle reviendra à la prochaine ouverture, tant que le journal des
   dépôts ne dit pas que c'est fait. */
async function montrerRappels() {
  let r, ech = null;
  try { r = await api("rappels"); } catch (e) { return; }
  try { ech = await api("echeancier"); } catch (e) { ech = null; }
  let manques = [];
  try { manques = (await api("pour-demarrer")).manques || []; } catch (e) { manques = []; }
  const liste = r.rappels || [];
  // L'ÉCHÉANCIER (23/09/2026) : « un pop-up au démarrage : on est le…,
  // bienvenue sur PuppetPaie, n'oublie pas de faire la DSN ». Elle s'ouvrait
  // aussi une fois par jour quand rien n'attendait ; l'éditeur, 01/10/2026 :
  // « elle apparaît seulement s'il y a des trucs à faire ».
  if (!liste.length && !(ech && ech.a_faire) && !manques.length) return;
  boiteRappels(liste, ech, manques);
}

/* POUR DÉMARRER (09/10/2026). Une compagnie neuve découvrait ce qui lui
 * manquait un refus après l'autre. Le moteur (`pour-demarrer`) rend tout
 * d'un coup : ce que chaque manque bloque, où il se saisit, où la compagnie
 * trouve la valeur. La page ne fait que l'afficher. */
function htmlDemarrage(manques) {
  if (!manques || !manques.length) return "";
  const ligne = (m) => `
    <li>
      <b>${echapper(m.libelle)}</b> <span class="discret">— bloque ${echapper(m.bloque)}</span>
      <div class="note">À saisir dans ${echapper(m.ou)}.</div>
      ${m.trouver ? `<details><summary>Où le trouver</summary><p class="note">${echapper(m.trouver)}</p></details>` : ""}
    </li>`;
  return `
    <div class="rappel retard demarrage">
      <h3>Pour démarrer, il manque encore ${manques.length === 1 ? "une chose" : manques.length + " choses"}</h3>
      <p class="note">Tant qu'elles manquent, ce qu'elles bloquent sera refusé. Elles se trouvent toutes sur les courriers des organismes.</p>
      <ul class="rappel-liste">${manques.map(ligne).join("")}</ul>
      <div class="actions"><button class="petit" data-aller-configuration>Ouvrir la configuration</button></div>
    </div>`;
}

/** La liste « Pour démarrer », en tête de Configuration › Entreprise. */
async function peindreDemarrage(conteneur) {
  let manques = [];
  try { manques = (await api("pour-demarrer")).manques || []; } catch (e) { return; }
  conteneur.innerHTML = htmlDemarrage(manques);
  const b = conteneur.querySelector("[data-aller-configuration]");
  if (b) b.remove();
}

/* L'en-tête de la boîte : la date, la bienvenue, ce qui est à faire ce
 * mois-ci, et l'année qui vient repliée. Tout vient du moteur
 * (`operations_echeancier`) : la page ne sait pas ce qu'est une échéance. */
function htmlEcheancier(ech) {
  if (!ech) return "";
  const pastille = (x) => ({
    en_retard: `<span class="pastille alerte">en retard</span>`,
    a_faire: `<span class="pastille alerte">à faire</span>`,
    fait: `<span class="pastille bien">fait</span>`,
    information: `<span class="pastille">pour info</span>`,
  }[x.etat] || "");
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
  const ceMois = (ech.ce_mois || []).filter((x) => x.etat !== "fait" || x.cochable);
  return `
    <div class="accueil">
      <p class="accueil-date">Nous sommes le ${echapper(ech.date)}</p>
      <h2 class="accueil-titre">${echapper(ech.bienvenue)}</h2>
      <h3>Ce mois-ci</h3>
      ${ceMois.length
        ? `<ul class="echeances">${ceMois.map(ligne).join("")}</ul>`
        : `<p class="discret">Rien à faire ce mois-ci.</p>`}
      ${annee ? `<details class="accueil-annee"><summary>L'année qui vient</summary>
        <ul class="echeances-annee">${annee}</ul></details>` : ""}
    </div>`;
}

function boiteRappels(liste, ech, manques = []) {
  const html = htmlDemarrage(manques) + htmlEcheancier(ech) + liste.map((r, i) => `
    <div class="rappel ${r.en_retard ? "retard" : ""}">
      <h3>${echapper(r.titre)}${r.en_retard ? ` <span class="pastille alerte">en retard</span>` : ""}</h3>
      <p class="rappel-question">${echapper(r.question)}</p>
      ${r.detail ? `<p class="note">${echapper(r.detail)}</p>` : ""}
      ${r.spectacles && r.spectacles.length ? `<ul class="rappel-liste">${r.spectacles.map((s) =>
        `<li>${echapper(s.titre)} <span class="discret">— ${echapper(s.numero_objet || "aucun numéro")}</span></li>`).join("")}</ul>` : ""}
      <div class="actions">${boutonRappel(r, i)}</div>
    </div>`).join("");
  ouvrirBoite(`
    ${html}
    <div class="actions">
      <button class="secondaire" onclick="fermerBoite()">${liste.length ? "Plus tard" : "Fermer"}</button>
    </div>`);
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

/* CE QUI EST DÛ (20/10/2026, #205) : une obligation à faire ou en retard
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
    // même suite que le bouton DSN de la colonne de droite. Un mois pas
    // encore écrit s'écrit d'abord (boiteProduire), même vide — la DSN
    // « néant » d'un mois sans cachet passe par là aussi.
    document.querySelector('.onglets button[data-vue="paie"]').click();
    await chargerMois(r.mois);
    const e = await api("etat-du-mois", { mois: r.mois });
    const d = (e.fichiers.dsn || [])[0];
    if (d) return boiteDepot("dsn", d.chemin);
    return boiteProduire(r.mois, async () => {
      const e2 = await api("etat-du-mois", { mois: r.mois });
      const d2 = (e2.fichiers.dsn || [])[0];
      if (d2) await boiteDepot("dsn", d2.chemin);
    });
  }
  if (r.quoi === "dpae") {
    // LA DPAE PRÉPARÉE D'AVANCE (27/09/2026) : son test est passé, le bouton
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
    // Ce qui manque au contrat se tape dans sa carte, sous « Contrat » : on va au mois.
    document.querySelector('.onglets button[data-vue="paie"]').click();
    return chargerMois(r.mois);
  }
  if (r.quoi === "cloture") return boiteCloture(r.annee);
  if (r.quoi === "export-compta") return lancerExportCompta(r.annee);
  if (r.quoi === "revue-spectacles") return boiteRevueSpectacles(r);
}

/* ---- La revue des spectacles, en début d'année (l'éditeur, 30/09/2026,
 * journal §167). Un numéro d'objet vaut tant que le spectacle se joue : on
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
      <span class="pastille ${clos ? "bien" : "alerte"}">${clos ? "clos" : "ouvert"}</span>
      <span class="rang-infos">
        ${x.mois.length} mois · ${x.dsn_acceptees.length} DSN acceptée${x.dsn_acceptees.length === 1 ? "" : "s"}
        ${clos ? ` · clos le ${echapper(quand(clos.le))}, ${echapper(clos.archive)}` : ""}
        ${x.export ? ` · export compta du ${echapper(quand(x.export.ecrit))}` : " · pas d'export compta"}
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


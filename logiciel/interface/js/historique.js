/* L'historique. */
"use strict";

/* =======================================================================
   HISTORIQUE
   =======================================================================
   TOUS LES ENVOIS ET TOUS LES RETOURS SE LISENT ICI, en un seul endroit.
   La page Paie est
   l'endroit où l'on FAIT le mois ; l'Historique, celui où l'on VOIT ce qu'il a
   produit, envoyé et reçu.

   Une ligne par mois. Elle se déplie sur la vue technique du mois, que le
   moteur rend (`historique-mois`) : ses documents, chacun ouvrable ; ses
   envois, tests de ce logiciel compris (repliés, marqués « test ») ; et sous
   chacun ses retours dans l'ordre d'arrivée, LUS par le moteur — l'organisme,
   le statut, le verdict, les anomalies — avec la pièce archivée à ouvrir en
   dernier recours. La page n'y décide rien : elle affiche.

   Le lien vers la page Paie ne reste que pour un mois où il y a encore
   quelque chose à faire (`a_faire`, décidé par le moteur) ; un mois fini se
   lit ici, en entier. */
function dessinerHistorique() {
  dessinerReprise();
  dessinerExercices();
  dessinerRegularisation();
  const cible = document.getElementById("historique");
  if (!ETAT.historique.length) {
    cible.innerHTML = `<p class="discret">Aucun mois pour l'instant.</p>`;
    return;
  }
  // L'état de la DSN, en mots et en couleur : c'est ce qu'on cherche en
  // premier dans un mois passé. Mots et couleur sont au moteur
  // (`bilan_des_mois`, `pastille`).
  const majuscule = (t) => t.charAt(0).toUpperCase() + t.slice(1);
  cible.innerHTML = ETAT.historique.map((m) => {
    const p = m.pastille;
    const couleur = CLASSE_DE_COULEUR[p.couleur] ?? "";
    const mots = p.mots;
    const depose = m.depose_le && m.dsn !== "a_deposer" ? ` le ${quand(m.depose_le).slice(0, 10)}` : "";
    return `
    <details class="mois-hist" data-mois="${echapper(m.mois)}" data-retard="${m.dsn === "en_retard" ? 1 : 0}"
             data-a-faire="${m.a_faire ? 1 : 0}">
      <summary class="rang mois">
        <span class="rang-nom">${echapper(majuscule(moisEnClair(m.mois)))}</span>
        <span class="rang-etat"><span class="pastille ${couleur}">${echapper(mots)}${echapper(depose)}</span>${
          m.clos ? ` <span class="pastille">clos</span>` : ""}</span>
        <span class="rang-infos" title="${echapper((m.fichiers || []).join("\n"))}">${
          (m.sortes || []).length ? echapper(m.sortes.join(" · ")) : "aucun document"}</span>
        <span class="rang-actions">
        </span>
      </summary>
      <div class="mois-vue"><p class="discret">Lecture du mois…</p></div>
    </details>`;
  }).join("");
  cible.querySelectorAll("details.mois-hist").forEach((d) => {
    d.addEventListener("toggle", () => { if (d.open) dessinerMoisHistorique(d); });
  });
}

/** Le mois `mois` déplié dans l'Historique : depuis l'étape d'un mois (« voir les envois et leurs retours ») ou le
 *  bandeau. */
async function ouvrirMoisHistorique(mois) {
  document.querySelector('.onglets button[data-vue="historique"]').click();
  const d = document.querySelector(`details.mois-hist[data-mois="${CSS.escape(mois || "")}"]`);
  if (!d) return;
  d.open = true;
  d.scrollIntoView({ block: "start" });
}


/** La vue technique d'un mois : ses documents, ses envois et leurs retours, tels que le moteur les lit. */
async function dessinerMoisHistorique(d) {
  const mois = d.dataset.mois;
  const vue = d.querySelector(".mois-vue");
  let h;
  try {
    h = await api("historique-mois", { mois });
  } catch (e) {
    vue.innerHTML = `<p class="discret">${echapper(e.message)}</p>`;
    return;
  }
  const paie = d.dataset.aFaire === "1"
    ? `<p><button class="secondaire" data-faire="${echapper(mois)}">Reprendre ce mois dans Paie</button>
       <span class="discret">— il y reste quelque chose à faire</span></p>` : "";

  // Les documents, par sorte, dans l'ordre que le moteur donne.
  const sortes = [];
  for (const doc of h.documents) {
    let s = sortes.find((x) => x.nom === doc.sorte);
    if (!s) sortes.push(s = { nom: doc.sorte, docs: [] });
    s.docs.push(doc);
  }
  const documents = sortes.length ? sortes.map((s) => `
    <div class="docs-sorte"><b>${echapper(s.nom)}</b>${s.docs.map((x) =>
      `<button class="lien" data-piece="${echapper(x.chemin)}" title="${echapper(x.chemin)}">${echapper(x.nom)}</button>`).join("")}
    </div>`).join("") : `<p class="discret">Aucun document écrit pour ce mois.</p>`;

  const envois = h.envois.length ? h.envois.map(dessinerEnvoi).join("")
    : `<p class="discret">Rien n'est parti de ce mois.</p>`;

  vue.innerHTML = `${paie}
    <h3>Les documents</h3>${documents}
    <h3>Les envois et leurs retours</h3>${envois}`;

  vue.querySelectorAll("[data-piece]").forEach((b) => {
    b.onclick = async () => {
      try {
        const r = await api("ouvrir-piece", { historique: mois, chemin: b.dataset.piece });
        if (r && r.phrase) dire(r.phrase);
      }
      catch (e) { dire(e.message, "erreur"); }
    };
  });
  vue.querySelectorAll("[data-faire]").forEach((b) => {
    b.onclick = async () => {
      document.querySelector('.onglets button[data-vue="paie"]').click();
      await chargerMois(mois);
      // Un mois dont la DSN est en retard : elle se fabrique et part, en retard.
      if (d.dataset.retard === "1") boiteDepot({ quoi: "dsn", mois }, { testerDabord: true });
    };
  });
}

/** Un envoi : ce qui est parti, quand, et sous lui ses retours. Un réel s'ouvre ; un test reste replié. */
function dessinerEnvoi(e) {
  const test = e.mode === "test";
  const id = e.identite;
  const retours = e.retours.length ? e.retours.map(dessinerRetour).join("")
    : e.attend_retour ? `<p class="discret retour">Aucun retour pour l'instant.</p>` : "";
  return `
    <details class="envoi ${test ? "test" : "reel"}" ${test ? "" : "open"}>
      <summary><b>${echapper(e.acte)}</b>
        ${test ? `<span class="pastille">test</span>` : ""}
        <span class="discret">— ${echapper(e.resume)}</span></summary>
      ${id ? `<div class="envoi-id">${echapper(id)}</div>` : ""}
      ${retours}
    </details>`;
}

/** Un retour, lu : l'organisme, ce que dit sa pièce, ses anomalies ; la pièce elle-même en dernier recours. */
function dessinerRetour(r) {
  const titre = r.titre || r.libelle || `retour de nature ${r.nature}`;
  const anomalies = (r.anomalies || []).length ? `
    <table class="anomalies"><tr><th>code</th><th>message</th><th>rubrique</th><th>salarié</th></tr>
    ${r.anomalies.map((a) => `<tr><td><code>${echapper(a.code || "")}</code></td>
      <td>${echapper(a.message || "")}${a.categorie ? ` <span class="discret">(${echapper(a.categorie)})</span>` : ""}${
        a.nombre && a.nombre !== "1" ? ` <span class="discret">× ${echapper(a.nombre)}</span>` : ""}</td>
      <td>${echapper(a.rubrique || "")}${a.ligne ? ` <span class="discret">ligne ${echapper(a.ligne)}</span>` : ""}</td>
      <td>${echapper(a.salarie || "")}</td></tr>`).join("")}</table>` : "";
  return `
    <details class="retour">
      <summary><span class="discret">${echapper(r.quand ? quand(r.quand) : "")}</span>
        ${r.organisme ? `${echapper(r.organisme)} — ` : ""}${echapper(titre)}
        ${pastilleDeLEtat(r.pastille)}</summary>
      <div class="retour-corps">
        ${r.nature ? `<p class="discret">Nature ${echapper(r.nature)} : ${echapper(r.libelle || "")}</p>` : ""}
        ${(r.phrases || []).map((p) => `<p>${echapper(p)}</p>`).join("")}
        ${anomalies}
        ${r.piece ? `<p><button class="lien" data-piece="${echapper(r.piece)}">Ouvrir la pièce telle quelle</button>
          <span class="discret">${echapper(r.piece)}</span></p>` : ""}
      </div>
    </details>`;
}

/** LA LISTE DES MOIS SUIT L'ÉTAT GARDÉ. Elle se peint d'abord avec ce
 *  que la page sait ; puis le moteur rend l'état frais de l'ouverture
 *  (`etat_garde`, refait après chaque écriture : `ouverture` avec `frais`), et
 *  la liste se repeint s'il a changé — un dépôt fait depuis l'ouverture s'y
 *  voit sans recharger. La page n'y décide rien : elle relit. Les mois dépliés
 *  le restent. */
async function rafraichirHistorique() {
  let etat;
  try { etat = lectureDe(await appelerMoteur("ouverture", { frais: true }), "etat"); } catch (e) { return; }
  if (!etat || JSON.stringify(etat.historique) === JSON.stringify(ETAT.historique)) return;
  ETAT.historique = etat.historique;
  if (document.getElementById("vue-historique").classList.contains("cachee")) return;
  const ouverts = [...document.querySelectorAll("details.mois-hist[open]")].map((d) => d.dataset.mois);
  dessinerHistorique();
  ouverts.forEach((m) => {
    const d = document.querySelector(`details.mois-hist[data-mois="${CSS.escape(m)}"]`);
    if (d) d.open = true;
  });
}

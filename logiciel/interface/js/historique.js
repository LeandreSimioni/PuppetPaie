/* L'historique. */
"use strict";

/* =======================================================================
   HISTORIQUE
   =======================================================================
   TOUS LES ENVOIS ET TOUS LES RETOURS SE LISENT ICI (Leandre, 20/10/2026 ;
   tickets #219, #220, #222 : « il n'y a pas d'endroit où voir tous les
   retours », « un système simple qui n'envoie pas de bug »). La page Paie est
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
  // L'état de la DSN, en mots : c'est ce qu'on cherche en premier dans un
  // mois passé. La décision est au moteur (`bilan_des_mois`).
  const DSN_ETATS = {
    passee: ["bien", "DSN passée"], attente: ["alerte", "DSN partie, bilan attendu"],
    refusee: ["erreur", "DSN refusée"], a_deposer: ["alerte", "DSN à déposer"],
    a_ecrire: ["", "saisi, pas encore écrit"], en_retard: ["erreur", "DSN en retard"],
    // Un mois tenu avant PuppetPaie (ticket #215) : sa DSN est celle de l'ancien logiciel, ou il n'en a pas trace ici.
    importee: ["bien", "DSN déclarée avant PuppetPaie"], sans_trace: ["alerte", "tenu avant PuppetPaie — aucune trace de DSN"],
  };
  const majuscule = (t) => t.charAt(0).toUpperCase() + t.slice(1);
  cible.innerHTML = ETAT.historique.map((m) => {
    const [couleur, mots] = DSN_ETATS[m.dsn] || ["", m.produit ? "écrit" : "saisi"];
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
          ${m.corrigeable && !m.clos ? `<button class="secondaire" data-corriger-mois="${echapper(m.mois)}"
            title="la correction partira dans la DSN du mois courant, datée de ce mois">Corriger…</button>` : ""}
        </span>
      </summary>
      <div class="mois-vue"><p class="discret">Lecture du mois…</p></div>
    </details>`;
  }).join("");
  cible.querySelectorAll("details.mois-hist").forEach((d) => {
    d.addEventListener("toggle", () => { if (d.open) dessinerMoisHistorique(d); });
  });
  cible.querySelectorAll("[data-corriger-mois]").forEach((b) => {
    b.onclick = (e) => { e.preventDefault(); e.stopPropagation(); boiteCorriger(b.dataset.corrigerMois); };
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

const STATUTS = { OK: "bien", KO: "erreur", ANO: "alerte", "EN COURS": "" };

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
      try { await api("ouvrir-piece", { historique: mois, chemin: b.dataset.piece }); }
      catch (e) { dire(e.message, "erreur"); }
    };
  });
  vue.querySelectorAll("[data-faire]").forEach((b) => {
    b.onclick = async () => {
      document.querySelector('.onglets button[data-vue="paie"]').click();
      await chargerMois(mois);
      // Un mois en retard s'ouvre en le sachant : la boîte dit les pénalités.
      if (d.dataset.retard === "1") boiteRouvrirMois(mois, MOIS_FERME);
    };
  });
}

/** Un envoi : ce qui est parti, quand, et sous lui ses retours. Un réel s'ouvre ; un test reste replié. */
function dessinerEnvoi(e) {
  const test = e.mode === "test";
  const id = [e.fichier && `fichier ${e.fichier}`, e.flux && `flux ${e.flux}`,
              e.destinataire && `à ${e.destinataire}`].filter(Boolean).join(" · ");
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
  const couleur = STATUTS[r.verdict || r.statut] ?? "";
  const statut = r.verdict || r.statut;
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
        ${statut ? `<span class="pastille ${couleur}">${echapper(statut)}</span>` : ""}</summary>
      <div class="retour-corps">
        <p class="discret">Nature ${echapper(r.nature)} : ${echapper(r.libelle || "")}</p>
        ${(r.phrases || []).map((p) => `<p>${echapper(p)}</p>`).join("")}
        ${anomalies}
        ${r.piece ? `<p><button class="lien" data-piece="${echapper(r.piece)}">Ouvrir la pièce telle quelle</button>
          <span class="discret">${echapper(r.piece)}</span></p>` : ""}
      </div>
    </details>`;
}

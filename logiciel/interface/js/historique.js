/* L'historique. */
"use strict";

/* =======================================================================
   HISTORIQUE
   ===================================================================== */
function dessinerHistorique() {
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
  };
  const majuscule = (t) => t.charAt(0).toUpperCase() + t.slice(1);
  cible.innerHTML = ETAT.historique.map((m) => {
    const [couleur, mots] = DSN_ETATS[m.dsn] || ["", m.produit ? "écrit" : "saisi"];
    const depose = m.depose_le && m.dsn !== "a_deposer" ? ` le ${quand(m.depose_le).slice(0, 10)}` : "";
    return `
    <div class="rang mois">
      <span class="rang-nom">${echapper(majuscule(moisEnClair(m.mois)))}</span>
      <span class="rang-etat"><span class="pastille ${couleur}">${echapper(mots)}${echapper(depose)}</span>${
        m.clos ? ` <span class="pastille">clos</span>` : ""}</span>
      <span class="rang-infos" title="${echapper((m.fichiers || []).join("\n"))}">${
        (m.sortes || []).length ? echapper(m.sortes.join(" · ")) : "aucun document"}</span>
      <span class="rang-actions">
        ${m.corrigeable && !m.clos ? `<button class="secondaire" data-corriger-mois="${echapper(m.mois)}"
          title="la correction partira dans la DSN du mois courant, datée de ce mois">Corriger…</button>` : ""}
        <button class="secondaire" data-ouvrir="${echapper(m.mois)}" data-retard="${m.dsn === "en_retard" ? 1 : 0}">Ouvrir</button>
      </span>
    </div>`;
  }).join("");
  cible.querySelectorAll("[data-ouvrir]").forEach((b) => {
    b.onclick = async () => {
      document.querySelector('.onglets button[data-vue="paie"]').click();
      await chargerMois(b.dataset.ouvrir);
      // Un mois en retard s'ouvre en le sachant : la boîte dit les pénalités.
      if (b.dataset.retard === "1") boiteRouvrirMois(b.dataset.ouvrir, MOIS_FERME);
    };
  });
  cible.querySelectorAll("[data-corriger-mois]").forEach((b) => {
    b.onclick = () => boiteCorriger(b.dataset.corrigerMois);
  });
}


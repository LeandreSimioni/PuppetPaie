/* Configuration → Modèle de bulletin. */
"use strict";

/* DEUX MODÈLES, CÔTE À CÔTE, SUR UNE PAIE FICTIVE.

   Au moins deux modèles, un complet et un condensé, et la compagnie choisit :
   dans la configuration, un sélecteur montre une paie fictive sous chaque
   modèle. La compagnie choisit pour tous ; un
   salarié qui demande l'autre l'obtient par une case de sa fiche (Avancé), qui
   l'emporte.

   La page ne décide rien (règle 2) : le moteur dit les modèles et celui qui est
   choisi (`bulletin-modeles`), fait le PDF de la paie fictive sous chacun (la
   fenêtre le sert à l'adresse qu'il donne) et pose le choix
   (`bulletin-modele-choisir`). La paie est celle d'une compagnie fictive livrée
   avec le logiciel : jamais un salarié de la compagnie. */
async function dessinerModelesBulletin(cible) {
  cible.innerHTML = `<p class="note">Les modèles arrivent…</p>`;
  let m;
  try {
    m = await appelerMoteur("bulletin-modeles", {});
  } catch (e) {
    cible.innerHTML = `<div class="encart erreur">${echapper(e.message)}</div>`;
    return;
  }
  const t = Date.now();          // un aperçu refait à chaque visite : le moteur du jour
  const carte = (x) => {
    const choisi = x.cle === m.choisi;
    const adresse = `${x.apercu}&jeton=${encodeURIComponent(JETON)}&t=${t}`;
    return `<div class="modele-bulletin" style="flex:1 1 320px;min-width:0">
      <div style="display:flex;align-items:baseline;gap:8px;flex-wrap:wrap">
        <b>${echapper(x.nom)}</b>
        ${choisi ? `<span class="pastille bien">${m.par_defaut ? "celui de la compagnie (par défaut)" : "celui de la compagnie"}</span>` : ""}
      </div>
      <p class="discret">${echapper(x.description)}</p>
      <iframe class="fictive-pdf-cadre" style="height:560px" title="Aperçu du modèle ${echapper(x.nom)}"
              src="${adresse}#page=1&view=FitH"></iframe>
      <div style="display:flex;gap:12px;align-items:center;margin-top:6px">
        ${choisi ? "" : `<button data-choisir="${echapper(x.cle)}">Choisir ce modèle pour tous</button>`}
        <a class="lien" href="${adresse}" target="_blank" rel="noopener">ouvrir le PDF</a>
      </div>
    </div>`;
  };
  const autrement = (m.salaries_autrement || []).length
    ? `<p class="note">Ces salariés ont demandé l'autre modèle (case de leur fiche, Avancé) et le gardent :
        ${m.salaries_autrement.map((s) => `${echapper(s.nom)} (${echapper(s.modele)})`).join(", ")}.</p>`
    : "";
  cible.innerHTML = `
    <p class="intro">Le bulletin que la compagnie remet à ses salariés. Chaque modèle est montré sur
      la même paie fictive (une compagnie et un salarié inventés) : mêmes lignes du modèle
      réglementaire, mêmes totaux, même net. Un salarié qui demande l'autre modèle l'obtient par la
      case « Modèle de bulletin demandé par le salarié » de sa fiche, dans Avancé.</p>
    <div style="display:flex;gap:16px;flex-wrap:wrap">${m.modeles.map(carte).join("")}</div>
    ${autrement}
    <p class="discret">Le choix vaut pour les bulletins produits ensuite ; ceux déjà remis ne changent
      pas.</p>`;
  cible.querySelectorAll("[data-choisir]").forEach((b) => {
    b.onclick = async () => {
      b.disabled = true;
      try {
        await api("bulletin-modele-choisir", { modele: b.dataset.choisir });
        // La fiche Entreprise garde la case (cachée) : la configuration relue, un enregistrement de la
        // fiche ne la remet pas à l'ancienne valeur.
        ETAT.configuration = await api("configuration", {});
        await dessinerModelesBulletin(cible);
      } catch (e) {
        b.disabled = false;
        dire(e.message, "erreur");
      }
    };
  });
}

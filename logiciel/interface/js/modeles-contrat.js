/* Configuration → Modèles de contrat. */
"use strict";

/* LES CONTRATS QUE LE LOGICIEL FAIT SIGNER, ET CEUX DE LA COMPAGNIE.

   Chaque nature de contrat (l'engagement d'un intermittent, le CDD et le CDI
   d'un permanent) a son modèle livré, qui porte déjà ce que la loi exige,
   rempli de lui-même. Une compagnie qui a son propre contrat le pose ici : il
   est rangé dans son dossier (gabarits/), sauvegardé avec elle, et pris à la
   place du modèle livré. Ses clauses particulières s'écrivent une fois et se
   reprennent dans chaque contrat.

   La page ne décide rien (règle 2) : le moteur dit les modèles, leurs champs,
   ce que la loi exige (`modeles-contrat`), donne le texte du modèle livré
   (`modele-contrat-livre`), vérifie et pose celui de la compagnie
   (`poser-modele-contrat`), le retire (`retirer-modele-contrat`), écrit les
   clauses (`clauses-particulieres`) ; un refus se montre tel qu'il vient. */
async function dessinerModelesContrat(cible) {
  cible.innerHTML = `<p class="note">…</p>`;
  let m;
  try {
    m = await appelerMoteur("modeles-contrat", {});
  } catch (e) {
    cible.innerHTML = `<div class="encart erreur">${echapper(e.message)}</div>`;
    return;
  }
  const piece = (n, x) => `
    <div class="modele-contrat" data-nature="${echapper(n.nature)}" data-quoi="${echapper(x.quoi)}">
      <div style="display:flex;align-items:baseline;gap:8px;flex-wrap:wrap">
        <b>${echapper(x.libelle)}</b>
        ${pastilleDeLEtat(x.pastille)}
        <span class="discret">${echapper(x.nom)}</span>
      </div>
      ${x.refus ? `<div class="encart erreur">${echapper(x.refus)}</div>` : ""}
      <div style="display:flex;gap:12px;align-items:center;flex-wrap:wrap;margin-top:6px">
        <button type="button" class="lien" data-ecrire>${echapper(x.bouton)}</button>
        <label class="lien">poser un fichier<input type="file" accept=".modele,.txt,text/plain" data-fichier hidden></label>
        ${x.propre ? `<button type="button" class="lien" data-retirer>Modèle du logiciel</button>` : ""}
      </div>
      <div class="modele-editeur" hidden>
        <textarea rows="18" spellcheck="false" style="width:100%;font-family:monospace;font-size:12px"></textarea>
        <div style="display:flex;gap:12px;margin-top:6px">
          <button type="button" data-poser>Poser ce modèle</button>
          <button type="button" class="lien" data-annuler>Annuler</button>
        </div>
      </div>
    </div>`;
  const champs = (n) => `
    <details class="modele-champs">
      <summary>${echapper(n.champs_titre)}</summary>
      <table class="tableau">
        <tr><th>Champ</th><th>Ce qui s'y écrit</th><th>La loi l'exige</th></tr>
        ${n.champs.map((c) => `<tr>
          <td><code>${echapper(c.marqueur)}</code></td>
          <td>${echapper(c.quoi)}</td>
          <td>${c.obligatoire ? echapper(c.texte || "oui") : ""}</td></tr>`).join("")}
      </table>
      <p class="discret">${echapper(m.ecran.avenant)}</p>
    </details>`;
  cible.innerHTML = `
    <p class="intro">${echapper(m.ecran.intro)}</p>
    <h3>Clauses particulières</h3>
    <p class="discret">${echapper(m.ecran.clauses)}</p>
    <textarea id="clauses-compagnie" rows="3" style="width:100%">${echapper(m.clauses_particulieres || "")}</textarea>
    <div><button type="button" id="clauses-enregistrer">Enregistrer les clauses</button></div>
    <h3>Modèle de la compagnie</h3>
    <ol class="discret">${m.ecran.comment.map((x) => `<li>${echapper(x)}</li>`).join("")}</ol>
    ${m.natures.map((n) => `
      <h3>${echapper(n.nom.charAt(0).toUpperCase() + n.nom.slice(1))} <span class="discret">— ${echapper(n.libelle)}</span></h3>
      ${n.modeles.map((x) => piece(n, x)).join("")}
      ${champs(n)}`).join("")}`;

  const redessiner = async (r) => {
    if (r && r.natures) await dessinerModelesContrat(cible);
  };
  document.getElementById("clauses-enregistrer").onclick = async (ev) => {
    ev.target.disabled = true;
    try {
      const r = await api("clauses-particulieres", { texte: document.getElementById("clauses-compagnie").value });
      ETAT.configuration = await api("configuration", {}); peindreEtatGlobal();
      dire(r.phrase);
    } catch (e) {
      dire(e.message, "erreur");
    } finally {
      ev.target.disabled = false;
    }
  };
  const poser = async (bloc, texte) => {
    try {
      const r = await api("poser-modele-contrat", { nature: bloc.dataset.nature, quoi: bloc.dataset.quoi, texte });
      await redessiner(r);
      dire(r.phrase);
    } catch (e) {
      dire(e.message, "erreur");
    }
  };
  cible.querySelectorAll(".modele-contrat").forEach((bloc) => {
    const n = m.natures.find((x) => x.nature === bloc.dataset.nature);
    const x = n.modeles.find((y) => y.quoi === bloc.dataset.quoi);
    const editeur = bloc.querySelector(".modele-editeur");
    bloc.querySelector("[data-ecrire]").onclick = async () => {
      let texte = x.texte;
      if (!x.propre) {
        try {
          texte = (await appelerMoteur("modele-contrat-livre", { nature: n.nature, quoi: x.quoi })).texte;
        } catch (e) {
          dire(e.message, "erreur");
          return;
        }
      }
      editeur.querySelector("textarea").value = texte || "";
      editeur.hidden = false;
    };
    bloc.querySelector("[data-annuler]").onclick = () => { editeur.hidden = true; };
    bloc.querySelector("[data-poser]").onclick = () => poser(bloc, editeur.querySelector("textarea").value);
    bloc.querySelector("[data-fichier]").onchange = (ev) => {
      const f = ev.target.files[0];
      if (!f) return;
      const l = new FileReader();
      l.onload = () => poser(bloc, String(l.result));
      l.readAsText(f, "utf-8");
    };
    const retirer = bloc.querySelector("[data-retirer]");
    if (retirer) {
      retirer.onclick = async () => {
        try {
          await redessiner(await api("retirer-modele-contrat", { nature: n.nature, quoi: x.quoi }));
        } catch (e) {
          dire(e.message, "erreur");
        }
      };
    }
  });
}

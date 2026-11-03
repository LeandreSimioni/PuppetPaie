/* La reprise du journal des dépôts. Les arrêts de travail et les fins de contrat sont dans js/imprevus.js. */
"use strict";

/* ------------------------------------------- la reprise du journal
   Ce qui est parti sans passer par le journal des dépôts — par un autre
   logiciel, par le site d'un organisme — n'y figure pas, et l'écran dirait
   « rien n'est parti » sur des actes qui ont eu lieu.

   Ce bloc rend au journal ce que les traces de dépôt attestent, en tête de l'Historique :
   les envois et leurs retours s'y lisent). Il n'apparaît QUE s'il y a quelque chose
   à reprendre : une fois le journal à jour, il disparaît et ne revient que si
   un nouveau dépôt atteste d'un chemin qui n'est pas passé par ici.

   DEUX TEMPS, comme partout ailleurs : on montre d'abord ce qui serait écrit,
   ligne par ligne, et on n'écrit que sur un second clic. Une reprise qui part
   au premier appel est une reprise qu'on lance « pour voir » et qui laisse
   des lignes que personne n'a relues. */
async function dessinerReprise() {
  const cible = document.getElementById("reprise");
  if (!cible) return;
  let r;
  try {
    r = await api("journal-reprise", {});
  } catch (e) {
    cible.innerHTML = "";
    return;
  }
  if (!r.ajoutes.length) { cible.innerHTML = ""; return; }

  const reels = r.ajoutes.filter((v) => v.mode === "reel" && v.accepte);
  // Replié en une ligne, sauf s'il y a un dépôt RÉEL accepté à reprendre :
  // des essais retrouvés sont un historique à ranger, pas une urgence.
  cible.innerHTML = `
    <details class="reprise" ${reels.length ? "open" : ""}>
    <summary>${echapper(r.ecran.resume)}</summary>
    <div class="encart">
      <b>${echapper(r.ecran.titre)}</b>
      ${echapper(r.ecran.explication)}
      ${r.ecran.reels ? `<br><b>${echapper(r.ecran.reels)}</b>` : ""}
    </div>
    <div class="liste-choix">
      ${r.ajoutes.map((v) => `
        <div class="choix"><span>
          <b>${echapper(v.dit)}</b>
          ${pastilleDeLEtat(v.pastille)}
          <br><span class="discret">${echapper(v.origine_dit)}</span>
        </span></div>`).join("")}
    </div>
    <button class="secondaire" id="reprendre">${echapper(r.ecran.bouton)}</button>
    <p class="note">${echapper(r.ecran.logiciel)}</p>
    </details>`;

  document.getElementById("reprendre").onclick = async () => {
    document.getElementById("reprendre").disabled = true;
    try {
      const f = await api("journal-reprise", { ecrire: true });
      dire(f.phrase);
      // La reprise remet des envois au journal : l'Historique, où elle se montre, se redessine avec eux.
      await rafraichirActions();
      dessinerHistorique();
    } catch (e) {
      document.getElementById("reprendre").disabled = false;
      dire(e.message, "erreur");
    }
  };
}


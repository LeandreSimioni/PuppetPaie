/* La reprise du journal des dépôts. Les arrêts de travail et les fins de contrat sont dans js/imprevus.js. */
"use strict";

/* ------------------------------------------- la reprise du journal
   Le journal des dépôts est né le 11/09/2026 au soir. Tout ce qui est parti
   avant — par un autre logiciel, par le site, ou par nos propres dépôts de la veille —
   n'y figure pas, et l'écran affiche donc « rien n'est parti » sur des actes
   qui ont bel et bien eu lieu. Devant la liste des embauches :

     « je comprends pas pourquoi il veut que je renvoie ça alors qu'on l'a
       déjà envoyé et on a les traces de dépôt il me semble »

   Ce bloc lui rend ce qu'il savait, en tête de l'Historique (20/10/2026 : tous
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
    <summary>${r.ajoutes.length} dépôt${r.ajoutes.length > 1 ? "s" : ""} retrouvé${r.ajoutes.length > 1 ? "s" : ""}
      dans les retours, absent${r.ajoutes.length > 1 ? "s" : ""} du journal${reels.length
        ? ` — <b>dont ${reels.length} réel${reels.length > 1 ? "s" : ""}</b>` : ""}</summary>
    <div class="encart">
      <b>${r.ajoutes.length} dépôt(s) attestés par les retours, absents du journal</b>
      Le journal des dépôts est né le 11/09/2026 ; ce qui est parti avant ne
      s'y est pas écrit. Les accusés d'enregistrement reçus des organismes,
      eux, s'en souviennent. Les reprendre ne dépose
      rien et n'envoie rien : ça remet au journal ce qui a déjà eu lieu.
      ${reels.length ? `<br><b>${reels.length} sont des dépôts RÉELS acceptés.</b>` : ""}
    </div>
    <div class="liste-choix">
      ${r.ajoutes.map((v) => `
        <div class="choix"><span>
          <b>${echapper(quand(v.quand))}</b>
          — ${echapper(v.mois ? moisEnClair(v.mois) : "mois inconnu")}
          ${pastilleDeLEtat(v.pastille)}
          <br><span class="discret">déposé par ${echapper(v.logiciel || "logiciel inconnu")}
            · flux ${echapper(v.flux)}</span>
        </span></div>`).join("")}
    </div>
    <button class="secondaire" id="reprendre">Reprendre ces ${r.ajoutes.length} dépôt(s)</button>
    <p class="note">Le nom du logiciel n'est pas une annotation : il est lu
      dans le certificat de conformité. C'est lui qui distingue le dépôt d'un
      autre logiciel d'un dépôt PuppetPaie.</p>
    </details>`;

  document.getElementById("reprendre").onclick = async () => {
    document.getElementById("reprendre").disabled = true;
    try {
      const f = await api("journal-reprise", { ecrire: true });
      dire(f.ajoutes.length + " dépôt(s) remis au journal"
           + (f.rattaches ? ", " + f.rattaches + " retour(s) rattachés." : "."));
      // La reprise remet des envois au journal : l'Historique, où elle se montre, se redessine avec eux.
      await rafraichirActions();
      dessinerHistorique();
    } catch (e) {
      document.getElementById("reprendre").disabled = false;
      dire(e.message, "erreur");
    }
  };
}


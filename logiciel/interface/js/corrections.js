/* Les corrections des mois déposés. */
"use strict";

/* Un engagement d'un mois déposé, modifié après la veille de l'échéance, ne
   refait plus la DSN de son mois : sa correction s'écrit d'elle-même et part
   dans la DSN d'un mois suivant, datée du mois d'origine (cahier technique
   2026.1 §2.2.2.5). Cet écran, dans l'Historique, montre les corrections
   écrites ; une correction pas encore portée s'annule. */
async function dessinerRegularisation() {
  const cible = document.getElementById("regularisation");
  const titre = document.getElementById("titre-corrections");
  let r;
  try { r = await api("regularisations"); }
  catch (e) { cible.innerHTML = `<p class="discret">${echapper(e.message)}</p>`; return; }
  // Les mois qui se corrigent et ceux en retard se disent sur leur ligne,
  // dans « Les mois » : il ne reste ici que les corrections ÉCRITES, et le
  // titre se tait quand il n'y en a pas.
  const ecrites = r.corrections || [];
  if (titre) titre.hidden = !ecrites.length;
  cible.innerHTML = ecrites.map((m) => `
      <div class="corrections-mois">
        <b>${echapper(moisEnClair(m.mois))}</b>
        ${m.corrections.map((c) => `
          <div class="rang">
            <span class="rang-nom">n° ${c.numero}</span>
            <span class="pastille ${c.etat === "portee" ? "bien" : c.etat === "annulee" ? "" : "alerte"}">${echapper(c.etat)}</span>
            <span class="rang-infos">${echapper(c.salarie)}, ${echapper(jjmmaaaa(c.debut))} —
              ${echapper(c.motif)} <span class="discret">(${echapper(c.decidee_le)}, ${echapper(c.qui)})</span>
              ${c.portee_par ? ` · portée par la DSN ${echapper(deMois(c.portee_par.mois))}` : ""}
              ${!c.portee_par && c.preparee_par ? ` · dans la DSN ${echapper(deMois(c.preparee_par.mois))}, pas encore déposée` : ""}</span>
            ${c.portee_par || c.annulee_le ? ""
              : `<button class="secondaire" data-annuler="${echapper(m.mois)}:${c.numero}">Annuler</button>`}
          </div>`).join("")}
      </div>`).join("");
  cible.querySelectorAll("[data-annuler]").forEach((b) => {
    b.onclick = async () => {
      const [mois, numero] = b.dataset.annuler.split(":");
      const preuve = r.mot_de_passe_pose === false
        ? null : await demanderMotDePasse("annuler la correction n° " + numero);
      if (r.mot_de_passe_pose !== false && !preuve) return;
      try {
        await api("regularisation-annuler", { mois, numero: +numero, motdepasse: preuve });
        dire("Correction n° " + numero + " annulée — la prochaine DSN ne l'embarque plus.");
        await dessinerRegularisation();
      } catch (e) { dire(e.message, "erreur"); }
    };
  });
}


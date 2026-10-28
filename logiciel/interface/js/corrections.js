/* Les corrections des mois déposés. */
"use strict";

/* Un engagement d'un mois déposé, modifié puis revalidé après la veille de
   l'échéance, ne refait plus la DSN de son mois : sa correction part dans la
   DSN d'un mois suivant, datée du mois d'origine (cahier technique 2026.1
   §2.2.2.5). Cet écran, dans l'Historique, montre pour chaque contrat la paie
   déclarée et la paie validée : celles à porter, puis celles déjà portées.
   Corriger, c'est modifier l'engagement et le revalider. */
async function dessinerRegularisation() {
  const cible = document.getElementById("regularisation");
  const titre = document.getElementById("titre-corrections");
  let r;
  try { r = await api("regularisations"); }
  catch (e) { cible.innerHTML = `<p class="discret">${echapper(e.message)}</p>`; return; }
  // Les mois qui se corrigent et ceux en retard se disent sur leur ligne,
  // dans « Les mois » : il ne reste ici que les corrections, et le titre se
  // tait quand il n'y en a pas.
  const mois = r.corrections || [];
  if (titre) titre.hidden = !mois.length;
  cible.innerHTML = mois.map((m) => `
      <div class="corrections-mois">
        <b>${echapper(moisEnClair(m.mois))}</b>
        ${m.corrections.map((c) => `
          <div class="rang">
            <span class="rang-nom">${echapper(c.nom)}</span>
            <span class="pastille ${c.portee_par ? "bien" : "alerte"}">${echapper(c.etat)}</span>
            <span class="rang-infos">contrat ${echapper(c.numero || "")}, ${echapper(jjmmaaaa(c.debut))} —
              brut ${echapper(c.avant.brut)} → ${echapper(c.apres.brut)},
              net ${echapper(c.avant.net_a_payer)} → ${echapper(c.apres.net_a_payer)}
              ${c.dates ? `<span class="discret">${echapper(c.dates)}</span>` : ""}
              ${c.portee_par ? ` · portée par la DSN ${echapper(deMois(c.portee_par))}` : " · part dans la prochaine DSN"}</span>
          </div>`).join("")}
      </div>`).join("");
}

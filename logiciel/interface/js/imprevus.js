/* Les imprévus du mois : les suspensions du contrat, et ce qu'un arrêt appelle. */
"use strict";

/* ------------------------------------------------ LES IMPRÉVUS (15/10/2026)
   Un arrêt de travail ou une suspension (congé parental, sabbatique, sans
   solde…) se saisit UNE FOIS, sur le salarié : le moteur en tire la ligne de
   paie de chaque mois qu'il traverse et le bloc de chaque DSN (60 ou 65).
   La saisie du mois n'a plus d'absence à taper.

   Ici, le strict nécessaire en attendant la porte « un imprévu » : les
   suspensions qui croisent le mois (en saisir une, la prolonger, la
   retirer), et, sur un arrêt, ce que ses faits appellent — le maintien de
   salaire du mois quand le contrat est payé au mois (`maintien_ouvert`), les
   jours déjà indemnisés quand le motif est une maladie ou un accident
   (`jours_deja_indemnises_attendus`), et les cachets payés pendant l'arrêt.
   Rien de remplissable s'il n'y a pas à le remplir : c'est le moteur qui
   dit ce que chaque fait ouvre. */
let IMPREVUS = null;   // la dernière lecture `imprevus` du mois

async function dessinerImprevus(mois) {
  const cible = document.getElementById("imprevus");
  if (!cible) return;
  try { IMPREVUS = await api("imprevus", { mois }); }
  catch (e) { cible.innerHTML = `<p class="discret">${echapper(e.message)}</p>`; return; }
  const tous = IMPREVUS.imprevus || [];
  const suspensions = tous.filter((i) => i.genre === "suspension");
  const arrets = tous.filter((i) => i.genre === "arret" && (i.maintien_ouvert || (i.cachets || []).length));
  const cachets = (i) => (i.cachets || []).map((c) =>
    `<div class="note attention">${echapper(c.libelle)} payé(s) pendant l'imprévu, le
       ${c.dates.map((d) => echapper(jjmmaaaa(d))).join(", ")} (${echapper(deMois(c.mois))}) :
       à retirer de la ligne, ou corriger les dates.</div>`).join("");
  cible.innerHTML = `
    ${suspensions.map((s) => `
      <div class="rang">
        <span class="rang-nom">${echapper(s.salarie)}</span>
        <span class="rang-infos">${echapper(s.motif)} ${echapper(s.motif_libelle)} — du
          ${echapper(jjmmaaaa(s.debut))} au ${echapper(jjmmaaaa(s.fin))} · contrat ${echapper(s.contrat)}
          ${s.reporte_par.length ? ` · déclarée par la DSN ${s.reporte_par.map(deMois).join(", ")}` : ""}</span>
        <button class="lien" data-fin="${echapper(s.salarie)}:${echapper(s.debut)}">Changer la fin…</button>
        ${s.reporte_par.length ? "" : `<button class="lien" data-oublier="${echapper(s.salarie)}:${echapper(s.debut)}">Retirer</button>`}
        ${cachets(s)}
      </div>`).join("")}
    ${arrets.map((a) => `
      <div class="rang">
        <span class="rang-nom">${echapper(a.salarie)}</span>
        <span class="rang-infos">arrêt (${echapper(a.motif_libelle)}) du ${echapper(jjmmaaaa(a.premier_jour))}
          au ${echapper(jjmmaaaa(a.dernier_jour))}
          ${a.maintien_ouvert ? ` · maintien de ${echapper(deMois(mois))} :
            ${a.maintien[mois] != null ? echapper(String(a.maintien[mois])) + " €" : "non saisi"}` : ""}</span>
        ${a.maintien_ouvert && a.mois_du_maintien.includes(mois)
          ? `<button class="lien" data-maintien="${echapper(a.salarie)}:${echapper(a.dernier_jour_travaille)}">Maintien…</button>` : ""}
        ${cachets(a)}
      </div>`).join("")}
    <button class="lien" id="ajouter-suspension">+ une suspension du contrat</button>`;
  const titre = document.getElementById("titre-imprevus");
  if (titre) titre.hidden = !(suspensions.length || arrets.length);
  document.getElementById("ajouter-suspension").onclick = () => boiteSuspension();
  cible.querySelectorAll("[data-fin]").forEach((b) => {
    const [salarie, debut] = b.dataset.fin.split(":");
    b.onclick = () => boiteFinSuspension(salarie, debut);
  });
  cible.querySelectorAll("[data-oublier]").forEach((b) => {
    const [salarie, debut] = b.dataset.oublier.split(":");
    b.onclick = async () => {
      try {
        await api("suspension-oublier", { salarie, debut });
        dire("Suspension retirée — aucune DSN ne l'avait déclarée.");
        await dessinerImprevus(MOIS_AFFICHE);
      } catch (e) { dire(e.message, "erreur"); }
    };
  });
  cible.querySelectorAll("[data-maintien]").forEach((b) => {
    const [salarie, jour] = b.dataset.maintien.split(":");
    b.onclick = () => boiteMaintien(mois, arrets.find((a) => a.salarie === salarie && a.dernier_jour_travaille === jour));
  });
}

/** Saisir une suspension : le salarié, ses deux dates, son motif. Le contrat
 *  suspendu se lit au moteur (les contrats qui couvrent le premier jour) ; il
 *  ne se demande que si deux contrats le couvrent. */
function boiteSuspension() {
  const motifs = (IMPREVUS && IMPREVUS.motifs_suspension) || [];
  const salaries = (ETAT && ETAT.salaries) || [];
  ouvrirBoite(`
    <h3>Une suspension du contrat</h3>
    <p class="note">Saisie une fois : chaque mois qu'elle traverse la déclare
      (bloc 65) et, si elle n'est pas payée, en retire le salaire.</p>
    <label class="champ-boite">Salarié
      <select id="su-salarie">
        ${salaries.map((s) => `<option value="${echapper(s.cle)}">${echapper(s.libelle)}</option>`).join("")}
      </select></label>
    <label class="champ-boite">Du <input type="date" id="su-debut"></label>
    <label class="champ-boite">Au <input type="date" id="su-fin"></label>
    <div id="su-contrats" class="note"></div>
    <label class="champ-boite">Motif
      <select id="su-motif">
        ${motifs.map((m) => `<option value="${echapper(m.code)}">${echapper(m.code)} — ${echapper(m.libelle)}</option>`).join("")}
      </select></label>
    <div class="actions">
      <button class="secondaire" onclick="fermerBoite()">Annuler</button>
      <button class="primaire" id="su-ok">Enregistrer</button>
    </div>`);
  let contrats = [];
  const relire = async () => {
    const salarie = document.getElementById("su-salarie").value;
    const jour = document.getElementById("su-debut").value;
    const bloc = document.getElementById("su-contrats");
    if (!salarie || !jour) { bloc.innerHTML = ""; return; }
    try { contrats = (await api("arret-contrats", { salarie, jour })).contrats || []; }
    catch (e) { bloc.innerHTML = `<span class="erreur">${echapper(e.message)}</span>`; return; }
    bloc.innerHTML = contrats.length > 1
      ? `Deux contrats couvrent ce jour-là — dire lequel :
         <select id="su-contrat">${contrats.map((c) =>
           `<option value="${echapper(c.numero)}">${echapper(c.numero)}, depuis le ${echapper(jjmmaaaa(c.debut))}</option>`).join("")}</select>`
      : "";
  };
  document.getElementById("su-salarie").onchange = relire;
  document.getElementById("su-debut").onchange = relire;
  document.getElementById("su-ok").onclick = async () => {
    const choisi = document.getElementById("su-contrat");
    try {
      await api("suspension-enregistrer", {
        salarie: document.getElementById("su-salarie").value,
        debut: document.getElementById("su-debut").value,
        fin: document.getElementById("su-fin").value,
        motif: document.getElementById("su-motif").value,
        ...(choisi ? { contrat: choisi.value } : {}),
      });
      fermerBoite();
      dire("Suspension enregistrée. Chaque mois qu'elle traverse la déclarera.");
      await dessinerImprevus(MOIS_AFFICHE);
    } catch (e) { dire(e.message, "erreur"); }
  };
}

function boiteFinSuspension(salarie, debut) {
  ouvrirBoite(`
    <h3>La fin de la suspension</h3>
    <label class="champ-boite">Jusqu'au <input type="date" id="sf-fin"></label>
    <div class="actions">
      <button class="secondaire" onclick="fermerBoite()">Annuler</button>
      <button class="primaire" id="sf-ok">Enregistrer</button>
    </div>`);
  document.getElementById("sf-ok").onclick = async () => {
    try {
      await api("suspension-modifier", { salarie, debut, fin: document.getElementById("sf-fin").value });
      fermerBoite();
      dire("Fin de la suspension changée.");
      await dessinerImprevus(MOIS_AFFICHE);
    } catch (e) { dire(e.message, "erreur"); }
  };
}

/** Le maintien de salaire d'un arrêt pour ce mois : le complément versé après
 *  les indemnités journalières (D1226-5), et — seulement si le moteur
 *  l'attend — les jours déjà indemnisés dans les douze mois (D1226-4). */
function boiteMaintien(mois, a) {
  ouvrirBoite(`
    <h3>Le maintien de salaire de ${echapper(deMois(mois))}</h3>
    <p class="note">Le complément versé au titre de l'arrêt, une fois déduites
      les indemnités journalières et la prévoyance : il ne se devine pas.</p>
    <label class="champ-boite">Montant
      <input type="number" step="0.01" min="0" id="ma-montant" value="${echapper(a.maintien[mois] ?? "")}"></label>
    ${a.jours_deja_indemnises_attendus ? `<label class="champ-boite">Jours déjà indemnisés
      <span class="discret">— dans les douze mois avant l'arrêt</span>
      <input type="number" step="1" min="0" id="ma-deja" value="${echapper(a.jours_deja_indemnises ?? "")}"></label>` : ""}
    <div class="actions">
      <button class="secondaire" onclick="fermerBoite()">Annuler</button>
      <button class="primaire" id="ma-ok">Enregistrer</button>
    </div>`);
  document.getElementById("ma-ok").onclick = async () => {
    const deja = document.getElementById("ma-deja");
    try {
      await api("arret-maintien", {
        salarie: a.salarie, dernier_jour: a.dernier_jour_travaille, mois,
        montant: document.getElementById("ma-montant").value,
        ...(deja ? { jours_deja_indemnises: deja.value } : {}),
      });
      fermerBoite();
      dire("Maintien noté : la paie du mois le portera.");
      await dessinerImprevus(MOIS_AFFICHE);
    } catch (e) { dire(e.message, "erreur"); }
  };
}

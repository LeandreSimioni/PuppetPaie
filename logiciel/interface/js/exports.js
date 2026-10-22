/* Les exports : etats, dossier du salarie, CASC, livre, cloture, export compta. */
"use strict";

/* =======================================================================
   EXPORTS  (22/09/2026)
   -----------------------------------------------------------------------
   « Il faut vraiment un endroit où on peut faire des exports, pour les
   trucs comptables. » Le voici, et il ne porte que quatre questions :
   QUOI (l'état), QUAND (un mois, une année, toutes les années), COMMENT
   (le regroupement et le format), et un APERÇU avant d'écrire.

   Tout ce qui s'affiche ici vient du moteur : les états, les formats, les
   regroupements et les périodes sont rendus par `exports-periodes`. Un état
   ajouté demain dans `operations_exports.ETATS` apparaît dans ce menu sans
   qu'on touche à ce fichier — c'est la règle 2 vue depuis la page : elle
   n'affiche pas ce qu'elle croit savoir, elle affiche ce que le moteur dit.

   Rien ne part d'ici. Pas de mot de passe, donc, et pas de confirmation :
   un export lit, et le pire qu'il puisse faire est d'écrire un fichier de
   plus dans `exports/`.
   ===================================================================== */
let EXPORTS = null;          // ce que `exports-periodes` a rendu

async function dessinerExports() {
  const cible = document.getElementById("exports-formulaire");
  try { EXPORTS = await api("exports-periodes"); }
  catch (e) { cible.innerHTML = `<p class="discret">${echapper(e.message)}</p>`; return; }
  const p = EXPORTS;
  if (!p.mois.length) {
    cible.innerHTML = `<p class="discret">Aucune paie saisie : il n'y a rien à
      exporter pour l'instant.</p>`;
    document.getElementById("exports-resultat").innerHTML = "";
    return;
  }
  const opt = (v, t, sel) =>
    `<option value="${echapper(v)}"${sel ? " selected" : ""}>${echapper(t)}</option>`;
  // Un format dont la brique manque se choisit quand même — le moteur écrit
  // alors le CSV et le dit —, mais l'étiquette prévient avant le clic.
  // D'ABORD LA CATEGORIE, PUIS L'ETAT, PUIS SES QUESTIONS (Leandre,
  // 08/10/2026) : rien n'est choisi d'office, et chaque choix deroule le
  // suivant. Une categorie d'un seul etat le choisit d'elle-meme.
  const fams = familles(p.etats);
  cible.innerHTML = `
    <div class="champs">
      <div class="champ">
        <label for="x-famille">Quoi</label>
        <select id="x-famille">
          <option value="" selected disabled>— choisir —</option>
          ${fams.map(([f], i) => opt(String(i), f)).join("")}
        </select>
      </div>
      <div class="champ cachee" id="x-etat-champ">
        <label for="x-etat">Lequel</label>
        <select id="x-etat"></select>
      </div>
    </div>
    <div id="x-suite"></div>`;
  const choixEtat = document.getElementById("x-etat");
  document.getElementById("x-famille").onchange = () => {
    const [, liste] = fams[Number(document.getElementById("x-famille").value)];
    document.getElementById("x-etat-champ").classList.remove("cachee");
    choixEtat.innerHTML = (liste.length > 1 ? `<option value="" selected disabled>— choisir —</option>` : "")
      + liste.map((e) => opt(e.etat, e.quoi)).join("");
    document.getElementById("x-suite").innerHTML = "";
    document.getElementById("exports-resultat").innerHTML = "";
    if (liste.length === 1) dessinerQuestions(opt);
  };
  choixEtat.onchange = () => dessinerQuestions(opt);
  dessinerDossier();
}

/* ------------------------------------------- le dossier d'un salarié */

/** TOUS SES DOCUMENTS, EN UNE FOIS (l'éditeur, 23/09/2026). On coche ce qu'on
 *  sort : une catégorie donne un PDF, plusieurs un ZIP d'un PDF par
 *  catégorie. Le moteur réédite ce qui se recrée — aux numéros lus, jamais
 *  neufs — et prend tel quel ce qui ne se recrée pas (contrat signé, AER,
 *  CoolPaie). La page ne décide rien : elle coche, le moteur répond. */
let DOSSIER = null;
async function dessinerDossier() {
  const cible = document.getElementById("dossier-formulaire");
  if (!cible) return;
  try { DOSSIER = await api("dossier-formes"); }
  catch (e) { cible.innerHTML = `<p class="discret">${echapper(e.message)}</p>`; return; }
  const f = DOSSIER;
  const opt = (v, t) => `<option value="${echapper(v)}">${echapper(t)}</option>`;
  cible.innerHTML = `
    <div class="champs">
      <div class="champ">
        <!-- QUI SE COCHE, COMME DANS LES PAIES (Leandre, 08/10/2026) : une
             case par salarié, et une case « Tous ». Un dossier chacun. -->
        <label>Qui</label>
        <div class="d-qui-liste" style="display:flex;flex-direction:column;gap:2px;max-height:12em;overflow:auto">
          <label class="coche"><input type="checkbox" id="d-tous" checked> <b>Tous</b></label>
          ${[...f.salaries.filter((s) => s.actif), ...f.salaries.filter((s) => !s.actif)].map((s) => `<label class="coche">
            <input type="checkbox" class="d-sal" value="${echapper(s.cle)}" checked>
            ${echapper(s.nom)}${s.actif ? "" : ' <span class="discret">— archivé</span>'}</label>`).join("")}
        </div>
      </div>
      <div class="champ">
        <label for="d-quand">Quand</label>
        <select id="d-quand">
          ${opt("toutes", "Toute la période")}${opt("annee", "Une année")}${opt("mois", "Un mois")}
        </select>
        <select id="d-periode" class="cachee"></select>
      </div>
      <div class="champ pleine">
        <label>Quoi <button type="button" class="lien d-tout">tout décocher</button></label>
        <div class="d-cats">${f.categories.map((c) => `<label class="coche">
          <input type="checkbox" class="d-cat" value="${echapper(c.categorie)}" checked>
          ${echapper(c.quoi)}</label>`).join("")}</div>
        <div class="aide">Une seule case : un PDF. Plusieurs : un ZIP, un PDF par catégorie.</div>
      </div>
      <div class="champ pleine">
        <label class="coche"><input type="checkbox" id="d-ensemble">
          Tous les salariés dans le même fichier <span class="discret">(ne s'envoie à personne)</span></label>
      </div>
    </div>
    <div class="actions">
      <button class="secondaire" id="d-apercu">Aperçu</button>
      <button class="primaire" id="d-ecrire">Écrire le dossier</button>
      ${f.boite_liee ? `<button class="secondaire" id="d-envoyer">L'envoyer à chaque salarié</button>` : ""}
      <button class="lien" id="d-ouvrir" title="${echapper(f.dossier)}">Ouvrir le dossier</button>
    </div>`;
  // « Tous » coche ou décoche chacun ; chacun remet « Tous » d'accord.
  const tous = document.getElementById("d-tous");
  const sals = [...cible.querySelectorAll(".d-sal")];
  tous.onchange = () => sals.forEach((c) => { c.checked = tous.checked; });
  sals.forEach((c) => { c.onchange = () => { tous.checked = sals.every((x) => x.checked); }; });
  const tout = cible.querySelector(".d-tout");
  tout.onclick = () => {
    const cases = [...cible.querySelectorAll(".d-cat")];
    const cocher = cases.every((c) => !c.checked);
    cases.forEach((c) => { c.checked = cocher; });
    tout.textContent = cocher ? "tout décocher" : "tout cocher";
  };
  document.getElementById("d-quand").onchange = () => {
    const quand = document.getElementById("d-quand").value;
    const sel = document.getElementById("d-periode");
    sel.classList.toggle("cachee", quand === "toutes");
    const liste = quand === "mois" ? f.mois.slice().reverse() : f.annees;
    sel.innerHTML = liste.map((v) => opt(v, quand === "mois" ? moisEnClair(v) : v)).join("");
  };
  // VENU D'UNE FICHE SALARIE (« Exporter ses documents », 23/09/2026) : ce
  // salarie, l'annee la plus recente, toutes les categories cochees.
  if (DOSSIER_PRECHOIX && f.salaries.some((s) => s.cle === DOSSIER_PRECHOIX)) {
    sals.forEach((c) => { c.checked = c.value === DOSSIER_PRECHOIX; });
    tous.checked = sals.every((x) => x.checked);
    if (f.annees.length) {
      document.getElementById("d-quand").value = "annee";
      document.getElementById("d-quand").onchange();
      document.getElementById("d-periode").value = [...f.annees].map(String).sort().pop();
    }
    document.getElementById("carte-dossier").open = true;
    cible.scrollIntoView({ behavior: "smooth", block: "start" });
  }
  DOSSIER_PRECHOIX = null;
  document.getElementById("d-apercu").onclick = () => lancerDossier("dossier-salarie", true);
  document.getElementById("d-ecrire").onclick = () => lancerDossier("dossier-salarie", false);
  if (f.boite_liee) document.getElementById("d-envoyer").onclick = () => lancerDossier("dossier-envoyer", false);
  document.getElementById("d-ouvrir").onclick = async () => {
    await ouvrirEtDire("ouvrir-exports", { sous: "dossiers" });
  };
}

async function lancerDossier(route, apercu) {
  const coches = [...document.querySelectorAll(".d-sal:checked")].map((c) => c.value);
  if (!coches.length) { dire("Cochez au moins un salarié.", "erreur"); return; }
  // Tous cochés : la liste vide, qui veut dire « chaque salarié » pour le moteur.
  const qui = document.getElementById("d-tous").checked ? [] : coches;
  const quand = document.getElementById("d-quand").value;
  const periode = document.getElementById("d-periode").value;
  const charge = {
    salaries: qui,
    categories: [...document.querySelectorAll(".d-cat:checked")].map((c) => c.value),
    ensemble: document.getElementById("d-ensemble").checked,
    apercu,
  };
  if (quand === "mois") charge.mois = periode;
  else if (quand === "annee") charge.annee = periode;
  else charge.toutes_les_annees = true;
  if (route === "dossier-envoyer") {
    const mdp = await demanderMotDePasse("envoyer leur dossier aux salariés", { envoi: true });
    if (!mdp) return;
    charge.motdepasse = mdp;
  }
  const cible = document.getElementById("dossier-resultat");
  cible.innerHTML = `<p class="discret">…</p>`;
  const repere = route === "dossier-envoyer" ? await repereCourrielsFictifs() : null;
  let r;
  try { r = await api(route, charge); }
  catch (e) { cible.innerHTML = `<div class="encart erreur">${echapper(e.message)}</div>`; return; }
  if (r.envoi) fenetreCourrielsFictifs("documents", repere);
  const lignes = r.salaries.map((c) => `<tr><td>${echapper(c)}</td><td>${
    Object.entries(r.pages[c] || {}).map(([k, n]) => echapper(r.libelles[k]) + " : " + n + " p.").join(" · ")
  }</td></tr>`).join("");
  cible.innerHTML = `
    <h3>Dossier — ${echapper(r.periode)}</h3>
    ${r.refus.map((a) => `<div class="encart alerte">${echapper(a)}</div>`).join("")}
    ${r.avertissements.map((a) => `<div class="encart">${echapper(a)}</div>`).join("")}
    <table class="tableau"><tr><th>Salarié</th><th>Pages</th></tr>${lignes}</table>
    ${r.fichiers.length ? `<p class="note">Écrit : ${r.fichiers.map((x) => `<code>${echapper(x)}</code>`).join(", ")}</p>` : ""}
    ${r.envoi ? `<div class="encart ok">${r.envoi.messages} dossier(s) envoyé(s) à
      ${echapper(r.envoi.partis.join(", "))}.</div>` : ""}`;
}

/** Les états rangés par famille, dans l'ordre que le moteur donne : une
 *  vingtaine d'états ne se lisent pas en vrac (23/09/2026). */
function familles(etats) {
  const out = [];
  for (const e of etats) {
    const f = e.famille || "Autres";
    if (!out.length || out[out.length - 1][0] !== f) out.push([f, []]);
    out[out.length - 1][1].push(e);
  }
  return out;
}

/** LES QUESTIONS DE L'ÉTAT CHOISI, et rien d'autre (23/09/2026). L'éditeur :
 *  « quand on arrive sur les exports on n'a qu'une case, et selon ce qu'on
 *  choisit on arrive sur ce qui est pertinent ». Quelles questions, quel
 *  bouton, un aperçu ou non : c'est le moteur qui le dit (`FORMES`). */
function dessinerQuestions(opt) {
  const p = EXPORTS;
  const forme = p.etats.find((e) => e.etat === document.getElementById("x-etat").value);
  const q = new Set(forme.questions);
  const manque = { xlsx: !p.xlsx, pdf: !p.pdf };
  document.getElementById("exports-resultat").innerHTML = "";
  document.getElementById("x-suite").innerHTML = `
    <div class="champs">
      ${q.has("periode") ? `<div class="champ pleine">
        <label>Quand <span class="discret" id="x-quand-dit"></span></label>
        <input type="hidden" id="x-quand"><input type="hidden" id="x-periode">
        <div class="calendrier" id="x-calendrier"></div>
      </div>` : ""}
      ${q.has("regroupement") ? `<div class="champ">
        <label for="x-par">Regroupement</label>
        <select id="x-par">
          ${p.regroupements.filter((r) => (forme.regroupements || []).includes(r.regroupement))
              .map((r) => opt(r.regroupement, r.quoi,
                              r.regroupement === forme.regroupements[0])).join("")}
        </select>
        <div class="aide">${forme.etat === "livre-de-paie"
          ? `Par salarié, ce qu'on envoie au comptable ; par mois, ce
          qu'on rapproche de la banque ; par spectacle, ce qui justifie une
          subvention. Les trois disent la même chose, et le total ne bouge pas.`
          : "Le total ne bouge pas : seule la répartition des lignes change."}</div>
      </div>` : ""}
      ${q.has("logiciel") ? `<div class="champ pleine">
        <label for="x-logiciel">Logiciel du comptable</label>
        <select id="x-logiciel">
          ${p.compta.logiciels.map((l) => opt(l.logiciel,
            `${l.nom} — ${l.format_lisible}`,
            l.logiciel === p.compta.logiciel)).join("")}
          ${opt("", "Aucun : je choisis le format", !p.compta.logiciel)}
        </select>
        <div class="aide" id="x-logiciel-aide"></div>
        <label class="choix" id="x-analytique-champ">
          <input type="checkbox" id="x-analytique"${p.compta.analytique ? " checked" : ""}>
          <span>Ventiler les salaires bruts par spectacle (analytique)</span>
        </label>
        <div class="aide">Le code de chaque spectacle se règle sur sa fiche :
          Configuration › Spectacles, case « Code analytique ». Vide, il est
          tiré du titre.</div>
        <div id="x-controle"></div>
      </div>` : ""}
      ${q.has("format") ? `<div class="champ" id="x-format-champ">
        <label for="x-format">Format</label>
        <select id="x-format">
          ${p.formats.filter((f) => (forme.formats || []).includes(f.format)).map((f) => opt(f.format,
            f.quoi + (manque[f.format] ? " — brique absente, le CSV sera écrit" : ""),
            f.format === "csv")).join("")}
        </select>
      </div>` : ""}
      ${q.has("coordonnees") ? `<div class="champ pleine">
        <label class="coche" title="Sans la case, rien de tout cela ne sort. Un fichier qui les porte ne s'envoie qu'à qui en a besoin.">
          <input type="checkbox" id="x-coordonnees">
          Avec les coordonnées des salariés (NIR, naissance, adresse)
        </label>
      </div>` : ""}
    </div>
    <div class="actions">
      ${forme.apercu ? `<button class="secondaire" id="x-apercu">Aperçu</button>` : ""}
      <button class="primaire" id="x-ecrire">${echapper(forme.bouton)}</button>
      <button class="lien" id="x-dossier" title="${echapper(p.dossier)} — et nulle part ailleurs : un export emporte des salaires">Ouvrir le dossier</button>
    </div>`;
  if (q.has("periode")) dessinerCalendrier(forme.periodes || ["annee", "mois", "toutes"]);
  if (q.has("logiciel")) {
    const retenir = async () => {
      document.getElementById("exports-resultat").innerHTML = "";
      try {
        EXPORTS.compta = await api("retenir-logiciel", {
          logiciel: document.getElementById("x-logiciel").value,
          analytique: document.getElementById("x-analytique").checked });
      } catch (e) { dire(e.message, "erreur"); }
      montrerLogiciel();
    };
    document.getElementById("x-logiciel").onchange = retenir;
    document.getElementById("x-analytique").onchange = retenir;
    const f = document.getElementById("x-format");
    if (f) f.addEventListener("change", montrerLogiciel);
    montrerLogiciel();
  }
  if (forme.apercu) document.getElementById("x-apercu").onclick = () => lancerExport(true);
  document.getElementById("x-ecrire").onclick = () => lancerExport(false);
  document.getElementById("x-dossier").onclick = async () => {
    await ouvrirEtDire("ouvrir-exports");
  };
}

/** LE LOGICIEL DU COMPTABLE (23/09/2026). Choisi une fois, retenu par le
 *  moteur (config/compta.json) : il fixe le format, et le menu des formats
 *  s'efface. La case d'analytique ne vaut que pour les formats qui la
 *  portent. Ce que le plan comptable ne tiendrait pas dans ce format est dit
 *  ici, avant d'exporter — c'est le moteur qui le dit. */
function montrerLogiciel() {
  const c = EXPORTS.compta;
  const choisi = c.logiciels.find((l) => l.logiciel === document.getElementById("x-logiciel").value);
  const fmt = document.getElementById("x-format-champ");
  if (fmt) fmt.classList.toggle("cachee", !!choisi);
  document.getElementById("x-logiciel-aide").textContent = choisi
    ? `Fichier ${choisi.format_lisible} : ${choisi.pourquoi}. Retenu pour les prochaines fois.`
    : "Le format se choisit ci-dessous.";
  // Sans logiciel, la case suit le format choisi à la main.
  const f = document.getElementById("x-format");
  const porte = choisi ? choisi.analytique
    : c.logiciels.some((l) => l.analytique && f && l.format === f.value);
  document.getElementById("x-analytique-champ").classList.toggle("cachee", !porte);
  document.getElementById("x-controle").innerHTML = (c.controle || []).map((m) =>
    `<div class="encart alerte">${echapper(m)}</div>`).join("");
}

/** LE CALENDRIER DES EXPORTS (Leandre, 08/10/2026). Une ligne par année,
 *  ses douze mois ; on clique une année, un mois, ou « Tout ». Seuls les
 *  mois saisis ou déclarés se cliquent, et ce que l'état admet (`periodes`,
 *  dit par le moteur) : un état annuel n'offre pas les mois. Le moteur prend
 *  UNE période ; le choix se range dans `x-quand` / `x-periode`. */
function dessinerCalendrier(periodes) {
  const cal = document.getElementById("x-calendrier");
  const dispo = new Set(EXPORTS.mois);
  const src = EXPORTS.sources || {};
  const annees = [...new Set(EXPORTS.mois.map((m) => m.slice(0, 4)))].sort().reverse();
  const peutMois = periodes.includes("mois");
  const peutAn = periodes.includes("annee");
  cal.innerHTML = `
    ${periodes.includes("toutes") ? `<div class="rangee"><button type="button" data-quand="toutes" data-periode="">Tout</button></div>` : ""}
    ${annees.map((a) => `<div class="rangee">
      <button type="button" class="an" data-quand="annee" data-periode="${a}"${peutAn ? "" : " disabled"}>${a}</button>
      ${MOIS_ABR.map((n, i) => {
        const m = `${a}-${String(i + 1).padStart(2, "0")}`;
        const ok = peutMois && dispo.has(m);
        return `<button type="button" data-quand="mois" data-periode="${m}"${ok ? "" : " disabled"}
          title="${echapper(moisEnClair(m) + (src[m] ? " — " + src[m] : ""))}">${n}</button>`;
      }).join("")}
    </div>`).join("")}`;
  const choisir = (quand, periode) => {
    document.getElementById("x-quand").value = quand;
    document.getElementById("x-periode").value = periode;
    cal.querySelectorAll("button").forEach((b) => {
      const d = b.dataset;
      const pris = quand === "toutes" ? d.quand === "toutes" || (d.quand === "mois" && !b.disabled)
        : quand === "annee" ? (d.quand === "annee" && d.periode === periode) || (d.quand === "mois" && d.periode.startsWith(periode + "-") && dispo.has(d.periode))
        : d.quand === "mois" && d.periode === periode;
      b.classList.toggle("pris", !!pris);
    });
    document.getElementById("x-quand-dit").textContent = "— " + (quand === "toutes" ? "toutes les années"
      : quand === "annee" ? "l'année " + periode : moisEnClair(periode));
    document.getElementById("exports-resultat").innerHTML = "";
  };
  cal.querySelectorAll("button").forEach((b) => {
    b.onclick = () => choisir(b.dataset.quand, b.dataset.periode);
  });
  // Par défaut, le plus récent que l'état admet : le dernier mois, sinon la dernière année, sinon tout.
  const dernier = EXPORTS.mois[EXPORTS.mois.length - 1];
  if (peutMois && dernier) choisir("mois", dernier);
  else if (peutAn && annees.length) choisir("annee", annees[0]);
  else choisir("toutes", "");
}

async function lancerExport(apercu) {
  const val = (id) => { const e = document.getElementById(id); return e ? e.value : undefined; };
  const quand = val("x-quand");
  const periode = val("x-periode");
  const coord = document.getElementById("x-coordonnees");
  const charge = {
    etat: val("x-etat"),
    regroupement: val("x-par"),
    format: val("x-format"),
    coordonnees: coord ? coord.checked : false,
    apercu,
  };
  const logiciel = val("x-logiciel");
  if (logiciel !== undefined) {
    if (logiciel) charge.logiciel = logiciel;
    const champ = document.getElementById("x-analytique-champ");
    charge.analytique = !champ.classList.contains("cachee")
      && document.getElementById("x-analytique").checked;
  }
  if (quand === "mois") charge.mois = periode;
  else if (quand === "annee") charge.annee = periode;
  else charge.toutes_les_annees = true;
  const cible = document.getElementById("exports-resultat");
  cible.innerHTML = `<p class="discret">…</p>`;
  let r;
  try { r = await api("exporter", charge); }
  catch (e) { cible.innerHTML = `<div class="encart erreur">${echapper(e.message)}</div>`; return; }
  if (r.etat === "regularisation-casc-svp") { dessinerCasc(r); return; }
  if (r.colonnes && r.sections) { dessinerTableau(r, apercu); return; }
  dessinerLivre(r, apercu);
}

/** LA RÉGULARISATION CASC-SVP. Deux étapes, dans l'ordre où on les vit
 *  (l'éditeur, 23/09/2026) : faire les virements, puis envoyer — par la boîte
 *  de la compagnie si elle est posée, c'est le premier choix et il se voit ;
 *  sinon à la main, les fichiers sont là. Le détail des mois se déplie. */
function dessinerCasc(r) {
  const cible = document.getElementById("exports-resultat");
  const lignes = r.mois.map((m) => `<tr><td>${echapper(m.libelle)}</td>
      <td class="r">${echapper(m.masse)}</td><td class="r">${echapper(m.taux)} %</td>
      <td class="r">${echapper(m.cotisation)}</td></tr>`).join("");
  const annees = r.annees.map((a) => `<tr class="sous-total"><td><b>${echapper(a.annee)}</b></td>
      <td class="r">${echapper(a.masse)}</td><td></td><td class="r"><b>${echapper(a.cotisation)}</b></td></tr>`).join("");
  const courriel = (c, titre) => `<details><summary>${titre} — à ${echapper(c.a)}, objet ${echapper(c.objet)}</summary>
      <pre class="courriel">${echapper(c.corps)}</pre></details>`;
  const n = r.annees.length;
  cible.innerHTML = `
    <h3>Régularisation CASC-SVP — ${echapper(r.periode)} — ${echapper(euros(r.total))}</h3>
    ${r.avertissements.map((a) => `<div class="encart alerte">${echapper(a)}</div>`).join("")}
    <div class="casc-etape">
      <h4>1. Faire ${n > 1 ? "les " + n + " virements" : "le virement"}</h4>
      <ul>${r.annees.map((a) => `<li><b>${echapper(euros(a.cotisation))}</b>, libellé
        « ${echapper(a.libelle_virement)} »</li>`).join("")}</ul>
      <p class="discret">Vers Audiens Santé Prévoyance – Encaissement. L'IBAN se lit dans
        l'espace employeur Audiens ou sur la page employeurs du CASC-SVP.</p>
    </div>
    <div class="casc-etape">
      <h4>2. Envoyer les deux courriels</h4>
      <div class="casc-choix">
        <div class="casc-option principale">
          <h5>Automatiquement, depuis la boîte de la compagnie</h5>
          ${r.boite_liee ? `
          <div class="e-contrat-champs">
            ${r.annees.map((a) => `<label>Virement ${echapper(a.annee)} fait le
              <input type="date" class="casc-date" data-annee="${echapper(a.annee)}"></label>`).join("")}
          </div>
          <button class="primaire" id="casc-envoyer">Envoyer les 2 courriels</button>
          <p class="discret">Audiens reçoit les masses et les dates des virements ; le CASC-SVP,
            la liste nominative. Une copie reste dans le dossier, et les années sont notées
            régularisées.</p>` : `
          <p>La boîte courriel n'est pas posée : Configuration › Courriel. Une fois posée,
            l'envoi se fait d'ici en un clic.</p>`}
        </div>
        <div class="casc-option">
          <h5>À la main</h5>
          <p>Joindre la liste nominative au courriel du CASC-SVP, recopier les deux courriels
            ci-dessous, puis noter la régularisation.</p>
          <button class="secondaire" id="casc-dossier">Ouvrir les fichiers (liste nominative .xlsx)</button>
          ${courriel(r.courriel_audiens, "Courriel à Audiens")}
          ${courriel(r.courriel_casc, "Courriel au CASC-SVP")}
          <div class="e-contrat-champs">
            <label>Année <select id="casc-annee">${r.annees.map((a) =>
              `<option>${echapper(a.annee)}</option>`).join("")}</select></label>
            <label>Virée le <input type="date" id="casc-vire-le"></label>
            <label>Montant <input type="text" id="casc-montant" size="8"></label>
            <button class="secondaire" id="casc-noter">Noter comme régularisé</button>
          </div>
        </div>
      </div>
    </div>
    <details><summary>Le détail, mois par mois</summary>
      <table class="livre"><thead><tr><th>mois</th><th class="r">masse</th><th class="r">taux</th>
        <th class="r">CASC</th></tr></thead><tbody>${lignes}${annees}</tbody></table>
    </details>`;
  document.getElementById("casc-dossier").onclick = async () => {
    await ouvrirEtDire("ouvrir-exports", { sous: "casc-svp" });
  };
  const choixAnnee = document.getElementById("casc-annee");
  const montant = document.getElementById("casc-montant");
  const preremplir = () => {
    const a = r.annees.find((x) => x.annee === choixAnnee.value);
    montant.value = a ? a.cotisation : "";
  };
  choixAnnee.onchange = preremplir;
  preremplir();
  document.getElementById("casc-noter").onclick = async () => {
    const vire = document.getElementById("casc-vire-le").value;
    if (!vire) { dire("La date du virement d'abord : on note ce qui est payé."); return; }
    try {
      const e = await api("casc-regularise",
        { annee: choixAnnee.value, vire_le: vire, montant: montant.value });
      dire(`CASC-SVP ${e.annee} notée régularisée (${e.mois.length} mois).`);
    } catch (x) { dire(x.message, "erreur"); }
  };
  const envoyer = document.getElementById("casc-envoyer");
  if (envoyer) envoyer.onclick = async () => {
    const vire_le = {};
    for (const c of document.querySelectorAll(".casc-date")) if (c.value) vire_le[c.dataset.annee] = c.value;
    const liste = Object.keys(vire_le);
    if (!liste.length) { dire("La date d'au moins un virement d'abord : les courriels annoncent des virements faits."); return; }
    const mdp = await demanderMotDePasse("envoyer à Audiens et au CASC-SVP la régularisation " + liste.join(" et "), { envoi: true });
    if (!mdp) return;
    const rendre = occupe(envoyer, "Envoi des courriels en cours…");
    const repere = await repereCourrielsFictifs();
    try {
      const e = await api("casc-envoyer", { vire_le, motdepasse: mdp });
      dire(`${e.partis.length} courriel(s) envoyé(s) à ${e.partis.join(" et ")} — ` +
           `${e.annees.map((a) => a.annee).join(" et ")} notée(s) régularisée(s).`, "envoye");
      cible.innerHTML = `<p>CASC-SVP ${echapper(e.periode)} : ${echapper(euros(e.total))}, envoyé depuis
        ${echapper(e.expediteur)}. Copie des courriels dans ${echapper(e.dossier)}.</p>`;
      fenetreCourrielsFictifs("courriel", repere);
    } catch (x) {
      rendre();
      dire("Envoi impossible : " + x.message, "erreur");
    }
  };
}

/** UN ÉTAT DE LA MACHINERIE COMMUNE (23/09/2026) : ses colonnes, ses
 *  sections et ses totaux viennent du moteur (`tableaux.Tableau`). La page ne
 *  sait rien de chaque état : un état neuf s'affiche sans qu'on touche ici. */
function dessinerTableau(r, apercu) {
  const cible = document.getElementById("exports-resultat");
  const droite = (c) => ["montant", "nombre", "taux"].includes(c.type);
  const cel = (c, v) => {
    const t = v === null || v === undefined ? "" : String(v);
    return `<td${droite(c) ? ' class="r"' : ""}>${echapper(droite(c) ? t.replace(".", ",") : t)}</td>`;
  };
  const rang = (d, classe) => `<tr${classe ? ` class="${classe}"` : ""}>${
    r.colonnes.map((c) => cel(c, d[c.cle])).join("")}</tr>`;
  const plusieurs = r.sections.length > 1;
  const corps = r.sections.map((s) =>
    (plusieurs && s.libelle ? `<tr class="sous-total"><td colspan="${r.colonnes.length}"><b>${
      echapper(s.libelle)}</b></td></tr>` : "")
    + s.lignes.map((l) => rang(l, "")).join("")
    + (plusieurs && s.total ? rang(s.total, "sous-total") : "")).join("");
  const n = r.sections.reduce((a, s) => a + s.lignes.length, 0);
  cible.innerHTML = `
    <h2>${echapper(r.titre)} — ${echapper(r.periode)}</h2>
    <p class="discret">${n} ligne${n === 1 ? "" : "s"}${r.fichier
        ? ` — écrit : <b>${echapper(r.fichier)}</b>`
        : " — aperçu, rien n'a été écrit"}.</p>
    ${r.donnees_personnelles ? `<div class="encart alerte">Cet état porte des données
      personnelles : il reste dans prive/, et ne s'envoie qu'à qui en a besoin.</div>` : ""}
    ${(r.avertissements || []).map((a) =>
        `<div class="encart info">${echapperLiens(a)}</div>`).join("")}
    <div class="tableau-compta"><table class="compta">
      <tr>${r.colonnes.map((c) => `<th${droite(c) ? ' class="r"' : ""}>${echapper(c.libelle)}</th>`).join("")}</tr>
      ${corps}
      ${r.total ? rang(r.total, "total-general") : ""}
    </table></div>
    ${(r.sources || []).length ? `<details><summary>Sources</summary><ul>${
      r.sources.map((x) => `<li class="discret">${echapper(x)}</li>`).join("")}</ul></details>` : ""}
    ${r.fichier ? `<div class="actions">
      <button class="lien" id="x-dossier-2">Ouvrir le dossier</button></div>` : ""}`;
  const o = document.getElementById("x-dossier-2");
  if (o) o.onclick = async () => {
    await ouvrirEtDire("ouvrir-exports");
  };
  if (!apercu) dire("Export écrit.");
}

/** L'état rendu, tel quel : les groupes, leurs sous-totaux, le pied. */
function dessinerLivre(r, apercu) {
  const cible = document.getElementById("exports-resultat");
  const eur = (v) => (v === null || v === undefined ? "—" : echapper(v));
  const ligne = (l) => `
    <tr>
      <td>${echapper(l.mois)}</td>
      <td>${echapper(l.salarie)}</td>
      <td class="discret">${echapper(l.spectacle || l.nature)}</td>
      <td class="discret">${echapper(l.source)}</td>
      <td class="r">${eur(l.brut)}</td>
      <td class="r">${eur(l.cotisations_salariales)}</td>
      <td class="r">${eur(l.cotisations_patronales)}</td>
      <td class="r">${eur(l.net_imposable)}</td>
      <td class="r">${eur(l.net_a_payer)}</td>
      <td class="r">${eur(l.pas)}</td>
      <td class="r">${eur(l.cout_employeur)}</td>
    </tr>`;
  const total = (t, titre, classe) => `
    <tr class="${classe}">
      <td></td><td><b>${echapper(titre)}</b></td>
      <td class="discret">${t.bulletins} bulletin${t.bulletins === 1 ? "" : "s"}</td>
      <td></td>
      <td class="r"><b>${eur(t.brut)}</b></td>
      <td class="r"><b>${eur(t.cotisations_salariales)}</b></td>
      <td class="r"><b>${eur(t.cotisations_patronales)}</b></td>
      <td class="r"><b>${eur(t.net_imposable)}</b></td>
      <td class="r"><b>${eur(t.net_a_payer)}</b></td>
      <td class="r"><b>${eur(t.pas)}</b></td>
      <td class="r"><b>${eur(t.cout_employeur)}</b></td>
    </tr>`;
  const corps = r.groupes.map((g) =>
    g.lignes.map(ligne).join("") + total(g.totaux, "Total " + g.libelle, "sous-total")
  ).join("");
  cible.innerHTML = `
    <h2>${echapper(r.periode)}</h2>
    <p class="discret">${r.totaux.bulletins} bulletin${r.totaux.bulletins === 1 ? "" : "s"},
      ${echapper(r.mois.length)} mois${r.fichier
        ? ` — écrit : <b>${echapper(r.fichier)}</b>`
        : " — aperçu, rien n'a été écrit"}.</p>
    ${Object.keys(r.totaux.inconnus || {}).length
      ? `<div class="encart info">Totaux partiels :
         ${Object.entries(r.totaux.inconnus).sort().map(([c, n]) =>
           `${echapper(c.replace(/_/g, " "))} sur ${r.totaux.bulletins - n}
            bulletin${r.totaux.bulletins - n === 1 ? "" : "s"} sur ${r.totaux.bulletins}`)
           .join(" ; ")}.</div>`
      : ""}
    ${(r.avertissements || []).map((a) =>
        `<div class="encart info">${echapperLiens(a)}</div>`).join("")}
    <div class="tableau-compta"><table class="compta">
      <tr><th>Mois</th><th>Salarié</th><th>Spectacle</th><th>Source</th><th>Brut</th>
          <th>Cot. sal.</th><th>Cot. pat.</th><th>Net imposable</th>
          <th>Net à payer</th><th>PAS</th><th>Coût employeur</th></tr>
      ${corps}
      ${total(r.totaux, "TOTAL GÉNÉRAL", "total-general")}
    </table></div>
    ${r.fichier ? `<div class="actions">
      <button class="lien" id="x-dossier-2">Ouvrir le dossier</button></div>` : ""}`;
  const o = document.getElementById("x-dossier-2");
  if (o) o.onclick = async () => {
    await ouvrirEtDire("ouvrir-exports");
  };
  if (!apercu) dire("Export écrit.");
}

function boiteCloture(annee) {
  ouvrirBoite(`
    <h3>Clôturer l'exercice ${echapper(annee)}</h3>
    <div class="encart">
      <b>L'année se fige</b>
      Les saisies et les sorties de ${echapper(annee)} ne se modifieront plus
      (rouvrir reste possible, depuis l'Historique). Tout ce qu'il faut garder —
      saisies, bulletins, contrats signés, DPAE, DSN, certificats, retours des
      organismes, journal des dépôts, fiches, export compta — est réuni dans
      <b>Exercice_${echapper(annee)}.zip</b>.
    </div>
    <p class="note">La clôture vient après la dernière DSN de l'année : elle est
      refusée si un mois avec des paies n'a pas de DSN acceptée au journal.</p>
    <label class="coche"><input type="checkbox" id="c-forcer">
      Clôturer quand même si une DSN manque au journal (déposée ailleurs, par exemple)</label>
    <div class="actions">
      <button class="secondaire" onclick="fermerBoite()">Annuler</button>
      <button class="primaire" id="c-lancer">Clôturer ${echapper(annee)}</button>
    </div>`);
  document.getElementById("c-lancer").onclick = async () => {
    document.getElementById("c-lancer").disabled = true;
    try {
      const r = await api("cloture", { annee, forcer: document.getElementById("c-forcer").checked });
      fermerBoite();
      dire("Exercice " + r.annee + " clos : " + r.archive + " (" + r.fichiers + " fichiers).");
      if (r.manques && r.manques.length) dire(r.manques.join(" · "));
      if (document.getElementById("exercices")) await dessinerExercices();
    } catch (e) {
      document.getElementById("c-lancer").disabled = false;
      dire(e.message, "erreur");
    }
  };
}

async function lancerExportCompta(annee) {
  let r;
  try { r = await api("export-compta", { annee }); }
  catch (e) { dire(e.message, "erreur"); return; }
  const lignes = r.ecritures.map((e) => `
    <tr><td>${echapper(e.date.slice(5, 7))}</td><td>${echapper(e.compte)}</td>
        <td>${echapper(e.libelle)}</td>
        <td class="r">${e.debit !== "0.00" ? echapper(e.debit) : ""}</td>
        <td class="r">${e.credit !== "0.00" ? echapper(e.credit) : ""}</td>
        <td class="discret">${echapper(e.analytique)}</td></tr>`).join("");
  ouvrirBoite(`
    <h3>Export compta ${echapper(r.annee)}</h3>
    <p class="discret">Mois ${r.mois.map((m) => m.slice(5)).join(", ")} — ${r.ecritures.length}
      écritures, débit ${echapper(euros(r.total_debit))} = crédit ${echapper(euros(r.total_credit))}.
      ${r.fichier ? `Écrit : <b>${echapper(r.fichier)}</b>` : ""}</p>
    ${(r.avertissements || []).map((a) => `<div class="encart info">${echapperLiens(a)}</div>`).join("")}
    <div class="tableau-compta"><table class="compta">
      <tr><th>Mois</th><th>Compte</th><th>Libellé</th><th>Débit</th><th>Crédit</th><th>Analytique</th></tr>
      ${lignes}
    </table></div>
    <div class="actions">
      <button class="secondaire" id="x-ouvrir">Ouvrir le dossier</button>
      <button class="primaire" onclick="fermerBoite()">Fermer</button>
    </div>`);
  document.getElementById("x-ouvrir").onclick = async () => {
    await ouvrirEtDire("ouvrir-exercices");
  };
  if (document.getElementById("exercices")) dessinerExercices();
}


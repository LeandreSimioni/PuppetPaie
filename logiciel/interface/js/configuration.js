/* La configuration : la liste, l'importation, metiers, organismes, AT/MP, sante. */
"use strict";

/* =======================================================================
   CONFIGURATION
   ===================================================================== */
let FICHE_ACTIVE = "salarie";
let CLE_ACTIVE = null;
/* Une fiche neuve est ouverte : `dessinerConfig` ne l'efface pas. */
let FICHE_NEUVE = false;

document.querySelectorAll(".sous-onglets button").forEach((b) => {
  b.onclick = async () => {
    if (!(await quitterLaFiche())) return;
    document.querySelectorAll(".sous-onglets button").forEach((x) => x.classList.remove("actif"));
    b.classList.add("actif");
    if (FICHE_ACTIVE !== b.dataset.fiche) FICHE_NEUVE = false;
    FICHE_ACTIVE = b.dataset.fiche;
    CLE_ACTIVE = null;
    dessinerConfig();
  };
});

const ARCHIVES_VISIBLES = { salarie: false, spectacle: false };
/* Ce qu'on cherche dans chaque liste ; gardé quand la liste se redessine. */
const RECHERCHE = { salarie: "", spectacle: "" };

function fichesDe(nom) {
  const c = ETAT.configuration;
  if (nom === "entreprise") return [c.entreprise];
  return nom === "salarie" ? c.salaries : c.spectacles;
}

function dessinerConfig() {
  const liste = document.getElementById("config-liste");
  if (FICHE_ACTIVE === "importer") {
    document.getElementById("config-formulaire").innerHTML = "";
    dessinerImportation(liste);
    return;
  }
  if (FICHE_ACTIVE === "coffre") {
    document.getElementById("config-formulaire").innerHTML = "";
    dessinerCoffre(liste);
    return;
  }
  if (FICHE_ACTIVE === "identite") {
    document.getElementById("config-formulaire").innerHTML = "";
    dessinerIdentite(liste);
    return;
  }
  if (FICHE_ACTIVE === "metiers") {
    document.getElementById("config-formulaire").innerHTML = "";
    dessinerMetiers(liste);
    return;
  }
  if (FICHE_ACTIVE === "organismes") {
    document.getElementById("config-formulaire").innerHTML = "";
    dessinerOrganismes(liste);
    return;
  }
  // Le modèle du bulletin (lot R11) : `modeles-bulletin.js`.
  if (FICHE_ACTIVE === "bulletin") {
    document.getElementById("config-formulaire").innerHTML = "";
    dessinerModelesBulletin(liste);
    return;
  }
  const fiches = fichesDe(FICHE_ACTIVE);
  if (FICHE_ACTIVE === "entreprise") {
    liste.innerHTML = "";
    peindreDemarrage(liste);
    CLE_ACTIVE = null;
    dessinerFormulaire(fiches[0]);
    return;
  }
  // Les archives en dernier, jamais melangees aux courantes. Elles restent
  // trouvables : une fiche qu'on ne voit plus passe pour perdue, et on la
  // ressaisit -- avec un deuxieme code salarie, ce qui serait pire.
  const courantes = fiches.filter((f) => f.donnees.actif !== false);
  // Les spectacles par titre.
  if (FICHE_ACTIVE === "spectacle") {
    courantes.sort((a, b) => String(a.donnees.titre || "").localeCompare(
      String(b.donnees.titre || ""), "fr", { sensitivity: "base" }));
  }
  // Les numéros d'objet archivés, du plus récent au plus ancien.
  const archivees = fiches.filter((f) => f.donnees.actif === false)
    .sort((a, b) => String(b.donnees.numero_objet || "").localeCompare(
      String(a.donnees.numero_objet || "")));
  const rangees = [...courantes, ...archivees];
  const salarie = FICHE_ACTIVE === "salarie";

  // UN SPECTACLE, PAS UNE FICHE PAR NUMÉRO (§94). Une fiche archivée du
  // même titre qu'un spectacle actif (un ancien numéro) se montre sous lui,
  // pas dans les archives.
  const titreDe = (d) => String(d.titre || "").normalize("NFD")
    .replace(/[\u0300-\u036f]/g, "").toLowerCase().replace(/\s+/g, " ").trim();
  const sansTitre = (d) => titreDe(d) === "a completer";
  // L'année qu'écrivent les deux premiers chiffres d'un numéro d'objet
  // (266Z… : 2026), pour dire de quelles années datent les numéros sans titre
  // (`archivesHtml`). Elle était définie ici avant le 30/09/2026 (d7c2590,
  // « le numéro d'objet n'est plus annuel ») : retirée avec ses autres
  // usages, la phrase des archives l'appelait encore — ReferenceError dès
  // qu'on affichait un numéro archivé sans titre.
  const anDe = (d) => /^\d{2}/.test(String(d.numero_objet || ""))
    ? 2000 + Number(String(d.numero_objet).slice(0, 2)) : null;
  const passees = {};
  const sousUnActif = new Set();
  if (!salarie) {
    const titresActifs = new Set(courantes.map((f) => titreDe(f.donnees)));
    archivees.forEach((f) => {
      const t = titreDe(f.donnees);
      if (sansTitre(f.donnees) || !titresActifs.has(t)) return;
      (passees[t] = passees[t] || []).push(f);
      sousUnActif.add(f.cle);
    });
  }
  // Les archives qui restent : les spectacles qui ne se jouent plus, par
  // titre ; puis les numéros sans titre, groupés -- à nommer si on le sait.
  const archiveesSeules = archivees.filter((f) => !sousUnActif.has(f.cle))
    .sort((a, b) => (sansTitre(a.donnees) - sansTitre(b.donnees))
      || titreDe(a.donnees).localeCompare(titreDe(b.donnees))
      || String(b.donnees.numero_objet || "").localeCompare(String(a.donnees.numero_objet || "")));

  // UNE LIGNE PAR FICHE (l'éditeur, 24/09/2026 : « j'aime bien les lignes pour
  // les salariés »). Elle remplace la grille de cartes : le nom, ce qui
  // l'identifie, son état, puis les gestes -- « Archiver » dit en toutes
  // lettres, là où le clic sur toute la carte archivait sans prévenir.
  const rangHtml = (f, trouve) => {
    const d = f.donnees;
    const err = f.anomalies.filter((a) => a.gravite === "erreur").length;
    const av = f.anomalies.length - err;
    const archive = d.actif === false;
    const titre = salarie
      ? `${d.nom || ""} ${d.prenom_affichage || d.prenom || ""}`.trim()
      : String(d.titre || "").trim();
    // Un numero d'objet importe de CoolPaie n'a pas de titre : le moteur
    // pose « A COMPLETER », que la ligne dit en minuscules.
    const aCompleter = !salarie && titre.toUpperCase() === "A COMPLETER";
    const quoi = salarie
      ? [d.emploi_affichage || d.emploi || "", d.code_salarie ? "matricule " + d.code_salarie : ""]
      : [d.numero_objet || "", d.code_analytique ? "analytique " + d.code_analytique : ""];
    const autresNumeros = !salarie && !archive && !trouve ? (passees[titreDe(d)] || [])
      .slice().sort((a, b) => String(b.donnees.numero_objet || "").localeCompare(
        String(a.donnees.numero_objet || ""))) : [];
    const etat = archive ? `<span class="pastille archive">archivé</span>`
      : err ? `<span class="pastille erreur">${err} erreur${err > 1 ? "s" : ""}</span>`
      : av ? `<span class="pastille alerte">${av} à vérifier</span>`
      : `<span class="pastille bien">complet</span>`;
    return `<div class="rang fiche${archive ? " archivee" : ""}${f.cle === CLE_ACTIVE ? " selectionnee" : ""}"
        data-cle="${echapper(f.cle)}">
      <div class="rang-infos">
        <b class="rang-nom">${aCompleter ? `<span class="a-completer">titre à compléter</span>`
          : echapper(titre || f.cle)}</b>
        ${echapper(quoi.filter(Boolean).join(" · "))}
        ${trouve ? `<span class="trouve">trouvé dans ${echapper(trouve.ou)} : « ${echapper(trouve.quoi)} »</span>` : ""}
        ${autresNumeros.length ? `<span class="annees">anciens numéros :
          ${autresNumeros.map((p) => `<button class="lien" data-ouvrir="${echapper(p.cle)}">${
            echapper(p.donnees.numero_objet || p.cle)}</button>`).join(" · ")}</span>` : ""}
      </div>
      <div class="rang-etat">${etat}</div>
      <div class="rang-actions">
        <button class="lien" data-activer="${archive ? "oui" : "non"}">${archive ? "Réactiver" : "Archiver"}</button>
      </div>
    </div>`;
  };

  // CACHÉES PAR DÉFAUT (l'éditeur, 23/09/2026) : un spectacle archivé ne
  // se joue plus. Une case les montre ; elle reste
  // cochée tant qu'on ne la décoche pas, et une fiche archivée ouverte les
  // montre d'office.
  const voirArchives = ARCHIVES_VISIBLES[FICHE_ACTIVE]
    || archivees.some((f) => f.cle === CLE_ACTIVE);
  // UNE BARRE, DEUX GESTES : chercher, ajouter. Le titre qui répétait le
  // sous-onglet (« SALARIÉS » sous Salariés) est parti (24/09/2026).
  liste.innerHTML = `<div class="barre-liste">
      <input type="search" id="config-recherche" class="recherche" autocomplete="off"
        value="${echapper(RECHERCHE[FICHE_ACTIVE] || "")}"
        placeholder="${salarie
          ? "Rechercher dans toutes les fiches : nom, NIR, adresse, courriel, emploi…"
          : "Rechercher : titre, numéro d'objet, code analytique…"}">
      <button class="secondaire" id="config-nouvelle">+ ${salarie
        ? "un salarié" : "un spectacle"}</button>
    </div>
    <div id="config-rangs"></div>`
    + (salarie ? fichesAttenduesHtml() : "");

  // LA RECHERCHE DANS TOUS LES CHAMPS (l'éditeur, 24/09/2026). Chaque valeur
  // de la fiche, les anciennes valeurs datées comprises (un courriel changé
  // retrouve encore la personne), sans accents, sans majuscules, sans
  // espaces (« 2 93 08 » trouve le NIR) ; une date se cherche aussi comme
  // on l'écrit (23/07/1990). Plusieurs mots : chacun doit se trouver
  // quelque part dans la fiche. Elle fouille les archivées aussi -- c'est
  // souvent là qu'on cherche.
  const libelles = {};
  (ETAT.configuration.description[FICHE_ACTIVE] || []).forEach((c) => { libelles[c.cle] = c.libelle; });
  const libelleDe = (cle) => libelles[cle] || libelles[cle.replace(/_affichage$/, "")]
    || cle.replace(/_/g, " ");
  const plat = (t) => String(t).normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase();
  const valeurs = (f) => {
    const out = [];
    const ajouter = (ou, v) => {
      if (v === null || v === undefined || v === "" || typeof v === "boolean") return;
      if (typeof v === "object") {
        Object.entries(v).forEach(([k, x]) => { if (!k.startsWith("_")) ajouter(ou, x); });
        return;
      }
      const t = String(v);
      out.push({ ou, quoi: t });
      if (/^\d{4}-\d{2}-\d{2}$/.test(t)) out.push({ ou, quoi: jjmmaaaa(t) });
    };
    // La forme lisible d'abord (« Avallon »), la forme déclarée ensuite
    // (« AVALLON ») : c'est la première trouvée que la ligne cite.
    Object.entries(f.donnees)
      .sort(([a], [b]) => b.endsWith("_affichage") - a.endsWith("_affichage"))
      .forEach(([k, v]) => {
        if (k.startsWith("_") || k === "historique" || k === "actif") return;
        ajouter(libelleDe(k), v);
      });
    (f.donnees.historique || []).forEach((h) =>
      ajouter(libelleDe(h.champ) + " (ancienne valeur)", h.valeur));
    return out.map((x) => ({ ...x, cle: plat(x.quoi), serre: plat(x.quoi).replace(/\s+/g, "") }));
  };
  const chercher = (f, mots) => {
    const vs = valeurs(f);
    let premier = null;
    for (const m of mots) {
      const serre = m.replace(/\s+/g, "");
      const v = vs.find((x) => x.cle.includes(m) || x.serre.includes(serre));
      if (!v) return null;
      premier = premier || v;
    }
    return premier || {};
  };

  // Les numéros sans titre (importés des DSN, « A COMPLETER ») en un groupe,
  // sous une phrase qui dit d'où ils viennent et ce qu'on peut en faire.
  const archivesHtml = () => {
    const nommees = archiveesSeules.filter((f) => salarie || !sansTitre(f.donnees));
    const anonymes = salarie ? [] : archiveesSeules.filter((f) => sansTitre(f.donnees));
    const ans = anonymes.map((f) => anDe(f.donnees)).filter(Boolean);
    return `${nommees.length ? `<div class="rangs">${nommees.map((f) => rangHtml(f)).join("")}</div>` : ""}
      ${anonymes.length ? `<p class="note">${anonymes.length} numéro${anonymes.length > 1 ? "s" : ""}
        sans titre, trouvé${anonymes.length > 1 ? "s" : ""} dans les DSN${ans.length
          ? ` de ${Math.min(...ans)}${Math.max(...ans) !== Math.min(...ans) ? "–" + Math.max(...ans) : ""}` : ""}
        — les nommer si vous savez à quel spectacle ils étaient (cliquer sur la ligne).</p>
        <div class="rangs">${anonymes.map((f) => rangHtml(f)).join("")}</div>` : ""}`;
  };

  const peindre = () => {
    const q = plat(RECHERCHE[FICHE_ACTIVE] || "").trim();
    const cible = document.getElementById("config-rangs");
    if (q) {
      const mots = q.split(/\s+/);
      const trouvees = rangees.map((f) => ({ f, t: chercher(f, mots) })).filter((x) => x.t);
      // Où l'on a trouvé, sauf si c'est le nom même qu'on lit sur la ligne.
      cible.innerHTML = trouvees.length
        ? `<div class="rangs">${trouvees.map(({ f, t }) =>
            rangHtml(f, t.ou && !/^(nom|prénom|titre)/i.test(t.ou) ? t : null)).join("")}</div>
          <p class="note">${trouvees.length} fiche${trouvees.length > 1 ? "s" : ""} sur ${rangees.length},
            archivées comprises.</p>`
        : `<p class="discret">Aucune fiche ne contient « ${echapper(RECHERCHE[FICHE_ACTIVE])} »,
            archivées comprises.</p>`;
    } else {
      cible.innerHTML = `${courantes.length
          ? `<div class="rangs">${courantes.map((f) => rangHtml(f)).join("")}</div>`
          : `<p class="discret">${salarie ? "Aucun salarié actif." : "Aucun spectacle actif."}</p>`}
        ${archiveesSeules.length ? `<label class="coche montrer-archives">
            <input type="checkbox" id="config-archives"${voirArchives ? " checked" : ""}>
            ${salarie ? "Afficher les salariés archivés" : "Afficher les spectacles archivés"}
            <span class="discret">(${archiveesSeules.length}) — ${salarie ? "partis ; leurs documents restent"
              : "ceux qui ne se jouent plus ; les AEM déposées portent leurs numéros"}</span></label>
          ${voirArchives ? archivesHtml() : ""}` : ""}`;
      const caseArchives = document.getElementById("config-archives");
      if (caseArchives) caseArchives.onchange = () => {
        ARCHIVES_VISIBLES[FICHE_ACTIVE] = caseArchives.checked;
        dessinerConfig();
      };
    }
    cible.querySelectorAll(".rang.fiche").forEach((r) => {
      const cle = r.dataset.cle;
      const nom = r.querySelector(".rang-nom").textContent.trim();
      r.querySelector("[data-activer]").onclick = async (e) => {
        const activer = e.currentTarget.dataset.activer === "oui";
        try {
          await api("activer-fiche", { nom: FICHE_ACTIVE, cle, actif: activer });
          ETAT = await api("etat");
          dire(`${nom} : ${activer ? "réactivé" : "archivé"}.`);
          dessinerConfig();
        } catch (err) { dire(err.message, "erreur"); }
      };
      const ouvrir = async (c) => {
        if (c !== CLE_ACTIVE && !(await quitterLaFiche())) return;
        CLE_ACTIVE = c;
        FICHE_NEUVE = false;
        dessinerConfig();
        dessinerFormulaire(rangees.find((f) => f.cle === c));
        document.getElementById("config-formulaire").scrollIntoView({ behavior: "smooth" });
      };
      // LA LIGNE S'OUVRE D'UN CLIC SUR ELLE (ticket #218) : plus de bouton « Modifier ».
      rendreOuvrable(r, () => ouvrir(cle), "Ouvrir la fiche");
      r.querySelectorAll("[data-ouvrir]").forEach((b) => { b.onclick = () => ouvrir(b.dataset.ouvrir); });
    });
  };
  peindre();
  const recherche = document.getElementById("config-recherche");
  recherche.oninput = () => { RECHERCHE[FICHE_ACTIVE] = recherche.value; peindre(); };
  document.getElementById("config-nouvelle").onclick = () => nouvelleFiche(FICHE_ACTIVE);
  brancherFichesAttendues();
  if (!CLE_ACTIVE && !FICHE_NEUVE) document.getElementById("config-formulaire").innerHTML = "";
}

/* ---------------------------------------------------- reprendre l'historique
   Deux temps, et jamais un seul : l'aperçu dit ce qui serait créé et ce que
   les DSN disaient autrement que les fiches ; le bouton n'écrit qu'après.
   Une fiche existante reçoit seulement ce qui y est vide ou « A COMPLETER »
   (`importation_completer`, lot I2) ; tout champ rempli fait foi, l'écart
   est signalé, un changement se date à la main (operations_fiches).

   LA BOÎTE AUX LETTRES (lot F du plan d'import, 10/10/2026). Une seule
   entrée : le dossier `import/` de la compagnie, où l'on met tout ce que
   l'ancien logiciel a laissé — DSN, AEM, bulletins de paie en PDF, archives
   zip. Le moteur trie par le contenu (`importation-apercu` avec `dossier`) et
   rend un compte rendu par fichier : la page le montre en tableau, le sort en
   pastille, la raison en clair. Elle ne décide rien : ce qui est repris, ce
   qui ne l'est pas et pourquoi, c'est le moteur qui le dit. */
let IMPORT_DOSSIER = "";

/* Le sort d'un fichier ou d'une page (`importation_bulletins::SORTS`) : son
   mot, et la couleur de sa pastille. Vert : repris ; orange : repris avec une
   réserve, ou laissé exprès ; rouge : pas repris, quelque chose manque. */
const SORTS_IMPORT = {
  repris: ["repris", "bien"],
  deja_repris: ["déjà repris", "bien"],
  libelle_inconnu: ["libellé inconnu", "alerte"],
  deja_saisi: ["déjà saisi", ""],
  incoherent: ["incohérent", "erreur"],
  incomplet: ["incomplet", "erreur"],
  sans_texte: ["sans texte", "erreur"],
  gabarit_inconnu: ["gabarit inconnu", "erreur"],
  plusieurs_gabarits: ["plusieurs gabarits", "erreur"],
  non_reconnu: ["non reconnu", "erreur"],
  doublon: ["doublon", ""],
  archive: ["archive", ""],
};
function pastilleSort(sort) {
  const [mot, classe] = SORTS_IMPORT[sort] || [String(sort || "?").replace(/_/g, " "), ""];
  return `<span class="pastille ${classe}">${echapper(mot)}</span>`;
}

/* « Ouvrir le dossier » n'a de sens que si la page tourne sur le poste,
   servie par sa fenêtre (Tauri, ou `paie interface` dans le navigateur du
   poste) : c'est elle qui pose `window.pywebview` (la cale de
   `interface/mod.rs`, avant le premier script) et qui sait la route
   `ouvrir-import` (`interface/api.rs`). Sans elle, le bouton n'apparaît pas. */
// Dans la fenêtre PuppetPaie (Tauri, `cadre.js`), et non dans un navigateur : le poste peut ouvrir son dossier.
const fenetreDuPoste = () => !!window.fenetrePuppetPaie;

/* REPRENDRE L'HISTORIQUE (24/09/2026 ; lot G2, 10/10/2026, Léandre : « une
   logique simple : ils mettent tout dans import/, ou ils choisissent un
   dossier avec tout dedans »). La page ne sert qu'une fois : le jour où une
   compagnie arrive avec l'historique d'un autre logiciel. Un dossier, celui
   de la compagnie prérempli ; « Parcourir… » pour en prendre un autre ;
   « Ouvrir le dossier » pour y déposer les fichiers ; puis l'aperçu. Aucun
   logiciel n'est nommé. Si l'historique est repris, l'état le dit en une
   ligne, et « Relire un dossier » reste pour ce qui manquait. */
async function dessinerImportation(cible) {
  let e = null;
  try { e = await api("importation-etat"); } catch (err) { e = null; }
  if (!IMPORT_DOSSIER && e && e.dossier) IMPORT_DOSSIER = e.dossier;
  const formulaire = `<p class="intro">Mettez dans ce dossier tout ce que votre ancien logiciel vous a
      laissé — bulletins de paie en PDF, DSN, AEM, archives zip —, puis « Voir ce qui serait repris ».
      Rien n'est écrit avant « Reprendre ».</p>
    <div class="e-contrat-champs">
      <label style="flex:1">Le dossier
        <span style="display:flex;gap:6px">
          <input type="text" id="import-dossier" style="flex:1" placeholder="vide : le dossier import/ de la compagnie"
                 value="${echapper(IMPORT_DOSSIER)}">
          <button type="button" class="secondaire" id="import-parcourir">Parcourir…</button>
          ${fenetreDuPoste() ? `<button type="button" class="secondaire" id="import-ouvrir">Ouvrir le dossier</button>` : ""}
        </span>
      </label>
      <button class="secondaire" id="import-apercu">Voir ce qui serait repris</button>
    </div>
    <div id="import-resultat"></div>`;
  const deja = e && ((e.reprises && e.reprises.length) || e.contrats);
  cible.innerHTML = `
    <div class="carte">
      <div id="import-etat">${vueEtatImportation(e)}</div>
      ${deja ? `<details class="relire"><summary>Relire un dossier — un fichier ajouté depuis, un
          salarié ou une année qui manquait</summary>
          <p class="note">Les fiches qui existent déjà ne sont pas touchées ; ce que les DSN ou les
            bulletins disaient autrement est seulement signalé. Ce qui est déjà repris ne se double pas.</p>
          ${formulaire}
        </details>`
      : formulaire}
    </div>`;
  document.getElementById("import-apercu").onclick = () => apercuImportation();
  // L'EXPLORATEUR (08/10/2026) : la boîte du système, ouverte par le poste.
  document.getElementById("import-parcourir").onclick = async () => {
    try {
      const r = await api("choisir-dossier", { titre: "Le dossier à reprendre" });
      if (r.dossier) document.getElementById("import-dossier").value = r.dossier;
    } catch (err) { dire(err.message, "erreur"); }
  };
  const ouvrir = document.getElementById("import-ouvrir");
  if (ouvrir) {
    ouvrir.onclick = async () => {
      try {
        const r = await api("ouvrir-import", { dossier: document.getElementById("import-dossier").value.trim() });
        // Le dossier ouvert se dit (#141) : l'explorateur peut s'ouvrir derrière la fenêtre.
        if (r && r.dossier) dire(`Dossier ouvert : ${r.dossier}`);
      } catch (err) { dire(err.message, "erreur"); }
    };
  }
}

/* L'ÉTAT (`importation-etat`), en une ligne et sans nommer de logiciel :
   « Historique repris le … : N bulletins, N déclarations, N contrats, de …
   à … ». Les bulletins : les paies qu'un bulletin a écrites ou complétées ;
   les déclarations : une DSN par mois déclaré (`declarations` de la reprise
   DSN) et une AEM par contrat qu'elle atteste ; les contrats : tous ceux de
   l'historique (`contrats`). Puis ce qui attend l'éditeur (`editeur`). */
function vueEtatImportation(e) {
  if (!e) return "";
  const reprises = e.reprises || [];
  const de = (source) => reprises.find((r) => r.source === source) || {};
  const compte = (n, mot) => (n ? `${n} ${mot}${n > 1 ? "s" : ""}` : "");
  const quoi = [
    compte(de("bulletins").nombre || 0, "bulletin"),
    compte((de("DSN").declarations || 0) + (de("AEM").nombre || 0), "déclaration"),
    compte(e.contrats || 0, "contrat"),
  ].filter((x) => x).join(", ");
  const bornes = [e.du, e.au, ...reprises.flatMap((r) => [r.du, r.au])]
    .filter((x) => x).map((x) => String(x).slice(0, 7)).sort();
  const [du, au] = [bornes[0], bornes[bornes.length - 1]];
  const periode = !du ? "" : du === au ? `, en ${moisEnClair(du)}` : `, de ${moisEnClair(du)} à ${moisEnClair(au)}`;
  const le = [e.importe_le, ...reprises.map((r) => r.le)].filter((x) => x).sort().pop();
  const repris = quoi ? `<div class="rang">
      <div class="rang-infos"><b class="rang-nom">Historique repris${le ? ` le ${echapper(jjmmaaaa(le))}` : ""}</b>
        ${echapper(quoi + periode)}</div>
      <div class="rang-etat"><span class="pastille bien">fait</span></div>
    </div>` : "";
  const editeur = (e.editeur || []).map((x) => `<div class="rang">
      <div class="rang-infos"><b class="rang-nom">${x.canal === "signalement" ? "Message à l'éditeur" : "Incident"}${
        x.quand ? ` du ${echapper(quand(x.quand))}` : ""}</b>${echapper(x.resume || "")}</div>
      <div class="rang-etat">${x.repondu ? `<span class="pastille bien">réponse reçue</span>`
        : x.parti ? `<span class="pastille alerte">attend l'éditeur</span>`
        : `<span class="pastille">pas encore parti</span>`}</div>
    </div>`).join("");
  return repris + (editeur ? `<h3>Ce qui attend l'éditeur</h3>${editeur}` : "");
}

/* D'où vient une valeur : la DSN d'un mois, ou un bulletin (fichier, page). */
function origineImport(e) {
  return e.source === "bulletin" || "bulletin" in e
    ? `le bulletin${e.fichier ? ` (${echapper(e.fichier)}${e.page ? `, page ${e.page}` : ""})` : ""}`
    : `la DSN de ${echapper(moisEnClair(e.mois))}`;
}

/* Ce que l'import met dans une case vide ou « A COMPLETER » d'une fiche
   existante (`completer`) : le moteur l'a décidé, la page le dit. */
function valeurImportee(v) {
  if (v && typeof v === "object") return Array.isArray(v)
    ? v.map((x) => (x && typeof x === "object" ? Object.values(x).join(" ") : x)).join(", ")
    : Object.entries(v).map(([k, x]) => `${k} ${x}`).join(", ");
  return v;
}

/* Une fiche que l'import crée, complète, laisse, ou sur laquelle il a à
   redire. Un écart vient d'une DSN (`dsn`, `mois`) ou d'un bulletin
   (`bulletin`, `fichier`, `page`) : la fiche fait foi, il est seulement
   signalé. Une case vide est complétée, et la page dit d'où. */
function fichesImportees(fiches, titre) {
  const lignes = (fiches || []).filter((f) => f).map((f) => {
    const aCompleter = Object.keys(f.completer || {}).length;
    const etat = f.etat === "completee" ? `<span class="pastille bien">complétée</span>`
      : f.etat === "existante" && aCompleter ? `<span class="pastille alerte">à compléter</span>`
      : f.etat === "existante" ? `<span class="pastille bien">existe déjà</span>`
      : f.etat === "creee" ? `<span class="pastille bien">créée (${echapper(f.cle)})</span>`
      : `<span class="pastille alerte">à créer</span>`;
    const completes = Object.entries(f.completer || {}).map(([champ, e]) =>
      `<div class="acte-dernier"><b>${echapper(champ)}</b> ${f.etat === "completee" ? "a reçu" : "recevra"}
         « ${echapper(valeurImportee(e.valeur))} », lu dans ${origineImport(e)} — la case était vide</div>`).join("");
    const ecarts = Object.entries(f.ecarts || {}).map(([champ, e]) => {
      const ou = "bulletin" in e
        ? `le bulletin (${echapper(e.fichier || "")}${e.page ? `, page ${e.page}` : ""}) disait`
        : `la DSN de ${echapper(moisEnClair(e.mois))} disait`;
      const lu = "bulletin" in e ? e.bulletin : e.dsn;
      return `<div class="acte-dernier">${ou} <b>${echapper(champ)}</b> = « ${echapper(lu)} »,
         la fiche dit « ${echapper(e.fiche)} » — la fiche fait foi</div>`;
    }).join("");
    const erreurs = (f.anomalies || []).filter((a) => a.gravite === "erreur").map((a) =>
      `<div class="acte-dernier faute">${echapper(a.champ)} : ${echapper(a.message)}</div>`).join("");
    return `<div class="rang import">
      <span class="rang-nom">${echapper(f.libelle)}</span>
      ${etat}
      <span class="rang-infos">${completes}${ecarts}${erreurs}</span></div>`;
  });
  if (!lignes.length) return "";
  return `<h3>${titre}</h3>${lignes.join("")}`;
}

/* Le compte rendu, fichier par fichier (`tri.fichiers`) : le chemin dans le
   dossier, le sort, la raison. Un PDF de plusieurs pages déplie ses pages. */
function compteRenduImport(tri) {
  if (!tri || !tri.fichiers) return "";
  const comptes = Object.entries(tri.comptes || {}).filter(([, n]) => n > 0)
    .map(([s, n]) => `${pastilleSort(s)} ${n}`).join(" &nbsp; ");
  const lignes = tri.fichiers.map((f) => {
    const pages = f.pages && f.pages.length > 1
      ? `<details class="relire"><summary>${f.pages.length} pages</summary>${f.pages.map((p) =>
          `<div class="acte-dernier">page ${p.page}${p.mois ? ` (${echapper(moisEnClair(p.mois))})` : ""}
             ${pastilleSort(p.sort)} ${echapper(p.raison || "")}</div>`).join("")}</details>` : "";
    const de = f.doublon_de ? ` <span class="discret">(copie de ${echapper(f.doublon_de)})</span>`
      : f.archive ? ` <span class="discret">(dans ${echapper(f.archive)})</span>` : "";
    return `<tr><td>${echapper(f.chemin)}${de}</td><td style="white-space:nowrap">${pastilleSort(f.sort)}</td>
      <td>${echapper(f.raison || "")}${pages}</td></tr>`;
  }).join("");
  return `<h3>Le dossier, fichier par fichier</h3>
    <p class="note">${tri.total} fichier${tri.total > 1 ? "s" : ""} dans
      <code>${echapper(tri.dossier || "")}</code> &nbsp; ${comptes}</p>
    <table class="tableau-detail"><thead><tr><th>Fichier</th><th>Sort</th><th>Raison</th></tr></thead>
      <tbody>${lignes}</tbody></table>`;
}

/* Les bulletins lus : combien, combien repris, de quand à quand, et ce
   qu'ils ajoutent (`bulletins`). Pas le logiciel qui les a faits. */
function bulletinsImportes(b, ecrit) {
  if (!b || !b.pages) return "";
  const periode = b.du ? ` du ${echapper(jjmmaaaa(b.du))} au ${echapper(jjmmaaaa(b.au))}` : "";
  const ajouts = [
    b.paies_nouvelles ? `${b.paies_nouvelles} paie${b.paies_nouvelles > 1 ? "s" : ""} qu'aucune DSN ne déclare` : "",
    b.paies_completees ? `${b.paies_completees} paie${b.paies_completees > 1 ? "s" : ""} déclarée${b.paies_completees > 1 ? "s" : ""} complétée${b.paies_completees > 1 ? "s" : ""} (cotisations, coût employeur, lignes)` : "",
    b.contrats_nouveaux ? `${b.contrats_nouveaux} contrat${b.contrats_nouveaux > 1 ? "s" : ""} attesté${b.contrats_nouveaux > 1 ? "s" : ""}` : "",
    b.fiches_a_creer ? `${b.fiches_a_creer} fiche${b.fiches_a_creer > 1 ? "s" : ""} à créer` : "",
  ].filter((x) => x);
  return `<div class="encart"><b>Bulletins de paie</b>
      ${b.lus} bulletin${b.lus > 1 ? "s" : ""} lu${b.lus > 1 ? "s" : ""} sur ${b.pages} page${b.pages > 1 ? "s" : ""}${
        periode} ; ${b.repris} ${ecrit ? `repris` : "à reprendre"}${
        ajouts.length ? ` : ${ajouts.join(", ")}` : ""}.</div>`;
}

/* Les écarts entre un bulletin et ce que la DSN (ou l'historique) disait
   de la même paie : le bulletin fait foi sur les montants, l'écart est
   noté (`tri.fichiers[].pages[].ecarts`). */
const CHAMPS_PAIE = { brut: "brut", net_imposable: "net imposable", net_a_payer: "net à payer", pas: "prélèvement à la source" };
function ecartsBulletinDsn(tri) {
  const lignes = [];
  for (const f of (tri && tri.fichiers) || []) {
    for (const p of f.pages || []) {
      for (const e of p.ecarts || []) {
        lignes.push(`<tr><td>${echapper(f.chemin)}${f.pages.length > 1 ? `, page ${p.page}` : ""}</td>
          <td>${echapper(moisEnClair(p.mois || ""))}</td>
          <td>${echapper(CHAMPS_PAIE[e.champ] || e.champ)}</td>
          <td class="nombre">${echapper(e.valeur == null ? "" : e.valeur)}</td>
          <td class="nombre">${echapper(e.bulletin == null ? "" : e.bulletin)}</td>
          <td>${e.le_bulletin_l_emporte ? `<span class="pastille alerte">le bulletin l'emporte</span>`
            : `<span class="pastille">noté</span>`}</td></tr>`);
      }
    }
  }
  if (!lignes.length) return "";
  return `<h3>Ce que les bulletins disent autrement</h3>
    <p class="note">Le bulletin fait foi : c'est son montant qui s'écrit, l'écart reste noté ici.</p>
    <table class="tableau-detail"><thead><tr><th>Bulletin</th><th>Mois</th><th>Champ</th>
      <th class="nombre">DSN ou historique</th><th class="nombre">Bulletin</th><th></th></tr></thead>
      <tbody>${lignes.join("")}</tbody></table>`;
}

/* Les fichiers qui n'ont pas pu être repris : la phrase de Léandre, et le
   message qui partirait chez l'éditeur, montré tel qu'il partirait (caviardé
   par le moteur, `importation-incident`). Rien ne part sans le clic sur
   « Envoyer » (`importation-incident-envoyer`). Quels fichiers n'ont pas
   été repris, c'est le moteur qui le dit (`fichiers` de la route) : la page
   ne fait que le montrer. */
async function preparerIncidentImport() {
  const cible = document.getElementById("import-incident");
  if (!cible) return;
  let r;
  try {
    r = await api("importation-incident", { dossier: IMPORT_DOSSIER });
  } catch (e) {
    cible.innerHTML = `<div class="encart erreur">Le message à l'éditeur ne se prépare pas : ${echapper(e.message)}</div>`;
    return;
  }
  const laisses = r.fichiers || [];
  if (!laisses.length && !r.a_envoyer) {
    cible.innerHTML = "";
    return;
  }
  const envoi = r.a_envoyer
    ? `<p class="note"><strong>Les noms, prénoms, numéros de sécurité sociale, dates de naissance et adresses
        ont été enlevés</strong>${r.resume ? ` (${echapper(r.resume)})` : ""}. Les fichiers eux-mêmes ne
        partent pas. Voici le message, tel qu'il partirait :</p>
      <pre class="reponse">${echapper(r.texte || "")}</pre>
      <p><button class="primaire" id="import-incident-envoyer">Envoyer</button>
        <span class="note">Rien ne part avant ce clic.</span></p>`
    : `<p class="note">Rien à envoyer.</p>`;
  cible.innerHTML = `<div class="encart attention"><b>Ces documents n'ont pas pu être repris.</b>
      ${laisses.map((f) => `<div class="acte-dernier">${echapper(f.chemin)} ${pastilleSort(f.sort)}
        ${echapper(f.raison || "")}</div>`).join("")}
      <p>Un message peut partir chez le développeur ; Léandre s'en occupe et vous répond.</p>
      <div id="import-incident-envoi">${envoi}</div>
    </div>`;
  const b = document.getElementById("import-incident-envoyer");
  if (b) b.onclick = () => envoyerIncidentImport();
}

async function envoyerIncidentImport() {
  const cible = document.getElementById("import-incident-envoi");
  const b = document.getElementById("import-incident-envoyer");
  if (b) b.disabled = true;
  try {
    const r = await api("importation-incident-envoyer", { dossier: IMPORT_DOSSIER });
    if (r.envoye) {
      cible.innerHTML = `<div class="encart bien"><b>Le message est parti.</b>${
        echapper(r.message || "Léandre s'en occupe et vous répond.")}</div>`;
      rafraichirEtatImportation();
    } else {
      cible.insertAdjacentHTML("beforeend", `<div class="encart erreur">${echapper(r.message || "Le message n'est pas parti.")}</div>`);
      if (b) b.disabled = false;
    }
  } catch (e) {
    cible.insertAdjacentHTML("beforeend", `<div class="encart erreur">${echapper(e.message)}</div>`);
    if (b) b.disabled = false;
  }
}

async function rafraichirEtatImportation() {
  const cible = document.getElementById("import-etat");
  if (!cible) return;
  try { cible.innerHTML = vueEtatImportation(await api("importation-etat")); } catch (e) { /* l'état se relira à la prochaine visite */ }
}

/* L'aperçu, ou ce qui a été repris. Aucun logiciel n'y est nommé (lot G2) :
   les déclarations sont des DSN et des AEM, les bulletins des bulletins ; le
   nom d'un gabarit ne reste que dans la raison d'un fichier non repris. */
function vueImportation(r) {
  const periode = r.premier_mois ? ` (${moisEnClair(r.premier_mois)} → ${moisEnClair(r.dernier_mois)})` : "";
  const contrats = r.contrats
    ? `${r.contrats} contrat(s) attesté(s) du ${jjmmaaaa(r.contrats_du)} au ${jjmmaaaa(r.contrats_au)} :
       <strong>${r.contrats_nouveaux} ${r.ecrit ? "ajouté(s)" : "à ajouter"}</strong> à l'historique,
       ${r.contrats - r.contrats_nouveaux - r.contrats_sans_fiche} déjà connu(s)${
         r.contrats_sans_fiche ? `, ${r.contrats_sans_fiche} sans fiche` : ""}`
    : "aucun contrat attesté par une AEM";
  const declarations = r.dsn || r.aem
    ? `${r.dsn} DSN${periode}, ${r.aem} fichier(s) AEM — ${contrats}`
    : "aucune DSN ni AEM dans le dossier";
  const avert = (r.avertissements || []).map((a) => `<div class="encart attention">${echapper(a)}</div>`).join("");
  const b = r.bulletins || {};
  const nb = [r.entreprise, ...(r.salaries || []), ...(r.spectacles || [])].filter((f) => f && (f.etat === "a_creer"
    || (f.etat === "existante" && Object.keys(f.completer || {}).length))).length;
  const nc = (r.contrats_nouveaux || 0) + (b.contrats_nouveaux || 0);
  const np = (b.paies_nouvelles || 0) + (b.paies_completees || 0);
  const quoi = [
    nb ? `${nb} fiche${nb > 1 ? "s" : ""}` : "",
    nc ? `${nc} contrat${nc > 1 ? "s" : ""}` : "",
    np ? `${np} bulletin${np > 1 ? "s" : ""}` : "",
  ].filter((x) => x);
  const bouton = r.ecrit
    ? `<div class="encart bien"><b>Repris.</b> ${(r.fichiers || []).map((f) => echapper(f)).join("<br>")}
        <p class="note">Les fichiers lus sont copiés dans l'historique de la compagnie ; vous pouvez vider
          le dossier <code>import/</code>.</p></div>`
    : r.rien_a_faire
      ? `<div class="encart bien"><b>Rien à reprendre</b> : tout est déjà là.</div>`
      : `<p><button class="primaire" id="import-ecrire">Reprendre${quoi.length ? " " + quoi.join(", ") : ""}</button>
         <span class="note">Rien n'est écrit avant ce clic.</span></p>`;
  return `<div id="import-incident"></div>
    ${compteRenduImport(r.tri)}
    <h3>${r.ecrit ? "Ce qui a été repris" : "Ce qui serait repris"}</h3>
    <div class="encart"><b>Déclarations</b>
      ${declarations}</div>
    ${bulletinsImportes(r.bulletins, r.ecrit)}
    ${avert}
    ${fichesImportees([r.entreprise], "Entreprise")}
    ${fichesImportees(r.salaries, "Salariés")}
    ${fichesImportees(r.spectacles, "Spectacles")}
    ${ecartsBulletinDsn(r.tri)}
    ${bouton}`;
}

function brancherImportation() {
  const b = document.getElementById("import-ecrire");
  if (b) b.onclick = () => ecrireImportation();
  if (document.getElementById("import-incident")) preparerIncidentImport();
}

async function apercuImportation() {
  IMPORT_DOSSIER = document.getElementById("import-dossier").value.trim();
  const cible = document.getElementById("import-resultat");
  cible.innerHTML = `<p class="discret">Lecture du dossier…</p>`;
  try {
    const r = await api("importation-apercu", { dossier: IMPORT_DOSSIER });
    cible.innerHTML = vueImportation(r);
    brancherImportation();
  } catch (e) {
    cible.innerHTML = `<div class="encart erreur">${echapper(e.message)}</div>`;
  }
}

async function ecrireImportation() {
  const cible = document.getElementById("import-resultat");
  cible.classList.add("attente");
  try {
    const r = await api("importer", { dossier: IMPORT_DOSSIER });
    cible.innerHTML = vueImportation(r);
    brancherImportation();
    // Les fiches créées doivent apparaître dans Salariés et Spectacles, et
    // dans les déroulants de la saisie, sans recharger la page.
    ETAT = await api("etat");
    rafraichirReferentiels();
    rafraichirEtatImportation();
    const b = r.bulletins || {};
    dire(`Repris : ${(r.contrats_nouveaux || 0) + (b.contrats_nouveaux || 0)} contrat(s), ${
      (b.paies_nouvelles || 0) + (b.paies_completees || 0)} bulletin(s), ${
      [r.entreprise, ...(r.salaries || []), ...(r.spectacles || [])].filter((f) => f && (f.etat === "creee" || f.etat === "completee")).length} fiche(s).`, "ok");
  } catch (e) {
    cible.innerHTML = `<div class="encart erreur">${echapper(e.message)}</div>`;
  } finally {
    cible.classList.remove("attente");
  }
}

/*  LA FICHE ENTREPRISE SE LIT (22/09/2026)
 *
 *  Elle etait une liste plate de trente cases, des accolades JSON a taper
 *  pour le signataire et la banque, une etiquette « DÉCLARÉ » sous chacune,
 *  et le SIREN, le NIC et le SIRET demandes trois fois de suite. L'editeur :
 *  « tout est un peu mélangé (...) pas très friendly pour ceux qui
 *  comprennent pas le code ».
 *
 *  Quatre decisions, et aucune ne touche au moteur :
 *
 *   - des SECTIONS (SECTIONS_FICHE) : on cherche « ma banque », pas « le
 *     champ 27 » ;
 *   - les structures se saisissent en SOUS-CHAMPS (SOUS_CHAMPS), une case
 *     par valeur, un « + » pour une liste. Les clefs qu'on ne montre pas
 *     (`_note`, `nom_aem`...) sont relues telles quelles a l'enregistrement
 *     — voir `lireFormulaire` : on ne perd pas ce qu'on n'affiche pas ;
 *   - ce que la machine DEDUIT ne se saisit pas : le SIREN et le NIC sont
 *     les 9 et les 5 chiffres du SIRET, poses en cases cachees (CACHES) ;
 *   - l'aide tient derriere un « ? » : presente pour qui la cherche,
 *     absente pour qui ne la cherche pas. Et l'etiquette « déclaré » ne
 *     s'affiche plus : savoir ce qui part en DSN est le travail du moteur.
 *
 *  `donnees/champs-base.json` (lu par `champs.rs`) reste la seule description des champs : ces tables ne
 *  disent que l'ORDRE et la MISE EN PAGE. Un champ nouveau qui n'y figure
 *  pas s'affiche quand meme, dans la derniere section.
 */
const SECTIONS_FICHE = {
  // LA FICHE ENTREPRISE, RELUE LE 23/09/2026. L'editeur : « on demande le
  // minimum d'information aux gens, après on traite l'information pour que
  // ça rentre dans la DSN ». Six blocs, dans l'ordre ou l'on cherche : qui
  // l'on est, ou l'on est, sous quelle convention, ses numeros, qui signe,
  // le compte. Les cases que le moteur deduit sont nommees ici pour que
  // leur case cachee soit posee dans le bloc de la valeur dont elles
  // viennent (voir CACHES).
  entreprise: [
    // La date de la première embauche sort de l'« Avancé » (30/09/2026) :
    // pour une compagnie de moins de cinq ans, sans elle l'effectif est
    // inconnu et le premier mois ne se calcule pas.
    ["La compagnie", ["nom_affichage", "siret", "siren", "nic", "naf",
                      "licence_spectacle", "premiere_embauche"]],
    ["Adresse", ["adresse_affichage", "code_postal", "commune_affichage"]],
    ["Convention collective", ["idcc", "convention_collective",
                               "code_convention_dsn", "annexe_convention",
                               "jauge_annexe_1", "type_radio_1922",
                               "cinquante_salaries_3241"]],
    // LE CURSEUR DES PERMANENTS SORT DE L'« AVANCÉ » (ticket #214, 20/10/2026).
    // C'est la question qui commande tout ce qui ne vaut que pour les
    // salariés au mois (natures CDI/CDD, santé et prévoyance, organismes) :
    // elle se pose en clair. Le moteur le lit (`contexte::emploie_des_permanents`).
    // Les conseils « Le saviez-vous ? » (ticket 224, Léandre, 21/10/2026) :
    // une option qui parle de vos salariés, elle se pose avec eux.
    ["Vos salariés", ["emploie_des_permanents", "conseils"]],
    // Le taux AT/MP se pose en tête de ce bloc (`ATMP_HTML`).
    ["Vos numéros chez les organismes",
     ["urssaf_compte", "congesspectacles_adherent",
      "congesspectacles_affiliation",
      "affiliation_france_travail", "code_employeur_france_travail",
      "france_travail_dossier_recouvrement"], { atmp: true }],
    ["Qui signe, qui répond", ["signataire", "declarant_dsn"]],
    ["Paiement des cotisations", ["banque"]],
    // LA TAXE SUR LES SALAIRES (lot V3b, 10/10/2026) : la DSN de décembre
    // déclare chaque année si la compagnie la doit (bloc S21.G00.44), et ne
    // se fait pas sans réponse. Le moteur ne connaît pas sa TVA.
    ["La TVA et la taxe sur les salaires", ["taxe_sur_les_salaires"]],
    // L'AVANCE (23/09/2026). L'editeur : « si on peut deviner que c'est le
    // truc de base, on le met en truc de base ; si les gens veulent changer,
    // ils cliquent sur avancé ». Chacun de ces champs a une valeur par
    // defaut qui est le cas ordinaire d'une compagnie : pas de salarie au
    // mois, pas de syndicat, pas de CSE, pas d'exemption, et un effectif lu
    // dans les retours de l'Urssaf plutot qu'une date de premiere embauche.
    // Le bloc s'ouvre de lui-meme si l'un d'eux s'ecarte du cas ordinaire.
    // Chaque case suit ce qui la commande (26/09/2026) : les fériés chômés
    // et la prévoyance ne se montrent qu'avec des permanents, la date
    // d'adhésion qu'avec un syndicat, le CSE qu'en CCNEAC, l'exemption
    // CASC-SVP qu'en 3090, l'article 45 qu'au cinéma.
    ["Avancé — cas particuliers", ["jours_feries_chomes", "plafond_temps_partiel",
      "deduction_forfaitaire_option",
      "prevoyance_permanents", "sante_permanents",
      "organisation_patronale", "organisation_patronale_depuis",
      "representation_personnel", "casc_svp_exemptee",
      "prevoyance_techniciens_3097",
      "contribution_supplementaire_apprentissage",
      "taxe_apprentissage_motif_non_assujettissement",
      "code_urssaf", "retraite_complementaire", "etablissements"],
     { avance: true }],
    // Sans titre : l'historique se replie derriere un lien (`histoireDuChamp`).
    ["", ["historique"]],
  ],
  // LA FICHE SALARIE, RELUE LE 23/09/2026 sur le modele de l'entreprise.
  // Dans l'ordre ou on la remplit, papiers en main : la carte Vitale, la
  // piece d'identite, l'adresse ; puis ce que l'employeur decide, l'emploi.
  // Chaque champ a ete confronte a ce qui le lit dans le moteur : le taux de
  // PAS de la fiche n'etait lu par personne, il ne se montre plus ; le sexe
  // ne sert que sans NIR, il ne se montre que dans ce cas (`brancherSexe`).
  salarie: [
    // Le numero et sa cle cote a cote, comme sur la carte Vitale.
    ["Identité", ["prenom_affichage", "nom_affichage", "nir", "nir_cle",
                  "date_naissance", "sexe"]],
    ["Naissance et nationalité", ["lieu_naissance_affichage",
      "departement_naissance", "pays_naissance", "pays_nationalite",
      "pays_nationalite_hors_liste", "code_nationalite_dsn"]],
    ["Coordonnées", ["adresse_affichage", "code_postal", "commune_affichage",
                     "email"]],
    // LE MÉTIER SE CHOISIT, SES CODES SE DÉDUISENT (30/09/2026). La liste
    // des métiers cochés (Configuration › Métiers) se pose en tête du bloc
    // (`METIER_HTML`) ; l'intitulé reste à portée de main — c'est lui que
    // lit le contrat. Statut, cadre, PCS-ESE et code France Travail se
    // remplissent au choix du métier : ils passent dans l'« Avancé ».
    ["Emploi", ["emploi_affichage"], { metier: true }],
    ["Intermittence", ["num_conges_spectacles", "anniversaire_droits"]],
    // Rien a remplir tant que la compagnie n'emploie pas de salarie au mois.
    ["Salarié au mois", ["sante_dispense", "sante_dispense_depuis",
                         "sante_dispense_jusqu_au", "prevoyance_branche_depuis",
                         "categorie_conventionnelle", "conges_reprise",
                         "forfait_jours"],
     { sante: true }],
    // L'avance : ce qui a une valeur par defaut juste pour presque tout le
    // monde. Le tiroir s'ouvre seul si l'une s'en ecarte (`ORDINAIRE`).
    ["Avancé — cas particuliers", ["statut", "statut_categoriel_rc",
      "pcs_ese", "code_emploi_ft", "date_anciennete", "code_salarie",
      "initiales_contrat", "statut_conventionnel", "deduction_forfaitaire",
      "deduction_forfaitaire_accord", "statut_boeth", "echelon_ccneac", "classement_conventionnel",
      "poste_depuis", "experimente_3241", "stagiaire_ccneac",
      "etablissement", "actif"], { avance: true }],
    ["", ["historique"]],
  ],
};

/* Ce qui, dans le tiroir « Avancé », vaut le cas ordinaire et ne l'ouvre
 * donc pas. Une case absente d'ici ouvre le tiroir des qu'elle est remplie. */
const ORDINAIRE = {
  code_urssaf: (v) => String(v) === String(ETAT.urssaf_deduit || ""),
  retraite_complementaire: (v) => String(v).trim().toUpperCase() === "AUDIENS",
  // Toujours remplis, et c'est normal : un matricule, un prefixe, une date.
  code_salarie: () => true,
  initiales_contrat: () => true,
  date_anciennete: () => true,
  actif: (v) => v !== false,
  statut_conventionnel: (v) => String(v) === "05",
  // Ce que le métier remplit : toujours là, et l'« Avancé » n'a pas à
  // s'ouvrir pour eux.
  statut: () => true,
  statut_categoriel_rc: () => true,
  pcs_ese: () => true,
  code_emploi_ft: () => true,
};

/* Les champs qu'on peut laisser vides : l'ecran le dit a cote du libelle,
 * plutot que de laisser croire qu'il manque quelque chose. L'editeur,
 * 23/09/2026 : « il y a des champs facultatifs, par exemple la date
 * anniversaire ». */
const FACULTATIFS = {
  salarie: ["num_conges_spectacles", "anniversaire_droits", "sante_dispense",
            "sante_dispense_depuis", "sante_dispense_jusqu_au",
            "prevoyance_branche_depuis", "echelon_ccneac",
            "classement_conventionnel", "conges_reprise",
            "categorie_conventionnelle", "poste_depuis"],
};

/* Les champs que la machine deduit d'un autre, ou qu'elle connait sans
 * qu'on les lui dise : ils restent dans le formulaire — donc ils repartent
 * a l'enregistrement avec leur valeur — mais en case cachee.
 *
 *  - `siren`, `nic` : le SIRET ; `convention_collective`,
 *    `code_convention_dsn` : l'IDCC (`brancherDeduits`, `brancherConvention`) ;
 *  - `congesspectacles_affiliation` : le numero d'adherent precede de 97,
 *    `code_employeur_france_travail` : l'affiliation sans sa cle ;
 *  - `etablissements` (le moteur n'en declare qu'un) : garde tel quel,
 *    jamais demande.
 *
 *  Le code Urssaf et la caisse de retraite, eux, sont de VRAIS choix : ils
 *  vont dans l'avance, vides par defaut -- le moteur prend alors l'Urssaf
 *  du code postal et Audiens. L'editeur : « il ne faut pas enlever de
 *  l'interface les choix que peut faire l'entreprise ». */
const CACHES = {
  entreprise: ["siren", "nic", "convention_collective", "code_convention_dsn",
               "congesspectacles_affiliation", "code_employeur_france_travail",
               "etablissements"],
  // `sante_svp` se remplit par les trois cases de la complementaire sante ;
  // `taux_pas` n'est plus lu par personne (il vit dans config/pas.json) :
  // gardes tels quels, jamais demandes.
  salarie: ["sante_svp", "taux_pas"],
};

/* Une structure, case par case. `liste: true` = plusieurs lignes, avec un
 * « + ». Ce qui n'est pas nomme ici est CONSERVE tel quel.
 *
 * LES PERSONNES SE SAISISSENT COMME ON LES ECRIT (23/09/2026). Le nom et le
 * prenom en capitales sans accents, c'est la forme de l'AEM et de la DSN :
 * le moteur la fabrique (`champs.deriver_personnes`), on ne la demande plus. */
const SOUS_CHAMPS = {
  // Le solde de congés d'un salarié au mois arrivé d'un autre logiciel
  // (§111) : ce que son dernier bulletin disait.
  conges_reprise: { champs: [
    ["date", "PuppetPaie compte à partir du", "AAAA-MM-JJ : le premier jour du premier mois produit par PuppetPaie"],
    ["report", "Reste N-1 (jours)", "Les jours de l'année de référence précédente qui restent à prendre"],
    ["acquis_report", "Acquis N-1 en tout (jours)", "Souvent 30 : il sert à payer un jour au dixième"],
    ["remuneration_report", "Brut de la période N-1 (€)", "L'assiette du dixième des jours N-1"],
    ["acquis", "Acquis N (jours)", "Les jours acquis sur la période en cours, avant cette date"],
    ["pris", "Pris sur N (jours)", "Les jours de la période en cours déjà pris avant cette date"],
    ["remuneration_periode", "Brut de la période N (€)", "Le brut versé sur la période en cours, avant cette date"],
  ] },
  signataire: { champs: [
    ["prenom_affichage", "Prénom", ""],
    ["nom_affichage", "Nom", ""],
    ["fonction", "Fonction", "Président, présidente, gérant, directrice…"],
    ["tel", "Téléphone", ""],
    ["email", "Courriel", ""],
  ] },
  // LA QUALITE DU DECLARANT NE S'AFFICHE PAS (22/09/2026). C'est
  // S20.G00.07.004, et elle vaut 04 : << la personne qui etablit la paie
  // dans l'entreprise >>. La valeur reste dans la fiche et repart telle
  // quelle (STRUCTURES_ORIGINE) ; `dsn.rs` la lit comme avant.
  // LA CIVILITE DU CONTACT (V1h, 10/10/2026) : S10.G00.02.001, 01 monsieur ou
  // 02 madame (cahier technique DSN, p. 132). Le moteur la lit ici et refuse
  // une DSN sans elle ; vide, la case est retiree a l'enregistrement.
  declarant_dsn: { champs: [
    ["civilite", "Civilité", "Celle de la personne que les organismes contactent : elle s'écrit dans chaque DSN.",
     { choix: { "": "—", "02": "Madame", "01": "Monsieur" } }],
    ["prenom_affichage", "Prénom", ""],
    ["nom_affichage", "Nom", ""],
    ["tel", "Téléphone", ""],
    ["email", "Courriel", ""],
  ] },
  banque: { champs: [
    ["iban", "IBAN", "Sur votre relevé d'identité bancaire (RIB) : 27 caractères commençant par FR."],
    ["bic", "BIC", "Sur le même RIB, 8 ou 11 caractères."],
  ] },
  // LA PREVOYANCE ET LA SANTE DES PERMANENTS (24/09/2026, journal §129).
  // Elles se saisissaient en JSON dans une zone de texte : un administrateur
  // n'avait pas a lire des accolades. Une case vide est RETIREE a
  // l'enregistrement, et le moteur applique alors la convention. Le format
  // des donnees ne change pas : « taux.t1_cadre » est la case `t1_cadre` du
  // bloc `taux`.
  prevoyance_permanents: { champs: [
    ["regime", "Régime", "Le cas ordinaire : celui de la convention, que le logiciel applique seul. Un contrat d'entreprise ne se choisit que s'il a été réellement souscrit, et plus favorable.",
     { choix: { "": "Celui de la convention (le cas ordinaire)",
                "contrat": "Un contrat d'entreprise plus favorable" } }],
    ["taux.t1_non_cadre", "Taux non-cadres (%)", "Seulement pour un contrat d'entreprise : son taux sur la tranche A, pour les non-cadres.", { nombre: true }],
    ["taux.t1_cadre", "Taux cadres (%)", "Seulement pour un contrat d'entreprise : son taux sur la tranche A, pour les cadres (au moins 1,50 %).", { nombre: true }],
    ["taux.part_patronale", "Part employeur (%)", "Seulement pour un contrat d'entreprise. 100 % pour les cadres, obligatoirement.", { nombre: true }],
    ["extension_t2", "Extension sur la tranche 2", "Facultative, si elle a été souscrite.",
     { choix: { "": "Aucune (le cas ordinaire)", "tous": "Pour tous les permanents",
                "cadres": "Pour les cadres seulement" } }],
    ["adhesion", "Organisme et numéro d'adhésion", "Pour mémoire : il ne part dans aucun calcul."],
  ] },
  sante_permanents: { champs: [
    ["taux_contrat", "Taux du contrat (% du plafond mensuel)", "Seulement pour une convention sans régime de santé (production audiovisuelle 2642) : le taux de votre contrat, pour le salarié seul. Il est sur votre contrat ou votre appel de cotisation.", { nombre: true }],
    ["part_patronale", "Part employeur (%)", "Au moins 50 %. Vide : 50 %.", { nombre: true }],
  ] },
  // LA TAXE SUR LES SALAIRES (ticket #216, 20/10/2026). On demande ce que la
  // compagnie sait de sa TVA, pas la conclusion du droit : le moteur conclut
  // (route `taxe-sur-les-salaires-regime`, CGI art. 231), dit quelles cases
  // suivent et écrit la conséquence sous elles (`brancherTaxeSalaires`). Les
  // oui/non s'enregistrent en vrai et faux (`booleen`).
  taxe_sur_les_salaires: { champs: [
    ["tva", "La compagnie et la TVA", "Assujettie : elle facture la TVA sur ses ventes (billetterie, cessions…) et la déclare. Exonérée : elle ne facture pas de TVA, ou ses activités n'y entrent pas — c'est le cas de beaucoup d'associations.",
     { choix: { "": "— à dire —", "assujettie": "Assujettie à la TVA", "exoneree": "Exonérée de TVA (ou hors de son champ)" } }],
    ["part_tva", "Part de vos recettes de l'an dernier soumise à la TVA (%)", "Vos recettes soumises à la TVA, divisées par toutes vos recettes et produits de l'année, subventions et dons compris. Votre comptable la connaît. À 90 % ou plus, pas de taxe sur les salaires.", { nombre: true }],
    ["sous_franchise", "Chiffre d'affaires de l'an dernier et franchise en base de TVA", "Les plafonds de la franchise en base de TVA (code général des impôts, art. 293 B), ceux qui dispensent une petite entreprise de facturer la TVA. En dessous, pas de taxe sur les salaires.",
     { booleen: true, choix: { "": "— à dire —", "false": "Au-dessus des plafonds", "true": "Sous les plafonds" } }],
    ["association_1679A", "Association loi 1901, syndicat ou mutuelle ?", "Une association loi 1901, une fondation reconnue d'utilité publique, un syndicat, une mutuelle de moins de trente salariés : l'abattement de l'article 1679 A s'applique à la taxe due.",
     { booleen: true, choix: { "": "Non", "true": "Oui (abattement de l'article 1679 A)" } }],
  ] },
  etablissements: { liste: true, ligne: "établissement", champs: [
    ["nic", "NIC", ""],
    ["siret", "SIRET", ""],
    ["libelle", "Nom de l'établissement", ""],
  ] },
};

/* Les valeurs d'une liste de choix, dites en clair. Le moteur garde ses
 * codes (`cse_adherent_fnas`), l'ecran montre une phrase. */
const LIBELLES_CHOIX = {
  // La table MNATA de la DSN (lot R2), telle que `donnees/mnata-base.json`
  // la livre : l'ecran la dit en clair, le moteur la lit et la controle.
  taxe_apprentissage_motif_non_assujettissement: {
    "": "— la compagnie doit la taxe (cas ordinaire) —",
    "001": "001 — Non assujetti par choix du régime fiscal (option IR en catégorie BNC)",
    "002": "002 — Collectivités publiques, Parlementaires",
    "003": "003 — Etablissements publics de recherche et les établissements publics d'enseignement supérieurs",
    "004": "004 — Personnes morales créées pour la gestion d'un pôle de recherche et d'enseignement supérieur ou d'un réseau thématique de recherche avancée",
    "005": "005 — Fondations reconnues d'utilité public du secteur de la recherche",
    "006": "006 — Sociétés et personnes morales ayant pour objet exclusif l'enseignement",
    "007": "007 — Associations sans but lucratif",
    "008": "008 — Syndicats professionnels",
    "009": "009 — Entreprises sans établissement en France (ESEF)",
    "010": "010 — Sociétés civiles de moyens (SCM) non commerciale",
    "011": "011 — Personnes physiques ou morales assujetties aux BNC ou BA",
    "012": "012 — Les coopératives agricoles",
    "013": "013 — Les coopératives artisanales, transport (y compris fluvial et maritime) et leurs unions",
    "014": "014 — Les groupements d'employeurs dont agricoles",
    "015": "015 — Les sociétés civiles agricoles",
    "016": "016 — Les mutuelles et organismes mutualistes",
    "017": "017 — Les organismes HLM, SA de crédit immobilier (I et au II de l'article L. 422-4 du code de la construction et de l'habitation ) et unions d'économie sociale",
    "018": "018 — Les sociétés coopératives de construction désignées à l'article L. 432-2 du code de la construction et de l'habitation.",
    "019": "019 — Organismes chargés de l'organisation en France d'une compétition sportive internationale en France (Article L6131-1 du code du travail)",
  },
  annexe_convention: {
    "1": "Annexe 1 — lieux, production ou diffusion de théâtre, danse, lyrique, musique classique",
    "2": "Annexe 2 — chanson, variétés, jazz, musiques actuelles",
    "3": "Annexe 3 — cabarets",
    "4": "Annexe 4 — production ou diffusion de spectacles en tournée",
    "5": "Annexe 5 — cirque",
    "6": "Annexe 6 — bals",
  },
  representation_personnel: {
    "aucune": "Pas de CSE (carence constatée)",
    "csec": "CSE conventionnel (11 à 49 salariés)",
    "cse_adherent_fnas": "CSE de 50 salariés et plus, adhérent au FNAS",
    "cse": "CSE qui n'adhère pas au FNAS",
  },
  sexe: { "1": "Homme", "2": "Femme" },
  code_nationalite_dsn: {
    "01": "Française",
    "02": "D'un pays de l'Union européenne",
    "03": "Islandaise, liechtensteinoise ou norvégienne (EEE hors Union)",
    "04": "D'un autre pays (Royaume-Uni et Suisse compris)",
  },
  statut: {
    "artiste": "Artiste",
    "technicien": "Technicien, ou autre emploi non artistique",
  },
  statut_categoriel_rc: {
    "04": "Non cadre",
    "01": "Cadre",
    "02": "Extension cadre pour la retraite complémentaire (code 02 de la DSN)",
  },
  statut_conventionnel: {
    "05": "Profession intermédiaire, technicien, agent de maîtrise — les artistes",
    "04": "Cadre au sens de la convention",
    "03": "Cadre dirigeant (vote au collège employeur des prud'hommes)",
    "06": "Employé administratif",
    "07": "Ouvrier ou employé technique",
  },
  // La règle est la proratisation (CSS L. 242-8, fiche net-entreprises 3369) ;
  // « entier » dit que l'établissement y renonce. Le moteur décide.
  plafond_temps_partiel: {
    "proratisé": "Proratisé à la durée du contrat (la règle)",
    "entier": "Entier : l'établissement renonce à la proratisation",
  },
  // La déduction forfaitaire spécifique (arrêté du 4 septembre 2025, art. 9) : le
  // moteur décide (`calcul::deduction_forfaitaire`), la page ne fait que nommer.
  deduction_forfaitaire_option: {
    "": "Non appliquée — le cas ordinaire",
    "accord_collectif": "Oui : un accord collectif l'a explicitement prévue",
    "accord_cse": "Oui : le CSE a donné son accord",
    "accord_individuel": "Oui : chaque salarié l'accepte (contrat ou lettre)",
  },
  deduction_forfaitaire: {
    "": "Aucune — pas de déduction pour ce salarié",
    "musicien": "Artiste musicien (catégorie 20 %)",
    "choriste": "Choriste (catégorie 20 %)",
    "chef_orchestre": "Chef d'orchestre (catégorie 20 %)",
    "regisseur_theatre": "Régisseur de théâtre (catégorie 20 %)",
    "artiste_dramatique": "Artiste dramatique (catégorie 25 %)",
    "artiste_lyrique": "Artiste lyrique (catégorie 25 %)",
    "artiste_choregraphique": "Artiste chorégraphique (catégorie 25 %)",
    "artiste_cinematographique": "Artiste de cinéma (catégorie 25 %)",
  },
  deduction_forfaitaire_accord: {
    "": "Pas encore dit",
    "contrat": "Dans son contrat de travail ou un avenant",
    "consentement_annuel": "Par son consentement de l'année (son silence vaut accord)",
    "refus": "Il a refusé : pas de déduction",
  },
};

/* Ou aller chercher une valeur qu'on n'a pas sous la main : une phrase et
 * un lien, derriere le « ? » du champ. VIDE DEPUIS LE 23/09/2026 : les
 * liens menaient a la page d'accueil d'un organisme, ce qui n'apprend rien.
 * L'aide de chaque champ (`donnees/champs-base.json`) dit maintenant ou lire la
 * valeur. Un lien ne revient ici que s'il mene a la page qui la donne. */
const OU_TROUVER = {};

/* Les structures telles qu'elles etaient AVANT la saisie : `lireFormulaire`
 * en repart pour ne pas perdre les clefs qu'on n'affiche pas. */
let STRUCTURES_ORIGINE = {};

/* L'aide derriere un « ? » : le bouton dans le libelle, le texte dessous,
 * replie. `OU_TROUVER` y ajoute une phrase et un lien quand la valeur se
 * demande a quelqu'un d'autre qu'a soi-meme. */
function aideHtml(cle, aide) {
  return (aide || OU_TROUVER[cle])
    ? `<button type="button" class="aide-bouton" data-aide="${echapper(cle)}"
         title="à quoi sert ce champ, où le trouver">?</button>` : "";
}

function aideCorps(cle, aide) {
  const ou = OU_TROUVER[cle];
  if (!aide && !ou) return "";
  return `<div class="aide" data-aide-corps="${echapper(cle)}" hidden>
    ${ou ? `<p class="ou-trouver">${echapper(ou[0])}
        <a href="${echapper(ou[1])}" target="_blank" rel="noopener">${echapper(ou[1])}</a></p>` : ""}
    ${aide ? `<p>${echapper(aide)}</p>` : ""}
  </div>`;
}

/*  UNE STRUCTURE SE SAISIT CASE PAR CASE, PAS EN ACCOLADES.
 *
 *  Deux formes : un objet (le signataire, la banque) rendu en une rangee de
 *  cases ; une liste (les licences, les etablissements) rendue en lignes,
 *  avec un « + » pour en ajouter une et une croix pour en retirer une. Ce
 *  que la table ne nomme pas n'est pas affiche, et n'est pas perdu.
 */
function structureHtml(c, v, sous, a, avecPermanents) {
  const cache = c.si_permanents && !avecPermanents;
  const tete = `<label class="titre-structure">${echapper(c.libelle)}
      ${c.obligatoire ? '<span class="obligatoire">*</span>' : ""}
      ${aideHtml(c.cle, c.aide)}</label>
    ${aideCorps(c.cle, c.aide)}`;
  let corps;
  if (sous.liste) {
    const lignes = Array.isArray(v) ? v : (v ? [v] : []);
    corps = `<div class="liste-structure" data-liste="${echapper(c.cle)}">
        ${lignes.map((l, i) => ligneStructure(c.cle, sous, l, i)).join("")}
      </div>
      <button type="button" class="secondaire ajouter-ligne"
        data-ajouter="${echapper(c.cle)}">+ Ajouter un ${echapper(sous.ligne || "élément")}</button>`;
  } else {
    const o = (v && typeof v === "object" && !Array.isArray(v)) ? v : {};
    corps = `<div class="sous-champs">${sous.champs.map(([k, lib, aide, genre = {}]) => `
      <div class="sous-champ">
        <label>${echapper(lib)}
          ${aide ? `<button type="button" class="aide-bouton"
             data-aide="${echapper(c.cle + "." + k)}" title="aide">?</button>` : ""}
          ${caseSousChamp(c.cle, k, valeurEn(o, k), genre)}</label>
        ${aide ? `<div class="aide" data-aide-corps="${echapper(c.cle + "." + k)}" hidden>
            <p>${echapper(aide)}</p></div>` : ""}
      </div>`).join("")}</div>`;
  }
  return `<div class="champ pleine structure${c.si_permanents ? " champ-permanents" : ""}"${
      cache ? " hidden" : ""}>
    ${tete}
    ${corps}
    ${a ? `<div class="anomalie ${a.gravite}">${echapper(a.message)}</div>` : ""}
  </div>`;
}

/* Une valeur dans un objet, par son chemin : « taux.t1_cadre ». */
function valeurEn(o, chemin) {
  return chemin.split(".").reduce((x, k) => (x && typeof x === "object") ? x[k] : undefined, o);
}

/* La case d'un sous-champ : une liste de choix, un nombre ou du texte. Les
 * deux premiers portent `data-sous-type` : vides, ils sont retires a
 * l'enregistrement (`lireFormulaire`). */
function caseSousChamp(parent, k, val, genre) {
  const attrs = `data-sous-de="${echapper(parent)}" data-sous-cle="${echapper(k)}"`;
  if (genre.choix) {
    // Un oui/non (`booleen`) se choisit comme un choix et s'enregistre en
    // vrai booleen (`lireFormulaire`).
    const type = genre.booleen ? "booleen" : "choix";
    return `<select ${attrs} data-sous-type="${type}">${Object.entries(genre.choix).map(([v, lib]) =>
      `<option value="${echapper(v)}" ${String(val ?? "") === v ? "selected" : ""}>${echapper(lib)}</option>`).join("")}</select>`;
  }
  if (genre.nombre) {
    return `<input type="number" step="0.01" min="0" ${attrs} data-sous-type="nombre"
      value="${echapper(val ?? "")}">`;
  }
  return `<input ${attrs} value="${echapper(val ?? "")}">`;
}

function ligneStructure(cle, sous, ligne, i) {
  const o = ligne && typeof ligne === "object" ? ligne : {};
  return `<div class="ligne-structure" data-index="${i}">
    ${sous.champs.map(([k, lib]) => `
      <label>${echapper(lib)}
        <input data-liste-de="${echapper(cle)}" data-liste-index="${i}"
          data-sous-cle="${echapper(k)}" value="${echapper(o[k] ?? "")}"></label>`).join("")}
    <button type="button" class="retirer-ligne" title="retirer cette ligne">✕</button>
  </div>`;
}

/* La fiche que le formulaire montre en ce moment : `quitterLaFiche` en a
 * besoin pour pouvoir l'enregistrer avant de partir. */
let FICHE_COURANTE = null;

/* La mention « facultatif », a cote du libelle d'un champ qu'on peut
 * laisser vide (FACULTATIFS). */
function facultatif(c) {
  return (FACULTATIFS[FICHE_ACTIVE] || []).includes(c.cle)
    ? ' <span class="facultatif">facultatif</span>' : "";
}

/* LE MÉTIER, en tête du bloc « Emploi » (revu le 30/09/2026). Un seul
 * déroulant, qui montre le métier de la fiche (son code France Travail) —
 * il affichait « — celui de la fiche — », emprunté à l'engagement, et ne
 * disait jamais ce qui était choisi. La liste est celle des métiers cochés ;
 * on en ajoute dans l'onglet Métiers, pas ici. */
function METIER_HTML(fiche) {
  const code = ((fiche || {}).donnees || {}).code_emploi_ft || "";
  return `<div class="champ" id="config-metier">
    <label>Métier <span class="obligatoire">*</span>
      <select id="config-metier-choix">${optionsMetiers(code, "— choisir —")}</select></label>
    <div class="discret petit">Un métier manque ? L'ajouter dans
      <button type="button" class="lien" id="aller-metiers">Configuration › Métiers</button>.</div>
  </div>`;
}

/* CE QU'UN SPECTACLE, OU UN SALARIÉ, A COÛTÉ DANS L'ANNÉE (30/09/2026).
 * L'éditeur : « si un administrateur veut savoir combien un de ses
 * spectacles lui a coûté dans l'année, il faut qu'il le trouve
 * facilement ». La réponse était dans Exports, un état parmi vingt-deux ;
 * elle est désormais en tête de la fiche. Le calcul est celui du livre de
 * paie (route « couts ») : une fois par année et par séance, oublié dès
 * qu'une saisie s'enregistre. */
const COUTS = {};
function coutsDe(annee) {
  if (!COUTS[annee]) COUTS[annee] = api("couts", { annee }).catch(() => null);
  return COUTS[annee];
}
function oublierCouts() { Object.keys(COUTS).forEach((k) => delete COUTS[k]); }

async function peindreCoutAnnee() {
  const el = document.querySelector("#config-formulaire .cout-annee");
  if (!el) return;
  const an = new Date().getFullYear();
  const ligne = (r, annee) => {
    const d = r && (r[el.dataset.cote] || {})[el.dataset.cle];
    if (!d || (!d.bulletins && !Number(d.prevu))) return "";
    const prevu = Number(d.prevu) ? ` · <span class="discret">+ ${echapper(euros(d.prevu))} déjà saisis pour la suite de l'année</span>` : "";
    return `<div><b>${annee}</b> : coût employeur <b>${echapper(euros(d.cout))}</b>
      <span class="discret">— brut ${echapper(euros(d.brut))}, charges ${echapper(euros(d.charges))},
      ${d.bulletins} bulletin${d.bulletins > 1 ? "s" : ""}${d.incomplet ? ", coût incomplet avant PuppetPaie" : ""}</span>${prevu}</div>`;
  };
  el.innerHTML = `<span class="discret">Coût de l'année…</span>`;
  const [cette, avant] = await Promise.all([coutsDe(an), coutsDe(an - 1)]);
  if (!el.isConnected) return;
  el.innerHTML = ligne(cette, an) + ligne(avant, an - 1);
}

/* L'ONGLET MÉTIERS (30/09/2026). L'éditeur : « les métiers, c'est vraiment
 * un onglet à part : on a les métiers de base de notre convention, on peut
 * en rajouter » — et les cocher ou les décocher. Ce qui est coché se
 * propose dans la fiche d'un salarié et dans un engagement ; rien d'autre
 * ne change, aucun calcul. Par famille, repliée, avec son compte ; la
 * recherche ouvre ce qu'elle trouve. Les métiers des autres conventions ne
 * se montrent que si on le demande. */
let METIERS_TOUS = false;
let METIERS_FILTRE = "";

async function dessinerMetiers(cible) {
  let r;
  try { r = await api("metiers-catalogue"); }
  catch (e) { cible.innerHTML = `<p class="discret">${echapper(e.message)}</p>`; return; }
  const sansAccent = (t) => String(t || "").normalize("NFD").replace(/[\u0300-\u036f]/g, "").toLowerCase();
  const peindre = () => {
    const filtre = sansAccent(METIERS_FILTRE.trim());
    const vus = r.metiers.filter((m) => (METIERS_TOUS || m.de_la_convention || m.retenu)
      && (!filtre || sansAccent(m.libelle).includes(filtre) || sansAccent(m.code).includes(filtre)));
    const familles = [...new Set(vus.map((m) => m.famille))];
    const retenus = r.metiers.filter((m) => m.retenu).length;
    document.getElementById("metiers-compte").textContent =
      `${retenus} métier${retenus > 1 ? "s" : ""} coché${retenus > 1 ? "s" : ""}`;
    document.getElementById("metiers-liste").innerHTML = familles.length ? familles.map((f) => {
      const dedans = vus.filter((m) => m.famille === f);
      const n = dedans.filter((m) => m.retenu).length;
      return `<details class="metiers-famille"${filtre ? " open" : ""}>
        <summary><b>${echapper(f)}</b> <span class="discret">${n} coché${n > 1 ? "s" : ""} sur ${dedans.length}</span></summary>
        <div class="metiers-colonnes">${dedans.map((m) => `
          <label class="coche" title="${echapper(m.code)}${m.de ? " — " + echapper(m.de) : ""}">
            <input type="checkbox" data-code="${echapper(m.code)}"${m.retenu ? " checked" : ""}>
            ${echapper(m.libelle)}${m.de ? ` <span class="discret">· autre convention</span>` : ""}
          </label>`).join("")}</div>
      </details>`;
    }).join("") : `<p class="discret">Aucun métier ne correspond.</p>`;
    document.querySelectorAll("#metiers-liste input[data-code]").forEach((c) => {
      c.onchange = async () => {
        try {
          const x = await api("metier-retenu", { code: c.dataset.code, retenu: c.checked });
          ETAT.emplois = x.emplois;
          ETAT.emplois_autres = x.emplois_autres;
          const m = r.metiers.find((y) => y.code === c.dataset.code);
          if (m) m.retenu = c.checked;
          const garde = [...document.querySelectorAll("#metiers-liste details[open] summary b")].map((b) => b.textContent);
          peindre();
          document.querySelectorAll("#metiers-liste details").forEach((d) => {
            if (garde.includes(d.querySelector("summary b").textContent)) d.open = true;
          });
        } catch (e) { c.checked = !c.checked; dire(e.message, "erreur"); }
      };
    });
  };
  cible.innerHTML = `
    <div class="carte">
      <h2>Les métiers de la compagnie</h2>
      <p class="note">Ceux qui sont cochés se proposent dans la fiche d'un
        salarié et dans un engagement. Ceux de votre convention collective
        le sont d'office.</p>
      <div class="metiers-outils">
        <input type="search" id="metiers-filtre" placeholder="Chercher un métier…"
               value="${echapper(METIERS_FILTRE)}">
        <label class="coche"><input type="checkbox" id="metiers-tous"${METIERS_TOUS ? " checked" : ""}>
          montrer aussi les autres conventions</label>
        <span id="metiers-compte" class="discret"></span>
      </div>
      <div id="metiers-liste"></div>
    </div>`;
  document.getElementById("metiers-filtre").oninput = (e) => { METIERS_FILTRE = e.target.value; peindre(); };
  document.getElementById("metiers-tous").onchange = (e) => { METIERS_TOUS = e.target.checked; peindre(); };
  peindre();
}

/* LES ORGANISMES ET LES CONTRATS DE LA DSN (09/10/2026). Une ligne par
 * contrat que la convention exige ; le moteur (`organismes-contrats`) dit ce
 * qui est fixé par la branche et ce que la compagnie saisit. Enregistrer
 * compose config/organismes.json (`organismes-enregistrer`) : la page ne
 * décide rien. */
async function dessinerOrganismes(cible) {
  let r;
  try { r = await api("organismes-contrats"); }
  catch (e) { cible.innerHTML = `<p class="discret">${echapper(e.message)}</p>`; return; }
  const champ = (c, k, fixe, place) => fixe
    ? `<span>${echapper(c[k]) || "—"}</span>`
    : `<input type="text" data-contrat="${echapper(c.cle)}" data-champ="${k}" value="${echapper(c[k])}"
         placeholder="${place}" maxlength="${k === "libelle" ? 30 : 9}">`;
  const ligne = (c) => `
    <tr>
      <td><b>${echapper(c.quoi)}</b>
        ${c.depuis ? `<div class="note">Depuis le ${echapper(jjmmaaaa(String(c.depuis || "").slice(0, 10)))}.</div>` : ""}
        ${c.note ? `<div class="note">${echapper(c.note)}</div>` : ""}</td>
      <td>${echapper(c.beneficiaires)}</td>
      <td>${champ(c, "organisme", c.organisme_fixe, "P0983")}</td>
      <td>${champ(c, "libelle", c.libelle_fixe, "référence du contrat")}</td>
    </tr>`;
  cible.innerHTML = `
    <div class="carte">
      <h2>Les organismes et les contrats de la DSN</h2>
      <p class="note">Ce que la DSN déclare de vos adhésions de prévoyance et de santé.
        La référence du contrat et le code de l'organisme se lisent sur le certificat
        d'adhésion et sur la fiche de paramétrage DSN de l'organisme. Le reste
        (organismes payés, Urssaf, taux AT/MP, compte France Travail) vient de la loi,
        de la convention et de la fiche Entreprise.</p>
      ${r.compose ? "" : `<p class="rappel retard">Pas encore enregistré : la DSN est refusée tant que ces contrats ne le sont pas.</p>`}
      <table class="tableau">
        <thead><tr><th>Contrat</th><th>Bénéficiaires</th><th>Organisme</th><th>Référence du contrat</th></tr></thead>
        <tbody>${r.contrats.map(ligne).join("")}</tbody>
      </table>
      <div class="actions"><button id="organismes-enregistrer">Enregistrer</button></div>
    </div>`;
  document.getElementById("organismes-enregistrer").onclick = async () => {
    const contrats = {};
    cible.querySelectorAll("input[data-contrat]").forEach((i) => {
      (contrats[i.dataset.contrat] ||= {})[i.dataset.champ] = i.value.trim();
    });
    try {
      await api("organismes-enregistrer", { contrats });
      dire("Organismes et contrats enregistrés.");
      dessinerOrganismes(cible);
    } catch (e) { dire(e.message, "erreur"); }
  };
}

/* LE TAUX AT/MP (30/09/2026), en tête du bloc « Vos numéros chez les
 * organismes ». Il n'est pas un champ de la fiche : il vit dans le barème
 * de la compagnie (`config/baremes/<année>.json`), s'enregistre seul, à la
 * sortie de la case, et ne marque pas la fiche « modifiée ». Sans lui, le
 * premier mois d'une compagnie neuve ne se calculait pas. */
function ATMP_HTML() {
  const an = new Date().getFullYear();
  return `<div class="encart" id="config-atmp" data-annee="${an}">
    <div class="ligne-champs">
      <label>Taux accident du travail (AT/MP) ${an}
        <input id="atmp-valeur" class="hors-fiche" inputmode="decimal" placeholder="1,25"></label>
      <label>Code risque
        <input id="atmp-code" class="hors-fiche" placeholder="923AD" maxlength="5"></label>
    </div>
    <div class="aide">En pourcentage, tel que l'Assurance Maladie le notifie
      chaque année (compte AT/MP sur net-entreprises). Nécessaire pour
      calculer la paie. <span id="atmp-etat"></span></div>
  </div>`;
}

async function brancherAtmp() {
  const bloc = document.getElementById("config-atmp");
  if (!bloc) return;
  const annee = Number(bloc.dataset.annee);
  const valeur = document.getElementById("atmp-valeur");
  const code = document.getElementById("atmp-code");
  const etat = document.getElementById("atmp-etat");
  // Repeindre n'efface jamais ce qu'on est en train de taper : la réponse
  // d'un premier enregistrement (le taux) arrivait pendant qu'on tapait le
  // code risque, et le vidait.
  const peindre = (t) => {
    if (!t) return;
    if (document.activeElement !== valeur) valeur.value = String(t.valeur).replace(".", ",");
    if (t.code_risque && document.activeElement !== code) code.value = t.code_risque;
  };
  // Ces deux cases ne sont pas la fiche : elles ne la marquent pas modifiée.
  [valeur, code].forEach((c) => { c.oninput = null; });
  try { peindre((await api("taux-atmp", { annee })).taux); } catch (e) { /* rien à montrer */ }
  const poser = async () => {
    if (!valeur.value.trim()) return;
    try {
      peindre((await api("taux-atmp", { annee, valeur: valeur.value,
                                        code_risque: code.value })).taux);
      etat.textContent = "Enregistré.";
      valeur.classList.remove("faute");
    } catch (e) {
      etat.textContent = e.message;
      valeur.classList.add("faute");
    }
  };
  valeur.onchange = poser;
  code.onchange = poser;
}

/* LA COMPLÉMENTAIRE SANTÉ D'UN PERMANENT (19/09/2026), en tete du bloc
 * « Salarié au mois ». Elle n'est pas un taux de barème : c'est un forfait
 * en pourcentage du PMSS, et il dépend de deux choses que seul le bulletin
 * d'adhésion du salarié dit. Trois cases plutôt qu'un JSON à taper ; le
 * champ `sante_svp` reste dans la fiche, en case cachée, et ces cases
 * l'écrivent (`brancherSante`). */
function SANTE_HTML() {
  return `<div class="encart" id="config-sante">
    <label>Complémentaire santé — le contrat souscrit par ce salarié
      <select id="config-sante-niveau">
        <option value="">— aucune (intermittent) —</option>
        ${(ETAT.niveaux_sante || []).map((n) =>
          `<option value="${echapper(n.niveau)}">niveau ${echapper(n.niveau)}
             — ${echapper(n.assure)} % du PMSS pour l'assuré seul</option>`).join("")}
      </select></label>
    <label class="coche"><input type="checkbox" id="config-sante-conjoint"> conjoint couvert</label>
    <label>Enfants couverts
      <input type="number" min="0" step="1" id="config-sante-enfants" value="0"></label>
    <div class="aide">Obligatoire pour un salarié au mois (art. L911-7) : sans
      elle, le logiciel refuse de le payer plutôt que de cotiser moins que le
      contrat. Elle se lit sur son bulletin d'adhésion.</div>
  </div>`;
}

/* LE HAUT D'UNE FICHE SALARIE (23/09/2026) : son nom en clair, et deux
 * gestes. « Exporter ses documents » mene a l'onglet Exports, le dossier de
 * ce salarie deja choisi ; une fiche neuve propose de la faire remplir par
 * la personne elle-meme, par courriel (`demanderFiche`). */
function enteteSalarie(fiche) {
  const d = fiche.donnees || {};
  const nom = `${d.prenom_affichage || d.prenom || ""} ${d.nom_affichage || d.nom || ""}`.trim();
  if (fiche.cle) {
    return `<div class="entete-fiche">
      <h2>${echapper(nom || fiche.cle)}
        ${d.code_salarie ? `<span class="discret">— matricule ${echapper(d.code_salarie)}</span>` : ""}</h2>
      <button type="button" class="secondaire petit" id="config-exporter"
        title="Bulletins, contrats, certificats de l'année : l'onglet Exports, ce salarié déjà choisi"
        >Exporter ses documents</button>
    </div>
    <div class="cout-annee" data-cote="salaries" data-cle="${echapper(fiche.cle)}"></div>
    <!-- SES IMPRÉVUS (15/10/2026) : ses arrêts et ses absences, passés et en
         cours, ce qui en est parti, et la porte « Un imprévu… ». js/imprevus.js. -->
    <div class="fiche-imprevus" id="fiche-imprevus"></div>
    <!-- CE QUE VOS CONTRATS FONT POUR SES DROITS (ticket 224) : les conseils sur
         l'historique, quand l'option « conseils » est cochée. js/conseils.js. -->
    <div class="fiche-conseils" id="fiche-conseils"></div>
    ${d.fiche_recue && fiche.anomalies.some((a) => a.gravite === "erreur")
      ? `<div class="encart">${echapper(d.fiche_recue)}</div>` : ""}`;
  }
  const lie = !!(ETAT.configuration || {}).boite_liee;
  return `<h2>Nouveau salarié</h2>
    <div class="encart invitation">
      <h3>La faire remplir par la personne</h3>
      <p class="aide">Elle reçoit un courriel avec un bouton : une réponse
        s'ouvre avec les questions (identité, naissance, numéro de sécurité
        sociale, adresse), elle y répond et l'envoie. Sa fiche se crée
        seule à l'arrivée de la réponse ; il vous restera l'emploi à
        choisir.</p>
      ${lie ? `<div class="champs">
        <div class="champ"><label>Son courriel
          <input type="email" id="invitation-courriel" placeholder="prenom.nom@exemple.fr"></label></div>
        <div class="champ"><label>Son prénom <span class="facultatif">facultatif</span>
          <input id="invitation-prenom" placeholder="pour dire bonjour"></label></div>
      </div>
      <div class="actions">
        <button type="button" class="primaire" id="invitation-envoyer">Envoyer la demande</button>
      </div>` : `<p class="note">Il faut d'abord lier la boîte courriel de la
        compagnie (Configuration › Mots de passe).</p>`}
    </div>
    <p class="note">Ou remplissez la fiche vous-même ci-dessous. Le matricule
      se donne à l'enregistrement.</p>`;
}


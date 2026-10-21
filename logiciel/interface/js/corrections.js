/* Corriger un mois depose, rouvrir un mois en retard. */
"use strict";

/* =======================================================================
   CORRIGER UN MOIS DÉPOSÉ, ET ROUVRIR UN MOIS EN RETARD   (18/09/2026)
   =======================================================================
   « Dans un endroit spécial, avec un gros avertissement si l'échéance est
   passée, sur les mois suivants. » D'où cet écran, dans l'Historique et non
   dans la saisie : ce qu'on écrit ici ne modifie AUCUNE déclaration partie.
   La correction est un objet à part ; c'est la DSN du mois courant qui
   l'emporte, datée du mois d'origine (cahier technique 2026.1 §2.2.2.5).

   Rien ne part de cet écran, donc rien n'y demande le mot de passe : écrire
   une correction n'engage pas l'employeur. Le dépôt de la DSN qui la portera,
   lui, le demandera comme tous les autres.

   La page ne décide de rien, une fois de plus : quel mois se corrige, ce que
   la correction change, ce qu'elle refuse — tout vient d'`operations`, et la
   ligne de commande (`python -m moteur corriger`) lit exactement les mêmes
   réponses. */
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
  cible.querySelectorAll("[data-corriger-mois]").forEach((b) => {
    b.onclick = () => boiteCorriger(b.dataset.corrigerMois);
  });
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

/** L'avertissement, en tête de chaque boîte de correction. C'est LA phrase
 *  de l'écran : elle dit que la déclaration partie ne bouge pas, et où la
 *  correction s'en va. Le mois porteur vient du moteur (`etat.mois_courant`),
 *  la page ne le calcule pas. */
function encartRegularisation(etat) {
  return `
    <div class="encart">
      <b>Ce mois est déposé, et son échéance est passée.</b>
      Ce que vous corrigez ici ne modifie pas la DSN déposée : la correction
      partira dans la DSN ${echapper(deMois(etat.mois_courant))},
      datée ${echapper(deMois(etat.mois))}. L'« avant » est la paie telle
      qu'elle a été déclarée.
    </div>`;
}

/** Le mois d'origine : ses engagements déclarés, tels que la dernière
 *  correction portée les a laissés. On en choisit un. */
async function boiteCorriger(mois) {
  let r;
  try { r = await api("regularisation-mois", { mois }); }
  catch (e) { dire(e.message, "erreur"); return; }
  if (!r.etat.regularisable) { dire(r.etat.raison); return; }
  // L'etat qu'on promene porte aussi ce que la boite doit savoir demander :
  // un poste sans mot de passe pose (un bac d'essai) ne se voit pas offrir
  // un popup qui ne prouverait rien.
  const etat = { ...r.etat, mot_de_passe_pose: r.mot_de_passe_pose };
  ouvrirBoite(`
    <h3>Corriger ${echapper(moisEnClair(mois))}</h3>
    ${encartRegularisation(etat)}
    ${r.engagements.length ? r.engagements.map((e, i) => `
      <div class="rang">
        <span class="rang-nom">${echapper(e.nom)}</span>
        <span class="rang-infos">${e.ajoute
            ? `<b>contrat ajouté après la DSN</b>${e.numero ? ` (${echapper(e.numero)})` : ""} · ${echapper(e.spectacle)} ·
               ${echapper(periodeEnClair(e.debut, e.fin))} · rien de déclaré : la correction le déclare en entier`
            : `contrat ${echapper(e.numero)} · ${echapper(e.spectacle)} ·
          ${echapper(periodeEnClair(e.debut, e.fin))} ·
          brut ${echapper(euros(e.brut))}, net ${echapper(euros(e.net_a_payer))}`}
          ${e.deja_corrige ? " · déjà corrigé par une DSN partie" : ""}
          ${e.engagement_saisi && !e.ajoute && !e.correction_en_cours ? " · <b>modifié dans la saisie : la correction en reprend les lignes</b>" : ""}
          ${e.correction_en_cours ? ` · <b>correction n° ${e.correction_en_cours.numero} en attente</b>` : ""}</span>
        <button class="secondaire" data-eng="${i}">Corriger…</button>
      </div>`).join("")
      : `<p class="discret">Aucun engagement dans la saisie de ce mois.</p>`}
    <p class="note">Un contrat oublié se saisit dans son mois, avec sa DPAE :
      il apparaît alors ici, et sa correction le déclare dans la DSN suivante.</p>
    <div class="actions">
      <button class="primaire" onclick="fermerBoite()">Fermer</button>
    </div>`);
  document.getElementById("boite").querySelectorAll("[data-eng]").forEach((b) => {
    b.onclick = () => boiteCorrigerEngagement(mois, r.engagements[+b.dataset.eng], etat);
  });
}

/** Une correction : les lignes de paie de l'engagement déclaré, telles
 *  qu'elles ont été déclarées, qu'on modifie. Le contrat ne bouge pas —
 *  numéro, bornes, spectacle identifient la déclaration d'origine, et le
 *  moteur refuse une correction qui les déplacerait.
 *
 *  Les lignes sont celles de la saisie (`ligneRubrique`), calendrier compris :
 *  un cachet se coche dans le mois d'ORIGINE, d'où le mois passé à
 *  `ouvrirCalendrier`. L'aperçu vient du moteur à chaque frappe, comme
 *  l'aperçu d'un mois — la page ne calcule aucune paie. */
function boiteCorrigerEngagement(mois, e, etat) {
  const tauxAvant = DERNIER_TAUX;     // taper un taux ici ne décide pas du prochain mois
  const rendre = () => { DERNIER_TAUX = tauxAvant; };
  ouvrirBoite(`
    <h3>${e.ajoute ? "Déclarer le contrat ajouté" : `Corriger le contrat ${echapper(e.numero)}`}</h3>
    ${encartRegularisation(etat)}
    <p class="discret">${echapper(e.nom)} — ${echapper(e.spectacle)},
      ${echapper(periodeEnClair(e.debut, e.fin))}.
      Le contrat garde son numéro et ses dates : seules ses lignes se corrigent.</p>
    <div id="cor-lignes"></div>
    <button class="secondaire" id="cor-ajouter">+ une ligne</button>
    <label>Ce qui était faux
      <input id="cor-motif" placeholder="un cachet oublié, un taux erroné…"
             value="${echapper((e.correction_en_cours || {}).motif || "")}">
    </label>
    <div id="cor-apercu" class="note">L'aperçu arrive…</div>
    <p class="note">Écrire demande le mot de passe du logiciel : rien ne part
      maintenant, mais cette correction repartira d'elle-même dans la
      prochaine DSN.</p>
    <div class="actions">
      <button class="secondaire" id="cor-retour">Retour</button>
      <button class="primaire" id="cor-ecrire" disabled>Écrire la correction…</button>
    </div>`, rendre);

  const zone = document.getElementById("cor-lignes");
  const vue = document.getElementById("cor-apercu");
  const bouton = document.getElementById("cor-ecrire");
  let minuteur = null, dernier = null;

  const poser = (l) => {
    const div = ligneRubrique(l);
    // Le calendrier du mois d'ORIGINE, et non celui de l'écran de paie.
    div.querySelector(".r-jours-btn").onclick = (ev) => { ev.preventDefault(); ouvrirCalendrier(div, mois); };
    // `new Event("input")` ne remonte pas : on s'accroche aux deux endroits
    // que le calendrier et la suppression touchent directement.
    const jours = div.querySelector(".r-jours");
    const avant = jours.oninput;
    jours.oninput = (ev) => { if (avant) avant(ev); plusTard(); };
    const sup = div.querySelector(".supprimer");
    const avantSup = sup.onclick;
    sup.onclick = (ev) => { if (avantSup) avantSup(ev); plusTard(); };
    zone.appendChild(div);
  };
  const plusTard = () => { clearTimeout(minuteur); minuteur = setTimeout(calculer, 350); };

  const calculer = async () => {
    const lignes = lignesDuBloc(zone, mois);
    let a;
    try {
      a = await api("regularisation-apercu", {
        mois, salarie: e.salarie, debut: e.debut, engagement: { lignes },
      });
    } catch (err) {
      dernier = null; bouton.disabled = true;
      vue.innerHTML = `<div class="encart attention">${echapper(err.message)}</div>`;
      return;
    }
    dernier = a;
    vue.innerHTML = vueApercuCorrection(a);
    majBoutonCorrection();
  };
  const majBoutonCorrection = () => {
    const motif = document.getElementById("cor-motif").value.trim();
    bouton.disabled = !dernier || dernier.rien_ne_change || !motif;
  };

  // LA PAIE MODIFIÉE DANS LA SAISIE APRÈS LA LIMITE (21/10/2026) : la correction part de ce qu'on a saisi
  // (`engagement_saisi`, donné par le moteur) ; l'aperçu compare toujours à ce qui est déclaré.
  const depart = (e.engagement_saisi || e.engagement).lignes || [];
  depart.forEach(poser);
  if (!depart.length) poser({});
  zone.addEventListener("input", plusTard);
  zone.addEventListener("change", plusTard);
  document.getElementById("cor-motif").oninput = majBoutonCorrection;
  document.getElementById("cor-ajouter").onclick = (ev) => { ev.preventDefault(); poser({}); plusTard(); };
  document.getElementById("cor-retour").onclick = () => { rendre(); boiteCorriger(mois); };
  bouton.onclick = async () => {
    // LE POPUP, ET PAS UN CLIC SEC (18/09/2026). « Je m'attendais pas à ce
    // que ça clique facilement, je m'attendais au mot de passe, c'est quand
    // même une action risquée. » Ecrire une correction n'envoie rien, mais
    // arme la prochaine DSN sur un mois deja declare : c'est un acte, et
    // tout acte passe par le meme popup. Le moteur l'exige de toute facon.
    const preuve = etat.mot_de_passe_pose === false
      ? null : await demanderMotDePasse("corriger " + moisEnClair(mois) + ", déjà déclaré");
    if (etat.mot_de_passe_pose !== false && !preuve) return;
    bouton.disabled = true;
    try {
      const r = await api("regularisation-ecrire", {
        mois, salarie: e.salarie, debut: e.debut,
        engagement: { lignes: lignesDuBloc(zone, mois) },
        motif: document.getElementById("cor-motif").value,
        motdepasse: preuve,
      });
      rendre();
      fermerBoite();
      dire("Correction n° " + r.correction.numero + " écrite — elle partira dans la DSN "
           + deMois(etat.mois_courant) + ".");
      await dessinerRegularisation();
      // L'étape DSN du mois affiché le dit aussitôt (`correction_ecrite`).
      if (MOIS_AFFICHE === mois) await rafraichirActions();
    } catch (err) {
      bouton.disabled = false;
      dire(err.message, "erreur");
    }
  };
  calculer();
}

function vueApercuCorrection(a) {
  const ligne = (titre, cle) => `
    <tr><td>${titre}</td><td class="r">${echapper(a.avant[cle])}</td>
        <td class="r">${echapper(a.apres[cle])}</td>
        <td class="r">${a.delta[cle] === undefined ? "" : echapper(a.delta[cle])}</td></tr>`;
  return `
    <table class="compta">
      <tr><th></th><th>avant</th><th>après</th><th>différence</th></tr>
      ${ligne("cachets", "cachets")}
      ${ligne("heures", "heures")}
      ${ligne("brut", "brut")}
      ${ligne("cot. salariales", "cotisations_salariales")}
      ${ligne("cot. patronales", "cotisations_patronales")}
      ${ligne("net imposable", "net_imposable")}
      ${ligne("net à payer", "net_a_payer")}
      ${ligne("coût employeur", "cout_employeur")}
    </table>
    ${(a.avertissements || []).map((x) => `<div class="encart attention">${echapper(x)}</div>`).join("")}
    ${a.rien_ne_change
      ? `<p class="note">Rien ne change : il n'y a pas de correction à écrire.</p>`
      : `<p class="note">Cette différence partira dans la DSN
           ${echapper(deMois(a.etat.mois_courant))}, datée
           ${echapper(deMois(a.mois))} — l'ancienne paie en négatif, la
           nouvelle en positif.</p>`}`;
}

/** UN MOIS FERMÉ QUI SE ROUVRE. Passé le 15 du mois suivant, le mois est en
 *  lecture seule : sa DSN était due. Mais une DSN oubliée doit partir — en
 *  retard, et en le sachant. Le moteur exige alors qu'on le redise
 *  (`forcer_delai`) ; ici, c'est cette boîte qui le fait dire. Elle ne
 *  rouvre que le mois affiché, et seulement jusqu'au prochain chargement. */
function boiteRouvrirMois(mois, limite) {
  ouvrirBoite(`
    <h3>Rouvrir ${echapper(moisEnClair(mois))}</h3>
    <div class="encart attention">
      <b>La DSN ${echapper(deMois(mois))} était due le ${echapper(jjmmaaaa(limite))}.</b>
      La déposer maintenant reste ce qu'il faut faire — une DSN en retard vaut
      mieux qu'une DSN absente — mais le retard est connu des organismes et
      peut être pénalisé.
    </div>
    <p class="note">Rouvrir n'envoie rien : le mois redevient modifiable, et il
      se produit puis se dépose comme d'habitude. Si sa DSN est déjà partie et
      qu'il faut en corriger le contenu, ce n'est pas ici : c'est
      Historique › Corriger un mois déposé.</p>
    <div class="actions">
      <button class="secondaire" onclick="fermerBoite()">Laisser fermé</button>
      <button class="primaire" id="rm-ok">Rouvrir ${echapper(moisEnClair(mois))}</button>
    </div>`);
  document.getElementById("rm-ok").onclick = () => {
    MOIS_FORCE = mois;
    fermerBoite();
    if (MOIS_AFFICHE === mois) majFermeture(mois, true);
    dire("Mois rouvert — ce qui part d'ici part en retard.");
  };
}


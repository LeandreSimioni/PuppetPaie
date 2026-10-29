/* Le formulaire d'une fiche : brancher, lire, verifier, enregistrer, dater. */
"use strict";

/* Ce que vaut la ligne vide d'une liste de choix, quand elle vaut quelque
 * chose : le moteur prend alors le cas ordinaire. */
const ORDINAIRE_VIDE = {
  statut_conventionnel: "vide : profession intermédiaire",
  statut_categoriel_rc: "vide : non cadre",
  plafond_temps_partiel: "vide : proratisé à la durée du contrat, la règle",
};

function dessinerFormulaire(fiche) {
  if (!fiche) return;
  FICHE_COURANTE = fiche;
  const desc = ETAT.configuration.description[FICHE_ACTIVE];
  // CE QUE LA CASE « Emploie des permanents » OUVRE (22/09/2026). Les
  // champs qui ne servent qu'aux permanents restent dans le formulaire —
  // avec leur valeur, qui repart à l'enregistrement — mais ne se montrent
  // qu'à qui a dit en employer. Une case affichée est une case à remplir, à
  // relire et à ne pas se tromper : celle de la complémentaire santé d'un
  // cachetier n'a jamais rien à dire.
  const avecPermanents = !!(ETAT.configuration || {}).permanents;
  // Une compagnie à un seul établissement ne voit pas la case « Établissement »
  // du salarié (25/09/2026) : la question ne se pose pas.
  const avecEtablissements = !!(ETAT.configuration || {}).etablissements;
  // Une case obligatoire encore vide n'est pas une faute tant qu'on n'a pas
  // essayé d'enregistrer : l'astérisque le dit (`aRemplir`).
  const anomalie = (cle) => fiche.anomalies.find((a) => a.champ === cle && !aRemplir(a));

  // Les champs CALCULES ne sont pas proposes a la saisie : c'est le
  // programme qui les fabrique. ET ILS NE SE MONTRENT PLUS (23/09/2026) :
  // `COMPAGNIE DU FIL` sous « Compagnie du Fil », c'etait faire lire a
  // l'utilisateur la forme de la DSN. L'editeur : « ça, les gens s'en
  // fichent ». Le moteur la fabrique, et ne revient vers l'humain que si
  // elle ne peut pas partir.

  // CE QU'ON N'AFFICHE PAS, ON LE GARDE. Le signataire porte un `nom_aem` et
  // une note ; la banque porte un avertissement de diffusion. Ces clefs ne
  // sont pas des cases a remplir, mais les effacer serait perdre une valeur
  // que quelqu'un a posee exprès. `lireFormulaire` repart de ces originaux.
  STRUCTURES_ORIGINE = {};
  desc.forEach((c) => {
    const v = fiche.donnees[c.cle];
    if (SOUS_CHAMPS[c.cle] && v && typeof v === "object") STRUCTURES_ORIGINE[c.cle] = v;
  });

  const caches = CACHES[FICHE_ACTIVE] || [];
  // UN CHAMP QUI DÉPEND D'UN AUTRE (`si`, 25/09/2026) se replie quel que soit
  // le chemin qui l'a dessiné — liste de choix, convention, champ simple :
  // la marque se pose ici, sur ce que chacun rend, et `brancherConditions`
  // la fait suivre à l'écran.
  // UN CHAMP QUI DÉPEND DE L'ENTREPRISE (`si_entreprise`, 26/09/2026) : la
  // convention décide, et elle ne change pas pendant qu'on remplit la fiche
  // d'un salarié. Le champ est posé, replié — il garde sa valeur.
  const entreprise = ((ETAT.configuration || {}).entreprise || {}).donnees || {};
  // TOUTES LES CONDITIONS AU MÊME ENDROIT (26/09/2026) : les chemins de
  // dessin (liste de choix, case, structure, champ simple) ne posaient pas
  // tous la marque des permanents — « Jours fériés chômés » se montrait à
  // une compagnie sans salarié au mois. Elle se pose ici, sur ce que chacun
  // rend, avec celles des établissements, de l'entreprise et du `si`.
  const champHtml = (c) => {
    const html = champHtmlBrut(c);
    if (!html) return html;
    const horsEntreprise = c.si_entreprise && FICHE_ACTIVE !== "entreprise"
      && !conditionRemplie(c.si_entreprise, (k) => entreprise[k]);
    if (!c.si && !horsEntreprise && !c.si_permanents && !c.si_etablissements) return html;
    const ok = !horsEntreprise && conditionRemplie(c.si, (k) => fiche.donnees[k])
      && !(c.si_permanents && !avecPermanents)
      && !(c.si_etablissements && !avecEtablissements);
    return html.replace(/^(\s*<div class="champ)([^"]*)"([^>]*)>/, (m, debut, classes, reste) => {
      const cl = classes + (c.si_permanents && !/champ-permanents/.test(classes)
        ? " champ-permanents" : "");
      return `${debut}${cl}"${reste.replace(/\s+hidden\b/g, "")}${
        c.si ? ` data-si="${echapper(JSON.stringify(c.si))}"` : ""}${
        horsEntreprise ? " data-hors-entreprise" : ""}${ok ? "" : " hidden"}>`;
    });
  };
  const champHtmlBrut = (c) => {
    const v = fiche.donnees[c.cle];
    const a = anomalie(c.cle);
    // L'histoire de la fiche se LIT, elle ne se tape pas : elle dit ce qui
    // est reellement parti aux organismes, et on ne reecrit pas une
    // declaration deposee. La montrer en JSON editable inviterait a la
    // corriger, ce qui est exactement ce qu'il ne faut pas.
    if (c.forme === "histoire") return histoireDuChamp(v, desc, fiche.donnees);
    // Ce que la machine deduit : present, mais pas a l'ecran.
    if (caches.includes(c.cle)) {
      // Une structure cachee repart telle quelle : en JSON, relu par
      // `lireFormulaire` comme une structure.
      if (c.forme === "structure" || (v !== null && typeof v === "object")) {
        return `<input type="hidden" data-cle="${c.cle}" data-structure="1"
                  value="${echapper(JSON.stringify(v ?? null))}">`;
      }
      return `<input type="hidden" data-cle="${c.cle}" data-forme="${c.forme}"
                value="${echapper(v ?? "")}">`;
    }
    const sous = SOUS_CHAMPS[c.cle];
    if (sous) return structureHtml(c, v, sous, a, avecPermanents);
    // LA CONVENTION SE CHOISIT, ELLE NE SE TAPE PAS (22/09/2026). Le 3090
    // etait demande trois fois : l'IDCC, son libelle, et le code convention
    // DSN -- la meme information sous trois noms. Un seul choix les remplit
    // tous les trois (`brancherConvention`). La liste vient du moteur
    // (`web._conventions`, section `par_convention` du socle) : l'ecran
    // n'invente ni un numero ni un libelle, et il DIT quand une convention
    // existe sans que ses taux soient releves -- le calcul la refuse.
    // LE CODE URSSAF SE CHOISIT (22/09/2026). Trois chiffres a taper, dont
    // personne ne sait ou ils se trouvent, et qu'un rejet de DPAE signale
    // des jours plus tard. La liste est celle de la Table des Urssaf, relevee
    // et datee (`donnees/urssaf-dpae-base.json`). Elle ne dit PAS quel organisme couvre
    // quel territoire : l'ecran ne le devine donc pas depuis le code postal,
    // il propose, et l'aide dit ou lire le code.
    // LA SIGNATURE DES COURRIELS SE COLLE (08/10/2026) : un cadre qui garde la
    // mise en forme copiee depuis une messagerie, son poids sous le cadre. Le
    // HTML part dans un champ cache ; le moteur le nettoie encore a l'envoi.
    if (c.cle === "courriel_signature") {
      const html = nettoyerHtmlColle(String(v ?? ""));
      return `<div class="champ pleine">
        <label>${echapper(c.libelle)}
          ${aideHtml(c.cle, c.aide)}</label>
        <div id="signature-cadre" contenteditable="true"
          style="min-height:4em;border:1px solid #ccc;border-radius:4px;padding:8px;background:#fff"
          >${html}</div>
        <textarea data-cle="${c.cle}" data-forme="${c.forme}" hidden
          >${echapper(html)}</textarea>
        <div style="display:flex;gap:12px;align-items:center;margin-top:6px">
          <span id="signature-poids" class="discret"></span>
          <button type="button" class="secondaire" id="signature-vider"
            >Revenir à la signature habituelle</button>
        </div>
        ${aideCorps(c.cle, c.aide)}
        ${a ? `<div class="anomalie ${a.gravite}">${echapper(a.message)}</div>` : ""}
      </div>`;
    }
    if (c.cle === "code_urssaf" && (ETAT.organismes_urssaf || []).length) {
      const org = ETAT.organismes_urssaf;
      const v3 = String(v ?? "");
      const connu = org.some((o) => o.code === v3);
      return `<div class="champ pleine">
        <label>${echapper(c.libelle)}
          ${aideHtml(c.cle, c.aide)}
          <select data-cle="${c.cle}" data-forme="chiffres">
            <option value=""${v3 ? "" : " selected"}>— celle de votre code postal (${
              echapper(ETAT.urssaf_deduit || "")}) —</option>
            ${org.map((o) => `<option value="${echapper(o.code)}"${
                o.code === v3 ? " selected" : ""}
              >${echapper(o.code)} — ${echapper(o.nom)}</option>`).join("")}
            ${connu || !v3 ? "" : `<option value="${echapper(v3)}" selected
              >${echapper(v3)} — inconnu de la table : la DPAE serait rejetée</option>`}
          </select></label>
        ${aideCorps(c.cle, c.aide)}
        ${a ? `<div class="anomalie ${a.gravite}">${echapper(a.message)}</div>` : ""}
      </div>`;
    }
    if (c.cle === "idcc" && (ETAT.conventions || []).length) {
      const cv = ETAT.conventions;
      const connue = cv.some((x) => String(x.idcc) === String(v ?? ""));
      return `<div class="champ pleine">
        <label>Convention collective
          ${c.obligatoire ? '<span class="obligatoire">*</span>' : ""}
          ${aideHtml(c.cle, c.aide)}
          <select data-cle="${c.cle}" data-forme="chiffres">
            ${cv.map((x) => `<option value="${echapper(x.idcc)}"${
                String(x.idcc) === String(v ?? "") ? " selected" : ""}
              >IDCC ${echapper(x.idcc)} — ${echapper(x.libelle)}${
                x.applicable ? "" : " (taux non relevés : le calcul refuse)"}</option>`).join("")}
            ${connue ? "" : `<option value="${echapper(v ?? "")}" selected
              >IDCC ${echapper(v ?? "(vide)")} — non servie par le barème</option>`}
          </select></label>
        ${aideCorps(c.cle, c.aide)}
        ${a ? `<div class="anomalie ${a.gravite}">${echapper(a.message)}</div>` : ""}
      </div>`;
    }
    // UN PAYS SE CHOISIT DANS LA LISTE (10/10/2026, vague 1k) : on tapait un
    // code ISO (« IT », « MA »). La liste vient du moteur (`etat.pays`, le Code
    // officiel geographique de l'INSEE), triee par nom ; une lettre tapee sur
    // la liste ouverte va au premier pays qui commence par elle. Un code
    // deja saisi que la liste ne connait pas reste affiche, et le moteur le
    // refuse en le disant.
    // Le pays de naissance a sa liste a lui (lot R2, 11/10/2026) : la table
    // « Pays » de la DSN que le moteur sert (`etat.pays_naissance`), le Kosovo
    // et les Etats disparus compris.
    const listePays = (c.cle === "pays_naissance" && (ETAT.pays_naissance || []).length)
      ? ETAT.pays_naissance : ETAT.pays;
    if (c.forme === "pays" && (listePays || []).length) {
      const liste = listePays;
      const v2 = String(v ?? "").trim();
      const connu = liste.some((p) => p.code === v2.toUpperCase());
      return `<div class="champ">
        <label>${echapper(c.libelle)}${facultatif(c)}
          ${c.obligatoire ? '<span class="obligatoire">*</span>' : ""}
          ${aideHtml(c.cle, c.aide)}
          <select data-cle="${c.cle}" data-forme="pays"
              class="${a ? (a.gravite === "erreur" ? "faute" : "suspect") : ""}">
            <option value=""${v2 ? "" : " selected"}>— à choisir —</option>
            ${liste.map((p) => `<option value="${echapper(p.code)}"${
                p.code === v2.toUpperCase() ? " selected" : ""}>${echapper(p.nom)}</option>`).join("")}
            ${connu || !v2 ? "" : `<option value="${echapper(v2)}" selected
              >${echapper(v2)} — inconnu de la liste : choisir le pays</option>`}
          </select></label>
        ${aideCorps(c.cle, c.aide)}
        ${a ? `<div class="anomalie ${a.gravite}">${echapper(a.message)}</div>` : ""}
      </div>`;
    }
    // UNE LISTE DE CHOIX SE CHOISIT (23/09/2026) : l'annexe s'ecrivait « 4 »
    // a la main, le CSE « cse_adherent_fnas ».
    // La fiche salarie a suivi le meme jour : le sexe s'ecrivait « 2 », la
    // nationalite « 01 », le statut cadre « 04 ».
    if (["entreprise", "salarie"].includes(FICHE_ACTIVE) && c.forme === "choix"
        && (c.choix || []).length) {
      const lib = LIBELLES_CHOIX[c.cle] || {};
      const v1 = String(v ?? "");
      // L'ordre de la table des libelles (le cas ordinaire d'abord), puis ce
      // qu'elle ne nomme pas ; un choix vide prevu par le moteur (« aucun »)
      // tient lieu de premiere ligne.
      const ordre = [...(c.choix.includes("") ? [""] : []),
        ...Object.keys(lib).filter((x) => x && c.choix.includes(x)),
        ...c.choix.filter((x) => x && !(x in lib))];
      return `<div class="champ${FICHE_ACTIVE === "entreprise" ? " pleine" : ""}">
        <label>${echapper(c.libelle)}${facultatif(c)}
          ${c.obligatoire ? '<span class="obligatoire">*</span>' : ""}
          ${aideHtml(c.cle, c.aide)}
          <select data-cle="${c.cle}" data-forme="choix">
            ${c.choix.includes("") ? "" : `<option value=""${v1 ? "" : " selected"}>— ${
              c.cle in ORDINAIRE_VIDE ? echapper(ORDINAIRE_VIDE[c.cle]) : "à choisir"} —</option>`}
            ${ordre.map((x) => `<option value="${echapper(x)}"${
                x === v1 ? " selected" : ""}>${echapper(lib[x] || x)}</option>`).join("")}
            ${!v1 || c.choix.includes(v1) ? "" : `<option value="${echapper(v1)}" selected
              >${echapper(v1)}</option>`}
          </select></label>
        ${aideCorps(c.cle, c.aide)}
        ${a ? `<div class="anomalie ${a.gravite}">${echapper(a.message)}</div>` : ""}
      </div>`;
    }
    // LES AUTRES NOMS D'UN SPECTACLE (23/09/2026) : une liste de mots, qui
    // s'affichait en JSON, crochets et guillemets compris. Une ligne, les
    // noms separes par des virgules ; `lireFormulaire` refait la liste.
    if (c.cle === "alias" && FICHE_ACTIVE === "spectacle"
        && (v == null || (Array.isArray(v) && v.every((x) => typeof x === "string")))) {
      return `<div class="champ pleine">
        <label>${echapper(c.libelle)}${facultatif(c)} ${aideHtml(c.cle, c.aide)}
          <input data-cle="${c.cle}" data-liste-textes="1" value="${echapper((v || []).join(", "))}"
                 placeholder="ex. zerbinette, aaah">
        </label>
        ${aideCorps(c.cle, c.aide)}
        ${a ? `<div class="anomalie ${a.gravite}">${echapper(a.message)}</div>` : ""}
      </div>`;
    }
    // UNE CASE A COCHER SE LIT COMME UNE PHRASE : la case, puis ce qu'elle dit.
    if (c.forme === "booleen") {
      const coche = v == null ? c.booleen_defaut !== false : v !== false;
      return `<div class="champ pleine case${c.si_permanents ? " champ-permanents" : ""}"${
          c.si_permanents && !avecPermanents ? " hidden" : ""}>
        <label class="coche"><input type="checkbox" data-cle="${c.cle}"
            data-forme="booleen"${coche ? " checked" : ""}>
          ${echapper(c.libelle)} ${aideHtml(c.cle, c.aide)}</label>
        ${aideCorps(c.cle, c.aide)}
        ${a ? `<div class="anomalie ${a.gravite}">${echapper(a.message)}</div>` : ""}
      </div>`;
    }
    const structure = c.forme === "structure" || (v !== null && typeof v === "object");
    // Un booleen se coche. Ecrit en toutes lettres, il partait en texte : et
    // « false » est une chaine VRAIE, donc la fiche serait restee active en
    // paraissant archivee.
    // Un booléen ABSENT ne vaut pas la même chose partout : `actif` absent
    // vaut vrai (aucune fiche existante n'a à être modifiée pour rester
    // active), `emploie_des_permanents` absent vaut faux. C'est le champ qui
    // le dit — la page cochait d'office, et écrivait donc l'inverse de ce que
    // le moteur lisait.
    const controle = c.forme === "booleen"
      ? `<input type="checkbox" data-cle="${c.cle}" data-forme="booleen"
            ${(v == null ? c.booleen_defaut !== false : v !== false) ? "checked" : ""}>`
      : structure
      ? `<textarea rows="4" data-cle="${c.cle}" data-structure="1">${echapper(JSON.stringify(v ?? null, null, 1))}</textarea>`
      : `<input data-cle="${c.cle}" data-forme="${c.forme}" value="${echapper(v ?? "")}"${
            // Une date se choisit dans un calendrier (23/09/2026 pour la fiche
            // salarié ; #236, 23/10/2026, pour toutes : « Date de la première
            // embauche » était un texte qui refusait 16/11/2026) ; la valeur reste
            // AAAA-MM-JJ, celle du moteur.
            c.forme === "date"
              && /^(\d{4}-\d{2}-\d{2})?$/.test(String(v ?? "")) ? ' type="date"' : ""}${
            c.exemple ? ` placeholder="ex. ${echapper(c.exemple)}"` : ""}
            class="${a ? (a.gravite === "erreur" ? "faute" : "suspect") : ""}">`;
    // L'adresse et le nom prennent la ligne entiere : le code postal et la
    // commune tiennent ensuite cote a cote, comme sur une enveloppe.
    const large = structure || (FICHE_ACTIVE === "entreprise"
      && ["adresse_affichage", "nom_affichage"].includes(c.cle))
      || (FICHE_ACTIVE === "salarie" && ["adresse_affichage", "emploi_affichage"].includes(c.cle));
    return `<div class="champ ${large ? "pleine" : ""}${
        c.si_permanents ? " champ-permanents" : ""}"${
        (c.si_permanents && !avecPermanents)
          || (c.si_etablissements && !avecEtablissements) ? " hidden" : ""}>
      <label>${echapper(c.libelle)}${facultatif(c)}
        ${c.obligatoire ? '<span class="obligatoire">*</span>' : ""}
        ${aideHtml(c.cle, c.aide)}
        ${controle}
      </label>
      ${aideCorps(c.cle, c.aide)}
      ${a ? `<div class="anomalie ${a.gravite}">${echapper(a.message)}</div>` : ""}
    </div>`;
  };

  // LES SECTIONS. Une fiche qui n'en declare pas garde sa liste plate ;
  // un champ oublie d'une table de sections s'affiche dans la derniere.
  // `ecran` faux : le champ garde sa valeur, la fiche ne le montre pas (§94).
  const saisissables = desc.filter((c) => !c.calcule && c.ecran !== false);
  const sections = SECTIONS_FICHE[FICHE_ACTIVE];
  let champs;
  if (!sections) {
    champs = saisissables.map(champHtml).join("");
  } else {
    const parCle = {};
    saisissables.forEach((c) => { parCle[c.cle] = c; });
    const places = new Set();
    sections.forEach(([, cles]) => cles.forEach((k) => places.add(k)));
    const reste = saisissables.filter((c) => !places.has(c.cle));
    // Un champ oublie des tables va dans la derniere section TITREE : la
    // derniere tout court peut etre l'historique, qui n'a pas de titre.
    const derniereTitree = sections.map(([t]) => !!t).lastIndexOf(true);
    champs = sections.map(([titre, cles, options], i) => {
      let dedans = cles.map((k) => parCle[k]).filter(Boolean);
      if (i === derniereTitree) dedans = dedans.concat(reste);
      if (!dedans.length) return "";
      // Une section vide ne s'affiche pas : « Ce qui a changé » n'a de titre
      // que le jour ou une valeur declaree a change (`histoireDuChamp` ne
      // rend rien avant).
      const dedansHtml = dedans.map(champHtml).join("");
      if (!dedansHtml.trim()) return "";
      // La section des permanents suit la case : rien a remplir tant que la
      // compagnie n'a pas dit en employer.
      const toute = dedans.every((c) => c.si_permanents);
      if ((options || {}).avance) {
        // Ce qui vaut le cas ordinaire n'ouvre pas le tiroir : l'Urssaf du
        // code postal, Audiens.
        // code postal, Audiens ; pour un salarie, le matricule et le reste
        // de ce qui est toujours rempli (ORDINAIRE).
        const ecart = dedans.some((c) => {
          const v = fiche.donnees[c.cle];
          return !caches.includes(c.cle) && v !== null && v !== undefined
            && v !== "" && v !== false && !(ORDINAIRE[c.cle] || (() => false))(v);
        });
        return `<details class="bloc avance"${ecart ? " open" : ""}>
          <summary>${echapper(titre)}</summary>
          <div class="champs">${dedansHtml}</div>
        </details>`;
      }
      // Le bloc de l'emploi s'ouvre sur la liste des metiers, celui du
      // salarie au mois sur sa complementaire sante : chacun a sa place,
      // et non plus en tete de fiche.
      const opt = options || {};
      const entete = opt.metier ? METIER_HTML(fiche) : opt.sante ? SANTE_HTML()
        : opt.atmp ? ATMP_HTML() : "";
      const permanents = toute || opt.sante;
      // LE BLOC DU SALARIÉ AU MOIS NE S'OUVRE QU'À LUI (26/09/2026). Dans une
      // compagnie qui emploie aussi des permanents, la fiche d'une comédienne
      // au cachet déroulait sa complémentaire santé et sa prévoyance. Replié
      // pour qui n'a jamais eu de contrat au mois — sauf s'il y a déjà une
      // valeur : rien de saisi ne devient invisible.
      if (opt.sante && avecPermanents && FICHE_ACTIVE === "salarie") {
        // Au mois : le moteur le dit (`salaries_au_mois`) — ou la saisie en cours
        // l'engage déjà au mois, pas encore enregistrée parce que sa santé manque
        // (#236 : le refus envoyait sur un tiroir replié).
        let enCours = false;
        try {
          enCours = (saisieCourante().engagements || []).some((x) => x.salarie === fiche.cle
            && natureDe(x.nature).mensuelle);
        } catch (e) { enCours = false; }
        const auMois = ((ETAT.configuration || {}).salaries_au_mois || []).includes(fiche.cle) || enCours;
        const rempli = dedans.some((c) => {
          const v = fiche.donnees[c.cle];
          return v !== null && v !== undefined && v !== "";
        }) || !!fiche.donnees.sante_svp;
        return `<details class="bloc avance" data-bloc-permanents="1"${auMois || rempli ? " open" : ""}>
          <summary>${echapper(titre)}${auMois ? "" : ` <span class="discret">— s'il est payé au mois</span>`}</summary>
          ${entete}
          <div class="champs">${dedansHtml}</div>
        </details>`;
      }
      return `<section class="bloc"${permanents && !avecPermanents ? ' hidden' : ''}${
          permanents ? ' data-bloc-permanents="1"' : ''}>
        ${titre ? `<h3>${echapper(titre)}</h3>` : ""}
        ${entete}
        <div class="champs">${dedansHtml}</div>
      </section>`;
    }).join("");
  }

  const err = fiche.anomalies.filter((a) => a.gravite === "erreur" && !aRemplir(a)).length;
  document.getElementById("config-formulaire").innerHTML = `
    <div class="carte" style="margin-top:16px">
      ${FICHE_ACTIVE === "salarie" ? enteteSalarie(fiche)
        : FICHE_ACTIVE === "entreprise" ? "" : `<h2>${fiche.cle
          // Le titre, pas la cle : `aaah-zerbinette` est un identifiant interne.
          ? `${echapper(String(fiche.donnees.titre || "").toUpperCase() === "A COMPLETER"
              ? "Spectacle à nommer" : fiche.donnees.titre || fiche.cle)}
             <span class="discret">— ${echapper(fiche.donnees.numero_objet || fiche.cle)}</span>`
          : "Nouveau spectacle"}</h2>
        ${fiche.cle && fiche.donnees.titre ? `<div class="cout-annee" data-cote="spectacles"
          data-cle="${echapper(fiche.donnees.titre)}"></div>` : ""}`}
      ${!fiche.cle && FICHE_ACTIVE === "spectacle" ? `<p class="note">La clé de la
        fiche se fabrique à l'enregistrement.</p>` : ""}
      ${err ? `<div class="encart erreur"><b>${err > 1 ? `${err} champs à corriger` : "Un champ à corriger"}</b>
        — marqué${err > 1 ? "s" : ""} ci-dessous.</div>` : ""}
      ${sections ? champs : `<div class="champs">${champs}</div>`}
      <div class="actions">
        <!-- ANNULER (ticket #124, 15/10/2026) : « quand je crée un salarié je
             peux pas annuler, j'ai juste enregistrer ». Voir annulerFiche. -->
        <button class="secondaire" id="config-annuler"${FICHE_ACTIVE === "entreprise" ? " hidden" : ""}
          title="${FICHE_ACTIVE === "entreprise" ? "Reprendre ce qui est enregistré"
            : fiche.cle ? "Laisser la fiche telle qu'elle est enregistrée et revenir à la liste"
            : "Ne pas créer cette fiche et revenir à la liste"}">Annuler</button>
        <button class="primaire" id="config-enregistrer">Enregistrer</button>
        <span class="note" id="config-modifie" hidden>modifications non enregistrées</span>
      </div>
    </div>`;

  // LA VERIFICATION EST EN DIRECT, ET ELLE EST SEULE (22/09/2026). Il y
  // avait un bouton « Vérifier sans enregistrer » a cote d'« Enregistrer ».
  // Il appelait la MEME fonction que la frappe, a 400 ms pres : un bouton
  // qui ne fait rien de plus que ce qui se fait deja tout seul. L'editeur :
  // « je comprends pas à quoi il sert ». Aucun champ ne demande d'appel
  // exterieur — tout se verifie sur le poste, dans `champs.rs` —,
  // donc rien ne justifie un declenchement a la main. Le jour ou un champ
  // ira interroger un service (l'annuaire des entreprises pour un SIRET),
  // c'est CE champ-la qui portera son bouton, pas le formulaire entier.
  document.querySelectorAll(
    "#config-formulaire input, #config-formulaire textarea, #config-formulaire select")
    .forEach((c) => { c.oninput = () => { marquerModifie();
      clearTimeout(verifierFormulaire._t);
      verifierFormulaire._t = setTimeout(verifierFormulaire, 400); }; });
  document.getElementById("config-enregistrer").onclick = () => enregistrerFiche(fiche);
  document.getElementById("config-annuler").onclick = annulerFiche;
  const choixMetier = document.getElementById("config-metier-choix");
  if (choixMetier) {
    choixMetier.value = (fiche.donnees || {}).code_emploi_ft || "";
    // L'INTITULÉ SUIT LE MÉTIER (#236, 23/10/2026). Le champ libre « Emploi »
    // côtoyait la liste « Métier » : on tapait l'emploi, on ne choisissait pas
    // le métier, et l'enregistrement refusait sur des codes cachés (PCS-ESE,
    // statut). L'intitulé n'apparaît qu'une fois le métier choisi — prérempli
    // par le moteur (route « emploi »), à retoucher au besoin — ou s'il a déjà
    // une valeur : rien de saisi ne devient invisible.
    const emploi = document.querySelector('#config-formulaire [data-cle="emploi_affichage"]');
    const caseEmploi = emploi && emploi.closest(".champ");
    const montrerEmploi = () => {
      if (caseEmploi) caseEmploi.hidden = !choixMetier.value && !String(emploi.value || "").trim();
    };
    montrerEmploi();
    choixMetier.onchange = async () => {
      await remplirDepuisMetier(choixMetier.value); montrerEmploi(); marquerModifie();
    };
  }
  const allerMetiers = document.getElementById("aller-metiers");
  if (allerMetiers) allerMetiers.onclick = () =>
    document.querySelector('.sous-onglets button[data-fiche="metiers"]').click();
  brancherAide();
  brancherAtmp();
  peindreCoutAnnee();
  brancherListes(fiche);
  brancherNationalite();
  brancherConvention();
  brancherUrssaf();
  brancherSignatureCourriel();
  brancherSante(fiche);
  brancherPermanents();
  brancherConditions();       // après : c'est lui qui a le dernier mot
  brancherTaxeSalaires();
  brancherNir();
  brancherSalarie(fiche);
  FICHE_MODIFIEE = false;
  try { FICHE_INITIALE = JSON.stringify(lireFormulaire()); }
  catch (e) { FICHE_INITIALE = null; }
  const t = document.getElementById("config-modifie");
  if (t) t.hidden = true;
}

/* Le HTML colle, sans ce qui s'executerait dans la page : scripts, cadres,
 * objets, formulaires, gestionnaires `on...`, liens `javascript:`. Le moteur
 * refait ce menage a l'envoi ; ici, c'est la page qu'on protege. */
const SIGNATURE_LIMITE = 150 * 1024;
function nettoyerHtmlColle(html) {
  const doc = new DOMParser().parseFromString(`<body>${html}</body>`, "text/html");
  doc.body.querySelectorAll(
    "script,style,iframe,object,embed,form,noscript,template,svg,math,meta,link,base,input,button,textarea,select")
    .forEach((n) => n.remove());
  doc.body.querySelectorAll("*").forEach((n) => {
    [...n.attributes].forEach((at) => {
      if (/^on/i.test(at.name) || /^\s*(java|vb)script:/i.test(at.value)) n.removeAttribute(at.name);
    });
  });
  return doc.body.innerHTML.trim();
}

function brancherSignatureCourriel() {
  const cadre = document.getElementById("signature-cadre");
  const champ = document.querySelector('#config-formulaire [data-cle="courriel_signature"]');
  if (!cadre || !champ) return;
  const poids = document.getElementById("signature-poids");
  const peser = () => {
    const n = new Blob([champ.value]).size;
    poids.textContent = n ? `${Math.ceil(n / 1024)} Ko sur 150 Ko` : "signature habituelle";
    poids.style.color = n > SIGNATURE_LIMITE ? "#B22222" : "";
    if (n > SIGNATURE_LIMITE) poids.textContent += " — trop lourde : une image collée pèse tout son poids, préférez une image en ligne";
  };
  const relire = () => {
    // Un cadre vide garde souvent un <br> : c'est vide.
    const propre = cadre.textContent.trim() || cadre.querySelector("img") ? nettoyerHtmlColle(cadre.innerHTML) : "";
    champ.value = propre;
    peser();
    marquerModifie();
    clearTimeout(verifierFormulaire._t);
    verifierFormulaire._t = setTimeout(verifierFormulaire, 400);
  };
  cadre.addEventListener("input", relire);
  // Le collage garde la mise en forme, mais passe par le menage avant d'entrer.
  cadre.addEventListener("paste", (e) => {
    const html = e.clipboardData && e.clipboardData.getData("text/html");
    if (!html) return;               // du texte seul : le cadre le colle lui-meme
    e.preventDefault();
    document.execCommand("insertHTML", false, nettoyerHtmlColle(html));
    relire();
  });
  document.getElementById("signature-vider").onclick = () => {
    cadre.innerHTML = "";
    relire();
  };
  peser();
}

/* CHANGER D'URSSAF SE CONFIRME (08/10/2026). Celle du code postal est
 * celle qui recoit les declarations ; en choisir une autre sans un mot
 * passait inapercu jusqu'au rejet. On demande, et « non » remet l'ancienne. */
function brancherUrssaf() {
  const choix = document.querySelector('#config-formulaire select[data-cle="code_urssaf"]');
  if (!choix) return;
  let avant = choix.value;
  choix.addEventListener("change", () => {
    const deduit = String(ETAT.urssaf_deduit || "");
    const neuf = choix.value;
    if (!neuf || !deduit || neuf === deduit) { avant = neuf; return; }
    const nom = (code) => {
      const o = (ETAT.organismes_urssaf || []).find((x) => x.code === code);
      return o ? `${code} — ${o.nom}` : code;
    };
    let repondu = false;
    const repondre = (garder) => {
      if (repondu) return;
      repondu = true;
      if (garder) avant = neuf;
      else { choix.value = avant; marquerModifie(); }
    };
    ouvrirBoite(`<h3>Changer d'Urssaf ?</h3>
      <p>D'après l'adresse de l'entreprise, vos déclarations vont à
        <strong>${echapper(nom(deduit))}</strong>. C'est elle qui reçoit
        normalement DPAE et DSN.</p>
      <p>Vous avez choisi <strong>${echapper(nom(neuf))}</strong>.
        Une Urssaf qui ne vous gère pas peut faire rejeter vos déclarations.
        Voulez-vous vraiment la changer ?</p>
      <div class="actions">
        <button class="secondaire" id="u-garder-ancienne">Non, revenir en arrière</button>
        <button class="primaire" id="u-changer">Oui, changer</button>
      </div>`, () => repondre(false));
    document.getElementById("u-changer").onclick = () => { repondre(true); fermerBoite(); };
    document.getElementById("u-garder-ancienne").onclick = () => { repondre(false); fermerBoite(); };
  });
}

/* Le « ? » de chaque champ : il ouvre et referme son aide, et rien d'autre. */
function brancherAide() {
  document.querySelectorAll("#config-formulaire .aide-bouton").forEach((b) => {
    b.onclick = () => {
      const corps = document.querySelector(
        `#config-formulaire [data-aide-corps="${CSS.escape(b.dataset.aide)}"]`);
      if (!corps) return;
      corps.hidden = !corps.hidden;
      b.classList.toggle("ouvert", !corps.hidden);
    };
  });
}

/* Le « + » et la croix des listes (licences, etablissements). On redessine
 * la liste seule, a partir de ce qui est tape : rien n'est perdu en route. */
function brancherListes(fiche) {
  const lire = (cle) => {
    const l = [];
    document.querySelectorAll(
      `#config-formulaire [data-liste-de="${CSS.escape(cle)}"]`).forEach((c) => {
      const i = +c.dataset.listeIndex;
      l[i] = l[i] || {};
      l[i][c.dataset.sousCle] = c.value;
    });
    return l.filter(Boolean);
  };
  const redessiner = (cle, lignes) => {
    const sous = SOUS_CHAMPS[cle];
    const boite = document.querySelector(
      `#config-formulaire [data-liste="${CSS.escape(cle)}"]`);
    boite.innerHTML = lignes.map((l, i) => ligneStructure(cle, sous, l, i)).join("");
    boite.querySelectorAll("input").forEach((c) => {
      c.oninput = () => { marquerModifie();
        clearTimeout(verifierFormulaire._t);
        verifierFormulaire._t = setTimeout(verifierFormulaire, 400); };
    });
    brancherListes(fiche);
    marquerModifie();
  };
  document.querySelectorAll("#config-formulaire .ajouter-ligne").forEach((b) => {
    b.onclick = () => redessiner(b.dataset.ajouter, lire(b.dataset.ajouter).concat([{}]));
  });
  document.querySelectorAll("#config-formulaire .retirer-ligne").forEach((b) => {
    b.onclick = () => {
      const boite = b.closest(".liste-structure");
      const cle = boite.dataset.liste;
      const i = +b.closest(".ligne-structure").dataset.index;
      redessiner(cle, lire(cle).filter((_, n) => n !== i));
    };
  });
}


/*  LA NATIONALITE DSN SE DEDUIT DU PAYS (10/10/2026, vague 1k)
 *
 *  Avec un pays de nationalite de la liste, la case « Nationalite »
 *  (S21.G00.30.013) ne se choisit plus : elle montre ce que le moteur en
 *  deduit (`codification_ue` de chaque pays dans `etat.pays`), et le moteur
 *  la recrit a l'enregistrement. Sans pays, elle se choisit comme avant ;
 *  avec un pays absent de la liste (« Pays de nationalite absent de la
 *  liste », vague 2h), elle se choisit aussi : c'est le moteur qui refuse
 *  un tel pays sans nationalite choisie (`codification_ue`).
 */
function brancherNationalite() {
  const pays = document.querySelector('#config-formulaire [data-cle="pays_nationalite"]');
  const code = document.querySelector('#config-formulaire select[data-cle="code_nationalite_dsn"]');
  if (!pays || !code) return;
  const champ = code.closest(".champ");
  let note = champ.querySelector(".nationalite-deduite");
  if (!note) {
    note = document.createElement("div");
    note.className = "nationalite-deduite";
    code.insertAdjacentElement("afterend", note);
  }
  const lib = LIBELLES_CHOIX.code_nationalite_dsn || {};
  const poser = () => {
    const p = (ETAT.pays || []).find((x) => x.code === String(pays.value || "").toUpperCase());
    code.hidden = !!p;
    code.disabled = !!p;
    note.hidden = !p;
    if (!p) return;
    code.value = p.codification_ue;
    note.textContent = `${lib[p.codification_ue] || p.codification_ue} — déduite du pays de nationalité (${p.nom})`;
  };
  pays.addEventListener("change", poser);
  poser();
}

/*  UN SEUL CHOIX POUR LA CONVENTION (22/09/2026)
 *
 *  L'IDCC remplit ses deux doubles, qui restent dans la fiche en cases
 *  cachees : le LIBELLE (il s'imprime sur le bulletin) et le CODE CONVENTION
 *  DSN (rubrique S21.G00.11.022, sur l'etablissement, la meme valeur que
 *  l'IDCC du contrat en S21.G00.40.017).
 *
 *  Et l'ANNEXE ne se montre que pour la 3090 : les six annexes sont les
 *  siennes, elles ne veulent rien dire ailleurs. Elle ne se declare nulle
 *  part -- elle sert aux minima conventionnels.
 */
function brancherConvention() {
  const idcc = document.querySelector('#config-formulaire [data-cle="idcc"]');
  if (!idcc) return;
  const libelle = document.querySelector('#config-formulaire [data-cle="convention_collective"]');
  const codeDsn = document.querySelector('#config-formulaire [data-cle="code_convention_dsn"]');
  const annexe = document.querySelector('#config-formulaire [data-cle="annexe_convention"]');
  // Le libelle ne se reecrit que si l'on CHANGE de convention : au
  // chargement, celui que la fiche porte (« Spectacle vivant du secteur
  // privé ») est celui que la compagnie a ecrit, et il reste.
  const idccDepart = String(idcc.value || "");
  const poser = () => {
    const v = String(idcc.value || "");
    const cv = (ETAT.conventions || []).find((x) => String(x.idcc) === v);
    if (cv && libelle && (v !== idccDepart || !libelle.value)) libelle.value = cv.libelle;
    if (codeDsn) codeDsn.value = v;
    const champAnnexe = annexe && annexe.closest(".champ");
    if (champAnnexe) champAnnexe.hidden = v !== "3090";
    // CE QUI NE CONCERNE QU'UNE CONVENTION, ET QU'UNE GRANDE STRUCTURE
    // (23/09/2026). Le CSE decide du FNAS de la CCNEAC ; l'exemption de la
    // CASC-SVP n'existe que dans la 3090, et seulement a cinquante salaries
    // avec un CSE -- qui se dit alors dans le meme champ. Une petite
    // compagnie ne voit ni l'un ni l'autre, sauf s'ils sont deja remplis.
    const casc = document.querySelector('#config-formulaire [data-cle="casc_svp_exemptee"]');
    const cse = document.querySelector('#config-formulaire [data-cle="representation_personnel"]');
    const champCasc = casc && casc.closest(".champ");
    const champCse = cse && cse.closest(".champ");
    if (champCasc) champCasc.hidden = v !== "3090";
    if (champCse) {
      champCse.hidden = !(v === "1285" || (v === "3090" && casc && casc.checked)
                          || String(cse.value || "").trim());
    }
  };
  idcc.addEventListener("change", poser);
  const casc = document.querySelector('#config-formulaire [data-cle="casc_svp_exemptee"]');
  if (casc) casc.addEventListener("change", poser);
  poser();
}

/*  LE NUMERO DE SECURITE SOCIALE SE COLLE D'UN COUP (23/09/2026)
 *
 *  Il s'ecrit en quinze chiffres sur la carte Vitale ; la fiche le garde en
 *  deux cases, le numero et sa cle, parce que la cle VERIFIE le numero (le
 *  moteur la recalcule). Coller les quinze dans la premiere les repartit.
 *  Et le SEXE ne se montre que sans numero : c'est le seul cas ou il sert.
 */
function brancherNir() {
  const nir = document.querySelector('#config-formulaire [data-cle="nir"]');
  const cle = document.querySelector('#config-formulaire [data-cle="nir_cle"]');
  if (!nir) return;
  const sexe = document.querySelector('#config-formulaire [data-cle="sexe"]');
  const champSexe = sexe && sexe.closest(".champ");
  const poser = () => {
    const v = (nir.value || "").replace(/[\s.\-]/g, "");
    if (cle && /^\d{15}$/.test(v)) {
      nir.value = v.slice(0, 13);
      cle.value = v.slice(13);
    }
    if (champSexe) champSexe.hidden = !!nir.value.trim() && !String(sexe.value || "").trim();
  };
  nir.addEventListener("input", poser);
  poser();
}

/*  CE QUE LA FICHE SALARIE BRANCHE DE PLUS (23/09/2026)
 *
 *  - l'echelon CCNEAC ne se montre que pour une compagnie de la CCNEAC ;
 *  - « Exporter ses documents » mene a l'onglet Exports, le dossier de ce
 *    salarie deja choisi, sur l'annee la plus recente ;
 *  - une fiche neuve peut partir chez la personne, par courriel ;
 *  - ses imprévus (15/10/2026), et la porte pour en saisir un (js/imprevus.js).
 */
function brancherSalarie(fiche) {
  if (FICHE_ACTIVE !== "salarie") return;
  const idcc = String((((ETAT.configuration || {}).entreprise || {}).donnees || {}).idcc || "");
  const echelon = document.querySelector('#config-formulaire [data-cle="echelon_ccneac"]');
  if (echelon && idcc !== "1285" && !String(echelon.value || "").trim()) {
    echelon.closest(".champ").hidden = true;
  }
  const exporter = document.getElementById("config-exporter");
  if (exporter) exporter.onclick = () => exporterSalarie(fiche.cle);
  const inviter = document.getElementById("invitation-envoyer");
  if (inviter) inviter.onclick = () => demanderFiche(inviter);
  if (fiche.cle) dessinerImprevusDuSalarie(fiche.cle);
  if (fiche.cle) dessinerConseilsDuSalarie(fiche.cle);
}

/* Le salarie a choisir dans « Le dossier d'un salarié », a l'arrivee dans
 * l'onglet Exports (`dessinerDossier` le lit, puis l'oublie). */
let DOSSIER_PRECHOIX = null;

async function exporterSalarie(cle) {
  if (!(await quitterLaFiche())) return;
  DOSSIER_PRECHOIX = cle;
  // L'onglet se dessine a son arrivee, et `dessinerDossier` y pose le choix.
  document.querySelector('.onglets button[data-vue="exports"]').click();
}

async function demanderFiche(bouton) {
  const adresse = (document.getElementById("invitation-courriel").value || "").trim();
  const prenom = (document.getElementById("invitation-prenom").value || "").trim();
  if (!/^[^@\s]+@[^@\s]+\.[^@\s]+$/.test(adresse)) {
    dire("Il faut son adresse de courriel.");
    return;
  }
  const mdp = await demanderMotDePasse(`envoyer la demande de fiche à ${adresse}`, { envoi: true });
  if (!mdp) return;
  const rendre = occupe(bouton, "Envoi de la demande en cours…");
  const repere = await repereCourrielsFictifs();
  try {
    const r = await api("fiche-demander", { adresse, prenom, motdepasse: mdp });
    // La réponse ne porte plus la configuration (ni NIR ni IBAN) : la page la relit.
    ETAT.configuration = await api("configuration", {});
    FICHE_MODIFIEE = false;
    CLE_ACTIVE = null;
    FICHE_NEUVE = false;
    dessinerConfig();
    dire(`Demande envoyée à ${r.adresse}. Sa fiche se créera à l'arrivée de sa réponse.`, "envoye");
    fenetreCourrielsFictifs("fiche", repere);
  } catch (e) {
    rendre();
    dire("Envoi impossible : " + e.message, "erreur");
  }
}

/* Les demandes parties et pas revenues, sous la liste des salaries. */
function fichesAttenduesHtml() {
  const l = (ETAT.configuration || {}).fiches_attendues || [];
  if (!l.length) return "";
  return `<div class="encart attente-fiches">
    <b>${l.length} fiche(s) demandée(s), en attente de réponse</b>
    <button class="lien" id="fiches-relever">relever maintenant</button>
    <ul>${l.map((a) => `<li>${echapper(a.prenom ? a.prenom + " — " : "")}${
        echapper(a.adresse)} <span class="discret">demandée le ${
        echapper(jjmmaaaa(String(a.quand || "").slice(0, 10)))}</span>
      <button class="lien" data-oublier-fiche="${echapper(a.code)}">ne plus attendre</button></li>`).join("")}</ul>
  </div>`;
}

function brancherFichesAttendues() {
  const relever = document.getElementById("fiches-relever");
  if (relever) relever.onclick = async () => {
    relever.disabled = true;
    try {
      const r = await api("fiches-relever", {});
      ETAT.configuration = await api("configuration", {});
      dessinerConfig();
      dire(r.recues.length ? `${r.recues.length} fiche(s) reçue(s) : ${
        r.recues.map((x) => x.nom).join(", ")}.`
        : (r.incidents[0] || "Aucune réponse pour l'instant."),
        undefined);
    } catch (e) { relever.disabled = false; dire(e.message, "erreur"); }
  };
  document.querySelectorAll("[data-oublier-fiche]").forEach((b) => {
    b.onclick = async () => {
      try {
        await api("fiche-oublier-demande", { code: b.dataset.oublierFiche });
        // La réponse ne porte plus la configuration (ni NIR ni IBAN) : la page la relit.
        ETAT.configuration = await api("configuration", {});
        dessinerConfig();
      } catch (e) { dire(e.message, "erreur"); }
    };
  });
}

/** LE CURSEUR DES PERMANENTS, TEL QUE L'ÉCRAN LE MONTRE EN CE MOMENT
 *  (ticket #214, 20/10/2026). C'est la réponse du moteur : celle de la
 *  configuration enregistrée (`ETAT.configuration.permanents`), puis, la case
 *  à peine cochée ou décochée, celle de la route `permanents` sur la fiche en
 *  cours. La page ne le déduit jamais elle-même. */
let PERMANENTS_A_L_ECRAN = false;

/** LA CASE OUVRE LES CHAMPS TOUT DE SUITE, sans attendre l'enregistrement.
 *  La cocher et ne rien voir apparaître, ce serait laisser croire qu'elle ne
 *  fait rien — alors que c'est elle qui décide de ce qu'il reste à remplir.
 *  Ce qui s'ouvre, c'est le moteur qui le dit (route `permanents`).
 *  Le déroulant « Nature » d'un engagement, lui, suit à l'enregistrement :
 *  il vient du serveur, qui ne propose que ce que le moteur accepterait. */
function brancherPermanents() {
  PERMANENTS_A_L_ECRAN = !!(ETAT.configuration || {}).permanents;
  const case_ = document.querySelector(
    '#config-formulaire [data-cle="emploie_des_permanents"]');
  if (!case_) return;
  const montrer = () => {
    const oui = PERMANENTS_A_L_ECRAN;
    // Un champ qui dépend AUSSI d'autre chose (`si`, l'entreprise) suit
    // `brancherConditions`, qui écoute cette case comme les autres.
    document.querySelectorAll("#config-formulaire .champ-permanents")
      .forEach((c) => {
        if (c.hasAttribute("data-si") || c.hasAttribute("data-hors-entreprise")) return;
        c.hidden = !oui;
      });
    // La section entiere avec, sinon son titre reste seul au-dessus du vide.
    document.querySelectorAll("#config-formulaire [data-bloc-permanents]")
      .forEach((s) => { s.hidden = !oui; });
    const sante = document.getElementById("config-sante");
    if (sante) sante.hidden = !oui;
    // Un champ des permanents qui dépend aussi d'un autre suit sa condition.
    const form = document.getElementById("config-formulaire");
    if (form && form.appliquerConditions) form.appliquerConditions();
  };
  case_.addEventListener("change", async () => {
    try {
      const r = await api("permanents", { entreprise: { emploie_des_permanents: case_.checked } });
      PERMANENTS_A_L_ECRAN = !!r.permanents;
    } catch (e) { dire(e.message, "erreur"); return; }
    montrer();
  });
  montrer();
}

/** La condition d'un champ (`si`, `si_entreprise` du moteur) : chaque clé
 *  doit valoir l'une des valeurs dites ; "*" veut dire « remplie ». */
function conditionRemplie(si, lire) {
  return Object.entries(si || {}).every(([k, vals]) => {
    const v = String(lire(k) ?? "").trim();
    return vals.includes(v) || (vals.includes("*") && v !== "" && v !== "false");
  });
}

/** LES CHAMPS QUI DÉPENDENT D'UN AUTRE (`si` du moteur) : ils suivent la
 *  valeur à l'écran, tout de suite, comme la case des permanents. */
function brancherConditions() {
  const form = document.getElementById("config-formulaire");
  if (!form) return;
  const valeur = (k) => {
    const c = form.querySelector(`[data-cle="${CSS.escape(k)}"]`);
    return c ? (c.type === "checkbox" ? String(c.checked) : String(c.value ?? "")) : "";
  };
  const appliquer = () => {
    form.querySelectorAll("[data-si]").forEach((el) => {
      let si = {};
      try { si = JSON.parse(el.dataset.si); } catch (e) { return; }
      const ok = conditionRemplie(si, valeur) && !el.hasAttribute("data-hors-entreprise");
      // Le curseur des permanents : la réponse du moteur (`brancherPermanents`).
      const permOk = !el.classList.contains("champ-permanents") || PERMANENTS_A_L_ECRAN;
      el.hidden = !(ok && permOk);
    });
  };
  form.appliquerConditions = appliquer;
  const cles = new Set();
  form.querySelectorAll("[data-si]").forEach((el) => {
    try { Object.keys(JSON.parse(el.dataset.si)).forEach((k) => cles.add(k)); } catch (e) { /* rien */ }
  });
  cles.forEach((k) => {
    const c = form.querySelector(`[data-cle="${CSS.escape(k)}"]`);
    if (c) c.addEventListener("change", appliquer);
  });
  appliquer();
}

/** LA TAXE SUR LES SALAIRES (ticket #216, 20/10/2026). La compagnie dit sa
 *  TVA ; le moteur (route `taxe-sur-les-salaires-regime`, CGI art. 231) dit
 *  quelles cases suivent et ce qui en découle, écrit sous les cases. La page
 *  ne montre que ce qu'il nomme, et ne conclut rien. */
function brancherTaxeSalaires() {
  const cases = [...document.querySelectorAll(
    '#config-formulaire [data-sous-de="taxe_sur_les_salaires"]')];
  if (!cases.length) return;
  const bloc = cases[0].closest(".structure");
  let dit = bloc.querySelector(".taxe-consequence");
  if (!dit) {
    dit = document.createElement("div");
    dit.className = "encart taxe-consequence";
    bloc.querySelector(".sous-champs").after(dit);
  }
  const lire = () => {
    const o = {};
    cases.forEach((c) => {
      const v = String(c.value).trim();
      if (v === "") return;
      const t = c.dataset.sousType;
      o[c.dataset.sousCle] = t === "nombre" ? Number(v) : t === "booleen" ? v === "true" : v;
    });
    return o;
  };
  let tour = 0;
  const peindre = async () => {
    const n = ++tour;
    let r;
    try { r = await api("taxe-sur-les-salaires-regime", { taxe_sur_les_salaires: lire() }); }
    catch (e) { dit.textContent = e.message; return; }
    if (n !== tour) return;            // une réponse plus récente est en route
    cases.forEach((c) => {
      c.closest(".sous-champ").hidden = !r.champs.includes(c.dataset.sousCle);
    });
    dit.textContent = r.consequence;
    dit.classList.toggle("attention", !r.complet);
    // Les plafonds de la franchise, chiffrés par le moteur (ticket #239) :
    // l'aide de la question et ses deux réponses. La question ne se
    // comprend pas sans les montants : son aide est ouverte d'emblée (le
    // « ? » la referme).
    const f = r.franchise;
    if (f) {
      const corps = bloc.querySelector(
        '[data-aide-corps="taxe_sur_les_salaires.sous_franchise"]');
      if (corps) {
        if (n === 1) corps.hidden = false;
        // Un paragraphe par ligne de l'aide du moteur (ticket #245).
        corps.replaceChildren(...f.aide.split("\n").map((ligne) => {
          const p = document.createElement("p");
          p.textContent = ligne;
          return p;
        }));
      }
      // Les réponses sont des boutons à cocher (ticket #245) : chacune porte
      // la phrase du moteur.
      const choix = cases.find((c) => c.dataset.sousCle === "sous_franchise");
      const groupe = choix && choix.closest(".boutons-choix");
      if (groupe) groupe.querySelectorAll("[data-choix-libelle]").forEach((s) => {
        if (f.choix[s.dataset.choixLibelle]) s.textContent = f.choix[s.dataset.choixLibelle];
      });
    }
  };
  // Avant la première réponse, la première case seule : rien ne clignote.
  cases.slice(1).forEach((c) => { c.closest(".sous-champ").hidden = true; });
  cases.forEach((c) => c.addEventListener(c.tagName === "SELECT" ? "change" : "input", peindre));
  peindre();
}

/** Les trois cases de la complémentaire santé, dans les deux sens : elles se
 *  remplissent de ce que la fiche porte, et elles réécrivent le champ
 *  `sante_svp` à chaque changement. Le champ reste là, et c'est voulu — il
 *  est la vérité, et un cas non prévu doit pouvoir s'y taper à la main. */
function brancherSante(fiche) {
  const niveau = document.getElementById("config-sante-niveau");
  if (!niveau) return;
  const conjoint = document.getElementById("config-sante-conjoint");
  const enfants = document.getElementById("config-sante-enfants");
  const champ = document.querySelector('#config-formulaire [data-cle="sante_svp"]');
  const choix = (fiche.donnees || {}).sante_svp || null;
  if (choix) {
    niveau.value = choix.niveau || "";
    conjoint.checked = !!choix.conjoint;
    enfants.value = choix.enfants || 0;
  }
  const ecrire = () => {
    if (!champ) return;
    // Pas de niveau : pas de mutuelle. `null` et non un objet vide — une
    // fiche qui porterait {"niveau": ""} ferait croire à un choix fait.
    champ.value = JSON.stringify(
      niveau.value ? { niveau: niveau.value, conjoint: conjoint.checked,
                       enfants: +enfants.value || 0 } : null, null, 1);
    champ.classList.remove("faute");
    verifierFormulaire();
  };
  [niveau, conjoint, enfants].forEach((c) => { c.onchange = ecrire; });
}

/** Les six champs d'emploi d'une fiche salarié, depuis un métier du
 *  catalogue. La page ne calcule rien : elle recopie ce que le moteur rend
 *  (route « emploi »), qui applique les mêmes règles qu'un engagement. */
async function remplirDepuisMetier(code) {
  if (!code) return;
  let r;
  try { r = await api("emploi", { metier: code }); }
  catch (e) { dire(e.message, "erreur"); return; }
  const poser = (cle, v) => {
    const c = document.querySelector(`#config-formulaire [data-cle="${cle}"]`);
    if (c && v != null) c.value = v;
  };
  poser("emploi_affichage", r.emploi_affichage);
  poser("pcs_ese", r.pcs_ese || "");
  poser("code_emploi_ft", r.code_emploi_ft);
  poser("statut", r.statut);
  poser("statut_conventionnel", r.statut_conventionnel || "");
  poser("statut_categoriel_rc", r.statut_categoriel_rc);
  verifierFormulaire();
}

/*  L'HISTOIRE D'UNE FICHE
 *
 *  Une fiche n'est pas un etat, c'est une suite d'etats : on demenage, on
 *  change de telephone, et une valeur declaree se corrige. Chaque ligne dit
 *  ce qu'un champ valait et jusqu'a quel jour -- au-dela, la valeur courante
 *  du formulaire reprend la main.
 *
 *  C'est ce qui permet de rejouer une declaration deja deposee : la DSN de
 *  juillet est partie avec l'adresse de juillet, et elle doit pouvoir se
 *  recalculer a l'identique apres un demenagement en septembre.
 */
function histoireDuChamp(lignes, desc, donnees) {
  if (!Array.isArray(lignes) || !lignes.length) return "";
  const libelle = (cle) => (desc.find((c) => c.cle === cle) || {}).libelle || cle;
  const rangees = [...lignes].sort((a, b) =>
    a.champ === b.champ ? b.jusqu_au.localeCompare(a.jusqu_au)
                        : a.champ.localeCompare(b.champ));
  // Ce qui a remplace une valeur : la ligne suivante du meme champ, ou la
  // valeur courante de la fiche pour la plus recente.
  const remplacante = (e) => {
    const plus_tard = rangees.filter((x) => x.champ === e.champ
      && x.jusqu_au > e.jusqu_au).sort((a, b) => a.jusqu_au.localeCompare(b.jusqu_au));
    return plus_tard.length ? plus_tard[0].valeur : (donnees || {})[e.champ];
  };
  // REPLIEE DEPUIS LE 23/09/2026 : elle sert a qui la cherche, et elle
  // montrait a tout le monde, en bas de chaque fiche, les formes declarees.
  return `<details class="champ pleine historique-fiche">
    <summary>Historique des modifications (${rangees.length})</summary>
    <div class="aide">Les valeurs remplacées, et jusqu'à quand elles ont
       servi. Elles restent pour les déclarations déjà envoyées.</div>
    <div class="histoire">${rangees.map((e) => `
      <div class="histoire-ligne">
        <span class="histoire-champ">${echapper(libelle(e.champ))}</span>
        <span class="histoire-valeur">${echapper(valeurHistorique(e.champ, e.valeur, remplacante(e)))}</span>
        <span class="histoire-date">jusqu'au ${echapper(jjmmaaaa(e.jusqu_au))}</span>
        ${e.note ? `<div class="histoire-note">${echapper(e.note)}</div>` : ""}
      </div>`).join("")}</div>
  </details>`;
}

/* MODIFIER, C'EST TOUJOURS LE MEME BOUTON (24/09/2026) : un crayon et le
   mot. Il y en avait trois formes -- un lien rouge « Modifier… » sur les
   paies, un bouton vert au crayon sur les fiches, « Remplacer… » ou
   « Changer… » sur les mots de passe et la signature. Les points de
   suspension disent qu'une boite s'ouvre avant. Voir `.modifier`. */
const CRAYON = `<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 20h4L19 9l-4-4L4 16v4z"/><path d="M13.5 6.5l4 4"/></svg>`;

/* UN MONTANT S'ECRIT D'UNE SEULE FACON (24/09/2026) : « 1 053,08 € ». Le
   moteur envoie des nombres (« 210.80 ») ; la page en avait trois mises en
   forme locales, et une dizaine d'endroits montraient le nombre tel quel --
   la boite de depot DSN disait « 210.80 € » a cote d'un apercu en
   « 1 053,08 € ». Ce qui n'est pas un nombre repart tel quel. */
function euros(v) {
  const n = typeof v === "number" ? v : Number(String(v ?? "").trim());
  if (v === null || v === undefined || String(v).trim() === "" || !isFinite(n)) {
    return String(v ?? "");
  }
  return n.toLocaleString("fr-FR", { minimumFractionDigits: 2,
                                      maximumFractionDigits: 2 }) + " €";
}

function lireFormulaire() {
  const d = {};
  document.querySelectorAll("#config-formulaire [data-cle]").forEach((c) => {
    if (c.dataset.listeTextes) {
      d[c.dataset.cle] = c.value.split(",").map((x) => x.trim()).filter(Boolean);
    } else if (c.dataset.structure) {
      try { d[c.dataset.cle] = JSON.parse(c.value); }
      catch (e) { c.classList.add("faute"); }
    } else if (c.dataset.forme === "booleen") {
      d[c.dataset.cle] = c.checked;      // un vrai booleen, pas « true »
    } else {
      d[c.dataset.cle] = c.value;
    }
  });
  // LES STRUCTURES SAISIES CASE PAR CASE. On repart de l'objet d'origine :
  // le signataire porte un `nom_aem` et une note que l'ecran ne montre pas,
  // la banque un avertissement de diffusion. Ne relire que les cases
  // affichees, ce serait les effacer au premier enregistrement.
  const types = new Set();
  document.querySelectorAll("#config-formulaire [data-sous-de]").forEach((c) => {
    const p = c.dataset.sousDe;
    if (!d[p] || typeof d[p] !== "object" || Array.isArray(d[p])) {
      d[p] = JSON.parse(JSON.stringify(STRUCTURES_ORIGINE[p] || {}));
    }
    const chemin = c.dataset.sousCle.split(".");
    let o = d[p];
    chemin.slice(0, -1).forEach((k) => {
      if (!o[k] || typeof o[k] !== "object") o[k] = {};
      o = o[k];
    });
    const k = chemin[chemin.length - 1];
    // Une case typee (choix, nombre) vide est RETIREE : le moteur applique
    // alors son defaut, celui de la convention. Un texte garde sa valeur.
    if (c.dataset.sousType) {
      types.add(p);
      if (String(c.value).trim() === "") delete o[k];
      else if (c.dataset.sousType === "nombre") o[k] = Number(c.value);
      // Un oui/non : un vrai booleen, comme une case a cocher.
      else if (c.dataset.sousType === "booleen") o[k] = c.value === "true";
      else o[k] = c.value;
    } else {
      o[k] = c.value;
    }
  });
  // Un bloc dont il ne reste rien (sous-blocs vides compris) vaut « rien ».
  const vide = (x) => x === "" || x == null ||
    (typeof x === "object" && !Array.isArray(x) && Object.values(x).every(vide));
  types.forEach((p) => {
    if (d[p] && typeof d[p] === "object") {
      Object.keys(d[p]).forEach((k) => { if (vide(d[p][k])) delete d[p][k]; });
      if (!Object.keys(d[p]).length) d[p] = null;
    }
  });
  // LES LISTES (licences, etablissements). Une ligne entierement vide ne
  // compte pas : le « + » en pose une avant qu'on la remplisse, et une liste
  // sans rien dedans vaut « rien », pas « une ligne vide ».
  const listes = {};
  document.querySelectorAll("#config-formulaire [data-liste-de]").forEach((c) => {
    const p = c.dataset.listeDe, i = +c.dataset.listeIndex;
    listes[p] = listes[p] || [];
    // Pas de reprise de l'original ici, contrairement aux objets : une ligne
    // ajoutee ou retiree decale les index, et on recopierait la valeur cachee
    // de la ligne d'a cote. Ces listes n'ont pas de clef masquee.
    listes[p][i] = listes[p][i] || {};
    listes[p][i][c.dataset.sousCle] = c.value;
  });
  Object.keys(listes).forEach((p) => {
    const propres = listes[p].filter((l) => l &&
      Object.values(l).some((v) => String(v ?? "").trim() !== ""));
    d[p] = propres.length ? propres : null;
  });
  return d;
}

/*  CE QUE LE MOTEUR PROPOSE POUR UNE CASE VIDE (22/09/2026)
 *
 *  Le code Urssaf se déduit du code postal — deux tables relevées, les 101
 *  départements couverts. Mais le rattachement d'une entreprise à une
 *  Urssaf est un fait ADMINISTRATIF : versement en lieu unique, pôle
 *  grandes entreprises, GUSO. On POSE donc la valeur (ne pas la poser
 *  reviendrait à faire chercher ce qu'on sait déjà), et on DIT qu'elle est
 *  déduite, avec ce qui la ferait changer.
 *
 *  Deux règles, et elles tiennent tout :
 *   - on ne touche jamais une case que quelqu'un a remplie. C'est le
 *     serveur qui le garantit : `suggestions_fiche` ne propose que pour un
 *     champ vide ;
 *   - la mention reste tant que la valeur est celle qu'on a proposée. Elle
 *     disparaît dès qu'on la change : ce n'est alors plus une déduction,
 *     c'est un choix.
 */
function appliquerSuggestions(suggestions) {
  Object.entries(suggestions).forEach(([cle, s]) => {
    const champ = document.querySelector(`#config-formulaire [data-cle="${CSS.escape(cle)}"]`);
    // Le code Urssaf vide vaut deja « celui du code postal » (23/09/2026) :
    // le remplir ici l'ecrirait dans la fiche, et un demenagement ne le
    // suivrait plus.
    if (cle === "code_urssaf") return;
    if (!champ || champ.type === "hidden" || String(champ.value || "").trim()) return;
    champ.value = s.valeur;
    const boite = champ.closest(".champ");
    if (!boite || boite.querySelector(".suggere")) return;
    const note = document.createElement("div");
    note.className = "suggere";
    note.textContent = "Rempli pour vous : " + s.pourquoi;
    boite.appendChild(note);
    // Elle ne vaut que tant qu'on n'a pas choisi soi-même.
    champ.addEventListener("change", () => note.remove(), { once: true });
  });
}

/*  CE QU'ON A TAPÉ ET PAS ENREGISTRÉ (22/09/2026)
 *
 *  La page disait, sous les boutons, que l'ancienne version est copiee dans
 *  `config/avant/` — de la plomberie, affichee a quelqu'un qui n'a rien
 *  demande. Elle ne disait pas, en revanche, ce qui compte vraiment :
 *  quitter l'onglet perdait la saisie, en silence. C'est l'inverse
 *  maintenant. Rien si on n'a rien touche.
 */
let FICHE_MODIFIEE = false;
// LA FICHE TELLE QU'ELLE S'EST OUVERTE (08/10/2026). Taper un « a » puis
// l'effacer demandait d'enregistrer une fiche identique : on compare
// maintenant ce qui est dans les cases a ce qui y etait a l'ouverture.
let FICHE_INITIALE = null;

function ficheIdentique() {
  if (FICHE_INITIALE === null) return false;
  try { return JSON.stringify(lireFormulaire()) === FICHE_INITIALE; }
  catch (e) { return false; }
}

function marquerModifie() {
  const annuler = document.getElementById("config-annuler");
  if (ficheIdentique()) {
    FICHE_MODIFIEE = false;
    const t = document.getElementById("config-modifie");
    if (t) t.hidden = true;
    if (annuler && FICHE_ACTIVE === "entreprise") annuler.hidden = true;
    return;
  }
  FICHE_MODIFIEE = true;
  const t = document.getElementById("config-modifie");
  if (t) { t.hidden = false; t.textContent = "modifications non enregistrées"; }
  if (annuler) annuler.hidden = false;
}

/* ANNULER (ticket #124, 15/10/2026), le pendant d'« Enregistrer » : ce
 * qu'on a tapé ne s'écrit pas. Enregistrer referme la fiche et revient à la
 * liste (ecrireFiche) ; Annuler aussi, sans rien écrire : une fiche neuve
 * n'est pas créée, une fiche existante reste telle qu'elle est enregistrée.
 * La fiche Entreprise n'a pas de liste où revenir : Annuler y reprend ce
 * qui est enregistré, et ne se montre que si l'on a changé quelque chose.
 * C'est le « Abandonner les modifications » de `quitterLaFiche`, demandé
 * d'un geste : il ne redemande pas. */
function annulerFiche() {
  FICHE_MODIFIEE = false;
  if (FICHE_ACTIVE === "entreprise") {
    dessinerFormulaire(FICHE_COURANTE);
    return;
  }
  CLE_ACTIVE = null;
  FICHE_NEUVE = false;
  dessinerConfig();
}

/** Appelee avant de quitter la fiche en cours. Rend `true` si on peut
 *  partir. Trois issues : enregistrer et partir, partir quand meme, rester. */
async function quitterLaFiche() {
  if (!FICHE_MODIFIEE || ficheIdentique()) return true;
  const fiche = FICHE_COURANTE;
  const choix = await new Promise((resoudre) => {
    let repondu = false;
    const repondre = (v) => { if (!repondu) { repondu = true; resoudre(v); } };
    ouvrirBoite(`<h3>Vous avez modifié cette fiche</h3>
      <p>Vos changements ne sont pas encore enregistrés.</p>
      <div class="actions">
        <button class="primaire" id="q-enregistrer">Enregistrer</button>
        <button class="secondaire" id="q-jeter">Abandonner les modifications</button>
        <button class="secondaire" id="q-rester">Rester sur la fiche</button>
      </div>`, () => repondre("rester"));
    // REPONDRE D'ABORD, FERMER ENSUITE (22/09/2026). Dans l'autre ordre,
    // `fermerBoite` appelait le rappel de fermeture -- « rester » -- avant le
    // bouton : les trois boutons valaient « Rester sur la fiche ».
    document.getElementById("q-enregistrer").onclick = () => { repondre("enregistrer"); fermerBoite(); };
    document.getElementById("q-jeter").onclick = () => { repondre("jeter"); fermerBoite(); };
    document.getElementById("q-rester").onclick = () => { repondre("rester"); fermerBoite(); };
  });
  if (choix === "rester") return false;
  if (choix === "jeter") {
    // La fiche redevient celle de l'entree : on la redessine depuis ce qui
    // est enregistre, sinon la saisie abandonnee resterait dans les cases.
    if (fiche) dessinerFormulaire(fiche);
    FICHE_MODIFIEE = false;
    return true;
  }
  if (fiche) await enregistrerFiche(fiche);
  // Enregistrer peut avoir ete annule (la boite des valeurs datees) : dans
  // ce cas la fiche est toujours modifiee, et on ne part pas.
  return !FICHE_MODIFIEE;
}

/* LA VÉRIFICATION PENDANT LA FRAPPE (revue le 30/09/2026). Le texte sous
 * chaque case suit désormais ce qu'on tape : « obligatoire » restait sous un
 * SIRET valide, et un SIRET faux se colorait sans dire pourquoi. Plus de
 * message volant « N point(s) à regarder » à chaque frappe : la case parle. */
async function verifierFormulaire() {
  const r = await api("verifier-fiche",
    { nom: FICHE_ACTIVE, donnees: lireFormulaire() });
  const anomalies = r.anomalies;
  document.querySelectorAll("#config-formulaire [data-cle]").forEach((c) => {
    const a = anomalies.find((x) => x.champ === c.dataset.cle && !aRemplir(x));
    c.classList.toggle("faute", !!a && a.gravite === "erreur");
    c.classList.toggle("suspect", !!a && a.gravite === "avertissement");
    const champ = c.closest(".champ");
    if (!champ || c.type === "hidden") return;
    let note = champ.querySelector(":scope > .anomalie");
    if (!a) { if (note) note.remove(); return; }
    if (!note) { note = document.createElement("div"); champ.appendChild(note); }
    note.className = `anomalie ${a.gravite}`;
    note.textContent = a.message;
  });
  // UNE VALEUR FAUSSE SE VOIT (#236) : le tiroir « Avancé » qui en tient une
  // s'ouvre de lui-même — rien à corriger ne reste replié hors de la vue. Une
  // case seulement vide n'ouvre rien : celles du tiroir se remplissent seules
  // (le matricule et l'ancienneté à l'enregistrement, les codes par le métier,
  // qui se dit sur sa liste : `enregistrerFiche`).
  document.querySelectorAll("#config-formulaire details.avance:not([open])").forEach((d) => {
    const fautif = [...d.querySelectorAll("[data-cle]")].some((c) => c.type !== "hidden"
      && anomalies.some((x) => x.champ === c.dataset.cle && x.gravite === "erreur" && !aRemplir(x)));
    if (fautif) d.open = true;
  });
  appliquerSuggestions(r.suggestions || {});
  return anomalies;
}

/** Une case obligatoire laissée vide : l'astérisque suffit à l'écran, le
 *  refus d'enregistrer la nommera. */
function aRemplir(a) {
  return !!a && a.gravite === "erreur" && a.message === "obligatoire";
}

/*  ENREGISTRER UNE FICHE : DATER CE QUI CHANGE
 *
 *  Le §18.12 du journal raconte ce qui arrive quand on ne le fait pas. Une
 *  commune de naissance a ete corrigee depuis cet ecran ; l'avertissement
 *  disait bien « enregistrer declarera PARIS 12E », et il etait juste. Ce
 *  qu'il ne disait pas, c'est que 55 DSN etaient deja parties avec l'autre
 *  forme, et que l'identite est ce sur quoi France Travail et la DGFiP
 *  rapprochent un salarie.
 *
 *  D'ou cette etape, et les deux choses qu'elle demande :
 *
 *   - A PARTIR DE QUAND la nouvelle valeur vaut. Par defaut aujourd'hui,
 *     modifiable : si une declaration est partie ce matin avec l'ancienne,
 *     la nouvelle ne vaut qu'a partir de demain. C'est l'humain qui sait.
 *   - POURQUOI. Une correction et un demenagement se datent pareil ; seule
 *     la note dira, dans deux ans, laquelle des deux c'etait.
 *
 *  Et elle MONTRE ce que l'ancienne valeur a deja servi a declarer, parce
 *  que c'est cette information-la qui fait decider.
 */
async function enregistrerFiche(fiche) {
  const donnees = lireFormulaire();
  // Le moteur dit ce qui change une valeur existante : chacune demande sa
  // date d'effet, et le moteur refuse sans elle (toute la configuration est
  // datée). Une fiche neuve, une case vide qu'on remplit : rien à demander.
  const { changements } = await api("changements-declares",
    { nom: FICHE_ACTIVE, cle: fiche.cle, donnees });
  if (changements.length) {
    const daté = await demanderDateDEffet(changements);
    if (!daté) return;                       // annulé : rien n'est écrit
    return ecrireFiche(fiche, donnees, daté);
  }
  return ecrireFiche(fiche, donnees, {});
}

/* Une valeur declaree n'est pas toujours un mot : `signataire` et `banque`
 * sont des structures. `echapper(objet)` donnait « [object Object] → [object
 * Object] », c'est-a-dire un changement qu'on ne peut ni lire ni juger --
 * alors que c'est precisement ce que cette boite demande de juger. */
function valeurLisible(v) {
  if (v === null || v === undefined) return "—";
  if (typeof v === "object") {
    return Array.isArray(v) ? v.map(valeurLisible).join(" · ")
      : Object.entries(v).map(([k, x]) => `${k} : ${valeurLisible(x)}`).join(" · ");
  }
  return String(v);
}

/* L'HISTORIQUE D'UN BLOC (24/09/2026). `declarant_dsn` est une structure :
 * l'historique affichait « [object Object] » quand on avait seulement passe
 * le courriel en minuscules. On ne montre que ce qui a CHANGE, avec le
 * libelle de la case -- « Courriel : CONTACT@EXEMPLE.FR ». Une cle que
 * l'ancienne valeur ne portait pas n'est pas un changement : elle est nee
 * depuis. Les cles `_…` sont des notes, pas des valeurs. */
const LIBELLES_CACHES = { nom_prenom: "Nom déclaré", qualite: "Qualité",
                          bic: "BIC", iban: "IBAN", taux: "Taux du contrat" };
function valeurHistorique(champ, avant, apres) {
  if (avant === null || typeof avant !== "object" || Array.isArray(avant)) {
    return valeurLisible(avant);
  }
  const libelles = Object.fromEntries(
    ((SOUS_CHAMPS[champ] || {}).champs || []).map(([k, l]) => [k, l]));
  const courant = apres && typeof apres === "object" ? apres : {};
  const cles = Object.keys(avant).filter((k) => !k.startsWith("_"));
  const changees = cles.filter((k) =>
    JSON.stringify(avant[k]) !== JSON.stringify(courant[k]));
  return (changees.length ? changees : cles)
    .map((k) => `${libelles[k] || LIBELLES_CACHES[k] || k} : ${valeurLisible(avant[k])}`)
    .join(" · ");
}

/* RIEN DE ROUGE (25/09/2026). « On a le droit de changer de métier, je ne sais
 * même pas pourquoi on mettrait un avertissement. » Un changement de fiche est
 * un geste normal : la boîte ne demande que la date d'effet — c'est elle qui
 * garde aux déclarations déjà faites leur ancienne valeur (fiches datées) —,
 * sur un ton neutre. Le métier se dit en UNE ligne : emploi, PCS-ESE et code
 * France Travail changent ensemble, ce sont trois écritures d'un seul fait. */
const CHAMPS_DU_METIER = ["emploi_affichage", "emploi", "pcs_ese", "code_emploi_ft", "statut"];

function demanderDateDEffet(changements) {
  const aujourdHui = aujourdhuiIso();   // le jour du moteur (#236)
  const metier = changements.filter((c) => CHAMPS_DU_METIER.includes(c.champ));
  const autres = changements.filter((c) => !CHAMPS_DU_METIER.includes(c.champ));
  const lisible = (c, cote) => valeurLisible(c[cote + "_lisible"] ?? c[cote]);
  const ligne = (titre, avant, apres, detail = "") => `<div class="changement">
      <div class="changement-titre">${echapper(titre)}</div>
      <div class="changement-valeurs">
        <span class="avant">${echapper(avant)}</span>
        <span class="fleche">→</span>
        <span class="apres">${echapper(apres)}</span>
      </div>${detail ? `<div class="changement-prix">${detail}</div>` : ""}
    </div>`;
  let blocs = "";
  if (metier.length) {
    const emploi = metier.find((c) => c.champ === "emploi_affichage")
      || metier.find((c) => c.champ === "emploi") || metier[0];
    const codes = metier.filter((c) => c !== emploi)
      .map((c) => `${c.libelle} : ${lisible(c, "apres")}`).join(" · ");
    blocs += ligne("Métier", lisible(emploi, "avant"), lisible(emploi, "apres"),
                   codes ? `<span class="discret">${echapper(codes)}</span>` : "");
  }
  blocs += autres.map((c) => ligne(c.libelle, lisible(c, "avant"), lisible(c, "apres"))).join("");
  const titre = metier.length && !autres.length ? "Nouveau métier" : "Ce qui change";
  return new Promise((resoudre) => {
    let repondu = false;
    const repondre = (v) => { if (!repondu) { repondu = true; resoudre(v); } };
    ouvrirBoite(`<h3>${titre}</h3>
      <div class="changements">${blocs}</div>
      <label class="champ-boite">À partir du
        <input type="date" id="b-depuis" value="${aujourdHui}">
      </label>
      <p class="note">Ce qui est déjà déclaré garde l'ancienne valeur ; la
         nouvelle vaut à partir de cette date.</p>
      <details class="b-note-bloc"><summary class="discret">ajouter une note</summary>
        <input type="text" id="b-note" placeholder="correction, déménagement…">
      </details>
      <div class="actions">
        <button class="secondaire" id="b-annuler">Annuler</button>
        <button class="primaire" id="b-dater">Enregistrer</button>
      </div>`,
      // Fermer la boite sans repondre -- clic a cote -- vaut annuler : c'est
      // ce que l'ecran montre, et rien ne doit s'ecrire en douce.
      () => repondre(null));
    document.getElementById("b-annuler").onclick = fermerBoite;
    document.getElementById("b-dater").onclick = () => {
      repondre({ depuis: document.getElementById("b-depuis").value,
                 note: document.getElementById("b-note").value });
      fermerBoite();
    };
  });
}

/* `forcer` : l'utilisateur a passé outre un refus, dans la boîte qui le
 * lui propose. Un seul chemin d'écriture, forcé ou non (30/09/2026) : le
 * chemin forcé se contentait de redessiner, et la fiche restait « modifiée »
 * -- quitter l'onglet demandait d'enregistrer ce qui l'était déjà. */
async function ecrireFiche(fiche, donnees, daté, forcer = false) {
  try {
    const r = await api("enregistrer-fiche",
      { nom: FICHE_ACTIVE, cle: fiche.cle, donnees, ...daté, ...(forcer ? { forcer: true } : {}) });
    ETAT = await api("etat");
    // Une fiche neuve : le moteur a fabriqué sa clé, c'est elle qu'on ouvre.
    if (!fiche.cle && r.fiche && r.fiche.cle) { CLE_ACTIVE = r.fiche.cle; FICHE_NEUVE = false; }
    // « Fiche créée : null. » pour l'entreprise, la clé technique pour un
    // spectacle : le mot suffit.
    dire(forcer ? "Enregistré malgré les erreurs." : fiche.cle ? "Enregistré." : "Fiche créée.");
    FICHE_MODIFIEE = false;
    // ENREGISTRER REFERME LA FICHE (24/09/2026). Elle se redessinait ouverte,
    // identique : rien ne distinguait une fiche enregistree d'une fiche qui
    // ne l'etait pas, et l'editeur attendait qu'elle se ferme. Un salarie ou un
    // spectacle revient a la liste, sa ligne marquee un instant ; la fiche
    // Entreprise, qui n'a pas de liste, reste et dit l'heure.
    const enregistree = CLE_ACTIVE;
    if (FICHE_ACTIVE !== "entreprise") { CLE_ACTIVE = null; FICHE_NEUVE = false; }
    dessinerConfig();
    if (FICHE_ACTIVE === "entreprise") {
      // L'heure du moteur (`enregistre_a`), pas celle du navigateur : au banc, le temps est simulé.
      const note = document.getElementById("config-modifie");
      if (note) { note.hidden = false; note.textContent = `enregistré à ${r.enregistre_a.slice(11, 16)}`; }
    } else if (enregistree) {
      const rang = document.querySelector(`#config-liste .rang.fiche[data-cle="${CSS.escape(enregistree)}"]`);
      if (rang) {
        rang.classList.add("vient-d-etre-enregistree");
        rang.scrollIntoView({ behavior: "smooth", block: "nearest" });
        setTimeout(() => rang.classList.remove("vient-d-etre-enregistree"), 2500);
      }
    }
    if (!fiche.cle) {
      // Une fiche neuve : les listes des blocs de paie doivent la proposer
      // sans qu'on recharge le mois.
      rafraichirReferentiels();
    }
  } catch (e) {
    if (e.detail?.type === "FicheRefusee") {
      // On propose de forcer, mais on ne force jamais tout seul : une regle
      // peut se tromper, l'utilisateur en sait parfois plus qu'elle.
      // Le libellé du champ, pas sa clé technique (« numero_objet »).
      const libelles = {};
      (ETAT.configuration.description[FICHE_ACTIVE] || []).forEach((c) => { libelles[c.cle] = c.libelle; });
      let erreurs = e.detail.anomalies.filter((a) => a.gravite === "erreur");
      // Les codes d'un métier qu'on n'a pas choisi manquent tous ensemble :
      // une ligne, « Métier : à choisir », plutôt que quatre codes.
      // Sans métier choisi, un seul code qui manque se dit aussi « Métier » :
      // ces codes ne se tapent pas, ils viennent du métier (#236).
      const sansMetier = !(document.getElementById("config-metier-choix") || {}).value;
      const duMetier = erreurs.filter((a) => CHAMPS_DU_METIER.includes(a.champ) && aRemplir(a));
      if (duMetier.length > 1 || (duMetier.length && sansMetier)) {
        erreurs = erreurs.filter((a) => !duMetier.includes(a));
        erreurs.unshift({ champ: "Métier", message: "à choisir dans la liste" });
      }
      // LE MOTEUR DIT CE QUI SE FORCE (#236, règle 2) : une erreur de forme
      // (`forcable`), oui ; une valeur obligatoire absente d'un salarié ou d'un
      // spectacle (son métier…), non — sans elle, ni DPAE ni DSN.
      const forcable = e.detail.anomalies.filter((a) => a.gravite === "erreur").every((a) => a.forcable !== false);
      // Le libellé et son message sur une ligne : `.encart b` est un titre.
      ouvrirBoite(`<h3>Enregistrement refusé</h3>
        <div class="encart erreur"><b>${erreurs.length > 1 ? `${erreurs.length} erreurs` : "Une erreur"}</b>
          ${erreurs.map((a) => `<div class="anomalie"><b>${echapper(libelles[a.champ] || a.champ)}</b> : ${echapper(a.message)}</div>`).join("")}</div>
        ${forcable
          ? `<p class="note">Ces champs partent dans des déclarations. Si la règle
           se trompe et que la valeur est juste, on peut passer outre — mais il
           faut le vouloir.</p>`
          : `<p class="note">Sans ces valeurs, ni la DPAE ni la DSN ne peuvent se
           faire : elles se remplissent avant d'enregistrer.</p>`}
        <div class="actions">
          <button class="${forcable ? "secondaire" : "primaire"}" onclick="fermerBoite()">Corriger</button>
          ${forcable ? `<button class="primaire" id="b-forcer">Enregistrer quand même</button>` : ""}
        </div>`);
      // La date d'effet deja saisie voyage avec : passer outre une erreur
      // ne doit pas faire perdre au passage ce qu'on venait de dater.
      const forcer = document.getElementById("b-forcer");
      if (forcer) forcer.onclick = () => { fermerBoite(); ecrireFiche(fiche, donnees, daté, true); };
    } else { dire(e.message, "erreur"); }
  }
}


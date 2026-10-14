/* L'identite de la compagnie : signature et tampon. */
"use strict";

/* Configuration → Signature & tampon (22/09/2026).

   L'éditeur : « je peux pas ajouter la signature dans l'interface, pourquoi ? ».
   Parce qu'il n'y avait AUCUNE route de fichier dans le logiciel. C'est la
   première, et elle sert la règle 0 : une compagnie qui installe PuppetPaie
   doit pouvoir poser SA signature, pas hériter de celle d'une autre.

   TROIS PARTIS PRIS, ET ILS TIENNENT ENSEMBLE :

   - ON MONTRE TOUJOURS CE QUI EST EN PLACE. « les gens doivent voir ce qu'ils
     ont mis » : poser une signature à l'aveugle, c'est ne jamais savoir ce
     qui part sur les attestations.
   - LE TAMPON EST DESSINÉ PAR DÉFAUT, et on le montre AUSSI — « que les gens
     le trouvent beau et ne passent pas trop de temps à faire le leur ». Il
     vient de la fiche Entreprise et la suit ; il n'y a rien à fournir.
   - RETIRER, C'EST REVENIR AU DÉFAUT. Un tampon importé qu'on retire rend la
     main au dessin. Rien à reconfigurer, rien à se rappeler. */
async function dessinerIdentite(cible) {
  const etat = (await api("identite")).identite;
  const t = Date.now();          // l'image vient d'être remplacée : pas de cache

  const apercu = (src, alt) => `<div class="identite-apercu">
    <img src="${src}" alt="${alt}"></div>`;

  // UNE LIGNE CHACUN (23/09/2026), comme les mots de passe : l'image en
  // place à gauche -- on voit toujours ce qui part --, ce qu'elle est et
  // d'où elle vient, puis les gestes. Le champ de fichier du navigateur
  // (« Choisir un fichier — aucun fichier choisi ») se cache derrière un
  // bouton qui dit ce qu'il fait.
  const rang = (quoi, titre, quoiDit) => {
    const e = etat[quoi] || { pose: false };
    // Le tampon a une image même quand rien n'est posé : celle que le
    // logiciel dessine. C'est tout l'objet de la case « automatique ».
    const vue = e.pose
      ? apercu(`/identite/${quoi}?t=${t}`, titre)
      : e.automatique ? apercu(`/identite/cachet-automatique?t=${t}`, titre)
      : `<div class="identite-apercu vide">rien de posé</div>`;
    const dit = e.pose
      ? `<span class="pastille bien">le vôtre</span> <span class="discret">${e.format.toUpperCase()},
           ${Math.round((e.octets || 0) / 1024)} Ko, posé le ${jjmmaaaa((e.quand || "").slice(0, 10))}</span>`
      : e.automatique
        ? `<span class="pastille bien">celui du logiciel</span> <span class="discret">dessiné
             depuis la fiche Entreprise, il la suit : rien à faire</span>`
        : `<span class="pastille alerte">rien de posé</span>`;
    return `<div class="rang rang-image">
      ${vue}
      <div class="rang-infos">
        <b class="rang-nom">${titre}</b>
        <span class="discret">${quoiDit}</span>
        <div>${dit}</div>
      </div>
      <div class="rang-actions">
        ${e.pose ? `<button class="lien" data-retirer="${quoi}">${
          e.automatique ? "Revenir à celui du logiciel" : "Retirer"}</button>` : ""}
        <label class="${e.pose ? "modifier" : "bouton-fichier"}" title="PNG, JPEG ou SVG, 2 Mo au plus">${
          e.pose ? CRAYON + "Modifier…" : e.automatique ? "Importer le vôtre…" : "Importer…"}
          <input type="file" accept="image/png,image/jpeg,image/svg+xml"
                 data-quoi="${quoi}"></label>
      </div>
    </div>`;
  };

  // OÙ VA CHAQUE IMAGE, en toutes lettres (§110) : le moteur en tient la
  // liste (`DOCUMENTS_IDENTITE`). La page disait de la signature
  // « l'attestation et le certificat » ; elle part aussi sur chaque contrat.
  const ou = (quoi) => {
    const docs = (etat[quoi] || {}).documents || [];
    return docs.length ? `Sur ${docs.map(echapper).join(", ")}` : "";
  };
  // LA SIGNATURE EST CELLE D'UNE PERSONNE : celle que la fiche Entreprise
  // nomme sous « Qui signe ». Posée pour quelqu'un d'autre, la page le dit.
  const sig = etat.signature || {};
  const qui = sig.signataire || {};
  const titreSignature = qui.nom
    ? `La signature de ${echapper(qui.nom)}${qui.fonction ? ", " + echapper(qui.fonction) : ""}`
    : "La signature";
  const ditSignature = `${ou("signature")}. Elle engage la personne qui signe au nom de
      la compagnie${qui.nom ? "" : ` — <span class="a-completer">la fiche Entreprise ne dit pas
      qui signe</span>`}.${sig.autre_personne ? `<span class="compte a-completer">Cette image a été
      posée pour ${echapper(sig.posee_pour)} ; la personne qui signe est maintenant
      ${echapper(qui.nom)}. Importer sa signature à elle, ou corriger la fiche Entreprise.</span>` : ""}
      <button class="lien" data-aller-fiche="entreprise">Qui signe (fiche Entreprise)</button>`;

  cible.innerHTML = `
    <p class="intro">Ce qui se pose sur les documents de la compagnie. PNG, JPEG ou
      SVG, 2 Mo au plus.</p>
    <div class="rangs">
      ${rang("signature", titreSignature, ditSignature)}
      ${rang("cachet", "Le tampon", `${ou("cachet")}. Il identifie la compagnie, il
        n'engage personne.`)}
      ${rang("logo", "Le logo", `${ou("logo")}, en haut à droite. Facultatif.`)}
    </div>`;
  cible.querySelectorAll("[data-aller-fiche]").forEach((b) => {
    b.onclick = () => document.querySelector(
      `.sous-onglets button[data-fiche="${b.dataset.allerFiche}"]`).click();
  });

  cible.querySelectorAll('input[type="file"]').forEach((ch) => {
    ch.onchange = async () => {
      const f = ch.files && ch.files[0];
      if (!f) return;
      ch.disabled = true;
      try {
        // On envoie les octets tels quels : le moteur reconnaît le format sur
        // l'en-tête du fichier, pas sur son extension — qui ne prouve rien.
        const contenu = await new Promise((ok, non) => {
          const l = new FileReader();
          l.onload = () => ok(String(l.result).split(",").pop());
          l.onerror = () => non(l.error);
          l.readAsDataURL(f);
        });
        await api("poser-identite", { quoi: ch.dataset.quoi, contenu });
        await dessinerIdentite(cible);
      } catch (e) {
        // Un format refusé se dit : il partait en exception, sans un mot.
        ch.disabled = false;
        ch.value = "";
        dire(e.message, "erreur");
      }
    };
  });

  cible.querySelectorAll("[data-retirer]").forEach((b) => {
    b.onclick = async () => {
      b.disabled = true;
      try {
        await api("retirer-identite", { quoi: b.dataset.retirer });
        await dessinerIdentite(cible);
      } catch (e) { b.disabled = false; dire(e.message, "erreur"); }
    };
  });
}

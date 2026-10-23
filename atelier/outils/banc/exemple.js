/* LE MODÈLE D'UNE PARTIE AU BANC (#236, phase 2) : le petit scénario de la preuve, joué dans la page comme un testeur, du
 * « + » à la DSN de novembre. Il a donné `paie/tests/donnees/scenarios/treteaux-du-banc/`.
 *
 *   PUPPETPAIE_ATELIER=<cible>/essais/paie-atelier node atelier/outils/banc/exemple.js /tmp/banc-treteaux
 *
 * Les captures vont dans `<banc>/captures/`. */
const fs = require("fs");
const path = require("path");
const banc = require("./banc.js");
const B = path.resolve(process.argv[2] || "/tmp/banc-treteaux");
const C = path.join(B, "captures") + "/";
const NOM = "Les Tréteaux du Banc";

async function voile(page) {
  for (let i = 0; i < 3; i++) if (await page.isVisible("#voile")) { await page.click("#voile", { position: { x: 5, y: 5 } }); await page.waitForTimeout(400); }
}
async function ouvrir() {
  const o = await banc.ouvrir(B, { compagnie: NOM });
  await o.page.waitForTimeout(2500);
  await voile(o.page);
  return o;
}
async function champ(page, cle, v) {
  const s = `#config-formulaire [data-cle="${cle}"]`;
  const tag = await page.$eval(s, (e) => e.tagName);
  if (tag === "SELECT") await page.selectOption(s, String(v));
  else if (await page.$eval(s, (e) => e.type) === "checkbox") { if (v) await page.check(s); else await page.uncheck(s); }
  else await page.fill(s, String(v));
}
async function sous(page, de, cle, v) {
  const s = `#config-formulaire [data-sous-de="${de}"][data-sous-cle="${cle}"]`;
  if (await page.$eval(s, (e) => e.tagName) === "SELECT") await page.selectOption(s, String(v)); else await page.fill(s, String(v));
}
const texte = (page) => page.$eval("body", (b) => b.innerText);
function etape(t) { console.log(`\n== ${t}`); }

(async () => {
  etape("lancer au 02/11/2026");
  const l = await banc.lancer(B, { jour: "2026-11-02" });
  console.log(l.url, l.jour);

  etape("« + » : ajouter la compagnie");
  let { page, fermer } = await banc.ouvrir(B);
  await page.click(".tuile.ajouter .invite");
  await page.waitForTimeout(300);
  await banc.capture(page, C + "p01-avis-ajouter.png");
  if (await page.isVisible("#avis-continuer")) await page.click("#avis-continuer");
  await page.fill(".tuile.ajouter input", NOM);
  await page.click(".tuile.ajouter button[type=submit]");
  await page.waitForLoadState("load");
  await page.waitForTimeout(3000);
  await banc.capture(page, C + "p02-compagnie-ajoutee.png");
  await voile(page);

  etape("la fiche Entreprise (convention 3090)");
  await page.click("button[data-vue=configuration]");
  await page.click("button[data-fiche=entreprise]"); await page.waitForTimeout(1500);
  await champ(page, "idcc", "3090"); await page.waitForTimeout(800);
  for (const [k, v] of Object.entries({ siret: "32520440200011", naf: "9001Z", licence_spectacle: "PLATESV-D-2024-000352",
    premiere_embauche: "2026-11-20", adresse_affichage: "12 quai de la Fosse", code_postal: "44000", commune_affichage: "Nantes",
    urssaf_compte: "5200000000", congesspectacles_adherent: "0000052000F", affiliation_france_travail: "80000052008" })) await champ(page, k, v);
  for (const [k, v] of Object.entries({ prenom_affichage: "Adolphe", nom_affichage: "Appia", fonction: "Gérant", tel: "0100000052", email: "direction@les-treteaux.invalid" })) await sous(page, "signataire", k, v);
  await sous(page, "declarant_dsn", "civilite", "01");
  for (const [k, v] of Object.entries({ prenom_affichage: "Adolphe", nom_affichage: "Appia", tel: "0100000052", email: "direction@les-treteaux.invalid" })) await sous(page, "declarant_dsn", k, v);
  await sous(page, "banque", "iban", "FR7630001007943252044000039"); await sous(page, "banque", "bic", "BANQFRPPXXX");
  await page.fill("#atmp-valeur", "1,10"); await page.fill("#atmp-code", "923AD");
  // Le code du service de santé au travail est sous « Avancé — cas particuliers », replié.
  await page.$eval("[data-cle=medecine_travail]", (e) => e.closest("details").querySelector("summary").click());
  await champ(page, "medecine_travail", "MT952");
  await banc.capture(page, C + "p03-entreprise-remplie.png");
  await page.click("#config-enregistrer"); await page.waitForTimeout(2500);
  let t = await texte(page);
  console.log(t.includes("Pour démarrer") ? t.slice(t.indexOf("Pour démarrer"), t.indexOf("Pour démarrer") + 300) : "plus rien ne manque");
  await fermer();

  etape("contournement : la qualité du déclarant (la fiche ne la montre pas)");
  const gestes = fs.readFileSync(B + "/scenario/gestes.jsonl", "utf8").trim().split("\n").map(JSON.parse);
  const donnees = gestes.filter((g) => g.route === "enregistrer-fiche" && g.ok && g.charge.nom === "entreprise").pop().charge.donnees;
  donnees.declarant_dsn = { ...donnees.declarant_dsn, qualite: "04" };
  ({ page, fermer } = await ouvrir());
  console.log(await page.evaluate((d) => api("enregistrer-fiche", { nom: "entreprise", cle: null, donnees: d }).then(() => "ok", (e) => "refus : " + e.message), donnees));
  await page.reload(); await page.waitForTimeout(2500); await voile(page);

  etape("les organismes et contrats de la DSN");
  await page.click("button[data-vue=configuration]");
  await page.click("button[data-fiche=organismes]"); await page.waitForTimeout(1500);
  await page.click("#organismes-enregistrer"); await page.waitForTimeout(2000);
  await page.click("button[data-fiche=entreprise]"); await page.waitForTimeout(1500);
  t = await texte(page);
  console.log(t.includes("Pour démarrer") ? t.slice(t.indexOf("Pour démarrer"), t.indexOf("Pour démarrer") + 300) : "plus rien ne manque");
  await banc.capture(page, C + "p04-configuration-complete.png");

  etape("une salariée : Loïe Fuller, comédienne");
  await page.click("button[data-fiche=salarie]"); await page.waitForTimeout(1000);
  await page.click("text=+ un salarié"); await page.waitForTimeout(1500);
  await page.selectOption("#config-metier-choix", "COM010"); await page.waitForTimeout(500);
  for (const [k, v] of Object.entries({ prenom_affichage: "Loïe", nom_affichage: "Fuller", nir: "2820144999002", nir_cle: "46",
    date_naissance: "1982-01-15", lieu_naissance_affichage: "Villeneuve-sur-Loire", departement_naissance: "44", pays_naissance: "FR",
    code_nationalite_dsn: "01", adresse_affichage: "7 rue de la Paix", code_postal: "44000", commune_affichage: "Nantes",
    email: "loie.fuller@exemple.invalid" })) await champ(page, k, v);
  await page.click("#config-enregistrer"); await page.waitForTimeout(2500);
  await banc.capture(page, C + "p05-salariee.png");

  etape("un spectacle");
  await page.click("button[data-fiche=spectacle]"); await page.waitForTimeout(1000);
  await page.click("text=+ un spectacle"); await page.waitForTimeout(1500);
  await champ(page, "titre", "Le Banc des menteurs");
  await champ(page, "numero_objet", "266Z00952011");
  await page.click("#config-enregistrer"); await page.waitForTimeout(2500);

  etape("l'engagement : deux cachets, les 20 et 21/11/2026");
  await page.click("button[data-vue=paie]"); await page.waitForTimeout(1500);
  await page.click("text=+ un engagement"); await page.waitForTimeout(1500);
  const bloc = page.locator(".engagement").last();
  await bloc.locator("text=— cocher —").click(); await page.waitForTimeout(400);
  await bloc.locator(".e-qui-case").first().check(); await page.waitForTimeout(400);
  await page.mouse.click(5, 300);
  await bloc.locator(".e-spectacle").selectOption({ index: 1 });
  await bloc.locator(".r-code").first().selectOption("cachet");
  await bloc.locator(".r-jours-btn").first().click(); await page.waitForTimeout(400);
  await page.click(".calendrier .c-jour[data-j='20']"); await page.click(".calendrier .c-jour[data-j='21']");
  await page.click(".calendrier .c-ok");
  await bloc.locator(".r-taux").first().fill("150");
  await bloc.locator(".r-lieu").first().fill("Salle fictive de Rezé");
  await bloc.locator(".r-lieu").first().press("Tab");
  await page.waitForTimeout(3000);
  await banc.capture(page, C + "p06-engagement.png");
  t = await texte(page);
  console.log(t.slice(t.indexOf("NET À VIRER"), t.indexOf("NET À VIRER") + 120));

  etape("DPAE refusée tant que la compagnie est une fictive ajoutée");
  await page.click("text=Envoyer les DPAE…").catch(() => {});
  await page.waitForTimeout(1500);
  await banc.capture(page, C + "p07-envoi-refuse-ajoutee.png");
  await fermer();

  etape("geste d'atelier : la compagnie devient une fictive aux envois simulés");
  await banc.simuler(B);
  for (const j of ["2026-11-20", "2026-12-01", "2026-12-16"]) {
    etape(`le calendrier avance au ${j}`);
    const r = await banc.jour(B, j);
    for (const p of (r.veille && r.veille.dernier && r.veille.dernier.horloge) || []) console.log("  " + p);
  }

  etape("relever les retours (« relever maintenant »)");
  ({ page, fermer } = await ouvrir());
  await page.waitForTimeout(7000);
  await page.click("#veille-maintenant"); await page.waitForTimeout(6000);
  await banc.capture(page, C + "p08-apres-releve.png");
  await page.click("button[data-vue=historique]").catch(() => {}); await page.waitForTimeout(2500);
  await banc.capture(page, C + "p09-historique.png");
  await fermer();
  banc.arreter(B);
  console.log("\nbanc arrêté ; scénario :", B + "/scenario");
})().catch((e) => { console.error(e); try { banc.arreter(B); } catch (x) {} process.exit(1); });

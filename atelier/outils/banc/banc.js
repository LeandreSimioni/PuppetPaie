#!/usr/bin/env node
/* LE BANC D'ESSAI DATÉ (#236, phase 2) : une installation bridée neuve, sa fenêtre lancée à un jour donné, chaque geste
 * enregistré dans un scénario rejouable sans navigateur (`paie/src/scenario.rs`), et le calendrier qui avance.
 *
 * Ligne de commande (la notice : LISEZ-MOI.md, à côté) :
 *   node banc.js lancer   <banc> --jour AAAA-MM-JJ [--paie <paie-atelier>] [--depot <dépôt public>]
 *   node banc.js jour     <banc> AAAA-MM-JJ
 *   node banc.js simuler  <banc>
 *   node banc.js etat     <banc>
 *   node banc.js capture  <banc> <fichier.png>
 *   node banc.js arreter  <banc>
 *
 * Module, pour un script Playwright :
 *   const banc = require("<dépôt>/atelier/outils/banc/banc.js");
 *   const { page, fermer } = await banc.ouvrir("<banc>");   // la page, jeton posé
 *   ... await page.click(...) ...
 *   await banc.jour("<banc>", "2026-12-01");                  // le calendrier avance (puis page.reload())
 *   await banc.capture(page, "/chemin/de/la/capture.png");
 *   await fermer();
 *
 * Le banc ne fait rien que le moteur ne fasse : il lance `paie-atelier interface` (le binaire de l'atelier, qui refuse
 * tout acte réel) et lui parle par `/atelier/<geste>` (`interface/mod.rs`, `Fenetre::atelier`). */
"use strict";

const fs = require("fs");
const path = require("path");
const http = require("http");
const { spawn, execSync } = require("child_process");

const DEPOT = path.resolve(__dirname, "../../..");
const PAIE_PAR_DEFAUT = [
  process.env.PUPPETPAIE_ATELIER,
  process.env.CARGO_TARGET_DIR && path.join(process.env.CARGO_TARGET_DIR, "essais/paie-atelier"),
  path.join(DEPOT, "atelier/sources/paie/target/essais/paie-atelier"),
].filter(Boolean);
// Les secrets du poste ne vont jamais à la fenêtre du banc.
const RETIREES = ["NET_ENTREPRISES_MOTDEPASSE", "URSSAF_MOTDEPASSE", "COURRIEL_COMPTE", "COURRIEL_MOTDEPASSE",
  "PUPPETPAIE_MOTDEPASSE", "PUPPETPAIE_CODE", "PUPPETPAIE_AUJOURDHUI", "PUPPETPAIE_INCIDENTS"];

function dire(t) { process.stdout.write(t + "\n"); }
function mourir(t) { process.stderr.write(t + "\n"); process.exit(1); }

function fichierEtat(banc) { return path.join(banc, "banc.json"); }
function lireEtat(banc) {
  try { return JSON.parse(fs.readFileSync(fichierEtat(banc), "utf8")); }
  catch (e) { throw new Error(`Pas de banc lancé dans ${banc} (node banc.js lancer ${banc} --jour AAAA-MM-JJ).`); }
}

function vivant(pid) {
  try { process.kill(pid, 0); return true; } catch (e) { return false; }
}

/* Un appel à la fenêtre : POST /atelier/<geste> (ou une autre adresse), le jeton en en-tête. */
function appeler(etat, chemin, charge, delaiMs = 600000) {
  return new Promise((resoudre, rejeter) => {
    const corps = Buffer.from(JSON.stringify(charge || {}));
    const r = http.request({ host: "127.0.0.1", port: etat.port, path: chemin, method: "POST", timeout: delaiMs,
      headers: { "Content-Type": "application/json", "X-Jeton": etat.jeton, "Content-Length": corps.length } }, (rep) => {
      let t = "";
      rep.setEncoding("utf8");
      rep.on("data", (m) => { t += m; });
      rep.on("end", () => { try { resoudre(JSON.parse(t)); } catch (e) { rejeter(new Error(`réponse illisible : ${t}`)); } });
    });
    r.on("timeout", () => r.destroy(new Error("la fenêtre ne répond pas")));
    r.on("error", rejeter);
    r.end(corps);
  });
}

/* LANCER : l'installation (copie de `logiciel/` et `fictives/` du dépôt, bridée : aucun `.edition.json`) si elle n'existe
 * pas encore, puis sa fenêtre au jour donné, enregistreur en marche. Un banc déjà commencé reprend son scénario. */
async function lancer(banc, { jour, paie, depot } = {}) {
  banc = path.resolve(banc);
  const ancien = (() => { try { return lireEtat(banc); } catch (e) { return null; } })();
  if (ancien && vivant(ancien.pid)) throw new Error(`Le banc ${banc} tourne déjà (pid ${ancien.pid}) : node banc.js arreter ${banc}.`);
  const source = path.resolve(depot || DEPOT);
  const inst = path.join(banc, "installation");
  if (!fs.existsSync(inst)) {
    fs.mkdirSync(inst, { recursive: true });
    fs.cpSync(path.join(source, "logiciel"), path.join(inst, "logiciel"), { recursive: true });
    fs.cpSync(path.join(source, "fictives"), path.join(inst, "fictives"), { recursive: true });
  }
  const binaire = paie || PAIE_PAR_DEFAUT.find((p) => fs.existsSync(p));
  if (!binaire || !fs.existsSync(binaire)) {
    throw new Error("paie-atelier introuvable : --paie <chemin>, ou le construire : cd atelier/sources/paie && "
      + "cargo build --profile essais --features atelier --bin paie-atelier");
  }
  const env = { ...process.env };
  for (const v of RETIREES) delete env[v];
  Object.assign(env, {
    PUPPETPAIE_CODE: path.join(inst, "logiciel", "donnees"),
    // Rien ne sort de la machine : ni le serveur de l'éditeur, ni la recherche de mise à jour (`garde_reseau.rs`).
    PUPPETPAIE_ESSAIS_HORS_RESEAU: "1",
    PUPPETPAIE_CLE: path.join(banc, "cle"),
    XDG_DATA_HOME: path.join(banc, "local"),
  });
  const args = ["interface", "--sans-navigateur", "--veille", "0", "--enregistrer", path.join(banc, "scenario")];
  if (jour) args.push("--jour", jour);
  else if (!fs.existsSync(path.join(banc, "scenario", "gestes.jsonl"))) throw new Error("Un banc neuf commence à un jour : --jour AAAA-MM-JJ.");
  const journal = fs.openSync(path.join(banc, "fenetre.log"), "a");
  const p = spawn(binaire, args, { env, cwd: banc, detached: true, stdio: ["ignore", "pipe", journal] });
  const url = await new Promise((resoudre, rejeter) => {
    let t = "";
    const delai = setTimeout(() => rejeter(new Error(`la fenêtre ne dit pas son adresse (voir ${banc}/fenetre.log)`)), 30000);
    p.stdout.on("data", (m) => {
      t += m;
      const l = t.split("\n").find((x) => x.startsWith("http"));
      if (l) { clearTimeout(delai); resoudre(l.trim()); }
    });
    p.on("exit", (c) => { clearTimeout(delai); rejeter(new Error(`la fenêtre s'est arrêtée (code ${c}) : ${t} (voir ${banc}/fenetre.log)`)); });
  });
  p.stdout.destroy();
  p.unref();
  const u = new URL(url);
  const etat = { pid: p.pid, port: Number(u.port), url, jeton: u.searchParams.get("jeton"), installation: inst, binaire };
  fs.writeFileSync(fichierEtat(banc), JSON.stringify(etat, null, 2) + "\n");
  return { ...etat, ...(await appeler(etat, "/atelier/etat", {})) };
}

/* LE CALENDRIER AVANCE au jour donné (9 h, Paris) ; la compagnie ouverte passe sa veille : l'horloge d'une fictive aux
 * envois simulés joue ce qui était dû (DPAE, mois, DSN, documents), les retours des dépôts joués reviennent. */
async function jour(banc, j) {
  const r = await appeler(lireEtat(path.resolve(banc)), "/atelier/jour", { jour: j });
  if (!r.ok) throw new Error(r.erreur);
  return r;
}

async function simuler(banc) {
  const r = await appeler(lireEtat(path.resolve(banc)), "/atelier/simuler", {});
  if (!r.ok) throw new Error(r.erreur);
  return r;
}

async function etat(banc) {
  return appeler(lireEtat(path.resolve(banc)), "/atelier/etat", {});
}

/* Playwright : celui du dépôt s'il y en a un, sinon celui de Node (npm -g). */
function playwright() {
  try { return require("playwright"); } catch (e) { /* le global */ }
  return require(path.join(execSync("npm root -g").toString().trim(), "playwright"));
}

function chromium() {
  const pw = playwright();
  const options = { headless: true };
  const racine = "/opt/pw-browsers";
  if (fs.existsSync(racine)) {
    const d = fs.readdirSync(racine).filter((n) => n.startsWith("chromium_headless_shell")).sort().pop();
    const exe = d && path.join(racine, d, "chrome-linux", "headless_shell");
    if (exe && fs.existsSync(exe)) options.executablePath = exe;
  }
  return pw.chromium.launch(options);
}

/* OUVRIR LA PAGE : un Chromium sans tête, la page de la fenêtre (le jeton passe en cookie). Rend { page, navigateur,
 * fermer }. */
async function ouvrir(banc, { largeur = 1400, hauteur = 900, compagnie } = {}) {
  const e = lireEtat(path.resolve(banc));
  const navigateur = await chromium();
  const contexte = await navigateur.newContext({ viewport: { width: largeur, height: hauteur }, locale: "fr-FR",
    timezoneId: "Europe/Paris" });
  // L'HORLOGE DE LA PAGE est celle du moteur : la page préremplit des dates (« à partir du », l'ancienneté) avec le jour
  // du navigateur ; au banc, c'est le jour simulé, qui coule ensuite au pas de la montre.
  const t = await appeler(e, "/atelier/etat", {});
  if (t.simule) await contexte.clock.install({ time: new Date(t.secondes * 1000) });
  const page = await contexte.newPage();
  await page.goto(e.url.replace("puppetpaie.localhost", "127.0.0.1"), { waitUntil: "load" });
  // `compagnie` : si l'accueil s'affiche (la fenêtre vient d'être lancée), sa tuile s'ouvre.
  if (compagnie && await page.locator(".tuile", { hasText: compagnie }).count()) {
    await page.locator(".tuile", { hasText: compagnie }).first().click();
    await page.waitForLoadState("load");
    await page.waitForSelector("button[data-vue]", { timeout: 60000 });
  }
  return { page, navigateur, fermer: () => navigateur.close() };
}

async function capture(page, fichier) {
  fs.mkdirSync(path.dirname(path.resolve(fichier)), { recursive: true });
  await page.screenshot({ path: fichier, fullPage: true });
  return fichier;
}

/* ARRÊTER la fenêtre du banc (l'installation et le scénario restent : `lancer` reprend). */
function arreter(banc) {
  const e = lireEtat(path.resolve(banc));
  if (vivant(e.pid)) {
    try { process.kill(-e.pid, "SIGTERM"); } catch (x) { process.kill(e.pid, "SIGTERM"); }
  }
  fs.rmSync(fichierEtat(path.resolve(banc)), { force: true });
}

module.exports = { lancer, jour, simuler, etat, ouvrir, capture, arreter, appeler, lireEtat };

/* ------------------------------------------------------------------ la ligne de commande */

function option(args, nom) {
  const i = args.indexOf(nom);
  if (i < 0) return undefined;
  const v = args[i + 1];
  args.splice(i, 2);
  return v;
}

async function principal() {
  const args = process.argv.slice(2);
  const commande = args.shift();
  const banc = args.shift();
  if (!commande || !banc) mourir("node banc.js lancer|jour|simuler|etat|capture|arreter <banc> ... (voir LISEZ-MOI.md)");
  switch (commande) {
    case "lancer": {
      const r = await lancer(banc, { jour: option(args, "--jour"), paie: option(args, "--paie"), depot: option(args, "--depot") });
      dire(`Banc lancé : ${r.url}\nJour simulé : ${r.jour} (${r.heure})\nScénario : ${r.scenario}`);
      break;
    }
    case "jour": {
      const r = await jour(banc, args[0]);
      dire(`Jour simulé : ${r.jour}`);
      const dernier = r.veille && r.veille.dernier;
      for (const p of (dernier && dernier.horloge) || []) dire(`  ${p}`);
      if (r.veille && r.veille.erreur) dire(`  ${r.veille.erreur}`);
      break;
    }
    case "simuler": await simuler(banc); dire("La compagnie ouverte a désormais ses envois simulés."); break;
    case "etat": dire(JSON.stringify(await etat(banc), null, 2)); break;
    case "capture": {
      const { page, fermer } = await ouvrir(banc);
      await page.waitForTimeout(1500);
      await capture(page, args[0]);
      await fermer();
      dire(args[0]);
      break;
    }
    case "arreter": arreter(banc); dire("Banc arrêté."); break;
    default: mourir(`Commande inconnue : ${commande}`);
  }
}

if (require.main === module) principal().catch((e) => mourir(e.message));

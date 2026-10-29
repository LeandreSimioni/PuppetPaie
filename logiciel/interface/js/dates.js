/* Les dates en francais : les tables des mois, et tout ce qui ecrit un mois ou un jour. */
"use strict";

/* LES TABLES DES MOIS. Trois, et pas une : elles ne disent pas la meme chose.
   MOIS_FR, le nom entier (« juillet 2021 », le titre de la case du mois).
   MOIS_ABR, l'abreviation (« +1 nov. » du planning, la grille des mois, le
   calendrier des exports). MOIS_COURTS, la carte d'un engagement valide, sous les
   jours : elle ne differe de MOIS_ABR que par « avril », ecrit en entier. */
const MOIS_FR = ["janvier", "février", "mars", "avril", "mai", "juin", "juillet",
                 "août", "septembre", "octobre", "novembre", "décembre"];
const MOIS_ABR = ["janv.", "févr.", "mars", "avr.", "mai", "juin", "juil.", "août",
                  "sept.", "oct.", "nov.", "déc."];
const MOIS_COURTS = ["janv.", "févr.", "mars", "avril", "mai", "juin", "juil.",
  "août", "sept.", "oct.", "nov.", "déc."];

/* Les jours de la semaine, en tête des calendriers (le planning d'un bloc),
   du lundi au dimanche. */
const JOURS_SEMAINE = ["L", "M", "M", "J", "V", "S", "D"];

/* LE JOUR QU'ON EST, CELUI DU MOTEUR. La page préremplissait
   des dates avec l'horloge du navigateur ; le jour vient du moteur (règle 2) —
   au banc d'essai, il a son temps simulé. L'ouverture le dit (`aujourdhui`,
   `etat_garde::FileDesEtats::ouverture`) et chaque ouverture fraîche le
   repose (`js/demarrage.js`). AAAA-MM-JJ ; vide tant qu'il n'est pas arrivé. */
let AUJOURDHUI = "";

/** Le jour du moteur, AAAA-MM-JJ. */
function aujourdhuiIso() { return AUJOURDHUI; }
/** L'année du moteur (un nombre), ou null avant l'ouverture. */
function anneeDuJour() { return AUJOURDHUI ? +AUJOURDHUI.slice(0, 4) : null; }


const moisEnTexte = (a, m) => `${a}-${String(m).padStart(2, "0")}`;
function moisLu(v) {
  const [a, m] = String(v || "").split("-").map(Number);
  return a && m ? [a, m] : null;
}

function moisPrecedent(mois) {
  const an = +mois.slice(0, 4), mo = +mois.slice(5, 7);
  return mo === 1 ? `${an - 1}-12` : `${an}-${String(mo - 1).padStart(2, "0")}`;
}

function moisSuivant(mois) {
  const an = +mois.slice(0, 4), mo = +mois.slice(5, 7);
  return mo === 12 ? `${an + 1}-01` : `${an}-${String(mo + 1).padStart(2, "0")}`;
}

/* « juillet 2021 » et non « 2021-07 » : on lit une phrase, pas un tri. */
function moisEnClair(iso) {
  if (!iso || iso.length !== 7) return iso || "";
  return `${MOIS_FR[Number(iso.slice(5, 7)) - 1]} ${iso.slice(0, 4)}`;
}

/* << d'aout >>, << de septembre >>. Une elision qui manque se lit comme une
 * faute de frappe, et une interface qui ecrit mal fait douter de ce qu'elle
 * compte. */
function deMois(iso) {
  const nom = moisEnClair(iso);
  return ("aeiouyâàéèêîôû".includes(nom[0]) ? "d'" : "de ") + nom;
}

/* Le mois déclaré, EN GROS : son nom en clair, et l'ISO dessous. */
function moisEnGros(mois) {
  return `<div class="mois-gros">
    <span class="clair">${echapper(moisEnClair(mois))}</span>
    <span class="iso">${echapper(mois)}</span>
  </div>`;
}

/* « 12/09/2026 ». Un texte qui n'est pas une date ISO revient tel quel ;
   pour un horodatage, on passe ses dix premiers caractères. */
function jjmmaaaa(iso) {
  if (!iso || iso.length !== 10) return iso || "";
  return `${iso.slice(8, 10)}/${iso.slice(5, 7)}/${iso.slice(0, 4)}`;
}

function quand(iso) {
  // « 12/09/2026 à 18:18 » : on travaille en France, une date à l'américaine
  // dans une phrase se lit mal. L'ISO reste dans les
  // fichiers et les échanges ; ce qui s'affiche passe par ici.
  const s = String(iso || "");
  if (s.length < 10 || s[4] !== "-" || s[7] !== "-") return s;
  const jour = `${s.slice(8, 10)}/${s.slice(5, 7)}/${s.slice(0, 4)}`;
  const heure = s.length >= 16 ? s.slice(11, 16) : "";
  return heure ? `${jour} à ${heure}` : jour;
}

/** « du 17/03/2026 au 20/03/2026 », ou « depuis le 04/09/2023 » pour un
 *  CDI : sans fin, le moteur rend `None` ou rien, et l'écran disait
 *  « au None ». */
function periodeEnClair(debut, fin) {
  return fin && fin !== "None"
    ? `du ${jjmmaaaa(debut)} au ${jjmmaaaa(fin)}`
    : `depuis le ${jjmmaaaa(debut)}`;
}

/** Les jours d'un mois, en clair et au plus court : « 6 », « 21–23 »,
 *  « 14 · 15 · 17 · 18 ». Des jours qui se suivent font une plage. */
function joursEnClair(isos) {
  const j = [...new Set(isos)].sort().map((x) => parseInt(x.slice(8, 10), 10));
  if (!j.length) return "";
  const suivis = j.every((v, k) => k === 0 || v === j[k - 1] + 1);
  if (j.length === 1) return String(j[0]);
  if (suivis) return `${j[0]}–${j[j.length - 1]}`;
  return j.join(" · ");
}

function jourDe(x) {
  return (x && typeof x === "object") ? String(x.jour || "") : String(x || "");
}


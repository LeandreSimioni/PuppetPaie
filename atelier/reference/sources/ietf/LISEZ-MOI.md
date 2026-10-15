# IETF : les formats d'Internet que le logiciel lit

## RFC 5322 : le format des messages (en-tête `Date` et fuseau)

| fichier | d'où | relevé |
|---|---|---|
| `ietf-rfc5322-internet-message-format-20081001.txt` | https://www.rfc-editor.org/rfc/rfc5322.txt, tel que servi (HTTP 200, `text/plain`, 122 322 octets) ; RFC 5322, « Internet Message Format », P. Resnick (éd.), Standards Track, octobre 2008 (obsolète la RFC 2822) | 13/10/2026 |

### Ce qu'on en lit

- **§ 3.3, « Date and Time Specification »** (lignes 828-849 du `.txt`) :
  - « The zone specifies the offset from Coordinated Universal Time (UTC […]) that the date and time-of-day represent. »
  - « The form "+0000" SHOULD be used to indicate a time zone at Universal Time. Though "-0000" also indicates
    Universal Time, it is used to indicate that the time was generated on a system that may be in a local time zone
    other than Universal Time and that the date-time contains no information about the local time zone. »
  - Donc **`-0000` est une heure UT** (comme `+0000`) ; ce qu'il ajoute à `+0000`, c'est que l'expéditeur *ne sait pas
    quel est son fuseau local*. Il ne dit jamais « heure locale du lecteur ».
- **§ 4.3, « Obsolete Date and Time »** (lignes 1838-1900) : `UT` et `GMT` valent `+0000` ; les zones américaines (`EST` =
  `-0500`…) ; « Z » équivaut à `+0000`, mais les zones militaires d'une lettre « SHOULD all be considered equivalent to
  "-0000" unless there is out-of-band information » ; et « Other multi-character (usually between 3 and 5) alphabetic
  time zones have been used in Internet messages. Any such time zone whose meaning is not known SHOULD be considered
  equivalent to "-0000" unless there is out-of-band information confirming their meaning. »
- **Annexe** (ligne 2879) : parmi les changements depuis la RFC 2822, « Specifically allow and give meaning to "-0000"
  time zone ».

### Où le moteur s'en sert

`atelier/sources/paie/src/courriel_lecture.rs` (`date_de`, `parsedate_tz`, `date_lue`) : la date d'un message de réponse
(fiche du salarié, signature), mise en heure de Paris pour le journal. Test : `tests/fichiers/courriel_lecture.rs`,
`les_dates_d_en_tete_en_heure_de_paris`.

### Le § 4.3, suivi depuis le lot U7 (13/10/2026)

- **Un nom de fuseau inconnu** (`CEST` seul, sans parenthèses) et **les zones militaires** d'une lettre : le § 4.3 (lignes
  1886-1900) dit de les tenir pour `-0000` (« SHOULD be considered equivalent to "-0000" unless there is out-of-band
  information confirming their meaning ») ; `-0000` est de l'UT (§ 3.3, plus haut). Le moteur le fait :
  `courriel_lecture::parsedate_tz`, un fuseau fait de lettres que sa table ne connaît pas vaut 0. Il n'a aucune
  information « hors bande » sur `CEST` : lui donner `+0200` serait une table inventée. Les zones américaines (`EST`…) et
  `UT`/`GMT` gardent leur décalage (§ 4.3, lignes 1838-1845 et 1867-1884) ; la table garde aussi `UTC`, `AST`, `ADT`, que le module
  `email` de Python lisait. Effet : un message daté `10:15:00 CEST` en octobre s'écrit 12:15 au journal, et non 10:15
  (l'oracle `courriel_lecture/cas.json` change de cette seule valeur, message 28).
- Tests : `tests/fichiers/courriel_lecture.rs`, `les_dates_d_en_tete_en_heure_de_paris` (`CEST`, `a`, `EST`).

### Ce que le moteur ne suit pas, faute de texte (inférence)

- **Un en-tête sans fuseau** (`Sat, 03 Oct 2026 10:15`) ou dont la seule indication est un commentaire (`(CEST)`, qui est du
  CFWS, pas une zone) : la RFC 5322 exige un fuseau (`date-time = [ day-of-week "," ] date time [CFWS]`, ligne 791 ;
  `zone = (FWS ( "+" / "-" ) 4DIGIT) / obs-zone`, ligne 818) ; ni le § 3.3 ni le § 4.3 ne disent comment lire une date qui
  n'en a pas. Le moteur la lit comme une heure de Paris, comme le faisait le module `email` de Python : un choix du moteur,
  pas un texte. (Le § 3.3 décrit `-0000` comme une date qui « contains no information about the local time zone » ; en
  faire la règle d'une date sans zone serait une analogie, que le moteur ne fait pas.)
- **Relu le 13/10/2026 (lot F1)** : la RFC ne tranche pas. La zone est obligatoire dans la syntaxe actuelle (`time =
  time-of-day zone`, ligne 808) comme dans l'ancienne (`obs-zone`, ligne 1838) : une date sans zone n'est conforme ni à
  l'une ni à l'autre. Ce qu'un lecteur fait d'une forme que le document ne décrit pas, le § 4 le laisse à
  l'implémentation (lignes 1711-1718) : « The fact that a particular form does not appear in any section of this
  document is not justification for computer programs to crash or for malformed data to be irretrievably lost by any
  implementation. It is up to the implementation to deal with messages robustly. » Lire l'heure comme celle de Paris
  reste donc une **inférence du moteur** (le lieu de la compagnie, et ce que faisait le module `email` de Python), nommée
  dans le commentaire de `courriel_lecture::date_de` et de `parsedate_tz` ; elle ne perd pas le message (la règle du
  § 4), et un message sans `Date` lisible prend l'heure de la relève.

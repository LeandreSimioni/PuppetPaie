# L'API Google Drive : les erreurs, les limites, le téléversement (relevé du 20/10/2026)

Relevé pour les tickets #231 et #232 : la sauvegarde du dossier privé dans le Drive (`paie/src/sauvegarde.rs`) recevait
parfois un « HTTP 500 : Internal Error » de Google, qui montait tel quel dans le bandeau des retours ; le passage suivant
passait. Ce que Google recommande pour ces erreurs-là.

Pages relevées le 20/10/2026 par `curl -L` (en anglais, `?hl=en`), puis transcrites par `pandoc -f html -t plain
--wrap=none` (le texte entier de la page, navigation comprise ; aucune retouche). Le contenu de ces pages est publié par
Google sous licence Creative Commons Attribution 4.0 (« Except as otherwise noted, the content of this page is licensed
under the Creative Commons Attribution 4.0 License », en pied de chaque page). Les originaux HTML, d'une entreprise privée,
vont au dépôt privé (`reference-privee/sources/google-drive-api/`) ; leurs condensats SHA-256 sont donnés ici.

| fichier | d'où | mise à jour de la page | SHA-256 de l'original HTML |
|---|---|---|---|
| `google-drive-api-gerer-les-erreurs-20261020.txt` | https://developers.google.com/workspace/drive/api/guides/handle-errors | 2026-09-03 | `de97e3c9814bc13f01652f7dda6f9ce1159b316313da0264efeb9f315c693a70` |
| `google-drive-api-limites-et-quotas-20261020.txt` | https://developers.google.com/workspace/drive/api/guides/limits | 2026-09-11 | `0c51b84ba49eec4b57e72c9fdfeea48cf6df51c34f813578bac14eec2a610b53` |
| `google-drive-api-televerser-20261020.txt` | https://developers.google.com/workspace/drive/api/guides/manage-uploads | 2026-09-03 | `7c1551633a0d496512c93a2482a10f6ea8fa6c1e7bf5104d73ee1081daea3fd5` |

## Ce qu'on en lit

- **Gérer les erreurs**, « 429 errors » : « These errors mean that too many requests were sent to the API too quickly. [...]
  To fix this error, use exponential backoff to retry the request. »
- **Gérer les erreurs**, « 500, 502, 503, 504 errors » : « These errors occur when an unexpected server error arises while
  processing the request. [...] 500 Backend error, 502 Bad Gateway, 503 Service Unavailable, 504 Gateway Timeout. To fix
  this error, use exponential backoff to retry the request. »
- **Limites et quotas**, « Resolve time-based quota errors » et « Example algorithm » : « wait 1 + random_number_milliseconds
  and retry the request », puis 2, puis 4, « up to a maximum_backoff time » ; « random_number_milliseconds is a random number
  of milliseconds less than or equal to 1,000 » ; « Continue waiting and retrying up to some maximum number of retries » ;
  « At some point, clients should be prevented from retrying indefinitely. » Et : « If you exceed a quota, you'll receive a
  403: User rate limit exceeded [...] Additional rate limit checks on the Drive backend might also generate a 429: Rate limit
  exceeded response. If this happens, you should use an exponential backoff algorithm and try again later. »
- **Téléverser**, « Handle media upload errors » : « For 5xx errors, resume or retry uploads that fail due to connection
  interruptions. » ; « For 403 rate limit errors, retry the upload. »

## Où le moteur s'en sert

- `paie/src/drive.rs` : `NOUVELLES_TENTATIVES` (trois), `passagere` (500, 502, 503, 504, 429, et un 403 de limite de
  débit), `attente` (1 s, 2 s, 4 s, plus un hasard d'au plus la base), dans `Client::appel_brut`, par où passent tous les
  appels Drive (dossiers, listages, dépôts, téléchargements).
- `paie/src/sauvegarde.rs` : ce qui échoue encore après ces tentatives est **reporté au passage suivant**, sans incident ;
  seul ce qui persiste se dit (`souci`).
- Preuves : `tests/fichiers/drive.rs` (un faux serveur qui rend 500 puis 200, quatre 5xx, 429 et 403 de débit, l'attente
  qui croît, le dépôt qui repart avec le même corps) et `tests/fichiers/sauvegarde.rs`, `tests/ouverture.rs`.

**Inférence**, non écrite par Google : trois nouvelles tentatives (sept secondes d'attente au plus, hasard compris
jusqu'à dix) suffisent pour une panne passagère ; au-delà, c'est le passage suivant de la veille (vingt minutes plus tard)
qui retente, ce qui tient lieu du « maximum_backoff ».

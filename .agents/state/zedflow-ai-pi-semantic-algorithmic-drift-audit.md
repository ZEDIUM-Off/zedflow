# Audit de drift sémantique et algorithmique Pi AI ↔ Zedflow AI

**Date :** 2026-07-13
**Périmètre :** `references/pi/packages/ai/` ↔ `crates/zedflow-ai/`
**Décision : NO-GO**

## 1. Verdict exécutif

Le port n'est pas fidèle à Pi au niveau comportemental ni architectural, malgré le passage de tous les gates Cargo.

Les drifts bloquants sont structurels :

1. **28 des 35 factories de providers construites via `static_provider()` gardent un dispatch qui retourne toujours `No Rust transport is available`,** alors que les modèles annoncent des API connues et que plusieurs transports existent déjà ailleurs dans le crate.
2. **Bedrock ignore le `Context` dans son entrée publique, exécute du HTTP bloquant dans un worker async, lit toute la réponse EventStream avant de produire le premier événement et ne propage pas l'annulation.**
3. **Codex SSE et WebSocket accumule la réponse complète avant tout événement public ; son `start` contient déjà le résultat final.** La voie directe omet aussi signal, hooks et politique de retry réellement exécutée.
4. **La transformation Pi commune des messages n'est pas le choke point du runtime Rust.** Anthropic, Bedrock et Mistral contournent la traduction fidèle isolée dans `api/transform-messages.rs`; plusieurs autres providers en maintiennent des copies privées divergentes.
5. **`Models::complete*()` peut deadlocker un runtime Tokio current-thread** et les workers fire-and-forget peuvent paniquer sans terminaliser le stream, laissant `result()` en attente infinie.
6. **Les événements Rust clonent un `AssistantMessage` croissant à chaque delta.** Le coût exact est la somme des tailles de préfixes : `O(n·b)` en général et `Θ(n·b)` si les fragments sont de taille comparable ; le backlog peut retenir jusqu'à `O(q·b)`, là où Pi conserve une seule instance mutable.

La correction stateful récente d'OpenAI Responses et Azure est réelle et fidèle : le processeur Rust conserve ses maps et traite chaque événement fournisseur une seule fois. Elle ne ferme toutefois ni le buffering Codex, ni les clones de snapshots, ni les autres drifts ci-dessus.

Les validations générales passent : **845 tests passés, 51 ignorés**. Cela ne change pas la décision : 15 lignes de manifest sont entièrement live/ignorées, 11 sont partielles et une ligne déterministe ne porte pas les assertions Pi correspondantes.

## 2. Méthodologie

1. Lecture intégrale des prérequis imposés :
   - `AGENTS.md` ;
   - `.agents/skills/rust-skills/SKILL.md` ;
   - les deux manifests ;
   - les trois rapports d'état demandés.
2. Inventaire de **148/148** couples source et **98/98** couples test ; vérification de présence des deux côtés.
3. Lecture Pi d'abord, puis suivi du flux Rust : factory provider → auth → options/hooks → transport → décodage → state machine → queue publique → terminal/result.
4. Recherches ciblées : `block_on`, runtimes/threads, clients bloquants, sleeps courts, queues, clones, `collect`, réponses lues en entier, JSON croissant, scans et dispatchs.
5. Comparaison mécanique des ensembles d'IDs extraits des fichiers `*.models.ts`/`*.models.rs` : aucune différence d'ID détectée ; comparaison séparée de la reconstruction des métadonnées.
6. Relecture des 98 couples de tests, avec comptage des tests, assertions et `#[ignore]`, puis inspection des assertions Pi et Rust.
7. Aucun appel réseau live. Les transports locaux existants ont été exécutés via la suite Cargo.
8. Aucun fichier de production modifié ; `zedflow-agent` n'a pas été modifié par cet audit.

### Définitions de complexité

- `n` : nombre d'événements/deltas fournisseur.
- `b` : volume final en octets du message ou du JSON accumulé.
- `q` : nombre d'événements publics en attente chez un consommateur lent.
- `k` : nombre de blocs de contenu simultanément suivis.
- `B` : taille en octets de la réponse HTTP/SSE/EventStream complète.
- `P`, `M` : nombres de providers et de modèles parcourus.

## 3. Matrice de couverture

### 3.1 Manifests

| Surface | Lignes | Examinées | Résultat |
|---|---:|---:|---|
| `ai-src.tsv` — `src/api` | 28 | 28 | Tous les flux API inventoriés ; transports principaux suivis de bout en bout. |
| `ai-src.tsv` — `src/auth` | 5 | 5 | Priorité, erreurs, stockage et résolution inspectés. |
| `ai-src.tsv` — `src/providers` | 75 | 75 | Ordre, factories, dispatch et catalogues inspectés ; IDs comparés mécaniquement. |
| `ai-src.tsv` — `src/utils` | 24 | 24 | Event stream, abort, retry, erreurs, OAuth, JSON, Unicode, validation inspectés. |
| `ai-src.tsv` — racine/core | 16 | 16 | Types, models, compat, images, OAuth et session resources inspectés. |
| **Total source** | **148** | **148** | **Présence 148/148 ; présence ≠ fidélité.** |
| `ai-tests.tsv` | 98 | 98 | Disposition complète ci-dessous. |

### 3.2 Sous-systèmes et providers obligatoires

| Sous-système | Pi suivi | Rust suivi | Conclusion |
|---|---|---|---|
| Anthropic | SDK async iterable → state de blocs → événements | voie enregistrée reqwest incrémentale ; autre `stream()` fixture-only | Streaming enregistré incrémental ; surface publique divisée et transformation incomplète. |
| OpenAI Completions | SDK stream single-pass | reqwest `bytes_stream` + state persistant | Incrémentalité présente ; retry et polling d'abort divergents. |
| OpenAI Responses | iterable unique → `processResponsesStream` | décodeur + `ResponsesStreamProcessor` persistant | **Correction single-pass validée.** Pas de replay de préfixe actuel. |
| Azure Responses | même processeur partagé | processeur persistant | **Correction single-pass validée.** Retry temporel divergent. |
| Codex SSE | `parseSSE` generator → processeur immédiat | `read_codex_sse() -> Vec` puis traitement | Buffering complet, cadence perdue. |
| Codex WebSocket | generator WS → processeur immédiat au premier événement | `Vec` complet dans runtime privé, puis traitement | Buffering complet, `start` tardif/final, runtime par requête. |
| Google Generative AI | SDK async iterable | reqwest SSE incrémental enregistré | Cadence enregistrée correcte ; snapshots clonés et surface publique parallèle. |
| Google Vertex | SDK async iterable | reqwest SSE incrémental enregistré | Même conclusion ; API key/ADC inspectés. |
| Mistral | SDK async iterable | voie enregistrée reqwest incrémentale ; voie publique parallèle bloquante | Voie enregistrée incrémentale ; transformation et surfaces divergentes. |
| Bedrock | AWS async EventStream | HTTP bloquant, `bytes()`, parse complet vers `Vec` | Drift fonctionnel, temporel, async et mémoire. |
| Faux | microtasks/timers et deltas | worker async, deltas, abort et usage | Adaptation globalement fidèle. |
| Images/OpenRouter | résultat structuré complet | API directe riche, adapter `ImagesModels` lossy | Drift de contrat et d'usage/coût. |
| Catalogues/dispatch | Map/factories reliées aux API | 28 factories gardent `unavailable_streams()` | Drift architectural bloquant. |

## 4. Findings classés par sévérité

### [BLOCKER] Vingt-huit providers intégrés annoncent des modèles mais dispatchent vers un transport toujours indisponible

- **Pi :** `references/pi/packages/ai/src/providers/all.ts:69-107` construit 35 providers ; chaque factory, par exemple `providers/openai-codex.ts:7-17` et `providers/amazon-bedrock.ts:27-34`, relie explicitement ses modèles à l'API réelle.
- **Rust :** `crates/zedflow-ai/src/providers/static_catalog.rs:12-28,254-297` installe `unavailable_streams()` ; 28 factories ne remplacent pas `provider.api` : amazon-bedrock, ant-ling, cerebras, cloudflare-ai-gateway, cloudflare-workers-ai, deepseek, google-vertex, groq, huggingface, kimi-coding, minimax, minimax-cn, moonshotai, moonshotai-cn, nvidia, openai-codex, opencode, opencode-go, openrouter, together, vercel-ai-gateway, xai, xiaomi, les trois xiaomi-token-plan, zai et zai-coding-cn. Exemple Codex : `providers/openai-codex.rs:23-30`. Les sept remplacements explicites sont Anthropic, Azure, Fireworks, GitHub Copilot, Google, Mistral et OpenAI.
- **Classification :** drift pur d'implémentation.
- **Cause racine :** reconstruction générique des providers séparée du registre des `ProviderStreams`; seules huit factories réassignent manuellement l'API.
- **Différence observable :** `builtin_models().stream*()` renvoie une erreur terminale locale pour des providers que Pi sait exécuter ; les modèles et credentials peuvent sembler valides mais aucune requête n'est faite.
- **Complexité Pi :** dispatch moyen `O(1)` via Map/provider API.
- **Complexité Rust :** dispatch `O(1)`, mais vers une erreur systématique ; le lookup préalable peut être `O(P+M)`.
- **Pourquoi les tests actuels ne l'ont pas détecté :** les tests de factories vérifient surtout id/name/catalogue ; les grandes matrices `stream`, handoff et providers live sont ignorées.
- **Correction de fond recommandée :** faire de la table API canonique l'unique source de `ProviderStreams` lors de `static_provider`, avec dispatch `ByApi` lorsque nécessaire ; supprimer `unavailable_streams` pour toute API implémentée, sans wrapper de compatibilité parallèle.
- **Tests de non-régression nécessaires :** pour chaque provider intégré, dispatch local sans réseau vers un transport capturé ; assertion que l'API choisie égale `model.api` et qu'aucun provider connu ne produit le message `No Rust transport`.
- **Unités/fichiers concernés :** `providers/static_catalog.rs`, les 28 factories listées, `providers/all.rs`, `models.rs`, tests `providers.rs`/`models-runtime.rs`.
- **Risque de la correction :** élevé : elle expose immédiatement les drifts de payload/auth actuellement masqués par l'erreur précoce.

### [BLOCKER] Bedrock perd le contexte et remplace l'EventStream incrémental par une réponse entièrement bufferisée

- **Pi :** `references/pi/packages/ai/src/api/bedrock-converse-stream.ts:220-247` sérialise messages, system et tools du `Context`; `:247-274` consomme `response.stream` événement par événement et émet `start`/deltas/end avant EOF.
- **Rust :** `crates/zedflow-ai/src/api/bedrock-converse-stream.rs:961-968` ignore `_context` et envoie `{ "messages": [] }`; `:1018-1095` exécute `reqwest::blocking::Client` dans une fonction async, appelle `response.bytes()`, parse tout vers `Vec<Value>`, puis `:996-1005` commence seulement le traitement public.
- **Classification :** drift pur d'implémentation et drift algorithmique.
- **Cause racine :** fallback HTTP conçu comme appel batch, non relié aux types canoniques ni à un décodeur AWS EventStream incrémental.
- **Différence observable :** prompts/system/tools perdus sur `stream()` ; aucun delta avant EOF ; annulation mid-stream absente ; un body long bloque un worker Tokio et augmente la latence/mémoire.
- **Complexité Pi :** temps `O(B)`, mémoire de transport `O(chunk + état)` hors queue consommateur.
- **Complexité Rust :** temps `O(B)`, mémoire `O(B+n+b)` avant la première émission ; blocage de l'exécuteur pendant l'I/O.
- **Pourquoi les tests actuels ne l'ont pas détecté :** les tests Bedrock couvrent conversion/payload/headers de seams séparés et deux cas live ignorés ; aucun serveur AWS EventStream local ne retarde EOF après un premier delta.
- **Correction de fond recommandée :** connecter le `Context` canonique au convertisseur Pi, utiliser reqwest async et un décodeur AWS EventStream stateful qui pousse chaque événement à la state machine au fil des chunks.
- **Tests de non-régression nécessaires :** capture exacte du payload avec user/assistant/tool-result/system/tools ; EventStream local en deux chunks prouvant un delta avant EOF ; abort après le premier delta ; erreur de frame partielle ; terminal unique.
- **Unités/fichiers concernés :** `api/bedrock-converse-stream.rs`, `api/bedrock-converse-stream.lazy.rs`, `providers/amazon-bedrock.rs`, tests Bedrock et `stream.rs`.
- **Risque de la correction :** élevé : parsing binaire AWS, SigV4, erreurs partielles et annulation doivent garder la même linéarisation terminale.

### [BLOCKER] La transformation commune Pi n'est pas le choke point des messages runtime Rust

- **Pi :** `api/transform-messages.ts:69-203` normalise contenu lax, replay même/autre modèle/provider, signatures, erreurs, résultats d'outils manquants et images ; elle est appelée par Anthropic, Bedrock, Google, Mistral et OpenAI avant leur conversion wire.
- **Rust :** un port proche existe dans `api/transform-messages.rs:345-508`, mais ses types dupliqués sont surtout consommés par les tests. Anthropic convertit directement à `anthropic-messages.rs:609-680`, Bedrock lit du JSON brut à `bedrock-converse-stream.rs:729-759`, Mistral ne fait qu'une normalisation d'IDs à `mistral-conversations.rs:935-949,1118-1161`; Google/OpenAI ont des copies privées.
- **Classification :** drift pur d'implémentation.
- **Cause racine :** plusieurs univers de messages/adapters provider-local au lieu d'une transformation canonique unique sur `crate::types::Message`.
- **Différence observable :** thinking/redacted/signatures étrangers peuvent être rejoués au lieu d'être supprimés/convertis ; tours error/aborted conservés ; résultats synthétiques `No result provided` manquants ; downgrade image et normalisation d'IDs non uniformes.
- **Complexité Pi :** transformation `O(m+c)` pour `m` messages et `c` blocs, plus maps d'IDs.
- **Complexité Rust :** plusieurs passes provider-local `O(m+c)` avec allocations répétées ; certaines voies perdent l'information avant de pouvoir appliquer les règles.
- **Pourquoi les tests actuels ne l'ont pas détecté :** `lax-message-content`, `tool-call-without-result` et `transform-messages-*` ciblent l'univers isolé, tandis que les handoffs runtime sont ignorés.
- **Correction de fond recommandée :** porter la logique fidèle sur les types canoniques, l'appeler exactement une fois avant chaque convertisseur wire, puis retirer les copies privées après captures de requêtes comparatives.
- **Tests de non-régression nécessaires :** matrice déterministe commune : même modèle, autre modèle même provider/API, autre provider/API, thinking signé/redacted/vide, IDs longs/pipe, résultat absent aux frontières, tours error/aborted, images supportées/non supportées et contenu null.
- **Unités/fichiers concernés :** `api/transform-messages.rs`, `types.rs`, tous les convertisseurs provider, tests de handoff/IDs/lax/tool-result.
- **Risque de la correction :** élevé : ordre des messages, associations tool-call/result et signatures de replay.

### [BLOCKER] Les APIs synchrones `Models` peuvent deadlocker Tokio

- **Pi :** `references/pi/packages/ai/src/models.ts:258-287` retourne immédiatement un lazy stream, attend auth/provider dans une Promise et expose `complete*()` async.
- **Rust :** `crates/zedflow-ai/src/models.rs:335-354` fait `block_on` de l'auth dans `stream*`; `:410-427` fait `block_on(stream.result())`. `utils/runtime.rs:6-8` spawn le worker sur le runtime Tokio courant.
- **Classification :** drift pur d'implémentation.
- **Cause racine :** façade synchrone imposée autour d'opérations intrinsèquement async, combinée à un spawn sur le runtime appelant.
- **Différence observable :** sur un runtime Tokio current-thread, le thread unique bloque en attendant une tâche qu'il vient lui-même de planifier ; auth avec timer/I/O peut aussi staller.
- **Complexité Pi :** attente coopérative `O(1)` en threads bloqués.
- **Complexité Rust :** attente potentiellement infinie ; un thread d'exécuteur immobilisé par appel.
- **Pourquoi les tests actuels ne l'ont pas détecté :** les tests utilisent surtout `futures::executor::block_on` hors runtime Tokio ou des futures immédiatement prêtes.
- **Correction de fond recommandée :** rendre auth/complete canoniquement async et déplacer l'auth dans le worker lazy qui terminalise le stream ; ne pas imbriquer de runtime ni masquer le deadlock par timeout.
- **Tests de non-régression nécessaires :** runtime current-thread avec auth différée et stream différé ; heartbeat concurrent ; `complete_async` termine sans blocage ; erreur auth devient un terminal unique.
- **Unités/fichiers concernés :** `models.rs`, `compat.rs`, `utils/runtime.rs`, tests `models-runtime.rs`.
- **Risque de la correction :** élevé : évolution d'API publique et propagation vers les appelants, mais nécessaire pour un contrat Rust idiomatique.

### [BLOCKER] Une panique de worker laisse `result()` en attente infinie

- **Pi :** les corps async de providers entourent les erreurs et poussent un terminal ; une rejection async est observable par le flux.
- **Rust :** les voies canoniques enregistrées passent majoritairement par `utils/runtime.rs:6-17`, où les `JoinHandle` sont abandonnés ; Codex utilise directement un `thread::spawn` également non supervisé à `openai-codex-responses.rs:1285-1287`. Dans les deux cas, les paniques ne sont pas converties. `utils/event-stream.rs:116-133` attend indéfiniment si aucun terminal/résultat n'arrive.
- **Classification :** drift pur d'implémentation.
- **Cause racine :** workers fire-and-forget sans supervision ni garde terminale RAII.
- **Différence observable :** itération et `result()` peuvent hang après panic d'un hook, parser ou invariant ; aucun `Error`, aucune fermeture.
- **Complexité Pi :** terminaison en temps borné par l'échec de la Promise.
- **Complexité Rust :** attente non bornée et ressources retenues.
- **Pourquoi les tests actuels ne l'ont pas détecté :** faux catch certaines paniques de factories lui-même ; aucun test ne panique le worker commun dans et hors Tokio.
- **Correction de fond recommandée :** superviser chaque worker et convertir panic/join failure en un seul terminal `Error`, avec garde qui ferme le stream si la tâche sort sans terminal.
- **Tests de non-régression nécessaires :** panic volontaire avant start, après delta et pendant hook ; terminal unique ; `result()` résolu ; aucun événement post-terminal.
- **Unités/fichiers concernés :** `utils/runtime.rs`, `utils/event-stream.rs`, tous les `stream_registered`.
- **Risque de la correction :** moyen : éviter double terminal lors des races panic/abort/done.

### [HIGH] Codex SSE et WebSocket n'émettent rien avant la fin de la réponse

- **Pi :** SSE passe `mapCodexEvents(parseSSE(...))` directement au processeur à `openai-codex-responses.ts:587-599`; WebSocket fait de même à `:1370-1431`, et pousse `start` au premier événement `:1360-1366`.
- **Rust :** SSE collecte dans `Vec` à `openai-codex-responses.rs:1437-1461`; WebSocket collecte à `:1659-1769`; `run_codex_live_worker` ne pousse `Start` et les événements qu'après retour complet à `:1291-1310`. Ce `Start` est construit depuis le message final.
- **Classification :** drift pur d'implémentation et drift algorithmique.
- **Cause racine :** transport retourne `(message, Vec<events>)` au lieu de conduire un processeur stateful attaché au stream public.
- **Différence observable :** aucun delta avant EOF/terminal ; impossibilité d'afficher ou d'annuler progressivement ; `partial` du `start` contient déjà la sortie finale ; erreurs tardives suppriment tous les deltas antérieurs.
- **Complexité Pi :** temps `O(B+n)`, mémoire transport `O(chunk + état)`.
- **Complexité Rust :** temps `O(B+n)` mais mémoire `O(B+n+b)` et latence première émission `Θ(durée totale)`.
- **Pourquoi les tests actuels ne l'ont pas détecté :** ils vérifient surtout événement final, résultat, cache/WS stats ; aucun serveur local ne bloque EOF après le premier delta observable.
- **Correction de fond recommandée :** utiliser un seul `ResponsesStreamProcessor` persistant pour SSE et WS, alimenté frame par frame, et pousser `start` lors du premier événement comme Pi.
- **Tests de non-régression nécessaires :** SSE et WS à deux barrières ; premier delta avant terminal/EOF ; erreur après delta conserve l'historique déjà émis ; un événement entrant traité une fois ; terminal unique.
- **Unités/fichiers concernés :** `api/openai-codex-responses.rs`, tests Codex SSE/WS/responseid.
- **Risque de la correction :** élevé : fallback WS→SSE, cache de session et frontière « started » dépendent du premier événement public.

### [HIGH] Les snapshots partiels clonés rendent les hot paths quadratiques en octets

- **Pi :** les événements portent la même référence mutable `output`, par exemple `openai-responses-shared.ts:389-509`, `anthropic-messages.ts:598-676`; la queue ne duplique pas tout le message.
- **Rust :** `types.rs:903-982` impose un `AssistantMessage` possédé dans chaque événement ; les providers font `partial: output.clone()` notamment dans `openai-responses-shared.rs:1273-1344`, `anthropic-messages.rs:1855-2036`, `bedrock-converse-stream.rs:1616-1923`, `mistral-conversations.rs:1328-1480` et `google-shared.rs:1072-1077`.
- **Classification :** drift algorithmique.
- **Cause racine :** traduction littérale de `partial` en valeur possédée plutôt qu'adaptation de la sémantique de référence partagée Pi.
- **Différence observable :** un consommateur lent voit des snapshots historiques immuables en Rust, alors que les références Pi reflètent les mutations ultérieures ; forte hausse CPU/mémoire.
- **Complexité Pi :** état `O(b)`, records d'événements `O(n)`, mémoire queue `O(q+b)` hors chaînes delta ; append dépend du moteur JS.
- **Complexité Rust :** somme des tailles de snapshots, donc `O(n·b)` en général et `Θ(n·b)` sous fragments de taille comparable ; mémoire backlog jusqu'à `O(q·b)`. À 4096 fragments de 64 octets, environ 512 MiB de copies cumulées ; à 16384 fragments de taille comparable et 1 MiB final, environ 8 GiB.
- **Pourquoi les tests actuels ne l'ont pas détecté :** ils comparent contenu/ordre final, pas compteurs de bytes clonés ni comportement d'événements retenus ; un test Google codifie même les snapshots Rust.
- **Correction de fond recommandée :** définir une représentation canonique unique de partial partagé/observé sans clone massif (état de stream partagé ou vue immutable bon marché), en conservant explicitement la sémantique observable Pi ; pas d'alias de compatibilité parallèle.
- **Tests de non-régression nécessaires :** 10/100/1000 deltas avec compteur déterministe de copies ; événements retenus puis lus après completion comparés à Pi ; slow consumer et high-water mémoire.
- **Unités/fichiers concernés :** `types.rs`, `utils/event-stream.rs`, tous les processeurs provider.
- **Risque de la correction :** élevé : changement du type public des événements, sérialisation et synchronisation.

### [HIGH] Annulation par polling, threads de sleep et transports non interruptibles

- **Pi :** `AbortSignal` notifie par listeners (`utils/abort-signals.ts:15-39`) et les SDK/fetch reçoivent le signal ; Mistral le passe dans request options, Codex à SSE/WS.
- **Rust :** six voies bouclent avec sleep 1 ms (`anthropic-messages.rs:971-974`, Google `:940-943`, Vertex `:1345-1348`, OpenAI Completions `:948-951`, Responses `:755-758`, Mistral `:2276-2279`), Azure à 5 ms (`:1112-1115`). OAuth crée un thread de sleep et sonde toutes les 50 ms (`utils/oauth/device-code.rs:251-309`). Codex direct n'a aucun champ `signal` (`openai-codex-responses.rs:497-530`) et Bedrock lit bloquant.
- **Classification :** drift pur d'implémentation.
- **Cause racine :** `AbortSignal` Rust possède déjà des listeners mais n'expose pas de future notifiée réutilisable aux transports.
- **Différence observable :** latence d'abort 1–50 ms minimum, timers/locks continus, thread par sleep OAuth, impossibilité d'arrêter Codex/Bedrock/Mistral direct pendant I/O bloquante.
- **Complexité Pi :** `O(1)` notification par abort.
- **Complexité Rust :** `O(t/δ)` réveils et locks pendant une attente de durée `t`; OAuth ajoute un thread par intervalle.
- **Pourquoi les tests actuels ne l'ont pas détecté :** ils valident surtout le terminal final et utilisent des délais courts ; aucun compteur de wakeups/threads ni transport réellement suspendu.
- **Correction de fond recommandée :** exposer une future cancellation-safe basée sur listener/waker et la sélectionner dans tous les transports ; supprimer polling et rendre toutes les I/O interruptibles.
- **Tests de non-régression nécessaires :** temps Tokio pausé sans wake périodique, compteur de polls, abort pendant headers/chunk/retry/hook, aucune frame après abort, race abort/terminal.
- **Unités/fichiers concernés :** `utils/abort-signals.rs`, `utils/oauth/device-code.rs`, API providers listées.
- **Risque de la correction :** moyen : désinscription des listeners, races et fuite de wakers.

### [HIGH] Les retries déclarés ne reproduisent pas la politique temporelle Pi

- **Pi :** OpenAI/Anthropic/Azure transmettent `maxRetries` aux SDK (`openai-completions.ts:189-197`, `openai-responses.ts:127-133`, `anthropic-messages.ts:534-540`); Codex implémente backoff, `Retry-After`, cap et sleep abortable à `openai-codex-responses.ts:347-418`.
- **Rust :** Responses enregistre `max_retries` mais fait un seul send (`openai-responses.rs:606-634`); Completions et Azure rebouclent immédiatement sans backoff (`openai-completions.rs:830-854`, `azure-openai-responses.rs:944-971`); Codex stocke le nombre mais `execute_codex_sse_live` envoie une fois (`:1362-1386`); Anthropic enregistré n'applique pas le champ.
- **Classification :** drift pur d'implémentation.
- **Cause racine :** options portées au niveau DTO sans moteur de retry transport fidèle.
- **Différence observable :** nombre d'essais erroné, rafales immédiates, `Retry-After` ignoré, cap non appliqué, abort pendant backoff impossible et hooks/cadence différents.
- **Complexité Pi :** `O(a)` requêtes espacées, mémoire `O(1)`, durée incluant backoff.
- **Complexité Rust :** `O(a)` requêtes immédiates pour certaines voies, `O(1)` pour d'autres malgré `a>0`.
- **Pourquoi les tests actuels ne l'ont pas détecté :** `openai-completions-retry` couvre classification/compteur mais pas horloge/backoff ; pas de suite locale Responses/Codex/Azure équivalente au SDK.
- **Correction de fond recommandée :** une politique async partagée, paramétrée seulement par les différences provider démontrées : statuts/erreurs retryables, `Retry-After`, cap, backoff et abort ; pas de boucle dupliquée par provider.
- **Tests de non-régression nécessaires :** horloge injectée, compte d'essais, 4xx non retryable, 429 terminal, `Retry-After` date/secondes/ms, cap, abort pendant attente et hook `onResponse`.
- **Unités/fichiers concernés :** APIs OpenAI/Azure/Codex/Anthropic, `utils/retry.rs`, tests retry.
- **Risque de la correction :** moyen : éviter replay de deltas après qu'un stream a commencé ; seuls les échecs pré-stream doivent être retentés.

### [HIGH] Les surfaces API parallèles n'ont pas la même sémantique que l'export Pi

- **Pi :** chaque module exporte un `stream` canonique live qui retourne immédiatement `AssistantMessageEventStream`, par exemple Anthropic `anthropic-messages.ts:468-546` et Mistral `mistral-conversations.ts:48-104`.
- **Rust :** plusieurs modules exposent à la fois `stream()` et `stream_registered()`/`stream_live()` avec types/comportements différents. Anthropic `stream()` exige une fixture raw SSE et sinon retourne `Unsupported` (`anthropic-messages.rs:1426-1446`) ; Google `stream()` construit une requête/fixture (`google-generative-ai.rs:602-634`) alors que la voie live est `stream_registered` (`:706-721`) ; Mistral direct retourne une `Result` synchrone et possède un autre worker (`mistral-conversations.rs:718-745,1972-1996`).
- **Classification :** drift pur d'implémentation architectural.
- **Cause racine :** seams déterministes promus en API publique au lieu de rendre le transport injectable derrière le contrat canonique.
- **Différence observable :** le même module peut throw/retourner `Err` synchronement, refuser le live ou produire une autre cadence selon la fonction choisie ; les tests peuvent valider une voie non utilisée par les providers.
- **Complexité Pi :** une state machine et une surface par API.
- **Complexité Rust :** deux implémentations/adapters à maintenir, avec conversions et tests dupliqués.
- **Pourquoi les tests actuels ne l'ont pas détecté :** beaucoup de tests importent les helpers directs/DTO ; les tests provider enregistrés sont plus rares ou live/ignorés.
- **Correction de fond recommandée :** garder une seule entrée canonique et injecter transport/clock/fixture sous cette entrée ; rendre les builders purs privés ou `pub(crate)`.
- **Tests de non-régression nécessaires :** même test contractuel appliqué à l'export public et au dispatch provider, identité des types/événements et erreurs setup terminalisées dans le stream.
- **Unités/fichiers concernés :** modules API `.rs`, modules `.lazy.rs`, `models.rs`, tests public API.
- **Risque de la correction :** moyen à élevé : réduction/changement d'API publique Rust.

### [HIGH] L'adapter `ImagesModels` détruit le résultat image canonique

- **Pi :** `references/pi/packages/ai/src/images-models.ts:176-215` retourne l'`AssistantImages` structuré du provider, avec output text/image, responseId, usage/coût et timestamp.
- **Rust :** `images-models.rs:22-33,90-105` définit des types minimaux parallèles ; `providers/openrouter-images.rs:111-140` aplatit les contenus en `Vec<String>` et perd response id, usage/coût et timestamp.
- **Classification :** drift pur d'implémentation.
- **Cause racine :** deuxième univers de types image entre API directe et collection de providers.
- **Différence observable :** l'appel direct riche et `builtin_images_models().generate_images()` ne renvoient pas les mêmes données ; accounting et provenance disparaissent.
- **Complexité Pi :** adaptation `O(o)` sans perte pour `o` outputs.
- **Complexité Rust :** `O(o)` avec allocations de data URLs et perte irréversible de champs.
- **Pourquoi les tests actuels ne l'ont pas détecté :** tests API directe et tests collection vérifient des contrats différents au lieu d'une identité de résultat.
- **Correction de fond recommandée :** réutiliser exclusivement `crate::types::{ImagesModel, ImagesOptions, AssistantImages}` dans provider collection et adapter ; supprimer les types minimaux parallèles.
- **Tests de non-régression nécessaires :** même fixture via API directe et `ImagesModels`, égalité structurelle complète, custom model non catalogué, usage/coût/cache et abort.
- **Unités/fichiers concernés :** `images-models.rs`, `providers/openrouter-images.rs`, `types.rs`, tests images.
- **Risque de la correction :** moyen : API publique et sérialisation.

### [MEDIUM] Contenu lax et signatures malformées divergent aux frontières canoniques

- **Pi :** `transform-messages.ts:69-74` transforme contenu null/undefined en liste vide ; `openai-responses-shared.ts:172-177` fait un `JSON.parse` strict de la signature reasoning.
- **Rust :** la normalisation null n'existe que dans les types isolés de `transform-messages.rs:170-227`, pas dans `types.rs:685-815`; `openai-responses-shared.rs:1102-1107` ignore silencieusement une signature reasoning non parseable.
- **Classification :** drift pur d'implémentation.
- **Cause racine :** validation/normalisation placée dans un univers non runtime et conversion OpenAI rendue tolérante sans décision documentée.
- **Différence observable :** anciennes sessions `content:null` peuvent échouer avant provider ; une signature corrompue est perdue silencieusement au lieu d'échouer comme Pi.
- **Complexité Pi :** `O(c)`.
- **Complexité Rust :** `O(c)` ; différence non asymptotique.
- **Pourquoi les tests actuels ne l'ont pas détecté :** `lax-message-content.rs` importe les types dupliqués ; aucun test ne désérialise un `Context` canonique puis ne capture une requête provider.
- **Correction de fond recommandée :** normaliser à l'ingestion canonique et rendre la conversion de signatures fallible avec propagation structurée.
- **Tests de non-régression nécessaires :** JSON de session null pour user/assistant/tool-result ; signature valide/legacy/malformée ; requête capturée ou erreur exacte.
- **Unités/fichiers concernés :** `types.rs`, `api/transform-messages.rs`, `api/openai-responses-shared.rs`, tests lax/replay.
- **Risque de la correction :** faible à moyen : compatibilité des anciennes sessions.

### [MEDIUM] OAuth device-code modifie les erreurs de poll et retarde l'annulation

- **Pi :** `utils/oauth/device-code.ts:46-97` propage directement la rejection de `poll()` et rend les sleeps abortables par listener.
- **Rust :** `utils/oauth/device-code.rs:175-177` enveloppe en `OAuthDeviceCodeFlowError::Poll`, ajoutant un préfixe ; `:251-309` sonde l'abort toutes les 50 ms dans un thread.
- **Classification :** drift pur d'implémentation ; l'usage de `Instant` pour la deadline est, séparément, une adaptation Rust fidèle.
- **Cause racine :** unification de toutes les erreurs sous un enum affiché et absence de future de notification AbortSignal.
- **Différence observable :** texte/source supérieur modifié ; annulation jusqu'à 50 ms plus tard et thread supplémentaire par sleep.
- **Complexité Pi :** notification `O(1)` sans thread bloqué.
- **Complexité Rust :** `O(t/50ms)` sondages et un thread de sleep.
- **Pourquoi les tests actuels ne l'ont pas détecté :** les tests couvrent cadence/slow_down/expiration mais pas identité exacte d'une erreur poll arbitraire ni compteur de polling.
- **Correction de fond recommandée :** préserver l'erreur source sans préfixe observable non-Pi et réutiliser la future abort notifiée commune.
- **Tests de non-régression nécessaires :** erreur sentinelle avec source/Display exacts ; abort immédiat sous horloge pausée ; aucun thread/sleep de polling.
- **Unités/fichiers concernés :** `utils/oauth/device-code.rs`, `utils/abort-signals.rs`, tests OAuth.
- **Risque de la correction :** faible.

## 5. Différences TS/Rust réellement intrinsèques

| Différence | Pourquoi intrinsèque | Équivalent Rust observable le plus proche |
|---|---|---|
| Observation Node des imports dynamiques (`registerHooks`) | Rust est lié statiquement et n'a pas le graphe de modules runtime Node. | Tester l'absence d'initialisation/side effect des providers non appelés ; les deux ignores correspondants sont justifiés. |
| Chaînes JS contenant des surrogates UTF-16 isolés | Un `String` Rust valide ne peut pas contenir un surrogate non apparié. | Valider/sanitiser les octets ou échappements JSON à la frontière session/FFI avant création du `String`. |
| `Record<string, unknown>` ouvert | Rust exige un type fermé ou une map explicite. | `StreamOptions.extra: HashMap<String, Value>` conserve les champs observables. |
| Deadline `Date.now()` vs `Instant` | JS utilise l'horloge murale ; Rust peut utiliser une horloge monotone. | `Instant` est plus robuste aux sauts d'horloge et fidèle à une durée d'expiration normale ; documenter le cas de changement d'horloge. |

La mutabilité partagée des objets `partial` Pi **n'est pas classée comme excuse intrinsèque** : Rust peut représenter une vue partagée sûre (`Arc` + synchronisation ou état de stream). Le choix actuel de cloner est donc un drift algorithmique, pas une fatalité de langage.

## 6. Adaptations Rust jugées fidèles

| Adaptation | Preuve d'équivalence |
|---|---|
| `VecDeque` pour la queue | `event-stream.rs:71,141` donne enqueue/dequeue amorti `O(1)` ; Pi conserve le même ordre mais `Array.shift()` peut déplacer les pointeurs. |
| Wakers Rust pour attente stream/result | `event-stream.rs:72-81,124-149` réveille sans polling et respecte l'ordre FIFO. |
| Linéarisation terminale sous mutex | `event-stream.rs:58-78` fixe le premier terminal, ignore les pushes suivants et réveille hors lock. |
| `ResponsesStreamProcessor` stateful | `openai-responses-shared.rs:1230-1415` conserve `output_slots`/tool calls et traite chaque événement une fois. |
| OpenAI/Azure transport single-pass | OpenAI `openai-responses.rs:661-705`, Azure `azure-openai-responses.rs:1004-1045` gardent un processeur par réponse. |
| `HashMap` pour slots Responses/Mistral | Équivalent aux `Map` Pi pour lookup moyen `O(1)` et évite une régression linéaire. |
| Erreurs Rust avec `source()` en plus du texte Pi | Les erreurs structurées conservent status/body/source sans modifier le message normalisé testé. |
| Hooks Rust `Result<Option<Value>, ProviderHookError>` | `None` correspond à `undefined`, `Some` remplace le payload, `Err` correspond à la rejection async ; fidèle sur les voies qui les `await`. |
| Deadline OAuth monotone | Même intervalle/slow_down/expiration sur entrées normales, sans sensibilité aux sauts de l'horloge murale. |
| Faux provider sur worker async | Ordre, pacing, usage/cache, abort et terminal sont couverts par 23 tests actifs et reproduisent les garanties Pi. |

## 7. Complexités Pi/Rust par hot path

| Hot path | `n`/volume | Pi temps | Rust temps | Pi mémoire | Rust mémoire | Impact/seuil |
|---|---|---:|---:|---:|---:|---|
| OpenAI/Azure Responses state transitions | `n` événements | `O(n)` hors JSON/snapshots | `O(n)` hors JSON/snapshots | `O(k+b)` | `O(k+b)` état | Correction single-pass fidèle. |
| Snapshots `partial` tous providers | `n`, final `b` | records `O(n)` ; une sortie partagée | somme des préfixes, `O(n·b)` ; `Θ(n·b)` si fragments comparables | `O(n+b)` | jusqu'à `O(q·b)` | Visible vers 1000 petits deltas ; ~512 MiB copiés à 4096×64 B comparables. |
| JSON tool-call croissant | `n` fragments, total `b` | `Θ(n·b)` parse | `Θ(n·b)` parse + `Θ(n·b)` snapshots | `O(b)` hors queue | `O(b)` état + snapshots | Test compteur dès 1000 deltas ou 256 KiB. Pas de `O(n³)` actuel prouvé. |
| Codex SSE/WS | réponse `B`, `n` événements | `O(B+n)`, première émission incrémentale | `O(B+n)`, première émission après fin | `O(chunk+b)` | `O(B+n+b)` | Visible dès une réponse lente >100–200 ms ; critique sur longues générations. |
| Bedrock EventStream | réponse `B` | `O(B)`, mémoire chunk | `O(B)`, body complet + Vec | `O(chunk+b)` | `O(B+n+b)` | Toute génération longue ; bloque aussi l'exécuteur. |
| Queue consommateur lent | `q` | enqueue `O(1)`, drain total potentiellement `O(q²)` via `shift` | enqueue/drain total `O(q)` | `O(q+b)` | `O(q·b)` avec snapshots | Rust améliore CPU de queue mais peut exploser en mémoire. |
| Anthropic lookup de bloc | `n`, `k` blocs | `Θ(n·k)` | `Θ(n·k)` | `O(k)` | `O(k)` | Fidèle mais plafond `O(n²)` si `k≈n`. |
| Catalogue chat | `P`,`M` | lookup moyen `O(1)` | `O(P+M)` et allocations via factories | `O(P+M)` catalogue | allocations temporaires | Visible dans boucles UI/lookup fréquentes ; indexer une fois. |
| Abort polling providers | attente `t`, pas `δ` | notification `O(1)` | `O(t/δ)` polls | `O(1)` | timers/wakes `O(t/δ)` | 1000 polls/s/attente pour δ=1 ms. |
| Worker hors Tokio | `r` requêtes | `O(r)` tasks event loop | `O(r)` threads + runtimes | tâches légères | stack/runtime par requête | Visible sous dizaines/centaines de requêtes concurrentes. |
| Erreur HTTP non bornée | body `E` | dépend SDK/normalizer | `O(E)` lecture/allocation | jusqu'à `O(E)` | jusqu'à `O(E)` | Prévoir cap déterministe, typiquement 1 MiB, sans perdre status/source. |

### Recherche explicite des anti-patterns

- **Replay de tout l'historique à chaque delta :** absent dans l'état actuel OpenAI/Azure ; occurrence `events.clone()` pertinente seulement en test batch/incrémental.
- **`Vec` accumulé avant émission :** présent et productif pour Codex et Bedrock.
- **Réponse streamée lue complètement :** Bedrock ; Codex accumule tous les événements.
- **JSON complet à chaque fragment :** présent pour tool JSON, comme Pi ; Rust ajoute le coût snapshot.
- **Clone d'un message partiel croissant :** présent dans toutes les familles principales.
- **Lookup linéaire vs Map :** slots OpenAI/Mistral utilisent bien des maps ; Anthropic reste linéaire comme Pi ; catalogues Rust régressent.
- **Runtime/client par événement :** aucun cas productif par événement trouvé.
- **Runtime/thread par requête :** présent dans fallback `spawn_worker` hors Tokio et Codex WS.
- **Accumulation non bornée :** queue publique des deux ports ; plus dangereuse en Rust à cause des snapshots.
- **`O(n³)` en octets :** non prouvé après la correction single-pass ; le pire chemin établi est `Θ(n·b)`, soit quadratique si `b=Θ(n)`.

## 8. Lacunes des tests actuels

### 8.1 Disposition complète des 98 lignes

- **F — port comportemental utile : 65**
  `abort`, `anthropic-adaptive-thinking-models`, `anthropic-cache-write-1h-cost`, `anthropic-eager-tool-input-compat`, `anthropic-empty-thinking-signature-compat`, `anthropic-force-adaptive-thinking`, `anthropic-oauth`, `anthropic-sse-parsing`, `anthropic-temperature-compat`, `anthropic-tool-name-normalization`, `azure-openai-base-url`, `bedrock-convert-messages`, `bedrock-custom-headers`, `bedrock-endpoint-resolution`, `cache-retention`, `compat-env`, `env-api-keys`, `error-body`, `faux-provider`, `fireworks-models`, `github-copilot-anthropic`, `github-copilot-oauth`, `google-shared-convert-tools`, `google-shared-gemini3-unsigned-tool-call`, `google-shared-image-tool-result-routing`, `google-thinking-disable`, `google-thinking-signature`, `google-vertex-api-key-resolution`, `images-models`, `images`, `lax-message-content`, `mistral-reasoning-mode`, `mistral-tool-schema`, `models-runtime`, `node-http-proxy`, `oauth-auth`, `oauth-device-code`, `openai-codex-cache-affinity-e2e`, `openai-codex-oauth`, `openai-codex-stream`, `openai-completions-cache-control-format`, `openai-completions-empty-tools`, `openai-completions-prompt-cache`, `openai-completions-reasoning-details`, `openai-completions-response-model`, `openai-completions-retry`, `openai-completions-thinking-as-text`, `openai-completions-tool-choice`, `openai-completions-tool-result-images`, `openai-responses-empty-tool-result`, `openai-responses-foreign-toolcall-id`, `openai-responses-message-id`, `openai-responses-partial-json-cleanup`, `openrouter-cache-write-repro`, `openrouter-images`, `overflow`, `provider-error-body-passthrough`, `provider-error-body-regression`, `providers`, `retry`, `supports-xhigh`, `together-models`, `transform-messages-copilot-openai-to-anthropic`, `validation`, `xiaomi-models`.

- **P — partiel/affaibli : 11**
  `anthropic-eager-tool-input-e2e`, `anthropic-long-cache-retention-e2e`, `anthropic-thinking-disable`, `bedrock-models`, `bedrock-thinking-payload`, `context-overflow`, `openai-responses-copilot-provider`, `openai-responses-tool-result-images`, `responseid`, `tool-call-id-normalization`, `tool-call-without-result`.

- **L — entièrement live/ignoré : 15**
  `anthropic-opus-4-8-smoke`, `cross-provider-handoff`, `empty`, `image-tool-result`, `interleaved-thinking`, `openai-responses-cache-affinity-e2e`, `openai-responses-reasoning-replay-e2e`, `scratch`, `stream`, `tokens`, `total-tokens`, `unicode-surrogate`, `xhigh`, `xiaomi-token-plan-ams-anthropic-empty-signature-smoke`, `zen`.

- **H — helpers/scripts Pi avec contrôles Rust ajoutés : 5**
  `azure-utils`, `bedrock-utils`, `cloudflare-utils`, `codex-websocket-cached-probe`, `oauth`.

- **J — divergence plateforme justifiée : 1**
  `lazy-module-load` pour deux observations Node `registerHooks` impossibles en Rust statique.

- **B — ligne déterministe non portée : 1**
  `openai-responses-terminal-event` : Pi vérifie cinq comportements déterministes à `test/openai-responses-terminal-event.test.ts:151-231`; Rust `tests/openai-responses-terminal-event.rs:27-49` ne vérifie que des options de requête.

**Total : 65 + 11 + 15 + 5 + 1 + 1 = 98.**

### 8.2 Preuves manquantes prioritaires

1. Delta observable avant EOF pour Codex SSE/WS et Bedrock.
2. Un compteur prouvant un seul traitement de chaque événement pour tous les processeurs stateful.
3. Compteur de bytes clonés et parsés sur 10/100/1000 deltas.
4. Panic worker → terminal/résultat, sans hang.
5. Runtime Tokio current-thread → aucun deadlock.
6. Dispatch des 35 providers → aucune API connue indisponible.
7. Replay canonique same model/other model/other provider sur les vraies voies enregistrées.
8. Slow consumer/backpressure/high-water queue.
9. Retry sous horloge injectée, pas benchmark temporel instable.
10. Race abort/done/error et suppression de tout événement post-terminal.
11. Bedrock EventStream partiel/malformé et erreur après delta.
12. Identité du résultat image API directe ↔ provider collection.

Les tests de résultat final seuls ne couvrent aucun de ces points.

## 9. Plan de correction ordonné par cause racine

1. **Reconnecter le graphe provider/API canonique.** Éliminer les 28 `unavailable_streams` pour API implémentées et faire échouer un test exhaustif de dispatch avant tout travail provider fin.
2. **Unifier les surfaces et types canoniques.** Une seule entrée stream par API, un seul univers messages/images/options/events ; seams injectables privés.
3. **Remplacer Bedrock et Codex batch par des pipelines stateful single-pass.** Décodeur incrémental + processeur persistant + émission immédiate ; aucune collecte complète.
4. **Rendre `Models` async-safe et superviser les workers.** Auth dans lazy worker, `complete_async`, terminal guard, conversion des panics/join failures.
5. **Faire de `transform_messages` le choke point canonique.** Puis retirer les copies provider seulement après captures wire comparatives.
6. **Rendre `AbortSignal` awaitable.** Supprimer tous les sleeps de polling et threads OAuth ; propager le signal aux I/O.
7. **Implémenter la politique de retry fidèle et partagée.** Seulement pré-stream, avec horloge injectée, backoff/headers/cap/abort.
8. **Supprimer les clones de partial croissants.** Choisir une représentation partagée unique fidèle à la sémantique Pi et mesurer les bytes, sans cache ou wrapper compensatoire.
9. **Unifier les types image.** Préserver output structuré, response id, usage/coût et timestamp.
10. **Fermer les tests déterministes et locaux avant live.** D'abord terminal-event, cadence, complexity counters, panic/deadlock, dispatch et handoffs capturés ; ensuite seulement la matrice live avec capacités.

## 10. Commandes de validation après correction

### Gates généraux

```bash
export CARGO_TARGET_DIR=/tmp/zedflow-ai-drift-audit-target
export TMPDIR=/tmp/zedflow-ai-drift-audit-tmp
mkdir -p "$CARGO_TARGET_DIR" "$TMPDIR"

cargo fmt --all --check
cargo check -p zedflow-ai --all-targets
cargo test -p zedflow-ai --all-targets
cargo doc -p zedflow-ai --no-deps
cargo clippy -p zedflow-ai --all-targets --no-deps -- -D warnings
git diff --check
```

### Gates ciblés à ajouter/exécuter

```bash
cargo test -p zedflow-ai --test stream-events
cargo test -p zedflow-ai --test openai-codex-stream
cargo test -p zedflow-ai --test openai-responses-terminal-event
cargo test -p zedflow-ai --test responseid
cargo test -p zedflow-ai --test models-runtime
cargo test -p zedflow-ai --test providers
cargo test -p zedflow-ai --test abort
cargo test -p zedflow-ai --test faux-provider
cargo test -p zedflow-ai --test images-models --test openrouter-images
```

Ajouter des targets déterministes dédiées : `stream-cadence`, `stream-complexity-counters`, `worker-panic-terminal`, `tokio-current-thread`, `bedrock-local-eventstream`, `codex-local-sse-ws`, `provider-dispatch-all`, `canonical-handoff-capture`.

### Résultats exécutés pendant cet audit

| Commande | Résultat |
|---|---|
| `cargo fmt --all --check` | PASS |
| `cargo check -p zedflow-ai --all-targets` | PASS |
| `cargo test -p zedflow-ai --all-targets` | PASS — 845 passés, 0 échec, 51 ignorés, 15 filtrés, 107 suites, 23,62 s |
| `cargo doc -p zedflow-ai --no-deps` | PASS |
| `cargo clippy -p zedflow-ai --all-targets --no-deps -- -D warnings` | PASS |
| `git diff --check` | PASS |

Logs locaux : `/tmp/zedflow-ai-drift-audit-logs/`.

## 11. Incertitudes et capacités live absentes

Aucun appel live n'a été effectué.

Absents dans l'environnement : Anthropic, OpenAI, Gemini/Google, Mistral, xAI, AWS/Bedrock, Azure OpenAI, OpenRouter, OpenCode, Xiaomi et GitHub Copilot via variables usuelles. `~/.pi/agent/oauth.json` est absent. `~/.pi/agent/auth.json` existe mais son contenu secret n'a pas été inspecté et n'a déclenché aucun appel.

Restent capability-gated : validation réelle des SDK/providers, compatibilité exacte des versions distantes, throttling serveur, service tiers facturés, AWS credential chain complète, cache provider réel, WS Codex distant et qualité multimodale.

Meilleurs substituts locaux : serveurs loopback HTTP/SSE/WS/EventStream, horloge injectée, fixtures de headers/status, compteurs de frames/bytes/polls, capture wire exacte et barrières contrôlant EOF.

Incertitude catalogue résiduelle : les ensembles d'IDs des fichiers `*.models` correspondent dans l'extraction mécanique et les tests catalogue passent, mais la reconstruction générique Rust perd des métadonnées non représentées par `CatalogModel`; toute nouvelle propriété Pi nécessite un test de champ, pas seulement un test d'ID.

## 12. Zones examinées sans drift détecté

- Correction single-pass OpenAI Responses et Azure : pas de replay de l'historique actuel.
- Ordre et terminalisation de base de `EventStream`; pushes post-terminal ignorés.
- `VecDeque` Rust améliore le coût de dequeue par rapport à `Array.shift()` sans changer l'ordre.
- Streaming enregistré Anthropic, Google, Vertex, OpenAI Completions, OpenAI Responses, Azure et Mistral : décodage incrémental présent sur leurs voies canoniques respectives.
- Faux provider : ordre des blocs, pacing, cache/session, usage/coût, factories async/fallibles et abort.
- Priorité auth explicite → credential/OAuth → environnement, et absence de fallback après erreur credential.
- Sémantique canonique des hooks lorsqu'ils sont réellement `await` : remplacement optionnel et propagation d'erreur structurée.
- Normalisation d'erreurs status/body JSON/raw, truncation et sources sur les voies testées.
- Ordre des 35 providers intégré.
- Ensembles d'IDs des catalogues `*.models` dans l'extraction mécanique.
- Maps de slots OpenAI Responses et tool calls Mistral : lookup moyen `O(1)`.
- Mapping usage/coût canonique incluant cache read/write, reasoning et cache write 1h dans les unités actives.
- Service-tier OpenAI/Codex : propagation et multiplicateurs présents dans les processeurs.
- Routage image tool-result Google/OpenAI sur les convertisseurs privés inspectés.
- Nettoyage de `partialJson` final OpenAI Responses.
- Absence de runtime/client reconstruit par événement.
- Absence de chemin productif `O(n³)` établi après la correction stateful.

## Décision finale

**NO-GO** : plusieurs drifts purs et algorithmiques sont incompatibles avec l'objectif de fidélité Pi, dont dispatch provider non fonctionnel, perte de contexte et buffering Bedrock, buffering Codex, transformation runtime non centralisée, deadlock/panic non terminalisée et copies quadratiques des partials.

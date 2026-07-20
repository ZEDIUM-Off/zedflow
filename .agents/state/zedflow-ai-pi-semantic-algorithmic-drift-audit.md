# Audit de drift sémantique et algorithmique Pi AI ↔ Zedflow AI

**Date initiale :** 2026-07-13
**Dernière reprise :** 2026-07-16
**Périmètre :** `references/pi/packages/ai/` ↔ `crates/zedflow-ai/`
**Décision : NO-GO**

## 1. Verdict exécutif

Le port n'est pas fidèle à Pi au niveau comportemental ni architectural, malgré le passage de tous les gates Cargo.

Les drifts bloquants sont structurels :

1. **Le dispatch des 28 factories concernées est maintenant reconnecté aux transports canoniques fonctionnels.** OpenAI Codex et Amazon Bedrock sont reliés à leurs transports réels.
2. **Bedrock utilise désormais le SDK AWS Rust officiel.** Son entrée builtin transforme le `Context` canonique, construit messages/system/tools/cache/thinking/metadata, consomme l'EventStream SDK incrémentalement et propage l'annulation.
3. **Codex SSE et WebSocket est désormais single-pass et incrémental sur la voie builtin.** Restent non fidèles : hooks absents, annulation incapable d'interrompre une I/O bloquée et réutilisation réelle des sockets WS non portée.
4. **La transformation Pi commune est désormais le choke point des neuf transports canoniques.** Les copies privées restantes appartiennent aux anciennes surfaces provider parallèles encore à supprimer.
5. **`Models` est désormais async-safe et les workers sont supervisés**, y compris panic, sortie sans terminal et Codex bloquant.
6. **Les événements portent maintenant un handle `partial` partagé fidèle à Pi**, ce qui corrige le backlog mémoire. Plusieurs state machines provider-local construisent toutefois encore un snapshot complet avant de mettre à jour ce handle : le coût CPU `Θ(n·b)` reste à supprimer avec leurs surfaces parallèles.

Les corrections stateful OpenAI Responses, Azure et Codex sont réelles : chaque transport conserve un processeur persistant et traite chaque événement fournisseur une seule fois. Elles ne ferment toutefois ni les clones de snapshots, ni les autres drifts ci-dessus.

Les validations générales passent après reprise : **853 tests passés, 50 ignorés**. Cela ne change pas la décision : 15 lignes de manifest sont entièrement live/ignorées, 11 sont partielles et une ligne déterministe ne porte pas les assertions Pi correspondantes.

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
7. Audit initial sans appel réseau live ; la reprise et son incident OpenRouter sont consignés en section 11.
8. La reprise modifie `zedflow-ai` et les seuls appelants `zedflow-agent` nécessaires à la migration async/stream.

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
| Codex SSE | `parseSSE` generator → processeur immédiat | parsing SSE incrémental → `ResponsesStreamProcessor` persistant | **Cadence et terminal avant EOF validés localement.** |
| Codex WebSocket | generator WS → processeur immédiat au premier événement | frames réassemblées → même processeur persistant | **Émission incrémentale et frontière `start` alignées ; runtime par requête résiduel.** |
| Google Generative AI | SDK async iterable | reqwest SSE incrémental enregistré | Cadence enregistrée correcte ; snapshots clonés et surface publique parallèle. |
| Google Vertex | SDK async iterable | reqwest SSE incrémental enregistré | Même conclusion ; API key/ADC inspectés. |
| Mistral | SDK async iterable | voie enregistrée reqwest incrémentale ; voie publique parallèle bloquante | Voie enregistrée incrémentale ; transformation et surfaces divergentes. |
| Bedrock | AWS async EventStream | SDK AWS Rust officiel, `EventReceiver::recv()` incrémental, state persistant | **Contexte builtin, cadence, abort, hooks et coût reconnectés.** |
| Faux | microtasks/timers et deltas | worker async, deltas, abort et usage | Adaptation globalement fidèle. |
| Images/OpenRouter | résultat structuré complet | mêmes types canoniques et conversion lossless | **Parité structurelle corrigée.** |
| Catalogues/dispatch | Map/factories reliées aux API | 28 factories reconnectées, Bedrock inclus | Dispatch builtin complet pour les APIs prêtes. |

## 4. Findings classés par sévérité

### [CORRIGÉ LE 2026-07-16] Dispatch builtin Bedrock

- **Pi :** `references/pi/packages/ai/src/providers/all.ts:69-107` construit 35 providers et chaque factory relie ses modèles à l'API réelle.
- **Rust initial :** 28 factories construites par `static_provider()` recevaient `unavailable_streams()` même lorsque leur API avait déjà un transport canonique.
- **Correction du 2026-07-16 :** `static_catalog.rs` résout au même choke point les sept APIs déjà prêtes puis `openai-codex-responses`. Les 27 factories correspondantes utilisent leur transport réel, en `Single` ou `ByApi`, sans modification factory par factory. `compat.rs` réutilise la même table.
- **Correction du 2026-07-16 (Bedrock) :** `bedrock-converse-stream.lazy.rs` retourne directement `provider_streams()` et `static_catalog.rs` résout `bedrock-converse-stream`; l'override global et la réponse d'indisponibilité ont été supprimés.
- **Classification :** drift d'implémentation corrigé.
- **Complexité :** lookup/dispatch moyen `O(1)` comme Pi ; pas de registre global, cache ou wrapper ajouté.
- **Tests :** parcours exhaustif des modèles builtin et résolution explicite de Bedrock passent sans appel live.

### [CORRIGÉ LE 2026-07-16] Bedrock canonique et EventStream incrémental

- **Pi :** `references/pi/packages/ai/src/api/bedrock-converse-stream.ts:220-247` sérialise messages, system et tools du `Context`; `:247-274` consomme `response.stream` événement par événement et émet `start`/deltas/end avant EOF.
- **Rust corrigé :** la voie builtin appelle `transform_context`, construit le payload complet et le convertit vers les types Smithy. `aws-config` fournit région/profile/credential chain, le SDK signe les headers injectés et `EventReceiver::recv()` alimente une state machine persistante événement par événement.
- **Suppression :** sender reqwest bloquant, SigV4/HMAC, lecture de profils et parser AWS EventStream manuels supprimés.
- **Abort/hooks/coût :** l'annulation concurrence l'envoi et chaque `recv`; `onPayload` précède la construction Smithy, `onResponse` reçoit les métadonnées SDK, et les coûts utilisent le catalogue canonique.
- **Limitation SDK documentée :** le connecteur public `aws-smithy-http-client` est compatible HTTP/1 mais active aussi l'ALPN HTTP/2 et n'expose aucun switch public HTTP/1-only. Les chemins custom endpoint/proxy utilisent ce connecteur officiel ; aucune pile HTTP parallèle n'est ajoutée.
- **Complexité :** temps `O(B)`, mémoire de transport `O(chunk + état)` hors queue consommateur, comme Pi.
- **Validation déterministe :** suites Bedrock/providers, test du tool config Smithy, state incrémental/coût, check et clippy passent sans credential ni réseau live.

### [CORRIGÉ LE 2026-07-16] Transformation commune canonique

`api/transform-messages.rs` travaille désormais directement sur `crate::types`, normalise replay/signatures/IDs/images/tool-results manquants et `content:null`, puis `transform_context` est appelé exactement une fois par les neuf transports canoniques. Les anciens tests ont été migrés sur les types runtime. Les transformations privées encore présentes relèvent du chantier de suppression des surfaces provider parallèles ci-dessous, pas d'un contournement de l'entrée canonique.

### [CORRIGÉ LE 2026-07-16] `Models` async-safe

`refresh`, `get_auth`, `complete` et `complete_simple` sont async sous leurs noms canoniques. `stream*()` retourne immédiatement puis résout auth et dispatch dans un worker lazy. Les wrappers `block_on` et doublons `_async` ont été supprimés ; les appelants de compaction `zedflow-agent` ont été migrés. Un test Tokio current-thread avec auth différée prouve l'absence de deadlock.

### [CORRIGÉ LE 2026-07-16] Supervision des workers

Tous les producteurs canoniques, Codex bloquant inclus, passent par le superviseur commun. Panique, erreur de runtime ou sortie sans terminal deviennent un unique `Error`; un terminal déjà produit gagne la race et une panique tardive conserve le dernier partial. Les cas panic avant/après delta, sortie vide et done-puis-panic sont couverts.

### [CORRIGÉ LE 2026-07-16] Codex SSE et WebSocket émettent maintenant en single-pass

- **Pi :** SSE pousse `start` après le HTTP 2xx puis traite `parseSSE` immédiatement ; WebSocket pousse `start` au premier événement normalisé et interdit ensuite le fallback SSE.
- **Rust corrigé :** un seul `CodexLiveResponseProcessor` réutilise `ResponsesStreamProcessor` pour SSE et WS. Chaque événement normalisé est traité une fois et ses événements publics sont drainés immédiatement ; aucun `Vec` de réponse complète ne subsiste.
- **Frontières conservées :** SSE pousse `start` après succès HTTP ; WS au premier événement normalisé ; aucun fallback après ce point ; `response.completed/incomplete` termine avant EOF ; une erreur tardive conserve les deltas déjà émis.
- **Protocole :** les événements SSE multi-lignes sont assemblés ; les messages WebSocket fragmentés sont réassemblés avec une limite de 16 MiB ; erreurs API/protocole WS distinguées des erreurs de transport.
- **Options/runtime :** le provider builtin convertit modèle, contexte, outils, messages, reasoning, transport, auth OAuth, timeouts, retry et coût vers les types canoniques. Les retries pré-stream couvrent réseau, statuts Pi, backoff, `retry-after-ms` et `Retry-After` numérique.
- **Complexité actuelle :** temps `O(B+n)`, mémoire transport `O(chunk + état)` hors snapshots publics, comme Pi.
- **Tests locaux :** retry 503 puis delta observable avant EOF, terminal observable avant EOF, réassemblage WS fragmenté, response id, dispatch builtin et erreur auth déterministe.
- **Résiduel :** hooks, interruption immédiate des I/O bloquées et cache réel de connexions WebSocket.

### [HIGH — PARTIELLEMENT CORRIGÉ] Partials partagés, producteurs locaux encore coûteux

`SharedAssistantMessage(Arc<Mutex<AssistantMessage>>)` porte maintenant tous les champs `partial`. Le stream canonicalise un handle unique : les événements retenus observent le message final et la queue passe à `O(q+b)`. Faux et le proxy mutent directement ce handle ; `zedflow-agent` ne snapshotte qu’aux frontières possédées. Les tests couvrent identité `ptr_eq`, observation finale, serde et ordre terminal.

Le résiduel reste bloquant pour la CPU : plusieurs state machines provider-local construisent encore un snapshot complet avant que le stream le fusionne (`Anthropic`, Bedrock, Google, Mistral, OpenAI). Leur mémoire backlog est corrigée, pas la somme de copies `Θ(n·b)`. La suppression des surfaces/DTO parallèles doit migrer ces state machines directement sur le handle canonique puis retirer leurs événements locaux.

### [CORRIGÉ EN MAJEURE PARTIE LE 2026-07-16] Annulation notifiée

`AbortSignal::cancelled()` est listener-based, race-free et cancellation-safe. Les polls 1/5 ms des providers et le thread OAuth 50 ms sont supprimés ; OAuth et les transports async sélectionnent directement I/O/timer contre le signal. Résiduel isolé : la voie Codex bloquante ne peut interrompre un `read_line` déjà engagé et son WS garde un runtime privé ; ce résiduel disparaîtra avec la suppression de sa surface/transport parallèle.

### [CORRIGÉ LE 2026-07-16] Retries pré-stream

Les briques communes de parsing `Retry-After` (ms, secondes, date HTTP), backoff/cap et attente abortable vivent dans `utils/retry.rs`. Anthropic, OpenAI Completions/Responses, Azure, OpenRouter Images et Codex les utilisent avant le premier événement uniquement ; `onResponse` est appelé pour chaque réponse et aucune génération commencée n'est rejouée.

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

### [CORRIGÉ LE 2026-07-16] Types image canoniques sans perte

`ImagesModels` utilise exclusivement `crate::types`. L'adapter OpenRouter conserve output text/image structuré, response id, usage/coût/cache, stop reason et timestamp, y compris pour un modèle custom non catalogué. Les types minimaux et l'aplatissement data-URL ont été supprimés.

### [MEDIUM — PARTIELLEMENT CORRIGÉ] Signature reasoning OpenAI malformée

La normalisation `content:null` est désormais portée par les types canoniques et testée. Reste un drift : `openai-responses-shared.rs` ignore une `thinking_signature` JSON malformée alors que Pi propage l'échec de `JSON.parse`. La conversion doit devenir fallible et terminaliser le stream avec l'erreur structurée correspondante.

### [CORRIGÉ LE 2026-07-16] OAuth device-code

Le polling conserve la source et le `Display` exact sans préfixe observable. Le sleep utilise `futures_timer` sélectionné contre `AbortSignal::cancelled()`, sans thread ni polling périodique ; `Instant` reste la deadline monotone fidèle.

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
| Codex SSE/WS | réponse `B`, `n` événements | `O(B+n)`, première émission incrémentale | `O(B+n)`, première émission incrémentale | `O(chunk+b)` | `O(chunk+b)` état hors snapshots | Correction single-pass validée localement. |
| Bedrock EventStream | réponse `B` | `O(B)`, mémoire chunk | `O(B)`, body complet + Vec | `O(chunk+b)` | `O(B+n+b)` | Toute génération longue ; bloque aussi l'exécuteur. |
| Queue consommateur lent | `q` | enqueue `O(1)`, drain total potentiellement `O(q²)` via `shift` | enqueue/drain total `O(q)` | `O(q+b)` | `O(q·b)` avec snapshots | Rust améliore CPU de queue mais peut exploser en mémoire. |
| Anthropic lookup de bloc | `n`, `k` blocs | `Θ(n·k)` | `Θ(n·k)` | `O(k)` | `O(k)` | Fidèle mais plafond `O(n²)` si `k≈n`. |
| Catalogue chat | `P`,`M` | lookup moyen `O(1)` | `O(P+M)` et allocations via factories | `O(P+M)` catalogue | allocations temporaires | Visible dans boucles UI/lookup fréquentes ; indexer une fois. |
| Abort polling providers | attente `t`, pas `δ` | notification `O(1)` | `O(t/δ)` polls | `O(1)` | timers/wakes `O(t/δ)` | 1000 polls/s/attente pour δ=1 ms. |
| Worker hors Tokio | `r` requêtes | `O(r)` tasks event loop | `O(r)` threads + runtimes | tâches légères | stack/runtime par requête | Visible sous dizaines/centaines de requêtes concurrentes. |
| Erreur HTTP non bornée | body `E` | dépend SDK/normalizer | `O(E)` lecture/allocation | jusqu'à `O(E)` | jusqu'à `O(E)` | Prévoir cap déterministe, typiquement 1 MiB, sans perdre status/source. |

### Recherche explicite des anti-patterns

- **Replay de tout l'historique à chaque delta :** absent dans l'état actuel OpenAI/Azure ; occurrence `events.clone()` pertinente seulement en test batch/incrémental.
- **`Vec` accumulé avant émission :** reste présent et productif pour Bedrock ; supprimé pour Codex.
- **Réponse streamée lue complètement :** Bedrock ; Codex s'arrête désormais au terminal avant EOF.
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

1. Delta observable avant EOF pour Bedrock et WebSocket Codex loopback complet ; SSE Codex est maintenant couvert.
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

1. **Reconnecter le graphe provider/API canonique — PARTIEL.** Huit APIs et 27 factories sont reconnectées, Codex inclus ; seule l'entrée Bedrock reste.
2. **Unifier les surfaces et types canoniques.** Une seule entrée stream par API, un seul univers messages/images/options/events ; seams injectables privés.
3. **Remplacer les pipelines batch — PARTIEL.** Codex est single-pass ; Bedrock doit encore recevoir un décodeur incrémental, un processeur persistant et une émission immédiate.
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

Logs locaux initiaux : `/tmp/zedflow-ai-drift-audit-logs/`.

### Validation de la reprise du 2026-07-16

| Commande | Résultat |
|---|---|
| `cargo fmt --all --check` | PASS |
| `cargo check -p zedflow-ai --all-targets` | PASS |
| `cargo test -p zedflow-ai openai_codex_responses --lib` | PASS — 12 passés |
| `cargo test -p zedflow-ai --test openai-codex-stream` | PASS — 22 passés |
| `cargo test -p zedflow-ai --test responseid` | PASS — 7 passés, 6 ignorés |
| `cargo test -p zedflow-ai --test providers` | PASS — 14 passés |
| `cargo test -p zedflow-ai --all-targets` avec capacités live isolées | PASS — 849 passés, 0 échec, 51 ignorés, 107 suites |
| `cargo doc -p zedflow-ai --no-deps` | PASS |
| `cargo clippy -p zedflow-ai --all-targets --no-deps -- -D warnings` | PASS |
| `git diff --check` | PASS |

Log complet Codex : `/tmp/zedflow-ai-codex-tests-20260716.log`.

## 11. Incertitudes et capacités live absentes

L'audit initial n'avait effectué aucun appel live. Pendant la reprise, un premier `cargo test --all-targets` a détecté la capacité OpenRouter locale et lancé les trois tests image live : deux ont passé, le cas image→image n'a renvoyé aucune image. Aucun secret n'a été lu ou affiché. La validation déterministe a ensuite été rejouée avec un `HOME` isolé et les variables provider retirées ; elle passe intégralement. Cet échec live, extérieur au diff de dispatch chat, reste à reproduire séparément avant toute conclusion sur l'adapter image.

Absents de la validation déterministe : Anthropic, OpenAI, Gemini/Google, Mistral, xAI, AWS/Bedrock, Azure OpenAI, OpenRouter, OpenCode, Xiaomi et GitHub Copilot. `~/.pi/agent/oauth.json` n'a pas été utilisé et le contenu secret de `~/.pi/agent/auth.json` n'a pas été inspecté.

Restent capability-gated : validation réelle des SDK/providers, compatibilité exacte des versions distantes, throttling serveur, service tiers facturés, AWS credential chain complète, cache provider réel, WS Codex distant et qualité multimodale.

Meilleurs substituts locaux : serveurs loopback HTTP/SSE/WS/EventStream, horloge injectée, fixtures de headers/status, compteurs de frames/bytes/polls, capture wire exacte et barrières contrôlant EOF.

Incertitude catalogue résiduelle : les ensembles d'IDs des fichiers `*.models` correspondent dans l'extraction mécanique et les tests catalogue passent, mais la reconstruction générique Rust perd des métadonnées non représentées par `CatalogModel`; toute nouvelle propriété Pi nécessite un test de champ, pas seulement un test d'ID.

## 12. Zones examinées sans drift détecté

- Correction single-pass OpenAI Responses et Azure : pas de replay de l'historique actuel.
- Ordre et terminalisation de base de `EventStream`; pushes post-terminal ignorés.
- `VecDeque` Rust améliore le coût de dequeue par rapport à `Array.shift()` sans changer l'ordre.
- Streaming enregistré Anthropic, Google, Vertex, OpenAI Completions, OpenAI Responses, Azure, Mistral et Codex : décodage incrémental présent sur leurs voies canoniques respectives.
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

**NO-GO** : dispatch, Bedrock, Codex, transformation canonique, Models async, supervision, retries, OAuth et images sont corrigés. Restent deux causes structurelles liées : surfaces/DTO provider parallèles encore publiques et snapshots `partial` quadratiques. Restent aussi le parsing strict des signatures reasoning OpenAI et le résiduel transport Codex bloquant/WS cache, à fermer pendant cette consolidation finale.

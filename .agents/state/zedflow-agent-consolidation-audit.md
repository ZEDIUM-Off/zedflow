<!-- migration-document-status: SUPPORTING BASELINE -->
> [!NOTE]
> **Migration status: SUPPORTING BASELINE.** Retained as an input/evidence snapshot; its counts and verdict are not live status. See `.agents/state/zedflow-ai-agent-pi-fidelity-current-status.md`.

# Audit de consolidation de `zedflow-agent`

Date : 2026-07-10

Références principales :

- Pi : `references/pi/packages/agent`
- Rust : `crates/zedflow-agent`
- Contrats IA : `crates/zedflow-ai`
- Plan du port agent : `.agents/plans/zedflow-agent-pi-agent-port.md`
- Plan global : `.agents/plans/pi-to-rust-package-port.md`
- Règles Rust : `.agents/skills/rust-skills/SKILL.md`

## Verdict

**No-go pour la vague officielle suivante du port global.** La crate compile, documente une large surface et représente tous les fichiers manifestés, mais elle n'est pas encore un port d'identité fidèle de Pi. Les écarts bloquants concernent précisément les flux asynchrones, les erreurs d'outils/hooks, la durabilité du harness et quelques contrats publics qui changeront probablement pendant la consolidation.

Le rapport précédent `.agents/state/zedflow-agent-pi-agent-port-final-report.md` reste exact sur la représentation des manifests et les gates Cargo, mais sa conclusion « ready for next wave » est invalidée par cet audit comportemental.

## Synthèse

| Axe | État | Conclusion |
|---|---|---|
| Représentation des sources | 25/25 cibles | complète au niveau fichiers, pas au niveau comportement |
| Représentation des tests | 20/20 cibles | complète au niveau fichiers |
| Fidélité des tests | 9 complètes, 6 partielles, 3 trompeuses, 2 placeholders | insuffisante pour accepter le port |
| Compatibilité `zedflow-ai` | compilation et identité des types principales : oui | adaptation async/erreurs : partielle |
| Qualité Rust | fmt/check/test/doc passent | `clippy -D warnings` échoue ; problèmes async/concurrence/API |
| Makefile | ajouté et validé | parité des scripts Pi couverte ; couverture nécessite `cargo-llvm-cov` |

## Écarts bloquants de fidélité et de flow

### B1 — Le flux public bas niveau n'est pas vivant

- Pi retourne immédiatement un `EventStream` puis exécute la boucle en arrière-plan : `references/pi/packages/agent/src/agent-loop.ts:37-53,64-92`.
- Rust appelle `futures::executor::block_on` avant de retourner : `crates/zedflow-agent/src/agent-loop.rs:62-82,90-111`.

Conséquences : les événements sont déjà tamponnés à la réception du stream, l'abort après obtention du stream perd son utilité, et un appel depuis un runtime peut bloquer ou imbriquer un executor. Le helper async `run_agent_loop` est meilleur, mais il ne répare pas la fidélité de l'API publique `agent_loop`.

### B2 — Les erreurs récupérables d'outils et de hooks ne sont pas représentables

- Pi exige `execute`, capture ses exceptions et les convertit en résultats d'outil `isError: true` : `references/pi/packages/agent/src/types.ts:373-390`, `references/pi/packages/agent/src/agent-loop.ts:610-648`.
- Rust rend `execute` optionnel et lui fait retourner directement `AgentToolResult` : `crates/zedflow-agent/src/types.rs:119-158`.
- Rust marque ensuite toute résolution du future comme succès : `crates/zedflow-agent/src/agent-loop.rs:807-874`.
- Les hooks `prepare_arguments`, `before_tool_call` et `after_tool_call` n'ont pas non plus de canal `Result`, alors que Pi capture les exceptions de préparation/finalisation : `references/pi/packages/agent/src/agent-loop.ts:549-618,650-714`.

Conséquences : un outil Rust doit paniquer ou encoder manuellement une erreur dans un résultat nominalement réussi ; `is_error` devient faux et le flow Pi n'est pas préservé.

### B3 — La mutation des arguments par `beforeToolCall` est perdue

- Pi transmet le même objet d'arguments validé au hook puis à `execute`; le cas est testé explicitement : `references/pi/packages/agent/test/agent-loop.test.ts:310-370`.
- Rust passe un clone au hook puis exécute `validated_args` inchangé : `crates/zedflow-agent/src/agent-loop.rs:733-790`.

Rust doit exposer un remplacement explicite des arguments plutôt que compter sur une mutation de clone.

### B4 — Le harness avale des erreurs de session, d'événements et de hooks

- L'adaptateur d'événements ignore `handle_agent_event` : `crates/zedflow-agent/src/harness/agent-harness.rs:1250-1261`.
- `finish_phase` ignore l'échec du flush final : `crates/zedflow-agent/src/harness/agent-harness.rs:915-922`.
- Le save point Rust n'effectue pas le flush ordonné avant son émission : `crates/zedflow-agent/src/harness/agent-harness.rs:1370-1435`.
- Plusieurs hooks de contexte/outils/provider sont ramenés silencieusement à leur valeur par défaut : `crates/zedflow-agent/src/harness/agent-harness.rs:1065-1188,1203-1247`.
- Pi propage ou normalise ces erreurs et flush avant le save point/turn suivant : `references/pi/packages/agent/src/harness/agent-harness.ts:436-503,586-604`.

Conséquences : un prompt peut paraître réussi après un échec de persistance ; les écritures différées peuvent ne pas être visibles au tour suivant ; un listener fautif peut être invisible à l'appelant.

### B5 — `AgentHarness::wait_for_idle` est un no-op

- Pi maintient `runPromise` et l'attend : `references/pi/packages/agent/src/harness/agent-harness.ts:303-311,999-1001`.
- Rust documente et implémente un no-op : `crates/zedflow-agent/src/harness/agent-harness.rs:895-896`.

Ceci est distinct de `Agent::wait_for_idle`, qui possède bien des waiters. Si `prompt` tourne dans une tâche séparée, le harness retourne immédiatement au lieu d'attendre le run, les listeners terminaux et les writes.

### B6 — L'admission concurrente de `Agent::prompt` n'est pas atomique

Le test d'activité et l'installation du contrôleur utilisent des acquisitions séparées : `crates/zedflow-agent/src/agent.rs:391-405,606-619`. Deux appels concurrents peuvent observer l'état idle. Pi installe `activeRun` synchroniquement avant le premier `await` : `references/pi/packages/agent/src/agent.ts:337-345,469-481`.

## Écarts matériels non bloquants isolément

| Écart | Pi | Rust | Impact |
|---|---|---|---|
| Effacement d'options stream | patch absent/set/clear et suppression par clé | `Option<T>` ne distingue pas absent de clear pour les scalaires/maps | timeout/metadata peuvent rester actifs ; `harness/types.rs:110-181` |
| Provenance de compaction | `fromHook` persisté et émis | toujours `None` / `false` | résumé hook traité comme résumé modèle ; `agent-harness.rs:480-508` |
| `StreamFn` agent | stream immédiat **ou Promise de stream** | alias direct du `StreamFunction` synchrone de `zedflow-ai` | setup async custom impossible ; `types.rs:20` |
| `prepareArguments` non-objet | la valeur transformée est validée/échoue | retour silencieux aux arguments originaux | exécution différente de Pi ; `agent-loop.rs:713-729` |
| Proxy | client HTTP/SSE complet | parsing/reconstruction uniquement | placeholder documenté ; `proxy.rs:7-10` |
| UUID session | UUIDv7 monotone | UUIDv4 | placeholder documenté ; `session/uuid.rs:3-10` |
| Process kill | arbre de processus | enfant direct | placeholder documenté ; `env/nodejs.rs` |
| Leaf invalide | setter fallible | setter non fallible | test ignoré ; `harness/types.rs:1013` |

## Compatibilité avec `zedflow-ai`

### Correct

- Les types `Model`, `Message`, `AssistantMessage`, `ToolCall`, `Tool`, `Context`, `SimpleStreamOptions` et `AssistantMessageEventStream` sont réexportés/aliasés depuis `zedflow-ai` : `crates/zedflow-agent/src/types.rs:8-23`.
- La validation d'arguments appelle `zedflow_ai::validate_tool_arguments`.
- `cargo check -p zedflow-agent --all-targets` passe avec le `zedflow-ai` actuel.
- Les callbacks utilisent `Arc`, `Send + Sync` et des futures `Send`.

### À consolider

- Le problème n'est pas une duplication générale des types IA, mais l'adaptation comportementale : stream pré-calculé, `block_on` dans le stream du harness, erreurs de hooks supprimées.
- `StreamFn` est plus étroit que le contrat Pi agent, qui accepte aussi `Promise<AssistantMessageEventStream>`.
- Les hooks provider du harness sont adaptés par `block_on` ou par `.ok().flatten()`, ce qui peut deadlocker et supprime les erreurs : `agent-harness.rs:1203-1247`.
- `AgentToolResultContent` reste une union locale proche des blocs `zedflow-ai`; ce n'est pas bloquant, mais la conversion manuelle doit rester cohérente.
- Dépendances Cargo sans usage direct détecté : `zedflow-core`, `zedflow-tools`, `zedflow-session`, `jsonschema`. Elles doivent être retirées ou justifiées après consolidation.

## Fidélité des tests

Les 20 lignes du manifest existent, mais leur qualité n'est pas homogène.

| Test Pi → Rust | Fidélité | Observation principale |
|---|---|---|
| `agent-loop.test.ts` → `agent-loop.rs` | partielle | mutation `beforeToolCall` et erreurs récupérables absentes |
| `agent.test.ts` → `agent.rs` | complète | bons flows d'état/queues/subscribers ; ajouter la course concurrente Rust |
| `e2e.test.ts` → `e2e.rs` | partielle | abort pendant streaming ignoré |
| `agent-harness-stream.test.ts` → homonyme | trompeuse | Rust conserve timeout/metadata là où Pi les efface ; snapshot save-point absent |
| `agent-harness.test.ts` → homonyme | partielle | wait/abort, erreurs hooks, flush/save-point et queues incomplets |
| `compaction.test.ts` → homonyme | partielle | permutations de contexte/reasoning/error incomplètes ; test live vide ignoré |
| `nodejs-env.test.ts` → homonyme | trompeuse | abort pré-déclenché au lieu d'annuler une commande active ; WSL/cleanup incomplets |
| `prompt-templates.test.ts` | complète | cas principaux préservés |
| `repo.test.ts` | complète | mémoire et JSONL couverts |
| `resource-formatting.test.ts` | complète | assertions équivalentes |
| `session-test-utils.ts` | complète (fixture) | helper représenté et utilisé |
| `session-uuid.test.ts` | placeholder | UUIDv7 entièrement ignoré |
| `session.test.ts` | complète | scénarios mémoire/JSONL couverts |
| `skills.test.ts` | complète | cas Pi et filtrage additionnel |
| `storage.test.ts` | partielle | leaf invalide ignoré |
| `system-prompt.test.ts` | complète | assertions équivalentes |
| `truncate.test.ts` | partielle acceptée | lone UTF-16 surrogate non représentable par `str` |
| `scratch/simple.ts` | placeholder | live/credentials correctement ignoré |
| `utils/calculate.ts` | complète | métadonnées, exécution et invalides couverts |
| `utils/get-current-time.ts` | trompeuse | timezones non UTC rejetées au lieu d'être prises en charge |

Total : **9 complètes, 6 partielles, 3 trompeuses, 2 placeholders**.

La suite actuelle passe avec **115 tests actifs et 6 ignorés**, mais elle ne prouve pas les flows bloquants ci-dessus. Les gates de consolidation doivent ajouter des fakes retardés/fautifs, sans réseau.

## Qualité Rust selon `rust-skills`

### Points positifs

- `#![forbid(unsafe_code)]`.
- APIs publiques largement documentées et `cargo doc` passe avec `RUSTDOCFLAGS=-D warnings`.
- Usage cohérent de `Result` sur de nombreuses frontières session/env/harness.
- Types IA réutilisés plutôt que dupliqués.
- Locks généralement relâchés avant les callbacks et suite de tests large.

### Écarts importants

- `err-result-over-panic` / `err-source-chain` : callbacks outils/hooks sans `Result`; plusieurs erreurs stockent seulement `Option<String>` comme cause.
- `async-*` : `block_on` dans les APIs stream ; `std::fs`/`std::process` derrière des traits async ; canal d'updates non borné.
- `conc-*` : admission de prompt non atomique ; `clippy` signale aussi `await_holding_lock` dans `handle_run_failure` malgré un `drop(state)` explicite, à éliminer par portée lexicale.
- `err-no-unwrap-prod` : plusieurs backends session font `expect()` sur les mutex empoisonnés.
- `doc-errors-section` : les traits publics fallibles n'ont pas tous une section `# Errors`.
- `mem-box-large-variant` : plusieurs grandes enums (`AgentEvent`, événements harness, entrées session) déclenchent `large_enum_variant`.
- `proj-mod-by-feature` : fichiers TypeScript-style avec `#[path]` et module inception (`session::session`, `compaction::compaction`).
- Warnings tests : retours d'unsubscribe `FnOnce`/`#[must_use]` ignorés et helpers morts.

`cargo clippy -p zedflow-agent --all-targets --no-deps -- -D warnings` échoue avec **36 diagnostics `zedflow-agent`**. Beaucoup sont mécaniques, mais async/concurrence/API doivent être corrigés avant de considérer la qualité consolidée. `zedflow-ai` émet en plus 26 warnings de compilation ; ils ne sont pas tous imputables à cette crate.

## Makefile

Fichier : `crates/zedflow-agent/Makefile`.

| Script Pi | Cible Make | Équivalent Rust |
|---|---|---|
| `clean` | `make clean` | `cargo clean -p zedflow-agent` |
| `build` | `make build` | `cargo build -p zedflow-agent` |
| `test` | `make test` | tous les targets de la crate |
| `test:harness` | `make test:harness` | tous les wrappers de tests harness |
| `coverage:harness` | `make coverage:harness` | texte + HTML + LCOV via `cargo-llvm-cov` |
| `prepublishOnly` | `make prepublishOnly` | `clean` puis `build` |

Cibles Rust additionnelles : `fmt`, `check`, `doc`, `package`. La cible par défaut construit au lieu de nettoyer.

Validation :

- parsing/dry-run de toutes les cibles : passé ;
- `make test:harness` : passé ;
- `make package` : passé avec `--allow-dirty` pour cette branche de port non commitée ;
- `make coverage:harness` : garde correcte, non exécutée car `cargo-llvm-cov` n'est pas installé.

## Gates exécutées

Toutes les compilations ont utilisé `/tmp/zedflow-agent-consolidation-target`.

| Gate | Résultat |
|---|---|
| `make fmt` | passé |
| `make check` | passé avec warnings |
| `make test` | passé : 115 actifs, 6 ignorés |
| `make test:harness` | passé |
| `make doc` | passé ; warnings de dépendance `zedflow-ai` |
| `make package` | passé |
| `make coverage:harness` | non disponible : `cargo-llvm-cov` absent |
| `cargo clippy ... --no-deps -- -D warnings` | échoué : 36 diagnostics crate |

Aucun test live/réseau/browser n'a été lancé.

## Gates minimales de consolidation

1. `agent_loop` et `agent_loop_continue` retournent avant la fin d'un faux provider retardé ; les événements sont observables incrémentalement.
2. `execute`, `before_tool_call`, `after_tool_call` et la préparation ont un canal d'erreur récupérable ; les erreurs deviennent des tool results Pi-compatibles.
3. Une modification d'arguments par le hook atteint réellement l'exécution via un contrat Rust explicite.
4. Les erreurs de subscribers/hooks/session sont propagées ; les writes sont flushés en ordre au save point et un write échoué n'est ni perdu ni déclaré réussi.
5. `AgentHarness::wait_for_idle` attend le run, les événements terminaux et la persistance.
6. L'admission de `Agent::prompt` est atomique et testée avec deux appels concurrents.
7. Les patches stream représentent absent/set/clear, y compris effacement de map et suppression par clé.
8. `from_hook` est persisté et émis pour les compactions fournies par hook.
9. Tests déterministes dédiés à chaque gate, puis fmt/check/test/doc et clippy crate propre.

## Avis pour le plan global

Ne pas marquer P2 (`packages/agent`) accepté et ne pas lancer officiellement W4/P3 comme si l'API agent était stable. Les corrections B1-B6 touchent des contrats que `packages/coding-agent` consommera ; les repousser créerait des adaptations puis une seconde migration.

Une reconnaissance indépendante du TUI peut continuer si elle ne dépend pas de ces APIs, mais la **vague officielle suivante doit attendre la consolidation de `zedflow-agent`**.

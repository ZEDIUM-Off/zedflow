# Validation du produit Rust/web

## Gates de la migration 0.2.0

Les preuves courantes sont dans `.agents/output/migration/P7.2.md` et les logs
sous `.agents/output/migration/p72/`. Un contrôle en échec ou non exécuté reste
signalé ; les totaux historiques ci-dessous ne qualifient pas le diff actuel.
CI Linux est configurée, pas déclarée exécutée à distance. Aucun modèle réel,
aucun poids téléchargé et aucune activation personnelle ne font partie des gates.

Depuis la racine du dépôt :

```sh
pnpm --dir web install --frozen-lockfile
pnpm --dir web build
pnpm --dir web typecheck
pnpm --dir web test
pnpm --dir e2e install --frozen-lockfile
PLAYWRIGHT_SKIP_BROWSER_GC=1 pnpm --dir e2e exec playwright install chromium
ZEDFLOW_E2E_STATIC=1 pnpm --dir e2e test
pnpm --dir tooling/releases test
```

Le premier build produit les exports publics `dist` du SDK et des adaptateurs Vue
avant leur consommation par les typechecks, y compris depuis un clone sans artefacts.

Cargo doit partir de `rust/` pour sélectionner la toolchain 1.96.1, même lorsqu'un
`--manifest-path` absolu pourrait trouver le manifeste depuis ailleurs :

```sh
cd rust
export CARGO_TARGET_DIR=/tmp/zedflow-adk-target
cargo fmt --all --check
cargo check --locked --workspace --all-targets
ZEDFLOW_TEST_CODEGEN=1 cargo test --locked --workspace --all-targets
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo check --locked --workspace --all-targets --all-features
```

Le SDK teste un consommateur construit hors workspace (Node, types et bundle
navigateur) ; ses imports ne dépendent pas du TS du client. `portable_export`
construit avec `--locked` un export déplacé, sans accès au checkout, puis reprend
sa fixture sans répéter les effets. Les dépendances Cargo déjà en cache sont permises.
L'all-features conserve le catalogue ADK et compile l'inférence CPU, sans appel modèle.

Après installation verrouillée, le téléchargement du binaire Electron doit être
explicite : `pnpm --dir web --filter @zedflow/desktop exec node node_modules/electron/install.js`
(pnpm bloque ses scripts implicites). Electron utilise `pnpm --dir web --filter @zedflow/desktop test:smoke`, avec
`ZEDFLOW_CLIENT_URL` vers un daemon statique dédié et un affichage de test
(`xvfb-run --auto-servernum` sous Linux sans affichage). Le daemon E2E exige
`ZEDFLOW_E2E_ROOT` temporaire et peut utiliser `ZEDFLOW_E2E_DAEMON_PORT=3167` ;
voir `.github/workflows/ci.yml` pour le cycle démarrage/health/arrêt. Ne jamais
récupérer une instance personnelle ni remplacer HOME/CODEX_HOME. L'E2E accepte
`CHROMIUM_PATH` si un Chromium système est explicitement choisi.

L’inspection d’un run sans nœud ni occurrence désigne désormais la définition
racine et le graphe **initiaux**, figés à l’admission, pas le dernier passage.
L’export Cargo conserve ce même pin. Les passages sélectionnés explicitement
retrouvent leurs définitions et graphes historiques, y compris après adoption ;
un mélange de révisions est refusé. Le cache initial est lié aux références de
contenu et à l’origine d’import du run ; les lectures de nœud « latest » restent
non réutilisables sans occurrence ni hash.

Le ledger nominatif conserve les sources, empreintes, destinations et adaptations.
Les tests lab retirés sont distingués des tests produit conservés. Les écarts de
migration ne sont pas résolus en retirant des assertions ou en ignorant des suites.

## Archives de validation avant migration

Les sections suivantes sont des observations datées de l'ancien arbre. Leurs
commandes, chemins retirés et résultats sont historiques, pas des instructions
pour le produit 0.2.0. Les liens vers les tests conservés suivent leur nouveau propriétaire.

### Reboot du laboratoire

Validation locale réalisée le 9 septembre 2026 sur Linux aarch64, avec Rust/Cargo
1.96.1. Les builds utilisent `/tmp/zedflow-adk-target`. La CI reprend les contrôles
Rust sur Ubuntu ; son exécution distante n'a pas été lancée pendant le reboot.

| Contrôle | Résultat |
|---|---|
| `cargo fetch --locked` | Sources récupérées ; 42 bibliothèques ADK fixées à 2.2.0 dans le manifeste et le lockfile |
| `cargo fmt --all --check` | Réussi |
| `cargo check --locked --workspace --all-targets --all-features` | Réussi, y compris plateforme ADK et `adk-mistralrs` sur CPU |
| `cargo test --locked --workspace --all-targets` | 6 tests réussis |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | Réussi |
| Launcher `list`, `research`, `agent`, `memory` | Exécution réussie, résultats JSONL vérifiés |
| Launcher `checkpoint start`, puis `resume` | Deux processus distincts ; état restauré, préparation exécutée une seule fois |
| Entrées CLI incorrectes | Requête vide, commande inconnue, arguments live incomplets et thread déjà terminé refusés |
| `cargo-adk` 2.2.0 | Installé dans le répertoire dédié ; aide de la sous-commande vérifiée |

Les tests couvrent : rejet d'une requête de recherche vide, retour de l'agent
après recherche avec isolation des canaux privés, mémoire partagée entre graphes
avec isolation du projet, reprise SQLite, puis les chemins CLI mémoire et reprise
dans de nouveaux processus.

## Inspecteur web

Ajout validé le 9 septembre 2026 : 10 tests réussis avec `--features web`
(les 6 existants et 4 tests HTTP). Les contrôles formatage, compilation de toutes
les features et Clippy passent. Les 6 tests et Clippy sans feature web passent aussi.

Vérification dans Chromium via CDP : affichage du graphe compilé, exécution de la
boucle agentique, sélection du nœud de recherche avec surlignage de sa source,
lecture de la chronologie sans déplacer la page, mémoire isolée entre deux
périmètres, pause puis reprise SQLite avec `preparations: 1`. Le guide web historique (retiré du produit) précise
la provenance des événements et les limites de la visualisation.

## Observations utiles

### Passage à ADK Studio — 10 septembre 2026

Studio upstream 1.0.1 est installé, son UI embarquée est servie sur le réseau privé
et le projet natif `Zedflow Studio Recherche` est chargé depuis le dépôt.
Vérification dans Chromium : canvas et propriétés, Build Successful, lancement
manuel, événements `cadrer → sources → reponse`, résultat interpolé avec la question
fournie et timeline de trois étapes réussies. Modification d'une description dans
les propriétés du canvas, puis vérification de sa sauvegarde dans le JSON du dépôt.
Aucune erreur JavaScript observée.
Le binaire généré a également été exécuté directement avec une autre question ;
il termine avec le résultat attendu et un code de sortie 0.

La procédure et les trois particularités upstream rencontrées (entrée `START`,
clé d'état `message`, configuration fournisseur exigée pour des actions seules)
sont documentées dans guide Studio historique (retiré du produit). Le launcher passe `sh -n` ; aucune
crate du workspace Rust n'a été modifiée. Le prototype web précédent est arrêté.

La compilation complète a nécessité le pin `get-size2` et D-Bus embarqué décrits
dans [adk.md](adk.md). Aucun fork du code upstream n'a été introduit.

La démonstration mémoire utilise le mot séparé `graph`. Lors du test initial,
la recherche de `graph` ne retrouvait pas la note contenant `graph:` : la recherche
de ce backend mémoire ne doit pas être interprétée comme un moteur sémantique
robuste. Le test CLI vérifie maintenant que la fixture choisie produit effectivement
un résultat dans le même projet et aucun dans l'autre.

Le stream du graphe agentique rend visible le passage `decide → research → decide`.
Les détails internes de recherche se voient en exécutant ce flow seul ; le lab ne
prétend pas fournir la traçabilité hiérarchique complète de la future composition.
ADK peut également ajouter des canaux internes de fan-in à l'état retourné.

## Limites de cette validation

Le mode Gemini réel est compilé mais aucun appel payant n'a été lancé. Les modèles
et recherches fixture vérifient l'exécution, pas la qualité d'une réponse réelle.
La compilation des crates ne valide pas l'accès aux clouds, navigateurs, bases
externes, appareils audio ou modèles locaux. Aucun poids n'a été téléchargé.

Le registre, la résolution par workspace/PWD, les bundles distribuables, les contrats
de scopes et l'évolution automatique sont documentés comme intentions, pas présentés
comme des fonctions implémentées ou validées.

## Harness de workspace — 11 septembre 2026

Le périmètre du premier harness est décrit dans [le guide de l'application](app.md).
L'audit de livraison a confronté le plan accepté aux implémentations, aux tests et
au client rendu. Les vérifications restent locales sur Linux aarch64 ; aucun
appel à un fournisseur réel ni publication distante n'a été effectué.

| Exigence du plan | Preuves dans le dépôt |
|---|---|
| Boucle explicite ADK, un outil par passage, erreurs rendues au modèle | [Tests API](../rust/crates/zf-serve/tests/harness_api.rs) : lecture/écriture/édition/commande réelles, réparation après fichier absent ; [tests du graphe](../rust/crates/zf-serve/tests/runtime_graph.rs) : résultats outils, frontières de steering et checkpoints |
| Modèles fixes/configurables, choix tardif, changement au prochain appel et trace effective | [Tests modèles](../rust/crates/zf-runtime/src/models.rs) : appel simulé suspendu pendant un changement, trace before/low puis after/high, modèle fixe inchangé ; API : attente contextualisée et modification concurrente refusée |
| Sélecteur de flow avec recherche et définition, réglages par nœud, navigation synchronisée | [Tests navigateur harness](../e2e/specs/harness.spec.ts) : recherche, ouverture de définition, brouillon indépendant du run, nœuds imbriqués, recentrage explicite après déplacement du graphe |
| Steering après le lot, follow-up après réponse, messages persistants et retirables | API : deux vrais appels shell, steering envoyé pendant le premier, consommation après le second, follow-up après la réponse ; identités, annulation et consommation unique depuis deux clients |
| Outils avec droits de l'utilisateur et cwd distinct, aperçus bornés, arrêt/timeout | [Tests outils](../rust/crates/zf-runtime/src/workspace_tools.rs) : chemins extérieurs et liens, édition après lecture, 180 Ko UTF-8 sans retour à la ligne, 2 000 lignes/50 Kio, fichier complet, stdout/stderr partiels, arrêt d'enfants et petits-enfants, annulation avant spawn |
| Instructions et skills progressifs avec provenance et contenu figé | [Tests contexte](../rust/crates/zf-runtime/src/workspace_context.rs) : hiérarchie, collisions, priorités, invocation explicite et catalogue sans corps ; API : skill mis en file puis source modifiée et daemon recréé, contenu envoyé conservé |
| Reprise durable et absence de répétition automatique d'effets incertains | [Tests runtime](../rust/crates/zf-runtime/src/runtime.rs) : reçus commencés/réussis/échoués, concurrence, remplacement des canaux ; API : arrêt/reprise, réponse acceptée avant crash, acquittements de messages imbriqués |
| Sous-graphes à deux niveaux, nouvelles occurrences distinctes et attente précise | Tests graphe/API : état enfant préservé après redémarrage, effet antérieur non répété, deux passages distincts, réponse identique avec nouvelle identité ; attentes portant chemin et occurrence |
| Refus explicite des attentes parallèles non prises en charge | Tests graphe : refus des branches indépendantes, y compris imbriquées et avec concurrence limitée à un nœud ; alternatives conditionnelles et jonctions `all` avant attente commune acceptées |
| Export Rust partageant les primitives, workspace et modèles à l'exécution | Tests d'export réellement activés : boucle modèle/outils équivalente, reprise de modèle imbriqué dans un nouveau processus sans répéter l'effet |
| Continuité du client et compatibilité du POC | [Tests navigateur](../tests/e2e) : WebRTC, SSE, récupération HTTP, acquittement obsolète, parcours existants ; [tests paramètres](../rust/crates/zf-serve/tests/compiler_parameters.rs) : anciennes compositions, reducers, politiques et interpolation |

Contrôles finaux après les corrections de l'audit :

| Commande/contrôle | Résultat |
|---|---|
| `cargo fmt --all --check` | Réussi |
| `cargo check --locked --workspace --all-targets` | Réussi |
| `ZEDFLOW_TEST_CODEGEN=1 cargo test --locked --workspace --all-targets` | 86 tests réussis, aucun échec ni test ignoré ; compilation et exécution des projets générés activées |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | Réussi |
| `cargo check --locked --workspace --all-targets --all-features` | Réussi |
| `pnpm typecheck` et `pnpm build` | Réussis ; avertissement Vite sur la taille des bundles conservé |
| Suite Playwright complète | 15 scénarios réussis sur un daemon isolé, avec les assets construits |

Les commandes Cargo utilisent `CARGO_TARGET_DIR=/tmp/zedflow-adk-target`. Le
navigateur teste un daemon sur le port 3158, dans un workspace et un répertoire
de données temporaires, sans utiliser le daemon utilisateur. Les parcours de
sélection et d'intervention simulés sont complétés par les tests API réels ; le
[parcours intégré](../e2e/specs/harness-integration.spec.ts) effectue deux lectures
réelles d'AGENTS.md via le graphe. Une [capture du harness](../output/app-preview/09-harness-live.png)
montre ce résultat.

Les limites intentionnelles restent celles du guide : pas de compaction,
capacités de réflexion exposées seulement lorsqu'intégrées au fournisseur,
pas de garantie exactement-une-fois pour un effet externe inconnu et pas de
prise en charge des attentes simultanées indépendantes. Les tests de fixtures
et de transports simulés ne prouvent pas l'accès d'un compte à un modèle réel.

## Workspaces, fichiers Rust et espaces de travail — 11 septembre 2026

La refonte décrite dans [le guide de l'application](app.md) et le
[format Rust structuré v1](flow-format.md) est validée sur Linux aarch64.

| Contrôle | Résultat |
|---|---|
| `pnpm typecheck` | Réussi |
| `pnpm build` | Réussi ; avertissement sur la taille de certains bundles |
| `CHROMIUM_PATH=/snap/bin/chromium pnpm test:e2e` | 21 scénarios réussis, aucun échec |
| `cargo fmt --all --check` | Réussi |
| `cargo check --locked --workspace --all-targets` | Réussi |
| `ZEDFLOW_TEST_CODEGEN=1 cargo test --locked --workspace --all-targets` | 108 tests réussis, aucun échec ni test ignoré |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | Réussi |
| `cargo check --locked --workspace --all-targets --all-features` | Réussi |

Les commandes Cargo utilisent `CARGO_TARGET_DIR=/tmp/zedflow-adk-target`. La suite
navigateur crée un daemon dédié sur `3157`, un client Vite sur `5177`, deux
workspaces, un répertoire de données et une racine de flows globale sous un
répertoire temporaire unique. Les serveurs sont arrêtés après la suite. Aucun
flow global personnel ni fournisseur LLM réel n'est utilisé par ces scénarios.

Les [tests de workspaces](../rust/crates/zf-serve/tests/workspace_flows.rs) vérifient les
quatre racines, les conflits de hash, les fichiers invalides ou spéciaux, la
migration SQLite sauvegardée et idempotente, et la reprise de sessions utilisant
leurs fichiers et instructions propres. L'export d'une session conserve la source
initiale après modification du flow ; les tests navigateur vérifient aussi tous
les chemins et octets du ZIP après suppression du fichier d'origine.

Les tests de `flow_source` couvrent le catalogue complet, les sous-graphes, canaux,
conditions, retries et modèles. Ils compilent le fichier Rust exact exporté et
exécutent un harness imbriqué avec sélection de modèle puis attente et reprise,
sans rejouer l'effet préalable. Les tests de timeline vérifient l'ordre commun,
la mise à jour des outils au même emplacement, les messages consommés et les
reconnexions. Le rendu navigateur couvre titres, listes, liens, tableaux et code.

Les courses de chargement reproduites dans le navigateur sont couvertes par des
réponses volontairement retardées : catalogue initial devant un nouveau brouillon,
résolution du chemin dans le navigateur de dossiers, et acquittement réseau
arrivant après un état plus récent. La suite vérifie aussi la réouverture d'un
workspace masqué, le popover, l'inspecteur et les écrans étroits.
La bibliothèque remplace la liste des sessions en Conception ; revenir en
Exécution retrouve la session et son contenu. Les assets construits du daemon
utilisateur ont été inspectés dans Chromium : [Exécution et popover](../output/app-preview/12-workspaces-execution.png),
[Conception à trois colonnes](../output/app-preview/13-flow-design.png).

## Agents composables et sessions partageables — 12 septembre 2026

Le lot décrit dans [le guide de l'application](app.md) et les
[formats Rust v1/v2](flow-format.md) passe la validation suivante sur Linux aarch64 :

| Contrôle | Résultat |
|---|---|
| `pnpm typecheck` | Réussi |
| `pnpm build` | Réussi ; avertissement existant sur les gros bundles |
| `pnpm test:e2e` | **33 scénarios réussis**, aucun échec |
| `cargo fmt --all --check` | Réussi |
| `cargo check --locked --workspace --all-targets` | Réussi |
| `ZEDFLOW_TEST_CODEGEN=1 cargo test --locked --workspace --all-targets` | **129 tests réussis**, aucun échec ni test ignoré |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | Réussi |
| `cargo check --locked --workspace --all-targets --all-features` | Réussi |

Cargo utilise `/tmp/zedflow-adk-target`. Playwright utilise le Chromium installé
dans le cache de test, un daemon dédié, des workspaces temporaires et deux racines
isolées pour les flows et le contexte (`--flow-home`, `--context-home`). Les tests
Rust injectent également des racines de contexte temporaires. Aucun scénario
n'appelle un modèle réel ni ne modifie les flows ou sessions du daemon personnel.

Les nouveaux tests couvrent les capacités distinctes de deux agents, l'absence de
contexte implicite, les activations de skills, le rejet des appels sans provenance
et la conservation du contexte intégral avec une référence compacte dans l'état.
Les tests Cargo exécutent les sources exactes v1 et v2, y compris un sous-graphe
repris sans répétition d'effet. Les conditions typées conservent des diagnostics
sur les incompatibilités, sans coercition implicite.

Les [tests d'archives](../rust/crates/zf-serve/tests/session_archive.rs) vérifient les
checkpoints imbriqués, les reçus, les sorties, le contexte et les appels modèle,
le remappage des seules références techniques, l'idempotence et les collisions.
Ils refusent les effets incertains et les archives altérées. Les tests unitaires
couvrent les chemins ZIP hostiles et la récupération d'un import interrompu.
Le [parcours navigateur de partage](../e2e/specs/session-sharing.spec.ts) exporte
depuis un daemon temporaire, importe dans un autre workspace puis reprend par une
confirmation explicite, sans répéter l'outil déjà terminé.

Les [tests du graphe](../e2e/specs/graph-design.spec.ts) mesurent les obstacles,
vérifient les segments orthogonaux et l'absence de déplacement involontaire des
nœuds, puis les pièces, les prédicats et les caméras compacte/agrandie. Le rail
ouvre l'occurrence exacte et aligne ses repères sur les résultats d'outils. Les
tests de [courses UI](../e2e/specs/ui-races.spec.ts) retiennent volontairement les
réponses réseau pour contrôler les brouillons, les sauvegardes et les changements
de session. Une réponse liée à une attente ancienne reste conservée et ne peut pas
reprendre silencieusement une autre étape.

L'audit visuel utilise des données fixtures :
[chat](../output/app-preview/execution-audit-chat.png),
[graphe agrandi](../output/app-preview/execution-audit-fullgraph.png),
[mobile](../output/app-preview/execution-audit-mobile.png) et
[conception avec obstacles](../output/app-preview/graph-obstacle-routing.png).

La migration a été répétée sur une copie des données locales, puis vérifiée après
mise à jour du daemon utilisateur : les quatre définitions sont des fichiers Rust
valides et les 51 sessions conservent leurs identités et états (33 en attente,
18 terminées). La sauvegarde `before-file-flows.sqlite` est présente dans le
répertoire de données ; la table des compositions éditables a disparu. L'historique
ancien peut porter une timeline approximative ; aucune source Rust exacte n'est
inventée pour ces anciennes sessions.

## Persistance v2 et synchronisation — 12 septembre 2026

Le stockage canonique et les contrats de transport sont décrits dans
[Application — Stockage, synchronisation et maintenance](app.md#stockage-synchronisation-et-maintenance).
Les contrôles utilisent des fixtures, des dossiers temporaires et le daemon E2E
dédié ; aucune requête modèle payante n'est nécessaire.

- `pnpm typecheck` et `pnpm build` : succès.
- `pnpm test:e2e` : **44 tests réussis**. Les contrôles couvrent les ACK minimaux,
  deltas et chevauchements de transport, le bootstrap retenu pendant une saisie,
  les brouillons, le footer sans session, les popovers, les écrans étroits, les
  détails chargés à l'ouverture et le téléchargement exact de sorties binaires.
  Sur ce DGX, `CHROMIUM_PATH=/snap/bin/chromium` utilise le navigateur installé.
- `cargo fmt --all --check`, `cargo check --locked --workspace --all-targets` : succès.
- `ZEDFLOW_TEST_CODEGEN=1 cargo test --locked --workspace --all-targets` :
  **169 tests réussis**. Les sources exactes exportées sont compilées et exécutées,
  avec reprises v1/v2 et sous-graphes sans répéter les effets terminés.
- `cargo clippy --locked --workspace --all-targets -- -D warnings` et
  `cargo check --locked --workspace --all-targets --all-features` : succès.
- Toutes les commandes Cargo utilisent `/tmp/zedflow-adk-target`.

Les tests de stockage font traverser six conditions et sept checkpoints à des
historiques de 10, 100 et 1 000 messages. Ils vérifient les valeurs reconstruites,
les préfixes partagés, les chaînes UTF-8 et les fragments binaires. Un autre test
ferme le pool SQLite après initialisation et vérifie que les heartbeats au repos
continuent sans lecture du stockage, y compris à la révision zéro. Une mise à jour
d'outil n'inclut pas la conversation.

Un cas de régression supplémentaire reprend 37 invocations d'un catalogue de
222 skills : leurs index pèsent moins de 10 Ko et chaque contexte reste exactement
reconstructible. Les aperçus d'erreurs couvrent Unicode, la double application et
les anciens deltas, sans modifier les détails canoniques.

Les tests de reprise couvrent le premier checkpoint jamais publié dans la
projection, une consommation dans un sous-graphe et une entrée `append` déjà
appliquée. Le redémarrage publie une nouvelle révision et reprend le checkpoint
durable, sans réappliquer cette entrée. L'abandon d'une future d'outil conserve
les fragments déjà commis et le reçu initial, qui interdit une répétition aveugle.

La fixture HTTP de routage (100 messages et 12 conditions, sans LLM) a produit
21 675 octets d'événements avec zéro puis trois abonnés SSE qui ne lisent jamais
leur flux. Les temps observés dans une passe debug étaient de 1,57 s et 1,72 s.
Ce contrôle vérifie l'indépendance vis-à-vis du consommateur ; il ne constitue
ni une comparaison avec Pi ni un gain mesuré sur la conversation historique.

Une répétition sur copie cohérente des données personnelles a conservé la session
`d5b185cb-db0e-414e-99bc-04ca3ec7ee18` et retiré les 51 sessions explicitement
visées. Un décodeur indépendant du codec Rust a confirmé l'égalité du document,
des 1 386 événements et séquences, des 212 checkpoints natifs et des 125 fichiers
runtime. Les preuves de répétition sont sous
`/tmp/zedflow-maintenance-rehearsal-0sy5vcc0/`, notamment
`independent-verification.json` et `metrics.json`.

| Donnée de la session conservée, lors de la répétition | Avant | Après |
| --- | ---: | ---: |
| Métadonnées dans `runs.document` | 39 204 922 octets | 1 706 octets |
| Documents du journal `events` | 59 010 329 octets | 381 285 octets |
| Contenus partagés uniques | inclus et recopiés dans les documents | 3 733 157 octets |

Les octets retirés des documents restent reconstructibles depuis le magasin de
contenus. La réduction des bases physiques comprend également le nettoyage des
51 sessions ; elle ne mesure donc pas à elle seule la déduplication.

La maintenance réelle a ensuite été appliquée au daemon utilisateur. L'arrêt a
abouti avec SIGINT, puis le service a redémarré avec ses arguments et son
environnement conservés. La sauvegarde cohérente se trouve dans
`/home/zedium/workspaces/zedflow/.zedflow.backup-7f01070f-554f-493f-bf92-555bc015a6bc`.
Le rapport et la vérification indépendante après bascule se trouvent dans
`/tmp/zedflow-personal-maintenance-20260912/maintenance.json` et
`/tmp/zedflow-personal-maintenance-20260912/independent-verification.json`.

Cette seconde vérification confirme à nouveau l'égalité du document de session,
des 1 386 événements avec leurs séquences, des 212 checkpoints natifs et des
125 fichiers runtime (10 051 427 octets). Les lignes de workspaces et les trois
fichiers Rust locaux sont inchangés. La session reste en attente au checkpoint
`3cc26ef5-62b1-40bc-9421-52d1f4c0f0a9`, avec la même identité d'attente ; aucun nœud
n'a été exécuté pendant cette vérification.

Après bascule, la base utilise WAL. Le document de métadonnées occupe 1 695 octets,
les documents d'événements 381 285 octets et les contenus uniques 3 733 372 octets
(tailles UTF-8). La liste des sessions renvoie 681 octets et un heartbeat HTTP
178 octets. Ces mesures concernent la session conservée, sans requête modèle.

Le contrôle HTTP/SSE du dernier binaire mesure un bootstrap de **301 594 octets**,
incluant 212 passages, 57 éléments de timeline et les 37 identités d'invocation.
Le catalogue de 222 skills est disponible depuis le détail d'invocation ; une
erreur de 50 959 caractères reste intégrale dans son détail et est résumée à
512 caractères dans le bootstrap. Trois messages SSE successifs ne contiennent
que le heartbeat de 178 octets, au même curseur 2594, à 0,02 s, 5,02 s et 10,02 s.
La vérification indépendante de la session a été répétée après ce dernier
redémarrage et reste identique. Les mesures sont consignées dans
`/tmp/zedflow-personal-maintenance-20260912/final-api-verification.json` ; les logs
finaux sont `/tmp/zedflow-final-v2-validation.json` et
`/tmp/zedflow-final-e2e-4.log` (44/44).

## Socle contexte et composition — 12 septembre 2026

Le [plan](context-engine.md) et les [interfaces d'auteur](context-api.md) distinguent
le socle livré de son raccord futur aux appels modèles et aux exécutions composées.
Tous les nouveaux essais utilisent des fixtures et des répertoires temporaires.

- `pnpm typecheck` et `pnpm build` : succès. Le build conserve l'avertissement Vite
  sur la taille de certains chunks ; aucun changement frontend dans ce lot.
- `CHROMIUM_PATH=/snap/bin/chromium pnpm test:e2e` : **44 tests réussis** en 2,4 min,
  avec le daemon dédié et des workspaces temporaires. Les parcours existants restent
  validés ; cette suite ne prétend pas couvrir l'éditeur de blocs encore à réaliser.
- `cargo fmt --all --check` et `cargo check --locked --workspace --all-targets` : succès.
- `ZEDFLOW_TEST_CODEGEN=1 cargo test --locked --workspace --all-targets` :
  **216 tests réussis, 0 échec, 0 ignoré**, soit 47 tests supplémentaires.
- `cargo clippy --locked --workspace --all-targets -- -D warnings` et
  `cargo check --locked --workspace --all-targets --all-features` : succès.
- Cible principale : `/tmp/zedflow-adk-target`. Le test de stratégie Rust utilise
  aussi `/tmp/zedflow-context-codegen-target` pour sa compilation isolée.

Les nouveaux contrôles couvrent le partage d'une valeur `Arc` entre scopes, les
révisions immuables, deux writers SQLite indépendants, les conflits et rollback,
la réouverture de base et les droits non extensibles par alias. Le cache est testé
sur 8 192 entrées sans seuil de temps : aucune collecte lors d'une lecture connue,
nettoyage amorti sur les insertions et conservation des hydratations actives.

La résolution vérifie imports indépendants, réutilisation explicite, dépendances
requises, cycles invalides, boucles de routes valides et compatibilité directionnelle
des liaisons. Les types nommés disposent d'une borne d'expansion qui empêche un petit
registre de provoquer un parcours exponentiel. Les permissions d'écriture seule
sont refusées dans ce lot, car elles ne sont pas représentables par le registre.

L'évaluation de contexte couvre les groupes, champs, projections, conditions,
ressources absentes et branches inactives, sans effet ni contexte implicite. La
source Rust exacte est compilée avec les modules runtime nécessaires, puis exécutée
sur deux snapshots : son résultat est égal à l'évaluation du daemon. Les cas
incluent types nommés, médias par référence, bornes i64/u64, flottants et Unicode.
Le lecteur refuse les statements, macros ou attributs non autorisés.

Les tests d'API couvrent deux workspaces, source exacte, sauvegardes concurrentes,
fichiers invalides, hash obsolète, capacités demandées sans grant, erreurs de liaison
par nœud et workspace inconnu sans fallback. L'intégration registre→contexte vérifie
qu'un passage conserve sa révision et son `Arc` après publication, tandis que le
passage suivant reçoit la nouvelle valeur.

Journaux de cette passe : `/tmp/zedflow-context-{typecheck,build,format,check,tests}.log`,
`/tmp/zedflow-context-clippy-all.log`, `/tmp/zedflow-context-all-features.log` et
`/tmp/zedflow-context-e2e.log`.

## Context engine et harness composables — 13 septembre 2026

Cette passe couvre les sept lots du [plan](context-engine.md), depuis le registre
jusqu’aux appels ADK, à l’éditeur et aux échanges de définitions. Les
[interfaces d’auteur](context-api.md) décrivent les formats et contrats effectifs.
Les essais utilisent des modèles fixtures, des workspaces temporaires et un daemon
E2E dédié. Le daemon personnel et ses données n’ont pas été utilisés pour ces essais.

### Résultats finaux

| Contrôle | Résultat |
| --- | --- |
| `pnpm typecheck` | Succès |
| `pnpm build` | Succès, 18,86 s ; avertissement Vite existant sur la taille de certains chunks |
| `CHROMIUM_PATH=/snap/bin/chromium pnpm test:e2e` | **64 tests réussis**, 5,2 min, sur le daemon reconstruit après le correctif d’observation |
| `cargo fmt --all --check` | Succès |
| `cargo check --locked --workspace --all-targets` | Succès |
| `ZEDFLOW_TEST_CODEGEN=1 cargo test --locked --workspace --all-targets --no-fail-fast` | **341 tests réussis**, aucun échec, aucun test ignoré ou filtré, 204,42 s |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | Succès |
| `cargo check --locked --workspace --all-targets --all-features` | Succès |

La cible Cargo principale est `/tmp/zedflow-adk-target` ; les compilations isolées
des projets exportés utilisent également des cibles externes au dépôt. L’option
`--no-fail-fast` collecte les résultats de toutes les suites sans en sélectionner
un sous-ensemble. Le frontend est inchangé depuis son dernier typecheck/build ;
la suite E2E finale utilise le correctif runtime complet. Ses serveurs temporaires
ont été arrêtés après les tests.

Le rapport Cargo est `/tmp/zedflow-context-final-4-validation.json`, avec les
journaux `/tmp/zedflow-context-final-4-{format,check,tests,clippy,all-features}.log`.
Les journaux frontend sont
`/tmp/zedflow-context-engine-final-2-{typecheck,build}.log` et
`/tmp/zedflow-context-engine-final-3-e2e.log`.

### Frontières vérifiées

| Domaine | Preuves |
| --- | --- |
| Données et types | Valeur hydratée partagée entre scopes, identités distinctes pour un même contenu, révisions immuables, conflits concurrents, droits par alias, réouverture SQLite et borne d’expansion des types nommés |
| Langage Rust | Aller-retour et compilation des sources exactes v1/v2, conditions sans coercition, collections, templates, bibliothèques paramétrées, construction de types nommés et budgets d’évaluation |
| Inférence | Deux agents reçoivent leurs seules ressources et capacités ; absence de contexte implicite, historique ADK explicite avec paires d’outils valides, données JSON conservées comme données en v2, médias par référence et diagnostic de modalité incompatible |
| Ressources et fenêtres | Lectures paresseuses depuis le contexte réel du passage, branches inactives sans lecture, producteurs durables et invalidation, intervention d’un second agent autorisée par bridge, révisions de fenêtres et sélection pour un seul passage futur compatible |
| Composition | Instances indépendantes, contrats et bridges requis, appels avec attente, lancement parallèle et transfert, résultats par références, attentes parent/enfant, reprise des reçus et conflits de publication |
| Révisions | Conservation des choix de stratégie par instance après Save, adoption au prochain point compatible, version de l’appel en cours préservée, reprise depuis la définition du checkpoint, rejet des dépendances incompatibles avant écriture et récupération d’une publication interrompue |
| Portabilité | Packages de stratégies/types/bibliothèques/bridges, sources et hashes vérifiés, import sans exécution, archives anciennes et canoniques, compilation des lecteurs standard et comparaison daemon/Cargo des sources exactes avec attente imbriquée et reprise de reçus |

Le test des sélections de contexte importe deux fois le même flow. Chaque instance
conserve sa stratégie après publication du flow et des sources : la requête de
l’une contient `ALPHA second`, celle de l’autre `BETA only`, sans contamination
croisée. Les choix invalides sont refusés avant création d’un run. Une composition
dont l’entrée autonome appelle un enfant interactif ouvre bien une session ; un
import inutilisé ou une route inaccessible depuis l’entrée choisie ne suffit pas.

La provenance d’une fenêtre dépend du programme lié complet, y compris types,
bibliothèques et bindings. Un scénario conserve la même source de stratégie tout
en modifiant les types liés : l’ancienne fenêtre retrouve les anciens types exacts,
et ne peut pas être sélectionnée comme si elle provenait du nouveau programme.
La conversion v1 → v2 crée une copie explicite ; elle ne réécrit pas la stratégie
historique. En v2, `adkMessages` et `json` ont des sémantiques distinctes.

Un résultat d’outil de 200 000 octets est lu depuis le magasin canonique, puis seule sa
mesure explicitement projetée atteint le modèle. Le contenu complet reste
consultable. L’export d’un lecteur standard est aussi exécuté dans un nouveau
workspace après retrait du workspace d’origine ; un lecteur natif absent du
bundle est refusé, sans remplacement silencieux.

### Observation et compatibilité des anciens scénarios

Les tests de timeout bloquent volontairement la sélection d’une révision ou la
capture SQLite avant l’entrée dans le nœud. Chaque tentative admise conserve une
identité unique et son terminal ; aucune révision exécutée n’est inventée quand
la préparation n’a pas abouti. Un receiver retenu jusqu’à la fin conserve les deux
tentatives. Une erreur réelle de capture reste une erreur de stockage, distincte
d’une annulation.

La file reste bornée. Un terminal utilise la file normale disponible ou son
emplacement de secours. Si le secours précédent reste occupé, la tentative suivante
est refusée avant effet, avec un diagnostic borné comptant les refus, y compris
avec `retryOn: any`. Le test de capacité un vérifie qu’aucune réservation destinée
à la fin d’un nœud ne bloque l’envoi de son propre événement de début.

Deux anciennes fixtures ont été adaptées aux décisions produit sans modifier le
runtime : le test de conversation attend maintenant un vrai nœud d’entrée humaine
avant `utilisateur → read → edit → exec → réponse` ; le test de workspaces vérifie
l’adoption après Save tout en gardant les anciennes sources. Ce dernier exécute
un effet avant la pause, sauvegarde, redémarre et reprend deux workspaces : l’effet
n’est pas répété, les données restent locales et l’inspection retrouve la source
exacte de chaque passage, ancien comme nouveau.

### Interface et limites de la preuve

Les parcours navigateur couvrent l’édition de blocs au clavier, les bibliothèques
et types, les conflits externes, les lecteurs, les packages, le choix de stratégie
par inférence, les fenêtres révisables, l’inspection historique et la conservation
des brouillons et caméras. Ils couvrent aussi les popovers, le footer sans session,
les écrans étroits, les transports sans doublons et les sources Cargo exactes après
édition ou suppression d’un fichier.

Les captures ont été relues visuellement : [éditeur](../output/context-engine/editor.png),
[préparation](../output/context-engine/preparation.png) et
[fenêtre historique](../output/context-engine/window-program.png). Le correctif
d’onglets est aussi contrôlé par une assertion géométrique.

Ces preuves ne mesurent pas la qualité d’un modèle réel. Les adaptateurs disponibles
restent ceux intégrés au daemon ; le langage de médias générique ne fournit pas à
lui seul un backend diffusion ou world model. Les changements de structure à chaud
restent limités aux frontières séquentielles vérifiées ; le remplacement arbitraire
de branches parallèles, d’instances ou de droits n’est pas accepté. Le test d’un
brouillon utilise un workspace temporaire, sans constituer un sandbox d’outils.

## Intégration Contexte → Modèle et exemple Working System — 13 septembre 2026

Les nouveaux flows utilisent le format 3 et deux nœuds exécutés séparément. Les
tests `context_model_nodes` vérifient les connexions obligatoires, une préparation
durable consommée une seule fois, la reprise sans nouvelle acquisition des sources,
le modèle figé à la préparation et l’exécution Cargo de la source exacte. Les
formats historiques restent couverts par leurs fixtures propres.

Les tests `context_starters` vérifient que les stratégies existantes ne sont jamais
remplacées, que l’exemple est idempotent et qu’une source existante invalide produit
un diagnostic sans écrasement. Ils vérifient aussi les instructions et commandes
depuis le cwd du flow, le partage des reçus et de l’annulation entre instances, et
trois tours de conversation sans accumulation des projections de fichiers.

Les parcours navigateur ajoutés couvrent :

- la création et l’inspection des passages Contexte et Modèle, ainsi que les
  flèches des connexions ;
- la conversion d’un ancien flow, avec conservation des positions, du workspace
  propriétaire et de la jointure de branches parallèles avant la préparation ;
- le guide Sources → Programme → Contexte, les exemples éditables, la sélection
  explicite des fragments et un catalogue retardé qui ne ferme pas l’éditeur actif ;
- l’installation de l’exemple depuis l’interface, deux modèles fixtures distincts,
  une lecture effective du `AGENTS.md` du dossier documentaire et la restitution
  dans le workspace du harness ;
- la conservation du brouillon lors de l’ouverture de l’exemple et l’accès à
  l’historique des exécutions autonomes depuis Exécution.

Ces essais n’utilisent aucun modèle réel. Les workspaces, sources et sessions de
test sont temporaires ; le daemon E2E est indépendant du daemon personnel. Sur ce
DGX, Chromium Snap doit écrire ses artefacts de téléchargement dans le répertoire
`test-results/` du workspace, ignoré par Git : son `/tmp` privé n’est pas celui du
processus Playwright.

Validation finale : `pnpm typecheck`, `pnpm build` et les **74 tests E2E** passent.
La suite Cargo complète passe avec `ZEDFLOW_TEST_CODEGEN=1` (351 tests), puis le
test supplémentaire des définitions d’exemple invalides passe dans la relance
des quatre tests `context_starters`. `cargo fmt --all --check`, `cargo check
--locked --workspace --all-targets`, Clippy avec `-D warnings` et le contrôle
`--all-features` passent, avec `CARGO_TARGET_DIR=/tmp/zedflow-adk-target`.
Le build web conserve l’avertissement Vite sur la taille du bundle principal.

## Studio de contexte structuré — 13 septembre 2026

Cette passe vérifie les contrats du [langage et de l’API](context-api.md) utilisés
par le studio structuré. Les tests utilisent des valeurs fixtures, des bases et
workspaces temporaires ; les tests d’inférence utilisent des modèles fixtures.
La suite ne mesure pas la qualité des réponses d’un modèle réel.

| Contrôle Rust | Résultat | Durée |
| --- | --- | ---: |
| `cargo fmt --all --check` | Succès | 0,5 s |
| `cargo check --locked --workspace --all-targets` | Succès | 3,9 s |
| `ZEDFLOW_TEST_CODEGEN=1 cargo test --locked --workspace --all-targets` | **374 tests réussis, 0 échec, 0 ignoré**, dans 59 targets/suites | 324,3 s |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | Succès | 19,4 s |
| `cargo check --locked --workspace --all-targets --all-features` | Succès | 44,5 s |

Toutes ces commandes utilisent `CARGO_TARGET_DIR=/tmp/zedflow-adk-target`. Les
compilations isolées de sources exportées gardent également leurs cibles hors du
dépôt. Les journaux finaux sont
`/tmp/zedflow-structured-final-cargo-{format,check,tests,clippy,all-features}.log`.

Les contrôles nouveaux couvrent :

- l’aperçu autonome avec des capacités demandées, et le rejet de ces mêmes
  capacités sans grant lors de la validation de liaison ;
- la persistance des types nommés dans la stratégie, leur relecture sans flow et
  le rejet des définitions contradictoires ;
- le catalogue dynamique, ses dépendances transitives, ses conflits d’identité,
  les données publiques des flows et l’isolation des workspaces ;
- les parcours `forEach` à sorties multiples, conditions, occurrences distinctes,
  variables imbriquées et restauration de leur provenance ;
- les valeurs sources partagées par référence, les listes absentes ou vides,
  les types incompatibles et les budgets face à une croissance imbriquée ;
- les listes construites depuis des expressions, les listes vides typées et la
  troncature Unicode, y compris zéro et les caractères hors ASCII ;
- l’arrivée effective des messages construits dans un modèle fixture sans
  historique implicite, et la compilation de la source Rust exacte contenant
  types embarqués, `for_each`, `list` et `truncate`.

La suite complète conserve les preuves antérieures de migration, archives,
provenance des outils et reprise sans répétition des effets. Les contrôles
supplémentaires de profondeur couvrent des schémas de 32 et 64 niveaux, leurs
échanges HTTP, leur réouverture et l’exécution du bundle Cargo exact. Un
dépassement des limites métier ou de représentation est refusé explicitement.

`pnpm typecheck` et `pnpm build` passent sur les sources frontend finales. Le
build conserve les avertissements Vite sur certains chunks supérieurs à500ko.
Les journaux sont `/tmp/zedflow-structured-typecheck-final.log` et
`/tmp/zedflow-structured-build.log`.

Les **87 scénarios E2E sont validés** avec Chromium et un daemon dédié :

- une première suite complète passe87/87 en7,4min ;
- après le dernier ajustement de décodage HTTP des compositions, la suite
  complète passe86/87 en8,5min ; un enregistrement de conversion v1 → v2 dépasse
  l’attente de5s alors qu’une compilation Rust tourne simultanément ;
- ce scénario passe ensuite seul en3,3s, sur un daemon neuf, avec le même code et
  la même assertion. Cette relance établit son succès ; elle n’isole pas à elle
  seule la cause exacte du délai observé.

Les journaux de ces trois essais sont `/tmp/zedflow-structured-e2e.log`,
`/tmp/zedflow-structured-e2e-release.log` et
`/tmp/zedflow-structured-e2e-release-retry.log`. Les artefacts du scénario ayant
dépassé son délai restent dans `test-results/context-studio-release/`.

Les parcours ajoutés vérifient la galerie et les types exposés, le renommage
des sources et des jeux d’essai, les insertions typées par glisser et clavier,
les déplacements entre branches, les brouillons et aperçus concurrents, les
messages structurés et échanges d’outils, la navigation précise dans plusieurs
occurrences et les panneaux sur écran étroit. Les quatre
[captures finales et leur comparaison aux maquettes](../output/context-design-structured/implemented/index.html)
ont été relues ; leur manifeste conserve les hashes et le scénario producteur.
Le [tableau des critères](prd/context-studio-structured.md) relie ces preuves aux
fonctionnalités demandées.

Deux défauts ont été corrigés pendant la validation : une analyse de source
profondément imbriquée débordait la pile du daemon, et une fixture de compilation
plaçait un module avant un attribut interne de crate. Les protections de lecture
sont décrites dans [le contrat du codec](context-api.md#limites-de-lecture-et-de-profondeur).
La fixture conserve maintenant les attributs en tête ; le scénario de reprise
de sous-graphe passe avec le code exporté, sans répétition des outils. La suite
Rust complète ci-dessus a été relancée après ces corrections.

## Révision des stratégies existantes — 13 septembre 2026

Les quatre stratégies enregistrées du workspace (`conversation-default`,
`tools-default`, `working-system-context`, `workspace-default`) ont été révisées
explicitement pour utiliser les types nommés et le parcours de conversation du
studio structuré. Leurs identifiants, capacités et noms de bindings sont conservés.
Les sources antérieures et leurs hashes sont sauvegardés dans
`.zedflow/backups/context-starters-20260913T143450Z/` ; les écritures sont passées
par l’API avec leur hash attendu. Après réouverture, les quatre stratégies sont
égales à celles produites par le builder Rust compilé.

Les six tests `context_starters` comprennent 32 comparaisons de requêtes ADK
complètes ancien/nouveau : quatre stratégies, huit cas, notamment historique
absent ou vide, raisonnement signé, appels/résultats multiples et nouvelle saisie
après un outil. L’initialisation reste sans écrasement, y compris lorsqu’un ancien
starter est déjà présent. Les nouveaux presets produisent une conversation ADK
valide et restent des données d’essai locales au brouillon.

Validation sur fixtures et daemon dédié :

- `pnpm typecheck` et `pnpm build` passent ; les avertissements Vite sur les gros
  chunks restent ceux du build précédent.
- Les **37 scénarios E2E relatifs au contexte passent**, dont cinq nouveaux
  scénarios couvrant les quatre stratégies et les cas historique absent/vide.
- `cargo fmt --all --check`, `cargo check --locked --workspace --all-targets`,
  **376 tests Rust sans échec ni test ignoré** avec `ZEDFLOW_TEST_CODEGEN=1`, puis
  Clippy avec `-D warnings` passent. La cible Cargo reste externe ; aucune
  dépendance ni feature n’a changé.

Les journaux sont `/tmp/zedflow-strategies-refresh-{typecheck,build,e2e}.log` et
`/tmp/zedflow-strategies-refresh-cargo-{format,check,tests,clippy}.log`.

Après installation, les cinq flows du workspace passent la validation et les
deux compositions Working System sont préparées avec leurs stratégies révisées,
sans lancer de nœud. Le daemon a été redémarré à l’arrêt des exécutions ; les deux
sessions en attente sont conservées. L’accès Windows sur `127.0.0.1:53410` a été
revérifié. La [capture de la stratégie installée](../output/context-design-structured/implemented/05-updated-workspace-strategy.png)
montre les cinq sources et les sept fragments produits par le jeu d’essai.

## Harness : routage, contexte explicite et messages intermédiaires — 14 septembre 2026

Le starter du Harness utilise dix nœuds en v3. Chaque itération passe par
`Réorientations → Routage du travail → Contexte → Modèle`. Le point public `work`
accueille les routes conditionnelles des bridges ; le modèle reçoit leur résultat
par un binding explicite de la stratégie `harness-default`. Un tour sans route
remet ce résultat à vide. Les formats historiques restent exécutables, mais le
nœud `agent` ne peut plus être ajouté ou configuré comme un nouveau nœud dans
l’éditeur. Les conversions proposées créent une copie.

Les tests de comportement couvrent :

- la continuation sans bridge, les gardes fausses puis vraies, les routes
  ambiguës, les sélections inconnues et les routes obligatoires ; une sélection
  conditionnelle connue dont la garde est fausse conserve sa sémantique ;
- l’absence de résultat périmé dans la requête modèle, le contrat typé du point
  et l’exécution du projet Cargo exporté depuis la source exacte ;
- une séquence `utilisateur → commentaire → exec → read → commentaire → write
  → réponse → inbox`, avec publication du premier commentaire pendant que
  l’outil est encore bloqué, sans attente utilisateur intermédiaire ;
- deux commentaires de texte identique avec des occurrences distinctes,
  l’absence de doublon après la fin du graphe et la conservation de la timeline
  et de l’attente après redémarrage ;
- les conversions de Harness avec exports Rust partiels, canaux d’entrée
  personnalisés cohérents, contrats existants et refus des montages modifiés
  qui ne peuvent pas être convertis automatiquement.

Validation avec modèles fixtures, répertoires temporaires et daemon E2E dédié :

- `pnpm typecheck`, `pnpm build`, `cargo fmt --all --check` et
  `cargo check --locked --workspace --all-targets` passent sur les sources
  finales. Vite conserve son avertissement sur les chunks supérieurs à 500 ko.
- **385 tests Rust passent, sans échec ni test ignoré**, avec
  `ZEDFLOW_TEST_CODEGEN=1`. Clippy passe avec `-D warnings`. La cible Cargo est
  `/tmp/zedflow-adk-target` ; aucune dépendance ni feature n’a été modifiée par
  cette intervention.
- Les **97 scénarios E2E ont chacun un résultat positif**. La suite complète
  initiale passe 88 scénarios sur 94. Deux tests de packages utilisaient encore
  les contrôles de l’ancien Agent : leurs fixtures ciblent maintenant le
  nœud Contexte v3, en conservant leurs assertions sur le typage, les sources
  exactes, les imports et le contenu envoyé au modèle. Ces deux tests et les
  quatre autres échecs passent ensuite sur un daemon neuf, sans changement de
  délai. Trois tests supplémentaires sur les limites de conversion passent
  séparément. Il ne s’agit donc pas d’une suite complète de 97 tests passée
  en une seule invocation.

Les quatre dépassements de délai sont survenus pendant des compilations Cargo
et des modifications de modules servis par Vite. La relance avec les sources
figées et sans compilation concurrente établit leur succès, sans isoler à elle
seule la cause exacte de chaque dépassement. Un premier test Rust avait également
révélé une régression de la route explicitement sélectionnée mais désactivée par
sa garde ; elle a été corrigée avant la suite finale de 385 tests.

Le nouveau flow installé est `af9f417b-57eb-4638-8891-075ff7778c5f`, nommé
« Harness de workspace ». L’API a refusé de convertir la définition existante en
place, car une session conserve son schéma et ses checkpoints v2. Cette définition
garde son identifiant `0cebd593-0c20-462e-9df9-b09a1537172a` et porte désormais le
nom « Harness de workspace · historique ». Sa source antérieure et les réponses
de l’API sont sauvegardées dans
`.zedflow/backups/harness-routing-20260914T133551Z/`.

Le daemon a été redémarré sans exécution active, puis les identifiants, statuts et
dates de modification des deux sessions en attente ont été comparés : ils sont
inchangés. Aucun modèle réel n’a été appelé pour cette validation. La compilation
du nouveau Harness depuis l’interface réussit ; l’accès par le tunnel du Mac sur
`http://localhost:60956/` est vérifié après le dernier redémarrage.

Le [manifeste de validation](../output/harness-routing/manifest.json) conserve les
empreintes des captures, la définition installée et les journaux. Les captures
montrent [la séparation Contexte/Modèle](../output/harness-routing/01-harness-definition.png)
et [le point de routage configurable](../output/harness-routing/02-routing-point.png).

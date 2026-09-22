# Base ADK-Rust

## Source et version

Le produit utilise les packages publiés de [Zavora ADK-Rust](https://github.com/zavora-ai/adk-rust),
fixés exactement à **2.2.0**. ADK-Rust est un projet indépendant ; il ne faut pas
assimiler sa version ou sa couverture fonctionnelle à celles de Google ADK.

Les manifestes publiés sur crates.io et leur code téléchargé par Cargo sont la
référence des API testées. La crate `adk-rust` 2.2.0 indique le commit upstream
[`74765eb`](https://github.com/zavora-ai/adk-rust/tree/74765eb04930648795b53c3db689ddd10032c31e).
Les documentations d'API sont disponibles sur [docs.rs](https://docs.rs/adk-rust/2.2.0/adk_rust/)
et [adk-graph](https://docs.rs/adk-graph/2.2.0/adk_graph/).

## Catalogue complet

Le workspace upstream inventorié contient 42 crates de bibliothèque publiables,
le binaire compagnon `cargo-adk`, et `xtask`, outil de maintenance non publié.
Les 42 bibliothèques sont des dépendances du catalogue `rust/crates/zf-runtime/Cargo.toml`, toutes présentes dans
le lockfile. [adk-catalog.json](adk-catalog.json) conserve la liste exploitable.

| Domaine | Crates |
|---|---|
| Base | `adk-rust`, `adk-core`, `adk-rust-macros` |
| Agents et exécution | `adk-agent`, `adk-runner`, `adk-graph`, `adk-managed` |
| Modèles | `adk-model`, `adk-gemini`, `adk-anthropic`, `adk-mistralrs` |
| État et connaissances | `adk-session`, `adk-memory`, `adk-artifact`, `adk-rag` |
| Outils et code | `adk-tool`, `adk-code`, `adk-codeact-monty`, `adk-sandbox`, `adk-devtools`, `adk-action` |
| Interfaces et protocoles | `adk-cli`, `adk-server`, `adk-acp`, `adk-awp`, `awp-types` |
| Interaction | `adk-browser`, `adk-computer-use`, `adk-realtime`, `adk-audio` |
| Extension et contrôle | `adk-skill`, `adk-plugin`, `adk-auth`, `adk-guardrail` |
| Mesure | `adk-eval`, `adk-bench`, `adk-telemetry`, `adk-retry-reflect` |
| Services et déploiement | `adk-gcp`, `adk-deploy`, `adk-payments`, `adk-enterprise` |

## Profils de compilation

Le profil par défaut compile les agents, graphes, modèles Gemini, sessions, mémoire,
outils et runner nécessaires au runtime produit. Les graphes activent SQLite,
functional, node-cache, delta-checkpoint et time-travel.

`adk-platform` rend disponibles toutes les autres bibliothèques sauf `adk-mistralrs`,
et active le profil `full` de la crate façade ainsi que son CLI et SQLite.
`local-inference` ajoute `adk-mistralrs` sur CPU. `all-adk` réunit ces deux profils.
`--all-features` est une gate du produit.

Une bibliothèque disponible ne signifie pas que tous ses services externes sont
configurés ou démarrés. Les profils ne téléchargent pas de poids de modèle et
n'activent pas tous les flags matériels ou fournisseurs upstream. CUDA, Metal,
ONNX, l'audio matériel, les clouds et les backends de bases de données demandent
des expériences et des prérequis spécifiques.

Les sources de toutes les dépendances sont récupérées par `cargo fetch --locked`.
Le catalogue complet est compilé séparément du profil produit par défaut.

## Corrections de résolution reproductibles

Deux ajustements du manifeste permettent de compiler le catalogue complet ici :

- `get-size2 = 0.10.1`, optionnel avec `adk-platform`, reprend la version du lockfile
  publié d'`adk-codeact-monty`. La version 0.10.3 dépend de `compact_str` 0.10 alors
  que Ruff 0.0.3 attend son implémentation pour 0.9, ce qui échoue à la compilation.
- `libdbus-sys` active `vendored` avec `adk-platform`, pour compiler la dépendance
  de keyring/adk-cli sans les en-têtes D-Bus du système.

Ces adaptations portent sur les dépendances ; aucun code ADK n'est forké ou patché.
Les réexaminer lors d'une mise à jour ADK, puis refaire la compilation complète.

## Outils compagnons

`cargo-adk` est un binaire séparé, pas une bibliothèque à lier au flow. Il est
installable avec Cargo :

```sh
cargo install cargo-adk --version 2.2.0 --locked
cargo adk --help
```

Le compagnon `cargo-adk` et ADK Studio sont des outils upstream optionnels,
non installés par le manifeste produit. Les anciennes expériences lab/Studio
ne sont pas un frontend ni une gate de Zedflow. Le catalogue compile la bibliothèque
`adk-cli` ; le daemon produit est `zedflow-daemon` (`zf-serve`) et sa CLI est `zf`.

Toutes les commandes Cargo de cette page partent de `rust/` (toolchain 1.96.1),
avec `CARGO_TARGET_DIR=/tmp/zedflow-adk-target`. Voir [validation](validation.md)
pour la distinction entre dépendance compilée et fournisseur réellement essayé.

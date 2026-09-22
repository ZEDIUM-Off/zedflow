# Provenances du harness de workspace

Cette note identifie les dépendances et références utilisées par le harness.
Les licences complètes des dépendances restent celles des paquets distribués.

## ADK-Rust 2.2.0

Projet : [zavora-ai/adk-rust](https://github.com/zavora-ai/adk-rust).
Les manifestes publiés de `adk-devtools` et `adk-skill` 2.2.0 déclarent la licence
Apache-2.0 et l'auteur James Karanja Maina.

Zedflow utilise directement `ReadFileTool`, `WriteFileTool`, `EditFileTool` et
leur `Workspace` partagé. Le parsing de skills repose sur
`adk_skill::parse_skill_markdown`, avec adaptation du frontmatter d'entrée.
ADK conserve l'exécution des nœuds, les transitions et les checkpoints.

## Pi 0.85.1

Référence inspectée : paquet `@earendil-works/pi-coding-agent` 0.85.1, auteur
déclaré Mario Zechner, licence déclarée MIT, dépôt publié
[earendil-works/pi](https://github.com/earendil-works/pi).

Les modules `dist/core/tools/bash.js`, `truncate.js`, `resource-loader.js`,
`package-manager.js`, `skills.js`, `agent-session.js` et `system-prompt.js` du
paquet ont servi à préciser les conventions de sortie shell, annulation,
instructions AGENTS, découverte progressive et invocation explicite des skills.
Le transport Codex existant prend également Pi comme référence.

Ces comportements sont adaptés dans le daemon Rust. Le harness n'embarque pas
le moteur de boucle TypeScript de Pi, ses extensions ou son système de packages.
Les outils fichiers gardent les contrats ADK, notamment l'édition exacte après
lecture ; cette intégration ne revendique pas une compatibilité complète Pi.

## libavoid-js 0.4.5

Le routage orthogonal utilise [Aksem/libavoid-js](https://github.com/Aksem/libavoid-js),
une distribution WebAssembly de libavoid, sous LGPL-2.1-or-later. Le JavaScript et
le WASM sont fournis par la dépendance npm épinglée ; la licence est distribuée
dans `web/apps/client/public/licenses/libavoid-js.txt` et accessible sous
`/licenses/libavoid-js.txt` dans l’application. Le code source correspondant et
les instructions de construction restent disponibles dans le dépôt amont.

## Frontières de distribution

Le catalogue ADK reste conservé dans `docs/adk-catalog.json`. Le SDK utilise
Zod 4 (MIT) pour ses contrats runtime ; Vue et Electron gardent leurs licences
amont. Les lockfiles indépendants `rust/Cargo.lock`, `web/pnpm-lock.yaml` et
`e2e/pnpm-lock.yaml` identifient les dépendances exactes. Un export Cargo inclut
les sources de support Zedflow et conserve les références/verrous des dépendances ;
ce document ne remplace pas les licences des paquets redistribués.

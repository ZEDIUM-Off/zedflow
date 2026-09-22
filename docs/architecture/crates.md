# Architecture Rust/web

ADK-Rust 2.2.0 exécute les graphes. Les bibliothèques Zedflow organisent les
frontières du produit, sans deuxième ordonnanceur ni cluster distribué.

| Crate | Responsabilité | Dépendances Zedflow permises |
|---|---|---|
| zf-core | identités, JSON, provenance, types | aucune |
| zf-context | préparation pure, contrats de ressources | core |
| zf-flows | définitions, formats, bridges, packages | core, context |
| zf-compiler | snapshots, résolution/lowering, exports | core, context, flows |
| zf-storage | contenus, catalogues, sessions et checkpoints opaques | core, context, flows |
| zf-runtime | matérialisation ADK, modèles/outils, Checkpointer | core, context, flows, compiler, storage |
| zf-execution | admission, runs, sessions, commandes et enfants locaux | couches précédentes |
| zf-serve | HTTP/SSE/RTC, assets, lifecycle | couches précédentes |
| zf-cli | templates et commandes `zf` | couches précédentes ; serve pour `serve` |

Compiler ne lit pas storage et ne dépend ni de runtime ni d'execution. Runtime
ne dépend pas d'execution. Le support embarqué des exports est une capture de
fichiers au build, pas une dépendance inverse. Les traits résident chez leur
consommateur bas ; les adaptateurs persistants vivent dans storage/runtime.

`zf-execution` est l'autorité locale unique, qu'une commande vienne de CLI, HTTP
ou d'un flow. Un run autonome ne crée pas de session ; les flows interactifs en
ont une. Les publications post-commit transportent des références, pas une
nouvelle copie intégrale de l'historique à chaque couche.

Le workspace pnpm est exclusivement `web/`. SDK et Vue sont consommés par leurs
exports construits. `e2e/` et `tooling/` restent autonomes, sans `workspace:*`
transversal. Les tests purs résident chez leur propriétaire ; les intégrations
multicrates, HTTP et codegen peuvent résider dans `zf-serve/tests` pour conserver
le graphe acyclique. Les scripts Cargo font `cd rust` avant l'invocation.

Les sources, packages et protocoles sont versionnés indépendamment. Zod est
l'autorité des types TypeScript, serde celle de Rust : les fixtures de conformité
vérifient leur accord, aucun générateur bidirectionnel n'est annoncé.

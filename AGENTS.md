# Zedflow — produit Rust/web

Lire `CONTEXT.md` pour le vocabulaire, `docs/architecture/crates.md` pour les
frontières et `docs/validation.md` avant de modifier tests, scripts ou CI.

- Cargo appartient à `rust/` ; pnpm à `web/`. `e2e/` et `tooling/` sont autonomes.
  Lancer Cargo depuis `rust/` pour sélectionner `rust-toolchain.toml` (1.96.1),
  avec `CARGO_TARGET_DIR=/tmp/zedflow-adk-target`.
- ADK-Rust 2.2.0 conserve l'exécution des graphes. Compiler est pur ; execution
  possède l'admission, les runs et les sessions, indépendamment de HTTP.
  Conserver le catalogue ADK complet décrit dans `docs/adk.md`.
- Les clients utilisent les exports publics de `@zedflow/sdk` (Zod), les
  composants Vue les adaptateurs `@zedflow/vue`. Garder JSON ouvert et raw exacts.
- Pour créer ou convertir un flow, lire `docs/development/flow-packages.md`.
  Le writer utilise `.zedflow/flow/<id>/` ; les quatre racines legacy restent
  lisibles. La conversion est explicite et ne réexécute pas les sessions.
- Avant Rust, charger `rust-skills` et les règles pertinentes. Avant de modifier
  ce fichier, charger `writing-for-agents`.
- Tests : fixtures locales, data/workspace/home explicites ; aucun modèle réel,
  aucun remplacement de HOME/CODEX_HOME. Les sorties vont sous `.agents/output/`.
  Ne jamais versionner credentials, bases personnelles ou caches runtime.
- Préserver le travail non lié. La branche par défaut est `main` ; publication,
  activation et nettoyage de données/worktrees nécessitent un mandat explicite.

Les gates complètes et les limites de plateforme ont une seule source :
`docs/validation.md`. Un ancien compte de tests n'est pas une preuve du diff actuel.

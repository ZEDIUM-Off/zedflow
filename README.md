# Zedflow — ADK-Rust lab

Un laboratoire Rust pour expérimenter des agents et des graphes composables avec
ADK-Rust. Cette branche repart sur ADK-Rust : le port Pi et le runtime LangGraph
ne font plus partie de son architecture ni de ses critères de validation.

L'intention produit reste un **harness dont les comportements, la composition et
l'évolution s'expriment en graphes**. Pour l'instant, le code explore directement
les primitives ADK. Les contrats et le registre propres à Zedflow restent à concevoir
à partir des expériences, sans les implémenter prématurément.

## Démarrer

Rust 1.96.1 est fixé dans `rust-toolchain.toml`. Le catalogue complet des 42 crates
de bibliothèque ADK-Rust est fixé à 2.2.0 et résolu dans `Cargo.lock`.

```sh
export CARGO_TARGET_DIR=/tmp/zedflow-adk-target
cargo fetch --locked
cargo run --locked -- list
cargo run --locked -- research
cargo run --locked -- agent
cargo run --locked -- memory
```

Ces commandes utilisent des données et un modèle de test locaux. Les événements
ADK sortent en JSONL sur stdout, avec le résultat final dans l'événement `done`.
La recherche travaille sur des fixtures clairement identifiées ; elle ne fait
aucune recherche web réelle.

Pour vérifier une pause et une reprise dans deux processus :

```sh
cargo run --locked -- checkpoint start --database 'sqlite:///tmp/zedflow-checkpoints.db?mode=rwc'
cargo run --locked -- checkpoint resume --database 'sqlite:///tmp/zedflow-checkpoints.db?mode=rwc'
```

Le premier lancement prépare une valeur et s'arrête avant `deliver`. Le second
reprend `deliver` avec l'état sauvegardé. Pour une nouvelle expérience dans la même
base, choisir un autre `--thread`.

Un modèle Gemini réel peut remplacer le modèle fixture dans la même boucle :

```sh
# Fournir GOOGLE_API_KEY dans l'environnement, puis choisir un modèle du compte.
cargo run --locked -- agent --live --model '<model-id>' 'Comment composer ces graphes ?'
```

Ce mode contacte le fournisseur et peut être facturé. Le sous-graphe de recherche
reste sur les fixtures ; cette commande teste l'intégration du modèle à la composition.

## Explorer

| Fichier | Responsabilité |
|---|---|
| `flows/research.rs` | Valider une requête, récupérer les fixtures, retourner des éléments de réponse |
| `flows/agent_loop.rs` | Boucler entre une décision d'agent et le sous-graphe de recherche |
| `flows/shared_memory.rs` | Partager un service mémoire entre graphes tout en isolant les projets |
| `flows/checkpoint.rs` | Préparer, suspendre et reprendre avec un checkpoint SQLite |
| `src/main.rs` | Choisir une expérience et afficher ses événements |
| `tests/flows.rs` | Vérifier les frontières et les résultats des expériences |

Le manifeste rend toutes les crates disponibles, avec deux groupes additionnels :

```sh
cargo check --locked --all-targets --features adk-platform
cargo check --locked --all-targets --features all-adk
```

`adk-platform` active les bibliothèques de plateforme et le profil `full` ADK.
`all-adk` ajoute `adk-mistralrs` pour l'inférence locale sur CPU. Cela ne signifie
pas activer simultanément tous les backends CUDA, Metal, audio, cloud ou bases de
données. Voir [le catalogue et l'installation des outils](docs/adk.md).

## Contexte de conception

- [Vision et périmètre](CONTEXT.md)
- [Composition, registre et partage de graphes](docs/composition.md)
- [Contextes, state, stores et ressources](docs/resources.md)
- [Évolution et capitalisation](docs/evolution.md)
- [Méthode des expériences](flows/README.md)
- [Validation du reboot](docs/validation.md)

ADK-Rust est le projet indépendant de Zavora. Le choix de cette base ne suppose
ni maintenance par Google ni équivalence avec les SDK Google ADK.

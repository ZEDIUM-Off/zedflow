# Validation du reboot

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

## Observations utiles

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

# Expériences de flows

Chaque fichier Rust porte un workflow précis et déclare son rôle dans son commentaire
de module. Les constructeurs retournent directement un `CompiledGraph` ADK. `mod.rs`
les expose et le launcher les sélectionne ; aucun registre Zedflow n'est caché ici.

| Expérience | Entrées | Sorties | Ressources et effets |
|---|---|---|---|
| `research.rs` | `query` non vide | `sources`, `evidence` | Fixtures locales constantes, sans réseau |
| `agent_loop.rs` | `question`, éventuellement `evidence` | `response`, `evidence` | LlmAgent ADK ; modèle fixture par défaut, Gemini explicite |
| `shared_memory.rs` | `note` optionnelle, `query` optionnelle | `matches` | Service ADK injecté, écritures mémoire limitées au projet |
| `checkpoint.rs` | État vide au premier lancement | `prepared`, `preparations`, `delivered` | Checkpoints SQLite, pause avant `deliver` |

## Ajouter une expérience

Décrire une question vérifiable avant de coder. Donner au fichier une responsabilité
limitée, rendre les canaux et ressources lisibles et utiliser les API upstream.
Ajouter une entrée de lancement si cela facilite l'exploration. Partager un flow
existant par composition plutôt que copier ses étapes lorsque c'est l'objet du test.

Documenter ce qui est simulé, les effets réels, les prérequis et les limites observées.
Un test est utile s'il vérifie une propriété importante : retour d'un sous-graphe,
isolation, refus d'une entrée incorrecte, reprise, conflit ou panne. Une succession
d'assertions qui répète le builder n'apporte pas cette preuve.

Le `FixtureModel` est volontairement simple et déterministe. Il exerce l'intégration
de `LlmAgent` dans `AgentNode`, mais ne mesure pas la qualité d'un modèle. Le routeur
textuel `RESEARCH` est une convention de lab ; un flow plus réaliste pourra tester
les sorties structurées et leurs chemins d'échec sans figer un contrat Zedflow.

## Observabilité disponible

Les commandes research, agent et memory impriment les événements du stream ADK en
JSONL. Pour conserver une trace locale :

```sh
mkdir -p .lab
cargo run --locked -- agent > .lab/agent.jsonl
```

Le mode debug expose les événements du graphe lancé. La granularité des événements
internes d'un `SubgraphNode` dépend d'ADK : cette trace n'est pas encore la traçabilité
hiérarchique complète voulue pour Zedflow. Le flow research est aussi exécutable
seul pour inspecter ses étapes. La sortie ne fournit pas encore un snapshot versionné
de composition, un registre ni une analyse causale des frictions.

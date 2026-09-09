# Comprendre les flows avec la vue web

L'inspecteur du lab affiche les graphes compilés et permet de relire leurs
exécutions. Le code des flows reste leur unique définition. Cette interface est
un petit outil du laboratoire ; elle n'est pas ADK Studio et ne convertit pas les
flows vers le format JSON de Studio.

## Lancer

```sh
export CARGO_TARGET_DIR=/tmp/zedflow-adk-target
cargo run --locked --features web -- web
```

Ouvrir <http://127.0.0.1:3141> sur l'appareil qui exécute la commande. Le serveur
écoute par défaut sur l'interface locale. Depuis un autre appareil, transmettre
le port via la connexion SSH au DGX, par exemple si l'alias `dgx` y est configuré :

```sh
ssh -N -L 3141:127.0.0.1:3141 dgx
```

Puis ouvrir la même URL dans le navigateur de cet appareil. `--listen` permet
aussi de choisir une adresse et un port explicites. Cet inspecteur n'a pas
d'authentification et est destiné à l'exploration locale des fixtures.

Pour un accès direct sur un réseau privé, on peut lier le serveur à l'adresse
Tailscale de la machine : `cargo run --locked --features web -- web --listen
<IP_TAILSCALE>:3141`, puis ouvrir `http://<IP_TAILSCALE>:3141` depuis un appareil
autorisé sur ce réseau. Le tunnel SSH ci-dessus suppose le serveur lancé avec
l'adresse locale par défaut.

Les checkpoints de l'interface sont conservés dans `.lab/viewer.db`. Un autre
emplacement est possible avec `--database 'sqlite:///chemin/lab.db?mode=rwc'`.
Les notes de mémoire restent en RAM jusqu'à l'arrêt du serveur.

## Parcours de lecture

1. Sélectionner une expérience à gauche pour voir ses connexions et son entrée.
2. Cliquer sur un nœud pour retrouver son nom dans la source Rust compilée.
3. Modifier l'état d'entrée JSON et exécuter le flow.
4. Choisir un événement dans la chronologie ou utiliser les commandes de lecture.
5. Comparer le nœud sélectionné, le dernier état confirmé et l'événement brut.

Les couleurs distinguent les nœuds disponibles, terminés dans le parcours affiché
et sélectionnés. La lecture anime une trace **déjà enregistrée** ; elle ne ralentit
pas le runtime et ne simule pas un stream en direct. Les données fixture permettent
de se concentrer sur le comportement et les frontières de la composition.

Pour la mémoire : écrire une note, puis lancer avec `{"query":"graph"}` dans le
même périmètre pour la retrouver sans la réécrire. Changer le périmètre pour
observer l'isolation. Pour le checkpoint : exécuter puis utiliser « Reprendre ».
Le serveur restaure la valeur préparée et continue à `deliver`.

L'export JSON conserve la topologie, les événements, l'entrée et le dernier
checkpoint. Les traces récentes restent accessibles en changeant de flow dans
l'onglet ouvert. Elles ne sont pas rechargées automatiquement après actualisation
de la page ; le stockage SQLite reste disponible mais une bibliothèque de runs
n'est pas implémentée.

## Ce qui est dérivé du runtime

`GraphAgent::topology()` fournit les membres et leurs connexions depuis le
`CompiledGraph` réellement construit. Le serveur appelle les mêmes constructeurs
`flows::*::build` que les expériences existantes. Aucun dessin manuel ne décrit
une seconde version du graphe.

Le mode debug ADK donne les événements de nœuds et de routage. Comme les instantanés
de valeurs utilisent un autre mode de stream, l'inspecteur lit les checkpoints
réellement sauvegardés entre les événements et les ajoute à la chronologie avec
`type: checkpoint` et `origin: adk_checkpointer`. Il n'exécute pas le graphe une
seconde fois pour obtenir l'état. Les autres événements sont les événements ADK.

La source présentée est embarquée dans le binaire avec `include_str!`. Après une
modification dans Zed, arrêter et relancer `cargo run` pour recompiler le code,
puis actualiser la page. Un rechargement du navigateur seul ne recompile pas Rust.

Si le lab est lancé par l'unité utilisateur `zedflow-lab-web.service`, recompiler
avec `CARGO_TARGET_DIR=/tmp/zedflow-adk-target cargo build --locked --features web`,
puis utiliser `systemctl --user restart zedflow-lab-web.service`. L'unité doit
pointer vers ce binaire ; `systemctl --user cat zedflow-lab-web.service` permet de
le vérifier. Pour l'arrêter : `systemctl --user stop zedflow-lab-web.service`.

## Limites utiles à garder en tête

- L'export de topologie ADK ne fournit pas les libellés des conditions ni les
  sorties `END`. Il représente les routes possibles entre les membres ; les
  décisions prises sont visibles dans les événements du run.
- Un sous-graphe apparaît comme un nœud d'appel. Le bouton d'exploration ouvre le
  flow de recherche séparément ; il ne prétend pas montrer la trace interne du
  sous-graphe de ce run parent.
- Les états sont confirmés aux checkpoints. Ils ne montrent pas toutes les
  mutations internes d'un nœud ni les données brutes d'un service mémoire.
- Le serveur utilise uniquement le modèle fixture. Les appels Gemini restent
  accessibles par le CLI explicite `agent --live`.
- Les requêtes sont exécutées jusqu'à la fin ou la pause avant de retourner la
  trace. Le lab reste adapté à ces petites expériences ; pas d'annulation ou de
  gestion de longues exécutions en arrière-plan dans cette première vue.

## Vérification

```sh
cargo test --locked --features web --all-targets
cargo clippy --locked --features web --all-targets -- -D warnings
```

Les tests HTTP vérifient que les nœuds observés appartiennent au graphe affiché,
que les snapshots sont présents, que les erreurs sont rapportées, que la mémoire
respecte les projets et qu'une reprise restaure l'état sans répéter la préparation.

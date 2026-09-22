# Packages de flows

Le writer utilise `.zedflow/flow/<id>/` (singulier), dans le workspace ou la
racine globale explicite. Les quatre racines historiques `.zedflow/flows` et
`.agents/flows`, locales et globales, restent lisibles. Modifier un fichier
legacy propose une conversion explicite ; aucune réécriture silencieuse.

Un package formatVersion=1 contient `flow.json`, son entrée `flow.rs`, un README
optionnel et les modules/assets déclarés. Le manifeste inventorie les chemins
relatifs et dépendances locales. Ce format est distinct des flows Rust v1–v4,
des bridges, des stratégies et des archives de sessions.

```sh
REPO="$(pwd -P)"
WORKSPACE="$(mktemp -d /tmp/zedflow-authoring.XXXXXX)"
export CARGO_TARGET_DIR=/tmp/zedflow-adk-target
(cd rust && cargo run --locked -p zf-cli --bin zf -- --workspace "$WORKSPACE" init flow demo)
(cd rust && cargo run --locked -p zf-cli --bin zf -- validate "$WORKSPACE/.zedflow/flow/demo")
(cd rust && cargo run --locked -p zf-cli --bin zf -- compile "$WORKSPACE/.zedflow/flow/demo")
(cd rust && cargo run --locked -p zf-cli --bin zf -- export "$WORKSPACE/.zedflow/flow/demo" --output "$WORKSPACE/export")
```

`init bridge` et `init context` créent les formats actuels dans leurs répertoires
existants ; ces artefacts ne deviennent pas des packages de flow. Les commandes
refusent l'écrasement, n'activent rien et ne lancent aucune exécution.

Identités séparées : id du flow, nom Cargo, chemin du package, révision de contenu,
révision des dépendances et hash exact du Rust exécuté. L'empreinte de package
couvre les octets de l'inventaire trié et la fermeture résolue des dépendances :
modifier un asset change la révision même si `flow.rs` reste identique. Les pins
d'admission utilisent cette révision ; l'historique conserve son hash source.

La découverte ne résout pas le réseau. Les chemins absolus/traversal, liens
sortants, références non figées et fichiers non déclarés produisent des diagnostics.
Les modules non éditables visuellement restent conservés. README n'accorde aucune
capacité. Les publications utilisent verrou catalogue, staging, rename et journal
de récupération ; les exports comprennent leur lock Cargo et les sources figées.

Conversion et import ne réexécutent aucun nœud ni effet, ne réécrivent pas les
commandes/texte libre et ne changent pas les définitions des passages historiques.
Une conversion personnelle exige sauvegarde et mandat distinct de cette documentation.


## Import des anciennes compositions SQLite

Une base contenant encore la table historique `compositions` demande un import
explicite avant admission des exécutions. Arrêtez les processus qui utilisent
cette base et conservez un snapshot cohérent de ses données et fichiers associés.
La cible est un workspace explicite ; les chemins ci-dessous sont des paramètres
à remplacer par des chemins absolus et canoniques existants.

```sh
zf --workspace /chemin/du/workspace migrate-compositions --data /chemin/des/donnees --flow-home /chemin/global --dry-run
zf --workspace /chemin/du/workspace migrate-compositions --data /chemin/des/donnees --flow-home /chemin/global
```

Le dry-run vérifie les sources, les identités et les destinations sans publier de
flow, de sauvegarde ou de reçu. L’import valide toutes les entrées avant la
publication, crée une sauvegarde SQLite cohérente comprenant le WAL, puis publie
les packages par le mécanisme de staging existant. La table `compositions`, les
sessions, les événements et les checkpoints restent conservés ; aucun nœud
n’est exécuté.

Le reçu `legacy-compositions-import.json`, dans le dossier de données, distingue
import incomplet et terminé. Un import interrompu peut laisser des packages déjà
publiés : le daemon refuse alors l’admission tant que la même commande n’a pas
repris et vérifié l’ensemble. Une source changée ou une destination différente
produit un conflit sans écrasement. Un import terminé n’est pas rejoué et ne
recrée pas les packages supprimés ou modifiés ensuite. Il ne fixe pas le
workspace d’entrée des prochains démarrages du daemon.

La sauvegarde SQLite de l’import ne remplace pas le snapshot complet précédant
la bascule. Un ancien binaire ne découvre pas nécessairement le répertoire
`.zedflow/flow/` et ignore les éditions ultérieures de ces packages. Le retour
arrière garanti restaure donc, à l’arrêt, le binaire et le snapshot complet
compatibles. Un simple changement de binaire ne revient pas sur les données.

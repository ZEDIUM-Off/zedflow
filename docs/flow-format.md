# Flows Rust structurés et packages

Un flow éditable par Zedflow possède une entrée Rust structurée. Les nouveaux
flows sont des [packages formatVersion=1](development/flow-packages.md), distincts
des versions du Rust structuré. Les fichiers legacy restent lisibles. Sa topologie est
définie par les appels ADK et ses configurations par les valeurs littérales
réellement passées aux primitives. Le fichier ne contient pas de document JSON
du graphe caché dans une chaîne ou dans un fichier annexe.

Le contrat est implémenté dans `rust/crates/zf-flows/src/flow_source.rs` :

- `render(&Composition, &dyn SourceValidator)` produit la forme Rust canonique d'une composition valide.
- `parse(&str, &dyn SourceValidator)` charge le sous-ensemble reconnu sans compiler ni exécuter le code.
- `zf-compiler::export::export_single` valide la composition avec ses primitives
  et conserve les octets de sa source dans `src/flow.rs`,
  avec les modules et le manifeste Cargo nécessaires à son exécution autonome.

Le stockage des fichiers et les révisions de sauvegarde relèvent du magasin de
flows. Le format ne constitue pas une base de données de définitions. Le graphe
chargé en mémoire sert au canvas et au compilateur ADK existant.

## Structure du fichier

Chaque module commence par `const ZEDFLOW_FORMAT: u32 = 1`. Les commentaires
`// @zedflow` identifient les métadonnées ; ce sont les constantes Rust qui en
portent les valeurs. Les commentaires ne modifient pas la sémantique du flow.

La version 2 utilise `const ZEDFLOW_FORMAT: u32 = 2` et le champ `formatVersion: 2`
dans la projection du canvas. Une version absente dans une composition historique
vaut 1. Le parent et ses sous-graphes utilisent la même version. Le lecteur et
le générateur v1 restent disponibles ; changer seulement la constante d'un
fichier v1 ne constitue pas une conversion valide.

```rust
// @zedflow flow-view: identity, name, revision
const FLOW: (&str, &str, i64) = ("review", "Relecture", 3);

// @zedflow node-view 1: identity, renderer, label, x, y
const NODE_1: (&str, &str, &str, f64, f64) =
    ("reviewer", "flow", "Relecteur", 280.0, 120.0);

// @zedflow edge-view 0: identity, source handle, label
const EDGE_0: (&str, Option<&str>, Option<&str>) =
    ("start-reviewer", None, Some("Commencer"));
```

`NODE_n` conserve l'identifiant, le type de rendu, le label et la position. Son
identifiant est aussi celui utilisé pour enregistrer le nœud ADK. `EDGE_n`
conserve l'identifiant du lien, sa poignée de sortie et son label ; il ne contient
pas ses extrémités. La poignée `true` ou `false` est utilisée par le routeur natif
d'un nœud conditionnel.

Dans `build_scope`, les variables `node_n` référencent les identifiants des nœuds
ou les sentinelles natives `START` et `END`. Cela conserve l'identité de plusieurs
terminaux graphiques sans introduire plusieurs nœuds de fin exécutables ADK.
Les extrémités des liens sont des paires Rust utilisées par les appels ADK :

```rust
let node_0 = START;
let node_1 = NODE_1.0;
let edge_0 = (node_0, node_1);
graph = graph.add_edge(edge_0.0, edge_0.1);
```

Les indices sont contigus et suivent l'ordre du document graphique. Le nom, la
position et les labels ne servent pas à identifier les nœuds lors de l'exécution.

## Configuration et primitives

Les configurations utilisent `json!` avec des **valeurs littérales Rust** :

```rust
let config_1 = json!({
    "modelBinding": "runtime",
    "tools": ["read", "write", "edit", "exec"],
    "systemPrompt": r#"Relire le changement et expliquer les risques."#
});
```

Objets à clés chaîne, tableaux, chaînes normales ou brutes, booléens, `null`,
entiers et nombres décimaux finis sont admis. Les clés dupliquées sont refusées.
Les grands entiers doivent porter un suffixe `i64` ou `u64` lorsque leur valeur
dépasse `i32`, car `json!` évalue de vraies expressions Rust. Le générateur ajoute
ces suffixes. Les suffixes non reconnus, calculs, appels, expansions de macros et
accès à l'environnement dans une configuration sont refusés. Les chaînes sont
échappées pour Rust, y compris les caractères de contrôle ; elles ne sont pas
simplement copiées depuis une sérialisation JSON.

Les paramètres `channels` et `settings` sont des valeurs littérales effectivement
consommées par `operations::state_schema` et `operations::configure`. Les canaux
privés de reprise sont dérivés du graphe par le générateur et vérifiés par le
parseur. Un retry par nœud lit la valeur `retry` de la configuration de ce nœud.

Les nœuds agent appellent `models::node_with_services`. Les autres opérations
appellent `operations::execute_with_services` dans un `add_node_fn` ADK. Les liens
conditionnels et les arrivées `fanIn: "any"` utilisent `Edge::Conditional` natif.
Les constructions de fonctions, closures et routeurs ont une forme précise,
vérifiée intégralement au chargement.

Le catalogue pris en charge est celui du compilateur actuel : `start`, `end`,
`set`, `output`, `input`, `condition`, `agent`, `subgraph`, `tool`, `context`,
`steering` et `inbox`. Il conserve notamment les bindings de modèles fixes ou
runtime, les outils, les paramètres de modèle, les renderers de résultat,
les canaux et reducers, les politiques de retry, les timeouts et la concurrence.
Il conserve aussi les propriétés supplémentaires des configurations actuellement
acceptées par le compilateur ; cela ne leur attribue pas un comportement nouveau.

## Sous-graphes et exécution

Un sous-graphe est un module Rust inline `mod subgraph_n { ... }` qui suit le même
format. Sa configuration locale ne contient pas de clé `composition` : sa
topologie existe dans son module et est reconstruite en mémoire au chargement.

`build(services, checkpointer)` appelle `build_scope(services, checkpointer, "")`.
Les appels enfant propagent le chemin des nœuds, les services de run et le
checkpointer partagé. `ResumableSubgraph` conserve les checkpoints par occurrence,
les réponses aux attentes et les accusés de réception durables. Le format
n'introduit ni interpréteur Rust ni autre moteur d'exécution.

Le daemon charge le fichier en `Composition`, puis utilise son compilateur ADK.
L'export compile la fonction `build` du fichier exact et les mêmes modules
`models`, `operations`, `runtime`, `workspace_context`, `workspace_tools`,
`subgraphs` et `codex`, avec ADK-Rust fixé à 2.2.0. Il fournit les options CLI
`--workspace`, `--home`, `--data`, `--run-id`, `--models` et `--input` déjà utilisées par
l'export runtime. Garder `--data` et `--run-id` permet de reprendre les checkpoints.

## Frontière d'édition

Le parseur lit un AST `syn`, extrait les éléments reconnus, valide la composition,
puis compare **l'ensemble de l'AST** à la forme attendue. Il ignore les différences
d'espacement, les commentaires ordinaires et les virgules finales facultatives.
Les configurations littérales acceptent aussi les chaînes brutes et l'ordre
différent des clés. `rustfmt` est compatible avec la forme produite.

Les éléments suivants restent hors format : Rust libre, helpers utilisateur,
imports supplémentaires, modules externes, closures de routage arbitraires,
conditions ou boucles Rust qui construisent le graphe, attributs supplémentaires,
expressions calculées et macros autres que celles de la forme reconnue. Un fichier
hors format produit un diagnostic ; le chargement ne le lance pas et ne le
réécrit pas. Il peut rester visible dans le catalogue pour consultation/correction.

Une modification de configuration peut être faite dans le code puis rechargée.
Une modification structurelle doit garder cohérents les indices, les appels ADK,
les métadonnées et les canaux privés dérivés. Le canvas produit cette cohérence
automatiquement. Il ne promet pas de reconstruire visuellement du Rust arbitraire.

L'export garde les octets du fichier validé, y compris les commentaires libres et
sa mise en forme. Une sauvegarde depuis le canvas régénère la forme canonique :
elle peut reformater le fichier et remplacer les commentaires libres. La détection
des éditions concurrentes est assurée par la révision du magasin de flows.

Les limites du compilateur restent applicables : 250 nœuds par composition, huit
niveaux d'imbrication, attentes simultanées indépendantes refusées, et contraintes
ADK de récursion des sous-graphes. Le fichier est limité à 2 Mio et une
configuration littérale à 64 niveaux d'imbrication. Aucun support de Rust libre
n'est implicite dans la version 1.

## Preuves de parité

Les tests internes de `flow_source` couvrent l'aller-retour du catalogue complet,
les métadonnées, deux niveaux de sous-graphes, les canaux/réglages/retries, une
édition littérale effectivement exécutée et le refus de code inconnu sans effet.
La validation supplémentaire suivante compile aussi le catalogue et le fichier
exporté exact, puis exécute sans réseau un harness imbriqué : attente de modèle,
boucle d'outil, attente humaine et reprise, sans rejouer l'effet précédent.

```sh
(cd rust && ZEDFLOW_TEST_CODEGEN=1 CARGO_TARGET_DIR=/tmp/zedflow-adk-target \
  cargo test --locked -p zf-runtime --test portable_export --offline)
```

## Capacités explicites de la version 2

Un agent v2 sans `attachments` ne reçoit ni instructions du workspace, ni catalogue
de skills, ni fichiers de contexte, ni outils implicites. Les anciennes propriétés
`instructions`, `globalInstructions` et `tools` ne lui accordent rien. Les quatre
pièces sont des configurations de cet agent, pas des étapes du graphe :

```json
{
  "attachments": {
    "instructions": {"items": [
      {"id": "project", "source": {"kind": "workspace"}},
      {"id": "mission", "source": {"kind": "text", "text": "Relire {{input}}"}, "mode": "template"}
    ]},
    "skills": {"items": [
      {"id": "catalog", "source": {"kind": "workspace"}, "activation": "explicit"}
    ]},
    "files": {"items": [
      {"id": "notes", "path": "notes.md", "startLine": 1, "endLine": 20, "maxChars": 32000}
    ]},
    "tools": {"items": [{"id": "read", "name": "read"}]}
  }
}
```

Chaque ressource possède un `id` unique dans l'agent et `enabled` vaut `true` par
défaut. Les instructions acceptent les sources `text`, `file` (avec `path`) et
`workspace` (instructions découvertes dans le snapshot du workspace). Leur mode
par défaut est `literal` : les substitutions du flow ne s'appliquent que si
`mode: "template"` est explicitement choisi. Les fichiers et les corps de skills
restent toujours littéraux. Les chemins relatifs se résolvent depuis le cwd du
run ; les chemins absolus sont admis selon les droits de l'utilisateur du daemon.

Un skill accepte une source `workspace` ou `file`, avec un éventuel filtre `name`.
Sa description est annoncée dans le catalogue attaché ; le corps n'est chargé
que si son activation le demande. L'activation vaut `always` par défaut pour
instructions/fichiers, `explicit` pour skills. Les activations sont propres au
chemin de l'agent : `itemId` pour instructions/fichiers, `itemId::skillName` pour un
skill. Ni les métadonnées d'un skill ni son activation n'accordent d'outils.

Les fichiers UTF-8 lus pour une injection sont limités à 1 Mio. La plage de lignes
est inclusive, les fins de ligne sont conservées, puis `maxChars` borne la portion
effectivement injectée (32 000 caractères par défaut, au plus 1 048 576). Chaque
invocation capture son contexte effectif, contenu compris, avant d'appeler le
modèle ; le total injecté est limité à 2 Mio. Les snapshots durables indiquent
agent, occurrence, ressources, hashes et troncature. Modifier ensuite un fichier
ne modifie pas le snapshot de l'appel précédent.

`modelResponse.contextSnapshotId` référence cet instantané ; le corps complet
reste dans les fichiers `capability-snapshots` et dans `run.contextSnapshots` pour
l'inspecteur et l'export. Le graphe ne transporte pas une nouvelle copie de ces
contenus dans chaque état, checkpoint et entrée d'activité suivant l'appel.

Les appels d'outils retournés par un modèle sont enregistrés avec leurs arguments,
leur agent demandeur et les outils accordés à cet appel. `execute_next_call` et
`execute_calls` revérifient ce registre avant tout effet ; modifier la file d'appels
du graphe ne peut ajouter une autorisation. Les reçus d'effets utilisent l'identité
de l'invocation et de l'appel, même si un autre nœud d'exécution reprend la demande.
Les nœuds outils programmés directement dans le flow restent exécutables.

## Conditions v2 et conversion explicite

Une condition v2 conserve exactement les sorties `true` et `false`. Sa propriété
`predicate` contient soit une comparaison, soit un groupe non vide `all` (ET) ou
`any` (OU) de prédicats. Exemple :

```json
{"kind":"all","items":[
  {"kind":"compare","field":"/input/count","operator":"gte","value":3},
  {"kind":"compare","field":"/input/tags","operator":"contains","value":"ready"}
]}
```

Les opérateurs sont `eq`, `ne`, `gt`, `gte`, `lt`, `lte`, `exists`, `contains` et
`in`. Un champ sans `/` initial désigne une clé exacte de l'état ; un champ
commençant par `/` est un JSON Pointer. Une valeur absente rend une comparaison
fausse ; `exists` distingue l'absence de la présence d'une valeur `null` et ne
prend pas de propriété `value`. Aucune conversion chaîne/nombre/booléen n'est
effectuée. Les comparaisons numériques conservent les entiers JSON de grande
taille ; l'inclusion accepte les chaînes ou les tableaux. Toutes les branches
d'un groupe sont évaluées, afin qu'une incompatibilité de type soit un diagnostic
d'exécution même si une autre branche détermine déjà le booléen. Le nœud ADK
évalue le prédicat puis son routeur natif consomme le booléen produit.

`convert_v1` produit une copie v2 avec nouvel identifiant et révision zéro. Il
transpose les instructions et outils en pièces, les égalités en prédicats, et
convertit les sous-graphes. Un ancien nœud `context` peut être retiré seulement
s'il a une entrée et une sortie simples, sans configuration spéciale ni référence
à son état interne. Les jonctions, branchements et configurations ambiguës sont
refusés avec diagnostic. Le contrôle du reste du graphe est conservé ; les règles
v2 (notamment les types stricts des conditions et l'activation explicite des
skills) s'appliquent à la nouvelle copie. Cette opération ne réécrit ni le fichier
source de départ, ni les sessions déjà lancées, ni leurs checkpoints.

L'export v2 inclut les mêmes modules `agent_capabilities` et `predicates` que le
daemon. Le test d'export v2 exécute un appel d'outil d'agent dans un sous-graphe,
vérifie les snapshots et la provenance enregistrée, puis reprend l'attente
humaine sans répéter l'effet. L'ancien test d'export v1 reste distinct.

L'exécutable exporté accepte `--capabilities` avec un objet JSON (ou le chemin de
son fichier), par exemple `{"child/model":["catalog::review","notes"]}`. Ces
activations sont persistées avec le run et restaurées à sa reprise, comme les
choix transmis par `--models`. Une liste vide désactive les ressources explicites
de l'agent désigné. Les ressources `always` restent actives selon la définition
du flow.

## Préparation explicite et répertoire du flow

Le format v3 accepte les nœuds `context` et `model` séparés. La source compilée
appelle respectivement `models::context_node_with_services` et
`models::inference_node_with_services` ; le lecteur valide leur paire et sa
connexion directe. Les fichiers historiques avec `agent` et `context` v1 restent
lisibles et exécutables. Voir les [contrats du contexte](context-api.md).

`settings.workingDirectory` est facultatif. S’il est présent, la source Rust
configure les services du flow via `for_working_directory`. Les chemins relatifs
sont résolus depuis le workspace ou le cwd hérité du sous-graphe. Le changement
est local à ces services, sans `set_current_dir` ni nouvelle base de reçus. Un
dossier absent ou un fichier à la place d’un dossier produit une erreur explicite.

## Boucle du Harness et publication intermédiaire

Le starter « Harness de workspace » utilise le format v3 et suit ce parcours :
`Début → Réorientations → Routage du travail → Contexte → Modèle`.
Après le modèle, la condition `hasToolCalls` conduit aux outils ou à la réponse
finale puis à l’inbox. Chaque retour des outils repasse par les réorientations,
le routage et le contexte. Le texte d’un modèle ne décide pas de l’attente humaine.

Le point `work` est exposé aux bridges par le nœud `dispatch`, de kind `route`
et d’invocation `condition`. Sans bridge actif ni sélection de route explicite,
il suit sa connexion locale. Dans une composition résolue, zéro route éligible
produit la même continuation ; plusieurs routes éligibles constituent une erreur.
Une route obligatoire sans candidat ou une sélection `routeId` absente de ce
point reste une erreur. Une route conditionnelle connue dont la garde est fausse
conserve la continuation locale, même lorsqu’elle a été sélectionnée explicitement.
Le contrat du point précise une entrée et une sortie texte.

La sortie est enregistrée dans `routeResult`. La configuration `fallback: ""`
remet ce canal à vide lorsqu’aucune route n’est empruntée, pour éviter de réutiliser
un résultat ancien. `harness-default`, sauvegardée dans `.zedflow/context/`, reprend
les instructions, skills, fichiers, conversation et capacités du workspace et
ajoute explicitement ce résultat lorsqu’il est non vide. Les autres stratégies
conservent leurs contrats. Un branchement exposé comme outil au modèle doit être
déclaré et accordé séparément ; il n’est pas confondu avec ce point conditionnel.

Lorsqu’un nœud `model` termine un appel contenant du texte et des demandes d’outils,
son texte est publié avec l’identité du passage avant les événements des outils.
Cette publication ne suspend pas le graphe. La réponse sans appel d’outil reste
publiée par le nœud `output`. La file d’observation ordonne ces éléments ; elle
n’attend pas que le navigateur les affiche pour continuer l’exécution.

L’éditeur ne propose plus de créer un nœud `agent`. Une définition historique
reste consultable et peut être convertie en copie avec une paire Contexte/Modèle.
La mise à jour de la définition du Harness concerne les nouveaux runs : les
sources, canaux et checkpoints d’une ancienne session ne changent pas de format
pendant sa reprise.

# API du contexte et des compositions

Les interfaces d’auteur valident les stratégies et résolvent les contrats sans
exécuter de flow. Les interfaces de lancement utilisent ensuite ces définitions
avec ADK. Une stratégie explicitement sélectionnée construit la requête de son
inférence ; les anciens nœuds v1/v2 gardent leur comportement historique. Le
périmètre et les lots sont décrits dans [le plan](context-engine.md).

## Catalogue Rust

| Requête | Effet |
|---|---|
| `GET /api/context-strategies?workspaceId=…` | Liste les fichiers du workspace, y compris ceux accompagnés de diagnostics |
| `GET /api/context-strategies/{key}?workspaceId=…` | Lit la définition, les octets Rust, leur hash et les diagnostics |
| `POST /api/context-strategies` | Valide et sauvegarde une stratégie dans `.zedflow/context/<id>.rs` |
| `POST /api/context-strategies/validate` | Vérifie les exigences d’une stratégie contre les types et capacités disponibles |
| `POST /api/context-strategies/preview` | Évalue une fenêtre depuis des ressources fixtures explicitement fournies |
| `POST /api/runtime-graphs/resolve` | Résout les contrats de flows/bridges et valide les stratégies sélectionnées |
| `GET/POST /api/context-libraries` | Liste ou sauvegarde les fonctions et projections dans `.zedflow/context/libraries/` |
| `GET/POST /api/context-types` | Liste ou sauvegarde les types nommés dans `.zedflow/types/` |
| `GET /api/context-source-types?workspaceId=…` | Décrit les types de sources disponibles pour composer une stratégie, leurs provenances et leurs dépendances nommées |
| `GET/POST /api/bridges` | Liste ou sauvegarde les bridges dans `.zedflow/bridges/` |
| `GET /api/context-readers` | Décrit les lecteurs natifs disponibles et leurs contrats |
| `POST /api/examples/working-system` | Crée les définitions manquantes du multiflow documentaire (`workspaceId`, `workingDirectory`), sans lancer de run |

Le workspace omis désigne celui ouvert par défaut par le daemon. Un identifiant
inconnu est refusé. Les sources sont locales au workspace : ce premier catalogue
n’ajoute aucune recherche globale implicite. Il ne parcourt pas les sous-dossiers.

Une sauvegarde fournit `workspaceId`, `strategy`, le registre `types` si nécessaire,
et `expectedHash` pour remplacer un fichier existant. Une création omet le hash.
Les types nommés propres à la stratégie sont conservés dans `strategy.types` et
générés en Rust par `.define_type(nom, type)`. Le registre `types` fourni à la
sauvegarde y est fusionné : il ne disparaît pas après la validation. Les
`requirements` désignent les ressources attendues ; ils restent indépendants des
bindings d’un flow. L’aperçu, la validation et l’inférence fusionnent les types
embarqués avec les types externes explicitement liés. Deux définitions identiques
d’un même nom sont acceptées ; deux définitions différentes produisent
`type_conflict` sur `types.<nom>`, sans choisir silencieusement l’une des deux.
Le champ `strategy.types` est facultatif et omis lorsqu’il est vide ; les anciennes
sources et stratégies figées restent lisibles sans conversion.

Les remplacements entre auteurs coopérants sont sérialisés par verrou fichier puis
publiés avec une intention durable de mise à jour des runs. La récupération après
interruption termine cette publication sans faire régresser un head plus récent.
Les dépendances inverses des flows et des runs sont validées avant écriture.
Une source externe invalide reste consultable avec son diagnostic.
Le lecteur accepte un sous-ensemble Rust de constructeurs documentés ; il n’exécute
ni code arbitraire, ni producteur, ni macro utilisateur.

Exemple de source produite :

```rust
// @zedflow-context 1
use zedflow_daemon::harness::context::*;
use zedflow_daemon::harness::types::DataType;
use std::collections::BTreeMap;

pub fn strategy() -> ContextStrategy {
    ContextStrategy::new("request-only", "Demande explicite")
        .require("request", DataType::Text)
        .with_program(vec![
            ContextBlock::emit("request", FragmentRole::Data, FragmentFormat::Text,
                ContextExpr::resource("request")
            ),
        ])
}
```

Cette source est un artefact Rust compilable contre les modules publics du daemon.
Le lecteur conserve ses contraintes de forme, notamment les imports et la fonction
`strategy`. L’édition n’est donc pas celle d’un module Rust arbitraire.

Les nouveaux programmes utilisent `// @zedflow-context 2` et
`ContextStrategy::new_v2`. La source v1 ci-dessus reste lisible et garde sa
sémantique, y compris l’interprétation historique de JSON lié aux messages ADK.
En v2, `FragmentFormat::Json` reste une donnée structurée et
`FragmentFormat::AdkMessages` demande explicitement une séquence de messages,
avec vérification des couples appel/résultat d’outil. Une projection JSON d’un
historique ne se transforme donc pas automatiquement en conversation.

`ContextExpr::construct("Terme", expression)` construit explicitement une valeur
d’un type nommé. La forme produite doit satisfaire le type déclaré, sans conversion
implicite de ses champs. Les types métier restent distincts même à structure égale.
`POST /api/context-strategies/convert` prépare une copie v2 depuis une stratégie v1,
ses bindings et les choix de format explicites. Les représentations ambiguës sont
signalées ; l’éditeur propose « Créer une copie v2 » sans changer la source v1.

## Catalogue des types de sources

`GET /api/context-source-types?workspaceId=…` renvoie `{entries, diagnostics}`.
Chaque entrée décrit un **contrat de donnée**, pas une donnée déjà acquise :

| Champ | Contenu |
|---|---|
| `id`, `label` | Identité du choix de catalogue et nom affichable |
| `category` | `messages`, `tools`, `documents`, `execution`, `data`, `media` ou `custom` |
| `type` | Type de la ressource, au format `DataType` |
| `types` | Définitions nommées nécessaires à ce type, avec ses dépendances transitives |
| `origin`, `providers` | Provenance de la définition et fournisseurs décrits ; aucun grant ni binding implicite |

Le catalogue réunit les types constructibles intégrés, les catalogues
`.zedflow/types`, les types embarqués des stratégies enregistrées, les types et
données publiques lisibles des flows découverts, ainsi que les lecteurs standard
dont la sortie possède un type fixe. Il ne lit pas les valeurs d’un run.

Les contrats intégrés distinguent notamment `Instructions`, `UserMessage`,
`ModelOutput`, `ToolDefinition`, `ToolCall`, `ToolResult`, `Document`, `Skill`,
`Passage`, `Run` et `State`, ainsi que les primitives et médias. Par exemple,
`Document` porte `title`, `content`, `path` ; `ToolResult` porte `callId`, `status`,
`content`, tous de type texte. Ce sont des formes à fournir ou à construire
explicitement dans un flow : leur présence dans le catalogue ne garantit pas que
l’état natif d’un nœud possède déjà cette forme. Le statut d’un résultat reste une
valeur texte, sans liste d’états imposée par ce type.

Les identités de catalogue sont préfixées par leur provenance : `builtin:…`,
`catalog:<clé>:<type>`, `strategy:<clé>:<type>`, `flow:<clé>:data:<nom>`,
`flow:<clé>:type:<type>` ou `reader:<id>`. Elles distinguent les choix de sources ;
le nom d’un type métier reste son identité dans le registre de la stratégie.
Les conflits de noms entre catalogues produisent `type_identity_conflict`. Chaque
entrée conserve sa propre définition ; les dépendances impossibles à résoudre
produisent un diagnostic et l’entrée concernée n’est pas proposée. Importer des
définitions contradictoires dans une stratégie reste refusé par la fusion stricte.

Sélectionner une entrée consiste à déclarer une ressource dans `requirements` et
à conserver les définitions nécessaires dans `strategy.types`. L’auteur fournit
ensuite des valeurs d’essai pour l’aperçu, ou un binding explicite sur le nœud
Contexte. Les blocs du programme choisissent enfin les champs et représentations
qui entrent dans la fenêtre.

## Parcours, listes et extraits de texte

Le bloc `forEach` évalue ses blocs enfants pour chaque élément d’une liste typée,
dans l’ordre de la liste. Il peut produire plusieurs fragments ou groupes par
élément, imbriquer des conditions et contenir d’autres parcours :

```json
{
  "kind": "forEach",
  "id": "each-document",
  "value": { "kind": "resource", "name": "documents" },
  "item": "document",
  "items": [
    {
      "kind": "emit",
      "id": "excerpt",
      "role": "data",
      "format": "text",
      "value": {
        "kind": "truncate",
        "value": {
          "kind": "field",
          "value": { "kind": "variable", "name": "document" },
          "field": "content"
        },
        "count": 1200
      }
    }
  ]
}
```

Ici `documents` doit être déclaré comme une liste dont chaque élément expose un
champ texte `content`. `item` nomme une variable lexicale accessible avec
`{kind: "variable", name: "document"}` seulement dans les enfants du parcours.
Une boucle imbriquée peut masquer ce nom ; la valeur et sa provenance extérieures
sont restaurées à la sortie. Une liste vide ne produit rien. Une liste absente
crée un besoin lorsqu’elle est consultée ; une valeur incompatible produit un
diagnostic. Les valeurs sources restent partagées, les projections dérivées
peuvent allouer.

L’expression `{kind: "list", itemType, items: [expressions…]}` construit une liste
ordonnée depuis des expressions. `itemType` est obligatoire, même pour une liste
vide ; chaque élément doit avoir un type compatible. Elle permet par exemple
d’assembler des `parts` puis des messages ADK à partir de champs dynamiques.
Le format de fragment `adkMessages` reste nécessaire pour les envoyer comme
conversation en v2 ; une liste JSON reste une donnée JSON.

`{kind: "truncate", value, count}` exige un texte et en conserve au plus `count`
valeurs scalaires Unicode, sans casser UTF-8 ni ajouter de suffixe. Il ne compte
ni les octets, ni les tokens, ni les graphèmes ; zéro produit un texte vide.
`take` conserve sa sémantique distincte de sélection des premiers éléments d’une
liste. Aucun nombre ou objet n’est implicitement converti en texte.

Les constructeurs Rust correspondants sont `ContextBlock::for_each(id, value,
item, items)`, `ContextExpr::list(item_type, items)` et
`ContextExpr::truncate(value, count)`. Le lecteur et le générateur conservent ces
constructions dans la source exacte. Leurs ajouts ne changent pas la sémantique
des formats de stratégie v1/v2 ni le format des flows.

Les limites d’évaluation couvrent opérations, éléments produits, entrées de trace
et octets dérivés ou émis, y compris les parcours imbriqués. Les valeurs par défaut
sont 100 000 opérations, 10 000 éléments et 16 Mio. Un dépassement produit
`evaluation_limit` avec `complete: false`, jamais une fenêtre tronquée déclarée
complète.

## Prévisualisation et validation

Une sélection est soit un brouillon (`kind: "draft"`, `strategy`), soit un fichier
figé (`kind: "file"`, `key`, `hash`). Le hash empêche de prévisualiser silencieusement
une définition modifiée après sa sélection. La réponse contient la stratégie et la
source exacte choisies, avec leur hash.

Exemple de corps pour `POST /api/context-strategies/preview` :

```json
{
  "selection": {
    "kind": "draft",
    "strategy": {
      "version": 1,
      "id": "request-only",
      "name": "Demande explicite",
      "requirements": { "request": { "kind": "text" } },
      "program": [
        {
          "kind": "emit",
          "id": "request",
          "role": "data",
          "format": "text",
          "value": { "kind": "resource", "name": "request" }
        }
      ]
    }
  },
  "resources": { "request": "Relire la définition de scope." }
}
```

`evaluation.items` est la fenêtre structurée. Chaque fragment contient son
identifiant d’occurrence, sa représentation et ses ressources sources. `complete` vaut
faux si une donnée utilisée manque ou si une valeur est incompatible. `needs`
décrit les ressources manquantes et les blocs consommateurs ; ce résultat ne lance
pas leur production. Une branche inactive ne demande pas ses ressources. Tester la
présence d’une ressource absente permet d’évaluer la branche alternative.

`evaluation.trace` relie les occurrences évaluées au programme, sans recopier les
contenus des ressources :

| Champ | Sens |
|---|---|
| `id`, `blockId` | Identité de l’occurrence et identifiant du bloc écrit par l’auteur |
| `path` | Chemin d’évaluation, incluant les positions des itérations |
| `iterations` | Pile `{blockId, index}` des parcours englobants, indices à partir de zéro |
| `sources` | Noms des ressources consultées par ce bloc et ses enfants exécutés |
| `outputIds` | Identifiants des éléments directement produits par cette occurrence |
| `outcome` | Pour un bloc `if` dont le prédicat est évalué : `true` ou `false` |

Les enfants d’une branche inactive n’ont pas d’entrée de trace. Un bloc peut avoir
une trace sans sortie, notamment une condition ou une ressource manquante.
`evaluation.reads` rassemble les ressources consultées, y compris les tests de
présence infructueux. Les consommateurs doivent relier les objets par identifiant,
sans assimiler l’ordre du tableau de trace à l’ordre des fragments.

Hors parcours, l’identifiant d’occurrence reste celui du bloc. Dans un parcours,
il distingue chaque répétition et reste déterministe pour le même programme et
les mêmes positions d’itération ; il ne représente pas une identité métier stable
après réordonnancement de la collection. Le lien vers le bloc vient de `blockId`,
sans analyser la chaîne `id`. La trace est un ajout au résultat d’évaluation ; le
codec des éléments des anciennes fenêtres reste inchangé et n’acquiert pas de
correspondance historique inventée.

Pour `/validate`, `resources` contient les **types exposés**, et non les valeurs.
Toutes les exigences doivent y être déclarées ; une donnée déclarée peut ensuite
être indisponible à un passage. Les types nommés sont résolus dans le registre
fusionnant `strategy.types` et les `types` externes. Les comparaisons ne
convertissent pas implicitement texte, nombres ou types métier.

`/preview` est autonome : il évalue les seules valeurs fixtures fournies et renvoie
les capacités demandées dans `evaluation.capabilities`. Il n’exige aucun grant de
flow. L’ancien paramètre `grantedCapabilities` reste accepté par cet endpoint mais
est ignoré. L’aperçu n’acquiert pas implicitement de fichiers, d’historique ni de
ressources personnelles, et n’exécute aucun outil ou modèle.

Pour `/validate`, `grantedCapabilities` décrit les droits disponibles dans le
scénario de liaison. Une capacité demandée mais absente produit
`capability_not_granted`. La résolution, la compilation du flow et l’inférence
conservent leurs contrôles stricts : schémas des outils réels, grants de l’agent et
provenance durable avant effet. Ni le succès d’un aperçu ni sa liste de capacités
demandées ne confèrent ces droits.

## Résolution de composition

Le corps de `/api/runtime-graphs/resolve` contient :

- `catalog` : `types`, `flows` et `bridges`, indexés par identité de définition ;
- `request` : `flow`, `entry` et la liste des `bridges` activés ;
- `contexts` : sélections de stratégies par chemin d’inférence résolu, par exemple
  `root/writer` ou `documentation/reviewer/agent` ;
- `workspaceId` si le catalogue de stratégies d’un autre workspace est choisi.

Un flow déclare `entries`, `branches`, `data`, `requires` et `inferenceNodes`.
Un bridge déclare `requires`, `imports`, `connections` et `bindings`. Ses endpoints
utilisent `root` ou un alias local d’import. Pour utiliser une instance d’un autre
bridge, l’import spécifie `reuse` et déclare ce bridge comme dépendance. Sans `reuse`,
chaque import crée une instance indépendante.

Les contrats d’entrée/sortie emploient `input` et `output`. Les expositions et
exigences de données emploient `dataType` et `permissions: {read, write}`. Une liaison
ne contient que des endpoints et permissions : aucune valeur de dataset n’est copiée
dans le plan. Les permissions demandées doivent rester dans l’exposition source ;
la vérification des types tient compte du sens de lecture et d’écriture.
Dans ce premier lot, les droits sont lecture seule ou lecture et écriture. Un
contrat d’écriture seule est refusé explicitement, car le registre ne sait pas
encore matérialiser ce droit sans autoriser également la lecture.

La réponse `stage: "resolved"` contient `graph` et les `contexts` liés. Une stratégie
déclarée par `contextStrategy` sur une inférence est chargée depuis le catalogue si
aucune sélection explicite ne la remplace. Un nœud sans stratégie sélectionnée
reste visible dans le plan pour configuration ultérieure. Un modèle `runtime`
reste également à sélectionner. Ce plan de contrats se distingue de la capture
des fichiers effectuée par `/api/runtime-graphs/prepare`, puis du lancement ADK.

`POST /api/runtime-graphs/prepare` reçoit `workspaceId` et `selection` : `flow`
(clé du fichier), `entry`, `bridges`, `flowHashes`, `bridgeHashes` et, facultativement,
`contexts`. Ce dernier dictionnaire associe un chemin d’inférence résolu à
`{key, hash}` pour remplacer le choix de stratégie de cette instance. Ses bindings,
types et bibliothèques restent ceux du nœud et doivent satisfaire la nouvelle
stratégie. Les fichiers d’origine ne sont pas modifiés. Les références sélectionnées
sont vérifiées avant lancement ; un hash périmé provoque un conflit.

La réponse `stage: "prepared"` contient `runtime` : graphe résolu, sources Rust
exactes des instances, exports et pins des définitions. `POST /api/runs` reçoit
la même sélection dans `runtimeSelection`, ainsi que `workspaceId`, `input` et
`modelBindings` indexés par chemin d’inférence. Il capture de nouveau les pins avant
de démarrer ADK. Les choix de stratégie par instance sont conservés lors d’une
sauvegarde ultérieure du flow, et leurs sources suivent les publications acceptées.

Les routes prennent les modes `callAwait`, `launch` et `handoff`. Un lancement
asynchrone conserve son propriétaire runtime quand le parent attend une réponse.
`runtimeActive` peut ainsi rester vrai avec `status: "waiting"`. L’export de session
est refusé avec HTTP 409 jusqu’à une frontière où aucun acteur ne travaille.
Les inputs parent et enfant
reprennent leurs checkpoints et identités d’attente propres.

Les exigences invalides produisent HTTP 422 avec `diagnostics` (`code`, `path`,
`message`). Les conflits de source produisent HTTP 409. Une erreur de fichier ou
une requête incorrecte produit HTTP 400. Les parseurs JSON peuvent retourner HTTP
422 avant la validation métier si le corps ne correspond pas au contrat.

## Registre de données Rust

`harness::data::DataRegistry` réutilise le `ContentStore` et son pool SQLite. Un
registre désigne un univers runtime et doit être cloné entre ses lecteurs afin de
partager son cache. Il fournit `create`, `grant`, `snapshot`, `revision` et `publish`.

Le snapshot porte `entity_id`, `revision`, `parent_revision`, `content_ref` et une
valeur partagée `Arc<Value>`. Les scopes `Flow`, `Bridge` et `Runtime` n’hébergent que
des alias. `publish` exige la révision attendue. Une vue en lecture seule ne peut
pas publier ni se redonner un accès en écriture. L’hôte garde la responsabilité du
choix des scopes ; ces namespaces ne sont pas un sandbox pour du Rust arbitraire.

Les tests publics sont dans `harness_data`, `harness_types`, `harness_composition`,
`harness_context`, `context_authoring_api` et `harness_integration`. Ils utilisent
des bases temporaires, des fixtures et les mêmes interfaces que les consommateurs.

La validation borne l’imbrication des types à 64 niveaux et leur expansion à
4 096 éléments. Le second plafond couvre les références nommées qui décrivent un
arbre exponentiel malgré un petit fichier source. Les limites produisent un
diagnostic ; elles ne tronquent pas silencieusement le contrat.

## Limites de lecture et de profondeur

La profondeur métier et celle de sa représentation sont distinctes. Un type
`record` ajoute un objet `fields` à chaque niveau JSON : un schéma de 64 niveaux,
avec l’enveloppe de requête, peut donc dépasser 128 conteneurs JSON.

Le [codec de contexte](../rust/crates/zf-context/src/context_json.rs) accepte au plus
256 conteneurs imbriqués dans le document JSON complet. Les requêtes d’auteur et
les requêtes contenant une composition utilisent ce codec, notamment sauvegarde,
validation, génération Cargo, aperçu de flow et lancement. Le lecteur Rust des
flows applique également une limite de représentation de 256 niveaux à chaque
configuration JSON. Ces plafonds ne relèvent pas la limite métier de 64.

Avant la désérialisation récursive, un parcours itératif vérifie les conteneurs ;
la ponctuation contenue dans les chaînes, y compris leurs échappements, ne compte
pas comme imbrication. Le lecteur Rust vérifie séparément la complexité des tokens
avant l’analyse `syn`, sans compter le contenu des littéraux ou des commentaires.
Ces précontrôles rejettent les corps JSON trop imbriqués et les sources Rust trop
complexes avant leur analyse récursive. Les entrées profondes admises utilisent
une pile dédiée ; l’AST Rust et sa destruction restent sur cette pile. La
validation métier intervient ensuite :
un type de 65 niveaux reste refusé avec diagnostic.

Les régressions couvrent les lectures JSON/Rust à 32 et 64 niveaux dans
[harness_context_depth](../rust/crates/zf-context/tests/harness_context_depth.rs), les échanges
HTTP de sauvegarde, réouverture et aperçu dans
[context_authoring_api](../rust/crates/zf-serve/tests/context_authoring_api.rs), ainsi que
les refus métier 65 et représentation 257 dans
[flow_context_depth](../rust/crates/zf-serve/tests/flow_context_depth.rs). Les tests internes
du lecteur de contexte couvrent aussi les grands programmes et les sources
excessivement imbriquées. Ces limites de lecture n’impliquent pas une exécution
sans copies ; les budgets d’évaluation restent ceux décrits plus haut.

## Sources et fenêtres des inférences

Un nœud associe `contextStrategy: {key, hash}` à `contextBindings`. Ses types et
projections réutilisables sont sélectionnés par `contextTypesRef` et
`contextLibraryRef`. Le programme capturé conserve les octets Rust, les hashes et
les bindings ; le runtime ne dépend plus de la présence des fichiers d’auteur pour
rejouer sa préparation. Les ressources externes effectivement lues restent, elles,
des dépendances du prochain passage.

Les bindings disponibles sont `state` (champ et JSON pointer facultatif),
`attachment` (pièce et skill explicites), `entity` (scope, alias, révision),
`produced` (producteur ADK déclaré) et `reader`. Exemple de ce dernier :

```json
{
  "kind": "reader",
  "reader": "file.json",
  "input": { "kind": "state", "field": "source", "pointer": "/document" }
}
```

L’entrée peut aussi être `{kind: "literal", value: …}`. `file.text` et `file.json`
attendent `{path}` ; `sqlite.json` attend `{path, table, id}` et lit uniquement une
table de documents `id/document`, en lecture seule. `content.text` et `content.json`
attendent `{contentRef}` pour retrouver un contenu canonique, par exemple une
sortie complète d’outil. Le contrat de sortie est un type fixe ou le type JSON
déclaré par la ressource, validé sans coercition. Les sources sont bornées à 16 Mio.

Les lecteurs sont acquis à la demande depuis l’état réel du passage. Une branche
inactive ne déclenche pas sa lecture. Une liaison `reader` décrit une acquisition ;
une liaison `produced` demande au producteur explicitement relié de fournir une
ressource indisponible. Ce producteur peut lui-même dépendre de lecteurs. Une erreur
de lecture active est signalée avant l’appel modèle. `ResourceReader` et `ReaderRegistry` permettent à l’hôte Rust
d’ajouter un lecteur natif avec contrats et provenance. Le catalogue de sources
n’exécute pas du Rust importé pour installer une extension.

| Interface de run, avec `workspaceId` | Usage |
|---|---|
| `GET /api/runs/{id}/context-windows` | Index léger des fenêtres, origines et révisions |
| `GET /api/runs/{id}/context-program?nodePath=…&hash=…` | Programme effectivement lié, sources et dépendances |
| `GET /api/runs/{id}/context-window?nodePath=…&alias=…&revision=…` | Contenu d’une fenêtre précise |
| `PATCH /api/runs/{id}/context-window` | Applique des patches avec `id`, `nodePath`, `alias`, `expectedRevision` |
| `POST /api/runs/{id}/context-window/select` | Sélection explicite de la fenêtre pour le prochain passage compatible |

Les patches gardent leurs identités et révision cible ; leur origine se retrouve
dans la commande d’interface ou la provenance d’appel d’outil. Groupes et fragments
peuvent être remplacés, déplacés ou retirés. `strategyRevision` identifie la source
de stratégie ; `programRevision` identifie l’ensemble effectivement lié, dont types,
bibliothèques et bindings. La sélection d’une fenêtre utilise cette seconde identité.
Un flow éditeur agit par les mêmes contrôles de révision, avec les droits donnés
par le bridge ; ses ressources et sa propre stratégie restent indépendantes.

## Publication, inspection et test de brouillon

Les sauvegardes de flows, stratégies, types, bibliothèques et bridges préparent
leurs publications avant l’écriture du fichier. La transaction SQL de publication
est idempotente ; le marqueur de maintenance permet de terminer le raccord après
une interruption. Les sources externes ne sont pas adoptées silencieusement : leur
acceptation explicite suit les hashes et les mêmes validations inverses.

`GET /api/runs/{id}/definition` accepte `nodePath`, `occurrenceId` ou `hash`. La
réponse porte la source et la définition réellement utilisées, le hash, l’instance
et le `graphRef`. Une ancienne occurrence sans origine exacte renvoie une limite
explicite, sans fabriquer de correspondance. `/revisions` décrit les heads publiés,
les révisions actives, les changements en attente et les diagnostics par scope.
`POST /api/generate` avec `runId`, `workspaceId` et la même sélection exporte cette
version ; sans sélection, il utilise le dernier passage identifié.

`POST /api/runs/preview` reçoit `workspaceId`, `composition`, `input` et
`modelBindings`. Il crée une exécution de brouillon dans un workspace temporaire,
après capture des sources choisies du workspace d’origine. Il ne sauvegarde pas
la définition éditée et ne publie pas ce brouillon dans les autres runs. Ses outils
ont ce répertoire courant ; ce test n’est pas un sandbox. La définition d’origine
reste accessible à la demande par `/api/runs/{id}/preview-source`.

## Packages et archives

`POST /api/context-packages/export` reçoit `workspaceId` et `artifacts`, une liste
de sélections de stratégies, types, bibliothèques et bridges. Le package contient
les sources et leurs hashes ; ses prérequis de flows restent explicites.
`/api/context-packages/validate` et `/import` reçoivent `{workspaceId, package}`.
L’import vérifie formats, hashes et collisions avant publication ; la réimportation
identique est idempotente. Aucun flow, modèle ou lecteur natif n’est exécuté.

Les archives de sessions v3 embarquent les définitions réellement utilisées,
registres, contenus référencés, fenêtres, reçus et checkpoints. Les lecteurs des
archives v1/v2 restent disponibles. Une source externe absente peut produire un
diagnostic informatif dans `import.resourceDiagnostics` ; le prochain passage
détermine si cette ressource est nécessaire. Les effets externes incertains gardent
leurs obstacles à la reprise, indépendamment de ce diagnostic de ressource.

Les exports Cargo contiennent les modules runtime, les stratégies et les types
figés ainsi que les sources des flows et bridges de la composition. Les lecteurs
standard sont embarqués ; un lecteur natif privé non embarqué provoque une erreur
d’export. Les essais compilent et exécutent ces sources avec des modèles fixtures,
y compris une reprise dans un nouveau processus.

Ils embarquent aussi le codec JSON borné, le lecteur Rust protégé et les options
de compilation nécessaires à la représentation des contextes profonds. La limite
de récursion des macros de la crate générée permet de compiler ces sources sans
assouplir la validation métier. Le test
`exact_cargo_bundle_executes_with_context_schema_depth_64`, activé par
`ZEDFLOW_TEST_CODEGEN=1` dans `flow_context_depth`, compile le bundle exact puis
l’exécute avec un modèle fixture et des données de 64 niveaux. Ces changements
n’altèrent ni les versions d’archives ni la limite de 512 niveaux du magasin CAS.

## Paire de nœuds Contexte / Modèle

En flow Rust v3, `kind: context` porte `modelNode`, `contextStrategy`,
`contextBindings`, les pièces sources et les capacités. `kind: model` porte
`contextNode` et les paramètres d’inférence. Les références doivent être réciproques,
avec une seule connexion directe entre eux et aucun contournement à l’arrivée du
modèle. L’ancien nœud `agent` et le contexte v1 conservent leur sémantique.

Le passage Contexte persiste un `prepared-requests` immuable ; son checkpoint
contient seulement son identifiant. Le passage Modèle vérifie l’identité du
consommateur, la configuration préparée et la non-consommation de cette préparation.
Une adoption de révision incompatible attend que la préparation pendante soit
consommée. Le manifeste relie l’inférence à son passage de préparation.

Deux bindings explicites complètent les sources :

- `{"kind":"attachments","slot":"instructions|skills|files"}` collecte une seule
  catégorie accordée. Le slot skills comprend le catalogue et les seuls contenus
  activés ; il n’accorde aucun outil.
- `{"kind":"conversation","historyField":"messages","inputField":"input"}`
  compile la conversation avec la demande non consommée, à partir des marqueurs
  d’entrée. Un résultat d’outil ne rajoute pas l’ancienne demande. Ce binding reste
  un choix de stratégie, pas un historique implicitement fourni à tout modèle.

Le Modèle conserve la conversation canonique et sa nouvelle réponse. Les fichiers
injectés et les projections de préparation ne sont pas réenregistrés dans cet
historique à chaque tour.

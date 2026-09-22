# Zedflow : intention et vocabulaire produit

## Décision

Zedflow explore un harness agentique natif graphe, modulaire et évolutif, construit
sur les primitives d'ADK-Rust. Le reboot abandonne la séquence « port Pi puis
LangGraph ». Aucun comportement Pi, sidecar Python ou runtime LangGraph n'est à
reproduire. L'historique Git conserve les anciens travaux ; leur vocabulaire produit
est adapté ci-dessous, sans reconduire les contraintes de l'ancienne architecture.

ADK-Rust fournit la base d'exécution. La contribution recherchée de Zedflow porte
sur la composition, le partage et la capitalisation de comportements exécutables,
avec une résolution précise du harness et des ressources qu'il expose.

## Le graphe est le programme de comportement

Les décisions et transitions significatives du harness s'inscrivent dans une
structure d'exécution explicite, y compris la boucle agentique la plus simple.
Un agent peut entrer dans un graphe de recherche, recevoir son résultat, puis
continuer sa boucle sans connaître les outils ou les étapes internes de cette
recherche. Le graphe peut contenir des cycles, du routage conditionnel, des appels
de modèles et du code adaptatif. Une structure explicite ne prédétermine ni le
chemin effectivement suivi ni la durée des appels.

Les graphes doivent pouvoir étendre les comportements **et les ressources** :
état, stores, services et interfaces partagées au niveau d'une composition, d'un
run, d'un workspace ou d'un périmètre déclaré. Cette possibilité ouvre notamment
la coopération sur une mémoire, un tableau de tâches ou un ensemble de preuves.
Elle nécessite des règles explicites de visibilité, de durée de vie et de partage.

## Ce qui se capitalise

Un flow de recherche, un flow de validation Rust, une procédure TypeScript, une
partie de ces flows ou leur assemblage peuvent être conservés, partagés et raffinés.
Un skill peut être une source de contexte qu'un nœud charge selon un résultat
précédent. Il n'est pas l'unité obligatoire de capitalisation.

La définition du graphe, sa composition résolue et les traces des runs doivent
pouvoir être reliées : versions, nœuds exécutés, entrées/sorties utiles, gates,
échecs, coûts et résultats. Cela aide à localiser les frictions et à comparer des
modifications ciblées. Une trace localise un échec observé ; elle ne démontre pas
à elle seule sa cause ni la supériorité d'une variante.

## Résolution et continuité

À terme, un registre permettra de choisir les graphes et leurs versions. La
configuration applicable sera résolue depuis le répertoire de lancement, avec une
priorité pratique aux racines de workspace et des règles explicables. Le nom du
fichier de configuration, sa syntaxe et un éventuel langage restent ouverts.
Ces choix de résolution sont distincts du [format Rust des flows](docs/flow-format.md)
déjà pris en charge par l'application.

Un nouveau run résout une composition. Le runtime reprend la définition et l'état
enregistrés et adopte les révisions publiées à la prochaine étape compatible :
chaque passage conserve sa définition exacte et une incompatibilité d'état garde
l'ancienne révision avec diagnostic. Les frontières ADK prises en charge et les
restrictions structurelles sont définies dans le
[plan du context engine](docs/context-engine.md#révisions-et-navigation).

## Méta-harness

L'amélioration peut elle-même être un graphe composable, exécuté par les mêmes
primitives et susceptible d'être amélioré. Il n'est pas nécessaire de créer deux
moteurs ou deux boucles permanentes. Il faudra cependant distinguer la version
active, la variante candidate et les observations qui justifient son adoption.

Les évaluateurs, optimiseurs et politiques d'adoption automatiques viennent après
le laboratoire. L'utilité du premier socle doit être tangible avant l'auto-amélioration :
lancer, comprendre, composer, reprendre et modifier des flows précis.

## État de réalisation

Le [plan du context engine et des compositions](docs/context-engine.md) précise
les décisions du 12 septembre 2026 et les lots de leur réalisation : stratégies
Rust, bridges, données par références et adoption de révisions. Les raccords ADK,
les éditeurs et les interfaces de partage y sont décrits ; les résultats des tests
effectivement exécutés appartiennent à [validation](docs/validation.md).

Le produit est réparti entre neuf crates dans `rust/`, un SDK TypeScript Zod,
des adaptateurs Vue, un client web et une coque Electron dans `web/`. Les frontières
sont décrites dans [l'architecture](docs/architecture/crates.md). `zf-execution`
porte le cycle de vie hors HTTP, `zf-compiler` compile sans stockage ni réseau,
et ADK-Rust reste le moteur des graphes. La CLI `zf` et le daemon utilisent ce socle.

Les définitions nouvelles sont des [packages de flows](docs/development/flow-packages.md).
Les flows Rust historiques restent lisibles ; conversion et import sont explicites,
les définitions exécutées et les effets passés ne sont pas réécrits. Distribution
générale, optimiseur et auto-amélioration ne sont pas implémentés par cette migration.

Ce document porte les définitions du vocabulaire Zedflow. Les règles de conception
sont développées dans [composition](docs/composition.md),
[ressources](docs/resources.md), [évolution](docs/evolution.md) et
[sessions et interfaces](docs/interaction.md). La [base ADK](docs/adk.md) porte les
versions et primitives retenues ; les documents de l'application et du format
portent les capacités implémentées et leurs limites.

## Vocabulaire technique

Les définitions décrivent l'intention produit, sauf mention explicite de
l'implémentation actuelle. Une exigence de validation ou de comportement dans ce
glossaire n'est pas une garantie déjà fournie par ADK ou par l'application.
Les noms techniques sont conservés pour retrouver les concepts établis ; leurs
définitions françaises font référence pour le reboot.

La restitution part de `pi-port/checkpoint-2026-09-09:CONTEXT.md`, commit
`00948328`, et intègre les précisions du reboot et de l'interaction. Les concepts
abandonnés ou renommés sont indiqués en fin de document. La terminologie n'impose
ni un nouveau moteur d'exécution ni une couche d'abstraction pour chaque flow.

### Flows et composition

**Flow** : comportement spécialisé dont l'orchestration est exprimée par un
graphe. Il peut être agentique, déterministe ou mixte, avec ou sans appel modèle.
Sa responsabilité, ses entrées, ses sorties et ses effets doivent être lisibles.
À distinguer d'une simple instruction de prompt ou d'une boucle agentique opaque.

**Root Flow** : flow choisi comme point d'entrée d'un harness. Il exprime sa boucle
et ses routes et accueille les comportements spécialisés de sa composition.
Le Root Flow d'un harness conversationnel prévoit un retour à l'entrée humaine ;
ce rôle ne prescrit ni une boucle Pi ni une racine unique pour toutes les compositions.
À distinguer d'un External Flow ou d'une primitive spéciale du runtime ADK.

**External Flow** : flow spécialisé intégré à une composition appelante, notamment
autour d'un Root Flow. Son identité produit ne dépend pas de sa réalisation comme
sous-graphe ADK, nœuds intégrés au parent ou exécution distincte explicitement lancée.
À distinguer d'un script de plugin, d'une macro d'outil ou du seul `SubgraphNode` ADK.

**Standalone Flow** : flow exécutable seul, avec ses propres entrées, sorties et
ressources déclarées. Il peut être interactif ou fonctionner sans interface humaine.
À distinguer d'un flow uniquement utilisable dans un graphe appelant.

**Embeddable Flow** : flow invocable par un autre flow à travers un contrat stable
d'entrées et de sorties. Les mappings et ressources nécessaires sont explicites.
À distinguer d'un flow dépendant implicitement de l'état interne de son appelant.

**Composable Flow** : flow exposant des points de connexion et des contrats qui
permettent de le relier à d'autres flows définis indépendamment, par des bridges.
Standalone, embeddable et composable sont des capacités cumulables.
À distinguer d'un assemblage dont tous les liens sont codés dans les flows sources.

**Flow Bridge** : définition de composition séparée entre des flows indépendants,
portant les imports, points de connexion, mappings de données, dépendances et
politiques utiles. Ses branchements font apparaître des routes dans le graphe résolu.
Plusieurs bridges peuvent relier les mêmes flows, y compris avec des conditions
simultanément satisfaites, si le branchement ou le parallélisme résultant est explicite
et valide. Leur définition ne nécessite pas de modifier les flows sources.
À distinguer d'un transfert implicite ou d'une simple arête interne à un flow.

**Flow Route** : chemin résultant d'un branchement défini par un bridge actif.
Sa présence dans le Runtime Graph, son éligibilité à un instant donné et son
invocation effective sont trois états distincts. Elle peut être appelée comme
outil, étape du flow ou branche conditionnelle. Le bridge et la route ne sont
pas des synonymes.

**Flow Composition** : assemblage de flows, bridges, politiques et bindings de
ressources. La composition complète résolue pour l'exécution constitue la surface
finale de validation, au-delà de la validité de chacun de ses composants.
À distinguer d'un flow isolé ou d'une accumulation de plugins par ordre de chargement.

**Flow Bundle** : artefact sauvegardé, versionné et partageable contenant un flow
ou une composition, ses dépendances, ses interfaces et ses points d'intégration
prévus dans une composition appelante. Un bundle utilisable seul peut aussi exposer
un point d'entrée autonome. Le bundle n'est pas nécessairement tout le Runtime Graph.
À distinguer d'un export de diagramme ou d'un format de sérialisation particulier.

**Flow Definition** : source sauvegardée définissant le comportement exécutable
d'un flow ou d'une composition, écrite par une personne, un agent ou un outil.
Dans l'application actuelle, c'est un fichier Rust
structuré utilisant les primitives ADK dans le sous-ensemble reconnu par le
[format](docs/flow-format.md). La représentation de travail du canvas est dérivée
de cette source ; elle ne constitue pas un document JSON caché qui gouvernerait
l'exécution. L'édition visuelle ne prend pas en charge le Rust arbitraire.
À distinguer d'un diagramme, d'un état de run ou du graphe compilé en mémoire.

**Runtime Graph** : composition concrète résolue pour une exécution à partir de
son point d'entrée, de ses flows, bundles, bridges, politiques et ressources.
Son exécution s'appuie sur `adk_graph::StateGraph`, `CompiledGraph` et les formes
de composition ADK retenues. Il peut intégrer des nœuds ou sous-graphes ; cette
forme technique ne redéfinit pas les unités partageables.
À distinguer d'un bundle individuel ou de la seule définition du Root Flow.

**Graph-Native Agent Loop** : boucle agentique dont les appels modèle, les demandes
et exécutions d'outils, les routes, les retours et les attentes sont inscrits dans
le graphe. L'application utilise des invocations `adk_core::Llm` et laisse les
transitions au graphe ; utiliser ADK n'implique pas de déléguer toute la boucle à
`LlmAgent` ou à `Runner`.
À distinguer d'une orchestration uniquement exprimée dans un prompt.

**Harness Determinism** : caractère explicite et déterministe des règles de
contrôle autour d'appels modèle ou outil éventuellement non déterministes : points
d'invocation, ordre contraint des actions, routage, erreurs et contexte exposé.
À données et politiques identiques, une décision de routage doit être explicable.
Cela ne garantit ni les réponses du modèle, ni la durée des appels, ni un parcours
unique quand les résultats ou la politique de concurrence permettent plusieurs suites.

### API, ressources et résolution

**Zedflow API** : surface d'API visée pour déclarer les ressources, composer les
flows, résoudre les scopes, valider les Runtime Graphs et intégrer événements,
stores, diagrammes et interfaces. Ses contrats fortement typés doivent rendre
les usages vérifiables et rester proches des primitives Rust et ADK utilisées.
Les endpoints du daemon actuel n'en constituent qu'une réalisation partielle.
À distinguer d'un registre de plugins sans contrats ou d'une configuration JSON seule.

**Flow Composition API** : partie de la Zedflow API consacrée à l'assemblage de
flows indépendants : bridges, contrats d'état, ressources, routage, parallélisme,
attentes typées, budgets, liens d'exécution et inspection. Elle doit s'appuyer sur
les primitives de composition ADK et ajouter les garanties produit nécessaires.
À distinguer d'un second ordonnanceur ou de la réécriture du harness dans chaque flow.

**Zedflow Extension Surface** : ensemble des API, points d'extension et ressources
par lesquels des modules peuvent fournir des flows, bundles, bridges, stores,
fournisseurs de modèles ou d'outils, interfaces et traitements d'événements.
La disponibilité d'une crate ADK de plugin ne réalise pas à elle seule cet ensemble.
À distinguer d'une extension limitée aux seuls flows.

**Zedflow Store** : ressource de données exposée aux flows avec un contrat explicite
d'accès, de visibilité et de durée de vie. Elle peut être privée, partagée dans une
composition ou rattachée à un run, workspace ou utilisateur. Sa persistance au-delà
d'un run ou d'un processus dépend du backend et de sa déclaration ; SQLite n'est
pas imposé par le concept. Les services ADK de mémoire ou d'artefacts peuvent
contribuer à certaines réalisations sans couvrir tous les usages d'un store.
À distinguer de l'état du run, d'un checkpoint et du contexte effectivement injecté
dans un appel modèle. Les règles de ressources appartiennent à [resources.md](docs/resources.md).

**Store Provider** : backend ou adaptateur fournissant un Zedflow Store, avec ses
garanties de stockage, accès, concurrence, isolation et persistance. Son binding
permet de fournir une ressource appropriée sans changer la responsabilité du flow.
À distinguer du contenu du store ou d'un backend codé implicitement dans chaque flow.

**Runtime Graph Scope** : ensemble déclaré des ressources et paramètres à résoudre
pour une composition et une exécution : point d'entrée, versions des flows et
bundles, bridges, politiques, configuration et services requis ou fournis.
Il est qualifié par le périmètre concerné, notamment le workspace ou l'utilisateur.
À distinguer d'un dossier, d'un bundle unique ou de la visibilité d'une clé d'état.

**Runtime Graph Scope Resolution** : résolution des références et bindings du
scope en une composition explicite et validée. Le parcours répertoire de lancement,
configuration applicable, versions et ressources choisies doit être retraçable.
Une reprise utilise la composition associée au run ; changer de version demande
une opération distincte. La découverte actuelle de fichiers ne constitue pas
encore cette résolution générale.
À distinguer d'une fusion par ordre de chargement ou d'un scan implicite non qualifié.

**Runtime Event Interface** : surface d'événements reliant l'exécution ADK et ses
observations aux consommateurs Zedflow : étapes, modèles, outils, état, checkpoints,
attentes, erreurs et filiation. Les événements du graphe et les événements d'agents
ADK sont intégrés selon leur source et leur granularité, avec les identités de run
et d'occurrence nécessaires au suivi. Les projections d'interface et les commandes
adressées au runtime restent distinctes des faits d'exécution.
À distinguer d'un Flow Bridge, d'un adaptateur réservé au TUI ou d'un protocole de transport.

### État et contrats de transition

**Flow State** : état d'exécution associé à un flow, avec ses canaux déclarés,
entrées, sorties, valeurs initiales et reducers. Il s'appuie sur l'état et le
`StateSchema` ADK ; les contrats Zedflow décrivent comment cet état peut être composé.
Dans ADK 2.2.0, les valeurs sont dynamiques : les garanties de type, visibilité et
disponibilité envisagées ici ne résultent pas du seul nom `StateSchema`.
À distinguer d'un store de ressources ou d'un second système d'état d'exécution.

**State Transition Contract** : déclaration des clés et types qu'une étape lit,
produit ou modifie, ainsi que des conditions nécessaires à leur consommation en
aval. La composition doit vérifier ces exigences contre les canaux et reducers
résolus et n'exposer que les clés visibles et disponibles au point considéré.
Les validations de valeurs qui dépendent de l'exécution restent explicites.
À distinguer d'un couplage implicite entre nœuds ou de la seule déclaration d'une clé.

**Logical State Key** : nom de clé déclaré par l'auteur d'un flow avant résolution
des namespaces, branches, collisions et réservations de la composition.
À distinguer du nom effectivement stocké dans l'état du Runtime Graph.

**Resolved State Key** : identité effective d'une clé après résolution de sa portée,
de son namespace et des éventuelles collisions ou séparations de branches.
La relation avec la clé logique d'origine doit rester traçable.
À distinguer d'un renommage arbitraire ou du seul nom visible dans l'éditeur.

**Runtime State Key** : cas particulier de Resolved State Key généré ou réservé
par la composition pour ses besoins d'exécution et pour éviter les collisions entre
flows indépendants. Sa réservation ne doit pas masquer les clés logiques des auteurs.
À distinguer d'un identifiant de flow ou d'une clé globale partagée implicitement.

**State Key Visibility** : exposition publique ou privée d'une clé à la composition.
Une clé publique peut être reliée à d'autres flows là où elle est disponible ; une
clé privée reste accessible dans le flow ou le périmètre plus étroit qui la possède.
À distinguer d'une permission d'exécution ou d'une frontière de sécurité pour du Rust libre.

**State Key Scope** : périmètre déclarant où une clé peut être lue ou écrite : flow,
nœuds, arêtes ou points de bridge sélectionnés. Il affine la visibilité publique
ou privée, sans constituer une troisième classe de visibilité.
À distinguer de sa durée de vie, d'un sandbox ou d'une convention de nommage seule.

**State Key Availability** : garantie qu'une clé possède une valeur utilisable à un
point du parcours, parce qu'elle a été produite en amont sur ce chemin ou qu'une
valeur initiale ou une garde établit cette condition. L'existence de la clé dans
le schéma ne suffit pas, notamment après des branches alternatives.
À distinguer d'une lecture optionnelle sans contrat ou d'une hypothèse de présence.

**Node State Surface** : ensemble résolu des clés visibles et disponibles à un
nœud, une arête ou une branche pour la composition. Il comprend les sorties directes
et les autres clés publiques encore utilisables à ce point.
À distinguer de tout l'état interne du flow ou des seules sorties du dernier nœud.

### Écritures parallèles et lecture

**Parallel State Write Conflict** : situation où plusieurs branches parallèles
écrivent une même clé publique ou des valeurs destinées à une même clé logique.
La composition doit expliciter la séparation des clés résolues et leur consommation,
ou la fusion des écritures partagées, avant qu'une valeur commune soit lue.
À distinguer d'un écrasement séquentiel intentionnel ou d'une mise à jour ordinaire.

**State Reducer** : règle déclarée de combinaison des écritures d'une clé résolue,
portée par le canal ou choisie au point de composition approprié. Elle s'appuie sur
les reducers ADK, intégrés ou personnalisés, et précise le sens du résultat obtenu.
À distinguer de la politique d'attente des branches et du choix de la valeur à lire.

**Parallel Branch Policy** : politique de coordination de branches parallèles :
attendre toutes les branches, retenir un premier succès, annuler des branches ou
limiter leur durée, selon les capacités disponibles. Les résultats incomplets et
les erreurs doivent avoir un traitement défini. Toutes ces politiques ne sont pas
nécessairement exposées par l'application actuelle.
À distinguer d'un reducer ou d'une stratégie de lecture.

**State Read Strategy** : règle de lecture lorsque plusieurs clés résolues
correspondent à une même clé logique : lire une branche ou un namespace précis,
retenir un résultat sélectionné ou consommer une valeur fusionnée explicitement.
À distinguer de l'ordonnancement des branches et de la fusion elle-même.

**Parallel Write Strategy** : déclaration autorisant intentionnellement les
écritures parallèles relatives à une même clé logique. Elle associe un reducer
explicite à la politique de branches ou de lecture nécessaire ; la séparation des
clés de branches rend leurs résultats identifiables avant une éventuelle fusion.
La validation Zedflow doit rejeter une composition qui laisse ces écritures sans
reducer déclaré. Un `Overwrite` par défaut ne vaut pas déclaration d'intention.
Cette exigence produit n'est pas une garantie automatique du compilateur ADK.
À distinguer d'une course tolérée par avertissement ou d'un dernier écrivain implicite.

### Routage et normalisation

**Routable State** : partie de l'état dont les champs et types sont définis pour
servir aux décisions de routage. Un résultat brut doit satisfaire ce contrat ou
passer par une étape de normalisation avant sa consommation.
À distinguer d'un texte libre ou d'un résultat d'outil arbitraire.

**Routing Input** : champ de Routable State déclaré comme nécessaire à une condition
de routage. Son type, sa visibilité et sa disponibilité doivent être établis au
point de décision, avec une politique explicite lorsque l'absence est autorisée.
À distinguer d'une donnée recherchée implicitement par le routeur.

**Normalization Node** : étape qui transforme une sortie brute en Routable State,
par code déterministe, outil spécialisé ou appel modèle avec Structured Response.
La conversion et sa validation sont observables ; un échec suit un chemin déclaré.
À distinguer d'un parseur ou d'un calcul sémantique caché dans le routeur.

**Routing Condition** : prédicat évalué sur des Routing Inputs déclarés pour
déterminer si une route est admissible. Les règles de comparaison et le traitement
des valeurs absentes ou invalides sont explicites.
À distinguer d'une demande libre du modèle de choisir le prochain nœud.

**Branching Policy** : règle d'interprétation des routes admissibles : exclusivité,
priorité, sélection multiple, parallélisme ou lancement de runs distincts. Une
sélection multiple doit préciser la coordination et les effets sur l'état.
Le parallélisme est une capacité de composition ; sa représentation n'impose ni
n'interdit un nœud dédié lorsque les primitives ADK utilisées le justifient.
À distinguer d'un comportement implicite lorsque plusieurs conditions sont vraies.

**Router Decision** : résultat de l'évaluation des conditions, de la politique de
branchement et de l'état : prochain nœud, flow ou ensemble explicite de destinations.
À distinguer d'une réponse modèle brute, d'un appel d'outil ou de l'exécution de la route.

**Router Node** : étape ne faisant que lire un état existant et produire une Router
Decision. Elle n'exécute ni outil, commande, appel modèle, scoring sémantique ou
inspection de projet. Les actions préparant la décision sont d'autres étapes.
Cette responsabilité peut se traduire par un nœud de condition et un routeur
d'arêtes ADK ; elle n'exige pas une nouvelle primitive du moteur.
À distinguer d'un nœud qui mélange décision de route et production de ses données.

### Modèles, contexte et outils

**Inference Node** : association d'une stratégie de contexte et d'un modèle de
prédiction, dont l'adaptateur déclare les entrées, sorties, modalités et paramètres
acceptés. Le concept couvre les LLM et d'autres familles de modèles. Le runtime
actuel exécute les adaptateurs déjà pris en charge ; déclarer une modalité ne crée
pas implicitement son adaptateur.

**Context Source** : source nommée susceptible d'alimenter un appel modèle :
instructions système, `AGENTS.md`, entrée humaine, sélection d'historique, fichier,
URL récupérée, élément de store, résultat d'outil, skill ou bloc de prompt.
Sa provenance et son contenu effectivement utilisé doivent être identifiables.
À distinguer d'un bloc anonyme ou d'une récupération décidée implicitement par le modèle.

**Context Strategy** : programme partageable réunissant la spécialisation de
l'agent, les capacités qu'il demande et la composition conditionnelle de son
contexte. Sa source est du Rust structuré dans `.zedflow/context`. Il sélectionne
et projette les ressources explicitement disponibles dans le scope de son nœud.
Partager un store ou attacher un catalogue de skills n'injecte pas automatiquement
tous leurs contenus. La stratégie peut préparer une fenêtre neuve ou choisir de
transformer une fenêtre précédente ; stabilité et mémoire résultent de la composition.
Les [interfaces implémentées](docs/context-api.md) décrivent le langage, ses lecteurs,
les fenêtres révisables et leur raccord aux inférences.

**Context Assembly Policy** : ancienne désignation de la politique d'assemblage,
désormais intégrée à la Context Strategy. Les pièces attachées des flows v1/v2
continuent de fonctionner selon leur sémantique historique.

**Graph Messages State** : canaux du Flow State transportant les messages nécessaires
aux appels modèle et au routage. Ils préservent la structure des messages ADK
`Content` et `Part`, les appels et résultats d'outils et les métadonnées nécessaires
au fournisseur. Leur politique de mise à jour doit respecter cette structure ;
un append générique n'est pas nécessairement approprié. L'application remplace
actuellement la valeur de ses canaux d'historique modèle avec l'historique complet.
À distinguer de la conversation affichée, du journal d'événements et du seul stockage
de session. Un reducer conscient des messages n'est pas supposé fourni implicitement.

**Structured Response** : configuration d'un appel modèle demandant une réponse
conforme à un schéma, via les capacités du fournisseur ou une stratégie d'appel
d'outil. Le contrat indique où la conformité est vérifiée et le chemin d'échec.
Transmettre un schéma et obtenir du JSON parseable ne prouvent pas sa validation
complète ; l'application actuelle ne promet pas cette validation pour tout JSON Schema.
À distinguer d'une extraction libre de texte ou d'une coercition silencieuse.

**Tool Call** : demande produite par un modèle pour appeler un outil avec des
arguments et une identité d'appel. Elle peut être refusée, différée ou exécutée
selon la composition et les contrôles applicables.
À distinguer de l'exécution effective ou d'une étape déterministe du flow.

**Tool Exposure Policy** : déclaration des outils présentés au modèle à une étape
donnée, avec leurs schémas, alias locaux éventuels et noms résolus. Elle précise
ce que le modèle peut demander ; l'autorisation d'exécuter et la validation des
arguments sont des contrôles distincts au moment du dispatch.
À distinguer d'une autorisation globale ou d'un accès implicite à tous les outils.

**Tool Execution** : invocation effective d'un outil ou d'une commande par le runtime
Rust, provoquée par un Tool Call ou programmée directement dans le flow. Ses effets,
résultats et erreurs sont observables. L'exécution et sa reprise suivent les règles
d'autorisation et de gestion des effets applicables.
À distinguer de la seule demande du modèle ou du routage qui la suit.

### Validation et inspection

**Bundle Validation** : validation isolée d'un bundle dans une composition de
référence adaptée à ses interfaces, avec un Root Flow minimal lorsqu'il est requis.
Elle vérifie ses contrats et intégrations prévus sans supposer que tous les autres
bundles installés sont présents.
À distinguer d'un lint de package ou de la validation de toute la composition active.

**Runtime Graph Validation** : validation de la composition complète résolue, avec
ses flows, bridges, ressources, politiques et configuration. Elle doit rendre visibles
les incompatibilités, collisions, erreurs de contrats et écritures parallèles non
déclarées, avec des diagnostics attribuables aux composants concernés.
À distinguer d'une validation isolée de bundle ou d'une preuve de qualité des réponses.

**Zedflow Validation Tooling** : commandes et API de validation et d'inspection
produisant des diagnostics structurés et des diagrammes pour les humains, agents
et futurs outils de langage. Leur périmètre de contrôle et leurs limites doivent
être explicites ; compiler le Rust généré ne prouve pas tous les contrats produit.
À distinguer du seul éditeur visuel ou d'un contrôle déclenché uniquement à l'exécution.

**Flow Diagram** : représentation dérivée d'un flow, bundle ou Runtime Graph pour
le comprendre, le relire ou le partager. Mermaid et SVG sont des formats d'export
de référence ; le concept n'impose pas qu'ils soient déjà disponibles dans chaque
interface. Un éditeur peut modifier la définition dont le diagramme est dérivé,
comme le canvas actuel le fait dans les limites du format Rust reconnu.
À distinguer de la source exécutable et de la trace d'un parcours réellement effectué.

### Erreurs et contrôle d'exécution

**Invocation Error** : erreur d'un appel modèle, outil ou fournisseur, avec les
informations permettant de comprendre son origine, son résultat éventuel et les
conditions d'une nouvelle tentative. Elle remplace la catégorie historique
`Pi-Compatible Error` sans obligation de reproduire les messages ou sessions Pi.
À distinguer d'une erreur d'ordonnancement du graphe ou d'une attente humaine normale.

**Graph Runtime Error** : erreur d'exécution du graphe, de routage, de checkpoint,
de reprise, de contrat d'état ou de composition, y compris timeout et épuisement
des tentatives selon la couche qui les applique. Sa provenance doit rester
distinguable d'une erreur du modèle, de l'outil ou de son fournisseur.
À distinguer d'un Budget Interrupt ou d'une pause attendue.

**Failure Path** : route explicite suivie pour traiter une Invocation Error ou une
Graph Runtime Error, selon les règles de retry, timeout, fallback ou arrêt du flow.
À distinguer d'un abandon silencieux ou d'un retour humain prévu par le budget.

**Validation Failure Path** : route explicite suivie lorsqu'une sortie ne satisfait
pas le State Transition Contract nécessaire à sa consommation en aval. Elle peut
conduire à une correction, un nouvel essai, une demande humaine ou un arrêt déclaré.
À distinguer d'une coercition silencieuse ou d'un parsing au mieux.

**Graph Drain** : demande d'arrêt coopératif d'un Runtime Graph à une frontière où
son état et la suite à exécuter peuvent être conservés pour reprise. La politique
doit définir le traitement des nœuds actifs, branches et effets en cours. La frontière
exacte et son intégration aux checkpoints ADK restent à établir ; une annulation
du daemon ne vaut pas automatiquement cette garantie.
À distinguer d'un Budget Interrupt, d'un arrêt brutal ou d'un rechargement de définition.

### Sessions, runs et filiation

**Flow Run** : exécution concrète d'une Flow Definition dans une composition résolue,
avec ses entrées, bindings, état et observations. Sa définition exécutée reste
identifiable même si le fichier source est ensuite modifié.
À distinguer de la définition elle-même, d'une session d'interface ou du seul
`thread_id` utilisé par les checkpoints ADK.

**Zedflow Session** : continuité d'exécution et d'interaction présentée à l'utilisateur,
accessible depuis plusieurs interfaces et reliée à ses événements et points de
reprise. Dans le modèle retenu, seuls les flows interactifs ouvrent des sessions ;
les flows autonomes ont des runs inspectables. Une session peut porter une
composition résolue avec plusieurs instances de flows, selon le
[plan](docs/context-engine.md). Le terme n'impose ni arbre Pi ni équivalence
avec une session du service `adk-session`.
À distinguer d'un onglet, d'une connexion, d'un simple transcript ou d'un Root Flow.

**Execution Binding** : association traçable entre un run, sa définition et sa
composition résolue, les observations de ses étapes et les checkpoints permettant
une reprise. Elle doit identifier l'état et les ressources à retrouver au point
choisi, sans déduire un état restaurable d'un simple message d'historique. Elle
n'impose ni table indexée par entrée Pi ni emplacement physique particulier.
À distinguer d'une clé métier du Flow State ou d'une simple note de provenance.

**Run Reference** : lien vers un autre Flow Run, permettant son identification,
son suivi et, lorsque le contrat le prévoit, la récupération d'un résultat.
À distinguer de la propriété de son état, de ses checkpoints ou de son cycle de vie.

**Session Branch** : branche d'historique utilisateur représentant une continuation
issue d'un point passé. Son lien avec les runs et forks doit être explicite ;
sa réalisation et la structure générale des sessions restent à préciser.
À distinguer d'une branche de routage dans le graphe ou d'une propriété native
supposée de toutes les sessions ADK.

**Resume** : continuation d'une exécution suspendue à partir de son état sauvegardé
et de sa composition associée, avec la réponse ou l'action de contrôle appropriée.
Une réponse à une attente identifiée ne doit pas reprendre deux fois la même attente.
À distinguer d'un Fork, d'un nouveau run après la fin ou d'une migration vers une
nouvelle définition. Une reprise ne rétablit pas les effets externes passés.

**Fork** : création d'une nouvelle continuation depuis un point antérieur
restaurable, en préservant l'histoire existante et en enregistrant sa filiation.
Les primitives de checkpoints et de time travel ADK peuvent soutenir cette opération ;
la politique complète de fork Zedflow reste à réaliser, notamment pour les ressources
et exécutions enfants. Le travail suivant le point choisi peut être réexécuté.
À distinguer d'une reprise de l'exécution courante, d'un retour arrière destructif
ou d'une annulation des effets externes déjà produits.

**Attached Path** : chemin de branchement restant dans le même Flow Run, avec l'état
et les checkpoints associés à cette exécution. Les règles de visibilité et de
mapping continuent de s'appliquer, y compris lorsqu'il utilise un sous-graphe isolé.
À distinguer d'une Session Branch ou d'un nouveau run indépendant.

**Detached Run** : Flow Run distinct lancé depuis un autre, avec son état, ses
checkpoints et son identité propres. Le parent conserve une Run Reference et peut
consommer un résultat déclaré. La relation de session, le cycle de vie et la
propagation de l'annulation doivent être définis séparément.
À distinguer d'un mode sans interface ou d'une simple branche dans le même run.

**Wait Mode** : comportement synchrone ou asynchrone de l'appelant vis-à-vis d'un
chemin ou d'un run lancé : attendre son résultat avant de poursuivre, ou continuer
avec un mécanisme explicite d'observation et d'éventuelle jonction ultérieure.
Ce choix est distinct du maintien dans le même run ou du lancement d'un Detached Run.
À distinguer d'une attente humaine, de la portée de l'état ou d'un mode headless.

**Run Lineage** : filiation entre runs créés notamment par fork ou lancement d'une
exécution distincte. Elle relie les identités parent et enfant, l'action d'origine
et, selon le cas, le nœud, l'occurrence et le checkpoint source.
À distinguer de la topologie du graphe, de l'historique de session ou de la propriété
automatique des ressources de l'enfant.

## Vocabulaire de l'interaction

Ces définitions complètent le vocabulaire technique avec les frontières humaines
et les règles de continuité. Les modalités fonctionnelles sont précisées dans
[sessions et interfaces](docs/interaction.md) ; le support actuel et ses limites,
notamment pour les attentes concurrentes et les réponses complexes, appartiennent
à la [documentation de l'application](docs/app.md).

**Human Input Interrupt Boundary** : frontière d'un flow où une branche d'exécution
est suspendue spécifiquement pour obtenir une réponse humaine. Elle peut correspondre
à une entrée ordinaire, une décision ou une commande attendue par le flow, quelle
que soit l'interface. Elle expose une Attente de réponse identifiée ; d'autres
branches peuvent rester actives si la composition le permet. Le chargement d'un
skill comme contexte et la réception d'une réorientation suivent leurs propres
politiques ; ils ne sont pas tous ramenés à cette frontière.
À distinguer d'un breakpoint, d'une attente async interne ou d'une injection au milieu
d'un nœud sans contrat.

**Interrupt Node** : nœud dont la seule responsabilité métier est d'établir une
Human Input Interrupt Boundary puis de fournir la réponse acceptée à sa suite.
Il n'effectue aucun appel modèle, aucune exécution d'outil, aucun lancement de
run ni effet métier durable, y compris une écriture de fichier. La sauvegarde du
checkpoint et de l'attente relève du mécanisme de suspension. L'action approuvée
est exécutée dans une autre étape.
À distinguer d'un nœud mélangeant demande de confirmation et effet à autoriser.

**Interrupt Shape** : contrat d'entrée d'une attente humaine : texte, confirmation,
choix simple ou multiple, sélection, formulaire structuré, fichier ou contenu à
valider ou éditer. Il définit les données attendues indépendamment du widget qui
les recueille. Les interfaces présentent une réponse équivalente lorsqu'elles
la prennent en charge ; une interface ouverte ne réserve pas l'attente.
À distinguer de la sémantique de l'action qui suivra ou d'un composant TUI particulier.

**Breakpoint** : point déclaré de pause ou d'arrêt de contrôle, attaché à un flow,
nœud, arête ou branche pour le debug, l'inspection ou la collecte d'un résultat
de branche. Il ne demande pas par lui-même une réponse humaine typée. Une action
manuelle de reprise ne transforme pas ce point en attente métier de réponse.
À distinguer d'une Human Input Interrupt Boundary ou d'une fin normale du flow.

**Human Return Budget** : politique explicite d'un flow ou run limitant le travail
autorisé avant un retour à une frontière humaine. Elle peut porter sur les étapes,
la durée, les appels modèle, les outils, les tokens, le coût ou leur combinaison.
Son périmètre, ses compteurs, leur renouvellement et le traitement des branches
parallèles doivent être déclarés. Les diagrammes doivent exposer les budgets
déclarés et le runtime signaler leurs dépassements.
À distinguer d'une limite technique de récursion ou d'un plafond caché du runtime.

**Human Return Distance** : distance d'un nœud ou d'un parcours à une prochaine
Human Input Interrupt Boundary, selon une mesure et des chemins explicités.
Elle est analysée statiquement lorsque possible, puis observée ou contrôlée à
l'exécution pour les chemins dynamiques. L'existence d'un chemin court ne borne
pas les détours ni les cycles qui peuvent éviter indéfiniment cette frontière.
À distinguer de la profondeur globale du graphe ou d'une garantie de durée d'appel.

**Interactive Flow** : flow qui comporte ou peut atteindre une frontière humaine
dans sa composition. Cette capacité découle du graphe et des flows raccordés ;
un drapeau d'interface ne la crée pas. Le Root Flow conversationnel est interactif
par construction, mais tous ses chemins ne sont pas nécessairement en attente.
À distinguer de l'état « attend actuellement une réponse » ou d'un mode TUI.

**Budget Interrupt** : suspension imposée par l'épuisement d'un Human Return Budget,
avec conservation de l'état et création d'une attente donnant le contrôle à l'humain.
La politique doit préciser la frontière de suspension et le sort des actions en
cours pour permettre une reprise compréhensible. Ce comportement reste une exigence
produit : la limite de récursion ADK produit une erreur et ne le fournit pas à elle seule.
À distinguer d'un timeout technique, d'un abandon silencieux ou d'un Graph Drain.

**Attente de réponse** : point d'une exécution suspendue qui demande une réponse
humaine d'une nature déterminée. Elle peut demander un texte, un choix, une
confirmation ou des données structurées ; elle ne désigne pas toute interruption.

**Interface abonnée** : interface qui reçoit les événements d'une session ou
d'une exécution et en présente l'évolution. Son ouverture ne lui donne pas un
accès exclusif à la session.

**Réponse acceptée** : réponse retenue pour satisfaire une attente encore ouverte
et permettre la reprise du flow. Pour une même attente, une réponse déjà acceptée
empêche une seconde réponse de provoquer une nouvelle reprise.

**Attente d'entrée dans la boucle agentique** : attente de réponse à un point
explicite du graphe dont la suite poursuit la boucle. Elle peut être atteinte
plusieurs fois et ne constitue pas une fin d'exécution.

**Fin d'exécution** : situation d'une exécution qui a atteint sa fin et ne conserve
pas une attente permettant sa continuation. Une nouvelle exécution ou une
bifurcation depuis un point passé ne constitue pas sa reprise.

Une attente est identifiée par son occurrence, pas seulement par le nœud auquel
elle appartient : une réponse à un tour passé ne satisfait pas l'attente du tour
suivant. La conception prévoit plusieurs attentes indépendantes lorsque des
branches le nécessitent ; leur prise en charge effective dépend du runtime et
de l'application. Fermer une interface ne termine pas l'exécution ni ses attentes.

## Adaptation du vocabulaire historique

Les concepts `LangGraph Sidecar Server` et `Runtime Adapter` dans son ancien sens
de client Rust du sidecar sont retirés : l'exécution utilise ADK-Rust directement.
Le daemon qui héberge l'application n'est pas un sidecar LangGraph renommé.

`Pi Session`, `Bound Range`, `Unbound Session Entry` et `Incomplete Bound Entry`
décrivaient la compatibilité entre l'historique Pi et les bindings LangGraph.
Ils ne font pas partie du modèle du reboot. Les besoins de restauration et de
traçabilité sont exprimés par Zedflow Session, Flow Run et Execution Binding,
sans imposer de migration Pi. `Pi-Compatible Error` devient Invocation Error.

Les autres termes historiques sont conservés avec les définitions ci-dessus.
Les choix sur le conteneur session, les forks, les ressources et les frontières
d'arrêt restent explicitement ouverts là où la conception n'a pas encore tranché.

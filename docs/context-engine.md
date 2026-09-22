# Stratégies de contexte et composition des harness

Décisions produit : conversation de conception avec le propriétaire de Zedflow,
12 septembre 2026. Ce document porte le plan de réalisation de ces décisions.
Il complète le vocabulaire de [CONTEXT.md](../CONTEXT.md), les concepts de
[composition](composition.md) et de [ressources](resources.md).

## Objectif et périmètre

Composer visuellement des stratégies de contexte dont la source de vérité est
du Rust structuré dans `<workspace>/.zedflow/context/*.rs`. Une paire de nœuds **Contexte → Modèle**
associe une stratégie et un modèle de prédiction. Le Contexte prépare une fenêtre
identifiée ; le Modèle la consomme lors du passage suivant. Les entrées peuvent comprendre
du texte, des données structurées ou des médias ; la compatibilité effective est
vérifiée par l’adaptateur du modèle. Le langage ne présuppose pas que tous les
modèles sont des LLM ni que tous les budgets sont exprimés en tokens.

La stratégie réunit le profil réutilisable de l’agent et son programme de contexte :
instructions, sélection de capacités, exigences de données et préparation des
entrées. Il n’existe pas un second objet « profil » à maintenir en parallèle.
Une stratégie sélectionne parmi les capacités accordées par le flow et ses bridges ;
sa déclaration ne crée pas une autorisation d’exécuter un outil.

Les ressources restent explicites. Une stratégie sans instructions, historique ou
outils attachés n’en reçoit pas implicitement. La stabilité du contexte, la
conservation d’un préfixe et la compression relèvent de la stratégie choisie.
La mémoire est un comportement composé à partir de ressources et de flows ; le
registre de données ne devient pas une mémoire centrale sémantique.

## Définitions et invariants

| Objet | Responsabilité |
|---|---|
| Flow | Comportement indépendant, points d’entrée et de branchement, données exposées et exigences ; aucune projection de contexte dans son interface de données |
| Bridge | Définition d’une composition entre ces points, avec dépendances, imports, bindings et opérations permises |
| Route | Chemin créé par un branchement de bridge ; il peut être présent, éligible puis effectivement emprunté |
| Runtime Graph | Instances, routes, données et sélections d’inférence résolues pour une exécution |
| Run | Exécution, interactive ou autonome, avec versions et provenance des passages |
| Session | Continuité utilisateur disponible pour un flow interactif ; une exécution autonome n’impose pas de conversation |
| Stratégie de contexte | Programme réutilisable qui sélectionne, transforme et organise les ressources avant une inférence |
| Fenêtre préparée | Résultat versionné d’une stratégie pour un appel précis, avec liens vers blocs, ressources et révisions |

Deux imports d’un même flow créent deux instances nommées indépendantes. Le partage
d’une instance existante est explicite. Un bridge peut créer plusieurs routes ; il
ne se réduit pas à une arête ou à un sous-graphe. Une composition peut utiliser des
sous-graphes ADK, des appels à des runs ou un transfert de contrôle selon son contrat.
ADK conserve la responsabilité de l’exécution.

Les continuations d’une route sont « appeler et attendre », « lancer et continuer »
ou « passer la main ». L’attente porte sur le parcours concerné. La publication du
résultat constitue un choix distinct : ressource exposée ou entrée déclarée d’un
destinataire, avec règle de fusion. Une route peut être invoquée par un outil fourni
au modèle, un nœud du flow, une condition ou une demande de production de ressource.

## Registre et scopes

Les scopes flow, bridge et Runtime Graph sont des vues et des droits sur un même
univers d’entités. Un bridge sélectionne les expositions de ses flows et les rend
disponibles à des destinataires déclarés. Une vue n’emporte pas une copie du contenu.

Trois identifiants sont distincts : identité logique d’entité, révision immuable,
hash du contenu. Deux entités identiques en octets restent deux entités. Une
publication A → B → A crée trois révisions et peut réutiliser deux contenus.
Les checkpoints et manifestes conservent des références persistables, jamais des
adresses mémoire. Les lecteurs d’une révision gardent cette version lorsque le head
de l’entité change. Une publication vérifie la révision attendue ou applique un
reducer explicitement déclaré ; un mutex seul ne protège pas d’un calcul périmé.

Dans le daemon, les lectures partagent des valeurs immuables hydratées par `Arc`.
Les projections dérivées sont de nouveaux contenus. La matérialisation d’une
requête, les valeurs internes ADK, les transports et les adaptateurs multimodaux
peuvent encore allouer : le partage des scopes ne promet pas un zéro-copie global.

Un registre de types décrit primitives, collections, enregistrements, types métier
nommés et références de médias. Les types métier ont une identité explicite : une
même structure ne suffit pas à convertir un `Terme` en `Personne`. Les projections
ou mappings déclarés réalisent les conversions nécessaires. Un type métier peut
être fourni simultanément par des fichiers Markdown et une base de données.

Le catalogue d’auteur rassemble les types constructibles intégrés, les catalogues
du workspace, les schémas embarqués des stratégies et les données exposées par les
flows. Choisir un type ne charge pas une ressource. Une stratégie conserve ses
exigences et les définitions nommées nécessaires dans son propre artefact Rust ;
le flow choisit les bindings qui satisfont ces exigences. Les conflits entre
définitions d’un même type sont signalés avant assemblage. Les formes JSON et les
provenances du catalogue sont décrites dans [l’API](context-api.md#catalogue-des-types-de-sources).

Une ressource déclare ses sources et, si nécessaire, son producteur. La composition
peut relier ou remplacer explicitement celui-ci. Absence, résultat vide valide,
production en cours, échec et péremption restent distincts. La validité dépend des
entrées et versions publiées, pas seulement du fait que le producteur a déjà tourné.

## Langage de contexte

Le document Rust et l’éditeur personnalisé représentent le même programme typé.
La validation partagée vérifie références, types, disponibilité et capacités ; les
connexions visuelles aident à composer mais ne remplacent pas le compilateur.

| Famille | Opérations à couvrir |
|---|---|
| Accès | Ressource, collection, champ, résultat d’un passage, sélection de source |
| Conditions | Présence, égalité, comparaisons typées, inclusion, ET/OU/NON |
| Collections | Filtre, tri, sélection, parcours, regroupement, déduplication |
| Projection | Extraction de champs, template, représentation courte/détaillée, projection métier |
| Assemblage | Groupes identifiés, fragments, ordre, placement |
| Fenêtre | Remplacer, retirer, déplacer, changer de représentation sur une révision précise |
| Réutilisation | Sous-programme avec paramètres et sortie typés |
| Besoins | Exiger, demander un producteur, suspendre durablement, traiter absence/échec |
| Mesure | Taille par modalité, budget, choix conditionnel de représentation |

L’évaluation pure compile un contexte depuis des références figées. Une opération
qui produit une information nouvelle, comme une recherche, une synthèse modèle ou
une écriture, est un flow ADK identifiable. Le context engine retourne un besoin
structuré ; le runtime l’honore selon les routes autorisées puis reprend l’évaluation.
Les règles dynamiques ne deviennent pas du Rust arbitraire exécuté par le lecteur.

Le parcours effectif de composition comporte trois opérations distinctes : déclarer
une source typée, composer les expressions et blocs qui la consultent, puis évaluer
la fenêtre avec des valeurs explicites. Une déclaration seule n’émet aucun fragment.
Un champ peut être projeté directement, transformé ou utilisé dans une condition
sans injecter tout l’objet dont il provient.

Le bloc `forEach` parcourt une liste avec une variable locale et peut émettre
plusieurs fragments ou groupes pour chaque élément. Les parcours et conditions
s’imbriquent ; les variables et leur provenance restent limitées à leur portée.
L’expression `list` construit une collection typée depuis d’autres expressions,
notamment pour assembler les parties de messages. `truncate` sélectionne un
préfixe de texte avec une limite Unicode explicite, tandis que `take` sélectionne
des éléments d’une liste. Les formats des fragments continuent de distinguer
une donnée JSON d’une conversation ADK. Les [contrats du langage](context-api.md#parcours-listes-et-extraits-de-texte)
précisent les types, les cas d’absence et les limites d’évaluation ; ces blocs
s’ajoutent aux familles existantes.

Les types conservent une profondeur métier maximale de 64 niveaux. Leur
représentation JSON dispose d’une borne distincte de 256 conteneurs, car les
enveloppes et les objets `fields` ajoutent des niveaux. Les corps JSON et les tokens
Rust font l’objet d’un précontrôle itératif avant l’analyse récursive ; les sources
profondes admises sont analysées sur une pile dédiée, puis soumises à la validation
métier.
Les exports Cargo emportent ce codec et les protections du lecteur, avec les
options nécessaires pour compiler la source exacte. Les
[limites de lecture et leurs contrôles](context-api.md#limites-de-lecture-et-de-profondeur)
précisent les frontières HTTP, Rust et Cargo vérifiées. Le partage des ressources
ne supprime pas les allocations liées au décodage, aux projections ou à la requête.

L’aperçu autonome évalue les valeurs d’essai sans grant de flow et renvoie les
capacités demandées pour inspection. La validation de liaison, la compilation et
le runtime vérifient toujours les capacités réellement accordées avant utilisation.
Un aperçu réussi n’autorise aucun outil et ne lance aucune inférence.

Les stratégies livrées (`workspace-default`, `conversation-default`,
`tools-default` et `working-system-context`) utilisent le programme structuré
du studio. Leurs contrats embarquent des types nommés décrivant les valeurs
réellement fournies : `WorkspaceInstructions`, `SkillCatalog`, `SelectedFiles`
et `UserInput` sont des textes ; l’historique est une liste de
`ConversationMessage`, avec `role` et `parts`. Les textes déjà agrégés ne sont
pas présentés comme des documents ou des skills individuels.

Trois fragments compacts précèdent la conversation. La branche historique
parcourt les messages entiers dans leur ordre d’origine, sans reconstruire leurs
parties : raisonnement, signatures, appels et résultats d’outils restent associés.
Une liste vide reste vide ; la saisie de secours n’est utilisée que si la source
d’historique est absente. L’initialisation crée les stratégies manquantes et
n’écrase jamais une stratégie existante. Réviser les définitions enregistrées
est une modification explicite ; les passages historiques conservent leur source.

L’éditeur montre ressources et blocs disponibles, programme composé, puis fenêtre
obtenue pour un passage ou préparée pour le prochain appel. Un fragment remonte à
son bloc et aux données sélectionnées. Développer une représentation pour l’inspecter
ne change pas ce qui sera envoyé. Une retouche de fenêtre et une modification de
stratégie indiquent leur portée ; leur transformation en règle réutilisable est explicite.

La trace d’évaluation associe chaque occurrence à son bloc, aux ressources
consultées, à ses itérations et aux éléments produits. Une condition évaluée y
conserve son résultat vrai ou faux. Les occurrences de boucle sont distinctes et
déterministes pour un ordre donné ; leur identifiant n’est pas celui d’une entité
métier après réordonnancement. La fenêtre garde son format historique : les liens
précis viennent de cette trace lorsqu’elle est disponible.

Un flow spécialisé peut agir sur le contexte d’un autre avec sa propre stratégie,
ses outils et les données exposées par le bridge. Le contrôle de révision de la
fenêtre cible empêche d’appliquer silencieusement une intervention devenue périmée.
Les événements de fichiers du harness pourront ensuite déclencher des flows de
documentation spécialisés. Aucun watcher global n’est prévu dans ce lot ; la
sémantique complète des triggers reste à spécifier séparément.

## Révisions et navigation

Les brouillons n’affectent pas les runs existants. Ils peuvent être testés dans un
workspace temporaire, avec session seulement si le flow est interactif. Les sources
de contexte choisies sont capturées depuis le workspace d’origine ; les outils
utilisent le workspace temporaire comme répertoire courant. Ce mécanisme n’est pas
un sandbox : un outil explicitement autorisé conserve ses possibilités d’accès.

Enregistrer publie une révision à tous les runs concernés par la définition ou ses
dépendances. Chaque run l’adopte à la prochaine étape compatible. Une opération déjà
engagée termine avec son contrat capturé. Une incompatibilité structurelle avec les
compositions dépendantes bloque l’enregistrement avec diagnostic. Une incompatibilité
de l’état courant d’un run conserve sa révision active et affiche le diagnostic ;
elle n’impose ni pause, ni fork, ni redémarrage.

Cette règle est raccordée aux frontières ADK, y compris branches parallèles et sous-graphes.
Les passages, requêtes et checkpoints enregistrent leur version réellement utilisée.
Les archives emportent toutes les versions nécessaires à la consultation et à la reprise.

Les modifications de configuration compatibles conservent l’exécuteur ADK. Une
modification de structure ne le reconstruit qu’à une frontière séquentielle prouvée,
avec les mêmes canaux et une continuation restaurable. Une reconstruction arbitraire
des branches parallèles, des instances ou des droits de la composition est refusée.
Une reprise reconstruit la définition enregistrée avec son checkpoint, jamais le
dernier fichier disponible par simple proximité de numéro d’étape.

L’interface relie définition, dépendances, révisions, runs et passages précis. Le
retour d’inspection conserve brouillon, position de lecture, caméra et sélection.
Le graphe résolu s’affiche avant lancement avec choix d’entrée, bridges actifs,
stratégies et modèles des nœuds d’inférence.

## Lots de réalisation

Les composants purs se trouvent dans `rust/crates/zf-context` et `zf-core`,
leurs raccords ADK dans `zf-runtime`, leur stockage dans `zf-storage` et leur
orchestration dans `zf-execution`. Le client vit dans `web/apps/client`. Les interfaces
publiques et leurs limites sont décrites dans [l’API](context-api.md). Les sources
de stratégie, bibliothèque et types sont capturées ensemble dans le programme de
chaque inférence. Les anciens nœuds sans stratégie conservent leur comportement.
La validation finale de cette intégration est consignée séparément dans
[validation](validation.md), sans assimiler un test de langage pur à un parcours
runtime ou navigateur.

| Lot | Résultat vérifiable | Dépendances |
|---|---|---|
| 1 — Socle | Types, entités et scopes persistants ; résolution déterministe de contrats ; stratégie Rust lisible, validée et prévisualisable par API | Stockage canonique existant |
| 2 — Exécution composée | Contrats associés aux fichiers de flows, catalogue de bridges, lancement du Runtime Graph par ADK, appels/attentes/transferts et reprise sans répétition d’effet | 1 |
| 3 — Contexte des inférences | Remplacement de l’assemblage actuel par stratégie, provenance exacte, adaptateurs et capabilities vérifiées ; producteurs durables et contexte multimodal compatible | 1–2 |
| 4 — Édition visuelle | Éditeur de blocs, catalogue types/sources/projections, choix des bridges, comparaison fenêtre/programme, navigation définition ↔ passage | 1–3 |
| 5 — Langage étendu | Collections, fenêtres révisables, sous-programmes, budgets, transformations et interventions d’un second flow | 3–4 |
| 6 — Adoption des révisions | Tests de brouillons, validation inverse des dépendances, adoption au prochain point ADK compatible, diagnostics par run et archives multirévisions | 2–5 |
| 7 — Interopérabilité | Import/export autonome de stratégies/bridges/types, extensions de lecteurs et projections, contrats publics stabilisés | Lots précédents |

Les producteurs et les interventions sur fenêtres utilisent des routes ADK
déclarées. Les données, les reçus d’appel, les publications et les fenêtres sont
reliés au magasin de contenus et aux checkpoints. Un second agent peut modifier
une fenêtre seulement par un grant explicite du bridge et sur la révision attendue.
La fenêtre retenue est vérifiée contre le programme effectif : changer une
bibliothèque, des types ou un binding invalide aussi sa sélection antérieure.

Les lecteurs natifs standard couvrent les fichiers texte/JSON, les documents
SQLite et les contenus canoniques. Le registre de lecteurs est extensible par
l’hôte Rust ; une source ou un package importé ne charge pas de code natif. Un
export Cargo refuse un lecteur dont l’implémentation n’est pas embarquée. Les
adaptateurs de prédiction actuels sont ceux des modèles intégrés : le type média
générique ne prétend pas fournir un backend diffusion ou world model.

Les formats de flows v1/v2 conservent leur sémantique historique sans stratégie
sélectionnée. Les formats de stratégies, de types, de bibliothèques, de bridges,
de stockage et d’archives restent versionnés indépendamment.

## Critères de validation

- Données : deux scopes pointent la même valeur résidente ; deux identités de même
  contenu restent distinctes ; snapshots stables, conflits concurrents, droits non
  extensibles par alias, isolation entre univers et reprise après réouverture SQLite.
- Composition : fermeture des bridges requis, dépendance manquante/cycle, ports et
  types incompatibles, imports indépendants et réutilisation explicite, absence de
  copies de datasets dans le plan résolu, diagnostics localisés.
- Langage : aller-retour Rust, rejet des constructions non reconnues, absence de
  coercition, branches inactives sans dépendances artificielles, médias par référence,
  capacités sélectionnées parmi celles accordées, source invalide et conflit d’édition.
- Intégration : sources versionnées depuis un workspace connu ; aucun appel modèle
  ni effet de flow pendant la validation, la résolution ou la prévisualisation pure
  d’une stratégie. Le test d’un brouillon de flow exécute ses opérations dans le
  workspace temporaire selon les modalités décrites plus haut.
- Lots runtime : captures depuis le contexte réel du passage, attentes périmées,
  reprise de sous-graphes et branches parallèles, frontière durable avant effet,
  adoption de révision sans perdre joins, retries ni résultats déjà consommés.
- Interface : composition au clavier, erreurs liées aux blocs, inspection sans mutation,
  conservation de navigation, écran étroit et source exacte retrouvable depuis le run.

Les essais utilisent fixtures et dossiers temporaires. Contrôles du dépôt :
`pnpm --dir web typecheck`, `pnpm --dir web build`, `pnpm --dir e2e test`, puis depuis `rust/` Cargo format, check, tests
avec `ZEDFLOW_TEST_CODEGEN=1`, clippy et check toutes features, cible externe.
Les résultats effectivement exécutés sont consignés dans [validation](validation.md).

# Composition et partage

Statut : intention de conception. Les types Zedflow décrits ici ne sont pas
implémentés. Les expériences utilisent directement `adk_graph`.

## Unités de composition

| Concept | Intention |
|---|---|
| Flow | Graphe responsable d'un comportement précis ; il peut être déterministe, agentique ou mixte |
| Root Flow | Point d'entrée d'un harness, dont la boucle et les routes sont explicites |
| External Flow | Comportement spécialisé branché à une composition, sans obligation de devenir un sous-graphe technique |
| Standalone Flow | Flow exécutable seul avec ses entrées, sorties et ressources |
| Embeddable Flow | Flow invocable par un autre à travers une interface déclarée |
| Composable Flow | Flow exposant des points de connexion utilisables indépendamment de son implémentation |
| Flow Bridge | Connexion définie séparément entre flows, portant mappings et conditions de routage |
| Flow Composition | Assemblage de flows, bridges, politiques et ressources |
| Flow Bundle | Artefact versionné et partageable contenant un flow ou une composition, ses dépendances et interfaces |
| Flow Definition | Source exécutable d'un flow ; aucun DSL ni format JSON imposé à ce stade |
| Runtime Graph | Composition concrète résolue pour une exécution et surface de validation de l'ensemble |

Standalone, embeddable et composable décrivent des capacités cumulables. Un bundle
n'est pas nécessairement toute la composition installée. Plusieurs bridges peuvent
relier les mêmes flows si la politique de sélection ou de parallélisme est explicite.

L'API de composition devrait rester proche d'ADK et ajouter les mécanismes utiles
à l'assemblage de composants indépendants. Il reste à expérimenter quand utiliser
un sous-graphe, intégrer des nœuds dans le graphe parent ou lancer un run distinct.
Ces formes d'exécution ne doivent pas redéfinir l'identité du flow partageable.

## Résolution du harness

Le registre doit, à terme, associer identité, version, provenance et capacités des
graphes. Il peut commencer localement ; une place de marché distante n'est pas un
prérequis. La configuration de lancement choisit un Root Flow, les bundles, les
bridges et les bindings de ressources.

La chaîne à rendre explicable est : répertoire courant → configuration applicable
→ références de graphes → versions et ressources résolues → Runtime Graph → run.
La recherche de configuration doit rester prévisible. Une racine de workspace
constitue le cas courant ; les configurations plus fines par répertoire demandent
des règles d'héritage et de priorité explicites, sans accumulation implicite.

Le format, le nom du fichier, la politique d'héritage et le stockage du registre
restent ouverts. Le launcher du lab sélectionne simplement un flow par commande.

## État et connexions

| Concept | Intention |
|---|---|
| Flow State | État d'exécution aligné sur les canaux et reducers du runtime ADK |
| State Transition Contract | Lectures, écritures et exigences d'une transition |
| Logical State Key | Nom qu'un auteur donne à un état avant assemblage |
| Resolved State Key | Identité après résolution des namespaces, collisions et branches |
| Runtime State Key | Clé générée ou réservée par l'assemblage |
| State Key Visibility | Exposition publique ou privée à la composition |
| State Key Scope | Périmètre de lecture/écriture qui affine cette exposition |
| State Key Availability | Présence garantie à un point du parcours, compte tenu des branches, defaults et gates |
| Node State Surface | Valeurs effectivement visibles et disponibles à un point de composition |

Déclarer une clé dans un schéma ne garantit pas qu'elle a été produite sur chaque
branche. La visibilité de composition ne constitue pas à elle seule une isolation
de sécurité. Une composition doit permettre de comprendre d'où vient une valeur
et quels composants peuvent la modifier.

Le partage simultané exige de distinguer trois décisions : comment fusionner
les écritures (reducer), quand poursuivre ou annuler les branches (politique de
parallélisme), et quelle valeur lire ensuite (sélection ou fusion). Un dernier
écrivain implicite ne résout pas la signification d'écritures concurrentes.

## Routes, interactions et erreurs

Les conditions de routage consomment un état approprié. Une normalisation peut
transformer une réponse de modèle ou un résultat d'outil en cet état. La sélection
de route et l'action qui produit les données de sélection doivent rester lisibles.
Les politiques multi-routes précisent priorité, exclusivité, fan-out et attente.

Les points d'attente humaine exposent un type d'entrée : texte, confirmation,
choix, édition ou formulaire. Leur responsabilité est l'attente ; les actions
durables se déroulent dans d'autres étapes pour permettre une reprise compréhensible.
Un breakpoint de debug est différent d'une attente de décision humaine.

Des budgets peuvent limiter les appels, étapes, tokens, coûts ou la durée avant
un retour à l'humain. Une distance vers cette frontière peut être analysée quand
la structure le permet, puis contrôlée au runtime pour les chemins dynamiques.
Ce sont des intentions produit ; le lab utilise seulement la limite de récursion ADK.

Les flows doivent rendre observables les erreurs de modèle/outils, celles du
runtime et celles de validation d'une sortie, ainsi que leurs chemins de reprise,
retry, fallback ou arrêt. Une demande d'outil par un modèle et son exécution
effective sont deux événements distincts. L'exposition d'un outil au modèle et
l'autorisation de l'exécuter sont également des décisions différentes.

## Validation et inspection

Une validation isolée de bundle vérifie ses interfaces dans une composition de
référence appropriée. La validation du Runtime Graph vérifie l'ensemble résolu :
connexions, ressources requises/fournies, collisions, cohérence des politiques,
écritures concurrentes et incompatibilités connues.

Cette validation structurelle ne prouve ni la qualité des réponses, ni l'absence
de bugs dans le code arbitraire des nœuds. Des exécutions et des cas contradictoires
restent nécessaires.

Un diagramme est une vue dérivée de la définition ou de l'assemblage. Les événements
et checkpoints décrivent le parcours réel. L'objectif est de relier ces vues avec
la version exécutée, sans prendre un historique de session pour la définition du
programme ni promettre une trajectoire déterminée à l'avance.

## Sessions et runs

Un Flow Run est une exécution concrète. La session est la continuité présentée à
l'utilisateur ; elle peut référencer plusieurs runs ou forks. La relation entre
messages, runs, événements et checkpoints doit rester traçable sans contrainte
de compatibilité avec une session Pi.

Resume continue une exécution sauvegardée. Fork crée une nouvelle continuation
depuis un point antérieur en préservant l'histoire. Un chemin attaché reste dans
le run ; un run détaché possède son état et ses checkpoints et expose une référence
et, si prévu, un résultat au parent. Leur filiation et leur mode d'attente sont
distincts de la topologie des branches du graphe. Reprise et fork ne rétablissent
pas automatiquement les effets externes déjà réalisés.

# Contextes, état et ressources modulaires

Statut : principes de ressources. Le [plan du context engine](context-engine.md)
précise le registre d’entités par références, les scopes et les stratégies de
projection, avec l’état de réalisation de chaque lot.

## Un module peut apporter des capacités

Un graphe modulaire peut fournir un store, ajouter des canaux d'état, consommer
un service existant ou exporter une interface. La composition étend ainsi les
comportements disponibles et les ressources avec lesquelles ils travaillent.

Exemple : un bundle de recherche apporte un store de sources et un état de couverture
de la question. Un flow d'analyse consulte les sources ; un flow de contrôle écrit
des contre-exemples. Les trois partagent une ressource déclarée dans leur assemblage.
Un autre assemblage peut fournir un autre store sans changer leur responsabilité.

Ce mécanisme pourrait permettre des mémoires spécialisées, des tableaux de travail
partagés entre agents ou des collections de preuves. Leur utilité et leurs règles
de coopération restent à mesurer ; la présence d'un store ne les garantit pas.

## Quatre choses à distinguer

| Élément | Fonction | Exemple |
|---|---|---|
| État du run | Données du parcours, destinées aux transitions et checkpoints | Branche choisie, résultat de validation |
| Store | Données conservées au-delà d'une étape ou d'un run selon son backend | Sources réutilisées, observations sur un dépôt |
| Ressource/service | Capacité disponible aux nœuds | Client de recherche, connexion de base, fournisseur de mémoire |
| Contexte modèle | Projection préparée pour un appel donné | Résumé de trois sources et instructions pertinentes |

Partager un store ne signifie pas injecter son contenu complet dans chaque prompt.
Un handle de service n'est généralement pas une valeur sérialisable de checkpoint.
La reprise doit retrouver les bindings de ressources appropriés, tandis que l'état
sérialisé restaure le parcours.

## Visibilité et durée de vie

Un état ou un store peut rester privé au flow, être partagé dans un assemblage,
être exposé au run entier ou se rattacher à un workspace/utilisateur. Un « global »
doit toujours préciser son univers : processus, run, workspace ou utilisateur.

Le périmètre de visibilité et la durée de vie sont deux axes distincts. Un store
durable peut n'être visible que par un sous-ensemble de flows ; un état commun à
tout un run peut être éphémère. Les namespaces de composition empêchent certains
couplages accidentels, mais ne constituent pas un sandbox pour du code Rust arbitraire.

## Questions que l'assemblage doit rendre explicites

- Ce que le module requiert, fournit et exporte ; quels bindings relient ces éléments.
- Qui crée une ressource, qui la possède, qui peut la lire ou l'écrire et qui la libère.
- Sa durée de vie, son éventuelle persistance et sa politique d'isolation.
- Les règles de concurrence, fusion, invalidation et déduplication.
- L'identité et la version des schémas, ainsi que la compatibilité lors d'une reprise.
- La projection de contexte destinée au modèle, avec provenance et ordre des sources.

Ajouter des champs à une map globale est possible techniquement, mais ne suffit
pas à composer des modules indépendants de manière compréhensible. L'enjeu est de
rendre les liens explicites sans imposer un framework disproportionné à chaque flow.

## Intégration du produit

Les anciennes expériences `shared_memory` et `agent_loop` du lab ont été retirées.
Elles ne constituent pas une preuve d'isolation ni une API du produit. Les primitives
ADK demeurent disponibles via le catalogue et les adaptateurs du runtime.

Le daemon raccorde désormais les scopes de flows, de bridges et de Runtime Graph
à un registre persistant d'entités et de révisions. Les stratégies sélectionnent
ces données ou des sources explicitement liées, puis les projettent pour une
inférence. Les [interfaces](context-api.md) portent les contrats implémentés ; les
[vérifications](validation.md) couvrent notamment les instances indépendantes,
les alias partagés, les ressources absentes, les conflits de publication et la
reprise. Les politiques de mémoire entre runs restent à composer à partir de
ces ressources et de leurs producteurs.

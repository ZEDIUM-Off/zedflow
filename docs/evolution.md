# Capitalisation et méta-harness

Statut : intention produit. Le reboot n'implémente ni optimiseur ni évaluateur.

## L'unité d'amélioration

Un nœud, une section de graphe, un flow, un bridge ou un assemblage peuvent évoluer
indépendamment quand leurs interfaces le permettent. Un agent qui analyse les runs
peut proposer une modification du flow exécuté par un autre agent. Il travaille
alors sur un comportement identifiable et des observations rattachées à ses étapes.

Exemple : un flow TypeScript échoue régulièrement au choix de la commande de test.
On peut modifier cette étape et la vérifier sur les dépôts concernés sans modifier
le flow Rust. Une instruction conditionnelle peut aussi être extraite dans un skill
chargé par le nœud approprié. Le choix entre code, topologie, politique, skill et
ressource dépend du problème observé.

## Une même infrastructure de composition

L'analyse de traces, la proposition de variantes et les comparaisons peuvent être
des flows ordinaires, assemblés avec les flows de travail ou exécutés à part.
Un flow d'amélioration peut lui-même être versionné, observé et amélioré. La vision
n'impose donc ni moteur spécial ni séparation permanente en deux boucles.

Elle exige cependant de distinguer une définition active d'une candidate. Modifier
une définition ne doit pas modifier rétroactivement l'identité du graphe déjà
exécuté. Le moment d'adoption et la politique de migration des runs restent explicites.

## Ce qu'une capitalisation utile conserve

- Version et provenance de la définition et de son assemblage résolu.
- Domaine d'emploi : dépôt, langage, outils disponibles, modèle, conditions de tâche.
- Runs et étapes concernés, résultats attendus/obtenus, frictions et contre-exemples.
- Diff de la modification et observations qui motivent son adoption ou son retrait.

Un workflow « meilleur » pour un contexte n'est pas nécessairement meilleur ailleurs.
Des gates précises facilitent la localisation et la comparaison ; elles ne prouvent
pas la causalité. Il faut tenir compte des interactions avec le graphe appelant,
du modèle utilisé et de la variabilité des tâches.

## Ordre d'apprentissage

1. Exécuter et inspecter des flows ADK spécialisés dans le lab.
2. Tester les formes d'assemblage et de partage de ressources avec des exemples réels.
3. Déduire les primitives minimales de registre, versionnement et résolution.
4. Relier définitions, assemblages et traces de runs de façon durable.
5. Expérimenter un flow d'amélioration, avec comparaison et adoption explicites.

Les approches de recherche peuvent être réutilisées pour proposer ou comparer des
candidats. La contribution recherchée de Zedflow est leur application à une
bibliothèque de comportements composables et à leurs ressources, avec une
traçabilité précise. Ce document ne revendique pas l'invention de l'auto-amélioration.

## Références de travail

Le point de départ demandé est [Prime Agent](https://github.com/PrimeIntellect-ai/prime-agent).
Les autres pistes de la discussion sont [Meta-Harness](https://github.com/stanford-iris-lab/meta-harness)
et [FlowEvo](https://github.com/DEFENSE-SEU/FlowEvo). Ces projets sont des références
à examiner pour de futures expériences, pas des dépendances du reboot ni des preuves
que l'architecture Zedflow fonctionne déjà.

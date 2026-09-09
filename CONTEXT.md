# Zedflow : intention du reboot

## Décision

Zedflow explore un harness agentique natif graphe, modulaire et évolutif, construit
sur les primitives d'ADK-Rust. Le reboot abandonne la séquence « port Pi puis
LangGraph ». Aucun comportement Pi, sidecar Python ou runtime LangGraph n'est à
reproduire. L'historique Git conserve les anciens travaux ; ils ne gouvernent pas
cette branche.

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

Un nouveau run résout une composition. Une reprise retrouve la composition et
l'état associés au run précédent ; elle ne doit pas choisir silencieusement la
dernière variante du registre. La continuation, la bifurcation et la migration
d'une ancienne composition sont des opérations distinctes.

## Méta-harness

L'amélioration peut elle-même être un graphe composable, exécuté par les mêmes
primitives et susceptible d'être amélioré. Il n'est pas nécessaire de créer deux
moteurs ou deux boucles permanentes. Il faudra cependant distinguer la version
active, la variante candidate et les observations qui justifient son adoption.

Les évaluateurs, optimiseurs et politiques d'adoption automatiques viennent après
le laboratoire. L'utilité du premier socle doit être tangible avant l'auto-amélioration :
lancer, comprendre, composer, reprendre et modifier des flows précis.

## Ce que le laboratoire implémente

Le code actuel est une collection d'expériences ADK, organisée dans `flows/`.
Il teste une boucle agentique avec appel de sous-graphe, l'isolation des canaux,
la mémoire partagée et les checkpoints. Il ne constitue pas encore le langage de
composition, le registre, le système de scopes ou le méta-harness de Zedflow.

Les expériences utilisent parfois des conventions simples (noms de canaux,
arguments Rust, services passés par `Arc`). Ces choix servent à apprendre ; ils
ne définissent pas les futurs contrats produit.

Les concepts détaillés sont conservés dans [composition](docs/composition.md),
[ressources](docs/resources.md) et [évolution](docs/evolution.md). Leur formulation
actualise le contexte historique à la suite des précisions du porteur du projet
dans la discussion de reboot du 9 septembre 2026.

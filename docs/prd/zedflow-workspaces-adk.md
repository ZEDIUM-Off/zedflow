# Zedflow — backend de workspaces, composition et expérience ADK

Statut : PRD rédigé ; vérification des frontières de test et destination de publication en attente. Aucune fonctionnalité décrite comme cible ne doit être tenue pour déjà implémentée.

## Problem Statement

L'utilisateur veut construire, conserver, réutiliser et suivre des comportements agentiques ou algorithmiques complexes avec ADK, depuis plusieurs interfaces et plusieurs machines. Le laboratoire actuel permet des expériences ADK et une exploration dans Studio, mais ne fournit pas encore le socle applicatif Zedflow qui organise ces compositions et leur continuité.

Une interface de chat seule masque la structure d'exécution. Un canvas seul ne permet pas de comprendre une exécution vivante, ses états, ses attentes ou ses appels enfants. Une connexion cliente ne doit pas devenir propriétaire du travail : fermer une fenêtre ou ouvrir un terminal ne doit ni arrêter ni accaparer une exécution.

L'unité d'organisation attendue est le workspace, situé sur une machine équipée d'un daemon. Un client doit pouvoir ouvrir les workspaces de différentes machines sans confondre leur identité, leurs fichiers, leurs ressources ou leurs exécutions.

## Solution

Construire un backend Zedflow minimal dans sa structure, suffisamment complet pour porter une expérience avancée de composition et de suivi. Un daemon par machine gère les workspaces admis sur cette machine, leur stockage applicatif et l'intégration ADK. Les clients accèdent à ces workspaces par une interface publique commune.

ADK conserve la sémantique d'exécution des graphes. Zedflow possède les documents de composition, leurs versions, leur organisation, les références d'exécution, les attentes accessibles aux utilisateurs et les vues nécessaires aux interfaces.

L'expérience distingue deux espaces : concevoir et analyser les compositions ; suivre et interagir avec les exécutions. La vue d'interaction associe texte ou contenu produit, contrôles de réponse typés, graphe d'exécution et état pertinent. Les exécutions sans interaction restent pleinement utilisables et organisées.

Le résultat attendu du premier socle est un parcours complet : ouvrir un workspace local ou distant, enregistrer et recharger une composition, la valider et la lancer avec ADK, retrouver son historique, répondre à une attente depuis un autre client et constater la reprise cohérente sur les interfaces abonnées.

## User Stories

1. As an utilisateur, I want identifier les machines auxquelles mon client peut accéder, so that je choisisse où ouvrir mon travail.
2. As an utilisateur, I want distinguer une machine, son daemon et mon client, so that je comprenne où les opérations s'exécutent.
3. As an utilisateur, I want ouvrir un workspace sur une machine équipée du daemon, so that je travaille depuis le client de mon choix.
4. As an utilisateur, I want inscrire explicitement une racine de workspace, so that le daemon connaisse les ressources qu'il peut exposer.
5. As an utilisateur, I want voir la machine et le chemin du workspace courant, so that je conserve la provenance de mes actions.
6. As an utilisateur, I want distinguer deux workspaces ayant le même chemin sur deux machines, so that leurs données ne soient pas mélangées.
7. As an utilisateur, I want connaître la disponibilité et les capacités du daemon, so that je comprenne quelles opérations sont possibles.
8. As an utilisateur, I want voir qu'une machine est inaccessible sans changement implicite d'hôte, so that une opération ne s'exécute pas ailleurs.
9. As an utilisateur, I want fermer un client sans arrêter le daemon, so that mon travail garde sa continuité.
10. As an utilisateur, I want retrouver mes workspaces après redémarrage du daemon, so that leur organisation soit durable.
11. As an concepteur, I want créer et nommer une composition dans un workspace, so that je puisse la conserver et la retrouver.
12. As an concepteur, I want enregistrer une composition incomplète, so that je puisse travailler progressivement.
13. As an concepteur, I want recharger mon document avec ses positions et annotations, so that je retrouve mon espace de conception.
14. As an concepteur, I want connaître les propriétés et capacités de chaque nature de nœud, so that je compose avec les primitives disponibles.
15. As an concepteur, I want distinguer les natures indisponibles dans le daemon courant, so that je ne confonde pas représentation et exécutabilité.
16. As an concepteur, I want configurer agents, actions, fonctions et sous-graphes, so that je puisse exprimer des comportements variés.
17. As an concepteur, I want représenter cycles, conditions, parallélisme et jonctions, so that la structure du comportement reste explicite.
18. As an concepteur, I want représenter la boucle LLM, outils, résultats et entrée utilisateur par des nœuds, so that je puisse la comprendre et la modifier.
19. As an concepteur, I want réutiliser une version d'une composition comme sous-graphe, so that je capitalise des comportements sans les recopier.
20. As an concepteur, I want explorer une composition à plusieurs échelles, so that les imbrications profondes restent lisibles.
21. As an concepteur, I want distinguer regroupement visuel et sous-graphe exécutable, so that arranger le canvas ne change pas le programme.
22. As an concepteur, I want référencer modèles, stores et services, so that leurs ressources soient injectées avec une portée explicite.
23. As an concepteur, I want recevoir des erreurs de validation rattachées aux éléments concernés, so that je corrige une composition avant lancement.
24. As an concepteur, I want détecter une sauvegarde concurrente, so that je n'écrase pas silencieusement le travail d'un autre client.
25. As an concepteur, I want lancer une version figée automatiquement, so that chaque exécution reste liée à sa définition exacte.
26. As an développeur, I want réutiliser un flow écrit en Rust, so that le canvas ne limite pas les capacités de composition.
27. As an développeur, I want savoir quelle source fait autorité, so that je ne maintienne pas deux définitions divergentes.
28. As an utilisateur, I want lancer un flow sans chat, so that les comportements purement algorithmiques soient accessibles.
29. As an utilisateur, I want distinguer activité, attente de réponse, pause et fin, so that je comprenne la situation d'une exécution.
30. As an utilisateur, I want retrouver les exécutions terminées, so that leurs résultats restent organisés et consultables.
31. As an utilisateur, I want voir les nœuds actifs à côté des échanges, so that je comprenne ce que fait le flow.
32. As an utilisateur, I want sélectionner une occurrence précise d'un nœud, so that je distingue ses différents passages dans une boucle.
33. As an utilisateur, I want inspecter l'état et sa portée au point consulté, so that je comprenne les données effectivement observées.
34. As an utilisateur, I want distinguer état sauvegardé, sortie en cours et ressource externe actuelle, so that je n'interprète pas une valeur comme une preuve différente.
35. As an utilisateur, I want ouvrir directement une exécution enfant et retrouver son parent, so that je puisse intervenir au bon niveau.
36. As an utilisateur, I want répondre par texte, confirmation, choix, formulaire, fichier ou édition de contenu, so that l'interaction corresponde à la demande du flow.
37. As an utilisateur, I want ouvrir simultanément Web, CLI et TUI sur une exécution, so that je choisisse librement ma surface d'interaction.
38. As an utilisateur, I want qu'une réponse acceptée depuis un client actualise les autres, so that leurs contrôles reflètent la situation réelle.
39. As an utilisateur, I want qu'une réponse tardive ne relance pas une attente déjà résolue, so that une action concurrente ne soit pas appliquée deux fois.
40. As an utilisateur, I want retrouver une attente après redémarrage du daemon, so that je puisse poursuivre sans conserver le flow en mémoire.
41. As an utilisateur, I want récupérer les événements manqués après reconnexion, so that l'interface reconstruise une vue cohérente.
42. As an utilisateur, I want reprendre avec la version initiale de la composition, so that une modification récente ne change pas silencieusement la continuation.
43. As an utilisateur, I want comprendre qu'une exécution terminée exige un nouveau lancement, so that reprise et nouvelle exécution ne soient pas confondues.
44. As an utilisateur, I want distinguer plusieurs attentes indépendantes lorsque le mode d'exécution le permet, so that chaque réponse vise la bonne continuation.
45. As an utilisateur, I want accéder aux résultats et artefacts avec leur provenance, so that je puisse les relier à l'exécution qui les a produits.
46. As an développeur de client, I want consommer les mêmes commandes et événements publics, so that les règles d'interaction ne soient pas réimplémentées dans chaque interface.
47. As an utilisateur, I want une présentation sombre sobre avec des couleurs de nature de nœuds distinctes des statuts, so that la densité fonctionnelle reste lisible.
48. As an utilisateur, I want que l'accès distant soit autorisé pour la machine et le workspace visés, so that une connexion ne donne pas implicitement accès à toutes les racines de l'hôte.

## Implementation Decisions

### Frontière ADK / Zedflow

- ADK demeure responsable des nœuds, arêtes, routages, reducers et mécanismes d'exécution et de checkpoint. Zedflow ne construit pas de deuxième ordonnanceur de nœuds.
- Zedflow possède la persistance et les règles applicatives de composition, version, organisation, suivi et interaction. Les services ADK existants sont réutilisés lorsqu'ils satisfont les besoins constatés.
- Le backend est initialement un daemon Rust organisé en modules de workspaces, compositions, exécutions, intégration ADK et stockage. Cette organisation n'impose ni microservices ni séparation immédiate en crates.

### Machines et workspaces

- Workspace remplace Projet comme unité applicative d'organisation. Un workspace possède une identité durable, un hôte propriétaire et une racine explicitement admise. Il ne se réduit pas à un dépôt Git.
- L'identité de l'hôte est distincte de celle du daemon en cours d'exécution, du client et de sa fenêtre. Les commandes ciblent explicitement l'hôte et le workspace ; le chemin seul n'est jamais une identité globale.
- Le daemon propriétaire gère les données et les opérations locales du workspace. Fermer une interface ne termine pas ses exécutions. Une perte de connexion indique un état distant non confirmé, pas une fin d'exécution.
- Le protocole expose les capacités et la compatibilité nécessaires aux clients. L'accès distant exige une identité autorisée et des racines admises ; il n'implique pas d'exposer publiquement tous les daemons.
- Le modèle s'inspire de Working OS. Il n'impose ni réutilisation de son implémentation ni coordinateur DGX. Connexion directe, relais et découverte sont des choix à arrêter lors du contrat réseau ; ils ne changent pas la propriété des données.
- Aucun basculement automatique d'une opération vers une autre machine. Une commande dont le résultat a été perdu doit être rapprochée de son identité sur l'hôte propriétaire avant un éventuel nouvel effet.

### Données et composition

- Le socle proposé utilise SQLite pour les données applicatives locales du daemon et un stockage de fichiers pour les contenus volumineux. Aucune réplication distribuée de cette base n'est requise. L'organisation physique par daemon ou par workspace reste un détail à arrêter avec les règles de sauvegarde.
- Les objets persistants sont les identités de machines pertinentes, workspaces, compositions, brouillons révisés, versions immuables, exécutions, événements et attentes. Les résultats volumineux et checkpoints sont référencés selon leur propriétaire.
- Une composition possède une identité stable. Son brouillon peut être incomplet ; une version lancée est immuable. Les références transitives des sous-compositions sont résolues et figées pour ce lancement.
- Le document de composition est un JSON avec version de schéma. Il distingue graphe, dépendances, références de ressources et présentation. Les configurations ADK sérialisables sont réutilisées lorsque leur couverture convient.
- Le format persistant est un format de construction de graphes ADK. Son existence ne justifie pas un langage d'exécution supplémentaire.
- Le canvas ne redéfinit pas le comportement par ses positions ou groupes visuels. Les nœuds et les connexions gardent des identifiants stables utilisables dans les diagnostics et l'historique.
- Le catalogue descriptif de nœuds expose les configurations et capacités réellement constructibles par le daemon. Sa première forme peut être déclarée en code. La couverture cible inclut toutes les natures ADK-Graph ; chaque nature doit être recensée comme exécutable, indisponible avec raison, ou restant à intégrer. Ne pas assimiler une carte dessinable à un support runtime.
- Une source fait autorité pour chaque définition : document pour une composition visuelle, Rust pour un flow écrit en code. Un import aller-retour général de Rust arbitraire n'est pas promis. Le choix entre construction directe et génération de Rust doit être explicité pour chaque mode supporté.
- Les ressources sont référencées et injectées. Les secrets et objets Rust vivants ne sont pas sérialisés dans les compositions. État du run, store, service et contexte modèle restent distincts.
- Une sauvegarde concurrente obsolète est rejetée explicitement. Un brouillon invalide peut être enregistré mais pas annoncé comme exécutable. Le lancement peut figer automatiquement sa version sans publication manuelle préalable.

### Exécution et interaction

- Une exécution Zedflow référence sa version, son workspace, son hôte, ses entrées, son parent éventuel et les identités ADK utiles. Les checkpoints ADK ne sont pas dupliqués en une seconde source faisant autorité.
- L'exécution, l'occurrence d'un nœud, l'attente et le checkpoint ont des identités distinctes. Un identifiant de nœud statique ne suffit pas pour les cycles ou appels répétés.
- Aucun conteneur universel Session n'est imposé. La terminologie de continuité entre plusieurs exécutions reste ouverte ; une boucle en attente conserve sa propre identité d'exécution.
- L'attente de réponse est distincte d'un await asynchrone, d'une pause de contrôle et d'une fin. Un flow arrivé à sa fin ne reprend pas par ajout d'un message.
- Une attente porte son occurrence d'origine, le type et les contraintes de réponse, son état et la continuation pertinente. Toute réponse vise cette occurrence précise.
- L'acceptation concurrente d'une réponse est atomique. L'acceptation durable et le travail de reprise doivent être liés de façon récupérable après crash. Cette garantie n'est pas une promesse d'exécution unique de tous les effets externes.
- Les interfaces partagent commandes et événements. Un abonnement reçoit un instantané cohérent et les événements suivants à partir d'un curseur, avec un traitement explicite des doublons et des trous d'historique. HTTP et SSE constituent le transport initial proposé, à confirmer dans le contrat réseau.
- Les événements ADK conservent leur provenance. Les événements applicatifs Zedflow ne doivent pas fabriquer un checkpoint ou présenter une reconstruction comme un état sauvegardé.
- Le moteur inspecté ADK 2.2.0 ne démontre pas la suspension indépendante de plusieurs branches dans un même run. Les enfants autonomes peuvent porter des attentes indépendantes si cette autonomie est réelle. La suspension fine interne exige une preuve ou une évolution ADK ; elle ne doit pas être simulée par les seuls statuts UI.

### Surfaces clientes

- Les menus de conception, bibliothèque, suivi, interaction et inspection reposent sur les mêmes identités et données. La bibliothèque initiale est une vue des compositions du workspace, pas une place de marché.
- La navigation distingue structure de composition, filiation des exécutions et historique. Une arborescence profonde unique n'est pas la seule représentation prévue.
- L'écran d'interaction associe texte et contrôles typés au graphe actif et à l'inspection des états. Les flows sans chat ne possèdent pas de champ de saisie trompeur.
- Le style retenu est sombre, sobre et proche des références visuelles approuvées. La couleur d'une nature de nœud est indépendante de son statut d'exécution. Les maquettes illustrent l'intention ; leurs détails générés ne constituent pas des contrats techniques.

## Testing Decisions

- Frontière principale proposée, à vérifier avec le porteur : l'interface publique de commandes et d'abonnement du daemon, consommée comme le ferait un client. Privilégier les comportements observables à travers cette interface aux tests des détails SQL ou des fonctions privées.
- Exercer derrière cette frontière les modules workspaces, compositions, stockage, intégration ADK et suivi, avec une base temporaire, les vrais graphes ADK et des modèles/outils déterministes hors réseau.
- Réutiliser les acquis des tests HTTP du lab : topologie affichée correspondant aux nœuds exécutés, reprise depuis l'état sauvegardé et refus d'une reprise en double. Réutiliser les expériences de sous-graphe isolé, mémoire partagée et checkpoint pour leurs comportements, sans promouvoir leurs conventions de fixtures en contrats produit.
- Ajouter un parcours de processus avec arrêt et redémarrage réels du daemon : retrouver workspace, version, historique et attente ; accepter une réponse puis reprendre la bonne composition. Tester aussi le crash entre acceptation et reprise.
- Faire concourir deux clients sur une même attente : une seule acceptation, résultat explicite pour le second et événements cohérents. Une ancienne réponse ne doit pas satisfaire le passage suivant du même nœud.
- Vérifier le document via sauvegarde et relecture, erreurs de validation localisées, conflit de révision, dépendances figées et changement de brouillon sans effet sur un run suspendu.
- Vérifier une déconnexion cliente puis le rattrapage par curseur ; contrôler que l'instantané et le flux ne perdent pas les événements survenus à leur jonction.
- Vérifier les capacités avec un graphe mixte et un sous-graphe. Un composant indisponible doit produire une raison avant exécution. Tester les comportements sensibles des natures intégrées plutôt qu'une copie de leur catalogue.
- Compléter la frontière principale par un parcours réel entre deux machines : un client ouvre le workspace de chaque daemon, les données et effets restent sur le bon hôte, les chemins identiques ne se confondent pas et une coupure ne déclenche aucun remplacement local. Deux daemons sur un seul hôte sont une fixture utile, pas la preuve de portabilité.
- Tester les accès refusés, la sortie de racine et les références inter-workspaces non autorisées par la même interface publique. Tester les commandes réessayées après perte de réponse avec leur identité conservée.
- Vérifier dans le navigateur les parcours visuels essentiels : composition enregistrée et rechargée, nœud actif, occurrence sélectionnée, attente typée, filiation enfant et resynchronisation. Éviter les assertions fragiles sur des pixels sans signification fonctionnelle.
- Ne pas annoncer la capacité de deux attentes internes et d'une branche continuant tant qu'un test ADK réel ne le prouve pas. L'exécution de plusieurs enfants autonomes est une preuve différente.
- Les tests live de modèles restent explicites. Les contrôles Rust du dépôt restent applicables aux changements d'implémentation ; la rédaction de ce PRD n'est pas une validation du futur backend.

## Out of Scope

- Réimplémentation du moteur de graphe, ordonnanceur distribué de nœuds ou remplacement d'ADK.
- Reproduction de Codex à l'identique, port Pi, compatibilité LangGraph ou restauration des anciennes exigences de fidélité.
- Réutilisation obligatoire du daemon Working OS, dépendance à son coordinateur DGX ou fusion des deux produits.
- Réplication automatique de workspaces, migration d'une exécution active entre machines, placement automatique de sous-graphes sur le mesh et basculement transparent en cas de panne.
- Place de marché, registre distant public, découverte automatique de tous les répertoires et orchestration complète des mises à jour des machines.
- Import bidirectionnel général de Rust arbitraire et exécution de code utilisateur non fiable sans travail spécifique d'isolation.
- Steering forcé, fork historique éditable, migration de compositions en cours, optimisation automatique et méta-harness dans le premier socle.
- Transport WebRTC, interfaces voix et couverture complète des clients natifs dans la première livraison. Le contrat d'interaction doit permettre leur ajout ultérieur.
- Garantie universelle d'effets externes exactement une fois.
- Suspension indépendante de branches internes présentée comme acquise sans qualification du moteur.

## Further Notes

### Sources et statut des décisions

- La conversation de conception Zedflow est la source des exigences utilisateur, notamment de la correction Projet vers Workspace et du mandat client/daemon multi-machine.
- Le contexte du reboot et les documents de composition, ressources et interaction du dépôt restent propriétaires du vocabulaire et des intentions détaillées. Le présent PRD en fixe le périmètre de réalisation sans redéfinir ces sources.
- La documentation canonique « Exécution sur le mesh » de Working OS fournit l'inspiration pour l'identité de l'hôte, la propriété locale des ressources, la continuité et les réponses perdues. Son état d'implémentation ne constitue pas une preuve de capacité Zedflow.
- Les projections visuelles approuvées précisent la direction de l'expérience. Elles ne prouvent ni l'existence d'une fonctionnalité ADK ni l'adoption des libellés ou types générés.
- Le lab Rust actuel et Studio n'ont pas une convergence de version ADK démontrée. Le format Studio et son générateur doivent être évalués avant réutilisation. L'inspecteur historique ne constitue pas déjà un serveur d'exécutions durable et réattachable.
- SQLite, le document JSON et les modules décrits sont les choix de départ proposés dans la conversation, acceptés dans leur principe. Le protocole exact, le routage entre hôtes et les détails de stockage doivent être précisés avant les tâches correspondantes ; ils ne justifient pas une redéfinition du périmètre fonctionnel.

### Ordre de livraison

1. Workspace et composition persistants : daemon local, identité de workspace, catalogue descriptif, brouillon révisé, version immuable, validation et construction ADK d'un graphe mixte avec sous-composition.
2. Ouverture distante : même contrat client vers deux daemons, admission des workspaces, hôte visible, absence de repli implicite et preuve sur deux machines.
3. Continuité d'exécution : index et historique persistants, version figée, attente typée, deux clients concurrents, redémarrage et rattrapage des événements.
4. Expérience intégrée : conception, bibliothèque, interaction texte + graphe + état et inspection des occurrences et enfants. Élargir ensuite la couverture des natures ADK en indiquant leur statut réel.

Le premier lot est réussi lorsque la fermeture puis la réouverture du client restitue une composition, sa présentation et ses références, et que sa version peut construire et lancer un véritable graphe ADK. Le socle complet est réussi lorsque ce même travail reste identifiable sur la bonne machine, survit aux déconnexions prévues et accepte une réponse depuis une autre interface sans double reprise.

### Publication

Destination proposée d'après le remote et les issues existantes : GitHub Issues du dépôt ZEDIUM-Off/zedflow. Le skill demande le label ready-for-agent, absent lors de la préparation. Après vérification des frontières de test et confirmation de cette destination, publier ce PRD avec ce label ; ne pas confondre ce label de triage avec une preuve d'implémentation ou la résolution de tous les choix détaillés.

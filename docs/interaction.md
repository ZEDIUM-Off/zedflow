# Sessions, exécutions et interfaces

Statut : conception fonctionnelle en cours. Ces intentions ne décrivent pas des
fonctionnalités toutes implémentées dans le produit. Source : précisions du porteur du
projet dans la discussion UX du 10 septembre 2026.

Les définitions de Flow, session et Flow Run appartiennent à
[composition](composition.md). Les distinctions état, store, ressource et contexte
modèle appartiennent à [ressources](resources.md). Le vocabulaire de l'interaction
est ancré dans [CONTEXT.md](../CONTEXT.md#vocabulaire-de-linteraction).

## Réponses déjà acquises

- L'interface comporte deux espaces principaux : édition, conception et analyse
  de flows ; suivi, exécution et interaction.
- La projection recherchée privilégie la place et la lisibilité des fonctions.
  L'habillage de marque et les fonctions périphériques ne sont pas le sujet des
  prochaines maquettes.
- Les sessions doivent être représentées explicitement, au-delà d'une liste de
  workflows exécutés.
- Les exécutions interactives et non interactives doivent être distinguables.
  Les exécutions non interactives terminées restent organisées et consultables.
- La filiation d'une exécution issue d'un autre flow doit être représentée.
- Une vue future pourra représenter un flow entier par un nœud et permettre une
  exploration à plusieurs échelles des grandes compositions imbriquées.
- Sessions et exécutions non interactives diffusent leurs événements vers les
  interfaces abonnées. Web, TUI, CLI et accès programmatiques doivent pouvoir
  coexister ; WebRTC et API ont été cités comme surfaces ou moyens d'accès sans
  arrêter ici une architecture de transport.
- Ouvrir ou lire une session depuis une interface ne coupe pas les autres accès.
- Une session en attente de réponse reste ouverte et accessible depuis ces
  interfaces pour permettre sa reprise.
- La première réponse acceptée pour cette attente permet la reprise. Les autres
  interfaces reçoivent l'évolution par les événements et actualisent leurs vues.
- Une attente peut porter sur autre chose qu'un texte libre.
- L'exécution doit rendre visibles les nœuds actifs et l'état pertinent ; la
  conception doit couvrir les différentes natures de nœuds ADK-Graph.

## Conséquences fonctionnelles

- Une session n'appartient pas à un onglet, à un terminal ou à une connexion.
- « En attente de réponse » et « terminée » sont des situations différentes.
- La nature interactive d'une exécution et son activité à un instant donné sont
  deux dimensions : une exécution interactive peut travailler sans attendre.
- Deux réponses visant la même attente ne doivent pas provoquer deux reprises.
  Une interface en retard ne peut pas considérer son ancien formulaire comme
  preuve que l'attente reste ouverte.
- Le statut d'une réponse doit être compréhensible : proposée ne signifie pas
  encore acceptée. Le traitement exact d'une réponse tardive reste à décider.
- L'historique d'une session, la filiation des exécutions et la structure de
  composition sont des relations différentes ; une vue ne doit pas les confondre.
- En présence de parallélisme, il peut y avoir plusieurs nœuds actifs. Le nœud
  sélectionné pour inspection peut être distinct de ces nœuds actifs.
- Le suivi d'un run doit distinguer l'état observé au point consulté et l'état
  courant, ainsi que les différents périmètres et les ressources externes.

## Réponses du deuxième tour

Source : annotations du porteur du projet sur les six questions du premier tour.

1. **Nom et unité commune — ouvert.** « Session » évoque trop le chat, « flow »
   trop le workflow. Le concept doit couvrir un comportement de sous-agent comme
   une suite de gates algorithmiques appelée par un agent. Il n'est pas décidé
   que toute exécution appartient à un conteneur session distinct. « Activité »
   est uniquement un libellé provisoire pour les maquettes.
2. **Cycle de vie — précisé.** La boucle agentique est représentée par des nœuds :
   entrée, invocation LLM, exécution d'outils, résultats, nouvelle invocation LLM
   et retour à l'entrée. Une attente d'entrée prévoit une suite dans le graphe ;
   elle ne signifie pas que l'exécution est terminée. Une exécution arrivée à sa
   fin ne reprend pas par simple nouveau message. Une pause de contrôle reste
   distincte d'une attente de réponse. Une attente async interne n'est pas une
   demande humaine. Pendant une attente d'entrée, l'exécution suspendue doit
   pouvoir être déchargée de la mémoire et retrouvée à la réponse ; le mécanisme
   de conservation n'est pas choisi. Si d'autres branches restent actives, leur
   activité doit être distinguée de la branche suspendue.
3. **Steering — différé.** Une entrée en file et un envoi manuel forcé sont des
   possibilités à conserver, de priorité faible. Leurs modalités restent ouvertes.
4. **Attentes concurrentes — accepté.** Plusieurs branches peuvent présenter des
   attentes indépendantes ; chaque interface doit les afficher et adresser les
   réponses sans ambiguïté.
5. **Accès aux enfants — accepté.** Une exécution enfant peut être ouverte
   directement, avec filiation accessible et regroupement sous le parent.
   La représentation visuelle reste à éprouver.
6. **Réponses typées — accepté.** Texte, confirmation, choix simple ou multiple,
   formulaire, fichier et validation ou édition d'un contenu sont dans le périmètre.

La référence historique `pi-port/checkpoint-2026-09-09:CONTEXT.md` décrit déjà
« Graph-Native Agent Loop », « Human Input Interrupt Boundary » et « Interrupt
Shape ». Elle éclaire cette précision ; les contraintes de port Pi et le runtime
LangGraph de cette branche ne sont pas réintroduits.

## Conséquences nouvelles

- Garder une session ouverte après la fin pour recevoir une nouvelle demande,
  comme proposé au premier tour, n'est pas une règle adoptée.
- Les actions de continuation correspondent à la situation : répondre à une
  attente, reprendre une pause, ou démarrer une nouvelle exécution.
- Une bifurcation depuis un point antérieur crée une nouvelle continuation avec
  filiation ; ce n'est pas une reprise de l'exécution terminée. Son périmètre
  précis reste futur ; elle ne rétablit pas les effets externes passés.
- Un nœud d'entrée peut être atteint à plusieurs tours. Les attentes successives
  restent distinctes : une ancienne réponse ne doit pas viser le tour suivant.
- Le statut global peut combiner des branches actives, des attentes ouvertes et
  des enfants terminés.
- Les interfaces connectées ne possèdent pas l'exécution et n'en réservent pas
  l'accès.

## Projections visuelles — deuxième tour

À chaque tour de grounding, produire plusieurs projections des faits établis,
selon la demande explicite du porteur du projet. Ces représentations accompagnent
l'entretien ; elles ne valent pas validation des dispositions ou noms proposés.

- **Vue par filiation** : comportements interactifs et automatiques dans une liste
  structurée ; accès direct à un enfant algorithmique terminé, sans champ de
  réponse ni commande de reprise.
- **Vue centrée sur le graphe** : boucle agentique dépliée, attente sur le nœud
  d'entrée, état sauvegardé et réponse contextualisée.
- **Transition entre interfaces** : deux demandes typées concurrentes, puis réponse
  à l'une depuis un CLI ; seule cette attente est résolue dans le web.

## Frontière ouverte

### Disposition d'interaction validée

Le 10 septembre 2026, le porteur du projet retient la coexistence du texte et de
la saisie avec le graphe d'exécution visible à côté, telle que représentée dans
[l'inspiration interaction 3](../output/ui-codex-dark/03-interaction.png).
Ce choix devient acquis pour la vue d'interaction. Sa direction visuelle appartient
à [ui-style.md](ui-style.md#référence-principale-validée).
Le comportement de sélection et de navigation dans ce panneau reste à préciser.
La saisie reste soumise aux capacités et attentes du flow ; cette disposition ne
rend pas tous les flows conversationnels et ne crée pas d'entrée après leur fin.

### Retour visuel — troisième tour

Le porteur du projet juge l'arbre de filiation intéressant mais potentiellement
illisible sur de nombreux niveaux. Il demande de réintroduire une richesse
fonctionnelle ADK importante dans les projections, tout en soignant davantage
l'esthétique ; sobriété de l'habillage ne signifie pas réduction des capacités.
L'exemple de réponses concurrentes est pertinent, sa présentation précédente
ne convient pas visuellement.

Trois hypothèses visuelles sont à éprouver, sans les considérer comme adoptées :

- Navigation par niveau, fil d'Ariane et aperçu global des grandes compositions,
  avec ouverture locale d'un sous-graphe au lieu d'un arbre entièrement développé.
- Suivi combinant graphe, branches concurrentes, état par périmètre, changements,
  ressources et chronologie ; distinguer les connexions structurelles des liens
  vers des exécutions enfants.
- Espace d'interaction soigné avec demandes typées, résultats et mises à jour
  provenant des autres interfaces, accompagné du contexte de graphe et d'état.

Les questions fonctionnelles ci-dessous restent ouvertes ; le retour visuel
ne constitue pas une réponse implicite à ces questions.

- **Nom commun.** Choisir un mot lisible pour le comportement en cours ou conservé,
  en distinguant sa définition de son occurrence. Candidats provisoires : activité,
  parcours, fil. Aucun n'est retenu comme terme canonique.
- **Réponse tardive.** Après résolution depuis une autre interface, conserver un
  brouillon concurrent sans l'envoyer à une autre attente ? Recommandation : oui,
  avec statut explicite.
- **Portée d'une pause.** Le contrôle vise-t-il l'élément sélectionné ou tout
  l'ensemble ? Recommandation : expliciter la cible puis définir séparément la
  propagation aux enfants.
- **Interface limitée.** Pour une interaction complexe, proposer une réponse
  équivalente lorsque possible et un accès à une interface adaptée sinon ?
  Recommandation : oui, sans réserver ni fermer l'attente.

La terminaison du parent avec un enfant actif et les détails du fork restent à
étudier dans leur périmètre. Aucun protocole, schéma de stockage ou contrat de
transport n'est choisi ici.

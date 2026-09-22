# Worker documentaire

Flow autonome appelé par le port `documentation` du harness parent, en callAwait.
Il reçoit une demande textuelle, prépare le contexte `working-system-context`,
appelle le modèle choisi explicitement et peut lire/exécuter des outils dans le
répertoire documentaire. Il retourne `response` sans posséder la boucle humaine.

Le parcours fixture complet et les clés de snapshot sont documentés dans le
README parent. Le package seul ne remplace pas l'installation des stratégies.
Aucune connaissance personnelle ni chemin absolu n'est embarqué.

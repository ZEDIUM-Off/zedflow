# Direction visuelle — référence Codex

Décision du porteur du projet : fixer un style proche de Codex tout en conservant
la richesse fonctionnelle de la composition et du suivi ADK. Relevé visuel du
10 septembre 2026, à partir de [huit captures publiques](../.agents/output/codex-style-reference/README.md).
Les captures couvrent plusieurs versions et plateformes ; elles ne constituent
pas une spécification de la version actuellement installée.

## Observations et sources

- [Application claire](https://developers.openai.com/images/codex/app/app-screenshot-light.webp)
  et [sombre](https://developers.openai.com/images/codex/app/app-screenshot-dark.webp) :
  barre latérale compacte, en-tête bas, zone de contenu principale et panneau
  latéral délimité par un séparateur discret. Les réponses sont largement du texte
  libre ; les résultats structurés sont regroupés dans des surfaces peu contrastées.
- [Formulaire clair](https://developers.openai.com/images/codex/app/create-automation-light.webp)
  et [sombre](https://developers.openai.com/images/codex/app/create-automation-dark.webp) :
  labels hiérarchisés, champs arrondis, bordures fines, action principale noire en
  clair et blanche en sombre, actions secondaires discrètes.
- [Windows sombre](https://developers.openai.com/images/codex/windows/codex-windows-dark.webp) :
  les événements d'outils secondaires restent compacts, les paragraphes importants
  ressortent, le champ de saisie regroupe ses options en pied.
- Les références tierces complètent les groupes de paramètres, l'édition d'un
  formulaire et les variations du panneau de diff. Leurs URL sont conservées dans
  le manifeste de sources ; elles ne prouvent pas la disponibilité actuelle des
  fonctions visibles.

Le bleu-violet entourant certaines fenêtres appartient au fond de présentation.
La teinte de la barre latérale peut être influencée par la transparence système ;
ne pas la transformer en couleur de marque. La police exacte et les dimensions
CSS ne peuvent pas être déduites avec certitude de ces captures redimensionnées.

## Règles retenues pour les prochaines projections

1. Surfaces neutres : blanc et gris en clair ; charbon et gris neutres en sombre.
   Retirer les fonds bleus métalliques et les éclairages des maquettes précédentes.
2. Hiérarchie par le texte, l'espace et les surfaces. Éviter l'empilement de cartes
   toutes encadrées ; utiliser des séparateurs fins entre grandes zones.
3. Typographie sans serif sobre, titres compacts, texte secondaire suffisamment
   lisible. Monospace pour le code, les clés et certaines valeurs.
4. Navigation en lignes compactes avec sélection sur fond neutre et icônes fines.
   Conserver la navigation à plusieurs échelles conçue pour Zedflow.
5. Actions principales monochromes, secondaires neutres. Les couleurs de statut
   ne deviennent pas des couleurs de décoration ou de boutons systématiques.
6. Couleur localisée : lien, diff, point de statut, petite indication d'attente.
   Associer texte ou icône à la couleur ; ne pas remplir chaque nœud selon son type.
7. Panneaux de même famille visuelle. Le graphe, l'inspecteur et le fil d'activité
   doivent sembler appartenir au même outil, avec des densités adaptées.
8. Contenu textuel sans carte quand aucune interaction ne justifie un conteneur.
   Demandes typées, diffs et résultats structurés peuvent utiliser une surface dédiée.
9. Champs et menus doucement arrondis ; arrondis plus importants pour la saisie
   principale et les dialogues. Pas de lueur de sélection.
10. En conception et inspection, conserver la richesse : scopes, mappings, reducers,
    ressources, concurrence, checkpoints, attentes et contexte d'invocation.
    Le style réduit le bruit visuel, pas la couverture fonctionnelle.

## Valeurs de départ proposées pour Zedflow

Ces valeurs sont des choix de travail à éprouver, pas des tokens extraits de Codex.

| Élément | Proposition |
|---|---|
| Fond sombre principal | `#181818` |
| Surface sombre secondaire | `#222222` |
| Surface sombre surélevée | `#2B2B2B` |
| Bordure sombre | `#333333` |
| Texte sombre principal / secondaire | `#F1F1F1` / `#A0A0A0` |
| Fond clair principal / secondaire | `#FFFFFF` / `#F6F6F6` |
| Bordure claire | `#E7E7E7` |
| Texte clair principal / secondaire | `#1B1B1B` / `#6B6B6B` |
| Texte UI / métadonnées | 14 px / 12 px |
| Texte de lecture | 15–16 px |
| Titres de panneau | 14–16 px, medium ou semibold |
| Contrôles / surfaces / saisie | rayons 6–8 / 10–12 / 18–22 px |
| Espacement | progression 4, 8, 12, 16, 24 px |

## Consigne de génération

### Référence principale validée

Le porteur du projet a validé visuellement
[l'inspiration 3 — interaction](../.agents/output/ui-codex-dark/03-interaction.png)
le 10 septembre 2026. Elle devient la référence principale pour les prochaines
projections Zedflow : essence Codex sombre, texte et saisie au centre, graphe
d'exécution visible à côté. Cette validation porte sur le style et la disposition
générale ; elle ne valide pas chaque texte, statut ou connexion générés dans l'image.
Les valeurs de départ ci-dessus restent des propositions à ajuster à cette référence.

Joindre les captures comme références de style aux prochaines générations,
en séparant explicitement leur rôle des références fonctionnelles Zedflow.
Préserver les concepts du grounding ; reprendre les surfaces, contrastes,
typographie, contrôles et rythmes visuels observés dans Codex. Pas de copie du
fond d'écran, du contenu des conversations, de la marque ou des rubriques sans
utilité pour Zedflow. Explorer les thèmes clair et sombre avec la même structure.

## Application aux retours du 11 septembre 2026

Les captures fournies par le porteur du projet dans la tâche de refonte complètent
les références publiques : sidebar de projets, composer compact, réponses libres,
outils regroupés et détails à la demande. Leur contenu de conversation sert de
référence visuelle uniquement. L'implémentation est décrite dans
[le parcours de l'application](app.md#navigation-chronologie-et-inspection).

Les surfaces de l'espace Exécution utilisent `#0b0b0e` pour le fond, `#101013`
pour la sidebar et `#1c1c21` pour le composer et les demandes utilisateur. Les
réponses assistant restent sans carte ; le texte de lecture est de 14 px avec un
interligne de 1,75. La colonne complète est bornée à 850 px, rail compris. Les
couleurs de statut restent localisées aux repères et aux erreurs.

La sidebar démarre à 280 px, entre 220 et 440 px, et réserve les actions secondaires
aux menus. Le rail relie les résultats visibles aux passages du graphe. Les outils
occupent une ligne par action ; leurs détails se déplient sans perdre arguments,
sorties ou durées. Le composer réunit le flow, les modèles et la réponse attendue.
Le graphe peut occuper tout l'espace principal pour une inspection approfondie.

Les formes distinguent les rôles de contrôle et les quatre pièces de capacité
restent attachées à leur agent. Les chemins orthogonaux contournent ces deux types
d'obstacles. Les ports Oui/Non portent la sémantique des conditions ; une différence
de couleur seule ne suffit pas à exprimer une branche.

# Studio de contexte structuré

Évolution demandée le 13 septembre 2026. Cette page porte les critères de cette
réalisation ; [context-engine.md](../context-engine.md) conserve la sémantique du
moteur et [ui-style.md](../ui-style.md) la direction visuelle de l’application.

## Références conservées

- [Stratégie vide](../../.agents/output/context-design-structured/01-empty-strategy.png).
- [Galerie des types](../../.agents/output/context-design-structured/02-source-type-gallery.png).
- [Composition par champs](../../.agents/output/context-design-structured/03-field-composition.png).
- [Images extraites de n8n et manifeste](../../.agents/output/context-design-structured/n8n-images/index.html).
- [Comparaison des maquettes et de l’interface réalisée](../../.agents/output/context-design-structured/implemented/index.html).

Les maquettes sont des références visuelles. Les états simulés, données et compteurs
qu’elles représentent doivent être produits par des fonctions réelles dans l’éditeur.

## Critères à vérifier

| ID | Fonction ou contrainte | État / preuve |
|---|---|---|
| S1 | Nouvelle stratégie : Sources vide ; déclaration distincte d’une injection | Implémenté — stratégie neuve et tiroir vide vérifiés en E2E. |
| S2 | Galerie recherchable, catégories, sélection multiple et validation explicite | Implémenté — galerie, recherche, catégories, multisélection et Échap testés. |
| S3 | Catalogue extensible aux types connus, y compris ceux exposés par les flows | Implémenté — catalogue daemon des types intégrés, embarqués et exposés ; déduplication avec provenances et conflits testée. |
| S4 | Seules les sources déclarées apparaissent dans le tiroir ; alias et types distincts | Implémenté — aliases, doublons intentionnels et renommage avec réécriture des références et jeux nommés testés. |
| S5 | Couleur stable par type, nom et icône dans les trois panneaux | Implémenté — couleurs de types stables, labels français et icônes ; contrôle après sauvegarde/réouverture. |
| S6 | Arbre de champs typés ; glisser et insertion clavier dans une expression compatible | Implémenté — champs, prises typées, glisser et clavier ; profondeur32 testée dans le navigateur, profondeur64 en API/Rust/Cargo et dépassements refusés explicitement. |
| S7 | Sources conversation, instructions, skills, outils/calls/results, documents, données, états et médias | Implémenté — catalogue extensible, types outils distincts et médias par référence. |
| P1 | Blocs numérotés, ordre explicite, déplacement, duplication, suppression et annulation | Implémenté — réordonnancement dans/entre branches, cycle refusé, duplication, suppression et invalidation de l’annulation au rechargement testés. |
| P2 | Expressions compactes avec paramètres avancés accessibles sans perdre la grammaire | Implémenté — prises compactes, résumés sémantiques stricts et éditeur complet dépliable ; profondeur64 avec diagnostic explicite. |
| P3 | Conditions typées Champ/Opérateur/Valeur, ET/OU/NON et branches imbriquées | Implémenté — prédicats typés et issue réelle des conditions issue de la trace ; aucun badge sur un aperçu périmé. |
| P4 | Boucles par élément, variables lexicales, groupes repliables et émissions multiples | Implémenté — forEach lexical à émissions multiples ; tests moteur et navigation de plusieurs occurrences. |
| P5 | Projections, filtrage, tri, sélection, templates, appels réutilisables et limites | Implémenté — grammaire conservée, listes construites et troncature Unicode ajoutées ; source Rust exacte compilée. |
| P6 | Source et champ sélectionnés reliés visuellement à leurs utilisations | Implémenté — jetons de champs typés, sélection des prises, sources consultées et surbrillance des résultats associés. |
| A1 | Prévisualisation autonome sans autorisation de flow ni effet réel | Corrigé — aperçu sans grant ni effet ; liaison au flow toujours contrôlée par le runtime. |
| A2 | Jeux d’essai sélectionnables et éditables, données absentes distinctes des valeurs vides | Implémenté — presets, valeurs manuelles/JSON, jeux nommés dans le brouillon ; renommage/retrait propagés. |
| A3 | Actualisation après édition ; états attente, calcul, obsolescence et erreur explicites | Implémenté — aperçu différé automatique, statut courant/périmé/erreur et réponses tardives ignorées ; courses testées. |
| A4 | Vues Contexte / Structure, fragments ordonnés et messages/médias lisibles | Implémenté — Contexte/Structure, rôles, groupes, données et références média ; contenus complets inspectables. |
| A5 | Association appel/résultat conservée ; erreur d’un outil distincte d’un diagnostic du moteur | Implémenté — deux prises appel/résultat avec identités conservées ; erreur outil rendue séparément des diagnostics de stratégie. |
| A6 | Navigation fragment → bloc → source, trace et itérations précisément identifiées | Implémenté — trace des sources et itérations, résultat sélectionné par identité et navigation au bloc exact. |
| I1 | Trois panneaux redimensionnables ; lecture et sélection conservées | Implémenté — proportions des maquettes, séparateurs souris/clavier et préférence locale ; navigation et sélection conservées. |
| I2 | Adaptation étroite, clavier, focus et fermeture des menus | Implémenté — panneaux étroits, menus Reka, focus et clavier ; scénario720px sans débordement horizontal. |
| I3 | Stratégies/types sauvegardables et partageables ; compatibilité vérifiée par les flows | Implémenté — types embarqués avec la stratégie, archives autonomes et compilation stricte des liaisons. |
| I4 | Bibliothèques, catalogues, Rust, conflits, brouillons et navigation existants préservés | Implémenté — bibliothèques, Rust, conflits, brouillons et conversion v1 → v2 vérifiés ; 87 scénarios E2E validés, dont une relance ciblée après un délai dépassé. |
| V1 | Comparaison visuelle des trois états aux maquettes, grand écran et écran étroit | Vérifié — captures réelles des trois états à1922×912 et d’un écran720px, relues et conservées avec leur manifeste dans la galerie comparative. |
| V2 | Scénario complexe : nesting, champs, conditions, boucle, aperçu et réouverture | Implémenté et vérifié — exemple éditable « Conversation, outils et documents », également utilisé par le scénario E2E de référence. |
| V3 | Suites frontend, E2E et Rust demandées, fixtures et daemon isolé | Vérifié — typecheck/build, 87 scénarios E2E dont une relance ciblée, 374 tests Rust avec CODEGEN, format/check/Clippy/toutes features ; détails et incidents dans [validation.md](../validation.md#studio-de-contexte-structuré--13-septembre-2026). |

## Sémantique à préserver

Déclarer un type ne lit aucune ressource et n’accorde aucun outil. Le programme
émet explicitement ce qui entre dans le contexte. La disponibilité d’un type dans
le catalogue ne prouve pas qu’un flow en fournit une instance. La validation de la
liaison au flow reste stricte ; l’aperçu utilise uniquement les valeurs d’essai.

Un appel d’outil enregistré, sa définition et son résultat sont des ressources
différentes. Afficher un résultat d’échec ne constitue pas une erreur d’évaluation
de la stratégie. Les messages structurés gardent leurs rôles et leurs associations.

Les couleurs, dimensions des panneaux et groupes repliés sont de la présentation.
L’ordre des émissions et les portées des variables appartiennent au programme.

## Comparaison visuelle

Le contrôle visuel porte sur la stratégie vide, la galerie ouverte avec six types
cochés, puis la composition instructions/conversation/condition/boucle. Les trois
panneaux, surfaces neutres, bordures, cartes de sources colorées, jetons de champs,
lignes de messages compactes et sorties ordonnées reprennent les maquettes. Les
réglages complets restent accessibles dans les menus et volets dépliables.

Les noms de champs viennent du schéma réel (`content`, `status`, `callId`) ; ils ne
sont pas renommés pour imiter le texte de l’image. Le compteur compte les fragments
émis, y compris les deux messages du groupe Conversation : ce scénario en produit
cinq, répartis en quatre ensembles visibles. Les champs d’une liste appartiennent
à la portée de son bloc `Pour chaque` et ne deviennent pas des valeurs scalaires
utilisables directement hors de la boucle.

Le jeu éditable correspondant s’ouvre depuis **Comprendre les blocs → Conversation,
outils et documents**. Il ne lit aucun fichier et ne lance aucun outil. Son
programme, ses types et ses données d’essai sont produits par
[`contextExamples.ts`](../../web/apps/client/src/contextExamples.ts) ; le scénario E2E
visuel utilise cette même définition.

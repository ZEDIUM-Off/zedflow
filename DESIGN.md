# Design

La source canonique du style reste [docs/ui-style.md](docs/ui-style.md).

Pour le studio de contexte, les trois maquettes approuvées sont conservées dans
[output/context-design-structured](.agents/output/context-design-structured/index.html) :
stratégie vide, galerie des types et composition par champs. La troisième est la
référence principale de disposition. Le [suivi de réalisation](docs/prd/context-studio-structured.md)
distingue les fonctions représentées de leur validation effective.

Conserver le chrome sombre neutre de l’application, les bordures fines, la
typographie compacte et trois panneaux Sources, Programme, Aperçu. Les couleurs
localisées identifient les types de sources à travers les trois panneaux ; elles
ne désignent pas simultanément les opérateurs et les rôles des messages.

Les blocs suivent un ordre explicite, avec titres compacts, numérotation et détails
repliables. Les emplacements de valeurs portent des jetons nommés. Les branches et
les boucles restent lisibles sans ouvrir tous leurs paramètres. La version étroite
utilise des onglets de panneaux avec état conservé.

Les composants vivent dans `web/apps/client`, les adaptateurs réactifs dans
`web/packages/vue`. Les données métier passent par les contrats du SDK, jamais
par une seconde implémentation du protocole dans les composants.

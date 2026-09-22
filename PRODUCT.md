# Product

## Register

product

## Users

Zedflow s’adresse aux développeurs et aux chercheurs en IA qui composent des
harness et souhaitent contrôler précisément les ressources et requêtes de leurs
modèles. L’intention et le vocabulaire restent définis dans [CONTEXT.md](CONTEXT.md).

## Product Purpose

Composer, exécuter et inspecter des flows indépendants et leurs stratégies de
contexte. Le [plan du context engine](docs/context-engine.md) porte les contrats ;
le [suivi du studio structuré](docs/prd/context-studio-structured.md) porte cette
évolution de l’éditeur.

## Brand Personality

Précis, sobre, explicable. L’interface de travail n’affiche pas de branding.

## Anti-references

Pas de blocs enfantins, de couleurs décoratives saturées ou de graphe libre dont
la position détermine implicitement l’ordre. Les formulaires détaillés ne doivent
pas masquer la structure d’un programme complexe.

## Design Principles

- Séparer déclaration des sources, traitement et résultat effectivement émis.
- Conserver les capacités avancées derrière une lecture compacte et progressive.
- Rendre visibles les types, la provenance et les dépendances.
- Préserver les brouillons et permettre de naviguer entre définition et résultat.

## Accessibility & Inclusion

Les actions de déplacement et d’insertion possèdent une alternative au clavier.
Les noms et icônes complètent les couleurs. La direction visuelle et les références
approuvées sont conservées dans [docs/ui-style.md](docs/ui-style.md).

## Implementation boundary

Le produit Rust/web expose les frontières de [l'architecture](docs/architecture/crates.md),
le [SDK public](docs/development/sdk.md) et les [packages de flows](docs/development/flow-packages.md).
La migration 0.2.0 ne livre ni registre distribué ni optimiseur. Les critères de
qualification et les résultats historiques sont distingués dans [validation](docs/validation.md).

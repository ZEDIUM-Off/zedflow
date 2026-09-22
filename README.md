# Zedflow

Composer, exécuter et inspecter des flows ADK-Rust, avec un daemon Rust, un client
Vue et un SDK TypeScript par domaines. ADK 2.2.0 conserve l'exécution des graphes ;
Zedflow possède la compilation de composition, l'admission et la continuité locale.
Le laboratoire ADK, son ancien web et ADK Studio ne sont plus des applications du dépôt.

## Essayer sans credentials

Prérequis : Node 24, pnpm 11.5.1, Rust 1.96.1. Depuis la racine du dépôt :

```sh
pnpm --dir web install --frozen-lockfile
pnpm --dir web build
export CARGO_TARGET_DIR=/tmp/zedflow-adk-target
REPO="$(pwd -P)"
FIXTURE="$(mktemp -d /tmp/zedflow-demo.XXXXXX)"
mkdir -p "$FIXTURE/workspace" "$FIXTURE/home"
(cd rust && cargo run --locked -p zf-serve --bin zedflow-daemon -- \
  --listen 127.0.0.1:3158 --web "$REPO/web/apps/client/dist" \
  --workspace "$FIXTURE/workspace" --data "$FIXTURE/data" \
  --flow-home "$FIXTURE/home" --context-home "$FIXTURE/home")
```

Ouvrir <http://127.0.0.1:3158>, choisir le fournisseur **Fixture**. Aucun compte ni
modèle réel n'est nécessaire. Les outils exécutent leurs effets avec les droits du
daemon : le workspace temporaire n'est pas un sandbox. Arrêter avec Ctrl-C.
Les options home ci-dessus ne déplacent ni HOME, ni CODEX_HOME, ni les credentials.

Cargo doit partir de `rust/` : `--manifest-path rust/Cargo.toml` seul depuis la
racine sélectionne le toolchain par défaut de la machine, pas le pin imbriqué.
Les flags des gates restent les mêmes ; voir [validation](docs/validation.md).

## Organisation

- `rust/crates/` : [bibliothèques et services](docs/architecture/crates.md), CLI `zf`
  et binaire public `zedflow-daemon`.
- `web/packages/sdk/` : [SDK Zod](docs/development/sdk.md), transports injectables,
  erreurs, synchronisation, cache et octets raw ; `web/packages/vue/` : lifecycle Vue.
- `web/apps/client/`, `web/apps/desktop/` : client web et shell Electron.
- `e2e/` : Playwright indépendant, fixtures locales et daemon dédié.
- `tooling/dev/`, `tooling/releases/` : outils Node autonomes, hors workspace web.
- `examples/working-system/` : [package déterministe](examples/working-system/README.md).

Les nouveaux flows sont des [packages](docs/development/flow-packages.md) dans
`.zedflow/flow/<id>/`. Les anciens fichiers restent lisibles et leur conversion
est explicite ; les sources historiques des runs restent exactes.

## Guides

- [Intention et vocabulaire](CONTEXT.md), [produit](PRODUCT.md), [design](DESIGN.md)
- [Application](docs/app.md), [versions locales](docs/app-versioning.md)
- [Composition](docs/composition.md), [ressources](docs/resources.md),
  [interaction](docs/interaction.md), [évolution](docs/evolution.md)
- [Formats Rust](docs/flow-format.md), [contexte](docs/context-engine.md),
  [API de contexte](docs/context-api.md)
- [Catalogue ADK](docs/adk.md), [provenances tierces](THIRD_PARTY_NOTICES.md)
- [Gates de validation](docs/validation.md), [candidat 0.2.0](docs/releases/0.2.0.md)

Aucune publication npm/crates.io ni installation multiplateforme n'est promise.
L'accès distant passe par un tunnel privé ; le daemon n'est pas un service public.

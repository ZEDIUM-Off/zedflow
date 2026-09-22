# Working System — composition locale reproductible

Ces sources sont celles du starter existant : harness interactif, worker documentaire,
stratégie documentaire et stratégie de conversation. Le répertoire `../docs` est
relatif au workspace d'exécution, jamais au checkout. Les fichiers `flow.json`
inventorient les deux packages ; ils ne fusionnent pas bridges et contextes en un
nouveau format.

Le bridge fourni référence la clé **documentation** du `CompilationSnapshot`
construit ci-dessous. Cette clé n'est pas un identifiant de catalogue universel :
**ne pas copier ce bridge tel quel dans un catalogue**. Dans l'application,
« Installer Working System » (`POST /api/examples/working-system`, avec
`workingDirectory`) installe les mêmes définitions et écrit la clé réelle du worker.
Le parcours suivant compile les sources de référence sans installation personnelle,
avec les API publiques existantes de la CLI. Il ne copie aucune fiche canonique.

Depuis la racine du dépôt :

```sh
REPO="$(pwd -P)"
FIXTURE="$(mktemp -d /tmp/zedflow-working-system.XXXXXX)"
export EXAMPLE="$FIXTURE/reference" SNAPSHOT="$FIXTURE/snapshot.json"
mkdir -p "$FIXTURE/workspace" "$FIXTURE/docs" "$FIXTURE/home"
cp -R examples/working-system "$EXAMPLE"
printf '# Documentation fixture\nAucun appel modèle réel.\n' > "$FIXTURE/docs/AGENTS.md"
node --input-type=module <<'JS'
import { readFile, writeFile } from 'node:fs/promises'
import { createHash } from 'node:crypto'
import { join } from 'node:path'
async function source(path) {
  const source = await readFile(join(process.env.EXAMPLE, path), 'utf8')
  return { source, hash: createHash('sha256').update(source).digest('hex') }
}
await writeFile(process.env.SNAPSHOT, JSON.stringify({
  flows: { root: await source('flow.rs'), documentation: await source('worker/flow.rs') },
  bridges: { 'working-system': await source('bridge.rs') },
  programs: { strategies: {
    'working-system-context': await source('context.rs'),
    'conversation-default': await source('conversation-context.rs'),
  }, libraries: {}, types: {} },
}))
JS
export CARGO_TARGET_DIR=/tmp/zedflow-adk-target
(cd rust && cargo run --locked -p zf-cli --bin zf -- validate "$EXAMPLE")
(cd rust && cargo run --locked -p zf-cli --bin zf -- validate "$EXAMPLE/worker")
(cd rust && cargo run --locked -p zf-cli --bin zf -- compile "$SNAPSHOT" --snapshot --flow root --bridge working-system) > "$FIXTURE/plan.json"
(cd rust && cargo run --locked -p zf-cli --bin zf -- --workspace "$FIXTURE/workspace" export "$FIXTURE/plan.json" --output "$FIXTURE/export")
(cd "$FIXTURE/export" && cargo run --locked --quiet -- --workspace "$FIXTURE/workspace" --home "$FIXTURE/home" --data "$FIXTURE/data" --run-id fixture \
  --models '{"root/model":{"provider":"fixture"},"working-system/documentation/model":{"provider":"fixture"}}' \
  --input '{"input":"AGENTS.md"}')
```

Le résultat est une attente humaine avec état et checkpoint persistés. Les modèles
**fixture** sont explicitement choisis ; aucun credential ni service personnel
n'est utilisé. L'export est un workspace Cargo autonome, son lock est obligatoire.
Ne pas remplacer HOME/CODEX_HOME. Un arrêt/reprise utilise le même data/run-id,
pas une nouvelle session ni une réexécution des effets déjà acquis.

Le README du worker précise sa responsabilité. Les gates et résultats observés
sont consignés dans `docs/validation.md` et le rapport P7.2 ; ces commandes ne
constituent pas une qualification Windows/macOS ni une évaluation de modèle live.

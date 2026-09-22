# SDK TypeScript

`@zedflow/sdk` expose core, daemon, workspaces, flows, composition, context,
models, runs, sessions et generation via son point d'entrée construit. Zod 4
dérive les entrées/sorties ; la validation refuse les contrats invalides sans
retirer les canaux JSON ouverts. Le SDK ne requiert ni Vue, ni Vite, ni Electron,
ni accès DOM à l'import.

```sh
pnpm --dir web install --frozen-lockfile
pnpm --dir web --filter @zedflow/sdk build
pnpm --dir web test:sdk
```

Un consommateur indépendant installe `file:/chemin/absolu/zedflow/web/packages/sdk`
après le build, et importe `@zedflow/sdk`, jamais `src/`. Exemple Node sans réseau :

```js
import { createClient, jsonObjectSchema } from '@zedflow/sdk'
const client = createClient({
  baseUrl: 'https://fixture.invalid/api', protocol: 1,
  fetch: async () => new Response('{"future":{"value":42}}'),
})
const value = await client.transport.json(
  { operation: 'fixture', method: 'GET', path: 'data' }, jsonObjectSchema,
)
console.log(value.future)
```

Le protocole/version du consommateur est injecté, pas importé depuis buildInfo.
Les transports et lifecycle navigateur sont explicites ; les erreurs réseau,
HTTP, JSON et schéma restent distinctes. Le raw est un Uint8Array original :
l'affichage texte ne remplace jamais les octets archivés.

La clé de cache inclut instance SDK, workspace, run, type/id de détail, révision
et query. Une frame est validée avant mutation ; un delta conserve les identités
intactes sans revalider/cloner tout l'historique. `@zedflow/vue` possède le
lifecycle réactif, pas un second transport ou cache métier.

`tests/consumer.test.ts` copie uniquement dist/package.json et Zod dans un dossier
extérieur : import Node sans DOM, contrat TypeScript et bundle navigateur y sont
exercés. Les tests Node déplacés depuis Playwright résident dans le SDK ; ceux
qui testent les vraies conversions UI restent dans `web/apps/client/tests/`.
Les gates et leur portée effective sont décrites dans [validation](../validation.md).

## Identités d'inspection

`hash` désigne les octets Rust, `package.root` l'inventaire du package et
`definitionRevision` l'identité exécutable complète (source, package et choix
de contexte). Ne pas substituer l'une à l'autre. Le cache d'inspection et le
téléchargement utilisent `definitionRevision ?? hash` pour les captures legacy.
Sans sélection, les deux paramètres `nodePath`/`occurrenceId` sont omis ; un
passage explicitement choisi conserve ses identifiants et son pin historique.

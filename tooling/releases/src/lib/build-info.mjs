import { createHash } from 'node:crypto'
import { readFileSync, readdirSync, statSync } from 'node:fs'
import { join, relative } from 'node:path'
import { execFileSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'
export const repository = fileURLToPath(new URL('../../../..', import.meta.url))
export const CLIENT_INPUTS = [
  'version.json', 'web/package.json', 'web/pnpm-workspace.yaml', 'web/pnpm-lock.yaml',
  'web/apps/client/src', 'web/apps/client/public', 'web/apps/client/index.html',
  'web/apps/client/package.json', 'web/apps/client/vite.config.ts', 'web/apps/client/tsconfig.json',
  'web/packages/sdk/src', 'web/packages/sdk/package.json', 'web/packages/sdk/tsconfig.json',
  'web/packages/vue/src', 'web/packages/vue/package.json', 'web/packages/vue/tsconfig.json',
  'tooling/releases/src/lib/build-info.mjs',
]
export function clientBuild(root = repository) {
  const version = JSON.parse(readFileSync(join(root, 'version.json'), 'utf8'))
  const hash = createHash('sha256')
  function visit(path) {
    if (statSync(path).isDirectory()) {
      for (const name of readdirSync(path).sort()) visit(join(path, name))
    } else {
      hash.update(relative(root, path).replaceAll('\\', '/')); hash.update('\0'); hash.update(readFileSync(path)); hash.update('\0')
    }
  }
  for (const name of CLIENT_INPUTS) visit(join(root, name))
  let revision = null
  try { revision = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: root, stdio: ['ignore','pipe','ignore'] }).toString().trim() } catch {}
  // Revision is embedded in the delivered client, so it is part of its cache identity.
  hash.update('revision\0'); hash.update(JSON.stringify(revision)); hash.update('\0')
  return { ...version, component: 'client', buildId: hash.digest('hex'), revision }
}

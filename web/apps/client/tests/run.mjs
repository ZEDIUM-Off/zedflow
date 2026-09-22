import { build } from 'esbuild'
import { spawnSync } from 'node:child_process'
import { mkdtempSync, readdirSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'

const directory = mkdtempSync(join(tmpdir(), 'zedflow-client-tests-'))
try {
  const source = fileURLToPath(new URL('.', import.meta.url))
  const tests = readdirSync(source).filter(name => name.endsWith('.test.ts'))
  await build({ entryPoints: tests.map(name => join(source, name)), bundle: true,
    platform: 'node', format: 'esm', outdir: directory, outExtension: { '.js': '.mjs' } })
  const result = spawnSync(process.execPath, ['--test', ...tests.map(name => join(directory, name.replace('.ts', '.mjs')))],
    { cwd: directory, stdio: 'inherit' })
  if (result.error) throw result.error
  process.exitCode = result.status ?? 1
} finally {
  rmSync(directory, { recursive: true, force: true })
}

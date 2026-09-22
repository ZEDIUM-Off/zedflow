import { test, expect } from '@playwright/test'
import { mkdir, readFile, writeFile } from 'node:fs/promises'
import { join } from 'node:path'
import { flowPackageSnapshotSchema, jsonObjectSchema } from '@zedflow/sdk'
import { fixturePath, saveFlow, waitRun } from './helpers'
import { legacyTemplate } from './fixtures/templates'

test('package admission preserves source hashes and refuses changed asset or dependency pins', async ({ request }) => {
  const workspaceId = (await (await request.get('/api/workspaces')).json()).find((w: { path: string }) => w.path === fixturePath('workspace-a')).id
  const composition = (id: string) => {
    const flow = legacyTemplate(false)
    flow.id = id; flow.name = id; flow.formatVersion = 3
    flow.nodes[0]!.data.config.exports = { contract: { entries: { main: { input: { kind: 'text' }, output: { kind: 'text' } } } }, entries: { main: { node: 'start', inputField: 'input', outputField: 'response' } }, interactive: false }
    return flow
  }
  const root = await saveFlow(request, composition('pins-root'), workspaceId)
  const dependency = await saveFlow(request, composition('pins-dependency'), workspaceId)
  const manifest = JSON.parse(await readFile(join(root.path, 'flow.json'), 'utf8'))
  manifest.files.push('asset.txt')
  manifest.dependencies = { dependency: { path: '../pins-dependency' } }
  await writeFile(join(root.path, 'asset.txt'), 'original asset')
  await writeFile(join(root.path, 'flow.json'), JSON.stringify(manifest))
  const dependencyManifest = JSON.parse(await readFile(join(dependency.path, 'flow.json'), 'utf8'))
  dependencyManifest.files.push('dependency-asset.txt')
  await writeFile(join(dependency.path, 'dependency-asset.txt'), 'original dependency')
  await writeFile(join(dependency.path, 'flow.json'), JSON.stringify(dependencyManifest))
  const dependencySource = await readFile(join(dependency.path, 'flow.rs'), 'utf8')
  const source = await readFile(join(root.path, 'flow.rs'), 'utf8')
  const prepare = await request.post('/api/runtime-graphs/prepare', { data: { workspaceId, selection: { flow: root.key, entry: 'main', bridges: [], flowHashes: {}, bridgeHashes: {}, contexts: {} } } })
  expect(prepare.ok(), await prepare.text()).toBeTruthy()
  const prepared = await prepare.json()
  const definitions = prepared.runtime.definitions
  const packages = jsonObjectSchema.parse(definitions.flowPackages)
  const pins: Record<string, string> = {}
  for (const [key, hash] of Object.entries(definitions.flowHashes)) {
    pins[key] = packages[key] ? flowPackageSnapshotSchema.parse(packages[key]).root : String(hash)
  }
  expect(pins[root.key]).not.toBe(definitions.flowHashes[root.key])
  const selection = { flow: root.key, entry: 'main', bridges: [], flowHashes: pins, bridgeHashes: {}, contexts: {} }
  const launch = () => request.post('/api/runs', { data: { workspaceId, runtimeSelection: selection, input: { input: 'fixture pins' } } })
  const accepted = await launch()
  expect(accepted.ok(), await accepted.text()).toBeTruthy()
  const run = await accepted.json()
  await waitRun(request, run.id, 'completed', workspaceId)
  const before = await (await request.get(`/api/runs/${run.id}?workspaceId=${workspaceId}`)).json()
  await writeFile(join(root.path, 'asset.txt'), 'changed asset')
  let refused = await launch()
  expect(refused.status(), await refused.text()).toBe(409)
  await writeFile(join(root.path, 'asset.txt'), 'original asset')
  await writeFile(join(dependency.path, 'dependency-asset.txt'), 'changed dependency asset')
  expect(await readFile(join(dependency.path, 'flow.rs'), 'utf8')).toBe(dependencySource)
  refused = await launch()
  expect(refused.status(), await refused.text()).toBe(409)
  expect(await readFile(join(root.path, 'flow.rs'), 'utf8')).toBe(source)
  const after = await (await request.get(`/api/runs/${run.id}?workspaceId=${workspaceId}`)).json()
  expect(after.flowSource).toBe(before.flowSource)
  expect(after.runtimeSelection).toEqual(before.runtimeSelection)

  const legacyDirectory = fixturePath('workspace-a', '.zedflow', 'flows')
  await mkdir(legacyDirectory, { recursive: true })
  await writeFile(join(legacyDirectory, 'pins-legacy.rs'), source.replaceAll('"pins-root"', '"pins-legacy"'))
  const files = await (await request.get(`/api/flows?workspaceId=${workspaceId}`)).json()
  const legacy = files.find((file: { path: string }) => file.path === join(legacyDirectory, 'pins-legacy.rs'))
  expect(legacy).toBeTruthy()
  const legacyRun = await request.post('/api/runs', { data: { workspaceId, runtimeSelection: { flow: legacy.key, entry: 'main', bridges: [], flowHashes: { [legacy.key]: legacy.hash }, bridgeHashes: {}, contexts: {} }, input: { input: 'legacy fixture' } } })
  expect(legacyRun.ok(), await legacyRun.text()).toBeTruthy()
  await waitRun(request, (await legacyRun.json()).id, 'completed', workspaceId)
})

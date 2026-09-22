import { test } from 'node:test'
import assert from 'node:assert/strict'
import { preparedFlowHashes } from '../src/compositionEngine'

test('prepared pins retain every resolved package revision and legacy source hash without changing history', () => {
  const definitions = {
    flowHashes: { root: 'root-rust', worker: 'worker-rust', legacy: 'legacy-rust' },
    bridgeHashes: { route: 'bridge-rust' },
    flowPackages: {
      root: { root: 'root-package', packages: {} },
      worker: { root: 'worker-package-with-dependency', packages: {} },
    },
  }
  const before = structuredClone(definitions)
  assert.deepEqual(preparedFlowHashes(definitions), {
    root: 'root-package', worker: 'worker-package-with-dependency', legacy: 'legacy-rust',
  })
  assert.deepEqual(definitions, before)
  definitions.flowPackages.worker.root = 'worker-package-after-asset-change'
  assert.equal(preparedFlowHashes(definitions).worker, 'worker-package-after-asset-change')
  assert.equal(definitions.flowHashes.worker, 'worker-rust')
  assert.deepEqual(preparedFlowHashes({ flowHashes: { legacy: 'exact-source' }, bridgeHashes: {} }), { legacy: 'exact-source' })
  assert.throws(() => preparedFlowHashes({ flowHashes: { root: 'rust' }, bridgeHashes: {}, flowPackages: { root: { root: 42 } } }))
})

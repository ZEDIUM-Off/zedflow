import { test } from 'node:test'
import assert from 'node:assert/strict'
import { runtimeGraphSummarySchema } from '@zedflow/sdk'
import { runtimeModelNodes } from '../src/compositionEngine'
import captures from '../../../packages/sdk/tests/fixtures/daemon-prepared-overviews.json'

test('runtimeModelNodes keeps null and absent paths unselected and maps a paired context exactly', () => {
  const unpaired = runtimeGraphSummarySchema.parse(captures.unpaired.overview)
  const absent = structuredClone(unpaired)
  delete absent.inferences['root/model']!.contextPath
  delete absent.inferences['root/model']!.context
  for (const [overview, expectedPath] of [[unpaired, null], [absent, undefined]] as const) {
    const nodes = runtimeModelNodes(overview)
    assert.equal(nodes[0]!.contextPath, expectedPath)
    assert.equal(nodes[0]!.context, undefined)
    // AgentContextPanel's selectedPath is a required string (empty when unselected).
    assert.equal(nodes.find(node => node.path === '' || node.contextPath === ''), undefined)
    assert.equal([nodes[0]!.path, nodes[0]!.contextPath].includes('root/context'), false)
  }
  const paired = runtimeGraphSummarySchema.parse(captures.paired.overview)
  const model = runtimeModelNodes(paired)[0]!
  assert.equal(model.path, 'root/model')
  assert.equal(model.contextPath, 'root/context')
  assert.equal(model.context?.id, 'context')
  assert.deepEqual(model.context?.data.config, paired.inferences['root/model']!.context!.config)
})

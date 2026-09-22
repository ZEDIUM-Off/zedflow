import { test } from 'node:test'
import assert from 'node:assert/strict'
import { executedSourceQuery } from '../src/composables/useExecutedDefinition'

test('whole-run source omits node and occurrence while explicit passages retain both', () => {
  assert.deepEqual(executedSourceQuery(''), {})
  assert.deepEqual(executedSourceQuery('root/output'), { nodePath: 'root/output' })
  assert.deepEqual(executedSourceQuery('root/output', 'old-passage'), { nodePath: 'root/output', occurrenceId: 'old-passage' })
  assert.throws(() => executedSourceQuery('', 'orphaned-passage'), /requires its node path/)
})

import { executedDefinitionRequest } from '../src/composables/useExecutedDefinition'
import { createClient, type RunSummary } from '@zedflow/sdk'

test('initial definition identity shares reads until admission content, import origin or scope changes', async () => {
  const run: RunSummary = {id:'run',name:'Run',status:'waiting',workspaceId:'workspace',createdAt:1,runtimeGraphRef:'graph-initial',flowSourceRef:'source',flowPackageRef:'package'}
  let calls=0
  const client=createClient({baseUrl:'https://fixture.invalid/api',protocol:1,fetch:async url=>{
    calls++
    const path=new URL(url).searchParams.get('nodePath')||''
    return new Response(JSON.stringify({exact:true,runId:run.id,instance:'root',nodePath:path,key:'root',hash:'source-hash',definitionRevision:'executable-revision',source:'// exact',composition:{id:'root',name:'Root',revision:0,nodes:[],edges:[]}}))
  }})
  try {
    const initial=executedDefinitionRequest(run)
    const selected=await client.definitions.load(initial)
    assert.equal(await client.definitions.load(initial),selected)
    assert.equal(await client.definitions.load(executedDefinitionRequest({...run,revision:12,activities:[],updatedAt:22})),selected)
    assert.equal(calls,1)
    for(const changed of [{runtimeGraphRef:'graph-replaced'},{flowPackageRef:'new-package'}, {workspaceId:'another-workspace'}, {import:{archiveHash:'archive',importedAt:2}}]) {
      await client.definitions.load(executedDefinitionRequest({...run,...changed}))
    }
    assert.equal(calls,5)
    client.definitions.invalidate(initial)
    await client.definitions.load(initial)
    assert.equal(calls,6)
    const unknown={id:'run',name:'Run',status:'waiting',workspaceId:'workspace'}
    assert.equal(executedDefinitionRequest(unknown).revision,undefined)
    await client.definitions.load(executedDefinitionRequest(unknown))
    await client.definitions.load(executedDefinitionRequest(unknown))
    assert.equal(calls,8, 'bare run IDs must not become permanent cache identities')
    await client.definitions.load(executedDefinitionRequest(run,'root/result'))
    await client.definitions.load(executedDefinitionRequest(run,'root/result'))
    assert.equal(calls,10,'node-latest stays uncached')
    assert.equal(executedDefinitionRequest(run,'root/result','old-passage').revision,undefined)
    assert.deepEqual(executedDefinitionRequest(run,'root/result','old-passage').query,{nodePath:'root/result',occurrenceId:'old-passage'})
  } finally {client.dispose()}
})

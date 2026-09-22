import { test } from 'node:test'
import assert from 'node:assert/strict'
import parallelJoin from '../../../../e2e/specs/fixtures/parallelJoin.json'
import type { Composition } from '@zedflow/sdk'
import { harnessTemplate, legacyHarnessTemplate, legacyTemplate, separateContextNodes, upgradeHarnessRouting } from '../src/templates'

test('Harness upgrade accepts sparse Rust exports without losing existing contracts or mutating the source',()=>{
  for(const original of [legacyHarnessTemplate(),separateContextNodes(legacyHarnessTemplate(),'workspace-default',false)]){
    const existing:Record<string,any>={contract:{data:{summary:{dataType:{kind:'text'},permissions:{read:true,write:false}}},entries:{review:{input:{kind:'text'}}}},entries:{review:{node:'start',inputField:'input'}},data:{summary:'response'},types:{Summary:{kind:'text'}}}
    if(original.formatVersion===3)existing.contract.inferenceNodes={model:{model:{kind:'runtime'},contextStrategy:'workspace-default',resources:['summary'],capabilities:['read']}}
    original.nodes.find(node=>node.id==='start')!.data.config.exports=structuredClone(existing)
    const snapshot=structuredClone(original),upgraded=upgradeHarnessRouting(original,false),exposed=upgraded.nodes.find(node=>node.id==='start')!.data.config.exports
    assert.deepEqual(original, snapshot)
    assert.deepEqual(exposed.contract.data, existing.contract.data)
    assert.deepEqual(exposed.contract.entries.review, existing.contract.entries.review)
    assert.deepEqual(exposed.entries.review, existing.entries.review)
    assert.deepEqual(exposed.data, existing.data)
    assert.deepEqual(exposed.types, existing.types)
    assert.deepEqual(exposed.contract.requires, {})
    assert.deepEqual(exposed.requires, {})
    assert.equal(exposed.branches.work, 'dispatch')
    assert.equal(exposed.contract.inferenceNodes.model.contextStrategy, 'harness-default')
    if(existing.contract.inferenceNodes){
      assert.deepEqual(exposed.contract.inferenceNodes.model.resources, ['summary'])
      assert.deepEqual(exposed.contract.inferenceNodes.model.capabilities, ['read'])
    }
    assert.deepEqual(upgradeHarnessRouting(upgraded,false), upgraded)
  }
})

test('Harness upgrade rejects customized dispatch behavior and bypassed or missing routing edges',()=>{
  const canonical=harnessTemplate()
  const edits:((flow:Composition)=>void)[]=[
    ...Object.entries({invocation:'node',field:'customResult',inputField:'customInput',fallback:'previous result',routeId:'chosen/route',retry:{maxAttempts:3}}).map(([key,value])=>(flow:Composition)=>{flow.nodes.find(node=>node.id==='dispatch')!.data.config[key]=value}),
    flow=>{delete flow.nodes.find(node=>node.id==='dispatch')!.data.config.fallback},
    flow=>{flow.edges=flow.edges.filter(edge=>edge.target!=='dispatch')},
    flow=>{flow.edges=flow.edges.filter(edge=>edge.source!=='dispatch')},
    flow=>{flow.edges.push({id:'bypass-routing',source:'steering',target:'context'})},
    flow=>{flow.edges.push({id:'extra-routing-output',source:'dispatch',target:'inbox'})},
    flow=>{flow.edges.find(edge=>edge.target==='dispatch')!.sourceHandle='custom'},
    flow=>{flow.nodes.find(node=>node.id==='start')!.data.config.exports.contract.branches.work.invocations=['node']},
    flow=>{flow.nodes.find(node=>node.id==='start')!.data.config.exports.contract.branches.work.contract.output={kind:'number'}},
  ]
  for(const edit of edits){
    const source=structuredClone(canonical);edit(source)
    const snapshot=structuredClone(source)
    assert.throws(()=>upgradeHarnessRouting(source,false), /personnalisé/)
    assert.deepEqual(source, snapshot)
  }
})

test('Harness upgrade preserves a consistent custom input field and rejects inconsistent input wiring',()=>{
  const source=legacyHarnessTemplate()
  source.nodes.find(node=>node.id==='model')!.data.config.inputField='prompt'
  for(const id of ['steering','inbox'])source.nodes.find(node=>node.id===id)!.data.config.field='prompt'
  const upgraded=upgradeHarnessRouting(source,false)
  assert.equal(upgraded.nodes.find(node=>node.id==='dispatch')!.data.config.inputField, 'prompt')
  assert.deepEqual(upgraded.nodes.find(node=>node.id==='context')!.data.config.contextBindings.input, {kind:'state',field:'prompt'})
  assert.equal(upgraded.nodes.find(node=>node.id==='context')!.data.config.contextBindings.history.inputField, 'prompt')
  assert.equal(upgraded.nodes.find(node=>node.id==='start')!.data.config.exports.entries.main.inputField, 'prompt')
  source.nodes.find(node=>node.id==='inbox')!.data.config.field='input'
  assert.throws(()=>upgradeHarnessRouting(source,false), new RegExp('même champ d’entrée'))
})


test('parallel join conversion preserves the all-predecessors barrier before context preparation',()=>{
  const original=legacyTemplate(false)
  original.name='Jointure préservée'
  original.settings={maxConcurrency:4}
  delete original.nodes.find(node=>node.id==='model')!.data.config.fanIn
  original.nodes.push(
    {id:'fast',type:'flow',position:{x:200,y:0},data:{kind:'set',label:'Première branche',config:{field:'fast',value:'ready'}}},
    {id:'delay',type:'flow',position:{x:200,y:250},data:{kind:'tool',label:'Branche retardée',config:{tool:'delay',arguments:{milliseconds:250},field:'delayResult'}}},
    {id:'slow',type:'flow',position:{x:450,y:250},data:{kind:'set',label:'Deuxième branche',config:{field:'slow',value:'ready'}}},
  )
  original.edges=original.edges.filter(edge=>edge.target!=='model')
  original.edges.push(
    {id:'start-fast',source:'start',target:'fast'},
    {id:'start-delay',source:'start',target:'delay'},
    {id:'delay-slow',source:'delay',target:'slow'},
    {id:'fast-model',source:'fast',target:'model'},
    {id:'slow-model',source:'slow',target:'model'},
  )
  const converted=separateContextNodes(original)
  // Only the copied flow ID is generated; node/edge IDs and all contracts are stable.
  const expected={...parallelJoin,id:converted.id}
  assert.notEqual(converted.id, original.id)
  assert.deepEqual(converted, expected)
  for(const mutation of ['lost','redirected']){
    const broken=structuredClone(converted)
    if(mutation==='lost')broken.edges=broken.edges.filter(edge=>edge.id!=='slow-model')
    else broken.edges.find(edge=>edge.id==='slow-model')!.target='model'
    assert.throws(()=>assert.deepEqual(broken,expected), assert.AssertionError)
  }
})

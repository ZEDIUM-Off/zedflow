import assert from 'node:assert/strict'
import test from 'node:test'
import { effectScope, ref, shallowRef } from 'vue'
import { useRun, ownClient } from '../dist/index.js'

test('switching a target closes its follower and ignores stale publications', () => {
  const handles = [], callbacks = []
  const client = { followRun(options) { callbacks.push(options); const handle={closeCount:0,state:undefined,close(){this.closeCount++},refresh:async()=>{},prependTimeline(){}};handles.push(handle);return handle } }
  const scope=effectScope(),target=shallowRef({runId:'a',workspaceId:'one'})
  const run=scope.run(()=>useRun(client,target))
  target.value={runId:'a',workspaceId:'two'}
  assert.equal(handles[0].closeCount,1)
  callbacks[0].receive({run:{id:'old'},revision:1,cursor:1})
  callbacks[0].status({transport:'sse'})
  assert.equal(run.status.value.transport,'offline')
  assert.equal(run.state.value,undefined)
  callbacks[1].receive({run:{id:'new'},revision:2,cursor:2})
  assert.equal(run.state.value.run.id,'new')
  scope.stop()
  assert.equal(handles[1].closeCount,1)
  callbacks[1].receive({run:{id:'late'},revision:3,cursor:3})
  assert.equal(run.state.value.run.id,'new')
})
test('clearing a target resets presentation and stops the stream',()=>{
  let closed=0
  const client={followRun(){return{close(){closed++},refresh:async()=>{},prependTimeline(){}}}}
  const scope=effectScope(),target=shallowRef({runId:'a',workspaceId:'one'})
  const run=scope.run(()=>useRun(client,target))
  target.value=null
  assert.equal(closed,1);assert.equal(run.state.value,undefined);assert.equal(run.status.value.transport,'offline')
  scope.stop();assert.equal(closed,1)
})
test('client ownership releases resources at scope disposal exactly once',()=>{
  let disposed=0;const client={dispose(){disposed++}},scope=effectScope()
  scope.run(()=>assert.equal(ownClient(client),client))
  scope.stop();scope.stop();assert.equal(disposed,1)
})

test('initial projection retains its reference and timeline operations keep their scope',()=>{
  const initial={run:{id:'a'},revision:4,cursor:7},scope=effectScope(),calls=[]
  const client={followRun(){return{state:initial,close(){},refresh:async()=>{calls.push('refresh')},prependTimeline(identity,page){calls.push([identity,page])}}}}
  const run=scope.run(()=>useRun(client,{runId:'a',workspaceId:'one',initial}))
  assert.equal(run.state.value,initial)
  const identity={runId:'a',workspaceId:'one'},page={entries:[],hasMore:false,before:null}
  run.prependTimeline(identity,page)
  assert.equal(calls[0][0],identity);assert.equal(calls[0][1],page)
  scope.stop()
  run.prependTimeline(identity,page)
  assert.equal(calls.length,1)
})


test('mutating reactive target identities replaces the subscription without retaining its scope',()=>{
  const handles=[],callbacks=[],scope=effectScope(),target=ref({runId:'a',workspaceId:'one'})
  const client={followRun(options){callbacks.push(options);const handle={closed:0,close(){this.closed++},refresh:async()=>{},prependTimeline(){}};handles.push(handle);return handle}}
  const run=scope.run(()=>useRun(client,target))
  target.value.workspaceId='two'
  assert.equal(handles[0].closed,1)
  assert.deepEqual(callbacks.map(value=>[value.runId,value.workspaceId]),[['a','one'],['a','two']])
  callbacks[0].receive({run:{id:'obsolete'},revision:1,cursor:1})
  assert.equal(run.state.value,undefined)
  target.value.runId='b'
  assert.equal(handles[1].closed,1)
  assert.deepEqual(callbacks.map(value=>[value.runId,value.workspaceId]),[['a','one'],['a','two'],['b','two']])
  scope.stop()
  assert.equal(handles[2].closed,1)
})

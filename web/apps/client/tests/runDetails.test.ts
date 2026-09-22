import { test } from 'node:test'
import assert from 'node:assert/strict'
import { createRenderer, defineComponent, h, nextTick, shallowRef } from 'vue'
import { createClient, contextSnapshotSchema, detailVersion, type ContextSnapshot, type Run } from '@zedflow/sdk'
import { provideClient } from '@zedflow/vue'
import { provideRunDetails, useContextDetail } from '../src/composables/runDetails'

// No browser or mocked composable: real Vue injection, watchers and SDK cache.
const renderer=createRenderer<object,object>({
  insert(){},remove(){},createElement(){return {}},createText(){return {}},createComment(){return {}},
  setText(){},setElementText(){},parentNode(){return null},nextSibling(){return null},patchProp(){},
})
async function flush(){for(let i=0;i<4;i++){await new Promise(resolve=>setTimeout(resolve,0));await nextTick()}}

test('useContextDetail shares hydrated identity and isolates changed references, scopes and late responses', async()=>{
  const run=shallowRef<Run>({id:'run-a',workspaceId:'workspace-a',name:'Fixture',status:'waiting',composition:{id:'flow',name:'Flow',revision:0,nodes:[],edges:[]},state:{},messages:[],wait:null})
  const index=shallowRef<ContextSnapshot>({invocationId:'snapshot',nodePath:'model',contentRef:'content-1'})
  const calls:{url:URL;resolve:(response:Response)=>void}[]=[]
  const client=createClient({baseUrl:'https://fixture.invalid/api',protocol:1,fetch:async url=>new Promise<Response>(resolve=>{calls.push({url:new URL(url),resolve})})})
  let parent!:ReturnType<typeof useContextDetail>,child!:ReturnType<typeof useContextDetail>
  const Child=defineComponent({setup(){child=useContextDetail(()=>parent.snapshot.value);return ()=>null}})
  const Parent=defineComponent({setup(){parent=useContextDetail(()=>index.value);return ()=>h(Child)}})
  const Scope=defineComponent({setup(){provideRunDetails(()=>run.value);return ()=>h(Parent)}})
  const Root=defineComponent({setup(){provideClient(client);return ()=>h(Scope)}})
  const app=renderer.createApp(Root)
  function complete(i:number,label:string,extra:Record<string,unknown>={}){
    // Like context_detail, the body has no self contentRef.
    calls[i].resolve(new Response(JSON.stringify({invocationId:calls[i].url.pathname.split('/').at(-1),resources:[{id:'instructions',kind:'instructions',content:label}],...extra})))
  }
  try {
    app.mount({});await flush()
    assert.equal(calls.length,1)
    complete(0,'first');await flush()
    assert.equal(parent.snapshot.value?.resources?.[0].content,'first')
    assert.equal(child.snapshot.value?.resources?.[0].content,'first')
    assert.equal(child.snapshot.value?.contentRef,'content-1')
    assert.equal(calls.length,1,'parent and nested child must share the exact same key')
    index.value={...index.value,contentRef:'content-2'};await flush()
    assert.equal(calls.length,2)
    assert.equal(parent.snapshot.value?.resources,undefined,'old body is not relabelled with the new reference')
    index.value={...index.value,contentRef:'content-3'};await flush()
    assert.equal(calls.length,3)
    complete(1,'obsolete second');await flush()
    assert.equal(child.snapshot.value?.resources,undefined)
    complete(2,'third');await flush()
    assert.equal(child.snapshot.value?.resources?.[0].content,'third')
    assert.equal(child.snapshot.value?.contentRef,'content-3')
    assert.equal(calls.length,3)
    index.value={...index.value,detailRevision:'revision-4'};await flush()
    assert.equal(calls.length,4)
    complete(3,'fourth',{detailRevision:'not-the-index-revision'});await flush()
    assert.equal(child.snapshot.value?.detailRevision,'revision-4')
    assert.equal(child.snapshot.value?.resources?.[0].content,'fourth')
    assert.equal(calls.length,4)
    run.value={...run.value,workspaceId:'workspace-b'};await flush()
    assert.equal(calls.length,5)
    assert.equal(calls[4].url.searchParams.get('workspaceId'),'workspace-b')
    run.value={...run.value,id:'run-b'};await flush()
    assert.equal(calls.length,6)
    assert.match(calls[5].url.pathname,/runs\/run-b\/context\/snapshot$/)
    complete(5,'other run');await flush();complete(4,'late workspace');await flush()
    assert.equal(child.snapshot.value?.resources?.[0].content,'other run')
    index.value={invocationId:'snapshot-other',contentRef:'other-content'};await flush()
    assert.equal(calls.length,7)
    complete(6,'other snapshot');await flush()
    assert.equal(child.snapshot.value?.invocationId,'snapshot-other')
    assert.equal(child.snapshot.value?.resources?.[0].content,'other snapshot')
    run.value={...run.value,id:'run-error'};await flush()
    assert.equal(calls.length,8)
    calls[7].resolve(new Response('{"error":"fixture failure"}',{status:500}));await flush()
    assert.ok(parent.entry.value?.error)
    assert.ok(child.entry.value?.error)
    assert.equal(child.entry.value?.value,undefined)
    assert.equal(child.snapshot.value?.resources,undefined)
    assert.equal(calls.length,8)
  } finally {app.unmount();client.dispose()}
})

function contextDetailsFixture(initial: ContextSnapshot) {
  const index=shallowRef<ContextSnapshot>(initial)
  const run=shallowRef<Run>({id:'run',workspaceId:'workspace',name:'Fixture',status:'waiting',composition:{id:'flow',name:'Flow',revision:0,nodes:[],edges:[]},state:{},messages:[],wait:null})
  const calls:((response:Response)=>void)[]=[]
  const client=createClient({baseUrl:'https://fixture.invalid/api',protocol:1,fetch:async()=>new Promise<Response>(resolve=>calls.push(resolve))})
  let parent!:ReturnType<typeof useContextDetail>,child!:ReturnType<typeof useContextDetail>
  const Child=defineComponent({setup(){child=useContextDetail(()=>parent.snapshot.value);return ()=>null}})
  const Parent=defineComponent({setup(){parent=useContextDetail(()=>index.value);return ()=>h(Child)}})
  const Scope=defineComponent({setup(){provideRunDetails(()=>run.value);return ()=>h(Parent)}})
  const Root=defineComponent({setup(){provideClient(client);return ()=>h(Scope)}})
  const app=renderer.createApp(Root)
  app.mount({})
  return {
    index,calls,parent,child,
    complete(i:number,content:string){calls[i](new Response(JSON.stringify({invocationId:'snapshot',resources:[{id:'context',kind:'instructions',content}]})))},
    close(){app.unmount();client.dispose()},
  }
}

for(const pending of [false,true])test(`useContextDetail never relabels ${pending?'in-flight':'loaded'} A as B at constant detailRevision`,async()=>{
  const fixture=contextDetailsFixture({invocationId:'snapshot',contentRef:'A',detailRevision:'R'})
  const {index,calls,parent,child,complete}=fixture
  const observations:{reference:unknown;body:unknown}[]=[]
  function observe(){
    for(const detail of [parent,child]){
      const value=detail.snapshot.value
      observations.push({reference:value?.contentRef,body:value?.resources?.[0].content})
      assert.equal(observations.some(value=>value.reference==='B'&&value.body==='body A'),false,'body A must never carry reference B')
    }
  }
  try {
    await flush();assert.equal(calls.length,1)
    if(!pending){complete(0,'body A');await flush();observe()}
    index.value={invocationId:'snapshot',contentRef:'B',detailRevision:'R'}
    observe();await flush();observe()
    if(pending){complete(0,'body A');await flush();observe()}
    assert.equal(calls.length,2,'one acquisition for each composite identity, shared by parent and child')
    for(const detail of [parent,child])assert.equal(detail.snapshot.value?.resources,undefined)
    complete(1,'body B');await flush();observe()
    for(const detail of [parent,child]){
      assert.equal(detail.snapshot.value?.resources?.[0].content,'body B')
      assert.equal(detail.snapshot.value?.contentRef,'B')
      assert.equal(detail.snapshot.value?.detailRevision,'R')
    }
    assert.equal(calls.length,2)
    index.value={invocationId:'snapshot',contentRef:'A',detailRevision:'R'};await flush();observe()
    for(const detail of [parent,child])assert.equal(detail.snapshot.value?.resources?.[0].content,'body A')
    assert.equal(calls.length,2,'old identity remains valid in its own cache slot')
  } finally {fixture.close()}
})


test('context detail identities preserve references-only and revisions-only, types, empty values and tuple boundaries',async()=>{
  const identities:ContextSnapshot[]=[
    {invocationId:'snapshot',contentRef:'a/b',detailRevision:'c'},
    {invocationId:'snapshot',contentRef:'a',detailRevision:'b/c'},
    {invocationId:'snapshot',contentRef:'a',detailRevision:0},
    {invocationId:'snapshot',contentRef:'a',detailRevision:'0'},
    {invocationId:'snapshot',detailRevision:'revision-only'},
    {invocationId:'snapshot',detailRevision:''},
    {invocationId:'snapshot',contentRef:'',detailRevision:''},
    {invocationId:'snapshot',contentRef:'collision'},
    {invocationId:'snapshot',contentRef:'collision',detailRevision:detailVersion({contentRef:'collision'})},
    {invocationId:'snapshot',contentRef:'reference-only'},
    {invocationId:'snapshot',contentRef:'reference-only-next'},
  ]
  for(const identity of identities)assert.deepEqual(contextSnapshotSchema.parse(identity),identity)
  assert.equal(contextSnapshotSchema.safeParse({invocationId:'snapshot',contentRef:null}).success,false)
  const fixture=contextDetailsFixture(identities[0])
  try {
    for(const [i,identity] of identities.entries()){
      fixture.index.value=identity;await flush()
      assert.equal(fixture.calls.length,i+1)
      for(const detail of [fixture.parent,fixture.child])assert.equal(detail.snapshot.value?.resources,undefined)
      fixture.complete(i,`body ${i}`);await flush()
      for(const detail of [fixture.parent,fixture.child])assert.equal(detail.snapshot.value?.resources?.[0].content,`body ${i}`)
      assert.equal(fixture.calls.length,i+1)
    }
    // detailRevision is open JSON in a context snapshot; null is not a usable
    // revision and retains detailVersion's existing reference-only semantics.
    fixture.index.value=contextSnapshotSchema.parse({...identities.at(-1),detailRevision:null});await flush()
    assert.equal(fixture.calls.length,identities.length)
    for(const detail of [fixture.parent,fixture.child])assert.equal(detail.snapshot.value?.resources?.[0].content,`body ${identities.length-1}`)
  } finally {fixture.close()}
})

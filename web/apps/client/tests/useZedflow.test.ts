import assert from 'node:assert/strict'
import { test, type TestContext } from 'node:test'
import { createRenderer, nextTick } from 'vue'
import { clientKey } from '@zedflow/vue'
import { createClient, type Composition, type FlowFile } from '@zedflow/sdk'
import { useZedflow } from '../src/composables/useZedflow'

// Exercise the real composable, Vue lifecycle and public SDK over controlled HTTP.
const renderer=createRenderer<object,object>({
  insert(){},remove(){},createElement(){return {}},createText(){return {}},createComment(){return {}},
  setText(){},setElementText(){},parentNode(){return null},nextSibling(){return null},patchProp(){},
})
const flush=async()=>{for(let i=0;i<60;i++)await nextTick()}
const json=(body:unknown,status=200)=>new Response(JSON.stringify(body),{status})
function deferred<T>() {let resolve!:(value:T)=>void;const promise=new Promise<T>(r=>resolve=r);return {promise,resolve}}
const composition:Composition={id:'flow',name:'Saved',revision:1,nodes:[],edges:[]}
const file=(doc:Composition=composition,key='key'):FlowFile=>({key,id:doc.id,name:doc.name,path:`/${key}/flow.rs`,scope:'workspace',workspaceId:'A',hash:`hash-${doc.revision}`,composition:doc,diagnostics:[]})
const models={models:[],providers:[]},context={instructions:[],skills:[],diagnostics:[]}
type Request={url:URL;body:Record<string,unknown>;reply:ReturnType<typeof deferred<Response>>}
async function setup(t:TestContext){
  const browser=Object.assign(new EventTarget(),{location:{href:'https://fixture.invalid/'},navigator:{onLine:true}})
  Object.defineProperty(globalThis,'window',{configurable:true,value:browser})
  Object.defineProperty(globalThis,'document',{configurable:true,value:Object.assign(new EventTarget(),{visibilityState:'visible'})})
  let controlled=false
  const saves:Request[]=[],lists:Request[]=[],capabilities:Request[]=[],generated:Request[]=[]
  const client=createClient({baseUrl:'https://fixture.invalid/api',protocol:1,fetch:async(url,init)=>{
    const request={url:new URL(url),body:JSON.parse(String(init?.body||'{}')),reply:deferred<Response>()},path=request.url.pathname
    if(path==='/api/health')return json({defaultWorkspaceId:'A'})
    if(path==='/api/workspaces')return json(['A','B'].map(id=>({id,name:id,path:`/${id}`,open:true})))
    if(path==='/api/runs')return json([])
    if(path==='/api/flows'&&init?.method==='POST'){saves.push(request);return request.reply.promise}
    if(path==='/api/flows'){if(!controlled)return json([]);lists.push(request);return request.reply.promise}
    if(path==='/api/models'||path==='/api/context'){
      if(!controlled)return json(path==='/api/models'?models:context)
      capabilities.push(request);return request.reply.promise
    }
    if(path==='/api/generate'){generated.push(request);return json({files:[]})}
    throw new Error(`Unexpected HTTP ${path}`)
  }})
  let z!:ReturnType<typeof useZedflow>
  const app=renderer.createApp({setup(){z=useZedflow();return ()=>null}});app.provide(clientKey,client)
  app.mount({});await flush();assert.equal(z.initialized.value,true);assert.equal(z.error.value,'')
  controlled=true;z.openDesign(file())
  t.after(()=>{app.unmount();client.dispose();delete (globalThis as {window?:unknown}).window;delete (globalThis as {document?:unknown}).document})
  function readyCapabilities(){for(const req of capabilities)req.reply.resolve(json(req.url.pathname==='/api/models'?models:context))}
  async function ack(index=0){const saved=file({...saves[index].body.composition as Composition,revision:2});saves[index].reply.resolve(json(saved));await flush();return saved}
  return {z,saves,lists,capabilities,generated,ack,readyCapabilities}
}

for(const kind of ['new','dirty','clean'] as const)test(`useZedflow launchDesign ${kind}: one final catalogue, navigation waits for catalogue AND capabilities`,async t=>{
  const f=await setup(t),{z,saves,lists,capabilities}=f
  if(kind==='new')z.createFlow('interactive');else if(kind==='dirty')z.doc.value.name='Edited'
  const launching=z.launchDesign();await flush();assert.equal(z.mode.value,'design')
  let saved=file()
  if(kind!=='clean'){assert.equal(saves.length,1);assert.equal(lists.length,0);saved=await f.ack()}
  else assert.equal(saves.length,0)
  assert.equal(lists.length,1);assert.equal(capabilities.length,2,'capabilities must start with the final refresh, not after another list')
  const fresh={...saved,hash:'external-hash',composition:{...saved.composition!,name:'Externally edited',revision:3}}
  lists[0].reply.resolve(json([fresh]));await flush();assert.equal(z.mode.value,'design')
  capabilities[0].reply.resolve(json(models));await flush();assert.equal(z.mode.value,'design')
  capabilities[1].reply.resolve(json(context));await launching
  assert.equal(lists.length,1);assert.equal(z.mode.value,'execution');assert.equal(z.executionFlow.value?.hash,'external-hash')
})

for(const dirty of [false,true])test(`useZedflow launchDesign switches to the design workspace, dirty=${dirty}, waiting for its fresh catalogue`,async t=>{
  const f=await setup(t),{z,lists}=f
  z.designWorkspaceId.value='B';if(dirty)z.doc.value.name='Submitted in B'
  const launching=z.launchDesign();await flush()
  let saved=file()
  if(dirty){assert.equal(f.saves[0].body.workspaceId,'B');saved=await f.ack()}
  else assert.equal(f.saves.length,0)
  assert.equal(z.workspaceId.value,'B');assert.equal(lists[0].url.searchParams.get('workspaceId'),'B')
  f.readyCapabilities();await flush();assert.equal(z.mode.value,'design')
  lists[0].reply.resolve(json([saved]));await launching;assert.equal(z.mode.value,'execution')
})

for(const phase of ['ACK','catalogue','capabilities'] as const)test(`useZedflow late A ${phase} never navigates or publishes notices/catalogue in B`,async t=>{
  const f=await setup(t),{z,lists}=f;z.doc.value.name='Edited'
  const launching=z.launchDesign();await flush()
  let saved:FlowFile|undefined
  if(phase!=='ACK'){saved=await f.ack();if(phase==='capabilities'){lists[0].reply.resolve(json([saved]));await flush()}}
  const switching=z.selectWorkspace('B');await flush()
  const b=lists.find(r=>r.url.searchParams.get('workspaceId')==='B')!;assert.ok(b)
  b.reply.resolve(json([file({...composition,id:'B'},'B')]));f.readyCapabilities();await switching
  if(phase==='ACK')await f.ack();else if(phase==='catalogue')lists[0].reply.resolve(json([saved]))
  await flush()
  // Fail promptly on an obsolete A continuation, rather than await its accidental extra refresh.
  assert.equal(z.workspaceId.value,'B');assert.equal(z.mode.value,'design');assert.equal(z.notice.value,'')
  assert.equal(z.flows.value[0]?.key,'B');assert.equal(lists.filter(r=>r.url.searchParams.get('workspaceId')==='A').length,phase==='ACK'?0:1)
  assert.equal(lists.length,phase==='ACK'?1:2,'obsolete operation must not start another refresh in B')
  await launching
})

test('useZedflow edits during ACK retain dirty draft and acknowledged hash/baseline',async t=>{
  const f=await setup(t),{z,lists}=f;z.doc.value.name='Submitted'
  const launching=z.launchDesign();await flush();z.doc.value.name='Later edit'
  const saved=await f.ack();assert.equal(z.dirty.value,true);assert.equal(z.designFile.value?.hash,saved.hash)
  assert.equal(z.doc.value.name,'Later edit');assert.equal(z.doc.value.revision,2)
  lists[0].reply.resolve(json([saved]));f.readyCapabilities();await flush()
  assert.equal(lists.length,1);await launching
  assert.equal(z.dirty.value,true);assert.equal(z.doc.value.name,'Later edit');assert.match(z.notice.value,/Version envoyée/)
  z.doc.value.name='Submitted';assert.equal(z.dirty.value,false,'baseline corresponds to the ACK, not the later edit')
})

for(const replacement of ['deleted','converted','invalid','different-id'] as const)test(`useZedflow ${replacement} after ACK: no stale selection or fallback`,async t=>{
  const f=await setup(t),{z,lists}=f;z.doc.value.name='Submitted';const launching=z.launchDesign();await flush();const saved=await f.ack()
  const catalogue=replacement==='deleted'?[]:replacement==='converted'?[{...saved,key:'converted-key'}]:replacement==='invalid'?[{...saved,composition:undefined,diagnostics:['invalid']}]:[{...saved,composition:{...saved.composition!,id:'replacement'}}]
  lists[0].reply.resolve(json(catalogue));f.readyCapabilities();await flush()
  assert.equal(lists.length,1);await launching;assert.equal(z.mode.value,'design');assert.equal(z.dirty.value,false)
  assert.match(z.error.value,/indisponible.*catalogue actualisé/);assert.match(z.notice.value,/enregistrée/)
})

test('useZedflow clean missing flow never claims persistence',async t=>{
  const f=await setup(t),{z,lists}=f;const launching=z.launchDesign();await flush()
  lists[0].reply.resolve(json([]));f.readyCapabilities();await launching
  assert.equal(z.mode.value,'design');assert.equal(z.notice.value,'');assert.match(z.error.value,/Flow indisponible/)
})

test('useZedflow ACK then refresh failure preserves persistence, clean retry never POSTs again',async t=>{
  const f=await setup(t),{z,lists,saves}=f;z.doc.value.name='Submitted';const launching=z.launchDesign();await flush();const saved=await f.ack()
  lists[0].reply.resolve(json({error:'catalogue unavailable'},500));f.readyCapabilities();await launching
  assert.equal(z.mode.value,'design');assert.equal(z.dirty.value,false);assert.equal(z.designFile.value?.hash,saved.hash)
  assert.match(z.error.value,/actualisation.*catalogue unavailable/i);assert.match(z.notice.value,/enregistrée/)
  const retry=z.launchDesign();await flush();assert.equal(saves.length,1)
  lists[1].reply.resolve(json([saved]));f.readyCapabilities();await retry;assert.equal(z.mode.value,'execution')
})

test('useZedflow rejected save neither refreshes nor navigates',async t=>{
  const f=await setup(t),{z,saves,lists}=f;z.doc.value.name='Submitted';const launching=z.launchDesign();await flush()
  saves[0].reply.resolve(json({error:'save rejected'},409));await launching
  assert.equal(z.mode.value,'design');assert.equal(lists.length,0);assert.equal(z.notice.value,'');assert.equal(z.dirty.value,true);assert.match(z.error.value,/save rejected/)
})

test('useZedflow superseded final refresh cancels navigation with an explicit retry diagnostic',async t=>{
  const f=await setup(t),{z,lists}=f;const launching=z.launchDesign();await flush()
  const concurrent=z.refresh();await flush();lists[1].reply.resolve(json([file({...composition,name:'New catalogue'})]));await concurrent
  lists[0].reply.resolve(json([file()]));f.readyCapabilities();await launching
  assert.equal(z.mode.value,'design');assert.equal(z.flows.value[0].name,'New catalogue');assert.match(z.error.value,/actualisation plus récente.*Réessayez/)
})

test('useZedflow new document during ACK is never replaced or launched',async t=>{
  const f=await setup(t),{z,lists}=f;z.doc.value.name='Submitted';const launching=z.launchDesign();await flush();z.createFlow('interactive');const id=z.doc.value.id
  await f.ack();await flush();assert.equal(lists.length,0);await launching
  assert.equal(z.doc.value.id,id);assert.equal(z.designFile.value,null);assert.equal(z.notice.value,'');assert.equal(z.mode.value,'design')
})

for(const action of ['save','generate'] as const)test(`useZedflow ${action} retains its catalogue barrier`,async t=>{
  const f=await setup(t),{z,lists,generated}=f;z.doc.value.name='Submitted';let finished=false
  const saving=z[action]().then(()=>{finished=true});await flush();const saved=await f.ack()
  assert.equal(lists.length,1);assert.equal(finished,false);assert.equal(generated.length,0)
  lists[0].reply.resolve(json([saved]));await saving
  assert.equal(z.mode.value,'design');assert.equal(finished,true);assert.equal(generated.length,action==='generate'?1:0)
})

test('useZedflow late refresh failure in A cannot report an error in B',async t=>{
  const f=await setup(t),{z,lists}=f;z.doc.value.name='Submitted';const launching=z.launchDesign();await flush();await f.ack()
  const switching=z.selectWorkspace('B');await flush();lists[1].reply.resolve(json([]));f.readyCapabilities();await switching
  lists[0].reply.resolve(json({error:'late A failure'},500));await launching
  assert.equal(z.workspaceId.value,'B');assert.equal(z.error.value,'');assert.equal(z.notice.value,'');assert.equal(z.mode.value,'design')
})

test('useZedflow edits after ACK and before failed refresh remain dirty and receive only a submitted-version notice',async t=>{
  const f=await setup(t),{z,lists}=f;z.doc.value.name='Submitted';const launching=z.launchDesign();await flush();await f.ack()
  z.doc.value.name='Edited after ACK';lists[0].reply.resolve(json({error:'refresh failed'},500));f.readyCapabilities();await launching
  assert.equal(z.dirty.value,true);assert.equal(z.doc.value.name,'Edited after ACK');assert.match(z.notice.value,/Version envoyée/)
  assert.equal(z.mode.value,'design');assert.match(z.error.value,/actualisation/)
})

for(const dirty of [true,false])test(`useZedflow invalid-with-composition, dirty=${dirty}: fresh collision refuses navigation and preserves the acknowledged baseline`,async t=>{
  const f=await setup(t),{z,lists,saves}=f
  if(dirty)z.doc.value.name='Submitted'
  const launching=z.launchDesign();await flush()
  const saved=dirty?await f.ack():file(),baseline=JSON.stringify(z.doc.value)
  const collision='Identité de flow concurrente : flow. Choisissez une conversion ou une identité distincte.'
  // The store retains composition on identity collisions; a different key is not a fallback.
  lists[0].reply.resolve(json([{...saved,diagnostics:[collision]},{...saved,key:'different-key'}]));f.readyCapabilities();await launching
  assert.equal(z.mode.value,'design');assert.equal(z.error.value,collision)
  assert.equal(z.dirty.value,false);assert.equal(JSON.stringify(z.doc.value),baseline)
  assert.equal(z.designFile.value?.hash,saved.hash);assert.equal(z.designFile.value?.key,saved.key)
  assert.equal(z.notice.value,dirty?'Version envoyée du flow enregistrée':'')
  assert.equal(saves.length,dirty?1:0);assert.equal(lists.length,1)
})
